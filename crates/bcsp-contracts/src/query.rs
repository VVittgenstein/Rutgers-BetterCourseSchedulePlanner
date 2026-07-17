use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use time::OffsetDateTime;

use crate::{
    CatalogContentVersion, CatalogSubjectCode, CourseGroupKey, MatchExplanation,
    NormalizedCourseGroupV1, NormalizedCourseVariantV1, NormalizedOccurrenceV1,
    NormalizedSectionV1, SectionIndex, SectionKey, TermId,
};

pub const QUERY_CONTRACT_VERSION: QueryContractVersion = QueryContractVersion::V3;
pub const FILTER_FIELD_COUNT: usize = 18;
pub const DEFAULT_PAGE_SIZE: u16 = 25;
pub const MAX_PAGE_SIZE: u16 = 200;
pub const MAX_FILTER_VALUES_PER_FIELD: usize = 100;
pub const MAX_AVAILABILITY_WINDOWS: usize = 64;
pub const MAX_TOTAL_FILTER_VALUES: usize = 512;
pub const DEFAULT_FILTER_OPTIONS_LIMIT: u16 = 50;
pub const MAX_FILTER_OPTIONS_LIMIT: u16 = 100;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum QueryContractVersion {
    V1,
    V2,
    V3,
}

impl QueryContractVersion {
    pub const fn as_u16(self) -> u16 {
        match self {
            Self::V1 => 1,
            Self::V2 => 2,
            Self::V3 => 3,
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
            2 => Ok(Self::V2),
            3 => Ok(Self::V3),
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
    CourseNumberBand,
    #[serde(rename = "FLT-C06")]
    CourseLevel,
    #[serde(rename = "FLT-C07")]
    CourseCredits,
    #[serde(rename = "FLT-C08")]
    CourseCoreCode,
    #[serde(rename = "FLT-C09")]
    CoursePrerequisite,
    #[serde(rename = "FLT-S01")]
    SectionIndex,
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
    #[serde(rename = "FLT-S09")]
    SectionExam,
    #[serde(rename = "FLT-S10")]
    SectionPermission,
}

impl FilterFieldId {
    /// Fields exposed by the active V2 product contract. Removed V1 IDs are
    /// recognized only by the raw Saved-view migration codec.
    pub const ALL: &'static [Self] = &[
        Self::CourseTerm,
        Self::CourseCampus,
        Self::CourseSubject,
        Self::CourseText,
        Self::CourseNumberBand,
        Self::CourseLevel,
        Self::CourseCredits,
        Self::CourseCoreCode,
        Self::CoursePrerequisite,
        Self::SectionIndex,
        Self::SectionOpenStatus,
        Self::SectionModality,
        Self::SectionSynchronicity,
        Self::SectionInstructor,
        Self::SectionAvailability,
        Self::SectionMeetingLocation,
        Self::SectionExam,
        Self::SectionPermission,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::CourseTerm => "FLT-C01",
            Self::CourseCampus => "FLT-C02",
            Self::CourseSubject => "FLT-C03",
            Self::CourseText => "FLT-C04",
            Self::CourseNumberBand => "FLT-C05",
            Self::CourseLevel => "FLT-C06",
            Self::CourseCredits => "FLT-C07",
            Self::CourseCoreCode => "FLT-C08",
            Self::CoursePrerequisite => "FLT-C09",
            Self::SectionIndex => "FLT-S01",
            Self::SectionOpenStatus => "FLT-S03",
            Self::SectionModality => "FLT-S04a",
            Self::SectionSynchronicity => "FLT-S04b",
            Self::SectionInstructor => "FLT-S05",
            Self::SectionAvailability => "FLT-S06",
            Self::SectionMeetingLocation => "FLT-S07",
            Self::SectionExam => "FLT-S09",
            Self::SectionPermission => "FLT-S10",
        }
    }

