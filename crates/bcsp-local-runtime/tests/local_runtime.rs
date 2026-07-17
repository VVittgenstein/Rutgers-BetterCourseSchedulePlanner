use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use bcsp_application::{
    ExtensionRequest, NoopWatchDispatchSink, OpenRuntimeSnapshot, PRODUCT_CATALOG_DISCOVERY_PATH,
    PRODUCT_FILTER_SCHEMA_PATH, PRODUCT_SERVICE_STATUS_PATH, RequestMethod, RouteExtension,
    SharedWatchSocket, TargetRefreshDemand, WebSocketExtension,
};
use bcsp_contracts::{
    CampusCode, CatalogDiscoveryRequestV1, FilterRequestV1, FilterValuesInputV1,
    HttpRequestEnvelope, NormalizedFilterValuesV1, SectionKey, TermCampusKey, TermId, TraceId,
    WatchClientCommandV1, WatchPolicyV1, WatchStartItemV1, WatchStartItemsV1, WsClientEnvelope,
};
use bcsp_domain::{RutgersTermWindow, RutgersTermWindowScope};
use bcsp_local_runtime::{
    LocalRuntimeError, LocalRuntimePaths, LocalSurfaceState, PersonalSurface, PreparedLocalRuntime,
    prepare_and_start_with,
};
use bcsp_local_user_state::{
    CatalogRefreshMinutes, LocalSettings, OpenRefreshSeconds, PersonalStateStore, SettingsRevision,
    UserStateRevision, WatchFastLaneSeconds,
};
use bcsp_open::OpenCounterAudience;
use bcsp_operational_storage::{
    DiscoveredCampus, DiscoveredTerm, DiscoveryRefreshCommand, DiscoverySnapshot,
    DiscoverySourceKind, DiscoverySourceVersion,
};
use bcsp_watch::WatchStartAdmission;
use rusqlite::{Connection, TransactionBehavior};
use time::OffsetDateTime;
use tokio::sync::mpsc;

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

fn trace(value: u64) -> TraceId {
    TraceId::from_str(&format!("00000000-0000-4000-8000-{value:012x}"))
        .expect("valid deterministic UUIDv4")
}

fn filter_request(term: &str) -> FilterRequestV1 {
    let values = FilterValuesInputV1::for_term(TermId::try_from(term).unwrap());
    FilterRequestV1::new(NormalizedFilterValuesV1::try_new(values).unwrap())
}

fn manual_term_discovery(term: TermId) -> DiscoverySnapshot {
    let source_digest = "a".repeat(64);
    let source_version_id = format!("selector:{source_digest}");
    let source = DiscoverySourceVersion {
        source_version_id: source_version_id.clone(),
        source_kind: DiscoverySourceKind::Selector,
        source_identity: "RUTGERS_SELECTOR".to_owned(),
        content_sha256: source_digest,
        canonical_facts: serde_json::json!({"test": true}),
        observed_at: "2026-07-17T00:00:00Z".to_owned(),
    };
    let campus = |code: &str| DiscoveredCampus {
        target: TermCampusKey::new(term.clone(), CampusCode::try_from(code).unwrap()),
        display_name: Some(code.to_owned()),
        category: None,
        enabled: Some(true),
        canonical_facts: serde_json::json!({"test": true}),
        source_version_id: source_version_id.clone(),
    };
    DiscoverySnapshot {
        sources: vec![source],
        terms: vec![DiscoveredTerm {
            term_id: term.clone(),
            year: None,
            term_code: None,
            display_name: Some(term.as_str().to_owned()),
            published: Some(true),
            canonical_facts: serde_json::json!({"test": true}),
            source_version_id: source_version_id.clone(),
        }],
        campuses: vec![campus("NB"), campus("ONLINE_NB")],
        subjects: Vec::new(),
    }
}

