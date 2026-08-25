use bcsp_contracts::{CourseGroupKey, CourseVariantKey, SectionKey, TermCampusKey, TraceId};
use bcsp_operational_storage::{
    BeginOpenPullAttemptCommand, CatalogRefreshCommand, CatalogSnapshot, EmptySnapshotDecision,
    FinishOpenPullFailureCommand, FinishOpenPullSuccessCommand, OpenAttemptClassification,
    OpenCacheStatus, OpenCircuitState, OpenHttpAuditMetadata, OpenOriginState, OpenRequestLane,
    OpenScheduleState, OpenSectionState, OperationalStorage, StorageError, StoredCourseGroup,
    StoredCourseVariant, StoredSection, catalog_content_sha256_v1,
};
use rusqlite::Connection;
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Barrier};
use tempfile::TempDir;

const STARTED: &str = "2026-07-14T00:00:00Z";
const COMPLETED: &str = "2026-07-14T00:00:01Z";
const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn target(term: &str, campus: &str) -> TermCampusKey {
    TermCampusKey::try_new(term, campus).expect("synthetic target")
}

fn section(target: &TermCampusKey, index: &str) -> SectionKey {
    SectionKey::try_new(target.term().as_str(), target.campus().as_str(), index)
        .expect("synthetic section")
}

fn trace(label: &str) -> TraceId {
    let digest = Sha256::digest(label.as_bytes());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    )
    .parse()
    .expect("deterministic UUIDv4")
}

fn catalog_snapshot(
    target: &TermCampusKey,
    indices: &[&str],
    canonical_sha256: &str,
) -> CatalogSnapshot {
    let group = CourseGroupKey::try_new(
        target.term().as_str(),
        target.campus().as_str(),
        "SYN:OPEN:001",
    )
    .expect("group");
    let variant =
        CourseVariantKey::try_new(group.clone(), &format!("v1:{HASH_A}")).expect("variant");
    CatalogSnapshot {
        course_groups: vec![StoredCourseGroup {
            key: group,
            canonical_facts: json!({"fixture": "open-storage"}),
        }],
        course_variants: vec![StoredCourseVariant {
            key: variant.clone(),
            subject_code: Some("SYN".to_owned()),
            course_number: Some("001".to_owned()),
            title: Some("Synthetic Open Storage".to_owned()),
            description: None,
            credits_summary: None,
            supplement: None,
            search_document: "SYN:OPEN:001 synthetic open storage".to_owned(),
            canonical_sha256: canonical_sha256.to_owned(),
            raw_multiplicity: 1,
            canonical_facts: json!({"fixture": "open-storage"}),
        }],
        sections: indices
            .iter()
            .map(|index| StoredSection {
                key: section(target, index),
                variant_key: variant.clone(),
                section_number: None,
                catalog_status: None,
                section_course_type: None,
                delivery_modality: "UNKNOWN".to_owned(),
                synchronicity: "UNKNOWN".to_owned(),
                canonical_facts: json!({"index": index}),
                canonical_sha256: HASH_C.to_owned(),
            })
            .collect(),
        occurrences: Vec::new(),
        provenance: Vec::new(),
    }
}

fn publish_catalog(
    storage: &mut OperationalStorage,
    target: &TermCampusKey,
    label: &str,
    indices: &[&str],
    canonical_sha256: &str,
) -> u64 {
    let snapshot = catalog_snapshot(target, indices, canonical_sha256);
    let body = format!("synthetic-{label}");
    let source_content_sha256 = format!("{:x}", Sha256::digest(body.as_bytes()));
    let semantic_content_sha256 =
        catalog_content_sha256_v1(target, &snapshot).expect("catalog hash");
    let outcome = storage
        .apply_catalog_refresh(
            CatalogRefreshCommand {
                observation_id: trace(&format!("catalog-{label}")),
                target: target.clone(),
                started_at: STARTED.to_owned(),
                completed_at: COMPLETED.to_owned(),
                source_content_sha256,
                semantic_content_sha256,
                source_bytes: body.len() as u64,
                raw_payload: None,
                snapshot,
            },
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
        )
        .expect("publish catalog");
    match outcome {
        bcsp_operational_storage::PublishOutcome::AppliedChanged {
            content_version, ..
        }
        | bcsp_operational_storage::PublishOutcome::AppliedUnchanged {
            content_version, ..
        } => content_version,
        other => panic!("unexpected catalog outcome: {other:?}"),
    }
}

fn stage_catalog(
    storage: &mut OperationalStorage,
    target: &TermCampusKey,
    label: &str,
    indices: &[&str],
    canonical_sha256: &str,
) -> TraceId {
    let snapshot = catalog_snapshot(target, indices, canonical_sha256);
    let body = format!("synthetic-{label}");
    let observation_id = trace(&format!("catalog-{label}"));
    storage
        .stage_catalog_refresh(CatalogRefreshCommand {
            observation_id,
            target: target.clone(),
            started_at: STARTED.to_owned(),
            completed_at: COMPLETED.to_owned(),
            source_content_sha256: format!("{:x}", Sha256::digest(body.as_bytes())),
            semantic_content_sha256: catalog_content_sha256_v1(target, &snapshot)
                .expect("catalog hash"),
            source_bytes: body.len() as u64,
            raw_payload: None,
            snapshot,
        })
        .expect("stage catalog");
    observation_id
}

fn begin(
    storage: &mut OperationalStorage,
    target: &TermCampusKey,
    attempt: &str,
    run: &str,
    version: u64,
    day: &str,
) {
    storage
        .begin_open_pull_attempt(&begin_command(target, attempt, run, version, day))
        .expect("begin Open attempt");
}

fn begin_command(
    target: &TermCampusKey,
    attempt: &str,
    run: &str,
    version: u64,
    day: &str,
) -> BeginOpenPullAttemptCommand {
    BeginOpenPullAttemptCommand {
        attempt_id: trace(attempt),
        run_id: trace(run),
        target: target.clone(),
        captured_catalog_content_version: version,
        rutgers_day: day.to_owned(),
        started_at: STARTED.to_owned(),
        lane: OpenRequestLane::General,
        requested_interval_seconds: Some(30),
        effective_interval_seconds: Some(45),
        schedule_lag_ms: Some(12),
    }
}

fn success_http(decoded_bytes: u64) -> OpenHttpAuditMetadata {
    OpenHttpAuditMetadata {
        http_status: Some(200),
        cache_status: Some(OpenCacheStatus::Miss),
        decoded_bytes: Some(decoded_bytes),
        decoded_body_sha256: Some(HASH_A.to_owned()),
        content_type: Some("application/json; charset=utf-8".to_owned()),
        etag: Some("\"synthetic-etag\"".to_owned()),
        cache_control: Some("max-age=30".to_owned()),
        date: Some("Tue, 14 Jul 2026 00:00:01 GMT".to_owned()),
        age_seconds: Some(7),
        last_modified: Some("Mon, 13 Jul 2026 23:59:00 GMT".to_owned()),
        retry_after: None,
        retry_after_seconds: None,
    }
}

fn failure_http() -> OpenHttpAuditMetadata {
    OpenHttpAuditMetadata {
        http_status: Some(429),
        cache_status: Some(OpenCacheStatus::NotApplicable),
        decoded_bytes: Some(21),
        decoded_body_sha256: Some(HASH_B.to_owned()),
        content_type: Some("application/json".to_owned()),
        etag: Some("\"failure-etag\"".to_owned()),
        cache_control: Some("no-cache".to_owned()),
        date: Some("Tue, 14 Jul 2026 00:00:01 GMT".to_owned()),
        age_seconds: Some(0),
        last_modified: None,
        retry_after: Some("120".to_owned()),
        retry_after_seconds: Some(120),
    }
}

