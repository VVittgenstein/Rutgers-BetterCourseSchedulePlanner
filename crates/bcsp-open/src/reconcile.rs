use std::collections::{BTreeMap, BTreeSet};

use bcsp_contracts::{CatalogContentVersion, SectionIndex, SectionKey, TermCampusKey};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ReconciledOpenState {
    Closed,
    Open,
}

impl ReconciledOpenState {
    const fn wire_name(self) -> &'static str {
        match self {
            Self::Closed => "CLOSED",
            Self::Open => "OPEN",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogOpenBatch {
    target: TermCampusKey,
    content_version: CatalogContentVersion,
    sections: BTreeMap<SectionIndex, SectionKey>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ReconcileInputError {
    #[error("a Catalog Section does not belong to the Open batch target")]
    ForeignCatalogSection,
    #[error("a Catalog target contains the same Section index more than once")]
    DuplicateCatalogSectionIndex,
    #[error("the Open set raw count is smaller than its unique member count")]
    InvalidOpenSetCount,
    #[error("the Open set does not belong to the Catalog batch target")]
    ForeignOpenBatch,
}

impl CatalogOpenBatch {
    pub fn try_new(
        target: TermCampusKey,
        content_version: CatalogContentVersion,
        sections: impl IntoIterator<Item = SectionKey>,
    ) -> Result<Self, ReconcileInputError> {
        let mut by_index = BTreeMap::new();
        for section in sections {
            if section.target() != target {
                return Err(ReconcileInputError::ForeignCatalogSection);
            }
            if by_index.insert(section.index().clone(), section).is_some() {
                return Err(ReconcileInputError::DuplicateCatalogSectionIndex);
            }
        }
        Ok(Self {
            target,
            content_version,
            sections: by_index,
        })
    }

    pub const fn target(&self) -> &TermCampusKey {
        &self.target
    }

    pub const fn content_version(&self) -> CatalogContentVersion {
        self.content_version
    }

    pub fn section_count(&self) -> usize {
        self.sections.len()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenSetEvidence {
    target: TermCampusKey,
    indexes: BTreeSet<SectionIndex>,
    raw_value_count: u64,
    duplicate_count: u64,
    canonical_set_hash: String,
}

impl OpenSetEvidence {
    pub fn try_new(
        target: TermCampusKey,
        indexes: impl IntoIterator<Item = SectionIndex>,
        raw_value_count: u64,
    ) -> Result<Self, ReconcileInputError> {
        let indexes = indexes.into_iter().collect::<BTreeSet<_>>();
        let unique_count = u64::try_from(indexes.len()).unwrap_or(u64::MAX);
        let Some(duplicate_count) = raw_value_count.checked_sub(unique_count) else {
            return Err(ReconcileInputError::InvalidOpenSetCount);
        };
        let canonical_set_hash = canonical_open_set_hash_v1(indexes.iter());
        Ok(Self {
            target,
            indexes,
            raw_value_count,
            duplicate_count,
            canonical_set_hash,
        })
    }

    pub const fn target(&self) -> &TermCampusKey {
        &self.target
    }

    pub fn indexes(&self) -> impl ExactSizeIterator<Item = &SectionIndex> {
        self.indexes.iter()
    }

    pub const fn raw_value_count(&self) -> u64 {
        self.raw_value_count
    }

    pub const fn duplicate_count(&self) -> u64 {
        self.duplicate_count
    }

    pub fn unique_count(&self) -> usize {
        self.indexes.len()
    }

    pub fn canonical_set_hash(&self) -> &str {
        &self.canonical_set_hash
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenReconcileClassification {
    ValidApplied,
    ValidEmptyNoRows,
    UnsafeEmpty,
    UnsafeZeroIntersection,
    StaleCatalogRace,
}

impl OpenReconcileClassification {
    pub const fn is_valid(self) -> bool {
        matches!(self, Self::ValidApplied | Self::ValidEmptyNoRows)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectionOpenUpdate {
    pub section_key: SectionKey,
    pub state: ReconciledOpenState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenReconcilePlan {
    pub target: TermCampusKey,
    pub captured_catalog_content_version: CatalogContentVersion,
    pub current_catalog_content_version: CatalogContentVersion,
    pub classification: OpenReconcileClassification,
    pub canonical_set_hash: String,
    pub state_hash: Option<String>,
    pub raw_value_count: u64,
    pub unique_value_count: u64,
    pub duplicate_count: u64,
    pub catalog_section_count: u64,
    pub orphan_count: u64,
    pub open_count: u64,
    pub closed_count: u64,
    pub updates: Vec<SectionOpenUpdate>,
}

pub fn reconcile_open_set(
    captured_catalog_content_version: CatalogContentVersion,
    current_catalog: &CatalogOpenBatch,
    open_set: &OpenSetEvidence,
) -> Result<OpenReconcilePlan, ReconcileInputError> {
    if open_set.target != current_catalog.target {
        return Err(ReconcileInputError::ForeignOpenBatch);
    }
    let base = |classification: OpenReconcileClassification,
                orphan_count: usize,
                updates: Vec<SectionOpenUpdate>| {
        let open_count = updates
            .iter()
            .filter(|update| update.state == ReconciledOpenState::Open)
            .count();
        let closed_count = updates.len().saturating_sub(open_count);
        let state_hash = classification
            .is_valid()
            .then(|| open_state_hash_v1(&current_catalog.target, &updates));
        OpenReconcilePlan {
            target: current_catalog.target.clone(),
            captured_catalog_content_version,
            current_catalog_content_version: current_catalog.content_version,
            classification,
            canonical_set_hash: open_set.canonical_set_hash.clone(),
            state_hash,
            raw_value_count: open_set.raw_value_count,
            unique_value_count: u64::try_from(open_set.indexes.len()).unwrap_or(u64::MAX),
            duplicate_count: open_set.duplicate_count,
            catalog_section_count: u64::try_from(current_catalog.sections.len())
                .unwrap_or(u64::MAX),
            orphan_count: u64::try_from(orphan_count).unwrap_or(u64::MAX),
            open_count: u64::try_from(open_count).unwrap_or(u64::MAX),
            closed_count: u64::try_from(closed_count).unwrap_or(u64::MAX),
            updates,
        }
    };

    if captured_catalog_content_version != current_catalog.content_version {
        return Ok(base(
            OpenReconcileClassification::StaleCatalogRace,
            0,
            Vec::new(),
        ));
    }

    if open_set.indexes.is_empty() {
        return Ok(if current_catalog.sections.is_empty() {
            base(OpenReconcileClassification::ValidEmptyNoRows, 0, Vec::new())
        } else {
            let updates = current_catalog
                .sections
                .values()
                .cloned()
                .map(|section_key| SectionOpenUpdate {
                    section_key,
                    state: ReconciledOpenState::Closed,
                })
                .collect();
            base(OpenReconcileClassification::ValidApplied, 0, updates)
        });
    }

    let intersection_count = open_set
        .indexes
        .iter()
        .filter(|index| current_catalog.sections.contains_key(*index))
        .count();
    let orphan_count = open_set.indexes.len().saturating_sub(intersection_count);
    if !current_catalog.sections.is_empty() && intersection_count == 0 {
        return Ok(base(
            OpenReconcileClassification::UnsafeZeroIntersection,
            orphan_count,
            Vec::new(),
        ));
    }

    let updates = current_catalog
        .sections
        .iter()
        .map(|(index, section_key)| SectionOpenUpdate {
            section_key: section_key.clone(),
            state: if open_set.indexes.contains(index) {
                ReconciledOpenState::Open
            } else {
                ReconciledOpenState::Closed
            },
        })
        .collect();
    Ok(base(
        OpenReconcileClassification::ValidApplied,
        orphan_count,
        updates,
    ))
}

pub fn canonical_open_set_hash_v1<'a>(
    indexes: impl IntoIterator<Item = &'a SectionIndex>,
) -> String {
    let mut preimage = String::from("bcsp-open-set-v1\n");
    for index in indexes {
        preimage.push_str(index.as_str());
        preimage.push('\n');
    }
    bcsp_rutgers_client::sha256_hex(preimage.as_bytes())
}

fn open_state_hash_v1(target: &TermCampusKey, updates: &[SectionOpenUpdate]) -> String {
    let mut preimage = format!(
        "bcsp-open-state-v1\n{}\n{}\n",
        target.term(),
        target.campus(),
    );
    for update in updates {
        preimage.push_str(update.section_key.index().as_str());
        preimage.push('\t');
        preimage.push_str(update.state.wire_name());
        preimage.push('\n');
    }
    bcsp_rutgers_client::sha256_hex(preimage.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(campus: &str) -> TermCampusKey {
        TermCampusKey::try_new("92026", campus).expect("synthetic target")
    }

    fn section(campus: &str, index: &str) -> SectionKey {
        SectionKey::try_new("92026", campus, index).expect("synthetic section")
    }

    fn version(value: u64) -> CatalogContentVersion {
        CatalogContentVersion::try_from(value).expect("synthetic version")
    }

    fn index(value: &str) -> SectionIndex {
        SectionIndex::try_from(value).expect("synthetic index")
    }

    #[test]
    fn valid_nonempty_intersects_atomically_and_audits_duplicates_and_orphans() {
        let catalog = CatalogOpenBatch::try_new(
            target("NB"),
            version(7),
            [section("NB", "00001"), section("NB", "00002")],
        )
        .expect("catalog");
        let set = OpenSetEvidence::try_new(
            target("NB"),
            [index("00001"), index("00001"), index("99999")],
            3,
        )
        .expect("Open set");
        let plan = reconcile_open_set(version(7), &catalog, &set).expect("same target");

        assert_eq!(
            plan.classification,
            OpenReconcileClassification::ValidApplied
        );
        assert_eq!(plan.duplicate_count, 1);
        assert_eq!(plan.orphan_count, 1);
        assert_eq!(plan.open_count, 1);
        assert_eq!(plan.closed_count, 1);
        assert_eq!(
            plan.updates
                .iter()
                .map(|update| update.state)
                .collect::<Vec<_>>(),
            vec![ReconciledOpenState::Open, ReconciledOpenState::Closed]
        );
        assert!(plan.state_hash.is_some());
    }

    #[test]
    fn typed_empty_closes_nonempty_catalog_while_zero_intersection_stays_unsafe() {
        let catalog = CatalogOpenBatch::try_new(target("NB"), version(1), [section("NB", "00001")])
            .expect("catalog");
        let empty = OpenSetEvidence::try_new(target("NB"), [], 0).expect("empty set");
        let plan = reconcile_open_set(version(1), &catalog, &empty).expect("same target");
        assert_eq!(
            plan.classification,
            OpenReconcileClassification::ValidApplied
        );
        assert_eq!(plan.open_count, 0);
        assert_eq!(plan.closed_count, 1);
        assert_eq!(plan.updates[0].state, ReconciledOpenState::Closed);
        assert!(plan.state_hash.is_some());

        let foreign =
            OpenSetEvidence::try_new(target("NB"), [index("99999")], 1).expect("foreign set");
        let plan = reconcile_open_set(version(1), &catalog, &foreign).expect("same target");
        assert_eq!(
            plan.classification,
            OpenReconcileClassification::UnsafeZeroIntersection
        );
        assert_eq!(plan.orphan_count, 1);
        assert!(plan.updates.is_empty());
    }

    #[test]
    fn empty_catalog_accepts_empty_or_nonempty_sets_without_rows() {
        let catalog =
            CatalogOpenBatch::try_new(target("NB"), version(1), []).expect("empty catalog");
        let empty = reconcile_open_set(
            version(1),
            &catalog,
            &OpenSetEvidence::try_new(target("NB"), [], 0).expect("empty set"),
        )
        .expect("same target");
        assert_eq!(
            empty.classification,
            OpenReconcileClassification::ValidEmptyNoRows
        );
        assert_eq!(empty.orphan_count, 0);

        let nonempty = reconcile_open_set(
            version(1),
            &catalog,
            &OpenSetEvidence::try_new(target("NB"), [index("99999")], 1).expect("nonempty set"),
        )
        .expect("same target");
        assert_eq!(
            nonempty.classification,
            OpenReconcileClassification::ValidApplied
        );
        assert_eq!(nonempty.orphan_count, 1);
        assert!(nonempty.updates.is_empty());
    }

    #[test]
    fn catalog_version_race_produces_no_state_hash_or_updates() {
        let catalog = CatalogOpenBatch::try_new(target("NB"), version(2), [section("NB", "00001")])
            .expect("catalog");
        let plan = reconcile_open_set(
            version(1),
            &catalog,
            &OpenSetEvidence::try_new(target("NB"), [index("00001")], 1).expect("set"),
        )
        .expect("same target");
        assert_eq!(
            plan.classification,
            OpenReconcileClassification::StaleCatalogRace
        );
        assert!(plan.updates.is_empty());
        assert!(plan.state_hash.is_none());
    }

    #[test]
    fn target_and_count_invariants_fail_closed() {
        assert_eq!(
            CatalogOpenBatch::try_new(target("NB"), version(1), [section("NWK", "00001")]),
            Err(ReconcileInputError::ForeignCatalogSection)
        );
        assert_eq!(
            OpenSetEvidence::try_new(target("NB"), [index("00001")], 0),
            Err(ReconcileInputError::InvalidOpenSetCount)
        );

        let catalog = CatalogOpenBatch::try_new(target("NB"), version(1), [section("NB", "00001")])
            .expect("catalog");
        let other_batch =
            OpenSetEvidence::try_new(target("NWK"), [index("00001")], 1).expect("set");
        assert_eq!(
            reconcile_open_set(version(1), &catalog, &other_batch),
            Err(ReconcileInputError::ForeignOpenBatch)
        );
    }

    #[test]
    fn canonical_hashes_are_order_independent_and_state_is_version_independent() {
        let forward = OpenSetEvidence::try_new(target("NB"), [index("00002"), index("00001")], 2)
            .expect("forward");
        let reverse = OpenSetEvidence::try_new(target("NB"), [index("00001"), index("00002")], 2)
            .expect("reverse");
        assert_eq!(forward.canonical_set_hash, reverse.canonical_set_hash);
        assert_eq!(
            forward.canonical_set_hash,
            "817ceb456303f598c8631b1d31dcfd471c8facd6bef7b93c4d4f44433784f369"
        );

        let first = CatalogOpenBatch::try_new(
            target("NB"),
            version(1),
            [section("NB", "00002"), section("NB", "00001")],
        )
        .expect("catalog");
        let second = CatalogOpenBatch::try_new(
            target("NB"),
            version(2),
            [section("NB", "00001"), section("NB", "00002")],
        )
        .expect("catalog");
        let first_plan = reconcile_open_set(version(1), &first, &forward).expect("same target");
        let second_plan = reconcile_open_set(version(2), &second, &forward).expect("same target");
        assert_eq!(
            first_plan.state_hash.as_deref(),
            Some("09a3c51f4f65611abf20f0179c001a6cd782c057524ba591899dfdd3b4331d3b")
        );
        assert_eq!(first_plan.state_hash, second_plan.state_hash);
    }
}
