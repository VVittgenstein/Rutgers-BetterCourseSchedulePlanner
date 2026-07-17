use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bcsp_application::{
    ApplicationClock, ExtensionRequest, FixedRefreshPolicyProvider, OpenRuntimeSnapshotRegistry,
    PRODUCT_COURSE_DETAIL_PATH, PRODUCT_COURSE_SEARCH_PATH, PRODUCT_DYNAMIC_FILTER_VALIDATION_PATH,
    PRODUCT_FILTER_OPTIONS_PATH, PRODUCT_SECTION_DETAIL_PATH, PRODUCT_SERVICE_STATUS_PATH,
    RefreshPolicy, RequestMethod, RouteExtension, SharedProductRoutes, SharedRuntimeContext,
};
use bcsp_contracts::{
    CampusCode, CatalogFieldKnowledge, CatalogFieldPresence, CatalogSubjectCode,
    CourseDetailRequestV1, CourseQueryRequestV1, CourseQueryResponseV1, CourseSortV1,
    DynamicFilterValidationRequestV3, DynamicFilterValidationResponseV3, FilterOptionsFieldV2,
    FilterOptionsRequestV2, FilterRequestV1, FilterTokenV1, FilterValuesInputV1,
    HttpRequestEnvelope, HttpSuccessEnvelope, NormalizedFilterValuesV1, PageRequestV1,
    QUERY_CONTRACT_VERSION, SectionDetailRequestV1, ServiceStatusV2, ServiceTermPublicationV2,
    TermCampusKey, TermId,
};
use bcsp_open::{GeneralOpenInterval, OpenCounterAudience};
use bcsp_operational_storage::OperationalStorage;
use bcsp_rutgers_client::sha256_hex;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;
use tracing_subscriber::fmt::MakeWriter;

const FIXED_NOW_UNIX: i64 = 1_784_289_600;
const REFERENCE_TERM: &str = "92026";
const PRODUCT_CAMPUSES: [&str; 3] = ["NB", "NK", "CM"];
const UNOPTIMIZED_REFERENCE_ARTIFACT: &str = "v3-unoptimized-reference-final.json";
const UNOPTIMIZED_REFERENCE_SHA256: &str =
    "79e76b499675db5c09d62341ceb6719ec347d700a26bb6ca83a36d80b112e82b";
const OPTIMIZED_REFERENCE_ARTIFACT: &str = "v3-optimized-reference-final.json";
const OPTIMIZED_PROFILE_ARTIFACT: &str = "v3-optimized-profile-final.json";

const STATUS_P95_BUDGET_MS: f64 = 250.0;
const STATUS_MAX_BUDGET_MS: f64 = 1_000.0;
const FILTER_OPTIONS_P95_BUDGET_MS: f64 = 500.0;
const FILTER_OPTIONS_MAX_BUDGET_MS: f64 = 1_500.0;
const SINGLE_SEARCH_P95_BUDGET_MS: f64 = 1_000.0;
const SINGLE_SEARCH_MAX_BUDGET_MS: f64 = 2_000.0;
const COMPLEX_SEARCH_P95_BUDGET_MS: f64 = 2_000.0;
const COMPLEX_SEARCH_MAX_BUDGET_MS: f64 = 5_000.0;

#[derive(Clone, Copy)]
struct FixedClock(OffsetDateTime);

