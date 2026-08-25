use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

use bcsp_application::WatchDispatchSink;
use bcsp_contracts::{
    OpenEpisodeState, OpenEpisodeV1, SystemTraceIdSource, TraceId, TraceIdSource,
    WatchServerEventV1, WatchStopReason,
};
use bcsp_local_user_state::{
    EpisodeActionInput, EpisodeActionKind, EpisodeDisposition, EpisodeHistoryIdentity,
    EpisodeHistorySummary, EpisodeSummaryInput, HistoryFilter, PageRequest, PersonalStateError,
    PersonalStateResult, PersonalStateStore, UnixMillis,
};
use bcsp_watch::{
    WatchAction, WatchActionKind, WatchCleanupReason, WatchCleanupReport, WatchDispatch,
};

/// Projects the shared in-memory watch reducer into local-only durable episode history.
pub(crate) struct LocalWatchHistorySink {
    sender: Option<mpsc::Sender<LocalWatchHistoryWork>>,
    worker: Option<JoinHandle<()>>,
}

enum LocalWatchHistoryWork {
    Dispatch(WatchDispatch),
    Cleanup(WatchCleanupReport),
    Flush(mpsc::SyncSender<()>),
}

struct LocalWatchHistoryWriter {
    store: PersonalStateStore,
    run_id: TraceId,
}

impl LocalWatchHistorySink {
    pub(crate) fn new(store: PersonalStateStore) -> Self {
        let mut ids = SystemTraceIdSource;
        Self::with_run_id(store, ids.next_trace_id())
    }

    fn with_run_id(store: PersonalStateStore, run_id: TraceId) -> Self {
        let (sender, receiver) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("bcsp-watch-history".to_owned())
            .spawn(move || LocalWatchHistoryWriter { store, run_id }.run(receiver))
            .expect("local watch history worker must start");
        Self {
            sender: Some(sender),
            worker: Some(worker),
        }
    }

    fn enqueue(&self, work: LocalWatchHistoryWork) {
        let Some(sender) = &self.sender else {
            tracing::error!("local watch history worker is unavailable");
            return;
        };
        if sender.send(work).is_err() {
            tracing::error!("local watch history worker stopped unexpectedly");
        }
    }

    fn flush_queue(&self) {
        let (completed, waiting) = mpsc::sync_channel(0);
        let Some(sender) = &self.sender else {
            tracing::error!("local watch history worker is unavailable during flush");
            return;
        };
        if sender
            .send(LocalWatchHistoryWork::Flush(completed))
            .is_err()
        {
            tracing::error!("local watch history worker stopped before flush");
            return;
        }
        if waiting.recv().is_err() {
            tracing::error!("local watch history worker failed during flush");
        }
    }
}

impl Drop for LocalWatchHistorySink {
    fn drop(&mut self) {
        self.flush_queue();
        self.sender.take();
        if let Some(worker) = self.worker.take()
            && worker.join().is_err()
        {
            tracing::error!("local watch history worker panicked");
        }
    }
}

impl LocalWatchHistoryWriter {
    fn run(mut self, receiver: mpsc::Receiver<LocalWatchHistoryWork>) {
        while let Ok(work) = receiver.recv() {
            match work {
                LocalWatchHistoryWork::Dispatch(dispatch) => {
                    if let Err(error) = self.persist_dispatch(&dispatch) {
                        tracing::error!(?error, "failed to persist local watch dispatch history");
                    }
                }
                LocalWatchHistoryWork::Cleanup(cleanup) => {
                    if let Err(error) = self.persist_cleanup(&cleanup) {
                        tracing::error!(?error, "failed to persist local watch cleanup history");
                    }
                }
                LocalWatchHistoryWork::Flush(completed) => {
                    let _ = completed.send(());
                }
            }
        }
    }

