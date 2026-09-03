mod support;

use std::collections::BTreeSet;
use std::sync::Arc;

use bcsp_contracts::{
    AvailabilityWindowV1, CampusCode, CatalogFieldKnowledge, CatalogModality,
    CatalogOccurrenceEvidence, CatalogPrerequisiteState, CatalogRequiredness, CatalogSubjectCode,
    CatalogSynchronicity, CatalogUnknownReason, CoreFilterV1, CourseDetailRequestV1,
    CourseGroupKey, CourseQueryRequestV1, CourseSortFieldV1, CourseSortV1, CourseVariantKey,
    CreditRangeV1, FilterFieldId, FilterRequestV1, FilterSearchTextV1, FilterSetModeV1,
    FilterTokenV1, LiveOpenEvidenceV1, LiveOpenStateV1, MatchOutcome, MatchReasonCode,
    MeetingLocationFilterV2, MeetingLocationMatchModeV2, PageRequestV1, PermissionFilterV1,
    PrerequisiteFilterV1, SectionDetailRequestV1, SectionQueryRequestV1, SectionSortV1,
    SortDirectionV1, UserModalityV3, UserSynchronicityV3, WeekdayV1,
};
use bcsp_query::{
    CorpusError, PreparedCatalogCorpus, PreparedOpenOverlay, QueryEngine, QueryError, TextHit,
    TextHitPlan, TextTargetVersion, evaluate_section_filters,
};
use time::Duration;

use support::*;

fn token(value: &str) -> FilterTokenV1 {
    FilterTokenV1::try_from(value).unwrap()
}

#[test]
fn prepared_open_lookup_preserves_freshness_boundary_and_forced_mask_precedence() {
    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    let target = catalog.target.clone();
    let corpus = PreparedCatalogCorpus::try_new([Arc::new(catalog)]).expect("prepared corpus");
    let detail = SectionDetailRequestV1::new(keys.section.clone());

    let mut conflicting = open_evidence(&keys.section, LiveOpenStateV1::Open);
    conflicting.evidence.uncertainty = Some(MatchReasonCode::ConflictingEvidence);
    let overlay = PreparedOpenOverlay::try_new(&corpus, &target, [conflicting])
        .expect("prepared conflicting evidence");
    let engine = QueryEngine::try_new_prepared(
        &corpus,
        std::slice::from_ref(&target),
        std::slice::from_ref(&target),
        now(),
        &[&overlay],
    )
    .expect("forced prepared engine");
    assert_eq!(
        engine
            .section_detail(&detail)
            .unwrap()
            .section
            .open
            .uncertainty,
        Some(MatchReasonCode::SourceUnavailable),
        "forced precommit unavailability overrides an existing uncertainty"
    );

    let missing =
        PreparedOpenOverlay::try_new(&corpus, &target, []).expect("prepared missing evidence");
    let engine = QueryEngine::try_new_prepared(
        &corpus,
        std::slice::from_ref(&target),
        std::slice::from_ref(&target),
        now(),
        &[&missing],
    )
    .expect("forced missing engine");
    assert_eq!(
        engine
            .section_detail(&detail)
            .unwrap()
            .section
            .open
            .uncertainty,
        Some(MatchReasonCode::MissingReliableData),
        "a missing Open record remains missing rather than becoming forced evidence"
    );

    let fresh_until = now() + Duration::minutes(5);
    let mut fresh = open_evidence(&keys.section, LiveOpenStateV1::Open);
    fresh.evidence.observed_at = Some(now());
    fresh.evidence.fresh_until = Some(fresh_until);
    fresh.evidence.uncertainty = None;
    let fresh =
        PreparedOpenOverlay::try_new(&corpus, &target, [fresh]).expect("prepared fresh evidence");
    let at_boundary = QueryEngine::try_new_prepared(
        &corpus,
        std::slice::from_ref(&target),
        &[],
        fresh_until,
        &[&fresh],
    )
    .expect("boundary engine");
    assert_eq!(
        at_boundary
            .section_detail(&detail)
            .unwrap()
            .section
            .open
            .uncertainty,
        None
    );
    let after_boundary = QueryEngine::try_new_prepared(
        &corpus,
        std::slice::from_ref(&target),
        &[],
        fresh_until + Duration::nanoseconds(1),
        &[&fresh],
    )
    .expect("stale engine");
    assert_eq!(
        after_boundary
            .section_detail(&detail)
            .unwrap()
            .section
            .open
            .uncertainty,
        Some(MatchReasonCode::SourceUnavailable)
    );
}

#[test]
fn all_18_v3_filters_match_one_same_variant_and_section() {
    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    let mut input = neutral_input();
    input.campuses = vec![CampusCode::try_from("NB").unwrap()];
    input.subjects = vec![CatalogSubjectCode::try_from("198").unwrap()];
    input.text = Some(FilterSearchTextV1::try_from("01:198:111").unwrap());
    input.course_number_bands = vec![100];
    input.levels = vec![token("U")];
    input.credits = Some(CreditRangeV1::try_new(Some(300), Some(300)).unwrap());
    input.core.codes = vec![token("CC"), token("QR")];
    input.core.mode = FilterSetModeV1::All;
    input.prerequisite = PrerequisiteFilterV1::Has;
    input.section_indexes = vec![keys.section.index().clone()];
    input.open_statuses = vec![LiveOpenStateV1::Open];
    input.modalities = vec![UserModalityV3::Online];
    input.synchronicities = vec![UserSynchronicityV3::Sync];
    input.instructors = vec![token("ADA   LOVELACE")];
    input.availability = vec![
        AvailabilityWindowV1::try_new(WeekdayV1::Monday, 540, 570).unwrap(),
        AvailabilityWindowV1::try_new(WeekdayV1::Monday, 570, 600).unwrap(),
        AvailabilityWindowV1::try_new(WeekdayV1::Monday, 540, 600).unwrap(),
    ];
    input.meeting_locations = MeetingLocationFilterV2 {
        locations: vec![token("BUSCH")],
        ..MeetingLocationFilterV2::default()
    };
    // This intentionally matches the text field rather than the code field.
    input.exam_codes = vec![token("FINAL")];
    input.permission = PermissionFilterV1::Required;
    let filters = normalized(input);
    let text = filters.text().unwrap().clone();
    let request = CourseQueryRequestV1 {
        filters: FilterRequestV1::new(filters),
        page: PageRequestV1::default(),
        sort: CourseSortV1::default(),
    };
    let plan = TextHitPlan::try_new(
        &text,
        [TextTargetVersion {
            target: catalog.target.clone(),
            content_version: catalog.content_version,
        }],
        [TextHit {
            variant_key: keys.variant.clone(),
            fts_rank: 42,
        }],
    )
    .unwrap();
    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(
        &catalogs,
        now(),
        [open_evidence(&keys.section, LiveOpenStateV1::Open)],
    )
    .unwrap();
    let response = engine.course_search(&request, Some(&plan)).unwrap();

    assert_eq!(response.page.total, 1);
    assert_eq!(response.items[0].explanation.outcome(), MatchOutcome::Match);
    let variant = &response.items[0].variants[0];
    assert_eq!(variant.explanation.outcome(), MatchOutcome::Match);
    assert_eq!(variant.filter_matches.len(), 9);
    assert_eq!(variant.sections[0].filter_matches.len(), 9);
    assert_eq!(
        variant.sections[0].explanation.outcome(),
        MatchOutcome::Match
    );
    assert!(variant.text_match.as_ref().unwrap().exact_course_identifier);
    let observed = variant
        .filter_matches
        .iter()
        .chain(variant.sections[0].filter_matches.iter())
        .map(|entry| entry.field_id)
        .collect::<BTreeSet<_>>();
    assert_eq!(observed, FilterFieldId::ALL.iter().copied().collect());
    assert!(
        variant
            .filter_matches
            .iter()
            .chain(variant.sections[0].filter_matches.iter())
            .all(|entry| entry.explanation.outcome() == MatchOutcome::Match)
    );
}

