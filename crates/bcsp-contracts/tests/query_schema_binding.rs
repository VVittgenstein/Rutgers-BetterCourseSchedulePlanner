use std::collections::BTreeSet;

use bcsp_contracts::{
    AvailabilityWindowV1, CatalogContentVersion, CatalogDiagnosticCode, CatalogFieldKnowledge,
    CatalogInstructorReliability, CatalogModality, CatalogOccurrenceEvidence,
    CatalogOccurrenceKeyV1, CatalogOccurrenceKind, CatalogOpenStatusProvenance,
    CatalogPrerequisiteState, CatalogRequiredness, CatalogSnapshotOpenStatusV1,
    CatalogSynchronicity, CatalogTimeKnowledgeV1, CatalogUnknownReason, CoreFilterV1,
    CourseDetailRequestV1, CourseDetailResponseV1, CourseGroupKey, CourseQueryItemV1,
    CourseQueryRequestV1, CourseQueryResponseV1, CourseSortV1, CourseVariantKey,
    CourseVariantQueryItemV1, CreditRangeV1, DynamicFilterValidationRequestV3,
    DynamicFilterValidationResponseV3, FilterCanonicalNeutralV1, FilterFieldId, FilterMatchV1,
    FilterOptionTargetVersionV2, FilterRequestV1, FilterSchemaV1, FilterSetModeV1, FilterTokenV1,
    InvalidDynamicFilterValueV3, LiveOpenEvidenceV1, LiveOpenStateV1, MatchExplanation,
    NormalizedCourseGroupV1, NormalizedCourseVariantV1, NormalizedFilterValuesV1,
    NormalizedOccurrenceV1, NormalizedSectionV1, PageInfoV1, PageRequestV1, QUERY_CONTRACT_VERSION,
    SchemaDirection, SectionDetailRequestV1, SectionDetailResponseV1, SectionKey,
    SectionQueryItemV1, SectionQueryRequestV1, SectionQueryResponseV1, SectionSearchItemV1,
    SectionSortV1, TermCampusKey, TermId, TextMatchEvidenceV1, UnknownFieldPolicy, WeekdayV1,
    contract_manifest, filter_schema_v1,
};
use serde::Serialize;
use serde_json::Value;

fn object_keys(value: &impl Serialize) -> BTreeSet<String> {
    let Value::Object(object) = serde_json::to_value(value).unwrap() else {
        panic!("representative is not an object")
    };
    object.into_iter().map(|(key, _)| key).collect()
}

fn assert_binding(id: &str, value: &impl Serialize) {
    let manifest = contract_manifest();
    let schema = manifest
        .schemas
        .iter()
        .find(|schema| schema.id == id)
        .unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(
        object_keys(value),
        schema
            .fields
            .iter()
            .map(|field| field.name.clone())
            .collect::<BTreeSet<_>>(),
        "{id}"
    );
}

fn filters() -> FilterRequestV1 {
    FilterRequestV1::new(NormalizedFilterValuesV1::for_term(
        TermId::try_from("T2026F").unwrap(),
    ))
}

fn group_key() -> CourseGroupKey {
    CourseGroupKey::try_new("T2026F", "CAMPUS_A", "01:198:112").unwrap()
}

fn section_key() -> SectionKey {
    SectionKey::try_new("T2026F", "CAMPUS_A", "12345").unwrap()
}

fn variant_key() -> CourseVariantKey {
    CourseVariantKey::try_new(group_key(), &format!("v1:{}", "a".repeat(64))).unwrap()
}

fn variant() -> NormalizedCourseVariantV1 {
    NormalizedCourseVariantV1 {
        key: variant_key(),
        title: CatalogFieldKnowledge::present("Synthetic course".to_owned()),
        expanded_title: CatalogFieldKnowledge::absent(),
        description: CatalogFieldKnowledge::absent(),
        notes: CatalogFieldKnowledge::absent(),
        subject_group_notes: CatalogFieldKnowledge::absent(),
        subject_notes: CatalogFieldKnowledge::absent(),
        unit_notes: CatalogFieldKnowledge::absent(),
        synopsis_url: CatalogFieldKnowledge::absent(),
        prerequisite_notes: CatalogFieldKnowledge::absent(),
        prerequisite_state: CatalogPrerequisiteState::NoneReported,
        credits: CatalogFieldKnowledge::present("3".to_owned()),
        level: CatalogFieldKnowledge::present("UG".to_owned()),
        subject_code: CatalogFieldKnowledge::present("198".to_owned()),
        subject_description: CatalogFieldKnowledge::present("Computer Science".to_owned()),
        course_number: CatalogFieldKnowledge::present("112".to_owned()),
        supplement_code: CatalogFieldKnowledge::absent(),
        school_code: CatalogFieldKnowledge::absent(),
        offering_unit: CatalogFieldKnowledge::absent(),
        offering_unit_title: CatalogFieldKnowledge::absent(),
        core_codes: CatalogFieldKnowledge::present(Vec::new()),
        campus_locations: CatalogFieldKnowledge::present(Vec::new()),
        section_keys: vec![section_key()],
    }
}

