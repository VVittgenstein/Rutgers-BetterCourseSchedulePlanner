use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bcsp_contracts::{TermCampusKey, TermId};
use bcsp_domain::{RutgersTermWindow, RutgersTermWindowScope};
use bcsp_open::{
    CompletionSchedule, MonotonicTime, OriginDispatchHint, OriginJobKind, OriginSchedulerLane,
    RetryMode, TargetRetryClass, target_retry_directive,
};
use thiserror::Error;
use time::OffsetDateTime;
use tokio::sync::{Semaphore, mpsc, watch};
use tokio::task::{JoinHandle, JoinSet};

use crate::refresh_coordinator::{
    ConcurrentOpenExecution, ConcurrentOpenSchedule, TargetWorkflowControl,
};
use crate::{
    CoordinatorDispatchOutcome, CoordinatorError, ManualTargetRetryRequest, ProductStorageAccess,
    RefreshPolicyProvider, RefreshUpstream, ScheduledRefreshTarget, SharedRefreshCoordinator,
};

const REFRESH_COORDINATOR_POLL_INTERVAL: Duration = Duration::from_millis(250);
pub const REFRESH_MAX_CONCURRENCY: usize = 3;

enum RefreshRuntimeCommand {
    Register {
        target: ScheduledRefreshTarget,
        manual_one_shot: bool,
        complete_snapshot_ready: bool,
    },
    Reclassify {
        target: TermCampusKey,
        manual_one_shot: bool,
    },
    ActivateOpen(TermCampusKey),
    ManualRetry {
        target: TermCampusKey,
        generation: u64,
    },
    ReleaseProductDemand(TermCampusKey),
    RemoveTarget(TermCampusKey),
}

struct TargetCoordinator<C> {
    coordinator: C,
    registration: ScheduledRefreshTarget,
    control: Arc<TargetWorkflowControl>,
    manual_one_shot: bool,
    product_demand_active: bool,
}

struct ConcurrentOpenState {
    schedule: ConcurrentOpenSchedule,
    next_due: Instant,
    in_flight: bool,
    registration: ScheduledRefreshTarget,
    control: Arc<TargetWorkflowControl>,
}

type CoordinatorOutcome = Result<Option<CoordinatorDispatchOutcome>, CoordinatorError>;

enum RuntimeWorkflowCompletion<C> {
    Primary {
        target: TermCampusKey,
        kind: OriginJobKind,
        entry: TargetCoordinator<C>,
        outcome: CoordinatorOutcome,
    },
    ConcurrentOpen {
        target: TermCampusKey,
        outcome: Result<Option<ConcurrentOpenExecution>, CoordinatorError>,
    },
}

struct PendingPrimary<C> {
    entry: TargetCoordinator<C>,
    outcome: CoordinatorOutcome,
}

/// Lifecycle handle for one process-wide, bounded target supervisor.
///
/// Every registered target contributes its next due workflow to one global priority selection.
/// At most three selected workflows execute concurrently; there are no reserved workers or
/// static target-to-worker partitions.
pub struct RefreshRuntime {
    shutdown: watch::Sender<bool>,
    command_sender: mpsc::UnboundedSender<RefreshRuntimeCommand>,
    registered_targets: Arc<Mutex<BTreeSet<TermCampusKey>>>,
    registered_target_modes: Arc<Mutex<BTreeMap<TermCampusKey, bool>>>,
    active_open_targets: Arc<Mutex<BTreeSet<TermCampusKey>>>,
    manual_retry_generations: Arc<Mutex<BTreeMap<TermCampusKey, u64>>>,
    task: Option<JoinHandle<()>>,
}

impl RefreshRuntime {
    pub fn spawn<S, U, P>(
        coordinator: SharedRefreshCoordinator<S, U, P>,
        targets: Vec<ScheduledRefreshTarget>,
    ) -> Result<Self, CoordinatorError>
    where
        S: ProductStorageAccess + Clone + Send + 'static,
        U: RefreshUpstream + Clone + Send + 'static,
        P: RefreshPolicyProvider + Clone + Send + 'static,
    {
        Self::spawn_pool(vec![coordinator], targets)
    }

    pub fn spawn_pool<S, U, P>(
        coordinators: Vec<SharedRefreshCoordinator<S, U, P>>,
        targets: Vec<ScheduledRefreshTarget>,
    ) -> Result<Self, CoordinatorError>
    where
        S: ProductStorageAccess + Clone + Send + 'static,
        U: RefreshUpstream + Clone + Send + 'static,
        P: RefreshPolicyProvider + Clone + Send + 'static,
    {
        Self::spawn_pool_with_manual_terms(coordinators, targets, &[])
    }

    pub fn spawn_pool_with_manual_terms<S, U, P>(
        coordinators: Vec<SharedRefreshCoordinator<S, U, P>>,
        targets: Vec<ScheduledRefreshTarget>,
        manual_terms: &[TermId],
    ) -> Result<Self, CoordinatorError>
    where
        S: ProductStorageAccess + Clone + Send + 'static,
        U: RefreshUpstream + Clone + Send + 'static,
        P: RefreshPolicyProvider + Clone + Send + 'static,
    {
        Self::spawn_pool_with_manual_terms_and_budget(
            coordinators,
            targets,
            manual_terms,
            Arc::new(Semaphore::new(REFRESH_MAX_CONCURRENCY)),
        )
    }

    pub(crate) fn spawn_pool_with_manual_terms_and_budget<S, U, P>(
        coordinators: Vec<SharedRefreshCoordinator<S, U, P>>,
        targets: Vec<ScheduledRefreshTarget>,
        manual_terms: &[TermId],
        network_budget: Arc<Semaphore>,
    ) -> Result<Self, CoordinatorError>
    where
        S: ProductStorageAccess + Clone + Send + 'static,
        U: RefreshUpstream + Clone + Send + 'static,
        P: RefreshPolicyProvider + Clone + Send + 'static,
    {
        let manual_terms = manual_terms.iter().cloned().collect::<BTreeSet<_>>();
        let manual_targets = targets
            .iter()
            .filter(|target| manual_terms.contains(target.target().term()))
            .map(|target| target.target().clone())
            .collect::<BTreeSet<_>>();
        Self::spawn_pool_with_manual_targets_and_budget(
            coordinators,
            targets,
            &manual_targets,
            network_budget,
        )
    }

