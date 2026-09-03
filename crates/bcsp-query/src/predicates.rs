use bcsp_contracts::{
    CatalogFieldKnowledge, CatalogFieldPresence, CatalogModality, CatalogOccurrenceEvidence,
    CatalogPrerequisiteState, CatalogRequiredness, CatalogSynchronicity, CourseGroupKey,
    CreditRangeV1, FilterFieldId, FilterMatchV1, FilterSetModeV1, FilterTokenV1,
    LiveOpenEvidenceV1, LiveOpenStateV1, MatchExplanation, MatchOutcome, MatchReasonCode,
    NormalizedCourseVariantV1, NormalizedFilterValuesV1, NormalizedOccurrenceV1,
    NormalizedSectionV1, PermissionFilterV1, PrerequisiteFilterV1, UserModalityV3,
    UserSynchronicityV3, course_number_band,
};
use time::OffsetDateTime;

use crate::availability::evaluate_availability;
use crate::credits::{CreditValueParseError, parse_credit_value};
use crate::knowledge::{evaluate_known, unknown_reason_code};
use crate::text::TextHitPlan;
use crate::{PredicateEvaluation, and_all, or_active};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct EvaluatedFilters {
    pub(crate) evaluation: PredicateEvaluation,
    pub(crate) filter_matches: Vec<FilterMatchV1>,
    pub(crate) admitted: bool,
}

pub(crate) fn evaluate_course_filter_matches(
    group: &CourseGroupKey,
    variant: &NormalizedCourseVariantV1,
    filters: &NormalizedFilterValuesV1,
    text_hits: Option<&TextHitPlan>,
) -> EvaluatedFilters {
    let evaluations =
        course_filter_evaluations(group, variant, filters, text_hits).collect::<Vec<_>>();
    summarize_filter_evaluations(evaluations, filters)
}

pub fn evaluate_section_filters(
    section: &NormalizedSectionV1,
    occurrences: Option<&[&NormalizedOccurrenceV1]>,
    open: &LiveOpenEvidenceV1,
    now: OffsetDateTime,
    filters: &NormalizedFilterValuesV1,
) -> (PredicateEvaluation, Vec<FilterMatchV1>) {
    let evaluated = evaluate_section_filter_matches(section, occurrences, open, now, filters);
    (evaluated.evaluation, evaluated.filter_matches)
}

pub(crate) fn evaluate_section_filter_matches(
    section: &NormalizedSectionV1,
    occurrences: Option<&[&NormalizedOccurrenceV1]>,
    open: &LiveOpenEvidenceV1,
    now: OffsetDateTime,
    filters: &NormalizedFilterValuesV1,
) -> EvaluatedFilters {
    summarize_filter_evaluations(
        section_filter_evaluations(section, occurrences, open, now, filters),
        filters,
    )
}

pub(crate) fn course_filter_evaluations<'a>(
    group: &'a CourseGroupKey,
    variant: &'a NormalizedCourseVariantV1,
    filters: &'a NormalizedFilterValuesV1,
    text_hits: Option<&'a TextHitPlan>,
) -> impl Iterator<Item = (FilterFieldId, PredicateEvaluation)> + 'a {
    [
        (
            FilterFieldId::CourseTerm,
            bool_result(group.term() == filters.term(), FilterFieldId::CourseTerm),
        ),
        (
            FilterFieldId::CourseCampus,
            inactive_or_bool(
                filters.campuses().is_empty(),
                filters.campuses().contains(group.campus()),
                FilterFieldId::CourseCampus,
            ),
        ),
        (
            FilterFieldId::CourseSubject,
            exact_values(
                &variant.subject_code,
                filters.subjects().iter().map(|value| value.as_str()),
                FilterFieldId::CourseSubject,
            ),
        ),
        (
            FilterFieldId::CourseText,
            match filters.text() {
                None => PredicateEvaluation::matched(),
                Some(_) => bool_result(
                    text_hits.is_some_and(|plan| plan.rank(&variant.key).is_some()),
                    FilterFieldId::CourseText,
                ),
            },
        ),
        (
            FilterFieldId::CourseNumberBand,
            evaluate_course_number_band(&variant.course_number, filters.course_number_bands()),
        ),
        (
            FilterFieldId::CourseLevel,
            exact_values(
                &variant.level,
                token_values(filters.levels()),
                FilterFieldId::CourseLevel,
            ),
        ),
        (
            FilterFieldId::CourseCredits,
            evaluate_credits(&variant.credits, filters.credits()),
        ),
        (
            FilterFieldId::CourseCoreCode,
            evaluate_core(&variant.core_codes, filters),
        ),
        (
            FilterFieldId::CoursePrerequisite,
            evaluate_prerequisite(variant.prerequisite_state, filters.prerequisite()),
        ),
    ]
    .into_iter()
}

