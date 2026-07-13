//! Windows-local runtime adapter boundary; startup behavior is absent.

#![forbid(unsafe_code)]
#![deny(warnings)]

pub const PACKAGE_BOUNDARY: &str = "bcsp-local-runtime";

pub fn boundary_marker() -> &'static str {
    let _ = (
        bcsp_application::PACKAGE_BOUNDARY,
        bcsp_local_user_state::PACKAGE_BOUNDARY,
        bcsp_operational_storage::PACKAGE_BOUNDARY,
    );
    PACKAGE_BOUNDARY
}

mod dependency_contract {
    use include_dir as _;
    use open as _;
    use thiserror as _;
    use tokio as _;
    use tracing as _;
    use tracing_subscriber as _;
}
