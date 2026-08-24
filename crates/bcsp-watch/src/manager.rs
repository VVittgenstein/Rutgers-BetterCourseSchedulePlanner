use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::Duration;

use bcsp_contracts::{
    MAX_ACTIVE_WATCHES, OpenObservationV1, OpenState, SectionKey, TermCampusKey, TraceId,
    TraceIdSource,
};
use thiserror::Error;
use time::OffsetDateTime;

use crate::effect::{
    CoreAction, CoreActionKind, CoreAlertChange, CoreAlertUpdate, CoreAudioDisposition,
    CoreCleanupReason, CoreCleanupReport, CoreCueCancellationReason, CoreCueOutcome, CoreDispatch,
    CoreEvent, CoreMixerStopReason, CoreOneShotDisposition, CorePublishOutcome,
    CoreStartDisposition, CoreStartOutcome, CoreStartResult, CoreTickOutcome,
};
use crate::state::{
    CoreEpisode, CoreEpisodeEndReason, CoreEpisodeState, CoreInitialState, CoreNotificationMode,
    CorePolicy, CoreWatchSession, ObservationCursor, PendingCue, ReliableState,
};
use crate::{WatchClock, WatchInstant};

const START_REPLAY_WINDOW: usize = 256;
const SECTION_CURSOR_WINDOW: usize = 4096;

#[derive(Clone, Copy, Debug)]
pub(crate) enum CoreStartAdmission {
    Admitted {
        initial_state: CoreInitialState,
        initial_cursor: Option<ObservationCursor>,
    },
    SectionNotFound,
    TargetUnavailable,
    UnsupportedTarget,
    TermOutOfRange,
}

#[derive(Clone, Debug)]
pub(crate) struct CoreStartSpec {
    pub(crate) section: SectionKey,
    pub(crate) policy: CorePolicy,
    pub(crate) admission: CoreStartAdmission,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub(crate) enum CoreManagerError {
    #[error("heartbeat timeout must be positive")]
    InvalidHeartbeatTimeout,
    #[error("connection already exists")]
    ConnectionAlreadyExists,
    #[error("connection does not exist")]
    UnknownConnection,
    #[error("an observation sequence mapped to two observation IDs")]
    ObservationSequenceConflict,
    #[error("watch does not exist on this connection")]
    UnknownWatch,
    #[error("episode does not exist on this connection")]
    UnknownEpisode,
    #[error("alert does not exist on this connection")]
    UnknownAlert,
    #[error("timed-out episode cannot be resumed")]
    EpisodeNotResumable,
    #[error("episode action is only valid for CONTINUOUS mode")]
    InvalidEpisodeAction,
}

#[derive(Clone, Debug)]
struct CoreConnection {
    last_touch: WatchInstant,
    watches: BTreeMap<SectionKey, CoreWatchSession>,
    start_replays: BTreeMap<TraceId, (CoreDispatch, u8)>,
    start_replay_order: VecDeque<TraceId>,
}

#[derive(Clone, Copy, Debug)]
struct SectionObservationWatermark {
    cursor: ObservationCursor,
    published: bool,
}

impl CoreConnection {
    fn new(now: WatchInstant) -> Self {
        Self {
            last_touch: now,
            watches: BTreeMap::new(),
            start_replays: BTreeMap::new(),
            start_replay_order: VecDeque::new(),
        }
    }

