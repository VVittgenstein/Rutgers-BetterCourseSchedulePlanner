use std::str::FromStr;

use bcsp_contracts::{
    CATALOG_CONTRACT_VERSION, CatalogCommentV1, CatalogContentVersion, CatalogContractVersion,
    CatalogDiagnosticCode, CatalogDiscoveryAvailability, CatalogDiscoveryErrorClass,
    CatalogDiscoveryErrorV1, CatalogDiscoveryPointV1, CatalogDiscoveryProvenanceV1,
    CatalogDiscoveryRequestV1, CatalogDiscoveryResponseV1, CatalogDiscoverySourceId,
    CatalogDiscoverySourceKind, CatalogDiscoverySourceV1, CatalogDiscoveryStatusV1,
    CatalogEntityCountsV1, CatalogFieldKnowledge, CatalogInstructorReliability, CatalogModality,
    CatalogOccurrenceEvidence, CatalogOccurrenceKeyV1, CatalogOccurrenceKind,
    CatalogOpenStatusProvenance, CatalogPayloadDigest, CatalogPrerequisiteState,
    CatalogProvenanceV1, CatalogRefreshCheckpointPointV1, CatalogRefreshCheckpointV1,
    CatalogRefreshClassification, CatalogRefreshErrorClass, CatalogRefreshObservationV1,
    CatalogRefreshPointV1, CatalogRefreshStatusV1, CatalogRequiredness,
    CatalogSnapshotOpenStatusV1, CatalogSourceKind, CatalogSubjectCode, CatalogSubjectV1,
    CatalogSynchronicity, CatalogTargetV1, CatalogTimeKnowledgeV1, CatalogUnitMajorV1,
    CatalogUnknownReason, CourseGroupKey, CourseVariantKey, NormalizedCatalogV1,
    NormalizedCourseGroupV1, NormalizedCourseVariantV1, NormalizedOccurrenceV1,
    NormalizedSectionV1, SectionKey, TermCampusKey, TraceId,
};
use serde_json::json;
use time::OffsetDateTime;

fn timestamp() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(0).unwrap()
}

fn trace_id(suffix: u8) -> TraceId {
    TraceId::from_str(&format!("00000000-0000-4000-8000-{suffix:012x}")).unwrap()
}

fn target() -> TermCampusKey {
    TermCampusKey::try_new("T2026F", "CAMPUS_A").unwrap()
}

fn section_key() -> SectionKey {
    SectionKey::try_new("T2026F", "CAMPUS_A", "00001").unwrap()
}

fn group_key() -> CourseGroupKey {
    CourseGroupKey::try_new("T2026F", "CAMPUS_A", "00:000:001").unwrap()
}

fn variant_key() -> CourseVariantKey {
    CourseVariantKey::try_new(group_key(), &format!("v1:{}", "a".repeat(64))).unwrap()
}

fn digest() -> CatalogPayloadDigest {
    CatalogPayloadDigest::try_from("a".repeat(64)).unwrap()
}

fn provenance() -> CatalogProvenanceV1 {
    CatalogProvenanceV1 {
        observation_id: trace_id(1),
        source: CatalogSourceKind::RutgersCatalog,
        target: target(),
        observed_at: timestamp(),
        payload_digest: digest(),
    }
}

fn discovery_provenance() -> CatalogDiscoveryProvenanceV1 {
    CatalogDiscoveryProvenanceV1 {
        observation_id: trace_id(1),
        source_id: CatalogDiscoverySourceId::try_from("SYNTHETIC_SELECTOR").unwrap(),
        source_kind: CatalogDiscoverySourceKind::Selector,
        observed_at: timestamp(),
        payload_digest: digest(),
    }
}

fn discovery_point() -> CatalogDiscoveryPointV1 {
    CatalogDiscoveryPointV1 {
        observation_id: trace_id(1),
        observed_at: timestamp(),
        content_version: Some(CatalogContentVersion::try_from(1).unwrap()),
    }
}

