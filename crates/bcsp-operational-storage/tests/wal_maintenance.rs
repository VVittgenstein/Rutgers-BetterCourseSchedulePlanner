//! WAL maintenance and database-growth contracts of the operational store.
//!
//! Two failures were observed live on the local product and both are pinned
//! here. A runtime-owned connection that never ends a read transaction pins
//! the WAL read mark, so no checkpoint can backfill past it and the log grows
//! by every Open commit until the process exits; every public read API
//! therefore leaves the connection out of any transaction, and a second
//! connection proves it by checkpointing every frame. And the per-attempt
//! Open detail (a full catalog membership copy each time) used to be exempt
//! from pruning for the whole current Rutgers day; retention is now a count,
//! applied incrementally, so the main file stays bounded too.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use bcsp_contracts::{CourseGroupKey, CourseVariantKey, SectionKey, TermCampusKey, TraceId};
use bcsp_operational_storage::{
    BeginOpenPullAttemptCommand, CatalogRefreshCommand, CatalogSnapshot, CourseTextSearchTokens,
    DiscoveredCampus, DiscoveredTerm, DiscoveryRefreshCommand, DiscoverySnapshot,
    DiscoverySourceKind, DiscoverySourceVersion, EmptySnapshotDecision,
    FinishOpenPullFailureCommand, FinishOpenPullSuccessCommand,
    OPEN_DIAGNOSTIC_PRUNE_ATTEMPTS_PER_COMMIT, OPEN_DIAGNOSTIC_RETENTION_PER_TARGET,
    OpenCacheStatus, OpenCircuitState, OpenHttpAuditMetadata, OpenOriginState, OpenRequestLane,
    OpenScheduleState, OperationalStorage, StorageTransactionState, StoredCourseGroup,
    StoredCourseVariant, StoredSection, WAL_JOURNAL_SIZE_LIMIT_BYTES, WalCheckpointMode,
    catalog_content_sha256_v1,
};
use rusqlite::Connection;
use serde_json::json;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const STARTED: &str = "2026-07-14T00:00:00Z";
const COMPLETED: &str = "2026-07-14T00:00:01Z";
const DAY: &str = "2026-07-14";
const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn target() -> TermCampusKey {
    TermCampusKey::try_new("92026", "NB").expect("synthetic target")
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
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
    .parse()
    .expect("deterministic UUIDv4")
}

fn indices(count: usize) -> Vec<String> {
    (1..=count).map(|index| format!("{index:05}")).collect()
}