impl ApplicationClock for FixedClock {
    fn now(&self) -> OffsetDateTime {
        self.0
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DurationSummary {
    cold_ms: f64,
    warm_repetitions: usize,
    warm_p50_ms: f64,
    warm_p95_ms: f64,
    warm_max_ms: f64,
    warm_samples_ms: Vec<f64>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct TargetVectorEntry {
    term: String,
    campus: String,
    catalog_content_version: u64,
    open_observation_sequence: u64,
    open_catalog_content_version: u64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FunctionalWitness {
    response_sha256: String,
    response_bytes: usize,
    page: Value,
    ordered_course_keys: Vec<Value>,
    materialized_section_keys: Vec<Value>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct TermPublicationWitness {
    term: String,
    publication: ServiceTermPublicationV2,
    ready_target_count: usize,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct FixtureEvidence {
    timing: DurationSummary,
    witness: FunctionalWitness,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BudgetEvaluation {
    cohort: &'static str,
    warm_p95_budget_ms: f64,
    warm_max_budget_ms: f64,
    warm_p95_pass: bool,
    warm_max_pass: bool,
    pass: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OracleWitnessComparison {
    expected_response_sha256: String,
    actual_response_sha256: String,
    exact_witness: bool,
}

#[derive(Clone)]
struct TraceBuffer(Arc<Mutex<Vec<u8>>>);

struct TraceBufferWriter(Arc<Mutex<Vec<u8>>>);

impl Write for TraceBufferWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.0
            .lock()
            .expect("trace buffer lock")
            .extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl<'writer> MakeWriter<'writer> for TraceBuffer {
    type Writer = TraceBufferWriter;

    fn make_writer(&'writer self) -> Self::Writer {
        TraceBufferWriter(Arc::clone(&self.0))
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileEvent {
    phase: String,
    elapsed_us: u64,
    dimensions: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
enum PreparedMetricEvent {
    Access {
        hit: Option<bool>,
        hits: Option<u64>,
        misses: Option<u64>,
        publishes: Option<u64>,
        replacements: Option<u64>,
        evictions: Option<u64>,
        capacity: Option<u64>,
    },
    Build {
        elapsed_us: Option<u64>,
        elapsed_ms: Option<f64>,
        ready_targets: Option<u64>,
        catalog_json_bytes: Option<u64>,
        dictionary_estimated_bytes: Option<u64>,
        query_index_estimated_bytes: Option<u64>,
        open_index_estimated_bytes: Option<u64>,
        foundation_resident_estimated_bytes: Option<u64>,
        resident_estimated_bytes: Option<u64>,
        resident_budget_bytes: Option<u64>,
        resident_high_watermark: Option<u64>,
        fts_rows: Option<u64>,
        fts_document_bytes: Option<u64>,
        fts_index_bytes: Option<u64>,
        sqlite_cache_kib: Option<u64>,
        aggregate_resident_budget_bytes: Option<u64>,
        registry_capacity: Option<u64>,
    },
    Publish {
        elapsed_us: Option<u64>,
        publishes: Option<u64>,
        replacements: Option<u64>,
        evictions: Option<u64>,
        replaced: Option<bool>,
        capacity: Option<u64>,
        foundation_resident_estimated_bytes: Option<u64>,
        snapshot_resident_estimated_bytes: Option<u64>,
        fts_index_bytes: Option<u64>,
        sqlite_cache_budget_bytes: Option<u64>,
        aggregate_resident_budget_bytes: Option<u64>,
    },
    OpenRebuild {
        open_index_estimated_bytes: Option<u64>,
        resident_estimated_bytes: Option<u64>,
        replacement_peak_estimated_bytes: Option<u64>,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProfileRun {
    elapsed_ms: f64,
    response_sha256: String,
    response_bytes: usize,
    phase_totals_ms: BTreeMap<String, f64>,
    events: Vec<ProfileEvent>,
    non_duration_performance_phases: Vec<String>,
    prepared_metrics: Vec<PreparedMetricEvent>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DatabaseArtifact {
    artifact: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvironmentEvidence {
    build_identity: String,
    os: &'static str,
    architecture: &'static str,
    available_parallelism: usize,
    processor_identifier: Option<String>,
}

fn fixed_routes(
    path: &Path,
) -> (
    SharedProductRoutes<FixedClock, FixedRefreshPolicyProvider>,
    Arc<Mutex<OperationalStorage>>,
) {
    let storage = OperationalStorage::open(path).expect("open the fixed RC4 reference database");
    let storage = Arc::new(Mutex::new(storage));
    let now = OffsetDateTime::from_unix_timestamp(FIXED_NOW_UNIX).expect("fixed 2026-07-17 clock");
    let policy = RefreshPolicy::try_new(Duration::from_secs(600), GeneralOpenInterval::public())
        .expect("fixed performance policy");
    let runtime = SharedRuntimeContext::new(
        OpenCounterAudience::Public,
        FixedClock(now),
        FixedRefreshPolicyProvider::new(policy),
    );
    let routes = SharedProductRoutes::new(
        Arc::clone(&storage),
        runtime,
        Arc::new(OpenRuntimeSnapshotRegistry::default()),
    );
    (routes, storage)
}

fn fixed_routes_profiled(
    path: &Path,
) -> (
    SharedProductRoutes<FixedClock, FixedRefreshPolicyProvider>,
    Arc<Mutex<OperationalStorage>>,
    Vec<PreparedMetricEvent>,
) {
    let ((routes, storage), trace) = capture_tracing(|| fixed_routes(path));
    let parsed = parse_profile_trace(&trace);
    let build_count = parsed
        .prepared_metrics
        .iter()
        .filter(|metric| matches!(metric, PreparedMetricEvent::Build { .. }))
        .count();
    assert_eq!(
        build_count, 1,
        "optimized route construction must publish exactly one prepared build"
    );
    let Some(PreparedMetricEvent::Build {
        query_index_estimated_bytes,
        open_index_estimated_bytes,
        foundation_resident_estimated_bytes,
        ..
    }) = parsed
        .prepared_metrics
        .iter()
        .find(|metric| matches!(metric, PreparedMetricEvent::Build { .. }))
    else {
        unreachable!("build_count asserted one prepared build")
    };
    assert!(
        query_index_estimated_bytes.is_some()
            && open_index_estimated_bytes.is_some()
            && foundation_resident_estimated_bytes.is_some(),
        "final optimized build must record query/Open/foundation resident estimates"
    );
    let publish_metrics = parsed
        .prepared_metrics
        .iter()
        .filter_map(|metric| match metric {
            PreparedMetricEvent::Publish {
                snapshot_resident_estimated_bytes,
                ..
            } => Some(snapshot_resident_estimated_bytes),
            PreparedMetricEvent::Access { .. }
            | PreparedMetricEvent::Build { .. }
            | PreparedMetricEvent::OpenRebuild { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        publish_metrics.len(),
        1,
        "optimized route construction must publish exactly one prepared snapshot"
    );
    assert!(
        publish_metrics[0].is_some(),
        "final optimized publish must record snapshot resident estimate"
    );
    (routes, storage, parsed.prepared_metrics)
}

fn campuses(values: &[&str]) -> Vec<CampusCode> {
    values
        .iter()
        .map(|value| CampusCode::try_from(*value).expect("frozen Campus"))
        .collect()
}

fn targets(values: &[&str]) -> Vec<TermCampusKey> {
    values
        .iter()
        .map(|campus| TermCampusKey::try_new(REFERENCE_TERM, campus).expect("frozen target"))
        .collect()
}

fn request_body<T: Serialize>(payload: T) -> Vec<u8> {
    serde_json::to_vec(&HttpRequestEnvelope::new(payload)).expect("serialize request")
}

fn invoke(
    routes: &impl RouteExtension,
    method: RequestMethod,
    path: &'static str,
    body: &[u8],
) -> (Duration, Vec<u8>) {
    let started = Instant::now();
    let response = routes.handle(ExtensionRequest::new(method, path, None, body.to_vec()));
    let elapsed = started.elapsed();
    assert_eq!(
        response.status(),
        200,
        "{path}: {}",
        String::from_utf8_lossy(response.body())
    );
    (elapsed, response.body().to_vec())
}

fn invoke_profiled(
    routes: &impl RouteExtension,
    method: RequestMethod,
    path: &'static str,
    body: &[u8],
) -> (ProfileRun, Vec<u8>) {
    let ((elapsed, response), trace) = capture_tracing(|| invoke(routes, method, path, body));
    let parsed = parse_profile_trace(&trace);
    (
        ProfileRun {
            elapsed_ms: duration_ms(elapsed),
            response_sha256: sha256_hex(&response),
            response_bytes: response.len(),
            phase_totals_ms: parsed.phase_totals_ms,
            events: parsed.events,
            non_duration_performance_phases: parsed.non_duration_performance_phases,
            prepared_metrics: parsed.prepared_metrics,
        },
        response,
    )
}

fn capture_tracing<T>(operation: impl FnOnce() -> T) -> (T, Vec<u8>) {
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::fmt()
        .json()
        .with_ansi(false)
        .with_target(true)
        .with_current_span(false)
        .with_span_list(false)
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(TraceBuffer(Arc::clone(&bytes)))
        .finish();
    let result = tracing::subscriber::with_default(subscriber, operation);
    let trace = bytes.lock().expect("trace buffer lock").clone();
    (result, trace)
}

struct ParsedProfileTrace {
    phase_totals_ms: BTreeMap<String, f64>,
    events: Vec<ProfileEvent>,
    non_duration_performance_phases: Vec<String>,
    prepared_metrics: Vec<PreparedMetricEvent>,
}

fn parse_profile_trace(trace: &[u8]) -> ParsedProfileTrace {
    let mut events = Vec::new();
    let mut non_duration_performance_phases = Vec::new();
    let mut prepared_metrics = Vec::new();
    let mut phase_totals_us = BTreeMap::<String, u64>::new();
    for line in trace
        .split(|byte| *byte == b'\n')
        .filter(|line| !line.is_empty())
    {
        let event: Value = serde_json::from_slice(line).expect("JSON tracing event");
        let Some(fields) = event.get("fields") else {
            continue;
        };
        match event.get("target").and_then(Value::as_str) {
            Some("bcsp_performance") => {
                let Some(phase) = fields.get("phase").and_then(Value::as_str) else {
                    continue;
                };
                let Some(elapsed_us) = fields.get("elapsed_us").and_then(Value::as_u64) else {
                    if phase == "prepared_snapshot_bind" {
                        non_duration_performance_phases.push(phase.to_owned());
                    }
                    continue;
                };
                *phase_totals_us.entry(phase.to_owned()).or_default() += elapsed_us;
                events.push(ProfileEvent {
                    phase: phase.to_owned(),
                    elapsed_us,
                    dimensions: safe_performance_dimensions(fields),
                });
            }
            Some("bcsp_prepared_serving") => {
                if let Some(metric) = parse_prepared_metric(fields) {
                    prepared_metrics.push(metric);
                }
            }
            Some(_) | None => {}
        }
    }
    let phase_totals_ms = phase_totals_us
        .into_iter()
        .map(|(phase, micros)| (phase, micros as f64 / 1_000.0))
        .collect();
    ParsedProfileTrace {
        phase_totals_ms,
        events,
        non_duration_performance_phases,
        prepared_metrics,
    }
}

fn safe_performance_dimensions(fields: &Value) -> BTreeMap<String, Value> {
    const SAFE_FIELDS: [&str; 10] = [
        "route",
        "query_kind",
        "catalog_vector",
        "open_vector",
        "catalogs",
        "sections",
        "candidates",
        "page_items",
        "options",
        "response_bytes",
    ];
    SAFE_FIELDS
        .into_iter()
        .filter_map(|field| {
            fields
                .get(field)
                .filter(|value| value.is_string() || value.is_number() || value.is_boolean())
                .cloned()
                .map(|value| (field.to_owned(), value))
        })
        .collect()
}

fn parse_prepared_metric(fields: &Value) -> Option<PreparedMetricEvent> {
    let phase = fields.get("phase")?.as_str()?;
    let optional_u64 = |field: &'static str| fields.get(field).and_then(Value::as_u64);
    let optional_bool = |field: &'static str| fields.get(field).and_then(Value::as_bool);
    match phase {
        "prepared_snapshot_access" => Some(PreparedMetricEvent::Access {
            hit: optional_bool("hit"),
            hits: optional_u64("hits"),
            misses: optional_u64("misses"),
            publishes: optional_u64("publishes"),
            replacements: optional_u64("replacements"),
            evictions: optional_u64("evictions"),
            capacity: optional_u64("capacity"),
        }),
        "prepared_snapshot_build" => Some(PreparedMetricEvent::Build {
            elapsed_us: optional_u64("elapsed_us"),
            elapsed_ms: fields.get("elapsed_ms").and_then(Value::as_f64),
            ready_targets: optional_u64("ready_targets"),
            catalog_json_bytes: optional_u64("catalog_json_bytes"),
            dictionary_estimated_bytes: optional_u64("dictionary_estimated_bytes"),
            query_index_estimated_bytes: optional_u64("query_index_estimated_bytes"),
            open_index_estimated_bytes: optional_u64("open_index_estimated_bytes"),
            foundation_resident_estimated_bytes: optional_u64(
                "foundation_resident_estimated_bytes",
            ),
            resident_estimated_bytes: optional_u64("resident_estimated_bytes"),
            resident_budget_bytes: optional_u64("resident_budget_bytes"),
            resident_high_watermark: optional_u64("resident_high_watermark"),
            fts_rows: optional_u64("fts_rows"),
            fts_document_bytes: optional_u64("fts_document_bytes"),
            fts_index_bytes: optional_u64("fts_index_bytes"),
            sqlite_cache_kib: optional_u64("sqlite_cache_kib"),
            aggregate_resident_budget_bytes: optional_u64("aggregate_resident_budget_bytes"),
            registry_capacity: optional_u64("registry_capacity"),
        }),
        "prepared_snapshot_publish" => Some(PreparedMetricEvent::Publish {
            elapsed_us: optional_u64("elapsed_us"),
            publishes: optional_u64("publishes"),
            replacements: optional_u64("replacements"),
            evictions: optional_u64("evictions"),
            replaced: optional_bool("replaced"),
            capacity: optional_u64("capacity"),
            foundation_resident_estimated_bytes: optional_u64(
                "foundation_resident_estimated_bytes",
            ),
            snapshot_resident_estimated_bytes: optional_u64("snapshot_resident_estimated_bytes"),
            fts_index_bytes: optional_u64("fts_index_bytes"),
            sqlite_cache_budget_bytes: optional_u64("sqlite_cache_budget_bytes"),
            aggregate_resident_budget_bytes: optional_u64("aggregate_resident_budget_bytes"),
        }),
        "prepared_open_overlay_rebuild" => Some(PreparedMetricEvent::OpenRebuild {
            open_index_estimated_bytes: optional_u64("open_index_estimated_bytes"),
            resident_estimated_bytes: optional_u64("resident_estimated_bytes"),
            replacement_peak_estimated_bytes: optional_u64("replacement_peak_estimated_bytes"),
        }),
        _ => None,
    }
}

#[test]
fn optimized_trace_parser_is_duration_safe_and_field_whitelisted() {
    let records = [
        json!({
            "target": "bcsp_performance",
            "fields": {
                "phase": "prepared_snapshot_bind",
                "catalog_vector": "safe trace-only vector",
            },
        }),
        json!({
            "target": "bcsp_performance",
            "fields": {
                "phase": "predicate",
                "elapsed_us": 250,
                "route": "course_search",
                "path": "PRIVATE_MARKER_SHOULD_NOT_PERSIST",
            },
        }),
        json!({
            "target": "bcsp_performance",
            "fields": {
                "phase": "prepared_snapshot_bind",
                "elapsed_us": 5,
                "catalog_vector": "92026/NB/catalog=1",
                "open_vector": "92026/NB/catalog=1/open=82",
                "path": "PRIVATE_MARKER_SHOULD_NOT_PERSIST",
            },
        }),
        json!({
            "target": "bcsp_prepared_serving",
            "fields": {
                "phase": "prepared_snapshot_access",
                "hit": true,
                "hits": 4,
                "misses": 0,
                "publishes": 1,
                "replacements": 0,
                "evictions": 0,
                "capacity": 1,
                "path": "PRIVATE_MARKER_SHOULD_NOT_PERSIST",
            },
        }),
        json!({
            "target": "bcsp_prepared_serving",
            "fields": {
                "phase": "prepared_snapshot_build",
                "elapsed_us": 1500,
                "elapsed_ms": 1.5,
                "resident_estimated_bytes": 100,
                "aggregate_resident_budget_bytes": 200,
            },
        }),
        json!({
            "target": "bcsp_prepared_serving",
            "fields": {
                "phase": "prepared_snapshot_publish",
                "elapsed_us": 7,
                "publishes": 2,
                "replacements": 1,
                "evictions": 1,
                "replaced": true,
                "capacity": 1,
                "foundation_resident_estimated_bytes": 100,
                "snapshot_resident_estimated_bytes": 120,
                "fts_index_bytes": 20,
                "sqlite_cache_budget_bytes": 30,
                "aggregate_resident_budget_bytes": 200,
            },
        }),
        json!({
            "target": "bcsp_prepared_serving",
            "fields": {
                "phase": "prepared_open_overlay_rebuild",
                "open_index_estimated_bytes": 20,
                "resident_estimated_bytes": 120,
                "replacement_peak_estimated_bytes": 140,
            },
        }),
    ];
    let trace = records
        .into_iter()
        .map(|record| serde_json::to_string(&record).expect("synthetic trace record"))
        .collect::<Vec<_>>()
        .join("\n");
    let parsed = parse_profile_trace(trace.as_bytes());

    assert_eq!(parsed.phase_totals_ms.get("predicate"), Some(&0.25));
    assert_eq!(
        parsed.phase_totals_ms.get("prepared_snapshot_bind"),
        Some(&0.005)
    );
    assert_eq!(
        parsed.non_duration_performance_phases,
        vec!["prepared_snapshot_bind"]
    );
    assert_eq!(parsed.events.len(), 2);
    assert_eq!(
        parsed.events[0].dimensions.get("route"),
        Some(&json!("course_search"))
    );
    assert!(!parsed.events[0].dimensions.contains_key("path"));
    assert_eq!(
        parsed.events[1].dimensions.get("catalog_vector"),
        Some(&json!("92026/NB/catalog=1"))
    );
    assert_eq!(parsed.prepared_metrics.len(), 4);
    assert!(matches!(
        parsed.prepared_metrics[0],
        PreparedMetricEvent::Access {
            hit: Some(true),
            publishes: Some(1),
            ..
        }
    ));
    assert!(matches!(
        parsed.prepared_metrics[1],
        PreparedMetricEvent::Build {
            elapsed_us: Some(1500),
            catalog_json_bytes: None,
            query_index_estimated_bytes: None,
            open_index_estimated_bytes: None,
            foundation_resident_estimated_bytes: None,
            aggregate_resident_budget_bytes: Some(200),
            ..
        }
    ));
    assert!(matches!(
        parsed.prepared_metrics[2],
        PreparedMetricEvent::Publish {
            replaced: Some(true),
            evictions: Some(1),
            snapshot_resident_estimated_bytes: Some(120),
            ..
        }
    ));
    assert!(matches!(
        parsed.prepared_metrics[3],
        PreparedMetricEvent::OpenRebuild {
            open_index_estimated_bytes: Some(20),
            resident_estimated_bytes: Some(120),
            replacement_peak_estimated_bytes: Some(140),
        }
    ));
    let sanitized = serde_json::to_string(&json!({
        "events": parsed.events,
        "preparedMetrics": parsed.prepared_metrics,
    }))
    .expect("serialize sanitized trace");
    assert!(!sanitized.contains("PRIVATE_MARKER"));
    assert!(!sanitized.contains("SHOULD_NOT_PERSIST"));
}

fn profile_repeated(
    routes: &impl RouteExtension,
    method: RequestMethod,
    path: &'static str,
    body: &[u8],
    repetitions: usize,
) -> Vec<ProfileRun> {
    let (_, reference) = invoke(routes, method.clone(), path, body);
    (0..repetitions)
        .map(|_| {
            let (profile, response) = invoke_profiled(routes, method.clone(), path, body);
            assert_eq!(response, reference, "profiled response changed for {path}");
            profile
        })
        .collect()
}

fn measure(
    routes: &impl RouteExtension,
    method: RequestMethod,
    path: &'static str,
    body: &[u8],
    warm_repetitions: usize,
) -> (DurationSummary, Vec<u8>) {
    let (cold, reference) = invoke(routes, method.clone(), path, body);
    let mut warm = Vec::with_capacity(warm_repetitions);
    for _ in 0..warm_repetitions {
        let (elapsed, response) = invoke(routes, method.clone(), path, body);
        assert_eq!(
            response, reference,
            "warm response changed for the frozen fixture {path}"
        );
        warm.push(elapsed);
    }
    let mut sorted = warm.clone();
    sorted.sort_unstable();
    let percentile = |numerator: usize| {
        let rank = (sorted.len() * numerator).div_ceil(100).max(1);
        duration_ms(sorted[rank - 1])
    };
    (
        DurationSummary {
            cold_ms: duration_ms(cold),
            warm_repetitions,
            warm_p50_ms: percentile(50),
            warm_p95_ms: percentile(95),
            warm_max_ms: duration_ms(*sorted.last().expect("at least one warm sample")),
            warm_samples_ms: warm.into_iter().map(duration_ms).collect(),
        },
        reference,
    )
}

fn duration_ms(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn evaluate_budget(
    timing: &DurationSummary,
    warm_p95_budget_ms: f64,
    warm_max_budget_ms: f64,
) -> BudgetEvaluation {
    let warm_p95_pass = timing.warm_p95_ms <= warm_p95_budget_ms;
    let warm_max_pass = timing.warm_max_ms <= warm_max_budget_ms;
    BudgetEvaluation {
        cohort: "warm",
        warm_p95_budget_ms,
        warm_max_budget_ms,
        warm_p95_pass,
        warm_max_pass,
        pass: warm_p95_pass && warm_max_pass,
    }
}

fn decode_success<T: DeserializeOwned>(body: &[u8]) -> T {
    serde_json::from_slice::<HttpSuccessEnvelope<T>>(body)
        .expect("typed success response")
        .into_data()
}

fn known_string(value: &CatalogFieldKnowledge<String>) -> Option<&str> {
    match value {
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Present { value },
        } => Some(value),
        CatalogFieldKnowledge::Known { .. } | CatalogFieldKnowledge::Unknown { .. } => None,
    }
}

fn neutral_request(selected_campuses: &[&str], page: u32) -> CourseQueryRequestV1 {
    let mut input =
        FilterValuesInputV1::for_term(TermId::try_from(REFERENCE_TERM).expect("reference term"));
    input.campuses = campuses(selected_campuses);
    CourseQueryRequestV1 {
        filters: FilterRequestV1::new(
            NormalizedFilterValuesV1::try_new(input).expect("neutral fixture"),
        ),
        page: PageRequestV1::try_new(page, 20).expect("page"),
        sort: CourseSortV1::default(),
    }
}

fn complex_request(reference: &CourseQueryResponseV1, page: u32) -> CourseQueryRequestV1 {
    let first = reference
        .items
        .first()
        .expect("neutral Search must return a Course");
    let variant = first
        .variants
        .first()
        .expect("neutral Search must materialize a variant");
    let section = variant
        .sections
        .first()
        .expect("neutral Search must materialize a same-section witness");
    let mut input =
        FilterValuesInputV1::for_term(TermId::try_from(REFERENCE_TERM).expect("reference term"));
    input.campuses = campuses(&PRODUCT_CAMPUSES);
    input.section_indexes = vec![section.section.key.index().clone()];
    if let Some(value) = known_string(&variant.variant.subject_code) {
        input.subjects = vec![CatalogSubjectCode::try_from(value).expect("published subject")];
    }
    if let Some(value) = known_string(&variant.variant.course_number)
        && let Some(band) = bcsp_contracts::course_number_band(value)
    {
        input.course_number_bands = vec![band];
    }
    if let Some(value) = known_string(&variant.variant.level) {
        input.levels = vec![FilterTokenV1::try_from(value).expect("published level")];
    }
    CourseQueryRequestV1 {
        filters: FilterRequestV1::new(
            NormalizedFilterValuesV1::try_new(input).expect("complex fixture"),
        ),
        page: PageRequestV1::try_new(page, 20).expect("page"),
        sort: CourseSortV1::default(),
    }
}

fn course_witness(body: &[u8]) -> FunctionalWitness {
    let response: HttpSuccessEnvelope<CourseQueryResponseV1> =
        serde_json::from_slice(body).expect("Course Search response");
    let data = response.data();
    let ordered_course_keys = data
        .items
        .iter()
        .map(|item| serde_json::to_value(&item.group.key).expect("Course key"))
        .collect();
    let materialized_section_keys = data
        .items
        .iter()
        .flat_map(|item| &item.variants)
        .flat_map(|variant| &variant.sections)
        .map(|section| serde_json::to_value(&section.section.key).expect("Section key"))
        .collect();
    FunctionalWitness {
        response_sha256: sha256_hex(body),
        response_bytes: body.len(),
        page: serde_json::to_value(data.page).expect("page info"),
        ordered_course_keys,
        materialized_section_keys,
    }
}

fn generic_witness(body: &[u8]) -> FunctionalWitness {
    let envelope: Value = serde_json::from_slice(body).expect("JSON response");
    FunctionalWitness {
        response_sha256: sha256_hex(body),
        response_bytes: body.len(),
        page: envelope
            .pointer("/data/page")
            .cloned()
            .unwrap_or(Value::Null),
        ordered_course_keys: Vec::new(),
        materialized_section_keys: Vec::new(),
    }
}

fn publication_witness(body: &[u8]) -> Vec<TermPublicationWitness> {
    let status = decode_success::<ServiceStatusV2>(body);
    status
        .term_window
        .visible_terms
        .into_iter()
        .map(|term| TermPublicationWitness {
            ready_target_count: status
                .targets
                .iter()
                .filter(|target| target.target.term() == &term.term && target.usable)
                .count(),
            term: term.term.as_str().to_owned(),
            publication: term.publication,
        })
        .collect()
}

fn serving_vector(
    storage: &Arc<Mutex<OperationalStorage>>,
    selected_targets: &[TermCampusKey],
) -> Vec<TargetVectorEntry> {
    let storage = storage.lock().expect("storage lock");
    let mut selected_targets = selected_targets.to_vec();
    selected_targets.sort();
    selected_targets
        .into_iter()
        .map(|target| {
            let catalog = storage
                .target_state(&target)
                .expect("Catalog state")
                .expect("published Catalog");
            let open = storage
                .open_batch_state(&target)
                .expect("Open state")
                .expect("published Open state");
            TargetVectorEntry {
                term: target.term().as_str().to_owned(),
                campus: target.campus().as_str().to_owned(),
                catalog_content_version: catalog.current_content_version,
                open_observation_sequence: open.last_observation_sequence,
                open_catalog_content_version: open.current_catalog_content_version,
            }
        })
        .collect()
}

fn require_ready_targets(
    storage: &Arc<Mutex<OperationalStorage>>,
    selected_targets: &[TermCampusKey],
) {
    let storage = storage.lock().expect("storage lock");
    for target in selected_targets {
        assert!(
            storage
                .complete_target_snapshot_state(target)
                .expect("complete target state")
                .ready,
            "the fixed HumanTest target is not READY: {target:?}"
        );
    }
}

fn required_path(name: &str) -> PathBuf {
    std::env::var_os(name)
        .map(PathBuf::from)
        .unwrap_or_else(|| panic!("{name} must name an explicit cache path"))
}

fn required_text(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} must be set explicitly"))
}

fn optimized_output_path(file_name: &str) -> PathBuf {
    let directory = required_path("RC4_OPTIMIZED_EVIDENCE_DIR");
    fs::create_dir_all(&directory).expect("create optimized evidence directory");
    directory.join(file_name)
}

fn load_unoptimized_oracle(database_bundle: &[DatabaseArtifact]) -> Value {
    let path = required_path("RC4_UNOPTIMIZED_REFERENCE");
    assert_eq!(
        path.file_name().and_then(|value| value.to_str()),
        Some(UNOPTIMIZED_REFERENCE_ARTIFACT),
        "optimized evidence must bind the authoritative oracle basename"
    );
    let (_, sha256) = file_identity(&path);
    assert_eq!(
        sha256, UNOPTIMIZED_REFERENCE_SHA256,
        "authoritative unoptimized V3 oracle hash changed"
    );
    let oracle: Value = serde_json::from_slice(&fs::read(path).expect("read unoptimized oracle"))
        .expect("parse unoptimized oracle");
    assert_eq!(
        oracle.pointer("/schemaVersion"),
        Some(&json!(1)),
        "unexpected oracle schema"
    );
    assert_eq!(
        oracle.pointer("/referenceKind").and_then(Value::as_str),
        Some("UNOPTIMIZED_QUERY_V3_FUNCTIONAL_REFERENCE")
    );
    assert_eq!(
        oracle.pointer("/fixedClock").and_then(Value::as_str),
        Some("2026-07-17T12:00:00Z")
    );
    assert_eq!(oracle.pointer("/queryContractVersion"), Some(&json!(3)));
    assert_eq!(
        oracle.pointer("/term").and_then(Value::as_str),
        Some(REFERENCE_TERM)
    );
    assert_oracle_value(&oracle, "/campuses", &json!(PRODUCT_CAMPUSES));
    assert_oracle_value(
        &oracle,
        "/databaseBundle",
        &serde_json::to_value(database_bundle).expect("database bundle evidence"),
    );
    oracle
}

fn assert_oracle_value(oracle: &Value, pointer: &str, actual: &Value) {
    assert_eq!(
        oracle
            .pointer(pointer)
            .unwrap_or_else(|| panic!("oracle lacks {pointer}")),
        actual,
        "optimized evidence differs from oracle at {pointer}"
    );
}

fn compare_oracle_witness(
    oracle: &Value,
    pointer: &str,
    actual: &FunctionalWitness,
) -> OracleWitnessComparison {
    let actual_value = serde_json::to_value(actual).expect("serialize optimized witness");
    let expected = oracle
        .pointer(pointer)
        .unwrap_or_else(|| panic!("oracle lacks {pointer}"));
    assert_eq!(
        expected, &actual_value,
        "optimized witness differs at {pointer}"
    );
    let expected_response_sha256 = expected
        .get("responseSha256")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("oracle witness {pointer} lacks responseSha256"))
        .to_owned();
    OracleWitnessComparison {
        expected_response_sha256,
        actual_response_sha256: actual.response_sha256.clone(),
        exact_witness: true,
    }
}

fn assert_profile_matches_oracle(oracle: &Value, pointer: &str, runs: &[ProfileRun]) {
    let expected = oracle
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("oracle lacks response hash {pointer}"));
    assert!(
        runs.iter().all(|run| run.response_sha256 == expected),
        "optimized profile response differs from oracle at {pointer}"
    );
}

fn repeated_response_sha256(label: &str, runs: &[ProfileRun]) -> String {
    let expected = runs
        .first()
        .unwrap_or_else(|| panic!("{label} profile must contain at least one run"))
        .response_sha256
        .clone();
    assert!(
        runs.iter().all(|run| run.response_sha256 == expected),
        "{label} response changed across profiled repetitions"
    );
    expected
}

fn assert_prepared_hits(label: &str, runs: &[ProfileRun]) {
    assert!(
        runs.iter().all(|run| {
            run.prepared_metrics.iter().any(|metric| {
                matches!(
                    metric,
                    PreparedMetricEvent::Access {
                        hit: Some(true),
                        ..
                    }
                )
            })
        }),
        "{label} did not bind a prepared snapshot in every profiled run"
    );
}

fn assert_no_repeated_request_build_phases(label: &str, runs: &[ProfileRun]) {
    const FORBIDDEN_PHASES: [&str; 5] = [
        "sqlite_catalog_load",
        "catalog_projection",
        "discovery_full_projection",
        "corpus_build",
        "open_overlay_build",
    ];
    for (run_index, run) in runs.iter().enumerate() {
        for phase in FORBIDDEN_PHASES {
            assert!(
                !run.phase_totals_ms.contains_key(phase)
                    && run.events.iter().all(|event| event.phase != phase),
                "{label} optimized profile run {run_index} repeated forbidden request phase {phase}"
            );
        }
    }
}

fn suffixed_path(path: &Path, suffix: &str) -> PathBuf {
    let mut value = OsString::from(path.as_os_str());
    value.push(suffix);
    PathBuf::from(value)
}

fn database_bundle_identity(database_path: &Path) -> Vec<DatabaseArtifact> {
    [
        database_path.to_path_buf(),
        suffixed_path(database_path, "-wal"),
        suffixed_path(database_path, "-shm"),
    ]
    .into_iter()
    .map(|path| {
        let (bytes, sha256) = file_identity(&path);
        DatabaseArtifact {
            artifact: path
                .file_name()
                .and_then(|value| value.to_str())
                .expect("database artifact file name")
                .to_owned(),
            bytes,
            sha256,
        }
    })
    .collect()
}

fn file_identity(path: &Path) -> (u64, String) {
    let mut file = File::open(path).expect("open evidence input artifact");
    let bytes = file.metadata().expect("evidence input metadata").len();
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .expect("hash evidence input artifact");
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    (bytes, format!("{:x}", digest.finalize()))
}

fn environment_evidence() -> EnvironmentEvidence {
    EnvironmentEvidence {
        build_identity: required_text("RC4_BUILD_IDENTITY"),
        os: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        available_parallelism: std::thread::available_parallelism()
            .expect("available parallelism")
            .get(),
        processor_identifier: std::env::var("PROCESSOR_IDENTIFIER").ok(),
    }
}

#[test]
#[ignore = "manual RC4 V3 performance evidence runner"]
fn capture_unoptimized_v3_functional_reference() {
    let database_path = required_path("RC4_REFERENCE_DB");
    let output_path = required_path("RC4_REFERENCE_OUTPUT");
    let database_bundle = database_bundle_identity(&database_path);
    let environment = environment_evidence();
    let (routes, storage) = fixed_routes(&database_path);
    let one_target = targets(&["NB"]);
    let three_targets = targets(&PRODUCT_CAMPUSES);
    require_ready_targets(&storage, &three_targets);
    let vector_before = serving_vector(&storage, &three_targets);

    let (status_timing, status_body) = measure(
        &routes,
        RequestMethod::Get,
        PRODUCT_SERVICE_STATUS_PATH,
        &[],
        20,
    );
    let status_publication = publication_witness(&status_body);
    let options_request = FilterOptionsRequestV2 {
        contract_version: QUERY_CONTRACT_VERSION,
        term: TermId::try_from(REFERENCE_TERM).expect("reference term"),
        campuses: campuses(&PRODUCT_CAMPUSES),
        field: FilterOptionsFieldV2::CourseNumberBand,
        query: None,
        limit: None,
    };
    let (options_timing, options_body) = measure(
        &routes,
        RequestMethod::Post,
        PRODUCT_FILTER_OPTIONS_PATH,
        &request_body(options_request),
        10,
    );

    let single_request = neutral_request(&["NB"], 1);
    let (single_timing, single_body) = measure(
        &routes,
        RequestMethod::Post,
        PRODUCT_COURSE_SEARCH_PATH,
        &request_body(single_request),
        10,
    );
    let single_response = decode_success::<CourseQueryResponseV1>(&single_body);
    assert!(
        single_response.page.total > 0,
        "single-Campus fixture is empty"
    );

    let complex_page_one = complex_request(&single_response, 1);
    let (complex_timing, complex_body) = measure(
        &routes,
        RequestMethod::Post,
        PRODUCT_COURSE_SEARCH_PATH,
        &request_body(&complex_page_one),
        10,
    );
    let complex_response = decode_success::<CourseQueryResponseV1>(&complex_body);
    assert!(complex_response.page.total > 0, "complex fixture is empty");
    assert!(
        complex_response
            .items
            .iter()
            .flat_map(|item| &item.variants)
            .flat_map(|variant| &variant.sections)
            .any(|section| {
                complex_page_one
                    .filters
                    .values()
                    .section_indexes()
                    .contains(section.section.key.index())
            }),
        "complex fixture lacks a same-section witness"
    );
    let exact_validation_request =
        DynamicFilterValidationRequestV3::new(complex_page_one.filters.clone());
    let (exact_validation_timing, exact_validation_body) = measure(
        &routes,
        RequestMethod::Post,
        PRODUCT_DYNAMIC_FILTER_VALIDATION_PATH,
        &request_body(&exact_validation_request),
        10,
    );
    let exact_validation_response =
        decode_success::<DynamicFilterValidationResponseV3>(&exact_validation_body);
    assert!(
        exact_validation_response.invalid_values.is_empty(),
        "frozen active dynamic values must be exact-valid"
    );
    assert_eq!(
        exact_validation_response.target_versions.len(),
        PRODUCT_CAMPUSES.len(),
        "exact validation must bind the complete three-Campus Catalog vector"
    );
    let (_, complex_page_two_body) = invoke(
        &routes,
        RequestMethod::Post,
        PRODUCT_COURSE_SEARCH_PATH,
        &request_body(complex_request(&single_response, 2)),
    );

    let vector_after = serving_vector(&storage, &three_targets);
    assert_eq!(
        vector_after, vector_before,
        "reference requests changed serving vector"
    );
    let expected_nb = vector_before
        .iter()
        .find(|entry| entry.campus == "NB")
        .expect("NB serving vector")
        .clone();
    assert_eq!(serving_vector(&storage, &one_target), vec![expected_nb]);

    let evidence = json!({
        "schemaVersion": 1,
        "referenceKind": "UNOPTIMIZED_QUERY_V3_FUNCTIONAL_REFERENCE",
        "fixedClock": "2026-07-17T12:00:00Z",
        "databaseBundleLabel": required_text("RC4_DATABASE_BUNDLE_LABEL"),
        "databaseBundle": database_bundle,
        "environment": environment,
        "queryContractVersion": 3,
        "term": REFERENCE_TERM,
        "campuses": PRODUCT_CAMPUSES,
        "coldDefinition": "first route invocation after opening one SQLite connection; OS file cache uncontrolled",
        "warmDefinition": "immediately repeated identical in-process route invocations on the same connection",
        "budgetsMs": {
            "status": {"p95": 250, "max": 1000},
            "filterOptions": {"p95": 500, "max": 1500},
            "singleCampusSearch": {"p95": 1000, "max": 2000},
            "threeCampusComplexSearch": {"p95": 2000, "max": 5000},
        },
        "status": FixtureEvidence {
            timing: status_timing,
            witness: generic_witness(&status_body),
        },
        "statusTermPublication": status_publication,
        "filterOptionsCourseNumberBands": FixtureEvidence {
            timing: options_timing,
            witness: generic_witness(&options_body),
        },
        "singleCampusNeutralSearch": FixtureEvidence {
            timing: single_timing,
            witness: course_witness(&single_body),
        },
        "threeCampusComplexSearch": FixtureEvidence {
            timing: complex_timing,
            witness: course_witness(&complex_body),
        },
        "exactDynamicFilterValidation": FixtureEvidence {
            timing: exact_validation_timing,
            witness: generic_witness(&exact_validation_body),
        },
        "exactDynamicFilterValidationResponse": exact_validation_response,
        "threeCampusComplexPageTwo": course_witness(&complex_page_two_body),
        "complexFilters": complex_page_one.filters,
        "servingVectorBefore": vector_before,
        "servingVectorAfter": vector_after,
    });
    fs::write(
        &output_path,
        serde_json::to_vec_pretty(&evidence).expect("serialize evidence"),
    )
    .expect("write cache evidence");
}

#[test]
#[ignore = "manual RC4 V3 profiler evidence runner"]
fn capture_unoptimized_v3_profiler_breakdown() {
    let database_path = required_path("RC4_REFERENCE_DB");
    let output_path = required_path("RC4_PROFILE_OUTPUT");
    let database_bundle = database_bundle_identity(&database_path);
    let environment = environment_evidence();
    let (routes, storage) = fixed_routes(&database_path);
    let three_targets = targets(&PRODUCT_CAMPUSES);
    require_ready_targets(&storage, &three_targets);
    let vector_before = serving_vector(&storage, &three_targets);

    let options_request = FilterOptionsRequestV2 {
        contract_version: QUERY_CONTRACT_VERSION,
        term: TermId::try_from(REFERENCE_TERM).expect("reference term"),
        campuses: campuses(&PRODUCT_CAMPUSES),
        field: FilterOptionsFieldV2::CourseNumberBand,
        query: None,
        limit: None,
    };
    let single_request = neutral_request(&["NB"], 1);
    let (_, single_body) = invoke(
        &routes,
        RequestMethod::Post,
        PRODUCT_COURSE_SEARCH_PATH,
        &request_body(&single_request),
    );
    let single_response = decode_success::<CourseQueryResponseV1>(&single_body);
    let complex_request = complex_request(&single_response, 1);
    let exact_validation_request =
        DynamicFilterValidationRequestV3::new(complex_request.filters.clone());

    let status = profile_repeated(
        &routes,
        RequestMethod::Get,
        PRODUCT_SERVICE_STATUS_PATH,
        &[],
        3,
    );
    let options = profile_repeated(
        &routes,
        RequestMethod::Post,
        PRODUCT_FILTER_OPTIONS_PATH,
        &request_body(&options_request),
        3,
    );
    let single = profile_repeated(
        &routes,
        RequestMethod::Post,
        PRODUCT_COURSE_SEARCH_PATH,
        &request_body(&single_request),
        3,
    );
    let complex = profile_repeated(
        &routes,
        RequestMethod::Post,
        PRODUCT_COURSE_SEARCH_PATH,
        &request_body(&complex_request),
        3,
    );
    let exact_validation = profile_repeated(
        &routes,
        RequestMethod::Post,
        PRODUCT_DYNAMIC_FILTER_VALIDATION_PATH,
        &request_body(&exact_validation_request),
        3,
    );

    for run in single.iter().chain(&complex) {
        for phase in [
            "storage_lock_wait",
            "storage_lock_held",
            "sqlite_catalog_load",
            "catalog_projection",
            "open_projection",
            "corpus_build",
            "open_overlay_build",
            "predicate",
            "sort",
            "pagination",
            "materialization",
            "serialization",
        ] {
            assert!(
                run.phase_totals_ms.contains_key(phase),
                "missing required profile phase {phase}"
            );
        }
    }
    let vector_after = serving_vector(&storage, &three_targets);
    assert_eq!(
        vector_after, vector_before,
        "profiling changed serving vector"
    );

    let evidence = json!({
        "schemaVersion": 1,
        "referenceKind": "UNOPTIMIZED_QUERY_V3_PROFILE",
        "fixedClock": "2026-07-17T12:00:00Z",
        "databaseBundleLabel": required_text("RC4_DATABASE_BUNDLE_LABEL"),
        "databaseBundle": database_bundle,
        "environment": environment,
        "buildProfile": "release",
        "repetitions": 3,
        "status": status,
        "filterOptionsCourseNumberBands": options,
        "singleCampusNeutralSearch": single,
        "threeCampusComplexSearch": complex,
        "exactDynamicFilterValidation": exact_validation,
        "complexFilters": complex_request.filters,
        "servingVectorBefore": vector_before,
        "servingVectorAfter": vector_after,
    });
    fs::write(
        output_path,
        serde_json::to_vec_pretty(&evidence).expect("serialize profile evidence"),
    )
    .expect("write profiler evidence");
}

#[test]
#[ignore = "manual RC4 optimized V3 performance evidence runner"]
fn capture_optimized_v3_functional_reference() {
    let database_path = required_path("RC4_OPTIMIZED_DB");
    let output_path = optimized_output_path(OPTIMIZED_REFERENCE_ARTIFACT);
    let database_bundle = database_bundle_identity(&database_path);
    let oracle = load_unoptimized_oracle(&database_bundle);
    let environment = environment_evidence();
    let (routes, storage, prepared_initial_build) = fixed_routes_profiled(&database_path);
    let one_target = targets(&["NB"]);
    let three_targets = targets(&PRODUCT_CAMPUSES);
    require_ready_targets(&storage, &three_targets);
    let vector_before = serving_vector(&storage, &three_targets);
    assert_oracle_value(
        &oracle,
        "/servingVectorBefore",
        &serde_json::to_value(&vector_before).expect("optimized serving vector"),
    );

    let (status_timing, status_body) = measure(
        &routes,
        RequestMethod::Get,
        PRODUCT_SERVICE_STATUS_PATH,
        &[],
        20,
    );
    let status_publication = publication_witness(&status_body);
    assert_oracle_value(
        &oracle,
        "/statusTermPublication",
        &serde_json::to_value(&status_publication).expect("status publication witness"),
    );

    let options_request = FilterOptionsRequestV2 {
        contract_version: QUERY_CONTRACT_VERSION,
        term: TermId::try_from(REFERENCE_TERM).expect("reference term"),
        campuses: campuses(&PRODUCT_CAMPUSES),
        field: FilterOptionsFieldV2::CourseNumberBand,
        query: None,
        limit: None,
    };
    let (options_timing, options_body) = measure(
        &routes,
        RequestMethod::Post,
        PRODUCT_FILTER_OPTIONS_PATH,
        &request_body(options_request),
        10,
    );

    let single_request = neutral_request(&["NB"], 1);
    let (single_timing, single_body) = measure(
        &routes,
        RequestMethod::Post,
        PRODUCT_COURSE_SEARCH_PATH,
        &request_body(single_request),
        10,
    );
    let single_response = decode_success::<CourseQueryResponseV1>(&single_body);
    assert!(
        single_response.page.total > 0,
        "single-Campus fixture is empty"
    );

    let complex_page_one = complex_request(&single_response, 1);
    assert_oracle_value(
        &oracle,
        "/complexFilters",
        &serde_json::to_value(&complex_page_one.filters).expect("complex filters"),
    );
    let (complex_timing, complex_body) = measure(
        &routes,
        RequestMethod::Post,
        PRODUCT_COURSE_SEARCH_PATH,
        &request_body(&complex_page_one),
        10,
    );
    let complex_response = decode_success::<CourseQueryResponseV1>(&complex_body);
    assert!(complex_response.page.total > 0, "complex fixture is empty");
    assert!(
        complex_response
            .items
            .iter()
            .flat_map(|item| &item.variants)
            .flat_map(|variant| &variant.sections)
            .any(|section| {
                complex_page_one
                    .filters
                    .values()
                    .section_indexes()
                    .contains(section.section.key.index())
            }),
        "complex fixture lacks a same-section witness"
    );

    let exact_validation_request =
        DynamicFilterValidationRequestV3::new(complex_page_one.filters.clone());
    let (exact_validation_timing, exact_validation_body) = measure(
        &routes,
        RequestMethod::Post,
        PRODUCT_DYNAMIC_FILTER_VALIDATION_PATH,
        &request_body(&exact_validation_request),
        10,
    );
    let exact_validation_response =
        decode_success::<DynamicFilterValidationResponseV3>(&exact_validation_body);
    assert!(
        exact_validation_response.invalid_values.is_empty(),
        "frozen active dynamic values must be exact-valid"
    );
    assert_oracle_value(
        &oracle,
        "/exactDynamicFilterValidationResponse",
        &serde_json::to_value(&exact_validation_response).expect("exact validation response"),
    );

    let (_, complex_page_two_body) = invoke(
        &routes,
        RequestMethod::Post,
        PRODUCT_COURSE_SEARCH_PATH,
        &request_body(complex_request(&single_response, 2)),
    );
    let vector_after = serving_vector(&storage, &three_targets);
    assert_eq!(
        vector_after, vector_before,
        "optimized requests changed serving vector"
    );
    assert_oracle_value(
        &oracle,
        "/servingVectorAfter",
        &serde_json::to_value(&vector_after).expect("optimized serving vector"),
    );
    let expected_nb = vector_before
        .iter()
        .find(|entry| entry.campus == "NB")
        .expect("NB serving vector")
        .clone();
    assert_eq!(serving_vector(&storage, &one_target), vec![expected_nb]);

    let status_witness = generic_witness(&status_body);
    let options_witness = generic_witness(&options_body);
    let single_witness = course_witness(&single_body);
    let complex_witness = course_witness(&complex_body);
    let exact_validation_witness = generic_witness(&exact_validation_body);
    let complex_page_two_witness = course_witness(&complex_page_two_body);
    let status_comparison = compare_oracle_witness(&oracle, "/status/witness", &status_witness);
    let options_comparison = compare_oracle_witness(
        &oracle,
        "/filterOptionsCourseNumberBands/witness",
        &options_witness,
    );
    let single_comparison = compare_oracle_witness(
        &oracle,
        "/singleCampusNeutralSearch/witness",
        &single_witness,
    );
    let complex_comparison = compare_oracle_witness(
        &oracle,
        "/threeCampusComplexSearch/witness",
        &complex_witness,
    );
    let exact_validation_comparison = compare_oracle_witness(
        &oracle,
        "/exactDynamicFilterValidation/witness",
        &exact_validation_witness,
    );
    assert_oracle_value(
        &oracle,
        "/threeCampusComplexPageTwo",
        &serde_json::to_value(&complex_page_two_witness).expect("complex page two witness"),
    );

    let status_budget = evaluate_budget(&status_timing, STATUS_P95_BUDGET_MS, STATUS_MAX_BUDGET_MS);
    let options_budget = evaluate_budget(
        &options_timing,
        FILTER_OPTIONS_P95_BUDGET_MS,
        FILTER_OPTIONS_MAX_BUDGET_MS,
    );
    let single_budget = evaluate_budget(
        &single_timing,
        SINGLE_SEARCH_P95_BUDGET_MS,
        SINGLE_SEARCH_MAX_BUDGET_MS,
    );
    let complex_budget = evaluate_budget(
        &complex_timing,
        COMPLEX_SEARCH_P95_BUDGET_MS,
        COMPLEX_SEARCH_MAX_BUDGET_MS,
    );
    let all_budgets_pass =
        status_budget.pass && options_budget.pass && single_budget.pass && complex_budget.pass;

    let evidence = json!({
        "schemaVersion": 2,
        "referenceKind": "OPTIMIZED_QUERY_V3_FUNCTIONAL_REFERENCE",
        "oracle": {
            "artifact": UNOPTIMIZED_REFERENCE_ARTIFACT,
            "sha256": UNOPTIMIZED_REFERENCE_SHA256,
            "allFunctionalWitnessesExact": true,
        },
        "fixedClock": "2026-07-17T12:00:00Z",
        "databaseBundleLabel": required_text("RC4_DATABASE_BUNDLE_LABEL"),
        "databaseBundle": database_bundle,
        "environment": environment,
        "queryContractVersion": 3,
        "term": REFERENCE_TERM,
        "campuses": PRODUCT_CAMPUSES,
        "coldDefinition": "first route invocation after one prepared snapshot was built and published from one SQLite connection; OS file cache uncontrolled",
        "warmDefinition": "immediately repeated identical in-process route invocations on the same prepared generation",
        "percentileMethod": "nearest-rank ceil(N*p/100)",
        "budgetsApplyTo": "warm cohort",
        "budgetsMs": {
            "status": {"p95": STATUS_P95_BUDGET_MS, "max": STATUS_MAX_BUDGET_MS},
            "filterOptions": {"p95": FILTER_OPTIONS_P95_BUDGET_MS, "max": FILTER_OPTIONS_MAX_BUDGET_MS},
            "singleCampusSearch": {"p95": SINGLE_SEARCH_P95_BUDGET_MS, "max": SINGLE_SEARCH_MAX_BUDGET_MS},
            "threeCampusComplexSearch": {"p95": COMPLEX_SEARCH_P95_BUDGET_MS, "max": COMPLEX_SEARCH_MAX_BUDGET_MS},
        },
        "allBudgetsPass": all_budgets_pass,
        "preparedServingInitialBuild": prepared_initial_build,
        "preparedServingMetricsAvailability": {
            "initialBuildAndBytes": "CAPTURED_FROM_WHITELISTED_TRACE",
            "requestHitMiss": "CAPTURED_IN_OPTIMIZED_PROFILE",
            "publishReplacementEviction": "CAPTURED_FROM_WHITELISTED_TRACE",
        },
        "status": {
            "timing": status_timing,
            "witness": status_witness,
            "budgetEvaluation": status_budget,
            "oracleComparison": status_comparison,
        },
        "statusTermPublication": status_publication,
        "filterOptionsCourseNumberBands": {
            "timing": options_timing,
            "witness": options_witness,
            "budgetEvaluation": options_budget,
            "oracleComparison": options_comparison,
        },
        "singleCampusNeutralSearch": {
            "timing": single_timing,
            "witness": single_witness,
            "budgetEvaluation": single_budget,
            "oracleComparison": single_comparison,
        },
        "threeCampusComplexSearch": {
            "timing": complex_timing,
            "witness": complex_witness,
            "budgetEvaluation": complex_budget,
            "oracleComparison": complex_comparison,
        },
        "exactDynamicFilterValidation": {
            "timing": exact_validation_timing,
            "witness": exact_validation_witness,
            "oracleComparison": exact_validation_comparison,
        },
        "exactDynamicFilterValidationResponse": exact_validation_response,
        "threeCampusComplexPageTwo": complex_page_two_witness,
        "complexFilters": complex_page_one.filters,
        "servingVectorBefore": vector_before,
        "servingVectorAfter": vector_after,
    });
    fs::write(
        output_path,
        serde_json::to_vec_pretty(&evidence).expect("serialize optimized evidence"),
    )
    .expect("write optimized functional evidence");
    assert!(
        all_budgets_pass,
        "one or more optimized warm budgets failed"
    );
}

#[test]
#[ignore = "manual RC4 optimized V3 profiler evidence runner"]
fn capture_optimized_v3_profiler_breakdown() {
    let database_path = required_path("RC4_OPTIMIZED_DB");
    let output_path = optimized_output_path(OPTIMIZED_PROFILE_ARTIFACT);
    let database_bundle = database_bundle_identity(&database_path);
    let oracle = load_unoptimized_oracle(&database_bundle);
    let environment = environment_evidence();
    let (routes, storage, prepared_initial_build) = fixed_routes_profiled(&database_path);
    let three_targets = targets(&PRODUCT_CAMPUSES);
    require_ready_targets(&storage, &three_targets);
    let vector_before = serving_vector(&storage, &three_targets);
    assert_oracle_value(
        &oracle,
        "/servingVectorBefore",
        &serde_json::to_value(&vector_before).expect("optimized serving vector"),
    );

    let options_request = FilterOptionsRequestV2 {
        contract_version: QUERY_CONTRACT_VERSION,
        term: TermId::try_from(REFERENCE_TERM).expect("reference term"),
        campuses: campuses(&PRODUCT_CAMPUSES),
        field: FilterOptionsFieldV2::CourseNumberBand,
        query: None,
        limit: None,
    };
    let single_request = neutral_request(&["NB"], 1);
    let (_, single_body) = invoke(
        &routes,
        RequestMethod::Post,
        PRODUCT_COURSE_SEARCH_PATH,
        &request_body(&single_request),
    );
    let single_response = decode_success::<CourseQueryResponseV1>(&single_body);
    let first_course = single_response
        .items
        .first()
        .expect("single-Campus Search must return a Course for detail profiling");
    let course_detail_key = first_course.group.key.clone();
    let section_detail_key = first_course
        .variants
        .first()
        .expect("detail fixture Course must materialize a variant")
        .sections
        .first()
        .expect("detail fixture variant must materialize a Section")
        .section
        .key
        .clone();
    let course_detail_request = CourseDetailRequestV1::new(course_detail_key.clone());
    let section_detail_request = SectionDetailRequestV1::new(section_detail_key.clone());
    let complex_request = complex_request(&single_response, 1);
    assert_oracle_value(
        &oracle,
        "/complexFilters",
        &serde_json::to_value(&complex_request.filters).expect("complex filters"),
    );
    let exact_validation_request =
        DynamicFilterValidationRequestV3::new(complex_request.filters.clone());

    let status = profile_repeated(
        &routes,
        RequestMethod::Get,
        PRODUCT_SERVICE_STATUS_PATH,
        &[],
        3,
    );
    let options = profile_repeated(
        &routes,
        RequestMethod::Post,
        PRODUCT_FILTER_OPTIONS_PATH,
        &request_body(&options_request),
        3,
    );
    let single = profile_repeated(
        &routes,
        RequestMethod::Post,
        PRODUCT_COURSE_SEARCH_PATH,
        &request_body(&single_request),
        3,
    );
    let complex = profile_repeated(
        &routes,
        RequestMethod::Post,
        PRODUCT_COURSE_SEARCH_PATH,
        &request_body(&complex_request),
        3,
    );
    let exact_validation = profile_repeated(
        &routes,
        RequestMethod::Post,
        PRODUCT_DYNAMIC_FILTER_VALIDATION_PATH,
        &request_body(&exact_validation_request),
        3,
    );
    let course_detail = profile_repeated(
        &routes,
        RequestMethod::Post,
        PRODUCT_COURSE_DETAIL_PATH,
        &request_body(&course_detail_request),
        3,
    );
    let section_detail = profile_repeated(
        &routes,
        RequestMethod::Post,
        PRODUCT_SECTION_DETAIL_PATH,
        &request_body(&section_detail_request),
        3,
    );

    assert_profile_matches_oracle(&oracle, "/status/witness/responseSha256", &status);
    assert_profile_matches_oracle(
        &oracle,
        "/filterOptionsCourseNumberBands/witness/responseSha256",
        &options,
    );
    assert_profile_matches_oracle(
        &oracle,
        "/singleCampusNeutralSearch/witness/responseSha256",
        &single,
    );
    assert_profile_matches_oracle(
        &oracle,
        "/threeCampusComplexSearch/witness/responseSha256",
        &complex,
    );
    assert_profile_matches_oracle(
        &oracle,
        "/exactDynamicFilterValidation/witness/responseSha256",
        &exact_validation,
    );
    assert_prepared_hits("filter options", &options);
    assert_prepared_hits("single-Campus Search", &single);
    assert_prepared_hits("three-Campus Search", &complex);
    assert_prepared_hits("exact validation", &exact_validation);
    assert_prepared_hits("Course Detail", &course_detail);
    assert_prepared_hits("Section Detail", &section_detail);
    assert_no_repeated_request_build_phases("filter options", &options);
    assert_no_repeated_request_build_phases("single-Campus Search", &single);
    assert_no_repeated_request_build_phases("three-Campus Search", &complex);
    assert_no_repeated_request_build_phases("exact validation", &exact_validation);
    assert_no_repeated_request_build_phases("Course Detail", &course_detail);
    assert_no_repeated_request_build_phases("Section Detail", &section_detail);

    let course_detail_response_sha256 = repeated_response_sha256("Course Detail", &course_detail);
    let section_detail_response_sha256 =
        repeated_response_sha256("Section Detail", &section_detail);

    for (label, runs) in [
        ("filter options", options.as_slice()),
        ("single-Campus Search", single.as_slice()),
        ("three-Campus Search", complex.as_slice()),
        ("exact validation", exact_validation.as_slice()),
        ("Course Detail", course_detail.as_slice()),
        ("Section Detail", section_detail.as_slice()),
    ] {
        assert!(
            runs.iter().all(|run| {
                run.events
                    .iter()
                    .any(|event| event.phase == "prepared_snapshot_bind")
                    && run.phase_totals_ms.contains_key("prepared_snapshot_bind")
            }),
            "{label} lacks a duration-bearing prepared vector-binding trace"
        );
    }

    let vector_after = serving_vector(&storage, &three_targets);
    assert_eq!(
        vector_after, vector_before,
        "optimized profiling changed serving vector"
    );
    assert_oracle_value(
        &oracle,
        "/servingVectorAfter",
        &serde_json::to_value(&vector_after).expect("optimized serving vector"),
    );

    let evidence = json!({
        "schemaVersion": 2,
        "referenceKind": "OPTIMIZED_QUERY_V3_PROFILE",
        "oracle": {
            "artifact": UNOPTIMIZED_REFERENCE_ARTIFACT,
            "sha256": UNOPTIMIZED_REFERENCE_SHA256,
            "allOracleCoveredProfiledResponseHashesExact": true,
            "detailResponseHashesAvailable": false,
        },
        "fixedClock": "2026-07-17T12:00:00Z",
        "databaseBundleLabel": required_text("RC4_DATABASE_BUNDLE_LABEL"),
        "databaseBundle": database_bundle,
        "environment": environment,
        "buildProfile": "release",
        "repetitions": 3,
        "tracePolicy": {
            "performanceDurations": "only bcsp_performance events carrying elapsed_us are aggregated",
            "preparedMetrics": ["prepared_snapshot_access", "prepared_snapshot_build", "prepared_snapshot_publish", "prepared_open_overlay_rebuild"],
            "rawTraceFieldsPersisted": false,
        },
        "preparedServingInitialBuild": prepared_initial_build,
        "preparedServingMetricsAvailability": {
            "initialBuildAndBytes": "CAPTURED_FROM_WHITELISTED_TRACE",
            "requestHitMiss": "CAPTURED_FROM_WHITELISTED_TRACE",
            "publishReplacementEviction": "CAPTURED_FROM_WHITELISTED_TRACE",
        },
        "status": status,
        "filterOptionsCourseNumberBands": options,
        "singleCampusNeutralSearch": single,
        "threeCampusComplexSearch": complex,
        "exactDynamicFilterValidation": exact_validation,
        "courseDetail": {
            "key": course_detail_key,
            "responseSha256": course_detail_response_sha256,
            "runs": course_detail,
            "functionalOracleBinding": {
                "kind": "IN_PROCESS_PREPARED_REFERENCE_JSON_PARITY_TEST",
                "test": "bcsp_application::query_service::tests::prepared_snapshot_matches_reference_options_validation_search_and_details",
                "unoptimizedArtifactDetailHashAvailable": false,
                "repeatAssertion": "BYTE_EXACT_WITHIN_FIXED_FIXTURE",
            },
        },
        "sectionDetail": {
            "key": section_detail_key,
            "responseSha256": section_detail_response_sha256,
            "runs": section_detail,
            "functionalOracleBinding": {
                "kind": "IN_PROCESS_PREPARED_REFERENCE_JSON_PARITY_TEST",
                "test": "bcsp_application::query_service::tests::prepared_snapshot_matches_reference_options_validation_search_and_details",
                "unoptimizedArtifactDetailHashAvailable": false,
                "repeatAssertion": "BYTE_EXACT_WITHIN_FIXED_FIXTURE",
            },
        },
        "complexFilters": complex_request.filters,
        "servingVectorBefore": vector_before,
        "servingVectorAfter": vector_after,
    });
    fs::write(
        output_path,
        serde_json::to_vec_pretty(&evidence).expect("serialize optimized profile evidence"),
    )
    .expect("write optimized profiler evidence");
}
