//! Linux-public runtime adapter boundary; service behavior is absent.

#![forbid(unsafe_code)]
#![deny(warnings)]

pub const PACKAGE_BOUNDARY: &str = "bcsp-public-runtime";

pub fn boundary_marker() -> &'static str {
    let _ = (
        bcsp_application::PACKAGE_BOUNDARY,
        bcsp_operational_storage::PACKAGE_BOUNDARY,
        bcsp_public_operations::PACKAGE_BOUNDARY,
    );
    PACKAGE_BOUNDARY
}

mod dependency_contract {
    use include_dir as _;
    use thiserror as _;
    use tokio as _;
    use tracing as _;
    use tracing_subscriber as _;
}
