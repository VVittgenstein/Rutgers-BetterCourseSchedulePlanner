use std::fmt;
use std::str::FromStr;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

pub(crate) const TERM_MAX_BYTES: usize = 16;
pub(crate) const SECTION_INDEX_WIDTH: usize = 5;
pub(crate) const CAMPUS_MAX_BYTES: usize = 32;
pub(crate) const COURSE_STRING_MAX_BYTES: usize = 64;
pub(crate) const FINGERPRINT_PREFIX: &str = "v1:";
pub(crate) const SHA256_HEX_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdentityErrorCode {
    Empty,
    WrongLength,
    TooLong,
    InvalidCharacter,
    NonCanonicalCase,
    WrongSegmentCount,
    InvalidFingerprint,
}

impl fmt::Display for IdentityErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "EMPTY",
            Self::WrongLength => "WRONG_LENGTH",
            Self::TooLong => "TOO_LONG",
            Self::InvalidCharacter => "INVALID_CHARACTER",
            Self::NonCanonicalCase => "NON_CANONICAL_CASE",
            Self::WrongSegmentCount => "WRONG_SEGMENT_COUNT",
            Self::InvalidFingerprint => "INVALID_FINGERPRINT",
        })
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("invalid {field}: {code}")]
pub struct IdentityError {
    field: &'static str,
    code: IdentityErrorCode,
}

impl IdentityError {
    const fn new(field: &'static str, code: IdentityErrorCode) -> Self {
        Self { field, code }
    }

    pub const fn field(&self) -> &'static str {
        self.field
    }

    pub const fn code(&self) -> IdentityErrorCode {
        self.code
    }
}

fn validate_term(value: &str) -> Result<(), IdentityError> {
    if value.is_empty() {
        return Err(IdentityError::new("term", IdentityErrorCode::Empty));
    }
    if value.len() > TERM_MAX_BYTES {
        return Err(IdentityError::new("term", IdentityErrorCode::TooLong));
    }
    if value.bytes().any(|byte| byte.is_ascii_lowercase()) {
        return Err(IdentityError::new(
            "term",
            IdentityErrorCode::NonCanonicalCase,
        ));
    }
    if !value.bytes().all(|byte| {
        byte.is_ascii_uppercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
    }) {
        return Err(IdentityError::new(
            "term",
            IdentityErrorCode::InvalidCharacter,
        ));
    }
    Ok(())
}

fn validate_section_index(value: &str) -> Result<(), IdentityError> {
    if value.len() != SECTION_INDEX_WIDTH {
        return Err(IdentityError::new("index", IdentityErrorCode::WrongLength));
    }
    if !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(IdentityError::new(
            "index",
            IdentityErrorCode::InvalidCharacter,
        ));
    }
    Ok(())
}

fn validate_campus(value: &str) -> Result<(), IdentityError> {
    if value.is_empty() {
        return Err(IdentityError::new("campus", IdentityErrorCode::Empty));
    }
    if value.len() > CAMPUS_MAX_BYTES {
        return Err(IdentityError::new("campus", IdentityErrorCode::TooLong));
    }
    if value.bytes().any(|byte| byte.is_ascii_lowercase()) {
        return Err(IdentityError::new(
            "campus",
            IdentityErrorCode::NonCanonicalCase,
        ));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(IdentityError::new(
            "campus",
            IdentityErrorCode::InvalidCharacter,
        ));
    }
    Ok(())
}

fn validate_course_string(value: &str) -> Result<(), IdentityError> {
    if value.is_empty() {
        return Err(IdentityError::new("courseString", IdentityErrorCode::Empty));
    }
    if value.len() > COURSE_STRING_MAX_BYTES {
        return Err(IdentityError::new(
            "courseString",
            IdentityErrorCode::TooLong,
        ));
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'.' | b'_' | b'-'))
    {
        return Err(IdentityError::new(
            "courseString",
            IdentityErrorCode::InvalidCharacter,
        ));
    }
    Ok(())
}

fn validate_fingerprint(value: &str) -> Result<(), IdentityError> {
    let Some(hex) = value.strip_prefix(FINGERPRINT_PREFIX) else {
        return Err(IdentityError::new(
            "fingerprint",
            IdentityErrorCode::InvalidFingerprint,
        ));
    };
    if hex.len() != SHA256_HEX_BYTES
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(IdentityError::new(
            "fingerprint",
            IdentityErrorCode::InvalidFingerprint,
        ));
    }
    Ok(())
}

