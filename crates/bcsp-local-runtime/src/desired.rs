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
use bcsp_watch::{WatchManagerError, WatchStartAdmission};
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

/// The `revision` and `materializationEpoch` a commit reports when the row it
/// wrote is legitimately no longer in the authority it is answered from.
///
/// Zero and not the pre-rotation numbers, and frozen rather than merely
/// conventional: no live row ever carries it, so a decoder can require
/// exactly this pair for a missing row and refuse a body whose commit claims
/// numbers the state it shipped does not hold.
pub const DESIRED_WATCH_ABSENT_COMMITTED_NUMBER: u64 = 0;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesiredWatchCommittedV1 {
    /// The row's revision after the write, or
    /// [`DESIRED_WATCH_ABSENT_COMMITTED_NUMBER`] when the row is no longer in
    /// the authority this answer carries.
    pub revision: u64,
    /// The row's materialization epoch after the write, under the same rule.
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

/// Production cadence for re-asking whether a RUNNING watch could still be
/// started today.
///
/// Every condition materialization depends on is revocable: a term rolls out
/// of the window, a campus stops being a product target, a catalog stops
/// publishing a section, an integrity gate takes a target back. Checking them
/// only at arm time turns the first one that changes into a permanent green
/// light for a watch that is no longer legitimate, so the coordinator keeps
/// asking.
///
/// This is BOUNDED EVENTUAL revalidation, not per-poll revalidation. The
/// shortest legal local watch interval is
/// `LOCAL_MINIMUM_WATCH_OPEN_INTERVAL_SECONDS`, three seconds, so a user who
/// has chosen a fast cadence can see several polls of a revoked target before
/// this notices -- what is guaranteed is that the green light goes out within
/// a bounded time, not that no poll happens in between. Shortening it to
/// match the fastest poll would ask storage and the catalog the same question
/// every three seconds for every watched section, which is a real load change
/// and not this milestone's to make.
/// `the_revalidation_cadence_is_bounded_but_not_per_poll` pins both numbers so
/// the relationship cannot drift unnoticed.
pub const DESIRED_WATCH_REVALIDATE_INTERVAL: Duration = Duration::from_secs(15);

/// The physical-watch side of materialization.
///
/// The coordinator decides WHAT should be running from durable intent; this
/// is everything it needs from the thing that actually runs it. It exists as
/// a trait for one reason: the failure paths -- a teardown that will not tear
/// down, a policy edit that will not apply -- are the ones that leave a watch
/// alive but unaddressable, and they cannot be reached at all through a
/// healthy socket. A seam makes them reachable, so those paths are tested
/// rather than reasoned about.
pub trait DesiredWatchOwner: Send + Sync + 'static {
    /// How many BROWSER pages are attached. The owner itself is excluded, or
    /// the audience would never be empty and the watches never torn down.
    fn audience_connection_count(&self) -> usize;

    /// Every physical watch the process holds, by identity.
    fn watch_targets(&self) -> Vec<ActiveWatchTargetV1>;

    /// Whether one section could be armed right now, asked of the same
    /// authoritative view a START would consult.
    fn admission_for(&self, section: &SectionKey) -> WatchStartAdmission;

    fn start(
        &self,
        items: WatchStartItemsV1,
    ) -> Result<Vec<WatchStartItemResultV1>, WatchManagerError>;

    fn stop(&self, target: ActiveWatchTargetV1) -> Result<(), WatchManagerError>;

    fn update_policy(
        &self,
        target: ActiveWatchTargetV1,
        policy: WatchPolicyV1,
    ) -> Result<(), WatchManagerError>;
}

impl DesiredWatchOwner for SharedWatchSocket {
    fn audience_connection_count(&self) -> usize {
        Self::audience_connection_count(self)
    }

    fn watch_targets(&self) -> Vec<ActiveWatchTargetV1> {
        self.owner_watch_targets()
    }

    fn admission_for(&self, section: &SectionKey) -> WatchStartAdmission {
        Self::admission_for(self, section)
    }

    fn start(
        &self,
        items: WatchStartItemsV1,
    ) -> Result<Vec<WatchStartItemResultV1>, WatchManagerError> {
        self.owner_start(items)
    }

    fn stop(&self, target: ActiveWatchTargetV1) -> Result<(), WatchManagerError> {
        self.owner_stop(target)
    }

    fn update_policy(
        &self,
        target: ActiveWatchTargetV1,
        policy: WatchPolicyV1,
    ) -> Result<(), WatchManagerError> {
        self.owner_update_policy(target, policy)
    }
}

/// A named point inside the coordinator that a test can stand at.
///
/// Compiled only under `cfg(test)`, and therefore only into this crate's own
/// unit tests. It is not exported, not reachable from an integration test,
/// and not present at all in a shipping build: the two guarantees it exists
/// for are about instruction-level ordering inside this module, so the tests
/// that need it live in this module too rather than the seam being widened
/// into the crate's public API to reach them.
#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DesiredWatchCheckpoint {
    /// The store has decided one mutation's outcome. The response it will be
    /// reported with has not been assembled yet, and no coordinator lock is
    /// held.
    MutationDecided,
    /// The rotation budget has just been read and found due. The rotation
    /// itself has not happened yet.
    RotationDue,
}

/// A seam for standing at a [`DesiredWatchCheckpoint`].
///
/// Two of this module's guarantees are about WHEN one thing is read relative
/// to another -- the rotation budget against the rotation it authorises, a
/// mutation's outcome against the authority snapshot its answer is stamped
/// from -- and a window a few instructions wide is not something a test can
/// reach by sleeping and hoping. Every earlier attempt to prove them ended up
/// asserting that a guessed delay had been long enough, which passes just as
/// happily against the implementation it was written to reject.
///
/// `exclusive` is what turns "these two happen close together" into "no other
/// caller can be between them": it reports whether the coordinator's
/// exclusive domain is held at this point. A check that reads the budget
/// outside the domain and then takes it arrives here holding nothing, and
/// says so.
///
/// A shipping build has no such thing. The trait, the field that holds one
/// and the calls that reach it are all `cfg(test)`, so there is no public
/// hook a caller could install an arbitrary blocking callback into and no way
/// to run one while the materialization mutex is held.
#[cfg(test)]
trait DesiredWatchCheckpoints: Send + Sync + 'static {
    fn arrive(&self, checkpoint: DesiredWatchCheckpoint, exclusive: bool);
}

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
    /// A physical watch that should be gone and is not.
    ///
    /// A failed teardown is not "already stopped": the watch is still alive
    /// and can still ring. Its identity moves HERE rather than staying in
    /// `armed`, because the two are different claims -- `armed` is what makes
    /// a section green, and a watch being torn down must never be green --
    /// and rather than being dropped, because that id is the only thing that
    /// can ever stop it. Everything that finishes a teardown, including the
    /// STOP the user presses next, addresses the watch from here.
    stopping: Option<StoppingWatch>,
    blocked_on_slot: bool,
    failure: Option<StampedFailure>,
    retry: Option<RetrySchedule>,
}

/// A materialization failure, and the authority tuple it was decided under.
///
/// The stamp is what stops a verdict from outliving the question. A
/// `PERMANENT` failure is the strongest thing this module records -- it says
/// "do not try again" -- and without a stamp that instruction attaches to the
/// SECTION rather than to the intent that produced it: the user stops the
/// section, starts it again, edits its policy, or the authority rotates, and
/// a conclusion drawn about a revision nobody is asking about any more goes
/// on suppressing every arm for the rest of the process's life.
///
/// Stamped, it is a statement about one `(generation, revision, epoch)`. A
/// new intent is a new question, asked again from scratch; if the answer is
/// still no, it is recorded again under the stamp that earned it, which is
/// also what the page is shown. It is deliberately the same tuple
/// [`RetrySchedule`] carries, and for the same reason.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StampedFailure {
    authority_generation: u64,
    revision: u64,
    materialization_epoch: u64,
    failure: DesiredWatchFailureV1,
}

impl StampedFailure {
    /// Whether this verdict was reached about the intent described here.
    const fn describes(&self, generation: u64, revision: u64, epoch: u64) -> bool {
        self.authority_generation == generation
            && self.revision == revision
            && self.materialization_epoch == epoch
    }
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

/// An unfinished teardown, and when it may be attempted again.
///
/// Deliberately NOT stamped with an authority tuple, and deliberately holding
/// the watch id rather than expecting a caller to still have it: it is
/// addressed by the id captured when the watch was armed, so however late the
/// retry fires it can only ever stop that one watch. A teardown that
/// re-resolved the section could kill a watch armed since.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StoppingWatch {
    active_watch_id: ActiveWatchId,
    attempt: usize,
    due_at: Instant,
}