fn section() -> NormalizedSectionV1 {
    NormalizedSectionV1 {
        key: section_key(),
        variant_key: variant_key(),
        section_number: CatalogFieldKnowledge::present("01".to_owned()),
        subtitle: CatalogFieldKnowledge::absent(),
        subtopic: CatalogFieldKnowledge::absent(),
        section_notes: CatalogFieldKnowledge::absent(),
        session_dates: CatalogFieldKnowledge::absent(),
        session_date_print_indicator: CatalogFieldKnowledge::absent(),
        comments: CatalogFieldKnowledge::absent(),
        comments_text: CatalogFieldKnowledge::absent(),
        cross_listed_sections_text: CatalogFieldKnowledge::absent(),
        cross_listed_section_type: CatalogFieldKnowledge::absent(),
        instructors: CatalogFieldKnowledge::present(vec!["Ada Lovelace".to_owned()]),
        instructor_reliability: CatalogInstructorReliability::NameOnly,
        raw_section_course_type: CatalogFieldKnowledge::present("T".to_owned()),
        delivery_modality: CatalogModality::OnCampusOrInPerson,
        synchronicity: CatalogSynchronicity::Sync,
        exam_code: CatalogFieldKnowledge::absent(),
        exam_code_text: CatalogFieldKnowledge::absent(),
        special_permission_add_code: CatalogFieldKnowledge::absent(),
        special_permission_add_description: CatalogFieldKnowledge::absent(),
        special_permission_drop_code: CatalogFieldKnowledge::absent(),
        special_permission_drop_description: CatalogFieldKnowledge::absent(),
        major_codes: CatalogFieldKnowledge::present(Vec::new()),
        unit_codes: CatalogFieldKnowledge::present(Vec::new()),
        minor_codes: CatalogFieldKnowledge::present(Vec::new()),
        honor_program_codes: CatalogFieldKnowledge::present(Vec::new()),
        unit_majors: CatalogFieldKnowledge::present(Vec::new()),
        eligibility_text: CatalogFieldKnowledge::absent(),
        open_to_text: CatalogFieldKnowledge::absent(),
        catalog_open_status: CatalogSnapshotOpenStatusV1 {
            value: CatalogFieldKnowledge::present(true),
            provenance: CatalogOpenStatusProvenance::CatalogSnapshotOnly,
        },
        occurrence_keys: CatalogFieldKnowledge::present(vec![CatalogOccurrenceKeyV1 {
            section: section_key(),
            ordinal: 0,
        }]),
    }
}

fn occurrence() -> NormalizedOccurrenceV1 {
    NormalizedOccurrenceV1 {
        key: CatalogOccurrenceKeyV1 {
            section: section_key(),
            ordinal: 0,
        },
        raw_code: CatalogFieldKnowledge::present("SYN".to_owned()),
        raw_description: CatalogFieldKnowledge::present("Synthetic".to_owned()),
        modality: CatalogFieldKnowledge::present(CatalogModality::OnCampusOrInPerson),
        synchronicity: CatalogFieldKnowledge::present(CatalogSynchronicity::Sync),
        raw_day: CatalogFieldKnowledge::present("M".to_owned()),
        days: CatalogFieldKnowledge::present(vec!["M".to_owned()]),
        raw_start_time: CatalogFieldKnowledge::present("0900".to_owned()),
        raw_end_time: CatalogFieldKnowledge::present("1000".to_owned()),
        time: CatalogTimeKnowledgeV1::Known {
            start_minute: 540,
            end_minute: 600,
        },
        start_date: CatalogFieldKnowledge::unknown(CatalogUnknownReason::NotObserved),
        end_date: CatalogFieldKnowledge::unknown(CatalogUnknownReason::NotObserved),
        campus: CatalogFieldKnowledge::present("CAMPUS_A".to_owned()),
        campus_name: CatalogFieldKnowledge::present("Synthetic campus".to_owned()),
        building: CatalogFieldKnowledge::present("HILL".to_owned()),
        room: CatalogFieldKnowledge::present("114".to_owned()),
        requiredness: CatalogRequiredness::UnknownRequiredness,
        kind: CatalogOccurrenceKind::Scheduled,
        evidence: CatalogOccurrenceEvidence::Physical,
        normalization_reason: CatalogDiagnosticCode::try_from("SYNTHETIC").unwrap(),
    }
}

