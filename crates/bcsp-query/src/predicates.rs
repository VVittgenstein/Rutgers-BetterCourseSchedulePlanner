use bcsp_contracts::{
    CatalogFieldKnowledge, CatalogFieldPresence, CatalogModality, CatalogPrerequisiteState,
    CatalogRequiredness, CatalogSynchronicity, CourseGroupKey, CreditRangeV1, FilterFieldId,
    FilterMatchV1, FilterSetModeV1, FilterTokenV1, LiveOpenEvidenceV1, LiveOpenStateV1,
    MatchExplanation, MatchReasonCode, MeetingLocationMatchModeV2, ModalityFilterV1,
    NormalizedCourseVariantV1, NormalizedFilterValuesV1, NormalizedOccurrenceV1,
    NormalizedSectionV1, PermissionFilterV1, PrerequisiteFilterV1,
};
use time::OffsetDateTime;

use crate::availability::evaluate_availability;
use crate::credits::{CreditValueParseError, parse_credit_value};
use crate::knowledge::{evaluate_known, unknown_reason_code};
use crate::text::TextHitPlan;
use crate::{PredicateEvaluation, and_all, or_active};

pub(crate) fn evaluate_course_filter_matches(
    group: &CourseGroupKey,
    variant: &NormalizedCourseVariantV1,
    filters: &NormalizedFilterValuesV1,
    text_hits: Option<&TextHitPlan>,
) -> (PredicateEvaluation, Vec<FilterMatchV1>) {
    let evaluations =
        course_filter_evaluations(group, variant, filters, text_hits).collect::<Vec<_>>();
    let overall = and_all(evaluations.iter().map(|(_, value)| value.clone()));
    let matches = evaluations
        .into_iter()
        .map(|(field_id, evaluation)| FilterMatchV1 {
            field_id,
            explanation: evaluation.into_explanation(),
        })
        .collect();
    (overall, matches)
}

pub fn evaluate_section_filters(
    section: &NormalizedSectionV1,
    occurrences: Option<&[&NormalizedOccurrenceV1]>,
    open: &LiveOpenEvidenceV1,
    now: OffsetDateTime,
    filters: &NormalizedFilterValuesV1,
) -> (PredicateEvaluation, Vec<FilterMatchV1>) {
    let evaluations = section_filter_evaluations(section, occurrences, open, now, filters);
    let overall = and_all(evaluations.iter().map(|(_, value)| value.clone()));
    let matches = evaluations
        .into_iter()
        .map(|(field_id, evaluation)| FilterMatchV1 {
            field_id,
            explanation: evaluation.into_explanation(),
        })
        .collect();
    (overall, matches)
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
            FilterFieldId::CourseNumber,
            exact_values(
                &variant.course_number,
                token_values(filters.course_numbers()),
                FilterFieldId::CourseNumber,
            ),
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
            evaluate_synchronicity(section.synchronicity, filters.synchronicities()),
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
            evaluate_meeting_location(occurrences, filters.meeting_locations()),
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

fn evaluate_modality(
    actual: CatalogModality,
    selected: &[ModalityFilterV1],
) -> PredicateEvaluation {
    if selected.is_empty() {
        return PredicateEvaluation::matched();
    }
    let field = FilterFieldId::SectionModality.wire_name();
    match actual {
        CatalogModality::Unknown | CatalogModality::UnknownConflict => {
            if selected.contains(&ModalityFilterV1::Unknown) {
                PredicateEvaluation::matched()
            } else {
                PredicateEvaluation::uncertain(
                    field,
                    if actual == CatalogModality::UnknownConflict {
                        MatchReasonCode::ConflictingEvidence
                    } else {
                        MatchReasonCode::UnknownValue
                    },
                )
            }
        }
        known => {
            let selected_value = match known {
                CatalogModality::OnCampusOrInPerson => ModalityFilterV1::OnCampusOrInPerson,
                CatalogModality::Hybrid => ModalityFilterV1::Hybrid,
                CatalogModality::Online => ModalityFilterV1::Online,
                CatalogModality::Other => ModalityFilterV1::Other,
                CatalogModality::Unknown | CatalogModality::UnknownConflict => unreachable!(),
            };
            bool_result(
                selected.contains(&selected_value),
                FilterFieldId::SectionModality,
            )
        }
    }
}

fn evaluate_synchronicity(
    actual: CatalogSynchronicity,
    selected: &[CatalogSynchronicity],
) -> PredicateEvaluation {
    if selected.is_empty() {
        return PredicateEvaluation::matched();
    }
    let field = FilterFieldId::SectionSynchronicity.wire_name();
    match actual {
        CatalogSynchronicity::Unknown => {
            if selected.contains(&CatalogSynchronicity::Unknown) {
                PredicateEvaluation::matched()
            } else {
                PredicateEvaluation::uncertain(field, MatchReasonCode::UnknownValue)
            }
        }
        CatalogSynchronicity::Unspecified
            if !selected.contains(&CatalogSynchronicity::Unspecified) =>
        {
            PredicateEvaluation::uncertain(field, MatchReasonCode::UnknownValue)
        }
        known => bool_result(
            selected.contains(&known),
            FilterFieldId::SectionSynchronicity,
        ),
    }
}

fn evaluate_meeting_location(
    occurrences: Option<&[&NormalizedOccurrenceV1]>,
    selected: &bcsp_contracts::MeetingLocationFilterV2,
) -> PredicateEvaluation {
    if selected.locations.is_empty() {
        return PredicateEvaluation::matched();
    }
    let Some(occurrences) = occurrences.filter(|values| !values.is_empty()) else {
        return PredicateEvaluation::uncertain(
            FilterFieldId::SectionMeetingLocation.wire_name(),
            MatchReasonCode::MissingReliableData,
        );
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
    match selected.mode {
        MeetingLocationMatchModeV2::AnyMeeting => {
            or_active(occurrences.iter().map(evaluate_occurrence))
        }
        MeetingLocationMatchModeV2::AllRequiredMeetings => {
            let required = occurrences
                .iter()
                .filter_map(|occurrence| match occurrence.requiredness {
                    CatalogRequiredness::Required => Some(evaluate_occurrence(occurrence)),
                    CatalogRequiredness::Optional => None,
                    CatalogRequiredness::UnknownRequiredness => {
                        Some(PredicateEvaluation::uncertain(
                            FilterFieldId::SectionMeetingLocation.wire_name(),
                            MatchReasonCode::MissingReliableData,
                        ))
                    }
                })
                .collect::<Vec<_>>();
            if required.is_empty() {
                PredicateEvaluation::uncertain(
                    FilterFieldId::SectionMeetingLocation.wire_name(),
                    MatchReasonCode::MissingReliableData,
                )
            } else {
                and_all(required)
            }
        }
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
