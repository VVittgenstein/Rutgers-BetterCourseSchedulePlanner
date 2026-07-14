use bcsp_catalog::{
    normalize_target, to_catalog_refresh_command, to_discovery_refresh_command,
    to_operational_discovery_snapshot, to_operational_snapshot,
};
use bcsp_contracts::{TermCampusKey, TraceId};
use bcsp_operational_storage::{
    CatalogSnapshot, DiscoveryPublishOutcome, DiscoverySnapshot as StoredDiscoverySnapshot,
    EmptySnapshotDecision, OperationalStorage, PublishOutcome, catalog_content_sha256_v1,
};
use bcsp_rutgers_client::{
    DiscoverySnapshot, DiscoverySourceInput, DiscoverySourceKind, SourceProvenance,
    decode_catalog_payload, decode_discovery_payload,
};

const STARTED: &str = "2030-01-01T00:00:00Z";
const COMPLETED: &str = "2030-01-01T00:00:01Z";

fn trace_id(suffix: u8) -> TraceId {
    format!("00000000-0000-4000-8000-{suffix:012x}")
        .parse()
        .expect("synthetic v4 trace ID")
}

fn catalog_target(body: &[u8], request_id: &str) -> bcsp_catalog::NormalizedCatalogTarget {
    let target = TermCampusKey::try_new("T2030", "SYN_MAP").expect("synthetic target");
    let raw = decode_catalog_payload(body).expect("synthetic Catalog JSON");
    normalize_target(
        target,
        raw,
        SourceProvenance::from_body(request_id, STARTED, body),
    )
    .expect("synthetic Catalog target should normalize")
}

fn catalog_body() -> &'static [u8] {
    br#"[
      {
        "campusCode":"SYN_MAP",
        "courseString":"SYN:MAP:001",
        "subject":"SYN_SUBJECT",
        "courseNumber":"001",
        "title":"SYNTHETIC MAPPING",
        "openSections":3,
        "courseFeeDescr":null,
        "courseNotes":"",
        "preReqNotes":7,
        "sections":[{
          "campusCode":"SYN_MAP",
          "index":"70001",
          "number":"01",
          "sectionCourseType":"O",
          "openStatus":true,
          "meetingTimes":[{
            "meetingModeCode":"93",
            "meetingDay":"TTH",
            "startTimeMilitary":"1000",
            "endTimeMilitary":"1100",
            "campusLocation":"O"
          }],
          "instructors":[{"name":"SYNTHETIC_INSTRUCTOR_A"}]
        }]
      }
    ]"#
}

#[test]
fn catalog_command_matches_independent_storage_hash_and_preserves_projection_facts() {
    let normalized = catalog_target(
        catalog_body(),
        "SYNTHETIC_REQUEST_ID_MUST_NOT_REACH_STORED_PROVENANCE",
    );
    let command = to_catalog_refresh_command(&normalized, trace_id(1), STARTED, COMPLETED)
        .expect("mapping should succeed");

    assert_eq!(
        command.semantic_content_sha256,
        normalized.content_hash.as_str()
    );
    assert_eq!(
        catalog_content_sha256_v1(&command.target, &command.snapshot)
            .expect("storage should independently hash the snapshot"),
        normalized.content_hash.as_str()
    );
    assert_eq!(command.source_content_sha256.len(), 64);
    assert_eq!(command.raw_payload, None);

    let group = &command.snapshot.course_groups[0];
    assert!(group.canonical_facts.get("title").is_none());
    assert!(group.canonical_facts.get("courseDescription").is_none());
    let variant = &command.snapshot.course_variants[0];
    assert_eq!(variant.title.as_deref(), Some("SYNTHETIC MAPPING"));
    assert_eq!(
        variant.canonical_facts["courseFeeDescr"],
        serde_json::Value::Null
    );
    assert_eq!(variant.canonical_facts["courseNotes"], "");
    assert_eq!(variant.canonical_facts["title"], "SYNTHETIC MAPPING");
    assert!(variant.canonical_facts.get("courseFee").is_none());
    assert_eq!(
        variant.canonical_facts["preReqNotes"]["$malformed"]["jsonType"],
        "NUMBER"
    );

    let occurrence = &command.snapshot.occurrences[0];
    assert_eq!(occurrence.weekday.as_deref(), Some("T,H"));
    assert_eq!(
        (occurrence.start_minute, occurrence.end_minute),
        (Some(600), Some(660))
    );
    assert_eq!(occurrence.time_knowledge, "KNOWN");
    assert_eq!(occurrence.modality, "ONLINE");
    assert_eq!(occurrence.synchronicity, "ASYNC");
    let section = &command.snapshot.sections[0];
    assert_eq!(section.delivery_modality, "ONLINE");
    assert_eq!(section.synchronicity, "ASYNC");

    let serialized = serde_json::to_string(&command.snapshot).expect("snapshot serialization");
    assert!(!serialized.contains("SYNTHETIC_REQUEST_ID_MUST_NOT_REACH_STORED_PROVENANCE"));
    let round_trip: CatalogSnapshot =
        serde_json::from_str(&serialized).expect("snapshot round trip");
    assert_eq!(round_trip, command.snapshot);
}

