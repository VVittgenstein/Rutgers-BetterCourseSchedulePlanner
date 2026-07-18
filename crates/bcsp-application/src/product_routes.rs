use std::collections::BTreeSet;
use std::ops::DerefMut;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

use bcsp_contracts::{
    ApiErrorBody, ApiErrorCode, ApiErrorDetail, ApiErrorEnvelope, CatalogDiscoveryRequestV1,
    CourseDetailRequestV1, CourseQueryRequestV1, DynamicFilterValidationRequestV3,
    FilterOptionsRequestV2, HttpRequestEnvelope, HttpSuccessEnvelope, NormalizedFilterValuesV1,
    OpenSectionStatusRequestV1, OpenStatusRequestV1, QueryContractVersion, SectionDetailRequestV1,
    SectionQueryRequestV1, ServiceIssueComponentV1, ServiceOperationV1, ServiceRuntimeV1,
    SystemTraceIdSource, TermCampusKey, TraceIdSource, decode_versioned_envelope_json,
};
use bcsp_domain::{RutgersTermWindow, RutgersTermWindowScope};
use bcsp_operational_storage::{OperationalStorage, OperationalStorageInterruptHandle};
use bcsp_query::QueryError;
use serde::Serialize;

use crate::service_status::{project_service_status_v2, unavailable_service_status};
use crate::{
    ApplicationClock, ExtensionRequest, ExtensionResponse, ExtensionRoute,
    OpenRuntimeSnapshotRegistry, OpenRuntimeSnapshotRegistryError, PreparedQueryService,
    PreparedServingBinding, PreparedServingError, PreparedServingRegistry, PreparedServingSnapshot,
    RefreshPolicyProvider, RequestMethod, RouteExtension, ServiceStatusRegistry, SharedQueryError,
    SharedRuntimeContext, SharedRuntimeError, TargetRefreshDemand, TargetRefreshDemandError,
    is_product_campus, rebuild_prepared_serving_from_access,
};

pub const PRODUCT_FILTER_SCHEMA_PATH: &str = "/api/v1/query/filter-schema";
pub const PRODUCT_FILTER_OPTIONS_PATH: &str = "/api/v1/query/filter-options";
pub const PRODUCT_DYNAMIC_FILTER_VALIDATION_PATH: &str = "/api/v1/query/validate-dynamic-filters";
pub const PRODUCT_CATALOG_DISCOVERY_PATH: &str = "/api/v1/catalog/discovery";
pub const PRODUCT_COURSE_SEARCH_PATH: &str = "/api/v1/query/courses";
pub const PRODUCT_SECTION_SEARCH_PATH: &str = "/api/v1/query/sections";
pub const PRODUCT_COURSE_DETAIL_PATH: &str = "/api/v1/query/course-detail";
pub const PRODUCT_SECTION_DETAIL_PATH: &str = "/api/v1/query/section-detail";
pub const PRODUCT_OPEN_STATUS_PATH: &str = "/api/v1/open/status";
pub const PRODUCT_OPEN_SECTION_STATUS_PATH: &str = "/api/v1/open/section-status";
pub const PRODUCT_SERVICE_STATUS_PATH: &str = "/api/v1/service/status";

