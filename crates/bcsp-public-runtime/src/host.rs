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
use axum::http::header::RETRY_AFTER;
use axum::routing::{any, get, post};
use bcsp_application::{
    ExtensionRequest, ExtensionResponse, ExtensionRoute, FixedRefreshPolicyProvider,
    OfficialRefreshRuntime, OfficialRefreshRuntimeBuildError, OpenRuntimeSnapshotRegistry,
    RefreshPolicyError, RequestMethod, RouteExtension, SHARED_WATCH_SUBPROTOCOL,
    SharedProductStorage, SharedWatchSocket, TargetRefreshDemand, WebSocketExtension,
    serve_websocket, shared_websocket_upgrade,
};
use bcsp_contracts::{
    ActiveWatchTargetV1, ApiErrorBody, ApiErrorCode, ApiErrorDetail, ApiErrorEnvelope,
    FilterRequestV1, FilterSchemaV1, HttpSuccessEnvelope, SectionKey, SessionValidateRequestV1,
    SessionValidateResponseV1, SystemTraceIdSource, TraceIdSource, WatchAlertV1, WatchPolicyV1,
    filter_schema_v1,
};
use bcsp_open::OpenCounterAudience;
use bcsp_operational_storage::OperationalStorage;
use bcsp_public_operations::{PublicOperationalStore, PublicOperationsError};
use include_dir::{Dir, include_dir};
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
use crate::rate_limit::{IssuanceRateLimiter, RateDecision};
use crate::session::{
    DocumentSessionError, DocumentSessionRegistry, PublicLocale, ReserveWsError, ValidateOutcome,
    locale_for_tag, negotiate_locale,
};
use crate::status::{
    InMemoryPublicSchedulerStatus, PublicSchedulerStatusSource, PublicServiceInspector,
    PublicServiceSnapshot, PublicServiceStateSource,
};
use crate::watch::create_public_watch_socket;

const MAX_REQUEST_BODY_BYTES: usize = 1024 * 1024;
const SESSION_HEADER: &str = "x-bcsp-session";
pub const PUBLIC_WS_SUBPROTOCOL: &str = SHARED_WATCH_SUBPROTOCOL;
const WATCH_MAINTENANCE_INTERVAL: Duration = Duration::from_millis(250);
const SHUTDOWN_DRAIN_TIMEOUT: Duration = Duration::from_secs(10);
const DEFAULT_VOLUME_PERCENT: u8 = 100;
pub const PUBLIC_LIVENESS_PATH: &str = "/health/live";
pub const PUBLIC_READINESS_PATH: &str = "/health/ready";
pub const PUBLIC_METRICS_PATH: &str = "/metrics";
pub const PUBLIC_WATCH_PATH: &str = "/api/v1/watch";
pub const PUBLIC_SESSION_VALIDATE_PATH: &str = "/api/v1/session/validate";
/// Route-level body cap for the validate endpoint (design contract: 4 KB
/// under the 1 MB global cap; an oversized body is a malformed body -> 400).
const MAX_VALIDATE_BODY_BYTES: usize = 4 * 1024;
/// The reverse proxy (loopback-only bind guarantees one exists in front)
/// appends the real client address as the LAST entry of this header.
const FORWARDED_FOR_HEADER: &str = "x-forwarded-for";
pub static PUBLIC_RUNTIME_ROUTE_INVENTORY: &[ExtensionRoute] = &[
    ExtensionRoute::new(RequestMethod::Get, PUBLIC_LIVENESS_PATH),
    ExtensionRoute::new(RequestMethod::Head, PUBLIC_LIVENESS_PATH),
    ExtensionRoute::new(RequestMethod::Get, PUBLIC_READINESS_PATH),
    ExtensionRoute::new(RequestMethod::Head, PUBLIC_READINESS_PATH),
    ExtensionRoute::new(RequestMethod::Get, PUBLIC_METRICS_PATH),
    ExtensionRoute::new(RequestMethod::Head, PUBLIC_METRICS_PATH),
    ExtensionRoute::new(RequestMethod::Get, PUBLIC_WATCH_PATH),
    ExtensionRoute::new(RequestMethod::Post, PUBLIC_SESSION_VALIDATE_PATH),
];
static PUBLIC_WEB_ASSETS: Dir<'_> = include_dir!("$OUT_DIR/web-assets");
const PUBLIC_HTML: &str = "public.html";

#[derive(Clone)]
struct PublicHostState {
    config: Arc<PublicHostConfig>,
    sessions: Arc<DocumentSessionRegistry>,
    /// One shared bucket set for both anonymous issuance surfaces: document
    /// GETs and the validate endpoint (design: same policy, same bucket).
    issuance_limits: Arc<IssuanceRateLimiter>,
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
    #[cfg(test)]
    sessions: Arc<DocumentSessionRegistry>,
}

impl PublicRuntime {
    pub async fn spawn(
        config: PublicHostConfig,
        serving_storage: SharedProductStorage,
        product_routes: Arc<dyn RouteExtension>,
    ) -> Result<Self, PublicRuntimeError> {
        Self::spawn_with_open_runtime(
            config,
            serving_storage,
            product_routes,
            Arc::new(OpenRuntimeSnapshotRegistry::default()),
        )
        .await
    }

    pub async fn spawn_with_open_runtime(
        config: PublicHostConfig,
        serving_storage: SharedProductStorage,
        product_routes: Arc<dyn RouteExtension>,
        open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
    ) -> Result<Self, PublicRuntimeError> {
        let watch = create_public_watch_socket(serving_storage.clone(), open_runtime.clone())
            .map_err(|_| PublicRuntimeError::WatchInitialization)?;
        let scheduler = Arc::new(InMemoryPublicSchedulerStatus::default());
        let scheduler_source: Arc<dyn PublicSchedulerStatusSource> = scheduler.clone();
        let service: Arc<dyn PublicServiceStateSource> =
            Arc::new(PublicServiceInspector::with_system_clock(
                serving_storage,
                scheduler_source,
                watch.clone(),
            ));
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
        let sessions = Arc::new(DocumentSessionRegistry::default());
        let state = PublicHostState {
            config: Arc::new(config),
            sessions: sessions.clone(),
            issuance_limits: Arc::new(IssuanceRateLimiter::default()),
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
                let watch = maintenance_watch.clone();
                if tokio::task::spawn_blocking(move || watch.tick())
                    .await
                    .is_err()
                {
                    break;
                }
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
            #[cfg(test)]
            sessions,
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

    #[cfg(test)]
    fn document_session_count(&self) -> usize {
        self.sessions.len()
    }

    pub async fn shutdown(mut self) -> Result<(), PublicRuntimeError> {
        if let Some(refresh) = self.refresh.take() {
            refresh.shutdown().await;
        }
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.watch.seal_and_stop();
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
                    self.watch.seal_and_stop();
                    return Err(PublicRuntimeError::ShutdownTimeout);
                }
            }
        }
        self.watch.seal_and_stop();
        Ok(())
    }
}

