use std::str::FromStr;

use bcsp_contracts::{
    CatalogContentVersion, OPEN_CONTRACT_VERSION, OpenAppliedClassification, OpenAttemptPointV1,
    OpenAttemptScheduleV1, OpenBatchKey, OpenCanonicalSetHash, OpenCircuitState,
    OpenCircuitStatusV1, OpenCounterSnapshotV1, OpenDecodedBodySha256, OpenFailureClass,
    OpenFailurePointV1, OpenFreshnessState, OpenFreshnessV1, OpenHttpHeaderValue,
    OpenHttpMetadataV1, OpenObservationPointV1, OpenObservationV1, OpenPullAttemptV1,
    OpenPullCountsV1, OpenReconcileCountsV1, OpenRefreshClassification, OpenRefreshObservationV1,
    OpenRefreshStatusV1, OpenSchedulerLane, OpenSchedulerStatusV1, OpenSequence, OpenState,
    OpenStateHash, RutgersDay, RutgersDayTimezone, SectionKey, TraceId,
};
use serde::Serialize;
use time::{Duration, OffsetDateTime};

fn pretty<T: Serialize>(value: &T) -> String {
    format!("{}\n", serde_json::to_string_pretty(value).unwrap())
}

fn canonical_golden(expected: &str) -> String {
    expected.replace("\r\n", "\n")
}

fn timestamp() -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH
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

fn pull_counts() -> OpenPullCountsV1 {
    OpenPullCountsV1 {
        attempted: 9,
        succeeded: 7,
        failed: 2,
        empty: 1,
    }
}