#[derive(Debug, Default)]
struct Materialization {
    sections: BTreeMap<SectionKey, SectionMaterialization>,
    audience: usize,
    /// A Full Reset owns every physical watch until it finishes.
    ///
    /// Between the moment a reset starts and the moment it has both cleared
    /// the authority and torn the watches down, the rows still on disk do NOT
    /// describe what should be running -- they describe what is being
    /// deleted. A reconcile that ran in that window would read them and arm,
    /// and the reset would then clear the records of watches it had just
    /// caused to exist: an authority that is empty and a process that is
    /// still polling Rutgers for the user.
    resetting: bool,
    /// A Full Reset that could not finish tearing the watches down.
    ///
    /// The barrier stays raised while this is set. An authority that is empty
    /// while the process still holds a watch nobody can see is precisely the
    /// state the reset exists to prevent, so the reset has not happened until
    /// the last one is gone -- and something has to come back and finish it.
    reset_incomplete: bool,
    /// When the running watches were last re-checked against the world.
    last_revalidated: Option<Instant>,
}

/// Why a coordinator operation could not be completed.
#[derive(Debug)]
pub enum DesiredWatchCoordinatorError {
    /// The authority could not be read or written right now. Retryable.
    Storage(PersonalStateError),
    /// A coordinator lock was poisoned by a panic in another thread.
    Poisoned,
    /// A Full Reset could not stop every physical watch the process holds.
    ///
    /// Reported rather than logged, because the alternative is the one answer
    /// this surface must never give: "reset succeeded" beside a process that
    /// is still polling Rutgers for rows it has just deleted. The barrier
    /// stays raised, the identities stay addressable, and finishing is
    /// retried -- so this is retryable, not a defect.
    ResetIncomplete,
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
    owner: Arc<dyn DesiredWatchOwner>,
    materialization: Mutex<Materialization>,
    backoff: Vec<Duration>,
    revalidate_interval: Duration,
    #[cfg(test)]
    checkpoints: Option<Arc<dyn DesiredWatchCheckpoints>>,
}

impl DesiredWatchCoordinator {
    pub fn new(store: PersonalStateStore, watch: Arc<SharedWatchSocket>) -> Self {
        Self::with_owner(store, watch)
    }

    pub fn with_owner(store: PersonalStateStore, owner: Arc<dyn DesiredWatchOwner>) -> Self {
        Self {
            store: Mutex::new(store),
            owner,
            materialization: Mutex::new(Materialization::default()),
            backoff: DESIRED_WATCH_MATERIALIZE_BACKOFF.to_vec(),
            revalidate_interval: DESIRED_WATCH_REVALIDATE_INTERVAL,
            #[cfg(test)]
            checkpoints: None,
        }
    }

