use std::path::Path;

use bcsp_contracts::{CourseGroupKey, CourseVariantKey, SectionKey, TermCampusKey, TraceId};
use bcsp_operational_storage::{
    BeginDiscoveryAttemptCommand, BeginRefreshAttemptCommand, CatalogDeliveryRewrite,
    CatalogRefreshCommand, CatalogSnapshot, CatalogTargetVersion, CourseTextSearchTokens,
    DiscoveredCampus, DiscoveredSubject, DiscoveredTerm, DiscoveryAvailability,
    DiscoveryPublishOutcome, DiscoveryRefreshCommand, DiscoverySnapshot, DiscoverySourceKind,
    DiscoverySourceVersion, EmptySnapshotDecision, FinishDiscoveryFailureCommand,
    FinishRefreshFailureCommand, InitialEmptyProof, OccurrenceDeliveryRewrite, OperationalStorage,
    ProvenanceEntityKind, PublishOutcome, RawStagingPayload, RefreshFailureStage, RefreshStatus,
    SectionDeliveryRewrite, StorageError, StoredCourseGroup, StoredCourseVariant, StoredOccurrence,
    StoredProvenance, StoredSection, catalog_content_sha256_v1, discovery_content_sha256_v1,
};
use rusqlite::Connection;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const STARTED: &str = "2026-07-14T00:00:00Z";
const COMPLETED: &str = "2026-07-14T00:00:01Z";
const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const HASH_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn target(term: &str, campus: &str) -> TermCampusKey {
    TermCampusKey::try_new(term, campus).expect("synthetic target")
}

fn text_tokens(tokens: &[&str]) -> CourseTextSearchTokens {
    CourseTextSearchTokens::try_new(tokens).expect("valid synthetic search tokens")
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn trace(label: &str) -> TraceId {
    let digest = Sha256::digest(label.as_bytes());
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let value = format!(
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
    );
    value.parse().expect("deterministic synthetic UUIDv4")
}

fn presence_facts(entity: &str) -> Value {
    json!({
        "entity": entity,
        "missing": {"presence": "MISSING"},
        "null": null,
        "empty": "",
        "value": "synthetic-value"
    })
}

fn snapshot(target: &TermCampusKey, index: &str, title: &str) -> CatalogSnapshot {
    let group = CourseGroupKey::try_new(
        target.term().as_str(),
        target.campus().as_str(),
        "SYN:CAT:001",
    )
    .expect("group key");
    let variant =
        CourseVariantKey::try_new(group.clone(), &format!("v1:{HASH_A}")).expect("variant key");
    let section = SectionKey::try_new(target.term().as_str(), target.campus().as_str(), index)
        .expect("section key");
    CatalogSnapshot {
        course_groups: vec![StoredCourseGroup {
            key: group,
            canonical_facts: presence_facts("group"),
        }],
        course_variants: vec![StoredCourseVariant {
            key: variant.clone(),
            subject_code: Some("SYN".to_owned()),
            course_number: Some("001".to_owned()),
            title: Some(title.to_owned()),
            description: Some(String::new()),
            credits_summary: None,
            supplement: Some(String::new()),
            search_document: format!("SYN:CAT:001 {title} synthetic computing 计算机"),
            canonical_sha256: HASH_A.to_owned(),
            raw_multiplicity: 1,
            canonical_facts: presence_facts("variant"),
        }],
        sections: vec![StoredSection {
            key: section.clone(),
            variant_key: variant,
            section_number: Some("01".to_owned()),
            catalog_status: None,
            section_course_type: Some(String::new()),
            delivery_modality: "UNKNOWN".to_owned(),
            synchronicity: "UNKNOWN".to_owned(),
            canonical_facts: presence_facts("section"),
            canonical_sha256: HASH_B.to_owned(),
        }],
        occurrences: vec![StoredOccurrence {
            section_key: section,
            occurrence_key: "meeting.0".to_owned(),
            ordinal: 0,
            weekday: None,
            start_minute: None,
            end_minute: None,
            time_knowledge: "MISSING".to_owned(),
            requiredness: "UNKNOWN_REQUIREDNESS".to_owned(),
            occurrence_kind: "UNSPECIFIED".to_owned(),
            evidence: "NONE".to_owned(),
            normalization_reason: "SYNTHETIC_FIXTURE".to_owned(),
            modality: "UNKNOWN".to_owned(),
            synchronicity: "UNKNOWN".to_owned(),
            location: Some(String::new()),
            building: None,
            room: Some("101".to_owned()),
            raw_sha256: HASH_B.to_owned(),
            canonical_facts: presence_facts("occurrence"),
        }],
        provenance: vec![StoredProvenance {
            entity_kind: ProvenanceEntityKind::CourseVariant,
            entity_key: format!(
                "{}/{}/SYN:CAT:001/v1:{HASH_A}",
                target.term(),
                target.campus()
            ),
            field_name: "title".to_owned(),
            source_sha256: HASH_A.to_owned(),
            source_ordinal: 0,
            detail: json!({"source": "SYNTHETIC_FIXTURE"}),
        }],
    }
}

fn refresh_command(
    observation_id: &str,
    target: &TermCampusKey,
    snapshot: CatalogSnapshot,
    body: &[u8],
) -> CatalogRefreshCommand {
    let body_sha256 = sha256(body);
    let semantic_content_sha256 =
        catalog_content_sha256_v1(target, &snapshot).expect("catalog-content-v1 hash");
    CatalogRefreshCommand {
        observation_id: trace(observation_id),
        target: target.clone(),
        started_at: STARTED.to_owned(),
        completed_at: COMPLETED.to_owned(),
        source_content_sha256: body_sha256.clone(),
        semantic_content_sha256,
        source_bytes: body.len() as u64,
        raw_payload: Some(RawStagingPayload {
            sha256: body_sha256,
            bytes: body.to_vec(),
        }),
        snapshot,
    }
}

fn db_path(temp: &TempDir) -> std::path::PathBuf {
    temp.path().join("synthetic-operational.sqlite")
}

fn assert_catalog_variant_fk_indexes(path: &Path) {
    let connection = Connection::open(path).expect("open schema inspection connection");
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .expect("enable foreign keys for cascade query plans");
    for (index, expected_columns) in [
        (
            "catalog_staging_sections_variant_fk",
            ["observation_id", "course_string", "fingerprint"],
        ),
        (
            "catalog_sections_variant_fk",
            ["target_id", "course_string", "fingerprint"],
        ),
    ] {
        let mut statement = connection
            .prepare("SELECT name FROM pragma_index_info(?1) ORDER BY seqno")
            .expect("prepare index inspection");
        let columns = statement
            .query_map([index], |row| row.get::<_, String>(0))
            .expect("read index columns")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect index columns");
        assert_eq!(columns, expected_columns, "{index}");
    }

    for (sql, child_table, expected_index, full_lookup) in [
        (
            "DELETE FROM catalog_staging_course_variants WHERE observation_id = ?1",
            "catalog_staging_sections",
            "catalog_staging_sections_variant_fk",
            "observation_id=? AND course_string=? AND fingerprint=?",
        ),
        (
            "DELETE FROM catalog_course_variants WHERE target_id = ?1",
            "catalog_sections",
            "catalog_sections_variant_fk",
            "target_id=? AND course_string=? AND fingerprint=?",
        ),
    ] {
        let explain = format!("EXPLAIN QUERY PLAN {sql}");
        let mut statement = connection.prepare(&explain).expect("prepare cascade plan");
        let details = statement
            .query_map(["SYNTHETIC_SCOPE"], |row| row.get::<_, String>(3))
            .expect("explain cascade")
            .collect::<Result<Vec<_>, _>>()
            .expect("collect cascade plan");
        let child_lookup = details
            .iter()
            .find(|detail| detail.contains(child_table))
            .unwrap_or_else(|| panic!("missing {child_table} lookup in {details:#?}"));
        assert!(
            child_lookup.contains(expected_index) && child_lookup.contains(full_lookup),
            "cascade child lookup must use the complete FK key: {child_lookup}"
        );
    }
}

#[test]
fn fresh_schema_has_no_catalog_seed_and_migration_checksum_is_strict() {
    let temp = TempDir::new().expect("temp dir");
    let path = db_path(&temp);
    let storage = OperationalStorage::open(&path).expect("initialize schema");
    assert_eq!(storage.migration_records().expect("migrations").len(), 7);
    assert!(storage.discovered_targets().expect("targets").is_empty());
    let discovery = storage.discovery_state().expect("discovery state");
    assert_eq!((discovery.term_count, discovery.campus_count), (0, 0));
    assert!(
        storage
            .operational_table_names()
            .expect("table names")
            .contains(&"catalog_course_fts".to_owned())
    );
    drop(storage);

    let connection = Connection::open(&path).expect("open raw test connection");
    connection
        .execute(
            "UPDATE bcsp_operational_migrations SET sha256 = ?1 WHERE migration_id = 1",
            ["0".repeat(64)],
        )
        .expect("tamper checksum");
    drop(connection);
    assert!(matches!(
        OperationalStorage::open(&path),
        Err(StorageError::MigrationChecksumMismatch { migration_id: 1 })
    ));
}

#[test]
fn catalog_variant_fk_child_indexes_cover_fresh_and_v2_upgraded_databases() {
    let fresh = TempDir::new().expect("fresh temp dir");
    let fresh_path = db_path(&fresh);
    drop(OperationalStorage::open(&fresh_path).expect("initialize fresh schema"));
    assert_catalog_variant_fk_indexes(&fresh_path);

    let upgraded = TempDir::new().expect("upgrade temp dir");
    let upgraded_path = db_path(&upgraded);
    let connection = Connection::open(&upgraded_path).expect("raw v2 database");
    for (migration_id, name, sql) in [
        (
            1,
            "operational_catalog",
            include_str!("../migrations/0001_operational_catalog.sql"),
        ),
        (
            2,
            "operational_open",
            include_str!("../migrations/0002_operational_open.sql"),
        ),
    ] {
        connection
            .execute_batch(sql)
            .expect("apply legacy migration");
        connection
            .execute(
                "INSERT INTO bcsp_operational_migrations
                    (migration_id, name, sha256, applied_at)
                 VALUES (?1, ?2, ?3, ?4)",
                (
                    migration_id,
                    name,
                    format!("{:x}", Sha256::digest(sql.as_bytes())),
                    COMPLETED,
                ),
            )
            .expect("record legacy migration");
    }
    drop(connection);

    let storage = OperationalStorage::open(&upgraded_path).expect("upgrade v2 database");
    let migrations = storage.migration_records().expect("upgraded migrations");
    assert_eq!(migrations.len(), 7);
    assert_eq!(migrations[2].name, "catalog_variant_fk_indexes");
    drop(storage);
    assert_catalog_variant_fk_indexes(&upgraded_path);
}

#[test]
fn v3_upgrade_closes_legacy_fatal_origin_without_deleting_online_history() {
    let temp = TempDir::new().expect("upgrade temp dir");
    let path = db_path(&temp);
    let connection = Connection::open(&path).expect("raw v3 database");
    for (migration_id, name, sql) in [
        (
            1,
            "operational_catalog",
            include_str!("../migrations/0001_operational_catalog.sql"),
        ),
        (
            2,
            "operational_open",
            include_str!("../migrations/0002_operational_open.sql"),
        ),
        (
            3,
            "catalog_variant_fk_indexes",
            include_str!("../migrations/0003_catalog_variant_fk_indexes.sql"),
        ),
    ] {
        connection
            .execute_batch(sql)
            .expect("apply legacy migration");
        connection
            .execute(
                "INSERT INTO bcsp_operational_migrations
                    (migration_id, name, sha256, applied_at)
                 VALUES (?1, ?2, ?3, ?4)",
                (
                    migration_id,
                    name,
                    format!("{:x}", Sha256::digest(sql.as_bytes())),
                    COMPLETED,
                ),
            )
            .expect("record legacy migration");
    }
    connection
        .execute(
            "INSERT INTO catalog_targets
                (target_id, term_id, campus_code, created_at, updated_at)
             VALUES ('legacy-online', '92026', 'ONLINE_NB', ?1, ?1)",
            [COMPLETED],
        )
        .expect("seed retained ONLINE history");
    connection
        .execute(
            "INSERT INTO catalog_targets
                (target_id, term_id, campus_code, created_at, updated_at,
                 current_content_version)
             VALUES ('92026/NB', '92026', 'NB', ?1, ?1, 1)",
            [COMPLETED],
        )
        .expect("seed matching Catalog snapshot");
    connection
        .execute(
            "INSERT INTO open_batch_state
                (target_id, last_attempt_sequence, last_observation_sequence,
                 current_catalog_content_version, lkg_attempt_id,
                 lkg_observation_sequence, lkg_observed_at,
                 lkg_canonical_set_sha256, lkg_state_sha256,
                 last_attempt_id, last_attempt_at, last_success_at)
             VALUES (
                 '92026/NB', 1, 1, 1,
                 '00000000-0000-4000-8000-000000000123',
                 1, ?1, ?2, ?3,
                 '00000000-0000-4000-8000-000000000123', ?1, ?1
             )",
            [COMPLETED, HASH_A, HASH_B],
        )
        .expect("seed matching Open LKG snapshot");
    connection
        .execute(
            "INSERT INTO open_origin_state
                (origin_id, circuit_state, reason_code, opened_at, retry_at,
                 diagnostic_recheck_required, updated_at)
             VALUES ('rutgers', 'FATAL_DIAGNOSTIC', 'LEGACY_FATAL', ?1, NULL, 1, ?1)",
            [COMPLETED],
        )
        .expect("seed legacy fatal origin");
    drop(connection);

    let storage = OperationalStorage::open(&path).expect("upgrade v3 database");
    assert_eq!(storage.migration_records().expect("migrations").len(), 7);
    let origin = storage
        .open_origin_state("rutgers")
        .expect("origin state")
        .expect("retained origin row");
    assert_eq!(origin.circuit_state.as_str(), "CLOSED");
    assert_eq!(origin.reason_code, None);
    assert_eq!(origin.opened_at, None);
    assert_eq!(origin.retry_at, None);
    assert!(!origin.diagnostic_recheck_required);
    assert!(
        storage
            .complete_target_snapshot_state(&target("92026", "NB"))
            .expect("matching complete snapshot state")
            .ready,
        "a matching RC2 Catalog/Open snapshot remains READY",
    );
    drop(storage);

    let connection = Connection::open(&path).expect("inspect upgraded database");
    let online_rows: i64 = connection
        .query_row(
            "SELECT count(*) FROM catalog_targets
             WHERE term_id = '92026' AND campus_code = 'ONLINE_NB'",
            [],
            |row| row.get(0),
        )
        .expect("retained ONLINE row count");
    assert_eq!(online_rows, 1, "migration retains historical ONLINE rows");
}

#[test]
fn migration_prefix_name_future_and_transaction_rollback_are_fail_closed() {
    let prefix_temp = TempDir::new().expect("temp dir");
    let prefix_path = db_path(&prefix_temp);
    let first = OperationalStorage::open(&prefix_path).expect("initial prefix");
    let records = first.migration_records().expect("records");
    drop(first);
    let reopened = OperationalStorage::open(&prefix_path).expect("exact prefix reopens");
    assert_eq!(reopened.migration_records().expect("records"), records);
    drop(reopened);

    let name_temp = TempDir::new().expect("temp dir");
    let name_path = db_path(&name_temp);
    drop(OperationalStorage::open(&name_path).expect("initialize"));
    let connection = Connection::open(&name_path).expect("raw connection");
    connection
        .execute(
            "UPDATE bcsp_operational_migrations SET name = 'renamed' WHERE migration_id = 1",
            [],
        )
        .expect("name drift");
    drop(connection);
    assert!(matches!(
        OperationalStorage::open(&name_path),
        Err(StorageError::MigrationNameMismatch { migration_id: 1 })
    ));

    let future_temp = TempDir::new().expect("temp dir");
    let future_path = db_path(&future_temp);
    drop(OperationalStorage::open(&future_path).expect("initialize"));
    let connection = Connection::open(&future_path).expect("raw connection");
    connection
        .execute(
            "INSERT INTO bcsp_operational_migrations
                (migration_id, name, sha256, applied_at)
             VALUES (8, 'future', ?1, ?2)",
            [HASH_A, COMPLETED],
        )
        .expect("future migration");
    drop(connection);
    assert!(matches!(
        OperationalStorage::open(&future_path),
        Err(StorageError::UnknownMigration {
            migration_id: 8,
            ..
        })
    ));

    let rollback_temp = TempDir::new().expect("temp dir");
    let rollback_path = db_path(&rollback_temp);
    let connection = Connection::open(&rollback_path).expect("raw connection");
    connection
        .execute_batch("CREATE TABLE catalog_targets(conflict INTEGER);")
        .expect("install migration fault");
    drop(connection);
    assert!(OperationalStorage::open(&rollback_path).is_err());
    let connection = Connection::open(&rollback_path).expect("inspect rollback");
    let ledger_exists: bool = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema
             WHERE type = 'table' AND name = 'bcsp_operational_migrations')",
            [],
            |row| row.get(0),
        )
        .expect("ledger existence");
    assert!(!ledger_exists, "failed migration must roll back its ledger");
}

