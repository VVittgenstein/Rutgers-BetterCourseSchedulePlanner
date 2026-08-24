//! The local desired-watch authority surface and the process-level
//! coordinator that materializes it.
//!
//! Everything in this module is local-only. `desired_watches` is the user's
//! standing answer to "which sections do I want watched", it survives
//! restarts, and it is edited over ordinary local HTTP by every page equally.
//! There is no desired WebSocket, no projection stream and no leader tab: a
//! page reads the authority when it loads and after its own submissions, and
//! a change made in one tab becomes visible in another when that tab reloads.
//!
//! The coordinator is the one thing in the process that turns that intent
//! into physical watches. It holds the store, decides when the authority
//! needs rotating, owns the materialization records and the retry schedule,
//! and keeps at most ONE physical watch per section no matter how many pages
//! are open -- while the alerts that watch produces are fanned out to all of
//! them, because all of them really are watching.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use bcsp_application::SharedWatchSocket;
use bcsp_contracts::{
    ActiveWatchId, ActiveWatchTargetV1, SectionKey, TraceId, WatchPolicyV1, WatchStartItemResultV1,
    WatchStartItemV1, WatchStartItemsV1, WatchStartRejectionReason,
};
use bcsp_local_user_state::{
    DesiredWatchAuthority, DesiredWatchBudgetKind, DesiredWatchCommand, DesiredWatchEntry,
    DesiredWatchMutationOutcome, DesiredWatchReceiptOutcome, PersonalStateError,
    PersonalStateStore,
};
use bcsp_watch::WatchManagerError;
use serde::{Deserialize, Serialize};

/// Frozen path of the local desired-watch authority resource.
///
/// Ordinary local HTTP, registered beside `settings` and `selection` in the
/// local route extension. It is deliberately NOT a WebSocket route: a page
/// that opens a socket here reaches the HTTP handler, never an upgrade.
pub const LOCAL_DESIRED_WATCH_PATH: &str = "/api/v1/local/desired-watch";

/// Version of the local desired-watch read/write contract.
///
/// Separate from the transport `protocolVersion`: the envelope shape is
/// shared with every other local route, this number is about the desired
/// projection's own field set, and only the local build reads it.
pub const LOCAL_DESIRED_WATCH_CONTRACT_VERSION: u32 = 1;

/// Explicit byte budget for the authority read.
///
/// The largest legal authority state is nine desired rows plus a full
/// removal history: 521 rows, a bound the writer enforces rather than a
/// reconciler's good intentions. This response is never paginated and never
/// truncated -- a page that received part of the removal history would treat
/// the missing tombstones as absent rows, and a delayed START would then be
/// admitted against a revision of 0 and resurrect cancelled intent. So the
/// budget exists to be PROVEN against the largest legal state, not to clip
/// the response: `the_largest_authority_read_fits_the_local_budget` fails
/// the build rather than the request.
///
/// 256 KiB is the logical-state bound already frozen for this authority. The
/// 32 KiB figure in the earlier design was a bootstrap FRAME budget and has
/// no bearing here.
pub const LOCAL_DESIRED_WATCH_RESPONSE_BUDGET_BYTES: usize = 256 * 1024;

/// Bounded backoff for a materialization that failed for a reason that can
/// stop being true. Capped, and re-derived from the authority every time, so
/// a retry can never apply an answer to a state the user has since changed.
pub const DESIRED_WATCH_MATERIALIZE_BACKOFF: [Duration; 4] = [
    Duration::from_secs(5),
    Duration::from_secs(10),
    Duration::from_secs(20),
    Duration::from_secs(30),
];

// ---------------------------------------------------------------------------
// Wire contract
// ---------------------------------------------------------------------------

/// The authority read.
///
/// Carries the authority generation, every row INCLUDING tombstones with its
/// revision and materialization epoch, and the process's materialization
/// record for each one.
///
/// There is deliberately no `armed` field. Whether a section is really being
/// watched is `materialized != null` AND its four-part stamp equalling the
/// authority's -- generation, revision, epoch and policy. A separate boolean
/// could disagree with that stamp, and a page that believed the boolean
/// would show a green light for a watch that is not running. The page
/// derives it; nothing on the wire can contradict the parts.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesiredWatchStateV1 {
    pub contract_version: u32,
    pub authority_generation: u64,
    pub entries: Vec<DesiredWatchEntryV1>,
}

