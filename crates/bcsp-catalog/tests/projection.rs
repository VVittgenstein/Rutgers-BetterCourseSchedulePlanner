use bcsp_catalog::{
    normalize_target, to_catalog_discovery_response_v1, to_catalog_refresh_checkpoint_v1,
    to_catalog_refresh_command, to_catalog_refresh_observation_v1, to_catalog_refresh_status_v1,
    to_discovery_refresh_command, to_normalized_catalog_v1,
};
use bcsp_contracts::{
    CatalogCommentV1, CatalogDiscoveryAvailability, CatalogDiscoverySourceKind,
    CatalogFieldKnowledge, CatalogModality, CatalogOccurrenceEvidence, CatalogPrerequisiteState,
    CatalogRefreshClassification, CatalogRefreshErrorClass, CatalogSynchronicity,
    CatalogTimeKnowledgeV1, CatalogUnitMajorV1, CatalogUnknownReason, NormalizedCatalogV1,
    TermCampusKey, TraceId,
};
use bcsp_operational_storage::{
    BeginDiscoveryAttemptCommand, BeginRefreshAttemptCommand, CatalogCounts,
    DiscoveryPublishOutcome, EmptySnapshotDecision, FinishDiscoveryFailureCommand,
    OperationalStorage, PublishOutcome, RefreshFailureStage, RefreshObservation, RefreshStatus,
};
use bcsp_rutgers_client::{
    DiscoverySnapshot, DiscoverySourceInput, DiscoverySourceKind, SourceProvenance,
    decode_catalog_payload, decode_discovery_payload,
};
use tempfile::TempDir;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const STARTED: &str = "2030-01-01T00:00:00Z";
const COMPLETED: &str = "2030-01-01T00:00:01Z";

fn trace_id(suffix: u8) -> TraceId {
    format!("00000000-0000-4000-8000-{suffix:012x}")
        .parse()
        .expect("synthetic v4 trace ID")
}

fn timestamp(value: &str) -> OffsetDateTime {
    OffsetDateTime::parse(value, &Rfc3339).expect("synthetic RFC 3339 timestamp")
}

fn catalog_body(campus: &str, course: &str, index: &str, reordered_sets: bool) -> Vec<u8> {
    let (campus_locations, core_codes, instructors) = if reordered_sets {
        (
            r#"[{"code":"A"},{"code":" Z "}]"#,
            r#"["A"," Z "]"#,
            r#"[{"name":"Alpha Instructor"},{"name":" Zed Instructor "}]"#,
        )
    } else {
        (
            r#"[{"code":" Z "},{"code":"A"}]"#,
            r#"[" Z ","A"]"#,
            r#"[{"name":" Zed Instructor "},{"name":"Alpha Instructor"}]"#,
        )
    };
    format!(
        r#"[{{
          "campusCode":"{campus}",
          "campusLocations":{campus_locations},
          "coreCodes":{core_codes},
          "courseString":"{course}",
          "subject":"SYN_SUBJECT",
          "subjectDescription":" Synthetic Subject ",
          "courseNumber":" 001 ",
          "title":" Synthetic Projection ",
          "courseDescription":null,
          "courseNotes":"",
          "subjectGroupNotes":" Synthetic Subject Group Note ",
          "subjectNotes":" Synthetic Subject Note ",
          "unitNotes":" Synthetic Unit Note ",
          "synopsisUrl":" https://example.invalid/synthetic-course ",
          "preReqNotes":7,
          "creditsObject":{{"code":" 3 "}},
          "school":{{"code":" SYN_SCHOOL "}},
          "offeringUnitCode":" SYN_UNIT ",
          "offeringUnitTitle":" Synthetic Unit Title ",
          "sections":[{{
            "campusCode":"{campus}",
            "index":"{index}",
            "number":" 01 ",
            "subtitle":" Synthetic Subtitle ",
            "subtopic":" Synthetic Subtopic ",
            "sectionNotes":" Synthetic Section Note ",
            "sessionDates":" 01/01/2030-05/01/2030 ",
            "sessionDatePrintIndicator":" Y ",
            "comments":[
              {{"code":" SYN_COMMENT ","description":" Synthetic Comment Description "}},
              " Synthetic Free-form Comment "
            ],
            "commentsText":" Synthetic Comment Description; Synthetic Free-form Comment ",
            "crossListedSections":[{{"code":" SYN:001:01 "}}],
            "crossListedSectionsText":" SYN:001:01 ",
            "crossListedSectionType":" CROSS_LISTED ",
            "sectionCourseType":" O ",
            "examCode":" SYN_EXAM ",
            "examCodeText":" Synthetic Exam ",
            "specialPermissionAddCode":" SYN_PERMISSION ",
            "specialPermissionAddCodeDescription":" Synthetic Permission ",
            "specialPermissionDropCode":" SYN_DROP ",
            "specialPermissionDropCodeDescription":" Synthetic Drop Permission ",
            "majors":[
              {{"code":" SYN_MAJOR ","isMajorCode":true,"isUnitCode":false}},
              {{"code":" SYN_UNIT ","isMajorCode":false,"isUnitCode":true}}
            ],
            "minors":[{{"code":" SYN_MINOR "}}],
            "honorPrograms":[{{"code":" SYN_HONOR "}}],
            "unitMajors":[{{"unitCode":" SYN_UNIT ","majorCode":" SYN_UNIT_MAJOR "}}],
            "sectionEligibility":" Synthetic Eligibility ",
            "openToText":" Synthetic Open To ",
            "openStatus":true,
            "instructors":{instructors},
            "meetingTimes":[{{
              "meetingModeCode":"92",
              "meetingModeDesc":" Remote exact description ",
              "meetingDay":"TH",
              "startTimeMilitary":"0900",
              "endTimeMilitary":"1020",
              "campusLocation":"O",
              "campusName":" Synthetic Campus Name ",
              "roomNumber":null
            }}]
          }}]
        }}]"#
    )
    .into_bytes()
}

