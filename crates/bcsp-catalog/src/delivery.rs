use bcsp_rutgers_client::{Presence, RawMeeting};

use crate::canonical::canonical_occurrence_hash_v1;
use crate::model::{
    DayKnowledge, DeliveryModality, NormalizedOccurrence, OccurrenceEvidence, OccurrenceKind,
    OccurrenceModality, Requiredness, Synchronicity, TimeKnowledge, Weekday, WeekdaySet,
};

pub fn parse_weekdays(raw: &Presence<String>) -> DayKnowledge {
    let raw = match raw {
        Presence::Missing => return DayKnowledge::Missing,
        Presence::Null => return DayKnowledge::Null,
        Presence::Malformed(_) => return DayKnowledge::Invalid,
        Presence::Value(raw) if raw.is_empty() => return DayKnowledge::Empty,
        Presence::Value(raw) => raw,
    };

    let token = raw.trim().to_ascii_uppercase();
    if token.is_empty() {
        return DayKnowledge::Empty;
    }
    let bytes = token.as_bytes();
    let mut position = 0;
    let mut weekdays = Vec::new();
    while position < bytes.len() {
        if bytes[position..].starts_with(b"TH") {
            weekdays.push(Weekday::Thursday);
            position += 2;
            continue;
        }
        let weekday = match bytes[position] {
            b'M' => Weekday::Monday,
            b'T' => Weekday::Tuesday,
            b'W' => Weekday::Wednesday,
            b'H' => Weekday::Thursday,
            b'F' => Weekday::Friday,
            b'S' => Weekday::Saturday,
            b'U' => Weekday::Sunday,
            _ => return DayKnowledge::Invalid,
        };
        weekdays.push(weekday);
        position += 1;
    }
    DayKnowledge::Known(WeekdaySet::new(weekdays))
}

pub fn normalize_time(start: &Presence<String>, end: &Presence<String>) -> TimeKnowledge {
    match (start, end) {
        (Presence::Missing, Presence::Missing) => TimeKnowledge::Missing,
        (Presence::Null, Presence::Null) => TimeKnowledge::Null,
        (Presence::Malformed(_), _) | (_, Presence::Malformed(_)) => TimeKnowledge::Invalid,
        (Presence::Value(start), Presence::Value(end)) if start.is_empty() && end.is_empty() => {
            TimeKnowledge::Empty
        }
        (Presence::Value(start), Presence::Value(end)) if !start.is_empty() && !end.is_empty() => {
            let Some(start_minute) = parse_military(start, false) else {
                return TimeKnowledge::Invalid;
            };
            let Some(end_minute) = parse_military(end, true) else {
                return TimeKnowledge::Invalid;
            };
            if end_minute <= start_minute {
                TimeKnowledge::Invalid
            } else {
                TimeKnowledge::Known {
                    start_minute,
                    end_minute,
                }
            }
        }
        _ => TimeKnowledge::Partial,
    }
}

fn parse_military(value: &str, allow_end_of_day: bool) -> Option<u16> {
    let bytes = value.as_bytes();
    if bytes.len() != 4 || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let hour = value[..2].parse::<u16>().ok()?;
    let minute = value[2..].parse::<u16>().ok()?;
    if allow_end_of_day && hour == 24 && minute == 0 {
        return Some(1_440);
    }
    (hour < 24 && minute < 60).then_some(hour * 60 + minute)
}