fn seed_ready_query_scope(prepared: &PreparedLocalRuntime, terms: &[&str]) {
    const STARTED: &str = "2026-07-17T00:00:00Z";
    const COMPLETED: &str = "2026-07-17T00:00:01Z";
    let source_digest = "b".repeat(64);
    let source_version_id = format!("selector:{source_digest}");
    let terms = terms
        .iter()
        .map(|term| TermId::try_from(*term).unwrap())
        .collect::<Vec<_>>();
    let snapshot = DiscoverySnapshot {
        sources: vec![DiscoverySourceVersion {
            source_version_id: source_version_id.clone(),
            source_kind: DiscoverySourceKind::Selector,
            source_identity: "RUTGERS_SELECTOR".to_owned(),
            content_sha256: source_digest,
            canonical_facts: serde_json::json!({"test": true}),
            observed_at: STARTED.to_owned(),
        }],
        terms: terms
            .iter()
            .map(|term| DiscoveredTerm {
                term_id: term.clone(),
                year: None,
                term_code: None,
                display_name: Some(term.as_str().to_owned()),
                published: Some(true),
                canonical_facts: serde_json::json!({"test": true}),
                source_version_id: source_version_id.clone(),
            })
            .collect(),
        campuses: terms
            .iter()
            .map(|term| DiscoveredCampus {
                target: TermCampusKey::new(
                    term.clone(),
                    CampusCode::try_from("NB").expect("NB Campus"),
                ),
                display_name: Some("New Brunswick".to_owned()),
                category: None,
                enabled: Some(true),
                canonical_facts: serde_json::json!({"test": true}),
                source_version_id: source_version_id.clone(),
            })
            .collect(),
        subjects: Vec::new(),
    };
    let database = prepared.operational().database();
    let mut database = database.lock().unwrap();
    database
        .operational_mut()
        .apply_discovery_refresh(DiscoveryRefreshCommand {
            observation_id: trace(0x800),
            started_at: STARTED.to_owned(),
            completed_at: COMPLETED.to_owned(),
            snapshot,
        })
        .expect("publish ready-scope discovery fixture");
    drop(database);

    let connection = Connection::open(prepared.paths().database()).expect("fixture connection");
    for (position, term) in terms.into_iter().enumerate() {
        let target_id = format!("{term}/NB");
        let attempt_id = trace(0x810 + u64::try_from(position).unwrap());
        connection
            .execute(
                "INSERT INTO catalog_targets
                    (target_id, term_id, campus_code, created_at, updated_at,
                     current_content_version)
                 VALUES (?1, ?2, 'NB', ?3, ?3, 1)",
                (&target_id, term.as_str(), COMPLETED),
            )
            .expect("seed ready Catalog target");
        connection
            .execute(
                "INSERT INTO open_batch_state
                    (target_id, last_attempt_sequence, last_observation_sequence,
                     current_catalog_content_version, lkg_attempt_id,
                     lkg_observation_sequence, lkg_observed_at,
                     lkg_canonical_set_sha256, lkg_state_sha256,
                     last_attempt_id, last_attempt_at, last_success_at)
                 VALUES (?1, 1, 1, 1, ?2, 1, ?3, ?4, ?5, ?2, ?3, ?3)",
                (
                    &target_id,
                    attempt_id.to_string(),
                    COMPLETED,
                    "c".repeat(64),
                    "d".repeat(64),
                ),
            )
            .expect("seed matching Open LKG");
    }
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

#[test]
fn local_manual_term_pull_is_window_bounded_idempotent_and_excludes_online_aliases() {
    let temp = TestDirectory::new("manual-term-pull");
    let (_root, executable) = package(&temp);
    let prepared = PreparedLocalRuntime::from_executable(executable).unwrap();
    let window = RutgersTermWindow::at(OffsetDateTime::now_utc(), RutgersTermWindowScope::Local)
        .expect("test execution date is covered by the bundled calendar");
    let manual_term = window
        .visible_terms()
        .iter()
        .find(|term| !term.auto_managed())
        .expect("Local window exposes manual terms")
        .term()
        .clone();
    let database = prepared.operational().database();
    database
        .lock()
        .unwrap()
        .operational_mut()
        .apply_discovery_refresh(DiscoveryRefreshCommand {
            observation_id: trace(0x701),
            started_at: "2026-07-17T00:00:00Z".to_owned(),
            completed_at: "2026-07-17T00:00:01Z".to_owned(),
            snapshot: manual_term_discovery(manual_term.clone()),
        })
        .expect("publish manual-term discovery fixture");
    let demand = TargetRefreshDemand::default();
    let mutation_store = PersonalStateStore::open(prepared.paths().database()).unwrap();
    let surface = PersonalSurface::new(
        database,
        mutation_store,
        Arc::new(
            SharedWatchSocket::try_new(
                Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
                Arc::new(NoopWatchDispatchSink),
            )
            .unwrap(),
        ),
    )
    .with_target_refresh_demand(demand.clone());
    let body = serde_json::to_vec(&HttpRequestEnvelope::new(serde_json::json!({
        "contractVersion": 1,
        "term": manual_term,
    })))
    .unwrap();

    let first: serde_json::Value =
        serde_json::from_slice(&surface.pull_term(&body).expect("first pull request")).unwrap();
    assert_eq!(first["data"]["disposition"], "ENQUEUED");
    assert_eq!(
        first["data"]["targetCount"], 1,
        "ONLINE_NB is not a real target"
    );
    let second: serde_json::Value =
        serde_json::from_slice(&surface.pull_term(&body).expect("duplicate pull request")).unwrap();
    assert_eq!(second["data"]["disposition"], "ALREADY_REQUESTED");
    assert_eq!(
        demand.manual_terms_snapshot().expect("manual demand"),
        vec![manual_term]
    );

    let automatic = serde_json::to_vec(&HttpRequestEnvelope::new(serde_json::json!({
        "contractVersion": 1,
        "term": window.current_term(),
    })))
    .unwrap();
    assert_eq!(
        surface.pull_term(&automatic),
        Err(bcsp_local_runtime::LocalSurfaceFailure::unprocessable(
            bcsp_local_runtime::LocalApiErrorCode::TermOutOfRange,
        )),
        "the Local pull surface cannot be used for the automatic terms",
    );
}

#[test]
fn local_selection_retains_legacy_out_of_range_rows_but_rejects_new_ones() {
    let temp = TestDirectory::new("selection-term-window");
    let (_root, executable) = package(&temp);
    let prepared = PreparedLocalRuntime::from_executable(executable).unwrap();
    let database = prepared.operational().database();
    let old = SectionKey::try_new("92025", "NB", "12345").unwrap();
    database
        .lock()
        .unwrap()
        .personal_mut()
        .replace_selected_sections(UserStateRevision::try_from(1).unwrap(), &[old.clone()])
        .expect("seed a legacy selection outside the current window");
    let surface = PersonalSurface::new(
        database,
        PersonalStateStore::open(prepared.paths().database()).unwrap(),
        Arc::new(
            SharedWatchSocket::try_new(
                Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
                Arc::new(NoopWatchDispatchSink),
            )
            .unwrap(),
        ),
    );
    let remove = serde_json::to_vec(&HttpRequestEnvelope::new(serde_json::json!({
        "expectedUserStateRevision": 1,
        "sections": [],
    })))
    .unwrap();
    assert!(
        surface.put_selection(&remove).is_ok(),
        "legacy rows remain removable"
    );
    let add = serde_json::to_vec(&HttpRequestEnvelope::new(serde_json::json!({
        "expectedUserStateRevision": 1,
        "sections": [old],
    })))
    .unwrap();
    assert_eq!(
        surface.put_selection(&add),
        Err(bcsp_local_runtime::LocalSurfaceFailure::unprocessable(
            bcsp_local_runtime::LocalApiErrorCode::TermOutOfRange,
        )),
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
            watch_fast_lane_seconds: WatchFastLaneSeconds::try_from(60).unwrap(),
            ..LocalSettings::default()
        };
        let state_revision = database.personal().user_state_revision().unwrap();
        database
            .personal_mut()
            .compare_and_swap_settings(state_revision, SettingsRevision::ZERO, &minimum)
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
            watch_fast_lane_seconds: WatchFastLaneSeconds::try_from(60).unwrap(),
            ..LocalSettings::default()
        };
        let state_revision = database.personal().user_state_revision().unwrap();
        database
            .personal_mut()
            .compare_and_swap_settings(
                state_revision,
                SettingsRevision::try_from(1).unwrap(),
                &maximum,
            )
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
        Duration::from_secs(60)
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
fn catalog_writer_does_not_block_local_bootstrap_or_product_serving_reads() {
    let temp = TestDirectory::new("writer-serving-isolation");
    let (_root, executable) = package(&temp);
    let prepared = PreparedLocalRuntime::from_executable(executable).unwrap();
    let refresh_storage = prepared.operational().refresh_storage();
    let _refresh_guard = refresh_storage.lock().unwrap();

    let mut external_writer = Connection::open(prepared.paths().database()).unwrap();
    external_writer
        .pragma_update(None, "journal_mode", "WAL")
        .unwrap();
    let transaction = external_writer
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();
    transaction
        .execute(
            "UPDATE bcsp_operational_migrations SET name = name WHERE migration_id = 1",
            [],
        )
        .unwrap();

    let routes = prepared.route_extension();
    let local_routes = routes.clone();
    let (local_response, local_response_rx) = std::sync::mpsc::channel();
    let local_worker = std::thread::spawn(move || {
        local_response
            .send(local_routes.handle(ExtensionRequest::new(
                RequestMethod::Get,
                "/api/v1/local/bootstrap",
                None,
                Vec::new(),
            )))
            .expect("publish local bootstrap response");
    });
    let local = local_response_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("local bootstrap must not wait for the refresh writer");
    local_worker.join().expect("local bootstrap worker");
    assert_eq!(local.status(), 200);

    let body =
        serde_json::to_vec(&HttpRequestEnvelope::new(CatalogDiscoveryRequestV1::new())).unwrap();
    let discovery_routes = routes.clone();
    let (discovery_response, discovery_response_rx) = std::sync::mpsc::channel();
    let discovery_worker = std::thread::spawn(move || {
        discovery_response
            .send(discovery_routes.handle(ExtensionRequest::new(
                RequestMethod::Post,
                PRODUCT_CATALOG_DISCOVERY_PATH,
                None,
                body,
            )))
            .expect("publish Catalog discovery response");
    });
    let discovery = discovery_response_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("Catalog discovery must not wait for the refresh writer");
    discovery_worker.join().expect("Catalog discovery worker");
    assert_eq!(discovery.status(), 200);
    assert_eq!(find_sqlite_files(temp.path()).len(), 1);

    let (mutation_started, mutation_started_rx) = std::sync::mpsc::sync_channel(0);
    let (mutation_finished, mutation_finished_rx) = std::sync::mpsc::channel();
    let mutation_routes = routes.clone();
    let mutation_body = serde_json::json!({
        "protocolVersion": 1,
        "payload": {
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 0,
            "filters": filter_request("T2026F"),
        },
    })
    .to_string()
    .into_bytes();
    let personal_mutation = std::thread::spawn(move || {
        let started_at = Instant::now();
        mutation_started
            .send(())
            .expect("announce personal mutation attempt");
        let response = mutation_routes.handle(ExtensionRequest::new(
            RequestMethod::Put,
            "/api/v1/local/current-filters",
            None,
            mutation_body,
        ));
        mutation_finished
            .send((response, started_at.elapsed()))
            .expect("publish personal mutation result");
    });
    mutation_started_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("personal mutation attempt started");
    std::thread::sleep(Duration::from_millis(5_250));

    assert!(
        matches!(
            mutation_finished_rx.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ),
        "personal mutation must remain pending while the Catalog writer owns SQLite's writer slot",
    );
    let prepare_routes = routes.clone();
    let prepare_body = serde_json::json!({
        "protocolVersion": 1,
        "payload": {"expectedUserStateRevision": 1},
    })
    .to_string()
    .into_bytes();
    let (prepare_response, prepare_response_rx) = std::sync::mpsc::channel();
    let prepare_worker = std::thread::spawn(move || {
        prepare_response
            .send(prepare_routes.handle(ExtensionRequest::new(
                RequestMethod::Post,
                "/api/v1/local/user-data-reset/prepare",
                None,
                prepare_body,
            )))
            .unwrap();
    });
    std::thread::sleep(Duration::from_millis(100));
    assert!(
        matches!(
            prepare_response_rx.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ),
        "prepare must serialize behind the pending personal mutation",
    );
    for path in ["/api/v1/local/bootstrap", PRODUCT_SERVICE_STATUS_PATH] {
        let read_routes = routes.clone();
        let (read_response, read_response_rx) = std::sync::mpsc::channel();
        let read_worker = std::thread::spawn(move || {
            read_response
                .send(read_routes.handle(ExtensionRequest::new(
                    RequestMethod::Get,
                    path,
                    None,
                    Vec::new(),
                )))
                .expect("publish read response");
        });
        let read = read_response_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap_or_else(|_| panic!("{path} must not wait behind a pending personal mutation"));
        read_worker.join().expect("read worker");
        assert_eq!(read.status(), 200, "{path}");
    }

    transaction.rollback().unwrap();
    let (mutation_response, mutation_elapsed) = mutation_finished_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("personal mutation completes after the Catalog writer releases");
    assert_eq!(mutation_response.status(), 200);
    let current_filters = routes.handle(ExtensionRequest::new(
        RequestMethod::Get,
        "/api/v1/local/current-filters",
        None,
        Vec::new(),
    ));
    assert_eq!(current_filters.status(), 200);
    let current_filters: serde_json::Value =
        serde_json::from_slice(current_filters.body()).unwrap();
    assert_eq!(current_filters["data"]["revision"], 1);

    let bootstrap = routes.handle(ExtensionRequest::new(
        RequestMethod::Get,
        "/api/v1/local/bootstrap",
        None,
        Vec::new(),
    ));
    assert_eq!(bootstrap.status(), 200);
    let bootstrap: serde_json::Value = serde_json::from_slice(bootstrap.body()).unwrap();
    assert_eq!(bootstrap["data"]["state"]["currentFilters"]["revision"], 1);
    let prepared_reset = prepare_response_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("prepare completes after the earlier mutation");
    assert_eq!(prepared_reset.status(), 200);
    assert!(
        mutation_elapsed >= Duration::from_secs(5),
        "personal mutation must remain in SQLite busy-wait beyond the old five-second limit"
    );
    personal_mutation.join().expect("personal mutation thread");
    prepare_worker.join().expect("prepare worker");
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
        "/api/v1/local/current-filters",
        "/api/v1/local/saved-views",
    ] {
        let response = request(authority, &format!("GET {path}"), &origin, nonce, "");
        assert_eq!(status(&response), 200, "{path}: {response}");
    }

    let bootstrap = request(authority, "GET /api/v1/local/bootstrap", &origin, nonce, "");
    assert!(bootstrap.contains("\"activeWatchCount\":0"));
    let websocket = websocket_handshake(authority, &origin, nonce);
    assert_eq!(status(&websocket), 101, "{websocket}");
    assert!(
        websocket
            .to_ascii_lowercase()
            .contains("sec-websocket-protocol: bcsp.v1")
    );

    let settings = serde_json::json!({
        "protocolVersion": 1,
        "payload": {
            "expectedUserStateRevision": 1,
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
    let selection = r#"{"protocolVersion":1,"payload":{"expectedUserStateRevision":1,"sections":[{"term":"72026","campus":"NB","index":"12345"}]}}"#;
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
async fn loopback_server_serves_the_embedded_vite_local_application() {
    let temp = TestDirectory::new("embedded-vite-ui");
    let (_root, executable) = package(&temp);
    let running = PreparedLocalRuntime::from_executable(executable)
        .unwrap()
        .start()
        .await
        .unwrap();
    let origin = running.origin().to_owned();
    let authority = origin.strip_prefix("http://").unwrap();
    let nonce = running.nonce().as_str();

    let root = request(authority, "GET /", &origin, nonce, "");
    assert_eq!(status(&root), 200, "{root}");
    assert!(
        root.to_ascii_lowercase()
            .contains("content-type: text/html; charset=utf-8")
    );
    let root_body = body(&root);
    assert!(root_body.contains("<script type=\"module\""), "{root_body}");
    assert!(
        !root_body.to_ascii_lowercase().contains("<base "),
        "the embedded shell must remain compatible with CSP base-uri 'none'"
    );
    assert!(
        !root_body.contains("product interface will be installed in a later task")
            && !root_body.contains("Final visual UI is intentionally deferred"),
        "the embedded root must not be the retired placeholder shell"
    );

    let module_src = module_script_src(root_body);
    let module_path = module_src.to_owned();
    assert!(
        module_path.starts_with("/assets/") && module_path.ends_with(".js"),
        "unexpected Vite module path: {module_path}"
    );
    let module = request(authority, &format!("GET {module_path}"), &origin, nonce, "");
    assert_eq!(status(&module), 200, "{module_path}: {module}");
    assert!(
        module
            .to_ascii_lowercase()
            .contains("content-type: application/javascript; charset=utf-8"),
        "{module_path} must use the JavaScript MIME type"
    );
    assert!(
        !body(&module).trim().is_empty(),
        "the embedded Vite entry module must not be empty"
    );

    let section = request(
        authority,
        "GET /sections/TERM_2026_FALL/CAMPUS_A/12345",
        &origin,
        nonce,
        "",
    );
    assert_eq!(status(&section), 200, "{section}");
    assert_eq!(body(&section), root_body);

    for path in [
        "/assets/definitely-missing.js",
        "/assets/../local.html",
        "/assets/%2e%2e/local.html",
        "/assets//local.js",
        "/asset-manifest.json",
        "/capability-manifest.json",
        "/module-manifest.json",
    ] {
        let response = request(authority, &format!("GET {path}"), &origin, nonce, "");
        assert_eq!(status(&response), 404, "{path}: {response}");
    }

    running.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_application_reloads_serve_only_the_safe_spa_shell_routes() {
    let temp = TestDirectory::new("section-direct-reload");
    let (_root, executable) = package(&temp);
    let running = PreparedLocalRuntime::from_executable(executable)
        .unwrap()
        .start()
        .await
        .unwrap();
    let origin = running.origin().to_owned();
    let authority = origin.strip_prefix("http://").unwrap();
    let nonce = running.nonce().as_str();
    let root = request(authority, "GET /", &origin, nonce, "");

    for path in [
        "/history",
        "/history?focus=latest",
        "/saved-views",
        "/saved-views?focus=current",
        "/sections",
        "/sections?sort=course",
        "/settings",
        "/settings?focus=sound",
        "/watch",
        "/watch?focus=alerts",
        "/sections/2026FA/NB/12345",
        "/sections/not-semantic/still-safe/index",
    ] {
        let response = request(authority, &format!("GET {path}"), &origin, nonce, "");
        assert_eq!(status(&response), 200, "{path}: {response}");
        assert_eq!(
            body(&response),
            body(&root),
            "{path} must serve the root shell"
        );
        assert!(
            response
                .to_ascii_lowercase()
                .contains("content-type: text/html; charset=utf-8"),
            "{path} must remain an HTML shell response",
        );
    }

    for path in [
        "/history/",
        "/saved-views/",
        "/sections/",
        "/settings/",
        "/watch/",
        "/sections/2026FA/NB",
        "/sections/2026FA/NB/12345/extra",
        "/sections//NB/12345",
        "/sections/2026FA/NB/12%2f345",
        "/sections/2026FA/NB/12345%3fignored",
        "/api",
        "/api/v1/local/bootstrap/extra",
    ] {
        let response = request(authority, &format!("GET {path}"), &origin, nonce, "");
        assert_eq!(status(&response), 404, "{path}: {response}");
    }

    for path in [
        "/history",
        "/saved-views",
        "/sections",
        "/settings",
        "/watch",
    ] {
        let post = request(authority, &format!("POST {path}"), &origin, nonce, "{}");
        assert_eq!(status(&post), 404, "{path}: {post}");
    }

    running.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn local_host_composes_shared_product_routes_without_replacing_local_routes() {
    let temp = TestDirectory::new("shared-product-routes");
    let (_root, executable) = package(&temp);
    let running = PreparedLocalRuntime::from_executable(executable)
        .unwrap()
        .start()
        .await
        .unwrap();
    let origin = running.origin().to_owned();
    let authority = origin.strip_prefix("http://").unwrap();
    let nonce = running.nonce().as_str();

    let schema = request(
        authority,
        &format!("GET {PRODUCT_FILTER_SCHEMA_PATH}"),
        &origin,
        nonce,
        "",
    );
    assert_eq!(status(&schema), 200, "{schema}");
    let schema: serde_json::Value = serde_json::from_str(body(&schema)).unwrap();
    assert_eq!(schema["protocolVersion"], 1);
    assert_eq!(
        schema["data"]["fields"].as_array().unwrap().len(),
        bcsp_contracts::FILTER_FIELD_COUNT
    );

    let unauthenticated =
        request_without_session(authority, &format!("POST {PRODUCT_CATALOG_DISCOVERY_PATH}"));
    assert_eq!(status(&unauthenticated), 403, "{unauthenticated}");

    let empty_catalog = raw_api(
        authority,
        "POST",
        PRODUCT_CATALOG_DISCOVERY_PATH,
        &origin,
        nonce,
        serde_json::to_value(bcsp_contracts::CatalogDiscoveryRequestV1::new()).unwrap(),
    );
    assert_eq!(status(&empty_catalog), 200, "{empty_catalog}");
    let empty_catalog: serde_json::Value = serde_json::from_str(body(&empty_catalog)).unwrap();
    assert_eq!(
        empty_catalog["data"]["status"]["availability"],
        "UNAVAILABLE_NO_FIRST_SUCCESS"
    );

    let local = request(authority, "GET /api/v1/local/bootstrap", &origin, nonce, "");
    assert_eq!(status(&local), 200, "{local}");
    assert_eq!(find_sqlite_files(temp.path()).len(), 1);

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn saved_view_http_routes_cover_crud_dirty_state_cas_and_no_url_restore() {
    let temp = TestDirectory::new("saved-view-http");
    let (_root, executable) = package(&temp);
    let prepared = PreparedLocalRuntime::from_executable(executable).unwrap();
    seed_ready_query_scope(&prepared, &["72026", "92026"]);
    let running = prepared.start().await.unwrap();
    let origin = running.origin().to_owned();
    let authority = origin.strip_prefix("http://").unwrap();
    let nonce = running.nonce().as_str();
    let filters_a = serde_json::to_value(filter_request("72026")).unwrap();
    let filters_b = serde_json::to_value(filter_request("92026")).unwrap();

    let initial = success_payload(&request(
        authority,
        "GET /api/v1/local/current-filters",
        &origin,
        nonce,
        "",
    ));
    assert_eq!(initial["stateRevision"], 1);
    assert_eq!(initial["revision"], 0);
    assert!(initial["value"].is_null());

    let current = post_api(
        authority,
        "PUT",
        "/api/v1/local/current-filters",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 0,
            "filters": filters_a,
        }),
    );
    assert_eq!(current["revision"], 1);
    assert_eq!(current["value"]["association"]["kind"], "CUSTOM");

    let created = post_api(
        authority,
        "POST",
        "/api/v1/local/saved-views",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 1,
            "name": "Morning plan",
            "filters": filters_a,
        }),
    );
    let id = created["definition"]["id"].as_str().unwrap().to_owned();
    assert_eq!(created["definition"]["revision"], 1);
    assert_eq!(created["currentFilters"]["revision"], 2);

    let library = saved_view_library(authority, &origin, nonce);
    assert_eq!(library["views"].as_array().unwrap().len(), 1);
    assert_eq!(library["views"][0]["matchState"], "CLEAN");

    let modified = post_api(
        authority,
        "PUT",
        "/api/v1/local/current-filters",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 2,
            "filters": filters_b,
        }),
    );
    assert_eq!(modified["revision"], 3);
    assert_eq!(
        modified["value"]["association"]["viewId"], id,
        "ordinary edits preserve the Applied association",
    );
    assert_eq!(
        saved_view_library(authority, &origin, nonce)["views"][0]["matchState"],
        "MODIFIED"
    );

    let clean_again = post_api(
        authority,
        "PUT",
        "/api/v1/local/current-filters",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 3,
            "filters": filters_a,
        }),
    );
    assert_eq!(clean_again["revision"], 4);
    assert_eq!(
        saved_view_library(authority, &origin, nonce)["views"][0]["matchState"],
        "CLEAN"
    );

    let renamed = post_api(
        authority,
        "POST",
        "/api/v1/local/saved-views/rename",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 4,
            "id": id,
            "expectedViewRevision": 1,
            "name": "Renamed plan",
        }),
    );
    assert_eq!(renamed["definition"]["revision"], 2);
    assert_eq!(renamed["currentFilters"]["revision"], 5);
    assert_eq!(
        renamed["currentFilters"]["value"]["association"]["revision"],
        2
    );

    let stale = raw_api(
        authority,
        "POST",
        "/api/v1/local/saved-views/rename",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 5,
            "id": id,
            "expectedViewRevision": 1,
            "name": "Stale overwrite",
        }),
    );
    assert_eq!(status(&stale), 409);
    let stale = response_json(&stale);
    assert_eq!(stale["error"]["code"], "SAVED_VIEW_REVISION_CONFLICT");
    assert_eq!(stale["error"]["details"][0]["revision"], 2);

    let updated = post_api(
        authority,
        "POST",
        "/api/v1/local/saved-views/update",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 5,
            "id": id,
            "expectedViewRevision": 2,
            "filters": filters_b,
        }),
    );
    assert_eq!(updated["definition"]["revision"], 3);
    assert_eq!(updated["currentFilters"]["revision"], 6);

    let duplicate = post_api(
        authority,
        "POST",
        "/api/v1/local/saved-views/duplicate",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "id": id,
            "expectedViewRevision": 3,
            "name": "Copy",
        }),
    );
    let copy_id = duplicate["definition"]["id"].as_str().unwrap().to_owned();
    assert_ne!(copy_id, id);
    assert_eq!(duplicate["definition"]["revision"], 1);
    assert_eq!(duplicate["currentFilters"]["revision"], 6);

    let conflict = raw_api(
        authority,
        "POST",
        "/api/v1/local/saved-views/duplicate",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "id": id,
            "expectedViewRevision": 3,
            "name": "renamed PLAN",
        }),
    );
    assert_eq!(status(&conflict), 409);
    assert_eq!(
        response_json(&conflict)["error"]["code"],
        "SAVED_VIEW_NAME_CONFLICT"
    );

    let deleted_copy = post_api(
        authority,
        "POST",
        "/api/v1/local/saved-views/delete",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 6,
            "id": copy_id,
            "expectedViewRevision": 1,
        }),
    );
    assert_eq!(deleted_copy["currentFilters"]["revision"], 6);

    let applied = post_api(
        authority,
        "POST",
        "/api/v1/local/saved-views/apply",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 6,
            "id": id,
            "expectedViewRevision": 3,
        }),
    );
    assert_eq!(applied["revision"], 7);

    let query_shell = request(
        authority,
        "GET /?savedView=must-not-restore",
        &origin,
        nonce,
        "",
    );
    assert_eq!(status(&query_shell), 200);
    assert_eq!(
        success_payload(&request(
            authority,
            "GET /api/v1/local/current-filters",
            &origin,
            nonce,
            "",
        ))["revision"],
        7,
        "URL query state must not mutate or restore local filters",
    );
    assert_eq!(
        status(&request(
            authority,
            "GET /api/v1/local/saved-views/restore-url",
            &origin,
            nonce,
            "",
        )),
        404,
    );

    let deleted = post_api(
        authority,
        "POST",
        "/api/v1/local/saved-views/delete",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 7,
            "id": id,
            "expectedViewRevision": 3,
        }),
    );
    assert_eq!(deleted["currentFilters"]["revision"], 8);
    assert_eq!(
        deleted["currentFilters"]["value"]["association"]["kind"],
        "CUSTOM"
    );
    assert!(
        saved_view_library(authority, &origin, nonce)["views"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    running.shutdown().await.unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reset_http_routes_keep_three_scopes_distinct_and_guard_the_destructive_reset() {
    let temp = TestDirectory::new("reset-scopes");
    let (_root, executable) = package(&temp);
    let prepared = PreparedLocalRuntime::from_executable(executable).unwrap();
    seed_ready_query_scope(&prepared, &["72026"]);
    let running = prepared.start().await.unwrap();
    let origin = running.origin().to_owned();
    let authority = origin.strip_prefix("http://").unwrap();
    let nonce = running.nonce().as_str();
    let database_path = running.prepared().paths().database().to_path_buf();
    let filters = serde_json::to_value(filter_request("72026")).unwrap();

    let connection = Connection::open(&database_path).unwrap();
    connection
        .execute_batch(
            "CREATE TABLE operational_reset_probe(value TEXT NOT NULL) STRICT;
             INSERT INTO operational_reset_probe(value) VALUES ('preserve-me');",
        )
        .unwrap();
    drop(connection);

    let created = post_api(
        authority,
        "POST",
        "/api/v1/local/saved-views",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 0,
            "name": "Keep through filter reset",
            "filters": filters,
        }),
    );
    let id = created["definition"]["id"].as_str().unwrap().to_owned();
    let reset_filters = post_api(
        authority,
        "POST",
        "/api/v1/local/filters/reset",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 1,
        }),
    );
    assert_eq!(reset_filters["revision"], 2);
    assert!(reset_filters["value"].is_null());
    assert_eq!(
        saved_view_library(authority, &origin, nonce)["views"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "ordinary filter reset must preserve the Saved-view library",
    );

    let applied = post_api(
        authority,
        "POST",
        "/api/v1/local/saved-views/apply",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 2,
            "id": id,
            "expectedViewRevision": 1,
        }),
    );
    assert_eq!(applied["revision"], 3);
    let deleted_all = post_api(
        authority,
        "POST",
        "/api/v1/local/saved-views/delete-all",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 3,
        }),
    );
    assert_eq!(deleted_all["deletedViews"], 1);
    assert_eq!(deleted_all["currentFilters"]["revision"], 4);
    assert_eq!(
        deleted_all["currentFilters"]["value"]["association"]["kind"],
        "CUSTOM"
    );
    assert_eq!(
        deleted_all["currentFilters"]["value"]["content"]["filters"], filters,
        "delete-all must preserve the current canonical filters",
    );
    assert!(
        saved_view_library(authority, &origin, nonce)["views"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let recreated = post_api(
        authority,
        "POST",
        "/api/v1/local/saved-views",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 4,
            "name": "Delete only on confirmed user reset",
            "filters": filters,
        }),
    );
    assert_eq!(recreated["currentFilters"]["revision"], 5);

    let settings = serde_json::json!({
        "protocolVersion": 1,
        "payload": {
            "expectedUserStateRevision": 1,
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
    let selection = serde_json::json!({
        "expectedUserStateRevision": 1,
        "sections": [{"term": "72026", "campus": "CAMPUS_A", "index": "12345"}],
    });
    let selection = post_api(
        authority,
        "PUT",
        "/api/v1/local/selection",
        &origin,
        nonce,
        selection,
    );
    assert_eq!(selection.as_array().unwrap().len(), 1);

    let unauthenticated =
        request_without_session(authority, "POST /api/v1/local/user-data-reset/prepare");
    assert_eq!(status(&unauthenticated), 403);
    let bare_boolean = raw_api(
        authority,
        "POST",
        "/api/v1/local/user-data-reset/prepare",
        &origin,
        nonce,
        serde_json::json!({"confirmed": true}),
    );
    assert_eq!(status(&bare_boolean), 400);
    assert_eq!(
        response_json(&bare_boolean)["error"]["code"],
        "MALFORMED_REQUEST"
    );

    let prepared = post_api(
        authority,
        "POST",
        "/api/v1/local/user-data-reset/prepare",
        &origin,
        nonce,
        serde_json::json!({"expectedUserStateRevision": 1}),
    );
    let token = prepared["confirmationToken"].as_str().unwrap().to_owned();
    assert_eq!(prepared["expiresInSeconds"], 60);

    let invalid = raw_api(
        authority,
        "POST",
        "/api/v1/local/user-data-reset/confirm",
        &origin,
        nonce,
        serde_json::json!({"confirmationToken": trace(999)}),
    );
    assert_eq!(status(&invalid), 409);
    assert_eq!(
        response_json(&invalid)["error"]["code"],
        "RESET_CONFIRMATION_INVALID"
    );

    let confirmed = post_api(
        authority,
        "POST",
        "/api/v1/local/user-data-reset/confirm",
        &origin,
        nonce,
        serde_json::json!({"confirmationToken": token}),
    );
    assert_eq!(confirmed["stateRevision"], 2);
    assert_eq!(confirmed["deletedSettings"], 1);
    assert_eq!(confirmed["deletedCurrentFilters"], 1);
    assert_eq!(confirmed["deletedSavedViews"], 1);
    assert_eq!(confirmed["deletedSelectedSections"], 1);

    let reused = raw_api(
        authority,
        "POST",
        "/api/v1/local/user-data-reset/confirm",
        &origin,
        nonce,
        serde_json::json!({"confirmationToken": token}),
    );
    assert_eq!(status(&reused), 409);
    assert_eq!(
        response_json(&reused)["error"]["code"],
        "RESET_CONFIRMATION_REQUIRED"
    );

    let bootstrap = success_payload(&request(
        authority,
        "GET /api/v1/local/bootstrap",
        &origin,
        nonce,
        "",
    ));
    assert_eq!(bootstrap["state"]["stateRevision"], 2);
    assert_eq!(bootstrap["state"]["settings"]["revision"], 0);
    assert_eq!(
        bootstrap["state"]["settings"]["value"]["catalogRefreshMinutes"],
        10
    );
    assert_eq!(
        bootstrap["state"]["settings"]["value"]["openRefreshSeconds"],
        30
    );
    assert_eq!(
        bootstrap["state"]["settings"]["value"]["watchFastLaneSeconds"],
        10
    );
    assert_eq!(bootstrap["state"]["currentFilters"]["revision"], 0);
    assert!(bootstrap["state"]["currentFilters"]["value"].is_null());
    assert!(
        bootstrap["state"]["savedViews"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        bootstrap["state"]["selectedSections"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert!(
        bootstrap["state"]["episodeHistory"]["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(bootstrap["state"]["activeWatchCount"], 0);

    let stale_settings = raw_api(
        authority,
        "PUT",
        "/api/v1/local/settings",
        &origin,
        nonce,
        serde_json::json!({
            "expectedUserStateRevision": 1,
            "expectedRevision": 0,
            "value": LocalSettings::default(),
        }),
    );
    assert_eq!(status(&stale_settings), 409);
    let stale_settings = response_json(&stale_settings);
    assert_eq!(
        stale_settings["error"]["code"],
        "USER_STATE_REVISION_CONFLICT"
    );
    assert_eq!(stale_settings["error"]["details"][0]["revision"], 2);

    let connection = Connection::open(&database_path).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT value FROM operational_reset_probe", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
        "preserve-me",
    );
    assert!(
        connection
            .query_row("SELECT COUNT(*) FROM catalog_terms", [], |row| {
                row.get::<_, i64>(0)
            })
            .is_ok(),
        "operational Catalog schema must survive local user reset",
    );
    drop(connection);

    running.shutdown().await.unwrap();
}

#[test]
fn confirmed_reset_stops_active_watches_before_attempting_the_personal_transaction() {
    let temp = TestDirectory::new("reset-stop-order");
    let (_root, executable) = package(&temp);
    let prepared = PreparedLocalRuntime::from_executable(executable).unwrap();
    let database = prepared.operational().database();
    let admission = Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None));
    let watch =
        Arc::new(SharedWatchSocket::try_new(admission, Arc::new(NoopWatchDispatchSink)).unwrap());
    let connection_id = trace(100);
    let (outbound, mut responses) = mpsc::unbounded_channel();
    assert!(watch.connect(connection_id, outbound));
    let section = SectionKey::try_new("T2026F", "CAMPUS_A", "12345").unwrap();
    let items = WatchStartItemsV1::try_from(vec![WatchStartItemV1::new(
        section,
        WatchPolicyV1::default(),
    )])
    .unwrap();
    let frame = serde_json::to_string(&WsClientEnvelope::new(
        trace(101),
        WatchClientCommandV1::StartWatch { items },
    ))
    .unwrap();
    watch.receive_text(connection_id, &frame);
    assert!(responses.try_recv().is_ok());
    assert_eq!(watch.total_active_watch_count(), 1);

    let mutation_store = PersonalStateStore::open(prepared.paths().database()).unwrap();
    let surface = PersonalSurface::new(database.clone(), mutation_store, watch.clone());
    let prepare = serde_json::json!({
        "protocolVersion": 1,
        "payload": {"expectedUserStateRevision": 1},
    })
    .to_string();
    let prepared_reset = surface
        .prepare_local_user_data_reset(prepare.as_bytes())
        .unwrap();
    let prepared_reset: serde_json::Value = serde_json::from_slice(&prepared_reset).unwrap();
    let token = prepared_reset["data"]["confirmationToken"].clone();

    {
        let mut database = database.lock().unwrap();
        database
            .personal_mut()
            .clear_personal_data(UserStateRevision::try_from(1).unwrap())
            .unwrap();
    }
    assert_eq!(
        surface.prepare_local_user_data_reset(prepare.as_bytes()),
        Err(bcsp_local_runtime::LocalSurfaceFailure::revision_conflict(
            bcsp_local_runtime::LocalApiErrorCode::UserStateRevisionConflict,
            2,
        )),
        "a failed prepare must not replace the existing confirmation token",
    );
    let confirm = serde_json::json!({
        "protocolVersion": 1,
        "payload": {"confirmationToken": token},
    })
    .to_string();
    assert_eq!(
        surface.confirm_local_user_data_reset(confirm.as_bytes()),
        Err(bcsp_local_runtime::LocalSurfaceFailure::revision_conflict(
            bcsp_local_runtime::LocalApiErrorCode::UserStateRevisionConflict,
            2,
        ))
    );
    assert_eq!(
        watch.total_active_watch_count(),
        0,
        "watch cleanup must happen before even a failing reset transaction",
    );
}

#[test]
fn confirmed_reset_serializes_a_later_filter_mutation_until_reset_commits() {
    let temp = TestDirectory::new("confirm-mutation-serialization");
    let (_root, executable) = package(&temp);
    let prepared = PreparedLocalRuntime::from_executable(executable).unwrap();
    let database = prepared.operational().database();
    let admission = Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None));
    let watch =
        Arc::new(SharedWatchSocket::try_new(admission, Arc::new(NoopWatchDispatchSink)).unwrap());
    let connection_id = trace(110);
    let (outbound, mut responses) = mpsc::unbounded_channel();
    assert!(watch.connect(connection_id, outbound));
    let items = WatchStartItemsV1::try_from(vec![WatchStartItemV1::new(
        SectionKey::try_new("T2026F", "CAMPUS_A", "12345").unwrap(),
        WatchPolicyV1::default(),
    )])
    .unwrap();
    watch.receive_text(
        connection_id,
        &serde_json::to_string(&WsClientEnvelope::new(
            trace(111),
            WatchClientCommandV1::StartWatch { items },
        ))
        .unwrap(),
    );
    assert!(responses.try_recv().is_ok());
    assert_eq!(watch.total_active_watch_count(), 1);

    let mutation_store = PersonalStateStore::open(prepared.paths().database()).unwrap();
    let surface = Arc::new(PersonalSurface::new(
        database,
        mutation_store,
        watch.clone(),
    ));
    let prepare = serde_json::json!({
        "protocolVersion": 1,
        "payload": {"expectedUserStateRevision": 1},
    })
    .to_string();
    let prepared_reset = surface
        .prepare_local_user_data_reset(prepare.as_bytes())
        .unwrap();
    let prepared_reset: serde_json::Value = serde_json::from_slice(&prepared_reset).unwrap();
    let token = prepared_reset["data"]["confirmationToken"].clone();

    let mut external_writer = Connection::open(prepared.paths().database()).unwrap();
    external_writer
        .pragma_update(None, "journal_mode", "WAL")
        .unwrap();
    let transaction = external_writer
        .transaction_with_behavior(TransactionBehavior::Exclusive)
        .unwrap();
    transaction
        .execute(
            "UPDATE bcsp_operational_migrations SET name = name WHERE migration_id = 1",
            [],
        )
        .unwrap();

    let confirm_surface = surface.clone();
    let confirm_body = serde_json::json!({
        "protocolVersion": 1,
        "payload": {"confirmationToken": token},
    })
    .to_string()
    .into_bytes();
    let (confirm_result, confirm_result_rx) = std::sync::mpsc::channel();
    let confirm_worker = std::thread::spawn(move || {
        confirm_result
            .send(confirm_surface.confirm_local_user_data_reset(&confirm_body))
            .unwrap();
    });

    let deadline = Instant::now() + Duration::from_secs(2);
    while watch.total_active_watch_count() != 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        watch.total_active_watch_count(),
        0,
        "watch stop proves confirm owns the mutation lane before it waits for SQLite",
    );
    assert!(matches!(
        confirm_result_rx.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));

    let put_surface = surface.clone();
    let put_body = serde_json::json!({
        "protocolVersion": 1,
        "payload": {
            "expectedUserStateRevision": 1,
            "expectedCurrentFiltersRevision": 0,
            "filters": filter_request("T2026F"),
        },
    })
    .to_string()
    .into_bytes();
    let (put_result, put_result_rx) = std::sync::mpsc::channel();
    let put_worker = std::thread::spawn(move || {
        put_result
            .send(put_surface.put_current_filters(&put_body))
            .unwrap();
    });
    std::thread::sleep(Duration::from_millis(100));
    assert!(matches!(
        put_result_rx.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));

    transaction.rollback().unwrap();
    let confirmed = confirm_result_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("confirm completes after the external writer releases")
        .unwrap();
    let confirmed: serde_json::Value = serde_json::from_slice(&confirmed).unwrap();
    assert_eq!(confirmed["data"]["stateRevision"], 2);
    assert_eq!(
        put_result_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("later PUT runs after confirm completes"),
        Err(bcsp_local_runtime::LocalSurfaceFailure::revision_conflict(
            bcsp_local_runtime::LocalApiErrorCode::UserStateRevisionConflict,
            2,
        )),
    );
    confirm_worker.join().unwrap();
    put_worker.join().unwrap();
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

fn module_script_src(html: &str) -> &str {
    let module = html
        .find("<script type=\"module\"")
        .map(|start| &html[start..])
        .expect("Vite HTML must contain a module script");
    let src = module
        .find("src=\"")
        .map(|start| &module[start + 5..])
        .expect("Vite module script must contain src");
    let end = src.find('"').expect("Vite module src must be quoted");
    &src[..end]
}

fn raw_api(
    authority: &str,
    method: &str,
    path: &str,
    origin: &str,
    nonce: &str,
    payload: serde_json::Value,
) -> String {
    let body = serde_json::json!({
        "protocolVersion": 1,
        "payload": payload,
    })
    .to_string();
    request(authority, &format!("{method} {path}"), origin, nonce, &body)
}

fn post_api(
    authority: &str,
    method: &str,
    path: &str,
    origin: &str,
    nonce: &str,
    payload: serde_json::Value,
) -> serde_json::Value {
    let response = raw_api(authority, method, path, origin, nonce, payload);
    assert_eq!(status(&response), 200, "{path}: {response}");
    success_payload(&response)
}

fn saved_view_library(authority: &str, origin: &str, nonce: &str) -> serde_json::Value {
    success_payload(&request(
        authority,
        "GET /api/v1/local/saved-views",
        origin,
        nonce,
        "",
    ))
}

fn success_payload(response: &str) -> serde_json::Value {
    response_json(response)["data"].clone()
}

fn response_json(response: &str) -> serde_json::Value {
    serde_json::from_str(body(response)).unwrap()
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
        "GET /api/v1/watch?session={nonce} HTTP/1.1\r\nHost: {authority}\r\nOrigin: {origin}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: bcsp.v1\r\n\r\n"
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
