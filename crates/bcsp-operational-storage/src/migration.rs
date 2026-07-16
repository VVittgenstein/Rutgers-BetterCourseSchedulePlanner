use rusqlite::{Connection, TransactionBehavior, params};
use sha2::{Digest, Sha256};

use crate::migration_bundle::BUNDLED_MIGRATIONS;
use crate::{MigrationRecord, StorageError, StorageResult};

#[derive(Debug)]
struct EmbeddedMigration {
    id: u32,
    name: &'static str,
    sql: &'static str,
    sha256: String,
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

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    for migration in migrations.iter().skip(applied.len()) {
        transaction.execute_batch(migration.sql)?;
        transaction.execute(
            "INSERT INTO bcsp_operational_migrations
                (migration_id, name, sha256, applied_at)
             VALUES (?1, ?2, ?3, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
            params![migration.id, migration.name, migration.sha256],
        )?;
    }
    transaction.commit()?;
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
        assert_eq!(migrations.len(), 3);
        assert_eq!(migrations[0].id, 1);
        assert_eq!(migrations[1].id, 2);
        assert_eq!(migrations[2].id, 3);
        assert!(migrations.iter().all(|migration| {
            migration.sha256.len() == 64
                && migration
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        }));
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