fn discovery_status() -> CatalogDiscoveryStatusV1 {
    CatalogDiscoveryStatusV1 {
        availability: CatalogDiscoveryAvailability::Current,
        latest_attempt: Some(discovery_point()),
        last_success: Some(discovery_point()),
        is_stale: false,
        error: None,
    }
}

fn discovery_source() -> CatalogDiscoverySourceV1 {
    CatalogDiscoverySourceV1 {
        source_id: CatalogDiscoverySourceId::try_from("SYNTHETIC_SELECTOR").unwrap(),
        source_kind: CatalogDiscoverySourceKind::Selector,
        source_version: CatalogFieldKnowledge::present("v1".to_owned()),
        payload_digest: digest(),
        observed_at: timestamp(),
    }
}

fn refresh_point(classification: CatalogRefreshClassification) -> CatalogRefreshPointV1 {
    CatalogRefreshPointV1 {
        observation_id: trace_id(1),
        observed_at: timestamp(),
        classification,
        content_version: Some(CatalogContentVersion::try_from(1).unwrap()),
    }
}

fn checkpoint_point(
    observation_suffix: u8,
    classification: CatalogRefreshClassification,
) -> CatalogRefreshCheckpointPointV1 {
    CatalogRefreshCheckpointPointV1 {
        observation_id: trace_id(observation_suffix),
        observed_at: timestamp(),
        classification,
        content_version: Some(
            CatalogContentVersion::try_from(u64::from(observation_suffix)).unwrap(),
        ),
    }
}

