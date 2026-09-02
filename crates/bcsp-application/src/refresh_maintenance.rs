//! WAL maintenance for the shared operational database.
//!
//! Every Open commit appends a few hundred pages to the write-ahead log, and
//! SQLite's automatic checkpoint only backfills them while no connection is
//! reading an older snapshot. On the local product a runtime-owned reader
//! once pinned the read mark for a whole session and the log reached three
//! gigabytes in ninety minutes. This module makes the checkpoint an explicit,
//! observed step of the refresh runtime instead of a side effect the process
//! hopes for:
//!
//! - the persistence layer only SIGNALS that maintenance is due (it runs on
//!   tokio worker threads inside the async Open service and must not block);
//! - the refresh runtime runs [`WalMaintenanceState::run`] on the blocking
//!   pool through the refresh writer connection, after every Open commit and
//!   after every Catalog publication;
//! - a PASSIVE checkpoint runs every time, an escalation to RESTART or
//!   TRUNCATE is decided by the pure [`WalMaintenancePolicy`] from the size
//!   of the log, and a [`WalStarvationDetector`] suppresses escalation while
//!   a pinned reader would make every escalation wait out its busy timeout
//!   for nothing, logging `WAL_CHECKPOINT_STARVED` with the transaction state
//!   of every long-lived connection the host can name instead.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::{Duration, Instant};

use bcsp_operational_storage::{
    OperationalStorage, StorageError, StorageTransactionState, WalCheckpointMode,
    WalCheckpointReport,
};

use crate::ProductStorageAccess;

/// Log size above which a PASSIVE checkpoint is followed by RESTART, so the
/// next writer starts the log from the beginning. 16,384 frames of 4 KiB is
/// the 64 MiB `journal_size_limit` the storage crates set.
pub const WAL_RESTART_LOG_FRAMES: u64 = 16_384;
/// Log size above which the escalation is TRUNCATE instead: the log file is
/// cut to zero bytes, which is what actually returns disk space.
pub const WAL_TRUNCATE_LOG_FRAMES: u64 = 65_536;
/// RESTART and TRUNCATE take the WAL write lock and wait for readers up to
/// the connection's busy timeout; personal-state writers wait behind them.
/// Spacing escalations keeps that wait from stacking behind a user action.
pub const WAL_ESCALATION_MIN_INTERVAL: Duration = Duration::from_secs(60);
/// A PASSIVE checkpoint that moved nothing while the log is at least this
/// large is one piece of evidence for a pinned reader.
pub const WAL_STARVATION_MIN_LOG_FRAMES: u64 = 4_096;
/// How many consecutive zero-progress reports it takes to call it starved.
pub const WAL_STARVATION_ZERO_PROGRESS_REPORTS: u32 = 3;

/// Why maintenance is due. A Catalog publication writes hundreds of
/// megabytes, so it earns a TRUNCATE outright; an Open commit runs the
/// ordinary PASSIVE-then-escalate step.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WalMaintenanceReason {
    OpenCommit,
    CatalogPublished,
}

impl WalMaintenanceReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OpenCommit => "open_commit",
            Self::CatalogPublished => "catalog_published",
        }
    }
}

const SIGNAL_OPEN_COMMIT: u8 = 0b01;
const SIGNAL_CATALOG_PUBLISHED: u8 = 0b10;

/// The "maintenance due" flag the commit path raises and the refresh runtime
/// consumes. Cloning shares the flag.
///
/// It is a flag rather than a queue on purpose: three commits before the
/// runtime looks are one maintenance run, not three, and a Catalog
/// publication dominates an Open commit that arrived in the same window.
#[derive(Clone, Debug, Default)]
pub struct WalMaintenanceSignal {
    due: Arc<AtomicU8>,
}

impl WalMaintenanceSignal {
    pub fn mark_open_commit(&self) {
        self.due.fetch_or(SIGNAL_OPEN_COMMIT, Ordering::AcqRel);
    }

    pub fn mark_catalog_published(&self) {
        self.due
            .fetch_or(SIGNAL_CATALOG_PUBLISHED, Ordering::AcqRel);
    }