/// One authority row. `policy` is `null` exactly for a tombstone.
///
/// A tombstone is not debris: it holds the revision of a section the user
/// removed, so a command that read the row before the removal fails its
/// compare-and-swap instead of finding nothing and being admitted.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesiredWatchEntryV1 {
    pub section: SectionKey,
    pub policy: Option<WatchPolicyV1>,
    pub revision: u64,
    pub materialization_epoch: u64,
    pub materialized: Option<DesiredWatchMaterializedV1>,
    /// This section's own teardown is still in progress. The physical watch
    /// is still alive and can still ring, so this is never reported as
    /// stopped.
    pub pending_disarm: bool,
    /// The arm is waiting for a physical slot another section has not
    /// released yet. An expected state, not a failure.
    pub blocked_on_slot: bool,
    pub failure: Option<DesiredWatchFailureV1>,
}

/// What is actually running, and under which authority stamp.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesiredWatchMaterializedV1 {
    pub authority_generation: u64,
    pub revision: u64,
    pub materialization_epoch: u64,
    pub policy: WatchPolicyV1,
    pub active_watch_id: ActiveWatchId,
}

/// Why the intent could not be materialized, and whether anything is still
/// trying.
///
/// A `PERMANENT` failure keeps the row. The authority does not withdraw a
/// user's intent because the runtime cannot act on it -- the section may be
/// republished next term, and silently clearing the row would mean the user
/// finds an empty list and no explanation. The reason is reported and the
/// STOP stays theirs to make.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesiredWatchFailureV1 {
    pub classification: DesiredWatchFailureClassV1,
    pub reason: DesiredWatchFailureReasonV1,
    pub retry_scheduled: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DesiredWatchFailureClassV1 {
    /// Can stop being true on its own: a snapshot rebuilding, an integrity
    /// gate holding a target, a physical slot not yet released.
    Transient,
    /// Cannot, not without something outside this process changing: the
    /// campus is not a product target, the term is outside the window, the
    /// catalog does not publish the section.
    Permanent,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DesiredWatchFailureReasonV1 {
    SectionNotFound,
    TargetUnavailable,
    UnsupportedTarget,
    TermOutOfRange,
    AlreadyActive,
    ConnectionClosing,
    MaxActiveWatches,
}

impl DesiredWatchFailureReasonV1 {
    const fn from_rejection(reason: WatchStartRejectionReason) -> Self {
        match reason {
            WatchStartRejectionReason::SectionNotFound => Self::SectionNotFound,
            WatchStartRejectionReason::TargetUnavailable => Self::TargetUnavailable,
            WatchStartRejectionReason::UnsupportedTarget => Self::UnsupportedTarget,
            WatchStartRejectionReason::TermOutOfRange => Self::TermOutOfRange,
            WatchStartRejectionReason::AlreadyActive => Self::AlreadyActive,
            WatchStartRejectionReason::ConnectionClosing => Self::ConnectionClosing,
            WatchStartRejectionReason::MaxActiveWatches => Self::MaxActiveWatches,
        }
    }

    const fn classification(self) -> DesiredWatchFailureClassV1 {
        match self {
            // Not a product campus, outside the term window, or absent from
            // the published catalog: nothing this process does changes any
            // of those, so retrying is only noise.
            Self::UnsupportedTarget | Self::TermOutOfRange | Self::SectionNotFound => {
                DesiredWatchFailureClassV1::Permanent
            }
            // A snapshot that is not ready, a gate holding a target, a slot
            // still occupied, a connection closing: all of these resolve.
            Self::TargetUnavailable
            | Self::AlreadyActive
            | Self::ConnectionClosing
            | Self::MaxActiveWatches => DesiredWatchFailureClassV1::Transient,
        }
    }
}

/// A submitted compare-and-swap.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesiredWatchMutationV1 {
    pub contract_version: u32,
    pub section: SectionKey,
    /// `Some` asks for the section to be watched, `None` asks for it to stop.
    pub policy: Option<WatchPolicyV1>,
    /// The revision the submitting page read. `0` states "I read no row at
    /// all", which a tombstone still refuses.
    pub based_on_revision: u64,
    pub authority_generation: u64,
    pub mutation_id: TraceId,
}