#[test]
fn catalog_content_v1_golden_and_forgery_guard_are_stable() {
    let empty_target = target("SUMMER_2027", "EMPTY_DYNAMIC");
    assert_eq!(
        catalog_content_sha256_v1(&empty_target, &CatalogSnapshot::default()).expect("golden hash"),
        "974dd79063cee76db14c356cbe4bb7707f7f53c30d1234f02187d3101c7f246f"
    );

    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let scope = target("FALL_2026", "SYNTHETIC_HASH_GUARD");
    let mut forged = refresh_command(
        "forged.1",
        &scope,
        snapshot(&scope, "00001", "Synthetic Hash Guard"),
        b"synthetic-hash-guard",
    );
    let group = &forged.snapshot.course_groups[0];
    let variant = &forged.snapshot.course_variants[0];
    let section = &forged.snapshot.sections[0];
    forged.semantic_content_sha256 = sha256(
        &serde_json::to_vec(&json!({
            "schema": "catalog-content-v1",
            "target": {
                "term": scope.term().as_str(),
                "campus": scope.campus().as_str(),
            },
            "groups": [{
                "courseString": group.key.course_string().as_str(),
                "variants": [{
                    "fingerprint": variant.key.fingerprint().as_str(),
                    "canonicalHash": variant.canonical_sha256,
                    "sections": [{
                        "term": section.key.term().as_str(),
                        "campus": section.key.campus().as_str(),
                        "index": section.key.index().as_str(),
                        "semanticHash": section.canonical_sha256,
                    }],
                }],
            }],
        }))
        .expect("legacy untagged preimage"),
    );
    assert!(matches!(
        storage.apply_catalog_refresh(
            forged,
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        ),
        Err(StorageError::InvalidCommand {
            field: "semantic_content_sha256",
            ..
        })
    ));
    let state = storage
        .target_state(&scope)
        .expect("state")
        .expect("target");
    assert_eq!(state.current_content_version, 0);
    assert_eq!(state.last_success_observation_id, None);
}

