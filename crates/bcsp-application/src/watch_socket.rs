use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use bcsp_contracts::{
    OpenObservationV1, SectionKey, SystemTraceIdSource, TraceId, TraceIdSource,
    WatchClientCommandV1, WatchServerEventV1, WatchStopReason, WsClientEnvelope, WsServerEnvelope,
    decode_versioned_envelope_json,
};
use bcsp_watch::{
    WatchCleanupReport, WatchClock, WatchDispatch, WatchInstant, WatchManager, WatchManagerError,
    WatchStartAdmission,
};
use time::OffsetDateTime;
use tokio::sync::mpsc;

use crate::WebSocketExtension;

const DEFAULT_HEARTBEAT_TIMEOUT: Duration = Duration::from_secs(60);

/// Application-level heartbeat cadence: one server PING **text frame** per
/// connection per interval. A text frame (not a WS control Ping) because the
/// page must observe liveness from its message handler even in a throttled
/// background tab, and browsers surface no event for control-frame pongs.
///
/// The direction that matters is server-to-page: the PING lets the PAGE prove
/// the server is still delivering. The returning ACK is an ordinary inbound
/// frame -- it refreshes the manager heartbeat exactly like any command, and
/// the server does not require it, so a frozen page whose transport still
/// auto-pongs is not detected here. Detecting that is the page's job (the
/// staleness bound in the Readiness chain), by design.
pub const WATCH_APP_PING_INTERVAL: Duration = Duration::from_secs(10);

/// Supplies the Catalog/storage admission decision and optional current Open observation for START.
///
/// The transport deliberately does not infer admission from browser selection. Both composition
/// roots must inject their authoritative shared storage view here.
pub trait WatchAdmissionSource: Send + Sync + 'static {
    fn admission_for(&self, section: &SectionKey) -> WatchStartAdmission;

    fn target_supported(&self, _section: &SectionKey) -> bool {
        true
    }

    fn term_in_range(&self, _section: &SectionKey) -> bool {
        true
    }
}

impl<F> WatchAdmissionSource for F
where
    F: Fn(&SectionKey) -> WatchStartAdmission + Send + Sync + 'static,
{
    fn admission_for(&self, section: &SectionKey) -> WatchStartAdmission {
        self(section)
    }
}

/// Receives the product actions emitted by the shared watch reducer.
///
/// Local runtime implementations can project these actions into persistent history. Public
/// runtime implementations may intentionally use [`NoopWatchDispatchSink`]. Replayed START
/// results and replayed Open observations are not delivered to this sink a second time.
pub trait WatchDispatchSink: Send + Sync + 'static {
    fn record_dispatch(&self, dispatch: &WatchDispatch);

    fn record_cleanup(&self, cleanup: &WatchCleanupReport);

    /// Waits until all records accepted before this call have been processed.
    ///
    /// Most sinks are synchronous and need no extra work. Local durable sinks can
    /// override this barrier after moving storage writes off the watch transport
    /// path.
    fn flush(&self) {}
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NoopWatchDispatchSink;

impl WatchDispatchSink for NoopWatchDispatchSink {
    fn record_dispatch(&self, _dispatch: &WatchDispatch) {}

    fn record_cleanup(&self, _cleanup: &WatchCleanupReport) {}
}

/// Production monotonic/wall clock pair used by the shared watch reducer.
#[derive(Clone, Debug)]
pub struct SystemWatchClock {
    started: Instant,
}

impl Default for SystemWatchClock {
    fn default() -> Self {
        Self {
            started: Instant::now(),
        }
    }
}

impl WatchClock for SystemWatchClock {
    fn now(&mut self) -> WatchInstant {
        let milliseconds = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        WatchInstant::from_milliseconds(milliseconds)
    }