fn section_item() -> SectionQueryItemV1 {
    SectionQueryItemV1 {
        section: section(),
        occurrences: vec![occurrence()],
        open: LiveOpenEvidenceV1 {
            state: LiveOpenStateV1::Unknown,
            observed_at: None,
            fresh_until: None,
            uncertainty: Some(bcsp_contracts::MatchReasonCode::MissingReliableData),
        },
        explanation: MatchExplanation::matched(),
        filter_matches: vec![FilterMatchV1 {
            field_id: FilterFieldId::SectionIndex,
            explanation: MatchExplanation::matched(),
        }],
    }
}

#[test]
fn query_manifest_directions_enforce_strict_requests_and_additive_responses() {
    for schema in contract_manifest()
        .schemas
        .into_iter()
        .filter(|schema| schema.id.starts_with("bcsp.query."))
    {
        match schema.direction {
            SchemaDirection::ClientToServer => {
                assert_eq!(
                    schema.unknown_fields,
                    UnknownFieldPolicy::Reject,
                    "{}",
                    schema.id
                )
            }
            SchemaDirection::ServerToClient => {
                assert_eq!(
                    schema.unknown_fields,
                    UnknownFieldPolicy::Ignore,
                    "{}",
                    schema.id
                )
            }
            SchemaDirection::Bidirectional => {
                assert_eq!(
                    schema.unknown_fields,
                    UnknownFieldPolicy::Reject,
                    "{}",
                    schema.id
                )
            }
            SchemaDirection::SharedIdentity => panic!("unexpected query shared identity"),
        }
    }
}

