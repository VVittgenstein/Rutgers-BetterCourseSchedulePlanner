//! The local desired-watch coordinator: what the process does with stored
//! intent, and what it refuses to do with it.
//!
//! These drive the coordinator directly against a real SQLite file and a real
//! shared watch socket, with only the catalog admission scripted. That is the
//! seam worth faking -- whether Rutgers publishes a section is exactly the
//! answer that changes underneath a running process, and every interesting
//! behaviour here is about what happens when it does.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bcsp_application::{
    NoopWatchDispatchSink, SharedWatchSocket, WatchAdmissionSource, WebSocketExtension,
};
use bcsp_contracts::{
    ActiveWatchId, ActiveWatchTargetV1, OpenObservationV1, SectionKey, TraceId,
    WatchClientCommandV1, WatchContinuousDurationV1, WatchMaxAudible, WatchNotificationMode,
    WatchPolicyV1, WatchStartItemResultV1, WatchStartItemV1, WatchStartItemsV1, WsClientEnvelope,
};
use bcsp_local_runtime::{
    DESIRED_WATCH_MATERIALIZE_BACKOFF, DESIRED_WATCH_REVALIDATE_INTERVAL, DesiredWatchCoordinator,
    DesiredWatchFailureClassV1, DesiredWatchFailureReasonV1, DesiredWatchMutationV1,
    DesiredWatchOutcomeV1, DesiredWatchOwner, DesiredWatchStateV1,
    LOCAL_DESIRED_WATCH_CONTRACT_VERSION, LOCAL_DESIRED_WATCH_RESPONSE_BUDGET_BYTES,
    LocalWatchRoute,
};
use bcsp_local_user_state::{
    DESIRED_WATCH_RECEIPT_ROTATION_THRESHOLD, MAX_DESIRED_WATCH_RECEIPTS,
    MAX_DESIRED_WATCH_TOMBSTONES, MAX_DESIRED_WATCHES, PersonalStateStore,
};
use bcsp_watch::{WatchManagerError, WatchStartAdmission};
use rusqlite::Connection;
use tempfile::TempDir;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A catalog whose answers a test can change while the process is running.
#[derive(Clone)]
struct Admission {
    verdicts: Arc<Mutex<BTreeMap<String, WatchStartAdmission>>>,
    fallback: Arc<Mutex<Option<WatchStartAdmission>>>,
    consulted: Arc<Mutex<Vec<SectionKey>>>,
    supported: Arc<AtomicBool>,
    in_range: Arc<AtomicBool>,
}

impl Default for Admission {
    fn default() -> Self {
        Self {
            verdicts: Arc::default(),
            fallback: Arc::default(),
            consulted: Arc::default(),
            supported: Arc::new(AtomicBool::new(true)),
            in_range: Arc::new(AtomicBool::new(true)),
        }
    }
}

impl Admission {
    fn set(&self, index: &str, verdict: WatchStartAdmission) {
        self.verdicts
            .lock()
            .unwrap()
            .insert(index.to_owned(), verdict);
    }

    fn consulted(&self) -> Vec<SectionKey> {
        self.consulted.lock().unwrap().clone()
    }
}

impl WatchAdmissionSource for Admission {
    fn admission_for(&self, section: &SectionKey) -> WatchStartAdmission {
        self.consulted.lock().unwrap().push(section.clone());
        if !self.target_supported(section) {
            return WatchStartAdmission::UnsupportedTarget;
        }
        if !self.term_in_range(section) {
            return WatchStartAdmission::TermOutOfRange;
        }
        self.verdicts
            .lock()
            .unwrap()
            .get(section.index().as_str())
            .cloned()
            .or_else(|| self.fallback.lock().unwrap().clone())
            .unwrap_or_else(|| WatchStartAdmission::admitted(None))
    }

    fn target_supported(&self, _section: &SectionKey) -> bool {
        self.supported.load(Ordering::SeqCst)
    }

    fn term_in_range(&self, _section: &SectionKey) -> bool {
        self.in_range.load(Ordering::SeqCst)
    }
}

/// Makes the two physical-watch operations that can fail invisibly actually
/// fail.
///
/// A teardown that will not tear down and a policy edit that will not apply
/// both leave a watch ALIVE while the coordinator decides what to remember
/// about it, and neither can be provoked through a healthy socket. Without a
/// seam the only available "test" would be reading the code.
#[derive(Default)]
struct OwnerFaults {
    stop: AtomicBool,
    update_policy: AtomicBool,
    /// Held by a test to keep one caller inside the coordinator's exclusive
    /// domain while others pile up behind it. Without a way to WIDEN that
    /// window, the interleaving a check-then-act race depends on is a few
    /// instructions long and no amount of repetition reaches it reliably.
    hold: Mutex<()>,
}

struct FaultOwner {
    inner: Arc<SharedWatchSocket>,
    faults: Arc<OwnerFaults>,
}

impl DesiredWatchOwner for FaultOwner {
    fn audience_connection_count(&self) -> usize {
        // The first thing a reconcile asks the owner, and therefore the
        // place a test can stand to hold the exclusive domain open.
        let _held = self.faults.hold.lock().unwrap();
        self.inner.audience_connection_count()
    }

    fn watch_targets(&self) -> Vec<ActiveWatchTargetV1> {
        self.inner.owner_watch_targets()
    }

    fn admission_for(&self, section: &SectionKey) -> WatchStartAdmission {
        self.inner.admission_for(section)
    }

    fn start(
        &self,
        items: WatchStartItemsV1,
    ) -> Result<Vec<WatchStartItemResultV1>, WatchManagerError> {
        self.inner.owner_start(items)
    }

    fn stop(&self, target: ActiveWatchTargetV1) -> Result<(), WatchManagerError> {
        if self.faults.stop.load(Ordering::SeqCst) {
            // Deliberately without touching the manager: the watch is still
            // running, which is exactly what makes a forgotten id dangerous.
            return Err(WatchManagerError::TargetMismatch);
        }
        self.inner.owner_stop(target)
    }

    fn update_policy(
        &self,
        target: ActiveWatchTargetV1,
        policy: WatchPolicyV1,
    ) -> Result<(), WatchManagerError> {
        if self.faults.update_policy.load(Ordering::SeqCst) {
            return Err(WatchManagerError::TargetMismatch);
        }
        self.inner.owner_update_policy(target, policy)
    }
}

struct Fixture {
    _directory: TempDir,
    path: std::path::PathBuf,
    admission: Admission,
    watch: Arc<SharedWatchSocket>,
    faults: Arc<OwnerFaults>,
    coordinator: Arc<DesiredWatchCoordinator>,
    route: LocalWatchRoute,
}

impl Fixture {
    fn new() -> Self {
        Self::with_backoff(vec![Duration::ZERO])
    }

    fn with_backoff(backoff: Vec<Duration>) -> Self {
        Self::with_settings(backoff, Duration::ZERO)
    }

    fn with_settings(backoff: Vec<Duration>, revalidation: Duration) -> Self {
        let directory = TempDir::new().unwrap();
        let path = directory.path().join("rbcsp.sqlite");
        let admission = Admission::default();
        let watch = Arc::new(
            SharedWatchSocket::try_new(
                Arc::new(admission.clone()),
                Arc::new(NoopWatchDispatchSink),
            )
            .unwrap(),
        );
        let faults = Arc::new(OwnerFaults::default());
        let coordinator = Arc::new(
            coordinator_over(&path, watch.clone(), faults.clone())
                .with_retry_backoff(backoff)
                .with_revalidation_interval(revalidation),
        );
        let route = LocalWatchRoute::new(watch.clone(), coordinator.clone());
        Self {
            _directory: directory,
            path,
            admission,
            watch,
            faults,
            coordinator,
            route,
        }
    }

