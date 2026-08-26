use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use bcsp_application::{
    ApplicationClock, CoordinatorClock, CoordinatorDispatchOutcome, CoordinatorStatusSink,
    CoordinatorStatusSnapshot, ExtensionRequest, FixedRefreshPolicyProvider, OpenDispatchTerminal,
    OpenRuntimeSnapshotRegistry, OutboundSender, PRODUCT_CATALOG_DISCOVERY_PATH,
    PRODUCT_COURSE_DETAIL_PATH, PRODUCT_COURSE_SEARCH_PATH, PRODUCT_FILTER_SCHEMA_PATH,
    PRODUCT_OPEN_SECTION_STATUS_PATH, PRODUCT_OPEN_STATUS_PATH, PRODUCT_SECTION_DETAIL_PATH,
    PRODUCT_SECTION_SEARCH_PATH, ProductStorageAccess, ProductStorageLockError, RefreshFuture,
    RefreshPolicy, RefreshUpstream, RequestMethod, RouteExtension, ScheduledRefreshTarget,
    SharedProductRoutes, SharedRefreshCoordinator, SharedRuntimeContext, SharedWatchSocket,
    TargetWorkActivity, WebSocketExtension, publish_discovery_for_refresh,
};
use bcsp_contracts::{
    CatalogDiscoveryRequestV1, CatalogDiscoveryResponseV1, CourseDetailRequestV1,
    CourseDetailResponseV1, CourseQueryRequestV1, CourseQueryResponseV1, CourseSortV1,
    FilterRequestV1, FilterSchemaV1, FilterSearchTextV1, FilterValuesInputV1, HttpRequestEnvelope,
    HttpSuccessEnvelope, LiveOpenStateV1, NormalizedFilterValuesV1, OpenBatchKey, OpenFailureClass,
    OpenFreshnessState, OpenRefreshClassification, OpenRefreshStatusV1, OpenSchedulerLane,
    OpenSectionStatusRequestV1, OpenSectionStatusV1, OpenState, OpenStatusRequestV1,
    OpenUncertaintyReason, PageRequestV1, SectionDetailRequestV1, SectionDetailResponseV1,
    SectionIndex, SectionKey, SectionQueryRequestV1, SectionQueryResponseV1, SectionSortV1,
    ServiceOperationPhaseV1, ServiceOperationStageV2, ServiceOperationV1, ServiceWorkStateV2,
    TermCampusKey, TermId, TraceId, TraceIdSource, WatchClientCommandV1, WatchPolicyV1,
    WatchServerEventV1, WatchStartItemResultV1, WatchStartItemV1, WatchStartItemsV1,
    WsClientEnvelope, WsServerEnvelope,
};
use bcsp_open::{GeneralOpenInterval, MonotonicTime, OpenCounterAudience};
use bcsp_operational_storage::{OperationalStorage, PublishOutcome};
use bcsp_rutgers_client::{
    DiscoverySnapshot, DiscoverySourceInput, OpenPayloadClassification, OpenResponseMetadata,
    OpenSectionsError, OpenSectionsFailure, OpenSectionsRequest, OpenSectionsResponse,
    SourceProvenance, canonical_open_set_sha256, decode_catalog_payload, decode_discovery_payload,
    sha256_hex,
};
use bcsp_watch::WatchStartAdmission;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tempfile::TempDir;
use time::OffsetDateTime;
use tokio::sync::mpsc;

const BASE_UNIX_SECONDS: i64 = 1_784_246_400;
const DISCOVERY_BODY: &[u8] = br#"{
  "sourceVersion":"synthetic-v1",
  "terms":[{
    "termId":"92026",
    "year":2026,
    "termCode":"9",
    "display":"Fall 2026",
    "published":true
  }],
  "campuses":[{"campusCode":"NB","display":"New Brunswick","enabled":true}],
  "targets":[{"termId":"92026","campusCode":"NB","enabled":true}],
  "subjects":[{
    "termId":"92026",
    "campusCode":"NB",
    "subjectCode":"198",
    "display":"Computer Science",
    "enabled":true
  }]
}"#;
const CATALOG_BODY: &[u8] = br#"[{
  "campusCode":"NB",
  "courseString":"01:198:111",
  "subject":"198",
  "subjectDescription":"Computer Science",
  "courseNumber":"111",
  "title":"Introduction to Computer Science",
  "sections":[{
    "campusCode":"NB",
    "index":"10001",
    "number":"01",
    "sectionCourseType":"LECTURE",
    "openStatus":false,
    "meetingTimes":[]
  }]
}]"#;

fn trace(suffix: u64) -> TraceId {
    format!("00000000-0000-4000-8000-{suffix:012x}")
        .parse()
        .expect("synthetic trace ID")
}

#[derive(Clone, Default)]
struct FakeClock {
    milliseconds: Arc<AtomicU64>,
}

impl FakeClock {
    fn advance(&self, duration: Duration) {
        self.milliseconds.fetch_add(
            u64::try_from(duration.as_millis()).unwrap_or(u64::MAX),
            Ordering::SeqCst,
        );
    }

    fn expected_wall(&self, seconds: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(BASE_UNIX_SECONDS + seconds)
            .expect("synthetic timestamp")
    }
}

impl CoordinatorClock for FakeClock {
    fn monotonic_now(&self) -> MonotonicTime {
        MonotonicTime::from_millis(self.milliseconds.load(Ordering::SeqCst))
    }

    fn wall_now(&self) -> OffsetDateTime {
        let milliseconds =
            i64::try_from(self.milliseconds.load(Ordering::SeqCst)).unwrap_or(i64::MAX);
        OffsetDateTime::from_unix_timestamp(BASE_UNIX_SECONDS).expect("synthetic epoch")
            + time::Duration::milliseconds(milliseconds)
    }
}

impl ApplicationClock for FakeClock {
    fn now(&self) -> OffsetDateTime {
        self.wall_now()
    }
}

struct FakeIds(u64);

impl TraceIdSource for FakeIds {
    fn next_trace_id(&mut self) -> TraceId {
        self.0 = self.0.saturating_add(1);
        trace(self.0)
    }
}

#[derive(Default)]
struct RecordingStatus {
    activities: Mutex<Vec<ServiceOperationV1>>,
    target_activities: Mutex<Vec<TargetWorkActivity>>,
    snapshots: Mutex<Vec<CoordinatorStatusSnapshot>>,
    stopped: AtomicBool,
}

impl CoordinatorStatusSink for RecordingStatus {
    fn publish(&self, snapshot: CoordinatorStatusSnapshot) {
        self.snapshots.lock().expect("status lock").push(snapshot);
    }

    fn mark_stopped(&self) {
        self.stopped.store(true, Ordering::SeqCst);
    }

    fn publish_activity(&self, operation: ServiceOperationV1) {
        self.activities
            .lock()
            .expect("activity lock")
            .push(operation);
    }

    fn publish_target_activity(&self, activity: TargetWorkActivity) {
        self.target_activities
            .lock()
            .expect("target activity lock")
            .push(activity);
    }
}

enum FakeOpenResult {
    Success(Vec<SectionIndex>),
    Failure(OpenSectionsError),
}

struct FakeUpstreamState {
    catalog: VecDeque<
        Result<bcsp_application::CatalogPullResponse, bcsp_application::CatalogPullFailure>,
    >,
    open: VecDeque<FakeOpenResult>,
}

