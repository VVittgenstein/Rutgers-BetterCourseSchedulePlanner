use std::collections::BTreeSet;
use std::str::FromStr;

use bcsp_contracts::{
    CatalogContentVersion, ContractSchema, OPEN_CONTRACT_VERSION, OpenAppliedClassification,
    OpenAttemptPointV1, OpenAttemptScheduleV1, OpenBatchKey, OpenCanonicalSetHash,
    OpenCircuitReason, OpenCircuitState, OpenCircuitStatusV1, OpenCounterSnapshotV1,
    OpenDecodedBodySha256, OpenFailureClass, OpenFailurePointV1, OpenFreshnessState,
    OpenFreshnessV1, OpenHttpHeaderValue, OpenHttpMetadataV1, OpenObservationPointV1,
    OpenObservationV1, OpenPullAttemptV1, OpenPullCountsV1, OpenReconcileCountsV1,
    OpenRefreshClassification, OpenRefreshObservationV1, OpenRefreshStatusV1, OpenSchedulerLane,
    OpenSchedulerStatusV1, OpenSectionStatusRequestV1, OpenSectionStatusV1, OpenSequence,
    OpenState, OpenStateHash, OpenStatusRequestV1, OpenUncertaintyReason, RutgersDay,
    RutgersDayTimezone, SectionKey, TraceId, contract_manifest,
};
use serde::Serialize;
use serde_json::Value;
use time::{Duration, OffsetDateTime};

fn schema(id: &str) -> ContractSchema {
    contract_manifest()
        .schemas
        .into_iter()
        .find(|schema| schema.id == id)
        .unwrap_or_else(|| panic!("missing contract schema {id}"))
}

fn object_keys<T: Serialize>(value: &T) -> BTreeSet<String> {
    let Value::Object(object) = serde_json::to_value(value).unwrap() else {
        panic!("representative is not an object")
    };
    object.into_iter().map(|(key, _)| key).collect()
}

fn assert_object_binding<T: Serialize>(schema_id: &str, value: &T) {
    let schema = schema(schema_id);
    let expected = schema
        .fields
        .into_iter()
        .map(|field| field.name)
        .collect::<BTreeSet<_>>();
    assert_eq!(object_keys(value), expected, "{schema_id}");
}

fn assert_enum_binding<T: Serialize>(schema_id: &str, values: &[T]) {
    let actual = values
        .iter()
        .map(|value| serde_json::to_value(value).unwrap())
        .collect::<Vec<_>>();
    let expected = schema(schema_id)
        .enum_values
        .into_iter()
        .map(Value::String)
        .collect::<Vec<_>>();
    assert_eq!(actual, expected, "{schema_id}");
}

fn timestamp() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_784_006_400).unwrap()
}

fn trace_id() -> TraceId {
    TraceId::from_str("00000000-0000-4000-8000-000000000001").unwrap()
}

fn batch() -> OpenBatchKey {
    OpenBatchKey::try_new("T2026F", "CAMPUS_A").unwrap()
}

fn section_key() -> SectionKey {
    SectionKey::try_new("T2026F", "CAMPUS_A", "00001").unwrap()
}

fn sequence() -> OpenSequence {
    OpenSequence::try_from(7).unwrap()
}

fn version() -> CatalogContentVersion {
    CatalogContentVersion::try_from(3).unwrap()
}

fn set_hash() -> OpenCanonicalSetHash {
    OpenCanonicalSetHash::try_from("a".repeat(64)).unwrap()
}

fn state_hash() -> OpenStateHash {
    OpenStateHash::try_from("b".repeat(64)).unwrap()
}

fn body_hash() -> OpenDecodedBodySha256 {
    OpenDecodedBodySha256::try_from("c".repeat(64)).unwrap()
}

fn header(value: &str) -> OpenHttpHeaderValue {
    OpenHttpHeaderValue::try_from(value).unwrap()
}

fn counts() -> OpenPullCountsV1 {
    OpenPullCountsV1 {
        attempted: 9,
        succeeded: 7,
        failed: 2,
        empty: 1,
    }
}

fn counter_snapshot() -> OpenCounterSnapshotV1 {
    OpenCounterSnapshotV1 {
        run_counts: Some(counts()),
        today_counts: counts(),
        rutgers_day: RutgersDay::try_from("2026-07-14").unwrap(),
        day_timezone: RutgersDayTimezone::AmericaNewYork,
    }
}

fn freshness() -> OpenFreshnessV1 {
    OpenFreshnessV1 {
        state: OpenFreshnessState::Stale,
        observed_at: Some(timestamp()),
        fresh_until: Some(timestamp() + Duration::seconds(75)),
        last_known_good_age_seconds: Some(90),
        uncertainty: Some(OpenUncertaintyReason::LatestAttemptFailed),
    }
}