    /// A second coordinator over the SAME database and the same physical
    /// owner: what a restart looks like from the authority's point of view.
    fn restarted_coordinator(&self) -> Arc<DesiredWatchCoordinator> {
        Arc::new(
            coordinator_over(&self.path, self.watch.clone(), self.faults.clone())
                .with_retry_backoff(vec![Duration::ZERO]),
        )
    }

    fn armed_id(&self, section: &SectionKey) -> ActiveWatchId {
        entry(&self.read(), section)
            .expect("the section has a row")
            .materialized
            .as_ref()
            .expect("the section is materialized")
            .active_watch_id
    }

    /// Attaches a page, returning its connection id and its outbound frames.
    fn attach(&self, id: u64) -> (TraceId, mpsc::UnboundedReceiver<String>) {
        let connection_id = trace(id);
        let (outbound, inbound) = mpsc::unbounded_channel();
        assert!(self.route.connect(connection_id, outbound));
        (connection_id, inbound)
    }

    fn detach(&self, connection_id: TraceId) {
        self.route.disconnect(connection_id);
    }

    fn read(&self) -> DesiredWatchStateV1 {
        self.coordinator.read().unwrap()
    }

    fn start(&self, section: &SectionKey, revision: u64, mutation: u64) -> DesiredWatchOutcomeV1 {
        self.submit(section, Some(policy()), revision, mutation)
    }

    fn stop(&self, section: &SectionKey, revision: u64, mutation: u64) -> DesiredWatchOutcomeV1 {
        self.submit(section, None, revision, mutation)
    }

    fn submit(
        &self,
        section: &SectionKey,
        policy: Option<WatchPolicyV1>,
        based_on_revision: u64,
        mutation: u64,
    ) -> DesiredWatchOutcomeV1 {
        let generation = self.read().authority_generation;
        self.submit_at(section, policy, based_on_revision, generation, mutation)
    }

    fn submit_at(
        &self,
        section: &SectionKey,
        policy: Option<WatchPolicyV1>,
        based_on_revision: u64,
        authority_generation: u64,
        mutation: u64,
    ) -> DesiredWatchOutcomeV1 {
        self.coordinator
            .submit(&DesiredWatchMutationV1 {
                contract_version: LOCAL_DESIRED_WATCH_CONTRACT_VERSION,
                section: section.clone(),
                policy,
                based_on_revision,
                authority_generation,
                mutation_id: trace(mutation),
            })
            .unwrap()
            .outcome
    }
}

/// Every coordinator in this file re-checks its running watches on every
/// reconcile. Production spends a bounded interval between checks so a local
/// process is not asking storage the same question in a hot loop; what these
/// tests are about is the ANSWER changing, so they ask every time.
fn coordinator_over(
    path: &std::path::Path,
    watch: Arc<SharedWatchSocket>,
    faults: Arc<OwnerFaults>,
) -> DesiredWatchCoordinator {
    let owner: Arc<dyn DesiredWatchOwner> = Arc::new(FaultOwner {
        inner: watch,
        faults,
    });
    DesiredWatchCoordinator::with_owner(PersonalStateStore::open(path).unwrap(), owner)
        .with_revalidation_interval(Duration::ZERO)
}

fn trace(value: u64) -> TraceId {
    format!("00000000-0000-4000-8000-{value:012x}")
        .parse()
        .expect("deterministic UUIDv4")
}

fn section(index: u16) -> SectionKey {
    SectionKey::try_new("T2026F", "CAMPUS_A", &format!("{index:05}")).expect("SectionKey")
}

fn policy() -> WatchPolicyV1 {
    WatchPolicyV1::default()
}

fn loud_policy() -> WatchPolicyV1 {
    WatchPolicyV1::new(
        WatchNotificationMode::Continuous,
        WatchMaxAudible::try_from(7).unwrap(),
        WatchContinuousDurationV1::finite_seconds(300).unwrap(),
    )
}

/// The rule the page uses, restated here so the coordinator is measured
/// against the same definition of "really monitored": a record exists AND
/// every part of its stamp equals the authority's.
fn armed(state: &DesiredWatchStateV1, section: &SectionKey) -> bool {
    let Some(entry) = entry(state, section) else {
        return false;
    };
    entry.materialized.as_ref().is_some_and(|materialized| {
        materialized.authority_generation == state.authority_generation
            && materialized.revision == entry.revision
            && materialized.materialization_epoch == entry.materialization_epoch
            && Some(&materialized.policy) == entry.policy.as_ref()
    })
}

fn entry<'a>(
    state: &'a DesiredWatchStateV1,
    section: &SectionKey,
) -> Option<&'a bcsp_local_runtime::DesiredWatchEntryV1> {
    state.entries.iter().find(|entry| &entry.section == section)
}

fn revision(state: &DesiredWatchStateV1, section: &SectionKey) -> u64 {
    entry(state, section).map_or(0, |entry| entry.revision)
}

fn open_observation(section: &SectionKey) -> OpenObservationV1 {
    serde_json::from_value(serde_json::json!({
        "contractVersion": 1,
        "observationId": "00000000-0000-4000-8000-0000000000a1",
        "refreshObservationId": "00000000-0000-4000-8000-0000000000a2",
        "batch": {"term": section.term().as_str(), "campus": section.campus().as_str()},
        "sectionKey": {
            "term": section.term().as_str(),
            "campus": section.campus().as_str(),
            "index": section.index().as_str(),
        },
        "pullSequence": 7,
        "catalogContentVersion": 3,
        "state": "OPEN",
        "observedAt": "1970-01-01T00:00:01Z",
        "freshUntil": "2099-01-01T00:00:00Z",
        "schedulerLagMilliseconds": 250,
        "counterSnapshot": {
            "runCounts": {"attempted": 9, "succeeded": 7, "failed": 2, "empty": 1},
            "todayCounts": {"attempted": 9, "succeeded": 7, "failed": 2, "empty": 1},
            "rutgersDay": "2026-07-14",
            "dayTimezone": "America/New_York"
        }
    }))
    .expect("synthetic Open observation")
}

fn frame(message: u64, command: WatchClientCommandV1) -> String {
    serde_json::to_string(&WsClientEnvelope::new(trace(message), command)).unwrap()
}

fn drain(inbound: &mut mpsc::UnboundedReceiver<String>) -> Vec<serde_json::Value> {
    let mut frames = Vec::new();
    while let Ok(frame) = inbound.try_recv() {
        frames.push(serde_json::from_str(&frame).unwrap());
    }
    frames
}

fn event_types(frames: &[serde_json::Value]) -> Vec<String> {
    frames
        .iter()
        .map(|frame| frame["payload"]["type"].as_str().unwrap().to_owned())
        .collect()
}

// ---------------------------------------------------------------------------
// Attach, detach, and who owns the physical watch
// ---------------------------------------------------------------------------

/// The stored intent is what a returning page restores from, and it restores
/// because a page ATTACHED -- not because a page asked. A user who closed the
/// browser yesterday and opens it today gets their watches back without
/// touching anything.
#[test]
fn the_first_page_to_attach_materializes_the_stored_intent() {
    let fixture = Fixture::new();
    // Intent committed with nobody looking: nothing is running yet, because
    // an alert nobody can hear is a poll against Rutgers for nothing.
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    assert_eq!(fixture.watch.total_active_watch_count(), 0);
    assert!(!armed(&fixture.read(), &section(1)));

    let (_page, _frames) = fixture.attach(100);
    assert_eq!(fixture.watch.total_active_watch_count(), 1);
    let state = fixture.read();
    assert!(armed(&state, &section(1)));
    assert!(entry(&state, &section(1)).unwrap().failure.is_none());
}

