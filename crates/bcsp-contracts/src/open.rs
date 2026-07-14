use std::fmt;
use std::num::NonZeroU64;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use time::OffsetDateTime;

use crate::{
    CampusCode, CatalogContentVersion, IdentityError, SectionKey, TermCampusKey, TermId, TraceId,
};

pub const OPEN_CONTRACT_VERSION: OpenContractVersion = OpenContractVersion::V1;

const SHA256_HEX_BYTES: usize = 64;
const OPEN_HTTP_HEADER_VALUE_MAX_BYTES: usize = 4_096;

/// Version of the shared Open attempt, observation, scheduler, and status contracts.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum OpenContractVersion {
    V1,
}

impl OpenContractVersion {
    pub const fn as_u16(self) -> u16 {
        match self {
            Self::V1 => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("unsupported Open contract version")]
pub struct OpenContractVersionError {
    observed: u16,
}

impl OpenContractVersionError {
    pub const fn observed(self) -> u16 {
        self.observed
    }
}

impl TryFrom<u16> for OpenContractVersion {
    type Error = OpenContractVersionError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::V1),
            observed => Err(OpenContractVersionError { observed }),
        }
    }
}

impl Serialize for OpenContractVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u16(self.as_u16())
    }
}

impl<'de> Deserialize<'de> for OpenContractVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u16::deserialize(deserializer)?;
        Self::try_from(value).map_err(D::Error::custom)
    }
}

/// Fixed `(term,campus)` identity for one independently committed Open set.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct OpenBatchKey(TermCampusKey);

impl OpenBatchKey {
    pub const fn new(term: TermId, campus: CampusCode) -> Self {
        Self(TermCampusKey::new(term, campus))
    }

    pub fn try_new(term: &str, campus: &str) -> Result<Self, IdentityError> {
        TermCampusKey::try_new(term, campus).map(Self)
    }

    pub const fn term(&self) -> &TermId {
        self.0.term()
    }

    pub const fn campus(&self) -> &CampusCode {
        self.0.campus()
    }

    pub fn target(&self) -> TermCampusKey {
        self.0.clone()
    }
}

impl From<TermCampusKey> for OpenBatchKey {
    fn from(value: TermCampusKey) -> Self {
        Self(value)
    }
}

impl From<OpenBatchKey> for TermCampusKey {
    fn from(value: OpenBatchKey) -> Self {
        value.0
    }
}

impl fmt::Display for OpenBatchKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}", self.term(), self.campus())
    }
}

/// Nonzero target-scoped attempt or observation sequence.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct OpenSequence(NonZeroU64);

impl OpenSequence {
    pub const fn new(value: NonZeroU64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl TryFrom<u64> for OpenSequence {
    type Error = OpenSequenceError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        NonZeroU64::new(value).map(Self).ok_or(OpenSequenceError)
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("Open sequence must be nonzero")]
pub struct OpenSequenceError;

macro_rules! open_hash_type {
    ($name:ident, $error:ident, $message:literal) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_inner(self) -> String {
                self.0
            }
        }

        #[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
        #[error($message)]
        pub struct $error;

        impl TryFrom<String> for $name {
            type Error = $error;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                if value.len() == SHA256_HEX_BYTES
                    && value.bytes().all(|byte| byte.is_ascii_hexdigit())
                    && !value.bytes().any(|byte| byte.is_ascii_uppercase())
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

open_hash_type!(
    OpenCanonicalSetHash,
    OpenCanonicalSetHashError,
    "Open canonical set hash must be exactly 64 lowercase hexadecimal characters"
);
open_hash_type!(
    OpenStateHash,
    OpenStateHashError,
    "Open state hash must be exactly 64 lowercase hexadecimal characters"
);
open_hash_type!(
    OpenDecodedBodySha256,
    OpenDecodedBodySha256Error,
    "Open decoded body SHA-256 must be exactly 64 lowercase hexadecimal characters"
);

/// Bounded, canonical HTTP response-header value retained for audit without a
/// response body. Values are already unfolded and trimmed by the HTTP client.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OpenHttpHeaderValue(String);

impl OpenHttpHeaderValue {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error(
    "Open HTTP header value must be nonempty, trim-stable, at most 4096 bytes, and contain only visible ASCII or horizontal tabs"
)]
pub struct OpenHttpHeaderValueError;

impl TryFrom<String> for OpenHttpHeaderValue {
    type Error = OpenHttpHeaderValueError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty()
            || value.len() > OPEN_HTTP_HEADER_VALUE_MAX_BYTES
            || value.trim() != value
            || !value
                .bytes()
                .all(|byte| byte == b'\t' || (b' '..=b'~').contains(&byte))
        {
            return Err(OpenHttpHeaderValueError);
        }
        Ok(Self(value))
    }
}

impl TryFrom<&str> for OpenHttpHeaderValue {
    type Error = OpenHttpHeaderValueError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_from(value.to_owned())
    }
}

impl Serialize for OpenHttpHeaderValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for OpenHttpHeaderValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_from(value).map_err(D::Error::custom)
    }
}

