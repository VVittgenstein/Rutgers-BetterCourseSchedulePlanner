use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::time::Duration;

use bcsp_contracts::TermCampusKey;
use thiserror::Error;

use crate::{OpenRefreshIntervals, RetryDirective, RetryMode};

const CATALOG_ONE_SHOT_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

#[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct MonotonicTime(u64);

impl MonotonicTime {
    pub const ZERO: Self = Self(0);

    pub const fn from_millis(value: u64) -> Self {
        Self(value)
    }

    pub const fn as_millis(self) -> u64 {
        self.0
    }

    pub fn saturating_add(self, duration: Duration) -> Self {
        let millis = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
        Self(self.0.saturating_add(millis))
    }

    pub const fn saturating_duration_since(self, earlier: Self) -> Duration {
        Duration::from_millis(self.0.saturating_sub(earlier.0))
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum OriginJobKind {
    Catalog,
    Open,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OriginJobKey {
    pub kind: OriginJobKind,
    pub target: TermCampusKey,
}

impl OriginJobKey {
    pub fn catalog(target: TermCampusKey) -> Self {
        Self {
            kind: OriginJobKind::Catalog,
            target,
        }
    }

    pub fn open(target: TermCampusKey) -> Self {
        Self {
            kind: OriginJobKind::Open,
            target,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OriginSchedulerLane {
    Catalog,
    OpenGeneral,
    OpenActiveWatch,
    OpenFirstLoad,
    OpenManualRefresh,
    OpenCatalogRaceRecheck,
}

impl OriginSchedulerLane {
    const fn watched_priority(self) -> bool {
        matches!(self, Self::OpenActiveWatch)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct JobSchedule {
    active: bool,
    interval: Duration,
    next_due: MonotonicTime,
    lane: OriginSchedulerLane,
    normal_lane: OriginSchedulerLane,
    active_watch_count: u64,
    recurring: bool,
    one_shot_pending: bool,
    generation_pending: bool,
    last_start: Option<MonotonicTime>,
    failure_streak: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JobRegistrationMode {
    Recurring,
    OneShot,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum OriginCircuit {
    #[default]
    Closed,
    RetryAfter {
        until: MonotonicTime,
    },
    FatalDiagnostic {
        job: OriginJobKey,
        recheck_after: MonotonicTime,
        authorized: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InFlight {
    token: u64,
    key: OriginJobKey,
    scheduled_due: MonotonicTime,
    started_at: MonotonicTime,
    diagnostic_probe: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginDispatch {
    pub token: u64,
    pub key: OriginJobKey,
    pub lane: OriginSchedulerLane,
    pub scheduled_due: MonotonicTime,
    pub started_at: MonotonicTime,
    pub scheduler_lag: Duration,
    pub actual_start_to_start_interval: Option<Duration>,
    pub requested_interval: Duration,
    pub failure_streak: u32,
    /// True only for the bounded, non-recurring observation that hydrates an
    /// otherwise idle target for the current discovery generation.
    pub first_load_one_shot: bool,
}

/// Read-only view of the next due workflow used by the process-wide bounded supervisor.
///
/// Calling this method never reserves the workflow. The owning coordinator must still call
/// [`OriginEdfScheduler::start_next`] immediately before execution; this hint exists so several
/// target-local schedulers can participate in one global priority queue.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OriginDispatchHint {
    pub key: OriginJobKey,
    pub lane: OriginSchedulerLane,
    pub scheduled_due: MonotonicTime,
    pub active_watch_count: u64,
    pub generation_pending: bool,
    pub failure_streak: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompletionSchedule {
    Success,
    CatalogRace,
    Retry(RetryDirective),
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SchedulerError {
    #[error("scheduler interval must be nonzero")]
    ZeroInterval,
    #[error("scheduler job is not registered")]
    UnknownJob,
    #[error("scheduler completion token does not match the in-flight job")]
    InvalidCompletionToken,
    #[error("scheduler completion precedes request start")]
    CompletionBeforeStart,
    #[error("fatal diagnostic circuit has not reached its minimum recheck time")]
    DiagnosticCooldownActive,
}

#[derive(Debug, Default)]
pub struct OriginEdfScheduler {
    jobs: BTreeMap<OriginJobKey, JobSchedule>,
    in_flight: Option<InFlight>,
    circuit: OriginCircuit,
    next_token: u64,
}

impl OriginEdfScheduler {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_catalog(
        &mut self,
        target: TermCampusKey,
        interval: Duration,
        first_due: MonotonicTime,
    ) -> Result<(), SchedulerError> {
        self.upsert(
            OriginJobKey::catalog(target),
            interval,
            first_due,
            OriginSchedulerLane::Catalog,
            0,
            JobRegistrationMode::Recurring,
            false,
        )
    }

    /// Registers the Catalog work required to hydrate one target in the
    /// current discovery generation. A watched target receives bootstrap
    /// priority until this Catalog succeeds, after which ordinary Catalog
    /// refreshes return to normal EDF ordering.
    pub fn register_catalog_initial(
        &mut self,
        target: TermCampusKey,
        interval: Duration,
        active_watch_count: u64,
        first_due: MonotonicTime,
    ) -> Result<(), SchedulerError> {
        self.upsert(
            OriginJobKey::catalog(target),
            interval,
            first_due,
            OriginSchedulerLane::Catalog,
            active_watch_count,
            JobRegistrationMode::Recurring,
            true,
        )
    }

    /// Registers one Catalog hydration attempt without enabling periodic refresh after success.
    /// Retries remain active until the one-shot succeeds; product demand may promote it through
    /// [`Self::activate_catalog`].
    pub fn register_catalog_one_shot(
        &mut self,
        target: TermCampusKey,
        active_watch_count: u64,
        first_due: MonotonicTime,
    ) -> Result<(), SchedulerError> {
        self.upsert(
            OriginJobKey::catalog(target),
            CATALOG_ONE_SHOT_INTERVAL,
            first_due,
            OriginSchedulerLane::Catalog,
            active_watch_count,
            JobRegistrationMode::OneShot,
            true,
        )
    }

    pub fn register_open<I>(
        &mut self,
        target: TermCampusKey,
        intervals: I,
        active_watch_count: u64,
        now: MonotonicTime,
        first_due: MonotonicTime,
    ) -> Result<(), SchedulerError>
    where
        I: Into<OpenRefreshIntervals>,
    {
        self.register_open_with_mode(
            target,
            intervals.into(),
            active_watch_count,
            now,
            first_due,
            true,
        )
    }

    /// Registers one mandatory Open observation without making an otherwise idle target recur.
    ///
    /// A later call to [`Self::activate_open`] promotes the same job to the normal recurring
    /// cadence. An active watch also keeps it scheduled after the one-shot observation.
    pub fn register_open_initial<I>(
        &mut self,
        target: TermCampusKey,
        intervals: I,
        active_watch_count: u64,
        now: MonotonicTime,
        first_due: MonotonicTime,
    ) -> Result<(), SchedulerError>
    where
        I: Into<OpenRefreshIntervals>,
    {
        let key = OriginJobKey::open(target.clone());
        self.register_open_with_mode(
            target,
            intervals.into(),
            active_watch_count,
            now,
            first_due,
            false,
        )?;
        if active_watch_count == 0 {
            self.jobs
                .get_mut(&key)
                .expect("the initial Open job was just registered")
                .lane = OriginSchedulerLane::OpenFirstLoad;
        }
        Ok(())
    }

    fn register_open_with_mode(
        &mut self,
        target: TermCampusKey,
        intervals: OpenRefreshIntervals,
        active_watch_count: u64,
        now: MonotonicTime,
        first_due: MonotonicTime,
        recurring: bool,
    ) -> Result<(), SchedulerError> {
        let watched = active_watch_count > 0;
        let normal_lane = if watched {
            OriginSchedulerLane::OpenActiveWatch
        } else {
            OriginSchedulerLane::OpenGeneral
        };
        let key = OriginJobKey::open(target);
        let existed = self.jobs.contains_key(&key);
        self.upsert(
            key.clone(),
            intervals.effective(watched),
            first_due,
            normal_lane,
            active_watch_count,
            if recurring {
                JobRegistrationMode::Recurring
            } else {
                JobRegistrationMode::OneShot
            },
            !recurring,
        )?;
        if existed {
            self.update_due_for_interval_change(&key, now);
        }
        Ok(())
    }

    fn upsert(
        &mut self,
        key: OriginJobKey,
        interval: Duration,
        first_due: MonotonicTime,
        normal_lane: OriginSchedulerLane,
        active_watch_count: u64,
        registration_mode: JobRegistrationMode,
        generation_pending: bool,
    ) -> Result<(), SchedulerError> {
        if interval.is_zero() {
            return Err(SchedulerError::ZeroInterval);
        }
        match self.jobs.get_mut(&key) {
            Some(schedule) => {
                schedule.active = true;
                schedule.interval = interval;
                schedule.normal_lane = normal_lane;
                schedule.lane = normal_lane;
                schedule.active_watch_count = active_watch_count;
                schedule.recurring |= registration_mode == JobRegistrationMode::Recurring;
                schedule.one_shot_pending |= registration_mode == JobRegistrationMode::OneShot;
                schedule.generation_pending |= generation_pending;
            }
            None => {
                self.jobs.insert(
                    key,
                    JobSchedule {
                        active: true,
                        interval,
                        next_due: first_due,
                        lane: normal_lane,
                        normal_lane,
                        active_watch_count,
                        recurring: registration_mode == JobRegistrationMode::Recurring,
                        one_shot_pending: registration_mode == JobRegistrationMode::OneShot,
                        generation_pending,
                        last_start: None,
                        failure_streak: 0,
                    },
                );
            }
        }
        Ok(())
    }

    pub fn set_catalog_watch_count(
        &mut self,
        target: &TermCampusKey,
        active_watch_count: u64,
    ) -> Result<(), SchedulerError> {
        let key = OriginJobKey::catalog(target.clone());
        let schedule = self.jobs.get_mut(&key).ok_or(SchedulerError::UnknownJob)?;
        schedule.active_watch_count = active_watch_count;
        Ok(())
    }

    /// Promotes a registered Catalog target to periodic product-demand cadence.
    ///
    /// Promotion never postpones already-due initial or retry work. A parked one-shot is made
    /// active and due no later than one normal interval from `now`.
    pub fn activate_catalog(
        &mut self,
        target: &TermCampusKey,
        interval: Duration,
        now: MonotonicTime,
    ) -> Result<(), SchedulerError> {
        if interval.is_zero() {
            return Err(SchedulerError::ZeroInterval);
        }
        let key = OriginJobKey::catalog(target.clone());
        let schedule = self.jobs.get_mut(&key).ok_or(SchedulerError::UnknownJob)?;
        schedule.active = true;
        schedule.recurring = true;
        schedule.interval = interval;
        schedule.normal_lane = OriginSchedulerLane::Catalog;
        schedule.lane = OriginSchedulerLane::Catalog;
        schedule.next_due = schedule.next_due.min(now.saturating_add(interval));
        Ok(())
    }

    pub fn demote_catalog(
        &mut self,
        target: &TermCampusKey,
        maintenance_interval: Option<Duration>,
        now: MonotonicTime,
    ) -> Result<(), SchedulerError> {
        let key = OriginJobKey::catalog(target.clone());
        let schedule = self.jobs.get_mut(&key).ok_or(SchedulerError::UnknownJob)?;
        schedule.normal_lane = OriginSchedulerLane::Catalog;
        schedule.lane = OriginSchedulerLane::Catalog;
        match maintenance_interval {
            Some(interval) if !interval.is_zero() => {
                schedule.interval = interval;
                schedule.recurring = true;
                schedule.active = true;
                schedule.next_due = schedule.last_start.map_or_else(
                    || now.saturating_add(interval),
                    |last| last.saturating_add(interval),
                );
            }
            Some(_) => return Err(SchedulerError::ZeroInterval),
            None => {
                schedule.recurring = false;
                schedule.active = schedule.generation_pending || schedule.one_shot_pending;
            }
        }
        Ok(())
    }

    /// Promotes a registered Open target to its recurring general/watch cadence exactly once.
    ///
    /// Promotion preserves the stored next due time, including retry/circuit cooldowns. If the
    /// target has been parked long enough for that due time to pass, it becomes immediately due
    /// without creating catch-up dispatches.
    pub fn activate_open<I>(
        &mut self,
        target: &TermCampusKey,
        intervals: I,
        now: MonotonicTime,
    ) -> Result<(), SchedulerError>
    where
        I: Into<OpenRefreshIntervals>,
    {
        let intervals = intervals.into();
        let key = OriginJobKey::open(target.clone());
        let schedule = self.jobs.get_mut(&key).ok_or(SchedulerError::UnknownJob)?;
        let watched = schedule.active_watch_count > 0;
        let was_active = schedule.active;
        schedule.active = true;
        schedule.recurring = true;
        schedule.interval = intervals.effective(watched);
        schedule.normal_lane = if watched {
            OriginSchedulerLane::OpenActiveWatch
        } else {
            OriginSchedulerLane::OpenGeneral
        };
        if !schedule.one_shot_pending || !was_active {
            schedule.lane = schedule.normal_lane;
        }
        if !was_active && schedule.next_due < now {
            schedule.next_due = now;
        }
        Ok(())
    }

    pub fn demote_open<I>(
        &mut self,
        target: &TermCampusKey,
        intervals: I,
        active_watch_count: u64,
        now: MonotonicTime,
    ) -> Result<(), SchedulerError>
    where
        I: Into<OpenRefreshIntervals>,
    {
        let intervals = intervals.into();
        let key = OriginJobKey::open(target.clone());
        let schedule = self.jobs.get_mut(&key).ok_or(SchedulerError::UnknownJob)?;
        schedule.active_watch_count = active_watch_count;
        schedule.interval = intervals.effective(active_watch_count > 0);
        if active_watch_count > 0 {
            schedule.active = true;
            schedule.recurring = true;
            schedule.normal_lane = OriginSchedulerLane::OpenActiveWatch;
            schedule.lane = OriginSchedulerLane::OpenActiveWatch;
            if schedule.next_due < now {
                schedule.next_due = now;
            }
        } else {
            schedule.recurring = false;
            schedule.active = schedule.one_shot_pending;
            schedule.normal_lane = OriginSchedulerLane::OpenGeneral;
            if schedule.one_shot_pending {
                schedule.lane = OriginSchedulerLane::OpenFirstLoad;
            }
        }
        Ok(())
    }

    fn update_due_for_interval_change(&mut self, key: &OriginJobKey, now: MonotonicTime) {
        let Some(schedule) = self.jobs.get_mut(key) else {
            return;
        };
        let anchored = schedule
            .last_start
            .unwrap_or(now)
            .saturating_add(schedule.interval);
        if schedule.normal_lane.watched_priority() {
            schedule.next_due = schedule.next_due.min(anchored);
        } else if schedule.last_start.is_some() {
            schedule.next_due = anchored;
        }
    }

    pub fn set_open_watch_count<I>(
        &mut self,
        target: &TermCampusKey,
        intervals: I,
        active_watch_count: u64,
        now: MonotonicTime,
    ) -> Result<(), SchedulerError>
    where
        I: Into<OpenRefreshIntervals>,
    {
        let intervals = intervals.into();
        let key = OriginJobKey::open(target.clone());
        let schedule = self.jobs.get_mut(&key).ok_or(SchedulerError::UnknownJob)?;
        let previously_watched = schedule.active_watch_count > 0;
        let was_active = schedule.active;
        let watched = active_watch_count > 0;
        schedule.active_watch_count = active_watch_count;
        schedule.interval = intervals.effective(watched);
        schedule.normal_lane = if watched {
            OriginSchedulerLane::OpenActiveWatch
        } else {
            OriginSchedulerLane::OpenGeneral
        };
        if watched || !schedule.one_shot_pending {
            schedule.lane = schedule.normal_lane;
        }
        let anchored = schedule
            .last_start
            .unwrap_or(now)
            .saturating_add(schedule.interval);
        if watched && !previously_watched {
            schedule.active = true;
            schedule.next_due = schedule.next_due.min(anchored);
            if !was_active && schedule.next_due < now {
                schedule.next_due = now;
            }
        } else if !watched && previously_watched {
            schedule.next_due = anchored;
            schedule.active = schedule.recurring || schedule.one_shot_pending;
        }
        Ok(())
    }

    pub fn request_open_due(
        &mut self,
        target: &TermCampusKey,
        due: MonotonicTime,
        lane: OriginSchedulerLane,
    ) -> Result<(), SchedulerError> {
        let key = OriginJobKey::open(target.clone());
        let schedule = self.jobs.get_mut(&key).ok_or(SchedulerError::UnknownJob)?;
        if self
            .in_flight
            .as_ref()
            .is_some_and(|in_flight| in_flight.key == key)
        {
            return Ok(());
        }
        let was_active = schedule.active;
        schedule.active = true;
        if !schedule.recurring {
            schedule.one_shot_pending = true;
        }
        schedule.next_due = if was_active {
            schedule.next_due.min(due)
        } else {
            due
        };
        schedule.lane = if schedule.active_watch_count > 0 {
            OriginSchedulerLane::OpenActiveWatch
        } else {
            lane
        };
        Ok(())
    }

    /// Completes the registered initial Open observation when the Catalog workflow already
    /// fetched and atomically committed the matching Open snapshot.
    ///
    /// This is the scheduler-side equivalent of a successful initial Open dispatch. It prevents
    /// a second redundant Open request immediately after a complete snapshot publication.
    pub fn complete_open_bootstrap(
        &mut self,
        target: &TermCampusKey,
        completed_at: MonotonicTime,
    ) -> Result<(), SchedulerError> {
        let key = OriginJobKey::open(target.clone());
        let schedule = self.jobs.get_mut(&key).ok_or(SchedulerError::UnknownJob)?;
        schedule.failure_streak = 0;
        schedule.lane = schedule.normal_lane;
        schedule.generation_pending = false;
        schedule.one_shot_pending = false;
        schedule.next_due = completed_at.saturating_add(schedule.interval);
        schedule.active = schedule.recurring || schedule.active_watch_count > 0;
        Ok(())
    }

    /// Coalesces an Open workflow executed concurrently by the process-wide supervisor while this
    /// target's Catalog workflow was in flight. The external workflow already persisted its Open
    /// attempt; this only advances the target-local schedule so it is not immediately duplicated.
    pub fn adopt_external_open_state(
        &mut self,
        target: &TermCampusKey,
        now: MonotonicTime,
        next_due: MonotonicTime,
        failure_streak: u32,
    ) -> Result<(), SchedulerError> {
        let key = OriginJobKey::open(target.clone());
        let schedule = self.jobs.get_mut(&key).ok_or(SchedulerError::UnknownJob)?;
        schedule.failure_streak = failure_streak;
        schedule.last_start = Some(now);
        schedule.next_due = next_due;
        schedule.lane = schedule.normal_lane;
        schedule.one_shot_pending = false;
        schedule.generation_pending = false;
        schedule.active = schedule.recurring || schedule.active_watch_count > 0;
        Ok(())
    }

    pub fn start_next(&mut self, now: MonotonicTime) -> Option<OriginDispatch> {
        if self.in_flight.is_some() {
            return None;
        }
        let diagnostic_job = match &self.circuit {
            OriginCircuit::Closed => None,
            OriginCircuit::RetryAfter { until } if now >= *until => {
                self.circuit = OriginCircuit::Closed;
                None
            }
            OriginCircuit::RetryAfter { .. } => return None,
            OriginCircuit::FatalDiagnostic {
                job,
                recheck_after,
                authorized: true,
            } if now >= *recheck_after => Some(job.clone()),
            OriginCircuit::FatalDiagnostic { .. } => return None,
        };
        let diagnostic_probe = diagnostic_job.is_some();
        let (key, schedule) = if let Some(key) = diagnostic_job {
            let schedule = self.jobs.get(&key)?.clone();
            if schedule.next_due > now {
                return None;
            }
            (key, schedule)
        } else {
            let (key, schedule) = self
                .jobs
                .iter()
                .filter(|(_, schedule)| schedule.active && schedule.next_due <= now)
                .min_by(|(left_key, left), (right_key, right)| {
                    compare_due_jobs(left_key, left, right_key, right)
                })?;
            (key.clone(), schedule.clone())
        };
        self.next_token = self.next_token.saturating_add(1).max(1);
        let token = self.next_token;
        let actual_start_to_start_interval = schedule
            .last_start
            .map(|last| now.saturating_duration_since(last));
        self.jobs
            .get_mut(&key)
            .expect("selected scheduler job remains registered")
            .last_start = Some(now);
        self.in_flight = Some(InFlight {
            token,
            key: key.clone(),
            scheduled_due: schedule.next_due,
            started_at: now,
            diagnostic_probe,
        });
        if diagnostic_probe
            && let OriginCircuit::FatalDiagnostic { authorized, .. } = &mut self.circuit
        {
            *authorized = false;
        }
        let first_load_one_shot =
            key.kind == OriginJobKind::Open && schedule.one_shot_pending && !schedule.recurring;
        Some(OriginDispatch {
            token,
            key,
            lane: schedule.lane,
            scheduled_due: schedule.next_due,
            started_at: now,
            scheduler_lag: now.saturating_duration_since(schedule.next_due),
            actual_start_to_start_interval,
            requested_interval: schedule.interval,
            failure_streak: schedule.failure_streak,
            first_load_one_shot,
        })
    }

    pub fn next_dispatch_hint(&self, now: MonotonicTime) -> Option<OriginDispatchHint> {
        if self.in_flight.is_some() {
            return None;
        }
        let diagnostic_job = match &self.circuit {
            OriginCircuit::Closed => None,
            OriginCircuit::RetryAfter { until } if now >= *until => None,
            OriginCircuit::RetryAfter { .. } => return None,
            OriginCircuit::FatalDiagnostic {
                job,
                recheck_after,
                authorized: true,
            } if now >= *recheck_after => Some(job.clone()),
            OriginCircuit::FatalDiagnostic { .. } => return None,
        };
        let (key, schedule) = if let Some(key) = diagnostic_job {
            let schedule = self.jobs.get(&key)?;
            if schedule.next_due > now {
                return None;
            }
            (key, schedule)
        } else {
            let (key, schedule) = self
                .jobs
                .iter()
                .filter(|(_, schedule)| schedule.active && schedule.next_due <= now)
                .min_by(|(left_key, left), (right_key, right)| {
                    compare_due_jobs(left_key, left, right_key, right)
                })?;
            (key.clone(), schedule)
        };
        Some(OriginDispatchHint {
            key,
            lane: schedule.lane,
            scheduled_due: schedule.next_due,
            active_watch_count: schedule.active_watch_count,
            generation_pending: schedule.generation_pending,
            failure_streak: schedule.failure_streak,
        })
    }

    pub fn finish(
        &mut self,
        token: u64,
        completed_at: MonotonicTime,
        completion: CompletionSchedule,
    ) -> Result<(), SchedulerError> {
        let in_flight = self
            .in_flight
            .take()
            .ok_or(SchedulerError::InvalidCompletionToken)?;
        if in_flight.token != token {
            self.in_flight = Some(in_flight);
            return Err(SchedulerError::InvalidCompletionToken);
        }
        if completed_at < in_flight.started_at {
            self.in_flight = Some(in_flight);
            return Err(SchedulerError::CompletionBeforeStart);
        }
        let schedule = self
            .jobs
            .get_mut(&in_flight.key)
            .ok_or(SchedulerError::UnknownJob)?;
        schedule.lane = schedule.normal_lane;
        match completion {
            CompletionSchedule::Success => {
                schedule.failure_streak = 0;
                schedule.next_due =
                    first_future_due(in_flight.scheduled_due, schedule.interval, completed_at);
                if in_flight.diagnostic_probe {
                    self.circuit = OriginCircuit::Closed;
                }
            }
            CompletionSchedule::CatalogRace => {
                schedule.failure_streak = 0;
                schedule.next_due =
                    first_future_due(in_flight.scheduled_due, schedule.interval, completed_at);
                if in_flight.diagnostic_probe {
                    self.circuit = OriginCircuit::Closed;
                }
            }
            CompletionSchedule::Retry(directive) => {
                schedule.failure_streak = directive.next_failure_streak;
                schedule.next_due = completed_at.saturating_add(directive.delay);
                match directive.mode {
                    RetryMode::Automatic => {}
                    RetryMode::OriginCircuit => {
                        self.circuit = OriginCircuit::RetryAfter {
                            until: schedule.next_due,
                        };
                    }
                    RetryMode::ExplicitDiagnosticRecheck => {
                        self.circuit = OriginCircuit::FatalDiagnostic {
                            job: in_flight.key.clone(),
                            recheck_after: schedule.next_due,
                            authorized: false,
                        };
                    }
                }
                if in_flight.diagnostic_probe
                    && directive.mode == RetryMode::Automatic
                    && directive.clears_fatal_diagnostic
                {
                    self.circuit = OriginCircuit::Closed;
                } else if in_flight.diagnostic_probe
                    && directive.mode == RetryMode::Automatic
                    && !directive.clears_fatal_diagnostic
                {
                    self.circuit = OriginCircuit::FatalDiagnostic {
                        job: in_flight.key.clone(),
                        recheck_after: schedule.next_due,
                        authorized: false,
                    };
                }
            }
        }
        if completion == CompletionSchedule::Success && schedule.one_shot_pending {
            schedule.one_shot_pending = false;
            schedule.active = schedule.recurring
                || (in_flight.key.kind == OriginJobKind::Open && schedule.active_watch_count > 0);
        }
        if completion == CompletionSchedule::Success {
            schedule.generation_pending = false;
        }
        Ok(())
    }

    pub fn authorize_diagnostic_recheck(
        &mut self,
        now: MonotonicTime,
    ) -> Result<(), SchedulerError> {
        match &mut self.circuit {
            OriginCircuit::FatalDiagnostic {
                recheck_after,
                authorized,
                ..
            } if now >= *recheck_after => {
                *authorized = true;
                Ok(())
            }
            OriginCircuit::FatalDiagnostic { .. } => Err(SchedulerError::DiagnosticCooldownActive),
            OriginCircuit::Closed | OriginCircuit::RetryAfter { .. } => Ok(()),
        }
    }

    pub fn circuit(&self) -> OriginCircuit {
        self.circuit.clone()
    }

    pub fn in_flight(&self) -> Option<&OriginJobKey> {
        self.in_flight.as_ref().map(|value| &value.key)
    }

    pub fn next_due(&self, key: &OriginJobKey) -> Option<MonotonicTime> {
        self.jobs
            .get(key)
            .filter(|schedule| schedule.active)
            .map(|schedule| schedule.next_due)
    }

    pub fn is_active(&self, key: &OriginJobKey) -> bool {
        self.jobs.get(key).is_some_and(|schedule| schedule.active)
    }

    pub fn active_watch_count(&self, target: &TermCampusKey) -> Option<u64> {
        self.jobs
            .get(&OriginJobKey::open(target.clone()))
            .map(|schedule| schedule.active_watch_count)
    }

    pub fn failure_streak(&self, key: &OriginJobKey) -> Option<u32> {
        self.jobs.get(key).map(|schedule| schedule.failure_streak)
    }
}

fn compare_due_jobs(
    left_key: &OriginJobKey,
    left: &JobSchedule,
    right_key: &OriginJobKey,
    right: &JobSchedule,
) -> Ordering {
    let dispatch_priority =
        dispatch_priority(left_key, left).cmp(&dispatch_priority(right_key, right));
    if dispatch_priority != Ordering::Equal {
        return dispatch_priority;
    }
    left.next_due
        .cmp(&right.next_due)
        .then_with(|| left_key.cmp(right_key))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum DispatchPriority {
    ActiveWatchOpen,
    WatchedCatalogPrerequisite,
    Generation,
    Recovery,
    Ordinary,
}

fn dispatch_priority(key: &OriginJobKey, schedule: &JobSchedule) -> DispatchPriority {
    if schedule.active_watch_count > 0
        && key.kind == OriginJobKind::Catalog
        && schedule.generation_pending
    {
        DispatchPriority::WatchedCatalogPrerequisite
    } else if schedule.active_watch_count > 0 && key.kind == OriginJobKind::Open {
        DispatchPriority::ActiveWatchOpen
    } else if schedule.generation_pending {
        DispatchPriority::Generation
    } else if schedule.failure_streak > 0 {
        DispatchPriority::Recovery
    } else {
        DispatchPriority::Ordinary
    }
}

fn first_future_due(
    scheduled_due: MonotonicTime,
    interval: Duration,
    completed_at: MonotonicTime,
) -> MonotonicTime {
    if scheduled_due > completed_at {
        return scheduled_due;
    }
    let interval_millis = u64::try_from(interval.as_millis())
        .unwrap_or(u64::MAX)
        .max(1);
    let elapsed = completed_at
        .as_millis()
        .saturating_sub(scheduled_due.as_millis());
    let steps = elapsed / interval_millis + 1;
    MonotonicTime::from_millis(
        scheduled_due
            .as_millis()
            .saturating_add(steps.saturating_mul(interval_millis)),
    )
}

pub fn restore_monotonic_due(
    now_monotonic: MonotonicTime,
    now_wall_millis: i128,
    persisted_due_wall_millis: i128,
) -> MonotonicTime {
    let remaining = persisted_due_wall_millis
        .saturating_sub(now_wall_millis)
        .max(0);
    let remaining = u64::try_from(remaining).unwrap_or(u64::MAX);
    MonotonicTime::from_millis(now_monotonic.as_millis().saturating_add(remaining))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::{
        GeneralOpenInterval, OpenFailureKind, OpenRefreshIntervals, WatchOpenInterval,
        retry_directive,
    };

    use super::*;

    fn target(campus: &str) -> TermCampusKey {
        TermCampusKey::try_new("92026", campus).expect("synthetic target")
    }

    fn seconds(value: u64) -> MonotonicTime {
        MonotonicTime::from_millis(value * 1_000)
    }

    fn legacy_fatal_directive(current_failure_streak: u32) -> RetryDirective {
        RetryDirective {
            delay: Duration::from_secs(60),
            mode: RetryMode::ExplicitDiagnosticRecheck,
            next_failure_streak: current_failure_streak.saturating_add(1),
            clears_fatal_diagnostic: false,
        }
    }

    #[test]
    fn active_watch_preempts_older_ordinary_due_work() {
        let mut scheduler = OriginEdfScheduler::new();
        scheduler
            .register_catalog(target("CAT"), Duration::from_secs(600), seconds(4))
            .expect("catalog");
        scheduler
            .register_open(
                target("WATCH"),
                GeneralOpenInterval::local(30).expect("interval"),
                1,
                MonotonicTime::ZERO,
                seconds(5),
            )
            .expect("watch");
        let first = scheduler.start_next(seconds(5)).expect("first due");
        assert_eq!(first.lane, OriginSchedulerLane::OpenActiveWatch);
    }

    #[test]
    fn equal_priority_work_remains_earliest_deadline_first() {
        let mut scheduler = OriginEdfScheduler::new();
        let earlier = target("EARLIER");
        scheduler
            .register_catalog(target("LATER"), Duration::from_secs(600), seconds(5))
            .expect("later Catalog");
        scheduler
            .register_catalog(earlier.clone(), Duration::from_secs(600), seconds(4))
            .expect("earlier Catalog");

        assert_eq!(
            scheduler.start_next(seconds(5)).expect("first due").key,
            OriginJobKey::catalog(earlier)
        );
    }

    #[test]
    fn overdue_active_watch_preempts_the_bounded_first_load_backlog() {
        let mut scheduler = OriginEdfScheduler::new();
        scheduler
            .register_open_initial(
                target("FIRST_LOAD"),
                GeneralOpenInterval::local(30).expect("interval"),
                0,
                MonotonicTime::ZERO,
                MonotonicTime::ZERO,
            )
            .expect("first-load job");
        scheduler
            .register_open(
                target("WATCH"),
                GeneralOpenInterval::local(30).expect("interval"),
                1,
                MonotonicTime::ZERO,
                seconds(10),
            )
            .expect("active-watch job");

        let dispatch = scheduler.start_next(seconds(20)).expect("due dispatch");

        assert_eq!(dispatch.lane, OriginSchedulerLane::OpenActiveWatch);
        assert_eq!(dispatch.requested_interval, Duration::from_secs(10));
        assert!(!dispatch.first_load_one_shot);
    }

    #[test]
    fn recovery_waits_until_due_then_follows_watch_and_preempts_ordinary_work() {
        let mut scheduler = OriginEdfScheduler::new();
        let recovery = target("RECOVERY");
        let recovery_key = OriginJobKey::catalog(recovery.clone());
        scheduler
            .register_catalog(
                recovery.clone(),
                Duration::from_secs(600),
                MonotonicTime::ZERO,
            )
            .expect("recovery Catalog");
        let failed = scheduler
            .start_next(MonotonicTime::ZERO)
            .expect("failed Catalog dispatch");
        let directive = retry_directive(
            &recovery,
            Duration::from_secs(30),
            failed.failure_streak,
            OpenFailureKind::Transient,
        );
        scheduler
            .finish(
                failed.token,
                MonotonicTime::ZERO,
                CompletionSchedule::Retry(directive),
            )
            .expect("schedule recovery");
        let retry_due = scheduler.next_due(&recovery_key).expect("retry due");

        let ordinary_now = target("ORDINARY_NOW");
        scheduler
            .register_catalog(
                ordinary_now.clone(),
                Duration::from_secs(600),
                MonotonicTime::ZERO,
            )
            .expect("ordinary current work");
        let ordinary = scheduler
            .start_next(MonotonicTime::ZERO)
            .expect("future recovery must not block due work");
        assert_eq!(ordinary.key, OriginJobKey::catalog(ordinary_now));
        scheduler
            .finish(
                ordinary.token,
                MonotonicTime::ZERO,
                CompletionSchedule::Success,
            )
            .expect("finish ordinary work");

        let overdue_ordinary = target("ORDINARY_OVERDUE");
        scheduler
            .register_catalog(
                overdue_ordinary.clone(),
                Duration::from_secs(600),
                MonotonicTime::ZERO,
            )
            .expect("overdue ordinary work");
        scheduler
            .register_open(
                target("WATCH"),
                GeneralOpenInterval::local(30).expect("interval"),
                1,
                MonotonicTime::ZERO,
                retry_due,
            )
            .expect("active watch");

        let watched = scheduler
            .start_next(retry_due)
            .expect("active watch stays highest priority");
        assert_eq!(watched.lane, OriginSchedulerLane::OpenActiveWatch);
        scheduler
            .finish(watched.token, retry_due, CompletionSchedule::Success)
            .expect("finish watch");

        let recovered = scheduler
            .start_next(retry_due)
            .expect("due recovery preempts ordinary cadence");
        assert_eq!(recovered.key, recovery_key);
        scheduler
            .finish(recovered.token, retry_due, CompletionSchedule::Success)
            .expect("finish recovery");
        assert_eq!(
            scheduler
                .start_next(retry_due)
                .expect("ordinary work resumes")
                .key,
            OriginJobKey::catalog(overdue_ordinary)
        );
    }

    #[test]
    fn active_watch_open_preempts_its_catalog_prerequisite_and_generation_backlog() {
        let mut scheduler = OriginEdfScheduler::new();
        let ordinary = target("AAA");
        let watched = target("ZZZ");
        let general = GeneralOpenInterval::local(30).expect("interval");
        scheduler
            .register_catalog_initial(
                ordinary.clone(),
                Duration::from_secs(600),
                0,
                MonotonicTime::ZERO,
            )
            .expect("ordinary Catalog");
        scheduler
            .register_open_initial(
                ordinary.clone(),
                general,
                0,
                MonotonicTime::ZERO,
                MonotonicTime::from_millis(1),
            )
            .expect("ordinary Open");
        scheduler
            .register_catalog_initial(watched.clone(), Duration::from_secs(600), 1, seconds(5))
            .expect("watched Catalog");
        scheduler
            .register_open_initial(watched.clone(), general, 1, MonotonicTime::ZERO, seconds(5))
            .expect("watched Open");

        let watched_open = scheduler
            .start_next(seconds(20))
            .expect("active-watch Open remains the first origin workflow");
        assert_eq!(watched_open.key, OriginJobKey::open(watched.clone()));
        assert_eq!(watched_open.lane, OriginSchedulerLane::OpenActiveWatch);
        scheduler
            .finish(watched_open.token, seconds(20), CompletionSchedule::Success)
            .expect("watched Open success");

        let watched_catalog = scheduler
            .start_next(seconds(20))
            .expect("watched Catalog follows before the remaining generation");
        assert_eq!(watched_catalog.key, OriginJobKey::catalog(watched));
        scheduler
            .finish(
                watched_catalog.token,
                seconds(20),
                CompletionSchedule::Success,
            )
            .expect("watched Catalog success");

        let ordinary_catalog = scheduler
            .start_next(seconds(20))
            .expect("ordinary generation resumes after the watched bootstrap");
        assert_eq!(
            ordinary_catalog.key,
            OriginJobKey::catalog(ordinary.clone())
        );
        scheduler
            .finish(
                ordinary_catalog.token,
                seconds(20),
                CompletionSchedule::Success,
            )
            .expect("ordinary Catalog success");
        scheduler
            .request_open_due(&ordinary, seconds(20), OriginSchedulerLane::OpenFirstLoad)
            .expect("Catalog publishes the ordinary Open");
        let ordinary_open = scheduler
            .start_next(seconds(20))
            .expect("ordinary first-load Open");
        assert_eq!(ordinary_open.key, OriginJobKey::open(ordinary));
        assert_eq!(ordinary_open.lane, OriginSchedulerLane::OpenFirstLoad);
        assert_eq!(ordinary_open.requested_interval, Duration::from_secs(30));
    }

    #[test]
    fn shared_origin_and_same_target_are_never_double_dispatched() {
        let mut scheduler = OriginEdfScheduler::new();
        let scope = target("NB");
        scheduler
            .register_open(
                scope.clone(),
                GeneralOpenInterval::local(3).expect("interval"),
                0,
                MonotonicTime::ZERO,
                MonotonicTime::ZERO,
            )
            .expect("register");
        let first = scheduler.start_next(MonotonicTime::ZERO).expect("dispatch");
        assert!(scheduler.start_next(seconds(1)).is_none());
        scheduler
            .request_open_due(&scope, seconds(1), OriginSchedulerLane::OpenManualRefresh)
            .expect("coalesce");
        scheduler
            .finish(first.token, seconds(2), CompletionSchedule::Success)
            .expect("finish");
        assert!(scheduler.start_next(seconds(2)).is_none());
    }

    #[test]
    fn missed_ticks_advance_to_first_future_due_without_catch_up() {
        let mut scheduler = OriginEdfScheduler::new();
        let scope = target("NB");
        let key = OriginJobKey::open(scope.clone());
        scheduler
            .register_open(
                scope,
                GeneralOpenInterval::local(3).expect("interval"),
                0,
                MonotonicTime::ZERO,
                MonotonicTime::ZERO,
            )
            .expect("register");
        let dispatch = scheduler.start_next(MonotonicTime::ZERO).expect("dispatch");
        scheduler
            .finish(dispatch.token, seconds(10), CompletionSchedule::Success)
            .expect("finish");
        assert_eq!(scheduler.next_due(&key), Some(seconds(12)));
        assert!(scheduler.start_next(seconds(10)).is_none());
    }

    #[test]
    fn initial_open_parks_after_success_and_demand_reactivates_normal_cadence() {
        let mut scheduler = OriginEdfScheduler::new();
        let scope = target("NB");
        let key = OriginJobKey::open(scope.clone());
        let general = GeneralOpenInterval::local(30).expect("interval");
        scheduler
            .register_open_initial(
                scope.clone(),
                general,
                0,
                MonotonicTime::ZERO,
                MonotonicTime::ZERO,
            )
            .expect("initial registration");

        let initial = scheduler
            .start_next(MonotonicTime::ZERO)
            .expect("initial dispatch");
        scheduler
            .finish(
                initial.token,
                MonotonicTime::ZERO,
                CompletionSchedule::Success,
            )
            .expect("initial success");
        assert_eq!(scheduler.next_due(&key), None);
        assert!(scheduler.start_next(seconds(90)).is_none());

        scheduler
            .activate_open(&scope, general, seconds(90))
            .expect("product demand");
        let demanded = scheduler
            .start_next(seconds(90))
            .expect("stale parked target is immediately due");
        assert_eq!(demanded.lane, OriginSchedulerLane::OpenGeneral);
        scheduler
            .finish(demanded.token, seconds(90), CompletionSchedule::Success)
            .expect("recurring success");
        assert_eq!(scheduler.next_due(&key), Some(seconds(120)));
    }

    #[test]
    fn configurable_watch_interval_drives_registration_and_watch_transitions() {
        let mut scheduler = OriginEdfScheduler::new();
        let scope = target("NB");
        let intervals = OpenRefreshIntervals::new(
            GeneralOpenInterval::local(30).expect("general interval"),
            WatchOpenInterval::local(7).expect("watch interval"),
        );
        scheduler
            .register_open(
                scope.clone(),
                intervals,
                1,
                MonotonicTime::ZERO,
                MonotonicTime::ZERO,
            )
            .expect("watched registration");
        let watched = scheduler
            .start_next(MonotonicTime::ZERO)
            .expect("watched dispatch");
        assert_eq!(watched.requested_interval, Duration::from_secs(7));
        scheduler
            .finish(
                watched.token,
                MonotonicTime::ZERO,
                CompletionSchedule::Success,
            )
            .expect("watched success");

        scheduler
            .set_open_watch_count(&scope, intervals, 0, MonotonicTime::ZERO)
            .expect("watch demotion");
        assert_eq!(
            scheduler.next_due(&OriginJobKey::open(scope.clone())),
            Some(seconds(30))
        );
        scheduler
            .set_open_watch_count(&scope, intervals, 1, seconds(5))
            .expect("watch promotion");
        assert_eq!(
            scheduler.next_due(&OriginJobKey::open(scope)),
            Some(seconds(7))
        );
    }

    #[test]
    fn catalog_one_shot_parks_after_success_and_activation_uses_normal_cadence() {
        let mut scheduler = OriginEdfScheduler::new();
        let scope = target("NB");
        let key = OriginJobKey::catalog(scope.clone());
        scheduler
            .register_catalog_one_shot(scope.clone(), 0, MonotonicTime::ZERO)
            .expect("one-shot Catalog");
        let initial = scheduler
            .start_next(MonotonicTime::ZERO)
            .expect("initial Catalog dispatch");
        assert!(!initial.first_load_one_shot);
        scheduler
            .finish(
                initial.token,
                MonotonicTime::ZERO,
                CompletionSchedule::Success,
            )
            .expect("initial Catalog success");
        assert!(!scheduler.is_active(&key));

        scheduler
            .activate_catalog(&scope, Duration::from_secs(600), seconds(100))
            .expect("Catalog product demand");
        assert_eq!(scheduler.next_due(&key), Some(seconds(700)));
        assert!(scheduler.start_next(seconds(699)).is_none());
        let recurring = scheduler
            .start_next(seconds(700))
            .expect("activated Catalog dispatch");
        assert_eq!(recurring.requested_interval, Duration::from_secs(600));
    }

    #[test]
    fn catalog_activation_never_postpones_existing_initial_or_retry_due() {
        let mut scheduler = OriginEdfScheduler::new();
        let scope = target("NB");
        let key = OriginJobKey::catalog(scope.clone());
        scheduler
            .register_catalog_initial(scope.clone(), Duration::from_secs(86_400), 0, seconds(20))
            .expect("low-frequency Catalog");
        scheduler
            .activate_catalog(&scope, Duration::from_secs(600), seconds(10))
            .expect("Catalog demand");
        assert_eq!(scheduler.next_due(&key), Some(seconds(20)));
    }

    #[test]
    fn complete_snapshot_open_bootstrap_prevents_a_duplicate_initial_request() {
        let mut scheduler = OriginEdfScheduler::new();
        let scope = target("NB");
        let key = OriginJobKey::open(scope.clone());
        let intervals = OpenRefreshIntervals::new(
            GeneralOpenInterval::local(30).expect("general interval"),
            WatchOpenInterval::local(7).expect("watch interval"),
        );
        scheduler
            .register_open_initial(scope.clone(), intervals, 0, MonotonicTime::ZERO, seconds(1))
            .expect("initial Open registration");
        scheduler
            .complete_open_bootstrap(&scope, seconds(5))
            .expect("Catalog workflow committed matching Open");
        assert!(!scheduler.is_active(&key));
        assert!(scheduler.start_next(seconds(5)).is_none());

        scheduler
            .activate_open(&scope, intervals, seconds(20))
            .expect("later product demand");
        assert_eq!(scheduler.next_due(&key), Some(seconds(35)));
    }

    #[test]
    fn complete_snapshot_open_bootstrap_keeps_recurring_or_watched_targets_scheduled() {
        let intervals = OpenRefreshIntervals::new(
            GeneralOpenInterval::local(30).expect("general interval"),
            WatchOpenInterval::local(7).expect("watch interval"),
        );

        let mut recurring = OriginEdfScheduler::new();
        let recurring_target = target("NB");
        recurring
            .register_open(
                recurring_target.clone(),
                intervals,
                0,
                MonotonicTime::ZERO,
                seconds(1),
            )
            .expect("recurring Open registration");
        recurring
            .complete_open_bootstrap(&recurring_target, seconds(5))
            .expect("complete recurring bootstrap");
        assert_eq!(
            recurring.next_due(&OriginJobKey::open(recurring_target)),
            Some(seconds(35))
        );

        let mut watched = OriginEdfScheduler::new();
        let watched_target = target("NK");
        watched
            .register_open_initial(
                watched_target.clone(),
                intervals,
                1,
                MonotonicTime::ZERO,
                seconds(1),
            )
            .expect("watched initial registration");
        watched
            .complete_open_bootstrap(&watched_target, seconds(5))
            .expect("complete watched bootstrap");
        assert_eq!(
            watched.next_due(&OriginJobKey::open(watched_target)),
            Some(seconds(12))
        );
    }

    #[test]
    fn externally_executed_open_coalesces_the_target_schedule_without_a_duplicate_dispatch() {
        let mut scheduler = OriginEdfScheduler::new();
        let scope = target("NB");
        let key = OriginJobKey::open(scope.clone());
        scheduler
            .register_open(
                scope.clone(),
                GeneralOpenInterval::local(30).expect("interval"),
                1,
                MonotonicTime::ZERO,
                MonotonicTime::ZERO,
            )
            .expect("register Open");
        scheduler
            .adopt_external_open_state(&scope, seconds(5), seconds(15), 3)
            .expect("adopt external state");

        assert_eq!(scheduler.failure_streak(&key), Some(3));
        assert_eq!(scheduler.next_due(&key), Some(seconds(15)));
        assert!(scheduler.start_next(seconds(14)).is_none());
        assert_eq!(
            scheduler
                .start_next(seconds(15))
                .expect("coalesced due Open")
                .lane,
            OriginSchedulerLane::OpenActiveWatch
        );
    }

    #[test]
    fn initial_sweep_is_bounded_to_one_success_per_idle_target() {
        let mut scheduler = OriginEdfScheduler::new();
        let general = GeneralOpenInterval::local(30).expect("interval");
        for campus in 0..15 {
            scheduler
                .register_open_initial(
                    target(&format!("C{campus:02}")),
                    general,
                    0,
                    MonotonicTime::ZERO,
                    MonotonicTime::ZERO,
                )
                .expect("initial registration");
        }

        let mut attempts = 0;
        while let Some(dispatch) = scheduler.start_next(MonotonicTime::ZERO) {
            attempts += 1;
            scheduler
                .finish(
                    dispatch.token,
                    MonotonicTime::ZERO,
                    CompletionSchedule::Success,
                )
                .expect("initial success");
        }
        assert_eq!(attempts, 15);
        assert!(scheduler.start_next(seconds(480)).is_none());
    }

    #[test]
    fn demand_before_initial_success_keeps_the_target_recurring() {
        let mut scheduler = OriginEdfScheduler::new();
        let scope = target("NB");
        let key = OriginJobKey::open(scope.clone());
        let general = GeneralOpenInterval::local(30).expect("interval");
        scheduler
            .register_open_initial(
                scope.clone(),
                general,
                0,
                MonotonicTime::ZERO,
                MonotonicTime::ZERO,
            )
            .expect("initial registration");
        scheduler
            .activate_open(&scope, general, MonotonicTime::ZERO)
            .expect("early demand");

        let initial = scheduler
            .start_next(MonotonicTime::ZERO)
            .expect("initial dispatch");
        scheduler
            .finish(
                initial.token,
                MonotonicTime::ZERO,
                CompletionSchedule::Success,
            )
            .expect("initial success");
        assert_eq!(scheduler.next_due(&key), Some(seconds(30)));
    }

    #[test]
    fn initial_catalog_race_and_retry_remain_active_until_success() {
        let mut scheduler = OriginEdfScheduler::new();
        let scope = target("NB");
        let key = OriginJobKey::open(scope.clone());
        scheduler
            .register_open_initial(
                scope,
                GeneralOpenInterval::local(30).expect("interval"),
                0,
                MonotonicTime::ZERO,
                MonotonicTime::ZERO,
            )
            .expect("initial registration");

        let raced = scheduler
            .start_next(MonotonicTime::ZERO)
            .expect("initial dispatch");
        scheduler
            .finish(
                raced.token,
                MonotonicTime::ZERO,
                CompletionSchedule::CatalogRace,
            )
            .expect("catalog race");
        assert_eq!(scheduler.next_due(&key), Some(seconds(30)));

        let retried = scheduler.start_next(seconds(30)).expect("race retry");
        let directive = retry_directive(
            &retried.key.target,
            Duration::from_secs(30),
            retried.failure_streak,
            OpenFailureKind::Transient,
        );
        scheduler
            .finish(
                retried.token,
                seconds(30),
                CompletionSchedule::Retry(directive),
            )
            .expect("transient retry");
        let retry_due = scheduler.next_due(&key).expect("retry remains active");
        assert!(retry_due >= seconds(35));
        assert!(retry_due < seconds(36));

        let succeeded = scheduler.start_next(retry_due).expect("retry dispatch");
        scheduler
            .finish(succeeded.token, retry_due, CompletionSchedule::Success)
            .expect("valid terminal success");
        assert_eq!(scheduler.next_due(&key), None);
    }

    #[test]
    fn demand_does_not_bypass_an_initial_origin_retry_cooldown() {
        let mut scheduler = OriginEdfScheduler::new();
        let scope = target("NB");
        let key = OriginJobKey::open(scope.clone());
        let general = GeneralOpenInterval::local(30).expect("interval");
        scheduler
            .register_open_initial(
                scope.clone(),
                general,
                0,
                MonotonicTime::ZERO,
                MonotonicTime::ZERO,
            )
            .expect("initial registration");
        let initial = scheduler
            .start_next(MonotonicTime::ZERO)
            .expect("initial dispatch");
        let directive = retry_directive(
            &scope,
            Duration::from_secs(30),
            initial.failure_streak,
            OpenFailureKind::RateLimited { retry_after: None },
        );
        scheduler
            .finish(
                initial.token,
                MonotonicTime::ZERO,
                CompletionSchedule::Retry(directive),
            )
            .expect("origin retry");
        let retry_due = scheduler.next_due(&key).expect("retry due");

        scheduler
            .activate_open(&scope, general, seconds(10))
            .expect("product demand");
        assert_eq!(scheduler.next_due(&key), Some(retry_due));
        assert!(scheduler.start_next(seconds(59)).is_none());
        assert!(scheduler.start_next(retry_due).is_some());
    }

    #[test]
    fn watch_activates_a_parked_initial_target_and_demotion_parks_it_again() {
        let mut scheduler = OriginEdfScheduler::new();
        let scope = target("NB");
        let key = OriginJobKey::open(scope.clone());
        let general = GeneralOpenInterval::local(30).expect("interval");
        scheduler
            .register_open_initial(
                scope.clone(),
                general,
                0,
                MonotonicTime::ZERO,
                MonotonicTime::ZERO,
            )
            .expect("initial registration");
        let initial = scheduler
            .start_next(MonotonicTime::ZERO)
            .expect("initial dispatch");
        scheduler
            .finish(
                initial.token,
                MonotonicTime::ZERO,
                CompletionSchedule::Success,
            )
            .expect("initial success");
        assert_eq!(scheduler.next_due(&key), None);

        scheduler
            .set_open_watch_count(&scope, general, 1, seconds(40))
            .expect("watch promotion");
        let watched = scheduler
            .start_next(seconds(40))
            .expect("parked watch is immediately due");
        assert_eq!(watched.lane, OriginSchedulerLane::OpenActiveWatch);
        scheduler
            .finish(watched.token, seconds(40), CompletionSchedule::Success)
            .expect("watched success");
        assert_eq!(scheduler.next_due(&key), Some(seconds(50)));

        scheduler
            .set_open_watch_count(&scope, general, 0, seconds(40))
            .expect("watch demotion");
        assert_eq!(scheduler.next_due(&key), None);
    }

    #[test]
    fn watch_promotion_and_demotion_change_one_batch_not_request_count() {
        let mut scheduler = OriginEdfScheduler::new();
        let scope = target("NB");
        let key = OriginJobKey::open(scope.clone());
        let general = GeneralOpenInterval::local(30).expect("interval");
        scheduler
            .register_open(scope.clone(), general, 0, MonotonicTime::ZERO, seconds(30))
            .expect("register");
        scheduler
            .set_open_watch_count(&scope, general, 1, MonotonicTime::ZERO)
            .expect("promote");
        assert_eq!(scheduler.next_due(&key), Some(seconds(10)));
        scheduler
            .set_open_watch_count(&scope, general, 9, MonotonicTime::ZERO)
            .expect("same shared lane");
        assert_eq!(scheduler.next_due(&key), Some(seconds(10)));
        assert_eq!(scheduler.active_watch_count(&scope), Some(9));

        let dispatch = scheduler.start_next(seconds(10)).expect("watch dispatch");
        scheduler
            .finish(dispatch.token, seconds(10), CompletionSchedule::Success)
            .expect("finish");
        scheduler
            .set_open_watch_count(&scope, general, 0, seconds(10))
            .expect("demote");
        assert_eq!(scheduler.next_due(&key), Some(seconds(40)));
    }

    #[test]
    fn retry_and_fatal_circuits_block_every_origin_job() {
        let mut scheduler = OriginEdfScheduler::new();
        let first_scope = target("NB");
        let second_scope = target("NWK");
        for scope in [&first_scope, &second_scope] {
            scheduler
                .register_open(
                    scope.clone(),
                    GeneralOpenInterval::local(30).expect("interval"),
                    0,
                    MonotonicTime::ZERO,
                    MonotonicTime::ZERO,
                )
                .expect("register");
        }
        let dispatch = scheduler.start_next(MonotonicTime::ZERO).expect("dispatch");
        let directive = retry_directive(
            &dispatch.key.target,
            Duration::from_secs(30),
            dispatch.failure_streak,
            OpenFailureKind::RateLimited { retry_after: None },
        );
        scheduler
            .finish(
                dispatch.token,
                MonotonicTime::ZERO,
                CompletionSchedule::Retry(directive),
            )
            .expect("finish");
        assert!(scheduler.start_next(seconds(59)).is_none());
        assert!(scheduler.start_next(seconds(60)).is_some());

        let mut fatal = OriginEdfScheduler::new();
        fatal
            .register_open(
                first_scope,
                GeneralOpenInterval::local(30).expect("interval"),
                0,
                MonotonicTime::ZERO,
                MonotonicTime::ZERO,
            )
            .expect("register");
        let dispatch = fatal.start_next(MonotonicTime::ZERO).expect("dispatch");
        let directive = legacy_fatal_directive(0);
        fatal
            .finish(
                dispatch.token,
                MonotonicTime::ZERO,
                CompletionSchedule::Retry(directive),
            )
            .expect("finish");
        assert!(fatal.start_next(seconds(60)).is_none());
        fatal
            .authorize_diagnostic_recheck(seconds(60))
            .expect("authorize");
        assert!(fatal.start_next(seconds(60)).is_some());
    }

    #[test]
    fn fatal_diagnostic_authorization_is_single_use_and_bound_to_failed_job() {
        let mut scheduler = OriginEdfScheduler::new();
        let failed_target = target("NB");
        let failed_key = OriginJobKey::open(failed_target.clone());
        scheduler
            .register_open(
                failed_target,
                GeneralOpenInterval::local(30).expect("interval"),
                0,
                MonotonicTime::ZERO,
                MonotonicTime::ZERO,
            )
            .expect("failed target");
        scheduler
            .register_catalog(target("NWK"), Duration::from_secs(600), seconds(1))
            .expect("other overdue job");
        let failed = scheduler
            .start_next(MonotonicTime::ZERO)
            .expect("failed pull");
        scheduler
            .finish(
                failed.token,
                MonotonicTime::ZERO,
                CompletionSchedule::Retry(legacy_fatal_directive(0)),
            )
            .expect("open fatal circuit");

        scheduler
            .authorize_diagnostic_recheck(seconds(60))
            .expect("authorize exact probe");
        let probe = scheduler.start_next(seconds(60)).expect("diagnostic probe");
        assert_eq!(
            probe.key, failed_key,
            "another overdue job cannot consume authorization"
        );
        scheduler
            .finish(
                probe.token,
                seconds(60),
                CompletionSchedule::Retry(retry_directive(
                    &probe.key.target,
                    Duration::from_secs(30),
                    probe.failure_streak,
                    OpenFailureKind::Transient,
                )),
            )
            .expect("inconclusive probe");
        let next_probe_due = scheduler.next_due(&failed_key).expect("retry due");
        assert!(scheduler.start_next(next_probe_due).is_none());
        scheduler
            .authorize_diagnostic_recheck(next_probe_due)
            .expect("authorize another exact probe");
        assert_eq!(
            scheduler
                .start_next(next_probe_due)
                .expect("second probe")
                .key,
            failed_key
        );
    }

    #[test]
    fn catalog_race_waits_for_the_normal_coalesced_due_without_immediate_request() {
        let mut scheduler = OriginEdfScheduler::new();
        let scope = target("NB");
        let key = OriginJobKey::open(scope.clone());
        scheduler
            .register_open(
                scope,
                GeneralOpenInterval::local(3_600).expect("interval"),
                0,
                MonotonicTime::ZERO,
                MonotonicTime::ZERO,
            )
            .expect("register");
        let dispatch = scheduler.start_next(MonotonicTime::ZERO).expect("pull");
        scheduler
            .finish(dispatch.token, seconds(1), CompletionSchedule::CatalogRace)
            .expect("race");
        assert_eq!(scheduler.next_due(&key), Some(seconds(3_600)));
        assert!(scheduler.start_next(seconds(1)).is_none());
    }

    #[test]
    fn mixed_virtual_day_makes_progress_without_negative_lag_or_catch_up() {
        let mut scheduler = OriginEdfScheduler::new();
        let mut counts = BTreeMap::<OriginJobKey, u64>::new();
        for index in 0..15 {
            let scope = target(&format!("C{index:02}"));
            let interval = match index % 4 {
                0 => GeneralOpenInterval::local(3).expect("interval"),
                1 => GeneralOpenInterval::local(10).expect("interval"),
                2 => GeneralOpenInterval::local(30).expect("interval"),
                _ => GeneralOpenInterval::local(3_600).expect("interval"),
            };
            scheduler
                .register_open(
                    scope.clone(),
                    interval,
                    u64::from(index % 5 == 0),
                    MonotonicTime::ZERO,
                    MonotonicTime::ZERO,
                )
                .expect("open");
            scheduler
                .register_catalog(scope, Duration::from_secs(600), MonotonicTime::ZERO)
                .expect("catalog");
        }

        let mut now = MonotonicTime::ZERO;
        let end = seconds(24 * 60 * 60);
        while now < end {
            if let Some(dispatch) = scheduler.start_next(now) {
                assert!(
                    dispatch.scheduler_lag <= now.saturating_duration_since(MonotonicTime::ZERO)
                );
                *counts.entry(dispatch.key.clone()).or_default() += 1;
                now = now.saturating_add(Duration::from_secs(1));
                scheduler
                    .finish(dispatch.token, now, CompletionSchedule::Success)
                    .expect("finish");
            } else {
                now = now.saturating_add(Duration::from_secs(1));
            }
        }
        assert_eq!(counts.len(), 30);
        assert!(counts.values().all(|count| *count > 0));
        assert!(scheduler.in_flight().is_none());
    }

    #[test]
    fn restart_due_translation_never_creates_negative_time() {
        assert_eq!(
            restore_monotonic_due(seconds(10), 50_000, 40_000),
            seconds(10)
        );
        assert_eq!(
            restore_monotonic_due(seconds(10), 40_000, 45_000),
            seconds(15)
        );
    }
}
