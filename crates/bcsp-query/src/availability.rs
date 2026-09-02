use bcsp_contracts::{
    AvailabilityWindowV1, CatalogFieldKnowledge, CatalogFieldPresence, CatalogRequiredness,
    CatalogSynchronicity, CatalogTimeKnowledgeV1, MatchReasonCode, NormalizedOccurrenceV1,
    NormalizedSectionV1, WeekdayV1,
};

use crate::knowledge::unknown_reason_code;
use crate::{PredicateEvaluation, and_all};

const FIELD: &str = "FLT-S06";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScheduleEvidence {
    ExplicitAsync,
    Timed {
        days: [Option<WeekdayV1>; 7],
        day_count: usize,
        start: u16,
        end: u16,
    },
    Unknown(MatchReasonCode),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OccurrenceFact {
    requiredness: CatalogRequiredness,
    schedule: ScheduleEvidence,
}

/// FLT-S06: every meeting the student attends must fit one of the windows.
///
/// Rutgers lists only the meetings a student attends, and the feed states no
/// requiredness for them (every stored occurrence is
/// `UNKNOWN_REQUIREDNESS`), so an occurrence of unknown requiredness is
/// evaluated exactly like a required one: a scheduled meeting that fits no
/// window excludes the section. Only an explicit `OPTIONAL` occurrence is
/// left out, and an occurrence without usable times (TBA, missing, invalid)
/// stays uncertain rather than excluding anything.
pub(crate) fn evaluate_availability(
    section: &NormalizedSectionV1,
    occurrences: Option<&[&NormalizedOccurrenceV1]>,
    windows: &[AvailabilityWindowV1],
) -> PredicateEvaluation {
    if windows.is_empty() {
        return PredicateEvaluation::matched();
    }
    let Some(occurrences) = occurrences else {
        return if section.synchronicity == CatalogSynchronicity::Async {
            PredicateEvaluation::matched()
        } else {
            PredicateEvaluation::uncertain(FIELD, MatchReasonCode::MissingReliableData)
        };
    };
    if occurrences.is_empty() {
        return if section.synchronicity == CatalogSynchronicity::Async {
            PredicateEvaluation::matched()
        } else {
            PredicateEvaluation::uncertain(FIELD, MatchReasonCode::MissingReliableData)
        };
    }

    let facts = occurrences
        .iter()
        .map(|occurrence| OccurrenceFact {
            requiredness: occurrence.requiredness,
            schedule: schedule_evidence(occurrence),
        })
        .collect::<Vec<_>>();
    evaluate_facts(&facts, windows)
}

fn evaluate_facts(
    facts: &[OccurrenceFact],
    windows: &[AvailabilityWindowV1],
) -> PredicateEvaluation {
    and_all(facts.iter().filter_map(|fact| {
        if fact.requiredness == CatalogRequiredness::Optional {
            return None;
        }
        let evaluation = match fact.schedule {
            ScheduleEvidence::ExplicitAsync => PredicateEvaluation::matched(),
            ScheduleEvidence::Unknown(code) => PredicateEvaluation::uncertain(FIELD, code),
            ScheduleEvidence::Timed {
                days,
                day_count,
                start,
                end,
            } => {
                let fits = days[..day_count].iter().all(|day| {
                    windows.iter().any(|window| {
                        Some(window.weekday()) == *day
                            && window.start_minute() <= start
                            && end <= window.end_minute()
                    })
                });
                if fits {
                    PredicateEvaluation::matched()
                } else {
                    // Required and unknown requiredness alike: the feed does
                    // not list meetings the student can skip, so a scheduled
                    // meeting outside every window is a real exclusion.
                    PredicateEvaluation::no_match(FIELD)
                }
            }
        };
        Some(evaluation)
    }))
}

fn schedule_evidence(occurrence: &NormalizedOccurrenceV1) -> ScheduleEvidence {
    if matches!(
        occurrence.synchronicity,
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Present {
                value: CatalogSynchronicity::Async
            }
        }
    ) {
        return ScheduleEvidence::ExplicitAsync;
    }

    let (start, end) = match occurrence.time {
        CatalogTimeKnowledgeV1::Known {
            start_minute,
            end_minute,
        } => (start_minute, end_minute),
        CatalogTimeKnowledgeV1::Missing => {
            return ScheduleEvidence::Unknown(MatchReasonCode::MissingReliableData);
        }
        CatalogTimeKnowledgeV1::ExplicitNull
        | CatalogTimeKnowledgeV1::Empty
        | CatalogTimeKnowledgeV1::Partial => {
            return ScheduleEvidence::Unknown(MatchReasonCode::Tba);
        }
        CatalogTimeKnowledgeV1::Invalid => {
            return ScheduleEvidence::Unknown(MatchReasonCode::InvalidValue);
        }
    };
    match &occurrence.days {
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Present { value },
        } if !value.is_empty() && value.len() <= 7 => {
            let mut days = [None; 7];
            for (slot, day) in days.iter_mut().zip(value) {
                let Some(day) = weekday(day) else {
                    return ScheduleEvidence::Unknown(MatchReasonCode::InvalidValue);
                };
                *slot = Some(day);
            }
            ScheduleEvidence::Timed {
                days,
                day_count: value.len(),
                start,
                end,
            }
        }
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Present { .. } | CatalogFieldPresence::ExplicitNull,
        } => ScheduleEvidence::Unknown(MatchReasonCode::Tba),
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Absent,
        } => ScheduleEvidence::Unknown(MatchReasonCode::MissingReliableData),
        CatalogFieldKnowledge::Unknown { reason } => {
            ScheduleEvidence::Unknown(unknown_reason_code(*reason))
        }
    }
}