/// What the authority decided, and the state it left behind.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesiredWatchMutationResultV1 {
    pub contract_version: u32,
    pub outcome: DesiredWatchOutcomeV1,
    /// True when this answer came from the receipt ledger rather than from
    /// deciding the command again. It does NOT change the HTTP status: a
    /// replayed refusal is still a refusal, and answering 200 because the
    /// envelope says "replayed" would tell a page its command succeeded.
    pub replayed: bool,
    /// Always the CURRENT authority generation, so a page that lost a race
    /// has the number it needs to re-read with.
    pub authority_generation: u64,
    /// The named section's current revision, when the refusal was about it.
    pub current_revision: Option<u64>,
    /// The product cap, when that is what was hit.
    pub maximum: Option<u32>,
    pub committed: Option<DesiredWatchCommittedV1>,
    /// The full authority read as of just after the commit, so the
    /// submitting page shows the state it produced rather than the state it
    /// hoped for. Present only on success; a refused page must re-read.
    pub state: Option<DesiredWatchStateV1>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesiredWatchCommittedV1 {
    pub revision: u64,
    pub materialization_epoch: u64,
    /// True when the `desired` VALUE changed and a new epoch was allocated.
    /// A policy edit keeps the epoch, which is the difference between
    /// adjusting a watch and restarting it.
    pub epoch_changed: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DesiredWatchOutcomeV1 {
    Committed,
    StaleGeneration,
    StaleRevision,
    MutationIdConflict,
    LimitExceeded,
    AuthorityFull,
}

impl DesiredWatchOutcomeV1 {
    /// The HTTP status this outcome is reported with.
    ///
    /// `409` for every terminal refusal, because all four mean the same
    /// thing to a page: what you read is not what is there, re-read before
    /// asking again. `503` for a full authority, because that one is NOT
    /// terminal -- nothing was written, no receipt was left, and the same id
    /// may be presented again once maintenance has run. Refusing a STOP
    /// forever would leave a watch the user cannot turn off.
    pub const fn http_status(self) -> u16 {
        match self {
            Self::Committed => 200,
            Self::StaleGeneration
            | Self::StaleRevision
            | Self::MutationIdConflict
            | Self::LimitExceeded => 409,
            Self::AuthorityFull => 503,
        }
    }
}

// ---------------------------------------------------------------------------
// Coordinator
// ---------------------------------------------------------------------------

/// What the process currently has running for one section.
#[derive(Clone, Debug, Eq, PartialEq)]
struct ArmedWatch {
    authority_generation: u64,
    revision: u64,
    materialization_epoch: u64,
    policy: WatchPolicyV1,
    active_watch_id: ActiveWatchId,
}

#[derive(Clone, Debug, Default)]
struct SectionMaterialization {
    armed: Option<ArmedWatch>,
    pending_disarm: bool,
    blocked_on_slot: bool,
    failure: Option<DesiredWatchFailureV1>,
    retry: Option<RetrySchedule>,
}

/// A scheduled retry, stamped with the authority tuple it was scheduled for.
///
/// The stamp is what makes a late retry safe. Between scheduling and firing
/// the user may have stopped the section, edited its policy, or reset the
/// whole authority; applying an answer derived from the old tuple would undo
/// them. A retry whose stamp no longer matches is dropped, not applied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RetrySchedule {
    authority_generation: u64,
    revision: u64,
    materialization_epoch: u64,
    attempt: usize,
    due_at: Instant,
}

#[derive(Debug, Default)]
struct Materialization {
    sections: BTreeMap<SectionKey, SectionMaterialization>,
    audience: usize,
}

/// Why a coordinator operation could not be completed.
#[derive(Debug)]
pub enum DesiredWatchCoordinatorError {
    /// The authority could not be read or written right now. Retryable.
    Storage(PersonalStateError),
    /// A coordinator lock was poisoned by a panic in another thread.
    Poisoned,
}

impl From<PersonalStateError> for DesiredWatchCoordinatorError {
    fn from(value: PersonalStateError) -> Self {
        Self::Storage(value)
    }
}