pub(crate) fn normalize_occurrence(
    raw: &RawMeeting,
    source_ordinal: usize,
) -> Result<NormalizedOccurrence, serde_json::Error> {
    let canonical_facts = serde_json::to_value(raw)?;
    let raw_hash = canonical_occurrence_hash_v1(&canonical_facts)?;
    let days = parse_weekdays(&raw.meeting_day);
    let time = normalize_time(&raw.start_time_military, &raw.end_time_military);
    let code = trimmed_value(&raw.meeting_mode_code);
    let campus_location = trimmed_value(&raw.campus_location);
    let observed_code = code.is_some_and(is_frozen_observed_mode_code);
    let mode_remote = observed_code && matches!(code, Some("90" | "92" | "93"));
    let location_remote = matches!(campus_location, Some("O" | "T"));
    let location_physical = campus_location
        .is_some_and(|location| !location.is_empty() && !matches!(location, "O" | "T"));
    let remote = observed_code && (mode_remote || location_remote);
    // This reproduces the frozen P3 section evidence calculation. A remote
    // mode paired with a physical location is retained separately below as an
    // occurrence-level conflict instead of being erased.
    let physical = observed_code && location_physical && !mode_remote;
    let evidence = if remote {
        OccurrenceEvidence::Remote
    } else if physical {
        OccurrenceEvidence::Physical
    } else {
        OccurrenceEvidence::None
    };
    let by_arrangement = matches!(trimmed_value(&raw.ba_class_hours), Some("B"))
        && matches!(campus_location, Some("O" | "T"))
        && matches!(
            time,
            TimeKnowledge::Empty | TimeKnowledge::Missing | TimeKnowledge::Null
        );
    let kind = if by_arrangement {
        OccurrenceKind::ByArrangement
    } else if matches!(time, TimeKnowledge::Known { .. }) {
        OccurrenceKind::Scheduled
    } else {
        OccurrenceKind::Unspecified
    };
    let synchronicity = match code {
        Some("92") => Synchronicity::Synchronous,
        Some("93") => Synchronicity::Asynchronous,
        Some("90") => Synchronicity::Unspecified,
        _ if observed_code && matches!(time, TimeKnowledge::Known { .. }) => {
            Synchronicity::Synchronous
        }
        _ if observed_code && by_arrangement => Synchronicity::Unspecified,
        _ => Synchronicity::Unknown,
    };
    let modality = if !observed_code {
        OccurrenceModality::Unknown
    } else if mode_remote && location_physical {
        OccurrenceModality::UnknownConflict
    } else {
        match code {
            Some("91") => OccurrenceModality::Hybrid,
            Some("90" | "92" | "93") => OccurrenceModality::Online,
            _ if location_remote => OccurrenceModality::Online,
            _ if location_physical => OccurrenceModality::OnCampusOrInPerson,
            _ => OccurrenceModality::Unknown,
        }
    };
    let normalization_reason = match (code, time, by_arrangement, modality) {
        (_, _, _, _) if !observed_code => "UNRECOGNIZED_MODE_TUPLE",
        (_, _, _, OccurrenceModality::UnknownConflict) => "MODE_LOCATION_CONFLICT",
        (Some("90"), _, _, _) => "GENERIC_ONLINE_UNSPECIFIED",
        (Some("91"), _, _, _) => "EXPLICIT_HYBRID",
        (Some("92"), _, _, _) => "EXPLICIT_REMOTE_SYNCHRONOUS",
        (Some("93"), _, _, _) => "EXPLICIT_REMOTE_ASYNCHRONOUS",
        (_, _, true, _) => "OFFICIAL_BY_ARRANGEMENT",
        (_, TimeKnowledge::Known { .. }, false, _) => "SCHEDULED_DAY_TIME",
        (_, TimeKnowledge::Partial, false, _) => "PARTIAL_TIME",
        (_, TimeKnowledge::Invalid, false, _) => "INVALID_TIME",
        _ => "INSUFFICIENT_OCCURRENCE_EVIDENCE",
    };

    Ok(NormalizedOccurrence {
        source_ordinal,
        raw_hash,
        raw_day: raw.meeting_day.clone(),
        days,
        raw_start_time_military: raw.start_time_military.clone(),
        raw_end_time_military: raw.end_time_military.clone(),
        time,
        requiredness: Requiredness::UnknownRequiredness,
        kind,
        raw_mode_code: raw.meeting_mode_code.clone(),
        raw_mode_description: raw.meeting_mode_desc.clone(),
        modality,
        synchronicity,
        evidence,
        raw_campus_location: raw.campus_location.clone(),
        raw_building_code: raw.building_code.clone(),
        raw_room_number: raw.room_number.clone(),
        normalization_reason,
        canonical_facts,
    })
}