struct FakeUpstream {
    storage: Arc<Mutex<OperationalStorage>>,
    state: Mutex<FakeUpstreamState>,
    catalog_calls: AtomicUsize,
    open_calls: AtomicUsize,
    active: AtomicUsize,
    maximum_active: AtomicUsize,
    all_fetches_lock_free: AtomicBool,
    open_clock_advance: Option<(FakeClock, Duration)>,
}

#[derive(Clone)]
struct FakeUpstreamHandle(Arc<FakeUpstream>);

impl FakeUpstream {
    fn new(
        storage: Arc<Mutex<OperationalStorage>>,
        catalog: impl IntoIterator<
            Item = Result<
                bcsp_application::CatalogPullResponse,
                bcsp_application::CatalogPullFailure,
            >,
        >,
        open: impl IntoIterator<Item = FakeOpenResult>,
    ) -> Self {
        Self {
            storage,
            state: Mutex::new(FakeUpstreamState {
                catalog: catalog.into_iter().collect(),
                open: open.into_iter().collect(),
            }),
            catalog_calls: AtomicUsize::new(0),
            open_calls: AtomicUsize::new(0),
            active: AtomicUsize::new(0),
            maximum_active: AtomicUsize::new(0),
            all_fetches_lock_free: AtomicBool::new(true),
            open_clock_advance: None,
        }
    }

    fn with_open_clock_advance(mut self, clock: FakeClock, duration: Duration) -> Self {
        self.open_clock_advance = Some((clock, duration));
        self
    }

    async fn enter_fetch(&self) {
        let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum_active.fetch_max(active, Ordering::SeqCst);
        if self.storage.try_lock().is_err() {
            self.all_fetches_lock_free.store(false, Ordering::SeqCst);
        }
        tokio::task::yield_now().await;
    }

    fn leave_fetch(&self) {
        self.active.fetch_sub(1, Ordering::SeqCst);
    }
}

impl RefreshUpstream for FakeUpstreamHandle {
    fn fetch_catalog<'a>(
        &'a self,
        _target: &'a TermCampusKey,
    ) -> RefreshFuture<
        'a,
        Result<bcsp_application::CatalogPullResponse, bcsp_application::CatalogPullFailure>,
    > {
        Box::pin(async move {
            self.0.catalog_calls.fetch_add(1, Ordering::SeqCst);
            self.0.enter_fetch().await;
            let result = self
                .0
                .state
                .lock()
                .expect("fake upstream lock")
                .catalog
                .pop_front()
                .unwrap_or(Err(bcsp_application::CatalogPullFailure::Transport));
            self.0.leave_fetch();
            result
        })
    }

    fn fetch_open<'a>(
        &'a self,
        request: OpenSectionsRequest,
    ) -> RefreshFuture<'a, Result<OpenSectionsResponse, OpenSectionsFailure>> {
        Box::pin(async move {
            self.0.open_calls.fetch_add(1, Ordering::SeqCst);
            self.0.enter_fetch().await;
            if let Some((clock, duration)) = &self.0.open_clock_advance {
                clock.advance(*duration);
            }
            let selected = self
                .0
                .state
                .lock()
                .expect("fake upstream lock")
                .open
                .pop_front()
                .unwrap_or(FakeOpenResult::Failure(OpenSectionsError::Network));
            let result = match selected {
                FakeOpenResult::Success(indexes) => {
                    let body = serde_json::to_vec(
                        &indexes.iter().map(SectionIndex::as_str).collect::<Vec<_>>(),
                    )
                    .expect("synthetic Open body");
                    Ok(OpenSectionsResponse {
                        target: request.target().clone(),
                        classification: if indexes.is_empty() {
                            OpenPayloadClassification::Empty
                        } else {
                            OpenPayloadClassification::Nonempty
                        },
                        metadata: OpenResponseMetadata {
                            http_status: 200,
                            decoded_bytes: body.len(),
                            decoded_body_sha256: sha256_hex(&body),
                            canonical_set_sha256: canonical_open_set_sha256(&indexes),
                            raw_value_count: indexes.len(),
                            unique_value_count: indexes.len(),
                            duplicate_value_count: 0,
                            content_type: "application/json".to_owned(),
                            etag: None,
                            cache_control: Some("no-store".to_owned()),
                            date: None,
                            age_seconds: None,
                            last_modified: None,
                        },
                        open_indexes: indexes,
                    })
                }
                FakeOpenResult::Failure(error) => Err(error.into()),
            };
            self.0.leave_fetch();
            result
        })
    }
}

struct Fixture {
    _directory: TempDir,
    storage: Arc<Mutex<OperationalStorage>>,
    target: TermCampusKey,
    section: SectionKey,
    open_request: OpenSectionsRequest,
    catalog_response: bcsp_application::CatalogPullResponse,
}

#[derive(Clone)]
struct DelayedCatalogPersistence {
    storage: Arc<Mutex<OperationalStorage>>,
    lock_calls: Arc<AtomicUsize>,
    delay: Duration,
}

impl ProductStorageAccess for DelayedCatalogPersistence {
    type Guard<'a> = MutexGuard<'a, OperationalStorage>;

    fn lock_operational(&self) -> Result<Self::Guard<'_>, ProductStorageLockError> {
        if self.lock_calls.fetch_add(1, Ordering::SeqCst) == 1 {
            std::thread::sleep(self.delay);
        }
        self.storage.lock().map_err(|_| ProductStorageLockError)
    }
}

fn fixture() -> Fixture {
    let directory = TempDir::new().expect("temporary directory");
    let storage = Arc::new(Mutex::new(
        OperationalStorage::open(directory.path().join("coordinator.sqlite"))
            .expect("operational SQLite"),
    ));
    let provenance = SourceProvenance::from_body(
        "SYNTHETIC_DISCOVERY",
        "2030-01-01T00:00:00Z",
        DISCOVERY_BODY,
    );
    let discovery = DiscoverySnapshot::try_from_bundle(vec![DiscoverySourceInput::selector(
        decode_discovery_payload(DISCOVERY_BODY).expect("synthetic discovery"),
        provenance,
    )])
    .expect("discovery snapshot");
    let published = publish_discovery_for_refresh(
        &storage,
        &discovery,
        trace(1),
        "2030-01-01T00:00:00Z",
        "2030-01-01T00:00:00Z",
    )
    .expect("publish discovery and derive refresh targets");
    let registration = published.targets.first().expect("refresh target");
    let target = registration.target().clone();
    let open_request = registration.open_request().clone();
    let section = SectionKey::try_new("92026", "NB", "10001").expect("section");
    let catalog_response = bcsp_application::CatalogPullResponse {
        target: target.clone(),
        courses: decode_catalog_payload(CATALOG_BODY).expect("synthetic Catalog"),
        provenance: SourceProvenance::from_body(
            "SYNTHETIC_CATALOG",
            "2030-01-01T00:00:00Z",
            CATALOG_BODY,
        ),
        selector_confirms_target: true,
    };
    Fixture {
        _directory: directory,
        storage,
        target,
        section,
        open_request,
        catalog_response,
    }
}

fn policy(open: GeneralOpenInterval) -> RefreshPolicy {
    RefreshPolicy::try_new(Duration::from_secs(600), open).expect("refresh policy")
}

fn create_socket() -> Arc<SharedWatchSocket> {
    Arc::new(
        SharedWatchSocket::try_new(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            Arc::new(bcsp_application::NoopWatchDispatchSink),
        )
        .expect("watch socket"),
    )
}

