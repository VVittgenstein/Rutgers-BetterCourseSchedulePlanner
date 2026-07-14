use std::future::Future;
use std::time::Duration;

use bcsp_contracts::{
    CatalogContentVersion, CatalogContentVersionError, SectionKey, TermCampusKey, TraceId,
};
use bcsp_operational_storage::{
    BeginOpenPullAttemptCommand, FinishOpenPullFailureCommand, FinishOpenPullSuccessCommand,
    OpenAttemptClassification, OpenCacheStatus, OpenCatalogSnapshot, OpenCommitOutcome,
    OpenHttpAuditMetadata, OpenRequestLane, OperationalStorage, StorageError, StorageResult,
};
use bcsp_rutgers_client::{
    OpenResponseMetadata, OpenSectionsError, OpenSectionsFailure, OpenSectionsRequest,
    OpenSectionsResponse, RedirectScope, RetryAfterHeader, RetryAfterValue,
    RutgersOpenSectionsClient, canonical_open_set_sha256,
};
use jiff::{Timestamp, tz::TimeZone};
use thiserror::Error;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::{
    CatalogOpenBatch, CompletionSchedule, GeneralOpenInterval, OpenFailureKind,
    OpenReconcileClassification, OpenReconcilePlan, OpenSetEvidence, OriginSchedulerLane,
    ReconcileInputError, reconcile_open_set, retry_directive,
};

const RUTGERS_TIME_ZONE: &str = "America/New_York";

/// Persistence seam used by the Open pull coordinator. The production
/// implementation is [`OperationalStorage`]; tests can use a small in-memory
/// fake without weakening the SQLite integration tests in that crate.
pub trait OpenPullPersistence {
    fn serving_open_catalog_snapshot(
        &mut self,
        target: &TermCampusKey,
    ) -> StorageResult<Option<OpenCatalogSnapshot>>;

    fn begin_open_pull_attempt(
        &mut self,
        command: &BeginOpenPullAttemptCommand,
    ) -> StorageResult<u64>;

    fn finish_open_pull_success(
        &mut self,
        command: FinishOpenPullSuccessCommand,
    ) -> StorageResult<OpenCommitOutcome>;

    fn finish_open_pull_failure(
        &mut self,
        command: &FinishOpenPullFailureCommand,
    ) -> StorageResult<OpenCommitOutcome>;
}

impl OpenPullPersistence for OperationalStorage {
    fn serving_open_catalog_snapshot(
        &mut self,
        target: &TermCampusKey,
    ) -> StorageResult<Option<OpenCatalogSnapshot>> {
        OperationalStorage::serving_open_catalog_snapshot(self, target)
    }

    fn begin_open_pull_attempt(
        &mut self,
        command: &BeginOpenPullAttemptCommand,
    ) -> StorageResult<u64> {
        OperationalStorage::begin_open_pull_attempt(self, command)
    }

    fn finish_open_pull_success(
        &mut self,
        command: FinishOpenPullSuccessCommand,
    ) -> StorageResult<OpenCommitOutcome> {
        OperationalStorage::finish_open_pull_success(self, command)
    }

    fn finish_open_pull_failure(
        &mut self,
        command: &FinishOpenPullFailureCommand,
    ) -> StorageResult<OpenCommitOutcome> {
        OperationalStorage::finish_open_pull_failure(self, command)
    }
}

