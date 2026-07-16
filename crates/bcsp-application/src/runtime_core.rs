use std::collections::{BTreeMap, BTreeSet};
use std::sync::RwLock;
use std::time::Duration;

use bcsp_catalog::{ProjectionError, to_catalog_discovery_response_v1};
use bcsp_contracts::{
    CatalogDiscoveryResponseV1, CourseDetailRequestV1, CourseDetailResponseV1,
    CourseQueryRequestV1, CourseQueryResponseV1, FilterSchemaV1, LiveOpenEvidenceV1,
    LiveOpenStateV1, MatchReasonCode, OpenCircuitState, OpenCircuitStatusV1, OpenSchedulerLane,
    OpenSchedulerStatusV1, OpenState, SectionDetailRequestV1, SectionDetailResponseV1,
    SectionQueryRequestV1, SectionQueryResponseV1, TermCampusKey, filter_schema_v1,
};
use bcsp_open::{
    GeneralOpenInterval, OpenCounterAudience, OpenProjectionError, OpenProjectionRuntime,
    ProjectedOpenStatusV1, project_open_status,
};
use bcsp_operational_storage::{DiscoverySnapshot, OperationalStorage, StorageError};
use bcsp_query::OpenEvidence;
use thiserror::Error;
use time::OffsetDateTime;

use crate::{SharedQueryError, SharedQueryService};

/// The refresh values consumed by shared Catalog/Open services.
///
/// Target adapters remain responsible for validating their own configuration
/// surface and converting it into this target-neutral type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RefreshPolicy {
    catalog_interval: Duration,
    open_general_interval: GeneralOpenInterval,
}

impl RefreshPolicy {
    pub fn try_new(
        catalog_interval: Duration,
        open_general_interval: GeneralOpenInterval,
    ) -> Result<Self, RefreshPolicyError> {
        if catalog_interval.is_zero() {
            return Err(RefreshPolicyError::ZeroCatalogInterval);
        }
        Ok(Self {
            catalog_interval,
            open_general_interval,
        })
    }

    pub const fn catalog_interval(self) -> Duration {
        self.catalog_interval
    }

    pub const fn open_general_interval(self) -> GeneralOpenInterval {
        self.open_general_interval
    }

