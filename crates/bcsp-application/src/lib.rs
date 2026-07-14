//! Shared application composition boundary; routes and runtime behavior are absent.

#![forbid(unsafe_code)]
#![deny(warnings)]

pub const PACKAGE_BOUNDARY: &str = "bcsp-application";

mod query_service;

pub use query_service::{SharedQueryError, SharedQueryService};

pub fn boundary_marker() -> &'static str {
    let _ = (
        bcsp_catalog::PACKAGE_BOUNDARY,
        bcsp_contracts::PACKAGE_BOUNDARY,
        bcsp_domain::PACKAGE_BOUNDARY,
        bcsp_open::PACKAGE_BOUNDARY,
        bcsp_operational_storage::PACKAGE_BOUNDARY,
        bcsp_query::PACKAGE_BOUNDARY,
        bcsp_rutgers_client::PACKAGE_BOUNDARY,
        bcsp_watch::PACKAGE_BOUNDARY,
    );
    PACKAGE_BOUNDARY
}

mod dependency_contract {
    use axum as _;
    use serde_json as _;
    use tokio as _;
    use tower as _;
    use tower_http as _;
    use tracing as _;
}
