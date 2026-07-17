//! Pure, target-neutral catalog filtering and result assembly.
//!
//! This crate deliberately does not know how Catalog or live Open evidence is
//! fetched or stored. Callers provide normalized Catalog snapshots, an
//! optional text-hit plan, and live Open evidence. The same evaluator is used
//! for course search, section search, and detail lookup.

#![forbid(unsafe_code)]
#![deny(warnings)]

mod availability;
mod corpus;
mod credits;
mod engine;
mod evaluation;
mod knowledge;
mod predicates;
mod text;

pub use corpus::{CatalogCorpus, CorpusError, PreparedCatalogCorpus};
pub use credits::{CreditValue, CreditValueParseError, parse_credit_value};
pub use engine::{OpenEvidence, PreparedOpenOverlay, QueryEngine, QueryError};
pub use evaluation::{PredicateEvaluation, and_all, or_active};
pub use predicates::evaluate_section_filters;
pub use text::{TextHit, TextHitPlan, TextHitPlanError, TextTargetVersion};

pub const PACKAGE_BOUNDARY: &str = "bcsp-query";

pub fn boundary_marker() -> &'static str {
    let _ = (
        bcsp_contracts::PACKAGE_BOUNDARY,
        bcsp_domain::PACKAGE_BOUNDARY,
    );
    PACKAGE_BOUNDARY
}

mod dependency_contract {
    use serde as _;
    use thiserror as _;
}

#[cfg(test)]
mod dev_dependency_contract {
    use proptest as _;
}