#[test]
fn every_03_through_18_canonical_neutral_preserves_unknown_and_missing_corpus() {
    let mut catalog = empty_catalog("NB", 1);
    add_course(&mut catalog, "01:198:111", "00001", 'a');
    let incomplete = add_course(&mut catalog, "01:198:211", "00002", 'b');
    let variant = variant_mut(&mut catalog, &incomplete.variant);
    variant.subject_code = CatalogFieldKnowledge::unknown(CatalogUnknownReason::NotObserved);
    variant.course_number = CatalogFieldKnowledge::present("NOT_NUMERIC".to_owned());
    variant.level = CatalogFieldKnowledge::unknown(CatalogUnknownReason::SourceUnavailable);
    variant.credits = CatalogFieldKnowledge::absent();
    variant.core_codes = CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed);
    variant.prerequisite_notes = CatalogFieldKnowledge::explicit_null();
    variant.prerequisite_state = CatalogPrerequisiteState::Unknown;
    let section = section_mut(&mut catalog, &incomplete.section);
    section.instructors = CatalogFieldKnowledge::unknown(CatalogUnknownReason::SparseEvidence);
    section.delivery_modality = CatalogModality::UnknownConflict;
    section.synchronicity = CatalogSynchronicity::Unknown;
    section.exam_code = CatalogFieldKnowledge::unknown(CatalogUnknownReason::NotObserved);
    section.exam_code_text = CatalogFieldKnowledge::absent();
    section.special_permission_add_code =
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::ConflictingEvidence);
    section.special_permission_add_description = CatalogFieldKnowledge::explicit_null();
    section.special_permission_drop_code = CatalogFieldKnowledge::absent();
    section.special_permission_drop_description = CatalogFieldKnowledge::absent();
    let occurrence = occurrence_mut(&mut catalog, &incomplete.section);
    occurrence.modality = CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed);
    occurrence.synchronicity =
        CatalogFieldKnowledge::unknown(CatalogUnknownReason::SourceUnavailable);
    occurrence.days = CatalogFieldKnowledge::unknown(CatalogUnknownReason::NotObserved);
    occurrence.time = bcsp_contracts::CatalogTimeKnowledgeV1::Invalid;
    occurrence.campus = CatalogFieldKnowledge::unknown(CatalogUnknownReason::SparseEvidence);
    occurrence.building = CatalogFieldKnowledge::explicit_null();
    occurrence.room = CatalogFieldKnowledge::absent();
    occurrence.requiredness = CatalogRequiredness::UnknownRequiredness;

    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).expect("fixed mixed-quality corpus");
    let assert_neutral = |field: FilterFieldId, input: bcsp_contracts::FilterValuesInputV1| {
        let course = engine
            .course_search(
                &course_request(
                    input.clone(),
                    PageRequestV1::default(),
                    CourseSortV1::default(),
                ),
                None,
            )
            .unwrap_or_else(|error| panic!("{field} course neutral failed: {error:?}"));
        let section = engine
            .section_search(
                &section_request(input, PageRequestV1::default(), SectionSortV1::default()),
                None,
            )
            .unwrap_or_else(|error| panic!("{field} section neutral failed: {error:?}"));
        assert_eq!(course.page.total, 2, "{field} narrowed Course results");
        assert_eq!(section.page.total, 2, "{field} narrowed Section results");
    };

    let fields = [
        FilterFieldId::CourseSubject,
        FilterFieldId::CourseText,
        FilterFieldId::CourseNumberBand,
        FilterFieldId::CourseLevel,
        FilterFieldId::CourseCredits,
        FilterFieldId::CourseCoreCode,
        FilterFieldId::CoursePrerequisite,
        FilterFieldId::SectionIndex,
        FilterFieldId::SectionOpenStatus,
        FilterFieldId::SectionModality,
        FilterFieldId::SectionSynchronicity,
        FilterFieldId::SectionInstructor,
        FilterFieldId::SectionAvailability,
        FilterFieldId::SectionMeetingLocation,
        FilterFieldId::SectionExam,
        FilterFieldId::SectionPermission,
    ];
    assert_eq!(fields, FilterFieldId::ALL[2..]);
    for field in fields {
        let mut input = neutral_input();
        match field {
            FilterFieldId::CourseSubject => input.subjects = Vec::new(),
            FilterFieldId::CourseText => input.text = None,
            FilterFieldId::CourseNumberBand => input.course_number_bands = Vec::new(),
            FilterFieldId::CourseLevel => input.levels = Vec::new(),
            FilterFieldId::CourseCredits => input.credits = None,
            FilterFieldId::CourseCoreCode => input.core = CoreFilterV1::default(),
            FilterFieldId::CoursePrerequisite => input.prerequisite = PrerequisiteFilterV1::Any,
            FilterFieldId::SectionIndex => input.section_indexes = Vec::new(),
            FilterFieldId::SectionOpenStatus => input.open_statuses = Vec::new(),
            FilterFieldId::SectionModality => input.modalities = Vec::new(),
            FilterFieldId::SectionSynchronicity => input.synchronicities = Vec::new(),
            FilterFieldId::SectionInstructor => input.instructors = Vec::new(),
            FilterFieldId::SectionAvailability => input.availability = Vec::new(),
            FilterFieldId::SectionMeetingLocation => {
                input.meeting_locations = MeetingLocationFilterV2::default()
            }
            FilterFieldId::SectionExam => input.exam_codes = Vec::new(),
            FilterFieldId::SectionPermission => input.permission = PermissionFilterV1::Any,
            FilterFieldId::CourseTerm | FilterFieldId::CourseCampus => {
                unreachable!("01–02 are QueryScope, not the 03–18 filter rail")
            }
        }
        assert_neutral(field, input);
    }

    for field in [
        FilterFieldId::CoursePrerequisite,
        FilterFieldId::SectionModality,
        FilterFieldId::SectionSynchronicity,
    ] {
        let mut input = neutral_input();
        match field {
            FilterFieldId::CoursePrerequisite => input.include_incomplete.prerequisite = true,
            FilterFieldId::SectionModality => input.include_incomplete.modality = true,
            FilterFieldId::SectionSynchronicity => input.include_incomplete.synchronicity = true,
            _ => unreachable!("the include-incomplete controls are field-bound"),
        }
        assert_neutral(field, input);
    }
}

#[test]
fn section_predicates_cannot_stitch_across_sections_or_variants() {
    let mut catalog = empty_catalog("NB", 1);
    let first = add_course(&mut catalog, "01:198:111", "00001", 'a');
    let second_section = add_section(&mut catalog, &first.variant, "00002");
    section_mut(&mut catalog, &second_section).instructors =
        CatalogFieldKnowledge::present(vec!["GRACE HOPPER".to_owned()]);
    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(
        &catalogs,
        now(),
        [
            open_evidence(&first.section, LiveOpenStateV1::Open),
            open_evidence(&second_section, LiveOpenStateV1::Closed),
        ],
    )
    .unwrap();
    let mut input = neutral_input();
    input.open_statuses = vec![LiveOpenStateV1::Open];
    input.instructors = vec![token("GRACE HOPPER")];
    let request = course_request(input, PageRequestV1::default(), CourseSortV1::default());
    assert_eq!(engine.course_search(&request, None).unwrap().page.total, 0);

    let mut catalog = empty_catalog("NB", 1);
    let first = add_course(&mut catalog, "01:198:111", "00001", 'a');
    let second = add_variant(&mut catalog, &first.group, "00002", 'b');
    variant_mut(&mut catalog, &second.variant).level =
        CatalogFieldKnowledge::present("G".to_owned());
    section_mut(&mut catalog, &second.section).instructors =
        CatalogFieldKnowledge::present(vec!["GRACE HOPPER".to_owned()]);
    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();
    let mut input = neutral_input();
    input.levels = vec![token("U")];
    input.instructors = vec![token("GRACE HOPPER")];
    let request = course_request(input, PageRequestV1::default(), CourseSortV1::default());
    assert_eq!(engine.course_search(&request, None).unwrap().page.total, 0);
}

#[test]
fn full_section_key_keeps_same_index_across_campuses_distinct() {
    let mut nb = empty_catalog("NB", 1);
    let nb_keys = add_course(&mut nb, "01:198:111", "00001", 'a');
    let mut nk = empty_catalog("NK", 1);
    let nk_keys = add_course(&mut nk, "21:198:111", "00001", 'b');
    let catalogs = vec![nb, nk];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();
    let request = section_request(
        neutral_input(),
        PageRequestV1::default(),
        SectionSortV1::default(),
    );
    let response = engine.section_search(&request, None).unwrap();
    assert_eq!(response.page.total, 2);
    assert_ne!(
        response.items[0].section.section.key,
        response.items[1].section.section.key
    );
    assert_eq!(response.items[0].section.occurrences.len(), 1);

    let detail = engine
        .section_detail(&SectionDetailRequestV1::new(nk_keys.section.clone()))
        .unwrap();
    assert_eq!(detail.section.section.key.campus().as_str(), "NK");
    assert_eq!(detail.section.occurrences.len(), 1);
    let course = engine
        .course_detail(&CourseDetailRequestV1::new(nb_keys.group.clone()))
        .unwrap();
    assert_eq!(course.course.group.key.campus().as_str(), "NB");

    let mut input = neutral_input();
    input.campuses = vec![CampusCode::try_from("NB").unwrap()];
    let request = section_request(
        input,
        PageRequestV1::try_new(1, 1).unwrap(),
        SectionSortV1::default(),
    );
    let response = engine.section_search(&request, None).unwrap();
    assert_eq!(response.page.total, 1);
    assert_eq!(response.items.len(), 1);
    assert_eq!(
        response.items[0].section.section.key.campus().as_str(),
        "NB"
    );
}

#[test]
fn unknown_open_is_selectable_but_not_proof_of_open_or_closed() {
    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    let section = section_mut(&mut catalog, &keys.section).clone();
    let occurrence_refs = catalog.occurrences.iter().collect::<Vec<_>>();

    let outcome = |selected: LiveOpenStateV1, evidence: LiveOpenEvidenceV1| {
        let mut input = neutral_input();
        input.open_statuses = vec![selected];
        evaluate_section_filters(
            &section,
            Some(&occurrence_refs),
            &evidence,
            now(),
            &normalized(input),
        )
        .0
        .outcome()
    };
    let unknown = LiveOpenEvidenceV1 {
        state: LiveOpenStateV1::Unknown,
        observed_at: None,
        fresh_until: None,
        uncertainty: Some(MatchReasonCode::SourceUnavailable),
    };
    assert_eq!(
        outcome(LiveOpenStateV1::Unknown, unknown.clone()),
        MatchOutcome::Match
    );
    assert_eq!(
        outcome(LiveOpenStateV1::Open, unknown),
        MatchOutcome::Uncertain
    );
    let stale_open = LiveOpenEvidenceV1 {
        state: LiveOpenStateV1::Open,
        observed_at: Some(now() - Duration::minutes(10)),
        fresh_until: Some(now() - Duration::seconds(1)),
        uncertainty: None,
    };
    assert_eq!(
        outcome(LiveOpenStateV1::Unknown, stale_open.clone()),
        MatchOutcome::Match
    );
    assert_eq!(
        outcome(LiveOpenStateV1::Open, stale_open),
        MatchOutcome::Uncertain
    );
    let fresh_closed = open_evidence(&keys.section, LiveOpenStateV1::Closed).evidence;
    assert_eq!(
        outcome(LiveOpenStateV1::Open, fresh_closed),
        MatchOutcome::NoMatch
    );
}