#[test]
fn publication_preserves_presence_and_handles_unchanged_suspect_and_valid_empty() {
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let scope = target("FALL_2026", "NB");
    let first = storage
        .apply_catalog_refresh(
            refresh_command(
                "catalog.1",
                &scope,
                snapshot(&scope, "00001", "Synthetic Systems"),
                b"synthetic-upstream-one",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("first publish");
    assert!(matches!(
        first,
        PublishOutcome::AppliedChanged {
            content_version: 1,
            ..
        }
    ));
    assert_eq!(storage.staged_payload_count().expect("staging count"), 0);
    let stored_sections = storage.sections(&scope).expect("stored sections");
    assert_eq!(stored_sections.len(), 1);
    assert_eq!(stored_sections[0].delivery_modality, "UNKNOWN");
    assert_eq!(stored_sections[0].synchronicity, "UNKNOWN");
    assert_eq!(
        storage
            .search_course_variants(&scope, 1, &text_tokens(&["computing"]))
            .expect("FTS search")
            .hits
            .len(),
        1
    );
    assert_eq!(
        storage
            .search_course_variants(&scope, 1, &text_tokens(&["计算机"]))
            .expect("Unicode FTS search")
            .hits
            .len(),
        1
    );
    let facts = storage.canonical_facts(&scope).expect("canonical facts");
    assert_eq!(facts.len(), 4);
    for fact in facts {
        assert_eq!(fact.canonical_facts["missing"]["presence"], "MISSING");
        assert!(fact.canonical_facts["null"].is_null());
        assert_eq!(fact.canonical_facts["empty"], "");
        assert_eq!(fact.canonical_facts["value"], "synthetic-value");
    }
    assert!(
        storage
            .integrity_report(&scope)
            .expect("integrity")
            .is_clean()
    );

    let mut same_serving_projection = snapshot(&scope, "00001", "Synthetic Systems");
    same_serving_projection.course_variants[0].raw_multiplicity = 7;
    same_serving_projection.occurrences[0].raw_sha256 = HASH_A.to_owned();
    same_serving_projection.provenance[0].source_sha256 = HASH_B.to_owned();
    same_serving_projection.provenance[0].detail = json!({"attempt": "different"});
    let unchanged = storage
        .apply_catalog_refresh(
            refresh_command(
                "catalog.2",
                &scope,
                same_serving_projection,
                b"different-raw-body-same-normalized-content",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("unchanged publish");
    assert!(matches!(
        unchanged,
        PublishOutcome::AppliedUnchanged {
            content_version: 1,
            ..
        }
    ));

    let suspect = storage
        .apply_catalog_refresh(
            refresh_command(
                "catalog.3",
                &scope,
                CatalogSnapshot::default(),
                b"synthetic-empty-array",
            ),
            EmptySnapshotDecision::RetainLastKnownGood,
            1,
        )
        .expect("suspect empty");
    assert!(matches!(
        suspect,
        PublishOutcome::SuspectEmptyRetained {
            content_version: 1,
            ..
        }
    ));
    assert_eq!(
        storage
            .serving_section_keys(&scope)
            .expect("sections")
            .len(),
        1
    );
    let state = storage
        .target_state(&scope)
        .expect("state")
        .expect("target");
    assert_eq!(state.last_success_observation_id, Some(trace("catalog.3")));
    assert_eq!(
        state.last_published_observation_id,
        Some(trace("catalog.2"))
    );

    let confirmed_empty = storage
        .apply_catalog_refresh(
            refresh_command(
                "catalog.4",
                &scope,
                CatalogSnapshot::default(),
                b"independent-synthetic-empty-array",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("confirmed empty");
    assert!(matches!(
        confirmed_empty,
        PublishOutcome::SuspectEmptyRetained {
            content_version: 1,
            ..
        }
    ));
    assert_eq!(
        storage
            .target_state(&scope)
            .expect("state")
            .expect("target")
            .pending_empty_observation_id,
        Some(trace("catalog.4"))
    );
    assert_eq!(
        storage
            .serving_section_keys(&scope)
            .expect("sections")
            .len(),
        1
    );
    storage
        .begin_refresh_attempt(&BeginRefreshAttemptCommand {
            observation_id: trace("catalog.5-transport-failure"),
            target: scope.clone(),
            started_at: STARTED.to_owned(),
            source_content_sha256: None,
            source_bytes: None,
        })
        .expect("begin post-empty failure");
    storage
        .finish_refresh_failure(&FinishRefreshFailureCommand {
            observation_id: trace("catalog.5-transport-failure"),
            completed_at: COMPLETED.to_owned(),
            stage: RefreshFailureStage::Transport,
            source_content_sha256: None,
            source_bytes: None,
            error_code: "UPSTREAM_TIMEOUT".to_owned(),
            diagnostic_token: None,
        })
        .expect("finish post-empty failure");
    let after_failure = storage
        .target_state(&scope)
        .expect("state")
        .expect("target");
    assert_eq!(
        after_failure.pending_empty_observation_id,
        Some(trace("catalog.4"))
    );
    assert_eq!(
        after_failure
            .pending_empty
            .as_ref()
            .map(|observation| observation.observation_id),
        Some(trace("catalog.4"))
    );
    assert!(
        storage
            .integrity_report(&scope)
            .expect("integrity")
            .is_clean()
    );

    let first_empty_scope = target("SPRING_2028", "EMPTY_CAMPUS");
    assert!(matches!(
        storage.apply_catalog_refresh(
            refresh_command(
                "catalog.first-empty.forged",
                &first_empty_scope,
                CatalogSnapshot::default(),
                b"forged-selector-proof",
            ),
            EmptySnapshotDecision::AcceptInitialSelectorConfirmedEmpty(
                InitialEmptyProof::CurrentSelectorMembership,
            ),
            1,
        ),
        Err(StorageError::InvalidCommand {
            field: "empty_decision",
            ..
        })
    ));
    assert_eq!(
        storage
            .target_state(&first_empty_scope)
            .expect("forged state")
            .expect("forged target")
            .current_content_version,
        0
    );
    storage
        .apply_discovery_refresh(DiscoveryRefreshCommand {
            observation_id: trace("discovery.selector-for-empty"),
            started_at: STARTED.to_owned(),
            completed_at: COMPLETED.to_owned(),
            snapshot: discovery_snapshot(),
        })
        .expect("publish selector membership before accepting initial empty");
    let first_empty = storage
        .apply_catalog_refresh(
            refresh_command(
                "catalog.first-empty",
                &first_empty_scope,
                CatalogSnapshot::default(),
                b"first-valid-empty",
            ),
            EmptySnapshotDecision::AcceptInitialSelectorConfirmedEmpty(
                InitialEmptyProof::CurrentSelectorMembership,
            ),
            1,
        )
        .expect("first valid empty");
    assert!(matches!(
        first_empty,
        PublishOutcome::InitialValidEmpty { content_version: 1 }
    ));
    assert_eq!(
        storage
            .refresh_observation(&trace("catalog.first-empty"))
            .expect("observation")
            .expect("present")
            .status,
        RefreshStatus::EmptyValidInitial
    );
    assert!(matches!(
        storage
            .apply_catalog_refresh(
                refresh_command(
                    "catalog.first-empty.unchanged",
                    &first_empty_scope,
                    CatalogSnapshot::default(),
                    b"same-normalized-empty",
                ),
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
                1,
            )
            .expect("existing unchanged empty"),
        PublishOutcome::AppliedUnchanged {
            content_version: 1,
            ..
        }
    ));

    let unproved_scope = target("SUMMER_2027", "UNPROVED_EMPTY");
    assert!(
        storage
            .apply_catalog_refresh(
                refresh_command(
                    "catalog.unproved-empty",
                    &unproved_scope,
                    CatalogSnapshot::default(),
                    b"unproved-first-empty",
                ),
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
                1,
            )
            .is_err()
    );
    let unproved = storage
        .target_state(&unproved_scope)
        .expect("state")
        .expect("target");
    assert_eq!(unproved.current_content_version, 0);
    assert_eq!(unproved.pending_empty_observation_id, None);
}

#[test]
fn replacement_is_target_scoped_and_fts_version_matches_serving_version() {
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let target_a = target("FALL_2026", "NB");
    let target_b = target("FALL_2026", "CAMDEN_NEW");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "target-a.1",
                &target_a,
                snapshot(&target_a, "00001", "Alpha"),
                b"alpha-1",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("target A");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "target-b.1",
                &target_b,
                snapshot(&target_b, "00002", "Beta"),
                b"beta-1",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("target B");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "target-a.2",
                &target_a,
                snapshot(&target_a, "00003", "Alpha Replacement"),
                b"alpha-2",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("replace target A");

    assert_eq!(
        storage.serving_section_keys(&target_a).expect("A sections")[0]
            .index()
            .as_str(),
        "00003"
    );
    assert_eq!(
        storage.serving_section_keys(&target_b).expect("B sections")[0]
            .index()
            .as_str(),
        "00002"
    );
    assert_eq!(
        storage
            .target_state(&target_a)
            .expect("A state")
            .expect("A")
            .current_content_version,
        2
    );
    assert_eq!(
        storage
            .target_state(&target_b)
            .expect("B state")
            .expect("B")
            .current_content_version,
        1
    );
    assert!(
        storage
            .integrity_report(&target_a)
            .expect("A integrity")
            .is_clean()
    );
    assert!(
        storage
            .integrity_report(&target_b)
            .expect("B integrity")
            .is_clean()
    );
}

#[test]
fn variants_in_one_group_retain_independent_course_facts() {
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let scope = target("FALL_2026", "SYNTHETIC_MULTI");
    let mut normalized = snapshot(&scope, "00001", "Synthetic Variant Alpha");
    let group = normalized.course_variants[0].key.group().clone();
    normalized.course_variants.push(StoredCourseVariant {
        key: CourseVariantKey::try_new(group, &format!("v1:{HASH_B}")).expect("second variant key"),
        subject_code: Some("SYN".to_owned()),
        course_number: Some("001".to_owned()),
        title: Some("Synthetic Variant Beta".to_owned()),
        description: None,
        credits_summary: Some("VARIABLE".to_owned()),
        supplement: None,
        search_document: "SYN:CAT:001 Synthetic Variant Beta 计算机".to_owned(),
        canonical_sha256: HASH_B.to_owned(),
        raw_multiplicity: 1,
        canonical_facts: json!({
            "title": {"presence": "VALUE", "value": "Synthetic Variant Beta"},
            "description": {"presence": "MISSING"}
        }),
    });
    storage
        .apply_catalog_refresh(
            refresh_command("variants.1", &scope, normalized, b"two-synthetic-variants"),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("publish two variants");
    let variants = storage.course_variants(&scope).expect("variants");
    assert_eq!(variants.len(), 2);
    assert_eq!(
        variants
            .iter()
            .map(|variant| variant.title.as_deref())
            .collect::<Vec<_>>(),
        vec![
            Some("Synthetic Variant Alpha"),
            Some("Synthetic Variant Beta")
        ]
    );
    assert_eq!(variants[0].description.as_deref(), Some(""));
    assert_eq!(variants[1].description, None);
    assert_eq!(
        storage
            .search_course_variants(&scope, 1, &text_tokens(&["计算机"]))
            .expect("FTS variants")
            .hits
            .len(),
        2
    );
    assert!(
        storage
            .integrity_report(&scope)
            .expect("integrity")
            .is_clean()
    );
}

#[test]
fn course_text_search_tokens_are_nonempty_bounded_and_control_free() {
    assert!(matches!(
        CourseTextSearchTokens::try_new(std::iter::empty::<&str>()),
        Err(StorageError::InvalidCommand {
            field: "course_text_search_tokens",
            ..
        })
    ));
    for invalid in ["", "   ", "line\nbreak", "nul\0byte", "\"", "()", "***"] {
        assert!(matches!(
            CourseTextSearchTokens::try_new([invalid]),
            Err(StorageError::InvalidCommand {
                field: "course_text_search_tokens",
                ..
            })
        ));
    }
    assert!(CourseTextSearchTokens::try_new(vec!["token"; 32]).is_ok());
    assert!(matches!(
        CourseTextSearchTokens::try_new(vec!["token"; 33]),
        Err(StorageError::InvalidCommand { .. })
    ));
    assert!(matches!(
        CourseTextSearchTokens::try_new(["x".repeat(129)]),
        Err(StorageError::InvalidCommand { .. })
    ));
    assert!(matches!(
        CourseTextSearchTokens::try_new(vec!["x".repeat(128); 5]),
        Err(StorageError::InvalidCommand { .. })
    ));
    let trimmed = CourseTextSearchTokens::try_new(["  computing  ", "计算机"])
        .expect("trimmed Unicode literal tokens");
    assert_eq!(trimmed.len(), 2);
    assert!(!trimmed.is_empty());
}

#[test]
fn fts_search_uses_literal_token_and_and_returns_stable_variant_rank() {
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let scope = target("FALL_2026", "FTS_LITERAL");
    let mut normalized = snapshot(&scope, "00001", "Literal Alpha Beta");
    normalized.course_variants[0].search_document =
        "SYN:CAT:001 shared alpha OR beta quoted value 计算机".to_owned();
    let group = normalized.course_variants[0].key.group().clone();
    normalized.course_variants.push(StoredCourseVariant {
        key: CourseVariantKey::try_new(group.clone(), &format!("v1:{HASH_B}"))
            .expect("second variant key"),
        subject_code: Some("SYN".to_owned()),
        course_number: Some("001".to_owned()),
        title: Some("Literal Alpha".to_owned()),
        description: None,
        credits_summary: None,
        supplement: None,
        search_document: "SYN:CAT:001 shared alpha only".to_owned(),
        canonical_sha256: HASH_B.to_owned(),
        raw_multiplicity: 1,
        canonical_facts: json!({"title": "Literal Alpha"}),
    });
    normalized.course_variants.push(StoredCourseVariant {
        key: CourseVariantKey::try_new(group, &format!("v1:{HASH_C}")).expect("third variant key"),
        subject_code: Some("SYN".to_owned()),
        course_number: Some("001".to_owned()),
        title: Some("Literal Beta".to_owned()),
        description: None,
        credits_summary: None,
        supplement: None,
        search_document: "SYN:CAT:001 shared beta only".to_owned(),
        canonical_sha256: HASH_C.to_owned(),
        raw_multiplicity: 1,
        canonical_facts: json!({"title": "Literal Beta"}),
    });
    storage
        .apply_catalog_refresh(
            refresh_command(
                "fts.literal.1",
                &scope,
                normalized,
                b"fts-literal-synthetic",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("publish FTS fixture");

    let keyword_terms = storage
        .published_course_fts_terms(&scope, 1, Some("ALP"), 10)
        .expect("published FTS vocabulary is target/version scoped");
    assert_eq!(keyword_terms, vec!["alpha"]);
    assert!(
        storage
            .published_course_fts_term_exists(&scope, 1, "alpha")
            .expect("exact published term exists")
    );
    assert!(
        storage
            .published_course_fts_term_exists(&scope, 1, "ALPHA")
            .expect("exact published term lookup is case-insensitive")
    );
    assert!(
        !storage
            .published_course_fts_term_exists(&scope, 1, "absent")
            .expect("absent exact published term")
    );
    assert!(matches!(
        storage.published_course_fts_terms(&scope, 2, None, 10),
        Err(StorageError::CatalogContentVersionMismatch {
            requested: 2,
            current: 1
        })
    ));
    assert!(matches!(
        storage.published_course_fts_term_exists(&scope, 2, "alpha"),
        Err(StorageError::CatalogContentVersionMismatch {
            requested: 2,
            current: 1
        })
    ));

    let and_result = storage
        .search_course_variants(&scope, 1, &text_tokens(&["alpha", "beta"]))
        .expect("literal token AND");
    assert_eq!(and_result.target, scope);
    assert_eq!(and_result.content_version, 1);
    assert_eq!(and_result.hits.len(), 1);
    assert_eq!(
        and_result.hits[0].key.fingerprint().as_str(),
        format!("v1:{HASH_A}")
    );

    let operator_result = storage
        .search_course_variants(&scope, 1, &text_tokens(&["OR"]))
        .expect("operator word remains literal");
    assert_eq!(operator_result.hits.len(), 1);
    assert_eq!(operator_result.hits[0].key, and_result.hits[0].key);
    let quoted_phrase = storage
        .search_course_variants(&scope, 1, &text_tokens(&["alpha\" OR beta"]))
        .expect("embedded quote remains literal");
    assert_eq!(quoted_phrase.hits.len(), 1);
    assert_eq!(quoted_phrase.hits[0].key, and_result.hits[0].key);
    assert_eq!(
        storage
            .search_course_variants(&scope, 1, &text_tokens(&["计算机"]))
            .expect("Unicode literal")
            .hits,
        and_result.hits
    );

    let ranked_once = storage
        .search_course_variants(&scope, 1, &text_tokens(&["shared"]))
        .expect("ranked search");
    let ranked_twice = storage
        .search_course_variants(&scope, 1, &text_tokens(&["shared"]))
        .expect("repeat ranked search");
    assert_eq!(ranked_once, ranked_twice);
    assert_eq!(
        ranked_once
            .hits
            .iter()
            .map(|hit| hit.fts_rank)
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
}

#[test]
fn fts_search_is_strictly_bound_to_target_and_published_content_version() {
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let unpublished = target("FALL_2026", "FTS_UNPUBLISHED");
    assert!(matches!(
        storage.search_course_variants(&unpublished, 1, &text_tokens(&["synthetic"])),
        Err(StorageError::CatalogTargetNotPublished)
    ));
    assert!(matches!(
        storage.search_course_variants(&unpublished, 0, &text_tokens(&["synthetic"])),
        Err(StorageError::InvalidCommand {
            field: "content_version",
            ..
        })
    ));

    let target_a = target("FALL_2026", "FTS_A");
    let target_b = target("FALL_2026", "FTS_B");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "fts.target.a.1",
                &target_a,
                snapshot(&target_a, "00001", "Shared Legacy Alpha"),
                b"fts-target-a-1",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("publish A v1");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "fts.target.b.1",
                &target_b,
                snapshot(&target_b, "00002", "Shared Stable Beta"),
                b"fts-target-b-1",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("publish B v1");
    let a_v1 = storage
        .search_course_variants(&target_a, 1, &text_tokens(&["legacy"]))
        .expect("A v1 search");
    assert_eq!(a_v1.hits.len(), 1);
    assert_eq!(a_v1.hits[0].key.group().campus(), target_a.campus());
    assert!(
        storage
            .search_course_variants(&target_b, 1, &text_tokens(&["legacy"]))
            .expect("B cannot see A")
            .hits
            .is_empty()
    );

    storage
        .apply_catalog_refresh(
            refresh_command(
                "fts.target.a.2",
                &target_a,
                snapshot(&target_a, "00003", "Shared Replacement Gamma"),
                b"fts-target-a-2",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("replace A");
    assert!(matches!(
        storage.search_course_variants(&target_a, 1, &text_tokens(&["legacy"])),
        Err(StorageError::CatalogContentVersionMismatch {
            requested: 1,
            current: 2
        })
    ));
    assert!(
        storage
            .search_course_variants(&target_a, 2, &text_tokens(&["legacy"]))
            .expect("old A row removed")
            .hits
            .is_empty()
    );
    assert_eq!(
        storage
            .search_course_variants(&target_a, 2, &text_tokens(&["replacement"]))
            .expect("A v2 search")
            .hits
            .len(),
        1
    );
    assert_eq!(
        storage
            .search_course_variants(&target_b, 1, &text_tokens(&["stable"]))
            .expect("B remains on v1")
            .hits
            .len(),
        1
    );
    assert!(
        storage
            .integrity_report(&target_a)
            .expect("A parity")
            .is_clean()
    );
    assert!(
        storage
            .integrity_report(&target_b)
            .expect("B parity")
            .is_clean()
    );
}

#[test]
fn interrupted_staging_is_recovered_without_mutating_last_known_good() {
    let temp = TempDir::new().expect("temp dir");
    let path = db_path(&temp);
    let scope = target("FALL_2026", "NB");
    let mut storage = OperationalStorage::open(&path).expect("storage");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "stable.1",
                &scope,
                snapshot(&scope, "00001", "Stable"),
                b"stable-body",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("stable publish");
    storage
        .stage_catalog_refresh(refresh_command(
            "interrupted.2",
            &scope,
            snapshot(&scope, "00002", "Interrupted"),
            b"raw-body-must-be-cleaned",
        ))
        .expect("stage replacement");
    assert_eq!(storage.staged_payload_count().expect("staged"), 1);
    drop(storage);

    let storage = OperationalStorage::open(&path).expect("recover on reopen");
    assert_eq!(storage.staged_payload_count().expect("staged"), 0);
    assert_eq!(
        storage
            .refresh_observation(&trace("interrupted.2"))
            .expect("observation")
            .expect("present")
            .status,
        RefreshStatus::Interrupted
    );
    assert_eq!(
        storage.serving_section_keys(&scope).expect("sections")[0]
            .index()
            .as_str(),
        "00001"
    );
    assert_eq!(
        storage
            .target_state(&scope)
            .expect("state")
            .expect("target")
            .current_content_version,
        1
    );
    let restarted = storage
        .target_state(&scope)
        .expect("restart state")
        .expect("target");
    assert_eq!(
        restarted
            .last_attempt
            .as_ref()
            .map(|observation| observation.status),
        Some(RefreshStatus::Interrupted)
    );
    let published = restarted.last_published.expect("published freshness point");
    assert_eq!(published.observation_id, trace("stable.1"));
    assert_eq!(published.completed_at.as_deref(), Some(COMPLETED));
    assert_eq!(published.counts.sections, 1);
    assert_eq!(published.resulting_content_version, Some(1));
    assert_eq!(
        published.source_content_sha256,
        Some(sha256(b"stable-body"))
    );
}

#[test]
fn pending_empty_and_safe_freshness_points_survive_restart() {
    let temp = TempDir::new().expect("temp dir");
    let path = db_path(&temp);
    let scope = target("FALL_2026", "SYNTHETIC_RESTART");
    let mut storage = OperationalStorage::open(&path).expect("storage");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "pending.1",
                &scope,
                snapshot(&scope, "00001", "Synthetic Restart"),
                b"synthetic-nonempty",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("publish LKG");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "pending.2",
                &scope,
                CatalogSnapshot::default(),
                b"synthetic-suspect-empty",
            ),
            EmptySnapshotDecision::RetainLastKnownGood,
            1,
        )
        .expect("retain LKG");
    drop(storage);

    let mut storage = OperationalStorage::open(&path).expect("restart");
    let state = storage
        .target_state(&scope)
        .expect("state")
        .expect("target");
    assert_eq!(state.current_content_version, 1);
    let pending = state.pending_empty.expect("pending empty freshness point");
    assert_eq!(pending.observation_id, trace("pending.2"));
    assert_eq!(pending.status, RefreshStatus::EmptySuspectRetained);
    assert_eq!(pending.counts, Default::default());
    assert_eq!(pending.completed_at.as_deref(), Some(COMPLETED));
    let published = state.last_published.expect("last published point");
    assert_eq!(published.observation_id, trace("pending.1"));
    assert_eq!(published.counts.sections, 1);
    let snapshot = storage
        .published_catalog_snapshot(&scope)
        .expect("stale published snapshot")
        .expect("last-known-good publication");
    assert_eq!(snapshot.publication.observation_id, trace("pending.1"));
    assert_eq!(snapshot.snapshot.sections.len(), 1);
}

#[test]
fn publish_fault_rolls_back_replacement_and_records_only_safe_failure_metadata() {
    let temp = TempDir::new().expect("temp dir");
    let path = db_path(&temp);
    let scope = target("FALL_2026", "NB");
    let mut storage = OperationalStorage::open(&path).expect("storage");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "rollback.1",
                &scope,
                snapshot(&scope, "00001", "Stable"),
                b"stable-body",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("stable publish");
    storage
        .stage_catalog_refresh(refresh_command(
            "rollback.2",
            &scope,
            snapshot(&scope, "00002", "Must Roll Back"),
            b"synthetic-forbidden-raw-body",
        ))
        .expect("stage replacement");

    let fault_connection = Connection::open(&path).expect("fault connection");
    fault_connection
        .execute_batch(
            "CREATE TRIGGER synthetic_publish_fault
             BEFORE INSERT ON catalog_course_groups
             BEGIN SELECT RAISE(ABORT, 'synthetic fault'); END;",
        )
        .expect("install fault trigger");
    drop(fault_connection);
    assert!(
        storage
            .publish_staged_refresh(
                &trace("rollback.2"),
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
                1,
            )
            .is_err()
    );
    assert_eq!(
        storage.serving_section_keys(&scope).expect("sections")[0]
            .index()
            .as_str(),
        "00001"
    );
    assert_eq!(
        storage
            .target_state(&scope)
            .expect("state")
            .expect("target")
            .current_content_version,
        1
    );
    let failed = storage
        .refresh_observation(&trace("rollback.2"))
        .expect("observation")
        .expect("present");
    assert_eq!(failed.status, RefreshStatus::Failed);
    assert_eq!(failed.error_code.as_deref(), Some("STORAGE_PUBLISH_FAILED"));
    assert_eq!(failed.diagnostic_token, None);
    assert_eq!(storage.staged_payload_count().expect("staging"), 0);
}

#[test]
fn transport_failure_has_no_fallback_and_rejects_raw_diagnostic_detail() {
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let scope = target("FUTURE_2030", "DYNAMIC_X");
    storage
        .begin_refresh_attempt(&BeginRefreshAttemptCommand {
            observation_id: trace("transport.1"),
            target: scope.clone(),
            started_at: STARTED.to_owned(),
            source_content_sha256: None,
            source_bytes: None,
        })
        .expect("begin attempt");
    let rejected = storage.finish_refresh_failure(&FinishRefreshFailureCommand {
        observation_id: trace("transport.1"),
        completed_at: COMPLETED.to_owned(),
        stage: RefreshFailureStage::Transport,
        source_content_sha256: None,
        source_bytes: None,
        error_code: "UPSTREAM_TIMEOUT".to_owned(),
        diagnostic_token: Some("{\"rawPayload\":\"forbidden\"}".to_owned()),
    });
    assert!(matches!(rejected, Err(StorageError::InvalidCommand { .. })));
    storage
        .finish_refresh_failure(&FinishRefreshFailureCommand {
            observation_id: trace("transport.1"),
            completed_at: COMPLETED.to_owned(),
            stage: RefreshFailureStage::Transport,
            source_content_sha256: None,
            source_bytes: None,
            error_code: "UPSTREAM_TIMEOUT".to_owned(),
            diagnostic_token: Some("trace.abc123".to_owned()),
        })
        .expect("safe failure");
    let state = storage
        .target_state(&scope)
        .expect("state")
        .expect("target");
    assert_eq!(state.current_content_version, 0);
    assert_eq!(state.last_success_observation_id, None);
    let observation = storage
        .refresh_observation(&trace("transport.1"))
        .expect("observation")
        .expect("present");
    assert_eq!(observation.error_code.as_deref(), Some("UPSTREAM_TIMEOUT"));
    assert_eq!(
        observation.diagnostic_token.as_deref(),
        Some("trace.abc123")
    );
}

#[test]
fn catalog_attempt_sequence_is_idempotent_durable_monotonic_and_target_scoped() {
    let temp = TempDir::new().expect("temp dir");
    let path = db_path(&temp);
    let target_a = target("FALL_2026", "SEQUENCE_A");
    let target_b = target("FALL_2026", "SEQUENCE_B");
    let mut storage = OperationalStorage::open(&path).expect("storage");

    let first = BeginRefreshAttemptCommand {
        observation_id: trace("sequence.a.failure.1"),
        target: target_a.clone(),
        started_at: STARTED.to_owned(),
        source_content_sha256: None,
        source_bytes: None,
    };
    storage.begin_refresh_attempt(&first).expect("first begin");
    storage
        .begin_refresh_attempt(&first)
        .expect("idempotent repeated begin");
    assert_eq!(
        storage
            .target_state(&target_a)
            .expect("target A state")
            .expect("target A")
            .last_attempt_sequence,
        1
    );
    assert_eq!(
        storage
            .refresh_observation(&trace("sequence.a.failure.1"))
            .expect("first observation")
            .expect("first present")
            .attempt_sequence,
        1
    );
    storage
        .finish_refresh_failure(&FinishRefreshFailureCommand {
            observation_id: trace("sequence.a.failure.1"),
            completed_at: COMPLETED.to_owned(),
            stage: RefreshFailureStage::Transport,
            source_content_sha256: None,
            source_bytes: None,
            error_code: "UPSTREAM_TIMEOUT".to_owned(),
            diagnostic_token: None,
        })
        .expect("first failure");

    storage
        .apply_catalog_refresh(
            refresh_command(
                "sequence.a.success.2",
                &target_a,
                snapshot(&target_a, "00001", "Sequence Success"),
                b"sequence-success",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("second successful attempt");
    assert_eq!(
        storage
            .refresh_observation(&trace("sequence.a.success.2"))
            .expect("second observation")
            .expect("second present")
            .attempt_sequence,
        2
    );

    storage
        .begin_refresh_attempt(&BeginRefreshAttemptCommand {
            observation_id: trace("sequence.b.failure.1"),
            target: target_b.clone(),
            started_at: STARTED.to_owned(),
            source_content_sha256: None,
            source_bytes: None,
        })
        .expect("target B first begin");
    storage
        .finish_refresh_failure(&FinishRefreshFailureCommand {
            observation_id: trace("sequence.b.failure.1"),
            completed_at: COMPLETED.to_owned(),
            stage: RefreshFailureStage::Schema,
            source_content_sha256: None,
            source_bytes: None,
            error_code: "SCHEMA_REJECTED".to_owned(),
            diagnostic_token: None,
        })
        .expect("target B failure");
    assert_eq!(
        storage
            .target_state(&target_b)
            .expect("target B state")
            .expect("target B")
            .last_attempt_sequence,
        1
    );
    drop(storage);

    let mut storage = OperationalStorage::open(&path).expect("restart storage");
    assert_eq!(
        storage
            .target_state(&target_a)
            .expect("restarted target A state")
            .expect("restarted target A")
            .last_attempt_sequence,
        2
    );
    storage
        .begin_refresh_attempt(&BeginRefreshAttemptCommand {
            observation_id: trace("sequence.a.failure.3"),
            target: target_a.clone(),
            started_at: STARTED.to_owned(),
            source_content_sha256: None,
            source_bytes: None,
        })
        .expect("post-restart third begin");
    storage
        .finish_refresh_failure(&FinishRefreshFailureCommand {
            observation_id: trace("sequence.a.failure.3"),
            completed_at: COMPLETED.to_owned(),
            stage: RefreshFailureStage::Transport,
            source_content_sha256: None,
            source_bytes: None,
            error_code: "UPSTREAM_TIMEOUT".to_owned(),
            diagnostic_token: None,
        })
        .expect("post-restart failure");
    assert_eq!(
        storage
            .target_state(&target_a)
            .expect("final target A state")
            .expect("final target A")
            .last_attempt_sequence,
        3
    );
    assert_eq!(
        storage
            .refresh_observation(&trace("sequence.a.failure.3"))
            .expect("third observation")
            .expect("third present")
            .attempt_sequence,
        3
    );
    assert_eq!(
        storage
            .target_state(&target_b)
            .expect("final target B state")
            .expect("final target B")
            .last_attempt_sequence,
        1
    );
}

#[test]
fn catalog_attempt_identity_and_source_metadata_are_immutable_across_retries() {
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let target_a = target("FALL_2026", "RETRY_A");
    let target_b = target("FALL_2026", "RETRY_B");
    let observation_id = trace("retry.catalog.immutable");
    let original = BeginRefreshAttemptCommand {
        observation_id,
        target: target_a.clone(),
        started_at: STARTED.to_owned(),
        source_content_sha256: Some(HASH_A.to_owned()),
        source_bytes: Some(17),
    };
    storage
        .begin_refresh_attempt(&original)
        .expect("initial begin");
    storage
        .begin_refresh_attempt(&original)
        .expect("exact retry is idempotent");

    let mut mismatched = original.clone();
    mismatched.started_at = "2026-07-14T00:00:02Z".to_owned();
    assert!(matches!(
        storage.begin_refresh_attempt(&mismatched),
        Err(StorageError::InvalidCommand {
            field: "started_at",
            ..
        })
    ));

    let mut mismatched = original.clone();
    mismatched.target = target_b.clone();
    assert!(matches!(
        storage.begin_refresh_attempt(&mismatched),
        Err(StorageError::ObservationNotStarted(id)) if id == observation_id
    ));
    assert!(
        storage
            .target_state(&target_b)
            .expect("mismatched target lookup")
            .is_none(),
        "a rejected retry must roll back creation of the mismatched target"
    );

    let mut mismatched = original.clone();
    mismatched.source_content_sha256 = Some(HASH_B.to_owned());
    assert!(matches!(
        storage.begin_refresh_attempt(&mismatched),
        Err(StorageError::InvalidCommand {
            field: "source_content_sha256",
            ..
        })
    ));

    let mut mismatched = original.clone();
    mismatched.source_bytes = Some(18);
    assert!(matches!(
        storage.begin_refresh_attempt(&mismatched),
        Err(StorageError::InvalidCommand {
            field: "source_bytes",
            ..
        })
    ));

    let observation = storage
        .refresh_observation(&observation_id)
        .expect("observation lookup")
        .expect("observation present");
    assert_eq!(observation.target, target_a);
    assert_eq!(observation.started_at, STARTED);
    assert_eq!(observation.source_content_sha256.as_deref(), Some(HASH_A));
    assert_eq!(observation.source_bytes, Some(17));
    assert_eq!(observation.status, RefreshStatus::Started);

    let fill_id = trace("retry.catalog.fill-once");
    let empty_metadata = BeginRefreshAttemptCommand {
        observation_id: fill_id,
        target: target_a.clone(),
        started_at: STARTED.to_owned(),
        source_content_sha256: None,
        source_bytes: None,
    };
    storage
        .begin_refresh_attempt(&empty_metadata)
        .expect("begin without source metadata");
    let filled_metadata = BeginRefreshAttemptCommand {
        source_content_sha256: Some(HASH_A.to_owned()),
        source_bytes: Some(17),
        ..empty_metadata.clone()
    };
    storage
        .begin_refresh_attempt(&filled_metadata)
        .expect("fill previously unknown source metadata");
    storage
        .begin_refresh_attempt(&empty_metadata)
        .expect("omitted metadata does not erase stored values");

    assert!(matches!(
        storage.finish_refresh_failure(&FinishRefreshFailureCommand {
            observation_id: fill_id,
            completed_at: COMPLETED.to_owned(),
            stage: RefreshFailureStage::Transport,
            source_content_sha256: Some(HASH_B.to_owned()),
            source_bytes: Some(17),
            error_code: "UPSTREAM_TIMEOUT".to_owned(),
            diagnostic_token: None,
        }),
        Err(StorageError::InvalidCommand {
            field: "source_content_sha256",
            ..
        })
    ));
    assert!(matches!(
        storage.finish_refresh_failure(&FinishRefreshFailureCommand {
            observation_id: fill_id,
            completed_at: COMPLETED.to_owned(),
            stage: RefreshFailureStage::Transport,
            source_content_sha256: Some(HASH_A.to_owned()),
            source_bytes: Some(18),
            error_code: "UPSTREAM_TIMEOUT".to_owned(),
            diagnostic_token: None,
        }),
        Err(StorageError::InvalidCommand {
            field: "source_bytes",
            ..
        })
    ));
    let still_started = storage
        .refresh_observation(&fill_id)
        .expect("filled observation lookup")
        .expect("filled observation present");
    assert_eq!(still_started.status, RefreshStatus::Started);
    assert_eq!(still_started.source_content_sha256.as_deref(), Some(HASH_A));
    assert_eq!(still_started.source_bytes, Some(17));

    storage
        .finish_refresh_failure(&FinishRefreshFailureCommand {
            observation_id: fill_id,
            completed_at: COMPLETED.to_owned(),
            stage: RefreshFailureStage::Transport,
            source_content_sha256: None,
            source_bytes: None,
            error_code: "UPSTREAM_TIMEOUT".to_owned(),
            diagnostic_token: None,
        })
        .expect("finish without erasing source metadata");
    let finished = storage
        .refresh_observation(&fill_id)
        .expect("finished observation lookup")
        .expect("finished observation present");
    assert_eq!(finished.source_content_sha256.as_deref(), Some(HASH_A));
    assert_eq!(finished.source_bytes, Some(17));
    assert!(matches!(
        storage.begin_refresh_attempt(&filled_metadata),
        Err(StorageError::ObservationNotStarted(id)) if id == fill_id
    ));
}

#[test]
fn discovery_attempt_start_is_immutable_across_retries() {
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let observation_id = trace("retry.discovery.immutable");
    let original = BeginDiscoveryAttemptCommand {
        observation_id,
        started_at: STARTED.to_owned(),
    };
    storage
        .begin_discovery_attempt(&original)
        .expect("initial discovery begin");
    storage
        .begin_discovery_attempt(&original)
        .expect("exact discovery retry is idempotent");
    assert!(matches!(
        storage.begin_discovery_attempt(&BeginDiscoveryAttemptCommand {
            observation_id,
            started_at: "2026-07-14T00:00:02Z".to_owned(),
        }),
        Err(StorageError::InvalidCommand {
            field: "started_at",
            ..
        })
    ));
    let observation = storage
        .discovery_observation(&observation_id)
        .expect("discovery observation lookup")
        .expect("discovery observation present");
    assert_eq!(observation.started_at, STARTED);
    assert_eq!(
        observation.status,
        bcsp_operational_storage::DiscoveryStatus::Started
    );

    storage
        .finish_discovery_failure(&FinishDiscoveryFailureCommand {
            observation_id,
            completed_at: COMPLETED.to_owned(),
            stage: RefreshFailureStage::Transport,
            error_code: "UPSTREAM_TIMEOUT".to_owned(),
            diagnostic_token: None,
        })
        .expect("finish discovery attempt");
    assert!(matches!(
        storage.begin_discovery_attempt(&original),
        Err(StorageError::ObservationNotStarted(id)) if id == observation_id
    ));
    assert_eq!(
        storage
            .discovery_observation(&observation_id)
            .expect("finished discovery lookup")
            .expect("finished discovery present")
            .started_at,
        STARTED
    );
}

#[test]
fn catalog_stage_and_discovery_apply_reject_completion_before_persisted_start() {
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let reversed = "2026-07-13T23:59:59Z";
    let scope = target("FALL_2026", "TIME_STAGE");
    let mut catalog = refresh_command(
        "time.catalog.stage",
        &scope,
        snapshot(&scope, "00001", "Time Stage"),
        b"time-stage",
    );
    catalog.completed_at = reversed.to_owned();
    assert!(matches!(
        storage.stage_catalog_refresh(catalog),
        Err(StorageError::InvalidCommand {
            field: "completed_at",
            ..
        })
    ));
    let catalog_observation = storage
        .refresh_observation(&trace("time.catalog.stage"))
        .expect("catalog time observation")
        .expect("catalog time observation present");
    assert_eq!(catalog_observation.started_at, STARTED);
    assert_eq!(catalog_observation.completed_at, None);
    assert_eq!(catalog_observation.status, RefreshStatus::Started);
    assert_eq!(storage.staged_payload_count().expect("staging count"), 0);

    assert!(matches!(
        storage.apply_discovery_refresh(DiscoveryRefreshCommand {
            observation_id: trace("time.discovery.apply"),
            started_at: STARTED.to_owned(),
            completed_at: reversed.to_owned(),
            snapshot: discovery_snapshot(),
        }),
        Err(StorageError::InvalidCommand {
            field: "completed_at",
            ..
        })
    ));
    let discovery_observation = storage
        .discovery_observation(&trace("time.discovery.apply"))
        .expect("discovery time observation")
        .expect("discovery time observation present");
    assert_eq!(discovery_observation.started_at, STARTED);
    assert_eq!(discovery_observation.completed_at, None);
    assert_eq!(
        discovery_observation.status,
        bcsp_operational_storage::DiscoveryStatus::Started
    );
}

#[test]
fn failure_finalization_rejects_completion_before_persisted_start() {
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let scope = target("FALL_2026", "TIME_ORDER");
    let reversed = "2026-07-13T23:59:59Z";
    storage
        .begin_refresh_attempt(&BeginRefreshAttemptCommand {
            observation_id: trace("time.catalog"),
            target: scope,
            started_at: STARTED.to_owned(),
            source_content_sha256: None,
            source_bytes: None,
        })
        .expect("begin catalog failure");
    assert!(matches!(
        storage.finish_refresh_failure(&FinishRefreshFailureCommand {
            observation_id: trace("time.catalog"),
            completed_at: reversed.to_owned(),
            stage: RefreshFailureStage::Transport,
            source_content_sha256: None,
            source_bytes: None,
            error_code: "UPSTREAM_TIMEOUT".to_owned(),
            diagnostic_token: None,
        }),
        Err(StorageError::InvalidCommand {
            field: "completed_at",
            ..
        })
    ));
    assert_eq!(
        storage
            .refresh_observation(&trace("time.catalog"))
            .expect("catalog observation")
            .expect("catalog present")
            .status,
        RefreshStatus::Started
    );
    storage
        .finish_refresh_failure(&FinishRefreshFailureCommand {
            observation_id: trace("time.catalog"),
            completed_at: COMPLETED.to_owned(),
            stage: RefreshFailureStage::Transport,
            source_content_sha256: None,
            source_bytes: None,
            error_code: "UPSTREAM_TIMEOUT".to_owned(),
            diagnostic_token: None,
        })
        .expect("finish catalog failure in order");

    storage
        .begin_discovery_attempt(&BeginDiscoveryAttemptCommand {
            observation_id: trace("time.discovery"),
            started_at: STARTED.to_owned(),
        })
        .expect("begin discovery failure");
    assert!(matches!(
        storage.finish_discovery_failure(&FinishDiscoveryFailureCommand {
            observation_id: trace("time.discovery"),
            completed_at: reversed.to_owned(),
            stage: RefreshFailureStage::Transport,
            error_code: "UPSTREAM_TIMEOUT".to_owned(),
            diagnostic_token: None,
        }),
        Err(StorageError::InvalidCommand {
            field: "completed_at",
            ..
        })
    ));
    assert_eq!(
        storage
            .discovery_observation(&trace("time.discovery"))
            .expect("discovery observation")
            .expect("discovery present")
            .status,
        bcsp_operational_storage::DiscoveryStatus::Started
    );
    storage
        .finish_discovery_failure(&FinishDiscoveryFailureCommand {
            observation_id: trace("time.discovery"),
            completed_at: COMPLETED.to_owned(),
            stage: RefreshFailureStage::Transport,
            error_code: "UPSTREAM_TIMEOUT".to_owned(),
            diagnostic_token: None,
        })
        .expect("finish discovery failure in order");
}

#[test]
fn catalog_storage_rejects_forged_delivery_and_occurrence_wire_values() {
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let scope = target("FALL_2026", "WIRE_GUARD");
    let mut cases = Vec::new();

    let mut invalid = snapshot(&scope, "00001", "Invalid Section Modality");
    invalid.sections[0].delivery_modality = "IN_PERSON".to_owned();
    cases.push((
        "wire.section-modality",
        invalid,
        "section.delivery_modality",
    ));

    let mut invalid = snapshot(&scope, "00001", "Invalid Section Synchronicity");
    invalid.sections[0].synchronicity = "SYNCHRONOUS".to_owned();
    cases.push(("wire.section-sync", invalid, "section.synchronicity"));

    let mut invalid = snapshot(&scope, "00001", "Invalid Time Token");
    invalid.occurrences[0].time_knowledge = "UNKNOWN".to_owned();
    cases.push(("wire.time-token", invalid, "occurrence.time_knowledge"));

    let mut invalid = snapshot(&scope, "00001", "Invalid Missing Projection");
    invalid.occurrences[0].start_minute = Some(600);
    cases.push(("wire.time-projection", invalid, "occurrence.time_knowledge"));

    let mut invalid = snapshot(&scope, "00001", "Invalid Known Range");
    invalid.occurrences[0].time_knowledge = "KNOWN".to_owned();
    invalid.occurrences[0].start_minute = Some(600);
    invalid.occurrences[0].end_minute = Some(600);
    cases.push(("wire.time-range", invalid, "occurrence.time_knowledge"));

    let mut invalid = snapshot(&scope, "00001", "Invalid Requiredness");
    invalid.occurrences[0].requiredness = "UNKNOWN".to_owned();
    cases.push(("wire.requiredness", invalid, "occurrence.requiredness"));

    let mut invalid = snapshot(&scope, "00001", "Invalid Occurrence Kind");
    invalid.occurrences[0].occurrence_kind = "MEETING".to_owned();
    cases.push(("wire.kind", invalid, "occurrence.occurrence_kind"));

    let mut invalid = snapshot(&scope, "00001", "Invalid Occurrence Modality");
    invalid.occurrences[0].modality = "IN_PERSON".to_owned();
    cases.push(("wire.occurrence-modality", invalid, "occurrence.modality"));

    let mut invalid = snapshot(&scope, "00001", "Invalid Occurrence Synchronicity");
    invalid.occurrences[0].synchronicity = "CONFLICT".to_owned();
    cases.push(("wire.occurrence-sync", invalid, "occurrence.synchronicity"));

    for (observation_id, invalid, expected_field) in cases {
        let body = format!("synthetic-{observation_id}");
        match storage.apply_catalog_refresh(
            refresh_command(observation_id, &scope, invalid, body.as_bytes()),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        ) {
            Err(StorageError::InvalidCommand { field, .. }) => {
                assert_eq!(field, expected_field, "{observation_id}")
            }
            other => panic!("{observation_id} unexpectedly returned {other:?}"),
        }
    }

    let mut known = snapshot(&scope, "00001", "Valid Known Time");
    known.occurrences[0].time_knowledge = "KNOWN".to_owned();
    known.occurrences[0].start_minute = Some(600);
    known.occurrences[0].end_minute = Some(660);
    known.occurrences[0].occurrence_kind = "SCHEDULED".to_owned();
    storage
        .apply_catalog_refresh(
            refresh_command("wire.valid", &scope, known, b"valid-known-time"),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("valid exact wire values and known range");

    for wire_value in ["SYNC", "ASYNC", "MIXED", "UNSPECIFIED", "UNKNOWN"] {
        let mut exact_storage = OperationalStorage::open_in_memory().expect("exact wire storage");
        let mut exact = snapshot(&scope, "00001", &format!("Valid {wire_value}"));
        exact.sections[0].synchronicity = wire_value.to_owned();
        exact.occurrences[0].synchronicity = wire_value.to_owned();
        let observation_id = format!("wire.valid.{wire_value}");
        exact_storage
            .apply_catalog_refresh(
                refresh_command(&observation_id, &scope, exact, observation_id.as_bytes()),
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
                1,
            )
            .unwrap_or_else(|error| panic!("{wire_value} must be accepted: {error:?}"));

        let published = exact_storage
            .published_catalog_snapshot(&scope)
            .expect("read exact wire publication")
            .expect("published exact wire snapshot");
        assert_eq!(published.snapshot.sections[0].synchronicity, wire_value);
        assert_eq!(published.snapshot.occurrences[0].synchronicity, wire_value);
    }

    for wire_value in [
        "ON_CAMPUS_OR_IN_PERSON",
        "HYBRID",
        "ONLINE",
        "OTHER",
        "UNKNOWN",
        "UNKNOWN_CONFLICT",
    ] {
        let mut exact_storage = OperationalStorage::open_in_memory().expect("exact wire storage");
        let mut exact = snapshot(&scope, "00001", &format!("Valid {wire_value}"));
        exact.sections[0].delivery_modality = wire_value.to_owned();
        exact.occurrences[0].modality = wire_value.to_owned();
        let observation_id = format!("wire.valid.modality.{wire_value}");
        exact_storage
            .apply_catalog_refresh(
                refresh_command(&observation_id, &scope, exact, observation_id.as_bytes()),
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
                1,
            )
            .unwrap_or_else(|error| panic!("{wire_value} must be accepted: {error:?}"));

        let published = exact_storage
            .published_catalog_snapshot(&scope)
            .expect("read exact wire publication")
            .expect("published exact wire snapshot");
        assert_eq!(published.snapshot.sections[0].delivery_modality, wire_value);
        assert_eq!(published.snapshot.occurrences[0].modality, wire_value);
    }

    for wire_value in ["REQUIRED", "OPTIONAL", "UNKNOWN_REQUIREDNESS"] {
        let mut exact_storage = OperationalStorage::open_in_memory().expect("exact wire storage");
        let mut exact = snapshot(&scope, "00001", &format!("Valid {wire_value}"));
        exact.occurrences[0].requiredness = wire_value.to_owned();
        let observation_id = format!("wire.valid.requiredness.{wire_value}");
        exact_storage
            .apply_catalog_refresh(
                refresh_command(&observation_id, &scope, exact, observation_id.as_bytes()),
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
                1,
            )
            .unwrap_or_else(|error| panic!("{wire_value} must be accepted: {error:?}"));

        let published = exact_storage
            .published_catalog_snapshot(&scope)
            .expect("read exact wire publication")
            .expect("published exact wire snapshot");
        assert_eq!(published.snapshot.occurrences[0].requiredness, wire_value);
    }
}

fn discovery_snapshot() -> DiscoverySnapshot {
    DiscoverySnapshot {
        sources: vec![DiscoverySourceVersion {
            source_version_id: format!("selector:{HASH_A}"),
            source_kind: DiscoverySourceKind::Selector,
            source_identity: "RUTGERS_SELECTOR".to_owned(),
            content_sha256: HASH_A.to_owned(),
            canonical_facts: json!({
                "schema": "discovery-source-v1",
                "reportedSourceVersion": {"state": "VALUE", "value": "synthetic-v1"}
            }),
            observed_at: STARTED.to_owned(),
        }],
        terms: vec![DiscoveredTerm {
            term_id: "SPRING_2028".try_into().expect("term"),
            year: Some(2028),
            term_code: Some("1".to_owned()),
            display_name: Some("Synthetic Spring 2028".to_owned()),
            published: None,
            canonical_facts: presence_facts("term"),
            source_version_id: format!("selector:{HASH_A}"),
        }],
        campuses: vec![
            DiscoveredCampus {
                target: target("SPRING_2028", "FUTURE_X"),
                display_name: Some("Future Campus".to_owned()),
                category: None,
                enabled: Some(true),
                canonical_facts: presence_facts("campus"),
                source_version_id: format!("selector:{HASH_A}"),
            },
            DiscoveredCampus {
                target: target("SPRING_2028", "EMPTY_CAMPUS"),
                display_name: Some("Valid Empty Campus".to_owned()),
                category: Some("SYNTHETIC".to_owned()),
                enabled: Some(true),
                canonical_facts: presence_facts("campus-empty"),
                source_version_id: format!("selector:{HASH_A}"),
            },
        ],
        subjects: vec![DiscoveredSubject {
            term_id: "SPRING_2028".try_into().expect("term"),
            campus_code: "FUTURE_X".try_into().expect("campus"),
            subject_code: "未来科目".to_owned(),
            display_name: Some("Synthetic Subject".to_owned()),
            enabled: None,
            canonical_facts: presence_facts("subject"),
            source_version_id: format!("selector:{HASH_A}"),
        }],
    }
}

#[test]
fn discovery_is_dynamic_versioned_and_failure_retains_last_known_dictionary() {
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    storage
        .begin_discovery_attempt(&BeginDiscoveryAttemptCommand {
            observation_id: trace("discovery.failure-first"),
            started_at: STARTED.to_owned(),
        })
        .expect("begin discovery");
    storage
        .finish_discovery_failure(&FinishDiscoveryFailureCommand {
            observation_id: trace("discovery.failure-first"),
            completed_at: COMPLETED.to_owned(),
            stage: RefreshFailureStage::Transport,
            error_code: "UPSTREAM_TIMEOUT".to_owned(),
            diagnostic_token: None,
        })
        .expect("finish discovery failure");
    let first_failure = storage.discovery_state().expect("state");
    assert_eq!(first_failure.content_version, 0);
    assert_eq!(first_failure.campus_count, 0);
    assert!(!first_failure.is_stale);
    assert_eq!(
        first_failure.availability,
        DiscoveryAvailability::UnavailableNoFirstSuccess
    );

    let changed = storage
        .apply_discovery_refresh(DiscoveryRefreshCommand {
            observation_id: trace("discovery.success"),
            started_at: STARTED.to_owned(),
            completed_at: COMPLETED.to_owned(),
            snapshot: discovery_snapshot(),
        })
        .expect("discovery publish");
    assert_eq!(
        changed,
        DiscoveryPublishOutcome::AppliedChanged { content_version: 1 }
    );
    let targets = storage.discovered_targets().expect("targets");
    assert!(targets.contains(&target("SPRING_2028", "FUTURE_X")));
    assert!(targets.contains(&target("SPRING_2028", "EMPTY_CAMPUS")));
    let success_state = storage.discovery_state().expect("success state");
    assert_eq!(success_state.availability, DiscoveryAvailability::Fresh);
    let success_point = success_state.last_published.expect("freshness point");
    assert_eq!(success_point.started_at, STARTED);
    assert_eq!(success_point.completed_at.as_deref(), Some(COMPLETED));
    assert_eq!(success_point.counts.campuses, 2);
    assert_eq!(success_point.source_content_sha256, vec![HASH_A.to_owned()]);

    let mut same_content_later = discovery_snapshot();
    same_content_later.sources[0].observed_at = "2026-07-14T00:02:00Z".to_owned();
    let unchanged = storage
        .apply_discovery_refresh(DiscoveryRefreshCommand {
            observation_id: trace("discovery.unchanged"),
            started_at: "2026-07-14T00:02:00Z".to_owned(),
            completed_at: "2026-07-14T00:02:01Z".to_owned(),
            snapshot: same_content_later,
        })
        .expect("identical content observed later must remain valid");
    assert_eq!(
        unchanged,
        DiscoveryPublishOutcome::AppliedUnchanged { content_version: 1 }
    );

    storage
        .begin_discovery_attempt(&BeginDiscoveryAttemptCommand {
            observation_id: trace("discovery.failure-later"),
            started_at: STARTED.to_owned(),
        })
        .expect("begin later failure");
    storage
        .finish_discovery_failure(&FinishDiscoveryFailureCommand {
            observation_id: trace("discovery.failure-later"),
            completed_at: COMPLETED.to_owned(),
            stage: RefreshFailureStage::Schema,
            error_code: "SCHEMA_REJECTED".to_owned(),
            diagnostic_token: Some("shape.v2".to_owned()),
        })
        .expect("later failure");
    let retained = storage.discovery_state().expect("state");
    assert_eq!(retained.content_version, 1);
    assert_eq!(retained.campus_count, 2);
    assert_eq!(
        retained.last_success_observation_id,
        Some(trace("discovery.unchanged"))
    );
    assert!(retained.is_stale);
    assert_eq!(retained.availability, DiscoveryAvailability::Stale);
    assert_eq!(storage.discovered_targets().expect("targets"), targets);

    let mut unsafe_source = discovery_snapshot();
    unsafe_source.sources[0].source_identity = "SOC_SELECTOR?query=forbidden".to_owned();
    assert!(
        storage
            .apply_discovery_refresh(DiscoveryRefreshCommand {
                observation_id: trace("discovery.unsafe-source"),
                started_at: STARTED.to_owned(),
                completed_at: COMPLETED.to_owned(),
                snapshot: unsafe_source,
            })
            .is_err()
    );
}

#[test]
fn published_catalog_snapshots_round_trip_after_restart_and_remain_target_scoped() {
    let temp = TempDir::new().expect("temp dir");
    let path = db_path(&temp);
    let target_a = target("FALL_2026", "READER_A");
    let target_b = target("FALL_2026", "READER_B");
    let snapshot_a = snapshot(&target_a, "00001", "Reader A");
    let snapshot_b = snapshot(&target_b, "00002", "Reader B");
    let mut storage = OperationalStorage::open(&path).expect("storage");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "reader.catalog.a",
                &target_a,
                snapshot_a.clone(),
                b"reader-a",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("publish target A");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "reader.catalog.b",
                &target_b,
                snapshot_b.clone(),
                b"reader-b",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("publish target B");
    drop(storage);

    let mut storage = OperationalStorage::open(&path).expect("restart storage");
    let published_a = storage
        .published_catalog_snapshot(&target_a)
        .expect("read target A")
        .expect("published target A");
    assert_eq!(published_a.target, target_a);
    assert_eq!(published_a.content_version, 1);
    assert_eq!(
        published_a.publication.observation_id,
        trace("reader.catalog.a")
    );
    assert_eq!(published_a.snapshot, snapshot_a);
    assert_eq!(published_a.snapshot.occurrences[0].evidence, "NONE");
    assert_eq!(
        published_a.snapshot.occurrences[0].normalization_reason,
        "SYNTHETIC_FIXTURE"
    );
    let published_b = storage
        .published_catalog_snapshot(&target_b)
        .expect("read target B")
        .expect("published target B");
    assert_eq!(published_b.target, target_b);
    assert_eq!(
        published_b.publication.observation_id,
        trace("reader.catalog.b")
    );
    assert_eq!(published_b.snapshot, snapshot_b);
    assert!(
        published_a
            .snapshot
            .sections
            .iter()
            .all(|section| section.key.campus() == target_a.campus())
    );
    assert!(
        published_b
            .snapshot
            .sections
            .iter()
            .all(|section| section.key.campus() == target_b.campus())
    );
}

#[test]
fn published_initial_empty_catalog_survives_restart_without_seed_rows() {
    let temp = TempDir::new().expect("temp dir");
    let path = db_path(&temp);
    let scope = target("SPRING_2028", "EMPTY_CAMPUS");
    let mut storage = OperationalStorage::open(&path).expect("storage");
    storage
        .apply_discovery_refresh(DiscoveryRefreshCommand {
            observation_id: trace("reader.empty.discovery"),
            started_at: STARTED.to_owned(),
            completed_at: COMPLETED.to_owned(),
            snapshot: discovery_snapshot(),
        })
        .expect("selector membership");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "reader.empty.catalog",
                &scope,
                CatalogSnapshot::default(),
                b"[]",
            ),
            EmptySnapshotDecision::AcceptInitialSelectorConfirmedEmpty(
                InitialEmptyProof::CurrentSelectorMembership,
            ),
            1,
        )
        .expect("publish initial empty");
    drop(storage);

    let mut storage = OperationalStorage::open(&path).expect("restart storage");
    let published = storage
        .published_catalog_snapshot(&scope)
        .expect("read empty")
        .expect("published empty");
    assert_eq!(published.content_version, 1);
    assert_eq!(
        published.publication.status,
        RefreshStatus::EmptyValidInitial
    );
    assert!(published.snapshot.is_empty());
    assert_eq!(published.snapshot.counts(), Default::default());
}

#[test]
fn discovery_reader_updates_unchanged_source_ownership_and_retains_it_when_stale() {
    let temp = TempDir::new().expect("temp dir");
    let path = db_path(&temp);
    let mut storage = OperationalStorage::open(&path).expect("storage");
    storage
        .apply_discovery_refresh(DiscoveryRefreshCommand {
            observation_id: trace("reader.discovery.first"),
            started_at: STARTED.to_owned(),
            completed_at: COMPLETED.to_owned(),
            snapshot: discovery_snapshot(),
        })
        .expect("first discovery");

    let mut unchanged = discovery_snapshot();
    unchanged.sources[0].content_sha256 = HASH_B.to_owned();
    unchanged.sources[0].source_version_id = format!("selector:{HASH_B}");
    unchanged.sources[0].observed_at = "2026-07-14T00:02:00Z".to_owned();
    for term in &mut unchanged.terms {
        term.source_version_id = format!("selector:{HASH_B}");
    }
    for campus in &mut unchanged.campuses {
        campus.source_version_id = format!("selector:{HASH_B}");
    }
    for subject in &mut unchanged.subjects {
        subject.source_version_id = format!("selector:{HASH_B}");
    }
    assert_eq!(
        storage
            .apply_discovery_refresh(DiscoveryRefreshCommand {
                observation_id: trace("reader.discovery.unchanged"),
                started_at: "2026-07-14T00:02:00Z".to_owned(),
                completed_at: "2026-07-14T00:02:01Z".to_owned(),
                snapshot: unchanged,
            })
            .expect("unchanged discovery"),
        DiscoveryPublishOutcome::AppliedUnchanged { content_version: 1 }
    );
    storage
        .begin_discovery_attempt(&BeginDiscoveryAttemptCommand {
            observation_id: trace("reader.discovery.failure"),
            started_at: "2026-07-14T00:03:00Z".to_owned(),
        })
        .expect("begin later failure");
    storage
        .finish_discovery_failure(&FinishDiscoveryFailureCommand {
            observation_id: trace("reader.discovery.failure"),
            completed_at: "2026-07-14T00:03:01Z".to_owned(),
            stage: RefreshFailureStage::Transport,
            error_code: "UPSTREAM_TIMEOUT".to_owned(),
            diagnostic_token: None,
        })
        .expect("finish later failure");
    drop(storage);

    let mut storage = OperationalStorage::open(&path).expect("restart storage");
    let published = storage
        .published_discovery_snapshot()
        .expect("published discovery");
    assert_eq!(published.state.availability, DiscoveryAvailability::Stale);
    assert_eq!(published.state.content_version, 1);
    assert_eq!(published.sources[0].content_sha256, HASH_B);
    assert_eq!(
        published.state.last_success_observation_id,
        Some(trace("reader.discovery.unchanged"))
    );
    assert!(
        published
            .snapshot
            .terms
            .iter()
            .all(|term| term.source_version_id == format!("selector:{HASH_B}"))
    );
    assert!(
        published
            .snapshot
            .campuses
            .iter()
            .all(|campus| campus.source_version_id == format!("selector:{HASH_B}"))
    );
    assert!(
        published
            .snapshot
            .subjects
            .iter()
            .all(|subject| subject.source_version_id == format!("selector:{HASH_B}"))
    );
}

#[test]
fn published_readers_fail_closed_on_corrupt_trace_source_and_provenance() {
    let trace_temp = TempDir::new().expect("trace temp");
    let trace_path = db_path(&trace_temp);
    let trace_target = target("FALL_2026", "CORRUPT_TRACE");
    let mut storage = OperationalStorage::open(&trace_path).expect("trace storage");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "corrupt.trace.catalog",
                &trace_target,
                snapshot(&trace_target, "00001", "Trace"),
                b"trace",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("publish trace fixture");
    drop(storage);
    let connection = Connection::open(&trace_path).expect("tamper trace database");
    let noncanonical_trace = trace("corrupt.trace.catalog").to_string().to_uppercase();
    connection
        .execute(
            "UPDATE catalog_targets SET last_published_observation_id = ?1",
            [&noncanonical_trace],
        )
        .expect("tamper trace pointer");
    drop(connection);
    let storage = OperationalStorage::open(&trace_path).expect("reopen trace database");
    assert!(matches!(
        storage.target_state(&trace_target),
        Err(StorageError::InvalidStoredTraceId { .. })
    ));

    let source_temp = TempDir::new().expect("source temp");
    let source_path = db_path(&source_temp);
    let mut storage = OperationalStorage::open(&source_path).expect("source storage");
    storage
        .apply_discovery_refresh(DiscoveryRefreshCommand {
            observation_id: trace("corrupt.source.discovery"),
            started_at: STARTED.to_owned(),
            completed_at: COMPLETED.to_owned(),
            snapshot: discovery_snapshot(),
        })
        .expect("publish source fixture");
    drop(storage);
    let connection = Connection::open(&source_path).expect("tamper source database");
    connection
        .execute(
            "UPDATE catalog_discovery_source_versions SET source_identity = 'ATTACK'",
            [],
        )
        .expect("tamper source identity");
    drop(connection);
    let mut storage = OperationalStorage::open(&source_path).expect("reopen source database");
    assert!(matches!(
        storage.published_discovery_snapshot(),
        Err(StorageError::InvalidStoredProjection {
            table: "catalog_discovery_source_versions",
            field: "source identity",
            ..
        })
    ));

    let provenance_temp = TempDir::new().expect("provenance temp");
    let provenance_path = db_path(&provenance_temp);
    let provenance_target = target("FALL_2026", "CORRUPT_PROVENANCE");
    let mut storage = OperationalStorage::open(&provenance_path).expect("provenance storage");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "corrupt.provenance.catalog",
                &provenance_target,
                snapshot(&provenance_target, "00001", "Provenance"),
                b"provenance",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("publish provenance fixture");
    drop(storage);
    let connection = Connection::open(&provenance_path).expect("tamper provenance database");
    connection
        .execute(
            "UPDATE catalog_provenance SET entity_key = 'FALL_2026/CORRUPT_PROVENANCE/99999'",
            [],
        )
        .expect("tamper provenance ownership");
    drop(connection);
    let mut storage =
        OperationalStorage::open(&provenance_path).expect("reopen provenance database");
    assert!(matches!(
        storage.published_catalog_snapshot(&provenance_target),
        Err(StorageError::OrphanedStoredEntity {
            entity_kind: "provenance"
        })
    ));
}

#[test]
fn catalog_reader_rejects_corrupt_timestamp_version_and_enum() {
    let temp = TempDir::new().expect("temp dir");
    let path = db_path(&temp);
    let scope = target("FALL_2026", "CORRUPT_ROWS");
    let mut storage = OperationalStorage::open(&path).expect("storage");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "corrupt.rows.catalog",
                &scope,
                snapshot(&scope, "00001", "Corrupt Rows"),
                b"corrupt-rows",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("publish fixture");
    drop(storage);

    let connection = Connection::open(&path).expect("tamper timestamp");
    connection
        .execute(
            "UPDATE catalog_refresh_observations SET started_at = 'not-a-timestamp'",
            [],
        )
        .expect("write invalid timestamp");
    drop(connection);
    let storage = OperationalStorage::open(&path).expect("reopen timestamp database");
    assert!(matches!(
        storage.target_state(&scope),
        Err(StorageError::InvalidStoredTimestamp { .. })
    ));
    drop(storage);

    let connection = Connection::open(&path).expect("restore and tamper version");
    connection
        .execute(
            "UPDATE catalog_refresh_observations SET started_at = ?1",
            [STARTED],
        )
        .expect("restore timestamp");
    connection
        .execute("UPDATE catalog_sections SET content_version = 2", [])
        .expect("tamper version");
    drop(connection);
    let mut storage = OperationalStorage::open(&path).expect("reopen version database");
    assert!(matches!(
        storage.published_catalog_snapshot(&scope),
        Err(StorageError::InconsistentPublishedVersion {
            entity_kind: "section"
        })
    ));
    drop(storage);

    let connection = Connection::open(&path).expect("restore and tamper enum");
    connection
        .execute_batch("PRAGMA ignore_check_constraints = ON;")
        .expect("allow synthetic corruption");
    connection
        .execute("UPDATE catalog_sections SET content_version = 1", [])
        .expect("restore version");
    connection
        .execute(
            "UPDATE catalog_sections SET delivery_modality = 'FORGED'",
            [],
        )
        .expect("tamper enum");
    drop(connection);
    let mut storage = OperationalStorage::open(&path).expect("reopen enum database");
    assert!(matches!(
        storage.published_catalog_snapshot(&scope),
        Err(StorageError::InvalidStoredProjection {
            table: "catalog_sections",
            field: "delivery_modality",
            ..
        })
    ));
}

#[test]
fn discovery_content_v1_domain_golden_is_stable() {
    assert_eq!(
        discovery_content_sha256_v1(&discovery_snapshot()).expect("discovery hash"),
        "94d3d6570a3079d2347e80da2cb6e22e264bce0c345d9250f8cccdc276a0c7ac"
    );
}

#[test]
fn no_database_artifact_is_part_of_the_test_source_tree() {
    fn visit(path: &Path) -> Vec<std::path::PathBuf> {
        std::fs::read_dir(path)
            .expect("read source directory")
            .flat_map(|entry| {
                let path = entry.expect("directory entry").path();
                if path.is_dir() {
                    visit(&path)
                } else {
                    vec![path]
                }
            })
            .collect()
    }
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(visit(crate_root).iter().all(|path| {
        !matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("sqlite" | "sqlite3" | "db")
        )
    }));
}

/// The fixture snapshot plus the one `OCCURRENCE`/`canonicalFacts` provenance
/// row the publication mapping writes per occurrence, keyed exactly the way
/// `bcsp-catalog` keys it: SectionKey display form joined with the occurrence key.
fn snapshot_with_occurrence_provenance(
    target: &TermCampusKey,
    index: &str,
    title: &str,
) -> CatalogSnapshot {
    let mut snapshot = snapshot(target, index, title);
    snapshot.provenance.push(StoredProvenance {
        entity_kind: ProvenanceEntityKind::Occurrence,
        entity_key: format!("{}/{}/{index}/meeting.0", target.term(), target.campus()),
        field_name: "canonicalFacts".to_owned(),
        source_sha256: HASH_B.to_owned(),
        source_ordinal: 0,
        detail: json!({
            "rawSha256": HASH_B,
            "normalizationReason": "SYNTHETIC_FIXTURE",
            "evidence": "NONE",
        }),
    });
    snapshot
}

fn section_rewrite(target: &TermCampusKey, index: &str) -> SectionDeliveryRewrite {
    SectionDeliveryRewrite {
        key: SectionKey::try_new(target.term().as_str(), target.campus().as_str(), index)
            .expect("section key"),
        delivery_modality: "ONLINE".to_owned(),
        synchronicity: "ASYNC".to_owned(),
    }
}

fn occurrence_rewrite(target: &TermCampusKey, index: &str) -> OccurrenceDeliveryRewrite {
    OccurrenceDeliveryRewrite {
        section_key: SectionKey::try_new(target.term().as_str(), target.campus().as_str(), index)
            .expect("section key"),
        occurrence_key: "meeting.0".to_owned(),
        occurrence_kind: "BY_ARRANGEMENT".to_owned(),
        modality: "ONLINE".to_owned(),
        synchronicity: "ASYNC".to_owned(),
        evidence: "REMOTE".to_owned(),
        normalization_reason: "ONLINE_BY_ARRANGEMENT_ASYNCHRONOUS".to_owned(),
    }
}

fn delivery_rewrite(
    derivation_version: u32,
    sections: Vec<SectionDeliveryRewrite>,
    occurrences: Vec<OccurrenceDeliveryRewrite>,
) -> CatalogDeliveryRewrite {
    CatalogDeliveryRewrite {
        derivation_version,
        stamped_at: "2026-09-01T00:00:00Z".to_owned(),
        sections,
        occurrences,
    }
}

fn begin_and_fail_attempt(storage: &mut OperationalStorage, label: &str, target: &TermCampusKey) {
    storage
        .begin_refresh_attempt(&BeginRefreshAttemptCommand {
            observation_id: trace(label),
            target: target.clone(),
            started_at: STARTED.to_owned(),
            source_content_sha256: None,
            source_bytes: None,
        })
        .expect("begin attempt");
    storage
        .finish_refresh_failure(&FinishRefreshFailureCommand {
            observation_id: trace(label),
            completed_at: COMPLETED.to_owned(),
            stage: RefreshFailureStage::Transport,
            source_content_sha256: None,
            source_bytes: None,
            error_code: "UPSTREAM_TIMEOUT".to_owned(),
            diagnostic_token: None,
        })
        .expect("finish failure");
}

#[test]
fn catalog_target_versions_enumerate_every_target_row_including_unpublished_and_selector_absent() {
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    assert!(storage.catalog_target_versions().expect("empty").is_empty());

    let published = target("FALL_2026", "NB");
    let unpublished = target("FALL_2026", "CM");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "versions.published",
                &published,
                snapshot(&published, "10001", "Published"),
                b"versions-published",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("publish");
    // A target row that only ever failed: content version 0, still enumerated.
    begin_and_fail_attempt(&mut storage, "versions.failed", &unpublished);
    // Neither target is a selector member; the enumeration must not depend on that.
    assert!(storage.discovered_targets().expect("selector").is_empty());

    assert_eq!(
        storage.catalog_target_versions().expect("versions"),
        vec![
            CatalogTargetVersion {
                target: unpublished,
                current_content_version: 0,
            },
            CatalogTargetVersion {
                target: published,
                current_content_version: 1,
            },
        ]
    );
}

#[test]
fn publication_stamps_the_derivation_version_only_when_serving_rows_are_replaced() {
    let temp = TempDir::new().expect("temp dir");
    let path = db_path(&temp);
    let mut storage = OperationalStorage::open(&path).expect("storage");
    let scope = target("FALL_2026", "NB");
    assert!(
        storage
            .catalog_derivation_versions()
            .expect("no stamps")
            .is_empty()
    );

    // AppliedChanged stamps the version that produced the new rows.
    storage
        .apply_catalog_refresh(
            refresh_command(
                "stamp.1",
                &scope,
                snapshot(&scope, "10001", "One"),
                b"stamp-one",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("first publish");
    assert_eq!(
        storage
            .catalog_derivation_versions()
            .expect("stamps")
            .get(&scope),
        Some(&1)
    );

    // AppliedUnchanged leaves the rows alone, so it leaves the stamp alone even
    // though the publishing binary carries a newer rule.
    let unchanged = storage
        .apply_catalog_refresh(
            refresh_command(
                "stamp.2",
                &scope,
                snapshot(&scope, "10001", "One"),
                b"stamp-one-different-body",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            2,
        )
        .expect("unchanged publish");
    assert!(matches!(unchanged, PublishOutcome::AppliedUnchanged { .. }));
    assert_eq!(
        storage
            .catalog_derivation_versions()
            .expect("stamps")
            .get(&scope),
        Some(&1)
    );

    // A changed publication under the newer rule moves the stamp.
    let changed = storage
        .apply_catalog_refresh(
            refresh_command(
                "stamp.3",
                &scope,
                snapshot(&scope, "10002", "Two"),
                b"stamp-two",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            2,
        )
        .expect("changed publish");
    assert!(matches!(changed, PublishOutcome::AppliedChanged { .. }));
    assert_eq!(
        storage
            .catalog_derivation_versions()
            .expect("stamps")
            .get(&scope),
        Some(&2)
    );

    // InitialValidEmpty replaces (zero) rows and stamps too.
    storage
        .apply_discovery_refresh(DiscoveryRefreshCommand {
            observation_id: trace("stamp.discovery"),
            started_at: STARTED.to_owned(),
            completed_at: COMPLETED.to_owned(),
            snapshot: discovery_snapshot(),
        })
        .expect("selector membership");
    let empty_scope = target("SPRING_2028", "EMPTY_CAMPUS");
    let first_empty = storage
        .apply_catalog_refresh(
            refresh_command(
                "stamp.empty",
                &empty_scope,
                CatalogSnapshot::default(),
                b"stamp-empty",
            ),
            EmptySnapshotDecision::AcceptInitialSelectorConfirmedEmpty(
                InitialEmptyProof::CurrentSelectorMembership,
            ),
            2,
        )
        .expect("first valid empty");
    assert!(matches!(
        first_empty,
        PublishOutcome::InitialValidEmpty { content_version: 1 }
    ));
    assert_eq!(
        storage
            .catalog_derivation_versions()
            .expect("stamps")
            .get(&empty_scope),
        Some(&2)
    );

    // A target that never published has no stamp; so does a target whose row
    // predates migration 0007 (simulated by deleting it): both read as absent.
    let failed_scope = target("FALL_2026", "CM");
    begin_and_fail_attempt(&mut storage, "stamp.failed", &failed_scope);
    assert!(
        !storage
            .catalog_derivation_versions()
            .expect("stamps")
            .contains_key(&failed_scope)
    );
    assert!(matches!(
        storage.apply_catalog_refresh(
            refresh_command(
                "stamp.zero",
                &failed_scope,
                snapshot(&failed_scope, "10001", "Z"),
                b"z"
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            0,
        ),
        Err(StorageError::InvalidCommand {
            field: "derivation_version",
            ..
        })
    ));
    drop(storage);
    let connection = Connection::open(&path).expect("raw connection");
    connection
        .execute("DELETE FROM catalog_derivation_state", [])
        .expect("simulate a pre-0007 database");
    drop(connection);
    let storage = OperationalStorage::open(&path).expect("reopen");
    assert!(
        storage
            .catalog_derivation_versions()
            .expect("stamps")
            .is_empty()
    );
    assert_eq!(storage.catalog_target_versions().expect("targets").len(), 3);
}

#[test]
fn rewrite_catalog_delivery_replaces_only_derived_columns_in_place_and_stamps_the_target() {
    let temp = TempDir::new().expect("temp dir");
    let path = db_path(&temp);
    let mut storage = OperationalStorage::open(&path).expect("storage");
    let scope = target("FALL_2026", "NB");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "rewrite.1",
                &scope,
                snapshot_with_occurrence_provenance(&scope, "10001", "Rewrite"),
                b"rewrite-one",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("publish");
    let before = storage
        .published_catalog_snapshot(&scope)
        .expect("read")
        .expect("published");
    let state_before = storage
        .target_state(&scope)
        .expect("state")
        .expect("target");
    let checkpoint_digest = |path: &Path| {
        let connection = Connection::open(path).expect("raw connection");
        let checkpoints: (i64, Option<i64>) = connection
            .query_row(
                "SELECT COUNT(*), MAX(content_version) FROM catalog_refresh_checkpoints
                 WHERE target_id = 'FALL_2026/NB'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("checkpoints");
        let fts: (i64, Option<i64>) = connection
            .query_row(
                "SELECT COUNT(*), MAX(content_version) FROM catalog_course_fts
                 WHERE target_id = 'FALL_2026/NB'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("fts");
        (checkpoints, fts)
    };
    let digest_before = checkpoint_digest(&path);

    let report = storage
        .rewrite_catalog_delivery(
            &scope,
            &delivery_rewrite(
                2,
                vec![section_rewrite(&scope, "10001")],
                vec![occurrence_rewrite(&scope, "10001")],
            ),
        )
        .expect("rewrite");
    assert_eq!(report.target, scope);
    assert_eq!(report.derivation_version, 2);
    assert_eq!(
        (report.sections_rewritten, report.occurrences_rewritten),
        (1, 1)
    );
    assert_eq!(
        storage
            .catalog_derivation_versions()
            .expect("stamps")
            .get(&scope),
        Some(&2)
    );

    // Exactly the derived columns changed; everything else is byte-identical,
    // including the identity and hash columns the projection checks.
    let after = storage
        .published_catalog_snapshot(&scope)
        .expect("read")
        .expect("published");
    let mut expected = before.clone();
    expected.snapshot.sections[0].delivery_modality = "ONLINE".to_owned();
    expected.snapshot.sections[0].synchronicity = "ASYNC".to_owned();
    expected.snapshot.occurrences[0].occurrence_kind = "BY_ARRANGEMENT".to_owned();
    expected.snapshot.occurrences[0].modality = "ONLINE".to_owned();
    expected.snapshot.occurrences[0].synchronicity = "ASYNC".to_owned();
    expected.snapshot.occurrences[0].evidence = "REMOTE".to_owned();
    expected.snapshot.occurrences[0].normalization_reason =
        "ONLINE_BY_ARRANGEMENT_ASYNCHRONOUS".to_owned();
    let occurrence_provenance = expected
        .snapshot
        .provenance
        .iter_mut()
        .find(|row| row.entity_kind == ProvenanceEntityKind::Occurrence)
        .expect("occurrence provenance row");
    assert_eq!(
        occurrence_provenance.entity_key, "FALL_2026/NB/10001/meeting.0",
        "entity key is the SectionKey display form joined with the occurrence key"
    );
    occurrence_provenance.detail["normalizationReason"] =
        json!("ONLINE_BY_ARRANGEMENT_ASYNCHRONOUS");
    occurrence_provenance.detail["evidence"] = json!("REMOTE");
    assert_eq!(after, expected);
    assert_eq!(after.content_version, 1);
    assert_eq!(after.snapshot.sections[0].canonical_sha256, HASH_B);
    assert_eq!(
        storage
            .target_state(&scope)
            .expect("state")
            .expect("target"),
        state_before,
        "content version, accepted semantic hash, counts and observations are untouched"
    );
    assert_eq!(checkpoint_digest(&path), digest_before);
    assert!(
        storage
            .integrity_report(&scope)
            .expect("integrity")
            .is_clean()
    );

    // An empty rewrite only moves the stamp (the idempotent second pass).
    let report = storage
        .rewrite_catalog_delivery(&scope, &delivery_rewrite(3, Vec::new(), Vec::new()))
        .expect("stamp only");
    assert_eq!(
        (report.sections_rewritten, report.occurrences_rewritten),
        (0, 0)
    );
    assert_eq!(
        storage
            .catalog_derivation_versions()
            .expect("stamps")
            .get(&scope),
        Some(&3)
    );
    assert_eq!(
        storage
            .published_catalog_snapshot(&scope)
            .expect("read")
            .expect("published"),
        after
    );
}

#[test]
fn rewrite_catalog_delivery_rejects_forged_tokens_and_unknown_rows_without_partial_writes() {
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    let scope = target("FALL_2026", "NB");
    let other = target("FALL_2026", "CM");
    storage
        .apply_catalog_refresh(
            refresh_command(
                "reject.1",
                &scope,
                snapshot_with_occurrence_provenance(&scope, "10001", "Reject"),
                b"reject-one",
            ),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            1,
        )
        .expect("publish");
    let published = storage
        .published_catalog_snapshot(&scope)
        .expect("read")
        .expect("published");

    let mut forged_section = section_rewrite(&scope, "10001");
    forged_section.synchronicity = "async".to_owned();
    assert!(matches!(
        storage.rewrite_catalog_delivery(
            &scope,
            &delivery_rewrite(2, vec![forged_section], Vec::new())
        ),
        Err(StorageError::InvalidCommand {
            field: "synchronicity",
            ..
        })
    ));
    let mut forged_reason = occurrence_rewrite(&scope, "10001");
    forged_reason.normalization_reason = "not-a-code".to_owned();
    assert!(matches!(
        storage.rewrite_catalog_delivery(
            &scope,
            &delivery_rewrite(2, Vec::new(), vec![forged_reason])
        ),
        Err(StorageError::InvalidCommand {
            field: "normalization_reason",
            ..
        })
    ));
    assert!(matches!(
        storage.rewrite_catalog_delivery(
            &scope,
            &delivery_rewrite(2, vec![section_rewrite(&other, "10001")], Vec::new())
        ),
        Err(StorageError::InvalidCommand {
            field: "sections",
            ..
        })
    ));
    assert!(matches!(
        storage.rewrite_catalog_delivery(&scope, &delivery_rewrite(0, Vec::new(), Vec::new())),
        Err(StorageError::InvalidCommand {
            field: "derivation_version",
            ..
        })
    ));

    // A valid section row plus an unknown occurrence row: the whole rewrite
    // rolls back, so neither the section nor the stamp moves.
    let mut unknown_occurrence = occurrence_rewrite(&scope, "10001");
    unknown_occurrence.occurrence_key = "meeting.9".to_owned();
    assert!(matches!(
        storage.rewrite_catalog_delivery(
            &scope,
            &delivery_rewrite(
                2,
                vec![section_rewrite(&scope, "10001")],
                vec![unknown_occurrence]
            )
        ),
        Err(StorageError::InvalidStoredProjection {
            table: "catalog_occurrences",
            ..
        })
    ));
    assert!(matches!(
        storage.rewrite_catalog_delivery(
            &scope,
            &delivery_rewrite(2, vec![section_rewrite(&scope, "99999")], Vec::new())
        ),
        Err(StorageError::InvalidStoredProjection {
            table: "catalog_sections",
            ..
        })
    ));
    assert_eq!(
        storage
            .published_catalog_snapshot(&scope)
            .expect("read")
            .expect("published"),
        published
    );
    assert_eq!(
        storage
            .catalog_derivation_versions()
            .expect("stamps")
            .get(&scope),
        Some(&1)
    );

    // Unknown or never-published targets accept only a stamp.
    assert!(matches!(
        storage.rewrite_catalog_delivery(&other, &delivery_rewrite(2, Vec::new(), Vec::new())),
        Err(StorageError::CatalogTargetNotPublished)
    ));
    begin_and_fail_attempt(&mut storage, "reject.failed", &other);
    assert!(matches!(
        storage.rewrite_catalog_delivery(
            &other,
            &delivery_rewrite(2, vec![section_rewrite(&other, "10001")], Vec::new())
        ),
        Err(StorageError::CatalogTargetNotPublished)
    ));
    let report = storage
        .rewrite_catalog_delivery(&other, &delivery_rewrite(2, Vec::new(), Vec::new()))
        .expect("stamp an unpublished target");
    assert_eq!(
        (report.sections_rewritten, report.occurrences_rewritten),
        (0, 0)
    );
    assert_eq!(
        storage
            .catalog_derivation_versions()
            .expect("stamps")
            .get(&other),
        Some(&2)
    );
}
