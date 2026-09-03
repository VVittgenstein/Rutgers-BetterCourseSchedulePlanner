use std::str::FromStr;

use bcsp_contracts::{
    CatalogContentVersion, MAX_OPEN_BATCH_STATUS_SECTIONS, OPEN_CONTRACT_VERSION,
    OpenAppliedClassification, OpenBatchKey, OpenBatchStatusRequestV1, OpenCanonicalSetHash,
    OpenContractVersion, OpenCounterSnapshotV1, OpenDecodedBodySha256, OpenHttpHeaderValue,
    OpenHttpMetadataV1, OpenObservationV1, OpenPullCountsV1, OpenRefreshClassification,
    OpenRefreshObservationV1, OpenSectionStatusRequestV1, OpenSequence, OpenStateHash,
    OpenStatusRequestV1, RutgersDay, RutgersDayTimezone, SectionKey, TermCampusKey, TraceId,
};
use time::OffsetDateTime;

fn trace_id() -> TraceId {
    TraceId::from_str("00000000-0000-4000-8000-000000000001").unwrap()
}

#[test]
fn open_contract_version_and_sequences_fail_closed() {
    assert_eq!(OPEN_CONTRACT_VERSION.as_u16(), 1);
    assert_eq!(OpenContractVersion::try_from(1), Ok(OPEN_CONTRACT_VERSION));
    assert_eq!(OpenContractVersion::try_from(0).unwrap_err().observed(), 0);
    assert_eq!(OpenContractVersion::try_from(2).unwrap_err().observed(), 2);
    assert!(serde_json::from_str::<OpenContractVersion>("0").is_err());
    assert!(serde_json::from_str::<OpenContractVersion>("2").is_err());

    assert!(OpenSequence::try_from(0).is_err());
    assert_eq!(OpenSequence::try_from(1).unwrap().get(), 1);
}

#[test]
fn open_batch_key_is_exactly_one_catalog_target_identity() {
    let target = TermCampusKey::try_new("T2026F", "CAMPUS_A").unwrap();
    let batch = OpenBatchKey::from(target.clone());
    assert_eq!(batch.term().as_str(), "T2026F");
    assert_eq!(batch.campus().as_str(), "CAMPUS_A");
    assert_eq!(batch.target(), target);
    assert_eq!(TermCampusKey::from(batch.clone()), target);
    assert_eq!(batch.to_string(), "T2026F/CAMPUS_A");
    assert_eq!(
        serde_json::to_value(batch).unwrap(),
        serde_json::json!({"term":"T2026F","campus":"CAMPUS_A"})
    );
}

#[test]
fn open_hashes_and_rutgers_day_are_validated_at_decode() {
    assert!(OpenCanonicalSetHash::try_from("a".repeat(64)).is_ok());
    assert!(OpenStateHash::try_from("0".repeat(64)).is_ok());
    assert!(OpenCanonicalSetHash::try_from("A".repeat(64)).is_err());
    assert!(OpenStateHash::try_from("g".repeat(64)).is_err());
    assert!(OpenStateHash::try_from("a".repeat(63)).is_err());

    assert!(RutgersDay::try_from("2028-02-29").is_ok());
    assert!(RutgersDay::try_from("2027-02-29").is_err());
    assert!(RutgersDay::try_from("2026-13-01").is_err());
    assert!(RutgersDay::try_from("2026-07-14x").is_err());
}

#[test]
fn failed_and_unsafe_attempts_cannot_decode_as_valid_observation_classifications() {
    for classification in [
        OpenRefreshClassification::UnsafeEmpty,
        OpenRefreshClassification::UnsafeZeroIntersection,
        OpenRefreshClassification::StaleCatalogRace,
        OpenRefreshClassification::Failed,
    ] {
        let encoded = serde_json::to_string(&classification).unwrap();
        assert!(serde_json::from_str::<OpenAppliedClassification>(&encoded).is_err());
    }
}

#[test]
fn valid_observation_contract_has_no_failure_or_unknown_attempt_escape_hatch() {
    let value = serde_json::json!({
        "contractVersion": 1,
        "observationId": trace_id(),
        "batch": {"term":"T2026F","campus":"CAMPUS_A"},
        "attemptSequence": 1,
        "observationSequence": 1,
        "catalogContentVersion": 1,
        "startedAt": "2026-07-14T00:00:00Z",
        "completedAt": "2026-07-14T00:00:01Z",
        "observedAt": "2026-07-14T00:00:01Z",
        "classification": "FAILED",
        "http": {
            "statusCode":500,"decodedBytes":0,"decodedBodySha256":null,
            "contentType":null,"etag":null,"cacheControl":null,"date":null,
            "ageSeconds":null,"lastModified":null,"retryAfter":null
        },
        "canonicalSetHash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "stateHash": "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "counts": {
            "canonicalOpenIndexes":0,"duplicateIndexes":0,"catalogSections":1,
            "matchedOpenSections":0,"closedSections":0,"orphanIndexes":0
        },
        "schedule": {
            "lane":"GENERAL","requestedGeneralIntervalSeconds":30,
            "requestedEffectiveIntervalSeconds":30,"dueAt":"2026-07-14T00:00:00Z",
            "schedulerLagMilliseconds":0,"actualStartToStartIntervalMilliseconds":null
        },
        "bodyChanged":false,"stateChanged":false,"lastKnownGoodAgeSeconds":30
    });
    assert!(serde_json::from_value::<OpenRefreshObservationV1>(value).is_err());
}