fn normalized_catalog(
    target: TermCampusKey,
    body: &[u8],
    request_id: &str,
    observed_at: &str,
) -> bcsp_catalog::NormalizedCatalogTarget {
    normalize_target(
        target,
        decode_catalog_payload(body).expect("synthetic Catalog JSON"),
        SourceProvenance::from_body(request_id, observed_at, body),
    )
    .expect("synthetic Catalog target")
}

fn catalog_body_with_meeting_order(reversed: bool) -> Vec<u8> {
    let mut body: serde_json::Value =
        serde_json::from_slice(&catalog_body("SYN_ORDER", "SYN:ORDER:001", "73001", false))
            .expect("base Catalog JSON");
    let mut meetings = vec![
        serde_json::json!({
            "meetingModeCode": "92",
            "meetingDay": "M",
            "startTimeMilitary": "0900",
            "endTimeMilitary": "1000",
            "campusLocation": "O"
        }),
        serde_json::json!({
            "meetingModeCode": "93",
            "meetingDay": "T",
            "startTimeMilitary": "1100",
            "endTimeMilitary": "1200",
            "campusLocation": "O"
        }),
    ];
    if reversed {
        meetings.reverse();
    }
    body[0]["sections"][0]["meetingTimes"] = serde_json::Value::Array(meetings);
    serde_json::to_vec(&body).expect("ordered Catalog JSON")
}

fn discovery_bundle(observed_at: &str) -> DiscoverySnapshot {
    let selector = br#"{
      "sourceVersion":null,
      "terms":[
        {"termId":"T2030","published":true},
        {"termId":"T_DISABLED","display":"Disabled term","published":false}
      ],
      "campuses":[
        {"campusCode":"SYN_SELECTOR","display":null,"enabled":true},
        {"campusCode":"SYN_OFF","display":"Disabled target","enabled":true}
      ],
      "targets":[
        {"termId":"T2030","campusCode":"SYN_SELECTOR","enabled":true},
        {"termId":"T2030","campusCode":"SYN_OFF","enabled":false}
      ],
      "subjects":[
        {"termId":"T2030","campusCode":"SYN_SELECTOR","subjectCode":"SYN_SELECTOR_SUBJECT","display":7,"enabled":true},
        {"termId":"T2030","campusCode":"SYN_SELECTOR","subjectCode":"SYN_DISABLED_SUBJECT","display":"Disabled","enabled":false}
      ]
    }"#;
    let bootstrap = br#"{
      "terms":[{"termId":"T2029","display":"","published":true}],
      "campuses":[{"campusCode":"SYN_BOOTSTRAP","display":"Bootstrap Campus","enabled":true}],
      "targets":[{"termId":"T2029","campusCode":"SYN_BOOTSTRAP","enabled":true}],
      "subjects":[]
    }"#;
    let input = |kind, body: &[u8], request_id: &str| {
        DiscoverySourceInput::new(
            kind,
            decode_discovery_payload(body).expect("synthetic discovery JSON"),
            SourceProvenance::from_body(request_id, observed_at, body),
        )
    };
    DiscoverySnapshot::try_from_bundle(vec![
        input(
            DiscoverySourceKind::Selector,
            selector,
            "SYNTHETIC_SELECTOR_REQUEST",
        ),
        input(
            DiscoverySourceKind::Bootstrap,
            bootstrap,
            "SYNTHETIC_BOOTSTRAP_REQUEST",
        ),
    ])
    .expect("synthetic multi-source discovery")
}