    fn persist_dispatch(&mut self, dispatch: &WatchDispatch) -> PersonalStateResult<()> {
        // The one place every alert the runtime raises passes through. The
        // code is what the local console turns into a line the user can read;
        // the history write below is what makes it durable.
        for event in &dispatch.events {
            if let WatchServerEventV1::AlertUpdated { alert } = event
                && alert.visible
            {
                let section = &alert.episode.section_key;
                tracing::info!(
                    code = "LOCAL_SECTION_OPEN",
                    section = %format!(
                        "{} {} {}",
                        section.term().as_str(),
                        section.campus().as_str(),
                        section.index().as_str()
                    ),
                    "a watched section is open",
                );
            }
        }
        let store = &mut self.store;
        let stop_reasons = dispatch
            .events
            .iter()
            .filter_map(|event| match event {
                WatchServerEventV1::WatchStopped { stopped } => {
                    Some((stopped.section_key.clone(), stopped.reason))
                }
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        let restarted = dispatch
            .actions
            .iter()
            .filter(|action| action.kind == WatchActionKind::WatchRestarted)
            .filter_map(|action| action.section_key.clone())
            .collect::<BTreeSet<_>>();

        for episode in dispatch.events.iter().filter_map(|event| match event {
            WatchServerEventV1::EpisodeUpdated { episode } => Some(episode),
            _ => None,
        }) {
            let identity = EpisodeHistoryIdentity::new(
                episode.section_key.clone(),
                self.run_id,
                episode.episode_id,
            );
            let existing = find_summary(store, &identity)?;
            let should_record_observation = existing.as_ref().is_some_and(|previous| {
                episode.observation_count > previous.observation_count
                    && episode.latest_observation_id != previous.last_observation_id
            });
            let closed_disposition = if episode.state == OpenEpisodeState::Closed {
                Some(match stop_reasons.get(&episode.section_key).copied() {
                    Some(reason) => EpisodeDisposition::WatchStopped { reason },
                    None if restarted.contains(&episode.section_key) => {
                        EpisodeDisposition::WatchStopped {
                            reason: WatchStopReason::UserRequested,
                        }
                    }
                    None => EpisodeDisposition::SectionClosed,
                })
            } else {
                None
            };
            let summary = project_episode_summary(
                identity.clone(),
                episode,
                existing.as_ref(),
                closed_disposition,
            )?;
            store.upsert_episode_summary(&summary)?;
            if should_record_observation {
                store.append_episode_action(&EpisodeActionInput {
                    action_id: episode.latest_observation_id,
                    identity,
                    kind: EpisodeActionKind::Updated,
                    occurred_at: unix_millis(episode.last_observed_at.unix_timestamp_nanos())?,
                })?;
            }
        }

        let occurred_at = unix_millis(dispatch.emitted_at.unix_timestamp_nanos())?;
        for action in &dispatch.actions {
            persist_action(store, self.run_id, action, occurred_at)?;
        }
        Ok(())
    }

    fn persist_cleanup(&mut self, cleanup: &WatchCleanupReport) -> PersonalStateResult<()> {
        let store = &mut self.store;
        let occurred_at = unix_millis(cleanup.recorded_at.unix_timestamp_nanos())?;
        let reason = match cleanup.reason {
            WatchCleanupReason::ConnectionClosed => WatchStopReason::ConnectionClosed,
            WatchCleanupReason::HeartbeatExpired => WatchStopReason::HeartbeatTimeout,
            WatchCleanupReason::ProcessStopped => WatchStopReason::ServiceStopping,
        };

        for action in &cleanup.actions {
            let (Some(section), Some(episode_id)) = (action.section_key.clone(), action.episode_id)
            else {
                continue;
            };
            let identity = EpisodeHistoryIdentity::new(section, self.run_id, episode_id);
            if action.kind == WatchActionKind::EpisodeClosed
                && let Some(existing) = find_summary(store, &identity)?
            {
                let closed_at = occurred_at.max(existing.last_observed_at);
                store.upsert_episode_summary(&EpisodeSummaryInput {
                    identity: identity.clone(),
                    state: OpenEpisodeState::Closed,
                    mode: existing.mode,
                    first_observed_at: existing.first_observed_at,
                    last_observed_at: existing.last_observed_at,
                    acknowledged_at: existing.acknowledged_at,
                    timed_out_at: existing.timed_out_at,
                    closed_at: Some(closed_at),
                    disposition: Some(EpisodeDisposition::WatchStopped { reason }),
                    audible_count: existing.audible_count,
                    observation_count: existing.observation_count,
                    last_observation_id: existing.last_observation_id,
                })?;
            }
            persist_action(store, self.run_id, action, occurred_at)?;
        }
        Ok(())
    }
}

impl WatchDispatchSink for LocalWatchHistorySink {
    fn record_dispatch(&self, dispatch: &WatchDispatch) {
        self.enqueue(LocalWatchHistoryWork::Dispatch(dispatch.clone()));
    }

    fn record_cleanup(&self, cleanup: &WatchCleanupReport) {
        self.enqueue(LocalWatchHistoryWork::Cleanup(cleanup.clone()));
    }

    fn flush(&self) {
        self.flush_queue();
    }
}

fn project_episode_summary(
    identity: EpisodeHistoryIdentity,
    episode: &OpenEpisodeV1,
    existing: Option<&EpisodeHistorySummary>,
    closed_disposition: Option<EpisodeDisposition>,
) -> PersonalStateResult<EpisodeSummaryInput> {
    let first_observed_at = unix_millis(episode.first_observed_at.unix_timestamp_nanos())?;
    let last_observed_at = unix_millis(episode.last_observed_at.unix_timestamp_nanos())?;
    let state_changed_at = unix_millis(episode.state_changed_at.unix_timestamp_nanos())?;
    let (acknowledged_at, timed_out_at, closed_at, disposition) = match episode.state {
        OpenEpisodeState::Unacknowledged => (None, None, None, None),
        OpenEpisodeState::Acknowledged => (
            Some(state_changed_at),
            None,
            None,
            Some(EpisodeDisposition::Acknowledged),
        ),
        OpenEpisodeState::TimedOut => (
            None,
            Some(state_changed_at),
            None,
            Some(EpisodeDisposition::TimedOut),
        ),
        OpenEpisodeState::Closed => (
            existing.and_then(|summary| summary.acknowledged_at),
            existing.and_then(|summary| summary.timed_out_at),
            Some(unix_millis(
                episode
                    .closed_at
                    .ok_or(PersonalStateError::InvalidEpisodeSummary)?
                    .unix_timestamp_nanos(),
            )?),
            Some(closed_disposition.unwrap_or(EpisodeDisposition::SectionClosed)),
        ),
    };
    Ok(EpisodeSummaryInput {
        identity,
        state: episode.state,
        mode: episode.notification_mode,
        first_observed_at,
        last_observed_at,
        acknowledged_at,
        timed_out_at,
        closed_at,
        disposition,
        audible_count: episode.audible_count,
        observation_count: episode.observation_count,
        last_observation_id: episode.latest_observation_id,
    })
}

fn persist_action(
    store: &mut PersonalStateStore,
    run_id: TraceId,
    action: &WatchAction,
    occurred_at: UnixMillis,
) -> PersonalStateResult<()> {
    let Some(kind) = history_action_kind(action.kind) else {
        return Ok(());
    };
    let (Some(section), Some(episode_id)) = (action.section_key.clone(), action.episode_id) else {
        return Ok(());
    };
    let identity = EpisodeHistoryIdentity::new(section, run_id, episode_id);
    if find_summary(store, &identity)?.is_none() {
        return Ok(());
    }
    store.append_episode_action(&EpisodeActionInput {
        action_id: action.action_id,
        identity,
        kind,
        occurred_at,
    })?;
    Ok(())
}

const fn history_action_kind(kind: WatchActionKind) -> Option<EpisodeActionKind> {
    match kind {
        WatchActionKind::EpisodeOpened => Some(EpisodeActionKind::Opened),
        WatchActionKind::EpisodeAcknowledged | WatchActionKind::EpisodesAcknowledgedAll => {
            Some(EpisodeActionKind::Acknowledged)
        }
        WatchActionKind::EpisodeTimedOut => Some(EpisodeActionKind::TimedOut),
        WatchActionKind::EpisodeResumed => Some(EpisodeActionKind::Resumed),
        WatchActionKind::EpisodeClosed => Some(EpisodeActionKind::Closed),
        WatchActionKind::CueStarted => Some(EpisodeActionKind::CuePlayed),
        WatchActionKind::CueMuted => Some(EpisodeActionKind::CueMuted),
        WatchActionKind::CueAutoplayBlocked => Some(EpisodeActionKind::CueAutoplayBlocked),
        WatchActionKind::CueFailed => Some(EpisodeActionKind::CueFailed),
        WatchActionKind::AlertDismissed => Some(EpisodeActionKind::AlertDismissed),
        WatchActionKind::PolicyUpdated => Some(EpisodeActionKind::PolicyUpdated),
        WatchActionKind::WatchStarted
        | WatchActionKind::WatchRestarted
        | WatchActionKind::WatchStopped
        | WatchActionKind::AudibleCountReset => None,
    }
}

fn find_summary(
    store: &PersonalStateStore,
    identity: &EpisodeHistoryIdentity,
) -> PersonalStateResult<Option<EpisodeHistorySummary>> {
    let page = store.episode_history(
        &HistoryFilter {
            section_key: Some(identity.section_key.clone()),
            run_id: Some(identity.run_id),
            episode_id: Some(identity.episode_id),
            action_kind: None,
        },
        PageRequest {
            offset: 0,
            limit: 1,
        },
    )?;
    Ok(page.items.into_iter().next())
}

fn unix_millis(timestamp_nanos: i128) -> PersonalStateResult<UnixMillis> {
    let milliseconds = timestamp_nanos.div_euclid(1_000_000);
    let milliseconds =
        i64::try_from(milliseconds).map_err(|_| PersonalStateError::InvalidEpisodeSummary)?;
    UnixMillis::try_from(milliseconds).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::str::FromStr;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex, mpsc::RecvTimeoutError};
    use std::time::Duration;

    use bcsp_contracts::{ActiveWatchId, OpenEpisodeId, SectionKey, WatchStoppedV1};
    use rusqlite::Connection;

    use super::*;
    use crate::{LocalPrimaryDatabase, LocalRuntimePaths, OperationalGate};

    static NEXT_TEMP_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let id = NEXT_TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "rbcsp-local-watch-history-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn trace(value: u64) -> TraceId {
        TraceId::from_str(&format!("00000000-0000-4000-8000-{value:012x}")).unwrap()
    }

