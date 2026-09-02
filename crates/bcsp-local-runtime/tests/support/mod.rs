//! The deterministic PUBLISHED + Gate-pass fixture, shared by the ordinary
//! local-runtime tests and the packaging seeder.
//!
//! Materializing a desired watch needs four things to be true at once: the
//! campus is a product target, the term is inside the Rutgers window, the
//! catalog publishes the section, and the integrity gate has released the
//! target. A test that arranges only some of them proves that stored intent
//! is RESTORED, which is a different and much weaker claim than "restored and
//! actually being watched" -- and the second one is the milestone.
//!
//! One implementation, used from both places, so the release rehearsal and
//! the ordinary suite cannot drift into proving different things. Nothing
//! here reaches Rutgers: every row is synthesized locally, and none of it is
//! part of the shipped package.

#![allow(dead_code)]

// ---------------------------------------------------------------------------
// The physical-watch seam
// ---------------------------------------------------------------------------

/// Makes the two physical-watch operations that can fail invisibly actually
/// fail, and records every teardown that was attempted.
///
/// A teardown that will not tear down and a policy edit that will not apply
/// both leave a watch ALIVE while the coordinator decides what to remember
/// about it, and neither can be provoked through a healthy socket. Without a
/// seam the only available "test" would be reading the code -- and these are
/// precisely the paths that can leave a watch still polling Rutgers with
/// nothing left able to name it.
#[derive(Default)]
pub struct OwnerFaults {
    stop: AtomicBool,
    update_policy: AtomicBool,
    stops: Mutex<Vec<ActiveWatchTargetV1>>,
    retained: Mutex<Vec<ActiveWatchTargetV1>>,
}

impl OwnerFaults {
    pub fn fail_stops(&self, failing: bool) {
        self.stop.store(failing, Ordering::SeqCst);
    }

    pub fn fail_policy_edits(&self, failing: bool) {
        self.update_policy.store(failing, Ordering::SeqCst);
    }

    /// Adds a physical watch the socket's own bulk `stop()` does not reach.
    ///
    /// The Full Reset's teardown of last resort exists for exactly this: a
    /// watch the process is holding that the coordinator has no record of and
    /// that clearing the connections did not take with it. A healthy socket
    /// never produces one, so putting one there is the only way to ask what
    /// the reset does when its final teardown does not finish.
    pub fn retain(&self, target: ActiveWatchTargetV1) {
        self.retained.lock().unwrap().push(target);
    }

    pub fn retained(&self) -> Vec<ActiveWatchTargetV1> {
        self.retained.lock().unwrap().clone()
    }

    /// Every teardown the coordinator attempted, in order.
    ///
    /// A retry is only safe because it names the id captured when the watch
    /// was armed. Nothing about the resulting STATE tells that apart from a
    /// teardown that re-resolved the section and stopped whatever is running
    /// now, so the attempts themselves are what a test has to look at.
    pub fn stop_attempts(&self) -> Vec<ActiveWatchTargetV1> {
        self.stops.lock().unwrap().clone()
    }

    pub fn stopped_ids(&self) -> Vec<ActiveWatchId> {
        self.stop_attempts()
            .into_iter()
            .map(|target| target.active_watch_id)
            .collect()
    }
}

/// A [`DesiredWatchOwner`] over a real shared socket, with those two
/// operations made reachable.
pub struct FaultOwner {
    inner: Arc<SharedWatchSocket>,
    faults: Arc<OwnerFaults>,
}

impl FaultOwner {
    pub fn new(inner: Arc<SharedWatchSocket>, faults: Arc<OwnerFaults>) -> Self {
        Self { inner, faults }
    }
}

impl DesiredWatchOwner for FaultOwner {
    fn audience_connection_count(&self) -> usize {
        self.inner.audience_connection_count()
    }