impl Drop for PublicRuntime {
    fn drop(&mut self) {
        drop(self.refresh.take());
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.watch.seal_and_stop();
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
    let serving_storage = Arc::new(Mutex::new(
        OperationalStorage::open(store.database_path()).map_err(|source| {
            PublicRuntimeError::OperationalState(PublicOperationsError::OpenDatabase { source })
        })?,
    ));
    let store = Arc::new(Mutex::new(store));
    let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
    let target_refresh_demand = TargetRefreshDemand::default();
    let product_routes =
        create_public_product_routes(serving_storage.clone(), open_runtime.clone())
            .map_err(PublicRuntimeError::ProductComposition)?
            .with_target_refresh_demand(target_refresh_demand.clone());
    let service_status = product_routes.service_status_registry();
    let prepared_serving = product_routes.prepared_serving_registry();
    let mut runtime = PublicRuntime::spawn_with_open_runtime(
        config,
        serving_storage,
        Arc::new(product_routes),
        open_runtime.clone(),
    )
    .await?;
    let policy = FixedRefreshPolicyProvider::new(
        crate::fixed_public_refresh_policy().map_err(PublicRuntimeError::ProductComposition)?,
    );
    service_status
        .set_delegate(runtime.scheduler.clone())
        .map_err(|_| PublicRuntimeError::StatusComposition)?;
    let mut ids = SystemTraceIdSource;
    let refresh = OfficialRefreshRuntime::spawn_with_target_refresh_demand_and_prepared(
        crate::PublicProductStorageAccess::new(store),
        policy,
        ids.next_trace_id(),
        OpenCounterAudience::Public,
        runtime.watch.clone(),
        open_runtime,
        service_status,
        target_refresh_demand,
        prepared_serving,
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
        .filter(|route| route.method() != &RequestMethod::Head)
        .fold(Router::<PublicHostState>::new(), register_builtin_route)
        .fallback(any(handle_fallback))
        .with_state(state)
}

fn register_builtin_route(
    router: Router<PublicHostState>,
    route: &ExtensionRoute,
) -> Router<PublicHostState> {
    match (route.method(), route.path()) {
        (RequestMethod::Get, PUBLIC_LIVENESS_PATH) => {
            router.route(route.path(), get(handle_liveness))
        }
        (RequestMethod::Get, PUBLIC_READINESS_PATH) => {
            router.route(route.path(), get(handle_readiness))
        }
        (RequestMethod::Get, PUBLIC_METRICS_PATH) => router.route(route.path(), get(handle_metrics)),
        (RequestMethod::Get, PUBLIC_WATCH_PATH) => {
            router.route(route.path(), get(handle_watch_socket))
        }
        (RequestMethod::Post, PUBLIC_SESSION_VALIDATE_PATH) => {
            router.route(route.path(), post(handle_session_validate))
        }
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
    let snapshot = match load_service_snapshot(state.service).await {
        Ok(snapshot) => snapshot,
        Err(()) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiErrorCode::InternalError,
            );
        }
    };
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
    let snapshot = match load_service_snapshot(state.service).await {
        Ok(snapshot) => snapshot,
        Err(()) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiErrorCode::InternalError,
            );
        }
    };
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
    // Status-code discipline (acceptance checklist item 4): a wrong
    // authority is 421 everywhere, including the WebSocket handshake; every
    // other admission failure stays 403.
    if !valid_host(headers, &state) {
        return api_error_response(
            StatusCode::MISDIRECTED_REQUEST,
            ApiErrorCode::MalformedRequest,
        );
    }
    let session = strict_session_query(request.uri().query());
    let admitted = header_text(headers, ORIGIN).as_deref()
        == Some(state.config.external_origin())
        && requested_subprotocol(headers);
    let Some(nonce) = session.filter(|_| admitted) else {
        return api_error_response(StatusCode::FORBIDDEN, ApiErrorCode::MalformedRequest);
    };
    // Atomic reserve_ws lease (design section 2b): validity + per-session
    // connection cap + count increment + activity touch in one lock. The
    // lease rides the upgrade closure and lives for the whole transport
    // pump, so the session cannot be pruned or evicted mid-handshake, and
    // any exit path (including task abort) releases the slot.
    let lease = match state.sessions.reserve_ws(nonce) {
        Ok(lease) => lease,
        Err(ReserveWsError::UnknownSession) => {
            return api_error_response(StatusCode::FORBIDDEN, ApiErrorCode::MalformedRequest);
        }
        Err(ReserveWsError::ConnectionLimit) => {
            // The browser cannot see this status; the server-side record is
            // the observable signal for over-cap sessions.
            tracing::warn!(code = "PUBLIC_WS_SESSION_CONNECTION_LIMIT");
            return api_error_response(StatusCode::FORBIDDEN, ApiErrorCode::MalformedRequest);
        }
        Err(ReserveWsError::Unavailable) => {
            return api_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                ApiErrorCode::InternalError,
            );
        }
    };
    let mut source = SystemTraceIdSource;
    let connection_id = source.next_trace_id();
    let extension: Arc<dyn WebSocketExtension> = state.watch;
    shared_websocket_upgrade(upgrade, PUBLIC_WS_SUBPROTOCOL).on_upgrade(move |socket| async move {
        let _lease = lease;
        serve_websocket(socket, extension, connection_id).await;
    })
}