    fn section(index: usize) -> SectionKey {
        SectionKey::try_new("T2026F", "CAMPUS_A", &format!("{index:05}")).unwrap()
    }

    #[allow(clippy::too_many_arguments)]
    fn episode(
        identity: (u64, usize),
        state: OpenEpisodeState,
        observation_count: u64,
        latest_observation_id: TraceId,
        first_observed_at: &str,
        last_observed_at: &str,
        state_changed_at: &str,
        closed_at: Option<&str>,
    ) -> OpenEpisodeV1 {
        serde_json::from_value(serde_json::json!({
            "contractVersion": 1,
            "episodeId": OpenEpisodeId::new(trace(identity.0)),
            "activeWatchId": ActiveWatchId::new(trace(0x500 + identity.0)),
            "sectionKey": section(identity.1),
            "state": state,
            "notificationMode": "CONTINUOUS",
            "continuousDuration": {"kind": "FINITE", "seconds": 30},
            "maxAudible": 3,
            "audibleCount": 0,
            "firstObservedAt": first_observed_at,
            "lastObservedAt": last_observed_at,
            "observationCount": observation_count,
            "latestObservationId": latest_observation_id,
            "stateChangedAt": state_changed_at,
            "closedAt": closed_at,
        }))
        .unwrap()
    }