/// The process-level owner of desired-watch intent and its materialization.
///
/// One per running local process. It is the only writer of the authority,
/// the only caller of the physical watch owner, and the only thing that
/// decides when the authority is rotated -- which matters because rotation
/// raises the generation and invalidates every page's `basedOnRevision`, and
/// leaving that to whoever noticed the budget first would produce two
/// rotations for one threshold crossing.
pub struct DesiredWatchCoordinator {
    store: Mutex<PersonalStateStore>,
    watch: Arc<SharedWatchSocket>,
    materialization: Mutex<Materialization>,
    backoff: Vec<Duration>,
}

impl DesiredWatchCoordinator {
    pub fn new(store: PersonalStateStore, watch: Arc<SharedWatchSocket>) -> Self {
        Self {
            store: Mutex::new(store),
            watch,
            materialization: Mutex::new(Materialization::default()),
            backoff: DESIRED_WATCH_MATERIALIZE_BACKOFF.to_vec(),
        }
    }

    /// Replaces the retry schedule.
    ///
    /// The schedule is a parameter rather than a constant read at the call
    /// site so a test can prove that the retry MECHANISM works -- the stamp
    /// check, the classification, the recovery -- without spending the
    /// production interval doing it. Production never calls this, and the
    /// default it would otherwise use is asserted separately.
    pub fn with_retry_backoff(mut self, backoff: Vec<Duration>) -> Self {
        if !backoff.is_empty() {
            self.backoff = backoff;
        }
        self
    }

    /// The authority read, with this process's materialization record folded
    /// in.
    pub fn read(&self) -> Result<DesiredWatchStateV1, DesiredWatchCoordinatorError> {
        let materialization = self.lock_materialization()?;
        let authority = self.lock_store()?.desired_watch_authority()?;
        Ok(project(&authority, &materialization))
    }

    /// Submits one compare-and-swap and, if it committed, immediately brings
    /// the physical watches back in line with what it left behind.
    ///
    /// Reconciling here rather than on a timer is the whole reason a page
    /// sees its own submission take effect: the response carries the
    /// authority read taken AFTER the reconcile, so a START that armed says
    /// so, and a START that could not be armed says why.
    pub fn submit(
        &self,
        mutation: &DesiredWatchMutationV1,
    ) -> Result<DesiredWatchMutationResultV1, DesiredWatchCoordinatorError> {
        let command = DesiredWatchCommand {
            section: mutation.section.clone(),
            policy: mutation.policy.clone(),
            based_on_revision: mutation.based_on_revision,
            authority_generation: mutation.authority_generation,
            mutation_id: mutation.mutation_id,
        };
        // The store lock is released before anything else is touched: the
        // reconcile below takes the materialization lock first, and holding
        // both in the other order here would be the one cycle in this
        // module.
        let outcome = self.lock_store()?.commit_desired_watch_mutation(&command)?;
        let mut result = self.describe(outcome)?;
        if result.outcome == DesiredWatchOutcomeV1::Committed {
            self.reconcile()?;
            self.rotate_if_due()?;
            result.state = Some(self.read()?);
        }
        Ok(result)
    }

    /// Brings the physical watches in line with the committed authority.
    ///
    /// The whole state machine lives here rather than being spread across
    /// callbacks, because every entry point wants the same answer to the
    /// same question: given what the user has asked for and who is looking,
    /// what should be running?
    pub fn reconcile(&self) -> Result<(), DesiredWatchCoordinatorError> {
        let mut materialization = self.lock_materialization()?;
        let authority = self.lock_store()?.desired_watch_authority()?;
        self.reconcile_locked(&mut materialization, &authority);
        Ok(())
    }

    /// Re-reads the audience and reconciles. Called when a page attaches or
    /// leaves.
    pub fn audience_changed(&self) -> Result<(), DesiredWatchCoordinatorError> {
        self.reconcile()
    }

    /// One maintenance step, driven by the shared socket cadence.
    ///
    /// Two things make this necessary rather than decorative. A heartbeat
    /// sweep can drop the last page without any disconnect callback running,
    /// and a transient materialization failure needs something to come back
    /// and try again. Both are answered by re-deriving the whole picture,
    /// which is also why a lost event cannot strand the process.
    pub fn tick(&self) {
        let due = {
            let Ok(materialization) = self.lock_materialization() else {
                return;
            };
            let audience_changed = materialization.audience != self.watch.audience_connection_count();
            let now = Instant::now();
            audience_changed
                || materialization
                    .sections
                    .values()
                    .any(|section| section.retry.is_some_and(|retry| retry.due_at <= now))
        };
        if !due {
            return;
        }
        if let Err(error) = self.reconcile() {
            tracing::warn!(?error, "desired-watch reconcile failed; will retry");
        }
    }

