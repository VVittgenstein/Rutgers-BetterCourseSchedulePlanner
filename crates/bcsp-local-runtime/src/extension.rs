use std::sync::Mutex;

use bcsp_application::{
    ExtensionRequest, ExtensionResponse, RequestMethod, RouteExtension, SessionNonce,
};
use bcsp_contracts::{
    StableErrorCode, StableErrorCodeSet, SystemTraceIdSource, TraceIdSource, TypedApiErrorBody,
    TypedApiErrorEnvelope,
};
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use thiserror::Error;

static SHELL_ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/shell");

pub trait LocalSurfaceState: Send + 'static {
    fn bootstrap(&mut self, nonce: &SessionNonce) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn settings(&mut self) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn put_settings(&mut self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn selection(&mut self) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn put_selection(&mut self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn history(&mut self) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn checkpoint_wal(&mut self) -> Result<(), LocalSurfaceFailure>;
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocalApiErrorCode {
    MalformedRequest,
    UnsupportedProtocolVersion,
    MethodNotAllowed,
    SettingsRevisionConflict,
    InvalidLocalState,
    InternalError,
}

impl LocalApiErrorCode {
    pub const ALL: &'static [Self] = &[
        Self::MalformedRequest,
        Self::UnsupportedProtocolVersion,
        Self::MethodNotAllowed,
        Self::SettingsRevisionConflict,
        Self::InvalidLocalState,
        Self::InternalError,
    ];

    pub const fn message_key(self) -> &'static str {
        match self {
            Self::MalformedRequest => "error.malformed_request",
            Self::UnsupportedProtocolVersion => "error.unsupported_protocol_version",
            Self::MethodNotAllowed => "local.error.method_not_allowed",
            Self::SettingsRevisionConflict => "local.error.settings_revision_conflict",
            Self::InvalidLocalState => "local.error.invalid_state",
            Self::InternalError => "error.internal",
        }
    }

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::MalformedRequest => "MALFORMED_REQUEST",
            Self::UnsupportedProtocolVersion => "UNSUPPORTED_PROTOCOL_VERSION",
            Self::MethodNotAllowed => "METHOD_NOT_ALLOWED",
            Self::SettingsRevisionConflict => "SETTINGS_REVISION_CONFLICT",
            Self::InvalidLocalState => "INVALID_LOCAL_STATE",
            Self::InternalError => "INTERNAL_ERROR",
        }
    }
}

impl StableErrorCode for LocalApiErrorCode {
    fn message_key(self) -> &'static str {
        Self::message_key(self)
    }

    fn wire_name(self) -> &'static str {
        Self::wire_name(self)
    }
}

impl StableErrorCodeSet for LocalApiErrorCode {
    fn all() -> &'static [Self] {
        Self::ALL
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("local surface failed with HTTP {status}: {code:?}")]
pub struct LocalSurfaceFailure {
    status: u16,
    code: LocalApiErrorCode,
}

impl LocalSurfaceFailure {
    pub const fn bad_request(code: LocalApiErrorCode) -> Self {
        Self { status: 400, code }
    }

    pub const fn conflict(code: LocalApiErrorCode) -> Self {
        Self { status: 409, code }
    }

    pub const fn internal(code: LocalApiErrorCode) -> Self {
        Self { status: 500, code }
    }
}

pub struct LocalRouteExtension {
    nonce: SessionNonce,
    surface: Mutex<Box<dyn LocalSurfaceState>>,
    request_exit: Box<dyn Fn() + Send + Sync>,
}

impl LocalRouteExtension {
    pub fn new(
        nonce: SessionNonce,
        surface: Box<dyn LocalSurfaceState>,
        request_exit: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self {
            nonce,
            surface: Mutex::new(surface),
            request_exit: Box::new(request_exit),
        }
    }

    pub fn checkpoint_wal(&self) -> Result<(), LocalSurfaceFailure> {
        self.surface
            .lock()
            .map_err(|_| LocalSurfaceFailure::internal(LocalApiErrorCode::InternalError))?
            .checkpoint_wal()
    }

    fn surface_response(
        &self,
        operation: impl FnOnce(&mut dyn LocalSurfaceState) -> Result<Vec<u8>, LocalSurfaceFailure>,
    ) -> ExtensionResponse {
        let mut surface = match self.surface.lock() {
            Ok(surface) => surface,
            Err(_) => {
                return failure_response(LocalSurfaceFailure::internal(
                    LocalApiErrorCode::InternalError,
                ));
            }
        };
        match operation(surface.as_mut()) {
            Ok(body) => ExtensionResponse::json_bytes(200, body),
            Err(error) => failure_response(error),
        }
    }
}

impl RouteExtension for LocalRouteExtension {
    fn handle(&self, request: ExtensionRequest) -> ExtensionResponse {
        match (request.method(), request.path()) {
            (RequestMethod::Get, "/") => shell_asset("index.html", true),
            (RequestMethod::Get, "/runtime.txt") => shell_asset("runtime.txt", false),
            (RequestMethod::Get, "/api/v1/local/bootstrap") => {
                self.surface_response(|surface| surface.bootstrap(&self.nonce))
            }
            (RequestMethod::Get, "/api/v1/local/settings") => {
                self.surface_response(|surface| surface.settings())
            }
            (RequestMethod::Put, "/api/v1/local/settings") => {
                self.surface_response(|surface| surface.put_settings(request.body()))
            }
            (RequestMethod::Get, "/api/v1/local/selection") => {
                self.surface_response(|surface| surface.selection())
            }
            (RequestMethod::Put, "/api/v1/local/selection") => {
                self.surface_response(|surface| surface.put_selection(request.body()))
            }
            (RequestMethod::Get, "/api/v1/local/history") => {
                self.surface_response(|surface| surface.history())
            }
            (RequestMethod::Post, "/api/v1/local/exit") => {
                (self.request_exit)();
                ExtensionResponse::no_content()
            }
            (_, "/")
            | (_, "/runtime.txt")
            | (_, "/api/v1/local/bootstrap")
            | (_, "/api/v1/local/settings")
            | (_, "/api/v1/local/selection")
            | (_, "/api/v1/local/history")
            | (_, "/api/v1/local/exit") => method_not_allowed(),
            _ => ExtensionResponse::not_found(),
        }
    }
}

fn shell_asset(name: &str, html: bool) -> ExtensionResponse {
    let Some(file) = SHELL_ASSETS.get_file(name) else {
        return ExtensionResponse::not_found();
    };
    if html {
        ExtensionResponse::html(200, file.contents().to_vec())
    } else {
        ExtensionResponse::text(200, file.contents().to_vec())
    }
}

fn method_not_allowed() -> ExtensionResponse {
    error_response(405, LocalApiErrorCode::MethodNotAllowed)
}

fn failure_response(error: LocalSurfaceFailure) -> ExtensionResponse {
    error_response(error.status, error.code)
}

fn error_response(status: u16, code: LocalApiErrorCode) -> ExtensionResponse {
    let mut source = SystemTraceIdSource;
    let envelope = TypedApiErrorEnvelope::new(TypedApiErrorBody::new(code, source.next_trace_id()));
    let body = serde_json::to_vec(&envelope).unwrap_or_else(|_| {
        br#"{"protocolVersion":1,"error":{"code":"INTERNAL_ERROR","messageKey":"error.internal","traceId":"00000000-0000-4000-8000-000000000001","details":[]}}"#.to_vec()
    });
    ExtensionResponse::json_bytes(status, body)
}