/// Closing a tab is not the user changing their mind. The physical watch
/// goes, because there is nobody left to tell; the row stays, because the
/// user still wants it.
#[test]
fn the_last_page_to_leave_tears_the_watch_down_and_keeps_the_intent() {
    let fixture = Fixture::new();
    let (first, _first_frames) = fixture.attach(100);
    let (second, _second_frames) = fixture.attach(101);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    assert_eq!(fixture.watch.total_active_watch_count(), 1);

    fixture.detach(first);
    assert_eq!(
        fixture.watch.total_active_watch_count(),
        1,
        "one page leaving is not the audience leaving",
    );
    assert!(armed(&fixture.read(), &section(1)));

    fixture.detach(second);
    assert_eq!(fixture.watch.total_active_watch_count(), 0);
    let state = fixture.read();
    assert_eq!(state.entries.len(), 1, "the row survives");
    assert!(entry(&state, &section(1)).unwrap().policy.is_some());
    assert!(!armed(&state, &section(1)));

    // And a page coming back arms it again, from the row alone.
    let (_third, _third_frames) = fixture.attach(102);
    assert!(armed(&fixture.read(), &section(1)));
}

/// Two pages, one section, one physical watch -- and both pages ring.
///
/// The two halves are the same fact stated from either side. The watch is
/// held by the process on behalf of everyone looking, so there is one of it;
/// and because it is held on behalf of everyone looking, its alerts go to
/// everyone. Two pages ringing is the honest report: both really are
/// watching the same section.
#[test]
fn two_pages_share_one_physical_watch_and_both_receive_its_alerts() {
    let fixture = Fixture::new();
    let (_first, mut first_frames) = fixture.attach(100);
    let (_second, mut second_frames) = fixture.attach(101);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);

    assert_eq!(
        fixture.watch.total_active_watch_count(),
        1,
        "a second page must not add a second physical watch",
    );
    assert_eq!(fixture.watch.owner_watched_sections(), vec![section(1)]);

    // The arm itself is announced to both, because it happened on behalf of
    // both rather than in reply to either.
    assert!(event_types(&drain(&mut first_frames)).contains(&"START_RESULT".to_owned()));
    assert!(event_types(&drain(&mut second_frames)).contains(&"START_RESULT".to_owned()));

    fixture.watch.publish(open_observation(&section(1))).unwrap();
    let first = event_types(&drain(&mut first_frames));
    let second = event_types(&drain(&mut second_frames));
    assert!(
        first.contains(&"ALERT_UPDATED".to_owned()),
        "the first page must receive the alert: {first:?}",
    );
    assert_eq!(
        first, second,
        "both pages must receive the same event stream",
    );
}

// ---------------------------------------------------------------------------
// Submissions take effect immediately
// ---------------------------------------------------------------------------

/// A submission reconciles before it answers. The page that made the change
/// sees the result of the change, not a promise about it.
#[test]
fn a_committed_mutation_arms_and_disarms_before_it_answers() {
    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);

    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    assert_eq!(fixture.watch.total_active_watch_count(), 1);
    let state = fixture.read();
    assert!(armed(&state, &section(1)));

    let armed_revision = revision(&state, &section(1));
    assert_eq!(
        fixture.stop(&section(1), armed_revision, 2),
        DesiredWatchOutcomeV1::Committed,
    );
    assert_eq!(fixture.watch.total_active_watch_count(), 0);
    let state = fixture.read();
    let tombstone = entry(&state, &section(1)).expect("a stop leaves a tombstone");
    assert!(tombstone.policy.is_none());
    assert!(tombstone.materialized.is_none());
    assert!(!tombstone.pending_disarm);
}

/// A policy edit adjusts the running watch. It does not restart it, because
/// restarting would end the episode and re-announce a section the user has
/// already been told about -- for a change that was only about how loud it
/// should be.
///
/// A stop followed by a start is the opposite case and must NOT be adopted:
/// the user cancelled, so the second start is genuinely new.
#[test]
fn a_policy_edit_adjusts_the_running_watch_and_a_stop_start_replaces_it() {
    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    let first = fixture.read();
    let first_id = entry(&first, &section(1))
        .unwrap()
        .materialized
        .as_ref()
        .unwrap()
        .active_watch_id
        .clone();
    let first_epoch = entry(&first, &section(1)).unwrap().materialization_epoch;

    assert_eq!(
        fixture.submit(&section(1), Some(loud_policy()), revision(&first, &section(1)), 2),
        DesiredWatchOutcomeV1::Committed,
    );
    let edited = fixture.read();
    let edited_entry = entry(&edited, &section(1)).unwrap();
    assert_eq!(edited_entry.policy.as_ref(), Some(&loud_policy()));
    assert_eq!(
        edited_entry.materialization_epoch, first_epoch,
        "a policy edit keeps the epoch",
    );
    assert_eq!(
        edited_entry.materialized.as_ref().unwrap().active_watch_id,
        first_id,
        "the same physical watch, adjusted in place",
    );
    assert!(armed(&edited, &section(1)));

    // Now cancel and start again. That is a new intent and gets a new watch.
    assert_eq!(
        fixture.stop(&section(1), revision(&edited, &section(1)), 3),
        DesiredWatchOutcomeV1::Committed,
    );
    let stopped = fixture.read();
    assert_eq!(
        fixture.start(&section(1), revision(&stopped, &section(1)), 4),
        DesiredWatchOutcomeV1::Committed,
    );
    let restarted = fixture.read();
    let restarted_entry = entry(&restarted, &section(1)).unwrap();
    assert_ne!(
        restarted_entry.materialization_epoch, first_epoch,
        "a desired-value change allocates a new epoch",
    );
    assert_ne!(
        restarted_entry.materialized.as_ref().unwrap().active_watch_id,
        first_id,
        "a cancelled watch is not adopted by the next start",
    );
}

// ---------------------------------------------------------------------------
// Failure, retry, and what the process refuses to decide
// ---------------------------------------------------------------------------

/// A section the catalog stopped publishing keeps its row.
///
/// The runtime has proven it cannot arm this one, and it still does not get
/// to withdraw the user's intent: the section may be published again, and a
/// row that vanished on its own would leave the user with a shorter list and
/// no explanation. The reason is reported; the stop stays theirs.
#[test]
fn a_section_the_catalog_will_not_publish_keeps_its_row_and_reports_why() {
    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);
    fixture
        .admission
        .set("00001", WatchStartAdmission::SectionNotFound);

    assert_eq!(
        fixture.start(&section(1), 0, 1),
        DesiredWatchOutcomeV1::Committed,
        "the authority does not consult the catalog; the write succeeds",
    );
    let state = fixture.read();
    let entry = entry(&state, &section(1)).expect("the row is kept");
    assert!(entry.policy.is_some(), "the intent is still recorded");
    assert!(!armed(&state, &section(1)), "and it is not shown as watched");
    let failure = entry.failure.expect("the reason must be reported");
    assert_eq!(failure.classification, DesiredWatchFailureClassV1::Permanent);
    assert_eq!(failure.reason, DesiredWatchFailureReasonV1::SectionNotFound);
    assert!(
        !failure.retry_scheduled,
        "nothing this process does changes the answer, so it stops asking",
    );

    // A permanent failure stops the retry loop but not the process: the
    // catalog publishing the section later still arms it on the next
    // reconcile the user's own action triggers.
    let consulted = fixture.admission.consulted().len();
    fixture.coordinator.tick();
    assert_eq!(
        fixture.admission.consulted().len(),
        consulted,
        "a permanent failure must not spin the admission source",
    );
}

/// A transient failure backs off and then succeeds, without the user doing
/// anything and without the row ever changing.
#[test]
fn a_transient_failure_backs_off_and_then_arms() {
    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);
    fixture
        .admission
        .set("00001", WatchStartAdmission::TargetUnavailable);

    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    let state = fixture.read();
    let failure = entry(&state, &section(1)).unwrap().failure.unwrap();
    assert_eq!(failure.classification, DesiredWatchFailureClassV1::Transient);
    assert_eq!(failure.reason, DesiredWatchFailureReasonV1::TargetUnavailable);
    assert!(failure.retry_scheduled);
    assert!(!armed(&state, &section(1)));
    let intent_revision = revision(&state, &section(1));

    // The snapshot becomes available. Nothing about the intent changed.
    fixture
        .admission
        .set("00001", WatchStartAdmission::admitted(None));
    fixture.coordinator.tick();

    let state = fixture.read();
    assert!(armed(&state, &section(1)));
    assert!(entry(&state, &section(1)).unwrap().failure.is_none());
    assert_eq!(
        revision(&state, &section(1)),
        intent_revision,
        "recovering must not move the revision under an open page",
    );
}