async fn handle_session_validate(
    State(state): State<PublicHostState>,
    request: Request,
) -> Response {
    let headers = request.headers().clone();
    if !valid_host(&headers, &state) {
        return api_error_response(
            StatusCode::MISDIRECTED_REQUEST,
            ApiErrorCode::MalformedRequest,
        );
    }
    if header_text(&headers, ORIGIN).as_deref() != Some(state.config.external_origin()) {
        return api_error_response(StatusCode::FORBIDDEN, ApiErrorCode::MalformedRequest);
    }
    // Renewal is anonymous issuance: it draws from the same per-client
    // bucket as document GETs, so neither surface bypasses the other.
    if let RateDecision::Deny {
        retry_after_seconds,
    } = state.issuance_limits.check(&client_rate_key(&headers))
    {
        return rate_limited_response(retry_after_seconds);
    }
    let body = match to_bytes(request.into_body(), MAX_VALIDATE_BODY_BYTES).await {
        Ok(body) => body,
        Err(_) => {
            // Over the 4 KB route cap: a malformed body per the frozen
            // contract (the status matrix has no 413 entry).
            return api_error_response(StatusCode::BAD_REQUEST, ApiErrorCode::MalformedRequest);
        }
    };
    let Ok(request_body) = serde_json::from_slice::<SessionValidateRequestV1>(&body) else {
        return api_error_response(StatusCode::BAD_REQUEST, ApiErrorCode::MalformedRequest);
    };
    // The manifest's session-nonce scalar is enforced at runtime too: a
    // non-canonical nonce is a malformed body, never a renewable one.
    if !bcsp_contracts::is_canonical_session_nonce(&request_body.nonce) {
        return api_error_response(StatusCode::BAD_REQUEST, ApiErrorCode::MalformedRequest);
    }
    // Locale precedence mirrors document issuance: request body first,
    // Accept-Language fallback (an unknown body tag falls through too).
    let locale = request_body
        .locale
        .as_deref()
        .and_then(locale_for_tag)
        .unwrap_or_else(|| negotiate_locale(header_text(&headers, ACCEPT_LANGUAGE).as_deref()));
    match state
        .sessions
        .validate_or_renew(&request_body.nonce, locale)
    {
        Ok(ValidateOutcome::Valid) => json_response(
            StatusCode::OK,
            &HttpSuccessEnvelope::new(SessionValidateResponseV1::valid()),
        ),
        Ok(ValidateOutcome::Renewed(nonce)) => {
            let Some(nonce) =
                bcsp_contracts::CanonicalSessionNonce::try_new(nonce.as_str().to_owned())
            else {
                // Unreachable: the registry only generates canonical nonces.
                return api_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ApiErrorCode::InternalError,
                );
            };
            json_response(
                StatusCode::OK,
                &HttpSuccessEnvelope::new(SessionValidateResponseV1::renewed(nonce)),
            )
        }
        Err(DocumentSessionError::CapacityExhausted) => api_error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::InternalError,
        ),
        Err(DocumentSessionError::Unavailable) => api_error_response(
            StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::InternalError,
        ),
    }
}

/// Per-client key for the shared issuance buckets. The public listener is
/// loopback-only behind the first-hop reverse proxy (Caddy overwrites
/// client-supplied `X-Forwarded-*` values), so the LAST `X-Forwarded-For`
/// entry is the true client address; anything forged earlier in the list is
/// ignored. If a CDN or additional proxy hop is ever introduced, this rule
/// must be re-frozen and re-tested against the real chain. Without the
/// header (direct local access) every caller shares one bucket.
///
/// IPv6 clients are aggregated to their /64 prefix -- a single routed /64
/// must not multiply its allowance by rotating interface identifiers.
/// IPv4-mapped IPv6 addresses key as their embedded IPv4 address.
fn client_rate_key(headers: &HeaderMap) -> String {
    let address = header_text_name(headers, FORWARDED_FOR_HEADER)
        .as_deref()
        .and_then(|value| value.rsplit(',').next())
        .map(str::trim)
        .and_then(|candidate| candidate.parse::<std::net::IpAddr>().ok());
    match address {
        Some(std::net::IpAddr::V4(address)) => address.to_string(),
        Some(std::net::IpAddr::V6(address)) => match address.to_ipv4_mapped() {
            Some(mapped) => mapped.to_string(),
            None => {
                let segments = address.segments();
                format!(
                    "{:x}:{:x}:{:x}:{:x}::/64",
                    segments[0], segments[1], segments[2], segments[3],
                )
            }
        },
        None => "direct".to_owned(),
    }
}

