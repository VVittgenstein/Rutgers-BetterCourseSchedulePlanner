use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};
use std::ops::Deref;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};

use bcsp_catalog::{ProjectionError, to_catalog_discovery_response_v1, to_normalized_catalog_v1};
use bcsp_contracts::{
    CatalogContentVersion, CatalogDiscoveryResponseV1, FilterFieldId, FilterOptionV2,
    FilterOptionsFieldV2, FilterSearchTextV1, InvalidDynamicFilterValueV3, LiveOpenEvidenceV1,
    LiveOpenStateV1, MatchReasonCode, NormalizedCatalogV1, NormalizedFilterValuesV1,
    ServiceRuntimeV1, TermCampusKey,
};
use bcsp_domain::{RutgersTermWindow, RutgersTermWindowError, RutgersTermWindowScope};
#[cfg(test)]
use bcsp_operational_storage::PublishedCourseFtsDocument;
use bcsp_operational_storage::{
    CourseFtsCorpusSignature, CourseTextSearchTokens, CourseVariantSearchHit, OperationalStorage,
    StorageError,
};
use bcsp_query::{
    CorpusError, OpenEvidence, PreparedCatalogCorpus,
    PreparedOpenOverlay as PreparedQueryOpenOverlay, QueryError, TextHit, TextHitPlan,
    TextHitPlanError, TextTargetVersion,
};
use rusqlite::{Connection, OptionalExtension, params};
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::{
    ApplicationClock, OpenRuntimeSnapshotRegistry, OpenRuntimeSnapshotRegistryError, RefreshPolicy,
    RefreshPolicyProvider, SharedRuntimeContext, SharedRuntimeError, SystemApplicationClock,
    is_product_campus, product_target_keys,
};

#[derive(Clone)]
pub struct PreparedServingRebuildDemand {
    sender: mpsc::UnboundedSender<PreparedRebuildRequest>,
    registry: Arc<PreparedServingRegistry>,
    lifecycle: PublicationLifecycle,
}

#[derive(Clone)]
struct PublicationLifecycle {
    state: Arc<(Mutex<PublicationLifecycleState>, Condvar)>,
}

struct PublicationLifecycleState {
    accepting: bool,
    active_admissions: usize,
}

const PREPARED_SHUTDOWN_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// How long a request path waits for a committed publication barrier to clear
/// before it gives up with [`PreparedServingError::SnapshotRebuilding`].
///
/// The measured Committed window in the released local build is 3.5-4.5 s
/// (the worker's own Open projection plus a few queued route projections),
/// so five seconds covers today's worst case with margin while staying well
/// under a user's "hung" perception. Once the rebuild window shrinks this is
/// only the safety cap; the public runtime chooses a shorter wait because its
/// request waiters and the rebuild share one blocking pool.
pub const PREPARED_REQUEST_ADMISSION_WAIT: Duration = Duration::from_secs(5);

#[derive(Clone, Default)]
struct PreparedRebuildCancellation {
    requested: Arc<AtomicBool>,
}

impl PreparedRebuildCancellation {
    fn request(&self) {
        self.requested.store(true, Ordering::Release);
    }

    fn check(&self) -> Result<(), PreparedServingError> {
        if self.requested.load(Ordering::Acquire) {
            Err(PreparedServingError::RebuildCancelled)
        } else {
            Ok(())
        }
    }

    fn requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }
}

impl Default for PublicationLifecycle {
    fn default() -> Self {
        Self {
            state: Arc::new((
                Mutex::new(PublicationLifecycleState {
                    accepting: true,
                    active_admissions: 0,
                }),
                Condvar::new(),
            )),
        }
    }
}

impl PublicationLifecycle {
    fn begin(&self) -> Result<PublicationLifecycleAdmission, PreparedServingError> {
        let (state, _) = &*self.state;
        let mut state = state
            .lock()
            .map_err(|_| PreparedServingError::RegistryUnavailable)?;
        if !state.accepting {
            return Err(PreparedServingError::PublicationWorkerUnavailable);
        }
        state.active_admissions = state.active_admissions.saturating_add(1);
        Ok(PublicationLifecycleAdmission {
            lifecycle: self.clone(),
            active: true,
        })
    }

    fn stop_and_wait(&self, timeout: std::time::Duration) -> bool {
        let (state, changed) = &*self.state;
        let Ok(mut state) = state.lock() else {
            return false;
        };
        state.accepting = false;
        let deadline = Instant::now() + timeout;
        while state.active_admissions > 0 {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return false;
            }
            let Ok((next, timed)) = changed.wait_timeout(state, remaining) else {
                return false;
            };
            state = next;
            if timed.timed_out() && state.active_admissions > 0 {
                return false;
            }
        }
        true
    }

    fn stop_accepting(&self) {
        let (state, changed) = &*self.state;
        if let Ok(mut state) = state.lock() {
            state.accepting = false;
            changed.notify_all();
        }
    }
}

struct PublicationLifecycleAdmission {
    lifecycle: PublicationLifecycle,
    active: bool,
}

impl Drop for PublicationLifecycleAdmission {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let (state, changed) = &*self.lifecycle.state;
        if let Ok(mut state) = state.lock() {
            state.active_admissions = state.active_admissions.saturating_sub(1);
            changed.notify_all();
        }
        self.active = false;
    }
}

/// A short publication barrier installed immediately before the authoritative
/// SQLite transaction. Dropping an uncommitted barrier restores admission;
/// committing it queues the rebuild while retaining the dirty gate.
pub struct PreparedServingPublicationBarrier {
    demand: PreparedServingRebuildDemand,
    epoch: u64,
    committed: bool,
    _lifecycle: PublicationLifecycleAdmission,
}

pub struct PreparedOpenPublicationBarrier {
    demand: PreparedServingRebuildDemand,
    target: TermCampusKey,
    admission: Option<OpenDirtyAdmission>,
    _lifecycle: PublicationLifecycleAdmission,
}

impl PreparedOpenPublicationBarrier {
    pub fn commit(mut self) {
        let Some(admission) = self.admission.take() else {
            return;
        };
        if let Err(error) = self
            .demand
            .registry
            .commit_open_dirty(&self.target, admission.epoch)
        {
            tracing::error!(
                target: "bcsp_prepared_serving",
                ?error,
                open_target = ?self.target,
                epoch = admission.epoch,
                "failed to commit prepared Open publication ticket; admission remains conservative",
            );
            return;
        }
        if self
            .demand
            .sender
            .send(PreparedRebuildRequest::Open(
                self.target.clone(),
                admission.epoch,
            ))
            .is_err()
        {
            // The Open attempt is durable, so never restore the previous
            // evidence. Admit the conservative overlay and report worker
            // failure instead of leaving an orphaned dirty epoch.
            tracing::error!(
                target: "bcsp_prepared_serving",
                open_target = ?self.target,
                epoch = admission.epoch,
                "prepared Open rebuild channel closed after commit; conservative evidence remains admitted",
            );
        }
    }
}

impl Drop for PreparedOpenPublicationBarrier {
    fn drop(&mut self) {
        if let Some(admission) = self.admission.take() {
            self.demand
                .registry
                .rollback_open_dirty(&self.target, admission);
        }
    }
}

impl PreparedServingPublicationBarrier {
    pub fn commit(mut self) {
        self.committed = true;
        if let Err(error) = self.demand.registry.commit_full_dirty(self.epoch) {
            tracing::error!(
                target: "bcsp_prepared_serving",
                ?error,
                epoch = self.epoch,
                "failed to commit prepared FULL publication ticket; admission remains blocked",
            );
            return;
        }
        if self
            .demand
            .sender
            .send(PreparedRebuildRequest::Full(self.epoch))
            .is_err()
        {
            // The DB commit already happened. Retaining the dirty barrier is
            // the only safe admission state until worker health is restored.
            tracing::error!(
                target: "bcsp_prepared_serving",
                epoch = self.epoch,
                "prepared rebuild channel closed after an authoritative commit; admission remains blocked",
            );
        }
    }

    pub fn commit_as_open(mut self, target: TermCampusKey) {
        self.committed = true;
        match self
            .demand
            .registry
            .convert_full_dirty_to_open(self.epoch, &target)
        {
            Ok(open_epoch) => {
                if self
                    .demand
                    .sender
                    .send(PreparedRebuildRequest::Open(target.clone(), open_epoch))
                    .is_err()
                {
                    tracing::error!(
                        target: "bcsp_prepared_serving",
                        open_target = ?target,
                        open_epoch,
                        "prepared channel closed after non-applied candidate terminal; conservative evidence remains admitted",
                    );
                }
            }
            Err(error) => {
                // The authoritative terminal already committed. Keep the FULL
                // gate and request a conservative full rebuild rather than
                // ever reopening stale evidence.
                let _ = self
                    .demand
                    .sender
                    .send(PreparedRebuildRequest::Full(self.epoch));
                tracing::error!(
                    target: "bcsp_prepared_serving",
                    ?error,
                    "failed to narrow FULL barrier to Open; retaining fail-closed FULL rebuild",
                );
            }
        }
    }
}

impl Drop for PreparedServingPublicationBarrier {
    fn drop(&mut self) {
        if !self.committed {
            self.demand.registry.rollback_full_dirty(self.epoch);
        }
    }
}

impl PreparedServingRebuildDemand {
    pub fn begin_publication(
        &self,
    ) -> Result<PreparedServingPublicationBarrier, PreparedServingError> {
        let lifecycle = self.lifecycle.begin()?;
        Ok(PreparedServingPublicationBarrier {
            demand: self.clone(),
            epoch: self.registry.mark_full_dirty()?,
            committed: false,
            _lifecycle: lifecycle,
        })
    }

    pub fn begin_open_publication(
        &self,
        target: TermCampusKey,
    ) -> Result<PreparedOpenPublicationBarrier, PreparedServingError> {
        let lifecycle = self.lifecycle.begin()?;
        let admission = self.registry.mark_open_dirty(&target)?;
        Ok(PreparedOpenPublicationBarrier {
            demand: self.clone(),
            target,
            admission: Some(admission),
            _lifecycle: lifecycle,
        })
    }
}

#[derive(Debug)]
enum PreparedRebuildRequest {
    Full(u64),
    Open(TermCampusKey, u64),
    Shutdown,
}

/// One dirty/coalescing worker per process. Full Catalog rebuilds dominate
/// pending Open overlays; events arriving during build remain queued and force
/// another pass before the worker becomes idle.
pub struct PreparedServingRebuildRuntime {
    demand: PreparedServingRebuildDemand,
    cancellation: PreparedRebuildCancellation,
    task: Option<JoinHandle<()>>,
}

/// Marks the worker gone when its task ends for any reason (ordered shutdown,
/// abort, or panic): new publications are refused, and request waiters parked
/// on a committed ticket are released so they fail fast instead of sitting out
/// their full admission wait behind a worker that will never clear it.
struct PublicationWorkerGuard {
    lifecycle: PublicationLifecycle,
    registry: Arc<PreparedServingRegistry>,
}

impl Drop for PublicationWorkerGuard {
    fn drop(&mut self) {
        self.lifecycle.stop_accepting();
        self.registry.close();
    }
}

impl PreparedServingRebuildRuntime {
    pub fn spawn<S, P>(
        storage: S,
        policy: P,
        counter_audience: bcsp_open::OpenCounterAudience,
        open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
        registry: Arc<PreparedServingRegistry>,
    ) -> Self
    where
        S: crate::ProductStorageAccess + Clone + Send + 'static,
        P: RefreshPolicyProvider + Clone + Send + 'static,
    {
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let lifecycle = PublicationLifecycle::default();
        let demand = PreparedServingRebuildDemand {
            sender,
            registry: Arc::clone(&registry),
            lifecycle: lifecycle.clone(),
        };
        let cancellation = PreparedRebuildCancellation::default();
        let worker_cancellation = cancellation.clone();
        // A registry outlives its worker in tests and across restarts; a new
        // worker re-arms request waiting that an earlier worker's exit closed.
        registry.reopen();
        let guard_registry = Arc::clone(&registry);
        let task = tokio::spawn(async move {
            let _worker_guard = PublicationWorkerGuard {
                lifecycle,
                registry: guard_registry,
            };
            let service_runtime = match counter_audience {
                bcsp_open::OpenCounterAudience::Local { .. } => ServiceRuntimeV1::Local,
                bcsp_open::OpenCounterAudience::Public => ServiceRuntimeV1::Public,
            };
            let mut full_epochs = BTreeSet::new();
            let mut open_targets: BTreeMap<TermCampusKey, BTreeSet<u64>> = BTreeMap::new();
            let mut shutdown_requested = false;
            while let Some(request) = receiver.recv().await {
                if merge_rebuild_request(request, &mut full_epochs, &mut open_targets) {
                    shutdown_requested = true;
                }
                while let Ok(request) = receiver.try_recv() {
                    if merge_rebuild_request(request, &mut full_epochs, &mut open_targets) {
                        shutdown_requested = true;
                    }
                }

                loop {
                    let executed_full_epochs = std::mem::take(&mut full_epochs);
                    let executed_open_epochs = if !executed_full_epochs.is_empty() {
                        BTreeMap::new()
                    } else {
                        std::mem::take(&mut open_targets)
                    };
                    let targets = executed_open_epochs.keys().cloned().collect::<Vec<_>>();
                    let execute_full = !executed_full_epochs.is_empty();
                    if !execute_full && targets.is_empty() {
                        break;
                    }

                    let build_storage = storage.clone();
                    let policy = policy.clone();
                    let open_runtime = Arc::clone(&open_runtime);
                    let build_registry = Arc::clone(&registry);
                    let build_cancellation = worker_cancellation.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        let runtime = SharedRuntimeContext::new(
                            counter_audience,
                            SystemApplicationClock,
                            policy,
                        );
                        if execute_full {
                            rebuild_prepared_serving_from_access_with_cancellation(
                                &build_storage,
                                &runtime,
                                &open_runtime,
                                service_runtime,
                                &build_registry,
                                &build_cancellation,
                            )
                        } else {
                            rebuild_prepared_open_from_access_with_cancellation(
                                &build_storage,
                                &runtime,
                                &open_runtime,
                                &targets,
                                &build_registry,
                                &build_cancellation,
                            )
                        }
                    })
                    .await;

                    match result {
                        Ok(Ok(_)) => {
                            if execute_full {
                                registry.clear_full_tickets(&executed_full_epochs);
                            } else {
                                for (target, epochs) in &executed_open_epochs {
                                    registry.clear_open_tickets(target, epochs);
                                }
                            }
                        }
                        Ok(Err(
                            PreparedServingError::PublicationChanged
                            | PreparedServingError::SupersededBuild,
                        )) => {
                            if shutdown_requested {
                                // Shutdown is fail-closed: leave the exact
                                // registry tickets in place while the serving
                                // surface is being removed, but do not retry
                                // indefinitely.
                            } else if execute_full {
                                full_epochs.extend(executed_full_epochs.iter().copied());
                            } else {
                                requeue_open_tickets(&mut open_targets, &executed_open_epochs);
                            }
                            tokio::task::yield_now().await;
                        }
                        Ok(Err(PreparedServingError::RebuildCancelled)) => {
                            // Shutdown requested cancellation before this build completed. The
                            // exact dirty tickets remain fail-closed while the surface drains.
                        }
                        Ok(Err(
                            PreparedServingError::TargetNotPrepared(_)
                            | PreparedServingError::SnapshotUnavailable
                            | PreparedServingError::PreparedDictionaryUnavailable,
                        )) if !execute_full => {
                            if shutdown_requested {
                                // See the fail-closed shutdown note above.
                            } else {
                                match registry.promote_open_tickets_to_full(&executed_open_epochs) {
                                    Ok(epoch) => {
                                        full_epochs.insert(epoch);
                                    }
                                    Err(error) => {
                                        requeue_open_tickets(
                                            &mut open_targets,
                                            &executed_open_epochs,
                                        );
                                        tracing::error!(
                                            target: "bcsp_prepared_serving",
                                            ?error,
                                            "failed to atomically promote prepared Open tickets to a full rebuild",
                                        );
                                    }
                                }
                            }
                        }
                        Ok(Err(error)) => {
                            let source_unchanged = execute_full
                                && authoritative_foundation_matches_access(&storage, &registry)
                                    .unwrap_or(false);
                            if shutdown_requested {
                                // Do not turn shutdown into an unbounded retry
                                // loop. Dirty tickets remain fail-closed until
                                // the surface is gone.
                            } else if execute_full && source_unchanged {
                                registry.clear_full_tickets(&executed_full_epochs);
                            } else if execute_full {
                                // The authoritative Catalog generation changed.
                                // Retain the publication barrier and retry with
                                // bounded backoff; never resurrect the old Arc.
                                full_epochs.extend(executed_full_epochs.iter().copied());
                                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            } else {
                                requeue_open_tickets(&mut open_targets, &executed_open_epochs);
                                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            }
                            tracing::warn!(
                                target: "bcsp_prepared_serving",
                                ?error,
                                source_unchanged,
                                "prepared serving rebuild failed; previous generation is admitted only when authoritative Catalog generation is unchanged",
                            );
                        }
                        Err(error) => {
                            let source_unchanged = execute_full
                                && authoritative_foundation_matches_access(&storage, &registry)
                                    .unwrap_or(false);
                            if shutdown_requested {
                                // Same bounded fail-closed shutdown policy as
                                // storage/build errors above.
                            } else if execute_full && source_unchanged {
                                registry.clear_full_tickets(&executed_full_epochs);
                            } else if execute_full {
                                full_epochs.extend(executed_full_epochs.iter().copied());
                                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            } else {
                                requeue_open_tickets(&mut open_targets, &executed_open_epochs);
                                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                            }
                            tracing::error!(
                                target: "bcsp_prepared_serving",
                                ?error,
                                source_unchanged,
                                "prepared serving rebuild worker failed",
                            );
                        }
                    }

                    while let Ok(request) = receiver.try_recv() {
                        if merge_rebuild_request(request, &mut full_epochs, &mut open_targets) {
                            shutdown_requested = true;
                        }
                    }
                    if full_epochs.is_empty() && open_targets.is_empty() {
                        break;
                    }
                }
                if shutdown_requested && full_epochs.is_empty() && open_targets.is_empty() {
                    break;
                }
            }
        });
        Self {
            demand,
            cancellation,
            task: Some(task),
        }
    }

    pub fn demand(&self) -> PreparedServingRebuildDemand {
        self.demand.clone()
    }

    pub async fn shutdown(mut self) {
        self.cancellation.request();
        // Release request waiters before draining: the HTTP surface is torn
        // down after this worker, and a request parked on a ticket the
        // worker will no longer clear must not delay exit by its full wait.
        self.demand.registry.close();
        let _ = self.demand.sender.send(PreparedRebuildRequest::Shutdown);
        let drained = self
            .demand
            .lifecycle
            .stop_and_wait(PREPARED_SHUTDOWN_DRAIN_TIMEOUT);
        if !drained {
            tracing::warn!(
                target: "bcsp_prepared_serving",
                "prepared publication admissions did not drain before bounded shutdown",
            );
        }
        if let Some(task) = self.task.take() {
            let mut task = task;
            if tokio::time::timeout(PREPARED_SHUTDOWN_DRAIN_TIMEOUT, &mut task)
                .await
                .is_err()
            {
                task.abort();
                let _ = task.await;
            }
        }
    }
}