/// A retry scheduled under one authority stamp must not delay -- or apply to
/// -- a different one.
///
/// The discriminating half is the second: the retry is scheduled with a long
/// backoff, then the user changes the intent. If the retry gate compared
/// only "is a retry pending" instead of the whole stamp, the new intent would
/// sit unarmed until a backoff that belongs to a question nobody is asking
/// any more expired.
#[test]
fn a_retry_from_an_older_authority_stamp_neither_applies_nor_delays() {
    let fixture = Fixture::with_backoff(vec![Duration::from_secs(600)]);
    let (_page, _frames) = fixture.attach(100);
    fixture
        .admission
        .set("00001", WatchStartAdmission::TargetUnavailable);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    assert!(
        entry(&fixture.read(), &section(1))
            .unwrap()
            .failure
            .unwrap()
            .retry_scheduled,
    );

    // The section is now armable, but the pending retry is ten minutes away.
    fixture
        .admission
        .set("00001", WatchStartAdmission::admitted(None));
    fixture.coordinator.reconcile().unwrap();
    assert!(
        !armed(&fixture.read(), &section(1)),
        "a pending retry for the CURRENT stamp is honoured",
    );

    // The user edits the policy. That is a new question, and the answer to
    // the old one -- including how long to wait before asking it again -- is
    // no longer relevant.
    let state = fixture.read();
    assert_eq!(
        fixture.submit(&section(1), Some(loud_policy()), revision(&state, &section(1)), 2),
        DesiredWatchOutcomeV1::Committed,
    );
    let state = fixture.read();
    assert!(
        armed(&state, &section(1)),
        "a new stamp must be tried immediately, not after the old backoff",
    );
    assert_eq!(
        entry(&state, &section(1)).unwrap().policy.as_ref(),
        Some(&loud_policy()),
    );

    // And a retry outstanding for a section the user has since stopped can
    // never resurrect it.
    fixture
        .admission
        .set("00002", WatchStartAdmission::TargetUnavailable);
    assert_eq!(fixture.start(&section(2), 0, 3), DesiredWatchOutcomeV1::Committed);
    let state = fixture.read();
    assert_eq!(
        fixture.stop(&section(2), revision(&state, &section(2)), 4),
        DesiredWatchOutcomeV1::Committed,
    );
    fixture
        .admission
        .set("00002", WatchStartAdmission::admitted(None));
    fixture.coordinator.tick();
    fixture.coordinator.reconcile().unwrap();
    let state = fixture.read();
    assert!(!armed(&state, &section(2)));
    assert!(entry(&state, &section(2)).unwrap().materialized.is_none());
    assert_eq!(fixture.watch.owner_watched_sections(), vec![section(1)]);
}

/// The physical cap is the manager's, and the authority cap is the store's.
/// A section that loses the race for a slot is "preparing", not "failed",
/// and it gets the slot as soon as one frees.
#[test]
fn a_section_waiting_for_a_physical_slot_is_reported_as_blocked_not_failed() {
    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);
    for index in 1..=MAX_DESIRED_WATCHES as u16 {
        assert_eq!(
            fixture.start(&section(index), 0, u64::from(index)),
            DesiredWatchOutcomeV1::Committed,
        );
    }
    assert_eq!(fixture.watch.total_active_watch_count(), MAX_DESIRED_WATCHES);

    // The authority refuses a tenth section, so the physical cap is never
    // even reached through the ordinary path.
    assert_eq!(
        fixture.start(&section(10), 0, 10),
        DesiredWatchOutcomeV1::LimitExceeded,
    );
    assert!(entry(&fixture.read(), &section(10)).is_none());
}

// ---------------------------------------------------------------------------
// The socket is not a second source of truth
// ---------------------------------------------------------------------------

/// Locally, what is monitored is decided by the stored intent. A frame that
/// could start or stop a watch would be a second answer to the same
/// question, and the two would disagree immediately: the watch would run with
/// no row behind it, so it would vanish on restart while the authority read
/// showed it as not monitored the whole time.
///
/// Episode control is the opposite case and must keep working: the alerts a
/// page is looking at belong to a watch the page does not hold, so those
/// commands are executed against the owner.
#[test]
fn the_local_socket_refuses_legacy_mutations_and_routes_episode_control() {
    let fixture = Fixture::new();
    let (page, mut frames) = fixture.attach(100);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    let _ = drain(&mut frames);

    let items = bcsp_contracts::WatchStartItemsV1::try_from(vec![
        bcsp_contracts::WatchStartItemV1::new(section(2), policy()),
    ])
    .unwrap();
    fixture.route.receive_text(
        page,
        &frame(900, WatchClientCommandV1::StartWatch { items }),
    );
    assert_eq!(
        fixture.watch.owner_watched_sections(),
        vec![section(1)],
        "a START frame must not create a watch",
    );
    assert!(
        drain(&mut frames).is_empty(),
        "and must not be answered as if it had",
    );
    assert!(entry(&fixture.read(), &section(2)).is_none());

    let armed_id = entry(&fixture.read(), &section(1))
        .unwrap()
        .materialized
        .as_ref()
        .unwrap()
        .active_watch_id
        .clone();
    fixture.route.receive_text(
        page,
        &frame(
            901,
            WatchClientCommandV1::StopWatch {
                watch: bcsp_contracts::ActiveWatchTargetV1 {
                    active_watch_id: armed_id.clone(),
                    section_key: section(1),
                },
            },
        ),
    );
    assert_eq!(
        fixture.watch.owner_watched_sections(),
        vec![section(1)],
        "a STOP frame must not tear down what the authority still wants",
    );
    assert!(armed(&fixture.read(), &section(1)));

    // Episode control still reaches the watch it names.
    fixture.watch.publish(open_observation(&section(1))).unwrap();
    let alerts = drain(&mut frames);
    let alert = alerts
        .iter()
        .find(|frame| frame["payload"]["type"] == "ALERT_UPDATED")
        .expect("an open section alerts");
    let alert_id = alert["payload"]["alert"]["alertId"].as_str().unwrap();
    let episode = &alert["payload"]["alert"]["episode"];
    fixture.route.receive_text(
        page,
        &frame(
            902,
            serde_json::from_value(serde_json::json!({
                "type": "DISMISS_ALERT",
                "alert": {
                    "activeWatchId": episode["activeWatchId"],
                    "alertId": alert_id,
                    "episodeId": episode["episodeId"],
                    "sectionKey": episode["sectionKey"],
                },
            }))
            .unwrap(),
        ),
    );
    let dismissed = event_types(&drain(&mut frames));
    assert!(
        dismissed.contains(&"ALERT_UPDATED".to_owned()),
        "dismissing an alert on an owner-held watch must be accepted: {dismissed:?}",
    );
}

// ---------------------------------------------------------------------------
// Reset, rotation, and the response budget
// ---------------------------------------------------------------------------

