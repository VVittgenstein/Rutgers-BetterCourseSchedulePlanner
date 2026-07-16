use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use bcsp_contracts::{
    ServiceOperationPhaseV1, ServiceOperationV1, SystemTraceIdSource, TermCampusKey, TraceId,
    TraceIdSource,
};
use bcsp_open::OpenCounterAudience;
use bcsp_rutgers_client::{
    DiscoveryClientBuildError, DiscoveryFailure, DiscoveryTransportError, RutgersDiscoveryClient,
};
use thiserror::Error;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::{
    CoordinatorStatusSink, OpenRuntimeSnapshotRegistry, ProductStorageAccess,
    RefreshPolicyProvider, RefreshRuntime, RutgersRefreshUpstream,
    RutgersRefreshUpstreamBuildError, SelectorTargetMembership, SharedRefreshCoordinator,
    SharedWatchSocket, SystemCoordinatorClock, TargetRefreshDemand, publish_discovery_for_refresh,
    record_discovery_transport_failure, restore_refresh_targets,
};

/// Discovery is deliberately lower frequency than Catalog and Open polling. All three still use
/// one serialized origin workflow: the coordinator is stopped before a discovery refresh begins.
pub const DISCOVERY_REFRESH_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
pub const DISCOVERY_RETRY_INTERVAL: Duration = Duration::from_secs(5 * 60);
const TARGET_DEMAND_SCAN_INTERVAL: Duration = Duration::from_millis(250);
const CI_NO_RUTGERS_ENVIRONMENT: &str = "BCSP_CI_NO_RUTGERS";

