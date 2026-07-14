use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum SettingValueError {
    #[error("Catalog refresh interval must be between 1 and 1440 minutes")]
    CatalogRefreshOutOfRange,
    #[error("Open refresh interval must be between 3 and 3600 seconds")]
    OpenRefreshOutOfRange,
    #[error("volume must be between 0 and 100 percent")]
    VolumeOutOfRange,
    #[error("timestamp must be a non-negative Unix millisecond value")]
    NegativeTimestamp,
}

#[derive(Debug, Error)]
pub enum PersonalStateError {
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("stored identity is invalid: {0}")]
    Identity(#[from] bcsp_contracts::IdentityError),
    #[error("stored trace ID is invalid: {0}")]
    TraceId(#[from] bcsp_contracts::TraceIdError),
    #[error(transparent)]
    InvalidSetting(#[from] SettingValueError),
    #[error("personal migration sequence is not a contiguous applied prefix")]
    InvalidMigrationSequence,
    #[error("database contains unknown personal migration {migration_id} ({name})")]
    UnknownMigration { migration_id: u32, name: String },
    #[error("personal migration {migration_id} name differs from the embedded migration")]
    MigrationNameMismatch { migration_id: u32 },
    #[error("personal migration {migration_id} checksum differs from the embedded migration")]
    MigrationChecksumMismatch { migration_id: u32 },
    #[error("settings revision conflict: expected {expected}, actual {actual}")]
    RevisionConflict { expected: u64, actual: u64 },
    #[error("settings revision cannot be represented by SQLite")]
    RevisionOverflow,
    #[error("selection contains duplicate SectionKey {0}")]
    DuplicateSelection(bcsp_contracts::SectionKey),
    #[error("selection limit exceeded; maximum is {maximum}")]
    SelectionLimitExceeded { maximum: usize },
    #[error("history page limit must be between 1 and 100")]
    InvalidPageLimit,
    #[error("history page offset cannot be represented by SQLite")]
    InvalidPageOffset,
    #[error("episode summary timeline or state is invalid")]
    InvalidEpisodeSummary,
    #[error("episode summary does not exist")]
    EpisodeSummaryNotFound,
    #[error("history action ID is already bound to different content")]
    ActionIdConflict,
    #[error("stored personal state is invalid in {table}.{field}")]
    InvalidStoredValue {
        table: &'static str,
        field: &'static str,
    },
    #[error("stored integer cannot be represented by the public type")]
    StoredIntegerOutOfRange,
    #[error("SQLite runtime configuration is not active: {0}")]
    SqliteConfiguration(&'static str),
}

pub type PersonalStateResult<T> = Result<T, PersonalStateError>;