#[test]
fn staged_catalog_and_open_publish_as_one_complete_snapshot() {
    let scope = target("92026", "NB");
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let catalog_observation =
        stage_catalog(&mut storage, &scope, "complete-initial", &["00001"], HASH_A);
    let candidate = storage
        .candidate_open_catalog_snapshot(
            &catalog_observation,
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
        )
        .expect("candidate")
        .expect("publishable candidate");
    assert_eq!(candidate.base_content_version, 0);
    assert_eq!(candidate.catalog.content_version, 1);
    let open_attempt = begin_command(
        &scope,
        "complete-open-initial",
        "complete-run",
        candidate.catalog.content_version,
        "2026-07-14",
    );
    storage
        .begin_candidate_open_pull_attempt(&candidate, &open_attempt)
        .expect("begin candidate Open");
    let outcome = storage
        .finish_candidate_open_pull_success(
            catalog_observation,
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
                attempt_id: open_attempt.attempt_id,
                completed_at: "2026-07-14T00:00:02Z".to_owned(),
                open_sections: vec![section(&scope, "00001")],
                source_value_count: 1,
                watched_sections: Vec::new(),
                http: success_http(16),
            },
        )
        .expect("atomic complete snapshot");
    assert!(outcome.catalog.is_some());
    assert!(outcome.open.classification.is_success());
    let state = storage
        .complete_target_snapshot_state(&scope)
        .expect("complete state");
    assert!(state.ready);
    assert_eq!(state.catalog_content_version, 1);
    assert_eq!(state.open_catalog_content_version, 1);
}

#[test]
fn failed_candidate_open_retains_the_previous_complete_pair() {
    let scope = target("92026", "NB");
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let initial_catalog = stage_catalog(
        &mut storage,
        &scope,
        "complete-retained-v1",
        &["00001"],
        HASH_A,
    );
    let initial_candidate = storage
        .candidate_open_catalog_snapshot(
            &initial_catalog,
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
        )
        .expect("candidate")
        .expect("publishable");
    let first_attempt = begin_command(
        &scope,
        "complete-retained-open-v1",
        "complete-retained-run",
        1,
        "2026-07-14",
    );
    storage
        .begin_candidate_open_pull_attempt(&initial_candidate, &first_attempt)
        .expect("begin initial");
    storage
        .finish_candidate_open_pull_success(
            initial_catalog,
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
                attempt_id: first_attempt.attempt_id,
                completed_at: "2026-07-14T00:00:02Z".to_owned(),
                open_sections: vec![section(&scope, "00001")],
                source_value_count: 1,
                watched_sections: Vec::new(),
                http: success_http(16),
            },
        )
        .expect("initial complete snapshot");

    let replacement_catalog = stage_catalog(
        &mut storage,
        &scope,
        "complete-retained-v2",
        &["00002"],
        HASH_B,
    );
    let replacement_candidate = storage
        .candidate_open_catalog_snapshot(
            &replacement_catalog,
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
        )
        .expect("candidate")
        .expect("publishable");
    assert_eq!(replacement_candidate.catalog.content_version, 2);
    let failed_attempt = begin_command(
        &scope,
        "complete-retained-open-v2",
        "complete-retained-run",
        2,
        "2026-07-14",
    );
    storage
        .begin_candidate_open_pull_attempt(&replacement_candidate, &failed_attempt)
        .expect("begin replacement");
    storage
        .finish_candidate_open_pull_failure(
            replacement_catalog,
            &FinishOpenPullFailureCommand {
                attempt_id: failed_attempt.attempt_id,
                completed_at: "2026-07-14T00:00:03Z".to_owned(),
                http: failure_http(),
                error_code: "OPEN_TRANSIENT_HTTP".to_owned(),
                diagnostic_token: None,
            },
        )
        .expect("record failed candidate");

    let state = storage
        .complete_target_snapshot_state(&scope)
        .expect("complete state");
    assert!(state.ready);
    assert_eq!(state.catalog_content_version, 1);
    assert_eq!(state.open_catalog_content_version, 1);
    let published = storage
        .published_catalog_snapshot(&scope)
        .expect("catalog read")
        .expect("retained catalog");
    assert_eq!(published.content_version, 1);
    assert_eq!(published.snapshot.sections[0].key.index().as_str(), "00001");
}

fn fail(storage: &mut OperationalStorage, attempt: &str) {
    storage
        .finish_open_pull_failure(&FinishOpenPullFailureCommand {
            attempt_id: trace(attempt),
            completed_at: COMPLETED.to_owned(),
            http: failure_http(),
            error_code: "OPEN_TRANSIENT_HTTP".to_owned(),
            diagnostic_token: Some("trace.synthetic".to_owned()),
        })
        .expect("finish Open failure");
}

#[test]
fn existing_v1_database_migrates_to_open_storage_without_seed_rows() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("operational.sqlite");
    let connection = Connection::open(&path).expect("raw database");
    let migration_v1 = include_str!("../migrations/0001_operational_catalog.sql");
    connection.execute_batch(migration_v1).expect("apply v1");
    connection
        .execute(
            "INSERT INTO bcsp_operational_migrations(migration_id, name, sha256, applied_at)
             VALUES (1, 'operational_catalog', ?1, ?2)",
            [
                format!("{:x}", Sha256::digest(migration_v1.as_bytes())),
                COMPLETED.to_owned(),
            ],
        )
        .expect("record v1");
    drop(connection);

    let storage = OperationalStorage::open(&path).expect("migrate to current schema");
    assert_eq!(storage.migration_records().expect("records").len(), 6);
    let tables = storage.operational_table_names().expect("tables");
    assert!(tables.contains(&"open_pull_attempts".to_owned()));
    assert!(tables.contains(&"open_section_events".to_owned()));
    let schema_connection = Connection::open(&path).expect("schema connection");
    let active_watch_column_count = schema_connection
        .query_row(
            "SELECT count(*) FROM pragma_table_info('open_schedule_state')
             WHERE name = 'active_watch_count'",
            [],
            |row| row.get::<_, i64>(0),
        )
        .expect("schedule columns");
    assert_eq!(active_watch_column_count, 0);
    assert!(
        storage
            .read_open_section_events(0, 10)
            .expect("events")
            .is_empty()
    );
}