fn counters() -> OpenCounterSnapshotV1 {
    OpenCounterSnapshotV1 {
        run_counts: Some(pull_counts()),
        today_counts: pull_counts(),
        rutgers_day: RutgersDay::try_from("2026-07-14").unwrap(),
        day_timezone: RutgersDayTimezone::AmericaNewYork,
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

fn failed_attempt() -> OpenPullAttemptV1 {
    OpenPullAttemptV1 {
        contract_version: OPEN_CONTRACT_VERSION,
        attempt_id: trace_id(),
        batch: batch(),
        attempt_sequence: sequence(),
        catalog_content_version: version(),
        started_at: timestamp(),
        completed_at: Some(timestamp() + Duration::seconds(1)),
        classification: OpenRefreshClassification::Failed,
        failure_class: Some(OpenFailureClass::Http5xx),
        http: OpenHttpMetadataV1 {
            status_code: Some(503),
            decoded_bytes: Some(0),
            decoded_body_sha256: Some(body_hash()),
            content_type: Some(header("text/plain")),
            etag: None,
            cache_control: Some(header("no-cache")),
            date: Some(header("Tue, 14 Jul 2026 00:00:01 GMT")),
            age_seconds: Some(0),
            last_modified: None,
            retry_after: Some(header("120")),
        },
        canonical_set_hash: None,
        state_hash: None,
        counts: OpenReconcileCountsV1 {
            canonical_open_indexes: 0,
            duplicate_indexes: 0,
            catalog_sections: 10,
            matched_open_sections: 0,
            closed_sections: 0,
            orphan_indexes: 0,
        },
        schedule: schedule(),
        last_known_good_age_seconds: Some(60),
    }
}

fn refresh_observation() -> OpenRefreshObservationV1 {
    OpenRefreshObservationV1 {
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
        http: OpenHttpMetadataV1 {
            status_code: Some(200),
            decoded_bytes: Some(42),
            decoded_body_sha256: Some(body_hash()),
            content_type: Some(header("application/json;charset=UTF-8")),
            etag: Some(header("\"open-v7\"")),
            cache_control: Some(header("max-age=0, no-cache")),
            date: Some(header("Tue, 14 Jul 2026 00:00:01 GMT")),
            age_seconds: Some(12),
            last_modified: Some(header("Mon, 13 Jul 2026 23:59:00 GMT")),
            retry_after: None,
        },
        canonical_set_hash: set_hash(),
        state_hash: state_hash(),
        counts: reconcile_counts(),
        schedule: schedule(),
        body_changed: true,
        state_changed: false,
        last_known_good_age_seconds: 0,
    }
}

fn observation() -> OpenObservationV1 {
    OpenObservationV1::try_new(
        OPEN_CONTRACT_VERSION,
        trace_id(),
        trace_id(),
        batch(),
        section_key(),
        sequence(),
        version(),
        OpenState::Open,
        timestamp() + Duration::seconds(1),
        timestamp() + Duration::seconds(36),
        250,
        counters(),
    )
    .unwrap()
}

fn refresh_status() -> OpenRefreshStatusV1 {
    OpenRefreshStatusV1 {
        contract_version: OPEN_CONTRACT_VERSION,
        batch: batch(),
        catalog_content_version: Some(version()),
        latest_attempt: Some(OpenAttemptPointV1 {
            attempt_sequence: sequence(),
            started_at: timestamp(),
            completed_at: Some(timestamp() + Duration::seconds(1)),
            classification: OpenRefreshClassification::ValidApplied,
            failure_class: None,
        }),
        latest_failure: Some(OpenFailurePointV1 {
            attempt_sequence: OpenSequence::try_from(6).unwrap(),
            failed_at: timestamp() - Duration::seconds(60),
            failure_class: OpenFailureClass::Http5xx,
        }),
        last_valid_observation: Some(OpenObservationPointV1 {
            observation_id: trace_id(),
            observation_sequence: sequence(),
            catalog_content_version: version(),
            observed_at: timestamp() + Duration::seconds(1),
            canonical_set_hash: set_hash(),
            state_hash: state_hash(),
        }),
        last_body_change_at: Some(timestamp() + Duration::seconds(1)),
        last_state_change_at: Some(timestamp() - Duration::seconds(180)),
        freshness: OpenFreshnessV1 {
            state: OpenFreshnessState::Fresh,
            observed_at: Some(timestamp() + Duration::seconds(1)),
            fresh_until: Some(timestamp() + Duration::seconds(36)),
            last_known_good_age_seconds: Some(0),
            uncertainty: None,
        },
        scheduler: OpenSchedulerStatusV1 {
            lane: OpenSchedulerLane::ActiveWatch,
            requested_general_interval_seconds: 30,
            requested_effective_interval_seconds: 10,
            active_watch_count: 2,
            next_due_at: Some(timestamp() + Duration::seconds(30)),
            in_flight: false,
            scheduler_lag_milliseconds: 250,
            actual_start_to_start_interval_milliseconds: Some(10_250),
            failure_streak: 0,
        },
        circuit: OpenCircuitStatusV1 {
            state: OpenCircuitState::Closed,
            reason: None,
            opened_at: None,
            retry_at: None,
            diagnostic_recheck_required: false,
        },
        counter_snapshot: counters(),
    }
}

#[test]
fn failed_attempt_wire_is_exact_and_round_trips() {
    let value = failed_attempt();
    let golden = include_str!("golden/open-pull-attempt-v1.json");
    assert_eq!(pretty(&value), canonical_golden(golden));
    assert_eq!(
        serde_json::from_str::<OpenPullAttemptV1>(golden).unwrap(),
        value
    );
}

#[test]
fn valid_refresh_observation_wire_is_exact_and_round_trips() {
    let value = refresh_observation();
    let golden = include_str!("golden/open-refresh-observation-v1.json");
    assert_eq!(pretty(&value), canonical_golden(golden));
    assert_eq!(
        serde_json::from_str::<OpenRefreshObservationV1>(golden).unwrap(),
        value
    );
}

#[test]
fn section_observation_event_wire_is_exact_and_round_trips() {
    let value = observation();
    let golden = include_str!("golden/open-observation-v1.json");
    assert_eq!(pretty(&value), canonical_golden(golden));
    assert_eq!(
        serde_json::from_str::<OpenObservationV1>(golden).unwrap(),
        value
    );
}

#[test]
fn status_preserves_latest_failure_independently_from_latest_attempt() {
    let value = refresh_status();
    let golden = include_str!("golden/open-refresh-status-v1.json");
    assert_eq!(pretty(&value), canonical_golden(golden));
    assert_eq!(
        serde_json::from_str::<OpenRefreshStatusV1>(golden).unwrap(),
        value
    );
}
