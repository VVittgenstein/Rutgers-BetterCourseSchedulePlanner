//! Startup re-derivation of stored delivery columns: a database written under
//! the legacy v1 rule must become servable in place, idempotently, and a stamp
//! from a newer binary must be re-derived rather than trusted.

use std::collections::BTreeMap;

use bcsp_catalog::{
    CATALOG_DERIVATION_VERSION, LEGACY_CATALOG_DERIVATION_VERSION, ProjectionError,
    RederivationError, normalize_target, rederive_stored_delivery, rederive_stored_delivery_now,
    to_catalog_refresh_command, to_normalized_catalog_v1,
};
use bcsp_contracts::{CatalogSynchronicity, TermCampusKey, TraceId};
use bcsp_operational_storage::{
    BeginRefreshAttemptCommand, CatalogDeliveryRewrite, EmptySnapshotDecision,
    FinishRefreshFailureCommand, OccurrenceDeliveryRewrite, OperationalStorage, PublishOutcome,
    PublishedCatalogSnapshot, RefreshFailureStage, SectionDeliveryRewrite,
};
use bcsp_rutgers_client::{SourceProvenance, decode_catalog_payload};

const STARTED: &str = "2030-01-01T00:00:00Z";
const COMPLETED: &str = "2030-01-01T00:00:01Z";
const REDERIVED_AT: &str = "2030-01-02T00:00:00Z";

fn trace_id(suffix: u8) -> TraceId {
    format!("00000000-0000-4000-8000-{suffix:012x}")
        .parse()
        .expect("synthetic v4 trace ID")
}

fn real_shape_body() -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!([{
        "campusCode": "NB",
        "courseString": "01:198:111",
        "subject": "198",
        "courseNumber": "111",
        "title": "Synthetic Online Course",
        "sections": [
            {
                "campusCode": "NB",
                "index": "10001",
                "number": "90",
                "sectionCourseType": "O",
                "meetingTimes": [{
                    "meetingModeCode": "90",
                    "meetingDay": "",
                    "startTimeMilitary": "",
                    "endTimeMilitary": "",
                    "baClassHours": "B",
                    "campusLocation": "O"
                }]
            },
            {
                "campusCode": "NB",
                "index": "10002",
                "number": "02",
                "sectionCourseType": "H",
                "meetingTimes": [
                    {
                        "meetingModeCode": "02",
                        "meetingDay": "W",
                        "startTimeMilitary": "1000",
                        "endTimeMilitary": "1120",
                        "campusLocation": "NB"
                    },
                    {
                        "meetingModeCode": "90",
                        "meetingDay": "",
                        "startTimeMilitary": "",
                        "endTimeMilitary": "",
                        "baClassHours": "B",
                        "campusLocation": "O"
                    }
                ]
            },
            {
                "campusCode": "NB",
                "index": "10003",
                "number": "03",
                "sectionCourseType": "O",
                "meetingTimes": [{
                    "meetingModeCode": "92",
                    "meetingDay": "M",
                    "startTimeMilitary": "0900",
                    "endTimeMilitary": "1020",
                    "campusLocation": "O"
                }]
            }
        ]
    }]))
    .expect("synthetic Catalog JSON")
}

fn publish(storage: &mut OperationalStorage, target: &TermCampusKey, suffix: u8) {
    let body = real_shape_body();
    let normalized = normalize_target(
        target.clone(),
        decode_catalog_payload(&body).expect("decode synthetic Catalog"),
        SourceProvenance::from_body("SYN_REDERIVE", STARTED, &body),
    )
    .expect("normalize synthetic Catalog");
    let outcome = storage
        .apply_catalog_refresh(
            to_catalog_refresh_command(&normalized, trace_id(suffix), STARTED, COMPLETED)
                .expect("Catalog command"),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            CATALOG_DERIVATION_VERSION,
        )
        .expect("Catalog publication");
    assert!(matches!(outcome, PublishOutcome::AppliedChanged { .. }));
}

fn published(storage: &mut OperationalStorage, target: &TermCampusKey) -> PublishedCatalogSnapshot {
    storage
        .published_catalog_snapshot(target)
        .expect("read published Catalog")
        .expect("published Catalog")
}