    /// Forgets every materialization record without touching intent.
    ///
    /// Called after a Full Reset, which raises the authority generation,
    /// deletes every row, and stops the watch socket -- so the physical
    /// watches this coordinator was tracking no longer exist and their
    /// records would otherwise be adopted against the new generation.
    pub fn on_authority_cleared(&self) {
        if let Ok(mut materialization) = self.lock_materialization() {
            materialization.sections.clear();
            materialization.audience = 0;
        }
    }

    /// Rotates the authority when either budget has reached its threshold.
    ///
    /// The coordinator is the production owner of this decision. Rotation
    /// raises the generation and renumbers every surviving row, so every
    /// page must re-read; doing it from a storage call on whichever caller
    /// happened to notice would produce several rotations for one crossing.
    ///
    /// Healthy watches are NOT restarted by it. Rotation changes the numbers
    /// an intent is stamped with, not the intent, so the reconcile that
    /// follows adopts each running watch under its new stamp: same watch id,
    /// same episode, no second announcement.
    pub fn rotate_if_due(&self) -> Result<bool, DesiredWatchCoordinatorError> {
        if !self.lock_store()?.desired_watch_budget()?.rotation_due() {
            return Ok(false);
        }
        let mut materialization = self.lock_materialization()?;
        let authority = {
            let mut store = self.lock_store()?;
            store.rotate_desired_watch_authority()?;
            store.desired_watch_authority()?
        };
        tracing::info!(
            generation = authority.counters.authority_generation,
            "rotated the desired-watch authority",
        );
        self.reconcile_locked(&mut materialization, &authority);
        Ok(true)
    }

