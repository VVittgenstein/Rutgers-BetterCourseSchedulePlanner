//! Local-only durable settings, selection, desired-watch intent, and notification history.
//!
//! The caller chooses the SQLite path. This crate owns only `personal_*` tables inside that
//! database. It persists desired-watch INTENT (section + policy, written on user START and
//! removed on explicit STOP) so a reload can re-arm monitoring, but it deliberately has no
//! representation for a browser connection, a live watch identifier, or ring consumption --
//! restoring stored state never resurrects a live watch, it only re-arms intent.
//!
//! The desired-watch table is **authority state** for the revision/CAS co-editing design:
//! every row carries a revision and a materialization epoch, a removal leaves a tombstone
//! rather than vanishing, and repeated mutations -- successes and terminal rejections alike --
//! are settled against a receipt ledger, so the same mutation id can never be answered two
//! different ways. The CAS writer lives here; nothing calls it yet, and `desired_watches()`
//! deliberately hides tombstones so the bootstrap wire stays byte-identical to protocol v1
//! until the frontend slice consumes the new shape.

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
    DesiredWatchAdmission, DesiredWatchAuthority, DesiredWatchCommand, DesiredWatchCommitted,
    DesiredWatchCounters, DesiredWatchEntry, DesiredWatchMutationOutcome, DesiredWatchReceipt,
    DesiredWatchReceiptOutcome, DesiredWatchRejection, DesiredWatchRetirementOutcome,
    EpisodeActionInput, EpisodeActionKind, EpisodeActionRecord, EpisodeDisposition,
    EpisodeHistoryIdentity, EpisodeHistorySummary, EpisodeSummaryInput, FilterAssociation,
    HistoryFilter, HistoryPage, HistoryWriteOutcome, LocalSettings, LocaleOverride,
    OpenRefreshSeconds, PageRequest, PersonalMigrationRecord, PersonalResetResult,
    PersonalStateSnapshot, PersonalTableCounts, SavedViewContent, SavedViewDefinition,
    SavedViewDeleteResult, SavedViewIncompatibility, SavedViewMatch, SavedViewMutation,
    SavedViewReviewCode, SavedViewReviewReason, SavedViewRevision, SavedViewsDeleteAllResult,
    SelectionMutation, SettingsRevision, SqliteConfiguration, StoredCurrentFilters, StoredSettings,
    UnixMillis, UserStateRevision, VolumePercent, WalCheckpoint, WatchFastLaneSeconds,
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
    "personal_desired_watch_receipts_v1",
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
    "personal_desired_watch_receipts_v1",
    "personal_episode_summaries_v1",
    "personal_episode_actions_v1",
];
pub const MAX_SELECTED_SECTIONS: usize = bcsp_contracts::MAX_ACTIVE_WATCHES as usize;
/// The product admission cap on desired watches, tested against the state a
/// mutation would LEAVE BEHIND rather than the state it found.
pub const MAX_DESIRED_WATCHES: usize = bcsp_contracts::MAX_ACTIVE_WATCHES as usize;

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