fn normalized_catalog() -> NormalizedCatalogV1 {
    let occurrence_key = CatalogOccurrenceKeyV1 {
        section: section_key(),
        ordinal: 0,
    };
    NormalizedCatalogV1 {
        contract_version: CATALOG_CONTRACT_VERSION,
        target: target(),
        content_version: CatalogContentVersion::try_from(1).unwrap(),
        provenance: provenance(),
        course_groups: vec![NormalizedCourseGroupV1 {
            key: group_key(),
            variant_keys: vec![variant_key()],
        }],
        course_variants: vec![NormalizedCourseVariantV1 {
            key: variant_key(),
            title: CatalogFieldKnowledge::present("Synthetic variant".to_owned()),
            expanded_title: CatalogFieldKnowledge::absent(),
            description: CatalogFieldKnowledge::unknown(CatalogUnknownReason::ExplicitNull),
            notes: CatalogFieldKnowledge::absent(),
            subject_group_notes: CatalogFieldKnowledge::absent(),
            subject_notes: CatalogFieldKnowledge::absent(),
            unit_notes: CatalogFieldKnowledge::absent(),
            synopsis_url: CatalogFieldKnowledge::absent(),
            prerequisite_notes: CatalogFieldKnowledge::unknown(CatalogUnknownReason::NotObserved),
            prerequisite_state: CatalogPrerequisiteState::Unknown,
            credits: CatalogFieldKnowledge::unknown(CatalogUnknownReason::SparseEvidence),
            level: CatalogFieldKnowledge::present("SYNTHETIC_LEVEL".to_owned()),
            subject_code: CatalogFieldKnowledge::present("SYN_SUBJECT".to_owned()),
            subject_description: CatalogFieldKnowledge::present("Synthetic subject".to_owned()),
            course_number: CatalogFieldKnowledge::present("SYN_COURSE".to_owned()),
            supplement_code: CatalogFieldKnowledge::absent(),
            school_code: CatalogFieldKnowledge::absent(),
            offering_unit: CatalogFieldKnowledge::explicit_null(),
            offering_unit_title: CatalogFieldKnowledge::absent(),
            core_codes: CatalogFieldKnowledge::present(vec!["SYN_CORE".to_owned()]),
            campus_locations: CatalogFieldKnowledge::present(vec!["CAMPUS_A".to_owned()]),
            section_keys: vec![section_key()],
        }],
        sections: vec![NormalizedSectionV1 {
            key: section_key(),
            variant_key: variant_key(),
            section_number: CatalogFieldKnowledge::present("01".to_owned()),
            subtitle: CatalogFieldKnowledge::absent(),
            subtopic: CatalogFieldKnowledge::absent(),
            section_notes: CatalogFieldKnowledge::absent(),
            session_dates: CatalogFieldKnowledge::absent(),
            session_date_print_indicator: CatalogFieldKnowledge::absent(),
            comments: CatalogFieldKnowledge::present(vec![CatalogCommentV1 {
                code: CatalogFieldKnowledge::present("SYN_COMMENT".to_owned()),
                description: CatalogFieldKnowledge::present("Synthetic comment".to_owned()),
            }]),
            comments_text: CatalogFieldKnowledge::present("Synthetic comment".to_owned()),
            cross_listed_sections_text: CatalogFieldKnowledge::absent(),
            cross_listed_section_type: CatalogFieldKnowledge::absent(),
            instructors: CatalogFieldKnowledge::present(vec!["Synthetic Instructor".to_owned()]),
            instructor_reliability: CatalogInstructorReliability::NameOnly,
            raw_section_course_type: CatalogFieldKnowledge::present("T".to_owned()),
            delivery_modality: CatalogModality::OnCampusOrInPerson,
            synchronicity: CatalogSynchronicity::Sync,
            exam_code: CatalogFieldKnowledge::present("SYN_EXAM".to_owned()),
            exam_code_text: CatalogFieldKnowledge::present("Synthetic exam".to_owned()),
            special_permission_add_code: CatalogFieldKnowledge::explicit_null(),
            special_permission_add_description: CatalogFieldKnowledge::explicit_null(),
            special_permission_drop_code: CatalogFieldKnowledge::explicit_null(),
            special_permission_drop_description: CatalogFieldKnowledge::explicit_null(),
            major_codes: CatalogFieldKnowledge::present(Vec::new()),
            unit_codes: CatalogFieldKnowledge::present(Vec::new()),
            minor_codes: CatalogFieldKnowledge::present(Vec::new()),
            honor_program_codes: CatalogFieldKnowledge::present(Vec::new()),
            unit_majors: CatalogFieldKnowledge::present(vec![CatalogUnitMajorV1 {
                unit_code: "SYN_UNIT".to_owned(),
                major_code: String::new(),
            }]),
            eligibility_text: CatalogFieldKnowledge::present(String::new()),
            open_to_text: CatalogFieldKnowledge::present(String::new()),
            catalog_open_status: CatalogSnapshotOpenStatusV1 {
                value: CatalogFieldKnowledge::present(true),
                provenance: CatalogOpenStatusProvenance::CatalogSnapshotOnly,
            },
            occurrence_keys: CatalogFieldKnowledge::present(vec![occurrence_key.clone()]),
        }],
        occurrences: vec![NormalizedOccurrenceV1 {
            key: occurrence_key,
            raw_code: CatalogFieldKnowledge::present("SYN".to_owned()),
            raw_description: CatalogFieldKnowledge::present("Synthetic meeting".to_owned()),
            modality: CatalogFieldKnowledge::present(CatalogModality::OnCampusOrInPerson),
            synchronicity: CatalogFieldKnowledge::present(CatalogSynchronicity::Sync),
            raw_day: CatalogFieldKnowledge::present("M".to_owned()),
            days: CatalogFieldKnowledge::present(vec!["M".to_owned()]),
            raw_start_time: CatalogFieldKnowledge::present("0900".to_owned()),
            raw_end_time: CatalogFieldKnowledge::present("1020".to_owned()),
            time: CatalogTimeKnowledgeV1::Known {
                start_minute: 540,
                end_minute: 620,
            },
            start_date: CatalogFieldKnowledge::unknown(CatalogUnknownReason::NotObserved),
            end_date: CatalogFieldKnowledge::unknown(CatalogUnknownReason::NotObserved),
            campus: CatalogFieldKnowledge::present("CAMPUS_A".to_owned()),
            campus_name: CatalogFieldKnowledge::present("Synthetic campus".to_owned()),
            building: CatalogFieldKnowledge::unknown(CatalogUnknownReason::Missing),
            room: CatalogFieldKnowledge::unknown(CatalogUnknownReason::ExplicitNull),
            requiredness: CatalogRequiredness::UnknownRequiredness,
            kind: CatalogOccurrenceKind::Scheduled,
            evidence: CatalogOccurrenceEvidence::Physical,
            normalization_reason: CatalogDiagnosticCode::try_from("SYNTHETIC_PRIMARY").unwrap(),
        }],
    }
}

