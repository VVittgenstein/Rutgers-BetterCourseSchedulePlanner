//! Shared domain boundary; product types and rules are intentionally absent.

#![forbid(unsafe_code)]
#![deny(warnings)]

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
