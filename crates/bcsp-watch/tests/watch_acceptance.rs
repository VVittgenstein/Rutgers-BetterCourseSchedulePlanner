use std::str::FromStr;

use bcsp_contracts::{
    ActiveWatchId, ActiveWatchTargetV1, CatalogContentVersion, MAX_ACTIVE_WATCHES,
    OPEN_CONTRACT_VERSION, OpenBatchKey, OpenCounterSnapshotV1, OpenEpisodeId, OpenEpisodeState,
    OpenEpisodeV1, OpenObservationV1, OpenPullCountsV1, OpenSequence, OpenState, RutgersDay,
    RutgersDayTimezone, SectionKey, TraceId, TraceIdSource, WatchAudioCueV1,
    WatchAudioDispositionV1, WatchAudioTriggerV1, WatchContinuousDurationV1, WatchCueOutcome,
    WatchCueOutcomeReceiptV1, WatchCueOutcomeReportV1, WatchEpisodeTargetV1, WatchMaxAudible,
    WatchNotificationMode, WatchPolicyV1, WatchServerEventV1, WatchStartItemResultV1,
    WatchStartItemV1, WatchStartItemsV1, WatchStartRejectionReason, WatchStartResultV1,
};
use bcsp_watch::{
    MAX_SELECTED_SECTIONS, SectionSelection, SelectionChange, SelectionError, WatchClock,
    WatchInstant, WatchManager,
};
use time::{Duration, OffsetDateTime};

const BASE_WALL_TIME: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

#[derive(Debug)]
struct FakeClock {
    monotonic: WatchInstant,
    wall: OffsetDateTime,
}

impl Default for FakeClock {
    fn default() -> Self {
        Self {
            monotonic: WatchInstant::ZERO,
            wall: BASE_WALL_TIME,
        }
    }
}

#[allow(dead_code)]
impl FakeClock {
    fn advance(&mut self, duration: std::time::Duration) {
        self.monotonic = self.monotonic.saturating_add(duration);
        self.wall += Duration::try_from(duration).expect("test duration fits time::Duration");
    }
}

impl WatchClock for FakeClock {
    fn now(&mut self) -> WatchInstant {
        self.monotonic
    }

    fn wall_now(&mut self) -> OffsetDateTime {
        self.wall
    }
}

#[derive(Debug)]
struct FakeIds {
    next: u64,
}

impl Default for FakeIds {
    fn default() -> Self {
        Self { next: 0x100 }
    }
}

impl TraceIdSource for FakeIds {
    fn next_trace_id(&mut self) -> TraceId {
        self.next += 1;
        trace(self.next)
    }
}

fn trace(value: u64) -> TraceId {
    TraceId::from_str(&format!("00000000-0000-4000-8000-{value:012x}"))
        .expect("synthetic trace ID is valid")
}

fn section(index: usize) -> SectionKey {
    SectionKey::try_new("92026", "NB", &format!("{index:05}"))
        .expect("synthetic Section identity is valid")
}

#[allow(dead_code)]
fn observation(
    section: &SectionKey,
    pull_sequence: u64,
    observation_id: u64,
    state: OpenState,
) -> OpenObservationV1 {
    let observed_at = BASE_WALL_TIME + Duration::seconds(pull_sequence as i64);
    OpenObservationV1::try_new(
        OPEN_CONTRACT_VERSION,
        trace(observation_id),
        trace(0xf000 + pull_sequence),
        OpenBatchKey::from(section.target()),
        section.clone(),
        OpenSequence::try_from(pull_sequence).expect("positive test sequence"),
        CatalogContentVersion::try_from(1).expect("positive catalog version"),
        state,
        observed_at,
        observed_at + Duration::seconds(60),
        0,
        OpenCounterSnapshotV1 {
            run_counts: None,
            today_counts: OpenPullCountsV1 {
                attempted: pull_sequence,
                succeeded: pull_sequence,
                failed: 0,
                empty: 0,
            },
            rutgers_day: RutgersDay::try_from("2026-07-14").expect("valid Rutgers day"),
            day_timezone: RutgersDayTimezone::AmericaNewYork,
        },
    )
    .expect("observation target matches Section")
}