fn section_filter_evaluations(
    section: &NormalizedSectionV1,
    occurrences: Option<&[&NormalizedOccurrenceV1]>,
    open: &LiveOpenEvidenceV1,
    now: OffsetDateTime,
    filters: &NormalizedFilterValuesV1,
) -> Vec<(FilterFieldId, PredicateEvaluation)> {
    vec![
        (
            FilterFieldId::SectionIndex,
            inactive_or_bool(
                filters.section_indexes().is_empty(),
                filters.section_indexes().contains(section.key.index()),
                FilterFieldId::SectionIndex,
            ),
        ),
        (
            FilterFieldId::SectionOpenStatus,
            evaluate_open(open, now, filters.open_statuses()),
        ),
        (
            FilterFieldId::SectionModality,
            evaluate_modality(section.delivery_modality, filters.modalities()),
        ),
        (
            FilterFieldId::SectionSynchronicity,
            evaluate_synchronicity(section, occurrences, filters.synchronicities()),
        ),
        (
            FilterFieldId::SectionInstructor,
            instructor_collection(
                &section.instructors,
                filters.instructors(),
                FilterFieldId::SectionInstructor,
            ),
        ),
        (
            FilterFieldId::SectionAvailability,
            evaluate_availability(section, occurrences, filters.availability()),
        ),
        (
            FilterFieldId::SectionMeetingLocation,
            evaluate_meeting_location(section, occurrences, filters.meeting_locations()),
        ),
        (
            FilterFieldId::SectionExam,
            if filters.exam_codes().is_empty() {
                PredicateEvaluation::matched()
            } else {
                or_active([
                    exact_values(
                        &section.exam_code,
                        token_values(filters.exam_codes()),
                        FilterFieldId::SectionExam,
                    ),
                    exact_values(
                        &section.exam_code_text,
                        token_values(filters.exam_codes()),
                        FilterFieldId::SectionExam,
                    ),
                ])
            },
        ),
        (
            FilterFieldId::SectionPermission,
            evaluate_permission(&section.special_permission_add_code, filters.permission()),
        ),
    ]
}

fn evaluate_credits(
    value: &CatalogFieldKnowledge<String>,
    selected: Option<&CreditRangeV1>,
) -> PredicateEvaluation {
    let Some(selected) = selected else {
        return PredicateEvaluation::matched();
    };
    let field = FilterFieldId::CourseCredits.wire_name();
    match value {
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Present { value },
        } => match parse_credit_value(value) {
            Ok(course) => {
                let contains_minimum = selected
                    .minimum_hundredths()
                    .is_none_or(|minimum| course.minimum_hundredths() >= minimum);
                let contains_maximum = selected
                    .maximum_hundredths()
                    .is_none_or(|maximum| course.maximum_hundredths() <= maximum);
                if contains_minimum && contains_maximum {
                    PredicateEvaluation::matched()
                } else {
                    PredicateEvaluation::no_match(field)
                }
            }
            Err(CreditValueParseError::Unbounded) => {
                PredicateEvaluation::uncertain(field, MatchReasonCode::UnknownValue)
            }
            Err(CreditValueParseError::Invalid) => {
                PredicateEvaluation::uncertain(field, MatchReasonCode::InvalidValue)
            }
        },
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::ExplicitNull | CatalogFieldPresence::Absent,
        } => PredicateEvaluation::uncertain(field, MatchReasonCode::MissingReliableData),
        CatalogFieldKnowledge::Unknown { reason } => {
            PredicateEvaluation::uncertain(field, unknown_reason_code(*reason))
        }
    }
}