    fn action(action_id: u64, kind: WatchActionKind, episode: &OpenEpisodeV1) -> WatchAction {
        WatchAction {
            action_id: trace(action_id),
            kind,
            section_key: Some(episode.section_key.clone()),
            active_watch_id: Some(episode.active_watch_id),
            episode_id: Some(episode.episode_id),
        }
    }

    fn dispatch(
        emitted_like: &OpenEpisodeV1,
        events: Vec<WatchServerEventV1>,
        actions: Vec<WatchAction>,
    ) -> WatchDispatch {
        WatchDispatch {
            connection_id: trace(0x700),
            emitted_at: emitted_like.state_changed_at,
            events,
            actions,
        }
    }

    fn sink() -> (
        TestDirectory,
        OperationalGate,
        Arc<Mutex<LocalPrimaryDatabase>>,
        LocalWatchHistorySink,
    ) {
        let temp = TestDirectory::new();
        let package = temp.path().join("package");
        fs::create_dir_all(&package).unwrap();
        let executable = package.join("RBCSP.exe");
        fs::write(&executable, b"test").unwrap();
        let gate =
            OperationalGate::open(LocalRuntimePaths::from_executable(executable).unwrap()).unwrap();
        let database = gate.database();
        let history_store = PersonalStateStore::open(gate.paths().database()).unwrap();
        let sink = LocalWatchHistorySink::with_run_id(history_store, trace(0x900));
        (temp, gate, database, sink)
    }

