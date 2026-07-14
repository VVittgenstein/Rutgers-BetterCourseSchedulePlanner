use std::collections::BTreeSet;

use bcsp_contracts::{
    AvailabilityWindowV1, CourseQueryResponseV1, CreditRangeV1, FILTER_FIELD_COUNT, FilterFieldId,
    FilterRequestV1, FilterScopeV1, FilterSearchTextV1, FilterTokenV1, FilterValuesInputV1,
    LiveOpenStateV1, MAX_AVAILABILITY_WINDOWS, MAX_FILTER_VALUES_PER_FIELD, MAX_PAGE_SIZE,
    NormalizedFilterValuesV1, PageRequestV1, QueryContractVersion, TermId, WeekdayV1,
    filter_schema_v1,
};
use serde_json::{Value, json};

fn neutral_values_json() -> Value {
    json!({
        "term": "T2026F",
        "campuses": [],
        "subjects": [],
        "text": null,
        "courseNumbers": [],
        "levels": [],
        "credits": null,
        "core": { "codes": [], "mode": "ANY" },
        "prerequisite": "ANY",
        "courseLocations": [],
        "sectionIndexes": [],
        "sectionNumbers": [],
        "openStatuses": [],
        "modalities": [],
        "synchronicities": [],
        "instructors": [],
        "availability": [],
        "meetingLocations": [],
        "buildingRoom": { "buildingCodes": [], "roomNumbers": [] },
        "examCodes": [],
        "permission": "ANY",
        "eligibility": {
            "majorCodes": [],
            "minorCodes": [],
            "honorProgramCodes": [],
            "unitCodes": [],
            "unitMajors": []
        }
    })
}

#[test]
fn stable_filter_ids_are_exact_ordered_and_unique() {
    let actual = FilterFieldId::ALL
        .iter()
        .map(|value| serde_json::to_value(value).unwrap())
        .collect::<Vec<_>>();
    let expected = [
        "FLT-C01", "FLT-C02", "FLT-C03", "FLT-C04", "FLT-C05", "FLT-C06", "FLT-C07", "FLT-C08",
        "FLT-C09", "FLT-C10", "FLT-S01", "FLT-S02", "FLT-S03", "FLT-S04a", "FLT-S04b", "FLT-S05",
        "FLT-S06", "FLT-S07", "FLT-S08", "FLT-S09", "FLT-S10", "FLT-S11",
    ]
    .map(|value| Value::String(value.to_owned()));

    assert_eq!(actual, expected);
    assert_eq!(actual.len(), FILTER_FIELD_COUNT);
    assert_eq!(
        FilterFieldId::ALL
            .iter()
            .map(|field| field.wire_name())
            .collect::<BTreeSet<_>>()
            .len(),
        actual.len()
    );
    assert_eq!(
        FilterFieldId::ALL
            .iter()
            .filter(|field| field.scope() == FilterScopeV1::Course)
            .count(),
        10
    );
    assert_eq!(
        FilterFieldId::ALL
            .iter()
            .filter(|field| field.scope() == FilterScopeV1::Section)
            .count(),
        12
    );
}

