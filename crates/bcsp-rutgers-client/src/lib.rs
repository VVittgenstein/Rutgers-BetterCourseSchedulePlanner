//! Rutgers transport adapter boundary.
//!
//! This crate owns upstream-shaped DTOs, selector discovery values, and
//! response provenance.  It deliberately does not decide refresh cadence or
//! Catalog publication policy.

#![forbid(unsafe_code)]
#![deny(warnings)]

pub const PACKAGE_BOUNDARY: &str = "bcsp-rutgers-client";

mod discovery;
mod hashing;
mod raw_catalog;

pub use discovery::{
    DiscoveredCampus, DiscoveredSubject, DiscoveredTarget, DiscoveredTerm, DiscoveryDecodeError,
    DiscoveryError, DiscoveryFailureDisposition, DiscoverySnapshot, DiscoverySource,
    DiscoverySourceId, DiscoverySourceInput, DiscoverySourceKind, RawDiscoveryCampus,
    RawDiscoveryDocument, RawDiscoverySubject, RawDiscoveryTarget, RawDiscoveryTerm,
    decide_discovery_failure, decode_discovery_payload,
};
pub use hashing::{sha256_hex, sha256_v1};
pub use raw_catalog::{
    CatalogDecodeError, JsonType, MalformedField, Presence, RawCatalogCourse, RawCatalogSection,
    RawInstructor, RawMeeting, SourceProvenance, decode_catalog_payload,
};

pub fn boundary_marker() -> &'static str {
    let _ = bcsp_contracts::PACKAGE_BOUNDARY;
    PACKAGE_BOUNDARY
}

mod dependency_contract {
    use reqwest as _;
    use serde as _;
    use serde_json as _;
    use sha2 as _;
    use thiserror as _;
    use tokio as _;
    use tracing as _;
}
