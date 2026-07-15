use std::sync::{Arc, Mutex};
use std::time::Duration;

use bcsp_application::{
    ApplicationClock, ExtensionRequest, FixedRefreshPolicyProvider, OpenRuntimeSnapshot,
    OpenRuntimeSnapshotRegistry, PRODUCT_CATALOG_DISCOVERY_PATH, PRODUCT_COURSE_DETAIL_PATH,
    PRODUCT_COURSE_SEARCH_PATH, PRODUCT_FILTER_SCHEMA_PATH, PRODUCT_OPEN_SECTION_STATUS_PATH,
    PRODUCT_OPEN_STATUS_PATH, PRODUCT_SECTION_DETAIL_PATH, PRODUCT_SECTION_SEARCH_PATH,
    RefreshPolicy, RefreshPolicyProvider, RefreshPolicyReadError, RequestMethod, RouteExtension,
    SHARED_PRODUCT_ROUTE_INVENTORY, SharedProductRoutes, SharedRuntimeContext, TargetRefreshDemand,
};
use bcsp_catalog::{normalize_target, to_catalog_refresh_command, to_discovery_refresh_command};
use bcsp_contracts::{
    ApiErrorCode, ApiErrorEnvelope, CatalogDiscoveryRequestV1, CatalogDiscoveryResponseV1,
    CatalogFieldKnowledge, CatalogSubjectProvenanceV1, CourseDetailRequestV1,
    CourseDetailResponseV1, CourseQueryRequestV1, CourseQueryResponseV1, CourseSortV1,
    FilterRequestV1, FilterSchemaV1, FilterSearchTextV1, FilterValuesInputV1, HttpRequestEnvelope,
    HttpSuccessEnvelope, LiveOpenStateV1, OpenBatchKey, OpenRefreshStatusV1, OpenSchedulerLane,
    OpenSectionStatusRequestV1, OpenSectionStatusV1, OpenState, OpenStatusRequestV1, PageRequestV1,
    SectionDetailRequestV1, SectionDetailResponseV1, SectionKey, SectionQueryRequestV1,
    SectionQueryResponseV1, SectionSortV1, TermCampusKey, TermId, TraceId,
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

const STARTED: &str = "2030-01-01T00:00:00Z";
const COMPLETED: &str = "2030-01-01T00:00:01Z";
const BODY_SHA256: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

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
    target: TermCampusKey,
    section: SectionKey,
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
    let body = serde_json::to_vec(&serde_json::json!({
      "sourceVersion": "synthetic-v1",
      "terms": [
        {
          "termId": "92026",
          "year": 2026,
          "termCode": "9",
          "display": "Fall 2026",
          "published": true
        },
        {
          "termId": "12027",
          "year": 2027,
          "termCode": "1",
          "display": "Spring 2027",
          "published": false
        }
      ],
      "campuses": campuses.iter().map(|(code, display)| serde_json::json!({
        "campusCode": code,
        "display": display,
        "enabled": true
      })).collect::<Vec<_>>(),
      "targets": campuses.iter().map(|(code, _)| serde_json::json!({
        "termId": "92026",
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
            to_discovery_refresh_command(&snapshot, trace(1), STARTED, COMPLETED)
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
    let body = serde_json::to_vec(&serde_json::json!([{
        "campusCode": target.campus().as_str(),
        "courseString": format!("01:{subject}:111"),
        "subject": subject,
        "subjectDescription": subject_description,
        "courseNumber": "111",
        "title": "Introduction to a Synthetic Subject",
        "sections": [{
            "campusCode": target.campus().as_str(),
            "index": "10001",
            "number": "01",
            "sectionCourseType": "LECTURE",
            "openStatus": false,
            "meetingTimes": []
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
    storage
        .begin_open_pull_attempt(&BeginOpenPullAttemptCommand {
            attempt_id: trace(3),
            run_id: trace(4),
            target: target.clone(),
            captured_catalog_content_version: catalog_content_version,
            rutgers_day: "2029-12-31".to_owned(),
            started_at: STARTED.to_owned(),
            lane: OpenRequestLane::ActiveWatch,
            requested_interval_seconds: Some(30),
            effective_interval_seconds: Some(10),
            schedule_lag_ms: Some(25),
        })
        .expect("begin synthetic Open pull");
    storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
            attempt_id: trace(3),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section.clone()],
            source_value_count: 1,
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
    let target = TermCampusKey::try_new("92026", "NB").expect("synthetic target");
    let section = SectionKey::try_new("92026", "NB", "10001").expect("synthetic section");
    publish_discovery(&mut storage);
    let content_version = publish_catalog(&mut storage, &target);
    publish_open(&mut storage, &target, &section, content_version);

    let now = OffsetDateTime::from_unix_timestamp(1_893_456_005).expect("2030 timestamp");
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
    let routes = SharedProductRoutes::new(storage.clone(), runtime, open_runtime);
    Fixture {
        _directory: directory,
        routes,
        storage,
        target,
        section,
    }
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
        FilterValuesInputV1::for_term(TermId::try_from("92026").expect("synthetic term"));
    input.text =
        Some(FilterSearchTextV1::try_from("Computer Science").expect("synthetic search text"));
    FilterRequestV1::new(
        bcsp_contracts::NormalizedFilterValuesV1::try_new(input).expect("normalized filters"),
    )
}

#[test]
fn shared_inventory_serves_real_sqlite_catalog_query_detail_and_open_projections() {
    let fixture = fixture();
    assert_eq!(SHARED_PRODUCT_ROUTE_INVENTORY.len(), 8);
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
fn product_routes_read_dynamic_policy_before_locking_the_shared_database() {
    let fixture = fixture();
    let policy = RefreshPolicy::try_new(Duration::from_secs(600), GeneralOpenInterval::public())
        .expect("fixed public refresh policy");
    let runtime = SharedRuntimeContext::new(
        OpenCounterAudience::Public,
        FixedClock(OffsetDateTime::from_unix_timestamp(1_893_456_005).expect("2030 timestamp")),
        StorageLockSensitivePolicy {
            storage: fixture.storage.clone(),
            policy,
        },
    );
    let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
    let routes = SharedProductRoutes::new(fixture.storage.clone(), runtime, open_runtime);

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
        "2030-01-02T00:00:00Z",
        "2030-01-02T00:00:01Z",
    );
    assert_eq!(replacement_version, 2);

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
fn discovery_never_exposes_a_partial_latest_term_catalog_subject_dictionary() {
    let directory = TempDir::new().expect("temporary directory");
    let database = directory.path().join("subject-hydration.sqlite");
    let mut storage = OperationalStorage::open(database).expect("operational SQLite");
    publish_discovery_campuses(&mut storage, &[("NB", "New Brunswick"), ("NWK", "Newark")]);
    let first_target = TermCampusKey::try_new("92026", "NB").expect("first target");
    let second_target = TermCampusKey::try_new("92026", "NWK").expect("second target");
    publish_catalog_subject(
        &mut storage,
        &first_target,
        "198",
        "Computer Science",
        6,
        STARTED,
        COMPLETED,
    );

    let now = OffsetDateTime::from_unix_timestamp(1_893_456_005).expect("2030 timestamp");
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
    );

    let partial: CatalogDiscoveryResponseV1 = post(
        &routes,
        PRODUCT_CATALOG_DISCOVERY_PATH,
        CatalogDiscoveryRequestV1::new(),
    );
    assert!(partial.subjects.is_empty());

    publish_catalog_subject(
        &mut routes.storage().lock().expect("operational storage lock"),
        &second_target,
        "640",
        "Statistics",
        7,
        "2030-01-02T00:00:00Z",
        "2030-01-02T00:00:01Z",
    );
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
fn parsed_product_target_signals_refresh_demand_before_projection() {
    let fixture = fixture();
    let demand = TargetRefreshDemand::default();
    let routes = fixture.routes.with_target_refresh_demand(demand.clone());
    let historical = TermCampusKey::try_new("92025", "NB").expect("historical target");
    let body = serde_json::to_vec(&HttpRequestEnvelope::new(OpenStatusRequestV1::new(
        OpenBatchKey::from(historical.clone()),
    )))
    .expect("request envelope");

    let response = routes.handle(ExtensionRequest::new(
        RequestMethod::Post,
        PRODUCT_OPEN_STATUS_PATH,
        None,
        body,
    ));

    assert_eq!(response.status(), 503);
    assert_eq!(
        demand.snapshot().expect("demand snapshot"),
        vec![historical]
    );
}

#[test]
fn open_runtime_registry_is_target_scoped_and_missing_targets_are_neutral() {
    let registry = OpenRuntimeSnapshotRegistry::default();
    let first = TermCampusKey::try_new("92026", "NB").expect("first target");
    let second = TermCampusKey::try_new("92026", "NWK").expect("second target");
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
