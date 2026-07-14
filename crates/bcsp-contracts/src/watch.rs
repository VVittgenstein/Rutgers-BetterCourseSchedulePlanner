use std::collections::BTreeSet;
use std::num::NonZeroU64;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};
use thiserror::Error;
use time::OffsetDateTime;

use crate::{OpenObservationV1, SectionKey, TraceId};

pub const WATCH_CONTRACT_VERSION: WatchContractVersion = WatchContractVersion::V1;
pub const MAX_ACTIVE_WATCHES: u8 = 9;
pub const DEFAULT_MAX_AUDIBLE: u64 = 3;
pub const DEFAULT_CONTINUOUS_DURATION_SECONDS: u64 = 600;
pub const ACTIVE_WATCH_STATE_PERSISTENT: bool = false;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WatchContractVersion {
    V1,
}

impl WatchContractVersion {
    pub const fn as_u16(self) -> u16 {
        match self {
            Self::V1 => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("unsupported watch contract version")]
pub struct WatchContractVersionError {
    observed: u16,
}

impl WatchContractVersionError {
    pub const fn observed(self) -> u16 {
        self.observed
    }
}

impl TryFrom<u16> for WatchContractVersion {
    type Error = WatchContractVersionError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::V1),
            observed => Err(WatchContractVersionError { observed }),
        }
    }
}

impl Serialize for WatchContractVersion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_u16(self.as_u16())
    }
}

impl<'de> Deserialize<'de> for WatchContractVersion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u16::deserialize(deserializer)?;
        Self::try_from(value).map_err(D::Error::custom)
    }
}

macro_rules! watch_id {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(TraceId);

        impl $name {
            pub const fn new(value: TraceId) -> Self {
                Self(value)
            }

            pub const fn trace_id(self) -> TraceId {
                self.0
            }
        }

        impl From<TraceId> for $name {
            fn from(value: TraceId) -> Self {
                Self(value)
            }
        }

        impl From<$name> for TraceId {
            fn from(value: $name) -> Self {
                value.0
            }
        }
    };
}

watch_id!(ActiveWatchId);
watch_id!(OpenEpisodeId);
watch_id!(WatchAlertId);
watch_id!(WatchCueId);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WatchNotificationMode {
    OneShot,
    Continuous,
}

impl WatchNotificationMode {
    pub const ALL: &'static [Self] = &[Self::OneShot, Self::Continuous];
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("watch value must be a positive integer")]
pub struct WatchPositiveValueError;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct WatchMaxAudible(NonZeroU64);

impl WatchMaxAudible {
    pub const fn get(self) -> u64 {
        self.0.get()
    }
}

impl Default for WatchMaxAudible {
    fn default() -> Self {
        Self(NonZeroU64::new(DEFAULT_MAX_AUDIBLE).expect("default max audible is positive"))
    }
}

impl TryFrom<u64> for WatchMaxAudible {
    type Error = WatchPositiveValueError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        NonZeroU64::new(value)
            .map(Self)
            .ok_or(WatchPositiveValueError)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WatchContinuousDurationV1 {
    Finite { seconds: NonZeroU64 },
    Unlimited,
}

impl WatchContinuousDurationV1 {
    pub fn finite_seconds(seconds: u64) -> Result<Self, WatchPositiveValueError> {
        NonZeroU64::new(seconds)
            .map(|seconds| Self::Finite { seconds })
            .ok_or(WatchPositiveValueError)
    }

