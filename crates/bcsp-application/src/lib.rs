//! Shared application composition, loopback hosting, query/Open projection, and watch transport.

#![forbid(unsafe_code)]
#![deny(warnings)]

pub const PACKAGE_BOUNDARY: &str = "bcsp-application";

mod host;
mod query_service;
mod runtime_core;
mod watch_socket;

pub use host::{
    ExtensionRequest, ExtensionResponse, LoopbackServer, LoopbackServerError, RequestMethod,
    RouteExtension, SessionNonce, WebSocketExtension, serve_websocket, spawn_loopback_server,
    spawn_loopback_server_with_socket,
};
pub use query_service::{SharedQueryError, SharedQueryService};
pub use runtime_core::{
    ApplicationClock, FixedRefreshPolicyProvider, OpenRuntimeSnapshot, RefreshPolicy,
    RefreshPolicyError, RefreshPolicyProvider, RefreshPolicyReadError, SharedRuntimeContext,
    SharedRuntimeError, SystemApplicationClock,
};
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
