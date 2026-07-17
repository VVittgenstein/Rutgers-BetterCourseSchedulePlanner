use bcsp_contracts::{OpenObservationV1, SectionKey, TraceId};
use time::OffsetDateTime;

use crate::state::{CoreEpisode, CoreEpisodeEndReason};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreStartDisposition {
    Started,
    Restarted,
    RejectedLimit,
    RejectedDuplicate,
    RejectedSectionNotFound,
    RejectedTargetUnavailable,
    RejectedUnsupportedTarget,
    RejectedTermOutOfRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreStartResult {
    pub(crate) section: SectionKey,
    pub(crate) disposition: CoreStartDisposition,
    pub(crate) watch_id: Option<TraceId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreAlertChange {
    Opened,
    Updated,
    Dismissed,
    Closed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreAlertUpdate {
    pub(crate) alert_id: TraceId,
    pub(crate) episode_id: TraceId,
    pub(crate) section: SectionKey,
    pub(crate) change: CoreAlertChange,
    pub(crate) observation_count: u64,
    pub(crate) dismissed: bool,
    pub(crate) episode: CoreEpisode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreCueOutcome {
    Started,
    Muted,
    AutoplayBlocked,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreOneShotDisposition {
    Requested,
    Queued,
    SilentCap,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreCueCancellationReason {
    WatchStopped,
    WatchRestarted,
    NotificationModeChanged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreMixerStopReason {
    NoUnacknowledgedEpisodes,
    NotificationModeChanged,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CoreAudioDisposition {
    OneShot {
        watch_id: TraceId,
        section: SectionKey,
        observation_id: TraceId,
        cue_id: Option<TraceId>,
        disposition: CoreOneShotDisposition,
        audible_count: u64,
        max_audible: u64,
    },
    CueCancelled {
        watch_id: TraceId,
        section: SectionKey,
        cue_id: TraceId,
        observation_id: TraceId,
        reason: CoreCueCancellationReason,
    },
    ContinuousMixer {
        active: bool,
        active_episode_ids: Vec<TraceId>,
        stop_reason: Option<CoreMixerStopReason>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreActionKind {
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
pub(crate) struct CoreAction {
    pub(crate) action_id: TraceId,
    pub(crate) kind: CoreActionKind,
    pub(crate) section: Option<SectionKey>,
    pub(crate) watch_id: Option<TraceId>,
    pub(crate) episode_id: Option<TraceId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CoreEvent {
    StartResult(CoreStartResult),
    WatchStopped {
        section: SectionKey,
        watch_id: TraceId,
    },
    Observation(OpenObservationV1),
    EpisodeUpdated(CoreEpisode),
    AlertUpdated(CoreAlertUpdate),
    AudioDisposition(CoreAudioDisposition),
    CueOutcomeRecorded {
        cue_id: TraceId,
        watch_id: TraceId,
        section: SectionKey,
        outcome: CoreCueOutcome,
        audible_count_consumed: bool,
        audible_count: u64,
    },
    Action(CoreAction),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreDispatch {
    pub(crate) connection_id: TraceId,
    pub(crate) emitted_at: OffsetDateTime,
    pub(crate) events: Vec<CoreEvent>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreStartOutcome {
    pub(crate) dispatch: CoreDispatch,
    pub(crate) replayed: bool,
    pub(crate) active_watch_count: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CorePublishOutcome {
    pub(crate) dispatches: Vec<CoreDispatch>,
    pub(crate) replayed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreCleanupReason {
    ConnectionClosed,
    HeartbeatExpired,
    ProcessStopped,
}

impl CoreCleanupReason {
    pub(crate) const fn episode_reason(self) -> CoreEpisodeEndReason {
        match self {
            Self::ConnectionClosed => CoreEpisodeEndReason::ConnectionClosed,
            Self::HeartbeatExpired => CoreEpisodeEndReason::HeartbeatExpired,
            Self::ProcessStopped => CoreEpisodeEndReason::ProcessStopped,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreCleanupReport {
    pub(crate) connection_id: TraceId,
    pub(crate) reason: CoreCleanupReason,
    pub(crate) recorded_at: OffsetDateTime,
    pub(crate) sections: Vec<SectionKey>,
    pub(crate) actions: Vec<CoreAction>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct CoreTickOutcome {
    pub(crate) expired_connections: Vec<CoreCleanupReport>,
    pub(crate) dispatches: Vec<CoreDispatch>,
}
