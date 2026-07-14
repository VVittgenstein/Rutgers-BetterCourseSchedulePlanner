use std::collections::BTreeMap;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex, MutexGuard};

use bcsp_contracts::{
    ApiErrorBody, ApiErrorCode, ApiErrorEnvelope, CatalogDiscoveryRequestV1, CourseDetailRequestV1,
    CourseQueryRequestV1, HttpRequestEnvelope, HttpSuccessEnvelope, NormalizedFilterValuesV1,
    OpenSectionStatusRequestV1, OpenStatusRequestV1, SectionDetailRequestV1, SectionQueryRequestV1,
    SystemTraceIdSource, TermCampusKey, TraceIdSource, decode_versioned_envelope_json,
};
use bcsp_operational_storage::OperationalStorage;
use bcsp_query::QueryError;
use serde::Serialize;

use crate::{
    ApplicationClock, ExtensionRequest, ExtensionResponse, ExtensionRoute, OpenRuntimeSnapshot,
    OpenRuntimeSnapshotRegistry, OpenRuntimeSnapshotRegistryError, RefreshPolicyProvider,
    RequestMethod, RouteExtension, SharedQueryError, SharedRuntimeContext, SharedRuntimeError,
    TargetRefreshDemand, TargetRefreshDemandError,
};

pub const PRODUCT_FILTER_SCHEMA_PATH: &str = "/api/v1/query/filter-schema";
pub const PRODUCT_CATALOG_DISCOVERY_PATH: &str = "/api/v1/catalog/discovery";
pub const PRODUCT_COURSE_SEARCH_PATH: &str = "/api/v1/query/courses";
pub const PRODUCT_SECTION_SEARCH_PATH: &str = "/api/v1/query/sections";
pub const PRODUCT_COURSE_DETAIL_PATH: &str = "/api/v1/query/course-detail";
pub const PRODUCT_SECTION_DETAIL_PATH: &str = "/api/v1/query/section-detail";
pub const PRODUCT_OPEN_STATUS_PATH: &str = "/api/v1/open/status";
pub const PRODUCT_OPEN_SECTION_STATUS_PATH: &str = "/api/v1/open/section-status";

pub static SHARED_PRODUCT_ROUTE_INVENTORY: &[ExtensionRoute] = &[
    ExtensionRoute::new(RequestMethod::Get, PRODUCT_FILTER_SCHEMA_PATH),
    ExtensionRoute::new(RequestMethod::Post, PRODUCT_CATALOG_DISCOVERY_PATH),
    ExtensionRoute::new(RequestMethod::Post, PRODUCT_COURSE_SEARCH_PATH),
    ExtensionRoute::new(RequestMethod::Post, PRODUCT_SECTION_SEARCH_PATH),
    ExtensionRoute::new(RequestMethod::Post, PRODUCT_COURSE_DETAIL_PATH),
    ExtensionRoute::new(RequestMethod::Post, PRODUCT_SECTION_DETAIL_PATH),
    ExtensionRoute::new(RequestMethod::Post, PRODUCT_OPEN_STATUS_PATH),
    ExtensionRoute::new(RequestMethod::Post, PRODUCT_OPEN_SECTION_STATUS_PATH),
];

pub type SharedProductStorage = Arc<Mutex<OperationalStorage>>;

/// Target-owned access to the operational database used by shared product
/// routes. Adapters keep their existing database wrapper and mutex while the
/// shared route implementation borrows only the operational storage surface.
pub trait ProductStorageAccess: Send + Sync + 'static {
    type Guard<'a>: DerefMut<Target = OperationalStorage>
    where
        Self: 'a;

    fn lock_operational(&self) -> Result<Self::Guard<'_>, ProductStorageLockError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductStorageLockError;

impl ProductStorageAccess for SharedProductStorage {
    type Guard<'a> = MutexGuard<'a, OperationalStorage>;

    fn lock_operational(&self) -> Result<Self::Guard<'_>, ProductStorageLockError> {
        self.lock().map_err(|_| ProductStorageLockError)
    }
}

/// Target-neutral HTTP product surface backed by the shared runtime services.
///
/// Hosts retain authority over Origin/session admission and static documents.
/// This extension owns only typed product-route decoding, service composition,
/// and typed response envelopes.
pub struct SharedProductRoutes<C, P, S = SharedProductStorage> {
    storage: S,
    runtime: SharedRuntimeContext<C, P>,
    open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
    target_refresh_demand: Option<TargetRefreshDemand>,
}