    pub(crate) fn spawn_pool_with_manual_targets_and_budget<S, U, P>(
        coordinators: Vec<SharedRefreshCoordinator<S, U, P>>,
        targets: Vec<ScheduledRefreshTarget>,
        manual_targets: &BTreeSet<TermCampusKey>,
        network_budget: Arc<Semaphore>,
    ) -> Result<Self, CoordinatorError>
    where
        S: ProductStorageAccess + Clone + Send + 'static,
        U: RefreshUpstream + Clone + Send + 'static,
        P: RefreshPolicyProvider + Clone + Send + 'static,
    {
        Self::spawn_pool_with_target_states_and_budget(
            coordinators,
            targets,
            manual_targets,
            &BTreeSet::new(),
            network_budget,
        )
    }

    pub(crate) fn spawn_pool_with_target_states_and_budget<S, U, P>(
        coordinators: Vec<SharedRefreshCoordinator<S, U, P>>,
        mut targets: Vec<ScheduledRefreshTarget>,
        manual_targets: &BTreeSet<TermCampusKey>,
        complete_snapshot_targets: &BTreeSet<TermCampusKey>,
        network_budget: Arc<Semaphore>,
    ) -> Result<Self, CoordinatorError>
    where
        S: ProductStorageAccess + Clone + Send + 'static,
        U: RefreshUpstream + Clone + Send + 'static,
        P: RefreshPolicyProvider + Clone + Send + 'static,
    {
        let template = coordinators
            .into_iter()
            .next()
            .ok_or(CoordinatorError::NoRefreshWorkers)?;
        targets.sort_by(|left, right| left.target().cmp(right.target()));

        let mut target_coordinators = BTreeMap::new();
        for target in targets {
            let key = target.target().clone();
            if target_coordinators.contains_key(&key) {
                continue;
            }
            let manual_one_shot = manual_targets.contains(&key);
            let complete_snapshot_ready = complete_snapshot_targets.contains(&key);
            let mut coordinator = template.fork_empty();
            let control = Arc::new(TargetWorkflowControl::default());
            coordinator.attach_workflow_control(Arc::clone(&control));
            register_with_mode(
                &mut coordinator,
                target.clone(),
                manual_one_shot,
                complete_snapshot_ready,
            )?;
            target_coordinators.insert(
                key,
                TargetCoordinator {
                    coordinator,
                    registration: target,
                    control,
                    manual_one_shot,
                    product_demand_active: false,
                },
            );
        }

        let registered_targets = Arc::new(Mutex::new(
            target_coordinators.keys().cloned().collect::<BTreeSet<_>>(),
        ));
        let registered_target_modes = Arc::new(Mutex::new(
            target_coordinators
                .iter()
                .map(|(target, entry)| (target.clone(), entry.manual_one_shot))
                .collect::<BTreeMap<_, _>>(),
        ));
        let active_open_targets = Arc::new(Mutex::new(BTreeSet::new()));
        let manual_retry_generations = Arc::new(Mutex::new(BTreeMap::new()));
        let (shutdown, mut shutdown_receiver) = watch::channel(false);
        let (command_sender, mut command_receiver) =
            mpsc::unbounded_channel::<RefreshRuntimeCommand>();

        let task = tokio::spawn(async move {
            let mut desired_product_demand = BTreeSet::new();
            let mut removed_targets = BTreeSet::new();
            let mut running: JoinSet<RuntimeWorkflowCompletion<SharedRefreshCoordinator<S, U, P>>> =
                JoinSet::new();
            let mut concurrent_open = BTreeMap::<TermCampusKey, ConcurrentOpenState>::new();
            let mut pending_primary =
                BTreeMap::<TermCampusKey, PendingPrimary<SharedRefreshCoordinator<S, U, P>>>::new();
            let mut pending_reclassifications = BTreeMap::<TermCampusKey, bool>::new();
            let mut origin_pause = None::<Instant>;
            let mut interval = tokio::time::interval(REFRESH_COORDINATOR_POLL_INTERVAL);
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    changed = shutdown_receiver.changed() => {
                        if changed.is_err() || *shutdown_receiver.borrow_and_update() {
                            break;
                        }
                    }
                    command = command_receiver.recv() => {
                        let Some(command) = command else {
                            break;
                        };
                        match command {
                            RefreshRuntimeCommand::Register {
                                target,
                                manual_one_shot,
                                complete_snapshot_ready,
                            } => {
                                let key = target.target().clone();
                                if target_coordinators.contains_key(&key) {
                                    continue;
                                }
                                let mut coordinator = template.fork_empty();
                                let control = Arc::new(TargetWorkflowControl::default());
                                coordinator.attach_workflow_control(Arc::clone(&control));
                                if let Err(error) = register_with_mode(
                                    &mut coordinator,
                                    target.clone(),
                                    manual_one_shot,
                                    complete_snapshot_ready,
                                ) {
                                    tracing::error!(
                                        code = "SHARED_REFRESH_REGISTRATION_FAILED",
                                        target = ?key,
                                        error = ?error,
                                    );
                                    continue;
                                }
                                let product_demand_active =
                                    desired_product_demand.contains(&key);
                                if product_demand_active
                                    && coordinator.activate_open_target(&key).is_err()
                                {
                                    tracing::error!(
                                        code = "SHARED_OPEN_ACTIVATION_FAILED",
                                        target = ?key,
                                    );
                                }
                                target_coordinators.insert(
                                    key,
                                    TargetCoordinator {
                                        coordinator,
                                        registration: target,
                                        control,
                                        manual_one_shot,
                                        product_demand_active,
                                    },
                                );
                            }
                            RefreshRuntimeCommand::Reclassify {
                                target,
                                manual_one_shot,
                            } => {
                                if let Some(entry) = target_coordinators.get_mut(&target) {
                                    reconcile_manual_mode(&target, entry, manual_one_shot);
                                } else {
                                    pending_reclassifications.insert(target, manual_one_shot);
                                }
                            }
                            RefreshRuntimeCommand::ActivateOpen(target) => {
                                desired_product_demand.insert(target.clone());
                                if let Some(entry) = target_coordinators.get_mut(&target)
                                    && !entry.product_demand_active
                                {
                                    if entry.coordinator.activate_open_target(&target).is_ok() {
                                        entry.product_demand_active = true;
                                    } else {
                                        tracing::error!(
                                            code = "SHARED_OPEN_ACTIVATION_FAILED",
                                            target = ?target,
                                        );
                                    }
                                }
                            }
                            RefreshRuntimeCommand::ManualRetry { target, generation } => {
                                if let Some(entry) = target_coordinators.get_mut(&target)
                                    && let Err(error) =
                                        entry.coordinator.request_manual_target_retry(&target)
                                {
                                    tracing::error!(
                                        code = "SHARED_MANUAL_RETRY_FAILED",
                                        target = ?target,
                                        generation,
                                        error = ?error,
                                    );
                                }
                                // A registered target absent from the idle map is already running;
                                // consuming this generation is the required active-target dedupe.
                            }
                            RefreshRuntimeCommand::ReleaseProductDemand(target) => {
                                desired_product_demand.remove(&target);
                                if let Some(entry) = target_coordinators.get_mut(&target)
                                    && entry.product_demand_active
                                {
                                    if entry
                                        .coordinator
                                        .release_product_demand_target(&target)
                                        .is_ok()
                                    {
                                        entry.product_demand_active = false;
                                    } else {
                                        tracing::error!(
                                            code = "SHARED_TARGET_DEMOTION_FAILED",
                                            target = ?target,
                                        );
                                    }
                                }
                            }
                            RefreshRuntimeCommand::RemoveTarget(target) => {
                                desired_product_demand.remove(&target);
                                pending_reclassifications.remove(&target);
                                if let Some(entry) = target_coordinators.remove(&target) {
                                    entry.coordinator.stop();
                                } else {
                                    removed_targets.insert(target);
                                }
                            }
                        }
                    }
                    completed = running.join_next(), if !running.is_empty() => {
                        let Some(completed) = completed else {
                            continue;
                        };
                        let Ok(completed) = completed else {
                            tracing::error!(code = "SHARED_REFRESH_WORKFLOW_JOIN_FAILED");
                            continue;
                        };
                        match completed {
                            RuntimeWorkflowCompletion::Primary {
                                target,
                                kind,
                                entry,
                                outcome,
                            } => {
                                entry.control.mark_idle();
                                if kind == OriginJobKind::Catalog
                                    && concurrent_open
                                        .get(&target)
                                        .is_some_and(|state| state.in_flight)
                                {
                                    pending_primary.insert(
                                        target,
                                        PendingPrimary { entry, outcome },
                                    );
                                    continue;
                                }
                                let concurrent = (kind == OriginJobKind::Catalog)
                                    .then(|| concurrent_open.remove(&target))
                                    .flatten();
                                finish_primary_workflow(
                                    target,
                                    entry,
                                    outcome,
                                    concurrent,
                                    &desired_product_demand,
                                    &mut pending_reclassifications,
                                    &mut removed_targets,
                                    &mut origin_pause,
                                    &mut target_coordinators,
                                );
                            }
                            RuntimeWorkflowCompletion::ConcurrentOpen { target, outcome } => {
                                if let Some(state) = concurrent_open.get_mut(&target) {
                                    state.in_flight = false;
                                    update_concurrent_open_state(
                                        &target,
                                        state,
                                        outcome,
                                        &mut origin_pause,
                                    );
                                }
                                if let Some(pending) = pending_primary.remove(&target) {
                                    let concurrent = concurrent_open.remove(&target);
                                    finish_primary_workflow(
                                        target,
                                        pending.entry,
                                        pending.outcome,
                                        concurrent,
                                        &desired_product_demand,
                                        &mut pending_reclassifications,
                                        &mut removed_targets,
                                        &mut origin_pause,
                                        &mut target_coordinators,
                                    );
                                }
                            }
                        }
                    }
                    _ = interval.tick() => {
                        clear_expired_origin_pause(&mut origin_pause);
                        if origin_pause.is_some() {
                            continue;
                        }
                        while running.len() < REFRESH_MAX_CONCURRENCY {
                            let now = Instant::now();
                            let parallel_open_target = concurrent_open
                                .iter()
                                .filter(|(_, state)| {
                                    !state.in_flight
                                        && state.next_due <= now
                                        && state.control.allows_parallel_serving_open()
                                })
                                .min_by_key(|(target, state)| {
                                    (
                                        if target.campus().as_str().eq_ignore_ascii_case("NB") {
                                            0
                                        } else {
                                            1
                                        },
                                        state.next_due,
                                        (*target).clone(),
                                    )
                                })
                                .map(|(target, _)| target.clone());
                            if let Some(target) = parallel_open_target {
                                let Ok(network_permit) = Arc::clone(&network_budget)
                                    .try_acquire_owned()
                                else {
                                    break;
                                };
                                let state = concurrent_open
                                    .get_mut(&target)
                                    .expect("selected concurrent Open state remains present");
                                state.in_flight = true;
                                let registration = state.registration.clone();
                                let control = Arc::clone(&state.control);
                                let failure_streak = state.schedule.failure_streak;
                                let mut executor = template.fork_empty();
                                running.spawn(async move {
                                    let _permit = network_permit;
                                    let outcome = executor
                                        .execute_concurrent_serving_open(
                                            registration,
                                            control,
                                            failure_streak,
                                        )
                                        .await;
                                    RuntimeWorkflowCompletion::ConcurrentOpen { target, outcome }
                                });
                                continue;
                            }
                            let term_window = RutgersTermWindow::at(
                                OffsetDateTime::now_utc(),
                                RutgersTermWindowScope::Public,
                            ).ok();
                            let mut selected = None;
                            for (target, entry) in &mut target_coordinators {
                                let hint = match entry.coordinator.next_dispatch_hint() {
                                    Ok(Some(hint)) => hint,
                                    Ok(None) => continue,
                                    Err(error) => {
                                        tracing::error!(
                                            code = "SHARED_REFRESH_HINT_FAILED",
                                            target = ?target,
                                            error = ?error,
                                        );
                                        continue;
                                    }
                                };
                                let priority = global_workflow_priority(
                                    target,
                                    &hint,
                                    entry.manual_one_shot,
                                    entry.product_demand_active,
                                    term_window.as_ref(),
                                );
                                if selected.as_ref().is_none_or(|(_, _, selected_priority)| {
                                    priority < *selected_priority
                                }) {
                                    selected = Some((
                                        target.clone(),
                                        hint.key.kind,
                                        priority,
                                    ));
                                }
                            }
                            let Some((target, kind, _)) = selected else {
                                break;
                            };
                            let Ok(network_permit) = Arc::clone(&network_budget)
                                .try_acquire_owned()
                            else {
                                break;
                            };
                            let mut entry = target_coordinators
                                .remove(&target)
                                .expect("selected target coordinator remains idle");
                            if kind == OriginJobKind::Catalog {
                                match entry.coordinator.concurrent_open_schedule(&target) {
                                    Ok(Some(schedule)) => {
                                        concurrent_open.insert(
                                            target.clone(),
                                            ConcurrentOpenState {
                                                next_due: Instant::now()
                                                    .checked_add(schedule.next_due_after)
                                                    .unwrap_or_else(Instant::now),
                                                schedule,
                                                in_flight: false,
                                                registration: entry.registration.clone(),
                                                control: Arc::clone(&entry.control),
                                            },
                                        );
                                    }
                                    Ok(None) => {}
                                    Err(error) => tracing::error!(
                                        code = "SHARED_CONCURRENT_OPEN_STATE_FAILED",
                                        target = ?target,
                                        error = ?error,
                                    ),
                                }
                            }
                            running.spawn(async move {
                                let _permit = network_permit;
                                let outcome = entry.coordinator.run_next().await;
                                RuntimeWorkflowCompletion::Primary {
                                    target,
                                    kind,
                                    entry,
                                    outcome,
                                }
                            });
                        }
                    }
                }
            }

            running.abort_all();
            while running.join_next().await.is_some() {}
            for entry in target_coordinators.values() {
                entry.coordinator.stop();
            }
        });

        Ok(Self {
            shutdown,
            command_sender,
            registered_targets,
            registered_target_modes,
            active_open_targets,
            manual_retry_generations,
            task: Some(task),
        })
    }

    pub fn register_target(
        &self,
        target: ScheduledRefreshTarget,
    ) -> Result<bool, RefreshRuntimeRegistrationError> {
        self.register_target_with_mode(target, false)
    }

    pub fn register_manual_target(
        &self,
        target: ScheduledRefreshTarget,
    ) -> Result<bool, RefreshRuntimeRegistrationError> {
        self.register_target_with_mode(target, true)
    }

    fn register_target_with_mode(
        &self,
        target: ScheduledRefreshTarget,
        manual_one_shot: bool,
    ) -> Result<bool, RefreshRuntimeRegistrationError> {
        self.register_target_with_snapshot_state(target, manual_one_shot, false)
    }

    pub(crate) fn register_target_with_snapshot_state(
        &self,
        target: ScheduledRefreshTarget,
        manual_one_shot: bool,
        complete_snapshot_ready: bool,
    ) -> Result<bool, RefreshRuntimeRegistrationError> {
        let key = target.target().clone();
        let mut registered = self
            .registered_targets
            .lock()
            .map_err(|_| RefreshRuntimeRegistrationError::Unavailable)?;
        if !registered.insert(key.clone()) {
            drop(registered);
            let mut modes = self
                .registered_target_modes
                .lock()
                .map_err(|_| RefreshRuntimeRegistrationError::Unavailable)?;
            let previous = modes.insert(key.clone(), manual_one_shot);
            if previous == Some(manual_one_shot) {
                return Ok(false);
            }
            if self
                .command_sender
                .send(RefreshRuntimeCommand::Reclassify {
                    target: key.clone(),
                    manual_one_shot,
                })
                .is_err()
            {
                match previous {
                    Some(previous) => {
                        modes.insert(key, previous);
                    }
                    None => {
                        modes.remove(&key);
                    }
                }
                return Err(RefreshRuntimeRegistrationError::Stopped);
            }
            return Ok(false);
        }
        self.registered_target_modes
            .lock()
            .map_err(|_| RefreshRuntimeRegistrationError::Unavailable)?
            .insert(key.clone(), manual_one_shot);
        if self
            .command_sender
            .send(RefreshRuntimeCommand::Register {
                target,
                manual_one_shot,
                complete_snapshot_ready,
            })
            .is_err()
        {
            registered.remove(&key);
            self.registered_target_modes
                .lock()
                .map_err(|_| RefreshRuntimeRegistrationError::Unavailable)?
                .remove(&key);
            return Err(RefreshRuntimeRegistrationError::Stopped);
        }
        Ok(true)
    }

    pub fn activate_open(
        &self,
        target: &TermCampusKey,
    ) -> Result<bool, RefreshRuntimeRegistrationError> {
        if !self
            .registered_targets
            .lock()
            .map_err(|_| RefreshRuntimeRegistrationError::Unavailable)?
            .contains(target)
        {
            return Err(RefreshRuntimeRegistrationError::UnknownTarget);
        }
        let mut active = self
            .active_open_targets
            .lock()
            .map_err(|_| RefreshRuntimeRegistrationError::Unavailable)?;
        if !active.insert(target.clone()) {
            return Ok(false);
        }
        if self
            .command_sender
            .send(RefreshRuntimeCommand::ActivateOpen(target.clone()))
            .is_err()
        {
            active.remove(target);
            return Err(RefreshRuntimeRegistrationError::Stopped);
        }
        Ok(true)
    }

    pub fn request_manual_target_retry(
        &self,
        request: &ManualTargetRetryRequest,
    ) -> Result<bool, RefreshRuntimeRegistrationError> {
        if !self
            .registered_targets
            .lock()
            .map_err(|_| RefreshRuntimeRegistrationError::Unavailable)?
            .contains(request.target())
        {
            return Err(RefreshRuntimeRegistrationError::UnknownTarget);
        }
        let mut generations = self
            .manual_retry_generations
            .lock()
            .map_err(|_| RefreshRuntimeRegistrationError::Unavailable)?;
        let previous = generations.get(request.target()).copied();
        if previous.is_some_and(|generation| generation >= request.generation()) {
            return Ok(false);
        }
        generations.insert(request.target().clone(), request.generation());
        if self
            .command_sender
            .send(RefreshRuntimeCommand::ManualRetry {
                target: request.target().clone(),
                generation: request.generation(),
            })
            .is_err()
        {
            match previous {
                Some(previous) => {
                    generations.insert(request.target().clone(), previous);
                }
                None => {
                    generations.remove(request.target());
                }
            }
            return Err(RefreshRuntimeRegistrationError::Stopped);
        }
        Ok(true)
    }

    pub fn sync_product_demand(
        &self,
        demanded: &BTreeSet<TermCampusKey>,
    ) -> Result<(), RefreshRuntimeRegistrationError> {
        let registered = self
            .registered_targets
            .lock()
            .map_err(|_| RefreshRuntimeRegistrationError::Unavailable)?
            .clone();
        let mut active = self
            .active_open_targets
            .lock()
            .map_err(|_| RefreshRuntimeRegistrationError::Unavailable)?;
        for target in active.difference(demanded).cloned().collect::<Vec<_>>() {
            self.command_sender
                .send(RefreshRuntimeCommand::ReleaseProductDemand(target.clone()))
                .map_err(|_| RefreshRuntimeRegistrationError::Stopped)?;
            active.remove(&target);
        }
        drop(active);
        for target in demanded {
            if registered.contains(target) {
                self.activate_open(target)?;
            }
        }
        Ok(())
    }

    pub fn registered_targets(
        &self,
    ) -> Result<Vec<TermCampusKey>, RefreshRuntimeRegistrationError> {
        self.registered_targets
            .lock()
            .map_err(|_| RefreshRuntimeRegistrationError::Unavailable)
            .map(|targets| targets.iter().cloned().collect())
    }

    pub fn retain_registered_targets(
        &self,
        retained: &BTreeSet<TermCampusKey>,
    ) -> Result<(), RefreshRuntimeRegistrationError> {
        let removals = {
            let mut registered = self
                .registered_targets
                .lock()
                .map_err(|_| RefreshRuntimeRegistrationError::Unavailable)?;
            let removals = registered.difference(retained).cloned().collect::<Vec<_>>();
            for target in &removals {
                registered.remove(target);
            }
            removals
        };
        if !removals.is_empty() {
            let mut modes = self
                .registered_target_modes
                .lock()
                .map_err(|_| RefreshRuntimeRegistrationError::Unavailable)?;
            for target in &removals {
                modes.remove(target);
            }
        }
        if !removals.is_empty() {
            let mut active = self
                .active_open_targets
                .lock()
                .map_err(|_| RefreshRuntimeRegistrationError::Unavailable)?;
            for target in &removals {
                active.remove(target);
            }
        }
        for target in removals {
            self.command_sender
                .send(RefreshRuntimeCommand::RemoveTarget(target))
                .map_err(|_| RefreshRuntimeRegistrationError::Stopped)?;
        }
        Ok(())
    }

    pub async fn shutdown(mut self) {
        self.shutdown.send_replace(true);
        if let Some(task) = self.task.take() {
            let _ = task.await;
        }
    }
}

