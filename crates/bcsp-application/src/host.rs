use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Request, State};
use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, CONTENT_TYPE, HOST, ORIGIN, REFERRER_POLICY,
    SEC_WEBSOCKET_PROTOCOL, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use axum::http::{HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::{any, get};
use bcsp_contracts::{
    ApiErrorBody, ApiErrorCode, ApiErrorEnvelope, SystemTraceIdSource, TraceId, TraceIdError,
    TraceIdSource,
};
use thiserror::Error;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

const MAX_EXTENSION_BODY_BYTES: usize = 1024 * 1024;
/// The one WebSocket frame/message ceiling every target shares.
///
/// It is applied at the upgrade rather than inside [`serve_websocket`]:
/// axum fixes both limits when the upgrade is configured, so the pump
/// receives a socket that is already bounded -- or already unbounded, with
/// no way left to narrow it. [`shared_websocket_upgrade`] is therefore the
/// single place either target may build a socket from.
pub const MAX_WEBSOCKET_MESSAGE_BYTES: usize = 64 * 1024;
const LOOPBACK_SHUTDOWN_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);
const SESSION_HEADER: &str = "x-bcsp-session";
pub const SHARED_WATCH_SUBPROTOCOL: &str = "bcsp.v1";
const SOCKET_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
const SOCKET_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(60);
const SOCKET_PRODUCT_TICK_INTERVAL: Duration = Duration::from_millis(250);
/// The host's own WebSocket route, and the only path the router registers
/// without a target asking for it. No injected route may claim it; a second
/// built-in route would have to join [`secondary_route_path_rejection`]'s
/// reserved check at the same time it joins the router.
const WATCH_SOCKET_PATH: &str = "/api/v1/watch";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestMethod {
    Get,
    Head,
    Post,
    Put,
    Patch,
    Delete,
    Options,
    Other(String),
}

impl RequestMethod {
    pub fn from_http(method: &axum::http::Method) -> Self {
        match *method {
            axum::http::Method::GET => Self::Get,
            axum::http::Method::HEAD => Self::Head,
            axum::http::Method::POST => Self::Post,
            axum::http::Method::PUT => Self::Put,
            axum::http::Method::PATCH => Self::Patch,
            axum::http::Method::DELETE => Self::Delete,
            axum::http::Method::OPTIONS => Self::Options,
            _ => Self::Other(method.as_str().to_owned()),
        }
    }

    pub const fn changes_state(&self) -> bool {
        !matches!(self, Self::Get | Self::Head | Self::Options)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionRequest {
    method: RequestMethod,
    path: String,
    query: Option<String>,
    body: Vec<u8>,
}

impl ExtensionRequest {
    pub fn new(
        method: RequestMethod,
        path: impl Into<String>,
        query: Option<String>,
        body: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            method,
            path: path.into(),
            query,
            body: body.into(),
        }
    }

    pub const fn method(&self) -> &RequestMethod {
        &self.method
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionResponse {
    status: u16,
    content_type: &'static str,
    body: Vec<u8>,
}

impl ExtensionResponse {
    pub fn bytes(status: u16, content_type: &'static str, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            content_type,
            body: body.into(),
        }
    }

    pub fn json_bytes(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self::bytes(status, "application/json; charset=utf-8", body)
    }

    pub fn html(status: u16, value: impl Into<Vec<u8>>) -> Self {
        Self::bytes(status, "text/html; charset=utf-8", value)
    }

    pub fn text(status: u16, value: impl Into<Vec<u8>>) -> Self {
        Self::bytes(status, "text/plain; charset=utf-8", value)
    }

    pub const fn no_content() -> Self {
        Self {
            status: 204,
            content_type: "text/plain; charset=utf-8",
            body: Vec::new(),
        }
    }

    pub fn not_found() -> Self {
        Self::text(404, "not found")
    }

    pub const fn status(&self) -> u16 {
        self.status
    }

    pub const fn content_type(&self) -> &'static str {
        self.content_type
    }

    pub fn body(&self) -> &[u8] {
        &self.body
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExtensionRoute {
    method: RequestMethod,
    path: &'static str,
}

impl ExtensionRoute {
    pub const fn new(method: RequestMethod, path: &'static str) -> Self {
        Self { method, path }
    }

    pub fn matches(&self, method: &RequestMethod, path: &str) -> bool {
        self.method == *method && self.path == path
    }

    pub const fn method(&self) -> &RequestMethod {
        &self.method
    }

    pub const fn path(&self) -> &'static str {
        self.path
    }
}

pub trait RouteExtension: Send + Sync + 'static {
    fn route_inventory(&self) -> &'static [ExtensionRoute] {
        &[]
    }

    fn handle(&self, request: ExtensionRequest) -> ExtensionResponse;
}

/// Shared text-frame transport seam used by the watch protocol on both targets.
/// Implementations own typed frame decoding and connection-bound watch state.
pub trait WebSocketExtension: Send + Sync + 'static {
    fn connect(&self, connection_id: TraceId, outbound: mpsc::UnboundedSender<String>) -> bool;

    fn transport_activity(&self, _connection_id: TraceId) {}

    fn receive_text(&self, connection_id: TraceId, message: &str);

    fn disconnect(&self, connection_id: TraceId);

    fn tick(&self) {}
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SessionNonce(String);