#[test]
fn mapped_catalog_publishes_changed_then_unchanged_without_content_version_churn() {
    let normalized = catalog_target(catalog_body(), "SYNTHETIC_ATTEMPT");
    let mut storage = OperationalStorage::open_in_memory().expect("in-memory storage");

    let first = storage
        .apply_catalog_refresh(
            to_catalog_refresh_command(&normalized, trace_id(2), STARTED, COMPLETED)
                .expect("first command"),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
        )
        .expect("first publication");
    assert!(matches!(
        first,
        PublishOutcome::AppliedChanged {
            content_version: 1,
            ..
        }
    ));

    let second = storage
        .apply_catalog_refresh(
            to_catalog_refresh_command(&normalized, trace_id(3), STARTED, COMPLETED)
                .expect("second command"),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
        )
        .expect("unchanged publication");
    assert!(matches!(
        second,
        PublishOutcome::AppliedUnchanged {
            content_version: 1,
            ..
        }
    ));
    let state = storage
        .target_state(&normalized.target)
        .expect("target state")
        .expect("published target");
    assert_eq!(
        state.accepted_semantic_sha256.as_deref(),
        Some(normalized.content_hash.as_str())
    );
}

#[test]
fn open_sections_is_snapshot_only_provenance_and_never_semantic_identity() {
    let baseline = catalog_target(catalog_body(), "SYNTHETIC_OPEN_COUNT_A");
    let changed_body = std::str::from_utf8(catalog_body())
        .expect("synthetic UTF-8")
        .replace(r#""openSections":3"#, r#""openSections":9"#);
    let changed = catalog_target(changed_body.as_bytes(), "SYNTHETIC_OPEN_COUNT_B");
    assert_eq!(
        baseline.groups[0].variants[0].key.fingerprint(),
        changed.groups[0].variants[0].key.fingerprint()
    );
    assert_eq!(baseline.content_hash, changed.content_hash);
    assert!(
        baseline.groups[0].variants[0]
            .canonical_course_facts
            .get("openSections")
            .is_none()
    );

    let baseline_command = to_catalog_refresh_command(&baseline, trace_id(4), STARTED, COMPLETED)
        .expect("baseline command");
    let changed_command = to_catalog_refresh_command(&changed, trace_id(5), STARTED, COMPLETED)
        .expect("changed command");
    let snapshot_detail = |snapshot: &CatalogSnapshot| {
        snapshot
            .provenance
            .iter()
            .find(|fact| fact.field_name == "catalogOpenSectionsSnapshot")
            .expect("openSections snapshot provenance")
            .detail
            .clone()
    };
    let baseline_fact = snapshot_detail(&baseline_command.snapshot);
    let changed_fact = snapshot_detail(&changed_command.snapshot);
    assert_eq!(baseline_fact["catalogOpenSectionsIsSnapshotOnly"], true);
    assert_eq!(baseline_fact["openSections"]["state"], "VALUE");
    assert_eq!(baseline_fact["openSections"]["value"], 3);
    assert_eq!(changed_fact["openSections"]["value"], 9);
    assert_eq!(
        baseline_fact["rawCourseSha256"]
            .as_str()
            .expect("domain-tagged hash")
            .len(),
        64
    );
    let serialized =
        serde_json::to_string(&baseline_command.snapshot).expect("snapshot serialization");
    let round_trip: CatalogSnapshot =
        serde_json::from_str(&serialized).expect("snapshot round trip");
    assert_eq!(snapshot_detail(&round_trip), baseline_fact);

    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    assert!(matches!(
        storage
            .apply_catalog_refresh(
                baseline_command,
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            )
            .expect("baseline publish"),
        PublishOutcome::AppliedChanged {
            content_version: 1,
            ..
        }
    ));
    assert!(matches!(
        storage
            .apply_catalog_refresh(
                changed_command,
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            )
            .expect("snapshot-only change publish"),
        PublishOutcome::AppliedUnchanged {
            content_version: 1,
            ..
        }
    ));
}

#[test]
fn equivalent_duplicate_audit_is_complete_sorted_and_publishable() {
    let body = br#"[
      {
        "campusCode":"SYN_MAP","courseString":"SYN:MAP:002","title":"SYNTHETIC DUPLICATE",
        "sections":[{
          "campusCode":"SYN_MAP","index":"70002",
          "comments":["SYN_A","SYN_B"],"commentsText":"SYN_A SYN_B",
          "meetingTimes":[],"instructors":[]
        }]
      },
      {
        "campusCode":"SYN_MAP","courseString":"SYN:MAP:002","title":"SYNTHETIC DUPLICATE",
        "sections":[{
          "campusCode":"SYN_MAP","index":"70002",
          "comments":["SYN_B","SYN_A"],"commentsText":"SYN_B SYN_A",
          "meetingTimes":[],"instructors":[]
        }]
      }
    ]"#;
    let normalized = catalog_target(body, "SYNTHETIC_DUPLICATE_AUDIT");
    assert_eq!(normalized.duplicate_audits.len(), 1);
    let command = to_catalog_refresh_command(&normalized, trace_id(6), STARTED, COMPLETED)
        .expect("duplicate audit command");
    let audit = command
        .snapshot
        .provenance
        .iter()
        .find(|fact| fact.field_name == "duplicateAudit")
        .expect("duplicate audit provenance");
    assert_eq!(audit.detail["rawRecordCount"], 2);
    assert_eq!(audit.detail["disposition"], "AUDITED_EQUIVALENT_COLLAPSE");
    assert_eq!(audit.detail["reason"], "ALL_SECTION_SEMANTIC_HASHES_EQUAL");
    let hashes = audit.detail["rawRecordSha256"]
        .as_array()
        .expect("raw record hashes");
    assert_eq!(hashes.len(), 2);
    assert!(
        hashes[0].as_str().expect("hash") < hashes[1].as_str().expect("hash"),
        "audit raw hashes must be sorted"
    );
    assert_eq!(
        audit.detail["semanticSha256"]
            .as_str()
            .expect("semantic hash")
            .len(),
        64
    );
    let serialized = serde_json::to_string(&command.snapshot).expect("snapshot serialization");
    let round_trip: CatalogSnapshot =
        serde_json::from_str(&serialized).expect("snapshot round trip");
    assert!(
        round_trip
            .provenance
            .iter()
            .any(|fact| fact.field_name == "duplicateAudit")
    );
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    assert!(matches!(
        storage
            .apply_catalog_refresh(
                command,
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            )
            .expect("duplicate audit publication"),
        PublishOutcome::AppliedChanged { .. }
    ));
}