fn start_watch(socket: &SharedWatchSocket, section: SectionKey) -> mpsc::UnboundedReceiver<String> {
    let (outbound, mut receiver) = OutboundSender::unbounded_pair();
    let connection_id = trace(80);
    assert!(socket.connect(connection_id, outbound));
    let items = WatchStartItemsV1::try_from(vec![WatchStartItemV1::new(
        section,
        WatchPolicyV1::default(),
    )])
    .expect("watch items");
    let frame = serde_json::to_string(&WsClientEnvelope::new(
        trace(81),
        WatchClientCommandV1::StartWatch { items },
    ))
    .expect("watch command");
    socket.receive_text(connection_id, &frame);
    let start = serde_json::from_str::<WsServerEnvelope<WatchServerEventV1>>(
        &receiver.try_recv().expect("START_RESULT"),
    )
    .expect("typed START_RESULT")
    .into_payload();
    let WatchServerEventV1::StartResult { result } = start else {
        panic!("START must produce START_RESULT");
    };
    assert_eq!(result.active_watch_count(), 1);
    assert!(matches!(
        result.items(),
        [WatchStartItemResultV1::Active { .. }]
    ));
    receiver
}

type ProductRoutes = SharedProductRoutes<FakeClock, FixedRefreshPolicyProvider>;