impl SessionNonce {
    pub fn generate() -> Self {
        let mut source = SystemTraceIdSource;
        Self(source.next_trace_id().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionNonce {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for SessionNonce {
    type Err = TraceIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = TraceId::from_str(value)?;
        Ok(Self(value.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum LoopbackServerError {
    #[error("failed to bind the loopback listener: {0}")]
    Bind(#[source] std::io::Error),
    #[error("loopback server failed: {0}")]
    Serve(#[source] std::io::Error),
    #[error("loopback server task failed: {0}")]
    Join(#[source] tokio::task::JoinError),
    #[error("secondary WebSocket route path {path:?} rejected: {rejection}")]
    SecondaryRoutePath {
        path: &'static str,
        rejection: SecondaryRoutePathRejection,
    },
}

/// Why a target-supplied secondary WebSocket path was refused.
///
/// Every one of these is refused before a listener exists, because the two
/// downstream failure modes are both worse than an error return: a path
/// axum reads as a pattern silently widens the host's attack surface, and a
/// duplicate path makes `Router::route` panic inside the spawn path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SecondaryRoutePathRejection {
    /// The path does not start at the root.
    NotAbsolute,
    /// The path ends in `/`, or two separators are adjacent.
    EmptySegment,
    /// A segment is `.` or `..`.
    RelativeSegment,
    /// The path carries a query or fragment instead of being a bare path.
    QueryOrFragment,
    /// A character outside the RFC 3986 unreserved set. This is how axum
    /// path parameters (`{id}`), wildcards (`*rest`), and percent escapes
    /// would get in.
    NotLiteral,
    /// The path is the host's own built-in watch route.
    ReservedPath,
    /// Another route in the same set already claimed this path.
    Duplicate,
}

impl fmt::Display for SecondaryRoutePathRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotAbsolute => "not an absolute path",
            Self::EmptySegment => "empty path segment",
            Self::RelativeSegment => "relative path segment",
            Self::QueryOrFragment => "carries a query or fragment",
            Self::NotLiteral => "not a literal path",
            Self::ReservedPath => "reserved by the built-in watch route",
            Self::Duplicate => "already claimed by another route in the set",
        })
    }
}

pub struct LoopbackServer {
    origin: String,
    nonce: SessionNonce,
    shutdown: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<Result<(), std::io::Error>>>,
    socket_maintenance: Option<JoinHandle<()>>,
}

impl LoopbackServer {
    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub const fn nonce(&self) -> &SessionNonce {
        &self.nonce
    }

    pub fn browser_url(&self) -> String {
        format!("{}/", self.origin)
    }

    pub async fn shutdown(mut self) -> Result<(), LoopbackServerError> {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(socket_maintenance) = self.socket_maintenance.take() {
            socket_maintenance.abort();
            let _ = socket_maintenance.await;
        }
        if let Some(task) = self.task.take() {
            drain_loopback_server_task(task, LOOPBACK_SHUTDOWN_DRAIN_TIMEOUT).await?;
        }
        Ok(())
    }
}

async fn drain_loopback_server_task(
    mut task: JoinHandle<Result<(), std::io::Error>>,
    timeout: Duration,
) -> Result<(), LoopbackServerError> {
    match tokio::time::timeout(timeout, &mut task).await {
        Ok(result) => result
            .map_err(LoopbackServerError::Join)?
            .map_err(LoopbackServerError::Serve),
        Err(_) => {
            tracing::warn!(code = "LOOPBACK_SHUTDOWN_DRAIN_TIMEOUT");
            task.abort();
            let _ = task.await;
            Ok(())
        }
    }
}

impl Drop for LoopbackServer {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        if let Some(socket_maintenance) = self.socket_maintenance.take() {
            socket_maintenance.abort();
        }
    }
}

#[derive(Clone)]
struct HostState {
    authority: String,
    origin: String,
    nonce: SessionNonce,
    extension: Arc<dyn RouteExtension>,
    socket: Option<Arc<dyn WebSocketExtension>>,
    secondary_sockets: Arc<BTreeMap<&'static str, Arc<dyn WebSocketExtension>>>,
}

/// A target-injected additional WebSocket route. The shared host registers
/// only the routes a target supplies at spawn time; an un-injected path does
/// not exist in the route table and falls through to the extension fallback,
/// whose 404 is the local extension's behaviour rather than a promise of this
/// seam. Admission (Host/Origin/session nonce/subprotocol) is the host's
/// standard WebSocket policy, identical to the built-in watch route.
pub struct SecondaryWebSocketRoute {
    path: &'static str,
    socket: Arc<dyn WebSocketExtension>,
}

impl SecondaryWebSocketRoute {
    /// Validates the path at construction, so a rejected path can never
    /// reach `Router::route`.
    pub fn new(
        path: &'static str,
        socket: Arc<dyn WebSocketExtension>,
    ) -> Result<Self, LoopbackServerError> {
        if let Some(rejection) = secondary_route_path_rejection(path) {
            return Err(LoopbackServerError::SecondaryRoutePath { path, rejection });
        }
        Ok(Self { path, socket })
    }

    pub const fn path(&self) -> &'static str {
        self.path
    }
}

/// Absolute, exact, literal, and not the built-in watch path.
fn secondary_route_path_rejection(path: &str) -> Option<SecondaryRoutePathRejection> {
    if path == WATCH_SOCKET_PATH {
        return Some(SecondaryRoutePathRejection::ReservedPath);
    }
    if path.contains('?') || path.contains('#') {
        return Some(SecondaryRoutePathRejection::QueryOrFragment);
    }
    let Some(segments) = path.strip_prefix('/') else {
        return Some(SecondaryRoutePathRejection::NotAbsolute);
    };
    for segment in segments.split('/') {
        if segment.is_empty() {
            return Some(SecondaryRoutePathRejection::EmptySegment);
        }
        if segment == "." || segment == ".." {
            return Some(SecondaryRoutePathRejection::RelativeSegment);
        }
        if !segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~'))
        {
            return Some(SecondaryRoutePathRejection::NotLiteral);
        }
    }
    None
}

pub async fn spawn_loopback_server(
    extension: Arc<dyn RouteExtension>,
    nonce: SessionNonce,
) -> Result<LoopbackServer, LoopbackServerError> {
    spawn_loopback_server_internal(extension, None, Vec::new(), nonce).await
}

pub async fn spawn_loopback_server_with_socket(
    extension: Arc<dyn RouteExtension>,
    socket: Arc<dyn WebSocketExtension>,
    nonce: SessionNonce,
) -> Result<LoopbackServer, LoopbackServerError> {
    spawn_loopback_server_internal(extension, Some(socket), Vec::new(), nonce).await
}

/// Spawns with a validated set of secondary routes. The set is checked for
/// in-set path uniqueness before anything is bound or registered.
pub async fn spawn_loopback_server_with_sockets(
    extension: Arc<dyn RouteExtension>,
    socket: Arc<dyn WebSocketExtension>,
    secondary_routes: Vec<SecondaryWebSocketRoute>,
    nonce: SessionNonce,
) -> Result<LoopbackServer, LoopbackServerError> {
    spawn_loopback_server_internal(extension, Some(socket), secondary_routes, nonce).await
}

async fn spawn_loopback_server_internal(
    extension: Arc<dyn RouteExtension>,
    socket: Option<Arc<dyn WebSocketExtension>>,
    secondary_routes: Vec<SecondaryWebSocketRoute>,
    nonce: SessionNonce,
) -> Result<LoopbackServer, LoopbackServerError> {
    // Collapse the set before binding: `Router::route` panics on a duplicate
    // path, and a panic here would take the process down instead of handing
    // the target a wiring error it can report.
    let mut secondary_sockets: BTreeMap<&'static str, Arc<dyn WebSocketExtension>> =
        BTreeMap::new();
    for route in secondary_routes {
        let SecondaryWebSocketRoute { path, socket } = route;
        if secondary_sockets.insert(path, socket).is_some() {
            return Err(LoopbackServerError::SecondaryRoutePath {
                path,
                rejection: SecondaryRoutePathRejection::Duplicate,
            });
        }
    }
    let secondary_sockets = Arc::new(secondary_sockets);
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(LoopbackServerError::Bind)?;
    let address = listener.local_addr().map_err(LoopbackServerError::Bind)?;
    let authority = format!("127.0.0.1:{}", address.port());
    let origin = format!("http://{authority}");
    // One maintenance task drives every injected socket extension on the
    // shared cadence; a socket with the default no-op tick costs nothing.
    let tick_sockets: Vec<Arc<dyn WebSocketExtension>> = socket
        .iter()
        .chain(secondary_sockets.values())
        .map(Arc::clone)
        .collect();
    let socket_maintenance = (!tick_sockets.is_empty()).then(|| {
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(SOCKET_PRODUCT_TICK_INTERVAL);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tick.tick().await;
                let tick_targets = tick_sockets.clone();
                if tokio::task::spawn_blocking(move || {
                    for target in &tick_targets {
                        target.tick();
                    }
                })
                .await
                .is_err()
                {
                    break;
                }
            }
        })
    });
    let state = HostState {
        authority,
        origin: origin.clone(),
        nonce: nonce.clone(),
        extension,
        socket,
        secondary_sockets: Arc::clone(&secondary_sockets),
    };
    let mut router = Router::new().route(WATCH_SOCKET_PATH, get(handle_watch_socket));
    for path in secondary_sockets.keys() {
        router = router.route(path, get(handle_secondary_socket));
    }
    let router = router.fallback(any(handle_extension)).with_state(state);
    let (shutdown, receiver) = oneshot::channel();
    let task = tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = receiver.await;
            })
            .await
    });
    Ok(LoopbackServer {
        origin,
        nonce,
        shutdown: Some(shutdown),
        task: Some(task),
        socket_maintenance,
    })
}

