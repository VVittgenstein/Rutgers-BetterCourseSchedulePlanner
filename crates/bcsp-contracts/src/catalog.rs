use std::fmt;
use std::num::NonZeroU64;
use std::str::FromStr;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use time::OffsetDateTime;

use crate::{CourseGroupKey, CourseVariantKey, SectionKey, TermCampusKey, TraceId};

pub const CATALOG_CONTRACT_VERSION: CatalogContractVersion = CatalogContractVersion::V1;

const SHA256_HEX_BYTES: usize = 64;
const SUBJECT_CODE_MAX_BYTES: usize = 64;
const DISCOVERY_TOKEN_MAX_BYTES: usize = 64;

/// Version of the normalized Catalog and refresh-checkpoint contracts.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CatalogContractVersion {
    V1,
}

impl CatalogContractVersion {
    pub const fn as_u16(self) -> u16 {
        match self {
            Self::V1 => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("unsupported Catalog contract version")]
pub struct CatalogContractVersionError {
    observed: u16,
}

impl CatalogContractVersionError {
    pub const fn observed(self) -> u16 {
        self.observed
    }
}

impl TryFrom<u16> for CatalogContractVersion {
    type Error = CatalogContractVersionError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::V1),
            observed => Err(CatalogContractVersionError { observed }),
        }
    }
}

impl Serialize for CatalogContractVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u16(self.as_u16())
    }
}

impl<'de> Deserialize<'de> for CatalogContractVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u16::deserialize(deserializer)?;
        Self::try_from(value).map_err(D::Error::custom)
    }
}

/// Monotonic publication version. Zero is never a valid published version.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CatalogContentVersion(NonZeroU64);

impl CatalogContentVersion {
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl TryFrom<u64> for CatalogContentVersion {
    type Error = CatalogContentVersionError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(CatalogContentVersionError)
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("Catalog content version must be nonzero")]
pub struct CatalogContentVersionError;

/// Lowercase SHA-256 digest of the observed upstream payload.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CatalogPayloadDigest(String);

impl CatalogPayloadDigest {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("Catalog payload digest must be exactly 64 lowercase hexadecimal characters")]
pub struct CatalogPayloadDigestError;

impl TryFrom<String> for CatalogPayloadDigest {
    type Error = CatalogPayloadDigestError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() == SHA256_HEX_BYTES
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            Ok(Self(value))
        } else {
            Err(CatalogPayloadDigestError)
        }
    }
}

impl TryFrom<&str> for CatalogPayloadDigest {
    type Error = CatalogPayloadDigestError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_from(value.to_owned())
    }
}

impl FromStr for CatalogPayloadDigest {
    type Err = CatalogPayloadDigestError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value)
    }
}

impl fmt::Display for CatalogPayloadDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for CatalogPayloadDigest {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CatalogPayloadDigest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_from(value).map_err(D::Error::custom)
    }
}

/// Opaque dynamic subject identifier discovered from the upstream source.
/// It is deliberately not checked against a compiled subject allowlist.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CatalogSubjectCode(String);

impl CatalogSubjectCode {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("invalid dynamic Catalog subject code")]
pub struct CatalogSubjectCodeError;

impl TryFrom<String> for CatalogSubjectCode {
    type Error = CatalogSubjectCodeError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if !value.is_empty()
            && value.len() <= SUBJECT_CODE_MAX_BYTES
            && value.trim() == value
            && !value.chars().any(char::is_control)
        {
            Ok(Self(value))
        } else {
            Err(CatalogSubjectCodeError)
        }
    }
}

impl TryFrom<&str> for CatalogSubjectCode {
    type Error = CatalogSubjectCodeError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_from(value.to_owned())
    }
}

impl FromStr for CatalogSubjectCode {
    type Err = CatalogSubjectCodeError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::try_from(value)
    }
}

impl fmt::Display for CatalogSubjectCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Serialize for CatalogSubjectCode {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for CatalogSubjectCode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_from(value).map_err(D::Error::custom)
    }
}