impl<C, P> SharedProductRoutes<C, P>
where
    C: ApplicationClock,
    P: RefreshPolicyProvider,
{
    pub const fn new(
        storage: SharedProductStorage,
        runtime: SharedRuntimeContext<C, P>,
        open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
    ) -> Self {
        Self {
            storage,
            runtime,
            open_runtime,
            target_refresh_demand: None,
        }
    }

    pub fn storage(&self) -> SharedProductStorage {
        Arc::clone(&self.storage)
    }
}

impl<C, P, S> SharedProductRoutes<C, P, S>
where
    C: ApplicationClock,
    P: RefreshPolicyProvider,
    S: ProductStorageAccess,
{
    pub const fn with_storage_access(
        storage: S,
        runtime: SharedRuntimeContext<C, P>,
        open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
    ) -> Self {
        Self {
            storage,
            runtime,
            open_runtime,
            target_refresh_demand: None,
        }
    }

    pub fn with_target_refresh_demand(mut self, demand: TargetRefreshDemand) -> Self {
        self.target_refresh_demand = Some(demand);
        self
    }

    pub const fn storage_access(&self) -> &S {
        &self.storage
    }

    pub fn open_runtime(&self) -> Arc<OpenRuntimeSnapshotRegistry> {
        Arc::clone(&self.open_runtime)
    }

    fn filter_schema(&self) -> ExtensionResponse {
        success_response(self.runtime.filter_schema())
    }

    fn catalog_discovery(&self, request: &ExtensionRequest) -> ExtensionResponse {
        let _: CatalogDiscoveryRequestV1 = match decode_payload(request.body()) {
            Ok(request) => request,
            Err(response) => return response,
        };
        self.with_storage(|storage| {
            self.runtime
                .catalog_discovery(storage)
                .map_err(ProductRouteFailure::Runtime)
        })
    }

    fn course_search(&self, request: &ExtensionRequest) -> ExtensionResponse {
        let request: CourseQueryRequestV1 = match decode_payload(request.body()) {
            Ok(request) => request,
            Err(response) => return response,
        };
        self.with_storage(|storage| {
            let targets = self.search_targets(storage, request.filters.values())?;
            self.request_target_refresh(&targets)?;
            let snapshots = self.snapshots(&targets)?;
            self.runtime
                .course_search(storage, &targets, &request, |target| {
                    runtime_for(&snapshots, target)
                })
                .map_err(ProductRouteFailure::Runtime)
        })
    }

    fn section_search(&self, request: &ExtensionRequest) -> ExtensionResponse {
        let request: SectionQueryRequestV1 = match decode_payload(request.body()) {
            Ok(request) => request,
            Err(response) => return response,
        };
        self.with_storage(|storage| {
            let targets = self.search_targets(storage, request.filters.values())?;
            self.request_target_refresh(&targets)?;
            let snapshots = self.snapshots(&targets)?;
            self.runtime
                .section_search(storage, &targets, &request, |target| {
                    runtime_for(&snapshots, target)
                })
                .map_err(ProductRouteFailure::Runtime)
        })
    }

    fn course_detail(&self, request: &ExtensionRequest) -> ExtensionResponse {
        let request: CourseDetailRequestV1 = match decode_payload(request.body()) {
            Ok(request) => request,
            Err(response) => return response,
        };
        let target = request.key.target();
        let targets = vec![target];
        if let Err(error) = self.request_target_refresh(&targets) {
            return product_failure_response(error);
        }
        self.with_storage(|storage| {
            let snapshots = self.snapshots(&targets)?;
            self.runtime
                .course_detail(storage, &targets, &request, |target| {
                    runtime_for(&snapshots, target)
                })
                .map_err(ProductRouteFailure::Runtime)
        })
    }

    fn section_detail(&self, request: &ExtensionRequest) -> ExtensionResponse {
        let request: SectionDetailRequestV1 = match decode_payload(request.body()) {
            Ok(request) => request,
            Err(response) => return response,
        };
        let target = request.key.target();
        let targets = vec![target];
        if let Err(error) = self.request_target_refresh(&targets) {
            return product_failure_response(error);
        }
        self.with_storage(|storage| {
            let snapshots = self.snapshots(&targets)?;
            self.runtime
                .section_detail(storage, &targets, &request, |target| {
                    runtime_for(&snapshots, target)
                })
                .map_err(ProductRouteFailure::Runtime)
        })
    }

    fn open_status(&self, request: &ExtensionRequest) -> ExtensionResponse {
        let request: OpenStatusRequestV1 = match decode_payload(request.body()) {
            Ok(request) => request,
            Err(response) => return response,
        };
        let target = request.batch.target();
        if let Err(error) = self.request_target_refresh(std::slice::from_ref(&target)) {
            return product_failure_response(error);
        }
        self.with_storage(|storage| {
            let snapshot = self.open_runtime.snapshot(&target)?;
            self.runtime
                .open_status(storage, &target, &snapshot)
                .map(|status| status.refresh)
                .map_err(ProductRouteFailure::Runtime)
        })
    }

    fn open_section_status(&self, request: &ExtensionRequest) -> ExtensionResponse {
        let request: OpenSectionStatusRequestV1 = match decode_payload(request.body()) {
            Ok(request) => request,
            Err(response) => return response,
        };
        let target = request.section_key.target();
        if let Err(error) = self.request_target_refresh(std::slice::from_ref(&target)) {
            return product_failure_response(error);
        }
        self.with_storage(|storage| {
            let snapshot = self.open_runtime.snapshot(&target)?;
            self.runtime
                .open_status(storage, &target, &snapshot)
                .map_err(ProductRouteFailure::Runtime)?
                .sections
                .into_iter()
                .find(|status| status.section_key == request.section_key)
                .ok_or(ProductRouteFailure::SectionNotFound)
        })
    }

    fn search_targets(
        &self,
        storage: &mut OperationalStorage,
        filters: &NormalizedFilterValuesV1,
    ) -> Result<Vec<TermCampusKey>, ProductRouteFailure> {
        if !filters.campuses().is_empty() {
            return Ok(filters
                .campuses()
                .iter()
                .cloned()
                .map(|campus| TermCampusKey::new(filters.term().clone(), campus))
                .collect());
        }

        let discovery = self
            .runtime
            .catalog_discovery(storage)
            .map_err(ProductRouteFailure::Runtime)?;
        let targets = discovery
            .targets
            .into_iter()
            .map(|target| target.key)
            .filter(|target| target.term() == filters.term())
            .collect::<Vec<_>>();
        if targets.is_empty() {
            Err(ProductRouteFailure::CatalogUnavailable)
        } else {
            Ok(targets)
        }
    }

    fn snapshots(
        &self,
        targets: &[TermCampusKey],
    ) -> Result<BTreeMap<TermCampusKey, OpenRuntimeSnapshot>, ProductRouteFailure> {
        self.open_runtime
            .snapshots(targets)
            .map_err(ProductRouteFailure::OpenRuntime)
    }

    fn request_target_refresh(&self, targets: &[TermCampusKey]) -> Result<(), ProductRouteFailure> {
        if let Some(demand) = &self.target_refresh_demand {
            demand.request_all(targets)?;
        }
        Ok(())
    }

    fn with_storage<T>(
        &self,
        operation: impl FnOnce(&mut OperationalStorage) -> Result<T, ProductRouteFailure>,
    ) -> ExtensionResponse
    where
        T: Serialize,
    {
        let mut storage = match self.storage.lock_operational() {
            Ok(storage) => storage,
            Err(_) => return api_error_response(500, ApiErrorCode::InternalError),
        };
        match operation(&mut storage) {
            Ok(value) => success_response(value),
            Err(error) => product_failure_response(error),
        }
    }
}

