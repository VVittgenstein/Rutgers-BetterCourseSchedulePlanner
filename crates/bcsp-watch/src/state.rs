use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::time::Duration;

use bcsp_contracts::{SectionKey, TraceId};
use time::OffsetDateTime;

use crate::WatchInstant;

pub(crate) const DEFAULT_MAX_AUDIBLE: NonZeroU64 = NonZeroU64::new(3).unwrap();
pub(crate) const DEFAULT_CONTINUOUS_SECONDS: NonZeroU64 = NonZeroU64::new(600).unwrap();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreNotificationMode {
    OneShot,
    Continuous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreContinuousDuration {
    Finite(NonZeroU64),
    Unlimited,
}

impl CoreContinuousDuration {
    pub(crate) fn deadline(self, now: WatchInstant) -> Option<WatchInstant> {
        match self {
            Self::Finite(seconds) => Some(now.saturating_add(Duration::from_secs(seconds.get()))),
            Self::Unlimited => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CorePolicy {
    pub(crate) mode: CoreNotificationMode,
    pub(crate) max_audible: NonZeroU64,
    pub(crate) continuous_duration: CoreContinuousDuration,
}

impl Default for CorePolicy {
    fn default() -> Self {
        Self {
            mode: CoreNotificationMode::OneShot,
            max_audible: DEFAULT_MAX_AUDIBLE,
            continuous_duration: CoreContinuousDuration::Finite(DEFAULT_CONTINUOUS_SECONDS),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreInitialState {
    Unknown,
    Closed,
    Open {
        observation_id: TraceId,
        observed_at: OffsetDateTime,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ReliableState {
    Closed,
    Open,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreEpisodeState {
    Unacknowledged,
    Acknowledged,
    TimedOut,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoreEpisodeEndReason {
    ObservedClosed,
    WatchStopped,
    ConnectionClosed,
    HeartbeatExpired,
    ProcessStopped,
    Restarted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CoreEpisode {
    pub(crate) id: TraceId,
    pub(crate) alert_id: TraceId,
    pub(crate) watch_id: TraceId,
    pub(crate) section: SectionKey,
    pub(crate) policy: CorePolicy,
    pub(crate) audible_count: u64,
    pub(crate) sequence: u64,
    pub(crate) state: CoreEpisodeState,
    pub(crate) observation_count: u64,
    pub(crate) first_observation_id: TraceId,
    pub(crate) last_observation_id: TraceId,
    pub(crate) first_observed_at: OffsetDateTime,
    pub(crate) last_observed_at: OffsetDateTime,
    pub(crate) state_changed_at: OffsetDateTime,
    pub(crate) closed_at: Option<OffsetDateTime>,
    pub(crate) deadline: Option<WatchInstant>,
    pub(crate) end_reason: Option<CoreEpisodeEndReason>,
    pub(crate) alert_dismissed: bool,
}

impl CoreEpisode {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        id: TraceId,
        alert_id: TraceId,
        watch_id: TraceId,
        section: SectionKey,
        policy: CorePolicy,
        audible_count: u64,
        sequence: u64,
        observation_id: TraceId,
        observed_at: OffsetDateTime,
        deadline: Option<WatchInstant>,
    ) -> Self {
        Self {
            id,
            alert_id,
            watch_id,
            section,
            policy,
            audible_count,
            sequence,
            state: CoreEpisodeState::Unacknowledged,
            observation_count: 1,
            first_observation_id: observation_id,
            last_observation_id: observation_id,
            first_observed_at: observed_at,
            last_observed_at: observed_at,
            state_changed_at: observed_at,
            closed_at: None,
            deadline,
            end_reason: None,
            alert_dismissed: false,
        }
    }

    pub(crate) fn observe_open(&mut self, observation_id: TraceId, observed_at: OffsetDateTime) {
        self.observation_count = self.observation_count.saturating_add(1);
        self.last_observation_id = observation_id;
        self.last_observed_at = self.last_observed_at.max(observed_at);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingCue {
    pub(crate) id: TraceId,
    pub(crate) observation_id: TraceId,
    pub(crate) episode_id: TraceId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ObservationCursor {
    pub(crate) pull_sequence: u64,
    pub(crate) observation_id: TraceId,
}

#[derive(Clone, Debug)]
pub(crate) struct CoreWatchSession {
    pub(crate) id: TraceId,
    pub(crate) section: SectionKey,
    pub(crate) policy: CorePolicy,
    pub(crate) reliable_state: Option<ReliableState>,
    pub(crate) episode_sequence: u64,
    pub(crate) episode: Option<CoreEpisode>,
    pub(crate) audible_count: u64,
    pub(crate) active_cue: Option<PendingCue>,
    pub(crate) queued_cues: VecDeque<PendingCue>,
    pub(crate) observation_cursor: Option<ObservationCursor>,
}

impl CoreWatchSession {
    pub(crate) fn new(
        id: TraceId,
        section: SectionKey,
        policy: CorePolicy,
        initial_state: CoreInitialState,
        observation_cursor: Option<ObservationCursor>,
    ) -> Self {
        Self {
            id,
            section,
            policy,
            reliable_state: match initial_state {
                CoreInitialState::Unknown => None,
                CoreInitialState::Closed => Some(ReliableState::Closed),
                CoreInitialState::Open { .. } => Some(ReliableState::Open),
            },
            episode_sequence: 0,
            episode: None,
            audible_count: 0,
            active_cue: None,
            queued_cues: VecDeque::new(),
            observation_cursor,
        }
    }

    pub(crate) fn continuous_alarm_active(&self) -> bool {
        self.policy.mode == CoreNotificationMode::Continuous
            && self
                .episode
                .as_ref()
                .is_some_and(|episode| episode.state == CoreEpisodeState::Unacknowledged)
    }

    pub(crate) fn take_pending_cues(&mut self) -> Vec<PendingCue> {
        self.active_cue
            .take()
            .into_iter()
            .chain(self.queued_cues.drain(..))
            .collect()
    }
}
