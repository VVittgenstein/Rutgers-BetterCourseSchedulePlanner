use std::str::FromStr;

use bcsp_contracts::{
    CATALOG_CONTRACT_VERSION, CatalogContentVersion, CatalogDiagnosticCode, CatalogFieldKnowledge,
    CatalogInstructorReliability, CatalogModality, CatalogOccurrenceEvidence,
    CatalogOccurrenceKeyV1, CatalogOccurrenceKind, CatalogOpenStatusProvenance,
    CatalogPayloadDigest, CatalogPrerequisiteState, CatalogProvenanceV1, CatalogRequiredness,
    CatalogSnapshotOpenStatusV1, CatalogSourceKind, CatalogSynchronicity, CatalogTimeKnowledgeV1,
    CatalogUnitMajorV1, CourseGroupKey, CourseQueryRequestV1, CourseSortV1, CourseVariantKey,
    FilterRequestV1, FilterValuesInputV1, LiveOpenEvidenceV1, LiveOpenStateV1, NormalizedCatalogV1,
    NormalizedCourseGroupV1, NormalizedCourseVariantV1, NormalizedFilterValuesV1,
    NormalizedOccurrenceV1, NormalizedSectionV1, PageRequestV1, SectionKey, SectionQueryRequestV1,
    SectionSortV1, TermCampusKey, TermId, TraceId,
};
use bcsp_query::OpenEvidence;
use time::{Duration, OffsetDateTime};

pub const TERM: &str = "92026";

#[derive(Clone)]
pub struct AddedKeys {
    pub group: CourseGroupKey,
    pub variant: CourseVariantKey,
    pub section: SectionKey,
}

pub fn now() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_800_000_000).unwrap()
}

pub fn empty_catalog(campus: &str, version: u64) -> NormalizedCatalogV1 {
    let target = TermCampusKey::try_new(TERM, campus).unwrap();
    NormalizedCatalogV1 {
        contract_version: CATALOG_CONTRACT_VERSION,
        target: target.clone(),
        content_version: CatalogContentVersion::try_from(version).unwrap(),
        provenance: CatalogProvenanceV1 {
            observation_id: TraceId::from_str(&format!("00000000-0000-4000-8000-{version:012x}"))
                .unwrap(),
            source: CatalogSourceKind::RutgersCatalog,
            target,
            observed_at: now(),
            payload_digest: CatalogPayloadDigest::try_from("a".repeat(64)).unwrap(),
        },
        course_groups: Vec::new(),
        course_variants: Vec::new(),
        sections: Vec::new(),
        occurrences: Vec::new(),
    }
}

pub fn add_course(
    catalog: &mut NormalizedCatalogV1,
    course_string: &str,
    index: &str,
    fingerprint_seed: char,
) -> AddedKeys {
    let group = CourseGroupKey::try_new(
        catalog.target.term().as_str(),
        catalog.target.campus().as_str(),
        course_string,
    )
    .unwrap();
    let variant = CourseVariantKey::try_new(
        group.clone(),
        &format!("v1:{}", fingerprint_seed.to_string().repeat(64)),
    )
    .unwrap();
    catalog.course_groups.push(NormalizedCourseGroupV1 {
        key: group.clone(),
        variant_keys: vec![variant.clone()],
    });
    let section = add_section_record(catalog, &variant, index);
    catalog
        .course_variants
        .push(course_variant(variant.clone(), vec![section.clone()]));
    AddedKeys {
        group,
        variant,
        section,
    }
}

pub fn add_variant(
    catalog: &mut NormalizedCatalogV1,
    group: &CourseGroupKey,
    index: &str,
    fingerprint_seed: char,
) -> AddedKeys {
    let variant = CourseVariantKey::try_new(
        group.clone(),
        &format!("v1:{}", fingerprint_seed.to_string().repeat(64)),
    )
    .unwrap();
    catalog
        .course_groups
        .iter_mut()
        .find(|candidate| &candidate.key == group)
        .unwrap()
        .variant_keys
        .push(variant.clone());
    let section = add_section_record(catalog, &variant, index);
    catalog
        .course_variants
        .push(course_variant(variant.clone(), vec![section.clone()]));
    AddedKeys {
        group: group.clone(),
        variant,
        section,
    }
}

pub fn add_section(
    catalog: &mut NormalizedCatalogV1,
    variant: &CourseVariantKey,
    index: &str,
) -> SectionKey {
    let section = add_section_record(catalog, variant, index);
    catalog
        .course_variants
        .iter_mut()
        .find(|candidate| &candidate.key == variant)
        .unwrap()
        .section_keys
        .push(section.clone());
    section
}