fn discovery_snapshot() -> DiscoverySnapshot {
    let body = br#"{
      "sourceVersion":"future-v7",
      "terms":[{
        "termId":"T2030",
        "year":2030,
        "termCode":null,
        "display":"",
        "published":true
      }],
      "campuses":[
        {
          "campusCode":"SYN_FUTURE",
          "display":"Synthetic Future Campus",
          "category":7,
          "enabled":true
        },
        {
          "campusCode":"SYN_EMPTY",
          "display":"",
          "category":null,
          "enabled":null
        }
      ],
      "targets":[
        {"termId":"T2030","campusCode":"SYN_FUTURE","enabled":true},
        {"termId":"T2030","campusCode":"SYN_EMPTY","enabled":true}
      ],
      "subjects":[{
        "termId":"T2030",
        "campusCode":"SYN_FUTURE",
        "subjectCode":"\u672a\u6765\u5b66\u79d1",
        "display":"",
        "enabled":true
      }]
    }"#;
    let raw = decode_discovery_payload(body).expect("synthetic discovery JSON");
    DiscoverySnapshot::try_from_raw(
        raw,
        SourceProvenance::from_body("SYNTHETIC_DISCOVERY_REQUEST", STARTED, body),
    )
    .expect("synthetic discovery should normalize")
}