#[test]
fn query_manifest_object_fields_match_serde_shapes() {
    let filter_schema: FilterSchemaV1 = filter_schema_v1();
    let values = NormalizedFilterValuesV1::for_term(TermId::try_from("T2026F").unwrap());
    let filter_request = FilterRequestV1::new(values.clone());
    let dynamic_validation_request = DynamicFilterValidationRequestV3::new(filter_request.clone());
    let invalid_dynamic_value = InvalidDynamicFilterValueV3 {
        field: FilterFieldId::CourseSubject,
        value: "999".to_owned(),
    };
    let dynamic_validation_response = DynamicFilterValidationResponseV3 {
        contract_version: QUERY_CONTRACT_VERSION,
        target_versions: vec![FilterOptionTargetVersionV2 {
            target: TermCampusKey::try_new("T2026F", "CAMPUS_A").unwrap(),
            content_version: CatalogContentVersion::try_from(1).unwrap(),
        }],
        invalid_values: vec![invalid_dynamic_value.clone()],
    };
    let page_request = PageRequestV1::default();
    let course_sort = CourseSortV1::default();
    let section_sort = SectionSortV1::default();
    let course_query = CourseQueryRequestV1 {
        filters: filters(),
        page: page_request,
        sort: course_sort,
    };
    let section_query = SectionQueryRequestV1 {
        filters: filters(),
        page: page_request,
        sort: section_sort,
    };
    let course_detail = CourseDetailRequestV1::new(group_key());
    let section_detail = SectionDetailRequestV1::new(section_key());
    let page = PageInfoV1 {
        page: 1,
        page_size: 25,
        total: 0,
        total_pages: 0,
    };
    let course_response = CourseQueryResponseV1 {
        contract_version: QUERY_CONTRACT_VERSION,
        page,
        items: Vec::new(),
    };
    let section_response = SectionQueryResponseV1 {
        contract_version: QUERY_CONTRACT_VERSION,
        page,
        items: Vec::new(),
    };
    let credit_range = CreditRangeV1::try_new(Some(300), Some(400)).unwrap();
    let availability = AvailabilityWindowV1::try_new(WeekdayV1::Monday, 540, 720).unwrap();
    let core = CoreFilterV1 {
        codes: vec![FilterTokenV1::try_from("CC").unwrap()],
        mode: FilterSetModeV1::Any,
    };
    let section_item = section_item();
    let live_open = section_item.open.clone();
    let filter_match = section_item.filter_matches[0].clone();
    let text_match = TextMatchEvidenceV1 {
        exact_course_identifier: true,
        matched_tokens: vec!["01:198:112".to_owned()],
    };
    let section_search_item = SectionSearchItemV1 {
        variant: variant(),
        section: section_item.clone(),
        course_filter_matches: FilterFieldId::ALL[..10]
            .iter()
            .copied()
            .map(|field_id| FilterMatchV1 {
                field_id,
                explanation: MatchExplanation::matched(),
            })
            .collect(),
        text_match: Some(text_match.clone()),
    };
    let variant_item = CourseVariantQueryItemV1 {
        variant: variant(),
        explanation: MatchExplanation::matched(),
        filter_matches: vec![filter_match.clone()],
        text_match: Some(text_match.clone()),
        sections: vec![section_item.clone()],
    };
    let course_item = CourseQueryItemV1 {
        group: NormalizedCourseGroupV1 {
            key: group_key(),
            variant_keys: vec![variant_key()],
        },
        explanation: MatchExplanation::matched(),
        variants: vec![variant_item.clone()],
    };
    let course_detail_response = CourseDetailResponseV1 {
        contract_version: QUERY_CONTRACT_VERSION,
        course: course_item.clone(),
    };
    let section_detail_response = SectionDetailResponseV1 {
        contract_version: QUERY_CONTRACT_VERSION,
        variant: variant(),
        section: section_item.clone(),
    };

    assert_binding("bcsp.query.filter-schema.v1", &filter_schema);
    assert_binding(
        "bcsp.query.filter-field-schema.v1",
        &filter_schema.fields[0],
    );
    assert_binding("bcsp.query.normalized-filter-values.v1", &values);
    assert_binding("bcsp.query.filter-request.v1", &filter_request);
    assert_binding(
        "bcsp.query.dynamic-filter-validation-request.v3",
        &dynamic_validation_request,
    );
    assert_binding(
        "bcsp.query.invalid-dynamic-filter-value.v3",
        &invalid_dynamic_value,
    );
    assert_binding(
        "bcsp.query.dynamic-filter-validation-response.v3",
        &dynamic_validation_response,
    );
    assert_binding("bcsp.query.page-request.v1", &page_request);
    assert_binding("bcsp.query.course-sort.v1", &course_sort);
    assert_binding("bcsp.query.section-sort.v1", &section_sort);
    assert_binding("bcsp.query.course-query-request.v1", &course_query);
    assert_binding("bcsp.query.section-query-request.v1", &section_query);
    assert_binding("bcsp.query.course-detail-request.v1", &course_detail);
    assert_binding("bcsp.query.section-detail-request.v1", &section_detail);
    assert_binding("bcsp.query.page-info.v1", &page);
    assert_binding("bcsp.query.course-query-response.v1", &course_response);
    assert_binding("bcsp.query.section-query-response.v1", &section_response);
    assert_binding("bcsp.query.credit-range.v1", &credit_range);
    assert_binding("bcsp.query.availability-window.v1", &availability);
    assert_binding("bcsp.query.core-filter.v1", &core);
    assert_binding("bcsp.query.live-open-evidence.v1", &live_open);
    assert_binding("bcsp.query.filter-match.v1", &filter_match);
    assert_binding("bcsp.query.text-match-evidence.v1", &text_match);
    assert_binding("bcsp.query.section-query-item.v1", &section_item);
    assert_binding("bcsp.query.section-search-item.v1", &section_search_item);
    assert_eq!(section_search_item.course_filter_matches.len(), 10);
    assert!(
        section_search_item
            .course_filter_matches
            .iter()
            .any(|value| value.field_id == FilterFieldId::CourseText)
    );
    assert_eq!(section_search_item.text_match, Some(text_match.clone()));
    assert_binding("bcsp.query.course-variant-query-item.v1", &variant_item);
    assert_binding("bcsp.query.course-query-item.v1", &course_item);
    assert_binding(
        "bcsp.query.course-detail-response.v1",
        &course_detail_response,
    );
    assert_binding(
        "bcsp.query.section-detail-response.v1",
        &section_detail_response,
    );
}

#[test]
fn canonical_neutral_manifest_binds_required_and_json_variants() {
    let schema = contract_manifest()
        .schemas
        .into_iter()
        .find(|schema| schema.id == "bcsp.query.filter-canonical-neutral.v1")
        .unwrap();
    assert_eq!(schema.discriminator.as_deref(), Some("kind"));

    let cases = [
        (
            "REQUIRED",
            FilterCanonicalNeutralV1::Required,
            BTreeSet::from(["kind".to_owned()]),
        ),
        (
            "JSON",
            FilterCanonicalNeutralV1::Json { value: Value::Null },
            BTreeSet::from(["kind".to_owned(), "value".to_owned()]),
        ),
    ];
    for (tag, value, expected_keys) in cases {
        let serialized = serde_json::to_value(value).unwrap();
        assert_eq!(serialized["kind"], Value::String(tag.to_owned()));
        let Value::Object(object) = serialized else {
            panic!("canonical neutral must serialize as an object")
        };
        assert_eq!(
            object.keys().cloned().collect::<BTreeSet<_>>(),
            expected_keys
        );
        assert!(
            schema
                .variants
                .iter()
                .any(|variant| variant.tag_value == tag)
        );
    }
}