    pub const fn seconds(self) -> Option<u64> {
        match self {
            Self::Finite { seconds } => Some(seconds.get()),
            Self::Unlimited => None,
        }
    }
}

impl Default for WatchContinuousDurationV1 {
    fn default() -> Self {
        Self::finite_seconds(DEFAULT_CONTINUOUS_DURATION_SECONDS)
            .expect("default continuous duration is positive")
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WatchPolicyV1 {
    pub notification_mode: WatchNotificationMode,
    pub max_audible: WatchMaxAudible,
    pub continuous_duration: WatchContinuousDurationV1,
}

impl WatchPolicyV1 {
    pub const fn new(
        notification_mode: WatchNotificationMode,
        max_audible: WatchMaxAudible,
        continuous_duration: WatchContinuousDurationV1,
    ) -> Self {
        Self {
            notification_mode,
            max_audible,
            continuous_duration,
        }
    }
}

impl Default for WatchPolicyV1 {
    fn default() -> Self {
        Self::new(
            WatchNotificationMode::OneShot,
            WatchMaxAudible::default(),
            WatchContinuousDurationV1::default(),
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WatchStartItemV1 {
    pub section_key: SectionKey,
    pub policy: WatchPolicyV1,
}

impl WatchStartItemV1 {
    pub const fn new(section_key: SectionKey, policy: WatchPolicyV1) -> Self {
        Self {
            section_key,
            policy,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum WatchStartItemsError {
    #[error("watch start must contain at least one Section")]
    Empty,
    #[error("watch start Section identities must be unique")]
    DuplicateSection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct WatchStartItemsV1(Vec<WatchStartItemV1>);

impl WatchStartItemsV1 {
    pub fn as_slice(&self) -> &[WatchStartItemV1] {
        &self.0
    }

    pub fn into_inner(self) -> Vec<WatchStartItemV1> {
        self.0
    }
}

impl TryFrom<Vec<WatchStartItemV1>> for WatchStartItemsV1 {
    type Error = WatchStartItemsError;

    fn try_from(value: Vec<WatchStartItemV1>) -> Result<Self, Self::Error> {
        if value.is_empty() {
            return Err(WatchStartItemsError::Empty);
        }
        let unique = value
            .iter()
            .map(|item| item.section_key.clone())
            .collect::<BTreeSet<_>>();
        if unique.len() != value.len() {
            return Err(WatchStartItemsError::DuplicateSection);
        }
        Ok(Self(value))
    }
}

impl<'de> Deserialize<'de> for WatchStartItemsV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(Vec::<WatchStartItemV1>::deserialize(deserializer)?)
            .map_err(D::Error::custom)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActiveWatchTargetV1 {
    pub active_watch_id: ActiveWatchId,
    pub section_key: SectionKey,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WatchEpisodeTargetV1 {
    pub active_watch_id: ActiveWatchId,
    pub episode_id: OpenEpisodeId,
    pub section_key: SectionKey,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WatchAlertTargetV1 {
    pub active_watch_id: ActiveWatchId,
    pub episode_id: OpenEpisodeId,
    pub alert_id: WatchAlertId,
    pub section_key: SectionKey,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WatchCueOutcome {
    Started,
    Muted,
    AutoplayBlocked,
    Failed,
}

impl WatchCueOutcome {
    pub const ALL: &'static [Self] = &[
        Self::Started,
        Self::Muted,
        Self::AutoplayBlocked,
        Self::Failed,
    ];
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WatchCueOutcomeReportV1 {
    pub cue_id: WatchCueId,
    pub active_watch_id: ActiveWatchId,
    pub section_key: SectionKey,
    pub outcome: WatchCueOutcome,
    #[serde(with = "time::serde::rfc3339")]
    pub reported_at: OffsetDateTime,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WatchClientCommandV1 {
    StartWatch {
        items: WatchStartItemsV1,
    },
    StopWatch {
        watch: ActiveWatchTargetV1,
    },
    UpdatePolicy {
        watch: ActiveWatchTargetV1,
        policy: WatchPolicyV1,
    },
    AcknowledgeEpisode {
        episode: WatchEpisodeTargetV1,
    },
    AcknowledgeAllEpisodes {},
    ResumeTimedOutEpisode {
        episode: WatchEpisodeTargetV1,
    },
    ResetAudibleCount {
        watch: ActiveWatchTargetV1,
    },
    ReportCueOutcome {
        report: WatchCueOutcomeReportV1,
    },
    DismissAlert {
        alert: WatchAlertTargetV1,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WatchStartRejectionReason {
    MaxActiveWatches,
    AlreadyActive,
    SectionNotFound,
    TargetUnavailable,
    ConnectionClosing,
}

impl WatchStartRejectionReason {
    pub const ALL: &'static [Self] = &[
        Self::MaxActiveWatches,
        Self::AlreadyActive,
        Self::SectionNotFound,
        Self::TargetUnavailable,
        Self::ConnectionClosing,
    ];
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "status",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum WatchStartItemResultV1 {
    Active {
        section_key: SectionKey,
        active_watch_id: ActiveWatchId,
        #[serde(with = "time::serde::rfc3339")]
        started_at: OffsetDateTime,
    },
    Rejected {
        section_key: SectionKey,
        reason: WatchStartRejectionReason,
    },
}

impl WatchStartItemResultV1 {
    pub const fn section_key(&self) -> &SectionKey {
        match self {
            Self::Active { section_key, .. } | Self::Rejected { section_key, .. } => section_key,
        }
    }

    pub const fn is_active(&self) -> bool {
        matches!(self, Self::Active { .. })
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum WatchStartResultError {
    #[error("watch start result must contain at least one item")]
    Empty,
    #[error("watch start result Section identities must be unique")]
    DuplicateSection,
    #[error("active watch count cannot exceed nine")]
    TooManyActive,
    #[error("active result items cannot exceed the reported connection active count")]
    InconsistentActiveCount,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchStartResultV1 {
    pub contract_version: WatchContractVersion,
    items: Vec<WatchStartItemResultV1>,
    active_watch_count: u8,
}

impl WatchStartResultV1 {
    pub fn try_new(
        items: Vec<WatchStartItemResultV1>,
        active_watch_count: u8,
    ) -> Result<Self, WatchStartResultError> {
        if items.is_empty() {
            return Err(WatchStartResultError::Empty);
        }
        let unique = items
            .iter()
            .map(|item| item.section_key().clone())
            .collect::<BTreeSet<_>>();
        if unique.len() != items.len() {
            return Err(WatchStartResultError::DuplicateSection);
        }
        if active_watch_count > MAX_ACTIVE_WATCHES {
            return Err(WatchStartResultError::TooManyActive);
        }
        if items.iter().filter(|item| item.is_active()).count() > usize::from(active_watch_count) {
            return Err(WatchStartResultError::InconsistentActiveCount);
        }
        Ok(Self {
            contract_version: WATCH_CONTRACT_VERSION,
            items,
            active_watch_count,
        })
    }

    pub fn items(&self) -> &[WatchStartItemResultV1] {
        &self.items
    }

    pub const fn active_watch_count(&self) -> u8 {
        self.active_watch_count
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WatchStartResultWireV1 {
    contract_version: WatchContractVersion,
    items: Vec<WatchStartItemResultV1>,
    active_watch_count: u8,
}

impl<'de> Deserialize<'de> for WatchStartResultV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = WatchStartResultWireV1::deserialize(deserializer)?;
        let value = Self::try_new(wire.items, wire.active_watch_count).map_err(D::Error::custom)?;
        if wire.contract_version != value.contract_version {
            return Err(D::Error::custom("watch contract version mismatch"));
        }
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WatchStopReason {
    UserRequested,
    ConnectionClosed,
    HeartbeatTimeout,
    ServiceStopping,
}

impl WatchStopReason {
    pub const ALL: &'static [Self] = &[
        Self::UserRequested,
        Self::ConnectionClosed,
        Self::HeartbeatTimeout,
        Self::ServiceStopping,
    ];
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchStoppedV1 {
    pub contract_version: WatchContractVersion,
    pub active_watch_id: ActiveWatchId,
    pub section_key: SectionKey,
    pub reason: WatchStopReason,
    #[serde(with = "time::serde::rfc3339")]
    pub stopped_at: OffsetDateTime,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchOpenObservationFanoutV1 {
    pub contract_version: WatchContractVersion,
    pub active_watch_id: ActiveWatchId,
    pub observation: OpenObservationV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OpenEpisodeState {
    Unacknowledged,
    Acknowledged,
    TimedOut,
    Closed,
}

impl OpenEpisodeState {
    pub const ALL: &'static [Self] = &[
        Self::Unacknowledged,
        Self::Acknowledged,
        Self::TimedOut,
        Self::Closed,
    ];
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum OpenEpisodeValidationError {
    #[error("episode timestamps are inconsistent")]
    InvalidTimeline,
    #[error("only a closed episode may carry closedAt")]
    InvalidClosedState,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenEpisodeV1 {
    pub contract_version: WatchContractVersion,
    pub episode_id: OpenEpisodeId,
    pub active_watch_id: ActiveWatchId,
    pub section_key: SectionKey,
    pub state: OpenEpisodeState,
    pub notification_mode: WatchNotificationMode,
    pub continuous_duration: WatchContinuousDurationV1,
    pub max_audible: WatchMaxAudible,
    pub audible_count: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub first_observed_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub last_observed_at: OffsetDateTime,
    pub observation_count: NonZeroU64,
    pub latest_observation_id: TraceId,
    #[serde(with = "time::serde::rfc3339")]
    pub state_changed_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub closed_at: Option<OffsetDateTime>,
}

impl OpenEpisodeV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn try_new(
        episode_id: OpenEpisodeId,
        active_watch_id: ActiveWatchId,
        section_key: SectionKey,
        state: OpenEpisodeState,
        policy: &WatchPolicyV1,
        audible_count: u64,
        first_observed_at: OffsetDateTime,
        last_observed_at: OffsetDateTime,
        observation_count: NonZeroU64,
        latest_observation_id: TraceId,
        state_changed_at: OffsetDateTime,
        closed_at: Option<OffsetDateTime>,
    ) -> Result<Self, OpenEpisodeValidationError> {
        if last_observed_at < first_observed_at || state_changed_at < first_observed_at {
            return Err(OpenEpisodeValidationError::InvalidTimeline);
        }
        if (state == OpenEpisodeState::Closed) != closed_at.is_some()
            || closed_at.is_some_and(|value| value < last_observed_at)
        {
            return Err(OpenEpisodeValidationError::InvalidClosedState);
        }
        Ok(Self {
            contract_version: WATCH_CONTRACT_VERSION,
            episode_id,
            active_watch_id,
            section_key,
            state,
            notification_mode: policy.notification_mode,
            continuous_duration: policy.continuous_duration,
            max_audible: policy.max_audible,
            audible_count,
            first_observed_at,
            last_observed_at,
            observation_count,
            latest_observation_id,
            state_changed_at,
            closed_at,
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpenEpisodeWireV1 {
    contract_version: WatchContractVersion,
    episode_id: OpenEpisodeId,
    active_watch_id: ActiveWatchId,
    section_key: SectionKey,
    state: OpenEpisodeState,
    notification_mode: WatchNotificationMode,
    continuous_duration: WatchContinuousDurationV1,
    max_audible: WatchMaxAudible,
    audible_count: u64,
    #[serde(with = "time::serde::rfc3339")]
    first_observed_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    last_observed_at: OffsetDateTime,
    observation_count: NonZeroU64,
    latest_observation_id: TraceId,
    #[serde(with = "time::serde::rfc3339")]
    state_changed_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    closed_at: Option<OffsetDateTime>,
}

impl<'de> Deserialize<'de> for OpenEpisodeV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = OpenEpisodeWireV1::deserialize(deserializer)?;
        let policy = WatchPolicyV1::new(
            wire.notification_mode,
            wire.max_audible,
            wire.continuous_duration,
        );
        let value = Self::try_new(
            wire.episode_id,
            wire.active_watch_id,
            wire.section_key,
            wire.state,
            &policy,
            wire.audible_count,
            wire.first_observed_at,
            wire.last_observed_at,
            wire.observation_count,
            wire.latest_observation_id,
            wire.state_changed_at,
            wire.closed_at,
        )
        .map_err(D::Error::custom)?;
        if wire.contract_version != value.contract_version {
            return Err(D::Error::custom("watch contract version mismatch"));
        }
        Ok(value)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WatchAlertDisposition {
    Opened,
    Updated,
    Dismissed,
    Closed,
}

impl WatchAlertDisposition {
    pub const ALL: &'static [Self] = &[Self::Opened, Self::Updated, Self::Dismissed, Self::Closed];
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchAlertV1 {
    pub contract_version: WatchContractVersion,
    pub alert_id: WatchAlertId,
    pub disposition: WatchAlertDisposition,
    pub visible: bool,
    pub episode: OpenEpisodeV1,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum WatchAudioTriggerV1 {
    OneShotObservation { observation_id: TraceId },
    ContinuousEpisode { episode_id: OpenEpisodeId },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchAudioCueV1 {
    pub cue_id: WatchCueId,
    pub active_watch_id: ActiveWatchId,
    pub section_key: SectionKey,
    pub trigger: WatchAudioTriggerV1,
    #[serde(with = "time::serde::rfc3339")]
    pub emitted_at: OffsetDateTime,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WatchContinuousMixerStopReason {
    NoUnacknowledgedEpisodes,
    NotificationModeChanged,
    ConnectionClosed,
    ServiceStopping,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WatchCueCancellationReason {
    WatchStopped,
    WatchRestarted,
    NotificationModeChanged,
    ConnectionClosed,
}

impl WatchCueCancellationReason {
    pub const ALL: &'static [Self] = &[
        Self::WatchStopped,
        Self::WatchRestarted,
        Self::NotificationModeChanged,
        Self::ConnectionClosed,
    ];
}

impl WatchContinuousMixerStopReason {
    pub const ALL: &'static [Self] = &[
        Self::NoUnacknowledgedEpisodes,
        Self::NotificationModeChanged,
        Self::ConnectionClosed,
        Self::ServiceStopping,
    ];
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "disposition",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum WatchAudioDispositionV1 {
    CueRequested {
        cue: WatchAudioCueV1,
    },
    CueQueued {
        active_watch_id: ActiveWatchId,
        section_key: SectionKey,
        observation_id: TraceId,
        #[serde(with = "time::serde::rfc3339")]
        queued_at: OffsetDateTime,
    },
    CueCancelled {
        active_watch_id: ActiveWatchId,
        section_key: SectionKey,
        observation_id: TraceId,
        cue_id: Option<WatchCueId>,
        reason: WatchCueCancellationReason,
        #[serde(with = "time::serde::rfc3339")]
        cancelled_at: OffsetDateTime,
    },
    SilentMaxAudible {
        active_watch_id: ActiveWatchId,
        section_key: SectionKey,
        observation_id: TraceId,
        audible_count: u64,
        max_audible: WatchMaxAudible,
        #[serde(with = "time::serde::rfc3339")]
        emitted_at: OffsetDateTime,
    },
    ContinuousMixerActive {
        episode_ids: BTreeSet<OpenEpisodeId>,
        #[serde(with = "time::serde::rfc3339")]
        emitted_at: OffsetDateTime,
    },
    ContinuousMixerStopped {
        reason: WatchContinuousMixerStopReason,
        #[serde(with = "time::serde::rfc3339")]
        emitted_at: OffsetDateTime,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchCueOutcomeReceiptV1 {
    pub contract_version: WatchContractVersion,
    pub cue_id: WatchCueId,
    pub active_watch_id: ActiveWatchId,
    pub section_key: SectionKey,
    pub outcome: WatchCueOutcome,
    pub audible_count_consumed: bool,
    pub audible_count: u64,
    #[serde(with = "time::serde::rfc3339")]
    pub recorded_at: OffsetDateTime,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase"
)]
pub enum WatchServerEventV1 {
    StartResult {
        result: WatchStartResultV1,
    },
    WatchStopped {
        stopped: WatchStoppedV1,
    },
    OpenObservation {
        fanout: WatchOpenObservationFanoutV1,
    },
    EpisodeUpdated {
        episode: OpenEpisodeV1,
    },
    AlertUpdated {
        alert: WatchAlertV1,
    },
    AudioDisposition {
        audio: WatchAudioDispositionV1,
    },
    CueOutcomeRecorded {
        receipt: WatchCueOutcomeReceiptV1,
    },
}