    /// Consumes the flag, returning the strongest reason that was pending.
    pub fn take(&self) -> Option<WalMaintenanceReason> {
        let pending = self.due.swap(0, Ordering::AcqRel);
        if pending & SIGNAL_CATALOG_PUBLISHED != 0 {
            Some(WalMaintenanceReason::CatalogPublished)
        } else if pending & SIGNAL_OPEN_COMMIT != 0 {
            Some(WalMaintenanceReason::OpenCommit)
        } else {
            None
        }
    }

    pub fn is_due(&self) -> bool {
        self.due.load(Ordering::Acquire) != 0
    }
}

/// The transaction state of one connection a host owns, as reported in the
/// starvation diagnostic. `transaction_state` is the typed store's answer,
/// or a short reason why it could not be asked (`LOCKED` when another owner
/// holds the connection's mutex, `ERROR` when SQLite refused).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageConnectionInventoryEntry {
    pub connection: &'static str,
    pub transaction_state: &'static str,
}

impl StorageConnectionInventoryEntry {
    pub const LOCKED: &'static str = "LOCKED";
    pub const ERROR: &'static str = "ERROR";

    pub fn new(connection: &'static str, transaction_state: &'static str) -> Self {
        Self {
            connection,
            transaction_state,
        }
    }

    pub fn from_state(
        connection: &'static str,
        state: Result<StorageTransactionState, StorageError>,
    ) -> Self {
        Self::new(
            connection,
            state.map_or(Self::ERROR, StorageTransactionState::as_str),
        )
    }
}

/// The pure escalation rule. `decide` never touches SQLite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WalMaintenancePolicy {
    pub restart_when_log_frames_over: u64,
    pub truncate_when_log_frames_over: u64,
    pub min_interval_between_escalations: Duration,
}

impl Default for WalMaintenancePolicy {
    fn default() -> Self {
        Self {
            restart_when_log_frames_over: WAL_RESTART_LOG_FRAMES,
            truncate_when_log_frames_over: WAL_TRUNCATE_LOG_FRAMES,
            min_interval_between_escalations: WAL_ESCALATION_MIN_INTERVAL,
        }
    }
}

impl WalMaintenancePolicy {
    /// Which checkpoint, if any, should follow the PASSIVE one that produced
    /// `report`.
    ///
    /// A Catalog publication asks for TRUNCATE regardless of size. Otherwise
    /// the log size picks RESTART or TRUNCATE, the interval since the last
    /// escalation gates it, and a starved log gets nothing: RESTART and
    /// TRUNCATE would only wait out their busy timeout against the pinned
    /// reader and make no progress either.
    pub fn decide(
        &self,
        reason: WalMaintenanceReason,
        report: WalCheckpointReport,
        last_escalation: Option<Instant>,
        now: Instant,
        starved: bool,
    ) -> Option<WalCheckpointMode> {
        if starved {
            return None;
        }
        if reason == WalMaintenanceReason::CatalogPublished {
            return Some(WalCheckpointMode::Truncate);
        }
        let due = last_escalation
            .is_none_or(|last| now.duration_since(last) >= self.min_interval_between_escalations);
        if !due {
            return None;
        }
        if report.log_frames > self.truncate_when_log_frames_over {
            Some(WalCheckpointMode::Truncate)
        } else if report.log_frames > self.restart_when_log_frames_over {
            Some(WalCheckpointMode::Restart)
        } else {
            None
        }
    }
}

/// Counts consecutive PASSIVE checkpoints that made no progress on a log
/// that is large enough to matter. Progress of any size resets it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WalStarvationDetector {
    consecutive_zero_progress: u32,
    starved: bool,
}

impl WalStarvationDetector {
    /// Feeds one PASSIVE report. Returns `true` exactly when this report is
    /// the one that crosses into starvation, so the caller logs once per
    /// episode rather than once per commit.
    pub fn observe(&mut self, report: WalCheckpointReport) -> bool {
        let zero_progress =
            report.checkpointed_frames == 0 && report.log_frames >= WAL_STARVATION_MIN_LOG_FRAMES;
        if !zero_progress {
            self.consecutive_zero_progress = 0;
            self.starved = false;
            return false;
        }
        self.consecutive_zero_progress = self.consecutive_zero_progress.saturating_add(1);
        if self.starved {
            return false;
        }
        if self.consecutive_zero_progress >= WAL_STARVATION_ZERO_PROGRESS_REPORTS {
            self.starved = true;
            return true;
        }
        false
    }