fn evaluate_core(
    value: &CatalogFieldKnowledge<Vec<String>>,
    filters: &NormalizedFilterValuesV1,
) -> PredicateEvaluation {
    if filters.core().codes.is_empty() {
        return PredicateEvaluation::matched();
    }
    let field = FilterFieldId::CourseCoreCode.wire_name();
    match value {
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Present { value },
        } => {
            let evaluations = filters.core().codes.iter().map(|selected| {
                bool_result(
                    value.iter().any(|actual| exact(actual, selected.as_str())),
                    FilterFieldId::CourseCoreCode,
                )
            });
            match filters.core().mode {
                FilterSetModeV1::Any => or_active(evaluations),
                FilterSetModeV1::All => and_all(evaluations),
            }
        }
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::ExplicitNull | CatalogFieldPresence::Absent,
        } => PredicateEvaluation::uncertain(field, MatchReasonCode::MissingReliableData),
        CatalogFieldKnowledge::Unknown { reason } => {
            PredicateEvaluation::uncertain(field, unknown_reason_code(*reason))
        }
    }
}

fn evaluate_prerequisite(
    actual: CatalogPrerequisiteState,
    selected: PrerequisiteFilterV1,
) -> PredicateEvaluation {
    let field = FilterFieldId::CoursePrerequisite.wire_name();
    match selected {
        PrerequisiteFilterV1::Any => PredicateEvaluation::matched(),
        _ if actual == CatalogPrerequisiteState::Unknown => {
            PredicateEvaluation::uncertain(field, MatchReasonCode::UnknownValue)
        }
        PrerequisiteFilterV1::Has => bool_result(
            actual == CatalogPrerequisiteState::Has,
            FilterFieldId::CoursePrerequisite,
        ),
        PrerequisiteFilterV1::NoneReported => bool_result(
            actual == CatalogPrerequisiteState::NoneReported,
            FilterFieldId::CoursePrerequisite,
        ),
    }
}

fn evaluate_course_number_band(
    actual: &CatalogFieldKnowledge<String>,
    selected: &[u32],
) -> PredicateEvaluation {
    if selected.is_empty() {
        return PredicateEvaluation::matched();
    }
    let field = FilterFieldId::CourseNumberBand.wire_name();
    match actual {
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Present { value },
        } => match course_number_band(value) {
            Some(band) if selected.contains(&band) => PredicateEvaluation::matched(),
            Some(_) => PredicateEvaluation::no_match(field),
            None => PredicateEvaluation::uncertain(field, MatchReasonCode::InvalidValue),
        },
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::ExplicitNull | CatalogFieldPresence::Absent,
        } => PredicateEvaluation::uncertain(field, MatchReasonCode::MissingReliableData),
        CatalogFieldKnowledge::Unknown { reason } => {
            PredicateEvaluation::uncertain(field, unknown_reason_code(*reason))
        }
    }
}

fn evaluate_open(
    evidence: &LiveOpenEvidenceV1,
    now: OffsetDateTime,
    selected: &[LiveOpenStateV1],
) -> PredicateEvaluation {
    if selected.is_empty() {
        return PredicateEvaluation::matched();
    }
    let field = FilterFieldId::SectionOpenStatus.wire_name();
    let fresh = evidence.observed_at.is_some()
        && evidence
            .fresh_until
            .is_some_and(|fresh_until| now <= fresh_until);
    if !fresh || evidence.state == LiveOpenStateV1::Unknown || evidence.uncertainty.is_some() {
        if selected.contains(&LiveOpenStateV1::Unknown) {
            return PredicateEvaluation::matched();
        }
        let code = evidence
            .uncertainty
            .filter(|code| !code.is_known_mismatch())
            .unwrap_or(MatchReasonCode::MissingReliableData);
        return PredicateEvaluation::uncertain(field, code);
    }
    if selected.contains(&evidence.state) {
        PredicateEvaluation::matched()
    } else {
        PredicateEvaluation::no_match(field)
    }
}

