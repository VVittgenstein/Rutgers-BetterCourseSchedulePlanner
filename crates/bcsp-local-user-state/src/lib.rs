//! Local-only durable settings, selection, desired-watch intent, and notification history.
//!
//! The caller chooses the SQLite path. This crate owns only `personal_*` tables inside that
//! database. It persists desired-watch INTENT (section + policy, written on user START and
//! removed on explicit STOP) so a reload can re-arm monitoring, but it deliberately has no
//! representation for a browser connection, a live watch identifier, or ring consumption --
//! restoring stored state never resurrects a live watch, it only re-arms intent.

#![forbid(unsafe_code)]
#![deny(warnings)]

mod error;
mod migration;
mod model;
mod saved_store;
mod saved_view;
mod store;

pub use error::{PersonalStateError, PersonalStateResult, SettingValueError};
pub use model::{
    CatalogRefreshMinutes, CurrentFilters, CurrentFiltersRevision, DesiredWatch,
    DesiredWatchMutation, EpisodeActionInput, EpisodeActionKind, EpisodeActionRecord,
    EpisodeDisposition, EpisodeHistoryIdentity, EpisodeHistorySummary, EpisodeSummaryInput,
    FilterAssociation, HistoryFilter, HistoryPage,
    HistoryWriteOutcome, LocalSettings, LocaleOverride, OpenRefreshSeconds, PageRequest,
    PersonalMigrationRecord, PersonalResetResult, PersonalStateSnapshot, PersonalTableCounts,
    SavedViewContent, SavedViewDefinition, SavedViewDeleteResult, SavedViewIncompatibility,
    SavedViewMatch, SavedViewMutation, SavedViewReviewCode, SavedViewReviewReason,
    SavedViewRevision, SavedViewsDeleteAllResult, SelectionMutation, SettingsRevision,
    SqliteConfiguration, StoredCurrentFilters, StoredSettings, UnixMillis, UserStateRevision,
    VolumePercent, WalCheckpoint, WatchFastLaneSeconds,
};
pub use store::PersonalStateStore;

pub const PACKAGE_BOUNDARY: &str = "bcsp-local-user-state";
pub const PERSONAL_MIGRATION_ID_BASE: u32 = 10_000;
pub const PERSONAL_MIGRATION_LEDGER_TABLE: &str = "personal_migration_ledger";
pub const PERSONAL_DATA_TABLE_ALLOWLIST: &[&str] = &[
    "personal_settings_v1",
    "personal_current_filters_v1",
    "personal_saved_views_v1",
    "personal_selected_sections_v1",
    "personal_desired_watches_v1",
    "personal_episode_summaries_v1",
    "personal_episode_actions_v1",
];
pub const PERSONAL_TABLE_ALLOWLIST: &[&str] = &[
    PERSONAL_MIGRATION_LEDGER_TABLE,
    "personal_state_metadata_v1",
    "personal_settings_v1",
    "personal_current_filters_v1",
    "personal_saved_views_v1",
    "personal_selected_sections_v1",
    "personal_desired_watches_v1",
    "personal_episode_summaries_v1",
    "personal_episode_actions_v1",
];
pub const MAX_SELECTED_SECTIONS: usize = bcsp_contracts::MAX_ACTIVE_WATCHES as usize;

pub fn boundary_marker() -> &'static str {
    let _ = (
        bcsp_contracts::PACKAGE_BOUNDARY,
        bcsp_domain::PACKAGE_BOUNDARY,
    );
    PACKAGE_BOUNDARY
}

mod dependency_contract {
    use tracing as _;
}

#[cfg(test)]
mod dev_dependency_contract {
    use tempfile as _;
}
