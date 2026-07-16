//! Shared application composition, loopback hosting, query/Open projection, and watch transport.

#![forbid(unsafe_code)]
#![deny(warnings)]

pub const PACKAGE_BOUNDARY: &str = "bcsp-application";

mod discovery_runtime;
mod host;
mod official_refresh_runtime;
mod product_routes;
mod query_service;
mod refresh_coordinator;
mod refresh_runtime;
mod runtime_core;
mod rutgers_refresh_upstream;
mod service_status;
mod target_refresh_demand;
mod watch_socket;

pub use discovery_runtime::{
    DiscoveryRuntimeError, PublishedRefreshTargets, publish_discovery_for_refresh,
    record_discovery_transport_failure, restore_refresh_targets,
};
pub use host::{
    ExtensionRequest, ExtensionResponse, ExtensionRoute, LoopbackServer, LoopbackServerError,
    RequestMethod, RouteExtension, SHARED_WATCH_SUBPROTOCOL, SessionNonce, WebSocketExtension,
    serve_websocket, spawn_loopback_server, spawn_loopback_server_with_socket,
};
pub use official_refresh_runtime::{
    DISCOVERY_REFRESH_INTERVAL, DISCOVERY_RETRY_INTERVAL, OfficialRefreshRuntime,
    OfficialRefreshRuntimeBuildError,
};
pub use product_routes::{
    PRODUCT_CATALOG_DISCOVERY_PATH, PRODUCT_COURSE_DETAIL_PATH, PRODUCT_COURSE_SEARCH_PATH,
    PRODUCT_FILTER_SCHEMA_PATH, PRODUCT_OPEN_SECTION_STATUS_PATH, PRODUCT_OPEN_STATUS_PATH,
    PRODUCT_SECTION_DETAIL_PATH, PRODUCT_SECTION_SEARCH_PATH, PRODUCT_SERVICE_STATUS_PATH,
    ProductStorageAccess, ProductStorageLockError, SHARED_PRODUCT_ROUTE_INVENTORY,
    SharedProductRoutes, SharedProductStorage,
};
pub use query_service::{SharedQueryError, SharedQueryService};
pub use refresh_coordinator::{
    CatalogPullFailure, CatalogPullResponse, CoordinatorClock, CoordinatorDispatchOutcome,
    CoordinatorError, CoordinatorStatusSink, CoordinatorStatusSnapshot, NoopCoordinatorStatusSink,
    OpenDispatchTerminal, RefreshFuture, RefreshUpstream, ScheduledRefreshTarget,
    SharedRefreshCoordinator, SystemCoordinatorClock,
};
pub use refresh_runtime::{RefreshRuntime, RefreshRuntimeRegistrationError};
pub use runtime_core::{
    ApplicationClock, FixedRefreshPolicyProvider, OpenRuntimeSnapshot, OpenRuntimeSnapshotRegistry,
    OpenRuntimeSnapshotRegistryError, RefreshPolicy, RefreshPolicyError, RefreshPolicyProvider,
    RefreshPolicyReadError, SharedRuntimeContext, SharedRuntimeError, SystemApplicationClock,
};
pub use rutgers_refresh_upstream::{
    RutgersRefreshUpstream, RutgersRefreshUpstreamBuildError, SelectorTargetMembership,
};
pub use service_status::{
    ServiceActivitySnapshot, ServiceStatusRegistry, ServiceStatusRegistryError,
};
pub use target_refresh_demand::{TargetRefreshDemand, TargetRefreshDemandError};
pub use watch_socket::{
    NoopWatchDispatchSink, SharedWatchSocket, SystemWatchClock, WatchAdmissionSource,
    WatchDispatchSink,
};

pub fn boundary_marker() -> &'static str {
    let _ = (
        bcsp_catalog::PACKAGE_BOUNDARY,
        bcsp_contracts::PACKAGE_BOUNDARY,
        bcsp_domain::PACKAGE_BOUNDARY,
        bcsp_open::PACKAGE_BOUNDARY,
        bcsp_operational_storage::PACKAGE_BOUNDARY,
        bcsp_query::PACKAGE_BOUNDARY,
        bcsp_rutgers_client::PACKAGE_BOUNDARY,
        bcsp_watch::PACKAGE_BOUNDARY,
    );
    PACKAGE_BOUNDARY
}

mod dependency_contract {
    use axum as _;
    use serde_json as _;
    use tokio as _;
    use tower as _;
    use tower_http as _;
    use tracing as _;
}