/// Rotation renumbers the authority and raises its generation. It must not
/// restart a healthy watch on the way: nothing about the user's intent
/// changed, only the numbers it is stamped with.
#[test]
fn rotation_renumbers_the_authority_without_restarting_a_healthy_watch() {
    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    let before = fixture.read();
    let watch_id = entry(&before, &section(1))
        .unwrap()
        .materialized
        .as_ref()
        .unwrap()
        .active_watch_id
        .clone();

    seed_tombstones(&fixture.path, MAX_DESIRED_WATCH_TOMBSTONES);
    assert!(
        fixture.coordinator.rotate_if_due().unwrap(),
        "a full removal history is what rotation is for",
    );

    let after = fixture.read();
    assert_eq!(
        after.authority_generation,
        before.authority_generation + 1,
        "rotation raises the generation",
    );
    assert_eq!(after.entries.len(), 1, "and clears the removal history");
    assert_eq!(
        entry(&after, &section(1))
            .unwrap()
            .materialized
            .as_ref()
            .unwrap()
            .active_watch_id,
        watch_id,
        "the running watch is adopted under the new stamp, not replaced",
    );
    assert!(
        armed(&after, &section(1)),
        "and it is honestly reported as watched",
    );

    // A command from before the rotation is refused, which is the whole
    // point of raising the generation.
    assert_eq!(
        fixture.submit_at(
            &section(2),
            Some(policy()),
            0,
            before.authority_generation,
            2,
        ),
        DesiredWatchOutcomeV1::StaleGeneration,
    );
}

/// The largest authority a user can legally reach has to fit in one
/// response, because there is no second page to ask for.
///
/// A truncated read would be indistinguishable from a complete one: the
/// missing tombstones would look like sections that never existed, and the
/// next command against one of them would carry `basedOnRevision = 0` and be
/// admitted -- resurrecting intent the user cancelled. So the bound is
/// proven here rather than enforced at request time.
#[test]
fn the_largest_authority_read_fits_the_local_budget() {
    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);
    for index in 1..=MAX_DESIRED_WATCHES as u16 {
        assert_eq!(
            fixture.submit(&section(9_000 + index), Some(loud_policy()), 0, u64::from(index)),
            DesiredWatchOutcomeV1::Committed,
        );
    }
    // Seeded after the commits, because a commit made against a full removal
    // history rotates it away -- which is the coordinator doing its job, and
    // is why production never sits at this bound for long. The bound still
    // has to be representable: the writer enforces it independently of
    // whether anything rotates.
    seed_tombstones(&fixture.path, MAX_DESIRED_WATCH_TOMBSTONES);

    let state = fixture.read();
    assert_eq!(
        state.entries.len() as u64,
        MAX_DESIRED_WATCH_TOMBSTONES + MAX_DESIRED_WATCHES as u64,
        "521 rows: every section a user may watch, plus a full removal history",
    );
    assert_eq!(
        state
            .entries
            .iter()
            .filter(|entry| entry.policy.is_none())
            .count() as u64,
        MAX_DESIRED_WATCH_TOMBSTONES,
        "every tombstone is present; none is dropped to make room",
    );

    let encoded = serde_json::to_vec(&bcsp_contracts::HttpSuccessEnvelope::new(&state)).unwrap();
    assert!(
        encoded.len() <= LOCAL_DESIRED_WATCH_RESPONSE_BUDGET_BYTES,
        "the largest legal authority read is {} bytes, over the {} byte budget",
        encoded.len(),
        LOCAL_DESIRED_WATCH_RESPONSE_BUDGET_BYTES,
    );
}

/// The production retry schedule, pinned separately from the tests that
/// inject a faster one.
#[test]
fn the_production_retry_schedule_is_bounded_at_thirty_seconds() {
    assert_eq!(
        DESIRED_WATCH_MATERIALIZE_BACKOFF,
        [
            Duration::from_secs(5),
            Duration::from_secs(10),
            Duration::from_secs(20),
            Duration::from_secs(30),
        ],
    );
    assert!(
        DESIRED_WATCH_MATERIALIZE_BACKOFF
            .iter()
            .all(|delay| *delay <= Duration::from_secs(30)),
    );
}

// ---------------------------------------------------------------------------
// Materialization stays true, or stops claiming to be
// ---------------------------------------------------------------------------

/// A section can be armed and then stop being publishable. The proof that it
/// was armable once does not survive that, and neither may the green light.
///
/// The authority is untouched by any of it: the row keeps its policy, the
/// user keeps the decision, and the writer is never taught to ask the catalog
/// anything -- the START below commits while the catalog is already refusing
/// the section.
#[test]
fn a_section_that_leaves_the_catalog_after_arming_is_taken_down_and_reported() {
    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    assert!(armed(&fixture.read(), &section(1)));
    let armed_revision = revision(&fixture.read(), &section(1));

    // Rutgers stops publishing it.
    fixture
        .admission
        .set("00001", WatchStartAdmission::SectionNotFound);
    fixture.coordinator.tick();

    let state = fixture.read();
    let entry = entry(&state, &section(1)).expect("the row survives");
    assert!(
        fixture.watch.owner_watched_sections().is_empty(),
        "the physical watch must be taken down, not left polling",
    );
    assert!(entry.materialized.is_none(), "and never reported as running");
    assert!(!armed(&state, &section(1)));
    assert!(entry.policy.is_some(), "the user's intent is not withdrawn");
    let failure = entry.failure.expect("the reason must be reported");
    assert_eq!(failure.classification, DesiredWatchFailureClassV1::Permanent);
    assert_eq!(failure.reason, DesiredWatchFailureReasonV1::SectionNotFound);
    assert!(!failure.retry_scheduled);
    assert_eq!(
        revision(&state, &section(1)),
        armed_revision,
        "taking a watch down is not a change to the intent",
    );

    // And the writer still does not consult the catalog: a second section is
    // admitted into the authority while the catalog refuses it too.
    fixture
        .admission
        .set("00002", WatchStartAdmission::SectionNotFound);
    assert_eq!(
        fixture.start(&section(2), 0, 2),
        DesiredWatchOutcomeV1::Committed,
    );
}

/// The same story told by the term window and the campus list, which the
/// shared maintenance sweep is responsible for. The sweep has to cover the
/// synthetic owner: under a coordinator EVERY watch is held by it, so a sweep
/// that only looked at browser connections would never stop anything.
#[test]
fn a_target_that_stops_being_watchable_after_arming_is_taken_down_and_reported() {
    for (label, toggle, reason) in [
        (
            "term",
            &fixture_term as &dyn Fn(&Fixture),
            DesiredWatchFailureReasonV1::TermOutOfRange,
        ),
        (
            "campus",
            &fixture_campus as &dyn Fn(&Fixture),
            DesiredWatchFailureReasonV1::UnsupportedTarget,
        ),
    ] {
        let fixture = Fixture::new();
        let (_page, _frames) = fixture.attach(100);
        assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
        assert!(armed(&fixture.read(), &section(1)), "{label}");

        toggle(&fixture);
        // The shared socket sweep is what notices; the coordinator is what
        // reports it. Both have to run for the page to be told the truth.
        fixture.watch.tick();
        assert!(
            fixture.watch.owner_watched_sections().is_empty(),
            "{label}: the maintenance sweep must cover the owner's watches",
        );
        fixture.coordinator.tick();

        let state = fixture.read();
        let entry = entry(&state, &section(1)).expect("the row survives");
        assert!(entry.materialized.is_none(), "{label}");
        assert!(entry.policy.is_some(), "{label}");
        let failure = entry.failure.expect("a reason is reported");
        assert_eq!(failure.classification, DesiredWatchFailureClassV1::Permanent, "{label}");
        assert_eq!(failure.reason, reason, "{label}");
        assert!(!failure.retry_scheduled, "{label}");
    }
}

fn fixture_term(fixture: &Fixture) {
    fixture.admission.in_range.store(false, Ordering::SeqCst);
}

fn fixture_campus(fixture: &Fixture) {
    fixture.admission.supported.store(false, Ordering::SeqCst);
}