fn evaluate_modality(actual: CatalogModality, selected: &[UserModalityV3]) -> PredicateEvaluation {
    if selected.is_empty() {
        return PredicateEvaluation::matched();
    }
    let field = FilterFieldId::SectionModality.wire_name();
    match actual {
        CatalogModality::Other | CatalogModality::Unknown | CatalogModality::UnknownConflict => {
            PredicateEvaluation::uncertain(
                field,
                if actual == CatalogModality::UnknownConflict {
                    MatchReasonCode::ConflictingEvidence
                } else {
                    MatchReasonCode::UnknownValue
                },
            )
        }
        known => {
            let selected_value = match known {
                CatalogModality::OnCampusOrInPerson => UserModalityV3::OnCampusOrInPerson,
                CatalogModality::Hybrid => UserModalityV3::Hybrid,
                CatalogModality::Online => UserModalityV3::Online,
                CatalogModality::Other
                | CatalogModality::Unknown
                | CatalogModality::UnknownConflict => unreachable!(),
            };
            bool_result(
                selected.contains(&selected_value),
                FilterFieldId::SectionModality,
            )
        }
    }
}

fn evaluate_synchronicity(
    section: &NormalizedSectionV1,
    occurrences: Option<&[&NormalizedOccurrenceV1]>,
    selected: &[UserSynchronicityV3],
) -> PredicateEvaluation {
    if selected.is_empty() {
        return PredicateEvaluation::matched();
    }
    let field = FilterFieldId::SectionSynchronicity.wire_name();

    let Some(occurrences) = occurrences.filter(|values| !values.is_empty()) else {
        return match section.delivery_modality {
            // A wholly in-person Section has no online component for this
            // filter to constrain.
            CatalogModality::OnCampusOrInPerson => PredicateEvaluation::matched(),
            // When the whole Section is online, the stored aggregate already
            // describes exactly the applicable component set.
            CatalogModality::Online => {
                evaluate_synchronicity_value(section.synchronicity, selected)
            }
            // A HYBRID aggregate also contains in-person timing, so it cannot
            // answer an online-only filter without occurrence evidence.
            CatalogModality::Hybrid
            | CatalogModality::Other
            | CatalogModality::Unknown
            | CatalogModality::UnknownConflict => {
                PredicateEvaluation::uncertain(field, MatchReasonCode::MissingReliableData)
            }
        };
    };

    let mut has_sync = false;
    let mut has_async = false;
    let mut uncertainties = Vec::new();
    for occurrence in occurrences {
        match online_applicability(section.delivery_modality, occurrence) {
            ComponentApplicability::NotApplicable => continue,
            ComponentApplicability::Uncertain(code) => {
                uncertainties.push(PredicateEvaluation::uncertain(field, code));
            }
            ComponentApplicability::Applicable => {
                let remote_by_arrangement = occurrence.evidence
                    == CatalogOccurrenceEvidence::Remote
                    && occurrence.kind == bcsp_contracts::CatalogOccurrenceKind::ByArrangement;
                match &occurrence.synchronicity {
                    CatalogFieldKnowledge::Known {
                        presence: CatalogFieldPresence::Present { value },
                    } => match value {
                        CatalogSynchronicity::Sync => has_sync = true,
                        CatalogSynchronicity::Async => has_async = true,
                        CatalogSynchronicity::Mixed => {
                            has_sync = true;
                            has_async = true;
                        }
                        CatalogSynchronicity::ByArrangement
                        | CatalogSynchronicity::Unspecified
                        | CatalogSynchronicity::Unknown => {
                            if remote_by_arrangement {
                                // Rutgers' normalized combination proves
                                // online asynchronous content when the stored
                                // synchronicity value itself is inconclusive.
                                has_async = true;
                            } else {
                                uncertainties.push(PredicateEvaluation::uncertain(
                                    field,
                                    MatchReasonCode::UnknownValue,
                                ));
                            }
                        }
                    },
                    CatalogFieldKnowledge::Known {
                        presence: CatalogFieldPresence::ExplicitNull | CatalogFieldPresence::Absent,
                    } => {
                        if remote_by_arrangement {
                            has_async = true;
                        } else {
                            uncertainties.push(PredicateEvaluation::uncertain(
                                field,
                                MatchReasonCode::MissingReliableData,
                            ));
                        }
                    }
                    CatalogFieldKnowledge::Unknown { reason } => {
                        if remote_by_arrangement {
                            has_async = true;
                        } else {
                            uncertainties.push(PredicateEvaluation::uncertain(
                                field,
                                unknown_reason_code(*reason),
                            ));
                        }
                    }
                }
            }
        }
    }

    if !uncertainties.is_empty() {
        // Unknown members must not erase facts already proved by known online
        // components. A known SYNC member means the final category can only
        // be SYNC or MIXED, never ASYNC; the dual holds for a known ASYNC
        // member. Once both are known, MIXED is already proved. With only one
        // known kind, preserve UNCERTAIN whenever at least one selected
        // category is still possible, so includeIncomplete remains the only
        // way to admit incomplete evidence; return NO_MATCH when every
        // selected category is already impossible.
        if has_sync && has_async {
            // Both kinds are already proved. Extra unknown components cannot
            // undo their coexistence, so MIXED is a conclusive result.
            return bool_result(
                selected.contains(&UserSynchronicityV3::Mixed),
                FilterFieldId::SectionSynchronicity,
            );
        }
        let could_match = match (has_sync, has_async) {
            (true, true) => unreachable!("handled above"),
            (true, false) => selected.iter().any(|value| {
                matches!(
                    value,
                    UserSynchronicityV3::Sync | UserSynchronicityV3::Mixed
                )
            }),
            (false, true) => selected.iter().any(|value| {
                matches!(
                    value,
                    UserSynchronicityV3::Async | UserSynchronicityV3::Mixed
                )
            }),
            (false, false) => true,
        };
        return if could_match {
            and_all(uncertainties)
        } else {
            PredicateEvaluation::no_match(field)
        };
    }
    let applicable = match (has_sync, has_async) {
        (true, true) => CatalogSynchronicity::Mixed,
        (true, false) => CatalogSynchronicity::Sync,
        (false, true) => CatalogSynchronicity::Async,
        (false, false) => {
            return if section.delivery_modality == CatalogModality::OnCampusOrInPerson {
                PredicateEvaluation::matched()
            } else {
                // The Section claims an online or uncertain overall format,
                // but the complete occurrence set cannot identify its online
                // timing component.
                PredicateEvaluation::uncertain(field, MatchReasonCode::MissingReliableData)
            };
        }
    };
    evaluate_synchronicity_value(applicable, selected)
}

