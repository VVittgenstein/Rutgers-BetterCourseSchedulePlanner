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
    CoordinatorStatusSink, ExtensionRequest, NoopWatchDispatchSink, OpenRuntimeSnapshot,
    PRODUCT_CATALOG_DISCOVERY_PATH, PRODUCT_FILTER_SCHEMA_PATH, PRODUCT_SERVICE_STATUS_PATH,
    RequestMethod, RouteExtension, ServiceStatusRegistry, SharedWatchSocket, TargetRefreshDemand,
    TargetWorkActivity, WebSocketExtension,
};
use bcsp_catalog::{normalize_target, to_catalog_refresh_command, to_discovery_refresh_command};
use bcsp_contracts::{
    CampusCode, CatalogDiscoveryRequestV1, FilterRequestV1, FilterValuesInputV1,
    HttpRequestEnvelope, NormalizedFilterValuesV1, SectionKey, ServiceOperationStageV2,
    ServiceRuntimeV1, ServiceWorkStateV2, TermCampusKey, TermId, TraceId, WatchClientCommandV1,
    WatchPolicyV1, WatchStartItemV1, WatchStartItemsV1, WsClientEnvelope,
};
use bcsp_domain::{RutgersTermWindow, RutgersTermWindowScope};
use bcsp_local_runtime::{
    LOCAL_DESIRED_WATCH_PATH, LOCAL_PRESENCE_SOCKET_PATH, LocalRuntimeError, LocalRuntimePaths,
    LocalSurfaceState, PersonalSurface, PreparedLocalRuntime, prepare_and_start_with,
};
use bcsp_local_user_state::{
    CatalogRefreshMinutes, LocalSettings, OpenRefreshSeconds, PersonalStateStore, SettingsRevision,
    UserStateRevision, WatchFastLaneSeconds,
};
use bcsp_open::OpenCounterAudience;
use bcsp_operational_storage::{
    BeginOpenPullAttemptCommand, DiscoveredCampus, DiscoveredTerm, DiscoveryRefreshCommand,
    DiscoverySnapshot, DiscoverySourceKind, DiscoverySourceVersion, EmptySnapshotDecision,
    FinishOpenPullSuccessCommand, OpenCacheStatus, OpenHttpAuditMetadata, OpenRequestLane,
    PublishOutcome,
};
use bcsp_rutgers_client::{
    DiscoverySnapshot as RutgersDiscoverySnapshot, DiscoverySourceInput, SourceProvenance,
    decode_catalog_payload, decode_discovery_payload,
};
use bcsp_watch::WatchStartAdmission;
use rusqlite::{Connection, TransactionBehavior};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
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

fn filter_request_with_nb(term: &str) -> FilterRequestV1 {
    let mut values = FilterValuesInputV1::for_term(TermId::try_from(term).unwrap());
    values.campuses = vec![CampusCode::try_from("NB").expect("NB Campus")];
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
        canonical_facts: serde_json::json!({
            "campusEnabled": {"state": "VALUE", "value": true},
            "targetEnabled": {"state": "VALUE", "value": true}
        }),
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
            canonical_facts: serde_json::json!({
                "published": {"state": "VALUE", "value": true}
            }),
            source_version_id: source_version_id.clone(),
        }],
        campuses: vec![campus("NB"), campus("ONLINE_NB")],
        subjects: Vec::new(),
    }
}