#[test]
fn discovery_mapping_is_dynamic_lossless_and_keeps_an_empty_campus() {
    let discovery = discovery_snapshot();
    let snapshot =
        to_operational_discovery_snapshot(&discovery).expect("discovery mapping should succeed");

    assert_eq!(snapshot.sources.len(), 1);
    assert!(
        snapshot.sources[0]
            .source_version_id
            .starts_with("selector:")
    );
    assert_eq!(snapshot.sources[0].source_version_id.len(), 73);
    assert_eq!(snapshot.sources[0].source_identity, "RUTGERS_SELECTOR");
    assert_eq!(
        snapshot.sources[0].canonical_facts["reportedSourceVersion"]["value"],
        "future-v7"
    );
    assert_eq!(snapshot.campuses.len(), 2);
    assert!(snapshot.campuses.iter().any(|campus| {
        campus.target.campus().as_str() == "SYN_FUTURE"
            && campus.display_name.as_deref() == Some("Synthetic Future Campus")
    }));
    let empty = snapshot
        .campuses
        .iter()
        .find(|campus| campus.target.campus().as_str() == "SYN_EMPTY")
        .expect("selector-confirmed empty campus remains discoverable");
    assert_eq!(empty.display_name.as_deref(), Some(""));
    assert_eq!(
        empty.canonical_facts["campusCategory"]["state"],
        "EXPLICIT_NULL"
    );
    let future = snapshot
        .campuses
        .iter()
        .find(|campus| campus.target.campus().as_str() == "SYN_FUTURE")
        .expect("future campus");
    assert_eq!(
        future.canonical_facts["campusCategory"]["state"],
        "MALFORMED"
    );
    assert_eq!(snapshot.subjects.len(), 1);
    assert_eq!(snapshot.subjects[0].subject_code, "未来学科");
    assert_eq!(
        snapshot.terms[0].canonical_facts["termCode"]["state"],
        "EXPLICIT_NULL"
    );
    assert_eq!(snapshot.terms[0].canonical_facts["display"]["value"], "");

    let serialized = serde_json::to_string(&snapshot).expect("discovery serialization");
    let round_trip: StoredDiscoverySnapshot =
        serde_json::from_str(&serialized).expect("discovery round trip");
    assert_eq!(round_trip, snapshot);

    let mut storage = OperationalStorage::open_in_memory().expect("in-memory storage");
    let changed = storage
        .apply_discovery_refresh(
            to_discovery_refresh_command(&discovery, trace_id(7), STARTED, COMPLETED)
                .expect("discovery command"),
        )
        .expect("discovery publication");
    assert_eq!(
        changed,
        DiscoveryPublishOutcome::AppliedChanged { content_version: 1 }
    );
    let targets = storage.discovered_targets().expect("discovered targets");
    assert_eq!(targets.len(), 2);
    assert!(
        targets
            .iter()
            .any(|target| target.campus().as_str() == "SYN_EMPTY")
    );
}

#[test]
fn direct_snapshot_mapper_and_command_mapper_share_exact_catalog_projection() {
    let normalized = catalog_target(catalog_body(), "SYNTHETIC_ATTEMPT");
    let direct = to_operational_snapshot(&normalized).expect("direct snapshot");
    let command = to_catalog_refresh_command(&normalized, trace_id(8), STARTED, COMPLETED)
        .expect("command snapshot");
    assert_eq!(direct, command.snapshot);
}