#[test]
fn restart_projects_catalog_details_and_isolates_multiple_targets() {
    let directory = TempDir::new().expect("temporary storage directory");
    let database = directory.path().join("operational.sqlite3");
    let first_target = TermCampusKey::try_new("T2030", "SYN_API").expect("first target");
    let second_target = TermCampusKey::try_new("T2031", "SYN_SECOND").expect("second target");
    let first_body = catalog_body("SYN_API", "SYN:API:001", "70001", false);
    let second_body = catalog_body("SYN_SECOND", "SYN:SECOND:001", "80001", false);

    let mut storage = OperationalStorage::open(&database).expect("file-backed storage");
    for (target, body, observation) in [
        (&first_target, first_body.as_slice(), trace_id(1)),
        (&second_target, second_body.as_slice(), trace_id(2)),
    ] {
        let normalized = normalized_catalog(target.clone(), body, "SYNTHETIC_REQUEST", STARTED);
        assert!(matches!(
            storage
                .apply_catalog_refresh(
                    to_catalog_refresh_command(&normalized, observation, STARTED, COMPLETED)
                        .expect("Catalog command"),
                    EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
                )
                .expect("Catalog publication"),
            PublishOutcome::AppliedChanged {
                content_version: 1,
                ..
            }
        ));
    }
    drop(storage);

    let mut storage = OperationalStorage::open(&database).expect("reopened storage");
    let first = storage
        .published_catalog_snapshot(&first_target)
        .expect("first reader")
        .expect("first publication");
    let second = storage
        .published_catalog_snapshot(&second_target)
        .expect("second reader")
        .expect("second publication");
    let projected = to_normalized_catalog_v1(&first).expect("strict Catalog projection");
    let second_projected = to_normalized_catalog_v1(&second).expect("second projection");

    assert_eq!(projected.target, first_target);
    assert_eq!(second_projected.target, second_target);
    assert_ne!(
        projected.course_groups[0].key,
        second_projected.course_groups[0].key
    );
    let variant = &projected.course_variants[0];
    assert_eq!(
        variant.title,
        CatalogFieldKnowledge::present("Synthetic Projection".to_owned())
    );
    assert_eq!(variant.expanded_title, CatalogFieldKnowledge::absent());
    assert_eq!(variant.description, CatalogFieldKnowledge::explicit_null());
    assert_eq!(variant.notes, CatalogFieldKnowledge::present(String::new()));
    assert_eq!(
        variant.subject_group_notes,
        CatalogFieldKnowledge::present("Synthetic Subject Group Note".to_owned())
    );
    assert_eq!(
        variant.subject_notes,
        CatalogFieldKnowledge::present("Synthetic Subject Note".to_owned())
    );
    assert_eq!(
        variant.unit_notes,
        CatalogFieldKnowledge::present("Synthetic Unit Note".to_owned())
    );
    assert_eq!(
        variant.synopsis_url,
        CatalogFieldKnowledge::present("https://example.invalid/synthetic-course".to_owned())
    );
    assert_eq!(
        variant.prerequisite_notes,
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed)
    );
    assert_eq!(
        variant.prerequisite_state,
        CatalogPrerequisiteState::Unknown
    );
    assert_eq!(
        variant.core_codes,
        CatalogFieldKnowledge::present(vec!["A".to_owned(), "Z".to_owned()])
    );
    assert_eq!(
        variant.campus_locations,
        CatalogFieldKnowledge::present(vec!["A".to_owned(), "Z".to_owned()])
    );
    assert_eq!(
        variant.school_code,
        CatalogFieldKnowledge::present("SYN_SCHOOL".to_owned())
    );
    assert_eq!(
        variant.offering_unit_title,
        CatalogFieldKnowledge::present("Synthetic Unit Title".to_owned())
    );

    let section = &projected.sections[0];
    assert_eq!(
        section.section_number,
        CatalogFieldKnowledge::present("01".to_owned())
    );
    assert_eq!(
        section.subtitle,
        CatalogFieldKnowledge::present("Synthetic Subtitle".to_owned())
    );
    assert_eq!(
        section.subtopic,
        CatalogFieldKnowledge::present("Synthetic Subtopic".to_owned())
    );
    assert_eq!(
        section.section_notes,
        CatalogFieldKnowledge::present("Synthetic Section Note".to_owned())
    );
    assert_eq!(
        section.session_dates,
        CatalogFieldKnowledge::present("01/01/2030-05/01/2030".to_owned())
    );
    assert_eq!(
        section.session_date_print_indicator,
        CatalogFieldKnowledge::present("Y".to_owned())
    );
    assert_eq!(
        section.comments,
        CatalogFieldKnowledge::present(vec![
            CatalogCommentV1 {
                code: CatalogFieldKnowledge::absent(),
                description: CatalogFieldKnowledge::present(
                    "Synthetic Free-form Comment".to_owned()
                ),
            },
            CatalogCommentV1 {
                code: CatalogFieldKnowledge::present("SYN_COMMENT".to_owned()),
                description: CatalogFieldKnowledge::present(
                    "Synthetic Comment Description".to_owned()
                ),
            },
        ])
    );
    assert_eq!(
        section.comments_text,
        CatalogFieldKnowledge::present(
            "Synthetic Free-form Comment; Synthetic Comment Description".to_owned()
        )
    );
    assert_eq!(
        section.cross_listed_sections_text,
        CatalogFieldKnowledge::present("SYN:001:01".to_owned())
    );
    assert_eq!(
        section.cross_listed_section_type,
        CatalogFieldKnowledge::present("CROSS_LISTED".to_owned())
    );
    assert_eq!(
        section.instructors,
        CatalogFieldKnowledge::present(vec![
            "Alpha Instructor".to_owned(),
            "Zed Instructor".to_owned(),
        ])
    );
    assert_eq!(
        section.raw_section_course_type,
        CatalogFieldKnowledge::present("O".to_owned())
    );
    assert_eq!(section.delivery_modality, CatalogModality::Online);
    assert_eq!(section.synchronicity, CatalogSynchronicity::Sync);
    assert_eq!(
        section.exam_code,
        CatalogFieldKnowledge::present("SYN_EXAM".to_owned())
    );
    assert_eq!(
        section.exam_code_text,
        CatalogFieldKnowledge::present("Synthetic Exam".to_owned())
    );
    assert_eq!(
        section.special_permission_add_code,
        CatalogFieldKnowledge::present("SYN_PERMISSION".to_owned())
    );
    assert_eq!(
        section.special_permission_add_description,
        CatalogFieldKnowledge::present("Synthetic Permission".to_owned())
    );
    assert_eq!(
        section.special_permission_drop_code,
        CatalogFieldKnowledge::present("SYN_DROP".to_owned())
    );
    assert_eq!(
        section.special_permission_drop_description,
        CatalogFieldKnowledge::present("Synthetic Drop Permission".to_owned())
    );
    assert_eq!(
        section.major_codes,
        CatalogFieldKnowledge::present(vec!["SYN_MAJOR".to_owned()])
    );
    assert_eq!(
        section.unit_codes,
        CatalogFieldKnowledge::present(vec!["SYN_UNIT".to_owned()])
    );
    assert_eq!(
        section.minor_codes,
        CatalogFieldKnowledge::present(vec!["SYN_MINOR".to_owned()])
    );
    assert_eq!(
        section.honor_program_codes,
        CatalogFieldKnowledge::present(vec!["SYN_HONOR".to_owned()])
    );
    assert_eq!(
        section.unit_majors,
        CatalogFieldKnowledge::present(vec![CatalogUnitMajorV1 {
            unit_code: "SYN_UNIT".to_owned(),
            major_code: "SYN_UNIT_MAJOR".to_owned(),
        }])
    );
    assert_eq!(
        section.eligibility_text,
        CatalogFieldKnowledge::present("Synthetic Eligibility".to_owned())
    );
    assert_eq!(
        section.open_to_text,
        CatalogFieldKnowledge::present("Synthetic Open To".to_owned())
    );
    assert!(matches!(
        &section.occurrence_keys,
        CatalogFieldKnowledge::Known {
            presence: bcsp_contracts::CatalogFieldPresence::Present { value }
        } if value.len() == 1 && value[0].ordinal == 0
    ));
    assert_eq!(
        section.catalog_open_status.value,
        CatalogFieldKnowledge::present(true)
    );

    let occurrence = &projected.occurrences[0];
    assert_eq!(occurrence.key.ordinal, 0);
    assert_eq!(
        occurrence.raw_description,
        CatalogFieldKnowledge::present(" Remote exact description ".to_owned())
    );
    assert_eq!(
        occurrence.time,
        CatalogTimeKnowledgeV1::Known {
            start_minute: 540,
            end_minute: 620,
        }
    );
    assert_eq!(occurrence.evidence, CatalogOccurrenceEvidence::Remote);
    assert_eq!(
        occurrence.requiredness,
        bcsp_contracts::CatalogRequiredness::UnknownRequiredness
    );
    assert_eq!(
        occurrence.campus_name,
        CatalogFieldKnowledge::present(" Synthetic Campus Name ".to_owned())
    );
    assert_eq!(occurrence.building, CatalogFieldKnowledge::absent());
    assert_eq!(occurrence.room, CatalogFieldKnowledge::explicit_null());
    assert_eq!(
        occurrence.start_date,
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::NotObserved)
    );
    assert_eq!(projected.provenance.observation_id, trace_id(1));
    assert_eq!(projected.provenance.payload_digest.as_str().len(), 64);

    let encoded = serde_json::to_string(&projected).expect("public Catalog JSON");
    assert!(!encoded.contains("canonicalFacts"));
    assert!(!encoded.contains("finalExam"));
    let decoded: NormalizedCatalogV1 =
        serde_json::from_str(&encoded).expect("public Catalog round trip");
    assert_eq!(decoded, projected);

    let mut corrupted = first;
    corrupted.snapshot.course_variants[0].canonical_facts["title"] = serde_json::json!("CORRUPTED");
    assert!(to_normalized_catalog_v1(&corrupted).is_err());
}

