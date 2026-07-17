use std::collections::BTreeMap;

use bcsp_contracts::{CatalogContentVersion, CourseVariantKey, FilterSearchTextV1, TermCampusKey};
use thiserror::Error;

/// A storage-independent FTS hit. Lower rank sorts first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextHit {
    pub variant_key: CourseVariantKey,
    pub fts_rank: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextTargetVersion {
    pub target: TermCampusKey,
    pub content_version: CatalogContentVersion,
}

/// Text evidence bound to the exact normalized token-AND query that produced
/// it. The query engine never interpolates SQL and never falls back to a
/// partial string scan when evidence is unavailable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextHitPlan {
    query: FilterSearchTextV1,
    target_versions: BTreeMap<TermCampusKey, CatalogContentVersion>,
    hits: BTreeMap<CourseVariantKey, u32>,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum TextHitPlanError {
    #[error("text hit plan contains the same variant more than once")]
    DuplicateVariant,
    #[error("text hit plan contains the same target more than once")]
    DuplicateTarget,
    #[error("text hit belongs to a target not bound by the plan")]
    ForeignHitTarget,
}

impl TextHitPlan {
    pub fn try_new(
        query: &FilterSearchTextV1,
        target_versions: impl IntoIterator<Item = TextTargetVersion>,
        hits: impl IntoIterator<Item = TextHit>,
    ) -> Result<Self, TextHitPlanError> {
        let mut by_target = BTreeMap::new();
        for binding in target_versions {
            if by_target
                .insert(binding.target, binding.content_version)
                .is_some()
            {
                return Err(TextHitPlanError::DuplicateTarget);
            }
        }
        let mut by_variant = BTreeMap::new();
        for hit in hits {
            if !by_target.contains_key(&hit.variant_key.group().target()) {
                return Err(TextHitPlanError::ForeignHitTarget);
            }
            if by_variant.insert(hit.variant_key, hit.fts_rank).is_some() {
                return Err(TextHitPlanError::DuplicateVariant);
            }
        }
        Ok(Self {
            query: query.clone(),
            target_versions: by_target,
            hits: by_variant,
        })
    }

    pub const fn query(&self) -> &FilterSearchTextV1 {
        &self.query
    }

    pub fn rank(&self, key: &CourseVariantKey) -> Option<u32> {
        self.hits.get(key).copied()
    }

    pub fn target_versions(
        &self,
    ) -> impl ExactSizeIterator<Item = (&TermCampusKey, CatalogContentVersion)> {
        self.target_versions
            .iter()
            .map(|(target, version)| (target, *version))
    }

    pub fn hit_keys(&self) -> impl ExactSizeIterator<Item = &CourseVariantKey> {
        self.hits.keys()
    }
}

#[cfg(test)]
mod tests {
    use bcsp_contracts::{CourseGroupKey, CourseVariantKey};

    use super::*;

    fn variant(seed: char) -> CourseVariantKey {
        CourseVariantKey::try_new(
            CourseGroupKey::try_new("92026", "NB", "01:198:111").unwrap(),
            &format!("v1:{}", seed.to_string().repeat(64)),
        )
        .unwrap()
    }

    fn binding() -> TextTargetVersion {
        TextTargetVersion {
            target: TermCampusKey::try_new("92026", "NB").unwrap(),
            content_version: 1_u64.try_into().unwrap(),
        }
    }

    #[test]
    fn plan_normalizes_query_and_rejects_duplicate_keys() {
        let key = variant('a');
        let duplicate = TextHitPlan::try_new(
            &FilterSearchTextV1::try_from("Data Structures").unwrap(),
            [binding()],
            [
                TextHit {
                    variant_key: key.clone(),
                    fts_rank: 2,
                },
                TextHit {
                    variant_key: key,
                    fts_rank: 1,
                },
            ],
        );
        assert_eq!(duplicate, Err(TextHitPlanError::DuplicateVariant));

        let plan = TextHitPlan::try_new(
            &FilterSearchTextV1::try_from("Data   Structures").unwrap(),
            [binding()],
            [TextHit {
                variant_key: variant('b'),
                fts_rank: 4,
            }],
        )
        .unwrap();
        assert_eq!(plan.query().as_str(), "Data Structures");
        assert_eq!(plan.query().tokens(), &["Data", "Structures"]);
    }
}
