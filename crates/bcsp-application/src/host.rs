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
const LOOPBACK_SHUTDOWN_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);
const SESSION_HEADER: &str = "x-bcsp-session";
pub const SHARED_WATCH_SUBPROTOCOL: &str = "bcsp.v1";
const SOCKET_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
const SOCKET_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(60);
const SOCKET_PRODUCT_TICK_INTERVAL: Duration = Duration::from_millis(250);

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
    secondary_socket: Option<Arc<dyn WebSocketExtension>>,
}

/// A target-injected additional WebSocket route. The shared host registers it
/// only when the target supplies one at spawn time; without injection the
/// path does not exist in the route table and falls through to the extension
/// fallback (404). Admission (Host/Origin/session nonce/subprotocol) is the
/// host's standard WebSocket policy, identical to the built-in watch route.
pub struct SecondaryWebSocketRoute {
    path: &'static str,
    socket: Arc<dyn WebSocketExtension>,
}

impl SecondaryWebSocketRoute {
    pub fn new(path: &'static str, socket: Arc<dyn WebSocketExtension>) -> Self {
        Self { path, socket }
    }
}

pub async fn spawn_loopback_server(
    extension: Arc<dyn RouteExtension>,
    nonce: SessionNonce,
) -> Result<LoopbackServer, LoopbackServerError> {
    spawn_loopback_server_internal(extension, None, None, nonce).await
}

pub async fn spawn_loopback_server_with_socket(
    extension: Arc<dyn RouteExtension>,
    socket: Arc<dyn WebSocketExtension>,
    nonce: SessionNonce,
) -> Result<LoopbackServer, LoopbackServerError> {
    spawn_loopback_server_internal(extension, Some(socket), None, nonce).await
}

pub async fn spawn_loopback_server_with_sockets(
    extension: Arc<dyn RouteExtension>,
    socket: Arc<dyn WebSocketExtension>,
    secondary: SecondaryWebSocketRoute,
    nonce: SessionNonce,
) -> Result<LoopbackServer, LoopbackServerError> {
    spawn_loopback_server_internal(extension, Some(socket), Some(secondary), nonce).await
}

async fn spawn_loopback_server_internal(
    extension: Arc<dyn RouteExtension>,
    socket: Option<Arc<dyn WebSocketExtension>>,
    secondary: Option<SecondaryWebSocketRoute>,
    nonce: SessionNonce,
) -> Result<LoopbackServer, LoopbackServerError> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(LoopbackServerError::Bind)?;
    let address = listener.local_addr().map_err(LoopbackServerError::Bind)?;
    let authority = format!("127.0.0.1:{}", address.port());
    let origin = format!("http://{authority}");
    let (secondary_path, secondary_socket) = match secondary {
        Some(route) => (Some(route.path), Some(route.socket)),
        None => (None, None),
    };
    // One maintenance task drives every injected socket extension on the
    // shared cadence; a socket with the default no-op tick costs nothing.
    let tick_sockets: Vec<Arc<dyn WebSocketExtension>> = socket
        .iter()
        .chain(secondary_socket.iter())
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
        secondary_socket,
    };
    let mut router = Router::new().route("/api/v1/watch", get(handle_watch_socket));
    if let Some(path) = secondary_path {
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
    upgrade
        .protocols([SHARED_WATCH_SUBPROTOCOL])
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
    let Some(extension) = state.secondary_socket else {
        // Unreachable while the route is only registered alongside the
        // injected socket; kept as a fail-closed guard rather than a panic.
        return extension_response(ExtensionResponse::not_found());
    };
    let mut source = SystemTraceIdSource;
    let connection_id = source.next_trace_id();
    upgrade
        .protocols([SHARED_WATCH_SUBPROTOCOL])
        .on_upgrade(move |socket| serve_websocket(socket, extension, connection_id))
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
                secondary_socket: None,
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
        let Ok(mut stream) = TcpStream::connect_timeout(&address, timeout) else {
            return false;
        };
        if stream.set_read_timeout(Some(timeout)).is_err()
            || stream.set_write_timeout(Some(timeout)).is_err()
        {
            return false;
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
            return false;
        }
        let mut response = Vec::new();
        let mut buffer = [0_u8; 512];
        while !response.windows(4).any(|window| window == b"\r\n\r\n") && response.len() <= 8 * 1024
        {
            let Ok(read) = stream.read(&mut buffer) else {
                return false;
            };
            if read == 0 {
                return false;
            }
            response.extend_from_slice(&buffer[..read]);
        }
        response.starts_with(b"HTTP/1.1 101 ")
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
            SecondaryWebSocketRoute::new("/api/v1/second-socket", secondary),
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
            SecondaryWebSocketRoute::new("/api/v1/second-socket", secondary.clone()),
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
}
