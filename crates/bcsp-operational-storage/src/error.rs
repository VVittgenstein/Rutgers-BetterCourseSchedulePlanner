use bcsp_contracts::TraceId;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("SQLite operation failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("stored identity is invalid: {0}")]
    Identity(#[from] bcsp_contracts::IdentityError),
    #[error("FTS5 is unavailable in the active SQLite runtime")]
    Fts5Unavailable,
    #[error("required SQLite configuration is unavailable: {0}")]
    SqliteConfiguration(&'static str),
    #[error("the requested catalog target has no published content")]
    CatalogTargetNotPublished,
    #[error(
        "requested catalog content version {requested} does not match current version {current}"
    )]
    CatalogContentVersionMismatch { requested: u64, current: u64 },
    #[error("invalid write command field {field}: {reason}")]
    InvalidCommand {
        field: &'static str,
        reason: &'static str,
    },
    #[error(
        "operational migration filenames or identifiers are not a contiguous monotonic sequence"
    )]
    InvalidMigrationSequence,
    #[error("database contains unknown operational migration {migration_id} ({name})")]
    UnknownMigration { migration_id: u32, name: String },
    #[error("operational migration {migration_id} name differs from the embedded migration")]
    MigrationNameMismatch { migration_id: u32 },
    #[error("operational migration {migration_id} checksum differs from the embedded migration")]
    MigrationChecksumMismatch { migration_id: u32 },
    #[error("refresh observation {0} does not exist")]
    ObservationNotFound(TraceId),
    #[error("refresh observation {0} is not staged")]
    ObservationNotStaged(TraceId),
    #[error("refresh observation {0} is not in STARTED state")]
    ObservationNotStarted(TraceId),
    #[error("Open pull attempt {0} does not exist")]
    OpenAttemptNotFound(TraceId),
    #[error("Open pull attempt {0} is not in STARTED state")]
    OpenAttemptNotStarted(TraceId),
    #[error("Open pull attempt {0} is already active for this target")]
    OpenTargetInFlight(TraceId),
    #[error("Open pull attempt {0} was superseded by a later target attempt")]
    OpenAttemptSuperseded(TraceId),
    #[error("database row contains an unknown refresh status")]
    UnknownRefreshStatus,
    #[error("database row contains an invalid trace ID in {table}.{column}")]
    InvalidStoredTraceId {
        table: &'static str,
        column: &'static str,
    },
    #[error("database row contains an invalid timestamp in {table}.{column}")]
    InvalidStoredTimestamp {
        table: &'static str,
        column: &'static str,
    },
    #[error("database row contains an invalid projection in {table}.{field}: {reason}")]
    InvalidStoredProjection {
        table: &'static str,
        field: &'static str,
        reason: &'static str,
    },
    #[error("published rows contain inconsistent content versions for {entity_kind}")]
    InconsistentPublishedVersion { entity_kind: &'static str },
    #[error("published {entity_kind} has invalid discovery source ownership")]
    InvalidStoredSourceOwnership { entity_kind: &'static str },
    #[error("published {entity_kind} is orphaned")]
    OrphanedStoredEntity { entity_kind: &'static str },
    #[error("published state is missing its {scope} observation")]
    MissingPublishedObservation { scope: &'static str },
    #[error("stored numeric value is outside the supported range")]
    StoredIntegerOutOfRange,
    #[error("injected publish fault at {0}")]
    InjectedFault(&'static str),
}

pub type StorageResult<T> = Result<T, StorageError>;