#[test]
fn applied_lkg_accepts_typed_empty_then_survives_zero_intersection_race_and_restart() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("operational.sqlite");
    let scope = target("FALL_2026", "OPEN_LKG");
    let run = "open-run-lkg";
    let mut storage = OperationalStorage::open(&path).expect("storage");
    let v1 = publish_catalog(&mut storage, &scope, "lkg-v1", &["00001", "00002"], HASH_A);
    let catalog_for_open = storage
        .serving_open_catalog_snapshot(&scope)
        .expect("Open Catalog snapshot")
        .expect("published snapshot");
    assert_eq!(catalog_for_open.content_version, v1);
    assert_eq!(catalog_for_open.sections.len(), 2);

    begin(&mut storage, &scope, "open-lkg-1", run, v1, "2026-07-14");
    let applied = storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
            attempt_id: trace("open-lkg-1"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00001"), section(&scope, "99999")],
            source_value_count: 3,
            watched_sections: vec![section(&scope, "00001"), section(&scope, "00002")],
            http: success_http(42),
        })
        .expect("apply Open LKG");
    assert_eq!(
        applied.classification,
        OpenAttemptClassification::ValidApplied
    );
    assert_eq!((applied.duplicate_count, applied.orphan_count), (1, 1));
    assert!(applied.body_changed);
    assert!(applied.state_changed);
    let observation_commit = applied
        .observation_commit
        .as_ref()
        .expect("valid observation commit");
    assert_eq!(observation_commit.rutgers_day, "2026-07-14");
    assert_eq!(observation_commit.effective_interval_seconds, 45);
    assert_eq!(observation_commit.section_events.len(), 2);
    assert_eq!(observation_commit.run_counts.succeeded, 1);
    assert_eq!(observation_commit.target_day_counts.succeeded, 1);
    assert_eq!(observation_commit.service_day_counts.succeeded, 1);
    let refresh_observation_id = applied
        .refresh_observation_id
        .expect("refresh observation ID");
    let batch_observation = storage
        .open_batch_observation(&trace("open-lkg-1"))
        .expect("batch observation")
        .expect("present");
    assert_eq!(batch_observation.source_value_count, 3);
    assert_eq!(batch_observation.duplicate_count, 1);
    assert_eq!(batch_observation.observation_id, refresh_observation_id);
    assert_eq!(batch_observation.http.decoded_bytes, Some(42));
    assert_eq!(
        batch_observation.http.etag.as_deref(),
        Some("\"synthetic-etag\"")
    );
    assert_eq!(
        batch_observation.http.decoded_body_sha256.as_deref(),
        Some(HASH_A)
    );
    assert_eq!(
        batch_observation.http.content_type.as_deref(),
        Some("application/json; charset=utf-8")
    );
    assert_eq!(
        batch_observation.http.cache_control.as_deref(),
        Some("max-age=30")
    );
    assert_eq!(batch_observation.http.age_seconds, Some(7));
    assert!(batch_observation.body_changed);
    assert!(batch_observation.state_changed);
    let stored_attempt = storage
        .open_attempt(&trace("open-lkg-1"))
        .expect("attempt")
        .expect("present");
    assert_eq!(stored_attempt.http, batch_observation.http);
    let current = storage.open_section_current(&scope).expect("current");
    assert_eq!(current.len(), 2);
    assert_eq!(current[0].state, OpenSectionState::Open);
    assert_eq!(current[1].state, OpenSectionState::Closed);
    let events = storage.read_open_section_events(0, 10).expect("events");
    assert_eq!(events.len(), 2);
    assert_ne!(events[0].observation_id, events[1].observation_id);
    assert_eq!(events[0].refresh_observation_id, refresh_observation_id);
    assert_eq!(events[1].refresh_observation_id, refresh_observation_id);
    assert_eq!(
        storage.read_open_section_events(0, 10).expect("replay"),
        events
    );

    begin(&mut storage, &scope, "open-lkg-2", run, v1, "2026-07-14");
    fail(&mut storage, "open-lkg-2");
    assert_eq!(
        storage
            .open_attempt(&trace("open-lkg-2"))
            .expect("failed attempt")
            .expect("present")
            .http
            .retry_after_seconds,
        Some(120)
    );
    begin(&mut storage, &scope, "open-lkg-3", run, v1, "2026-07-14");
    let valid_empty = storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
            attempt_id: trace("open-lkg-3"),
            completed_at: COMPLETED.to_owned(),
            open_sections: Vec::new(),
            source_value_count: 0,
            watched_sections: Vec::new(),
            http: success_http(2),
        })
        .expect("apply typed empty response");
    assert_eq!(
        valid_empty.classification,
        OpenAttemptClassification::ValidApplied
    );
    assert_eq!(valid_empty.changed_section_count, 1);
    assert!(valid_empty.observation_commit.is_some());
    assert_eq!(
        storage
            .open_section_current(&scope)
            .expect("closed current state")
            .iter()
            .map(|section| section.state)
            .collect::<Vec<_>>(),
        vec![OpenSectionState::Closed, OpenSectionState::Closed]
    );
    begin(&mut storage, &scope, "open-lkg-4", run, v1, "2026-07-14");
    let zero = storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
            attempt_id: trace("open-lkg-4"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "99999")],
            source_value_count: 1,
            watched_sections: Vec::new(),
            http: success_http(9),
        })
        .expect("classify zero intersection");
    assert_eq!(
        zero.classification,
        OpenAttemptClassification::UnsafeZeroIntersection
    );

    begin(&mut storage, &scope, "open-lkg-5", run, v1, "2026-07-14");
    let v2 = publish_catalog(&mut storage, &scope, "lkg-v2", &["00001", "00002"], HASH_B);
    assert_eq!(v2, v1 + 1);
    let race = storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
            attempt_id: trace("open-lkg-5"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00002")],
            source_value_count: 1,
            watched_sections: vec![section(&scope, "00002")],
            http: success_http(9),
        })
        .expect("classify Catalog race");
    assert_eq!(
        race.classification,
        OpenAttemptClassification::StaleCatalogRace
    );
    assert!(race.observation_commit.is_none());
    assert_eq!(
        storage
            .open_batch_state(&scope)
            .expect("batch")
            .expect("state")
            .lkg_attempt_id,
        Some(trace("open-lkg-3"))
    );
    assert_eq!(
        storage.open_section_current(&scope).expect("LKG")[0].state,
        OpenSectionState::Closed
    );
    assert_eq!(
        storage
            .open_day_counters(&scope, "2026-07-14")
            .expect("day counters"),
        bcsp_operational_storage::OpenAttemptCounters {
            attempted: 5,
            succeeded: 2,
            failed: 3,
            empty: 1,
        }
    );
    drop(storage);

    let storage = OperationalStorage::open(&path).expect("restart");
    let batch = storage
        .open_batch_state(&scope)
        .expect("batch")
        .expect("state");
    assert_eq!(batch.lkg_attempt_id, Some(trace("open-lkg-3")));
    assert_eq!(batch.last_failure_attempt_sequence, Some(5));
    assert_eq!(batch.last_failure_at.as_deref(), Some(COMPLETED));
    assert_eq!(
        batch.last_failure_error_code.as_deref(),
        Some("CATALOG_CONTENT_VERSION_CHANGED")
    );
    assert_eq!(batch.last_failure_http_status, Some(200));
    assert_eq!(storage.open_section_current(&scope).expect("LKG").len(), 2);
}

#[test]
fn latest_failure_snapshot_and_http_audit_survive_attempt_detail_removal() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("operational.sqlite");
    let scope = target("FALL_2026", "OPEN_FAILURE_AUDIT");
    let mut storage = OperationalStorage::open(&path).expect("storage");
    let version = publish_catalog(&mut storage, &scope, "failure-audit", &["00001"], HASH_A);
    begin(
        &mut storage,
        &scope,
        "open-failure-audit",
        "open-failure-audit-run",
        version,
        "2026-07-11",
    );
    fail(&mut storage, "open-failure-audit");

    let attempt = storage
        .open_attempt(&trace("open-failure-audit"))
        .expect("attempt")
        .expect("present");
    assert_eq!(attempt.http, failure_http());
    let expected = storage
        .open_batch_state(&scope)
        .expect("batch")
        .expect("present");
    assert_eq!(expected.last_failure_attempt_sequence, Some(1));
    assert_eq!(expected.last_failure_at.as_deref(), Some(COMPLETED));
    assert_eq!(
        expected.last_failure_error_code.as_deref(),
        Some("OPEN_TRANSIENT_HTTP")
    );
    assert_eq!(expected.last_failure_http_status, Some(429));
    drop(storage);

    let detail_connection = Connection::open(&path).expect("detail connection");
    detail_connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("foreign keys");
    assert_eq!(
        detail_connection
            .execute(
                "DELETE FROM open_pull_attempts WHERE attempt_id = ?1",
                [trace("open-failure-audit").to_string()],
            )
            .expect("remove retained detail"),
        1
    );
    drop(detail_connection);

    let storage = OperationalStorage::open(&path).expect("restart");
    assert!(
        storage
            .open_attempt(&trace("open-failure-audit"))
            .expect("attempt lookup")
            .is_none()
    );
    let actual = storage
        .open_batch_state(&scope)
        .expect("batch")
        .expect("present");
    assert_eq!(actual.last_failure_attempt_sequence, Some(1));
    assert_eq!(actual.last_failure_at, expected.last_failure_at);
    assert_eq!(
        actual.last_failure_error_code,
        expected.last_failure_error_code
    );
    assert_eq!(actual.last_failure_http_status, Some(429));
}

