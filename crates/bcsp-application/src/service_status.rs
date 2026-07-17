use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use bcsp_catalog::to_catalog_discovery_response_v1;
use bcsp_contracts::{
    CatalogContentVersion, CatalogDiscoveryAvailability, CatalogDiscoveryErrorClass,
    CatalogDiscoveryStatusV1, OpenFreshnessState, SERVICE_STATUS_CONTRACT_VERSION,
    SERVICE_STATUS_V2_CONTRACT_VERSION, ServiceAutomaticTermSummaryV2,
    ServiceAvailabilitySummaryV1, ServiceAvailabilityV1, ServiceIssueComponentV1,
    ServiceIssueRecoveryV1, ServiceIssueSeverityV1, ServiceIssueV1, ServiceLevelV1,
    ServiceOperationPhaseV1, ServiceOperationV1, ServiceOperationV2, ServiceRuntimeV1,
    ServiceSnapshotAvailabilityV2, ServiceStatusV1, ServiceStatusV2, ServiceTargetStatusV1,
    ServiceTargetStatusV2, ServiceTermWindowV2, ServiceVisibleTermV2, ServiceWorkStateV2,
    TermCampusKey,
};
use bcsp_domain::{RutgersTermWindow, RutgersTermWindowScope};
use bcsp_operational_storage::{
    CatalogFailureAudit, OpenAttemptRecord, OpenBatchState, OperationalStorage, RefreshStatus,
    TargetState,
};
use time::OffsetDateTime;

use crate::{
    ApplicationClock, CoordinatorStatusSink, CoordinatorStatusSnapshot,
    OpenRuntimeSnapshotRegistry, RefreshPolicy, RefreshPolicyProvider, SharedRuntimeContext,
    TargetWorkActivity, WorkflowOperationActivity, WorkflowOperationId, is_product_campus,
    product_target_keys, product_term_publication,
};

#[derive(Clone, Debug)]
pub struct ServiceActivitySnapshot {
    pub operation: ServiceOperationV1,
    pub coordinator: CoordinatorStatusSnapshot,
    pub scheduler_running: bool,
    pub target_activities: Vec<TargetWorkActivity>,
    pub workflow_operations: Vec<WorkflowOperationActivity>,
}

struct ServiceActivityState {
    operation: ServiceOperationV1,
    coordinator: CoordinatorStatusSnapshot,
    scheduler_running: bool,
    target_activities: BTreeMap<TermCampusKey, TargetWorkActivity>,
    workflow_operations: BTreeMap<WorkflowOperationId, WorkflowOperationActivity>,
}

/// Process-local activity and scheduler facts shared by the status endpoint and refresh runtime.
/// Durable Catalog/Open availability remains authoritative in operational storage.
pub struct ServiceStatusRegistry {
    runtime: ServiceRuntimeV1,
    state: RwLock<ServiceActivityState>,
    delegate: RwLock<Option<Arc<dyn CoordinatorStatusSink>>>,
}

impl ServiceStatusRegistry {
    pub fn new(runtime: ServiceRuntimeV1) -> Self {
        Self {
            runtime,
            state: RwLock::new(ServiceActivityState {
                operation: ServiceOperationV1::starting(),
                coordinator: CoordinatorStatusSnapshot::default(),
                scheduler_running: false,
                target_activities: BTreeMap::new(),
                workflow_operations: BTreeMap::new(),
            }),
            delegate: RwLock::new(None),
        }
    }

    pub const fn runtime(&self) -> ServiceRuntimeV1 {
        self.runtime
    }

    pub fn set_delegate(
        &self,
        delegate: Arc<dyn CoordinatorStatusSink>,
    ) -> Result<(), ServiceStatusRegistryError> {
        let snapshot = self.snapshot()?;
        delegate.publish(snapshot.coordinator);
        for activity in snapshot.target_activities {
            delegate.publish_target_activity(activity);
        }
        for operation in snapshot.workflow_operations {
            delegate.publish_workflow_operation(operation);
        }
        if !snapshot.scheduler_running {
            delegate.mark_stopped();
        }
        *self
            .delegate
            .write()
            .map_err(|_| ServiceStatusRegistryError)? = Some(delegate);
        Ok(())
    }

    pub fn snapshot(&self) -> Result<ServiceActivitySnapshot, ServiceStatusRegistryError> {
        let state = self.state.read().map_err(|_| ServiceStatusRegistryError)?;
        Ok(ServiceActivitySnapshot {
            operation: state.operation.clone(),
            coordinator: state.coordinator,
            scheduler_running: state.scheduler_running,
            target_activities: state.target_activities.values().cloned().collect(),
            workflow_operations: state.workflow_operations.values().cloned().collect(),
        })
    }

    fn delegate(&self) -> Option<Arc<dyn CoordinatorStatusSink>> {
        self.delegate
            .read()
            .ok()
            .and_then(|delegate| delegate.clone())
    }
}

impl CoordinatorStatusSink for ServiceStatusRegistry {
    fn publish(&self, snapshot: CoordinatorStatusSnapshot) {
        if let Ok(mut state) = self.state.write() {
            state.coordinator = snapshot;
            state.scheduler_running = true;
        }
        if let Some(delegate) = self.delegate() {
            delegate.publish(snapshot);
        }
    }