#[test]
fn selector_identity_is_stable_while_source_versions_bind_content() {
    let first_body = br#"{
      "sourceVersion":"reused-version",
      "terms":[{"termId":"T2030"}],
      "campuses":[{"campusCode":"SYN_FUTURE","display":"Synthetic A"}],
      "targets":[{"termId":"T2030","campusCode":"SYN_FUTURE"}],
      "subjects":[]
    }"#;
    let second_body = br#"{
      "sourceVersion":"reused-version",
      "terms":[{"termId":"T2030"}],
      "campuses":[{"campusCode":"SYN_FUTURE","display":"Synthetic B"}],
      "targets":[{"termId":"T2030","campusCode":"SYN_FUTURE"}],
      "subjects":[]
    }"#;
    let make = |body: &[u8], request_id: &str, observed_at: &str| {
        let raw = decode_discovery_payload(body).expect("synthetic discovery JSON");
        DiscoverySnapshot::try_from_raw(
            raw,
            SourceProvenance::from_body(request_id, observed_at, body),
        )
        .expect("synthetic discovery should normalize")
    };
    let first = to_operational_discovery_snapshot(&make(
        first_body,
        "SYNTHETIC_REQUEST_A",
        "2030-01-01T00:00:00Z",
    ))
    .expect("first mapping");
    let second = to_operational_discovery_snapshot(&make(
        second_body,
        "SYNTHETIC_REQUEST_B",
        "2030-01-02T00:00:00Z",
    ))
    .expect("second mapping");

    assert_eq!(first.sources[0].source_identity, "RUTGERS_SELECTOR");
    assert_eq!(second.sources[0].source_identity, "RUTGERS_SELECTOR");
    assert_ne!(
        first.sources[0].source_version_id,
        second.sources[0].source_version_id
    );
    let encoded = serde_json::to_string(&(first, second)).expect("source serialization");
    assert!(!encoded.contains("SYNTHETIC_REQUEST_A"));
    assert!(!encoded.contains("SYNTHETIC_REQUEST_B"));
}

#[test]
fn identical_selector_content_at_a_later_observation_is_unchanged() {
    let first = discovery_snapshot();
    let mut later = first.clone();
    later.sources[0].provenance.request_id = "SYNTHETIC_DISCOVERY_RETRY".to_owned();
    later.sources[0].provenance.observed_at_utc = "2030-01-02T00:00:00Z".to_owned();
    let first_projection = to_operational_discovery_snapshot(&first).expect("first projection");
    let later_projection = to_operational_discovery_snapshot(&later).expect("later projection");
    assert_eq!(
        first_projection.sources[0].source_version_id,
        later_projection.sources[0].source_version_id
    );
    assert_eq!(
        first_projection.sources[0].source_identity,
        later_projection.sources[0].source_identity
    );

    let mut storage = OperationalStorage::open_in_memory().expect("in-memory storage");
    let changed = storage
        .apply_discovery_refresh(
            to_discovery_refresh_command(&first, trace_id(9), STARTED, COMPLETED)
                .expect("first command"),
        )
        .expect("first apply");
    assert_eq!(
        changed,
        DiscoveryPublishOutcome::AppliedChanged { content_version: 1 }
    );
    let unchanged = storage
        .apply_discovery_refresh(
            to_discovery_refresh_command(
                &later,
                trace_id(10),
                "2030-01-02T00:00:00Z",
                "2030-01-02T00:00:01Z",
            )
            .expect("retry command"),
        )
        .expect("identical-content retry");
    assert_eq!(
        unchanged,
        DiscoveryPublishOutcome::AppliedUnchanged { content_version: 1 }
    );
}