impl<C, P, S> RouteExtension for SharedProductRoutes<C, P, S>
where
    C: ApplicationClock + 'static,
    P: RefreshPolicyProvider + 'static,
    S: ProductStorageAccess,
{
    fn route_inventory(&self) -> &'static [ExtensionRoute] {
        SHARED_PRODUCT_ROUTE_INVENTORY
    }

    fn handle(&self, request: ExtensionRequest) -> ExtensionResponse {
        match (request.method(), request.path()) {
            (RequestMethod::Get, PRODUCT_FILTER_SCHEMA_PATH) => self.filter_schema(),
            (RequestMethod::Post, PRODUCT_CATALOG_DISCOVERY_PATH) => {
                self.catalog_discovery(&request)
            }
            (RequestMethod::Post, PRODUCT_COURSE_SEARCH_PATH) => self.course_search(&request),
            (RequestMethod::Post, PRODUCT_SECTION_SEARCH_PATH) => self.section_search(&request),
            (RequestMethod::Post, PRODUCT_COURSE_DETAIL_PATH) => self.course_detail(&request),
            (RequestMethod::Post, PRODUCT_SECTION_DETAIL_PATH) => self.section_detail(&request),
            (RequestMethod::Post, PRODUCT_OPEN_STATUS_PATH) => self.open_status(&request),
            (RequestMethod::Post, PRODUCT_OPEN_SECTION_STATUS_PATH) => {
                self.open_section_status(&request)
            }
            _ => ExtensionResponse::not_found(),
        }
    }
}