#[test]
fn selector_scope_projects_payloads_with_a_different_upstream_campus() {
    let target = TermCampusKey::try_new("T2030", "SYN_SELECTOR").expect("selector target");
    let body = catalog_body("SYN_OFF", "SYN:OFF:001", "70999", false);
    let normalized = normalized_catalog(target.clone(), &body, "SYNTHETIC_REQUEST", STARTED);
    assert!(
        normalized.groups[0].variants[0]
            .canonical_course_facts
            .get("campusCode")
            .is_none()
    );
    assert_eq!(
        normalized.groups[0].variants[0].sections[0].canonical_facts["campusCode"],
        "SYN_OFF"
    );

    let mut storage = OperationalStorage::open_in_memory().expect("in-memory storage");
    storage
        .apply_catalog_refresh(
            to_catalog_refresh_command(&normalized, trace_id(40), STARTED, COMPLETED)
                .expect("Catalog command"),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
        )
        .expect("Catalog publication");
    let published = storage
        .published_catalog_snapshot(&target)
        .expect("Catalog reader")
        .expect("published target");
    let projected = to_normalized_catalog_v1(&published).expect("selector-scoped projection");

    assert_eq!(projected.target, target);
    assert_eq!(
        projected.course_groups[0].key.campus().as_str(),
        "SYN_SELECTOR"
    );
    assert_eq!(projected.sections[0].key.campus().as_str(), "SYN_SELECTOR");
}

#[test]
fn occurrence_key_knowledge_preserves_missing_null_malformed_and_empty_after_storage() {
    let cases = [
        ("SYN_OCC_MISSING", None, CatalogFieldKnowledge::absent()),
        (
            "SYN_OCC_NULL",
            Some(serde_json::Value::Null),
            CatalogFieldKnowledge::explicit_null(),
        ),
        (
            "SYN_OCC_MALFORMED",
            Some(serde_json::json!("MALFORMED_SYNTHETIC")),
            CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed),
        ),
        (
            "SYN_OCC_EMPTY",
            Some(serde_json::json!([])),
            CatalogFieldKnowledge::present(Vec::new()),
        ),
    ];
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    for (ordinal, (campus, meeting_times, expected)) in cases.into_iter().enumerate() {
        let target = TermCampusKey::try_new("T2030", campus).expect("target");
        let mut section = serde_json::json!({
            "campusCode":campus,
            "index":format!("8{ordinal:04}")
        });
        if let Some(meeting_times) = meeting_times {
            section
                .as_object_mut()
                .unwrap()
                .insert("meetingTimes".to_owned(), meeting_times);
        }
        let body = serde_json::to_vec(&serde_json::json!([{
            "campusCode":campus,
            "courseString":format!("SYN:OCC:{ordinal:03}"),
            "sections":[section]
        }]))
        .unwrap();
        let normalized = normalized_catalog(target.clone(), &body, "SYNTHETIC_OCCURRENCE", STARTED);
        storage
            .apply_catalog_refresh(
                to_catalog_refresh_command(
                    &normalized,
                    trace_id(40 + u8::try_from(ordinal).unwrap()),
                    STARTED,
                    COMPLETED,
                )
                .expect("command"),
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            )
            .expect("publish");
        let stored = storage
            .published_catalog_snapshot(&target)
            .expect("read")
            .expect("published");
        let projected = to_normalized_catalog_v1(&stored).expect("projection");
        assert_eq!(projected.sections[0].occurrence_keys, expected);
    }
}