#[test]
fn fts_plan_is_version_bound_exact_priority_and_filter_before_page() {
    let mut catalog = empty_catalog("NB", 2);
    let _first = add_course(&mut catalog, "01:198:111", "00001", 'a');
    let exact = add_course(&mut catalog, "01:198:112", "00002", 'b');
    let third = add_course(&mut catalog, "01:198:113", "00003", 'c');
    let mut input = neutral_input();
    input.text = Some(FilterSearchTextV1::try_from("01:198:112").unwrap());
    let filters = normalized(input);
    let text = filters.text().unwrap().clone();
    let request = CourseQueryRequestV1 {
        filters: FilterRequestV1::new(filters),
        page: PageRequestV1::try_new(1, 1).unwrap(),
        sort: CourseSortV1::default(),
    };
    let plan = TextHitPlan::try_new(
        &text,
        [TextTargetVersion {
            target: catalog.target.clone(),
            content_version: catalog.content_version,
        }],
        [
            TextHit {
                variant_key: exact.variant.clone(),
                fts_rank: 100,
            },
            TextHit {
                variant_key: third.variant,
                fts_rank: 2,
            },
        ],
    )
    .unwrap();
    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();
    let response = engine.course_search(&request, Some(&plan)).unwrap();
    assert_eq!(response.page.total, 2);
    assert_eq!(response.page.total_pages, 2);
    assert_eq!(response.items[0].group.key, exact.group);

    let section_request = SectionQueryRequestV1 {
        filters: request.filters.clone(),
        page: PageRequestV1::try_new(1, 1).unwrap(),
        sort: SectionSortV1::default(),
    };
    let section_response = engine
        .section_search(&section_request, Some(&plan))
        .unwrap();
    assert_eq!(section_response.page.total, 2);
    assert_eq!(section_response.items[0].variant.key, exact.variant);
    assert_eq!(section_response.items[0].course_filter_matches.len(), 9);
    let text_match = section_response.items[0].text_match.as_ref().unwrap();
    assert!(text_match.exact_course_identifier);
    assert_eq!(text_match.matched_tokens, text.tokens());

    let stale_plan = TextHitPlan::try_new(
        &text,
        [TextTargetVersion {
            target: catalogs[0].target.clone(),
            content_version: 1_u64.try_into().unwrap(),
        }],
        [],
    )
    .unwrap();
    assert_eq!(
        engine.course_search(&request, Some(&stale_plan)),
        Err(QueryError::TextContentVersionMismatch)
    );
    assert_eq!(
        engine.course_search(&request, None),
        Err(QueryError::TextEvidenceUnavailable)
    );
}

#[test]
fn active_section_filter_requires_a_real_section_witness() {
    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    catalog.sections.clear();
    catalog.occurrences.clear();
    variant_mut(&mut catalog, &keys.variant)
        .section_keys
        .clear();
    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();

    let neutral = course_request(
        neutral_input(),
        PageRequestV1::default(),
        CourseSortV1::default(),
    );
    assert_eq!(engine.course_search(&neutral, None).unwrap().page.total, 1);

    let mut active = neutral_input();
    active.section_indexes = vec![keys.section.index().clone()];
    let active = course_request(active, PageRequestV1::default(), CourseSortV1::default());
    assert_eq!(engine.course_search(&active, None).unwrap().page.total, 0);
}

#[test]
fn course_search_materializes_only_section_witnesses_but_detail_stays_complete() {
    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    let rejected = add_section(&mut catalog, &keys.variant, "00002");
    section_mut(&mut catalog, &rejected).section_number =
        CatalogFieldKnowledge::present("02".to_owned());
    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();

    let mut input = neutral_input();
    input.section_indexes = vec![keys.section.index().clone()];
    let request = course_request(input, PageRequestV1::default(), CourseSortV1::default());
    let response = engine.course_search(&request, None).unwrap();
    let variant = &response.items[0].variants[0];
    assert_eq!(variant.sections.len(), 1);
    assert_eq!(variant.sections[0].section.key, keys.section);
    assert!(
        variant
            .sections
            .iter()
            .all(|section| section.explanation.outcome() != MatchOutcome::NoMatch)
    );

    let detail = engine
        .course_detail(&CourseDetailRequestV1::new(keys.group))
        .unwrap();
    assert_eq!(detail.course.variants[0].sections.len(), 2);
    assert!(
        detail.course.variants[0]
            .sections
            .iter()
            .any(|section| section.section.key == rejected)
    );
}

#[test]
fn unknown_core_code_is_rejected_against_selected_catalog_targets() {
    let mut catalog = empty_catalog("NB", 1);
    add_course(&mut catalog, "01:198:111", "00001", 'a');
    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();
    let mut input = neutral_input();
    input.core.codes = vec![token("NOT_A_REAL_CORE")];
    let request = course_request(input, PageRequestV1::default(), CourseSortV1::default());
    assert_eq!(
        engine.course_search(&request, None),
        Err(QueryError::UnknownCoreCode {
            code: "NOT_A_REAL_CORE".to_owned(),
        })
    );
}

#[test]
fn core_any_and_all_are_evaluated_within_each_variant() {
    let mut catalog = empty_catalog("NB", 1);
    let first = add_course(&mut catalog, "01:198:111", "00001", 'a');
    let second = add_variant(&mut catalog, &first.group, "00002", 'b');
    variant_mut(&mut catalog, &first.variant).core_codes =
        CatalogFieldKnowledge::present(vec!["CC".to_owned()]);
    variant_mut(&mut catalog, &second.variant).core_codes =
        CatalogFieldKnowledge::present(vec!["QR".to_owned()]);
    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();

    let mut any = neutral_input();
    any.core.codes = vec![token("CC"), token("QR")];
    any.core.mode = FilterSetModeV1::Any;
    let response = engine
        .course_search(
            &course_request(any, PageRequestV1::default(), CourseSortV1::default()),
            None,
        )
        .unwrap();
    assert_eq!(response.page.total, 1);
    assert_eq!(response.items[0].variants.len(), 2);

    let mut all = neutral_input();
    all.core.codes = vec![token("CC"), token("QR")];
    all.core.mode = FilterSetModeV1::All;
    let response = engine
        .course_search(
            &course_request(all, PageRequestV1::default(), CourseSortV1::default()),
            None,
        )
        .unwrap();
    assert_eq!(response.page.total, 0);
}

#[test]
fn credit_ranges_require_full_containment_without_guessing_arranged_values() {
    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    variant_mut(&mut catalog, &keys.variant).credits =
        CatalogFieldKnowledge::present("1_0-3_0".to_owned());
    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();

    let mut too_narrow = neutral_input();
    too_narrow.credits = Some(CreditRangeV1::try_new(None, Some(200)).unwrap());
    let request = course_request(
        too_narrow,
        PageRequestV1::default(),
        CourseSortV1::default(),
    );
    assert_eq!(engine.course_search(&request, None).unwrap().page.total, 0);

    let mut contains = neutral_input();
    contains.credits = Some(CreditRangeV1::try_new(Some(100), Some(300)).unwrap());
    let request = course_request(contains, PageRequestV1::default(), CourseSortV1::default());
    assert_eq!(engine.course_search(&request, None).unwrap().page.total, 1);

    let mut boundary_catalog = catalogs[0].clone();
    variant_mut(&mut boundary_catalog, &keys.variant).credits =
        CatalogFieldKnowledge::present("2_0".to_owned());
    let boundary_catalogs = vec![boundary_catalog];
    let engine = QueryEngine::try_new(&boundary_catalogs, now(), []).unwrap();
    let mut boundary = neutral_input();
    boundary.credits = Some(CreditRangeV1::try_new(None, Some(200)).unwrap());
    let request = course_request(boundary, PageRequestV1::default(), CourseSortV1::default());
    assert_eq!(engine.course_search(&request, None).unwrap().page.total, 1);

    let mut fixed_catalog = catalogs[0].clone();
    variant_mut(&mut fixed_catalog, &keys.variant).credits =
        CatalogFieldKnowledge::present("3_0".to_owned());
    let fixed_catalogs = vec![fixed_catalog];
    let engine = QueryEngine::try_new(&fixed_catalogs, now(), []).unwrap();
    let mut maximum_two = neutral_input();
    maximum_two.credits = Some(CreditRangeV1::try_new(None, Some(200)).unwrap());
    let request = course_request(
        maximum_two,
        PageRequestV1::default(),
        CourseSortV1::default(),
    );
    assert_eq!(engine.course_search(&request, None).unwrap().page.total, 0);

    let mut arranged_catalog = catalogs[0].clone();
    variant_mut(&mut arranged_catalog, &keys.variant).credits =
        CatalogFieldKnowledge::present("BA".to_owned());
    let arranged_catalogs = vec![arranged_catalog];
    let engine = QueryEngine::try_new(&arranged_catalogs, now(), []).unwrap();
    let mut input = neutral_input();
    input.credits = Some(CreditRangeV1::try_new(Some(300), Some(300)).unwrap());
    let request = course_request(input, PageRequestV1::default(), CourseSortV1::default());
    let response = engine.course_search(&request, None).unwrap();
    assert_eq!(response.page.total, 1);
    assert_eq!(
        response.items[0].explanation.outcome(),
        MatchOutcome::Uncertain
    );

    let mut invalid_catalog = catalogs[0].clone();
    variant_mut(&mut invalid_catalog, &keys.variant).credits =
        CatalogFieldKnowledge::present("3 OR 4".to_owned());
    let invalid_catalogs = vec![invalid_catalog];
    let engine = QueryEngine::try_new(&invalid_catalogs, now(), []).unwrap();
    let mut input = neutral_input();
    input.credits = Some(CreditRangeV1::try_new(None, Some(200)).unwrap());
    let request = course_request(input, PageRequestV1::default(), CourseSortV1::default());
    let response = engine.course_search(&request, None).unwrap();
    let credit_match = response.items[0].variants[0]
        .filter_matches
        .iter()
        .find(|entry| entry.field_id == FilterFieldId::CourseCredits)
        .expect("credit explanation");
    assert_eq!(credit_match.explanation.outcome(), MatchOutcome::Uncertain);
    assert_eq!(
        credit_match.explanation.reasons()[0].code(),
        MatchReasonCode::InvalidValue
    );
}

