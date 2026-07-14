mod support;

use std::collections::BTreeSet;

use bcsp_contracts::{
    AvailabilityWindowV1, BuildingRoomFilterV1, CampusCode, CatalogFieldKnowledge, CatalogModality,
    CatalogSubjectCode, CatalogSynchronicity, CourseDetailRequestV1, CourseGroupKey,
    CourseQueryRequestV1, CourseSortFieldV1, CourseSortV1, CourseVariantKey, CreditRangeV1,
    EligibilityFilterV1, EligibilityUnitMajorV1, FilterFieldId, FilterRequestV1,
    FilterSearchTextV1, FilterSetModeV1, FilterTokenV1, LiveOpenEvidenceV1, LiveOpenStateV1,
    MatchOutcome, MatchReasonCode, ModalityFilterV1, PageRequestV1, PermissionFilterV1,
    PrerequisiteFilterV1, SectionDetailRequestV1, SectionQueryRequestV1, SectionSortV1,
    SortDirectionV1, WeekdayV1,
};
use bcsp_query::{
    CorpusError, QueryEngine, QueryError, TextHit, TextHitPlan, TextTargetVersion,
    evaluate_section_filters,
};
use time::Duration;

use support::*;

fn token(value: &str) -> FilterTokenV1 {
    FilterTokenV1::try_from(value).unwrap()
}

#[test]
fn all_22_active_filters_match_one_same_variant_and_section() {
    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    let mut input = neutral_input();
    input.campuses = vec![CampusCode::try_from("NB").unwrap()];
    input.subjects = vec![CatalogSubjectCode::try_from("198").unwrap()];
    input.text = Some(FilterSearchTextV1::try_from("01:198:111").unwrap());
    input.course_numbers = vec![token("111")];
    input.levels = vec![token("U")];
    input.credits = Some(CreditRangeV1::try_new(Some(300), Some(300)).unwrap());
    input.core.codes = vec![token("CC"), token("QR")];
    input.core.mode = FilterSetModeV1::All;
    input.prerequisite = PrerequisiteFilterV1::Has;
    input.course_locations = vec![token("BUSCH")];
    input.section_indexes = vec![keys.section.index().clone()];
    input.section_numbers = vec![token("01")];
    input.open_statuses = vec![LiveOpenStateV1::Open];
    input.modalities = vec![ModalityFilterV1::Online];
    input.synchronicities = vec![CatalogSynchronicity::Sync];
    input.instructors = vec![token("ADA   LOVELACE")];
    input.availability = vec![
        AvailabilityWindowV1::try_new(WeekdayV1::Monday, 540, 570).unwrap(),
        AvailabilityWindowV1::try_new(WeekdayV1::Monday, 570, 600).unwrap(),
        AvailabilityWindowV1::try_new(WeekdayV1::Monday, 540, 600).unwrap(),
    ];
    input.meeting_locations = vec![token("BUSCH")];
    input.building_room = BuildingRoomFilterV1 {
        building_codes: vec![token("SEC")],
        room_numbers: vec![token("101")],
    };
    // This intentionally matches the text field rather than the code field.
    input.exam_codes = vec![token("FINAL")];
    input.permission = PermissionFilterV1::Required;
    input.eligibility = EligibilityFilterV1 {
        major_codes: vec![token("198")],
        minor_codes: vec![token("M1")],
        honor_program_codes: vec![token("H1")],
        unit_codes: vec![token("01")],
        unit_majors: vec![EligibilityUnitMajorV1 {
            unit_code: token("01"),
            major_code: token("198"),
        }],
    };
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
    assert_eq!(variant.filter_matches.len(), 10);
    assert_eq!(variant.sections[0].filter_matches.len(), 12);
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
    assert_eq!(section_response.items[0].course_filter_matches.len(), 10);
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
    active.section_numbers = vec![token("01")];
    let active = course_request(active, PageRequestV1::default(), CourseSortV1::default());
    assert_eq!(engine.course_search(&active, None).unwrap().page.total, 0);
}