fn http() -> OpenHttpMetadataV1 {
    OpenHttpMetadataV1 {
        status_code: Some(200),
        decoded_bytes: Some(42),
        decoded_body_sha256: Some(body_hash()),
        content_type: Some(header("application/json")),
        etag: Some(header("\"open-v7\"")),
        cache_control: Some(header("no-cache")),
        date: Some(header("Tue, 14 Jul 2026 00:00:01 GMT")),
        age_seconds: Some(0),
        last_modified: Some(header("Mon, 13 Jul 2026 23:59:00 GMT")),
        retry_after: None,
    }
}

fn reconcile_counts() -> OpenReconcileCountsV1 {
    OpenReconcileCountsV1 {
        canonical_open_indexes: 3,
        duplicate_indexes: 1,
        catalog_sections: 10,
        matched_open_sections: 2,
        closed_sections: 8,
        orphan_indexes: 1,
    }
}

fn schedule() -> OpenAttemptScheduleV1 {
    OpenAttemptScheduleV1 {
        lane: OpenSchedulerLane::ActiveWatch,
        requested_general_interval_seconds: 30,
        requested_effective_interval_seconds: 10,
        due_at: timestamp(),
        scheduler_lag_milliseconds: 250,
        actual_start_to_start_interval_milliseconds: Some(10_250),
    }
}

fn attempt_point() -> OpenAttemptPointV1 {
    OpenAttemptPointV1 {
        attempt_sequence: sequence(),
        started_at: timestamp(),
        completed_at: Some(timestamp() + Duration::seconds(1)),
        classification: OpenRefreshClassification::Failed,
        failure_class: Some(OpenFailureClass::Http5xx),
    }
}

fn observation_point() -> OpenObservationPointV1 {
    OpenObservationPointV1 {
        observation_id: trace_id(),
        observation_sequence: sequence(),
        catalog_content_version: version(),
        observed_at: timestamp(),
        canonical_set_hash: set_hash(),
        state_hash: state_hash(),
    }
}

fn circuit() -> OpenCircuitStatusV1 {
    OpenCircuitStatusV1 {
        state: OpenCircuitState::RetryAfter,
        reason: Some(OpenCircuitReason::RateLimited),
        opened_at: Some(timestamp()),
        retry_at: Some(timestamp() + Duration::minutes(15)),
        diagnostic_recheck_required: false,
    }
}

fn scheduler() -> OpenSchedulerStatusV1 {
    OpenSchedulerStatusV1 {
        lane: OpenSchedulerLane::ActiveWatch,
        requested_general_interval_seconds: 30,
        requested_effective_interval_seconds: 10,
        active_watch_count: 2,
        next_due_at: Some(timestamp() + Duration::seconds(10)),
        in_flight: false,
        scheduler_lag_milliseconds: 250,
        actual_start_to_start_interval_milliseconds: Some(10_250),
        failure_streak: 1,
    }
}

#[test]
fn open_enum_manifest_values_bind_to_serde_names() {
    assert_enum_binding(
        "bcsp.open.refresh-classification.v1",
        OpenRefreshClassification::ALL,
    );
    assert_enum_binding(
        "bcsp.open.applied-classification.v1",
        OpenAppliedClassification::ALL,
    );
    assert_enum_binding("bcsp.open.failure-class.v1", OpenFailureClass::ALL);
    assert_enum_binding("bcsp.open.state.v1", OpenState::ALL);
    assert_enum_binding("bcsp.open.freshness-state.v1", OpenFreshnessState::ALL);
    assert_enum_binding(
        "bcsp.open.uncertainty-reason.v1",
        OpenUncertaintyReason::ALL,
    );
    assert_enum_binding("bcsp.open.scheduler-lane.v1", OpenSchedulerLane::ALL);
    assert_enum_binding("bcsp.open.circuit-state.v1", OpenCircuitState::ALL);
    assert_enum_binding("bcsp.open.circuit-reason.v1", OpenCircuitReason::ALL);
    assert_enum_binding("bcsp.open.rutgers-day-timezone.v1", RutgersDayTimezone::ALL);
}