pub(crate) fn classify_delivery(
    section_type: &Presence<String>,
    occurrences: &[NormalizedOccurrence],
) -> (DeliveryModality, Synchronicity) {
    // The frozen P3 section oracle is deliberately evidence-based.  An
    // occurrence modality is a separate, more granular conclusion: for
    // example, mode 91 remains HYBRID at the occurrence layer, but does not
    // by itself rewrite a T section.  Likewise a remote mode paired with a
    // physical location remains UNKNOWN_CONFLICT on that occurrence while
    // its remote evidence can still agree with an O section.  Changing this
    // to aggregate occurrence modality enums would reclassify approved real
    // rows and break the frozen 184-row section-conflict oracle.
    let has_remote = occurrences
        .iter()
        .any(|occurrence| occurrence.evidence == OccurrenceEvidence::Remote);
    let has_physical = occurrences
        .iter()
        .any(|occurrence| occurrence.evidence == OccurrenceEvidence::Physical);
    let modality = match trimmed_value(section_type) {
        Some("T") if has_remote => DeliveryModality::UnknownConflict,
        Some("T") => DeliveryModality::OnCampusOrInPerson,
        Some("H") => DeliveryModality::Hybrid,
        Some("O") if has_physical => DeliveryModality::UnknownConflict,
        Some("O") => DeliveryModality::Online,
        _ => DeliveryModality::Unknown,
    };

    // Current occurrence requiredness is UNKNOWN_REQUIREDNESS.  Therefore a
    // heterogeneous set cannot prove that a section is MIXED: sync+async,
    // known+unspecified, and any set containing unknown all remain UNKNOWN.
    // MIXED stays representable for a future source that explicitly supplies
    // reliable evidence, but is never synthesized from today's tuples.
    let synchronicity = occurrences
        .first()
        .map(|first| first.synchronicity)
        .filter(|first| {
            occurrences
                .iter()
                .all(|occurrence| occurrence.synchronicity == *first)
        })
        .unwrap_or(Synchronicity::Unknown);
    (modality, synchronicity)
}

fn trimmed_value(value: &Presence<String>) -> Option<&str> {
    value.value().map(|value| value.trim())
}

fn is_frozen_observed_mode_code(code: &str) -> bool {
    matches!(
        code,
        "02" | "03"
            | "04"
            | "05"
            | "06"
            | "07"
            | "08"
            | "09"
            | "10"
            | "11"
            | "12"
            | "14"
            | "15"
            | "16"
            | "18"
            | "19"
            | "20"
            | "21"
            | "23"
            | "26"
            | "27"
            | "28"
            | "29"
            | "80"
            | "81"
            | "82"
            | "83"
            | "90"
            | "91"
            | "92"
            | "93"
            | "99"
    )
}

#[cfg(test)]
mod tests {
    use bcsp_rutgers_client::{Presence, RawMeeting};
    use serde_json::{Value, json};

    use super::{classify_delivery, normalize_occurrence, normalize_time, parse_weekdays};
    use crate::{
        DayKnowledge, DeliveryModality, NormalizedOccurrence, OccurrenceEvidence,
        OccurrenceModality, Synchronicity, TimeKnowledge, Weekday,
    };

    fn occurrence(value: Value, source_ordinal: usize) -> NormalizedOccurrence {
        let raw: RawMeeting = serde_json::from_value(value).expect("valid synthetic raw meeting");
        normalize_occurrence(&raw, source_ordinal).expect("synthetic meeting should normalize")
    }

    fn section_type(value: &str) -> Presence<String> {
        Presence::Value(value.to_owned())
    }

    #[test]
    fn longest_token_weekday_parser_supports_rutgers_thursday_forms() {
        let cases = [
            ("H", vec![Weekday::Thursday]),
            ("TH", vec![Weekday::Thursday]),
            ("MTH", vec![Weekday::Monday, Weekday::Thursday]),
            ("TTH", vec![Weekday::Tuesday, Weekday::Thursday]),
        ];
        for (raw, expected) in cases {
            let DayKnowledge::Known(actual) = parse_weekdays(&Presence::Value(raw.to_owned()))
            else {
                panic!("{raw} should be a known weekday token");
            };
            assert_eq!(actual.as_slice(), expected);
        }
    }

