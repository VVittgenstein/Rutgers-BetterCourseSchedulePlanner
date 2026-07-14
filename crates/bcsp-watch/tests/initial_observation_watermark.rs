use std::str::FromStr;

use bcsp_contracts::{
    CatalogContentVersion, OPEN_CONTRACT_VERSION, OpenBatchKey, OpenCounterSnapshotV1,
    OpenObservationV1, OpenPullCountsV1, OpenSequence, OpenState, RutgersDay, RutgersDayTimezone,
    SectionKey, TraceId, TraceIdSource, WatchPolicyV1, WatchServerEventV1, WatchStartItemV1,
    WatchStartItemsV1,
};
use bcsp_watch::{WatchClock, WatchInstant, WatchManager};
use time::{Duration, OffsetDateTime};

#[derive(Default)]
struct FakeClock;

impl WatchClock for FakeClock {
    fn now(&mut self) -> WatchInstant {
        WatchInstant::ZERO
    }

    fn wall_now(&mut self) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH
    }
}

#[derive(Default)]
struct FakeIds(u64);

impl TraceIdSource for FakeIds {
    fn next_trace_id(&mut self) -> TraceId {
        self.0 += 1;
        trace(0x100 + self.0)
    }
}

fn trace(value: u64) -> TraceId {
    TraceId::from_str(&format!("00000000-0000-4000-8000-{value:012x}")).expect("synthetic trace ID")
}

fn section() -> SectionKey {
    SectionKey::try_new("92026", "NB", "00001").expect("synthetic Section")
}

fn observation(
    section: &SectionKey,
    sequence: u64,
    observation_id: u64,
    state: OpenState,
) -> OpenObservationV1 {
    let observed_at = OffsetDateTime::UNIX_EPOCH + Duration::seconds(sequence as i64);
    OpenObservationV1::try_new(
        OPEN_CONTRACT_VERSION,
        trace(observation_id),
        trace(0xf000 + sequence),
        OpenBatchKey::from(section.target()),
        section.clone(),
        OpenSequence::try_from(sequence).expect("positive sequence"),
        CatalogContentVersion::try_from(1).expect("positive Catalog version"),
        state,
        observed_at,
        observed_at + Duration::minutes(5),
        0,
        OpenCounterSnapshotV1 {
            run_counts: None,
            today_counts: OpenPullCountsV1 {
                attempted: sequence,
                succeeded: sequence,
                failed: 0,
                empty: 0,
            },
            rutgers_day: RutgersDay::try_from("2026-07-14").expect("Rutgers day"),
            day_timezone: RutgersDayTimezone::AmericaNewYork,
        },
    )
    .expect("matching observation target")
}

fn start_items(section: SectionKey) -> WatchStartItemsV1 {
    WatchStartItemsV1::try_from(vec![WatchStartItemV1::new(
        section,
        WatchPolicyV1::default(),
    )])
    .expect("one valid start item")
}

#[test]
fn initial_snapshot_raises_global_watermark_without_losing_first_exact_fanout() {
    let section = section();
    let current = observation(&section, 2, 0x200, OpenState::Open);
    let first_connection = trace(1);
    let second_connection = trace(2);
    let mut manager = WatchManager::try_new(
        FakeClock,
        FakeIds::default(),
        std::time::Duration::from_secs(30),
    )
    .expect("manager");

    manager.connect(first_connection).expect("first connection");
    manager
        .start(
            first_connection,
            trace(3),
            start_items(section.clone()),
            std::slice::from_ref(&current),
        )
        .expect("start from current snapshot");
    manager
        .connect(second_connection)
        .expect("second connection");
    manager
        .start_without_current(second_connection, trace(4), start_items(section.clone()))
        .expect("second start");

    let delayed_lower = manager
        .publish(observation(&section, 1, 0x201, OpenState::Closed))
        .expect("lower observation is classified");
    assert!(delayed_lower.replayed);
    assert!(delayed_lower.dispatches.is_empty());

    let first_exact = manager.publish(current.clone()).expect("exact publish");
    assert!(!first_exact.replayed);
    assert_eq!(first_exact.dispatches.len(), 2);
    let first_events = &first_exact
        .dispatches
        .iter()
        .find(|dispatch| dispatch.connection_id == first_connection)
        .expect("first recipient")
        .events;
    assert!(first_events.iter().any(|event| matches!(
        event,
        WatchServerEventV1::OpenObservation { fanout }
            if fanout.observation.observation_id == current.observation_id
    )));
    assert!(
        !first_events
            .iter()
            .any(|event| matches!(event, WatchServerEventV1::EpisodeUpdated { .. }))
    );

    let exact_replay = manager.publish(current).expect("exact replay");
    assert!(exact_replay.replayed);
    assert!(exact_replay.dispatches.is_empty());

    let lower_after_publish = manager
        .publish(observation(&section, 1, 0x202, OpenState::Open))
        .expect("watermark remains monotonic");
    assert!(lower_after_publish.replayed);
    assert!(lower_after_publish.dispatches.is_empty());
}