fn register_with_mode<S, U, P>(
    coordinator: &mut SharedRefreshCoordinator<S, U, P>,
    target: ScheduledRefreshTarget,
    manual_one_shot: bool,
    complete_snapshot_ready: bool,
) -> Result<(), CoordinatorError>
where
    S: ProductStorageAccess + Clone,
    U: RefreshUpstream,
    P: RefreshPolicyProvider,
{
    coordinator.register_target_with_snapshot_state(
        target,
        manual_one_shot,
        complete_snapshot_ready,
    )
}

fn reconcile_product_demand<S, U, P>(
    target: &TermCampusKey,
    entry: &mut TargetCoordinator<SharedRefreshCoordinator<S, U, P>>,
    desired: bool,
) where
    S: ProductStorageAccess + Clone,
    U: RefreshUpstream,
    P: RefreshPolicyProvider,
{
    if desired == entry.product_demand_active {
        return;
    }
    let result = if desired {
        entry.coordinator.activate_open_target(target)
    } else {
        entry.coordinator.release_product_demand_target(target)
    };
    if result.is_ok() {
        entry.product_demand_active = desired;
    } else {
        tracing::error!(
            code = if desired {
                "SHARED_OPEN_ACTIVATION_FAILED"
            } else {
                "SHARED_TARGET_DEMOTION_FAILED"
            },
            target = ?target,
        );
    }
}