    #[test]
    fn start_observations_acknowledgement_and_stop_are_durable() {
        let (_temp, _gate, database, sink) = sink();
        let opened = episode(
            (0x100, 1),
            OpenEpisodeState::Unacknowledged,
            1,
            trace(1),
            "1970-01-01T00:00:01Z",
            "1970-01-01T00:00:01Z",
            "1970-01-01T00:00:01Z",
            None,
        );
        sink.record_dispatch(&dispatch(
            &opened,
            Vec::new(),
            vec![action(0x10, WatchActionKind::WatchStarted, &opened)],
        ));
        sink.record_dispatch(&dispatch(
            &opened,
            vec![WatchServerEventV1::EpisodeUpdated {
                episode: opened.clone(),
            }],
            vec![action(0x11, WatchActionKind::EpisodeOpened, &opened)],
        ));

        let updated = episode(
            (0x100, 1),
            OpenEpisodeState::Unacknowledged,
            2,
            trace(2),
            "1970-01-01T00:00:01Z",
            "1970-01-01T00:00:02Z",
            "1970-01-01T00:00:01Z",
            None,
        );
        sink.record_dispatch(&dispatch(
            &updated,
            vec![WatchServerEventV1::EpisodeUpdated {
                episode: updated.clone(),
            }],
            Vec::new(),
        ));

        let acknowledged = episode(
            (0x100, 1),
            OpenEpisodeState::Acknowledged,
            2,
            trace(2),
            "1970-01-01T00:00:01Z",
            "1970-01-01T00:00:02Z",
            "1970-01-01T00:00:02.500Z",
            None,
        );
        sink.record_dispatch(&dispatch(
            &acknowledged,
            vec![WatchServerEventV1::EpisodeUpdated {
                episode: acknowledged.clone(),
            }],
            vec![action(
                0x12,
                WatchActionKind::EpisodeAcknowledged,
                &acknowledged,
            )],
        ));

        let closed = episode(
            (0x100, 1),
            OpenEpisodeState::Closed,
            2,
            trace(2),
            "1970-01-01T00:00:01Z",
            "1970-01-01T00:00:02Z",
            "1970-01-01T00:00:03Z",
            Some("1970-01-01T00:00:03Z"),
        );
        sink.record_dispatch(&dispatch(
            &closed,
            vec![
                WatchServerEventV1::EpisodeUpdated {
                    episode: closed.clone(),
                },
                WatchServerEventV1::WatchStopped {
                    stopped: WatchStoppedV1 {
                        contract_version: bcsp_contracts::WATCH_CONTRACT_VERSION,
                        active_watch_id: closed.active_watch_id,
                        section_key: closed.section_key.clone(),
                        reason: WatchStopReason::UserRequested,
                        stopped_at: closed.state_changed_at,
                    },
                },
            ],
            vec![action(0x13, WatchActionKind::EpisodeClosed, &closed)],
        ));
        sink.flush_queue();

        let database = database.lock().unwrap();
        let page = database
            .personal()
            .episode_history(&HistoryFilter::default(), PageRequest::DEFAULT)
            .unwrap();
        assert_eq!(page.total, 1);
        let summary = &page.items[0];
        assert_eq!(summary.state, OpenEpisodeState::Closed);
        assert_eq!(summary.observation_count.get(), 2);
        assert_eq!(summary.acknowledged_at.unwrap().get(), 2_500);
        assert_eq!(
            summary.disposition,
            Some(EpisodeDisposition::WatchStopped {
                reason: WatchStopReason::UserRequested
            })
        );
        let actions = database
            .personal()
            .episode_actions(&HistoryFilter::default(), PageRequest::DEFAULT)
            .unwrap();
        assert_eq!(actions.total, 4);
        for expected in [
            EpisodeActionKind::Opened,
            EpisodeActionKind::Updated,
            EpisodeActionKind::Acknowledged,
            EpisodeActionKind::Closed,
        ] {
            assert!(actions.items.iter().any(|action| action.kind == expected));
        }
    }