fn rate_limited_response(retry_after_seconds: u32) -> Response {
    let mut source = SystemTraceIdSource;
    let envelope = ApiErrorEnvelope::new(
        ApiErrorBody::new(ApiErrorCode::RateLimited, source.next_trace_id()).with_details(vec![
            ApiErrorDetail::RetryAfterSeconds {
                seconds: retry_after_seconds,
            },
        ]),
    );
    let mut response = json_response(StatusCode::TOO_MANY_REQUESTS, &envelope);
    if let Ok(value) = HeaderValue::from_str(&retry_after_seconds.to_string()) {
        response.headers_mut().insert(RETRY_AFTER, value);
    }
    response
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
    if method == RequestMethod::Get {
        if let Some(asset_path) = public_asset_path(&path) {
            return public_asset_response(asset_path);
        }
        if is_public_document_path(&path) {
            // Document GETs issue a fresh session nonce, so they draw from
            // the same per-client buckets as the validate endpoint.
            if let RateDecision::Deny {
                retry_after_seconds,
            } = state
                .issuance_limits
                .check(&client_rate_key(request.headers()))
            {
                return rate_limited_response(retry_after_seconds);
            }
            return document_response(&state, request.headers()).await;
        }
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
    let scoped_status = method == RequestMethod::Get
        && path == bcsp_application::PRODUCT_SERVICE_STATUS_PATH
        && request.uri().query().is_some_and(|query| {
            query
                .split('&')
                .any(|field| field.starts_with("activeTerm=") || field.starts_with("activeCampus="))
        });
    if scoped_status && !authenticated_session(request.headers(), &state) {
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
    let routes = state.product_routes;
    match tokio::task::spawn_blocking(move || {
        routes.handle(ExtensionRequest::new(method, path, query, body))
    })
    .await
    {
        Ok(response) => extension_response(response),
        Err(_) => api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiErrorCode::InternalError,
        ),
    }
}

async fn load_service_snapshot(
    service: Arc<dyn PublicServiceStateSource>,
) -> Result<PublicServiceSnapshot, ()> {
    tokio::task::spawn_blocking(move || service.snapshot())
        .await
        .map_err(|_| ())
}

async fn document_response(state: &PublicHostState, headers: &HeaderMap) -> Response {
    let locale = negotiate_locale(header_text(headers, ACCEPT_LANGUAGE).as_deref());
    let service = state.service.clone();
    let (service_snapshot, current_filters) = match tokio::task::spawn_blocking(move || {
        (service.snapshot(), service.default_current_filters())
    })
    .await
    {
        Ok(inspection) => inspection,
        Err(_) => {
            return api_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiErrorCode::InternalError,
            );
        }
    };
    let nonce = match state.sessions.issue(locale) {
        Ok(nonce) => nonce,
        Err(DocumentSessionError::Unavailable | DocumentSessionError::CapacityExhausted) => {
            // Capacity exhaustion (every session leased) is a 503 by the
            // frozen status contract -- never a silent eviction.
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
        current_filters,
        selected_sections: Vec::new(),
        active_watches: Vec::new(),
        current_page_alerts: Vec::new(),
        volume_percent: DEFAULT_VOLUME_PERCENT,
        watch_policy: WatchPolicyV1::default(),
        refresh_policy: PublicRefreshPolicyView::default(),
        service: service_snapshot,
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
    let Some(template) = PUBLIC_WEB_ASSETS
        .get_file(PUBLIC_HTML)
        .and_then(|file| file.contents_utf8())
    else {
        return api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiErrorCode::InternalError,
        );
    };
    let Some(html) =
        render_public_document(template, locale.html_lang(), nonce.as_str(), &bootstrap)
    else {
        return api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiErrorCode::InternalError,
        );
    };
    html_response(StatusCode::OK, html, nonce.as_str())
}

fn is_public_document_path(path: &str) -> bool {
    if matches!(path, "/" | "/sections" | "/watch") {
        return true;
    }
    let Some(rest) = path.strip_prefix("/sections/") else {
        return false;
    };
    let mut segments = rest.split('/');
    let (Some(term), Some(campus), Some(index), None) = (
        segments.next(),
        segments.next(),
        segments.next(),
        segments.next(),
    ) else {
        return false;
    };
    SectionKey::try_new(term, campus, index).is_ok()
}

fn public_asset_path(path: &str) -> Option<&str> {
    let asset = path.strip_prefix("/assets/")?;
    if asset.is_empty() {
        return None;
    }
    let relative = path.strip_prefix('/')?;
    safe_embedded_path(relative).then_some(relative)
}

fn safe_embedded_path(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', '%', '\0'])
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && !matches!(segment, "." | ".."))
}

fn public_asset_response(path: &str) -> Response {
    let Some(file) = PUBLIC_WEB_ASSETS.get_file(path) else {
        return extension_response(ExtensionResponse::not_found());
    };
    secured_response(
        StatusCode::OK,
        public_asset_content_type(path),
        file.contents().to_vec(),
        None,
    )
}

fn public_asset_content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("js") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("avif") => "image/avif",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ttf") => "font/ttf",
        Some("mp3") => "audio/mpeg",
        Some("ogg") => "audio/ogg",
        Some("wav") => "audio/wav",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}

fn render_public_document(
    template: &str,
    html_lang: &str,
    nonce: &str,
    bootstrap: &str,
) -> Option<String> {
    if template.contains("id=\"bcsp-bootstrap\"") {
        return None;
    }
    let mut html = replace_html_language(template, html_lang)?;
    html = html
        .replace("=\"./assets/", "=\"/assets/")
        .replace("='./assets/", "='/assets/");
    let body_end = html.rfind("</body>")?;
    let bootstrap_script = format!(
        "    <script id=\"bcsp-bootstrap\" type=\"application/json\" nonce=\"{nonce}\">{bootstrap}</script>\n"
    );
    html.insert_str(body_end, &bootstrap_script);
    (html.matches("id=\"bcsp-bootstrap\"").count() == 1).then_some(html)
}

fn replace_html_language(template: &str, html_lang: &str) -> Option<String> {
    let html_start = template.find("<html")?;
    let tag_end_offset = template[html_start..].find('>')?;
    let tag_end = html_start + tag_end_offset;
    let opening_tag = &template[html_start..=tag_end];
    let mut output = template.to_owned();
    if let Some(lang_offset) = opening_tag.find(" lang=\"") {
        let value_start = html_start + lang_offset + " lang=\"".len();
        let value_end = value_start + template[value_start..].find('"')?;
        output.replace_range(value_start..value_end, html_lang);
    } else {
        output.insert_str(tag_end, &format!(" lang=\"{html_lang}\""));
    }
    Some(output)
}

fn authenticated_mutation(headers: &HeaderMap, state: &PublicHostState) -> bool {
    if header_text(headers, ORIGIN).as_deref() != Some(state.config.external_origin()) {
        return false;
    }
    authenticated_session(headers, state)
}

