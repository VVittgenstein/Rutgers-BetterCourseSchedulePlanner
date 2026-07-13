//! Shared business-domain values and invariants.
//!
//! The local and public products consume this same implementation. The crate
//! is deliberately free of IO, persistence, runtime-host, and page-session
//! policy. `CourseGroup` and `CourseVariant` are invariant-bearing aggregates,
//! not wire DTOs; P7.1-005 owns normalized Catalog DTOs and persistence while
//! consuming these identities instead of introducing a second domain model.

#![forbid(unsafe_code)]
#![deny(warnings)]

mod course;
mod match_result;

pub use bcsp_contracts::{
    CourseGroupKey, CourseVariantKey, MatchExplanation, MatchExplanationError, MatchOutcome,
    MatchReason, MatchReasonCode, ReasonField, SectionKey,
};
pub use course::{
    CourseGroup, CourseVariant, DomainModelError, DomainReasonCode, DuplicateDisposition,
    SectionDuplicate, classify_section_duplicates,
};
pub use match_result::{MatchOutcomeAlgebra, evaluate_active_dimension};

pub const PACKAGE_BOUNDARY: &str = "bcsp-domain";

pub fn boundary_marker() -> &'static str {
    let _ = bcsp_contracts::PACKAGE_BOUNDARY;
    PACKAGE_BOUNDARY
}

mod dependency_contract {
    use jiff as _;
    use serde as _;
    use thiserror as _;
    use time as _;
    use uuid as _;
}

#[cfg(test)]
mod dev_dependency_contract {
    use proptest as _;
}