#[test]
fn scalar_contracts_reject_ambiguous_or_noncanonical_values() {
    assert!(CatalogContractVersion::try_from(0).is_err());
    assert!(CatalogContractVersion::try_from(2).is_err());
    assert!(CatalogContentVersion::try_from(0).is_err());
    assert!(serde_json::from_str::<CatalogContentVersion>("0").is_err());

    let first = CatalogContentVersion::try_from(1).unwrap();
    let second = CatalogContentVersion::try_from(2).unwrap();
    assert!(first < second);

    assert!(CatalogPayloadDigest::try_from("A".repeat(64)).is_err());
    assert!(CatalogPayloadDigest::try_from("a".repeat(63)).is_err());
    assert!(CatalogPayloadDigest::try_from("a".repeat(65)).is_err());
    assert_eq!(digest().as_str(), "a".repeat(64));

    assert!(CatalogSubjectCode::try_from("").is_err());
    assert!(CatalogSubjectCode::try_from(" 198").is_err());
    assert!(CatalogSubjectCode::try_from("198\n").is_err());
    assert_eq!(
        CatalogSubjectCode::try_from("SYN:SUBJECT")
            .unwrap()
            .as_str(),
        "SYN:SUBJECT"
    );
    assert_eq!(
        CatalogSubjectCode::try_from("SYN SUBJECT")
            .unwrap()
            .as_str(),
        "SYN SUBJECT"
    );

    assert!(CatalogDiscoverySourceId::try_from("synthetic-selector").is_err());
    assert!(CatalogDiscoverySourceId::try_from("SYNTHETIC/SELECTOR").is_err());
    assert!(CatalogDiagnosticCode::try_from("parser detail").is_err());
    assert!(CatalogDiagnosticCode::try_from("UPSTREAM_ERROR\nforged").is_err());
    assert!(CatalogDiagnosticCode::try_from("A".repeat(65)).is_err());
}

#[test]
fn knowledge_and_presence_preserve_missing_null_malformed_sparse_and_not_observed() {
    let cases = [
        CatalogFieldKnowledge::<String>::unknown(CatalogUnknownReason::Missing),
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::ExplicitNull),
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed),
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::SparseEvidence),
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::NotObserved),
        CatalogFieldKnowledge::explicit_null(),
        CatalogFieldKnowledge::absent(),
        CatalogFieldKnowledge::present(String::new()),
    ];

    for value in cases {
        let encoded = serde_json::to_string(&value).unwrap();
        let decoded: CatalogFieldKnowledge<String> = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, value, "{encoded}");
    }
}