fn start_items(sections: impl IntoIterator<Item = SectionKey>) -> WatchStartItemsV1 {
    WatchStartItemsV1::try_from(
        sections
            .into_iter()
            .map(|section_key| WatchStartItemV1::new(section_key, WatchPolicyV1::default()))
            .collect::<Vec<_>>(),
    )
    .expect("test start list is nonempty and unique")
}

fn start_items_with_policy(
    sections: impl IntoIterator<Item = SectionKey>,
    policy: WatchPolicyV1,
) -> WatchStartItemsV1 {
    WatchStartItemsV1::try_from(
        sections
            .into_iter()
            .map(|section_key| WatchStartItemV1::new(section_key, policy.clone()))
            .collect::<Vec<_>>(),
    )
    .expect("test start list is nonempty and unique")
}

fn start_result(events: &[WatchServerEventV1]) -> &WatchStartResultV1 {
    events
        .iter()
        .find_map(|event| match event {
            WatchServerEventV1::StartResult { result } => Some(result),
            _ => None,
        })
        .expect("START_RESULT event")
}

fn active_watch_id(result: &WatchStartResultV1, section: &SectionKey) -> ActiveWatchId {
    result
        .items()
        .iter()
        .find_map(|item| match item {
            WatchStartItemResultV1::Active {
                section_key,
                active_watch_id,
                ..
            } if section_key == section => Some(*active_watch_id),
            _ => None,
        })
        .expect("Section start is active")
}

fn requested_cue(events: &[WatchServerEventV1]) -> Option<&WatchAudioCueV1> {
    events.iter().find_map(|event| match event {
        WatchServerEventV1::AudioDisposition {
            audio: WatchAudioDispositionV1::CueRequested { cue },
        } => Some(cue),
        _ => None,
    })
}

fn latest_episode(events: &[WatchServerEventV1]) -> Option<&OpenEpisodeV1> {
    events.iter().rev().find_map(|event| match event {
        WatchServerEventV1::EpisodeUpdated { episode } => Some(episode),
        _ => None,
    })
}

fn cue_receipt(events: &[WatchServerEventV1]) -> &WatchCueOutcomeReceiptV1 {
    events
        .iter()
        .find_map(|event| match event {
            WatchServerEventV1::CueOutcomeRecorded { receipt } => Some(receipt),
            _ => None,
        })
        .expect("CUE_OUTCOME_RECORDED event")
}

fn active_mixer(
    events: &[WatchServerEventV1],
) -> Option<&std::collections::BTreeSet<OpenEpisodeId>> {
    events.iter().rev().find_map(|event| match event {
        WatchServerEventV1::AudioDisposition {
            audio: WatchAudioDispositionV1::ContinuousMixerActive { episode_ids, .. },
        } => Some(episode_ids),
        _ => None,
    })
}

#[test]
fn a_browser_session_accepts_nine_unique_sections_and_rejects_the_tenth() {
    let mut selection = SectionSelection::new();

    for index in 1..=MAX_SELECTED_SECTIONS {
        assert_eq!(selection.add(section(index)), Ok(SelectionChange::Added));
    }

    assert_eq!(
        selection.add(section(MAX_SELECTED_SECTIONS + 1)),
        Err(SelectionError::LimitExceeded)
    );
    assert_eq!(
        selection.sections(),
        (1..=9).map(section).collect::<Vec<_>>()
    );

    assert_eq!(
        selection.add(section(1)),
        Ok(SelectionChange::AlreadySelected),
        "an idempotent retry must not consume capacity"
    );
}