    fn mark_stopped(&self) {
        if let Ok(mut state) = self.state.write() {
            state.scheduler_running = false;
            state.target_activities.clear();
            state.workflow_operations.clear();
            state.operation = ServiceOperationV1 {
                phase: ServiceOperationPhaseV1::Stopped,
                target: None,
                started_at: Some(OffsetDateTime::now_utc()),
                next_retry_at: None,
            };
        }
        if let Some(delegate) = self.delegate() {
            delegate.mark_stopped();
        }
    }

    fn publish_activity(&self, operation: ServiceOperationV1) {
        if let Ok(mut state) = self.state.write() {
            state.operation = operation.clone();
        }
        if let Some(delegate) = self.delegate() {
            delegate.publish_activity(operation);
        }
    }

    fn publish_target_activity(&self, activity: TargetWorkActivity) {
        if let Ok(mut state) = self.state.write() {
            state
                .target_activities
                .insert(activity.target.clone(), activity.clone());
        }
        if let Some(delegate) = self.delegate() {
            delegate.publish_target_activity(activity);
        }
    }

    fn clear_target_activity(&self, target: &TermCampusKey) {
        if let Ok(mut state) = self.state.write() {
            state.target_activities.remove(target);
        }
        if let Some(delegate) = self.delegate() {
            delegate.clear_target_activity(target);
        }
    }

    fn publish_workflow_operation(&self, operation: WorkflowOperationActivity) {
        if let Ok(mut state) = self.state.write() {
            match state.workflow_operations.get_mut(&operation.id) {
                Some(current) => {
                    current.stage = operation.stage;
                }
                None => {
                    state
                        .workflow_operations
                        .insert(operation.id.clone(), operation.clone());
                }
            }
            state.scheduler_running = true;
        }
        if let Some(delegate) = self.delegate() {
            delegate.publish_workflow_operation(operation);
        }
    }

    fn clear_workflow_operation(&self, id: &WorkflowOperationId) {
        if let Ok(mut state) = self.state.write() {
            state.workflow_operations.remove(id);
        }
        if let Some(delegate) = self.delegate() {
            delegate.clear_workflow_operation(id);
        }
    }