fn evaluate_synchronicity_value(
    actual: CatalogSynchronicity,
    selected: &[UserSynchronicityV3],
) -> PredicateEvaluation {
    let field = FilterFieldId::SectionSynchronicity.wire_name();
    match actual {
        // BY_ARRANGEMENT is a display value with no filter option behind it, so
        // it evaluates exactly as UNKNOWN did before it existed: the reader can
        // still admit these Sections through the include-incomplete switch.
        CatalogSynchronicity::Unknown
        | CatalogSynchronicity::Unspecified
        | CatalogSynchronicity::ByArrangement => {
            PredicateEvaluation::uncertain(field, MatchReasonCode::UnknownValue)
        }
        known => {
            let selected_value = match known {
                CatalogSynchronicity::Sync => UserSynchronicityV3::Sync,
                CatalogSynchronicity::Async => UserSynchronicityV3::Async,
                CatalogSynchronicity::Mixed => UserSynchronicityV3::Mixed,
                CatalogSynchronicity::ByArrangement
                | CatalogSynchronicity::Unspecified
                | CatalogSynchronicity::Unknown => unreachable!(),
            };
            bool_result(
                selected.contains(&selected_value),
                FilterFieldId::SectionSynchronicity,
            )
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ComponentApplicability {
    Applicable,
    NotApplicable,
    Uncertain(MatchReasonCode),
}

/// FLT-S04b constrains only an online component. Remote occurrence evidence
/// is sufficient even when the occurrence's location creates a modality
/// conflict; HYBRID is also applicable because it contains an online part.
fn online_applicability(
    section_modality: CatalogModality,
    occurrence: &NormalizedOccurrenceV1,
) -> ComponentApplicability {
    if occurrence.evidence == CatalogOccurrenceEvidence::Remote {
        return ComponentApplicability::Applicable;
    }
    match &occurrence.modality {
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Present { value },
        } => match value {
            CatalogModality::Online | CatalogModality::Hybrid => ComponentApplicability::Applicable,
            _ if occurrence.evidence == CatalogOccurrenceEvidence::Physical => {
                ComponentApplicability::NotApplicable
            }
            CatalogModality::OnCampusOrInPerson => ComponentApplicability::NotApplicable,
            CatalogModality::UnknownConflict => {
                ComponentApplicability::Uncertain(MatchReasonCode::ConflictingEvidence)
            }
            CatalogModality::Other | CatalogModality::Unknown => match section_modality {
                CatalogModality::OnCampusOrInPerson => ComponentApplicability::NotApplicable,
                CatalogModality::Online => ComponentApplicability::Applicable,
                CatalogModality::Hybrid
                | CatalogModality::Other
                | CatalogModality::Unknown
                | CatalogModality::UnknownConflict => {
                    ComponentApplicability::Uncertain(MatchReasonCode::UnknownValue)
                }
            },
        },
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::ExplicitNull | CatalogFieldPresence::Absent,
        } => match (occurrence.evidence, section_modality) {
            (CatalogOccurrenceEvidence::Physical, _) => ComponentApplicability::NotApplicable,
            (CatalogOccurrenceEvidence::None, CatalogModality::OnCampusOrInPerson) => {
                ComponentApplicability::NotApplicable
            }
            (CatalogOccurrenceEvidence::None, CatalogModality::Online) => {
                ComponentApplicability::Applicable
            }
            (CatalogOccurrenceEvidence::None, _) => {
                ComponentApplicability::Uncertain(MatchReasonCode::MissingReliableData)
            }
            (CatalogOccurrenceEvidence::Remote, _) => unreachable!("handled above"),
        },
        CatalogFieldKnowledge::Unknown { reason } => {
            match (occurrence.evidence, section_modality) {
                (CatalogOccurrenceEvidence::Physical, _) => ComponentApplicability::NotApplicable,
                (CatalogOccurrenceEvidence::None, CatalogModality::OnCampusOrInPerson) => {
                    ComponentApplicability::NotApplicable
                }
                (CatalogOccurrenceEvidence::None, CatalogModality::Online) => {
                    ComponentApplicability::Applicable
                }
                (CatalogOccurrenceEvidence::None, _) => {
                    ComponentApplicability::Uncertain(unknown_reason_code(*reason))
                }
                (CatalogOccurrenceEvidence::Remote, _) => unreachable!("handled above"),
            }
        }
    }
}

fn summarize_filter_evaluations(
    evaluations: Vec<(FilterFieldId, PredicateEvaluation)>,
    filters: &NormalizedFilterValuesV1,
) -> EvaluatedFilters {
    let admitted = evaluations
        .iter()
        .all(|(field, evaluation)| admit_filter_evaluation(*field, evaluation, filters));
    let evaluation = and_all(evaluations.iter().map(|(_, value)| value.clone()));
    let filter_matches = evaluations
        .into_iter()
        .map(|(field_id, evaluation)| FilterMatchV1 {
            field_id,
            explanation: evaluation.into_explanation(),
        })
        .collect();
    EvaluatedFilters {
        evaluation,
        filter_matches,
        admitted,
    }
}

fn admit_filter_evaluation(
    field: FilterFieldId,
    evaluation: &PredicateEvaluation,
    filters: &NormalizedFilterValuesV1,
) -> bool {
    match evaluation.outcome() {
        MatchOutcome::NoMatch => false,
        MatchOutcome::Match => true,
        MatchOutcome::Uncertain => match field {
            FilterFieldId::CoursePrerequisite
                if filters.prerequisite() != PrerequisiteFilterV1::Any =>
            {
                filters.include_incomplete().prerequisite
            }
            FilterFieldId::SectionModality if !filters.modalities().is_empty() => {
                filters.include_incomplete().modality
            }
            FilterFieldId::SectionSynchronicity if !filters.synchronicities().is_empty() => {
                filters.include_incomplete().synchronicity
            }
            _ => true,
        },
    }
}

/// FLT-S07: every meeting the student has to travel to must be in a selected
/// location.
///
/// Choosing College Avenue states where the student can be, so a Section that
/// also meets on another campus is excluded rather than admitted on the
/// strength of its one College Avenue meeting. The Rutgers feed never states
/// requiredness and lists no skippable meetings, so an occurrence of unknown
/// requiredness counts exactly like a required one; only an explicit
/// `OPTIONAL` occurrence is left out.
///
/// An online or remote meeting imposes no travel, so it is skipped instead of
/// being treated as a location the student failed to select -- the same
/// principle that exempts an asynchronous occurrence from FLT-S06. A Section
/// whose in-person meetings are all on College Avenue therefore still matches
/// when it also carries an online component.
///
/// `MeetingLocationFilterV2::mode` no longer selects between behaviours; it is
/// retained so stored filter state keeps deserializing.
fn evaluate_meeting_location(
    section: &NormalizedSectionV1,
    occurrences: Option<&[&NormalizedOccurrenceV1]>,
    selected: &bcsp_contracts::MeetingLocationFilterV2,
) -> PredicateEvaluation {
    if selected.locations.is_empty() {
        return PredicateEvaluation::matched();
    }
    let Some(occurrences) = occurrences.filter(|values| !values.is_empty()) else {
        return if section.delivery_modality == CatalogModality::Online {
            PredicateEvaluation::matched()
        } else {
            PredicateEvaluation::uncertain(
                FilterFieldId::SectionMeetingLocation.wire_name(),
                MatchReasonCode::MissingReliableData,
            )
        };
    };
    let evaluate_occurrence = |occurrence: &&NormalizedOccurrenceV1| {
        or_active([
            exact_values(
                &occurrence.campus,
                token_values(&selected.locations),
                FilterFieldId::SectionMeetingLocation,
            ),
            exact_values(
                &occurrence.campus_name,
                token_values(&selected.locations),
                FilterFieldId::SectionMeetingLocation,
            ),
        ])
    };
    and_all(occurrences.iter().filter_map(|occurrence| {
        if occurrence.requiredness == CatalogRequiredness::Optional {
            return None;
        }
        match travel_applicability(section.delivery_modality, occurrence) {
            ComponentApplicability::Applicable => Some(evaluate_occurrence(occurrence)),
            ComponentApplicability::NotApplicable => None,
            ComponentApplicability::Uncertain(code) => Some(PredicateEvaluation::uncertain(
                FilterFieldId::SectionMeetingLocation.wire_name(),
                code,
            )),
        }
    }))
}

/// Whether FLT-S07 has a physical place to constrain. The normalized evidence
/// axis is decisive: REMOTE imposes no travel, while PHYSICAL does. Only when
/// that evidence is absent do we fall back to the occurrence modality.
fn travel_applicability(
    section_modality: CatalogModality,
    occurrence: &NormalizedOccurrenceV1,
) -> ComponentApplicability {
    match occurrence.evidence {
        CatalogOccurrenceEvidence::Remote => ComponentApplicability::NotApplicable,
        CatalogOccurrenceEvidence::Physical => ComponentApplicability::Applicable,
        CatalogOccurrenceEvidence::None => match &occurrence.modality {
            CatalogFieldKnowledge::Known {
                presence: CatalogFieldPresence::Present { value },
            } => match value {
                CatalogModality::Online => ComponentApplicability::NotApplicable,
                CatalogModality::OnCampusOrInPerson | CatalogModality::Hybrid => {
                    ComponentApplicability::Applicable
                }
                CatalogModality::UnknownConflict => {
                    ComponentApplicability::Uncertain(MatchReasonCode::ConflictingEvidence)
                }
                CatalogModality::Other | CatalogModality::Unknown => match section_modality {
                    CatalogModality::Online => ComponentApplicability::NotApplicable,
                    CatalogModality::OnCampusOrInPerson => ComponentApplicability::Applicable,
                    CatalogModality::Hybrid
                    | CatalogModality::Other
                    | CatalogModality::Unknown
                    | CatalogModality::UnknownConflict => {
                        ComponentApplicability::Uncertain(MatchReasonCode::UnknownValue)
                    }
                },
            },
            CatalogFieldKnowledge::Known {
                presence: CatalogFieldPresence::ExplicitNull | CatalogFieldPresence::Absent,
            } => match section_modality {
                CatalogModality::Online => ComponentApplicability::NotApplicable,
                CatalogModality::OnCampusOrInPerson => ComponentApplicability::Applicable,
                CatalogModality::Hybrid
                | CatalogModality::Other
                | CatalogModality::Unknown
                | CatalogModality::UnknownConflict => {
                    ComponentApplicability::Uncertain(MatchReasonCode::MissingReliableData)
                }
            },
            CatalogFieldKnowledge::Unknown { .. }
                if section_modality == CatalogModality::Online =>
            {
                ComponentApplicability::NotApplicable
            }
            CatalogFieldKnowledge::Unknown { .. }
                if section_modality == CatalogModality::OnCampusOrInPerson =>
            {
                ComponentApplicability::Applicable
            }
            CatalogFieldKnowledge::Unknown { reason } => {
                ComponentApplicability::Uncertain(unknown_reason_code(*reason))
            }
        },
    }
}

fn evaluate_permission(
    value: &CatalogFieldKnowledge<String>,
    selected: PermissionFilterV1,
) -> PredicateEvaluation {
    if selected == PermissionFilterV1::Any {
        return PredicateEvaluation::matched();
    }
    let field = FilterFieldId::SectionPermission.wire_name();
    match value {
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Present { value },
        } => {
            let required = !value.trim().is_empty();
            bool_result(
                matches!(
                    (selected, required),
                    (PermissionFilterV1::Required, true) | (PermissionFilterV1::NotRequired, false)
                ),
                FilterFieldId::SectionPermission,
            )
        }
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::ExplicitNull,
        } => bool_result(
            selected == PermissionFilterV1::NotRequired,
            FilterFieldId::SectionPermission,
        ),
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Absent,
        } => PredicateEvaluation::uncertain(field, MatchReasonCode::MissingReliableData),
        CatalogFieldKnowledge::Unknown { reason } => {
            PredicateEvaluation::uncertain(field, unknown_reason_code(*reason))
        }
    }
}

