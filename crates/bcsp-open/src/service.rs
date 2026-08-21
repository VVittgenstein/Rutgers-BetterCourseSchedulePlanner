use std::collections::BTreeSet;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use bcsp_contracts::{
    CatalogContentVersion, CatalogContentVersionError, OPEN_CONTRACT_VERSION, OpenBatchKey,
    OpenCounterSnapshotV1, OpenObservationTargetMismatchError, OpenObservationV1, OpenPullCountsV1,
    OpenSequence, OpenSequenceError, OpenState, RutgersDay, RutgersDayTimezone, SectionIndex,
    SectionKey, TermCampusKey, TraceId,
};
use bcsp_operational_storage::{
    BeginOpenPullAttemptCommand, FinishOpenPullFailureCommand, FinishOpenPullSuccessCommand,
    OpenAttemptClassification, OpenAttemptCounters, OpenCacheStatus, OpenCatalogSnapshot,
    OpenCommitOutcome, OpenGateAttemptSummary, OpenHttpAuditMetadata, OpenObservationCommit,
    OpenRequestLane, OpenSectionEvent, OpenSectionState, OperationalStorage, StorageError,
    StorageResult,
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

use crate::gate::{
    CatalogSetIdentity, GateDecision, GateDisposition, GateSample, RestartAttemptSummary,
    TargetGateSet, catalog_section_set_identity_v1, rebuild_after_restart,
};
use crate::{
    CatalogOpenBatch, CompletionSchedule, OpenCounterAudience, OpenFailureKind,
    OpenReconcileClassification, OpenReconcilePlan, OpenRefreshIntervals, OpenSetEvidence,
    OriginSchedulerLane, ReconcileInputError, reconcile_open_set, retry_directive,
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

    /// Integrity-gate seeding read: the last-applied OPEN index set.
    /// REQUIRED, deliberately without a default: a fail-open default here
    /// silently left the production wrapper returning `None` and the gate
    /// permanently unseeded. Every implementation must decide explicitly.
    fn lkg_open_index_set(
        &mut self,
        target: &TermCampusKey,
    ) -> StorageResult<Option<BTreeSet<SectionIndex>>>;

    /// Integrity-gate restart read: newest-first completed attempt summaries
    /// for the restart rebuild rules. REQUIRED for the same reason as
    /// [`Self::lkg_open_index_set`].
    fn recent_open_gate_attempt_summaries(
        &mut self,
        target: &TermCampusKey,
        limit: u32,
    ) -> StorageResult<Vec<OpenGateAttemptSummary>>;
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

    fn lkg_open_index_set(
        &mut self,
        target: &TermCampusKey,
    ) -> StorageResult<Option<BTreeSet<SectionIndex>>> {
        let Some(raw) = OperationalStorage::serving_lkg_open_index_set(self, target)? else {
            return Ok(None);
        };
        let mut indexes = BTreeSet::new();
        for value in raw {
            indexes.insert(SectionIndex::try_from(value.as_str())?);
        }
        Ok(Some(indexes))
    }

    fn recent_open_gate_attempt_summaries(
        &mut self,
        target: &TermCampusKey,
        limit: u32,
    ) -> StorageResult<Vec<OpenGateAttemptSummary>> {
        OperationalStorage::recent_open_gate_attempt_summaries(self, target, limit)
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
    pub refresh_intervals: OpenRefreshIntervals,
    pub active_watch_count: u64,
    /// Basis used only to derive the validity horizon of the committed
    /// observation. Normal/demanded pulls set this to the effective 30s/10s
    /// cadence. A bounded first-load one-shot may use a generation-sized
    /// horizon without turning the target into a recurring origin job.
    pub freshness_interval_seconds: u32,
    pub lane: OriginSchedulerLane,
    pub scheduler_lag: Duration,
    pub current_failure_streak: u32,
    pub counter_audience: OpenCounterAudience,
    /// Snapshot integrity gate wiring. `None` disables the gate for this
    /// pull (test fakes, paths predating the gate); production coordinators
    /// pass the target's shared gate set and the route this pull takes.
    pub gate: Option<OpenGateWiring>,
}

/// Shared handle to one target's gate runtimes plus the route of this pull.
/// The same `Arc` must be shared by every execution path of the target
/// (primary, concurrent-serving, candidate) -- the per-target serial lock
/// property depends on it.
#[derive(Clone)]
pub struct OpenGateWiring {
    pub gates: Arc<Mutex<TargetGateSet>>,
    pub route: OpenGateRoute,
}

impl std::fmt::Debug for OpenGateWiring {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OpenGateWiring")
            .field("route", &self.route)
            .finish_non_exhaustive()
    }
}

impl PartialEq for OpenGateWiring {
    fn eq(&self, other: &Self) -> bool {
        // Identity semantics: the same shared gate set and the same route.
        Arc::ptr_eq(&self.gates, &other.gates) && self.route == other.route
    }
}

impl Eq for OpenGateWiring {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenGateRoute {
    /// A pull reconciled against the serving catalog.
    Serving,
    /// A pull paired with an unpublished candidate catalog: routed to an
    /// isolated candidate runtime keyed by the candidate's set identity.
    Candidate,
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

impl OpenPullFailure {
    pub const fn diagnostic_code(&self) -> &'static str {
        match self {
            Self::ResponseTargetMismatch(_) => "OPEN_RESPONSE_TARGET_MISMATCH",
            Self::ResponseMetadataMismatch(_) => "OPEN_RESPONSE_METADATA_MISMATCH",
            Self::CatalogUnavailableAfterFetch(_) => "OPEN_CATALOG_UNAVAILABLE_AFTER_FETCH",
            Self::Upstream(failure) => match failure.kind() {
                OpenSectionsError::Timeout => "OPEN_TIMEOUT",
                OpenSectionsError::Network => "OPEN_NETWORK",
                OpenSectionsError::RequestConstruction => "OPEN_REQUEST_CONSTRUCTION",
                OpenSectionsError::ResponseUrlMismatch => "OPEN_RESPONSE_URL_MISMATCH",
                OpenSectionsError::TransientHttp { .. } => "OPEN_TRANSIENT_HTTP",
                OpenSectionsError::RateLimited { .. } => "OPEN_RATE_LIMITED",
                OpenSectionsError::Forbidden => "OPEN_FORBIDDEN",
                OpenSectionsError::Redirect { scope, .. } => match scope {
                    RedirectScope::OffOrigin => "OPEN_OFF_ORIGIN_REDIRECT",
                    RedirectScope::SameOrigin => "OPEN_SAME_ORIGIN_REDIRECT",
                    RedirectScope::InvalidLocation => "OPEN_INVALID_REDIRECT",
                },
                OpenSectionsError::FatalClientHttp { .. } => "OPEN_FATAL_CLIENT_HTTP",
                OpenSectionsError::UnsupportedHttp { .. } => "OPEN_UNSUPPORTED_HTTP",
                OpenSectionsError::InvalidContentType => "OPEN_INVALID_CONTENT_TYPE",
                OpenSectionsError::ResponseTooLarge => "OPEN_RESPONSE_TOO_LARGE",
                OpenSectionsError::ContentDecoding => "OPEN_CONTENT_DECODING",
                OpenSectionsError::InvalidJson => "OPEN_INVALID_JSON",
                OpenSectionsError::RootNotArray => "OPEN_ROOT_NOT_ARRAY",
                OpenSectionsError::NonStringValue { .. } => "OPEN_NON_STRING_VALUE",
                OpenSectionsError::InvalidSectionIndex { .. } => "OPEN_INVALID_SECTION_INDEX",
            },
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenPullExecution {
    pub outcome: OpenCommitOutcome,
    pub scheduler_completion: CompletionSchedule,
    pub terminal: OpenPullTerminal,
    pub reconcile: Option<OpenReconcilePlan>,
    /// Committed per-watch observations ready for fanout; non-valid pulls are empty.
    pub observations: Vec<OpenObservationV1>,
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
    #[error("local Open counter audience must use the command run ID")]
    CounterAudienceRunMismatch,
    #[error("committed Open observation sequence was invalid")]
    ObservationSequence(#[from] OpenSequenceError),
    #[error("committed Open observation target was inconsistent")]
    ObservationTarget(#[from] OpenObservationTargetMismatchError),
    #[error("committed Open observation handoff was inconsistent: {reason}")]
    ObservationCommit { reason: &'static str },
    #[error("Open observation freshness deadline overflowed")]
    FreshUntilOverflow,
    #[error("Open observation freshness interval must be nonzero")]
    ZeroFreshnessInterval,
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
        validate_counter_audience(&command)?;
        if command.freshness_interval_seconds == 0 {
            return Err(SharedOpenServiceError::ZeroFreshnessInterval);
        }
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
            .refresh_intervals
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
                requested_interval_seconds: Some(u64::from(
                    command.refresh_intervals.general().seconds(),
                )),
                effective_interval_seconds: Some(u64::from(command.freshness_interval_seconds)),
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
            .refresh_intervals
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
        // --- Snapshot integrity gate (design v5). Hard-reject shapes bypass
        // the gate entirely and are never samples; otherwise the per-target
        // serial lock spans decide -> storage commit -> advance (lock order:
        // gate, then the storage mutex inside the persistence call).
        let observed_intersection: BTreeSet<SectionIndex> = open_set
            .indexes()
            .filter(|index| catalog.contains_index(index))
            .cloned()
            .collect();
        let is_hard_reject = CatalogContentVersion::try_from(captured_catalog_content_version)?
            != catalog.content_version()
            || (open_set.unique_count() > 0
                && catalog.section_count() > 0
                && observed_intersection.is_empty());
        let mut gate_hold = false;
        let mut gate_advance: Option<(
            MutexGuard<'_, TargetGateSet>,
            CatalogSetIdentity,
            OpenGateRoute,
            GateDecision,
        )> = None;
        if let Some(wiring) = command.gate.as_ref() {
            if !is_hard_reject {
                let identity = catalog_section_set_identity_v1(catalog.section_indexes());
                let mut gates = wiring.gates.lock().expect("gate lock poisoned");
                let decision = {
                    let runtime = match wiring.route {
                        OpenGateRoute::Serving => {
                            if !gates.serving_installed() {
                                // Lazy restart rebuild on first use (design
                                // section 4, restart): seed from persisted
                                // LKG and continue an interrupted quarantine
                                // only per the contiguity/hash/gap rules.
                                let reference = self
                                    .persistence
                                    .lkg_open_index_set(&target)?
                                    .map(|set| lkg_reference_within(&set, &catalog));
                                let summaries = gate_restart_summaries(
                                    self.persistence
                                        .recent_open_gate_attempt_summaries(&target, 32)?,
                                );
                                gates.install_serving(rebuild_after_restart(
                                    identity.clone(),
                                    reference,
                                    &summaries,
                                    completed_at,
                                ));
                            }
                            let serving = gates.serving_mut();
                            if serving
                                .catalog_identity()
                                .is_some_and(|current| current != &identity)
                            {
                                // Serving catalog changed: reset the epoch
                                // with the LKG ∩ new-catalog transfer seed.
                                let transferred = self
                                    .persistence
                                    .lkg_open_index_set(&target)?
                                    .map(|set| lkg_reference_within(&set, &catalog));
                                serving.reset_for_catalog_identity(identity.clone(), transferred);
                            }
                            gates.serving_mut()
                        }
                        OpenGateRoute::Candidate => {
                            let seed = self
                                .persistence
                                .lkg_open_index_set(&target)?
                                .map(|set| lkg_reference_within(&set, &catalog));
                            gates.candidate_mut(&identity, || seed)
                        }
                    };
                    runtime.evaluate(&GateSample {
                        catalog_identity: &identity,
                        observed: &observed_intersection,
                        observed_at: completed_at,
                    })
                };
                gate_hold = decision.disposition == GateDisposition::Hold;
                gate_advance = Some((gates, identity, wiring.route, decision));
            }
        }
        let reconcile = reconcile_open_set(
            CatalogContentVersion::try_from(captured_catalog_content_version)?,
            &catalog,
            &open_set,
            gate_hold,
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
        let expected_watched_sections = watched_sections.iter().cloned().collect::<BTreeSet<_>>();
        let outcome = self
            .persistence
            .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold,
                gate_catalog_set_identity: gate_advance
                    .as_ref()
                    .map(|(_, identity, _, _)| identity.as_str().to_owned()),
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
        // Gate state advances only after the transaction committed and the
        // cross-check agreed; a storage-side catalog race means the decision
        // was computed against a superseded catalog and is discarded (the
        // guard drops here either way, releasing the serial lock).
        if let Some((mut gates, identity, route, decision)) = gate_advance.take() {
            if outcome.classification != OpenAttemptClassification::StaleCatalogRace {
                match route {
                    OpenGateRoute::Serving => gates.serving_mut().advance(&decision),
                    OpenGateRoute::Candidate => {
                        gates.candidate_mut(&identity, || None).advance(&decision);
                        if outcome.classification.is_success() {
                            // A successful candidate finish is the atomic
                            // catalog+open publish: promote the candidate
                            // runtime to serving inside this same
                            // serial-lock critical section (design v5).
                            gates.promote_candidate(&identity);
                        }
                    }
                }
            }
        }
        let observations = if outcome.classification.is_success() {
            build_committed_observations(&command, &outcome, &expected_watched_sections)?
        } else {
            if outcome.observation_commit.is_some() {
                return Err(SharedOpenServiceError::ObservationCommit {
                    reason: "a non-valid pull returned an observation commit",
                });
            }
            Vec::new()
        };
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
            OpenAttemptClassification::SuspectPartialSnapshot => (
                CompletionSchedule::Retry(retry_directive(
                    &target,
                    effective_interval,
                    command.current_failure_streak,
                    OpenFailureKind::SuspectPartial,
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
            observations,
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
        if outcome.observation_commit.is_some() {
            return Err(SharedOpenServiceError::ObservationCommit {
                reason: "a failed pull returned an observation commit",
            });
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
            observations: Vec::new(),
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

fn validate_counter_audience(command: &OpenPullCommand) -> Result<(), SharedOpenServiceError> {
    if let OpenCounterAudience::Local { run_id } = command.counter_audience
        && run_id != command.run_id
    {
        return Err(SharedOpenServiceError::CounterAudienceRunMismatch);
    }
    Ok(())
}

fn build_committed_observations(
    command: &OpenPullCommand,
    outcome: &OpenCommitOutcome,
    expected_watched_sections: &BTreeSet<SectionKey>,
) -> Result<Vec<OpenObservationV1>, SharedOpenServiceError> {
    let commit =
        outcome
            .observation_commit
            .as_ref()
            .ok_or(SharedOpenServiceError::ObservationCommit {
                reason: "a valid pull omitted its observation commit",
            })?;
    let refresh_observation_id =
        outcome
            .refresh_observation_id
            .ok_or(SharedOpenServiceError::ObservationCommit {
                reason: "a valid pull omitted its refresh observation ID",
            })?;
    let observation_sequence =
        outcome
            .observation_sequence
            .ok_or(SharedOpenServiceError::ObservationCommit {
                reason: "a valid pull omitted its observation sequence",
            })?;
    let pull_sequence = OpenSequence::try_from(outcome.attempt_sequence)?;
    let catalog_content_version = CatalogContentVersion::try_from(outcome.catalog_content_version)?;
    let counter_snapshot = committed_counter_snapshot(commit, command.counter_audience)?;
    if commit.effective_interval_seconds == 0 {
        return Err(SharedOpenServiceError::ObservationCommit {
            reason: "a valid pull returned a zero effective interval",
        });
    }
    let freshness_seconds = commit
        .effective_interval_seconds
        .checked_mul(2)
        .and_then(|value| value.checked_add(15))
        .and_then(|value| i64::try_from(value).ok())
        .ok_or(SharedOpenServiceError::FreshUntilOverflow)?;
    let batch = OpenBatchKey::from(command.source_request.target().target());
    let scheduler_lag_milliseconds = duration_millis(command.scheduler_lag);
    let mut observation_ids = BTreeSet::new();
    let mut section_keys = BTreeSet::new();
    let mut observations = Vec::with_capacity(commit.section_events.len());
    for event in &commit.section_events {
        validate_committed_section_event(
            event,
            command.attempt_id,
            refresh_observation_id,
            observation_sequence,
            outcome.catalog_content_version,
        )?;
        if !observation_ids.insert(event.observation_id) {
            return Err(SharedOpenServiceError::ObservationCommit {
                reason: "committed Section observation IDs are duplicated",
            });
        }
        if !section_keys.insert(event.section.clone()) {
            return Err(SharedOpenServiceError::ObservationCommit {
                reason: "committed watched Section identities are duplicated",
            });
        }
        let observed_at = OffsetDateTime::parse(&event.observed_at, &Rfc3339)
            .map_err(|_| SharedOpenServiceError::TimestampFormat)?;
        let fresh_until = observed_at
            .checked_add(time::Duration::seconds(freshness_seconds))
            .ok_or(SharedOpenServiceError::FreshUntilOverflow)?;
        let state = match event.state {
            OpenSectionState::Open => OpenState::Open,
            OpenSectionState::Closed => OpenState::Closed,
        };
        observations.push(OpenObservationV1::try_new(
            OPEN_CONTRACT_VERSION,
            event.observation_id,
            event.refresh_observation_id,
            batch.clone(),
            event.section.clone(),
            pull_sequence,
            catalog_content_version,
            state,
            observed_at,
            fresh_until,
            scheduler_lag_milliseconds,
            counter_snapshot.clone(),
        )?);
    }
    if section_keys != *expected_watched_sections {
        return Err(SharedOpenServiceError::ObservationCommit {
            reason: "committed Section events differ from the sampled watch set",
        });
    }
    Ok(observations)
}

fn validate_committed_section_event(
    event: &OpenSectionEvent,
    attempt_id: TraceId,
    refresh_observation_id: TraceId,
    observation_sequence: u64,
    catalog_content_version: u64,
) -> Result<(), SharedOpenServiceError> {
    if event.event_id == 0
        || event.attempt_id != attempt_id
        || event.refresh_observation_id != refresh_observation_id
        || event.observation_sequence != observation_sequence
        || event.catalog_content_version != catalog_content_version
    {
        return Err(SharedOpenServiceError::ObservationCommit {
            reason: "committed Section event disagrees with its target commit",
        });
    }
    Ok(())
}

fn committed_counter_snapshot(
    commit: &OpenObservationCommit,
    audience: OpenCounterAudience,
) -> Result<OpenCounterSnapshotV1, SharedOpenServiceError> {
    let (run_counts, today_counts) = match audience {
        OpenCounterAudience::Local { .. } => (
            Some(pull_counts(commit.run_counts)),
            pull_counts(commit.target_day_counts),
        ),
        OpenCounterAudience::Public => (None, pull_counts(commit.service_day_counts)),
    };
    Ok(OpenCounterSnapshotV1 {
        run_counts,
        today_counts,
        rutgers_day: RutgersDay::try_from(commit.rutgers_day.clone())
            .map_err(|_| SharedOpenServiceError::RutgersDay)?,
        day_timezone: RutgersDayTimezone::AmericaNewYork,
    })
}

const fn pull_counts(value: OpenAttemptCounters) -> OpenPullCountsV1 {
    OpenPullCountsV1 {
        attempted: value.attempted,
        succeeded: value.succeeded,
        failed: value.failed,
        empty: value.empty,
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
        OpenReconcileClassification::SuspectPartialSnapshot => {
            OpenAttemptClassification::SuspectPartialSnapshot
        }
        OpenReconcileClassification::StaleCatalogRace => {
            OpenAttemptClassification::StaleCatalogRace
        }
    };
    outcome.classification == expected_classification
        && outcome.catalog_content_version == reconcile.current_catalog_content_version.get()
        && outcome.source_value_count == reconcile.raw_value_count
        && outcome.catalog_section_count == reconcile.catalog_section_count
        // Compared against the updates-independent field so a withheld
        // (suspect) plan still cross-checks the true intersection.
        && outcome.intersection_count == reconcile.observed_intersection_count
        && outcome.orphan_count == reconcile.orphan_count
        && outcome.duplicate_count == reconcile.duplicate_count
}

/// |LKG open set ∩ catalog|, the gate's transfer/seed reference.
fn lkg_reference_within(lkg_open_set: &BTreeSet<SectionIndex>, catalog: &CatalogOpenBatch) -> u64 {
    lkg_open_set
        .iter()
        .filter(|index| catalog.contains_index(index))
        .count() as u64
}

/// Maps persisted attempt summaries to the restart-rebuild input. Rows that
/// are not fully usable (missing hash or unparsable completion time) become
/// run breakers so the rebuild never continues an episode across them.
fn gate_restart_summaries(rows: Vec<OpenGateAttemptSummary>) -> Vec<RestartAttemptSummary> {
    rows.into_iter()
        .map(|row| {
            let completed_at = row
                .completed_at
                .as_deref()
                .and_then(|text| OffsetDateTime::parse(text, &Rfc3339).ok());
            // An unparsable stored identity degrades to `None`, which the
            // rebuild treats as a run breaker -- never as a match.
            let catalog_set_identity = row
                .catalog_set_identity
                .as_deref()
                .and_then(CatalogSetIdentity::from_sha256_hex);
            match (row.classification, row.canonical_set_sha256, completed_at) {
                (
                    OpenAttemptClassification::SuspectPartialSnapshot,
                    Some(canonical_set_sha256),
                    Some(completed_at),
                ) => RestartAttemptSummary {
                    is_suspect_partial: true,
                    canonical_set_sha256,
                    completed_at,
                    catalog_set_identity,
                },
                (_, hash, at) => RestartAttemptSummary {
                    is_suspect_partial: false,
                    canonical_set_sha256: hash.unwrap_or_default(),
                    completed_at: at.unwrap_or(OffsetDateTime::UNIX_EPOCH),
                    catalog_set_identity,
                },
            }
        })
        .collect()
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
    use std::collections::{BTreeMap, BTreeSet, VecDeque};
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
            let canonical_indices = self
                .success
                .as_ref()
                .map(|command| {
                    command
                        .open_sections
                        .iter()
                        .map(|section| section.index().clone())
                        .collect::<BTreeSet<_>>()
                })
                .unwrap_or_default();
            let source_value_count = self
                .success
                .as_ref()
                .map_or(0, |command| command.source_value_count);
            let intersection_count = canonical_indices
                .iter()
                .filter(|index| matches!(index.as_str(), "00001" | "00002"))
                .count() as u64;
            let unique_value_count = canonical_indices.len() as u64;
            let orphan_count = unique_value_count.saturating_sub(intersection_count);
            let duplicate_count = source_value_count.saturating_sub(unique_value_count);
            let refresh_observation_id = trace(10);
            let observation_commit = classification.is_success().then(|| {
                let section_events = self
                    .success
                    .as_ref()
                    .expect("success command before outcome")
                    .watched_sections
                    .iter()
                    .enumerate()
                    .map(|(ordinal, section)| OpenSectionEvent {
                        event_id: u64::try_from(ordinal + 1).expect("event ID"),
                        observation_id: trace(
                            u8::try_from(ordinal + 20).expect("observation ID suffix"),
                        ),
                        refresh_observation_id,
                        attempt_id: trace(1),
                        section: section.clone(),
                        state: if canonical_indices.contains(section.index()) {
                            OpenSectionState::Open
                        } else {
                            OpenSectionState::Closed
                        },
                        catalog_content_version: 7,
                        observation_sequence: 1,
                        observed_at: "2026-07-14T04:00:00Z".to_owned(),
                    })
                    .collect();
                OpenObservationCommit {
                    rutgers_day: "2026-07-13".to_owned(),
                    effective_interval_seconds: 10,
                    run_counts: OpenAttemptCounters {
                        attempted: 1,
                        succeeded: 1,
                        failed: 0,
                        empty: 0,
                    },
                    target_day_counts: OpenAttemptCounters {
                        attempted: 2,
                        succeeded: 1,
                        failed: 1,
                        empty: 0,
                    },
                    service_day_counts: OpenAttemptCounters {
                        attempted: 9,
                        succeeded: 7,
                        failed: 2,
                        empty: 0,
                    },
                    section_events,
                }
            });
            OpenCommitOutcome {
                attempt_id: trace(1),
                attempt_sequence: 1,
                classification,
                refresh_observation_id: classification
                    .is_success()
                    .then_some(refresh_observation_id),
                observation_sequence: classification.is_success().then_some(1),
                catalog_content_version: 7,
                source_value_count,
                catalog_section_count: 2,
                intersection_count,
                orphan_count,
                duplicate_count,
                changed_section_count: if classification.is_success() { 2 } else { 0 },
                body_changed: classification.is_success(),
                state_changed: classification.is_success(),
                retained_lkg_attempt_id: classification.is_success().then_some(trace(1)),
                observation_commit,
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

        fn lkg_open_index_set(
            &mut self,
            _target: &TermCampusKey,
        ) -> StorageResult<Option<BTreeSet<SectionIndex>>> {
            Ok(None)
        }

        fn recent_open_gate_attempt_summaries(
            &mut self,
            _target: &TermCampusKey,
            _limit: u32,
        ) -> StorageResult<Vec<OpenGateAttemptSummary>> {
            Ok(Vec::new())
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
        command_for(1, 2, active_watch_count)
    }

    fn command_for(attempt_suffix: u8, run_suffix: u8, active_watch_count: u64) -> OpenPullCommand {
        OpenPullCommand {
            gate: None,
            attempt_id: trace(attempt_suffix),
            run_id: trace(run_suffix),
            source_request: source_request(),
            refresh_intervals: OpenRefreshIntervals::local_default(),
            active_watch_count,
            freshness_interval_seconds: if active_watch_count > 0 { 10 } else { 30 },
            lane: OriginSchedulerLane::OpenActiveWatch,
            scheduler_lag: Duration::from_millis(250),
            current_failure_streak: 0,
            counter_audience: OpenCounterAudience::Local {
                run_id: trace(run_suffix),
            },
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
        let mut first_load = command(0);
        first_load.freshness_interval_seconds = 3_600;
        let target = first_load.source_request.target().target();
        let execution = service
            .execute_with(
                first_load,
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
        assert_eq!(
            execution.scheduler_completion,
            CompletionSchedule::Retry(retry_directive(
                &target,
                Duration::from_secs(30),
                0,
                OpenFailureKind::Transient,
            )),
            "a first-load freshness lease must not lengthen the real 30-second retry cadence"
        );
        let stored = persistence.failure.expect("failure finalization");
        assert_eq!(stored.http.http_status, Some(503));
        assert_eq!(stored.error_code, "OPEN_TRANSIENT_HTTP");
    }

    #[tokio::test]
    async fn content_decoding_failure_uses_target_local_content_retry() {
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
        assert_eq!(directive.mode, crate::RetryMode::Automatic);
        assert!(directive.delay >= Duration::from_secs(60));
        assert!(directive.delay <= Duration::from_secs(66));
        assert_eq!(
            persistence.failure.expect("failure").error_code,
            "OPEN_CONTENT_DECODING"
        );
    }

    #[tokio::test]
    async fn typed_empty_response_is_valid_and_closes_the_catalog() {
        let mut persistence = FakePersistence::new(OpenAttemptClassification::ValidApplied);
        let mut service = SharedOpenService::new(&mut persistence);
        let execution = service
            .execute_with(
                command(0),
                &mut clock(),
                |_| async { Ok(empty_response()) },
                |_| Vec::new(),
            )
            .await
            .expect("valid empty classification");
        assert!(matches!(execution.terminal, OpenPullTerminal::Valid(_)));
        assert_eq!(execution.scheduler_completion, CompletionSchedule::Success);
        assert_eq!(
            execution
                .reconcile
                .expect("read-side reconcile")
                .closed_count,
            2
        );
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
    async fn local_counter_audience_must_match_the_pull_run_before_any_side_effect() {
        let mut persistence = FakePersistence::new(OpenAttemptClassification::ValidApplied);
        let began = Arc::clone(&persistence.began);
        let mut mismatched = command(0);
        mismatched.counter_audience = OpenCounterAudience::Local { run_id: trace(99) };
        let result = SharedOpenService::new(&mut persistence)
            .execute_with(
                mismatched,
                &mut clock(),
                |_| async { Ok(response()) },
                |_| Vec::new(),
            )
            .await;
        assert!(matches!(
            result,
            Err(SharedOpenServiceError::CounterAudienceRunMismatch)
        ));
        assert!(!began.load(Ordering::SeqCst));
        assert!(persistence.success.is_none());
        assert!(persistence.failure.is_none());
    }

    #[tokio::test]
    async fn real_sqlite_handoff_has_exact_watch_cardinality_shared_ids_and_unchanged_events() {
        let mut storage = OperationalStorage::open_in_memory().expect("storage");
        publish_catalog(&mut storage);
        let watched_open = SectionKey::try_new("92026", "NB", "00001").expect("open watch");
        let watched_closed = SectionKey::try_new("92026", "NB", "00002").expect("closed watch");
        let watched = vec![watched_open.clone(), watched_closed.clone()];
        let first = SharedOpenService::new(&mut storage)
            .execute_with(
                command(1),
                &mut clock(),
                |_| async { Ok(response()) },
                |_| watched.clone(),
            )
            .await
            .expect("real storage execution");

        assert_eq!(
            first.outcome.classification,
            OpenAttemptClassification::ValidApplied
        );
        assert_eq!(first.outcome.duplicate_count, 1);
        assert_eq!(first.outcome.orphan_count, 1);
        assert_eq!(first.observations.len(), 2);
        let first_refresh_id = first
            .outcome
            .refresh_observation_id
            .expect("refresh observation ID");
        assert!(
            first
                .observations
                .iter()
                .all(|value| value.refresh_observation_id == first_refresh_id)
        );
        assert_ne!(
            first.observations[0].observation_id,
            first.observations[1].observation_id
        );
        assert_eq!(first.observations[0].section_key(), &watched_open);
        assert_eq!(first.observations[0].state, OpenState::Open);
        assert_eq!(first.observations[1].section_key(), &watched_closed);
        assert_eq!(first.observations[1].state, OpenState::Closed);
        assert_eq!(first.observations[0].pull_sequence.get(), 1);
        assert_eq!(first.observations[0].catalog_content_version.get(), 1);
        assert_eq!(
            first.observations[0].observed_at,
            datetime!(2026-07-14 04:00:00 UTC)
        );
        assert_eq!(
            first.observations[0].fresh_until,
            datetime!(2026-07-14 04:00:35 UTC)
        );
        assert_eq!(first.observations[0].scheduler_lag_milliseconds, 250);
        assert_eq!(
            first.observations[0].counter_snapshot,
            first.observations[1].counter_snapshot
        );
        assert_eq!(
            first.observations[0]
                .counter_snapshot
                .run_counts
                .as_ref()
                .expect("local run counters")
                .succeeded,
            1
        );
        assert_eq!(
            first.observations[0]
                .counter_snapshot
                .today_counts
                .succeeded,
            1
        );
        assert_eq!(
            first.observations[0].counter_snapshot.rutgers_day.as_str(),
            "2026-07-13"
        );
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
        let first_events = storage
            .read_open_section_events(0, 10)
            .expect("Section events");
        assert_eq!(first_events.len(), 2);
        for (event, observation) in first_events.iter().zip(&first.observations) {
            assert_eq!(event.observation_id, observation.observation_id);
            assert_eq!(
                event.refresh_observation_id,
                observation.refresh_observation_id
            );
            assert_eq!(&event.section, observation.section_key());
        }

        let mut public_command = command_for(3, 2, 1);
        public_command.counter_audience = OpenCounterAudience::Public;
        let second = SharedOpenService::new(&mut storage)
            .execute_with(
                public_command,
                &mut clock(),
                |_| async { Ok(response()) },
                |_| vec![watched_closed.clone(), watched_open.clone()],
            )
            .await
            .expect("unchanged real storage execution");
        assert!(!second.outcome.body_changed);
        assert!(!second.outcome.state_changed);
        assert_eq!(second.outcome.changed_section_count, 0);
        assert_eq!(second.observations.len(), 2);
        assert!(
            second
                .observations
                .iter()
                .all(|value| value.pull_sequence.get() == 2)
        );
        assert_eq!(
            second.observations[0].counter_snapshot,
            second.observations[1].counter_snapshot
        );
        assert_eq!(second.observations[0].counter_snapshot.run_counts, None);
        assert_eq!(
            second.observations[0]
                .counter_snapshot
                .today_counts
                .succeeded,
            2
        );
        let all_events = storage
            .read_open_section_events(0, 10)
            .expect("all Section events");
        assert_eq!(all_events.len(), 4);
        for (event, observation) in all_events[2..].iter().zip(&second.observations) {
            assert_eq!(event.observation_id, observation.observation_id);
            assert_eq!(
                event.refresh_observation_id,
                observation.refresh_observation_id
            );
            assert_eq!(&event.section, observation.section_key());
        }
        assert_eq!(
            storage
                .open_day_counters(&batch().target(), "2026-07-13")
                .expect("day counters")
                .succeeded,
            2
        );
    }

    #[tokio::test]
    async fn real_sqlite_empty_success_hands_off_closed_state_and_failure_does_not() {
        let mut storage = OperationalStorage::open_in_memory().expect("storage");
        publish_catalog(&mut storage);
        let watched = SectionKey::try_new("92026", "NB", "00001").expect("watched Section");
        let empty_execution = SharedOpenService::new(&mut storage)
            .execute_with(
                command_for(4, 5, 1),
                &mut clock(),
                |_| async { Ok(empty_response()) },
                |_| vec![watched.clone()],
            )
            .await
            .expect("empty response committed");
        assert_eq!(
            empty_execution.outcome.classification,
            OpenAttemptClassification::ValidApplied
        );
        assert_eq!(empty_execution.observations.len(), 1);
        assert_eq!(empty_execution.observations[0].state, OpenState::Closed);
        assert!(empty_execution.outcome.observation_commit.is_some());

        let failed_execution = SharedOpenService::new(&mut storage)
            .execute_with(
                command_for(6, 5, 1),
                &mut clock(),
                |_| async {
                    let mut value = response();
                    value.metadata.duplicate_value_count = 0;
                    Ok(value)
                },
                |_| vec![watched],
            )
            .await
            .expect("failed response recorded");
        assert!(matches!(
            failed_execution.terminal,
            OpenPullTerminal::Failed(OpenPullFailure::ResponseMetadataMismatch(_))
        ));
        assert!(failed_execution.observations.is_empty());
        assert!(failed_execution.outcome.observation_commit.is_none());
        assert_eq!(
            storage
                .read_open_section_events(0, 10)
                .expect("Section events")
                .len(),
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

    /// PR-checklist item 1 (design section 6): under the per-target serial
    /// lock the reviewer's stale-Confirm counterexample is impossible BY
    /// CONSTRUCTION -- decisions are computed inside the lock -- and the
    /// epoch assertion in `GateRuntime::advance` is the tripwire for any
    /// bypass. This test drives two REAL service executions concurrently on
    /// one shared gate and asserts: (a) the commit critical sections never
    /// overlap (B holds the lock through its storage commit while A blocks),
    /// (b) commits land in lock order (B's Suspect first, then A re-decided
    /// against B's post-state), and (c) no epoch panic occurred.
    #[tokio::test(flavor = "multi_thread", worker_threads = 3)]
    async fn serial_lock_spans_decide_commit_advance_across_concurrent_pulls() {
        use crate::gate::{GateRuntime, TargetGateSet, catalog_section_set_identity_v1};

        fn probe_index(value: usize) -> SectionIndex {
            SectionIndex::try_from(format!("{value:05}").as_str()).expect("probe index")
        }
        fn probe_response(count: usize) -> OpenSectionsResponse {
            let open_indexes: Vec<SectionIndex> = (0..count).map(probe_index).collect();
            let canonical_set_sha256 = canonical_open_set_sha256(&open_indexes);
            OpenSectionsResponse {
                target: batch(),
                classification: OpenPayloadClassification::Nonempty,
                metadata: OpenResponseMetadata {
                    http_status: 200,
                    decoded_bytes: 64,
                    decoded_body_sha256:
                        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                            .to_owned(),
                    canonical_set_sha256,
                    raw_value_count: count,
                    unique_value_count: count,
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

        #[derive(Clone)]
        struct GateProbePersistence {
            catalog: Vec<SectionKey>,
            committed: Arc<Mutex<Vec<OpenAttemptClassification>>>,
            in_finish: Arc<AtomicBool>,
            overlap: Arc<AtomicBool>,
            first_finish_entered: Arc<tokio::sync::Notify>,
            notified_once: Arc<AtomicBool>,
        }

        impl OpenPullPersistence for GateProbePersistence {
            fn serving_open_catalog_snapshot(
                &mut self,
                _target: &TermCampusKey,
            ) -> StorageResult<Option<OpenCatalogSnapshot>> {
                Ok(Some(OpenCatalogSnapshot {
                    target: batch().target(),
                    content_version: 7,
                    sections: self.catalog.clone(),
                }))
            }

            fn begin_open_pull_attempt(
                &mut self,
                _command: &BeginOpenPullAttemptCommand,
            ) -> StorageResult<u64> {
                Ok(1)
            }

            fn finish_open_pull_success(
                &mut self,
                command: FinishOpenPullSuccessCommand,
            ) -> StorageResult<OpenCommitOutcome> {
                if self.in_finish.swap(true, Ordering::SeqCst) {
                    self.overlap.store(true, Ordering::SeqCst);
                }
                if !self.notified_once.swap(true, Ordering::SeqCst) {
                    // First committer (B) is inside the critical section:
                    // release A so it contends for the gate lock NOW.
                    self.first_finish_entered.notify_one();
                }
                std::thread::sleep(std::time::Duration::from_millis(40));
                let canonical: BTreeSet<SectionIndex> = command
                    .open_sections
                    .iter()
                    .map(|section| section.index().clone())
                    .collect();
                let catalog_indices: BTreeSet<SectionIndex> = self
                    .catalog
                    .iter()
                    .map(|section| section.index().clone())
                    .collect();
                let intersection_count = canonical.intersection(&catalog_indices).count() as u64;
                // Mirror the storage classify contract for non-hard shapes.
                let classification = if command.gate_hold {
                    OpenAttemptClassification::SuspectPartialSnapshot
                } else {
                    OpenAttemptClassification::ValidApplied
                };
                self.committed
                    .lock()
                    .expect("commit log")
                    .push(classification);
                let unique = canonical.len() as u64;
                let outcome = OpenCommitOutcome {
                    attempt_id: command.attempt_id,
                    attempt_sequence: 1,
                    classification,
                    refresh_observation_id: classification.is_success().then_some(trace(10)),
                    observation_sequence: classification.is_success().then_some(1),
                    catalog_content_version: 7,
                    source_value_count: command.source_value_count,
                    catalog_section_count: self.catalog.len() as u64,
                    intersection_count,
                    orphan_count: unique.saturating_sub(intersection_count),
                    duplicate_count: command.source_value_count.saturating_sub(unique),
                    changed_section_count: 0,
                    body_changed: true,
                    state_changed: false,
                    retained_lkg_attempt_id: None,
                    observation_commit: classification.is_success().then(|| OpenObservationCommit {
                        rutgers_day: "2026-08-19".to_owned(),
                        effective_interval_seconds: 10,
                        run_counts: OpenAttemptCounters {
                            attempted: 1,
                            succeeded: 1,
                            failed: 0,
                            empty: 0,
                        },
                        target_day_counts: OpenAttemptCounters {
                            attempted: 1,
                            succeeded: 1,
                            failed: 0,
                            empty: 0,
                        },
                        service_day_counts: OpenAttemptCounters {
                            attempted: 1,
                            succeeded: 1,
                            failed: 0,
                            empty: 0,
                        },
                        section_events: Vec::new(),
                    }),
                };
                self.in_finish.store(false, Ordering::SeqCst);
                Ok(outcome)
            }

            fn finish_open_pull_failure(
                &mut self,
                _command: &FinishOpenPullFailureCommand,
            ) -> StorageResult<OpenCommitOutcome> {
                unreachable!("probe pulls never fail")
            }

            fn lkg_open_index_set(
                &mut self,
                _target: &TermCampusKey,
            ) -> StorageResult<Option<BTreeSet<SectionIndex>>> {
                // The probe pre-installs a seeded serving runtime, so the
                // lazy-rebuild reads are never consulted.
                Ok(None)
            }

            fn recent_open_gate_attempt_summaries(
                &mut self,
                _target: &TermCampusKey,
                _limit: u32,
            ) -> StorageResult<Vec<OpenGateAttemptSummary>> {
                Ok(Vec::new())
            }
        }

        let catalog: Vec<SectionKey> = (0..300)
            .map(|value| {
                SectionKey::try_new("92026", "NB", format!("{value:05}").as_str())
                    .expect("catalog section")
            })
            .collect();
        let identity = catalog_section_set_identity_v1(
            catalog
                .iter()
                .map(|section| section.index())
                .collect::<BTreeSet<_>>()
                .into_iter(),
        );
        let mut initial_gates = TargetGateSet::new();
        initial_gates.install_serving(GateRuntime::seeded(identity, 300));
        let gates = Arc::new(Mutex::new(initial_gates));

        let shared = GateProbePersistence {
            catalog,
            committed: Arc::new(Mutex::new(Vec::new())),
            in_finish: Arc::new(AtomicBool::new(false)),
            overlap: Arc::new(AtomicBool::new(false)),
            first_finish_entered: Arc::new(tokio::sync::Notify::new()),
            notified_once: Arc::new(AtomicBool::new(false)),
        };
        let committed = Arc::clone(&shared.committed);
        let overlap = Arc::clone(&shared.overlap);
        let release_a = Arc::clone(&shared.first_finish_entered);

        let mut command_a = command_for(31, 32, 0);
        command_a.gate = Some(OpenGateWiring {
            gates: Arc::clone(&gates),
            route: OpenGateRoute::Serving,
        });
        let mut command_b = command_for(41, 42, 0);
        command_b.gate = Some(OpenGateWiring {
            gates: Arc::clone(&gates),
            route: OpenGateRoute::Serving,
        });

        let mut persistence_a = shared.clone();
        let mut persistence_b = shared.clone();
        let task_a = async {
            SharedOpenService::new(&mut persistence_a)
                .execute_with(
                    command_a,
                    &mut clock(),
                    |_| async {
                        release_a.notified().await;
                        Ok(probe_response(290))
                    },
                    |_| Vec::new(),
                )
                .await
                .expect("pull A")
        };
        let task_b = async {
            SharedOpenService::new(&mut persistence_b)
                .execute_with(
                    command_b,
                    &mut clock(),
                    |_| async { Ok(probe_response(240)) },
                    |_| Vec::new(),
                )
                .await
                .expect("pull B")
        };
        let (execution_a, execution_b) = tokio::join!(task_a, task_b);

        assert!(
            !overlap.load(Ordering::SeqCst),
            "commit critical sections must never overlap under the gate lock",
        );
        assert_eq!(
            committed.lock().expect("commit log").as_slice(),
            &[
                OpenAttemptClassification::SuspectPartialSnapshot,
                OpenAttemptClassification::ValidApplied,
            ],
            "B's held commit lands first; A is re-decided against B's post-state",
        );
        assert!(matches!(execution_b.terminal, OpenPullTerminal::Unsafe(_)));
        assert!(matches!(execution_a.terminal, OpenPullTerminal::Valid(_)));
        assert!(
            !gates
                .lock()
                .expect("gate set")
                .serving_mut()
                .is_quarantined(),
            "A's recovery closed the quarantine",
        );
    }

    /// Reviewer merge-blocker pin: the serving lazy rebuild on first use MUST
    /// consume the persisted LKG set and the persisted suspect history via
    /// the persistence seam.
    ///
    /// Pull 1 (partial 200 of 300) proves the LKG read: a fail-open `None`
    /// would leave the runtime unseeded and the partial would seed-and-apply;
    /// with the read it must Hold against the LKG floor. Pull 2 proves the
    /// history read: the persisted 3-suspect run makes the second live sample
    /// the confirming one (count 5, span >= 300s), while ignored history
    /// would leave count at 2 and keep holding.
    #[tokio::test]
    async fn serving_lazy_rebuild_consumes_persisted_lkg_and_history() {
        use crate::gate::{TargetGateSet, catalog_section_set_identity_v1};

        fn probe_index(value: usize) -> SectionIndex {
            SectionIndex::try_from(format!("{value:05}").as_str()).expect("probe index")
        }
        fn probe_response(count: usize) -> OpenSectionsResponse {
            let open_indexes: Vec<SectionIndex> = (0..count).map(probe_index).collect();
            let canonical_set_sha256 = canonical_open_set_sha256(&open_indexes);
            OpenSectionsResponse {
                target: batch(),
                classification: OpenPayloadClassification::Nonempty,
                metadata: OpenResponseMetadata {
                    http_status: 200,
                    decoded_bytes: 64,
                    decoded_body_sha256:
                        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                            .to_owned(),
                    canonical_set_sha256,
                    raw_value_count: count,
                    unique_value_count: count,
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

        struct RebuildProbePersistence {
            catalog: Vec<SectionKey>,
            lkg: BTreeSet<SectionIndex>,
            history: Vec<OpenGateAttemptSummary>,
            committed: Vec<OpenAttemptClassification>,
        }

        impl OpenPullPersistence for RebuildProbePersistence {
            fn serving_open_catalog_snapshot(
                &mut self,
                _target: &TermCampusKey,
            ) -> StorageResult<Option<OpenCatalogSnapshot>> {
                Ok(Some(OpenCatalogSnapshot {
                    target: batch().target(),
                    content_version: 7,
                    sections: self.catalog.clone(),
                }))
            }

            fn begin_open_pull_attempt(
                &mut self,
                _command: &BeginOpenPullAttemptCommand,
            ) -> StorageResult<u64> {
                Ok(1)
            }

            fn finish_open_pull_success(
                &mut self,
                command: FinishOpenPullSuccessCommand,
            ) -> StorageResult<OpenCommitOutcome> {
                let canonical: BTreeSet<SectionIndex> = command
                    .open_sections
                    .iter()
                    .map(|section| section.index().clone())
                    .collect();
                let catalog_indices: BTreeSet<SectionIndex> = self
                    .catalog
                    .iter()
                    .map(|section| section.index().clone())
                    .collect();
                let intersection_count = canonical.intersection(&catalog_indices).count() as u64;
                let classification = if command.gate_hold {
                    OpenAttemptClassification::SuspectPartialSnapshot
                } else {
                    OpenAttemptClassification::ValidApplied
                };
                self.committed.push(classification);
                let unique = canonical.len() as u64;
                Ok(OpenCommitOutcome {
                    attempt_id: command.attempt_id,
                    attempt_sequence: 1,
                    classification,
                    refresh_observation_id: classification.is_success().then_some(trace(10)),
                    observation_sequence: classification.is_success().then_some(1),
                    catalog_content_version: 7,
                    source_value_count: command.source_value_count,
                    catalog_section_count: self.catalog.len() as u64,
                    intersection_count,
                    orphan_count: unique.saturating_sub(intersection_count),
                    duplicate_count: command.source_value_count.saturating_sub(unique),
                    changed_section_count: 0,
                    body_changed: true,
                    state_changed: false,
                    retained_lkg_attempt_id: None,
                    observation_commit: classification.is_success().then(|| {
                        OpenObservationCommit {
                            rutgers_day: "2026-07-13".to_owned(),
                            effective_interval_seconds: 10,
                            run_counts: OpenAttemptCounters {
                                attempted: 1,
                                succeeded: 1,
                                failed: 0,
                                empty: 0,
                            },
                            target_day_counts: OpenAttemptCounters {
                                attempted: 1,
                                succeeded: 1,
                                failed: 0,
                                empty: 0,
                            },
                            service_day_counts: OpenAttemptCounters {
                                attempted: 1,
                                succeeded: 1,
                                failed: 0,
                                empty: 0,
                            },
                            section_events: Vec::new(),
                        }
                    }),
                })
            }

            fn finish_open_pull_failure(
                &mut self,
                _command: &FinishOpenPullFailureCommand,
            ) -> StorageResult<OpenCommitOutcome> {
                unreachable!("rebuild probe pulls never fail")
            }

            fn lkg_open_index_set(
                &mut self,
                _target: &TermCampusKey,
            ) -> StorageResult<Option<BTreeSet<SectionIndex>>> {
                Ok(Some(self.lkg.clone()))
            }

            fn recent_open_gate_attempt_summaries(
                &mut self,
                _target: &TermCampusKey,
                _limit: u32,
            ) -> StorageResult<Vec<OpenGateAttemptSummary>> {
                Ok(self.history.clone())
            }
        }

        let catalog: Vec<SectionKey> = (0..300)
            .map(|value| {
                SectionKey::try_new("92026", "NB", format!("{value:05}").as_str())
                    .expect("catalog section")
            })
            .collect();
        let identity = catalog_section_set_identity_v1(
            catalog
                .iter()
                .map(|section| section.index())
                .collect::<BTreeSet<_>>()
                .into_iter(),
        );
        let partial_hash = probe_response(200)
            .metadata
            .canonical_set_sha256
            .as_str()
            .to_owned();
        // Persisted suspect run: 3 same-hash suspects at 90s spacing, newest
        // 90s before the first live sample (04:00:00) -- all inside MAX_GAP.
        let history: Vec<OpenGateAttemptSummary> = ["03:58:30", "03:57:00", "03:55:30"]
            .iter()
            .map(|time| OpenGateAttemptSummary {
                classification: OpenAttemptClassification::SuspectPartialSnapshot,
                canonical_set_sha256: Some(partial_hash.clone()),
                completed_at: Some(format!("2026-07-14T{time}Z")),
                catalog_set_identity: Some(identity.as_str().to_owned()),
            })
            .collect();

        let mut persistence = RebuildProbePersistence {
            catalog,
            lkg: (0..300).map(probe_index).collect(),
            history,
            committed: Vec::new(),
        };
        let gates = Arc::new(Mutex::new(TargetGateSet::new()));

        // Pull 1 at 04:00:00: partial 200 of 300, missing 100 >= 30 -> the
        // rebuilt LKG floor (300) holds it; count 3 + 1, span 270s < 300s.
        let mut command = command_for(31, 32, 0);
        command.gate = Some(OpenGateWiring {
            gates: Arc::clone(&gates),
            route: OpenGateRoute::Serving,
        });
        let execution = SharedOpenService::new(&mut persistence)
            .execute_with(
                command,
                &mut FixedClock(VecDeque::from([
                    datetime!(2026-07-14 03:59:59 UTC),
                    datetime!(2026-07-14 04:00:00 UTC),
                ])),
                |_| async { Ok(probe_response(200)) },
                |_| Vec::new(),
            )
            .await
            .expect("rebuild pull 1");
        assert!(
            matches!(execution.terminal, OpenPullTerminal::Unsafe(_)),
            "persisted LKG floor must hold the partial (unseeded would apply it)",
        );
        assert!(gates.lock().expect("gate set").serving_mut().is_quarantined());

        // Pull 2 at 04:01:30 (gap 90s): same set -> count 5, span 360s ->
        // QuarantineConfirm applies. Ignored history would sit at count 2 and
        // keep holding, so this pins that the persisted run was consumed.
        let mut command = command_for(41, 42, 0);
        command.gate = Some(OpenGateWiring {
            gates: Arc::clone(&gates),
            route: OpenGateRoute::Serving,
        });
        let execution = SharedOpenService::new(&mut persistence)
            .execute_with(
                command,
                &mut FixedClock(VecDeque::from([
                    datetime!(2026-07-14 04:01:29 UTC),
                    datetime!(2026-07-14 04:01:30 UTC),
                ])),
                |_| async { Ok(probe_response(200)) },
                |_| Vec::new(),
            )
            .await
            .expect("rebuild pull 2");
        assert!(
            matches!(execution.terminal, OpenPullTerminal::Valid(_)),
            "the persisted run + two live samples must confirm the new reality",
        );
        assert_eq!(
            persistence.committed,
            vec![
                OpenAttemptClassification::SuspectPartialSnapshot,
                OpenAttemptClassification::ValidApplied,
            ],
        );
        assert!(!gates.lock().expect("gate set").serving_mut().is_quarantined());
    }
}