    #[test]
    fn timeout_and_connection_cleanup_close_the_existing_episode() {
        let (_temp, _gate, database, sink) = sink();
        let opened = episode(
            (0x200, 2),
            OpenEpisodeState::Unacknowledged,
            1,
            trace(20),
            "1970-01-01T00:00:01Z",
            "1970-01-01T00:00:01Z",
            "1970-01-01T00:00:01Z",
            None,
        );
        sink.record_dispatch(&dispatch(
            &opened,
            vec![WatchServerEventV1::EpisodeUpdated {
                episode: opened.clone(),
            }],
            vec![action(0x21, WatchActionKind::EpisodeOpened, &opened)],
        ));
        let timed_out = episode(
            (0x200, 2),
            OpenEpisodeState::TimedOut,
            1,
            trace(20),
            "1970-01-01T00:00:01Z",
            "1970-01-01T00:00:01Z",
            "1970-01-01T00:00:02Z",
            None,
        );
        sink.record_dispatch(&dispatch(
            &timed_out,
            vec![WatchServerEventV1::EpisodeUpdated {
                episode: timed_out.clone(),
            }],
            vec![action(0x22, WatchActionKind::EpisodeTimedOut, &timed_out)],
        ));
        sink.record_cleanup(&WatchCleanupReport {
            connection_id: trace(0x700),
            reason: WatchCleanupReason::ConnectionClosed,
            recorded_at: timed_out.state_changed_at + std::time::Duration::from_secs(1),
            sections: vec![timed_out.section_key.clone()],
            actions: vec![action(0x23, WatchActionKind::EpisodeClosed, &timed_out)],
        });
        sink.flush_queue();

        let database = database.lock().unwrap();
        let page = database
            .personal()
            .episode_history(&HistoryFilter::default(), PageRequest::DEFAULT)
            .unwrap();
        assert_eq!(page.total, 1);
        let summary = &page.items[0];
        assert_eq!(summary.state, OpenEpisodeState::Closed);
        assert_eq!(summary.timed_out_at.unwrap().get(), 2_000);
        assert_eq!(summary.closed_at.unwrap().get(), 3_000);
        assert_eq!(
            summary.disposition,
            Some(EpisodeDisposition::WatchStopped {
                reason: WatchStopReason::ConnectionClosed
            })
        );
        assert_eq!(summary.action_count, 3);
    }