#[test]
fn http_audit_validation_rejects_incomplete_success_and_unsafe_header_text() {
    let scope = target("FALL_2026", "OPEN_HTTP_VALIDATION");
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let version = publish_catalog(&mut storage, &scope, "http-validation", &["00001"], HASH_A);
    begin(
        &mut storage,
        &scope,
        "open-http-validation",
        "open-http-validation-run",
        version,
        "2026-07-14",
    );

    let mut incomplete = success_http(9);
    incomplete.content_type = None;
    assert!(matches!(
        storage.finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
            attempt_id: trace("open-http-validation"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00001")],
            source_value_count: 1,
            watched_sections: Vec::new(),
            http: incomplete,
        }),
        Err(StorageError::InvalidCommand {
            field: "content_type",
            ..
        })
    ));

    let mut unsafe_header = failure_http();
    unsafe_header.retry_after = Some("120\r\nX-Unsafe: true".to_owned());
    assert!(matches!(
        storage.finish_open_pull_failure(&FinishOpenPullFailureCommand {
            attempt_id: trace("open-http-validation"),
            completed_at: COMPLETED.to_owned(),
            http: unsafe_header,
            error_code: "OPEN_TRANSIENT_HTTP".to_owned(),
            diagnostic_token: None,
        }),
        Err(StorageError::InvalidCommand {
            field: "retry_after",
            ..
        })
    ));
    fail(&mut storage, "open-http-validation");
}

#[test]
fn empty_catalog_accepts_nonempty_all_orphan_response_and_true_empty_response() {
    let scope = target("FALL_2026", "OPEN_NO_ROWS");
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let version = publish_catalog(&mut storage, &scope, "no-rows", &[], HASH_A);
    begin(
        &mut storage,
        &scope,
        "open-no-rows-1",
        "open-no-rows-run",
        version,
        "2026-07-14",
    );
    let all_orphan = storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
            attempt_id: trace("open-no-rows-1"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "99999")],
            source_value_count: 1,
            watched_sections: Vec::new(),
            http: success_http(9),
        })
        .expect("apply all-orphan empty Catalog response");
    assert_eq!(
        all_orphan.classification,
        OpenAttemptClassification::ValidApplied
    );
    assert_eq!(
        (all_orphan.catalog_section_count, all_orphan.orphan_count),
        (0, 1)
    );
    assert!(
        storage
            .open_section_current(&scope)
            .expect("no current rows")
            .is_empty()
    );

    begin(
        &mut storage,
        &scope,
        "open-no-rows-2",
        "open-no-rows-run",
        version,
        "2026-07-14",
    );
    let empty = storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
            attempt_id: trace("open-no-rows-2"),
            completed_at: COMPLETED.to_owned(),
            open_sections: Vec::new(),
            source_value_count: 0,
            watched_sections: Vec::new(),
            http: success_http(2),
        })
        .expect("apply valid empty no rows");
    assert_eq!(
        empty.classification,
        OpenAttemptClassification::ValidEmptyNoRows
    );
    assert_eq!(
        storage
            .open_day_counters(&scope, "2026-07-14")
            .expect("counters")
            .empty,
        1
    );
}

#[test]
fn unchanged_valid_batch_still_emits_a_watched_section_event() {
    let scope = target("FALL_2026", "OPEN_UNCHANGED");
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let version = publish_catalog(&mut storage, &scope, "unchanged", &["00001"], HASH_A);
    for ordinal in 1..=2 {
        let attempt = format!("open-unchanged-{ordinal}");
        begin(
            &mut storage,
            &scope,
            &attempt,
            "open-unchanged-run",
            version,
            "2026-07-14",
        );
        let outcome = storage
            .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
                attempt_id: trace(&attempt),
                completed_at: COMPLETED.to_owned(),
                open_sections: vec![section(&scope, "00001")],
                source_value_count: 1,
                watched_sections: vec![section(&scope, "00001")],
                http: success_http(9),
            })
            .expect("apply valid batch");
        if ordinal == 2 {
            assert_eq!(outcome.changed_section_count, 0);
            assert!(!outcome.body_changed);
            assert!(!outcome.state_changed);
        }
    }
    let events = storage
        .read_open_section_events(0, 10)
        .expect("watched events");
    assert_eq!(events.len(), 2);
    assert_eq!(events[1].attempt_id, trace("open-unchanged-2"));
    assert_eq!(events[1].state, OpenSectionState::Open);
}

#[test]
fn target_single_flight_is_transactional_across_connections_and_old_finishes_are_inert() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("operational.sqlite");
    let scope = target("FALL_2026", "OPEN_SINGLE_FLIGHT");
    let mut first_connection = OperationalStorage::open(&path).expect("first connection");
    let version = publish_catalog(
        &mut first_connection,
        &scope,
        "single-flight",
        &["00001", "00002"],
        HASH_A,
    );
    let first = begin_command(
        &scope,
        "open-single-flight-1",
        "open-single-flight-run",
        version,
        "2026-07-14",
    );
    assert_eq!(
        first_connection
            .begin_open_pull_attempt(&first)
            .expect("first start"),
        1
    );
    let mut second_connection =
        OperationalStorage::open(&path).expect("connection opened while attempt is active");
    assert_eq!(
        second_connection
            .begin_open_pull_attempt(&first)
            .expect("cross-connection idempotent start"),
        1
    );
    let later = begin_command(
        &scope,
        "open-single-flight-2",
        "open-single-flight-run",
        version,
        "2026-07-14",
    );
    assert!(matches!(
        second_connection.begin_open_pull_attempt(&later),
        Err(StorageError::OpenTargetInFlight(active))
            if active == trace("open-single-flight-1")
    ));

    second_connection
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
            attempt_id: trace("open-single-flight-1"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00001")],
            source_value_count: 1,
            watched_sections: Vec::new(),
            http: success_http(9),
        })
        .expect("finish first attempt");
    assert_eq!(
        first_connection
            .begin_open_pull_attempt(&later)
            .expect("start after first completed"),
        2
    );
    first_connection
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
            attempt_id: trace("open-single-flight-2"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00002")],
            source_value_count: 1,
            watched_sections: Vec::new(),
            http: success_http(9),
        })
        .expect("finish later attempt");
    assert!(matches!(
        second_connection.finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
            attempt_id: trace("open-single-flight-1"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00001")],
            source_value_count: 1,
            watched_sections: Vec::new(),
            http: success_http(9),
        }),
        Err(StorageError::OpenAttemptNotStarted(attempt))
            if attempt == trace("open-single-flight-1")
    ));
    let current = second_connection
        .open_section_current(&scope)
        .expect("later current state");
    assert_eq!(
        current
            .iter()
            .map(|section| section.state)
            .collect::<Vec<_>>(),
        vec![OpenSectionState::Closed, OpenSectionState::Open]
    );
    assert!(
        current
            .iter()
            .all(|section| section.attempt_id == trace("open-single-flight-2"))
    );
}