#[test]
fn duplicate_target_snapshots_are_rejected_even_when_empty() {
    let catalogs = vec![empty_catalog("NB", 1), empty_catalog("NB", 2)];
    assert!(matches!(
        QueryEngine::try_new(&catalogs, now(), []),
        Err(QueryError::InvalidCorpus(CorpusError::DuplicateTarget))
    ));
}

#[test]
fn only_matching_variant_controls_relevance_title_and_section_display_outcome() {
    let mut catalog = empty_catalog("NB", 1);
    let group_a = add_course(&mut catalog, "01:198:111", "00001", 'a');
    let rejected = add_variant(&mut catalog, &group_a.group, "00002", 'b');
    variant_mut(&mut catalog, &group_a.variant).title =
        CatalogFieldKnowledge::present("ZZZ".to_owned());
    variant_mut(&mut catalog, &rejected.variant).title =
        CatalogFieldKnowledge::present("AAA".to_owned());
    variant_mut(&mut catalog, &rejected.variant).credits =
        CatalogFieldKnowledge::present("9".to_owned());
    let group_b = add_course(&mut catalog, "01:198:112", "00003", 'c');
    variant_mut(&mut catalog, &group_b.variant).title =
        CatalogFieldKnowledge::present("MMM".to_owned());
    let rejected_group = add_course(&mut catalog, "01:198:113", "00004", 'd');
    variant_mut(&mut catalog, &rejected_group.variant).credits =
        CatalogFieldKnowledge::present("9".to_owned());

    let mut input = neutral_input();
    input.text = Some(FilterSearchTextV1::try_from("data").unwrap());
    input.credits = Some(CreditRangeV1::try_new(Some(300), Some(300)).unwrap());
    let filters = normalized(input);
    let text = filters.text().unwrap().clone();
    let plan = TextHitPlan::try_new(
        &text,
        [TextTargetVersion {
            target: catalog.target.clone(),
            content_version: catalog.content_version,
        }],
        [
            TextHit {
                variant_key: group_a.variant.clone(),
                fts_rank: 50,
            },
            TextHit {
                variant_key: rejected.variant.clone(),
                fts_rank: 0,
            },
            TextHit {
                variant_key: group_b.variant.clone(),
                fts_rank: 10,
            },
            TextHit {
                variant_key: rejected_group.variant,
                fts_rank: 0,
            },
        ],
    )
    .unwrap();
    let request = CourseQueryRequestV1 {
        filters: FilterRequestV1::new(filters.clone()),
        page: PageRequestV1::default(),
        sort: CourseSortV1::default(),
    };
    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();
    let response = engine.course_search(&request, Some(&plan)).unwrap();
    assert_eq!(response.page.total, 2);
    assert_eq!(response.items[0].group.key, group_b.group);

    let group_a_item = response
        .items
        .iter()
        .find(|item| item.group.key == group_a.group)
        .unwrap();
    assert_eq!(group_a_item.variants.len(), 1);
    assert_eq!(group_a_item.variants[0].variant.key, group_a.variant);
    assert!(
        group_a_item
            .variants
            .iter()
            .all(|item| item.variant.key != rejected.variant)
    );

    let title_request = CourseQueryRequestV1 {
        filters: FilterRequestV1::new(filters),
        page: PageRequestV1::default(),
        sort: CourseSortV1 {
            field: CourseSortFieldV1::Title,
            direction: SortDirectionV1::Ascending,
        },
    };
    let response = engine.course_search(&title_request, Some(&plan)).unwrap();
    assert_eq!(response.items[0].group.key, group_b.group);
    assert_eq!(response.items[1].group.key, group_a.group);
}

#[test]
fn text_plan_rejects_query_target_set_and_foreign_variant_mismatches() {
    let mut nb = empty_catalog("NB", 1);
    let nb_keys = add_course(&mut nb, "01:198:111", "00001", 'a');
    let mut nk = empty_catalog("NK", 1);
    add_course(&mut nk, "21:198:111", "00001", 'b');
    let mut input = neutral_input();
    input.text = Some(FilterSearchTextV1::try_from("data").unwrap());
    let filters = normalized(input);
    let query = filters.text().unwrap().clone();
    let request = CourseQueryRequestV1 {
        filters: FilterRequestV1::new(filters),
        page: PageRequestV1::default(),
        sort: CourseSortV1::default(),
    };
    let catalogs = vec![nb, nk];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();

    let wrong_query = TextHitPlan::try_new(
        &FilterSearchTextV1::try_from("algorithms").unwrap(),
        catalogs.iter().map(|catalog| TextTargetVersion {
            target: catalog.target.clone(),
            content_version: catalog.content_version,
        }),
        [],
    )
    .unwrap();
    assert_eq!(
        engine.course_search(&request, Some(&wrong_query)),
        Err(QueryError::TextQueryMismatch)
    );

    let missing_target = TextHitPlan::try_new(
        &query,
        [TextTargetVersion {
            target: catalogs[0].target.clone(),
            content_version: catalogs[0].content_version,
        }],
        [TextHit {
            variant_key: nb_keys.variant.clone(),
            fts_rank: 0,
        }],
    )
    .unwrap();
    assert_eq!(
        engine.course_search(&request, Some(&missing_target)),
        Err(QueryError::TextTargetSetMismatch)
    );

    let foreign_key = CourseVariantKey::try_new(
        CourseGroupKey::try_new(TERM, "NB", "01:198:999").unwrap(),
        &format!("v1:{}", "f".repeat(64)),
    )
    .unwrap();
    let foreign_hit = TextHitPlan::try_new(
        &query,
        catalogs.iter().map(|catalog| TextTargetVersion {
            target: catalog.target.clone(),
            content_version: catalog.content_version,
        }),
        [TextHit {
            variant_key: foreign_key,
            fts_rank: 0,
        }],
    )
    .unwrap();
    assert_eq!(
        engine.course_search(&request, Some(&foreign_hit)),
        Err(QueryError::ForeignTextHit)
    );
}

#[test]
fn permission_null_keeps_three_value_semantics() {
    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    section_mut(&mut catalog, &keys.section).special_permission_add_code =
        CatalogFieldKnowledge::explicit_null();
    let section = catalog.sections[0].clone();
    let occurrence_refs = catalog.occurrences.iter().collect::<Vec<_>>();
    let mut input = neutral_input();
    input.permission = PermissionFilterV1::NotRequired;
    let (outcome, matches) = evaluate_section_filters(
        &section,
        Some(&occurrence_refs),
        &open_evidence(&keys.section, LiveOpenStateV1::Open).evidence,
        now(),
        &normalized(input),
    );
    assert_eq!(outcome.outcome(), MatchOutcome::Match);
    assert_eq!(
        matches
            .iter()
            .find(|entry| entry.field_id == FilterFieldId::SectionPermission)
            .unwrap()
            .explanation
            .outcome(),
        MatchOutcome::Match
    );
}

#[test]
fn large_synthetic_corpus_has_stable_complete_nonduplicated_pages() {
    let mut catalog = empty_catalog("NB", 1);
    for ordinal in 1..=250 {
        add_course(
            &mut catalog,
            &format!("01:198:{ordinal:03}"),
            &format!("{ordinal:05}"),
            'a',
        );
    }
    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();
    let mut observed = Vec::new();
    for page in 1..=7 {
        let request = course_request(
            neutral_input(),
            PageRequestV1::try_new(page, 37).unwrap(),
            CourseSortV1::default(),
        );
        let response = engine.course_search(&request, None).unwrap();
        assert_eq!(response.page.total, 250);
        assert_eq!(response.page.total_pages, 7);
        observed.extend(response.items.into_iter().map(|item| item.group.key));
    }
    assert_eq!(observed.len(), 250);
    assert_eq!(observed.iter().collect::<BTreeSet<_>>().len(), 250);

    let request = course_request(
        neutral_input(),
        PageRequestV1::try_new(1, 37).unwrap(),
        CourseSortV1::default(),
    );
    let first = engine
        .course_search(&request, None)
        .unwrap()
        .items
        .into_iter()
        .map(|item| item.group.key)
        .collect::<Vec<_>>();
    assert_eq!(&observed[..37], first);
}

#[test]
fn large_nested_course_result_preserves_total_sort_and_only_returns_page_shape() {
    const GROUP_COUNT: u32 = 600;
    const SECTIONS_PER_GROUP: u32 = 6;
    const PAGE: u32 = 7;
    const PAGE_SIZE: u16 = 25;

    let mut catalog = empty_catalog("NB", 1);
    let mut expected_keys = Vec::new();
    for ordinal in 0..GROUP_COUNT {
        let subject = 100 + ordinal / 100;
        let course_number = ordinal % 100;
        let course_string = format!("01:{subject:03}:{course_number:03}");
        let first_index = ordinal * SECTIONS_PER_GROUP + 1;
        let keys = add_course(
            &mut catalog,
            &course_string,
            &format!("{first_index:05}"),
            'a',
        );
        for extra in 1..SECTIONS_PER_GROUP {
            add_section(
                &mut catalog,
                &keys.variant,
                &format!("{:05}", first_index + extra),
            );
        }
        expected_keys.push(keys.group);
    }

    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();
    let request = course_request(
        neutral_input(),
        PageRequestV1::try_new(PAGE, PAGE_SIZE).unwrap(),
        CourseSortV1::default(),
    );
    let response = engine.course_search(&request, None).unwrap();
    let start = usize::try_from((PAGE - 1) * u32::from(PAGE_SIZE)).unwrap();
    let end = start + usize::from(PAGE_SIZE);

    assert_eq!(response.page.total, u64::from(GROUP_COUNT));
    assert_eq!(response.items.len(), usize::from(PAGE_SIZE));
    assert_eq!(
        response
            .items
            .iter()
            .map(|item| item.group.key.clone())
            .collect::<Vec<_>>(),
        expected_keys[start..end]
    );
    assert_eq!(
        response
            .items
            .iter()
            .flat_map(|item| &item.variants)
            .flat_map(|variant| &variant.sections)
            .count(),
        usize::from(PAGE_SIZE) * usize::try_from(SECTIONS_PER_GROUP).unwrap()
    );
}

