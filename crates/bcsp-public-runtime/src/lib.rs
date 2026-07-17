//! Linux-public HTTP/WebSocket runtime and ephemeral document-session adapter.

#![forbid(unsafe_code)]
#![deny(warnings)]

mod config;
mod host;
mod product;
mod session;
mod status;
mod watch;

pub use config::{
    PUBLIC_BIND_ADDRESS, PUBLIC_CATALOG_INTERVAL_SECONDS, PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS,
    PUBLIC_WATCHED_OPEN_INTERVAL_SECONDS, PublicHostConfig, PublicHostConfigError,
    fixed_public_refresh_policy,
};
pub use host::{
    NoPublicProductRoutes, PUBLIC_LIVENESS_PATH, PUBLIC_METRICS_PATH, PUBLIC_READINESS_PATH,
    PUBLIC_RUNTIME_ROUTE_INVENTORY, PUBLIC_WATCH_PATH, PUBLIC_WS_SUBPROTOCOL, PublicRuntime,
    PublicRuntimeError, build_production_runtime, run_production,
};
pub use product::{
    PublicProductRoutes, PublicProductStorageAccess, PublicProductStorageGuard,
    create_public_product_routes,
};
pub use session::{PublicLocale, negotiate_locale};
pub use status::{
    InMemoryPublicSchedulerStatus, PublicClock, PublicDayCounters, PublicReadinessReason,
    PublicSchedulerSnapshot, PublicSchedulerStatusSource, PublicServiceInspector,
    PublicServiceSnapshot, PublicServiceStateSource, PublicStatusLevel, PublicWatchDemandSource,
    SharedPublicOperationalStore, SystemPublicClock,
};
pub use watch::create_public_watch_socket;

pub const PACKAGE_BOUNDARY: &str = "bcsp-public-runtime";

pub fn boundary_marker() -> &'static str {
    let _ = (
        bcsp_application::PACKAGE_BOUNDARY,
        bcsp_contracts::PACKAGE_BOUNDARY,
        bcsp_open::PACKAGE_BOUNDARY,
        bcsp_operational_storage::PACKAGE_BOUNDARY,
        bcsp_public_operations::PACKAGE_BOUNDARY,
        bcsp_watch::PACKAGE_BOUNDARY,
    );
    PACKAGE_BOUNDARY
}

mod dependency_contract {
    use include_dir as _;
    use tracing as _;
    use tracing_subscriber as _;
}