#[test]
fn simultaneous_target_starts_across_connections_admit_exactly_one_attempt() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("operational.sqlite");
    let scope = target("FALL_2026", "OPEN_CONCURRENT_START");
    let mut first_connection = OperationalStorage::open(&path).expect("first connection");
    let version = publish_catalog(
        &mut first_connection,
        &scope,
        "concurrent-start",
        &["00001"],
        HASH_A,
    );
    let second_connection = OperationalStorage::open(&path).expect("second connection");
    let barrier = Arc::new(Barrier::new(3));
    let first_barrier = Arc::clone(&barrier);
    let first_scope = scope.clone();
    let first = std::thread::spawn(move || {
        let mut storage = first_connection;
        first_barrier.wait();
        let result = storage.begin_open_pull_attempt(&begin_command(
            &first_scope,
            "open-concurrent-start-1",
            "open-concurrent-start-run",
            version,
            "2026-07-14",
        ));
        (storage, result)
    });
    let second_barrier = Arc::clone(&barrier);
    let second_scope = scope.clone();
    let second = std::thread::spawn(move || {
        let mut storage = second_connection;
        second_barrier.wait();
        let result = storage.begin_open_pull_attempt(&begin_command(
            &second_scope,
            "open-concurrent-start-2",
            "open-concurrent-start-run",
            version,
            "2026-07-14",
        ));
        (storage, result)
    });
    barrier.wait();
    let (mut first_connection, first_result) = first.join().expect("first start thread");
    let (mut second_connection, second_result) = second.join().expect("second start thread");

    match (first_result, second_result) {
        (Ok(1), Err(StorageError::OpenTargetInFlight(active))) => {
            assert_eq!(active, trace("open-concurrent-start-1"));
            fail(&mut first_connection, "open-concurrent-start-1");
        }
        (Err(StorageError::OpenTargetInFlight(active)), Ok(1)) => {
            assert_eq!(active, trace("open-concurrent-start-2"));
            fail(&mut second_connection, "open-concurrent-start-2");
        }
        (first, second) => panic!("unexpected concurrent start results: {first:?}, {second:?}"),
    }
}

#[test]
fn metadata_only_catalog_version_bump_does_not_change_open_state_hash() {
    let scope = target("FALL_2026", "OPEN_STATE_HASH");
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let v1 = publish_catalog(
        &mut storage,
        &scope,
        "state-hash-v1",
        &["00001", "00002"],
        HASH_A,
    );
    begin(
        &mut storage,
        &scope,
        "open-state-hash-1",
        "open-state-hash-run",
        v1,
        "2026-07-14",
    );
    storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
            attempt_id: trace("open-state-hash-1"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00001")],
            source_value_count: 1,
            watched_sections: Vec::new(),
            http: success_http(9),
        })
        .expect("first Open state");
    let first_hash = storage
        .open_attempt(&trace("open-state-hash-1"))
        .expect("first attempt")
        .expect("present")
        .state_sha256
        .expect("state hash");

    let v2 = publish_catalog(
        &mut storage,
        &scope,
        "state-hash-v2",
        &["00001", "00002"],
        HASH_B,
    );
    assert_eq!(v2, v1 + 1);
    begin(
        &mut storage,
        &scope,
        "open-state-hash-2",
        "open-state-hash-run",
        v2,
        "2026-07-14",
    );
    let second = storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
            attempt_id: trace("open-state-hash-2"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00001")],
            source_value_count: 1,
            watched_sections: Vec::new(),
            http: success_http(9),
        })
        .expect("same Open state after Catalog metadata change");
    assert!(!second.body_changed);
    assert!(!second.state_changed);
    assert_eq!(second.changed_section_count, 0);
    assert_eq!(
        storage
            .open_attempt(&trace("open-state-hash-2"))
            .expect("second attempt")
            .expect("present")
            .state_sha256
            .as_deref(),
        Some(first_hash.as_str())
    );
}

#[test]
fn catalog_race_counts_are_derived_from_the_captured_snapshot() {
    let scope = target("FALL_2026", "OPEN_RACE_COUNTS");
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let v1 = publish_catalog(
        &mut storage,
        &scope,
        "race-counts-v1",
        &["00001", "00002"],
        HASH_A,
    );
    begin(
        &mut storage,
        &scope,
        "open-race-counts",
        "open-race-counts-run",
        v1,
        "2026-07-14",
    );
    let v2 = publish_catalog(&mut storage, &scope, "race-counts-v2", &["00001"], HASH_B);
    assert_eq!(v2, v1 + 1);
    let race = storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
            attempt_id: trace("open-race-counts"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00002"), section(&scope, "99999")],
            source_value_count: 2,
            watched_sections: Vec::new(),
            http: success_http(18),
        })
        .expect("Catalog race classification");
    assert_eq!(
        race.classification,
        OpenAttemptClassification::StaleCatalogRace
    );
    assert_eq!(
        (
            race.catalog_section_count,
            race.intersection_count,
            race.orphan_count,
        ),
        (2, 1, 1)
    );
    let stored = storage
        .open_attempt(&trace("open-race-counts"))
        .expect("race attempt")
        .expect("present");
    assert_eq!(
        (
            stored.catalog_section_count,
            stored.intersection_count,
            stored.orphan_count,
        ),
        (2, 1, 1)
    );
    assert!(
        storage
            .open_section_current(&scope)
            .expect("no LKG")
            .is_empty()
    );
}

#[test]
fn scheduler_lanes_round_trip_and_service_day_counters_sum_targets() {
    let target_a = target("FALL_2026", "OPEN_LANES_A");
    let target_b = target("FALL_2026", "OPEN_LANES_B");
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let version_a = publish_catalog(&mut storage, &target_a, "lanes-a", &["00001"], HASH_A);
    let version_b = publish_catalog(&mut storage, &target_b, "lanes-b", &["00001"], HASH_A);
    let lanes = [
        OpenRequestLane::General,
        OpenRequestLane::ActiveWatch,
        OpenRequestLane::FirstLoad,
        OpenRequestLane::ManualRefresh,
        OpenRequestLane::CatalogRaceRecheck,
    ];
    for (ordinal, lane) in lanes.into_iter().enumerate() {
        let attempt = format!("open-lane-{ordinal}");
        storage
            .begin_open_pull_attempt(&BeginOpenPullAttemptCommand {
                attempt_id: trace(&attempt),
                run_id: trace("open-lane-run"),
                target: target_a.clone(),
                captured_catalog_content_version: version_a,
                rutgers_day: "2026-07-14".to_owned(),
                started_at: STARTED.to_owned(),
                lane,
                requested_interval_seconds: Some(30),
                effective_interval_seconds: Some(30),
                schedule_lag_ms: Some(0),
            })
            .expect("begin lane attempt");
        fail(&mut storage, &attempt);
        assert_eq!(
            storage
                .open_attempt(&trace(&attempt))
                .expect("attempt")
                .expect("present")
                .lane,
            lane
        );
    }
    begin(
        &mut storage,
        &target_b,
        "open-lane-target-b",
        "open-lane-run",
        version_b,
        "2026-07-14",
    );
    fail(&mut storage, "open-lane-target-b");
    assert_eq!(
        storage
            .open_service_day_counters("2026-07-14")
            .expect("service counters"),
        bcsp_operational_storage::OpenAttemptCounters {
            attempted: 6,
            succeeded: 0,
            failed: 6,
            empty: 0,
        }
    );
}

