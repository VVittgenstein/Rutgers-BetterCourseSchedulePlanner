use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use sha2::{Digest, Sha256};

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

pub(crate) fn apply_migrations(connection: &mut Connection) -> PersonalStateResult<()> {
    connection.execute_batch(CREATE_LEDGER_SQL)?;
    let applied = read_migration_records(connection)?;
    verify_applied_prefix(&applied)?;

    if applied.len() == MIGRATIONS.len() {
        return Ok(());
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
}
