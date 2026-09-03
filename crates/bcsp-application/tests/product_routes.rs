use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bcsp_application::{
    ApplicationClock, CoordinatorStatusSink, ExtensionRequest, FixedRefreshPolicyProvider,
    OpenRuntimeSnapshot, OpenRuntimeSnapshotRegistry, PRODUCT_CATALOG_DISCOVERY_PATH,
    PRODUCT_COURSE_DETAIL_PATH, PRODUCT_COURSE_SEARCH_PATH, PRODUCT_DYNAMIC_FILTER_VALIDATION_PATH,
    PRODUCT_FILTER_OPTIONS_PATH, PRODUCT_FILTER_SCHEMA_PATH, PRODUCT_OPEN_BATCH_STATUS_PATH,
    PRODUCT_OPEN_SECTION_STATUS_PATH, PRODUCT_OPEN_STATUS_PATH, PRODUCT_SECTION_DETAIL_PATH,
    PRODUCT_SECTION_SEARCH_PATH, PRODUCT_SERVICE_STATUS_PATH, PreparedServingError,
    PreparedServingRebuildRuntime, REFRESH_MAX_CONCURRENCY, RefreshPolicy, RefreshPolicyProvider,
    RefreshPolicyReadError, RequestMethod, RouteExtension, SHARED_PRODUCT_ROUTE_INVENTORY,
    ServiceStatusRegistry, SharedProductRoutes, SharedRuntimeContext, TargetRefreshDemand,
    TargetWorkActivity, TargetWorkflowKind, WorkflowOperationActivity, WorkflowOperationId,
};
use bcsp_catalog::{normalize_target, to_catalog_refresh_command, to_discovery_refresh_command};
use bcsp_contracts::{
    ApiErrorCode, ApiErrorDetail, ApiErrorEnvelope, CampusCode, CatalogDiscoveryRequestV1,
    CatalogDiscoveryResponseV1, CatalogFieldKnowledge, CatalogSubjectProvenanceV1,
    CourseDetailRequestV1, CourseDetailResponseV1, CourseQueryRequestV1, CourseQueryResponseV1,
    CourseSortV1, DynamicFilterValidationRequestV3, DynamicFilterValidationResponseV3,
    FilterFieldId, FilterOptionsFieldV2, FilterOptionsRequestV2, FilterOptionsResponseV2,
    FilterRequestV1, FilterSchemaV1, FilterSearchTextV1, FilterTokenV1, FilterValuesInputV1,
    HttpRequestEnvelope, HttpSuccessEnvelope, LiveOpenStateV1, OpenBatchKey,
    OpenBatchStatusRequestV1, OpenBatchStatusV1, OpenRefreshStatusV1, OpenSchedulerLane,
    OpenSectionStatusRequestV1, OpenSectionStatusV1, OpenState, OpenStatusRequestV1, PageRequestV1,
    QUERY_CONTRACT_VERSION, SectionDetailRequestV1, SectionDetailResponseV1, SectionKey,
    SectionQueryRequestV1, SectionQueryResponseV1, SectionSortV1, ServiceLevelV1,
    ServiceOperationStageV2, ServiceRuntimeV1, ServiceSnapshotAvailabilityV2, ServiceStatusV2,
    ServiceTargetErrorV2, ServiceTermPublicationV2, ServiceWorkStateV2, TermCampusKey, TermId,
    TraceId,
};
use bcsp_open::{GeneralOpenInterval, OpenCounterAudience};
use bcsp_operational_storage::{
    BeginOpenPullAttemptCommand, EmptySnapshotDecision, FinishOpenPullSuccessCommand,
    OpenCacheStatus, OpenHttpAuditMetadata, OpenRequestLane, OperationalStorage, PublishOutcome,
};
use bcsp_rutgers_client::{
    DiscoverySnapshot, DiscoverySourceInput, SourceProvenance, decode_catalog_payload,
    decode_discovery_payload,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use tempfile::TempDir;
use time::OffsetDateTime;

const STARTED: &str = "2026-07-17T12:00:00Z";
const COMPLETED: &str = "2026-07-17T12:00:01Z";
const BODY_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const WINDOW_NOW_UNIX: i64 = 1_784_289_602;

#[derive(Clone, Copy)]
struct FixedClock(OffsetDateTime);

impl ApplicationClock for FixedClock {
    fn now(&self) -> OffsetDateTime {
        self.0
    }
}

type Routes = SharedProductRoutes<FixedClock, FixedRefreshPolicyProvider>;

struct Fixture {
    _directory: TempDir,
    routes: Routes,
    storage: Arc<Mutex<OperationalStorage>>,
    status: Arc<ServiceStatusRegistry>,
    target: TermCampusKey,
    section: SectionKey,
    content_version: u64,
}

fn trace(suffix: u8) -> TraceId {
    format!("00000000-0000-4000-8000-{suffix:012x}")
        .parse()
        .expect("synthetic trace ID")
}

fn publish_discovery(storage: &mut OperationalStorage) {
    publish_discovery_campuses(storage, &[("NB", "New Brunswick")]);
}

fn publish_discovery_campuses(storage: &mut OperationalStorage, campuses: &[(&str, &str)]) {
    publish_discovery_campuses_with_suffix(storage, campuses, 1);
}

fn publish_discovery_campuses_with_suffix(
    storage: &mut OperationalStorage,
    campuses: &[(&str, &str)],
    suffix: u8,
) {
    let body = serde_json::to_vec(&serde_json::json!({
      "sourceVersion": "synthetic-v1",
      "terms": [
        {
          "termId": "72026",
          "year": 2026,
          "termCode": "7",
          "display": "Summer 2026",
          "published": true
        },
        {
          "termId": "92026",
          "year": 2026,
          "termCode": "9",
          "display": "Fall 2026",
          "published": true
        }
      ],
      "campuses": campuses.iter().map(|(code, display)| serde_json::json!({
        "campusCode": code,
        "display": display,
        "enabled": true
      })).collect::<Vec<_>>(),
      "targets": campuses.iter().map(|(code, _)| serde_json::json!({
        "termId": "72026",
        "campusCode": code,
        "enabled": true
      })).collect::<Vec<_>>(),
      "subjects": []
    }))
    .expect("synthetic discovery JSON");
    let snapshot = DiscoverySnapshot::try_from_bundle(vec![DiscoverySourceInput::selector(
        decode_discovery_payload(&body).expect("synthetic discovery JSON"),
        SourceProvenance::from_body("SYNTHETIC_DISCOVERY", STARTED, &body),
    )])
    .expect("synthetic discovery");
    storage
        .apply_discovery_refresh(
            to_discovery_refresh_command(&snapshot, trace(suffix), STARTED, COMPLETED)
                .expect("discovery command"),
        )
        .expect("publish discovery");
}

fn publish_catalog(storage: &mut OperationalStorage, target: &TermCampusKey) -> u64 {
    publish_catalog_subject(
        storage,
        target,
        "198",
        "Computer Science",
        2,
        STARTED,
        COMPLETED,
    )
}

fn publish_catalog_subject(
    storage: &mut OperationalStorage,
    target: &TermCampusKey,
    subject: &str,
    subject_description: &str,
    observation_suffix: u8,
    started_at: &str,
    completed_at: &str,
) -> u64 {
    publish_catalog_subject_with_core(
        storage,
        target,
        (subject, subject_description),
        observation_suffix,
        started_at,
        completed_at,
        ("CCO", "Communication"),
    )
}

fn publish_catalog_subject_with_core(
    storage: &mut OperationalStorage,
    target: &TermCampusKey,
    (subject, subject_description): (&str, &str),
    observation_suffix: u8,
    started_at: &str,
    completed_at: &str,
    (core_code, core_description): (&str, &str),
) -> u64 {
    let body = serde_json::to_vec(&serde_json::json!([{
        "campusCode": target.campus().as_str(),
        "courseString": format!("01:{subject}:111"),
        "subject": subject,
        "subjectDescription": subject_description,
        "courseNumber": "111",
        "title": "Introduction to a Synthetic Subject",
        "coreCodes": [{
            "coreCode": core_code,
            "coreCodeDescription": core_description
        }],
        "sections": [{
            "campusCode": target.campus().as_str(),
            "index": "10001",
            "number": "01",
            "sectionCourseType": "LECTURE",
            "openStatus": false,
            "meetingTimes": [],
            "instructors": [{"name": "Pat Smith"}]
        }]
    }]))
    .expect("synthetic Catalog JSON");
    let request_id = format!("SYNTHETIC_CATALOG_{observation_suffix}");
    let normalized = normalize_target(
        target.clone(),
        decode_catalog_payload(&body).expect("synthetic Catalog JSON"),
        SourceProvenance::from_body(&request_id, started_at, &body),
    )
    .expect("normalize synthetic Catalog");
    match storage
        .apply_catalog_refresh(
            to_catalog_refresh_command(
                &normalized,
                trace(observation_suffix),
                started_at,
                completed_at,
            )
            .expect("Catalog command"),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            bcsp_catalog::CATALOG_DERIVATION_VERSION,
        )
        .expect("publish Catalog")
    {
        PublishOutcome::AppliedChanged {
            content_version, ..
        }
        | PublishOutcome::AppliedUnchanged {
            content_version, ..
        } => content_version,
        outcome => panic!("unexpected Catalog outcome: {outcome:?}"),
    }
}

fn publish_open(
    storage: &mut OperationalStorage,
    target: &TermCampusKey,
    section: &SectionKey,
    catalog_content_version: u64,
) {
    publish_open_with_suffix(storage, target, section, catalog_content_version, 3, 4);
}

fn publish_open_with_suffix(
    storage: &mut OperationalStorage,
    target: &TermCampusKey,
    section: &SectionKey,
    catalog_content_version: u64,
    attempt_suffix: u8,
    run_suffix: u8,
) {
    publish_open_state_with_suffix(
        storage,
        target,
        section,
        catalog_content_version,
        attempt_suffix,
        run_suffix,
        true,
    );
}

fn publish_open_state_with_suffix(
    storage: &mut OperationalStorage,
    target: &TermCampusKey,
    section: &SectionKey,
    catalog_content_version: u64,
    attempt_suffix: u8,
    run_suffix: u8,
    open: bool,
) {
    storage
        .begin_open_pull_attempt(&BeginOpenPullAttemptCommand {
            attempt_id: trace(attempt_suffix),
            run_id: trace(run_suffix),
            target: target.clone(),
            captured_catalog_content_version: catalog_content_version,
            rutgers_day: "2026-07-17".to_owned(),
            started_at: STARTED.to_owned(),
            lane: OpenRequestLane::ActiveWatch,
            requested_interval_seconds: Some(30),
            effective_interval_seconds: Some(10),
            schedule_lag_ms: Some(25),
        })
        .expect("begin synthetic Open pull");
    storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
            gate_hold: false,
            gate_catalog_set_identity: None,
            attempt_id: trace(attempt_suffix),
            completed_at: COMPLETED.to_owned(),
            open_sections: open.then(|| section.clone()).into_iter().collect(),
            source_value_count: u64::from(open),
            watched_sections: vec![section.clone()],
            http: OpenHttpAuditMetadata {
                http_status: Some(200),
                cache_status: Some(OpenCacheStatus::Miss),
                decoded_bytes: Some(9),
                decoded_body_sha256: Some(BODY_SHA256.to_owned()),
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

fn fixture() -> Fixture {
    let directory = TempDir::new().expect("temporary directory");
    let database = directory.path().join("product-routes.sqlite");
    let mut storage = OperationalStorage::open(database).expect("file-backed operational SQLite");
    let target = TermCampusKey::try_new("72026", "NB").expect("synthetic target");
    let section = SectionKey::try_new("72026", "NB", "10001").expect("synthetic section");
    publish_discovery(&mut storage);
    let content_version = publish_catalog(&mut storage, &target);
    publish_open(&mut storage, &target, &section, content_version);

    let now = OffsetDateTime::from_unix_timestamp(WINDOW_NOW_UNIX).expect("2026-07-17 timestamp");
    let policy = RefreshPolicy::try_new(Duration::from_secs(600), GeneralOpenInterval::public())
        .expect("fixed public refresh policy");
    let runtime = SharedRuntimeContext::new(
        OpenCounterAudience::Public,
        FixedClock(now),
        FixedRefreshPolicyProvider::new(policy),
    );
    let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
    open_runtime
        .replace(
            target.clone(),
            OpenRuntimeSnapshot {
                lane: OpenSchedulerLane::ActiveWatch,
                active_watch_count: 2,
                scheduler_lag_milliseconds: 25,
                ..OpenRuntimeSnapshot::default()
            },
        )
        .expect("register target runtime");
    let storage = Arc::new(Mutex::new(storage));
    let status = Arc::new(ServiceStatusRegistry::new(ServiceRuntimeV1::Public));
    let routes = SharedProductRoutes::new(storage.clone(), runtime, open_runtime)
        .with_service_status(Arc::clone(&status));
    Fixture {
        _directory: directory,
        routes,
        storage,
        status,
        target,
        section,
        content_version,
    }
}

/// Holds a mutex on a helper thread until released, so a test can keep the
/// rebuild worker parked on its storage lock without holding a std guard
/// across an await.
struct HeldMutex {
    release: Option<mpsc::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl HeldMutex {
    fn hold<T: Send + 'static>(mutex: Arc<Mutex<T>>) -> Self {
        let (held, held_rx) = mpsc::channel();
        let (release, release_rx) = mpsc::channel::<()>();
        let thread = std::thread::spawn(move || {
            let guard = mutex.lock().expect("hold the mutex");
            held.send(()).expect("announce the hold");
            let _ = release_rx.recv();
            drop(guard);
        });
        held_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the holder thread acquires the mutex");
        Self {
            release: Some(release),
            thread: Some(thread),
        }
    }

    fn release(mut self) {
        drop(self.release.take());
        if let Some(thread) = self.thread.take() {
            thread.join().expect("holder thread");
        }
    }
}

fn spawn_prepared_worker(
    storage: &Arc<Mutex<OperationalStorage>>,
    routes: &Routes,
) -> PreparedServingRebuildRuntime {
    let policy = RefreshPolicy::try_new(Duration::from_secs(600), GeneralOpenInterval::public())
        .expect("fixed public refresh policy");
    PreparedServingRebuildRuntime::spawn(
        storage.clone(),
        FixedRefreshPolicyProvider::new(policy),
        OpenCounterAudience::Public,
        routes.open_runtime(),
        routes.prepared_serving_registry(),
    )
}

async fn course_search_response(
    routes: Arc<Routes>,
) -> (bcsp_application::ExtensionResponse, Duration) {
    tokio::task::spawn_blocking(move || {
        let body = serde_json::to_vec(&HttpRequestEnvelope::new(CourseQueryRequestV1 {
            filters: search_filters(),
            page: PageRequestV1::default(),
            sort: CourseSortV1::default(),
        }))
        .expect("request envelope");
        let started = Instant::now();
        let response = routes.handle(ExtensionRequest::new(
            RequestMethod::Post,
            PRODUCT_COURSE_SEARCH_PATH,
            None,
            body,
        ));
        (response, started.elapsed())
    })
    .await
    .expect("request task")
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn course_search_issued_during_committed_open_rebuild_succeeds_once_the_worker_clears() {
    let fixture = fixture();
    let routes = Arc::new(
        fixture
            .routes
            .with_prepared_admission_wait(Duration::from_secs(5)),
    );
    let registry = routes.prepared_serving_registry();
    let worker = spawn_prepared_worker(&fixture.storage, &routes);
    let demand = worker.demand();
    let old = Arc::clone(
        registry
            .snapshot()
            .expect("initial generation")
            .generation(),
    );

    // The authoritative Open attempt commits behind a publication barrier
    // exactly as the refresh coordinator does it: begin, write, commit.
    let barrier = demand
        .begin_open_publication(fixture.target.clone())
        .expect("Open publication barrier");
    {
        let mut storage = fixture.storage.lock().expect("authoritative writer lock");
        publish_open_state_with_suffix(
            &mut storage,
            &fixture.target,
            &fixture.section,
            fixture.content_version,
            5,
            6,
            false,
        );
    }
    // Keep the worker from clearing the ticket until the request is parked.
    let held = HeldMutex::hold(fixture.storage.clone());
    barrier.commit();
    assert!(matches!(
        registry.snapshot(),
        Err(PreparedServingError::SnapshotRebuilding)
    ));

    let request = tokio::spawn(course_search_response(routes.clone()));
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        !request.is_finished(),
        "the request must wait for the committed ticket instead of failing"
    );
    held.release();

    let (response, elapsed) = request.await.expect("request task");
    assert_eq!(
        response.status(),
        200,
        "{}",
        String::from_utf8_lossy(response.body())
    );
    assert!(elapsed >= Duration::from_millis(100), "elapsed {elapsed:?}");
    let courses =
        serde_json::from_slice::<HttpSuccessEnvelope<CourseQueryResponseV1>>(response.body())
            .expect("success envelope")
            .into_data();
    assert_eq!(
        courses.items[0].variants[0].sections[0].open.state,
        LiveOpenStateV1::Closed,
        "the response reflects the attempt that committed behind the barrier"
    );
    let admitted = registry.snapshot().expect("cleared ticket");
    assert!(!Arc::ptr_eq(admitted.generation(), &old));
    let vector = admitted
        .open_vector(std::slice::from_ref(&fixture.target))
        .expect("open vector");
    let old_vector = old
        .open_vector(std::slice::from_ref(&fixture.target))
        .expect("old vector");
    assert!(vector[0].attempt_sequence > old_vector[0].attempt_sequence);
    assert_eq!(registry.snapshot_wait_timeouts(), 0);

    worker.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn course_search_returns_503_after_admission_wait_timeout() {
    let fixture = fixture();
    let routes = Arc::new(
        fixture
            .routes
            .with_prepared_admission_wait(Duration::from_millis(50)),
    );
    assert_eq!(routes.prepared_admission_wait(), Duration::from_millis(50));
    let registry = routes.prepared_serving_registry();
    let worker = spawn_prepared_worker(&fixture.storage, &routes);
    let demand = worker.demand();

    let barrier = demand
        .begin_open_publication(fixture.target.clone())
        .expect("Open publication barrier");
    // The worker never gets its storage lock, so the ticket is never cleared.
    let held = HeldMutex::hold(fixture.storage.clone());
    barrier.commit();

    let (response, elapsed) = course_search_response(routes.clone()).await;
    assert_eq!(response.status(), 503);
    let error: ApiErrorEnvelope = serde_json::from_slice(response.body()).expect("error envelope");
    assert_eq!(error.error().code(), ApiErrorCode::UpstreamUnavailable);
    assert!(
        elapsed >= Duration::from_millis(50) && elapsed < Duration::from_secs(2),
        "elapsed {elapsed:?}"
    );
    assert_eq!(registry.snapshot_wait_timeouts(), 1);

    // Once the worker can run it clears the ticket and the same request succeeds.
    held.release();
    let waiting_registry = registry.clone();
    tokio::task::spawn_blocking(move || waiting_registry.snapshot_within(Duration::from_secs(5)))
        .await
        .expect("waiting task")
        .expect("the worker clears the ticket once it can rebuild");
    let (response, _) = course_search_response(routes.clone()).await;
    assert_eq!(response.status(), 200);

    worker.shutdown().await;
}

fn post<Request, Response>(
    routes: &impl RouteExtension,
    path: &'static str,
    request: Request,
) -> Response
where
    Request: Serialize,
    Response: DeserializeOwned,
{
    let body = serde_json::to_vec(&HttpRequestEnvelope::new(request)).expect("request envelope");
    let response = routes.handle(ExtensionRequest::new(RequestMethod::Post, path, None, body));
    assert_eq!(
        response.status(),
        200,
        "{}",
        String::from_utf8_lossy(response.body())
    );
    serde_json::from_slice::<HttpSuccessEnvelope<Response>>(response.body())
        .expect("success envelope")
        .into_data()
}

fn get<Response>(routes: &impl RouteExtension, path: &'static str) -> Response
where
    Response: DeserializeOwned,
{
    let response = routes.handle(ExtensionRequest::new(
        RequestMethod::Get,
        path,
        None,
        Vec::new(),
    ));
    assert_eq!(
        response.status(),
        200,
        "{}",
        String::from_utf8_lossy(response.body())
    );
    serde_json::from_slice::<HttpSuccessEnvelope<Response>>(response.body())
        .expect("success envelope")
        .into_data()
}

#[derive(Clone)]
struct StorageLockSensitivePolicy {
    storage: Arc<Mutex<OperationalStorage>>,
    policy: RefreshPolicy,
}

impl RefreshPolicyProvider for StorageLockSensitivePolicy {
    fn refresh_policy(&self) -> Result<RefreshPolicy, RefreshPolicyReadError> {
        let _storage = self
            .storage
            .try_lock()
            .map_err(|_| RefreshPolicyReadError)?;
        Ok(self.policy)
    }
}

fn search_filters() -> FilterRequestV1 {
    let mut input =
        FilterValuesInputV1::for_term(TermId::try_from("72026").expect("synthetic term"));
    input.text =
        Some(FilterSearchTextV1::try_from("Computer Science").expect("synthetic search text"));
    FilterRequestV1::new(
        bcsp_contracts::NormalizedFilterValuesV1::try_new(input).expect("normalized filters"),
    )
}

#[test]
fn shared_inventory_serves_real_sqlite_catalog_query_detail_and_open_projections() {
    let fixture = fixture();
    assert_eq!(SHARED_PRODUCT_ROUTE_INVENTORY.len(), 12);
    assert!(SHARED_PRODUCT_ROUTE_INVENTORY.iter().all(|route| {
        route.path().starts_with("/api/v1/")
            && matches!(route.method(), RequestMethod::Get | RequestMethod::Post)
    }));

    let schema_response = fixture.routes.handle(ExtensionRequest::new(
        RequestMethod::Get,
        PRODUCT_FILTER_SCHEMA_PATH,
        None,
        Vec::new(),
    ));
    assert_eq!(schema_response.status(), 200);
    let schema =
        serde_json::from_slice::<HttpSuccessEnvelope<FilterSchemaV1>>(schema_response.body())
            .expect("filter-schema envelope")
            .into_data();
    assert_eq!(schema.fields.len(), bcsp_contracts::FILTER_FIELD_COUNT);

    let discovery: CatalogDiscoveryResponseV1 = post(
        &fixture.routes,
        PRODUCT_CATALOG_DISCOVERY_PATH,
        CatalogDiscoveryRequestV1::new(),
    );
    assert_eq!(discovery.targets.len(), 1);
    assert_eq!(discovery.targets[0].key, fixture.target);

    let options: FilterOptionsResponseV2 = post(
        &fixture.routes,
        PRODUCT_FILTER_OPTIONS_PATH,
        FilterOptionsRequestV2 {
            contract_version: QUERY_CONTRACT_VERSION,
            term: TermId::try_from("72026").unwrap(),
            campuses: vec![CampusCode::try_from("NB").unwrap()],
            field: FilterOptionsFieldV2::Instructor,
            query: Some("SMI".to_owned()),
            limit: Some(10),
        },
    );
    assert_eq!(options.options.len(), 1);
    assert_eq!(options.options[0].value, "Pat Smith");

    let mut validation_input =
        FilterValuesInputV1::for_term(TermId::try_from("72026").expect("synthetic term"));
    validation_input.campuses = vec![CampusCode::try_from("NB").unwrap()];
    validation_input.subjects = ["997", "999"]
        .into_iter()
        .map(|value| value.try_into().unwrap())
        .collect();
    validation_input.text = Some(FilterSearchTextV1::try_from("retired missing").unwrap());
    validation_input.course_number_bands = vec![900];
    validation_input.levels = vec![FilterTokenV1::try_from("G").unwrap()];
    validation_input.core.codes = ["AAA", "ZZZ"]
        .into_iter()
        .map(|value| FilterTokenV1::try_from(value).unwrap())
        .collect();
    validation_input.instructors = vec![FilterTokenV1::try_from("Removed Instructor").unwrap()];
    validation_input.meeting_locations.locations = vec![FilterTokenV1::try_from("BUSCH").unwrap()];
    validation_input.exam_codes = vec![FilterTokenV1::try_from("X").unwrap()];
    let validation: DynamicFilterValidationResponseV3 = post(
        &fixture.routes,
        PRODUCT_DYNAMIC_FILTER_VALIDATION_PATH,
        DynamicFilterValidationRequestV3::new(FilterRequestV1::new(
            bcsp_contracts::NormalizedFilterValuesV1::try_new(validation_input)
                .expect("validation filters"),
        )),
    );
    assert_eq!(validation.contract_version, QUERY_CONTRACT_VERSION);
    assert_eq!(validation.target_versions.len(), 1);
    assert_eq!(validation.target_versions[0].target, fixture.target);
    assert_eq!(validation.target_versions[0].content_version.get(), 1);
    assert_eq!(
        validation
            .invalid_values
            .iter()
            .map(|invalid| (invalid.field, invalid.value.as_str()))
            .collect::<Vec<_>>(),
        vec![
            (FilterFieldId::CourseSubject, "997"),
            (FilterFieldId::CourseSubject, "999"),
            (FilterFieldId::CourseText, "missing"),
            (FilterFieldId::CourseText, "retired"),
            (FilterFieldId::CourseNumberBand, "900"),
            (FilterFieldId::CourseLevel, "G"),
            (FilterFieldId::CourseCoreCode, "AAA"),
            (FilterFieldId::CourseCoreCode, "ZZZ"),
            (FilterFieldId::SectionInstructor, "Removed Instructor"),
            (FilterFieldId::SectionMeetingLocation, "BUSCH"),
            (FilterFieldId::SectionExam, "X"),
        ]
    );

    let filters = search_filters();
    let courses: CourseQueryResponseV1 = post(
        &fixture.routes,
        PRODUCT_COURSE_SEARCH_PATH,
        CourseQueryRequestV1 {
            filters: filters.clone(),
            page: PageRequestV1::default(),
            sort: CourseSortV1::default(),
        },
    );
    assert_eq!(courses.page.total, 1);
    let course_key = courses.items[0].group.key.clone();
    assert_eq!(
        courses.items[0].variants[0].sections[0].open.state,
        LiveOpenStateV1::Open
    );

    let sections: SectionQueryResponseV1 = post(
        &fixture.routes,
        PRODUCT_SECTION_SEARCH_PATH,
        SectionQueryRequestV1 {
            filters,
            page: PageRequestV1::default(),
            sort: SectionSortV1::default(),
        },
    );
    assert_eq!(sections.page.total, 1);
    assert_eq!(sections.items[0].section.section.key, fixture.section);
    assert_eq!(sections.items[0].section.open.state, LiveOpenStateV1::Open);

    let course_detail: CourseDetailResponseV1 = post(
        &fixture.routes,
        PRODUCT_COURSE_DETAIL_PATH,
        CourseDetailRequestV1::new(course_key.clone()),
    );
    assert_eq!(course_detail.course.group.key, course_key);
    let section_detail: SectionDetailResponseV1 = post(
        &fixture.routes,
        PRODUCT_SECTION_DETAIL_PATH,
        SectionDetailRequestV1::new(fixture.section.clone()),
    );
    assert_eq!(section_detail.section.section.key, fixture.section);
    assert_eq!(section_detail.section.open.state, LiveOpenStateV1::Open);

    let open: OpenRefreshStatusV1 = post(
        &fixture.routes,
        PRODUCT_OPEN_STATUS_PATH,
        OpenStatusRequestV1::new(OpenBatchKey::from(fixture.target.clone())),
    );
    assert_eq!(open.scheduler.lane, OpenSchedulerLane::ActiveWatch);
    assert_eq!(open.scheduler.active_watch_count, 2);
    assert!(open.last_valid_observation.is_some());

    let open_section: OpenSectionStatusV1 = post(
        &fixture.routes,
        PRODUCT_OPEN_SECTION_STATUS_PATH,
        OpenSectionStatusRequestV1::new(fixture.section.clone()),
    );
    assert_eq!(open_section.section_key, fixture.section);
    assert_eq!(open_section.state, OpenState::Open);
}

#[test]
fn batch_status_projects_once_and_keeps_valid_sections_when_one_saved_key_is_missing() {
    let fixture = fixture();
    let missing = SectionKey::try_new(
        fixture.target.term().as_str(),
        fixture.target.campus().as_str(),
        "99999",
    )
    .expect("valid stale Section identity");

    let status: OpenBatchStatusV1 = post(
        &fixture.routes,
        PRODUCT_OPEN_BATCH_STATUS_PATH,
        OpenBatchStatusRequestV1::new(
            OpenBatchKey::from(fixture.target.clone()),
            vec![fixture.section.clone(), missing],
        ),
    );

    assert_eq!(status.refresh.batch.target(), fixture.target);
    assert_eq!(status.sections.len(), 1);
    assert_eq!(status.sections[0].section_key, fixture.section);
    assert_eq!(status.sections[0].state, OpenState::Open);
}

#[test]
fn service_status_v2_reports_the_window_and_complete_target_readiness() {
    let fixture = fixture();
    let target = fixture.target.clone();
    let demand = TargetRefreshDemand::default();
    let routes = fixture.routes.with_target_refresh_demand(demand.clone());

    let status: ServiceStatusV2 = get(&routes, PRODUCT_SERVICE_STATUS_PATH);

    assert_eq!(status.contract_version, 2);
    assert_eq!(status.term_window.current_term.as_str(), "72026");
    assert_eq!(status.term_window.next_term.as_str(), "92026");
    assert_eq!(status.term_window.visible_terms.len(), 2);
    assert_eq!(
        status.term_window.visible_terms[0].publication,
        ServiceTermPublicationV2::Published,
    );
    assert_eq!(
        status.term_window.visible_terms[1].publication,
        ServiceTermPublicationV2::Unpublished,
    );
    assert_eq!(status.targets.len(), 6);
    assert_eq!(status.level, ServiceLevelV1::PartiallyReady);
    assert!(status.automatic_term_summaries.iter().all(|summary| {
        summary.total_target_count == 3
            && summary.ready_target_count
                == u64::from(summary.term.as_str() == fixture.target.term().as_str())
    }));
    assert!(
        status
            .targets
            .iter()
            .all(|target| { matches!(target.target.campus().as_str(), "NB" | "NK" | "CM") })
    );
    let ready = status
        .targets
        .iter()
        .find(|state| state.target == target)
        .expect("ready current target");
    assert_eq!(
        ready.snapshot_availability,
        ServiceSnapshotAvailabilityV2::Ready
    );
    assert!(ready.usable);
    assert!(demand.snapshot().expect("demand snapshot").is_empty());
}

#[test]
fn stale_publication_evidence_does_not_remove_ready_target_usability() {
    let fixture = fixture();
    let stale_now = OffsetDateTime::from_unix_timestamp(WINDOW_NOW_UNIX + 6 * 60 * 60 + 1)
        .expect("stale discovery timestamp");
    let policy = RefreshPolicy::try_new(Duration::from_secs(600), GeneralOpenInterval::public())
        .expect("fixed public refresh policy");
    let runtime = SharedRuntimeContext::new(
        OpenCounterAudience::Public,
        FixedClock(stale_now),
        FixedRefreshPolicyProvider::new(policy),
    );
    let routes = SharedProductRoutes::new(
        fixture.storage.clone(),
        runtime,
        Arc::new(OpenRuntimeSnapshotRegistry::default()),
    )
    .with_service_status(Arc::new(ServiceStatusRegistry::new(
        ServiceRuntimeV1::Public,
    )));

    let status: ServiceStatusV2 = get(&routes, PRODUCT_SERVICE_STATUS_PATH);
    let current = status
        .term_window
        .visible_terms
        .iter()
        .find(|term| &term.term == fixture.target.term())
        .expect("current visible term");
    assert_eq!(current.publication, ServiceTermPublicationV2::Unknown);
    let ready = status
        .targets
        .iter()
        .find(|target| target.target == fixture.target)
        .expect("retained ready target");
    assert_eq!(
        ready.snapshot_availability,
        ServiceSnapshotAvailabilityV2::Ready,
    );
    assert!(ready.usable);
}

#[test]
fn local_service_status_v2_is_a_five_term_by_three_campus_matrix() {
    let fixture = fixture();
    let routes = fixture
        .routes
        .with_service_status(Arc::new(ServiceStatusRegistry::new(
            ServiceRuntimeV1::Local,
        )));

    let status: ServiceStatusV2 = get(&routes, PRODUCT_SERVICE_STATUS_PATH);

    assert_eq!(status.term_window.visible_terms.len(), 5);
    assert_eq!(
        status
            .term_window
            .visible_terms
            .iter()
            .map(|term| (term.term.as_str(), term.relative_offset))
            .collect::<Vec<_>>(),
        vec![
            ("02026", -2),
            ("12026", -1),
            ("72026", 0),
            ("92026", 1),
            ("02027", 2),
        ]
    );
    assert_eq!(status.targets.len(), 15);
    for term in &status.term_window.visible_terms {
        assert_eq!(
            status
                .targets
                .iter()
                .filter(|target| target.target.term() == &term.term)
                .map(|target| target.target.campus().as_str())
                .collect::<Vec<_>>(),
            vec!["CM", "NB", "NK"],
        );
    }
}

#[test]
fn service_status_v2_preserves_concurrent_workflow_identity_for_one_target() {
    let fixture = fixture();
    let catalog_operation = WorkflowOperationId {
        target: fixture.target.clone(),
        kind: TargetWorkflowKind::CompleteSnapshot,
    };
    let serving_open_operation = WorkflowOperationId {
        target: fixture.target.clone(),
        kind: TargetWorkflowKind::OpenRefresh,
    };
    let third_operation = WorkflowOperationId {
        target: TermCampusKey::try_new("92026", "NK").expect("third target"),
        kind: TargetWorkflowKind::CompleteSnapshot,
    };
    for (id, stage) in [
        (
            catalog_operation.clone(),
            ServiceOperationStageV2::CatalogProcess,
        ),
        (
            serving_open_operation.clone(),
            ServiceOperationStageV2::OpenFetch,
        ),
        (third_operation, ServiceOperationStageV2::CatalogFetch),
    ] {
        fixture
            .status
            .publish_workflow_operation(WorkflowOperationActivity {
                id,
                stage,
                started_at: OffsetDateTime::from_unix_timestamp(WINDOW_NOW_UNIX)
                    .expect("operation timestamp"),
            });
    }

    let concurrent: ServiceStatusV2 = get(&fixture.routes, PRODUCT_SERVICE_STATUS_PATH);
    assert_eq!(concurrent.operations.len(), REFRESH_MAX_CONCURRENCY);
    assert!(concurrent.operations.len() <= REFRESH_MAX_CONCURRENCY);
    assert_eq!(
        concurrent
            .operations
            .iter()
            .filter(|operation| operation.target == fixture.target)
            .count(),
        2
    );
    let target = concurrent
        .targets
        .iter()
        .find(|target| target.target == fixture.target)
        .expect("aggregated target");
    assert_eq!(target.work_state, ServiceWorkStateV2::Running);
    assert_eq!(target.stage, Some(ServiceOperationStageV2::CatalogProcess));

    fixture
        .status
        .clear_workflow_operation(&serving_open_operation);
    let after_open: ServiceStatusV2 = get(&fixture.routes, PRODUCT_SERVICE_STATUS_PATH);
    assert_eq!(after_open.operations.len(), 2);
    assert!(after_open.operations.iter().any(|operation| {
        operation.target == catalog_operation.target
            && operation.stage == ServiceOperationStageV2::CatalogProcess
    }));
    assert_eq!(
        after_open
            .operations
            .iter()
            .filter(|operation| operation.target == fixture.target)
            .count(),
        1
    );
}

#[test]
fn product_routes_read_dynamic_policy_before_locking_the_shared_database() {
    let fixture = fixture();
    let policy = RefreshPolicy::try_new(Duration::from_secs(600), GeneralOpenInterval::public())
        .expect("fixed public refresh policy");
    let runtime = SharedRuntimeContext::new(
        OpenCounterAudience::Public,
        FixedClock(
            OffsetDateTime::from_unix_timestamp(WINDOW_NOW_UNIX).expect("2026-07-17 timestamp"),
        ),
        StorageLockSensitivePolicy {
            storage: fixture.storage.clone(),
            policy,
        },
    );
    let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
    open_runtime
        .replace(
            fixture.target.clone(),
            OpenRuntimeSnapshot {
                lane: OpenSchedulerLane::ActiveWatch,
                active_watch_count: 1,
                ..OpenRuntimeSnapshot::default()
            },
        )
        .expect("register target runtime");
    let routes = SharedProductRoutes::new(fixture.storage.clone(), runtime, open_runtime)
        .with_service_status(Arc::new(ServiceStatusRegistry::new(
            ServiceRuntimeV1::Public,
        )));

    let status: ServiceStatusV2 = get(&routes, PRODUCT_SERVICE_STATUS_PATH);
    assert!(
        status
            .targets
            .iter()
            .any(|target| target.target == fixture.target && target.usable)
    );

    let courses: CourseQueryResponseV1 = post(
        &routes,
        PRODUCT_COURSE_SEARCH_PATH,
        CourseQueryRequestV1 {
            filters: search_filters(),
            page: PageRequestV1::default(),
            sort: CourseSortV1::default(),
        },
    );
    assert_eq!(courses.page.total, 1);

    let open: OpenRefreshStatusV1 = post(
        &routes,
        PRODUCT_OPEN_STATUS_PATH,
        OpenStatusRequestV1::new(OpenBatchKey::from(fixture.target)),
    );
    assert!(open.last_valid_observation.is_some());
}

#[test]
fn discovery_subject_dictionary_follows_the_current_catalog_publication() {
    let fixture = fixture();
    let first: CatalogDiscoveryResponseV1 = post(
        &fixture.routes,
        PRODUCT_CATALOG_DISCOVERY_PATH,
        CatalogDiscoveryRequestV1::new(),
    );
    assert_eq!(first.subjects.len(), 1);
    assert_eq!(first.subjects[0].code.as_str(), "198");
    assert_eq!(
        first.subjects[0].label,
        CatalogFieldKnowledge::present("Computer Science".to_owned())
    );
    let CatalogSubjectProvenanceV1::Catalog {
        content_version,
        catalog,
    } = &first.subjects[0].provenance
    else {
        panic!("an empty selector dictionary must be supplemented by Catalog evidence");
    };
    assert_eq!(content_version.get(), 1);
    assert_eq!(catalog.observation_id, trace(2));
    assert_eq!(catalog.target, fixture.target);

    let replacement_version = publish_catalog_subject(
        &mut fixture.storage.lock().expect("operational storage lock"),
        &fixture.target,
        "640",
        "Statistics",
        5,
        "2026-07-18T00:00:00Z",
        "2026-07-18T00:00:01Z",
    );
    assert_eq!(replacement_version, 2);
    publish_open_with_suffix(
        &mut fixture.storage.lock().expect("operational storage lock"),
        &fixture.target,
        &fixture.section,
        replacement_version,
        8,
        9,
    );
    fixture
        .routes
        .rebuild_prepared_serving_snapshot()
        .expect("publish replacement prepared generation");

    let replacement: CatalogDiscoveryResponseV1 = post(
        &fixture.routes,
        PRODUCT_CATALOG_DISCOVERY_PATH,
        CatalogDiscoveryRequestV1::new(),
    );
    assert_eq!(replacement.subjects.len(), 1);
    assert_eq!(replacement.subjects[0].code.as_str(), "640");
    assert_eq!(
        replacement.subjects[0].label,
        CatalogFieldKnowledge::present("Statistics".to_owned())
    );
    let CatalogSubjectProvenanceV1::Catalog {
        content_version,
        catalog,
    } = &replacement.subjects[0].provenance
    else {
        panic!("replacement dictionary entry must retain Catalog provenance");
    };
    assert_eq!(content_version.get(), 2);
    assert_eq!(catalog.observation_id, trace(5));
    assert_eq!(catalog.target, fixture.target);
    assert!(
        replacement
            .subjects
            .iter()
            .all(|subject| subject.code.as_str() != "198")
    );
}

#[test]
fn discovery_exposes_dictionaries_for_each_ready_target_without_a_global_term_gate() {
    let directory = TempDir::new().expect("temporary directory");
    let database = directory.path().join("subject-hydration.sqlite");
    let mut storage = OperationalStorage::open(database).expect("operational SQLite");
    publish_discovery_campuses(&mut storage, &[("NB", "New Brunswick"), ("NK", "Newark")]);
    let first_target = TermCampusKey::try_new("72026", "NB").expect("first target");
    let second_target = TermCampusKey::try_new("72026", "NK").expect("second target");
    let first_section = SectionKey::try_new("72026", "NB", "10001").expect("first section");
    let second_section = SectionKey::try_new("72026", "NK", "10001").expect("second section");
    let first_version = publish_catalog_subject(
        &mut storage,
        &first_target,
        "198",
        "Computer Science",
        6,
        STARTED,
        COMPLETED,
    );
    publish_open_with_suffix(
        &mut storage,
        &first_target,
        &first_section,
        first_version,
        8,
        9,
    );

    let now = OffsetDateTime::from_unix_timestamp(WINDOW_NOW_UNIX).expect("2026-07-17 timestamp");
    let policy = RefreshPolicy::try_new(Duration::from_secs(600), GeneralOpenInterval::public())
        .expect("fixed public refresh policy");
    let runtime = SharedRuntimeContext::new(
        OpenCounterAudience::Public,
        FixedClock(now),
        FixedRefreshPolicyProvider::new(policy),
    );
    let routes = SharedProductRoutes::new(
        Arc::new(Mutex::new(storage)),
        runtime,
        Arc::new(OpenRuntimeSnapshotRegistry::default()),
    )
    .with_service_status(Arc::new(ServiceStatusRegistry::new(
        ServiceRuntimeV1::Public,
    )));

    let partial: CatalogDiscoveryResponseV1 = post(
        &routes,
        PRODUCT_CATALOG_DISCOVERY_PATH,
        CatalogDiscoveryRequestV1::new(),
    );
    assert_eq!(partial.subjects.len(), 1);
    assert_eq!(partial.subjects[0].target, first_target);
    assert_eq!(partial.subjects[0].code.as_str(), "198");
    assert_eq!(partial.core_code_dictionaries.len(), 1);
    assert_eq!(partial.core_code_dictionaries[0].target, first_target);
    assert_eq!(partial.core_code_dictionaries[0].options[0].code, "CCO");

    let second_version = publish_catalog_subject(
        &mut routes.storage().lock().expect("operational storage lock"),
        &second_target,
        "640",
        "Statistics",
        7,
        "2026-07-18T00:00:00Z",
        "2026-07-18T00:00:01Z",
    );
    publish_open_with_suffix(
        &mut routes.storage().lock().expect("operational storage lock"),
        &second_target,
        &second_section,
        second_version,
        10,
        11,
    );
    routes
        .rebuild_prepared_serving_snapshot()
        .expect("publish second-target prepared generation");
    let complete: CatalogDiscoveryResponseV1 = post(
        &routes,
        PRODUCT_CATALOG_DISCOVERY_PATH,
        CatalogDiscoveryRequestV1::new(),
    );
    assert_eq!(complete.subjects.len(), 2);
    assert!(
        complete
            .subjects
            .iter()
            .any(|subject| { subject.target == first_target && subject.code.as_str() == "198" })
    );
    assert!(
        complete
            .subjects
            .iter()
            .any(|subject| { subject.target == second_target && subject.code.as_str() == "640" })
    );
}

#[test]
fn product_routes_require_typed_http_envelopes_and_do_not_guess_unlisted_routes() {
    let fixture = fixture();
    let bare_request = CourseQueryRequestV1 {
        filters: search_filters(),
        page: PageRequestV1::default(),
        sort: CourseSortV1::default(),
    };
    let response = fixture.routes.handle(ExtensionRequest::new(
        RequestMethod::Post,
        PRODUCT_COURSE_SEARCH_PATH,
        None,
        serde_json::to_vec(&bare_request).expect("bare request"),
    ));
    assert_eq!(response.status(), 400);
    let error: ApiErrorEnvelope = serde_json::from_slice(response.body()).expect("error envelope");
    assert_eq!(error.error().code(), ApiErrorCode::MalformedRequest);

    let response = fixture.routes.handle(ExtensionRequest::new(
        RequestMethod::Post,
        "/api/v1/query/unlisted",
        None,
        Vec::new(),
    ));
    assert_eq!(response.status(), 404);
}

#[test]
fn selected_discovered_target_without_a_complete_snapshot_returns_target_details() {
    let fixture = fixture();
    publish_discovery_campuses_with_suffix(
        &mut fixture.storage.lock().expect("storage lock"),
        &[("NB", "New Brunswick"), ("NK", "Newark")],
        9,
    );
    fixture
        .routes
        .rebuild_prepared_serving_snapshot()
        .expect("publish discovery-only prepared generation");
    let mut input =
        FilterValuesInputV1::for_term(TermId::try_from("72026").expect("synthetic term"));
    input.campuses = vec![CampusCode::try_from("NK").expect("synthetic campus")];
    let request = CourseQueryRequestV1 {
        filters: FilterRequestV1::new(
            bcsp_contracts::NormalizedFilterValuesV1::try_new(input).expect("filters"),
        ),
        page: PageRequestV1::default(),
        sort: CourseSortV1::default(),
    };
    let body = serde_json::to_vec(&HttpRequestEnvelope::new(request)).expect("request envelope");

    let response = fixture.routes.handle(ExtensionRequest::new(
        RequestMethod::Post,
        PRODUCT_COURSE_SEARCH_PATH,
        None,
        body,
    ));

    assert_eq!(response.status(), 503);
    assert!(String::from_utf8_lossy(response.body()).contains("\"kind\":\"TARGET_NOT_READY\""));
    let error: ApiErrorEnvelope = serde_json::from_slice(response.body()).expect("error envelope");
    assert_eq!(error.error().code(), ApiErrorCode::SearchDataNotReady);
    assert_eq!(
        error.error().details(),
        &[ApiErrorDetail::TargetNotReady {
            target: TermCampusKey::try_new("72026", "NK").expect("unready target")
        }]
    );
}

#[test]
fn exact_selected_target_gate_ignores_unselected_unready_targets() {
    let fixture = fixture();
    {
        let mut storage = fixture.storage.lock().expect("storage lock");
        publish_discovery_campuses_with_suffix(
            &mut storage,
            &[("NB", "New Brunswick"), ("NK", "Newark")],
            9,
        );
    }

    let mut input =
        FilterValuesInputV1::for_term(TermId::try_from("72026").expect("synthetic term"));
    input.campuses = vec![CampusCode::try_from("NB").expect("synthetic campus")];
    let filters = FilterRequestV1::new(
        bcsp_contracts::NormalizedFilterValuesV1::try_new(input).expect("filters"),
    );
    let course = CourseQueryRequestV1 {
        filters: filters.clone(),
        page: PageRequestV1::default(),
        sort: CourseSortV1::default(),
    };
    let section = SectionQueryRequestV1 {
        filters,
        page: PageRequestV1::default(),
        sort: SectionSortV1::default(),
    };
    let options = FilterOptionsRequestV2 {
        contract_version: QUERY_CONTRACT_VERSION,
        term: TermId::try_from("72026").unwrap(),
        campuses: vec![CampusCode::try_from("NB").unwrap()],
        field: FilterOptionsFieldV2::Instructor,
        query: Some("smi".to_owned()),
        limit: None,
    };

    let courses: CourseQueryResponseV1 = post(&fixture.routes, PRODUCT_COURSE_SEARCH_PATH, course);
    assert_eq!(courses.page.total, 1);
    let sections: SectionQueryResponseV1 =
        post(&fixture.routes, PRODUCT_SECTION_SEARCH_PATH, section);
    assert_eq!(sections.page.total, 1);
    let options: FilterOptionsResponseV2 =
        post(&fixture.routes, PRODUCT_FILTER_OPTIONS_PATH, options);
    assert_eq!(options.options[0].value, "Pat Smith");
}

#[test]
fn mixed_case_core_codes_validate_and_match_in_their_canonical_uppercase_form() {
    let directory = TempDir::new().expect("temporary directory");
    let database = directory.path().join("mixed-case-core.sqlite");
    let mut storage = OperationalStorage::open(database).expect("operational SQLite");
    publish_discovery(&mut storage);
    let target = TermCampusKey::try_new("72026", "NB").expect("synthetic target");
    let section = SectionKey::try_new("72026", "NB", "10001").expect("synthetic section");
    let content_version = publish_catalog_subject_with_core(
        &mut storage,
        &target,
        ("198", "Computer Science"),
        2,
        STARTED,
        COMPLETED,
        ("AHo", "Arts and Humanities (o)"),
    );
    publish_open(&mut storage, &target, &section, content_version);
    let now = OffsetDateTime::from_unix_timestamp(WINDOW_NOW_UNIX).expect("2026-07-17 timestamp");
    let policy = RefreshPolicy::try_new(Duration::from_secs(600), GeneralOpenInterval::public())
        .expect("fixed public refresh policy");
    let runtime = SharedRuntimeContext::new(
        OpenCounterAudience::Public,
        FixedClock(now),
        FixedRefreshPolicyProvider::new(policy),
    );
    let routes = SharedProductRoutes::new(
        Arc::new(Mutex::new(storage)),
        runtime,
        Arc::new(OpenRuntimeSnapshotRegistry::default()),
    )
    .with_service_status(Arc::new(ServiceStatusRegistry::new(
        ServiceRuntimeV1::Public,
    )));

    // Discovery keeps the feed's spelling: it is what the page displays.
    let discovery: CatalogDiscoveryResponseV1 = post(
        &routes,
        PRODUCT_CATALOG_DISCOVERY_PATH,
        CatalogDiscoveryRequestV1::new(),
    );
    assert_eq!(discovery.core_code_dictionaries.len(), 1);
    assert_eq!(discovery.core_code_dictionaries[0].options[0].code, "AHo");

    let filters_for = |code: &str| {
        let mut input =
            FilterValuesInputV1::for_term(TermId::try_from("72026").expect("synthetic term"));
        input.campuses = vec![CampusCode::try_from("NB").expect("NB")];
        input.core.codes = vec![FilterTokenV1::try_from(code).expect("core token")];
        FilterRequestV1::new(
            bcsp_contracts::NormalizedFilterValuesV1::try_new(input).expect("normalized filters"),
        )
    };
    // Whatever case the page sends, the canonical request is uppercase and
    // must validate and match against the mixed-case stored code.
    for requested in ["AHo", "AHO", "aho"] {
        let filters = filters_for(requested);
        let validation: DynamicFilterValidationResponseV3 = post(
            &routes,
            PRODUCT_DYNAMIC_FILTER_VALIDATION_PATH,
            DynamicFilterValidationRequestV3::new(filters.clone()),
        );
        assert!(
            validation.invalid_values.is_empty(),
            "{requested}: {:?}",
            validation.invalid_values
        );
        let courses: CourseQueryResponseV1 = post(
            &routes,
            PRODUCT_COURSE_SEARCH_PATH,
            CourseQueryRequestV1 {
                filters: filters.clone(),
                page: PageRequestV1::default(),
                sort: CourseSortV1::default(),
            },
        );
        assert_eq!(courses.page.total, 1, "{requested}");
        let sections: SectionQueryResponseV1 = post(
            &routes,
            PRODUCT_SECTION_SEARCH_PATH,
            SectionQueryRequestV1 {
                filters,
                page: PageRequestV1::default(),
                sort: SectionSortV1::default(),
            },
        );
        assert_eq!(sections.page.total, 1, "{requested}");
    }

    // A code the target does not carry stays invalid in every case.
    let validation: DynamicFilterValidationResponseV3 = post(
        &routes,
        PRODUCT_DYNAMIC_FILTER_VALIDATION_PATH,
        DynamicFilterValidationRequestV3::new(filters_for("AHp")),
    );
    assert_eq!(
        validation
            .invalid_values
            .iter()
            .map(|invalid| (invalid.field, invalid.value.as_str()))
            .collect::<Vec<_>>(),
        vec![(FilterFieldId::CourseCoreCode, "AHP")]
    );
}

#[test]
fn unknown_target_scoped_core_code_returns_the_existing_invalid_option_error() {
    let fixture = fixture();
    let mut input =
        FilterValuesInputV1::for_term(TermId::try_from("72026").expect("synthetic term"));
    input.core.codes = vec![FilterTokenV1::try_from("NOT_A_REAL_CORE").expect("core token")];
    let request = CourseQueryRequestV1 {
        filters: FilterRequestV1::new(
            bcsp_contracts::NormalizedFilterValuesV1::try_new(input).expect("filters"),
        ),
        page: PageRequestV1::default(),
        sort: CourseSortV1::default(),
    };
    let body = serde_json::to_vec(&HttpRequestEnvelope::new(request)).expect("request envelope");

    let response = fixture.routes.handle(ExtensionRequest::new(
        RequestMethod::Post,
        PRODUCT_COURSE_SEARCH_PATH,
        None,
        body,
    ));

    assert_eq!(response.status(), 400);
    let error: ApiErrorEnvelope = serde_json::from_slice(response.body()).expect("error envelope");
    assert_eq!(error.error().code(), ApiErrorCode::InvalidFilterOption);
}

#[test]
fn scoped_service_status_renews_target_demand_without_an_apply_endpoint() {
    let fixture = fixture();
    let demand = TargetRefreshDemand::default();
    let routes = fixture.routes.with_target_refresh_demand(demand.clone());

    let response = routes.handle(ExtensionRequest::new(
        RequestMethod::Get,
        PRODUCT_SERVICE_STATUS_PATH,
        Some("activeTerm=72026&activeCampus=NB".to_owned()),
        Vec::new(),
    ));

    assert_eq!(response.status(), 200);
    let status = serde_json::from_slice::<HttpSuccessEnvelope<ServiceStatusV2>>(response.body())
        .expect("status envelope")
        .into_data();
    assert_eq!(status.term_window.current_term.as_str(), "72026");
    assert_eq!(
        demand.snapshot().expect("demand snapshot"),
        vec![TermCampusKey::try_new("72026", "NB").expect("scoped target")]
    );
}

#[test]
fn scoped_service_status_rejects_non_product_and_online_alias_campuses() {
    let fixture = fixture();
    for campus in ["NWK", "CAM", "ONLINE_NB"] {
        let response = fixture.routes.handle(ExtensionRequest::new(
            RequestMethod::Get,
            PRODUCT_SERVICE_STATUS_PATH,
            Some(format!("activeTerm=72026&activeCampus={campus}")),
            Vec::new(),
        ));
        assert_eq!(
            response.status(),
            400,
            "{campus} must not enter product scope"
        );
        let error: ApiErrorEnvelope =
            serde_json::from_slice(response.body()).expect("error envelope");
        assert_eq!(error.error().code(), ApiErrorCode::InvalidFilter);
    }
}

#[test]
fn ready_target_in_retry_wait_remains_queryable_from_its_last_complete_snapshot() {
    let fixture = fixture();
    let registry = Arc::new(ServiceStatusRegistry::new(ServiceRuntimeV1::Public));
    registry.publish_target_activity(TargetWorkActivity {
        target: fixture.target.clone(),
        work_state: ServiceWorkStateV2::RetryWait,
        stage: Some(ServiceOperationStageV2::OpenFetch),
        started_at: None,
        next_retry_at: Some(
            OffsetDateTime::from_unix_timestamp(WINDOW_NOW_UNIX + 30).expect("retry timestamp"),
        ),
        error: Some(ServiceTargetErrorV2 {
            code: "OPEN_UPSTREAM_TRANSPORT".to_owned(),
            http_status: Some(503),
            content_type: Some("application/json".to_owned()),
            content_encoding: None,
            decoded_bytes: None,
            error_class: Some("TRANSIENT".to_owned()),
            error_chain: Some("upstream unavailable".to_owned()),
            trace_id: Some(trace(12)),
        }),
    });
    let routes = fixture.routes.with_service_status(registry);

    let status: ServiceStatusV2 = get(&routes, PRODUCT_SERVICE_STATUS_PATH);
    let target = status
        .targets
        .iter()
        .find(|target| target.target == fixture.target)
        .expect("target status");
    assert_eq!(
        target.snapshot_availability,
        ServiceSnapshotAvailabilityV2::Ready
    );
    assert_eq!(target.work_state, ServiceWorkStateV2::RetryWait);
    assert!(target.usable);
    assert!(target.error.is_some());

    let courses: CourseQueryResponseV1 = post(
        &routes,
        PRODUCT_COURSE_SEARCH_PATH,
        CourseQueryRequestV1 {
            filters: search_filters(),
            page: PageRequestV1::default(),
            sort: CourseSortV1::default(),
        },
    );
    assert_eq!(courses.page.total, 1);
}

#[test]
fn online_aliases_are_excluded_from_discovery_status_and_implicit_search_scope() {
    let fixture = fixture();
    publish_discovery_campuses_with_suffix(
        &mut fixture.storage.lock().expect("storage lock"),
        &[
            ("NB", "New Brunswick"),
            ("ONLINE_NB", "New Brunswick Online"),
            ("ONLINE_NK", "Newark Online"),
            ("ONLINE_CM", "Camden Online"),
        ],
        13,
    );

    let discovery: CatalogDiscoveryResponseV1 = post(
        &fixture.routes,
        PRODUCT_CATALOG_DISCOVERY_PATH,
        CatalogDiscoveryRequestV1::new(),
    );
    assert_eq!(discovery.targets.len(), 1);
    assert_eq!(discovery.targets[0].key, fixture.target);
    let status: ServiceStatusV2 = get(&fixture.routes, PRODUCT_SERVICE_STATUS_PATH);
    assert_eq!(status.targets.len(), 6);
    assert!(
        status
            .targets
            .iter()
            .all(|target| { matches!(target.target.campus().as_str(), "NB" | "NK" | "CM") })
    );
    assert!(
        status
            .targets
            .iter()
            .any(|target| target.target == fixture.target)
    );

    let courses: CourseQueryResponseV1 = post(
        &fixture.routes,
        PRODUCT_COURSE_SEARCH_PATH,
        CourseQueryRequestV1 {
            filters: search_filters(),
            page: PageRequestV1::default(),
            sort: CourseSortV1::default(),
        },
    );
    assert_eq!(courses.page.total, 1);
}

#[test]
fn open_runtime_registry_is_target_scoped_and_missing_targets_are_neutral() {
    let registry = OpenRuntimeSnapshotRegistry::default();
    let first = TermCampusKey::try_new("72026", "NB").expect("first target");
    let second = TermCampusKey::try_new("72026", "NWK").expect("second target");
    registry
        .replace(
            first.clone(),
            OpenRuntimeSnapshot {
                active_watch_count: 7,
                ..OpenRuntimeSnapshot::default()
            },
        )
        .expect("replace snapshot");
    assert_eq!(registry.snapshot(&first).unwrap().active_watch_count, 7);
    assert_eq!(
        registry.snapshot(&second).unwrap(),
        OpenRuntimeSnapshot::default()
    );
    assert_eq!(
        registry.remove(&first).unwrap().unwrap().active_watch_count,
        7
    );
    assert_eq!(
        registry.snapshot(&first).unwrap(),
        OpenRuntimeSnapshot::default()
    );
}