    fn reconcile_locked(
        &self,
        materialization: &mut MutexGuard<'_, Materialization>,
        authority: &DesiredWatchAuthority,
    ) {
        let generation = authority.counters.authority_generation;
        materialization.audience = self.watch.audience_connection_count();

        // The manager is the truth about whether a physical watch EXISTS; a
        // record for a section it no longer holds is stale, whatever caused
        // it (a process stop, a reset, an expired owner). The record is the
        // truth about which authority stamp that watch was armed under.
        let live = self
            .watch
            .owner_watched_sections()
            .into_iter()
            .collect::<BTreeSet<_>>();
        for (section, state) in materialization.sections.iter_mut() {
            if state.armed.is_some() && !live.contains(section) {
                state.armed = None;
            }
        }

        // No page is looking. Tear every physical watch down and keep every
        // row: the user's intent did not change when they closed a tab, and
        // a watch nobody can hear is a poll against Rutgers for nothing.
        if materialization.audience == 0 {
            let armed = materialization
                .sections
                .iter()
                .filter_map(|(section, state)| {
                    state
                        .armed
                        .as_ref()
                        .map(|armed| (section.clone(), armed.active_watch_id))
                })
                .collect::<Vec<_>>();
            for (section, active_watch_id) in armed {
                self.disarm(materialization, &section, active_watch_id);
            }
            // Only what actually came down is forgotten. A record whose
            // teardown FAILED still names a physical watch that may be alive
            // and may still ring; dropping it would leave the process holding
            // a watch it can no longer address -- every later arm for that
            // section would come back `AlreadyActive`, forever.
            materialization
                .sections
                .retain(|_, state| state.armed.is_some() || state.pending_disarm);
            return;
        }

        let desired = authority
            .entries
            .iter()
            .filter(|entry| entry.policy.is_some())
            .map(|entry| (entry.section.clone(), entry))
            .collect::<BTreeMap<_, _>>();

        // Anything no longer desired loses its physical watch first, so a
        // section waiting on a slot gets one in the same pass.
        let retired = materialization
            .sections
            .iter()
            .filter(|(section, _)| !desired.contains_key(*section))
            .filter_map(|(section, state)| {
                state
                    .armed
                    .as_ref()
                    .map(|armed| (section.clone(), armed.active_watch_id))
            })
            .collect::<Vec<_>>();
        for (section, active_watch_id) in retired {
            self.disarm(materialization, &section, active_watch_id);
        }
        materialization.sections.retain(|section, state| {
            desired.contains_key(section) || state.armed.is_some() || state.pending_disarm
        });

        let now = Instant::now();
        let mut to_arm = Vec::new();
        for (section, entry) in &desired {
            let policy = entry.policy.clone().expect("desired rows carry a policy");
            let state = materialization.sections.entry(section.clone()).or_default();
            match state.armed.clone() {
                // The physical watch is already running with the policy the
                // user wants. Adopt it under the current stamp instead of
                // restarting it: rotation and a bumped revision change the
                // numbers, not the watch, and tearing a healthy watch down
                // would end its episode and re-announce an open section the
                // user has already been told about.
                Some(armed) if armed.policy == policy => {
                    state.armed = Some(ArmedWatch {
                        authority_generation: generation,
                        revision: entry.revision,
                        materialization_epoch: entry.materialization_epoch,
                        policy,
                        active_watch_id: armed.active_watch_id,
                    });
                    state.failure = None;
                    state.retry = None;
                    state.blocked_on_slot = false;
                }
                // A policy edit. Applied in place for the same reason.
                Some(armed) => {
                    let target = ActiveWatchTargetV1 {
                        active_watch_id: armed.active_watch_id,
                        section_key: section.clone(),
                    };
                    match self.watch.owner_update_policy(target, policy.clone()) {
                        Ok(()) => {
                            state.armed = Some(ArmedWatch {
                                authority_generation: generation,
                                revision: entry.revision,
                                materialization_epoch: entry.materialization_epoch,
                                policy,
                                active_watch_id: armed.active_watch_id,
                            });
                            state.failure = None;
                            state.retry = None;
                        }
                        Err(error) => {
                            tracing::warn!(?error, "failed to apply a desired-watch policy edit");
                            state.armed = None;
                            state.failure = Some(DesiredWatchFailureV1 {
                                classification: DesiredWatchFailureClassV1::Transient,
                                reason: DesiredWatchFailureReasonV1::TargetUnavailable,
                                retry_scheduled: true,
                            });
                            schedule_retry(state, entry, generation, now, &self.backoff);
                        }
                    }
                }
                None => {
                    let permanent = state.failure.is_some_and(|failure| {
                        failure.classification == DesiredWatchFailureClassV1::Permanent
                    });
                    let waiting = state.retry.is_some_and(|retry| {
                        retry.due_at > now
                            && retry.authority_generation == generation
                            && retry.revision == entry.revision
                            && retry.materialization_epoch == entry.materialization_epoch
                    });
                    // A retry scheduled under a stamp the authority has since
                    // moved past is not a reason to wait: the question has
                    // changed, so ask it again now.
                    if !permanent && !waiting {
                        to_arm.push((section.clone(), entry, policy));
                    }
                }
            }
        }

        if to_arm.is_empty() {
            return;
        }
        let items = to_arm
            .iter()
            .map(|(section, _, policy)| WatchStartItemV1 {
                section_key: section.clone(),
                policy: policy.clone(),
            })
            .collect::<Vec<_>>();
        let results = match WatchStartItemsV1::try_from(items)
            .map_err(|_| WatchManagerError::InvalidContractProjection)
            .and_then(|items| self.watch.owner_start(items))
        {
            Ok(results) => results,
            Err(error) => {
                tracing::warn!(?error, "failed to arm desired watches");
                for (section, entry, _) in &to_arm {
                    let Some(state) = materialization.sections.get_mut(section) else {
                        continue;
                    };
                    state.failure = Some(DesiredWatchFailureV1 {
                        classification: DesiredWatchFailureClassV1::Transient,
                        reason: DesiredWatchFailureReasonV1::TargetUnavailable,
                        retry_scheduled: true,
                    });
                    schedule_retry(state, entry, generation, now, &self.backoff);
                }
                return;
            }
        };

        for (section, entry, policy) in &to_arm {
            let Some(state) = materialization.sections.get_mut(section) else {
                continue;
            };
            let result = results
                .iter()
                .find(|result| result.section_key() == section);
            match result {
                Some(WatchStartItemResultV1::Active {
                    active_watch_id, ..
                }) => {
                    state.armed = Some(ArmedWatch {
                        authority_generation: generation,
                        revision: entry.revision,
                        materialization_epoch: entry.materialization_epoch,
                        policy: policy.clone(),
                        active_watch_id: active_watch_id.clone(),
                    });
                    state.failure = None;
                    state.retry = None;
                    state.blocked_on_slot = false;
                }
                Some(WatchStartItemResultV1::Rejected { reason, .. }) => {
                    let reason = DesiredWatchFailureReasonV1::from_rejection(reason.clone());
                    let classification = reason.classification();
                    state.armed = None;
                    state.blocked_on_slot =
                        reason == DesiredWatchFailureReasonV1::MaxActiveWatches;
                    let retry_scheduled =
                        classification == DesiredWatchFailureClassV1::Transient;
                    state.failure = Some(DesiredWatchFailureV1 {
                        classification,
                        reason,
                        retry_scheduled,
                    });
                    if retry_scheduled {
                        schedule_retry(state, entry, generation, now, &self.backoff);
                    } else {
                        // Nothing this process does will change the answer.
                        // The row stays; the reason is on the wire; the STOP
                        // is the user's.
                        state.retry = None;
                    }
                }
                None => {
                    state.armed = None;
                    state.failure = Some(DesiredWatchFailureV1 {
                        classification: DesiredWatchFailureClassV1::Transient,
                        reason: DesiredWatchFailureReasonV1::TargetUnavailable,
                        retry_scheduled: true,
                    });
                    schedule_retry(state, entry, generation, now, &self.backoff);
                }
            }
        }
    }

