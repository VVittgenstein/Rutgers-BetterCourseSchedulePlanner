use std::collections::BTreeMap;
use std::num::NonZeroU8;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use bcsp_contracts::{
    ActiveWatchTargetV1, OpenObservationV1, SectionKey, SystemTraceIdSource, TraceId,
    TraceIdSource, WatchClientCommandV1, WatchPolicyV1, WatchServerEventV1, WatchStartItemResultV1,
    WatchStartItemsV1, WatchStopReason, WsClientEnvelope, WsServerEnvelope,
    decode_versioned_envelope_json,
};
use bcsp_watch::{
    WatchCleanupReport, WatchClock, WatchDispatch, WatchInstant, WatchManager, WatchManagerError,
    WatchStartAdmission,
};
use time::OffsetDateTime;

use crate::{OutboundSender, WebSocketExtension};

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

    /// Resolves one start/revalidation round as a batch.
    ///
    /// The default preserves the original per-Section behavior. Admission
    /// sources backed by a target-wide projection can override this to build
    /// each target once and return decisions in the same order as `sections`.
    fn admissions_for(&self, sections: &[SectionKey]) -> Vec<WatchStartAdmission> {
        sections
            .iter()
            .map(|section| self.admission_for(section))
            .collect()
    }

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
    sender: OutboundSender,
    last_ping_at: WatchInstant,
    ping_sequence: u64,
    /// The highest PING sequence this connection has already had accepted.
    ///
    /// An acknowledgement is evidence exactly once. Replaying sequence 7
    /// says nothing new about the page that sent it, so the counter below
    /// only moves forward.
    acknowledged_sequence: u64,
}

struct SocketState<C, I, E> {
    manager: WatchManager<C, I>,
    connections: BTreeMap<TraceId, ConnectionChannel>,
    envelope_ids: E,
    sealed: bool,
    /// The synthetic connection that holds watches on the PROCESS's behalf
    /// rather than a browser's, when a target has asked for one.
    ///
    /// It is a manager connection like any other -- it obeys the physical
    /// watch cap -- with two differences that follow from having no socket:
    /// its heartbeat is refreshed by the maintenance tick rather than by
    /// inbound frames, and everything it emits is fanned out to every
    /// audience instead of unicast back to a requester that does not exist.
    owner: Option<TraceId>,
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
    /// How many client heartbeat acknowledgements this process has ACCEPTED.
    ///
    /// One aggregate number, no identity of any kind: not a session, not a
    /// nonce, not a Section, not a connection id. It exists so a soak can
    /// prove the server-side half of the heartbeat -- that a valid ACK was
    /// received, decoded, matched to a sequence this server really issued,
    /// and taken -- instead of inferring acceptance from the absence of a
    /// rejection in a log.
    accepted_heartbeat_acks: std::sync::atomic::AtomicU64,
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