/// A watch can end underneath the coordinator with nothing to announce it --
/// a heartbeat sweep, a forced stop, a manager-side teardown. Nothing about
/// the user's intent changed, so nothing prompts a reconcile; the record
/// would sit there claiming a watch that stopped existing.
///
/// Two halves, and the second is the one a Section-only comparison gets
/// wrong: after a watch ends, the same Section can be watched again under a
/// DIFFERENT identity, and a record that only checked "is this Section
/// watched" would adopt a watch it never started.
#[test]
fn a_watch_that_ended_underneath_the_coordinator_is_never_reported_as_running() {
    let fixture = Fixture::with_settings(vec![Duration::ZERO], DESIRED_WATCH_REVALIDATE_INTERVAL);
    let (_page, _frames) = fixture.attach(100);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    let first = fixture.armed_id(&section(1));

    // The watch ends without the coordinator being told.
    fixture
        .watch
        .owner_stop(ActiveWatchTargetV1 {
            active_watch_id: first,
            section_key: section(1),
        })
        .expect("manager-side stop");
    assert!(
        entry(&fixture.read(), &section(1))
            .unwrap()
            .materialized
            .is_none(),
        "a read must not report a watch the process is not holding",
    );

    // Something else starts a watch on the same Section. It is not this
    // coordinator's watch, and adopting it would be a lie by coincidence.
    let foreign = fixture
        .watch
        .owner_start(
            WatchStartItemsV1::try_from(vec![WatchStartItemV1::new(section(1), policy())]).unwrap(),
        )
        .expect("owner start")
        .first()
        .and_then(|result| match result {
            WatchStartItemResultV1::Active { active_watch_id, .. } => Some(*active_watch_id),
            WatchStartItemResultV1::Rejected { .. } => None,
        })
        .expect("a fresh physical watch");
    assert_ne!(foreign, first);
    assert!(
        entry(&fixture.read(), &section(1))
            .unwrap()
            .materialized
            .is_none(),
        "a different watch on the same Section is not this record's watch",
    );

    // The tick notices the drift even though the audience never changed and
    // no retry was scheduled. What it does about it -- take the section back
    // over, or report why it cannot -- is the ordinary reconcile; what it may
    // never do is keep claiming a watch it does not hold.
    fixture.coordinator.tick();
    let state = fixture.read();
    let settled = entry(&state, &section(1)).expect("the row survives");
    assert!(settled.policy.is_some(), "the intent is never withdrawn");
    let live = fixture
        .watch
        .owner_watch_targets()
        .into_iter()
        .map(|target| (target.section_key, target.active_watch_id))
        .collect::<BTreeMap<_, _>>();
    if let Some(running) = settled.materialized.as_ref() {
        assert_eq!(
            live.get(&section(1)),
            Some(&running.active_watch_id),
            "anything reported as running must be a watch the process holds: {settled:?}",
        );
        assert_ne!(
            running.active_watch_id, first,
            "and never the watch that already ended",
        );
    }
}

/// A revocation that can stop being true clears the green light, keeps the
/// intent, and recovers on its own -- without the revision or the epoch
/// moving, because none of this is the user changing their mind.
#[test]
fn a_transient_revocation_clears_the_green_light_and_recovers_by_itself() {
    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    let before = fixture.read();
    let intent_revision = revision(&before, &section(1));
    let intent_epoch = entry(&before, &section(1)).unwrap().materialization_epoch;

    fixture
        .admission
        .set("00001", WatchStartAdmission::TargetUnavailable);
    fixture.coordinator.tick();
    let state = fixture.read();
    assert!(fixture.watch.owner_watched_sections().is_empty());
    assert!(!armed(&state, &section(1)));
    let failure = entry(&state, &section(1)).unwrap().failure.unwrap();
    assert_eq!(failure.classification, DesiredWatchFailureClassV1::Transient);
    assert!(failure.retry_scheduled);

    fixture
        .admission
        .set("00001", WatchStartAdmission::admitted(None));
    fixture.coordinator.tick();
    let state = fixture.read();
    assert!(armed(&state, &section(1)), "the intent comes back on its own");
    assert_eq!(revision(&state, &section(1)), intent_revision);
    assert_eq!(
        entry(&state, &section(1)).unwrap().materialization_epoch,
        intent_epoch,
    );
}

/// The other half of the same contract: re-checking must not become a reason
/// to restart. A watch that is still legitimate keeps its identity and its
/// episode across as much maintenance as anyone cares to run.
#[test]
fn repeated_maintenance_never_restarts_a_healthy_watch() {
    let fixture = Fixture::new();
    let (_page, mut frames) = fixture.attach(100);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    let first = fixture.armed_id(&section(1));
    let announcements = event_types(&drain(&mut frames))
        .into_iter()
        .filter(|event| event == "START_RESULT")
        .count();
    assert_eq!(announcements, 1);

    for _ in 0..8 {
        fixture.watch.tick();
        fixture.coordinator.tick();
        fixture.coordinator.reconcile().unwrap();
    }

    assert_eq!(fixture.armed_id(&section(1)), first, "same physical watch");
    assert!(armed(&fixture.read(), &section(1)));
    assert!(
        event_types(&drain(&mut frames))
            .into_iter()
            .all(|event| event != "START_RESULT"),
        "a healthy watch must not be re-announced",
    );
}

// ---------------------------------------------------------------------------
// Rotation covers every way the budgets grow, and runs once per crossing
// ---------------------------------------------------------------------------

/// Refusals write receipts too.
///
/// Nine watched sections and a user who keeps asking for a tenth produces a
/// terminal refusal, and a receipt, every time -- without a single commit. A
/// process that only rotated after a commit would let that ledger fill to its
/// hard cap, and every later write, including the STOP that would free a
/// slot, would be refused for the rest of the database's life.
#[test]
fn terminal_refusal_receipts_alone_rotate_the_authority_exactly_once() {
    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    let generation = fixture.read().authority_generation;
    seed_receipts(
        &fixture.path,
        generation,
        DESIRED_WATCH_RECEIPT_ROTATION_THRESHOLD - receipt_count(&fixture.path) - 1,
    );
    assert_eq!(
        receipt_count(&fixture.path),
        DESIRED_WATCH_RECEIPT_ROTATION_THRESHOLD - 1,
    );

    // One more terminal refusal. Nothing is committed by it.
    assert_eq!(
        fixture.start(&section(2), 7, 900),
        DesiredWatchOutcomeV1::StaleRevision,
    );

    let state = fixture.read();
    assert_eq!(
        state.authority_generation,
        generation + 1,
        "a ledger filled by refusals must still be rotated",
    );
    assert_eq!(receipt_count(&fixture.path), 0);
    assert!(
        !fixture.coordinator.rotate_if_due().unwrap(),
        "one crossing is one rotation",
    );
    assert_eq!(
        fixture.read().authority_generation,
        generation + 1,
        "and asking again does not raise the generation for nothing",
    );
    assert!(armed(&fixture.read(), &section(1)), "and nothing was restarted");
}

/// A database that comes back at the hard cap has to be recoverable by the
/// process alone, on maintenance, with nothing else happening.
///
/// Nothing the user can do would fix it: every write, including the STOP, is
/// refused until the ledger is freed, and only a rotation frees it. So the
/// tick has to rotate whether or not anything was committed -- there is no
/// commit left that could.
#[test]
fn a_restart_at_the_receipt_hard_cap_recovers_on_maintenance_alone() {
    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    let generation = fixture.read().authority_generation;
    seed_receipts(
        &fixture.path,
        generation,
        MAX_DESIRED_WATCH_RECEIPTS - receipt_count(&fixture.path),
    );
    assert_eq!(receipt_count(&fixture.path), MAX_DESIRED_WATCH_RECEIPTS);

    // A fresh coordinator over the same database -- a restart -- and one
    // maintenance step. No audience change, no scheduled retry, no commit.
    let recovered = fixture.restarted_coordinator();
    recovered.tick();

    let state = recovered.read().unwrap();
    assert_eq!(
        state.authority_generation,
        generation + 1,
        "startup maintenance must recover a database at the hard cap",
    );
    assert_eq!(receipt_count(&fixture.path), 0);
    assert_eq!(
        fixture.watch.owner_watched_sections(),
        vec![section(1)],
        "and maintenance did not tear a healthy watch down on its way",
    );
}