impl Drop for PreparedServingRebuildRuntime {
    fn drop(&mut self) {
        self.cancellation.request();
        self.demand.lifecycle.stop_accepting();
        self.demand.registry.close();
        let _ = self.demand.sender.send(PreparedRebuildRequest::Shutdown);
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

fn merge_rebuild_request(
    request: PreparedRebuildRequest,
    full_epochs: &mut BTreeSet<u64>,
    open_targets: &mut BTreeMap<TermCampusKey, BTreeSet<u64>>,
) -> bool {
    match request {
        PreparedRebuildRequest::Full(epoch) => {
            full_epochs.insert(epoch);
            false
        }
        PreparedRebuildRequest::Open(target, epoch) => {
            open_targets.entry(target).or_default().insert(epoch);
            false
        }
        PreparedRebuildRequest::Shutdown => true,
    }
}

fn requeue_open_tickets(
    pending: &mut BTreeMap<TermCampusKey, BTreeSet<u64>>,
    executed: &BTreeMap<TermCampusKey, BTreeSet<u64>>,
) {
    for (target, epochs) in executed {
        pending
            .entry(target.clone())
            .or_default()
            .extend(epochs.iter().copied());
    }
}

/// One ordered Catalog binding carried by every prepared request trace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedCatalogVectorEntry {
    pub target: TermCampusKey,
    pub content_version: CatalogContentVersion,
}

/// One ordered Catalog/Open binding carried by Search and Detail traces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedOpenVectorEntry {
    pub target: TermCampusKey,
    pub catalog_content_version: CatalogContentVersion,
    pub observation_sequence: u64,
    /// Internal overlay generation. This distinguishes a later failed/unsafe
    /// attempt that intentionally preserves the public observation sequence.
    /// It is trace-only and never widens the ordinary response wire.
    pub attempt_sequence: u64,
}

#[derive(Clone, Debug)]
pub struct PreparedOpenOverlay {
    pub catalog_content_version: CatalogContentVersion,
    pub observation_sequence: u64,
    pub attempt_sequence: u64,
    execution: Option<Arc<PreparedQueryOpenOverlay>>,
}

/// Immutable, version-bound serving generation.
///
/// Catalog projection, Open projection, option-dictionary source data and the
/// FTS5 index are built before this value is published. A request clones one
/// `Arc` and cannot observe a mixed generation if refresh publishes later.
pub struct PreparedServingSnapshot {
    foundation: Arc<PreparedCatalogFoundation>,
    open: BTreeMap<TermCampusKey, PreparedOpenOverlay>,
    resident_estimated_bytes: u64,
}

/// One admitted request generation plus the small precommit Open mask that was
/// observed at the same registry linearization point.
#[derive(Clone)]
pub struct PreparedServingBinding {
    snapshot: Arc<PreparedServingSnapshot>,
    forced_open_unavailable: BTreeSet<TermCampusKey>,
}

impl PreparedServingBinding {
    pub fn generation(&self) -> &Arc<PreparedServingSnapshot> {
        &self.snapshot
    }

    pub const fn forced_open_unavailable(&self) -> &BTreeSet<TermCampusKey> {
        &self.forced_open_unavailable
    }
}

impl Deref for PreparedServingBinding {
    type Target = PreparedServingSnapshot;

    fn deref(&self) -> &Self::Target {
        &self.snapshot
    }
}

struct PreparedCatalogFoundation {
    discovery: CatalogDiscoveryResponseV1,
    discovered_targets: BTreeSet<TermCampusKey>,
    ready_targets: BTreeSet<TermCampusKey>,
    catalogs: BTreeMap<TermCampusKey, Arc<NormalizedCatalogV1>>,
    query_corpora: BTreeMap<String, Arc<PreparedCatalogCorpus>>,
    dictionaries: BTreeMap<TermCampusKey, PreparedTargetDictionary>,
    source_generation: FoundationSourceGeneration,
    fts: PreparedFtsIndex,
    resident_estimated_bytes: u64,
    fts_index_bytes: u64,
}

struct PreparedTargetDictionary {
    options: BTreeMap<FilterOptionsFieldV2, Arc<[FilterOptionV2]>>,
    dynamic_values: BTreeMap<FilterFieldId, BTreeSet<String>>,
    keyword_fts_values: BTreeSet<String>,
    keyword_identifier_values: BTreeSet<String>,
}

const PREPARED_FOUNDATION_RESIDENT_BYTE_BUDGET: u64 = 512 * 1024 * 1024;
const PREPARED_SQLITE_CACHE_BYTE_BUDGET: u64 = 16 * 1024 * 1024;
const PREPARED_AGGREGATE_RESIDENT_BYTE_BUDGET: u64 =
    PREPARED_FOUNDATION_RESIDENT_BYTE_BUDGET + PREPARED_SQLITE_CACHE_BYTE_BUDGET;
static PREPARED_RESIDENT_HIGH_WATERMARK: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
struct FoundationSourceGeneration {
    scope: RutgersTermWindowScope,
    discovery_content_version: u64,
    discovery_semantic_sha256: Option<String>,
    targets: BTreeMap<TermCampusKey, FoundationCatalogGeneration>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FoundationCatalogGeneration {
    catalog_content_version: u64,
    ready: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FoundationTargetGeneration {
    catalog_content_version: u64,
    open_catalog_content_version: u64,
    ready: bool,
}

impl PreparedServingSnapshot {
    pub fn discovery(&self) -> &CatalogDiscoveryResponseV1 {
        &self.foundation.discovery
    }

    pub fn discovery_at(&self, observed_at: time::OffsetDateTime) -> CatalogDiscoveryResponseV1 {
        let mut discovery = self.foundation.discovery.clone();
        discovery.observed_at = observed_at;
        discovery
    }

    pub fn is_discovered(&self, target: &TermCampusKey) -> bool {
        self.foundation.discovered_targets.contains(target)
    }

    pub fn is_ready(&self, target: &TermCampusKey) -> bool {
        self.foundation.ready_targets.contains(target)
    }

    pub fn catalogs<'snapshot>(
        &'snapshot self,
        targets: &[TermCampusKey],
    ) -> Result<Vec<&'snapshot NormalizedCatalogV1>, PreparedServingError> {
        targets
            .iter()
            .map(|target| {
                self.foundation
                    .catalogs
                    .get(target)
                    .map(Arc::as_ref)
                    .ok_or_else(|| PreparedServingError::TargetNotPrepared(target.clone()))
            })
            .collect()
    }

    pub(crate) fn query_corpus(
        &self,
        target: &TermCampusKey,
    ) -> Result<&PreparedCatalogCorpus, PreparedServingError> {
        self.foundation
            .query_corpora
            .get(target.term().as_str())
            .map(Arc::as_ref)
            .ok_or_else(|| PreparedServingError::TargetNotPrepared(target.clone()))
    }