fn post<Request, Response>(routes: &ProductRoutes, path: &'static str, request: Request) -> Response
where
    Request: Serialize,
    Response: DeserializeOwned,
{
    let body = serde_json::to_vec(&HttpRequestEnvelope::new(request)).expect("request envelope");
    let response = routes.handle(ExtensionRequest::new(RequestMethod::Post, path, None, body));
    assert_eq!(
        response.status(),
        200,
        "{path}: {}",
        String::from_utf8_lossy(response.body())
    );
    serde_json::from_slice::<HttpSuccessEnvelope<Response>>(response.body())
        .expect("success envelope")
        .into_data()
}

fn product_search_filters() -> FilterRequestV1 {
    let mut input =
        FilterValuesInputV1::for_term(TermId::try_from("92026").expect("synthetic term"));
    input.text =
        Some(FilterSearchTextV1::try_from("Computer Science").expect("synthetic search text"));
    FilterRequestV1::new(
        NormalizedFilterValuesV1::try_new(input).expect("normalized synthetic filters"),
    )
}

fn watch_events(receiver: &mut mpsc::UnboundedReceiver<String>) -> Vec<WatchServerEventV1> {
    std::iter::from_fn(|| receiver.try_recv().ok())
        .map(|frame| {
            serde_json::from_str::<WsServerEnvelope<WatchServerEventV1>>(&frame)
                .expect("typed watch event")
                .into_payload()
        })
        .collect()
}

#[tokio::test(flavor = "current_thread")]
async fn catalog_persistence_does_not_block_the_async_runtime_thread() {
    let fixture = fixture();
    let clock = FakeClock::default();
    let upstream_state = Arc::new(FakeUpstream::new(
        Arc::clone(&fixture.storage),
        [Ok(fixture.catalog_response.clone())],
        [FakeOpenResult::Success(vec![
            fixture.section.index().clone(),
        ])],
    ));
    let delayed_storage = DelayedCatalogPersistence {
        storage: Arc::clone(&fixture.storage),
        lock_calls: Arc::new(AtomicUsize::new(0)),
        delay: Duration::from_millis(250),
    };
    let status = Arc::new(RecordingStatus::default());
    let mut coordinator = SharedRefreshCoordinator::with_parts(
        delayed_storage,
        FakeUpstreamHandle(upstream_state),
        FixedRefreshPolicyProvider::new(policy(GeneralOpenInterval::public())),
        clock,
        FakeIds(300),
        trace(90),
        OpenCounterAudience::Public,
        create_socket(),
        Arc::new(OpenRuntimeSnapshotRegistry::default()),
        status.clone(),
    );
    coordinator
        .register_target(
            ScheduledRefreshTarget::try_new(fixture.target, fixture.open_request)
                .expect("registration"),
        )
        .expect("register target");

    let started = Instant::now();
    let catalog = tokio::spawn(async move { coordinator.run_next().await });
    tokio::task::yield_now().await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(
        started.elapsed() < Duration::from_millis(125),
        "Catalog persistence blocked the single Tokio worker"
    );
    assert!(matches!(
        catalog
            .await
            .expect("Catalog task")
            .expect("Catalog dispatch"),
        Some(CoordinatorDispatchOutcome::CatalogPublished { .. })
    ));
}

#[tokio::test]
async fn initial_open_is_one_shot_until_product_demand_activates_recurring() {
    let fixture = fixture();
    let clock = FakeClock::default();
    let upstream_state = Arc::new(FakeUpstream::new(
        Arc::clone(&fixture.storage),
        [Ok(fixture.catalog_response.clone())],
        [
            FakeOpenResult::Success(vec![fixture.section.index().clone()]),
            FakeOpenResult::Success(vec![fixture.section.index().clone()]),
        ],
    ));
    let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
    let mut coordinator = SharedRefreshCoordinator::with_parts(
        Arc::clone(&fixture.storage),
        FakeUpstreamHandle(Arc::clone(&upstream_state)),
        FixedRefreshPolicyProvider::new(policy(GeneralOpenInterval::public())),
        clock.clone(),
        FakeIds(350),
        trace(94),
        OpenCounterAudience::Public,
        create_socket(),
        Arc::clone(&open_runtime),
        Arc::new(RecordingStatus::default()),
    );
    coordinator
        .register_target(
            ScheduledRefreshTarget::try_new(fixture.target.clone(), fixture.open_request)
                .expect("registration"),
        )
        .expect("register target");

    coordinator
        .run_next()
        .await
        .expect("initial complete snapshot");
    assert_eq!(upstream_state.open_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        open_runtime
            .snapshot(&fixture.target)
            .expect("parked runtime")
            .next_due_at,
        None
    );

    clock.advance(Duration::from_secs(95));
    assert!(
        coordinator
            .run_next()
            .await
            .expect("parked target")
            .is_none()
    );
    assert_eq!(upstream_state.open_calls.load(Ordering::SeqCst), 1);

    coordinator
        .activate_open_target(&fixture.target)
        .expect("product demand activation");
    assert!(matches!(
        coordinator.run_next().await.expect("demanded Open"),
        Some(CoordinatorDispatchOutcome::OpenCompleted {
            terminal: OpenDispatchTerminal::Valid,
            ..
        })
    ));
    assert_eq!(upstream_state.open_calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        open_runtime
            .snapshot(&fixture.target)
            .expect("recurring runtime")
            .next_due_at,
        Some(clock.expected_wall(125))
    );
}

#[tokio::test]
async fn typed_empty_open_for_nonempty_catalog_is_current_and_counted() {
    let fixture = fixture();
    let clock = FakeClock::default();
    let upstream_state = Arc::new(FakeUpstream::new(
        Arc::clone(&fixture.storage),
        [Ok(fixture.catalog_response.clone())],
        [FakeOpenResult::Success(Vec::new())],
    ));
    let fixed_policy = FixedRefreshPolicyProvider::new(policy(GeneralOpenInterval::public()));
    let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
    let mut coordinator = SharedRefreshCoordinator::with_parts(
        Arc::clone(&fixture.storage),
        FakeUpstreamHandle(upstream_state),
        fixed_policy,
        clock.clone(),
        FakeIds(375),
        trace(95),
        OpenCounterAudience::Public,
        create_socket(),
        Arc::clone(&open_runtime),
        Arc::new(RecordingStatus::default()),
    );
    coordinator
        .register_target(
            ScheduledRefreshTarget::try_new(fixture.target.clone(), fixture.open_request.clone())
                .expect("registration"),
        )
        .expect("register target");

    assert!(matches!(
        coordinator
            .run_next()
            .await
            .expect("complete snapshot dispatch"),
        Some(CoordinatorDispatchOutcome::CatalogPublished { .. })
    ));

    let routes = SharedProductRoutes::new(
        Arc::clone(&fixture.storage),
        SharedRuntimeContext::new(OpenCounterAudience::Public, clock, fixed_policy),
        open_runtime,
    );
    assert!(
        fixture
            .storage
            .lock()
            .expect("storage")
            .complete_target_snapshot_state(&fixture.target)
            .expect("complete state")
            .ready
    );

    let open: OpenRefreshStatusV1 = post(
        &routes,
        PRODUCT_OPEN_STATUS_PATH,
        OpenStatusRequestV1::new(OpenBatchKey::from(fixture.target.clone())),
    );
    assert_eq!(open.freshness.state, OpenFreshnessState::Fresh);
    assert_eq!(open.counter_snapshot.today_counts.attempted, 1);
    assert_eq!(open.counter_snapshot.today_counts.succeeded, 1);
    assert_eq!(open.counter_snapshot.today_counts.failed, 0);
    assert_eq!(open.counter_snapshot.today_counts.empty, 1);

    let section: OpenSectionStatusV1 = post(
        &routes,
        PRODUCT_OPEN_SECTION_STATUS_PATH,
        OpenSectionStatusRequestV1::new(fixture.section),
    );
    assert_eq!(section.state, OpenState::Closed);
    assert_eq!(section.freshness.state, OpenFreshnessState::Fresh);
}

#[tokio::test]
async fn long_full_first_load_sweep_reaches_all_current_without_recurring_every_target() {
    const CAMPUSES: [&str; 3] = ["NB", "NK", "CM"];
    const TARGET_COUNT: usize = CAMPUSES.len();
    const OPEN_ELAPSED: Duration = Duration::from_secs(30);
    assert!(
        OPEN_ELAPSED * u32::try_from(TARGET_COUNT).expect("bounded target count")
            > Duration::from_secs(75),
        "the fixture must exceed the legacy 2*30s+15s freshness window"
    );

    let directory = TempDir::new().expect("temporary directory");
    let storage = Arc::new(Mutex::new(
        OperationalStorage::open(directory.path().join("long-first-load.sqlite"))
            .expect("operational SQLite"),
    ));
    let campuses = CAMPUSES
        .iter()
        .map(|campus| {
            serde_json::json!({
                "campusCode": campus,
                "display": format!("Campus {campus}"),
                "enabled": true
            })
        })
        .collect::<Vec<_>>();
    let targets = CAMPUSES
        .iter()
        .map(|campus| {
            serde_json::json!({
                "termId": "92026",
                "campusCode": campus,
                "enabled": true
            })
        })
        .collect::<Vec<_>>();
    let discovery_body = serde_json::to_vec(&serde_json::json!({
        "sourceVersion": "long-first-load-v1",
        "terms": [{
            "termId": "92026",
            "year": 2026,
            "termCode": "9",
            "display": "Fall 2026",
            "published": true
        }],
        "campuses": campuses,
        "targets": targets,
        "subjects": []
    }))
    .expect("discovery body");
    let discovery = DiscoverySnapshot::try_from_bundle(vec![DiscoverySourceInput::selector(
        decode_discovery_payload(&discovery_body).expect("synthetic discovery"),
        SourceProvenance::from_body(
            "LONG_FIRST_LOAD_DISCOVERY",
            "2030-01-01T00:00:00Z",
            &discovery_body,
        ),
    )])
    .expect("discovery snapshot");
    let published = publish_discovery_for_refresh(
        &storage,
        &discovery,
        trace(5_000),
        "2030-01-01T00:00:00Z",
        "2030-01-01T00:00:00Z",
    )
    .expect("publish discovery");
    assert_eq!(published.targets.len(), TARGET_COUNT);

    let catalog = published.targets.iter().map(|registration| {
        Ok(bcsp_application::CatalogPullResponse {
            target: registration.target().clone(),
            courses: Vec::new(),
            provenance: SourceProvenance::from_body(
                "LONG_FIRST_LOAD_CATALOG",
                "2030-01-01T00:00:00Z",
                b"[]",
            ),
            selector_confirms_target: true,
        })
    });
    let open = std::iter::repeat_with(|| FakeOpenResult::Success(Vec::new())).take(TARGET_COUNT);
    let clock = FakeClock::default();
    let upstream_state = Arc::new(
        FakeUpstream::new(Arc::clone(&storage), catalog, open)
            .with_open_clock_advance(clock.clone(), OPEN_ELAPSED),
    );
    let fixed_policy = FixedRefreshPolicyProvider::new(policy(GeneralOpenInterval::public()));
    let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
    let mut coordinator = SharedRefreshCoordinator::with_parts(
        Arc::clone(&storage),
        FakeUpstreamHandle(Arc::clone(&upstream_state)),
        fixed_policy,
        clock.clone(),
        FakeIds(5_100),
        trace(5_100),
        OpenCounterAudience::Public,
        create_socket(),
        Arc::clone(&open_runtime),
        Arc::new(RecordingStatus::default()),
    );
    for registration in &published.targets {
        coordinator
            .register_target(registration.clone())
            .expect("register discovered target");
    }
    let routes = SharedProductRoutes::new(
        Arc::clone(&storage),
        SharedRuntimeContext::new(OpenCounterAudience::Public, clock.clone(), fixed_policy),
        Arc::clone(&open_runtime),
    );
    assert!(published.targets.iter().all(|registration| {
        !storage
            .lock()
            .expect("storage")
            .complete_target_snapshot_state(registration.target())
            .expect("complete state")
            .ready
    }));

    let mut catalog_valid_empty = 0;
    while catalog_valid_empty < TARGET_COUNT {
        match coordinator
            .run_next()
            .await
            .expect("serialized refresh")
            .expect("a first-load job remains")
        {
            CoordinatorDispatchOutcome::CatalogPublished {
                outcome: PublishOutcome::InitialValidEmpty { .. },
                ..
            } => catalog_valid_empty += 1,
            unexpected => panic!("unexpected first-load outcome: {unexpected:?}"),
        }
    }
    assert_eq!(
        clock.monotonic_now(),
        MonotonicTime::from_millis(
            u64::try_from((OPEN_ELAPSED * u32::try_from(TARGET_COUNT).unwrap()).as_millis())
                .expect("bounded first-load duration"),
        )
    );

    assert!(published.targets.iter().all(|registration| {
        storage
            .lock()
            .expect("storage")
            .complete_target_snapshot_state(registration.target())
            .expect("complete state")
            .ready
    }));

    let first_target = published.targets[0].target().clone();
    let first_open: OpenRefreshStatusV1 = post(
        &routes,
        PRODUCT_OPEN_STATUS_PATH,
        OpenStatusRequestV1::new(OpenBatchKey::from(first_target.clone())),
    );
    assert_eq!(
        first_open.freshness.observed_at,
        Some(
            clock.expected_wall(
                i64::try_from(OPEN_ELAPSED.as_secs()).expect("bounded Open duration"),
            ),
        ),
        "the lease extends freshness without rewriting the real observation time"
    );
    assert_eq!(first_open.scheduler.requested_general_interval_seconds, 30);
    assert_eq!(
        first_open.scheduler.requested_effective_interval_seconds,
        30
    );
    assert_eq!(first_open.scheduler.next_due_at, None);
    assert_eq!(
        first_open.counter_snapshot.today_counts.empty,
        TARGET_COUNT as u64
    );
    assert_eq!(upstream_state.maximum_active.load(Ordering::SeqCst), 1);
    assert!(published.targets.iter().all(|registration| {
        open_runtime
            .snapshot(registration.target())
            .expect("runtime snapshot")
            .next_due_at
            .is_none()
    }));
}

#[tokio::test]
async fn fake_upstream_drives_all_product_routes_and_watch_without_client_amplification() {
    let fixture = fixture();
    let clock = FakeClock::default();
    let socket = create_socket();
    let upstream_state = Arc::new(FakeUpstream::new(
        Arc::clone(&fixture.storage),
        [Ok(fixture.catalog_response.clone())],
        [FakeOpenResult::Success(vec![
            fixture.section.index().clone(),
        ])],
    ));
    let upstream = FakeUpstreamHandle(Arc::clone(&upstream_state));
    let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
    let fixed_policy = FixedRefreshPolicyProvider::new(policy(GeneralOpenInterval::public()));
    let status = Arc::new(RecordingStatus::default());
    let mut coordinator = SharedRefreshCoordinator::with_parts(
        Arc::clone(&fixture.storage),
        upstream,
        fixed_policy,
        clock.clone(),
        FakeIds(400),
        trace(93),
        OpenCounterAudience::Public,
        Arc::clone(&socket),
        Arc::clone(&open_runtime),
        status.clone(),
    );
    coordinator
        .register_target(
            ScheduledRefreshTarget::try_new(fixture.target.clone(), fixture.open_request.clone())
                .expect("registration"),
        )
        .expect("register target");

    let mut receiver = start_watch(&socket, fixture.section.clone());
    assert!(matches!(
        coordinator.run_next().await.expect("Catalog dispatch"),
        Some(CoordinatorDispatchOutcome::CatalogPublished { .. })
    ));
    assert_eq!(upstream_state.catalog_calls.load(Ordering::SeqCst), 1);
    assert_eq!(upstream_state.open_calls.load(Ordering::SeqCst), 1);
    let phases = status
        .activities
        .lock()
        .expect("activity lock")
        .iter()
        .map(|operation| operation.phase)
        .collect::<Vec<_>>();
    assert_eq!(
        phases,
        vec![
            ServiceOperationPhaseV1::CatalogFetch,
            ServiceOperationPhaseV1::CatalogProcess,
            ServiceOperationPhaseV1::CatalogPublish,
            ServiceOperationPhaseV1::OpenFetch,
            ServiceOperationPhaseV1::Idle,
        ]
    );

    let events = watch_events(&mut receiver);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, WatchServerEventV1::OpenObservation { .. }))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, WatchServerEventV1::EpisodeUpdated { .. }))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, WatchServerEventV1::AlertUpdated { .. }))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, WatchServerEventV1::AudioDisposition { .. }))
    );

    let runtime =
        SharedRuntimeContext::new(OpenCounterAudience::Public, clock.clone(), fixed_policy);
    let routes = SharedProductRoutes::new(
        Arc::clone(&fixture.storage),
        runtime,
        Arc::clone(&open_runtime),
    );
    let second_client_routes = SharedProductRoutes::new(
        Arc::clone(&fixture.storage),
        SharedRuntimeContext::new(OpenCounterAudience::Public, clock, fixed_policy),
        Arc::clone(&open_runtime),
    );

    let schema_response = routes.handle(ExtensionRequest::new(
        RequestMethod::Get,
        PRODUCT_FILTER_SCHEMA_PATH,
        None,
        Vec::new(),
    ));
    assert_eq!(schema_response.status(), 200);
    let schema =
        serde_json::from_slice::<HttpSuccessEnvelope<FilterSchemaV1>>(schema_response.body())
            .expect("filter schema envelope")
            .into_data();
    assert_eq!(schema.fields.len(), bcsp_contracts::FILTER_FIELD_COUNT);

    let discovery: CatalogDiscoveryResponseV1 = post(
        &routes,
        PRODUCT_CATALOG_DISCOVERY_PATH,
        CatalogDiscoveryRequestV1::new(),
    );
    assert_eq!(discovery.targets.len(), 1);
    assert_eq!(discovery.targets[0].key, fixture.target);
    assert_eq!(discovery.subjects.len(), 1);

    let filters = product_search_filters();
    let courses: CourseQueryResponseV1 = post(
        &routes,
        PRODUCT_COURSE_SEARCH_PATH,
        CourseQueryRequestV1 {
            filters: filters.clone(),
            page: PageRequestV1::default(),
            sort: CourseSortV1::default(),
        },
    );
    assert_eq!(courses.page.total, 1);
    assert_eq!(
        courses.items[0].variants[0].sections[0].open.state,
        LiveOpenStateV1::Open
    );
    let course_key = courses.items[0].group.key.clone();

    let sections: SectionQueryResponseV1 = post(
        &routes,
        PRODUCT_SECTION_SEARCH_PATH,
        SectionQueryRequestV1 {
            filters: filters.clone(),
            page: PageRequestV1::default(),
            sort: SectionSortV1::default(),
        },
    );
    assert_eq!(sections.page.total, 1);
    assert_eq!(sections.items[0].section.section.key, fixture.section);
    assert_eq!(sections.items[0].section.open.state, LiveOpenStateV1::Open);

    let course_detail: CourseDetailResponseV1 = post(
        &routes,
        PRODUCT_COURSE_DETAIL_PATH,
        CourseDetailRequestV1::new(course_key.clone()),
    );
    assert_eq!(course_detail.course.group.key, course_key);
    assert_eq!(
        course_detail.course.variants[0].sections[0].open.state,
        LiveOpenStateV1::Open
    );
    let section_detail: SectionDetailResponseV1 = post(
        &routes,
        PRODUCT_SECTION_DETAIL_PATH,
        SectionDetailRequestV1::new(fixture.section.clone()),
    );
    assert_eq!(section_detail.section.section.key, fixture.section);
    assert_eq!(section_detail.section.open.state, LiveOpenStateV1::Open);

    let open: OpenRefreshStatusV1 = post(
        &routes,
        PRODUCT_OPEN_STATUS_PATH,
        OpenStatusRequestV1::new(OpenBatchKey::from(fixture.target.clone())),
    );
    assert_eq!(open.freshness.state, OpenFreshnessState::Fresh);
    assert!(open.freshness.observed_at.is_some());
    assert!(open.freshness.fresh_until.is_some());
    assert_eq!(open.scheduler.lane, OpenSchedulerLane::ActiveWatch);
    assert_eq!(open.scheduler.active_watch_count, 1);
    assert_eq!(open.scheduler.scheduler_lag_milliseconds, 0);
    assert_eq!(open.counter_snapshot.today_counts.attempted, 1);
    assert_eq!(open.counter_snapshot.today_counts.succeeded, 1);
    assert_eq!(open.counter_snapshot.today_counts.failed, 0);

    let open_section: OpenSectionStatusV1 = post(
        &routes,
        PRODUCT_OPEN_SECTION_STATUS_PATH,
        OpenSectionStatusRequestV1::new(fixture.section.clone()),
    );
    assert_eq!(open_section.section_key, fixture.section);
    assert_eq!(open_section.state, OpenState::Open);
    assert_eq!(open_section.freshness.state, OpenFreshnessState::Fresh);
    assert_eq!(open_section.scheduler_lag_milliseconds, 0);
    assert_eq!(open_section.counter_snapshot.today_counts.attempted, 1);
    assert_eq!(open_section.counter_snapshot.today_counts.succeeded, 1);

    let second_discovery: CatalogDiscoveryResponseV1 = post(
        &second_client_routes,
        PRODUCT_CATALOG_DISCOVERY_PATH,
        CatalogDiscoveryRequestV1::new(),
    );
    assert_eq!(second_discovery.targets, discovery.targets);
    let second_courses: CourseQueryResponseV1 = post(
        &second_client_routes,
        PRODUCT_COURSE_SEARCH_PATH,
        CourseQueryRequestV1 {
            filters,
            page: PageRequestV1::default(),
            sort: CourseSortV1::default(),
        },
    );
    assert_eq!(second_courses.page.total, 1);
    assert_eq!(upstream_state.catalog_calls.load(Ordering::SeqCst), 1);
    assert_eq!(upstream_state.open_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn shared_coordinator_fans_out_committed_open_and_skips_catch_up_bursts() {
    let fixture = fixture();
    let clock = FakeClock::default();
    let socket = create_socket();
    let mut receiver = start_watch(&socket, fixture.section.clone());
    let open_index = fixture.section.index().clone();
    let upstream_state = Arc::new(FakeUpstream::new(
        Arc::clone(&fixture.storage),
        [Ok(fixture.catalog_response.clone())],
        [
            FakeOpenResult::Success(vec![open_index.clone()]),
            FakeOpenResult::Success(vec![open_index]),
        ],
    ));
    let upstream = FakeUpstreamHandle(Arc::clone(&upstream_state));
    let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
    let readiness = Arc::new(RecordingStatus::default());
    let mut coordinator = SharedRefreshCoordinator::with_parts(
        Arc::clone(&fixture.storage),
        upstream,
        FixedRefreshPolicyProvider::new(policy(GeneralOpenInterval::public())),
        clock.clone(),
        FakeIds(100),
        trace(90),
        OpenCounterAudience::Public,
        Arc::clone(&socket),
        Arc::clone(&open_runtime),
        readiness.clone(),
    );
    coordinator
        .register_target(
            ScheduledRefreshTarget::try_new(fixture.target.clone(), fixture.open_request)
                .expect("registration"),
        )
        .expect("register target");

    assert!(matches!(
        coordinator.run_next().await.expect("Catalog dispatch"),
        Some(CoordinatorDispatchOutcome::CatalogPublished { .. })
    ));
    let events = std::iter::from_fn(|| receiver.try_recv().ok())
        .map(|frame| {
            serde_json::from_str::<WsServerEnvelope<WatchServerEventV1>>(&frame)
                .expect("typed watch event")
                .into_payload()
        })
        .collect::<Vec<_>>();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, WatchServerEventV1::OpenObservation { .. }))
    );
    assert_eq!(
        open_runtime
            .snapshot(&fixture.target)
            .expect("runtime")
            .next_due_at,
        Some(clock.expected_wall(10))
    );

    clock.advance(Duration::from_secs(95));
    assert!(matches!(
        coordinator.run_next().await.expect("overdue Open dispatch"),
        Some(CoordinatorDispatchOutcome::OpenCompleted { .. })
    ));
    assert_eq!(
        open_runtime
            .snapshot(&fixture.target)
            .expect("runtime")
            .next_due_at,
        Some(clock.expected_wall(100))
    );
    assert!(coordinator.run_next().await.expect("no catch up").is_none());
    assert_eq!(upstream_state.maximum_active.load(Ordering::SeqCst), 1);
    assert!(upstream_state.all_fetches_lock_free.load(Ordering::SeqCst));
    let status = readiness.snapshots.lock().expect("status");
    assert!(status.iter().any(|snapshot| snapshot.in_flight));
    assert!(!status.last().expect("latest status").in_flight);
}

