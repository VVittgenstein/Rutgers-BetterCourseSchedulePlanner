use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use bcsp_catalog::{normalize_target, to_catalog_refresh_command};
use bcsp_contracts::{
    OpenCircuitState, OpenCircuitStatusV1, OpenSchedulerLane, SystemTraceIdSource, TermCampusKey,
    TraceId, TraceIdSource,
};
use bcsp_open::{
    CompletionSchedule, MonotonicTime, OpenCounterAudience, OpenFailureKind, OpenPullClock,
    OpenPullCommand, OpenPullPersistence, OpenPullTerminal, OriginCircuit, OriginDispatch,
    OriginEdfScheduler, OriginJobKey, OriginJobKind, OriginSchedulerLane, SchedulerError,
    SharedOpenService, SharedOpenServiceError, retry_directive,
};
use bcsp_operational_storage::{
    BeginOpenPullAttemptCommand, BeginRefreshAttemptCommand, EmptySnapshotDecision,
    FinishOpenPullFailureCommand, FinishOpenPullSuccessCommand, FinishRefreshFailureCommand,
    InitialEmptyProof, OpenCatalogSnapshot, OpenCommitOutcome, OperationalStorage, PublishOutcome,
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

use crate::{
    OpenRuntimeSnapshot, OpenRuntimeSnapshotRegistry, OpenRuntimeSnapshotRegistryError,
    ProductStorageAccess, RefreshPolicy, RefreshPolicyProvider, RefreshPolicyReadError,
    SharedWatchSocket,
};

const STORAGE_LOCK_FIELD: &str = "operational_storage_lock";
const STORAGE_LOCK_REASON: &str = "unavailable";

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

    const fn retry_kind(self) -> OpenFailureKind {
        match self {
            Self::Transport => OpenFailureKind::Transient,
            Self::RateLimited { retry_after } => OpenFailureKind::RateLimited { retry_after },
            Self::Schema => OpenFailureKind::FatalProtocol,
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
}

pub trait CoordinatorClock: Send + Sync {
    fn monotonic_now(&self) -> MonotonicTime;

    fn wall_now(&self) -> OffsetDateTime;
}

#[derive(Debug)]
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

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CoordinatorStatusSnapshot {
    pub maximum_lag_milliseconds: u64,
    pub origin_circuit_open: bool,
    pub overloaded: bool,
    pub in_flight: bool,
}

/// Readiness bridge owned by an entrypoint. The public runtime can project these four aggregate
/// facts into its readiness surface without introducing a dependency from shared application code
/// back to the public crate.
pub trait CoordinatorStatusSink: Send + Sync + 'static {
    fn publish(&self, snapshot: CoordinatorStatusSnapshot);

    fn mark_stopped(&self);
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
    scheduler: OriginEdfScheduler,
    targets: BTreeMap<TermCampusKey, OpenSectionsRequest>,
    maximum_lag_milliseconds: u64,
    overloaded: bool,
}

impl<S, U, P> SharedRefreshCoordinator<S, U, P, SystemCoordinatorClock, SystemTraceIdSource>
where
    S: ProductStorageAccess + Clone,
    U: RefreshUpstream,
    P: RefreshPolicyProvider,
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
            scheduler: OriginEdfScheduler::new(),
            targets: BTreeMap::new(),
            maximum_lag_milliseconds: 0,
            overloaded: false,
        }
    }

    pub fn register_target(
        &mut self,
        registration: ScheduledRefreshTarget,
    ) -> Result<(), CoordinatorError> {
        if self.targets.contains_key(registration.target()) {
            return Ok(());
        }
        let policy = self.refresh_policy()?;
        let now = self.clock.monotonic_now();
        let active_watch_count = self.watch.active_watch_count(&registration.target);
        self.scheduler.register_catalog(
            registration.target.clone(),
            policy.catalog_interval(),
            now,
        )?;
        self.scheduler.register_open_initial(
            registration.target.clone(),
            policy.open_general_interval(),
            active_watch_count,
            now,
            now.saturating_add(Duration::from_millis(1)),
        )?;
        self.targets
            .insert(registration.target.clone(), registration.open_request);
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
            .activate_open(target, policy.open_general_interval(), now)?;
        self.publish_open_snapshot(target, now, None)?;
        self.publish_status(false);
        Ok(())
    }

    pub fn registered_targets(&self) -> Vec<TermCampusKey> {
        self.targets.keys().cloned().collect()
    }

    pub fn stop(&self) {
        self.status.mark_stopped();
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
            return Ok(None);
        };

        self.maximum_lag_milliseconds = duration_millis(dispatch.scheduler_lag);
        self.overloaded = dispatch.scheduler_lag > dispatch.requested_interval;
        self.publish_all_open_snapshots(now, Some(&dispatch))?;
        self.publish_status(true);

        let execution = match dispatch.key.kind {
            OriginJobKind::Catalog => self.execute_catalog(&dispatch, policy).await,
            OriginJobKind::Open => self.execute_open(&dispatch, policy).await,
        };

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
            let catalog_due = self
                .scheduler
                .next_due(&catalog_key)
                .ok_or(SchedulerError::UnknownJob)?;
            self.scheduler.register_catalog(
                target.clone(),
                policy.catalog_interval(),
                catalog_due,
            )?;
            self.scheduler.set_open_watch_count(
                target,
                policy.open_general_interval(),
                self.watch.active_watch_count(target),
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
        let completed_at = format_timestamp(self.clock.wall_now())?;
        let response = match response {
            Ok(response) if response.target == dispatch.key.target => response,
            Ok(response) => {
                self.finish_catalog_failure(
                    attempt_id,
                    completed_at,
                    RefreshFailureStage::Schema,
                    Some(response.provenance.body_sha256),
                    Some(response.provenance.decoded_bytes),
                    "CATALOG_RESPONSE_TARGET_MISMATCH",
                )?;
                return Ok(self.catalog_failure_result(
                    dispatch,
                    policy,
                    CatalogPullFailure::Schema,
                ));
            }
            Err(failure) => {
                self.finish_catalog_failure(
                    attempt_id,
                    completed_at,
                    failure.storage_stage(),
                    None,
                    None,
                    failure.storage_code(),
                )?;
                return Ok(self.catalog_failure_result(dispatch, policy, failure));
            }
        };

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
                self.finish_catalog_failure(
                    attempt_id,
                    completed_at,
                    RefreshFailureStage::Normalization,
                    Some(source_content_sha256),
                    Some(source_bytes),
                    "CATALOG_NORMALIZATION_FAILED",
                )?;
                return Ok(self.catalog_failure_result(
                    dispatch,
                    policy,
                    CatalogPullFailure::Schema,
                ));
            }
            Err(CatalogPreparationFailure::Mapping) => {
                self.finish_catalog_failure(
                    attempt_id,
                    completed_at,
                    RefreshFailureStage::Normalization,
                    Some(source_content_sha256),
                    Some(source_bytes),
                    "CATALOG_MAPPING_FAILED",
                )?;
                return Ok(self.catalog_failure_result(
                    dispatch,
                    policy,
                    CatalogPullFailure::Schema,
                ));
            }
        };
        let storage_access = self.storage.clone();
        let outcome = tokio::task::spawn_blocking(move || -> Result<_, CoordinatorError> {
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
            storage
                .apply_catalog_refresh(command, empty_decision)
                .map_err(CoordinatorError::Storage)
        })
        .await
        .map_err(|_| CoordinatorError::CatalogBlockingTask)??;
        let immediate_open_lane = match &outcome {
            PublishOutcome::AppliedChanged {
                content_version: 1, ..
            }
            | PublishOutcome::InitialValidEmpty { content_version: 1 } => {
                Some(OriginSchedulerLane::OpenFirstLoad)
            }
            PublishOutcome::AppliedChanged { .. } => {
                Some(OriginSchedulerLane::OpenCatalogRaceRecheck)
            }
            PublishOutcome::AppliedUnchanged { .. }
            | PublishOutcome::InitialValidEmpty { .. }
            | PublishOutcome::SuspectEmptyRetained { .. } => None,
        };
        Ok((
            CoordinatorDispatchOutcome::CatalogPublished {
                target: dispatch.key.target.clone(),
                outcome,
            },
            CompletionSchedule::Success,
            immediate_open_lane,
        ))
    }

    fn catalog_failure_result(
        &self,
        dispatch: &OriginDispatch,
        policy: RefreshPolicy,
        failure: CatalogPullFailure,
    ) -> (
        CoordinatorDispatchOutcome,
        CompletionSchedule,
        Option<OriginSchedulerLane>,
    ) {
        let retry = retry_directive(
            &dispatch.key.target,
            policy.catalog_interval(),
            dispatch.failure_streak,
            failure.retry_kind(),
        );
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
        let command = OpenPullCommand {
            attempt_id: self.ids.next_trace_id(),
            run_id: self.run_id,
            source_request,
            general_interval: policy.open_general_interval(),
            active_watch_count,
            lane: dispatch.lane,
            scheduler_lag: dispatch.scheduler_lag,
            current_failure_streak: dispatch.failure_streak,
            counter_audience: self.counter_audience,
        };
        let mut persistence = ShortLockOpenPersistence::new(self.storage.clone());
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

    fn finish_catalog_failure(
        &self,
        observation_id: TraceId,
        completed_at: String,
        stage: RefreshFailureStage,
        source_content_sha256: Option<String>,
        source_bytes: Option<usize>,
        error_code: &'static str,
    ) -> Result<(), CoordinatorError> {
        let source_bytes = source_bytes
            .map(u64::try_from)
            .transpose()
            .map_err(|_| CoordinatorError::SourceBytesOverflow)?;
        self.with_storage(|storage| {
            storage.finish_refresh_failure(&FinishRefreshFailureCommand {
                observation_id,
                completed_at,
                stage,
                source_content_sha256,
                source_bytes,
                error_code: error_code.to_owned(),
                diagnostic_token: None,
            })
        })
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
}

struct ShortLockOpenPersistence<S> {
    storage: S,
}

impl<S> ShortLockOpenPersistence<S> {
    const fn new(storage: S) -> Self {
        Self { storage }
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
        self.with_storage(|storage| storage.serving_open_catalog_snapshot(target))
    }

    fn begin_open_pull_attempt(
        &mut self,
        command: &BeginOpenPullAttemptCommand,
    ) -> StorageResult<u64> {
        self.with_storage(|storage| storage.begin_open_pull_attempt(command))
    }

    fn finish_open_pull_success(
        &mut self,
        command: FinishOpenPullSuccessCommand,
    ) -> StorageResult<OpenCommitOutcome> {
        self.with_storage(|storage| storage.finish_open_pull_success(command))
    }

    fn finish_open_pull_failure(
        &mut self,
        command: &FinishOpenPullFailureCommand,
    ) -> StorageResult<OpenCommitOutcome> {
        self.with_storage(|storage| storage.finish_open_pull_failure(command))
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CatalogPreparationFailure {
    Normalization,
    Mapping,
}

fn format_timestamp(value: OffsetDateTime) -> Result<String, CoordinatorError> {
    value
        .format(&Rfc3339)
        .map_err(|_| CoordinatorError::TimestampFormat)
}

#[derive(Debug, Error)]
pub enum CoordinatorError {
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
