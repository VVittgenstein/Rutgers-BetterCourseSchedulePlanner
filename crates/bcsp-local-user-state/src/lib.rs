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
//! different ways.
//!
//! The writer enforces only rules about the AUTHORITY: the generation, the receipt ledger, the
//! based-on revision, the nine-section product cap measured against the state a mutation leaves
//! behind, and the two resource budgets. It never asks whether a section can be watched. Whether
//! the campus is a product target, the term is in the window, the catalog publishes the section
//! and the integrity gate has released it are all conditions of MATERIALIZING an intent, they are
//! all revocable, and none of them is the user's intent changing. Nothing in this crate can
//! withdraw intent on the user's behalf either: a section the runtime proves it can never arm
//! keeps its row, and the reason is reported rather than acted on.
//!
//! `desired_watches()` deliberately hides tombstones so the bootstrap wire stays byte-identical
//! to protocol v1; the authority read that includes them is `desired_watch_authority()`.

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
    DesiredWatchAuthority, DesiredWatchBudget, DesiredWatchBudgetKind, DesiredWatchCommand,
    DesiredWatchCommitted, DesiredWatchCounters, DesiredWatchEntry, DesiredWatchMutationOutcome,
    DesiredWatchReceipt, DesiredWatchReceiptOutcome, DesiredWatchRotation, EpisodeActionInput,
    EpisodeActionKind, EpisodeActionRecord, EpisodeDisposition, EpisodeHistoryIdentity,
    EpisodeHistorySummary, EpisodeSummaryInput, FilterAssociation, HistoryFilter, HistoryPage,
    HistoryWriteOutcome, LocalSettings, LocaleOverride, OpenRefreshSeconds, PageRequest,
    PersonalMigrationRecord, PersonalResetResult, PersonalStateSnapshot, PersonalTableCounts,
    PersonalTransactionState, SavedViewContent, SavedViewDefinition, SavedViewDeleteResult,
    SavedViewIncompatibility, SavedViewMatch, SavedViewMutation, SavedViewReviewCode,
    SavedViewReviewReason, SavedViewRevision, SavedViewsDeleteAllResult, SelectionMutation,
    SettingsRevision, SqliteConfiguration, StoredCurrentFilters, StoredSettings, UnixMillis,
    UserStateRevision, VolumePercent, WalCheckpoint, WalCheckpointMode, WatchFastLaneSeconds,
};
pub use store::PersonalStateStore;

pub const PACKAGE_BOUNDARY: &str = "bcsp-local-user-state";
/// Upper bound SQLite keeps the `-wal` file at after a checkpoint that resets
/// the log. Set on every writer connection this crate opens, because the
/// connection that resets the log is the one that truncates the file.
pub const WAL_JOURNAL_SIZE_LIMIT_BYTES: u64 = 64 * 1024 * 1024;
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
/// Frozen hard cap on removal history. A tombstone holds a section's revision
/// so a delayed command cannot be mistaken for a fresh one, so they can only
/// be cleared by raising the generation -- which is what rotation does.
pub const MAX_DESIRED_WATCH_TOMBSTONES: u64 = 512;
/// Frozen hard cap on the receipt ledger.
pub const MAX_DESIRED_WATCH_RECEIPTS: u64 = 2048;
/// Rotation is due at 80% of a budget, floored: `512 * 4 / 5 == 409`.
pub const DESIRED_WATCH_TOMBSTONE_ROTATION_THRESHOLD: u64 = MAX_DESIRED_WATCH_TOMBSTONES * 4 / 5;
/// `2048 * 4 / 5 == 1638`.
pub const DESIRED_WATCH_RECEIPT_ROTATION_THRESHOLD: u64 = MAX_DESIRED_WATCH_RECEIPTS * 4 / 5;
/// The most authority rows that can legally exist at once: every desired
/// section plus a full removal history. This is the number a frame-size proof
/// has to be made against, which is why the caps above are enforced in the
/// writer rather than left to a reconciler that might miss a round.
pub const MAX_DESIRED_WATCH_AUTHORITY_ROWS: u64 =
    MAX_DESIRED_WATCH_TOMBSTONES + MAX_DESIRED_WATCHES as u64;

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