    /// Creates a socket with a target-specific owner/browser watch limit.
    ///
    /// The ordinary constructor retains the shared/public limit. The local
    /// composition uses this explicit seam to opt into the full `u8` wire
    /// domain without widening the public service.
    pub fn try_new_with_max_active_watches(
        admission: Arc<dyn WatchAdmissionSource>,
        sink: Arc<dyn WatchDispatchSink>,
        max_active_watches: NonZeroU8,
    ) -> Result<Self, WatchManagerError> {
        Self::try_new_with_parts_and_max_active_watches(
            SystemWatchClock::default(),
            SystemTraceIdSource,
            SystemTraceIdSource,
            DEFAULT_HEARTBEAT_TIMEOUT,
            admission,
            sink,
            max_active_watches,
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
        Self::try_new_with_parts_and_max_active_watches(
            clock,
            manager_ids,
            envelope_ids,
            heartbeat_timeout,
            admission,
            sink,
            NonZeroU8::new(bcsp_contracts::MAX_ACTIVE_WATCHES)
                .expect("the default watch limit is nonzero"),
        )
    }

    pub fn try_new_with_parts_and_max_active_watches(
        clock: C,
        manager_ids: I,
        envelope_ids: E,
        heartbeat_timeout: Duration,
        admission: Arc<dyn WatchAdmissionSource>,
        sink: Arc<dyn WatchDispatchSink>,
        max_active_watches: NonZeroU8,
    ) -> Result<Self, WatchManagerError> {
        Ok(Self {
            state: Mutex::new(SocketState {
                manager: WatchManager::try_new_with_max_active_watches(
                    clock,
                    manager_ids,
                    heartbeat_timeout,
                    max_active_watches,
                )?,
                connections: BTreeMap::new(),
                envelope_ids,
                sealed: false,
                owner: None,
            }),
            admission,
            sink,
            accepted_heartbeat_acks: std::sync::atomic::AtomicU64::new(0),
        })
    }

    /// How many heartbeat acknowledgements this socket has accepted since the
    /// process started.
    ///
    /// Monotonic for the life of the process: a restart resets it to zero,
    /// which is why an observer comparing two readings must also prove it is
    /// still looking at the same service invocation.
    pub fn accepted_heartbeat_acks(&self) -> u64 {
        self.accepted_heartbeat_acks
            .load(std::sync::atomic::Ordering::Relaxed)
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
    ///
    /// The owner connection is counted even though it has no transport
    /// channel. A page asking how many watches are running wants the answer
    /// about the process, and under a coordinator every one of them is held
    /// by the owner -- reporting zero would be the exact kind of comfortable
    /// lie the watch surface exists to avoid.
    pub fn total_active_watch_count(&self) -> usize {
        self.lock_state().map_or(0, |state| {
            state.connections.keys().copied().chain(state.owner).fold(
                0usize,
                |total, connection_id| {
                    total.saturating_add(state.manager.connection_watches(connection_id).len())
                },
            )
        })
    }

    /// How many BROWSER connections are attached right now.
    ///
    /// This is the audience count a coordinator reconciles against, so it
    /// deliberately excludes the owner: the owner exists to serve the
    /// audience, and counting it would mean the audience is never empty and
    /// the physical watches are never torn down.
    pub fn audience_connection_count(&self) -> usize {
        self.lock_state().map_or(0, |state| state.connections.len())
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
                // The owner has no socket, so no inbound frame ever refreshes
                // it. Without this the heartbeat sweep below would expire the
                // connection holding every physical watch, and the watches
                // would vanish while the pages that wanted them stayed open.
                if let Some(owner) = state.owner {
                    let _ = state.manager.touch(owner);
                }
                // Every connection that can hold a watch, INCLUDING the
                // owner. A target whose watches all live on the owner would
                // otherwise never have its term window or campus re-checked,
                // and a section that rolled out of the window would keep
                // being polled -- and keep being reported as watched.
                let candidates = state
                    .connections
                    .keys()
                    .copied()
                    .chain(state.owner)
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
                            if state.owner == Some(report.connection_id) {
                                state.owner = None;
                            }
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
    fn due_pings(state: &mut SocketState<C, I, E>) -> Vec<(OutboundSender, String)>
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

    /// Returns the owner connection, creating it if this socket does not have
    /// one yet or has lost it to a process stop.
    ///
    /// Idempotent on purpose. `stop()` clears every manager connection, so a
    /// remembered id can outlive the connection it names; asking the manager
    /// to refresh it is the only way to tell the two cases apart.
    pub fn ensure_owner_connection(&self) -> Result<TraceId, WatchManagerError>
    where
        E: TraceIdSource,
    {
        let mut state = self
            .lock_state()
            .ok_or(WatchManagerError::InvalidContractProjection)?;
        // A sealed socket is a process shutting down. Transport reconnects
        // are already refused there; the owner is refused for the same
        // reason and with more force, because nothing would ever close it --
        // it has no socket to drop, so a watch rebuilt here would outlive
        // the runtime that was tearing itself down.
        if state.sealed {
            return Err(WatchManagerError::SocketSealed);
        }
        if let Some(owner) = state.owner
            && state.manager.touch(owner).is_ok()
        {
            return Ok(owner);
        }
        let owner = state.envelope_ids.next_trace_id();
        state.manager.connect(owner)?;
        state.owner = Some(owner);
        Ok(owner)
    }

    /// The owner connection, if one has been opened.
    pub fn owner_connection(&self) -> Option<TraceId> {
        self.lock_state().and_then(|state| state.owner)
    }

    /// The Sections the owner currently holds a physical watch on.
    pub fn owner_watched_sections(&self) -> Vec<SectionKey> {
        self.lock_state().map_or_else(Vec::new, |state| {
            state
                .owner
                .map(|owner| state.manager.connection_watches(owner))
                .unwrap_or_default()
        })
    }

    /// Every physical watch the owner holds, with the identity a caller needs
    /// to recognise -- or to stop -- exactly the watch it started.
    ///
    /// A coordinator that compared only Sections could not tell "the watch I
    /// armed is still running" from "that watch ended and something started a
    /// new one for the same Section". The second is a drift the page must be
    /// told about, and the id is the only thing that distinguishes them.
    pub fn owner_watch_targets(&self) -> Vec<ActiveWatchTargetV1> {
        self.lock_state().map_or_else(Vec::new, |state| {
            state
                .owner
                .map(|owner| state.manager.connection_watch_targets(owner))
                .unwrap_or_default()
        })
    }

    /// Asks the injected admission source whether one Section could be armed
    /// right now.
    ///
    /// Deliberately lock-free with respect to this socket: it consults the
    /// same authoritative storage view a START would, so a caller can check
    /// an ALREADY running watch without taking the socket lock and without
    /// disturbing the manager. Materialization conditions are revocable --
    /// a term rolls over, a campus stops being a product target, a catalog
    /// stops publishing a Section -- and something has to keep asking.
    pub fn admission_for(&self, section: &SectionKey) -> WatchStartAdmission {
        self.admission.admission_for(section)
    }

    /// Batched counterpart to [`Self::admission_for`]. Expensive projection
    /// work happens before the socket state lock is taken.
    pub fn admissions_for(&self, sections: &[SectionKey]) -> Vec<WatchStartAdmission> {
        self.admission.admissions_for(sections)
    }

    /// Starts watches on the owner connection and reports the per-item
    /// outcome, so the caller can tell "armed" from "the catalog does not
    /// publish this" without parsing a fanned-out event.
    pub fn owner_start(
        &self,
        items: WatchStartItemsV1,
    ) -> Result<Vec<WatchStartItemResultV1>, WatchManagerError>
    where
        E: TraceIdSource,
    {
        let admissions = self.admissions_for_items(&items)?;
        let owner = self.ensure_owner_connection()?;
        let (message_id, dispatch, record) = {
            let mut state = self
                .lock_state()
                .ok_or(WatchManagerError::InvalidContractProjection)?;
            let message_id = state.envelope_ids.next_trace_id();
            let mut admissions = admissions.into_iter();
            let outcome = state
                .manager
                .start_with_admission(owner, message_id, items, |_| {
                    admissions
                        .next()
                        .unwrap_or(WatchStartAdmission::TargetUnavailable)
                })?;
            (message_id, outcome.dispatch, !outcome.replayed)
        };
        let results = start_item_results(&dispatch);
        self.deliver(dispatch, Some(message_id), record);
        Ok(results)
    }

    /// Stops one owner-held watch. `UnknownWatch` is the caller's to
    /// interpret: for a teardown it means the goal is already met.
    pub fn owner_stop(&self, target: ActiveWatchTargetV1) -> Result<(), WatchManagerError>
    where
        E: TraceIdSource,
    {
        self.owner_dispatch(|manager, owner| manager.stop_watch(owner, target.clone()))
    }

    /// Applies a new policy to an owner-held watch IN PLACE.
    ///
    /// The distinction matters: a policy edit must not end the episode, mint
    /// a new watch id or re-announce the section, because none of that is
    /// what the user asked for when they changed how loud it should be.
    pub fn owner_update_policy(
        &self,
        target: ActiveWatchTargetV1,
        policy: WatchPolicyV1,
    ) -> Result<(), WatchManagerError>
    where
        E: TraceIdSource,
    {
        self.owner_dispatch(|manager, owner| {
            manager.update_policy(owner, target.clone(), policy.clone())
        })
    }

    /// Runs one episode-control command against the owner connection.
    ///
    /// A target whose watches live on the owner needs this: acknowledging an
    /// episode, resuming a timed-out one, resetting the audible count,
    /// reporting a cue outcome or dismissing an alert are all addressed by
    /// the identity of a watch the page does not hold. Refuses the three
    /// commands that would create or destroy a watch -- those are the
    /// coordinator's, driven by durable intent rather than by a frame.
    pub fn owner_command(&self, command: WatchClientCommandV1) -> Result<(), WatchManagerError>
    where
        E: TraceIdSource,
    {
        match command {
            WatchClientCommandV1::StartWatch { .. }
            | WatchClientCommandV1::StopWatch { .. }
            | WatchClientCommandV1::UpdatePolicy { .. }
            | WatchClientCommandV1::HeartbeatAck { .. } => Err(WatchManagerError::TargetMismatch),
            WatchClientCommandV1::AcknowledgeEpisode { episode } => {
                self.owner_dispatch(|manager, owner| {
                    manager.acknowledge_episode(owner, episode.clone())
                })
            }
            WatchClientCommandV1::AcknowledgeAllEpisodes {} => {
                self.owner_dispatch(|manager, owner| manager.acknowledge_all_episodes(owner))
            }
            WatchClientCommandV1::ResumeTimedOutEpisode { episode } => {
                self.owner_dispatch(|manager, owner| {
                    manager.resume_timed_out_episode(owner, episode.clone())
                })
            }
            WatchClientCommandV1::ResetAudibleCount { watch } => self
                .owner_dispatch(|manager, owner| manager.reset_audible_count(owner, watch.clone())),
            WatchClientCommandV1::ReportCueOutcome { report } => self
                .owner_dispatch(|manager, owner| manager.report_cue_outcome(owner, report.clone())),
            WatchClientCommandV1::DismissAlert { alert } => {
                self.owner_dispatch(|manager, owner| manager.dismiss_alert(owner, alert.clone()))
            }
        }
    }

    fn owner_dispatch<F>(&self, operation: F) -> Result<(), WatchManagerError>
    where
        E: TraceIdSource,
        F: FnOnce(&mut WatchManager<C, I>, TraceId) -> Result<WatchDispatch, WatchManagerError>,
    {
        let owner = self.ensure_owner_connection()?;
        let dispatch = {
            let mut state = self
                .lock_state()
                .ok_or(WatchManagerError::InvalidContractProjection)?;
            operation(&mut state.manager, owner)?
        };
        self.deliver(dispatch, None, true);
        Ok(())
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
                // The manager has forgotten every connection, including the
                // owner, so the remembered id now names nothing. Clearing it
                // here keeps `ensure_owner_connection` from handing a
                // coordinator an id the manager will reject.
                state.owner = None;
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
        let admissions = match &command {
            WatchClientCommandV1::StartWatch { items } => Some(self.admissions_for_items(items)?),
            _ => None,
        };
        let mut state = self
            .lock_state()
            .ok_or(WatchManagerError::InvalidContractProjection)?;
        state.manager.touch(connection_id)?;
        match command {
            WatchClientCommandV1::StartWatch { items } => {
                let mut admissions = admissions
                    .expect("START_WATCH admission was prepared")
                    .into_iter();
                let outcome =
                    state
                        .manager
                        .start_with_admission(connection_id, message_id, items, |_| {
                            admissions
                                .next()
                                .unwrap_or(WatchStartAdmission::TargetUnavailable)
                        })?;
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
            WatchClientCommandV1::HeartbeatAck { sequence } => {
                // The pre-dispatch touch above is the entire product effect:
                // an ACK refreshes the manager heartbeat and must never create
                // product events, history actions, or an outbound reply.
                //
                // It is also the one place an acknowledgement becomes
                // evidence. Reaching here already means the frame decoded as a
                // versioned envelope carrying a v1 command and named a
                // connection the manager knows. Counting it additionally
                // requires the sequence to be one this connection was really
                // sent and has not already acknowledged. Arbitrary inbound
                // text, a replay, and a sequence never issued all still
                // refresh the heartbeat exactly as before -- they simply are
                // not evidence that the heartbeat round trip works.
                if let Some(channel) = state.connections.get_mut(&connection_id)
                    && sequence >= 1
                    && sequence <= channel.ping_sequence
                    && sequence > channel.acknowledged_sequence
                {
                    channel.acknowledged_sequence = sequence;
                    self.accepted_heartbeat_acks
                        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
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

    fn admissions_for_items(
        &self,
        items: &WatchStartItemsV1,
    ) -> Result<Vec<WatchStartAdmission>, WatchManagerError> {
        let sections = items
            .as_slice()
            .iter()
            .map(|item| item.section_key.clone())
            .collect::<Vec<_>>();
        let admissions = self.admission.admissions_for(&sections);
        if admissions.len() != sections.len() {
            return Err(WatchManagerError::InvalidContractProjection);
        }
        Ok(admissions)
    }

    /// Sends one dispatch's events to whoever should see them.
    ///
    /// A dispatch produced by a browser connection goes back to that
    /// connection. A dispatch produced by the OWNER goes to every attached
    /// page instead: the watch is held for all of them, so an alert on it
    /// belongs to all of them. Two pages open therefore means two pages ring,
    /// which is the honest report -- both really are watching.
    fn deliver(&self, dispatch: WatchDispatch, message_id: Option<TraceId>, record: bool)
    where
        E: TraceIdSource,
    {
        let (senders, frames) = match self.lock_state() {
            Some(mut state) => {
                let senders = if state.owner == Some(dispatch.connection_id) {
                    state
                        .connections
                        .values()
                        .map(|channel| channel.sender.clone())
                        .collect::<Vec<_>>()
                } else {
                    state
                        .connections
                        .get(&dispatch.connection_id)
                        .map(|channel| channel.sender.clone())
                        .into_iter()
                        .collect::<Vec<_>>()
                };
                let frames = dispatch
                    .events
                    .iter()
                    .map(|event| {
                        let envelope_id =
                            message_id.unwrap_or_else(|| state.envelope_ids.next_trace_id());
                        serde_json::to_string(&WsServerEnvelope::new(envelope_id, event.clone()))
                    })
                    .collect::<Result<Vec<_>, _>>();
                (senders, frames)
            }
            None => return,
        };

        match frames {
            Ok(frames) => {
                for sender in senders {
                    for frame in &frames {
                        if sender.send(frame.clone()).is_err() {
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

/// Pulls the per-item start outcomes out of a dispatch, so a caller that
/// asked for the start learns what happened to each Section.
fn start_item_results(dispatch: &WatchDispatch) -> Vec<WatchStartItemResultV1> {
    dispatch
        .events
        .iter()
        .find_map(|event| match event {
            WatchServerEventV1::StartResult { result } => Some(result.items().to_vec()),
            _ => None,
        })
        .unwrap_or_default()
}

impl<C, I, E> WebSocketExtension for SharedWatchSocket<C, I, E>
where
    C: WatchClock + Send + 'static,
    I: TraceIdSource + Send + 'static,
    E: TraceIdSource + Send + 'static,
{
    fn connect(&self, connection_id: TraceId, outbound: OutboundSender) -> bool {
        let Some(mut state) = self.lock_state() else {
            return false;
        };
        if state.sealed {
            return false;
        }
        // The owner is constructed in Rust and holds watches nobody may stop
        // by naming its id. A transport connection that presented that id
        // would inherit them.
        if state.owner == Some(connection_id) {
            tracing::warn!("refused a transport connection claiming the owner connection id");
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
                acknowledged_sequence: 0,
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
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Barrier, Mutex};

    use bcsp_contracts::{
        ActiveWatchTargetV1, OpenEpisodeState, OpenObservationV1, ProtocolVersion,
        WS_PROTOCOL_VERSION, WatchContinuousDurationV1, WatchNotificationMode, WatchPolicyV1,
        WatchServerEventV1, WatchStartItemResultV1, WatchStartItemV1, WatchStartItemsV1,
        WatchStartRejectionReason,
    };
    use bcsp_watch::{WatchActionKind, WatchCleanupReason};
    use tokio::sync::mpsc;

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

    #[derive(Default)]
    struct CountingBatchAdmission {
        batches: AtomicUsize,
        singles: AtomicUsize,
        last_batch_len: AtomicUsize,
    }

    impl WatchAdmissionSource for CountingBatchAdmission {
        fn admission_for(&self, _section: &SectionKey) -> WatchStartAdmission {
            self.singles.fetch_add(1, Ordering::SeqCst);
            WatchStartAdmission::admitted(None)
        }

        fn admissions_for(&self, sections: &[SectionKey]) -> Vec<WatchStartAdmission> {
            self.batches.fetch_add(1, Ordering::SeqCst);
            self.last_batch_len.store(sections.len(), Ordering::SeqCst);
            vec![WatchStartAdmission::admitted(None); sections.len()]
        }
    }

    struct BlockingBatchAdmission {
        entered: Barrier,
        release: Barrier,
    }

    impl WatchAdmissionSource for BlockingBatchAdmission {
        fn admission_for(&self, _section: &SectionKey) -> WatchStartAdmission {
            WatchStartAdmission::admitted(None)
        }

        fn admissions_for(&self, sections: &[SectionKey]) -> Vec<WatchStartAdmission> {
            self.entered.wait();
            self.release.wait();
            vec![WatchStartAdmission::admitted(None); sections.len()]
        }
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

    /// A watch the process holds on everyone's behalf must reach everyone.
    ///
    /// The alternative is a lie by omission: one page would hear the alert
    /// and the others, equally attached and equally watching that section,
    /// would sit silent. Two pages ringing is the honest report.
    ///
    /// The second half is the control. A dispatch produced by an ordinary
    /// connection still goes only to that connection, so this is fan-out for
    /// the owner rather than broadcast for everything.
    #[test]
    fn owner_events_reach_every_audience_and_connection_events_reach_only_one() {
        let socket = socket(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            Arc::new(NoopWatchDispatchSink),
        );
        let first = trace(1);
        let second = trace(2);
        let (first_outbound, mut first_frames) = OutboundSender::unbounded_pair();
        let (second_outbound, mut second_frames) = OutboundSender::unbounded_pair();
        assert!(socket.connect(first, first_outbound));
        assert!(socket.connect(second, second_outbound));

        let results = socket
            .owner_start(items([section(1)]))
            .expect("owner start");
        assert!(results[0].is_active());
        assert_eq!(socket.owner_watched_sections(), vec![section(1)]);
        assert_eq!(socket.audience_connection_count(), 2);
        assert_eq!(
            socket.total_active_watch_count(),
            1,
            "the owner's watches are counted even though it has no socket",
        );
        for frames in [&mut first_frames, &mut second_frames] {
            assert!(matches!(
                receive(frames).into_payload(),
                WatchServerEventV1::StartResult { .. },
            ));
        }

        socket.publish(open_observation()).expect("publish");
        for frames in [&mut first_frames, &mut second_frames] {
            let mut seen = Vec::new();
            while let Ok(frame) = frames.try_recv() {
                let envelope: WsServerEnvelope<WatchServerEventV1> =
                    serde_json::from_str(&frame).unwrap();
                seen.push(envelope.into_payload());
            }
            assert!(
                seen.iter()
                    .any(|event| matches!(event, WatchServerEventV1::AlertUpdated { .. })),
                "every attached page must receive the owner's alert",
            );
        }

        // The control: a connection-scoped START still answers only its own
        // connection.
        send(
            &socket,
            first,
            trace(3),
            WatchClientCommandV1::StartWatch {
                items: items([section(2)]),
            },
        );
        assert!(matches!(
            receive(&mut first_frames).into_payload(),
            WatchServerEventV1::StartResult { .. },
        ));
        assert!(
            second_frames.try_recv().is_err(),
            "another connection's START must not be broadcast",
        );
    }

    /// The owner id is minted in Rust and never travels. A transport
    /// connection that presented it would inherit every watch the process
    /// holds -- and could then stop them by naming ids it did not create.
    #[test]
    fn a_transport_connection_cannot_claim_the_owner_connection_id() {
        let socket = socket(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            Arc::new(NoopWatchDispatchSink),
        );
        let owner = socket.ensure_owner_connection().expect("owner");
        assert_eq!(
            socket.ensure_owner_connection().expect("owner"),
            owner,
            "asking twice must not open a second owner",
        );

        let (outbound, _frames) = OutboundSender::unbounded_pair();
        assert!(
            !socket.connect(owner, outbound),
            "a wire connection must not be able to become the owner",
        );
    }

    /// The owner has no socket, so no inbound frame ever refreshes it. The
    /// maintenance tick does instead -- otherwise the heartbeat sweep would
    /// expire the connection holding every physical watch, and monitoring
    /// would stop while the pages that asked for it stayed open.
    #[test]
    fn the_maintenance_tick_keeps_the_owner_connection_alive() {
        let (socket, clock) = socket_with_clock(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            Arc::new(NoopWatchDispatchSink),
        );
        let (outbound, _frames) = OutboundSender::unbounded_pair();
        assert!(socket.connect(trace(1), outbound));
        socket
            .owner_start(items([section(1)]))
            .expect("owner start");
        let owner = socket.owner_connection().expect("owner");

        // Well past the 60s heartbeat timeout, with the owner never having
        // been sent a frame.
        for _ in 0..5 {
            clock.advance(Duration::from_secs(30));
            socket.tick();
        }
        assert_eq!(socket.owner_connection(), Some(owner));
        assert_eq!(socket.owner_watched_sections(), vec![section(1)]);
        assert_eq!(socket.total_active_watch_count(), 1);
    }

    /// A process stop clears the manager, so the remembered owner id names
    /// nothing. Handing it to a coordinator afterwards would produce an
    /// unbroken stream of unknown-connection errors; the next request opens
    /// a fresh one instead.
    #[test]
    fn a_process_stop_forgets_the_owner_so_the_next_request_opens_a_new_one() {
        let socket = socket(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            Arc::new(NoopWatchDispatchSink),
        );
        socket
            .owner_start(items([section(1)]))
            .expect("owner start");
        let first = socket.owner_connection().expect("owner");

        socket.stop();
        assert_eq!(socket.owner_connection(), None);
        assert_eq!(socket.total_active_watch_count(), 0);

        let second = socket.ensure_owner_connection().expect("owner");
        assert_ne!(second, first);
        assert!(
            socket
                .owner_start(items([section(1)]))
                .expect("owner start")[0]
                .is_active(),
        );
    }

    /// The three commands that create or destroy a watch are refused on the
    /// owner API too. A target that routes frames to the owner must not be
    /// able to reintroduce, through that route, the second source of truth
    /// it refused on the wire.
    #[test]
    fn the_owner_command_entry_point_refuses_watch_lifecycle_commands() {
        let socket = socket(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            Arc::new(NoopWatchDispatchSink),
        );
        let (outbound, _frames) = OutboundSender::unbounded_pair();
        assert!(socket.connect(trace(1), outbound));
        socket
            .owner_start(items([section(1)]))
            .expect("owner start");

        assert_eq!(
            socket.owner_command(WatchClientCommandV1::StartWatch {
                items: items([section(2)]),
            }),
            Err(WatchManagerError::TargetMismatch),
        );
        assert_eq!(
            socket.owner_command(WatchClientCommandV1::AcknowledgeAllEpisodes {}),
            Ok(()),
            "episode control is what the owner entry point is for",
        );
        assert_eq!(socket.owner_watched_sections(), vec![section(1)]);
    }

    /// Sealing is for a runtime that is shutting down, and the owner is the
    /// one connection nothing would ever close: it has no socket to drop, so
    /// a watch rebuilt on it would outlive the process that was tearing
    /// itself down and keep polling Rutgers with nobody to tell.
    ///
    /// The second half is the control, and it is the difference between the
    /// two clean-ups: an ORDINARY stop -- what a Full Reset does -- leaves
    /// the socket usable, because the user is still there and the next page
    /// must be able to attach and be served.
    #[test]
    fn a_sealed_socket_refuses_to_rebuild_the_owner_but_an_ordinary_stop_does_not() {
        let socket = socket(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            Arc::new(NoopWatchDispatchSink),
        );
        socket
            .owner_start(items([section(1)]))
            .expect("owner start");
        assert_eq!(socket.owner_watched_sections(), vec![section(1)]);

        // An ordinary stop: the owner is forgotten, and asking again opens a
        // new one.
        socket.stop();
        assert_eq!(socket.owner_connection(), None);
        socket
            .ensure_owner_connection()
            .expect("an ordinary stop leaves the socket usable");
        assert!(
            socket
                .owner_start(items([section(1)]))
                .expect("owner start")[0]
                .is_active(),
        );
        let (outbound, _frames) = OutboundSender::unbounded_pair();
        assert!(socket.connect(trace(1), outbound));

        socket.seal_and_stop();
        assert_eq!(socket.owner_connection(), None);
        assert_eq!(
            socket.ensure_owner_connection(),
            Err(WatchManagerError::SocketSealed),
        );
        assert_eq!(
            socket.owner_start(items([section(1)])).err(),
            Some(WatchManagerError::SocketSealed),
        );
        assert_eq!(
            socket.owner_stop(ActiveWatchTargetV1 {
                active_watch_id: bcsp_contracts::ActiveWatchId::new(trace(0xdead)),
                section_key: section(1),
            }),
            Err(WatchManagerError::SocketSealed),
        );
        assert_eq!(socket.owner_connection(), None);
        assert_eq!(socket.total_active_watch_count(), 0);
    }

    /// Under a coordinator every physical watch is held by the owner. A
    /// maintenance sweep that only looked at browser connections would
    /// therefore never stop anything -- a section that rolled out of the term
    /// window or off a product campus would keep being polled, and keep being
    /// reported as watched, for as long as the process ran.
    #[test]
    fn the_maintenance_sweep_stops_an_owner_held_watch_that_leaves_the_term_window() {
        let in_range = Arc::new(AtomicBool::new(true));
        let sink = Arc::new(RecordingSink::default());
        let socket = socket(
            Arc::new(MutableRangeAdmission {
                in_range: in_range.clone(),
            }),
            sink.clone(),
        );
        let (outbound, mut frames) = OutboundSender::unbounded_pair();
        assert!(socket.connect(trace(1), outbound));
        socket
            .owner_start(items([section(1)]))
            .expect("owner start");
        assert_eq!(socket.owner_watched_sections(), vec![section(1)]);
        while frames.try_recv().is_ok() {}

        in_range.store(false, Ordering::SeqCst);
        socket.tick();

        assert!(
            socket.owner_watched_sections().is_empty(),
            "the sweep must cover the connection that actually holds the watches",
        );
        assert_eq!(socket.total_active_watch_count(), 0);
        // The page hears about it, because the watch was held for the page.
        let stopped = std::iter::from_fn(|| frames.try_recv().ok())
            .map(|frame| {
                serde_json::from_str::<WsServerEnvelope<WatchServerEventV1>>(&frame).unwrap()
            })
            .any(|envelope| {
                matches!(
                    envelope.payload(),
                    WatchServerEventV1::WatchStopped { stopped }
                        if stopped.reason == WatchStopReason::TermOutOfRange
                )
            });
        assert!(
            stopped,
            "the owner's forced stop is fanned out to every page"
        );
    }

    /// The identity of every owner-held watch, which is what tells "the watch
    /// I armed is still running" apart from "that one ended and something
    /// started a new one for the same Section".
    #[test]
    fn owner_watch_targets_report_the_identity_of_each_running_watch() {
        let socket = socket(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            Arc::new(NoopWatchDispatchSink),
        );
        let results = socket
            .owner_start(items([section(1)]))
            .expect("owner start");
        let WatchStartItemResultV1::Active {
            active_watch_id, ..
        } = results[0]
        else {
            panic!("the watch must be active");
        };
        let targets = socket.owner_watch_targets();
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].section_key, section(1));
        assert_eq!(targets[0].active_watch_id, active_watch_id);

        socket.owner_stop(targets[0].clone()).expect("owner stop");
        assert!(socket.owner_watch_targets().is_empty());
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
        let (outbound, mut receiver) = OutboundSender::unbounded_pair();
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
    fn oversized_start_uses_one_batch_admission_and_keeps_the_public_cap() {
        let admission = Arc::new(CountingBatchAdmission::default());
        let socket = socket(admission.clone(), Arc::new(NoopWatchDispatchSink));
        let connection_id = trace(1);
        let (outbound, mut receiver) = OutboundSender::unbounded_pair();
        assert!(socket.connect(connection_id, outbound));

        send(
            &socket,
            connection_id,
            trace(2),
            WatchClientCommandV1::StartWatch {
                items: items((1..=usize::from(u8::MAX)).map(section)),
            },
        );

        let WatchServerEventV1::StartResult { result } = receive(&mut receiver).into_payload()
        else {
            panic!("START must produce START_RESULT");
        };
        assert_eq!(
            result.active_watch_count(),
            bcsp_contracts::MAX_ACTIVE_WATCHES
        );
        assert_eq!(
            result
                .items()
                .iter()
                .filter(|item| item.is_active())
                .count(),
            usize::from(bcsp_contracts::MAX_ACTIVE_WATCHES),
        );
        assert!(
            result.items()[usize::from(bcsp_contracts::MAX_ACTIVE_WATCHES)..]
                .iter()
                .all(|item| matches!(
                    item,
                    WatchStartItemResultV1::Rejected {
                        reason: WatchStartRejectionReason::MaxActiveWatches,
                        ..
                    }
                ))
        );
        assert_eq!(admission.batches.load(Ordering::SeqCst), 1);
        assert_eq!(admission.singles.load(Ordering::SeqCst), 0);
        assert_eq!(
            admission.last_batch_len.load(Ordering::SeqCst),
            usize::from(u8::MAX),
        );
    }

    #[test]
    fn explicit_local_capacity_arms_255_items_with_one_batch_admission() {
        let admission = Arc::new(CountingBatchAdmission::default());
        let socket = SharedWatchSocket::try_new_with_parts_and_max_active_watches(
            FakeClock::default(),
            FakeIds(0x100),
            FakeIds(0x900),
            Duration::from_secs(60),
            admission.clone(),
            Arc::new(NoopWatchDispatchSink),
            NonZeroU8::new(u8::MAX).unwrap(),
        )
        .unwrap();

        let results = socket
            .owner_start(items((1..=usize::from(u8::MAX)).map(section)))
            .expect("local owner start");
        assert_eq!(results.len(), usize::from(u8::MAX));
        assert!(results.iter().all(WatchStartItemResultV1::is_active));
        assert_eq!(socket.owner_watched_sections().len(), usize::from(u8::MAX));
        assert_eq!(admission.batches.load(Ordering::SeqCst), 1);
        assert_eq!(admission.singles.load(Ordering::SeqCst), 0);

        let overflow = socket
            .owner_start(items([section(256)]))
            .expect("per-item capacity result");
        assert_eq!(
            overflow,
            vec![WatchStartItemResultV1::Rejected {
                section_key: section(256),
                reason: WatchStartRejectionReason::MaxActiveWatches,
            }]
        );
    }

    #[test]
    fn batch_admission_does_not_hold_the_socket_lock_away_from_ping_maintenance() {
        let admission = Arc::new(BlockingBatchAdmission {
            entered: Barrier::new(2),
            release: Barrier::new(2),
        });
        let (socket, clock) = socket_with_clock(admission.clone(), Arc::new(NoopWatchDispatchSink));
        let socket = Arc::new(socket);
        let (outbound, mut receiver) = OutboundSender::unbounded_pair();
        assert!(socket.connect(trace(1), outbound));
        clock.advance(WATCH_APP_PING_INTERVAL);

        let starting = {
            let socket = socket.clone();
            std::thread::spawn(move || socket.owner_start(items([section(1)])))
        };
        admission.entered.wait();

        let (finished, observed) = std::sync::mpsc::sync_channel(1);
        let ticking = {
            let socket = socket.clone();
            std::thread::spawn(move || {
                socket.tick();
                let _ = finished.send(());
            })
        };
        let ping_was_not_blocked = observed.recv_timeout(Duration::from_millis(250)).is_ok();
        let ping = receiver.try_recv().ok();

        admission.release.wait();
        ticking.join().unwrap();
        starting.join().unwrap().expect("owner start");

        assert!(ping_was_not_blocked, "PING maintenance waited on admission");
        let ping = ping.expect("PING was emitted while admission was blocked");
        let envelope: WsServerEnvelope<WatchServerEventV1> = serde_json::from_str(&ping).unwrap();
        assert!(matches!(
            envelope.payload(),
            WatchServerEventV1::Ping { .. }
        ));
    }

    #[test]
    fn ordinary_stop_allows_a_new_connection_after_user_data_reset() {
        let socket = socket(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            Arc::new(NoopWatchDispatchSink),
        );
        let first_connection = trace(90);
        let second_connection = trace(91);
        let (first_outbound, mut first_receiver) = OutboundSender::unbounded_pair();
        assert!(socket.connect(first_connection, first_outbound));

        socket.stop();

        assert_eq!(socket.connection_count(), 0);
        assert!(matches!(
            first_receiver.try_recv(),
            Err(mpsc::error::TryRecvError::Disconnected)
        ));
        let (second_outbound, _second_receiver) = OutboundSender::unbounded_pair();
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
        let (first_outbound, mut first_receiver) = OutboundSender::unbounded_pair();
        assert!(socket.connect(first_connection, first_outbound));

        socket.seal_and_stop();
        socket.seal_and_stop();

        assert_eq!(socket.connection_count(), 0);
        assert!(matches!(
            first_receiver.try_recv(),
            Err(mpsc::error::TryRecvError::Disconnected)
        ));
        let (second_outbound, _second_receiver) = OutboundSender::unbounded_pair();
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
        let (outbound, mut receiver) = OutboundSender::unbounded_pair();
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
        let (outbound, mut receiver) = OutboundSender::unbounded_pair();
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
        let (first_outbound, mut first_receiver) = OutboundSender::unbounded_pair();
        let (second_outbound, mut second_receiver) = OutboundSender::unbounded_pair();
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
        let (outbound, mut receiver) = OutboundSender::unbounded_pair();
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
        let (outbound, mut receiver) = OutboundSender::unbounded_pair();
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
        let (outbound, mut receiver) = OutboundSender::unbounded_pair();
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
        let (outbound, mut receiver) = OutboundSender::unbounded_pair();
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
        let (outbound, mut receiver) = OutboundSender::unbounded_pair();
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
        let (outbound, mut receiver) = OutboundSender::unbounded_pair();
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
        let (outbound, mut receiver) = OutboundSender::unbounded_pair();
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
        let (outbound, mut receiver) = OutboundSender::unbounded_pair();
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
        let (outbound, mut receiver) = OutboundSender::unbounded_pair();
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
        let (outbound, mut receiver) = OutboundSender::unbounded_pair();
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

    /// H9 (R2): the accepted-ACK counter is the server's OWN positive
    /// evidence that the heartbeat round trip closed, so it may move only for
    /// an acknowledgement this server can actually vouch for -- decoded as a
    /// v1 command, on a known connection, naming a sequence this connection
    /// was really sent and has not already acknowledged.
    ///
    /// Every refusal below is a way the counter could have been made to lie:
    /// counting arbitrary inbound text (transport activity), counting an
    /// invented sequence, counting a replay, or counting any command that
    /// happens to arrive. The soak reads exactly this number.
    #[test]
    fn only_a_fresh_ack_for_an_issued_sequence_counts_as_accepted() {
        let sink = Arc::new(RecordingSink::default());
        let (socket, clock) = socket_with_clock(
            Arc::new(|_: &SectionKey| WatchStartAdmission::admitted(None)),
            sink.clone(),
        );
        let connection_id = trace(0xa0);
        let (outbound, mut receiver) = OutboundSender::unbounded_pair();
        assert!(socket.connect(connection_id, outbound));
        assert_eq!(socket.accepted_heartbeat_acks(), 0);

        // Transport activity and undecodable text are not acknowledgements.
        socket.transport_activity(connection_id);
        socket.receive_text(connection_id, "{not a frame at all");
        assert_eq!(
            socket.accepted_heartbeat_acks(),
            0,
            "inbound bytes are not evidence that an ACK was accepted"
        );

        // No PING has been issued yet, so no sequence can be acknowledged.
        send(
            &socket,
            connection_id,
            trace(0xa001),
            WatchClientCommandV1::HeartbeatAck { sequence: 1 },
        );
        assert_eq!(
            socket.accepted_heartbeat_acks(),
            0,
            "an ACK for a sequence the server never sent is not evidence"
        );

        clock.advance(WATCH_APP_PING_INTERVAL);
        socket.tick();
        let WatchServerEventV1::Ping { sequence } = receive(&mut receiver).into_payload() else {
            panic!("expected an application-level PING");
        };
        assert_eq!(sequence, 1);

        for (label, invalid) in [("zero", 0u64), ("ahead of the issued sequence", 2u64)] {
            send(
                &socket,
                connection_id,
                trace(0xa002),
                WatchClientCommandV1::HeartbeatAck { sequence: invalid },
            );
            assert_eq!(
                socket.accepted_heartbeat_acks(),
                0,
                "a {label} sequence must not count as an accepted ACK"
            );
        }

        send(
            &socket,
            connection_id,
            trace(0xa003),
            WatchClientCommandV1::HeartbeatAck { sequence },
        );
        assert_eq!(
            socket.accepted_heartbeat_acks(),
            1,
            "the real acknowledgement of an issued sequence is the evidence"
        );
        send(
            &socket,
            connection_id,
            trace(0xa004),
            WatchClientCommandV1::HeartbeatAck { sequence },
        );
        assert_eq!(
            socket.accepted_heartbeat_acks(),
            1,
            "replaying an acknowledged sequence says nothing new"
        );

        // An ordinary command refreshes the heartbeat like an ACK does; it is
        // still not an ACK.
        send(
            &socket,
            connection_id,
            trace(0xa005),
            WatchClientCommandV1::StartWatch {
                items: items([section(160)]),
            },
        );
        let _ = receive(&mut receiver);
        assert_eq!(socket.accepted_heartbeat_acks(), 1);

        clock.advance(WATCH_APP_PING_INTERVAL);
        socket.tick();
        let WatchServerEventV1::Ping { sequence: second } = receive(&mut receiver).into_payload()
        else {
            panic!("expected the second application-level PING");
        };
        assert_eq!(second, 2);
        send(
            &socket,
            connection_id,
            trace(0xa006),
            WatchClientCommandV1::HeartbeatAck { sequence: second },
        );
        assert_eq!(socket.accepted_heartbeat_acks(), 2);
        send(
            &socket,
            connection_id,
            trace(0xa007),
            WatchClientCommandV1::HeartbeatAck { sequence },
        );
        assert_eq!(
            socket.accepted_heartbeat_acks(),
            2,
            "an ACK older than the last accepted one is stale, not evidence"
        );
        assert!(
            receiver.try_recv().is_err(),
            "none of this produced an outbound frame"
        );
    }
}