#[test]
fn set_reordering_is_unchanged_and_matches_a_clean_rebuild_projection() {
    let target = TermCampusKey::try_new("T2030", "SYN_STABLE").expect("target");
    let baseline_body = catalog_body("SYN_STABLE", "SYN:STABLE:001", "71001", false);
    let mut reordered_json: serde_json::Value =
        serde_json::from_slice(&catalog_body("SYN_STABLE", "SYN:STABLE:001", "71001", true))
            .expect("reordered Catalog JSON");
    reordered_json[0]["sections"][0]["commentsText"] =
        serde_json::json!("DERIVED TEXT MUST NOT CONTROL PUBLIC OUTPUT");
    reordered_json[0]["sections"][0]["crossListedSectionsText"] =
        serde_json::json!("DERIVED CROSS LIST MUST NOT CONTROL PUBLIC OUTPUT");
    reordered_json[0]["sections"][0]["sectionCourseType"] = serde_json::json!("O");
    let reordered_body = serde_json::to_vec(&reordered_json).expect("reordered Catalog bytes");
    let baseline = normalized_catalog(target.clone(), &baseline_body, "BASELINE", STARTED);
    let reordered = normalized_catalog(
        target.clone(),
        &reordered_body,
        "REORDERED",
        "2030-01-02T00:00:00Z",
    );
    assert_eq!(baseline.content_hash, reordered.content_hash);

    let mut retained = OperationalStorage::open_in_memory().expect("retained storage");
    retained
        .apply_catalog_refresh(
            to_catalog_refresh_command(&baseline, trace_id(3), STARTED, COMPLETED)
                .expect("baseline command"),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
        )
        .expect("baseline publication");
    assert!(matches!(
        retained
            .apply_catalog_refresh(
                to_catalog_refresh_command(
                    &reordered,
                    trace_id(4),
                    "2030-01-02T00:00:00Z",
                    "2030-01-02T00:00:01Z",
                )
                .expect("reordered command"),
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            )
            .expect("unchanged publication"),
        PublishOutcome::AppliedUnchanged { .. }
    ));
    let retained = to_normalized_catalog_v1(
        &retained
            .published_catalog_snapshot(&target)
            .expect("retained reader")
            .expect("retained publication"),
    )
    .expect("retained projection");

    let mut rebuilt = OperationalStorage::open_in_memory().expect("clean storage");
    rebuilt
        .apply_catalog_refresh(
            to_catalog_refresh_command(
                &reordered,
                trace_id(5),
                "2030-01-02T00:00:00Z",
                "2030-01-02T00:00:01Z",
            )
            .expect("clean command"),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
        )
        .expect("clean publication");
    let rebuilt = to_normalized_catalog_v1(
        &rebuilt
            .published_catalog_snapshot(&target)
            .expect("clean reader")
            .expect("clean publication"),
    )
    .expect("clean projection");

    assert_eq!(retained.course_groups, rebuilt.course_groups);
    assert_eq!(retained.course_variants, rebuilt.course_variants);
    assert_eq!(retained.sections, rebuilt.sections);
    assert_eq!(retained.occurrences, rebuilt.occurrences);
}

#[test]
fn meeting_order_is_content_and_public_ordinals_remain_source_ordinals() {
    let target = TermCampusKey::try_new("T2030", "SYN_ORDER").expect("target");
    let baseline_body = catalog_body_with_meeting_order(false);
    let reversed_body = catalog_body_with_meeting_order(true);
    let baseline = normalized_catalog(target.clone(), &baseline_body, "ORDER_A", STARTED);
    let reversed = normalized_catalog(
        target.clone(),
        &reversed_body,
        "ORDER_B",
        "2030-01-02T00:00:00Z",
    );
    assert_ne!(baseline.content_hash, reversed.content_hash);

    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    storage
        .apply_catalog_refresh(
            to_catalog_refresh_command(&baseline, trace_id(6), STARTED, COMPLETED)
                .expect("baseline command"),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
        )
        .expect("baseline publication");
    assert!(matches!(
        storage
            .apply_catalog_refresh(
                to_catalog_refresh_command(
                    &reversed,
                    trace_id(7),
                    "2030-01-02T00:00:00Z",
                    "2030-01-02T00:00:01Z",
                )
                .expect("reversed command"),
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            )
            .expect("reversed publication"),
        PublishOutcome::AppliedChanged {
            content_version: 2,
            ..
        }
    ));
    let projected = to_normalized_catalog_v1(
        &storage
            .published_catalog_snapshot(&target)
            .expect("reader")
            .expect("publication"),
    )
    .expect("projection");
    assert_eq!(projected.occurrences[0].key.ordinal, 0);
    assert_eq!(
        projected.occurrences[0].raw_code,
        CatalogFieldKnowledge::present("93".to_owned())
    );
    assert_eq!(projected.occurrences[1].key.ordinal, 1);
    assert_eq!(
        projected.occurrences[1].raw_code,
        CatalogFieldKnowledge::present("92".to_owned())
    );
}

#[test]
fn malformed_course_and_meeting_facts_survive_storage_projection() {
    let target = TermCampusKey::try_new("T2030", "SYN_MALFORMED").expect("target");
    let mut body: serde_json::Value = serde_json::from_slice(&catalog_body(
        "SYN_MALFORMED",
        "SYN:MALFORMED:001",
        "74001",
        false,
    ))
    .expect("base Catalog JSON");
    let meeting = &mut body[0]["sections"][0]["meetingTimes"][0];
    meeting["meetingModeCode"] = serde_json::json!(7);
    meeting["meetingModeDesc"] = serde_json::json!(8);
    meeting["startTimeMilitary"] = serde_json::json!(900);
    meeting["buildingCode"] = serde_json::json!({"unexpected": true});
    let body = serde_json::to_vec(&body).expect("malformed-shape Catalog JSON");
    let normalized = normalized_catalog(target.clone(), &body, "MALFORMED", STARTED);
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    storage
        .apply_catalog_refresh(
            to_catalog_refresh_command(&normalized, trace_id(8), STARTED, COMPLETED)
                .expect("command"),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
        )
        .expect("publication");
    let projected = to_normalized_catalog_v1(
        &storage
            .published_catalog_snapshot(&target)
            .expect("reader")
            .expect("publication"),
    )
    .expect("projection");
    assert_eq!(
        projected.course_variants[0].prerequisite_notes,
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed)
    );
    let occurrence = &projected.occurrences[0];
    assert_eq!(
        occurrence.raw_code,
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed)
    );
    assert_eq!(
        occurrence.raw_description,
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed)
    );
    assert_eq!(occurrence.time, CatalogTimeKnowledgeV1::Invalid);
    assert_eq!(
        occurrence.building,
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed)
    );
}

