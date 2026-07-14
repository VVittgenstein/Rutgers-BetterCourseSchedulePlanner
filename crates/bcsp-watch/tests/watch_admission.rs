use std::str::FromStr;

use bcsp_contracts::{
    ActiveWatchTargetV1, SectionKey, TraceId, TraceIdSource, WatchPolicyV1, WatchServerEventV1,
    WatchStartItemResultV1, WatchStartItemV1, WatchStartItemsV1, WatchStartRejectionReason,
    WatchStartResultV1,
};
use bcsp_watch::{WatchClock, WatchInstant, WatchManager, WatchStartAdmission};
use time::OffsetDateTime;

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

struct FakeIds(u64);

impl TraceIdSource for FakeIds {
    fn next_trace_id(&mut self) -> TraceId {
        self.0 += 1;
        trace(self.0)
    }
}

fn trace(value: u64) -> TraceId {
    TraceId::from_str(&format!("00000000-0000-4000-8000-{value:012x}")).expect("synthetic trace ID")
}

fn section(index: usize) -> SectionKey {
    SectionKey::try_new("92026", "NB", &format!("{index:05}")).expect("synthetic Section identity")
}

fn items(sections: impl IntoIterator<Item = SectionKey>) -> WatchStartItemsV1 {
    WatchStartItemsV1::try_from(
        sections
            .into_iter()
            .map(|section_key| WatchStartItemV1::new(section_key, WatchPolicyV1::default()))
            .collect::<Vec<_>>(),
    )
    .expect("nonempty unique START items")
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

#[test]
fn mixed_admission_is_ordered_and_rejections_do_not_consume_capacity() {
    let connection = trace(1);
    let found = section(1);
    let missing = section(2);
    let unavailable = section(3);
    let mut manager = WatchManager::try_new(
        FakeClock,
        FakeIds(0x100),
        std::time::Duration::from_secs(30),
    )
    .unwrap();
    manager.connect(connection).unwrap();

    let mixed = manager
        .start_with_admission(
            connection,
            trace(2),
            items([found.clone(), missing.clone(), unavailable.clone()]),
            |candidate| {
                if candidate == &missing {
                    WatchStartAdmission::SectionNotFound
                } else if candidate == &unavailable {
                    WatchStartAdmission::TargetUnavailable
                } else {
                    WatchStartAdmission::admitted(None)
                }
            },
        )
        .unwrap();
    let result = start_result(&mixed.dispatch.events);
    assert_eq!(result.active_watch_count(), 1);
    assert!(
        matches!(result.items()[0], WatchStartItemResultV1::Active { ref section_key, .. } if section_key == &found)
    );
    assert_eq!(
        result.items()[1],
        WatchStartItemResultV1::Rejected {
            section_key: missing,
            reason: WatchStartRejectionReason::SectionNotFound,
        }
    );
    assert_eq!(
        result.items()[2],
        WatchStartItemResultV1::Rejected {
            section_key: unavailable,
            reason: WatchStartRejectionReason::TargetUnavailable,
        }
    );

    manager
        .start_without_current(connection, trace(3), items((4..=10).map(section)))
        .unwrap();
    let rejected = section(11);
    let ninth = section(12);
    let tenth = section(13);
    let capped = manager
        .start_with_admission(
            connection,
            trace(4),
            items([rejected.clone(), ninth.clone(), tenth.clone()]),
            |candidate| {
                if candidate == &rejected {
                    WatchStartAdmission::SectionNotFound
                } else {
                    WatchStartAdmission::admitted(None)
                }
            },
        )
        .unwrap();
    let result = start_result(&capped.dispatch.events);
    assert_eq!(result.active_watch_count(), 9);
    assert_eq!(
        result.items()[0],
        WatchStartItemResultV1::Rejected {
            section_key: rejected,
            reason: WatchStartRejectionReason::SectionNotFound,
        }
    );
    assert!(
        matches!(result.items()[1], WatchStartItemResultV1::Active { ref section_key, .. } if section_key == &ninth)
    );
    assert_eq!(
        result.items()[2],
        WatchStartItemResultV1::Rejected {
            section_key: tenth,
            reason: WatchStartRejectionReason::MaxActiveWatches,
        }
    );
}

#[test]
fn rejected_restart_leaves_the_existing_watch_active() {
    let connection = trace(20);
    let watched = section(20);
    let mut manager = WatchManager::try_new(
        FakeClock,
        FakeIds(0x200),
        std::time::Duration::from_secs(30),
    )
    .unwrap();
    manager.connect(connection).unwrap();
    let started = manager
        .start_without_current(connection, trace(21), items([watched.clone()]))
        .unwrap();
    let active_watch_id = match &start_result(&started.dispatch.events).items()[0] {
        WatchStartItemResultV1::Active {
            active_watch_id, ..
        } => *active_watch_id,
        WatchStartItemResultV1::Rejected { .. } => panic!("watch should start"),
    };

    let rejected = manager
        .start_with_admission(connection, trace(22), items([watched.clone()]), |_| {
            WatchStartAdmission::SectionNotFound
        })
        .unwrap();
    assert!(rejected.dispatch.actions.is_empty());
    assert_eq!(
        manager.connection_watches(connection),
        vec![watched.clone()]
    );
    manager
        .stop_watch(
            connection,
            ActiveWatchTargetV1 {
                active_watch_id,
                section_key: watched,
            },
        )
        .expect("the original watch ID remains active");
}

#[test]
fn message_replay_preserves_the_first_admission_snapshot() {
    let connection = trace(30);
    let found = section(30);
    let missing = section(31);
    let message = trace(31);
    let mut manager = WatchManager::try_new(
        FakeClock,
        FakeIds(0x300),
        std::time::Duration::from_secs(30),
    )
    .unwrap();
    manager.connect(connection).unwrap();
    let first = manager
        .start_with_admission(
            connection,
            message,
            items([found.clone(), missing.clone()]),
            |candidate| {
                if candidate == &missing {
                    WatchStartAdmission::SectionNotFound
                } else {
                    WatchStartAdmission::admitted(None)
                }
            },
        )
        .unwrap();
    let replay = manager
        .start_with_admission(
            connection,
            message,
            items([found.clone(), missing]),
            |candidate| {
                if candidate == &found {
                    WatchStartAdmission::SectionNotFound
                } else {
                    WatchStartAdmission::admitted(None)
                }
            },
        )
        .unwrap();

    assert!(replay.replayed);
    assert_eq!(replay.dispatch, first.dispatch);
    assert_eq!(manager.connection_watches(connection), vec![found]);
}