macro_rules! validated_discovery_token {
    ($name:ident, $error:ident, $validator:expr, $message:literal) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        #[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
        #[error($message)]
        pub struct $error;

        impl TryFrom<String> for $name {
            type Error = $error;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                if !value.is_empty()
                    && value.len() <= DISCOVERY_TOKEN_MAX_BYTES
                    && value.bytes().all($validator)
                {
                    Ok(Self(value))
                } else {
                    Err($error)
                }
            }
        }

        impl TryFrom<&str> for $name {
            type Error = $error;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::try_from(value.to_owned())
            }
        }

        impl FromStr for $name {
            type Err = $error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::try_from(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Self::try_from(value).map_err(D::Error::custom)
            }
        }
    };
}

validated_discovery_token!(
    CatalogDiscoverySourceId,
    CatalogDiscoverySourceIdError,
    |byte: u8| byte.is_ascii_uppercase()
        || byte.is_ascii_digit()
        || matches!(byte, b'_' | b'.' | b':' | b'-'),
    "invalid stable Catalog discovery source identifier"
);

validated_discovery_token!(
    CatalogDiagnosticCode,
    CatalogDiagnosticCodeError,
    |byte: u8| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_',
    "invalid stable Catalog diagnostic code"
);

/// Why a field cannot currently be treated as known.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogUnknownReason {
    Missing,
    ExplicitNull,
    Malformed,
    SparseEvidence,
    NotObserved,
    SourceUnavailable,
    ConflictingEvidence,
}

impl CatalogUnknownReason {
    pub const ALL: &'static [Self] = &[
        Self::Missing,
        Self::ExplicitNull,
        Self::Malformed,
        Self::SparseEvidence,
        Self::NotObserved,
        Self::SourceUnavailable,
        Self::ConflictingEvidence,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Missing => "MISSING",
            Self::ExplicitNull => "EXPLICIT_NULL",
            Self::Malformed => "MALFORMED",
            Self::SparseEvidence => "SPARSE_EVIDENCE",
            Self::NotObserved => "NOT_OBSERVED",
            Self::SourceUnavailable => "SOURCE_UNAVAILABLE",
            Self::ConflictingEvidence => "CONFLICTING_EVIDENCE",
        }
    }
}

/// Presence of a field after its value is known to be trustworthy.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "presence", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogFieldPresence<T> {
    Present { value: T },
    ExplicitNull,
    Absent,
}

/// Knowledge and presence remain separate so missing/null source values never
/// silently become an empty string, false, zero, or empty collection.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "knowledge", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogFieldKnowledge<T> {
    Known { presence: CatalogFieldPresence<T> },
    Unknown { reason: CatalogUnknownReason },
}

impl<T> CatalogFieldKnowledge<T> {
    pub fn present(value: T) -> Self {
        Self::Known {
            presence: CatalogFieldPresence::Present { value },
        }
    }

    pub const fn explicit_null() -> Self {
        Self::Known {
            presence: CatalogFieldPresence::ExplicitNull,
        }
    }

    pub const fn absent() -> Self {
        Self::Known {
            presence: CatalogFieldPresence::Absent,
        }
    }

