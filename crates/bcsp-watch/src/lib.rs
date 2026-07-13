//! Shared watch and episode boundary; WebSocket behavior is absent.

#![forbid(unsafe_code)]
#![deny(warnings)]

pub const PACKAGE_BOUNDARY: &str = "bcsp-watch";

pub fn boundary_marker() -> &'static str {
    let _ = (
        bcsp_contracts::PACKAGE_BOUNDARY,
        bcsp_domain::PACKAGE_BOUNDARY,
        bcsp_open::PACKAGE_BOUNDARY,
    );
    PACKAGE_BOUNDARY
}

mod dependency_contract {
    use serde as _;
    use serde_json as _;
    use thiserror as _;
    use tokio as _;
    use tracing as _;
    use uuid as _;
}

#[cfg(test)]
mod dev_dependency_contract {
    use proptest as _;
}
