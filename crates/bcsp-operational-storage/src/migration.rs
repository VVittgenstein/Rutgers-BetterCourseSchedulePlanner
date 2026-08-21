use rusqlite::{Connection, TransactionBehavior, params};
use sha2::{Digest, Sha256};

use crate::migration_bundle::BUNDLED_MIGRATIONS;
use crate::{MigrationRecord, StorageError, StorageResult};

/// Marker comment on the first line of a migration's SQL requesting that the
/// runner disable foreign-key enforcement OUTSIDE the migration transaction
/// (required for twelve-step table rebuilds of parents with ON DELETE CASCADE
/// children: an in-transaction `PRAGMA foreign_keys` is a no-op, and dropping
/// the parent with enforcement on would silently empty the children).
const FOREIGN_KEYS_OFF_MARKER: &str = "-- bcsp:requires_foreign_keys_off";

#[derive(Debug)]
struct EmbeddedMigration {
    id: u32,
    name: &'static str,
    sql: &'static str,
    sha256: String,
    requires_foreign_keys_off: bool,
}

fn embedded_migrations() -> StorageResult<Vec<EmbeddedMigration>> {
    let mut migrations = BUNDLED_MIGRATIONS
        .iter()
        .map(|bundled| {
            let filename = bundled.filename;
            let stem = filename
                .strip_suffix(".sql")
                .ok_or(StorageError::InvalidMigrationSequence)?;
            let (id, name) = stem
                .split_once('_')
                .ok_or(StorageError::InvalidMigrationSequence)?;
            let id = id
                .parse::<u32>()
                .map_err(|_| StorageError::InvalidMigrationSequence)?;
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
            {
                return Err(StorageError::InvalidMigrationSequence);
            }
            Ok(EmbeddedMigration {
                id,
                name,
                sql: bundled.sql,
                sha256: sha256_hex(bundled.sql.as_bytes()),
                requires_foreign_keys_off: bundled
                    .sql
                    .lines()
                    .next()
                    .is_some_and(|line| line.trim() == FOREIGN_KEYS_OFF_MARKER),
            })
        })
        .collect::<StorageResult<Vec<_>>>()?;
    migrations.sort_by_key(|migration| migration.id);
    if migrations.is_empty()
        || migrations
            .iter()
            .enumerate()
            .any(|(index, migration)| migration.id != (index + 1) as u32)
    {
        return Err(StorageError::InvalidMigrationSequence);
    }
    Ok(migrations)
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub(crate) fn probe_fts5(connection: &Connection) -> StorageResult<()> {
    connection
        .execute_batch(
            "DROP TABLE IF EXISTS temp.bcsp_fts5_capability_probe;
             CREATE VIRTUAL TABLE temp.bcsp_fts5_capability_probe USING fts5(value);
             DROP TABLE temp.bcsp_fts5_capability_probe;",
        )
        .map_err(|_| StorageError::Fts5Unavailable)
}

pub(crate) fn apply_migrations(connection: &mut Connection) -> StorageResult<()> {
    let migrations = embedded_migrations()?;
    let ledger_exists = connection.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_schema
            WHERE type = 'table' AND name = 'bcsp_operational_migrations'
        )",
        [],
        |row| row.get::<_, bool>(0),
    )?;

    let applied = if ledger_exists {
        read_migration_records(connection)?
    } else {
        Vec::new()
    };
    verify_applied_prefix(&migrations, &applied)?;

    if applied.len() == migrations.len() {
        return Ok(());
    }

    // Segmented commits: one transaction and one ledger row per migration,
    // so an interrupted sequence leaves a valid, retryable prefix.
    for migration in migrations.iter().skip(applied.len()) {
        apply_one_migration(connection, migration)?;
    }
    Ok(())
}

fn apply_one_migration(
    connection: &mut Connection,
    migration: &EmbeddedMigration,
) -> StorageResult<()> {
    if migration.requires_foreign_keys_off {
        // Must happen outside any transaction, with a readback: an ignored
        // in-transaction pragma would let the rebuild cascade-empty children.
        connection.execute_batch("PRAGMA foreign_keys = OFF;")?;
        ensure_foreign_keys_state(connection, false)?;
    }
    let applied = run_migration_transaction(connection, migration);
    let restored = if migration.requires_foreign_keys_off {
        // Restore enforcement on success AND failure. If restoration itself
        // fails, the returned error makes the caller abandon this connection,
        // so a connection with enforcement off never escapes the runner.
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .map_err(StorageError::from)
            .and_then(|()| ensure_foreign_keys_state(connection, true))
    } else {
        Ok(())
    };
    applied.and(restored)
}