    pub fn effective_open_interval(self, has_active_watch: bool) -> Duration {
        self.open_general_interval.effective(has_active_watch)
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RefreshPolicyError {
    #[error("Catalog refresh interval must be positive")]
    ZeroCatalogInterval,
}

/// Dynamic policy seam. An entrypoint can expose its current policy for every
/// projection without teaching the shared services how that policy is stored.
pub trait RefreshPolicyProvider: Send + Sync {
    fn refresh_policy(&self) -> Result<RefreshPolicy, RefreshPolicyReadError>;
}

impl<F> RefreshPolicyProvider for F
where
    F: Fn() -> Result<RefreshPolicy, RefreshPolicyReadError> + Send + Sync,
{
    fn refresh_policy(&self) -> Result<RefreshPolicy, RefreshPolicyReadError> {
        self()
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("refresh policy is unavailable")]
pub struct RefreshPolicyReadError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FixedRefreshPolicyProvider(RefreshPolicy);

impl FixedRefreshPolicyProvider {
    pub const fn new(policy: RefreshPolicy) -> Self {
        Self(policy)
    }
}

impl RefreshPolicyProvider for FixedRefreshPolicyProvider {
    fn refresh_policy(&self) -> Result<RefreshPolicy, RefreshPolicyReadError> {
        Ok(self.0)
    }
}

pub trait ApplicationClock: Send + Sync {
    fn now(&self) -> OffsetDateTime;
}

impl<F> ApplicationClock for F
where
    F: Fn() -> OffsetDateTime + Send + Sync,
{
    fn now(&self) -> OffsetDateTime {
        self()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemApplicationClock;

impl ApplicationClock for SystemApplicationClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

/// Ephemeral scheduler/circuit facts for one Open target. These are never
/// reconstructed as active work after restart. Durable attempts, LKG state,
/// and counters continue to come from [`OperationalStorage`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenRuntimeSnapshot {
    pub lane: OpenSchedulerLane,
    pub active_watch_count: u64,
    pub next_due_at: Option<OffsetDateTime>,
    pub in_flight: bool,
    pub scheduler_lag_milliseconds: u64,
    pub actual_start_to_start_interval_milliseconds: Option<u64>,
    pub failure_streak: u32,
    pub circuit: OpenCircuitStatusV1,
}

impl Default for OpenRuntimeSnapshot {
    fn default() -> Self {
        Self {
            lane: OpenSchedulerLane::General,
            active_watch_count: 0,
            next_due_at: None,
            in_flight: false,
            scheduler_lag_milliseconds: 0,
            actual_start_to_start_interval_milliseconds: None,
            failure_streak: 0,
            circuit: OpenCircuitStatusV1 {
                state: OpenCircuitState::Closed,
                reason: None,
                opened_at: None,
                retry_at: None,
                diagnostic_recheck_required: false,
            },
        }
    }
}

/// Thread-safe ephemeral scheduler/circuit state keyed by Open target.
///
/// Missing targets deliberately project the neutral runtime snapshot. Durable
/// Open observations and counters remain in [`OperationalStorage`].
#[derive(Debug, Default)]
pub struct OpenRuntimeSnapshotRegistry {
    snapshots: RwLock<BTreeMap<TermCampusKey, OpenRuntimeSnapshot>>,
}

impl OpenRuntimeSnapshotRegistry {
    pub fn snapshot(
        &self,
        target: &TermCampusKey,
    ) -> Result<OpenRuntimeSnapshot, OpenRuntimeSnapshotRegistryError> {
        self.snapshots
            .read()
            .map_err(|_| OpenRuntimeSnapshotRegistryError)?
            .get(target)
            .cloned()
            .map_or_else(|| Ok(OpenRuntimeSnapshot::default()), Ok)
    }

    pub fn snapshots(
        &self,
        targets: &[TermCampusKey],
    ) -> Result<BTreeMap<TermCampusKey, OpenRuntimeSnapshot>, OpenRuntimeSnapshotRegistryError>
    {
        let snapshots = self
            .snapshots
            .read()
            .map_err(|_| OpenRuntimeSnapshotRegistryError)?;
        Ok(targets
            .iter()
            .cloned()
            .map(|target| {
                let snapshot = snapshots.get(&target).cloned().unwrap_or_default();
                (target, snapshot)
            })
            .collect())
    }

    pub fn replace(
        &self,
        target: TermCampusKey,
        snapshot: OpenRuntimeSnapshot,
    ) -> Result<Option<OpenRuntimeSnapshot>, OpenRuntimeSnapshotRegistryError> {
        self.snapshots
            .write()
            .map_err(|_| OpenRuntimeSnapshotRegistryError)
            .map(|mut snapshots| snapshots.insert(target, snapshot))
    }

    pub fn remove(
        &self,
        target: &TermCampusKey,
    ) -> Result<Option<OpenRuntimeSnapshot>, OpenRuntimeSnapshotRegistryError> {
        self.snapshots
            .write()
            .map_err(|_| OpenRuntimeSnapshotRegistryError)
            .map(|mut snapshots| snapshots.remove(target))
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("Open runtime snapshot registry is unavailable")]
pub struct OpenRuntimeSnapshotRegistryError;

/// Small target-neutral composition for both product entrypoints. The caller
/// explicitly selects the counter audience while storage ownership,
/// scheduling, HTTP hosting, and upstream transport remain in their existing
/// owners.
#[derive(Clone)]
pub struct SharedRuntimeContext<C, P> {
    counter_audience: OpenCounterAudience,
    clock: C,
    policy: P,
}

impl<C, P> SharedRuntimeContext<C, P>
where
    C: ApplicationClock,
    P: RefreshPolicyProvider,
{
    pub fn now(&self) -> OffsetDateTime {
        self.clock.now()
    }

    pub const fn new(counter_audience: OpenCounterAudience, clock: C, policy: P) -> Self {
        Self {
            counter_audience,
            clock,
            policy,
        }
    }

    pub const fn counter_audience(&self) -> OpenCounterAudience {
        self.counter_audience
    }

    pub fn refresh_policy(&self) -> Result<RefreshPolicy, SharedRuntimeError> {
        self.policy
            .refresh_policy()
            .map_err(|_| SharedRuntimeError::RefreshPolicyUnavailable)
    }

    pub fn projection_runtime(
        &self,
        snapshot: &OpenRuntimeSnapshot,
    ) -> Result<OpenProjectionRuntime, SharedRuntimeError> {
        let now = self.clock.now();
        let policy = self.refresh_policy()?;
        Ok(self.projection_runtime_at(now, policy, snapshot))
    }

    pub fn open_status(
        &self,
        storage: &mut OperationalStorage,
        target: &TermCampusKey,
        snapshot: &OpenRuntimeSnapshot,
    ) -> Result<ProjectedOpenStatusV1, SharedRuntimeError> {
        let policy = self.refresh_policy()?;
        self.open_status_with_policy(storage, target, snapshot, policy)
    }

    pub(crate) fn open_status_with_policy(
        &self,
        storage: &mut OperationalStorage,
        target: &TermCampusKey,
        snapshot: &OpenRuntimeSnapshot,
        policy: RefreshPolicy,
    ) -> Result<ProjectedOpenStatusV1, SharedRuntimeError> {
        let runtime = self.projection_runtime_at(self.clock.now(), policy, snapshot);
        project_open_status(storage, target, &runtime).map_err(|source| {
            SharedRuntimeError::OpenProjection {
                target: target.clone(),
                source,
            }
        })
    }

    pub fn course_search(
        &self,
        storage: &mut OperationalStorage,
        targets: &[TermCampusKey],
        request: &CourseQueryRequestV1,
        runtime_for: impl Fn(&TermCampusKey) -> OpenRuntimeSnapshot,
    ) -> Result<CourseQueryResponseV1, SharedRuntimeError> {
        let policy = self.refresh_policy()?;
        self.course_search_with_policy(storage, targets, request, runtime_for, policy)
    }

    pub(crate) fn course_search_with_policy(
        &self,
        storage: &mut OperationalStorage,
        targets: &[TermCampusKey],
        request: &CourseQueryRequestV1,
        runtime_for: impl Fn(&TermCampusKey) -> OpenRuntimeSnapshot,
        policy: RefreshPolicy,
    ) -> Result<CourseQueryResponseV1, SharedRuntimeError> {
        self.ensure_catalogs_published(storage, targets)?;
        let (now, evidence) =
            self.query_evidence_with_policy(storage, targets, runtime_for, policy)?;
        SharedQueryService::new(storage)
            .course_search(targets, request, now, &evidence)
            .map_err(SharedRuntimeError::Query)
    }

    pub fn section_search(
        &self,
        storage: &mut OperationalStorage,
        targets: &[TermCampusKey],
        request: &SectionQueryRequestV1,
        runtime_for: impl Fn(&TermCampusKey) -> OpenRuntimeSnapshot,
    ) -> Result<SectionQueryResponseV1, SharedRuntimeError> {
        let policy = self.refresh_policy()?;
        self.section_search_with_policy(storage, targets, request, runtime_for, policy)
    }

    pub(crate) fn section_search_with_policy(
        &self,
        storage: &mut OperationalStorage,
        targets: &[TermCampusKey],
        request: &SectionQueryRequestV1,
        runtime_for: impl Fn(&TermCampusKey) -> OpenRuntimeSnapshot,
        policy: RefreshPolicy,
    ) -> Result<SectionQueryResponseV1, SharedRuntimeError> {
        self.ensure_catalogs_published(storage, targets)?;
        let (now, evidence) =
            self.query_evidence_with_policy(storage, targets, runtime_for, policy)?;
        SharedQueryService::new(storage)
            .section_search(targets, request, now, &evidence)
            .map_err(SharedRuntimeError::Query)
    }

    pub fn course_detail(
        &self,
        storage: &mut OperationalStorage,
        targets: &[TermCampusKey],
        request: &CourseDetailRequestV1,
        runtime_for: impl Fn(&TermCampusKey) -> OpenRuntimeSnapshot,
    ) -> Result<CourseDetailResponseV1, SharedRuntimeError> {
        let policy = self.refresh_policy()?;
        self.course_detail_with_policy(storage, targets, request, runtime_for, policy)
    }

    pub(crate) fn course_detail_with_policy(
        &self,
        storage: &mut OperationalStorage,
        targets: &[TermCampusKey],
        request: &CourseDetailRequestV1,
        runtime_for: impl Fn(&TermCampusKey) -> OpenRuntimeSnapshot,
        policy: RefreshPolicy,
    ) -> Result<CourseDetailResponseV1, SharedRuntimeError> {
        self.ensure_catalogs_published(storage, targets)?;
        let (now, evidence) =
            self.query_evidence_with_policy(storage, targets, runtime_for, policy)?;
        SharedQueryService::new(storage)
            .course_detail(targets, request, now, &evidence)
            .map_err(SharedRuntimeError::Query)
    }

    pub fn section_detail(
        &self,
        storage: &mut OperationalStorage,
        targets: &[TermCampusKey],
        request: &SectionDetailRequestV1,
        runtime_for: impl Fn(&TermCampusKey) -> OpenRuntimeSnapshot,
    ) -> Result<SectionDetailResponseV1, SharedRuntimeError> {
        let policy = self.refresh_policy()?;
        self.section_detail_with_policy(storage, targets, request, runtime_for, policy)
    }

    pub(crate) fn section_detail_with_policy(
        &self,
        storage: &mut OperationalStorage,
        targets: &[TermCampusKey],
        request: &SectionDetailRequestV1,
        runtime_for: impl Fn(&TermCampusKey) -> OpenRuntimeSnapshot,
        policy: RefreshPolicy,
    ) -> Result<SectionDetailResponseV1, SharedRuntimeError> {
        self.ensure_catalogs_published(storage, targets)?;
        let (now, evidence) =
            self.query_evidence_with_policy(storage, targets, runtime_for, policy)?;
        SharedQueryService::new(storage)
            .section_detail(targets, request, now, &evidence)
            .map_err(SharedRuntimeError::Query)
    }

    pub fn filter_schema(&self) -> FilterSchemaV1 {
        filter_schema_v1()
    }

    pub fn catalog_discovery(
        &self,
        storage: &mut OperationalStorage,
    ) -> Result<CatalogDiscoveryResponseV1, SharedRuntimeError> {
        let observed_at = self.clock.now();
        let published = storage
            .published_discovery_snapshot()
            .map_err(SharedRuntimeError::CatalogDiscoveryStorage)?;
        let mut selector = to_catalog_discovery_response_v1(&published, &[], observed_at)
            .map_err(SharedRuntimeError::CatalogDiscoveryProjection)?;
        let mut catalogs = Vec::new();
        for target in selector.targets.iter().map(|target| &target.key) {
            if let Some(catalog) = storage
                .published_catalog_snapshot(target)
                .map_err(SharedRuntimeError::CatalogDiscoveryStorage)?
            {
                catalogs.push(catalog);
            }
        }
        let available = to_catalog_discovery_response_v1(&published, &catalogs, observed_at)
            .map_err(SharedRuntimeError::CatalogDiscoveryProjection)?;
        selector.core_code_dictionaries = available.core_code_dictionaries.clone();
        let selectable_terms = selector
            .targets
            .iter()
            .map(|target| target.key.term().clone())
            .collect::<BTreeSet<_>>();
        let latest_term = latest_automatic_term(&published.snapshot, &selectable_terms);
        if !selector.targets.is_empty() && latest_term.is_none() {
            return Ok(selector);
        }
        let automatic_targets = selector
            .targets
            .iter()
            .map(|target| target.key.clone())
            .filter(|target| latest_term.is_some_and(|term| target.term() == term))
            .collect::<BTreeSet<_>>();

        // Catalog publications arrive target by target. Returning the partial merge would make a
        // browser freeze whichever subject subset happened to publish first. Check the cheap
        // publication states for the whole automatically refreshed term before loading any full
        // snapshot; valid-empty targets have a nonzero content version and count as complete.
        for target in &automatic_targets {
            let ready = storage
                .target_state(target)
                .map_err(SharedRuntimeError::CatalogDiscoveryStorage)?
                .is_some_and(|state| state.current_content_version > 0);
            if !ready {
                return Ok(selector);
            }
        }

        Ok(available)
    }

    fn ensure_catalogs_published(
        &self,
        storage: &OperationalStorage,
        targets: &[TermCampusKey],
    ) -> Result<(), SharedRuntimeError> {
        for target in targets {
            let state = storage.target_state(target).map_err(|source| {
                SharedRuntimeError::Query(SharedQueryError::Storage {
                    target: target.clone(),
                    source,
                })
            })?;
            if !state.is_some_and(|state| state.current_content_version > 0) {
                return Err(SharedRuntimeError::Query(
                    SharedQueryError::TargetNotPublished {
                        target: target.clone(),
                    },
                ));
            }
        }
        Ok(())
    }

    fn query_evidence_with_policy(
        &self,
        storage: &mut OperationalStorage,
        targets: &[TermCampusKey],
        runtime_for: impl Fn(&TermCampusKey) -> OpenRuntimeSnapshot,
        policy: RefreshPolicy,
    ) -> Result<(OffsetDateTime, Vec<OpenEvidence>), SharedRuntimeError> {
        let now = self.clock.now();
        let mut evidence = Vec::new();
        for target in targets {
            let snapshot = runtime_for(target);
            let runtime = self.projection_runtime_at(now, policy, &snapshot);
            let projected = project_open_status(storage, target, &runtime).map_err(|source| {
                SharedRuntimeError::OpenProjection {
                    target: target.clone(),
                    source,
                }
            })?;
            evidence.extend(projected.sections.into_iter().map(open_evidence));
        }
        Ok((now, evidence))
    }

    fn projection_runtime_at(
        &self,
        now: OffsetDateTime,
        policy: RefreshPolicy,
        snapshot: &OpenRuntimeSnapshot,
    ) -> OpenProjectionRuntime {
        let general = policy.open_general_interval();
        let has_active_watch = snapshot.active_watch_count > 0;
        OpenProjectionRuntime {
            now,
            audience: self.counter_audience,
            scheduler: OpenSchedulerStatusV1 {
                lane: snapshot.lane,
                requested_general_interval_seconds: general.seconds(),
                requested_effective_interval_seconds: general.effective_seconds(has_active_watch),
                active_watch_count: snapshot.active_watch_count,
                next_due_at: snapshot.next_due_at,
                in_flight: snapshot.in_flight,
                scheduler_lag_milliseconds: snapshot.scheduler_lag_milliseconds,
                actual_start_to_start_interval_milliseconds: snapshot
                    .actual_start_to_start_interval_milliseconds,
                failure_streak: snapshot.failure_streak,
            },
            circuit: snapshot.circuit.clone(),
        }
    }
}

fn latest_automatic_term<'a>(
    snapshot: &'a DiscoverySnapshot,
    selectable_terms: &BTreeSet<bcsp_contracts::TermId>,
) -> Option<&'a bcsp_contracts::TermId> {
    snapshot
        .terms
        .iter()
        .filter(|term| selectable_terms.contains(&term.term_id))
        .filter_map(|term| {
            Some((
                term.year?,
                term.term_code.as_deref()?.parse::<u8>().ok()?,
                &term.term_id,
            ))
        })
        .max_by_key(|(year, term_code, _)| (*year, *term_code))
        .map(|(_, _, term)| term)
}

fn open_evidence(status: bcsp_contracts::OpenSectionStatusV1) -> OpenEvidence {
    let state = match status.state {
        OpenState::Open => LiveOpenStateV1::Open,
        OpenState::Closed => LiveOpenStateV1::Closed,
        OpenState::Unknown => LiveOpenStateV1::Unknown,
    };
    OpenEvidence {
        section_key: status.section_key,
        evidence: LiveOpenEvidenceV1 {
            state,
            observed_at: status.freshness.observed_at,
            fresh_until: status.freshness.fresh_until,
            uncertainty: status
                .freshness
                .uncertainty
                .map(|_| MatchReasonCode::SourceUnavailable),
        },
    }
}

#[derive(Debug, Error)]
pub enum SharedRuntimeError {
    #[error("refresh policy could not be read")]
    RefreshPolicyUnavailable,
    #[error("Open status projection failed for target {target:?}")]
    OpenProjection {
        target: TermCampusKey,
        #[source]
        source: OpenProjectionError,
    },
    #[error("Catalog discovery storage read failed")]
    CatalogDiscoveryStorage(#[source] StorageError),
    #[error("Catalog discovery projection failed")]
    CatalogDiscoveryProjection(#[source] ProjectionError),
    #[error(transparent)]
    Query(#[from] SharedQueryError),
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU16, Ordering};

    use bcsp_contracts::TraceId;
    use bcsp_open::GeneralOpenInterval;

    use super::*;

    fn run_id() -> TraceId {
        TraceId::from_str("00000000-0000-4000-8000-000000000010").expect("valid run ID")
    }

    fn policy(open_seconds: u32) -> RefreshPolicy {
        RefreshPolicy::try_new(
            Duration::from_secs(600),
            GeneralOpenInterval::local(open_seconds).expect("valid local interval"),
        )
        .expect("valid policy")
    }

    #[test]
    fn active_watch_uses_the_shared_minimum_policy_without_clamping_three_seconds() {
        let fast = policy(3);
        let normal = policy(30);
        assert_eq!(fast.effective_open_interval(false), Duration::from_secs(3));
        assert_eq!(fast.effective_open_interval(true), Duration::from_secs(3));
        assert_eq!(
            normal.effective_open_interval(false),
            Duration::from_secs(30)
        );
        assert_eq!(
            normal.effective_open_interval(true),
            Duration::from_secs(10)
        );
    }

    #[test]
    fn runtime_projection_binds_injected_counter_scope_clock_and_live_policy() {
        let current_seconds = Arc::new(AtomicU16::new(30));
        let provider_seconds = Arc::clone(&current_seconds);
        let provider = move || Ok(policy(u32::from(provider_seconds.load(Ordering::SeqCst))));
        let now = OffsetDateTime::from_unix_timestamp(1_800_000_000).expect("valid time");
        let context = SharedRuntimeContext::new(
            OpenCounterAudience::Local { run_id: run_id() },
            move || now,
            provider,
        );
        let snapshot = OpenRuntimeSnapshot {
            lane: OpenSchedulerLane::ActiveWatch,
            active_watch_count: 2,
            ..OpenRuntimeSnapshot::default()
        };

        let first = context
            .projection_runtime(&snapshot)
            .expect("first projection");
        assert_eq!(first.now, now);
        assert_eq!(
            first.audience,
            OpenCounterAudience::Local { run_id: run_id() }
        );
        assert_eq!(first.scheduler.requested_general_interval_seconds, 30);
        assert_eq!(first.scheduler.requested_effective_interval_seconds, 10);
        assert_eq!(first.scheduler.active_watch_count, 2);

        current_seconds.store(3, Ordering::SeqCst);
        let after_setting_change = context
            .projection_runtime(&snapshot)
            .expect("updated projection");
        assert_eq!(
            after_setting_change
                .scheduler
                .requested_general_interval_seconds,
            3
        );
        assert_eq!(
            after_setting_change
                .scheduler
                .requested_effective_interval_seconds,
            3
        );

        let public = SharedRuntimeContext::new(
            OpenCounterAudience::Public,
            move || now,
            FixedRefreshPolicyProvider::new(policy(30)),
        )
        .projection_runtime(&OpenRuntimeSnapshot::default())
        .expect("public projection");
        assert_eq!(public.audience, OpenCounterAudience::Public);
        assert_eq!(public.scheduler.requested_general_interval_seconds, 30);
        assert_eq!(public.scheduler.requested_effective_interval_seconds, 30);
    }

    #[test]
    fn invalid_catalog_interval_and_unavailable_settings_fail_closed() {
        assert_eq!(
            RefreshPolicy::try_new(
                Duration::ZERO,
                GeneralOpenInterval::local(30).expect("valid Open interval")
            ),
            Err(RefreshPolicyError::ZeroCatalogInterval)
        );
        let context =
            SharedRuntimeContext::new(OpenCounterAudience::Public, OffsetDateTime::now_utc, || {
                Err(RefreshPolicyReadError)
            });
        assert!(matches!(
            context.projection_runtime(&OpenRuntimeSnapshot::default()),
            Err(SharedRuntimeError::RefreshPolicyUnavailable)
        ));
    }
}