/// What the v0.1.1 (v1) rule stored for this fixture: every mode-90 occurrence
/// UNSPECIFIED / GENERIC_ONLINE_UNSPECIFIED, and any section whose occurrence
/// set is therefore heterogeneous or unspecified degraded accordingly.
fn legacy_rewrite(published: &PublishedCatalogSnapshot, version: u32) -> CatalogDeliveryRewrite {
    let mut sections = Vec::new();
    for section in &published.snapshot.sections {
        let legacy = match section.synchronicity.as_str() {
            "ASYNC" => "UNSPECIFIED",
            "MIXED" => "UNKNOWN",
            _ => continue,
        };
        sections.push(SectionDeliveryRewrite {
            key: section.key.clone(),
            delivery_modality: section.delivery_modality.clone(),
            synchronicity: legacy.to_owned(),
        });
    }
    let mut occurrences = Vec::new();
    for occurrence in &published.snapshot.occurrences {
        if occurrence.normalization_reason != "ONLINE_BY_ARRANGEMENT_ASYNCHRONOUS" {
            continue;
        }
        occurrences.push(OccurrenceDeliveryRewrite {
            section_key: occurrence.section_key.clone(),
            occurrence_key: occurrence.occurrence_key.clone(),
            occurrence_kind: occurrence.occurrence_kind.clone(),
            modality: occurrence.modality.clone(),
            synchronicity: "UNSPECIFIED".to_owned(),
            evidence: occurrence.evidence.clone(),
            normalization_reason: "GENERIC_ONLINE_UNSPECIFIED".to_owned(),
        });
    }
    assert_eq!((sections.len(), occurrences.len()), (2, 2));
    CatalogDeliveryRewrite {
        derivation_version: version,
        stamped_at: STARTED.to_owned(),
        sections,
        occurrences,
    }
}

fn stamps(storage: &OperationalStorage) -> BTreeMap<TermCampusKey, u32> {
    storage
        .catalog_derivation_versions()
        .expect("derivation stamps")
}

fn projected_synchronicities(
    published: &PublishedCatalogSnapshot,
) -> BTreeMap<String, CatalogSynchronicity> {
    to_normalized_catalog_v1(published)
        .expect("projection")
        .sections
        .into_iter()
        .map(|section| {
            (
                section.key.index().as_str().to_owned(),
                section.synchronicity,
            )
        })
        .collect()
}

#[test]
fn legacy_rows_are_rederived_in_place_idempotently_and_a_newer_stamp_is_rederived_too() {
    let target = TermCampusKey::try_new("92026", "NB").expect("target");
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    publish(&mut storage, &target, 1);
    let fresh = published(&mut storage, &target);
    assert_eq!(
        stamps(&storage).get(&target),
        Some(&CATALOG_DERIVATION_VERSION)
    );

    // A binary that already matches the stamp touches nothing.
    let report = rederive_stored_delivery(&mut storage, REDERIVED_AT).expect("no-op pass");
    assert!(report.is_noop());
    assert_eq!(report.derivation_version, CATALOG_DERIVATION_VERSION);
    assert_eq!(published(&mut storage, &target), fresh);

    // Simulate a database written by v0.1.1: legacy derived values under the
    // legacy stamp. The projection rejects those rows, which is exactly the
    // upgrade fail-closed that the startup pass exists to prevent.
    storage
        .rewrite_catalog_delivery(
            &target,
            &legacy_rewrite(&fresh, LEGACY_CATALOG_DERIVATION_VERSION),
        )
        .expect("write legacy rows");
    let legacy = published(&mut storage, &target);
    assert_ne!(legacy, fresh);
    assert!(matches!(
        to_normalized_catalog_v1(&legacy),
        Err(ProjectionError::InvalidStoredProjection { .. })
    ));

    let report = rederive_stored_delivery(&mut storage, REDERIVED_AT).expect("re-derivation");
    assert_eq!(report.targets.len(), 1);
    assert_eq!(report.targets[0].target, target);
    assert_eq!(
        report.targets[0].previous_version,
        LEGACY_CATALOG_DERIVATION_VERSION
    );
    assert_eq!(report.targets[0].content_version, 1);
    assert_eq!(report.sections_rewritten(), 2);
    assert_eq!(report.occurrences_rewritten(), 2);
    assert_eq!(published(&mut storage, &target), fresh);
    assert_eq!(
        stamps(&storage).get(&target),
        Some(&CATALOG_DERIVATION_VERSION)
    );
    let synchronicities = projected_synchronicities(&published(&mut storage, &target));
    assert_eq!(synchronicities["10001"], CatalogSynchronicity::Async);
    assert_eq!(synchronicities["10002"], CatalogSynchronicity::Mixed);
    assert_eq!(synchronicities["10003"], CatalogSynchronicity::Sync);

    // Second run: idempotent.
    let report = rederive_stored_delivery(&mut storage, REDERIVED_AT).expect("second pass");
    assert!(report.is_noop());
    assert_eq!(published(&mut storage, &target), fresh);

    // A stamp HIGHER than the version this binary carries (a downgrade) is
    // re-derived as well: the stamp is never trusted over the rule in the binary.
    storage
        .rewrite_catalog_delivery(&target, &legacy_rewrite(&fresh, 99))
        .expect("write rows under a future stamp");
    assert_eq!(stamps(&storage).get(&target), Some(&99));
    let report = rederive_stored_delivery(&mut storage, REDERIVED_AT).expect("downgrade pass");
    assert_eq!(report.targets.len(), 1);
    assert_eq!(report.targets[0].previous_version, 99);
    assert_eq!(report.sections_rewritten(), 2);
    assert_eq!(report.occurrences_rewritten(), 2);
    assert_eq!(published(&mut storage, &target), fresh);
    assert_eq!(
        stamps(&storage).get(&target),
        Some(&CATALOG_DERIVATION_VERSION)
    );

    // The clock-driven entry point is the same pass.
    let report = rederive_stored_delivery_now(&mut storage).expect("clock-driven pass");
    assert!(report.is_noop());
}