fn run_migration_transaction(
    connection: &mut Connection,
    migration: &EmbeddedMigration,
) -> StorageResult<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute_batch(migration.sql)?;
    if migration.requires_foreign_keys_off {
        verify_no_foreign_key_violations(&transaction, migration.id)?;
    }
    transaction.execute(
        "INSERT INTO bcsp_operational_migrations
            (migration_id, name, sha256, applied_at)
         VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
        params![migration.id, migration.name, migration.sha256],
    )?;
    transaction.commit()?;
    Ok(())
}

fn ensure_foreign_keys_state(connection: &Connection, expected: bool) -> StorageResult<()> {
    let state = connection.pragma_query_value(None, "foreign_keys", |row| row.get::<_, bool>(0))?;
    if state != expected {
        return Err(StorageError::SqliteConfiguration("foreign_keys"));
    }
    Ok(())
}

/// Runs `PRAGMA foreign_key_check` and walks EVERY result row (enforcement is
/// off, so this is the only integrity net for the rebuild), failing the
/// migration when any violation exists.
fn verify_no_foreign_key_violations(
    transaction: &rusqlite::Transaction<'_>,
    migration_id: u32,
) -> StorageResult<()> {
    let mut statement = transaction.prepare("PRAGMA foreign_key_check")?;
    let mut rows = statement.query([])?;
    let mut violation_count: u64 = 0;
    let mut first_table: Option<String> = None;
    while let Some(row) = rows.next()? {
        violation_count += 1;
        if first_table.is_none() {
            first_table = Some(row.get::<_, String>(0)?);
        }
    }
    if violation_count > 0 {
        return Err(StorageError::MigrationForeignKeyViolations {
            migration_id,
            violation_count,
            first_table: first_table.unwrap_or_default(),
        });
    }
    Ok(())
}