    pub(crate) fn query_open_overlays<'snapshot>(
        &'snapshot self,
        targets: &[TermCampusKey],
    ) -> Result<Vec<&'snapshot PreparedQueryOpenOverlay>, PreparedServingError> {
        targets
            .iter()
            .map(|target| {
                self.open
                    .get(target)
                    .and_then(|overlay| overlay.execution.as_deref())
                    .ok_or_else(|| PreparedServingError::TargetNotPrepared(target.clone()))
            })
            .collect()
    }

    pub fn open_evidence(
        &self,
        targets: &[TermCampusKey],
        now: time::OffsetDateTime,
    ) -> Result<Vec<OpenEvidence>, PreparedServingError> {
        let mut evidence = Vec::new();
        for target in targets {
            let overlay = self
                .open
                .get(target)
                .ok_or_else(|| PreparedServingError::TargetNotPrepared(target.clone()))?;
            evidence.extend(
                overlay
                    .execution
                    .as_deref()
                    .ok_or_else(|| PreparedServingError::TargetNotPrepared(target.clone()))?
                    .evidence()
                    .cloned()
                    .map(|evidence| materialize_open_evidence_at(evidence, now)),
            );
        }
        Ok(evidence)
    }

    pub fn catalog_vector(
        &self,
        targets: &[TermCampusKey],
    ) -> Result<Vec<PreparedCatalogVectorEntry>, PreparedServingError> {
        self.catalogs(targets).map(|catalogs| {
            catalogs
                .into_iter()
                .map(|catalog| PreparedCatalogVectorEntry {
                    target: catalog.target.clone(),
                    content_version: catalog.content_version,
                })
                .collect()
        })
    }

    pub fn open_vector(
        &self,
        targets: &[TermCampusKey],
    ) -> Result<Vec<PreparedOpenVectorEntry>, PreparedServingError> {
        targets
            .iter()
            .map(|target| {
                let overlay = self
                    .open
                    .get(target)
                    .ok_or_else(|| PreparedServingError::TargetNotPrepared(target.clone()))?;
                Ok(PreparedOpenVectorEntry {
                    target: target.clone(),
                    catalog_content_version: overlay.catalog_content_version,
                    observation_sequence: overlay.observation_sequence,
                    attempt_sequence: overlay.attempt_sequence,
                })
            })
            .collect()
    }

    pub fn text_hit_plan(
        &self,
        targets: &[TermCampusKey],
        query: &FilterSearchTextV1,
    ) -> Result<TextHitPlan, PreparedServingError> {
        let tokens = CourseTextSearchTokens::try_new(query.tokens())?;
        let catalogs = self.catalogs(targets)?;
        let mut bindings = Vec::with_capacity(catalogs.len());
        let mut hits = Vec::new();
        for catalog in catalogs {
            bindings.push(TextTargetVersion {
                target: catalog.target.clone(),
                content_version: catalog.content_version,
            });
            hits.extend(
                self.foundation
                    .fts
                    .search(&catalog.target, catalog.content_version.get(), &tokens)?
                    .into_iter()
                    .map(|hit| TextHit {
                        variant_key: hit.key,
                        fts_rank: hit.fts_rank,
                    }),
            );
        }
        TextHitPlan::try_new(query, bindings, hits).map_err(PreparedServingError::TextPlan)
    }

    pub fn keyword_terms(
        &self,
        target: &TermCampusKey,
        content_version: u64,
        query: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, PreparedServingError> {
        self.require_catalog_version(target, content_version)?;
        self.foundation
            .fts
            .terms(target, content_version, query, limit)
    }

    pub fn keyword_exists(
        &self,
        target: &TermCampusKey,
        content_version: u64,
        term: &str,
    ) -> Result<bool, PreparedServingError> {
        self.require_catalog_version(target, content_version)?;
        self.foundation
            .fts
            .term_exists(target, content_version, term)
    }

    pub fn prepared_filter_options(
        &self,
        targets: &[TermCampusKey],
        field: FilterOptionsFieldV2,
        query: Option<&str>,
    ) -> Result<Vec<FilterOptionV2>, PreparedServingError> {
        let query = if matches!(
            field,
            FilterOptionsFieldV2::Keyword | FilterOptionsFieldV2::Instructor
        ) {
            query
        } else {
            None
        };
        let query_unicode = query.map(str::to_lowercase);
        let query_ascii = query.map(str::to_ascii_lowercase);
        let mut merged = BTreeMap::new();
        for target in targets {
            let dictionary = self
                .foundation
                .dictionaries
                .get(target)
                .ok_or_else(|| PreparedServingError::TargetNotPrepared(target.clone()))?;
            let options = dictionary
                .options
                .get(&field)
                .ok_or(PreparedServingError::PreparedDictionaryUnavailable)?;
            for option in options.iter().filter(|option| match field {
                FilterOptionsFieldV2::Keyword => query.is_none_or(|_| {
                    let fts_match = dictionary.keyword_fts_values.contains(&option.value)
                        && query_ascii
                            .as_ref()
                            .is_some_and(|query| option.value.to_ascii_lowercase().contains(query));
                    let identifier_match =
                        dictionary.keyword_identifier_values.contains(&option.value)
                            && query_unicode
                                .as_ref()
                                .is_some_and(|query| option.value.to_lowercase().contains(query));
                    fts_match || identifier_match
                }),
                FilterOptionsFieldV2::Instructor => query_unicode.as_ref().is_none_or(|query| {
                    option.value.to_lowercase().contains(query)
                        || option.label.to_lowercase().contains(query)
                }),
                _ => true,
            }) {
                insert_prepared_option(&mut merged, option.value.clone(), option.label.clone());
            }
        }
        let mut options = merged.into_values().collect::<Vec<_>>();
        sort_prepared_options(field, &mut options);
        Ok(options)
    }

    pub fn invalid_dynamic_filter_values(
        &self,
        targets: &[TermCampusKey],
        filters: &NormalizedFilterValuesV1,
    ) -> Result<Vec<InvalidDynamicFilterValueV3>, PreparedServingError> {
        for target in targets {
            if !self.foundation.dictionaries.contains_key(target) {
                return Err(PreparedServingError::TargetNotPrepared(target.clone()));
            }
        }
        let mut invalid = Vec::new();
        if let Some(keywords) = filters.keywords() {
            for keyword in keywords.tokens() {
                self.push_invalid_if_missing(
                    targets,
                    FilterFieldId::CourseText,
                    keyword,
                    &mut invalid,
                );
            }
        }
        for subject in filters.subjects() {
            self.push_invalid_if_missing(
                targets,
                FilterFieldId::CourseSubject,
                subject.as_str(),
                &mut invalid,
            );
        }
        for level in filters.levels() {
            self.push_invalid_if_missing(
                targets,
                FilterFieldId::CourseLevel,
                level.as_str(),
                &mut invalid,
            );
        }
        for band in filters.course_number_bands() {
            self.push_invalid_if_missing(
                targets,
                FilterFieldId::CourseNumberBand,
                &band.to_string(),
                &mut invalid,
            );
        }
        for core in &filters.core().codes {
            self.push_invalid_if_missing(
                targets,
                FilterFieldId::CourseCoreCode,
                core.as_str(),
                &mut invalid,
            );
        }
        for instructor in filters.instructors() {
            self.push_invalid_if_missing(
                targets,
                FilterFieldId::SectionInstructor,
                instructor.as_str(),
                &mut invalid,
            );
        }
        for location in &filters.meeting_locations().locations {
            self.push_invalid_if_missing(
                targets,
                FilterFieldId::SectionMeetingLocation,
                location.as_str(),
                &mut invalid,
            );
        }
        for exam in filters.exam_codes() {
            self.push_invalid_if_missing(
                targets,
                FilterFieldId::SectionExam,
                exam.as_str(),
                &mut invalid,
            );
        }
        invalid.sort();
        invalid.dedup();
        Ok(invalid)
    }

    fn push_invalid_if_missing(
        &self,
        targets: &[TermCampusKey],
        field: FilterFieldId,
        value: &str,
        invalid: &mut Vec<InvalidDynamicFilterValueV3>,
    ) {
        let normalized = normalize_dynamic_value(field, value);
        let found = targets.iter().any(|target| {
            self.foundation
                .dictionaries
                .get(target)
                .and_then(|dictionary| dictionary.dynamic_values.get(&field))
                .is_some_and(|values| values.contains(&normalized))
        });
        if !found {
            invalid.push(InvalidDynamicFilterValueV3 {
                field,
                value: value.to_owned(),
            });
        }
    }

    fn require_catalog_version(
        &self,
        target: &TermCampusKey,
        content_version: u64,
    ) -> Result<(), PreparedServingError> {
        let catalog = self
            .foundation
            .catalogs
            .get(target)
            .ok_or_else(|| PreparedServingError::TargetNotPrepared(target.clone()))?;
        if catalog.content_version.get() == content_version {
            Ok(())
        } else {
            Err(PreparedServingError::PublicationChanged)
        }
    }

    fn authoritative_foundation_matches(
        &self,
        storage: &mut OperationalStorage,
        now: time::OffsetDateTime,
    ) -> Result<bool, PreparedServingError> {
        let current_targets = RutgersTermWindow::at(now, self.foundation.source_generation.scope)?
            .visible_terms()
            .iter()
            .flat_map(|term| product_target_keys(term.term()))
            .collect::<BTreeSet<_>>();
        if current_targets
            != self
                .foundation
                .source_generation
                .targets
                .keys()
                .cloned()
                .collect()
        {
            return Ok(false);
        }
        let discovery = storage.published_discovery_snapshot()?;
        if discovery.state.content_version
            != self.foundation.source_generation.discovery_content_version
            || discovery.state.semantic_sha256
                != self.foundation.source_generation.discovery_semantic_sha256
        {
            return Ok(false);
        }
        for (target, expected) in &self.foundation.source_generation.targets {
            let state = storage.complete_target_snapshot_state(target)?;
            let current = FoundationCatalogGeneration {
                catalog_content_version: state.catalog_content_version,
                ready: state.ready,
            };
            if &current != expected {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn materialize_open_evidence_at(
    mut evidence: OpenEvidence,
    now: time::OffsetDateTime,
) -> OpenEvidence {
    let fresh = evidence.evidence.observed_at.is_some()
        && evidence
            .evidence
            .fresh_until
            .is_some_and(|fresh_until| now <= fresh_until);
    if !fresh && evidence.evidence.uncertainty.is_none() {
        evidence.evidence.uncertainty = Some(bcsp_contracts::MatchReasonCode::SourceUnavailable);
    }
    evidence
}

fn build_prepared_dictionaries(
    catalogs: &BTreeMap<TermCampusKey, Arc<NormalizedCatalogV1>>,
    fts: &PreparedFtsIndex,
    catalog_estimated_bytes: u64,
    cancellation: Option<&PreparedRebuildCancellation>,
) -> Result<(BTreeMap<TermCampusKey, PreparedTargetDictionary>, u64), PreparedServingError> {
    let mut dictionaries = BTreeMap::new();
    let mut estimated_bytes = 0_u64;
    for (target, catalog) in catalogs {
        check_prepared_rebuild_cancellation(cancellation)?;
        let keyword_fts_values = fts.term_set(target, catalog.content_version.get())?;
        let keyword_identifier_values = catalog
            .course_groups
            .iter()
            .map(|group| group.key.course_string().as_str().to_owned())
            .collect::<BTreeSet<_>>();
        let mut options = BTreeMap::new();
        for field in [
            FilterOptionsFieldV2::Keyword,
            FilterOptionsFieldV2::CourseNumberBand,
            FilterOptionsFieldV2::CourseLevel,
            FilterOptionsFieldV2::Instructor,
            FilterOptionsFieldV2::MeetingLocation,
            FilterOptionsFieldV2::ExamCode,
        ] {
            check_prepared_rebuild_cancellation(cancellation)?;
            let mut values = if field == FilterOptionsFieldV2::Keyword {
                let mut values = BTreeMap::new();
                for term in &keyword_fts_values {
                    let term = term.clone();
                    insert_prepared_option(&mut values, term.clone(), term);
                }
                for identifier in &keyword_identifier_values {
                    let identifier = identifier.clone();
                    insert_prepared_option(&mut values, identifier.clone(), identifier);
                }
                values
            } else {
                crate::query_service::collect_catalog_options(&[catalog.as_ref()], field, None)
            }
            .into_values()
            .collect::<Vec<_>>();
            sort_prepared_options(field, &mut values);
            options.insert(field, Arc::<[FilterOptionV2]>::from(values));
        }

        let mut dynamic_values = BTreeMap::new();
        dynamic_values.insert(
            FilterFieldId::CourseText,
            keyword_fts_values
                .iter()
                .chain(&keyword_identifier_values)
                .map(|value| normalize_dynamic_value(FilterFieldId::CourseText, value))
                .collect(),
        );
        for (option_field, filter_field) in [
            (
                FilterOptionsFieldV2::CourseNumberBand,
                FilterFieldId::CourseNumberBand,
            ),
            (
                FilterOptionsFieldV2::CourseLevel,
                FilterFieldId::CourseLevel,
            ),
            (
                FilterOptionsFieldV2::Instructor,
                FilterFieldId::SectionInstructor,
            ),
            (
                FilterOptionsFieldV2::MeetingLocation,
                FilterFieldId::SectionMeetingLocation,
            ),
            (FilterOptionsFieldV2::ExamCode, FilterFieldId::SectionExam),
        ] {
            dynamic_values.insert(
                filter_field,
                options[&option_field]
                    .iter()
                    .map(|option| normalize_dynamic_value(filter_field, &option.value))
                    .collect(),
            );
        }
        dynamic_values.insert(
            FilterFieldId::CourseSubject,
            catalog
                .course_variants
                .iter()
                .filter_map(|variant| crate::query_service::known_value(&variant.subject_code))
                .map(|value| normalize_dynamic_value(FilterFieldId::CourseSubject, value.as_str()))
                .collect(),
        );
        dynamic_values.insert(
            FilterFieldId::CourseCoreCode,
            catalog
                .course_variants
                .iter()
                .filter_map(|variant| crate::query_service::known_value(&variant.core_codes))
                .flatten()
                .map(|value| normalize_dynamic_value(FilterFieldId::CourseCoreCode, value.as_str()))
                .collect(),
        );
        let dictionary = PreparedTargetDictionary {
            options,
            dynamic_values,
            keyword_fts_values,
            keyword_identifier_values,
        };
        estimated_bytes = estimated_bytes
            .checked_add(prepared_dictionary_estimated_bytes(&dictionary)?)
            .ok_or(PreparedServingError::StoredIntegerOutOfRange)?;
        ensure_prepared_resident_budget(
            catalog_estimated_bytes
                .checked_add(estimated_bytes)
                .ok_or(PreparedServingError::StoredIntegerOutOfRange)?,
        )?;
        dictionaries.insert(target.clone(), dictionary);
    }
    Ok((dictionaries, estimated_bytes))
}

fn insert_prepared_option(
    options: &mut BTreeMap<String, FilterOptionV2>,
    value: String,
    label: String,
) {
    crate::query_service::insert_option(options, value, label);
}

fn normalize_dynamic_value(field: FilterFieldId, value: &str) -> String {
    match field {
        // The authoritative subject validator compares canonical contract
        // values case-sensitively; subject codes are digits, so nothing is
        // lost.
        FilterFieldId::CourseSubject => value.to_owned(),
        // Core codes are mixed-case in the Rutgers feed ("AHo", "WCd"), while
        // a request's codes are canonicalized to ASCII uppercase by
        // `NormalizedFilterValuesV1::try_new` and the engine compares them
        // case-insensitively. Both the dictionary and the lookup come through
        // here, so both meet in uppercase; the discovery dictionary keeps the
        // feed's spelling for display.
        FilterFieldId::CourseCoreCode => value.to_ascii_uppercase(),
        // SQLite lower()/NOCASE and Course identifier eq_ignore_ascii_case are
        // ASCII-only; Rust Unicode lowercasing would accept values the
        // reference path rejects.
        FilterFieldId::CourseText => value.to_ascii_lowercase(),
        _ => value.to_lowercase(),
    }
}

fn sort_prepared_options(field: FilterOptionsFieldV2, options: &mut [FilterOptionV2]) {
    if field == FilterOptionsFieldV2::CourseNumberBand {
        options.sort_by_key(|option| option.value.parse::<u32>().unwrap_or(u32::MAX));
    } else {
        options.sort_by(|left, right| {
            left.label
                .to_lowercase()
                .cmp(&right.label.to_lowercase())
                .then_with(|| left.value.cmp(&right.value))
        });
    }
}

fn prepared_dictionary_estimated_bytes(
    dictionary: &PreparedTargetDictionary,
) -> Result<u64, PreparedServingError> {
    let mut bytes = u64::try_from(std::mem::size_of::<PreparedTargetDictionary>())
        .map_err(|_| PreparedServingError::StoredIntegerOutOfRange)?;
    for options in dictionary.options.values() {
        bytes = bytes
            .checked_add(
                u64::try_from(options.len())
                    .map_err(|_| PreparedServingError::StoredIntegerOutOfRange)?
                    .saturating_mul(
                        u64::try_from(
                            std::mem::size_of::<FilterOptionV2>()
                                + 3 * std::mem::size_of::<usize>(),
                        )
                        .map_err(|_| PreparedServingError::StoredIntegerOutOfRange)?,
                    ),
            )
            .ok_or(PreparedServingError::StoredIntegerOutOfRange)?;
        for option in options.iter() {
            bytes = bytes
                .checked_add(u64::try_from(option.value.capacity()).unwrap_or(u64::MAX))
                .and_then(|bytes| {
                    bytes.checked_add(u64::try_from(option.label.capacity()).unwrap_or(u64::MAX))
                })
                .ok_or(PreparedServingError::StoredIntegerOutOfRange)?;
        }
    }
    for values in dictionary.dynamic_values.values() {
        bytes = bytes
            .checked_add(
                u64::try_from(values.len())
                    .map_err(|_| PreparedServingError::StoredIntegerOutOfRange)?
                    .saturating_mul(
                        u64::try_from(
                            std::mem::size_of::<String>() + 3 * std::mem::size_of::<usize>(),
                        )
                        .map_err(|_| PreparedServingError::StoredIntegerOutOfRange)?,
                    ),
            )
            .ok_or(PreparedServingError::StoredIntegerOutOfRange)?;
        for value in values {
            bytes = bytes
                .checked_add(u64::try_from(value.capacity()).unwrap_or(u64::MAX))
                .ok_or(PreparedServingError::StoredIntegerOutOfRange)?;
        }
    }
    for values in [
        &dictionary.keyword_fts_values,
        &dictionary.keyword_identifier_values,
    ] {
        bytes = bytes
            .checked_add(
                u64::try_from(values.len())
                    .map_err(|_| PreparedServingError::StoredIntegerOutOfRange)?
                    .saturating_mul(
                        u64::try_from(
                            std::mem::size_of::<String>() + 3 * std::mem::size_of::<usize>(),
                        )
                        .map_err(|_| PreparedServingError::StoredIntegerOutOfRange)?,
                    ),
            )
            .ok_or(PreparedServingError::StoredIntegerOutOfRange)?;
        for value in values {
            bytes = bytes
                .checked_add(u64::try_from(value.capacity()).unwrap_or(u64::MAX))
                .ok_or(PreparedServingError::StoredIntegerOutOfRange)?;
        }
    }
    Ok(bytes)
}

/// One process-local publication cell. Build failures never replace the prior
/// generation, which is the prepared LKG contract.
#[derive(Default)]
pub struct PreparedServingRegistry {
    current: RwLock<Option<Arc<PreparedServingSnapshot>>>,
    rebuild: Mutex<()>,
    dirty: Mutex<PreparedDirtyState>,
    /// Signalled (under `dirty`) on every transition that can open admission
    /// and when the registry closes, so [`Self::snapshot_within`] waiters wake
    /// exactly when the answer may have changed. A std Condvar because every
    /// request path is synchronous and already runs on tokio's blocking pool.
    dirty_changed: Condvar,
    /// Set once the rebuild worker is gone. Committed tickets then stay
    /// fail-closed (nothing will clear them), so waiting is pointless:
    /// waiters are released and fail fast instead of sitting out the timeout.
    closed: AtomicBool,
    hits: AtomicU64,
    misses: AtomicU64,
    wait_timeouts: AtomicU64,
    publishes: AtomicU64,
    replacements: AtomicU64,
    evictions: AtomicU64,
}

#[derive(Default)]
struct PreparedDirtyState {
    next_epoch: u64,
    full_epochs: BTreeMap<u64, DirtyPhase>,
    open_epochs: BTreeMap<TermCampusKey, BTreeMap<u64, DirtyPhase>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DirtyPhase {
    Precommit,
    Committed,
}

impl PreparedServingRegistry {
    pub fn rebuild_guard(&self) -> Result<std::sync::MutexGuard<'_, ()>, PreparedServingError> {
        self.rebuild
            .lock()
            .map_err(|_| PreparedServingError::RegistryUnavailable)
    }

    /// Binds the current generation without waiting: a committed publication
    /// barrier fails the call immediately with
    /// [`PreparedServingError::SnapshotRebuilding`]. Request paths use
    /// [`Self::snapshot_within`]; this zero-wait variant serves rebuilds,
    /// tests and anything that must never park a thread.
    pub fn snapshot(&self) -> Result<PreparedServingBinding, PreparedServingError> {
        self.snapshot_within(Duration::ZERO)
    }

    /// Binds the current generation, waiting up to `timeout` for admission to
    /// open when a committed Full/Open ticket is blocking it.
    ///
    /// The invariant is unchanged: a request is never bound to an overlay a
    /// committed authoritative transaction has superseded. The wait only
    /// turns "fail now" into "fail after `timeout`" -- once the worker clears
    /// the ticket the binding is to the NEW generation, exactly as
    /// [`Self::snapshot`] would return it. A registry with no generation at
    /// all never waits, and a closed registry (worker gone) releases waiters
    /// immediately because nothing will clear the ticket.
    ///
    /// Callers must not hold any lock the rebuild worker needs while waiting.
    pub fn snapshot_within(
        &self,
        timeout: Duration,
    ) -> Result<PreparedServingBinding, PreparedServingError> {
        // Keep the admission guard through the Arc clone. This is the
        // linearization point shared with mark_full_dirty/mark_open_dirty;
        // checking dirty and then dropping it before reading current would
        // allow a new reference to cross a publication barrier.
        let mut dirty = self
            .dirty
            .lock()
            .map_err(|_| PreparedServingError::RegistryUnavailable)?;
        let wait_started = Instant::now();
        let mut timed_out = false;
        if admission_blocked(&dirty) && !timeout.is_zero() {
            if self
                .current
                .read()
                .map_err(|_| PreparedServingError::RegistryUnavailable)?
                .is_none()
            {
                // No generation has ever been published: waiting cannot
                // produce one, and the caller's answer is "unavailable", not
                // "rebuilding".
                self.trace_snapshot_access(false, 0, false);
                return Err(PreparedServingError::SnapshotUnavailable);
            }
            let deadline = wait_started + timeout;
            while admission_blocked(&dirty) {
                if self.closed.load(Ordering::Acquire) {
                    break;
                }
                let now = Instant::now();
                if now >= deadline {
                    timed_out = true;
                    break;
                }
                let (guard, _) = self
                    .dirty_changed
                    .wait_timeout(dirty, deadline - now)
                    .map_err(|_| PreparedServingError::RegistryUnavailable)?;
                dirty = guard;
            }
        }
        let waited_us = elapsed_micros_u64(wait_started);
        if admission_blocked(&dirty) {
            // Once an authoritative Full/Open transaction commits, the public
            // observation/attempt vector may have advanced. Never bind a new
            // request to the superseded overlay while the worker catches up.
            // Arcs pinned before the barrier remain valid for their request.
            self.trace_snapshot_access(false, waited_us, timed_out);
            return Err(PreparedServingError::SnapshotRebuilding);
        }
        let snapshot = self
            .current
            .read()
            .map_err(|_| PreparedServingError::RegistryUnavailable)?
            .clone()
            .ok_or(PreparedServingError::SnapshotUnavailable)?;
        let forced_open_unavailable = dirty.open_epochs.keys().cloned().collect();
        self.trace_snapshot_access(true, waited_us, false);
        Ok(PreparedServingBinding {
            snapshot,
            forced_open_unavailable,
        })
    }

    /// Releases every [`Self::snapshot_within`] waiter and makes later calls
    /// fail fast while a committed ticket blocks admission. Called when the
    /// rebuild worker stops; the last-good generation stays bound for
    /// zero-wait callers. Idempotent.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Release);
        // Take the dirty lock before notifying so a waiter that has checked
        // the flag but not yet parked cannot miss the wake-up.
        if let Ok(dirty) = self.dirty.lock() {
            self.dirty_changed.notify_all();
            drop(dirty);
        }
    }

    fn reopen(&self) {
        self.closed.store(false, Ordering::Release);
    }

    /// Number of [`Self::snapshot_within`] calls that gave up after waiting
    /// their full timeout. A rising count means the rebuild window is longer
    /// than the request admission wait.
    pub fn snapshot_wait_timeouts(&self) -> u64 {
        self.wait_timeouts.load(Ordering::Relaxed)
    }

    /// Wakes waiters after a transition that may have opened admission. Must
    /// be called while `dirty` is held so the wake-up cannot be lost.
    fn notify_admission_changed(&self, _dirty: &PreparedDirtyState) {
        self.dirty_changed.notify_all();
    }

    fn snapshot_for_rebuild(&self) -> Result<Arc<PreparedServingSnapshot>, PreparedServingError> {
        self.current
            .read()
            .map_err(|_| PreparedServingError::RegistryUnavailable)?
            .clone()
            .ok_or(PreparedServingError::SnapshotUnavailable)
    }

    fn trace_snapshot_access(&self, hit: bool, waited_us: u64, wait_timed_out: bool) {
        let (hits, misses) = if hit {
            (
                self.hits.fetch_add(1, Ordering::Relaxed).saturating_add(1),
                self.misses.load(Ordering::Relaxed),
            )
        } else {
            (
                self.hits.load(Ordering::Relaxed),
                self.misses
                    .fetch_add(1, Ordering::Relaxed)
                    .saturating_add(1),
            )
        };
        let wait_timeouts = if wait_timed_out {
            self.wait_timeouts
                .fetch_add(1, Ordering::Relaxed)
                .saturating_add(1)
        } else {
            self.wait_timeouts.load(Ordering::Relaxed)
        };
        tracing::debug!(
            target: "bcsp_prepared_serving",
            phase = "prepared_snapshot_access",
            hit,
            hits,
            misses,
            waited_us,
            wait_timed_out,
            wait_timeouts,
            publishes = self.publishes.load(Ordering::Relaxed),
            replacements = self.replacements.load(Ordering::Relaxed),
            evictions = self.evictions.load(Ordering::Relaxed),
            capacity = 1_u8,
        );
    }

    fn mark_full_dirty(&self) -> Result<u64, PreparedServingError> {
        let mut dirty = self
            .dirty
            .lock()
            .map_err(|_| PreparedServingError::RegistryUnavailable)?;
        let epoch = next_dirty_epoch(&mut dirty);
        dirty.full_epochs.insert(epoch, DirtyPhase::Precommit);
        Ok(epoch)
    }

    fn commit_full_dirty(&self, epoch: u64) -> Result<(), PreparedServingError> {
        let mut dirty = self
            .dirty
            .lock()
            .map_err(|_| PreparedServingError::RegistryUnavailable)?;
        let phase = dirty
            .full_epochs
            .get_mut(&epoch)
            .ok_or(PreparedServingError::PublicationBarrierUnavailable)?;
        *phase = DirtyPhase::Committed;
        Ok(())
    }

    fn rollback_full_dirty(&self, epoch: u64) {
        if let Ok(mut dirty) = self.dirty.lock()
            && dirty.full_epochs.get(&epoch) == Some(&DirtyPhase::Precommit)
        {
            dirty.full_epochs.remove(&epoch);
            self.notify_admission_changed(&dirty);
        }
    }

    fn mark_open_dirty(
        &self,
        target: &TermCampusKey,
    ) -> Result<OpenDirtyAdmission, PreparedServingError> {
        let mut dirty = self
            .dirty
            .lock()
            .map_err(|_| PreparedServingError::RegistryUnavailable)?;
        let epoch = next_dirty_epoch(&mut dirty);
        dirty
            .open_epochs
            .entry(target.clone())
            .or_default()
            .insert(epoch, DirtyPhase::Precommit);
        Ok(OpenDirtyAdmission { epoch })
    }

    fn commit_open_dirty(
        &self,
        target: &TermCampusKey,
        epoch: u64,
    ) -> Result<(), PreparedServingError> {
        let mut dirty = self
            .dirty
            .lock()
            .map_err(|_| PreparedServingError::RegistryUnavailable)?;
        let phase = dirty
            .open_epochs
            .get_mut(target)
            .and_then(|epochs| epochs.get_mut(&epoch))
            .ok_or(PreparedServingError::PublicationBarrierUnavailable)?;
        *phase = DirtyPhase::Committed;
        Ok(())
    }

    fn convert_full_dirty_to_open(
        &self,
        full_epoch: u64,
        target: &TermCampusKey,
    ) -> Result<u64, PreparedServingError> {
        let mut dirty = self
            .dirty
            .lock()
            .map_err(|_| PreparedServingError::RegistryUnavailable)?;
        let phase = dirty
            .full_epochs
            .remove(&full_epoch)
            .ok_or(PreparedServingError::PublicationBarrierUnavailable)?;
        if phase != DirtyPhase::Precommit {
            dirty.full_epochs.insert(full_epoch, phase);
            return Err(PreparedServingError::PublicationBarrierUnavailable);
        }
        let open_epoch = next_dirty_epoch(&mut dirty);
        dirty
            .open_epochs
            .entry(target.clone())
            .or_default()
            .insert(open_epoch, DirtyPhase::Committed);
        // Still blocked by the committed Open ticket; waking is harmless and
        // keeps "every ticket removal notifies" a rule without exceptions.
        self.notify_admission_changed(&dirty);
        Ok(open_epoch)
    }

    fn rollback_open_dirty(&self, target: &TermCampusKey, admission: OpenDirtyAdmission) {
        let Ok(mut dirty) = self.dirty.lock() else {
            return;
        };
        if dirty
            .open_epochs
            .get(target)
            .and_then(|epochs| epochs.get(&admission.epoch))
            == Some(&DirtyPhase::Precommit)
            && let Some(epochs) = dirty.open_epochs.get_mut(target)
        {
            epochs.remove(&admission.epoch);
            if epochs.is_empty() {
                dirty.open_epochs.remove(target);
            }
            self.notify_admission_changed(&dirty);
        }
    }

    fn clear_full_tickets(&self, completed: &BTreeSet<u64>) {
        if let Ok(mut dirty) = self.dirty.lock() {
            dirty.full_epochs.retain(|epoch, phase| {
                !completed.contains(epoch) || *phase == DirtyPhase::Precommit
            });
            self.notify_admission_changed(&dirty);
        }
    }

    fn clear_open_tickets(&self, target: &TermCampusKey, completed: &BTreeSet<u64>) {
        if let Ok(mut dirty) = self.dirty.lock()
            && let Some(epochs) = dirty.open_epochs.get_mut(target)
        {
            epochs.retain(|epoch, phase| {
                !completed.contains(epoch) || *phase == DirtyPhase::Precommit
            });
            if epochs.is_empty() {
                dirty.open_epochs.remove(target);
            }
            self.notify_admission_changed(&dirty);
        }
    }

    fn promote_open_tickets_to_full(
        &self,
        executed: &BTreeMap<TermCampusKey, BTreeSet<u64>>,
    ) -> Result<u64, PreparedServingError> {
        let mut dirty = self
            .dirty
            .lock()
            .map_err(|_| PreparedServingError::RegistryUnavailable)?;
        for (target, epochs) in executed {
            let active = dirty
                .open_epochs
                .get(target)
                .ok_or(PreparedServingError::PublicationBarrierUnavailable)?;
            if epochs
                .iter()
                .any(|epoch| active.get(epoch) != Some(&DirtyPhase::Committed))
            {
                return Err(PreparedServingError::PublicationBarrierUnavailable);
            }
        }
        for (target, epochs) in executed {
            if let Some(active) = dirty.open_epochs.get_mut(target) {
                active.retain(|epoch, _| !epochs.contains(epoch));
                if active.is_empty() {
                    dirty.open_epochs.remove(target);
                }
            }
        }
        let epoch = next_dirty_epoch(&mut dirty);
        dirty.full_epochs.insert(epoch, DirtyPhase::Committed);
        // Admission stays blocked behind the committed Full ticket; see
        // convert_full_dirty_to_open for why every removal still notifies.
        self.notify_admission_changed(&dirty);
        Ok(epoch)
    }

    pub fn publish_if_unchanged(
        &self,
        expected: Option<&Arc<PreparedServingSnapshot>>,
        snapshot: PreparedServingSnapshot,
    ) -> Result<Arc<PreparedServingSnapshot>, PreparedServingError> {
        let publish_started = Instant::now();
        let snapshot = Arc::new(snapshot);
        let mut current = self
            .current
            .write()
            .map_err(|_| PreparedServingError::RegistryUnavailable)?;
        let unchanged = match (&*current, expected) {
            (None, None) => true,
            (Some(current), Some(expected)) => Arc::ptr_eq(current, expected),
            (None, Some(_)) | (Some(_), None) => false,
        };
        if !unchanged {
            return Err(PreparedServingError::SupersededBuild);
        }
        let replacing = current.is_some();
        *current = Some(Arc::clone(&snapshot));
        let publishes = self
            .publishes
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1);
        let replacements = if replacing {
            self.replacements
                .fetch_add(1, Ordering::Relaxed)
                .saturating_add(1)
        } else {
            self.replacements.load(Ordering::Relaxed)
        };
        let evictions = if replacing {
            self.evictions
                .fetch_add(1, Ordering::Relaxed)
                .saturating_add(1)
        } else {
            self.evictions.load(Ordering::Relaxed)
        };
        tracing::info!(
            target: "bcsp_prepared_serving",
            phase = "prepared_snapshot_publish",
            elapsed_us = u64::try_from(publish_started.elapsed().as_micros()).unwrap_or(u64::MAX),
            publishes,
            replacements,
            evictions,
            replaced = replacing,
            capacity = 1_u8,
            foundation_resident_estimated_bytes = snapshot.foundation.resident_estimated_bytes,
            snapshot_resident_estimated_bytes = snapshot.resident_estimated_bytes,
            fts_index_bytes = snapshot.foundation.fts_index_bytes,
            sqlite_cache_budget_bytes = PREPARED_SQLITE_CACHE_BYTE_BUDGET,
            aggregate_resident_budget_bytes = PREPARED_AGGREGATE_RESIDENT_BYTE_BUDGET,
        );
        Ok(snapshot)
    }
}

struct OpenDirtyAdmission {
    epoch: u64,
}

fn next_dirty_epoch(dirty: &mut PreparedDirtyState) -> u64 {
    dirty.next_epoch = dirty.next_epoch.saturating_add(1).max(1);
    dirty.next_epoch
}

/// The single admission predicate: any Full ticket, or any COMMITTED Open
/// ticket, closes new bindings. Precommit Open tickets only mask their target
/// as conservatively unavailable and never block.
fn admission_blocked(dirty: &PreparedDirtyState) -> bool {
    !dirty.full_epochs.is_empty()
        || dirty
            .open_epochs
            .values()
            .any(|epochs| epochs.values().any(|phase| *phase == DirtyPhase::Committed))
}

fn elapsed_micros_u64(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

/// Serializes and publishes one full Catalog/FTS generation from an
/// independent read-only SQLite connection whenever the store is file-backed.
pub fn rebuild_prepared_serving_from_access<S, C, P>(
    storage_access: &S,
    runtime: &SharedRuntimeContext<C, P>,
    open_runtime: &OpenRuntimeSnapshotRegistry,
    service_runtime: ServiceRuntimeV1,
    registry: &PreparedServingRegistry,
) -> Result<Arc<PreparedServingSnapshot>, PreparedServingError>
where
    S: crate::ProductStorageAccess,
    C: ApplicationClock,
    P: RefreshPolicyProvider,
{
    rebuild_prepared_serving_from_access_inner(
        storage_access,
        runtime,
        open_runtime,
        service_runtime,
        registry,
        None,
    )
}

fn rebuild_prepared_serving_from_access_with_cancellation<S, C, P>(
    storage_access: &S,
    runtime: &SharedRuntimeContext<C, P>,
    open_runtime: &OpenRuntimeSnapshotRegistry,
    service_runtime: ServiceRuntimeV1,
    registry: &PreparedServingRegistry,
    cancellation: &PreparedRebuildCancellation,
) -> Result<Arc<PreparedServingSnapshot>, PreparedServingError>
where
    S: crate::ProductStorageAccess,
    C: ApplicationClock,
    P: RefreshPolicyProvider,
{
    rebuild_prepared_serving_from_access_inner(
        storage_access,
        runtime,
        open_runtime,
        service_runtime,
        registry,
        Some(cancellation),
    )
}

fn rebuild_prepared_serving_from_access_inner<S, C, P>(
    storage_access: &S,
    runtime: &SharedRuntimeContext<C, P>,
    open_runtime: &OpenRuntimeSnapshotRegistry,
    service_runtime: ServiceRuntimeV1,
    registry: &PreparedServingRegistry,
    cancellation: Option<&PreparedRebuildCancellation>,
) -> Result<Arc<PreparedServingSnapshot>, PreparedServingError>
where
    S: crate::ProductStorageAccess,
    C: ApplicationClock,
    P: RefreshPolicyProvider,
{
    check_prepared_rebuild_cancellation(cancellation)?;
    let _rebuild = registry.rebuild_guard()?;
    let expected = registry.snapshot_for_rebuild().ok();
    check_prepared_rebuild_cancellation(cancellation)?;
    let mut storage = storage_access
        .lock_operational()
        .map_err(|_| PreparedServingError::StorageAccessUnavailable)?;
    let reader = storage.open_prepared_snapshot_reader()?;
    let snapshot = if let Some(mut reader) = reader {
        drop(storage);
        build_prepared_serving_snapshot_inner(
            &mut reader,
            runtime,
            open_runtime,
            service_runtime,
            cancellation,
        )?
    } else {
        build_prepared_serving_snapshot_inner(
            &mut storage,
            runtime,
            open_runtime,
            service_runtime,
            cancellation,
        )?
    };
    check_prepared_rebuild_cancellation(cancellation)?;
    registry.publish_if_unchanged(expected.as_ref(), snapshot)
}

/// Serializes and publishes a lightweight Open-only generation. The Catalog
/// foundation and full FTS corpus are shared by `Arc` and are never rebuilt.
pub fn rebuild_prepared_open_from_access<S, C, P>(
    storage_access: &S,
    runtime: &SharedRuntimeContext<C, P>,
    open_runtime: &OpenRuntimeSnapshotRegistry,
    targets: &[TermCampusKey],
    registry: &PreparedServingRegistry,
) -> Result<Arc<PreparedServingSnapshot>, PreparedServingError>
where
    S: crate::ProductStorageAccess,
    C: ApplicationClock,
    P: RefreshPolicyProvider,
{
    rebuild_prepared_open_from_access_inner(
        storage_access,
        runtime,
        open_runtime,
        targets,
        registry,
        None,
    )
}

fn rebuild_prepared_open_from_access_with_cancellation<S, C, P>(
    storage_access: &S,
    runtime: &SharedRuntimeContext<C, P>,
    open_runtime: &OpenRuntimeSnapshotRegistry,
    targets: &[TermCampusKey],
    registry: &PreparedServingRegistry,
    cancellation: &PreparedRebuildCancellation,
) -> Result<Arc<PreparedServingSnapshot>, PreparedServingError>
where
    S: crate::ProductStorageAccess,
    C: ApplicationClock,
    P: RefreshPolicyProvider,
{
    rebuild_prepared_open_from_access_inner(
        storage_access,
        runtime,
        open_runtime,
        targets,
        registry,
        Some(cancellation),
    )
}

fn rebuild_prepared_open_from_access_inner<S, C, P>(
    storage_access: &S,
    runtime: &SharedRuntimeContext<C, P>,
    open_runtime: &OpenRuntimeSnapshotRegistry,
    targets: &[TermCampusKey],
    registry: &PreparedServingRegistry,
    cancellation: Option<&PreparedRebuildCancellation>,
) -> Result<Arc<PreparedServingSnapshot>, PreparedServingError>
where
    S: crate::ProductStorageAccess,
    C: ApplicationClock,
    P: RefreshPolicyProvider,
{
    check_prepared_rebuild_cancellation(cancellation)?;
    let _rebuild = registry.rebuild_guard()?;
    let expected = registry.snapshot_for_rebuild()?;
    check_prepared_rebuild_cancellation(cancellation)?;
    let mut storage = storage_access
        .lock_operational()
        .map_err(|_| PreparedServingError::StorageAccessUnavailable)?;
    let reader = storage.open_prepared_snapshot_reader()?;
    let snapshot = if let Some(mut reader) = reader {
        drop(storage);
        rebuild_prepared_open_overlay(&mut reader, runtime, open_runtime, &expected, targets)?
    } else {
        rebuild_prepared_open_overlay(&mut storage, runtime, open_runtime, &expected, targets)?
    };
    check_prepared_rebuild_cancellation(cancellation)?;
    registry.publish_if_unchanged(Some(&expected), snapshot)
}

fn check_prepared_rebuild_cancellation(
    cancellation: Option<&PreparedRebuildCancellation>,
) -> Result<(), PreparedServingError> {
    cancellation.map_or(Ok(()), PreparedRebuildCancellation::check)
}

fn authoritative_foundation_matches_access<S>(
    storage_access: &S,
    registry: &PreparedServingRegistry,
) -> Result<bool, PreparedServingError>
where
    S: crate::ProductStorageAccess,
{
    let snapshot = registry.snapshot_for_rebuild()?;
    let mut storage = storage_access
        .lock_operational()
        .map_err(|_| PreparedServingError::StorageAccessUnavailable)?;
    let reader = storage.open_prepared_snapshot_reader()?;
    if let Some(mut reader) = reader {
        drop(storage);
        snapshot.authoritative_foundation_matches(&mut reader, SystemApplicationClock.now())
    } else {
        snapshot.authoritative_foundation_matches(&mut storage, SystemApplicationClock.now())
    }
}

/// Builds a complete generation from one SQLite read snapshot. Callers use a
/// dedicated read-only connection for production rebuilds; the in-memory
/// fixture fallback runs only during startup/test composition.
pub fn build_prepared_serving_snapshot<C, P>(
    storage: &mut OperationalStorage,
    runtime: &SharedRuntimeContext<C, P>,
    open_runtime: &OpenRuntimeSnapshotRegistry,
    service_runtime: ServiceRuntimeV1,
) -> Result<PreparedServingSnapshot, PreparedServingError>
where
    C: ApplicationClock,
    P: RefreshPolicyProvider,
{
    build_prepared_serving_snapshot_inner(storage, runtime, open_runtime, service_runtime, None)
}

fn build_prepared_serving_snapshot_inner<C, P>(
    storage: &mut OperationalStorage,
    runtime: &SharedRuntimeContext<C, P>,
    open_runtime: &OpenRuntimeSnapshotRegistry,
    service_runtime: ServiceRuntimeV1,
    cancellation: Option<&PreparedRebuildCancellation>,
) -> Result<PreparedServingSnapshot, PreparedServingError>
where
    C: ApplicationClock,
    P: RefreshPolicyProvider,
{
    check_prepared_rebuild_cancellation(cancellation)?;
    let build_started = Instant::now();
    let now = runtime.now();
    let policy = runtime.refresh_policy()?;
    let scope = match service_runtime {
        ServiceRuntimeV1::Local => RutgersTermWindowScope::Local,
        ServiceRuntimeV1::Public => RutgersTermWindowScope::Public,
    };
    let window = RutgersTermWindow::at(now, scope)?;
    let allowed_targets = window
        .visible_terms()
        .iter()
        .flat_map(|term| product_target_keys(term.term()))
        .collect::<BTreeSet<_>>();

    let published_discovery = storage.published_discovery_snapshot()?;
    let selector = to_catalog_discovery_response_v1(&published_discovery, &[], now)?;
    let discovered_targets = selector
        .targets
        .iter()
        .map(|target| target.key.clone())
        .filter(|target| {
            allowed_targets.contains(target) && is_product_campus(target.campus().as_str())
        })
        .collect::<BTreeSet<_>>();

    let target_generations = allowed_targets
        .iter()
        .map(|target| {
            let state = storage.complete_target_snapshot_state(target)?;
            Ok((target.clone(), foundation_generation(&state)))
        })
        .collect::<Result<BTreeMap<TermCampusKey, FoundationTargetGeneration>, StorageError>>()?;
    let source_generation = FoundationSourceGeneration {
        scope,
        discovery_content_version: published_discovery.state.content_version,
        discovery_semantic_sha256: published_discovery.state.semantic_sha256.clone(),
        targets: target_generations
            .iter()
            .map(|(target, generation)| {
                (
                    target.clone(),
                    FoundationCatalogGeneration {
                        catalog_content_version: generation.catalog_content_version,
                        ready: generation.ready,
                    },
                )
            })
            .collect(),
    };

    let mut ready_targets = BTreeSet::new();
    let mut catalogs = BTreeMap::new();
    let mut open = BTreeMap::new();
    // BM25 uses corpus-wide statistics before the target predicate is
    // applied. Stream every authoritative row into a bounded-cache temporary
    // SQLite index instead of retaining the history-sized corpus in memory.
    let (fts, fts_signature) = PreparedFtsIndex::build_from_storage(storage, cancellation)?;
    check_prepared_rebuild_cancellation(cancellation)?;
    let mut discovery_catalogs = Vec::new();
    for (target, generation) in &target_generations {
        check_prepared_rebuild_cancellation(cancellation)?;
        if !generation.ready {
            continue;
        }
        let published = storage
            .published_catalog_snapshot(target)?
            .ok_or_else(|| PreparedServingError::TargetNotPrepared(target.clone()))?;
        let catalog = to_normalized_catalog_v1(&published)?;
        ready_targets.insert(target.clone());
        discovery_catalogs.push(published);
        catalogs.insert(target.clone(), Arc::new(catalog));
    }
    let query_corpora = build_prepared_query_corpora(&catalogs, cancellation)?;
    for (target, catalog) in &catalogs {
        check_prepared_rebuild_cancellation(cancellation)?;
        let corpus = query_corpora
            .get(target.term().as_str())
            .ok_or_else(|| PreparedServingError::TargetNotPrepared(target.clone()))?;
        open.insert(
            target.clone(),
            project_prepared_open_overlay(
                storage,
                runtime,
                open_runtime,
                policy,
                target,
                catalog,
                corpus,
            )?,
        );
    }

    let mut discovery =
        to_catalog_discovery_response_v1(&published_discovery, &discovery_catalogs, now)?;
    discovery.targets.retain(|target| {
        discovered_targets.contains(&target.key) && allowed_targets.contains(&target.key)
    });
    discovery
        .subjects
        .retain(|subject| ready_targets.contains(&subject.target));
    discovery
        .core_code_dictionaries
        .retain(|dictionary| ready_targets.contains(&dictionary.target));

    // A concurrent publication must never produce a mixed generation. The
    // entire build is discarded if either discovery or any Catalog binding
    // changed while the independent reader was projecting it.
    if storage.published_discovery_snapshot()? != published_discovery {
        return Err(PreparedServingError::PublicationChanged);
    }
    for (target, expected) in &target_generations {
        check_prepared_rebuild_cancellation(cancellation)?;
        let current = foundation_generation(&storage.complete_target_snapshot_state(target)?);
        if &current != expected {
            return Err(PreparedServingError::PublicationChanged);
        }
    }
    if storage.course_fts_corpus_signature()? != fts_signature {
        return Err(PreparedServingError::PublicationChanged);
    }

    let catalog_json_bytes = catalogs.values().try_fold(0_u64, |total, catalog| {
        let mut writer = CountingWriter::default();
        serde_json::to_writer(&mut writer, catalog.as_ref())?;
        total
            .checked_add(writer.bytes)
            .ok_or(PreparedServingError::StoredIntegerOutOfRange)
    })?;
    let catalog_estimated_bytes = catalog_json_bytes
        .checked_mul(4)
        .ok_or(PreparedServingError::StoredIntegerOutOfRange)?;
    let (dictionaries, dictionary_estimated_bytes) =
        build_prepared_dictionaries(&catalogs, &fts, catalog_estimated_bytes, cancellation)?;
    check_prepared_rebuild_cancellation(cancellation)?;
    let query_index_estimated_bytes = prepared_query_index_estimated_bytes(&query_corpora);
    let open_index_estimated_bytes = prepared_open_index_estimated_bytes(&open);
    // Parsed Catalog structures are bounded using a deliberately conservative
    // four-times serialized-size estimate; dictionaries count owned string
    // capacities and container-node overhead directly. Compact query/Open
    // indexes count their owned vectors, hash buckets and collision storage;
    // Catalog Arcs do not double-count the already bounded Catalog objects.
    // The FTS temp database has its own explicit 16 MiB page-cache bound.
    let foundation_resident_estimated_bytes = catalog_estimated_bytes
        .checked_add(dictionary_estimated_bytes)
        .and_then(|bytes| bytes.checked_add(query_index_estimated_bytes))
        .ok_or(PreparedServingError::StoredIntegerOutOfRange)?;
    let resident_estimated_bytes = foundation_resident_estimated_bytes
        .checked_add(open_index_estimated_bytes)
        .ok_or(PreparedServingError::StoredIntegerOutOfRange)?;
    ensure_prepared_resident_budget(resident_estimated_bytes)?;
    let resident_high_watermark = PREPARED_RESIDENT_HIGH_WATERMARK
        .fetch_max(resident_estimated_bytes, Ordering::Relaxed)
        .max(resident_estimated_bytes);
    let build_elapsed = build_started.elapsed();
    tracing::info!(
        target: "bcsp_prepared_serving",
        phase = "prepared_snapshot_build",
        elapsed_us = u64::try_from(build_elapsed.as_micros()).unwrap_or(u64::MAX),
        elapsed_ms = build_elapsed.as_secs_f64() * 1_000.0,
        ready_targets = ready_targets.len(),
        catalog_json_bytes,
        dictionary_estimated_bytes,
        query_index_estimated_bytes,
        open_index_estimated_bytes,
        foundation_resident_estimated_bytes,
        resident_estimated_bytes,
        resident_budget_bytes = PREPARED_FOUNDATION_RESIDENT_BYTE_BUDGET,
        resident_high_watermark,
        fts_rows = fts_signature.row_count,
        fts_document_bytes = fts_signature.document_bytes,
        fts_index_bytes = fts.index_bytes,
        sqlite_cache_kib = PREPARED_SQLITE_CACHE_BYTE_BUDGET / 1024,
        aggregate_resident_budget_bytes = PREPARED_AGGREGATE_RESIDENT_BYTE_BUDGET,
        registry_capacity = 1_u8,
    );

    Ok(PreparedServingSnapshot {
        foundation: Arc::new(PreparedCatalogFoundation {
            discovery,
            discovered_targets,
            ready_targets,
            catalogs,
            query_corpora,
            dictionaries,
            source_generation,
            resident_estimated_bytes: foundation_resident_estimated_bytes,
            fts_index_bytes: fts.index_bytes,
            fts,
        }),
        open,
        resident_estimated_bytes,
    })
}

fn build_prepared_query_corpora(
    catalogs: &BTreeMap<TermCampusKey, Arc<NormalizedCatalogV1>>,
    cancellation: Option<&PreparedRebuildCancellation>,
) -> Result<BTreeMap<String, Arc<PreparedCatalogCorpus>>, PreparedServingError> {
    let mut by_term = BTreeMap::<String, Vec<Arc<NormalizedCatalogV1>>>::new();
    for catalog in catalogs.values() {
        by_term
            .entry(catalog.target.term().as_str().to_owned())
            .or_default()
            .push(Arc::clone(catalog));
    }
    let mut corpora = BTreeMap::new();
    for (term, catalogs) in by_term {
        check_prepared_rebuild_cancellation(cancellation)?;
        let corpus = PreparedCatalogCorpus::try_new_with_cancellation(catalogs, || {
            cancellation.is_some_and(PreparedRebuildCancellation::requested)
        })
        .map_err(|error| match error {
            CorpusError::Cancelled => PreparedServingError::RebuildCancelled,
            error => PreparedServingError::Corpus(error),
        })?;
        corpora.insert(term, Arc::new(corpus));
    }
    Ok(corpora)
}

fn prepared_query_index_estimated_bytes(
    corpora: &BTreeMap<String, Arc<PreparedCatalogCorpus>>,
) -> u64 {
    let mut bytes = u64::try_from(std::mem::size_of_val(corpora)).unwrap_or(u64::MAX);
    for (term, corpus) in corpora {
        bytes = bytes
            .saturating_add(
                u64::try_from(
                    std::mem::size_of::<String>()
                        + std::mem::size_of::<Arc<PreparedCatalogCorpus>>()
                        + 3 * std::mem::size_of::<usize>(),
                )
                .unwrap_or(u64::MAX),
            )
            .saturating_add(u64::try_from(term.capacity()).unwrap_or(u64::MAX))
            .saturating_add(corpus.index_estimated_bytes())
            .saturating_add(u64::try_from(2 * std::mem::size_of::<usize>()).unwrap_or(u64::MAX));
    }
    bytes
}

fn prepared_open_target_estimated_bytes(
    target: &TermCampusKey,
    overlay: &PreparedOpenOverlay,
) -> u64 {
    let execution = overlay
        .execution
        .as_deref()
        .map_or(0, PreparedQueryOpenOverlay::index_estimated_bytes);
    u64::try_from(
        std::mem::size_of::<TermCampusKey>()
            + std::mem::size_of::<PreparedOpenOverlay>()
            + 3 * std::mem::size_of::<usize>(),
    )
    .unwrap_or(u64::MAX)
    .saturating_add(u64::try_from(target.term().as_str().len()).unwrap_or(u64::MAX))
    .saturating_add(u64::try_from(target.campus().as_str().len()).unwrap_or(u64::MAX))
    .saturating_add(execution)
    .saturating_add(u64::try_from(2 * std::mem::size_of::<usize>()).unwrap_or(u64::MAX))
}

fn prepared_open_index_estimated_bytes(open: &BTreeMap<TermCampusKey, PreparedOpenOverlay>) -> u64 {
    open.iter().fold(
        u64::try_from(std::mem::size_of_val(open)).unwrap_or(u64::MAX),
        |bytes, (target, overlay)| {
            bytes.saturating_add(prepared_open_target_estimated_bytes(target, overlay))
        },
    )
}

fn ensure_prepared_resident_budget(estimated_bytes: u64) -> Result<(), PreparedServingError> {
    if estimated_bytes > PREPARED_FOUNDATION_RESIDENT_BYTE_BUDGET {
        Err(PreparedServingError::ResidentBudgetExceeded {
            estimated_bytes,
            budget_bytes: PREPARED_FOUNDATION_RESIDENT_BYTE_BUDGET,
        })
    } else {
        Ok(())
    }
}

fn foundation_generation(
    state: &bcsp_operational_storage::CompleteTargetSnapshotState,
) -> FoundationTargetGeneration {
    FoundationTargetGeneration {
        catalog_content_version: state.catalog_content_version,
        open_catalog_content_version: state.open_catalog_content_version,
        ready: state.ready,
    }
}

/// Reprojects only the lightweight Open overlay while sharing the immutable
/// Catalog/FTS foundation. A 30-second Open refresh must never rebuild the
/// complete Catalog corpus.
pub fn rebuild_prepared_open_overlay<C, P>(
    storage: &mut OperationalStorage,
    runtime: &SharedRuntimeContext<C, P>,
    open_runtime: &OpenRuntimeSnapshotRegistry,
    base: &Arc<PreparedServingSnapshot>,
    targets: &[TermCampusKey],
) -> Result<PreparedServingSnapshot, PreparedServingError>
where
    C: ApplicationClock,
    P: RefreshPolicyProvider,
{
    let policy = runtime.refresh_policy()?;
    let targets = targets.iter().collect::<BTreeSet<_>>();
    let replaced_open_estimated_bytes = targets.iter().fold(0_u64, |bytes, target| {
        bytes.saturating_add(base.open.get(*target).map_or(0, |overlay| {
            prepared_open_target_estimated_bytes(target, overlay)
        }))
    });
    let mut open = base.open.clone();
    for target in targets {
        let catalog = base
            .foundation
            .catalogs
            .get(target)
            .ok_or_else(|| PreparedServingError::TargetNotPrepared((*target).clone()))?;
        let corpus = base.query_corpus(target)?;
        let overlay = project_prepared_open_overlay(
            storage,
            runtime,
            open_runtime,
            policy,
            target,
            catalog,
            corpus,
        )?;
        open.insert((*target).clone(), overlay);
    }
    let open_index_estimated_bytes = prepared_open_index_estimated_bytes(&open);
    let resident_estimated_bytes = base
        .foundation
        .resident_estimated_bytes
        .checked_add(open_index_estimated_bytes)
        .ok_or(PreparedServingError::StoredIntegerOutOfRange)?;
    let replacement_peak_estimated_bytes = resident_estimated_bytes
        .checked_add(replaced_open_estimated_bytes)
        .ok_or(PreparedServingError::StoredIntegerOutOfRange)?;
    ensure_prepared_resident_budget(replacement_peak_estimated_bytes)?;
    PREPARED_RESIDENT_HIGH_WATERMARK.fetch_max(replacement_peak_estimated_bytes, Ordering::Relaxed);
    tracing::info!(
        target: "bcsp_prepared_serving",
        phase = "prepared_open_overlay_rebuild",
        open_index_estimated_bytes,
        resident_estimated_bytes,
        replacement_peak_estimated_bytes,
    );
    Ok(PreparedServingSnapshot {
        foundation: Arc::clone(&base.foundation),
        open,
        resident_estimated_bytes,
    })
}

fn project_prepared_open_overlay<C, P>(
    storage: &mut OperationalStorage,
    runtime: &SharedRuntimeContext<C, P>,
    open_runtime: &OpenRuntimeSnapshotRegistry,
    policy: RefreshPolicy,
    target: &TermCampusKey,
    catalog: &NormalizedCatalogV1,
    corpus: &PreparedCatalogCorpus,
) -> Result<PreparedOpenOverlay, PreparedServingError>
where
    C: ApplicationClock,
    P: RefreshPolicyProvider,
{
    let before = (
        storage.complete_target_snapshot_state(target)?,
        storage.open_batch_state(target)?,
    );
    if !before.0.ready || before.0.catalog_content_version != catalog.content_version.get() {
        return Err(PreparedServingError::PublicationChanged);
    }
    let scheduler = open_runtime.snapshot(target)?;
    let projected = runtime.open_status_with_policy(storage, target, &scheduler, policy)?;
    let after = (
        storage.complete_target_snapshot_state(target)?,
        storage.open_batch_state(target)?,
    );
    if after != before {
        if !after.0.ready || after.0.catalog_content_version != catalog.content_version.get() {
            return Err(PreparedServingError::PublicationChanged);
        }
        return conservative_open_overlay(catalog, corpus, after.1.as_ref());
    }
    let observation_sequence = after
        .1
        .as_ref()
        .map_or(0, |state| state.last_observation_sequence);
    let attempt_sequence = after
        .1
        .as_ref()
        .map_or(0, |state| state.last_attempt_sequence);
    let evidence = projected
        .sections
        .into_iter()
        .map(crate::runtime_core::open_evidence)
        .collect::<Vec<_>>();
    prepared_open_overlay(
        catalog,
        corpus,
        observation_sequence,
        attempt_sequence,
        evidence,
    )
}

fn conservative_open_overlay(
    catalog: &NormalizedCatalogV1,
    corpus: &PreparedCatalogCorpus,
    state: Option<&bcsp_operational_storage::OpenBatchState>,
) -> Result<PreparedOpenOverlay, PreparedServingError> {
    prepared_open_overlay(
        catalog,
        corpus,
        state.map_or(0, |state| state.last_observation_sequence),
        state.map_or(0, |state| state.last_attempt_sequence),
        catalog
            .sections
            .iter()
            .map(|section| OpenEvidence {
                section_key: section.key.clone(),
                evidence: LiveOpenEvidenceV1 {
                    state: LiveOpenStateV1::Unknown,
                    observed_at: None,
                    fresh_until: None,
                    uncertainty: Some(MatchReasonCode::SourceUnavailable),
                },
            })
            .collect(),
    )
}

fn prepared_open_overlay(
    catalog: &NormalizedCatalogV1,
    corpus: &PreparedCatalogCorpus,
    observation_sequence: u64,
    attempt_sequence: u64,
    evidence: Vec<OpenEvidence>,
) -> Result<PreparedOpenOverlay, PreparedServingError> {
    let execution = PreparedQueryOpenOverlay::try_new(corpus, &catalog.target, evidence)?;
    Ok(PreparedOpenOverlay {
        catalog_content_version: catalog.content_version,
        observation_sequence,
        attempt_sequence,
        execution: Some(Arc::new(execution)),
    })
}

struct PreparedFtsIndex {
    connection: Mutex<Connection>,
    index_bytes: u64,
}

impl PreparedFtsIndex {
    fn build_from_storage(
        storage: &OperationalStorage,
        cancellation: Option<&PreparedRebuildCancellation>,
    ) -> Result<(Self, CourseFtsCorpusSignature), PreparedServingError> {
        check_prepared_rebuild_cancellation(cancellation)?;
        let mut connection = prepared_fts_connection()?;
        let transaction = connection.transaction()?;
        let mut cancellation_observed = false;
        let signature_result = {
            let mut insert = transaction.prepare(
                "INSERT INTO prepared_course_fts
                    (target_id, course_string, fingerprint, content_version, document)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            storage.visit_all_course_fts_documents(|document| {
                if cancellation.is_some_and(PreparedRebuildCancellation::requested) {
                    cancellation_observed = true;
                    return Err(StorageError::InjectedFault(
                        "prepared serving rebuild cancelled",
                    ));
                }
                insert.execute(params![
                    target_id(&document.key.group().target()),
                    document.key.group().course_string().as_str(),
                    document.key.fingerprint().as_str(),
                    i64::try_from(document.content_version)
                        .map_err(|_| StorageError::StoredIntegerOutOfRange)?,
                    &document.document,
                ])?;
                Ok(())
            })
        };
        if cancellation_observed {
            return Err(PreparedServingError::RebuildCancelled);
        }
        let signature = signature_result?;
        check_prepared_rebuild_cancellation(cancellation)?;
        transaction.commit()?;
        let index_bytes = prepared_fts_index_bytes(&connection)?;
        Ok((
            Self {
                connection: Mutex::new(connection),
                index_bytes,
            },
            signature,
        ))
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn build(documents: &[PublishedCourseFtsDocument]) -> Result<Self, PreparedServingError> {
        let mut connection = prepared_fts_connection()?;
        let transaction = connection.transaction()?;
        {
            let mut insert = transaction.prepare(
                "INSERT INTO prepared_course_fts
                    (target_id, course_string, fingerprint, content_version, document)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for document in documents {
                insert.execute(params![
                    target_id(&document.key.group().target()),
                    document.key.group().course_string().as_str(),
                    document.key.fingerprint().as_str(),
                    i64::try_from(document.content_version)
                        .map_err(|_| PreparedServingError::StoredIntegerOutOfRange)?,
                    &document.document,
                ])?;
            }
        }
        transaction.commit()?;
        let index_bytes = prepared_fts_index_bytes(&connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
            index_bytes,
        })
    }

    fn search(
        &self,
        target: &TermCampusKey,
        content_version: u64,
        tokens: &CourseTextSearchTokens,
    ) -> Result<Vec<CourseVariantSearchHit>, PreparedServingError> {
        if content_version == 0 {
            return Err(PreparedServingError::InvalidFtsInput {
                field: "content_version",
            });
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| PreparedServingError::PreparedFtsUnavailable)?;
        let query = fts5_literal_and_query(tokens);
        let mut statement = connection.prepare(
            "SELECT course_string, fingerprint
               FROM prepared_course_fts
              WHERE prepared_course_fts MATCH ?1
                AND target_id = ?2
                AND content_version = ?3
              ORDER BY bm25(prepared_course_fts),
                       course_string COLLATE BINARY,
                       fingerprint COLLATE BINARY",
        )?;
        let rows = statement.query_map(
            params![
                query,
                target_id(target),
                i64::try_from(content_version)
                    .map_err(|_| PreparedServingError::StoredIntegerOutOfRange)?
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )?;
        rows.enumerate()
            .map(|(rank, row)| {
                let (course_string, fingerprint) = row?;
                let group = bcsp_contracts::CourseGroupKey::try_new(
                    target.term().as_str(),
                    target.campus().as_str(),
                    &course_string,
                )?;
                Ok(CourseVariantSearchHit {
                    key: bcsp_contracts::CourseVariantKey::try_new(group, &fingerprint)?,
                    fts_rank: u32::try_from(rank)
                        .map_err(|_| PreparedServingError::StoredIntegerOutOfRange)?,
                })
            })
            .collect()
    }

    fn terms(
        &self,
        target: &TermCampusKey,
        content_version: u64,
        query: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, PreparedServingError> {
        if content_version == 0 || limit == 0 {
            return Err(PreparedServingError::InvalidFtsInput {
                field: "filter_options",
            });
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| PreparedServingError::PreparedFtsUnavailable)?;
        let query = query.unwrap_or("");
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut statement = connection.prepare(
            "SELECT DISTINCT vocab.term
               FROM prepared_course_fts_vocab AS vocab
               JOIN prepared_course_fts AS f ON f.rowid = vocab.doc
              WHERE f.target_id = ?1
                AND f.content_version = ?2
                AND (?3 = '' OR instr(lower(vocab.term), lower(?3)) > 0)
              ORDER BY vocab.term COLLATE NOCASE, vocab.term COLLATE BINARY
              LIMIT ?4",
        )?;
        let rows = statement.query_map(
            params![
                target_id(target),
                i64::try_from(content_version)
                    .map_err(|_| PreparedServingError::StoredIntegerOutOfRange)?,
                query,
                limit
            ],
            |row| row.get::<_, String>(0),
        )?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(PreparedServingError::Sqlite)
    }

    /// Streams the complete target/version vocabulary directly into its final
    /// ordered set. The foundation build must not allocate an unbounded
    /// intermediate `Vec` before the resident-budget gate can account for the
    /// owned dictionary strings.
    fn term_set(
        &self,
        target: &TermCampusKey,
        content_version: u64,
    ) -> Result<BTreeSet<String>, PreparedServingError> {
        if content_version == 0 {
            return Err(PreparedServingError::InvalidFtsInput {
                field: "filter_options",
            });
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| PreparedServingError::PreparedFtsUnavailable)?;
        let mut statement = connection.prepare(
            "SELECT DISTINCT vocab.term
               FROM prepared_course_fts_vocab AS vocab
               JOIN prepared_course_fts AS f ON f.rowid = vocab.doc
              WHERE f.target_id = ?1
                AND f.content_version = ?2
              ORDER BY vocab.term COLLATE NOCASE, vocab.term COLLATE BINARY",
        )?;
        let rows = statement.query_map(
            params![
                target_id(target),
                i64::try_from(content_version)
                    .map_err(|_| PreparedServingError::StoredIntegerOutOfRange)?
            ],
            |row| row.get::<_, String>(0),
        )?;
        let mut values = BTreeSet::new();
        for row in rows {
            values.insert(row?);
        }
        Ok(values)
    }

    fn term_exists(
        &self,
        target: &TermCampusKey,
        content_version: u64,
        term: &str,
    ) -> Result<bool, PreparedServingError> {
        if content_version == 0 || term.is_empty() {
            return Err(PreparedServingError::InvalidFtsInput {
                field: "dynamic_filter_validation",
            });
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| PreparedServingError::PreparedFtsUnavailable)?;
        connection
            .query_row(
                "SELECT 1
                   FROM prepared_course_fts_vocab AS vocab
                   JOIN prepared_course_fts AS f ON f.rowid = vocab.doc
                  WHERE f.target_id = ?1
                    AND f.content_version = ?2
                    AND lower(vocab.term) = lower(?3)
                  LIMIT 1",
                params![
                    target_id(target),
                    i64::try_from(content_version)
                        .map_err(|_| PreparedServingError::StoredIntegerOutOfRange)?,
                    term
                ],
                |_| Ok(()),
            )
            .optional()
            .map(|found| found.is_some())
            .map_err(PreparedServingError::Sqlite)
    }
}

fn prepared_fts_connection() -> Result<Connection, PreparedServingError> {
    // An empty SQLite filename creates a process-owned temporary file that is
    // removed on close. The explicit negative cache_size is a 16 MiB page-cache
    // ceiling; mmap is disabled so historical corpus growth consumes disk, not
    // unbounded process memory.
    let connection = Connection::open("")?;
    connection.execute_batch(
        "PRAGMA cache_size = -16384;
         PRAGMA mmap_size = 0;
         PRAGMA journal_mode = OFF;
         PRAGMA synchronous = OFF;
         CREATE VIRTUAL TABLE prepared_course_fts USING fts5(
             target_id UNINDEXED,
             course_string UNINDEXED,
             fingerprint UNINDEXED,
             content_version UNINDEXED,
             document,
             tokenize = 'unicode61 remove_diacritics 2'
         );
         CREATE VIRTUAL TABLE prepared_course_fts_vocab
         USING fts5vocab(prepared_course_fts, instance);",
    )?;
    Ok(connection)
}

fn prepared_fts_index_bytes(connection: &Connection) -> Result<u64, PreparedServingError> {
    let page_count = connection.query_row("PRAGMA page_count", [], |row| row.get::<_, i64>(0))?;
    let page_size = connection.query_row("PRAGMA page_size", [], |row| row.get::<_, i64>(0))?;
    let page_count =
        u64::try_from(page_count).map_err(|_| PreparedServingError::StoredIntegerOutOfRange)?;
    let page_size =
        u64::try_from(page_size).map_err(|_| PreparedServingError::StoredIntegerOutOfRange)?;
    page_count
        .checked_mul(page_size)
        .ok_or(PreparedServingError::StoredIntegerOutOfRange)
}

#[derive(Default)]
struct CountingWriter {
    bytes: u64,
}

impl Write for CountingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(u64::try_from(buffer.len()).unwrap_or(u64::MAX))
            .ok_or_else(|| io::Error::other("prepared Catalog byte count overflow"))?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn target_id(target: &TermCampusKey) -> String {
    format!("{}/{}", target.term(), target.campus())
}

fn fts5_literal_and_query(tokens: &CourseTextSearchTokens) -> String {
    tokens
        .as_slice()
        .iter()
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

#[derive(Debug, Error)]
pub enum PreparedServingError {
    #[error("prepared serving snapshot is unavailable")]
    SnapshotUnavailable,
    #[error("prepared serving snapshot is rebuilding after publication")]
    SnapshotRebuilding,
    #[error("prepared serving registry is unavailable")]
    RegistryUnavailable,
    #[error("another prepared publication barrier is already active")]
    PublicationBarrierBusy,
    #[error("prepared publication barrier ticket is unavailable")]
    PublicationBarrierUnavailable,
    #[error("prepared publication worker is not accepting new publications")]
    PublicationWorkerUnavailable,
    #[error("prepared serving storage access is unavailable")]
    StorageAccessUnavailable,
    #[error("prepared serving build was superseded by a newer generation")]
    SupersededBuild,
    #[error("prepared serving rebuild was cancelled during shutdown")]
    RebuildCancelled,
    #[error("prepared FTS index is unavailable")]
    PreparedFtsUnavailable,
    #[error("prepared FTS input is invalid for {field}")]
    InvalidFtsInput { field: &'static str },
    #[error("prepared target is unavailable: {0:?}")]
    TargetNotPrepared(TermCampusKey),
    #[error("publication changed while a prepared snapshot was built")]
    PublicationChanged,
    #[error("prepared integer is outside the supported range")]
    StoredIntegerOutOfRange,
    #[error(
        "prepared foundation resident estimate {estimated_bytes} exceeds budget {budget_bytes}"
    )]
    ResidentBudgetExceeded {
        estimated_bytes: u64,
        budget_bytes: u64,
    },
    #[error("prepared target dictionary is unavailable")]
    PreparedDictionaryUnavailable,
    #[error("prepared SQLite operation failed")]
    Sqlite(#[from] rusqlite::Error),
    #[error("prepared storage read failed")]
    Storage(#[from] StorageError),
    #[error("prepared Catalog projection failed")]
    Projection(#[from] ProjectionError),
    #[error("prepared Catalog metric serialization failed")]
    Serialization(#[from] serde_json::Error),
    #[error("prepared runtime projection failed")]
    Runtime(#[from] SharedRuntimeError),
    #[error("prepared Open runtime registry failed")]
    OpenRuntime(#[from] OpenRuntimeSnapshotRegistryError),
    #[error("prepared term window failed")]
    TermWindow(#[from] RutgersTermWindowError),
    #[error("prepared text plan failed")]
    TextPlan(#[from] TextHitPlanError),
    #[error("prepared Catalog corpus failed")]
    Corpus(#[from] CorpusError),
    #[error("prepared query execution state failed")]
    Query(#[from] QueryError),
    #[error("prepared identity failed")]
    Identity(#[from] bcsp_contracts::IdentityError),
}

#[cfg(test)]
mod tests {
    use std::time::Duration as StdDuration;

    use super::*;
    use bcsp_catalog::{normalize_target, to_catalog_refresh_command};
    use bcsp_contracts::{
        CATALOG_CONTRACT_VERSION, CatalogDiscoveryAvailability, CatalogDiscoveryStatusV1,
        CourseGroupKey, CourseQueryRequestV1, CourseSortV1, CourseVariantKey, FilterRequestV1,
        FilterValuesInputV1, LiveOpenStateV1, MatchReasonCode, NormalizedFilterValuesV1,
        PageRequestV1, SectionKey, TermId, TraceId,
    };
    use bcsp_open::{GeneralOpenInterval, OpenCounterAudience};
    use bcsp_operational_storage::{
        BeginOpenPullAttemptCommand, EmptySnapshotDecision, FinishOpenPullSuccessCommand,
        OpenCacheStatus, OpenHttpAuditMetadata, OpenRequestLane, PublishOutcome,
    };
    use bcsp_rutgers_client::{SourceProvenance, decode_catalog_payload};
    use serde_json::json;
    use time::{Duration, OffsetDateTime};

    fn target(campus: &str) -> TermCampusKey {
        TermCampusKey::try_new("92026", campus).expect("target")
    }

    fn document(
        target: &TermCampusKey,
        course: &str,
        fingerprint: char,
        text: impl Into<String>,
    ) -> PublishedCourseFtsDocument {
        let group =
            CourseGroupKey::try_new(target.term().as_str(), target.campus().as_str(), course)
                .expect("group");
        PublishedCourseFtsDocument {
            key: CourseVariantKey::try_new(
                group,
                &format!("v1:{}", fingerprint.to_string().repeat(64)),
            )
            .expect("variant"),
            content_version: 1,
            document: text.into(),
        }
    }

    fn evidence(target: &TermCampusKey, uncertainty: Option<MatchReasonCode>) -> OpenEvidence {
        let now = OffsetDateTime::from_unix_timestamp(1_784_236_800).expect("fixed time");
        OpenEvidence {
            section_key: SectionKey::try_new(
                target.term().as_str(),
                target.campus().as_str(),
                "10001",
            )
            .expect("section"),
            evidence: LiveOpenEvidenceV1 {
                state: LiveOpenStateV1::Open,
                observed_at: Some(now),
                fresh_until: Some(now + Duration::minutes(5)),
                uncertainty,
            },
        }
    }

    fn dummy_snapshot(
        target: &TermCampusKey,
        observation_sequence: u64,
        attempt_sequence: u64,
        _uncertainty: Option<MatchReasonCode>,
    ) -> PreparedServingSnapshot {
        let now = OffsetDateTime::from_unix_timestamp(1_784_236_800).expect("fixed time");
        let fts = PreparedFtsIndex::build(&[]).expect("empty FTS");
        PreparedServingSnapshot {
            foundation: Arc::new(PreparedCatalogFoundation {
                discovery: CatalogDiscoveryResponseV1 {
                    contract_version: CATALOG_CONTRACT_VERSION,
                    observed_at: now,
                    status: CatalogDiscoveryStatusV1 {
                        availability: CatalogDiscoveryAvailability::UnavailableNoFirstSuccess,
                        latest_attempt: None,
                        last_success: None,
                        is_stale: false,
                        error: None,
                    },
                    sources: Vec::new(),
                    targets: Vec::new(),
                    subjects: Vec::new(),
                    core_code_dictionaries: Vec::new(),
                },
                discovered_targets: BTreeSet::new(),
                ready_targets: BTreeSet::new(),
                catalogs: BTreeMap::new(),
                query_corpora: BTreeMap::new(),
                dictionaries: BTreeMap::new(),
                source_generation: FoundationSourceGeneration {
                    scope: RutgersTermWindowScope::Local,
                    discovery_content_version: 0,
                    discovery_semantic_sha256: None,
                    targets: BTreeMap::new(),
                },
                resident_estimated_bytes: 0,
                fts_index_bytes: fts.index_bytes,
                fts,
            }),
            open: BTreeMap::from([(
                target.clone(),
                PreparedOpenOverlay {
                    catalog_content_version: CatalogContentVersion::try_from(1).expect("version"),
                    observation_sequence,
                    attempt_sequence,
                    execution: None,
                },
            )]),
            resident_estimated_bytes: 0,
        }
    }

    fn dummy_snapshot_many(
        entries: &[(TermCampusKey, u64, u64, Option<MatchReasonCode>)],
    ) -> PreparedServingSnapshot {
        let (first, observation, attempt, uncertainty) =
            entries.first().expect("at least one target");
        let mut snapshot = dummy_snapshot(first, *observation, *attempt, *uncertainty);
        snapshot.open = entries
            .iter()
            .map(
                |(target, observation_sequence, attempt_sequence, _uncertainty)| {
                    (
                        target.clone(),
                        PreparedOpenOverlay {
                            catalog_content_version: CatalogContentVersion::try_from(1)
                                .expect("version"),
                            observation_sequence: *observation_sequence,
                            attempt_sequence: *attempt_sequence,
                            execution: None,
                        },
                    )
                },
            )
            .collect();
        snapshot
    }

    fn published_registry(
        target: &TermCampusKey,
    ) -> (Arc<PreparedServingRegistry>, Arc<PreparedServingSnapshot>) {
        let registry = Arc::new(PreparedServingRegistry::default());
        let snapshot = registry
            .publish_if_unchanged(None, dummy_snapshot(target, 7, 8, None))
            .expect("publish fixture");
        (registry, snapshot)
    }

    fn trace(suffix: u8) -> TraceId {
        format!("00000000-0000-4000-8000-{suffix:012x}")
            .parse()
            .expect("synthetic trace ID")
    }

    fn publish_worker_catalog(storage: &mut OperationalStorage, target: &TermCampusKey) {
        let body = serde_json::to_vec(&[json!({
            "campusCode": target.campus().as_str(),
            "courseString": "01:198:111",
            "subject": "198",
            "courseNumber": "111",
            "title": "Prepared worker interleaving",
            "level": "U",
            "preReqNotes": "",
            "sections": [{
                "campusCode": target.campus().as_str(),
                "index": "10001",
                "number": "01",
                "sectionCourseType": "O",
                "openStatus": true,
                "meetingTimes": [{"meetingModeCode": "92", "campusLocation": "O"}],
                "instructors": [{"name": "Pat Smith"}]
            }]
        })])
        .expect("worker Catalog body");
        let normalized = normalize_target(
            target.clone(),
            decode_catalog_payload(&body).expect("decode worker Catalog"),
            SourceProvenance::from_body("SYNTHETIC_PREPARED_WORKER", "2026-07-18T00:00:00Z", &body),
        )
        .expect("normalize worker Catalog");
        let outcome = storage
            .apply_catalog_refresh(
                to_catalog_refresh_command(
                    &normalized,
                    trace(60),
                    "2026-07-18T00:00:00Z",
                    "2026-07-18T00:00:01Z",
                )
                .expect("map worker Catalog"),
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
                bcsp_catalog::CATALOG_DERIVATION_VERSION,
            )
            .expect("publish worker Catalog");
        assert!(matches!(outcome, PublishOutcome::AppliedChanged { .. }));
    }

    fn publish_worker_open(
        storage: &mut OperationalStorage,
        target: &TermCampusKey,
        suffix: u8,
        open: bool,
    ) {
        storage
            .begin_open_pull_attempt(&BeginOpenPullAttemptCommand {
                attempt_id: trace(suffix),
                run_id: trace(suffix.saturating_add(1)),
                target: target.clone(),
                captured_catalog_content_version: 1,
                rutgers_day: "2026-07-18".to_owned(),
                started_at: "2026-07-18T00:00:02Z".to_owned(),
                lane: OpenRequestLane::ActiveWatch,
                requested_interval_seconds: Some(30),
                effective_interval_seconds: Some(10),
                schedule_lag_ms: Some(0),
            })
            .expect("begin worker Open attempt");
        let section =
            SectionKey::try_new(target.term().as_str(), target.campus().as_str(), "10001")
                .expect("worker Section");
        storage
            .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
                attempt_id: trace(suffix),
                completed_at: "2026-07-18T00:00:03Z".to_owned(),
                open_sections: open.then_some(section).into_iter().collect(),
                source_value_count: u64::from(open),
                watched_sections: Vec::new(),
                http: OpenHttpAuditMetadata {
                    http_status: Some(200),
                    cache_status: Some(OpenCacheStatus::Miss),
                    decoded_bytes: Some(2),
                    decoded_body_sha256: Some(format!("{:064x}", suffix)),
                    content_type: Some("application/json".to_owned()),
                    etag: None,
                    cache_control: None,
                    date: None,
                    age_seconds: None,
                    last_modified: None,
                    retry_after: None,
                    retry_after_seconds: None,
                },
            })
            .expect("finish worker Open attempt");
    }

    #[test]
    fn core_code_dictionary_and_lookup_meet_in_ascii_uppercase() {
        let target = target("NB");
        let mut storage = OperationalStorage::open_in_memory().expect("in-memory storage");
        let body = serde_json::to_vec(&[json!({
            "campusCode": target.campus().as_str(),
            "courseString": "01:198:111",
            "subject": "198",
            "courseNumber": "111",
            "title": "Mixed-case core code",
            "level": "U",
            "preReqNotes": "",
            "coreCodes": [{
                "coreCode": "AHo",
                "coreCodeDescription": "Arts and Humanities (o)"
            }],
            "sections": [{
                "campusCode": target.campus().as_str(),
                "index": "10001",
                "number": "01",
                "sectionCourseType": "O",
                "openStatus": true,
                "meetingTimes": [{"meetingModeCode": "92", "campusLocation": "O"}],
                "instructors": [{"name": "Pat Smith"}]
            }]
        })])
        .expect("core Catalog body");
        let normalized = normalize_target(
            target.clone(),
            decode_catalog_payload(&body).expect("decode core Catalog"),
            SourceProvenance::from_body("SYNTHETIC_CORE_CASE", "2026-07-18T00:00:00Z", &body),
        )
        .expect("normalize core Catalog");
        storage
            .apply_catalog_refresh(
                to_catalog_refresh_command(
                    &normalized,
                    trace(61),
                    "2026-07-18T00:00:00Z",
                    "2026-07-18T00:00:01Z",
                )
                .expect("map core Catalog"),
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
                bcsp_catalog::CATALOG_DERIVATION_VERSION,
            )
            .expect("publish core Catalog");
        let published = storage
            .published_catalog_snapshot(&target)
            .expect("read published Catalog")
            .expect("published Catalog");
        let catalog = to_normalized_catalog_v1(&published).expect("normalized Catalog");
        assert_eq!(
            crate::query_service::known_value(&catalog.course_variants[0].core_codes),
            Some(&vec!["AHo".to_owned()]),
            "the stored variant keeps the feed's spelling"
        );

        let fts = PreparedFtsIndex::build(&[]).expect("empty FTS");
        let catalogs = BTreeMap::from([(target.clone(), Arc::new(catalog))]);
        let (dictionaries, _) =
            build_prepared_dictionaries(&catalogs, &fts, 0, None).expect("dictionaries");
        let core = &dictionaries[&target].dynamic_values[&FilterFieldId::CourseCoreCode];
        assert_eq!(
            core.iter().cloned().collect::<Vec<_>>(),
            vec!["AHO".to_owned()],
            "the dictionary holds the canonical uppercase form"
        );
        for requested in ["AHO", "AHo", "aho"] {
            assert!(
                core.contains(&normalize_dynamic_value(
                    FilterFieldId::CourseCoreCode,
                    requested
                )),
                "{requested} must validate against the AHo dictionary"
            );
        }
        assert!(!core.contains(&normalize_dynamic_value(
            FilterFieldId::CourseCoreCode,
            "AHP"
        )));
    }

    #[test]
    fn request_now_materializes_fresh_until_at_the_exact_boundary() {
        let target = target("NB");
        let value = evidence(&target, None);
        let fresh_until = value.evidence.fresh_until.expect("fresh until");
        assert_eq!(
            materialize_open_evidence_at(value.clone(), fresh_until)
                .evidence
                .uncertainty,
            None
        );
        assert_eq!(
            materialize_open_evidence_at(value, fresh_until + Duration::nanoseconds(1))
                .evidence
                .uncertainty,
            Some(MatchReasonCode::SourceUnavailable)
        );
    }

    #[test]
    fn full_corpus_bm25_preserves_global_statistics_and_binary_ties() {
        let selected = target("NB");
        let other = target("CM");
        let selected_docs = vec![
            document(&selected, "01:198:111", 'a', "needle"),
            document(&selected, "01:198:112", 'b', "needle needle b"),
        ];
        let selected_only = PreparedFtsIndex::build(&selected_docs).expect("selected FTS");
        let tokens = CourseTextSearchTokens::try_new(&["needle".to_owned()]).expect("tokens");
        let selected_order = selected_only
            .search(&selected, 1, &tokens)
            .expect("selected search")
            .into_iter()
            .map(|hit| hit.key.group().course_string().as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(selected_order, ["01:198:111", "01:198:112"]);

        let mut full_docs = selected_docs;
        for ordinal in 0..20_u16 {
            full_docs.push(document(
                &other,
                &format!("50:999:{ordinal:03}"),
                'c',
                "x ".repeat(200),
            ));
        }
        let full = PreparedFtsIndex::build(&full_docs).expect("global FTS");
        let full_order = full
            .search(&selected, 1, &tokens)
            .expect("global search")
            .into_iter()
            .map(|hit| hit.key.group().course_string().as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(full_order, ["01:198:112", "01:198:111"]);
    }

    #[test]
    fn full_and_open_barriers_preserve_old_arc_and_close_new_admission() {
        let target = target("NB");
        let (registry, old) = published_registry(&target);
        let (sender, _receiver) = mpsc::unbounded_channel();
        let demand = PreparedServingRebuildDemand {
            sender,
            registry: Arc::clone(&registry),
            lifecycle: PublicationLifecycle::default(),
        };

        let full = demand.begin_publication().expect("full barrier");
        assert!(matches!(
            registry.snapshot(),
            Err(PreparedServingError::SnapshotRebuilding)
        ));
        // The zero-wait entry points must never drift apart.
        assert!(matches!(
            registry.snapshot_within(StdDuration::ZERO),
            Err(PreparedServingError::SnapshotRebuilding)
        ));
        drop(full);
        assert!(Arc::ptr_eq(
            registry.snapshot().expect("restored").generation(),
            &old
        ));
        assert!(Arc::ptr_eq(
            registry
                .snapshot_within(StdDuration::ZERO)
                .expect("restored via zero wait")
                .generation(),
            &old
        ));

        let open = demand
            .begin_open_publication(target.clone())
            .expect("open barrier");
        let conservative = registry.snapshot().expect("conservative generation");
        assert_eq!(
            conservative.forced_open_unavailable(),
            &BTreeSet::from([target.clone()])
        );
        assert!(Arc::ptr_eq(conservative.generation(), &old));
        let conservative_zero_wait = registry
            .snapshot_within(StdDuration::ZERO)
            .expect("conservative generation via zero wait");
        assert_eq!(
            conservative_zero_wait.forced_open_unavailable(),
            conservative.forced_open_unavailable()
        );
        assert!(Arc::ptr_eq(conservative_zero_wait.generation(), &old));
        drop(open);
        assert!(Arc::ptr_eq(
            registry.snapshot().expect("rolled back").generation(),
            &old
        ));
    }

    #[test]
    fn publication_ticket_sets_restore_nested_full_and_open_in_both_orders() {
        let target = target("NB");
        let (registry, old) = published_registry(&target);
        let full_epoch = registry.mark_full_dirty().expect("full epoch");
        let open_admission = registry.mark_open_dirty(&target).expect("open epoch");
        registry.rollback_full_dirty(full_epoch);
        let binding = registry.snapshot().expect("Open remains conservative");
        assert!(Arc::ptr_eq(binding.generation(), &old));
        assert_eq!(
            binding.forced_open_unavailable(),
            &BTreeSet::from([target.clone()])
        );
        registry.rollback_open_dirty(&target, open_admission);
        assert!(Arc::ptr_eq(
            registry.snapshot().expect("both rolled back").generation(),
            &old
        ));

        let open_admission = registry.mark_open_dirty(&target).expect("open epoch");
        let full_epoch = registry.mark_full_dirty().expect("nested full epoch");
        registry.rollback_open_dirty(&target, open_admission);
        assert!(matches!(
            registry.snapshot(),
            Err(PreparedServingError::SnapshotRebuilding)
        ));
        registry.rollback_full_dirty(full_epoch);
        assert!(Arc::ptr_eq(
            registry.snapshot().expect("reverse rollback").generation(),
            &old
        ));

        let first = registry.mark_full_dirty().expect("first full");
        let second = registry.mark_full_dirty().expect("second full");
        registry.rollback_full_dirty(second);
        assert!(matches!(
            registry.snapshot(),
            Err(PreparedServingError::SnapshotRebuilding)
        ));
        registry.rollback_full_dirty(first);
        assert!(Arc::ptr_eq(
            registry
                .snapshot()
                .expect("full stack rollback")
                .generation(),
            &old
        ));
    }

    #[test]
    fn committed_open_ticket_blocks_new_binding_until_the_exact_overlay_is_published() {
        let target = target("NB");
        let (registry, old) = published_registry(&target);
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let demand = PreparedServingRebuildDemand {
            sender,
            registry: Arc::clone(&registry),
            lifecycle: PublicationLifecycle::default(),
        };

        let barrier = demand
            .begin_open_publication(target.clone())
            .expect("Open publication barrier");
        let precommit = registry.snapshot().expect("precommit conservative Arc");
        assert!(Arc::ptr_eq(precommit.generation(), &old));
        assert_eq!(
            precommit.forced_open_unavailable(),
            &BTreeSet::from([target.clone()])
        );
        barrier.commit();
        assert!(matches!(
            registry.snapshot(),
            Err(PreparedServingError::SnapshotRebuilding)
        ));
        assert!(matches!(
            registry.snapshot_within(StdDuration::ZERO),
            Err(PreparedServingError::SnapshotRebuilding)
        ));
        assert_eq!(
            registry.snapshot_wait_timeouts(),
            0,
            "a zero wait is a miss, not a timed-out wait"
        );
        let epoch = match receiver.try_recv().expect("committed rebuild demand") {
            PreparedRebuildRequest::Open(request_target, epoch) => {
                assert_eq!(request_target, target);
                epoch
            }
            other => panic!("unexpected request: {other:?}"),
        };

        let published = registry
            .publish_if_unchanged(Some(&old), dummy_snapshot(&target, 8, 9, None))
            .expect("publish rebuilt Open overlay");
        registry.clear_open_tickets(&target, &BTreeSet::from([epoch]));
        let admitted = registry.snapshot().expect("rebuilt snapshot admitted");
        assert!(Arc::ptr_eq(admitted.generation(), &published));
        assert!(Arc::ptr_eq(
            registry
                .snapshot_within(StdDuration::ZERO)
                .expect("rebuilt snapshot admitted via zero wait")
                .generation(),
            &published
        ));
        assert_eq!(
            admitted
                .open_vector(std::slice::from_ref(&target))
                .expect("vector")[0]
                .observation_sequence,
            8
        );
        assert_eq!(
            old.open_vector(std::slice::from_ref(&target))
                .expect("old vector")[0]
                .observation_sequence,
            7
        );
    }

    fn dead_channel_demand(
        registry: &Arc<PreparedServingRegistry>,
    ) -> PreparedServingRebuildDemand {
        let (sender, _receiver) = mpsc::unbounded_channel();
        PreparedServingRebuildDemand {
            sender,
            registry: Arc::clone(registry),
            lifecycle: PublicationLifecycle::default(),
        }
    }

    #[test]
    fn snapshot_within_returns_immediately_when_admission_is_open() {
        let target = target("NB");
        let (registry, old) = published_registry(&target);
        let started = Instant::now();
        let binding = registry
            .snapshot_within(StdDuration::from_secs(1))
            .expect("open admission binds without waiting");
        assert!(Arc::ptr_eq(binding.generation(), &old));
        assert!(Arc::ptr_eq(
            binding.generation(),
            registry.snapshot().expect("zero-wait twin").generation()
        ));
        assert!(binding.forced_open_unavailable().is_empty());
        assert!(
            started.elapsed() < StdDuration::from_millis(200),
            "an open registry must not park the caller"
        );
        assert_eq!(registry.snapshot_wait_timeouts(), 0);
    }

    #[test]
    fn snapshot_within_waits_for_committed_open_ticket_to_clear() {
        let target = target("NB");
        let (registry, old) = published_registry(&target);
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let demand = PreparedServingRebuildDemand {
            sender,
            registry: Arc::clone(&registry),
            lifecycle: PublicationLifecycle::default(),
        };
        demand
            .begin_open_publication(target.clone())
            .expect("Open publication barrier")
            .commit();
        let epoch = match receiver.try_recv().expect("committed rebuild demand") {
            PreparedRebuildRequest::Open(_, epoch) => epoch,
            other => panic!("unexpected request: {other:?}"),
        };
        assert!(matches!(
            registry.snapshot(),
            Err(PreparedServingError::SnapshotRebuilding)
        ));

        let clearing_registry = Arc::clone(&registry);
        let clearing_target = target.clone();
        let clearing_old = Arc::clone(&old);
        let worker = std::thread::spawn(move || {
            std::thread::sleep(StdDuration::from_millis(50));
            let published = clearing_registry
                .publish_if_unchanged(
                    Some(&clearing_old),
                    dummy_snapshot(&clearing_target, 8, 9, None),
                )
                .expect("publish rebuilt Open overlay");
            clearing_registry.clear_open_tickets(&clearing_target, &BTreeSet::from([epoch]));
            published
        });

        let started = Instant::now();
        let admitted = registry
            .snapshot_within(StdDuration::from_secs(2))
            .expect("the wait ends when the worker clears the ticket");
        let waited = started.elapsed();
        let published = worker.join().expect("clearing thread");
        assert!(
            Arc::ptr_eq(admitted.generation(), &published),
            "the waiter binds the NEW generation"
        );
        assert!(!Arc::ptr_eq(admitted.generation(), &old));
        assert_eq!(
            admitted
                .open_vector(std::slice::from_ref(&target))
                .expect("vector")[0]
                .observation_sequence,
            8
        );
        assert!(
            waited >= StdDuration::from_millis(40) && waited < StdDuration::from_secs(2),
            "waited {waited:?}"
        );
        assert_eq!(registry.snapshot_wait_timeouts(), 0);
    }

    #[test]
    fn snapshot_within_times_out_with_snapshot_rebuilding_when_ticket_persists() {
        let target = target("NB");
        let (registry, _old) = published_registry(&target);
        let demand = dead_channel_demand(&registry);
        demand
            .begin_open_publication(target.clone())
            .expect("Open publication barrier")
            .commit();

        let started = Instant::now();
        let result = registry.snapshot_within(StdDuration::from_millis(50));
        let waited = started.elapsed();
        assert!(matches!(
            result,
            Err(PreparedServingError::SnapshotRebuilding)
        ));
        assert!(
            waited >= StdDuration::from_millis(50) && waited < StdDuration::from_millis(500),
            "waited {waited:?}"
        );
        assert_eq!(registry.snapshot_wait_timeouts(), 1);
        assert!(matches!(
            registry.snapshot(),
            Err(PreparedServingError::SnapshotRebuilding)
        ));
        assert_eq!(
            registry.snapshot_wait_timeouts(),
            1,
            "a zero-wait miss is not a timed-out wait"
        );
    }

    #[test]
    fn snapshot_within_waits_through_full_ticket_and_rollback_wakes_waiters() {
        let target = target("NB");
        let (registry, old) = published_registry(&target);
        let full_epoch = registry.mark_full_dirty().expect("Precommit full ticket");
        assert!(matches!(
            registry.snapshot(),
            Err(PreparedServingError::SnapshotRebuilding)
        ));

        let rollback_registry = Arc::clone(&registry);
        let rollback = std::thread::spawn(move || {
            std::thread::sleep(StdDuration::from_millis(50));
            rollback_registry.rollback_full_dirty(full_epoch);
        });
        let started = Instant::now();
        let restored = registry
            .snapshot_within(StdDuration::from_secs(2))
            .expect("rollback wakes the waiter");
        let waited = started.elapsed();
        rollback.join().expect("rollback thread");
        assert!(Arc::ptr_eq(restored.generation(), &old));
        assert!(
            waited >= StdDuration::from_millis(40) && waited < StdDuration::from_secs(2),
            "waited {waited:?}"
        );

        // A Precommit Open ticket never blocks: it only masks its target.
        let admission = registry
            .mark_open_dirty(&target)
            .expect("Precommit open ticket");
        let started = Instant::now();
        let masked = registry
            .snapshot_within(StdDuration::from_secs(2))
            .expect("Precommit Open admits conservatively");
        assert!(started.elapsed() < StdDuration::from_millis(200));
        assert!(Arc::ptr_eq(masked.generation(), &old));
        assert_eq!(
            masked.forced_open_unavailable(),
            &BTreeSet::from([target.clone()])
        );
        registry.rollback_open_dirty(&target, admission);
        let unmasked = registry
            .snapshot_within(StdDuration::from_secs(2))
            .expect("rolled back");
        assert!(unmasked.forced_open_unavailable().is_empty());
        assert_eq!(registry.snapshot_wait_timeouts(), 0);
    }

    #[test]
    fn snapshot_within_does_not_wait_when_no_generation_is_published() {
        let registry = Arc::new(PreparedServingRegistry::default());
        let started = Instant::now();
        assert!(matches!(
            registry.snapshot_within(StdDuration::from_secs(5)),
            Err(PreparedServingError::SnapshotUnavailable)
        ));
        assert!(started.elapsed() < StdDuration::from_millis(200));

        // Even behind a committed ticket: nothing can be waited for.
        let target = target("NB");
        let demand = dead_channel_demand(&registry);
        demand
            .begin_open_publication(target)
            .expect("Open publication barrier")
            .commit();
        let started = Instant::now();
        assert!(matches!(
            registry.snapshot_within(StdDuration::from_secs(5)),
            Err(PreparedServingError::SnapshotUnavailable)
        ));
        assert!(started.elapsed() < StdDuration::from_millis(200));
        assert_eq!(registry.snapshot_wait_timeouts(), 0);
    }

    #[test]
    fn closing_the_registry_wakes_waiters_and_fails_fast_afterwards() {
        let target = target("NB");
        let (registry, old) = published_registry(&target);
        let demand = dead_channel_demand(&registry);
        demand
            .begin_open_publication(target.clone())
            .expect("Open publication barrier")
            .commit();

        let closing_registry = Arc::clone(&registry);
        let closer = std::thread::spawn(move || {
            std::thread::sleep(StdDuration::from_millis(50));
            closing_registry.close();
        });
        let started = Instant::now();
        let result = registry.snapshot_within(StdDuration::from_secs(5));
        let waited = started.elapsed();
        closer.join().expect("closing thread");
        assert!(matches!(
            result,
            Err(PreparedServingError::SnapshotRebuilding)
        ));
        assert!(
            waited >= StdDuration::from_millis(40) && waited < StdDuration::from_secs(2),
            "close must release the waiter well before its timeout; waited {waited:?}"
        );
        assert_eq!(
            registry.snapshot_wait_timeouts(),
            0,
            "a close-released waiter did not time out"
        );

        // Closed and still blocked: fail fast, no wait at all.
        let started = Instant::now();
        assert!(matches!(
            registry.snapshot_within(StdDuration::from_secs(5)),
            Err(PreparedServingError::SnapshotRebuilding)
        ));
        assert!(started.elapsed() < StdDuration::from_millis(200));

        // Closed only affects waiting: the last-good generation still binds
        // once nothing blocks admission.
        let dirty_epoch = registry
            .dirty
            .lock()
            .expect("dirty state")
            .open_epochs
            .get(&target)
            .and_then(|epochs| epochs.keys().next().copied())
            .expect("committed epoch");
        registry.clear_open_tickets(&target, &BTreeSet::from([dirty_epoch]));
        assert!(Arc::ptr_eq(
            registry
                .snapshot_within(StdDuration::from_secs(1))
                .expect("closed registry still serves its LKG")
                .generation(),
            &old
        ));
        registry.reopen();
        assert!(!registry.closed.load(Ordering::Relaxed));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn file_backed_worker_publication_barrier_preserves_pinned_generation_without_mixing() {
        let directory = tempfile::tempdir().expect("worker temp directory");
        let database = directory.path().join("prepared-worker.sqlite");
        let target = target("NB");
        let mut initial_storage = OperationalStorage::open(database).expect("file-backed storage");
        publish_worker_catalog(&mut initial_storage, &target);
        publish_worker_open(&mut initial_storage, &target, 70, true);
        let storage = Arc::new(Mutex::new(initial_storage));

        let now = OffsetDateTime::from_unix_timestamp(1_784_289_602).expect("fixed 2026-07-17");
        let policy =
            RefreshPolicy::try_new(StdDuration::from_secs(600), GeneralOpenInterval::public())
                .expect("worker policy");
        let policy_provider = crate::FixedRefreshPolicyProvider::new(policy);
        let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
        let build_runtime =
            SharedRuntimeContext::new(OpenCounterAudience::Public, move || now, policy_provider);
        let initial = {
            let mut storage = storage.lock().expect("initial storage lock");
            build_prepared_serving_snapshot(
                &mut storage,
                &build_runtime,
                &open_runtime,
                ServiceRuntimeV1::Public,
            )
            .expect("initial prepared generation")
        };
        let registry = Arc::new(PreparedServingRegistry::default());
        let old = registry
            .publish_if_unchanged(None, initial)
            .expect("publish initial generation");
        let old_foundation = Arc::clone(&old.foundation);
        let old_vector = old
            .open_vector(std::slice::from_ref(&target))
            .expect("old vector");

        let worker = PreparedServingRebuildRuntime::spawn(
            Arc::clone(&storage),
            policy_provider,
            OpenCounterAudience::Public,
            Arc::clone(&open_runtime),
            Arc::clone(&registry),
        );
        let demand = worker.demand();

        let mut input = FilterValuesInputV1::for_term(TermId::try_from("92026").unwrap());
        input.campuses = Vec::new();
        let request = CourseQueryRequestV1 {
            filters: FilterRequestV1::new(
                NormalizedFilterValuesV1::try_new(input).expect("neutral worker filters"),
            ),
            page: PageRequestV1::default(),
            sort: CourseSortV1::default(),
        };
        let old_binding = registry.snapshot().expect("pin old route binding");
        let old_response = crate::PreparedQueryService::from_binding(&old_binding)
            .course_search(std::slice::from_ref(&target), &request, now)
            .expect("old prepared route");
        assert_eq!(
            old_response.items[0].variants[0].sections[0].open.state,
            LiveOpenStateV1::Open
        );

        let barrier = demand
            .begin_open_publication(target.clone())
            .expect("install publication barrier");
        let precommit = registry
            .snapshot()
            .expect("precommit binding remains admitted");
        assert!(Arc::ptr_eq(precommit.generation(), &old));
        assert_eq!(
            precommit.forced_open_unavailable(),
            &BTreeSet::from([target.clone()])
        );
        let precommit_response = crate::PreparedQueryService::from_binding(&precommit)
            .course_search(std::slice::from_ref(&target), &request, now)
            .expect("precommit forced-unavailable route");
        assert_eq!(
            precommit_response.items[0].variants[0].sections[0]
                .open
                .uncertainty,
            Some(MatchReasonCode::SourceUnavailable)
        );

        {
            let mut storage = storage.lock().expect("authoritative writer lock");
            publish_worker_open(&mut storage, &target, 72, false);
        }
        barrier.commit();
        assert!(matches!(
            registry.snapshot(),
            Err(PreparedServingError::SnapshotRebuilding)
        ));

        // A request path parks on the committed ticket exactly the way the
        // HTTP surface does (a blocking-pool thread) and is released by the
        // real worker's clear_open_tickets, bound to the generation it built.
        let waiting_registry = Arc::clone(&registry);
        let new_binding = tokio::task::spawn_blocking(move || {
            waiting_registry.snapshot_within(StdDuration::from_secs(5))
        })
        .await
        .expect("waiting task")
        .expect("worker must clear the committed ticket within the admission wait");
        assert!(!Arc::ptr_eq(new_binding.generation(), &old));
        assert_eq!(registry.snapshot_wait_timeouts(), 0);
        assert!(Arc::ptr_eq(
            registry
                .snapshot()
                .expect("zero-wait binding after the worker cleared")
                .generation(),
            new_binding.generation()
        ));
        let new = new_binding.generation();
        assert!(Arc::ptr_eq(&new.foundation, &old_foundation));
        let new_vector = new
            .open_vector(std::slice::from_ref(&target))
            .expect("new vector");
        assert_eq!(
            new_vector[0].catalog_content_version,
            old_vector[0].catalog_content_version
        );
        assert!(new_vector[0].observation_sequence > old_vector[0].observation_sequence);

        let new_response = crate::PreparedQueryService::from_binding(&new_binding)
            .course_search(std::slice::from_ref(&target), &request, now)
            .expect("new prepared route");
        assert_eq!(
            new_response.items[0].variants[0].sections[0].open.state,
            LiveOpenStateV1::Closed
        );
        let old_after_publish = crate::PreparedQueryService::from_binding(&old_binding)
            .course_search(std::slice::from_ref(&target), &request, now)
            .expect("pinned old route completes after replacement");
        assert_eq!(
            old_after_publish.items[0].variants[0].sections[0]
                .open
                .state,
            LiveOpenStateV1::Open
        );
        assert_eq!(
            serde_json::to_value(old_after_publish).unwrap(),
            serde_json::to_value(old_response).unwrap(),
            "the pinned request never observes a mixed generation"
        );

        worker.shutdown().await;
    }

    #[test]
    fn inverted_open_commit_order_clears_only_the_built_ticket() {
        let target = target("NB");
        let (registry, old) = published_registry(&target);
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let demand = PreparedServingRebuildDemand {
            sender,
            registry: Arc::clone(&registry),
            lifecycle: PublicationLifecycle::default(),
        };
        let first = demand
            .begin_open_publication(target.clone())
            .expect("first barrier");
        let second = demand
            .begin_open_publication(target.clone())
            .expect("second barrier");

        second.commit();
        let second_epoch = match receiver.try_recv().expect("second demand") {
            PreparedRebuildRequest::Open(_, epoch) => epoch,
            other => panic!("unexpected request: {other:?}"),
        };
        let failed = registry
            .publish_if_unchanged(
                Some(&old),
                dummy_snapshot(&target, 7, 9, Some(MatchReasonCode::SourceUnavailable)),
            )
            .expect("publish later failed attempt");
        registry.clear_open_tickets(&target, &BTreeSet::from([second_epoch]));
        let precommit = registry
            .snapshot()
            .expect("first precommit remains conservative");
        assert_eq!(
            precommit
                .open_vector(std::slice::from_ref(&target))
                .expect("vector")[0]
                .attempt_sequence,
            9
        );

        first.commit();
        assert!(matches!(
            registry.snapshot(),
            Err(PreparedServingError::SnapshotRebuilding)
        ));
        let first_epoch = match receiver.try_recv().expect("first demand") {
            PreparedRebuildRequest::Open(_, epoch) => epoch,
            other => panic!("unexpected request: {other:?}"),
        };
        let recovered = registry
            .publish_if_unchanged(Some(&failed), dummy_snapshot(&target, 8, 10, None))
            .expect("publish recovered attempt");
        registry.clear_open_tickets(&target, &BTreeSet::from([first_epoch]));
        assert!(Arc::ptr_eq(
            registry
                .snapshot()
                .expect("recovered admitted")
                .generation(),
            &recovered
        ));
    }

    #[test]
    fn three_target_open_tickets_and_full_ticket_remain_fail_closed_until_each_build_completes() {
        let targets = [target("NB"), target("NK"), target("CM")];
        let registry = Arc::new(PreparedServingRegistry::default());
        let old = registry
            .publish_if_unchanged(
                None,
                dummy_snapshot_many(&[
                    (targets[0].clone(), 7, 8, None),
                    (targets[1].clone(), 7, 8, None),
                    (targets[2].clone(), 7, 8, None),
                ]),
            )
            .expect("publish three-target fixture");
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let demand = PreparedServingRebuildDemand {
            sender,
            registry: Arc::clone(&registry),
            lifecycle: PublicationLifecycle::default(),
        };
        let open = targets
            .iter()
            .cloned()
            .map(|target| demand.begin_open_publication(target).expect("Open barrier"))
            .collect::<Vec<_>>();
        let full = demand.begin_publication().expect("full barrier");
        for barrier in open {
            barrier.commit();
        }
        full.commit();
        let mut open_epochs = BTreeMap::<TermCampusKey, BTreeSet<u64>>::new();
        let mut full_epoch = None;
        for _ in 0..4 {
            match receiver.try_recv().expect("publication demand") {
                PreparedRebuildRequest::Open(target, epoch) => {
                    open_epochs.entry(target).or_default().insert(epoch);
                }
                PreparedRebuildRequest::Full(epoch) => full_epoch = Some(epoch),
                PreparedRebuildRequest::Shutdown => panic!("unexpected shutdown"),
            }
        }
        let foundation = registry
            .publish_if_unchanged(
                Some(&old),
                dummy_snapshot_many(&[
                    (targets[0].clone(), 7, 8, None),
                    (targets[1].clone(), 7, 8, None),
                    (targets[2].clone(), 7, 8, None),
                ]),
            )
            .expect("publish full rebuild");
        registry.clear_full_tickets(&BTreeSet::from([full_epoch.expect("full epoch")]));
        assert!(matches!(
            registry.snapshot(),
            Err(PreparedServingError::SnapshotRebuilding)
        ));
        let overlays = registry
            .publish_if_unchanged(
                Some(&foundation),
                dummy_snapshot_many(&[
                    (targets[0].clone(), 8, 9, None),
                    (targets[1].clone(), 9, 10, None),
                    (targets[2].clone(), 10, 11, None),
                ]),
            )
            .expect("publish Open rebuild");
        for (target, epochs) in &open_epochs {
            registry.clear_open_tickets(target, epochs);
        }
        let admitted = registry.snapshot().expect("all tickets completed");
        assert!(Arc::ptr_eq(admitted.generation(), &overlays));
        assert_eq!(
            admitted
                .open_vector(&targets)
                .expect("ordered three-target vector")
                .iter()
                .map(|entry| entry.observation_sequence)
                .collect::<Vec<_>>(),
            [8, 9, 10]
        );
    }

    #[test]
    fn inverted_full_commits_and_open_to_full_promotion_preserve_newer_exact_tickets() {
        let target = target("NB");
        let (registry, old) = published_registry(&target);
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let demand = PreparedServingRebuildDemand {
            sender,
            registry: Arc::clone(&registry),
            lifecycle: PublicationLifecycle::default(),
        };
        let first = demand.begin_publication().expect("first full barrier");
        let second = demand.begin_publication().expect("second full barrier");
        second.commit();
        let second_epoch = match receiver.try_recv().expect("second full demand") {
            PreparedRebuildRequest::Full(epoch) => epoch,
            other => panic!("unexpected request: {other:?}"),
        };
        let second_build = registry
            .publish_if_unchanged(Some(&old), dummy_snapshot(&target, 7, 8, None))
            .expect("second full build");
        registry.clear_full_tickets(&BTreeSet::from([second_epoch]));
        assert!(matches!(
            registry.snapshot(),
            Err(PreparedServingError::SnapshotRebuilding)
        ));
        first.commit();
        let first_epoch = match receiver.try_recv().expect("first full demand") {
            PreparedRebuildRequest::Full(epoch) => epoch,
            other => panic!("unexpected request: {other:?}"),
        };
        let first_build = registry
            .publish_if_unchanged(Some(&second_build), dummy_snapshot(&target, 7, 8, None))
            .expect("first full build");
        registry.clear_full_tickets(&BTreeSet::from([first_epoch]));
        assert!(Arc::ptr_eq(
            registry
                .snapshot()
                .expect("both full tickets complete")
                .generation(),
            &first_build
        ));

        let older = registry.mark_open_dirty(&target).expect("older Open");
        registry
            .commit_open_dirty(&target, older.epoch)
            .expect("commit older Open");
        let newer = registry.mark_open_dirty(&target).expect("newer Open");
        registry
            .commit_open_dirty(&target, newer.epoch)
            .expect("commit newer Open");
        let promoted = registry
            .promote_open_tickets_to_full(&BTreeMap::from([(
                target.clone(),
                BTreeSet::from([older.epoch]),
            )]))
            .expect("promote only older Open ticket");
        registry.clear_full_tickets(&BTreeSet::from([promoted]));
        assert!(matches!(
            registry.snapshot(),
            Err(PreparedServingError::SnapshotRebuilding)
        ));
        registry.clear_open_tickets(&target, &BTreeSet::from([newer.epoch]));
        assert!(registry.snapshot().is_ok());
    }

    #[test]
    fn shutdown_admission_wait_is_bounded_and_capacity_one_metrics_are_exact() {
        let target = target("NB");
        let (registry, first) = published_registry(&target);
        let replacement = registry
            .publish_if_unchanged(Some(&first), dummy_snapshot(&target, 8, 9, None))
            .expect("replace capacity-one slot");
        assert_eq!(registry.publishes.load(Ordering::Relaxed), 2);
        assert_eq!(registry.replacements.load(Ordering::Relaxed), 1);
        assert_eq!(registry.evictions.load(Ordering::Relaxed), 1);
        assert!(Arc::ptr_eq(
            registry.snapshot().expect("hit").generation(),
            &replacement
        ));
        assert_eq!(registry.hits.load(Ordering::Relaxed), 1);

        let (sender, _receiver) = mpsc::unbounded_channel();
        let demand = PreparedServingRebuildDemand {
            sender,
            registry: Arc::clone(&registry),
            lifecycle: PublicationLifecycle::default(),
        };
        let barrier = demand.begin_publication().expect("active admission");
        assert!(
            !demand.lifecycle.stop_and_wait(std::time::Duration::ZERO),
            "zero-time shutdown drain must return instead of waiting"
        );
        drop(barrier);
        assert!(
            demand.lifecycle.stop_and_wait(std::time::Duration::ZERO),
            "rolled-back admission drains deterministically"
        );
        assert!(Arc::ptr_eq(
            registry
                .snapshot()
                .expect("rollback keeps replacement")
                .generation(),
            &replacement
        ));
    }

    #[test]
    fn same_observation_sequence_tracks_later_failure_attempt_generation() {
        let target = target("NB");
        let old = dummy_snapshot(&target, 11, 12, None);
        let failed = dummy_snapshot(&target, 11, 13, Some(MatchReasonCode::SourceUnavailable));
        let old_vector = old
            .open_vector(std::slice::from_ref(&target))
            .expect("old vector");
        let failed_vector = failed
            .open_vector(std::slice::from_ref(&target))
            .expect("failed vector");
        assert_eq!(
            old_vector[0].observation_sequence,
            failed_vector[0].observation_sequence
        );
        assert!(old_vector[0].attempt_sequence < failed_vector[0].attempt_sequence);
    }

    #[test]
    fn stopped_worker_lifecycle_rejects_new_publications_without_dirtying_the_lkg() {
        let target = target("NB");
        let (registry, old) = published_registry(&target);
        let (sender, _receiver) = mpsc::unbounded_channel();
        let demand = PreparedServingRebuildDemand {
            sender,
            registry: Arc::clone(&registry),
            lifecycle: PublicationLifecycle::default(),
        };
        assert!(demand.lifecycle.stop_and_wait(std::time::Duration::ZERO));
        assert!(matches!(
            demand.begin_open_publication(target.clone()),
            Err(PreparedServingError::PublicationWorkerUnavailable)
        ));
        assert!(matches!(
            demand.begin_publication(),
            Err(PreparedServingError::PublicationWorkerUnavailable)
        ));
        assert!(Arc::ptr_eq(
            registry.snapshot().expect("unchanged LKG").generation(),
            &old
        ));
    }

    #[test]
    fn resident_budget_rejects_an_oversized_generation() {
        assert!(ensure_prepared_resident_budget(PREPARED_FOUNDATION_RESIDENT_BYTE_BUDGET).is_ok());
        assert!(matches!(
            ensure_prepared_resident_budget(PREPARED_FOUNDATION_RESIDENT_BYTE_BUDGET + 1),
            Err(PreparedServingError::ResidentBudgetExceeded { .. })
        ));
    }
}