    /// Installs somewhere for the coordinator to stand at its checkpoints.
    ///
    /// See [`DesiredWatchCheckpoints`] for why the seam exists at all rather
    /// than being replaced by a longer sleep, and why it exists only here.
    #[cfg(test)]
    fn with_checkpoints(mut self, checkpoints: Arc<dyn DesiredWatchCheckpoints>) -> Self {
        self.checkpoints = Some(checkpoints);
        self
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

    /// Replaces the interval between re-checks of a running watch.
    ///
    /// Same reason as the backoff: the behaviour worth proving is that a
    /// watch whose target stopped being admissible is taken down, not that
    /// fifteen seconds have elapsed.
    pub const fn with_revalidation_interval(mut self, interval: Duration) -> Self {
        self.revalidate_interval = interval;
        self
    }

    /// The authority read, with this process's materialization record folded
    /// in.
    pub fn read(&self) -> Result<DesiredWatchStateV1, DesiredWatchCoordinatorError> {
        let materialization = self.lock_materialization()?;
        let authority = self.lock_store()?.desired_watch_authority()?;
        // The live set, not the record, decides whether anything is running.
        // A record can outlive the watch it names -- a process stop, a
        // heartbeat sweep, a term rollover all end a watch without anyone
        // telling the coordinator -- and a read that trusted the record alone
        // would show a green light for a watch nobody is holding.
        Ok(project(
            &authority,
            &materialization,
            &live_watches(self.owner.as_ref()),
        ))
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
        let mut result = describe(outcome);
        #[cfg(test)]
        self.arrive(DesiredWatchCheckpoint::MutationDecided);
        if result.outcome == DesiredWatchOutcomeV1::Committed {
            self.reconcile()?;
        }
        // Rotation maintenance runs after EVERY decision, not only after a
        // commit. Terminal refusals write receipts too, so a page repeatedly
        // refused -- nine watched sections and a tenth the user keeps asking
        // for -- grows the ledger without a single commit. If only commits
        // rotated, that ledger would reach its hard cap and every later
        // write, including the STOP that would fix it, would be refused
        // forever.
        self.rotate_if_due()?;
        self.stamp(&mut result, &mutation.section)?;
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
    ///
    /// An attach is also the moment a restarted process first has a reason to
    /// look at its own maintenance: a database that came back with a full
    /// receipt ledger would otherwise refuse every write until something
    /// else happened to rotate it.
    pub fn audience_changed(&self) -> Result<(), DesiredWatchCoordinatorError> {
        self.rotate_if_due()?;
        self.reconcile()
    }

    /// One maintenance step, driven by the shared socket cadence.
    ///
    /// Three things make this necessary rather than decorative. A heartbeat
    /// sweep can drop the last page without any disconnect callback running;
    /// a transient materialization failure needs something to come back and
    /// try again; and a watch can end underneath the coordinator -- a term
    /// rollover, a manager-side stop -- with nothing at all to announce it.
    /// All three are answered by re-deriving the whole picture, which is also
    /// why a lost event cannot strand the process.
    pub fn tick(&self) {
        // Unconditional, and deliberately before the cheap early return: the
        // budget can be filled entirely by refusals, so a process whose
        // rotation only ran on a commit could sit permanently full.
        if let Err(error) = self.rotate_if_due() {
            tracing::warn!(
                ?error,
                "desired-watch rotation maintenance failed; will retry"
            );
        }
        let due = {
            let Ok(mut materialization) = self.lock_materialization() else {
                return;
            };
            if materialization.resetting {
                // A Full Reset that could not tear everything down is the one
                // barrier that must not simply be waited out: the rows are
                // already gone, so nothing else in the process will ever go
                // looking for the watches it still holds.
                if materialization.reset_incomplete
                    && let Err(error) = self.finish_authority_reset_locked(&mut materialization)
                {
                    tracing::warn!(?error, "the Full Reset teardown is unfinished; will retry");
                }
                return;
            }
            let now = Instant::now();
            let audience_changed =
                materialization.audience != self.owner.audience_connection_count();
            let revalidation_due = materialization
                .last_revalidated
                .is_none_or(|last| now.duration_since(last) >= self.revalidate_interval);
            let retry_due = materialization
                .sections
                .values()
                .any(|section| section.retry.is_some_and(|retry| retry.due_at <= now));
            let disarm_due = materialization.sections.values().any(|section| {
                section
                    .stopping
                    .is_some_and(|stopping| stopping.due_at <= now)
            });
            // A watch can end without anyone telling the coordinator. This is
            // the cheap half of noticing -- identity against the live set, no
            // storage touched -- and it is what turns a manager-side stop
            // into a reported failure instead of a permanent green light.
            let drifted = {
                let live = live_watches(self.owner.as_ref());
                materialization.sections.iter().any(|(section, state)| {
                    state
                        .armed
                        .as_ref()
                        .is_some_and(|armed| live.get(section) != Some(&armed.active_watch_id))
                })
            };
            audience_changed || revalidation_due || retry_due || disarm_due || drifted
        };
        if !due {
            return;
        }
        if let Err(error) = self.reconcile() {
            tracing::warn!(?error, "desired-watch reconcile failed; will retry");
        }
    }

    /// Enters the Full Reset barrier.
    ///
    /// From here until [`Self::finish_authority_reset`], no reconcile may arm
    /// anything. Taking the materialization lock is also what makes an
    /// in-flight reconcile safe: it either finished before this call, in
    /// which case whatever it armed is torn down at the end of the barrier,
    /// or it has not started, in which case it will find the barrier raised.
    /// There is no third case, because arming only ever happens under this
    /// lock.
    pub fn begin_authority_reset(&self) -> Result<(), DesiredWatchCoordinatorError> {
        let mut materialization = self.lock_materialization()?;
        materialization.resetting = true;
        materialization.reset_incomplete = false;
        Ok(())
    }

    /// Leaves the barrier: every physical watch the process still holds is
    /// stopped, every record is forgotten, and reconciling is allowed again.
    ///
    /// The teardown is driven by the OWNER's live set rather than by the
    /// records, so a watch this coordinator has no record of -- one armed by
    /// a reconcile that raced the start of the reset -- is stopped too. It is
    /// called on the failure path as well: a reset that could not clear the
    /// database must not leave the process holding watches it has forgotten
    /// how to address.
    ///
    /// The barrier comes down only on an EMPTY process. A teardown that
    /// failed leaves a watch that is still polling Rutgers and can still
    /// ring, and forgetting it here would produce the one state this whole
    /// module exists to prevent: an authority with nothing in it, a page with
    /// nothing to press, and a watch nobody can address. So the failure is
    /// reported, the identities are kept, and [`Self::tick`] comes back.
    pub fn finish_authority_reset(&self) -> Result<(), DesiredWatchCoordinatorError> {
        let mut materialization = self.lock_materialization()?;
        self.finish_authority_reset_locked(&mut materialization)
    }

    fn finish_authority_reset_locked(
        &self,
        materialization: &mut MutexGuard<'_, Materialization>,
    ) -> Result<(), DesiredWatchCoordinatorError> {
        let now = Instant::now();
        for target in self.owner.watch_targets() {
            if let Err(error) = self.owner.stop(target) {
                tracing::warn!(?error, "a physical watch outlived the Full Reset barrier");
            }
        }
        // The live set, re-read, is the answer -- not what the stops returned.
        // A stop that reported an error may still have taken the watch down,
        // and a stop that reported success cannot be trusted to have covered
        // a watch armed while this loop was running.
        let live = live_watches(self.owner.as_ref());
        if !live.is_empty() {
            // Re-derived from the live set rather than kept as they were:
            // what still has to come down is exactly what the owner still
            // holds, including a watch this coordinator never recorded.
            let previous = std::mem::take(&mut materialization.sections);
            materialization.sections = live
                .into_iter()
                .map(|(section, active_watch_id)| {
                    let attempt = previous
                        .get(&section)
                        .and_then(|state| state.stopping)
                        .filter(|stopping| stopping.active_watch_id == active_watch_id)
                        .map_or(0, |stopping| stopping.attempt + 1);
                    let state = SectionMaterialization {
                        stopping: Some(StoppingWatch {
                            active_watch_id,
                            attempt,
                            due_at: now + self.backoff[attempt.min(self.backoff.len() - 1)],
                        }),
                        ..SectionMaterialization::default()
                    };
                    (section, state)
                })
                .collect();
            materialization.reset_incomplete = true;
            return Err(DesiredWatchCoordinatorError::ResetIncomplete);
        }
        materialization.sections.clear();
        materialization.audience = self.owner.audience_connection_count();
        materialization.last_revalidated = None;
        materialization.reset_incomplete = false;
        materialization.resetting = false;
        Ok(())
    }

    /// Rotates the authority when either budget has reached its threshold.
    ///
    /// The coordinator is the production owner of this decision. Rotation
    /// raises the generation and renumbers every surviving row, so every
    /// page must re-read; doing it from a storage call on whichever caller
    /// happened to notice would produce several rotations for one crossing.
    ///
    /// The budget is read INSIDE the materialization lock and read again
    /// there even if the caller already looked. Two callers that both saw one
    /// crossing outside the lock would otherwise rotate twice, and the second
    /// rotation would raise the generation a second time for nothing --
    /// invalidating the `basedOnRevision` of every page that had just
    /// re-read after the first.
    ///
    /// Healthy watches are NOT restarted by it. Rotation changes the numbers
    /// an intent is stamped with, not the intent, so the reconcile that
    /// follows adopts each running watch under its new stamp: same watch id,
    /// same episode, no second announcement.
    ///
    /// Neither are unfinished teardowns forgotten by it. A tombstone whose
    /// section still has a physical watch is the only row the read has to
    /// report `pendingDisarm` on; purging it would take a watch that is still
    /// alive and can still ring off every page and out of every control at
    /// once. Which sections those are is known HERE and nowhere else, so the
    /// set is computed under the same lock that authorises the rotation and
    /// handed to the writer.
    pub fn rotate_if_due(&self) -> Result<bool, DesiredWatchCoordinatorError> {
        let mut materialization = self.lock_materialization()?;
        if !self.lock_store()?.desired_watch_budget()?.rotation_due() {
            return Ok(false);
        }
        // Between the answer above and the act below, nothing may enter the
        // domain -- and a test stands here to prove it, rather than to widen
        // the window.
        #[cfg(test)]
        self.arrive(DesiredWatchCheckpoint::RotationDue);
        // Which tombstones survive is decided by what the process is REALLY
        // holding, not by which label the materialization has had time to
        // move a section to.
        //
        // `stopping` alone is a record of teardowns this coordinator has
        // already tried and failed, and it is not the same question. A STOP
        // commits its tombstone with the store lock and then takes the
        // materialization lock to reconcile; a rotation that runs in between
        // sees the section still recorded as `armed`, finds nothing in
        // `stopping`, and purges the tombstone of a watch that is very much
        // alive. If the teardown that follows then fails, the watch goes on
        // polling Rutgers with no row, no `pendingDisarm` and no id any page
        // can reach.
        //
        // The live set answers both cases at once, and is read HERE, under
        // the lock that authorises the rotation, so nothing can arm between
        // the question and the act. The `stopping` records are unioned in
        // rather than replaced: they name watches this process could not
        // stop, and an owner that under-reports one for a moment must not be
        // the reason its row goes. The asymmetry is deliberate -- a watch
        // that ends just after this read costs one tombstone the next
        // rotation collects, while a live watch missed here costs the user a
        // watch nothing can ever stop.
        let preserve = live_watches(self.owner.as_ref())
            .into_keys()
            .chain(
                materialization
                    .sections
                    .iter()
                    .filter(|(_, state)| state.stopping.is_some())
                    .map(|(section, _)| section.clone()),
            )
            .collect::<BTreeSet<_>>();
        let authority = {
            let mut store = self.lock_store()?;
            store.rotate_desired_watch_authority(&preserve)?;
            store.desired_watch_authority()?
        };
        tracing::info!(
            generation = authority.counters.authority_generation,
            "rotated the desired-watch authority",
        );
        self.reconcile_locked(&mut materialization, &authority);
        Ok(true)
    }

    /// Stamps one mutation response from ONE authority snapshot.
    ///
    /// Every number a page could write with next comes from here, and from
    /// the same read: the generation it must present, the revision it must
    /// present for this section, and -- for a commit -- the rows it will
    /// render. The store's own answer is not used for any of them.
    ///
    /// Not "restamp only if this caller rotated". Whether THIS caller was the
    /// one to rotate says nothing about whether the authority moved: a second
    /// caller crossing the same threshold, or committing to the same section,
    /// moves it just as well, and a response assembled from the numbers read
    /// before that would hand the page a generation from one authority beside
    /// a revision from another. The page would then re-read with a pair no
    /// authority ever held, and be refused for as long as it kept trying.
    fn stamp(
        &self,
        result: &mut DesiredWatchMutationResultV1,
        section: &SectionKey,
    ) -> Result<(), DesiredWatchCoordinatorError> {
        let state = self.read()?;
        result.authority_generation = state.authority_generation;
        let entry = state.entries.iter().find(|entry| &entry.section == section);
        if result.outcome != DesiredWatchOutcomeV1::Committed {
            // A refusal answers with the numbers the page needs to re-read
            // with, so those numbers have to be the current ones. An absent
            // row reads as revision 0, exactly as the writer's own
            // compare-and-swap reads it.
            if result.current_revision.is_some() {
                result.current_revision = Some(entry.map_or(0, |entry| entry.revision));
            }
            return Ok(());
        }
        if let Some(committed) = result.committed.as_mut() {
            // `epochChanged` is a fact about the COMMIT -- did the desired
            // value change -- and rotation does not change it. The two
            // numbers are the ones rotation renumbers.
            //
            // An ABSENT row is a legal outcome of a commit, not a missing
            // one: a STOP that crosses the rotation threshold writes a
            // tombstone and then, in the same call, rotates it away, and a
            // concurrent caller can rotate between the decision and this
            // read. What must never happen is reporting the numbers the row
            // held before it went, because that pair describes an authority
            // that no longer exists -- a page comparing them against the
            // state in the same body would find no row holding them and
            // could only guess which half to believe. So absence has ONE
            // frozen shape, [`DESIRED_WATCH_ABSENT_COMMITTED_NUMBER`] in both
            // fields, and the decoder accepts nothing else for a missing row.
            match entry {
                Some(entry) => {
                    committed.revision = entry.revision;
                    committed.materialization_epoch = entry.materialization_epoch;
                }
                None => {
                    committed.revision = DESIRED_WATCH_ABSENT_COMMITTED_NUMBER;
                    committed.materialization_epoch = DESIRED_WATCH_ABSENT_COMMITTED_NUMBER;
                }
            }
        }
        result.state = Some(state);
        Ok(())
    }

    /// Stands at a named point inside the coordinator, for a test that has
    /// installed somewhere to stand. A shipping build does not compile this
    /// at all.
    #[cfg(test)]
    fn arrive(&self, checkpoint: DesiredWatchCheckpoint) {
        let Some(checkpoints) = self.checkpoints.as_ref() else {
            return;
        };
        // Asked of the mutex rather than passed in by the caller, so it
        // cannot be claimed: a thread that already holds it is refused here,
        // which is precisely the question -- is this point inside the
        // exclusive domain, or merely near it?
        let exclusive = self.materialization.try_lock().is_err();
        checkpoints.arrive(checkpoint, exclusive);
    }

    fn reconcile_locked(
        &self,
        materialization: &mut MutexGuard<'_, Materialization>,
        authority: &DesiredWatchAuthority,
    ) {
        if materialization.resetting {
            // A Full Reset owns the physical watches until it finishes.
            return;
        }
        let generation = authority.counters.authority_generation;
        materialization.audience = self.owner.audience_connection_count();

        // The owner is the truth about which physical watches EXIST and which
        // identity each one has; a record naming a watch the owner does not
        // hold under that id is stale, whatever caused it (a process stop, a
        // heartbeat sweep, a term rollover, a stop and a fresh start for the
        // same section). The record is the truth about which authority stamp
        // the watch was armed under.
        let live = live_watches(self.owner.as_ref());
        for (section, state) in materialization.sections.iter_mut() {
            if state
                .armed
                .as_ref()
                .is_some_and(|armed| live.get(section) != Some(&armed.active_watch_id))
            {
                state.armed = None;
                state.blocked_on_slot = false;
            }
            // A teardown whose watch is gone has achieved what it wanted,
            // however it got there. Compared by identity rather than by
            // presence: the same section running under a DIFFERENT id is not
            // this teardown finishing.
            if state
                .stopping
                .is_some_and(|stopping| live.get(section) != Some(&stopping.active_watch_id))
            {
                state.stopping = None;
            }
        }

        let now = Instant::now();

        // Every teardown that has not finished is retried from the identity
        // captured when the watch was armed, whatever the authority now says
        // about the section and whoever is looking. This is the only thing
        // that can finish one: nothing else in the process holds that id, and
        // the row it belongs to is no longer green, so no other pass will go
        // looking for it.
        let unfinished = materialization
            .sections
            .iter()
            .filter_map(|(section, state)| {
                state
                    .stopping
                    .filter(|stopping| stopping.due_at <= now)
                    .map(|stopping| (section.clone(), stopping.active_watch_id))
            })
            .collect::<Vec<_>>();
        for (section, active_watch_id) in unfinished {
            // Deliberately without clearing `failure` or `retry`: finishing a
            // teardown answers "is it still running", not "why was it taken
            // down", and the reason is what the page shows the user.
            self.disarm(materialization, &section, active_watch_id, now);
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
                self.retire(materialization, &section, active_watch_id, now);
            }
            // Only what actually came down is forgotten. A record whose
            // teardown FAILED still names a physical watch that may be alive
            // and may still ring; dropping it would leave the process holding
            // a watch it can no longer address -- every later arm for that
            // section would come back `AlreadyActive`, forever.
            materialization
                .sections
                .retain(|_, state| state.armed.is_some() || state.stopping.is_some());
            return;
        }

        let desired = authority
            .entries
            .iter()
            .filter(|entry| entry.policy.is_some())
            .map(|entry| (entry.section.clone(), entry))
            .collect::<BTreeMap<_, _>>();

        // Everything that IS running is re-asked whether it could be started
        // today. Arming proves a target was admissible once; nothing about
        // that proof survives the catalog dropping the section, the term
        // rolling over or the campus leaving the product. A watch whose
        // answer has changed is taken down here rather than reported green
        // until something else happens to notice.
        let revalidate = materialization
            .last_revalidated
            .is_none_or(|last| now.duration_since(last) >= self.revalidate_interval);
        if revalidate {
            materialization.last_revalidated = Some(now);
            let running = materialization
                .sections
                .iter()
                .filter(|(section, _)| desired.contains_key(*section))
                .filter_map(|(section, state)| {
                    state
                        .armed
                        .as_ref()
                        .map(|armed| (section.clone(), armed.active_watch_id))
                })
                .collect::<Vec<_>>();
            for (section, active_watch_id) in running {
                let Some(reason) = revoked_reason(self.owner.admission_for(&section)) else {
                    continue;
                };
                tracing::info!(
                    ?reason,
                    "a running desired watch is no longer admissible; taking it down",
                );
                // Only ever reached for a section the authority still wants:
                // `running` was filtered against `desired` above, and this is
                // the stamp the verdict below belongs to.
                let Some(entry) = desired.get(&section).copied() else {
                    continue;
                };
                self.disarm(materialization, &section, active_watch_id, now);
                let Some(state) = materialization.sections.get_mut(&section) else {
                    continue;
                };
                // NOT `state.armed = None` here. The teardown may itself have
                // failed, in which case `disarm` has already moved the id
                // into `stopping` -- the one place that keeps a still-live
                // watch addressable while keeping it out of the green light.
                // Zeroing the record on top of that would throw the only
                // address away and leave a watch that polls Rutgers forever,
                // answers every later arm with `AlreadyActive`, and cannot be
                // reached by the STOP the user presses.
                let classification = reason.classification();
                let retry_scheduled = classification == DesiredWatchFailureClassV1::Transient;
                record_failure(
                    state,
                    entry,
                    generation,
                    DesiredWatchFailureV1 {
                        classification,
                        reason,
                        retry_scheduled,
                    },
                );
                if retry_scheduled {
                    schedule_retry(state, entry, generation, now, &self.backoff);
                } else {
                    state.retry = None;
                }
            }
        }

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
            self.retire(materialization, &section, active_watch_id, now);
        }
        materialization.sections.retain(|section, state| {
            desired.contains_key(section) || state.armed.is_some() || state.stopping.is_some()
        });

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
                    match self.owner.update_policy(target, policy.clone()) {
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
                            // The edit failed, so the watch is still running
                            // under the OLD policy. The record keeps naming
                            // it -- that id is the only way anything can stop
                            // it later -- and keeps the old stamp, which no
                            // longer matches the authority, so the read
                            // reports preparing rather than watching. Zeroing
                            // the record here would leave a live watch the
                            // process could not address: every later arm
                            // would come back `AlreadyActive` and the STOP
                            // the user pressed would have nothing to name.
                            state.armed = Some(armed);
                            record_failure(
                                state,
                                entry,
                                generation,
                                DesiredWatchFailureV1 {
                                    classification: DesiredWatchFailureClassV1::Transient,
                                    reason: DesiredWatchFailureReasonV1::TargetUnavailable,
                                    retry_scheduled: true,
                                },
                            );
                            schedule_retry(state, entry, generation, now, &self.backoff);
                        }
                    }
                }
                None => {
                    // A `PERMANENT` verdict is only a reason not to try when
                    // it was reached about THIS intent. Read without its
                    // stamp it would attach to the section instead: a STOP, a
                    // fresh START, a policy edit or a rotation all leave the
                    // old conclusion in place, and the section could never be
                    // armed again for the life of the process.
                    let permanent = state.failure.is_some_and(|failure| {
                        failure.failure.classification == DesiredWatchFailureClassV1::Permanent
                            && failure.describes(
                                generation,
                                entry.revision,
                                entry.materialization_epoch,
                            )
                    });
                    let waiting = state.retry.is_some_and(|retry| {
                        retry.due_at > now
                            && retry.authority_generation == generation
                            && retry.revision == entry.revision
                            && retry.materialization_epoch == entry.materialization_epoch
                    });
                    // A retry scheduled under a stamp the authority has since
                    // moved past is not a reason to wait: the question has
                    // changed, so ask it again now. An unfinished teardown IS
                    // a reason to wait: the section still has a physical
                    // watch, and arming a second one would answer
                    // `AlreadyActive` and leave two ids for one section. It is
                    // also not permanent: the moment that teardown finishes,
                    // the next pass arms the new intent.
                    if !permanent && !waiting && state.stopping.is_none() {
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
            .and_then(|items| self.owner.start(items))
        {
            Ok(results) => results,
            Err(error) => {
                tracing::warn!(?error, "failed to arm desired watches");
                for (section, entry, _) in &to_arm {
                    let Some(state) = materialization.sections.get_mut(section) else {
                        continue;
                    };
                    record_failure(
                        state,
                        entry,
                        generation,
                        DesiredWatchFailureV1 {
                            classification: DesiredWatchFailureClassV1::Transient,
                            reason: DesiredWatchFailureReasonV1::TargetUnavailable,
                            retry_scheduled: true,
                        },
                    );
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
                    // A stable code the local console turns into a line in
                    // the user's language. Emitting rather than calling keeps
                    // the coordinator ignorant of consoles and locales.
                    tracing::info!(
                        code = "LOCAL_WATCH_ARMED",
                        section = %section_label(section),
                        "a desired watch is now running",
                    );
                    state.armed = Some(ArmedWatch {
                        authority_generation: generation,
                        revision: entry.revision,
                        materialization_epoch: entry.materialization_epoch,
                        policy: policy.clone(),
                        active_watch_id: *active_watch_id,
                    });
                    state.failure = None;
                    state.retry = None;
                    state.blocked_on_slot = false;
                }
                Some(WatchStartItemResultV1::Rejected { reason, .. }) => {
                    let reason = DesiredWatchFailureReasonV1::from_rejection(*reason);
                    let classification = reason.classification();
                    state.armed = None;
                    state.blocked_on_slot = reason == DesiredWatchFailureReasonV1::MaxActiveWatches;
                    let retry_scheduled = classification == DesiredWatchFailureClassV1::Transient;
                    record_failure(
                        state,
                        entry,
                        generation,
                        DesiredWatchFailureV1 {
                            classification,
                            reason,
                            retry_scheduled,
                        },
                    );
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
                    record_failure(
                        state,
                        entry,
                        generation,
                        DesiredWatchFailureV1 {
                            classification: DesiredWatchFailureClassV1::Transient,
                            reason: DesiredWatchFailureReasonV1::TargetUnavailable,
                            retry_scheduled: true,
                        },
                    );
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
    ///
    /// Whether it succeeded or not, the id leaves `armed`: a watch that is
    /// being torn down is not a watch the page may be shown a green light
    /// for. Where it goes is the whole difference. On success it is gone; on
    /// failure it moves to `stopping`, which reports the section as stopping,
    /// keeps it out of every arm decision, and keeps the one address that can
    /// still finish the job.
    ///
    /// Returns whether the watch is gone.
    fn disarm(
        &self,
        materialization: &mut MutexGuard<'_, Materialization>,
        section: &SectionKey,
        active_watch_id: ActiveWatchId,
        now: Instant,
    ) -> bool {
        let target = ActiveWatchTargetV1 {
            active_watch_id,
            section_key: section.clone(),
        };
        let outcome = self.owner.stop(target);
        let Some(state) = materialization.sections.get_mut(section) else {
            return matches!(outcome, Ok(()) | Err(WatchManagerError::UnknownWatch));
        };
        state.armed = None;
        state.blocked_on_slot = false;
        match outcome {
            // An unknown watch has already achieved what the teardown wanted.
            Ok(()) | Err(WatchManagerError::UnknownWatch) => {
                state.stopping = None;
                tracing::info!(
                    code = "LOCAL_WATCH_DISARMED",
                    section = %section_label(section),
                    "a desired watch is no longer running",
                );
                true
            }
            Err(error) => {
                // The physical watch may still be alive and may still ring,
                // so this is never reported as stopped -- and something has
                // to come back and finish the teardown, or the record sits at
                // `pendingDisarm` for the rest of the process's life. The
                // retry carries the same captured id, so however late it
                // fires it can only ever stop the watch it was meant to.
                tracing::warn!(?error, "failed to tear down a desired watch");
                let attempt = state.stopping.map_or(0, |stopping| stopping.attempt + 1);
                state.stopping = Some(StoppingWatch {
                    active_watch_id,
                    attempt,
                    due_at: now + self.backoff[attempt.min(self.backoff.len() - 1)],
                });
                false
            }
        }
    }

    /// Tears one physical watch down because the process no longer wants it
    /// there -- the user stopped it, or the last page left.
    ///
    /// The difference from a bare [`Self::disarm`] is what a SUCCESS means
    /// here: the section's problem and its retry described a watch that is
    /// now gone, so keeping them would make a page that comes back wait out a
    /// backoff scheduled for a question nobody is asking any more.
    fn retire(
        &self,
        materialization: &mut MutexGuard<'_, Materialization>,
        section: &SectionKey,
        active_watch_id: ActiveWatchId,
        now: Instant,
    ) {
        if !self.disarm(materialization, section, active_watch_id, now) {
            return;
        }
        let Some(state) = materialization.sections.get_mut(section) else {
            return;
        };
        state.failure = None;
        state.retry = None;
    }

    /// The transaction state of the coordinator's own connection, for the
    /// WAL starvation diagnostic. `None` when another owner holds the store
    /// right now: the diagnostic must never wait behind a mutation.
    pub fn store_transaction_state(
        &self,
    ) -> Option<Result<bcsp_local_user_state::PersonalTransactionState, PersonalStateError>> {
        self.store
            .try_lock()
            .ok()
            .map(|store| store.transaction_state())
    }

    fn lock_store(
        &self,
    ) -> Result<MutexGuard<'_, PersonalStateStore>, DesiredWatchCoordinatorError> {
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

/// Turns one storage decision into the shape of an answer.
///
/// Deliberately without reading the authority: every number a page could act
/// on is filled in later, from a single snapshot, by
/// [`DesiredWatchCoordinator::stamp`]. Reading here as well is what let a
/// generation from before a concurrent rotation reach the wire beside a
/// revision from after it.
fn describe(outcome: DesiredWatchMutationOutcome) -> DesiredWatchMutationResultV1 {
    let mut result = DesiredWatchMutationResultV1 {
        contract_version: LOCAL_DESIRED_WATCH_CONTRACT_VERSION,
        outcome: DesiredWatchOutcomeV1::Committed,
        replayed: false,
        authority_generation: 0,
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
    result
}

/// The physical watches the process holds right now, by section.
fn live_watches(owner: &dyn DesiredWatchOwner) -> BTreeMap<SectionKey, ActiveWatchId> {
    owner
        .watch_targets()
        .into_iter()
        .map(|target| (target.section_key, target.active_watch_id))
        .collect()
}

/// Why an already-running watch may no longer be run, or `None` when it may.
fn revoked_reason(admission: WatchStartAdmission) -> Option<DesiredWatchFailureReasonV1> {
    match admission {
        WatchStartAdmission::Admitted { .. } => None,
        WatchStartAdmission::SectionNotFound => Some(DesiredWatchFailureReasonV1::SectionNotFound),
        WatchStartAdmission::TargetUnavailable => {
            Some(DesiredWatchFailureReasonV1::TargetUnavailable)
        }
        WatchStartAdmission::UnsupportedTarget => {
            Some(DesiredWatchFailureReasonV1::UnsupportedTarget)
        }
        WatchStartAdmission::TermOutOfRange => Some(DesiredWatchFailureReasonV1::TermOutOfRange),
    }
}

/// Records why one intent could not be materialized, against the intent it
/// was decided about.
///
/// Always through here rather than by assigning the field, so a verdict
/// cannot be written without the stamp that bounds it -- which is the whole
/// difference between "this intent failed" and "this section is broken".
fn record_failure(
    state: &mut SectionMaterialization,
    entry: &DesiredWatchEntry,
    generation: u64,
    failure: DesiredWatchFailureV1,
) {
    state.failure = Some(StampedFailure {
        authority_generation: generation,
        revision: entry.revision,
        materialization_epoch: entry.materialization_epoch,
        failure,
    });
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
    live: &BTreeMap<SectionKey, ActiveWatchId>,
) -> DesiredWatchStateV1 {
    let generation = authority.counters.authority_generation;
    DesiredWatchStateV1 {
        contract_version: LOCAL_DESIRED_WATCH_CONTRACT_VERSION,
        authority_generation: generation,
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
                            .filter(|armed| {
                                live.get(&entry.section) == Some(&armed.active_watch_id)
                            })
                            .map(|armed| DesiredWatchMaterializedV1 {
                                authority_generation: armed.authority_generation,
                                revision: armed.revision,
                                materialization_epoch: armed.materialization_epoch,
                                policy: armed.policy.clone(),
                                active_watch_id: armed.active_watch_id,
                            })
                    }),
                    pending_disarm: state.is_some_and(|state| state.stopping.is_some()),
                    blocked_on_slot: state.is_some_and(|state| state.blocked_on_slot),
                    // A verdict about an intent the row has moved past is not
                    // this row's problem, and reporting it would put an
                    // explanation on the page for a question nobody asked --
                    // "cannot watch, needs your decision" on a section the
                    // user has just restarted, or on a tombstone. The
                    // reconcile asks again from scratch, and records the
                    // answer under the stamp that earned it.
                    failure: state
                        .and_then(|state| state.failure)
                        .filter(|failure| {
                            failure.describes(
                                generation,
                                entry.revision,
                                entry.materialization_epoch,
                            )
                        })
                        .map(|failure| failure.failure),
                }
            })
            .collect(),
    }
}

// ---------------------------------------------------------------------------
// Interleavings that only this module can stand inside
// ---------------------------------------------------------------------------

/// How a section reads on the console: term, campus and index, and nothing
/// else. It is what the user typed into the desk, so it is what they can
/// match a console line back to.
fn section_label(section: &SectionKey) -> String {
    format!(
        "{} {} {}",
        section.term().as_str(),
        section.campus().as_str(),
        section.index().as_str()
    )
}

/// The coordinator's own unit tests.
///
/// Everything about the desired-watch surface that can be reached through the
/// crate's public API is tested from `tests/desired_watch.rs`, against a real
/// SQLite file and a real shared socket, and belongs there. What lives here is
/// the small set of guarantees about WHEN one thing happens relative to
/// another inside a single call -- the rotation budget against the rotation it
/// authorises, a mutation's outcome against the snapshot its answer is stamped
/// from -- because reaching those points needs a seam a shipping build must
/// not have. Rather than widening the crate's API to let any caller install a
/// callback that runs while the materialization mutex is held, the tests that
/// need the seam sit next to it, and both are `cfg(test)`.
///
/// Nothing here sleeps. Each rendezvous asks a question the correct and the
/// broken shape answer differently, and blocks until the other caller has
/// actually reached the state that makes the interleaving real.
#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
    use std::sync::{Barrier, Mutex};

    use bcsp_contracts::WatchStartRejectionReason;
    use bcsp_local_user_state::DESIRED_WATCH_RECEIPT_ROTATION_THRESHOLD;
    use rusqlite::{Connection, params};
    use tempfile::TempDir;

    use super::*;

    /// A physical-watch owner with no socket behind it.
    ///
    /// These tests are about the authority's own ordering, so the owner only
    /// has to be truthful about identity: what it holds, under which id, and
    /// that a stop really removes it. Admission always says yes, because a
    /// revoked target is a different file's subject.
    #[derive(Default)]
    struct Owner {
        audience: AtomicUsize,
        live: Mutex<BTreeMap<SectionKey, ActiveWatchId>>,
        next: AtomicU64,
        /// Every teardown is refused, and the watch stays exactly where it
        /// was. This is the failure the coordinator cannot see coming and
        /// cannot undo: the id it captured is from then on the only thing in
        /// the process that can address a watch that is still polling.
        refuse_stops: AtomicBool,
        stops: Mutex<Vec<ActiveWatchTargetV1>>,
    }

    impl Owner {
        fn attach(&self) {
            self.audience.fetch_add(1, Ordering::SeqCst);
        }

        fn refuse_stops(&self, refuse: bool) {
            self.refuse_stops.store(refuse, Ordering::SeqCst);
        }

        /// A watch that ended without anyone telling the coordinator.
        ///
        /// A term rollover, a manager-side stop, a heartbeat sweep: the
        /// process is no longer holding it, and no teardown was ever asked
        /// for. Nothing is tearing down afterwards, which is what makes the
        /// section's tombstone ordinary removal history.
        fn lose(&self, section: &SectionKey) {
            self.live.lock().unwrap().remove(section);
        }

        fn teardowns_asked(&self) -> usize {
            self.stops.lock().unwrap().len()
        }

        fn stops_of(&self, section: &SectionKey) -> Vec<ActiveWatchId> {
            self.stops
                .lock()
                .unwrap()
                .iter()
                .filter(|target| &target.section_key == section)
                .map(|target| target.active_watch_id)
                .collect()
        }
    }

    impl DesiredWatchOwner for Owner {
        fn audience_connection_count(&self) -> usize {
            self.audience.load(Ordering::SeqCst)
        }

        fn watch_targets(&self) -> Vec<ActiveWatchTargetV1> {
            self.live
                .lock()
                .unwrap()
                .iter()
                .map(|(section_key, active_watch_id)| ActiveWatchTargetV1 {
                    active_watch_id: *active_watch_id,
                    section_key: section_key.clone(),
                })
                .collect()
        }

        fn admission_for(&self, _section: &SectionKey) -> WatchStartAdmission {
            WatchStartAdmission::admitted(None)
        }

        fn start(
            &self,
            items: WatchStartItemsV1,
        ) -> Result<Vec<WatchStartItemResultV1>, WatchManagerError> {
            let mut live = self.live.lock().unwrap();
            Ok(items
                .as_slice()
                .iter()
                .map(|item| {
                    let section_key = item.section_key.clone();
                    if live.contains_key(&section_key) {
                        return WatchStartItemResultV1::Rejected {
                            section_key,
                            reason: WatchStartRejectionReason::AlreadyActive,
                        };
                    }
                    let active_watch_id =
                        ActiveWatchId::new(watch_trace(self.next.fetch_add(1, Ordering::SeqCst)));
                    live.insert(section_key.clone(), active_watch_id);
                    WatchStartItemResultV1::Active {
                        section_key,
                        active_watch_id,
                        started_at: time::OffsetDateTime::UNIX_EPOCH,
                    }
                })
                .collect())
        }

        fn stop(&self, target: ActiveWatchTargetV1) -> Result<(), WatchManagerError> {
            self.stops.lock().unwrap().push(target.clone());
            if self.refuse_stops.load(Ordering::SeqCst) {
                // Refused, and the watch is still live. Not `UnknownWatch`,
                // which the coordinator is right to read as "already gone".
                return Err(WatchManagerError::TargetMismatch);
            }
            let mut live = self.live.lock().unwrap();
            match live.get(&target.section_key) {
                Some(id) if *id == target.active_watch_id => {
                    live.remove(&target.section_key);
                    Ok(())
                }
                _ => Err(WatchManagerError::UnknownWatch),
            }
        }

        fn update_policy(
            &self,
            _target: ActiveWatchTargetV1,
            _policy: WatchPolicyV1,
        ) -> Result<(), WatchManagerError> {
            Ok(())
        }
    }

    struct Fixture {
        _directory: TempDir,
        path: std::path::PathBuf,
        coordinator: Arc<DesiredWatchCoordinator>,
    }

    impl Fixture {
        fn new(checkpoints: Arc<dyn DesiredWatchCheckpoints>) -> Self {
            Self::owned(Arc::new(Owner::default()), checkpoints)
        }

        /// The same fixture over an owner the TEST already holds, for the
        /// cases whose question is about what the process was physically
        /// holding at a particular instant.
        fn owned(owner: Arc<Owner>, checkpoints: Arc<dyn DesiredWatchCheckpoints>) -> Self {
            let directory = TempDir::new().unwrap();
            let path = directory.path().join("rbcsp.sqlite");
            owner.attach();
            let coordinator = DesiredWatchCoordinator::with_owner(
                PersonalStateStore::open(&path).unwrap(),
                owner.clone(),
            )
            .with_retry_backoff(vec![Duration::ZERO])
            .with_revalidation_interval(Duration::ZERO)
            .with_checkpoints(checkpoints);
            Self {
                _directory: directory,
                path,
                coordinator: Arc::new(coordinator),
            }
        }

        fn read(&self) -> DesiredWatchStateV1 {
            self.coordinator.read().unwrap()
        }

        /// The id the section's physical watch is running under.
        fn armed_id(&self, section: &SectionKey) -> ActiveWatchId {
            entry_for(&self.read(), section)
                .and_then(|entry| entry.materialized.as_ref())
                .map(|materialized| materialized.active_watch_id)
                .expect("the section is armed")
        }

        fn submit(
            &self,
            section: &SectionKey,
            policy: Option<WatchPolicyV1>,
            based_on_revision: u64,
            authority_generation: u64,
            mutation: u64,
        ) -> DesiredWatchMutationResultV1 {
            self.coordinator
                .submit(&DesiredWatchMutationV1 {
                    contract_version: LOCAL_DESIRED_WATCH_CONTRACT_VERSION,
                    section: section.clone(),
                    policy,
                    based_on_revision,
                    authority_generation,
                    mutation_id: watch_trace(mutation),
                })
                .unwrap()
        }

        /// Fills the receipt ledger to the point where the next look at the
        /// budget says "rotate".
        fn make_rotation_due(&self) {
            let generation = self.read().authority_generation;
            let connection = Connection::open(&self.path).unwrap();
            let existing: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM personal_desired_watch_receipts_v1",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            let wanted =
                i64::try_from(DESIRED_WATCH_RECEIPT_ROTATION_THRESHOLD).unwrap() - existing;
            connection.execute_batch("BEGIN IMMEDIATE").unwrap();
            for index in 0..wanted.max(0) {
                connection
                    .execute(
                        "INSERT INTO personal_desired_watch_receipts_v1
                             (authority_generation, mutation_id, term_id, campus_code,
                              section_index, fingerprint, outcome_json)
                         VALUES (?1, ?2, 'T2026F', 'CAMPUS_A', '30001', ?3,
                                 '{\"outcome\":\"STALE_REVISION\",\"current\":1}')",
                        params![
                            i64::try_from(generation).unwrap(),
                            format!("00000000-0000-4000-8000-{:012x}", 0x10_0000 + index),
                            "b".repeat(64),
                        ],
                    )
                    .unwrap();
            }
            connection.execute_batch("COMMIT").unwrap();
        }
    }

    fn watch_trace(value: u64) -> TraceId {
        format!("00000000-0000-4000-8000-{value:012x}")
            .parse()
            .expect("deterministic UUIDv4")
    }

    fn section(index: u16) -> SectionKey {
        SectionKey::try_new("T2026F", "CAMPUS_A", &format!("{index:05}")).expect("SectionKey")
    }

    fn entry_for<'a>(
        state: &'a DesiredWatchStateV1,
        section: &SectionKey,
    ) -> Option<&'a DesiredWatchEntryV1> {
        state.entries.iter().find(|entry| &entry.section == section)
    }

    fn revision_of(state: &DesiredWatchStateV1, section: &SectionKey) -> u64 {
        entry_for(state, section).map_or(0, |entry| entry.revision)
    }

    fn armed(state: &DesiredWatchStateV1, section: &SectionKey) -> bool {
        entry_for(state, section).is_some_and(|entry| {
            entry.materialized.as_ref().is_some_and(|materialized| {
                materialized.authority_generation == state.authority_generation
                    && materialized.revision == entry.revision
                    && materialized.materialization_epoch == entry.materialization_epoch
                    && Some(&materialized.policy) == entry.policy.as_ref()
            })
        })
    }

    /// Concurrent callers, one threshold, one rotation.
    ///
    /// Rotation raises the generation, so a second one for the same crossing
    /// invalidates the `basedOnRevision` every page has just re-read with. The
    /// budget must therefore be read and acted on inside ONE exclusive domain:
    /// a caller that decides "due" outside it and then takes the lock will
    /// rotate again on an authority that is no longer due.
    ///
    /// Nothing here is timed. The rendezvous stands at the point BETWEEN the
    /// budget read and the rotation and asks a question the two shapes answer
    /// differently -- is this point inside the exclusive domain? The fixed
    /// shape arrives holding it, and cannot deadlock waiting for a second
    /// arrival because there cannot be one: the other caller is blocked on the
    /// mutex, and when it finally enters, the budget is no longer due and it
    /// never arrives at all. A check taken outside the domain arrives holding
    /// nothing, and the rendezvous then holds it until its partner has read
    /// the same due budget -- so the double rotation the old shape permits
    /// actually happens instead of depending on the scheduler.
    #[test]
    fn concurrent_callers_crossing_one_threshold_rotate_once() {
        /// Parks an arrival only when it is NOT inside the exclusive domain.
        struct Domain {
            arrivals: Mutex<Vec<bool>>,
            outside: Barrier,
        }

        impl DesiredWatchCheckpoints for Domain {
            fn arrive(&self, checkpoint: DesiredWatchCheckpoint, exclusive: bool) {
                if checkpoint != DesiredWatchCheckpoint::RotationDue {
                    return;
                }
                self.arrivals.lock().unwrap().push(exclusive);
                if exclusive {
                    // Inside the domain: nobody else can be here, so waiting
                    // for a second arrival would hang forever. Recorded and
                    // released.
                    return;
                }
                // Outside it. Hold until the other caller has read the same
                // due budget, so both act on it.
                self.outside.wait();
            }
        }

        let domain = Arc::new(Domain {
            arrivals: Mutex::new(Vec::new()),
            outside: Barrier::new(2),
        });
        let fixture = Fixture::new(domain.clone());
        assert_eq!(
            fixture
                .submit(&section(1), Some(WatchPolicyV1::default()), 0, 1, 1)
                .outcome,
            DesiredWatchOutcomeV1::Committed,
        );
        let generation = fixture.read().authority_generation;
        fixture.make_rotation_due();

        let workers = (0..2)
            .map(|_| {
                let coordinator = fixture.coordinator.clone();
                std::thread::spawn(move || coordinator.rotate_if_due().unwrap())
            })
            .collect::<Vec<_>>();
        let rotated = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .filter(|rotated| *rotated)
            .count();

        let arrivals = domain.arrivals.lock().unwrap().clone();
        assert_eq!(
            arrivals,
            vec![true],
            "the budget must be read once, inside the exclusive domain that acts on it",
        );
        assert_eq!(rotated, 1, "exactly one caller may rotate one crossing");
        assert_eq!(
            fixture.read().authority_generation,
            generation + 1,
            "and the generation moves exactly once",
        );
        assert!(
            armed(&fixture.read(), &section(1)),
            "and the running watch survived it",
        );
    }

    /// A response rotated by SOMEONE ELSE mid-call.
    ///
    /// This is the interleaving that "did I rotate?" cannot see. The outcome
    /// is decided against generation G; another caller crosses the threshold
    /// and rotates to G+1, renumbering every row; and only then is the answer
    /// assembled. A response stamped from what this caller read on the way in
    /// would pair G's revision with G+1's generation -- a pair no authority
    /// ever held, which the page would then be refused for as long as it kept
    /// presenting it.
    ///
    /// The interleaving is made to happen rather than waited for: the
    /// checkpoint fires after the store has decided and before anything is
    /// stamped, and the other caller is a real second thread doing a real
    /// rotation.
    #[test]
    fn a_terminal_refusal_rotated_by_another_caller_answers_from_one_authority() {
        let rendezvous = Arc::new(Handoff::new());
        let fixture = Fixture::new(rendezvous.clone());
        // Committed out of Section order, so the pre-rotation revision and
        // the post-rotation renumbering genuinely differ.
        assert_eq!(
            fixture
                .submit(&section(3), Some(WatchPolicyV1::default()), 0, 1, 1)
                .outcome,
            DesiredWatchOutcomeV1::Committed,
        );
        assert_eq!(
            fixture
                .submit(&section(1), Some(WatchPolicyV1::default()), 0, 1, 2)
                .outcome,
            DesiredWatchOutcomeV1::Committed,
        );
        let generation = fixture.read().authority_generation;
        let before = revision_of(&fixture.read(), &section(1));
        fixture.make_rotation_due();

        let other = rendezvous.spawn_rotation(&fixture);
        rendezvous.arm();
        // Stale by revision: terminal, and stamped after the other caller has
        // finished rotating.
        let result = fixture.submit(
            &section(1),
            Some(WatchPolicyV1::default()),
            99,
            generation,
            910,
        );
        other.join().unwrap();

        assert_eq!(result.outcome, DesiredWatchOutcomeV1::StaleRevision);
        assert_eq!(result.outcome.http_status(), 409);
        assert!(result.state.is_none());
        let after = fixture.read();
        assert_eq!(
            after.authority_generation,
            generation + 1,
            "somebody else rotated in the middle of this call",
        );
        assert_eq!(
            result.authority_generation, after.authority_generation,
            "and the answer is stamped from the authority that exists now",
        );
        let current = revision_of(&after, &section(1));
        assert_ne!(current, before, "the rotation really did renumber the row");
        assert_eq!(
            result.current_revision,
            Some(current),
            "the revision and the generation must come from the same authority",
        );
    }

    /// A COMMIT whose row is legitimately gone by the time it is stamped.
    ///
    /// The user stops a Section nothing is physically watching any more -- the
    /// watch ended underneath the process, so the tombstone that STOP writes
    /// is removal history the moment it is written. Another caller crosses the
    /// rotation threshold between the decision and the stamp and collects it.
    /// The commit still happened, so the answer is still `COMMITTED` -- but
    /// the numbers it reports must describe the authority it ships, and no row
    /// in that authority holds the pre-rotation pair. Reporting them anyway is
    /// a body that contradicts itself, and a strict decoder is right to refuse
    /// it. So absence has one frozen shape, and this is where it is produced.
    #[test]
    fn a_commit_purged_by_another_caller_before_it_is_stamped_reports_the_absent_shape() {
        let owner = Arc::new(Owner::default());
        let rendezvous = Arc::new(Handoff::watching(owner.clone()));
        let fixture = Fixture::owned(owner.clone(), rendezvous.clone());
        assert_eq!(
            fixture
                .submit(&section(1), Some(WatchPolicyV1::default()), 0, 1, 1)
                .outcome,
            DesiredWatchOutcomeV1::Committed,
        );
        let generation = fixture.read().authority_generation;
        let revision = revision_of(&fixture.read(), &section(1));
        fixture.make_rotation_due();
        // The watch is already gone, without a teardown and without anyone
        // telling the coordinator. This is the premise the absent shape
        // belongs to: there is nothing left for the tombstone to address.
        owner.lose(&section(1));

        let other = rendezvous.spawn_rotation(&fixture);
        rendezvous.arm();
        let result = fixture.submit(&section(1), None, revision, generation, 910);
        other.join().unwrap();

        assert_eq!(
            rendezvous.at_rotation(),
            AtRotation {
                live: BTreeMap::new(),
                teardowns_asked: 0,
            },
            "nothing physical was left for the rotation to protect",
        );
        assert_eq!(result.outcome, DesiredWatchOutcomeV1::Committed);
        let state = result.state.as_ref().expect("a commit carries its state");
        assert!(
            entry_for(state, &section(1)).is_none(),
            "the rotation purged a tombstone nothing was still tearing down",
        );
        let committed = result.committed.expect("a commit says what it wrote");
        assert_eq!(committed.revision, DESIRED_WATCH_ABSENT_COMMITTED_NUMBER);
        assert_eq!(
            committed.materialization_epoch,
            DESIRED_WATCH_ABSENT_COMMITTED_NUMBER,
        );
        assert_eq!(
            result.authority_generation, state.authority_generation,
            "the generation, the state and the committed numbers are one authority",
        );
        assert_eq!(
            state.authority_generation,
            generation + 1,
            "and it is the authority that exists now",
        );
    }

    /// A STOP whose tombstone is committed but not yet reconciled, and a
    /// rotation that runs in the gap.
    ///
    /// This is the interleaving a preserve set built from `stopping` cannot
    /// see. A STOP writes its tombstone under the store lock and releases it;
    /// only then does it take the materialization lock to reconcile. A
    /// rotation that gets there first finds the section still recorded as
    /// `armed` -- no teardown has been attempted, so nothing is in `stopping`
    /// -- and purges a tombstone whose physical watch is very much alive. The
    /// teardown that follows then fails, and the process is left holding a
    /// watch that polls Rutgers with no authority row, no `pendingDisarm`, no
    /// id any page can reach and no STOP anyone can press.
    ///
    /// The order is made, not waited for. The mutation parks at
    /// `MutationDecided` -- after its commit, before its reconcile -- and the
    /// second thread's rotation runs to completion from there, so the
    /// mutation cannot possibly have reconciled first. `AtRotation` records
    /// what was true inside that window rather than after it: a live watch,
    /// and not one teardown asked for.
    #[test]
    fn a_rotation_keeps_the_tombstone_of_a_stop_that_has_not_reconciled_yet() {
        let owner = Arc::new(Owner::default());
        let rendezvous = Arc::new(Handoff::watching(owner.clone()));
        let fixture = Fixture::owned(owner.clone(), rendezvous.clone());
        assert_eq!(
            fixture
                .submit(&section(1), Some(WatchPolicyV1::default()), 0, 1, 1)
                .outcome,
            DesiredWatchOutcomeV1::Committed,
        );
        let original = fixture.armed_id(&section(1));
        let generation = fixture.read().authority_generation;
        let revision = revision_of(&fixture.read(), &section(1));
        fixture.make_rotation_due();
        // The teardown will not succeed. Without it the bug is invisible: a
        // purged tombstone and a watch that really stopped agree.
        owner.refuse_stops(true);

        let other = rendezvous.spawn_rotation(&fixture);
        rendezvous.arm();
        let result = fixture.submit(&section(1), None, revision, generation, 910);
        other.join().unwrap();

        assert_eq!(
            rendezvous.at_rotation(),
            AtRotation {
                live: BTreeMap::from([(section(1), original)]),
                teardowns_asked: 0,
            },
            "the rotation was authorised while the watch was still live, \
             and before anything had tried to stop it -- so `stopping` \
             was empty and only the live set could have saved the row",
        );

        assert_eq!(result.outcome, DesiredWatchOutcomeV1::Committed);
        let state = result.state.as_ref().expect("a commit carries its state");
        assert_eq!(
            state.authority_generation,
            generation + 1,
            "the other caller really did rotate in the middle of this call",
        );
        let row = entry_for(state, &section(1)).expect("a live watch keeps its row");
        assert!(row.policy.is_none(), "the user did ask for it to stop");
        assert!(
            row.pending_disarm,
            "and a STOP is not finished while the watch it names can still ring",
        );
        assert!(row.materialized.is_none(), "a teardown is never green");
        let committed = result.committed.expect("a commit says what it wrote");
        assert_eq!(
            (committed.revision, committed.materialization_epoch),
            (row.revision, row.materialization_epoch),
            "the surviving row is renumbered into the new generation, and \
             the answer describes it rather than the absent shape",
        );
        assert_ne!(
            committed.revision, DESIRED_WATCH_ABSENT_COMMITTED_NUMBER,
            "a row that is still there was not legitimately collected",
        );
        assert_eq!(
            live_watches(owner.as_ref()),
            BTreeMap::from([(section(1), original)]),
            "and the watch really is still running under the id it was \
             armed with",
        );

        // Every page reads the same row, not just the caller that stopped it.
        let read = fixture.read();
        assert_eq!(read.authority_generation, state.authority_generation);
        let visible = entry_for(&read, &section(1)).expect("the desk can still see it");
        assert!(visible.pending_disarm);
        assert!(visible.materialized.is_none());

        // Clearing the fault finishes the teardown by the ORIGINAL id, and
        // only then is the row ordinary removal history.
        owner.refuse_stops(false);
        fixture.coordinator.tick();
        assert!(
            live_watches(owner.as_ref()).is_empty(),
            "the captured id is the only thing that could still stop it",
        );
        assert!(
            !owner.stops_of(&section(1)).is_empty()
                && owner.stops_of(&section(1)).iter().all(|id| *id == original),
            "and every teardown named the watch it was meant to",
        );
        let settled = fixture.read();
        assert!(
            !entry_for(&settled, &section(1))
                .expect("still removal history")
                .pending_disarm,
        );
    }

    /// What the process could say about itself at the instant a rotation was
    /// authorised.
    ///
    /// `teardowns_asked` is what separates the two interleavings that look
    /// alike from outside. A section can only be in `stopping` because a
    /// teardown was already attempted and refused; zero attempts means the
    /// materialization still called this section `armed`, and the tombstone
    /// the rotation is about to see was committed by a STOP that has not
    /// reached its reconcile.
    #[derive(Clone, Debug, Eq, PartialEq)]
    struct AtRotation {
        live: BTreeMap<SectionKey, ActiveWatchId>,
        teardowns_asked: usize,
    }

    /// A blocking hand-off from inside `MutationDecided` to a second thread.
    ///
    /// Both directions block, so the other caller's rotation is COMPLETE
    /// before the mutation goes on to assemble its answer. `armed` exists
    /// because the setup commits reach the same checkpoint and only one of
    /// them is the interleaving under test.
    struct Handoff {
        armed: AtomicBool,
        /// What the process was physically holding at the instant the
        /// rotation was authorised, when a test asked to be told.
        ///
        /// Recorded rather than asserted from outside, because the whole
        /// question is what was true INSIDE that window: by the time either
        /// thread has finished, the mutation has reconciled and the answer
        /// would be about a different moment.
        observer: Option<Arc<Owner>>,
        at_rotation: Mutex<Option<AtRotation>>,
        rotate: SyncSender<()>,
        rotate_rx: Mutex<Option<Receiver<()>>>,
        rotated_tx: SyncSender<u64>,
        rotated: Mutex<Receiver<u64>>,
    }

    impl Handoff {
        fn new() -> Self {
            Self::observing(None)
        }

        /// A hand-off that also reports the owner's live set from inside
        /// `RotationDue`.
        fn watching(owner: Arc<Owner>) -> Self {
            Self::observing(Some(owner))
        }

        fn observing(observer: Option<Arc<Owner>>) -> Self {
            let (rotate, rotate_rx) = sync_channel(0);
            let (rotated_tx, rotated) = sync_channel(0);
            Self {
                armed: AtomicBool::new(false),
                observer,
                at_rotation: Mutex::new(None),
                rotate,
                rotate_rx: Mutex::new(Some(rotate_rx)),
                rotated_tx,
                rotated: Mutex::new(rotated),
            }
        }

        fn arm(&self) {
            self.armed.store(true, Ordering::SeqCst);
        }

        fn at_rotation(&self) -> AtRotation {
            self.at_rotation
                .lock()
                .unwrap()
                .clone()
                .expect("the rotation reached its checkpoint")
        }

        fn spawn_rotation(&self, fixture: &Fixture) -> std::thread::JoinHandle<()> {
            let coordinator = fixture.coordinator.clone();
            let rotate_rx = self.rotate_rx.lock().unwrap().take().expect("one rotation");
            let rotated_tx = self.rotated_tx.clone();
            std::thread::spawn(move || {
                rotate_rx.recv().expect("a mutation reached its checkpoint");
                assert!(
                    coordinator.rotate_if_due().expect("rotate"),
                    "the budget was due",
                );
                let generation = coordinator.read().unwrap().authority_generation;
                rotated_tx
                    .send(generation)
                    .expect("the mutation is waiting");
            })
        }
    }

    impl DesiredWatchCheckpoints for Handoff {
        fn arrive(&self, checkpoint: DesiredWatchCheckpoint, exclusive: bool) {
            if checkpoint == DesiredWatchCheckpoint::RotationDue {
                let Some(owner) = self.observer.as_ref() else {
                    return;
                };
                assert!(
                    exclusive,
                    "the set a rotation preserves must be decided inside the domain",
                );
                *self.at_rotation.lock().unwrap() = Some(AtRotation {
                    live: live_watches(owner.as_ref()),
                    teardowns_asked: owner.teardowns_asked(),
                });
                return;
            }
            if checkpoint != DesiredWatchCheckpoint::MutationDecided
                || !self.armed.swap(false, Ordering::SeqCst)
            {
                return;
            }
            assert!(
                !exclusive,
                "a mutation must not be decided while holding the domain a reconcile needs",
            );
            self.rotate.send(()).expect("the other caller is listening");
            self.rotated
                .lock()
                .unwrap()
                .recv()
                .expect("the other caller rotated");
        }
    }
}