#[test]
fn eligibility_projection_preserves_knowledge_states_and_validates_items() {
    let target = TermCampusKey::try_new("T2030", "SYN_ELIGIBILITY").expect("target");
    let mut body: serde_json::Value = serde_json::from_slice(&catalog_body(
        "SYN_ELIGIBILITY",
        "SYN:ELIGIBILITY:001",
        "74002",
        false,
    ))
    .expect("base Catalog JSON");
    let course = body[0].as_object_mut().expect("synthetic course object");
    course.remove("subjectGroupNotes");
    course.insert("subjectNotes".to_owned(), serde_json::Value::Null);
    course.insert("unitNotes".to_owned(), serde_json::json!([]));
    course.insert("synopsisUrl".to_owned(), serde_json::json!(""));
    let section = body[0]["sections"][0]
        .as_object_mut()
        .expect("synthetic section object");
    section.insert("subtopic".to_owned(), serde_json::json!(7));
    section.insert("sectionNotes".to_owned(), serde_json::Value::Null);
    section.remove("sessionDates");
    section.insert(
        "sessionDatePrintIndicator".to_owned(),
        serde_json::Value::Null,
    );
    section.insert("comments".to_owned(), serde_json::Value::Null);
    section.remove("commentsText");
    section.insert("crossListedSectionsText".to_owned(), serde_json::json!([]));
    section.insert("crossListedSectionType".to_owned(), serde_json::json!(""));
    section.remove("examCode");
    section.insert("examCodeText".to_owned(), serde_json::Value::Null);
    section.insert("specialPermissionAddCode".to_owned(), serde_json::json!([]));
    section.insert(
        "specialPermissionAddCodeDescription".to_owned(),
        serde_json::json!(""),
    );
    section.remove("specialPermissionDropCode");
    section.insert(
        "specialPermissionDropCodeDescription".to_owned(),
        serde_json::Value::Null,
    );
    section.insert("majors".to_owned(), serde_json::json!([]));
    section.insert("minors".to_owned(), serde_json::json!([{"code":" "}]));
    section.insert(
        "honorPrograms".to_owned(),
        serde_json::json!([{"code":" SYN_HONOR "},{"code":"SYN_HONOR"}]),
    );
    section.insert(
        "unitMajors".to_owned(),
        serde_json::json!([
            {"unitCode":" SYN_UNIT ","majorCode":""},
            {"unitCode":"SYN_UNIT","majorCode":""}
        ]),
    );
    section.remove("sectionEligibility");
    section.insert("openToText".to_owned(), serde_json::Value::Null);
    section.insert(
        "finalExam".to_owned(),
        serde_json::json!({"futureShape":"SYNTHETIC"}),
    );

    let body = serde_json::to_vec(&body).expect("knowledge-state Catalog JSON");
    let normalized = normalized_catalog(target.clone(), &body, "ELIGIBILITY", STARTED);
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    storage
        .apply_catalog_refresh(
            to_catalog_refresh_command(&normalized, trace_id(9), STARTED, COMPLETED)
                .expect("command"),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
        )
        .expect("publication");
    let projected = to_normalized_catalog_v1(
        &storage
            .published_catalog_snapshot(&target)
            .expect("reader")
            .expect("publication"),
    )
    .expect("projection");
    let variant = &projected.course_variants[0];
    assert_eq!(variant.subject_group_notes, CatalogFieldKnowledge::absent());
    assert_eq!(
        variant.subject_notes,
        CatalogFieldKnowledge::explicit_null()
    );
    assert_eq!(
        variant.unit_notes,
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed)
    );
    assert_eq!(
        variant.synopsis_url,
        CatalogFieldKnowledge::present(String::new())
    );
    assert_eq!(
        variant.offering_unit_title,
        CatalogFieldKnowledge::present("Synthetic Unit Title".to_owned())
    );
    let section = &projected.sections[0];
    assert_eq!(
        section.subtopic,
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed)
    );
    assert_eq!(
        section.section_notes,
        CatalogFieldKnowledge::explicit_null()
    );
    assert_eq!(section.session_dates, CatalogFieldKnowledge::absent());
    assert_eq!(
        section.session_date_print_indicator,
        CatalogFieldKnowledge::explicit_null()
    );
    assert_eq!(section.comments, CatalogFieldKnowledge::explicit_null());
    assert_eq!(section.comments_text, CatalogFieldKnowledge::absent());
    assert_eq!(
        section.cross_listed_sections_text,
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed)
    );
    assert_eq!(
        section.cross_listed_section_type,
        CatalogFieldKnowledge::present(String::new())
    );
    assert_eq!(section.exam_code, CatalogFieldKnowledge::absent());
    assert_eq!(
        section.exam_code_text,
        CatalogFieldKnowledge::explicit_null()
    );
    assert_eq!(
        section.special_permission_add_code,
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed)
    );
    assert_eq!(
        section.special_permission_add_description,
        CatalogFieldKnowledge::present(String::new())
    );
    assert_eq!(
        section.special_permission_drop_code,
        CatalogFieldKnowledge::absent()
    );
    assert_eq!(
        section.special_permission_drop_description,
        CatalogFieldKnowledge::explicit_null()
    );
    assert_eq!(
        section.major_codes,
        CatalogFieldKnowledge::present(Vec::new())
    );
    assert_eq!(
        section.unit_codes,
        CatalogFieldKnowledge::present(Vec::new())
    );
    assert_eq!(
        section.minor_codes,
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed)
    );
    assert_eq!(
        section.honor_program_codes,
        CatalogFieldKnowledge::present(vec!["SYN_HONOR".to_owned()])
    );
    assert_eq!(
        section.unit_majors,
        CatalogFieldKnowledge::present(vec![CatalogUnitMajorV1 {
            unit_code: "SYN_UNIT".to_owned(),
            major_code: String::new(),
        }])
    );
    assert_eq!(section.eligibility_text, CatalogFieldKnowledge::absent());
    assert_eq!(section.open_to_text, CatalogFieldKnowledge::explicit_null());
}

