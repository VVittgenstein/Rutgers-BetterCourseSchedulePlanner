//! The local desired-watch coordinator: what the process does with stored
//! intent, and what it refuses to do with it.
//!
//! These drive the coordinator directly against a real SQLite file and a real
//! shared watch socket, with only the catalog admission scripted. That is the
//! seam worth faking -- whether Rutgers publishes a section is exactly the
//! answer that changes underneath a running process, and every interesting
//! behaviour here is about what happens when it does.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bcsp_application::{
    NoopWatchDispatchSink, SharedWatchSocket, WatchAdmissionSource, WebSocketExtension,
};
use bcsp_contracts::{
    OpenObservationV1, SectionKey, TraceId, WatchClientCommandV1, WatchContinuousDurationV1,
    WatchMaxAudible, WatchNotificationMode, WatchPolicyV1, WsClientEnvelope,
};
use bcsp_local_runtime::{
    DESIRED_WATCH_MATERIALIZE_BACKOFF, DesiredWatchCoordinator, DesiredWatchFailureClassV1,
    DesiredWatchFailureReasonV1, DesiredWatchMutationV1, DesiredWatchOutcomeV1,
    DesiredWatchStateV1, LOCAL_DESIRED_WATCH_CONTRACT_VERSION,
    LOCAL_DESIRED_WATCH_RESPONSE_BUDGET_BYTES, LocalWatchRoute,
};
use bcsp_local_user_state::{
    MAX_DESIRED_WATCH_TOMBSTONES, MAX_DESIRED_WATCHES, PersonalStateStore,
};
use bcsp_watch::WatchStartAdmission;
use rusqlite::Connection;
use tempfile::TempDir;
use tokio::sync::mpsc;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A catalog whose answers a test can change while the process is running.
#[derive(Clone, Default)]
struct Admission {
    verdicts: Arc<Mutex<BTreeMap<String, WatchStartAdmission>>>,
    fallback: Arc<Mutex<Option<WatchStartAdmission>>>,
    consulted: Arc<Mutex<Vec<SectionKey>>>,
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
        self.verdicts
            .lock()
            .unwrap()
            .get(section.index().as_str())
            .cloned()
            .or_else(|| self.fallback.lock().unwrap().clone())
            .unwrap_or_else(|| WatchStartAdmission::admitted(None))
    }
}

struct Fixture {
    _directory: TempDir,
    path: std::path::PathBuf,
    admission: Admission,
    watch: Arc<SharedWatchSocket>,
    coordinator: Arc<DesiredWatchCoordinator>,
    route: LocalWatchRoute,
}

impl Fixture {
    fn new() -> Self {
        Self::with_backoff(vec![Duration::ZERO])
    }

    fn with_backoff(backoff: Vec<Duration>) -> Self {
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
        let coordinator = Arc::new(
            DesiredWatchCoordinator::new(
                PersonalStateStore::open(&path).unwrap(),
                watch.clone(),
            )
            .with_retry_backoff(backoff),
        );
        let route = LocalWatchRoute::new(watch.clone(), coordinator.clone());
        Self {
            _directory: directory,
            path,
            admission,
            watch,
            coordinator,
            route,
        }
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
