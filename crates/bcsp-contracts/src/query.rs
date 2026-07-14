use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use time::OffsetDateTime;

use crate::{
    CatalogSubjectCode, CatalogSynchronicity, CourseGroupKey, MatchExplanation,
    NormalizedCourseGroupV1, NormalizedCourseVariantV1, NormalizedOccurrenceV1,
    NormalizedSectionV1, SectionIndex, SectionKey, TermId,
};

pub const QUERY_CONTRACT_VERSION: QueryContractVersion = QueryContractVersion::V1;
pub const FILTER_FIELD_COUNT: usize = 22;
pub const DEFAULT_PAGE_SIZE: u16 = 25;
pub const MAX_PAGE_SIZE: u16 = 200;
pub const MAX_FILTER_VALUES_PER_FIELD: usize = 100;
pub const MAX_AVAILABILITY_WINDOWS: usize = 64;
pub const MAX_TOTAL_FILTER_VALUES: usize = 512;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum QueryContractVersion {
    V1,
}

impl QueryContractVersion {
    pub const fn as_u16(self) -> u16 {
        match self {
            Self::V1 => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("unsupported query contract version")]
pub struct QueryContractVersionError {
    observed: u16,
}

impl QueryContractVersionError {
    pub const fn observed(self) -> u16 {
        self.observed
    }
}

impl TryFrom<u16> for QueryContractVersion {
    type Error = QueryContractVersionError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::V1),
            observed => Err(QueryContractVersionError { observed }),
        }
    }
}

impl Serialize for QueryContractVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u16(self.as_u16())
    }
}

impl<'de> Deserialize<'de> for QueryContractVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(u16::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

/// Stable IDs are serialized explicitly because `S04a`/`S04b` are
/// case-sensitive public identities, not enum-style names.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub enum FilterFieldId {
    #[serde(rename = "FLT-C01")]
    CourseTerm,
    #[serde(rename = "FLT-C02")]
    CourseCampus,
    #[serde(rename = "FLT-C03")]
    CourseSubject,
    #[serde(rename = "FLT-C04")]
    CourseText,
    #[serde(rename = "FLT-C05")]
    CourseNumber,
    #[serde(rename = "FLT-C06")]
    CourseLevel,
    #[serde(rename = "FLT-C07")]
    CourseCredits,
    #[serde(rename = "FLT-C08")]
    CourseCoreCode,
    #[serde(rename = "FLT-C09")]
    CoursePrerequisite,
    #[serde(rename = "FLT-C10")]
    CourseLocation,
    #[serde(rename = "FLT-S01")]
    SectionIndex,
    #[serde(rename = "FLT-S02")]
    SectionNumber,
    #[serde(rename = "FLT-S03")]
    SectionOpenStatus,
    #[serde(rename = "FLT-S04a")]
    SectionModality,
    #[serde(rename = "FLT-S04b")]
    SectionSynchronicity,
    #[serde(rename = "FLT-S05")]
    SectionInstructor,
    #[serde(rename = "FLT-S06")]
    SectionAvailability,
    #[serde(rename = "FLT-S07")]
    SectionMeetingLocation,
    #[serde(rename = "FLT-S08")]
    SectionBuildingRoom,
    #[serde(rename = "FLT-S09")]
    SectionExam,
    #[serde(rename = "FLT-S10")]
    SectionPermission,
    #[serde(rename = "FLT-S11")]
    SectionEligibility,
}

impl FilterFieldId {
    pub const ALL: &'static [Self] = &[
        Self::CourseTerm,
        Self::CourseCampus,
        Self::CourseSubject,
        Self::CourseText,
        Self::CourseNumber,
        Self::CourseLevel,
        Self::CourseCredits,
        Self::CourseCoreCode,
        Self::CoursePrerequisite,
        Self::CourseLocation,
        Self::SectionIndex,
        Self::SectionNumber,
        Self::SectionOpenStatus,
        Self::SectionModality,
        Self::SectionSynchronicity,
        Self::SectionInstructor,
        Self::SectionAvailability,
        Self::SectionMeetingLocation,
        Self::SectionBuildingRoom,
        Self::SectionExam,
        Self::SectionPermission,
        Self::SectionEligibility,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::CourseTerm => "FLT-C01",
            Self::CourseCampus => "FLT-C02",
            Self::CourseSubject => "FLT-C03",
            Self::CourseText => "FLT-C04",
            Self::CourseNumber => "FLT-C05",
            Self::CourseLevel => "FLT-C06",
            Self::CourseCredits => "FLT-C07",
            Self::CourseCoreCode => "FLT-C08",
            Self::CoursePrerequisite => "FLT-C09",
            Self::CourseLocation => "FLT-C10",
            Self::SectionIndex => "FLT-S01",
            Self::SectionNumber => "FLT-S02",
            Self::SectionOpenStatus => "FLT-S03",
            Self::SectionModality => "FLT-S04a",
            Self::SectionSynchronicity => "FLT-S04b",
            Self::SectionInstructor => "FLT-S05",
            Self::SectionAvailability => "FLT-S06",
            Self::SectionMeetingLocation => "FLT-S07",
            Self::SectionBuildingRoom => "FLT-S08",
            Self::SectionExam => "FLT-S09",
            Self::SectionPermission => "FLT-S10",
            Self::SectionEligibility => "FLT-S11",
        }
    }

    pub const fn scope(self) -> FilterScopeV1 {
        match self {
            Self::CourseTerm
            | Self::CourseCampus
            | Self::CourseSubject
            | Self::CourseText
            | Self::CourseNumber
            | Self::CourseLevel
            | Self::CourseCredits
            | Self::CourseCoreCode
            | Self::CoursePrerequisite
            | Self::CourseLocation => FilterScopeV1::Course,
            _ => FilterScopeV1::Section,
        }
    }
}