    fn clear_target_workflow_operations(&self, target: &TermCampusKey) {
        if let Ok(mut state) = self.state.write() {
            state
                .workflow_operations
                .retain(|id, _| &id.target != target);
        }
        if let Some(delegate) = self.delegate() {
            delegate.clear_target_workflow_operations(target);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceStatusRegistryError;

#[allow(dead_code)]
pub(crate) fn project_service_status<C, P>(
    storage: &mut OperationalStorage,
    runtime: &SharedRuntimeContext<C, P>,
    open_runtime: &OpenRuntimeSnapshotRegistry,
    registry: &ServiceStatusRegistry,
    refresh_policy: Option<RefreshPolicy>,
) -> ServiceStatusV1
where
    C: ApplicationClock,
    P: RefreshPolicyProvider,
{
    let observed_at = runtime.now();
    let activity = registry.snapshot().unwrap_or(ServiceActivitySnapshot {
        operation: ServiceOperationV1::starting(),
        coordinator: CoordinatorStatusSnapshot {
            origin_circuit_open: true,
            overloaded: true,
            ..CoordinatorStatusSnapshot::default()
        },
        scheduler_running: false,
        target_activities: Vec::new(),
        workflow_operations: Vec::new(),
    });
    let published = match storage.published_discovery_snapshot() {
        Ok(published) => published,
        Err(_) => {
            return unavailable_service_status(
                observed_at,
                registry.runtime(),
                activity.operation,
                ServiceIssueComponentV1::Storage,
                "SERVICE_STATUS_STORAGE_UNAVAILABLE",
            );
        }
    };
    let discovery = match to_catalog_discovery_response_v1(&published, &[], observed_at) {
        Ok(discovery) => discovery,
        Err(_) => {
            return unavailable_service_status(
                observed_at,
                registry.runtime(),
                activity.operation,
                ServiceIssueComponentV1::Discovery,
                "DISCOVERY_STATUS_PROJECTION_FAILED",
            );
        }
    };
    let primary_term = latest_primary_term(&published.snapshot);
    let mut issues = Vec::new();
    if refresh_policy.is_none() {
        issues.push(ServiceIssueV1 {
            component: ServiceIssueComponentV1::Open,
            target: None,
            code: "OPEN_REFRESH_POLICY_UNAVAILABLE".to_owned(),
            severity: ServiceIssueSeverityV1::Degraded,
            recovery: ServiceIssueRecoveryV1::UserActionRequired,
            retry_at: None,
        });
    }
    if let Some(error) = &discovery.status.error {
        issues.push(ServiceIssueV1 {
            component: ServiceIssueComponentV1::Discovery,
            target: None,
            code: error.code.to_string(),
            severity: if discovery.status.availability
                == CatalogDiscoveryAvailability::UnavailableNoFirstSuccess
            {
                ServiceIssueSeverityV1::Blocking
            } else {
                ServiceIssueSeverityV1::Degraded
            },
            recovery: if error.class == CatalogDiscoveryErrorClass::Persist {
                ServiceIssueRecoveryV1::UserActionRequired
            } else {
                ServiceIssueRecoveryV1::AutomaticRetry
            },
            retry_at: activity.operation.next_retry_at,
        });
    }

    let mut targets = Vec::with_capacity(discovery.targets.len());
    for discovered in &discovery.targets {
        let target = discovered.key.clone();
        let target_state = match storage.target_state(&target) {
            Ok(state) => state,
            Err(_) => {
                issues.push(storage_issue(Some(target.clone())));
                None
            }
        };
        let (catalog_availability, catalog_content_version) =
            catalog_availability(target_state.as_ref());
        if let Some(issue) = catalog_issue(&target, target_state.as_ref(), catalog_availability) {
            issues.push(issue);
        }
        let search_available = catalog_availability != ServiceAvailabilityV1::Unavailable;
        let open_availability = if search_available {
            let target_retry_at = (activity.operation.phase == ServiceOperationPhaseV1::RetryWait
                && activity.operation.target.as_ref() == Some(&target))
            .then_some(activity.operation.next_retry_at)
            .flatten();
            match storage.open_batch_state(&target) {
                Ok(Some(batch)) => {
                    if let Some(issue) = open_failure_issue(&target, &batch, target_retry_at) {
                        issues.push(issue);
                    }
                }
                Ok(None) => {}
                Err(_) => issues.push(storage_issue(Some(target.clone()))),
            }
            match (open_runtime.snapshot(&target), refresh_policy) {
                (Ok(snapshot), Some(policy)) => {
                    match runtime.open_status_with_policy(storage, &target, &snapshot, policy) {
                        Ok(status) => match status.refresh.freshness.state {
                            OpenFreshnessState::Fresh => ServiceAvailabilityV1::Current,
                            OpenFreshnessState::Stale => ServiceAvailabilityV1::Stale,
                            OpenFreshnessState::Unknown => ServiceAvailabilityV1::Unavailable,
                        },
                        Err(_) => ServiceAvailabilityV1::Unavailable,
                    }
                }
                (Err(_), _) => {
                    issues.push(storage_issue(Some(target.clone())));
                    ServiceAvailabilityV1::Unavailable
                }
                (Ok(_), None) => ServiceAvailabilityV1::Unavailable,
            }
        } else {
            ServiceAvailabilityV1::Unavailable
        };
        targets.push(ServiceTargetStatusV1 {
            primary: primary_term
                .as_ref()
                .is_some_and(|term| target.term() == term),
            target,
            catalog_availability,
            catalog_content_version,
            open_availability,
            search_available,
        });
    }

    if activity.coordinator.origin_circuit_open {
        issues.push(ServiceIssueV1 {
            component: ServiceIssueComponentV1::Scheduler,
            target: activity.operation.target.clone(),
            code: "RUTGERS_ORIGIN_CIRCUIT_OPEN".to_owned(),
            severity: if targets.iter().any(|target| target.search_available) {
                ServiceIssueSeverityV1::Degraded
            } else {
                ServiceIssueSeverityV1::Blocking
            },
            recovery: ServiceIssueRecoveryV1::AutomaticRetry,
            retry_at: activity.operation.next_retry_at,
        });
    }
    if activity.coordinator.overloaded {
        issues.push(ServiceIssueV1 {
            component: ServiceIssueComponentV1::Scheduler,
            target: activity.operation.target.clone(),
            code: "REFRESH_SCHEDULER_OVERLOADED".to_owned(),
            severity: ServiceIssueSeverityV1::Degraded,
            recovery: ServiceIssueRecoveryV1::AutomaticRetry,
            retry_at: activity.operation.next_retry_at,
        });
    }

    let catalog = availability_summary(&targets, |target| target.catalog_availability);
    let open = availability_summary(&targets, |target| target.open_availability);
    // Search is a global product surface. It becomes READY only after every
    // discovered target has current Catalog and Open data; the latest term is
    // still marked as primary for presentation, but no longer narrows readiness.
    let readiness_scope = targets.iter().collect::<Vec<_>>();
    let level = service_level(&discovery.status, &readiness_scope, &issues);

    ServiceStatusV1 {
        contract_version: SERVICE_STATUS_CONTRACT_VERSION,
        observed_at,
        runtime: registry.runtime(),
        level,
        operation: activity.operation,
        discovery: discovery.status,
        catalog,
        open,
        targets,
        issues,
    }
}

// The error is the complete V1 fallback payload required by the raw-wire contract. Boxing it
// would complicate every caller without reducing the payload that must ultimately be served.
#[allow(clippy::result_large_err)]
pub(crate) fn project_service_status_v2<C, P>(
    storage: &mut OperationalStorage,
    runtime: &SharedRuntimeContext<C, P>,
    registry: &ServiceStatusRegistry,
) -> Result<ServiceStatusV2, ServiceStatusV1>
where
    C: ApplicationClock,
    P: RefreshPolicyProvider,
{
    let observed_at = runtime.now();
    let activity = registry.snapshot().unwrap_or(ServiceActivitySnapshot {
        operation: ServiceOperationV1::starting(),
        coordinator: CoordinatorStatusSnapshot {
            origin_circuit_open: true,
            overloaded: true,
            ..CoordinatorStatusSnapshot::default()
        },
        scheduler_running: false,
        target_activities: Vec::new(),
        workflow_operations: Vec::new(),
    });
    let scope = match registry.runtime() {
        ServiceRuntimeV1::Local => RutgersTermWindowScope::Local,
        ServiceRuntimeV1::Public => RutgersTermWindowScope::Public,
    };
    let window = RutgersTermWindow::at(observed_at, scope).map_err(|error| {
        unavailable_service_status(
            observed_at,
            registry.runtime(),
            activity.operation.clone(),
            ServiceIssueComponentV1::Scheduler,
            error.diagnostic_code(),
        )
    })?;
    let published = storage.published_discovery_snapshot().map_err(|_| {
        unavailable_service_status(
            observed_at,
            registry.runtime(),
            activity.operation.clone(),
            ServiceIssueComponentV1::Storage,
            "SERVICE_STATUS_STORAGE_UNAVAILABLE",
        )
    })?;
    let discovery =
        to_catalog_discovery_response_v1(&published, &[], observed_at).map_err(|_| {
            unavailable_service_status(
                observed_at,
                registry.runtime(),
                activity.operation.clone(),
                ServiceIssueComponentV1::Discovery,
                "DISCOVERY_STATUS_PROJECTION_FAILED",
            )
        })?;
    let visible_ids = window
        .visible_terms()
        .iter()
        .map(|term| term.term().clone())
        .collect::<BTreeSet<_>>();
    let visible_terms = window
        .visible_terms()
        .iter()
        .map(|term| ServiceVisibleTermV2 {
            term: term.term().clone(),
            relative_offset: term.relative_offset(),
            publication: product_term_publication(&published, term.term(), observed_at),
            auto_managed: term.auto_managed(),
            manual_pull_allowed: registry.runtime() == ServiceRuntimeV1::Local
                && !term.auto_managed(),
            watchable: term.watchable(),
        })
        .collect::<Vec<_>>();
    let target_activity = activity
        .target_activities
        .iter()
        .map(|item| (item.target.clone(), item))
        .collect::<BTreeMap<_, _>>();
    let mut running_by_target = BTreeMap::<TermCampusKey, &WorkflowOperationActivity>::new();
    for operation in &activity.workflow_operations {
        match running_by_target.get_mut(&operation.id.target) {
            Some(current) if current.id.kind <= operation.id.kind => {}
            Some(current) => *current = operation,
            None => {
                running_by_target.insert(operation.id.target.clone(), operation);
            }
        }
    }
    let product_targets = window
        .visible_terms()
        .iter()
        .flat_map(|term| product_target_keys(term.term()))
        .collect::<Vec<_>>();
    let mut targets = Vec::with_capacity(product_targets.len());
    let mut issues = Vec::new();
    if let Some(error) = &discovery.status.error {
        issues.push(ServiceIssueV1 {
            component: ServiceIssueComponentV1::Discovery,
            target: None,
            code: error.code.to_string(),
            severity: ServiceIssueSeverityV1::Degraded,
            recovery: ServiceIssueRecoveryV1::AutomaticRetry,
            retry_at: activity.operation.next_retry_at,
        });
    }
    for target in product_targets {
        let candidate_activity = target_activity.get(&target).copied();
        let running_operation = running_by_target.get(&target).copied();
        let catalog_state = storage.target_state(&target).map_err(|_| {
            unavailable_service_status(
                observed_at,
                registry.runtime(),
                activity.operation.clone(),
                ServiceIssueComponentV1::Storage,
                "SERVICE_STATUS_STORAGE_UNAVAILABLE",
            )
        })?;
        let open_state = storage.open_batch_state(&target).map_err(|_| {
            unavailable_service_status(
                observed_at,
                registry.runtime(),
                activity.operation.clone(),
                ServiceIssueComponentV1::Storage,
                "SERVICE_STATUS_STORAGE_UNAVAILABLE",
            )
        })?;
        let complete = storage
            .complete_target_snapshot_state(&target)
            .map_err(|_| {
                unavailable_service_status(
                    observed_at,
                    registry.runtime(),
                    activity.operation.clone(),
                    ServiceIssueComponentV1::Storage,
                    "SERVICE_STATUS_STORAGE_UNAVAILABLE",
                )
            })?;
        let requested = candidate_activity.is_some()
            || running_operation.is_some()
            || catalog_state.is_some()
            || open_state.is_some();
        let snapshot_availability = if complete.ready {
            ServiceSnapshotAvailabilityV2::Ready
        } else if requested {
            ServiceSnapshotAvailabilityV2::NoCompleteSnapshot
        } else {
            ServiceSnapshotAvailabilityV2::Unrequested
        };
        let last_complete_at = if complete.ready {
            open_state
                .as_ref()
                .and_then(|state| state.last_success_at.as_deref())
                .and_then(parse_timestamp)
        } else {
            None
        };
        let catalog_audit = catalog_state
            .as_ref()
            .and_then(|state| state.last_attempt.as_ref())
            .filter(|attempt| {
                matches!(
                    attempt.status,
                    RefreshStatus::Failed | RefreshStatus::Interrupted
                )
            })
            .map(|attempt| storage.catalog_failure_audit(&attempt.observation_id))
            .transpose()
            .map_err(|_| {
                unavailable_service_status(
                    observed_at,
                    registry.runtime(),
                    activity.operation.clone(),
                    ServiceIssueComponentV1::Storage,
                    "SERVICE_STATUS_STORAGE_UNAVAILABLE",
                )
            })?
            .flatten();
        let stored_error = candidate_activity
            .and_then(|item| item.error.clone())
            .or_else(|| {
                let open_failure_attempt = open_state
                    .as_ref()
                    .filter(|state| {
                        state.last_failure_attempt_sequence == Some(state.last_attempt_sequence)
                    })
                    .and_then(|state| state.last_attempt_id)
                    .and_then(|attempt_id| storage.open_attempt(&attempt_id).ok().flatten());
                target_last_error(
                    catalog_state.as_ref(),
                    open_state.as_ref(),
                    catalog_audit.as_ref(),
                    open_failure_attempt.as_ref(),
                )
            });
        targets.push(ServiceTargetStatusV2 {
            primary: target.campus().as_str().eq_ignore_ascii_case("NB"),
            target,
            snapshot_availability,
            work_state: if running_operation.is_some() {
                ServiceWorkStateV2::Running
            } else {
                candidate_activity.map_or(ServiceWorkStateV2::Idle, |item| item.work_state)
            },
            stage: running_operation
                .map(|operation| operation.stage)
                .or_else(|| candidate_activity.and_then(|item| item.stage)),
            usable: complete.ready,
            catalog_content_version: complete
                .ready
                .then(|| CatalogContentVersion::try_from(complete.catalog_content_version).ok())
                .flatten(),
            last_complete_at,
            next_retry_at: running_operation
                .is_none()
                .then(|| candidate_activity.and_then(|item| item.next_retry_at))
                .flatten(),
            error: stored_error,
        });
    }
    targets.sort_by(|left, right| left.target.cmp(&right.target));
    let automatic_term_summaries = [window.current_term(), window.next_term()]
        .into_iter()
        .map(|term| ServiceAutomaticTermSummaryV2 {
            term: term.clone(),
            ready_target_count: targets
                .iter()
                .filter(|target| target.target.term() == term && target.usable)
                .count() as u64,
            total_target_count: targets
                .iter()
                .filter(|target| target.target.term() == term)
                .count() as u64,
        })
        .collect::<Vec<_>>();
    let operations = activity
        .workflow_operations
        .iter()
        .filter(|operation| {
            visible_ids.contains(operation.id.target.term())
                && is_product_campus(operation.id.target.campus().as_str())
        })
        .map(|operation| ServiceOperationV2 {
            target: operation.id.target.clone(),
            stage: operation.stage,
            started_at: operation.started_at,
        })
        .collect::<Vec<_>>();
    let automatic_total = automatic_term_summaries
        .iter()
        .map(|summary| summary.total_target_count)
        .sum::<u64>();
    let automatic_ready = automatic_term_summaries
        .iter()
        .map(|summary| summary.ready_target_count)
        .sum::<u64>();
    let any_retry = targets
        .iter()
        .any(|target| target.work_state == ServiceWorkStateV2::RetryWait);
    let level = if automatic_total > 0 && automatic_ready == automatic_total {
        if any_retry {
            ServiceLevelV1::Degraded
        } else {
            ServiceLevelV1::Ready
        }
    } else if automatic_ready > 0 {
        ServiceLevelV1::PartiallyReady
    } else if any_retry {
        ServiceLevelV1::Degraded
    } else {
        ServiceLevelV1::Initializing
    };
    Ok(ServiceStatusV2 {
        contract_version: SERVICE_STATUS_V2_CONTRACT_VERSION,
        observed_at,
        runtime: registry.runtime(),
        level,
        discovery: discovery.status,
        term_window: ServiceTermWindowV2 {
            current_term: window.current_term().clone(),
            next_term: window.next_term().clone(),
            visible_terms,
        },
        automatic_term_summaries,
        operations,
        targets,
        issues,
    })
}

fn parse_timestamp(value: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).ok()
}

fn target_last_error(
    catalog: Option<&TargetState>,
    open: Option<&OpenBatchState>,
    catalog_audit: Option<&CatalogFailureAudit>,
    open_failure_attempt: Option<&OpenAttemptRecord>,
) -> Option<bcsp_contracts::ServiceTargetErrorV2> {
    let catalog_attempt = catalog
        .and_then(|state| state.last_attempt.as_ref())
        .filter(|attempt| {
            matches!(
                attempt.status,
                RefreshStatus::Failed | RefreshStatus::Interrupted
            )
        })
        .filter(|attempt| attempt.error_code.is_some());
    if let Some(attempt) = catalog_attempt {
        return Some(bcsp_contracts::ServiceTargetErrorV2 {
            code: attempt.error_code.as_deref()?.to_owned(),
            http_status: catalog_audit.and_then(|audit| audit.http_status),
            content_type: catalog_audit.and_then(|audit| audit.content_type.clone()),
            content_encoding: catalog_audit.and_then(|audit| audit.content_encoding.clone()),
            decoded_bytes: catalog_audit.and_then(|audit| audit.decoded_bytes),
            error_class: None,
            error_chain: catalog_audit.and_then(|audit| audit.error_chain.clone()),
            trace_id: Some(attempt.observation_id),
        });
    }
    let open = open.filter(|state| {
        state.last_failure_at.is_some()
            && state.last_failure_attempt_sequence == Some(state.last_attempt_sequence)
    })?;
    let attempt = open_failure_attempt?;
    Some(bcsp_contracts::ServiceTargetErrorV2 {
        code: attempt
            .error_code
            .as_deref()
            .or(open.last_failure_error_code.as_deref())?
            .to_owned(),
        http_status: attempt.http.http_status.or(open.last_failure_http_status),
        content_type: attempt.http.content_type.clone(),
        content_encoding: None,
        decoded_bytes: attempt.http.decoded_bytes,
        error_class: None,
        error_chain: None,
        trace_id: Some(attempt.attempt_id),
    })
}

pub(crate) fn unavailable_service_status(
    observed_at: OffsetDateTime,
    runtime: ServiceRuntimeV1,
    operation: ServiceOperationV1,
    component: ServiceIssueComponentV1,
    code: &str,
) -> ServiceStatusV1 {
    ServiceStatusV1 {
        contract_version: SERVICE_STATUS_CONTRACT_VERSION,
        observed_at,
        runtime,
        level: ServiceLevelV1::Error,
        operation,
        discovery: CatalogDiscoveryStatusV1 {
            availability: CatalogDiscoveryAvailability::UnavailableNoFirstSuccess,
            latest_attempt: None,
            last_success: None,
            is_stale: false,
            error: None,
        },
        catalog: ServiceAvailabilitySummaryV1::default(),
        open: ServiceAvailabilitySummaryV1::default(),
        targets: Vec::new(),
        issues: vec![ServiceIssueV1 {
            component,
            target: None,
            code: code.to_owned(),
            severity: ServiceIssueSeverityV1::Blocking,
            recovery: ServiceIssueRecoveryV1::UserActionRequired,
            retry_at: None,
        }],
    }
}

#[allow(dead_code)]
fn latest_primary_term(
    snapshot: &bcsp_operational_storage::DiscoverySnapshot,
) -> Option<bcsp_contracts::TermId> {
    let target_terms = snapshot
        .campuses
        .iter()
        .map(|campus| campus.target.term().clone())
        .collect::<BTreeSet<_>>();
    snapshot
        .terms
        .iter()
        .filter(|term| target_terms.contains(&term.term_id))
        .filter_map(|term| {
            Some((
                term.year?,
                term.term_code.as_deref()?.parse::<u8>().ok()?,
                term.term_id.clone(),
            ))
        })
        .max_by_key(|(year, term_code, _)| (*year, *term_code))
        .map(|(_, _, term)| term)
}

#[allow(dead_code)]
fn catalog_availability(
    state: Option<&TargetState>,
) -> (ServiceAvailabilityV1, Option<CatalogContentVersion>) {
    let Some(state) = state.filter(|state| state.current_content_version > 0) else {
        return (ServiceAvailabilityV1::Unavailable, None);
    };
    let version = CatalogContentVersion::try_from(state.current_content_version).ok();
    let stale = state.last_attempt.as_ref().is_some_and(|attempt| {
        matches!(
            attempt.status,
            RefreshStatus::Failed
                | RefreshStatus::Interrupted
                | RefreshStatus::EmptySuspectRetained
        )
    });
    (
        if stale {
            ServiceAvailabilityV1::Stale
        } else {
            ServiceAvailabilityV1::Current
        },
        version,
    )
}

#[allow(dead_code)]
fn catalog_issue(
    target: &TermCampusKey,
    state: Option<&TargetState>,
    availability: ServiceAvailabilityV1,
) -> Option<ServiceIssueV1> {
    let attempt = state?.last_attempt.as_ref()?;
    if !matches!(
        attempt.status,
        RefreshStatus::Failed | RefreshStatus::Interrupted | RefreshStatus::EmptySuspectRetained
    ) {
        return None;
    }
    Some(ServiceIssueV1 {
        component: ServiceIssueComponentV1::Catalog,
        target: Some(target.clone()),
        code: attempt
            .error_code
            .clone()
            .unwrap_or_else(|| "CATALOG_REFRESH_FAILED".to_owned()),
        severity: if availability == ServiceAvailabilityV1::Unavailable {
            ServiceIssueSeverityV1::Blocking
        } else {
            ServiceIssueSeverityV1::Degraded
        },
        recovery: ServiceIssueRecoveryV1::AutomaticRetry,
        retry_at: None,
    })
}

#[allow(dead_code)]
fn storage_issue(target: Option<TermCampusKey>) -> ServiceIssueV1 {
    ServiceIssueV1 {
        component: ServiceIssueComponentV1::Storage,
        target,
        code: "SERVICE_STATUS_STORAGE_UNAVAILABLE".to_owned(),
        severity: ServiceIssueSeverityV1::Blocking,
        recovery: ServiceIssueRecoveryV1::UserActionRequired,
        retry_at: None,
    }
}

#[allow(dead_code)]
fn open_failure_issue(
    target: &TermCampusKey,
    batch: &OpenBatchState,
    retry_at: Option<OffsetDateTime>,
) -> Option<ServiceIssueV1> {
    let code = current_open_failure_code(
        batch.last_attempt_sequence,
        batch.last_failure_attempt_sequence,
        batch.last_failure_error_code.as_deref(),
    )?;
    Some(ServiceIssueV1 {
        component: ServiceIssueComponentV1::Open,
        target: Some(target.clone()),
        code: code.to_owned(),
        severity: ServiceIssueSeverityV1::Degraded,
        recovery: ServiceIssueRecoveryV1::AutomaticRetry,
        retry_at,
    })
}

#[allow(dead_code)]
fn current_open_failure_code(
    last_attempt_sequence: u64,
    last_failure_attempt_sequence: Option<u64>,
    last_failure_error_code: Option<&str>,
) -> Option<&str> {
    (last_failure_attempt_sequence == Some(last_attempt_sequence))
        .then_some(last_failure_error_code)
        .flatten()
}

#[allow(dead_code)]
fn availability_summary(
    targets: &[ServiceTargetStatusV1],
    availability: impl Fn(&ServiceTargetStatusV1) -> ServiceAvailabilityV1,
) -> ServiceAvailabilitySummaryV1 {
    let current_target_count = u64::try_from(
        targets
            .iter()
            .filter(|target| availability(target) == ServiceAvailabilityV1::Current)
            .count(),
    )
    .unwrap_or(u64::MAX);
    let stale_target_count = u64::try_from(
        targets
            .iter()
            .filter(|target| availability(target) == ServiceAvailabilityV1::Stale)
            .count(),
    )
    .unwrap_or(u64::MAX);
    let unavailable_target_count = u64::try_from(
        targets
            .iter()
            .filter(|target| availability(target) == ServiceAvailabilityV1::Unavailable)
            .count(),
    )
    .unwrap_or(u64::MAX);
    ServiceAvailabilitySummaryV1 {
        total_target_count: u64::try_from(targets.len()).unwrap_or(u64::MAX),
        current_target_count,
        stale_target_count,
        unavailable_target_count,
        available_target_count: current_target_count.saturating_add(stale_target_count),
    }
}

#[allow(dead_code)]
fn service_level(
    discovery: &CatalogDiscoveryStatusV1,
    targets: &[&ServiceTargetStatusV1],
    issues: &[ServiceIssueV1],
) -> ServiceLevelV1 {
    let scope = targets
        .iter()
        .map(|target| target.target.clone())
        .collect::<BTreeSet<_>>();
    let relevant_issue = |issue: &&ServiceIssueV1| {
        issue
            .target
            .as_ref()
            .is_none_or(|target| scope.contains(target))
    };
    let search_count = targets
        .iter()
        .filter(|target| target.search_available)
        .count();
    let open_count = targets
        .iter()
        .filter(|target| target.open_availability != ServiceAvailabilityV1::Unavailable)
        .count();
    let blocking = issues
        .iter()
        .filter(relevant_issue)
        .any(|issue| issue.severity == ServiceIssueSeverityV1::Blocking);
    let degraded = issues
        .iter()
        .filter(relevant_issue)
        .any(|issue| issue.severity == ServiceIssueSeverityV1::Degraded)
        || targets.iter().any(|target| {
            target.catalog_availability == ServiceAvailabilityV1::Stale
                || target.open_availability == ServiceAvailabilityV1::Stale
        });
    if blocking && search_count == 0 {
        return ServiceLevelV1::Error;
    }
    if discovery.availability == CatalogDiscoveryAvailability::UnavailableNoFirstSuccess
        || targets.is_empty()
        || search_count == 0
    {
        return ServiceLevelV1::Initializing;
    }
    if search_count < targets.len() || open_count < targets.len() {
        return if degraded {
            ServiceLevelV1::Degraded
        } else {
            ServiceLevelV1::PartiallyReady
        };
    }
    if degraded || blocking {
        ServiceLevelV1::Degraded
    } else {
        ServiceLevelV1::Ready
    }
}

#[cfg(test)]
mod tests {
    use bcsp_contracts::{
        CatalogDiscoveryAvailability, CatalogDiscoveryStatusV1, ServiceIssueRecoveryV1,
        ServiceOperationPhaseV1,
    };
    use bcsp_operational_storage::{
        BeginRefreshAttemptCommand, FinishRefreshFailureCommand, RefreshFailureStage,
    };

    use super::*;

    fn discovery(availability: CatalogDiscoveryAvailability) -> CatalogDiscoveryStatusV1 {
        CatalogDiscoveryStatusV1 {
            availability,
            latest_attempt: None,
            last_success: None,
            is_stale: false,
            error: None,
        }
    }

    fn target(
        catalog: ServiceAvailabilityV1,
        open: ServiceAvailabilityV1,
    ) -> ServiceTargetStatusV1 {
        ServiceTargetStatusV1 {
            target: TermCampusKey::try_new("92026", "NB").expect("target"),
            primary: true,
            catalog_availability: catalog,
            catalog_content_version: None,
            open_availability: open,
            search_available: catalog != ServiceAvailabilityV1::Unavailable,
        }
    }

    #[test]
    fn stored_catalog_failure_audit_projects_into_target_diagnostics() {
        let mut storage = OperationalStorage::open_in_memory().expect("storage");
        let target = TermCampusKey::try_new("72026", "NB").expect("target");
        let observation_id = "123e4567-e89b-42d3-a456-426614174000"
            .parse()
            .expect("trace ID");
        storage
            .begin_refresh_attempt(&BeginRefreshAttemptCommand {
                observation_id,
                target: target.clone(),
                started_at: "2026-07-17T00:00:00Z".to_owned(),
                source_content_sha256: None,
                source_bytes: None,
            })
            .expect("begin attempt");
        let audit = CatalogFailureAudit {
            http_status: Some(503),
            content_type: Some("application/json".to_owned()),
            content_encoding: Some("gzip".to_owned()),
            decoded_bytes: Some(27),
            error_chain: Some("Catalog upstream returned non-success HTTP status 503".to_owned()),
        };
        storage
            .finish_refresh_failure_with_audit(
                &FinishRefreshFailureCommand {
                    observation_id,
                    completed_at: "2026-07-17T00:00:01Z".to_owned(),
                    stage: RefreshFailureStage::Transport,
                    source_content_sha256: None,
                    source_bytes: None,
                    error_code: "CATALOG_UPSTREAM_TRANSPORT".to_owned(),
                    diagnostic_token: None,
                },
                Some(&audit),
            )
            .expect("finish attempt");
        let state = storage
            .target_state(&target)
            .expect("state")
            .expect("target");

        let diagnostic =
            target_last_error(Some(&state), None, Some(&audit), None).expect("diagnostic");
        assert_eq!(diagnostic.http_status, Some(503));
        assert_eq!(diagnostic.content_type.as_deref(), Some("application/json"));
        assert_eq!(diagnostic.content_encoding.as_deref(), Some("gzip"));
        assert_eq!(diagnostic.decoded_bytes, Some(27));
        assert_eq!(diagnostic.trace_id, Some(observation_id));
        assert_eq!(diagnostic.error_chain, audit.error_chain);
    }

    #[test]
    fn catalog_ready_while_open_pending_is_partially_ready() {
        let target = target(
            ServiceAvailabilityV1::Current,
            ServiceAvailabilityV1::Unavailable,
        );
        assert_eq!(
            service_level(
                &discovery(CatalogDiscoveryAvailability::Current),
                &[&target],
                &[]
            ),
            ServiceLevelV1::PartiallyReady
        );
    }

    #[test]
    fn level_contract_covers_initializing_ready_and_error() {
        assert_eq!(
            service_level(
                &discovery(CatalogDiscoveryAvailability::UnavailableNoFirstSuccess),
                &[],
                &[]
            ),
            ServiceLevelV1::Initializing
        );

        let ready = target(
            ServiceAvailabilityV1::Current,
            ServiceAvailabilityV1::Current,
        );
        assert_eq!(
            service_level(
                &discovery(CatalogDiscoveryAvailability::Current),
                &[&ready],
                &[]
            ),
            ServiceLevelV1::Ready
        );

        let unavailable = target(
            ServiceAvailabilityV1::Unavailable,
            ServiceAvailabilityV1::Unavailable,
        );
        let blocking = ServiceIssueV1 {
            component: ServiceIssueComponentV1::Storage,
            target: Some(unavailable.target.clone()),
            code: "SERVICE_STATUS_STORAGE_UNAVAILABLE".to_owned(),
            severity: ServiceIssueSeverityV1::Blocking,
            recovery: ServiceIssueRecoveryV1::UserActionRequired,
            retry_at: None,
        };
        assert_eq!(
            service_level(
                &discovery(CatalogDiscoveryAvailability::Current),
                &[&unavailable],
                &[blocking]
            ),
            ServiceLevelV1::Error
        );
    }

    #[test]
    fn lkg_with_degraded_issue_remains_searchable_and_degraded() {
        let target = target(ServiceAvailabilityV1::Stale, ServiceAvailabilityV1::Stale);
        let issue = ServiceIssueV1 {
            component: ServiceIssueComponentV1::Catalog,
            target: Some(target.target.clone()),
            code: "CATALOG_UPSTREAM_TRANSPORT".to_owned(),
            severity: ServiceIssueSeverityV1::Degraded,
            recovery: ServiceIssueRecoveryV1::AutomaticRetry,
            retry_at: None,
        };
        assert!(target.search_available);
        assert_eq!(
            service_level(
                &discovery(CatalogDiscoveryAvailability::StaleLastSuccess),
                &[&target],
                &[issue]
            ),
            ServiceLevelV1::Degraded
        );
    }

    #[test]
    fn latest_open_failure_with_lkg_is_degraded_but_an_old_failure_is_not() {
        assert_eq!(
            current_open_failure_code(5, Some(5), Some("OPEN_UPSTREAM_TRANSPORT")),
            Some("OPEN_UPSTREAM_TRANSPORT")
        );
        assert_eq!(
            current_open_failure_code(6, Some(5), Some("OPEN_UPSTREAM_TRANSPORT")),
            None
        );

        let target = target(
            ServiceAvailabilityV1::Current,
            ServiceAvailabilityV1::Current,
        );
        let issue = ServiceIssueV1 {
            component: ServiceIssueComponentV1::Open,
            target: Some(target.target.clone()),
            code: "OPEN_UPSTREAM_TRANSPORT".to_owned(),
            severity: ServiceIssueSeverityV1::Degraded,
            recovery: ServiceIssueRecoveryV1::AutomaticRetry,
            retry_at: None,
        };
        assert_eq!(
            service_level(
                &discovery(CatalogDiscoveryAvailability::Current),
                &[&target],
                &[issue]
            ),
            ServiceLevelV1::Degraded
        );
    }

    #[test]
    fn registry_records_real_activity_without_touching_storage() {
        let registry = ServiceStatusRegistry::new(ServiceRuntimeV1::Local);
        let target = TermCampusKey::try_new("92026", "NB").expect("target");
        registry.publish_activity(ServiceOperationV1 {
            phase: ServiceOperationPhaseV1::CatalogFetch,
            target: Some(target.clone()),
            started_at: Some(OffsetDateTime::UNIX_EPOCH),
            next_retry_at: None,
        });
        let snapshot = registry.snapshot().expect("snapshot");
        assert_eq!(
            snapshot.operation.phase,
            ServiceOperationPhaseV1::CatalogFetch
        );
        assert_eq!(snapshot.operation.target, Some(target));

        registry.mark_stopped();
        let stopped = registry.snapshot().expect("stopped snapshot");
        assert_eq!(stopped.operation.phase, ServiceOperationPhaseV1::Stopped);
        assert!(!stopped.scheduler_running);
    }
}