    pub const fn unknown(reason: CatalogUnknownReason) -> Self {
        Self::Unknown { reason }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogDiscoveryRequestV1 {
    pub contract_version: CatalogContractVersion,
}

impl CatalogDiscoveryRequestV1 {
    pub const fn new() -> Self {
        Self {
            contract_version: CATALOG_CONTRACT_VERSION,
        }
    }
}

impl Default for CatalogDiscoveryRequestV1 {
    fn default() -> Self {
        Self::new()
    }
}

/// A dynamic discovery target. Labels are evidence, never identity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogTargetV1 {
    pub key: TermCampusKey,
    pub term_label: CatalogFieldKnowledge<String>,
    pub campus_label: CatalogFieldKnowledge<String>,
    pub provenance: CatalogDiscoveryProvenanceV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDiscoveryResponseV1 {
    pub contract_version: CatalogContractVersion,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    pub status: CatalogDiscoveryStatusV1,
    pub sources: Vec<CatalogDiscoverySourceV1>,
    pub targets: Vec<CatalogTargetV1>,
    pub subjects: Vec<CatalogSubjectV1>,
    pub core_code_dictionaries: Vec<CatalogCoreCodeDictionaryV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogDiscoverySourceKind {
    Selector,
    Bootstrap,
}

impl CatalogDiscoverySourceKind {
    pub const ALL: &'static [Self] = &[Self::Selector, Self::Bootstrap];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Selector => "SELECTOR",
            Self::Bootstrap => "BOOTSTRAP",
        }
    }
}

/// Safe source identity/version/hash projection. It contains no raw body.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDiscoverySourceV1 {
    pub source_id: CatalogDiscoverySourceId,
    pub source_kind: CatalogDiscoverySourceKind,
    pub source_version: CatalogFieldKnowledge<String>,
    pub payload_digest: CatalogPayloadDigest,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogDiscoveryAvailability {
    Current,
    StaleLastSuccess,
    UnavailableNoFirstSuccess,
}

impl CatalogDiscoveryAvailability {
    pub const ALL: &'static [Self] = &[
        Self::Current,
        Self::StaleLastSuccess,
        Self::UnavailableNoFirstSuccess,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Current => "CURRENT",
            Self::StaleLastSuccess => "STALE_LAST_SUCCESS",
            Self::UnavailableNoFirstSuccess => "UNAVAILABLE_NO_FIRST_SUCCESS",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogDiscoveryErrorClass {
    Transport,
    Http,
    Decode,
    Validate,
    Persist,
}

impl CatalogDiscoveryErrorClass {
    pub const ALL: &'static [Self] = &[
        Self::Transport,
        Self::Http,
        Self::Decode,
        Self::Validate,
        Self::Persist,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Transport => "TRANSPORT",
            Self::Http => "HTTP",
            Self::Decode => "DECODE",
            Self::Validate => "VALIDATE",
            Self::Persist => "PERSIST",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDiscoveryErrorV1 {
    pub class: CatalogDiscoveryErrorClass,
    pub code: CatalogDiagnosticCode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDiscoveryPointV1 {
    pub observation_id: TraceId,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    pub content_version: Option<CatalogContentVersion>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDiscoveryStatusV1 {
    pub availability: CatalogDiscoveryAvailability,
    pub latest_attempt: Option<CatalogDiscoveryPointV1>,
    pub last_success: Option<CatalogDiscoveryPointV1>,
    pub is_stale: bool,
    pub error: Option<CatalogDiscoveryErrorV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDiscoveryProvenanceV1 {
    pub observation_id: TraceId,
    pub source_id: CatalogDiscoverySourceId,
    pub source_kind: CatalogDiscoverySourceKind,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    pub payload_digest: CatalogPayloadDigest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogSourceKind {
    RutgersCatalog,
}

impl CatalogSourceKind {
    pub const ALL: &'static [Self] = &[Self::RutgersCatalog];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::RutgersCatalog => "RUTGERS_CATALOG",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogProvenanceV1 {
    pub observation_id: TraceId,
    pub source: CatalogSourceKind,
    pub target: TermCampusKey,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    pub payload_digest: CatalogPayloadDigest,
}

/// Exact source family behind one subject dictionary entry.
///
/// Discovery and Catalog observations have different identities and lifecycles. Keeping them in
/// a tagged union prevents a Catalog-derived label from masquerading as selector evidence. The
/// Catalog variant also binds the entry to the precise serving content version from which it was
/// derived.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum CatalogSubjectProvenanceV1 {
    Discovery {
        discovery: CatalogDiscoveryProvenanceV1,
    },
    Catalog {
        content_version: CatalogContentVersion,
        catalog: CatalogProvenanceV1,
    },
}

/// Dynamic subject dictionary entry scoped to one discovered target and tied
/// to the exact source observation that supplied it.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSubjectV1 {
    pub target: TermCampusKey,
    pub code: CatalogSubjectCode,
    pub label: CatalogFieldKnowledge<String>,
    pub provenance: CatalogSubjectProvenanceV1,
}

/// One selectable Core Curriculum code derived from a published Catalog.
/// The description remains evidence so conflicting or malformed upstream
/// labels cannot masquerade as an authoritative display value.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCoreCodeOptionV1 {
    pub code: String,
    pub description: CatalogFieldKnowledge<String>,
}

/// Target-scoped Core Curriculum dictionary tied to the exact Catalog
/// publication from which it was derived.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCoreCodeDictionaryV1 {
    pub target: TermCampusKey,
    pub content_version: CatalogContentVersion,
    pub provenance: CatalogProvenanceV1,
    pub options: Vec<CatalogCoreCodeOptionV1>,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogOccurrenceKeyV1 {
    pub section: SectionKey,
    pub ordinal: u32,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedCourseGroupV1 {
    pub key: CourseGroupKey,
    pub variant_keys: Vec<CourseVariantKey>,
}

/// Coarse prerequisite availability derived from the upstream field state.
/// This does not attempt to parse prerequisite prose into requirements.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogPrerequisiteState {
    Has,
    NoneReported,
    Unknown,
}

impl CatalogPrerequisiteState {
    pub const ALL: &'static [Self] = &[Self::Has, Self::NoneReported, Self::Unknown];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Has => "HAS",
            Self::NoneReported => "NONE_REPORTED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedCourseVariantV1 {
    pub key: CourseVariantKey,
    pub title: CatalogFieldKnowledge<String>,
    pub expanded_title: CatalogFieldKnowledge<String>,
    pub description: CatalogFieldKnowledge<String>,
    pub notes: CatalogFieldKnowledge<String>,
    pub subject_group_notes: CatalogFieldKnowledge<String>,
    pub subject_notes: CatalogFieldKnowledge<String>,
    pub unit_notes: CatalogFieldKnowledge<String>,
    pub synopsis_url: CatalogFieldKnowledge<String>,
    pub prerequisite_notes: CatalogFieldKnowledge<String>,
    pub prerequisite_state: CatalogPrerequisiteState,
    pub credits: CatalogFieldKnowledge<String>,
    pub level: CatalogFieldKnowledge<String>,
    pub subject_code: CatalogFieldKnowledge<String>,
    pub subject_description: CatalogFieldKnowledge<String>,
    pub course_number: CatalogFieldKnowledge<String>,
    pub supplement_code: CatalogFieldKnowledge<String>,
    pub school_code: CatalogFieldKnowledge<String>,
    pub offering_unit: CatalogFieldKnowledge<String>,
    pub offering_unit_title: CatalogFieldKnowledge<String>,
    pub core_codes: CatalogFieldKnowledge<Vec<String>>,
    pub campus_locations: CatalogFieldKnowledge<Vec<String>>,
    pub section_keys: Vec<SectionKey>,
}

/// Catalog openness is historical snapshot evidence only. Live openness is
/// owned by the openSections pipeline and must not be inferred from this DTO.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogOpenStatusProvenance {
    CatalogSnapshotOnly,
}

impl CatalogOpenStatusProvenance {
    pub const ALL: &'static [Self] = &[Self::CatalogSnapshotOnly];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::CatalogSnapshotOnly => "CATALOG_SNAPSHOT_ONLY",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSnapshotOpenStatusV1 {
    pub value: CatalogFieldKnowledge<bool>,
    pub provenance: CatalogOpenStatusProvenance,
}

/// Rutgers Catalog currently exposes instructor display names only; stable
/// instructor identities are not implied by this projection.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogInstructorReliability {
    NameOnly,
}

impl CatalogInstructorReliability {
    pub const ALL: &'static [Self] = &[Self::NameOnly];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::NameOnly => "NAME_ONLY",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogUnitMajorV1 {
    pub unit_code: String,
    pub major_code: String,
}

/// One typed Rutgers section comment. A legacy string comment is represented
/// as a known description with an absent code; object fields retain their own
/// source-presence knowledge.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCommentV1 {
    pub code: CatalogFieldKnowledge<String>,
    pub description: CatalogFieldKnowledge<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedSectionV1 {
    pub key: SectionKey,
    pub variant_key: CourseVariantKey,
    pub section_number: CatalogFieldKnowledge<String>,
    pub subtitle: CatalogFieldKnowledge<String>,
    pub subtopic: CatalogFieldKnowledge<String>,
    pub section_notes: CatalogFieldKnowledge<String>,
    pub session_dates: CatalogFieldKnowledge<String>,
    pub session_date_print_indicator: CatalogFieldKnowledge<String>,
    pub comments: CatalogFieldKnowledge<Vec<CatalogCommentV1>>,
    pub comments_text: CatalogFieldKnowledge<String>,
    pub cross_listed_sections_text: CatalogFieldKnowledge<String>,
    pub cross_listed_section_type: CatalogFieldKnowledge<String>,
    pub instructors: CatalogFieldKnowledge<Vec<String>>,
    pub instructor_reliability: CatalogInstructorReliability,
    pub raw_section_course_type: CatalogFieldKnowledge<String>,
    pub delivery_modality: CatalogModality,
    pub synchronicity: CatalogSynchronicity,
    pub exam_code: CatalogFieldKnowledge<String>,
    pub exam_code_text: CatalogFieldKnowledge<String>,
    pub special_permission_add_code: CatalogFieldKnowledge<String>,
    pub special_permission_add_description: CatalogFieldKnowledge<String>,
    pub special_permission_drop_code: CatalogFieldKnowledge<String>,
    pub special_permission_drop_description: CatalogFieldKnowledge<String>,
    pub major_codes: CatalogFieldKnowledge<Vec<String>>,
    pub unit_codes: CatalogFieldKnowledge<Vec<String>>,
    pub minor_codes: CatalogFieldKnowledge<Vec<String>>,
    pub honor_program_codes: CatalogFieldKnowledge<Vec<String>>,
    pub unit_majors: CatalogFieldKnowledge<Vec<CatalogUnitMajorV1>>,
    pub eligibility_text: CatalogFieldKnowledge<String>,
    pub open_to_text: CatalogFieldKnowledge<String>,
    pub catalog_open_status: CatalogSnapshotOpenStatusV1,
    pub occurrence_keys: CatalogFieldKnowledge<Vec<CatalogOccurrenceKeyV1>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogModality {
    OnCampusOrInPerson,
    Hybrid,
    Online,
    Other,
    Unknown,
    UnknownConflict,
}

impl CatalogModality {
    pub const ALL: &'static [Self] = &[
        Self::OnCampusOrInPerson,
        Self::Hybrid,
        Self::Online,
        Self::Other,
        Self::Unknown,
        Self::UnknownConflict,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::OnCampusOrInPerson => "ON_CAMPUS_OR_IN_PERSON",
            Self::Hybrid => "HYBRID",
            Self::Online => "ONLINE",
            Self::Other => "OTHER",
            Self::Unknown => "UNKNOWN",
            Self::UnknownConflict => "UNKNOWN_CONFLICT",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogSynchronicity {
    Sync,
    Async,
    Mixed,
    /// Rutgers published "hours by arrangement" for a meeting that is not
    /// online or remote. The Schedule of Classes shows exactly this case as
    /// "Hours by arrangement" rather than "Asynchronous content", so it is a
    /// stated fact about WHEN the Section meets, not an absence of evidence.
    ByArrangement,
    Unspecified,
    Unknown,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogOccurrenceKind {
    Scheduled,
    ByArrangement,
    Unspecified,
}

impl CatalogOccurrenceKind {
    pub const ALL: &'static [Self] = &[Self::Scheduled, Self::ByArrangement, Self::Unspecified];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Scheduled => "SCHEDULED",
            Self::ByArrangement => "BY_ARRANGEMENT",
            Self::Unspecified => "UNSPECIFIED",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogOccurrenceEvidence {
    None,
    Physical,
    Remote,
}

impl CatalogOccurrenceEvidence {
    pub const ALL: &'static [Self] = &[Self::None, Self::Physical, Self::Remote];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Physical => "PHYSICAL",
            Self::Remote => "REMOTE",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogRequiredness {
    Required,
    Optional,
    UnknownRequiredness,
}

impl CatalogRequiredness {
    pub const ALL: &'static [Self] = &[Self::Required, Self::Optional, Self::UnknownRequiredness];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Required => "REQUIRED",
            Self::Optional => "OPTIONAL",
            Self::UnknownRequiredness => "UNKNOWN_REQUIREDNESS",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogTimeKnowledgeV1 {
    Missing,
    ExplicitNull,
    Empty,
    Partial,
    Invalid,
    Known { start_minute: u16, end_minute: u16 },
}

#[derive(Deserialize, Serialize)]
#[serde(
    tag = "knowledge",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
enum CatalogTimeKnowledgeWireV1 {
    Missing,
    ExplicitNull,
    Empty,
    Partial,
    Invalid,
    Known { start_minute: u16, end_minute: u16 },
}

impl Serialize for CatalogTimeKnowledgeV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let wire = match *self {
            Self::Missing => CatalogTimeKnowledgeWireV1::Missing,
            Self::ExplicitNull => CatalogTimeKnowledgeWireV1::ExplicitNull,
            Self::Empty => CatalogTimeKnowledgeWireV1::Empty,
            Self::Partial => CatalogTimeKnowledgeWireV1::Partial,
            Self::Invalid => CatalogTimeKnowledgeWireV1::Invalid,
            Self::Known {
                start_minute,
                end_minute,
            } if start_minute < 1_440 && end_minute <= 1_440 && end_minute > start_minute => {
                CatalogTimeKnowledgeWireV1::Known {
                    start_minute,
                    end_minute,
                }
            }
            Self::Known { .. } => {
                return Err(<S::Error as serde::ser::Error>::custom(
                    "known Catalog time must be an ordered minute range within one day",
                ));
            }
        };
        wire.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for CatalogTimeKnowledgeV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(
            match CatalogTimeKnowledgeWireV1::deserialize(deserializer)? {
                CatalogTimeKnowledgeWireV1::Missing => Self::Missing,
                CatalogTimeKnowledgeWireV1::ExplicitNull => Self::ExplicitNull,
                CatalogTimeKnowledgeWireV1::Empty => Self::Empty,
                CatalogTimeKnowledgeWireV1::Partial => Self::Partial,
                CatalogTimeKnowledgeWireV1::Invalid => Self::Invalid,
                CatalogTimeKnowledgeWireV1::Known {
                    start_minute,
                    end_minute,
                } if start_minute < 1_440 && end_minute <= 1_440 && end_minute > start_minute => {
                    Self::Known {
                        start_minute,
                        end_minute,
                    }
                }
                CatalogTimeKnowledgeWireV1::Known { .. } => {
                    return Err(D::Error::custom(
                        "known Catalog time must be an ordered minute range within one day",
                    ));
                }
            },
        )
    }
}

impl CatalogSynchronicity {
    pub const ALL: &'static [Self] = &[
        Self::Sync,
        Self::Async,
        Self::Mixed,
        Self::ByArrangement,
        Self::Unspecified,
        Self::Unknown,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Sync => "SYNC",
            Self::Async => "ASYNC",
            Self::Mixed => "MIXED",
            Self::ByArrangement => "BY_ARRANGEMENT",
            Self::Unspecified => "UNSPECIFIED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedOccurrenceV1 {
    pub key: CatalogOccurrenceKeyV1,
    pub raw_code: CatalogFieldKnowledge<String>,
    pub raw_description: CatalogFieldKnowledge<String>,
    pub modality: CatalogFieldKnowledge<CatalogModality>,
    pub synchronicity: CatalogFieldKnowledge<CatalogSynchronicity>,
    pub raw_day: CatalogFieldKnowledge<String>,
    pub days: CatalogFieldKnowledge<Vec<String>>,
    pub raw_start_time: CatalogFieldKnowledge<String>,
    pub raw_end_time: CatalogFieldKnowledge<String>,
    pub time: CatalogTimeKnowledgeV1,
    pub start_date: CatalogFieldKnowledge<String>,
    pub end_date: CatalogFieldKnowledge<String>,
    pub campus: CatalogFieldKnowledge<String>,
    pub campus_name: CatalogFieldKnowledge<String>,
    pub building: CatalogFieldKnowledge<String>,
    pub room: CatalogFieldKnowledge<String>,
    pub requiredness: CatalogRequiredness,
    pub kind: CatalogOccurrenceKind,
    pub evidence: CatalogOccurrenceEvidence,
    pub normalization_reason: CatalogDiagnosticCode,
}

/// Complete target-scoped normalized projection. Collections are explicit and
/// an empty collection never by itself proves that an empty source is valid.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedCatalogV1 {
    pub contract_version: CatalogContractVersion,
    pub target: TermCampusKey,
    pub content_version: CatalogContentVersion,
    pub provenance: CatalogProvenanceV1,
    pub course_groups: Vec<NormalizedCourseGroupV1>,
    pub course_variants: Vec<NormalizedCourseVariantV1>,
    pub sections: Vec<NormalizedSectionV1>,
    pub occurrences: Vec<NormalizedOccurrenceV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogRefreshClassification {
    Started,
    Staged,
    AppliedChanged,
    AppliedUnchanged,
    EmptyValidInitial,
    EmptySuspect,
    Partial,
    Error,
    Interrupted,
}

impl CatalogRefreshClassification {
    pub const ALL: &'static [Self] = &[
        Self::Started,
        Self::Staged,
        Self::AppliedChanged,
        Self::AppliedUnchanged,
        Self::EmptyValidInitial,
        Self::EmptySuspect,
        Self::Partial,
        Self::Error,
        Self::Interrupted,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Started => "STARTED",
            Self::Staged => "STAGED",
            Self::AppliedChanged => "APPLIED_CHANGED",
            Self::AppliedUnchanged => "APPLIED_UNCHANGED",
            Self::EmptyValidInitial => "EMPTY_VALID_INITIAL",
            Self::EmptySuspect => "EMPTY_SUSPECT",
            Self::Partial => "PARTIAL",
            Self::Error => "ERROR",
            Self::Interrupted => "INTERRUPTED",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CatalogRefreshErrorClass {
    Transport,
    Http,
    Decode,
    Normalize,
    Validate,
    Persist,
}

impl CatalogRefreshErrorClass {
    pub const ALL: &'static [Self] = &[
        Self::Transport,
        Self::Http,
        Self::Decode,
        Self::Normalize,
        Self::Validate,
        Self::Persist,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Transport => "TRANSPORT",
            Self::Http => "HTTP",
            Self::Decode => "DECODE",
            Self::Normalize => "NORMALIZE",
            Self::Validate => "VALIDATE",
            Self::Persist => "PERSIST",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntityCountsV1 {
    pub course_groups: u64,
    pub course_variants: u64,
    pub sections: u64,
    pub occurrences: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogRefreshObservationV1 {
    pub contract_version: CatalogContractVersion,
    pub observation_id: TraceId,
    pub target: TermCampusKey,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub finished_at: OffsetDateTime,
    pub classification: CatalogRefreshClassification,
    pub content_version: Option<CatalogContentVersion>,
    pub payload_digest: Option<CatalogPayloadDigest>,
    pub counts: CatalogEntityCountsV1,
    pub error_class: Option<CatalogRefreshErrorClass>,
    pub partial_reasons: Vec<CatalogUnknownReason>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogRefreshPointV1 {
    pub observation_id: TraceId,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    pub classification: CatalogRefreshClassification,
    pub content_version: Option<CatalogContentVersion>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogRefreshCheckpointPointV1 {
    pub observation_id: TraceId,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    pub classification: CatalogRefreshClassification,
    pub content_version: Option<CatalogContentVersion>,
}

/// Durable internal input. Unknown fields fail closed so a newer checkpoint is
/// never silently interpreted using older mutation semantics.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogRefreshCheckpointV1 {
    pub contract_version: CatalogContractVersion,
    pub target: TermCampusKey,
    pub latest_attempt: CatalogRefreshCheckpointPointV1,
    pub last_success: Option<CatalogRefreshCheckpointPointV1>,
    pub last_published: Option<CatalogRefreshCheckpointPointV1>,
    pub last_nonempty: Option<CatalogRefreshCheckpointPointV1>,
    pub pending_empty: Option<CatalogRefreshCheckpointPointV1>,
}

/// Additive server projection of refresh state. The four markers intentionally
/// remain separate; this contract does not define automatic empty confirmation.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogRefreshStatusV1 {
    pub contract_version: CatalogContractVersion,
    pub target: TermCampusKey,
    pub latest_attempt: CatalogRefreshPointV1,
    pub last_success: Option<CatalogRefreshPointV1>,
    pub last_published: Option<CatalogRefreshPointV1>,
    pub last_nonempty: Option<CatalogRefreshPointV1>,
    pub pending_empty: Option<CatalogRefreshPointV1>,
}
