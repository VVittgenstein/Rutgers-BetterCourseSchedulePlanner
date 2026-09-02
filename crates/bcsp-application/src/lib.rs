//! Shared application composition, loopback hosting, query/Open projection, and watch transport.

#![forbid(unsafe_code)]
#![deny(warnings)]

pub const PACKAGE_BOUNDARY: &str = "bcsp-application";

mod discovery_runtime;
mod host;
mod official_refresh_runtime;
mod prepared_serving;
mod product_routes;
mod product_scope;
mod query_service;
mod refresh_coordinator;
mod refresh_generation;
mod refresh_maintenance;
mod refresh_runtime;
mod runtime_core;
mod rutgers_refresh_upstream;
mod service_status;
mod target_refresh_demand;
mod watch_socket;

// The startup re-derivation of stored delivery columns lives in bcsp-catalog;
// the runtime adapters reach it through this crate so the dependency graph
// (local/public runtime -> application -> catalog) stays as declared.
pub use bcsp_catalog::{
    CATALOG_DERIVATION_VERSION, RederivationError, RederivationReport, TargetRederivationReport,
    rederive_stored_delivery, rederive_stored_delivery_now,
};
pub use discovery_runtime::{
    DiscoveryRuntimeError, PublishedRefreshTargets, publish_discovery_for_refresh,
    record_discovery_transport_failure, restore_refresh_targets,
};
pub use host::{
    BoundedOutboundConfig, BoundedOutboundStats, ExtensionRequest, ExtensionResponse,
    ExtensionRoute, LoopbackServer, LoopbackServerError, MAX_WEBSOCKET_MESSAGE_BYTES,
    OutboundByteBudget, OutboundSendError, OutboundSender, RequestMethod, RouteExtension,
    SHARED_WATCH_SUBPROTOCOL, SecondaryRoutePathRejection, SecondaryWebSocketRoute, SessionNonce,
    WebSocketExtension, serve_websocket, serve_websocket_with_bounded_outbound,
    shared_websocket_upgrade, spawn_loopback_server, spawn_loopback_server_with_socket,
    spawn_loopback_server_with_sockets,
};
pub use official_refresh_runtime::{
    DISCOVERY_RETRY_INTERVAL, OfficialRefreshRuntime, OfficialRefreshRuntimeBuildError,
};
pub use prepared_serving::{
    PREPARED_REQUEST_ADMISSION_WAIT, PreparedCatalogVectorEntry, PreparedOpenOverlay,
    PreparedOpenPublicationBarrier, PreparedOpenVectorEntry, PreparedServingBinding,
    PreparedServingError, PreparedServingRebuildDemand, PreparedServingRebuildRuntime,
    PreparedServingRegistry, PreparedServingSnapshot, build_prepared_serving_snapshot,
    rebuild_prepared_open_from_access, rebuild_prepared_open_overlay,
    rebuild_prepared_serving_from_access,
};
pub use product_routes::{
    PRODUCT_CATALOG_DISCOVERY_PATH, PRODUCT_COURSE_DETAIL_PATH, PRODUCT_COURSE_SEARCH_PATH,
    PRODUCT_DYNAMIC_FILTER_VALIDATION_PATH, PRODUCT_FILTER_OPTIONS_PATH,
    PRODUCT_FILTER_SCHEMA_PATH, PRODUCT_OPEN_SECTION_STATUS_PATH, PRODUCT_OPEN_STATUS_PATH,
    PRODUCT_SECTION_DETAIL_PATH, PRODUCT_SECTION_SEARCH_PATH, PRODUCT_SERVICE_STATUS_PATH,
    ProductStorageAccess, ProductStorageLockError, SHARED_PRODUCT_ROUTE_INVENTORY,
    SharedProductRoutes, SharedProductStorage,
};
pub use product_scope::{
    PRODUCT_CAMPUS_CODES, is_product_campus, product_target_keys, product_term_publication,
};
pub use query_service::{PreparedQueryService, SharedQueryError, SharedQueryService};
pub use refresh_coordinator::{
    CatalogPullFailure, CatalogPullResponse, CoordinatorClock, CoordinatorDispatchOutcome,
    CoordinatorError, CoordinatorStatusSink, CoordinatorStatusSnapshot, NoopCoordinatorStatusSink,
    OpenDispatchTerminal, RefreshFuture, RefreshUpstream, ScheduledRefreshTarget,
    SharedRefreshCoordinator, SystemCoordinatorClock, TargetWorkActivity, TargetWorkflowKind,
    WorkflowOperationActivity, WorkflowOperationId,
};
pub use refresh_generation::DISCOVERY_REFRESH_INTERVAL;
pub use refresh_maintenance::{
    StorageConnectionInventoryEntry, WAL_ESCALATION_MIN_INTERVAL, WAL_RESTART_LOG_FRAMES,
    WAL_STARVATION_MIN_LOG_FRAMES, WAL_STARVATION_ZERO_PROGRESS_REPORTS, WAL_TRUNCATE_LOG_FRAMES,
    WalMaintenanceOutcome, WalMaintenancePolicy, WalMaintenanceReason, WalMaintenanceSignal,
    WalMaintenanceState, WalStarvationDetector,
};
pub use refresh_runtime::{
    REFRESH_MAX_CONCURRENCY, RefreshRuntime, RefreshRuntimeRegistrationError,
};
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
pub use target_refresh_demand::{
    ManualTargetRetryRequest, TargetRefreshDemand, TargetRefreshDemandError,
};
pub use watch_socket::{
    NoopWatchDispatchSink, SharedWatchSocket, SystemWatchClock, WATCH_APP_PING_INTERVAL,
    WatchAdmissionSource, WatchDispatchSink,
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
    use rusqlite as _;
    use serde_json as _;
    use tokio as _;
    use tower as _;
    use tower_http as _;
    use tracing as _;
}