#[test]
fn client_requests_reject_additions_but_server_projections_accept_them() {
    let request = OpenStatusRequestV1::new(OpenBatchKey::try_new("T2026F", "CAMPUS_A").unwrap());
    let mut encoded = serde_json::to_value(request).unwrap();
    encoded["futureClientField"] = serde_json::json!(true);
    assert!(serde_json::from_value::<OpenStatusRequestV1>(encoded).is_err());

    let section_request = OpenSectionStatusRequestV1::new(
        SectionKey::try_new("T2026F", "CAMPUS_A", "00001").unwrap(),
    );
    let mut encoded = serde_json::to_value(section_request).unwrap();
    encoded["futureClientField"] = serde_json::json!(true);
    assert!(serde_json::from_value::<OpenSectionStatusRequestV1>(encoded).is_err());

    let section = SectionKey::try_new("T2026F", "CAMPUS_A", "00001").unwrap();
    let batch_request = OpenBatchStatusRequestV1::new(
        OpenBatchKey::try_new("T2026F", "CAMPUS_A").unwrap(),
        vec![section.clone()],
    );
    assert!(batch_request.is_valid());
    let mut encoded = serde_json::to_value(batch_request).unwrap();
    encoded["futureClientField"] = serde_json::json!(true);
    assert!(serde_json::from_value::<OpenBatchStatusRequestV1>(encoded).is_err());

    assert!(
        !OpenBatchStatusRequestV1::new(
            OpenBatchKey::try_new("T2026F", "CAMPUS_A").unwrap(),
            Vec::new(),
        )
        .is_valid()
    );
    assert!(
        !OpenBatchStatusRequestV1::new(
            OpenBatchKey::try_new("T2026F", "CAMPUS_A").unwrap(),
            vec![section.clone(), section],
        )
        .is_valid()
    );
    assert!(
        !OpenBatchStatusRequestV1::new(
            OpenBatchKey::try_new("T2026F", "CAMPUS_A").unwrap(),
            vec![SectionKey::try_new("T2026F", "CAMPUS_B", "00001").unwrap()],
        )
        .is_valid()
    );

    let over_limit = (0..=MAX_OPEN_BATCH_STATUS_SECTIONS)
        .map(|index| {
            SectionKey::try_new("T2026F", "CAMPUS_A", &format!("{:05}", index + 1)).unwrap()
        })
        .collect();
    assert!(
        !OpenBatchStatusRequestV1::new(
            OpenBatchKey::try_new("T2026F", "CAMPUS_A").unwrap(),
            over_limit,
        )
        .is_valid()
    );

    let counts: OpenPullCountsV1 = serde_json::from_value(serde_json::json!({
        "attempted":4,"succeeded":2,"failed":2,"empty":1,"futureServerField":true
    }))
    .unwrap();
    assert_eq!(counts.attempted, 4);
}

#[test]
fn public_counter_snapshot_may_omit_run_counts() {
    let snapshot: OpenCounterSnapshotV1 = serde_json::from_value(serde_json::json!({
        "todayCounts":{"attempted":4,"succeeded":3,"failed":1,"empty":1},
        "rutgersDay":"2026-07-14",
        "dayTimezone":"America/New_York"
    }))
    .unwrap();
    assert_eq!(snapshot.run_counts, None);
    assert_eq!(snapshot.day_timezone, RutgersDayTimezone::AmericaNewYork);
    assert!(
        !serde_json::to_value(snapshot)
            .unwrap()
            .as_object()
            .unwrap()
            .contains_key("runCounts")
    );
}

#[test]
fn section_observation_rejects_a_batch_from_another_target() {
    let golden = include_str!("golden/open-observation-v1.json");
    let valid: OpenObservationV1 = serde_json::from_str(golden).unwrap();
    assert_eq!(valid.batch().target(), valid.section_key().target());

    let construction_error = OpenObservationV1::try_new(
        valid.contract_version,
        valid.observation_id,
        valid.refresh_observation_id,
        OpenBatchKey::try_new("T2026F", "CAMPUS_B").unwrap(),
        valid.section_key().clone(),
        valid.pull_sequence,
        valid.catalog_content_version,
        valid.state,
        valid.observed_at,
        valid.fresh_until,
        valid.scheduler_lag_milliseconds,
        valid.counter_snapshot.clone(),
    )
    .unwrap_err();
    assert_eq!(
        construction_error.to_string(),
        "Open observation batch must match the Section target"
    );

    let mut value: serde_json::Value = serde_json::from_str(golden).unwrap();
    value["batch"]["campus"] = serde_json::json!("CAMPUS_B");

    let error = serde_json::from_value::<OpenObservationV1>(value).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("batch must match the Section target")
    );
}

#[test]
fn http_audit_metadata_rejects_noncanonical_values() {
    assert!(OpenDecodedBodySha256::try_from("a".repeat(64)).is_ok());
    assert!(OpenDecodedBodySha256::try_from("A".repeat(64)).is_err());
    assert!(OpenHttpHeaderValue::try_from("\"open-v7\"").is_ok());
    assert!(OpenHttpHeaderValue::try_from(" leading-space").is_err());
    assert!(OpenHttpHeaderValue::try_from("line\r\nbreak").is_err());
    assert!(OpenHttpHeaderValue::try_from("x".repeat(4_097)).is_err());

    let invalid_status = serde_json::json!({
        "statusCode": 99,
        "decodedBytes": null,
        "decodedBodySha256": null,
        "contentType": null,
        "etag": null,
        "cacheControl": null,
        "date": null,
        "ageSeconds": null,
        "lastModified": null,
        "retryAfter": null
    });
    assert!(serde_json::from_value::<OpenHttpMetadataV1>(invalid_status).is_err());
}

#[test]
fn timestamp_and_catalog_version_dependencies_remain_strict() {
    let timestamp = OffsetDateTime::from_unix_timestamp(0).unwrap();
    assert_eq!(timestamp.unix_timestamp(), 0);
    assert_eq!(CatalogContentVersion::try_from(1).unwrap().get(), 1);
}
