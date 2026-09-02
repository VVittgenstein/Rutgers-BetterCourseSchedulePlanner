//! Windows-local runtime adapter boundary.

#![forbid(unsafe_code)]
#![deny(warnings)]

pub const PACKAGE_BOUNDARY: &str = "bcsp-local-runtime";

mod bootstrap;
mod console;
mod desired;
mod diagnostic;
mod extension;
mod history;
mod instance;
mod lifecycle;
mod path;
mod personal;
mod policy;
mod presence;
mod product;
mod watch;

pub use bootstrap::{
    LocalBootstrapError, LocalPrimaryDatabase, LocalRuntimeState, OperationalGate,
};
pub use console::{
    LocalConsole, LocalConsoleEvent, LocalConsoleLayer, LocalConsoleLocale, LocalConsoleSink,
    LocalExitReason,
};
pub use desired::{
    DESIRED_WATCH_ABSENT_COMMITTED_NUMBER, DESIRED_WATCH_MATERIALIZE_BACKOFF,
    DESIRED_WATCH_REVALIDATE_INTERVAL, DesiredWatchCommittedV1, DesiredWatchCoordinator,
    DesiredWatchCoordinatorError, DesiredWatchEntryV1, DesiredWatchFailureClassV1,
    DesiredWatchFailureReasonV1, DesiredWatchFailureV1, DesiredWatchMaterializedV1,
    DesiredWatchMutationResultV1, DesiredWatchMutationV1, DesiredWatchOutcomeV1, DesiredWatchOwner,
    DesiredWatchStateV1, LOCAL_DESIRED_WATCH_CONTRACT_VERSION, LOCAL_DESIRED_WATCH_PATH,
    LOCAL_DESIRED_WATCH_RESPONSE_BUDGET_BYTES,
};
pub use diagnostic::StartupFailureReport;
pub use extension::{
    LocalApiErrorCode, LocalRouteExtension, LocalSurfaceFailure, LocalSurfaceOutcome,
    LocalSurfaceState,
};
pub use instance::{
    ExistingLocalInstance, LOCAL_INSTANCE_LOCK_FILE_NAME, LocalInstanceClaim, LocalInstanceError,
    LoopbackBrowserUrl, PrimaryInstanceLease,
};
pub use lifecycle::{
    LocalRuntimeError, PreparedLocalRuntime, RunningLocalRuntime, SKIP_BROWSER_LAUNCH_ENVIRONMENT,
    browser_launch_skipped, prepare_and_start_with, run, run_blocking,
};
pub use path::{
    LOCAL_DATA_DIRECTORY_NAME, LOCAL_DATABASE_FILE_NAME, LocalPathError, LocalRuntimePaths,
};
pub use personal::PersonalSurface;
pub use policy::{LocalRefreshPolicyProvider, LocalRuntimeCore, create_local_runtime_core};
pub use presence::{
    LOCAL_IDLE_EXIT_COUNTDOWN, LOCAL_PRESENCE_HELLO_DEADLINE, LocalPresenceCommandV1,
    LocalPresenceEventV1, LocalPresencePhase, LocalPresenceRoute, LocalPresenceStateV1,
};
pub use product::LOCAL_PREPARED_ADMISSION_WAIT;
pub use watch::{LOCAL_PRESENCE_SOCKET_PATH, LocalWatchRoute, create_local_watch_socket};

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