fn weekday(value: &str) -> Option<WeekdayV1> {
    match value {
        "M" => Some(WeekdayV1::Monday),
        "T" => Some(WeekdayV1::Tuesday),
        "W" => Some(WeekdayV1::Wednesday),
        "H" | "TH" => Some(WeekdayV1::Thursday),
        "F" => Some(WeekdayV1::Friday),
        "S" => Some(WeekdayV1::Saturday),
        "U" => Some(WeekdayV1::Sunday),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(day: WeekdayV1, start: u16, end: u16) -> AvailabilityWindowV1 {
        AvailabilityWindowV1::try_new(day, start, end).unwrap()
    }

    fn timed(
        requiredness: CatalogRequiredness,
        day: WeekdayV1,
        start: u16,
        end: u16,
    ) -> OccurrenceFact {
        let mut days = [None; 7];
        days[0] = Some(day);
        OccurrenceFact {
            requiredness,
            schedule: ScheduleEvidence::Timed {
                days,
                day_count: 1,
                start,
                end,
            },
        }
    }

    #[test]
    fn every_required_occurrence_must_fit_and_boundaries_are_inclusive() {
        let windows = [
            window(WeekdayV1::Monday, 540, 660),
            window(WeekdayV1::Wednesday, 600, 720),
        ];
        let facts = [
            timed(CatalogRequiredness::Required, WeekdayV1::Monday, 540, 660),
            timed(
                CatalogRequiredness::Required,
                WeekdayV1::Wednesday,
                600,
                720,
            ),
        ];
        assert_eq!(
            evaluate_facts(&facts, &windows).outcome(),
            bcsp_contracts::MatchOutcome::Match
        );

        let outside = [
            facts[0],
            timed(CatalogRequiredness::Required, WeekdayV1::Tuesday, 600, 660),
        ];
        assert_eq!(
            evaluate_facts(&outside, &windows).outcome(),
            bcsp_contracts::MatchOutcome::NoMatch
        );
    }

    #[test]
    fn unknown_requiredness_is_evaluated_like_required() {
        let windows = [window(WeekdayV1::Monday, 540, 660)];
        assert_eq!(
            evaluate_facts(
                &[timed(
                    CatalogRequiredness::UnknownRequiredness,
                    WeekdayV1::Monday,
                    540,
                    660,
                )],
                &windows,
            )
            .outcome(),
            bcsp_contracts::MatchOutcome::Match
        );
        // The Rutgers feed lists only meetings the student attends and never
        // states requiredness, so a scheduled meeting outside every window
        // excludes the section instead of leaving it uncertain.
        assert_eq!(
            evaluate_facts(
                &[timed(
                    CatalogRequiredness::UnknownRequiredness,
                    WeekdayV1::Tuesday,
                    540,
                    660,
                )],
                &windows,
            )
            .outcome(),
            bcsp_contracts::MatchOutcome::NoMatch
        );
        // One fitting and one non-fitting meeting of unknown requiredness
        // still exclude: every attended meeting has to fit.
        assert_eq!(
            evaluate_facts(
                &[
                    timed(
                        CatalogRequiredness::UnknownRequiredness,
                        WeekdayV1::Monday,
                        540,
                        600,
                    ),
                    timed(
                        CatalogRequiredness::UnknownRequiredness,
                        WeekdayV1::Monday,
                        700,
                        760,
                    ),
                ],
                &windows,
            )
            .outcome(),
            bcsp_contracts::MatchOutcome::NoMatch
        );
        // Without usable times there is nothing to exclude on: TBA stays
        // uncertain whatever the requiredness.
        assert_eq!(
            evaluate_facts(
                &[OccurrenceFact {
                    requiredness: CatalogRequiredness::UnknownRequiredness,
                    schedule: ScheduleEvidence::Unknown(MatchReasonCode::Tba),
                }],
                &windows,
            )
            .outcome(),
            bcsp_contracts::MatchOutcome::Uncertain
        );
    }

    #[test]
    fn optional_is_ignored_async_matches_and_tba_is_uncertain() {
        let windows = [window(WeekdayV1::Monday, 540, 660)];
        let facts = [
            timed(CatalogRequiredness::Optional, WeekdayV1::Tuesday, 100, 200),
            OccurrenceFact {
                requiredness: CatalogRequiredness::Required,
                schedule: ScheduleEvidence::ExplicitAsync,
            },
        ];
        assert_eq!(
            evaluate_facts(&facts, &windows).outcome(),
            bcsp_contracts::MatchOutcome::Match
        );
        let tba = [OccurrenceFact {
            requiredness: CatalogRequiredness::Required,
            schedule: ScheduleEvidence::Unknown(MatchReasonCode::Tba),
        }];
        assert_eq!(
            evaluate_facts(&tba, &windows).outcome(),
            bcsp_contracts::MatchOutcome::Uncertain
        );
    }

    #[test]
    fn adjacent_windows_cannot_be_stitched_and_invalid_time_is_uncertain() {
        let adjacent = [
            window(WeekdayV1::Monday, 540, 570),
            window(WeekdayV1::Monday, 570, 600),
        ];
        assert_eq!(
            evaluate_facts(
                &[timed(
                    CatalogRequiredness::Required,
                    WeekdayV1::Monday,
                    540,
                    600,
                )],
                &adjacent,
            )
            .outcome(),
            bcsp_contracts::MatchOutcome::NoMatch
        );
        assert_eq!(
            evaluate_facts(
                &[OccurrenceFact {
                    requiredness: CatalogRequiredness::Required,
                    schedule: ScheduleEvidence::Unknown(MatchReasonCode::InvalidValue),
                }],
                &adjacent,
            )
            .outcome(),
            bcsp_contracts::MatchOutcome::Uncertain
        );
    }

    #[test]
    fn every_day_of_one_multi_day_occurrence_needs_a_window() {
        let mut days = [None; 7];
        days[0] = Some(WeekdayV1::Monday);
        days[1] = Some(WeekdayV1::Wednesday);
        let fact = OccurrenceFact {
            requiredness: CatalogRequiredness::Required,
            schedule: ScheduleEvidence::Timed {
                days,
                day_count: 2,
                start: 540,
                end: 600,
            },
        };
        assert_eq!(
            evaluate_facts(
                &[fact],
                &[
                    window(WeekdayV1::Monday, 540, 600),
                    window(WeekdayV1::Wednesday, 540, 600),
                ],
            )
            .outcome(),
            bcsp_contracts::MatchOutcome::Match
        );
        assert_eq!(
            evaluate_facts(&[fact], &[window(WeekdayV1::Monday, 540, 600)],).outcome(),
            bcsp_contracts::MatchOutcome::NoMatch
        );
    }
}