#[test]
fn discovery_request_is_strict_and_response_is_additive() {
    let request = serde_json::to_string(&CatalogDiscoveryRequestV1::new()).unwrap();
    assert_eq!(request, r#"{"contractVersion":1}"#);
    assert!(
        serde_json::from_str::<CatalogDiscoveryRequestV1>(
            r#"{"contractVersion":1,"futureRequestField":true}"#,
        )
        .is_err()
    );

    let response = json!({
        "contractVersion": 1,
        "observedAt": "1970-01-01T00:00:00Z",
        "status": {
            "availability": "STALE_LAST_SUCCESS",
            "latestAttempt": {
                "observationId": "00000000-0000-4000-8000-000000000002",
                "observedAt": "1970-01-01T00:00:00Z",
                "contentVersion": null
            },
            "lastSuccess": {
                "observationId": "00000000-0000-4000-8000-000000000001",
                "observedAt": "1970-01-01T00:00:00Z",
                "contentVersion": 1
            },
            "isStale": true,
            "error": {"class": "TRANSPORT", "code": "UPSTREAM_UNAVAILABLE"},
            "futureStatusField": true
        },
        "sources": [{
            "sourceId": "SYNTHETIC_SELECTOR",
            "sourceKind": "SELECTOR",
            "sourceVersion": {"knowledge": "KNOWN", "presence": {"presence": "PRESENT", "value": "v1"}},
            "payloadDigest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "observedAt": "1970-01-01T00:00:00Z",
            "futureSourceField": true
        }],
        "targets": [{
            "key": {"term": "T2026F", "campus": "CAMPUS_A"},
            "termLabel": {"knowledge": "KNOWN", "presence": {"presence": "PRESENT", "value": "Fall"}},
            "campusLabel": {"knowledge": "UNKNOWN", "reason": "NOT_OBSERVED"},
            "provenance": {
                "observationId": "00000000-0000-4000-8000-000000000001",
                "sourceId": "SYNTHETIC_SELECTOR",
                "sourceKind": "SELECTOR",
                "observedAt": "1970-01-01T00:00:00Z",
                "payloadDigest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            },
            "futureTargetField": true
        }],
        "subjects": [{
            "target": {"term": "T2026F", "campus": "CAMPUS_A"},
            "code": "SYN:SUBJECT",
            "label": {"knowledge": "KNOWN", "presence": {"presence": "PRESENT", "value": "Synthetic subject"}},
            "provenance": {
                "observationId": "00000000-0000-4000-8000-000000000001",
                "sourceId": "SYNTHETIC_SELECTOR",
                "sourceKind": "SELECTOR",
                "observedAt": "1970-01-01T00:00:00Z",
                "payloadDigest": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "futureProvenanceField": true
            },
            "futureSubjectField": true
        }],
        "futureResponseField": true
    });
    let decoded: CatalogDiscoveryResponseV1 = serde_json::from_value(response).unwrap();
    assert_eq!(decoded.targets[0].key, target());
    assert_eq!(
        decoded.status.availability,
        CatalogDiscoveryAvailability::StaleLastSuccess
    );
    assert!(decoded.status.is_stale);
    assert_eq!(decoded.subjects[0].code.as_str(), "SYN:SUBJECT");
    assert_eq!(
        decoded.subjects[0].provenance.source_kind,
        CatalogDiscoverySourceKind::Selector
    );
}

#[test]
fn normalized_projection_is_additive_and_catalog_open_status_is_snapshot_only() {
    let expected = normalized_catalog();
    let mut value = serde_json::to_value(&expected).unwrap();
    value["futureCatalogField"] = json!(true);
    value["sections"][0]["futureSectionField"] = json!(true);
    value["occurrences"][0]["futureOccurrenceField"] = json!(true);

    let decoded: NormalizedCatalogV1 = serde_json::from_value(value).unwrap();
    assert_eq!(decoded, expected);
    assert_eq!(
        decoded.sections[0].catalog_open_status.provenance,
        CatalogOpenStatusProvenance::CatalogSnapshotOnly
    );
}

#[test]
fn checkpoint_is_strict_while_status_projection_is_additive() {
    let checkpoint = CatalogRefreshCheckpointV1 {
        contract_version: CATALOG_CONTRACT_VERSION,
        target: target(),
        latest_attempt: checkpoint_point(4, CatalogRefreshClassification::EmptySuspect),
        last_success: Some(checkpoint_point(3, CatalogRefreshClassification::Partial)),
        last_published: Some(checkpoint_point(
            2,
            CatalogRefreshClassification::EmptyValidInitial,
        )),
        last_nonempty: Some(checkpoint_point(
            1,
            CatalogRefreshClassification::AppliedChanged,
        )),
        pending_empty: Some(checkpoint_point(
            4,
            CatalogRefreshClassification::EmptySuspect,
        )),
    };
    let mut outer_unknown = serde_json::to_value(&checkpoint).unwrap();
    outer_unknown["futureCheckpointField"] = json!(true);
    assert!(serde_json::from_value::<CatalogRefreshCheckpointV1>(outer_unknown).is_err());

    let mut nested_unknown = serde_json::to_value(&checkpoint).unwrap();
    nested_unknown["latestAttempt"]["futureMarkerField"] = json!(true);
    assert!(serde_json::from_value::<CatalogRefreshCheckpointV1>(nested_unknown).is_err());

    let status = CatalogRefreshStatusV1 {
        contract_version: CATALOG_CONTRACT_VERSION,
        target: target(),
        latest_attempt: refresh_point(CatalogRefreshClassification::EmptySuspect),
        last_success: Some(refresh_point(CatalogRefreshClassification::Partial)),
        last_published: Some(refresh_point(
            CatalogRefreshClassification::EmptyValidInitial,
        )),
        last_nonempty: Some(refresh_point(CatalogRefreshClassification::AppliedChanged)),
        pending_empty: Some(refresh_point(CatalogRefreshClassification::EmptySuspect)),
    };
    let mut additive = serde_json::to_value(&status).unwrap();
    additive["futureStatusField"] = json!(true);
    additive["latestAttempt"]["futureMarkerField"] = json!(true);
    let decoded: CatalogRefreshStatusV1 = serde_json::from_value(additive).unwrap();
    assert_eq!(decoded, status);

    let keys = serde_json::to_value(&status)
        .unwrap()
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        keys,
        [
            "contractVersion",
            "lastNonempty",
            "lastPublished",
            "lastSuccess",
            "latestAttempt",
            "pendingEmpty",
            "target",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect()
    );
}

#[test]
fn known_catalog_time_rejects_reversed_or_out_of_day_ranges() {
    for value in [
        json!({"knowledge":"KNOWN","startMinute":600,"endMinute":600}),
        json!({"knowledge":"KNOWN","startMinute":700,"endMinute":600}),
        json!({"knowledge":"KNOWN","startMinute":1440,"endMinute":1441}),
    ] {
        assert!(serde_json::from_value::<CatalogTimeKnowledgeV1>(value).is_err());
    }
    for value in [
        CatalogTimeKnowledgeV1::Known {
            start_minute: 600,
            end_minute: 600,
        },
        CatalogTimeKnowledgeV1::Known {
            start_minute: 700,
            end_minute: 600,
        },
        CatalogTimeKnowledgeV1::Known {
            start_minute: 1_440,
            end_minute: 1_441,
        },
        CatalogTimeKnowledgeV1::Known {
            start_minute: 1_439,
            end_minute: 1_441,
        },
    ] {
        assert!(serde_json::to_value(value).is_err());
    }
    assert_eq!(
        serde_json::from_value::<CatalogTimeKnowledgeV1>(
            json!({"knowledge":"KNOWN","startMinute":600,"endMinute":660})
        )
        .unwrap(),
        CatalogTimeKnowledgeV1::Known {
            start_minute: 600,
            end_minute: 660,
        }
    );
    let whole_day = CatalogTimeKnowledgeV1::Known {
        start_minute: 0,
        end_minute: 1_440,
    };
    let whole_day_wire = serde_json::to_value(whole_day).unwrap();
    assert_eq!(
        whole_day_wire,
        json!({"knowledge":"KNOWN","startMinute":0,"endMinute":1440})
    );
    assert_eq!(
        serde_json::from_value::<CatalogTimeKnowledgeV1>(whole_day_wire).unwrap(),
        whole_day
    );
}

#[test]
fn refresh_observation_keeps_empty_partial_and_error_classification_explicit() {
    let cases = [
        CatalogRefreshClassification::Started,
        CatalogRefreshClassification::Staged,
        CatalogRefreshClassification::AppliedChanged,
        CatalogRefreshClassification::AppliedUnchanged,
        CatalogRefreshClassification::EmptyValidInitial,
        CatalogRefreshClassification::EmptySuspect,
        CatalogRefreshClassification::Partial,
        CatalogRefreshClassification::Error,
        CatalogRefreshClassification::Interrupted,
    ];
    assert_eq!(CatalogRefreshClassification::ALL, cases);

    let observation = CatalogRefreshObservationV1 {
        contract_version: CATALOG_CONTRACT_VERSION,
        observation_id: trace_id(1),
        target: target(),
        started_at: timestamp(),
        finished_at: timestamp(),
        classification: CatalogRefreshClassification::Partial,
        content_version: None,
        payload_digest: Some(digest()),
        counts: CatalogEntityCountsV1 {
            course_groups: 1,
            course_variants: 1,
            sections: 1,
            occurrences: 0,
        },
        error_class: Some(CatalogRefreshErrorClass::Normalize),
        partial_reasons: vec![CatalogUnknownReason::Malformed],
    };
    let encoded = serde_json::to_string(&observation).unwrap();
    assert_eq!(
        serde_json::from_str::<CatalogRefreshObservationV1>(&encoded).unwrap(),
        observation
    );
}

#[test]
fn discovery_golden_includes_dynamic_subject_scope_and_provenance() {
    let value = CatalogDiscoveryResponseV1 {
        contract_version: CATALOG_CONTRACT_VERSION,
        observed_at: timestamp(),
        status: discovery_status(),
        sources: vec![discovery_source()],
        targets: vec![CatalogTargetV1 {
            key: target(),
            term_label: CatalogFieldKnowledge::present("Synthetic term".to_owned()),
            campus_label: CatalogFieldKnowledge::present("Synthetic campus".to_owned()),
            provenance: discovery_provenance(),
        }],
        subjects: vec![CatalogSubjectV1 {
            target: target(),
            code: CatalogSubjectCode::try_from("SYN:SUBJECT").unwrap(),
            label: CatalogFieldKnowledge::present("Synthetic subject".to_owned()),
            provenance: discovery_provenance(),
        }],
    };
    let actual = format!("{}\n", serde_json::to_string_pretty(&value).unwrap());
    let expected = include_str!("golden/catalog-discovery-v1.json").replace("\r\n", "\n");
    assert_eq!(actual, expected);
    assert_eq!(
        serde_json::from_str::<CatalogDiscoveryResponseV1>(&expected).unwrap(),
        value
    );
}

#[test]
fn discovery_status_expresses_no_first_success_fallback_without_fabricated_data() {
    let status = CatalogDiscoveryStatusV1 {
        availability: CatalogDiscoveryAvailability::UnavailableNoFirstSuccess,
        latest_attempt: Some(discovery_point()),
        last_success: None,
        is_stale: false,
        error: Some(CatalogDiscoveryErrorV1 {
            class: CatalogDiscoveryErrorClass::Transport,
            code: CatalogDiagnosticCode::try_from("UPSTREAM_UNAVAILABLE").unwrap(),
        }),
    };
    let value = serde_json::to_value(status).unwrap();
    assert_eq!(value["availability"], "UNAVAILABLE_NO_FIRST_SUCCESS");
    assert!(value["lastSuccess"].is_null());
    assert!(!value["isStale"].as_bool().unwrap());
}

#[test]
fn delivery_projection_preserves_conflict_and_generic_online_unspecified() {
    let conflict = serde_json::to_string(&CatalogModality::UnknownConflict).unwrap();
    assert_eq!(conflict, r#""UNKNOWN_CONFLICT""#);

    let generic_online = (CatalogModality::Online, CatalogSynchronicity::Unspecified);
    assert_eq!(
        serde_json::to_value(generic_online).unwrap(),
        json!(["ONLINE", "UNSPECIFIED"])
    );
}