    fn active_episode_ids(&self) -> Vec<TraceId> {
        self.watches
            .values()
            .filter(|watch| watch.continuous_alarm_active())
            .filter_map(|watch| watch.episode.as_ref().map(|episode| episode.id))
            .collect()
    }
}

pub(crate) struct CoreWatchManager<C, I> {
    clock: C,
    ids: I,
    heartbeat_timeout: Duration,
    connections: BTreeMap<TraceId, CoreConnection>,
    connections_by_section: BTreeMap<SectionKey, BTreeSet<TraceId>>,
    observation_cursors: BTreeMap<SectionKey, SectionObservationWatermark>,
    observation_cursor_order: VecDeque<SectionKey>,
}

impl<C, I> CoreWatchManager<C, I>
where
    C: WatchClock,
    I: TraceIdSource,
{
    pub(crate) fn try_new(
        clock: C,
        ids: I,
        heartbeat_timeout: Duration,
    ) -> Result<Self, CoreManagerError> {
        if heartbeat_timeout.is_zero() {
            return Err(CoreManagerError::InvalidHeartbeatTimeout);
        }
        Ok(Self {
            clock,
            ids,
            heartbeat_timeout,
            connections: BTreeMap::new(),
            connections_by_section: BTreeMap::new(),
            observation_cursors: BTreeMap::new(),
            observation_cursor_order: VecDeque::new(),
        })
    }

    pub(crate) fn clock_mut(&mut self) -> &mut C {
        &mut self.clock
    }

    pub(crate) fn connection_count(&self) -> usize {
        self.connections.len()
    }

    pub(crate) fn connect(&mut self, connection_id: TraceId) -> Result<(), CoreManagerError> {
        if self.connections.contains_key(&connection_id) {
            return Err(CoreManagerError::ConnectionAlreadyExists);
        }
        let now = self.clock.now();
        self.connections
            .insert(connection_id, CoreConnection::new(now));
        Ok(())
    }

    pub(crate) fn touch(&mut self, connection_id: TraceId) -> Result<(), CoreManagerError> {
        let now = self.clock.now();
        let connection = self
            .connections
            .get_mut(&connection_id)
            .ok_or(CoreManagerError::UnknownConnection)?;
        connection.last_touch = now;
        Ok(())
    }

    pub(crate) fn start(
        &mut self,
        connection_id: TraceId,
        message_id: TraceId,
        specs: Vec<CoreStartSpec>,
    ) -> Result<CoreStartOutcome, CoreManagerError> {
        let now = self.clock.now();
        let mut connection = self
            .connections
            .remove(&connection_id)
            .ok_or(CoreManagerError::UnknownConnection)?;
        if let Some((dispatch, active_watch_count)) =
            connection.start_replays.get(&message_id).cloned()
        {
            connection.last_touch = now;
            self.connections.insert(connection_id, connection);
            return Ok(CoreStartOutcome {
                dispatch,
                replayed: true,
                active_watch_count,
            });
        }
        if let Err(error) = self.validate_initial_watermarks(&specs) {
            self.connections.insert(connection_id, connection);
            return Err(error);
        }

        let emitted_at = self.clock.wall_now();
        connection.last_touch = now;
        let previous_mixer = connection.active_episode_ids();
        let mut events = Vec::new();
        let mut in_message = BTreeSet::new();
        for spec in specs {
            let section = spec.section.clone();
            if !in_message.insert(section.clone()) {
                events.push(CoreEvent::StartResult(CoreStartResult {
                    section,
                    disposition: CoreStartDisposition::RejectedDuplicate,
                    watch_id: None,
                }));
                continue;
            }

            let (initial_state, initial_cursor) = match spec.admission {
                CoreStartAdmission::Admitted {
                    initial_state,
                    initial_cursor,
                } => (initial_state, initial_cursor),
                CoreStartAdmission::SectionNotFound => {
                    events.push(CoreEvent::StartResult(CoreStartResult {
                        section,
                        disposition: CoreStartDisposition::RejectedSectionNotFound,
                        watch_id: None,
                    }));
                    continue;
                }
                CoreStartAdmission::TargetUnavailable => {
                    events.push(CoreEvent::StartResult(CoreStartResult {
                        section,
                        disposition: CoreStartDisposition::RejectedTargetUnavailable,
                        watch_id: None,
                    }));
                    continue;
                }
                CoreStartAdmission::UnsupportedTarget => {
                    events.push(CoreEvent::StartResult(CoreStartResult {
                        section,
                        disposition: CoreStartDisposition::RejectedUnsupportedTarget,
                        watch_id: None,
                    }));
                    continue;
                }
                CoreStartAdmission::TermOutOfRange => {
                    events.push(CoreEvent::StartResult(CoreStartResult {
                        section,
                        disposition: CoreStartDisposition::RejectedTermOutOfRange,
                        watch_id: None,
                    }));
                    continue;
                }
            };

            let previous = connection.watches.remove(&section);
            let disposition = if previous.is_some() {
                CoreStartDisposition::Restarted
            } else if connection.watches.len() == usize::from(MAX_ACTIVE_WATCHES) {
                events.push(CoreEvent::StartResult(CoreStartResult {
                    section,
                    disposition: CoreStartDisposition::RejectedLimit,
                    watch_id: None,
                }));
                continue;
            } else {
                CoreStartDisposition::Started
            };

            if let Some(mut previous) = previous {
                self.end_session(
                    &mut previous,
                    CoreEpisodeEndReason::Restarted,
                    emitted_at,
                    Some(CoreCueCancellationReason::WatchRestarted),
                    &mut events,
                );
            } else {
                self.connections_by_section
                    .entry(section.clone())
                    .or_default()
                    .insert(connection_id);
            }

            let watch_id = self.ids.next_trace_id();
            let mut session = CoreWatchSession::new(
                watch_id,
                section.clone(),
                spec.policy,
                initial_state,
                initial_cursor,
            );
            if let Some(cursor) = initial_cursor {
                self.seed_observation_watermark(section.clone(), cursor);
            }
            events.push(CoreEvent::StartResult(CoreStartResult {
                section: section.clone(),
                disposition,
                watch_id: Some(watch_id),
            }));
            self.push_action(
                &mut events,
                match disposition {
                    CoreStartDisposition::Started => CoreActionKind::WatchStarted,
                    CoreStartDisposition::Restarted => CoreActionKind::WatchRestarted,
                    CoreStartDisposition::RejectedLimit
                    | CoreStartDisposition::RejectedDuplicate
                    | CoreStartDisposition::RejectedSectionNotFound
                    | CoreStartDisposition::RejectedTargetUnavailable
                    | CoreStartDisposition::RejectedUnsupportedTarget
                    | CoreStartDisposition::RejectedTermOutOfRange => unreachable!(),
                },
                Some(section.clone()),
                Some(watch_id),
                None,
            );
            if let CoreInitialState::Open {
                observation_id,
                observed_at,
            } = initial_state
            {
                self.open_episode(&mut session, observation_id, observed_at, now, &mut events);
                if session.policy.mode == CoreNotificationMode::OneShot {
                    self.queue_one_shot(&mut session, observation_id, &mut events);
                }
            }
            connection.watches.insert(section, session);
        }
        self.push_mixer_if_changed(
            &connection,
            previous_mixer,
            CoreMixerStopReason::NoUnacknowledgedEpisodes,
            &mut events,
        );
        let active_watch_count = u8::try_from(connection.watches.len())
            .expect("connection watch count is capped at nine");
        let dispatch = CoreDispatch {
            connection_id,
            emitted_at,
            events,
        };
        connection
            .start_replays
            .insert(message_id, (dispatch.clone(), active_watch_count));
        connection.start_replay_order.push_back(message_id);
        if connection.start_replay_order.len() > START_REPLAY_WINDOW
            && let Some(expired) = connection.start_replay_order.pop_front()
        {
            connection.start_replays.remove(&expired);
        }
        self.connections.insert(connection_id, connection);
        Ok(CoreStartOutcome {
            dispatch,
            replayed: false,
            active_watch_count,
        })
    }

    pub(crate) fn stop_watch(
        &mut self,
        connection_id: TraceId,
        watch_id: TraceId,
    ) -> Result<CoreDispatch, CoreManagerError> {
        let mut connection = self
            .connections
            .remove(&connection_id)
            .ok_or(CoreManagerError::UnknownConnection)?;
        let previous_mixer = connection.active_episode_ids();
        let Some(section) = connection
            .watches
            .iter()
            .find_map(|(section, watch)| (watch.id == watch_id).then(|| section.clone()))
        else {
            self.connections.insert(connection_id, connection);
            return Err(CoreManagerError::UnknownWatch);
        };
        let mut session = connection
            .watches
            .remove(&section)
            .expect("located watch exists");
        let emitted_at = self.clock.wall_now();
        let mut events = Vec::new();
        self.end_session(
            &mut session,
            CoreEpisodeEndReason::WatchStopped,
            emitted_at,
            Some(CoreCueCancellationReason::WatchStopped),
            &mut events,
        );
        self.push_action(
            &mut events,
            CoreActionKind::WatchStopped,
            Some(section.clone()),
            Some(watch_id),
            session.episode.as_ref().map(|episode| episode.id),
        );
        events.push(CoreEvent::WatchStopped {
            section: section.clone(),
            watch_id,
        });
        self.unlink(connection_id, &section);
        self.push_mixer_if_changed(
            &connection,
            previous_mixer,
            CoreMixerStopReason::NoUnacknowledgedEpisodes,
            &mut events,
        );
        self.connections.insert(connection_id, connection);
        Ok(CoreDispatch {
            connection_id,
            emitted_at,
            events,
        })
    }

    pub(crate) fn publish(
        &mut self,
        observation: OpenObservationV1,
    ) -> Result<CorePublishOutcome, CoreManagerError> {
        let section = observation.section_key().clone();
        let pull_sequence = observation.pull_sequence.get();
        let watermark = self.observation_cursors.get(&section);
        let globally_new = cursor_accepts(
            watermark.map(|value| &value.cursor),
            pull_sequence,
            observation.observation_id,
        )?;
        let seeded_exact = watermark.is_some_and(|value| {
            !value.published
                && value.cursor.pull_sequence == pull_sequence
                && value.cursor.observation_id == observation.observation_id
        });
        if !globally_new && !seeded_exact {
            return Ok(CorePublishOutcome {
                dispatches: Vec::new(),
                replayed: true,
            });
        }
        let connection_ids = self
            .connections_by_section
            .get(&section)
            .cloned()
            .unwrap_or_default();
        for connection_id in &connection_ids {
            let Some(session) = self
                .connections
                .get(connection_id)
                .and_then(|connection| connection.watches.get(&section))
            else {
                continue;
            };
            cursor_accepts(
                session.observation_cursor.as_ref(),
                pull_sequence,
                observation.observation_id,
            )?;
        }

        self.record_published_observation(
            section.clone(),
            ObservationCursor {
                pull_sequence,
                observation_id: observation.observation_id,
            },
        );
        if connection_ids.is_empty() {
            return Ok(CorePublishOutcome {
                dispatches: Vec::new(),
                replayed: false,
            });
        }
        let now = self.clock.now();
        let emitted_at = self.clock.wall_now();
        let mut dispatches = Vec::with_capacity(connection_ids.len());
        for connection_id in connection_ids {
            let Some(mut connection) = self.connections.remove(&connection_id) else {
                continue;
            };
            let previous_mixer = connection.active_episode_ids();
            let mut events = Vec::new();
            if let Some(mut session) = connection.watches.remove(&section) {
                events.push(CoreEvent::Observation(observation.clone()));
                if cursor_accepts(
                    session.observation_cursor.as_ref(),
                    pull_sequence,
                    observation.observation_id,
                )? {
                    session.observation_cursor = Some(ObservationCursor {
                        pull_sequence,
                        observation_id: observation.observation_id,
                    });
                    self.apply_observation(&mut session, &observation, now, &mut events);
                }
                connection.watches.insert(section.clone(), session);
            }
            self.push_mixer_if_changed(
                &connection,
                previous_mixer,
                CoreMixerStopReason::NoUnacknowledgedEpisodes,
                &mut events,
            );
            self.connections.insert(connection_id, connection);
            if !events.is_empty() {
                dispatches.push(CoreDispatch {
                    connection_id,
                    emitted_at,
                    events,
                });
            }
        }
        Ok(CorePublishOutcome {
            replayed: dispatches.is_empty(),
            dispatches,
        })
    }

    pub(crate) fn update_policy(
        &mut self,
        connection_id: TraceId,
        watch_id: TraceId,
        policy: CorePolicy,
    ) -> Result<CoreDispatch, CoreManagerError> {
        let mut connection = self.take_connection(connection_id)?;
        let previous_mixer = connection.active_episode_ids();
        let now = self.clock.now();
        let emitted_at = self.clock.wall_now();
        let Some(session) = connection
            .watches
            .values_mut()
            .find(|session| session.id == watch_id)
        else {
            self.connections.insert(connection_id, connection);
            return Err(CoreManagerError::UnknownWatch);
        };
        let previous_mode = session.policy.mode;
        let cancelled_cues = if previous_mode == CoreNotificationMode::OneShot
            && policy.mode == CoreNotificationMode::Continuous
        {
            session.take_pending_cues()
        } else {
            Vec::new()
        };
        if policy.mode == CoreNotificationMode::Continuous {
            if let Some(episode) = session.episode.as_mut()
                && episode.state == CoreEpisodeState::Unacknowledged
            {
                episode.deadline = policy.continuous_duration.deadline(now);
            }
        } else {
            if let Some(episode) = session.episode.as_mut() {
                episode.deadline = None;
            }
        }
        session.policy = policy;
        if let Some(episode) = session.episode.as_mut() {
            episode.policy = policy;
        }
        let mut events = Vec::new();
        push_cancelled_cues(
            &mut events,
            session.id,
            &session.section,
            cancelled_cues,
            CoreCueCancellationReason::NotificationModeChanged,
        );
        if let Some(episode) = session.episode.as_ref() {
            events.push(CoreEvent::EpisodeUpdated(episode.clone()));
        }
        self.push_action(
            &mut events,
            CoreActionKind::PolicyUpdated,
            Some(session.section.clone()),
            Some(watch_id),
            session.episode.as_ref().map(|episode| episode.id),
        );
        self.push_mixer_if_changed(
            &connection,
            previous_mixer,
            if previous_mode != policy.mode {
                CoreMixerStopReason::NotificationModeChanged
            } else {
                CoreMixerStopReason::NoUnacknowledgedEpisodes
            },
            &mut events,
        );
        self.connections.insert(connection_id, connection);
        Ok(CoreDispatch {
            connection_id,
            emitted_at,
            events,
        })
    }

    pub(crate) fn acknowledge_episode(
        &mut self,
        connection_id: TraceId,
        episode_id: TraceId,
    ) -> Result<CoreDispatch, CoreManagerError> {
        let mut connection = self.take_connection(connection_id)?;
        let previous_mixer = connection.active_episode_ids();
        let Some(session) = connection.watches.values_mut().find(|session| {
            session
                .episode
                .as_ref()
                .is_some_and(|episode| episode.id == episode_id)
        }) else {
            self.connections.insert(connection_id, connection);
            return Err(CoreManagerError::UnknownEpisode);
        };
        if session.policy.mode != CoreNotificationMode::Continuous {
            self.connections.insert(connection_id, connection);
            return Err(CoreManagerError::InvalidEpisodeAction);
        }
        let emitted_at = self.clock.wall_now();
        let mut events = Vec::new();
        if let Some(episode) = session.episode.as_mut()
            && episode.state == CoreEpisodeState::Unacknowledged
        {
            episode.state = CoreEpisodeState::Acknowledged;
            episode.deadline = None;
            episode.state_changed_at = emitted_at.max(episode.first_observed_at);
            events.push(CoreEvent::EpisodeUpdated(episode.clone()));
            self.push_action(
                &mut events,
                CoreActionKind::EpisodeAcknowledged,
                Some(session.section.clone()),
                Some(session.id),
                Some(episode_id),
            );
        }
        self.push_mixer_if_changed(
            &connection,
            previous_mixer,
            CoreMixerStopReason::NoUnacknowledgedEpisodes,
            &mut events,
        );
        self.connections.insert(connection_id, connection);
        Ok(CoreDispatch {
            connection_id,
            emitted_at,
            events,
        })
    }

    pub(crate) fn acknowledge_all(
        &mut self,
        connection_id: TraceId,
    ) -> Result<CoreDispatch, CoreManagerError> {
        let mut connection = self.take_connection(connection_id)?;
        let previous_mixer = connection.active_episode_ids();
        let emitted_at = self.clock.wall_now();
        let mut events = Vec::new();
        let mut acknowledged = Vec::new();
        for session in connection.watches.values_mut() {
            if session.policy.mode != CoreNotificationMode::Continuous {
                continue;
            }
            let Some(episode) = session.episode.as_mut() else {
                continue;
            };
            if episode.state != CoreEpisodeState::Unacknowledged {
                continue;
            }
            episode.state = CoreEpisodeState::Acknowledged;
            episode.deadline = None;
            episode.state_changed_at = emitted_at.max(episode.first_observed_at);
            events.push(CoreEvent::EpisodeUpdated(episode.clone()));
            acknowledged.push((session.section.clone(), session.id, episode.id));
        }
        for (section, watch_id, episode_id) in acknowledged {
            self.push_action(
                &mut events,
                CoreActionKind::EpisodesAcknowledgedAll,
                Some(section),
                Some(watch_id),
                Some(episode_id),
            );
        }
        self.push_mixer_if_changed(
            &connection,
            previous_mixer,
            CoreMixerStopReason::NoUnacknowledgedEpisodes,
            &mut events,
        );
        self.connections.insert(connection_id, connection);
        Ok(CoreDispatch {
            connection_id,
            emitted_at,
            events,
        })
    }

    pub(crate) fn resume_episode(
        &mut self,
        connection_id: TraceId,
        episode_id: TraceId,
    ) -> Result<CoreDispatch, CoreManagerError> {
        let now = self.clock.now();
        let mut connection = self.take_connection(connection_id)?;
        let previous_mixer = connection.active_episode_ids();
        let Some(session) = connection.watches.values_mut().find(|session| {
            session
                .episode
                .as_ref()
                .is_some_and(|episode| episode.id == episode_id)
        }) else {
            self.connections.insert(connection_id, connection);
            return Err(CoreManagerError::UnknownEpisode);
        };
        if session.policy.mode != CoreNotificationMode::Continuous
            || session.reliable_state != Some(ReliableState::Open)
            || session.episode.as_ref().map(|episode| episode.state)
                != Some(CoreEpisodeState::TimedOut)
        {
            self.connections.insert(connection_id, connection);
            return Err(CoreManagerError::EpisodeNotResumable);
        }
        let emitted_at = self.clock.wall_now();
        let episode = session.episode.as_mut().expect("checked episode");
        episode.state = CoreEpisodeState::Unacknowledged;
        episode.deadline = session.policy.continuous_duration.deadline(now);
        episode.state_changed_at = emitted_at.max(episode.first_observed_at);
        let mut events = vec![CoreEvent::EpisodeUpdated(episode.clone())];
        self.push_action(
            &mut events,
            CoreActionKind::EpisodeResumed,
            Some(session.section.clone()),
            Some(session.id),
            Some(episode_id),
        );
        self.push_mixer_if_changed(
            &connection,
            previous_mixer,
            CoreMixerStopReason::NoUnacknowledgedEpisodes,
            &mut events,
        );
        self.connections.insert(connection_id, connection);
        Ok(CoreDispatch {
            connection_id,
            emitted_at,
            events,
        })
    }

    pub(crate) fn reset_audible_count(
        &mut self,
        connection_id: TraceId,
        watch_id: TraceId,
    ) -> Result<CoreDispatch, CoreManagerError> {
        let mut connection = self.take_connection(connection_id)?;
        let Some(session) = connection
            .watches
            .values_mut()
            .find(|session| session.id == watch_id)
        else {
            self.connections.insert(connection_id, connection);
            return Err(CoreManagerError::UnknownWatch);
        };
        let emitted_at = self.clock.wall_now();
        session.audible_count = 0;
        if let Some(episode) = session.episode.as_mut() {
            episode.audible_count = 0;
        }
        let mut events = Vec::new();
        if let Some(episode) = session.episode.as_ref() {
            events.push(CoreEvent::EpisodeUpdated(episode.clone()));
        }
        self.push_action(
            &mut events,
            CoreActionKind::AudibleCountReset,
            Some(session.section.clone()),
            Some(watch_id),
            session.episode.as_ref().map(|episode| episode.id),
        );
        self.drive_one_shot(session, &mut events);
        self.connections.insert(connection_id, connection);
        Ok(CoreDispatch {
            connection_id,
            emitted_at,
            events,
        })
    }

    pub(crate) fn report_cue_outcome(
        &mut self,
        connection_id: TraceId,
        cue_id: TraceId,
        outcome: CoreCueOutcome,
    ) -> Result<CoreDispatch, CoreManagerError> {
        let mut connection = self.take_connection(connection_id)?;
        let emitted_at = self.clock.wall_now();
        let section = connection.watches.iter().find_map(|(section, session)| {
            session
                .active_cue
                .as_ref()
                .is_some_and(|cue| cue.id == cue_id)
                .then(|| section.clone())
        });
        let Some(section) = section else {
            self.connections.insert(connection_id, connection);
            return Ok(CoreDispatch {
                connection_id,
                emitted_at,
                events: Vec::new(),
            });
        };
        let session = connection
            .watches
            .get_mut(&section)
            .expect("located watch exists");
        let cue = session
            .active_cue
            .take()
            .expect("located active cue exists");
        if outcome == CoreCueOutcome::Started {
            session.audible_count = session.audible_count.saturating_add(1);
            if let Some(episode) = session.episode.as_mut() {
                episode.audible_count = session.audible_count;
            }
        }
        let mut events = vec![CoreEvent::CueOutcomeRecorded {
            cue_id,
            watch_id: session.id,
            section: session.section.clone(),
            outcome,
            audible_count_consumed: outcome == CoreCueOutcome::Started,
            audible_count: session.audible_count,
        }];
        self.push_action(
            &mut events,
            match outcome {
                CoreCueOutcome::Started => CoreActionKind::CueStarted,
                CoreCueOutcome::Muted => CoreActionKind::CueMuted,
                CoreCueOutcome::AutoplayBlocked => CoreActionKind::CueAutoplayBlocked,
                CoreCueOutcome::Failed => CoreActionKind::CueFailed,
            },
            Some(section),
            Some(session.id),
            Some(cue.episode_id),
        );
        if outcome == CoreCueOutcome::Started
            && let Some(episode) = session.episode.as_ref()
        {
            events.push(CoreEvent::EpisodeUpdated(episode.clone()));
        }
        self.drive_one_shot(session, &mut events);
        self.connections.insert(connection_id, connection);
        Ok(CoreDispatch {
            connection_id,
            emitted_at,
            events,
        })
    }

    pub(crate) fn dismiss_alert(
        &mut self,
        connection_id: TraceId,
        alert_id: TraceId,
    ) -> Result<CoreDispatch, CoreManagerError> {
        let mut connection = self.take_connection(connection_id)?;
        let Some(session) = connection.watches.values_mut().find(|session| {
            session
                .episode
                .as_ref()
                .is_some_and(|episode| episode.alert_id == alert_id)
        }) else {
            self.connections.insert(connection_id, connection);
            return Err(CoreManagerError::UnknownAlert);
        };
        let emitted_at = self.clock.wall_now();
        let episode = session.episode.as_mut().expect("located episode exists");
        episode.alert_dismissed = true;
        let mut events = vec![CoreEvent::AlertUpdated(alert_update(
            &session.section,
            episode,
            CoreAlertChange::Dismissed,
        ))];
        self.push_action(
            &mut events,
            CoreActionKind::AlertDismissed,
            Some(session.section.clone()),
            Some(session.id),
            Some(episode.id),
        );
        self.connections.insert(connection_id, connection);
        Ok(CoreDispatch {
            connection_id,
            emitted_at,
            events,
        })
    }

    pub(crate) fn disconnect(
        &mut self,
        connection_id: TraceId,
    ) -> Result<CoreCleanupReport, CoreManagerError> {
        self.cleanup(connection_id, CoreCleanupReason::ConnectionClosed)
    }

    pub(crate) fn tick(&mut self) -> CoreTickOutcome {
        let now = self.clock.now();
        let emitted_at = self.clock.wall_now();
        let expired = self
            .connections
            .iter()
            .filter_map(|(connection_id, connection)| {
                (now.elapsed_since(connection.last_touch) >= self.heartbeat_timeout)
                    .then_some(*connection_id)
            })
            .collect::<Vec<_>>();
        let mut outcome = CoreTickOutcome::default();
        for connection_id in expired {
            if let Ok(report) = self.cleanup(connection_id, CoreCleanupReason::HeartbeatExpired) {
                outcome.expired_connections.push(report);
            }
        }

        let connection_ids = self.connections.keys().copied().collect::<Vec<_>>();
        for connection_id in connection_ids {
            let Some(mut connection) = self.connections.remove(&connection_id) else {
                continue;
            };
            let previous_mixer = connection.active_episode_ids();
            let mut events = Vec::new();
            let mut timed_out = Vec::new();
            for session in connection.watches.values_mut() {
                if session.policy.mode != CoreNotificationMode::Continuous {
                    continue;
                }
                let Some(episode) = session.episode.as_mut() else {
                    continue;
                };
                if episode.state != CoreEpisodeState::Unacknowledged
                    || episode.deadline.is_none_or(|deadline| deadline > now)
                {
                    continue;
                }
                episode.state = CoreEpisodeState::TimedOut;
                episode.deadline = None;
                episode.state_changed_at = emitted_at.max(episode.first_observed_at);
                events.push(CoreEvent::EpisodeUpdated(episode.clone()));
                timed_out.push((session.section.clone(), session.id, episode.id));
            }
            for (section, watch_id, episode_id) in timed_out {
                self.push_action(
                    &mut events,
                    CoreActionKind::EpisodeTimedOut,
                    Some(section),
                    Some(watch_id),
                    Some(episode_id),
                );
            }
            self.push_mixer_if_changed(
                &connection,
                previous_mixer,
                CoreMixerStopReason::NoUnacknowledgedEpisodes,
                &mut events,
            );
            self.connections.insert(connection_id, connection);
            if !events.is_empty() {
                outcome.dispatches.push(CoreDispatch {
                    connection_id,
                    emitted_at,
                    events,
                });
            }
        }
        outcome
    }

    pub(crate) fn process_stop(&mut self) -> Vec<CoreCleanupReport> {
        let connection_ids = self.connections.keys().copied().collect::<Vec<_>>();
        connection_ids
            .into_iter()
            .filter_map(|connection_id| {
                self.cleanup(connection_id, CoreCleanupReason::ProcessStopped)
                    .ok()
            })
            .collect()
    }

    pub(crate) fn watched_sections(&self, target: &TermCampusKey) -> Vec<SectionKey> {
        self.connections_by_section
            .keys()
            .filter(|section| &section.target() == target)
            .cloned()
            .collect()
    }

    pub(crate) fn active_watch_count(&self, target: &TermCampusKey) -> u64 {
        u64::try_from(self.watched_sections(target).len()).unwrap_or(u64::MAX)
    }

    pub(crate) fn connection_watches(&self, connection_id: TraceId) -> Vec<SectionKey> {
        self.connections
            .get(&connection_id)
            .map(|connection| connection.watches.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// The Sections a connection holds AND the identity of each physical
    /// watch.
    ///
    /// The Section alone cannot tell a caller whether the watch it started is
    /// the one still running: a watch can be stopped and a new one started for
    /// the same Section, and a record compared only by Section would call the
    /// replacement its own.
    pub(crate) fn connection_watch_targets(
        &self,
        connection_id: TraceId,
    ) -> Vec<(SectionKey, TraceId)> {
        self.connections
            .get(&connection_id)
            .map(|connection| {
                connection
                    .watches
                    .iter()
                    .map(|(section, watch)| (section.clone(), watch.id))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(crate) fn watch_id_for_section(
        &self,
        connection_id: TraceId,
        section: &SectionKey,
    ) -> Option<TraceId> {
        self.connections
            .get(&connection_id)?
            .watches
            .get(section)
            .map(|watch| watch.id)
    }

    pub(crate) fn watch_section(
        &self,
        connection_id: TraceId,
        watch_id: TraceId,
    ) -> Option<&SectionKey> {
        self.connections
            .get(&connection_id)?
            .watches
            .values()
            .find(|watch| watch.id == watch_id)
            .map(|watch| &watch.section)
    }

    pub(crate) fn episode_target(
        &self,
        connection_id: TraceId,
        episode_id: TraceId,
    ) -> Option<(TraceId, &SectionKey)> {
        let watch = self
            .connections
            .get(&connection_id)?
            .watches
            .values()
            .find(|watch| {
                watch
                    .episode
                    .as_ref()
                    .is_some_and(|episode| episode.id == episode_id)
            })?;
        Some((watch.id, &watch.section))
    }

    pub(crate) fn alert_target(
        &self,
        connection_id: TraceId,
        alert_id: TraceId,
    ) -> Option<(TraceId, TraceId, &SectionKey)> {
        let watch = self
            .connections
            .get(&connection_id)?
            .watches
            .values()
            .find(|watch| {
                watch
                    .episode
                    .as_ref()
                    .is_some_and(|episode| episode.alert_id == alert_id)
            })?;
        let episode = watch.episode.as_ref()?;
        Some((watch.id, episode.id, &watch.section))
    }

    pub(crate) fn cue_target(
        &self,
        connection_id: TraceId,
        cue_id: TraceId,
    ) -> Option<(TraceId, &SectionKey)> {
        let watch = self
            .connections
            .get(&connection_id)?
            .watches
            .values()
            .find(|watch| {
                watch
                    .active_cue
                    .as_ref()
                    .is_some_and(|cue| cue.id == cue_id)
            })?;
        Some((watch.id, &watch.section))
    }

    fn take_connection(
        &mut self,
        connection_id: TraceId,
    ) -> Result<CoreConnection, CoreManagerError> {
        self.connections
            .remove(&connection_id)
            .ok_or(CoreManagerError::UnknownConnection)
    }

    fn validate_initial_watermarks(&self, specs: &[CoreStartSpec]) -> Result<(), CoreManagerError> {
        let mut candidates = self
            .observation_cursors
            .iter()
            .map(|(section, value)| (section.clone(), value.cursor))
            .collect::<BTreeMap<_, _>>();
        for spec in specs {
            let CoreStartAdmission::Admitted {
                initial_cursor: Some(cursor),
                ..
            } = spec.admission
            else {
                continue;
            };
            if cursor_accepts(
                candidates.get(&spec.section),
                cursor.pull_sequence,
                cursor.observation_id,
            )? {
                candidates.insert(spec.section.clone(), cursor);
            }
        }
        Ok(())
    }

    fn seed_observation_watermark(&mut self, section: SectionKey, cursor: ObservationCursor) {
        if let Some(watermark) = self.observation_cursors.get_mut(&section) {
            if cursor.pull_sequence > watermark.cursor.pull_sequence {
                watermark.cursor = cursor;
                watermark.published = false;
            }
            return;
        }
        self.make_watermark_room(&section);
        self.observation_cursors.insert(
            section,
            SectionObservationWatermark {
                cursor,
                published: false,
            },
        );
    }

    fn record_published_observation(&mut self, section: SectionKey, cursor: ObservationCursor) {
        if let Some(watermark) = self.observation_cursors.get_mut(&section) {
            debug_assert!(cursor.pull_sequence >= watermark.cursor.pull_sequence);
            watermark.cursor = cursor;
            watermark.published = true;
            return;
        }
        self.make_watermark_room(&section);
        self.observation_cursors.insert(
            section,
            SectionObservationWatermark {
                cursor,
                published: true,
            },
        );
    }

    fn make_watermark_room(&mut self, section: &SectionKey) {
        if self.observation_cursors.contains_key(section) {
            return;
        }
        if self.observation_cursors.len() == SECTION_CURSOR_WINDOW
            && let Some(expired) = self.observation_cursor_order.pop_front()
        {
            self.observation_cursors.remove(&expired);
        }
        self.observation_cursor_order.push_back(section.clone());
    }

    fn apply_observation(
        &mut self,
        session: &mut CoreWatchSession,
        observation: &OpenObservationV1,
        now: WatchInstant,
        events: &mut Vec<CoreEvent>,
    ) {
        match observation.state {
            OpenState::Open => {
                let creates_episode = session.reliable_state != Some(ReliableState::Open)
                    || session
                        .episode
                        .as_ref()
                        .is_none_or(|episode| episode.state == CoreEpisodeState::Closed);
                session.reliable_state = Some(ReliableState::Open);
                if creates_episode {
                    self.open_episode(
                        session,
                        observation.observation_id,
                        observation.observed_at,
                        now,
                        events,
                    );
                } else if let Some(episode) = session.episode.as_mut() {
                    episode.observe_open(observation.observation_id, observation.observed_at);
                    events.push(CoreEvent::EpisodeUpdated(episode.clone()));
                    events.push(CoreEvent::AlertUpdated(alert_update(
                        &session.section,
                        episode,
                        CoreAlertChange::Updated,
                    )));
                }
                if session.policy.mode == CoreNotificationMode::OneShot {
                    self.queue_one_shot(session, observation.observation_id, events);
                }
            }
            OpenState::Closed => {
                session.reliable_state = Some(ReliableState::Closed);
                self.end_session(
                    session,
                    CoreEpisodeEndReason::ObservedClosed,
                    observation.observed_at,
                    None,
                    events,
                );
            }
            OpenState::Unknown => {
                // UNKNOWN updates the raw observation visible to the client but never
                // masquerades as a reliable CLOSED transition or rearms an episode.
            }
        }
    }

    fn open_episode(
        &mut self,
        session: &mut CoreWatchSession,
        observation_id: TraceId,
        observed_at: OffsetDateTime,
        now: WatchInstant,
        events: &mut Vec<CoreEvent>,
    ) {
        session.episode_sequence = session.episode_sequence.saturating_add(1);
        let deadline = (session.policy.mode == CoreNotificationMode::Continuous)
            .then(|| session.policy.continuous_duration.deadline(now))
            .flatten();
        let episode = CoreEpisode::new(
            self.ids.next_trace_id(),
            self.ids.next_trace_id(),
            session.id,
            session.section.clone(),
            session.policy,
            session.audible_count,
            session.episode_sequence,
            observation_id,
            observed_at,
            deadline,
        );
        self.push_action(
            events,
            CoreActionKind::EpisodeOpened,
            Some(session.section.clone()),
            Some(session.id),
            Some(episode.id),
        );
        events.push(CoreEvent::EpisodeUpdated(episode.clone()));
        events.push(CoreEvent::AlertUpdated(alert_update(
            &session.section,
            &episode,
            CoreAlertChange::Opened,
        )));
        session.episode = Some(episode);
    }

    fn end_session(
        &mut self,
        session: &mut CoreWatchSession,
        reason: CoreEpisodeEndReason,
        closed_at: OffsetDateTime,
        cancel_reason: Option<CoreCueCancellationReason>,
        events: &mut Vec<CoreEvent>,
    ) {
        if let Some(cancel_reason) = cancel_reason {
            let cues = session.take_pending_cues();
            push_cancelled_cues(events, session.id, &session.section, cues, cancel_reason);
        }
        let Some(episode) = session.episode.as_mut() else {
            return;
        };
        if episode.state == CoreEpisodeState::Closed {
            return;
        }
        episode.state = CoreEpisodeState::Closed;
        episode.deadline = None;
        episode.end_reason = Some(reason);
        let closed_at = closed_at.max(episode.last_observed_at);
        episode.state_changed_at = closed_at.max(episode.first_observed_at);
        episode.closed_at = Some(closed_at);
        events.push(CoreEvent::EpisodeUpdated(episode.clone()));
        events.push(CoreEvent::AlertUpdated(alert_update(
            &session.section,
            episode,
            CoreAlertChange::Closed,
        )));
        self.push_action(
            events,
            CoreActionKind::EpisodeClosed,
            Some(session.section.clone()),
            Some(session.id),
            Some(episode.id),
        );
    }

    fn queue_one_shot(
        &mut self,
        session: &mut CoreWatchSession,
        observation_id: TraceId,
        events: &mut Vec<CoreEvent>,
    ) {
        let max = session.policy.max_audible.get();
        if session.audible_count >= max {
            events.push(CoreEvent::AudioDisposition(CoreAudioDisposition::OneShot {
                watch_id: session.id,
                section: session.section.clone(),
                observation_id,
                cue_id: None,
                disposition: CoreOneShotDisposition::SilentCap,
                audible_count: session.audible_count,
                max_audible: max,
            }));
            return;
        }
        if session.active_cue.is_some() {
            let cue = PendingCue {
                id: self.ids.next_trace_id(),
                observation_id,
                episode_id: session
                    .episode
                    .as_ref()
                    .expect("OPEN observation has an episode")
                    .id,
            };
            session.queued_cues.push_back(cue.clone());
            events.push(CoreEvent::AudioDisposition(CoreAudioDisposition::OneShot {
                watch_id: session.id,
                section: session.section.clone(),
                observation_id,
                cue_id: Some(cue.id),
                disposition: CoreOneShotDisposition::Queued,
                audible_count: session.audible_count,
                max_audible: max,
            }));
            return;
        }
        let cue = PendingCue {
            id: self.ids.next_trace_id(),
            observation_id,
            episode_id: session
                .episode
                .as_ref()
                .expect("OPEN observation has an episode")
                .id,
        };
        self.request_cue(session, cue, events);
    }

    fn request_cue(
        &mut self,
        session: &mut CoreWatchSession,
        cue: PendingCue,
        events: &mut Vec<CoreEvent>,
    ) {
        let cue_id = cue.id;
        let observation_id = cue.observation_id;
        session.active_cue = Some(cue);
        events.push(CoreEvent::AudioDisposition(CoreAudioDisposition::OneShot {
            watch_id: session.id,
            section: session.section.clone(),
            observation_id,
            cue_id: Some(cue_id),
            disposition: CoreOneShotDisposition::Requested,
            audible_count: session.audible_count,
            max_audible: session.policy.max_audible.get(),
        }));
    }

    fn drive_one_shot(&mut self, session: &mut CoreWatchSession, events: &mut Vec<CoreEvent>) {
        if session.active_cue.is_some() {
            return;
        }
        let max = session.policy.max_audible.get();
        if session.audible_count >= max {
            while let Some(cue) = session.queued_cues.pop_front() {
                events.push(CoreEvent::AudioDisposition(CoreAudioDisposition::OneShot {
                    watch_id: session.id,
                    section: session.section.clone(),
                    observation_id: cue.observation_id,
                    cue_id: None,
                    disposition: CoreOneShotDisposition::SilentCap,
                    audible_count: session.audible_count,
                    max_audible: max,
                }));
            }
            return;
        }
        if let Some(cue) = session.queued_cues.pop_front() {
            self.request_cue(session, cue, events);
        }
    }

    fn push_mixer_if_changed(
        &mut self,
        connection: &CoreConnection,
        previous: Vec<TraceId>,
        stop_reason: CoreMixerStopReason,
        events: &mut Vec<CoreEvent>,
    ) {
        let current = connection.active_episode_ids();
        if current != previous {
            let active = !current.is_empty();
            events.push(CoreEvent::AudioDisposition(
                CoreAudioDisposition::ContinuousMixer {
                    active,
                    active_episode_ids: current,
                    stop_reason: (!active).then_some(stop_reason),
                },
            ));
        }
    }

    fn push_action(
        &mut self,
        events: &mut Vec<CoreEvent>,
        kind: CoreActionKind,
        section: Option<SectionKey>,
        watch_id: Option<TraceId>,
        episode_id: Option<TraceId>,
    ) {
        events.push(CoreEvent::Action(CoreAction {
            action_id: self.ids.next_trace_id(),
            kind,
            section,
            watch_id,
            episode_id,
        }));
    }

    fn unlink(&mut self, connection_id: TraceId, section: &SectionKey) {
        let remove_section =
            self.connections_by_section
                .get_mut(section)
                .is_some_and(|connections| {
                    connections.remove(&connection_id);
                    connections.is_empty()
                });
        if remove_section {
            self.connections_by_section.remove(section);
        }
    }

    fn cleanup(
        &mut self,
        connection_id: TraceId,
        reason: CoreCleanupReason,
    ) -> Result<CoreCleanupReport, CoreManagerError> {
        let recorded_at = self.clock.wall_now();
        let connection = self
            .connections
            .remove(&connection_id)
            .ok_or(CoreManagerError::UnknownConnection)?;
        let mut sections = Vec::with_capacity(connection.watches.len());
        let mut actions = Vec::new();
        for (section, mut session) in connection.watches {
            sections.push(section.clone());
            if let Some(episode) = session.episode.as_mut()
                && episode.state != CoreEpisodeState::Closed
            {
                episode.state = CoreEpisodeState::Closed;
                episode.end_reason = Some(reason.episode_reason());
                let closed_at = recorded_at.max(episode.last_observed_at);
                episode.state_changed_at = closed_at.max(episode.first_observed_at);
                episode.closed_at = Some(closed_at);
                actions.push(CoreAction {
                    action_id: self.ids.next_trace_id(),
                    kind: CoreActionKind::EpisodeClosed,
                    section: Some(section.clone()),
                    watch_id: Some(session.id),
                    episode_id: Some(episode.id),
                });
            }
            actions.push(CoreAction {
                action_id: self.ids.next_trace_id(),
                kind: CoreActionKind::WatchStopped,
                section: Some(section.clone()),
                watch_id: Some(session.id),
                episode_id: session.episode.as_ref().map(|episode| episode.id),
            });
            self.unlink(connection_id, &section);
        }
        Ok(CoreCleanupReport {
            connection_id,
            reason,
            recorded_at,
            sections,
            actions,
        })
    }
}

fn cursor_accepts(
    cursor: Option<&ObservationCursor>,
    pull_sequence: u64,
    observation_id: TraceId,
) -> Result<bool, CoreManagerError> {
    let Some(cursor) = cursor else {
        return Ok(true);
    };
    if pull_sequence < cursor.pull_sequence {
        return Ok(false);
    }
    if pull_sequence > cursor.pull_sequence {
        return Ok(true);
    }
    if observation_id == cursor.observation_id {
        Ok(false)
    } else {
        Err(CoreManagerError::ObservationSequenceConflict)
    }
}

fn push_cancelled_cues(
    events: &mut Vec<CoreEvent>,
    watch_id: TraceId,
    section: &SectionKey,
    cues: Vec<PendingCue>,
    reason: CoreCueCancellationReason,
) {
    events.extend(cues.into_iter().map(|cue| {
        CoreEvent::AudioDisposition(CoreAudioDisposition::CueCancelled {
            watch_id,
            section: section.clone(),
            cue_id: cue.id,
            observation_id: cue.observation_id,
            reason,
        })
    }));
}

fn alert_update(
    section: &SectionKey,
    episode: &CoreEpisode,
    change: CoreAlertChange,
) -> CoreAlertUpdate {
    CoreAlertUpdate {
        alert_id: episode.alert_id,
        episode_id: episode.id,
        section: section.clone(),
        change,
        observation_count: episode.observation_count,
        dismissed: episode.alert_dismissed,
        episode: episode.clone(),
    }
}
