use std::collections::BTreeSet;
use std::num::NonZeroU64;
use std::time::Duration;

use bcsp_contracts::{
    ActiveWatchId, ActiveWatchTargetV1, OpenEpisodeId, OpenEpisodeState, OpenEpisodeV1,
    OpenObservationV1, OpenState, SectionKey, TermCampusKey, TraceId, TraceIdSource,
    WATCH_CONTRACT_VERSION, WatchAlertDisposition, WatchAlertId, WatchAlertTargetV1, WatchAlertV1,
    WatchAudioCueV1, WatchAudioDispositionV1, WatchAudioTriggerV1, WatchContinuousDurationV1,
    WatchContinuousMixerStopReason, WatchCueCancellationReason, WatchCueId, WatchCueOutcome,
    WatchCueOutcomeReceiptV1, WatchCueOutcomeReportV1, WatchEpisodeTargetV1, WatchMaxAudible,
    WatchNotificationMode, WatchOpenObservationFanoutV1, WatchPolicyV1, WatchServerEventV1,
    WatchStartItemResultV1, WatchStartItemsV1, WatchStartRejectionReason, WatchStartResultV1,
    WatchStopReason, WatchStoppedV1,
};
use thiserror::Error;
use time::OffsetDateTime;

use crate::WatchClock;
use crate::effect::{
    CoreAction, CoreActionKind, CoreAlertChange, CoreAudioDisposition, CoreCleanupReason,
    CoreCleanupReport, CoreCueCancellationReason, CoreCueOutcome, CoreDispatch, CoreEvent,
    CoreMixerStopReason, CoreOneShotDisposition,
};
use crate::manager::{CoreManagerError, CoreStartAdmission, CoreStartSpec, CoreWatchManager};
use crate::state::{
    CoreContinuousDuration, CoreEpisode, CoreEpisodeState, CoreInitialState, CoreNotificationMode,
    CorePolicy, ObservationCursor,
};

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum WatchManagerError {
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
    #[error("command target does not match the active connection state")]
    TargetMismatch,
    #[error("initial observation does not match its start Section")]
    InitialObservationMismatch,
    #[error("internal watch state could not be represented by the wire contract")]
    InvalidContractProjection,
    /// The socket has been sealed for shutdown. Nothing may open a connection
    /// on it again, including a process-owned one.
    #[error("the shared watch socket is sealed")]
    SocketSealed,
}