fn runtime_for(
    snapshots: &BTreeMap<TermCampusKey, OpenRuntimeSnapshot>,
    target: &TermCampusKey,
) -> OpenRuntimeSnapshot {
    snapshots.get(target).cloned().unwrap_or_default()
}

fn decode_payload<T>(body: &[u8]) -> Result<T, ExtensionResponse>
where
    T: for<'de> serde::Deserialize<'de>,
{
    decode_versioned_envelope_json::<HttpRequestEnvelope<T>>(body)
        .map(HttpRequestEnvelope::into_payload)
        .map_err(|error| api_error_response(400, error.code()))
}

fn success_response<T>(value: T) -> ExtensionResponse
where
    T: Serialize,
{
    match serde_json::to_vec(&HttpSuccessEnvelope::new(value)) {
        Ok(body) => ExtensionResponse::json_bytes(200, body),
        Err(_) => api_error_response(500, ApiErrorCode::InternalError),
    }
}

fn api_error_response(status: u16, code: ApiErrorCode) -> ExtensionResponse {
    let mut source = SystemTraceIdSource;
    let envelope = ApiErrorEnvelope::new(ApiErrorBody::new(code, source.next_trace_id()));
    let body = serde_json::to_vec(&envelope).unwrap_or_else(|_| {
        br#"{"protocolVersion":1,"error":{"code":"INTERNAL_ERROR","messageKey":"error.internal","traceId":"00000000-0000-4000-8000-000000000001","details":[]}}"#.to_vec()
    });
    ExtensionResponse::json_bytes(status, body)
}

enum ProductRouteFailure {
    Runtime(SharedRuntimeError),
    OpenRuntime(OpenRuntimeSnapshotRegistryError),
    TargetDemand(TargetRefreshDemandError),
    CatalogUnavailable,
    SectionNotFound,
}

impl From<OpenRuntimeSnapshotRegistryError> for ProductRouteFailure {
    fn from(error: OpenRuntimeSnapshotRegistryError) -> Self {
        Self::OpenRuntime(error)
    }
}

impl From<TargetRefreshDemandError> for ProductRouteFailure {
    fn from(error: TargetRefreshDemandError) -> Self {
        Self::TargetDemand(error)
    }
}

fn product_failure_response(error: ProductRouteFailure) -> ExtensionResponse {
    let (status, code) = match error {
        ProductRouteFailure::CatalogUnavailable => (503, ApiErrorCode::UpstreamUnavailable),
        ProductRouteFailure::SectionNotFound => (404, ApiErrorCode::SectionNotFound),
        ProductRouteFailure::OpenRuntime(_) => (500, ApiErrorCode::InternalError),
        ProductRouteFailure::TargetDemand(_) => (500, ApiErrorCode::InternalError),
        ProductRouteFailure::Runtime(error) => runtime_failure_status(error),
    };
    api_error_response(status, code)
}

fn runtime_failure_status(error: SharedRuntimeError) -> (u16, ApiErrorCode) {
    match error {
        SharedRuntimeError::RefreshPolicyUnavailable
        | SharedRuntimeError::CatalogDiscoveryStorage(_)
        | SharedRuntimeError::CatalogDiscoveryProjection(_)
        | SharedRuntimeError::OpenProjection { .. } => (503, ApiErrorCode::UpstreamUnavailable),
        SharedRuntimeError::Query(SharedQueryError::Query {
            source: QueryError::CourseNotFound,
        }) => (404, ApiErrorCode::InvalidCourseGroupKey),
        SharedRuntimeError::Query(SharedQueryError::Query {
            source: QueryError::SectionNotFound,
        }) => (404, ApiErrorCode::SectionNotFound),
        SharedRuntimeError::Query(
            SharedQueryError::EmptyTargetSet
            | SharedQueryError::TargetNotPublished { .. }
            | SharedQueryError::PublicationChanged { .. }
            | SharedQueryError::FtsUnavailable { .. }
            | SharedQueryError::Storage { .. }
            | SharedQueryError::Projection { .. }
            | SharedQueryError::TextEvidence { .. },
        ) => (503, ApiErrorCode::UpstreamUnavailable),
        SharedRuntimeError::Query(
            SharedQueryError::DuplicateTarget { .. }
            | SharedQueryError::FilterTermMismatch { .. }
            | SharedQueryError::FilterCampusSetMismatch
            | SharedQueryError::DetailTargetMismatch
            | SharedQueryError::InvalidTextTokens { .. },
        ) => (400, ApiErrorCode::InvalidFilter),
        SharedRuntimeError::Query(SharedQueryError::Query { .. }) => {
            (500, ApiErrorCode::InternalError)
        }
    }
}