    pub const fn is_starved(self) -> bool {
        self.starved
    }

    pub const fn consecutive_zero_progress(self) -> u32 {
        self.consecutive_zero_progress
    }
}

/// What one maintenance run did, for tests and the DEBUG trace.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WalMaintenanceOutcome {
    pub reason: WalMaintenanceReason,
    pub passive: WalCheckpointReport,
    pub escalation: Option<(WalCheckpointMode, WalCheckpointReport)>,
    pub starved: bool,
}

/// The refresh runtime's maintenance state: policy, detector, and the time
/// of the last escalation. One per process; moved into each blocking run and
/// back.
#[derive(Clone, Debug)]
pub struct WalMaintenanceState {
    policy: WalMaintenancePolicy,
    detector: WalStarvationDetector,
    last_escalation: Option<Instant>,
}

impl Default for WalMaintenanceState {
    fn default() -> Self {
        Self::new(WalMaintenancePolicy::default())
    }
}

impl WalMaintenanceState {
    pub fn new(policy: WalMaintenancePolicy) -> Self {
        Self {
            policy,
            detector: WalStarvationDetector::default(),
            last_escalation: None,
        }
    }

    pub const fn detector(&self) -> WalStarvationDetector {
        self.detector
    }

    /// Runs one maintenance step through the refresh writer.
    ///
    /// Blocking: call it from the blocking pool. The storage lock is held
    /// for the checkpoints only; the connection inventory is collected after
    /// it is released, so naming the pinned reader never waits behind the
    /// refresh writer itself. Returns `None` for in-memory fixtures.
    pub fn run<S>(
        &mut self,
        storage: &S,
        reason: WalMaintenanceReason,
    ) -> Option<WalMaintenanceOutcome>
    where
        S: ProductStorageAccess,
    {
        let now = Instant::now();
        let started = Instant::now();
        let checkpoints = {
            let Ok(guard) = storage.lock_operational() else {
                tracing::error!(code = "WAL_MAINTENANCE_STORAGE_UNAVAILABLE");
                return None;
            };
            run_checkpoints(
                &guard,
                reason,
                &self.policy,
                self.detector,
                self.last_escalation,
                now,
            )
        };
        let (outcome, newly_starved) = match checkpoints {
            Ok(Some(result)) => result,
            Ok(None) => return None,
            Err(error) => {
                tracing::warn!(code = "WAL_CHECKPOINT_FAILED", error = ?error);
                return None;
            }
        };
        self.detector = outcome.detector;
        if outcome.escalation.is_some() {
            self.last_escalation = Some(now);
        }
        tracing::debug!(
            target: "bcsp_performance",
            phase = "wal_checkpoint",
            reason = reason.as_str(),
            mode = WalCheckpointMode::Passive.as_str(),
            busy = outcome.passive.busy,
            log_frames = outcome.passive.log_frames,
            checkpointed_frames = outcome.passive.checkpointed_frames,
            elapsed_us = elapsed_micros(started),
        );
        if let Some((mode, report)) = outcome.escalation {
            tracing::debug!(
                target: "bcsp_performance",
                phase = "wal_checkpoint",
                reason = reason.as_str(),
                mode = mode.as_str(),
                busy = report.busy,
                log_frames = report.log_frames,
                checkpointed_frames = report.checkpointed_frames,
                elapsed_us = elapsed_micros(started),
            );
        }
        if newly_starved {
            let inventory = storage.connection_inventory();
            let connections = inventory
                .iter()
                .map(|entry| format!("{}={}", entry.connection, entry.transaction_state))
                .collect::<Vec<_>>()
                .join(",");
            tracing::warn!(
                code = "WAL_CHECKPOINT_STARVED",
                log_frames = outcome.passive.log_frames,
                zero_progress_reports = self.detector.consecutive_zero_progress(),
                connections = %connections,
                "the write-ahead log cannot be checkpointed: a connection is holding a read transaction",
            );
        }
        Some(WalMaintenanceOutcome {
            reason,
            passive: outcome.passive,
            escalation: outcome.escalation,
            starved: self.detector.is_starved(),
        })
    }
}