#[test]
fn discovery_restart_uses_stable_source_ids_and_fail_closed_selectability() {
    let directory = TempDir::new().expect("temporary storage directory");
    let database = directory.path().join("discovery.sqlite3");
    let first = discovery_bundle(STARTED);
    let later = discovery_bundle("2030-01-02T00:00:00Z");
    let mut storage = OperationalStorage::open(&database).expect("file-backed storage");
    assert_eq!(
        storage
            .apply_discovery_refresh(
                to_discovery_refresh_command(&first, trace_id(10), STARTED, COMPLETED)
                    .expect("first discovery command"),
            )
            .expect("first discovery publication"),
        DiscoveryPublishOutcome::AppliedChanged { content_version: 1 }
    );
    assert_eq!(
        storage
            .apply_discovery_refresh(
                to_discovery_refresh_command(
                    &later,
                    trace_id(11),
                    "2030-01-02T00:00:00Z",
                    "2030-01-02T00:00:01Z",
                )
                .expect("unchanged discovery command"),
            )
            .expect("unchanged discovery publication"),
        DiscoveryPublishOutcome::AppliedUnchanged { content_version: 1 }
    );
    drop(storage);

    let mut storage = OperationalStorage::open(&database).expect("reopened storage");
    let published = storage
        .published_discovery_snapshot()
        .expect("published discovery reader");
    let projected = to_catalog_discovery_response_v1(&published, timestamp("2030-01-02T00:00:02Z"))
        .expect("strict discovery projection");

    let subject_source_version = &published.snapshot.subjects[0].source_version_id;
    let conflicting_source_version = published
        .sources
        .iter()
        .find(|source| &source.source_version_id != subject_source_version)
        .expect("the synthetic bundle has a second source")
        .source_version_id
        .clone();
    let mut corrupted = published.clone();
    corrupted.snapshot.subjects[0].source_version_id = conflicting_source_version;
    assert!(
        to_catalog_discovery_response_v1(&corrupted, timestamp("2030-01-02T00:00:02Z")).is_err(),
        "a restarted snapshot must reject cross-source subject attribution"
    );

    assert_eq!(
        projected.status.availability,
        CatalogDiscoveryAvailability::Current
    );
    assert_eq!(
        projected
            .status
            .latest_attempt
            .as_ref()
            .expect("latest attempt")
            .observation_id,
        trace_id(11)
    );
    assert_eq!(projected.sources.len(), 2);
    let selector = projected
        .sources
        .iter()
        .find(|source| source.source_kind == CatalogDiscoverySourceKind::Selector)
        .expect("selector source");
    let bootstrap = projected
        .sources
        .iter()
        .find(|source| source.source_kind == CatalogDiscoverySourceKind::Bootstrap)
        .expect("bootstrap source");
    assert_eq!(selector.source_id.as_str(), "RUTGERS_SELECTOR");
    assert_eq!(bootstrap.source_id.as_str(), "RBCSP_BOOTSTRAP");
    assert!(!selector.source_id.as_str().starts_with("selector:"));
    assert_eq!(
        selector.source_version,
        CatalogFieldKnowledge::explicit_null()
    );
    assert_eq!(bootstrap.source_version, CatalogFieldKnowledge::absent());

    assert_eq!(projected.targets.len(), 2);
    assert!(
        projected
            .targets
            .iter()
            .all(|target| target.key.campus().as_str() != "SYN_OFF")
    );
    let selector_target = projected
        .targets
        .iter()
        .find(|target| target.key.campus().as_str() == "SYN_SELECTOR")
        .expect("selector target");
    assert_eq!(selector_target.term_label, CatalogFieldKnowledge::absent());
    assert_eq!(
        selector_target.campus_label,
        CatalogFieldKnowledge::explicit_null()
    );
    assert_eq!(
        selector_target.provenance.source_id.as_str(),
        "RUTGERS_SELECTOR"
    );
    assert_eq!(selector_target.provenance.observation_id, trace_id(11));
    assert_eq!(projected.subjects.len(), 1);
    assert_eq!(
        projected.subjects[0].label,
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed)
    );
    assert_eq!(
        projected.subjects[0].provenance.source_id.as_str(),
        "RUTGERS_SELECTOR"
    );

    storage
        .begin_discovery_attempt(&BeginDiscoveryAttemptCommand {
            observation_id: trace_id(12),
            started_at: "2030-01-03T00:00:00Z".to_owned(),
        })
        .expect("failed attempt start");
    storage
        .finish_discovery_failure(&FinishDiscoveryFailureCommand {
            observation_id: trace_id(12),
            completed_at: "2030-01-03T00:00:01Z".to_owned(),
            stage: RefreshFailureStage::Transport,
            error_code: "UPSTREAM_UNAVAILABLE".to_owned(),
            diagnostic_token: None,
        })
        .expect("failed attempt finish");
    let stale = to_catalog_discovery_response_v1(
        &storage
            .published_discovery_snapshot()
            .expect("stale discovery reader"),
        timestamp("2030-01-03T00:00:02Z"),
    )
    .expect("stale projection");
    assert_eq!(
        stale.status.availability,
        CatalogDiscoveryAvailability::StaleLastSuccess
    );
    assert!(stale.status.is_stale);
    assert_eq!(stale.targets.len(), 2);
    assert!(stale.status.error.is_some());

    let encoded = serde_json::to_string(&projected).expect("public discovery JSON");
    let decoded = serde_json::from_str(&encoded).expect("public discovery round trip");
    assert_eq!(projected, decoded);
}

