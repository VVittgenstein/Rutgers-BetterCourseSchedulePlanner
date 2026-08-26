use std::collections::{BTreeMap, BTreeSet};

use bcsp_contracts::{SectionKey, TermCampusKey, TraceId};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;
use time::{Date, Month, OffsetDateTime};

use crate::storage::{clear_staging, parse_stored_trace_id, publish_staged_in_transaction};
use crate::{
    EmptySnapshotDecision, OperationalStorage, PublishOutcome, StorageError, StorageResult,
};

const OPEN_DIAGNOSTIC_RETENTION_PER_TARGET: u64 = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenAttemptClassification {
    Started,
    ValidApplied,
    ValidEmptyNoRows,
    Failed,
    UnsafeEmpty,
    UnsafeZeroIntersection,
    SuspectPartialSnapshot,
    StaleCatalogRace,
    Interrupted,
}

impl OpenAttemptClassification {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Started => "STARTED",
            Self::ValidApplied => "VALID_APPLIED",
            Self::ValidEmptyNoRows => "VALID_EMPTY_NO_ROWS",
            Self::Failed => "FAILED",
            Self::UnsafeEmpty => "UNSAFE_EMPTY",
            Self::UnsafeZeroIntersection => "UNSAFE_ZERO_INTERSECTION",
            Self::SuspectPartialSnapshot => "SUSPECT_PARTIAL_SNAPSHOT",
            Self::StaleCatalogRace => "STALE_CATALOG_RACE",
            Self::Interrupted => "INTERRUPTED",
        }
    }

    fn parse(value: &str) -> StorageResult<Self> {
        match value {
            "STARTED" => Ok(Self::Started),
            "VALID_APPLIED" => Ok(Self::ValidApplied),
            "VALID_EMPTY_NO_ROWS" => Ok(Self::ValidEmptyNoRows),
            "FAILED" => Ok(Self::Failed),
            "UNSAFE_EMPTY" => Ok(Self::UnsafeEmpty),
            "UNSAFE_ZERO_INTERSECTION" => Ok(Self::UnsafeZeroIntersection),
            "SUSPECT_PARTIAL_SNAPSHOT" => Ok(Self::SuspectPartialSnapshot),
            "STALE_CATALOG_RACE" => Ok(Self::StaleCatalogRace),
            "INTERRUPTED" => Ok(Self::Interrupted),
            _ => Err(invalid_stored("open_pull_attempts", "classification")),
        }
    }

    pub const fn is_success(self) -> bool {
        matches!(self, Self::ValidApplied | Self::ValidEmptyNoRows)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenRequestLane {
    General,
    ActiveWatch,
    FirstLoad,
    ManualRefresh,
    CatalogRaceRecheck,
}

impl OpenRequestLane {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::General => "GENERAL",
            Self::ActiveWatch => "ACTIVE_WATCH",
            Self::FirstLoad => "FIRST_LOAD",
            Self::ManualRefresh => "MANUAL_REFRESH",
            Self::CatalogRaceRecheck => "CATALOG_RACE_RECHECK",
        }
    }

    fn parse(value: &str) -> StorageResult<Self> {
        match value {
            "GENERAL" => Ok(Self::General),
            "ACTIVE_WATCH" => Ok(Self::ActiveWatch),
            "FIRST_LOAD" => Ok(Self::FirstLoad),
            "MANUAL_REFRESH" => Ok(Self::ManualRefresh),
            "CATALOG_RACE_RECHECK" => Ok(Self::CatalogRaceRecheck),
            _ => Err(invalid_stored("open_pull_attempts", "lane")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenCacheStatus {
    NotApplicable,
    Hit,
    Miss,
    Revalidated,
    Bypass,
}

impl OpenCacheStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotApplicable => "NOT_APPLICABLE",
            Self::Hit => "HIT",
            Self::Miss => "MISS",
            Self::Revalidated => "REVALIDATED",
            Self::Bypass => "BYPASS",
        }
    }

    fn parse(value: &str) -> StorageResult<Self> {
        match value {
            "NOT_APPLICABLE" => Ok(Self::NotApplicable),
            "HIT" => Ok(Self::Hit),
            "MISS" => Ok(Self::Miss),
            "REVALIDATED" => Ok(Self::Revalidated),
            "BYPASS" => Ok(Self::Bypass),
            _ => Err(invalid_stored("open_pull_attempts", "cache_status")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenSectionState {
    Open,
    Closed,
}

impl OpenSectionState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "OPEN",
            Self::Closed => "CLOSED",
        }
    }

    fn parse(table: &'static str, value: &str) -> StorageResult<Self> {
        match value {
            "OPEN" => Ok(Self::Open),
            "CLOSED" => Ok(Self::Closed),
            _ => Err(invalid_stored(table, "state")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeginOpenPullAttemptCommand {
    pub attempt_id: TraceId,
    pub run_id: TraceId,
    pub target: TermCampusKey,
    pub captured_catalog_content_version: u64,
    pub rutgers_day: String,
    pub started_at: String,
    pub lane: OpenRequestLane,
    pub requested_interval_seconds: Option<u64>,
    pub effective_interval_seconds: Option<u64>,
    pub schedule_lag_ms: Option<u64>,
}

/// Redacted HTTP response facts retained for an Open attempt.
///
/// Header values are bounded audit text. Response-body bytes are never accepted;
/// only their decoded length and SHA-256 may be retained.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OpenHttpAuditMetadata {
    pub http_status: Option<u16>,
    pub cache_status: Option<OpenCacheStatus>,
    pub decoded_bytes: Option<u64>,
    pub decoded_body_sha256: Option<String>,
    pub content_type: Option<String>,
    pub etag: Option<String>,
    pub cache_control: Option<String>,
    pub date: Option<String>,
    pub age_seconds: Option<u64>,
    pub last_modified: Option<String>,
    pub retry_after: Option<String>,
    pub retry_after_seconds: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinishOpenPullSuccessCommand {
    pub attempt_id: TraceId,
    pub completed_at: String,
    /// Canonical sorted/deduplicated five-digit Section identities returned by the Open client.
    pub open_sections: Vec<SectionKey>,
    /// Count before client canonicalization; must be at least the unique identity count.
    pub source_value_count: u64,
    /// Current watched sections that require one observation event for this valid batch.
    pub watched_sections: Vec<SectionKey>,
    pub http: OpenHttpAuditMetadata,
    /// Integrity-gate directive computed under the per-target gate lock: when
    /// true, a would-be-applicable snapshot is withheld and classified
    /// SUSPECT_PARTIAL_SNAPSHOT instead (LKG retained, no events). Hard
    /// rejections ignore this flag.
    pub gate_hold: bool,
    /// Exact catalog section-set identity (sha-256 hex) the gate decision was
    /// computed against; `None` for ungated commits and hard rejections. The
    /// restart rebuild only resumes a suspect run whose members carry the
    /// identity of the current serving catalog.
    pub gate_catalog_set_identity: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinishOpenPullFailureCommand {
    pub attempt_id: TraceId,
    pub completed_at: String,
    pub http: OpenHttpAuditMetadata,
    pub error_code: String,
    pub diagnostic_token: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenCommitOutcome {
    pub attempt_id: TraceId,
    pub attempt_sequence: u64,
    pub classification: OpenAttemptClassification,
    pub refresh_observation_id: Option<TraceId>,
    pub observation_sequence: Option<u64>,
    pub catalog_content_version: u64,
    pub source_value_count: u64,
    pub catalog_section_count: u64,
    pub intersection_count: u64,
    pub orphan_count: u64,
    pub duplicate_count: u64,
    pub changed_section_count: u64,
    pub body_changed: bool,
    pub state_changed: bool,
    pub retained_lkg_attempt_id: Option<TraceId>,
    /// Present only for a valid atomically applied target observation.
    pub observation_commit: Option<OpenObservationCommit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenObservationCommit {
    pub rutgers_day: String,
    pub effective_interval_seconds: u64,
    pub run_counts: OpenAttemptCounters,
    pub target_day_counts: OpenAttemptCounters,
    pub service_day_counts: OpenAttemptCounters,
    pub section_events: Vec<OpenSectionEvent>,
}

/// Newest-first attempt summary consumed by the integrity gate's restart
/// rebuild (classification + canonical set hash + completion timestamp text
/// + the catalog section-set identity the gated attempt was decided against).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenGateAttemptSummary {
    pub classification: OpenAttemptClassification,
    pub canonical_set_sha256: Option<String>,
    pub completed_at: Option<String>,
    pub catalog_set_identity: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenAttemptRecord {
    pub attempt_id: TraceId,
    pub run_id: TraceId,
    pub target: TermCampusKey,
    pub attempt_sequence: u64,
    pub rutgers_day: String,
    pub captured_catalog_content_version: u64,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub classification: OpenAttemptClassification,
    pub lane: OpenRequestLane,
    pub http: OpenHttpAuditMetadata,
    pub canonical_set_sha256: Option<String>,
    pub state_sha256: Option<String>,
    pub source_value_count: u64,
    pub catalog_section_count: u64,
    pub intersection_count: u64,
    pub orphan_count: u64,
    pub duplicate_count: u64,
    pub requested_interval_seconds: Option<u64>,
    pub effective_interval_seconds: Option<u64>,
    pub schedule_lag_ms: Option<u64>,
    pub lkg_age_ms: Option<u64>,
    pub error_code: Option<String>,
    pub diagnostic_token: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenBatchState {
    pub target: TermCampusKey,
    pub last_attempt_sequence: u64,
    pub last_observation_sequence: u64,
    pub current_catalog_content_version: u64,
    pub lkg_attempt_id: Option<TraceId>,
    pub lkg_observation_sequence: Option<u64>,
    pub lkg_observed_at: Option<String>,
    pub lkg_canonical_set_sha256: Option<String>,
    pub lkg_state_sha256: Option<String>,
    pub last_attempt_id: Option<TraceId>,
    pub last_attempt_at: Option<String>,
    pub last_success_at: Option<String>,
    pub last_failure_attempt_sequence: Option<u64>,
    pub last_failure_at: Option<String>,
    pub last_failure_error_code: Option<String>,
    pub last_failure_http_status: Option<u16>,
    pub last_body_change_at: Option<String>,
    pub last_state_change_at: Option<String>,
    pub requested_interval_seconds: Option<u64>,
    pub effective_interval_seconds: Option<u64>,
    pub last_schedule_lag_ms: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenBatchObservation {
    pub attempt_id: TraceId,
    pub observation_id: TraceId,
    pub target: TermCampusKey,
    pub observation_sequence: u64,
    pub catalog_content_version: u64,
    pub observed_at: String,
    pub classification: OpenAttemptClassification,
    pub http: OpenHttpAuditMetadata,
    pub canonical_set_sha256: String,
    pub state_sha256: String,
    pub source_value_count: u64,
    pub catalog_section_count: u64,
    pub intersection_count: u64,
    pub orphan_count: u64,
    pub duplicate_count: u64,
    pub changed_section_count: u64,
    pub body_changed: bool,
    pub state_changed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenCatalogSnapshot {
    pub target: TermCampusKey,
    pub content_version: u64,
    pub sections: Vec<SectionKey>,
}

/// A staged Catalog candidate that can be reconciled with Open without exposing either half.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogCandidateOpenSnapshot {
    pub observation_id: TraceId,
    pub base_content_version: u64,
    pub catalog: OpenCatalogSnapshot,
}

/// Result of attempting to publish one Catalog/Open pair.
///
/// `catalog` is absent when the Open response was unsafe and the staged Catalog was discarded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteSnapshotCommitOutcome {
    pub catalog: Option<PublishOutcome>,
    pub open: OpenCommitOutcome,
}

/// Storage-derived usability for one `(term, campus)` target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompleteTargetSnapshotState {
    pub target: TermCampusKey,
    pub catalog_content_version: u64,
    pub open_catalog_content_version: u64,
    pub open_lkg_attempt_id: Option<TraceId>,
    pub ready: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenSectionCurrent {
    pub section: SectionKey,
    pub state: OpenSectionState,
    pub catalog_content_version: u64,
    pub attempt_id: TraceId,
    pub observation_sequence: u64,
    pub observed_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenSectionEvent {
    pub event_id: u64,
    pub observation_id: TraceId,
    pub refresh_observation_id: TraceId,
    pub attempt_id: TraceId,
    pub section: SectionKey,
    pub state: OpenSectionState,
    pub catalog_content_version: u64,
    pub observation_sequence: u64,
    pub observed_at: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OpenAttemptCounters {
    pub attempted: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub empty: u64,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct OpenRetentionReport {
    pub attempts_removed: u64,
    pub batch_observations_removed: u64,
    pub section_events_removed: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenCircuitState {
    Closed,
    RetryAfter,
    FatalDiagnostic,
}

impl OpenCircuitState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Closed => "CLOSED",
            Self::RetryAfter => "RETRY_AFTER",
            Self::FatalDiagnostic => "FATAL_DIAGNOSTIC",
        }
    }

    fn parse(value: &str) -> StorageResult<Self> {
        match value {
            "CLOSED" => Ok(Self::Closed),
            "RETRY_AFTER" => Ok(Self::RetryAfter),
            "FATAL_DIAGNOSTIC" => Ok(Self::FatalDiagnostic),
            _ => Err(invalid_stored("open_origin_state", "circuit_state")),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenOriginState {
    pub origin_id: String,
    pub circuit_state: OpenCircuitState,
    pub reason_code: Option<String>,
    pub opened_at: Option<String>,
    pub retry_at: Option<String>,
    pub diagnostic_recheck_required: bool,
    pub updated_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenScheduleState {
    pub target: TermCampusKey,
    pub requested_interval_seconds: u64,
    pub effective_interval_seconds: u64,
    pub next_due_at: String,
    pub last_scheduled_at: Option<String>,
    pub last_actual_start_at: Option<String>,
    pub schedule_lag_ms: u64,
    pub failure_streak: u64,
    pub updated_at: String,
}

#[derive(Debug)]
struct StartedOpenAttempt {
    target: TermCampusKey,
    run_id: TraceId,
    attempt_sequence: u64,
    rutgers_day: String,
    captured_catalog_content_version: u64,
    started_at: String,
    effective_interval_seconds: Option<u64>,
    candidate_catalog_observation_id: Option<TraceId>,
    candidate_base_content_version: Option<u64>,
}

#[derive(Debug)]
struct CanonicalOpenResponse {
    canonical_indices: BTreeSet<String>,
    source_value_count: u64,
    duplicate_count: u64,
    canonical_set_sha256: String,
}

impl OperationalStorage {
    /// Durably records request start before any upstream result is interpreted.
    /// Repeating the exact same command is idempotent and returns the original sequence.
    pub fn begin_open_pull_attempt(
        &mut self,
        command: &BeginOpenPullAttemptCommand,
    ) -> StorageResult<u64> {
        self.begin_open_pull_attempt_with_catalog(command, None)
    }

    /// Begins an Open attempt against staged Catalog rows rather than the serving Catalog.
    pub fn begin_candidate_open_pull_attempt(
        &mut self,
        candidate: &CatalogCandidateOpenSnapshot,
        command: &BeginOpenPullAttemptCommand,
    ) -> StorageResult<u64> {
        if command.target != candidate.catalog.target
            || command.captured_catalog_content_version != candidate.catalog.content_version
        {
            return Err(StorageError::InvalidCommand {
                field: "candidate_catalog",
                reason: "must match the Open attempt target and captured version",
            });
        }
        self.begin_open_pull_attempt_with_catalog(command, Some(candidate))
    }

    fn begin_open_pull_attempt_with_catalog(
        &mut self,
        command: &BeginOpenPullAttemptCommand,
        candidate: Option<&CatalogCandidateOpenSnapshot>,
    ) -> StorageResult<u64> {
        validate_timestamp("started_at", &command.started_at)?;
        validate_rutgers_day(&command.rutgers_day)?;
        validate_optional_positive(
            "requested_interval_seconds",
            command.requested_interval_seconds,
        )?;
        validate_optional_positive(
            "effective_interval_seconds",
            command.effective_interval_seconds,
        )?;
        let captured_version = positive_u64_to_i64(
            "captured_catalog_content_version",
            command.captured_catalog_content_version,
        )?;
        let requested_interval = optional_u64_to_i64(command.requested_interval_seconds)?;
        let effective_interval = optional_u64_to_i64(command.effective_interval_seconds)?;
        let schedule_lag = optional_u64_to_i64(command.schedule_lag_ms)?;
        let target_id = open_target_id(&command.target);
        let attempt_id = command.attempt_id.to_string();
        let run_id = command.run_id.to_string();

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let serving_version = transaction
            .query_row(
                "SELECT current_content_version FROM catalog_targets WHERE target_id = ?1",
                [&target_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map(stored_u64)
            .transpose()?
            .unwrap_or(0);
        let current_version = match candidate {
            Some(candidate) => {
                let staged_target_id = transaction
                    .query_row(
                        "SELECT target_id FROM catalog_refresh_observations
                         WHERE observation_id = ?1 AND status = 'STAGED'",
                        [candidate.observation_id.to_string()],
                        |row| row.get::<_, String>(0),
                    )
                    .optional()?
                    .ok_or(StorageError::ObservationNotStaged(candidate.observation_id))?;
                if staged_target_id != target_id
                    || serving_version != candidate.base_content_version
                {
                    return Err(StorageError::CatalogContentVersionMismatch {
                        requested: candidate.base_content_version,
                        current: serving_version,
                    });
                }
                candidate.catalog.content_version
            }
            None if serving_version > 0 => serving_version,
            None => return Err(StorageError::CatalogTargetNotPublished),
        };
        let existing = transaction
            .query_row(
                "SELECT target_id, run_id, attempt_sequence, rutgers_day,
                        captured_catalog_content_version, started_at, lane,
                        requested_interval_seconds, effective_interval_seconds, schedule_lag_ms,
                        candidate_catalog_observation_id, candidate_base_content_version
                 FROM open_pull_attempts WHERE attempt_id = ?1",
                [&attempt_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                        row.get::<_, Option<i64>>(8)?,
                        row.get::<_, Option<i64>>(9)?,
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, Option<i64>>(11)?,
                    ))
                },
            )
            .optional()?;
        if let Some(existing) = existing {
            let matches = existing.0 == target_id
                && existing.1 == run_id
                && existing.3 == command.rutgers_day
                && existing.4 == captured_version
                && existing.5 == command.started_at
                && existing.6 == command.lane.as_str()
                && existing.7 == requested_interval
                && existing.8 == effective_interval
                && existing.9 == schedule_lag
                && existing.10 == candidate.map(|candidate| candidate.observation_id.to_string())
                && existing.11
                    == candidate
                        .map(|candidate| u64_to_i64(candidate.base_content_version))
                        .transpose()?;
            if !matches {
                return Err(StorageError::InvalidCommand {
                    field: "attempt_id",
                    reason: "already identifies a different Open attempt",
                });
            }
            let sequence = stored_u64(existing.2)?;
            transaction.commit()?;
            return Ok(sequence);
        }

        let active_attempt_id = transaction
            .query_row(
                "SELECT attempt_id FROM open_pull_attempts
                 WHERE target_id = ?1 AND classification = 'STARTED'",
                [&target_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if let Some(active_attempt_id) = active_attempt_id {
            return Err(StorageError::OpenTargetInFlight(parse_stored_trace_id(
                "open_pull_attempts",
                "attempt_id",
                &active_attempt_id,
            )?));
        }

        if current_version != command.captured_catalog_content_version {
            return Err(StorageError::CatalogContentVersionMismatch {
                requested: command.captured_catalog_content_version,
                current: current_version,
            });
        }
        transaction.execute(
            "INSERT OR IGNORE INTO open_batch_state(target_id) VALUES (?1)",
            [&target_id],
        )?;

        let lkg_observed_at = transaction.query_row(
            "SELECT lkg_observed_at FROM open_batch_state WHERE target_id = ?1",
            [&target_id],
            |row| row.get::<_, Option<String>>(0),
        )?;
        let lkg_age_ms = lkg_observed_at
            .as_deref()
            .map(|observed_at| elapsed_milliseconds(observed_at, &command.started_at))
            .transpose()?
            .map(u64_to_i64)
            .transpose()?;

        let attempt_sequence = transaction.query_row(
            "UPDATE open_batch_state
             SET last_attempt_sequence = last_attempt_sequence + 1,
                 last_attempt_id = ?2,
                 last_attempt_at = ?3,
                 requested_interval_seconds = ?4,
                 effective_interval_seconds = ?5,
                 last_schedule_lag_ms = ?6
             WHERE target_id = ?1
             RETURNING last_attempt_sequence",
            params![
                target_id,
                attempt_id,
                command.started_at,
                requested_interval,
                effective_interval,
                schedule_lag,
            ],
            |row| row.get::<_, i64>(0),
        )?;
        transaction.execute(
            "INSERT INTO open_pull_attempts(
                attempt_id, target_id, run_id, attempt_sequence, rutgers_day,
                captured_catalog_content_version, started_at, classification, lane,
                requested_interval_seconds, effective_interval_seconds, schedule_lag_ms,
                lkg_age_ms, candidate_catalog_observation_id,
                candidate_base_content_version
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'STARTED', ?8, ?9, ?10, ?11, ?12,
                       ?13, ?14)",
            params![
                attempt_id,
                target_id,
                run_id,
                attempt_sequence,
                command.rutgers_day,
                captured_version,
                command.started_at,
                command.lane.as_str(),
                requested_interval,
                effective_interval,
                schedule_lag,
                lkg_age_ms,
                candidate.map(|candidate| candidate.observation_id.to_string()),
                candidate
                    .map(|candidate| u64_to_i64(candidate.base_content_version))
                    .transpose()?,
            ],
        )?;
        match candidate {
            Some(candidate) => {
                transaction.execute(
                    "INSERT INTO open_attempt_catalog_sections(attempt_id, section_index)
                     SELECT ?1, section_index FROM catalog_staging_sections
                     WHERE observation_id = ?2",
                    params![attempt_id, candidate.observation_id.to_string()],
                )?;
            }
            None => {
                transaction.execute(
                    "INSERT INTO open_attempt_catalog_sections(attempt_id, section_index)
                     SELECT ?1, section_index FROM catalog_sections
                     WHERE target_id = ?2 AND content_version = ?3",
                    params![attempt_id, target_id, captured_version],
                )?;
            }
        }
        increment_attempted_counters(&transaction, &target_id, &run_id, &command.rutgers_day)?;
        transaction.commit()?;
        stored_u64(attempt_sequence)
    }

    /// Classifies a validated official Open response and atomically applies it only while
    /// the captured Catalog content version is still current.
    pub fn finish_open_pull_success(
        &mut self,
        command: FinishOpenPullSuccessCommand,
    ) -> StorageResult<OpenCommitOutcome> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let outcome = Self::finish_open_pull_success_transaction(&transaction, command, false)?;
        transaction.commit()?;
        Ok(outcome)
    }

    fn finish_open_pull_success_transaction(
        transaction: &Transaction<'_>,
        command: FinishOpenPullSuccessCommand,
        accept_candidate_base_version: bool,
    ) -> StorageResult<OpenCommitOutcome> {
        validate_timestamp("completed_at", &command.completed_at)?;
        validate_open_http_audit(&command.http, true)?;
        let http_status = i64::from(required_success_http_status(&command.http)?);
        let cache_status = command.http.cache_status.map(OpenCacheStatus::as_str);
        let decoded_bytes = u64_to_i64(required_success_decoded_bytes(&command.http)?)?;
        let decoded_body_sha256 = required_success_header(
            "decoded_body_sha256",
            command.http.decoded_body_sha256.as_deref(),
        )?;
        let content_type =
            required_success_header("content_type", command.http.content_type.as_deref())?;
        let attempt_id = command.attempt_id.to_string();

        let attempt = load_started_open_attempt(transaction, command.attempt_id)?;
        ensure_started_open_attempt_is_current(transaction, &attempt, command.attempt_id)?;
        validate_timestamp_order(&attempt.started_at, &command.completed_at)?;
        let target_id = open_target_id(&attempt.target);
        let response = canonicalize_open_response(
            &attempt.target,
            &command.open_sections,
            command.source_value_count,
        )?;
        let retained_lkg_attempt_id = load_lkg_attempt_id(transaction, &target_id)?;
        let (lkg_canonical_set_sha256, lkg_state_sha256) =
            load_lkg_hashes(transaction, &target_id)?;
        let body_changed =
            lkg_canonical_set_sha256.as_deref() != Some(response.canonical_set_sha256.as_str());
        let current_catalog_content_version =
            load_current_catalog_version(transaction, &target_id)?;
        let catalog_indices = load_attempt_catalog_indices(transaction, command.attempt_id)?;
        let intersection_count = response
            .canonical_indices
            .intersection(&catalog_indices)
            .count() as u64;
        let orphan_count = response
            .canonical_indices
            .difference(&catalog_indices)
            .count() as u64;
        let catalog_section_count = catalog_indices.len() as u64;

        let candidate_base_matches = accept_candidate_base_version
            && attempt.candidate_base_content_version == Some(current_catalog_content_version);
        if current_catalog_content_version != attempt.captured_catalog_content_version
            && !candidate_base_matches
        {
            finalize_non_applied_attempt(
                transaction,
                &attempt,
                command.attempt_id,
                &command.completed_at,
                OpenAttemptClassification::StaleCatalogRace,
                &command.http,
                Some(&response),
                catalog_section_count,
                intersection_count,
                orphan_count,
                "CATALOG_CONTENT_VERSION_CHANGED",
                None,
                None,
            )?;
            prune_open_diagnostics_transaction(
                transaction,
                &target_id,
                &attempt.rutgers_day,
                OPEN_DIAGNOSTIC_RETENTION_PER_TARGET,
            )?;
            return Ok(OpenCommitOutcome {
                attempt_id: command.attempt_id,
                attempt_sequence: attempt.attempt_sequence,
                classification: OpenAttemptClassification::StaleCatalogRace,
                refresh_observation_id: None,
                observation_sequence: None,
                catalog_content_version: attempt.captured_catalog_content_version,
                source_value_count: response.source_value_count,
                catalog_section_count,
                intersection_count,
                orphan_count,
                duplicate_count: response.duplicate_count,
                changed_section_count: 0,
                body_changed,
                state_changed: false,
                retained_lkg_attempt_id,
                observation_commit: None,
            });
        }

        let classification = classify_open_response(
            &response.canonical_indices,
            &catalog_indices,
            intersection_count,
            command.gate_hold,
        );
        let intended_states = catalog_indices
            .iter()
            .map(|index| {
                let state = if response.canonical_indices.contains(index) {
                    OpenSectionState::Open
                } else {
                    OpenSectionState::Closed
                };
                (index.clone(), state)
            })
            .collect::<BTreeMap<_, _>>();
        let state_sha256 = open_state_sha256_v1(&attempt.target, &intended_states);
        let state_changed = lkg_state_sha256.as_deref() != Some(state_sha256.as_str());

        if !classification.is_success() {
            let error_code = match classification {
                OpenAttemptClassification::UnsafeEmpty => "UNSAFE_EMPTY_WITH_CATALOG_ROWS",
                OpenAttemptClassification::UnsafeZeroIntersection => "UNSAFE_ZERO_INTERSECTION",
                OpenAttemptClassification::SuspectPartialSnapshot => "SUSPECT_PARTIAL_SNAPSHOT",
                _ => unreachable!("only unsafe response classifications reach this branch"),
            };
            finalize_non_applied_attempt(
                transaction,
                &attempt,
                command.attempt_id,
                &command.completed_at,
                classification,
                &command.http,
                Some(&response),
                catalog_section_count,
                intersection_count,
                orphan_count,
                error_code,
                Some(&state_sha256),
                command.gate_catalog_set_identity.as_deref(),
            )?;
            prune_open_diagnostics_transaction(
                transaction,
                &target_id,
                &attempt.rutgers_day,
                OPEN_DIAGNOSTIC_RETENTION_PER_TARGET,
            )?;
            return Ok(OpenCommitOutcome {
                attempt_id: command.attempt_id,
                attempt_sequence: attempt.attempt_sequence,
                classification,
                refresh_observation_id: None,
                observation_sequence: None,
                catalog_content_version: attempt.captured_catalog_content_version,
                source_value_count: response.source_value_count,
                catalog_section_count,
                intersection_count,
                orphan_count,
                duplicate_count: response.duplicate_count,
                changed_section_count: 0,
                body_changed,
                state_changed,
                retained_lkg_attempt_id,
                observation_commit: None,
            });
        }

        let effective_interval_seconds = attempt
            .effective_interval_seconds
            .ok_or_else(|| invalid_stored("open_pull_attempts", "effective_interval_seconds"))?;

        let watched_indices = canonicalize_watched_sections(
            &attempt.target,
            &command.watched_sections,
            &catalog_indices,
        )?;
        let previous_states = load_current_state_map(transaction, &target_id)?;
        let changed_section_count = changed_state_count(&previous_states, &intended_states);
        let observation_sequence = transaction.query_row(
            "SELECT last_observation_sequence + 1 FROM open_batch_state WHERE target_id = ?1",
            [&target_id],
            |row| row.get::<_, i64>(0),
        )?;
        let observation_sequence_u64 = stored_u64(observation_sequence)?;
        let captured_version = u64_to_i64(attempt.captured_catalog_content_version)?;
        let refresh_observation_id = derive_open_refresh_observation_id(command.attempt_id)?;

        transaction.execute(
            "INSERT INTO open_batch_observations(
                attempt_id, observation_id, target_id, observation_sequence, catalog_content_version,
                observed_at, classification, http_status, cache_status, decoded_bytes,
                decoded_body_sha256, content_type, etag, cache_control, response_date,
                age_seconds, last_modified, retry_after, retry_after_seconds,
                canonical_set_sha256, state_sha256,
                source_value_count, catalog_section_count, intersection_count,
                orphan_count, duplicate_count, changed_section_count, body_changed, state_changed
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23,
                ?24, ?25, ?26, ?27, ?28, ?29
             )",
            params![
                attempt_id,
                refresh_observation_id.to_string(),
                target_id,
                observation_sequence,
                captured_version,
                command.completed_at,
                classification.as_str(),
                http_status,
                cache_status,
                decoded_bytes,
                decoded_body_sha256,
                content_type,
                command.http.etag.as_deref(),
                command.http.cache_control.as_deref(),
                command.http.date.as_deref(),
                optional_u64_to_i64(command.http.age_seconds)?,
                command.http.last_modified.as_deref(),
                command.http.retry_after.as_deref(),
                optional_u64_to_i64(command.http.retry_after_seconds)?,
                response.canonical_set_sha256,
                state_sha256,
                u64_to_i64(response.source_value_count)?,
                u64_to_i64(catalog_section_count)?,
                u64_to_i64(intersection_count)?,
                u64_to_i64(orphan_count)?,
                u64_to_i64(response.duplicate_count)?,
                u64_to_i64(changed_section_count)?,
                i64::from(body_changed),
                i64::from(state_changed),
            ],
        )?;
        transaction.execute(
            "DELETE FROM open_section_current WHERE target_id = ?1",
            [&target_id],
        )?;
        {
            let mut statement = transaction.prepare_cached(
                "INSERT INTO open_section_current(
                    target_id, section_index, state, catalog_content_version,
                    attempt_id, observation_sequence, observed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for (index, state) in &intended_states {
                statement.execute(params![
                    target_id,
                    index,
                    state.as_str(),
                    captured_version,
                    attempt_id,
                    observation_sequence,
                    command.completed_at,
                ])?;
            }
        }
        let mut section_events = Vec::with_capacity(watched_indices.len());
        for index in watched_indices {
            let state = intended_states
                .get(&index)
                .copied()
                .ok_or_else(|| invalid_stored("open_batch_observations", "watched_sections"))?;
            let observation_id = derive_open_section_observation_id(
                command.attempt_id,
                &SectionKey::try_new(
                    attempt.target.term().as_str(),
                    attempt.target.campus().as_str(),
                    &index,
                )?,
            )?;
            let event_id = transaction.query_row(
                "INSERT INTO open_section_events(
                    observation_id, refresh_observation_id, attempt_id, target_id,
                    section_index, state, catalog_content_version, observation_sequence, observed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                 RETURNING event_id",
                params![
                    observation_id.to_string(),
                    refresh_observation_id.to_string(),
                    attempt_id,
                    target_id,
                    index,
                    state.as_str(),
                    captured_version,
                    observation_sequence,
                    command.completed_at,
                ],
                |row| row.get::<_, i64>(0),
            )?;
            section_events.push(OpenSectionEvent {
                event_id: stored_u64(event_id)?,
                observation_id,
                refresh_observation_id,
                attempt_id: command.attempt_id,
                section: SectionKey::try_new(
                    attempt.target.term().as_str(),
                    attempt.target.campus().as_str(),
                    &index,
                )?,
                state,
                catalog_content_version: attempt.captured_catalog_content_version,
                observation_sequence: observation_sequence_u64,
                observed_at: command.completed_at.clone(),
            });
        }
        let attempt_updated = transaction.execute(
            "UPDATE open_pull_attempts
             SET completed_at = ?2, classification = ?3, http_status = ?4, cache_status = ?5,
                 decoded_bytes = ?6, decoded_body_sha256 = ?7, content_type = ?8,
                 etag = ?9, cache_control = ?10, response_date = ?11, age_seconds = ?12,
                 last_modified = ?13, retry_after = ?14, retry_after_seconds = ?15,
                 canonical_set_sha256 = ?16, state_sha256 = ?17,
                 source_value_count = ?18, catalog_section_count = ?19,
                 intersection_count = ?20, orphan_count = ?21, duplicate_count = ?22,
                 gate_catalog_set_identity = ?23,
                 error_code = NULL, diagnostic_token = NULL
             WHERE attempt_id = ?1 AND classification = 'STARTED'",
            params![
                attempt_id,
                command.completed_at,
                classification.as_str(),
                http_status,
                cache_status,
                decoded_bytes,
                decoded_body_sha256,
                content_type,
                command.http.etag,
                command.http.cache_control,
                command.http.date,
                optional_u64_to_i64(command.http.age_seconds)?,
                command.http.last_modified,
                command.http.retry_after,
                optional_u64_to_i64(command.http.retry_after_seconds)?,
                response.canonical_set_sha256,
                state_sha256,
                u64_to_i64(response.source_value_count)?,
                u64_to_i64(catalog_section_count)?,
                u64_to_i64(intersection_count)?,
                u64_to_i64(orphan_count)?,
                u64_to_i64(response.duplicate_count)?,
                command.gate_catalog_set_identity,
            ],
        )?;
        if attempt_updated != 1 {
            return Err(StorageError::OpenAttemptNotStarted(command.attempt_id));
        }
        let batch_updated = transaction.execute(
            "UPDATE open_batch_state
             SET last_observation_sequence = ?2,
                 current_catalog_content_version = ?3,
                 lkg_attempt_id = ?4,
                 lkg_observation_sequence = ?2,
                 lkg_observed_at = ?5,
                 last_body_change_at = CASE
                     WHEN lkg_canonical_set_sha256 IS NULL OR lkg_canonical_set_sha256 != ?6
                     THEN ?5 ELSE last_body_change_at END,
                 last_state_change_at = CASE
                     WHEN lkg_state_sha256 IS NULL OR lkg_state_sha256 != ?7
                     THEN ?5 ELSE last_state_change_at END,
                 lkg_canonical_set_sha256 = ?6,
                 lkg_state_sha256 = ?7,
                 last_success_at = ?5
             WHERE target_id = ?1
               AND last_attempt_sequence = ?8
               AND last_attempt_id = ?4",
            params![
                target_id,
                observation_sequence,
                captured_version,
                attempt_id,
                command.completed_at,
                response.canonical_set_sha256,
                state_sha256,
                u64_to_i64(attempt.attempt_sequence)?,
            ],
        )?;
        if batch_updated != 1 {
            return Err(StorageError::OpenAttemptSuperseded(command.attempt_id));
        }
        increment_final_counters(
            transaction,
            &target_id,
            &attempt.run_id.to_string(),
            &attempt.rutgers_day,
            true,
            response.canonical_indices.is_empty(),
        )?;
        let committed_counters = load_committed_open_counters(
            transaction,
            &target_id,
            &attempt.run_id.to_string(),
            &attempt.rutgers_day,
        )?;
        prune_open_diagnostics_transaction(
            transaction,
            &target_id,
            &attempt.rutgers_day,
            OPEN_DIAGNOSTIC_RETENTION_PER_TARGET,
        )?;
        Ok(OpenCommitOutcome {
            attempt_id: command.attempt_id,
            attempt_sequence: attempt.attempt_sequence,
            classification,
            refresh_observation_id: Some(refresh_observation_id),
            observation_sequence: Some(observation_sequence_u64),
            catalog_content_version: attempt.captured_catalog_content_version,
            source_value_count: response.source_value_count,
            catalog_section_count,
            intersection_count,
            orphan_count,
            duplicate_count: response.duplicate_count,
            changed_section_count,
            body_changed,
            state_changed,
            retained_lkg_attempt_id: Some(command.attempt_id),
            observation_commit: Some(OpenObservationCommit {
                rutgers_day: attempt.rutgers_day,
                effective_interval_seconds,
                run_counts: committed_counters.0,
                target_day_counts: committed_counters.1,
                service_day_counts: committed_counters.2,
                section_events,
            }),
        })
    }

    /// Atomically publishes a staged Catalog and the Open result reconciled against it.
    ///
    /// Unsafe Open evidence finalizes the Open attempt for diagnostics but discards the staged
    /// Catalog, leaving the last complete serving pair untouched.
    pub fn finish_candidate_open_pull_success(
        &mut self,
        catalog_observation_id: TraceId,
        empty_decision: EmptySnapshotDecision,
        mut command: FinishOpenPullSuccessCommand,
    ) -> StorageResult<CompleteSnapshotCommitOutcome> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let attempt = load_started_open_attempt(&transaction, command.attempt_id)?;
        ensure_started_open_attempt_is_current(&transaction, &attempt, command.attempt_id)?;
        if attempt.candidate_catalog_observation_id != Some(catalog_observation_id) {
            return Err(StorageError::InvalidCommand {
                field: "candidate_catalog_observation_id",
                reason: "must match the staged Catalog bound to the Open attempt",
            });
        }
        let target_id = open_target_id(&attempt.target);
        let current_version = stored_u64(transaction.query_row(
            "SELECT current_content_version FROM catalog_targets WHERE target_id = ?1",
            [&target_id],
            |row| row.get::<_, i64>(0),
        )?)?;
        let candidate_base = attempt.candidate_base_content_version.ok_or(
            StorageError::InvalidStoredProjection {
                table: "open_pull_attempts",
                field: "candidate_base_content_version",
                reason: "candidate attempts require a base Catalog version",
            },
        )?;
        if current_version != candidate_base {
            return Err(StorageError::CatalogContentVersionMismatch {
                requested: candidate_base,
                current: current_version,
            });
        }

        let response = canonicalize_open_response(
            &attempt.target,
            &command.open_sections,
            command.source_value_count,
        )?;
        let catalog_indices = load_attempt_catalog_indices(&transaction, command.attempt_id)?;
        command.watched_sections.retain(|section| {
            section.target() == attempt.target && catalog_indices.contains(section.index().as_str())
        });
        let intersection_count = response
            .canonical_indices
            .intersection(&catalog_indices)
            .count() as u64;
        let classification = classify_open_response(
            &response.canonical_indices,
            &catalog_indices,
            intersection_count,
            command.gate_hold,
        );

        if !classification.is_success() {
            let completed_at = command.completed_at.clone();
            let open = Self::finish_open_pull_success_transaction(&transaction, command, true)?;
            clear_staging(&transaction, catalog_observation_id)?;
            let updated = transaction.execute(
                "UPDATE catalog_refresh_observations
                 SET status = 'FAILED', completed_at = ?2,
                     error_stage = 'SCHEMA', error_code = 'OPEN_CANDIDATE_UNSAFE',
                     diagnostic_token = NULL
                 WHERE observation_id = ?1 AND status = 'STAGED'",
                params![catalog_observation_id.to_string(), completed_at],
            )?;
            if updated != 1 {
                return Err(StorageError::ObservationNotStaged(catalog_observation_id));
            }
            transaction.commit()?;
            return Ok(CompleteSnapshotCommitOutcome {
                catalog: None,
                open,
            });
        }

        let catalog =
            publish_staged_in_transaction(&transaction, catalog_observation_id, empty_decision)?;
        let published_version = publish_outcome_content_version(&catalog);
        if published_version != attempt.captured_catalog_content_version
            || matches!(catalog, PublishOutcome::SuspectEmptyRetained { .. })
        {
            return Err(StorageError::CatalogContentVersionMismatch {
                requested: attempt.captured_catalog_content_version,
                current: published_version,
            });
        }
        let open = Self::finish_open_pull_success_transaction(&transaction, command, false)?;
        if !open.classification.is_success() || open.catalog_content_version != published_version {
            return Err(StorageError::InvalidStoredProjection {
                table: "open_batch_state",
                field: "complete snapshot",
                reason: "Open did not commit against the published Catalog candidate",
            });
        }
        transaction.commit()?;
        Ok(CompleteSnapshotCommitOutcome {
            catalog: Some(catalog),
            open,
        })
    }

    /// Derives target usability from the two serving version pointers. No compatibility flag can
    /// promote Catalog-only or Open-only RC2 data to READY.
    pub fn complete_target_snapshot_state(
        &self,
        target: &TermCampusKey,
    ) -> StorageResult<CompleteTargetSnapshotState> {
        let target_id = open_target_id(target);
        let catalog_content_version = self
            .connection
            .query_row(
                "SELECT current_content_version FROM catalog_targets WHERE target_id = ?1",
                [&target_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map(stored_u64)
            .transpose()?
            .unwrap_or(0);
        let open = self
            .connection
            .query_row(
                "SELECT current_catalog_content_version, lkg_attempt_id
                 FROM open_batch_state WHERE target_id = ?1",
                [&target_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        let (open_catalog_content_version, open_lkg_attempt_id) = match open {
            Some((version, attempt_id)) => (
                stored_u64(version)?,
                parse_optional_trace("open_batch_state", "lkg_attempt_id", attempt_id)?,
            ),
            None => (0, None),
        };
        let ready = catalog_content_version > 0
            && open_lkg_attempt_id.is_some()
            && open_catalog_content_version == catalog_content_version;
        Ok(CompleteTargetSnapshotState {
            target: target.clone(),
            catalog_content_version,
            open_catalog_content_version,
            open_lkg_attempt_id,
            ready,
        })
    }

    pub fn finish_open_pull_failure(
        &mut self,
        command: &FinishOpenPullFailureCommand,
    ) -> StorageResult<OpenCommitOutcome> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let outcome = Self::finish_open_pull_failure_transaction(&transaction, command)?;
        transaction.commit()?;
        Ok(outcome)
    }

    pub fn finish_candidate_open_pull_failure(
        &mut self,
        catalog_observation_id: TraceId,
        command: &FinishOpenPullFailureCommand,
    ) -> StorageResult<OpenCommitOutcome> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let attempt = load_started_open_attempt(&transaction, command.attempt_id)?;
        if attempt.candidate_catalog_observation_id != Some(catalog_observation_id) {
            return Err(StorageError::InvalidCommand {
                field: "candidate_catalog_observation_id",
                reason: "must match the staged Catalog bound to the Open attempt",
            });
        }
        let outcome = Self::finish_open_pull_failure_transaction(&transaction, command)?;
        clear_staging(&transaction, catalog_observation_id)?;
        let updated = transaction.execute(
            "UPDATE catalog_refresh_observations
             SET status = 'FAILED', completed_at = ?2,
                 error_stage = 'TRANSPORT', error_code = 'OPEN_CANDIDATE_FAILED',
                 diagnostic_token = NULL
             WHERE observation_id = ?1 AND status = 'STAGED'",
            params![catalog_observation_id.to_string(), command.completed_at],
        )?;
        if updated != 1 {
            return Err(StorageError::ObservationNotStaged(catalog_observation_id));
        }
        transaction.commit()?;
        Ok(outcome)
    }

    fn finish_open_pull_failure_transaction(
        transaction: &Transaction<'_>,
        command: &FinishOpenPullFailureCommand,
    ) -> StorageResult<OpenCommitOutcome> {
        validate_timestamp("completed_at", &command.completed_at)?;
        validate_safe_code("error_code", &command.error_code)?;
        validate_optional_safe_token("diagnostic_token", command.diagnostic_token.as_deref())?;
        validate_open_http_audit(&command.http, false)?;
        let attempt = load_started_open_attempt(transaction, command.attempt_id)?;
        ensure_started_open_attempt_is_current(transaction, &attempt, command.attempt_id)?;
        validate_timestamp_order(&attempt.started_at, &command.completed_at)?;
        let target_id = open_target_id(&attempt.target);
        let retained_lkg_attempt_id = load_lkg_attempt_id(transaction, &target_id)?;
        let catalog_section_count =
            load_attempt_catalog_indices(transaction, command.attempt_id)?.len() as u64;
        finalize_non_applied_attempt(
            transaction,
            &attempt,
            command.attempt_id,
            &command.completed_at,
            OpenAttemptClassification::Failed,
            &command.http,
            None,
            catalog_section_count,
            0,
            0,
            &command.error_code,
            None,
            None,
        )?;
        transaction.execute(
            "UPDATE open_pull_attempts SET diagnostic_token = ?2 WHERE attempt_id = ?1",
            params![command.attempt_id.to_string(), command.diagnostic_token],
        )?;
        prune_open_diagnostics_transaction(
            transaction,
            &target_id,
            &attempt.rutgers_day,
            OPEN_DIAGNOSTIC_RETENTION_PER_TARGET,
        )?;
        Ok(OpenCommitOutcome {
            attempt_id: command.attempt_id,
            attempt_sequence: attempt.attempt_sequence,
            classification: OpenAttemptClassification::Failed,
            refresh_observation_id: None,
            observation_sequence: None,
            catalog_content_version: attempt.captured_catalog_content_version,
            source_value_count: 0,
            catalog_section_count,
            intersection_count: 0,
            orphan_count: 0,
            duplicate_count: 0,
            changed_section_count: 0,
            body_changed: false,
            state_changed: false,
            retained_lkg_attempt_id,
            observation_commit: None,
        })
    }

    /// Integrity-gate seeding read: the last-applied OPEN index set for a
    /// target from the serving current-state mirror. `None` when no state
    /// was ever applied (distinct from an applied state with zero opens).
    pub fn serving_lkg_open_index_set(
        &self,
        target: &TermCampusKey,
    ) -> StorageResult<Option<std::collections::BTreeSet<String>>> {
        let target_id = open_target_id(target);
        let any_state: bool = self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM open_section_current WHERE target_id = ?1)",
            [&target_id],
            |row| row.get(0),
        )?;
        if !any_state {
            return Ok(None);
        }
        let mut statement = self.connection.prepare(
            "SELECT section_index FROM open_section_current
             WHERE target_id = ?1 AND state = 'OPEN'",
        )?;
        let rows = statement.query_map([&target_id], |row| row.get::<_, String>(0))?;
        let mut open_set = std::collections::BTreeSet::new();
        for row in rows {
            open_set.insert(row?);
        }
        Ok(Some(open_set))
    }

    /// Integrity-gate restart read: newest-first completed attempt summaries
    /// (classification, canonical set hash, completion time, gate catalog
    /// identity) for the restart rebuild rules.
    ///
    /// Candidate attempt rows are split by what they did to the serving
    /// state: UNPUBLISHED candidates (Hold / unsafe / failed / interrupted)
    /// were decided by an independent runtime against a catalog that never
    /// served, so they are invisible to the serving history. A SUCCESSFUL
    /// candidate, however, atomically published its catalog and promoted its
    /// runtime to serving -- that closed any live quarantine, so its row
    /// MUST remain in the stream as a non-suspect run breaker. Filtering it
    /// out would stitch pre-publish suspects onto a post-publish episode
    /// that the live runtime had already reset.
    pub fn recent_open_gate_attempt_summaries(
        &self,
        target: &TermCampusKey,
        limit: u32,
    ) -> StorageResult<Vec<OpenGateAttemptSummary>> {
        let mut statement = self.connection.prepare(
            "SELECT classification, canonical_set_sha256, completed_at,
                    gate_catalog_set_identity
             FROM open_pull_attempts
             WHERE target_id = ?1 AND classification != 'STARTED'
               AND (candidate_catalog_observation_id IS NULL
                    OR classification IN ('VALID_APPLIED', 'VALID_EMPTY_NO_ROWS'))
             ORDER BY attempt_sequence DESC
             LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![open_target_id(target), limit],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )?;
        let mut summaries = Vec::new();
        for row in rows {
            let (classification, canonical_set_sha256, completed_at, catalog_set_identity) = row?;
            summaries.push(OpenGateAttemptSummary {
                classification: OpenAttemptClassification::parse(&classification)?,
                canonical_set_sha256,
                completed_at,
                catalog_set_identity,
            });
        }
        Ok(summaries)
    }

    pub fn open_attempt(&self, attempt_id: &TraceId) -> StorageResult<Option<OpenAttemptRecord>> {
        let mut statement = self.connection.prepare(
            "SELECT a.attempt_id, a.run_id, t.term_id, t.campus_code,
                    a.attempt_sequence, a.rutgers_day, a.captured_catalog_content_version,
                    a.started_at, a.completed_at, a.classification, a.lane,
                    a.http_status, a.cache_status, a.decoded_bytes, a.decoded_body_sha256,
                    a.content_type, a.etag, a.cache_control, a.response_date, a.age_seconds,
                    a.last_modified, a.retry_after, a.retry_after_seconds,
                    a.canonical_set_sha256, a.state_sha256,
                    a.source_value_count, a.catalog_section_count, a.intersection_count,
                    a.orphan_count, a.duplicate_count, a.requested_interval_seconds,
                    a.effective_interval_seconds, a.schedule_lag_ms, a.lkg_age_ms,
                    a.error_code, a.diagnostic_token
             FROM open_pull_attempts a
             JOIN catalog_targets t ON t.target_id = a.target_id
             WHERE a.attempt_id = ?1",
        )?;
        statement
            .query_row([attempt_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, Option<i64>>(11)?,
                    row.get::<_, Option<String>>(12)?,
                    row.get::<_, Option<i64>>(13)?,
                    row.get::<_, Option<String>>(14)?,
                    row.get::<_, Option<String>>(15)?,
                    row.get::<_, Option<String>>(16)?,
                    row.get::<_, Option<String>>(17)?,
                    row.get::<_, Option<String>>(18)?,
                    row.get::<_, Option<i64>>(19)?,
                    row.get::<_, Option<String>>(20)?,
                    row.get::<_, Option<String>>(21)?,
                    row.get::<_, Option<i64>>(22)?,
                    row.get::<_, Option<String>>(23)?,
                    row.get::<_, Option<String>>(24)?,
                    row.get::<_, i64>(25)?,
                    row.get::<_, i64>(26)?,
                    row.get::<_, i64>(27)?,
                    row.get::<_, i64>(28)?,
                    row.get::<_, i64>(29)?,
                    row.get::<_, Option<i64>>(30)?,
                    row.get::<_, Option<i64>>(31)?,
                    row.get::<_, Option<i64>>(32)?,
                    row.get::<_, Option<i64>>(33)?,
                    row.get::<_, Option<String>>(34)?,
                    row.get::<_, Option<String>>(35)?,
                ))
            })
            .optional()?
            .map(|row| {
                Ok(OpenAttemptRecord {
                    attempt_id: parse_stored_trace_id("open_pull_attempts", "attempt_id", &row.0)?,
                    run_id: parse_stored_trace_id("open_pull_attempts", "run_id", &row.1)?,
                    target: TermCampusKey::try_new(&row.2, &row.3)?,
                    attempt_sequence: stored_u64(row.4)?,
                    rutgers_day: row.5,
                    captured_catalog_content_version: stored_u64(row.6)?,
                    started_at: row.7,
                    completed_at: row.8,
                    classification: OpenAttemptClassification::parse(&row.9)?,
                    lane: OpenRequestLane::parse(&row.10)?,
                    http: OpenHttpAuditMetadata {
                        http_status: row.11.map(stored_u16).transpose()?,
                        cache_status: row.12.as_deref().map(OpenCacheStatus::parse).transpose()?,
                        decoded_bytes: row.13.map(stored_u64).transpose()?,
                        decoded_body_sha256: row.14,
                        content_type: row.15,
                        etag: row.16,
                        cache_control: row.17,
                        date: row.18,
                        age_seconds: row.19.map(stored_u64).transpose()?,
                        last_modified: row.20,
                        retry_after: row.21,
                        retry_after_seconds: row.22.map(stored_u64).transpose()?,
                    },
                    canonical_set_sha256: row.23,
                    state_sha256: row.24,
                    source_value_count: stored_u64(row.25)?,
                    catalog_section_count: stored_u64(row.26)?,
                    intersection_count: stored_u64(row.27)?,
                    orphan_count: stored_u64(row.28)?,
                    duplicate_count: stored_u64(row.29)?,
                    requested_interval_seconds: row.30.map(stored_u64).transpose()?,
                    effective_interval_seconds: row.31.map(stored_u64).transpose()?,
                    schedule_lag_ms: row.32.map(stored_u64).transpose()?,
                    lkg_age_ms: row.33.map(stored_u64).transpose()?,
                    error_code: row.34,
                    diagnostic_token: row.35,
                })
            })
            .transpose()
    }

    pub fn open_batch_state(
        &self,
        target: &TermCampusKey,
    ) -> StorageResult<Option<OpenBatchState>> {
        let row = self
            .connection
            .query_row(
                "SELECT t.term_id, t.campus_code, s.last_attempt_sequence,
                        s.last_observation_sequence, s.current_catalog_content_version,
                        s.lkg_attempt_id, s.lkg_observation_sequence, s.lkg_observed_at,
                        s.lkg_canonical_set_sha256, s.lkg_state_sha256,
                        s.last_attempt_id, s.last_attempt_at, s.last_success_at,
                        s.last_failure_attempt_sequence, s.last_failure_at,
                        s.last_failure_error_code, s.last_failure_http_status,
                        s.last_body_change_at, s.last_state_change_at,
                        s.requested_interval_seconds, s.effective_interval_seconds,
                        s.last_schedule_lag_ms
                 FROM open_batch_state s
                 JOIN catalog_targets t ON t.target_id = s.target_id
                 WHERE s.target_id = ?1",
                [open_target_id(target)],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<i64>>(6)?,
                        row.get::<_, Option<String>>(7)?,
                        row.get::<_, Option<String>>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, Option<String>>(11)?,
                        row.get::<_, Option<String>>(12)?,
                        row.get::<_, Option<i64>>(13)?,
                        row.get::<_, Option<String>>(14)?,
                        row.get::<_, Option<String>>(15)?,
                        row.get::<_, Option<i64>>(16)?,
                        row.get::<_, Option<String>>(17)?,
                        row.get::<_, Option<String>>(18)?,
                        row.get::<_, Option<i64>>(19)?,
                        row.get::<_, Option<i64>>(20)?,
                        row.get::<_, Option<i64>>(21)?,
                    ))
                },
            )
            .optional()?;
        row.map(|row| {
            let last_failure_attempt_sequence = row.13.map(stored_u64).transpose()?;
            let last_failure_http_status = row.16.map(stored_u16).transpose()?;
            let failure_snapshot_is_valid = match (
                last_failure_attempt_sequence,
                row.14.as_ref(),
                row.15.as_ref(),
            ) {
                (None, None, None) => last_failure_http_status.is_none(),
                (Some(_), Some(_), Some(_)) => true,
                _ => false,
            };
            if !failure_snapshot_is_valid {
                return Err(invalid_stored("open_batch_state", "last_failure_snapshot"));
            }
            Ok(OpenBatchState {
                target: TermCampusKey::try_new(&row.0, &row.1)?,
                last_attempt_sequence: stored_u64(row.2)?,
                last_observation_sequence: stored_u64(row.3)?,
                current_catalog_content_version: stored_u64(row.4)?,
                lkg_attempt_id: parse_optional_trace("open_batch_state", "lkg_attempt_id", row.5)?,
                lkg_observation_sequence: row.6.map(stored_u64).transpose()?,
                lkg_observed_at: row.7,
                lkg_canonical_set_sha256: row.8,
                lkg_state_sha256: row.9,
                last_attempt_id: parse_optional_trace(
                    "open_batch_state",
                    "last_attempt_id",
                    row.10,
                )?,
                last_attempt_at: row.11,
                last_success_at: row.12,
                last_failure_attempt_sequence,
                last_failure_at: row.14,
                last_failure_error_code: row.15,
                last_failure_http_status,
                last_body_change_at: row.17,
                last_state_change_at: row.18,
                requested_interval_seconds: row.19.map(stored_u64).transpose()?,
                effective_interval_seconds: row.20.map(stored_u64).transpose()?,
                last_schedule_lag_ms: row.21.map(stored_u64).transpose()?,
            })
        })
        .transpose()
    }

    /// Reads the published Catalog version and its serving Section identities from one
    /// deferred transaction. Callers may reconcile outside the transaction; the Open commit
    /// path independently compares this captured version again under an immediate transaction.
    pub fn serving_open_catalog_snapshot(
        &mut self,
        target: &TermCampusKey,
    ) -> StorageResult<Option<OpenCatalogSnapshot>> {
        let target_id = open_target_id(target);
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)?;
        let published = transaction
            .query_row(
                "SELECT current_content_version, section_count
                 FROM catalog_targets WHERE target_id = ?1",
                [&target_id],
                |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?
            .map(|row| -> StorageResult<_> { Ok((stored_u64(row.0)?, stored_u64(row.1)?)) })
            .transpose()?;
        let Some((content_version, expected_section_count)) =
            published.filter(|(version, _)| *version > 0)
        else {
            transaction.commit()?;
            return Ok(None);
        };
        let sections = {
            let mut statement = transaction.prepare(
                "SELECT section_index FROM catalog_sections
                 WHERE target_id = ?1 AND content_version = ?2 ORDER BY section_index",
            )?;
            let rows = statement
                .query_map(params![target_id, u64_to_i64(content_version)?], |row| {
                    row.get::<_, String>(0)
                })?;
            rows.map(|row| {
                SectionKey::try_new(target.term().as_str(), target.campus().as_str(), &row?)
                    .map_err(StorageError::from)
            })
            .collect::<StorageResult<Vec<_>>>()?
        };
        if u64::try_from(sections.len()).map_err(|_| StorageError::StoredIntegerOutOfRange)?
            != expected_section_count
        {
            return Err(invalid_stored("catalog_targets", "section_count"));
        }
        transaction.commit()?;
        Ok(Some(OpenCatalogSnapshot {
            target: target.clone(),
            content_version,
            sections,
        }))
    }

    pub fn open_batch_observation(
        &self,
        attempt_id: &TraceId,
    ) -> StorageResult<Option<OpenBatchObservation>> {
        let row = self
            .connection
            .query_row(
                "SELECT b.attempt_id, b.observation_id, t.term_id, t.campus_code,
                        b.observation_sequence, b.catalog_content_version, b.observed_at,
                        b.classification, b.http_status, b.cache_status, b.decoded_bytes,
                        b.decoded_body_sha256, b.content_type, b.etag, b.cache_control,
                        b.response_date, b.age_seconds, b.last_modified, b.retry_after,
                        b.retry_after_seconds,
                        b.canonical_set_sha256, b.state_sha256, b.source_value_count,
                        b.catalog_section_count, b.intersection_count, b.orphan_count,
                        b.duplicate_count, b.changed_section_count, b.body_changed, b.state_changed
                 FROM open_batch_observations b
                 JOIN catalog_targets t ON t.target_id = b.target_id
                 WHERE b.attempt_id = ?1",
                [attempt_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, String>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, Option<String>>(9)?,
                        row.get::<_, i64>(10)?,
                        row.get::<_, String>(11)?,
                        row.get::<_, String>(12)?,
                        row.get::<_, Option<String>>(13)?,
                        row.get::<_, Option<String>>(14)?,
                        row.get::<_, Option<String>>(15)?,
                        row.get::<_, Option<i64>>(16)?,
                        row.get::<_, Option<String>>(17)?,
                        row.get::<_, Option<String>>(18)?,
                        row.get::<_, Option<i64>>(19)?,
                        row.get::<_, String>(20)?,
                        row.get::<_, String>(21)?,
                        row.get::<_, i64>(22)?,
                        row.get::<_, i64>(23)?,
                        row.get::<_, i64>(24)?,
                        row.get::<_, i64>(25)?,
                        row.get::<_, i64>(26)?,
                        row.get::<_, i64>(27)?,
                        row.get::<_, i64>(28)?,
                        row.get::<_, i64>(29)?,
                    ))
                },
            )
            .optional()?;
        row.map(|row| {
            let classification = OpenAttemptClassification::parse(&row.7)?;
            if !classification.is_success() {
                return Err(invalid_stored("open_batch_observations", "classification"));
            }
            Ok(OpenBatchObservation {
                attempt_id: parse_stored_trace_id("open_batch_observations", "attempt_id", &row.0)?,
                observation_id: parse_stored_trace_id(
                    "open_batch_observations",
                    "observation_id",
                    &row.1,
                )?,
                target: TermCampusKey::try_new(&row.2, &row.3)?,
                observation_sequence: stored_u64(row.4)?,
                catalog_content_version: stored_u64(row.5)?,
                observed_at: row.6,
                classification,
                http: OpenHttpAuditMetadata {
                    http_status: Some(stored_u16(row.8)?),
                    cache_status: row.9.as_deref().map(OpenCacheStatus::parse).transpose()?,
                    decoded_bytes: Some(stored_u64(row.10)?),
                    decoded_body_sha256: Some(row.11),
                    content_type: Some(row.12),
                    etag: row.13,
                    cache_control: row.14,
                    date: row.15,
                    age_seconds: row.16.map(stored_u64).transpose()?,
                    last_modified: row.17,
                    retry_after: row.18,
                    retry_after_seconds: row.19.map(stored_u64).transpose()?,
                },
                canonical_set_sha256: row.20,
                state_sha256: row.21,
                source_value_count: stored_u64(row.22)?,
                catalog_section_count: stored_u64(row.23)?,
                intersection_count: stored_u64(row.24)?,
                orphan_count: stored_u64(row.25)?,
                duplicate_count: stored_u64(row.26)?,
                changed_section_count: stored_u64(row.27)?,
                body_changed: stored_bool("open_batch_observations", "body_changed", row.28)?,
                state_changed: stored_bool("open_batch_observations", "state_changed", row.29)?,
            })
        })
        .transpose()
    }

    pub fn open_section_current(
        &self,
        target: &TermCampusKey,
    ) -> StorageResult<Vec<OpenSectionCurrent>> {
        let mut statement = self.connection.prepare(
            "SELECT section_index, state, catalog_content_version, attempt_id,
                    observation_sequence, observed_at
             FROM open_section_current WHERE target_id = ?1 ORDER BY section_index",
        )?;
        let rows = statement.query_map([open_target_id(target)], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        rows.map(|row| {
            let row = row?;
            Ok(OpenSectionCurrent {
                section: SectionKey::try_new(
                    target.term().as_str(),
                    target.campus().as_str(),
                    &row.0,
                )?,
                state: OpenSectionState::parse("open_section_current", &row.1)?,
                catalog_content_version: stored_u64(row.2)?,
                attempt_id: parse_stored_trace_id("open_section_current", "attempt_id", &row.3)?,
                observation_sequence: stored_u64(row.4)?,
                observed_at: row.5,
            })
        })
        .collect()
    }

    /// Monotonic event-log seam for fanout; this does not perform WebSocket delivery.
    pub fn read_open_section_events(
        &self,
        after_event_id: u64,
        limit: u32,
    ) -> StorageResult<Vec<OpenSectionEvent>> {
        if limit == 0 || limit > 1_000 {
            return Err(StorageError::InvalidCommand {
                field: "limit",
                reason: "must be between 1 and 1000",
            });
        }
        let mut statement = self.connection.prepare(
            "SELECT e.event_id, e.observation_id, e.refresh_observation_id, e.attempt_id,
                    t.term_id, t.campus_code, e.section_index, e.state,
                    e.catalog_content_version, e.observation_sequence, e.observed_at
             FROM open_section_events e
             JOIN catalog_targets t ON t.target_id = e.target_id
             WHERE e.event_id > ?1 ORDER BY e.event_id LIMIT ?2",
        )?;
        let rows = statement.query_map(
            params![u64_to_i64(after_event_id)?, i64::from(limit)],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, String>(10)?,
                ))
            },
        )?;
        rows.map(|row| {
            let row = row?;
            Ok(OpenSectionEvent {
                event_id: stored_u64(row.0)?,
                observation_id: parse_stored_trace_id(
                    "open_section_events",
                    "observation_id",
                    &row.1,
                )?,
                refresh_observation_id: parse_stored_trace_id(
                    "open_section_events",
                    "refresh_observation_id",
                    &row.2,
                )?,
                attempt_id: parse_stored_trace_id("open_section_events", "attempt_id", &row.3)?,
                section: SectionKey::try_new(&row.4, &row.5, &row.6)?,
                state: OpenSectionState::parse("open_section_events", &row.7)?,
                catalog_content_version: stored_u64(row.8)?,
                observation_sequence: stored_u64(row.9)?,
                observed_at: row.10,
            })
        })
        .collect()
    }

    pub fn open_day_counters(
        &self,
        target: &TermCampusKey,
        rutgers_day: &str,
    ) -> StorageResult<OpenAttemptCounters> {
        validate_rutgers_day(rutgers_day)?;
        load_open_counters(
            &self.connection,
            "SELECT attempted, succeeded, failed, empty
             FROM open_daily_counters WHERE target_id = ?1 AND rutgers_day = ?2",
            &open_target_id(target),
            rutgers_day,
        )
    }

    pub fn open_service_day_counters(
        &self,
        rutgers_day: &str,
    ) -> StorageResult<OpenAttemptCounters> {
        validate_rutgers_day(rutgers_day)?;
        load_open_service_day_counters(&self.connection, rutgers_day)
    }

    pub fn open_run_counters(
        &self,
        target: &TermCampusKey,
        run_id: &TraceId,
    ) -> StorageResult<OpenAttemptCounters> {
        load_open_counters(
            &self.connection,
            "SELECT attempted, succeeded, failed, empty
             FROM open_run_counters WHERE target_id = ?1 AND run_id = ?2",
            &open_target_id(target),
            &run_id.to_string(),
        )
    }

    /// Retains every detail from the current Rutgers day plus at least the most recent 256 attempts.
    /// Daily and run counters are separate durable aggregates and are not pruned.
    pub fn prune_open_diagnostics(
        &mut self,
        target: &TermCampusKey,
        current_rutgers_day: &str,
        retain_recent: u64,
    ) -> StorageResult<OpenRetentionReport> {
        validate_rutgers_day(current_rutgers_day)?;
        if retain_recent < OPEN_DIAGNOSTIC_RETENTION_PER_TARGET {
            return Err(StorageError::InvalidCommand {
                field: "retain_recent",
                reason: "must retain at least 256 attempts per target",
            });
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let report = prune_open_diagnostics_transaction(
            &transaction,
            &open_target_id(target),
            current_rutgers_day,
            retain_recent,
        )?;
        transaction.commit()?;
        Ok(report)
    }

    pub fn put_open_origin_state(&mut self, state: &OpenOriginState) -> StorageResult<()> {
        validate_safe_identifier("origin_id", &state.origin_id)?;
        validate_optional_safe_token("reason_code", state.reason_code.as_deref())?;
        validate_timestamp("updated_at", &state.updated_at)?;
        if let Some(opened_at) = state.opened_at.as_deref() {
            validate_timestamp("opened_at", opened_at)?;
        }
        if let Some(retry_at) = state.retry_at.as_deref() {
            validate_timestamp("retry_at", retry_at)?;
        }
        validate_open_origin_state(state)?;
        self.connection.execute(
            "INSERT INTO open_origin_state(
                origin_id, circuit_state, reason_code, opened_at, retry_at,
                diagnostic_recheck_required, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(origin_id) DO UPDATE SET
                circuit_state = excluded.circuit_state,
                reason_code = excluded.reason_code,
                opened_at = excluded.opened_at,
                retry_at = excluded.retry_at,
                diagnostic_recheck_required = excluded.diagnostic_recheck_required,
                updated_at = excluded.updated_at",
            params![
                state.origin_id,
                state.circuit_state.as_str(),
                state.reason_code,
                state.opened_at,
                state.retry_at,
                i64::from(state.diagnostic_recheck_required),
                state.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn open_origin_state(&self, origin_id: &str) -> StorageResult<Option<OpenOriginState>> {
        validate_safe_identifier("origin_id", origin_id)?;
        self.connection
            .query_row(
                "SELECT origin_id, circuit_state, reason_code, opened_at, retry_at,
                        diagnostic_recheck_required, updated_at
                 FROM open_origin_state WHERE origin_id = ?1",
                [origin_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()?
            .map(|row| {
                Ok(OpenOriginState {
                    origin_id: row.0,
                    circuit_state: OpenCircuitState::parse(&row.1)?,
                    reason_code: row.2,
                    opened_at: row.3,
                    retry_at: row.4,
                    diagnostic_recheck_required: stored_bool(
                        "open_origin_state",
                        "diagnostic_recheck_required",
                        row.5,
                    )?,
                    updated_at: row.6,
                })
            })
            .transpose()
    }

    pub fn put_open_schedule_state(&mut self, state: &OpenScheduleState) -> StorageResult<()> {
        validate_timestamp("next_due_at", &state.next_due_at)?;
        validate_timestamp("updated_at", &state.updated_at)?;
        if let Some(value) = state.last_scheduled_at.as_deref() {
            validate_timestamp("last_scheduled_at", value)?;
        }
        if let Some(value) = state.last_actual_start_at.as_deref() {
            validate_timestamp("last_actual_start_at", value)?;
        }
        let target_id = open_target_id(&state.target);
        require_catalog_target(&self.connection, &target_id)?;
        self.connection.execute(
            "INSERT INTO open_schedule_state(
                target_id, requested_interval_seconds, effective_interval_seconds,
                next_due_at, last_scheduled_at, last_actual_start_at, schedule_lag_ms,
                failure_streak, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(target_id) DO UPDATE SET
                requested_interval_seconds = excluded.requested_interval_seconds,
                effective_interval_seconds = excluded.effective_interval_seconds,
                next_due_at = excluded.next_due_at,
                last_scheduled_at = excluded.last_scheduled_at,
                last_actual_start_at = excluded.last_actual_start_at,
                schedule_lag_ms = excluded.schedule_lag_ms,
                failure_streak = excluded.failure_streak,
                updated_at = excluded.updated_at",
            params![
                target_id,
                positive_u64_to_i64(
                    "requested_interval_seconds",
                    state.requested_interval_seconds,
                )?,
                positive_u64_to_i64(
                    "effective_interval_seconds",
                    state.effective_interval_seconds,
                )?,
                state.next_due_at,
                state.last_scheduled_at,
                state.last_actual_start_at,
                u64_to_i64(state.schedule_lag_ms)?,
                u64_to_i64(state.failure_streak)?,
                state.updated_at,
            ],
        )?;
        Ok(())
    }

    pub fn open_schedule_state(
        &self,
        target: &TermCampusKey,
    ) -> StorageResult<Option<OpenScheduleState>> {
        self.connection
            .query_row(
                "SELECT requested_interval_seconds, effective_interval_seconds,
                        next_due_at, last_scheduled_at, last_actual_start_at,
                        schedule_lag_ms, failure_streak, updated_at
                 FROM open_schedule_state WHERE target_id = ?1",
                [open_target_id(target)],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()?
            .map(|row| {
                Ok(OpenScheduleState {
                    target: target.clone(),
                    requested_interval_seconds: stored_u64(row.0)?,
                    effective_interval_seconds: stored_u64(row.1)?,
                    next_due_at: row.2,
                    last_scheduled_at: row.3,
                    last_actual_start_at: row.4,
                    schedule_lag_ms: stored_u64(row.5)?,
                    failure_streak: stored_u64(row.6)?,
                    updated_at: row.7,
                })
            })
            .transpose()
    }
}

fn load_started_open_attempt(
    transaction: &Transaction<'_>,
    attempt_id: TraceId,
) -> StorageResult<StartedOpenAttempt> {
    let row = transaction
        .query_row(
            "SELECT t.term_id, t.campus_code, a.run_id, a.attempt_sequence,
                    a.rutgers_day, a.captured_catalog_content_version,
                    a.started_at, a.effective_interval_seconds, a.classification,
                    a.candidate_catalog_observation_id,
                    a.candidate_base_content_version
             FROM open_pull_attempts a
             JOIN catalog_targets t ON t.target_id = a.target_id
             WHERE a.attempt_id = ?1",
            [attempt_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<i64>>(10)?,
                ))
            },
        )
        .optional()?
        .ok_or(StorageError::OpenAttemptNotFound(attempt_id))?;
    if row.8 != OpenAttemptClassification::Started.as_str() {
        return Err(StorageError::OpenAttemptNotStarted(attempt_id));
    }
    Ok(StartedOpenAttempt {
        target: TermCampusKey::try_new(&row.0, &row.1)?,
        run_id: parse_stored_trace_id("open_pull_attempts", "run_id", &row.2)?,
        attempt_sequence: stored_u64(row.3)?,
        rutgers_day: row.4,
        captured_catalog_content_version: stored_u64(row.5)?,
        started_at: row.6,
        effective_interval_seconds: row.7.map(stored_u64).transpose()?,
        candidate_catalog_observation_id: parse_optional_trace(
            "open_pull_attempts",
            "candidate_catalog_observation_id",
            row.9,
        )?,
        candidate_base_content_version: row.10.map(stored_u64).transpose()?,
    })
}

fn ensure_started_open_attempt_is_current(
    transaction: &Transaction<'_>,
    attempt: &StartedOpenAttempt,
    attempt_id: TraceId,
) -> StorageResult<()> {
    let target_id = open_target_id(&attempt.target);
    let (last_attempt_sequence, last_attempt_id) = transaction.query_row(
        "SELECT last_attempt_sequence, last_attempt_id
         FROM open_batch_state WHERE target_id = ?1",
        [&target_id],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
    )?;
    let last_attempt_id =
        parse_optional_trace("open_batch_state", "last_attempt_id", last_attempt_id)?;
    if stored_u64(last_attempt_sequence)? != attempt.attempt_sequence
        || last_attempt_id != Some(attempt_id)
    {
        return Err(StorageError::OpenAttemptSuperseded(attempt_id));
    }
    Ok(())
}

fn canonicalize_open_response(
    target: &TermCampusKey,
    open_sections: &[SectionKey],
    source_value_count: u64,
) -> StorageResult<CanonicalOpenResponse> {
    let mut canonical_indices = BTreeSet::new();
    for section in open_sections {
        if section.target() != *target {
            return Err(StorageError::InvalidCommand {
                field: "open_sections",
                reason: "all sections must belong to the attempt target",
            });
        }
        canonical_indices.insert(section.index().as_str().to_owned());
    }
    let canonical_count = u64::try_from(canonical_indices.len())
        .map_err(|_| StorageError::StoredIntegerOutOfRange)?;
    let duplicate_count =
        source_value_count
            .checked_sub(canonical_count)
            .ok_or(StorageError::InvalidCommand {
                field: "source_value_count",
                reason: "must be at least the canonical unique Open identity count",
            })?;
    let canonical_set_sha256 = open_set_sha256_v1(&canonical_indices);
    Ok(CanonicalOpenResponse {
        canonical_indices,
        source_value_count,
        duplicate_count,
        canonical_set_sha256,
    })
}

fn classify_open_response(
    canonical_indices: &BTreeSet<String>,
    catalog_indices: &BTreeSet<String>,
    intersection_count: u64,
    gate_hold: bool,
) -> OpenAttemptClassification {
    // Hard rejections always win and never consult the gate; the gate only
    // withholds snapshots that would otherwise be applicable (integrity gate
    // design, section 3).
    if canonical_indices.is_empty() {
        if catalog_indices.is_empty() {
            OpenAttemptClassification::ValidEmptyNoRows
        } else if gate_hold {
            OpenAttemptClassification::SuspectPartialSnapshot
        } else {
            OpenAttemptClassification::ValidApplied
        }
    } else if !catalog_indices.is_empty() && intersection_count == 0 {
        OpenAttemptClassification::UnsafeZeroIntersection
    } else if gate_hold {
        OpenAttemptClassification::SuspectPartialSnapshot
    } else {
        OpenAttemptClassification::ValidApplied
    }
}

const fn publish_outcome_content_version(outcome: &PublishOutcome) -> u64 {
    match outcome {
        PublishOutcome::AppliedChanged {
            content_version, ..
        }
        | PublishOutcome::AppliedUnchanged {
            content_version, ..
        }
        | PublishOutcome::InitialValidEmpty { content_version }
        | PublishOutcome::SuspectEmptyRetained {
            content_version, ..
        } => *content_version,
    }
}

fn canonicalize_watched_sections(
    target: &TermCampusKey,
    watched_sections: &[SectionKey],
    catalog_indices: &BTreeSet<String>,
) -> StorageResult<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    for section in watched_sections {
        if section.target() != *target {
            return Err(StorageError::InvalidCommand {
                field: "watched_sections",
                reason: "all watched sections must belong to the attempt target",
            });
        }
        let index = section.index().as_str();
        if !catalog_indices.contains(index) {
            return Err(StorageError::InvalidCommand {
                field: "watched_sections",
                reason: "all watched sections must exist in the captured Catalog version",
            });
        }
        result.insert(index.to_owned());
    }
    Ok(result)
}

fn open_set_sha256_v1(indices: &BTreeSet<String>) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"bcsp-open-set-v1\n");
    for index in indices {
        hasher.update(index.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

fn open_state_sha256_v1(
    target: &TermCampusKey,
    states: &BTreeMap<String, OpenSectionState>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"bcsp-open-state-v1\n");
    hasher.update(target.term().as_str().as_bytes());
    hasher.update(b"\n");
    hasher.update(target.campus().as_str().as_bytes());
    hasher.update(b"\n");
    for (index, state) in states {
        hasher.update(index.as_bytes());
        hasher.update(b"\t");
        hasher.update(state.as_str().as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

fn load_current_catalog_version(
    transaction: &Transaction<'_>,
    target_id: &str,
) -> StorageResult<u64> {
    transaction
        .query_row(
            "SELECT current_content_version FROM catalog_targets WHERE target_id = ?1",
            [target_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .map(stored_u64)
        .transpose()?
        .filter(|version| *version > 0)
        .ok_or(StorageError::CatalogTargetNotPublished)
}

fn load_attempt_catalog_indices(
    transaction: &Transaction<'_>,
    attempt_id: TraceId,
) -> StorageResult<BTreeSet<String>> {
    let mut statement = transaction.prepare(
        "SELECT section_index FROM open_attempt_catalog_sections
         WHERE attempt_id = ?1 ORDER BY section_index",
    )?;
    let rows = statement.query_map([attempt_id.to_string()], |row| row.get::<_, String>(0))?;
    rows.collect::<Result<BTreeSet<_>, _>>().map_err(Into::into)
}

fn load_current_state_map(
    transaction: &Transaction<'_>,
    target_id: &str,
) -> StorageResult<BTreeMap<String, OpenSectionState>> {
    let mut statement = transaction.prepare(
        "SELECT section_index, state FROM open_section_current
         WHERE target_id = ?1 ORDER BY section_index",
    )?;
    let rows = statement.query_map([target_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.map(|row| {
        let (index, state) = row?;
        Ok((
            index,
            OpenSectionState::parse("open_section_current", &state)?,
        ))
    })
    .collect()
}

fn changed_state_count(
    previous: &BTreeMap<String, OpenSectionState>,
    intended: &BTreeMap<String, OpenSectionState>,
) -> u64 {
    let changed_existing = intended
        .iter()
        .filter(|(index, state)| previous.get(*index) != Some(*state))
        .count();
    let removed = previous
        .keys()
        .filter(|index| !intended.contains_key(*index))
        .count();
    (changed_existing + removed) as u64
}

fn load_lkg_attempt_id(
    transaction: &Transaction<'_>,
    target_id: &str,
) -> StorageResult<Option<TraceId>> {
    let value = transaction
        .query_row(
            "SELECT lkg_attempt_id FROM open_batch_state WHERE target_id = ?1",
            [target_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();
    parse_optional_trace("open_batch_state", "lkg_attempt_id", value)
}

fn load_lkg_hashes(
    transaction: &Transaction<'_>,
    target_id: &str,
) -> StorageResult<(Option<String>, Option<String>)> {
    transaction
        .query_row(
            "SELECT lkg_canonical_set_sha256, lkg_state_sha256
             FROM open_batch_state WHERE target_id = ?1",
            [target_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| invalid_stored("open_batch_state", "target_id"))
}

#[allow(clippy::too_many_arguments)]
fn finalize_non_applied_attempt(
    transaction: &Transaction<'_>,
    attempt: &StartedOpenAttempt,
    attempt_id: TraceId,
    completed_at: &str,
    classification: OpenAttemptClassification,
    http: &OpenHttpAuditMetadata,
    response: Option<&CanonicalOpenResponse>,
    catalog_section_count: u64,
    intersection_count: u64,
    orphan_count: u64,
    error_code: &str,
    state_sha256: Option<&str>,
    gate_catalog_set_identity: Option<&str>,
) -> StorageResult<()> {
    validate_safe_code("error_code", error_code)?;
    validate_open_http_audit(http, false)?;
    let source_value_count = response.map_or(0, |value| value.source_value_count);
    let duplicate_count = response.map_or(0, |value| value.duplicate_count);
    let canonical_set_sha256 = response.map(|value| value.canonical_set_sha256.as_str());
    let updated = transaction.execute(
        "UPDATE open_pull_attempts
         SET completed_at = ?2, classification = ?3, http_status = ?4, cache_status = ?5,
             decoded_bytes = ?6, decoded_body_sha256 = ?7, content_type = ?8,
             etag = ?9, cache_control = ?10, response_date = ?11, age_seconds = ?12,
             last_modified = ?13, retry_after = ?14, retry_after_seconds = ?15,
             canonical_set_sha256 = ?16, state_sha256 = ?17,
             source_value_count = ?18, catalog_section_count = ?19,
             intersection_count = ?20, orphan_count = ?21, duplicate_count = ?22,
             error_code = ?23, gate_catalog_set_identity = ?24, diagnostic_token = NULL
         WHERE attempt_id = ?1 AND classification = 'STARTED'",
        params![
            attempt_id.to_string(),
            completed_at,
            classification.as_str(),
            http.http_status.map(i64::from),
            http.cache_status.map(OpenCacheStatus::as_str),
            optional_u64_to_i64(http.decoded_bytes)?,
            http.decoded_body_sha256.as_deref(),
            http.content_type.as_deref(),
            http.etag.as_deref(),
            http.cache_control.as_deref(),
            http.date.as_deref(),
            optional_u64_to_i64(http.age_seconds)?,
            http.last_modified.as_deref(),
            http.retry_after.as_deref(),
            optional_u64_to_i64(http.retry_after_seconds)?,
            canonical_set_sha256,
            state_sha256,
            u64_to_i64(source_value_count)?,
            u64_to_i64(catalog_section_count)?,
            u64_to_i64(intersection_count)?,
            u64_to_i64(orphan_count)?,
            u64_to_i64(duplicate_count)?,
            error_code,
            gate_catalog_set_identity,
        ],
    )?;
    if updated != 1 {
        return Err(StorageError::OpenAttemptNotStarted(attempt_id));
    }
    let batch_updated = transaction.execute(
        "UPDATE open_batch_state
         SET last_failure_attempt_sequence = ?2,
             last_failure_at = ?3,
             last_failure_error_code = ?4,
             last_failure_http_status = ?5
         WHERE target_id = ?1
           AND last_attempt_sequence = ?2
           AND last_attempt_id = ?6",
        params![
            open_target_id(&attempt.target),
            u64_to_i64(attempt.attempt_sequence)?,
            completed_at,
            error_code,
            http.http_status.map(i64::from),
            attempt_id.to_string(),
        ],
    )?;
    if batch_updated != 1 {
        return Err(StorageError::OpenAttemptSuperseded(attempt_id));
    }
    increment_final_counters(
        transaction,
        &open_target_id(&attempt.target),
        &attempt.run_id.to_string(),
        &attempt.rutgers_day,
        false,
        response.is_some_and(|value| value.canonical_indices.is_empty()),
    )
}

fn increment_attempted_counters(
    transaction: &Transaction<'_>,
    target_id: &str,
    run_id: &str,
    rutgers_day: &str,
) -> StorageResult<()> {
    transaction.execute(
        "INSERT INTO open_daily_counters(target_id, rutgers_day, attempted)
         VALUES (?1, ?2, 1)
         ON CONFLICT(target_id, rutgers_day) DO UPDATE SET attempted = attempted + 1",
        params![target_id, rutgers_day],
    )?;
    transaction.execute(
        "INSERT INTO open_run_counters(run_id, target_id, attempted)
         VALUES (?1, ?2, 1)
         ON CONFLICT(run_id, target_id) DO UPDATE SET attempted = attempted + 1",
        params![run_id, target_id],
    )?;
    Ok(())
}

fn increment_final_counters(
    transaction: &Transaction<'_>,
    target_id: &str,
    run_id: &str,
    rutgers_day: &str,
    succeeded: bool,
    empty: bool,
) -> StorageResult<()> {
    let failed = i64::from(!succeeded);
    let succeeded = i64::from(succeeded);
    let empty = i64::from(empty);
    let daily_updated = transaction.execute(
        "UPDATE open_daily_counters
         SET succeeded = succeeded + ?3, failed = failed + ?4, empty = empty + ?5
         WHERE target_id = ?1 AND rutgers_day = ?2",
        params![target_id, rutgers_day, succeeded, failed, empty],
    )?;
    let run_updated = transaction.execute(
        "UPDATE open_run_counters
         SET succeeded = succeeded + ?3, failed = failed + ?4, empty = empty + ?5
         WHERE run_id = ?1 AND target_id = ?2",
        params![run_id, target_id, succeeded, failed, empty],
    )?;
    if daily_updated != 1 || run_updated != 1 {
        return Err(invalid_stored("open_pull_attempts", "counters"));
    }
    Ok(())
}

fn load_open_counters(
    connection: &Connection,
    sql: &str,
    first: &str,
    second: &str,
) -> StorageResult<OpenAttemptCounters> {
    connection
        .query_row(sql, params![first, second], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .optional()?
        .map(|row| {
            Ok(OpenAttemptCounters {
                attempted: stored_u64(row.0)?,
                succeeded: stored_u64(row.1)?,
                failed: stored_u64(row.2)?,
                empty: stored_u64(row.3)?,
            })
        })
        .transpose()
        .map(Option::unwrap_or_default)
}

fn load_open_service_day_counters(
    connection: &Connection,
    rutgers_day: &str,
) -> StorageResult<OpenAttemptCounters> {
    let row = connection.query_row(
        "SELECT COALESCE(sum(attempted), 0), COALESCE(sum(succeeded), 0),
                COALESCE(sum(failed), 0), COALESCE(sum(empty), 0)
         FROM open_daily_counters WHERE rutgers_day = ?1",
        [rutgers_day],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        },
    )?;
    Ok(OpenAttemptCounters {
        attempted: stored_u64(row.0)?,
        succeeded: stored_u64(row.1)?,
        failed: stored_u64(row.2)?,
        empty: stored_u64(row.3)?,
    })
}

fn load_committed_open_counters(
    transaction: &Transaction<'_>,
    target_id: &str,
    run_id: &str,
    rutgers_day: &str,
) -> StorageResult<(
    OpenAttemptCounters,
    OpenAttemptCounters,
    OpenAttemptCounters,
)> {
    let run_counts = load_open_counters(
        transaction,
        "SELECT attempted, succeeded, failed, empty
         FROM open_run_counters WHERE target_id = ?1 AND run_id = ?2",
        target_id,
        run_id,
    )?;
    let target_day_counts = load_open_counters(
        transaction,
        "SELECT attempted, succeeded, failed, empty
         FROM open_daily_counters WHERE target_id = ?1 AND rutgers_day = ?2",
        target_id,
        rutgers_day,
    )?;
    let service_day_counts = load_open_service_day_counters(transaction, rutgers_day)?;
    Ok((run_counts, target_day_counts, service_day_counts))
}

fn prune_open_diagnostics_transaction(
    transaction: &Transaction<'_>,
    target_id: &str,
    current_rutgers_day: &str,
    retain_recent: u64,
) -> StorageResult<OpenRetentionReport> {
    let retain_recent = positive_u64_to_i64("retain_recent", retain_recent)?;
    let candidate_filter = "a.target_id = ?1
         AND a.classification != 'STARTED'
         AND a.rutgers_day != ?2
         AND a.attempt_id != COALESCE(
             (SELECT lkg_attempt_id FROM open_batch_state WHERE target_id = ?1), ''
         )
         AND a.attempt_id NOT IN (
             SELECT recent.attempt_id FROM open_pull_attempts recent
             WHERE recent.target_id = ?1
             ORDER BY recent.attempt_sequence DESC LIMIT ?3
         )";
    let batch_observations_removed = transaction.query_row(
        &format!(
            "SELECT count(*) FROM open_batch_observations b
             JOIN open_pull_attempts a ON a.attempt_id = b.attempt_id
             WHERE {candidate_filter}"
        ),
        params![target_id, current_rutgers_day, retain_recent],
        |row| row.get::<_, i64>(0),
    )?;
    let section_events_removed = transaction.query_row(
        &format!(
            "SELECT count(*) FROM open_section_events e
             JOIN open_pull_attempts a ON a.attempt_id = e.attempt_id
             WHERE {candidate_filter}"
        ),
        params![target_id, current_rutgers_day, retain_recent],
        |row| row.get::<_, i64>(0),
    )?;
    let attempts_removed = transaction.execute(
        &format!("DELETE FROM open_pull_attempts AS a WHERE {candidate_filter}"),
        params![target_id, current_rutgers_day, retain_recent],
    )?;
    Ok(OpenRetentionReport {
        attempts_removed: u64::try_from(attempts_removed)
            .map_err(|_| StorageError::StoredIntegerOutOfRange)?,
        batch_observations_removed: stored_u64(batch_observations_removed)?,
        section_events_removed: stored_u64(section_events_removed)?,
    })
}

pub(crate) fn recover_interrupted_open_attempts(connection: &mut Connection) -> StorageResult<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let pending = {
        let mut statement = transaction.prepare(
            "SELECT attempt_id, target_id, run_id, rutgers_day, attempt_sequence
             FROM open_pull_attempts WHERE classification = 'STARTED'
             ORDER BY target_id, attempt_sequence",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    if pending.is_empty() {
        transaction.commit()?;
        return Ok(());
    }
    let completed_at =
        OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .map_err(|_| StorageError::InvalidCommand {
                field: "completed_at",
                reason: "could not format recovery timestamp",
            })?;
    for (attempt_id, target_id, run_id, rutgers_day, attempt_sequence) in pending {
        transaction.execute(
            "UPDATE open_pull_attempts
             SET completed_at = ?2, classification = 'INTERRUPTED',
                 error_code = 'PROCESS_RESTARTED'
             WHERE attempt_id = ?1 AND classification = 'STARTED'",
            params![attempt_id, completed_at],
        )?;
        transaction.execute(
            "UPDATE open_batch_state
             SET last_failure_attempt_sequence = ?2,
                 last_failure_at = ?3,
                 last_failure_error_code = 'PROCESS_RESTARTED',
                 last_failure_http_status = NULL
             WHERE target_id = ?1",
            params![target_id, attempt_sequence, completed_at],
        )?;
        increment_final_counters(
            &transaction,
            &target_id,
            &run_id,
            &rutgers_day,
            false,
            false,
        )?;
    }
    transaction.commit()?;
    Ok(())
}

fn require_catalog_target(connection: &Connection, target_id: &str) -> StorageResult<()> {
    let exists = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM catalog_targets WHERE target_id = ?1)",
        [target_id],
        |row| row.get::<_, bool>(0),
    )?;
    if exists {
        Ok(())
    } else {
        Err(StorageError::CatalogTargetNotPublished)
    }
}

fn validate_timestamp(field: &'static str, value: &str) -> StorageResult<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|_| StorageError::InvalidCommand {
        field,
        reason: "must be an RFC 3339 timestamp",
    })
}

fn validate_timestamp_order(started_at: &str, completed_at: &str) -> StorageResult<()> {
    let started = validate_timestamp("started_at", started_at)?;
    let completed = validate_timestamp("completed_at", completed_at)?;
    if completed < started {
        return Err(StorageError::InvalidCommand {
            field: "completed_at",
            reason: "must not precede started_at",
        });
    }
    Ok(())
}

fn elapsed_milliseconds(earlier: &str, later: &str) -> StorageResult<u64> {
    let earlier = validate_timestamp("lkg_observed_at", earlier)?;
    let later = validate_timestamp("started_at", later)?;
    let elapsed = (later - earlier).whole_milliseconds().max(0);
    u64::try_from(elapsed).map_err(|_| StorageError::StoredIntegerOutOfRange)
}

fn validate_rutgers_day(value: &str) -> StorageResult<()> {
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
    {
        return Err(StorageError::InvalidCommand {
            field: "rutgers_day",
            reason: "must be a valid YYYY-MM-DD America/New_York day",
        });
    }
    let year = value[0..4]
        .parse::<i32>()
        .map_err(|_| invalid_rutgers_day())?;
    let month = value[5..7]
        .parse::<u8>()
        .ok()
        .and_then(|month| Month::try_from(month).ok())
        .ok_or_else(invalid_rutgers_day)?;
    let day = value[8..10]
        .parse::<u8>()
        .map_err(|_| invalid_rutgers_day())?;
    Date::from_calendar_date(year, month, day).map_err(|_| invalid_rutgers_day())?;
    Ok(())
}

fn invalid_rutgers_day() -> StorageError {
    StorageError::InvalidCommand {
        field: "rutgers_day",
        reason: "must be a valid YYYY-MM-DD America/New_York day",
    }
}

fn validate_safe_code(field: &'static str, value: &str) -> StorageResult<()> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(StorageError::InvalidCommand {
            field,
            reason: "must be a non-empty upper snake-case code of at most 64 bytes",
        });
    }
    Ok(())
}

fn validate_optional_safe_token(field: &'static str, value: Option<&str>) -> StorageResult<()> {
    if let Some(value) = value
        && (value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b':' | b'-')
            }))
    {
        return Err(StorageError::InvalidCommand {
            field,
            reason: "must be a safe token of at most 64 bytes",
        });
    }
    Ok(())
}

fn validate_open_http_audit(
    metadata: &OpenHttpAuditMetadata,
    success_required: bool,
) -> StorageResult<()> {
    if metadata
        .http_status
        .is_some_and(|status| !(100..=599).contains(&status))
    {
        return Err(StorageError::InvalidCommand {
            field: "http_status",
            reason: "must be a valid HTTP status",
        });
    }
    for (field, value) in [
        ("decoded_bytes", metadata.decoded_bytes),
        ("age_seconds", metadata.age_seconds),
        ("retry_after_seconds", metadata.retry_after_seconds),
    ] {
        if value.is_some_and(|value| value > i64::MAX as u64) {
            return Err(StorageError::InvalidCommand {
                field,
                reason: "must fit the durable signed 64-bit range",
            });
        }
    }
    if let Some(value) = metadata.decoded_body_sha256.as_deref() {
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(StorageError::InvalidCommand {
                field: "decoded_body_sha256",
                reason: "must be exactly 64 lowercase hexadecimal characters",
            });
        }
        if metadata.decoded_bytes.is_none() {
            return Err(StorageError::InvalidCommand {
                field: "decoded_body_sha256",
                reason: "requires decoded_bytes",
            });
        }
    }
    for (field, value) in [
        ("content_type", metadata.content_type.as_deref()),
        ("etag", metadata.etag.as_deref()),
        ("cache_control", metadata.cache_control.as_deref()),
        ("date", metadata.date.as_deref()),
        ("last_modified", metadata.last_modified.as_deref()),
        ("retry_after", metadata.retry_after.as_deref()),
    ] {
        validate_optional_http_header(field, value)?;
    }
    if success_required {
        let status = required_success_http_status(metadata)?;
        if !(200..=299).contains(&status) {
            return Err(StorageError::InvalidCommand {
                field: "http_status",
                reason: "a successful Open response must use a 2xx status",
            });
        }
        required_success_decoded_bytes(metadata)?;
        required_success_header(
            "decoded_body_sha256",
            metadata.decoded_body_sha256.as_deref(),
        )?;
        required_success_header("content_type", metadata.content_type.as_deref())?;
    }
    Ok(())
}

fn validate_optional_http_header(field: &'static str, value: Option<&str>) -> StorageResult<()> {
    if let Some(value) = value
        && (value.is_empty()
            || value.len() > 4_096
            || value.bytes().any(|byte| matches!(byte, b'\r' | b'\n')))
    {
        return Err(StorageError::InvalidCommand {
            field,
            reason: "must be non-empty CR/LF-free HTTP audit text of at most 4096 bytes",
        });
    }
    Ok(())
}

fn required_success_http_status(metadata: &OpenHttpAuditMetadata) -> StorageResult<u16> {
    metadata.http_status.ok_or(StorageError::InvalidCommand {
        field: "http_status",
        reason: "is required for a successful Open response",
    })
}

fn required_success_decoded_bytes(metadata: &OpenHttpAuditMetadata) -> StorageResult<u64> {
    metadata.decoded_bytes.ok_or(StorageError::InvalidCommand {
        field: "decoded_bytes",
        reason: "is required for a successful Open response",
    })
}

fn required_success_header<'a>(
    field: &'static str,
    value: Option<&'a str>,
) -> StorageResult<&'a str> {
    value.ok_or(StorageError::InvalidCommand {
        field,
        reason: "is required for a successful Open response",
    })
}

fn validate_safe_identifier(field: &'static str, value: &str) -> StorageResult<()> {
    validate_optional_safe_token(field, Some(value))
}

fn validate_open_origin_state(state: &OpenOriginState) -> StorageResult<()> {
    let valid = match state.circuit_state {
        OpenCircuitState::Closed => {
            state.reason_code.is_none()
                && state.opened_at.is_none()
                && state.retry_at.is_none()
                && !state.diagnostic_recheck_required
        }
        OpenCircuitState::RetryAfter => {
            state.reason_code.is_some()
                && state.opened_at.is_some()
                && state.retry_at.is_some()
                && !state.diagnostic_recheck_required
        }
        OpenCircuitState::FatalDiagnostic => {
            state.reason_code.is_some()
                && state.opened_at.is_some()
                && state.retry_at.is_none()
                && state.diagnostic_recheck_required
        }
    };
    if !valid {
        return Err(StorageError::InvalidCommand {
            field: "circuit_state",
            reason: "origin circuit metadata is inconsistent with its state",
        });
    }
    Ok(())
}

fn validate_optional_positive(field: &'static str, value: Option<u64>) -> StorageResult<()> {
    if value == Some(0) {
        return Err(StorageError::InvalidCommand {
            field,
            reason: "must be positive when present",
        });
    }
    Ok(())
}

fn positive_u64_to_i64(field: &'static str, value: u64) -> StorageResult<i64> {
    if value == 0 {
        return Err(StorageError::InvalidCommand {
            field,
            reason: "must be positive",
        });
    }
    u64_to_i64(value)
}

fn optional_u64_to_i64(value: Option<u64>) -> StorageResult<Option<i64>> {
    value.map(u64_to_i64).transpose()
}

fn u64_to_i64(value: u64) -> StorageResult<i64> {
    i64::try_from(value).map_err(|_| StorageError::StoredIntegerOutOfRange)
}

fn stored_u64(value: i64) -> StorageResult<u64> {
    u64::try_from(value).map_err(|_| StorageError::StoredIntegerOutOfRange)
}

fn stored_u16(value: i64) -> StorageResult<u16> {
    u16::try_from(value).map_err(|_| StorageError::StoredIntegerOutOfRange)
}

fn stored_bool(table: &'static str, field: &'static str, value: i64) -> StorageResult<bool> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(invalid_stored(table, field)),
    }
}

fn open_target_id(target: &TermCampusKey) -> String {
    format!("{}/{}", target.term(), target.campus())
}

/// Reconstructs the stable refresh-observation identity written for a valid
/// Open commit. Read-side projections use this exact seam instead of minting a
/// new identity when a watch starts after the commit.
pub fn derive_open_refresh_observation_id(attempt_id: TraceId) -> StorageResult<TraceId> {
    derived_observation_id("bcsp-open-refresh-observation-v1", attempt_id, None)
}

/// Reconstructs the stable per-Section observation identity written for a
/// valid Open commit. The attempt identity already binds the target; the
/// Section index is the same discriminator used by the atomic commit path.
pub fn derive_open_section_observation_id(
    attempt_id: TraceId,
    section: &SectionKey,
) -> StorageResult<TraceId> {
    derived_observation_id(
        "bcsp-open-section-observation-v1",
        attempt_id,
        Some(section.index().as_str()),
    )
}

fn derived_observation_id(
    domain: &str,
    attempt_id: TraceId,
    discriminator: Option<&str>,
) -> StorageResult<TraceId> {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    hasher.update(b"\n");
    hasher.update(attempt_id.to_string().as_bytes());
    hasher.update(b"\n");
    if let Some(discriminator) = discriminator {
        hasher.update(discriminator.as_bytes());
        hasher.update(b"\n");
    }
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    )
    .parse()
    .map_err(|_| invalid_stored("open_section_events", "observation_id"))
}

fn parse_optional_trace(
    table: &'static str,
    column: &'static str,
    value: Option<String>,
) -> StorageResult<Option<TraceId>> {
    value
        .as_deref()
        .map(|value| parse_stored_trace_id(table, column, value))
        .transpose()
}

fn invalid_stored(table: &'static str, field: &'static str) -> StorageError {
    StorageError::InvalidStoredProjection {
        table,
        field,
        reason: "contains an unsupported Open projection",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use bcsp_contracts::{CourseGroupKey, CourseVariantKey};
    use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
    use serde_json::json;

    use crate::{
        CatalogRefreshCommand, CatalogSnapshot, StoredCourseGroup, StoredCourseVariant,
        StoredSection, catalog_content_sha256_v1,
    };

    const STARTED: &str = "2026-07-14T00:00:00Z";
    const COMPLETED: &str = "2026-07-14T00:00:01Z";
    const HASH_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const HASH_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn target(term: &str, campus: &str) -> TermCampusKey {
        TermCampusKey::try_new(term, campus).expect("synthetic target")
    }

    fn section(target: &TermCampusKey, index: &str) -> SectionKey {
        SectionKey::try_new(target.term().as_str(), target.campus().as_str(), index)
            .expect("synthetic section")
    }

    fn trace(label: &str) -> TraceId {
        let digest = Sha256::digest(label.as_bytes());
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        format!(
            "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            bytes[0],
            bytes[1],
            bytes[2],
            bytes[3],
            bytes[4],
            bytes[5],
            bytes[6],
            bytes[7],
            bytes[8],
            bytes[9],
            bytes[10],
            bytes[11],
            bytes[12],
            bytes[13],
            bytes[14],
            bytes[15],
        )
        .parse()
        .expect("deterministic UUIDv4")
    }

    fn catalog_snapshot(
        target: &TermCampusKey,
        indices: &[&str],
        canonical_sha256: &str,
    ) -> CatalogSnapshot {
        let group = CourseGroupKey::try_new(
            target.term().as_str(),
            target.campus().as_str(),
            "SYN:OPEN:001",
        )
        .expect("group");
        let variant =
            CourseVariantKey::try_new(group.clone(), &format!("v1:{HASH_A}")).expect("variant");
        CatalogSnapshot {
            course_groups: vec![StoredCourseGroup {
                key: group,
                canonical_facts: json!({"fixture": "open-storage"}),
            }],
            course_variants: vec![StoredCourseVariant {
                key: variant.clone(),
                subject_code: Some("SYN".to_owned()),
                course_number: Some("001".to_owned()),
                title: Some("Synthetic Open Storage".to_owned()),
                description: None,
                credits_summary: None,
                supplement: None,
                search_document: "SYN:OPEN:001 synthetic open storage".to_owned(),
                canonical_sha256: canonical_sha256.to_owned(),
                raw_multiplicity: 1,
                canonical_facts: json!({"fixture": "open-storage"}),
            }],
            sections: indices
                .iter()
                .map(|index| StoredSection {
                    key: section(target, index),
                    variant_key: variant.clone(),
                    section_number: None,
                    catalog_status: None,
                    section_course_type: None,
                    delivery_modality: "UNKNOWN".to_owned(),
                    synchronicity: "UNKNOWN".to_owned(),
                    canonical_facts: json!({"index": index}),
                    canonical_sha256: HASH_C.to_owned(),
                })
                .collect(),
            occurrences: Vec::new(),
            provenance: Vec::new(),
        }
    }

    fn publish_catalog(
        storage: &mut OperationalStorage,
        target: &TermCampusKey,
        label: &str,
        indices: &[&str],
        canonical_sha256: &str,
    ) -> u64 {
        let snapshot = catalog_snapshot(target, indices, canonical_sha256);
        let body = format!("synthetic-{label}");
        let source_content_sha256 = format!("{:x}", Sha256::digest(body.as_bytes()));
        let semantic_content_sha256 =
            catalog_content_sha256_v1(target, &snapshot).expect("catalog hash");
        let outcome = storage
            .apply_catalog_refresh(
                CatalogRefreshCommand {
                    observation_id: trace(&format!("catalog-{label}")),
                    target: target.clone(),
                    started_at: STARTED.to_owned(),
                    completed_at: COMPLETED.to_owned(),
                    source_content_sha256,
                    semantic_content_sha256,
                    source_bytes: body.len() as u64,
                    raw_payload: None,
                    snapshot,
                },
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            )
            .expect("publish catalog");
        match outcome {
            PublishOutcome::AppliedChanged {
                content_version, ..
            }
            | PublishOutcome::AppliedUnchanged {
                content_version, ..
            } => content_version,
            other => panic!("unexpected catalog outcome: {other:?}"),
        }
    }

    fn begin(
        storage: &mut OperationalStorage,
        target: &TermCampusKey,
        attempt: &str,
        run: &str,
        version: u64,
        day: &str,
    ) {
        storage
            .begin_open_pull_attempt(&BeginOpenPullAttemptCommand {
                attempt_id: trace(attempt),
                run_id: trace(run),
                target: target.clone(),
                captured_catalog_content_version: version,
                rutgers_day: day.to_owned(),
                started_at: STARTED.to_owned(),
                lane: OpenRequestLane::General,
                requested_interval_seconds: Some(30),
                effective_interval_seconds: Some(45),
                schedule_lag_ms: Some(12),
            })
            .expect("begin Open attempt");
    }

    fn success_http(decoded_bytes: u64) -> OpenHttpAuditMetadata {
        OpenHttpAuditMetadata {
            http_status: Some(200),
            cache_status: Some(OpenCacheStatus::Miss),
            decoded_bytes: Some(decoded_bytes),
            decoded_body_sha256: Some(HASH_A.to_owned()),
            content_type: Some("application/json; charset=utf-8".to_owned()),
            etag: Some("\"synthetic-etag\"".to_owned()),
            cache_control: Some("max-age=30".to_owned()),
            date: Some("Tue, 14 Jul 2026 00:00:01 GMT".to_owned()),
            age_seconds: Some(7),
            last_modified: Some("Mon, 13 Jul 2026 23:59:00 GMT".to_owned()),
            retry_after: None,
            retry_after_seconds: None,
        }
    }

    #[test]
    fn open_mirror_rebuild_prepares_the_insert_once_and_reuses_it_across_commits() {
        let scope = target("FALL_2026", "OPEN_PREPARE_COUNT");
        let mut storage = OperationalStorage::open_in_memory().expect("storage");
        let version = publish_catalog(
            &mut storage,
            &scope,
            "prepare-count",
            &["00001", "00002", "00003"],
            HASH_A,
        );

        let insert_prepares = Arc::new(AtomicUsize::new(0));
        let hook_counter = Arc::clone(&insert_prepares);
        {
            let connection = &storage.connection;
            connection.flush_prepared_statement_cache();
            connection
                .authorizer(Some(move |context: AuthContext<'_>| {
                    if matches!(
                        context.action,
                        AuthAction::Insert { table_name } if table_name == "open_section_current"
                    ) {
                        hook_counter.fetch_add(1, Ordering::SeqCst);
                    }
                    Authorization::Allow
                }))
                .expect("install authorizer");
        }

        begin(
            &mut storage,
            &scope,
            "prepare-count-1",
            "prepare-count-run",
            version,
            "2026-07-14",
        );
        let first = storage
            .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
                attempt_id: trace("prepare-count-1"),
                completed_at: COMPLETED.to_owned(),
                open_sections: vec![section(&scope, "00001")],
                source_value_count: 1,
                watched_sections: Vec::new(),
                http: success_http(9),
            })
            .expect("first applied commit");
        assert_eq!(first.classification, OpenAttemptClassification::ValidApplied);
        assert_eq!(
            insert_prepares.load(Ordering::SeqCst),
            1,
            "the 3-row mirror rebuild must prepare its INSERT exactly once"
        );

        begin(
            &mut storage,
            &scope,
            "prepare-count-2",
            "prepare-count-run",
            version,
            "2026-07-14",
        );
        let second = storage
            .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
                attempt_id: trace("prepare-count-2"),
                completed_at: COMPLETED.to_owned(),
                open_sections: vec![section(&scope, "00002")],
                source_value_count: 1,
                watched_sections: Vec::new(),
                http: success_http(9),
            })
            .expect("second applied commit");
        assert_eq!(
            second.classification,
            OpenAttemptClassification::ValidApplied
        );
        assert_eq!(
            insert_prepares.load(Ordering::SeqCst),
            1,
            "the second commit must reuse the connection-cached statement"
        );

        let current = storage.open_section_current(&scope).expect("current");
        assert_eq!(current.len(), 3);
        assert!(
            current
                .iter()
                .all(|row| row.attempt_id == trace("prepare-count-2"))
        );
        storage
            .connection
            .authorizer(None::<fn(AuthContext<'_>) -> Authorization>)
            .expect("clear authorizer");
    }

    #[test]
    fn open_hashes_match_the_locked_client_and_reconciler_goldens() {
        let indexes = ["00001".to_owned(), "00002".to_owned()]
            .into_iter()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            open_set_sha256_v1(&indexes),
            "817ceb456303f598c8631b1d31dcfd471c8facd6bef7b93c4d4f44433784f369"
        );

        let target = TermCampusKey::try_new("92026", "NB").expect("locked target");
        let states = indexes
            .into_iter()
            .map(|index| (index, OpenSectionState::Open))
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            open_state_sha256_v1(&target, &states),
            "09a3c51f4f65611abf20f0179c001a6cd782c057524ba591899dfdd3b4331d3b"
        );
    }
}