#[test]
fn unknown_course_knowledge_remains_uncertain_and_reasons_are_stable() {
    let mut catalog = empty_catalog("NB", 1);
    let matched = add_course(&mut catalog, "01:198:111", "00001", 'a');
    let uncertain = add_variant(&mut catalog, &matched.group, "00002", 'b');
    let variant = variant_mut(&mut catalog, &uncertain.variant);
    variant.subject_code = CatalogFieldKnowledge::absent();
    variant.course_number = CatalogFieldKnowledge::absent();
    variant.level = CatalogFieldKnowledge::absent();
    variant.credits = CatalogFieldKnowledge::present("BA".to_owned());
    variant.core_codes = CatalogFieldKnowledge::absent();
    variant.prerequisite_state = bcsp_contracts::CatalogPrerequisiteState::Unknown;
    variant.campus_locations = CatalogFieldKnowledge::absent();

    let mut input = neutral_input();
    input.subjects = vec![CatalogSubjectCode::try_from("198").unwrap()];
    input.course_number_bands = vec![100];
    input.levels = vec![token("U")];
    input.credits = Some(CreditRangeV1::try_new(Some(300), Some(300)).unwrap());
    input.core.codes = vec![token("CC")];
    input.prerequisite = PrerequisiteFilterV1::Has;
    input.include_incomplete.prerequisite = true;
    let request = course_request(input, PageRequestV1::default(), CourseSortV1::default());
    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();
    let response = engine.course_search(&request, None).unwrap();
    let uncertain = response.items[0]
        .variants
        .iter()
        .find(|item| item.variant.key == uncertain.variant)
        .unwrap();
    assert_eq!(uncertain.explanation.outcome(), MatchOutcome::Uncertain);
    for field in [
        FilterFieldId::CourseSubject,
        FilterFieldId::CourseNumberBand,
        FilterFieldId::CourseLevel,
        FilterFieldId::CourseCredits,
        FilterFieldId::CourseCoreCode,
        FilterFieldId::CoursePrerequisite,
    ] {
        let explanation = &uncertain
            .filter_matches
            .iter()
            .find(|entry| entry.field_id == field)
            .unwrap()
            .explanation;
        assert_eq!(explanation.outcome(), MatchOutcome::Uncertain, "{field}");
        assert!(
            explanation
                .reasons()
                .iter()
                .all(|reason| reason.field().as_str() == field.wire_name())
        );
    }
    assert_eq!(
        uncertain.sections[0].explanation.outcome(),
        MatchOutcome::Uncertain
    );
}

#[test]
fn delivery_axes_do_not_guess_conflicting_or_unspecified_values() {
    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    let section = section_mut(&mut catalog, &keys.section);
    section.delivery_modality = CatalogModality::UnknownConflict;
    section.synchronicity = CatalogSynchronicity::Unspecified;
    occurrence_mut(&mut catalog, &keys.section).synchronicity =
        CatalogFieldKnowledge::present(CatalogSynchronicity::Unspecified);
    let mut input = neutral_input();
    input.modalities = vec![UserModalityV3::Online];
    input.synchronicities = vec![UserSynchronicityV3::Sync];
    input.include_incomplete.modality = true;
    input.include_incomplete.synchronicity = true;
    let filters = normalized(input);
    let occurrence_refs = catalog.occurrences.iter().collect::<Vec<_>>();
    let (overall, matches) = evaluate_section_filters(
        &catalog.sections[0],
        Some(&occurrence_refs),
        &open_evidence(&keys.section, LiveOpenStateV1::Open).evidence,
        now(),
        &filters,
    );
    assert_eq!(overall.outcome(), MatchOutcome::Uncertain);
    assert_eq!(
        matches
            .iter()
            .find(|entry| entry.field_id == FilterFieldId::SectionModality)
            .unwrap()
            .explanation
            .reasons()[0]
            .code(),
        MatchReasonCode::ConflictingEvidence
    );
    assert_eq!(
        matches
            .iter()
            .find(|entry| entry.field_id == FilterFieldId::SectionSynchronicity)
            .unwrap()
            .explanation
            .outcome(),
        MatchOutcome::Uncertain
    );
}

#[test]
fn incomplete_admission_is_additive_and_required_for_each_active_uncertain_axis() {
    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    variant_mut(&mut catalog, &keys.variant).prerequisite_state =
        bcsp_contracts::CatalogPrerequisiteState::Unknown;
    let section = section_mut(&mut catalog, &keys.section);
    section.delivery_modality = CatalogModality::Other;
    section.synchronicity = CatalogSynchronicity::Unspecified;
    occurrence_mut(&mut catalog, &keys.section).synchronicity =
        CatalogFieldKnowledge::present(CatalogSynchronicity::Unspecified);

    let request_for = |prerequisite: bool, modality: bool, synchronicity: bool| {
        let mut input = neutral_input();
        input.prerequisite = PrerequisiteFilterV1::Has;
        input.modalities = vec![UserModalityV3::Online];
        input.synchronicities = vec![UserSynchronicityV3::Sync];
        input.include_incomplete.prerequisite = prerequisite;
        input.include_incomplete.modality = modality;
        input.include_incomplete.synchronicity = synchronicity;
        course_request(input, PageRequestV1::default(), CourseSortV1::default())
    };

    let excluded_request = request_for(false, false, false);
    let occurrence_refs = catalog.occurrences.iter().collect::<Vec<_>>();
    let (raw_section_evaluation, raw_section_matches) = evaluate_section_filters(
        &catalog.sections[0],
        Some(&occurrence_refs),
        &open_evidence(&keys.section, LiveOpenStateV1::Open).evidence,
        now(),
        excluded_request.filters.values(),
    );
    assert_eq!(
        raw_section_evaluation.outcome(),
        MatchOutcome::Uncertain,
        "admission must not rewrite raw predicate evidence"
    );
    for field in [
        FilterFieldId::SectionModality,
        FilterFieldId::SectionSynchronicity,
    ] {
        let explanation = &raw_section_matches
            .iter()
            .find(|entry| entry.field_id == field)
            .unwrap()
            .explanation;
        assert_eq!(explanation.outcome(), MatchOutcome::Uncertain, "{field}");
        assert!(
            explanation
                .reasons()
                .iter()
                .all(|reason| !reason.code().is_known_mismatch()),
            "excluded uncertainty must retain uncertainty reasons for {field}"
        );
    }

    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();
    for flags in [
        (false, false, false),
        (true, false, false),
        (true, true, false),
    ] {
        let response = engine
            .course_search(&request_for(flags.0, flags.1, flags.2), None)
            .unwrap();
        assert_eq!(response.page.total, 0, "flags={flags:?}");
    }
    let admitted = engine
        .course_search(&request_for(true, true, true), None)
        .unwrap();
    assert_eq!(admitted.page.total, 1);
    assert_eq!(
        admitted.items[0].explanation.outcome(),
        MatchOutcome::Uncertain
    );
}

#[test]
fn incomplete_section_admission_is_additive_without_hiding_raw_descendants_or_detail() {
    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    let uncertain_section = add_section(&mut catalog, &keys.variant, "00002");
    let known_mismatch_section = add_section(&mut catalog, &keys.variant, "00003");
    section_mut(&mut catalog, &uncertain_section).delivery_modality = CatalogModality::Other;
    section_mut(&mut catalog, &known_mismatch_section).delivery_modality =
        CatalogModality::OnCampusOrInPerson;

    let request_for = |include_incomplete: bool| {
        let mut input = neutral_input();
        input.modalities = vec![UserModalityV3::Online];
        input.include_incomplete.modality = include_incomplete;
        input
    };
    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();

    let strict_course = engine
        .course_search(
            &course_request(
                request_for(false),
                PageRequestV1::default(),
                CourseSortV1::default(),
            ),
            None,
        )
        .unwrap();
    assert_eq!(strict_course.page.total, 1);
    assert_eq!(strict_course.items[0].variants[0].sections.len(), 1);
    assert_eq!(
        strict_course.items[0].variants[0].sections[0].section.key,
        keys.section
    );
    let strict_sections = engine
        .section_search(
            &section_request(
                request_for(false),
                PageRequestV1::default(),
                SectionSortV1::default(),
            ),
            None,
        )
        .unwrap();
    assert_eq!(strict_sections.page.total, 1);

    let additive_course = engine
        .course_search(
            &course_request(
                request_for(true),
                PageRequestV1::default(),
                CourseSortV1::default(),
            ),
            None,
        )
        .unwrap();
    assert_eq!(additive_course.page.total, 1);
    let sections = &additive_course.items[0].variants[0].sections;
    assert_eq!(sections.len(), 2, "known mismatch must stay excluded");
    let uncertain = sections
        .iter()
        .find(|item| item.section.key == uncertain_section)
        .unwrap();
    assert_eq!(uncertain.section.delivery_modality, CatalogModality::Other);
    assert_eq!(uncertain.explanation.outcome(), MatchOutcome::Uncertain);
    let modality_evidence = &uncertain
        .filter_matches
        .iter()
        .find(|entry| entry.field_id == FilterFieldId::SectionModality)
        .unwrap()
        .explanation;
    assert_eq!(modality_evidence.outcome(), MatchOutcome::Uncertain);
    assert_eq!(
        modality_evidence.reasons()[0].code(),
        MatchReasonCode::UnknownValue
    );
    assert!(
        sections
            .iter()
            .all(|item| item.section.key != known_mismatch_section),
        "includeIncomplete must never admit a known mismatch"
    );

    let additive_sections = engine
        .section_search(
            &section_request(
                request_for(true),
                PageRequestV1::default(),
                SectionSortV1::default(),
            ),
            None,
        )
        .unwrap();
    assert_eq!(additive_sections.page.total, 2);
    assert_eq!(
        additive_sections
            .items
            .iter()
            .find(|item| item.section.section.key == uncertain_section)
            .unwrap()
            .section
            .explanation
            .outcome(),
        MatchOutcome::Uncertain
    );

    let course_detail = engine
        .course_detail(&CourseDetailRequestV1::new(keys.group))
        .unwrap();
    assert_eq!(course_detail.course.variants[0].sections.len(), 3);
    let section_detail = engine
        .section_detail(&SectionDetailRequestV1::new(uncertain_section))
        .unwrap();
    assert_eq!(
        section_detail.section.section.delivery_modality,
        CatalogModality::Other
    );
}