#[test]
fn discovery_no_first_success_has_no_fabricated_fallback() {
    let mut storage = OperationalStorage::open_in_memory().expect("in-memory storage");
    storage
        .begin_discovery_attempt(&BeginDiscoveryAttemptCommand {
            observation_id: trace_id(20),
            started_at: STARTED.to_owned(),
        })
        .expect("first attempt start");
    storage
        .finish_discovery_failure(&FinishDiscoveryFailureCommand {
            observation_id: trace_id(20),
            completed_at: COMPLETED.to_owned(),
            stage: RefreshFailureStage::Schema,
            error_code: "UPSTREAM_SCHEMA_INVALID".to_owned(),
            diagnostic_token: None,
        })
        .expect("first attempt failure");
    let projected = to_catalog_discovery_response_v1(
        &storage
            .published_discovery_snapshot()
            .expect("empty discovery reader"),
        timestamp("2030-01-01T00:00:02Z"),
    )
    .expect("no-first-success projection");
    assert_eq!(
        projected.status.availability,
        CatalogDiscoveryAvailability::UnavailableNoFirstSuccess
    );
    assert!(projected.sources.is_empty());
    assert!(projected.targets.is_empty());
    assert!(projected.subjects.is_empty());
    assert!(projected.status.last_success.is_none());
}

#[test]
fn refresh_pending_chain_failure_mapping_and_incomplete_rules_are_strict() {
    let target = TermCampusKey::try_new("T2030", "SYN_REFRESH").expect("target");
    let body = catalog_body("SYN_REFRESH", "SYN:REFRESH:001", "72001", false);
    let normalized = normalized_catalog(target.clone(), &body, "NONEMPTY", STARTED);
    let empty = normalized_catalog(target.clone(), b"[]", "EMPTY", "2030-01-02T00:00:00Z");
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    storage
        .apply_catalog_refresh(
            to_catalog_refresh_command(&normalized, trace_id(30), STARTED, COMPLETED)
                .expect("nonempty command"),
            EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
        )
        .expect("nonempty publication");
    assert!(matches!(
        storage
            .apply_catalog_refresh(
                to_catalog_refresh_command(
                    &empty,
                    trace_id(31),
                    "2030-01-02T00:00:00Z",
                    "2030-01-02T00:00:01Z",
                )
                .expect("empty command"),
                EmptySnapshotDecision::RetainLastKnownGood,
            )
            .expect("suspect empty result"),
        PublishOutcome::SuspectEmptyRetained { .. }
    ));
    let state = storage
        .target_state(&target)
        .expect("target state")
        .expect("target row");
    let observation = to_catalog_refresh_observation_v1(
        state
            .pending_empty
            .as_ref()
            .expect("pending empty observation"),
    )
    .expect("pending observation projection");
    assert_eq!(
        observation.classification,
        CatalogRefreshClassification::EmptySuspect
    );
    assert_eq!(
        observation.partial_reasons,
        vec![CatalogUnknownReason::SparseEvidence]
    );
    let checkpoint = to_catalog_refresh_checkpoint_v1(&state).expect("checkpoint projection");
    let status = to_catalog_refresh_status_v1(&state).expect("status projection");
    assert_eq!(
        checkpoint
            .pending_empty
            .as_ref()
            .expect("checkpoint pending")
            .observation_id,
        trace_id(31)
    );
    assert_eq!(
        status
            .last_published
            .as_ref()
            .expect("published point")
            .observation_id,
        trace_id(30)
    );

    for (stage, expected) in [
        ("TRANSPORT", CatalogRefreshErrorClass::Transport),
        ("SCHEMA", CatalogRefreshErrorClass::Decode),
        ("NORMALIZATION", CatalogRefreshErrorClass::Normalize),
        ("PUBLISH", CatalogRefreshErrorClass::Persist),
    ] {
        let failed = RefreshObservation {
            observation_id: trace_id(40),
            target: target.clone(),
            attempt_sequence: 40,
            started_at: STARTED.to_owned(),
            completed_at: Some(COMPLETED.to_owned()),
            status: RefreshStatus::Failed,
            source_content_sha256: None,
            semantic_sha256: None,
            source_bytes: None,
            counts: CatalogCounts::default(),
            changed: None,
            resulting_content_version: None,
            retained_observation_id: None,
            error_stage: Some(stage.to_owned()),
            error_code: Some("SYNTHETIC_FAILURE".to_owned()),
            diagnostic_token: None,
        };
        assert_eq!(
            to_catalog_refresh_observation_v1(&failed)
                .expect("failed observation projection")
                .error_class,
            Some(expected)
        );
    }

    for status in [RefreshStatus::Started, RefreshStatus::Staged] {
        let incomplete = RefreshObservation {
            observation_id: trace_id(41),
            target: target.clone(),
            attempt_sequence: 41,
            started_at: STARTED.to_owned(),
            completed_at: None,
            status,
            source_content_sha256: None,
            semantic_sha256: None,
            source_bytes: None,
            counts: CatalogCounts::default(),
            changed: None,
            resulting_content_version: None,
            retained_observation_id: None,
            error_stage: None,
            error_code: None,
            diagnostic_token: None,
        };
        assert!(to_catalog_refresh_observation_v1(&incomplete).is_err());
    }

    let started_target = TermCampusKey::try_new("T2032", "SYN_STARTED").expect("started target");
    storage
        .begin_refresh_attempt(&BeginRefreshAttemptCommand {
            observation_id: trace_id(42),
            target: started_target.clone(),
            started_at: STARTED.to_owned(),
            source_content_sha256: None,
            source_bytes: None,
        })
        .expect("started target attempt");
    let started_state = storage
        .target_state(&started_target)
        .expect("started target state")
        .expect("started target row");
    assert_eq!(
        to_catalog_refresh_status_v1(&started_state)
            .expect("started point projection")
            .latest_attempt
            .classification,
        CatalogRefreshClassification::Started
    );
}