#[test]
fn connection_start_is_per_item_bounded_replay_safe_and_error_atomic() {
    let connection_id = trace(1);
    let message_id = trace(2);
    let sections = (1..=10).map(section).collect::<Vec<_>>();
    let items = start_items(sections.clone());
    let mut manager = WatchManager::try_new(
        FakeClock::default(),
        FakeIds::default(),
        std::time::Duration::from_secs(30),
    )
    .expect("valid manager configuration");
    manager.connect(connection_id).expect("new connection");

    let first = manager
        .start(connection_id, message_id, items.clone(), &[])
        .expect("valid start command");
    assert!(!first.replayed);
    let result = start_result(&first.dispatch.events);
    assert_eq!(result.active_watch_count(), MAX_ACTIVE_WATCHES);
    assert_eq!(
        result
            .items()
            .iter()
            .filter(|item| item.is_active())
            .count(),
        usize::from(MAX_ACTIVE_WATCHES)
    );
    assert!(matches!(
        result.items().last(),
        Some(WatchStartItemResultV1::Rejected {
            section_key,
            reason: WatchStartRejectionReason::MaxActiveWatches,
        }) if section_key == &sections[9]
    ));

    let first_watch = active_watch_id(result, &sections[0]);
    let replay = manager
        .start(connection_id, message_id, items, &[])
        .expect("exact message replay is accepted");
    assert!(replay.replayed);
    assert_eq!(
        active_watch_id(start_result(&replay.dispatch.events), &sections[0]),
        first_watch,
        "an exact START replay must preserve the watch generation"
    );

    let invalid_target = ActiveWatchTargetV1 {
        active_watch_id: ActiveWatchId::new(trace(0xdead)),
        section_key: sections[0].clone(),
    };
    assert!(manager.stop_watch(connection_id, invalid_target).is_err());
    assert_eq!(manager.connection_count(), 1);
    assert_eq!(manager.connection_watches(connection_id).len(), 9);
    assert!(
        manager
            .connection_watches(connection_id)
            .contains(&sections[0])
    );
}

#[test]
fn watches_are_connection_bound_and_cleanup_never_rehydrates_them() {
    let mut manager = WatchManager::try_new(
        FakeClock::default(),
        FakeIds::default(),
        std::time::Duration::from_secs(5),
    )
    .expect("valid manager configuration");

    let disconnected = trace(10);
    let disconnected_section = section(20);
    manager.connect(disconnected).unwrap();
    manager
        .start(
            disconnected,
            trace(11),
            start_items([disconnected_section.clone()]),
            &[],
        )
        .unwrap();
    manager.disconnect(disconnected).unwrap();
    assert_eq!(manager.connection_count(), 0);
    assert!(
        manager
            .watched_sections(&disconnected_section.target())
            .is_empty()
    );

    manager.connect(disconnected).unwrap();
    assert!(
        manager.connection_watches(disconnected).is_empty(),
        "reconnect creates a fresh browser session"
    );
    manager.disconnect(disconnected).unwrap();

    let expired = trace(12);
    let expired_section = section(21);
    manager.connect(expired).unwrap();
    manager
        .start(
            expired,
            trace(13),
            start_items([expired_section.clone()]),
            &[],
        )
        .unwrap();
    manager
        .clock_mut()
        .advance(std::time::Duration::from_secs(5));
    manager.tick().expect("tick conversion remains valid");
    assert_eq!(manager.connection_count(), 0);
    assert!(
        manager
            .watched_sections(&expired_section.target())
            .is_empty()
    );
    manager.connect(expired).unwrap();
    assert!(manager.connection_watches(expired).is_empty());
    manager.disconnect(expired).unwrap();

    let stopped = trace(14);
    manager.connect(stopped).unwrap();
    manager
        .start(stopped, trace(15), start_items([section(22)]), &[])
        .unwrap();
    let reports = manager.process_stop();
    assert_eq!(reports.len(), 1);
    assert_eq!(manager.connection_count(), 0);
}

