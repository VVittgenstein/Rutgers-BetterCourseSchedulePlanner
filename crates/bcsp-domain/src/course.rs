use std::collections::BTreeSet;

use bcsp_contracts::{CourseGroupKey, CourseVariantKey, SectionKey};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DomainReasonCode {
    EmptyCourseGroup,
    VariantGroupMismatch,
    DuplicateVariantFingerprint,
    SectionScopeMismatch,
    DuplicateSection,
    SectionAcrossVariants,
    EmptyDuplicateSet,
    DuplicateKeyMismatch,
    ConflictingDuplicate,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum DomainModelError {
    #[error("course group must contain at least one variant")]
    EmptyCourseGroup,
    #[error("course variant belongs to a different group")]
    VariantGroupMismatch,
    #[error("course group contains a duplicate variant fingerprint")]
    DuplicateVariantFingerprint,
    #[error("section scope differs from its course variant scope")]
    SectionScopeMismatch,
    #[error("course variant contains a duplicate section")]
    DuplicateSection,
    #[error("the same section is attached to multiple variants")]
    SectionAcrossVariants,
    #[error("duplicate classification requires at least one record")]
    EmptyDuplicateSet,
    #[error("duplicate records do not share one complete section key")]
    DuplicateKeyMismatch,
    #[error("duplicate records have conflicting semantic fingerprints")]
    ConflictingDuplicate,
}

impl DomainModelError {
    pub const fn reason_code(self) -> DomainReasonCode {
        match self {
            Self::EmptyCourseGroup => DomainReasonCode::EmptyCourseGroup,
            Self::VariantGroupMismatch => DomainReasonCode::VariantGroupMismatch,
            Self::DuplicateVariantFingerprint => DomainReasonCode::DuplicateVariantFingerprint,
            Self::SectionScopeMismatch => DomainReasonCode::SectionScopeMismatch,
            Self::DuplicateSection => DomainReasonCode::DuplicateSection,
            Self::SectionAcrossVariants => DomainReasonCode::SectionAcrossVariants,
            Self::EmptyDuplicateSet => DomainReasonCode::EmptyDuplicateSet,
            Self::DuplicateKeyMismatch => DomainReasonCode::DuplicateKeyMismatch,
            Self::ConflictingDuplicate => DomainReasonCode::ConflictingDuplicate,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CourseVariant {
    key: CourseVariantKey,
    section_keys: Vec<SectionKey>,
}

impl CourseVariant {
    pub fn try_new(
        key: CourseVariantKey,
        mut section_keys: Vec<SectionKey>,
    ) -> Result<Self, DomainModelError> {
        if section_keys.iter().any(|section| {
            section.term() != key.group().term() || section.campus() != key.group().campus()
        }) {
            return Err(DomainModelError::SectionScopeMismatch);
        }

        section_keys.sort();
        if section_keys.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DomainModelError::DuplicateSection);
        }

        Ok(Self { key, section_keys })
    }

    pub const fn key(&self) -> &CourseVariantKey {
        &self.key
    }

    pub fn section_keys(&self) -> &[SectionKey] {
        &self.section_keys
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CourseGroup {
    key: CourseGroupKey,
    variants: Vec<CourseVariant>,
}

impl CourseGroup {
    pub fn try_new(
        key: CourseGroupKey,
        mut variants: Vec<CourseVariant>,
    ) -> Result<Self, DomainModelError> {
        if variants.is_empty() {
            return Err(DomainModelError::EmptyCourseGroup);
        }
        if variants.iter().any(|variant| variant.key().group() != &key) {
            return Err(DomainModelError::VariantGroupMismatch);
        }

        variants.sort_by(|left, right| left.key().cmp(right.key()));
        if variants
            .windows(2)
            .any(|pair| pair[0].key() == pair[1].key())
        {
            return Err(DomainModelError::DuplicateVariantFingerprint);
        }

        let mut attached_sections = BTreeSet::new();
        for section in variants.iter().flat_map(CourseVariant::section_keys) {
            if !attached_sections.insert(section) {
                return Err(DomainModelError::SectionAcrossVariants);
            }
        }

        Ok(Self { key, variants })
    }

    pub const fn key(&self) -> &CourseGroupKey {
        &self.key
    }

    pub fn variants(&self) -> &[CourseVariant] {
        &self.variants
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectionDuplicate<T> {
    section_key: SectionKey,
    semantic_fingerprint: T,
}

impl<T> SectionDuplicate<T> {
    pub const fn new(section_key: SectionKey, semantic_fingerprint: T) -> Self {
        Self {
            section_key,
            semantic_fingerprint,
        }
    }

    pub const fn section_key(&self) -> &SectionKey {
        &self.section_key
    }

    pub const fn semantic_fingerprint(&self) -> &T {
        &self.semantic_fingerprint
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DuplicateDisposition {
    Unique,
    AuditedEquivalentCollapse,
}

pub fn classify_section_duplicates<T, I>(
    records: I,
) -> Result<DuplicateDisposition, DomainModelError>
where
    T: Ord,
    I: IntoIterator<Item = SectionDuplicate<T>>,
{
    let records = records.into_iter().collect::<Vec<_>>();
    let Some(first) = records.first() else {
        return Err(DomainModelError::EmptyDuplicateSet);
    };
    if records
        .iter()
        .any(|record| record.section_key() != first.section_key())
    {
        return Err(DomainModelError::DuplicateKeyMismatch);
    }
    if records.len() == 1 {
        return Ok(DuplicateDisposition::Unique);
    }

    let distinct = records
        .iter()
        .map(SectionDuplicate::semantic_fingerprint)
        .collect::<BTreeSet<_>>()
        .len();
    if distinct == 1 {
        Ok(DuplicateDisposition::AuditedEquivalentCollapse)
    } else {
        Err(DomainModelError::ConflictingDuplicate)
    }
}
