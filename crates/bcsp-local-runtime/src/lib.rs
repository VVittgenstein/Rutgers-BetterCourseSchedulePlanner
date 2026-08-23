//! Windows-local runtime adapter boundary.

#![forbid(unsafe_code)]
#![deny(warnings)]

pub const PACKAGE_BOUNDARY: &str = "bcsp-local-runtime";

mod bootstrap;
mod diagnostic;
mod extension;
mod history;
mod instance;
mod lifecycle;
mod path;
mod personal;
mod policy;
mod product;
mod watch;

pub use bootstrap::{
    LocalBootstrapError, LocalPrimaryDatabase, LocalRuntimeState, OperationalGate,
};
pub use diagnostic::StartupFailureReport;
pub use extension::{
    LocalApiErrorCode, LocalRouteExtension, LocalSurfaceFailure, LocalSurfaceState,
};
pub use instance::{
    ExistingLocalInstance, LOCAL_INSTANCE_LOCK_FILE_NAME, LocalInstanceClaim, LocalInstanceError,
    LoopbackBrowserUrl, PrimaryInstanceLease,
};
pub use lifecycle::{
    LocalRuntimeError, PreparedLocalRuntime, RunningLocalRuntime, prepare_and_start_with, run,
    run_blocking,
};
pub use path::{
    LOCAL_DATA_DIRECTORY_NAME, LOCAL_DATABASE_FILE_NAME, LocalPathError, LocalRuntimePaths,
};
pub use personal::PersonalSurface;
pub use policy::{LocalRefreshPolicyProvider, LocalRuntimeCore, create_local_runtime_core};
pub use watch::{
    LOCAL_DESIRED_WATCH_SOCKET_PATH, LOCAL_PRESENCE_SOCKET_PATH, create_local_watch_socket,
};

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