#[test]
fn one_committed_observation_fans_out_once_without_duplicate_poll_demand() {
    let first_connection = trace(30);
    let second_connection = trace(31);
    let watched = section(30);
    let mut manager = WatchManager::try_new(
        FakeClock::default(),
        FakeIds::default(),
        std::time::Duration::from_secs(30),
    )
    .unwrap();
    for (connection, message) in [
        (first_connection, trace(32)),
        (second_connection, trace(33)),
    ] {
        manager.connect(connection).unwrap();
        manager
            .start(connection, message, start_items([watched.clone()]), &[])
            .unwrap();
    }

    assert_eq!(
        manager.watched_sections(&watched.target()),
        vec![watched.clone()]
    );
    assert_eq!(manager.active_demand_count(&watched.target()), 1);

    let committed = observation(&watched, 2, 0x200, OpenState::Open);
    let published = manager.publish(committed.clone()).unwrap();
    assert!(!published.replayed);
    assert_eq!(published.dispatches.len(), 2);
    let mut recipients = Vec::new();
    for dispatch in &published.dispatches {
        recipients.push(dispatch.connection_id);
        let fanouts = dispatch
            .events
            .iter()
            .filter_map(|event| match event {
                WatchServerEventV1::OpenObservation { fanout } => Some(fanout),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(fanouts.len(), 1);
        assert_eq!(
            fanouts[0].observation.observation_id,
            committed.observation_id
        );
        assert_eq!(fanouts[0].observation.section_key(), &watched);
    }
    recipients.sort_unstable();
    assert_eq!(recipients, vec![first_connection, second_connection]);

    let replay = manager.publish(committed.clone()).unwrap();
    assert!(replay.replayed);
    assert!(replay.dispatches.is_empty());
    let lower = manager
        .publish(observation(&watched, 1, 0x201, OpenState::Closed))
        .unwrap();
    assert!(lower.replayed);
    assert!(lower.dispatches.is_empty());
    assert!(
        manager
            .publish(observation(&watched, 2, 0x202, OpenState::Open))
            .is_err(),
        "one Section sequence cannot identify two observations"
    );
}

#[test]
fn observation_sequence_is_monotonic_per_section_not_per_target() {
    let connection = trace(40);
    let section_a = section(40);
    let section_b = section(41);
    assert_eq!(section_a.target(), section_b.target());
    let mut manager = WatchManager::try_new(
        FakeClock::default(),
        FakeIds::default(),
        std::time::Duration::from_secs(30),
    )
    .unwrap();
    manager.connect(connection).unwrap();
    manager
        .start(
            connection,
            trace(41),
            start_items([section_a.clone(), section_b.clone()]),
            &[],
        )
        .unwrap();

    manager
        .publish(observation(&section_a, 2, 0x210, OpenState::Open))
        .unwrap();
    let b_first = manager
        .publish(observation(&section_b, 1, 0x211, OpenState::Open))
        .unwrap();
    assert!(!b_first.replayed);
    assert_eq!(b_first.dispatches.len(), 1);
    assert!(b_first.dispatches[0].events.iter().any(|event| {
        matches!(
            event,
            WatchServerEventV1::OpenObservation { fanout }
                if fanout.observation.section_key() == &section_b
        )
    }));
}

#[test]
fn initial_open_is_notified_once_across_start_publish_race() {
    let existing_connection = trace(50);
    let joining_connection = trace(51);
    let watched = section(50);
    let committed = observation(&watched, 1, 0x220, OpenState::Open);
    let mut manager = WatchManager::try_new(
        FakeClock::default(),
        FakeIds::default(),
        std::time::Duration::from_secs(30),
    )
    .unwrap();

    manager.connect(existing_connection).unwrap();
    manager
        .start(
            existing_connection,
            trace(52),
            start_items([watched.clone()]),
            &[],
        )
        .unwrap();

    manager.connect(joining_connection).unwrap();
    let joining_start = manager
        .start(
            joining_connection,
            trace(53),
            start_items([watched.clone()]),
            std::slice::from_ref(&committed),
        )
        .unwrap();
    let initial_episode = latest_episode(&joining_start.dispatch.events)
        .expect("fresh initial OPEN creates an episode");
    assert_eq!(initial_episode.state, OpenEpisodeState::Unacknowledged);
    assert_eq!(
        initial_episode.latest_observation_id,
        committed.observation_id
    );
    assert!(
        requested_cue(&joining_start.dispatch.events).is_some(),
        "ONE_SHOT treats the fresh initial OPEN as an audible observation"
    );

    let published = manager.publish(committed).unwrap();
    assert_eq!(published.dispatches.len(), 2);
    let existing = published
        .dispatches
        .iter()
        .find(|dispatch| dispatch.connection_id == existing_connection)
        .unwrap();
    assert!(requested_cue(&existing.events).is_some());
    assert!(latest_episode(&existing.events).is_some());

    let joining = published
        .dispatches
        .iter()
        .find(|dispatch| dispatch.connection_id == joining_connection)
        .unwrap();
    assert!(
        latest_episode(&joining.events).is_none(),
        "joining watch already consumed this committed observation"
    );
    assert!(requested_cue(&joining.events).is_none());
    assert!(!joining.events.iter().any(|event| matches!(
        event,
        WatchServerEventV1::AudioDisposition {
            audio: WatchAudioDispositionV1::CueQueued { .. }
                | WatchAudioDispositionV1::SilentMaxAudible { .. }
        }
    )));
}

#[test]
fn stale_initial_open_seeds_replay_without_opening_an_episode_or_cue() {
    let connection = trace(55);
    let watched = section(55);
    let stale = observation(&watched, 1, 0x225, OpenState::Open);
    let mut manager = WatchManager::try_new(
        FakeClock::default(),
        FakeIds::default(),
        std::time::Duration::from_secs(300),
    )
    .unwrap();
    manager
        .clock_mut()
        .advance(std::time::Duration::from_secs(120));
    manager.connect(connection).unwrap();

    let started = manager
        .start(
            connection,
            trace(56),
            start_items([watched]),
            std::slice::from_ref(&stale),
        )
        .unwrap();
    assert!(latest_episode(&started.dispatch.events).is_none());
    assert!(requested_cue(&started.dispatch.events).is_none());

    let published = manager.publish(stale).unwrap();
    assert!(!published.replayed);
    assert_eq!(published.dispatches.len(), 1);
    assert!(latest_episode(&published.dispatches[0].events).is_none());
    assert!(requested_cue(&published.dispatches[0].events).is_none());
}

#[test]
fn unknown_never_rearms_but_closed_then_open_creates_a_new_episode() {
    let connection = trace(60);
    let watched = section(60);
    let mut manager = WatchManager::try_new(
        FakeClock::default(),
        FakeIds::default(),
        std::time::Duration::from_secs(30),
    )
    .unwrap();
    manager.connect(connection).unwrap();
    manager
        .start(connection, trace(61), start_items([watched.clone()]), &[])
        .unwrap();

    let unknown = manager
        .publish(observation(&watched, 1, 0x230, OpenState::Unknown))
        .unwrap();
    assert!(latest_episode(&unknown.dispatches[0].events).is_none());

    let first_open = manager
        .publish(observation(&watched, 2, 0x231, OpenState::Open))
        .unwrap();
    let first = latest_episode(&first_open.dispatches[0].events).unwrap();
    let first_episode_id = first.episode_id;
    assert_eq!(first.state, OpenEpisodeState::Unacknowledged);
    assert_eq!(first.observation_count.get(), 1);

    let repeated_open = manager
        .publish(observation(&watched, 3, 0x232, OpenState::Open))
        .unwrap();
    let repeated = latest_episode(&repeated_open.dispatches[0].events).unwrap();
    assert_eq!(repeated.episode_id, first_episode_id);
    assert_eq!(repeated.observation_count.get(), 2);

    let unknown_again = manager
        .publish(observation(&watched, 4, 0x233, OpenState::Unknown))
        .unwrap();
    assert!(latest_episode(&unknown_again.dispatches[0].events).is_none());

    let closed = manager
        .publish(observation(&watched, 5, 0x234, OpenState::Closed))
        .unwrap();
    let closed_episode = latest_episode(&closed.dispatches[0].events).unwrap();
    assert_eq!(closed_episode.episode_id, first_episode_id);
    assert_eq!(closed_episode.state, OpenEpisodeState::Closed);

    let reopened = manager
        .publish(observation(&watched, 6, 0x235, OpenState::Open))
        .unwrap();
    let second = latest_episode(&reopened.dispatches[0].events).unwrap();
    assert_ne!(second.episode_id, first_episode_id);
    assert_eq!(second.state, OpenEpisodeState::Unacknowledged);
    assert_eq!(second.observation_count.get(), 1);
}

#[test]
fn one_shot_burst_is_ordered_and_only_started_audio_consumes_the_cap() {
    let connection = trace(70);
    let watched = section(70);
    let policy = WatchPolicyV1::new(
        WatchNotificationMode::OneShot,
        WatchMaxAudible::try_from(2).unwrap(),
        WatchContinuousDurationV1::default(),
    );
    let mut manager = WatchManager::try_new(
        FakeClock::default(),
        FakeIds::default(),
        std::time::Duration::from_secs(30),
    )
    .unwrap();
    manager.connect(connection).unwrap();
    let started = manager
        .start(
            connection,
            trace(71),
            start_items_with_policy([watched.clone()], policy),
            &[],
        )
        .unwrap();
    let active_watch_id = active_watch_id(start_result(&started.dispatch.events), &watched);

    let first = manager
        .publish(observation(&watched, 1, 0x240, OpenState::Open))
        .unwrap();
    let first_cue = requested_cue(&first.dispatches[0].events).unwrap().clone();
    assert!(matches!(
        first_cue.trigger,
        WatchAudioTriggerV1::OneShotObservation { observation_id }
            if observation_id == trace(0x240)
    ));

    for (sequence, id) in [(2, 0x241), (3, 0x242)] {
        let queued = manager
            .publish(observation(&watched, sequence, id, OpenState::Open))
            .unwrap();
        assert!(queued.dispatches[0].events.iter().any(|event| matches!(
            event,
            WatchServerEventV1::AudioDisposition {
                audio: WatchAudioDispositionV1::CueQueued { observation_id, .. }
            } if *observation_id == trace(id)
        )));
    }

    let muted = manager
        .report_cue_outcome(
            connection,
            WatchCueOutcomeReportV1 {
                cue_id: first_cue.cue_id,
                active_watch_id,
                section_key: watched.clone(),
                outcome: WatchCueOutcome::Muted,
                reported_at: BASE_WALL_TIME,
            },
        )
        .unwrap();
    assert!(!cue_receipt(&muted.events).audible_count_consumed);
    assert_eq!(cue_receipt(&muted.events).audible_count, 0);
    let second_cue = requested_cue(&muted.events).unwrap().clone();
    assert!(matches!(
        second_cue.trigger,
        WatchAudioTriggerV1::OneShotObservation { observation_id }
            if observation_id == trace(0x241)
    ));

    let blocked = manager
        .report_cue_outcome(
            connection,
            WatchCueOutcomeReportV1 {
                cue_id: second_cue.cue_id,
                active_watch_id,
                section_key: watched.clone(),
                outcome: WatchCueOutcome::AutoplayBlocked,
                reported_at: BASE_WALL_TIME,
            },
        )
        .unwrap();
    assert!(!cue_receipt(&blocked.events).audible_count_consumed);
    assert_eq!(cue_receipt(&blocked.events).audible_count, 0);
    let third_cue = requested_cue(&blocked.events).unwrap().clone();
    assert!(matches!(
        third_cue.trigger,
        WatchAudioTriggerV1::OneShotObservation { observation_id }
            if observation_id == trace(0x242)
    ));

    let first_started = manager
        .report_cue_outcome(
            connection,
            WatchCueOutcomeReportV1 {
                cue_id: third_cue.cue_id,
                active_watch_id,
                section_key: watched.clone(),
                outcome: WatchCueOutcome::Started,
                reported_at: BASE_WALL_TIME,
            },
        )
        .unwrap();
    assert!(cue_receipt(&first_started.events).audible_count_consumed);
    assert_eq!(cue_receipt(&first_started.events).audible_count, 1);

    let fourth = manager
        .publish(observation(&watched, 4, 0x243, OpenState::Open))
        .unwrap();
    let fourth_cue = requested_cue(&fourth.dispatches[0].events).unwrap().clone();
    let second_started = manager
        .report_cue_outcome(
            connection,
            WatchCueOutcomeReportV1 {
                cue_id: fourth_cue.cue_id,
                active_watch_id,
                section_key: watched.clone(),
                outcome: WatchCueOutcome::Started,
                reported_at: BASE_WALL_TIME,
            },
        )
        .unwrap();
    assert_eq!(cue_receipt(&second_started.events).audible_count, 2);

    let capped = manager
        .publish(observation(&watched, 5, 0x244, OpenState::Open))
        .unwrap();
    assert!(capped.dispatches[0].events.iter().any(|event| matches!(
        event,
        WatchServerEventV1::AudioDisposition {
            audio: WatchAudioDispositionV1::SilentMaxAudible {
                observation_id,
                audible_count: 2,
                ..
            }
        } if *observation_id == trace(0x244)
    )));
    assert!(
        capped.dispatches[0]
            .events
            .iter()
            .any(|event| matches!(event, WatchServerEventV1::OpenObservation { .. }))
    );
    assert!(latest_episode(&capped.dispatches[0].events).is_some());
    assert_eq!(manager.connection_watches(connection), vec![watched]);
}

#[test]
fn cue_outcome_target_cannot_consume_another_watchs_cue() {
    let connection = trace(80);
    let section_a = section(80);
    let section_b = section(81);
    let mut manager = WatchManager::try_new(
        FakeClock::default(),
        FakeIds::default(),
        std::time::Duration::from_secs(30),
    )
    .unwrap();
    manager.connect(connection).unwrap();
    let started = manager
        .start(
            connection,
            trace(81),
            start_items([section_a.clone(), section_b.clone()]),
            &[],
        )
        .unwrap();
    let result = start_result(&started.dispatch.events);
    let watch_a = active_watch_id(result, &section_a);
    let watch_b = active_watch_id(result, &section_b);
    manager
        .publish(observation(&section_a, 1, 0x250, OpenState::Open))
        .unwrap();
    let published_b = manager
        .publish(observation(&section_b, 1, 0x251, OpenState::Open))
        .unwrap();
    let cue_b = requested_cue(&published_b.dispatches[0].events)
        .unwrap()
        .clone();

    let forged = WatchCueOutcomeReportV1 {
        cue_id: cue_b.cue_id,
        active_watch_id: watch_a,
        section_key: section_a,
        outcome: WatchCueOutcome::Started,
        reported_at: BASE_WALL_TIME,
    };
    assert!(manager.report_cue_outcome(connection, forged).is_err());

    let valid = manager
        .report_cue_outcome(
            connection,
            WatchCueOutcomeReportV1 {
                cue_id: cue_b.cue_id,
                active_watch_id: watch_b,
                section_key: section_b,
                outcome: WatchCueOutcome::Started,
                reported_at: BASE_WALL_TIME,
            },
        )
        .unwrap();
    assert!(cue_receipt(&valid.events).audible_count_consumed);
    assert_eq!(cue_receipt(&valid.events).audible_count, 1);
}

#[test]
fn continuous_alarm_aggregates_acknowledges_times_out_resumes_and_rearms() {
    let connection = trace(90);
    let section_a = section(90);
    let section_c = section(91);
    let section_d = section(92);
    let policy = WatchPolicyV1::new(
        WatchNotificationMode::Continuous,
        WatchMaxAudible::default(),
        WatchContinuousDurationV1::finite_seconds(2).unwrap(),
    );
    let mut manager = WatchManager::try_new(
        FakeClock::default(),
        FakeIds::default(),
        std::time::Duration::from_secs(30),
    )
    .unwrap();
    manager.connect(connection).unwrap();
    let started = manager
        .start(
            connection,
            trace(91),
            start_items_with_policy(
                [section_a.clone(), section_c.clone(), section_d.clone()],
                policy,
            ),
            &[],
        )
        .unwrap();
    let result = start_result(&started.dispatch.events);
    let watch_a = active_watch_id(result, &section_a);
    let watch_c = active_watch_id(result, &section_c);
    let _watch_d = active_watch_id(result, &section_d);

    let mut episode_ids = std::collections::BTreeSet::new();
    let mut episode_a = None;
    let mut episode_c = None;
    for (section_key, id, destination) in [
        (&section_a, 0x260, &mut episode_a),
        (&section_c, 0x261, &mut episode_c),
    ] {
        let outcome = manager
            .publish(observation(section_key, 1, id, OpenState::Open))
            .unwrap();
        let episode = latest_episode(&outcome.dispatches[0].events).unwrap();
        *destination = Some(episode.episode_id);
        episode_ids.insert(episode.episode_id);
        assert_eq!(
            active_mixer(&outcome.dispatches[0].events),
            Some(&episode_ids)
        );
    }
    let opened_d = manager
        .publish(observation(&section_d, 1, 0x262, OpenState::Open))
        .unwrap();
    let episode_d = latest_episode(&opened_d.dispatches[0].events)
        .unwrap()
        .episode_id;
    episode_ids.insert(episode_d);
    assert_eq!(
        active_mixer(&opened_d.dispatches[0].events),
        Some(&episode_ids)
    );
    let episode_a = episode_a.unwrap();
    let episode_c = episode_c.unwrap();

    let acknowledged_c = manager
        .acknowledge_episode(
            connection,
            WatchEpisodeTargetV1 {
                active_watch_id: watch_c,
                episode_id: episode_c,
                section_key: section_c.clone(),
            },
        )
        .unwrap();
    episode_ids.remove(&episode_c);
    assert_eq!(active_mixer(&acknowledged_c.events), Some(&episode_ids));

    manager
        .clock_mut()
        .advance(std::time::Duration::from_secs(2));
    let timed_out = manager.tick().unwrap();
    assert_eq!(timed_out.dispatches.len(), 1);
    let timeout_events = &timed_out.dispatches[0].events;
    let timed_out_ids = timeout_events
        .iter()
        .filter_map(|event| match event {
            WatchServerEventV1::EpisodeUpdated { episode }
                if episode.state == OpenEpisodeState::TimedOut =>
            {
                Some(episode.episode_id)
            }
            _ => None,
        })
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        timed_out_ids,
        std::collections::BTreeSet::from([episode_a, episode_d])
    );
    assert!(timeout_events.iter().any(|event| matches!(
        event,
        WatchServerEventV1::AudioDisposition {
            audio: WatchAudioDispositionV1::ContinuousMixerStopped { .. }
        }
    )));

    let resumed_a = manager
        .resume_episode(
            connection,
            WatchEpisodeTargetV1 {
                active_watch_id: watch_a,
                episode_id: episode_a,
                section_key: section_a.clone(),
            },
        )
        .unwrap();
    assert_eq!(
        active_mixer(&resumed_a.events),
        Some(&std::collections::BTreeSet::from([episode_a]))
    );
    let acknowledged_all = manager.acknowledge_all(connection).unwrap();
    assert!(acknowledged_all.events.iter().any(|event| matches!(
        event,
        WatchServerEventV1::AudioDisposition {
            audio: WatchAudioDispositionV1::ContinuousMixerStopped { .. }
        }
    )));

    manager
        .publish(observation(&section_a, 2, 0x263, OpenState::Closed))
        .unwrap();
    let reopened = manager
        .publish(observation(&section_a, 3, 0x264, OpenState::Open))
        .unwrap();
    let rearmed = latest_episode(&reopened.dispatches[0].events).unwrap();
    assert_ne!(rearmed.episode_id, episode_a);
    assert_eq!(rearmed.state, OpenEpisodeState::Unacknowledged);
    assert_eq!(
        active_mixer(&reopened.dispatches[0].events),
        Some(&std::collections::BTreeSet::from([rearmed.episode_id]))
    );

    assert_eq!(manager.connection_watches(connection).len(), 3);
}
