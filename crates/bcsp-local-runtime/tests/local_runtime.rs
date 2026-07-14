use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use bcsp_application::OpenRuntimeSnapshot;
use bcsp_local_runtime::{
    LocalRuntimeError, LocalRuntimePaths, PreparedLocalRuntime, prepare_and_start_with,
};
use bcsp_local_user_state::{
    CatalogRefreshMinutes, LocalSettings, OpenRefreshSeconds, PersonalStateStore, SettingsRevision,
};
use bcsp_open::OpenCounterAudience;
use rusqlite::Connection;

static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);
static CURRENT_DIRECTORY_LOCK: Mutex<()> = Mutex::new(());

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "rbcsp-local-runtime-{label}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn package(temp: &TestDirectory) -> (PathBuf, PathBuf) {
    let root = temp.path().join("RBCSP 课程 包");
    fs::create_dir_all(&root).unwrap();
    let executable = root.join("RBCSP.exe");
    fs::write(&executable, b"test executable").unwrap();
    (root.canonicalize().unwrap(), executable)
}

#[test]
fn package_paths_ignore_the_working_directory_and_restart_empty() {
    let temp = TestDirectory::new("paths");
    let (root, executable) = package(&temp);
    let elsewhere = temp.path().join("unrelated cwd");
    fs::create_dir_all(&elsewhere).unwrap();

    let _cwd_lock = CURRENT_DIRECTORY_LOCK.lock().unwrap();
    let original = std::env::current_dir().unwrap();
    std::env::set_current_dir(&elsewhere).unwrap();
    let paths = LocalRuntimePaths::from_executable(&executable).unwrap();
    std::env::set_current_dir(original).unwrap();

    assert_eq!(paths.package_root(), root);
    assert_eq!(paths.database(), root.join("data/rbcsp.sqlite"));
    let prepared = PreparedLocalRuntime::open(paths.clone()).unwrap();
    assert_eq!(prepared.state().active_watch_count, 0);
    drop(prepared);

    let store = PersonalStateStore::open(paths.database()).unwrap();
    let counts = store.personal_table_counts().unwrap();
    assert_eq!(counts.settings, 0);
    assert_eq!(counts.selected_sections, 0);
    assert_eq!(counts.episode_summaries, 0);
    assert_eq!(counts.episode_actions, 0);
    drop(store);
    assert_operational_business_tables_are_empty(paths.database());

    let restarted = PreparedLocalRuntime::open(paths).unwrap();
    assert_eq!(restarted.state().active_watch_count, 0);
    drop(restarted);
    assert_eq!(
        find_sqlite_files(temp.path()),
        vec![root.join("data/rbcsp.sqlite")]
    );
}