#[test]
fn source_metadata_recoverably_preserves_all_five_presence_states_even_without_entities() {
    let cases = [
        (
            r#"{"terms":[],"campuses":[],"targets":[],"subjects":[]}"#,
            "MISSING",
            None,
        ),
        (
            r#"{"sourceVersion":null,"terms":[],"campuses":[],"targets":[],"subjects":[]}"#,
            "EXPLICIT_NULL",
            None,
        ),
        (
            r#"{"sourceVersion":7,"terms":[],"campuses":[],"targets":[],"subjects":[]}"#,
            "MALFORMED",
            None,
        ),
        (
            r#"{"sourceVersion":"","terms":[],"campuses":[],"targets":[],"subjects":[]}"#,
            "VALUE",
            Some(""),
        ),
        (
            r#"{"sourceVersion":"synthetic-v1","terms":[],"campuses":[],"targets":[],"subjects":[]}"#,
            "VALUE",
            Some("synthetic-v1"),
        ),
    ];
    for (body, expected_state, expected_value) in cases {
        let raw = decode_discovery_payload(body.as_bytes()).expect("synthetic discovery JSON");
        let discovery = DiscoverySnapshot::try_from_raw(
            raw,
            SourceProvenance::from_body("SYNTHETIC_SOURCE_STATE", STARTED, body.as_bytes()),
        )
        .expect("empty synthetic discovery should normalize");
        let mapped =
            to_operational_discovery_snapshot(&discovery).expect("source metadata mapping");
        assert!(mapped.terms.is_empty());
        assert!(mapped.campuses.is_empty());
        assert!(mapped.subjects.is_empty());
        let fact = &mapped.sources[0].canonical_facts["reportedSourceVersion"];
        assert_eq!(fact["state"], expected_state);
        if let Some(expected_value) = expected_value {
            assert_eq!(fact["value"], expected_value);
        }
    }
}

#[test]
fn selector_and_bootstrap_sources_publish_without_ownership_misattribution() {
    let selector_body = br#"{
      "sourceVersion":"selector-v1",
      "terms":[{"termId":"T2030","display":"Synthetic Selector Term"}],
      "campuses":[{"campusCode":"SYN_SELECTOR","display":"Synthetic Selector Campus"}],
      "targets":[{"termId":"T2030","campusCode":"SYN_SELECTOR","enabled":true}],
      "subjects":[{"termId":"T2030","campusCode":"SYN_SELECTOR","subjectCode":"SYN_SELECTOR_SUBJECT"}]
    }"#;
    let bootstrap_body = br#"{
      "sourceVersion":"bootstrap-v1",
      "terms":[{"termId":"T2029","display":"Synthetic Bootstrap Term"}],
      "campuses":[{"campusCode":"SYN_BOOTSTRAP","display":"Synthetic Bootstrap Campus"}],
      "targets":[{"termId":"T2029","campusCode":"SYN_BOOTSTRAP","enabled":true}],
      "subjects":[]
    }"#;
    let input = |kind, body: &[u8], request_id: &str| {
        DiscoverySourceInput::new(
            kind,
            decode_discovery_payload(body).expect("synthetic discovery"),
            SourceProvenance::from_body(request_id, STARTED, body),
        )
    };
    let discovery = DiscoverySnapshot::try_from_bundle(vec![
        input(
            DiscoverySourceKind::Selector,
            selector_body,
            "SYN_SELECTOR_REQUEST",
        ),
        input(
            DiscoverySourceKind::Bootstrap,
            bootstrap_body,
            "SYN_BOOTSTRAP_REQUEST",
        ),
    ])
    .expect("multi-source discovery");
    let command = to_discovery_refresh_command(&discovery, trace_id(11), STARTED, COMPLETED)
        .expect("multi-source command");
    assert_eq!(command.snapshot.sources.len(), 2);
    let selector = command
        .snapshot
        .sources
        .iter()
        .find(|source| source.source_identity == "RUTGERS_SELECTOR")
        .expect("selector source");
    let bootstrap = command
        .snapshot
        .sources
        .iter()
        .find(|source| source.source_identity == "RBCSP_BOOTSTRAP")
        .expect("bootstrap source");
    assert!(selector.source_version_id.starts_with("selector:"));
    assert!(bootstrap.source_version_id.starts_with("bootstrap:"));
    assert!(command.snapshot.terms.iter().any(|term| {
        term.term_id.as_str() == "T2030" && term.source_version_id == selector.source_version_id
    }));
    assert!(command.snapshot.campuses.iter().any(|campus| {
        campus.target.term().as_str() == "T2029"
            && campus.source_version_id == bootstrap.source_version_id
    }));
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    assert_eq!(
        storage
            .apply_discovery_refresh(command)
            .expect("multi-source publication"),
        DiscoveryPublishOutcome::AppliedChanged { content_version: 1 }
    );
    assert_eq!(storage.discovered_targets().expect("targets").len(), 2);
}