fn reconcile_manual_mode<S, U, P>(
    target: &TermCampusKey,
    entry: &mut TargetCoordinator<SharedRefreshCoordinator<S, U, P>>,
    manual_one_shot: bool,
) where
    S: ProductStorageAccess + Clone,
    U: RefreshUpstream,
    P: RefreshPolicyProvider,
{
    if manual_one_shot == entry.manual_one_shot {
        return;
    }
    if entry
        .coordinator
        .set_manual_one_shot_mode(target, manual_one_shot)
        .is_ok()
    {
        entry.manual_one_shot = manual_one_shot;
    } else {
        tracing::error!(
            code = "SHARED_TARGET_RECLASSIFICATION_FAILED",
            target = ?target,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn finish_primary_workflow<S, U, P>(
    target: TermCampusKey,
    mut entry: TargetCoordinator<SharedRefreshCoordinator<S, U, P>>,
    outcome: CoordinatorOutcome,
    concurrent_open: Option<ConcurrentOpenState>,
    desired_product_demand: &BTreeSet<TermCampusKey>,
    pending_reclassifications: &mut BTreeMap<TermCampusKey, bool>,
    removed_targets: &mut BTreeSet<TermCampusKey>,
    origin_pause: &mut Option<Instant>,
    idle: &mut BTreeMap<TermCampusKey, TargetCoordinator<SharedRefreshCoordinator<S, U, P>>>,
) where
    S: ProductStorageAccess + Clone,
    U: RefreshUpstream,
    P: RefreshPolicyProvider,
{
    let published_candidate = matches!(
        &outcome,
        Ok(Some(CoordinatorDispatchOutcome::CatalogPublished {
            outcome: bcsp_operational_storage::PublishOutcome::AppliedChanged { .. }
                | bcsp_operational_storage::PublishOutcome::AppliedUnchanged { .. }
                | bcsp_operational_storage::PublishOutcome::InitialValidEmpty { .. },
            ..
        }))
    );
    if !published_candidate && let Some(mut concurrent_open) = concurrent_open {
        concurrent_open.schedule.next_due_after = concurrent_open
            .next_due
            .saturating_duration_since(Instant::now());
        if let Err(error) = entry
            .coordinator
            .adopt_concurrent_open_schedule(&target, concurrent_open.schedule)
        {
            tracing::error!(
                code = "SHARED_CONCURRENT_OPEN_ADOPTION_FAILED",
                target = ?target,
                error = ?error,
            );
        }
    }
    if let Err(error) = outcome {
        tracing::error!(
            code = "SHARED_REFRESH_COORDINATOR_FAILED",
            target = ?target,
            error = ?error,
        );
    }
    if let Some(delay) = entry.coordinator.origin_retry_after_remaining() {
        extend_origin_pause(origin_pause, delay);
    }
    if removed_targets.remove(&target) {
        pending_reclassifications.remove(&target);
        entry.coordinator.stop();
        return;
    }
    if let Some(manual_one_shot) = pending_reclassifications.remove(&target) {
        reconcile_manual_mode(&target, &mut entry, manual_one_shot);
    }
    reconcile_product_demand(
        &target,
        &mut entry,
        desired_product_demand.contains(&target),
    );
    idle.insert(target, entry);
}

fn update_concurrent_open_state(
    target: &TermCampusKey,
    state: &mut ConcurrentOpenState,
    outcome: Result<Option<ConcurrentOpenExecution>, CoordinatorError>,
    origin_pause: &mut Option<Instant>,
) {
    let directive = match outcome {
        Ok(Some(execution)) => match execution.completion {
            CompletionSchedule::Success => {
                state.schedule.failure_streak = 0;
                state.schedule.next_due_after = state.schedule.interval;
                state.next_due = Instant::now()
                    .checked_add(state.schedule.interval)
                    .unwrap_or_else(Instant::now);
                return;
            }
            CompletionSchedule::CatalogRace => {
                state.schedule.next_due_after = Duration::ZERO;
                state.next_due = Instant::now();
                return;
            }
            CompletionSchedule::Retry(directive) => directive,
        },
        Ok(None) => return,
        Err(error) => {
            tracing::error!(
                code = "SHARED_CONCURRENT_OPEN_FAILED",
                target = ?target,
                error = ?error,
            );
            target_retry_directive(
                target,
                state.schedule.failure_streak,
                TargetRetryClass::Transient,
            )
        }
    };
    state.schedule.failure_streak = directive.next_failure_streak;
    state.schedule.next_due_after = directive.delay;
    state.next_due = Instant::now()
        .checked_add(directive.delay)
        .unwrap_or_else(Instant::now);
    if directive.mode == RetryMode::OriginCircuit {
        extend_origin_pause(origin_pause, directive.delay);
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum GlobalWorkflowClass {
    ActiveWatchOpen,
    ManualCompleteSnapshot,
    AppliedOpen,
    CurrentInitialSnapshot,
    NextInitialSnapshot,
    AppliedCatalog,
    LowFrequencyMaintenance,
}

fn global_workflow_priority(
    target: &TermCampusKey,
    hint: &OriginDispatchHint,
    manual_one_shot: bool,
    product_demand_active: bool,
    term_window: Option<&RutgersTermWindow>,
) -> (GlobalWorkflowClass, u8, MonotonicTime, TermCampusKey) {
    let class = if hint.key.kind == OriginJobKind::Open
        && (hint.active_watch_count > 0 || hint.lane == OriginSchedulerLane::OpenActiveWatch)
    {
        GlobalWorkflowClass::ActiveWatchOpen
    } else if manual_one_shot && hint.key.kind == OriginJobKind::Catalog && hint.generation_pending
    {
        GlobalWorkflowClass::ManualCompleteSnapshot
    } else if product_demand_active && hint.key.kind == OriginJobKind::Open {
        GlobalWorkflowClass::AppliedOpen
    } else if hint.key.kind == OriginJobKind::Catalog
        && hint.generation_pending
        && term_window.is_some_and(|window| target.term() == window.current_term())
    {
        GlobalWorkflowClass::CurrentInitialSnapshot
    } else if hint.key.kind == OriginJobKind::Catalog
        && hint.generation_pending
        && term_window.is_some_and(|window| target.term() == window.next_term())
    {
        GlobalWorkflowClass::NextInitialSnapshot
    } else if product_demand_active && hint.key.kind == OriginJobKind::Catalog {
        GlobalWorkflowClass::AppliedCatalog
    } else {
        GlobalWorkflowClass::LowFrequencyMaintenance
    };
    (
        class,
        if target.campus().as_str().eq_ignore_ascii_case("NB") {
            0
        } else {
            1
        },
        hint.scheduled_due,
        target.clone(),
    )
}

fn clear_expired_origin_pause(pause: &mut Option<Instant>) {
    if pause.is_some_and(|until| until <= Instant::now()) {
        *pause = None;
    }
}

fn extend_origin_pause(pause: &mut Option<Instant>, delay: Duration) {
    let until = Instant::now()
        .checked_add(delay)
        .unwrap_or_else(Instant::now);
    if pause.is_none_or(|current| current < until) {
        *pause = Some(until);
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RefreshRuntimeRegistrationError {
    #[error("refresh runtime is stopped")]
    Stopped,
    #[error("refresh runtime registration state is unavailable")]
    Unavailable,
    #[error("refresh target is not registered")]
    UnknownTarget,
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
    use std::sync::atomic::{AtomicUsize, Ordering};

    use bcsp_open::{GeneralOpenInterval, OpenCounterAudience};
    use bcsp_operational_storage::OperationalStorage;
    use bcsp_rutgers_client::{OpenSectionsFailure, OpenSectionsRequest, OpenSectionsResponse};
    use bcsp_watch::WatchStartAdmission;

    use super::*;
    use crate::{
        CoordinatorStatusSink, CoordinatorStatusSnapshot, SystemCoordinatorClock,
        WorkflowOperationActivity, WorkflowOperationId,
    };

    const SOURCE: &str =
        "selector:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn registration_for(campus: &str) -> ScheduledRefreshTarget {
        let target = TermCampusKey::try_new("92026", campus).expect("target");
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

    #[derive(Clone, Default)]
    struct ConcurrentFailingUpstream {
        active: Arc<AtomicUsize>,
        maximum: Arc<AtomicUsize>,
        attempts: Arc<AtomicUsize>,
    }

    impl RefreshUpstream for ConcurrentFailingUpstream {
        fn fetch_catalog<'a>(
            &'a self,
            _target: &'a TermCampusKey,
        ) -> crate::RefreshFuture<'a, Result<crate::CatalogPullResponse, crate::CatalogPullFailure>>
        {
            Box::pin(async move {
                self.attempts.fetch_add(1, Ordering::SeqCst);
                let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
                self.maximum.fetch_max(active, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(75)).await;
                self.active.fetch_sub(1, Ordering::SeqCst);
                Err(crate::CatalogPullFailure::Transport)
            })
        }

        fn fetch_open<'a>(
            &'a self,
            _request: OpenSectionsRequest,
        ) -> crate::RefreshFuture<'a, Result<OpenSectionsResponse, OpenSectionsFailure>> {
            Box::pin(async move { unreachable!("Catalog failure cannot start candidate Open") })
        }
    }

    #[derive(Default)]
    struct WorkflowTrackingStatus {
        operations: Mutex<BTreeSet<WorkflowOperationId>>,
        maximum: AtomicUsize,
    }

    impl CoordinatorStatusSink for WorkflowTrackingStatus {
        fn publish(&self, _snapshot: CoordinatorStatusSnapshot) {}

        fn mark_stopped(&self) {
            self.operations.lock().expect("operations").clear();
        }

        fn publish_workflow_operation(&self, operation: WorkflowOperationActivity) {
            let mut operations = self.operations.lock().expect("operations");
            operations.insert(operation.id);
            self.maximum.fetch_max(operations.len(), Ordering::SeqCst);
        }

        fn clear_workflow_operation(&self, id: &WorkflowOperationId) {
            self.operations.lock().expect("operations").remove(id);
        }

        fn clear_target_workflow_operations(&self, target: &TermCampusKey) {
            self.operations
                .lock()
                .expect("operations")
                .retain(|operation| &operation.target != target);
        }
    }

    fn runtime_shell(
        registered: BTreeSet<TermCampusKey>,
    ) -> (
        RefreshRuntime,
        mpsc::UnboundedReceiver<RefreshRuntimeCommand>,
    ) {
        let (shutdown, _) = watch::channel(false);
        let (command_sender, command_receiver) = mpsc::unbounded_channel();
        let registered_modes = registered
            .iter()
            .cloned()
            .map(|target| (target, false))
            .collect();
        (
            RefreshRuntime {
                shutdown,
                command_sender,
                registered_targets: Arc::new(Mutex::new(registered)),
                registered_target_modes: Arc::new(Mutex::new(registered_modes)),
                active_open_targets: Arc::new(Mutex::new(BTreeSet::new())),
                manual_retry_generations: Arc::new(Mutex::new(BTreeMap::new())),
                task: None,
            },
            command_receiver,
        )
    }

    #[test]
    fn dynamic_registration_accepts_each_target_once() {
        let registration = registration_for("NB");
        let (runtime, mut commands) = runtime_shell(BTreeSet::new());
        assert!(
            runtime
                .register_target(registration.clone())
                .expect("first")
        );
        assert!(!runtime.register_target(registration).expect("duplicate"));
        assert!(matches!(
            commands.try_recv(),
            Ok(RefreshRuntimeCommand::Register {
                manual_one_shot: false,
                ..
            })
        ));
        assert!(commands.try_recv().is_err());
    }

    #[test]
    fn retained_target_reclassifies_when_the_term_window_role_changes() {
        let registration = registration_for("NB");
        let key = registration.target().clone();
        let (runtime, mut commands) = runtime_shell(BTreeSet::new());
        assert!(
            runtime
                .register_target(registration.clone())
                .expect("first")
        );
        let _ = commands.try_recv().expect("registration command");

        assert!(
            !runtime
                .register_manual_target(registration)
                .expect("reclassification")
        );
        assert!(matches!(
            commands.try_recv(),
            Ok(RefreshRuntimeCommand::Reclassify {
                target,
                manual_one_shot: true,
            }) if target == key
        ));
        assert!(commands.try_recv().is_err());
    }

    #[test]
    fn open_activation_accepts_each_target_once() {
        let target = registration_for("NB").target().clone();
        let (runtime, mut commands) = runtime_shell(BTreeSet::from([target.clone()]));
        assert!(runtime.activate_open(&target).expect("first"));
        assert!(!runtime.activate_open(&target).expect("duplicate"));
        assert!(matches!(
            commands.try_recv(),
            Ok(RefreshRuntimeCommand::ActivateOpen(received)) if received == target
        ));
    }

    #[test]
    fn manual_retry_generations_dedupe_per_target_and_keep_partial_targets_independent() {
        let nb = registration_for("NB").target().clone();
        let nk = registration_for("NK").target().clone();
        let cm = registration_for("CM").target().clone();
        let (runtime, mut commands) = runtime_shell(BTreeSet::from([nb.clone(), nk, cm.clone()]));
        let demand = crate::TargetRefreshDemand::default();
        assert_eq!(
            demand
                .request_manual_target_retries(&[nb.clone(), cm.clone()])
                .expect("partial target retry"),
            2,
        );
        let retries = demand
            .pending_manual_target_retries()
            .expect("retry generations");
        for retry in &retries {
            assert!(
                runtime
                    .request_manual_target_retry(retry)
                    .expect("first generation")
            );
            assert!(
                !runtime
                    .request_manual_target_retry(retry)
                    .expect("same generation is coalesced")
            );
        }
        let received = std::iter::from_fn(|| commands.try_recv().ok())
            .map(|command| match command {
                RefreshRuntimeCommand::ManualRetry { target, generation } => (target, generation),
                _ => panic!("only manual retry commands are expected"),
            })
            .collect::<Vec<_>>();
        assert_eq!(received.len(), 2);
        assert_eq!(
            received
                .iter()
                .map(|(target, _)| target.clone())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([nb, cm]),
            "the ready/active middle target is never included or fabricated",
        );
        assert!(received[0].1 < received[1].1);
    }

    #[test]
    fn global_priority_is_plan_order_then_nb_due_and_identity() {
        let target = TermCampusKey::try_new("92026", "NB").expect("target");
        let open = OriginDispatchHint {
            key: bcsp_open::OriginJobKey::open(target.clone()),
            lane: OriginSchedulerLane::OpenActiveWatch,
            scheduled_due: MonotonicTime::ZERO,
            active_watch_count: 1,
            generation_pending: false,
            failure_streak: 0,
        };
        let catalog = OriginDispatchHint {
            key: bcsp_open::OriginJobKey::catalog(target.clone()),
            lane: OriginSchedulerLane::Catalog,
            scheduled_due: MonotonicTime::ZERO,
            active_watch_count: 1,
            generation_pending: true,
            failure_streak: 0,
        };
        assert!(
            global_workflow_priority(&target, &open, false, true, None)
                < global_workflow_priority(&target, &catalog, true, true, None)
        );
        let other = TermCampusKey::try_new("92026", "AC").expect("other");
        assert!(
            global_workflow_priority(&target, &catalog, true, false, None)
                < global_workflow_priority(&other, &catalog, true, false, None)
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn one_global_queue_runs_at_most_three_target_workflows_concurrently() {
        let storage = Arc::new(Mutex::new(
            OperationalStorage::open_in_memory().expect("storage"),
        ));
        let upstream = ConcurrentFailingUpstream::default();
        let policy = crate::FixedRefreshPolicyProvider::new(
            crate::RefreshPolicy::try_new(Duration::from_secs(600), GeneralOpenInterval::public())
                .expect("policy"),
        );
        let watch = Arc::new(
            crate::SharedWatchSocket::try_new(
                Arc::new(|_: &bcsp_contracts::SectionKey| WatchStartAdmission::admitted(None)),
                Arc::new(crate::NoopWatchDispatchSink),
            )
            .expect("watch"),
        );
        let registry = Arc::new(crate::OpenRuntimeSnapshotRegistry::default());
        let run_id = "00000000-0000-4000-8000-000000000777"
            .parse()
            .expect("run ID");
        let status = Arc::new(WorkflowTrackingStatus::default());
        let coordinators = vec![SharedRefreshCoordinator::with_parts(
            Arc::clone(&storage),
            upstream.clone(),
            policy,
            SystemCoordinatorClock::default(),
            bcsp_contracts::SystemTraceIdSource,
            run_id,
            OpenCounterAudience::Public,
            Arc::clone(&watch),
            Arc::clone(&registry),
            status.clone(),
        )];
        let runtime = RefreshRuntime::spawn_pool(
            coordinators,
            vec![
                registration_for("NB"),
                registration_for("NK"),
                registration_for("CM"),
                registration_for("AC"),
            ],
        )
        .expect("runtime");
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(upstream.maximum.load(Ordering::SeqCst), 3);
        assert_eq!(status.maximum.load(Ordering::SeqCst), 3);
        assert!(status.maximum.load(Ordering::SeqCst) <= REFRESH_MAX_CONCURRENCY);
        assert_eq!(runtime.registered_targets().expect("targets").len(), 4);
        runtime.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn explicit_retry_restarts_an_idle_failed_target_and_dedupes_an_active_generation() {
        let storage = Arc::new(Mutex::new(
            OperationalStorage::open_in_memory().expect("storage"),
        ));
        let upstream = ConcurrentFailingUpstream::default();
        let policy = crate::FixedRefreshPolicyProvider::new(
            crate::RefreshPolicy::try_new(Duration::from_secs(600), GeneralOpenInterval::public())
                .expect("policy"),
        );
        let watch = Arc::new(
            crate::SharedWatchSocket::try_new(
                Arc::new(|_: &bcsp_contracts::SectionKey| WatchStartAdmission::admitted(None)),
                Arc::new(crate::NoopWatchDispatchSink),
            )
            .expect("watch"),
        );
        let registry = Arc::new(crate::OpenRuntimeSnapshotRegistry::default());
        let run_id = "00000000-0000-4000-8000-000000000779"
            .parse()
            .expect("run ID");
        let registration = registration_for("NB");
        let target = registration.target().clone();
        let coordinator = SharedRefreshCoordinator::new(
            storage,
            upstream.clone(),
            policy,
            run_id,
            OpenCounterAudience::Public,
            watch,
            registry,
        );
        let runtime = RefreshRuntime::spawn_pool_with_manual_targets_and_budget(
            vec![coordinator],
            vec![registration],
            &BTreeSet::from([target.clone()]),
            Arc::new(Semaphore::new(REFRESH_MAX_CONCURRENCY)),
        )
        .expect("runtime");

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if upstream.attempts.load(Ordering::SeqCst) == 1
                    && upstream.active.load(Ordering::SeqCst) == 0
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("first failed workflow becomes idle");
        tokio::time::sleep(Duration::from_millis(50)).await;

        let demand = crate::TargetRefreshDemand::default();
        assert_eq!(
            demand
                .request_manual_target_retries(std::slice::from_ref(&target))
                .expect("terminal retry generation"),
            1,
        );
        let first_retry = demand
            .pending_manual_target_retries()
            .expect("pending retry")
            .into_iter()
            .next()
            .expect("retry generation");
        assert!(
            runtime
                .request_manual_target_retry(&first_retry)
                .expect("retry command")
        );

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                if upstream.attempts.load(Ordering::SeqCst) >= 2
                    && upstream.active.load(Ordering::SeqCst) == 1
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("explicit command restarts before the five-second automatic backoff");

        assert!(
            demand
                .acknowledge_manual_target_retry(&first_retry)
                .expect("first generation acknowledged")
        );
        assert_eq!(
            demand
                .request_manual_target_retries(std::slice::from_ref(&target))
                .expect("active generation"),
            1,
        );
        let active_retry = demand
            .pending_manual_target_retries()
            .expect("active retry")
            .into_iter()
            .next()
            .expect("active retry generation");
        assert!(
            runtime
                .request_manual_target_retry(&active_retry)
                .expect("active command accepted for coalescing")
        );
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert_eq!(
            upstream.attempts.load(Ordering::SeqCst),
            2,
            "a retry generation received while the target is running is consumed without a duplicate workflow",
        );
        runtime.shutdown().await;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn discovery_permit_is_part_of_the_same_three_workflow_budget() {
        let storage = Arc::new(Mutex::new(
            OperationalStorage::open_in_memory().expect("storage"),
        ));
        let upstream = ConcurrentFailingUpstream::default();
        let policy = crate::FixedRefreshPolicyProvider::new(
            crate::RefreshPolicy::try_new(Duration::from_secs(600), GeneralOpenInterval::public())
                .expect("policy"),
        );
        let watch = Arc::new(
            crate::SharedWatchSocket::try_new(
                Arc::new(|_: &bcsp_contracts::SectionKey| WatchStartAdmission::admitted(None)),
                Arc::new(crate::NoopWatchDispatchSink),
            )
            .expect("watch"),
        );
        let registry = Arc::new(crate::OpenRuntimeSnapshotRegistry::default());
        let run_id = "00000000-0000-4000-8000-000000000778"
            .parse()
            .expect("run ID");
        let coordinator = SharedRefreshCoordinator::new(
            storage,
            upstream.clone(),
            policy,
            run_id,
            OpenCounterAudience::Public,
            watch,
            registry,
        );
        let budget = Arc::new(Semaphore::new(REFRESH_MAX_CONCURRENCY));
        let discovery_permit = Arc::clone(&budget)
            .acquire_owned()
            .await
            .expect("budget permit");
        let runtime = RefreshRuntime::spawn_pool_with_manual_targets_and_budget(
            vec![coordinator],
            vec![
                registration_for("NB"),
                registration_for("NK"),
                registration_for("CM"),
            ],
            &BTreeSet::new(),
            budget,
        )
        .expect("runtime");

        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(upstream.maximum.load(Ordering::SeqCst), 2);
        runtime.shutdown().await;
        drop(discovery_permit);
    }
}