pub(crate) fn read_migration_records(
    connection: &Connection,
) -> StorageResult<Vec<MigrationRecord>> {
    let mut statement = connection.prepare(
        "SELECT migration_id, name, sha256
         FROM bcsp_operational_migrations
         ORDER BY migration_id",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(MigrationRecord {
            migration_id: row.get(0)?,
            name: row.get(1)?,
            sha256: row.get(2)?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn verify_applied_prefix(
    embedded: &[EmbeddedMigration],
    applied: &[MigrationRecord],
) -> StorageResult<()> {
    for (index, record) in applied.iter().enumerate() {
        let Some(expected) = embedded.get(index) else {
            return Err(StorageError::UnknownMigration {
                migration_id: record.migration_id,
                name: record.name.clone(),
            });
        };
        if record.migration_id != expected.id {
            return Err(StorageError::InvalidMigrationSequence);
        }
        if record.name != expected.name {
            return Err(StorageError::MigrationNameMismatch {
                migration_id: record.migration_id,
            });
        }
        if record.sha256 != expected.sha256 {
            return Err(StorageError::MigrationChecksumMismatch {
                migration_id: record.migration_id,
            });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::*;

    #[test]
    fn embedded_migration_ids_are_contiguous_and_checksums_are_lower_hex() {
        let migrations = embedded_migrations().expect("valid embedded migrations");
        assert_eq!(migrations.len(), 5);
        for (index, migration) in migrations.iter().enumerate() {
            assert_eq!(migration.id, (index + 1) as u32);
        }
        assert!(migrations.iter().all(|migration| {
            migration.sha256.len() == 64
                && migration
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        }));
        // The foreign-keys-off marker is parsed from the SQL itself and only
        // the table-rebuild migration carries it.
        assert!(
            migrations
                .iter()
                .map(|migration| (migration.id, migration.requires_foreign_keys_off))
                .eq([(1, false), (2, false), (3, false), (4, false), (5, true)])
        );
    }

    fn hex64(fill: char) -> String {
        std::iter::repeat_n(fill, 64).collect()
    }

    /// Builds a database exactly as a legacy (v4) runner left it: first four
    /// migrations applied with ledger rows, plus one attempt row with rows in
    /// BOTH `ON DELETE CASCADE` child tables.
    fn build_v4_database_with_children() -> Connection {
        let mut connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .expect("enable foreign keys");
        let migrations = embedded_migrations().expect("embedded migrations");
        for migration in migrations.iter().take(4) {
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .expect("legacy migration transaction");
            transaction
                .execute_batch(migration.sql)
                .expect("legacy migration SQL");
            transaction
                .execute(
                    "INSERT INTO bcsp_operational_migrations
                        (migration_id, name, sha256, applied_at)
                     VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                    params![migration.id, migration.name, migration.sha256],
                )
                .expect("legacy ledger row");
            transaction.commit().expect("legacy commit");
        }
        connection
            .execute_batch(&format!(
                "INSERT INTO catalog_targets
                    (target_id, term_id, campus_code, created_at, updated_at)
                 VALUES ('t1', '92026', 'NB', 'ts', 'ts');
                 INSERT INTO open_pull_attempts
                    (attempt_id, target_id, run_id, attempt_sequence, rutgers_day,
                     captured_catalog_content_version, started_at, completed_at,
                     classification, lane)
                 VALUES ('a1', 't1', 'r1', 1, '2026-08-19', 1, 'ts', 'ts',
                         'VALID_APPLIED', 'GENERAL');
                 INSERT INTO open_attempt_catalog_sections (attempt_id, section_index)
                 VALUES ('a1', '12345'), ('a1', '23456');
                 INSERT INTO open_batch_observations
                    (attempt_id, observation_id, target_id, observation_sequence,
                     catalog_content_version, observed_at, classification, http_status,
                     decoded_bytes, decoded_body_sha256, content_type,
                     canonical_set_sha256, state_sha256, source_value_count,
                     catalog_section_count, intersection_count, orphan_count,
                     duplicate_count, changed_section_count, body_changed, state_changed)
                 VALUES ('a1', 'o1', 't1', 1, 1, 'ts', 'VALID_APPLIED', 200,
                         10, '{h}', 'application/json', '{h}', '{h}', 2, 2, 2, 0, 0, 0, 1, 1);",
                h = hex64('a'),
            ))
            .expect("seed v4 parent and children");
        connection
    }

    fn count(connection: &Connection, sql: &str) -> i64 {
        connection
            .query_row(sql, [], |row| row.get(0))
            .expect("count query")
    }

    #[test]
    fn v4_to_v5_upgrade_preserves_children_indexes_and_columns() {
        let mut connection = build_v4_database_with_children();
        // Pre-assert the v4 CHECK really rejects the new classification.
        let rejected = connection.execute(
            "INSERT INTO open_pull_attempts
                (attempt_id, target_id, run_id, attempt_sequence, rutgers_day,
                 captured_catalog_content_version, started_at, classification, lane)
             VALUES ('a2', 't1', 'r1', 2, '2026-08-19', 1, 'ts',
                     'SUSPECT_PARTIAL_SNAPSHOT', 'GENERAL')",
            [],
        );
        assert!(rejected.is_err(), "v4 CHECK must reject the new value");

        apply_migrations(&mut connection).expect("v4 -> v5 upgrade");

        assert_eq!(count(&connection, "SELECT COUNT(*) FROM bcsp_operational_migrations"), 5);
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM open_pull_attempts"), 1);
        // Cascade preservation: both child tables keep their rows.
        assert_eq!(
            count(&connection, "SELECT COUNT(*) FROM open_attempt_catalog_sections"),
            2
        );
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM open_batch_observations"), 1);
        // Enforcement restored and the net is clean.
        let fk_on = connection
            .pragma_query_value(None, "foreign_keys", |row| row.get::<_, bool>(0))
            .expect("foreign_keys pragma");
        assert!(fk_on, "foreign key enforcement restored after the rebuild");
        let violations = count(
            &connection,
            "SELECT COUNT(*) FROM pragma_foreign_key_check()",
        );
        assert_eq!(violations, 0);
        // 0004 columns survive the rebuild.
        let candidate_columns = count(
            &connection,
            "SELECT COUNT(*) FROM pragma_table_info('open_pull_attempts')
             WHERE name IN ('candidate_catalog_observation_id', 'candidate_base_content_version')",
        );
        assert_eq!(candidate_columns, 2);
        // All three indexes were recreated.
        assert_eq!(
            count(
                &connection,
                "SELECT COUNT(*) FROM sqlite_schema
                 WHERE type = 'index' AND tbl_name = 'open_pull_attempts'
                   AND name LIKE 'open_pull_attempts_%'",
            ),
            3
        );
        // The rebuilt CHECK accepts the new classification.
        connection
            .execute(
                "INSERT INTO open_pull_attempts
                    (attempt_id, target_id, run_id, attempt_sequence, rutgers_day,
                     captured_catalog_content_version, started_at, completed_at,
                     classification, lane)
                 VALUES ('a2', 't1', 'r1', 2, '2026-08-19', 1, 'ts', 'ts',
                         'SUSPECT_PARTIAL_SNAPSHOT', 'GENERAL')",
                [],
            )
            .expect("v5 CHECK admits SUSPECT_PARTIAL_SNAPSHOT");
        // ON DELETE CASCADE still works against the rebuilt parent.
        connection
            .execute("DELETE FROM open_pull_attempts WHERE attempt_id = 'a1'", [])
            .expect("delete parent");
        assert_eq!(
            count(&connection, "SELECT COUNT(*) FROM open_attempt_catalog_sections"),
            0,
            "cascade re-attached to the rebuilt parent",
        );
    }

    #[test]
    fn v5_detects_orphans_fails_cleanly_and_is_retryable() {
        let mut connection = build_v4_database_with_children();
        // Plant an orphan child row (enforcement off so the legacy DB can
        // contain it, as a corrupted store would).
        connection
            .execute_batch(
                "PRAGMA foreign_keys = OFF;
                 INSERT INTO open_attempt_catalog_sections (attempt_id, section_index)
                 VALUES ('ghost', '99999');
                 PRAGMA foreign_keys = ON;",
            )
            .expect("plant orphan child row");

        let error = apply_migrations(&mut connection).expect_err("orphan must fail the rebuild");
        assert!(
            matches!(
                error,
                StorageError::MigrationForeignKeyViolations { migration_id: 5, violation_count, .. }
                    if violation_count > 0
            ),
            "unexpected error: {error:?}",
        );
        // Failure is clean: no 0005 ledger row, data intact, enforcement back on.
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM bcsp_operational_migrations"), 4);
        assert_eq!(
            count(&connection, "SELECT COUNT(*) FROM open_attempt_catalog_sections"),
            3
        );
        let fk_on = connection
            .pragma_query_value(None, "foreign_keys", |row| row.get::<_, bool>(0))
            .expect("foreign_keys pragma");
        assert!(fk_on, "enforcement restored even after a failed rebuild");

        // Remove the orphan and retry: the same runner completes the upgrade.
        connection
            .execute(
                "DELETE FROM open_attempt_catalog_sections WHERE attempt_id = 'ghost'",
                [],
            )
            .expect("repair orphan");
        apply_migrations(&mut connection).expect("retry succeeds after repair");
        assert_eq!(count(&connection, "SELECT COUNT(*) FROM bcsp_operational_migrations"), 5);
        assert_eq!(
            count(&connection, "SELECT COUNT(*) FROM open_attempt_catalog_sections"),
            2
        );
    }

    #[test]
    fn bundled_migrations_match_the_version_controlled_sql_directory_byte_for_byte() {
        let migrations = embedded_migrations().expect("valid bundled migrations");
        let directory = Path::new("migrations");
        let mut audit_filenames = fs::read_dir(directory)
            .expect("version-controlled migration directory")
            .map(|entry| {
                entry
                    .expect("readable migration directory entry")
                    .file_name()
                    .into_string()
                    .expect("UTF-8 migration filename")
            })
            .filter(|filename| filename.ends_with(".sql"))
            .collect::<Vec<_>>();
        audit_filenames.sort();
        assert_eq!(
            audit_filenames,
            BUNDLED_MIGRATIONS
                .iter()
                .map(|migration| migration.filename.to_owned())
                .collect::<Vec<_>>()
        );
        for (migration, bundled) in migrations.into_iter().zip(BUNDLED_MIGRATIONS) {
            let audited = fs::read_to_string(directory.join(bundled.filename))
                .expect("version-controlled migration SQL");
            assert_eq!(audited, migration.sql, "{} drifted", bundled.filename);
            assert_eq!(sha256_hex(audited.as_bytes()), migration.sha256);
        }
    }

    #[test]
    fn fts5_probe_failure_is_an_explicit_capability_error() {
        let connection = Connection::open_in_memory().expect("in-memory SQLite");
        connection
            .execute_batch(
                "CREATE TEMP VIEW bcsp_fts5_capability_probe AS SELECT 'synthetic' AS value;",
            )
            .expect("install deterministic probe conflict");
        assert!(matches!(
            probe_fts5(&connection),
            Err(StorageError::Fts5Unavailable)
        ));
    }
}