fn catalog_snapshot(target: &TermCampusKey, indices: &[String]) -> CatalogSnapshot {
    let group = CourseGroupKey::try_new(
        target.term().as_str(),
        target.campus().as_str(),
        "SYN:WAL:001",
    )
    .expect("group");
    let variant =
        CourseVariantKey::try_new(group.clone(), &format!("v1:{HASH_A}")).expect("variant");
    CatalogSnapshot {
        course_groups: vec![StoredCourseGroup {
            key: group,
            canonical_facts: json!({"fixture": "wal-maintenance"}),
        }],
        course_variants: vec![StoredCourseVariant {
            key: variant.clone(),
            subject_code: Some("SYN".to_owned()),
            course_number: Some("001".to_owned()),
            title: Some("Synthetic Wal Maintenance".to_owned()),
            description: Some("checkpoint retention growth".to_owned()),
            credits_summary: None,
            supplement: None,
            search_document: "SYN:WAL:001 synthetic wal maintenance checkpoint".to_owned(),
            canonical_sha256: HASH_A.to_owned(),
            raw_multiplicity: 1,
            canonical_facts: json!({"fixture": "wal-maintenance"}),
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

fn publish_catalog(storage: &mut OperationalStorage, target: &TermCampusKey, count: usize) -> u64 {
    let snapshot = catalog_snapshot(target, &indices(count));
    let body = format!("synthetic-wal-{count}");
    let outcome = storage
        .apply_catalog_refresh(
            CatalogRefreshCommand {
                observation_id: trace(&format!("catalog-{count}")),
                target: target.clone(),
                started_at: STARTED.to_owned(),
                completed_at: COMPLETED.to_owned(),
                source_content_sha256: format!("{:x}", Sha256::digest(body.as_bytes())),
                semantic_content_sha256: catalog_content_sha256_v1(target, &snapshot)
                    .expect("catalog hash"),
                source_bytes: body.len() as u64,
                raw_payload: None,
                snapshot,
            },
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
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
    count: usize,
) -> TraceId {
    let snapshot = catalog_snapshot(target, &indices(count));
    let body = format!("synthetic-staged-{count}");
    let observation_id = trace(&format!("staged-{count}"));
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

fn publish_discovery(storage: &mut OperationalStorage, target: &TermCampusKey) {
    storage
        .apply_discovery_refresh(DiscoveryRefreshCommand {
            observation_id: trace("discovery"),
            started_at: STARTED.to_owned(),
            completed_at: COMPLETED.to_owned(),
            snapshot: DiscoverySnapshot {
                sources: vec![DiscoverySourceVersion {
                    source_version_id: format!("selector:{HASH_A}"),
                    source_kind: DiscoverySourceKind::Selector,
                    source_identity: "RUTGERS_SELECTOR".to_owned(),
                    content_sha256: HASH_A.to_owned(),
                    canonical_facts: json!({"schema": "discovery-source-v1"}),
                    observed_at: STARTED.to_owned(),
                }],
                terms: vec![DiscoveredTerm {
                    term_id: target.term().clone(),
                    year: Some(2026),
                    term_code: Some("9".to_owned()),
                    display_name: Some("Fall 2026".to_owned()),
                    published: Some(true),
                    canonical_facts: json!({"entity": "term"}),
                    source_version_id: format!("selector:{HASH_A}"),
                }],
                campuses: vec![DiscoveredCampus {
                    target: target.clone(),
                    display_name: Some("New Brunswick".to_owned()),
                    category: None,
                    enabled: Some(true),
                    canonical_facts: json!({"entity": "campus"}),
                    source_version_id: format!("selector:{HASH_A}"),
                }],
                subjects: Vec::new(),
            },
        })
        .expect("publish discovery");
}

fn begin_command(
    target: &TermCampusKey,
    attempt: &str,
    version: u64,
) -> BeginOpenPullAttemptCommand {
    BeginOpenPullAttemptCommand {
        attempt_id: trace(attempt),
        run_id: trace("wal-run"),
        target: target.clone(),
        captured_catalog_content_version: version,
        rutgers_day: DAY.to_owned(),
        started_at: STARTED.to_owned(),
        lane: OpenRequestLane::General,
        requested_interval_seconds: Some(30),
        effective_interval_seconds: Some(10),
        schedule_lag_ms: Some(0),
    }
}

fn success_http() -> OpenHttpAuditMetadata {
    OpenHttpAuditMetadata {
        http_status: Some(200),
        cache_status: Some(OpenCacheStatus::Miss),
        decoded_bytes: Some(64),
        decoded_body_sha256: Some(HASH_A.to_owned()),
        content_type: Some("application/json".to_owned()),
        etag: None,
        cache_control: Some("no-store".to_owned()),
        date: None,
        age_seconds: None,
        last_modified: None,
        retry_after: None,
        retry_after_seconds: None,
    }
}

fn succeed(
    storage: &mut OperationalStorage,
    target: &TermCampusKey,
    attempt: &str,
    version: u64,
    open: &[String],
) {
    storage
        .begin_open_pull_attempt(&begin_command(target, attempt, version))
        .expect("begin Open attempt");
    storage
        .finish_open_pull_success(FinishOpenPullSuccessCommand {
            gate_hold: false,
            gate_catalog_set_identity: None,
            attempt_id: trace(attempt),
            completed_at: COMPLETED.to_owned(),
            open_sections: open.iter().map(|index| section(target, index)).collect(),
            source_value_count: open.len() as u64,
            watched_sections: open
                .first()
                .map(|index| section(target, index))
                .into_iter()
                .collect(),
            http: success_http(),
        })
        .expect("finish Open success");
}

fn fail(storage: &mut OperationalStorage, target: &TermCampusKey, attempt: &str, version: u64) {
    storage
        .begin_open_pull_attempt(&begin_command(target, attempt, version))
        .expect("begin Open attempt");
    storage
        .finish_open_pull_failure(&FinishOpenPullFailureCommand {
            attempt_id: trace(attempt),
            completed_at: COMPLETED.to_owned(),
            http: success_http(),
            error_code: "OPEN_TRANSIENT_HTTP".to_owned(),
            diagnostic_token: None,
        })
        .expect("finish Open failure");
}

/// One public read, boxed so the invariant loop can name it.
type StorageRead = Box<dyn FnMut(&mut OperationalStorage)>;

fn wal_bytes(path: &Path) -> u64 {
    let mut wal = path.as_os_str().to_owned();
    wal.push("-wal");
    fs::metadata(wal)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

fn count(path: &Path, sql: &str) -> u64 {
    let connection = Connection::open(path).expect("raw connection");
    let value: i64 = connection
        .query_row(sql, [], |row| row.get(0))
        .expect("count query");
    u64::try_from(value).expect("nonnegative count")
}

/// A small write from one connection, then a PASSIVE checkpoint from another:
/// complete only if no connection anywhere holds a stale read transaction.
fn prove_no_pinned_reader(path: &Path) {
    let mut writer = OperationalStorage::open(path).expect("writer");
    writer
        .put_open_origin_state(&OpenOriginState {
            origin_id: "rutgers_catalog".to_owned(),
            circuit_state: OpenCircuitState::Closed,
            reason_code: None,
            opened_at: None,
            retry_at: None,
            diagnostic_recheck_required: false,
            updated_at: COMPLETED.to_owned(),
        })
        .expect("small write");
    let probe = OperationalStorage::open(path).expect("probe");
    let report = probe
        .checkpoint_wal(WalCheckpointMode::Passive)
        .expect("checkpoint")
        .expect("file-backed");
    assert!(!report.busy, "a passive checkpoint never waits: {report:?}");
    assert!(
        report.log_frames > 0,
        "the write must have produced frames: {report:?}"
    );
    assert!(
        report.is_complete(),
        "a connection is pinning the WAL read mark: {report:?}"
    );
    drop(probe);
    drop(writer);
}

#[test]
fn in_memory_fixtures_have_no_wal_to_maintain() {
    let storage = OperationalStorage::open_in_memory().expect("storage");
    assert_eq!(
        storage
            .checkpoint_wal(WalCheckpointMode::Truncate)
            .expect("checkpoint"),
        None
    );
    assert_eq!(storage.wal_journal_size_limit_bytes().expect("limit"), None);
    assert_eq!(
        storage.transaction_state().expect("state"),
        StorageTransactionState::None
    );
}

#[test]
fn every_writer_connection_bounds_the_journal_file() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("operational.sqlite");
    let serving = OperationalStorage::open(&path).expect("serving");
    let refresh = OperationalStorage::open(&path).expect("refresh");
    for connection in [&serving, &refresh] {
        assert_eq!(
            connection.wal_journal_size_limit_bytes().expect("limit"),
            Some(WAL_JOURNAL_SIZE_LIMIT_BYTES)
        );
    }
    assert_eq!(WAL_JOURNAL_SIZE_LIMIT_BYTES, 64 * 1024 * 1024);
}

#[test]
fn every_public_read_leaves_the_connection_out_of_a_transaction() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("operational.sqlite");
    let scope = target();
    let mut storage = OperationalStorage::open(&path).expect("storage");
    publish_discovery(&mut storage, &scope);
    let version = publish_catalog(&mut storage, &scope, 24);
    let open = indices(6);
    succeed(&mut storage, &scope, "read-a1", version, &open);
    fail(&mut storage, &scope, "read-a2", version);
    storage
        .put_open_schedule_state(&OpenScheduleState {
            target: scope.clone(),
            requested_interval_seconds: 30,
            effective_interval_seconds: 10,
            next_due_at: COMPLETED.to_owned(),
            last_scheduled_at: Some(STARTED.to_owned()),
            last_actual_start_at: Some(STARTED.to_owned()),
            schedule_lag_ms: 0,
            failure_streak: 1,
            updated_at: COMPLETED.to_owned(),
        })
        .expect("schedule state");
    let tokens = CourseTextSearchTokens::try_new(["synthetic"]).expect("tokens");
    let staged = stage_catalog(&mut storage, &scope, 25);

    let mut reads: Vec<(&str, StorageRead)> = vec![
        (
            "migration_records",
            Box::new(|s| {
                s.migration_records().unwrap();
            }),
        ),
        (
            "operational_table_names",
            Box::new(|s| {
                s.operational_table_names().unwrap();
            }),
        ),
        (
            "target_state",
            Box::new(|s| {
                s.target_state(&target()).unwrap();
            }),
        ),
        (
            "published_catalog_snapshot",
            Box::new(|s| {
                s.published_catalog_snapshot(&target()).unwrap();
            }),
        ),
        (
            "refresh_observation",
            Box::new(|s| {
                s.refresh_observation(&trace("catalog-24")).unwrap();
            }),
        ),
        (
            "catalog_failure_audit",
            Box::new(|s| {
                s.catalog_failure_audit(&trace("catalog-24")).unwrap();
            }),
        ),
        (
            "serving_section_keys",
            Box::new(|s| {
                s.serving_section_keys(&target()).unwrap();
            }),
        ),
        (
            "sections",
            Box::new(|s| {
                s.sections(&target()).unwrap();
            }),
        ),
        (
            "canonical_facts",
            Box::new(|s| {
                s.canonical_facts(&target()).unwrap();
            }),
        ),
        (
            "course_variants",
            Box::new(|s| {
                s.course_variants(&target()).unwrap();
            }),
        ),
        (
            "search_course_variants",
            Box::new(move |s| {
                s.search_course_variants(&target(), version, &tokens)
                    .unwrap();
            }),
        ),
        (
            "published_course_fts_documents",
            Box::new(move |s| {
                s.published_course_fts_documents(&target(), version)
                    .unwrap();
            }),
        ),
        (
            "visit_all_course_fts_documents",
            Box::new(|s| {
                s.visit_all_course_fts_documents(|_| Ok(())).unwrap();
            }),
        ),
        (
            "visit_all_course_fts_documents (visitor error)",
            Box::new(|s| {
                assert!(
                    s.visit_all_course_fts_documents(|_| Err(
                        bcsp_operational_storage::StorageError::InjectedFault("stop")
                    ))
                    .is_err()
                );
            }),
        ),
        (
            "course_fts_corpus_signature",
            Box::new(|s| {
                s.course_fts_corpus_signature().unwrap();
            }),
        ),
        (
            "published_course_fts_terms",
            Box::new(move |s| {
                s.published_course_fts_terms(&target(), version, Some("syn"), 8)
                    .unwrap();
            }),
        ),
        (
            "published_course_fts_term_exists",
            Box::new(move |s| {
                s.published_course_fts_term_exists(&target(), version, "synthetic")
                    .unwrap();
            }),
        ),
        (
            "integrity_report",
            Box::new(|s| {
                s.integrity_report(&target()).unwrap();
            }),
        ),
        (
            "staged_payload_count",
            Box::new(|s| {
                s.staged_payload_count().unwrap();
            }),
        ),
        (
            "candidate_open_catalog_snapshot",
            Box::new(move |s| {
                s.candidate_open_catalog_snapshot(
                    &staged,
                    EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
                )
                .unwrap()
                .unwrap();
            }),
        ),
        (
            "complete_target_snapshot_state",
            Box::new(|s| {
                s.complete_target_snapshot_state(&target()).unwrap();
            }),
        ),
        (
            "serving_lkg_open_index_set",
            Box::new(|s| {
                s.serving_lkg_open_index_set(&target()).unwrap();
            }),
        ),
        (
            "recent_open_gate_attempt_summaries",
            Box::new(|s| {
                s.recent_open_gate_attempt_summaries(&target(), 8).unwrap();
            }),
        ),
        (
            "open_attempt",
            Box::new(|s| {
                s.open_attempt(&trace("read-a1")).unwrap().unwrap();
            }),
        ),
        (
            "open_batch_state",
            Box::new(|s| {
                s.open_batch_state(&target()).unwrap().unwrap();
            }),
        ),
        (
            "serving_open_catalog_snapshot",
            Box::new(|s| {
                s.serving_open_catalog_snapshot(&target()).unwrap().unwrap();
            }),
        ),
        (
            "open_batch_observation",
            Box::new(|s| {
                s.open_batch_observation(&trace("read-a1"))
                    .unwrap()
                    .unwrap();
            }),
        ),
        (
            "open_section_current",
            Box::new(|s| {
                s.open_section_current(&target()).unwrap();
            }),
        ),
        (
            "read_open_section_events",
            Box::new(|s| {
                s.read_open_section_events(0, 100).unwrap();
            }),
        ),
        (
            "open_day_counters",
            Box::new(|s| {
                s.open_day_counters(&target(), DAY).unwrap();
            }),
        ),
        (
            "open_service_day_counters",
            Box::new(|s| {
                s.open_service_day_counters(DAY).unwrap();
            }),
        ),
        (
            "open_run_counters",
            Box::new(|s| {
                s.open_run_counters(&target(), &trace("wal-run")).unwrap();
            }),
        ),
        (
            "open_origin_state",
            Box::new(|s| {
                s.open_origin_state("rutgers_catalog").unwrap();
            }),
        ),
        (
            "open_schedule_state",
            Box::new(|s| {
                s.open_schedule_state(&target()).unwrap().unwrap();
            }),
        ),
        (
            "discovery_state",
            Box::new(|s| {
                s.discovery_state().unwrap();
            }),
        ),
        (
            "discovery_observation",
            Box::new(|s| {
                s.discovery_observation(&trace("discovery"))
                    .unwrap()
                    .unwrap();
            }),
        ),
        (
            "published_discovery_snapshot",
            Box::new(|s| {
                s.published_discovery_snapshot().unwrap();
            }),
        ),
        (
            "discovered_targets",
            Box::new(|s| {
                s.discovered_targets().unwrap();
            }),
        ),
    ];
    let mut reader = storage
        .open_prepared_snapshot_reader()
        .expect("reader")
        .expect("file-backed");
    for (name, read) in &mut reads {
        read(&mut storage);
        assert_eq!(
            storage.transaction_state().expect("state"),
            StorageTransactionState::None,
            "{name} left the writer inside a transaction"
        );
        read(&mut reader);
        assert_eq!(
            reader.transaction_state().expect("state"),
            StorageTransactionState::None,
            "{name} left the prepared-snapshot reader inside a transaction"
        );
    }
    prove_no_pinned_reader(&path);
    drop(reader);
    drop(storage);
    prove_no_pinned_reader(&path);
}

#[test]
fn checkpoints_backfill_and_truncate_once_no_reader_is_pinned() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("operational.sqlite");
    let scope = target();
    let mut writer = OperationalStorage::open(&path).expect("writer");
    let serving = OperationalStorage::open(&path).expect("serving");
    let version = publish_catalog(&mut writer, &scope, 2_000);
    let open = indices(50);
    for attempt in 1..=20 {
        succeed(
            &mut writer,
            &scope,
            &format!("cp-{attempt}"),
            version,
            &open,
        );
        // A short serving read between attempts, like a product route.
        serving.open_batch_state(&scope).expect("serving read");
        assert_eq!(
            serving.transaction_state().expect("state"),
            StorageTransactionState::None
        );
    }
    let main_before = fs::metadata(&path).expect("main file").len();
    let passive = writer
        .checkpoint_wal(WalCheckpointMode::Passive)
        .expect("checkpoint")
        .expect("file-backed");
    assert!(!passive.busy, "{passive:?}");
    assert!(passive.is_complete(), "{passive:?}");
    let truncate = writer
        .checkpoint_wal(WalCheckpointMode::Truncate)
        .expect("checkpoint")
        .expect("file-backed");
    assert!(!truncate.busy, "{truncate:?}");
    assert_eq!(wal_bytes(&path), 0, "TRUNCATE leaves an empty log file");
    assert!(
        fs::metadata(&path).expect("main file").len() >= main_before,
        "backfill moves pages into the main file"
    );
    // Another write after the truncate restarts the log from the beginning.
    succeed(&mut writer, &scope, "cp-after", version, &open);
    let restarted = writer
        .checkpoint_wal(WalCheckpointMode::Passive)
        .expect("checkpoint")
        .expect("file-backed");
    assert!(
        restarted.log_frames < passive.log_frames,
        "{restarted:?} vs {passive:?}"
    );
    assert!(restarted.is_complete(), "{restarted:?}");
}

#[test]
fn a_reader_inside_a_transaction_caps_the_checkpoint_and_is_reported_as_such() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("operational.sqlite");
    let scope = target();
    let mut writer = OperationalStorage::open(&path).expect("writer");
    let version = publish_catalog(&mut writer, &scope, 100);
    writer
        .checkpoint_wal(WalCheckpointMode::Truncate)
        .expect("checkpoint");
    // A raw reader pinned in a read transaction stands in for the bug: the
    // typed stores never do this, which is what the test above proves.
    let pinned = Connection::open(&path).expect("raw reader");
    pinned
        .execute_batch("BEGIN; SELECT count(*) FROM open_pull_attempts;")
        .expect("pin");
    succeed(&mut writer, &scope, "pinned-1", version, &indices(3));
    succeed(&mut writer, &scope, "pinned-2", version, &indices(4));
    let capped = writer
        .checkpoint_wal(WalCheckpointMode::Passive)
        .expect("checkpoint")
        .expect("file-backed");
    assert!(capped.log_frames > 0, "{capped:?}");
    assert!(
        !capped.is_complete(),
        "the pinned reader caps the backfill: {capped:?}"
    );
    pinned.execute_batch("COMMIT;").expect("release");
    let released = writer
        .checkpoint_wal(WalCheckpointMode::Passive)
        .expect("checkpoint")
        .expect("file-backed");
    assert!(released.is_complete(), "{released:?}");
}

#[test]
fn open_detail_retention_is_a_count_and_ignores_the_rutgers_day() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("operational.sqlite");
    let scope = target();
    let mut storage = OperationalStorage::open(&path).expect("storage");
    let section_count = 40;
    let version = publish_catalog(&mut storage, &scope, section_count);
    let total = OPEN_DIAGNOSTIC_RETENTION_PER_TARGET + 60;
    let open = indices(3);
    for ordinal in 1..=total {
        // Same Rutgers day for every attempt: the old rule kept all of them.
        succeed(
            &mut storage,
            &scope,
            &format!("day-{ordinal}"),
            version,
            &open,
        );
        let attempts = count(&path, "SELECT count(*) FROM open_pull_attempts");
        assert!(
            attempts <= OPEN_DIAGNOSTIC_RETENTION_PER_TARGET,
            "attempt {ordinal}: {attempts} attempts retained"
        );
        let membership = count(&path, "SELECT count(*) FROM open_attempt_catalog_sections");
        assert_eq!(
            membership,
            attempts * section_count as u64,
            "attempt {ordinal}"
        );
    }
    assert!(
        storage
            .open_attempt(&trace("day-1"))
            .expect("lookup")
            .is_none(),
        "the oldest attempt of the day is pruned"
    );
    assert!(
        storage
            .open_attempt(&trace(&format!("day-{total}")))
            .expect("lookup")
            .is_some()
    );
    let day = storage
        .open_day_counters(&scope, DAY)
        .expect("day counters");
    assert_eq!((day.attempted, day.succeeded), (total, total));
    let run = storage
        .open_run_counters(&scope, &trace("wal-run"))
        .expect("run counters");
    assert_eq!(run.attempted, total);
    let service = storage
        .open_service_day_counters(DAY)
        .expect("service counters");
    assert_eq!(service.attempted, total);
    // The last-known-good attempt is the newest one and stays.
    let batch = storage
        .open_batch_state(&scope)
        .expect("batch")
        .expect("present");
    assert_eq!(batch.lkg_attempt_id, Some(trace(&format!("day-{total}"))));
}

#[test]
fn a_prune_backlog_is_worked_off_a_bounded_number_of_attempts_per_commit() {
    let temp = TempDir::new().expect("temp");
    let path = temp.path().join("operational.sqlite");
    let scope = target();
    let mut storage = OperationalStorage::open(&path).expect("storage");
    let version = publish_catalog(&mut storage, &scope, 8);
    for ordinal in 1..=OPEN_DIAGNOSTIC_RETENTION_PER_TARGET {
        fail(&mut storage, &scope, &format!("backlog-{ordinal}"), version);
    }
    // Build a backlog the per-commit prune never saw: an attempt that ends
    // by process restart is recovered as INTERRUPTED at the next open, and
    // recovery does not prune. Forty of those is the shape a whole day of
    // exempt attempts used to leave behind, in miniature.
    let backlog = 40;
    for ordinal in 1..=backlog {
        storage
            .begin_open_pull_attempt(&begin_command(
                &scope,
                &format!("interrupted-{ordinal}"),
                version,
            ))
            .expect("begin attempt");
        drop(storage);
        storage = OperationalStorage::open(&path).expect("reopen recovers");
    }
    assert_eq!(
        count(&path, "SELECT count(*) FROM open_pull_attempts"),
        OPEN_DIAGNOSTIC_RETENTION_PER_TARGET + backlog
    );
    for ordinal in 1..=backlog {
        fail(&mut storage, &scope, &format!("fresh-{ordinal}"), version);
        let attempts = count(&path, "SELECT count(*) FROM open_pull_attempts");
        // Each commit adds one attempt and removes at most the bound, so the
        // backlog shrinks by at most (bound - 1) per commit and never grows.
        let floor = (OPEN_DIAGNOSTIC_RETENTION_PER_TARGET + backlog)
            .saturating_sub(ordinal * (OPEN_DIAGNOSTIC_PRUNE_ATTEMPTS_PER_COMMIT - 1))
            .max(OPEN_DIAGNOSTIC_RETENTION_PER_TARGET);
        assert!(
            attempts >= floor && attempts <= OPEN_DIAGNOSTIC_RETENTION_PER_TARGET + backlog,
            "commit {ordinal}: {attempts} attempts retained (floor {floor})"
        );
    }
    let before = count(&path, "SELECT count(*) FROM open_pull_attempts");
    let removed = storage
        .prune_open_diagnostics(&scope, OPEN_DIAGNOSTIC_RETENTION_PER_TARGET)
        .expect("prune")
        .attempts_removed;
    assert!(
        removed <= OPEN_DIAGNOSTIC_PRUNE_ATTEMPTS_PER_COMMIT,
        "one prune step removes at most the bound: {removed}"
    );
    assert_eq!(
        count(&path, "SELECT count(*) FROM open_pull_attempts"),
        before - removed
    );
    let mut steps = 0;
    while storage
        .prune_open_diagnostics(&scope, OPEN_DIAGNOSTIC_RETENTION_PER_TARGET)
        .expect("prune")
        .attempts_removed
        > 0
    {
        steps += 1;
        assert!(steps < 64, "the backlog must drain");
    }
    assert_eq!(
        count(&path, "SELECT count(*) FROM open_pull_attempts"),
        OPEN_DIAGNOSTIC_RETENTION_PER_TARGET
    );
    let kept = (1..=backlog)
        .map(|ordinal| trace(&format!("fresh-{ordinal}")))
        .filter(|attempt| storage.open_attempt(attempt).expect("lookup").is_some())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        kept.len(),
        backlog as usize,
        "the newest attempts are never the ones pruned"
    );
    // The aggregates count every attempt that ever ran, pruned or not.
    let day = storage
        .open_day_counters(&scope, DAY)
        .expect("day counters");
    assert_eq!(
        day.attempted,
        OPEN_DIAGNOSTIC_RETENTION_PER_TARGET + backlog + backlog
    );
    assert_eq!(
        day.failed,
        OPEN_DIAGNOSTIC_RETENTION_PER_TARGET + backlog + backlog
    );
}

/// Concurrent readers on a verbatim (`\?\`-prefixed) Windows path.
///
/// `Path::canonicalize` on Windows yields such a path, and the local runtime
/// opens every connection through one. SQLite's Windows VFS treats any path
/// that starts with two backslashes as a UNC path and switches its WAL-index
/// locking to a single shared file handle per process, whose shared-lock
/// bookkeeping races between connections: two readers taking the same read
/// lock at once can stack two OS locks and release only one, after which no
/// checkpoint can backfill a single frame until the process exits. The store
/// therefore hands SQLite a plain drive path, and this proves it by racing
/// readers and then demanding a complete checkpoint.
#[test]
fn concurrent_readers_on_a_verbatim_path_do_not_pin_the_wal() {
    let directory = TempDir::new().expect("temp dir");
    let verbatim = std::fs::canonicalize(directory.path())
        .expect("canonical temp dir")
        .join("verbatim.sqlite");
    let mut storage = OperationalStorage::open(&verbatim).expect("file-backed storage");
    publish_discovery(&mut storage, &target());
    let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let readers = (0..8)
        .map(|_| {
            let path = verbatim.clone();
            let stop = std::sync::Arc::clone(&stop);
            std::thread::spawn(move || {
                let reader = OperationalStorage::open(&path).expect("reader storage");
                let mut reads = 0_u64;
                while !stop.load(std::sync::atomic::Ordering::Relaxed) {
                    reader.migration_records().expect("read");
                    reads += 1;
                }
                reads
            })
        })
        .collect::<Vec<_>>();
    std::thread::sleep(std::time::Duration::from_millis(800));
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    let reads = readers
        .into_iter()
        .map(|reader| reader.join().expect("reader thread"))
        .sum::<u64>();
    assert!(reads > 0);

    // A write after the race, then the proof: with no reader left, a PASSIVE
    // checkpoint from a fresh connection must backfill every frame.
    publish_catalog(&mut storage, &target(), 3);
    let probe = OperationalStorage::open(&verbatim).expect("probe storage");
    let report = probe
        .checkpoint_wal(WalCheckpointMode::Passive)
        .expect("checkpoint")
        .expect("file-backed");
    assert!(report.log_frames > 0, "{report:?}");
    assert!(
        report.is_complete(),
        "a leaked WAL read lock is pinning the log after {reads} concurrent reads: {report:?}"
    );
}