#[test]
fn unstamped_targets_without_serving_rows_are_stamped_and_other_targets_are_untouched() {
    let published_target = TermCampusKey::try_new("92026", "NB").expect("target");
    let failed_target = TermCampusKey::try_new("92026", "CM").expect("target");
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    publish(&mut storage, &published_target, 1);
    let fresh = published(&mut storage, &published_target);
    storage
        .begin_refresh_attempt(&BeginRefreshAttemptCommand {
            observation_id: trace_id(2),
            target: failed_target.clone(),
            started_at: STARTED.to_owned(),
            source_content_sha256: None,
            source_bytes: None,
        })
        .expect("begin attempt");
    storage
        .finish_refresh_failure(&FinishRefreshFailureCommand {
            observation_id: trace_id(2),
            completed_at: COMPLETED.to_owned(),
            stage: RefreshFailureStage::Transport,
            source_content_sha256: None,
            source_bytes: None,
            error_code: "UPSTREAM_TIMEOUT".to_owned(),
            diagnostic_token: None,
        })
        .expect("finish failure");
    assert!(!stamps(&storage).contains_key(&failed_target));

    let report = rederive_stored_delivery(&mut storage, REDERIVED_AT).expect("stamp pass");
    assert_eq!(report.targets.len(), 1);
    assert_eq!(report.targets[0].target, failed_target);
    assert_eq!(
        report.targets[0].previous_version,
        LEGACY_CATALOG_DERIVATION_VERSION
    );
    assert_eq!(report.targets[0].content_version, 0);
    assert_eq!(report.sections_rewritten(), 0);
    assert_eq!(report.occurrences_rewritten(), 0);
    assert_eq!(
        stamps(&storage).get(&failed_target),
        Some(&CATALOG_DERIVATION_VERSION)
    );
    assert_eq!(published(&mut storage, &published_target), fresh);
    assert!(
        storage
            .published_catalog_snapshot(&failed_target)
            .expect("read")
            .is_none()
    );

    let report = rederive_stored_delivery(&mut storage, REDERIVED_AT).expect("second pass");
    assert!(report.is_noop());
}

#[test]
fn rederivation_fails_closed_before_writing_when_storage_rejects_the_stamp() {
    let target = TermCampusKey::try_new("92026", "NB").expect("target");
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    publish(&mut storage, &target, 1);
    // A current stamp never reaches storage, so an unusable timestamp is harmless.
    assert!(rederive_stored_delivery(&mut storage, "").is_ok());
    let legacy = legacy_rewrite(&published(&mut storage, &target), 1);
    storage
        .rewrite_catalog_delivery(&target, &legacy)
        .expect("legacy rows");
    // Storage rejects the empty timestamp before touching any row.
    assert!(matches!(
        rederive_stored_delivery(&mut storage, ""),
        Err(RederivationError::Storage(_))
    ));
    assert_eq!(stamps(&storage).get(&target), Some(&1));
    assert!(to_normalized_catalog_v1(&published(&mut storage, &target)).is_err());
}
