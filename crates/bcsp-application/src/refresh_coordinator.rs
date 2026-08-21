use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use bcsp_catalog::{normalize_target, to_catalog_refresh_command};
use bcsp_contracts::{
    OpenCircuitState, OpenCircuitStatusV1, OpenSchedulerLane, ServiceOperationPhaseV1,
    ServiceOperationStageV2, ServiceOperationV1, ServiceTargetErrorV2, ServiceWorkStateV2,
    SystemTraceIdSource, TermCampusKey, TraceId, TraceIdSource,
};
use bcsp_open::{
    CompletionSchedule, MonotonicTime, OpenCounterAudience, OpenFailureKind, OpenGateRoute,
    OpenGateWiring, OpenPullClock, OpenPullCommand, OpenPullPersistence, OpenPullTerminal,
    OriginCircuit, OriginDispatch, OriginDispatchHint, OriginEdfScheduler, OriginJobKey,
    OriginJobKind, OriginSchedulerLane, RetryDirective, RetryMode, SchedulerError,
    SharedOpenService, SharedOpenServiceError, TargetGateSet, TargetRetryClass, retry_directive,
    target_retry_directive,
};
use bcsp_operational_storage::{
    BeginOpenPullAttemptCommand, BeginRefreshAttemptCommand, CatalogCandidateOpenSnapshot,
    CatalogFailureAudit, EmptySnapshotDecision, FinishOpenPullFailureCommand,
    FinishOpenPullSuccessCommand, FinishRefreshFailureCommand, InitialEmptyProof,
    OpenAttemptRecord, OpenCatalogSnapshot, OpenCommitOutcome, OperationalStorage, PublishOutcome,
    RefreshFailureStage, StorageError, StorageResult,
};
use bcsp_rutgers_client::{
    OpenSectionsFailure, OpenSectionsRequest, OpenSectionsResponse, RawCatalogCourse,
    SourceProvenance,
};
use bcsp_watch::WatchManagerError;
use thiserror::Error;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;
use tokio::sync::Mutex as AsyncMutex;

use crate::refresh_generation::refresh_generation_interval;
use crate::{
    OpenRuntimeSnapshot, OpenRuntimeSnapshotRegistry, OpenRuntimeSnapshotRegistryError,
    PreparedOpenPublicationBarrier, PreparedServingError, PreparedServingRebuildDemand,
    ProductStorageAccess, RefreshPolicy, RefreshPolicyProvider, RefreshPolicyReadError,
    SharedWatchSocket,
};

const STORAGE_LOCK_FIELD: &str = "operational_storage_lock";
const STORAGE_LOCK_REASON: &str = "unavailable";
const LOW_FREQUENCY_MAINTENANCE_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

fn first_load_freshness_interval_seconds(target_count: usize) -> u32 {
    u32::try_from(refresh_generation_interval(target_count).as_secs())
        .expect("the shared generation interval is capped to u32")
}

pub type RefreshFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// One decoded Catalog response ready for target-neutral normalization.
///
/// The coordinator intentionally accepts typed rows rather than response bytes. The Rutgers
/// transport remains responsible for bounds, content type, decoding, and provenance, while this
/// layer owns the shared normalize-and-publish path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogPullResponse {
    pub target: TermCampusKey,
    pub courses: Vec<RawCatalogCourse>,
    pub provenance: SourceProvenance,
    pub selector_confirms_target: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogPullFailure {
    Transport,
    RateLimited { retry_after: Option<Duration> },
    Schema,
}

impl CatalogPullFailure {
    const fn storage_stage(self) -> RefreshFailureStage {
        match self {
            Self::Transport | Self::RateLimited { .. } => RefreshFailureStage::Transport,
            Self::Schema => RefreshFailureStage::Schema,
        }
    }

    const fn storage_code(self) -> &'static str {
        match self {
            Self::Transport => "CATALOG_UPSTREAM_TRANSPORT",
            Self::RateLimited { .. } => "CATALOG_UPSTREAM_RATE_LIMITED",
            Self::Schema => "CATALOG_UPSTREAM_SCHEMA",
        }
    }
}

/// Injectable shared upstream seam. Production adapters use the bounded Rutgers clients; focused
/// integration tests use deterministic in-memory responses and never connect to Rutgers.
pub trait RefreshUpstream: Send + Sync {
    fn fetch_catalog<'a>(
        &'a self,
        target: &'a TermCampusKey,
    ) -> RefreshFuture<'a, Result<CatalogPullResponse, CatalogPullFailure>>;

    fn fetch_open<'a>(
        &'a self,
        request: OpenSectionsRequest,
    ) -> RefreshFuture<'a, Result<OpenSectionsResponse, OpenSectionsFailure>>;

    /// Consume response metadata captured by the immediately preceding Catalog fetch.
    ///
    /// Test doubles need not implement this side channel. Production keeps it target-keyed so the
    /// bounded worker pool can fetch different targets concurrently without changing the compact
    /// failure enum used by scheduler tests.
    fn take_catalog_audit(&self, _target: &TermCampusKey) -> Option<CatalogFailureAudit> {
        None
    }
}

impl<T> RefreshUpstream for Arc<T>
where
    T: RefreshUpstream + ?Sized,
{
    fn fetch_catalog<'a>(
        &'a self,
        target: &'a TermCampusKey,
    ) -> RefreshFuture<'a, Result<CatalogPullResponse, CatalogPullFailure>> {
        (**self).fetch_catalog(target)
    }

    fn fetch_open<'a>(
        &'a self,
        request: OpenSectionsRequest,
    ) -> RefreshFuture<'a, Result<OpenSectionsResponse, OpenSectionsFailure>> {
        (**self).fetch_open(request)
    }

    fn take_catalog_audit(&self, target: &TermCampusKey) -> Option<CatalogFailureAudit> {
        (**self).take_catalog_audit(target)
    }
}

pub trait CoordinatorClock: Send + Sync {
    fn monotonic_now(&self) -> MonotonicTime;

    fn wall_now(&self) -> OffsetDateTime;
}

#[derive(Clone, Debug)]
pub struct SystemCoordinatorClock {
    started: Instant,
}

impl Default for SystemCoordinatorClock {
    fn default() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

impl CoordinatorClock for SystemCoordinatorClock {
    fn monotonic_now(&self) -> MonotonicTime {
        MonotonicTime::from_millis(
            u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX),
        )
    }

    fn wall_now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

const WORKFLOW_IDLE: u8 = 0;
const WORKFLOW_CATALOG_FETCH: u8 = 1;
const WORKFLOW_CATALOG_PROCESS: u8 = 2;
const WORKFLOW_CANDIDATE_OPEN: u8 = 3;

#[derive(Debug, Default)]
pub(crate) struct TargetWorkflowControl {
    phase: AtomicU8,
    open_gate: AsyncMutex<()>,
    /// Snapshot-integrity gate runtimes for this target. The control is the
    /// stable per-target object shared by every execution path (primary,
    /// concurrent-serving, candidate), which is what gives the gate its
    /// per-target serial-lock property.
    integrity_gates: Arc<std::sync::Mutex<TargetGateSet>>,
}

impl TargetWorkflowControl {
    pub(crate) fn gate_wiring(&self, route: OpenGateRoute) -> OpenGateWiring {
        OpenGateWiring {
            gates: Arc::clone(&self.integrity_gates),
            route,
        }
    }

    pub(crate) fn allows_parallel_serving_open(&self) -> bool {
        matches!(
            self.phase.load(Ordering::Acquire),
            WORKFLOW_CATALOG_FETCH | WORKFLOW_CATALOG_PROCESS
        )
    }

    pub(crate) fn mark_idle(&self) {
        self.phase.store(WORKFLOW_IDLE, Ordering::Release);
    }

    fn mark_catalog_fetch(&self) {
        self.phase.store(WORKFLOW_CATALOG_FETCH, Ordering::Release);
    }

    fn mark_catalog_process(&self) {
        self.phase
            .store(WORKFLOW_CATALOG_PROCESS, Ordering::Release);
    }