    #[test]
    fn stop_history_enqueue_does_not_wait_for_sqlite_writer_and_flush_preserves_fifo() {
        let (_temp, gate, database, sink) = sink();
        let sink = Arc::new(sink);
        let opened = episode(
            (0x300, 3),
            OpenEpisodeState::Unacknowledged,
            1,
            trace(30),
            "1970-01-01T00:00:01Z",
            "1970-01-01T00:00:01Z",
            "1970-01-01T00:00:01Z",
            None,
        );
        sink.record_dispatch(&dispatch(
            &opened,
            vec![WatchServerEventV1::EpisodeUpdated {
                episode: opened.clone(),
            }],
            vec![action(0x31, WatchActionKind::EpisodeOpened, &opened)],
        ));
        sink.flush_queue();

        let blocker = Connection::open(gate.paths().database()).unwrap();
        blocker.execute_batch("BEGIN IMMEDIATE").unwrap();

        let updated = episode(
            (0x300, 3),
            OpenEpisodeState::Unacknowledged,
            2,
            trace(32),
            "1970-01-01T00:00:01Z",
            "1970-01-01T00:00:02Z",
            "1970-01-01T00:00:01Z",
            None,
        );
        let closed = episode(
            (0x300, 3),
            OpenEpisodeState::Closed,
            2,
            trace(32),
            "1970-01-01T00:00:01Z",
            "1970-01-01T00:00:02Z",
            "1970-01-01T00:00:03Z",
            Some("1970-01-01T00:00:03Z"),
        );
        let (enqueued, observe_enqueue) = mpsc::sync_channel(0);
        let enqueue_sink = sink.clone();
        let enqueue = thread::spawn(move || {
            enqueue_sink.record_dispatch(&dispatch(
                &updated,
                vec![WatchServerEventV1::EpisodeUpdated {
                    episode: updated.clone(),
                }],
                Vec::new(),
            ));
            enqueue_sink.record_dispatch(&dispatch(
                &closed,
                vec![
                    WatchServerEventV1::EpisodeUpdated {
                        episode: closed.clone(),
                    },
                    WatchServerEventV1::WatchStopped {
                        stopped: WatchStoppedV1 {
                            contract_version: bcsp_contracts::WATCH_CONTRACT_VERSION,
                            active_watch_id: closed.active_watch_id,
                            section_key: closed.section_key.clone(),
                            reason: WatchStopReason::UserRequested,
                            stopped_at: closed.state_changed_at,
                        },
                    },
                ],
                vec![action(0x33, WatchActionKind::EpisodeClosed, &closed)],
            ));
            let _ = enqueued.send(());
        });
        let enqueue_returned = observe_enqueue.recv_timeout(Duration::from_secs(1)).is_ok();
        enqueue.join().unwrap();
        thread::sleep(Duration::from_millis(100));
        let serving_database_remained_available = database.try_lock().is_ok();

        let (flushed, observe_flush) = mpsc::sync_channel(0);
        let flush_sink = sink.clone();
        let flush = thread::spawn(move || {
            flush_sink.flush_queue();
            let _ = flushed.send(());
        });
        let flush_waited_for_writer = matches!(
            observe_flush.recv_timeout(Duration::from_millis(100)),
            Err(RecvTimeoutError::Timeout)
        );

        blocker.execute_batch("ROLLBACK").unwrap();
        let flush_completed = observe_flush.recv_timeout(Duration::from_secs(5)).is_ok();
        flush.join().unwrap();

        assert!(
            enqueue_returned,
            "record_dispatch waited for the SQLite writer"
        );
        assert!(
            serving_database_remained_available,
            "watch history persistence held the serving database mutex while SQLite was busy"
        );
        assert!(
            flush_waited_for_writer,
            "flush did not wait for queued persistence"
        );
        assert!(
            flush_completed,
            "flush did not complete after the writer lock was released"
        );

        let database = database.lock().unwrap();
        let page = database
            .personal()
            .episode_history(&HistoryFilter::default(), PageRequest::DEFAULT)
            .unwrap();
        assert_eq!(page.total, 1);
        let summary = &page.items[0];
        assert_eq!(summary.state, OpenEpisodeState::Closed);
        assert_eq!(summary.observation_count.get(), 2);
        assert_eq!(summary.action_count, 3);
        assert_eq!(
            summary.disposition,
            Some(EpisodeDisposition::WatchStopped {
                reason: WatchStopReason::UserRequested
            })
        );
    }
}