#[tokio::test]
async fn failed_storage_gate_never_starts_network() {
    let temp = TestDirectory::new("gate");
    let (_root, executable) = package(&temp);
    let paths = LocalRuntimePaths::from_executable(executable).unwrap();
    fs::write(paths.data_directory(), b"not a directory").unwrap();
    let calls = AtomicUsize::new(0);

    let result = prepare_and_start_with(paths, |_, _| {
        calls.fetch_add(1, Ordering::SeqCst);
        async { Ok(()) }
    })
    .await;

    assert!(matches!(result, Err(LocalRuntimeError::Bootstrap(_))));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn existing_non_file_database_target_is_rejected() {
    let temp = TestDirectory::new("database-target");
    let (_root, executable) = package(&temp);
    let paths = LocalRuntimePaths::from_executable(executable).unwrap();
    fs::create_dir_all(paths.database()).unwrap();
    assert!(matches!(
        PreparedLocalRuntime::open(paths),
        Err(LocalRuntimeError::Bootstrap(_))
    ));
}

#[test]
fn configured_refresh_policy_is_live_bounded_and_scoped_to_a_fresh_run() {
    let temp = TestDirectory::new("refresh-policy");
    let (_root, executable) = package(&temp);
    let prepared = PreparedLocalRuntime::from_executable(&executable).unwrap();

    let initial = prepared.core().refresh_policy().unwrap();
    assert_eq!(initial.catalog_interval(), Duration::from_secs(600));
    assert_eq!(initial.open_general_interval().seconds(), 30);
    assert_eq!(
        initial.effective_open_interval(true),
        Duration::from_secs(10)
    );
    let first_audience = prepared.core().counter_audience();
    assert!(matches!(first_audience, OpenCounterAudience::Local { .. }));

    let database = prepared.operational().database();
    {
        let mut database = database.lock().unwrap();
        let minimum = LocalSettings {
            catalog_refresh_minutes: CatalogRefreshMinutes::try_from(1).unwrap(),
            open_refresh_seconds: OpenRefreshSeconds::try_from(3).unwrap(),
            ..LocalSettings::default()
        };
        database
            .personal_mut()
            .compare_and_swap_settings(SettingsRevision::ZERO, &minimum)
            .unwrap();
    }
    let minimum = prepared.core().refresh_policy().unwrap();
    assert_eq!(minimum.catalog_interval(), Duration::from_secs(60));
    assert_eq!(
        minimum.effective_open_interval(false),
        Duration::from_secs(3)
    );
    assert_eq!(
        minimum.effective_open_interval(true),
        Duration::from_secs(3)
    );

    {
        let mut database = database.lock().unwrap();
        let maximum = LocalSettings {
            catalog_refresh_minutes: CatalogRefreshMinutes::try_from(1_440).unwrap(),
            open_refresh_seconds: OpenRefreshSeconds::try_from(3_600).unwrap(),
            ..LocalSettings::default()
        };
        database
            .personal_mut()
            .compare_and_swap_settings(SettingsRevision::try_from(1).unwrap(), &maximum)
            .unwrap();
    }
    let maximum = prepared.core().refresh_policy().unwrap();
    assert_eq!(maximum.catalog_interval(), Duration::from_secs(86_400));
    assert_eq!(
        maximum.effective_open_interval(false),
        Duration::from_secs(3_600)
    );
    assert_eq!(
        maximum.effective_open_interval(true),
        Duration::from_secs(10)
    );
    let runtime = prepared
        .core()
        .projection_runtime(&OpenRuntimeSnapshot::default())
        .unwrap();
    assert_eq!(runtime.audience, first_audience);
    drop(prepared);

    let restarted = PreparedLocalRuntime::from_executable(executable).unwrap();
    assert_ne!(restarted.core().counter_audience(), first_audience);
    assert_eq!(
        restarted
            .core()
            .refresh_policy()
            .unwrap()
            .open_general_interval()
            .seconds(),
        3_600
    );
    assert_eq!(restarted.state().active_watch_count, 0);
}

#[test]
fn database_symlink_cannot_escape_the_package() {
    let temp = TestDirectory::new("database-link");
    let (_root, executable) = package(&temp);
    let paths = LocalRuntimePaths::from_executable(executable).unwrap();
    fs::create_dir_all(paths.data_directory()).unwrap();
    let outside = temp.path().join("outside.sqlite");
    fs::write(&outside, b"").unwrap();
    if create_file_symlink(&outside, paths.database()).is_err() {
        return;
    }
    assert!(matches!(
        PreparedLocalRuntime::open(paths),
        Err(LocalRuntimeError::Bootstrap(_))
    ));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn loopback_server_exposes_the_local_surface_and_method_boundaries() {
    let temp = TestDirectory::new("server");
    let (_root, executable) = package(&temp);
    let running = PreparedLocalRuntime::from_executable(executable)
        .unwrap()
        .start()
        .await
        .unwrap();
    let origin = running.origin().to_owned();
    let authority = origin.strip_prefix("http://").unwrap();
    let nonce = running.nonce().as_str();

    for path in [
        "/",
        "/runtime.txt",
        "/api/v1/local/bootstrap",
        "/api/v1/local/settings",
        "/api/v1/local/selection",
        "/api/v1/local/history",
    ] {
        let response = request(authority, &format!("GET {path}"), &origin, nonce, "");
        assert_eq!(status(&response), 200, "{path}: {response}");
    }

    let bootstrap = request(authority, "GET /api/v1/local/bootstrap", &origin, nonce, "");
    assert!(bootstrap.contains("\"activeWatchCount\":0"));
    let websocket = websocket_handshake(authority, &origin, nonce);
    assert_eq!(status(&websocket), 101, "{websocket}");

    let settings = serde_json::json!({
        "protocolVersion": 1,
        "payload": {
            "expectedRevision": 0,
            "value": LocalSettings::default(),
        },
    })
    .to_string();
    assert_eq!(
        status(&request(
            authority,
            "PUT /api/v1/local/settings",
            &origin,
            nonce,
            &settings,
        )),
        200
    );
    let selection = r#"{"protocolVersion":1,"payload":{"sections":[{"term":"2026FA","campus":"NB","index":"12345"}]}}"#;
    assert_eq!(
        status(&request(
            authority,
            "PUT /api/v1/local/selection",
            &origin,
            nonce,
            selection,
        )),
        200
    );
    let selected = request(authority, "GET /api/v1/local/selection", &origin, nonce, "");
    assert!(selected.contains("12345"));
    let method_not_allowed = request(
        authority,
        "POST /api/v1/local/settings",
        &origin,
        nonce,
        "{}",
    );
    assert_eq!(status(&method_not_allowed), 405);
    let error: serde_json::Value = serde_json::from_str(body(&method_not_allowed)).unwrap();
    assert_eq!(error["protocolVersion"], 1);
    assert_eq!(error["error"]["code"], "METHOD_NOT_ALLOWED");
    assert_eq!(
        error["error"]["messageKey"],
        "local.error.method_not_allowed"
    );
    assert!(error["error"]["traceId"].as_str().is_some());
    assert_eq!(error["error"]["details"], serde_json::json!([]));
    assert_eq!(
        status(&request(authority, "GET /missing", &origin, nonce, "")),
        404
    );

    running.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn only_an_authenticated_ui_exit_request_signals_ordered_shutdown() {
    let temp = TestDirectory::new("ui-exit");
    let (_root, executable) = package(&temp);
    let mut running = PreparedLocalRuntime::from_executable(executable)
        .unwrap()
        .start()
        .await
        .unwrap();
    let origin = running.origin().to_owned();
    let authority = origin.strip_prefix("http://").unwrap().to_owned();
    let nonce = running.nonce().as_str().to_owned();

    let unauthenticated = request_without_session(&authority, "POST /api/v1/local/exit");
    assert_eq!(status(&unauthenticated), 403);
    assert!(
        tokio::time::timeout(
            Duration::from_millis(50),
            running.wait_for_local_exit_request()
        )
        .await
        .is_err()
    );

    let wrong_method = request(&authority, "GET /api/v1/local/exit", &origin, &nonce, "");
    assert_eq!(status(&wrong_method), 405);
    assert!(
        tokio::time::timeout(
            Duration::from_millis(50),
            running.wait_for_local_exit_request()
        )
        .await
        .is_err()
    );

    let accepted = request(&authority, "POST /api/v1/local/exit", &origin, &nonce, "");
    assert_eq!(status(&accepted), 204);
    tokio::time::timeout(
        Duration::from_secs(1),
        running.wait_for_local_exit_request(),
    )
    .await
    .expect("authenticated exit request must reach the lifecycle")
    .unwrap();

    assert_eq!(
        status(&request(
            &authority,
            "GET /api/v1/local/bootstrap",
            &origin,
            &nonce,
            "",
        )),
        200,
        "the exit route must not bypass centralized graceful shutdown",
    );
    running.shutdown().await.unwrap();
}

fn request(authority: &str, request_line: &str, origin: &str, nonce: &str, body: &str) -> String {
    let mut stream = TcpStream::connect(authority).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "{request_line} HTTP/1.1\r\nHost: {authority}\r\nOrigin: {origin}\r\nx-bcsp-session: {nonce}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .unwrap();
    stream.flush().unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn request_without_session(authority: &str, request_line: &str) -> String {
    let mut stream = TcpStream::connect(authority).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "{request_line} HTTP/1.1\r\nHost: {authority}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    stream.flush().unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn websocket_handshake(authority: &str, origin: &str, nonce: &str) -> String {
    let mut stream = TcpStream::connect(authority).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "GET /api/v1/watch?session={nonce} HTTP/1.1\r\nHost: {authority}\r\nOrigin: {origin}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
    )
    .unwrap();
    stream.flush().unwrap();
    let mut buffer = [0_u8; 2_048];
    let read = stream.read(&mut buffer).unwrap();
    String::from_utf8(buffer[..read].to_vec()).unwrap()
}

fn status(response: &str) -> u16 {
    response.split_whitespace().nth(1).unwrap().parse().unwrap()
}

fn body(response: &str) -> &str {
    response.split_once("\r\n\r\n").unwrap().1
}

fn find_sqlite_files(root: &Path) -> Vec<PathBuf> {
    let mut matches = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().and_then(|value| value.to_str()) == Some("sqlite") {
                matches.push(path.canonicalize().unwrap());
            }
        }
    }
    matches.sort();
    matches
}

fn assert_operational_business_tables_are_empty(database: &Path) {
    const TABLES: &[&str] = &[
        "catalog_discovery_observations",
        "catalog_discovery_source_versions",
        "catalog_discovery_observation_sources",
        "catalog_terms",
        "catalog_campuses",
        "catalog_subjects",
        "catalog_targets",
        "catalog_refresh_observations",
        "catalog_refresh_checkpoints",
        "catalog_staging_payloads",
        "catalog_staging_course_groups",
        "catalog_staging_course_variants",
        "catalog_staging_sections",
        "catalog_staging_occurrences",
        "catalog_staging_provenance",
        "catalog_course_groups",
        "catalog_course_variants",
        "catalog_sections",
        "catalog_occurrences",
        "catalog_provenance",
        "open_batch_state",
        "open_pull_attempts",
        "open_attempt_catalog_sections",
        "open_batch_observations",
        "open_section_current",
        "open_section_events",
        "open_daily_counters",
        "open_run_counters",
        "open_origin_state",
        "open_schedule_state",
    ];
    let connection = Connection::open(database).unwrap();
    for table in TABLES {
        let sql = format!("SELECT COUNT(*) FROM {table}");
        let count = connection
            .query_row(&sql, [], |row| row.get::<_, i64>(0))
            .unwrap();
        assert_eq!(count, 0, "first start must not seed {table}");
    }
}

#[cfg(unix)]
fn create_file_symlink(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(source, target)
}

#[cfg(windows)]
fn create_file_symlink(source: &Path, target: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_file(source, target)
}
