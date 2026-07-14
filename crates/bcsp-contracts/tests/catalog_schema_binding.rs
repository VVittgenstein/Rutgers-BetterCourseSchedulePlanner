use std::collections::BTreeSet;
use std::str::FromStr;

use bcsp_contracts::{
    CATALOG_CONTRACT_VERSION, CatalogCommentV1, CatalogContentVersion, CatalogDiagnosticCode,
    CatalogDiscoveryAvailability, CatalogDiscoveryErrorClass, CatalogDiscoveryErrorV1,
    CatalogDiscoveryPointV1, CatalogDiscoveryProvenanceV1, CatalogDiscoveryRequestV1,
    CatalogDiscoveryResponseV1, CatalogDiscoverySourceId, CatalogDiscoverySourceKind,
    CatalogDiscoverySourceV1, CatalogDiscoveryStatusV1, CatalogEntityCountsV1,
    CatalogFieldKnowledge, CatalogFieldPresence, CatalogInstructorReliability, CatalogModality,
    CatalogOccurrenceEvidence, CatalogOccurrenceKeyV1, CatalogOccurrenceKind,
    CatalogOpenStatusProvenance, CatalogPayloadDigest, CatalogPrerequisiteState,
    CatalogProvenanceV1, CatalogRefreshCheckpointPointV1, CatalogRefreshCheckpointV1,
    CatalogRefreshClassification, CatalogRefreshErrorClass, CatalogRefreshObservationV1,
    CatalogRefreshPointV1, CatalogRefreshStatusV1, CatalogRequiredness,
    CatalogSnapshotOpenStatusV1, CatalogSourceKind, CatalogSubjectCode, CatalogSubjectV1,
    CatalogSynchronicity, CatalogTargetV1, CatalogTimeKnowledgeV1, CatalogUnitMajorV1,
    CatalogUnknownReason, ContractSchema, CourseGroupKey, CourseVariantKey, NormalizedCatalogV1,
    NormalizedCourseGroupV1, NormalizedCourseVariantV1, NormalizedOccurrenceV1,
    NormalizedSectionV1, SectionKey, TermCampusKey, TraceId, contract_manifest,
};
use serde::Serialize;
use serde_json::Value;
use time::OffsetDateTime;

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
    assert!(schema.discriminator.is_none(), "{schema_id}");
    let expected = schema
        .fields
        .into_iter()
        .map(|field| field.name)
        .collect::<BTreeSet<_>>();
    assert_eq!(object_keys(value), expected, "{schema_id}");
}

fn assert_variant_binding<T: Serialize>(schema_id: &str, tag: &str, value: &T) {
    let schema = schema(schema_id);
    let discriminator = schema.discriminator.unwrap();
    let variant = schema
        .variants
        .iter()
        .find(|variant| variant.tag_value == tag)
        .unwrap();
    let mut expected = variant
        .fields
        .iter()
        .map(|field| field.name.clone())
        .collect::<BTreeSet<_>>();
    expected.insert(discriminator.clone());
    assert_eq!(object_keys(value), expected, "{schema_id}:{tag}");
    assert_eq!(
        serde_json::to_value(value).unwrap()[&discriminator],
        Value::String(tag.to_owned())
    );
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
    OffsetDateTime::from_unix_timestamp(0).unwrap()
}

