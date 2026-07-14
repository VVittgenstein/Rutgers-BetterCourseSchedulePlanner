use std::fmt::Write as _;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Request, State};
use axum::http::header::{
    ACCEPT_LANGUAGE, CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, HOST, ORIGIN,
    REFERRER_POLICY, SEC_WEBSOCKET_PROTOCOL, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::{any, get};
use bcsp_application::{
    ExtensionRequest, ExtensionResponse, ExtensionRoute, FixedRefreshPolicyProvider,
    OfficialRefreshRuntime, OfficialRefreshRuntimeBuildError, OpenRuntimeSnapshotRegistry,
    RefreshPolicyError, RequestMethod, RouteExtension, SHARED_WATCH_SUBPROTOCOL, SharedWatchSocket,
    TargetRefreshDemand, WebSocketExtension, serve_websocket,
};
use bcsp_contracts::{
    ActiveWatchTargetV1, ApiErrorBody, ApiErrorCode, ApiErrorEnvelope, FilterRequestV1,
    FilterSchemaV1, HttpSuccessEnvelope, SectionKey, SystemTraceIdSource, TraceIdSource,
    WatchAlertV1, WatchPolicyV1, filter_schema_v1,
};
use bcsp_open::OpenCounterAudience;
use bcsp_public_operations::{PublicOperationalStore, PublicOperationsError};
use serde::Serialize;
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::config::{
    PUBLIC_CATALOG_INTERVAL_SECONDS, PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS,
    PUBLIC_WATCHED_OPEN_INTERVAL_SECONDS, PublicHostConfig, PublicHostConfigError,
};
use crate::product::create_public_product_routes;
use crate::session::{
    DocumentSessionError, DocumentSessionRegistry, PublicLocale, negotiate_locale,
};
use crate::status::{
    InMemoryPublicSchedulerStatus, PublicSchedulerStatusSource, PublicServiceInspector,
    PublicServiceSnapshot, PublicServiceStateSource, SharedPublicOperationalStore,
};
use crate::watch::create_public_watch_socket;

const MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;
const MAX_WEBSOCKET_MESSAGE_BYTES: usize = 64 * 1024;
const SESSION_HEADER: &str = "x-bcsp-session";
pub const PUBLIC_WS_SUBPROTOCOL: &str = SHARED_WATCH_SUBPROTOCOL;
const WATCH_MAINTENANCE_INTERVAL: Duration = Duration::from_millis(250);
const SHUTDOWN_DRAIN_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_VOLUME_PERCENT: u8 = 100;
pub const PUBLIC_LIVENESS_PATH: &str = "/health/live";
pub const PUBLIC_READINESS_PATH: &str = "/health/ready";
pub const PUBLIC_METRICS_PATH: &str = "/metrics";
pub const PUBLIC_WATCH_PATH: &str = "/api/v1/watch";
pub static PUBLIC_RUNTIME_ROUTE_INVENTORY: &[ExtensionRoute] = &[
    ExtensionRoute::new(RequestMethod::Get, PUBLIC_LIVENESS_PATH),
    ExtensionRoute::new(RequestMethod::Head, PUBLIC_LIVENESS_PATH),
    ExtensionRoute::new(RequestMethod::Get, PUBLIC_READINESS_PATH),
    ExtensionRoute::new(RequestMethod::Head, PUBLIC_READINESS_PATH),
    ExtensionRoute::new(RequestMethod::Get, PUBLIC_METRICS_PATH),
    ExtensionRoute::new(RequestMethod::Head, PUBLIC_METRICS_PATH),
    ExtensionRoute::new(RequestMethod::Get, PUBLIC_WATCH_PATH),
];
const SHELL_TEMPLATE: &str = r#"<!doctype html>
<html lang="__BCSP_LANG__">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width,initial-scale=1">
    <title>Rutgers Better Course Schedule Planner</title>
  </head>
  <body>
    <main id="root" aria-live="polite"></main>
    <script id="bcsp-bootstrap" type="application/json" nonce="__BCSP_NONCE__">__BCSP_BOOTSTRAP__</script>
  </body>
</html>
"#;

#[derive(Clone)]
struct PublicHostState {
    config: Arc<PublicHostConfig>,
    sessions: Arc<DocumentSessionRegistry>,
    service: Arc<dyn PublicServiceStateSource>,
    product_routes: Arc<dyn RouteExtension>,
    watch: Arc<SharedWatchSocket>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicRefreshPolicyView {
    catalog_interval_seconds: u64,
    open_general_interval_seconds: u32,
    open_watched_interval_seconds: u32,
    user_configurable: bool,
}

impl Default for PublicRefreshPolicyView {
    fn default() -> Self {
        Self {
            catalog_interval_seconds: PUBLIC_CATALOG_INTERVAL_SECONDS,
            open_general_interval_seconds: PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS,
            open_watched_interval_seconds: PUBLIC_WATCHED_OPEN_INTERVAL_SECONDS,
            user_configurable: false,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PublicDocumentBootstrap {
    release: String,
    session_nonce: String,
    locale: PublicLocale,
    filter_schema: FilterSchemaV1,
    current_filters: Option<FilterRequestV1>,
    selected_sections: Vec<SectionKey>,
    active_watches: Vec<ActiveWatchTargetV1>,
    current_page_alerts: Vec<WatchAlertV1>,
    volume_percent: u8,
    watch_policy: WatchPolicyV1,
    refresh_policy: PublicRefreshPolicyView,
    service: PublicServiceSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum LivenessState {
    Live,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LivenessResponse {
    status: LivenessState,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoPublicProductRoutes;

impl RouteExtension for NoPublicProductRoutes {
    fn handle(&self, _request: ExtensionRequest) -> ExtensionResponse {
        ExtensionResponse::not_found()
    }
}

pub struct PublicRuntime {
    address: std::net::SocketAddr,
    scheduler: Arc<InMemoryPublicSchedulerStatus>,
    open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
    watch: Arc<SharedWatchSocket>,
    refresh: Option<OfficialRefreshRuntime>,
    shutdown: Option<oneshot::Sender<()>>,
    server_task: Option<JoinHandle<Result<(), std::io::Error>>>,
    maintenance_task: Option<JoinHandle<()>>,
}

impl PublicRuntime {
    pub async fn spawn(
        config: PublicHostConfig,
        store: SharedPublicOperationalStore,
        product_routes: Arc<dyn RouteExtension>,
    ) -> Result<Self, PublicRuntimeError> {
        Self::spawn_with_open_runtime(
            config,
            store,
            product_routes,
            Arc::new(OpenRuntimeSnapshotRegistry::default()),
        )
        .await
    }

    pub async fn spawn_with_open_runtime(
        config: PublicHostConfig,
        store: SharedPublicOperationalStore,
        product_routes: Arc<dyn RouteExtension>,
        open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
    ) -> Result<Self, PublicRuntimeError> {
        let watch = create_public_watch_socket(store.clone(), open_runtime.clone())
            .map_err(|_| PublicRuntimeError::WatchInitialization)?;
        let scheduler = Arc::new(InMemoryPublicSchedulerStatus::default());
        let scheduler_source: Arc<dyn PublicSchedulerStatusSource> = scheduler.clone();
        let service: Arc<dyn PublicServiceStateSource> = Arc::new(
            PublicServiceInspector::with_system_clock(store, scheduler_source, watch.clone()),
        );
        Self::spawn_with_state_and_open_runtime(
            config,
            product_routes,
            service,
            watch,
            scheduler,
            open_runtime,
        )
        .await
    }

    #[cfg(test)]
    async fn spawn_with_state(
        config: PublicHostConfig,
        product_routes: Arc<dyn RouteExtension>,
        service: Arc<dyn PublicServiceStateSource>,
        watch: Arc<SharedWatchSocket>,
        scheduler: Arc<InMemoryPublicSchedulerStatus>,
    ) -> Result<Self, PublicRuntimeError> {
        Self::spawn_with_state_and_open_runtime(
            config,
            product_routes,
            service,
            watch,
            scheduler,
            Arc::new(OpenRuntimeSnapshotRegistry::default()),
        )
        .await
    }

    async fn spawn_with_state_and_open_runtime(
        config: PublicHostConfig,
        product_routes: Arc<dyn RouteExtension>,
        service: Arc<dyn PublicServiceStateSource>,
        watch: Arc<SharedWatchSocket>,
        scheduler: Arc<InMemoryPublicSchedulerStatus>,
        open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
    ) -> Result<Self, PublicRuntimeError> {
        let listener = TcpListener::bind(config.bind())
            .await
            .map_err(|_| PublicRuntimeError::Bind)?;
        let address = listener
            .local_addr()
            .map_err(|_| PublicRuntimeError::Bind)?;
        let state = PublicHostState {
            config: Arc::new(config),
            sessions: Arc::new(DocumentSessionRegistry::default()),
            service,
            product_routes,
            watch: watch.clone(),
        };
        let router = public_router(state);
        let (shutdown, shutdown_receiver) = oneshot::channel();
        let server_task = tokio::spawn(async move {
            axum::serve(listener, router)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_receiver.await;
                })
                .await
        });
        let maintenance_watch = watch.clone();
        let maintenance_task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(WATCH_MAINTENANCE_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                maintenance_watch.tick();
            }
        });
        Ok(Self {
            address,
            scheduler,
            open_runtime,
            watch,
            refresh: None,
            shutdown: Some(shutdown),
            server_task: Some(server_task),
            maintenance_task: Some(maintenance_task),
        })
    }

    pub const fn address(&self) -> std::net::SocketAddr {
        self.address
    }

    pub fn scheduler_status(&self) -> Arc<InMemoryPublicSchedulerStatus> {
        self.scheduler.clone()
    }

    pub fn open_runtime(&self) -> Arc<OpenRuntimeSnapshotRegistry> {
        self.open_runtime.clone()
    }

    pub fn watch_socket(&self) -> Arc<SharedWatchSocket> {
        self.watch.clone()
    }

    pub async fn shutdown(mut self) -> Result<(), PublicRuntimeError> {
        if let Some(refresh) = self.refresh.take() {
            refresh.shutdown().await;
        }
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.watch.stop();
        if let Some(task) = self.maintenance_task.take() {
            task.abort();
            let _ = task.await;
        }
        if let Some(mut task) = self.server_task.take() {
            match tokio::time::timeout(SHUTDOWN_DRAIN_TIMEOUT, &mut task).await {
                Ok(result) => result
                    .map_err(|_| PublicRuntimeError::ServerTask)?
                    .map_err(|_| PublicRuntimeError::Serve)?,
                Err(_) => {
                    task.abort();
                    let _ = task.await;
                    self.watch.stop();
                    return Err(PublicRuntimeError::ShutdownTimeout);
                }
            }
        }
        self.watch.stop();
        Ok(())
    }
}

impl Drop for PublicRuntime {
    fn drop(&mut self) {
        drop(self.refresh.take());
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.watch.stop();
        if let Some(task) = self.maintenance_task.take() {
            task.abort();
        }
    }
}

pub async fn build_production_runtime() -> Result<PublicRuntime, PublicRuntimeError> {
    let config = PublicHostConfig::from_production_environment()
        .map_err(PublicRuntimeError::Configuration)?;
    let store =
        PublicOperationalStore::open_production().map_err(PublicRuntimeError::OperationalState)?;
    let store = Arc::new(Mutex::new(store));
    let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
    let target_refresh_demand = TargetRefreshDemand::default();
    let product_routes = create_public_product_routes(store.clone(), open_runtime.clone())
        .map_err(PublicRuntimeError::ProductComposition)?
        .with_target_refresh_demand(target_refresh_demand.clone());
    let mut runtime = PublicRuntime::spawn_with_open_runtime(
        config,
        store.clone(),
        Arc::new(product_routes),
        open_runtime.clone(),
    )
    .await?;
    let policy = FixedRefreshPolicyProvider::new(
        crate::fixed_public_refresh_policy().map_err(PublicRuntimeError::ProductComposition)?,
    );
    let mut ids = SystemTraceIdSource;
    let refresh = OfficialRefreshRuntime::spawn_with_target_refresh_demand(
        crate::PublicProductStorageAccess::new(store),
        policy,
        ids.next_trace_id(),
        OpenCounterAudience::Public,
        runtime.watch.clone(),
        open_runtime,
        runtime.scheduler.clone(),
        target_refresh_demand,
    )
    .map_err(PublicRuntimeError::RefreshStartup)?;
    runtime.refresh = Some(refresh);
    Ok(runtime)
}

pub async fn run_production() -> Result<(), PublicRuntimeError> {
    initialize_tracing();
    let runtime = build_production_runtime().await?;
    tracing::info!(code = "PUBLIC_RUNTIME_STARTED");
    wait_for_shutdown_signal().await?;
    tracing::info!(code = "PUBLIC_RUNTIME_STOPPING");
    runtime.shutdown().await
}

fn initialize_tracing() {
    let _ = tracing_subscriber::fmt()
        .json()
        .with_target(false)
        .with_current_span(false)
        .with_span_list(false)
        .try_init();
}

#[cfg(unix)]
async fn wait_for_shutdown_signal() -> Result<(), PublicRuntimeError> {
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        .map_err(|_| PublicRuntimeError::Signal)?;
    tokio::select! {
        result = tokio::signal::ctrl_c() => result.map_err(|_| PublicRuntimeError::Signal),
        _ = terminate.recv() => Ok(()),
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown_signal() -> Result<(), PublicRuntimeError> {
    tokio::signal::ctrl_c()
        .await
        .map_err(|_| PublicRuntimeError::Signal)
}

fn public_router(state: PublicHostState) -> Router {
    PUBLIC_RUNTIME_ROUTE_INVENTORY
        .iter()
        .filter(|route| route.method() == &RequestMethod::Get)
        .fold(Router::<PublicHostState>::new(), register_builtin_get_route)
        .fallback(any(handle_fallback))
        .with_state(state)
}

fn register_builtin_get_route(
    router: Router<PublicHostState>,
    route: &ExtensionRoute,
) -> Router<PublicHostState> {
    match route.path() {
        PUBLIC_LIVENESS_PATH => router.route(route.path(), get(handle_liveness)),
        PUBLIC_READINESS_PATH => router.route(route.path(), get(handle_readiness)),
        PUBLIC_METRICS_PATH => router.route(route.path(), get(handle_metrics)),
        PUBLIC_WATCH_PATH => router.route(route.path(), get(handle_watch_socket)),
        _ => panic!("unknown public built-in route inventory entry"),
    }
}

async fn handle_liveness(State(state): State<PublicHostState>, request: Request) -> Response {
    if !valid_host(request.headers(), &state) {
        return api_error_response(
            StatusCode::MISDIRECTED_REQUEST,
            ApiErrorCode::MalformedRequest,
        );
    }
    json_response(
        StatusCode::OK,
        &HttpSuccessEnvelope::new(LivenessResponse {
            status: LivenessState::Live,
        }),
    )
}

async fn handle_readiness(State(state): State<PublicHostState>, request: Request) -> Response {
    if !valid_host(request.headers(), &state) {
        return api_error_response(
            StatusCode::MISDIRECTED_REQUEST,
            ApiErrorCode::MalformedRequest,
        );
    }
    let snapshot = state.service.snapshot();
    let status = if snapshot.ready {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    json_response(status, &HttpSuccessEnvelope::new(snapshot))
}

async fn handle_metrics(State(state): State<PublicHostState>, request: Request) -> Response {
    if !valid_host(request.headers(), &state) {
        return api_error_response(
            StatusCode::MISDIRECTED_REQUEST,
            ApiErrorCode::MalformedRequest,
        );
    }
    let snapshot = state.service.snapshot();
    text_response(
        StatusCode::OK,
        "text/plain; version=0.0.4; charset=utf-8",
        render_metrics(&snapshot),
    )
}

async fn handle_watch_socket(
    State(state): State<PublicHostState>,
    upgrade: WebSocketUpgrade,
    request: Request,
) -> Response {
    let headers = request.headers();
    let session = strict_session_query(request.uri().query());
    let authenticated = valid_host(headers, &state)
        && header_text(headers, ORIGIN).as_deref() == Some(state.config.external_origin())
        && requested_subprotocol(headers)
        && session.is_some_and(|nonce| matches!(state.sessions.locale(nonce), Ok(Some(_))));
    if !authenticated {
        return api_error_response(StatusCode::FORBIDDEN, ApiErrorCode::MalformedRequest);
    }
    let mut source = SystemTraceIdSource;
    let connection_id = source.next_trace_id();
    let extension: Arc<dyn WebSocketExtension> = state.watch;
    upgrade
        .protocols([PUBLIC_WS_SUBPROTOCOL])
        .max_message_size(MAX_WEBSOCKET_MESSAGE_BYTES)
        .max_frame_size(MAX_WEBSOCKET_MESSAGE_BYTES)
        .on_upgrade(move |socket| serve_websocket(socket, extension, connection_id))
}

async fn handle_fallback(State(state): State<PublicHostState>, request: Request) -> Response {
    if !valid_host(request.headers(), &state) {
        return api_error_response(
            StatusCode::MISDIRECTED_REQUEST,
            ApiErrorCode::MalformedRequest,
        );
    }
    let method = RequestMethod::from_http(request.method());
    let path = request.uri().path().to_owned();
    if method == RequestMethod::Get && !path.starts_with("/api/") {
        return document_response(&state, request.headers());
    }
    if !path.starts_with("/api/")
        || !state
            .product_routes
            .route_inventory()
            .iter()
            .any(|route| route.matches(&method, &path))
    {
        return extension_response(ExtensionResponse::not_found());
    }
    if method.changes_state() && !authenticated_mutation(request.headers(), &state) {
        return api_error_response(StatusCode::FORBIDDEN, ApiErrorCode::MalformedRequest);
    }
    let query = request.uri().query().map(str::to_owned);
    let body = match to_bytes(request.into_body(), MAX_REQUEST_BODY_BYTES).await {
        Ok(body) => body.to_vec(),
        Err(_) => {
            return api_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                ApiErrorCode::MalformedRequest,
            );
        }
    };
    extension_response(
        state
            .product_routes
            .handle(ExtensionRequest::new(method, path, query, body)),
    )
}

fn document_response(state: &PublicHostState, headers: &HeaderMap) -> Response {
    let locale = negotiate_locale(header_text(headers, ACCEPT_LANGUAGE).as_deref());
    let nonce = match state.sessions.issue(locale) {
        Ok(nonce) => nonce,
        Err(DocumentSessionError::Unavailable) => {
            return api_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                ApiErrorCode::InternalError,
            );
        }
    };
    let bootstrap = HttpSuccessEnvelope::new(PublicDocumentBootstrap {
        release: state.config.release().to_owned(),
        session_nonce: nonce.to_string(),
        locale,
        filter_schema: filter_schema_v1(),
        current_filters: state.service.default_current_filters(),
        selected_sections: Vec::new(),
        active_watches: Vec::new(),
        current_page_alerts: Vec::new(),
        volume_percent: DEFAULT_VOLUME_PERCENT,
        watch_policy: WatchPolicyV1::default(),
        refresh_policy: PublicRefreshPolicyView::default(),
        service: state.service.snapshot(),
    });
    let bootstrap = match serde_json::to_string(&bootstrap) {
        Ok(value) => escape_inline_json(&value),
        Err(_) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiErrorCode::InternalError,
            );
        }
    };
    let html = SHELL_TEMPLATE
        .replace("__BCSP_LANG__", locale.html_lang())
        .replace("__BCSP_NONCE__", nonce.as_str())
        .replace("__BCSP_BOOTSTRAP__", &bootstrap);
    html_response(StatusCode::OK, html, nonce.as_str())
}

fn authenticated_mutation(headers: &HeaderMap, state: &PublicHostState) -> bool {
    if header_text(headers, ORIGIN).as_deref() != Some(state.config.external_origin()) {
        return false;
    }
    header_text_name(headers, SESSION_HEADER)
        .is_some_and(|nonce| matches!(state.sessions.locale(&nonce), Ok(Some(_))))
}

fn valid_host(headers: &HeaderMap, state: &PublicHostState) -> bool {
    header_text(headers, HOST).as_deref() == Some(state.config.external_authority())
}

fn strict_session_query(query: Option<&str>) -> Option<&str> {
    let field = query?;
    let value = field.strip_prefix("session=")?;
    (!value.is_empty() && !value.contains('&')).then_some(value)
}

fn requested_subprotocol(headers: &HeaderMap) -> bool {
    header_text(headers, SEC_WEBSOCKET_PROTOCOL).is_some_and(|value| {
        value
            .split(',')
            .map(str::trim)
            .any(|protocol| protocol == PUBLIC_WS_SUBPROTOCOL)
    })
}

fn header_text(headers: &HeaderMap, name: axum::http::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn header_text_name(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn escape_inline_json(value: &str) -> String {
    value
        .replace('&', "\\u0026")
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

fn render_metrics(snapshot: &PublicServiceSnapshot) -> String {
    let mut output = String::new();
    let ready = u8::from(snapshot.ready);
    let circuit = u8::from(snapshot.scheduler.origin_circuit_open);
    let overloaded = u8::from(snapshot.scheduler.overloaded);
    let in_flight = u8::from(snapshot.scheduler.in_flight);
    let _ = writeln!(output, "bcsp_service_live 1");
    let _ = writeln!(output, "bcsp_service_ready {ready}");
    let _ = writeln!(
        output,
        "bcsp_catalog_targets {}",
        snapshot.catalog_target_count
    );
    let _ = writeln!(
        output,
        "bcsp_catalog_targets_available {}",
        snapshot.catalog_available_target_count
    );
    let _ = writeln!(
        output,
        "bcsp_open_targets_available {}",
        snapshot.open_available_target_count
    );
    let _ = writeln!(
        output,
        "bcsp_active_watches {}",
        snapshot.active_watch_count
    );
    let _ = writeln!(
        output,
        "bcsp_websocket_connections {}",
        snapshot.websocket_connection_count
    );
    let _ = writeln!(
        output,
        "bcsp_scheduler_lag_milliseconds {}",
        snapshot.scheduler.maximum_lag_milliseconds
    );
    let _ = writeln!(output, "bcsp_scheduler_origin_circuit_open {circuit}");
    let _ = writeln!(output, "bcsp_scheduler_overloaded {overloaded}");
    let _ = writeln!(output, "bcsp_scheduler_in_flight {in_flight}");
    let _ = writeln!(
        output,
        "bcsp_open_attempted_rutgers_day {}",
        snapshot.today_counts.attempted
    );
    let _ = writeln!(
        output,
        "bcsp_open_succeeded_rutgers_day {}",
        snapshot.today_counts.succeeded
    );
    let _ = writeln!(
        output,
        "bcsp_open_failed_rutgers_day {}",
        snapshot.today_counts.failed
    );
    let _ = writeln!(
        output,
        "bcsp_open_empty_rutgers_day {}",
        snapshot.today_counts.empty
    );
    let _ = writeln!(
        output,
        "bcsp_catalog_requested_interval_seconds {}",
        PUBLIC_CATALOG_INTERVAL_SECONDS
    );
    let _ = writeln!(
        output,
        "bcsp_open_general_requested_interval_seconds {}",
        PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS
    );
    let _ = writeln!(
        output,
        "bcsp_open_watched_requested_interval_seconds {}",
        PUBLIC_WATCHED_OPEN_INTERVAL_SECONDS
    );
    output
}

fn api_error_response(status: StatusCode, code: ApiErrorCode) -> Response {
    let mut source = SystemTraceIdSource;
    let envelope = ApiErrorEnvelope::new(ApiErrorBody::new(code, source.next_trace_id()));
    json_response(status, &envelope)
}

fn json_response(status: StatusCode, value: &impl Serialize) -> Response {
    match serde_json::to_vec(value) {
        Ok(body) => secured_response(status, "application/json; charset=utf-8", body, None),
        Err(_) => secured_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "application/json; charset=utf-8",
            br#"{"error":{"code":"INTERNAL_ERROR"}}"#.to_vec(),
            None,
        ),
    }
}

fn html_response(status: StatusCode, body: String, nonce: &str) -> Response {
    secured_response(
        status,
        "text/html; charset=utf-8",
        body.into_bytes(),
        Some(nonce),
    )
}

fn text_response(status: StatusCode, content_type: &'static str, body: String) -> Response {
    secured_response(status, content_type, body.into_bytes(), None)
}

fn extension_response(value: ExtensionResponse) -> Response {
    let status = StatusCode::from_u16(value.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    secured_response(status, value.content_type(), value.body().to_vec(), None)
}

fn secured_response(
    status: StatusCode,
    content_type: &'static str,
    body: Vec<u8>,
    script_nonce: Option<&str>,
) -> Response {
    let mut response = Response::new(Body::from(body));
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(
        "cross-origin-opener-policy",
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        "cross-origin-resource-policy",
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), geolocation=(), microphone=()"),
    );
    let policy = script_nonce.map_or_else(
        || {
            "default-src 'none'; base-uri 'none'; frame-ancestors 'none'; object-src 'none'"
                .to_owned()
        },
        |nonce| {
            format!(
                "default-src 'self'; base-uri 'none'; frame-ancestors 'none'; object-src 'none'; connect-src 'self'; img-src 'self' data:; font-src 'self'; style-src 'self'; script-src 'self' 'nonce-{nonce}'; worker-src 'none'; form-action 'none'"
            )
        },
    );
    if let Ok(policy) = HeaderValue::from_str(&policy) {
        headers.insert(CONTENT_SECURITY_POLICY, policy);
    }
    response
}

#[derive(Debug, Error)]
pub enum PublicRuntimeError {
    #[error("public runtime configuration is unavailable")]
    Configuration(#[source] PublicHostConfigError),
    #[error("public operational state is unavailable")]
    OperationalState(#[source] PublicOperationsError),
    #[error("public product routes could not be composed")]
    ProductComposition(#[source] RefreshPolicyError),
    #[error("public watch runtime could not be initialized")]
    WatchInitialization,
    #[error("official Rutgers refresh clients could not be initialized")]
    RefreshStartup(#[source] OfficialRefreshRuntimeBuildError),
    #[error("public loopback listener could not be bound")]
    Bind,
    #[error("public HTTP service failed")]
    Serve,
    #[error("public HTTP service task failed")]
    ServerTask,
    #[error("public shutdown signal could not be installed")]
    Signal,
    #[error("public HTTP service did not drain before its shutdown deadline")]
    ShutdownTimeout,
}

impl PublicRuntimeError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Configuration(error) => error.code(),
            Self::OperationalState(error) => error.code(),
            Self::ProductComposition(_) => "PUBLIC_PRODUCT_COMPOSITION_FAILED",
            Self::WatchInitialization => "PUBLIC_WATCH_INITIALIZATION_FAILED",
            Self::RefreshStartup(_) => "PUBLIC_REFRESH_INITIALIZATION_FAILED",
            Self::Bind => "PUBLIC_BIND_FAILED",
            Self::Serve => "PUBLIC_SERVE_FAILED",
            Self::ServerTask => "PUBLIC_SERVER_TASK_FAILED",
            Self::Signal => "PUBLIC_SIGNAL_FAILED",
            Self::ShutdownTimeout => "PUBLIC_SHUTDOWN_TIMEOUT",
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use bcsp_contracts::TermCampusKey;
    use reqwest::Client;
    use serde_json::Value;
    use tempfile::TempDir;

    use super::*;
    use crate::status::{
        PublicDayCounters, PublicReadinessReason, PublicSchedulerSnapshot, PublicStatusLevel,
        PublicWatchDemandSource,
    };

    const TEST_ORIGIN: &str = "https://planner.example.test";
    const TEST_AUTHORITY: &str = "planner.example.test";
    static TEST_PRODUCT_ROUTE_INVENTORY: &[ExtensionRoute] = &[
        ExtensionRoute::new(RequestMethod::Get, "/api/v1/query/courses"),
        ExtensionRoute::new(RequestMethod::Post, "/api/v1/query/courses"),
    ];

    #[derive(Default)]
    struct CountingProductRoutes {
        calls: AtomicUsize,
        origin_starts: AtomicUsize,
        paths: Mutex<Vec<String>>,
    }

    impl RouteExtension for CountingProductRoutes {
        fn route_inventory(&self) -> &'static [ExtensionRoute] {
            TEST_PRODUCT_ROUTE_INVENTORY
        }

        fn handle(&self, request: ExtensionRequest) -> ExtensionResponse {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.paths.lock().expect("route paths").push(format!(
                "{:?} {}",
                request.method(),
                request.path()
            ));
            ExtensionResponse::json_bytes(200, br#"{"source":"shared-state"}"#.to_vec())
        }
    }

    struct StaticServiceState {
        snapshot: PublicServiceSnapshot,
    }

    impl PublicServiceStateSource for StaticServiceState {
        fn snapshot(&self) -> PublicServiceSnapshot {
            self.snapshot.clone()
        }

        fn default_current_filters(&self) -> Option<FilterRequestV1> {
            None
        }
    }

    fn test_config() -> PublicHostConfig {
        PublicHostConfig::try_new(
            "127.0.0.1:0".parse().expect("loopback address"),
            TEST_ORIGIN,
            "test-release",
        )
        .expect("public test config")
    }

    fn client() -> Client {
        Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .expect("test HTTP client")
    }

    async fn spawn_runtime(
        routes: Arc<dyn RouteExtension>,
    ) -> (TempDir, PublicRuntime, SharedPublicOperationalStore) {
        let temp = TempDir::new().expect("temporary directory");
        let store = Arc::new(Mutex::new(
            PublicOperationalStore::open_for_state_root(temp.path().join("state"))
                .expect("public operational state"),
        ));
        let runtime = PublicRuntime::spawn(test_config(), store.clone(), routes)
            .await
            .expect("public runtime");
        (temp, runtime, store)
    }

    fn request_url(runtime: &PublicRuntime, path: &str) -> String {
        format!("http://{}{}", runtime.address(), path)
    }

    async fn document(
        client: &Client,
        runtime: &PublicRuntime,
        path: &str,
        accept_language: &str,
    ) -> (reqwest::header::HeaderMap, String, Value) {
        let response = client
            .get(request_url(runtime, path))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .header(ACCEPT_LANGUAGE.as_str(), accept_language)
            .send()
            .await
            .expect("document response");
        assert_eq!(response.status(), StatusCode::OK);
        let headers = response.headers().clone();
        let html = response.text().await.expect("document body");
        let marker = r#"<script id="bcsp-bootstrap""#;
        let script = &html[html.find(marker).expect("bootstrap script")..];
        let content_start = script.find('>').expect("script opening tag") + 1;
        let content_end = script.find("</script>").expect("script closing tag");
        let bootstrap = serde_json::from_str(&script[content_start..content_end])
            .expect("typed bootstrap JSON");
        (headers, html, bootstrap)
    }

    fn bootstrap_data(value: &Value) -> &Value {
        value.get("data").expect("success data")
    }

    #[test]
    fn production_router_uses_an_explicit_builtin_inventory() {
        assert_eq!(
            PUBLIC_RUNTIME_ROUTE_INVENTORY,
            &[
                ExtensionRoute::new(RequestMethod::Get, PUBLIC_LIVENESS_PATH),
                ExtensionRoute::new(RequestMethod::Head, PUBLIC_LIVENESS_PATH),
                ExtensionRoute::new(RequestMethod::Get, PUBLIC_READINESS_PATH),
                ExtensionRoute::new(RequestMethod::Head, PUBLIC_READINESS_PATH),
                ExtensionRoute::new(RequestMethod::Get, PUBLIC_METRICS_PATH),
                ExtensionRoute::new(RequestMethod::Head, PUBLIC_METRICS_PATH),
                ExtensionRoute::new(RequestMethod::Get, PUBLIC_WATCH_PATH),
            ]
        );
        for get_route in PUBLIC_RUNTIME_ROUTE_INVENTORY.iter().filter(|route| {
            route.method() == &RequestMethod::Get && route.path() != PUBLIC_WATCH_PATH
        }) {
            assert!(PUBLIC_RUNTIME_ROUTE_INVENTORY.iter().any(|route| {
                route.method() == &RequestMethod::Head && route.path() == get_route.path()
            }));
        }
        assert!(NoPublicProductRoutes.route_inventory().is_empty());
    }

    #[tokio::test]
    async fn registered_builtin_method_surface_matches_the_inventory() {
        let (_temp, runtime, _store) = spawn_runtime(Arc::new(NoPublicProductRoutes)).await;
        let client = client();

        for route in PUBLIC_RUNTIME_ROUTE_INVENTORY {
            let request = match route.method() {
                RequestMethod::Get => client.get(request_url(&runtime, route.path())),
                RequestMethod::Head => client.head(request_url(&runtime, route.path())),
                method => panic!("unexpected built-in method: {method:?}"),
            };
            let response = request
                .header(HOST.as_str(), TEST_AUTHORITY)
                .send()
                .await
                .expect("inventory request");
            assert_ne!(response.status(), StatusCode::NOT_FOUND, "{route:?}");
            assert_ne!(
                response.status(),
                StatusCode::METHOD_NOT_ALLOWED,
                "{route:?}"
            );
            if route.method() == &RequestMethod::Head {
                assert!(response.bytes().await.expect("HEAD body").is_empty());
            }
        }

        let unsupported_watch_head = client
            .head(request_url(&runtime, PUBLIC_WATCH_PATH))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .send()
            .await
            .expect("unsupported WebSocket HEAD");
        assert_eq!(
            unsupported_watch_head.status(),
            StatusCode::METHOD_NOT_ALLOWED
        );

        for path in [
            PUBLIC_LIVENESS_PATH,
            PUBLIC_READINESS_PATH,
            PUBLIC_METRICS_PATH,
            PUBLIC_WATCH_PATH,
        ] {
            let response = client
                .post(request_url(&runtime, path))
                .header(HOST.as_str(), TEST_AUTHORITY)
                .send()
                .await
                .expect("unregistered method");
            assert_eq!(
                response.status(),
                StatusCode::METHOD_NOT_ALLOWED,
                "POST {path}"
            );
        }

        runtime.shutdown().await.expect("clean shutdown");
    }

    #[tokio::test]
    async fn public_host_uses_shared_product_routes_and_never_exposes_local_surfaces() {
        let temp = TempDir::new().expect("temporary directory");
        let store = Arc::new(Mutex::new(
            PublicOperationalStore::open_for_state_root(temp.path().join("state"))
                .expect("public operational state"),
        ));
        let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
        let routes = create_public_product_routes(store.clone(), open_runtime.clone())
            .expect("shared public product routes");
        assert!(Arc::ptr_eq(&routes.storage_access().store(), &store));
        assert_eq!(routes.route_inventory().len(), 8);

        let runtime = PublicRuntime::spawn_with_open_runtime(
            test_config(),
            store,
            Arc::new(routes),
            open_runtime.clone(),
        )
        .await
        .expect("public runtime");
        assert!(Arc::ptr_eq(&runtime.open_runtime(), &open_runtime));
        let client = client();

        let schema = client
            .get(request_url(
                &runtime,
                bcsp_application::PRODUCT_FILTER_SCHEMA_PATH,
            ))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .send()
            .await
            .expect("shared filter schema");
        assert_eq!(schema.status(), StatusCode::OK);
        let schema = schema.json::<Value>().await.expect("schema JSON");
        assert_eq!(schema["protocolVersion"], 1);
        assert_eq!(
            schema["data"]["fields"]
                .as_array()
                .expect("schema fields")
                .len(),
            bcsp_contracts::FILTER_FIELD_COUNT
        );

        let unauthenticated = client
            .post(request_url(
                &runtime,
                bcsp_application::PRODUCT_CATALOG_DISCOVERY_PATH,
            ))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .json(&serde_json::json!({
                "protocolVersion": 1,
                "payload": bcsp_contracts::CatalogDiscoveryRequestV1::new()
            }))
            .send()
            .await
            .expect("unauthenticated shared mutation");
        assert_eq!(unauthenticated.status(), StatusCode::FORBIDDEN);

        let (_, _, bootstrap) = document(&client, &runtime, "/", "en-US").await;
        let nonce = bootstrap_data(&bootstrap)["sessionNonce"]
            .as_str()
            .expect("document session nonce");
        let empty_catalog = client
            .post(request_url(
                &runtime,
                bcsp_application::PRODUCT_CATALOG_DISCOVERY_PATH,
            ))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .header(ORIGIN.as_str(), TEST_ORIGIN)
            .header(SESSION_HEADER, nonce)
            .json(&serde_json::json!({
                "protocolVersion": 1,
                "payload": bcsp_contracts::CatalogDiscoveryRequestV1::new()
            }))
            .send()
            .await
            .expect("authenticated shared mutation");
        assert_eq!(empty_catalog.status(), StatusCode::OK);
        let empty_catalog = empty_catalog
            .json::<Value>()
            .await
            .expect("empty Catalog response");
        assert_eq!(
            empty_catalog["data"]["status"]["availability"],
            "UNAVAILABLE_NO_FIRST_SUCCESS"
        );

        for response in [
            client
                .get(request_url(&runtime, "/api/v1/local/bootstrap"))
                .header(HOST.as_str(), TEST_AUTHORITY)
                .send()
                .await
                .expect("local read probe"),
            client
                .post(request_url(&runtime, "/api/v1/local/settings"))
                .header(HOST.as_str(), TEST_AUTHORITY)
                .send()
                .await
                .expect("local mutation probe"),
        ] {
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        runtime.shutdown().await.expect("clean shutdown");
    }

    #[tokio::test]
    async fn each_top_level_route_gets_fresh_typed_defaults_without_cookie_state() {
        let routes = Arc::new(CountingProductRoutes::default());
        let (_temp, runtime, _store) = spawn_runtime(routes.clone()).await;
        let client = client();

        let (first_headers, first_html, first) =
            document(&client, &runtime, "/", "zh-Hans;q=0.9,en;q=0.5").await;
        let (direct_headers, direct_html, direct) = document(
            &client,
            &runtime,
            "/course/TERM_2026_FALL/CAMPUS_A/01:198:111?filters=ignored",
            "en-US",
        )
        .await;
        let first = bootstrap_data(&first);
        let direct = bootstrap_data(&direct);

        assert_eq!(first["locale"], "zh-CN");
        assert_eq!(direct["locale"], "en-US");
        assert!(first_html.contains(r#"<html lang="zh-CN">"#));
        assert!(direct_html.contains(r#"<html lang="en-US">"#));
        assert_ne!(first["sessionNonce"], direct["sessionNonce"]);
        assert_eq!(
            first["filterSchema"]["fields"].as_array().unwrap().len(),
            22
        );
        assert!(first["currentFilters"].is_null());
        assert!(direct["currentFilters"].is_null());
        assert_eq!(first["selectedSections"], serde_json::json!([]));
        assert_eq!(first["activeWatches"], serde_json::json!([]));
        assert_eq!(first["currentPageAlerts"], serde_json::json!([]));
        assert_eq!(first["volumePercent"], 100);
        assert_eq!(first["watchPolicy"]["notificationMode"], "ONE_SHOT");
        assert_eq!(first["watchPolicy"]["maxAudible"], 3);
        assert_eq!(
            first["watchPolicy"]["continuousDuration"],
            serde_json::json!({"kind":"FINITE","seconds":600})
        );
        assert_eq!(first["refreshPolicy"]["catalogIntervalSeconds"], 600);
        assert_eq!(first["refreshPolicy"]["openGeneralIntervalSeconds"], 30);
        assert_eq!(first["refreshPolicy"]["openWatchedIntervalSeconds"], 10);
        assert_eq!(first["refreshPolicy"]["userConfigurable"], false);
        assert_eq!(first["service"]["ready"], false);
        for headers in [&first_headers, &direct_headers] {
            assert_eq!(
                headers
                    .get(CACHE_CONTROL)
                    .and_then(|value| value.to_str().ok()),
                Some("no-store")
            );
            assert!(!headers.contains_key("set-cookie"));
            assert!(!headers.contains_key("access-control-allow-origin"));
            assert!(!headers.contains_key("strict-transport-security"));
        }
        let nonce = first["sessionNonce"].as_str().expect("document nonce");
        assert!(
            first_headers
                .get(CONTENT_SECURITY_POLICY)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|policy| policy.contains(&format!("'nonce-{nonce}'")))
        );
        assert_eq!(routes.calls.load(Ordering::SeqCst), 0);
        assert_eq!(routes.origin_starts.load(Ordering::SeqCst), 0);

        runtime.shutdown().await.expect("clean shutdown");
    }

    #[tokio::test]
    async fn host_origin_and_document_session_guard_mutations_without_origin_amplification() {
        let routes = Arc::new(CountingProductRoutes::default());
        let (_temp, runtime, _store) = spawn_runtime(routes.clone()).await;
        let client = client();

        let wrong_host = client
            .get(request_url(&runtime, "/"))
            .header(HOST.as_str(), "wrong.example.test")
            .send()
            .await
            .expect("wrong Host response");
        assert_eq!(wrong_host.status(), StatusCode::MISDIRECTED_REQUEST);

        let (_, _, bootstrap) = document(&client, &runtime, "/", "en-US").await;
        let nonce = bootstrap_data(&bootstrap)["sessionNonce"]
            .as_str()
            .expect("document nonce")
            .to_owned();
        assert_eq!(routes.calls.load(Ordering::SeqCst), 0);

        let unauthenticated = client
            .post(request_url(&runtime, "/api/v1/query/courses"))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .body("{}")
            .send()
            .await
            .expect("unauthenticated mutation");
        assert_eq!(unauthenticated.status(), StatusCode::FORBIDDEN);
        let wrong_origin = client
            .post(request_url(&runtime, "/api/v1/query/courses"))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .header(ORIGIN.as_str(), "https://wrong.example.test")
            .header(SESSION_HEADER, &nonce)
            .body("{}")
            .send()
            .await
            .expect("wrong Origin mutation");
        assert_eq!(wrong_origin.status(), StatusCode::FORBIDDEN);
        let wrong_session = client
            .post(request_url(&runtime, "/api/v1/query/courses"))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .header(ORIGIN.as_str(), TEST_ORIGIN)
            .header(SESSION_HEADER, "00000000-0000-4000-8000-000000000001")
            .body("{}")
            .send()
            .await
            .expect("wrong session mutation");
        assert_eq!(wrong_session.status(), StatusCode::FORBIDDEN);
        assert_eq!(routes.calls.load(Ordering::SeqCst), 0);

        let accepted = client
            .post(request_url(&runtime, "/api/v1/query/courses"))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .header(ORIGIN.as_str(), TEST_ORIGIN)
            .header(SESSION_HEADER, &nonce)
            .body("{}")
            .send()
            .await
            .expect("authenticated query");
        assert_eq!(accepted.status(), StatusCode::OK);
        let read = client
            .get(request_url(&runtime, "/api/v1/query/courses"))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .send()
            .await
            .expect("shared-state read");
        assert_eq!(read.status(), StatusCode::OK);
        let unlisted_read = client
            .get(request_url(&runtime, "/api/v1/query/unlisted"))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .send()
            .await
            .expect("unlisted API read");
        assert_eq!(unlisted_read.status(), StatusCode::NOT_FOUND);
        let unlisted_mutation = client
            .post(request_url(&runtime, "/api/v1/query/unlisted"))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .send()
            .await
            .expect("unlisted API mutation");
        assert_eq!(unlisted_mutation.status(), StatusCode::NOT_FOUND);
        assert_eq!(routes.calls.load(Ordering::SeqCst), 2);
        assert_eq!(routes.origin_starts.load(Ordering::SeqCst), 0);
        assert_eq!(
            routes.paths.lock().expect("route paths").as_slice(),
            ["Post /api/v1/query/courses", "Get /api/v1/query/courses"]
        );

        runtime.shutdown().await.expect("clean shutdown");
    }

    #[tokio::test]
    async fn fresh_service_is_live_not_ready_and_metrics_are_fixed_safe_and_nonpersonal() {
        let routes = Arc::new(CountingProductRoutes::default());
        let (temp, runtime, _store) = spawn_runtime(routes).await;
        let client = client();
        let live = client
            .get(request_url(&runtime, "/health/live"))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .send()
            .await
            .expect("liveness");
        assert_eq!(live.status(), StatusCode::OK);
        assert_eq!(
            live.json::<Value>().await.expect("liveness JSON")["data"]["status"],
            "LIVE"
        );
        let ready = client
            .get(request_url(&runtime, "/health/ready"))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .send()
            .await
            .expect("readiness");
        assert_eq!(ready.status(), StatusCode::SERVICE_UNAVAILABLE);
        let ready = ready.json::<Value>().await.expect("readiness JSON");
        assert_eq!(ready["data"]["status"], "NOT_READY");
        assert_eq!(ready["data"]["ready"], false);
        assert_eq!(
            ready["data"]["reasons"],
            serde_json::json!(["CATALOG_UNAVAILABLE", "OPEN_UNAVAILABLE"])
        );

        runtime.scheduler_status().publish(PublicSchedulerSnapshot {
            maximum_lag_milliseconds: 987,
            origin_circuit_open: true,
            overloaded: true,
            in_flight: true,
        });
        let metrics = client
            .get(request_url(&runtime, "/metrics"))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .send()
            .await
            .expect("metrics")
            .text()
            .await
            .expect("metrics body");
        for expected in [
            "bcsp_service_live 1",
            "bcsp_service_ready 0",
            "bcsp_websocket_connections 0",
            "bcsp_scheduler_lag_milliseconds 987",
            "bcsp_scheduler_origin_circuit_open 1",
            "bcsp_scheduler_overloaded 1",
            "bcsp_open_attempted_rutgers_day 0",
            "bcsp_open_succeeded_rutgers_day 0",
            "bcsp_open_failed_rutgers_day 0",
            "bcsp_open_empty_rutgers_day 0",
            "bcsp_catalog_requested_interval_seconds 600",
            "bcsp_open_general_requested_interval_seconds 30",
            "bcsp_open_watched_requested_interval_seconds 10",
        ] {
            assert!(metrics.contains(expected), "missing metric: {expected}");
        }
        assert!(!metrics.contains(temp.path().to_string_lossy().as_ref()));
        assert!(!metrics.contains("rbcsp.sqlite"));
        assert!(!metrics.contains("sessionNonce"));

        runtime.shutdown().await.expect("clean shutdown");
    }

    #[tokio::test]
    async fn degraded_readiness_and_metrics_expose_only_aggregate_service_facts() {
        let temp = TempDir::new().expect("temporary directory");
        let store = Arc::new(Mutex::new(
            PublicOperationalStore::open_for_state_root(temp.path().join("state"))
                .expect("public operational state"),
        ));
        let watch =
            create_public_watch_socket(store, Arc::new(OpenRuntimeSnapshotRegistry::default()))
                .expect("public watch socket");
        let scheduler = Arc::new(InMemoryPublicSchedulerStatus::default());
        let snapshot = PublicServiceSnapshot {
            status: PublicStatusLevel::Degraded,
            ready: true,
            reasons: vec![
                PublicReadinessReason::OriginCircuitOpen,
                PublicReadinessReason::SchedulerOverloaded,
            ],
            catalog_target_count: 2,
            catalog_available_target_count: 2,
            open_available_target_count: 2,
            websocket_connection_count: 3,
            active_watch_count: 4,
            scheduler: PublicSchedulerSnapshot {
                maximum_lag_milliseconds: 1_234,
                origin_circuit_open: true,
                overloaded: true,
                in_flight: false,
            },
            rutgers_day: "2026-07-14".to_owned(),
            today_counts: PublicDayCounters {
                attempted: 9,
                succeeded: 6,
                failed: 3,
                empty: 1,
            },
        };
        let runtime = PublicRuntime::spawn_with_state(
            test_config(),
            Arc::new(NoPublicProductRoutes),
            Arc::new(StaticServiceState { snapshot }),
            watch,
            scheduler,
        )
        .await
        .expect("public runtime");
        let client = client();
        let ready = client
            .get(request_url(&runtime, "/health/ready"))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .send()
            .await
            .expect("degraded readiness");
        assert_eq!(ready.status(), StatusCode::OK);
        let ready = ready.json::<Value>().await.expect("readiness JSON");
        assert_eq!(ready["data"]["status"], "DEGRADED");
        assert_eq!(ready["data"]["todayCounts"]["attempted"], 9);
        assert_eq!(ready["data"]["todayCounts"]["succeeded"], 6);
        assert_eq!(ready["data"]["todayCounts"]["failed"], 3);
        assert_eq!(ready["data"]["todayCounts"]["empty"], 1);
        let metrics = client
            .get(request_url(&runtime, "/metrics"))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .send()
            .await
            .expect("degraded metrics")
            .text()
            .await
            .expect("metrics body");
        assert!(metrics.contains("bcsp_service_ready 1"));
        assert!(metrics.contains("bcsp_websocket_connections 3"));
        assert!(metrics.contains("bcsp_active_watches 4"));
        assert!(metrics.contains("bcsp_open_attempted_rutgers_day 9"));
        assert!(metrics.contains("bcsp_open_succeeded_rutgers_day 6"));
        assert!(metrics.contains("bcsp_open_failed_rutgers_day 3"));
        assert!(metrics.contains("bcsp_open_empty_rutgers_day 1"));
        assert!(!metrics.contains(temp.path().to_string_lossy().as_ref()));

        runtime.shutdown().await.expect("clean shutdown");
    }

    async fn websocket_handshake(
        address: std::net::SocketAddr,
        path: &str,
        origin: &str,
        protocol: Option<&str>,
    ) -> (String, TcpStream) {
        let path = path.to_owned();
        let origin = origin.to_owned();
        let protocol = protocol.map(str::to_owned);
        tokio::task::spawn_blocking(move || {
            let mut stream = TcpStream::connect(address).expect("WebSocket TCP connection");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("read timeout");
            let protocol = protocol
                .map(|value| format!("Sec-WebSocket-Protocol: {value}\r\n"))
                .unwrap_or_default();
            let request = format!(
                "GET {path} HTTP/1.1\r\nHost: {TEST_AUTHORITY}\r\nOrigin: {origin}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n{protocol}\r\n"
            );
            stream.write_all(request.as_bytes()).expect("write handshake");
            let mut response = Vec::new();
            let mut byte = [0_u8; 1];
            while !response.ends_with(b"\r\n\r\n") {
                let count = stream.read(&mut byte).expect("read handshake");
                if count == 0 {
                    break;
                }
                response.push(byte[0]);
                assert!(response.len() < 32 * 1024, "handshake header too large");
            }
            (
                String::from_utf8(response).expect("UTF-8 handshake response"),
                stream,
            )
        })
        .await
        .expect("handshake task")
    }

    #[tokio::test]
    async fn websocket_requires_origin_nonce_and_protocol_and_shutdown_closes_live_transport() {
        let routes = Arc::new(CountingProductRoutes::default());
        let (_temp, runtime, _store) = spawn_runtime(routes.clone()).await;
        let client = client();
        let (_, _, bootstrap) = document(&client, &runtime, "/section/direct", "en-US").await;
        let nonce = bootstrap_data(&bootstrap)["sessionNonce"]
            .as_str()
            .expect("document nonce")
            .to_owned();

        let (missing_protocol, _) = websocket_handshake(
            runtime.address(),
            &format!("/api/v1/watch?session={nonce}"),
            TEST_ORIGIN,
            None,
        )
        .await;
        assert!(missing_protocol.starts_with("HTTP/1.1 403"));
        let (wrong_protocol, _) = websocket_handshake(
            runtime.address(),
            &format!("/api/v1/watch?session={nonce}"),
            TEST_ORIGIN,
            Some("bcsp.v2"),
        )
        .await;
        assert!(wrong_protocol.starts_with("HTTP/1.1 403"));
        let (missing_nonce, _) = websocket_handshake(
            runtime.address(),
            "/api/v1/watch",
            TEST_ORIGIN,
            Some(PUBLIC_WS_SUBPROTOCOL),
        )
        .await;
        assert!(missing_nonce.starts_with("HTTP/1.1 403"));
        let (wrong_nonce, _) = websocket_handshake(
            runtime.address(),
            "/api/v1/watch?session=00000000-0000-4000-8000-000000000001",
            TEST_ORIGIN,
            Some(PUBLIC_WS_SUBPROTOCOL),
        )
        .await;
        assert!(wrong_nonce.starts_with("HTTP/1.1 403"));
        let (wrong_origin, _) = websocket_handshake(
            runtime.address(),
            &format!("/api/v1/watch?session={nonce}"),
            "https://wrong.example.test",
            Some(PUBLIC_WS_SUBPROTOCOL),
        )
        .await;
        assert!(wrong_origin.starts_with("HTTP/1.1 403"));

        let (accepted, _socket) = websocket_handshake(
            runtime.address(),
            &format!("/api/v1/watch?session={nonce}"),
            TEST_ORIGIN,
            Some(PUBLIC_WS_SUBPROTOCOL),
        )
        .await;
        assert!(accepted.starts_with("HTTP/1.1 101"));
        assert!(
            accepted
                .to_ascii_lowercase()
                .contains("sec-websocket-protocol: bcsp.v1")
        );
        let watch = runtime.watch_socket();
        for _ in 0..50 {
            if watch.connection_count() == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(watch.connection_count(), 1);
        assert_eq!(routes.origin_starts.load(Ordering::SeqCst), 0);

        tokio::time::timeout(Duration::from_secs(3), runtime.shutdown())
            .await
            .expect("graceful shutdown timeout")
            .expect("graceful shutdown");
        assert_eq!(watch.connection_count(), 0);
    }

    #[test]
    fn public_watch_demand_port_is_target_specific() {
        struct FixedDemand;
        impl crate::PublicWatchDemandSource for FixedDemand {
            fn connection_count(&self) -> u64 {
                3
            }

            fn total_active_watch_count(&self) -> u64 {
                7
            }

            fn active_watch_count(&self, target: &TermCampusKey) -> u64 {
                u64::from(target.campus().as_str() == "CAMPUS_A")
            }
        }
        let target_a = TermCampusKey::try_new("TERM_2026_FALL", "CAMPUS_A").expect("target A");
        let target_b = TermCampusKey::try_new("TERM_2026_FALL", "CAMPUS_B").expect("target B");
        let demand = FixedDemand;
        assert_eq!(demand.connection_count(), 3);
        assert_eq!(demand.total_active_watch_count(), 7);
        assert_eq!(demand.active_watch_count(&target_a), 1);
        assert_eq!(demand.active_watch_count(&target_b), 0);
    }
}
