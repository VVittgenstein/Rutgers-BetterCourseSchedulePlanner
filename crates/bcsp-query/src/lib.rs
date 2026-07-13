//! Shared query boundary; filtering behavior is owned by a later P7 task.

#![forbid(unsafe_code)]
#![deny(warnings)]

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