    #[test]
    fn time_knowledge_never_promotes_partial_or_invalid_ranges() {
        assert_eq!(
            normalize_time(&Presence::Missing, &Presence::Missing),
            TimeKnowledge::Missing
        );
        assert_eq!(
            normalize_time(&Presence::Null, &Presence::Null),
            TimeKnowledge::Null
        );
        assert_eq!(
            normalize_time(
                &Presence::Value(String::new()),
                &Presence::Value(String::new())
            ),
            TimeKnowledge::Empty
        );
        assert_eq!(
            normalize_time(&Presence::Value("0900".into()), &Presence::Missing),
            TimeKnowledge::Partial
        );
        assert_eq!(
            normalize_time(
                &Presence::Value("1260".into()),
                &Presence::Value("1300".into())
            ),
            TimeKnowledge::Invalid
        );
        assert_eq!(
            normalize_time(
                &Presence::Value("1300".into()),
                &Presence::Value("1200".into())
            ),
            TimeKnowledge::Invalid
        );
        assert_eq!(
            normalize_time(
                &Presence::Value("0000".into()),
                &Presence::Value("2400".into())
            ),
            TimeKnowledge::Known {
                start_minute: 0,
                end_minute: 1_440,
            }
        );
        assert_eq!(
            normalize_time(
                &Presence::Value("2359".into()),
                &Presence::Value("2400".into())
            ),
            TimeKnowledge::Known {
                start_minute: 23 * 60 + 59,
                end_minute: 1_440,
            }
        );
        assert_eq!(
            normalize_time(
                &Presence::Value("2400".into()),
                &Presence::Value("2400".into())
            ),
            TimeKnowledge::Invalid
        );
        assert_eq!(
            normalize_time(
                &Presence::Value("2359".into()),
                &Presence::Value("2401".into())
            ),
            TimeKnowledge::Invalid
        );
    }

    #[test]
    fn section_modality_uses_the_frozen_evidence_oracle_without_erasing_occurrences() {
        let physical = occurrence(
            json!({
                "meetingModeCode": "02",
                "meetingDay": "M",
                "startTimeMilitary": "0900",
                "endTimeMilitary": "1000",
                "campusLocation": "SYN"
            }),
            0,
        );
        let remote = occurrence(
            json!({
                "meetingModeCode": "90",
                "campusLocation": "O"
            }),
            1,
        );
        let hybrid = occurrence(
            json!({
                "meetingModeCode": "91",
                "meetingDay": "T",
                "startTimeMilitary": "1000",
                "endTimeMilitary": "1100",
                "campusLocation": "SYN"
            }),
            2,
        );
        let occurrence_conflict = occurrence(
            json!({
                "meetingModeCode": "92",
                "meetingDay": "W",
                "startTimeMilitary": "1200",
                "endTimeMilitary": "1300",
                "campusLocation": "SYN"
            }),
            3,
        );
        let unknown = occurrence(
            json!({
                "meetingModeCode": "NEW",
                "meetingDay": "H",
                "startTimeMilitary": "1400",
                "endTimeMilitary": "1500",
                "campusLocation": "O"
            }),
            4,
        );

        assert_eq!(physical.evidence, OccurrenceEvidence::Physical);
        assert_eq!(physical.modality, OccurrenceModality::OnCampusOrInPerson);
        assert_eq!(remote.evidence, OccurrenceEvidence::Remote);
        assert_eq!(remote.modality, OccurrenceModality::Online);
        assert_eq!(hybrid.evidence, OccurrenceEvidence::Physical);
        assert_eq!(hybrid.modality, OccurrenceModality::Hybrid);
        assert_eq!(occurrence_conflict.evidence, OccurrenceEvidence::Remote);
        assert_eq!(
            occurrence_conflict.modality,
            OccurrenceModality::UnknownConflict
        );
        assert_eq!(unknown.evidence, OccurrenceEvidence::None);
        assert_eq!(unknown.modality, OccurrenceModality::Unknown);

        assert_eq!(
            classify_delivery(&section_type("T"), std::slice::from_ref(&physical)).0,
            DeliveryModality::OnCampusOrInPerson
        );
        assert_eq!(
            classify_delivery(&section_type("T"), std::slice::from_ref(&remote)).0,
            DeliveryModality::UnknownConflict
        );
        assert_eq!(
            classify_delivery(&section_type("O"), std::slice::from_ref(&remote)).0,
            DeliveryModality::Online
        );
        assert_eq!(
            classify_delivery(&section_type("O"), std::slice::from_ref(&physical)).0,
            DeliveryModality::UnknownConflict
        );

        // Mode 91 is preserved as occurrence HYBRID.  The approved section
        // oracle still uses its physical/remote evidence, so a physical-only
        // 91 occurrence does not invent a new T-section conflict.
        assert_eq!(
            classify_delivery(&section_type("T"), std::slice::from_ref(&hybrid)).0,
            DeliveryModality::OnCampusOrInPerson
        );
        assert_eq!(
            classify_delivery(&section_type("O"), std::slice::from_ref(&hybrid)).0,
            DeliveryModality::UnknownConflict
        );

        // The mode/location contradiction remains visible on the occurrence.
        // Its remote evidence agrees with O but conflicts with T.
        assert_eq!(
            classify_delivery(
                &section_type("O"),
                std::slice::from_ref(&occurrence_conflict)
            )
            .0,
            DeliveryModality::Online
        );
        assert_eq!(
            classify_delivery(
                &section_type("T"),
                std::slice::from_ref(&occurrence_conflict)
            )
            .0,
            DeliveryModality::UnknownConflict
        );
        assert_eq!(
            classify_delivery(
                &section_type("H"),
                std::slice::from_ref(&occurrence_conflict)
            )
            .0,
            DeliveryModality::Hybrid
        );

        // An unrecognized tuple is retained as unknown occurrence evidence;
        // it neither invents nor contradicts a recognized section base type.
        assert_eq!(
            classify_delivery(&section_type("T"), std::slice::from_ref(&unknown)).0,
            DeliveryModality::OnCampusOrInPerson
        );
        assert_eq!(
            classify_delivery(&section_type("O"), std::slice::from_ref(&unknown)).0,
            DeliveryModality::Online
        );

        assert_eq!(
            classify_delivery(&section_type("H"), &[physical, remote, hybrid]).0,
            DeliveryModality::Hybrid
        );
        assert_eq!(
            classify_delivery(&section_type("X"), &[occurrence_conflict, unknown]).0,
            DeliveryModality::Unknown
        );
        assert_eq!(
            classify_delivery(&Presence::Missing, &[]).0,
            DeliveryModality::Unknown
        );
    }