impl From<CoreManagerError> for WatchManagerError {
    fn from(value: CoreManagerError) -> Self {
        match value {
            CoreManagerError::InvalidHeartbeatTimeout => Self::InvalidHeartbeatTimeout,
            CoreManagerError::ConnectionAlreadyExists => Self::ConnectionAlreadyExists,
            CoreManagerError::UnknownConnection => Self::UnknownConnection,
            CoreManagerError::ObservationSequenceConflict => Self::ObservationSequenceConflict,
            CoreManagerError::UnknownWatch => Self::UnknownWatch,
            CoreManagerError::UnknownEpisode => Self::UnknownEpisode,
            CoreManagerError::UnknownAlert => Self::UnknownAlert,
            CoreManagerError::EpisodeNotResumable => Self::EpisodeNotResumable,
            CoreManagerError::InvalidEpisodeAction => Self::InvalidEpisodeAction,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WatchStartAdmission {
    Admitted {
        current_observation: Option<Box<OpenObservationV1>>,
    },
    SectionNotFound,
    TargetUnavailable,
    UnsupportedTarget,
    TermOutOfRange,
}

impl WatchStartAdmission {
    pub fn admitted(current_observation: Option<OpenObservationV1>) -> Self {
        Self::Admitted {
            current_observation: current_observation.map(Box::new),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatchActionKind {
    WatchStarted,
    WatchRestarted,
    WatchStopped,
    EpisodeOpened,
    EpisodeAcknowledged,
    EpisodesAcknowledgedAll,
    EpisodeTimedOut,
    EpisodeResumed,
    EpisodeClosed,
    AudibleCountReset,
    CueStarted,
    CueMuted,
    CueAutoplayBlocked,
    CueFailed,
    AlertDismissed,
    PolicyUpdated,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchAction {
    pub action_id: TraceId,
    pub kind: WatchActionKind,
    pub section_key: Option<SectionKey>,
    pub active_watch_id: Option<ActiveWatchId>,
    pub episode_id: Option<OpenEpisodeId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchDispatch {
    pub connection_id: TraceId,
    pub emitted_at: OffsetDateTime,
    pub events: Vec<WatchServerEventV1>,
    pub actions: Vec<WatchAction>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchStartOutcome {
    pub dispatch: WatchDispatch,
    pub replayed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchPublishOutcome {
    pub dispatches: Vec<WatchDispatch>,
    pub replayed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WatchCleanupReason {
    ConnectionClosed,
    HeartbeatExpired,
    ProcessStopped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WatchCleanupReport {
    pub connection_id: TraceId,
    pub reason: WatchCleanupReason,
    pub recorded_at: OffsetDateTime,
    pub sections: Vec<SectionKey>,
    pub actions: Vec<WatchAction>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WatchTickOutcome {
    pub expired_connections: Vec<WatchCleanupReport>,
    pub dispatches: Vec<WatchDispatch>,
}

/// Connection-bound shared watch manager. It owns no storage and restores no active state.
pub struct WatchManager<C, I> {
    core: CoreWatchManager<C, I>,
}

impl<C, I> WatchManager<C, I>
where
    C: WatchClock,
    I: TraceIdSource,
{
    pub fn try_new(
        clock: C,
        ids: I,
        heartbeat_timeout: Duration,
    ) -> Result<Self, WatchManagerError> {
        Ok(Self {
            core: CoreWatchManager::try_new(clock, ids, heartbeat_timeout)?,
        })
    }

    pub fn clock_mut(&mut self) -> &mut C {
        self.core.clock_mut()
    }

    pub fn connection_count(&self) -> usize {
        self.core.connection_count()
    }

    pub fn connect(&mut self, connection_id: TraceId) -> Result<(), WatchManagerError> {
        Ok(self.core.connect(connection_id)?)
    }

    pub fn touch(&mut self, connection_id: TraceId) -> Result<(), WatchManagerError> {
        Ok(self.core.touch(connection_id)?)
    }

    pub fn start(
        &mut self,
        connection_id: TraceId,
        message_id: TraceId,
        items: WatchStartItemsV1,
        current_observations: &[OpenObservationV1],
    ) -> Result<WatchStartOutcome, WatchManagerError> {
        let mut sections = BTreeSet::new();
        if current_observations
            .iter()
            .any(|value| !sections.insert(value.section_key().clone()))
        {
            return Err(WatchManagerError::InitialObservationMismatch);
        }
        self.start_with_admission(connection_id, message_id, items, |section| {
            WatchStartAdmission::admitted(
                current_observations
                    .iter()
                    .find(|value| value.section_key() == section)
                    .cloned(),
            )
        })
    }

    pub fn start_without_current(
        &mut self,
        connection_id: TraceId,
        message_id: TraceId,
        items: WatchStartItemsV1,
    ) -> Result<WatchStartOutcome, WatchManagerError> {
        self.start_with_admission(connection_id, message_id, items, |_| {
            WatchStartAdmission::admitted(None)
        })
    }

    pub fn start_with_current<F>(
        &mut self,
        connection_id: TraceId,
        message_id: TraceId,
        items: WatchStartItemsV1,
        mut current_observation: F,
    ) -> Result<WatchStartOutcome, WatchManagerError>
    where
        F: FnMut(&SectionKey) -> Option<OpenObservationV1>,
    {
        self.start_with_admission(connection_id, message_id, items, |section| {
            WatchStartAdmission::admitted(current_observation(section))
        })
    }

    pub fn start_with_admission<F>(
        &mut self,
        connection_id: TraceId,
        message_id: TraceId,
        items: WatchStartItemsV1,
        mut admission_for: F,
    ) -> Result<WatchStartOutcome, WatchManagerError>
    where
        F: FnMut(&SectionKey) -> WatchStartAdmission,
    {
        let start_wall_now = self.core.clock_mut().wall_now();
        let mut specs = Vec::with_capacity(items.as_slice().len());
        for item in items.into_inner() {
            let admission = admission_for(&item.section_key);
            let current = match admission {
                WatchStartAdmission::Admitted {
                    current_observation,
                } => current_observation.map(|value| *value),
                WatchStartAdmission::SectionNotFound => {
                    specs.push(CoreStartSpec {
                        section: item.section_key,
                        policy: core_policy(&item.policy),
                        admission: CoreStartAdmission::SectionNotFound,
                    });
                    continue;
                }
                WatchStartAdmission::TargetUnavailable => {
                    specs.push(CoreStartSpec {
                        section: item.section_key,
                        policy: core_policy(&item.policy),
                        admission: CoreStartAdmission::TargetUnavailable,
                    });
                    continue;
                }
                WatchStartAdmission::UnsupportedTarget => {
                    specs.push(CoreStartSpec {
                        section: item.section_key,
                        policy: core_policy(&item.policy),
                        admission: CoreStartAdmission::UnsupportedTarget,
                    });
                    continue;
                }
                WatchStartAdmission::TermOutOfRange => {
                    specs.push(CoreStartSpec {
                        section: item.section_key,
                        policy: core_policy(&item.policy),
                        admission: CoreStartAdmission::TermOutOfRange,
                    });
                    continue;
                }
            };
            if current
                .as_ref()
                .is_some_and(|value| value.section_key() != &item.section_key)
            {
                return Err(WatchManagerError::InitialObservationMismatch);
            }
            let initial_cursor = current.as_ref().map(|value| ObservationCursor {
                pull_sequence: value.pull_sequence.get(),
                observation_id: value.observation_id,
            });
            let initial_state = match current.as_ref() {
                Some(value)
                    if value.fresh_until >= start_wall_now && value.state == OpenState::Open =>
                {
                    CoreInitialState::Open {
                        observation_id: value.observation_id,
                        observed_at: value.observed_at,
                    }
                }
                Some(value)
                    if value.fresh_until >= start_wall_now && value.state == OpenState::Closed =>
                {
                    CoreInitialState::Closed
                }
                _ => CoreInitialState::Unknown,
            };
            specs.push(CoreStartSpec {
                section: item.section_key,
                policy: core_policy(&item.policy),
                admission: CoreStartAdmission::Admitted {
                    initial_state,
                    initial_cursor,
                },
            });
        }
        let outcome = self.core.start(connection_id, message_id, specs)?;
        Ok(WatchStartOutcome {
            dispatch: project_dispatch(
                &self.core,
                outcome.dispatch,
                Some(outcome.active_watch_count),
                WatchStopReason::UserRequested,
            )?,
            replayed: outcome.replayed,
        })
    }

    pub fn stop_watch(
        &mut self,
        connection_id: TraceId,
        target: ActiveWatchTargetV1,
    ) -> Result<WatchDispatch, WatchManagerError> {
        self.ensure_watch_target(connection_id, &target)?;
        let dispatch = self
            .core
            .stop_watch(connection_id, target.active_watch_id.trace_id())?;
        project_dispatch(&self.core, dispatch, None, WatchStopReason::UserRequested)
    }

    pub fn stop_section_with_reason(
        &mut self,
        connection_id: TraceId,
        section: &SectionKey,
        reason: WatchStopReason,
    ) -> Result<WatchDispatch, WatchManagerError> {
        let watch_id = self
            .core
            .watch_id_for_section(connection_id, section)
            .ok_or(WatchManagerError::UnknownWatch)?;
        let dispatch = self.core.stop_watch(connection_id, watch_id)?;
        project_dispatch(&self.core, dispatch, None, reason)
    }

    pub fn update_policy(
        &mut self,
        connection_id: TraceId,
        target: ActiveWatchTargetV1,
        policy: WatchPolicyV1,
    ) -> Result<WatchDispatch, WatchManagerError> {
        self.ensure_watch_target(connection_id, &target)?;
        let dispatch = self.core.update_policy(
            connection_id,
            target.active_watch_id.trace_id(),
            core_policy(&policy),
        )?;
        project_dispatch(&self.core, dispatch, None, WatchStopReason::UserRequested)
    }

    pub fn acknowledge_episode(
        &mut self,
        connection_id: TraceId,
        target: WatchEpisodeTargetV1,
    ) -> Result<WatchDispatch, WatchManagerError> {
        self.ensure_episode_target(connection_id, &target)?;
        let dispatch = self
            .core
            .acknowledge_episode(connection_id, target.episode_id.trace_id())?;
        project_dispatch(&self.core, dispatch, None, WatchStopReason::UserRequested)
    }

    pub fn acknowledge_all(
        &mut self,
        connection_id: TraceId,
    ) -> Result<WatchDispatch, WatchManagerError> {
        let dispatch = self.core.acknowledge_all(connection_id)?;
        project_dispatch(&self.core, dispatch, None, WatchStopReason::UserRequested)
    }

    pub fn acknowledge_all_episodes(
        &mut self,
        connection_id: TraceId,
    ) -> Result<WatchDispatch, WatchManagerError> {
        self.acknowledge_all(connection_id)
    }

    pub fn resume_episode(
        &mut self,
        connection_id: TraceId,
        target: WatchEpisodeTargetV1,
    ) -> Result<WatchDispatch, WatchManagerError> {
        self.ensure_episode_target(connection_id, &target)?;
        let dispatch = self
            .core
            .resume_episode(connection_id, target.episode_id.trace_id())?;
        project_dispatch(&self.core, dispatch, None, WatchStopReason::UserRequested)
    }

    pub fn resume_timed_out_episode(
        &mut self,
        connection_id: TraceId,
        target: WatchEpisodeTargetV1,
    ) -> Result<WatchDispatch, WatchManagerError> {
        self.resume_episode(connection_id, target)
    }

    pub fn reset_audible_count(
        &mut self,
        connection_id: TraceId,
        target: ActiveWatchTargetV1,
    ) -> Result<WatchDispatch, WatchManagerError> {
        self.ensure_watch_target(connection_id, &target)?;
        let dispatch = self
            .core
            .reset_audible_count(connection_id, target.active_watch_id.trace_id())?;
        project_dispatch(&self.core, dispatch, None, WatchStopReason::UserRequested)
    }

    pub fn report_cue_outcome(
        &mut self,
        connection_id: TraceId,
        report: WatchCueOutcomeReportV1,
    ) -> Result<WatchDispatch, WatchManagerError> {
        let actual = self
            .core
            .cue_target(connection_id, report.cue_id.trace_id())
            .ok_or(WatchManagerError::TargetMismatch)?;
        if actual.0 != report.active_watch_id.trace_id() || actual.1 != &report.section_key {
            return Err(WatchManagerError::TargetMismatch);
        }
        let dispatch = self.core.report_cue_outcome(
            connection_id,
            report.cue_id.trace_id(),
            core_cue_outcome(report.outcome),
        )?;
        project_dispatch(&self.core, dispatch, None, WatchStopReason::UserRequested)
    }

    pub fn dismiss_alert(
        &mut self,
        connection_id: TraceId,
        target: WatchAlertTargetV1,
    ) -> Result<WatchDispatch, WatchManagerError> {
        let actual = self
            .core
            .alert_target(connection_id, target.alert_id.trace_id())
            .ok_or(WatchManagerError::TargetMismatch)?;
        if actual.0 != target.active_watch_id.trace_id()
            || actual.1 != target.episode_id.trace_id()
            || actual.2 != &target.section_key
        {
            return Err(WatchManagerError::TargetMismatch);
        }
        let dispatch = self
            .core
            .dismiss_alert(connection_id, target.alert_id.trace_id())?;
        project_dispatch(&self.core, dispatch, None, WatchStopReason::UserRequested)
    }

    pub fn publish(
        &mut self,
        observation: OpenObservationV1,
    ) -> Result<WatchPublishOutcome, WatchManagerError> {
        let outcome = self.core.publish(observation)?;
        let dispatches = outcome
            .dispatches
            .into_iter()
            .map(|dispatch| {
                project_dispatch(&self.core, dispatch, None, WatchStopReason::UserRequested)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(WatchPublishOutcome {
            dispatches,
            replayed: outcome.replayed,
        })
    }

    pub fn disconnect(
        &mut self,
        connection_id: TraceId,
    ) -> Result<WatchCleanupReport, WatchManagerError> {
        Ok(project_cleanup(self.core.disconnect(connection_id)?))
    }

    pub fn tick(&mut self) -> Result<WatchTickOutcome, WatchManagerError> {
        let outcome = self.core.tick();
        Ok(WatchTickOutcome {
            expired_connections: outcome
                .expired_connections
                .into_iter()
                .map(project_cleanup)
                .collect(),
            dispatches: outcome
                .dispatches
                .into_iter()
                .map(|dispatch| {
                    project_dispatch(
                        &self.core,
                        dispatch,
                        None,
                        WatchStopReason::HeartbeatTimeout,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }

    pub fn process_stop(&mut self) -> Vec<WatchCleanupReport> {
        self.core
            .process_stop()
            .into_iter()
            .map(project_cleanup)
            .collect()
    }

    pub fn watched_sections(&self, target: &TermCampusKey) -> Vec<SectionKey> {
        self.core.watched_sections(target)
    }

    pub fn active_demand_count(&self, target: &TermCampusKey) -> u64 {
        self.core.active_watch_count(target)
    }

    pub fn active_watch_count(&self, target: &TermCampusKey) -> u64 {
        self.active_demand_count(target)
    }

    pub fn connection_watches(&self, connection_id: TraceId) -> Vec<SectionKey> {
        self.core.connection_watches(connection_id)
    }

    /// The addressable identity of every watch a connection currently holds.
    ///
    /// A caller that remembers which watch it started needs the identity, not
    /// just the Section: a Section that is watched again under a NEW watch id
    /// is not the watch the caller is tracking, and treating it as one would
    /// report a replacement as the original -- or stop the replacement while
    /// meaning to stop something that is already gone.
    pub fn connection_watch_targets(&self, connection_id: TraceId) -> Vec<ActiveWatchTargetV1> {
        self.core
            .connection_watch_targets(connection_id)
            .into_iter()
            .map(|(section_key, watch_id)| ActiveWatchTargetV1 {
                active_watch_id: ActiveWatchId::new(watch_id),
                section_key,
            })
            .collect()
    }

    fn ensure_watch_target(
        &self,
        connection_id: TraceId,
        target: &ActiveWatchTargetV1,
    ) -> Result<(), WatchManagerError> {
        match self
            .core
            .watch_section(connection_id, target.active_watch_id.trace_id())
        {
            Some(section) if section == &target.section_key => Ok(()),
            _ => Err(WatchManagerError::TargetMismatch),
        }
    }

    fn ensure_episode_target(
        &self,
        connection_id: TraceId,
        target: &WatchEpisodeTargetV1,
    ) -> Result<(), WatchManagerError> {
        match self
            .core
            .episode_target(connection_id, target.episode_id.trace_id())
        {
            Some((watch_id, section))
                if watch_id == target.active_watch_id.trace_id()
                    && section == &target.section_key =>
            {
                Ok(())
            }
            _ => Err(WatchManagerError::TargetMismatch),
        }
    }
}

fn core_policy(value: &WatchPolicyV1) -> CorePolicy {
    CorePolicy {
        mode: match value.notification_mode {
            WatchNotificationMode::OneShot => CoreNotificationMode::OneShot,
            WatchNotificationMode::Continuous => CoreNotificationMode::Continuous,
        },
        max_audible: NonZeroU64::new(value.max_audible.get()).expect("WatchMaxAudible is positive"),
        continuous_duration: match value.continuous_duration {
            WatchContinuousDurationV1::Finite { seconds } => {
                CoreContinuousDuration::Finite(seconds)
            }
            WatchContinuousDurationV1::Unlimited => CoreContinuousDuration::Unlimited,
        },
    }
}

fn wire_policy(value: CorePolicy) -> WatchPolicyV1 {
    WatchPolicyV1::new(
        match value.mode {
            CoreNotificationMode::OneShot => WatchNotificationMode::OneShot,
            CoreNotificationMode::Continuous => WatchNotificationMode::Continuous,
        },
        WatchMaxAudible::try_from(value.max_audible.get()).expect("core max audible is positive"),
        match value.continuous_duration {
            CoreContinuousDuration::Finite(seconds) => {
                WatchContinuousDurationV1::Finite { seconds }
            }
            CoreContinuousDuration::Unlimited => WatchContinuousDurationV1::Unlimited,
        },
    )
}

fn core_cue_outcome(value: WatchCueOutcome) -> CoreCueOutcome {
    match value {
        WatchCueOutcome::Started => CoreCueOutcome::Started,
        WatchCueOutcome::Muted => CoreCueOutcome::Muted,
        WatchCueOutcome::AutoplayBlocked => CoreCueOutcome::AutoplayBlocked,
        WatchCueOutcome::Failed => CoreCueOutcome::Failed,
    }
}

fn project_dispatch<C, I>(
    core: &CoreWatchManager<C, I>,
    dispatch: CoreDispatch,
    start_active_count: Option<u8>,
    stop_reason: WatchStopReason,
) -> Result<WatchDispatch, WatchManagerError>
where
    C: WatchClock,
    I: TraceIdSource,
{
    let mut events = Vec::new();
    let mut actions = Vec::new();
    let mut starts = Vec::new();
    for event in dispatch.events {
        match event {
            CoreEvent::StartResult(value) => starts.push(match value.disposition {
                crate::effect::CoreStartDisposition::Started
                | crate::effect::CoreStartDisposition::Restarted => {
                    WatchStartItemResultV1::Active {
                        section_key: value.section,
                        active_watch_id: ActiveWatchId::new(
                            value
                                .watch_id
                                .ok_or(WatchManagerError::InvalidContractProjection)?,
                        ),
                        started_at: dispatch.emitted_at,
                    }
                }
                crate::effect::CoreStartDisposition::RejectedLimit => {
                    WatchStartItemResultV1::Rejected {
                        section_key: value.section,
                        reason: WatchStartRejectionReason::MaxActiveWatches,
                    }
                }
                crate::effect::CoreStartDisposition::RejectedDuplicate => {
                    WatchStartItemResultV1::Rejected {
                        section_key: value.section,
                        reason: WatchStartRejectionReason::AlreadyActive,
                    }
                }
                crate::effect::CoreStartDisposition::RejectedSectionNotFound => {
                    WatchStartItemResultV1::Rejected {
                        section_key: value.section,
                        reason: WatchStartRejectionReason::SectionNotFound,
                    }
                }
                crate::effect::CoreStartDisposition::RejectedTargetUnavailable => {
                    WatchStartItemResultV1::Rejected {
                        section_key: value.section,
                        reason: WatchStartRejectionReason::TargetUnavailable,
                    }
                }
                crate::effect::CoreStartDisposition::RejectedUnsupportedTarget => {
                    WatchStartItemResultV1::Rejected {
                        section_key: value.section,
                        reason: WatchStartRejectionReason::UnsupportedTarget,
                    }
                }
                crate::effect::CoreStartDisposition::RejectedTermOutOfRange => {
                    WatchStartItemResultV1::Rejected {
                        section_key: value.section,
                        reason: WatchStartRejectionReason::TermOutOfRange,
                    }
                }
            }),
            CoreEvent::WatchStopped { section, watch_id } => {
                events.push(WatchServerEventV1::WatchStopped {
                    stopped: WatchStoppedV1 {
                        contract_version: WATCH_CONTRACT_VERSION,
                        active_watch_id: ActiveWatchId::new(watch_id),
                        section_key: section,
                        reason: stop_reason,
                        stopped_at: dispatch.emitted_at,
                    },
                });
            }
            CoreEvent::Observation(observation) => {
                let watch_id = core
                    .watch_id_for_section(dispatch.connection_id, observation.section_key())
                    .ok_or(WatchManagerError::InvalidContractProjection)?;
                events.push(WatchServerEventV1::OpenObservation {
                    fanout: WatchOpenObservationFanoutV1 {
                        contract_version: WATCH_CONTRACT_VERSION,
                        active_watch_id: ActiveWatchId::new(watch_id),
                        observation,
                    },
                });
            }
            CoreEvent::EpisodeUpdated(episode) => {
                events.push(WatchServerEventV1::EpisodeUpdated {
                    episode: project_episode(episode)?,
                });
            }
            CoreEvent::AlertUpdated(alert) => {
                events.push(WatchServerEventV1::AlertUpdated {
                    alert: WatchAlertV1 {
                        contract_version: WATCH_CONTRACT_VERSION,
                        alert_id: WatchAlertId::new(alert.alert_id),
                        disposition: match alert.change {
                            CoreAlertChange::Opened => WatchAlertDisposition::Opened,
                            CoreAlertChange::Updated => WatchAlertDisposition::Updated,
                            CoreAlertChange::Dismissed => WatchAlertDisposition::Dismissed,
                            CoreAlertChange::Closed => WatchAlertDisposition::Closed,
                        },
                        visible: !alert.dismissed && alert.change != CoreAlertChange::Closed,
                        episode: project_episode(alert.episode)?,
                    },
                });
            }
            CoreEvent::AudioDisposition(audio) => {
                events.push(WatchServerEventV1::AudioDisposition {
                    audio: project_audio(audio, dispatch.emitted_at)?,
                });
            }
            CoreEvent::CueOutcomeRecorded {
                cue_id,
                watch_id,
                section,
                outcome,
                audible_count_consumed,
                audible_count,
            } => {
                events.push(WatchServerEventV1::CueOutcomeRecorded {
                    receipt: WatchCueOutcomeReceiptV1 {
                        contract_version: WATCH_CONTRACT_VERSION,
                        cue_id: WatchCueId::new(cue_id),
                        active_watch_id: ActiveWatchId::new(watch_id),
                        section_key: section,
                        outcome: match outcome {
                            CoreCueOutcome::Started => WatchCueOutcome::Started,
                            CoreCueOutcome::Muted => WatchCueOutcome::Muted,
                            CoreCueOutcome::AutoplayBlocked => WatchCueOutcome::AutoplayBlocked,
                            CoreCueOutcome::Failed => WatchCueOutcome::Failed,
                        },
                        audible_count_consumed,
                        audible_count,
                        recorded_at: dispatch.emitted_at,
                    },
                });
            }
            CoreEvent::Action(action) => actions.push(project_action(action)),
        }
    }
    if !starts.is_empty() {
        let active_watch_count =
            start_active_count.ok_or(WatchManagerError::InvalidContractProjection)?;
        events.insert(
            0,
            WatchServerEventV1::StartResult {
                result: WatchStartResultV1::try_new(starts, active_watch_count)
                    .map_err(|_| WatchManagerError::InvalidContractProjection)?,
            },
        );
    }
    Ok(WatchDispatch {
        connection_id: dispatch.connection_id,
        emitted_at: dispatch.emitted_at,
        events,
        actions,
    })
}

fn project_episode(value: CoreEpisode) -> Result<OpenEpisodeV1, WatchManagerError> {
    OpenEpisodeV1::try_new(
        OpenEpisodeId::new(value.id),
        ActiveWatchId::new(value.watch_id),
        value.section,
        match value.state {
            CoreEpisodeState::Unacknowledged => OpenEpisodeState::Unacknowledged,
            CoreEpisodeState::Acknowledged => OpenEpisodeState::Acknowledged,
            CoreEpisodeState::TimedOut => OpenEpisodeState::TimedOut,
            CoreEpisodeState::Closed => OpenEpisodeState::Closed,
        },
        &wire_policy(value.policy),
        value.audible_count,
        value.first_observed_at,
        value.last_observed_at,
        NonZeroU64::new(value.observation_count)
            .ok_or(WatchManagerError::InvalidContractProjection)?,
        value.last_observation_id,
        value.state_changed_at,
        value.closed_at,
    )
    .map_err(|_| WatchManagerError::InvalidContractProjection)
}

fn project_audio(
    value: CoreAudioDisposition,
    emitted_at: OffsetDateTime,
) -> Result<WatchAudioDispositionV1, WatchManagerError> {
    Ok(match value {
        CoreAudioDisposition::OneShot {
            watch_id,
            section,
            observation_id,
            cue_id,
            disposition,
            audible_count,
            max_audible,
        } => match disposition {
            CoreOneShotDisposition::Requested => WatchAudioDispositionV1::CueRequested {
                cue: WatchAudioCueV1 {
                    cue_id: WatchCueId::new(
                        cue_id.ok_or(WatchManagerError::InvalidContractProjection)?,
                    ),
                    active_watch_id: ActiveWatchId::new(watch_id),
                    section_key: section,
                    trigger: WatchAudioTriggerV1::OneShotObservation { observation_id },
                    emitted_at,
                },
            },
            CoreOneShotDisposition::Queued => WatchAudioDispositionV1::CueQueued {
                active_watch_id: ActiveWatchId::new(watch_id),
                section_key: section,
                observation_id,
                queued_at: emitted_at,
            },
            CoreOneShotDisposition::SilentCap => WatchAudioDispositionV1::SilentMaxAudible {
                active_watch_id: ActiveWatchId::new(watch_id),
                section_key: section,
                observation_id,
                audible_count,
                max_audible: WatchMaxAudible::try_from(max_audible)
                    .map_err(|_| WatchManagerError::InvalidContractProjection)?,
                emitted_at,
            },
        },
        CoreAudioDisposition::CueCancelled {
            watch_id,
            section,
            cue_id,
            observation_id,
            reason,
        } => WatchAudioDispositionV1::CueCancelled {
            active_watch_id: ActiveWatchId::new(watch_id),
            section_key: section,
            observation_id,
            cue_id: Some(WatchCueId::new(cue_id)),
            reason: match reason {
                CoreCueCancellationReason::WatchStopped => WatchCueCancellationReason::WatchStopped,
                CoreCueCancellationReason::WatchRestarted => {
                    WatchCueCancellationReason::WatchRestarted
                }
                CoreCueCancellationReason::NotificationModeChanged => {
                    WatchCueCancellationReason::NotificationModeChanged
                }
            },
            cancelled_at: emitted_at,
        },
        CoreAudioDisposition::ContinuousMixer {
            active,
            active_episode_ids,
            stop_reason,
        } => {
            if active {
                WatchAudioDispositionV1::ContinuousMixerActive {
                    episode_ids: active_episode_ids
                        .into_iter()
                        .map(OpenEpisodeId::new)
                        .collect::<BTreeSet<_>>(),
                    emitted_at,
                }
            } else {
                WatchAudioDispositionV1::ContinuousMixerStopped {
                    reason: match stop_reason.ok_or(WatchManagerError::InvalidContractProjection)? {
                        CoreMixerStopReason::NoUnacknowledgedEpisodes => {
                            WatchContinuousMixerStopReason::NoUnacknowledgedEpisodes
                        }
                        CoreMixerStopReason::NotificationModeChanged => {
                            WatchContinuousMixerStopReason::NotificationModeChanged
                        }
                    },
                    emitted_at,
                }
            }
        }
    })
}

fn project_action(value: CoreAction) -> WatchAction {
    WatchAction {
        action_id: value.action_id,
        kind: match value.kind {
            CoreActionKind::WatchStarted => WatchActionKind::WatchStarted,
            CoreActionKind::WatchRestarted => WatchActionKind::WatchRestarted,
            CoreActionKind::WatchStopped => WatchActionKind::WatchStopped,
            CoreActionKind::EpisodeOpened => WatchActionKind::EpisodeOpened,
            CoreActionKind::EpisodeAcknowledged => WatchActionKind::EpisodeAcknowledged,
            CoreActionKind::EpisodesAcknowledgedAll => WatchActionKind::EpisodesAcknowledgedAll,
            CoreActionKind::EpisodeTimedOut => WatchActionKind::EpisodeTimedOut,
            CoreActionKind::EpisodeResumed => WatchActionKind::EpisodeResumed,
            CoreActionKind::EpisodeClosed => WatchActionKind::EpisodeClosed,
            CoreActionKind::AudibleCountReset => WatchActionKind::AudibleCountReset,
            CoreActionKind::CueStarted => WatchActionKind::CueStarted,
            CoreActionKind::CueMuted => WatchActionKind::CueMuted,
            CoreActionKind::CueAutoplayBlocked => WatchActionKind::CueAutoplayBlocked,
            CoreActionKind::CueFailed => WatchActionKind::CueFailed,
            CoreActionKind::AlertDismissed => WatchActionKind::AlertDismissed,
            CoreActionKind::PolicyUpdated => WatchActionKind::PolicyUpdated,
        },
        section_key: value.section,
        active_watch_id: value.watch_id.map(ActiveWatchId::new),
        episode_id: value.episode_id.map(OpenEpisodeId::new),
    }
}

fn project_cleanup(value: CoreCleanupReport) -> WatchCleanupReport {
    WatchCleanupReport {
        connection_id: value.connection_id,
        reason: match value.reason {
            CoreCleanupReason::ConnectionClosed => WatchCleanupReason::ConnectionClosed,
            CoreCleanupReason::HeartbeatExpired => WatchCleanupReason::HeartbeatExpired,
            CoreCleanupReason::ProcessStopped => WatchCleanupReason::ProcessStopped,
        },
        recorded_at: value.recorded_at,
        sections: value.sections,
        actions: value.actions.into_iter().map(project_action).collect(),
    }
}