/// The host's single WebSocket admission policy: exact loopback authority,
/// exact origin, exactly one matching session nonce, and the shared
/// subprotocol. Every WebSocket route (built-in watch and any injected
/// secondary route) admits through this one check.
fn websocket_admission_denied(state: &HostState, request: &Request) -> bool {
    header_text(request.headers(), HOST).as_deref() != Some(state.authority.as_str())
        || header_text(request.headers(), ORIGIN).as_deref() != Some(state.origin.as_str())
        || session_query(request.uri().query()) != Some(state.nonce.as_str())
        || !requested_subprotocol(request.headers())
}

async fn handle_watch_socket(
    State(state): State<HostState>,
    upgrade: WebSocketUpgrade,
    request: Request,
) -> Response {
    if websocket_admission_denied(&state, &request) {
        return api_error_response(StatusCode::FORBIDDEN, ApiErrorCode::MalformedRequest);
    }
    let Some(extension) = state.socket else {
        return extension_response(ExtensionResponse::not_found());
    };
    let mut source = SystemTraceIdSource;
    let connection_id = source.next_trace_id();
    shared_websocket_upgrade(upgrade, SHARED_WATCH_SUBPROTOCOL)
        .on_upgrade(move |socket| serve_websocket(socket, extension, connection_id))
}

async fn handle_secondary_socket(
    State(state): State<HostState>,
    upgrade: WebSocketUpgrade,
    request: Request,
) -> Response {
    if websocket_admission_denied(&state, &request) {
        return api_error_response(StatusCode::FORBIDDEN, ApiErrorCode::MalformedRequest);
    }
    // Registered paths are exact literals, so the request path is the map
    // key. Fail closed rather than panic if that ever stops being true.
    let Some(extension) = state.secondary_sockets.get(request.uri().path()).cloned() else {
        return extension_response(ExtensionResponse::not_found());
    };
    let mut source = SystemTraceIdSource;
    let connection_id = source.next_trace_id();
    shared_websocket_upgrade(upgrade, SHARED_WATCH_SUBPROTOCOL)
        .on_upgrade(move |socket| serve_websocket(socket, extension, connection_id))
}

/// Configures a WebSocket upgrade the one way every target uses: the shared
/// subprotocol offer and [`MAX_WEBSOCKET_MESSAGE_BYTES`] on both the frame
/// and the message. An oversized frame fails inside axum's codec, so the
/// pump sees a transport error and tears the connection down without the
/// extension ever being handed the bytes.
pub fn shared_websocket_upgrade(
    upgrade: WebSocketUpgrade,
    subprotocol: &'static str,
) -> WebSocketUpgrade {
    upgrade
        .protocols([subprotocol])
        .max_message_size(MAX_WEBSOCKET_MESSAGE_BYTES)
        .max_frame_size(MAX_WEBSOCKET_MESSAGE_BYTES)
}

/// Runs the target-neutral WebSocket frame, heartbeat, and cleanup pump.
///
/// Target hosts own their Origin and session admission policy, then hand the
/// admitted socket to this one shared transport implementation.
pub async fn serve_websocket(
    mut socket: WebSocket,
    extension: Arc<dyn WebSocketExtension>,
    connection_id: TraceId,
) {
    let (outbound, mut outbound_messages) = mpsc::unbounded_channel();
    let connect_extension = Arc::clone(&extension);
    let connected =
        tokio::task::spawn_blocking(move || connect_extension.connect(connection_id, outbound))
            .await;
    if !matches!(connected, Ok(true)) {
        return;
    }
    let mut heartbeat = tokio::time::interval(SOCKET_HEARTBEAT_INTERVAL);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut last_seen = tokio::time::Instant::now();
    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(message))) => {
                        last_seen = tokio::time::Instant::now();
                        let text_extension = Arc::clone(&extension);
                        let message = message.to_string();
                        if tokio::task::spawn_blocking(move || {
                            text_extension.transport_activity(connection_id);
                            text_extension.receive_text(connection_id, &message);
                        })
                        .await
                        .is_err()
                        {
                            break;
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        last_seen = tokio::time::Instant::now();
                        let activity_extension = Arc::clone(&extension);
                        if tokio::task::spawn_blocking(move || {
                            activity_extension.transport_activity(connection_id);
                        })
                        .await
                        .is_err()
                        {
                            break;
                        }
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {
                        last_seen = tokio::time::Instant::now();
                        let activity_extension = Arc::clone(&extension);
                        if tokio::task::spawn_blocking(move || {
                            activity_extension.transport_activity(connection_id);
                        })
                        .await
                        .is_err()
                        {
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(Message::Binary(_))) => break,
                }
            }
            outbound = outbound_messages.recv() => {
                let Some(outbound) = outbound else {
                    break;
                };
                if socket.send(Message::Text(outbound.into())).await.is_err() {
                    break;
                }
            }
            _ = heartbeat.tick() => {
                if last_seen.elapsed() >= SOCKET_HEARTBEAT_TIMEOUT
                    || socket.send(Message::Ping(Vec::new().into())).await.is_err()
                {
                    break;
                }
            }
        }
    }
    let _ = tokio::task::spawn_blocking(move || extension.disconnect(connection_id)).await;
}

fn session_query(query: Option<&str>) -> Option<&str> {
    let mut session = None;
    for field in query?.split('&') {
        if let Some(value) = field.strip_prefix("session=")
            && session.replace(value).is_some()
        {
            return None;
        }
    }
    session
}

fn requested_subprotocol(headers: &axum::http::HeaderMap) -> bool {
    header_text(headers, SEC_WEBSOCKET_PROTOCOL).is_some_and(|value| {
        value
            .split(',')
            .map(str::trim)
            .any(|protocol| protocol == SHARED_WATCH_SUBPROTOCOL)
    })
}

