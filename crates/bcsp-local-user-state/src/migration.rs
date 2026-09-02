use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use sha2::{Digest, Sha256};
#[cfg(test)]
use std::sync::{Arc, Barrier, Mutex};

use crate::{
    PERSONAL_MIGRATION_ID_BASE, PERSONAL_MIGRATION_LEDGER_TABLE, PersonalMigrationRecord,
    PersonalStateError, PersonalStateResult,
};

const CREATE_LEDGER_SQL: &str = "CREATE TABLE IF NOT EXISTS personal_migration_ledger (
    migration_id INTEGER PRIMARY KEY CHECK (migration_id > 0),
    name TEXT NOT NULL UNIQUE CHECK (length(name) > 0),
    checksum TEXT NOT NULL CHECK (length(checksum) = 64),
    applied_at TEXT NOT NULL
) STRICT;";

struct EmbeddedMigration {
    id: u32,
    name: &'static str,
    sql: &'static str,
    after_sql: Option<fn(&Transaction<'_>) -> PersonalStateResult<()>>,
}

const MIGRATIONS: &[EmbeddedMigration] = &[
    EmbeddedMigration {
        // Personal migrations occupy a disjoint namespace from operational 1..N.
        id: PERSONAL_MIGRATION_ID_BASE + 1,
        name: "personal_state",
        sql: include_str!("../migrations/0001_personal_state.sql"),
        after_sql: None,
    },
    EmbeddedMigration {
        id: PERSONAL_MIGRATION_ID_BASE + 2,
        name: "saved_views",
        sql: include_str!("../migrations/0002_saved_views.sql"),
        after_sql: Some(crate::saved_view::migrate_legacy_current_filters),
    },
    EmbeddedMigration {
        id: PERSONAL_MIGRATION_ID_BASE + 3,
        name: "desired_watches",
        sql: include_str!("../migrations/0003_desired_watches.sql"),
        after_sql: None,
    },
    EmbeddedMigration {
        id: PERSONAL_MIGRATION_ID_BASE + 4,
        name: "desired_watch_authority",
        sql: include_str!("../migrations/0004_desired_watch_authority.sql"),
        after_sql: None,
    },
];

/// Test-only rendezvous placed between the unlocked ledger read and
/// `BEGIN IMMEDIATE`. Without it a race test cannot prove both openers were
/// ever in the window at the same time, so it would pass against the broken
/// implementation too.
#[cfg(test)]
pub(crate) static PRE_LOCK_RENDEZVOUS: Mutex<Option<Arc<Barrier>>> = Mutex::new(None);

pub(crate) fn apply_migrations(connection: &mut Connection) -> PersonalStateResult<()> {
    connection.execute_batch(CREATE_LEDGER_SQL)?;
    let applied = read_migration_records(connection)?;
    verify_applied_prefix(&applied)?;

    if applied.len() == MIGRATIONS.len() {
        return Ok(());
    }

    #[cfg(test)]
    {
        let rendezvous = PRE_LOCK_RENDEZVOUS
            .lock()
            .expect("rendezvous mutex")
            .clone();
        if let Some(rendezvous) = rendezvous {
            rendezvous.wait();
        }
    }

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    // Re-read the ledger now that we hold the write lock. The check above ran
    // unlocked, so a second opener could have applied the pending migrations
    // in between; without this, the loser of that race re-runs them and the
    // process fails to start.
    let applied = read_migration_records(&transaction)?;
    verify_applied_prefix(&applied)?;
    for migration in MIGRATIONS.iter().skip(applied.len()) {
        transaction.execute_batch(migration.sql)?;
        if let Some(after_sql) = migration.after_sql {
            after_sql(&transaction)?;
        }
        transaction.execute(
            "INSERT INTO personal_migration_ledger
                (migration_id, name, checksum, applied_at)
             VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![
                migration.id,
                migration.name,
                migration_checksum(migration.sql)
            ],
        )?;
    }
    transaction.commit()?;
    Ok(())
}

pub(crate) fn read_migration_records(
    connection: &Connection,
) -> PersonalStateResult<Vec<PersonalMigrationRecord>> {
    let ledger_exists = connection
        .query_row(
            "SELECT 1 FROM sqlite_schema WHERE type = 'table' AND name = ?1",
            [PERSONAL_MIGRATION_LEDGER_TABLE],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !ledger_exists {
        return Ok(Vec::new());
    }

    let mut statement = connection.prepare(
        "SELECT migration_id, name, checksum
         FROM personal_migration_ledger
         ORDER BY migration_id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(PersonalMigrationRecord {
            migration_id: row.get(0)?,
            name: row.get(1)?,
            checksum: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn verify_applied_prefix(applied: &[PersonalMigrationRecord]) -> PersonalStateResult<()> {
    for (index, record) in applied.iter().enumerate() {
        let Some(expected) = MIGRATIONS.get(index) else {
            return Err(PersonalStateError::UnknownMigration {
                migration_id: record.migration_id,
                name: record.name.clone(),
            });
        };
        if record.migration_id != expected.id {
            return Err(PersonalStateError::InvalidMigrationSequence);
        }
        if record.name != expected.name {
            return Err(PersonalStateError::MigrationNameMismatch {
                migration_id: record.migration_id,
            });
        }
        if record.checksum != migration_checksum(expected.sql) {
            return Err(PersonalStateError::MigrationChecksumMismatch {
                migration_id: record.migration_id,
            });
        }
    }
    Ok(())
}

fn migration_checksum(sql: &str) -> String {
    // Canonicalize CRLF so the real SHA-256 remains portable across Windows checkouts.
    let mut canonical = Vec::with_capacity(sql.len());
    let bytes = sql.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
            index += 1;
            continue;
        }
        canonical.push(bytes[index]);
        index += 1;
    }
    format!("{:x}", Sha256::digest(canonical))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_is_portable_across_line_endings() {
        assert_eq!(
            migration_checksum("a\nb\n"),
            migration_checksum("a\r\nb\r\n")
        );
    }

    /// Both openers are held at the rendezvous until each has finished the
    /// UNLOCKED ledger read, so both provably enter the window this fix
    /// exists for. Against the pre-fix code -- which skipped by the count it
    /// read outside the lock -- the loser re-runs 10004 and its ledger insert
    /// collides, so one opener fails. Against the fix, both start.
    #[test]
    fn two_openers_that_both_read_a_stale_prefix_still_both_start() {
        let directory = tempfile::TempDir::new().expect("temp dir");
        let path = directory.path().join("rbcsp.sqlite");
        drop(crate::PersonalStateStore::open(&path).expect("initial open"));

        // Rewind one migration so there is something pending to race on.
        let connection = Connection::open(&path).expect("rewind connection");
        connection
            .execute_batch(
                "DELETE FROM personal_migration_ledger WHERE migration_id = 10004;
                 DROP TABLE personal_desired_watch_receipts_v1;
                 DROP TABLE personal_desired_watches_v1;
                 CREATE TABLE personal_desired_watches_v1 (
                     term_id TEXT NOT NULL,
                     campus_code TEXT NOT NULL,
                     section_index TEXT NOT NULL,
                     policy_json TEXT NOT NULL CHECK (json_valid(policy_json)),
                     PRIMARY KEY (term_id, campus_code, section_index)
                 ) STRICT;
                 ALTER TABLE personal_state_metadata_v1 RENAME TO personal_state_metadata_post;
                 CREATE TABLE personal_state_metadata_v1 (
                     singleton_id INTEGER PRIMARY KEY CHECK (singleton_id = 1),
                     state_revision INTEGER NOT NULL CHECK (state_revision > 0)
                 ) STRICT;
                 INSERT INTO personal_state_metadata_v1(singleton_id, state_revision)
                     SELECT singleton_id, state_revision FROM personal_state_metadata_post;
                 DROP TABLE personal_state_metadata_post;",
            )
            .expect("rewind to 10003");
        drop(connection);

        *PRE_LOCK_RENDEZVOUS.lock().expect("rendezvous mutex") = Some(Arc::new(Barrier::new(2)));
        let outcomes = std::thread::scope(|scope| {
            let handles = (0..2)
                .map(|_| {
                    let path = path.clone();
                    scope.spawn(move || crate::PersonalStateStore::open(&path).map(|_| ()))
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("opener thread"))
                .collect::<Vec<_>>()
        });
        *PRE_LOCK_RENDEZVOUS.lock().expect("rendezvous mutex") = None;

        for outcome in outcomes {
            outcome.expect("both openers start");
        }

        let store = crate::PersonalStateStore::open(&path).expect("final open");
        let migrations = store.migration_records().expect("ledger");
        assert_eq!(migrations.len(), 4, "10004 applied exactly once");
        assert_eq!(migrations[3].migration_id, 10_004);
    }
}
