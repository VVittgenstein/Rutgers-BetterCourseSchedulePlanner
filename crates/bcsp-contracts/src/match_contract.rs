use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MatchOutcome {
    Match,
    Uncertain,
    NoMatch,
}

impl MatchOutcome {
    pub const ALL: &'static [Self] = &[Self::Match, Self::Uncertain, Self::NoMatch];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Match => "MATCH",
            Self::Uncertain => "UNCERTAIN",
            Self::NoMatch => "NO_MATCH",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MatchReasonCode {
    KnownMismatch,
    MissingReliableData,
    UnknownValue,
    Tba,
    ConflictingEvidence,
    InvalidValue,
    SourceUnavailable,
}

impl MatchReasonCode {
    pub const ALL: &'static [Self] = &[
        Self::KnownMismatch,
        Self::MissingReliableData,
        Self::UnknownValue,
        Self::Tba,
        Self::ConflictingEvidence,
        Self::InvalidValue,
        Self::SourceUnavailable,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::KnownMismatch => "KNOWN_MISMATCH",
            Self::MissingReliableData => "MISSING_RELIABLE_DATA",
            Self::UnknownValue => "UNKNOWN_VALUE",
            Self::Tba => "TBA",
            Self::ConflictingEvidence => "CONFLICTING_EVIDENCE",
            Self::InvalidValue => "INVALID_VALUE",
            Self::SourceUnavailable => "SOURCE_UNAVAILABLE",
        }
    }

    pub const fn is_known_mismatch(self) -> bool {
        matches!(self, Self::KnownMismatch)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReasonField(String);

impl ReasonField {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ReasonField {
    type Error = MatchExplanationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().enumerate().all(|(index, byte)| {
                byte.is_ascii_alphanumeric() || (index > 0 && matches!(byte, b'.' | b'_' | b'-'))
            })
        {
            return Err(MatchExplanationError::InvalidReasonField);
        }
        Ok(Self(value))
    }
}

impl TryFrom<&str> for ReasonField {
    type Error = MatchExplanationError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::try_from(value.to_owned())
    }
}

impl Serialize for ReasonField {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for ReasonField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_from(value).map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MatchReason {
    code: MatchReasonCode,
    field: ReasonField,
}

impl MatchReason {
    pub const fn new(code: MatchReasonCode, field: ReasonField) -> Self {
        Self { code, field }
    }

    pub const fn code(&self) -> MatchReasonCode {
        self.code
    }

    pub const fn field(&self) -> &ReasonField {
        &self.field
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum MatchExplanationError {
    #[error("match reason field is invalid")]
    InvalidReasonField,
    #[error("MATCH cannot carry mismatch or uncertainty reasons")]
    MatchHasReasons,
    #[error("UNCERTAIN requires at least one non-mismatch reason")]
    UncertainReasonContract,
    #[error("NO_MATCH requires a known mismatch reason")]
    NoMatchReasonContract,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchExplanation {
    outcome: MatchOutcome,
    reasons: Vec<MatchReason>,
}

impl MatchExplanation {
    pub fn try_new(
        outcome: MatchOutcome,
        reasons: Vec<MatchReason>,
    ) -> Result<Self, MatchExplanationError> {
        match outcome {
            MatchOutcome::Match if !reasons.is_empty() => {
                return Err(MatchExplanationError::MatchHasReasons);
            }
            MatchOutcome::Uncertain
                if reasons.is_empty()
                    || reasons
                        .iter()
                        .any(|reason| reason.code().is_known_mismatch()) =>
            {
                return Err(MatchExplanationError::UncertainReasonContract);
            }
            MatchOutcome::NoMatch
                if !reasons
                    .iter()
                    .any(|reason| reason.code().is_known_mismatch()) =>
            {
                return Err(MatchExplanationError::NoMatchReasonContract);
            }
            _ => {}
        }
        Ok(Self { outcome, reasons })
    }

    pub fn matched() -> Self {
        Self {
            outcome: MatchOutcome::Match,
            reasons: Vec::new(),
        }
    }

    pub const fn outcome(&self) -> MatchOutcome {
        self.outcome
    }

    pub fn reasons(&self) -> &[MatchReason] {
        &self.reasons
    }
}

impl<'de> Deserialize<'de> for MatchExplanation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct WireExplanation {
            outcome: MatchOutcome,
            reasons: Vec<MatchReason>,
        }

        let wire = WireExplanation::deserialize(deserializer)?;
        Self::try_new(wire.outcome, wire.reasons).map_err(D::Error::custom)
    }
}