fn exact_values<'a>(
    actual: &CatalogFieldKnowledge<String>,
    selected: impl IntoIterator<Item = &'a str>,
    field: FilterFieldId,
) -> PredicateEvaluation {
    let selected = selected.into_iter().collect::<Vec<_>>();
    if selected.is_empty() {
        return PredicateEvaluation::matched();
    }
    evaluate_known(actual, field.wire_name(), |actual| {
        selected.iter().any(|selected| exact(actual, selected))
    })
}

fn instructor_collection(
    actual: &CatalogFieldKnowledge<Vec<String>>,
    selected: &[FilterTokenV1],
    field: FilterFieldId,
) -> PredicateEvaluation {
    if selected.is_empty() {
        return PredicateEvaluation::matched();
    }
    evaluate_known(actual, field.wire_name(), |actual| {
        actual.iter().any(|actual| {
            let normalized = actual.split_whitespace().collect::<Vec<_>>().join(" ");
            selected
                .iter()
                .any(|selected| exact(&normalized, selected.as_str()))
        })
    })
}

fn token_values(values: &[FilterTokenV1]) -> impl Iterator<Item = &str> {
    values.iter().map(FilterTokenV1::as_str)
}

fn exact(left: &str, right: &str) -> bool {
    left == right || left.eq_ignore_ascii_case(right)
}

fn inactive_or_bool(inactive: bool, matches: bool, field: FilterFieldId) -> PredicateEvaluation {
    if inactive {
        PredicateEvaluation::matched()
    } else {
        bool_result(matches, field)
    }
}

fn bool_result(matches: bool, field: FilterFieldId) -> PredicateEvaluation {
    if matches {
        PredicateEvaluation::matched()
    } else {
        PredicateEvaluation::no_match(field.wire_name())
    }
}

pub(crate) fn section_filters_active(filters: &NormalizedFilterValuesV1) -> bool {
    !filters.section_indexes().is_empty()
        || !filters.open_statuses().is_empty()
        || !filters.modalities().is_empty()
        || !filters.synchronicities().is_empty()
        || !filters.instructors().is_empty()
        || !filters.availability().is_empty()
        || !filters.meeting_locations().locations.is_empty()
        || !filters.exam_codes().is_empty()
        || filters.permission() != PermissionFilterV1::Any
}

pub(crate) fn matched_explanation() -> MatchExplanation {
    PredicateEvaluation::matched().into_explanation()
}
