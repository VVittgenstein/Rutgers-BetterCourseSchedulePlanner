use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::time::Duration;

use bcsp_contracts::TermCampusKey;
use thiserror::Error;

use crate::{GeneralOpenInterval, RetryDirective, RetryMode};

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
    interval: Duration,
    next_due: MonotonicTime,
    lane: OriginSchedulerLane,
    normal_lane: OriginSchedulerLane,
    active_watch_count: u64,
    last_start: Option<MonotonicTime>,
    failure_streak: u32,
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
        )
    }

    pub fn register_open(
        &mut self,
        target: TermCampusKey,
        interval: GeneralOpenInterval,
        active_watch_count: u64,
        now: MonotonicTime,
        first_due: MonotonicTime,
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
            interval.effective(watched),
            first_due,
            normal_lane,
            active_watch_count,
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
    ) -> Result<(), SchedulerError> {
        if interval.is_zero() {
            return Err(SchedulerError::ZeroInterval);
        }
        match self.jobs.get_mut(&key) {
            Some(schedule) => {
                schedule.interval = interval;
                schedule.normal_lane = normal_lane;
                schedule.lane = normal_lane;
                schedule.active_watch_count = active_watch_count;
            }
            None => {
                self.jobs.insert(
                    key,
                    JobSchedule {
                        interval,
                        next_due: first_due,
                        lane: normal_lane,
                        normal_lane,
                        active_watch_count,
                        last_start: None,
                        failure_streak: 0,
                    },
                );
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

    pub fn set_open_watch_count(
        &mut self,
        target: &TermCampusKey,
        interval: GeneralOpenInterval,
        active_watch_count: u64,
        now: MonotonicTime,
    ) -> Result<(), SchedulerError> {
        let key = OriginJobKey::open(target.clone());
        let schedule = self.jobs.get_mut(&key).ok_or(SchedulerError::UnknownJob)?;
        let previously_watched = schedule.active_watch_count > 0;
        let watched = active_watch_count > 0;
        schedule.active_watch_count = active_watch_count;
        schedule.interval = interval.effective(watched);
        schedule.normal_lane = if watched {
            OriginSchedulerLane::OpenActiveWatch
        } else {
            OriginSchedulerLane::OpenGeneral
        };
        schedule.lane = schedule.normal_lane;
        let anchored = schedule
            .last_start
            .unwrap_or(now)
            .saturating_add(schedule.interval);
        if watched && !previously_watched {
            schedule.next_due = schedule.next_due.min(anchored);
        } else if !watched && previously_watched {
            schedule.next_due = anchored;
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
        schedule.next_due = schedule.next_due.min(due);
        schedule.lane = lane;
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
                .filter(|(_, schedule)| schedule.next_due <= now)
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
        self.jobs.get(key).map(|schedule| schedule.next_due)
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
    left.next_due
        .cmp(&right.next_due)
        .then_with(|| {
            right
                .lane
                .watched_priority()
                .cmp(&left.lane.watched_priority())
        })
        .then_with(|| left_key.cmp(right_key))
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

    use crate::{OpenFailureKind, retry_directive};

    use super::*;

    fn target(campus: &str) -> TermCampusKey {
        TermCampusKey::try_new("92026", campus).expect("synthetic target")
    }

    fn seconds(value: u64) -> MonotonicTime {
        MonotonicTime::from_millis(value * 1_000)
    }

    #[test]
    fn edf_uses_watch_priority_only_for_an_exact_due_tie() {
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
        assert_eq!(first.key.kind, OriginJobKind::Catalog);
        scheduler
            .finish(first.token, seconds(5), CompletionSchedule::Success)
            .expect("finish");

        let mut tied = OriginEdfScheduler::new();
        tied.register_catalog(target("CAT"), Duration::from_secs(600), seconds(5))
            .expect("catalog");
        tied.register_open(
            target("WATCH"),
            GeneralOpenInterval::local(30).expect("interval"),
            1,
            MonotonicTime::ZERO,
            seconds(5),
        )
        .expect("watch");
        let first = tied.start_next(seconds(5)).expect("first due");
        assert_eq!(first.lane, OriginSchedulerLane::OpenActiveWatch);
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
        assert!(scheduler.start_next(seconds(899)).is_none());
        assert!(scheduler.start_next(seconds(900)).is_some());

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
        let directive = retry_directive(
            &dispatch.key.target,
            Duration::from_secs(30),
            0,
            OpenFailureKind::FatalProtocol,
        );
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
                CompletionSchedule::Retry(retry_directive(
                    &failed.key.target,
                    Duration::from_secs(30),
                    0,
                    OpenFailureKind::FatalProtocol,
                )),
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
