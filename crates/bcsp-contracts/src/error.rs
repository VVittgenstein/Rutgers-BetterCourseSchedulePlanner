use std::fmt;
use std::str::FromStr;

use serde::de::{DeserializeOwned, Error as _};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use uuid::{Uuid, Variant, Version};

use crate::{API_PROTOCOL_VERSION, ProtocolVersion};

macro_rules! define_api_error_codes {
    ($( $variant:ident => ($wire:literal, $message_key:literal) ),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
        #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
        pub enum ApiErrorCode {
            $( $variant, )+
        }

        impl ApiErrorCode {
            pub const ALL: &'static [Self] = &[$( Self::$variant, )+];

            pub const fn message_key(self) -> &'static str {
                match self {
                    $( Self::$variant => $message_key, )+
                }
            }

            pub const fn wire_name(self) -> &'static str {
                match self {
                    $( Self::$variant => $wire, )+
                }
            }
        }
    };
}

define_api_error_codes! {
    MalformedRequest => ("MALFORMED_REQUEST", "error.malformed_request"),
    UnsupportedProtocolVersion => (
        "UNSUPPORTED_PROTOCOL_VERSION",
        "error.unsupported_protocol_version"
    ),
    InvalidSectionKey => ("INVALID_SECTION_KEY", "error.invalid_section_key"),
    InvalidCourseGroupKey => (
        "INVALID_COURSE_GROUP_KEY",
        "error.invalid_course_group_key"
    ),
    InvalidCourseVariantKey => (
        "INVALID_COURSE_VARIANT_KEY",
        "error.invalid_course_variant_key"
    ),
    InvalidFilter => ("INVALID_FILTER", "error.invalid_filter"),
    SectionNotFound => ("SECTION_NOT_FOUND", "error.section_not_found"),
    SelectionLimitExceeded => (
        "SELECTION_LIMIT_EXCEEDED",
        "error.selection_limit_exceeded"
    ),
    UpstreamUnavailable => ("UPSTREAM_UNAVAILABLE", "error.upstream_unavailable"),
    AudioBlocked => ("AUDIO_BLOCKED", "error.audio_blocked"),
    InternalError => ("INTERNAL_ERROR", "error.internal"),
}

pub trait StableErrorCode: Copy + Eq + Serialize {
    fn message_key(self) -> &'static str;
    fn wire_name(self) -> &'static str;
}

pub trait StableErrorCodeSet: StableErrorCode + 'static {
    fn all() -> &'static [Self];
}

impl StableErrorCode for ApiErrorCode {
    fn message_key(self) -> &'static str {
        Self::message_key(self)
    }

    fn wire_name(self) -> &'static str {
        Self::wire_name(self)
    }
}

impl StableErrorCodeSet for ApiErrorCode {
    fn all() -> &'static [Self] {
        Self::ALL
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum TraceIdError {
    #[error("trace ID is not a UUID")]
    InvalidUuid,
    #[error("trace ID must use canonical lowercase hyphenated form")]
    NonCanonical,
    #[error("trace ID must be an RFC 4122 random UUID v4")]
    NotRandomV4,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TraceId(Uuid);

impl TraceId {
    pub fn try_from_uuid(value: Uuid) -> Result<Self, TraceIdError> {
        if value.get_variant() != Variant::RFC4122 || value.get_version() != Some(Version::Random) {
            return Err(TraceIdError::NotRandomV4);
        }
        Ok(Self(value))
    }

    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl TryFrom<Uuid> for TraceId {
    type Error = TraceIdError;

    fn try_from(value: Uuid) -> Result<Self, Self::Error> {
        Self::try_from_uuid(value)
    }
}

impl fmt::Display for TraceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.hyphenated().fmt(formatter)
    }
}

impl FromStr for TraceId {
    type Err = TraceIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let parsed = Uuid::parse_str(value).map_err(|_| TraceIdError::InvalidUuid)?;
        if parsed.hyphenated().to_string() != value {
            return Err(TraceIdError::NonCanonical);
        }
        Self::try_from_uuid(parsed)
    }
}

impl Serialize for TraceId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for TraceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(D::Error::custom)
    }
}

pub trait TraceIdSource {
    fn next_trace_id(&mut self) -> TraceId;
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DetailName(String);

impl DetailName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for DetailName {
    type Error = DetailNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().enumerate().all(|(index, byte)| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || (index > 0 && matches!(byte, b'.' | b'_' | b'-'))
            })
        {
            return Err(DetailNameError);
        }
        Ok(Self(value))
    }
}

