use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use bcsp_application::{CoordinatorStatusSink, CoordinatorStatusSnapshot, SharedWatchSocket};
use bcsp_contracts::{
    FilterRequestV1, NormalizedFilterValuesV1, OpenCircuitState as ContractOpenCircuitState,
    OpenCircuitStatusV1, OpenFreshnessState, OpenSchedulerLane, OpenSchedulerStatusV1,
    TermCampusKey,
};
use bcsp_open::{OpenCounterAudience, OpenProjectionRuntime, project_open_status, rutgers_day_at};
use bcsp_operational_storage::{OpenCircuitState as StoredOpenCircuitState, OpenOriginState};
use bcsp_public_operations::PublicOperationalStore;
use serde::Serialize;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::{PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS, PUBLIC_WATCHED_OPEN_INTERVAL_SECONDS};

const PUBLIC_RUTGERS_ORIGIN_ID: &str = "rutgers_catalog";

pub type SharedPublicOperationalStore = Arc<Mutex<PublicOperationalStore>>;

pub trait PublicClock: Send + Sync + 'static {
    fn now(&self) -> OffsetDateTime;
}

impl<F> PublicClock for F
where
    F: Fn() -> OffsetDateTime + Send + Sync + 'static,
{
    fn now(&self) -> OffsetDateTime {
        self()
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemPublicClock;

impl PublicClock for SystemPublicClock {
    fn now(&self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicSchedulerSnapshot {
    pub maximum_lag_milliseconds: u64,
    pub origin_circuit_open: bool,
    pub overloaded: bool,
    pub in_flight: bool,
}

pub trait PublicSchedulerStatusSource: Send + Sync + 'static {
    fn snapshot(&self) -> PublicSchedulerSnapshot;

    fn scheduler_running(&self) -> bool;
}

#[derive(Default)]
pub struct InMemoryPublicSchedulerStatus {
    snapshot: RwLock<PublicSchedulerSnapshot>,
    running: AtomicBool,
}

impl InMemoryPublicSchedulerStatus {
    pub fn publish(&self, snapshot: PublicSchedulerSnapshot) {
        match self.snapshot.write() {
            Ok(mut current) => {
                *current = snapshot;
                self.running.store(true, Ordering::Release);
            }
            Err(_) => tracing::error!(code = "PUBLIC_SCHEDULER_STATE_UNAVAILABLE"),
        }
    }

    pub fn mark_stopped(&self) {
        self.running.store(false, Ordering::Release);
    }
}

impl CoordinatorStatusSink for InMemoryPublicSchedulerStatus {
    fn publish(&self, snapshot: CoordinatorStatusSnapshot) {
        self.publish(PublicSchedulerSnapshot {
            maximum_lag_milliseconds: snapshot.maximum_lag_milliseconds,
            origin_circuit_open: snapshot.origin_circuit_open,
            overloaded: snapshot.overloaded,
            in_flight: snapshot.in_flight,
        });
    }

    fn mark_stopped(&self) {
        self.mark_stopped();
    }
}

impl PublicSchedulerStatusSource for InMemoryPublicSchedulerStatus {
    fn snapshot(&self) -> PublicSchedulerSnapshot {
        match self.snapshot.read() {
            Ok(snapshot) => *snapshot,
            Err(_) => PublicSchedulerSnapshot {
                origin_circuit_open: true,
                overloaded: true,
                ..PublicSchedulerSnapshot::default()
            },
        }
    }

    fn scheduler_running(&self) -> bool {
        self.running.load(Ordering::Acquire)
    }
}

pub trait PublicWatchDemandSource: Send + Sync + 'static {
    fn connection_count(&self) -> u64;

    fn total_active_watch_count(&self) -> u64;

    fn active_watch_count(&self, target: &TermCampusKey) -> u64;
}

impl PublicWatchDemandSource for SharedWatchSocket {
    fn connection_count(&self) -> u64 {
        u64::try_from(self.connection_count()).unwrap_or(u64::MAX)
    }

    fn total_active_watch_count(&self) -> u64 {
        u64::try_from(self.total_active_watch_count()).unwrap_or(u64::MAX)
    }

    fn active_watch_count(&self, target: &TermCampusKey) -> u64 {
        SharedWatchSocket::active_watch_count(self, target)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicDayCounters {
    pub attempted: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub empty: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicStatusLevel {
    Ready,
    Degraded,
    NotReady,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicReadinessReason {
    OperationalStateUnavailable,
    SchedulerNotRunning,
    CatalogUnavailable,
    OpenUnavailable,
    PartialCatalog,
    PartialOpen,
    OpenStale,
    SchedulerLagging,
    BackingOff,
    OriginCircuitOpen,
    SchedulerOverloaded,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicServiceSnapshot {
    pub status: PublicStatusLevel,
    pub ready: bool,
    pub reasons: Vec<PublicReadinessReason>,
    pub catalog_target_count: u64,
    pub catalog_available_target_count: u64,
    pub open_available_target_count: u64,
    pub websocket_connection_count: u64,
    pub active_watch_count: u64,
    pub scheduler: PublicSchedulerSnapshot,
    pub rutgers_day: String,
    pub today_counts: PublicDayCounters,
}

pub trait PublicServiceStateSource: Send + Sync + 'static {
    fn snapshot(&self) -> PublicServiceSnapshot;

    fn default_current_filters(&self) -> Option<FilterRequestV1>;
}

pub struct PublicServiceInspector {
    store: SharedPublicOperationalStore,
    scheduler: Arc<dyn PublicSchedulerStatusSource>,
    watch_demand: Arc<dyn PublicWatchDemandSource>,
    clock: Arc<dyn PublicClock>,
}

impl PublicServiceInspector {
    pub fn new(
        store: SharedPublicOperationalStore,
        scheduler: Arc<dyn PublicSchedulerStatusSource>,
        watch_demand: Arc<dyn PublicWatchDemandSource>,
        clock: Arc<dyn PublicClock>,
    ) -> Self {
        Self {
            store,
            scheduler,
            watch_demand,
            clock,
        }
    }

    pub fn with_system_clock(
        store: SharedPublicOperationalStore,
        scheduler: Arc<dyn PublicSchedulerStatusSource>,
        watch_demand: Arc<dyn PublicWatchDemandSource>,
    ) -> Self {
        Self::new(store, scheduler, watch_demand, Arc::new(SystemPublicClock))
    }

    fn inspect(&self) -> Result<PublicServiceSnapshot, ()> {
        let now = self.clock.now();
        let rutgers_day = rutgers_day_at(now).map_err(|_| ())?;
        let mut scheduler = self.scheduler.snapshot();
        let scheduler_running = self.scheduler.scheduler_running();
        let websocket_connection_count = self.watch_demand.connection_count();
        let active_watch_count = self.watch_demand.total_active_watch_count();
        let mut store = self.store.lock().map_err(|_| ())?;
        let persisted_circuit = store
            .storage()
            .open_origin_state(PUBLIC_RUTGERS_ORIGIN_ID)
            .map_err(|_| ())?;
        let circuit = persisted_circuit_status(persisted_circuit.as_ref(), now)?;
        scheduler.origin_circuit_open |= circuit.active;
        let mut scheduler_backing_off = circuit.backing_off;
        let targets = store.storage().discovered_targets().map_err(|_| ())?;
        let mut catalog_available_target_count = 0_u64;
        let mut open_available_target_count = 0_u64;
        let mut open_stale_target_count = 0_u64;
        let mut maximum_persisted_lag = 0_u64;
        for target in &targets {
            let catalog_available = store
                .storage()
                .target_state(target)
                .map_err(|_| ())?
                .is_some_and(|state| state.current_content_version > 0);
            if catalog_available {
                catalog_available_target_count = catalog_available_target_count.saturating_add(1);
            }
            let schedule = store
                .storage()
                .open_schedule_state(target)
                .map_err(|_| ())?;
            if let Some(schedule) = &schedule {
                maximum_persisted_lag = maximum_persisted_lag.max(schedule.schedule_lag_ms);
                if schedule.failure_streak > 0
                    && parse_timestamp(&schedule.next_due_at).is_ok_and(|due| due > now)
                {
                    scheduler_backing_off = true;
                }
            }
            if catalog_available {
                let active_target_watches = self.watch_demand.active_watch_count(target);
                let projected = project_open_status(
                    store.storage_mut(),
                    target,
                    &OpenProjectionRuntime {
                        now,
                        audience: OpenCounterAudience::Public,
                        scheduler: projected_scheduler_status(
                            active_target_watches,
                            schedule.as_ref(),
                            scheduler.in_flight,
                        )?,
                        circuit: circuit.status.clone(),
                    },
                )
                .map_err(|_| ())?;
                let freshness = &projected.refresh.freshness;
                let (available, stale) = open_freshness_availability(freshness.state);
                if available {
                    open_available_target_count = open_available_target_count.saturating_add(1);
                }
                if stale {
                    open_stale_target_count = open_stale_target_count.saturating_add(1);
                }
            }
        }
        let counters = store.service_day_counters(&rutgers_day).map_err(|_| ())?;
        let catalog_target_count = u64::try_from(targets.len()).unwrap_or(u64::MAX);
        scheduler.maximum_lag_milliseconds = scheduler
            .maximum_lag_milliseconds
            .max(maximum_persisted_lag);

        let mut reasons = Vec::new();
        if !scheduler_running
            && catalog_available_target_count > 0
            && open_available_target_count > 0
        {
            reasons.push(PublicReadinessReason::SchedulerNotRunning);
        }
        if catalog_available_target_count == 0 {
            reasons.push(PublicReadinessReason::CatalogUnavailable);
        } else if catalog_available_target_count < catalog_target_count {
            reasons.push(PublicReadinessReason::PartialCatalog);
        }
        if open_available_target_count == 0 {
            reasons.push(PublicReadinessReason::OpenUnavailable);
        } else if open_available_target_count < catalog_available_target_count {
            reasons.push(PublicReadinessReason::PartialOpen);
        }
        if open_stale_target_count > 0 {
            reasons.push(PublicReadinessReason::OpenStale);
        }
        if scheduler.maximum_lag_milliseconds > 0 {
            reasons.push(PublicReadinessReason::SchedulerLagging);
        }
        if scheduler_backing_off {
            reasons.push(PublicReadinessReason::BackingOff);
        }
        if scheduler.origin_circuit_open {
            reasons.push(PublicReadinessReason::OriginCircuitOpen);
        }
        if scheduler.overloaded {
            reasons.push(PublicReadinessReason::SchedulerOverloaded);
        }
        let ready = scheduler_running
            && catalog_available_target_count > 0
            && open_available_target_count > 0;
        let status = if !ready {
            PublicStatusLevel::NotReady
        } else if reasons.is_empty() {
            PublicStatusLevel::Ready
        } else {
            PublicStatusLevel::Degraded
        };
        Ok(PublicServiceSnapshot {
            status,
            ready,
            reasons,
            catalog_target_count,
            catalog_available_target_count,
            open_available_target_count,
            websocket_connection_count,
            active_watch_count,
            scheduler,
            rutgers_day,
            today_counts: PublicDayCounters {
                attempted: counters.attempted,
                succeeded: counters.succeeded,
                failed: counters.failed,
                empty: counters.empty,
            },
        })
    }

    fn unavailable_snapshot(&self) -> PublicServiceSnapshot {
        PublicServiceSnapshot {
            status: PublicStatusLevel::NotReady,
            ready: false,
            reasons: vec![PublicReadinessReason::OperationalStateUnavailable],
            catalog_target_count: 0,
            catalog_available_target_count: 0,
            open_available_target_count: 0,
            websocket_connection_count: self.watch_demand.connection_count(),
            active_watch_count: self.watch_demand.total_active_watch_count(),
            scheduler: self.scheduler.snapshot(),
            rutgers_day: "1970-01-01".to_owned(),
            today_counts: PublicDayCounters::default(),
        }
    }
}

struct PersistedCircuitStatus {
    status: OpenCircuitStatusV1,
    active: bool,
    backing_off: bool,
}

fn persisted_circuit_status(
    persisted: Option<&OpenOriginState>,
    now: OffsetDateTime,
) -> Result<PersistedCircuitStatus, ()> {
    let Some(persisted) = persisted else {
        return Ok(PersistedCircuitStatus {
            status: OpenCircuitStatusV1 {
                state: ContractOpenCircuitState::Closed,
                reason: None,
                opened_at: None,
                retry_at: None,
                diagnostic_recheck_required: false,
            },
            active: false,
            backing_off: false,
        });
    };
    let opened_at = persisted
        .opened_at
        .as_deref()
        .map(parse_timestamp)
        .transpose()?;
    let retry_at = persisted
        .retry_at
        .as_deref()
        .map(parse_timestamp)
        .transpose()?;
    let (state, active, backing_off) = match persisted.circuit_state {
        StoredOpenCircuitState::Closed => (ContractOpenCircuitState::Closed, false, false),
        StoredOpenCircuitState::RetryAfter => {
            let active = retry_at.is_some_and(|retry_at| retry_at > now);
            (ContractOpenCircuitState::RetryAfter, active, active)
        }
        StoredOpenCircuitState::FatalDiagnostic => {
            (ContractOpenCircuitState::FatalDiagnostic, true, false)
        }
    };
    Ok(PersistedCircuitStatus {
        status: OpenCircuitStatusV1 {
            state,
            reason: None,
            opened_at,
            retry_at,
            diagnostic_recheck_required: persisted.diagnostic_recheck_required,
        },
        active,
        backing_off,
    })
}

fn projected_scheduler_status(
    active_watch_count: u64,
    schedule: Option<&bcsp_operational_storage::OpenScheduleState>,
    in_flight: bool,
) -> Result<OpenSchedulerStatusV1, ()> {
    let next_due_at = schedule
        .map(|schedule| parse_timestamp(&schedule.next_due_at))
        .transpose()?;
    Ok(OpenSchedulerStatusV1 {
        lane: if active_watch_count > 0 {
            OpenSchedulerLane::ActiveWatch
        } else {
            OpenSchedulerLane::General
        },
        requested_general_interval_seconds: PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS,
        requested_effective_interval_seconds: if active_watch_count > 0 {
            PUBLIC_WATCHED_OPEN_INTERVAL_SECONDS
        } else {
            PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS
        },
        active_watch_count,
        next_due_at,
        in_flight,
        scheduler_lag_milliseconds: schedule.map_or(0, |value| value.schedule_lag_ms),
        actual_start_to_start_interval_milliseconds: None,
        failure_streak: schedule.map_or(0, |value| {
            u32::try_from(value.failure_streak).unwrap_or(u32::MAX)
        }),
    })
}

fn parse_timestamp(value: &str) -> Result<OffsetDateTime, ()> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|_| ())
}

const fn open_freshness_availability(state: OpenFreshnessState) -> (bool, bool) {
    match state {
        OpenFreshnessState::Fresh => (true, false),
        OpenFreshnessState::Stale => (true, true),
        OpenFreshnessState::Unknown => (false, false),
    }
}

impl PublicServiceStateSource for PublicServiceInspector {
    fn snapshot(&self) -> PublicServiceSnapshot {
        self.inspect()
            .unwrap_or_else(|_| self.unavailable_snapshot())
    }

    fn default_current_filters(&self) -> Option<FilterRequestV1> {
        let store = self.store.lock().ok()?;
        let target = store
            .storage()
            .discovered_targets()
            .ok()?
            .into_iter()
            .next()?;
        Some(FilterRequestV1::new(NormalizedFilterValuesV1::for_term(
            target.term().clone(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use bcsp_operational_storage::{OpenCircuitState, OpenOriginState};
    use bcsp_public_operations::PublicOperationalStore;
    use tempfile::TempDir;
    use time::Duration;

    use super::*;

    #[derive(Default)]
    struct NoDemand;

    impl PublicWatchDemandSource for NoDemand {
        fn connection_count(&self) -> u64 {
            0
        }

        fn total_active_watch_count(&self) -> u64 {
            0
        }

        fn active_watch_count(&self, _target: &TermCampusKey) -> u64 {
            0
        }
    }

    #[test]
    fn fresh_operational_schema_is_live_but_not_product_ready() {
        let temp = TempDir::new().expect("temporary directory");
        let store = PublicOperationalStore::open_for_state_root(temp.path().join("state"))
            .expect("public state");
        let now = OffsetDateTime::from_unix_timestamp(1_784_006_400).expect("fixed time");
        let inspector = PublicServiceInspector::new(
            Arc::new(Mutex::new(store)),
            Arc::new(InMemoryPublicSchedulerStatus::default()),
            Arc::new(NoDemand),
            Arc::new(move || now),
        );
        let snapshot = inspector.snapshot();
        assert_eq!(snapshot.status, PublicStatusLevel::NotReady);
        assert!(!snapshot.ready);
        assert_eq!(
            snapshot.reasons,
            vec![
                PublicReadinessReason::CatalogUnavailable,
                PublicReadinessReason::OpenUnavailable
            ]
        );
        assert_eq!(snapshot.today_counts, PublicDayCounters::default());
        assert_eq!(inspector.default_current_filters(), None);
    }

    #[test]
    fn scheduler_running_is_live_state_and_stops_explicitly() {
        let scheduler = InMemoryPublicSchedulerStatus::default();
        assert!(!scheduler.scheduler_running());
        scheduler.publish(PublicSchedulerSnapshot::default());
        assert!(scheduler.scheduler_running());
        scheduler.mark_stopped();
        assert!(!scheduler.scheduler_running());
    }

    #[test]
    fn persisted_retry_after_is_reported_as_backoff_and_origin_circuit() {
        let temp = TempDir::new().expect("temporary directory");
        let mut store = PublicOperationalStore::open_for_state_root(temp.path().join("state"))
            .expect("public state");
        let now = OffsetDateTime::from_unix_timestamp(1_784_006_400).expect("fixed time");
        let opened_at = now.format(&Rfc3339).expect("opened timestamp");
        let retry_at = (now + Duration::hours(1))
            .format(&Rfc3339)
            .expect("retry timestamp");
        store
            .storage_mut()
            .put_open_origin_state(&OpenOriginState {
                origin_id: PUBLIC_RUTGERS_ORIGIN_ID.to_owned(),
                circuit_state: OpenCircuitState::RetryAfter,
                reason_code: Some("UPSTREAM_BACKOFF".to_owned()),
                opened_at: Some(opened_at.clone()),
                retry_at: Some(retry_at),
                diagnostic_recheck_required: false,
                updated_at: opened_at,
            })
            .expect("persist origin backoff");
        let inspector = PublicServiceInspector::new(
            Arc::new(Mutex::new(store)),
            Arc::new(InMemoryPublicSchedulerStatus::default()),
            Arc::new(NoDemand),
            Arc::new(move || now),
        );

        let snapshot = inspector.snapshot();
        assert!(!snapshot.ready);
        assert!(snapshot.scheduler.origin_circuit_open);
        assert!(
            snapshot
                .reasons
                .contains(&PublicReadinessReason::BackingOff)
        );
        assert!(
            snapshot
                .reasons
                .contains(&PublicReadinessReason::OriginCircuitOpen)
        );
    }

    #[test]
    fn stale_lkg_remains_available_but_is_never_described_as_fresh() {
        assert_eq!(
            open_freshness_availability(OpenFreshnessState::Fresh),
            (true, false)
        );
        assert_eq!(
            open_freshness_availability(OpenFreshnessState::Stale),
            (true, true)
        );
        assert_eq!(
            open_freshness_availability(OpenFreshnessState::Unknown),
            (false, false)
        );
    }

    #[test]
    fn persisted_or_live_lag_has_an_explicit_degradation_reason() {
        let temp = TempDir::new().expect("temporary directory");
        let store = PublicOperationalStore::open_for_state_root(temp.path().join("state"))
            .expect("public state");
        let scheduler = Arc::new(InMemoryPublicSchedulerStatus::default());
        scheduler.publish(PublicSchedulerSnapshot {
            maximum_lag_milliseconds: 42,
            ..PublicSchedulerSnapshot::default()
        });
        let now = OffsetDateTime::from_unix_timestamp(1_784_006_400).expect("fixed time");
        let inspector = PublicServiceInspector::new(
            Arc::new(Mutex::new(store)),
            scheduler,
            Arc::new(NoDemand),
            Arc::new(move || now),
        );

        let snapshot = inspector.snapshot();
        assert!(
            snapshot
                .reasons
                .contains(&PublicReadinessReason::SchedulerLagging)
        );
    }
}