    fn watch_targets(&self) -> Vec<ActiveWatchTargetV1> {
        let mut targets = self.inner.owner_watch_targets();
        for retained in self.faults.retained.lock().unwrap().iter() {
            if !targets
                .iter()
                .any(|target| target.active_watch_id == retained.active_watch_id)
            {
                targets.push(retained.clone());
            }
        }
        targets
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
        self.faults.stops.lock().unwrap().push(target.clone());
        if self.faults.stop.load(Ordering::SeqCst) {
            // Deliberately without touching the manager: the watch is still
            // running, which is exactly what makes a forgotten id dangerous.
            return Err(WatchManagerError::TargetMismatch);
        }
        let mut retained = self.faults.retained.lock().unwrap();
        if let Some(index) = retained
            .iter()
            .position(|candidate| candidate.active_watch_id == target.active_watch_id)
        {
            retained.remove(index);
            return Ok(());
        }
        drop(retained);
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

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use bcsp_application::SharedWatchSocket;
use bcsp_catalog::{normalize_target, to_catalog_refresh_command, to_discovery_refresh_command};
use bcsp_contracts::{
    ActiveWatchId, ActiveWatchTargetV1, CampusCode, SectionKey, TermCampusKey, TermId, TraceId,
    WatchPolicyV1, WatchStartItemResultV1, WatchStartItemsV1,
};
use bcsp_local_runtime::{DesiredWatchOwner, PreparedLocalRuntime};
use bcsp_operational_storage::{
    BeginOpenPullAttemptCommand, EmptySnapshotDecision, FinishOpenPullSuccessCommand,
    OpenCacheStatus, OpenHttpAuditMetadata, OpenRequestLane, PublishOutcome,
};
use bcsp_rutgers_client::{
    DiscoverySnapshot as RutgersDiscoverySnapshot, DiscoverySourceInput, SourceProvenance,
    decode_catalog_payload, decode_discovery_payload,
};
use bcsp_watch::{WatchManagerError, WatchStartAdmission};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

/// The one section every fixture publishes, on the one product campus.
pub const FIXTURE_CAMPUS: &str = "NB";
pub const FIXTURE_SECTION_INDEX: &str = "10001";

fn trace(value: u64) -> TraceId {
    format!("00000000-0000-4000-8000-{value:012x}")
        .parse()
        .expect("deterministic UUIDv4")
}

/// Publishes a discovery scope, a catalog snapshot and one successful Open
/// pull for each term, leaving `FIXTURE_CAMPUS`/`FIXTURE_SECTION_INDEX`
/// admissible in every one of them.
pub fn seed_ready_query_scope(prepared: &PreparedLocalRuntime, terms: &[&str]) {
    let completed = (OffsetDateTime::now_utc() - time::Duration::seconds(1))
        .format(&Rfc3339)
        .expect("fixture completion timestamp");
    let started = (OffsetDateTime::now_utc() - time::Duration::seconds(2))
        .format(&Rfc3339)
        .expect("fixture start timestamp");
    let terms = terms
        .iter()
        .map(|term| TermId::try_from(*term).unwrap())
        .collect::<Vec<_>>();
    let discovery_body = serde_json::to_vec(&serde_json::json!({
        "sourceVersion": "local-runtime-ready-fixture-v1",
        "terms": terms.iter().map(|term| {
            let raw = term.as_str();
            serde_json::json!({
                "termId": raw,
                "year": raw[1..].parse::<u16>().expect("fixture term year"),
                "termCode": &raw[..1],
                "display": raw,
                "published": true
            })
        }).collect::<Vec<_>>(),
        "campuses": [{
            "campusCode": FIXTURE_CAMPUS,
            "display": "New Brunswick",
            "enabled": true
        }],
        "targets": terms.iter().map(|term| serde_json::json!({
            "termId": term.as_str(),
            "campusCode": FIXTURE_CAMPUS,
            "enabled": true
        })).collect::<Vec<_>>(),
        "subjects": []
    }))
    .expect("synthetic discovery JSON");
    let snapshot = RutgersDiscoverySnapshot::try_from_bundle(vec![DiscoverySourceInput::selector(
        decode_discovery_payload(&discovery_body).expect("decode synthetic discovery"),
        SourceProvenance::from_body("LOCAL_RUNTIME_READY_DISCOVERY", &started, &discovery_body),
    )])
    .expect("normalize synthetic discovery");
    let database = prepared.operational().database();
    let mut database = database.lock().unwrap();
    database
        .operational_mut()
        .apply_discovery_refresh(
            to_discovery_refresh_command(&snapshot, trace(0x800), &started, &completed)
                .expect("build synthetic discovery command"),
        )
        .expect("publish ready-scope discovery fixture");
    for (position, term) in terms.iter().enumerate() {
        let target = TermCampusKey::new(
            term.clone(),
            CampusCode::try_from(FIXTURE_CAMPUS).expect("product Campus"),
        );
        let body = serde_json::to_vec(&serde_json::json!([{
            "campusCode": FIXTURE_CAMPUS,
            "courseString": "01:198:111",
            "subject": "198",
            "subjectDescription": "Computer Science",
            "courseNumber": "111",
            "title": "Synthetic Course",
            "sections": [{
                "campusCode": FIXTURE_CAMPUS,
                "index": FIXTURE_SECTION_INDEX,
                "number": "01",
                "sectionCourseType": "LECTURE",
                "openStatus": false,
                "meetingTimes": [],
                "instructors": []
            }]
        }]))
        .expect("synthetic Catalog JSON");
        let normalized = normalize_target(
            target.clone(),
            decode_catalog_payload(&body).expect("decode synthetic Catalog"),
            SourceProvenance::from_body("LOCAL_RUNTIME_READY_FIXTURE", &started, &body),
        )
        .expect("normalize synthetic Catalog");
        let observation = trace(0x820 + u64::try_from(position).unwrap());
        let outcome = database
            .operational_mut()
            .apply_catalog_refresh(
                to_catalog_refresh_command(&normalized, observation, &started, &completed)
                    .expect("build synthetic Catalog command"),
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
                bcsp_catalog::CATALOG_DERIVATION_VERSION,
            )
            .expect("publish synthetic Catalog");
        let content_version = match outcome {
            PublishOutcome::AppliedChanged {
                content_version, ..
            }
            | PublishOutcome::AppliedUnchanged {
                content_version, ..
            } => content_version,
            other => panic!("unexpected fixture Catalog outcome: {other:?}"),
        };
        assert_eq!(content_version, 1, "fixture Catalog content version");
        let attempt_id = trace(0x840 + u64::try_from(position).unwrap());
        let run_id = trace(0x850 + u64::try_from(position).unwrap());
        let section = SectionKey::try_new(term.as_str(), FIXTURE_CAMPUS, FIXTURE_SECTION_INDEX)
            .expect("synthetic section key");
        database
            .operational_mut()
            .begin_open_pull_attempt(&BeginOpenPullAttemptCommand {
                attempt_id,
                run_id,
                target,
                captured_catalog_content_version: content_version,
                rutgers_day: "2026-07-17".to_owned(),
                started_at: started.clone(),
                lane: OpenRequestLane::ActiveWatch,
                requested_interval_seconds: Some(30),
                effective_interval_seconds: Some(10),
                schedule_lag_ms: Some(0),
            })
            .expect("begin synthetic Open attempt");
        database
            .operational_mut()
            .finish_open_pull_success(FinishOpenPullSuccessCommand {
                // The integrity gate releases this target: an Open set the
                // gate is holding publishes no Sections, and a fixture that
                // left it held would prove only that nothing can be armed.
                gate_hold: false,
                gate_catalog_set_identity: None,
                attempt_id,
                completed_at: completed.clone(),
                open_sections: vec![section.clone()],
                source_value_count: 1,
                watched_sections: vec![section],
                http: OpenHttpAuditMetadata {
                    http_status: Some(200),
                    cache_status: Some(OpenCacheStatus::Miss),
                    decoded_bytes: Some(2),
                    decoded_body_sha256: Some("c".repeat(64)),
                    content_type: Some("application/json".to_owned()),
                    etag: None,
                    cache_control: Some("no-store".to_owned()),
                    date: None,
                    age_seconds: None,
                    last_modified: None,
                    retry_after: None,
                    retry_after_seconds: None,
                },
            })
            .expect("publish synthetic Open set");
    }
    drop(database);
    assert_ready(prepared, &terms);
}

/// Proves the fixture really left every target discoverable and released,
/// rather than leaving a later failure to be diagnosed from the outside.
fn assert_ready(prepared: &PreparedLocalRuntime, terms: &[TermId]) {
    let database = prepared.operational().database();
    let database = database.lock().unwrap();
    let discovered = database
        .operational()
        .discovered_targets()
        .expect("read fixture discovery targets");
    for term in terms {
        let target = TermCampusKey::new(
            term.clone(),
            CampusCode::try_from(FIXTURE_CAMPUS).expect("product Campus"),
        );
        assert!(
            discovered.contains(&target),
            "fixture target must be discovered"
        );
        assert!(
            database
                .operational()
                .complete_target_snapshot_state(&target)
                .expect("read fixture complete target state")
                .ready,
            "fixture target must be READY"
        );
    }
}