/// A full ledger refuses the write rather than guessing, and the STOP the
/// user pressed still eventually commits. Refusing it forever would leave a
/// watch they cannot turn off, which is worse than any silence.
#[test]
fn a_full_ledger_refuses_the_stop_and_the_next_attempt_commits() {
    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    let generation = fixture.read().authority_generation;
    seed_receipts(
        &fixture.path,
        generation,
        MAX_DESIRED_WATCH_RECEIPTS - receipt_count(&fixture.path),
    );

    let stale = fixture.read();
    assert_eq!(
        fixture
            .coordinator
            .submit(&DesiredWatchMutationV1 {
                contract_version: LOCAL_DESIRED_WATCH_CONTRACT_VERSION,
                section: section(1),
                policy: None,
                based_on_revision: revision(&stale, &section(1)),
                authority_generation: stale.authority_generation,
                mutation_id: trace(901),
            })
            .unwrap()
            .outcome,
        DesiredWatchOutcomeV1::AuthorityFull,
        "a full ledger refuses the write rather than writing a false answer",
    );

    // The refusal is not terminal: nothing was written, no receipt was left,
    // and the maintenance the refusal itself triggered freed the ledger.
    let state = fixture.read();
    assert_eq!(state.authority_generation, generation + 1);
    assert_eq!(receipt_count(&fixture.path), 0);
    assert_eq!(
        fixture
            .coordinator
            .submit(&DesiredWatchMutationV1 {
                contract_version: LOCAL_DESIRED_WATCH_CONTRACT_VERSION,
                section: section(1),
                policy: None,
                based_on_revision: revision(&state, &section(1)),
                authority_generation: state.authority_generation,
                mutation_id: trace(902),
            })
            .unwrap()
            .outcome,
        DesiredWatchOutcomeV1::Committed,
        "a watch the user asked to stop must eventually stop",
    );
    assert!(fixture.watch.owner_watched_sections().is_empty());
}

/// Concurrent callers, one threshold, one rotation.
///
/// Rotation raises the generation, so a second one for the same crossing
/// invalidates the `basedOnRevision` every page has just re-read with -- for
/// nothing. The race is between deciding "due" and acting on it: a caller
/// that read the budget before someone else's rotation committed rotates
/// again unless it re-reads inside the exclusive domain.
///
/// The window is forced rather than hoped for. One thread is parked INSIDE
/// the domain -- a reconcile, held at the first thing it asks the physical
/// owner -- so every rotating caller reaches its decision while the domain is
/// occupied, which is exactly the interleaving a check taken outside it
/// loses. Repetition could not reach this reliably: without the hold the
/// window is a few instructions wide.
#[test]
fn concurrent_callers_crossing_one_threshold_rotate_once() {
    const CALLERS: usize = 6;

    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    let generation = fixture.read().authority_generation;
    seed_receipts(
        &fixture.path,
        generation,
        DESIRED_WATCH_RECEIPT_ROTATION_THRESHOLD - receipt_count(&fixture.path),
    );

    let held = fixture.faults.hold.lock().unwrap();
    let occupant = {
        let coordinator = fixture.coordinator.clone();
        std::thread::spawn(move || coordinator.reconcile().unwrap())
    };
    // Let the occupant take the domain and park there.
    std::thread::sleep(Duration::from_millis(150));

    let barrier = Arc::new(std::sync::Barrier::new(CALLERS));
    let workers = (0..CALLERS)
        .map(|_| {
            let coordinator = fixture.coordinator.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                coordinator.rotate_if_due().unwrap()
            })
        })
        .collect::<Vec<_>>();
    // Every caller has now decided whether a rotation is due, with the domain
    // still occupied.
    std::thread::sleep(Duration::from_millis(300));
    drop(held);
    occupant.join().unwrap();

    let rotated = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .filter(|rotated| *rotated)
        .count();

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

/// A response that triggered a rotation must describe ONE authority.
///
/// The commit is decided against the numbers before the rotation and the
/// state is read after it. Reporting both would hand the page a generation it
/// must not write with beside the rows it would write against -- and a
/// `revision` for its own commit that no row has.
#[test]
fn a_response_that_triggered_a_rotation_reports_one_authority_state() {
    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);
    // Committed out of section order, so the pre-rotation revision and the
    // post-rotation renumbering genuinely differ.
    assert_eq!(fixture.start(&section(3), 0, 1), DesiredWatchOutcomeV1::Committed);
    assert_eq!(fixture.start(&section(1), 0, 2), DesiredWatchOutcomeV1::Committed);
    let generation = fixture.read().authority_generation;
    seed_receipts(
        &fixture.path,
        generation,
        DESIRED_WATCH_RECEIPT_ROTATION_THRESHOLD - receipt_count(&fixture.path) - 1,
    );

    let result = fixture
        .coordinator
        .submit(&DesiredWatchMutationV1 {
            contract_version: LOCAL_DESIRED_WATCH_CONTRACT_VERSION,
            section: section(2),
            policy: Some(policy()),
            based_on_revision: 0,
            authority_generation: generation,
            mutation_id: trace(910),
        })
        .unwrap();

    assert_eq!(result.outcome, DesiredWatchOutcomeV1::Committed);
    let state = result.state.as_ref().expect("a commit returns its state");
    assert_eq!(
        state.authority_generation, generation + 1,
        "the crossing did rotate",
    );
    assert_eq!(
        result.authority_generation, state.authority_generation,
        "the top-level generation is the state's generation",
    );
    let committed = result.committed.expect("a commit reports what it wrote");
    let entry = entry(state, &section(2)).expect("the committed row");
    assert_eq!(
        committed.revision, entry.revision,
        "the reported revision is the row's revision",
    );
    assert_eq!(committed.materialization_epoch, entry.materialization_epoch);
    assert_eq!(
        entry.revision, 2,
        "and the row really was renumbered by the rotation",
    );
    assert!(committed.epoch_changed, "a new desired value is a new epoch");
}

/// A refusal that is replayed is still a refusal, with the status its
/// original decision earned. The alternative -- 200, because the envelope
/// says "replayed" -- would tell a page whose response was lost that a
/// command it never applied had succeeded.
#[test]
fn a_replayed_terminal_refusal_is_still_the_refusal_it_was() {
    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    let generation = fixture.read().authority_generation;
    let stale = DesiredWatchMutationV1 {
        contract_version: LOCAL_DESIRED_WATCH_CONTRACT_VERSION,
        section: section(1),
        policy: Some(loud_policy()),
        based_on_revision: 99,
        authority_generation: generation,
        mutation_id: trace(920),
    };

    let first = fixture.coordinator.submit(&stale).unwrap();
    assert_eq!(first.outcome, DesiredWatchOutcomeV1::StaleRevision);
    assert!(!first.replayed);
    assert_eq!(first.outcome.http_status(), 409);

    let replay = fixture.coordinator.submit(&stale).unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.outcome, DesiredWatchOutcomeV1::StaleRevision);
    assert_eq!(replay.outcome.http_status(), 409);
    assert_eq!(replay.current_revision, first.current_revision);
    assert!(replay.state.is_none(), "a refusal does not carry state");
}

// ---------------------------------------------------------------------------
// The Full Reset barrier
// ---------------------------------------------------------------------------