pub trait OpenPullClock {
    fn now(&mut self) -> OffsetDateTime;
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SystemOpenPullClock;

impl OpenPullClock for SystemOpenPullClock {
    fn now(&mut self) -> OffsetDateTime {
        OffsetDateTime::now_utc()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenPullCommand {
    pub attempt_id: TraceId,
    pub run_id: TraceId,
    pub source_request: OpenSectionsRequest,
    pub general_interval: GeneralOpenInterval,
    pub active_watch_count: u64,
    pub lane: OriginSchedulerLane,
    pub scheduler_lag: Duration,
    pub current_failure_streak: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenPullTerminal {
    Valid(OpenSectionsResponse),
    Unsafe(OpenSectionsResponse),
    CatalogRace(OpenSectionsResponse),
    Failed(OpenPullFailure),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OpenPullFailure {
    Upstream(OpenSectionsFailure),
    ResponseTargetMismatch(OpenResponseMetadata),
    ResponseMetadataMismatch(OpenResponseMetadata),
    CatalogUnavailableAfterFetch(OpenResponseMetadata),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenPullExecution {
    pub outcome: OpenCommitOutcome,
    pub scheduler_completion: CompletionSchedule,
    pub terminal: OpenPullTerminal,
    pub reconcile: Option<OpenReconcilePlan>,
}

#[derive(Debug, Error)]
pub enum SharedOpenServiceError {
    #[error("Catalog target has no published content: {target:?}")]
    TargetNotPublished { target: TermCampusKey },
    #[error("Open persistence failed")]
    Storage(#[from] StorageError),
    #[error("Open timestamp could not be represented as RFC 3339")]
    TimestampFormat,
    #[error("Open Rutgers-day conversion failed")]
    RutgersDay,
    #[error("Open response count exceeded the supported range")]
    ResponseCountOverflow,
    #[error("stored Catalog version was not a published nonzero version")]
    InvalidCatalogVersion(#[from] CatalogContentVersionError),
    #[error("Open reconcile input was inconsistent")]
    Reconcile(#[from] ReconcileInputError),
    #[error("Open persistence disagreed with the shared reconcile plan")]
    ReconcileDivergence,
    #[error("Open persistence returned an impossible terminal classification")]
    UnexpectedClassification,
    #[error("a Catalog scheduler lane cannot execute an Open pull")]
    CatalogSchedulerLane,
}

pub struct SharedOpenService<'storage, P = OperationalStorage> {
    persistence: &'storage mut P,
}

impl<'storage, P> SharedOpenService<'storage, P>
where
    P: OpenPullPersistence,
{
    pub const fn new(persistence: &'storage mut P) -> Self {
        Self { persistence }
    }

    /// Executes exactly one already-coalesced target pull. The attempt start is
    /// durably recorded before `fetch` is invoked, and every returned upstream
    /// result is finalized before this method returns.
    pub async fn execute_with<C, F, Fut, W>(
        &mut self,
        command: OpenPullCommand,
        clock: &mut C,
        fetch: F,
        current_watched_sections: W,
    ) -> Result<OpenPullExecution, SharedOpenServiceError>
    where
        C: OpenPullClock,
        F: FnOnce(OpenSectionsRequest) -> Fut,
        Fut: Future<Output = Result<OpenSectionsResponse, OpenSectionsFailure>>,
        W: FnOnce(&TermCampusKey) -> Vec<SectionKey>,
    {
        let target = command.source_request.target().target();
        let captured_catalog = self
            .persistence
            .serving_open_catalog_snapshot(&target)?
            .ok_or_else(|| SharedOpenServiceError::TargetNotPublished {
                target: target.clone(),
            })?;
        let captured_catalog_content_version = captured_catalog.content_version;
        let started_at = clock.now();
        let started_at_text = format_timestamp(started_at)?;
        let rutgers_day = rutgers_day_at(started_at)?;
        let effective_interval = command
            .general_interval
            .effective(command.active_watch_count > 0);
        let lane = storage_lane(command.lane)?;
        self.persistence
            .begin_open_pull_attempt(&BeginOpenPullAttemptCommand {
                attempt_id: command.attempt_id,
                run_id: command.run_id,
                target: target.clone(),
                captured_catalog_content_version,
                rutgers_day,
                started_at: started_at_text,
                lane,
                requested_interval_seconds: Some(u64::from(command.general_interval.seconds())),
                effective_interval_seconds: Some(effective_interval.as_secs()),
                schedule_lag_ms: Some(duration_millis(command.scheduler_lag)),
            })?;

        let expected_batch = command.source_request.target().clone();
        let fetched = fetch(command.source_request.clone()).await;
        let completed_at = clock.now();
        let completed_at_text = format_timestamp(completed_at)?;

        match fetched {
            Ok(response)
                if response.target == expected_batch
                    && response_metadata_is_consistent(&response) =>
            {
                self.finish_success(
                    command,
                    captured_catalog_content_version,
                    response,
                    completed_at,
                    completed_at_text,
                    current_watched_sections,
                )
            }
            Ok(response) if response.target != expected_batch => {
                let metadata = response.metadata;
                self.finish_failure(
                    command,
                    target,
                    OpenPullFailure::ResponseTargetMismatch(metadata),
                    completed_at,
                    completed_at_text,
                    effective_interval,
                )
            }
            Ok(response) => {
                let metadata = response.metadata;
                self.finish_failure(
                    command,
                    target,
                    OpenPullFailure::ResponseMetadataMismatch(metadata),
                    completed_at,
                    completed_at_text,
                    effective_interval,
                )
            }
            Err(error) => self.finish_failure(
                command,
                target,
                OpenPullFailure::Upstream(error),
                completed_at,
                completed_at_text,
                effective_interval,
            ),
        }
    }

    fn finish_success<W>(
        &mut self,
        command: OpenPullCommand,
        captured_catalog_content_version: u64,
        response: OpenSectionsResponse,
        completed_at: OffsetDateTime,
        completed_at_text: String,
        current_watched_sections: W,
    ) -> Result<OpenPullExecution, SharedOpenServiceError>
    where
        W: FnOnce(&TermCampusKey) -> Vec<SectionKey>,
    {
        let target = command.source_request.target().target();
        let effective_interval = command
            .general_interval
            .effective(command.active_watch_count > 0);
        let source_value_count = u64::try_from(response.metadata.raw_value_count)
            .map_err(|_| SharedOpenServiceError::ResponseCountOverflow)?;
        let decoded_bytes = u64::try_from(response.metadata.decoded_bytes)
            .map_err(|_| SharedOpenServiceError::ResponseCountOverflow)?;
        let Some(current_catalog) = self.persistence.serving_open_catalog_snapshot(&target)? else {
            return self.finish_failure(
                command,
                target,
                OpenPullFailure::CatalogUnavailableAfterFetch(response.metadata.clone()),
                completed_at,
                completed_at_text,
                effective_interval,
            );
        };
        let catalog = CatalogOpenBatch::try_new(
            current_catalog.target,
            CatalogContentVersion::try_from(current_catalog.content_version)?,
            current_catalog.sections,
        )?;
        let open_set = OpenSetEvidence::try_new(
            target.clone(),
            response.open_indexes.iter().cloned(),
            source_value_count,
        )?;
        let reconcile = reconcile_open_set(
            CatalogContentVersion::try_from(captured_catalog_content_version)?,
            &catalog,
            &open_set,
        )?;
        let open_sections = response
            .open_indexes
            .iter()
            .cloned()
            .map(|index| SectionKey::new(target.term().clone(), target.campus().clone(), index))
            .collect();
        // Watches are intentionally sampled after the network await and immediately
        // before the atomic commit. Dispatch-time watch state must never leak across
        // the request boundary: a newly added watch receives this valid observation,
        // while a removed watch does not receive a stale event.
        let watched_sections = current_watched_sections(&target);
        let outcome = self
            .persistence
            .finish_open_pull_success(FinishOpenPullSuccessCommand {
                attempt_id: command.attempt_id,
                completed_at: completed_at_text,
                open_sections,
                source_value_count,
                watched_sections,
                http: OpenHttpAuditMetadata {
                    http_status: Some(response.metadata.http_status),
                    cache_status: Some(OpenCacheStatus::NotApplicable),
                    decoded_bytes: Some(decoded_bytes),
                    decoded_body_sha256: Some(response.metadata.decoded_body_sha256.clone()),
                    content_type: Some(response.metadata.content_type.clone()),
                    etag: response.metadata.etag.clone(),
                    cache_control: response.metadata.cache_control.clone(),
                    date: response.metadata.date.clone(),
                    age_seconds: response.metadata.age_seconds,
                    last_modified: response.metadata.last_modified.clone(),
                    retry_after: None,
                    retry_after_seconds: None,
                },
            })?;
        if !outcome_matches_reconcile(&outcome, &reconcile) {
            return Err(SharedOpenServiceError::ReconcileDivergence);
        }
        let (scheduler_completion, terminal) = match outcome.classification {
            OpenAttemptClassification::ValidApplied
            | OpenAttemptClassification::ValidEmptyNoRows => (
                CompletionSchedule::Success,
                OpenPullTerminal::Valid(response),
            ),
            OpenAttemptClassification::UnsafeEmpty => (
                CompletionSchedule::Retry(retry_directive(
                    &target,
                    effective_interval,
                    command.current_failure_streak,
                    OpenFailureKind::UnsafeEmpty,
                )),
                OpenPullTerminal::Unsafe(response),
            ),
            OpenAttemptClassification::UnsafeZeroIntersection => (
                CompletionSchedule::Retry(retry_directive(
                    &target,
                    effective_interval,
                    command.current_failure_streak,
                    OpenFailureKind::UnsafeZeroIntersection,
                )),
                OpenPullTerminal::Unsafe(response),
            ),
            OpenAttemptClassification::StaleCatalogRace => (
                CompletionSchedule::CatalogRace,
                OpenPullTerminal::CatalogRace(response),
            ),
            OpenAttemptClassification::Started
            | OpenAttemptClassification::Failed
            | OpenAttemptClassification::Interrupted => {
                return Err(SharedOpenServiceError::UnexpectedClassification);
            }
        };
        Ok(OpenPullExecution {
            outcome,
            scheduler_completion,
            terminal,
            reconcile: Some(reconcile),
        })
    }

    fn finish_failure(
        &mut self,
        command: OpenPullCommand,
        target: TermCampusKey,
        failure: OpenPullFailure,
        completed_at: OffsetDateTime,
        completed_at_text: String,
        effective_interval: Duration,
    ) -> Result<OpenPullExecution, SharedOpenServiceError> {
        let (http_status, retry_after_seconds, error_code, failure_kind) =
            failure_storage_fields(&failure, completed_at);
        let http = failure_http_audit(&failure, http_status, retry_after_seconds)?;
        let outcome = self
            .persistence
            .finish_open_pull_failure(&FinishOpenPullFailureCommand {
                attempt_id: command.attempt_id,
                completed_at: completed_at_text,
                http,
                error_code: error_code.to_owned(),
                diagnostic_token: None,
            })?;
        if outcome.classification != OpenAttemptClassification::Failed {
            return Err(SharedOpenServiceError::UnexpectedClassification);
        }
        Ok(OpenPullExecution {
            outcome,
            scheduler_completion: CompletionSchedule::Retry(retry_directive(
                &target,
                effective_interval,
                command.current_failure_streak,
                failure_kind,
            )),
            terminal: OpenPullTerminal::Failed(failure),
            reconcile: None,
        })
    }
}

impl SharedOpenService<'_, OperationalStorage> {
    pub async fn execute_official<W>(
        &mut self,
        command: OpenPullCommand,
        client: &RutgersOpenSectionsClient,
        current_watched_sections: W,
    ) -> Result<OpenPullExecution, SharedOpenServiceError>
    where
        W: FnOnce(&TermCampusKey) -> Vec<SectionKey>,
    {
        let mut clock = SystemOpenPullClock;
        self.execute_with(
            command,
            &mut clock,
            |request| async move { client.fetch(&request).await },
            current_watched_sections,
        )
        .await
    }
}

fn storage_lane(lane: OriginSchedulerLane) -> Result<OpenRequestLane, SharedOpenServiceError> {
    match lane {
        OriginSchedulerLane::Catalog => Err(SharedOpenServiceError::CatalogSchedulerLane),
        OriginSchedulerLane::OpenGeneral => Ok(OpenRequestLane::General),
        OriginSchedulerLane::OpenActiveWatch => Ok(OpenRequestLane::ActiveWatch),
        OriginSchedulerLane::OpenFirstLoad => Ok(OpenRequestLane::FirstLoad),
        OriginSchedulerLane::OpenManualRefresh => Ok(OpenRequestLane::ManualRefresh),
        OriginSchedulerLane::OpenCatalogRaceRecheck => Ok(OpenRequestLane::CatalogRaceRecheck),
    }
}

fn failure_http_audit(
    failure: &OpenPullFailure,
    fallback_http_status: Option<u16>,
    retry_after_seconds: Option<u64>,
) -> Result<OpenHttpAuditMetadata, SharedOpenServiceError> {
    let mut audit = OpenHttpAuditMetadata {
        http_status: fallback_http_status,
        cache_status: Some(OpenCacheStatus::NotApplicable),
        decoded_bytes: None,
        decoded_body_sha256: None,
        content_type: None,
        etag: None,
        cache_control: None,
        date: None,
        age_seconds: None,
        last_modified: None,
        retry_after: None,
        retry_after_seconds,
    };
    match failure {
        OpenPullFailure::Upstream(value) => {
            let Some(metadata) = value.response_metadata() else {
                return Ok(audit);
            };
            audit.http_status = Some(metadata.http_status);
            audit.decoded_bytes = metadata
                .decoded_bytes
                .map(u64::try_from)
                .transpose()
                .map_err(|_| SharedOpenServiceError::ResponseCountOverflow)?;
            audit.decoded_body_sha256 = metadata.decoded_body_sha256.clone();
            audit.content_type = metadata.content_type.clone();
            audit.etag = metadata.etag.clone();
            audit.cache_control = metadata.cache_control.clone();
            audit.date = metadata.date.clone();
            audit.age_seconds = metadata.age_seconds;
            audit.last_modified = metadata.last_modified.clone();
            audit.retry_after = metadata.retry_after_raw.clone();
        }
        OpenPullFailure::ResponseTargetMismatch(metadata)
        | OpenPullFailure::ResponseMetadataMismatch(metadata)
        | OpenPullFailure::CatalogUnavailableAfterFetch(metadata) => {
            audit.http_status = Some(metadata.http_status);
            audit.decoded_bytes = Some(
                u64::try_from(metadata.decoded_bytes)
                    .map_err(|_| SharedOpenServiceError::ResponseCountOverflow)?,
            );
            audit.decoded_body_sha256 = Some(metadata.decoded_body_sha256.clone());
            audit.content_type = Some(metadata.content_type.clone());
            audit.etag = metadata.etag.clone();
            audit.cache_control = metadata.cache_control.clone();
            audit.date = metadata.date.clone();
            audit.age_seconds = metadata.age_seconds;
            audit.last_modified = metadata.last_modified.clone();
        }
    }
    Ok(audit)
}

fn failure_storage_fields(
    failure: &OpenPullFailure,
    completed_at: OffsetDateTime,
) -> (Option<u16>, Option<u64>, &'static str, OpenFailureKind) {
    let OpenPullFailure::Upstream(failure) = failure else {
        let (error_code, failure_kind) = match failure {
            OpenPullFailure::ResponseTargetMismatch(_) => (
                "OPEN_RESPONSE_TARGET_MISMATCH",
                OpenFailureKind::FatalProtocol,
            ),
            OpenPullFailure::ResponseMetadataMismatch(_) => (
                "OPEN_RESPONSE_METADATA_MISMATCH",
                OpenFailureKind::FatalProtocol,
            ),
            OpenPullFailure::CatalogUnavailableAfterFetch(_) => (
                "OPEN_CATALOG_UNAVAILABLE_AFTER_FETCH",
                OpenFailureKind::Transient,
            ),
            OpenPullFailure::Upstream(_) => unreachable!("matched by the let-else"),
        };
        return (None, None, error_code, failure_kind);
    };
    match failure.kind() {
        OpenSectionsError::Timeout => (None, None, "OPEN_TIMEOUT", OpenFailureKind::Transient),
        OpenSectionsError::Network => (None, None, "OPEN_NETWORK", OpenFailureKind::Transient),
        OpenSectionsError::RequestConstruction => (
            None,
            None,
            "OPEN_REQUEST_CONSTRUCTION",
            OpenFailureKind::FatalProtocol,
        ),
        OpenSectionsError::ResponseUrlMismatch => (
            None,
            None,
            "OPEN_RESPONSE_URL_MISMATCH",
            OpenFailureKind::FatalProtocol,
        ),
        OpenSectionsError::TransientHttp { status } => (
            Some(*status),
            None,
            "OPEN_TRANSIENT_HTTP",
            OpenFailureKind::Transient,
        ),
        OpenSectionsError::RateLimited { retry_after } => {
            let retry_after = retry_after_delay(*retry_after, completed_at);
            (
                Some(429),
                retry_after.map(|delay| delay.as_secs()),
                "OPEN_RATE_LIMITED",
                OpenFailureKind::RateLimited { retry_after },
            )
        }
        OpenSectionsError::Forbidden => (
            Some(403),
            None,
            "OPEN_FORBIDDEN",
            OpenFailureKind::FatalProtocol,
        ),
        OpenSectionsError::Redirect { status, scope } => {
            let error_code = match scope {
                RedirectScope::OffOrigin => "OPEN_OFF_ORIGIN_REDIRECT",
                RedirectScope::SameOrigin => "OPEN_SAME_ORIGIN_REDIRECT",
                RedirectScope::InvalidLocation => "OPEN_INVALID_REDIRECT",
            };
            (
                Some(*status),
                None,
                error_code,
                OpenFailureKind::FatalProtocol,
            )
        }
        OpenSectionsError::FatalClientHttp { status } => (
            Some(*status),
            None,
            "OPEN_FATAL_CLIENT_HTTP",
            OpenFailureKind::FatalProtocol,
        ),
        OpenSectionsError::UnsupportedHttp { status } => (
            Some(*status),
            None,
            "OPEN_UNSUPPORTED_HTTP",
            OpenFailureKind::FatalProtocol,
        ),
        OpenSectionsError::InvalidContentType => (
            None,
            None,
            "OPEN_INVALID_CONTENT_TYPE",
            OpenFailureKind::FatalProtocol,
        ),
        OpenSectionsError::ResponseTooLarge => (
            None,
            None,
            "OPEN_RESPONSE_TOO_LARGE",
            OpenFailureKind::FatalProtocol,
        ),
        OpenSectionsError::ContentDecoding => (
            None,
            None,
            "OPEN_CONTENT_DECODING",
            OpenFailureKind::FatalProtocol,
        ),
        OpenSectionsError::InvalidJson => (
            None,
            None,
            "OPEN_INVALID_JSON",
            OpenFailureKind::FatalProtocol,
        ),
        OpenSectionsError::RootNotArray => (
            None,
            None,
            "OPEN_ROOT_NOT_ARRAY",
            OpenFailureKind::FatalProtocol,
        ),
        OpenSectionsError::NonStringValue { .. } => (
            None,
            None,
            "OPEN_NON_STRING_VALUE",
            OpenFailureKind::FatalProtocol,
        ),
        OpenSectionsError::InvalidSectionIndex { .. } => (
            None,
            None,
            "OPEN_INVALID_SECTION_INDEX",
            OpenFailureKind::FatalProtocol,
        ),
    }
}

fn outcome_matches_reconcile(outcome: &OpenCommitOutcome, reconcile: &OpenReconcilePlan) -> bool {
    if outcome.classification == OpenAttemptClassification::StaleCatalogRace {
        // The write transaction may observe a newer Catalog publication than
        // the immediately preceding read-side reconcile. That second race is
        // the storage guard doing its job, not a semantic disagreement.
        return outcome.observation_sequence.is_none();
    }
    let expected_classification = match reconcile.classification {
        OpenReconcileClassification::ValidApplied => OpenAttemptClassification::ValidApplied,
        OpenReconcileClassification::ValidEmptyNoRows => {
            OpenAttemptClassification::ValidEmptyNoRows
        }
        OpenReconcileClassification::UnsafeEmpty => OpenAttemptClassification::UnsafeEmpty,
        OpenReconcileClassification::UnsafeZeroIntersection => {
            OpenAttemptClassification::UnsafeZeroIntersection
        }
        OpenReconcileClassification::StaleCatalogRace => {
            OpenAttemptClassification::StaleCatalogRace
        }
    };
    outcome.classification == expected_classification
        && outcome.catalog_content_version == reconcile.current_catalog_content_version.get()
        && outcome.source_value_count == reconcile.raw_value_count
        && outcome.catalog_section_count == reconcile.catalog_section_count
        && outcome.intersection_count == reconcile.open_count
        && outcome.orphan_count == reconcile.orphan_count
        && outcome.duplicate_count == reconcile.duplicate_count
}

fn response_metadata_is_consistent(response: &OpenSectionsResponse) -> bool {
    let unique_count = response.open_indexes.len();
    let expected_classification = if unique_count == 0 {
        bcsp_rutgers_client::OpenPayloadClassification::Empty
    } else {
        bcsp_rutgers_client::OpenPayloadClassification::Nonempty
    };
    response.classification == expected_classification
        && response.metadata.unique_value_count == unique_count
        && response.metadata.raw_value_count >= unique_count
        && response.metadata.duplicate_value_count
            == response
                .metadata
                .raw_value_count
                .saturating_sub(unique_count)
        && canonical_open_set_sha256(&response.open_indexes)
            == response.metadata.canonical_set_sha256
}

fn retry_after_delay(
    retry_after: RetryAfterHeader,
    completed_at: OffsetDateTime,
) -> Option<Duration> {
    match retry_after {
        RetryAfterHeader::Valid(RetryAfterValue::DelaySeconds(seconds)) => {
            Some(Duration::from_secs(seconds))
        }
        RetryAfterHeader::Valid(RetryAfterValue::HttpDateUnixSeconds(unix_seconds)) => {
            let remaining = unix_seconds.saturating_sub(completed_at.unix_timestamp());
            Some(Duration::from_secs(u64::try_from(remaining).unwrap_or(0)))
        }
        RetryAfterHeader::Missing | RetryAfterHeader::Invalid => None,
    }
}

fn format_timestamp(value: OffsetDateTime) -> Result<String, SharedOpenServiceError> {
    value
        .format(&Rfc3339)
        .map_err(|_| SharedOpenServiceError::TimestampFormat)
}

pub fn rutgers_day_at(value: OffsetDateTime) -> Result<String, SharedOpenServiceError> {
    let nanosecond =
        i32::try_from(value.nanosecond()).map_err(|_| SharedOpenServiceError::RutgersDay)?;
    let timestamp = Timestamp::new(value.unix_timestamp(), nanosecond)
        .map_err(|_| SharedOpenServiceError::RutgersDay)?;
    let time_zone =
        TimeZone::get(RUTGERS_TIME_ZONE).map_err(|_| SharedOpenServiceError::RutgersDay)?;
    Ok(timestamp.to_zoned(time_zone).date().to_string())
}

fn duration_millis(value: Duration) -> u64 {
    u64::try_from(value.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, VecDeque};
    use std::str::FromStr;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use bcsp_contracts::{CourseGroupKey, CourseVariantKey, OpenBatchKey, SectionIndex};
    use bcsp_operational_storage::{
        CatalogRefreshCommand, CatalogSnapshot, EmptySnapshotDecision, OpenSectionState,
        PublishOutcome, StoredCourseGroup, StoredCourseVariant, StoredSection,
        catalog_content_sha256_v1,
    };
    use bcsp_rutgers_client::{
        DiscoverySnapshot, OpenPayloadClassification, Presence, RawDiscoveryCampus,
        RawDiscoveryDocument, RawDiscoveryTarget, RawDiscoveryTerm, SourceProvenance,
    };
    use time::macros::datetime;

    use super::*;

    #[derive(Debug)]
    struct FakePersistence {
        versions: VecDeque<u64>,
        began: Arc<AtomicBool>,
        success_classification: OpenAttemptClassification,
        success: Option<FinishOpenPullSuccessCommand>,
        failure: Option<FinishOpenPullFailureCommand>,
    }

    impl FakePersistence {
        fn new(classification: OpenAttemptClassification) -> Self {
            Self {
                versions: VecDeque::from([7, 7]),
                began: Arc::new(AtomicBool::new(false)),
                success_classification: classification,
                success: None,
                failure: None,
            }
        }

        fn with_versions(
            classification: OpenAttemptClassification,
            versions: impl IntoIterator<Item = u64>,
        ) -> Self {
            let mut value = Self::new(classification);
            value.versions = versions.into_iter().collect();
            value
        }

        fn outcome(&self, classification: OpenAttemptClassification) -> OpenCommitOutcome {
            let unsafe_empty = classification == OpenAttemptClassification::UnsafeEmpty;
            OpenCommitOutcome {
                attempt_id: trace(1),
                attempt_sequence: 1,
                classification,
                refresh_observation_id: classification.is_success().then_some(trace(1)),
                observation_sequence: classification.is_success().then_some(1),
                catalog_content_version: 7,
                source_value_count: if unsafe_empty { 0 } else { 3 },
                catalog_section_count: 2,
                intersection_count: if unsafe_empty { 0 } else { 1 },
                orphan_count: if unsafe_empty { 0 } else { 1 },
                duplicate_count: if unsafe_empty { 0 } else { 1 },
                changed_section_count: if classification.is_success() { 2 } else { 0 },
                body_changed: classification.is_success(),
                state_changed: classification.is_success(),
                retained_lkg_attempt_id: classification.is_success().then_some(trace(1)),
            }
        }
    }

    impl OpenPullPersistence for FakePersistence {
        fn serving_open_catalog_snapshot(
            &mut self,
            _target: &TermCampusKey,
        ) -> StorageResult<Option<OpenCatalogSnapshot>> {
            Ok(self
                .versions
                .pop_front()
                .map(|content_version| OpenCatalogSnapshot {
                    target: batch().target(),
                    content_version,
                    sections: vec![
                        SectionKey::try_new("92026", "NB", "00001").expect("Section"),
                        SectionKey::try_new("92026", "NB", "00002").expect("Section"),
                    ],
                }))
        }

        fn begin_open_pull_attempt(
            &mut self,
            _command: &BeginOpenPullAttemptCommand,
        ) -> StorageResult<u64> {
            self.began.store(true, Ordering::SeqCst);
            Ok(1)
        }

        fn finish_open_pull_success(
            &mut self,
            command: FinishOpenPullSuccessCommand,
        ) -> StorageResult<OpenCommitOutcome> {
            self.success = Some(command);
            Ok(self.outcome(self.success_classification))
        }

        fn finish_open_pull_failure(
            &mut self,
            command: &FinishOpenPullFailureCommand,
        ) -> StorageResult<OpenCommitOutcome> {
            self.failure = Some(command.clone());
            Ok(self.outcome(OpenAttemptClassification::Failed))
        }
    }

    struct FixedClock(VecDeque<OffsetDateTime>);

    impl OpenPullClock for FixedClock {
        fn now(&mut self) -> OffsetDateTime {
            self.0.pop_front().expect("one timestamp per clock read")
        }
    }

    fn trace(suffix: u8) -> TraceId {
        TraceId::from_str(&format!("00000000-0000-4000-8000-{suffix:012x}"))
            .expect("synthetic trace")
    }

    fn batch() -> OpenBatchKey {
        OpenBatchKey::try_new("92026", "NB").expect("synthetic batch")
    }

    fn source_request() -> OpenSectionsRequest {
        let snapshot = DiscoverySnapshot::try_from_raw(
            RawDiscoveryDocument {
                source_version: Presence::Value("synthetic-v1".to_owned()),
                terms: Presence::Value(vec![RawDiscoveryTerm {
                    term_id: Presence::Value("92026".to_owned()),
                    year: Presence::Value(2026),
                    term_code: Presence::Value("9".to_owned()),
                    display: Presence::Value("Fall 2026".to_owned()),
                    published: Presence::Value(true),
                    extra: BTreeMap::new(),
                }]),
                campuses: Presence::Value(vec![RawDiscoveryCampus {
                    campus_code: Presence::Value("NB".to_owned()),
                    display: Presence::Value("New Brunswick".to_owned()),
                    category: Presence::Missing,
                    enabled: Presence::Value(true),
                    extra: BTreeMap::new(),
                }]),
                targets: Presence::Value(vec![RawDiscoveryTarget {
                    term_id: Presence::Value("92026".to_owned()),
                    campus_code: Presence::Value("NB".to_owned()),
                    enabled: Presence::Value(true),
                    extra: BTreeMap::new(),
                }]),
                subjects: Presence::Value(Vec::new()),
                extra: BTreeMap::new(),
            },
            SourceProvenance::from_body(
                "synthetic-discovery",
                "2026-07-14T00:00:00Z",
                b"synthetic-discovery",
            ),
        )
        .expect("synthetic discovery");
        OpenSectionsRequest::try_from_discovery(&snapshot.terms[0], &snapshot.targets[0])
            .expect("synthetic source request")
    }

    fn command(active_watch_count: u64) -> OpenPullCommand {
        OpenPullCommand {
            attempt_id: trace(1),
            run_id: trace(2),
            source_request: source_request(),
            general_interval: GeneralOpenInterval::local(30).expect("interval"),
            active_watch_count,
            lane: OriginSchedulerLane::OpenActiveWatch,
            scheduler_lag: Duration::from_millis(250),
            current_failure_streak: 0,
        }
    }

    fn response() -> OpenSectionsResponse {
        let open_indexes = vec![index("00001"), index("99999")];
        let canonical_set_sha256 = canonical_open_set_sha256(&open_indexes);
        OpenSectionsResponse {
            target: batch(),
            classification: OpenPayloadClassification::Nonempty,
            open_indexes,
            metadata: OpenResponseMetadata {
                http_status: 200,
                decoded_bytes: 32,
                decoded_body_sha256:
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                canonical_set_sha256,
                raw_value_count: 3,
                unique_value_count: 2,
                duplicate_value_count: 1,
                content_type: "application/json".to_owned(),
                etag: Some("W/\"synthetic\"".to_owned()),
                cache_control: Some("max-age=5".to_owned()),
                date: Some("Tue, 14 Jul 2026 04:00:00 GMT".to_owned()),
                age_seconds: Some(2),
                last_modified: Some("Tue, 14 Jul 2026 03:59:00 GMT".to_owned()),
            },
        }
    }

    fn empty_response() -> OpenSectionsResponse {
        let open_indexes = Vec::new();
        OpenSectionsResponse {
            target: batch(),
            classification: OpenPayloadClassification::Empty,
            metadata: OpenResponseMetadata {
                http_status: 200,
                decoded_bytes: 2,
                decoded_body_sha256:
                    "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned(),
                canonical_set_sha256: canonical_open_set_sha256(&open_indexes),
                raw_value_count: 0,
                unique_value_count: 0,
                duplicate_value_count: 0,
                content_type: "application/json".to_owned(),
                etag: None,
                cache_control: None,
                date: None,
                age_seconds: None,
                last_modified: None,
            },
            open_indexes,
        }
    }

    fn index(value: &str) -> SectionIndex {
        SectionIndex::try_from(value).expect("synthetic index")
    }

    fn publish_catalog(storage: &mut OperationalStorage) {
        let target = batch().target();
        let group = CourseGroupKey::try_new(
            target.term().as_str(),
            target.campus().as_str(),
            "SYN:OPEN:001",
        )
        .expect("group");
        let variant = CourseVariantKey::try_new(
            group.clone(),
            "v1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        )
        .expect("variant");
        let snapshot = CatalogSnapshot {
            course_groups: vec![StoredCourseGroup {
                key: group,
                canonical_facts: Default::default(),
            }],
            course_variants: vec![StoredCourseVariant {
                key: variant.clone(),
                subject_code: Some("SYN".to_owned()),
                course_number: Some("001".to_owned()),
                title: Some("Synthetic Open Service".to_owned()),
                description: None,
                credits_summary: None,
                supplement: None,
                search_document: "SYN:OPEN:001 synthetic open service".to_owned(),
                canonical_sha256:
                    "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned(),
                raw_multiplicity: 1,
                canonical_facts: Default::default(),
            }],
            sections: ["00001", "00002"]
                .into_iter()
                .map(|value| StoredSection {
                    key: SectionKey::try_new("92026", "NB", value).expect("Section"),
                    variant_key: variant.clone(),
                    section_number: None,
                    catalog_status: None,
                    section_course_type: None,
                    delivery_modality: "UNKNOWN".to_owned(),
                    synchronicity: "UNKNOWN".to_owned(),
                    canonical_facts: Default::default(),
                    canonical_sha256:
                        "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
                            .to_owned(),
                })
                .collect(),
            occurrences: Vec::new(),
            provenance: Vec::new(),
        };
        let semantic_content_sha256 =
            catalog_content_sha256_v1(&target, &snapshot).expect("Catalog hash");
        let outcome = storage
            .apply_catalog_refresh(
                CatalogRefreshCommand {
                    observation_id: trace(9),
                    target,
                    started_at: "2026-07-14T00:00:00Z".to_owned(),
                    completed_at: "2026-07-14T00:00:01Z".to_owned(),
                    source_content_sha256: bcsp_rutgers_client::sha256_hex(b"synthetic-catalog"),
                    semantic_content_sha256,
                    source_bytes: 17,
                    raw_payload: None,
                    snapshot,
                },
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
            )
            .expect("publish Catalog");
        assert!(matches!(outcome, PublishOutcome::AppliedChanged { .. }));
    }

    fn clock() -> FixedClock {
        FixedClock(VecDeque::from([
            datetime!(2026-07-14 03:59:59 UTC),
            datetime!(2026-07-14 04:00:00 UTC),
        ]))
    }

    #[tokio::test]
    async fn attempt_is_durable_before_one_shared_fetch_and_raw_counts_survive() {
        let mut persistence = FakePersistence::new(OpenAttemptClassification::ValidApplied);
        let began = Arc::clone(&persistence.began);
        let fetches = Arc::new(AtomicUsize::new(0));
        let fetches_for_call = Arc::clone(&fetches);
        let watched = (1..=9)
            .map(|value| {
                SectionKey::try_new("92026", "NB", &format!("{value:05}"))
                    .expect("synthetic watched Section")
            })
            .collect();
        let mut service = SharedOpenService::new(&mut persistence);
        let execution = service
            .execute_with(
                command(9),
                &mut clock(),
                move |_| async move {
                    assert!(began.load(Ordering::SeqCst));
                    fetches_for_call.fetch_add(1, Ordering::SeqCst);
                    Ok(response())
                },
                move |_| watched,
            )
            .await
            .expect("execute");

        assert_eq!(fetches.load(Ordering::SeqCst), 1);
        assert!(matches!(execution.terminal, OpenPullTerminal::Valid(_)));
        assert_eq!(execution.scheduler_completion, CompletionSchedule::Success);
        let stored = persistence.success.expect("successful finalization");
        assert_eq!(stored.source_value_count, 3);
        assert_eq!(stored.open_sections.len(), 2);
        assert_eq!(stored.watched_sections.len(), 9);
    }

    #[tokio::test]
    async fn watched_sections_are_sampled_after_fetch_not_at_dispatch() {
        let mut persistence = FakePersistence::new(OpenAttemptClassification::ValidApplied);
        let removed = SectionKey::try_new("92026", "NB", "00002").expect("removed watch");
        let added = SectionKey::try_new("92026", "NB", "00001").expect("added watch");
        let live_watches = Arc::new(Mutex::new(vec![removed.clone()]));
        let watches_during_fetch = Arc::clone(&live_watches);
        let watches_at_commit = Arc::clone(&live_watches);
        let added_during_fetch = added.clone();

        SharedOpenService::new(&mut persistence)
            .execute_with(
                command(1),
                &mut clock(),
                move |_| async move {
                    *watches_during_fetch.lock().expect("watch lock") = vec![added_during_fetch];
                    Ok(response())
                },
                move |_| watches_at_commit.lock().expect("watch lock").clone(),
            )
            .await
            .expect("execute");

        let stored = persistence.success.expect("successful finalization");
        assert_eq!(stored.watched_sections, vec![added]);
        assert!(!stored.watched_sections.contains(&removed));
    }

    #[tokio::test]
    async fn failure_is_recorded_and_translated_to_scheduler_backoff() {
        let mut persistence = FakePersistence::new(OpenAttemptClassification::ValidApplied);
        let mut service = SharedOpenService::new(&mut persistence);
        let execution = service
            .execute_with(
                command(0),
                &mut clock(),
                |_| async {
                    Err(OpenSectionsFailure::from(
                        OpenSectionsError::TransientHttp { status: 503 },
                    ))
                },
                |_| Vec::new(),
            )
            .await
            .expect("recorded failure");

        assert!(matches!(execution.terminal, OpenPullTerminal::Failed(_)));
        assert!(matches!(
            execution.scheduler_completion,
            CompletionSchedule::Retry(_)
        ));
        let stored = persistence.failure.expect("failure finalization");
        assert_eq!(stored.http.http_status, Some(503));
        assert_eq!(stored.error_code, "OPEN_TRANSIENT_HTTP");
    }

    #[tokio::test]
    async fn content_decoding_failure_requires_explicit_diagnostic_recheck() {
        let mut persistence = FakePersistence::new(OpenAttemptClassification::ValidApplied);
        let execution = SharedOpenService::new(&mut persistence)
            .execute_with(
                command(0),
                &mut clock(),
                |_| async {
                    Err(OpenSectionsFailure::from(
                        OpenSectionsError::ContentDecoding,
                    ))
                },
                |_| Vec::new(),
            )
            .await
            .expect("recorded content failure");

        let CompletionSchedule::Retry(directive) = execution.scheduler_completion else {
            panic!("content failure must produce a retry directive");
        };
        assert_eq!(directive.mode, crate::RetryMode::ExplicitDiagnosticRecheck);
        assert_eq!(
            persistence.failure.expect("failure").error_code,
            "OPEN_CONTENT_DECODING"
        );
    }

    #[tokio::test]
    async fn unsafe_result_retains_lkg_and_uses_retry_policy() {
        let mut persistence = FakePersistence::new(OpenAttemptClassification::UnsafeEmpty);
        let mut service = SharedOpenService::new(&mut persistence);
        let execution = service
            .execute_with(
                command(0),
                &mut clock(),
                |_| async { Ok(empty_response()) },
                |_| Vec::new(),
            )
            .await
            .expect("unsafe classification");
        assert!(matches!(execution.terminal, OpenPullTerminal::Unsafe(_)));
        assert!(matches!(
            execution.scheduler_completion,
            CompletionSchedule::Retry(_)
        ));
    }

    #[tokio::test]
    async fn catalog_version_race_is_classified_without_applying_stale_state() {
        let mut persistence =
            FakePersistence::with_versions(OpenAttemptClassification::StaleCatalogRace, [7, 8]);
        let mut service = SharedOpenService::new(&mut persistence);
        let execution = service
            .execute_with(
                command(0),
                &mut clock(),
                |_| async { Ok(response()) },
                |_| Vec::new(),
            )
            .await
            .expect("Catalog race");
        assert!(matches!(
            execution.terminal,
            OpenPullTerminal::CatalogRace(_)
        ));
        assert_eq!(
            execution.scheduler_completion,
            CompletionSchedule::CatalogRace
        );
        assert_eq!(
            execution
                .reconcile
                .expect("read-side reconcile")
                .classification,
            OpenReconcileClassification::StaleCatalogRace
        );
    }

    #[tokio::test]
    async fn inconsistent_source_metadata_is_failed_before_reconcile_commit() {
        let mut persistence = FakePersistence::new(OpenAttemptClassification::ValidApplied);
        let mut service = SharedOpenService::new(&mut persistence);
        let execution = service
            .execute_with(
                command(0),
                &mut clock(),
                |_| async {
                    let mut value = response();
                    value.metadata.duplicate_value_count = 0;
                    Ok(value)
                },
                |_| Vec::new(),
            )
            .await
            .expect("metadata failure is recorded");
        assert!(matches!(
            execution.terminal,
            OpenPullTerminal::Failed(OpenPullFailure::ResponseMetadataMismatch(_))
        ));
        let stored = persistence.failure.expect("failure");
        assert_eq!(stored.error_code, "OPEN_RESPONSE_METADATA_MISMATCH");
        assert_eq!(stored.http.http_status, Some(200));
        assert_eq!(stored.http.decoded_bytes, Some(32));
        assert_eq!(
            stored.http.decoded_body_sha256.as_deref(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(
            stored.http.content_type.as_deref(),
            Some("application/json")
        );
        assert_eq!(stored.http.etag.as_deref(), Some("W/\"synthetic\""));
        assert!(persistence.success.is_none());
    }

    #[tokio::test]
    async fn fake_fetch_runs_through_real_sqlite_reconcile_and_observation_pipeline() {
        let mut storage = OperationalStorage::open_in_memory().expect("storage");
        publish_catalog(&mut storage);
        let watched = SectionKey::try_new("92026", "NB", "00001").expect("watched Section");
        let execution = SharedOpenService::new(&mut storage)
            .execute_with(
                command(1),
                &mut clock(),
                |_| async { Ok(response()) },
                |_| vec![watched.clone()],
            )
            .await
            .expect("real storage execution");

        assert_eq!(
            execution.outcome.classification,
            OpenAttemptClassification::ValidApplied
        );
        assert_eq!(execution.outcome.duplicate_count, 1);
        assert_eq!(execution.outcome.orphan_count, 1);
        let attempt = storage
            .open_attempt(&trace(1))
            .expect("attempt read")
            .expect("attempt");
        assert_eq!(attempt.http.decoded_bytes, Some(32));
        assert_eq!(
            attempt.http.decoded_body_sha256.as_deref(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(
            attempt.http.content_type.as_deref(),
            Some("application/json")
        );
        assert_eq!(attempt.http.etag.as_deref(), Some("W/\"synthetic\""));
        assert_eq!(attempt.http.cache_control.as_deref(), Some("max-age=5"));
        assert_eq!(attempt.http.age_seconds, Some(2));
        assert_eq!(
            attempt.http.last_modified.as_deref(),
            Some("Tue, 14 Jul 2026 03:59:00 GMT")
        );
        let current = storage
            .open_section_current(&batch().target())
            .expect("current states");
        assert_eq!(current.len(), 2);
        assert_eq!(current[0].state, OpenSectionState::Open);
        assert_eq!(current[1].state, OpenSectionState::Closed);
        let events = storage
            .read_open_section_events(0, 10)
            .expect("Section events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].section, watched);
        assert_ne!(events[0].observation_id, events[0].refresh_observation_id);
        assert_eq!(
            storage
                .open_day_counters(&batch().target(), "2026-07-13")
                .expect("day counters")
                .succeeded,
            1
        );
    }

    #[test]
    fn rutgers_day_uses_new_york_boundary_across_utc_midnight() {
        assert_eq!(
            rutgers_day_at(datetime!(2026-07-14 03:59:59 UTC)).expect("Rutgers day"),
            "2026-07-13"
        );
        assert_eq!(
            rutgers_day_at(datetime!(2026-07-14 04:00:00 UTC)).expect("Rutgers day"),
            "2026-07-14"
        );
    }

    #[test]
    fn retry_after_http_date_is_relative_to_completion_time() {
        assert_eq!(
            retry_after_delay(
                RetryAfterHeader::Valid(RetryAfterValue::HttpDateUnixSeconds(1_030)),
                OffsetDateTime::from_unix_timestamp(1_000).expect("timestamp")
            ),
            Some(Duration::from_secs(30))
        );
        assert_eq!(
            retry_after_delay(
                RetryAfterHeader::Valid(RetryAfterValue::HttpDateUnixSeconds(900)),
                OffsetDateTime::from_unix_timestamp(1_000).expect("timestamp")
            ),
            Some(Duration::ZERO)
        );
    }
}
