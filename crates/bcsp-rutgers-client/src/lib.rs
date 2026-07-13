//! Rutgers transport adapter boundary; no upstream request behavior exists yet.

#![forbid(unsafe_code)]
#![deny(warnings)]

pub const PACKAGE_BOUNDARY: &str = "bcsp-rutgers-client";

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
