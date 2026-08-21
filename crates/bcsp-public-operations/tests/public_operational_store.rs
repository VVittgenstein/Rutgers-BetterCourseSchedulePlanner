use std::fs;
use std::path::Path;

use bcsp_public_operations::{
    PUBLIC_DATABASE_FILENAME, PUBLIC_DATABASE_PATH, PUBLIC_STATE_ROOT, PublicOperationalStore,
    PublicOperationsError,
};
use rusqlite::Connection;
use tempfile::TempDir;

const DAY: &str = "2026-07-14";

#[test]
fn first_start_creates_only_an_empty_operational_database() {
    let temp = TempDir::new().expect("temporary directory");
    let state_root = temp.path().join("service-state");

    let store = PublicOperationalStore::open_for_state_root(&state_root)
        .expect("first public operational start");

    assert_eq!(store.state_root(), state_root);
    assert_eq!(
        store.database_path(),
        state_root.join(PUBLIC_DATABASE_FILENAME)
    );
    assert!(store.database_path().is_file());
    assert_eq!(
        store
            .storage()
            .migration_records()
            .expect("migrations")
            .len(),
        5
    );

    let tables = store
        .storage()
        .operational_table_names()
        .expect("operational tables");
    assert!(!tables.is_empty());
    assert!(tables.iter().all(|name| {
        let name = name.to_ascii_lowercase();
        !name.contains("personal")
            && !name.contains("saved")
            && !name.contains("selection")
            && !name.contains("history")
            && !name.contains("setting")
    }));

    let connection = Connection::open(store.database_path()).expect("inspect first-start data");
    for table in tables {
        if matches!(
            table.as_str(),
            "bcsp_operational_migrations" | "catalog_discovery_state"
        ) {
            continue;
        }
        let quoted = table.replace('"', "\"\"");
        let row_count = connection
            .query_row(&format!("SELECT count(*) FROM \"{quoted}\""), [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap_or_else(|error| panic!("count {table}: {error}"));
        assert_eq!(row_count, 0, "{table} must be empty before network work");
    }

    let discovery_state = connection
        .query_row(
            "SELECT content_version, semantic_sha256, last_attempt_observation_id,
                    last_success_observation_id, last_published_observation_id,
                    last_nonempty_observation_id
             FROM catalog_discovery_state WHERE singleton = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .expect("empty discovery sentinel");
    assert_eq!(discovery_state, (0, None, None, None, None, None));
}

#[test]
fn service_day_counters_survive_a_process_restart() {
    let temp = TempDir::new().expect("temporary directory");
    let state_root = temp.path().join("service-state");
    let store = PublicOperationalStore::open_for_state_root(&state_root).expect("first start");
    let database_path = store.database_path().to_path_buf();
    drop(store);

    let connection = Connection::open(&database_path).expect("seed operational facts");
    connection
        .execute(
            "INSERT INTO catalog_targets(
                 target_id, term_id, campus_code, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?4)",
            (
                "TERM_2026_FALL\u{1f}CAMPUS_NEW_BRUNSWICK",
                "TERM_2026_FALL",
                "CAMPUS_NEW_BRUNSWICK",
                "2026-07-14T00:00:00Z",
            ),
        )
        .expect("insert operational target");
    connection
        .execute(
            "INSERT INTO open_daily_counters(
                 target_id, rutgers_day, attempted, succeeded, failed, empty
             ) VALUES (?1, ?2, 9, 6, 3, 1)",
            ("TERM_2026_FALL\u{1f}CAMPUS_NEW_BRUNSWICK", DAY),
        )
        .expect("insert durable service counters");
    drop(connection);

    let restarted =
        PublicOperationalStore::open_for_state_root(&state_root).expect("service restart");
    let counters = restarted
        .service_day_counters(DAY)
        .expect("service-wide counters");
    assert_eq!(
        (
            counters.attempted,
            counters.succeeded,
            counters.failed,
            counters.empty,
        ),
        (9, 6, 3, 1)
    );
    assert_eq!(
        restarted
            .storage()
            .open_service_day_counters("2026-07-15")
            .expect("new day defaults"),
        Default::default()
    );
}

#[test]
fn state_path_failure_is_redacted_and_never_falls_back() {
    let temp = TempDir::new().expect("temporary directory");
    let blocked_root = temp.path().join("blocked-state-root");
    fs::write(&blocked_root, b"not a directory").expect("blocking file");

    let error = PublicOperationalStore::open_for_state_root(&blocked_root)
        .err()
        .expect("blocked root must fail");
    assert!(matches!(
        error,
        PublicOperationsError::CreateStateRoot { .. }
    ));
    assert_eq!(error.code(), "PUBLIC_STATE_ROOT_UNAVAILABLE");
    assert_eq!(
        error.to_string(),
        "public operational state root could not be created"
    );
    assert!(
        !error
            .to_string()
            .contains(temp.path().to_string_lossy().as_ref())
    );
    assert!(!blocked_root.join(PUBLIC_DATABASE_FILENAME).exists());

    let relative = Path::new("relative-public-state");
    let error = PublicOperationalStore::open_for_state_root(relative)
        .err()
        .expect("relative root must fail");
    assert!(matches!(error, PublicOperationsError::RelativeStateRoot));
    assert!(!relative.exists());
}

#[test]
fn production_path_is_fixed_and_not_configurable_data() {
    assert_eq!(PUBLIC_STATE_ROOT, "/var/lib/bcsp");
    assert_eq!(PUBLIC_DATABASE_PATH, "/var/lib/bcsp/rbcsp.sqlite");
    assert_eq!(
        Path::new(PUBLIC_STATE_ROOT).join(PUBLIC_DATABASE_FILENAME),
        Path::new(PUBLIC_DATABASE_PATH)
    );
}
