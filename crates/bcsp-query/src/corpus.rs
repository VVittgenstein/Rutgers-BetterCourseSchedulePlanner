use std::collections::{BTreeMap, BTreeSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::Arc;

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

/// Build-once, owned Catalog indexes used by the prepared serving path.
///
/// The normalized Catalog snapshots remain shared through `Arc`; only their
/// identity-to-position indexes are owned here. This avoids both self-
/// referential storage and rebuilding the complete corpus for every request.
#[derive(Debug)]
pub struct PreparedCatalogCorpus {
    term: String,
    catalogs: Vec<Arc<NormalizedCatalogV1>>,
    target_versions: BTreeMap<TermCampusKey, CatalogContentVersion>,
    targets: BTreeMap<TermCampusKey, PreparedTargetIndex>,
}

#[derive(Debug)]
struct PreparedTargetIndex {
    catalog: usize,
    group_order: Vec<usize>,
    section_order: Vec<usize>,
    groups: BTreeMap<u64, IndexBucket>,
    variants: BTreeMap<u64, IndexBucket>,
    sections: BTreeMap<u64, IndexBucket>,
    occurrences: BTreeMap<u64, IndexBucket>,
}

#[derive(Debug)]
enum IndexBucket {
    One(usize),
    Many(Vec<usize>),
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum CorpusError {
    #[error("catalog corpus construction was cancelled")]
    Cancelled,
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
        Self::try_new_from_refs(catalogs.iter().collect::<Vec<_>>().as_slice())
    }

    pub fn try_new_from_refs(catalogs: &[&'a NormalizedCatalogV1]) -> Result<Self, CorpusError> {
        Self::try_new_from_refs_with_cancellation(catalogs, &mut || false)
    }

    fn try_new_from_refs_with_cancellation(
        catalogs: &[&'a NormalizedCatalogV1],
        cancellation: &mut impl FnMut() -> bool,
    ) -> Result<Self, CorpusError> {
        if cancellation() {
            return Err(CorpusError::Cancelled);
        }
        let Some(first) = catalogs.first().copied() else {
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
        for &catalog in catalogs {
            if cancellation() {
                return Err(CorpusError::Cancelled);
            }
            if !targets.insert(&catalog.target) {
                return Err(CorpusError::DuplicateTarget);
            }
            target_versions.insert(&catalog.target, catalog.content_version);
            for group in &catalog.course_groups {
                if cancellation() {
                    return Err(CorpusError::Cancelled);
                }
                if group.key.target() != catalog.target {
                    return Err(CorpusError::TargetMismatch);
                }
                if groups.insert(&group.key, group).is_some() {
                    return Err(CorpusError::DuplicateCourseGroup);
                }
            }
            for variant in &catalog.course_variants {
                if cancellation() {
                    return Err(CorpusError::Cancelled);
                }
                if variant.key.group().target() != catalog.target {
                    return Err(CorpusError::TargetMismatch);
                }
                if variants.insert(&variant.key, variant).is_some() {
                    return Err(CorpusError::DuplicateCourseVariant);
                }
            }
            for section in &catalog.sections {
                if cancellation() {
                    return Err(CorpusError::Cancelled);
                }
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
                if cancellation() {
                    return Err(CorpusError::Cancelled);
                }
                if occurrence.key.section.target() != catalog.target {
                    return Err(CorpusError::TargetMismatch);
                }
                if occurrences.insert(&occurrence.key, occurrence).is_some() {
                    return Err(CorpusError::DuplicateOccurrence);
                }
            }
        }

        for group in groups.values() {
            if cancellation() {
                return Err(CorpusError::Cancelled);
            }
            if group.variant_keys.is_empty() {
                return Err(CorpusError::InvalidGroupVariantReference);
            }
            let mut seen = BTreeSet::new();
            for key in &group.variant_keys {
                if cancellation() {
                    return Err(CorpusError::Cancelled);
                }
                if key.group() != &group.key || !seen.insert(key) || !variants.contains_key(key) {
                    return Err(CorpusError::InvalidGroupVariantReference);
                }
            }
        }
        for variant in variants.values() {
            if cancellation() {
                return Err(CorpusError::Cancelled);
            }
            let Some(group) = groups.get(variant.key.group()) else {
                return Err(CorpusError::InvalidGroupVariantReference);
            };
            if !group.variant_keys.contains(&variant.key) {
                return Err(CorpusError::InvalidGroupVariantReference);
            }
            let mut seen = BTreeSet::new();
            for key in &variant.section_keys {
                if cancellation() {
                    return Err(CorpusError::Cancelled);
                }
                let Some(section) = sections.get(key) else {
                    return Err(CorpusError::InvalidVariantSectionReference);
                };
                if section.variant_key != variant.key || !seen.insert(key) {
                    return Err(CorpusError::InvalidVariantSectionReference);
                }
            }
        }
        for section in sections.values() {
            if cancellation() {
                return Err(CorpusError::Cancelled);
            }
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
                    if cancellation() {
                        return Err(CorpusError::Cancelled);
                    }
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
            if cancellation() {
                return Err(CorpusError::Cancelled);
            }
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

impl PreparedCatalogCorpus {
    pub fn try_new(
        catalogs: impl IntoIterator<Item = Arc<NormalizedCatalogV1>>,
    ) -> Result<Self, CorpusError> {
        Self::try_new_with_cancellation(catalogs, || false)
    }

    pub fn try_new_with_cancellation(
        catalogs: impl IntoIterator<Item = Arc<NormalizedCatalogV1>>,
        mut cancellation: impl FnMut() -> bool,
    ) -> Result<Self, CorpusError> {
        if cancellation() {
            return Err(CorpusError::Cancelled);
        }
        let catalogs = catalogs.into_iter().collect::<Vec<_>>();
        let catalog_refs = catalogs.iter().map(Arc::as_ref).collect::<Vec<_>>();
        // Reuse the reference oracle's complete identity and relationship
        // validation once, before publishing the owned indexes.
        CatalogCorpus::try_new_from_refs_with_cancellation(&catalog_refs, &mut cancellation)?;

        let term = catalogs
            .first()
            .expect("validated non-empty Catalog corpus")
            .target
            .term()
            .as_str()
            .to_owned();
        let mut target_versions = BTreeMap::new();
        let mut targets = BTreeMap::new();
        for (catalog_index, catalog) in catalogs.iter().enumerate() {
            if cancellation() {
                return Err(CorpusError::Cancelled);
            }
            let target = catalog.target.clone();
            target_versions.insert(target.clone(), catalog.content_version);
            let mut group_order = (0..catalog.course_groups.len()).collect::<Vec<_>>();
            group_order.sort_by(|left, right| {
                catalog.course_groups[*left]
                    .key
                    .cmp(&catalog.course_groups[*right].key)
            });
            let mut section_order = (0..catalog.sections.len()).collect::<Vec<_>>();
            section_order.sort_by(|left, right| {
                catalog.sections[*left]
                    .key
                    .cmp(&catalog.sections[*right].key)
            });
            if cancellation() {
                return Err(CorpusError::Cancelled);
            }
            targets.insert(
                target,
                PreparedTargetIndex {
                    catalog: catalog_index,
                    group_order,
                    section_order,
                    groups: build_index_with_cancellation(
                        catalog.course_groups.iter().map(|group| &group.key),
                        &mut cancellation,
                    )?,
                    variants: build_index_with_cancellation(
                        catalog.course_variants.iter().map(|variant| &variant.key),
                        &mut cancellation,
                    )?,
                    sections: build_index_with_cancellation(
                        catalog.sections.iter().map(|section| &section.key),
                        &mut cancellation,
                    )?,
                    occurrences: build_index_with_cancellation(
                        catalog.occurrences.iter().map(|occurrence| &occurrence.key),
                        &mut cancellation,
                    )?,
                },
            );
        }

        Ok(Self {
            term,
            catalogs,
            target_versions,
            targets,
        })
    }

    pub const fn term(&self) -> &str {
        self.term.as_str()
    }

    pub fn contains_target(&self, target: &TermCampusKey) -> bool {
        self.target_versions.contains_key(target)
    }

    pub fn content_version(&self, target: &TermCampusKey) -> Option<CatalogContentVersion> {
        self.target_versions.get(target).copied()
    }

    pub fn target_versions(
        &self,
    ) -> impl ExactSizeIterator<Item = (&TermCampusKey, CatalogContentVersion)> + '_ {
        self.target_versions
            .iter()
            .map(|(target, version)| (target, *version))
    }

    pub(crate) fn for_each_group_in_targets<'corpus>(
        &'corpus self,
        targets: &[TermCampusKey],
        mut visitor: impl FnMut(&'corpus NormalizedCourseGroupV1),
    ) {
        for target in targets {
            if let Some(index) = self.targets.get(target) {
                let catalog = &self.catalogs[index.catalog];
                for entity in &index.group_order {
                    visitor(&catalog.course_groups[*entity]);
                }
            }
        }
    }

    pub(crate) fn group(&self, key: &CourseGroupKey) -> Option<&NormalizedCourseGroupV1> {
        let index = self.target_index(key.term().as_str(), key.campus().as_str())?;
        let catalog = &self.catalogs[index.catalog];
        lookup_index(&index.groups, &catalog.course_groups, key, |group| {
            &group.key
        })
        .map(|entity| &catalog.course_groups[entity])
    }

    pub(crate) fn variants_for(
        &self,
        group: &NormalizedCourseGroupV1,
    ) -> Vec<&NormalizedCourseVariantV1> {
        let index = self
            .target_index(group.key.term().as_str(), group.key.campus().as_str())
            .expect("validated group target belongs to the prepared corpus");
        let catalog = &self.catalogs[index.catalog];
        group
            .variant_keys
            .iter()
            .map(|key| {
                let entity =
                    lookup_index(&index.variants, &catalog.course_variants, key, |variant| {
                        &variant.key
                    })
                    .expect("validated group references a prepared variant");
                &catalog.course_variants[entity]
            })
            .collect()
    }

    pub(crate) fn variant(&self, key: &CourseVariantKey) -> Option<&NormalizedCourseVariantV1> {
        let index =
            self.target_index(key.group().term().as_str(), key.group().campus().as_str())?;
        let catalog = &self.catalogs[index.catalog];
        lookup_index(&index.variants, &catalog.course_variants, key, |variant| {
            &variant.key
        })
        .map(|entity| &catalog.course_variants[entity])
    }

    pub(crate) fn for_each_section_in_targets<'corpus>(
        &'corpus self,
        targets: &[TermCampusKey],
        mut visitor: impl FnMut(&'corpus NormalizedSectionV1),
    ) {
        for target in targets {
            if let Some(index) = self.targets.get(target) {
                let catalog = &self.catalogs[index.catalog];
                for entity in &index.section_order {
                    visitor(&catalog.sections[*entity]);
                }
            }
        }
    }

    pub(crate) fn section(&self, key: &SectionKey) -> Option<&NormalizedSectionV1> {
        let index = self.target_index(key.term().as_str(), key.campus().as_str())?;
        let catalog = &self.catalogs[index.catalog];
        lookup_index(&index.sections, &catalog.sections, key, |section| {
            &section.key
        })
        .map(|entity| &catalog.sections[entity])
    }

    pub(crate) fn sections_for(
        &self,
        variant: &NormalizedCourseVariantV1,
    ) -> Vec<&NormalizedSectionV1> {
        let group = variant.key.group();
        let index = self
            .target_index(group.term().as_str(), group.campus().as_str())
            .expect("validated variant target belongs to the prepared corpus");
        let catalog = &self.catalogs[index.catalog];
        variant
            .section_keys
            .iter()
            .map(|key| {
                let entity = lookup_index(&index.sections, &catalog.sections, key, |section| {
                    &section.key
                })
                .expect("validated variant references a prepared section");
                &catalog.sections[entity]
            })
            .collect()
    }

    pub(crate) fn known_occurrences_for(
        &self,
        section: &NormalizedSectionV1,
    ) -> Option<Vec<&NormalizedOccurrenceV1>> {
        let index =
            self.target_index(section.key.term().as_str(), section.key.campus().as_str())?;
        let catalog = &self.catalogs[index.catalog];
        match &section.occurrence_keys {
            CatalogFieldKnowledge::Known {
                presence: CatalogFieldPresence::Present { value: keys },
            } => Some(
                keys.iter()
                    .map(|key| {
                        let entity = lookup_index(
                            &index.occurrences,
                            &catalog.occurrences,
                            key,
                            |occurrence| &occurrence.key,
                        )
                        .expect("validated section references a prepared occurrence");
                        &catalog.occurrences[entity]
                    })
                    .collect(),
            ),
            _ => None,
        }
    }

    pub fn index_estimated_bytes(&self) -> u64 {
        let mut bytes = usize_to_u64(std::mem::size_of::<Self>())
            .saturating_add(usize_to_u64(self.term.capacity()))
            .saturating_add(
                usize_to_u64(self.catalogs.capacity())
                    .saturating_mul(usize_to_u64(std::mem::size_of::<Arc<NormalizedCatalogV1>>())),
            )
            .saturating_add(map_node_bytes::<TermCampusKey, CatalogContentVersion>(
                self.target_versions.len(),
            ))
            .saturating_add(map_node_bytes::<TermCampusKey, PreparedTargetIndex>(
                self.targets.len(),
            ));
        for target in self.target_versions.keys().chain(self.targets.keys()) {
            bytes = bytes
                .saturating_add(usize_to_u64(target.term().as_str().len()))
                .saturating_add(usize_to_u64(target.campus().as_str().len()));
        }
        for index in self.targets.values() {
            bytes = bytes
                .saturating_add(
                    usize_to_u64(index.group_order.capacity())
                        .saturating_mul(usize_to_u64(std::mem::size_of::<usize>())),
                )
                .saturating_add(
                    usize_to_u64(index.section_order.capacity())
                        .saturating_mul(usize_to_u64(std::mem::size_of::<usize>())),
                );
            for buckets in [
                &index.groups,
                &index.variants,
                &index.sections,
                &index.occurrences,
            ] {
                bytes = bytes.saturating_add(index_bucket_map_bytes(buckets));
            }
        }
        bytes
    }

    fn target_index(&self, term: &str, campus: &str) -> Option<&PreparedTargetIndex> {
        self.targets.iter().find_map(|(target, index)| {
            (target.term().as_str() == term && target.campus().as_str() == campus).then_some(index)
        })
    }
}

fn build_index_with_cancellation<'key, K: Hash + 'key>(
    keys: impl IntoIterator<Item = &'key K>,
    cancellation: &mut impl FnMut() -> bool,
) -> Result<BTreeMap<u64, IndexBucket>, CorpusError> {
    let mut buckets = BTreeMap::new();
    for (index, key) in keys.into_iter().enumerate() {
        if cancellation() {
            return Err(CorpusError::Cancelled);
        }
        insert_index_bucket(&mut buckets, key_hash(key), index);
    }
    Ok(buckets)
}

#[cfg(test)]
fn build_index_with_hasher<'key, K: 'key>(
    keys: impl IntoIterator<Item = &'key K>,
    hash: impl Fn(&K) -> u64,
) -> BTreeMap<u64, IndexBucket> {
    let mut buckets = BTreeMap::new();
    for (index, key) in keys.into_iter().enumerate() {
        insert_index_bucket(&mut buckets, hash(key), index);
    }
    buckets
}

fn insert_index_bucket(buckets: &mut BTreeMap<u64, IndexBucket>, hash: u64, index: usize) {
    match buckets.entry(hash) {
        std::collections::btree_map::Entry::Vacant(entry) => {
            entry.insert(IndexBucket::One(index));
        }
        std::collections::btree_map::Entry::Occupied(mut entry) => match entry.get_mut() {
            IndexBucket::One(first) => {
                *entry.get_mut() = IndexBucket::Many(vec![*first, index]);
            }
            IndexBucket::Many(indices) => indices.push(index),
        },
    }
}

fn lookup_index<T, K: Eq + Hash>(
    buckets: &BTreeMap<u64, IndexBucket>,
    values: &[T],
    key: &K,
    key_of: impl Fn(&T) -> &K,
) -> Option<usize> {
    lookup_index_with_hasher(buckets, values, key, key_of, key_hash)
}

fn lookup_index_with_hasher<T, K: Eq>(
    buckets: &BTreeMap<u64, IndexBucket>,
    values: &[T],
    key: &K,
    key_of: impl Fn(&T) -> &K,
    hash: impl Fn(&K) -> u64,
) -> Option<usize> {
    let bucket = buckets.get(&hash(key))?;
    match bucket {
        IndexBucket::One(index) => (key_of(&values[*index]) == key).then_some(*index),
        IndexBucket::Many(indices) => indices
            .iter()
            .copied()
            .find(|index| key_of(&values[*index]) == key),
    }
}

fn key_hash(key: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

fn index_bucket_map_bytes(buckets: &BTreeMap<u64, IndexBucket>) -> u64 {
    let mut bytes = map_node_bytes::<u64, IndexBucket>(buckets.len());
    for bucket in buckets.values() {
        if let IndexBucket::Many(indices) = bucket {
            bytes = bytes.saturating_add(
                usize_to_u64(indices.capacity())
                    .saturating_mul(usize_to_u64(std::mem::size_of::<usize>())),
            );
        }
    }
    bytes
}

fn map_node_bytes<K, V>(len: usize) -> u64 {
    usize_to_u64(len).saturating_mul(usize_to_u64(
        std::mem::size_of::<K>() + std::mem::size_of::<V>() + 3 * std::mem::size_of::<usize>(),
    ))
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod prepared_index_tests {
    use super::*;

    fn constant_hash(_: &String) -> u64 {
        7
    }

    #[test]
    fn prepared_corpus_honors_cancellation_before_validation() {
        assert!(matches!(
            PreparedCatalogCorpus::try_new_with_cancellation(
                Vec::<Arc<NormalizedCatalogV1>>::new(),
                || true,
            ),
            Err(CorpusError::Cancelled)
        ));
    }

    #[test]
    fn compact_index_one_bucket_rechecks_the_complete_key() {
        let values = vec!["alpha".to_owned()];
        let index = build_index_with_hasher(values.iter(), constant_hash);
        assert_eq!(
            lookup_index_with_hasher(
                &index,
                &values,
                &"alpha".to_owned(),
                |value| value,
                constant_hash
            ),
            Some(0)
        );
        assert_eq!(
            lookup_index_with_hasher(
                &index,
                &values,
                &"beta".to_owned(),
                |value| value,
                constant_hash
            ),
            None
        );
    }

    #[test]
    fn compact_index_many_bucket_is_collision_safe_for_hits_and_misses() {
        let values = vec!["alpha".to_owned(), "beta".to_owned(), "gamma".to_owned()];
        let index = build_index_with_hasher(values.iter(), constant_hash);
        assert!(matches!(index.get(&7), Some(IndexBucket::Many(values)) if values == &[0, 1, 2]));
        for (expected, value) in values.iter().enumerate() {
            assert_eq!(
                lookup_index_with_hasher(&index, &values, value, |value| value, constant_hash),
                Some(expected)
            );
        }
        assert_eq!(
            lookup_index_with_hasher(
                &index,
                &values,
                &"delta".to_owned(),
                |value| value,
                constant_hash
            ),
            None
        );
    }
}