/// Rutgers-day key used for durable target-request counters.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RutgersDay(String);

impl RutgersDay {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("Rutgers day must be a valid YYYY-MM-DD calendar date")]
pub struct RutgersDayError;

impl TryFrom<String> for RutgersDay {
    type Error = RutgersDayError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let bytes = value.as_bytes();
        if bytes.len() != 10
            || bytes[4] != b'-'
            || bytes[7] != b'-'
            || bytes
                .iter()
                .enumerate()
                .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
        {
            return Err(RutgersDayError);
        }

        let year = value[0..4].parse::<u16>().map_err(|_| RutgersDayError)?;
        let month = value[5..7].parse::<u8>().map_err(|_| RutgersDayError)?;
        let day = value[8..10].parse::<u8>().map_err(|_| RutgersDayError)?;
        let leap =
            year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
        let maximum = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if leap => 29,
            2 => 28,
            _ => return Err(RutgersDayError),
        };
        if day == 0 || day > maximum {
            return Err(RutgersDayError);
        }
        Ok(Self(value))
    }
}

impl TryFrom<&str> for RutgersDay {
    type Error = RutgersDayError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_from(value.to_owned())
    }
}

impl Serialize for RutgersDay {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for RutgersDay {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_from(value).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum RutgersDayTimezone {
    #[serde(rename = "America/New_York")]
    AmericaNewYork,
}

impl RutgersDayTimezone {
    pub const ALL: &'static [Self] = &[Self::AmericaNewYork];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::AmericaNewYork => "America/New_York",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OpenRefreshClassification {
    InFlight,
    ValidApplied,
    ValidEmptyNoRows,
    UnsafeEmpty,
    UnsafeZeroIntersection,
    StaleCatalogRace,
    Failed,
}

impl OpenRefreshClassification {
    pub const ALL: &'static [Self] = &[
        Self::InFlight,
        Self::ValidApplied,
        Self::ValidEmptyNoRows,
        Self::UnsafeEmpty,
        Self::UnsafeZeroIntersection,
        Self::StaleCatalogRace,
        Self::Failed,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::InFlight => "IN_FLIGHT",
            Self::ValidApplied => "VALID_APPLIED",
            Self::ValidEmptyNoRows => "VALID_EMPTY_NO_ROWS",
            Self::UnsafeEmpty => "UNSAFE_EMPTY",
            Self::UnsafeZeroIntersection => "UNSAFE_ZERO_INTERSECTION",
            Self::StaleCatalogRace => "STALE_CATALOG_RACE",
            Self::Failed => "FAILED",
        }
    }
}

/// Successful target-observation classes. Failure has no representation here.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OpenAppliedClassification {
    ValidApplied,
    ValidEmptyNoRows,
}