macro_rules! validated_string_type {
    ($name:ident, $validator:ident) => {
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

        impl TryFrom<String> for $name {
            type Error = IdentityError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                $validator(&value)?;
                Ok(Self(value))
            }
        }

        impl TryFrom<&str> for $name {
            type Error = IdentityError;

            fn try_from(value: &str) -> Result<Self, Self::Error> {
                Self::try_from(value.to_owned())
            }
        }

        impl FromStr for $name {
            type Err = IdentityError;

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

validated_string_type!(TermId, validate_term);
validated_string_type!(CampusCode, validate_campus);
validated_string_type!(SectionIndex, validate_section_index);
validated_string_type!(CourseString, validate_course_string);
validated_string_type!(VariantFingerprint, validate_fingerprint);

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TermCampusKey {
    term: TermId,
    campus: CampusCode,
}

impl TermCampusKey {
    pub const fn new(term: TermId, campus: CampusCode) -> Self {
        Self { term, campus }
    }

    pub fn try_new(term: &str, campus: &str) -> Result<Self, IdentityError> {
        Ok(Self::new(term.try_into()?, campus.try_into()?))
    }

    pub const fn term(&self) -> &TermId {
        &self.term
    }

    pub const fn campus(&self) -> &CampusCode {
        &self.campus
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SectionKey {
    term: TermId,
    campus: CampusCode,
    index: SectionIndex,
}

impl SectionKey {
    pub const fn new(term: TermId, campus: CampusCode, index: SectionIndex) -> Self {
        Self {
            term,
            campus,
            index,
        }
    }

    pub fn try_new(term: &str, campus: &str, index: &str) -> Result<Self, IdentityError> {
        Ok(Self::new(
            term.try_into()?,
            campus.try_into()?,
            index.try_into()?,
        ))
    }

    pub const fn term(&self) -> &TermId {
        &self.term
    }

    pub const fn campus(&self) -> &CampusCode {
        &self.campus
    }

    pub const fn index(&self) -> &SectionIndex {
        &self.index
    }

    pub fn target(&self) -> TermCampusKey {
        TermCampusKey::new(self.term.clone(), self.campus.clone())
    }
}

impl fmt::Display for SectionKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}/{}/{}", self.term, self.campus, self.index)
    }
}

impl FromStr for SectionKey {
    type Err = IdentityError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let segments = value.split('/').collect::<Vec<_>>();
        if segments.len() != 3 {
            return Err(IdentityError::new(
                "sectionKey",
                IdentityErrorCode::WrongSegmentCount,
            ));
        }
        Self::try_new(segments[0], segments[1], segments[2])
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CourseGroupKey {
    term: TermId,
    campus: CampusCode,
    course_string: CourseString,
}

impl CourseGroupKey {
    pub const fn new(term: TermId, campus: CampusCode, course_string: CourseString) -> Self {
        Self {
            term,
            campus,
            course_string,
        }
    }

    pub fn try_new(term: &str, campus: &str, course_string: &str) -> Result<Self, IdentityError> {
        Ok(Self::new(
            term.try_into()?,
            campus.try_into()?,
            course_string.try_into()?,
        ))
    }

    pub const fn term(&self) -> &TermId {
        &self.term
    }

    pub const fn campus(&self) -> &CampusCode {
        &self.campus
    }

    pub const fn course_string(&self) -> &CourseString {
        &self.course_string
    }

    pub fn target(&self) -> TermCampusKey {
        TermCampusKey::new(self.term.clone(), self.campus.clone())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CourseVariantKey {
    group: CourseGroupKey,
    fingerprint: VariantFingerprint,
}

impl CourseVariantKey {
    pub const fn new(group: CourseGroupKey, fingerprint: VariantFingerprint) -> Self {
        Self { group, fingerprint }
    }

    pub fn try_new(group: CourseGroupKey, fingerprint: &str) -> Result<Self, IdentityError> {
        Ok(Self::new(group, fingerprint.try_into()?))
    }

    pub const fn group(&self) -> &CourseGroupKey {
        &self.group
    }

    pub const fn fingerprint(&self) -> &VariantFingerprint {
        &self.fingerprint
    }
}
