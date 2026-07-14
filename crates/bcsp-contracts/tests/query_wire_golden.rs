use bcsp_contracts::{
    AvailabilityWindowV1, BuildingRoomFilterV1, CampusCode, CatalogSubjectCode,
    CatalogSynchronicity, CoreFilterV1, CourseQueryRequestV1, CourseQueryResponseV1,
    CourseSortFieldV1, CourseSortV1, CreditRangeV1, EligibilityFilterV1, EligibilityUnitMajorV1,
    FilterRequestV1, FilterSearchTextV1, FilterSetModeV1, FilterTokenV1, FilterValuesInputV1,
    LiveOpenStateV1, ModalityFilterV1, NormalizedFilterValuesV1, PageInfoV1, PageRequestV1,
    PermissionFilterV1, PrerequisiteFilterV1, QUERY_CONTRACT_VERSION, SectionIndex,
    SortDirectionV1, TermId, WeekdayV1, filter_schema_v1,
};
use serde::Serialize;

fn pretty<T: Serialize>(value: &T) -> String {
    format!("{}\n", serde_json::to_string_pretty(value).unwrap())
}

fn canonical_golden(expected: &str) -> String {
    expected.replace("\r\n", "\n")
}

fn token(value: &str) -> FilterTokenV1 {
    FilterTokenV1::try_from(value).unwrap()
}

fn representative_request() -> CourseQueryRequestV1 {
    let mut input = FilterValuesInputV1::for_term(TermId::try_from("T2026F").unwrap());
    input.campuses = vec![CampusCode::try_from("CAMPUS_A").unwrap()];
    input.subjects = vec![CatalogSubjectCode::try_from("198").unwrap()];
    input.text = Some(FilterSearchTextV1::try_from("data structures").unwrap());
    input.course_numbers = vec![token("01")];
    input.levels = vec![token("ug")];
    input.credits = Some(CreditRangeV1::try_new(Some(300), Some(400)).unwrap());
    input.core = CoreFilterV1 {
        codes: vec![token("cc")],
        mode: FilterSetModeV1::All,
    };
    input.prerequisite = PrerequisiteFilterV1::Has;
    input.course_locations = vec![token("nb")];
    input.section_indexes = vec![SectionIndex::try_from("12345").unwrap()];
    input.section_numbers = vec![token("01")];
    input.open_statuses = vec![LiveOpenStateV1::Open];
    input.modalities = vec![ModalityFilterV1::Online];
    input.synchronicities = vec![CatalogSynchronicity::Sync];
    input.instructors = vec![token("  Ada   Lovelace  ")];
    input.availability = vec![AvailabilityWindowV1::try_new(WeekdayV1::Monday, 540, 720).unwrap()];
    input.meeting_locations = vec![token("cac")];
    input.building_room = BuildingRoomFilterV1 {
        building_codes: vec![token("hill")],
        room_numbers: vec![token("114")],
    };
    input.exam_codes = vec![token("a")];
    input.permission = PermissionFilterV1::Required;
    input.eligibility = EligibilityFilterV1 {
        major_codes: vec![token("198")],
        minor_codes: vec![token("cs")],
        honor_program_codes: vec![token("hon")],
        unit_codes: vec![token("01")],
        unit_majors: vec![EligibilityUnitMajorV1 {
            unit_code: token("01"),
            major_code: token("198"),
        }],
    };

    CourseQueryRequestV1 {
        filters: FilterRequestV1::new(NormalizedFilterValuesV1::try_new(input).unwrap()),
        page: PageRequestV1::try_new(2, 50).unwrap(),
        sort: CourseSortV1 {
            field: CourseSortFieldV1::Relevance,
            direction: SortDirectionV1::Descending,
        },
    }
}

#[test]
fn filter_schema_golden_locks_all_twenty_two_metadata_rows() {
    let golden = include_str!("golden/filter-schema-v1.json");
    let value = filter_schema_v1();
    assert_eq!(pretty(&value), canonical_golden(golden));
    assert_eq!(
        serde_json::from_str::<bcsp_contracts::FilterSchemaV1>(golden).unwrap(),
        value
    );
}

#[test]
fn normalized_course_query_request_wire_is_exact() {
    let golden = include_str!("golden/course-query-request-v1.json");
    let value = representative_request();
    assert_eq!(pretty(&value), canonical_golden(golden));
    assert_eq!(
        serde_json::from_str::<CourseQueryRequestV1>(golden).unwrap(),
        value
    );
}

#[test]
fn additive_course_query_response_wire_has_stable_required_fields() {
    let golden = include_str!("golden/course-query-response-v1.json");
    let value = CourseQueryResponseV1 {
        contract_version: QUERY_CONTRACT_VERSION,
        page: PageInfoV1 {
            page: 1,
            page_size: 25,
            total: 0,
            total_pages: 0,
        },
        items: Vec::new(),
    };
    assert_eq!(pretty(&value), canonical_golden(golden));
    assert_eq!(
        serde_json::from_str::<CourseQueryResponseV1>(golden).unwrap(),
        value
    );
}