#[test]
fn open_object_manifest_fields_bind_to_serde_shapes() {
    assert_object_binding("bcsp.open.batch-key.v1", &batch());
    assert_object_binding("bcsp.open.pull-counts.v1", &counts());
    assert_object_binding("bcsp.open.counter-snapshot.v1", &counter_snapshot());
    assert_object_binding("bcsp.open.freshness.v1", &freshness());
    assert_object_binding("bcsp.open.http-metadata.v1", &http());
    assert_object_binding("bcsp.open.reconcile-counts.v1", &reconcile_counts());
    assert_object_binding("bcsp.open.attempt-schedule.v1", &schedule());

    let attempt = OpenPullAttemptV1 {
        contract_version: OPEN_CONTRACT_VERSION,
        attempt_id: trace_id(),
        batch: batch(),
        attempt_sequence: sequence(),
        catalog_content_version: version(),
        started_at: timestamp(),
        completed_at: Some(timestamp() + Duration::seconds(1)),
        classification: OpenRefreshClassification::ValidApplied,
        failure_class: None,
        http: http(),
        canonical_set_hash: Some(set_hash()),
        state_hash: Some(state_hash()),
        counts: reconcile_counts(),
        schedule: schedule(),
        last_known_good_age_seconds: Some(0),
    };
    assert_object_binding("bcsp.open.pull-attempt.v1", &attempt);

    let refresh = OpenRefreshObservationV1 {
        contract_version: OPEN_CONTRACT_VERSION,
        observation_id: trace_id(),
        batch: batch(),
        attempt_sequence: sequence(),
        observation_sequence: sequence(),
        catalog_content_version: version(),
        started_at: timestamp(),
        completed_at: timestamp() + Duration::seconds(1),
        observed_at: timestamp() + Duration::seconds(1),
        classification: OpenAppliedClassification::ValidApplied,
        http: http(),
        canonical_set_hash: set_hash(),
        state_hash: state_hash(),
        counts: reconcile_counts(),
        schedule: schedule(),
        body_changed: true,
        state_changed: true,
        last_known_good_age_seconds: 0,
    };
    assert_object_binding("bcsp.open.refresh-observation.v1", &refresh);

    let observation = OpenObservationV1::try_new(
        OPEN_CONTRACT_VERSION,
        trace_id(),
        trace_id(),
        batch(),
        section_key(),
        sequence(),
        version(),
        OpenState::Open,
        timestamp(),
        timestamp() + Duration::seconds(35),
        250,
        counter_snapshot(),
    )
    .unwrap();
    assert_object_binding("bcsp.open.observation.v1", &observation);
    assert_object_binding("bcsp.open.attempt-point.v1", &attempt_point());
    assert_object_binding(
        "bcsp.open.failure-point.v1",
        &OpenFailurePointV1 {
            attempt_sequence: sequence(),
            failed_at: timestamp(),
            failure_class: OpenFailureClass::Http5xx,
        },
    );
    assert_object_binding("bcsp.open.observation-point.v1", &observation_point());
    assert_object_binding("bcsp.open.circuit-status.v1", &circuit());
    assert_object_binding("bcsp.open.scheduler-status.v1", &scheduler());

    let status = OpenRefreshStatusV1 {
        contract_version: OPEN_CONTRACT_VERSION,
        batch: batch(),
        catalog_content_version: Some(version()),
        latest_attempt: Some(attempt_point()),
        latest_failure: Some(OpenFailurePointV1 {
            attempt_sequence: sequence(),
            failed_at: timestamp(),
            failure_class: OpenFailureClass::Http5xx,
        }),
        last_valid_observation: Some(observation_point()),
        last_body_change_at: Some(timestamp()),
        last_state_change_at: Some(timestamp()),
        freshness: freshness(),
        scheduler: scheduler(),
        circuit: circuit(),
        counter_snapshot: counter_snapshot(),
    };
    assert_object_binding("bcsp.open.refresh-status.v1", &status);

    let section_status = OpenSectionStatusV1 {
        contract_version: OPEN_CONTRACT_VERSION,
        section_key: section_key(),
        state: OpenState::Open,
        last_observation_id: Some(trace_id()),
        catalog_content_version: Some(version()),
        freshness: freshness(),
        scheduler_lag_milliseconds: 250,
        counter_snapshot: counter_snapshot(),
    };
    assert_object_binding("bcsp.open.section-status.v1", &section_status);
    assert_object_binding(
        "bcsp.open.status-request.v1",
        &OpenStatusRequestV1::new(batch()),
    );
    assert_object_binding(
        "bcsp.open.section-status-request.v1",
        &OpenSectionStatusRequestV1::new(section_key()),
    );
}

#[test]
fn public_run_counts_field_is_documented_as_optional_presence() {
    let schema = schema("bcsp.open.counter-snapshot.v1");
    let field = schema
        .fields
        .iter()
        .find(|field| field.name == "runCounts")
        .unwrap();
    assert!(!field.required);
    assert_eq!(field.type_ref, "$schema:bcsp.open.pull-counts.v1");
}