#[test]
fn incomplete_switches_are_no_ops_for_neutral_fields() {
    let mut catalog = empty_catalog("NB", 1);
    add_course(&mut catalog, "01:198:111", "00001", 'a');
    let mut input = neutral_input();
    input.include_incomplete.prerequisite = true;
    input.include_incomplete.modality = true;
    input.include_incomplete.synchronicity = true;
    let request = course_request(input, PageRequestV1::default(), CourseSortV1::default());
    let catalogs = vec![catalog];
    let response = QueryEngine::try_new(&catalogs, now(), [])
        .unwrap()
        .course_search(&request, None)
        .unwrap();
    assert_eq!(response.page.total, 1);
    assert_eq!(response.items[0].explanation.outcome(), MatchOutcome::Match);
}

#[test]
fn course_number_band_zero_matches_every_actual_ascii_number_from_000_through_099() {
    let mut catalog = empty_catalog("NB", 1);
    for (identifier, index, fingerprint, number) in [
        ("01:198:000", "00001", 'a', "000"),
        ("01:198:099", "00002", 'b', "099"),
        ("01:198:100", "00003", 'c', "100"),
    ] {
        let keys = add_course(&mut catalog, identifier, index, fingerprint);
        variant_mut(&mut catalog, &keys.variant).course_number =
            CatalogFieldKnowledge::present(number.to_owned());
    }
    let mut input = neutral_input();
    input.course_number_bands = vec![0];
    let request = course_request(input, PageRequestV1::default(), CourseSortV1::default());
    let catalogs = vec![catalog];
    let response = QueryEngine::try_new(&catalogs, now(), [])
        .unwrap()
        .course_search(&request, None)
        .unwrap();
    assert_eq!(response.page.total, 2);
    assert!(response.items.iter().all(|item| {
        matches!(
            item.group.key.course_string().as_str(),
            "01:198:000" | "01:198:099"
        )
    }));
}

#[test]
fn synchronicity_constrains_only_the_online_components_of_one_section() {
    let judge = |section_modality: CatalogModality,
                 section_synchronicity: CatalogSynchronicity,
                 components: &[(
        CatalogModality,
        CatalogOccurrenceEvidence,
        CatalogSynchronicity,
    )],
                 selected: UserSynchronicityV3| {
        let mut catalog = empty_catalog("NB", 1);
        let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
        let template = catalog.occurrences[0].clone();
        catalog.occurrences = components
            .iter()
            .enumerate()
            .map(|(ordinal, (modality, evidence, synchronicity))| {
                let mut occurrence = template.clone();
                occurrence.key.ordinal = ordinal as u32;
                occurrence.modality = CatalogFieldKnowledge::present(*modality);
                occurrence.evidence = *evidence;
                occurrence.synchronicity = CatalogFieldKnowledge::present(*synchronicity);
                occurrence
            })
            .collect();
        let occurrence_keys = catalog
            .occurrences
            .iter()
            .map(|occurrence| occurrence.key.clone())
            .collect();
        let section = section_mut(&mut catalog, &keys.section);
        section.delivery_modality = section_modality;
        section.synchronicity = section_synchronicity;
        section.occurrence_keys = CatalogFieldKnowledge::present(occurrence_keys);
        let mut input = neutral_input();
        input.synchronicities = vec![selected];
        let filters = normalized(input);
        let refs = catalog.occurrences.iter().collect::<Vec<_>>();
        evaluate_section_filters(
            &catalog.sections[0],
            Some(&refs),
            &open_evidence(&keys.section, LiveOpenStateV1::Open).evidence,
            now(),
            &filters,
        )
        .0
        .outcome()
    };

    let in_person = (
        CatalogModality::OnCampusOrInPerson,
        CatalogOccurrenceEvidence::Physical,
        CatalogSynchronicity::Sync,
    );
    let online_sync = (
        CatalogModality::Online,
        CatalogOccurrenceEvidence::Remote,
        CatalogSynchronicity::Sync,
    );
    let online_async = (
        CatalogModality::Online,
        CatalogOccurrenceEvidence::Remote,
        CatalogSynchronicity::Async,
    );
    let online_unknown = (
        CatalogModality::Online,
        CatalogOccurrenceEvidence::Remote,
        CatalogSynchronicity::Unspecified,
    );
    let hybrid_sync_occurrence = (
        CatalogModality::Hybrid,
        CatalogOccurrenceEvidence::Physical,
        CatalogSynchronicity::Sync,
    );

    assert_eq!(
        judge(
            CatalogModality::OnCampusOrInPerson,
            CatalogSynchronicity::Sync,
            &[in_person],
            UserSynchronicityV3::Async,
        ),
        MatchOutcome::Match,
        "a timing preference has no online component to constrain"
    );
    assert_eq!(
        judge(
            CatalogModality::OnCampusOrInPerson,
            CatalogSynchronicity::ByArrangement,
            &[(
                CatalogModality::OnCampusOrInPerson,
                CatalogOccurrenceEvidence::Physical,
                CatalogSynchronicity::ByArrangement,
            )],
            UserSynchronicityV3::Sync,
        ),
        MatchOutcome::Match,
        "on-site hours by arrangement are outside the online timing filter"
    );
    // Rutgers can label the Section T while a finer meetingMode 91 occurrence
    // explicitly contains an online HYBRID component. The occurrence wins for
    // applicability; the coarse Section modality must not hide it.
    assert_eq!(
        judge(
            CatalogModality::OnCampusOrInPerson,
            CatalogSynchronicity::Sync,
            &[hybrid_sync_occurrence],
            UserSynchronicityV3::Async,
        ),
        MatchOutcome::NoMatch
    );
    assert_eq!(
        judge(
            CatalogModality::OnCampusOrInPerson,
            CatalogSynchronicity::Sync,
            &[hybrid_sync_occurrence],
            UserSynchronicityV3::Sync,
        ),
        MatchOutcome::Match
    );
    assert_eq!(
        judge(
            CatalogModality::Online,
            CatalogSynchronicity::Async,
            &[online_async],
            UserSynchronicityV3::Async,
        ),
        MatchOutcome::Match
    );
    assert_eq!(
        judge(
            CatalogModality::Online,
            CatalogSynchronicity::Async,
            &[online_async],
            UserSynchronicityV3::Sync,
        ),
        MatchOutcome::NoMatch
    );

    // The stored Section aggregate is MIXED because it includes the physical
    // scheduled meeting. FLT-S04b must derive ASYNC from its online subset.
    let hybrid = [in_person, online_async];
    assert_eq!(
        judge(
            CatalogModality::Hybrid,
            CatalogSynchronicity::Mixed,
            &hybrid,
            UserSynchronicityV3::Async,
        ),
        MatchOutcome::Match
    );
    assert_eq!(
        judge(
            CatalogModality::Hybrid,
            CatalogSynchronicity::Mixed,
            &hybrid,
            UserSynchronicityV3::Mixed,
        ),
        MatchOutcome::NoMatch
    );
    assert_eq!(
        judge(
            CatalogModality::Hybrid,
            CatalogSynchronicity::Mixed,
            &[in_person, online_sync, online_async],
            UserSynchronicityV3::Mixed,
        ),
        MatchOutcome::Match,
        "MIXED means both timing kinds occur within the online subset"
    );
    assert_eq!(
        judge(
            CatalogModality::Hybrid,
            CatalogSynchronicity::Sync,
            &[in_person],
            UserSynchronicityV3::Sync,
        ),
        MatchOutcome::Uncertain,
        "HYBRID promises an online part but no occurrence describes its timing"
    );
    assert_eq!(
        judge(
            CatalogModality::Online,
            CatalogSynchronicity::Unknown,
            &[(
                CatalogModality::UnknownConflict,
                CatalogOccurrenceEvidence::Remote,
                CatalogSynchronicity::Sync,
            )],
            UserSynchronicityV3::Sync,
        ),
        MatchOutcome::Match,
        "remote evidence keeps timing applicable despite a location conflict"
    );
    assert_eq!(
        judge(
            CatalogModality::Online,
            CatalogSynchronicity::Unknown,
            &[(
                CatalogModality::Online,
                CatalogOccurrenceEvidence::Remote,
                CatalogSynchronicity::Unspecified,
            )],
            UserSynchronicityV3::Sync,
        ),
        MatchOutcome::Uncertain
    );

    // Incomplete members retain known impossibilities. Once a SYNC online
    // component is known, the final category may be SYNC or MIXED but can no
    // longer be pure ASYNC. The remaining possible categories stay uncertain.
    let known_sync_and_unknown = [online_sync, online_unknown];
    assert_eq!(
        judge(
            CatalogModality::Online,
            CatalogSynchronicity::Unknown,
            &known_sync_and_unknown,
            UserSynchronicityV3::Async,
        ),
        MatchOutcome::NoMatch
    );
    for possible in [UserSynchronicityV3::Sync, UserSynchronicityV3::Mixed] {
        assert_eq!(
            judge(
                CatalogModality::Online,
                CatalogSynchronicity::Unknown,
                &known_sync_and_unknown,
                possible,
            ),
            MatchOutcome::Uncertain,
            "a still-possible category cannot be proved while one member is unknown"
        );
    }

    // A known pair conclusively proves MIXED. An unknown extra component
    // cannot undo the coexistence of both known kinds.
    let known_mixed_and_unknown = [online_sync, online_async, online_unknown];
    assert_eq!(
        judge(
            CatalogModality::Online,
            CatalogSynchronicity::Unknown,
            &known_mixed_and_unknown,
            UserSynchronicityV3::Mixed,
        ),
        MatchOutcome::Match
    );
    for impossible in [UserSynchronicityV3::Sync, UserSynchronicityV3::Async] {
        assert_eq!(
            judge(
                CatalogModality::Online,
                CatalogSynchronicity::Unknown,
                &known_mixed_and_unknown,
                impossible,
            ),
            MatchOutcome::NoMatch
        );
    }
}