    #[test]
    fn section_synchronicity_requires_homogeneous_reliable_evidence() {
        let synchronous = occurrence(
            json!({
                "meetingModeCode": "92",
                "meetingDay": "M",
                "startTimeMilitary": "0900",
                "endTimeMilitary": "1000",
                "campusLocation": "O"
            }),
            0,
        );
        let asynchronous = occurrence(
            json!({
                "meetingModeCode": "93",
                "campusLocation": "O"
            }),
            1,
        );
        let unspecified = occurrence(
            json!({
                "meetingModeCode": "90",
                "campusLocation": "O"
            }),
            2,
        );
        let unknown = occurrence(
            json!({
                "meetingModeCode": "NEW",
                "meetingDay": "T",
                "startTimeMilitary": "1000",
                "endTimeMilitary": "1100"
            }),
            3,
        );
        let mut explicit_mixed = synchronous.clone();
        explicit_mixed.synchronicity = Synchronicity::Mixed;

        let aggregate = |occurrences: &[NormalizedOccurrence]| {
            classify_delivery(&section_type("O"), occurrences).1
        };
        assert_eq!(
            aggregate(std::slice::from_ref(&synchronous)),
            Synchronicity::Synchronous
        );
        assert_eq!(
            aggregate(std::slice::from_ref(&asynchronous)),
            Synchronicity::Asynchronous
        );
        assert_eq!(
            aggregate(&[synchronous.clone(), asynchronous.clone()]),
            Synchronicity::Unknown
        );
        assert_eq!(
            aggregate(std::slice::from_ref(&explicit_mixed)),
            Synchronicity::Mixed
        );
        assert_eq!(
            aggregate(std::slice::from_ref(&unspecified)),
            Synchronicity::Unspecified
        );
        assert_eq!(
            aggregate(&[synchronous.clone(), unspecified.clone()]),
            Synchronicity::Unknown
        );
        assert_eq!(
            aggregate(&[asynchronous.clone(), unspecified]),
            Synchronicity::Unknown
        );
        assert_eq!(
            aggregate(std::slice::from_ref(&unknown)),
            Synchronicity::Unknown
        );
        assert_eq!(
            aggregate(&[synchronous, unknown.clone()]),
            Synchronicity::Unknown
        );
        assert_eq!(aggregate(&[asynchronous, unknown]), Synchronicity::Unknown);
        assert_eq!(aggregate(&[]), Synchronicity::Unknown);
    }
}