impl TryFrom<&str> for DetailName {
    type Error = DetailNameError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_from(value.to_owned())
    }
}

impl Serialize for DetailName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for DetailName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_from(value).map_err(D::Error::custom)
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("invalid safe detail name")]
pub struct DetailNameError;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ApiErrorDetail {
    InvalidField { field: DetailName },
    Limit { name: DetailName, maximum: u32 },
    RetryAfterSeconds { seconds: u32 },
}

/// Generic error body for a separately versioned target-specific error set.
///
/// The shared manifest describes only [`ApiErrorBody`], the concrete alias
/// bound to [`ApiErrorCode`]. A target-specific `C` must own a distinct schema
/// and compatibility gate instead of silently extending the shared enum.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TypedApiErrorBody<C> {
    code: C,
    message_key: String,
    trace_id: TraceId,
    details: Vec<ApiErrorDetail>,
}

pub type ApiErrorBody = TypedApiErrorBody<ApiErrorCode>;

impl<C> TypedApiErrorBody<C>
where
    C: StableErrorCode,
{
    pub fn new(code: C, trace_id: TraceId) -> Self {
        Self {
            code,
            message_key: code.message_key().to_owned(),
            trace_id,
            details: Vec::new(),
        }
    }

    pub fn with_details(mut self, details: Vec<ApiErrorDetail>) -> Self {
        self.details = details;
        self
    }

    pub const fn code(&self) -> C {
        self.code
    }

    pub fn message_key(&self) -> &str {
        &self.message_key
    }

    pub const fn trace_id(&self) -> TraceId {
        self.trace_id
    }

    pub fn details(&self) -> &[ApiErrorDetail] {
        &self.details
    }
}

impl<'de, C> Deserialize<'de> for TypedApiErrorBody<C>
where
    C: StableErrorCode + Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        #[serde(bound(deserialize = "C: Deserialize<'de>"))]
        struct WireBody<C> {
            code: C,
            message_key: String,
            trace_id: TraceId,
            details: Vec<ApiErrorDetail>,
        }

        let wire = WireBody::<C>::deserialize(deserializer)?;
        if wire.message_key != wire.code.message_key() {
            return Err(D::Error::custom("error code/message key mismatch"));
        }
        Ok(Self {
            code: wire.code,
            message_key: wire.message_key,
            trace_id: wire.trace_id,
            details: wire.details,
        })
    }
}

/// Generic envelope for a separately versioned target-specific error set.
/// The shared wire contract uses the concrete [`ApiErrorEnvelope`] alias.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    bound(deserialize = "C: StableErrorCode + Deserialize<'de>")
)]
pub struct TypedApiErrorEnvelope<C> {
    protocol_version: ProtocolVersion,
    error: TypedApiErrorBody<C>,
}

pub type ApiErrorEnvelope = TypedApiErrorEnvelope<ApiErrorCode>;

impl<C> TypedApiErrorEnvelope<C> {
    pub fn new(error: TypedApiErrorBody<C>) -> Self {
        Self {
            protocol_version: API_PROTOCOL_VERSION,
            error,
        }
    }

    pub const fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }

    pub const fn error(&self) -> &TypedApiErrorBody<C> {
        &self.error
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ContractDecodeError {
    #[error("malformed typed contract payload")]
    MalformedRequest,
    #[error("unsupported protocol version")]
    UnsupportedProtocolVersion { observed: u16 },
}

impl ContractDecodeError {
    pub const fn code(self) -> ApiErrorCode {
        match self {
            Self::MalformedRequest => ApiErrorCode::MalformedRequest,
            Self::UnsupportedProtocolVersion { .. } => ApiErrorCode::UnsupportedProtocolVersion,
        }
    }

    pub const fn observed_protocol_version(self) -> Option<u16> {
        match self {
            Self::MalformedRequest => None,
            Self::UnsupportedProtocolVersion { observed } => Some(observed),
        }
    }
}

pub fn decode_versioned_envelope_json<T>(bytes: &[u8]) -> Result<T, ContractDecodeError>
where
    T: DeserializeOwned,
{
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct VersionProbe {
        protocol_version: u16,
    }

    let probe = serde_json::from_slice::<VersionProbe>(bytes)
        .map_err(|_| ContractDecodeError::MalformedRequest)?;
    ProtocolVersion::try_from(probe.protocol_version).map_err(|error| {
        ContractDecodeError::UnsupportedProtocolVersion {
            observed: error.observed(),
        }
    })?;
    serde_json::from_slice(bytes).map_err(|_| ContractDecodeError::MalformedRequest)
}
