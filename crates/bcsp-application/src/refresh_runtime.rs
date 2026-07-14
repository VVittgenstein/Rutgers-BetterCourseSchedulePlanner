use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bcsp_contracts::TermCampusKey;
use thiserror::Error;
use tokio::sync::{mpsc, watch};
use tokio::task::JoinHandle;

use crate::{
    CoordinatorError, ProductStorageAccess, RefreshPolicyProvider, RefreshUpstream,
    ScheduledRefreshTarget, SharedRefreshCoordinator,
};

const REFRESH_COORDINATOR_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Lifecycle handle for the one shared Catalog/Open origin loop owned by an entrypoint.
pub struct RefreshRuntime {
    shutdown: watch::Sender<bool>,
    registration_sender: mpsc::UnboundedSender<ScheduledRefreshTarget>,
    registered_targets: Arc<Mutex<BTreeSet<TermCampusKey>>>,
    task: Option<JoinHandle<()>>,
}

impl RefreshRuntime {
    pub fn spawn<S, U, P>(
        mut coordinator: SharedRefreshCoordinator<S, U, P>,
        targets: Vec<ScheduledRefreshTarget>,
    ) -> Result<Self, CoordinatorError>
    where
        S: ProductStorageAccess + Clone + Send + 'static,
        U: RefreshUpstream + Send + 'static,
        P: RefreshPolicyProvider + Send + 'static,
    {
        let mut initial_targets = BTreeSet::new();
        for target in targets {
            if initial_targets.insert(target.target().clone()) {
                coordinator.register_target(target)?;
            }
        }
        let (shutdown, mut shutdown_receiver) = watch::channel(false);
        let (registration_sender, mut registration_receiver) =
            mpsc::unbounded_channel::<ScheduledRefreshTarget>();
        let registered_targets = Arc::new(Mutex::new(initial_targets));
        let task_registered_targets = registered_targets.clone();
        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(REFRESH_COORDINATOR_POLL_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    changed = shutdown_receiver.changed() => {
                        if changed.is_err() || *shutdown_receiver.borrow_and_update() {
                            break;
                        }
                    }
                    registration = registration_receiver.recv() => {
                        let Some(registration) = registration else {
                            break;
                        };
                        let target = registration.target().clone();
                        if coordinator.register_target(registration).is_err() {
                            if let Ok(mut registered) = task_registered_targets.lock() {
                                registered.remove(&target);
                            }
                            tracing::error!(code = "SHARED_REFRESH_REGISTRATION_FAILED");
                        }
                    }
                    _ = interval.tick() => {
                        if coordinator.run_next().await.is_err() {
                            tracing::error!(code = "SHARED_REFRESH_COORDINATOR_FAILED");
                        }
                    }
                }
            }
            coordinator.stop();
        });
        Ok(Self {
            shutdown,
            registration_sender,
            registered_targets,
            task: Some(task),
        })
    }

    /// Adds a discovered target to a running coordinator exactly once.
    ///
    /// `Ok(true)` means this runtime accepted a new registration; `Ok(false)` means the target was
    /// already active (including targets supplied to [`spawn`](Self::spawn)).
    pub fn register_target(
        &self,
        target: ScheduledRefreshTarget,
    ) -> Result<bool, RefreshRuntimeRegistrationError> {
        let key = target.target().clone();
        let mut registered = self
            .registered_targets
            .lock()
            .map_err(|_| RefreshRuntimeRegistrationError::Unavailable)?;
        if !registered.insert(key.clone()) {
            return Ok(false);
        }
        if self.registration_sender.send(target).is_err() {
            registered.remove(&key);
            return Err(RefreshRuntimeRegistrationError::Stopped);
        }
        Ok(true)
    }

    pub fn registered_targets(
        &self,
    ) -> Result<Vec<TermCampusKey>, RefreshRuntimeRegistrationError> {
        self.registered_targets
            .lock()
            .map_err(|_| RefreshRuntimeRegistrationError::Unavailable)
            .map(|targets| targets.iter().cloned().collect())
    }

    pub async fn shutdown(mut self) {
        self.shutdown.send_replace(true);
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RefreshRuntimeRegistrationError {
    #[error("refresh runtime is stopped")]
    Stopped,
    #[error("refresh runtime registration state is unavailable")]
    Unavailable,
}

impl Drop for RefreshRuntime {
    fn drop(&mut self) {
        self.shutdown.send_replace(true);
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use bcsp_contracts::TermCampusKey;
    use bcsp_rutgers_client::OpenSectionsRequest;

    use super::*;

    const SOURCE: &str =
        "selector:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn registration() -> ScheduledRefreshTarget {
        let target = TermCampusKey::try_new("92026", "NB").expect("target");
        let request = OpenSectionsRequest::try_from_persisted_discovery(
            SOURCE,
            target.term(),
            &target,
            Some(2026),
            Some("9"),
        )
        .expect("Open request");
        ScheduledRefreshTarget::try_new(target, request).expect("registration")
    }

    #[test]
    fn dynamic_registration_accepts_each_target_once() {
        let registration = registration();
        let (shutdown, _) = watch::channel(false);
        let (registration_sender, mut registration_receiver) = mpsc::unbounded_channel();
        let runtime = RefreshRuntime {
            shutdown,
            registration_sender,
            registered_targets: Arc::new(Mutex::new(BTreeSet::new())),
            task: None,
        };

        assert!(
            runtime
                .register_target(registration.clone())
                .expect("first registration")
        );
        assert!(
            !runtime
                .register_target(registration)
                .expect("duplicate registration")
        );
        assert!(registration_receiver.try_recv().is_ok());
        assert!(registration_receiver.try_recv().is_err());
    }
}