#[test]
fn remote_by_arrangement_repairs_only_inconclusive_stored_synchronicity() {
    let judge = |section_modality: CatalogModality,
                 components: &[(
        CatalogModality,
        CatalogOccurrenceEvidence,
        CatalogSynchronicity,
        bcsp_contracts::CatalogOccurrenceKind,
    )],
                 selected: UserSynchronicityV3| {
        let mut catalog = empty_catalog("NB", 1);
        let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
        let template = catalog.occurrences[0].clone();
        catalog.occurrences = components
            .iter()
            .enumerate()
            .map(|(ordinal, (modality, evidence, synchronicity, kind))| {
                let mut occurrence = template.clone();
                occurrence.key.ordinal = ordinal as u32;
                occurrence.modality = CatalogFieldKnowledge::present(*modality);
                occurrence.evidence = *evidence;
                occurrence.synchronicity = CatalogFieldKnowledge::present(*synchronicity);
                occurrence.kind = *kind;
                occurrence
            })
            .collect();
        let occurrence_keys = catalog
            .occurrences
            .iter()
            .map(|occurrence| occurrence.key.clone())
            .collect();
        let section = section_mut(&mut catalog, &keys.section);
        section.delivery_modality = section_modality;
        section.synchronicity = CatalogSynchronicity::Unknown;
        section.occurrence_keys = CatalogFieldKnowledge::present(occurrence_keys);
        let mut input = neutral_input();
        input.synchronicities = vec![selected];
        let filters = normalized(input);
        let refs = catalog.occurrences.iter().collect::<Vec<_>>();
        evaluate_section_filters(
            &catalog.sections[0],
            Some(&refs),
            &open_evidence(&keys.section, LiveOpenStateV1::Open).evidence,
            now(),
            &filters,
        )
        .0
        .outcome()
    };

    let physical_sync = (
        CatalogModality::OnCampusOrInPerson,
        CatalogOccurrenceEvidence::Physical,
        CatalogSynchronicity::Sync,
        bcsp_contracts::CatalogOccurrenceKind::Scheduled,
    );
    let remote_arranged_with_old_value = (
        CatalogModality::Online,
        CatalogOccurrenceEvidence::Remote,
        CatalogSynchronicity::Unspecified,
        bcsp_contracts::CatalogOccurrenceKind::ByArrangement,
    );
    let remote_sync = (
        CatalogModality::Online,
        CatalogOccurrenceEvidence::Remote,
        CatalogSynchronicity::Sync,
        bcsp_contracts::CatalogOccurrenceKind::Scheduled,
    );
    let remote_arranged_with_explicit_sync = (
        CatalogModality::Online,
        CatalogOccurrenceEvidence::Remote,
        CatalogSynchronicity::Sync,
        bcsp_contracts::CatalogOccurrenceKind::ByArrangement,
    );

    let hybrid = [physical_sync, remote_arranged_with_old_value];
    assert_eq!(
        judge(CatalogModality::Hybrid, &hybrid, UserSynchronicityV3::Async,),
        MatchOutcome::Match,
        "the physical SYNC component is outside FLT-S04b and remote by-arrangement is ASYNC"
    );
    assert_eq!(
        judge(CatalogModality::Hybrid, &hybrid, UserSynchronicityV3::Mixed,),
        MatchOutcome::NoMatch
    );

    let online_mixed = [remote_arranged_with_old_value, remote_sync];
    assert_eq!(
        judge(
            CatalogModality::Online,
            &online_mixed,
            UserSynchronicityV3::Mixed,
        ),
        MatchOutcome::Match,
        "remote by-arrangement plus remote scheduled content proves online MIXED"
    );
    assert_eq!(
        judge(
            CatalogModality::Online,
            &online_mixed,
            UserSynchronicityV3::Async,
        ),
        MatchOutcome::NoMatch
    );

    // An explicit 92-style SYNC value outranks the ByArrangement fallback.
    // The kind only repairs an otherwise inconclusive stored timing value.
    assert_eq!(
        judge(
            CatalogModality::Online,
            &[remote_arranged_with_explicit_sync],
            UserSynchronicityV3::Sync,
        ),
        MatchOutcome::Match
    );
    assert_eq!(
        judge(
            CatalogModality::Online,
            &[remote_arranged_with_explicit_sync],
            UserSynchronicityV3::Async,
        ),
        MatchOutcome::NoMatch
    );
}

#[test]
fn synchronicity_uses_section_facts_only_when_the_occurrence_subset_is_unavailable() {
    let judge = |modality: CatalogModality, actual: CatalogSynchronicity| {
        let mut catalog = empty_catalog("NB", 1);
        let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
        let section = section_mut(&mut catalog, &keys.section);
        section.delivery_modality = modality;
        section.synchronicity = actual;
        let mut input = neutral_input();
        input.synchronicities = vec![UserSynchronicityV3::Async];
        let filters = normalized(input);
        evaluate_section_filters(
            &catalog.sections[0],
            None,
            &open_evidence(&keys.section, LiveOpenStateV1::Open).evidence,
            now(),
            &filters,
        )
        .0
        .outcome()
    };

    assert_eq!(
        judge(
            CatalogModality::OnCampusOrInPerson,
            CatalogSynchronicity::Sync,
        ),
        MatchOutcome::Match
    );
    assert_eq!(
        judge(CatalogModality::Online, CatalogSynchronicity::Async),
        MatchOutcome::Match
    );
    assert_eq!(
        judge(CatalogModality::Online, CatalogSynchronicity::Sync),
        MatchOutcome::NoMatch
    );
    assert_eq!(
        judge(CatalogModality::Hybrid, CatalogSynchronicity::Mixed),
        MatchOutcome::Uncertain,
        "a whole-Hybrid aggregate cannot identify the online subset"
    );
}

#[test]
fn incomplete_synchronicity_never_admits_a_category_disproved_by_known_members() {
    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    let mut unknown = catalog.occurrences[0].clone();
    unknown.key.ordinal = 1;
    unknown.synchronicity = CatalogFieldKnowledge::present(CatalogSynchronicity::Unspecified);
    catalog.occurrences.push(unknown);
    let occurrence_keys = catalog
        .occurrences
        .iter()
        .map(|occurrence| occurrence.key.clone())
        .collect();
    let section = section_mut(&mut catalog, &keys.section);
    section.synchronicity = CatalogSynchronicity::Unknown;
    section.occurrence_keys = CatalogFieldKnowledge::present(occurrence_keys);
    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();
    let search = |selected: Vec<UserSynchronicityV3>, include_incomplete: bool| {
        let mut input = neutral_input();
        input.synchronicities = selected;
        input.include_incomplete.synchronicity = include_incomplete;
        engine
            .course_search(
                &course_request(input, PageRequestV1::default(), CourseSortV1::default()),
                None,
            )
            .unwrap()
            .page
            .total
    };

    assert_eq!(
        search(vec![UserSynchronicityV3::Async], true),
        0,
        "includeIncomplete cannot override a known SYNC/ASYNC-only contradiction"
    );
    assert_eq!(search(vec![UserSynchronicityV3::Sync], false), 0);
    assert_eq!(search(vec![UserSynchronicityV3::Sync], true), 1);
    assert_eq!(
        search(
            vec![
                UserSynchronicityV3::Sync,
                UserSynchronicityV3::Async,
                UserSynchronicityV3::Mixed,
            ],
            false,
        ),
        0,
        "selecting every known category does not turn incomplete evidence into MATCH"
    );
    assert_eq!(
        search(
            vec![
                UserSynchronicityV3::Sync,
                UserSynchronicityV3::Async,
                UserSynchronicityV3::Mixed,
            ],
            true,
        ),
        1
    );
}