fn seed_ready_query_scope(prepared: &PreparedLocalRuntime, terms: &[&str]) {
    let completed = (OffsetDateTime::now_utc() - time::Duration::seconds(1))
        .format(&Rfc3339)
        .expect("fixture completion timestamp");
    let started = (OffsetDateTime::now_utc() - time::Duration::seconds(2))
        .format(&Rfc3339)
        .expect("fixture start timestamp");
    let terms = terms
        .iter()
        .map(|term| TermId::try_from(*term).unwrap())
        .collect::<Vec<_>>();
    let discovery_body = serde_json::to_vec(&serde_json::json!({
        "sourceVersion": "local-runtime-ready-fixture-v1",
        "terms": terms.iter().map(|term| {
            let raw = term.as_str();
            serde_json::json!({
                "termId": raw,
                "year": raw[1..].parse::<u16>().expect("fixture term year"),
                "termCode": &raw[..1],
                "display": raw,
                "published": true
            })
        }).collect::<Vec<_>>(),
        "campuses": [{
            "campusCode": "NB",
            "display": "New Brunswick",
            "enabled": true
        }],
        "targets": terms.iter().map(|term| serde_json::json!({
            "termId": term.as_str(),
            "campusCode": "NB",
            "enabled": true
        })).collect::<Vec<_>>(),
        "subjects": []
    }))
    .expect("synthetic discovery JSON");
    let snapshot = RutgersDiscoverySnapshot::try_from_bundle(vec![DiscoverySourceInput::selector(
        decode_discovery_payload(&discovery_body).expect("decode synthetic discovery"),
        SourceProvenance::from_body("LOCAL_RUNTIME_READY_DISCOVERY", &started, &discovery_body),
    )])
    .expect("normalize synthetic discovery");
    let database = prepared.operational().database();
    let mut database = database.lock().unwrap();
    database
        .operational_mut()
        .apply_discovery_refresh(
            to_discovery_refresh_command(&snapshot, trace(0x800), &started, &completed)
                .expect("build synthetic discovery command"),
        )
        .expect("publish ready-scope discovery fixture");
    for (position, term) in terms.iter().enumerate() {
        let target =
            TermCampusKey::new(term.clone(), CampusCode::try_from("NB").expect("NB Campus"));
        let body = serde_json::to_vec(&serde_json::json!([{
            "campusCode": "NB",
            "courseString": "01:198:111",
            "subject": "198",
            "subjectDescription": "Computer Science",
            "courseNumber": "111",
            "title": "Synthetic Course",
            "sections": [{
                "campusCode": "NB",
                "index": "10001",
                "number": "01",
                "sectionCourseType": "LECTURE",
                "openStatus": false,
                "meetingTimes": [],
                "instructors": []
            }]
        }]))
        .expect("synthetic Catalog JSON");
        let normalized = normalize_target(
            target.clone(),
            decode_catalog_payload(&body).expect("decode synthetic Catalog"),
            SourceProvenance::from_body("LOCAL_RUNTIME_READY_FIXTURE", &started, &body),
        )
        .expect("normalize synthetic Catalog");
        let observation = trace(0x820 + u64::try_from(position).unwrap());
        let outcome = database
            .operational_mut()
            .apply_catalog_refresh(
                to_catalog_refresh_command(&normalized, observation, &started, &completed)
                    .expect("build synthetic Catalog command"),
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            )
            .expect("publish synthetic Catalog");
        let content_version = match outcome {
            PublishOutcome::AppliedChanged {
                content_version, ..
            }
            | PublishOutcome::AppliedUnchanged {
                content_version, ..
            } => content_version,
            other => panic!("unexpected fixture Catalog outcome: {other:?}"),
        };
        assert_eq!(content_version, 1, "fixture Catalog content version");
        let attempt_id = trace(0x840 + u64::try_from(position).unwrap());
        let run_id = trace(0x850 + u64::try_from(position).unwrap());
        let section =
            SectionKey::try_new(term.as_str(), "NB", "10001").expect("synthetic section key");
        database
            .operational_mut()
            .begin_open_pull_attempt(&BeginOpenPullAttemptCommand {
                attempt_id,
                run_id,
                target,
                captured_catalog_content_version: content_version,
                rutgers_day: "2026-07-17".to_owned(),
                started_at: started.clone(),
                lane: OpenRequestLane::ActiveWatch,
                requested_interval_seconds: Some(30),
                effective_interval_seconds: Some(10),
                schedule_lag_ms: Some(0),
            })
            .expect("begin synthetic Open attempt");
        database
            .operational_mut()
            .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
                attempt_id,
                completed_at: completed.clone(),
                open_sections: vec![section.clone()],
                source_value_count: 1,
                watched_sections: vec![section],
                http: OpenHttpAuditMetadata {
                    http_status: Some(200),
                    cache_status: Some(OpenCacheStatus::Miss),
                    decoded_bytes: Some(2),
                    decoded_body_sha256: Some("c".repeat(64)),
                    content_type: Some("application/json".to_owned()),
                    etag: None,
                    cache_control: Some("no-store".to_owned()),
                    date: None,
                    age_seconds: None,
                    last_modified: None,
                    retry_after: None,
                    retry_after_seconds: None,
                },
            })
            .expect("publish synthetic Open set");
    }
    drop(database);
    let database = prepared.operational().database();
    let database = database.lock().unwrap();
    let discovered = database
        .operational()
        .discovered_targets()
        .expect("read fixture discovery targets");
    for term in terms {
        let target = TermCampusKey::new(term, CampusCode::try_from("NB").expect("NB Campus"));
        assert!(
            discovered.contains(&target),
            "fixture target must be discovered"
        );
        assert!(
            database
                .operational()
                .complete_target_snapshot_state(&target)
                .expect("read fixture complete target state")
                .ready,
            "fixture target must be READY"
        );
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
    let completed_at = (OffsetDateTime::now_utc() - time::Duration::seconds(1))
        .format(&Rfc3339)
        .expect("completion timestamp");
    let started_at = (OffsetDateTime::now_utc() - time::Duration::seconds(2))
        .format(&Rfc3339)
        .expect("start timestamp");
    database
        .lock()
        .unwrap()
        .operational_mut()
        .apply_discovery_refresh(DiscoveryRefreshCommand {
            observation_id: trace(0x701),
            started_at,
            completed_at,
            snapshot: manual_term_discovery(manual_term.clone()),
        })
        .expect("publish manual-term discovery fixture");
    let demand = TargetRefreshDemand::default();
    let service_status = Arc::new(ServiceStatusRegistry::new(ServiceRuntimeV1::Local));
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
    .with_target_refresh_demand(demand.clone())
    .with_service_status(service_status.clone());
    let body = serde_json::to_vec(&HttpRequestEnvelope::new(serde_json::json!({
        "contractVersion": 1,
        "term": manual_term,
    })))
    .unwrap();

    let first: serde_json::Value =
        serde_json::from_slice(&surface.pull_term(&body).expect("first pull request")).unwrap();
    assert_eq!(first["data"]["disposition"], "ENQUEUED");
    assert_eq!(first["data"]["targetCount"], 3);
    let second: serde_json::Value =
        serde_json::from_slice(&surface.pull_term(&body).expect("duplicate pull request")).unwrap();
    assert_eq!(second["data"]["disposition"], "ALREADY_REQUESTED");
    assert_eq!(
        demand.manual_terms_snapshot().expect("manual demand"),
        vec![manual_term.clone()]
    );

    for retry in demand
        .pending_manual_target_retries()
        .expect("first target retry generation")
    {
        assert!(
            demand
                .acknowledge_manual_target_retry(&retry)
                .expect("supervisor acknowledgement")
        );
    }
    service_status.publish_target_activity(TargetWorkActivity {
        target: TermCampusKey::try_new(manual_term.as_str(), "NK").expect("active NK target"),
        work_state: ServiceWorkStateV2::Running,
        stage: Some(ServiceOperationStageV2::CatalogFetch),
        started_at: Some(OffsetDateTime::now_utc()),
        next_retry_at: None,
        error: None,
    });
    let partial: serde_json::Value = serde_json::from_slice(
        &surface
            .pull_term(&body)
            .expect("partial target retry request"),
    )
    .unwrap();
    assert_eq!(partial["data"]["disposition"], "ENQUEUED");
    assert_eq!(partial["data"]["targetCount"], 2);
    assert_eq!(
        demand
            .pending_manual_target_retries()
            .expect("partial retry generations")
            .iter()
            .map(|retry| retry.target().campus().as_str())
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from(["NB", "CM"]),
        "the active NK workflow is deduplicated while missing NB and CM retry independently",
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
        .replace_selected_sections(
            UserStateRevision::try_from(1).unwrap(),
            std::slice::from_ref(&old),
        )
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

#[test]
fn local_selection_retains_legacy_unsupported_campus_rows_but_rejects_new_ones() {
    let temp = TestDirectory::new("selection-legacy-campus");
    let (_root, executable) = package(&temp);
    let prepared = PreparedLocalRuntime::from_executable(executable).unwrap();
    let database = prepared.operational().database();
    let legacy = SectionKey::try_new("92026", "NWK", "12345").expect("legacy section");
    database
        .lock()
        .unwrap()
        .personal_mut()
        .replace_selected_sections(
            UserStateRevision::try_from(1).unwrap(),
            std::slice::from_ref(&legacy),
        )
        .expect("seed a legacy selection with an unsupported Campus");
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

    let retain = serde_json::to_vec(&HttpRequestEnvelope::new(serde_json::json!({
        "expectedUserStateRevision": 1,
        "sections": [legacy.clone()],
    })))
    .unwrap();
    assert!(
        surface.put_selection(&retain).is_ok(),
        "legacy unsupported Campus rows remain visible and retained"
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
        "sections": [legacy],
    })))
    .unwrap();
    assert_eq!(
        surface.put_selection(&add),
        Err(bcsp_local_runtime::LocalSurfaceFailure::unprocessable(
            bcsp_local_runtime::LocalApiErrorCode::InvalidLocalState,
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
    let websocket = websocket_handshake(authority, "/api/v1/watch", &origin, nonce);
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
    let window = RutgersTermWindow::at(OffsetDateTime::now_utc(), RutgersTermWindowScope::Local)
        .expect("test execution date is covered by the bundled calendar");
    let in_window_term = window.current_term().as_str();
    let selection = format!(
        r#"{{"protocolVersion":1,"payload":{{"expectedUserStateRevision":1,"sections":[{{"term":"{in_window_term}","campus":"NB","index":"12345"}}]}}}}"#
    );
    assert_eq!(
        status(&request(
            authority,
            "PUT /api/v1/local/selection",
            &origin,
            nonce,
            &selection,
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

/// Every desired-watch answer, and the status it earns.
///
/// The statuses are the contract, not an error mapping. `409` for the four
/// terminal refusals, because all four mean the same thing to a page: what
/// you read is not what is there, re-read before asking again. `503` for a
/// full authority, because that one is not terminal -- nothing was written
/// and the same id may be presented again. `400` for a body the route cannot
/// parse, which is a protocol fault rather than an answer.
///
/// The load-bearing assertion is the replay: a refusal replayed from the
/// receipt ledger comes back with the status the ORIGINAL answer earned. The
/// tempting alternative -- 200 because the envelope says it was replayed --
/// would tell a page that lost its response that a refused command had
/// succeeded, which is precisely the failure the ledger exists to prevent.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn every_desired_watch_outcome_is_reported_with_the_status_it_earned() {
    let temp = TestDirectory::new("desired-watch-statuses");
    let (_root, executable) = package(&temp);
    let running = PreparedLocalRuntime::from_executable(executable)
        .unwrap()
        .start()
        .await
        .unwrap();
    let origin = running.origin().to_owned();
    let authority = origin.strip_prefix("http://").unwrap();
    let nonce = running.nonce().as_str().to_owned();
    let nonce = nonce.as_str();
    let section = SectionKey::try_new("T2026F", "CAMPUS_A", "00001").unwrap();
    let other = SectionKey::try_new("T2026F", "CAMPUS_A", "00002").unwrap();

    // A commit.
    let (status_code, committed) = put_desired_watch(
        authority, &origin, nonce, &section, Some(watch_policy()), 0, 1, trace(1),
    );
    assert_eq!(status_code, 200, "{committed}");
    assert_eq!(committed["outcome"], "COMMITTED");
    assert_eq!(committed["replayed"], false);
    assert_eq!(committed["committed"]["epochChanged"], true);
    assert_eq!(
        committed["state"]["entries"].as_array().unwrap().len(),
        1,
        "a committing page is handed the authority its own write produced",
    );
    let revision = committed["committed"]["revision"].as_u64().unwrap();

    // The same id again: the ledger answers, and the answer is the same 200.
    let (status_code, replayed) = put_desired_watch(
        authority, &origin, nonce, &section, Some(watch_policy()), 0, 1, trace(1),
    );
    assert_eq!(status_code, 200, "{replayed}");
    assert_eq!(replayed["outcome"], "COMMITTED");
    assert_eq!(replayed["replayed"], true);
    assert_eq!(replayed["committed"]["revision"], revision);

    // A stale revision, and then the SAME id replayed. Both 409.
    let (status_code, stale) = put_desired_watch(
        authority, &origin, nonce, &section, Some(watch_policy()), 999, 1, trace(2),
    );
    assert_eq!(status_code, 409, "{stale}");
    assert_eq!(stale["outcome"], "STALE_REVISION");
    assert_eq!(stale["currentRevision"], revision);
    assert!(stale["state"].is_null(), "a refused page must re-read");
    let (status_code, stale_replay) = put_desired_watch(
        authority, &origin, nonce, &section, Some(watch_policy()), 999, 1, trace(2),
    );
    assert_eq!(
        status_code, 409,
        "a replayed refusal is still a refusal: {stale_replay}",
    );
    assert_eq!(stale_replay["outcome"], "STALE_REVISION");
    assert_eq!(stale_replay["replayed"], true);

    // A generation that no longer exists.
    let (status_code, stale_generation) = put_desired_watch(
        authority, &origin, nonce, &other, Some(watch_policy()), 0, 99, trace(3),
    );
    assert_eq!(status_code, 409, "{stale_generation}");
    assert_eq!(stale_generation["outcome"], "STALE_GENERATION");
    assert_eq!(
        stale_generation["authorityGeneration"], 1,
        "and it carries the generation the page has to re-read with",
    );

    // The same id carrying a different command.
    let (status_code, conflict) = put_desired_watch(
        authority, &origin, nonce, &other, Some(watch_policy()), 0, 1, trace(1),
    );
    assert_eq!(status_code, 409, "{conflict}");
    assert_eq!(conflict["outcome"], "MUTATION_ID_CONFLICT");

    // A body the route cannot parse is a protocol fault, and answers in the
    // shared typed error shape rather than as a desired-watch outcome.
    let malformed = request(
        authority,
        &format!("PUT {LOCAL_DESIRED_WATCH_PATH}"),
        &origin,
        nonce,
        r#"{"protocolVersion":1,"payload":{"contractVersion":1}}"#,
    );
    assert_eq!(status(&malformed), 400, "{malformed}");
    assert_eq!(response_json(&malformed)["error"]["code"], "MALFORMED_REQUEST");

    // A write still needs the Origin and the session nonce the rest of the
    // local surface needs.
    let unauthenticated =
        request_without_session(authority, &format!("PUT {LOCAL_DESIRED_WATCH_PATH}"));
    assert_eq!(status(&unauthenticated), 403, "{unauthenticated}");

    running.shutdown().await.unwrap();
}

/// The nine-section product cap, over HTTP, including the shape of the
/// refusal a page has to render.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_desired_watch_cap_refuses_a_tenth_section_with_the_maximum() {
    let temp = TestDirectory::new("desired-watch-cap");
    let (_root, executable) = package(&temp);
    let running = PreparedLocalRuntime::from_executable(executable)
        .unwrap()
        .start()
        .await
        .unwrap();
    let origin = running.origin().to_owned();
    let authority = origin.strip_prefix("http://").unwrap();
    let nonce = running.nonce().as_str().to_owned();
    let nonce = nonce.as_str();

    let mut revisions = Vec::new();
    for index in 1..=9_u16 {
        let section = SectionKey::try_new("T2026F", "CAMPUS_A", &format!("{index:05}")).unwrap();
        let (status_code, committed) = put_desired_watch(
            authority,
            &origin,
            nonce,
            &section,
            Some(watch_policy()),
            0,
            1,
            trace(u64::from(index)),
        );
        assert_eq!(status_code, 200, "{committed}");
        revisions.push(committed["committed"]["revision"].as_u64().unwrap());
    }

    let tenth = SectionKey::try_new("T2026F", "CAMPUS_A", "00010").unwrap();
    let (status_code, capped) = put_desired_watch(
        authority, &origin, nonce, &tenth, Some(watch_policy()), 0, 1, trace(10),
    );
    assert_eq!(status_code, 409, "{capped}");
    assert_eq!(capped["outcome"], "LIMIT_EXCEEDED");
    assert_eq!(capped["maximum"], 9);

    // A policy edit on one of the nine still commits: the cap is measured
    // against the state a mutation LEAVES, and this one leaves nine.
    let first = SectionKey::try_new("T2026F", "CAMPUS_A", "00001").unwrap();
    let (status_code, edited) = put_desired_watch(
        authority,
        &origin,
        nonce,
        &first,
        Some(watch_policy()),
        revisions[0],
        1,
        trace(20),
    );
    assert_eq!(status_code, 200, "a policy-only edit at the cap: {edited}");
    assert_eq!(edited["committed"]["epochChanged"], false);

    running.shutdown().await.unwrap();
}

/// The whole local vertical, over HTTP, across three process lifetimes:
/// write intent, restart, find it restored and materializing, Full Reset,
/// restart again and find it gone.
///
/// This is the shape the Windows packaging gate runs against a real
/// candidate. Running it here as well pins the behaviour in the ordinary
/// test suite rather than only in a release rehearsal, and it is what makes
/// migration 10004 safe to ship: the reset counters it introduced are
/// exercised against a NON-EMPTY table, which is the only case where they
/// can be wrong.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn desired_intent_survives_a_restart_and_a_full_reset_clears_it() {
    let temp = TestDirectory::new("desired-watch-restart");
    let (_root, executable) = package(&temp);
    let section = SectionKey::try_new("T2026F", "CAMPUS_A", "00001").unwrap();

    // First lifetime: commit intent.
    let running = PreparedLocalRuntime::from_executable(executable.clone())
        .unwrap()
        .start()
        .await
        .unwrap();
    let origin = running.origin().to_owned();
    let authority = origin.strip_prefix("http://").unwrap();
    let first_nonce = running.nonce().as_str().to_owned();
    let (status_code, committed) = put_desired_watch(
        authority,
        &origin,
        &first_nonce,
        &section,
        Some(watch_policy()),
        0,
        1,
        trace(1),
    );
    assert_eq!(status_code, 200, "{committed}");
    let bootstrap = response_json(&request(
        authority,
        "GET /api/v1/local/bootstrap",
        &origin,
        &first_nonce,
        "",
    ));
    assert_eq!(
        bootstrap["data"]["state"]["desiredWatches"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "the bootstrap wire keeps its protocol v1 shape and shows the intent",
    );
    running.shutdown().await.unwrap();

    // Second lifetime: the intent is still there, under the same generation,
    // and nothing is armed until a page attaches.
    let running = PreparedLocalRuntime::from_executable(executable.clone())
        .unwrap()
        .start()
        .await
        .unwrap();
    let origin = running.origin().to_owned();
    let authority = origin.strip_prefix("http://").unwrap();
    let second_nonce = running.nonce().as_str().to_owned();
    assert_ne!(second_nonce, first_nonce, "a restart is a new session");
    let state = read_desired_watch(authority, &origin, &second_nonce);
    assert_eq!(state["authorityGeneration"], 1);
    let entry = desired_entry(&state, &section).expect("the intent survived the restart");
    assert!(
        !entry["policy"].is_null(),
        "and it is still a watch, not a tombstone",
    );
    assert!(
        !is_armed(&state, entry),
        "nothing is armed before a page attaches, and the read says so plainly",
    );
    assert!(entry["materialized"].is_null());

    // A page attaches and stays. This fixture's section is on a campus the
    // product does not target, so it can never be armed -- and the honest
    // report is the intent plus the reason, never a green light, and never a
    // row the process quietly deleted on the user's behalf.
    let socket = open_watch_socket(authority, &origin, &second_nonce);
    let attached = desired_watch_until(authority, &origin, &second_nonce, |state| {
        desired_entry(state, &section)
            .is_some_and(|entry| !entry["failure"].is_null())
    })
    .await;
    let entry = desired_entry(&attached, &section).unwrap();
    assert!(!entry["policy"].is_null(), "the row is never withdrawn");
    assert!(!is_armed(&attached, entry));
    assert_eq!(
        entry["failure"]["classification"], "PERMANENT",
        "this fixture's campus is not a product target: {entry}",
    );
    assert_eq!(entry["failure"]["reason"], "UNSUPPORTED_TARGET");
    assert_eq!(
        entry["failure"]["retryScheduled"], false,
        "nothing this process does changes that answer, so it stops asking",
    );
    assert!(
        !entry["policy"].is_null(),
        "and it STILL does not withdraw the row: a section the runtime has          proven it cannot arm is the user's to remove, not the process's",
    );
    drop(socket);

    // Full Reset reports the deletion, which is the count migration 10004
    // added and which an empty-table rehearsal cannot exercise.
    let state_revision = response_json(&request(
        authority,
        "GET /api/v1/local/bootstrap",
        &origin,
        &second_nonce,
        "",
    ))["data"]["state"]["stateRevision"]
        .clone();
    let prepared = post_api(
        authority,
        "POST",
        "/api/v1/local/user-data-reset/prepare",
        &origin,
        &second_nonce,
        serde_json::json!({"expectedUserStateRevision": state_revision}),
    );
    let confirmed = post_api(
        authority,
        "POST",
        "/api/v1/local/user-data-reset/confirm",
        &origin,
        &second_nonce,
        serde_json::json!({"confirmationToken": prepared["confirmationToken"]}),
    );
    assert_eq!(
        confirmed["deletedDesiredWatches"], 1,
        "the non-empty path is the one that can be wrong",
    );
    assert_eq!(confirmed["deletedDesiredWatchReceipts"], 1);
    let after_reset = read_desired_watch(authority, &origin, &second_nonce);
    assert_eq!(after_reset["entries"].as_array().unwrap().len(), 0);
    assert_eq!(
        after_reset["authorityGeneration"], 2,
        "a reset raises the generation, so nothing from before it can commit",
    );
    running.shutdown().await.unwrap();

    // Third lifetime: still empty, and still at the raised generation.
    let running = PreparedLocalRuntime::from_executable(executable)
        .unwrap()
        .start()
        .await
        .unwrap();
    let origin = running.origin().to_owned();
    let authority = origin.strip_prefix("http://").unwrap();
    let third_nonce = running.nonce().as_str().to_owned();
    let state = read_desired_watch(authority, &origin, &third_nonce);
    assert_eq!(state["entries"].as_array().unwrap().len(), 0);
    assert_eq!(state["authorityGeneration"], 2);
    assert_eq!(
        response_json(&request(
            authority,
            "GET /api/v1/local/bootstrap",
            &origin,
            &third_nonce,
            "",
        ))["data"]["state"]["desiredWatches"]
            .as_array()
            .unwrap()
            .len(),
        0,
    );
    running.shutdown().await.unwrap();
}

/// S2b. The shared host promises nothing about a path no target injected --
/// it simply is not in the route table, and what a client sees there is
/// whatever the local extension fallback does with it. Presence is still
/// un-injected (it belongs to the page-lifecycle milestone), so pin it in
/// both shapes a page can reach it with; a future injection then has to be a
/// visible behaviour change rather than a silent one.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_un_injected_presence_path_is_404_not_a_websocket_route() {
    let temp = TestDirectory::new("no-secondary-socket");
    let (_root, executable) = package(&temp);
    let running = PreparedLocalRuntime::from_executable(executable)
        .unwrap()
        .start()
        .await
        .unwrap();
    let origin = running.origin().to_owned();
    let authority = origin.strip_prefix("http://").unwrap();
    let nonce = running.nonce().as_str();

    let upgrade = websocket_handshake(authority, LOCAL_PRESENCE_SOCKET_PATH, &origin, nonce);
    assert_eq!(status(&upgrade), 404, "presence upgrade: {upgrade}");
    assert_eq!(body(&upgrade), "not found");

    let read = request(
        authority,
        &format!("GET {LOCAL_PRESENCE_SOCKET_PATH}"),
        &origin,
        nonce,
        "",
    );
    assert_eq!(status(&read), 404, "presence read: {read}");
    assert_eq!(body(&read), "not found");

    // The built-in route is unaffected by any of this.
    let watch = websocket_handshake(authority, "/api/v1/watch", &origin, nonce);
    assert_eq!(status(&watch), 101, "{watch}");

    running.shutdown().await.unwrap();
}

/// The desired-watch path is an ordinary local HTTP resource, and a plain
/// GET on it succeeds. The earlier design put a second WebSocket route here
/// and an older test pinned the path at 404 for everything; both were
/// withdrawn when the co-editing model was cut back to "pages read on load
/// and after their own writes".
///
/// What still has to hold is that it is not SECRETLY a socket. A page that
/// sends an upgrade request here must not get one: it reaches the HTTP
/// handler, is answered like the GET it syntactically is, and no
/// `Sec-WebSocket-Accept` comes back. A route that quietly accepted an
/// upgrade would be a second, unversioned way to reach the authority.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn the_desired_watch_path_reads_over_http_and_never_upgrades() {
    let temp = TestDirectory::new("desired-watch-http");
    let (_root, executable) = package(&temp);
    let running = PreparedLocalRuntime::from_executable(executable)
        .unwrap()
        .start()
        .await
        .unwrap();
    let origin = running.origin().to_owned();
    let authority = origin.strip_prefix("http://").unwrap();
    let nonce = running.nonce().as_str();

    let read = request(
        authority,
        &format!("GET {LOCAL_DESIRED_WATCH_PATH}"),
        &origin,
        nonce,
        "",
    );
    assert_eq!(status(&read), 200, "desired-watch read: {read}");
    let state = desired_watch_data(&read);
    assert_eq!(state["contractVersion"], 1);
    assert_eq!(state["authorityGeneration"], 1);
    assert_eq!(state["entries"].as_array().unwrap().len(), 0);

    let upgrade = websocket_handshake(authority, LOCAL_DESIRED_WATCH_PATH, &origin, nonce);
    assert_ne!(status(&upgrade), 101, "desired-watch upgrade: {upgrade}");
    assert!(
        !upgrade.to_ascii_lowercase().contains("sec-websocket-accept"),
        "an upgrade must not be completed here: {upgrade}",
    );

    // Methods the resource does not define are refused by the same route,
    // rather than falling through to the product routes and 404ing.
    let posted = request(
        authority,
        &format!("POST {LOCAL_DESIRED_WATCH_PATH}"),
        &origin,
        nonce,
        "{}",
    );
    assert_eq!(status(&posted), 405, "{posted}");

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
    let prepared = PreparedLocalRuntime::from_executable(&executable).unwrap();
    seed_ready_query_scope(&prepared, &["72026", "92026"]);
    drop(prepared);
    let prepared = PreparedLocalRuntime::from_executable(&executable).unwrap();
    let running = prepared.start().await.unwrap();
    let origin = running.origin().to_owned();
    let authority = origin.strip_prefix("http://").unwrap();
    let nonce = running.nonce().as_str();
    let filters_a = serde_json::to_value(filter_request_with_nb("72026")).unwrap();
    let filters_b = serde_json::to_value(filter_request_with_nb("92026")).unwrap();

    let initial_response = request(
        authority,
        "GET /api/v1/local/current-filters",
        &origin,
        nonce,
        "",
    );
    assert_eq!(status(&initial_response), 200, "{initial_response}");
    let initial = success_payload(&initial_response);
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
    assert_eq!(
        library["views"][0]["matchState"], "CLEAN",
        "saved-view library: {library:#}"
    );

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
    let prepared = PreparedLocalRuntime::from_executable(&executable).unwrap();
    seed_ready_query_scope(&prepared, &["72026"]);
    drop(prepared);
    let prepared = PreparedLocalRuntime::from_executable(&executable).unwrap();
    let running = prepared.start().await.unwrap();
    let origin = running.origin().to_owned();
    let authority = origin.strip_prefix("http://").unwrap();
    let nonce = running.nonce().as_str();
    let database_path = running.prepared().paths().database().to_path_buf();
    let filters = serde_json::to_value(filter_request_with_nb("72026")).unwrap();

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
    let window = RutgersTermWindow::at(OffsetDateTime::now_utc(), RutgersTermWindowScope::Local)
        .expect("test execution date is covered by the bundled calendar");
    let selection = serde_json::json!({
        "expectedUserStateRevision": 1,
        "sections": [{"term": window.current_term().as_str(), "campus": "NB", "index": "12345"}],
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
    assert_eq!(confirmed["deletedDesiredWatches"], 0);
    assert_eq!(confirmed["deletedDesiredWatchReceipts"], 0);

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
        bootstrap["state"]["desiredWatches"]
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

/// Opens a watch socket and HOLDS it, so the caller is a real audience.
///
/// `websocket_handshake` drops its stream, which is right for a test that
/// only wants the status line -- and wrong for anything that then observes
/// what the process does while a page is attached, because the disconnect
/// callback fires immediately.
fn open_watch_socket(authority: &str, origin: &str, nonce: &str) -> TcpStream {
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
    let response = read_http_response(&mut stream);
    assert_eq!(status(&response), 101, "{response}");
    stream
}

/// Waits for the desired-watch read to satisfy `settled`.
///
/// The attach-time reconcile runs on the server's own task after the 101 is
/// written, so a read taken immediately afterwards can legitimately race it.
/// Polling a CONDITION rather than sleeping a fixed interval keeps the test
/// deterministic in what it asserts while tolerating the scheduling.
async fn desired_watch_until(
    authority: &str,
    origin: &str,
    nonce: &str,
    settled: impl Fn(&serde_json::Value) -> bool,
) -> serde_json::Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let state = read_desired_watch(authority, origin, nonce);
        if settled(&state) {
            return state;
        }
        assert!(
            Instant::now() < deadline,
            "the desired-watch read never settled: {state}",
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// The `data` object of a local desired-watch response.
fn desired_watch_data(response: &str) -> serde_json::Value {
    let envelope: serde_json::Value =
        serde_json::from_str(body(response)).unwrap_or_else(|error| {
            panic!("desired-watch response must be JSON ({error}): {response}")
        });
    assert_eq!(envelope["protocolVersion"], 1, "{response}");
    envelope["data"].clone()
}

/// Reads the desired-watch authority over HTTP.
fn read_desired_watch(authority: &str, origin: &str, nonce: &str) -> serde_json::Value {
    let response = request(
        authority,
        &format!("GET {LOCAL_DESIRED_WATCH_PATH}"),
        origin,
        nonce,
        "",
    );
    assert_eq!(status(&response), 200, "{response}");
    desired_watch_data(&response)
}

/// Submits one desired-watch compare-and-swap, returning the status and the
/// decoded `data` object so a test can assert on both.
fn put_desired_watch(
    authority: &str,
    origin: &str,
    nonce: &str,
    section: &SectionKey,
    policy: Option<WatchPolicyV1>,
    based_on_revision: u64,
    authority_generation: u64,
    mutation_id: TraceId,
) -> (u16, serde_json::Value) {
    let payload = serde_json::json!({
        "protocolVersion": 1,
        "payload": {
            "contractVersion": 1,
            "section": {
                "term": section.term().as_str(),
                "campus": section.campus().as_str(),
                "index": section.index().as_str(),
            },
            "policy": policy,
            "basedOnRevision": based_on_revision,
            "authorityGeneration": authority_generation,
            "mutationId": mutation_id.to_string(),
        },
    })
    .to_string();
    let response = request(
        authority,
        &format!("PUT {LOCAL_DESIRED_WATCH_PATH}"),
        origin,
        nonce,
        &payload,
    );
    (status(&response), desired_watch_data(&response))
}

/// The authority entry for one section, if the read has one.
fn desired_entry<'a>(
    state: &'a serde_json::Value,
    section: &SectionKey,
) -> Option<&'a serde_json::Value> {
    state["entries"].as_array().unwrap().iter().find(|entry| {
        entry["section"]["term"] == section.term().as_str()
            && entry["section"]["campus"] == section.campus().as_str()
            && entry["section"]["index"] == section.index().as_str()
    })
}

/// The frontend's rule, in Rust: a section is really being watched only when
/// a materialization record exists AND its whole stamp equals the
/// authority's. Anything less is "preparing", never green.
fn is_armed(state: &serde_json::Value, entry: &serde_json::Value) -> bool {
    let materialized = &entry["materialized"];
    !materialized.is_null()
        && materialized["authorityGeneration"] == state["authorityGeneration"]
        && materialized["revision"] == entry["revision"]
        && materialized["materializationEpoch"] == entry["materializationEpoch"]
        && materialized["policy"] == entry["policy"]
}

fn watch_policy() -> WatchPolicyV1 {
    WatchPolicyV1::default()
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

fn websocket_handshake(authority: &str, path: &str, origin: &str, nonce: &str) -> String {
    let mut stream = TcpStream::connect(authority).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    write!(
        stream,
        "GET {path}?session={nonce} HTTP/1.1\r\nHost: {authority}\r\nOrigin: {origin}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Protocol: bcsp.v1\r\n\r\n"
    )
    .unwrap();
    stream.flush().unwrap();
    read_http_response(&mut stream)
}

/// Reads exactly one HTTP response: the headers, then exactly the body its
/// `content-length` promises, and nothing past that. One `read` is not
/// enough -- TCP may deliver the body in a later segment than the headers,
/// and it may equally deliver the next thing on the wire in the same one.
/// A 101 carries no body, so it stops at the header terminator; the
/// connection stays open, which rules out simply reading to EOF.
fn read_http_response(stream: &mut TcpStream) -> String {
    let mut response = Vec::new();
    let mut buffer = [0_u8; 2_048];
    loop {
        if let Some(header_end) = response
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .map(|at| at + 4)
        {
            let head = String::from_utf8_lossy(&response[..header_end]).to_ascii_lowercase();
            let length = head
                .lines()
                .find_map(|line| line.strip_prefix("content-length:"))
                .map_or(0, |value| value.trim().parse::<usize>().unwrap());
            if response.len() >= header_end + length {
                // One read can carry more than this response: on a 101 the
                // host's heartbeat Ping (0x89 0x00) may share the segment
                // with the headers, and those bytes are not UTF-8. Cut at
                // the boundary; the caller drops the socket next.
                response.truncate(header_end + length);
                break;
            }
        }
        let read = stream.read(&mut buffer).unwrap();
        assert!(read > 0, "the response ended before it was complete");
        response.extend_from_slice(&buffer[..read]);
    }
    String::from_utf8(response).unwrap()
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
