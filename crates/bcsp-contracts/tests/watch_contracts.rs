use std::num::NonZeroU64;
use std::str::FromStr;

use bcsp_contracts::{
    ACTIVE_WATCH_STATE_PERSISTENT, ActiveWatchId, MAX_ACTIVE_WATCHES, OpenEpisodeId,
    OpenEpisodeState, OpenEpisodeV1, SectionKey, TraceId, WATCH_CONTRACT_VERSION,
    WatchContinuousDurationV1, WatchMaxAudible, WatchNotificationMode, WatchPolicyV1,
    WatchStartItemResultV1, WatchStartItemV1, WatchStartItemsV1, WatchStartRejectionReason,
    WatchStartResultV1,
};
use time::OffsetDateTime;

const _: () = assert!(!ACTIVE_WATCH_STATE_PERSISTENT);

fn trace(last: u8) -> TraceId {
    TraceId::from_str(&format!("00000000-0000-4000-8000-{last:012x}")).unwrap()
}

fn section(index: u16) -> SectionKey {
    SectionKey::try_new("T2026F", "CAMPUS_A", &format!("{index:05}")).unwrap()
}

#[test]
fn watch_contract_is_v1_connection_bound_and_nonpersistent() {
    assert_eq!(WATCH_CONTRACT_VERSION.as_u16(), 1);
    assert_eq!(MAX_ACTIVE_WATCHES, 9);
    assert!(serde_json::from_str::<bcsp_contracts::WatchContractVersion>("0").is_err());
    assert!(serde_json::from_str::<bcsp_contracts::WatchContractVersion>("2").is_err());
}

#[test]
fn positive_and_unlimited_audio_policy_values_are_exact() {
    assert!(WatchMaxAudible::try_from(0).is_err());
    assert_eq!(WatchMaxAudible::try_from(11).unwrap().get(), 11);
    assert!(WatchContinuousDurationV1::finite_seconds(0).is_err());
    assert_eq!(
        WatchContinuousDurationV1::finite_seconds(600)
            .unwrap()
            .seconds(),
        Some(600)
    );
    assert_eq!(WatchContinuousDurationV1::Unlimited.seconds(), None);
    assert!(
        serde_json::from_str::<WatchContinuousDurationV1>(r#"{"kind":"FINITE","seconds":0}"#)
            .is_err()
    );
}

#[test]
fn start_input_is_nonempty_and_identity_unique_but_tenth_item_reaches_per_item_result() {
    assert!(WatchStartItemsV1::try_from(Vec::new()).is_err());
    let policy = WatchPolicyV1::default();
    assert!(
        WatchStartItemsV1::try_from(vec![
            WatchStartItemV1::new(section(1), policy.clone()),
            WatchStartItemV1::new(section(1), policy.clone()),
        ])
        .is_err()
    );

    let ten = (1..=10)
        .map(|index| WatchStartItemV1::new(section(index), policy.clone()))
        .collect::<Vec<_>>();
    assert_eq!(
        WatchStartItemsV1::try_from(ten).unwrap().as_slice().len(),
        10
    );

    let mut results = (1..=9)
        .map(|index| WatchStartItemResultV1::Active {
            section_key: section(index),
            active_watch_id: ActiveWatchId::new(trace(index as u8 + 10)),
            started_at: OffsetDateTime::UNIX_EPOCH,
        })
        .collect::<Vec<_>>();
    results.push(WatchStartItemResultV1::Rejected {
        section_key: section(10),
        reason: WatchStartRejectionReason::MaxActiveWatches,
    });
    let result = WatchStartResultV1::try_new(results, 9).unwrap();
    assert_eq!(result.items().len(), 10);
    assert_eq!(result.active_watch_count(), 9);
    assert!(WatchStartResultV1::try_new(result.items().to_vec(), 10).is_err());
}

#[test]
fn initial_current_open_episode_is_bound_to_a_real_committed_observation() {
    let observation: bcsp_contracts::OpenObservationV1 =
        serde_json::from_str(include_str!("golden/open-observation-v1.json")).unwrap();
    let policy = WatchPolicyV1::new(
        WatchNotificationMode::Continuous,
        WatchMaxAudible::try_from(1).unwrap(),
        WatchContinuousDurationV1::Unlimited,
    );
    let episode = OpenEpisodeV1::try_new(
        OpenEpisodeId::new(trace(3)),
        ActiveWatchId::new(trace(2)),
        observation.section_key().clone(),
        OpenEpisodeState::Unacknowledged,
        &policy,
        3,
        observation.observed_at,
        observation.observed_at,
        NonZeroU64::new(1).unwrap(),
        observation.observation_id,
        observation.observed_at,
        None,
    )
    .unwrap();
    assert_eq!(episode.observation_count.get(), 1);
    assert_eq!(episode.latest_observation_id, observation.observation_id);
    assert_eq!(episode.first_observed_at, observation.observed_at);
    assert_eq!(episode.audible_count, 3);
    assert_eq!(episode.max_audible.get(), 1);
}
