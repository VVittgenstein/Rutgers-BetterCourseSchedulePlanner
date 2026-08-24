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

use bcsp_catalog::{normalize_target, to_catalog_refresh_command, to_discovery_refresh_command};
use bcsp_contracts::{CampusCode, SectionKey, TermCampusKey, TermId, TraceId};
use bcsp_local_runtime::PreparedLocalRuntime;
use bcsp_operational_storage::{
    BeginOpenPullAttemptCommand, EmptySnapshotDecision, FinishOpenPullSuccessCommand,
    OpenCacheStatus, OpenHttpAuditMetadata, OpenRequestLane, PublishOutcome,
};
use bcsp_rutgers_client::{
    DiscoverySnapshot as RutgersDiscoverySnapshot, DiscoverySourceInput, SourceProvenance,
    decode_catalog_payload, decode_discovery_payload,
};
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
