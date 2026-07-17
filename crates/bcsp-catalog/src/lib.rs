//! Target-neutral Rutgers Catalog normalization.
//!
//! This crate turns typed upstream DTOs into deterministic course groups,
//! offering variants, exact section identities, occurrences, and provenance.
//! Storage transactions, refresh scheduling, query predicates, and live Open
//! state are intentionally owned elsewhere.

#![forbid(unsafe_code)]
#![deny(warnings)]

mod canonical;
mod delivery;
mod empty;
mod mapping;
mod model;
mod normalize;
mod projection;

pub use delivery::{normalize_time, parse_weekdays};
pub use empty::{
    CatalogBaseline, CatalogCandidateDecision, CatalogEmptyPolicyError, decide_catalog_candidate,
};
pub use mapping::{
    SnapshotMappingError, to_catalog_refresh_command, to_discovery_refresh_command,
    to_operational_discovery_snapshot, to_operational_snapshot,
};
pub use model::{
    CatalogMetrics, CatalogOpenSectionsSnapshot, CollectionPresence, CourseVariantFields,
    DayKnowledge, DeliveryModality, DuplicateAudit, DuplicateDisposition, InstructorReliability,
    NormalizedCatalogTarget, NormalizedCourseGroup, NormalizedCourseVariant, NormalizedInstructor,
    NormalizedOccurrence, NormalizedSection, OccurrenceEvidence, OccurrenceKind,
    OccurrenceModality, Requiredness, Sha256Hex, Sha256HexError, Synchronicity, TimeKnowledge,
    Weekday, WeekdaySet,
};
pub use normalize::{NormalizationError, normalize_target, variant_fingerprint_v1};
pub use projection::{
    ProjectionError, to_catalog_discovery_response_v1, to_catalog_refresh_checkpoint_v1,
    to_catalog_refresh_observation_v1, to_catalog_refresh_status_v1, to_normalized_catalog_v1,
};

pub const PACKAGE_BOUNDARY: &str = "bcsp-catalog";

pub fn boundary_marker() -> &'static str {
    let _ = (
        bcsp_contracts::PACKAGE_BOUNDARY,
        bcsp_domain::PACKAGE_BOUNDARY,
        bcsp_operational_storage::PACKAGE_BOUNDARY,
        bcsp_rutgers_client::PACKAGE_BOUNDARY,
    );
    PACKAGE_BOUNDARY
}

mod dependency_contract {
    use serde as _;
    use serde_json as _;
    use thiserror as _;
    use tokio as _;
    use tracing as _;
}