fn add_section_record(
    catalog: &mut NormalizedCatalogV1,
    variant: &CourseVariantKey,
    index: &str,
) -> SectionKey {
    let section = SectionKey::try_new(
        catalog.target.term().as_str(),
        catalog.target.campus().as_str(),
        index,
    )
    .unwrap();
    let occurrence_key = CatalogOccurrenceKeyV1 {
        section: section.clone(),
        ordinal: 0,
    };
    catalog.sections.push(section_record(
        section.clone(),
        variant.clone(),
        occurrence_key.clone(),
    ));
    catalog.occurrences.push(occurrence(occurrence_key));
    section
}

fn course_variant(
    key: CourseVariantKey,
    section_keys: Vec<SectionKey>,
) -> NormalizedCourseVariantV1 {
    NormalizedCourseVariantV1 {
        key,
        title: CatalogFieldKnowledge::present("Data Structures".to_owned()),
        expanded_title: CatalogFieldKnowledge::present("Data Structures and Algorithms".to_owned()),
        description: CatalogFieldKnowledge::present("Synthetic description".to_owned()),
        notes: CatalogFieldKnowledge::absent(),
        subject_group_notes: CatalogFieldKnowledge::absent(),
        subject_notes: CatalogFieldKnowledge::absent(),
        unit_notes: CatalogFieldKnowledge::absent(),
        synopsis_url: CatalogFieldKnowledge::absent(),
        prerequisite_notes: CatalogFieldKnowledge::present("01:198:110".to_owned()),
        prerequisite_state: CatalogPrerequisiteState::Has,
        credits: CatalogFieldKnowledge::present("3".to_owned()),
        level: CatalogFieldKnowledge::present("U".to_owned()),
        subject_code: CatalogFieldKnowledge::present("198".to_owned()),
        subject_description: CatalogFieldKnowledge::present("Computer Science".to_owned()),
        course_number: CatalogFieldKnowledge::present("111".to_owned()),
        supplement_code: CatalogFieldKnowledge::absent(),
        school_code: CatalogFieldKnowledge::present("01".to_owned()),
        offering_unit: CatalogFieldKnowledge::present("198".to_owned()),
        offering_unit_title: CatalogFieldKnowledge::present("Computer Science".to_owned()),
        core_codes: CatalogFieldKnowledge::present(vec!["CC".to_owned(), "QR".to_owned()]),
        campus_locations: CatalogFieldKnowledge::present(vec!["BUSCH".to_owned()]),
        section_keys,
    }
}

fn section_record(
    key: SectionKey,
    variant_key: CourseVariantKey,
    occurrence_key: CatalogOccurrenceKeyV1,
) -> NormalizedSectionV1 {
    NormalizedSectionV1 {
        key,
        variant_key,
        section_number: CatalogFieldKnowledge::present("01".to_owned()),
        subtitle: CatalogFieldKnowledge::absent(),
        subtopic: CatalogFieldKnowledge::absent(),
        section_notes: CatalogFieldKnowledge::absent(),
        session_dates: CatalogFieldKnowledge::absent(),
        session_date_print_indicator: CatalogFieldKnowledge::absent(),
        comments: CatalogFieldKnowledge::present(Vec::new()),
        comments_text: CatalogFieldKnowledge::present(String::new()),
        cross_listed_sections_text: CatalogFieldKnowledge::absent(),
        cross_listed_section_type: CatalogFieldKnowledge::absent(),
        instructors: CatalogFieldKnowledge::present(vec!["ADA LOVELACE".to_owned()]),
        instructor_reliability: CatalogInstructorReliability::NameOnly,
        raw_section_course_type: CatalogFieldKnowledge::present("O".to_owned()),
        delivery_modality: CatalogModality::Online,
        synchronicity: CatalogSynchronicity::Sync,
        exam_code: CatalogFieldKnowledge::present("A".to_owned()),
        exam_code_text: CatalogFieldKnowledge::present("FINAL".to_owned()),
        special_permission_add_code: CatalogFieldKnowledge::present("03".to_owned()),
        special_permission_add_description: CatalogFieldKnowledge::present("REQUIRED".to_owned()),
        special_permission_drop_code: CatalogFieldKnowledge::explicit_null(),
        special_permission_drop_description: CatalogFieldKnowledge::explicit_null(),
        major_codes: CatalogFieldKnowledge::present(vec!["198".to_owned()]),
        unit_codes: CatalogFieldKnowledge::present(vec!["01".to_owned()]),
        minor_codes: CatalogFieldKnowledge::present(vec!["M1".to_owned()]),
        honor_program_codes: CatalogFieldKnowledge::present(vec!["H1".to_owned()]),
        unit_majors: CatalogFieldKnowledge::present(vec![CatalogUnitMajorV1 {
            unit_code: "01".to_owned(),
            major_code: "198".to_owned(),
        }]),
        eligibility_text: CatalogFieldKnowledge::present(
            "Structured synthetic eligibility".to_owned(),
        ),
        open_to_text: CatalogFieldKnowledge::present("Synthetic students".to_owned()),
        catalog_open_status: CatalogSnapshotOpenStatusV1 {
            value: CatalogFieldKnowledge::present(false),
            provenance: CatalogOpenStatusProvenance::CatalogSnapshotOnly,
        },
        occurrence_keys: CatalogFieldKnowledge::present(vec![occurrence_key]),
    }
}