pub static SHARED_PRODUCT_ROUTE_INVENTORY: &[ExtensionRoute] = &[
    ExtensionRoute::new(RequestMethod::Get, PRODUCT_FILTER_SCHEMA_PATH),
    ExtensionRoute::new(RequestMethod::Post, PRODUCT_FILTER_OPTIONS_PATH),
    ExtensionRoute::new(RequestMethod::Post, PRODUCT_DYNAMIC_FILTER_VALIDATION_PATH),
    ExtensionRoute::new(RequestMethod::Post, PRODUCT_CATALOG_DISCOVERY_PATH),
    ExtensionRoute::new(RequestMethod::Post, PRODUCT_COURSE_SEARCH_PATH),
    ExtensionRoute::new(RequestMethod::Post, PRODUCT_SECTION_SEARCH_PATH),
    ExtensionRoute::new(RequestMethod::Post, PRODUCT_COURSE_DETAIL_PATH),
    ExtensionRoute::new(RequestMethod::Post, PRODUCT_SECTION_DETAIL_PATH),
    ExtensionRoute::new(RequestMethod::Post, PRODUCT_OPEN_STATUS_PATH),
    ExtensionRoute::new(RequestMethod::Post, PRODUCT_OPEN_SECTION_STATUS_PATH),
    ExtensionRoute::new(RequestMethod::Get, PRODUCT_SERVICE_STATUS_PATH),
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

    fn operational_interrupt_handle(
        &self,
    ) -> Result<OperationalStorageInterruptHandle, ProductStorageLockError> {
        self.lock_operational()
            .map(|storage| storage.interrupt_handle())
    }
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
    service_status: Arc<ServiceStatusRegistry>,
    target_refresh_demand: Option<TargetRefreshDemand>,
    prepared_serving: Arc<PreparedServingRegistry>,
}