#[test]
fn variable_credit_ranges_use_inclusive_overlap_without_guessing_arranged_values() {
    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    variant_mut(&mut catalog, &keys.variant).credits =
        CatalogFieldKnowledge::present("1.5-4".to_owned());
    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();

    let mut overlap = neutral_input();
    overlap.credits = Some(CreditRangeV1::try_new(Some(400), Some(500)).unwrap());
    let request = course_request(overlap, PageRequestV1::default(), CourseSortV1::default());
    assert_eq!(engine.course_search(&request, None).unwrap().page.total, 1);

    let mut disjoint = neutral_input();
    disjoint.credits = Some(CreditRangeV1::try_new(Some(401), Some(500)).unwrap());
    let request = course_request(disjoint, PageRequestV1::default(), CourseSortV1::default());
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
    let rejected_item = group_a_item
        .variants
        .iter()
        .find(|item| item.variant.key == rejected.variant)
        .unwrap();
    assert_eq!(rejected_item.explanation.outcome(), MatchOutcome::NoMatch);
    assert_eq!(
        rejected_item.sections[0].explanation.outcome(),
        MatchOutcome::NoMatch
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
fn permission_null_and_missing_eligibility_keep_three_value_semantics() {
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

    let mut unknown_section = section;
    unknown_section.major_codes = CatalogFieldKnowledge::absent();
    let mut input = neutral_input();
    input.eligibility.major_codes = vec![token("198")];
    let (outcome, matches) = evaluate_section_filters(
        &unknown_section,
        Some(&occurrence_refs),
        &open_evidence(&keys.section, LiveOpenStateV1::Open).evidence,
        now(),
        &normalized(input),
    );
    assert_eq!(outcome.outcome(), MatchOutcome::Uncertain);
    assert_eq!(
        matches
            .iter()
            .find(|entry| entry.field_id == FilterFieldId::SectionEligibility)
            .unwrap()
            .explanation
            .outcome(),
        MatchOutcome::Uncertain
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
    input.course_numbers = vec![token("111")];
    input.levels = vec![token("U")];
    input.credits = Some(CreditRangeV1::try_new(Some(300), Some(300)).unwrap());
    input.core.codes = vec![token("CC")];
    input.prerequisite = PrerequisiteFilterV1::Has;
    input.course_locations = vec![token("BUSCH")];
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
        FilterFieldId::CourseNumber,
        FilterFieldId::CourseLevel,
        FilterFieldId::CourseCredits,
        FilterFieldId::CourseCoreCode,
        FilterFieldId::CoursePrerequisite,
        FilterFieldId::CourseLocation,
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
fn delivery_axes_and_same_occurrence_building_room_do_not_guess_or_stitch() {
    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    let other = add_section(&mut catalog, &keys.variant, "00002");
    occurrence_mut(&mut catalog, &keys.section).building =
        CatalogFieldKnowledge::present("SEC".to_owned());
    occurrence_mut(&mut catalog, &keys.section).room =
        CatalogFieldKnowledge::present("999".to_owned());
    occurrence_mut(&mut catalog, &other).building =
        CatalogFieldKnowledge::present("ARC".to_owned());
    occurrence_mut(&mut catalog, &other).room = CatalogFieldKnowledge::present("101".to_owned());

    let mut input = neutral_input();
    input.building_room = BuildingRoomFilterV1 {
        building_codes: vec![token("SEC")],
        room_numbers: vec![token("101")],
    };
    let request = course_request(input, PageRequestV1::default(), CourseSortV1::default());
    let catalogs = vec![catalog];
    let engine = QueryEngine::try_new(&catalogs, now(), []).unwrap();
    assert_eq!(engine.course_search(&request, None).unwrap().page.total, 0);

    let mut catalog = empty_catalog("NB", 1);
    let keys = add_course(&mut catalog, "01:198:111", "00001", 'a');
    let section = section_mut(&mut catalog, &keys.section);
    section.delivery_modality = CatalogModality::UnknownConflict;
    section.synchronicity = CatalogSynchronicity::Unspecified;
    let mut input = neutral_input();
    input.modalities = vec![ModalityFilterV1::Online];
    input.synchronicities = vec![CatalogSynchronicity::Sync];
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