    fn mark_candidate_open(&self) {
        self.phase.store(WORKFLOW_CANDIDATE_OPEN, Ordering::Release);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ConcurrentOpenSchedule {
    pub next_due_after: Duration,
    pub interval: Duration,
    pub failure_streak: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ConcurrentOpenExecution {
    pub completion: CompletionSchedule,
    pub terminal: OpenDispatchTerminal,
    pub attempt_id: TraceId,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CoordinatorStatusSnapshot {
    pub maximum_lag_milliseconds: u64,
    pub origin_circuit_open: bool,
    pub overloaded: bool,
    pub in_flight: bool,
}

/// Process-local work state for one target. Durable snapshot readiness remains storage-derived;
/// this value only explains what the bounded supervisor is doing now or when it will retry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetWorkActivity {
    pub target: TermCampusKey,
    pub work_state: ServiceWorkStateV2,
    pub stage: Option<ServiceOperationStageV2>,
    pub started_at: Option<OffsetDateTime>,
    pub next_retry_at: Option<OffsetDateTime>,
    pub error: Option<ServiceTargetErrorV2>,
}

/// Stable identity for one target-level network workflow. A complete snapshot may advance from
/// Catalog fetch through candidate Open without changing identity; a serving Open refresh has a
/// separate identity and can therefore be reported concurrently for the same target.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct WorkflowOperationId {
    pub target: TermCampusKey,
    pub kind: TargetWorkflowKind,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TargetWorkflowKind {
    CompleteSnapshot,
    OpenRefresh,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkflowOperationActivity {
    pub id: WorkflowOperationId,
    pub stage: ServiceOperationStageV2,
    pub started_at: OffsetDateTime,
}

/// Readiness bridge owned by an entrypoint. The public runtime can project these four aggregate
/// facts into its readiness surface without introducing a dependency from shared application code
/// back to the public crate.
pub trait CoordinatorStatusSink: Send + Sync + 'static {
    fn publish(&self, snapshot: CoordinatorStatusSnapshot);

    fn mark_stopped(&self);

    fn publish_activity(&self, _operation: ServiceOperationV1) {}

    fn publish_target_activity(&self, _activity: TargetWorkActivity) {}

    fn clear_target_activity(&self, _target: &TermCampusKey) {}

    fn publish_workflow_operation(&self, _operation: WorkflowOperationActivity) {}

    fn clear_workflow_operation(&self, _id: &WorkflowOperationId) {}

    fn clear_target_workflow_operations(&self, _target: &TermCampusKey) {}
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoopCoordinatorStatusSink;

impl CoordinatorStatusSink for NoopCoordinatorStatusSink {
    fn publish(&self, _snapshot: CoordinatorStatusSnapshot) {}

    fn mark_stopped(&self) {}
}

#[derive(Clone, Debug)]
pub struct ScheduledRefreshTarget {
    target: TermCampusKey,
    open_request: OpenSectionsRequest,
}

impl ScheduledRefreshTarget {
    pub fn try_new(
        target: TermCampusKey,
        open_request: OpenSectionsRequest,
    ) -> Result<Self, CoordinatorError> {
        if open_request.target().target() != target {
            return Err(CoordinatorError::OpenRequestTargetMismatch);
        }
        Ok(Self {
            target,
            open_request,
        })
    }

    pub const fn target(&self) -> &TermCampusKey {
        &self.target
    }

    pub const fn open_request(&self) -> &OpenSectionsRequest {
        &self.open_request
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenDispatchTerminal {
    Valid,
    Unsafe,
    CatalogRace,
    Failed,
}

impl From<&OpenPullTerminal> for OpenDispatchTerminal {
    fn from(terminal: &OpenPullTerminal) -> Self {
        match terminal {
            OpenPullTerminal::Valid(_) => Self::Valid,
            OpenPullTerminal::Unsafe(_) => Self::Unsafe,
            OpenPullTerminal::CatalogRace(_) => Self::CatalogRace,
            OpenPullTerminal::Failed(_) => Self::Failed,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoordinatorDispatchOutcome {
    CatalogPublished {
        target: TermCampusKey,
        outcome: PublishOutcome,
    },
    CatalogFailed {
        target: TermCampusKey,
        failure: CatalogPullFailure,
    },
    CompleteSnapshotOpenFailed {
        target: TermCampusKey,
        terminal: OpenDispatchTerminal,
        error_code: &'static str,
        trace_id: TraceId,
    },
    OpenCompleted {
        target: TermCampusKey,
        terminal: OpenDispatchTerminal,
        observation_count: usize,
    },
}

/// One shared, target-neutral coordinator for Catalog and Open work against the Rutgers origin.
///
/// `run_next` executes at most one due job. A host may call it from a small timer loop; the EDF
/// scheduler preserves origin concurrency one, skips missed catch-up bursts, and changes Open
/// cadence from authoritative shared watch demand.
pub struct SharedRefreshCoordinator<S, U, P, C = SystemCoordinatorClock, I = SystemTraceIdSource> {
    storage: S,
    upstream: U,
    policy: P,
    clock: C,
    ids: I,
    run_id: TraceId,
    counter_audience: OpenCounterAudience,
    watch: Arc<SharedWatchSocket>,
    open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
    status: Arc<dyn CoordinatorStatusSink>,
    workflow_control: Option<Arc<TargetWorkflowControl>>,
    prepared_rebuild: Option<PreparedServingRebuildDemand>,
    scheduler: OriginEdfScheduler,
    targets: BTreeMap<TermCampusKey, OpenSectionsRequest>,
    activated_targets: BTreeSet<TermCampusKey>,
    manual_one_shot_targets: BTreeSet<TermCampusKey>,
    catalog_failure_audits: BTreeMap<TermCampusKey, CatalogFailureAudit>,
    catalog_failure_observations: BTreeMap<TermCampusKey, TraceId>,
    maximum_lag_milliseconds: u64,
    overloaded: bool,
}

impl<S, U, P> SharedRefreshCoordinator<S, U, P, SystemCoordinatorClock, SystemTraceIdSource>
where
    S: ProductStorageAccess + Clone,
    U: RefreshUpstream + Clone,
    P: RefreshPolicyProvider + Clone,
{
    pub fn new(
        storage: S,
        upstream: U,
        policy: P,
        run_id: TraceId,
        counter_audience: OpenCounterAudience,
        watch: Arc<SharedWatchSocket>,
        open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
    ) -> Self {
        Self::with_parts(
            storage,
            upstream,
            policy,
            SystemCoordinatorClock::default(),
            SystemTraceIdSource,
            run_id,
            counter_audience,
            watch,
            open_runtime,
            Arc::new(NoopCoordinatorStatusSink),
        )
    }

    /// Creates an empty target-local coordinator that shares the same storage, upstream,
    /// policy, watch, status, and monotonic epoch. The bounded runtime uses these target-local
    /// schedulers as participants in one process-wide priority queue.
    pub(crate) fn fork_empty(&self) -> Self {
        let mut fork = Self::with_parts(
            self.storage.clone(),
            self.upstream.clone(),
            self.policy.clone(),
            self.clock.clone(),
            SystemTraceIdSource,
            self.run_id,
            self.counter_audience,
            Arc::clone(&self.watch),
            Arc::clone(&self.open_runtime),
            Arc::clone(&self.status),
        );
        fork.prepared_rebuild.clone_from(&self.prepared_rebuild);
        fork
    }
}

impl<S, U, P, C, I> SharedRefreshCoordinator<S, U, P, C, I>
where
    S: ProductStorageAccess + Clone,
    U: RefreshUpstream,
    P: RefreshPolicyProvider,
    C: CoordinatorClock,
    I: TraceIdSource,
{
    #[allow(clippy::too_many_arguments)]
    pub fn with_parts(
        storage: S,
        upstream: U,
        policy: P,
        clock: C,
        ids: I,
        run_id: TraceId,
        counter_audience: OpenCounterAudience,
        watch: Arc<SharedWatchSocket>,
        open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
        status: Arc<dyn CoordinatorStatusSink>,
    ) -> Self {
        Self {
            storage,
            upstream,
            policy,
            clock,
            ids,
            run_id,
            counter_audience,
            watch,
            open_runtime,
            status,
            workflow_control: None,
            prepared_rebuild: None,
            scheduler: OriginEdfScheduler::new(),
            targets: BTreeMap::new(),
            activated_targets: BTreeSet::new(),
            manual_one_shot_targets: BTreeSet::new(),
            catalog_failure_audits: BTreeMap::new(),
            catalog_failure_observations: BTreeMap::new(),
            maximum_lag_milliseconds: 0,
            overloaded: false,
        }
    }

    pub fn register_target(
        &mut self,
        registration: ScheduledRefreshTarget,
    ) -> Result<(), CoordinatorError> {
        self.register_target_with_mode(registration, false)
    }

    pub fn with_prepared_rebuild_demand(mut self, demand: PreparedServingRebuildDemand) -> Self {
        self.prepared_rebuild = Some(demand);
        self
    }

    pub fn register_manual_target(
        &mut self,
        registration: ScheduledRefreshTarget,
    ) -> Result<(), CoordinatorError> {
        self.register_target_with_mode(registration, true)
    }

    fn register_target_with_mode(
        &mut self,
        registration: ScheduledRefreshTarget,
        manual_one_shot: bool,
    ) -> Result<(), CoordinatorError> {
        self.register_target_with_snapshot_state(registration, manual_one_shot, false)
    }

    pub(crate) fn register_target_with_snapshot_state(
        &mut self,
        registration: ScheduledRefreshTarget,
        manual_one_shot: bool,
        complete_snapshot_ready: bool,
    ) -> Result<(), CoordinatorError> {
        if self.targets.contains_key(registration.target()) {
            return Ok(());
        }
        let policy = self.refresh_policy()?;
        let now = self.clock.monotonic_now();
        let active_watch_count = self.watch.active_watch_count(&registration.target);
        if complete_snapshot_ready {
            self.scheduler.register_catalog(
                registration.target.clone(),
                LOW_FREQUENCY_MAINTENANCE_INTERVAL,
                now.saturating_add(LOW_FREQUENCY_MAINTENANCE_INTERVAL),
            )?;
        } else if manual_one_shot {
            self.scheduler.register_catalog_one_shot(
                registration.target.clone(),
                active_watch_count,
                now,
            )?;
            self.manual_one_shot_targets
                .insert(registration.target.clone());
        } else {
            self.scheduler.register_catalog_initial(
                registration.target.clone(),
                LOW_FREQUENCY_MAINTENANCE_INTERVAL,
                active_watch_count,
                now,
            )?;
        }
        self.scheduler.register_open_initial(
            registration.target.clone(),
            policy.open_intervals(),
            active_watch_count,
            now,
            now.saturating_add(LOW_FREQUENCY_MAINTENANCE_INTERVAL),
        )?;
        if complete_snapshot_ready {
            self.scheduler
                .complete_open_bootstrap(&registration.target, now)?;
        }
        self.targets
            .insert(registration.target.clone(), registration.open_request);
        if complete_snapshot_ready {
            self.status.clear_target_activity(&registration.target);
        } else {
            self.status.publish_target_activity(TargetWorkActivity {
                target: registration.target.clone(),
                work_state: ServiceWorkStateV2::Queued,
                stage: Some(ServiceOperationStageV2::CatalogFetch),
                started_at: None,
                next_retry_at: None,
                error: None,
            });
        }
        self.publish_open_snapshot(&registration.target, now, None)?;
        self.publish_status(false);
        Ok(())
    }

    /// Promotes an already discovered target from its bounded initial Open observation to the
    /// normal recurring cadence demanded by product traffic.
    pub fn activate_open_target(&mut self, target: &TermCampusKey) -> Result<(), CoordinatorError> {
        if !self.targets.contains_key(target) {
            return Err(CoordinatorError::Scheduler(SchedulerError::UnknownJob));
        }
        let policy = self.refresh_policy()?;
        let now = self.clock.monotonic_now();
        self.scheduler
            .activate_open(target, policy.open_intervals(), now)?;
        self.scheduler
            .activate_catalog(target, policy.catalog_interval(), now)?;
        self.activated_targets.insert(target.clone());
        self.publish_open_snapshot(target, now, None)?;
        self.publish_status(false);
        Ok(())
    }

    /// Accepts one explicit Local retry for an idle target and makes its failed workflow due now.
    /// The scheduler is authoritative for origin cooldowns and in-flight coalescing.
    pub(crate) fn request_manual_target_retry(
        &mut self,
        target: &TermCampusKey,
    ) -> Result<bool, CoordinatorError> {
        if !self.targets.contains_key(target) {
            return Err(CoordinatorError::Scheduler(SchedulerError::UnknownJob));
        }
        let now = self.clock.monotonic_now();
        let Some(kind) = self.scheduler.request_manual_target_retry(target, now)? else {
            return Ok(false);
        };
        self.status.publish_target_activity(TargetWorkActivity {
            target: target.clone(),
            work_state: ServiceWorkStateV2::Queued,
            stage: Some(match kind {
                OriginJobKind::Catalog => ServiceOperationStageV2::CatalogFetch,
                OriginJobKind::Open => ServiceOperationStageV2::OpenFetch,
            }),
            started_at: None,
            next_retry_at: None,
            error: None,
        });
        self.publish_open_snapshot(target, now, None)?;
        self.publish_status(false);
        Ok(true)
    }

    pub fn release_product_demand_target(
        &mut self,
        target: &TermCampusKey,
    ) -> Result<(), CoordinatorError> {
        if !self.targets.contains_key(target) {
            return Err(CoordinatorError::Scheduler(SchedulerError::UnknownJob));
        }
        let policy = self.refresh_policy()?;
        let now = self.clock.monotonic_now();
        let maintenance = (!self.manual_one_shot_targets.contains(target))
            .then_some(LOW_FREQUENCY_MAINTENANCE_INTERVAL);
        self.scheduler.demote_catalog(target, maintenance, now)?;
        self.scheduler.demote_open(
            target,
            policy.open_intervals(),
            self.watch.active_watch_count(target),
            now,
        )?;
        self.activated_targets.remove(target);
        self.publish_open_snapshot(target, now, None)?;
        self.publish_status(false);
        Ok(())
    }

    /// Reclassifies a retained target when the Rutgers five-term window rolls.
    ///
    /// A Local manual one-shot that becomes current/next gains the automatic 24-hour
    /// maintenance cadence without losing an earlier retry due time. A target that rolls out of
    /// current/next is parked after its requested work unless product demand or an active watch
    /// still keeps it live.
    pub(crate) fn set_manual_one_shot_mode(
        &mut self,
        target: &TermCampusKey,
        manual_one_shot: bool,
    ) -> Result<(), CoordinatorError> {
        if !self.targets.contains_key(target) {
            return Err(CoordinatorError::Scheduler(SchedulerError::UnknownJob));
        }
        let was_manual = self.manual_one_shot_targets.contains(target);
        if was_manual == manual_one_shot {
            return Ok(());
        }
        if manual_one_shot {
            self.manual_one_shot_targets.insert(target.clone());
        } else {
            self.manual_one_shot_targets.remove(target);
        }
        if self.activated_targets.contains(target) {
            return Ok(());
        }

        let now = self.clock.monotonic_now();
        let policy = self.refresh_policy()?;
        if manual_one_shot {
            self.scheduler.demote_catalog(target, None, now)?;
        } else {
            self.scheduler
                .activate_catalog(target, LOW_FREQUENCY_MAINTENANCE_INTERVAL, now)?;
        }
        self.scheduler.demote_open(
            target,
            policy.open_intervals(),
            self.watch.active_watch_count(target),
            now,
        )?;
        self.publish_open_snapshot(target, now, None)?;
        self.publish_status(false);
        Ok(())
    }

    pub fn registered_targets(&self) -> Vec<TermCampusKey> {
        self.targets.keys().cloned().collect()
    }

    pub(crate) fn attach_workflow_control(&mut self, control: Arc<TargetWorkflowControl>) {
        self.workflow_control = Some(control);
    }

    pub(crate) fn concurrent_open_schedule(
        &mut self,
        target: &TermCampusKey,
    ) -> Result<Option<ConcurrentOpenSchedule>, CoordinatorError> {
        let policy = self.refresh_policy()?;
        let now = self.clock.monotonic_now();
        self.refresh_registered_policy(policy, now)?;
        if self.scheduler.active_watch_count(target).unwrap_or(0) == 0 {
            return Ok(None);
        }
        let key = OriginJobKey::open(target.clone());
        let Some(next_due) = self.scheduler.next_due(&key) else {
            return Ok(None);
        };
        Ok(Some(ConcurrentOpenSchedule {
            next_due_after: next_due.saturating_duration_since(now),
            interval: policy.open_intervals().effective(true),
            failure_streak: self.scheduler.failure_streak(&key).unwrap_or(0),
        }))
    }

    pub(crate) fn adopt_concurrent_open_schedule(
        &mut self,
        target: &TermCampusKey,
        schedule: ConcurrentOpenSchedule,
    ) -> Result<(), CoordinatorError> {
        let now = self.clock.monotonic_now();
        self.scheduler.adopt_external_open_state(
            target,
            now,
            now.saturating_add(schedule.next_due_after),
            schedule.failure_streak,
        )?;
        self.publish_open_snapshot(target, now, None)?;
        Ok(())
    }

    pub(crate) async fn execute_concurrent_serving_open(
        &mut self,
        registration: ScheduledRefreshTarget,
        control: Arc<TargetWorkflowControl>,
        current_failure_streak: u32,
    ) -> Result<Option<ConcurrentOpenExecution>, CoordinatorError> {
        let _gate = control.open_gate.lock().await;
        if !control.allows_parallel_serving_open() {
            return Ok(None);
        }
        let target = registration.target.clone();
        let active_watch_count = self.watch.active_watch_count(&target);
        if active_watch_count == 0 {
            return Ok(None);
        }
        let policy = self.refresh_policy()?;
        let attempt_id = self.ids.next_trace_id();
        let operation_id = WorkflowOperationId {
            target: target.clone(),
            kind: TargetWorkflowKind::OpenRefresh,
        };
        self.publish_workflow_running(&operation_id, ServiceOperationStageV2::OpenFetch);
        let execution = async {
            let command = OpenPullCommand {
                gate: Some(control.gate_wiring(OpenGateRoute::Serving)),
                attempt_id,
                run_id: self.run_id,
                source_request: registration.open_request,
                refresh_intervals: policy.open_intervals(),
                active_watch_count,
                freshness_interval_seconds: policy.open_intervals().effective_seconds(true),
                lane: OriginSchedulerLane::OpenActiveWatch,
                scheduler_lag: Duration::ZERO,
                current_failure_streak,
                counter_audience: self.counter_audience,
            };
            let mut persistence =
                ShortLockOpenPersistence::new(self.storage.clone(), self.prepared_rebuild.clone());
            let mut clock = CoordinatorOpenPullClock(&self.clock);
            let upstream = &self.upstream;
            let watched = Arc::clone(&self.watch);
            let execution = SharedOpenService::new(&mut persistence)
                .execute_with(
                    command,
                    &mut clock,
                    |request| upstream.fetch_open(request),
                    move |target| watched.watched_sections(target),
                )
                .await?;
            for observation in &execution.observations {
                self.watch.publish(observation.clone())?;
            }
            Ok(Some(ConcurrentOpenExecution {
                completion: execution.scheduler_completion,
                terminal: OpenDispatchTerminal::from(&execution.terminal),
                attempt_id,
            }))
        }
        .await;
        self.status.clear_workflow_operation(&operation_id);
        execution
    }

    pub(crate) fn next_dispatch_hint(
        &mut self,
    ) -> Result<Option<OriginDispatchHint>, CoordinatorError> {
        let policy = self.refresh_policy()?;
        let now = self.clock.monotonic_now();
        self.refresh_registered_policy(policy, now)?;
        Ok(self.scheduler.next_dispatch_hint(now))
    }

    /// Remaining origin-wide pause requested by an explicit upstream 429.
    pub fn origin_retry_after_remaining(&self) -> Option<Duration> {
        let now = self.clock.monotonic_now();
        match self.scheduler.circuit() {
            OriginCircuit::RetryAfter { until } if until > now => {
                Some(until.saturating_duration_since(now))
            }
            OriginCircuit::Closed
            | OriginCircuit::RetryAfter { .. }
            | OriginCircuit::FatalDiagnostic { .. } => None,
        }
    }

    pub fn stop(&self) {
        for target in self.targets.keys() {
            self.status.clear_target_activity(target);
            self.status.clear_target_workflow_operations(target);
        }
    }

    pub async fn run_next(
        &mut self,
    ) -> Result<Option<CoordinatorDispatchOutcome>, CoordinatorError> {
        let policy = self.refresh_policy()?;
        let now = self.clock.monotonic_now();
        self.refresh_registered_policy(policy, now)?;
        let Some(dispatch) = self.scheduler.start_next(now) else {
            self.publish_all_open_snapshots(now, None)?;
            self.publish_status(false);
            self.publish_activity(ServiceOperationPhaseV1::Idle, None, None);
            return Ok(None);
        };

        self.maximum_lag_milliseconds = duration_millis(dispatch.scheduler_lag);
        self.overloaded = dispatch.scheduler_lag > dispatch.requested_interval;
        self.publish_all_open_snapshots(now, Some(&dispatch))?;
        self.publish_status(true);
        self.publish_activity(
            match dispatch.key.kind {
                OriginJobKind::Catalog => ServiceOperationPhaseV1::CatalogFetch,
                OriginJobKind::Open => ServiceOperationPhaseV1::OpenFetch,
            },
            Some(dispatch.key.target.clone()),
            None,
        );
        let operation_id = workflow_operation_id(&dispatch.key);
        self.publish_workflow_running(
            &operation_id,
            match dispatch.key.kind {
                OriginJobKind::Catalog => ServiceOperationStageV2::CatalogFetch,
                OriginJobKind::Open => ServiceOperationStageV2::OpenFetch,
            },
        );

        let execution = match dispatch.key.kind {
            OriginJobKind::Catalog => self.execute_catalog(&dispatch, policy).await,
            OriginJobKind::Open => self.execute_open(&dispatch, policy).await,
        };
        self.status.clear_workflow_operation(&operation_id);

        match execution {
            Ok((outcome, completion, immediate_open_lane)) => {
                let completed_at = self.clock.monotonic_now();
                self.scheduler
                    .finish(dispatch.token, completed_at, completion)?;
                if let Some(lane) = immediate_open_lane {
                    let open_key = OriginJobKey::open(dispatch.key.target.clone());
                    if !self.scheduler.is_active(&open_key)
                        || self
                            .scheduler
                            .next_due(&open_key)
                            .is_some_and(|due| due > completed_at)
                    {
                        self.scheduler.request_open_due(
                            &dispatch.key.target,
                            completed_at,
                            lane,
                        )?;
                    }
                }
                self.publish_all_open_snapshots(completed_at, None)?;
                self.publish_status(false);
                let retrying = matches!(
                    &outcome,
                    CoordinatorDispatchOutcome::CatalogFailed { .. }
                        | CoordinatorDispatchOutcome::CompleteSnapshotOpenFailed { .. }
                        | CoordinatorDispatchOutcome::OpenCompleted {
                            terminal: OpenDispatchTerminal::Failed
                                | OpenDispatchTerminal::Unsafe
                                | OpenDispatchTerminal::CatalogRace,
                            ..
                        }
                );
                let next_retry_at = retrying
                    .then(|| self.scheduler.next_due(&dispatch.key))
                    .flatten()
                    .map(|due| wall_time_for_due(&self.clock, completed_at, due));
                self.publish_activity(
                    if retrying {
                        ServiceOperationPhaseV1::RetryWait
                    } else {
                        ServiceOperationPhaseV1::Idle
                    },
                    retrying.then_some(dispatch.key.target.clone()),
                    next_retry_at,
                );
                if retrying {
                    let catalog_audit = match &outcome {
                        CoordinatorDispatchOutcome::CatalogFailed { target, .. } => {
                            self.catalog_failure_audits.get(target)
                        }
                        _ => None,
                    };
                    let catalog_trace_id = match &outcome {
                        CoordinatorDispatchOutcome::CatalogFailed { target, .. } => {
                            self.catalog_failure_observations.get(target).copied()
                        }
                        _ => None,
                    };
                    let open_failure_attempt = if matches!(
                        &outcome,
                        CoordinatorDispatchOutcome::CompleteSnapshotOpenFailed { .. }
                            | CoordinatorDispatchOutcome::OpenCompleted { .. }
                    ) {
                        self.storage.lock_operational().ok().and_then(|storage| {
                            let state = storage
                                .open_batch_state(&dispatch.key.target)
                                .ok()
                                .flatten()?;
                            if state.last_failure_attempt_sequence
                                != Some(state.last_attempt_sequence)
                            {
                                return None;
                            }
                            state.last_attempt_id.and_then(|attempt_id| {
                                storage.open_attempt(&attempt_id).ok().flatten()
                            })
                        })
                    } else {
                        None
                    };
                    self.status.publish_target_activity(TargetWorkActivity {
                        target: dispatch.key.target.clone(),
                        work_state: ServiceWorkStateV2::RetryWait,
                        stage: Some(retry_stage_for_outcome(&outcome, dispatch.key.kind)),
                        started_at: None,
                        next_retry_at,
                        error: Some(target_error_for_outcome(
                            &outcome,
                            catalog_audit,
                            catalog_trace_id,
                            open_failure_attempt.as_ref(),
                        )),
                    });
                } else {
                    self.catalog_failure_audits.remove(&dispatch.key.target);
                    self.catalog_failure_observations
                        .remove(&dispatch.key.target);
                    self.status.clear_target_activity(&dispatch.key.target);
                }
                Ok(Some(outcome))
            }
            Err(error) => {
                let completed_at = self.clock.monotonic_now();
                let fallback = retry_directive(
                    &dispatch.key.target,
                    dispatch.requested_interval,
                    dispatch.failure_streak,
                    OpenFailureKind::Transient,
                );
                self.scheduler.finish(
                    dispatch.token,
                    completed_at,
                    CompletionSchedule::Retry(fallback),
                )?;
                self.publish_all_open_snapshots(completed_at, None)?;
                self.publish_status(false);
                let next_retry_at = self
                    .scheduler
                    .next_due(&dispatch.key)
                    .map(|due| wall_time_for_due(&self.clock, completed_at, due));
                self.publish_activity(
                    ServiceOperationPhaseV1::RetryWait,
                    Some(dispatch.key.target.clone()),
                    next_retry_at,
                );
                self.status.publish_target_activity(TargetWorkActivity {
                    target: dispatch.key.target.clone(),
                    work_state: ServiceWorkStateV2::RetryWait,
                    stage: Some(match dispatch.key.kind {
                        OriginJobKind::Catalog => ServiceOperationStageV2::CatalogFetch,
                        OriginJobKind::Open => ServiceOperationStageV2::OpenFetch,
                    }),
                    started_at: None,
                    next_retry_at,
                    error: Some(ServiceTargetErrorV2 {
                        code: "REFRESH_COORDINATOR_FAILED".to_owned(),
                        http_status: None,
                        content_type: None,
                        content_encoding: None,
                        decoded_bytes: None,
                        error_class: Some("INTERNAL".to_owned()),
                        error_chain: None,
                        trace_id: None,
                    }),
                });
                Err(error)
            }
        }
    }

    fn refresh_policy(&self) -> Result<RefreshPolicy, CoordinatorError> {
        self.policy
            .refresh_policy()
            .map_err(CoordinatorError::RefreshPolicy)
    }

    fn refresh_registered_policy(
        &mut self,
        policy: RefreshPolicy,
        now: MonotonicTime,
    ) -> Result<(), CoordinatorError> {
        for target in self.targets.keys() {
            let catalog_key = OriginJobKey::catalog(target.clone());
            if self.activated_targets.contains(target) {
                let catalog_due = self
                    .scheduler
                    .next_due(&catalog_key)
                    .unwrap_or(now.saturating_add(policy.catalog_interval()));
                self.scheduler.register_catalog(
                    target.clone(),
                    policy.catalog_interval(),
                    catalog_due,
                )?;
            } else if !self.manual_one_shot_targets.contains(target) {
                let catalog_due = self
                    .scheduler
                    .next_due(&catalog_key)
                    .ok_or(SchedulerError::UnknownJob)?;
                self.scheduler.register_catalog(
                    target.clone(),
                    LOW_FREQUENCY_MAINTENANCE_INTERVAL,
                    catalog_due,
                )?;
            }
            let active_watch_count = self.watch.active_watch_count(target);
            self.scheduler
                .set_catalog_watch_count(target, active_watch_count)?;
            self.scheduler.set_open_watch_count(
                target,
                policy.open_intervals(),
                active_watch_count,
                now,
            )?;
        }
        Ok(())
    }

    async fn execute_catalog(
        &mut self,
        dispatch: &OriginDispatch,
        policy: RefreshPolicy,
    ) -> Result<
        (
            CoordinatorDispatchOutcome,
            CompletionSchedule,
            Option<OriginSchedulerLane>,
        ),
        CoordinatorError,
    > {
        if let Some(control) = &self.workflow_control {
            control.mark_catalog_fetch();
        }
        let attempt_id = self.ids.next_trace_id();
        let started_at = format_timestamp(self.clock.wall_now())?;
        self.with_storage(|storage| {
            storage.begin_refresh_attempt(&BeginRefreshAttemptCommand {
                observation_id: attempt_id,
                target: dispatch.key.target.clone(),
                started_at: started_at.clone(),
                source_content_sha256: None,
                source_bytes: None,
            })
        })?;

        let response = self.upstream.fetch_catalog(&dispatch.key.target).await;
        let mut catalog_audit = self.upstream.take_catalog_audit(&dispatch.key.target);
        let completed_at = format_timestamp(self.clock.wall_now())?;
        let response = match response {
            Ok(response) if response.target == dispatch.key.target => response,
            Ok(response) => {
                set_catalog_audit_error(
                    &mut catalog_audit,
                    Some(response.provenance.decoded_bytes),
                    "Catalog response target did not match the requested target",
                );
                self.finish_catalog_failure(
                    attempt_id,
                    completed_at,
                    RefreshFailureStage::Schema,
                    Some(response.provenance.body_sha256),
                    Some(response.provenance.decoded_bytes),
                    "CATALOG_RESPONSE_TARGET_MISMATCH",
                    catalog_audit.as_ref(),
                )?;
                self.remember_catalog_failure_audit(
                    &dispatch.key.target,
                    attempt_id,
                    catalog_audit,
                );
                return Ok(self.catalog_failure_result(dispatch, CatalogPullFailure::Schema));
            }
            Err(failure) => {
                self.finish_catalog_failure(
                    attempt_id,
                    completed_at,
                    failure.storage_stage(),
                    None,
                    None,
                    failure.storage_code(),
                    catalog_audit.as_ref(),
                )?;
                self.remember_catalog_failure_audit(
                    &dispatch.key.target,
                    attempt_id,
                    catalog_audit,
                );
                return Ok(self.catalog_failure_result(dispatch, failure));
            }
        };
        self.catalog_failure_audits.remove(&dispatch.key.target);
        self.catalog_failure_observations
            .remove(&dispatch.key.target);

        self.publish_activity(
            ServiceOperationPhaseV1::CatalogProcess,
            Some(dispatch.key.target.clone()),
            None,
        );
        if let Some(control) = &self.workflow_control {
            control.mark_catalog_process();
        }
        self.publish_workflow_running(
            &workflow_operation_id(&dispatch.key),
            ServiceOperationStageV2::CatalogProcess,
        );
        let source_content_sha256 = response.provenance.body_sha256.clone();
        let source_bytes = response.provenance.decoded_bytes;
        let selector_confirms_target = response.selector_confirms_target;
        let command_completed_at = completed_at.clone();
        let command = tokio::task::spawn_blocking(move || {
            let normalized =
                normalize_target(response.target, response.courses, response.provenance)
                    .map_err(|_| CatalogPreparationFailure::Normalization)?;
            to_catalog_refresh_command(&normalized, attempt_id, started_at, command_completed_at)
                .map_err(|_| CatalogPreparationFailure::Mapping)
        })
        .await
        .map_err(|_| CoordinatorError::CatalogBlockingTask)?;
        let command = match command {
            Ok(command) => command,
            Err(CatalogPreparationFailure::Normalization) => {
                set_catalog_audit_error(
                    &mut catalog_audit,
                    Some(source_bytes),
                    "Catalog normalization failed",
                );
                self.finish_catalog_failure(
                    attempt_id,
                    completed_at,
                    RefreshFailureStage::Normalization,
                    Some(source_content_sha256),
                    Some(source_bytes),
                    "CATALOG_NORMALIZATION_FAILED",
                    catalog_audit.as_ref(),
                )?;
                self.remember_catalog_failure_audit(
                    &dispatch.key.target,
                    attempt_id,
                    catalog_audit,
                );
                return Ok(self.catalog_failure_result(dispatch, CatalogPullFailure::Schema));
            }
            Err(CatalogPreparationFailure::Mapping) => {
                set_catalog_audit_error(
                    &mut catalog_audit,
                    Some(source_bytes),
                    "Catalog normalized projection mapping failed",
                );
                self.finish_catalog_failure(
                    attempt_id,
                    completed_at,
                    RefreshFailureStage::Normalization,
                    Some(source_content_sha256),
                    Some(source_bytes),
                    "CATALOG_MAPPING_FAILED",
                    catalog_audit.as_ref(),
                )?;
                self.remember_catalog_failure_audit(
                    &dispatch.key.target,
                    attempt_id,
                    catalog_audit,
                );
                return Ok(self.catalog_failure_result(dispatch, CatalogPullFailure::Schema));
            }
        };
        self.publish_activity(
            ServiceOperationPhaseV1::CatalogPublish,
            Some(dispatch.key.target.clone()),
            None,
        );
        self.publish_workflow_running(
            &workflow_operation_id(&dispatch.key),
            ServiceOperationStageV2::SnapshotPublish,
        );
        let storage_access = self.storage.clone();
        let prepared_rebuild = self.prepared_rebuild.clone();
        let prepared = tokio::task::spawn_blocking(move || -> Result<_, CoordinatorError> {
            let mut storage = storage_access
                .lock_operational()
                .map_err(|_| CoordinatorError::StorageLock)?;
            let state = storage.target_state(&command.target)?;
            let empty_decision = if !command.snapshot.is_empty() {
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty
            } else if state
                .as_ref()
                .is_none_or(|state| state.current_content_version == 0)
            {
                if selector_confirms_target {
                    EmptySnapshotDecision::AcceptInitialSelectorConfirmedEmpty(
                        InitialEmptyProof::CurrentSelectorMembership,
                    )
                } else {
                    return Err(CoordinatorError::Storage(StorageError::InvalidCommand {
                        field: "selector_confirms_target",
                        reason: "initial empty Catalog requires current selector membership",
                    }));
                }
            } else if state
                .as_ref()
                .is_some_and(|state| state.counts.sections == 0)
            {
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty
            } else {
                EmptySnapshotDecision::RetainLastKnownGood
            };
            let observation_id = command.observation_id;
            storage.stage_catalog_refresh(command)?;
            match storage.candidate_open_catalog_snapshot(&observation_id, empty_decision)? {
                Some(candidate) => Ok(PreparedCompleteSnapshot::Candidate {
                    candidate,
                    empty_decision,
                }),
                None => {
                    let barrier = prepared_rebuild
                        .as_ref()
                        .map(PreparedServingRebuildDemand::begin_publication)
                        .transpose()?;
                    let outcome =
                        storage.publish_staged_refresh(&observation_id, empty_decision)?;
                    if let Some(barrier) = barrier {
                        barrier.commit();
                    }
                    Ok(PreparedCompleteSnapshot::Retained(outcome))
                }
            }
        })
        .await
        .map_err(|_| CoordinatorError::CatalogBlockingTask)??;
        let (candidate, empty_decision) = match prepared {
            PreparedCompleteSnapshot::Candidate {
                candidate,
                empty_decision,
            } => (candidate, empty_decision),
            PreparedCompleteSnapshot::Retained(outcome) => {
                return Ok((
                    CoordinatorDispatchOutcome::CatalogPublished {
                        target: dispatch.key.target.clone(),
                        outcome,
                    },
                    CompletionSchedule::Success,
                    None,
                ));
            }
        };

        let workflow_control = self.workflow_control.clone();
        let _candidate_open_gate = if let Some(control) = &workflow_control {
            control.mark_candidate_open();
            Some(control.open_gate.lock().await)
        } else {
            None
        };
        self.publish_activity(
            ServiceOperationPhaseV1::OpenFetch,
            Some(dispatch.key.target.clone()),
            None,
        );
        self.publish_workflow_running(
            &workflow_operation_id(&dispatch.key),
            ServiceOperationStageV2::OpenFetch,
        );
        let source_request = self
            .targets
            .get(&dispatch.key.target)
            .cloned()
            .ok_or(CoordinatorError::TargetNotRegistered)?;
        let active_watch_count = self.watch.active_watch_count(&dispatch.key.target);
        let freshness_interval_seconds = policy
            .open_intervals()
            .effective_seconds(active_watch_count > 0);
        let open_attempt_id = self.ids.next_trace_id();
        let command = OpenPullCommand {
            gate: self
                .workflow_control
                .as_ref()
                .map(|control| control.gate_wiring(OpenGateRoute::Candidate)),
            attempt_id: open_attempt_id,
            run_id: self.run_id,
            source_request,
            refresh_intervals: policy.open_intervals(),
            active_watch_count,
            freshness_interval_seconds,
            lane: if candidate.base_content_version == 0 {
                OriginSchedulerLane::OpenFirstLoad
            } else {
                OriginSchedulerLane::OpenCatalogRaceRecheck
            },
            scheduler_lag: dispatch.scheduler_lag,
            current_failure_streak: dispatch.failure_streak,
            counter_audience: self.counter_audience,
        };
        let mut persistence = ShortLockOpenPersistence::with_catalog_candidate(
            self.storage.clone(),
            candidate,
            empty_decision,
            self.prepared_rebuild.clone(),
        );
        let mut clock = CoordinatorOpenPullClock(&self.clock);
        let upstream = &self.upstream;
        let watched = Arc::clone(&self.watch);
        let execution = SharedOpenService::new(&mut persistence)
            .execute_with(
                command,
                &mut clock,
                |request| upstream.fetch_open(request),
                move |target| watched.watched_sections(target),
            )
            .await?;
        for observation in &execution.observations {
            self.watch.publish(observation.clone())?;
        }
        if let Some(outcome) = persistence.take_committed_catalog() {
            self.scheduler
                .complete_open_bootstrap(&dispatch.key.target, self.clock.monotonic_now())?;
            return Ok((
                CoordinatorDispatchOutcome::CatalogPublished {
                    target: dispatch.key.target.clone(),
                    outcome,
                },
                CompletionSchedule::Success,
                None,
            ));
        }
        let terminal = OpenDispatchTerminal::from(&execution.terminal);
        let error_code = match &execution.terminal {
            OpenPullTerminal::Failed(failure) => failure.diagnostic_code(),
            OpenPullTerminal::Unsafe(_) => "OPEN_RESPONSE_UNSAFE",
            OpenPullTerminal::CatalogRace(_) => "OPEN_CATALOG_RACE",
            OpenPullTerminal::Valid(_) => "OPEN_COMPLETE_SNAPSHOT_COMMIT_FAILED",
        };
        let completion = execution.scheduler_completion;
        Ok((
            CoordinatorDispatchOutcome::CompleteSnapshotOpenFailed {
                target: dispatch.key.target.clone(),
                terminal,
                error_code,
                trace_id: open_attempt_id,
            },
            completion,
            None,
        ))
    }

    fn catalog_failure_result(
        &self,
        dispatch: &OriginDispatch,
        failure: CatalogPullFailure,
    ) -> (
        CoordinatorDispatchOutcome,
        CompletionSchedule,
        Option<OriginSchedulerLane>,
    ) {
        let retry = catalog_retry_directive(&dispatch.key.target, dispatch.failure_streak, failure);
        (
            CoordinatorDispatchOutcome::CatalogFailed {
                target: dispatch.key.target.clone(),
                failure,
            },
            CompletionSchedule::Retry(retry),
            None,
        )
    }

    async fn execute_open(
        &mut self,
        dispatch: &OriginDispatch,
        policy: RefreshPolicy,
    ) -> Result<
        (
            CoordinatorDispatchOutcome,
            CompletionSchedule,
            Option<OriginSchedulerLane>,
        ),
        CoordinatorError,
    > {
        let source_request = self
            .targets
            .get(&dispatch.key.target)
            .cloned()
            .ok_or(CoordinatorError::TargetNotRegistered)?;
        let active_watch_count = self.watch.active_watch_count(&dispatch.key.target);
        let freshness_interval_seconds = if dispatch.first_load_one_shot && active_watch_count == 0
        {
            first_load_freshness_interval_seconds(self.targets.len())
        } else {
            policy
                .open_intervals()
                .effective_seconds(active_watch_count > 0)
        };
        let command = OpenPullCommand {
            gate: self
                .workflow_control
                .as_ref()
                .map(|control| control.gate_wiring(OpenGateRoute::Serving)),
            attempt_id: self.ids.next_trace_id(),
            run_id: self.run_id,
            source_request,
            refresh_intervals: policy.open_intervals(),
            active_watch_count,
            freshness_interval_seconds,
            lane: dispatch.lane,
            scheduler_lag: dispatch.scheduler_lag,
            current_failure_streak: dispatch.failure_streak,
            counter_audience: self.counter_audience,
        };
        let mut persistence =
            ShortLockOpenPersistence::new(self.storage.clone(), self.prepared_rebuild.clone());
        let mut clock = CoordinatorOpenPullClock(&self.clock);
        let upstream = &self.upstream;
        let watched = Arc::clone(&self.watch);
        let execution = SharedOpenService::new(&mut persistence)
            .execute_with(
                command,
                &mut clock,
                |request| upstream.fetch_open(request),
                move |target| watched.watched_sections(target),
            )
            .await?;
        let completion = execution.scheduler_completion;
        let immediate_open_lane = (completion == CompletionSchedule::CatalogRace)
            .then_some(OriginSchedulerLane::OpenCatalogRaceRecheck);
        for observation in &execution.observations {
            self.watch.publish(observation.clone())?;
        }
        let terminal = OpenDispatchTerminal::from(&execution.terminal);
        Ok((
            CoordinatorDispatchOutcome::OpenCompleted {
                target: dispatch.key.target.clone(),
                terminal,
                observation_count: execution.observations.len(),
            },
            completion,
            immediate_open_lane,
        ))
    }

    // This private adapter deliberately mirrors the durable failure command plus its
    // separately optional audit record; keeping those fields named at each call site
    // makes failure provenance reviewable.
    #[allow(clippy::too_many_arguments)]
    fn finish_catalog_failure(
        &self,
        observation_id: TraceId,
        completed_at: String,
        stage: RefreshFailureStage,
        source_content_sha256: Option<String>,
        source_bytes: Option<usize>,
        error_code: &'static str,
        audit: Option<&CatalogFailureAudit>,
    ) -> Result<(), CoordinatorError> {
        let source_bytes = source_bytes
            .map(u64::try_from)
            .transpose()
            .map_err(|_| CoordinatorError::SourceBytesOverflow)?;
        self.with_storage(|storage| {
            storage.finish_refresh_failure_with_audit(
                &FinishRefreshFailureCommand {
                    observation_id,
                    completed_at,
                    stage,
                    source_content_sha256,
                    source_bytes,
                    error_code: error_code.to_owned(),
                    diagnostic_token: None,
                },
                audit,
            )
        })
    }

    fn remember_catalog_failure_audit(
        &mut self,
        target: &TermCampusKey,
        observation_id: TraceId,
        audit: Option<CatalogFailureAudit>,
    ) {
        match audit {
            Some(audit) => {
                self.catalog_failure_audits.insert(target.clone(), audit);
                self.catalog_failure_observations
                    .insert(target.clone(), observation_id);
            }
            None => {
                self.catalog_failure_audits.remove(target);
                self.catalog_failure_observations.remove(target);
            }
        }
    }

    fn with_storage<T>(
        &self,
        operation: impl FnOnce(&mut OperationalStorage) -> StorageResult<T>,
    ) -> Result<T, CoordinatorError> {
        let mut storage = self
            .storage
            .lock_operational()
            .map_err(|_| CoordinatorError::StorageLock)?;
        operation(&mut storage).map_err(CoordinatorError::Storage)
    }

    fn publish_all_open_snapshots(
        &self,
        now: MonotonicTime,
        in_flight: Option<&OriginDispatch>,
    ) -> Result<(), CoordinatorError> {
        for target in self.targets.keys() {
            self.publish_open_snapshot(target, now, in_flight)?;
        }
        Ok(())
    }

    fn publish_open_snapshot(
        &self,
        target: &TermCampusKey,
        now: MonotonicTime,
        in_flight: Option<&OriginDispatch>,
    ) -> Result<(), CoordinatorError> {
        let key = OriginJobKey::open(target.clone());
        let active_watch_count = self.scheduler.active_watch_count(target).unwrap_or(0);
        let previous = self.open_runtime.snapshot(target)?;
        let active_dispatch = in_flight.filter(|dispatch| dispatch.key == key);
        let snapshot = OpenRuntimeSnapshot {
            lane: active_dispatch.map_or_else(
                || {
                    if active_watch_count > 0 {
                        OpenSchedulerLane::ActiveWatch
                    } else {
                        OpenSchedulerLane::General
                    }
                },
                |dispatch| contract_lane(dispatch.lane),
            ),
            active_watch_count,
            next_due_at: self
                .scheduler
                .next_due(&key)
                .map(|due| wall_time_for_due(&self.clock, now, due)),
            in_flight: active_dispatch.is_some(),
            scheduler_lag_milliseconds: active_dispatch
                .map_or(previous.scheduler_lag_milliseconds, |dispatch| {
                    duration_millis(dispatch.scheduler_lag)
                }),
            actual_start_to_start_interval_milliseconds: active_dispatch.map_or(
                previous.actual_start_to_start_interval_milliseconds,
                |dispatch| dispatch.actual_start_to_start_interval.map(duration_millis),
            ),
            failure_streak: self.scheduler.failure_streak(&key).unwrap_or(0),
            circuit: contract_circuit(
                self.scheduler.circuit(),
                &previous.circuit,
                &self.clock,
                now,
            ),
        };
        self.open_runtime.replace(target.clone(), snapshot)?;
        Ok(())
    }

    fn publish_status(&self, in_flight: bool) {
        self.status.publish(CoordinatorStatusSnapshot {
            maximum_lag_milliseconds: self.maximum_lag_milliseconds,
            origin_circuit_open: !matches!(self.scheduler.circuit(), OriginCircuit::Closed),
            overloaded: self.overloaded,
            in_flight,
        });
    }

    fn publish_activity(
        &self,
        phase: ServiceOperationPhaseV1,
        target: Option<TermCampusKey>,
        next_retry_at: Option<OffsetDateTime>,
    ) {
        self.status.publish_activity(ServiceOperationV1 {
            phase,
            target,
            started_at: (!matches!(phase, ServiceOperationPhaseV1::Idle))
                .then(|| self.clock.wall_now()),
            next_retry_at,
        });
    }

    fn publish_workflow_running(&self, id: &WorkflowOperationId, stage: ServiceOperationStageV2) {
        self.status
            .publish_workflow_operation(WorkflowOperationActivity {
                id: id.clone(),
                stage,
                started_at: self.clock.wall_now(),
            });
    }
}

fn workflow_operation_id(key: &OriginJobKey) -> WorkflowOperationId {
    WorkflowOperationId {
        target: key.target.clone(),
        kind: match key.kind {
            OriginJobKind::Catalog => TargetWorkflowKind::CompleteSnapshot,
            OriginJobKind::Open => TargetWorkflowKind::OpenRefresh,
        },
    }
}

struct ShortLockOpenPersistence<S> {
    storage: S,
    catalog_candidate: Option<CatalogCandidateContext>,
    committed_catalog: Option<PublishOutcome>,
    prepared_rebuild: Option<PreparedServingRebuildDemand>,
    open_publication_barrier: Option<PreparedOpenPublicationBarrier>,
    open_attempt_target: Option<TermCampusKey>,
}

#[derive(Clone)]
struct CatalogCandidateContext {
    candidate: CatalogCandidateOpenSnapshot,
    empty_decision: EmptySnapshotDecision,
}

impl<S> ShortLockOpenPersistence<S> {
    fn new(storage: S, prepared_rebuild: Option<PreparedServingRebuildDemand>) -> Self {
        Self {
            storage,
            catalog_candidate: None,
            committed_catalog: None,
            prepared_rebuild,
            open_publication_barrier: None,
            open_attempt_target: None,
        }
    }

    fn with_catalog_candidate(
        storage: S,
        candidate: CatalogCandidateOpenSnapshot,
        empty_decision: EmptySnapshotDecision,
        prepared_rebuild: Option<PreparedServingRebuildDemand>,
    ) -> Self {
        Self {
            storage,
            catalog_candidate: Some(CatalogCandidateContext {
                candidate,
                empty_decision,
            }),
            committed_catalog: None,
            prepared_rebuild,
            open_publication_barrier: None,
            open_attempt_target: None,
        }
    }

    fn take_committed_catalog(&mut self) -> Option<PublishOutcome> {
        self.committed_catalog.take()
    }
}

impl<S> ShortLockOpenPersistence<S>
where
    S: ProductStorageAccess,
{
    fn with_storage<T>(
        &self,
        operation: impl FnOnce(&mut OperationalStorage) -> StorageResult<T>,
    ) -> StorageResult<T> {
        let mut storage =
            self.storage
                .lock_operational()
                .map_err(|_| StorageError::InvalidCommand {
                    field: STORAGE_LOCK_FIELD,
                    reason: STORAGE_LOCK_REASON,
                })?;
        operation(&mut storage)
    }
}

impl<S> OpenPullPersistence for ShortLockOpenPersistence<S>
where
    S: ProductStorageAccess,
{
    fn serving_open_catalog_snapshot(
        &mut self,
        target: &TermCampusKey,
    ) -> StorageResult<Option<OpenCatalogSnapshot>> {
        if let Some(context) = &self.catalog_candidate {
            if context.candidate.catalog.target != *target {
                return Err(StorageError::InvalidCommand {
                    field: "candidate_catalog.target",
                    reason: "does not match the Open target",
                });
            }
            return Ok(Some(context.candidate.catalog.clone()));
        }
        self.with_storage(|storage| storage.serving_open_catalog_snapshot(target))
    }

    fn begin_open_pull_attempt(
        &mut self,
        command: &BeginOpenPullAttemptCommand,
    ) -> StorageResult<u64> {
        let candidate = self.catalog_candidate.clone();
        if candidate.is_none() {
            self.open_attempt_target = Some(command.target.clone());
        }
        let result = self.with_storage(|storage| match candidate {
            Some(context) => storage.begin_candidate_open_pull_attempt(&context.candidate, command),
            None => storage.begin_open_pull_attempt(command),
        });
        if result.is_err() {
            self.open_attempt_target = None;
        }
        result
    }

    fn finish_open_pull_success(
        &mut self,
        command: FinishOpenPullSuccessCommand,
    ) -> StorageResult<OpenCommitOutcome> {
        let candidate = self.catalog_candidate.clone();
        if candidate.is_none() {
            let target = self
                .open_attempt_target
                .clone()
                .ok_or(StorageError::InvalidCommand {
                    field: "prepared_open_attempt_target",
                    reason: "missing",
                })?;
            self.open_publication_barrier = self
                .prepared_rebuild
                .as_ref()
                .map(|demand| demand.begin_open_publication(target))
                .transpose()
                .map_err(|_| StorageError::InvalidCommand {
                    field: "prepared_open_publication_barrier",
                    reason: "unavailable",
                })?;
        }
        let fallback_failure = candidate.as_ref().map(|_| FinishOpenPullFailureCommand {
            attempt_id: command.attempt_id,
            completed_at: command.completed_at.clone(),
            http: command.http.clone(),
            error_code: "COMPLETE_SNAPSHOT_COMMIT_FAILED".to_owned(),
            diagnostic_token: None,
        });
        let publication_barrier = if candidate.is_some() {
            self.prepared_rebuild
                .as_ref()
                .map(PreparedServingRebuildDemand::begin_publication)
                .transpose()
                .map_err(|_| StorageError::InvalidCommand {
                    field: "prepared_serving_publication_barrier",
                    reason: "unavailable",
                })?
        } else {
            None
        };
        let result = self.with_storage(|storage| match candidate.clone() {
            Some(context) => storage.finish_candidate_open_pull_success(
                context.candidate.observation_id,
                context.empty_decision,
                command,
            ),
            None => storage.finish_open_pull_success(command).map(|open| {
                bcsp_operational_storage::CompleteSnapshotCommitOutcome {
                    catalog: None,
                    open,
                }
            }),
        });
        let result = match result {
            Ok(result) => result,
            Err(error) => {
                let mut fallback_committed = false;
                if let (Some(context), Some(failure)) = (candidate, fallback_failure) {
                    fallback_committed = self
                        .with_storage(|storage| {
                            storage.finish_candidate_open_pull_failure(
                                context.candidate.observation_id,
                                &failure,
                            )
                        })
                        .is_ok();
                }
                if fallback_committed {
                    if let Some(barrier) = publication_barrier {
                        barrier.commit_as_open(
                            self.catalog_candidate
                                .as_ref()
                                .expect("candidate checked above")
                                .candidate
                                .catalog
                                .target
                                .clone(),
                        );
                    }
                } else {
                    drop(publication_barrier);
                }
                return Err(error);
            }
        };
        if result.catalog.is_some() {
            if let Some(barrier) = publication_barrier {
                barrier.commit();
            }
        } else {
            if let Some(barrier) = publication_barrier {
                barrier.commit_as_open(
                    candidate
                        .as_ref()
                        .expect("candidate branch")
                        .candidate
                        .catalog
                        .target
                        .clone(),
                );
            }
        }
        if candidate.is_none()
            && let Some(barrier) = self.open_publication_barrier.take()
        {
            barrier.commit();
        }
        self.open_attempt_target = None;
        self.committed_catalog = result.catalog;
        Ok(result.open)
    }

    fn finish_open_pull_failure(
        &mut self,
        command: &FinishOpenPullFailureCommand,
    ) -> StorageResult<OpenCommitOutcome> {
        let candidate = self.catalog_candidate.clone();
        let candidate_open_barrier = if let Some(context) = candidate.as_ref() {
            self.prepared_rebuild
                .as_ref()
                .map(|demand| {
                    demand.begin_open_publication(context.candidate.catalog.target.clone())
                })
                .transpose()
                .map_err(|_| StorageError::InvalidCommand {
                    field: "prepared_open_publication_barrier",
                    reason: "unavailable",
                })?
        } else {
            None
        };
        if candidate.is_none() {
            let target = self
                .open_attempt_target
                .clone()
                .ok_or(StorageError::InvalidCommand {
                    field: "prepared_open_attempt_target",
                    reason: "missing",
                })?;
            self.open_publication_barrier = self
                .prepared_rebuild
                .as_ref()
                .map(|demand| demand.begin_open_publication(target))
                .transpose()
                .map_err(|_| StorageError::InvalidCommand {
                    field: "prepared_open_publication_barrier",
                    reason: "unavailable",
                })?;
        }
        let result = self.with_storage(|storage| match candidate.clone() {
            Some(context) => storage
                .finish_candidate_open_pull_failure(context.candidate.observation_id, command),
            None => storage.finish_open_pull_failure(command),
        });
        if result.is_ok()
            && candidate.is_none()
            && let Some(barrier) = self.open_publication_barrier.take()
        {
            barrier.commit();
        }
        if result.is_ok()
            && let Some(barrier) = candidate_open_barrier
        {
            barrier.commit();
        }
        if result.is_ok() {
            self.open_attempt_target = None;
        } else {
            self.open_publication_barrier.take();
        }
        result
    }
}

struct CoordinatorOpenPullClock<'a, C>(&'a C);

impl<C> OpenPullClock for CoordinatorOpenPullClock<'_, C>
where
    C: CoordinatorClock,
{
    fn now(&mut self) -> OffsetDateTime {
        self.0.wall_now()
    }
}

fn contract_lane(lane: OriginSchedulerLane) -> OpenSchedulerLane {
    match lane {
        OriginSchedulerLane::Catalog | OriginSchedulerLane::OpenGeneral => {
            OpenSchedulerLane::General
        }
        OriginSchedulerLane::OpenActiveWatch => OpenSchedulerLane::ActiveWatch,
        OriginSchedulerLane::OpenFirstLoad => OpenSchedulerLane::FirstLoad,
        OriginSchedulerLane::OpenManualRefresh => OpenSchedulerLane::ManualRefresh,
        OriginSchedulerLane::OpenCatalogRaceRecheck => OpenSchedulerLane::CatalogRaceRecheck,
    }
}

fn contract_circuit<C>(
    circuit: OriginCircuit,
    previous: &OpenCircuitStatusV1,
    clock: &C,
    now: MonotonicTime,
) -> OpenCircuitStatusV1
where
    C: CoordinatorClock,
{
    match circuit {
        OriginCircuit::Closed => OpenCircuitStatusV1 {
            state: OpenCircuitState::Closed,
            reason: None,
            opened_at: None,
            retry_at: None,
            diagnostic_recheck_required: false,
        },
        OriginCircuit::RetryAfter { until } => OpenCircuitStatusV1 {
            state: OpenCircuitState::RetryAfter,
            reason: None,
            opened_at: circuit_opened_at(previous, clock.wall_now()),
            retry_at: Some(wall_time_for_due(clock, now, until)),
            diagnostic_recheck_required: false,
        },
        OriginCircuit::FatalDiagnostic { recheck_after, .. } => OpenCircuitStatusV1 {
            state: OpenCircuitState::FatalDiagnostic,
            reason: None,
            opened_at: circuit_opened_at(previous, clock.wall_now()),
            retry_at: Some(wall_time_for_due(clock, now, recheck_after)),
            diagnostic_recheck_required: true,
        },
    }
}

fn circuit_opened_at(
    previous: &OpenCircuitStatusV1,
    now: OffsetDateTime,
) -> Option<OffsetDateTime> {
    if previous.state == OpenCircuitState::Closed {
        Some(now)
    } else {
        previous.opened_at.or(Some(now))
    }
}

fn wall_time_for_due<C>(clock: &C, now: MonotonicTime, due: MonotonicTime) -> OffsetDateTime
where
    C: CoordinatorClock,
{
    let remaining = due.saturating_duration_since(now);
    let milliseconds = i64::try_from(remaining.as_millis()).unwrap_or(i64::MAX);
    let wall_now = clock.wall_now();
    wall_now
        .checked_add(time::Duration::milliseconds(milliseconds))
        .unwrap_or(wall_now)
}

fn duration_millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

fn target_error_for_outcome(
    outcome: &CoordinatorDispatchOutcome,
    catalog_audit: Option<&CatalogFailureAudit>,
    catalog_trace_id: Option<TraceId>,
    open_failure: Option<&OpenAttemptRecord>,
) -> ServiceTargetErrorV2 {
    let (fallback_code, error_class) = match outcome {
        CoordinatorDispatchOutcome::CatalogFailed { failure, .. } => (
            failure.storage_code(),
            match failure {
                CatalogPullFailure::Transport => "TRANSIENT",
                CatalogPullFailure::RateLimited { .. } => "RATE_LIMITED",
                CatalogPullFailure::Schema => "CONTENT",
            },
        ),
        CoordinatorDispatchOutcome::OpenCompleted { terminal, .. } => (
            match terminal {
                OpenDispatchTerminal::Failed => "OPEN_REFRESH_FAILED",
                OpenDispatchTerminal::Unsafe => "OPEN_RESPONSE_UNSAFE",
                OpenDispatchTerminal::CatalogRace => "OPEN_CATALOG_RACE",
                OpenDispatchTerminal::Valid => "OPEN_REFRESH_UNEXPECTED_RETRY",
            },
            "OPEN",
        ),
        CoordinatorDispatchOutcome::CompleteSnapshotOpenFailed { error_code, .. } => {
            (*error_code, "OPEN")
        }
        CoordinatorDispatchOutcome::CatalogPublished { .. } => {
            ("REFRESH_UNEXPECTED_RETRY", "INTERNAL")
        }
    };
    let code = open_failure
        .and_then(|attempt| attempt.error_code.as_deref())
        .unwrap_or(fallback_code);
    ServiceTargetErrorV2 {
        code: code.to_owned(),
        http_status: open_failure
            .and_then(|attempt| attempt.http.http_status)
            .or_else(|| catalog_audit.and_then(|audit| audit.http_status)),
        content_type: open_failure
            .and_then(|attempt| attempt.http.content_type.clone())
            .or_else(|| catalog_audit.and_then(|audit| audit.content_type.clone())),
        content_encoding: catalog_audit.and_then(|audit| audit.content_encoding.clone()),
        decoded_bytes: open_failure
            .and_then(|attempt| attempt.http.decoded_bytes)
            .or_else(|| catalog_audit.and_then(|audit| audit.decoded_bytes)),
        error_class: Some(error_class.to_owned()),
        error_chain: catalog_audit.and_then(|audit| audit.error_chain.clone()),
        trace_id: open_failure
            .map(|attempt| attempt.attempt_id)
            .or(match outcome {
                CoordinatorDispatchOutcome::CompleteSnapshotOpenFailed { trace_id, .. } => {
                    Some(*trace_id)
                }
                _ => catalog_trace_id,
            }),
    }
}

fn retry_stage_for_outcome(
    outcome: &CoordinatorDispatchOutcome,
    dispatched_kind: OriginJobKind,
) -> ServiceOperationStageV2 {
    if matches!(
        outcome,
        CoordinatorDispatchOutcome::CompleteSnapshotOpenFailed { .. }
    ) {
        ServiceOperationStageV2::OpenFetch
    } else {
        match dispatched_kind {
            OriginJobKind::Catalog => ServiceOperationStageV2::CatalogFetch,
            OriginJobKind::Open => ServiceOperationStageV2::OpenFetch,
        }
    }
}

fn set_catalog_audit_error(
    audit: &mut Option<CatalogFailureAudit>,
    decoded_bytes: Option<usize>,
    error_chain: &'static str,
) {
    let audit = audit.get_or_insert_with(CatalogFailureAudit::default);
    audit.decoded_bytes = decoded_bytes.and_then(|value| u64::try_from(value).ok());
    audit.error_chain = Some(error_chain.to_owned());
}

fn catalog_retry_directive(
    target: &TermCampusKey,
    current_failure_streak: u32,
    failure: CatalogPullFailure,
) -> RetryDirective {
    if let CatalogPullFailure::RateLimited { retry_after } = failure {
        return RetryDirective {
            delay: retry_after.unwrap_or(Duration::from_secs(60)),
            mode: RetryMode::OriginCircuit,
            next_failure_streak: current_failure_streak.saturating_add(1),
            clears_fatal_diagnostic: true,
        };
    }
    target_retry_directive(
        target,
        current_failure_streak,
        if failure == CatalogPullFailure::Schema {
            TargetRetryClass::Content
        } else {
            TargetRetryClass::Transient
        },
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CatalogPreparationFailure {
    Normalization,
    Mapping,
}

enum PreparedCompleteSnapshot {
    Candidate {
        candidate: CatalogCandidateOpenSnapshot,
        empty_decision: EmptySnapshotDecision,
    },
    Retained(PublishOutcome),
}

fn format_timestamp(value: OffsetDateTime) -> Result<String, CoordinatorError> {
    value
        .format(&Rfc3339)
        .map_err(|_| CoordinatorError::TimestampFormat)
}

#[derive(Debug, Error)]
pub enum CoordinatorError {
    #[error("refresh supervisor requires at least one worker")]
    NoRefreshWorkers,
    #[error("registered Open request does not match the scheduled target")]
    OpenRequestTargetMismatch,
    #[error("refresh policy is unavailable")]
    RefreshPolicy(#[source] RefreshPolicyReadError),
    #[error("refresh scheduler failed")]
    Scheduler(#[from] SchedulerError),
    #[error("operational storage lock is unavailable")]
    StorageLock,
    #[error("operational storage failed")]
    Storage(#[from] StorageError),
    #[error("Catalog source byte count overflowed")]
    SourceBytesOverflow,
    #[error("Catalog blocking work did not complete")]
    CatalogBlockingTask,
    #[error("prepared serving publication barrier is unavailable")]
    PreparedServing(#[from] PreparedServingError),
    #[error("refresh timestamp could not be represented as RFC 3339")]
    TimestampFormat,
    #[error("Open target is not registered")]
    TargetNotRegistered,
    #[error("shared Open execution failed")]
    Open(#[from] SharedOpenServiceError),
    #[error("Open runtime registry is unavailable")]
    OpenRuntime(#[from] OpenRuntimeSnapshotRegistryError),
    #[error("watch observation fanout failed")]
    Watch(#[from] WatchManagerError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn candidate_open_waits_for_serving_open_and_blocks_late_duplicates() {
        let control = Arc::new(TargetWorkflowControl::default());
        control.mark_catalog_fetch();
        let serving_open = control.open_gate.lock().await;
        assert!(control.allows_parallel_serving_open());

        control.mark_candidate_open();
        assert!(!control.allows_parallel_serving_open());
        assert!(
            tokio::time::timeout(Duration::from_millis(10), control.open_gate.lock())
                .await
                .is_err(),
            "candidate Open must merge behind the already-running serving Open"
        );

        drop(serving_open);
        let candidate = tokio::time::timeout(Duration::from_millis(50), control.open_gate.lock())
            .await
            .expect("candidate Open acquires the gate after serving Open completes");
        drop(candidate);
        assert!(!control.allows_parallel_serving_open());
    }

    #[test]
    fn first_load_freshness_budget_scales_with_the_discovered_fleet() {
        assert_eq!(
            first_load_freshness_interval_seconds(1),
            u32::try_from(crate::DISCOVERY_REFRESH_INTERVAL.as_secs()).expect("discovery interval")
        );
        assert_eq!(
            Duration::from_secs(u64::from(first_load_freshness_interval_seconds(200))),
            refresh_generation_interval(200)
        );
    }

    #[test]
    fn catalog_retry_is_target_local_nb_weighted_and_never_fatal() {
        let nb = TermCampusKey::try_new("92026", "NB").expect("NB");
        let camden = TermCampusKey::try_new("92026", "CM").expect("CM");
        for (streak, seconds) in [5_u64, 10, 20, 30, 60].iter().enumerate() {
            let retry = catalog_retry_directive(
                &nb,
                u32::try_from(streak).expect("streak"),
                CatalogPullFailure::Transport,
            );
            assert_eq!(retry.mode, RetryMode::Automatic);
            assert!(retry.delay >= Duration::from_secs(*seconds));
            assert!(retry.delay <= Duration::from_millis(*seconds * 1_100));
        }
        let other = catalog_retry_directive(&camden, 0, CatalogPullFailure::Transport);
        assert!(other.delay >= Duration::from_secs(15));
        let content = catalog_retry_directive(&nb, 0, CatalogPullFailure::Schema);
        assert_eq!(content.mode, RetryMode::Automatic);
        assert!(content.delay >= Duration::from_secs(60));
        let rate_limited = catalog_retry_directive(
            &nb,
            0,
            CatalogPullFailure::RateLimited { retry_after: None },
        );
        assert_eq!(rate_limited.mode, RetryMode::OriginCircuit);
        assert_eq!(rate_limited.delay, Duration::from_secs(60));
    }
}