#[tokio::test]
async fn catalog_transport_recovery_uses_nb_weighted_continuous_backoff() {
    let fixture = fixture();
    let clock = FakeClock::default();
    let upstream_state = Arc::new(FakeUpstream::new(
        Arc::clone(&fixture.storage),
        [
            Err(bcsp_application::CatalogPullFailure::Transport),
            Err(bcsp_application::CatalogPullFailure::Transport),
            Ok(fixture.catalog_response.clone()),
        ],
        [FakeOpenResult::Success(vec![
            fixture.section.index().clone(),
        ])],
    ));
    let mut coordinator = SharedRefreshCoordinator::with_parts(
        Arc::clone(&fixture.storage),
        FakeUpstreamHandle(Arc::clone(&upstream_state)),
        FixedRefreshPolicyProvider::new(policy(GeneralOpenInterval::public())),
        clock.clone(),
        FakeIds(150),
        trace(95),
        OpenCounterAudience::Public,
        create_socket(),
        Arc::new(OpenRuntimeSnapshotRegistry::default()),
        Arc::new(RecordingStatus::default()),
    );
    coordinator
        .register_target(
            ScheduledRefreshTarget::try_new(fixture.target.clone(), fixture.open_request)
                .expect("registration"),
        )
        .expect("register target");

    assert!(matches!(
        coordinator.run_next().await.expect("first Catalog failure"),
        Some(CoordinatorDispatchOutcome::CatalogFailed {
            failure: bcsp_application::CatalogPullFailure::Transport,
            ..
        })
    ));
    clock.advance(Duration::from_millis(1));
    assert!(
        coordinator
            .run_next()
            .await
            .expect("retry remains in the future")
            .is_none()
    );
    clock.advance(Duration::from_secs(6));
    assert!(matches!(
        coordinator
            .run_next()
            .await
            .expect("second Catalog failure"),
        Some(CoordinatorDispatchOutcome::CatalogFailed {
            failure: bcsp_application::CatalogPullFailure::Transport,
            ..
        })
    ));
    assert_eq!(upstream_state.catalog_calls.load(Ordering::SeqCst), 2);

    clock.advance(Duration::from_millis(1));
    assert!(
        coordinator
            .run_next()
            .await
            .expect("second retry remains in the future")
            .is_none()
    );
    clock.advance(Duration::from_secs(11));
    assert!(matches!(
        coordinator.run_next().await.expect("Catalog recovery"),
        Some(CoordinatorDispatchOutcome::CatalogPublished { .. })
    ));
    assert_eq!(upstream_state.catalog_calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn failed_refreshes_retain_catalog_and_project_open_lkg_as_stale() {
    let fixture = fixture();
    let clock = FakeClock::default();
    let socket = create_socket();
    let _receiver = start_watch(&socket, fixture.section.clone());
    let index = fixture.section.index().clone();
    let upstream = FakeUpstreamHandle(Arc::new(FakeUpstream::new(
        Arc::clone(&fixture.storage),
        [
            Ok(fixture.catalog_response.clone()),
            Err(bcsp_application::CatalogPullFailure::Schema),
        ],
        [
            FakeOpenResult::Success(vec![index.clone()]),
            FakeOpenResult::Failure(OpenSectionsError::TransientHttp { status: 503 }),
            FakeOpenResult::Success(vec![index]),
        ],
    )));
    let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
    let fixed_policy = FixedRefreshPolicyProvider::new(policy(GeneralOpenInterval::public()));
    let status = Arc::new(RecordingStatus::default());
    let mut coordinator = SharedRefreshCoordinator::with_parts(
        Arc::clone(&fixture.storage),
        upstream,
        fixed_policy,
        clock.clone(),
        FakeIds(200),
        trace(91),
        OpenCounterAudience::Public,
        socket,
        Arc::clone(&open_runtime),
        status.clone(),
    );
    coordinator
        .register_target(
            ScheduledRefreshTarget::try_new(fixture.target.clone(), fixture.open_request)
                .expect("registration"),
        )
        .expect("register target");
    coordinator
        .activate_open_target(&fixture.target)
        .expect("applied target activation");
    coordinator
        .run_next()
        .await
        .expect("initial complete snapshot");

    clock.advance(Duration::from_secs(30));
    assert!(matches!(
        coordinator.run_next().await.expect("failed Open"),
        Some(CoordinatorDispatchOutcome::OpenCompleted {
            terminal: OpenDispatchTerminal::Failed,
            observation_count: 0,
            ..
        })
    ));
    let latest_activity = status
        .activities
        .lock()
        .expect("activity lock")
        .last()
        .cloned()
        .expect("failed Open activity");
    assert_eq!(latest_activity.phase, ServiceOperationPhaseV1::RetryWait);
    assert_eq!(latest_activity.target, Some(fixture.target.clone()));
    assert!(latest_activity.next_retry_at.is_some());
    let target_error = status
        .target_activities
        .lock()
        .expect("target activity lock")
        .last()
        .and_then(|activity| activity.error.clone())
        .expect("Open target error");
    assert_eq!(target_error.code, "OPEN_TRANSIENT_HTTP");
    assert_eq!(target_error.http_status, Some(503));
    assert!(target_error.trace_id.is_some());
    let runtime =
        SharedRuntimeContext::new(OpenCounterAudience::Public, clock.clone(), fixed_policy);
    let projected = runtime
        .open_status(
            &mut fixture.storage.lock().expect("storage"),
            &fixture.target,
            &open_runtime.snapshot(&fixture.target).expect("runtime"),
        )
        .expect("project stale Open");
    assert_eq!(projected.refresh.freshness.state, OpenFreshnessState::Stale);
    assert_eq!(projected.sections[0].state, OpenState::Open);

    clock.advance(Duration::from_secs(570));
    assert!(matches!(
        coordinator.run_next().await.expect("overdue Open"),
        Some(CoordinatorDispatchOutcome::OpenCompleted { .. })
    ));
    assert!(matches!(
        coordinator.run_next().await.expect("failed Catalog"),
        Some(CoordinatorDispatchOutcome::CatalogFailed {
            failure: bcsp_application::CatalogPullFailure::Schema,
            ..
        })
    ));
    let mut storage = fixture.storage.lock().expect("storage");
    let published = storage
        .published_catalog_snapshot(&fixture.target)
        .expect("Catalog read")
        .expect("Catalog LKG");
    assert_eq!(published.content_version, 1);
    assert_eq!(published.snapshot.sections.len(), 1);
}

#[tokio::test]
async fn first_open_failure_projects_unknown_without_mass_closing_sections() {
    let fixture = fixture();
    let clock = FakeClock::default();
    let socket = create_socket();
    let upstream = FakeUpstreamHandle(Arc::new(FakeUpstream::new(
        Arc::clone(&fixture.storage),
        [Ok(fixture.catalog_response)],
        [FakeOpenResult::Failure(OpenSectionsError::TransientHttp {
            status: 503,
        })],
    )));
    let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
    let fixed_policy = FixedRefreshPolicyProvider::new(policy(
        GeneralOpenInterval::local(3_600).expect("local maximum cadence"),
    ));
    let status = Arc::new(RecordingStatus::default());
    let mut coordinator = SharedRefreshCoordinator::with_parts(
        Arc::clone(&fixture.storage),
        upstream,
        fixed_policy,
        clock.clone(),
        FakeIds(300),
        trace(92),
        OpenCounterAudience::Local { run_id: trace(92) },
        socket,
        Arc::clone(&open_runtime),
        status.clone(),
    );
    coordinator
        .register_target(
            ScheduledRefreshTarget::try_new(fixture.target.clone(), fixture.open_request)
                .expect("registration"),
        )
        .expect("register target");
    assert!(matches!(
        coordinator
            .run_next()
            .await
            .expect("failed initial complete snapshot"),
        Some(CoordinatorDispatchOutcome::CompleteSnapshotOpenFailed {
            terminal: bcsp_application::OpenDispatchTerminal::Failed,
            error_code: "OPEN_TRANSIENT_HTTP",
            ..
        })
    ));

    let retry = status
        .target_activities
        .lock()
        .expect("target activities")
        .last()
        .cloned()
        .expect("retry activity");
    assert_eq!(retry.work_state, ServiceWorkStateV2::RetryWait);
    assert_eq!(retry.stage, Some(ServiceOperationStageV2::OpenFetch));
    assert_eq!(
        retry.error.as_ref().map(|error| error.code.as_str()),
        Some("OPEN_TRANSIENT_HTTP")
    );
    let error = retry.error.expect("candidate Open error");
    assert_eq!(error.http_status, Some(503));
    assert!(error.trace_id.is_some());

    let mut storage = fixture.storage.lock().expect("storage");
    assert!(
        storage
            .published_catalog_snapshot(&fixture.target)
            .expect("Catalog read")
            .is_none(),
        "Catalog-only data must not become serving state"
    );
    assert!(
        !storage
            .complete_target_snapshot_state(&fixture.target)
            .expect("complete target state")
            .ready
    );
}

fn wide_catalog_body(section_count: u32) -> Vec<u8> {
    let sections = (0..section_count)
        .map(|ordinal| {
            format!(
                r#"{{"campusCode":"NB","index":"{:05}","number":"{:02}","sectionCourseType":"LECTURE","openStatus":false,"meetingTimes":[]}}"#,
                10_001 + ordinal,
                ordinal + 1,
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"[{{"campusCode":"NB","courseString":"01:198:111","subject":"198","subjectDescription":"Computer Science","courseNumber":"111","title":"Introduction to Computer Science","sections":[{sections}]}}]"#
    )
    .into_bytes()
}

fn wide_catalog_response(
    fixture: &Fixture,
    section_count: u32,
) -> bcsp_application::CatalogPullResponse {
    let body = wide_catalog_body(section_count);
    bcsp_application::CatalogPullResponse {
        target: fixture.target.clone(),
        courses: decode_catalog_payload(&body).expect("wide synthetic Catalog"),
        provenance: SourceProvenance::from_body(
            "SYNTHETIC_CATALOG",
            "2030-01-01T00:00:00Z",
            &body,
        ),
        selector_confirms_target: true,
    }
}

fn wide_indexes(count: u32) -> Vec<SectionIndex> {
    (0..count)
        .map(|ordinal| {
            SectionIndex::try_from(format!("{:05}", 10_001 + ordinal).as_str())
                .expect("synthetic index")
        })
        .collect()
}

/// S1 end-to-end acceptance: the snapshot integrity gate on a REAL
/// coordinator over REAL SQLite storage, driven by a fake upstream.
///
/// Sequence: (1) catalog + full open snapshot applies and seeds the gate
/// baseline through the candidate-route promotion; (2) an implausibly
/// shrunken snapshot (20 of 60) is HELD -- persisted as
/// SUSPECT_PARTIAL_SNAPSHOT, zero watch fanout, `/open/status` keeps
/// projecting (uncertainty + failure class) and the per-Section LKG state
/// stays visible; (3) the recovered full snapshot applies immediately;
/// (4) after a process restart (new coordinator, same storage) a changed
/// candidate catalog seeds its gate from the persisted LKG through the
/// production persistence wrapper and still holds the partial, discarding
/// the staged catalog instead of publishing a gutted snapshot.
#[tokio::test]
async fn snapshot_integrity_gate_holds_partials_end_to_end_and_across_restart() {
    let fixture = fixture();
    let clock = FakeClock::default();
    let socket = create_socket();
    let full = wide_indexes(60);
    let partial: Vec<SectionIndex> = full[..20].to_vec();

    let upstream_state = Arc::new(FakeUpstream::new(
        Arc::clone(&fixture.storage),
        [Ok(wide_catalog_response(&fixture, 60))],
        [
            FakeOpenResult::Success(full.clone()),
            FakeOpenResult::Success(partial.clone()),
            FakeOpenResult::Success(full.clone()),
        ],
    ));
    let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
    let fixed_policy = FixedRefreshPolicyProvider::new(policy(GeneralOpenInterval::public()));
    let mut coordinator = SharedRefreshCoordinator::with_parts(
        Arc::clone(&fixture.storage),
        FakeUpstreamHandle(Arc::clone(&upstream_state)),
        fixed_policy,
        clock.clone(),
        FakeIds(500),
        trace(97),
        OpenCounterAudience::Public,
        Arc::clone(&socket),
        Arc::clone(&open_runtime),
        Arc::new(RecordingStatus::default()),
    );
    coordinator
        .register_target(
            ScheduledRefreshTarget::try_new(fixture.target.clone(), fixture.open_request.clone())
                .expect("registration"),
        )
        .expect("register target");
    let mut receiver = start_watch(&socket, fixture.section.clone());
    let routes = SharedProductRoutes::new(
        Arc::clone(&fixture.storage),
        SharedRuntimeContext::new(OpenCounterAudience::Public, clock.clone(), fixed_policy),
        Arc::clone(&open_runtime),
    );
    let batch_status = |routes: &ProductRoutes| -> OpenRefreshStatusV1 {
        post(
            routes,
            PRODUCT_OPEN_STATUS_PATH,
            OpenStatusRequestV1::new(OpenBatchKey::from(fixture.target.clone())),
        )
    };

    // (1) Catalog + full snapshot: applies, seeds, promotes to serving.
    assert!(matches!(
        coordinator.run_next().await.expect("Catalog dispatch"),
        Some(CoordinatorDispatchOutcome::CatalogPublished { .. })
    ));
    let events = watch_events(&mut receiver);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, WatchServerEventV1::OpenObservation { .. })),
        "the applied full snapshot fans out to the active watch",
    );

    // (2) Partial 20 of 60 (missing 40 >= max(ceil(60/10), 25)): HELD.
    clock.advance(Duration::from_secs(30));
    assert!(matches!(
        coordinator.run_next().await.expect("partial Open dispatch"),
        Some(CoordinatorDispatchOutcome::OpenCompleted {
            observation_count: 0,
            ..
        })
    ));
    assert!(
        watch_events(&mut receiver).is_empty(),
        "a held snapshot must fan out nothing",
    );
    let open = batch_status(&routes);
    assert_eq!(
        open.latest_attempt.expect("latest attempt").classification,
        OpenRefreshClassification::SuspectPartialSnapshot,
    );
    assert_eq!(
        open.latest_failure.expect("failure point").failure_class,
        OpenFailureClass::SuspectPartialSnapshot,
        "the suspect failure point must project, not break the status route",
    );
    assert_eq!(
        open.freshness.uncertainty,
        Some(OpenUncertaintyReason::SuspectPartialUpstream),
    );
    let open_section: OpenSectionStatusV1 = post(
        &routes,
        PRODUCT_OPEN_SECTION_STATUS_PATH,
        OpenSectionStatusRequestV1::new(fixture.section.clone()),
    );
    assert_eq!(
        open_section.state,
        OpenState::Open,
        "the last-known-good per-Section state survives the hold",
    );

    // (3) Recovery: the full snapshot applies immediately and fans out.
    clock.advance(Duration::from_secs(30));
    assert!(matches!(
        coordinator.run_next().await.expect("recovery Open dispatch"),
        Some(CoordinatorDispatchOutcome::OpenCompleted { .. })
    ));
    let events = watch_events(&mut receiver);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, WatchServerEventV1::OpenObservation { .. })),
        "recovery resumes normal fanout",
    );
    let open = batch_status(&routes);
    assert_eq!(
        open.latest_attempt.expect("latest attempt").classification,
        OpenRefreshClassification::ValidApplied,
    );
    assert_eq!(open.freshness.uncertainty, None);

    // (4) Restart: a fresh coordinator (empty in-memory gate state) over the
    // SAME storage. A changed candidate catalog (61 sections) forces the
    // candidate route; its gate seed |persisted LKG ∩ candidate catalog| MUST
    // flow through the production persistence wrapper -- a fail-open read
    // would seed from the partial itself and publish the gutted snapshot.
    drop(coordinator);
    let restarted_upstream = Arc::new(FakeUpstream::new(
        Arc::clone(&fixture.storage),
        [Ok(wide_catalog_response(&fixture, 61))],
        [FakeOpenResult::Success(partial.clone())],
    ));
    let mut restarted = SharedRefreshCoordinator::with_parts(
        Arc::clone(&fixture.storage),
        FakeUpstreamHandle(Arc::clone(&restarted_upstream)),
        fixed_policy,
        clock.clone(),
        FakeIds(600),
        trace(98),
        OpenCounterAudience::Public,
        Arc::clone(&socket),
        Arc::clone(&open_runtime),
        Arc::new(RecordingStatus::default()),
    );
    restarted
        .register_target(
            ScheduledRefreshTarget::try_new(fixture.target.clone(), fixture.open_request.clone())
                .expect("registration"),
        )
        .expect("register restarted target");
    clock.advance(Duration::from_secs(15));
    let outcome = restarted
        .run_next()
        .await
        .expect("restart Catalog dispatch");
    assert!(
        matches!(
            outcome,
            Some(CoordinatorDispatchOutcome::CompleteSnapshotOpenFailed {
                error_code: "OPEN_RESPONSE_UNSAFE",
                ..
            })
        ),
        "the held candidate must discard the staged catalog, got {outcome:?}",
    );
    assert!(
        watch_events(&mut receiver).is_empty(),
        "the restart hold fans out nothing",
    );
    let open = batch_status(&routes);
    assert_eq!(
        open.latest_attempt.expect("latest attempt").classification,
        OpenRefreshClassification::SuspectPartialSnapshot,
    );
    let open_section: OpenSectionStatusV1 = post(
        &routes,
        PRODUCT_OPEN_SECTION_STATUS_PATH,
        OpenSectionStatusRequestV1::new(fixture.section.clone()),
    );
    assert_eq!(open_section.state, OpenState::Open);
    let storage = fixture.storage.lock().expect("storage");
    let state = storage
        .complete_target_snapshot_state(&fixture.target)
        .expect("complete state");
    assert!(state.ready, "the previous complete pair stays serving");
    assert_eq!(
        state.catalog_content_version, 1,
        "the 61-section candidate catalog must not have published",
    );
}