struct CheckpointStep {
    passive: WalCheckpointReport,
    escalation: Option<(WalCheckpointMode, WalCheckpointReport)>,
    detector: WalStarvationDetector,
}

fn run_checkpoints(
    storage: &OperationalStorage,
    reason: WalMaintenanceReason,
    policy: &WalMaintenancePolicy,
    mut detector: WalStarvationDetector,
    last_escalation: Option<Instant>,
    now: Instant,
) -> Result<Option<(CheckpointStep, bool)>, StorageError> {
    let Some(passive) = storage.checkpoint_wal(WalCheckpointMode::Passive)? else {
        return Ok(None);
    };
    let newly_starved = detector.observe(passive);
    let escalation =
        match policy.decide(reason, passive, last_escalation, now, detector.is_starved()) {
            Some(mode) => storage.checkpoint_wal(mode)?.map(|report| (mode, report)),
            None => None,
        };
    Ok(Some((
        CheckpointStep {
            passive,
            escalation,
            detector,
        },
        newly_starved,
    )))
}

fn elapsed_micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(log_frames: u64, checkpointed_frames: u64) -> WalCheckpointReport {
        WalCheckpointReport {
            busy: false,
            log_frames,
            checkpointed_frames,
        }
    }

    #[test]
    fn the_signal_coalesces_and_the_catalog_reason_dominates() {
        let signal = WalMaintenanceSignal::default();
        assert_eq!(signal.take(), None);
        signal.mark_open_commit();
        signal.mark_open_commit();
        assert!(signal.is_due());
        assert_eq!(signal.take(), Some(WalMaintenanceReason::OpenCommit));
        assert_eq!(signal.take(), None);
        signal.mark_open_commit();
        signal.mark_catalog_published();
        assert_eq!(signal.take(), Some(WalMaintenanceReason::CatalogPublished));
        assert!(!signal.is_due());
        let shared = signal.clone();
        shared.mark_open_commit();
        assert_eq!(signal.take(), Some(WalMaintenanceReason::OpenCommit));
    }

    #[test]
    fn a_small_log_gets_passive_only() {
        let policy = WalMaintenancePolicy::default();
        let now = Instant::now();
        assert_eq!(
            policy.decide(
                WalMaintenanceReason::OpenCommit,
                report(WAL_RESTART_LOG_FRAMES, WAL_RESTART_LOG_FRAMES),
                None,
                now,
                false
            ),
            None
        );
    }

    #[test]
    fn the_log_size_picks_restart_then_truncate() {
        let policy = WalMaintenancePolicy::default();
        let now = Instant::now();
        assert_eq!(
            policy.decide(
                WalMaintenanceReason::OpenCommit,
                report(WAL_RESTART_LOG_FRAMES + 1, 100),
                None,
                now,
                false
            ),
            Some(WalCheckpointMode::Restart)
        );
        assert_eq!(
            policy.decide(
                WalMaintenanceReason::OpenCommit,
                report(WAL_TRUNCATE_LOG_FRAMES + 1, 100),
                None,
                now,
                false
            ),
            Some(WalCheckpointMode::Truncate)
        );
    }

    #[test]
    fn escalations_are_spaced_by_the_minimum_interval() {
        let policy = WalMaintenancePolicy::default();
        let now = Instant::now();
        let recent = now - Duration::from_secs(59);
        let old = now - Duration::from_secs(60);
        let large = report(WAL_TRUNCATE_LOG_FRAMES + 1, 0);
        assert_eq!(
            policy.decide(
                WalMaintenanceReason::OpenCommit,
                large,
                Some(recent),
                now,
                false
            ),
            None
        );
        assert_eq!(
            policy.decide(
                WalMaintenanceReason::OpenCommit,
                large,
                Some(old),
                now,
                false
            ),
            Some(WalCheckpointMode::Truncate)
        );
    }

    #[test]
    fn a_catalog_publication_truncates_regardless_of_size_or_interval() {
        let policy = WalMaintenancePolicy::default();
        let now = Instant::now();
        assert_eq!(
            policy.decide(
                WalMaintenanceReason::CatalogPublished,
                report(10, 10),
                Some(now),
                now,
                false
            ),
            Some(WalCheckpointMode::Truncate)
        );
    }

    #[test]
    fn starvation_suppresses_every_escalation() {
        let policy = WalMaintenancePolicy::default();
        let now = Instant::now();
        for reason in [
            WalMaintenanceReason::OpenCommit,
            WalMaintenanceReason::CatalogPublished,
        ] {
            assert_eq!(
                policy.decide(
                    reason,
                    report(WAL_TRUNCATE_LOG_FRAMES * 4, 0),
                    None,
                    now,
                    true
                ),
                None,
                "{reason:?}"
            );
        }
    }

    #[test]
    fn the_detector_needs_three_zero_progress_reports_on_a_large_log() {
        let mut detector = WalStarvationDetector::default();
        assert!(!detector.observe(report(WAL_STARVATION_MIN_LOG_FRAMES, 0)));
        assert!(!detector.observe(report(WAL_STARVATION_MIN_LOG_FRAMES + 500, 0)));
        assert!(!detector.is_starved());
        assert!(detector.observe(report(WAL_STARVATION_MIN_LOG_FRAMES + 1_000, 0)));
        assert!(detector.is_starved());
        // Already starved: reported once, not on every later commit.
        assert!(!detector.observe(report(WAL_STARVATION_MIN_LOG_FRAMES + 1_500, 0)));
        assert_eq!(detector.consecutive_zero_progress(), 4);
    }

    #[test]
    fn any_progress_resets_the_detector_and_a_small_log_never_counts() {
        let mut detector = WalStarvationDetector::default();
        detector.observe(report(WAL_STARVATION_MIN_LOG_FRAMES, 0));
        detector.observe(report(WAL_STARVATION_MIN_LOG_FRAMES, 0));
        assert!(!detector.observe(report(WAL_STARVATION_MIN_LOG_FRAMES, 1)));
        assert_eq!(detector.consecutive_zero_progress(), 0);
        for _ in 0..10 {
            assert!(!detector.observe(report(WAL_STARVATION_MIN_LOG_FRAMES - 1, 0)));
        }
        assert!(!detector.is_starved());
        // A reset also ends an episode, so the next one is reported again.
        for _ in 0..3 {
            detector.observe(report(WAL_STARVATION_MIN_LOG_FRAMES, 0));
        }
        assert!(detector.is_starved());
        detector.observe(report(WAL_STARVATION_MIN_LOG_FRAMES, 5));
        assert!(!detector.is_starved());
        assert!(!detector.observe(report(WAL_STARVATION_MIN_LOG_FRAMES, 0)));
        assert!(!detector.observe(report(WAL_STARVATION_MIN_LOG_FRAMES, 0)));
        assert!(detector.observe(report(WAL_STARVATION_MIN_LOG_FRAMES, 0)));
    }

    #[test]
    fn in_memory_storage_has_nothing_to_maintain() {
        let storage = Arc::new(std::sync::Mutex::new(
            OperationalStorage::open_in_memory().expect("storage"),
        ));
        let mut state = WalMaintenanceState::default();
        assert_eq!(state.run(&storage, WalMaintenanceReason::OpenCommit), None);
    }

    #[test]
    fn a_file_backed_run_checkpoints_and_reports_the_inventory_shape() {
        let temp = tempfile::TempDir::new().expect("temp");
        let path = temp.path().join("maintenance.sqlite");
        let storage = Arc::new(std::sync::Mutex::new(
            OperationalStorage::open(&path).expect("storage"),
        ));
        let mut state = WalMaintenanceState::default();
        let outcome = state
            .run(&storage, WalMaintenanceReason::CatalogPublished)
            .expect("file-backed");
        assert_eq!(outcome.reason, WalMaintenanceReason::CatalogPublished);
        assert!(outcome.passive.is_complete());
        assert!(matches!(
            outcome.escalation,
            Some((WalCheckpointMode::Truncate, _))
        ));
        assert!(!outcome.starved);
        let inventory = storage.connection_inventory();
        assert_eq!(
            inventory,
            vec![StorageConnectionInventoryEntry::new(
                "operational",
                StorageTransactionState::None.as_str()
            )]
        );
        // The interval gate now holds the next escalation back.
        let next = state
            .run(&storage, WalMaintenanceReason::OpenCommit)
            .expect("file-backed");
        assert_eq!(next.escalation, None);
    }
}