fn trace_id() -> TraceId {
    TraceId::from_str("00000000-0000-4000-8000-000000000001").unwrap()
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

fn provenance() -> CatalogProvenanceV1 {
    CatalogProvenanceV1 {
        observation_id: trace_id(),
        source: CatalogSourceKind::RutgersCatalog,
        target: target(),
        observed_at: timestamp(),
        payload_digest: CatalogPayloadDigest::try_from("a".repeat(64)).unwrap(),
    }
}

fn discovery_provenance() -> CatalogDiscoveryProvenanceV1 {
    CatalogDiscoveryProvenanceV1 {
        observation_id: trace_id(),
        source_id: CatalogDiscoverySourceId::try_from("SYNTHETIC_SELECTOR").unwrap(),
        source_kind: CatalogDiscoverySourceKind::Selector,
        observed_at: timestamp(),
        payload_digest: CatalogPayloadDigest::try_from("a".repeat(64)).unwrap(),
    }
}

#[test]
fn catalog_tagged_unions_bind_to_manifest_variants() {
    assert_variant_binding(
        "bcsp.catalog.field-presence.v1",
        "PRESENT",
        &CatalogFieldPresence::Present { value: "x" },
    );
    assert_variant_binding(
        "bcsp.catalog.field-presence.v1",
        "EXPLICIT_NULL",
        &CatalogFieldPresence::<String>::ExplicitNull,
    );
    assert_variant_binding(
        "bcsp.catalog.field-presence.v1",
        "ABSENT",
        &CatalogFieldPresence::<String>::Absent,
    );
    assert_variant_binding(
        "bcsp.catalog.field-knowledge.v1",
        "KNOWN",
        &CatalogFieldKnowledge::present("x"),
    );
    assert_variant_binding(
        "bcsp.catalog.field-knowledge.v1",
        "UNKNOWN",
        &CatalogFieldKnowledge::<String>::unknown(CatalogUnknownReason::Missing),
    );
    assert_variant_binding(
        "bcsp.catalog.time-knowledge.v1",
        "KNOWN",
        &CatalogTimeKnowledgeV1::Known {
            start_minute: 540,
            end_minute: 620,
        },
    );
}

#[test]
fn catalog_enum_manifest_values_bind_to_serde_names() {
    assert_enum_binding("bcsp.catalog.unknown-reason.v1", CatalogUnknownReason::ALL);
    assert_enum_binding("bcsp.catalog.source-kind.v1", CatalogSourceKind::ALL);
    assert_enum_binding(
        "bcsp.catalog.discovery-source-kind.v1",
        CatalogDiscoverySourceKind::ALL,
    );
    assert_enum_binding(
        "bcsp.catalog.discovery-availability.v1",
        CatalogDiscoveryAvailability::ALL,
    );
    assert_enum_binding(
        "bcsp.catalog.discovery-error-class.v1",
        CatalogDiscoveryErrorClass::ALL,
    );
    assert_enum_binding(
        "bcsp.catalog.open-status-provenance.v1",
        CatalogOpenStatusProvenance::ALL,
    );
    assert_enum_binding(
        "bcsp.catalog.prerequisite-state.v1",
        CatalogPrerequisiteState::ALL,
    );
    assert_enum_binding(
        "bcsp.catalog.instructor-reliability.v1",
        CatalogInstructorReliability::ALL,
    );
    assert_enum_binding("bcsp.catalog.modality.v1", CatalogModality::ALL);
    assert_enum_binding("bcsp.catalog.synchronicity.v1", CatalogSynchronicity::ALL);
    assert_enum_binding(
        "bcsp.catalog.occurrence-kind.v1",
        CatalogOccurrenceKind::ALL,
    );
    assert_enum_binding(
        "bcsp.catalog.occurrence-evidence.v1",
        CatalogOccurrenceEvidence::ALL,
    );
    assert_enum_binding("bcsp.catalog.requiredness.v1", CatalogRequiredness::ALL);
    assert_enum_binding(
        "bcsp.catalog.refresh-classification.v1",
        CatalogRefreshClassification::ALL,
    );
    assert_enum_binding(
        "bcsp.catalog.refresh-error-class.v1",
        CatalogRefreshErrorClass::ALL,
    );
}

#[test]
fn catalog_object_manifest_fields_bind_to_serde_shapes() {
    let discovery_request = CatalogDiscoveryRequestV1::new();
    let target_projection = CatalogTargetV1 {
        key: target(),
        term_label: CatalogFieldKnowledge::present("Term".to_owned()),
        campus_label: CatalogFieldKnowledge::present("Campus".to_owned()),
        provenance: discovery_provenance(),
    };
    let provenance = provenance();
    let discovery_source = CatalogDiscoverySourceV1 {
        source_id: CatalogDiscoverySourceId::try_from("SYNTHETIC_SELECTOR").unwrap(),
        source_kind: CatalogDiscoverySourceKind::Selector,
        source_version: CatalogFieldKnowledge::present("v1".to_owned()),
        payload_digest: CatalogPayloadDigest::try_from("a".repeat(64)).unwrap(),
        observed_at: timestamp(),
    };
    let discovery_point = CatalogDiscoveryPointV1 {
        observation_id: trace_id(),
        observed_at: timestamp(),
        content_version: Some(CatalogContentVersion::try_from(1).unwrap()),
    };
    let discovery_error = CatalogDiscoveryErrorV1 {
        class: CatalogDiscoveryErrorClass::Transport,
        code: CatalogDiagnosticCode::try_from("UPSTREAM_UNAVAILABLE").unwrap(),
    };
    let discovery_status = CatalogDiscoveryStatusV1 {
        availability: CatalogDiscoveryAvailability::StaleLastSuccess,
        latest_attempt: Some(discovery_point.clone()),
        last_success: Some(discovery_point.clone()),
        is_stale: true,
        error: Some(discovery_error.clone()),
    };
    let subject = CatalogSubjectV1 {
        target: target(),
        code: CatalogSubjectCode::try_from("SYN:SUBJECT").unwrap(),
        label: CatalogFieldKnowledge::present("Subject".to_owned()),
        provenance: discovery_provenance(),
    };
    let discovery_response = CatalogDiscoveryResponseV1 {
        contract_version: CATALOG_CONTRACT_VERSION,
        observed_at: timestamp(),
        status: discovery_status.clone(),
        sources: vec![discovery_source.clone()],
        targets: vec![target_projection.clone()],
        subjects: vec![subject.clone()],
    };
    let occurrence_key = CatalogOccurrenceKeyV1 {
        section: section_key(),
        ordinal: 0,
    };
    let group = NormalizedCourseGroupV1 {
        key: group_key(),
        variant_keys: vec![variant_key()],
    };
    let variant = NormalizedCourseVariantV1 {
        key: variant_key(),
        title: CatalogFieldKnowledge::present("Title".to_owned()),
        expanded_title: CatalogFieldKnowledge::absent(),
        description: CatalogFieldKnowledge::absent(),
        notes: CatalogFieldKnowledge::absent(),
        subject_group_notes: CatalogFieldKnowledge::absent(),
        subject_notes: CatalogFieldKnowledge::absent(),
        unit_notes: CatalogFieldKnowledge::absent(),
        synopsis_url: CatalogFieldKnowledge::absent(),
        prerequisite_notes: CatalogFieldKnowledge::absent(),
        prerequisite_state: CatalogPrerequisiteState::NoneReported,
        credits: CatalogFieldKnowledge::unknown(CatalogUnknownReason::SparseEvidence),
        level: CatalogFieldKnowledge::present("SYNTHETIC_LEVEL".to_owned()),
        subject_code: CatalogFieldKnowledge::present("SYN_SUBJECT".to_owned()),
        subject_description: CatalogFieldKnowledge::present("Synthetic subject".to_owned()),
        course_number: CatalogFieldKnowledge::present("SYN_COURSE".to_owned()),
        supplement_code: CatalogFieldKnowledge::absent(),
        school_code: CatalogFieldKnowledge::absent(),
        offering_unit: CatalogFieldKnowledge::explicit_null(),
        offering_unit_title: CatalogFieldKnowledge::absent(),
        core_codes: CatalogFieldKnowledge::present(Vec::new()),
        campus_locations: CatalogFieldKnowledge::present(Vec::new()),
        section_keys: vec![section_key()],
    };
    let open_status = CatalogSnapshotOpenStatusV1 {
        value: CatalogFieldKnowledge::present(true),
        provenance: CatalogOpenStatusProvenance::CatalogSnapshotOnly,
    };
    let section = NormalizedSectionV1 {
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
        instructors: CatalogFieldKnowledge::present(Vec::new()),
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
        catalog_open_status: open_status.clone(),
        occurrence_keys: CatalogFieldKnowledge::present(vec![occurrence_key.clone()]),
    };
    let occurrence = NormalizedOccurrenceV1 {
        key: occurrence_key.clone(),
        raw_code: CatalogFieldKnowledge::present("SYN".to_owned()),
        raw_description: CatalogFieldKnowledge::present("Synthetic".to_owned()),
        modality: CatalogFieldKnowledge::present(CatalogModality::OnCampusOrInPerson),
        synchronicity: CatalogFieldKnowledge::present(CatalogSynchronicity::Sync),
        raw_day: CatalogFieldKnowledge::present("M".to_owned()),
        days: CatalogFieldKnowledge::present(Vec::new()),
        raw_start_time: CatalogFieldKnowledge::unknown(CatalogUnknownReason::NotObserved),
        raw_end_time: CatalogFieldKnowledge::unknown(CatalogUnknownReason::NotObserved),
        time: CatalogTimeKnowledgeV1::Missing,
        start_date: CatalogFieldKnowledge::unknown(CatalogUnknownReason::NotObserved),
        end_date: CatalogFieldKnowledge::unknown(CatalogUnknownReason::NotObserved),
        campus: CatalogFieldKnowledge::present("CAMPUS_A".to_owned()),
        campus_name: CatalogFieldKnowledge::present("Synthetic campus".to_owned()),
        building: CatalogFieldKnowledge::unknown(CatalogUnknownReason::Missing),
        room: CatalogFieldKnowledge::unknown(CatalogUnknownReason::ExplicitNull),
        requiredness: CatalogRequiredness::UnknownRequiredness,
        kind: CatalogOccurrenceKind::Unspecified,
        evidence: CatalogOccurrenceEvidence::None,
        normalization_reason: CatalogDiagnosticCode::try_from("SYNTHETIC_PRIMARY").unwrap(),
    };
    let normalized = NormalizedCatalogV1 {
        contract_version: CATALOG_CONTRACT_VERSION,
        target: target(),
        content_version: CatalogContentVersion::try_from(1).unwrap(),
        provenance: provenance.clone(),
        course_groups: vec![group.clone()],
        course_variants: vec![variant.clone()],
        sections: vec![section.clone()],
        occurrences: vec![occurrence.clone()],
    };
    let counts = CatalogEntityCountsV1 {
        course_groups: 1,
        course_variants: 1,
        sections: 1,
        occurrences: 1,
    };
    let observation = CatalogRefreshObservationV1 {
        contract_version: CATALOG_CONTRACT_VERSION,
        observation_id: trace_id(),
        target: target(),
        started_at: timestamp(),
        finished_at: timestamp(),
        classification: CatalogRefreshClassification::AppliedChanged,
        content_version: Some(CatalogContentVersion::try_from(1).unwrap()),
        payload_digest: Some(CatalogPayloadDigest::try_from("a".repeat(64)).unwrap()),
        counts: counts.clone(),
        error_class: None,
        partial_reasons: Vec::new(),
    };
    let point = CatalogRefreshPointV1 {
        observation_id: trace_id(),
        observed_at: timestamp(),
        classification: CatalogRefreshClassification::AppliedChanged,
        content_version: Some(CatalogContentVersion::try_from(1).unwrap()),
    };
    let checkpoint_point = CatalogRefreshCheckpointPointV1 {
        observation_id: trace_id(),
        observed_at: timestamp(),
        classification: CatalogRefreshClassification::AppliedChanged,
        content_version: Some(CatalogContentVersion::try_from(1).unwrap()),
    };
    let checkpoint = CatalogRefreshCheckpointV1 {
        contract_version: CATALOG_CONTRACT_VERSION,
        target: target(),
        latest_attempt: checkpoint_point.clone(),
        last_success: None,
        last_published: None,
        last_nonempty: None,
        pending_empty: None,
    };
    let status = CatalogRefreshStatusV1 {
        contract_version: CATALOG_CONTRACT_VERSION,
        target: target(),
        latest_attempt: point.clone(),
        last_success: None,
        last_published: None,
        last_nonempty: None,
        pending_empty: None,
    };

    assert_object_binding("bcsp.catalog.discovery-request.v1", &discovery_request);
    assert_object_binding("bcsp.catalog.target.v1", &target_projection);
    assert_object_binding("bcsp.catalog.discovery-response.v1", &discovery_response);
    assert_object_binding("bcsp.catalog.discovery-source.v1", &discovery_source);
    assert_object_binding("bcsp.catalog.discovery-error.v1", &discovery_error);
    assert_object_binding("bcsp.catalog.discovery-point.v1", &discovery_point);
    assert_object_binding("bcsp.catalog.discovery-status.v1", &discovery_status);
    assert_object_binding(
        "bcsp.catalog.discovery-provenance.v1",
        &discovery_provenance(),
    );
    assert_object_binding("bcsp.catalog.provenance.v1", &provenance);
    assert_object_binding("bcsp.catalog.subject.v1", &subject);
    assert_object_binding("bcsp.catalog.occurrence-key.v1", &occurrence_key);
    assert_object_binding("bcsp.catalog.normalized-course-group.v1", &group);
    assert_object_binding("bcsp.catalog.normalized-course-variant.v1", &variant);
    assert_object_binding("bcsp.catalog.snapshot-open-status.v1", &open_status);
    assert_object_binding(
        "bcsp.catalog.unit-major.v1",
        &CatalogUnitMajorV1 {
            unit_code: "SYN_UNIT".to_owned(),
            major_code: String::new(),
        },
    );
    assert_object_binding(
        "bcsp.catalog.comment.v1",
        &CatalogCommentV1 {
            code: CatalogFieldKnowledge::present("SYN_COMMENT".to_owned()),
            description: CatalogFieldKnowledge::present("Synthetic comment".to_owned()),
        },
    );
    assert_object_binding("bcsp.catalog.normalized-section.v1", &section);
    assert_object_binding("bcsp.catalog.normalized-occurrence.v1", &occurrence);
    assert_object_binding("bcsp.catalog.normalized-catalog.v1", &normalized);
    assert_object_binding("bcsp.catalog.entity-counts.v1", &counts);
    assert_object_binding("bcsp.catalog.refresh-observation.v1", &observation);
    assert_object_binding("bcsp.catalog.refresh-point.v1", &point);
    assert_object_binding(
        "bcsp.catalog.refresh-checkpoint-point.v1",
        &checkpoint_point,
    );
    assert_object_binding("bcsp.catalog.refresh-checkpoint.v1", &checkpoint);
    assert_object_binding("bcsp.catalog.refresh-status.v1", &status);
}
