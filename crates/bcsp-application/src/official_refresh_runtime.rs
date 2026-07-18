use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use bcsp_contracts::{
    ServiceOperationPhaseV1, ServiceOperationV1, SystemTraceIdSource, TermCampusKey, TraceId,
    TraceIdSource,
};
use bcsp_domain::{RutgersTermWindow, RutgersTermWindowError, RutgersTermWindowScope};
use bcsp_open::OpenCounterAudience;
use bcsp_operational_storage::OperationalStorageInterruptHandle;
use bcsp_rutgers_client::{
    DiscoveryClientBuildError, DiscoveryFailure, DiscoveryTransportError, RutgersDiscoveryClient,
};
use thiserror::Error;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::sync::{Semaphore, watch};
use tokio::task::JoinHandle;

use crate::discovery_runtime::{
    product_refresh_targets, publish_discovery_for_refresh_with_prepared,
    restore_product_refresh_targets,
};
use crate::refresh_generation::refresh_generation_interval;
use crate::{
    CoordinatorStatusSink, OpenRuntimeSnapshotRegistry, PreparedServingRebuildRuntime,
    PreparedServingRegistry, ProductStorageAccess, RefreshPolicyProvider, RefreshRuntime,
    RutgersRefreshUpstream, RutgersRefreshUpstreamBuildError, SelectorTargetMembership,
    SharedRefreshCoordinator, SharedWatchSocket, SystemCoordinatorClock, TargetRefreshDemand,
    is_product_campus, record_discovery_transport_failure,
};

/// Discovery is deliberately lower frequency than Catalog and Open polling.
pub const DISCOVERY_RETRY_INTERVAL: Duration = Duration::from_secs(5 * 60);
const TARGET_DEMAND_SCAN_INTERVAL: Duration = Duration::from_millis(250);
const OFFICIAL_REFRESH_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);
const CI_NO_RUTGERS_ENVIRONMENT: &str = "BCSP_CI_NO_RUTGERS";

/// Production lifecycle for dynamic discovery plus the shared Catalog/Open coordinator.
pub struct OfficialRefreshRuntime {
    shutdown: watch::Sender<bool>,
    task: Option<JoinHandle<()>>,
    prepared_rebuild: Option<PreparedServingRebuildRuntime>,
    storage_interrupt: Option<OperationalStorageInterruptHandle>,
}