impl OpenAppliedClassification {
    pub const ALL: &'static [Self] = &[Self::ValidApplied, Self::ValidEmptyNoRows];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::ValidApplied => "VALID_APPLIED",
            Self::ValidEmptyNoRows => "VALID_EMPTY_NO_ROWS",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OpenFailureClass {
    Network,
    Timeout,
    #[serde(rename = "HTTP_408")]
    Http408,
    #[serde(rename = "HTTP_429")]
    Http429,
    Forbidden,
    #[serde(rename = "OTHER_HTTP_4XX")]
    OtherHttp4xx,
    #[serde(rename = "HTTP_5XX")]
    Http5xx,
    OffOriginRedirect,
    NonJson,
    Oversize,
    SchemaViolation,
    InvalidValue,
    Persist,
}

impl OpenFailureClass {
    pub const ALL: &'static [Self] = &[
        Self::Network,
        Self::Timeout,
        Self::Http408,
        Self::Http429,
        Self::Forbidden,
        Self::OtherHttp4xx,
        Self::Http5xx,
        Self::OffOriginRedirect,
        Self::NonJson,
        Self::Oversize,
        Self::SchemaViolation,
        Self::InvalidValue,
        Self::Persist,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Network => "NETWORK",
            Self::Timeout => "TIMEOUT",
            Self::Http408 => "HTTP_408",
            Self::Http429 => "HTTP_429",
            Self::Forbidden => "FORBIDDEN",
            Self::OtherHttp4xx => "OTHER_HTTP_4XX",
            Self::Http5xx => "HTTP_5XX",
            Self::OffOriginRedirect => "OFF_ORIGIN_REDIRECT",
            Self::NonJson => "NON_JSON",
            Self::Oversize => "OVERSIZE",
            Self::SchemaViolation => "SCHEMA_VIOLATION",
            Self::InvalidValue => "INVALID_VALUE",
            Self::Persist => "PERSIST",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OpenState {
    Open,
    Closed,
    Unknown,
}

impl OpenState {
    pub const ALL: &'static [Self] = &[Self::Open, Self::Closed, Self::Unknown];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::Closed => "CLOSED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OpenFreshnessState {
    Unknown,
    Fresh,
    Stale,
}

impl OpenFreshnessState {
    pub const ALL: &'static [Self] = &[Self::Unknown, Self::Fresh, Self::Stale];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Unknown => "UNKNOWN",
            Self::Fresh => "FRESH",
            Self::Stale => "STALE",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OpenUncertaintyReason {
    NeverObserved,
    StaleLastKnownGood,
    LatestAttemptFailed,
    LatestAttemptUnsafe,
    StaleCatalogRace,
    CatalogVersionUnavailable,
}

impl OpenUncertaintyReason {
    pub const ALL: &'static [Self] = &[
        Self::NeverObserved,
        Self::StaleLastKnownGood,
        Self::LatestAttemptFailed,
        Self::LatestAttemptUnsafe,
        Self::StaleCatalogRace,
        Self::CatalogVersionUnavailable,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::NeverObserved => "NEVER_OBSERVED",
            Self::StaleLastKnownGood => "STALE_LAST_KNOWN_GOOD",
            Self::LatestAttemptFailed => "LATEST_ATTEMPT_FAILED",
            Self::LatestAttemptUnsafe => "LATEST_ATTEMPT_UNSAFE",
            Self::StaleCatalogRace => "STALE_CATALOG_RACE",
            Self::CatalogVersionUnavailable => "CATALOG_VERSION_UNAVAILABLE",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OpenSchedulerLane {
    General,
    ActiveWatch,
    FirstLoad,
    ManualRefresh,
    CatalogRaceRecheck,
}

impl OpenSchedulerLane {
    pub const ALL: &'static [Self] = &[
        Self::General,
        Self::ActiveWatch,
        Self::FirstLoad,
        Self::ManualRefresh,
        Self::CatalogRaceRecheck,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::General => "GENERAL",
            Self::ActiveWatch => "ACTIVE_WATCH",
            Self::FirstLoad => "FIRST_LOAD",
            Self::ManualRefresh => "MANUAL_REFRESH",
            Self::CatalogRaceRecheck => "CATALOG_RACE_RECHECK",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OpenCircuitState {
    Closed,
    RetryAfter,
    FatalDiagnostic,
}

impl OpenCircuitState {
    pub const ALL: &'static [Self] = &[Self::Closed, Self::RetryAfter, Self::FatalDiagnostic];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Closed => "CLOSED",
            Self::RetryAfter => "RETRY_AFTER",
            Self::FatalDiagnostic => "FATAL_DIAGNOSTIC",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OpenCircuitReason {
    RateLimited,
    Forbidden,
    #[serde(rename = "OTHER_HTTP_4XX")]
    OtherHttp4xx,
    OffOriginRedirect,
    SameOriginRedirect,
    InvalidRedirect,
    NonJson,
    InvalidContentType,
    SchemaViolation,
    InvalidValue,
    Oversize,
}

impl OpenCircuitReason {
    pub const ALL: &'static [Self] = &[
        Self::RateLimited,
        Self::Forbidden,
        Self::OtherHttp4xx,
        Self::OffOriginRedirect,
        Self::SameOriginRedirect,
        Self::InvalidRedirect,
        Self::NonJson,
        Self::InvalidContentType,
        Self::SchemaViolation,
        Self::InvalidValue,
        Self::Oversize,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::RateLimited => "RATE_LIMITED",
            Self::Forbidden => "FORBIDDEN",
            Self::OtherHttp4xx => "OTHER_HTTP_4XX",
            Self::OffOriginRedirect => "OFF_ORIGIN_REDIRECT",
            Self::SameOriginRedirect => "SAME_ORIGIN_REDIRECT",
            Self::InvalidRedirect => "INVALID_REDIRECT",
            Self::NonJson => "NON_JSON",
            Self::InvalidContentType => "INVALID_CONTENT_TYPE",
            Self::SchemaViolation => "SCHEMA_VIOLATION",
            Self::InvalidValue => "INVALID_VALUE",
            Self::Oversize => "OVERSIZE",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenPullCountsV1 {
    pub attempted: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub empty: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCounterSnapshotV1 {
    /// Present for the Windows local runtime and omitted by the public runtime.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_counts: Option<OpenPullCountsV1>,
    pub today_counts: OpenPullCountsV1,
    pub rutgers_day: RutgersDay,
    pub day_timezone: RutgersDayTimezone,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenFreshnessV1 {
    pub state: OpenFreshnessState,
    #[serde(with = "time::serde::rfc3339::option")]
    pub observed_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub fresh_until: Option<OffsetDateTime>,
    pub last_known_good_age_seconds: Option<u64>,
    pub uncertainty: Option<OpenUncertaintyReason>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenHttpMetadataV1 {
    #[serde(deserialize_with = "deserialize_optional_http_status")]
    pub status_code: Option<u16>,
    pub decoded_bytes: Option<u64>,
    pub decoded_body_sha256: Option<OpenDecodedBodySha256>,
    pub content_type: Option<OpenHttpHeaderValue>,
    pub etag: Option<OpenHttpHeaderValue>,
    pub cache_control: Option<OpenHttpHeaderValue>,
    pub date: Option<OpenHttpHeaderValue>,
    pub age_seconds: Option<u64>,
    pub last_modified: Option<OpenHttpHeaderValue>,
    pub retry_after: Option<OpenHttpHeaderValue>,
}

fn deserialize_optional_http_status<'de, D>(deserializer: D) -> Result<Option<u16>, D::Error>
where
    D: Deserializer<'de>,
{
    let status = Option::<u16>::deserialize(deserializer)?;
    if status.is_some_and(|value| !(100..=599).contains(&value)) {
        return Err(D::Error::custom(
            "Open HTTP status code must be between 100 and 599",
        ));
    }
    Ok(status)
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenReconcileCountsV1 {
    pub canonical_open_indexes: u64,
    pub duplicate_indexes: u64,
    pub catalog_sections: u64,
    pub matched_open_sections: u64,
    pub closed_sections: u64,
    pub orphan_indexes: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAttemptScheduleV1 {
    pub lane: OpenSchedulerLane,
    pub requested_general_interval_seconds: u32,
    pub requested_effective_interval_seconds: u32,
    #[serde(with = "time::serde::rfc3339")]
    pub due_at: OffsetDateTime,
    pub scheduler_lag_milliseconds: u64,
    pub actual_start_to_start_interval_milliseconds: Option<u64>,
}

/// One actual request start. A failed or unsafe attempt ends here and does not
/// produce either of the valid-observation DTOs below.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenPullAttemptV1 {
    pub contract_version: OpenContractVersion,
    pub attempt_id: TraceId,
    pub batch: OpenBatchKey,
    pub attempt_sequence: OpenSequence,
    pub catalog_content_version: CatalogContentVersion,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub completed_at: Option<OffsetDateTime>,
    pub classification: OpenRefreshClassification,
    pub failure_class: Option<OpenFailureClass>,
    pub http: OpenHttpMetadataV1,
    pub canonical_set_hash: Option<OpenCanonicalSetHash>,
    pub state_hash: Option<OpenStateHash>,
    pub counts: OpenReconcileCountsV1,
    pub schedule: OpenAttemptScheduleV1,
    pub last_known_good_age_seconds: Option<u64>,
}

/// One valid, atomically applied target observation. Its closed enum cannot
/// represent failure, unsafe empty, zero intersection, or a Catalog race.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRefreshObservationV1 {
    pub contract_version: OpenContractVersion,
    pub observation_id: TraceId,
    pub batch: OpenBatchKey,
    pub attempt_sequence: OpenSequence,
    pub observation_sequence: OpenSequence,
    pub catalog_content_version: CatalogContentVersion,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub completed_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    pub classification: OpenAppliedClassification,
    pub http: OpenHttpMetadataV1,
    pub canonical_set_hash: OpenCanonicalSetHash,
    pub state_hash: OpenStateHash,
    pub counts: OpenReconcileCountsV1,
    pub schedule: OpenAttemptScheduleV1,
    pub body_changed: bool,
    pub state_changed: bool,
    pub last_known_good_age_seconds: u64,
}

/// Per-watched-Section event emitted only after a valid target reconcile.
/// `UNKNOWN` is reserved for a truthful valid unknown, never for request failure.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenObservationV1 {
    pub contract_version: OpenContractVersion,
    pub observation_id: TraceId,
    pub refresh_observation_id: TraceId,
    batch: OpenBatchKey,
    section_key: SectionKey,
    pub pull_sequence: OpenSequence,
    pub catalog_content_version: CatalogContentVersion,
    pub state: OpenState,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub fresh_until: OffsetDateTime,
    pub scheduler_lag_milliseconds: u64,
    pub counter_snapshot: OpenCounterSnapshotV1,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("Open observation batch must match the Section target")]
pub struct OpenObservationTargetMismatchError;

impl OpenObservationV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        contract_version: OpenContractVersion,
        observation_id: TraceId,
        refresh_observation_id: TraceId,
        batch: OpenBatchKey,
        section_key: SectionKey,
        pull_sequence: OpenSequence,
        catalog_content_version: CatalogContentVersion,
        state: OpenState,
        observed_at: OffsetDateTime,
        fresh_until: OffsetDateTime,
        scheduler_lag_milliseconds: u64,
        counter_snapshot: OpenCounterSnapshotV1,
    ) -> Result<Self, OpenObservationTargetMismatchError> {
        if batch.target() != section_key.target() {
            return Err(OpenObservationTargetMismatchError);
        }
        Ok(Self {
            contract_version,
            observation_id,
            refresh_observation_id,
            batch,
            section_key,
            pull_sequence,
            catalog_content_version,
            state,
            observed_at,
            fresh_until,
            scheduler_lag_milliseconds,
            counter_snapshot,
        })
    }

    pub const fn batch(&self) -> &OpenBatchKey {
        &self.batch
    }

    pub const fn section_key(&self) -> &SectionKey {
        &self.section_key
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenObservationWireV1 {
    contract_version: OpenContractVersion,
    observation_id: TraceId,
    refresh_observation_id: TraceId,
    batch: OpenBatchKey,
    section_key: SectionKey,
    pull_sequence: OpenSequence,
    catalog_content_version: CatalogContentVersion,
    state: OpenState,
    #[serde(with = "time::serde::rfc3339")]
    observed_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    fresh_until: OffsetDateTime,
    scheduler_lag_milliseconds: u64,
    counter_snapshot: OpenCounterSnapshotV1,
}

impl<'de> Deserialize<'de> for OpenObservationV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = OpenObservationWireV1::deserialize(deserializer)?;
        Self::try_new(
            wire.contract_version,
            wire.observation_id,
            wire.refresh_observation_id,
            wire.batch,
            wire.section_key,
            wire.pull_sequence,
            wire.catalog_content_version,
            wire.state,
            wire.observed_at,
            wire.fresh_until,
            wire.scheduler_lag_milliseconds,
            wire.counter_snapshot,
        )
        .map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenAttemptPointV1 {
    pub attempt_sequence: OpenSequence,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub completed_at: Option<OffsetDateTime>,
    pub classification: OpenRefreshClassification,
    pub failure_class: Option<OpenFailureClass>,
}

/// Most recent failed request retained independently from `latest_attempt`, so
/// a later success cannot erase the failure timestamp and class from status.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenFailurePointV1 {
    pub attempt_sequence: OpenSequence,
    #[serde(with = "time::serde::rfc3339")]
    pub failed_at: OffsetDateTime,
    pub failure_class: OpenFailureClass,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenObservationPointV1 {
    pub observation_id: TraceId,
    pub observation_sequence: OpenSequence,
    pub catalog_content_version: CatalogContentVersion,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    pub canonical_set_hash: OpenCanonicalSetHash,
    pub state_hash: OpenStateHash,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCircuitStatusV1 {
    pub state: OpenCircuitState,
    pub reason: Option<OpenCircuitReason>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub opened_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub retry_at: Option<OffsetDateTime>,
    pub diagnostic_recheck_required: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSchedulerStatusV1 {
    pub lane: OpenSchedulerLane,
    pub requested_general_interval_seconds: u32,
    pub requested_effective_interval_seconds: u32,
    pub active_watch_count: u64,
    #[serde(with = "time::serde::rfc3339::option")]
    pub next_due_at: Option<OffsetDateTime>,
    pub in_flight: bool,
    pub scheduler_lag_milliseconds: u64,
    pub actual_start_to_start_interval_milliseconds: Option<u64>,
    pub failure_streak: u32,
}

/// Additive target-level status projection for refresh UI and diagnostics.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenRefreshStatusV1 {
    pub contract_version: OpenContractVersion,
    pub batch: OpenBatchKey,
    pub catalog_content_version: Option<CatalogContentVersion>,
    pub latest_attempt: Option<OpenAttemptPointV1>,
    pub latest_failure: Option<OpenFailurePointV1>,
    pub last_valid_observation: Option<OpenObservationPointV1>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_body_change_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_state_change_at: Option<OffsetDateTime>,
    pub freshness: OpenFreshnessV1,
    pub scheduler: OpenSchedulerStatusV1,
    pub circuit: OpenCircuitStatusV1,
    pub counter_snapshot: OpenCounterSnapshotV1,
}

/// Additive per-Section current-state projection. A stale known state remains
/// visible with explicit freshness and uncertainty instead of becoming UNKNOWN.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenSectionStatusV1 {
    pub contract_version: OpenContractVersion,
    pub section_key: SectionKey,
    pub state: OpenState,
    pub last_observation_id: Option<TraceId>,
    pub catalog_content_version: Option<CatalogContentVersion>,
    pub freshness: OpenFreshnessV1,
    pub scheduler_lag_milliseconds: u64,
    pub counter_snapshot: OpenCounterSnapshotV1,
}

/// Shared status request. Client input remains strict while the response types
/// deliberately remain additive.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenStatusRequestV1 {
    pub contract_version: OpenContractVersion,
    pub batch: OpenBatchKey,
}

impl OpenStatusRequestV1 {
    pub fn new(batch: OpenBatchKey) -> Self {
        Self {
            contract_version: OPEN_CONTRACT_VERSION,
            batch,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenSectionStatusRequestV1 {
    pub contract_version: OpenContractVersion,
    pub section_key: SectionKey,
}

impl OpenSectionStatusRequestV1 {
    pub fn new(section_key: SectionKey) -> Self {
        Self {
            contract_version: OPEN_CONTRACT_VERSION,
            section_key,
        }
    }
}