impl<C, P> SharedProductRoutes<C, P>
where
    C: ApplicationClock,
    P: RefreshPolicyProvider,
{
    pub fn new(
        storage: SharedProductStorage,
        runtime: SharedRuntimeContext<C, P>,
        open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
    ) -> Self {
        let routes = Self {
            storage,
            runtime,
            open_runtime,
            service_status: Arc::new(ServiceStatusRegistry::new(ServiceRuntimeV1::Local)),
            target_refresh_demand: None,
            prepared_serving: Arc::new(PreparedServingRegistry::default()),
        };
        routes.rebuild_prepared_serving_lkg();
        routes
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
    pub fn with_storage_access(
        storage: S,
        runtime: SharedRuntimeContext<C, P>,
        open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
    ) -> Self {
        let routes = Self {
            storage,
            runtime,
            open_runtime,
            service_status: Arc::new(ServiceStatusRegistry::new(ServiceRuntimeV1::Local)),
            target_refresh_demand: None,
            prepared_serving: Arc::new(PreparedServingRegistry::default()),
        };
        routes.rebuild_prepared_serving_lkg();
        routes
    }

    pub fn with_target_refresh_demand(mut self, demand: TargetRefreshDemand) -> Self {
        self.target_refresh_demand = Some(demand);
        self
    }

    pub fn with_service_status(mut self, service_status: Arc<ServiceStatusRegistry>) -> Self {
        self.service_status = service_status;
        self.rebuild_prepared_serving_lkg();
        self
    }

    pub fn service_status_registry(&self) -> Arc<ServiceStatusRegistry> {
        Arc::clone(&self.service_status)
    }

    pub const fn storage_access(&self) -> &S {
        &self.storage
    }

    pub fn open_runtime(&self) -> Arc<OpenRuntimeSnapshotRegistry> {
        Arc::clone(&self.open_runtime)
    }

    pub fn prepared_serving_registry(&self) -> Arc<PreparedServingRegistry> {
        Arc::clone(&self.prepared_serving)
    }

    /// Rebuilds one immutable generation. File-backed stores release the
    /// global operational mutex before all Catalog/Open projection and FTS
    /// construction; in-memory stores are retained only as a test fallback.
    pub fn rebuild_prepared_serving_snapshot(
        &self,
    ) -> Result<Arc<PreparedServingSnapshot>, PreparedServingError> {
        rebuild_prepared_serving_from_access(
            &self.storage,
            &self.runtime,
            &self.open_runtime,
            self.service_status.runtime(),
            &self.prepared_serving,
        )
    }

    fn rebuild_prepared_serving_lkg(&self) {
        if let Err(error) = self.rebuild_prepared_serving_snapshot() {
            tracing::warn!(
                target: "bcsp_prepared_serving",
                ?error,
                "prepared serving rebuild failed; preserving previous generation",
            );
        }
    }

    fn filter_schema(&self) -> ExtensionResponse {
        success_response(self.runtime.filter_schema())
    }

    fn filter_options(&self, request: &ExtensionRequest) -> ExtensionResponse {
        let mut request: FilterOptionsRequestV2 = match decode_payload(request.body()) {
            Ok(request) => request,
            Err(response) => return response,
        };
        if request.validate().is_err() {
            return api_error_response(400, ApiErrorCode::InvalidFilter);
        }
        let targets = request.targets();
        self.with_prepared(|snapshot| {
            self.validate_prepared_targets(snapshot, &targets)?;
            self.request_prepared_target_refresh(snapshot, &targets)?;
            self.require_prepared_targets_ready(snapshot, &targets)?;
            PreparedQueryService::from_binding(snapshot)
                .filter_options(&targets, &request)
                .map_err(query_route_failure)
        })
    }

    fn validate_dynamic_filters(&self, request: &ExtensionRequest) -> ExtensionResponse {
        let request: DynamicFilterValidationRequestV3 = match decode_payload(request.body()) {
            Ok(request) => request,
            Err(response) => return response,
        };
        if request.filters.contract_version() != QueryContractVersion::V3 {
            return api_error_response(400, ApiErrorCode::InvalidFilter);
        }
        self.with_prepared(|snapshot| {
            let targets = self.prepared_search_targets(snapshot, request.filters.values())?;
            self.validate_prepared_targets(snapshot, &targets)?;
            self.request_prepared_target_refresh(snapshot, &targets)?;
            self.require_prepared_targets_ready(snapshot, &targets)?;
            PreparedQueryService::from_binding(snapshot)
                .dynamic_filter_validation(&targets, request.filters.values())
                .map_err(query_route_failure)
        })
    }

    fn service_status(&self, request: &ExtensionRequest) -> ExtensionResponse {
        let scoped_targets = match parse_status_scope(request.query()) {
            Ok(targets) => targets,
            Err(()) => return api_error_response(400, ApiErrorCode::InvalidFilter),
        };
        let observed_at = self.runtime.now();
        let activity = self
            .service_status
            .snapshot()
            .map(|snapshot| snapshot.operation)
            .unwrap_or_else(|_| ServiceOperationV1::starting());
        let lock_started = Instant::now();
        let mut storage = match self.storage.lock_operational() {
            Ok(storage) => storage,
            Err(_) => {
                return success_response(unavailable_service_status(
                    observed_at,
                    self.service_status.runtime(),
                    activity,
                    ServiceIssueComponentV1::Storage,
                    "SERVICE_STATUS_STORAGE_UNAVAILABLE",
                ));
            }
        };
        tracing::debug!(
            target: "bcsp_performance",
            phase = "storage_lock_wait",
            elapsed_us = elapsed_micros(lock_started),
            route = "service_status",
        );
        let operation_started = Instant::now();
        if !scoped_targets.is_empty() {
            if let Err(error) = self.validate_targets(&mut storage, &scoped_targets) {
                drop(storage);
                return product_failure_response(error);
            }
            if let Err(error) = self.request_target_refresh(&mut storage, &scoped_targets) {
                drop(storage);
                return product_failure_response(error);
            }
        }
        let status = project_service_status_v2(&mut storage, &self.runtime, &self.service_status);
        drop(storage);
        tracing::debug!(
            target: "bcsp_performance",
            phase = "storage_lock_held",
            elapsed_us = elapsed_micros(operation_started),
            route = "service_status",
        );
        match status {
            Ok(status) => success_response(status),
            Err(status) => success_response(status),
        }
    }

    fn catalog_discovery(&self, request: &ExtensionRequest) -> ExtensionResponse {
        let _: CatalogDiscoveryRequestV1 = match decode_payload(request.body()) {
            Ok(request) => request,
            Err(response) => return response,
        };
        self.with_prepared(|snapshot| Ok(snapshot.discovery_at(self.runtime.now())))
    }

    fn course_search(&self, request: &ExtensionRequest) -> ExtensionResponse {
        let request: CourseQueryRequestV1 = match decode_payload(request.body()) {
            Ok(request) => request,
            Err(response) => return response,
        };
        if request.filters.contract_version() != QueryContractVersion::V3 {
            return api_error_response(400, ApiErrorCode::InvalidFilter);
        }
        self.with_prepared(|snapshot| {
            let stage_started = Instant::now();
            let targets = self.prepared_search_targets(snapshot, request.filters.values())?;
            trace_route_stage("target_selection", "course_search", stage_started);
            let stage_started = Instant::now();
            self.validate_prepared_targets(snapshot, &targets)?;
            trace_route_stage("target_validation", "course_search", stage_started);
            let stage_started = Instant::now();
            self.request_prepared_target_refresh(snapshot, &targets)?;
            trace_route_stage("refresh_request", "course_search", stage_started);
            let stage_started = Instant::now();
            self.require_prepared_targets_ready(snapshot, &targets)?;
            trace_route_stage("ready_check", "course_search", stage_started);
            let stage_started = Instant::now();
            let result = PreparedQueryService::from_binding(snapshot)
                .course_search(&targets, &request, self.runtime.now())
                .map_err(query_route_failure);
            trace_route_stage("query_runtime", "course_search", stage_started);
            result
        })
    }

    fn section_search(&self, request: &ExtensionRequest) -> ExtensionResponse {
        let request: SectionQueryRequestV1 = match decode_payload(request.body()) {
            Ok(request) => request,
            Err(response) => return response,
        };
        if request.filters.contract_version() != QueryContractVersion::V3 {
            return api_error_response(400, ApiErrorCode::InvalidFilter);
        }
        self.with_prepared(|snapshot| {
            let targets = self.prepared_search_targets(snapshot, request.filters.values())?;
            self.validate_prepared_targets(snapshot, &targets)?;
            self.request_prepared_target_refresh(snapshot, &targets)?;
            self.require_prepared_targets_ready(snapshot, &targets)?;
            PreparedQueryService::from_binding(snapshot)
                .section_search(&targets, &request, self.runtime.now())
                .map_err(query_route_failure)
        })
    }

    fn course_detail(&self, request: &ExtensionRequest) -> ExtensionResponse {
        let request: CourseDetailRequestV1 = match decode_payload(request.body()) {
            Ok(request) => request,
            Err(response) => return response,
        };
        if request.contract_version != QueryContractVersion::V3 {
            return api_error_response(400, ApiErrorCode::InvalidFilter);
        }
        let target = request.key.target();
        let targets = vec![target];
        self.with_prepared(|snapshot| {
            self.validate_prepared_targets(snapshot, &targets)?;
            self.request_prepared_target_refresh(snapshot, &targets)?;
            self.require_prepared_targets_ready(snapshot, &targets)?;
            PreparedQueryService::from_binding(snapshot)
                .course_detail(&targets, &request, self.runtime.now())
                .map_err(query_route_failure)
        })
    }

    fn section_detail(&self, request: &ExtensionRequest) -> ExtensionResponse {
        let request: SectionDetailRequestV1 = match decode_payload(request.body()) {
            Ok(request) => request,
            Err(response) => return response,
        };
        if request.contract_version != QueryContractVersion::V3 {
            return api_error_response(400, ApiErrorCode::InvalidFilter);
        }
        let target = request.key.target();
        let targets = vec![target];
        self.with_prepared(|snapshot| {
            self.validate_prepared_targets(snapshot, &targets)?;
            self.request_prepared_target_refresh(snapshot, &targets)?;
            self.require_prepared_targets_ready(snapshot, &targets)?;
            PreparedQueryService::from_binding(snapshot)
                .section_detail(&targets, &request, self.runtime.now())
                .map_err(query_route_failure)
        })
    }

    fn open_status(&self, request: &ExtensionRequest) -> ExtensionResponse {
        let request: OpenStatusRequestV1 = match decode_payload(request.body()) {
            Ok(request) => request,
            Err(response) => return response,
        };
        let target = request.batch.target();
        let policy = match self.runtime.refresh_policy() {
            Ok(policy) => policy,
            Err(error) => {
                return product_failure_response(ProductRouteFailure::Runtime(error));
            }
        };
        self.with_storage(|storage| {
            self.validate_targets(storage, std::slice::from_ref(&target))?;
            self.request_target_refresh(storage, std::slice::from_ref(&target))?;
            self.require_targets_ready(storage, std::slice::from_ref(&target))?;
            let snapshot = self.open_runtime.snapshot(&target)?;
            self.runtime
                .open_status_with_policy(storage, &target, &snapshot, policy)
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
        let policy = match self.runtime.refresh_policy() {
            Ok(policy) => policy,
            Err(error) => {
                return product_failure_response(ProductRouteFailure::Runtime(error));
            }
        };
        self.with_storage(|storage| {
            self.validate_targets(storage, std::slice::from_ref(&target))?;
            self.request_target_refresh(storage, std::slice::from_ref(&target))?;
            self.require_targets_ready(storage, std::slice::from_ref(&target))?;
            let snapshot = self.open_runtime.snapshot(&target)?;
            self.runtime
                .open_status_with_policy(storage, &target, &snapshot, policy)
                .map_err(ProductRouteFailure::Runtime)?
                .sections
                .into_iter()
                .find(|status| status.section_key == request.section_key)
                .ok_or(ProductRouteFailure::SectionNotFound)
        })
    }

    fn prepared_search_targets(
        &self,
        snapshot: &PreparedServingSnapshot,
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

        let targets = snapshot
            .discovery()
            .targets
            .iter()
            .map(|target| target.key.clone())
            .filter(|target| target.term() == filters.term())
            .filter(|target| is_product_campus(target.campus().as_str()))
            .collect::<Vec<_>>();
        if targets.is_empty() {
            Err(ProductRouteFailure::CatalogUnavailable)
        } else {
            Ok(targets)
        }
    }

    fn request_prepared_target_refresh(
        &self,
        snapshot: &PreparedServingSnapshot,
        targets: &[TermCampusKey],
    ) -> Result<(), ProductRouteFailure> {
        let Some(demand) = &self.target_refresh_demand else {
            return Ok(());
        };
        let manual_terms = demand
            .manual_terms_snapshot()?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let local_window = (self.service_status.runtime() == ServiceRuntimeV1::Local)
            .then(|| RutgersTermWindow::at(self.runtime.now(), RutgersTermWindowScope::Local))
            .transpose()
            .map_err(|_| ProductRouteFailure::InvalidTarget)?;
        let eligible = targets
            .iter()
            .filter(|target| {
                let manual = local_window.as_ref().is_some_and(|window| {
                    window
                        .visible_terms()
                        .iter()
                        .find(|term| term.term() == target.term())
                        .is_some_and(|term| !term.auto_managed())
                });
                !manual || manual_terms.contains(target.term()) || snapshot.is_ready(target)
            })
            .cloned()
            .collect::<Vec<_>>();
        demand.request_all(&eligible)?;
        Ok(())
    }

    fn require_prepared_targets_ready(
        &self,
        snapshot: &PreparedServingSnapshot,
        targets: &[TermCampusKey],
    ) -> Result<(), ProductRouteFailure> {
        let unavailable = targets
            .iter()
            .filter(|target| !snapshot.is_ready(target))
            .cloned()
            .collect::<Vec<_>>();
        if unavailable.is_empty() {
            Ok(())
        } else {
            Err(ProductRouteFailure::SearchDataNotReady(unavailable))
        }
    }

    fn validate_prepared_targets(
        &self,
        snapshot: &PreparedServingSnapshot,
        targets: &[TermCampusKey],
    ) -> Result<(), ProductRouteFailure> {
        if targets.is_empty() {
            return Err(ProductRouteFailure::InvalidTarget);
        }
        let scope = match self.service_status.runtime() {
            ServiceRuntimeV1::Local => RutgersTermWindowScope::Local,
            ServiceRuntimeV1::Public => RutgersTermWindowScope::Public,
        };
        let window = RutgersTermWindow::at(self.runtime.now(), scope)
            .map_err(|_| ProductRouteFailure::InvalidTarget)?;
        let visible_terms = window
            .visible_terms()
            .iter()
            .map(|term| term.term())
            .collect::<BTreeSet<_>>();
        if targets.iter().all(|target| {
            visible_terms.contains(target.term())
                && is_product_campus(target.campus().as_str())
                && (snapshot.is_discovered(target) || snapshot.is_ready(target))
        }) {
            Ok(())
        } else {
            Err(ProductRouteFailure::InvalidTarget)
        }
    }

    fn request_target_refresh(
        &self,
        storage: &mut OperationalStorage,
        targets: &[TermCampusKey],
    ) -> Result<(), ProductRouteFailure> {
        let Some(demand) = &self.target_refresh_demand else {
            return Ok(());
        };
        let manual_terms = demand
            .manual_terms_snapshot()?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let local_window = (self.service_status.runtime() == ServiceRuntimeV1::Local)
            .then(|| RutgersTermWindow::at(self.runtime.now(), RutgersTermWindowScope::Local))
            .transpose()
            .map_err(|_| ProductRouteFailure::InvalidTarget)?;
        let mut eligible = Vec::with_capacity(targets.len());
        for target in targets {
            let manual = local_window.as_ref().is_some_and(|window| {
                window
                    .visible_terms()
                    .iter()
                    .find(|term| term.term() == target.term())
                    .is_some_and(|term| !term.auto_managed())
            });
            if manual && !manual_terms.contains(target.term()) {
                let ready = storage
                    .complete_target_snapshot_state(target)
                    .map_err(|source| {
                        ProductRouteFailure::Runtime(SharedRuntimeError::Query(
                            SharedQueryError::Storage {
                                target: target.clone(),
                                source,
                            },
                        ))
                    })?
                    .ready;
                if !ready {
                    continue;
                }
            }
            eligible.push(target.clone());
        }
        demand.request_all(&eligible)?;
        Ok(())
    }

    fn require_targets_ready(
        &self,
        storage: &mut OperationalStorage,
        targets: &[TermCampusKey],
    ) -> Result<(), ProductRouteFailure> {
        let mut unavailable = Vec::new();
        for target in targets {
            let state = storage
                .complete_target_snapshot_state(target)
                .map_err(|error| {
                    ProductRouteFailure::Runtime(SharedRuntimeError::Query(
                        SharedQueryError::Storage {
                            target: target.clone(),
                            source: error,
                        },
                    ))
                })?;
            if !state.ready {
                unavailable.push(target.clone());
            }
        }
        if unavailable.is_empty() {
            Ok(())
        } else {
            Err(ProductRouteFailure::SearchDataNotReady(unavailable))
        }
    }

    fn validate_targets(
        &self,
        storage: &mut OperationalStorage,
        targets: &[TermCampusKey],
    ) -> Result<(), ProductRouteFailure> {
        if targets.is_empty() {
            return Err(ProductRouteFailure::InvalidTarget);
        }
        let scope = match self.service_status.runtime() {
            ServiceRuntimeV1::Local => RutgersTermWindowScope::Local,
            ServiceRuntimeV1::Public => RutgersTermWindowScope::Public,
        };
        let window = RutgersTermWindow::at(self.runtime.now(), scope)
            .map_err(|_| ProductRouteFailure::InvalidTarget)?;
        let visible_terms = window
            .visible_terms()
            .iter()
            .map(|term| term.term())
            .collect::<BTreeSet<_>>();
        let discovered = self
            .runtime
            .catalog_discovery(storage)
            .map_err(ProductRouteFailure::Runtime)?
            .targets
            .into_iter()
            .map(|target| target.key)
            .filter(|target| is_product_campus(target.campus().as_str()))
            .collect::<BTreeSet<_>>();
        if targets.iter().all(|target| {
            visible_terms.contains(target.term())
                && is_product_campus(target.campus().as_str())
                && (discovered.contains(target)
                    || storage
                        .complete_target_snapshot_state(target)
                        .is_ok_and(|state| state.ready))
        }) {
            Ok(())
        } else {
            Err(ProductRouteFailure::InvalidTarget)
        }
    }

    fn with_storage<T>(
        &self,
        operation: impl FnOnce(&mut OperationalStorage) -> Result<T, ProductRouteFailure>,
    ) -> ExtensionResponse
    where
        T: Serialize,
    {
        let lock_started = Instant::now();
        let mut storage = match self.storage.lock_operational() {
            Ok(storage) => storage,
            Err(_) => return api_error_response(500, ApiErrorCode::InternalError),
        };
        tracing::debug!(
            target: "bcsp_performance",
            phase = "storage_lock_wait",
            elapsed_us = elapsed_micros(lock_started),
            route = "shared_product_operation",
        );
        let operation_started = Instant::now();
        let result = operation(&mut storage);
        drop(storage);
        tracing::debug!(
            target: "bcsp_performance",
            phase = "storage_lock_held",
            elapsed_us = elapsed_micros(operation_started),
            route = "shared_product_operation",
        );
        match result {
            Ok(value) => success_response(value),
            Err(error) => product_failure_response(error),
        }
    }

    fn with_prepared<T>(
        &self,
        operation: impl FnOnce(&PreparedServingBinding) -> Result<T, ProductRouteFailure>,
    ) -> ExtensionResponse
    where
        T: Serialize,
    {
        // Bind exactly once before target selection or demand bookkeeping.
        let snapshot = match self.prepared_serving.snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => return product_failure_response(prepared_route_failure(error)),
        };
        match operation(&snapshot) {
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
            (RequestMethod::Post, PRODUCT_FILTER_OPTIONS_PATH) => self.filter_options(&request),
            (RequestMethod::Post, PRODUCT_DYNAMIC_FILTER_VALIDATION_PATH) => {
                self.validate_dynamic_filters(&request)
            }
            (RequestMethod::Get, PRODUCT_SERVICE_STATUS_PATH) => self.service_status(&request),
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

fn query_route_failure(error: SharedQueryError) -> ProductRouteFailure {
    ProductRouteFailure::Runtime(SharedRuntimeError::Query(error))
}

fn prepared_route_failure(error: PreparedServingError) -> ProductRouteFailure {
    query_route_failure(SharedQueryError::Prepared {
        source: Box::new(error),
    })
}

fn parse_status_scope(query: Option<&str>) -> Result<Vec<TermCampusKey>, ()> {
    let Some(query) = query else {
        return Ok(Vec::new());
    };
    let mut term: Option<String> = None;
    let mut campuses = Vec::new();
    for pair in query.split('&').filter(|pair| !pair.is_empty()) {
        let (name, value) = pair.split_once('=').ok_or(())?;
        if value.is_empty() || value.contains('%') || value.contains('+') {
            return Err(());
        }
        match name {
            "activeTerm" if term.is_none() => term = Some(value.to_owned()),
            "activeCampus" => campuses.push(value.to_owned()),
            _ => return Err(()),
        }
    }
    match (term, campuses.is_empty()) {
        (None, true) => Ok(Vec::new()),
        (Some(_), true) | (None, false) => Err(()),
        (Some(term), false) => campuses
            .into_iter()
            .map(|campus| {
                if !is_product_campus(&campus) {
                    return Err(());
                }
                TermCampusKey::try_new(&term, &campus).map_err(|_| ())
            })
            .collect(),
    }
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
    let serialization_started = Instant::now();
    let response = match serde_json::to_vec(&HttpSuccessEnvelope::new(value)) {
        Ok(body) => ExtensionResponse::json_bytes(200, body),
        Err(_) => api_error_response(500, ApiErrorCode::InternalError),
    };
    tracing::debug!(
        target: "bcsp_performance",
        phase = "serialization",
        elapsed_us = elapsed_micros(serialization_started),
        response_bytes = response.body().len(),
    );
    response
}

fn elapsed_micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn trace_route_stage(phase: &'static str, route: &'static str, started: Instant) {
    tracing::debug!(
        target: "bcsp_performance",
        phase,
        elapsed_us = elapsed_micros(started),
        route,
    );
}

fn api_error_response(status: u16, code: ApiErrorCode) -> ExtensionResponse {
    api_error_response_with_details(status, code, Vec::new())
}

fn api_error_response_with_details(
    status: u16,
    code: ApiErrorCode,
    details: Vec<ApiErrorDetail>,
) -> ExtensionResponse {
    let mut source = SystemTraceIdSource;
    let envelope = ApiErrorEnvelope::new(
        ApiErrorBody::new(code, source.next_trace_id()).with_details(details),
    );
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
    SearchDataNotReady(Vec<TermCampusKey>),
    InvalidTarget,
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
    if let ProductRouteFailure::SearchDataNotReady(targets) = &error {
        return api_error_response_with_details(
            503,
            ApiErrorCode::SearchDataNotReady,
            targets
                .iter()
                .cloned()
                .map(|target| ApiErrorDetail::TargetNotReady { target })
                .collect(),
        );
    }
    let (status, code) = match error {
        ProductRouteFailure::CatalogUnavailable => (503, ApiErrorCode::CatalogNotReady),
        ProductRouteFailure::SearchDataNotReady(_) => unreachable!("handled above"),
        ProductRouteFailure::InvalidTarget => (400, ApiErrorCode::InvalidFilter),
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
        SharedRuntimeError::Query(SharedQueryError::Query {
            source: QueryError::UnknownCoreCode { .. },
        }) => (400, ApiErrorCode::InvalidFilter),
        SharedRuntimeError::Query(
            SharedQueryError::EmptyTargetSet
            | SharedQueryError::PublicationChanged { .. }
            | SharedQueryError::FtsUnavailable { .. }
            | SharedQueryError::Storage { .. }
            | SharedQueryError::Projection { .. }
            | SharedQueryError::TextEvidence { .. }
            | SharedQueryError::Prepared { .. },
        ) => (503, ApiErrorCode::UpstreamUnavailable),
        SharedRuntimeError::Query(SharedQueryError::TargetNotPublished { .. }) => {
            (503, ApiErrorCode::CatalogNotReady)
        }
        SharedRuntimeError::Query(
            SharedQueryError::DuplicateTarget { .. }
            | SharedQueryError::FilterTermMismatch { .. }
            | SharedQueryError::FilterCampusSetMismatch
            | SharedQueryError::DetailTargetMismatch
            | SharedQueryError::InvalidTextTokens { .. },
        ) => (400, ApiErrorCode::InvalidFilter),
        SharedRuntimeError::Query(SharedQueryError::InvalidFilterOption { .. }) => {
            (400, ApiErrorCode::InvalidFilterOption)
        }
        SharedRuntimeError::Query(SharedQueryError::Query { .. }) => {
            (500, ApiErrorCode::InternalError)
        }
    }
}