    fn wall_now(&mut self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

struct ConnectionChannel {
    sender: mpsc::UnboundedSender<String>,
    last_ping_at: WatchInstant,
    ping_sequence: u64,
}

struct SocketState<C, I, E> {
    manager: WatchManager<C, I>,
    connections: BTreeMap<TraceId, ConnectionChannel>,
    envelope_ids: E,
    sealed: bool,
}

/// Typed WebSocket adapter around the one shared [`WatchManager`].
///
/// This layer owns transport connection senders, not a second watch state machine. Active state is
/// memory-only and scoped to a WebSocket connection. The Open scheduler calls [`Self::publish`]
/// after committing a valid observation.
pub struct SharedWatchSocket<C = SystemWatchClock, I = SystemTraceIdSource, E = SystemTraceIdSource>
{
    state: Mutex<SocketState<C, I, E>>,
    admission: Arc<dyn WatchAdmissionSource>,
    sink: Arc<dyn WatchDispatchSink>,
}

impl SharedWatchSocket {
    pub fn try_new(
        admission: Arc<dyn WatchAdmissionSource>,
        sink: Arc<dyn WatchDispatchSink>,
    ) -> Result<Self, WatchManagerError> {
        Self::try_new_with_parts(
            SystemWatchClock::default(),
            SystemTraceIdSource,
            SystemTraceIdSource,
            DEFAULT_HEARTBEAT_TIMEOUT,
            admission,
            sink,
        )
    }
}

impl<C, I, E> SharedWatchSocket<C, I, E>
where
    C: WatchClock,
    I: TraceIdSource,
{
    pub fn try_new_with_parts(
        clock: C,
        manager_ids: I,
        envelope_ids: E,
        heartbeat_timeout: Duration,
        admission: Arc<dyn WatchAdmissionSource>,
        sink: Arc<dyn WatchDispatchSink>,
    ) -> Result<Self, WatchManagerError> {
        Ok(Self {
            state: Mutex::new(SocketState {
                manager: WatchManager::try_new(clock, manager_ids, heartbeat_timeout)?,
                connections: BTreeMap::new(),
                envelope_ids,
                sealed: false,
            }),
            admission,
            sink,
        })
    }

    pub fn connection_count(&self) -> usize {
        self.lock_state()
            .map_or(0, |state| state.manager.connection_count())
    }

    pub fn connection_watches(&self, connection_id: TraceId) -> Vec<SectionKey> {
        self.lock_state().map_or_else(Vec::new, |state| {
            state.manager.connection_watches(connection_id)
        })
    }

    /// Returns the number of active connection-to-Section watch entries across all sockets.
    pub fn total_active_watch_count(&self) -> usize {
        self.lock_state().map_or(0, |state| {
            state
                .connections
                .keys()
                .fold(0usize, |total, connection_id| {
                    total.saturating_add(state.manager.connection_watches(*connection_id).len())
                })
        })
    }

    /// Returns authoritative shared watch demand for one Open target.
    /// Multiple browser connections watching the same Section contribute one
    /// upstream demand item rather than one request per connection.
    pub fn active_watch_count(&self, target: &bcsp_contracts::TermCampusKey) -> u64 {
        self.lock_state()
            .map_or(0, |state| state.manager.active_watch_count(target))
    }

    /// Returns the deduplicated Section demand sampled by the Open scheduler
    /// for one target.
    pub fn watched_sections(&self, target: &bcsp_contracts::TermCampusKey) -> Vec<SectionKey> {
        self.lock_state()
            .map_or_else(Vec::new, |state| state.manager.watched_sections(target))
    }

    /// Fans out one committed, valid Open observation through the shared reducer.
    ///
    /// Returns `true` when the observation was an exact replay and therefore emitted no new
    /// product effects.
    pub fn publish(&self, observation: OpenObservationV1) -> Result<bool, WatchManagerError>
    where
        E: TraceIdSource,
    {
        let outcome = {
            let mut state = self
                .lock_state()
                .ok_or(WatchManagerError::InvalidContractProjection)?;
            state.manager.publish(observation)?
        };
        for dispatch in outcome.dispatches {
            self.deliver(dispatch, None, !outcome.replayed);
        }
        Ok(outcome.replayed)
    }

    /// Advances heartbeat and episode timers, fans out resulting events, and removes expired
    /// connections. The host calls this from one shared maintenance ticker.
    pub fn tick(&self)
    where
        E: TraceIdSource,
    {
        let (forced, outcome, pings) = match self.lock_state() {
            Some(mut state) => {
                let candidates = state
                    .connections
                    .keys()
                    .copied()
                    .flat_map(|connection_id| {
                        state
                            .manager
                            .connection_watches(connection_id)
                            .into_iter()
                            .map(move |section| (connection_id, section))
                    })
                    .filter_map(|(connection_id, section)| {
                        let reason = if !self.admission.target_supported(&section) {
                            Some(WatchStopReason::UnsupportedTarget)
                        } else if !self.admission.term_in_range(&section) {
                            Some(WatchStopReason::TermOutOfRange)
                        } else {
                            None
                        };
                        reason.map(|reason| (connection_id, section, reason))
                    })
                    .collect::<Vec<_>>();
                let forced = candidates
                    .into_iter()
                    .filter_map(|(connection_id, section, reason)| {
                        state
                            .manager
                            .stop_section_with_reason(connection_id, &section, reason)
                            .ok()
                    })
                    .collect::<Vec<_>>();
                match state.manager.tick() {
                    Ok(outcome) => {
                        for report in &outcome.expired_connections {
                            state.connections.remove(&report.connection_id);
                        }
                        let pings = Self::due_pings(&mut state);
                        (forced, outcome, pings)
                    }
                    Err(error) => {
                        tracing::error!(?error, "shared watch timer tick failed");
                        return;
                    }
                }
            }
            None => return,
        };
        for dispatch in forced {
            self.deliver(dispatch, None, true);
        }
        for dispatch in outcome.dispatches {
            self.deliver(dispatch, None, true);
        }
        for (sender, frame) in pings {
            let _ = sender.send(frame);
        }
        for report in outcome.expired_connections {
            self.sink.record_cleanup(&report);
        }
    }

    /// Serializes one application-level PING frame for every connection whose
    /// last PING is at least [`WATCH_APP_PING_INTERVAL`] old. Sequences are
    /// per-connection starting at 1; envelope IDs are fresh per frame, so
    /// client-side envelope dedup never coalesces two heartbeats.
    fn due_pings(state: &mut SocketState<C, I, E>) -> Vec<(mpsc::UnboundedSender<String>, String)>
    where
        E: TraceIdSource,
    {
        let SocketState {
            manager,
            connections,
            envelope_ids,
            ..
        } = state;
        let now = manager.clock_mut().now();
        let mut pings = Vec::new();
        for channel in connections.values_mut() {
            if now.elapsed_since(channel.last_ping_at) < WATCH_APP_PING_INTERVAL {
                continue;
            }
            channel.ping_sequence = channel.ping_sequence.saturating_add(1);
            channel.last_ping_at = now;
            let envelope = WsServerEnvelope::new(
                envelope_ids.next_trace_id(),
                WatchServerEventV1::Ping {
                    sequence: channel.ping_sequence,
                },
            );
            match serde_json::to_string(&envelope) {
                Ok(frame) => pings.push((channel.sender.clone(), frame)),
                Err(_) => tracing::error!("failed to serialize a watch heartbeat PING"),
            }
        }
        pings
    }

    /// Clears active connection state and records cleanup actions without persisting watches.
    /// The adapter remains available for a later connection, as required after a user-data reset.
    pub fn stop(&self) {
        self.stop_connections(false);
    }

    /// Seals this process-local adapter for runtime shutdown and clears active connection state.
    /// Later transport reconnects are rejected.
    pub fn seal_and_stop(&self) {
        self.stop_connections(true);
    }

    fn stop_connections(&self, seal: bool) {
        let reports = match self.lock_state() {
            Some(mut state) => {
                state.sealed |= seal;
                let reports = state.manager.process_stop();
                state.connections.clear();
                reports
            }
            None => return,
        };
        for report in reports {
            self.sink.record_cleanup(&report);
        }
    }

    /// Flushes the configured dispatch sink without changing in-memory watch state.
    pub fn flush_dispatch_sink(&self) {
        self.sink.flush();
    }

    fn lock_state(&self) -> Option<MutexGuard<'_, SocketState<C, I, E>>> {
        match self.state.lock() {
            Ok(state) => Some(state),
            Err(_) => {
                tracing::error!("shared watch WebSocket state lock is poisoned");
                None
            }
        }
    }

    fn route_command(
        &self,
        connection_id: TraceId,
        message_id: TraceId,
        command: WatchClientCommandV1,
    ) -> Result<(WatchDispatch, bool), WatchManagerError> {
        let mut state = self
            .lock_state()
            .ok_or(WatchManagerError::InvalidContractProjection)?;
        state.manager.touch(connection_id)?;
        match command {
            WatchClientCommandV1::StartWatch { items } => {
                let outcome = state.manager.start_with_admission(
                    connection_id,
                    message_id,
                    items,
                    |section| self.admission.admission_for(section),
                )?;
                Ok((outcome.dispatch, !outcome.replayed))
            }
            WatchClientCommandV1::StopWatch { watch } => {
                Ok((state.manager.stop_watch(connection_id, watch)?, true))
            }
            WatchClientCommandV1::UpdatePolicy { watch, policy } => Ok((
                state.manager.update_policy(connection_id, watch, policy)?,
                true,
            )),
            WatchClientCommandV1::AcknowledgeEpisode { episode } => Ok((
                state.manager.acknowledge_episode(connection_id, episode)?,
                true,
            )),
            WatchClientCommandV1::AcknowledgeAllEpisodes {} => {
                Ok((state.manager.acknowledge_all_episodes(connection_id)?, true))
            }
            WatchClientCommandV1::ResumeTimedOutEpisode { episode } => Ok((
                state
                    .manager
                    .resume_timed_out_episode(connection_id, episode)?,
                true,
            )),
            WatchClientCommandV1::ResetAudibleCount { watch } => Ok((
                state.manager.reset_audible_count(connection_id, watch)?,
                true,
            )),
            WatchClientCommandV1::ReportCueOutcome { report } => Ok((
                state.manager.report_cue_outcome(connection_id, report)?,
                true,
            )),
            WatchClientCommandV1::DismissAlert { alert } => {
                Ok((state.manager.dismiss_alert(connection_id, alert)?, true))
            }
            WatchClientCommandV1::HeartbeatAck { sequence: _ } => {
                // The pre-dispatch touch above is the entire effect: an ACK
                // refreshes the manager heartbeat and must never create
                // product events, history actions, or an outbound reply.
                let emitted_at = state.manager.clock_mut().wall_now();
                Ok((
                    WatchDispatch {
                        connection_id,
                        emitted_at,
                        events: Vec::new(),
                        actions: Vec::new(),
                    },
                    false,
                ))
            }
        }
    }

    fn deliver(&self, dispatch: WatchDispatch, message_id: Option<TraceId>, record: bool)
    where
        E: TraceIdSource,
    {
        let (sender, frames) = match self.lock_state() {
            Some(mut state) => {
                let sender = state
                    .connections
                    .get(&dispatch.connection_id)
                    .map(|channel| channel.sender.clone());
                let frames = dispatch
                    .events
                    .iter()
                    .map(|event| {
                        let envelope_id =
                            message_id.unwrap_or_else(|| state.envelope_ids.next_trace_id());
                        serde_json::to_string(&WsServerEnvelope::new(envelope_id, event.clone()))
                    })
                    .collect::<Result<Vec<_>, _>>();
                (sender, frames)
            }
            None => return,
        };

        match frames {
            Ok(frames) => {
                if let Some(sender) = sender {
                    for frame in frames {
                        if sender.send(frame).is_err() {
                            break;
                        }
                    }
                }
            }
            Err(_) => tracing::error!("failed to serialize a typed watch server event"),
        }
        if record {
            self.sink.record_dispatch(&dispatch);
        }
    }
}

impl<C, I, E> WebSocketExtension for SharedWatchSocket<C, I, E>
where
    C: WatchClock + Send + 'static,
    I: TraceIdSource + Send + 'static,
    E: TraceIdSource + Send + 'static,
{
    fn connect(&self, connection_id: TraceId, outbound: mpsc::UnboundedSender<String>) -> bool {
        let Some(mut state) = self.lock_state() else {
            return false;
        };
        if state.sealed {
            return false;
        }
        if let Err(error) = state.manager.connect(connection_id) {
            tracing::warn!(?error, "rejected watch WebSocket connection");
            return false;
        }
        let now = state.manager.clock_mut().now();
        state.connections.insert(
            connection_id,
            ConnectionChannel {
                sender: outbound,
                last_ping_at: now,
                ping_sequence: 0,
            },
        );
        true
    }

    fn transport_activity(&self, connection_id: TraceId) {
        let Some(mut state) = self.lock_state() else {
            return;
        };
        if let Err(error) = state.manager.touch(connection_id)
            && error != WatchManagerError::UnknownConnection
        {
            tracing::warn!(
                ?error,
                "failed to record watch WebSocket transport activity"
            );
        }
    }

    fn receive_text(&self, connection_id: TraceId, message: &str) {
        let envelope = match decode_versioned_envelope_json::<WsClientEnvelope<WatchClientCommandV1>>(
            message.as_bytes(),
        ) {
            Ok(envelope) => envelope,
            Err(error) => {
                tracing::warn!(?error, "rejected malformed watch WebSocket frame");
                return;
            }
        };
        let message_id = envelope.message_id();
        match self.route_command(connection_id, message_id, envelope.into_payload()) {
            Ok((dispatch, record)) => self.deliver(dispatch, Some(message_id), record),
            Err(error) => tracing::warn!(?error, "rejected invalid watch WebSocket command"),
        }
    }

    fn disconnect(&self, connection_id: TraceId) {
        let report = match self.lock_state() {
            Some(mut state) => {
                state.connections.remove(&connection_id);
                match state.manager.disconnect(connection_id) {
                    Ok(report) => report,
                    Err(WatchManagerError::UnknownConnection) => return,
                    Err(error) => {
                        tracing::warn!(?error, "failed to clean up watch WebSocket connection");
                        return;
                    }
                }
            }
            None => return,
        };
        self.sink.record_cleanup(&report);
    }

    fn tick(&self) {
        SharedWatchSocket::tick(self);
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

    use bcsp_contracts::{
        ActiveWatchTargetV1, OpenEpisodeState, OpenObservationV1, ProtocolVersion,
        WS_PROTOCOL_VERSION, WatchContinuousDurationV1, WatchNotificationMode, WatchPolicyV1,
        WatchServerEventV1, WatchStartItemResultV1, WatchStartItemV1, WatchStartItemsV1,
        WatchStartRejectionReason,
    };
    use bcsp_watch::{WatchActionKind, WatchCleanupReason};

    use super::*;

    #[derive(Clone, Default)]
    struct FakeClock {
        milliseconds: Arc<AtomicU64>,
    }

    impl FakeClock {
        fn advance(&self, duration: Duration) {
            let milliseconds = u64::try_from(duration.as_millis()).unwrap_or(u64::MAX);
            self.milliseconds.fetch_add(milliseconds, Ordering::SeqCst);
        }
    }

    impl WatchClock for FakeClock {
        fn now(&mut self) -> WatchInstant {
            WatchInstant::from_milliseconds(self.milliseconds.load(Ordering::SeqCst))
        }

        fn wall_now(&mut self) -> OffsetDateTime {
            let milliseconds =
                i64::try_from(self.milliseconds.load(Ordering::SeqCst)).unwrap_or(i64::MAX);
            OffsetDateTime::UNIX_EPOCH + time::Duration::milliseconds(milliseconds)
        }
    }

    struct FakeIds(u64);

    impl TraceIdSource for FakeIds {
        fn next_trace_id(&mut self) -> TraceId {
            self.0 += 1;
            trace(self.0)
        }
    }

    #[derive(Default)]
    struct RecordingSink {
        dispatches: Mutex<Vec<WatchDispatch>>,
        cleanups: Mutex<Vec<WatchCleanupReport>>,
    }

    impl WatchDispatchSink for RecordingSink {
        fn record_dispatch(&self, dispatch: &WatchDispatch) {
            self.dispatches.lock().unwrap().push(dispatch.clone());
        }

        fn record_cleanup(&self, cleanup: &WatchCleanupReport) {
            self.cleanups.lock().unwrap().push(cleanup.clone());
        }
    }

    struct MutableRangeAdmission {
        in_range: Arc<AtomicBool>,
    }

    impl WatchAdmissionSource for MutableRangeAdmission {
        fn admission_for(&self, _section: &SectionKey) -> WatchStartAdmission {
            WatchStartAdmission::admitted(None)
        }

        fn term_in_range(&self, _section: &SectionKey) -> bool {
            self.in_range.load(Ordering::SeqCst)
        }
    }

    struct MutableSupportAdmission {
        supported: Arc<AtomicBool>,
    }

    impl WatchAdmissionSource for MutableSupportAdmission {
        fn admission_for(&self, _section: &SectionKey) -> WatchStartAdmission {
            WatchStartAdmission::admitted(None)
        }

        fn target_supported(&self, _section: &SectionKey) -> bool {
            self.supported.load(Ordering::SeqCst)
        }
    }

    fn trace(value: u64) -> TraceId {
        TraceId::from_str(&format!("00000000-0000-4000-8000-{value:012x}"))
            .expect("synthetic trace ID")
    }

    fn section(index: usize) -> SectionKey {
        SectionKey::try_new("T2026F", "CAMPUS_A", &format!("{index:05}"))
            .expect("synthetic Section identity")
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

    fn open_observation() -> OpenObservationV1 {
        serde_json::from_value(serde_json::json!({
            "contractVersion": 1,
            "observationId": "00000000-0000-4000-8000-000000000001",
            "refreshObservationId": "00000000-0000-4000-8000-000000000001",
            "batch": {
                "term": "T2026F",
                "campus": "CAMPUS_A"
            },
            "sectionKey": {
                "term": "T2026F",
                "campus": "CAMPUS_A",
                "index": "00001"
            },
            "pullSequence": 7,
            "catalogContentVersion": 3,
            "state": "OPEN",
            "observedAt": "1970-01-01T00:00:01Z",
            "freshUntil": "1970-01-01T00:00:36Z",
            "schedulerLagMilliseconds": 250,
            "counterSnapshot": {
                "runCounts": {
                    "attempted": 9,
                    "succeeded": 7,
                    "failed": 2,
                    "empty": 1
                },
                "todayCounts": {
                    "attempted": 9,
                    "succeeded": 7,
                    "failed": 2,
                    "empty": 1
                },
                "rutgersDay": "2026-07-14",
                "dayTimezone": "America/New_York"
            }
        }))
        .expect("valid synthetic Open observation")
    }

    fn socket(
        admission: Arc<dyn WatchAdmissionSource>,
        sink: Arc<dyn WatchDispatchSink>,
    ) -> SharedWatchSocket<FakeClock, FakeIds, FakeIds> {
        socket_with_clock(admission, sink).0
    }

    fn socket_with_clock(
        admission: Arc<dyn WatchAdmissionSource>,
        sink: Arc<dyn WatchDispatchSink>,
    ) -> (SharedWatchSocket<FakeClock, FakeIds, FakeIds>, FakeClock) {
        let clock = FakeClock::default();
        SharedWatchSocket::try_new_with_parts(
            clock.clone(),
            FakeIds(0x100),
            FakeIds(0x900),
            Duration::from_secs(60),
            admission,
            sink,
        )
        .map(|socket| (socket, clock))
        .unwrap()
    }

    fn send(
        socket: &impl WebSocketExtension,
        connection_id: TraceId,
        message_id: TraceId,
        command: WatchClientCommandV1,
    ) {
        let frame = serde_json::to_string(&WsClientEnvelope::new(message_id, command)).unwrap();
        socket.receive_text(connection_id, &frame);
    }

    fn receive(
        receiver: &mut mpsc::UnboundedReceiver<String>,
    ) -> WsServerEnvelope<WatchServerEventV1> {
        serde_json::from_str(&receiver.try_recv().expect("outbound watch frame")).unwrap()
    }

    #[test]
    fn adapter_uses_authoritative_admission_and_shared_nine_watch_limit() {
        let missing = section(1);
        let missing_for_admission = missing.clone();
        let sink = Arc::new(RecordingSink::default());
        let socket = socket(
            Arc::new(move |candidate: &SectionKey| {
                if candidate == &missing_for_admission {
                    WatchStartAdmission::SectionNotFound
                } else {
                    WatchStartAdmission::admitted(None)
                }
            }),
            sink.clone(),
        );
        let connection_id = trace(1);
        let message_id = trace(2);
        let (outbound, mut receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(connection_id, outbound));

        send(
            &socket,
            connection_id,
            message_id,
            WatchClientCommandV1::StartWatch {
                items: items((1..=11).map(section)),
            },
        );

        let envelope = receive(&mut receiver);
        assert_eq!(envelope.protocol_version(), WS_PROTOCOL_VERSION);
        assert_eq!(envelope.message_id(), message_id);
        let WatchServerEventV1::StartResult { result } = envelope.into_payload() else {
            panic!("START must produce START_RESULT");
        };
        assert_eq!(result.active_watch_count(), 9);
        assert_eq!(
            result.items()[0],
            WatchStartItemResultV1::Rejected {
                section_key: missing,
                reason: WatchStartRejectionReason::SectionNotFound,
            }
        );
        assert_eq!(
            result.items()[10],
            WatchStartItemResultV1::Rejected {
                section_key: section(11),
                reason: WatchStartRejectionReason::MaxActiveWatches,
            }
        );
        assert_eq!(socket.connection_count(), 1);
        assert_eq!(socket.connection_watches(connection_id).len(), 9);
        assert_eq!(socket.total_active_watch_count(), 9);
        assert_eq!(sink.dispatches.lock().unwrap().len(), 1);

        socket.disconnect(connection_id);
        assert_eq!(socket.connection_count(), 0);
        assert_eq!(socket.total_active_watch_count(), 0);
        let cleanups = sink.cleanups.lock().unwrap();
        assert_eq!(cleanups.len(), 1);
        assert_eq!(cleanups[0].sections.len(), 9);
    }

    #[test]
    fn ordinary_stop_allows_a_new_connection_after_user_data_reset() {
        let socket = socket(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            Arc::new(NoopWatchDispatchSink),
        );
        let first_connection = trace(90);
        let second_connection = trace(91);
        let (first_outbound, mut first_receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(first_connection, first_outbound));

        socket.stop();

        assert_eq!(socket.connection_count(), 0);
        assert!(matches!(
            first_receiver.try_recv(),
            Err(mpsc::error::TryRecvError::Disconnected)
        ));
        let (second_outbound, _second_receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(second_connection, second_outbound));
        assert_eq!(socket.connection_count(), 1);

        socket.stop();
        socket.stop();
        assert_eq!(socket.connection_count(), 0);
    }

    #[test]
    fn sealed_socket_rejects_reconnects_during_runtime_shutdown() {
        let socket = socket(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            Arc::new(NoopWatchDispatchSink),
        );
        let first_connection = trace(92);
        let second_connection = trace(93);
        let (first_outbound, mut first_receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(first_connection, first_outbound));

        socket.seal_and_stop();
        socket.seal_and_stop();

        assert_eq!(socket.connection_count(), 0);
        assert!(matches!(
            first_receiver.try_recv(),
            Err(mpsc::error::TryRecvError::Disconnected)
        ));
        let (second_outbound, _second_receiver) = mpsc::unbounded_channel();
        assert!(!socket.connect(second_connection, second_outbound));
        assert_eq!(socket.connection_count(), 0);
    }

    #[test]
    fn direct_start_watch_cannot_bypass_the_term_window_admission() {
        let socket = socket(
            Arc::new(|_: &SectionKey| WatchStartAdmission::TermOutOfRange),
            Arc::new(NoopWatchDispatchSink),
        );
        let connection_id = trace(12);
        let watched = section(12);
        let (outbound, mut receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(connection_id, outbound));

        send(
            &socket,
            connection_id,
            trace(13),
            WatchClientCommandV1::StartWatch {
                items: items([watched.clone()]),
            },
        );

        let WatchServerEventV1::StartResult { result } = receive(&mut receiver).into_payload()
        else {
            panic!("START must produce START_RESULT");
        };
        assert_eq!(
            result.items(),
            &[WatchStartItemResultV1::Rejected {
                section_key: watched,
                reason: WatchStartRejectionReason::TermOutOfRange,
            }]
        );
        assert_eq!(result.active_watch_count(), 0);
        assert!(socket.connection_watches(connection_id).is_empty());
    }

    #[test]
    fn direct_start_watch_reports_an_unsupported_target() {
        let socket = socket(
            Arc::new(|_: &SectionKey| WatchStartAdmission::UnsupportedTarget),
            Arc::new(NoopWatchDispatchSink),
        );
        let connection_id = trace(14);
        let watched = section(14);
        let (outbound, mut receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(connection_id, outbound));

        send(
            &socket,
            connection_id,
            trace(15),
            WatchClientCommandV1::StartWatch {
                items: items([watched.clone()]),
            },
        );

        let WatchServerEventV1::StartResult { result } = receive(&mut receiver).into_payload()
        else {
            panic!("START must produce START_RESULT");
        };
        assert_eq!(
            result.items(),
            &[WatchStartItemResultV1::Rejected {
                section_key: watched,
                reason: WatchStartRejectionReason::UnsupportedTarget,
            }]
        );
        assert!(socket.connection_watches(connection_id).is_empty());
    }

    #[test]
    fn target_demand_deduplicates_the_same_section_across_connections() {
        let socket = socket(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            Arc::new(NoopWatchDispatchSink),
        );
        let first_connection = trace(1);
        let second_connection = trace(2);
        let (first_outbound, mut first_receiver) = mpsc::unbounded_channel();
        let (second_outbound, mut second_receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(first_connection, first_outbound));
        assert!(socket.connect(second_connection, second_outbound));
        send(
            &socket,
            first_connection,
            trace(3),
            WatchClientCommandV1::StartWatch {
                items: items([section(1)]),
            },
        );
        send(
            &socket,
            second_connection,
            trace(4),
            WatchClientCommandV1::StartWatch {
                items: items([section(1)]),
            },
        );
        let _ = receive(&mut first_receiver);
        let _ = receive(&mut second_receiver);

        assert_eq!(socket.total_active_watch_count(), 2);
        assert_eq!(socket.active_watch_count(&section(1).target()), 1);
        assert_eq!(
            socket.watched_sections(&section(1).target()),
            vec![section(1)]
        );
        socket.disconnect(first_connection);
        assert_eq!(socket.active_watch_count(&section(1).target()), 1);
        socket.disconnect(second_connection);
        assert_eq!(socket.active_watch_count(&section(1).target()), 0);
        assert!(socket.watched_sections(&section(1).target()).is_empty());
    }

    #[test]
    fn strict_client_envelopes_fail_closed_without_mutating_watch_state() {
        let sink = Arc::new(RecordingSink::default());
        let socket = socket(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            sink.clone(),
        );
        let connection_id = trace(20);
        let (outbound, mut receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(connection_id, outbound));
        let command = WatchClientCommandV1::StartWatch {
            items: items([section(20)]),
        };
        let mut unknown =
            serde_json::to_value(WsClientEnvelope::new(trace(21), command.clone())).unwrap();
        unknown
            .as_object_mut()
            .unwrap()
            .insert("unexpected".to_owned(), serde_json::json!(true));
        socket.receive_text(connection_id, &unknown.to_string());

        let mut unsupported = serde_json::to_value(WsClientEnvelope::new(trace(22), command))
            .expect("serialize client envelope");
        unsupported["protocolVersion"] = serde_json::json!(2);
        socket.receive_text(connection_id, &unsupported.to_string());

        assert!(receiver.try_recv().is_err());
        assert!(socket.connection_watches(connection_id).is_empty());
        assert!(sink.dispatches.lock().unwrap().is_empty());
        assert_eq!(WS_PROTOCOL_VERSION, ProtocolVersion::try_from(1).unwrap());
    }

    #[test]
    fn start_replay_resends_the_same_frame_without_duplicating_history_actions() {
        let sink = Arc::new(RecordingSink::default());
        let socket = socket(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            sink.clone(),
        );
        let connection_id = trace(30);
        let message_id = trace(31);
        let watched = section(30);
        let (outbound, mut receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(connection_id, outbound));
        let command = WatchClientCommandV1::StartWatch {
            items: items([watched.clone()]),
        };

        send(&socket, connection_id, message_id, command.clone());
        let first = receiver.try_recv().unwrap();
        send(&socket, connection_id, message_id, command);
        let replay = receiver.try_recv().unwrap();

        assert_eq!(replay, first);
        assert_eq!(socket.connection_watches(connection_id), vec![watched]);
        assert_eq!(sink.dispatches.lock().unwrap().len(), 1);
    }

    #[test]
    fn stop_command_and_socket_disconnect_have_distinct_product_cleanup() {
        let sink = Arc::new(RecordingSink::default());
        let socket = socket(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            sink.clone(),
        );
        let connection_id = trace(40);
        let watched = section(40);
        let (outbound, mut receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(connection_id, outbound));
        send(
            &socket,
            connection_id,
            trace(41),
            WatchClientCommandV1::StartWatch {
                items: items([watched.clone()]),
            },
        );
        let started = receive(&mut receiver);
        let WatchServerEventV1::StartResult { result } = started.into_payload() else {
            panic!("START must produce START_RESULT");
        };
        let WatchStartItemResultV1::Active {
            active_watch_id, ..
        } = result.items()[0]
        else {
            panic!("watch must be active");
        };

        send(
            &socket,
            connection_id,
            trace(42),
            WatchClientCommandV1::StopWatch {
                watch: ActiveWatchTargetV1 {
                    active_watch_id,
                    section_key: watched,
                },
            },
        );
        assert!(matches!(
            receive(&mut receiver).into_payload(),
            WatchServerEventV1::WatchStopped { .. }
        ));
        assert!(socket.connection_watches(connection_id).is_empty());
        assert_eq!(sink.dispatches.lock().unwrap().len(), 2);

        socket.disconnect(connection_id);
        assert_eq!(sink.cleanups.lock().unwrap().len(), 1);
    }

    #[test]
    fn maintenance_tick_stops_a_watch_that_rolls_out_of_the_term_window() {
        let in_range = Arc::new(AtomicBool::new(true));
        let sink = Arc::new(RecordingSink::default());
        let socket = socket(
            Arc::new(MutableRangeAdmission {
                in_range: in_range.clone(),
            }),
            sink.clone(),
        );
        let connection_id = trace(43);
        let watched = section(43);
        let (outbound, mut receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(connection_id, outbound));
        send(
            &socket,
            connection_id,
            trace(44),
            WatchClientCommandV1::StartWatch {
                items: items([watched.clone()]),
            },
        );
        assert!(matches!(
            receive(&mut receiver).into_payload(),
            WatchServerEventV1::StartResult { .. }
        ));
        assert_eq!(
            socket.connection_watches(connection_id),
            vec![watched.clone()]
        );

        in_range.store(false, Ordering::SeqCst);
        socket.tick();

        let WatchServerEventV1::WatchStopped { stopped } = receive(&mut receiver).into_payload()
        else {
            panic!("term-window rollover must emit WATCH_STOPPED");
        };
        assert_eq!(stopped.section_key, watched);
        assert_eq!(stopped.reason, WatchStopReason::TermOutOfRange);
        assert!(socket.connection_watches(connection_id).is_empty());
        assert_eq!(sink.dispatches.lock().unwrap().len(), 2);
    }

    #[test]
    fn maintenance_tick_stops_an_active_watch_when_its_campus_becomes_unsupported() {
        let supported = Arc::new(AtomicBool::new(true));
        let sink = Arc::new(RecordingSink::default());
        let socket = socket(
            Arc::new(MutableSupportAdmission {
                supported: supported.clone(),
            }),
            sink.clone(),
        );
        let connection_id = trace(45);
        let watched = section(45);
        let (outbound, mut receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(connection_id, outbound));
        send(
            &socket,
            connection_id,
            trace(46),
            WatchClientCommandV1::StartWatch {
                items: items([watched.clone()]),
            },
        );
        let _ = receive(&mut receiver);

        supported.store(false, Ordering::SeqCst);
        socket.tick();

        let WatchServerEventV1::WatchStopped { stopped } = receive(&mut receiver).into_payload()
        else {
            panic!("unsupported Campus must emit WATCH_STOPPED");
        };
        assert_eq!(stopped.section_key, watched);
        assert_eq!(stopped.reason, WatchStopReason::UnsupportedTarget);
        assert!(socket.connection_watches(connection_id).is_empty());
        assert_eq!(sink.dispatches.lock().unwrap().len(), 2);
    }

    #[test]
    fn committed_observation_is_fanned_out_as_typed_server_envelopes_once() {
        let sink = Arc::new(RecordingSink::default());
        let socket = socket(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            sink.clone(),
        );
        let observation = open_observation();
        let connection_id = trace(50);
        let (outbound, mut receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(connection_id, outbound));
        send(
            &socket,
            connection_id,
            trace(51),
            WatchClientCommandV1::StartWatch {
                items: items([observation.section_key().clone()]),
            },
        );
        let _ = receive(&mut receiver);

        assert!(!socket.publish(observation.clone()).unwrap());
        let events = std::iter::from_fn(|| receiver.try_recv().ok())
            .map(|frame| {
                serde_json::from_str::<WsServerEnvelope<WatchServerEventV1>>(&frame).unwrap()
            })
            .collect::<Vec<_>>();
        assert!(events.iter().any(|envelope| matches!(
            envelope.payload(),
            WatchServerEventV1::OpenObservation { .. }
        )));
        assert!(events.iter().all(|envelope| {
            envelope.protocol_version() == WS_PROTOCOL_VERSION && envelope.message_id() != trace(51)
        }));
        assert_eq!(sink.dispatches.lock().unwrap().len(), 2);

        assert!(socket.publish(observation).unwrap());
        assert!(receiver.try_recv().is_err());
        assert_eq!(sink.dispatches.lock().unwrap().len(), 2);
    }

    #[test]
    fn pong_activity_keeps_a_healthy_connection_alive_until_transport_really_times_out() {
        let sink = Arc::new(RecordingSink::default());
        let (socket, clock) = socket_with_clock(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            sink.clone(),
        );
        let connection_id = trace(60);
        let (outbound, mut receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(connection_id, outbound));
        send(
            &socket,
            connection_id,
            trace(61),
            WatchClientCommandV1::StartWatch {
                items: items([section(60)]),
            },
        );
        let _ = receive(&mut receiver);

        clock.advance(Duration::from_secs(59));
        socket.tick();
        assert_eq!(socket.connection_count(), 1);
        socket.transport_activity(connection_id);
        clock.advance(Duration::from_secs(59));
        socket.tick();
        assert_eq!(socket.connection_count(), 1);

        clock.advance(Duration::from_secs(1));
        socket.tick();
        assert_eq!(socket.connection_count(), 0);
        assert_eq!(socket.total_active_watch_count(), 0);
        let cleanups = sink.cleanups.lock().unwrap();
        assert_eq!(cleanups.len(), 1);
        assert_eq!(cleanups[0].reason, WatchCleanupReason::HeartbeatExpired);
        assert!(
            cleanups[0]
                .actions
                .iter()
                .any(|action| action.kind == WatchActionKind::WatchStopped)
        );
    }

    #[test]
    fn maintenance_tick_fans_out_continuous_episode_timeout_and_records_action() {
        let sink = Arc::new(RecordingSink::default());
        let (socket, clock) = socket_with_clock(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            sink.clone(),
        );
        let observation = open_observation();
        let connection_id = trace(70);
        let (outbound, mut receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(connection_id, outbound));
        let policy = WatchPolicyV1::new(
            WatchNotificationMode::Continuous,
            Default::default(),
            WatchContinuousDurationV1::finite_seconds(1).unwrap(),
        );
        let start_items = WatchStartItemsV1::try_from(vec![WatchStartItemV1::new(
            observation.section_key().clone(),
            policy,
        )])
        .unwrap();
        send(
            &socket,
            connection_id,
            trace(71),
            WatchClientCommandV1::StartWatch { items: start_items },
        );
        let _ = receive(&mut receiver);
        assert!(!socket.publish(observation).unwrap());
        while receiver.try_recv().is_ok() {}

        clock.advance(Duration::from_secs(1));
        socket.tick();
        let events = std::iter::from_fn(|| receiver.try_recv().ok())
            .map(|frame| {
                serde_json::from_str::<WsServerEnvelope<WatchServerEventV1>>(&frame).unwrap()
            })
            .collect::<Vec<_>>();
        assert!(events.iter().any(|envelope| matches!(
            envelope.payload(),
            WatchServerEventV1::EpisodeUpdated { episode }
                if episode.state == OpenEpisodeState::TimedOut
        )));
        assert!(
            sink.dispatches
                .lock()
                .unwrap()
                .last()
                .unwrap()
                .actions
                .iter()
                .any(|action| action.kind == WatchActionKind::EpisodeTimedOut)
        );
    }

    #[test]
    fn server_pings_every_ten_seconds_and_passive_acks_keep_the_connection_alive() {
        let sink = Arc::new(RecordingSink::default());
        let (socket, clock) = socket_with_clock(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            sink.clone(),
        );
        let connection_id = trace(80);
        let (outbound, mut receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(connection_id, outbound));
        send(
            &socket,
            connection_id,
            trace(81),
            WatchClientCommandV1::StartWatch {
                items: items([section(80)]),
            },
        );
        let _ = receive(&mut receiver);
        assert_eq!(sink.dispatches.lock().unwrap().len(), 1);

        socket.tick();
        assert!(
            receiver.try_recv().is_err(),
            "no PING before the interval elapses"
        );

        let mut seen_envelope_ids = std::collections::BTreeSet::new();
        // Six PING/ACK rounds cover 60s of wall time during which ONLY the
        // app-level ACK (never transport activity) refreshes the manager
        // heartbeat -- a passive background-tab page must survive this.
        for round in 1..=6u64 {
            clock.advance(WATCH_APP_PING_INTERVAL);
            socket.tick();
            socket.tick();
            let envelope = receive(&mut receiver);
            assert!(seen_envelope_ids.insert(envelope.message_id()));
            let WatchServerEventV1::Ping { sequence } = envelope.into_payload() else {
                panic!("expected an application-level PING");
            };
            assert_eq!(sequence, round);
            assert!(
                receiver.try_recv().is_err(),
                "exactly one PING per interval even across repeated ticks"
            );
            send(
                &socket,
                connection_id,
                trace(0x8100 + round),
                WatchClientCommandV1::HeartbeatAck { sequence },
            );
        }
        assert_eq!(
            socket.connection_count(),
            1,
            "60s of ACK-only activity keeps the connection alive"
        );
        assert!(
            receiver.try_recv().is_err(),
            "an ACK produces no reply frame"
        );
        assert_eq!(
            sink.dispatches.lock().unwrap().len(),
            1,
            "an ACK records no history dispatch"
        );

        clock.advance(Duration::from_secs(61));
        socket.tick();
        assert_eq!(socket.connection_count(), 0);
        {
            let cleanups = sink.cleanups.lock().unwrap();
            assert_eq!(cleanups.len(), 1);
            assert_eq!(cleanups[0].reason, WatchCleanupReason::HeartbeatExpired);
        }
        while receiver.try_recv().is_ok() {}
        clock.advance(WATCH_APP_PING_INTERVAL);
        socket.tick();
        assert!(
            receiver.try_recv().is_err(),
            "expired connections are not pinged"
        );
    }

    #[test]
    fn malformed_heartbeat_acks_fail_closed_and_refresh_nothing() {
        let sink = Arc::new(RecordingSink::default());
        let (socket, clock) = socket_with_clock(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            sink.clone(),
        );
        let connection_id = trace(90);
        let (outbound, mut receiver) = mpsc::unbounded_channel();
        assert!(socket.connect(connection_id, outbound));

        // 59s idle, then only a MALFORMED ack arrives (extra field under the
        // strict client decoder). This pins the decoder-internal ordering:
        // decode fails closed BEFORE route_command, so a malformed ACK
        // reaches neither the manager nor the sink. It is not an end-to-end
        // claim -- the production pump calls transport_activity before
        // receive_text, so on a live socket any inbound frame, malformed or
        // not, still refreshes the transport-level heartbeat.
        clock.advance(Duration::from_secs(59));
        let mut value = serde_json::to_value(WsClientEnvelope::new(
            trace(91),
            WatchClientCommandV1::HeartbeatAck { sequence: 1 },
        ))
        .unwrap();
        assert!(value["payload"].is_object());
        value["payload"]["future"] = serde_json::json!(true);
        socket.receive_text(connection_id, &value.to_string());
        assert_eq!(socket.connection_count(), 1);

        clock.advance(Duration::from_secs(1));
        socket.tick();
        assert_eq!(socket.connection_count(), 0);
        assert!(receiver.try_recv().is_err(), "no frame ever went out");
        assert!(sink.dispatches.lock().unwrap().is_empty());
    }
}