#[test]
fn single_filter_schema_covers_every_request_field_once() {
    let schema = filter_schema_v1();
    assert_eq!(schema.contract_version, QueryContractVersion::V1);
    assert_eq!(schema.fields.len(), FILTER_FIELD_COUNT);
    assert_eq!(
        schema
            .fields
            .iter()
            .map(|field| field.stable_id)
            .collect::<Vec<_>>(),
        FilterFieldId::ALL
    );
    assert_eq!(
        schema
            .fields
            .iter()
            .map(|field| field.request_field.as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        FILTER_FIELD_COUNT
    );
    for (index, field) in schema.fields.iter().enumerate() {
        assert_eq!(usize::from(field.chip_order), index);
        assert!(!field.request_field.is_empty());
        assert!(!field.i18n_key.is_empty());
        assert!(
            !field.normalization.is_empty()
                || matches!(
                    field.stable_id,
                    FilterFieldId::CoursePrerequisite | FilterFieldId::SectionPermission
                )
        );
    }
}

#[test]
fn normalized_filter_values_sort_and_deduplicate_every_collection() {
    let mut value = neutral_values_json();
    value["campuses"] = json!(["CAMPUS_B", "CAMPUS_A", "CAMPUS_A"]);
    value["courseNumbers"] = json!(["200", "100", "100"]);
    value["openStatuses"] = json!(["UNKNOWN", "OPEN", "OPEN"]);
    value["availability"] = json!([
        {"weekday":"TUESDAY", "startMinute":600, "endMinute":660},
        {"weekday":"MONDAY", "startMinute":600, "endMinute":660},
        {"weekday":"MONDAY", "startMinute":600, "endMinute":660}
    ]);
    let parsed: NormalizedFilterValuesV1 = serde_json::from_value(value).unwrap();

    assert_eq!(
        parsed
            .campuses()
            .iter()
            .map(|value| value.as_str())
            .collect::<Vec<_>>(),
        ["CAMPUS_A", "CAMPUS_B"]
    );
    assert_eq!(
        parsed
            .course_numbers()
            .iter()
            .map(|value| value.as_str())
            .collect::<Vec<_>>(),
        ["100", "200"]
    );
    assert_eq!(
        parsed.open_statuses(),
        &[LiveOpenStateV1::Open, LiveOpenStateV1::Unknown]
    );
    assert_eq!(parsed.availability().len(), 2);
}

#[test]
fn client_filter_request_rejects_unknown_fields_at_every_boundary() {
    let request = json!({"contractVersion": 1, "values": neutral_values_json()});
    serde_json::from_value::<FilterRequestV1>(request.clone()).unwrap();

    let mut top_level = request.clone();
    top_level["future"] = json!(true);
    assert!(serde_json::from_value::<FilterRequestV1>(top_level).is_err());

    let mut nested = request;
    nested["values"]["future"] = json!(true);
    assert!(serde_json::from_value::<FilterRequestV1>(nested).is_err());
}

#[test]
fn credits_and_availability_validate_canonical_numeric_boundaries() {
    assert!(CreditRangeV1::try_new(None, None).is_err());
    assert!(CreditRangeV1::try_new(Some(301), Some(300)).is_err());
    assert_eq!(
        CreditRangeV1::try_new(Some(250), Some(400))
            .unwrap()
            .minimum_hundredths(),
        Some(250)
    );
    assert!(AvailabilityWindowV1::try_new(WeekdayV1::Monday, 600, 600).is_err());
    assert!(AvailabilityWindowV1::try_new(WeekdayV1::Monday, 1_439, 1_440).is_ok());
}

#[test]
fn page_contract_rejects_zero_and_oversized_requests() {
    assert!(PageRequestV1::try_new(0, 25).is_err());
    assert!(PageRequestV1::try_new(1, 0).is_err());
    assert!(PageRequestV1::try_new(1, MAX_PAGE_SIZE + 1).is_err());
    assert!(PageRequestV1::try_new(1, MAX_PAGE_SIZE).is_ok());
    assert!(
        serde_json::from_value::<PageRequestV1>(json!({
            "page": 1,
            "pageSize": 25,
            "future": true
        }))
        .is_err()
    );
}

#[test]
fn search_text_exactly_matches_the_fts_acceptance_boundary() {
    let thirty_two = (0..32).map(|_| "a").collect::<Vec<_>>().join(" ");
    let parsed = FilterSearchTextV1::try_from(thirty_two.as_str()).unwrap();
    assert_eq!(parsed.tokens().len(), 32);
    assert_eq!(parsed.as_str(), thirty_two);

    let thirty_three = (0..33).map(|_| "a").collect::<Vec<_>>().join(" ");
    assert!(FilterSearchTextV1::try_from(thirty_three.as_str()).is_err());
    assert!(FilterSearchTextV1::try_from("a".repeat(128)).is_ok());
    assert!(FilterSearchTextV1::try_from("a".repeat(129)).is_err());
    assert!(FilterSearchTextV1::try_from("++").is_err());
    assert!(FilterSearchTextV1::try_from("😀").is_err());
    assert_eq!(
        FilterSearchTextV1::try_from("  数据\t结构  ")
            .unwrap()
            .tokens(),
        &["数据".to_owned(), "结构".to_owned()]
    );
}

#[test]
fn normalized_constructor_enforces_collection_limits() {
    let term = TermId::try_from("T2026F").unwrap();
    let mut too_many_one_field = FilterValuesInputV1::for_term(term.clone());
    too_many_one_field.course_numbers = (0..=MAX_FILTER_VALUES_PER_FIELD)
        .map(|value| FilterTokenV1::try_from(format!("C{value}")))
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(NormalizedFilterValuesV1::try_new(too_many_one_field).is_err());

    let mut too_many_windows = FilterValuesInputV1::for_term(term);
    too_many_windows.availability = (0..=MAX_AVAILABILITY_WINDOWS)
        .map(|minute| {
            AvailabilityWindowV1::try_new(
                WeekdayV1::Monday,
                u16::try_from(minute).unwrap(),
                u16::try_from(minute + 1).unwrap(),
            )
            .unwrap()
        })
        .collect();
    assert!(NormalizedFilterValuesV1::try_new(too_many_windows).is_err());

    let mut too_many_total = FilterValuesInputV1::for_term(TermId::try_from("T2026F").unwrap());
    let values = |prefix: &str| {
        (0..90)
            .map(|value| FilterTokenV1::try_from(format!("{prefix}{value}")))
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    };
    too_many_total.course_numbers = values("C");
    too_many_total.levels = values("L");
    too_many_total.core.codes = values("K");
    too_many_total.course_locations = values("O");
    too_many_total.section_numbers = values("S");
    too_many_total.meeting_locations = values("M");
    assert!(NormalizedFilterValuesV1::try_new(too_many_total).is_err());
}

#[test]
fn server_query_response_is_additive() {
    let value = json!({
        "contractVersion": 1,
        "page": {"page": 1, "pageSize": 25, "total": 0, "totalPages": 0, "future": true},
        "items": [],
        "future": {"newServerField": true}
    });
    let response: CourseQueryResponseV1 = serde_json::from_value(value).unwrap();
    assert_eq!(response.contract_version, QueryContractVersion::V1);
    assert!(response.items.is_empty());
}
