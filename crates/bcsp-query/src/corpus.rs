use std::collections::{BTreeMap, BTreeSet};

use bcsp_contracts::{
    CatalogContentVersion, CatalogFieldKnowledge, CatalogFieldPresence, CatalogOccurrenceKeyV1,
    CourseGroupKey, CourseVariantKey, NormalizedCatalogV1, NormalizedCourseGroupV1,
    NormalizedCourseVariantV1, NormalizedOccurrenceV1, NormalizedSectionV1, SectionKey,
    TermCampusKey,
};
use thiserror::Error;

/// Validated, read-only view over one term's target snapshots.
///
/// A corpus intentionally accepts multiple campus snapshots. Section indexes
/// are therefore never treated as globally unique; the full `SectionKey` is
/// used for every relationship and lookup.
#[derive(Debug)]
pub struct CatalogCorpus<'a> {
    term: &'a str,
    target_versions: BTreeMap<&'a TermCampusKey, CatalogContentVersion>,
    groups: BTreeMap<&'a CourseGroupKey, &'a NormalizedCourseGroupV1>,
    variants: BTreeMap<&'a CourseVariantKey, &'a NormalizedCourseVariantV1>,
    sections: BTreeMap<&'a SectionKey, &'a NormalizedSectionV1>,
    occurrences: BTreeMap<&'a CatalogOccurrenceKeyV1, &'a NormalizedOccurrenceV1>,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CorpusError {
    #[error("catalog corpus must contain at least one target snapshot")]
    Empty,
    #[error("all catalog target snapshots in a corpus must have one term")]
    MixedTerms,
    #[error("catalog corpus contains the same target snapshot more than once")]
    DuplicateTarget,
    #[error("catalog entity identity does not match its target snapshot")]
    TargetMismatch,
    #[error("catalog corpus contains a duplicate course group key")]
    DuplicateCourseGroup,
    #[error("catalog corpus contains a duplicate course variant key")]
    DuplicateCourseVariant,
    #[error("catalog corpus contains a duplicate section key")]
    DuplicateSection,
    #[error("catalog corpus contains a duplicate occurrence key")]
    DuplicateOccurrence,
    #[error("a course group references an unknown or foreign variant")]
    InvalidGroupVariantReference,
    #[error("a course variant references an unknown or foreign section")]
    InvalidVariantSectionReference,
    #[error("a section references an unknown or foreign occurrence")]
    InvalidSectionOccurrenceReference,
    #[error("an occurrence belongs to an unknown section")]
    OrphanOccurrence,
}

