use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bcsp_application::{
    ApplicationClock, CoordinatorClock, CoordinatorDispatchOutcome, CoordinatorStatusSink,
    CoordinatorStatusSnapshot, ExtensionRequest, FixedRefreshPolicyProvider, OpenDispatchTerminal,
    OpenRuntimeSnapshotRegistry, PRODUCT_CATALOG_DISCOVERY_PATH, PRODUCT_COURSE_DETAIL_PATH,
    PRODUCT_COURSE_SEARCH_PATH, PRODUCT_FILTER_SCHEMA_PATH, PRODUCT_OPEN_SECTION_STATUS_PATH,
    PRODUCT_OPEN_STATUS_PATH, PRODUCT_SECTION_DETAIL_PATH, PRODUCT_SECTION_SEARCH_PATH,
    RefreshFuture, RefreshPolicy, RefreshUpstream, RequestMethod, RouteExtension,
    ScheduledRefreshTarget, SharedProductRoutes, SharedRefreshCoordinator, SharedRuntimeContext,
    SharedWatchSocket, WebSocketExtension, publish_discovery_for_refresh,
};
use bcsp_contracts::{
    CatalogDiscoveryRequestV1, CatalogDiscoveryResponseV1, CourseDetailRequestV1,
    CourseDetailResponseV1, CourseQueryRequestV1, CourseQueryResponseV1, CourseSortV1,
    FilterRequestV1, FilterSchemaV1, FilterSearchTextV1, FilterValuesInputV1, HttpRequestEnvelope,
    HttpSuccessEnvelope, LiveOpenStateV1, NormalizedFilterValuesV1, OpenBatchKey,
    OpenFreshnessState, OpenRefreshStatusV1, OpenSchedulerLane, OpenSectionStatusRequestV1,
    OpenSectionStatusV1, OpenState, OpenStatusRequestV1, PageRequestV1, SectionDetailRequestV1,
    SectionDetailResponseV1, SectionIndex, SectionKey, SectionQueryRequestV1,
    SectionQueryResponseV1, SectionSortV1, TermCampusKey, TermId, TraceId, TraceIdSource,
    WatchClientCommandV1, WatchPolicyV1, WatchServerEventV1, WatchStartItemResultV1,
    WatchStartItemV1, WatchStartItemsV1, WsClientEnvelope, WsServerEnvelope,
};
use bcsp_open::{GeneralOpenInterval, MonotonicTime, OpenCounterAudience};
use bcsp_operational_storage::OperationalStorage;
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

const BASE_UNIX_SECONDS: i64 = 1_893_456_000;
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
        }
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
    let (outbound, mut receiver) = mpsc::unbounded_channel();
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
        Arc::new(RecordingStatus::default()),
    );
    coordinator
        .register_target(
            ScheduledRefreshTarget::try_new(fixture.target.clone(), fixture.open_request.clone())
                .expect("registration"),
        )
        .expect("register target");

    assert!(matches!(
        coordinator.run_next().await.expect("Catalog dispatch"),
        Some(CoordinatorDispatchOutcome::CatalogPublished { .. })
    ));
    let mut receiver = start_watch(&socket, fixture.section.clone());
    assert!(matches!(
        coordinator.run_next().await.expect("Open dispatch"),
        Some(CoordinatorDispatchOutcome::OpenCompleted {
            terminal: OpenDispatchTerminal::Valid,
            observation_count: 1,
            ..
        })
    ));
    assert_eq!(upstream_state.catalog_calls.load(Ordering::SeqCst), 1);
    assert_eq!(upstream_state.open_calls.load(Ordering::SeqCst), 1);

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
    assert!(matches!(
        coordinator.run_next().await.expect("Open dispatch"),
        Some(CoordinatorDispatchOutcome::OpenCompleted {
            terminal: OpenDispatchTerminal::Valid,
            observation_count: 1,
            ..
        })
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
            FakeOpenResult::Failure(OpenSectionsError::Network),
            FakeOpenResult::Success(vec![index]),
        ],
    )));
    let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
    let fixed_policy = FixedRefreshPolicyProvider::new(policy(GeneralOpenInterval::public()));
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
        Arc::new(RecordingStatus::default()),
    );
    coordinator
        .register_target(
            ScheduledRefreshTarget::try_new(fixture.target.clone(), fixture.open_request)
                .expect("registration"),
        )
        .expect("register target");
    coordinator.run_next().await.expect("initial Catalog");
    coordinator.run_next().await.expect("initial Open");

    clock.advance(Duration::from_secs(30));
    assert!(matches!(
        coordinator.run_next().await.expect("failed Open"),
        Some(CoordinatorDispatchOutcome::OpenCompleted {
            terminal: OpenDispatchTerminal::Failed,
            observation_count: 0,
            ..
        })
    ));
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
        [FakeOpenResult::Failure(OpenSectionsError::Network)],
    )));
    let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
    let fixed_policy = FixedRefreshPolicyProvider::new(policy(
        GeneralOpenInterval::local(3_600).expect("local maximum cadence"),
    ));
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
        Arc::new(RecordingStatus::default()),
    );
    coordinator
        .register_target(
            ScheduledRefreshTarget::try_new(fixture.target.clone(), fixture.open_request)
                .expect("registration"),
        )
        .expect("register target");
    coordinator.run_next().await.expect("initial Catalog");
    coordinator.run_next().await.expect("failed first Open");

    let runtime = SharedRuntimeContext::new(
        OpenCounterAudience::Local { run_id: trace(92) },
        clock,
        fixed_policy,
    );
    let projected = runtime
        .open_status(
            &mut fixture.storage.lock().expect("storage"),
            &fixture.target,
            &open_runtime.snapshot(&fixture.target).expect("runtime"),
        )
        .expect("project unknown Open");
    assert_eq!(
        projected.refresh.freshness.state,
        OpenFreshnessState::Unknown
    );
    assert!(
        projected
            .sections
            .iter()
            .all(|section| section.state == OpenState::Unknown)
    );
}
