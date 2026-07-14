use crate::Sha256Hex;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CatalogBaseline {
    Uninitialized,
    Empty {
        content_hash: Sha256Hex,
    },
    NonEmpty {
        content_hash: Sha256Hex,
        section_count: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogCandidateDecision {
    PublishNonEmpty,
    PublishInitialEmptyValid,
    PublishExistingEmpty,
    RetainLastKnownEmptySuspect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum CatalogEmptyPolicyError {
    #[error("first Catalog empty is not selector-confirmed")]
    InitialEmptyNotSelectorConfirmed,
}

pub fn decide_catalog_candidate(
    baseline: &CatalogBaseline,
    candidate_is_empty: bool,
    selector_confirms_target: bool,
) -> Result<CatalogCandidateDecision, CatalogEmptyPolicyError> {
    if !candidate_is_empty {
        return Ok(CatalogCandidateDecision::PublishNonEmpty);
    }
    match baseline {
        CatalogBaseline::Uninitialized if selector_confirms_target => {
            Ok(CatalogCandidateDecision::PublishInitialEmptyValid)
        }
        CatalogBaseline::Uninitialized => {
            Err(CatalogEmptyPolicyError::InitialEmptyNotSelectorConfirmed)
        }
        CatalogBaseline::Empty { .. } => Ok(CatalogCandidateDecision::PublishExistingEmpty),
        CatalogBaseline::NonEmpty { .. } => {
            // P3 freezes no numeric automatic confirmation threshold. A later
            // approved scheduler policy must explicitly authorize replacement.
            Ok(CatalogCandidateDecision::RetainLastKnownEmptySuspect)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CatalogBaseline, CatalogCandidateDecision, CatalogEmptyPolicyError,
        decide_catalog_candidate,
    };
    use crate::Sha256Hex;

    #[test]
    fn only_selector_confirmed_first_empty_is_valid() {
        assert_eq!(
            decide_catalog_candidate(&CatalogBaseline::Uninitialized, true, true),
            Ok(CatalogCandidateDecision::PublishInitialEmptyValid)
        );
        assert_eq!(
            decide_catalog_candidate(&CatalogBaseline::Uninitialized, true, false),
            Err(CatalogEmptyPolicyError::InitialEmptyNotSelectorConfirmed)
        );
    }

    #[test]
    fn nonempty_to_empty_always_retains_last_known_without_invented_threshold() {
        let baseline = CatalogBaseline::NonEmpty {
            content_hash: Sha256Hex::try_new("0".repeat(64)).unwrap(),
            section_count: 3,
        };
        for _ in 0..100 {
            assert_eq!(
                decide_catalog_candidate(&baseline, true, true),
                Ok(CatalogCandidateDecision::RetainLastKnownEmptySuspect)
            );
        }
    }
}