fn authenticated_session(headers: &HeaderMap, state: &PublicHostState) -> bool {
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
                "default-src 'self'; base-uri 'none'; frame-ancestors 'none'; object-src 'none'; connect-src 'self'; img-src 'self' data:; font-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self' 'nonce-{nonce}'; worker-src 'none'; form-action 'none'"
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
    #[error("public service status could not be composed")]
    StatusComposition,
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
            Self::StatusComposition => "PUBLIC_STATUS_COMPOSITION_FAILED",
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
        PublicWatchDemandSource, SharedPublicOperationalStore,
    };

    const TEST_ORIGIN: &str = "https://planner.example.test";
    const TEST_AUTHORITY: &str = "planner.example.test";
    static TEST_PRODUCT_ROUTE_INVENTORY: &[ExtensionRoute] = &[
        ExtensionRoute::new(RequestMethod::Get, "/api/v1/service/status"),
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
        let store = PublicOperationalStore::open_for_state_root(temp.path().join("state"))
            .expect("public operational state");
        let serving_storage = Arc::new(Mutex::new(
            OperationalStorage::open(store.database_path()).expect("public serving storage"),
        ));
        let store = Arc::new(Mutex::new(store));
        let runtime = PublicRuntime::spawn(test_config(), serving_storage, routes)
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

    fn module_script_path(html: &str) -> &str {
        let script_start = html.find("<script type=\"module\"").expect("module script");
        let script = &html[script_start..];
        let tag_end = script.find('>').expect("module script end");
        let opening_tag = &script[..tag_end];
        let source_start =
            opening_tag.find("src=\"").expect("module script source") + "src=\"".len();
        let source_end = source_start
            + opening_tag[source_start..]
                .find('"')
                .expect("module script source end");
        &opening_tag[source_start..source_end]
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
                ExtensionRoute::new(RequestMethod::Post, PUBLIC_SESSION_VALIDATE_PATH),
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

    #[test]
    fn embedded_public_asset_mime_types_cover_the_frontend_runtime_allowlist() {
        for (path, expected) in [
            ("assets/app.avif", "image/avif"),
            ("assets/app.css", "text/css; charset=utf-8"),
            ("assets/app.gif", "image/gif"),
            ("assets/app.ico", "image/x-icon"),
            ("assets/app.jpeg", "image/jpeg"),
            ("assets/app.jpg", "image/jpeg"),
            ("assets/app.js", "text/javascript; charset=utf-8"),
            ("assets/app.json", "application/json; charset=utf-8"),
            ("assets/app.mp3", "audio/mpeg"),
            ("assets/app.ogg", "audio/ogg"),
            ("assets/app.png", "image/png"),
            ("assets/app.svg", "image/svg+xml"),
            ("assets/app.ttf", "font/ttf"),
            ("assets/app.wasm", "application/wasm"),
            ("assets/app.wav", "audio/wav"),
            ("assets/app.webp", "image/webp"),
            ("assets/app.woff", "font/woff"),
            ("assets/app.woff2", "font/woff2"),
        ] {
            assert_eq!(public_asset_content_type(path), expected, "{path}");
        }
    }

    #[tokio::test]
    async fn registered_builtin_method_surface_matches_the_inventory() {
        let (_temp, runtime, _store) = spawn_runtime(Arc::new(NoPublicProductRoutes)).await;
        let client = client();

        for route in PUBLIC_RUNTIME_ROUTE_INVENTORY {
            let request = match route.method() {
                RequestMethod::Get => client.get(request_url(&runtime, route.path())),
                RequestMethod::Head => client.head(request_url(&runtime, route.path())),
                RequestMethod::Post => client.post(request_url(&runtime, route.path())),
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
        let store = PublicOperationalStore::open_for_state_root(temp.path().join("state"))
            .expect("public operational state");
        let serving_storage = Arc::new(Mutex::new(
            OperationalStorage::open(store.database_path()).expect("public serving storage"),
        ));
        let store = Arc::new(Mutex::new(store));
        {
            let _refresh_writer = store.lock().expect("refresh writer lock");
            assert!(serving_storage.try_lock().is_ok());
        }
        let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
        let routes = create_public_product_routes(serving_storage.clone(), open_runtime.clone())
            .expect("shared public product routes");
        assert!(Arc::ptr_eq(routes.storage_access(), &serving_storage));
        assert_eq!(
            routes.route_inventory().len(),
            bcsp_application::SHARED_PRODUCT_ROUTE_INVENTORY.len()
        );

        let runtime = PublicRuntime::spawn_with_open_runtime(
            test_config(),
            serving_storage,
            Arc::new(routes),
            open_runtime.clone(),
        )
        .await
        .expect("public runtime");
        assert!(Arc::ptr_eq(&runtime.open_runtime(), &open_runtime));
        let client = client();

        let writer_store = store.clone();
        let (writer_held, writer_held_rx) = std::sync::mpsc::channel();
        let (release_writer, release_writer_rx) = std::sync::mpsc::channel();
        let writer_thread = std::thread::spawn(move || {
            let _writer = writer_store.lock().expect("refresh writer lock");
            writer_held.send(()).expect("announce writer lock");
            release_writer_rx.recv().expect("release writer lock");
        });
        writer_held_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("writer lock held");
        let (_, _, isolated_bootstrap) = tokio::time::timeout(
            Duration::from_secs(2),
            document(&client, &runtime, "/sections", "en-US"),
        )
        .await
        .expect("public document must not wait for the refresh writer");
        assert!(
            bootstrap_data(&isolated_bootstrap)["sessionNonce"]
                .as_str()
                .is_some()
        );
        let readiness = tokio::time::timeout(Duration::from_secs(2), async {
            client
                .get(request_url(&runtime, PUBLIC_READINESS_PATH))
                .header(HOST.as_str(), TEST_AUTHORITY)
                .send()
                .await
        })
        .await
        .expect("public readiness must not wait for the refresh writer")
        .expect("readiness while refresh writer is locked");
        assert_eq!(readiness.status(), StatusCode::SERVICE_UNAVAILABLE);
        release_writer.send(()).expect("release writer");
        writer_thread.join().expect("writer thread");

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

        let service_status = client
            .get(request_url(
                &runtime,
                bcsp_application::PRODUCT_SERVICE_STATUS_PATH,
            ))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .send()
            .await
            .expect("shared service status");
        assert_eq!(service_status.status(), StatusCode::OK);
        let service_status = service_status
            .json::<Value>()
            .await
            .expect("service status JSON");
        assert_eq!(service_status["protocolVersion"], 1);
        assert_eq!(service_status["data"]["contractVersion"], 2);
        assert_eq!(service_status["data"]["runtime"], "PUBLIC");

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
            "/sections/TERM_2026_FALL/CAMPUS_A/12345?filters=ignored",
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
            bcsp_contracts::FILTER_FIELD_COUNT
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
        assert!(
            first_headers
                .get(CONTENT_SECURITY_POLICY)
                .and_then(|value| value.to_str().ok())
                .is_some_and(|policy| policy.contains("style-src 'self' 'unsafe-inline'"))
        );
        assert_eq!(routes.calls.load(Ordering::SeqCst), 0);
        assert_eq!(routes.origin_starts.load(Ordering::SeqCst), 0);

        runtime.shutdown().await.expect("clean shutdown");
    }

    #[tokio::test]
    async fn embedded_vite_ui_serves_only_runtime_assets_without_static_sessions_or_local_surface()
    {
        let routes = Arc::new(CountingProductRoutes::default());
        let (_temp, runtime, _store) = spawn_runtime(routes.clone()).await;
        let client = client();
        assert_eq!(runtime.document_session_count(), 0);

        let (first_headers, first_html, first_bootstrap) =
            document(&client, &runtime, "/", "en-US").await;
        assert_eq!(runtime.document_session_count(), 1);
        assert_eq!(first_html.matches("id=\"bcsp-bootstrap\"").count(), 1);
        assert!(first_html.contains("<html lang=\"en-US\">"));
        let first_nonce = bootstrap_data(&first_bootstrap)["sessionNonce"]
            .as_str()
            .expect("first document nonce");
        assert!(first_html.contains(&format!("nonce=\"{first_nonce}\"")));
        let content_security_policy = first_headers
            .get(CONTENT_SECURITY_POLICY)
            .and_then(|value| value.to_str().ok())
            .expect("document CSP");
        assert!(
            content_security_policy.contains(&format!("script-src 'self' 'nonce-{first_nonce}'"))
        );
        assert!(content_security_policy.contains("style-src 'self' 'unsafe-inline'"));

        let module_path = module_script_path(&first_html).to_owned();
        assert!(module_path.starts_with("/assets/"));
        assert!(module_path.ends_with(".js"));
        assert!(!module_path.contains("./assets/"));
        let expected_module = PUBLIC_WEB_ASSETS
            .get_file(module_path.trim_start_matches('/'))
            .expect("embedded Vite module")
            .contents();
        let module = client
            .get(request_url(&runtime, &module_path))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .send()
            .await
            .expect("Vite module response");
        assert_eq!(module.status(), StatusCode::OK);
        assert_eq!(
            module
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok()),
            Some("text/javascript; charset=utf-8")
        );
        let module_bytes = module.bytes().await.expect("Vite module bytes");
        assert_eq!(module_bytes.as_ref(), expected_module);
        assert!(!module_bytes.is_empty());
        assert!(
            !module_bytes
                .windows(b"sourceMappingURL=".len())
                .any(|window| window == b"sourceMappingURL=")
        );
        assert_eq!(runtime.document_session_count(), 1);

        let (_, second_html, second_bootstrap) = document(
            &client,
            &runtime,
            "/sections/TERM_2026_FALL/CAMPUS_A/12345",
            "zh-Hans",
        )
        .await;
        assert_eq!(runtime.document_session_count(), 2);
        assert!(second_html.contains("<html lang=\"zh-CN\">"));
        assert_eq!(module_script_path(&second_html), module_path);
        assert_ne!(
            bootstrap_data(&first_bootstrap)["sessionNonce"],
            bootstrap_data(&second_bootstrap)["sessionNonce"]
        );
        assert_eq!(routes.calls.load(Ordering::SeqCst), 0);
        assert_eq!(routes.origin_starts.load(Ordering::SeqCst), 0);

        runtime.shutdown().await.expect("clean shutdown");
    }

    #[tokio::test]
    async fn document_and_static_allowlists_reject_unknown_traversal_metadata_and_source_maps() {
        let (_temp, runtime, _store) = spawn_runtime(Arc::new(NoPublicProductRoutes)).await;
        let client = client();

        for path in [
            "/",
            "/sections",
            "/watch",
            "/sections/TERM_2026_FALL/CAMPUS_A/12345",
        ] {
            let response = client
                .get(request_url(&runtime, path))
                .header(HOST.as_str(), TEST_AUTHORITY)
                .send()
                .await
                .expect("allowed document route");
            assert_eq!(response.status(), StatusCode::OK, "{path}");
        }
        assert_eq!(runtime.document_session_count(), 4);

        for path in [
            "/unknown",
            "/sections/term/CAMPUS_A/12345",
            "/sections/TERM_2026_FALL/CAMPUS-A/12345",
            "/sections/TERM_2026_FALL/CAMPUS_A/1234",
            "/sections/TERM_2026_FALL/CAMPUS_A/12345/extra",
            "/sections/TERM_2026_FALL/CAMPUS_A/%2e%2e",
            "/assets/%2e%2e/public.html",
            "/assets/missing.js",
            "/assets/missing.js.map",
            "/asset-manifest.json",
            "/capability-manifest.json",
            "/module-manifest.json",
        ] {
            let response = client
                .get(request_url(&runtime, path))
                .header(HOST.as_str(), TEST_AUTHORITY)
                .send()
                .await
                .expect("rejected public path");
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        }
        assert_eq!(runtime.document_session_count(), 4);

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
        let unauthenticated_scope = client
            .get(request_url(
                &runtime,
                "/api/v1/service/status?activeTerm=72026&activeCampus=NB",
            ))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .send()
            .await
            .expect("unauthenticated scoped status");
        assert_eq!(unauthenticated_scope.status(), StatusCode::FORBIDDEN);
        let wrong_scope_session = client
            .get(request_url(
                &runtime,
                "/api/v1/service/status?activeTerm=72026&activeCampus=NB",
            ))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .header(SESSION_HEADER, "00000000-0000-4000-8000-000000000001")
            .send()
            .await
            .expect("wrong scoped-status session");
        assert_eq!(wrong_scope_session.status(), StatusCode::FORBIDDEN);
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
        let scoped_status = client
            .get(request_url(
                &runtime,
                "/api/v1/service/status?activeTerm=72026&activeCampus=NB",
            ))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .header(SESSION_HEADER, &nonce)
            .send()
            .await
            .expect("authenticated scoped status");
        assert_eq!(scoped_status.status(), StatusCode::OK);
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
        assert_eq!(routes.calls.load(Ordering::SeqCst), 3);
        assert_eq!(routes.origin_starts.load(Ordering::SeqCst), 0);
        assert_eq!(
            routes.paths.lock().expect("route paths").as_slice(),
            [
                "Post /api/v1/query/courses",
                "Get /api/v1/query/courses",
                "Get /api/v1/service/status",
            ]
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
        let store = PublicOperationalStore::open_for_state_root(temp.path().join("state"))
            .expect("public operational state");
        let serving_storage = Arc::new(Mutex::new(
            OperationalStorage::open(store.database_path()).expect("public serving storage"),
        ));
        let watch = create_public_watch_socket(
            serving_storage,
            Arc::new(OpenRuntimeSnapshotRegistry::default()),
        )
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
        websocket_handshake_with_host(address, path, TEST_AUTHORITY, origin, protocol).await
    }

    async fn websocket_handshake_with_host(
        address: std::net::SocketAddr,
        path: &str,
        host: &str,
        origin: &str,
        protocol: Option<&str>,
    ) -> (String, TcpStream) {
        let path = path.to_owned();
        let origin = origin.to_owned();
        let host = host.to_owned();
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
                "GET {path} HTTP/1.1\r\nHost: {host}\r\nOrigin: {origin}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n{protocol}\r\n"
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
        let (_, _, bootstrap) = document(
            &client,
            &runtime,
            "/sections/TERM_2026_FALL/CAMPUS_A/12345",
            "en-US",
        )
        .await;
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

    #[test]
    fn client_rate_keys_aggregate_networks_not_addresses() {
        let key = |forwarded: Option<&str>| {
            let mut headers = HeaderMap::new();
            if let Some(value) = forwarded {
                headers.insert(
                    FORWARDED_FOR_HEADER,
                    HeaderValue::from_str(value).expect("header value"),
                );
            }
            client_rate_key(&headers)
        };

        // IPv4 keys by full address; the LAST entry wins (forged prefixes
        // appended by the client are ignored -- Caddy appends the true peer).
        assert_eq!(key(Some("198.51.100.7")), "198.51.100.7");
        assert_eq!(key(Some("1.2.3.4, 198.51.100.7")), "198.51.100.7");

        // IPv6 aggregates to the routed /64: two interface identifiers in
        // one /64 share a bucket; the adjacent /64 does not.
        assert_eq!(
            key(Some("2001:db8:aaaa:1:1111:2222:3333:4444")),
            key(Some("2001:db8:aaaa:1:dead:beef:cafe:1234")),
            "same /64 must share one key",
        );
        assert_ne!(
            key(Some("2001:db8:aaaa:1:1111:2222:3333:4444")),
            key(Some("2001:db8:aaaa:2:1111:2222:3333:4444")),
            "the adjacent /64 is a different key",
        );

        // IPv4-mapped IPv6 keys as its embedded IPv4 address.
        assert_eq!(key(Some("::ffff:198.51.100.7")), "198.51.100.7");

        // Absent or unparsable headers share the single direct bucket.
        assert_eq!(key(None), "direct");
        assert_eq!(key(Some("not-an-address")), "direct");
    }

    async fn post_validate(
        client: &Client,
        runtime: &PublicRuntime,
        host: &str,
        origin: Option<&str>,
        body: &str,
    ) -> reqwest::Response {
        let mut request = client
            .post(request_url(runtime, PUBLIC_SESSION_VALIDATE_PATH))
            .header(HOST.as_str(), host)
            .header(CONTENT_TYPE.as_str(), "application/json")
            .body(body.to_owned());
        if let Some(origin) = origin {
            request = request.header(ORIGIN.as_str(), origin);
        }
        request.send().await.expect("validate response")
    }

    #[tokio::test]
    async fn session_validate_enforces_the_frozen_status_matrix() {
        let (_temp, runtime, _store) = spawn_runtime(Arc::new(NoPublicProductRoutes)).await;
        let client = client();
        let (_, _, bootstrap) = document(&client, &runtime, "/", "en-US").await;
        let nonce = bootstrap_data(&bootstrap)["sessionNonce"]
            .as_str()
            .expect("document nonce")
            .to_owned();
        let valid_body = format!(r#"{{"nonce":"{nonce}"}}"#);

        // 421: authority mismatch outranks everything else.
        let response = post_validate(
            &client,
            &runtime,
            "evil.example.test",
            Some(TEST_ORIGIN),
            &valid_body,
        )
        .await;
        assert_eq!(response.status(), StatusCode::MISDIRECTED_REQUEST);

        // 403: wrong or missing Origin.
        let response = post_validate(
            &client,
            &runtime,
            TEST_AUTHORITY,
            Some("https://evil.example.test"),
            &valid_body,
        )
        .await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = post_validate(&client, &runtime, TEST_AUTHORITY, None, &valid_body).await;
        assert_eq!(response.status(), StatusCode::FORBIDDEN);

        // 400: malformed JSON, unknown fields, non-canonical nonce, and an
        // over-cap body.
        for body in [
            "not json",
            r#"{"nonce":"x","extra":true}"#,
            r#"{}"#,
            r#"{"nonce":"not-a-canonical-nonce"}"#,
            r#"{"nonce":"00000000-0000-1000-8000-000000000001"}"#,
        ] {
            let response =
                post_validate(&client, &runtime, TEST_AUTHORITY, Some(TEST_ORIGIN), body).await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{body}");
        }
        let oversized = format!(
            r#"{{"nonce":"{nonce}","locale":"{}"}}"#,
            "x".repeat(5 * 1024),
        );
        let response =
            post_validate(&client, &runtime, TEST_AUTHORITY, Some(TEST_ORIGIN), &oversized).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // 200 {valid:true}: the registered nonce validates and is touched.
        let response =
            post_validate(&client, &runtime, TEST_AUTHORITY, Some(TEST_ORIGIN), &valid_body).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = response.json().await.expect("valid body");
        assert_eq!(body["data"]["valid"], Value::Bool(true));
        assert!(body["data"].get("renewed").is_none());

        // 200 {renewed}: an unregistered (well-formed) nonce is atomically
        // replaced, and the replacement immediately validates.
        let stale = r#"{"nonce":"00000000-0000-4000-8000-00000000dead"}"#;
        let response =
            post_validate(&client, &runtime, TEST_AUTHORITY, Some(TEST_ORIGIN), stale).await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = response.json().await.expect("renewed body");
        let renewed = body["data"]["renewed"].as_str().expect("renewed nonce");
        let renewed_body = format!(r#"{{"nonce":"{renewed}"}}"#);
        let response = post_validate(
            &client,
            &runtime,
            TEST_AUTHORITY,
            Some(TEST_ORIGIN),
            &renewed_body,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = response.json().await.expect("renewed validates");
        assert_eq!(body["data"]["valid"], Value::Bool(true));

        runtime.shutdown().await.expect("clean shutdown");
    }

    #[tokio::test]
    async fn validate_returns_503_when_every_session_is_leased() {
        let (_temp, runtime, _store) = spawn_runtime(Arc::new(NoPublicProductRoutes)).await;
        let client = client();

        // Pin the ENTIRE production-capacity registry: every session holds a
        // live WebSocket lease, so renewal admission has nothing to evict.
        let mut leases = Vec::new();
        loop {
            match runtime.sessions.issue(PublicLocale::EnUs) {
                Ok(nonce) => {
                    leases.push(
                        runtime
                            .sessions
                            .reserve_ws(nonce.as_str())
                            .expect("lease the fresh session"),
                    );
                }
                Err(DocumentSessionError::CapacityExhausted) => break,
                Err(DocumentSessionError::Unavailable) => panic!("registry unavailable"),
            }
            assert!(leases.len() <= 5_000, "capacity must exhaust");
        }

        // The frozen contract's 503 branch over real HTTP.
        let stale = r#"{"nonce":"00000000-0000-4000-8000-00000000dead"}"#;
        let response =
            post_validate(&client, &runtime, TEST_AUTHORITY, Some(TEST_ORIGIN), stale).await;
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        // Releasing one lease restores renewal admission.
        leases.pop();
        let response =
            post_validate(&client, &runtime, TEST_AUTHORITY, Some(TEST_ORIGIN), stale).await;
        assert_eq!(response.status(), StatusCode::OK);

        runtime.shutdown().await.expect("clean shutdown");
    }

    #[tokio::test]
    async fn issuance_rate_limit_is_one_bucket_across_documents_and_validate() {
        let (_temp, runtime, _store) = spawn_runtime(Arc::new(NoPublicProductRoutes)).await;
        let client = client();
        let stale = r#"{"nonce":"00000000-0000-4000-8000-00000000dead"}"#;

        // Drain the shared per-client bucket through the validate surface.
        let mut denied = None;
        for _ in 0..80 {
            let response =
                post_validate(&client, &runtime, TEST_AUTHORITY, Some(TEST_ORIGIN), stale).await;
            if response.status() == StatusCode::TOO_MANY_REQUESTS {
                denied = Some(response);
                break;
            }
            assert_eq!(response.status(), StatusCode::OK);
        }
        let denied = denied.expect("the issuance bucket must exhaust");
        let retry_after = denied
            .headers()
            .get(RETRY_AFTER.as_str())
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u32>().ok())
            .expect("Retry-After header");
        assert!(retry_after >= 1);
        let body: Value = denied.json().await.expect("denied body");
        assert_eq!(body["error"]["code"], "RATE_LIMITED");

        // Same bucket: the homepage document GET is now denied too, so the
        // validate surface cannot be used to bypass the issuance limit (nor
        // vice versa).
        let response = client
            .get(request_url(&runtime, "/"))
            .header(HOST.as_str(), TEST_AUTHORITY)
            .send()
            .await
            .expect("document under exhausted bucket");
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

        runtime.shutdown().await.expect("clean shutdown");
    }

    #[tokio::test]
    async fn websocket_wrong_authority_is_misdirected_and_leases_bound_connections() {
        let (_temp, runtime, _store) = spawn_runtime(Arc::new(NoPublicProductRoutes)).await;
        let client = client();
        let (_, _, bootstrap) = document(&client, &runtime, "/", "en-US").await;
        let nonce = bootstrap_data(&bootstrap)["sessionNonce"]
            .as_str()
            .expect("document nonce")
            .to_owned();
        let path = format!("/api/v1/watch?session={nonce}");

        // Checklist item 4: a wrong authority on the WS handshake is 421.
        let (response, _stream) = websocket_handshake_with_host(
            runtime.address(),
            &path,
            "evil.example.test",
            TEST_ORIGIN,
            Some(PUBLIC_WS_SUBPROTOCOL),
        )
        .await;
        assert!(
            response.starts_with("HTTP/1.1 421 "),
            "wrong authority must be misdirected, got: {response}",
        );

        // The per-session lease caps connections: the cap accepts exactly
        // MAX_WS_CONNECTIONS_PER_SESSION concurrent upgrades and rejects the
        // next while the leases are held.
        let mut held = Vec::new();
        for ordinal in 0..crate::session::MAX_WS_CONNECTIONS_PER_SESSION {
            let (response, stream) = websocket_handshake(
                runtime.address(),
                &path,
                TEST_ORIGIN,
                Some(PUBLIC_WS_SUBPROTOCOL),
            )
            .await;
            assert!(
                response.starts_with("HTTP/1.1 101 "),
                "connection {ordinal} within the cap must upgrade, got: {response}",
            );
            held.push(stream);
        }
        let (over_cap, _stream) = websocket_handshake(
            runtime.address(),
            &path,
            TEST_ORIGIN,
            Some(PUBLIC_WS_SUBPROTOCOL),
        )
        .await;
        assert!(
            over_cap.starts_with("HTTP/1.1 403 "),
            "the connection over the per-session cap must be rejected, got: {over_cap}",
        );

        runtime.shutdown().await.expect("clean shutdown");
    }
}