#[test]
fn open_commit_fault_rolls_back_and_failed_completion_keeps_lkg() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("operational.sqlite");
    let scope = target("FALL_2026", "OPEN_ROLLBACK");
    let mut storage = OperationalStorage::open(&path).expect("storage");
    let version = publish_catalog(
        &mut storage,
        &scope,
        "rollback",
        &["00001", "00002"],
        HASH_A,
    );
    begin(
        &mut storage,
        &scope,
        "open-rollback-1",
        "open-rollback-run",
        version,
        "2026-07-14",
    );
    storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
            attempt_id: trace("open-rollback-1"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00001")],
            source_value_count: 1,
            watched_sections: Vec::new(),
            http: success_http(9),
        })
        .expect("stable LKG");
    begin(
        &mut storage,
        &scope,
        "open-rollback-2",
        "open-rollback-run",
        version,
        "2026-07-14",
    );
    let fault_connection = Connection::open(&path).expect("fault connection");
    fault_connection
        .execute_batch(
            "CREATE TRIGGER synthetic_open_commit_fault
             BEFORE INSERT ON open_section_current
             BEGIN SELECT RAISE(ABORT, 'synthetic Open fault'); END;",
        )
        .expect("fault trigger");
    drop(fault_connection);
    assert!(
        storage
            .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
                attempt_id: trace("open-rollback-2"),
                completed_at: COMPLETED.to_owned(),
                open_sections: vec![section(&scope, "00002")],
                source_value_count: 1,
                watched_sections: Vec::new(),
                http: success_http(9),
            })
            .is_err()
    );
    assert_eq!(
        storage
            .open_attempt(&trace("open-rollback-2"))
            .expect("attempt")
            .expect("present")
            .classification,
        OpenAttemptClassification::Started
    );
    assert_eq!(
        storage.open_section_current(&scope).expect("LKG")[0].state,
        OpenSectionState::Open
    );
    fail(&mut storage, "open-rollback-2");
    assert_eq!(
        storage
            .open_batch_state(&scope)
            .expect("batch")
            .expect("state")
            .lkg_attempt_id,
        Some(trace("open-rollback-1"))
    );
}

#[test]
fn open_mirror_rebuild_prepares_the_insert_once_and_reuses_it_across_commits() {
    let scope = target("FALL_2026", "OPEN_PREPARE_COUNT");
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let version = publish_catalog(
        &mut storage,
        &scope,
        "prepare-count",
        &["00001", "00002", "00003"],
        HASH_A,
    );

    let insert_prepares = Arc::new(AtomicUsize::new(0));
    let hook_counter = Arc::clone(&insert_prepares);
    {
        let connection = storage.raw_connection_for_tests();
        connection.flush_prepared_statement_cache();
        connection
            .authorizer(Some(move |context: AuthContext<'_>| {
                if matches!(
                    context.action,
                    AuthAction::Insert { table_name } if table_name == "open_section_current"
                ) {
                    hook_counter.fetch_add(1, Ordering::SeqCst);
                }
                Authorization::Allow
            }))
            .expect("install authorizer");
    }

    begin(
        &mut storage,
        &scope,
        "prepare-count-1",
        "prepare-count-run",
        version,
        "2026-07-14",
    );
    let first = storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
            gate_hold: false,
            gate_catalog_set_identity: None,
            attempt_id: trace("prepare-count-1"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00001")],
            source_value_count: 1,
            watched_sections: Vec::new(),
            http: success_http(9),
        })
        .expect("first applied commit");
    assert_eq!(first.classification, OpenAttemptClassification::ValidApplied);
    assert_eq!(
        insert_prepares.load(Ordering::SeqCst),
        1,
        "the 3-row mirror rebuild must prepare its INSERT exactly once"
    );

    begin(
        &mut storage,
        &scope,
        "prepare-count-2",
        "prepare-count-run",
        version,
        "2026-07-14",
    );
    let second = storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
            gate_hold: false,
            gate_catalog_set_identity: None,
            attempt_id: trace("prepare-count-2"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00002")],
            source_value_count: 1,
            watched_sections: Vec::new(),
            http: success_http(9),
        })
        .expect("second applied commit");
    assert_eq!(
        second.classification,
        OpenAttemptClassification::ValidApplied
    );
    assert_eq!(
        insert_prepares.load(Ordering::SeqCst),
        1,
        "the second commit must reuse the connection-cached statement"
    );

    let current = storage.open_section_current(&scope).expect("current");
    assert_eq!(current.len(), 3);
    assert!(
        current
            .iter()
            .all(|row| row.attempt_id == trace("prepare-count-2"))
    );
    storage
        .raw_connection_for_tests()
        .authorizer(None::<fn(AuthContext<'_>) -> Authorization>)
        .expect("clear authorizer");
}

#[test]
fn mid_rebuild_fault_on_the_second_row_rolls_back_the_whole_open_commit() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("operational.sqlite");
    let scope = target("FALL_2026", "OPEN_MID_ROLLBACK");
    let mut storage = OperationalStorage::open(&path).expect("storage");
    let version = publish_catalog(
        &mut storage,
        &scope,
        "mid-rollback",
        &["00001", "00002", "00003"],
        HASH_A,
    );

    begin(
        &mut storage,
        &scope,
        "mid-rollback-1",
        "mid-rollback-run",
        version,
        "2026-07-14",
    );
    storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
            gate_hold: false,
            gate_catalog_set_identity: None,
            attempt_id: trace("mid-rollback-1"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00001"), section(&scope, "00003")],
            source_value_count: 2,
            watched_sections: vec![section(&scope, "00001")],
            http: success_http(18),
        })
        .expect("establish LKG");
    let baseline_current = storage.open_section_current(&scope).expect("baseline current");
    let baseline_batch = storage
        .open_batch_state(&scope)
        .expect("batch")
        .expect("state");
    let baseline_events = storage
        .read_open_section_events(0, 100)
        .expect("baseline events");
    assert_eq!(baseline_current.len(), 3);
    assert_eq!(baseline_batch.lkg_attempt_id, Some(trace("mid-rollback-1")));

    begin(
        &mut storage,
        &scope,
        "mid-rollback-2",
        "mid-rollback-run",
        version,
        "2026-07-14",
    );
    let fault_connection = Connection::open(&path).expect("fault connection");
    fault_connection
        .execute_batch(
            "CREATE TRIGGER synthetic_open_commit_fault
             BEFORE INSERT ON open_section_current
             WHEN NEW.section_index <> '00001'
             BEGIN SELECT RAISE(ABORT, 'synthetic Open fault'); END;",
        )
        .expect("fault trigger");
    drop(fault_connection);

    assert!(
        storage
            .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
                attempt_id: trace("mid-rollback-2"),
                completed_at: COMPLETED.to_owned(),
                open_sections: vec![section(&scope, "00002")],
                source_value_count: 1,
                watched_sections: vec![section(&scope, "00002")],
                http: success_http(9),
            })
            .is_err(),
        "the fault fires on the SECOND mirror row after the first inserted"
    );

    assert_eq!(
        storage
            .open_section_current(&scope)
            .expect("post-fault current"),
        baseline_current
    );

    let batch = storage
        .open_batch_state(&scope)
        .expect("batch")
        .expect("state");
    assert_eq!(
        batch.last_observation_sequence,
        baseline_batch.last_observation_sequence
    );
    assert_eq!(batch.lkg_attempt_id, Some(trace("mid-rollback-1")));
    assert_eq!(
        batch.lkg_observation_sequence,
        baseline_batch.lkg_observation_sequence
    );
    assert_eq!(batch.lkg_observed_at, baseline_batch.lkg_observed_at);
    assert_eq!(
        batch.lkg_canonical_set_sha256,
        baseline_batch.lkg_canonical_set_sha256
    );
    assert_eq!(batch.lkg_state_sha256, baseline_batch.lkg_state_sha256);
    assert_eq!(batch.last_success_at, baseline_batch.last_success_at);

    assert!(
        storage
            .open_batch_observation(&trace("mid-rollback-2"))
            .expect("observation read")
            .is_none()
    );
    assert_eq!(
        storage
            .read_open_section_events(0, 100)
            .expect("post-fault events"),
        baseline_events
    );

    assert_eq!(
        storage
            .open_attempt(&trace("mid-rollback-2"))
            .expect("attempt")
            .expect("present")
            .classification,
        OpenAttemptClassification::Started
    );
    fail(&mut storage, "mid-rollback-2");
    assert_eq!(
        storage
            .open_attempt(&trace("mid-rollback-2"))
            .expect("attempt")
            .expect("present")
            .classification,
        OpenAttemptClassification::Failed
    );
    let after_fail = storage
        .open_batch_state(&scope)
        .expect("batch")
        .expect("state");
    assert_eq!(after_fail.lkg_attempt_id, Some(trace("mid-rollback-1")));
    assert_eq!(
        after_fail.last_failure_error_code.as_deref(),
        Some("OPEN_TRANSIENT_HTTP")
    );
}

