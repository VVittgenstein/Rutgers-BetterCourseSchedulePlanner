//! Local-only durable settings, selection, and notification history.
//!
//! The caller chooses the SQLite path. This crate owns only `personal_*` tables inside that
//! database and deliberately has no representation for a browser connection or active watch.

#![forbid(unsafe_code)]
#![deny(warnings)]

mod error;
mod migration;
mod model;
mod store;

pub use error::{PersonalStateError, PersonalStateResult, SettingValueError};
pub use model::{
    CatalogRefreshMinutes, CurrentFilters, EpisodeActionInput, EpisodeActionKind,
    EpisodeActionRecord, EpisodeDisposition, EpisodeHistoryIdentity, EpisodeHistorySummary,
    EpisodeSummaryInput, FilterAssociation, HistoryFilter, HistoryPage, HistoryWriteOutcome,
    LocalSettings, LocaleOverride, OpenRefreshSeconds, PageRequest, PersonalMigrationRecord,
    PersonalResetResult, PersonalStateSnapshot, PersonalTableCounts, SelectionMutation,
    SettingsRevision, SqliteConfiguration, StoredSettings, UnixMillis, VolumePercent,
    WalCheckpoint,
};
pub use store::PersonalStateStore;

pub const PACKAGE_BOUNDARY: &str = "bcsp-local-user-state";
pub const PERSONAL_MIGRATION_ID_BASE: u32 = 10_000;
pub const PERSONAL_MIGRATION_LEDGER_TABLE: &str = "personal_migration_ledger";
pub const PERSONAL_DATA_TABLE_ALLOWLIST: &[&str] = &[
    "personal_settings_v1",
    "personal_selected_sections_v1",
    "personal_episode_summaries_v1",
    "personal_episode_actions_v1",
];
pub const PERSONAL_TABLE_ALLOWLIST: &[&str] = &[
    PERSONAL_MIGRATION_LEDGER_TABLE,
    "personal_settings_v1",
    "personal_selected_sections_v1",
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