async fn handle_extension(State(state): State<HostState>, request: Request) -> Response {
    if header_text(request.headers(), HOST).as_deref() != Some(state.authority.as_str()) {
        return api_error_response(
            StatusCode::MISDIRECTED_REQUEST,
            ApiErrorCode::MalformedRequest,
        );
    }

    let method = RequestMethod::from_http(request.method());
    let scoped_status = method == RequestMethod::Get
        && request.uri().path() == crate::PRODUCT_SERVICE_STATUS_PATH
        && request.uri().query().is_some_and(|query| {
            query
                .split('&')
                .any(|field| field.starts_with("activeTerm=") || field.starts_with("activeCampus="))
        });
    if method.changes_state()
        && (header_text(request.headers(), ORIGIN).as_deref() != Some(state.origin.as_str())
            || header_text_name(request.headers(), SESSION_HEADER).as_deref()
                != Some(state.nonce.as_str()))
    {
        return api_error_response(StatusCode::FORBIDDEN, ApiErrorCode::MalformedRequest);
    }
    if scoped_status
        && header_text_name(request.headers(), SESSION_HEADER).as_deref()
            != Some(state.nonce.as_str())
    {
        return api_error_response(StatusCode::FORBIDDEN, ApiErrorCode::MalformedRequest);
    }

    let path = request.uri().path().to_owned();
    let query = request.uri().query().map(str::to_owned);
    let body = match to_bytes(request.into_body(), MAX_EXTENSION_BODY_BYTES).await {
        Ok(body) => body.to_vec(),
        Err(_) => {
            return api_error_response(
                StatusCode::PAYLOAD_TOO_LARGE,
                ApiErrorCode::MalformedRequest,
            );
        }
    };
    let extension = state.extension;
    let response = tokio::task::spawn_blocking(move || {
        extension.handle(ExtensionRequest {
            method,
            path,
            query,
            body,
        })
    })
    .await;
    match response {
        Ok(response) => extension_response(response),
        Err(_) => api_error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiErrorCode::InternalError,
        ),
    }
}