#[test]
fn restart_interrupts_started_attempt_and_retention_preserves_aggregates() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("operational.sqlite");
    let scope = target("FALL_2026", "OPEN_RETENTION");
    let mut storage = OperationalStorage::open(&path).expect("storage");
    let version = publish_catalog(&mut storage, &scope, "retention", &["00001"], HASH_A);
    storage
        .put_open_origin_state(&OpenOriginState {
            origin_id: "rutgers_catalog".to_owned(),
            circuit_state: OpenCircuitState::RetryAfter,
            reason_code: Some("UPSTREAM_BACKOFF".to_owned()),
            opened_at: Some(STARTED.to_owned()),
            retry_at: Some(COMPLETED.to_owned()),
            diagnostic_recheck_required: false,
            updated_at: STARTED.to_owned(),
        })
        .expect("persist origin circuit");
    storage
        .put_open_schedule_state(&OpenScheduleState {
            target: scope.clone(),
            requested_interval_seconds: 30,
            effective_interval_seconds: 60,
            next_due_at: COMPLETED.to_owned(),
            last_scheduled_at: Some(STARTED.to_owned()),
            last_actual_start_at: Some(STARTED.to_owned()),
            schedule_lag_ms: 12,
            failure_streak: 2,
            updated_at: STARTED.to_owned(),
        })
        .expect("persist schedule state");
    begin(
        &mut storage,
        &scope,
        "open-interrupted",
        "open-retention-run",
        version,
        "2026-07-11",
    );
    drop(storage);
    let mut storage = OperationalStorage::open(&path).expect("recover restart");
    let origin = storage
        .open_origin_state("rutgers_catalog")
        .expect("origin")
        .expect("present");
    assert_eq!(origin.circuit_state, OpenCircuitState::RetryAfter);
    assert_eq!(origin.opened_at.as_deref(), Some(STARTED));
    assert_eq!(origin.retry_at.as_deref(), Some(COMPLETED));
    assert!(!origin.diagnostic_recheck_required);
    assert_eq!(
        storage
            .open_schedule_state(&scope)
            .expect("schedule")
            .expect("present")
            .failure_streak,
        2
    );
    assert_eq!(
        storage
            .open_attempt(&trace("open-interrupted"))
            .expect("attempt")
            .expect("present")
            .classification,
        OpenAttemptClassification::Interrupted
    );
    let interrupted_batch = storage
        .open_batch_state(&scope)
        .expect("batch")
        .expect("present");
    assert_eq!(interrupted_batch.last_failure_attempt_sequence, Some(1));
    assert!(interrupted_batch.last_failure_at.is_some());
    assert_eq!(
        interrupted_batch.last_failure_error_code.as_deref(),
        Some("PROCESS_RESTARTED")
    );
    assert_eq!(interrupted_batch.last_failure_http_status, None);
    for ordinal in 1..=257 {
        let label = format!("open-old-{ordinal}");
        begin(
            &mut storage,
            &scope,
            &label,
            "open-retention-run",
            version,
            "2026-07-11",
        );
        fail(&mut storage, &label);
    }
    for ordinal in 1..=2 {
        let label = format!("open-current-{ordinal}");
        begin(
            &mut storage,
            &scope,
            &label,
            "open-retention-run",
            version,
            "2026-07-14",
        );
        fail(&mut storage, &label);
    }
    let old_counters = storage
        .open_day_counters(&scope, "2026-07-11")
        .expect("old counters");
    assert_eq!((old_counters.attempted, old_counters.failed), (258, 258));
    assert!(matches!(
        storage.prune_open_diagnostics(&scope, "2026-07-14", 255),
        Err(StorageError::InvalidCommand {
            field: "retain_recent",
            reason: "must retain at least 256 attempts per target",
        })
    ));
    let report = storage
        .prune_open_diagnostics(&scope, "2026-07-14", 256)
        .expect("prune details");
    assert_eq!(report.attempts_removed, 0);
    assert!(
        storage
            .open_attempt(&trace("open-interrupted"))
            .expect("attempt lookup")
            .is_none()
    );
    assert_eq!(
        storage
            .open_day_counters(&scope, "2026-07-11")
            .expect("rolled aggregate"),
        old_counters
    );
}

#[test]
fn suspect_hold_retains_lkg_emits_nothing_and_recovery_applies() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("operational.sqlite");
    let mut storage = OperationalStorage::open(&path).expect("open");
    let scope = target("92026", "NB");
    let version = publish_catalog(
        &mut storage,
        &scope,
        "gate",
        &["00001", "00002", "00003"],
        HASH_A,
    );

    // Attempt 1: applied full snapshot establishes the LKG.
    begin(&mut storage, &scope, "gate-a1", "gate-run", version, "2026-08-19");
    let applied = storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
            gate_hold: false,
            gate_catalog_set_identity: None,
            attempt_id: trace("gate-a1"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00001"), section(&scope, "00002")],
            source_value_count: 2,
            watched_sections: vec![section(&scope, "00002")],
            http: success_http(64),
        })
        .expect("applied baseline");
    assert_eq!(applied.classification, OpenAttemptClassification::ValidApplied);
    let baseline_events = storage.read_open_section_events(0, 10).expect("events").len();

    // Attempt 2: an otherwise-applicable snapshot withheld by the gate.
    begin(&mut storage, &scope, "gate-a2", "gate-run", version, "2026-08-19");
    let held = storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
            gate_hold: true,
            gate_catalog_set_identity: None,
            attempt_id: trace("gate-a2"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00001")],
            source_value_count: 1,
            watched_sections: vec![section(&scope, "00002")],
            http: success_http(32),
        })
        .expect("held commit succeeds as a non-applied attempt");
    assert_eq!(
        held.classification,
        OpenAttemptClassification::SuspectPartialSnapshot
    );
    assert!(held.observation_commit.is_none(), "no observation for a held snapshot");
    assert_eq!(held.retained_lkg_attempt_id, Some(trace("gate-a1")));
    assert_eq!(held.intersection_count, 1, "true intersection still audited");

    // LKG state mirror untouched: 00002 stays OPEN despite its absence from
    // the held snapshot, and no new watch events were written.
    let current = storage.open_section_current(&scope).expect("current");
    assert_eq!(current.len(), 3);
    assert_eq!(current[0].state, OpenSectionState::Open);
    assert_eq!(current[1].state, OpenSectionState::Open);
    assert_eq!(current[2].state, OpenSectionState::Closed);
    assert_eq!(
        storage.read_open_section_events(0, 10).expect("events").len(),
        baseline_events,
        "a held snapshot fans out zero section events",
    );
    let stored = storage
        .open_attempt(&trace("gate-a2"))
        .expect("attempt read")
        .expect("present");
    assert_eq!(stored.error_code.as_deref(), Some("SUSPECT_PARTIAL_SNAPSHOT"));

    // Gate seeding reads: newest-first summaries and the LKG open set.
    let summaries = storage
        .recent_open_gate_attempt_summaries(&scope, 8)
        .expect("summaries");
    assert_eq!(summaries.len(), 2);
    assert_eq!(
        summaries[0].classification,
        OpenAttemptClassification::SuspectPartialSnapshot
    );
    assert!(summaries[0].canonical_set_sha256.is_some());
    assert!(summaries[0].completed_at.is_some());
    assert_eq!(summaries[1].classification, OpenAttemptClassification::ValidApplied);
    let lkg = storage
        .serving_lkg_open_index_set(&scope)
        .expect("lkg read")
        .expect("state exists");
    assert_eq!(
        lkg.into_iter().collect::<Vec<_>>(),
        vec!["00001".to_owned(), "00002".to_owned()],
    );

    // Attempt 3: recovery applies normally through the same code path.
    begin(&mut storage, &scope, "gate-a3", "gate-run", version, "2026-08-19");
    let recovered = storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
            gate_hold: false,
            gate_catalog_set_identity: None,
            attempt_id: trace("gate-a3"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00001"), section(&scope, "00002")],
            source_value_count: 2,
            watched_sections: vec![section(&scope, "00002")],
            http: success_http(64),
        })
        .expect("recovery applies");
    assert_eq!(
        recovered.classification,
        OpenAttemptClassification::ValidApplied
    );
    assert!(recovered.observation_commit.is_some());
}