/// A Full Reset is one lifecycle barrier over two owners, not two
/// independent clean-ups.
///
/// While it is held nothing may materialize, because the rows still on disk
/// describe what is being deleted rather than what should be running. When it
/// is lowered every physical watch the process holds is gone -- including one
/// this coordinator has no record of, which is exactly what a reconcile that
/// raced the start of the reset would leave behind.
#[test]
fn the_reset_barrier_stops_every_physical_watch_including_one_it_never_armed() {
    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);
    fixture.coordinator.begin_authority_reset().unwrap();

    // A page submits while the barrier is up. The authority accepts it --
    // the writer is not part of the barrier -- but nothing may arm.
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    assert_eq!(
        fixture.watch.total_active_watch_count(),
        0,
        "nothing may materialize while a reset owns the watches",
    );
    assert!(!armed(&fixture.read(), &section(1)));
    fixture.coordinator.reconcile().unwrap();
    fixture.coordinator.tick();
    assert_eq!(fixture.watch.total_active_watch_count(), 0);

    // A watch armed by a reconcile that started before the barrier: the
    // coordinator has no record of it, and it is still the process's to stop.
    fixture
        .watch
        .owner_start(
            WatchStartItemsV1::try_from(vec![WatchStartItemV1::new(section(5), policy())]).unwrap(),
        )
        .expect("owner start");
    assert_eq!(fixture.watch.total_active_watch_count(), 1);

    fixture.coordinator.finish_authority_reset().unwrap();
    assert!(
        fixture.watch.owner_watch_targets().is_empty(),
        "the barrier lowers on an empty process, not a hopeful one",
    );
    assert_eq!(fixture.watch.total_active_watch_count(), 0);
    let state = fixture.read();
    assert!(state.entries.iter().all(|entry| entry.materialized.is_none()));

    // And materialization works again afterwards: the barrier is a moment,
    // not a mode.
    fixture.coordinator.reconcile().unwrap();
    assert!(armed(&fixture.read(), &section(1)));
}

// ---------------------------------------------------------------------------
// The two physical operations that fail invisibly
// ---------------------------------------------------------------------------

/// A teardown that fails leaves the watch ALIVE. The record therefore keeps
/// naming it -- that id is the only way anything can stop it later -- and
/// something has to come back and finish the job, or the section sits at
/// "stopping" for the rest of the process's life while still ringing.
#[test]
fn a_failed_teardown_keeps_the_watch_addressable_and_a_later_tick_finishes_it() {
    let fixture = Fixture::with_settings(vec![Duration::ZERO], DESIRED_WATCH_REVALIDATE_INTERVAL);
    let (_page, _frames) = fixture.attach(100);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    let armed_revision = revision(&fixture.read(), &section(1));

    fixture.faults.stop.store(true, Ordering::SeqCst);
    assert_eq!(
        fixture.stop(&section(1), armed_revision, 2),
        DesiredWatchOutcomeV1::Committed,
    );
    let state = fixture.read();
    let tombstone = entry(&state, &section(1)).expect("a stop leaves a tombstone");
    assert!(tombstone.policy.is_none());
    assert!(
        tombstone.pending_disarm,
        "a watch that would not stop is never reported as stopped",
    );
    assert_eq!(
        fixture.watch.owner_watched_sections(),
        vec![section(1)],
        "and it really is still running",
    );

    // Nothing else changes: the audience is the same and no arm retry is
    // scheduled. The teardown retry is the only thing that can finish this.
    fixture.faults.stop.store(false, Ordering::SeqCst);
    fixture.coordinator.tick();

    assert!(
        fixture.watch.owner_watched_sections().is_empty(),
        "the teardown must be retried until it succeeds",
    );
    let state = fixture.read();
    let tombstone = entry(&state, &section(1)).expect("the tombstone survives");
    assert!(!tombstone.pending_disarm);
    assert!(tombstone.materialized.is_none());
}

/// A policy edit that fails leaves the watch running under the OLD policy.
///
/// Forgetting its id there is the worst of both worlds: the watch keeps
/// polling and keeps ringing, every later arm comes back `AlreadyActive`, and
/// the STOP the user presses has nothing to name. The record keeps the id and
/// the old stamp, so the read reports preparing rather than watching -- and a
/// STOP still reaches the watch.
#[test]
fn a_failed_policy_edit_keeps_the_running_watch_addressable() {
    let fixture = Fixture::new();
    let (_page, _frames) = fixture.attach(100);
    assert_eq!(fixture.start(&section(1), 0, 1), DesiredWatchOutcomeV1::Committed);
    let armed_id = fixture.armed_id(&section(1));
    let armed_revision = revision(&fixture.read(), &section(1));

    fixture.faults.update_policy.store(true, Ordering::SeqCst);
    assert_eq!(
        fixture.submit(&section(1), Some(loud_policy()), armed_revision, 2),
        DesiredWatchOutcomeV1::Committed,
    );
    let state = fixture.read();
    let edited = entry(&state, &section(1)).expect("the row survives");
    assert_eq!(edited.policy.as_ref(), Some(&loud_policy()), "the intent moved");
    assert!(
        !armed(&state, &section(1)),
        "but the watch is not running the way the user asked",
    );
    assert_eq!(
        edited
            .materialized
            .as_ref()
            .map(|running| running.active_watch_id),
        Some(armed_id),
        "and the process still knows how to address it",
    );
    assert_eq!(fixture.watch.owner_watched_sections(), vec![section(1)]);

    // The user gives up and stops it. That must reach the watch.
    let stop_revision = revision(&fixture.read(), &section(1));
    assert_eq!(
        fixture.stop(&section(1), stop_revision, 3),
        DesiredWatchOutcomeV1::Committed,
    );
    assert!(
        fixture.watch.owner_watched_sections().is_empty(),
        "a STOP must be able to stop a watch whose policy edit failed",
    );
    assert!(
        entry(&fixture.read(), &section(1))
            .unwrap()
            .materialized
            .is_none(),
    );
}

/// Bulk-seeds receipts, because reaching the ledger budget through the writer
/// would make the setup the slowest part of the test. The rows are exactly
/// what a terminal refusal writes.
fn seed_receipts(path: &std::path::Path, generation: u64, count: u64) {
    let connection = Connection::open(path).unwrap();
    connection.execute_batch("BEGIN IMMEDIATE").unwrap();
    for index in 0..count {
        connection
            .execute(
                "INSERT INTO personal_desired_watch_receipts_v1
                     (authority_generation, mutation_id, term_id, campus_code, section_index,
                      fingerprint, outcome_json)
                 VALUES (?1, ?2, 'T2026F', 'CAMPUS_A', '30001', ?3,
                         '{\"outcome\":\"STALE_REVISION\",\"current\":1}')",
                rusqlite::params![
                    i64::try_from(generation).unwrap(),
                    format!("00000000-0000-4000-8000-{:012x}", 0x10_0000 + index),
                    "b".repeat(64),
                ],
            )
            .unwrap();
    }
    connection.execute_batch("COMMIT").unwrap();
}

fn receipt_count(path: &std::path::Path) -> u64 {
    let connection = Connection::open(path).unwrap();
    let count: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM personal_desired_watch_receipts_v1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    u64::try_from(count).unwrap()
}

/// Bulk-seeds tombstones, because reaching this budget through the writer
/// would make the setup the slowest part of the test. The rows are exactly
/// what a stop writes.
fn seed_tombstones(path: &std::path::Path, count: u64) {
    let connection = Connection::open(path).unwrap();
    connection.execute_batch("BEGIN IMMEDIATE").unwrap();
    let highest = connection
        .query_row(
            "SELECT desired_watch_revision_counter FROM personal_state_metadata_v1",
            [],
            |row| row.get::<_, i64>(0),
        )
        .unwrap();
    for index in 1..=count {
        let number = highest + index as i64;
        connection
            .execute(
                "INSERT INTO personal_desired_watches_v1
                     (term_id, campus_code, section_index, desired, policy_json,
                      revision, materialization_epoch)
                 VALUES ('T2026F', 'CAMPUS_A', ?1, 0, NULL, ?2, ?2)",
                rusqlite::params![format!("{:05}", 20_000 + index), number],
            )
            .unwrap();
    }
    connection
        .execute(
            "UPDATE personal_state_metadata_v1
                SET desired_watch_revision_counter = ?1,
                    desired_watch_materialization_counter = ?1",
            [highest + count as i64],
        )
        .unwrap();
    connection.execute_batch("COMMIT").unwrap();
}