fn header_text(headers: &axum::http::HeaderMap, name: axum::http::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn header_text_name(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn api_error_response(status: StatusCode, code: ApiErrorCode) -> Response {
    let mut source = SystemTraceIdSource;
    let envelope = ApiErrorEnvelope::new(ApiErrorBody::new(code, source.next_trace_id()));
    let body = serde_json::to_vec(&envelope)
        .unwrap_or_else(|_| br#"{"error":{"code":"INTERNAL_ERROR"}}"#.to_vec());
    extension_response(ExtensionResponse::json_bytes(status.as_u16(), body))
}

fn extension_response(value: ExtensionResponse) -> Response {
    let status = StatusCode::from_u16(value.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let mut response = Response::new(Body::from(value.body));
    *response.status_mut() = status;
    let headers = response.headers_mut();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static(value.content_type));
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    headers.insert(
        CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; base-uri 'none'; frame-ancestors 'none'; object-src 'none'; connect-src 'self'; img-src 'self' data:; font-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self'; worker-src 'none'; form-action 'none'",
        ),
    );
    response
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpStream};
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Condvar, Mutex};

    use axum::http::Request as HttpRequest;
    use tower::ServiceExt;

    use super::*;

    struct CountingExtension {
        calls: AtomicUsize,
    }

    #[derive(Default)]
    struct BlockingConnectState {
        entered: bool,
        released: bool,
    }

    struct BlockingConnectSocket {
        block_next: AtomicBool,
        state: Mutex<BlockingConnectState>,
        changed: Condvar,
    }

    impl BlockingConnectSocket {
        fn new() -> Self {
            Self {
                block_next: AtomicBool::new(true),
                state: Mutex::new(BlockingConnectState::default()),
                changed: Condvar::new(),
            }
        }

        fn wait_until_blocked(&self, timeout: Duration) -> bool {
            let state = self.state.lock().unwrap();
            let (state, _) = self
                .changed
                .wait_timeout_while(state, timeout, |state| !state.entered)
                .unwrap();
            state.entered
        }

        fn release(&self) {
            let mut state = self.state.lock().unwrap();
            state.released = true;
            self.changed.notify_all();
        }
    }

    impl WebSocketExtension for BlockingConnectSocket {
        fn connect(
            &self,
            _connection_id: TraceId,
            _outbound: mpsc::UnboundedSender<String>,
        ) -> bool {
            if self.block_next.swap(false, Ordering::SeqCst) {
                let mut state = self.state.lock().unwrap();
                state.entered = true;
                self.changed.notify_all();
                while !state.released {
                    state = self.changed.wait(state).unwrap();
                }
            }
            true
        }

        fn receive_text(&self, _connection_id: TraceId, _message: &str) {}

        fn disconnect(&self, _connection_id: TraceId) {}
    }

    #[derive(Default)]
    struct CountingSocket {
        ticks: AtomicUsize,
    }

    impl WebSocketExtension for CountingSocket {
        fn connect(
            &self,
            _connection_id: TraceId,
            _outbound: mpsc::UnboundedSender<String>,
        ) -> bool {
            false
        }

        fn receive_text(&self, _connection_id: TraceId, _message: &str) {}

        fn disconnect(&self, _connection_id: TraceId) {}

        fn tick(&self) {
            self.ticks.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Accepts the connection, unlike [`CountingSocket`], so a test can put
    /// real frames through the shared pump.
    #[derive(Default)]
    struct RecordingSocket {
        connects: AtomicUsize,
        texts: AtomicUsize,
        bytes: AtomicUsize,
        // Held for the life of the connection on purpose: the pump ends as
        // soon as the outbound channel closes, so an extension that drops
        // its sender tears its own connection down before any frame lands.
        outbound: Mutex<Vec<mpsc::UnboundedSender<String>>>,
    }

    impl WebSocketExtension for RecordingSocket {
        fn connect(
            &self,
            _connection_id: TraceId,
            outbound: mpsc::UnboundedSender<String>,
        ) -> bool {
            self.outbound.lock().unwrap().push(outbound);
            self.connects.fetch_add(1, Ordering::SeqCst);
            true
        }

        fn receive_text(&self, _connection_id: TraceId, message: &str) {
            self.texts.fetch_add(1, Ordering::SeqCst);
            self.bytes.fetch_add(message.len(), Ordering::SeqCst);
        }

        fn disconnect(&self, _connection_id: TraceId) {}
    }

    impl RouteExtension for CountingExtension {
        fn handle(&self, request: ExtensionRequest) -> ExtensionResponse {
            self.calls.fetch_add(1, Ordering::SeqCst);
            ExtensionResponse::text(200, request.path().as_bytes().to_vec())
        }
    }

    fn router(extension: Arc<CountingExtension>) -> Router {
        Router::new()
            .fallback(any(handle_extension))
            .with_state(HostState {
                authority: "127.0.0.1:43210".to_owned(),
                origin: "http://127.0.0.1:43210".to_owned(),
                nonce: "00000000-0000-4000-8000-000000000001".parse().unwrap(),
                extension,
                socket: None,
                secondary_sockets: Arc::new(BTreeMap::new()),
            })
    }

    fn websocket_handshake(
        address: SocketAddr,
        path: &str,
        authority: &str,
        origin: &str,
        nonce: &str,
        timeout: Duration,
    ) -> bool {
        upgraded_websocket(address, path, authority, origin, nonce, timeout).is_some()
    }

    /// The handshake, keeping the upgraded stream so a test can drive frames
    /// through it.
    fn upgraded_websocket(
        address: SocketAddr,
        path: &str,
        authority: &str,
        origin: &str,
        nonce: &str,
        timeout: Duration,
    ) -> Option<TcpStream> {
        let Ok(mut stream) = TcpStream::connect_timeout(&address, timeout) else {
            return None;
        };
        if stream.set_read_timeout(Some(timeout)).is_err()
            || stream.set_write_timeout(Some(timeout)).is_err()
        {
            return None;
        }
        let request = format!(
            "GET {path}?session={nonce} HTTP/1.1\r\n\
             Host: {authority}\r\n\
             Origin: {origin}\r\n\
             Upgrade: websocket\r\n\
             Connection: Upgrade\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
             Sec-WebSocket-Version: 13\r\n\
             Sec-WebSocket-Protocol: {SHARED_WATCH_SUBPROTOCOL}\r\n\r\n"
        );
        if stream.write_all(request.as_bytes()).is_err() {
            return None;
        }
        let mut response = Vec::new();
        let mut buffer = [0_u8; 512];
        while !response.windows(4).any(|window| window == b"\r\n\r\n") && response.len() <= 8 * 1024
        {
            let Ok(read) = stream.read(&mut buffer) else {
                return None;
            };
            if read == 0 {
                return None;
            }
            response.extend_from_slice(&buffer[..read]);
        }
        response.starts_with(b"HTTP/1.1 101 ").then_some(stream)
    }

    /// A whole masked client text message: one FIN text frame.
    fn masked_text_frame(payload: &[u8]) -> Vec<u8> {
        masked_frame(0x81, payload)
    }

    /// A masked client frame with a caller-chosen first byte, so a test can
    /// build a fragmented message: `0x01` opens a text message with FIN
    /// clear, `0x80` closes it with a FIN continuation.
    fn masked_frame(first_byte: u8, payload: &[u8]) -> Vec<u8> {
        let mask = [0x37_u8, 0xfa, 0x21, 0x3d];
        let mut frame = vec![first_byte];
        let length = payload.len();
        if length < 126 {
            frame.push(0x80 | u8::try_from(length).unwrap());
        } else if let Ok(length) = u16::try_from(length) {
            frame.push(0x80 | 126);
            frame.extend_from_slice(&length.to_be_bytes());
        } else {
            frame.push(0x80 | 127);
            frame.extend_from_slice(&(length as u64).to_be_bytes());
        }
        frame.extend_from_slice(&mask);
        frame.extend(
            payload
                .iter()
                .enumerate()
                .map(|(index, byte)| byte ^ mask[index % 4]),
        );
        frame
    }

    /// True once the peer stops the connection. Three shapes mean the same
    /// thing to a client: a Close frame, a plain EOF, or a reset -- the last
    /// is what a host produces when it drops a socket whose receive buffer
    /// still holds bytes it refused to read. The shared pump's heartbeat
    /// sends a Ping the moment a connection opens, so server frames have to
    /// be walked rather than sampled. A read that merely times out is the
    /// one outcome meaning the connection is still up.
    fn closed_by_peer(stream: &mut TcpStream) -> bool {
        let mut pending = Vec::new();
        loop {
            while let Some(opcode) = take_server_frame(&mut pending) {
                if opcode & 0x0f == 0x08 {
                    return true;
                }
            }
            let mut buffer = [0_u8; 256];
            match stream.read(&mut buffer) {
                Ok(0) => return true,
                Ok(read) => pending.extend_from_slice(&buffer[..read]),
                Err(error) => {
                    return matches!(
                        error.kind(),
                        std::io::ErrorKind::ConnectionAborted | std::io::ErrorKind::ConnectionReset
                    );
                }
            }
        }
    }

    /// Pops one complete server frame and reports its first byte. Server
    /// frames are never masked, so the header is opcode plus length only.
    fn take_server_frame(pending: &mut Vec<u8>) -> Option<u8> {
        if pending.len() < 2 {
            return None;
        }
        let opcode = pending[0];
        let (header, length) = match pending[1] & 0x7f {
            126 => (
                4,
                usize::from(u16::from_be_bytes([*pending.get(2)?, *pending.get(3)?])),
            ),
            127 => {
                let bytes: [u8; 8] = pending.get(2..10)?.try_into().ok()?;
                (10, usize::try_from(u64::from_be_bytes(bytes)).ok()?)
            }
            short => (2, usize::from(short)),
        };
        if pending.len() < header + length {
            return None;
        }
        pending.drain(..header + length);
        Some(opcode)
    }

    #[test]
    fn target_hosts_can_construct_the_shared_extension_request() {
        let request = ExtensionRequest::new(
            RequestMethod::Post,
            "/api/v1/query",
            Some("page=2".to_owned()),
            br#"{}"#.to_vec(),
        );
        assert_eq!(request.method(), &RequestMethod::Post);
        assert_eq!(request.path(), "/api/v1/query");
        assert_eq!(request.query(), Some("page=2"));
        assert_eq!(request.body(), br#"{}"#);
    }

    #[tokio::test]
    async fn reads_are_loopback_host_bound_and_emit_no_cors_header() {
        let extension = Arc::new(CountingExtension {
            calls: AtomicUsize::new(0),
        });
        let response = router(extension.clone())
            .oneshot(
                HttpRequest::builder()
                    .uri("/status")
                    .header(HOST, "127.0.0.1:43210")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert!(
            !response
                .headers()
                .contains_key("access-control-allow-origin")
        );
        assert_eq!(
            response
                .headers()
                .get(CONTENT_SECURITY_POLICY)
                .and_then(|value| value.to_str().ok()),
            Some(
                "default-src 'self'; base-uri 'none'; frame-ancestors 'none'; object-src 'none'; connect-src 'self'; img-src 'self' data:; font-src 'self'; style-src 'self' 'unsafe-inline'; script-src 'self'; worker-src 'none'; form-action 'none'"
            )
        );
        assert_eq!(extension.calls.load(Ordering::SeqCst), 1);

        let response = router(extension.clone())
            .oneshot(
                HttpRequest::builder()
                    .uri("/status")
                    .header(HOST, "attacker.invalid")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::MISDIRECTED_REQUEST);
        assert_eq!(extension.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn mutations_require_exact_origin_and_session_nonce() {
        let extension = Arc::new(CountingExtension {
            calls: AtomicUsize::new(0),
        });
        let invalid = router(extension.clone())
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/settings")
                    .header(HOST, "127.0.0.1:43210")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(invalid.status(), StatusCode::FORBIDDEN);
        assert_eq!(extension.calls.load(Ordering::SeqCst), 0);

        let valid = router(extension.clone())
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/settings")
                    .header(HOST, "127.0.0.1:43210")
                    .header(ORIGIN, "http://127.0.0.1:43210")
                    .header(SESSION_HEADER, "00000000-0000-4000-8000-000000000001")
                    .body(Body::from("{}"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(valid.status(), StatusCode::OK);
        assert_eq!(extension.calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn websocket_session_query_is_exact_and_unique() {
        assert_eq!(session_query(Some("session=abc")), Some("abc"));
        assert_eq!(session_query(Some("other=1&session=abc")), Some("abc"));
        assert_eq!(session_query(Some("session=abc&session=abc")), None);
        assert_eq!(session_query(Some("session%3Dabc")), None);
        assert_eq!(session_query(None), None);
    }

    #[tokio::test]
    async fn one_host_maintenance_task_ticks_the_shared_socket_extension() {
        let extension = Arc::new(CountingExtension {
            calls: AtomicUsize::new(0),
        });
        let socket = Arc::new(CountingSocket::default());
        let server =
            spawn_loopback_server_with_socket(extension, socket.clone(), SessionNonce::generate())
                .await
                .unwrap();

        tokio::time::timeout(Duration::from_secs(1), async {
            while socket.ticks.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("shared socket maintenance tick");
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn a_stuck_transport_drain_is_bounded_before_database_shutdown() {
        let task = tokio::spawn(std::future::pending::<Result<(), std::io::Error>>());
        tokio::time::timeout(
            Duration::from_secs(1),
            drain_loopback_server_task(task, Duration::from_millis(20)),
        )
        .await
        .expect("the loopback drain deadline must be bounded")
        .expect("a bounded transport drain is a successful shutdown");
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 1)]
    async fn blocked_extension_connect_does_not_starve_a_second_websocket_handshake() {
        let extension = Arc::new(CountingExtension {
            calls: AtomicUsize::new(0),
        });
        let socket = Arc::new(BlockingConnectSocket::new());
        let server =
            spawn_loopback_server_with_socket(extension, socket.clone(), SessionNonce::generate())
                .await
                .unwrap();
        let origin = server.origin().to_owned();
        let authority = origin.strip_prefix("http://").unwrap().to_owned();
        let address = authority.parse::<SocketAddr>().unwrap();
        let nonce = server.nonce().as_str().to_owned();
        let watchdog_socket = socket.clone();

        let watchdog = std::thread::spawn(move || {
            let first = websocket_handshake(
                address,
                "/api/v1/watch",
                &authority,
                &origin,
                &nonce,
                Duration::from_secs(2),
            );
            let blocked = first && watchdog_socket.wait_until_blocked(Duration::from_secs(2));
            let second = blocked
                && websocket_handshake(
                    address,
                    "/api/v1/watch",
                    &authority,
                    &origin,
                    &nonce,
                    Duration::from_secs(1),
                );
            watchdog_socket.release();
            (first, blocked, second)
        });
        let (first, blocked, second) = tokio::task::spawn_blocking(move || watchdog.join())
            .await
            .unwrap()
            .unwrap();

        assert!(first, "first WebSocket handshake must be accepted");
        assert!(
            blocked,
            "first extension connect must reach the blocking seam"
        );
        assert!(
            second,
            "a blocked extension connect must not starve the async handshake worker"
        );
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn injected_secondary_websocket_route_admits_with_the_standard_policy() {
        let extension = Arc::new(CountingExtension {
            calls: AtomicUsize::new(0),
        });
        let watch = Arc::new(CountingSocket::default());
        let secondary = Arc::new(CountingSocket::default());
        let server = spawn_loopback_server_with_sockets(
            extension.clone(),
            watch,
            vec![SecondaryWebSocketRoute::new("/api/v1/second-socket", secondary).unwrap()],
            SessionNonce::generate(),
        )
        .await
        .unwrap();
        let origin = server.origin().to_owned();
        let authority = origin.strip_prefix("http://").unwrap().to_owned();
        let address = authority.parse::<SocketAddr>().unwrap();
        let nonce = server.nonce().as_str().to_owned();

        let (secondary_ok, wrong_nonce_ok, watch_ok) =
            tokio::task::spawn_blocking(move || {
                (
                    websocket_handshake(
                        address,
                        "/api/v1/second-socket",
                        &authority,
                        &origin,
                        &nonce,
                        Duration::from_secs(2),
                    ),
                    websocket_handshake(
                        address,
                        "/api/v1/second-socket",
                        &authority,
                        &origin,
                        "00000000-0000-4000-8000-0000000000ff",
                        Duration::from_secs(2),
                    ),
                    websocket_handshake(
                        address,
                        "/api/v1/watch",
                        &authority,
                        &origin,
                        &nonce,
                        Duration::from_secs(2),
                    ),
                )
            })
            .await
            .unwrap();

        assert!(secondary_ok, "the injected route must upgrade with valid admission");
        assert!(!wrong_nonce_ok, "the injected route shares the exact nonce admission");
        assert!(watch_ok, "the built-in watch route is untouched by the injection");
        assert_eq!(
            extension.calls.load(Ordering::SeqCst),
            0,
            "WebSocket paths never reach the extension fallback while registered",
        );
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn absent_secondary_route_falls_through_to_the_extension_fallback() {
        let extension = Arc::new(CountingExtension {
            calls: AtomicUsize::new(0),
        });
        let watch = Arc::new(CountingSocket::default());
        let server =
            spawn_loopback_server_with_socket(extension.clone(), watch, SessionNonce::generate())
                .await
                .unwrap();
        let origin = server.origin().to_owned();
        let authority = origin.strip_prefix("http://").unwrap().to_owned();
        let address = authority.parse::<SocketAddr>().unwrap();
        let nonce = server.nonce().as_str().to_owned();

        let upgraded = tokio::task::spawn_blocking(move || {
            websocket_handshake(
                address,
                "/api/v1/second-socket",
                &authority,
                &origin,
                &nonce,
                Duration::from_secs(2),
            )
        })
        .await
        .unwrap();

        assert!(!upgraded, "without injection the path must not exist as a WebSocket route");
        assert_eq!(
            extension.calls.load(Ordering::SeqCst),
            1,
            "the un-injected path falls through to the extension fallback",
        );
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn the_maintenance_task_ticks_primary_and_secondary_sockets() {
        let extension = Arc::new(CountingExtension {
            calls: AtomicUsize::new(0),
        });
        let watch = Arc::new(CountingSocket::default());
        let secondary = Arc::new(CountingSocket::default());
        let server = spawn_loopback_server_with_sockets(
            extension,
            watch.clone(),
            vec![SecondaryWebSocketRoute::new("/api/v1/second-socket", secondary.clone()).unwrap()],
            SessionNonce::generate(),
        )
        .await
        .unwrap();

        tokio::time::timeout(Duration::from_secs(1), async {
            while watch.ticks.load(Ordering::SeqCst) == 0
                || secondary.ticks.load(Ordering::SeqCst) == 0
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("both injected sockets share the maintenance cadence");
        server.shutdown().await.unwrap();
    }

    #[test]
    fn a_secondary_route_path_must_be_absolute_exact_and_literal() {
        let socket = || Arc::new(CountingSocket::default()) as Arc<dyn WebSocketExtension>;
        for (path, expected) in [
            (
                "api/v1/local/presence",
                SecondaryRoutePathRejection::NotAbsolute,
            ),
            ("", SecondaryRoutePathRejection::NotAbsolute),
            ("/", SecondaryRoutePathRejection::EmptySegment),
            ("/api//presence", SecondaryRoutePathRejection::EmptySegment),
            (
                "/api/v1/presence/",
                SecondaryRoutePathRejection::EmptySegment,
            ),
            (
                "/api/../watch",
                SecondaryRoutePathRejection::RelativeSegment,
            ),
            ("/api/./watch", SecondaryRoutePathRejection::RelativeSegment),
            (
                "/api/v1/presence?session=1",
                SecondaryRoutePathRejection::QueryOrFragment,
            ),
            (
                "/api/v1/presence#tab",
                SecondaryRoutePathRejection::QueryOrFragment,
            ),
            // Every axum pattern form, plus percent escapes, is a non-literal.
            ("/api/v1/{kind}", SecondaryRoutePathRejection::NotLiteral),
            ("/api/v1/*rest", SecondaryRoutePathRejection::NotLiteral),
            ("/api/v1/:kind", SecondaryRoutePathRejection::NotLiteral),
            (
                "/api/v1/local/pre%2Dsence",
                SecondaryRoutePathRejection::NotLiteral,
            ),
            (
                "/api/v1/local presence",
                SecondaryRoutePathRejection::NotLiteral,
            ),
            // Claiming the built-in path would panic the router at startup.
            ("/api/v1/watch", SecondaryRoutePathRejection::ReservedPath),
        ] {
            let error = SecondaryWebSocketRoute::new(path, socket())
                .err()
                .unwrap_or_else(|| panic!("{path} must be rejected"));
            let LoopbackServerError::SecondaryRoutePath {
                path: seen,
                rejection,
            } = error
            else {
                panic!("{path} must fail as a path rejection");
            };
            assert_eq!(seen, path);
            assert_eq!(rejection, expected, "{path}");
        }

        // The path frozen for the local page-presence topology, plus a
        // second literal: the seam carries a validated SET, so accepting one
        // path proves nothing about accepting two.
        for path in ["/api/v1/local/presence", "/api/v1/local/probe"] {
            let route = SecondaryWebSocketRoute::new(path, socket()).expect(path);
            assert_eq!(route.path(), path);
        }
    }

    #[tokio::test]
    async fn a_duplicate_path_in_the_route_set_is_an_error_rather_than_a_router_panic() {
        let extension = Arc::new(CountingExtension {
            calls: AtomicUsize::new(0),
        });
        let watch = Arc::new(CountingSocket::default());
        // `Router::route` panics on a duplicate path, so the set has to be
        // collapsed before registration; the listener must not exist yet
        // either, or the failure would leak a bound port.
        let error = spawn_loopback_server_with_sockets(
            extension,
            watch,
            vec![
                SecondaryWebSocketRoute::new(
                    "/api/v1/local/presence",
                    Arc::new(CountingSocket::default()),
                )
                .unwrap(),
                SecondaryWebSocketRoute::new(
                    "/api/v1/local/presence",
                    Arc::new(CountingSocket::default()),
                )
                .unwrap(),
            ],
            SessionNonce::generate(),
        )
        .await
        .err()
        .expect("a duplicate path must be refused");
        let LoopbackServerError::SecondaryRoutePath { path, rejection } = error else {
            panic!("a duplicate must fail as a path rejection");
        };
        assert_eq!(path, "/api/v1/local/presence");
        assert_eq!(rejection, SecondaryRoutePathRejection::Duplicate);
    }

    /// A multi-thread runtime, deliberately: these tests poll a counter that
    /// only a server task can advance, and on the single-threaded flavour a
    /// polling loop and the server compete for the one thread.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn each_route_in_the_set_reaches_its_own_socket() {
        let extension = Arc::new(CountingExtension {
            calls: AtomicUsize::new(0),
        });
        let watch = Arc::new(CountingSocket::default());
        let presence = Arc::new(RecordingSocket::default());
        let probe = Arc::new(RecordingSocket::default());
        let server = spawn_loopback_server_with_sockets(
            extension,
            watch,
            vec![
                SecondaryWebSocketRoute::new("/api/v1/local/presence", presence.clone()).unwrap(),
                SecondaryWebSocketRoute::new("/api/v1/local/probe", probe.clone()).unwrap(),
            ],
            SessionNonce::generate(),
        )
        .await
        .unwrap();
        let origin = server.origin().to_owned();
        let authority = origin.strip_prefix("http://").unwrap().to_owned();
        let address = authority.parse::<SocketAddr>().unwrap();
        let nonce = server.nonce().as_str().to_owned();

        let speak = move |path: &'static str, message: &'static str| {
            let (address, authority, origin, nonce) =
                (address, authority.clone(), origin.clone(), nonce.clone());
            tokio::task::spawn_blocking(move || {
                let mut stream = upgraded_websocket(
                    address,
                    path,
                    &authority,
                    &origin,
                    &nonce,
                    Duration::from_secs(5),
                )
                .unwrap_or_else(|| panic!("{path} upgrades"));
                stream
                    .write_all(&masked_text_frame(message.as_bytes()))
                    .unwrap();
                stream.flush().unwrap();
                stream
            })
        };

        let presence_stream = speak("/api/v1/local/presence", r#"{"type":"HELLO"}"#)
            .await
            .unwrap();
        await_count(&presence.texts, 1, "the presence socket receives its frame").await;
        assert_eq!(
            probe.connects.load(Ordering::SeqCst),
            0,
            "one path in the set must not be served by another path's socket",
        );

        let probe_stream = speak("/api/v1/local/probe", r#"{"type":"PROBE"}"#)
            .await
            .unwrap();
        await_count(&probe.texts, 1, "the second socket receives its own frame").await;
        assert_eq!(
            presence.texts.load(Ordering::SeqCst),
            1,
            "the second route's traffic must not reach the first route's socket",
        );

        drop((presence_stream, probe_stream));
        server.shutdown().await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_shared_ceiling_admits_a_full_size_frame_and_drops_the_next_byte() {
        let extension = Arc::new(CountingExtension {
            calls: AtomicUsize::new(0),
        });
        let watch = Arc::new(RecordingSocket::default());
        let server =
            spawn_loopback_server_with_socket(extension, watch.clone(), SessionNonce::generate())
                .await
                .unwrap();
        let origin = server.origin().to_owned();
        let authority = origin.strip_prefix("http://").unwrap().to_owned();
        let address = authority.parse::<SocketAddr>().unwrap();
        let nonce = server.nonce().as_str().to_owned();
        let connect = move || {
            upgraded_websocket(
                address,
                WATCH_SOCKET_PATH,
                &authority,
                &origin,
                &nonce,
                Duration::from_secs(5),
            )
            .expect("the watch route upgrades")
        };

        // Exactly at the ceiling is the positive control: it proves a later
        // "nothing arrived" means the cap fired rather than the pump being
        // broken, and it pins the boundary as inclusive.
        let at_ceiling = connect.clone();
        let admitted = tokio::task::spawn_blocking(move || {
            let mut stream = at_ceiling();
            let payload = vec![b'a'; MAX_WEBSOCKET_MESSAGE_BYTES];
            stream.write_all(&masked_text_frame(&payload)).unwrap();
            stream.flush().unwrap();
            stream
        })
        .await
        .unwrap();
        await_count(&watch.texts, 1, "a full-size frame reaches the extension").await;
        assert_eq!(
            watch.bytes.load(Ordering::SeqCst),
            MAX_WEBSOCKET_MESSAGE_BYTES,
            "the full-size payload arrives whole",
        );

        let closed = tokio::task::spawn_blocking(move || {
            let mut stream = connect();
            let payload = vec![b'b'; MAX_WEBSOCKET_MESSAGE_BYTES + 1];
            stream.write_all(&masked_text_frame(&payload)).unwrap();
            stream.flush().unwrap();
            closed_by_peer(&mut stream)
        })
        .await
        .unwrap();

        assert!(
            closed,
            "one byte past the shared ceiling must end the connection",
        );
        assert_eq!(
            watch.texts.load(Ordering::SeqCst),
            1,
            "the over-ceiling payload must never be handed to the extension",
        );
        drop(admitted);
        server.shutdown().await.unwrap();
    }

    /// The ceiling has to bound the reassembled message, not just one frame.
    /// Both fragments here sit far under the frame cap, so the frame cap
    /// cannot decide either case -- only the message cap can. Without it a
    /// caller could stream an unbounded message through the host one small
    /// fragment at a time.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_ceiling_bounds_a_reassembled_message_not_just_one_frame() {
        const HALF: usize = MAX_WEBSOCKET_MESSAGE_BYTES / 2;

        let extension = Arc::new(CountingExtension {
            calls: AtomicUsize::new(0),
        });
        let watch = Arc::new(RecordingSocket::default());
        let server =
            spawn_loopback_server_with_socket(extension, watch.clone(), SessionNonce::generate())
                .await
                .unwrap();
        let origin = server.origin().to_owned();
        let authority = origin.strip_prefix("http://").unwrap().to_owned();
        let address = authority.parse::<SocketAddr>().unwrap();
        let nonce = server.nonce().as_str().to_owned();
        // A text opener with FIN clear, then a continuation carrying FIN.
        let fragmented = move |tail: usize| {
            let (authority, origin, nonce) = (authority.clone(), origin.clone(), nonce.clone());
            tokio::task::spawn_blocking(move || {
                let mut stream = upgraded_websocket(
                    address,
                    WATCH_SOCKET_PATH,
                    &authority,
                    &origin,
                    &nonce,
                    Duration::from_secs(5),
                )
                .expect("the watch route upgrades");
                stream
                    .write_all(&masked_frame(0x01, &vec![b'a'; HALF]))
                    .unwrap();
                stream
                    .write_all(&masked_frame(0x80, &vec![b'b'; tail]))
                    .unwrap();
                stream.flush().unwrap();
                stream
            })
        };

        // Reassembles to exactly the ceiling: admitted as one message.
        let admitted = fragmented(HALF).await.unwrap();
        await_count(&watch.texts, 1, "a fragmented message is reassembled").await;
        assert_eq!(
            watch.bytes.load(Ordering::SeqCst),
            MAX_WEBSOCKET_MESSAGE_BYTES,
            "both fragments arrive as one whole message",
        );

        // One byte more, still two under-cap frames: refused on reassembly.
        let mut over = fragmented(HALF + 1).await.unwrap();
        let closed = tokio::task::spawn_blocking(move || closed_by_peer(&mut over))
            .await
            .unwrap();
        assert!(
            closed,
            "a reassembled message past the ceiling must end the connection",
        );
        assert_eq!(
            watch.texts.load(Ordering::SeqCst),
            1,
            "the over-ceiling message must never be handed to the extension",
        );
        assert_eq!(
            watch.bytes.load(Ordering::SeqCst),
            MAX_WEBSOCKET_MESSAGE_BYTES,
            "and none of its fragments may leak through either",
        );
        drop(admitted);
        server.shutdown().await.unwrap();
    }

    /// And the frame cap has to decide on the declared length, before the
    /// host buffers anything. This client announces a megabyte and sends a
    /// handful of bytes: with the frame cap the header alone is refused, and
    /// without it the host sits waiting for the rest. A megabyte, not
    /// something wilder, because axum's default frame cap is 16 MiB -- a
    /// larger claim would be refused by that default and prove nothing
    /// about the ceiling this host sets.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_frame_cap_refuses_a_declared_length_without_buffering_it() {
        let extension = Arc::new(CountingExtension {
            calls: AtomicUsize::new(0),
        });
        let watch = Arc::new(RecordingSocket::default());
        let server =
            spawn_loopback_server_with_socket(extension, watch.clone(), SessionNonce::generate())
                .await
                .unwrap();
        let origin = server.origin().to_owned();
        let authority = origin.strip_prefix("http://").unwrap().to_owned();
        let address = authority.parse::<SocketAddr>().unwrap();
        let nonce = server.nonce().as_str().to_owned();

        let closed = tokio::task::spawn_blocking(move || {
            let mut stream = upgraded_websocket(
                address,
                WATCH_SOCKET_PATH,
                &authority,
                &origin,
                &nonce,
                Duration::from_secs(5),
            )
            .expect("the watch route upgrades");
            let mut header = vec![0x81_u8, 0x80 | 127];
            header.extend_from_slice(&(1_u64 << 20).to_be_bytes());
            header.extend_from_slice(&[0x37, 0xfa, 0x21, 0x3d]);
            header.extend_from_slice(&[0x00; 8]);
            stream.write_all(&header).unwrap();
            stream.flush().unwrap();
            closed_by_peer(&mut stream)
        })
        .await
        .unwrap();

        assert!(
            closed,
            "a declared length past the frame cap must be refused on the header",
        );
        assert_eq!(
            watch.texts.load(Ordering::SeqCst),
            0,
            "nothing may reach the extension",
        );
        server.shutdown().await.unwrap();
    }

    /// Waits for a counter another task owns, without spinning the scheduler.
    async fn await_count(counter: &AtomicUsize, expected: usize, what: &str) {
        tokio::time::timeout(Duration::from_secs(10), async {
            while counter.load(Ordering::SeqCst) < expected {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("timed out waiting until {what}"));
    }
}