impl<'a> CatalogCorpus<'a> {
    pub fn try_new(catalogs: &'a [NormalizedCatalogV1]) -> Result<Self, CorpusError> {
        let Some(first) = catalogs.first() else {
            return Err(CorpusError::Empty);
        };
        let term = first.target.term().as_str();
        if catalogs
            .iter()
            .any(|catalog| catalog.target.term().as_str() != term)
        {
            return Err(CorpusError::MixedTerms);
        }

        let mut groups = BTreeMap::new();
        let mut variants = BTreeMap::new();
        let mut sections = BTreeMap::new();
        let mut occurrences = BTreeMap::new();
        let mut targets = BTreeSet::new();
        let mut target_versions = BTreeMap::new();
        for catalog in catalogs {
            if !targets.insert(&catalog.target) {
                return Err(CorpusError::DuplicateTarget);
            }
            target_versions.insert(&catalog.target, catalog.content_version);
            for group in &catalog.course_groups {
                if group.key.target() != catalog.target {
                    return Err(CorpusError::TargetMismatch);
                }
                if groups.insert(&group.key, group).is_some() {
                    return Err(CorpusError::DuplicateCourseGroup);
                }
            }
            for variant in &catalog.course_variants {
                if variant.key.group().target() != catalog.target {
                    return Err(CorpusError::TargetMismatch);
                }
                if variants.insert(&variant.key, variant).is_some() {
                    return Err(CorpusError::DuplicateCourseVariant);
                }
            }
            for section in &catalog.sections {
                if section.key.target() != catalog.target
                    || section.variant_key.group().target() != catalog.target
                {
                    return Err(CorpusError::TargetMismatch);
                }
                if sections.insert(&section.key, section).is_some() {
                    return Err(CorpusError::DuplicateSection);
                }
            }
            for occurrence in &catalog.occurrences {
                if occurrence.key.section.target() != catalog.target {
                    return Err(CorpusError::TargetMismatch);
                }
                if occurrences.insert(&occurrence.key, occurrence).is_some() {
                    return Err(CorpusError::DuplicateOccurrence);
                }
            }
        }

        for group in groups.values() {
            if group.variant_keys.is_empty() {
                return Err(CorpusError::InvalidGroupVariantReference);
            }
            let mut seen = BTreeSet::new();
            for key in &group.variant_keys {
                if key.group() != &group.key || !seen.insert(key) || !variants.contains_key(key) {
                    return Err(CorpusError::InvalidGroupVariantReference);
                }
            }
        }
        for variant in variants.values() {
            let Some(group) = groups.get(variant.key.group()) else {
                return Err(CorpusError::InvalidGroupVariantReference);
            };
            if !group.variant_keys.contains(&variant.key) {
                return Err(CorpusError::InvalidGroupVariantReference);
            }
            let mut seen = BTreeSet::new();
            for key in &variant.section_keys {
                let Some(section) = sections.get(key) else {
                    return Err(CorpusError::InvalidVariantSectionReference);
                };
                if section.variant_key != variant.key || !seen.insert(key) {
                    return Err(CorpusError::InvalidVariantSectionReference);
                }
            }
        }
        for section in sections.values() {
            let Some(variant) = variants.get(&section.variant_key) else {
                return Err(CorpusError::InvalidVariantSectionReference);
            };
            if !variant.section_keys.contains(&section.key) {
                return Err(CorpusError::InvalidVariantSectionReference);
            }
            if let CatalogFieldKnowledge::Known {
                presence: CatalogFieldPresence::Present { value: keys },
            } = &section.occurrence_keys
            {
                let mut seen = BTreeSet::new();
                for key in keys {
                    if key.section != section.key
                        || !seen.insert(key)
                        || !occurrences.contains_key(key)
                    {
                        return Err(CorpusError::InvalidSectionOccurrenceReference);
                    }
                }
            }
        }
        for occurrence in occurrences.values() {
            let Some(section) = sections.get(&occurrence.key.section) else {
                return Err(CorpusError::OrphanOccurrence);
            };
            match &section.occurrence_keys {
                CatalogFieldKnowledge::Known {
                    presence: CatalogFieldPresence::Present { value: keys },
                } if keys.contains(&occurrence.key) => {}
                _ => return Err(CorpusError::InvalidSectionOccurrenceReference),
            }
        }

        Ok(Self {
            term,
            target_versions,
            groups,
            variants,
            sections,
            occurrences,
        })
    }

    pub const fn term(&self) -> &str {
        self.term
    }

    pub fn groups(
        &self,
    ) -> impl DoubleEndedIterator<Item = &'a NormalizedCourseGroupV1> + ExactSizeIterator + '_ {
        self.groups.values().copied()
    }

    pub fn group(&self, key: &CourseGroupKey) -> Option<&'a NormalizedCourseGroupV1> {
        self.groups.get(key).copied()
    }

    pub fn sections(
        &self,
    ) -> impl DoubleEndedIterator<Item = &'a NormalizedSectionV1> + ExactSizeIterator + '_ {
        self.sections.values().copied()
    }

    pub fn target_versions(
        &self,
    ) -> impl ExactSizeIterator<Item = (&'a TermCampusKey, CatalogContentVersion)> + '_ {
        self.target_versions
            .iter()
            .map(|(target, version)| (*target, *version))
    }

    pub fn variant(&self, key: &CourseVariantKey) -> Option<&'a NormalizedCourseVariantV1> {
        self.variants.get(key).copied()
    }

    pub fn section(&self, key: &SectionKey) -> Option<&'a NormalizedSectionV1> {
        self.sections.get(key).copied()
    }

    pub fn occurrence(&self, key: &CatalogOccurrenceKeyV1) -> Option<&'a NormalizedOccurrenceV1> {
        self.occurrences.get(key).copied()
    }

    pub fn variants_for(
        &self,
        group: &NormalizedCourseGroupV1,
    ) -> Vec<&'a NormalizedCourseVariantV1> {
        group
            .variant_keys
            .iter()
            .map(|key| self.variants[key])
            .collect()
    }

    pub fn sections_for(
        &self,
        variant: &NormalizedCourseVariantV1,
    ) -> Vec<&'a NormalizedSectionV1> {
        variant
            .section_keys
            .iter()
            .map(|key| self.sections[key])
            .collect()
    }

    pub fn known_occurrences_for(
        &self,
        section: &NormalizedSectionV1,
    ) -> Option<Vec<&'a NormalizedOccurrenceV1>> {
        match &section.occurrence_keys {
            CatalogFieldKnowledge::Known {
                presence: CatalogFieldPresence::Present { value: keys },
            } => Some(keys.iter().map(|key| self.occurrences[key]).collect()),
            _ => None,
        }
    }
}