/// Reviewer merge-blocker pin: the restart summaries must round-trip the
/// exact catalog section-set identity a gated attempt was decided against,
/// and candidate attempts (decided by an independent runtime against an
/// unpublished catalog) must never appear in them.
#[test]
fn gate_summaries_bind_identity_and_exclude_candidate_attempts() {
    let scope = target("92026", "NB");
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let version = publish_catalog(
        &mut storage,
        &scope,
        "gate-summary",
        &["00001", "00002"],
        HASH_A,
    );

    // Serving attempt 1: applied, identity recorded.
    begin(&mut storage, &scope, "sum-a1", "sum-run", version, "2026-08-19");
    storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
            gate_hold: false,
            gate_catalog_set_identity: Some(HASH_B.to_owned()),
            attempt_id: trace("sum-a1"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00001"), section(&scope, "00002")],
            source_value_count: 2,
            watched_sections: Vec::new(),
            http: success_http(64),
        })
        .expect("applied serving attempt");

    // Serving attempt 2: gate hold, identity recorded on the suspect row.
    begin(&mut storage, &scope, "sum-a2", "sum-run", version, "2026-08-19");
    storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
            gate_hold: true,
            gate_catalog_set_identity: Some(HASH_B.to_owned()),
            attempt_id: trace("sum-a2"),
            completed_at: COMPLETED.to_owned(),
            open_sections: vec![section(&scope, "00001")],
            source_value_count: 1,
            watched_sections: Vec::new(),
            http: success_http(32),
        })
        .expect("held serving attempt");

    // Candidate attempt: a suspect decided against a staged (never published)
    // catalog. It is the newest row in the attempts table, yet it must not
    // surface in the serving restart summaries.
    let staged = stage_catalog(
        &mut storage,
        &scope,
        "gate-summary-v2",
        &["00001", "00003"],
        HASH_C,
    );
    let candidate = storage
        .candidate_open_catalog_snapshot(&staged, EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty)
        .expect("candidate")
        .expect("publishable candidate");
    let candidate_attempt = begin_command(
        &scope,
        "sum-c1",
        "sum-run",
        candidate.catalog.content_version,
        "2026-08-19",
    );
    storage
        .begin_candidate_open_pull_attempt(&candidate, &candidate_attempt)
        .expect("begin candidate attempt");
    let outcome = storage
        .finish_candidate_open_pull_success(
            staged,
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            FinishOpenPullSuccessCommand {
                gate_hold: true,
                gate_catalog_set_identity: Some(HASH_A.to_owned()),
                attempt_id: candidate_attempt.attempt_id,
                completed_at: COMPLETED.to_owned(),
                open_sections: vec![section(&scope, "00001")],
                source_value_count: 1,
                watched_sections: Vec::new(),
                http: success_http(32),
            },
        )
        .expect("candidate suspect finishes");
    assert!(outcome.catalog.is_none(), "held candidate discards the staged catalog");
    assert_eq!(
        outcome.open.classification,
        OpenAttemptClassification::SuspectPartialSnapshot
    );

    let summaries = storage
        .recent_open_gate_attempt_summaries(&scope, 8)
        .expect("summaries");
    assert_eq!(summaries.len(), 2, "unpublished candidate attempts are excluded");
    assert_eq!(
        summaries[0].classification,
        OpenAttemptClassification::SuspectPartialSnapshot
    );
    assert_eq!(summaries[0].catalog_set_identity.as_deref(), Some(HASH_B));
    assert!(summaries[0].canonical_set_sha256.is_some());
    assert!(summaries[0].completed_at.is_some());
    assert_eq!(
        summaries[1].classification,
        OpenAttemptClassification::ValidApplied
    );
    assert_eq!(summaries[1].catalog_set_identity.as_deref(), Some(HASH_B));

    // A SUCCESSFUL candidate publish promoted its runtime to serving and
    // closed any live quarantine -- its row must therefore SURVIVE the
    // candidate filter and stand in the stream as a non-suspect breaker
    // (reviewer counterexample: filtering it would stitch the pre-publish
    // suspects onto a post-publish episode the live runtime had reset).
    let published = stage_catalog(
        &mut storage,
        &scope,
        "gate-summary-v3",
        &["00001", "00003"],
        HASH_C,
    );
    let published_candidate = storage
        .candidate_open_catalog_snapshot(&published, EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty)
        .expect("candidate")
        .expect("publishable candidate");
    let published_attempt = begin_command(
        &scope,
        "sum-c2",
        "sum-run",
        published_candidate.catalog.content_version,
        "2026-08-19",
    );
    storage
        .begin_candidate_open_pull_attempt(&published_candidate, &published_attempt)
        .expect("begin publishing candidate attempt");
    let outcome = storage
        .finish_candidate_open_pull_success(
            published,
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: Some(HASH_C.to_owned()),
                attempt_id: published_attempt.attempt_id,
                completed_at: COMPLETED.to_owned(),
                open_sections: vec![section(&scope, "00001"), section(&scope, "00003")],
                source_value_count: 2,
                watched_sections: Vec::new(),
                http: success_http(64),
            },
        )
        .expect("candidate publishes atomically");
    assert!(outcome.catalog.is_some());
    assert!(outcome.open.classification.is_success());

    let summaries = storage
        .recent_open_gate_attempt_summaries(&scope, 8)
        .expect("summaries after publish");
    assert_eq!(
        summaries
            .iter()
            .map(|summary| summary.classification)
            .collect::<Vec<_>>(),
        vec![
            OpenAttemptClassification::ValidApplied,
            OpenAttemptClassification::SuspectPartialSnapshot,
            OpenAttemptClassification::ValidApplied,
        ],
        "the published candidate is newest and present as a breaker; the held candidate stays invisible",
    );
    assert_eq!(
        summaries[0].catalog_set_identity.as_deref(),
        Some(HASH_C),
        "the breaker row keeps its own decision identity",
    );
}
