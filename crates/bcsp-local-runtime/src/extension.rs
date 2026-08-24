use std::sync::Arc;

use bcsp_application::{
    ExtensionRequest, ExtensionResponse, RequestMethod, RouteExtension, SessionNonce,
};
use bcsp_contracts::{
    ApiErrorDetail, StableErrorCode, StableErrorCodeSet, SystemTraceIdSource, TraceIdSource,
    TypedApiErrorBody, TypedApiErrorEnvelope,
};
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use thiserror::Error;

static WEB_ASSETS: Dir<'_> = include_dir!("$OUT_DIR/web-assets");
static RUNTIME_TEXT: &[u8] = include_bytes!("../assets/shell/runtime.txt");

/// A response whose status the surface chose.
///
/// Most local routes answer 200 or fail, and `Result<Vec<u8>, _>` says that
/// exactly. The desired-watch write does not fit: its refusals are business
/// answers with a body worth reading, not transport errors, and which status
/// each one earns is part of the contract rather than an error mapping.
pub struct LocalSurfaceOutcome {
    pub status: u16,
    pub body: Vec<u8>,
}

pub trait LocalSurfaceState: Send + Sync + 'static {
    fn bootstrap(&self, nonce: &SessionNonce) -> Result<Vec<u8>, LocalSurfaceFailure>;
    /// The desired-watch authority read.
    fn desired_watch(&self) -> Result<Vec<u8>, LocalSurfaceFailure>;
    /// One desired-watch compare-and-swap.
    fn put_desired_watch(&self, body: &[u8])
    -> Result<LocalSurfaceOutcome, LocalSurfaceFailure>;
    fn settings(&self) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn put_settings(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn selection(&self) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn put_selection(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn history(&self) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn current_filters(&self) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn put_current_filters(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn saved_views(&self) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn create_saved_view(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn apply_saved_view(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn rename_saved_view(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn update_saved_view(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn duplicate_saved_view(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn delete_saved_view(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn delete_all_saved_views(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn reset_current_filters(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn prepare_local_user_data_reset(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn confirm_local_user_data_reset(&self, body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure>;
    fn pull_term(&self, _body: &[u8]) -> Result<Vec<u8>, LocalSurfaceFailure> {
        Err(LocalSurfaceFailure::bad_request(
            LocalApiErrorCode::MethodNotAllowed,
        ))
    }
    fn checkpoint_wal(&self) -> Result<(), LocalSurfaceFailure>;
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LocalApiErrorCode {
    MalformedRequest,
    UnsupportedProtocolVersion,
    MethodNotAllowed,
    SettingsRevisionConflict,
    UserStateRevisionConflict,
    CurrentFiltersRevisionConflict,
    SavedViewRevisionConflict,
    SavedViewNotFound,
    SavedViewNameConflict,
    InvalidSavedView,
    SavedViewIncompatible,
    StorageFull,
    ResetConfirmationRequired,
    ResetConfirmationInvalid,
    ResetConfirmationExpired,
    InvalidLocalState,
    TermOutOfRange,
    TermNotPublished,
    /// Local storage is momentarily unavailable -- another writer holds the
    /// database. Retryable, and deliberately not an internal error: nothing
    /// is wrong, and a caller that retries succeeds.
    StorageBusy,
    InternalError,
}

impl LocalApiErrorCode {
    pub const ALL: &'static [Self] = &[
        Self::MalformedRequest,
        Self::UnsupportedProtocolVersion,
        Self::MethodNotAllowed,
        Self::SettingsRevisionConflict,
        Self::UserStateRevisionConflict,
        Self::CurrentFiltersRevisionConflict,
        Self::SavedViewRevisionConflict,
        Self::SavedViewNotFound,
        Self::SavedViewNameConflict,
        Self::InvalidSavedView,
        Self::SavedViewIncompatible,
        Self::StorageFull,
        Self::ResetConfirmationRequired,
        Self::ResetConfirmationInvalid,
        Self::ResetConfirmationExpired,
        Self::InvalidLocalState,
        Self::TermOutOfRange,
        Self::TermNotPublished,
        Self::StorageBusy,
        Self::InternalError,
    ];

    pub const fn message_key(self) -> &'static str {
        match self {
            Self::MalformedRequest => "error.malformed_request",
            Self::UnsupportedProtocolVersion => "error.unsupported_protocol_version",
            Self::MethodNotAllowed => "local.error.method_not_allowed",
            Self::SettingsRevisionConflict => "local.error.settings_revision_conflict",
            Self::UserStateRevisionConflict => "local.error.user_state_revision_conflict",
            Self::CurrentFiltersRevisionConflict => "local.error.current_filters_revision_conflict",
            Self::SavedViewRevisionConflict => "local.error.saved_view_revision_conflict",
            Self::SavedViewNotFound => "local.error.saved_view_not_found",
            Self::SavedViewNameConflict => "local.error.saved_view_name_conflict",
            Self::InvalidSavedView => "local.error.invalid_saved_view",
            Self::SavedViewIncompatible => "local.error.saved_view_incompatible",
            Self::StorageFull => "local.error.storage_full",
            Self::ResetConfirmationRequired => "local.error.reset_confirmation_required",
            Self::ResetConfirmationInvalid => "local.error.reset_confirmation_invalid",
            Self::ResetConfirmationExpired => "local.error.reset_confirmation_expired",
            Self::InvalidLocalState => "local.error.invalid_state",
            Self::TermOutOfRange => "local.error.term_out_of_range",
            Self::TermNotPublished => "local.error.term_not_published",
            Self::StorageBusy => "local.error.storage_busy",
            Self::InternalError => "error.internal",
        }
    }

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::MalformedRequest => "MALFORMED_REQUEST",
            Self::UnsupportedProtocolVersion => "UNSUPPORTED_PROTOCOL_VERSION",
            Self::MethodNotAllowed => "METHOD_NOT_ALLOWED",
            Self::SettingsRevisionConflict => "SETTINGS_REVISION_CONFLICT",
            Self::UserStateRevisionConflict => "USER_STATE_REVISION_CONFLICT",
            Self::CurrentFiltersRevisionConflict => "CURRENT_FILTERS_REVISION_CONFLICT",
            Self::SavedViewRevisionConflict => "SAVED_VIEW_REVISION_CONFLICT",
            Self::SavedViewNotFound => "SAVED_VIEW_NOT_FOUND",
            Self::SavedViewNameConflict => "SAVED_VIEW_NAME_CONFLICT",
            Self::InvalidSavedView => "INVALID_SAVED_VIEW",
            Self::SavedViewIncompatible => "SAVED_VIEW_INCOMPATIBLE",
            Self::StorageFull => "STORAGE_FULL",
            Self::ResetConfirmationRequired => "RESET_CONFIRMATION_REQUIRED",
            Self::ResetConfirmationInvalid => "RESET_CONFIRMATION_INVALID",
            Self::ResetConfirmationExpired => "RESET_CONFIRMATION_EXPIRED",
            Self::InvalidLocalState => "INVALID_LOCAL_STATE",
            Self::TermOutOfRange => "TERM_OUT_OF_RANGE",
            Self::TermNotPublished => "TERM_NOT_PUBLISHED",
            Self::StorageBusy => "STORAGE_BUSY",
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
    current_revision: Option<u64>,
}

impl LocalSurfaceFailure {
    pub const fn bad_request(code: LocalApiErrorCode) -> Self {
        Self {
            status: 400,
            code,
            current_revision: None,
        }
    }

    pub const fn conflict(code: LocalApiErrorCode) -> Self {
        Self {
            status: 409,
            code,
            current_revision: None,
        }
    }

    pub const fn revision_conflict(code: LocalApiErrorCode, current_revision: u64) -> Self {
        Self {
            status: 409,
            code,
            current_revision: Some(current_revision),
        }
    }

    pub const fn not_found(code: LocalApiErrorCode) -> Self {
        Self {
            status: 404,
            code,
            current_revision: None,
        }
    }

    pub const fn unprocessable(code: LocalApiErrorCode) -> Self {
        Self {
            status: 422,
            code,
            current_revision: None,
        }
    }

    pub const fn insufficient_storage(code: LocalApiErrorCode) -> Self {
        Self {
            status: 507,
            code,
            current_revision: None,
        }
    }

    pub const fn service_unavailable(code: LocalApiErrorCode) -> Self {
        Self {
            status: 503,
            code,
            current_revision: None,
        }
    }

    pub const fn internal(code: LocalApiErrorCode) -> Self {
        Self {
            status: 500,
            code,
            current_revision: None,
        }
    }
}

pub struct LocalRouteExtension {
    nonce: SessionNonce,
    surface: Box<dyn LocalSurfaceState>,
    request_exit: Box<dyn Fn() + Send + Sync>,
    product_routes: Arc<dyn RouteExtension>,
}

impl LocalRouteExtension {
    pub fn new(
        nonce: SessionNonce,
        surface: Box<dyn LocalSurfaceState>,
        request_exit: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        Self::with_product_routes(nonce, surface, request_exit, Arc::new(NoLocalProductRoutes))
    }

    pub fn with_product_routes(
        nonce: SessionNonce,
        surface: Box<dyn LocalSurfaceState>,
        request_exit: impl Fn() + Send + Sync + 'static,
        product_routes: Arc<dyn RouteExtension>,
    ) -> Self {
        Self {
            nonce,
            surface,
            request_exit: Box::new(request_exit),
            product_routes,
        }
    }

    pub fn checkpoint_wal(&self) -> Result<(), LocalSurfaceFailure> {
        self.surface.checkpoint_wal()
    }

    fn surface_response(
        &self,
        operation: impl FnOnce(&dyn LocalSurfaceState) -> Result<Vec<u8>, LocalSurfaceFailure>,
    ) -> ExtensionResponse {
        match operation(self.surface.as_ref()) {
            Ok(body) => ExtensionResponse::json_bytes(200, body),
            Err(error) => failure_response(error),
        }
    }
}

impl RouteExtension for LocalRouteExtension {
    fn handle(&self, request: ExtensionRequest) -> ExtensionResponse {
        match (request.method(), request.path()) {
            (RequestMethod::Get, "/") => web_asset("local.html"),
            (RequestMethod::Get, path) if is_local_spa_shell_path(path) => web_asset("local.html"),
            (RequestMethod::Get, "/runtime.txt") => {
                ExtensionResponse::text(200, RUNTIME_TEXT.to_vec())
            }
            (RequestMethod::Get, path) if is_static_web_asset_path(path) => static_web_asset(path),
            (RequestMethod::Get, "/api/v1/local/bootstrap") => {
                self.surface_response(|surface| surface.bootstrap(&self.nonce))
            }
            (RequestMethod::Get, crate::LOCAL_DESIRED_WATCH_PATH) => {
                self.surface_response(|surface| surface.desired_watch())
            }
            (RequestMethod::Put, crate::LOCAL_DESIRED_WATCH_PATH) => {
                match self.surface.put_desired_watch(request.body()) {
                    Ok(outcome) => ExtensionResponse::json_bytes(outcome.status, outcome.body),
                    Err(error) => failure_response(error),
                }
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
            (RequestMethod::Get, "/api/v1/local/current-filters") => {
                self.surface_response(|surface| surface.current_filters())
            }
            (RequestMethod::Put, "/api/v1/local/current-filters") => {
                self.surface_response(|surface| surface.put_current_filters(request.body()))
            }
            (RequestMethod::Get, "/api/v1/local/saved-views") => {
                self.surface_response(|surface| surface.saved_views())
            }
            (RequestMethod::Post, "/api/v1/local/saved-views") => {
                self.surface_response(|surface| surface.create_saved_view(request.body()))
            }
            (RequestMethod::Post, "/api/v1/local/saved-views/apply") => {
                self.surface_response(|surface| surface.apply_saved_view(request.body()))
            }
            (RequestMethod::Post, "/api/v1/local/saved-views/rename") => {
                self.surface_response(|surface| surface.rename_saved_view(request.body()))
            }
            (RequestMethod::Post, "/api/v1/local/saved-views/update") => {
                self.surface_response(|surface| surface.update_saved_view(request.body()))
            }
            (RequestMethod::Post, "/api/v1/local/saved-views/duplicate") => {
                self.surface_response(|surface| surface.duplicate_saved_view(request.body()))
            }
            (RequestMethod::Post, "/api/v1/local/saved-views/delete") => {
                self.surface_response(|surface| surface.delete_saved_view(request.body()))
            }
            (RequestMethod::Post, "/api/v1/local/saved-views/delete-all") => {
                self.surface_response(|surface| surface.delete_all_saved_views(request.body()))
            }
            (RequestMethod::Post, "/api/v1/local/filters/reset") => {
                self.surface_response(|surface| surface.reset_current_filters(request.body()))
            }
            (RequestMethod::Post, "/api/v1/local/user-data-reset/prepare") => self
                .surface_response(|surface| surface.prepare_local_user_data_reset(request.body())),
            (RequestMethod::Post, "/api/v1/local/user-data-reset/confirm") => self
                .surface_response(|surface| surface.confirm_local_user_data_reset(request.body())),
            (RequestMethod::Post, "/api/v1/local/terms/pull") => {
                self.surface_response(|surface| surface.pull_term(request.body()))
            }
            (RequestMethod::Post, "/api/v1/local/exit") => {
                (self.request_exit)();
                ExtensionResponse::no_content()
            }
            (_, "/")
            | (_, "/runtime.txt")
            | (_, "/api/v1/local/bootstrap")
            | (_, "/api/v1/local/settings")
            | (_, crate::LOCAL_DESIRED_WATCH_PATH)
            | (_, "/api/v1/local/selection")
            | (_, "/api/v1/local/history")
            | (_, "/api/v1/local/current-filters")
            | (_, "/api/v1/local/saved-views")
            | (_, "/api/v1/local/saved-views/apply")
            | (_, "/api/v1/local/saved-views/rename")
            | (_, "/api/v1/local/saved-views/update")
            | (_, "/api/v1/local/saved-views/duplicate")
            | (_, "/api/v1/local/saved-views/delete")
            | (_, "/api/v1/local/saved-views/delete-all")
            | (_, "/api/v1/local/filters/reset")
            | (_, "/api/v1/local/user-data-reset/prepare")
            | (_, "/api/v1/local/user-data-reset/confirm")
            | (_, "/api/v1/local/terms/pull")
            | (_, "/api/v1/local/exit") => method_not_allowed(),
            _ => self.product_routes.handle(request),
        }
    }
}

fn is_local_spa_shell_path(path: &str) -> bool {
    if matches!(
        path,
        "/history" | "/saved-views" | "/sections" | "/settings" | "/watch"
    ) {
        return true;
    }

    let Some(identity) = path.strip_prefix("/sections/") else {
        return false;
    };
    let mut segments = identity.split('/');
    let Some(term) = segments.next() else {
        return false;
    };
    let Some(campus) = segments.next() else {
        return false;
    };
    let Some(index) = segments.next() else {
        return false;
    };

    segments.next().is_none()
        && [term, campus, index]
            .into_iter()
            .all(is_safe_spa_route_segment)
}

fn is_safe_spa_route_segment(segment: &str) -> bool {
    const MAX_ROUTE_SEGMENT_BYTES: usize = 64;

    !segment.is_empty()
        && segment.len() <= MAX_ROUTE_SEGMENT_BYTES
        && segment
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[derive(Clone, Copy, Debug, Default)]
struct NoLocalProductRoutes;

impl RouteExtension for NoLocalProductRoutes {
    fn handle(&self, _request: ExtensionRequest) -> ExtensionResponse {
        ExtensionResponse::not_found()
    }
}

fn is_static_web_asset_path(path: &str) -> bool {
    matches!(
        path,
        "/local.html"
            | "/asset-manifest.json"
            | "/capability-manifest.json"
            | "/module-manifest.json"
            | "/runtime.txt"
    ) || path.starts_with("/assets/")
}

fn static_web_asset(path: &str) -> ExtensionResponse {
    let Some(name) = safe_asset_name(path) else {
        return ExtensionResponse::not_found();
    };
    web_asset(name)
}

fn safe_asset_name(path: &str) -> Option<&str> {
    let name = path.strip_prefix('/')?;
    if name.is_empty() || name.contains('\\') || name.contains('\0') {
        return None;
    }
    name.split('/')
        .all(|segment| !segment.is_empty() && !matches!(segment, "." | ".."))
        .then_some(name)
}

fn web_asset(name: &str) -> ExtensionResponse {
    let Some(file) = WEB_ASSETS.get_file(name) else {
        return ExtensionResponse::not_found();
    };
    ExtensionResponse::bytes(200, asset_content_type(name), file.contents().to_vec())
}

fn asset_content_type(name: &str) -> &'static str {
    match name.rsplit_once('.').map(|(_, extension)| extension) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "application/javascript; charset=utf-8",
        Some("json" | "map") => "application/json; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("wasm") => "application/wasm",
        Some("txt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn method_not_allowed() -> ExtensionResponse {
    error_response(405, LocalApiErrorCode::MethodNotAllowed, None)
}

fn failure_response(error: LocalSurfaceFailure) -> ExtensionResponse {
    error_response(error.status, error.code, error.current_revision)
}

fn error_response(
    status: u16,
    code: LocalApiErrorCode,
    current_revision: Option<u64>,
) -> ExtensionResponse {
    let mut source = SystemTraceIdSource;
    let details = current_revision
        .map(|revision| vec![ApiErrorDetail::CurrentRevision { revision }])
        .unwrap_or_default();
    let body = TypedApiErrorBody::new(code, source.next_trace_id()).with_details(details);
    let envelope = TypedApiErrorEnvelope::new(body);
    let body = serde_json::to_vec(&envelope).unwrap_or_else(|_| {
        br#"{"protocolVersion":1,"error":{"code":"INTERNAL_ERROR","messageKey":"error.internal","traceId":"00000000-0000-4000-8000-000000000001","details":[]}}"#.to_vec()
    });
    ExtensionResponse::json_bytes(status, body)
}