impl fmt::Display for FilterFieldId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.wire_name())
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FilterScopeV1 {
    Course,
    Section,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FilterValueKindV1 {
    TermId,
    CampusCodeSet,
    SubjectCodeSet,
    TextQuery,
    CourseNumberSet,
    LevelSet,
    CreditRange,
    CoreCodeSet,
    PrerequisitePresence,
    CourseLocationSet,
    SectionIndexSet,
    SectionNumberSet,
    OpenStatusSet,
    ModalitySet,
    SynchronicitySet,
    InstructorNameSet,
    AvailabilityWindows,
    MeetingLocationSet,
    BuildingRoom,
    ExamCodeSet,
    PermissionRequirement,
    Eligibility,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FilterSchemaValueV1 {
    Required,
    EmptySet,
    EmptyText,
    UnboundedRange,
    Any,
    EmptyWindows,
    EmptyComposite,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FilterNormalizationV1 {
    CanonicalIdentity,
    Trim,
    TrimAndCollapseWhitespace,
    AsciiUppercase,
    SortDeduplicate,
    CreditHundredths,
    MinuteOfDay,
    TokenAnd,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FilterValidationV1 {
    Required,
    DynamicDictionary,
    NonemptyWhenActive,
    OrderedInclusiveRange,
    OrderedMinuteInterval,
    SectionIndexIdentity,
    StructuredOnly,
    Max32TextTokens,
    Max128TokenBytes,
    TokenContainsAlphanumeric,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FilterQueryEncodingV1 {
    ExactOne,
    ExactAny,
    TextTokenAndExactIdentifierPriority,
    InclusiveRange,
    ExplicitAnyAll,
    TernaryPresence,
    SameSectionExactAny,
    SameSectionAvailabilityAll,
    SameSectionStructuredDimensions,
}

/// Machine-readable, versioned registry entry shared by query, UI chips, and
/// local Saved-view codecs. It contains no target-specific UI implementation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterFieldSchemaV1 {
    pub stable_id: FilterFieldId,
    pub request_field: String,
    pub scope: FilterScopeV1,
    pub value_kind: FilterValueKindV1,
    pub neutral: FilterSchemaValueV1,
    pub default: FilterSchemaValueV1,
    pub normalization: Vec<FilterNormalizationV1>,
    pub validation: Vec<FilterValidationV1>,
    pub query_encoding: FilterQueryEncodingV1,
    pub i18n_key: String,
    pub chip_order: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterSchemaV1 {
    pub contract_version: QueryContractVersion,
    pub fields: Vec<FilterFieldSchemaV1>,
}

pub fn filter_schema_v1() -> FilterSchemaV1 {
    FilterSchemaV1 {
        contract_version: QUERY_CONTRACT_VERSION,
        fields: FilterFieldId::ALL
            .iter()
            .copied()
            .enumerate()
            .map(|(index, id)| filter_field_schema(id, index as u8))
            .collect(),
    }
}

fn filter_field_schema(id: FilterFieldId, chip_order: u8) -> FilterFieldSchemaV1 {
    use FilterFieldId as Id;
    use FilterNormalizationV1 as N;
    use FilterQueryEncodingV1 as Q;
    use FilterSchemaValueV1 as D;
    use FilterValidationV1 as V;
    use FilterValueKindV1 as K;

    let (request_field, value_kind, neutral, default, normalization, validation, query_encoding) =
        match id {
            Id::CourseTerm => (
                "term",
                K::TermId,
                D::Required,
                D::Required,
                vec![N::CanonicalIdentity],
                vec![V::Required, V::DynamicDictionary],
                Q::ExactOne,
            ),
            Id::CourseCampus => (
                "campuses",
                K::CampusCodeSet,
                D::EmptySet,
                D::EmptySet,
                vec![N::CanonicalIdentity, N::SortDeduplicate],
                vec![V::DynamicDictionary],
                Q::ExactAny,
            ),
            Id::CourseSubject => (
                "subjects",
                K::SubjectCodeSet,
                D::EmptySet,
                D::EmptySet,
                vec![N::CanonicalIdentity, N::SortDeduplicate],
                vec![V::DynamicDictionary],
                Q::ExactAny,
            ),
            Id::CourseText => (
                "text",
                K::TextQuery,
                D::EmptyText,
                D::EmptyText,
                vec![N::TrimAndCollapseWhitespace, N::TokenAnd],
                vec![
                    V::NonemptyWhenActive,
                    V::Max32TextTokens,
                    V::Max128TokenBytes,
                    V::TokenContainsAlphanumeric,
                ],
                Q::TextTokenAndExactIdentifierPriority,
            ),
            Id::CourseNumber => (
                "courseNumbers",
                K::CourseNumberSet,
                D::EmptySet,
                D::EmptySet,
                vec![N::Trim, N::AsciiUppercase, N::SortDeduplicate],
                vec![V::NonemptyWhenActive],
                Q::ExactAny,
            ),
            Id::CourseLevel => (
                "levels",
                K::LevelSet,
                D::EmptySet,
                D::EmptySet,
                vec![N::Trim, N::AsciiUppercase, N::SortDeduplicate],
                vec![V::DynamicDictionary],
                Q::ExactAny,
            ),
            Id::CourseCredits => (
                "credits",
                K::CreditRange,
                D::UnboundedRange,
                D::UnboundedRange,
                vec![N::CreditHundredths],
                vec![V::OrderedInclusiveRange],
                Q::InclusiveRange,
            ),
            Id::CourseCoreCode => (
                "core",
                K::CoreCodeSet,
                D::EmptySet,
                D::EmptySet,
                vec![N::Trim, N::AsciiUppercase, N::SortDeduplicate],
                vec![V::DynamicDictionary],
                Q::ExplicitAnyAll,
            ),
            Id::CoursePrerequisite => (
                "prerequisite",
                K::PrerequisitePresence,
                D::Any,
                D::Any,
                vec![],
                vec![],
                Q::TernaryPresence,
            ),
            Id::CourseLocation => (
                "courseLocations",
                K::CourseLocationSet,
                D::EmptySet,
                D::EmptySet,
                vec![N::Trim, N::AsciiUppercase, N::SortDeduplicate],
                vec![V::DynamicDictionary],
                Q::ExactAny,
            ),
            Id::SectionIndex => (
                "sectionIndexes",
                K::SectionIndexSet,
                D::EmptySet,
                D::EmptySet,
                vec![N::CanonicalIdentity, N::SortDeduplicate],
                vec![V::SectionIndexIdentity],
                Q::SameSectionExactAny,
            ),
            Id::SectionNumber => (
                "sectionNumbers",
                K::SectionNumberSet,
                D::EmptySet,
                D::EmptySet,
                vec![N::Trim, N::AsciiUppercase, N::SortDeduplicate],
                vec![V::NonemptyWhenActive],
                Q::SameSectionExactAny,
            ),
            Id::SectionOpenStatus => (
                "openStatuses",
                K::OpenStatusSet,
                D::EmptySet,
                D::EmptySet,
                vec![N::SortDeduplicate],
                vec![],
                Q::SameSectionExactAny,
            ),
            Id::SectionModality => (
                "modalities",
                K::ModalitySet,
                D::EmptySet,
                D::EmptySet,
                vec![N::SortDeduplicate],
                vec![],
                Q::SameSectionExactAny,
            ),
            Id::SectionSynchronicity => (
                "synchronicities",
                K::SynchronicitySet,
                D::EmptySet,
                D::EmptySet,
                vec![N::SortDeduplicate],
                vec![],
                Q::SameSectionExactAny,
            ),
            Id::SectionInstructor => (
                "instructors",
                K::InstructorNameSet,
                D::EmptySet,
                D::EmptySet,
                vec![N::TrimAndCollapseWhitespace, N::SortDeduplicate],
                vec![V::DynamicDictionary],
                Q::SameSectionExactAny,
            ),
            Id::SectionAvailability => (
                "availability",
                K::AvailabilityWindows,
                D::EmptyWindows,
                D::EmptyWindows,
                vec![N::MinuteOfDay, N::SortDeduplicate],
                vec![V::OrderedMinuteInterval],
                Q::SameSectionAvailabilityAll,
            ),
            Id::SectionMeetingLocation => (
                "meetingLocations",
                K::MeetingLocationSet,
                D::EmptySet,
                D::EmptySet,
                vec![N::Trim, N::AsciiUppercase, N::SortDeduplicate],
                vec![V::DynamicDictionary],
                Q::SameSectionExactAny,
            ),
            Id::SectionBuildingRoom => (
                "buildingRoom",
                K::BuildingRoom,
                D::EmptyComposite,
                D::EmptyComposite,
                vec![N::Trim, N::AsciiUppercase, N::SortDeduplicate],
                vec![V::StructuredOnly],
                Q::SameSectionStructuredDimensions,
            ),
            Id::SectionExam => (
                "examCodes",
                K::ExamCodeSet,
                D::EmptySet,
                D::EmptySet,
                vec![N::Trim, N::AsciiUppercase, N::SortDeduplicate],
                vec![V::DynamicDictionary],
                Q::SameSectionExactAny,
            ),
            Id::SectionPermission => (
                "permission",
                K::PermissionRequirement,
                D::Any,
                D::Any,
                vec![],
                vec![],
                Q::TernaryPresence,
            ),
            Id::SectionEligibility => (
                "eligibility",
                K::Eligibility,
                D::EmptyComposite,
                D::EmptyComposite,
                vec![N::Trim, N::AsciiUppercase, N::SortDeduplicate],
                vec![V::StructuredOnly],
                Q::SameSectionStructuredDimensions,
            ),
        };
    FilterFieldSchemaV1 {
        stable_id: id,
        request_field: request_field.to_owned(),
        scope: id.scope(),
        value_kind,
        neutral,
        default,
        normalization,
        validation,
        query_encoding,
        i18n_key: format!("filter.{}", id.wire_name().to_ascii_lowercase()),
        chip_order,
    }
}

const FILTER_TOKEN_MAX_BYTES: usize = 256;
const FILTER_TEXT_MAX_BYTES: usize = 512;

fn validate_filter_string(value: &str, max_bytes: usize) -> bool {
    !value.is_empty()
        && value.len() <= max_bytes
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FilterTokenV1(String);

impl FilterTokenV1 {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn make_ascii_uppercase(&mut self) {
        self.0.make_ascii_uppercase();
    }

    fn collapse_whitespace(&mut self) {
        self.0 = self.0.split_whitespace().collect::<Vec<_>>().join(" ");
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("filter token must be nonempty, trim-stable, control-free UTF-8")]
pub struct FilterTokenError;

impl TryFrom<String> for FilterTokenV1 {
    type Error = FilterTokenError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let normalized = value.trim().to_owned();
        validate_filter_string(&normalized, FILTER_TOKEN_MAX_BYTES)
            .then_some(Self(normalized))
            .ok_or(FilterTokenError)
    }
}

impl TryFrom<&str> for FilterTokenV1 {
    type Error = FilterTokenError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_from(value.to_owned())
    }
}

impl Serialize for FilterTokenV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for FilterTokenV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FilterSearchTextV1 {
    text: String,
    tokens: Vec<String>,
}

impl FilterSearchTextV1 {
    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn tokens(&self) -> &[String] {
        &self.tokens
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum FilterSearchTextError {
    #[error("filter search text requires 1 to 32 tokens")]
    TokenCount,
    #[error("filter search text exceeds 512 UTF-8 bytes")]
    TextTooLong,
    #[error("a filter search token exceeds 128 UTF-8 bytes")]
    TokenTooLong,
    #[error("every filter search token must contain a Unicode alphanumeric character")]
    TokenWithoutAlphanumeric,
    #[error("filter search text must not contain non-whitespace control characters")]
    ControlCharacter,
}

impl TryFrom<String> for FilterSearchTextV1 {
    type Error = FilterSearchTextError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value
            .chars()
            .any(|character| character.is_control() && !character.is_whitespace())
        {
            return Err(FilterSearchTextError::ControlCharacter);
        }
        let tokens = value
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        if tokens.is_empty() || tokens.len() > 32 {
            return Err(FilterSearchTextError::TokenCount);
        }
        if tokens.iter().any(|token| token.len() > 128) {
            return Err(FilterSearchTextError::TokenTooLong);
        }
        if tokens
            .iter()
            .any(|token| !token.chars().any(char::is_alphanumeric))
        {
            return Err(FilterSearchTextError::TokenWithoutAlphanumeric);
        }
        let text = tokens.join(" ");
        if !validate_filter_string(&text, FILTER_TEXT_MAX_BYTES) {
            return Err(FilterSearchTextError::TextTooLong);
        }
        Ok(Self { text, tokens })
    }
}

impl TryFrom<&str> for FilterSearchTextV1 {
    type Error = FilterSearchTextError;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_from(value.to_owned())
    }
}

impl Serialize for FilterSearchTextV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.text)
    }
}

impl<'de> Deserialize<'de> for FilterSearchTextV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(String::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FilterSetModeV1 {
    Any,
    All,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PrerequisiteFilterV1 {
    Any,
    Has,
    NoneReported,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LiveOpenStateV1 {
    Open,
    Closed,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModalityFilterV1 {
    OnCampusOrInPerson,
    Online,
    Hybrid,
    Other,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PermissionFilterV1 {
    Any,
    Required,
    NotRequired,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WeekdayV1 {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum FilterValueError {
    #[error("credit range requires at least one bound")]
    EmptyCreditRange,
    #[error("credit range bounds are reversed")]
    ReversedCreditRange,
    #[error("availability window must satisfy start < end <= 1440")]
    InvalidAvailabilityWindow,
    #[error("page number and page size must be nonzero and page size at most 200")]
    InvalidPage,
    #[error("a filter field exceeds its value-count limit")]
    TooManyFieldValues,
    #[error("the normalized filter request exceeds its total value-count limit")]
    TooManyTotalValues,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreditRangeV1 {
    minimum_hundredths: Option<u32>,
    maximum_hundredths: Option<u32>,
}

impl CreditRangeV1 {
    pub fn try_new(
        minimum_hundredths: Option<u32>,
        maximum_hundredths: Option<u32>,
    ) -> Result<Self, FilterValueError> {
        if minimum_hundredths.is_none() && maximum_hundredths.is_none() {
            return Err(FilterValueError::EmptyCreditRange);
        }
        if minimum_hundredths
            .zip(maximum_hundredths)
            .is_some_and(|(minimum, maximum)| minimum > maximum)
        {
            return Err(FilterValueError::ReversedCreditRange);
        }
        Ok(Self {
            minimum_hundredths,
            maximum_hundredths,
        })
    }

    pub const fn minimum_hundredths(&self) -> Option<u32> {
        self.minimum_hundredths
    }
    pub const fn maximum_hundredths(&self) -> Option<u32> {
        self.maximum_hundredths
    }
}

impl<'de> Deserialize<'de> for CreditRangeV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Wire {
            minimum_hundredths: Option<u32>,
            maximum_hundredths: Option<u32>,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::try_new(wire.minimum_hundredths, wire.maximum_hundredths).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailabilityWindowV1 {
    weekday: WeekdayV1,
    start_minute: u16,
    end_minute: u16,
}

impl AvailabilityWindowV1 {
    pub fn try_new(
        weekday: WeekdayV1,
        start_minute: u16,
        end_minute: u16,
    ) -> Result<Self, FilterValueError> {
        if start_minute >= end_minute || end_minute > 1_440 {
            return Err(FilterValueError::InvalidAvailabilityWindow);
        }
        Ok(Self {
            weekday,
            start_minute,
            end_minute,
        })
    }
    pub const fn weekday(&self) -> WeekdayV1 {
        self.weekday
    }
    pub const fn start_minute(&self) -> u16 {
        self.start_minute
    }
    pub const fn end_minute(&self) -> u16 {
        self.end_minute
    }
}

impl<'de> Deserialize<'de> for AvailabilityWindowV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Wire {
            weekday: WeekdayV1,
            start_minute: u16,
            end_minute: u16,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::try_new(wire.weekday, wire.start_minute, wire.end_minute).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CoreFilterV1 {
    pub codes: Vec<FilterTokenV1>,
    pub mode: FilterSetModeV1,
}

impl Default for CoreFilterV1 {
    fn default() -> Self {
        Self {
            codes: Vec::new(),
            mode: FilterSetModeV1::Any,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BuildingRoomFilterV1 {
    pub building_codes: Vec<FilterTokenV1>,
    pub room_numbers: Vec<FilterTokenV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EligibilityUnitMajorV1 {
    pub unit_code: FilterTokenV1,
    pub major_code: FilterTokenV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EligibilityFilterV1 {
    pub major_codes: Vec<FilterTokenV1>,
    pub minor_codes: Vec<FilterTokenV1>,
    pub honor_program_codes: Vec<FilterTokenV1>,
    pub unit_codes: Vec<FilterTokenV1>,
    pub unit_majors: Vec<EligibilityUnitMajorV1>,
}

fn canonicalize<T: Ord>(values: &mut Vec<T>) {
    values.sort_unstable();
    values.dedup();
}

fn uppercase_tokens(values: &mut [FilterTokenV1]) {
    values
        .iter_mut()
        .for_each(FilterTokenV1::make_ascii_uppercase);
}

/// Programmatic input separated from the normalized representation. Callers
/// may fill this neutral value and must pass it through
/// [`NormalizedFilterValuesV1::try_new`] before constructing a request.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilterValuesInputV1 {
    pub term: TermId,
    pub campuses: Vec<crate::CampusCode>,
    pub subjects: Vec<CatalogSubjectCode>,
    pub text: Option<FilterSearchTextV1>,
    pub course_numbers: Vec<FilterTokenV1>,
    pub levels: Vec<FilterTokenV1>,
    pub credits: Option<CreditRangeV1>,
    pub core: CoreFilterV1,
    pub prerequisite: PrerequisiteFilterV1,
    pub course_locations: Vec<FilterTokenV1>,
    pub section_indexes: Vec<SectionIndex>,
    pub section_numbers: Vec<FilterTokenV1>,
    pub open_statuses: Vec<LiveOpenStateV1>,
    pub modalities: Vec<ModalityFilterV1>,
    pub synchronicities: Vec<CatalogSynchronicity>,
    pub instructors: Vec<FilterTokenV1>,
    pub availability: Vec<AvailabilityWindowV1>,
    pub meeting_locations: Vec<FilterTokenV1>,
    pub building_room: BuildingRoomFilterV1,
    pub exam_codes: Vec<FilterTokenV1>,
    pub permission: PermissionFilterV1,
    pub eligibility: EligibilityFilterV1,
}

impl FilterValuesInputV1 {
    pub fn for_term(term: TermId) -> Self {
        Self {
            term,
            campuses: Vec::new(),
            subjects: Vec::new(),
            text: None,
            course_numbers: Vec::new(),
            levels: Vec::new(),
            credits: None,
            core: CoreFilterV1::default(),
            prerequisite: PrerequisiteFilterV1::Any,
            course_locations: Vec::new(),
            section_indexes: Vec::new(),
            section_numbers: Vec::new(),
            open_statuses: Vec::new(),
            modalities: Vec::new(),
            synchronicities: Vec::new(),
            instructors: Vec::new(),
            availability: Vec::new(),
            meeting_locations: Vec::new(),
            building_room: BuildingRoomFilterV1::default(),
            exam_codes: Vec::new(),
            permission: PermissionFilterV1::Any,
            eligibility: EligibilityFilterV1::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedFilterValuesV1 {
    term: TermId,
    campuses: Vec<crate::CampusCode>,
    subjects: Vec<CatalogSubjectCode>,
    text: Option<FilterSearchTextV1>,
    course_numbers: Vec<FilterTokenV1>,
    levels: Vec<FilterTokenV1>,
    credits: Option<CreditRangeV1>,
    core: CoreFilterV1,
    prerequisite: PrerequisiteFilterV1,
    course_locations: Vec<FilterTokenV1>,
    section_indexes: Vec<SectionIndex>,
    section_numbers: Vec<FilterTokenV1>,
    open_statuses: Vec<LiveOpenStateV1>,
    modalities: Vec<ModalityFilterV1>,
    synchronicities: Vec<CatalogSynchronicity>,
    instructors: Vec<FilterTokenV1>,
    availability: Vec<AvailabilityWindowV1>,
    meeting_locations: Vec<FilterTokenV1>,
    building_room: BuildingRoomFilterV1,
    exam_codes: Vec<FilterTokenV1>,
    permission: PermissionFilterV1,
    eligibility: EligibilityFilterV1,
}

impl NormalizedFilterValuesV1 {
    pub fn for_term(term: TermId) -> Self {
        Self::try_new(FilterValuesInputV1::for_term(term))
            .expect("the neutral filter input is always valid")
    }

    pub fn try_new(mut input: FilterValuesInputV1) -> Result<Self, FilterValueError> {
        uppercase_tokens(&mut input.course_numbers);
        uppercase_tokens(&mut input.levels);
        uppercase_tokens(&mut input.core.codes);
        uppercase_tokens(&mut input.course_locations);
        uppercase_tokens(&mut input.section_numbers);
        uppercase_tokens(&mut input.meeting_locations);
        uppercase_tokens(&mut input.building_room.building_codes);
        uppercase_tokens(&mut input.building_room.room_numbers);
        uppercase_tokens(&mut input.exam_codes);
        uppercase_tokens(&mut input.eligibility.major_codes);
        uppercase_tokens(&mut input.eligibility.minor_codes);
        uppercase_tokens(&mut input.eligibility.honor_program_codes);
        uppercase_tokens(&mut input.eligibility.unit_codes);
        for pair in &mut input.eligibility.unit_majors {
            pair.unit_code.make_ascii_uppercase();
            pair.major_code.make_ascii_uppercase();
        }
        input
            .instructors
            .iter_mut()
            .for_each(FilterTokenV1::collapse_whitespace);

        canonicalize(&mut input.campuses);
        canonicalize(&mut input.subjects);
        canonicalize(&mut input.course_numbers);
        canonicalize(&mut input.levels);
        canonicalize(&mut input.core.codes);
        canonicalize(&mut input.course_locations);
        canonicalize(&mut input.section_indexes);
        canonicalize(&mut input.section_numbers);
        canonicalize(&mut input.open_statuses);
        canonicalize(&mut input.modalities);
        input.synchronicities.sort_by_key(|value| value.wire_name());
        input.synchronicities.dedup();
        canonicalize(&mut input.instructors);
        canonicalize(&mut input.availability);
        canonicalize(&mut input.meeting_locations);
        canonicalize(&mut input.building_room.building_codes);
        canonicalize(&mut input.building_room.room_numbers);
        canonicalize(&mut input.exam_codes);
        canonicalize(&mut input.eligibility.major_codes);
        canonicalize(&mut input.eligibility.minor_codes);
        canonicalize(&mut input.eligibility.honor_program_codes);
        canonicalize(&mut input.eligibility.unit_codes);
        canonicalize(&mut input.eligibility.unit_majors);

        let ordinary_lengths = [
            input.campuses.len(),
            input.subjects.len(),
            input.course_numbers.len(),
            input.levels.len(),
            input.core.codes.len(),
            input.course_locations.len(),
            input.section_indexes.len(),
            input.section_numbers.len(),
            input.open_statuses.len(),
            input.modalities.len(),
            input.synchronicities.len(),
            input.instructors.len(),
            input.meeting_locations.len(),
            input.building_room.building_codes.len(),
            input.building_room.room_numbers.len(),
            input.exam_codes.len(),
            input.eligibility.major_codes.len(),
            input.eligibility.minor_codes.len(),
            input.eligibility.honor_program_codes.len(),
            input.eligibility.unit_codes.len(),
            input.eligibility.unit_majors.len(),
        ];
        if ordinary_lengths
            .iter()
            .any(|length| *length > MAX_FILTER_VALUES_PER_FIELD)
            || input.availability.len() > MAX_AVAILABILITY_WINDOWS
        {
            return Err(FilterValueError::TooManyFieldValues);
        }
        let total = ordinary_lengths.iter().sum::<usize>() + input.availability.len();
        if total > MAX_TOTAL_FILTER_VALUES {
            return Err(FilterValueError::TooManyTotalValues);
        }

        Ok(Self {
            term: input.term,
            campuses: input.campuses,
            subjects: input.subjects,
            text: input.text,
            course_numbers: input.course_numbers,
            levels: input.levels,
            credits: input.credits,
            core: input.core,
            prerequisite: input.prerequisite,
            course_locations: input.course_locations,
            section_indexes: input.section_indexes,
            section_numbers: input.section_numbers,
            open_statuses: input.open_statuses,
            modalities: input.modalities,
            synchronicities: input.synchronicities,
            instructors: input.instructors,
            availability: input.availability,
            meeting_locations: input.meeting_locations,
            building_room: input.building_room,
            exam_codes: input.exam_codes,
            permission: input.permission,
            eligibility: input.eligibility,
        })
    }

    pub const fn term(&self) -> &TermId {
        &self.term
    }
    pub fn campuses(&self) -> &[crate::CampusCode] {
        &self.campuses
    }
    pub fn subjects(&self) -> &[CatalogSubjectCode] {
        &self.subjects
    }
    pub const fn text(&self) -> Option<&FilterSearchTextV1> {
        self.text.as_ref()
    }
    pub fn course_numbers(&self) -> &[FilterTokenV1] {
        &self.course_numbers
    }
    pub fn levels(&self) -> &[FilterTokenV1] {
        &self.levels
    }
    pub const fn credits(&self) -> Option<&CreditRangeV1> {
        self.credits.as_ref()
    }
    pub const fn core(&self) -> &CoreFilterV1 {
        &self.core
    }
    pub const fn prerequisite(&self) -> PrerequisiteFilterV1 {
        self.prerequisite
    }
    pub fn course_locations(&self) -> &[FilterTokenV1] {
        &self.course_locations
    }
    pub fn section_indexes(&self) -> &[SectionIndex] {
        &self.section_indexes
    }
    pub fn section_numbers(&self) -> &[FilterTokenV1] {
        &self.section_numbers
    }
    pub fn open_statuses(&self) -> &[LiveOpenStateV1] {
        &self.open_statuses
    }
    pub fn modalities(&self) -> &[ModalityFilterV1] {
        &self.modalities
    }
    pub fn synchronicities(&self) -> &[CatalogSynchronicity] {
        &self.synchronicities
    }
    pub fn instructors(&self) -> &[FilterTokenV1] {
        &self.instructors
    }
    pub fn availability(&self) -> &[AvailabilityWindowV1] {
        &self.availability
    }
    pub fn meeting_locations(&self) -> &[FilterTokenV1] {
        &self.meeting_locations
    }
    pub const fn building_room(&self) -> &BuildingRoomFilterV1 {
        &self.building_room
    }
    pub fn exam_codes(&self) -> &[FilterTokenV1] {
        &self.exam_codes
    }
    pub const fn permission(&self) -> PermissionFilterV1 {
        self.permission
    }
    pub const fn eligibility(&self) -> &EligibilityFilterV1 {
        &self.eligibility
    }
}

impl<'de> Deserialize<'de> for NormalizedFilterValuesV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(FilterValuesInputV1::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilterRequestV1 {
    contract_version: QueryContractVersion,
    values: NormalizedFilterValuesV1,
}

impl FilterRequestV1 {
    pub const fn new(values: NormalizedFilterValuesV1) -> Self {
        Self {
            contract_version: QUERY_CONTRACT_VERSION,
            values,
        }
    }
    pub const fn contract_version(&self) -> QueryContractVersion {
        self.contract_version
    }
    pub const fn values(&self) -> &NormalizedFilterValuesV1 {
        &self.values
    }
    pub fn into_values(self) -> NormalizedFilterValuesV1 {
        self.values
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageRequestV1 {
    page: u32,
    page_size: u16,
}

impl PageRequestV1 {
    pub fn try_new(page: u32, page_size: u16) -> Result<Self, FilterValueError> {
        if page == 0 || page_size == 0 || page_size > MAX_PAGE_SIZE {
            return Err(FilterValueError::InvalidPage);
        }
        Ok(Self { page, page_size })
    }
    pub const fn page(self) -> u32 {
        self.page
    }
    pub const fn page_size(self) -> u16 {
        self.page_size
    }
}

impl Default for PageRequestV1 {
    fn default() -> Self {
        Self {
            page: 1,
            page_size: DEFAULT_PAGE_SIZE,
        }
    }
}

impl<'de> Deserialize<'de> for PageRequestV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Wire {
            page: u32,
            page_size: u16,
        }
        let wire = Wire::deserialize(deserializer)?;
        Self::try_new(wire.page, wire.page_size).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SortDirectionV1 {
    Ascending,
    Descending,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CourseSortFieldV1 {
    Relevance,
    CourseIdentifier,
    Title,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CourseSortV1 {
    pub field: CourseSortFieldV1,
    pub direction: SortDirectionV1,
}

impl Default for CourseSortV1 {
    fn default() -> Self {
        Self {
            field: CourseSortFieldV1::Relevance,
            direction: SortDirectionV1::Descending,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SectionSortFieldV1 {
    SectionIndex,
    SectionNumber,
    CourseIdentifier,
    OpenStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SectionSortV1 {
    pub field: SectionSortFieldV1,
    pub direction: SortDirectionV1,
}

impl Default for SectionSortV1 {
    fn default() -> Self {
        Self {
            field: SectionSortFieldV1::SectionIndex,
            direction: SortDirectionV1::Ascending,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CourseQueryRequestV1 {
    pub filters: FilterRequestV1,
    pub page: PageRequestV1,
    pub sort: CourseSortV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SectionQueryRequestV1 {
    pub filters: FilterRequestV1,
    pub page: PageRequestV1,
    pub sort: SectionSortV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CourseDetailRequestV1 {
    pub contract_version: QueryContractVersion,
    pub key: CourseGroupKey,
}

impl CourseDetailRequestV1 {
    pub const fn new(key: CourseGroupKey) -> Self {
        Self {
            contract_version: QUERY_CONTRACT_VERSION,
            key,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SectionDetailRequestV1 {
    pub contract_version: QueryContractVersion,
    pub key: SectionKey,
}

impl SectionDetailRequestV1 {
    pub const fn new(key: SectionKey) -> Self {
        Self {
            contract_version: QUERY_CONTRACT_VERSION,
            key,
        }
    }
}

/// Query consumes live Open evidence supplied by the Open subsystem. This DTO
/// intentionally contains no polling, reconciliation, or scheduling policy.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveOpenEvidenceV1 {
    pub state: LiveOpenStateV1,
    #[serde(with = "time::serde::rfc3339::option")]
    pub observed_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub fresh_until: Option<OffsetDateTime>,
    pub uncertainty: Option<crate::MatchReasonCode>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterMatchV1 {
    pub field_id: FilterFieldId,
    pub explanation: MatchExplanation,
}

/// Stable C04 evidence. Exact identifier priority is explicit and never
/// reconstructed from display strings by clients.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextMatchEvidenceV1 {
    pub exact_course_identifier: bool,
    pub matched_tokens: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionQueryItemV1 {
    pub section: NormalizedSectionV1,
    pub occurrences: Vec<NormalizedOccurrenceV1>,
    pub open: LiveOpenEvidenceV1,
    pub explanation: MatchExplanation,
    pub filter_matches: Vec<FilterMatchV1>,
}

/// Independent Section search carries its owning course variant so title,
/// credits, and course identity are available without a follow-up lookup.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionSearchItemV1 {
    pub variant: NormalizedCourseVariantV1,
    pub section: SectionQueryItemV1,
    pub course_filter_matches: Vec<FilterMatchV1>,
    pub text_match: Option<TextMatchEvidenceV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseVariantQueryItemV1 {
    pub variant: NormalizedCourseVariantV1,
    pub explanation: MatchExplanation,
    pub filter_matches: Vec<FilterMatchV1>,
    pub text_match: Option<TextMatchEvidenceV1>,
    pub sections: Vec<SectionQueryItemV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseQueryItemV1 {
    pub group: NormalizedCourseGroupV1,
    pub explanation: MatchExplanation,
    pub variants: Vec<CourseVariantQueryItemV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PageInfoV1 {
    pub page: u32,
    pub page_size: u16,
    pub total: u64,
    pub total_pages: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseQueryResponseV1 {
    pub contract_version: QueryContractVersion,
    pub page: PageInfoV1,
    pub items: Vec<CourseQueryItemV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionQueryResponseV1 {
    pub contract_version: QueryContractVersion,
    pub page: PageInfoV1,
    pub items: Vec<SectionSearchItemV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CourseDetailResponseV1 {
    pub contract_version: QueryContractVersion,
    pub course: CourseQueryItemV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionDetailResponseV1 {
    pub contract_version: QueryContractVersion,
    pub variant: NormalizedCourseVariantV1,
    pub section: SectionQueryItemV1,
}