impl OfficialRefreshRuntime {
    #[allow(clippy::too_many_arguments)]
    pub fn spawn<S, P>(
        storage: S,
        policy: P,
        run_id: TraceId,
        counter_audience: OpenCounterAudience,
        watch_socket: Arc<SharedWatchSocket>,
        open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
        status: Arc<dyn CoordinatorStatusSink>,
    ) -> Result<Self, OfficialRefreshRuntimeBuildError>
    where
        S: ProductStorageAccess + Clone + Send + 'static,
        P: RefreshPolicyProvider + Clone + Send + 'static,
    {
        Self::spawn_with_target_refresh_demand(
            storage,
            policy,
            run_id,
            counter_audience,
            watch_socket,
            open_runtime,
            status,
            TargetRefreshDemand::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn_with_target_refresh_demand<S, P>(
        storage: S,
        policy: P,
        run_id: TraceId,
        counter_audience: OpenCounterAudience,
        watch_socket: Arc<SharedWatchSocket>,
        open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
        status: Arc<dyn CoordinatorStatusSink>,
        target_refresh_demand: TargetRefreshDemand,
    ) -> Result<Self, OfficialRefreshRuntimeBuildError>
    where
        S: ProductStorageAccess + Clone + Send + 'static,
        P: RefreshPolicyProvider + Clone + Send + 'static,
    {
        Self::spawn_internal(
            storage,
            policy,
            run_id,
            counter_audience,
            watch_socket,
            open_runtime,
            status,
            target_refresh_demand,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn spawn_with_target_refresh_demand_and_prepared<S, P>(
        storage: S,
        policy: P,
        run_id: TraceId,
        counter_audience: OpenCounterAudience,
        watch_socket: Arc<SharedWatchSocket>,
        open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
        status: Arc<dyn CoordinatorStatusSink>,
        target_refresh_demand: TargetRefreshDemand,
        prepared_registry: Arc<PreparedServingRegistry>,
    ) -> Result<Self, OfficialRefreshRuntimeBuildError>
    where
        S: ProductStorageAccess + Clone + Send + 'static,
        P: RefreshPolicyProvider + Clone + Send + 'static,
    {
        Self::spawn_internal(
            storage,
            policy,
            run_id,
            counter_audience,
            watch_socket,
            open_runtime,
            status,
            target_refresh_demand,
            Some(prepared_registry),
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_internal<S, P>(
        storage: S,
        policy: P,
        run_id: TraceId,
        counter_audience: OpenCounterAudience,
        watch_socket: Arc<SharedWatchSocket>,
        open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
        status: Arc<dyn CoordinatorStatusSink>,
        target_refresh_demand: TargetRefreshDemand,
        prepared_registry: Option<Arc<PreparedServingRegistry>>,
    ) -> Result<Self, OfficialRefreshRuntimeBuildError>
    where
        S: ProductStorageAccess + Clone + Send + 'static,
        P: RefreshPolicyProvider + Clone + Send + 'static,
    {
        if ci_network_disabled() {
            status.mark_stopped();
            let (shutdown, _) = watch::channel(false);
            return Ok(Self {
                shutdown,
                task: None,
                prepared_rebuild: None,
                storage_interrupt: None,
            });
        }
        let storage_interrupt = storage
            .operational_interrupt_handle()
            .map_err(|_| OfficialRefreshRuntimeBuildError::StorageAccess)?;
        let discovery = RutgersDiscoveryClient::new_official()
            .map_err(OfficialRefreshRuntimeBuildError::Discovery)?;
        let membership = Arc::new(SelectorTargetMembership::new());
        let upstream = Arc::new(
            RutgersRefreshUpstream::new_official(membership.clone())
                .map_err(OfficialRefreshRuntimeBuildError::Refresh)?,
        );
        let (shutdown, mut shutdown_receiver) = watch::channel(false);
        let term_window_scope = match counter_audience {
            OpenCounterAudience::Local { .. } => RutgersTermWindowScope::Local,
            OpenCounterAudience::Public => RutgersTermWindowScope::Public,
        };
        let network_budget = Arc::new(Semaphore::new(crate::REFRESH_MAX_CONCURRENCY));
        let prepared_rebuild = prepared_registry.map(|registry| {
            PreparedServingRebuildRuntime::spawn(
                storage.clone(),
                policy.clone(),
                counter_audience,
                Arc::clone(&open_runtime),
                registry,
            )
        });
        let prepared_demand = prepared_rebuild
            .as_ref()
            .map(PreparedServingRebuildRuntime::demand);
        let task_prepared_demand = prepared_demand.clone();
        let task = tokio::spawn(async move {
            let mut ids = SystemTraceIdSource;
            let mut current_targets = restore_product_refresh_targets(&storage).unwrap_or_default();
            membership.replace(
                current_targets
                    .iter()
                    .map(|registration| registration.target().clone()),
            );
            if term_window_scope == RutgersTermWindowScope::Local
                && let Ok(window) =
                    RutgersTermWindow::at(OffsetDateTime::now_utc(), term_window_scope)
            {
                restore_manual_term_demand(
                    &storage,
                    &current_targets,
                    &target_refresh_demand,
                    &window,
                );
            }
            let mut coordinator_runtime: Option<RefreshRuntime> = None;
            let mut wait_before_attempt = Duration::ZERO;
            let mut active_window_terms =
                RutgersTermWindow::at(OffsetDateTime::now_utc(), term_window_scope)
                    .ok()
                    .map(|window| (window.current_term().clone(), window.next_term().clone()));

            loop {
                let mut wait_remaining = wait_before_attempt;
                while !wait_remaining.is_zero() {
                    let wait_slice = wait_remaining.min(TARGET_DEMAND_SCAN_INTERVAL);
                    tokio::select! {
                        changed = shutdown_receiver.changed() => {
                            if changed.is_err() || *shutdown_receiver.borrow_and_update() {
                                break;
                            }
                        }
                        _ = tokio::time::sleep(wait_slice) => {
                            wait_remaining = wait_remaining.saturating_sub(wait_slice);
                            if let Some(runtime) = &coordinator_runtime {
                                let demanded = target_refresh_demand.snapshot().unwrap_or_default();
                                let manual_terms = target_refresh_demand
                                    .manual_terms_snapshot()
                                    .unwrap_or_default();
                                let demanded_set = demanded.iter().cloned().collect::<BTreeSet<_>>();
                                let Ok(window) = RutgersTermWindow::at(
                                    OffsetDateTime::now_utc(),
                                    term_window_scope,
                                ) else {
                                    tracing::error!(code = "TERM_CALENDAR_UNAVAILABLE");
                                    continue;
                                };
                                let window_terms =
                                    (window.current_term().clone(), window.next_term().clone());
                                if active_window_terms
                                    .as_ref()
                                    .is_some_and(|active| active != &window_terms)
                                {
                                    active_window_terms = Some(window_terms);
                                    break;
                                }
                                active_window_terms = Some(window_terms);
                                let pending_manual_targets = pending_manual_refresh_targets(
                                    &storage,
                                    &current_targets,
                                    &manual_terms,
                                );
                                let complete_snapshot_targets = complete_snapshot_targets(
                                    &storage,
                                    &current_targets,
                                );
                                let Ok((active_targets, manual_targets)) =
                                    authoritative_refresh_targets(
                                    &current_targets,
                                    &demanded,
                                    &pending_manual_targets,
                                    Ok(window),
                                )
                                else {
                                    tracing::error!(code = "TERM_CALENDAR_UNAVAILABLE");
                                    continue;
                                };
                                for target in active_targets.iter().cloned() {
                                    let key = target.target().clone();
                                    let registration = runtime.register_target_with_snapshot_state(
                                        target,
                                        manual_targets.contains(&key),
                                        complete_snapshot_targets.contains(&key),
                                    );
                                    if registration.is_err() {
                                        tracing::error!(code = "SHARED_REFRESH_REGISTRATION_FAILED");
                                        continue;
                                    }
                                }
                                if retain_authoritative_refresh_targets(
                                    &active_targets,
                                    |retained| runtime.retain_registered_targets(retained),
                                )
                                .is_err()
                                {
                                    tracing::error!(code = "SHARED_REFRESH_TARGET_SYNC_FAILED");
                                }
                                if runtime.sync_product_demand(&demanded_set).is_err() {
                                    tracing::error!(code = "SHARED_TARGET_DEMAND_SYNC_FAILED");
                                }
                                dispatch_manual_target_retries(
                                    runtime,
                                    &target_refresh_demand,
                                );
                            }
                        }
                    }
                    if *shutdown_receiver.borrow() {
                        break;
                    }
                }
                if *shutdown_receiver.borrow() {
                    break;
                }

                publish_discovering(&*status);

                let observation_id = ids.next_trace_id();
                let started_at = system_timestamp();
                let request_id = ids.next_trace_id().to_string();
                let discovery_permit = tokio::select! {
                    changed = shutdown_receiver.changed() => {
                        if changed.is_err() || *shutdown_receiver.borrow_and_update() {
                            break;
                        }
                        continue;
                    }
                    permit = Arc::clone(&network_budget).acquire_owned() => {
                        permit.expect("the process network budget remains open")
                    }
                };
                let discovery_result = match &started_at {
                    Ok(started_at) => {
                        tokio::select! {
                            changed = shutdown_receiver.changed() => {
                                if changed.is_err() || *shutdown_receiver.borrow_and_update() {
                                    break;
                                }
                                continue;
                            }
                            result = discovery.fetch(&request_id, started_at) => result,
                        }
                    }
                    Err(()) => {
                        tracing::error!(code = "DISCOVERY_CLOCK_UNAVAILABLE");
                        wait_before_attempt = DISCOVERY_RETRY_INTERVAL;
                        publish_retry_wait(&*status, DISCOVERY_RETRY_INTERVAL);
                        continue;
                    }
                };
                drop(discovery_permit);
                let completed_at = match system_timestamp() {
                    Ok(completed_at) => completed_at,
                    Err(()) => {
                        tracing::error!(code = "DISCOVERY_CLOCK_UNAVAILABLE");
                        wait_before_attempt = DISCOVERY_RETRY_INTERVAL;
                        publish_retry_wait(&*status, DISCOVERY_RETRY_INTERVAL);
                        continue;
                    }
                };

                let discovery_succeeded = match discovery_result {
                    Ok(response) => {
                        let product_targets = product_refresh_targets(response.snapshot());
                        match publish_discovery_for_refresh_with_prepared(
                            &storage,
                            response.snapshot(),
                            observation_id,
                            started_at.expect("timestamp was validated above"),
                            completed_at,
                            task_prepared_demand.as_ref(),
                        ) {
                            Ok(_) if product_targets.is_ok() => {
                                let product_targets = product_targets
                                    .expect("the product target result was checked above");
                                membership.replace(
                                    product_targets
                                        .iter()
                                        .map(|registration| registration.target().clone()),
                                );
                                current_targets = product_targets;
                                wait_before_attempt =
                                    refresh_generation_interval(current_targets.len());
                                true
                            }
                            Err(_) => {
                                tracing::error!(code = "DISCOVERY_PUBLISH_FAILED");
                                wait_before_attempt = DISCOVERY_RETRY_INTERVAL;
                                false
                            }
                            Ok(_) => {
                                tracing::error!(
                                    code = "DISCOVERY_PRODUCT_TARGET_PROJECTION_FAILED"
                                );
                                wait_before_attempt = DISCOVERY_RETRY_INTERVAL;
                                false
                            }
                        }
                    }
                    Err(failure) => {
                        let persist_failure = || {
                            record_discovery_transport_failure(
                                &storage,
                                observation_id,
                                started_at.expect("timestamp was validated above"),
                                completed_at,
                                discovery_failure_code(&failure),
                            )
                        };
                        if let Some(rebuild) = &task_prepared_demand {
                            match rebuild.begin_publication() {
                                Ok(barrier) => {
                                    if persist_failure().is_ok() {
                                        barrier.commit();
                                    }
                                }
                                Err(error) => tracing::error!(
                                    ?error,
                                    code = "DISCOVERY_FAILURE_PUBLICATION_BARRIER_UNAVAILABLE"
                                ),
                            }
                        } else {
                            let _ = persist_failure();
                        }
                        tracing::warn!(code = discovery_failure_code(&failure));
                        wait_before_attempt = DISCOVERY_RETRY_INTERVAL;
                        false
                    }
                };
                if discovery_succeeded {
                    status.publish_activity(ServiceOperationV1 {
                        phase: ServiceOperationPhaseV1::Idle,
                        target: None,
                        started_at: None,
                        next_retry_at: None,
                    });
                } else {
                    publish_retry_wait(&*status, DISCOVERY_RETRY_INTERVAL);
                }

                let demanded = target_refresh_demand.snapshot().unwrap_or_default();
                let manual_terms = target_refresh_demand
                    .manual_terms_snapshot()
                    .unwrap_or_default();
                let pending_manual_targets =
                    pending_manual_refresh_targets(&storage, &current_targets, &manual_terms);
                let complete_snapshot_targets =
                    complete_snapshot_targets(&storage, &current_targets);
                let (active_targets, manual_targets) = match authoritative_refresh_targets(
                    &current_targets,
                    &demanded,
                    &pending_manual_targets,
                    RutgersTermWindow::at(OffsetDateTime::now_utc(), term_window_scope),
                ) {
                    Ok(selection) => selection,
                    Err(error) => {
                        tracing::error!(code = error.diagnostic_code());
                        // The bundled calendar being unavailable is not an authoritative empty
                        // discovery projection. Keep serving and maintaining the existing target
                        // set until a valid window can be computed again.
                        continue;
                    }
                };
                if let Some(runtime) = &coordinator_runtime {
                    for target in active_targets.iter().cloned() {
                        let key = target.target().clone();
                        let registration = runtime.register_target_with_snapshot_state(
                            target,
                            manual_targets.contains(&key),
                            complete_snapshot_targets.contains(&key),
                        );
                        if registration.is_err() {
                            tracing::error!(code = "SHARED_REFRESH_REGISTRATION_FAILED");
                        }
                    }
                    if retain_authoritative_refresh_targets(&active_targets, |retained| {
                        runtime.retain_registered_targets(retained)
                    })
                    .is_err()
                    {
                        tracing::error!(code = "SHARED_REFRESH_TARGET_SYNC_FAILED");
                    }
                    dispatch_manual_target_retries(runtime, &target_refresh_demand);
                    continue;
                }
                if active_targets.is_empty() {
                    continue;
                }
                let coordinator = SharedRefreshCoordinator::with_parts(
                    storage.clone(),
                    upstream.clone(),
                    policy.clone(),
                    SystemCoordinatorClock::default(),
                    SystemTraceIdSource,
                    run_id,
                    counter_audience,
                    watch_socket.clone(),
                    open_runtime.clone(),
                    status.clone(),
                );
                let coordinator = if let Some(rebuild) = &task_prepared_demand {
                    coordinator.with_prepared_rebuild_demand(rebuild.clone())
                } else {
                    coordinator
                };
                let coordinators = vec![coordinator];
                match RefreshRuntime::spawn_pool_with_target_states_and_budget(
                    coordinators,
                    active_targets,
                    &manual_targets,
                    &complete_snapshot_targets,
                    Arc::clone(&network_budget),
                ) {
                    Ok(runtime) => {
                        let demanded = demanded.iter().cloned().collect::<BTreeSet<_>>();
                        if runtime.sync_product_demand(&demanded).is_err() {
                            tracing::error!(code = "SHARED_PRODUCT_DEMAND_SYNC_FAILED");
                        }
                        dispatch_manual_target_retries(&runtime, &target_refresh_demand);
                        coordinator_runtime = Some(runtime);
                    }
                    Err(_) => tracing::error!(code = "SHARED_REFRESH_START_FAILED"),
                }
            }

            if let Some(runtime) = coordinator_runtime {
                runtime.shutdown().await;
            }
            status.mark_stopped();
        });
        Ok(Self {
            shutdown,
            task: Some(task),
            prepared_rebuild,
            storage_interrupt: Some(storage_interrupt),
        })
    }

    pub async fn shutdown(mut self) {
        self.shutdown.send_replace(true);
        if let Some(storage_interrupt) = self.storage_interrupt.take() {
            storage_interrupt.interrupt();
        }
        if let Some(prepared_rebuild) = self.prepared_rebuild.take() {
            prepared_rebuild.shutdown().await;
        }
        if let Some(task) = self.task.take()
            && !drain_refresh_supervisor(task, OFFICIAL_REFRESH_SHUTDOWN_TIMEOUT).await
        {
            tracing::warn!(
                target: "bcsp_official_refresh",
                "official refresh supervisor exceeded the bounded shutdown window and was aborted",
            );
        }
    }
}

async fn drain_refresh_supervisor(mut task: JoinHandle<()>, timeout: Duration) -> bool {
    if tokio::time::timeout(timeout, &mut task).await.is_ok() {
        return true;
    }
    task.abort();
    let _ = task.await;
    false
}

fn ci_network_disabled() -> bool {
    std::env::var_os(CI_NO_RUTGERS_ENVIRONMENT)
        .is_some_and(|value| value == std::ffi::OsStr::new("1"))
}

impl Drop for OfficialRefreshRuntime {
    fn drop(&mut self) {
        self.shutdown.send_replace(true);
        if let Some(storage_interrupt) = self.storage_interrupt.take() {
            storage_interrupt.interrupt();
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
        self.prepared_rebuild.take();
    }
}

fn selected_refresh_targets(
    available: &[crate::ScheduledRefreshTarget],
    demanded: &[TermCampusKey],
    pending_manual_targets: &BTreeSet<TermCampusKey>,
    window: &RutgersTermWindow,
) -> Vec<crate::ScheduledRefreshTarget> {
    let visible_terms = window
        .visible_terms()
        .iter()
        .map(|term| term.term().clone())
        .collect::<BTreeSet<_>>();
    let automatic_terms =
        BTreeSet::from([window.current_term().clone(), window.next_term().clone()]);
    let demanded = demanded.iter().cloned().collect::<BTreeSet<_>>();
    available
        .iter()
        .filter(|registration| {
            is_product_campus(registration.target().campus().as_str())
                && (automatic_terms.contains(registration.target().term())
                    || (visible_terms.contains(registration.target().term())
                        && (demanded.contains(registration.target())
                            || pending_manual_targets.contains(registration.target()))))
        })
        .cloned()
        .collect()
}

fn authoritative_refresh_targets(
    available: &[crate::ScheduledRefreshTarget],
    demanded: &[TermCampusKey],
    pending_manual_targets: &BTreeSet<TermCampusKey>,
    window: Result<RutgersTermWindow, RutgersTermWindowError>,
) -> Result<(Vec<crate::ScheduledRefreshTarget>, BTreeSet<TermCampusKey>), RutgersTermWindowError> {
    let window = window?;
    let active_targets =
        selected_refresh_targets(available, demanded, pending_manual_targets, &window);
    let manual_targets = active_targets
        .iter()
        .map(|registration| registration.target())
        .filter(|target| !is_automatically_managed_target(target, &window))
        .cloned()
        .collect();
    Ok((active_targets, manual_targets))
}

fn retain_authoritative_refresh_targets<E>(
    active_targets: &[crate::ScheduledRefreshTarget],
    retain: impl FnOnce(&BTreeSet<TermCampusKey>) -> Result<(), E>,
) -> Result<(), E> {
    let retained = active_targets
        .iter()
        .map(|target| target.target().clone())
        .collect();
    retain(&retained)
}

fn is_automatically_managed_target(target: &TermCampusKey, window: &RutgersTermWindow) -> bool {
    target.term() == window.current_term() || target.term() == window.next_term()
}

fn pending_manual_refresh_targets<S>(
    storage: &S,
    available: &[crate::ScheduledRefreshTarget],
    manual_terms: &[bcsp_contracts::TermId],
) -> BTreeSet<TermCampusKey>
where
    S: ProductStorageAccess,
{
    let manual_terms = manual_terms.iter().cloned().collect::<BTreeSet<_>>();
    if manual_terms.is_empty() {
        return BTreeSet::new();
    }
    let requested = available
        .iter()
        .filter(|registration| {
            manual_terms.contains(registration.target().term())
                && is_product_campus(registration.target().campus().as_str())
        })
        .map(|registration| registration.target().clone())
        .collect::<BTreeSet<_>>();
    let Ok(storage) = storage.lock_operational() else {
        return requested;
    };
    requested
        .into_iter()
        .filter(|target| {
            !storage
                .complete_target_snapshot_state(target)
                .is_ok_and(|state| state.ready)
        })
        .collect()
}

fn dispatch_manual_target_retries(runtime: &RefreshRuntime, demand: &TargetRefreshDemand) {
    let Ok(retries) = demand.pending_manual_target_retries() else {
        tracing::error!(code = "SHARED_MANUAL_RETRY_DEMAND_UNAVAILABLE");
        return;
    };
    for retry in retries {
        match runtime.request_manual_target_retry(&retry) {
            Ok(_) => {
                if demand.acknowledge_manual_target_retry(&retry).is_err() {
                    tracing::error!(
                        code = "SHARED_MANUAL_RETRY_ACKNOWLEDGEMENT_FAILED",
                        target = ?retry.target(),
                        generation = retry.generation(),
                    );
                }
            }
            Err(crate::RefreshRuntimeRegistrationError::UnknownTarget) => {
                // Registration and discovery replacement are asynchronous. Keep the generation
                // pending so the next target scan can deliver it to the authoritative runtime.
            }
            Err(error) => tracing::error!(
                code = "SHARED_MANUAL_RETRY_DISPATCH_FAILED",
                target = ?retry.target(),
                generation = retry.generation(),
                error = ?error,
            ),
        }
    }
}

fn complete_snapshot_targets<S>(
    storage: &S,
    available: &[crate::ScheduledRefreshTarget],
) -> BTreeSet<TermCampusKey>
where
    S: ProductStorageAccess,
{
    let Ok(storage) = storage.lock_operational() else {
        return BTreeSet::new();
    };
    available
        .iter()
        .filter(|registration| {
            storage
                .complete_target_snapshot_state(registration.target())
                .is_ok_and(|state| state.ready)
        })
        .map(|registration| registration.target().clone())
        .collect()
}

fn restore_manual_term_demand<S>(
    storage: &S,
    available: &[crate::ScheduledRefreshTarget],
    demand: &TargetRefreshDemand,
    window: &RutgersTermWindow,
) where
    S: ProductStorageAccess,
{
    let manual_terms = window
        .visible_terms()
        .iter()
        .filter(|term| !term.auto_managed())
        .map(|term| term.term().clone())
        .collect::<BTreeSet<_>>();
    let Ok(storage) = storage.lock_operational() else {
        return;
    };
    let mut requested_terms = BTreeSet::new();
    for registration in available {
        if !manual_terms.contains(registration.target().term())
            || !is_product_campus(registration.target().campus().as_str())
        {
            continue;
        }
        let attempted = storage
            .target_state(registration.target())
            .ok()
            .flatten()
            .is_some_and(|state| state.last_attempt_sequence > 0);
        if attempted {
            requested_terms.insert(registration.target().term().clone());
        }
    }
    drop(storage);
    for term in requested_terms {
        let _ = demand.request_manual_term(term);
    }
}

fn system_timestamp() -> Result<String, ()> {
    OffsetDateTime::now_utc().format(&Rfc3339).map_err(|_| ())
}

fn publish_discovering(status: &dyn CoordinatorStatusSink) {
    status.publish_activity(ServiceOperationV1 {
        phase: ServiceOperationPhaseV1::Discovering,
        target: None,
        started_at: Some(OffsetDateTime::now_utc()),
        next_retry_at: None,
    });
}

fn publish_retry_wait(status: &dyn CoordinatorStatusSink, delay: Duration) {
    let now = OffsetDateTime::now_utc();
    let next_retry_at = time::Duration::try_from(delay)
        .ok()
        .and_then(|delay| now.checked_add(delay));
    status.publish_activity(ServiceOperationV1 {
        phase: ServiceOperationPhaseV1::RetryWait,
        target: None,
        started_at: Some(now),
        next_retry_at,
    });
}

fn discovery_failure_code(failure: &DiscoveryFailure) -> &'static str {
    match failure.kind() {
        DiscoveryTransportError::Timeout => "DISCOVERY_UPSTREAM_TIMEOUT",
        DiscoveryTransportError::Network => "DISCOVERY_UPSTREAM_NETWORK",
        DiscoveryTransportError::NonSuccessHttp { .. } => "DISCOVERY_UPSTREAM_HTTP",
        DiscoveryTransportError::RequestConstruction
        | DiscoveryTransportError::ResponseUrlMismatch
        | DiscoveryTransportError::Redirect { .. }
        | DiscoveryTransportError::InvalidContentType { .. }
        | DiscoveryTransportError::ResponseTooLarge { .. }
        | DiscoveryTransportError::ContentDecoding { .. }
        | DiscoveryTransportError::InvalidBootstrap
        | DiscoveryTransportError::InvalidSelectorScriptUrl
        | DiscoveryTransportError::InvalidSelectorScript
        | DiscoveryTransportError::InvalidDiscoveryDocument => "DISCOVERY_UPSTREAM_PROTOCOL",
    }
}

#[derive(Debug, Error)]
pub enum OfficialRefreshRuntimeBuildError {
    #[error("the operational database could not expose its shutdown interrupt handle")]
    StorageAccess,
    #[error("the official Rutgers discovery client could not be built")]
    Discovery(#[source] DiscoveryClientBuildError),
    #[error("the official Rutgers refresh clients could not be built")]
    Refresh(#[source] RutgersRefreshUpstreamBuildError),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::time::{Duration, Instant};

    use bcsp_contracts::{ServiceOperationPhaseV1, ServiceRuntimeV1, TermCampusKey};
    use bcsp_domain::{RutgersTermWindow, RutgersTermWindowScope};
    use bcsp_rutgers_client::OpenSectionsRequest;
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

    use super::{
        authoritative_refresh_targets, drain_refresh_supervisor, is_automatically_managed_target,
        publish_discovering, publish_retry_wait, retain_authoritative_refresh_targets,
        selected_refresh_targets,
    };
    use crate::{ScheduledRefreshTarget, ServiceStatusRegistry};

    const SOURCE: &str =
        "selector:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    #[tokio::test]
    async fn non_cooperative_refresh_supervisor_has_a_bounded_shutdown() {
        let task = tokio::spawn(std::future::pending::<()>());
        let started = Instant::now();

        assert!(!drain_refresh_supervisor(task, Duration::from_millis(10)).await);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    fn registration(
        term: &str,
        campus: &str,
        year: u16,
        term_code: &str,
    ) -> ScheduledRefreshTarget {
        let target = TermCampusKey::try_new(term, campus).expect("target");
        let request = OpenSectionsRequest::try_from_persisted_discovery(
            SOURCE,
            target.term(),
            &target,
            Some(year),
            Some(term_code),
        )
        .expect("Open request");
        ScheduledRefreshTarget::try_new(target, request).expect("registration")
    }

    fn summer_2026_window() -> RutgersTermWindow {
        RutgersTermWindow::at(
            OffsetDateTime::parse("2026-07-17T12:00:00Z", &Rfc3339).expect("timestamp"),
            RutgersTermWindowScope::Local,
        )
        .expect("covered Rutgers calendar")
    }

    #[test]
    fn default_refresh_selects_current_and_next_terms_only() {
        let available = vec![
            registration("12027", "NWK", 2027, "1"),
            registration("92026", "NB", 2026, "9"),
            registration("72026", "NB", 2026, "7"),
            registration("92026", "CM", 2026, "9"),
        ];

        let selected =
            selected_refresh_targets(&available, &[], &BTreeSet::new(), &summer_2026_window())
                .into_iter()
                .map(|registration| registration.target().clone())
                .collect::<BTreeSet<_>>();
        assert_eq!(selected.len(), 3);
        assert!(
            selected
                .iter()
                .all(|target| matches!(target.term().as_str(), "72026" | "92026"))
        );
        assert!(
            selected
                .iter()
                .all(|target| matches!(target.campus().as_str(), "NB" | "NK" | "CM"))
        );
    }

    #[test]
    fn explicit_local_demand_can_add_one_visible_nonautomatic_term() {
        let demanded_target = TermCampusKey::try_new("02027", "NB").expect("target");
        let unavailable_target = TermCampusKey::try_new("12025", "NB").expect("target");
        let available = vec![
            registration("72026", "NB", 2026, "7"),
            registration("92026", "NB", 2026, "9"),
            registration("02027", "NB", 2027, "0"),
            registration("12025", "NB", 2025, "1"),
        ];

        let selected = selected_refresh_targets(
            &available,
            &[demanded_target.clone(), unavailable_target],
            &BTreeSet::new(),
            &summer_2026_window(),
        )
        .into_iter()
        .map(|registration| registration.target().clone())
        .collect::<BTreeSet<_>>();

        assert_eq!(selected.len(), 3);
        assert!(selected.contains(&demanded_target));
        assert!(
            !selected
                .iter()
                .any(|target| target.term().as_str() == "12025")
        );
    }

    #[test]
    fn requested_manual_term_only_requeues_targets_without_a_complete_snapshot() {
        let ready = TermCampusKey::try_new("02027", "NB").expect("ready target");
        let newly_discovered = TermCampusKey::try_new("02027", "CM").expect("new target");
        let available = vec![
            registration("72026", "NB", 2026, "7"),
            registration("92026", "NB", 2026, "9"),
            registration("02027", "NB", 2027, "0"),
            registration("02027", "CM", 2027, "0"),
        ];
        let selected = selected_refresh_targets(
            &available,
            &[],
            &BTreeSet::from([newly_discovered.clone()]),
            &summer_2026_window(),
        )
        .into_iter()
        .map(|registration| registration.target().clone())
        .collect::<BTreeSet<_>>();

        assert!(!selected.contains(&ready));
        assert!(selected.contains(&newly_discovered));
    }

    #[test]
    fn successful_empty_projection_is_authoritative_but_calendar_failure_is_not() {
        let (active, manual) =
            authoritative_refresh_targets(&[], &[], &BTreeSet::new(), Ok(summer_2026_window()))
                .expect("a valid term window makes an empty discovery projection authoritative");
        assert!(active.is_empty());
        assert!(manual.is_empty());
        let mut retained = None;
        retain_authoritative_refresh_targets(&active, |targets| {
            retained = Some(targets.clone());
            Ok::<(), ()>(())
        })
        .expect("an authoritative empty projection must still reach runtime retention");
        assert_eq!(retained, Some(BTreeSet::new()));

        let unavailable = RutgersTermWindow::at(
            OffsetDateTime::parse("2030-01-01T12:00:00Z", &Rfc3339).expect("timestamp"),
            RutgersTermWindowScope::Local,
        );
        assert!(authoritative_refresh_targets(&[], &[], &BTreeSet::new(), unavailable).is_err());
    }

    #[test]
    fn ready_out_of_band_term_remains_manual_when_product_demand_re_registers_it() {
        let window = summer_2026_window();
        let current = TermCampusKey::try_new("72026", "NB").expect("current target");
        let next = TermCampusKey::try_new("92026", "NB").expect("next target");
        let local_manual = TermCampusKey::try_new("02027", "NB").expect("manual target");

        assert!(is_automatically_managed_target(&current, &window));
        assert!(is_automatically_managed_target(&next, &window));
        assert!(!is_automatically_managed_target(&local_manual, &window));
    }

    #[test]
    fn discovery_activity_helpers_publish_discovering_and_retry_wait() {
        let status = ServiceStatusRegistry::new(ServiceRuntimeV1::Local);
        publish_discovering(&status);
        let discovering = status.snapshot().expect("discovering snapshot").operation;
        assert_eq!(discovering.phase, ServiceOperationPhaseV1::Discovering);
        assert!(discovering.started_at.is_some());
        assert!(discovering.next_retry_at.is_none());

        publish_retry_wait(&status, Duration::from_secs(30));
        let retry = status.snapshot().expect("retry snapshot").operation;
        assert_eq!(retry.phase, ServiceOperationPhaseV1::RetryWait);
        assert!(retry.started_at.is_some());
        assert!(retry.next_retry_at.is_some());
    }
}
