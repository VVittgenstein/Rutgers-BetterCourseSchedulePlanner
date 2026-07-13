//! Public-only operational host boundary; no deployment behavior exists here.

#![forbid(unsafe_code)]
#![deny(warnings)]

pub const PACKAGE_BOUNDARY: &str = "bcsp-public-operations";

pub fn boundary_marker() -> &'static str {
    let _ = bcsp_operational_storage::PACKAGE_BOUNDARY;
    PACKAGE_BOUNDARY
}

mod dependency_contract {
    use rusqlite as _;
    use thiserror as _;
    use tracing as _;
}

#[cfg(test)]
mod dev_dependency_contract {
    use tempfile as _;
}
