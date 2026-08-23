use std::num::NonZeroU64;

use bcsp_contracts::{
    FilterRequestV1, OpenEpisodeId, OpenEpisodeState, SectionKey, TraceId, WatchNotificationMode,
    WatchPolicyV1, WatchStopReason,
};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;

use crate::{PersonalStateError, PersonalStateResult, SettingValueError};

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub enum LocaleOverride {
    #[default]
    #[serde(rename = "system")]
    System,
    #[serde(rename = "en-US")]
    EnUs,
    #[serde(rename = "zh-CN")]
    ZhCn,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CatalogRefreshMinutes(u16);

impl CatalogRefreshMinutes {
    pub const DEFAULT: Self = Self(10);
    pub const MIN: u16 = 1;
    pub const MAX: u16 = 1440;

    pub const fn get(self) -> u16 {
        self.0
    }

    pub(crate) const fn is_valid(self) -> bool {
        self.0 >= Self::MIN && self.0 <= Self::MAX
    }
}

impl Default for CatalogRefreshMinutes {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TryFrom<u16> for CatalogRefreshMinutes {
    type Error = SettingValueError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        let value = Self(value);
        value
            .is_valid()
            .then_some(value)
            .ok_or(SettingValueError::CatalogRefreshOutOfRange)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct OpenRefreshSeconds(u16);

impl OpenRefreshSeconds {
    pub const DEFAULT: Self = Self(30);
    pub const MIN: u16 = 3;
    pub const MAX: u16 = 3600;

    pub const fn get(self) -> u16 {
        self.0
    }

    pub(crate) const fn is_valid(self) -> bool {
        self.0 >= Self::MIN && self.0 <= Self::MAX
    }
}

impl Default for OpenRefreshSeconds {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TryFrom<u16> for OpenRefreshSeconds {
    type Error = SettingValueError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        let value = Self(value);
        value
            .is_valid()
            .then_some(value)
            .ok_or(SettingValueError::OpenRefreshOutOfRange)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct WatchFastLaneSeconds(u8);

impl WatchFastLaneSeconds {
    pub const DEFAULT: Self = Self(10);
    pub const MIN: u8 = 3;
    pub const MAX: u8 = 60;

    pub const fn get(self) -> u8 {
        self.0
    }

    pub(crate) const fn is_valid(self) -> bool {
        self.0 >= Self::MIN && self.0 <= Self::MAX
    }
}

impl Default for WatchFastLaneSeconds {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TryFrom<u8> for WatchFastLaneSeconds {
    type Error = SettingValueError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let value = Self(value);
        value
            .is_valid()
            .then_some(value)
            .ok_or(SettingValueError::WatchFastLaneOutOfRange)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(transparent)]
pub struct VolumePercent(u8);

impl VolumePercent {
    pub const DEFAULT: Self = Self(100);
    pub const MAX: u8 = 100;

    pub const fn get(self) -> u8 {
        self.0
    }

    pub(crate) const fn is_valid(self) -> bool {
        self.0 <= Self::MAX
    }
}

impl Default for VolumePercent {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TryFrom<u8> for VolumePercent {
    type Error = SettingValueError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let value = Self(value);
        value
            .is_valid()
            .then_some(value)
            .ok_or(SettingValueError::VolumeOutOfRange)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum FilterAssociation {
    Custom,
    Applied {
        view_id: TraceId,
        revision: SavedViewRevision,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CurrentFilters {
    pub association: FilterAssociation,
    pub content: SavedViewContent,
}

impl CurrentFilters {
    pub fn new(association: FilterAssociation, filters: FilterRequestV1) -> Self {
        Self {
            association,
            content: SavedViewContent::Compatible {
                filters: Box::new(filters),
            },
        }
    }

    pub fn filters(&self) -> Option<&FilterRequestV1> {
        self.content.filters()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalSettings {
    pub locale_override: LocaleOverride,
    pub catalog_refresh_minutes: CatalogRefreshMinutes,
    pub open_refresh_seconds: OpenRefreshSeconds,
    #[serde(default)]
    pub watch_fast_lane_seconds: WatchFastLaneSeconds,
    pub volume_percent: VolumePercent,
    pub sound_policy: WatchPolicyV1,
}

impl Default for LocalSettings {
    fn default() -> Self {
        Self {
            locale_override: LocaleOverride::System,
            catalog_refresh_minutes: CatalogRefreshMinutes::DEFAULT,
            open_refresh_seconds: OpenRefreshSeconds::DEFAULT,
            watch_fast_lane_seconds: WatchFastLaneSeconds::DEFAULT,
            volume_percent: VolumePercent::DEFAULT,
            sound_policy: WatchPolicyV1::default(),
        }
    }
}

impl LocalSettings {
    pub(crate) fn validate(&self) -> Result<(), SettingValueError> {
        if !self.catalog_refresh_minutes.is_valid() {
            return Err(SettingValueError::CatalogRefreshOutOfRange);
        }
        if !self.open_refresh_seconds.is_valid() {
            return Err(SettingValueError::OpenRefreshOutOfRange);
        }
        if !self.watch_fast_lane_seconds.is_valid() {
            return Err(SettingValueError::WatchFastLaneOutOfRange);
        }
        if !self.volume_percent.is_valid() {
            return Err(SettingValueError::VolumeOutOfRange);
        }
        Ok(())
    }
}

#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct SettingsRevision(u64);

impl SettingsRevision {
    pub const ZERO: Self = Self(0);

    pub const fn get(self) -> u64 {
        self.0
    }

    pub(crate) const fn from_stored(value: u64) -> Self {
        Self(value)
    }
}

impl TryFrom<u64> for SettingsRevision {
    type Error = PersonalStateError;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        if value <= i64::MAX as u64 {
            Ok(Self(value))
        } else {
            Err(PersonalStateError::RevisionOverflow)
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredSettings {
    pub revision: SettingsRevision,
    pub value: LocalSettings,
}

macro_rules! revision_type {
    ($name:ident) => {
        #[derive(
            Clone,
            Copy,
            Debug,
            Default,
            Deserialize,
            Eq,
            Hash,
            Ord,
            PartialEq,
            PartialOrd,
            Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(u64);

        impl $name {
            pub const ZERO: Self = Self(0);

            pub const fn get(self) -> u64 {
                self.0
            }

            pub(crate) const fn from_stored(value: u64) -> Self {
                Self(value)
            }
        }

        impl TryFrom<u64> for $name {
            type Error = PersonalStateError;

            fn try_from(value: u64) -> Result<Self, Self::Error> {
                if value <= i64::MAX as u64 {
                    Ok(Self(value))
                } else {
                    Err(PersonalStateError::RevisionOverflow)
                }
            }
        }
    };
}

revision_type!(UserStateRevision);
revision_type!(CurrentFiltersRevision);
revision_type!(SavedViewRevision);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SavedViewIncompatibility {
    InvalidEnvelope,
    UnsupportedCodecVersion { observed: Option<u64> },
    FutureSchemaVersion { observed: u64, supported: u64 },
    UnknownField { stable_id: String },
    MissingRequiredField { stable_id: String },
    InvalidFieldData,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SavedViewReviewCode {
    ActiveLegacyFilter,
    UnsupportedCampus,
    ScopeUnavailable,
    DynamicValueUnavailable,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedViewReviewReason {
    pub stable_id: String,
    pub code: SavedViewReviewCode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "status",
    rename_all = "SCREAMING_SNAKE_CASE",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum SavedViewContent {
    Compatible {
        filters: Box<FilterRequestV1>,
    },
    Incompatible {
        raw_snapshot: JsonValue,
        reason: SavedViewIncompatibility,
    },
    ReviewRequired {
        raw_snapshot: JsonValue,
        reasons: Vec<SavedViewReviewReason>,
    },
}

impl SavedViewContent {
    pub fn filters(&self) -> Option<&FilterRequestV1> {
        match self {
            Self::Compatible { filters } => Some(filters.as_ref()),
            Self::Incompatible { .. } | Self::ReviewRequired { .. } => None,
        }
    }

    pub fn incompatibility(&self) -> Option<&SavedViewIncompatibility> {
        match self {
            Self::Compatible { .. } | Self::ReviewRequired { .. } => None,
            Self::Incompatible { reason, .. } => Some(reason),
        }
    }

    pub fn review_reasons(&self) -> Option<&[SavedViewReviewReason]> {
        match self {
            Self::ReviewRequired { reasons, .. } => Some(reasons),
            Self::Compatible { .. } | Self::Incompatible { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredCurrentFilters {
    pub state_revision: UserStateRevision,
    pub revision: CurrentFiltersRevision,
    pub value: Option<CurrentFilters>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedViewDefinition {
    pub id: TraceId,
    pub name: String,
    pub schema_version: u64,
    pub revision: SavedViewRevision,
    pub content: SavedViewContent,
    pub created_at: UnixMillis,
    pub updated_at: UnixMillis,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SavedViewMatch {
    NotApplied,
    Clean,
    Modified,
    Incompatible,
    ReviewRequired,
}

impl SavedViewDefinition {
    pub fn match_current(&self, current: &StoredCurrentFilters) -> SavedViewMatch {
        let Some(filters) = self.content.filters() else {
            return if matches!(self.content, SavedViewContent::ReviewRequired { .. }) {
                SavedViewMatch::ReviewRequired
            } else {
                SavedViewMatch::Incompatible
            };
        };
        let Some(current) = &current.value else {
            return SavedViewMatch::NotApplied;
        };
        let FilterAssociation::Applied { view_id, .. } = &current.association else {
            return SavedViewMatch::NotApplied;
        };
        if *view_id != self.id {
            return SavedViewMatch::NotApplied;
        }
        if current.filters() == Some(filters) {
            SavedViewMatch::Clean
        } else {
            SavedViewMatch::Modified
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedViewMutation {
    pub state_revision: UserStateRevision,
    pub definition: SavedViewDefinition,
    pub current_filters: StoredCurrentFilters,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedViewDeleteResult {
    pub state_revision: UserStateRevision,
    pub deleted_id: TraceId,
    pub current_filters: StoredCurrentFilters,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedViewsDeleteAllResult {
    pub state_revision: UserStateRevision,
    pub deleted_views: u64,
    pub current_filters: StoredCurrentFilters,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct UnixMillis(i64);

impl UnixMillis {
    pub const fn get(self) -> i64 {
        self.0
    }
}

impl TryFrom<i64> for UnixMillis {
    type Error = SettingValueError;

    fn try_from(value: i64) -> Result<Self, Self::Error> {
        (value >= 0)
            .then_some(Self(value))
            .ok_or(SettingValueError::NegativeTimestamp)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EpisodeHistoryIdentity {
    pub section_key: SectionKey,
    pub run_id: TraceId,
    pub episode_id: OpenEpisodeId,
}

impl EpisodeHistoryIdentity {
    pub const fn new(section_key: SectionKey, run_id: TraceId, episode_id: OpenEpisodeId) -> Self {
        Self {
            section_key,
            run_id,
            episode_id,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EpisodeSummaryInput {
    pub identity: EpisodeHistoryIdentity,
    pub state: OpenEpisodeState,
    pub mode: WatchNotificationMode,
    pub first_observed_at: UnixMillis,
    pub last_observed_at: UnixMillis,
    pub acknowledged_at: Option<UnixMillis>,
    pub timed_out_at: Option<UnixMillis>,
    pub closed_at: Option<UnixMillis>,
    pub disposition: Option<EpisodeDisposition>,
    pub audible_count: u64,
    pub observation_count: NonZeroU64,
    pub last_observation_id: TraceId,
}

impl EpisodeSummaryInput {
    pub(crate) fn validate(&self) -> PersonalStateResult<()> {
        let nonnegative = self.first_observed_at.get() >= 0
            && self.last_observed_at.get() >= 0
            && self.acknowledged_at.is_none_or(|value| value.get() >= 0)
            && self.timed_out_at.is_none_or(|value| value.get() >= 0)
            && self.closed_at.is_none_or(|value| value.get() >= 0);
        let within_timeline = |value: UnixMillis| {
            value >= self.first_observed_at
                && self.closed_at.is_none_or(|closed_at| value <= closed_at)
        };
        let timeline_is_valid = self.last_observed_at >= self.first_observed_at
            && self.acknowledged_at.is_none_or(within_timeline)
            && self.timed_out_at.is_none_or(within_timeline)
            && self
                .closed_at
                .is_none_or(|closed_at| closed_at >= self.last_observed_at)
            && !(self.acknowledged_at.is_some() && self.timed_out_at.is_some());
        let state_is_valid = match self.state {
            OpenEpisodeState::Unacknowledged => {
                self.acknowledged_at.is_none()
                    && self.timed_out_at.is_none()
                    && self.closed_at.is_none()
                    && self.disposition.is_none()
            }
            OpenEpisodeState::Acknowledged => {
                self.acknowledged_at.is_some()
                    && self.timed_out_at.is_none()
                    && self.closed_at.is_none()
                    && self.disposition == Some(EpisodeDisposition::Acknowledged)
            }
            OpenEpisodeState::TimedOut => {
                self.acknowledged_at.is_none()
                    && self.timed_out_at.is_some()
                    && self.closed_at.is_none()
                    && self.disposition == Some(EpisodeDisposition::TimedOut)
            }
            OpenEpisodeState::Closed => {
                self.closed_at.is_some()
                    && matches!(
                        self.disposition,
                        Some(
                            EpisodeDisposition::SectionClosed
                                | EpisodeDisposition::WatchStopped { .. }
                        )
                    )
            }
        };
        if nonnegative && timeline_is_valid && state_is_valid {
            Ok(())
        } else {
            Err(PersonalStateError::InvalidEpisodeSummary)
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EpisodeHistorySummary {
    pub identity: EpisodeHistoryIdentity,
    pub state: OpenEpisodeState,
    pub mode: WatchNotificationMode,
    pub first_observed_at: UnixMillis,
    pub last_observed_at: UnixMillis,
    pub acknowledged_at: Option<UnixMillis>,
    pub timed_out_at: Option<UnixMillis>,
    pub closed_at: Option<UnixMillis>,
    pub disposition: Option<EpisodeDisposition>,
    pub audible_count: u64,
    pub observation_count: NonZeroU64,
    pub last_observation_id: TraceId,
    pub action_count: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum EpisodeDisposition {
    SectionClosed,
    Acknowledged,
    TimedOut,
    WatchStopped { reason: WatchStopReason },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EpisodeActionKind {
    Opened,
    Updated,
    Acknowledged,
    TimedOut,
    Resumed,
    Closed,
    CuePlayed,
    CueMuted,
    CueAutoplayBlocked,
    CueFailed,
    AlertDismissed,
    PolicyUpdated,
}

impl EpisodeActionKind {
    pub const ALL: &'static [Self] = &[
        Self::Opened,
        Self::Updated,
        Self::Acknowledged,
        Self::TimedOut,
        Self::Resumed,
        Self::Closed,
        Self::CuePlayed,
        Self::CueMuted,
        Self::CueAutoplayBlocked,
        Self::CueFailed,
        Self::AlertDismissed,
        Self::PolicyUpdated,
    ];

    pub const fn wire_name(self) -> &'static str {
        match self {
            Self::Opened => "OPENED",
            Self::Updated => "UPDATED",
            Self::Acknowledged => "ACKNOWLEDGED",
            Self::TimedOut => "TIMED_OUT",
            Self::Resumed => "RESUMED",
            Self::Closed => "CLOSED",
            Self::CuePlayed => "CUE_PLAYED",
            Self::CueMuted => "CUE_MUTED",
            Self::CueAutoplayBlocked => "CUE_AUTOPLAY_BLOCKED",
            Self::CueFailed => "CUE_FAILED",
            Self::AlertDismissed => "ALERT_DISMISSED",
            Self::PolicyUpdated => "POLICY_UPDATED",
        }
    }

    pub(crate) fn from_wire(value: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|candidate| candidate.wire_name() == value)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EpisodeActionInput {
    pub action_id: TraceId,
    pub identity: EpisodeHistoryIdentity,
    pub kind: EpisodeActionKind,
    pub occurred_at: UnixMillis,
}

pub type EpisodeActionRecord = EpisodeActionInput;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HistoryWriteOutcome {
    Inserted,
    AlreadyPresent,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryFilter {
    pub section_key: Option<SectionKey>,
    pub run_id: Option<TraceId>,
    pub episode_id: Option<OpenEpisodeId>,
    pub action_kind: Option<EpisodeActionKind>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageRequest {
    pub offset: u64,
    pub limit: u32,
}

impl PageRequest {
    pub const DEFAULT: Self = Self {
        offset: 0,
        limit: 50,
    };

    pub fn try_new(offset: u64, limit: u32) -> PersonalStateResult<Self> {
        if !(1..=100).contains(&limit) {
            return Err(PersonalStateError::InvalidPageLimit);
        }
        if offset > i64::MAX as u64 {
            return Err(PersonalStateError::InvalidPageOffset);
        }
        Ok(Self { offset, limit })
    }
}

impl Default for PageRequest {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryPage<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub offset: u64,
    pub limit: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE", rename_all_fields = "camelCase")]
pub enum SelectionMutation {
    Added { position: u8 },
    AlreadySelected { position: u8 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersonalMigrationRecord {
    pub migration_id: u32,
    pub name: String,
    pub checksum: String,
}

/// One persisted desired-watch intent row: the section plus the policy the
/// user chose at START. No watch identifier, no ring consumption, no
/// connection state -- restoring it re-arms intent via a fresh START.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DesiredWatch {
    pub section: SectionKey,
    pub policy: WatchPolicyV1,
}

/// One row of desired-watch authority state, **including tombstones**.
///
/// `policy` is `None` exactly when the row is a tombstone. A tombstone is
/// not debris: without it a removed section has no revision left to compare a
/// late `basedOnRevision` against, and a delayed START would be admitted and
/// would resurrect intent the user had cancelled.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesiredWatchEntry {
    pub section: SectionKey,
    pub policy: Option<WatchPolicyV1>,
    pub revision: u64,
    pub materialization_epoch: u64,
}

impl DesiredWatchEntry {
    pub const fn is_tombstone(&self) -> bool {
        self.policy.is_none()
    }
}

/// The four persisted authority counters.
///
/// All four cross the wire to a JavaScript client, so the table bounds them at
/// `Number.MAX_SAFE_INTEGER`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DesiredWatchCounters {
    pub authority_generation: u64,
    pub revision_counter: u64,
    pub materialization_counter: u64,
    pub actor_incarnation: u64,
}

/// Full desired-watch authority state: counters plus every row.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DesiredWatchAuthority {
    pub counters: DesiredWatchCounters,
    pub entries: Vec<DesiredWatchEntry>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersonalTableCounts {
    pub settings: u64,
    pub current_filters: u64,
    pub saved_views: u64,
    pub selected_sections: u64,
    pub desired_watches: u64,
    pub desired_watch_receipts: u64,
    pub episode_summaries: u64,
    pub episode_actions: u64,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersonalResetResult {
    pub state_revision: UserStateRevision,
    pub deleted_settings: u64,
    pub deleted_current_filters: u64,
    pub deleted_saved_views: u64,
    pub deleted_selected_sections: u64,
    pub deleted_desired_watches: u64,
    pub deleted_desired_watch_receipts: u64,
    pub deleted_episode_summaries: u64,
    pub deleted_episode_actions: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersonalStateSnapshot {
    pub state_revision: UserStateRevision,
    pub settings: StoredSettings,
    pub current_filters: StoredCurrentFilters,
    pub saved_views: Vec<SavedViewDefinition>,
    pub selected_sections: Vec<SectionKey>,
    pub desired_watches: Vec<DesiredWatch>,
    pub episode_history: HistoryPage<EpisodeHistorySummary>,
    pub active_watch_count: u8,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SqliteConfiguration {
    pub journal_mode: String,
    pub foreign_keys: bool,
    pub busy_timeout_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WalCheckpoint {
    pub busy: u64,
    pub log_frames: u64,
    pub checkpointed_frames: u64,
}