#[test]
fn availability_windows_exclude_scheduled_meetings_of_unknown_requiredness() {
    fn outcome(
        catalog: &bcsp_contracts::NormalizedCatalogV1,
        section_key: &bcsp_contracts::SectionKey,
        windows: Vec<AvailabilityWindowV1>,
    ) -> MatchOutcome {
        let section = catalog.sections[0].clone();
        let open = open_evidence(section_key, LiveOpenStateV1::Open);
        let mut input = neutral_input();
        input.availability = windows;
        let filters = normalized(input);
        let occurrences = catalog.occurrences.iter().collect::<Vec<_>>();
        let (evaluation, _) = evaluate_section_filters(
            &section,
            Some(&occurrences),
            &open.evidence,
            now(),
            &filters,
        );
        evaluation.outcome()
    }
    let monday = || AvailabilityWindowV1::try_new(WeekdayV1::Monday, 540, 600).unwrap();
    let tuesday = || AvailabilityWindowV1::try_new(WeekdayV1::Tuesday, 540, 600).unwrap();

    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    // The fixture meets Monday 09:00-10:00; the real feed never states
    // requiredness, so this is the shape every stored occurrence has.
    occurrence_mut(&mut catalog, &keys.section).requiredness =
        CatalogRequiredness::UnknownRequiredness;
    assert_eq!(
        outcome(&catalog, &keys.section, vec![monday()]),
        MatchOutcome::Match
    );
    assert_eq!(
        outcome(&catalog, &keys.section, vec![tuesday()]),
        MatchOutcome::NoMatch,
        "a scheduled meeting of unknown requiredness outside every window excludes"
    );

    occurrence_mut(&mut catalog, &keys.section).requiredness = CatalogRequiredness::Optional;
    assert_eq!(
        outcome(&catalog, &keys.section, vec![tuesday()]),
        MatchOutcome::Match,
        "an explicitly optional meeting is not held against the section"
    );

    let occurrence = occurrence_mut(&mut catalog, &keys.section);
    occurrence.requiredness = CatalogRequiredness::UnknownRequiredness;
    occurrence.time = bcsp_contracts::CatalogTimeKnowledgeV1::Empty;
    assert_eq!(
        outcome(&catalog, &keys.section, vec![tuesday()]),
        MatchOutcome::Uncertain,
        "a meeting without usable times stays uncertain"
    );
}

#[test]
fn a_selected_location_excludes_the_sections_that_also_meet_elsewhere() {
    // Choosing BUSCH says where the student can be, so a Section that also
    // meets on another campus is excluded; an online component asks for no
    // travel and never decides the answer.
    let judge = |occurrences: &[(&str, CatalogModality, CatalogOccurrenceEvidence)]| {
        let mut catalog = empty_catalog("NB", 1);
        let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
        let template = catalog.occurrences[0].clone();
        catalog.occurrences = occurrences
            .iter()
            .enumerate()
            .map(|(ordinal, (campus, modality, evidence))| {
                let mut occurrence = template.clone();
                occurrence.key.ordinal = ordinal as u32;
                occurrence.campus = CatalogFieldKnowledge::present((*campus).to_owned());
                occurrence.campus_name = CatalogFieldKnowledge::present((*campus).to_owned());
                occurrence.modality = CatalogFieldKnowledge::present(*modality);
                occurrence.evidence = *evidence;
                occurrence
            })
            .collect();
        let has_unknown_applicability = occurrences
            .iter()
            .any(|(_, _, evidence)| *evidence == CatalogOccurrenceEvidence::None);
        let has_physical = occurrences
            .iter()
            .any(|(_, _, evidence)| *evidence == CatalogOccurrenceEvidence::Physical);
        let has_remote = occurrences
            .iter()
            .any(|(_, _, evidence)| *evidence == CatalogOccurrenceEvidence::Remote);
        section_mut(&mut catalog, &keys.section).delivery_modality = if has_unknown_applicability {
            CatalogModality::Unknown
        } else if has_physical && has_remote {
            CatalogModality::Hybrid
        } else if has_physical {
            CatalogModality::OnCampusOrInPerson
        } else {
            CatalogModality::Online
        };
        let mut input = neutral_input();
        input.meeting_locations = MeetingLocationFilterV2 {
            locations: vec![token("BUSCH")],
            mode: MeetingLocationMatchModeV2::AnyMeeting,
        };
        let filters = normalized(input);
        let refs = catalog.occurrences.iter().collect::<Vec<_>>();
        let (evaluation, _) = evaluate_section_filters(
            &catalog.sections[0],
            Some(&refs),
            &open_evidence(&keys.section, LiveOpenStateV1::Open).evidence,
            now(),
            &filters,
        );
        evaluation.outcome()
    };

    let in_person = CatalogModality::OnCampusOrInPerson;
    let physical = CatalogOccurrenceEvidence::Physical;
    let remote = CatalogOccurrenceEvidence::Remote;
    assert_eq!(
        judge(&[("BUSCH", in_person, physical)]),
        MatchOutcome::Match
    );
    // The case the reader cares about: one meeting drags them to Newark.
    assert_eq!(
        judge(&[
            ("BUSCH", in_person, physical),
            ("NEWARK", in_person, physical),
        ]),
        MatchOutcome::NoMatch
    );
    // An online component travels nowhere, so it does not spoil the match.
    assert_eq!(
        judge(&[
            ("BUSCH", in_person, physical),
            ("ONLINE", CatalogModality::Online, remote),
        ]),
        MatchOutcome::Match
    );
    // With nothing to travel to, there is no applicable location constraint.
    assert_eq!(
        judge(&[("ONLINE", CatalogModality::Online, remote)]),
        MatchOutcome::Match
    );
    // Explicit remote evidence remains non-travelling even when the raw
    // location produced an occurrence-level modality conflict.
    assert_eq!(
        judge(&[(
            "NEWARK",
            CatalogModality::UnknownConflict,
            CatalogOccurrenceEvidence::Remote,
        )]),
        MatchOutcome::Match
    );
    // With neither remote nor physical evidence, applicability is unknown;
    // the location must not fabricate a confident mismatch from the code.
    assert_eq!(
        judge(&[(
            "NEWARK",
            CatalogModality::Unknown,
            CatalogOccurrenceEvidence::None,
        )]),
        MatchOutcome::Uncertain
    );
    assert_eq!(
        judge(&[
            ("BUSCH", in_person, physical),
            (
                "NEWARK",
                CatalogModality::Unknown,
                CatalogOccurrenceEvidence::None,
            ),
        ]),
        MatchOutcome::Uncertain,
        "a matching physical component cannot erase unknown applicability"
    );
    assert_eq!(
        judge(&[
            ("NEWARK", in_person, physical),
            (
                "BUSCH",
                CatalogModality::Unknown,
                CatalogOccurrenceEvidence::None,
            ),
        ]),
        MatchOutcome::NoMatch,
        "a known physical mismatch remains decisive alongside uncertainty"
    );
    // The stored mode no longer selects a behaviour.
    assert_eq!(
        judge(&[("BUSCH", in_person, physical)]),
        MatchOutcome::Match
    );
}

#[test]
fn all_required_meeting_locations_treat_unknown_requiredness_as_required() {
    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    section_mut(&mut catalog, &keys.section).delivery_modality =
        CatalogModality::OnCampusOrInPerson;
    let occurrence = occurrence_mut(&mut catalog, &keys.section);
    occurrence.modality = CatalogFieldKnowledge::present(CatalogModality::OnCampusOrInPerson);
    occurrence.evidence = CatalogOccurrenceEvidence::Physical;
    let section = catalog.sections[0].clone();
    let open = open_evidence(&keys.section, LiveOpenStateV1::Open);
    let mut input = neutral_input();
    input.meeting_locations = MeetingLocationFilterV2 {
        locations: vec![token("BUSCH")],
        mode: MeetingLocationMatchModeV2::AllRequiredMeetings,
    };
    let filters = normalized(input);

    let occurrences = catalog.occurrences.iter().collect::<Vec<_>>();
    let (matched, _) = evaluate_section_filters(
        &section,
        Some(&occurrences),
        &open.evidence,
        now(),
        &filters,
    );
    assert_eq!(matched.outcome(), MatchOutcome::Match);

    occurrence_mut(&mut catalog, &keys.section).campus =
        CatalogFieldKnowledge::present("NEWARK".to_owned());
    occurrence_mut(&mut catalog, &keys.section).campus_name =
        CatalogFieldKnowledge::present("Newark".to_owned());
    let occurrences = catalog.occurrences.iter().collect::<Vec<_>>();
    let (no_match, _) = evaluate_section_filters(
        &section,
        Some(&occurrences),
        &open.evidence,
        now(),
        &filters,
    );
    assert_eq!(no_match.outcome(), MatchOutcome::NoMatch);

    // The feed never states requiredness, so an unknown-requiredness meeting
    // is held to the location like a required one: it matches at BUSCH and
    // excludes at NEWARK, instead of turning the whole section uncertain.
    let occurrence = occurrence_mut(&mut catalog, &keys.section);
    occurrence.campus = CatalogFieldKnowledge::present("BUSCH".to_owned());
    occurrence.requiredness = CatalogRequiredness::UnknownRequiredness;
    let occurrences = catalog.occurrences.iter().collect::<Vec<_>>();
    let (unknown, _) = evaluate_section_filters(
        &section,
        Some(&occurrences),
        &open.evidence,
        now(),
        &filters,
    );
    assert_eq!(unknown.outcome(), MatchOutcome::Match);

    occurrence_mut(&mut catalog, &keys.section).campus =
        CatalogFieldKnowledge::present("NEWARK".to_owned());
    let occurrences = catalog.occurrences.iter().collect::<Vec<_>>();
    let (unknown_elsewhere, _) = evaluate_section_filters(
        &section,
        Some(&occurrences),
        &open.evidence,
        now(),
        &filters,
    );
    assert_eq!(unknown_elsewhere.outcome(), MatchOutcome::NoMatch);
    occurrence_mut(&mut catalog, &keys.section).campus =
        CatalogFieldKnowledge::present("BUSCH".to_owned());

    occurrence_mut(&mut catalog, &keys.section).requiredness = CatalogRequiredness::Optional;
    let occurrences = catalog.occurrences.iter().collect::<Vec<_>>();
    let (only_optional, _) = evaluate_section_filters(
        &section,
        Some(&occurrences),
        &open.evidence,
        now(),
        &filters,
    );
    assert_eq!(
        only_optional.outcome(),
        MatchOutcome::Match,
        "an explicitly optional meeting leaves no applicable location constraint"
    );
}