fn occurrence(key: CatalogOccurrenceKeyV1) -> NormalizedOccurrenceV1 {
    NormalizedOccurrenceV1 {
        key,
        raw_code: CatalogFieldKnowledge::present("92".to_owned()),
        raw_description: CatalogFieldKnowledge::present("Remote synchronous".to_owned()),
        modality: CatalogFieldKnowledge::present(CatalogModality::Online),
        synchronicity: CatalogFieldKnowledge::present(CatalogSynchronicity::Sync),
        raw_day: CatalogFieldKnowledge::present("M".to_owned()),
        days: CatalogFieldKnowledge::present(vec!["M".to_owned()]),
        raw_start_time: CatalogFieldKnowledge::present("0900".to_owned()),
        raw_end_time: CatalogFieldKnowledge::present("1000".to_owned()),
        time: CatalogTimeKnowledgeV1::Known {
            start_minute: 540,
            end_minute: 600,
        },
        start_date: CatalogFieldKnowledge::absent(),
        end_date: CatalogFieldKnowledge::absent(),
        campus: CatalogFieldKnowledge::present("BUSCH".to_owned()),
        campus_name: CatalogFieldKnowledge::present("Busch".to_owned()),
        building: CatalogFieldKnowledge::present("SEC".to_owned()),
        room: CatalogFieldKnowledge::present("101".to_owned()),
        requiredness: CatalogRequiredness::Required,
        kind: CatalogOccurrenceKind::Scheduled,
        evidence: CatalogOccurrenceEvidence::Remote,
        normalization_reason: CatalogDiagnosticCode::try_from("SYNTHETIC_PRIMARY").unwrap(),
    }
}

pub fn variant_mut<'a>(
    catalog: &'a mut NormalizedCatalogV1,
    key: &CourseVariantKey,
) -> &'a mut NormalizedCourseVariantV1 {
    catalog
        .course_variants
        .iter_mut()
        .find(|variant| &variant.key == key)
        .unwrap()
}

pub fn section_mut<'a>(
    catalog: &'a mut NormalizedCatalogV1,
    key: &SectionKey,
) -> &'a mut NormalizedSectionV1 {
    catalog
        .sections
        .iter_mut()
        .find(|section| &section.key == key)
        .unwrap()
}

pub fn occurrence_mut<'a>(
    catalog: &'a mut NormalizedCatalogV1,
    section: &SectionKey,
) -> &'a mut NormalizedOccurrenceV1 {
    catalog
        .occurrences
        .iter_mut()
        .find(|occurrence| &occurrence.key.section == section)
        .unwrap()
}

pub fn neutral_input() -> FilterValuesInputV1 {
    FilterValuesInputV1::for_term(TermId::try_from(TERM).unwrap())
}

pub fn normalized(input: FilterValuesInputV1) -> NormalizedFilterValuesV1 {
    NormalizedFilterValuesV1::try_new(input).unwrap()
}

pub fn course_request(
    input: FilterValuesInputV1,
    page: PageRequestV1,
    sort: CourseSortV1,
) -> CourseQueryRequestV1 {
    CourseQueryRequestV1 {
        filters: FilterRequestV1::new(normalized(input)),
        page,
        sort,
    }
}

pub fn section_request(
    input: FilterValuesInputV1,
    page: PageRequestV1,
    sort: SectionSortV1,
) -> SectionQueryRequestV1 {
    SectionQueryRequestV1 {
        filters: FilterRequestV1::new(normalized(input)),
        page,
        sort,
    }
}

pub fn open_evidence(section: &SectionKey, state: LiveOpenStateV1) -> OpenEvidence {
    OpenEvidence {
        section_key: section.clone(),
        evidence: LiveOpenEvidenceV1 {
            state,
            observed_at: Some(now()),
            fresh_until: Some(now() + Duration::minutes(5)),
            uncertainty: None,
        },
    }
}
