//! Rutgers transport adapter boundary.
//!
//! This crate owns upstream-shaped DTOs, selector discovery values, and
//! response provenance.  It deliberately does not decide refresh cadence or
//! Catalog publication policy.

#![forbid(unsafe_code)]
#![deny(warnings)]

pub const PACKAGE_BOUNDARY: &str = "bcsp-rutgers-client";

mod catalog;
mod discovery;
mod hashing;
mod open_sections;
mod raw_catalog;
mod selector;

pub use catalog::{
    CATALOG_BODY_IDLE_TIMEOUT, CATALOG_CONNECT_TIMEOUT, CATALOG_MAX_DECODED_BYTES,
    CATALOG_RESPONSE_HEADER_TIMEOUT, CATALOG_TOTAL_TIMEOUT, CatalogClientBuildError,
    CatalogFailure, CatalogFailureResponseMetadata, CatalogRequest, CatalogRequestError,
    CatalogResponse, CatalogResponseMetadata, CatalogTransportError, RUTGERS_CATALOG_ENDPOINT,
    RutgersCatalogClient,
};
pub use discovery::{
    DiscoveredCampus, DiscoveredSubject, DiscoveredTarget, DiscoveredTerm, DiscoveryDecodeError,
    DiscoveryError, DiscoveryFailureDisposition, DiscoverySnapshot, DiscoverySource,
    DiscoverySourceId, DiscoverySourceInput, DiscoverySourceKind, RawDiscoveryCampus,
    RawDiscoveryDocument, RawDiscoverySubject, RawDiscoveryTarget, RawDiscoveryTerm,
    decide_discovery_failure, decode_discovery_payload,
};
pub use hashing::{sha256_hex, sha256_v1};
pub use open_sections::{
    OPEN_CONNECT_TIMEOUT, OPEN_MAX_DECODED_BYTES, OPEN_SET_HASH_DOMAIN, OPEN_TOTAL_TIMEOUT,
    OpenClientBuildError, OpenFailureDisposition, OpenFailureResponseMetadata,
    OpenPayloadClassification, OpenResponseMetadata, OpenSectionsError, OpenSectionsFailure,
    OpenSectionsRequest, OpenSectionsRequestError, OpenSectionsResponse,
    RUTGERS_OPEN_SECTIONS_ENDPOINT, RedirectScope, RetryAfterHeader, RetryAfterValue,
    RutgersOpenSectionsClient, canonical_open_set_sha256,
};
pub use raw_catalog::{
    CatalogDecodeError, JsonType, MalformedField, Presence, RawCatalogCourse, RawCatalogSection,
    RawInstructor, RawMeeting, SourceProvenance, decode_catalog_payload,
};
pub use selector::{
    DISCOVERY_CONNECT_TIMEOUT, DISCOVERY_MAX_BOOTSTRAP_BYTES, DISCOVERY_MAX_SELECTOR_BYTES,
    DISCOVERY_TOTAL_TIMEOUT, DiscoveryClientBuildError, DiscoveryFailure,
    DiscoveryFailureResponseMetadata, DiscoveryResourceKind, DiscoveryResourceMetadata,
    DiscoveryResponse, DiscoveryResponseMetadata, DiscoveryTransportError, RUTGERS_DISCOVERY_ROOT,
    RutgersDiscoveryClient,
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