    pub const fn scope(self) -> FilterScopeV1 {
        match self {
            Self::CourseTerm
            | Self::CourseCampus
            | Self::CourseSubject
            | Self::CourseText
            | Self::CourseNumberBand
            | Self::CourseLevel
            | Self::CourseCredits
            | Self::CourseCoreCode
            | Self::CoursePrerequisite => FilterScopeV1::Course,
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
    KeywordSet,
    CourseNumberBand,
    LevelSet,
    CreditRange,
    CoreCodeSet,
    PrerequisitePresence,
    SectionIndexSet,
    OpenStatusSet,
    ModalitySet,
    SynchronicitySet,
    InstructorNameSet,
    AvailabilityWindows,
    MeetingLocationSet,
    ExamCodeSet,
    PermissionRequirement,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FilterSchemaValueV1 {
    Required,
    EmptySet,
    UnboundedRange,
    Any,
    EmptyWindows,
    EmptyComposite,
}

/// Exact canonical JSON used when a filter field is neutral.
///
/// `Required` deliberately has no JSON value: callers migrating an older
/// filter payload must obtain that field from product context instead of
/// silently inventing a value. `Json` distinguishes a materialized JSON null
/// from an unavailable required value.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FilterCanonicalNeutralV1 {
    Required,
    Json { value: serde_json::Value },
}

impl FilterCanonicalNeutralV1 {
    pub const fn json(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Required => None,
            Self::Json { value } => Some(value),
        }
    }

    pub fn into_json(self) -> Option<serde_json::Value> {
        match self {
            Self::Required => None,
            Self::Json { value } => Some(value),
        }
    }
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
    NonnegativeHundredBand,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FilterQueryEncodingV1 {
    ExactOne,
    ExactAny,
    KeywordTokenAndExactIdentifierPriority,
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
    pub canonical_neutral: FilterCanonicalNeutralV1,
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
    use FilterCanonicalNeutralV1 as C;
    use FilterFieldId as Id;
    use FilterNormalizationV1 as N;
    use FilterQueryEncodingV1 as Q;
    use FilterSchemaValueV1 as D;
    use FilterValidationV1 as V;
    use FilterValueKindV1 as K;

    let (
        request_field,
        value_kind,
        neutral,
        default,
        canonical_neutral,
        normalization,
        validation,
        query_encoding,
    ) = match id {
        Id::CourseTerm => (
            "term",
            K::TermId,
            D::Required,
            D::Required,
            C::Required,
            vec![N::CanonicalIdentity],
            vec![V::Required, V::DynamicDictionary],
            Q::ExactOne,
        ),
        Id::CourseCampus => (
            "campuses",
            K::CampusCodeSet,
            D::EmptySet,
            D::EmptySet,
            canonical_json(serde_json::json!([])),
            vec![N::CanonicalIdentity, N::SortDeduplicate],
            vec![V::DynamicDictionary],
            Q::ExactAny,
        ),
        Id::CourseSubject => (
            "subjects",
            K::SubjectCodeSet,
            D::EmptySet,
            D::EmptySet,
            canonical_json(serde_json::json!([])),
            vec![N::CanonicalIdentity, N::SortDeduplicate],
            vec![V::DynamicDictionary],
            Q::ExactAny,
        ),
        Id::CourseText => (
            "keywords",
            K::KeywordSet,
            D::EmptySet,
            D::EmptySet,
            canonical_json(serde_json::json!([])),
            vec![
                N::TrimAndCollapseWhitespace,
                N::SortDeduplicate,
                N::TokenAnd,
            ],
            vec![
                V::DynamicDictionary,
                V::NonemptyWhenActive,
                V::Max32TextTokens,
                V::Max128TokenBytes,
                V::TokenContainsAlphanumeric,
            ],
            Q::KeywordTokenAndExactIdentifierPriority,
        ),
        Id::CourseNumberBand => (
            "courseNumberBands",
            K::CourseNumberBand,
            D::EmptySet,
            D::EmptySet,
            canonical_json(serde_json::json!([])),
            vec![N::SortDeduplicate],
            vec![V::NonnegativeHundredBand],
            Q::ExactAny,
        ),
        Id::CourseLevel => (
            "levels",
            K::LevelSet,
            D::EmptySet,
            D::EmptySet,
            canonical_json(serde_json::json!([])),
            vec![N::Trim, N::AsciiUppercase, N::SortDeduplicate],
            vec![V::DynamicDictionary],
            Q::ExactAny,
        ),
        Id::CourseCredits => (
            "credits",
            K::CreditRange,
            D::UnboundedRange,
            D::UnboundedRange,
            canonical_json(serde_json::Value::Null),
            vec![N::CreditHundredths],
            vec![V::OrderedInclusiveRange],
            Q::InclusiveRange,
        ),
        Id::CourseCoreCode => (
            "core",
            K::CoreCodeSet,
            D::EmptySet,
            D::EmptySet,
            canonical_json(serde_json::json!({"codes": [], "mode": "ANY"})),
            vec![N::Trim, N::AsciiUppercase, N::SortDeduplicate],
            vec![V::DynamicDictionary],
            Q::ExplicitAnyAll,
        ),
        Id::CoursePrerequisite => (
            "prerequisite",
            K::PrerequisitePresence,
            D::Any,
            D::Any,
            canonical_json(serde_json::json!("ANY")),
            vec![],
            vec![],
            Q::TernaryPresence,
        ),
        Id::SectionIndex => (
            "sectionIndexes",
            K::SectionIndexSet,
            D::EmptySet,
            D::EmptySet,
            canonical_json(serde_json::json!([])),
            vec![N::CanonicalIdentity, N::SortDeduplicate],
            vec![V::SectionIndexIdentity],
            Q::SameSectionExactAny,
        ),
        Id::SectionOpenStatus => (
            "openStatuses",
            K::OpenStatusSet,
            D::EmptySet,
            D::EmptySet,
            canonical_json(serde_json::json!([])),
            vec![N::SortDeduplicate],
            vec![],
            Q::SameSectionExactAny,
        ),
        Id::SectionModality => (
            "modalities",
            K::ModalitySet,
            D::EmptySet,
            D::EmptySet,
            canonical_json(serde_json::json!([])),
            vec![N::SortDeduplicate],
            vec![],
            Q::SameSectionExactAny,
        ),
        Id::SectionSynchronicity => (
            "synchronicities",
            K::SynchronicitySet,
            D::EmptySet,
            D::EmptySet,
            canonical_json(serde_json::json!([])),
            vec![N::SortDeduplicate],
            vec![],
            Q::SameSectionExactAny,
        ),
        Id::SectionInstructor => (
            "instructors",
            K::InstructorNameSet,
            D::EmptySet,
            D::EmptySet,
            canonical_json(serde_json::json!([])),
            vec![N::TrimAndCollapseWhitespace, N::SortDeduplicate],
            vec![V::DynamicDictionary],
            Q::SameSectionExactAny,
        ),
        Id::SectionAvailability => (
            "availability",
            K::AvailabilityWindows,
            D::EmptyWindows,
            D::EmptyWindows,
            canonical_json(serde_json::json!([])),
            vec![N::MinuteOfDay, N::SortDeduplicate],
            vec![V::OrderedMinuteInterval],
            Q::SameSectionAvailabilityAll,
        ),
        Id::SectionMeetingLocation => (
            "meetingLocations",
            K::MeetingLocationSet,
            D::EmptyComposite,
            D::EmptyComposite,
            canonical_json(serde_json::json!({
                "locations": [],
                "mode": "ANY_MEETING"
            })),
            vec![N::Trim, N::AsciiUppercase, N::SortDeduplicate],
            vec![V::DynamicDictionary],
            Q::ExplicitAnyAll,
        ),
        Id::SectionExam => (
            "examCodes",
            K::ExamCodeSet,
            D::EmptySet,
            D::EmptySet,
            canonical_json(serde_json::json!([])),
            vec![N::Trim, N::AsciiUppercase, N::SortDeduplicate],
            vec![V::DynamicDictionary],
            Q::SameSectionExactAny,
        ),
        Id::SectionPermission => (
            "permission",
            K::PermissionRequirement,
            D::Any,
            D::Any,
            canonical_json(serde_json::json!("ANY")),
            vec![],
            vec![],
            Q::TernaryPresence,
        ),
    };
    FilterFieldSchemaV1 {
        stable_id: id,
        request_field: request_field.to_owned(),
        scope: id.scope(),
        value_kind,
        neutral,
        default,
        canonical_neutral,
        normalization,
        validation,
        query_encoding,
        i18n_key: format!("filter.{}", id.wire_name().to_ascii_lowercase()),
        chip_order,
    }
}

fn canonical_json(value: serde_json::Value) -> FilterCanonicalNeutralV1 {
    FilterCanonicalNeutralV1::Json { value }
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

    /// V2 wire values are selected dictionary keywords, not a free-form text
    /// token. The legacy type name is retained to keep the query engine API
    /// source-compatible while the public representation is an array.
    pub fn try_from_keywords<I, S>(keywords: I) -> Result<Self, FilterSearchTextError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut tokens = keywords
            .into_iter()
            .map(|value| value.as_ref().trim().to_owned())
            .collect::<Vec<_>>();
        tokens.sort_by(|left, right| {
            left.to_lowercase()
                .cmp(&right.to_lowercase())
                .then_with(|| left.cmp(right))
        });
        tokens.dedup_by(|left, right| left.eq_ignore_ascii_case(right));
        Self::from_tokens(tokens)
    }

    fn from_tokens(tokens: Vec<String>) -> Result<Self, FilterSearchTextError> {
        if tokens.is_empty() || tokens.len() > 32 {
            return Err(FilterSearchTextError::TokenCount);
        }
        if tokens.iter().any(|token| {
            token.is_empty()
                || token.trim() != token
                || token
                    .chars()
                    .any(|character| character.is_control() && !character.is_whitespace())
        }) {
            return Err(FilterSearchTextError::ControlCharacter);
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
        if text.len() > FILTER_TEXT_MAX_BYTES {
            return Err(FilterSearchTextError::TextTooLong);
        }
        Ok(Self { text, tokens })
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
        Self::from_tokens(tokens)
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
        self.tokens.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for FilterSearchTextV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from_keywords(Vec::<String>::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

mod optional_keywords {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::{FilterSearchTextError, FilterSearchTextV1};

    pub fn serialize<S>(
        value: &Option<FilterSearchTextV1>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        value
            .as_ref()
            .map(FilterSearchTextV1::tokens)
            .unwrap_or_default()
            .serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<FilterSearchTextV1>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let values = Vec::<String>::deserialize(deserializer)?;
        if values.is_empty() {
            Ok(None)
        } else {
            FilterSearchTextV1::try_from_keywords(values)
                .map(Some)
                .map_err(|error: FilterSearchTextError| serde::de::Error::custom(error))
        }
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
pub enum MeetingLocationMatchModeV2 {
    AnyMeeting,
    AllRequiredMeetings,
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

/// Ordinary product-facing modality values accepted by Query V3.
///
/// Catalog `OTHER`, `UNKNOWN`, and `UNKNOWN_CONFLICT` remain available in raw
/// detail data, but are deliberately not selectable filter values.  When the
/// field is active they evaluate as uncertain and are admitted only through
/// [`IncludeIncompleteV3::modality`].
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserModalityV3 {
    OnCampusOrInPerson,
    Online,
    Hybrid,
}

/// Ordinary product-facing synchronicity values accepted by Query V3.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserSynchronicityV3 {
    Sync,
    Async,
    Mixed,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IncludeIncompleteV3 {
    pub prerequisite: bool,
    pub modality: bool,
    pub synchronicity: bool,
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
    #[error(
        "course-number bands must be nonnegative multiples of 100 with an inclusive upper bound"
    )]
    InvalidCourseNumberBand,
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MeetingLocationFilterV2 {
    pub locations: Vec<FilterTokenV1>,
    pub mode: MeetingLocationMatchModeV2,
}

impl Default for MeetingLocationFilterV2 {
    fn default() -> Self {
        Self {
            locations: Vec::new(),
            mode: MeetingLocationMatchModeV2::AnyMeeting,
        }
    }
}

fn canonicalize<T: Ord>(values: &mut Vec<T>) {
    values.sort_unstable();
    values.dedup();
}

/// Returns the canonical hundred-level band for an ASCII-decimal Catalog
/// course number. Non-decimal, empty, or out-of-range source values are not
/// fabricated into a band.
pub fn course_number_band(value: &str) -> Option<u32> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value
        .parse::<u32>()
        .ok()
        .map(|number| number / 100 * 100)
        .filter(|band| *band <= u32::MAX - 99)
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
    #[serde(rename = "keywords", with = "optional_keywords")]
    pub text: Option<FilterSearchTextV1>,
    pub course_number_bands: Vec<u32>,
    pub levels: Vec<FilterTokenV1>,
    pub credits: Option<CreditRangeV1>,
    pub core: CoreFilterV1,
    pub prerequisite: PrerequisiteFilterV1,
    pub section_indexes: Vec<SectionIndex>,
    pub open_statuses: Vec<LiveOpenStateV1>,
    pub modalities: Vec<UserModalityV3>,
    pub synchronicities: Vec<UserSynchronicityV3>,
    pub instructors: Vec<FilterTokenV1>,
    pub availability: Vec<AvailabilityWindowV1>,
    pub meeting_locations: MeetingLocationFilterV2,
    pub exam_codes: Vec<FilterTokenV1>,
    pub permission: PermissionFilterV1,
    pub include_incomplete: IncludeIncompleteV3,
}

impl FilterValuesInputV1 {
    pub fn for_term(term: TermId) -> Self {
        Self {
            term,
            campuses: Vec::new(),
            subjects: Vec::new(),
            text: None,
            course_number_bands: Vec::new(),
            levels: Vec::new(),
            credits: None,
            core: CoreFilterV1::default(),
            prerequisite: PrerequisiteFilterV1::Any,
            section_indexes: Vec::new(),
            open_statuses: Vec::new(),
            modalities: Vec::new(),
            synchronicities: Vec::new(),
            instructors: Vec::new(),
            availability: Vec::new(),
            meeting_locations: MeetingLocationFilterV2::default(),
            exam_codes: Vec::new(),
            permission: PermissionFilterV1::Any,
            include_incomplete: IncludeIncompleteV3::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedFilterValuesV1 {
    term: TermId,
    campuses: Vec<crate::CampusCode>,
    subjects: Vec<CatalogSubjectCode>,
    #[serde(rename = "keywords", with = "optional_keywords")]
    text: Option<FilterSearchTextV1>,
    course_number_bands: Vec<u32>,
    levels: Vec<FilterTokenV1>,
    credits: Option<CreditRangeV1>,
    core: CoreFilterV1,
    prerequisite: PrerequisiteFilterV1,
    section_indexes: Vec<SectionIndex>,
    open_statuses: Vec<LiveOpenStateV1>,
    modalities: Vec<UserModalityV3>,
    synchronicities: Vec<UserSynchronicityV3>,
    instructors: Vec<FilterTokenV1>,
    availability: Vec<AvailabilityWindowV1>,
    meeting_locations: MeetingLocationFilterV2,
    exam_codes: Vec<FilterTokenV1>,
    permission: PermissionFilterV1,
    include_incomplete: IncludeIncompleteV3,
}

impl NormalizedFilterValuesV1 {
    pub fn for_term(term: TermId) -> Self {
        Self::try_new(FilterValuesInputV1::for_term(term))
            .expect("the neutral filter input is always valid")
    }

    pub fn try_new(mut input: FilterValuesInputV1) -> Result<Self, FilterValueError> {
        uppercase_tokens(&mut input.levels);
        uppercase_tokens(&mut input.core.codes);
        uppercase_tokens(&mut input.meeting_locations.locations);
        uppercase_tokens(&mut input.exam_codes);
        input
            .instructors
            .iter_mut()
            .for_each(FilterTokenV1::collapse_whitespace);

        canonicalize(&mut input.campuses);
        canonicalize(&mut input.subjects);
        canonicalize(&mut input.course_number_bands);
        canonicalize(&mut input.levels);
        canonicalize(&mut input.core.codes);
        canonicalize(&mut input.section_indexes);
        canonicalize(&mut input.open_statuses);
        canonicalize(&mut input.modalities);
        canonicalize(&mut input.synchronicities);
        canonicalize(&mut input.instructors);
        canonicalize(&mut input.availability);
        canonicalize(&mut input.meeting_locations.locations);
        canonicalize(&mut input.exam_codes);

        let ordinary_lengths = [
            input.campuses.len(),
            input.subjects.len(),
            input.course_number_bands.len(),
            input.levels.len(),
            input.core.codes.len(),
            input.section_indexes.len(),
            input.open_statuses.len(),
            input.modalities.len(),
            input.synchronicities.len(),
            input.instructors.len(),
            input.meeting_locations.locations.len(),
            input.exam_codes.len(),
        ];
        if ordinary_lengths
            .iter()
            .any(|length| *length > MAX_FILTER_VALUES_PER_FIELD)
            || input.availability.len() > MAX_AVAILABILITY_WINDOWS
        {
            return Err(FilterValueError::TooManyFieldValues);
        }
        if input
            .course_number_bands
            .iter()
            .any(|band| band % 100 != 0 || *band > u32::MAX - 99)
        {
            return Err(FilterValueError::InvalidCourseNumberBand);
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
            course_number_bands: input.course_number_bands,
            levels: input.levels,
            credits: input.credits,
            core: input.core,
            prerequisite: input.prerequisite,
            section_indexes: input.section_indexes,
            open_statuses: input.open_statuses,
            modalities: input.modalities,
            synchronicities: input.synchronicities,
            instructors: input.instructors,
            availability: input.availability,
            meeting_locations: input.meeting_locations,
            exam_codes: input.exam_codes,
            permission: input.permission,
            include_incomplete: input.include_incomplete,
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
    pub const fn keywords(&self) -> Option<&FilterSearchTextV1> {
        self.text.as_ref()
    }
    pub fn course_number_bands(&self) -> &[u32] {
        &self.course_number_bands
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
    pub fn section_indexes(&self) -> &[SectionIndex] {
        &self.section_indexes
    }
    pub fn open_statuses(&self) -> &[LiveOpenStateV1] {
        &self.open_statuses
    }
    pub fn modalities(&self) -> &[UserModalityV3] {
        &self.modalities
    }
    pub fn synchronicities(&self) -> &[UserSynchronicityV3] {
        &self.synchronicities
    }
    pub fn instructors(&self) -> &[FilterTokenV1] {
        &self.instructors
    }
    pub fn availability(&self) -> &[AvailabilityWindowV1] {
        &self.availability
    }
    pub fn meeting_locations(&self) -> &MeetingLocationFilterV2 {
        &self.meeting_locations
    }
    pub fn exam_codes(&self) -> &[FilterTokenV1] {
        &self.exam_codes
    }
    pub const fn permission(&self) -> PermissionFilterV1 {
        self.permission
    }
    pub const fn include_incomplete(&self) -> IncludeIncompleteV3 {
        self.include_incomplete
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

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FilterOptionsFieldV2 {
    Keyword,
    CourseNumberBand,
    CourseLevel,
    Instructor,
    MeetingLocation,
    ExamCode,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum FilterOptionsRequestError {
    #[error("filter options require query contract version 3")]
    UnsupportedContractVersion,
    #[error("filter options require at least one campus")]
    EmptyCampusSet,
    #[error("filter options limit must be between 1 and 100")]
    InvalidLimit,
    #[error("filter options query is invalid for the selected field")]
    InvalidQuery,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FilterOptionsRequestV2 {
    pub contract_version: QueryContractVersion,
    pub term: TermId,
    pub campuses: Vec<crate::CampusCode>,
    pub field: FilterOptionsFieldV2,
    pub query: Option<String>,
    pub limit: Option<u16>,
}

impl FilterOptionsRequestV2 {
    pub fn validate(&mut self) -> Result<(), FilterOptionsRequestError> {
        if self.contract_version != QueryContractVersion::V3 {
            return Err(FilterOptionsRequestError::UnsupportedContractVersion);
        }
        canonicalize(&mut self.campuses);
        if self.campuses.is_empty() {
            return Err(FilterOptionsRequestError::EmptyCampusSet);
        }
        if self
            .limit
            .is_some_and(|limit| limit == 0 || limit > MAX_FILTER_OPTIONS_LIMIT)
        {
            return Err(FilterOptionsRequestError::InvalidLimit);
        }
        self.query = self
            .query
            .take()
            .map(|query| query.trim().to_owned())
            .filter(|query| !query.is_empty());
        if self.query.as_ref().is_some_and(|query| {
            query.len() > FILTER_TOKEN_MAX_BYTES || query.chars().any(char::is_control)
        }) || (self.query.is_some()
            && !matches!(
                self.field,
                FilterOptionsFieldV2::Keyword | FilterOptionsFieldV2::Instructor
            ))
        {
            return Err(FilterOptionsRequestError::InvalidQuery);
        }
        Ok(())
    }

    pub fn effective_limit(&self) -> usize {
        usize::from(self.limit.unwrap_or(DEFAULT_FILTER_OPTIONS_LIMIT))
    }

    pub fn targets(&self) -> Vec<crate::TermCampusKey> {
        self.campuses
            .iter()
            .cloned()
            .map(|campus| crate::TermCampusKey::new(self.term.clone(), campus))
            .collect()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterOptionTargetVersionV2 {
    pub target: crate::TermCampusKey,
    pub content_version: CatalogContentVersion,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterOptionV2 {
    pub value: String,
    pub label: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterOptionsResponseV2 {
    pub contract_version: QueryContractVersion,
    pub field: FilterOptionsFieldV2,
    pub target_versions: Vec<FilterOptionTargetVersionV2>,
    pub options: Vec<FilterOptionV2>,
    pub truncated: bool,
}

/// Exact, non-search validation of every target-bound dynamic filter value.
///
/// This is intentionally a separate product contract from filter-option
/// discovery: validation must not depend on a typeahead query, result limit,
/// or option ordering.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DynamicFilterValidationRequestV3 {
    pub filters: FilterRequestV1,
}

impl DynamicFilterValidationRequestV3 {
    pub const fn new(filters: FilterRequestV1) -> Self {
        Self { filters }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidDynamicFilterValueV3 {
    /// Stable filter identity (for example `FLT-C03` or `FLT-S07`).
    pub field: FilterFieldId,
    pub value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicFilterValidationResponseV3 {
    pub contract_version: QueryContractVersion,
    pub target_versions: Vec<FilterOptionTargetVersionV2>,
    pub invalid_values: Vec<InvalidDynamicFilterValueV3>,
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

// V2 is a wire-shape revision over the established query result model. These
// aliases let new callers state the active contract explicitly without
// duplicating result DTOs whose fields did not change.
pub type FilterSchemaV2 = FilterSchemaV1;
pub type FilterFieldSchemaV2 = FilterFieldSchemaV1;
pub type FilterRequestV2 = FilterRequestV1;
pub type FilterValuesInputV2 = FilterValuesInputV1;
pub type NormalizedFilterValuesV2 = NormalizedFilterValuesV1;
pub type CourseQueryRequestV2 = CourseQueryRequestV1;
pub type CourseQueryResponseV2 = CourseQueryResponseV1;
pub type SectionQueryRequestV2 = SectionQueryRequestV1;
pub type SectionQueryResponseV2 = SectionQueryResponseV1;

// V3 changes the strict request values while preserving the established Rust
// DTO names and additive result shape. These aliases make the active contract
// explicit at call sites without creating parallel evaluation models.
pub type FilterSchemaV3 = FilterSchemaV1;
pub type FilterFieldSchemaV3 = FilterFieldSchemaV1;
pub type FilterRequestV3 = FilterRequestV1;
pub type FilterValuesInputV3 = FilterValuesInputV1;
pub type NormalizedFilterValuesV3 = NormalizedFilterValuesV1;
pub type CourseQueryRequestV3 = CourseQueryRequestV1;
pub type CourseQueryResponseV3 = CourseQueryResponseV1;
pub type SectionQueryRequestV3 = SectionQueryRequestV1;
pub type SectionQueryResponseV3 = SectionQueryResponseV1;

pub fn filter_schema_v2() -> FilterSchemaV2 {
    filter_schema_v1()
}

pub fn filter_schema_v3() -> FilterSchemaV3 {
    filter_schema_v1()
}
