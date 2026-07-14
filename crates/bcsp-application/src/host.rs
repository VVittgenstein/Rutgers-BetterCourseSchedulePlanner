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
    X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
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
const SESSION_HEADER: &str = "x-bcsp-session";
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
    pub fn json_bytes(status: u16, body: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            content_type: "application/json; charset=utf-8",
            body: body.into(),
        }
    }

    pub fn html(status: u16, value: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            content_type: "text/html; charset=utf-8",
            body: value.into(),
        }
    }

    pub fn text(status: u16, value: impl Into<Vec<u8>>) -> Self {
        Self {
            status,
            content_type: "text/plain; charset=utf-8",
            body: value.into(),
        }
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
            task.await
                .map_err(LoopbackServerError::Join)?
                .map_err(LoopbackServerError::Serve)?;
        }
        Ok(())
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
}

pub async fn spawn_loopback_server(
    extension: Arc<dyn RouteExtension>,
    nonce: SessionNonce,
) -> Result<LoopbackServer, LoopbackServerError> {
    spawn_loopback_server_internal(extension, None, nonce).await
}

pub async fn spawn_loopback_server_with_socket(
    extension: Arc<dyn RouteExtension>,
    socket: Arc<dyn WebSocketExtension>,
    nonce: SessionNonce,
) -> Result<LoopbackServer, LoopbackServerError> {
    spawn_loopback_server_internal(extension, Some(socket), nonce).await
}

async fn spawn_loopback_server_internal(
    extension: Arc<dyn RouteExtension>,
    socket: Option<Arc<dyn WebSocketExtension>>,
    nonce: SessionNonce,
) -> Result<LoopbackServer, LoopbackServerError> {
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))
        .await
        .map_err(LoopbackServerError::Bind)?;
    let address = listener.local_addr().map_err(LoopbackServerError::Bind)?;
    let authority = format!("127.0.0.1:{}", address.port());
    let origin = format!("http://{authority}");
    let socket_maintenance = socket.as_ref().map(|socket| {
        let socket = Arc::clone(socket);
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(SOCKET_PRODUCT_TICK_INTERVAL);
            tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tick.tick().await;
                socket.tick();
            }
        })
    });
    let state = HostState {
        authority,
        origin: origin.clone(),
        nonce: nonce.clone(),
        extension,
        socket,
    };
    let router = Router::new()
        .route("/api/v1/watch", get(handle_watch_socket))
        .fallback(any(handle_extension))
        .with_state(state);
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

async fn handle_watch_socket(
    State(state): State<HostState>,
    upgrade: WebSocketUpgrade,
    request: Request,
) -> Response {
    if header_text(request.headers(), HOST).as_deref() != Some(state.authority.as_str())
        || header_text(request.headers(), ORIGIN).as_deref() != Some(state.origin.as_str())
        || session_query(request.uri().query()) != Some(state.nonce.as_str())
    {
        return api_error_response(StatusCode::FORBIDDEN, ApiErrorCode::MalformedRequest);
    }
    let Some(extension) = state.socket else {
        return extension_response(ExtensionResponse::not_found());
    };
    let mut source = SystemTraceIdSource;
    let connection_id = source.next_trace_id();
    upgrade.on_upgrade(move |socket| serve_websocket(socket, extension, connection_id))
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
    if !extension.connect(connection_id, outbound) {
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
                        extension.transport_activity(connection_id);
                        extension.receive_text(connection_id, message.as_str());
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        last_seen = tokio::time::Instant::now();
                        extension.transport_activity(connection_id);
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Some(Ok(Message::Pong(_))) => {
                        last_seen = tokio::time::Instant::now();
                        extension.transport_activity(connection_id);
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
    extension.disconnect(connection_id);
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

async fn handle_extension(State(state): State<HostState>, request: Request) -> Response {
    if header_text(request.headers(), HOST).as_deref() != Some(state.authority.as_str()) {
        return api_error_response(
            StatusCode::MISDIRECTED_REQUEST,
            ApiErrorCode::MalformedRequest,
        );
    }

    let method = RequestMethod::from_http(request.method());
    if method.changes_state()
        && (header_text(request.headers(), ORIGIN).as_deref() != Some(state.origin.as_str())
            || header_text_name(request.headers(), SESSION_HEADER).as_deref()
                != Some(state.nonce.as_str()))
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
    extension_response(state.extension.handle(ExtensionRequest {
        method,
        path,
        query,
        body,
    }))
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
            "default-src 'self'; base-uri 'none'; frame-ancestors 'none'; object-src 'none'",
        ),
    );
    response
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use axum::http::Request as HttpRequest;
    use tower::ServiceExt;

    use super::*;

    struct CountingExtension {
        calls: AtomicUsize,
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
            })
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
}