/// Production lifecycle for dynamic discovery plus the shared Catalog/Open coordinator.
pub struct OfficialRefreshRuntime {
    shutdown: watch::Sender<bool>,
    task: Option<JoinHandle<()>>,
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
        if ci_network_disabled() {
            status.mark_stopped();
            let (shutdown, _) = watch::channel(false);
            return Ok(Self {
                shutdown,
                task: None,
            });
        }
        let discovery = RutgersDiscoveryClient::new_official()
            .map_err(OfficialRefreshRuntimeBuildError::Discovery)?;
        let membership = Arc::new(SelectorTargetMembership::new());
        let upstream = Arc::new(
            RutgersRefreshUpstream::new_official(membership.clone())
                .map_err(OfficialRefreshRuntimeBuildError::Refresh)?,
        );
        let (shutdown, mut shutdown_receiver) = watch::channel(false);
        let task = tokio::spawn(async move {
            let mut ids = SystemTraceIdSource;
            let mut current_targets = restore_refresh_targets(&storage).unwrap_or_default();
            let mut coordinator_runtime: Option<RefreshRuntime> = None;
            let mut wait_before_attempt = Duration::ZERO;

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
                                for target in demanded_refresh_targets(&current_targets, &demanded) {
                                    let key = target.target().clone();
                                    if runtime.register_target(target).is_err() {
                                        tracing::error!(code = "SHARED_REFRESH_REGISTRATION_FAILED");
                                        continue;
                                    }
                                    if runtime.activate_open(&key).is_err() {
                                        tracing::error!(code = "SHARED_OPEN_ACTIVATION_FAILED");
                                    }
                                }
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

                // Let an in-flight Catalog/Open attempt reach its durable terminal state before
                // discovery touches the same Rutgers origin.
                if let Some(runtime) = coordinator_runtime.take() {
                    runtime.shutdown().await;
                }
                status.mark_stopped();
                publish_discovering(&*status);

                let observation_id = ids.next_trace_id();
                let started_at = system_timestamp();
                let request_id = ids.next_trace_id().to_string();
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
                    Ok(response) => match publish_discovery_for_refresh(
                        &storage,
                        response.snapshot(),
                        observation_id,
                        started_at.expect("timestamp was validated above"),
                        completed_at,
                    ) {
                        Ok(published) => {
                            membership.replace(
                                published
                                    .targets
                                    .iter()
                                    .map(|registration| registration.target().clone()),
                            );
                            current_targets = published.targets;
                            wait_before_attempt = DISCOVERY_REFRESH_INTERVAL;
                            true
                        }
                        Err(_) => {
                            membership.replace([]);
                            tracing::error!(code = "DISCOVERY_PUBLISH_FAILED");
                            wait_before_attempt = DISCOVERY_RETRY_INTERVAL;
                            false
                        }
                    },
                    Err(failure) => {
                        membership.replace([]);
                        let _ = record_discovery_transport_failure(
                            &storage,
                            observation_id,
                            started_at.expect("timestamp was validated above"),
                            completed_at,
                            discovery_failure_code(&failure),
                        );
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
                let active_targets = selected_refresh_targets(&current_targets, &demanded);
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
                match RefreshRuntime::spawn(coordinator, active_targets) {
                    Ok(runtime) => coordinator_runtime = Some(runtime),
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
        })
    }

    pub async fn shutdown(mut self) {
        self.shutdown.send_replace(true);
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

fn ci_network_disabled() -> bool {
    std::env::var_os(CI_NO_RUTGERS_ENVIRONMENT)
        .is_some_and(|value| value == std::ffi::OsStr::new("1"))
}

impl Drop for OfficialRefreshRuntime {
    fn drop(&mut self) {
        self.shutdown.send_replace(true);
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

fn selected_refresh_targets(
    available: &[crate::ScheduledRefreshTarget],
    demanded: &[TermCampusKey],
) -> Vec<crate::ScheduledRefreshTarget> {
    let latest_term = available
        .iter()
        .max_by_key(|registration| {
            (
                registration.open_request().year(),
                registration.open_request().term_code(),
            )
        })
        .map(|registration| registration.target().term().clone());
    let demanded = demanded.iter().cloned().collect::<BTreeSet<_>>();

    available
        .iter()
        .filter(|registration| {
            latest_term
                .as_ref()
                .is_some_and(|term| registration.target().term() == term)
                || demanded.contains(registration.target())
        })
        .cloned()
        .collect()
}

fn demanded_refresh_targets(
    available: &[crate::ScheduledRefreshTarget],
    demanded: &[TermCampusKey],
) -> Vec<crate::ScheduledRefreshTarget> {
    let demanded = demanded.iter().cloned().collect::<BTreeSet<_>>();
    available
        .iter()
        .filter(|registration| demanded.contains(registration.target()))
        .cloned()
        .collect()
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
    #[error("the official Rutgers discovery client could not be built")]
    Discovery(#[source] DiscoveryClientBuildError),
    #[error("the official Rutgers refresh clients could not be built")]
    Refresh(#[source] RutgersRefreshUpstreamBuildError),
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::time::Duration;

    use bcsp_contracts::{ServiceOperationPhaseV1, ServiceRuntimeV1, TermCampusKey};
    use bcsp_rutgers_client::OpenSectionsRequest;

    use super::{publish_discovering, publish_retry_wait, selected_refresh_targets};
    use crate::{ScheduledRefreshTarget, ServiceStatusRegistry};

    const SOURCE: &str =
        "selector:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

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

    #[test]
    fn default_refresh_selects_every_campus_in_latest_chronological_term() {
        let available = vec![
            registration("12027", "NWK", 2027, "1"),
            registration("92026", "NB", 2026, "9"),
            registration("12027", "NB", 2027, "1"),
            registration("92026", "CAMDEN", 2026, "9"),
        ];

        let selected = selected_refresh_targets(&available, &[])
            .into_iter()
            .map(|registration| registration.target().clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            selected,
            BTreeSet::from([
                TermCampusKey::try_new("12027", "NB").expect("target"),
                TermCampusKey::try_new("12027", "NWK").expect("target"),
            ])
        );
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
