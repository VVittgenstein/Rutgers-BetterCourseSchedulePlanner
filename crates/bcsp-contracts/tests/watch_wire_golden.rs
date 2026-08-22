use std::num::NonZeroU64;
use std::str::FromStr;

use bcsp_contracts::{
    ActiveWatchId, OpenEpisodeId, OpenEpisodeState, OpenEpisodeV1, TraceId, WATCH_CONTRACT_VERSION,
    WatchClientCommandV1, WatchContinuousDurationV1, WatchMaxAudible, WatchNotificationMode,
    WatchOpenObservationFanoutV1, WatchPolicyV1, WatchServerEventV1, WatchStartItemResultV1,
    WatchStartItemV1, WatchStartItemsV1, WatchStartRejectionReason, WatchStartResultV1,
};
use serde::Serialize;
use time::OffsetDateTime;

fn trace(value: &str) -> TraceId {
    TraceId::from_str(value).unwrap()
}

fn pretty<T: Serialize>(value: &T) -> String {
    format!("{}\n", serde_json::to_string_pretty(value).unwrap())
}

fn canonical(value: &str) -> String {
    value.replace("\r\n", "\n")
}

#[test]
fn start_command_wire_is_exact() {
    let command = WatchClientCommandV1::StartWatch {
        items: WatchStartItemsV1::try_from(vec![WatchStartItemV1::new(
            bcsp_contracts::SectionKey::try_new("T2026F", "CAMPUS_A", "00001").unwrap(),
            WatchPolicyV1::default(),
        )])
        .unwrap(),
    };
    let golden = include_str!("golden/watch-client-start-v1.json");
    assert_eq!(pretty(&command), canonical(golden));
    assert_eq!(
        serde_json::from_str::<WatchClientCommandV1>(golden).unwrap(),
        command
    );
}

#[test]
fn max_nine_start_result_wire_is_exact() {
    let result = WatchStartResultV1::try_new(
        vec![
            WatchStartItemResultV1::Active {
                section_key: bcsp_contracts::SectionKey::try_new("T2026F", "CAMPUS_A", "00001")
                    .unwrap(),
                active_watch_id: ActiveWatchId::new(trace("00000000-0000-4000-8000-000000000002")),
                started_at: OffsetDateTime::UNIX_EPOCH,
            },
            WatchStartItemResultV1::Rejected {
                section_key: bcsp_contracts::SectionKey::try_new("T2026F", "CAMPUS_A", "00010")
                    .unwrap(),
                reason: WatchStartRejectionReason::MaxActiveWatches,
            },
        ],
        9,
    )
    .unwrap();
    let event = WatchServerEventV1::StartResult { result };
    let golden = include_str!("golden/watch-server-start-result-v1.json");
    assert_eq!(pretty(&event), canonical(golden));
    assert_eq!(
        serde_json::from_str::<WatchServerEventV1>(golden).unwrap(),
        event
    );
}

#[test]
fn heartbeat_ping_and_ack_wires_are_exact() {
    let event = WatchServerEventV1::Ping { sequence: 1 };
    let golden = include_str!("golden/watch-server-ping-v1.json");
    assert_eq!(pretty(&event), canonical(golden));
    assert_eq!(
        serde_json::from_str::<WatchServerEventV1>(golden).unwrap(),
        event
    );

    let command = WatchClientCommandV1::HeartbeatAck { sequence: 1 };
    let golden = include_str!("golden/watch-client-heartbeat-ack-v1.json");
    assert_eq!(pretty(&command), canonical(golden));
    assert_eq!(
        serde_json::from_str::<WatchClientCommandV1>(golden).unwrap(),
        command
    );
}

#[test]
fn fanout_embeds_the_open_observation_without_copying_open_fields() {
    let observation: bcsp_contracts::OpenObservationV1 =
        serde_json::from_str(include_str!("golden/open-observation-v1.json")).unwrap();
    let event = WatchServerEventV1::OpenObservation {
        fanout: WatchOpenObservationFanoutV1 {
            contract_version: WATCH_CONTRACT_VERSION,
            active_watch_id: ActiveWatchId::new(trace("00000000-0000-4000-8000-000000000002")),
            observation,
        },
    };
    let golden = include_str!("golden/watch-server-observation-v1.json");
    assert_eq!(pretty(&event), canonical(golden));
    assert_eq!(
        serde_json::from_str::<WatchServerEventV1>(golden).unwrap(),
        event
    );
    let value = serde_json::to_value(event).unwrap();
    assert!(value["fanout"].get("sectionKey").is_none());
    assert!(value["fanout"]["observation"].get("sectionKey").is_some());
}

#[test]
fn initial_open_episode_wire_is_exact_and_observation_backed() {
    let observation: bcsp_contracts::OpenObservationV1 =
        serde_json::from_str(include_str!("golden/open-observation-v1.json")).unwrap();
    let policy = WatchPolicyV1::new(
        WatchNotificationMode::Continuous,
        WatchMaxAudible::try_from(3).unwrap(),
        WatchContinuousDurationV1::Unlimited,
    );
    let episode = OpenEpisodeV1::try_new(
        OpenEpisodeId::new(trace("00000000-0000-4000-8000-000000000003")),
        ActiveWatchId::new(trace("00000000-0000-4000-8000-000000000002")),
        observation.section_key().clone(),
        OpenEpisodeState::Unacknowledged,
        &policy,
        0,
        observation.observed_at,
        observation.observed_at,
        NonZeroU64::new(1).unwrap(),
        observation.observation_id,
        observation.observed_at,
        None,
    )
    .unwrap();
    let event = WatchServerEventV1::EpisodeUpdated { episode };
    let golden = include_str!("golden/watch-server-episode-v1.json");
    assert_eq!(pretty(&event), canonical(golden));
    assert_eq!(
        serde_json::from_str::<WatchServerEventV1>(golden).unwrap(),
        event
    );
}