    /// Tears one physical watch down, addressed by the id captured when it
    /// was armed.
    ///
    /// Never by re-resolving the section: between deciding to stop and
    /// stopping, the same section can have been armed again under a new id,
    /// and a stop that resolved late would kill the new watch instead of the
    /// old one.
    fn disarm(
        &self,
        materialization: &mut MutexGuard<'_, Materialization>,
        section: &SectionKey,
        active_watch_id: ActiveWatchId,
    ) {
        let target = ActiveWatchTargetV1 {
            active_watch_id,
            section_key: section.clone(),
        };
        let outcome = self.watch.owner_stop(target);
        let Some(state) = materialization.sections.get_mut(section) else {
            return;
        };
        match outcome {
            // An unknown watch has already achieved what the teardown wanted.
            Ok(()) | Err(WatchManagerError::UnknownWatch) => {
                state.armed = None;
                state.pending_disarm = false;
                state.blocked_on_slot = false;
                state.failure = None;
                state.retry = None;
            }
            Err(error) => {
                // The physical watch may still be alive and may still ring,
                // so this is never reported as stopped.
                tracing::warn!(?error, "failed to tear down a desired watch");
                state.pending_disarm = true;
            }
        }
    }

    fn describe(
        &self,
        outcome: DesiredWatchMutationOutcome,
    ) -> Result<DesiredWatchMutationResultV1, DesiredWatchCoordinatorError> {
        let generation = self
            .lock_store()?
            .desired_watch_authority()?
            .counters
            .authority_generation;
        let mut result = DesiredWatchMutationResultV1 {
            contract_version: LOCAL_DESIRED_WATCH_CONTRACT_VERSION,
            outcome: DesiredWatchOutcomeV1::Committed,
            replayed: false,
            authority_generation: generation,
            current_revision: None,
            maximum: None,
            committed: None,
            state: None,
        };
        let settled = match outcome {
            DesiredWatchMutationOutcome::Committed(commit) => {
                Some(DesiredWatchReceiptOutcome::committed(commit))
            }
            DesiredWatchMutationOutcome::Replayed(receipt) => {
                result.replayed = true;
                Some(receipt)
            }
            DesiredWatchMutationOutcome::MutationIdConflict(_) => {
                result.outcome = DesiredWatchOutcomeV1::MutationIdConflict;
                None
            }
            DesiredWatchMutationOutcome::StaleGeneration { current } => {
                result.outcome = DesiredWatchOutcomeV1::StaleGeneration;
                result.authority_generation = current;
                None
            }
            DesiredWatchMutationOutcome::StaleRevision { current } => {
                result.outcome = DesiredWatchOutcomeV1::StaleRevision;
                result.current_revision = Some(current);
                None
            }
            DesiredWatchMutationOutcome::LimitExceeded { maximum } => {
                result.outcome = DesiredWatchOutcomeV1::LimitExceeded;
                result.maximum = Some(u32::try_from(maximum).unwrap_or(u32::MAX));
                None
            }
            DesiredWatchMutationOutcome::AuthorityFull(kind) => {
                result.outcome = DesiredWatchOutcomeV1::AuthorityFull;
                result.maximum = Some(match kind {
                    DesiredWatchBudgetKind::Tombstones => {
                        u32::try_from(bcsp_local_user_state::MAX_DESIRED_WATCH_TOMBSTONES)
                            .unwrap_or(u32::MAX)
                    }
                    DesiredWatchBudgetKind::Receipts => {
                        u32::try_from(bcsp_local_user_state::MAX_DESIRED_WATCH_RECEIPTS)
                            .unwrap_or(u32::MAX)
                    }
                });
                None
            }
        };
        // A replayed answer is reported with the status the ORIGINAL answer
        // earned. The alternative -- 200 because the outer shape says
        // "replayed" -- would tell a page that lost its response that a
        // refused command had succeeded.
        if let Some(settled) = settled {
            match settled {
                DesiredWatchReceiptOutcome::Committed {
                    revision,
                    materialization_epoch,
                    epoch_changed,
                } => {
                    result.outcome = DesiredWatchOutcomeV1::Committed;
                    result.committed = Some(DesiredWatchCommittedV1 {
                        revision,
                        materialization_epoch,
                        epoch_changed,
                    });
                }
                DesiredWatchReceiptOutcome::StaleRevision { current } => {
                    result.outcome = DesiredWatchOutcomeV1::StaleRevision;
                    result.current_revision = Some(current);
                }
                DesiredWatchReceiptOutcome::LimitExceeded { maximum } => {
                    result.outcome = DesiredWatchOutcomeV1::LimitExceeded;
                    result.maximum = Some(u32::try_from(maximum).unwrap_or(u32::MAX));
                }
            }
        }
        Ok(result)
    }

    fn lock_store(&self) -> Result<MutexGuard<'_, PersonalStateStore>, DesiredWatchCoordinatorError> {
        self.store
            .lock()
            .map_err(|_| DesiredWatchCoordinatorError::Poisoned)
    }

    fn lock_materialization(
        &self,
    ) -> Result<MutexGuard<'_, Materialization>, DesiredWatchCoordinatorError> {
        self.materialization
            .lock()
            .map_err(|_| DesiredWatchCoordinatorError::Poisoned)
    }
}

fn schedule_retry(
    state: &mut SectionMaterialization,
    entry: &DesiredWatchEntry,
    generation: u64,
    now: Instant,
    backoff: &[Duration],
) {
    let attempt = state
        .retry
        .filter(|retry| {
            retry.authority_generation == generation
                && retry.revision == entry.revision
                && retry.materialization_epoch == entry.materialization_epoch
        })
        .map_or(0, |retry| retry.attempt + 1);
    let delay = backoff[attempt.min(backoff.len() - 1)];
    state.retry = Some(RetrySchedule {
        authority_generation: generation,
        revision: entry.revision,
        materialization_epoch: entry.materialization_epoch,
        attempt,
        due_at: now + delay,
    });
}

fn project(
    authority: &DesiredWatchAuthority,
    materialization: &Materialization,
) -> DesiredWatchStateV1 {
    DesiredWatchStateV1 {
        contract_version: LOCAL_DESIRED_WATCH_CONTRACT_VERSION,
        authority_generation: authority.counters.authority_generation,
        entries: authority
            .entries
            .iter()
            .map(|entry| {
                let state = materialization.sections.get(&entry.section);
                DesiredWatchEntryV1 {
                    section: entry.section.clone(),
                    policy: entry.policy.clone(),
                    revision: entry.revision,
                    materialization_epoch: entry.materialization_epoch,
                    materialized: state.and_then(|state| {
                        state
                            .armed
                            .as_ref()
                            .map(|armed| DesiredWatchMaterializedV1 {
                                authority_generation: armed.authority_generation,
                                revision: armed.revision,
                                materialization_epoch: armed.materialization_epoch,
                                policy: armed.policy.clone(),
                                active_watch_id: armed.active_watch_id,
                            })
                    }),
                    pending_disarm: state.is_some_and(|state| state.pending_disarm),
                    blocked_on_slot: state.is_some_and(|state| state.blocked_on_slot),
                    failure: state.and_then(|state| state.failure),
                }
            })
            .collect(),
    }
}
