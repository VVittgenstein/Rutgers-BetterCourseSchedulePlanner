use std::collections::{BTreeMap, BTreeSet};

use bcsp_contracts::{
    CatalogContentVersion, OPEN_CONTRACT_VERSION, OpenAttemptPointV1, OpenBatchKey,
    OpenCanonicalSetHash, OpenCircuitStatusV1, OpenCounterSnapshotV1, OpenFailureClass,
    OpenFailurePointV1, OpenFreshnessState, OpenFreshnessV1, OpenObservationPointV1,
    OpenObservationV1, OpenPullCountsV1, OpenRefreshClassification, OpenRefreshStatusV1,
    OpenSchedulerStatusV1, OpenSectionStatusV1, OpenSequence, OpenState, OpenStateHash,
    OpenUncertaintyReason, RutgersDay, RutgersDayTimezone, SectionKey, TermCampusKey, TraceId,
};
use bcsp_operational_storage::{
    OpenAttemptClassification, OpenAttemptCounters, OpenAttemptRecord, OpenBatchObservation,
    OpenBatchState, OpenCatalogSnapshot, OpenSectionCurrent, OpenSectionState, OperationalStorage,
    StorageError, StorageResult, derive_open_refresh_observation_id,
    derive_open_section_observation_id,
};
use thiserror::Error;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::freshness::{
    LastValidOpenPoint, LatestOpenAttemptPoint, OpenFreshnessError, project_open_freshness,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenCounterAudience {
    Local { run_id: TraceId },
    Public,
}

/// Runtime facts that are intentionally not reconstructed from diagnostic rows.
/// The scheduler and origin circuit remain live policy state; persistence supplies
/// the durable attempt, LKG, Section, and counter facts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenProjectionRuntime {
    pub now: OffsetDateTime,
    pub audience: OpenCounterAudience,
    pub scheduler: OpenSchedulerStatusV1,
    pub circuit: OpenCircuitStatusV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectedOpenStatusV1 {
    pub refresh: OpenRefreshStatusV1,
    pub sections: Vec<OpenSectionStatusV1>,
}

/// Narrow read seam for deterministic status tests. Production uses the same
/// OperationalStorage reads as the HTTP/query adapters.
pub trait OpenStatusReadStore {
    fn serving_open_catalog_snapshot(
        &mut self,
        target: &TermCampusKey,
    ) -> StorageResult<Option<OpenCatalogSnapshot>>;

    fn open_batch_state(&self, target: &TermCampusKey) -> StorageResult<Option<OpenBatchState>>;

    fn open_attempt(&self, attempt_id: &TraceId) -> StorageResult<Option<OpenAttemptRecord>>;

    fn open_batch_observation(
        &self,
        attempt_id: &TraceId,
    ) -> StorageResult<Option<OpenBatchObservation>>;

    fn open_section_current(
        &self,
        target: &TermCampusKey,
    ) -> StorageResult<Vec<OpenSectionCurrent>>;

    fn open_day_counters(
        &self,
        target: &TermCampusKey,
        rutgers_day: &str,
    ) -> StorageResult<OpenAttemptCounters>;

    fn open_service_day_counters(&self, rutgers_day: &str) -> StorageResult<OpenAttemptCounters>;

    fn open_run_counters(
        &self,
        target: &TermCampusKey,
        run_id: &TraceId,
    ) -> StorageResult<OpenAttemptCounters>;
}

impl OpenStatusReadStore for OperationalStorage {
    fn serving_open_catalog_snapshot(
        &mut self,
        target: &TermCampusKey,
    ) -> StorageResult<Option<OpenCatalogSnapshot>> {
        OperationalStorage::serving_open_catalog_snapshot(self, target)
    }

    fn open_batch_state(&self, target: &TermCampusKey) -> StorageResult<Option<OpenBatchState>> {
        OperationalStorage::open_batch_state(self, target)
    }

    fn open_attempt(&self, attempt_id: &TraceId) -> StorageResult<Option<OpenAttemptRecord>> {
        OperationalStorage::open_attempt(self, attempt_id)
    }

    fn open_batch_observation(
        &self,
        attempt_id: &TraceId,
    ) -> StorageResult<Option<OpenBatchObservation>> {
        OperationalStorage::open_batch_observation(self, attempt_id)
    }

    fn open_section_current(
        &self,
        target: &TermCampusKey,
    ) -> StorageResult<Vec<OpenSectionCurrent>> {
        OperationalStorage::open_section_current(self, target)
    }

    fn open_day_counters(
        &self,
        target: &TermCampusKey,
        rutgers_day: &str,
    ) -> StorageResult<OpenAttemptCounters> {
        OperationalStorage::open_day_counters(self, target, rutgers_day)
    }

    fn open_service_day_counters(&self, rutgers_day: &str) -> StorageResult<OpenAttemptCounters> {
        OperationalStorage::open_service_day_counters(self, rutgers_day)
    }

    fn open_run_counters(
        &self,
        target: &TermCampusKey,
        run_id: &TraceId,
    ) -> StorageResult<OpenAttemptCounters> {
        OperationalStorage::open_run_counters(self, target, run_id)
    }
}

#[derive(Debug, Error)]
pub enum OpenProjectionError {
    #[error("Open status persistence read failed")]
    Storage(#[from] StorageError),
    #[error("Open target has no published serving Catalog")]
    TargetNotPublished,
    #[error("Open Section is absent from the published serving Catalog")]
    SectionNotPublished,
    #[error("Open projection is missing a related {entity} row")]
    MissingRelatedRow { entity: &'static str },
    #[error("Open projection contains an invalid {field} timestamp")]
    InvalidTimestamp { field: &'static str },
    #[error("Open projection contains an invalid {field} value")]
    InvalidValue { field: &'static str },
    #[error("Open projection contains inconsistent durable state: {reason}")]
    InconsistentState { reason: &'static str },
    #[error("Open freshness projection failed")]
    Freshness(#[from] OpenFreshnessError),
    #[error("Rutgers-day projection failed")]
    RutgersDay,
    #[error("Open failure code has no stable public mapping")]
    UnknownFailureCode,
}

struct LkgProjection {
    attempt_id: TraceId,
    point: OpenObservationPointV1,
    freshness_point: LastValidOpenPoint,
}

/// Builds one target status and exactly one Section status for every Section in
/// the current serving Catalog. Old removed Section rows are ignored; a current
/// Catalog Section without a reliable Open row is explicitly UNKNOWN.
pub fn project_open_status<S: OpenStatusReadStore>(
    store: &mut S,
    target: &TermCampusKey,
    runtime: &OpenProjectionRuntime,
) -> Result<ProjectedOpenStatusV1, OpenProjectionError> {
    validate_runtime(runtime)?;
    let catalog = store
        .serving_open_catalog_snapshot(target)?
        .ok_or(OpenProjectionError::TargetNotPublished)?;
    if catalog.target != *target {
        return Err(OpenProjectionError::InconsistentState {
            reason: "serving Catalog target differs from the requested Open target",
        });
    }
    let catalog_content_version = content_version(catalog.content_version)?;
    let counter_snapshot = project_counters(store, target, runtime)?;
    let batch = store.open_batch_state(target)?;
    if batch.as_ref().is_some_and(|value| value.target != *target) {
        return Err(OpenProjectionError::InconsistentState {
            reason: "Open batch state belongs to another target",
        });
    }

    let latest_attempt = batch
        .as_ref()
        .and_then(|value| value.last_attempt_id)
        .map(|attempt_id| load_attempt(store, target, attempt_id))
        .transpose()?;
    if let (Some(batch), Some(latest)) = (&batch, &latest_attempt)
        && batch.last_attempt_sequence != latest.attempt_sequence
    {
        return Err(OpenProjectionError::InconsistentState {
            reason: "latest attempt sequence differs from batch state",
        });
    }
    let latest_point = latest_attempt
        .as_ref()
        .map(project_attempt_point)
        .transpose()?;
    let latest_freshness_point = latest_attempt
        .as_ref()
        .map(project_latest_freshness_point)
        .transpose()?
        .flatten();
    let latest_failure = project_latest_failure(batch.as_ref())?;

    let lkg = batch
        .as_ref()
        .and_then(|value| value.lkg_attempt_id)
        .map(|attempt_id| project_lkg(store, target, batch.as_ref(), attempt_id))
        .transpose()?;
    validate_absent_lkg_state(batch.as_ref(), lkg.as_ref())?;
    let freshness = project_open_freshness(
        runtime.now,
        lkg.as_ref().map(|value| value.freshness_point),
        latest_freshness_point,
        None,
    )?;

    let refresh = OpenRefreshStatusV1 {
        contract_version: OPEN_CONTRACT_VERSION,
        batch: OpenBatchKey::from(target.clone()),
        catalog_content_version: Some(catalog_content_version),
        latest_attempt: latest_point,
        latest_failure,
        last_valid_observation: lkg.as_ref().map(|value| value.point.clone()),
        last_body_change_at: parse_optional_timestamp(
            batch
                .as_ref()
                .and_then(|value| value.last_body_change_at.as_deref()),
            "last_body_change_at",
        )?,
        last_state_change_at: parse_optional_timestamp(
            batch
                .as_ref()
                .and_then(|value| value.last_state_change_at.as_deref()),
            "last_state_change_at",
        )?,
        freshness: freshness.clone(),
        scheduler: runtime.scheduler.clone(),
        circuit: runtime.circuit.clone(),
        counter_snapshot: counter_snapshot.clone(),
    };
    let sections = project_sections(
        store,
        catalog,
        catalog_content_version,
        lkg.as_ref(),
        &freshness,
        &counter_snapshot,
        runtime.scheduler.scheduler_lag_milliseconds,
    )?;
    Ok(ProjectedOpenStatusV1 { refresh, sections })
}

/// Reconstructs the current committed per-Section observation for watch START.
///
/// The same shared status projection decides freshness and uncertainty. A
/// retained LKG is intentionally not returned when it is stale, invalidated by
/// a later failed/unsafe/racing attempt, or belongs to an older serving Catalog
/// version. That keeps START from opening an episode on an uncertain value.
pub fn project_current_open_observation<S: OpenStatusReadStore>(
    store: &mut S,
    section: &SectionKey,
    runtime: &OpenProjectionRuntime,
) -> Result<Option<OpenObservationV1>, OpenProjectionError> {
    let target = section.target();
    let projected = project_open_status(store, &target, runtime)?;
    let section_status = projected
        .sections
        .iter()
        .find(|value| value.section_key == *section)
        .ok_or(OpenProjectionError::SectionNotPublished)?;

    if section_status.state == OpenState::Unknown
        || section_status.freshness.state != OpenFreshnessState::Fresh
    {
        return Ok(None);
    }

    let Some(current_catalog_version) = projected.refresh.catalog_content_version else {
        return Err(OpenProjectionError::InconsistentState {
            reason: "published Open projection omitted its Catalog version",
        });
    };
    let Some(section_catalog_version) = section_status.catalog_content_version else {
        return Err(OpenProjectionError::InconsistentState {
            reason: "known Open Section omitted its Catalog version",
        });
    };
    let Some(last_valid) = projected.refresh.last_valid_observation.as_ref() else {
        return Err(OpenProjectionError::InconsistentState {
            reason: "known fresh Open Section exists without an LKG observation",
        });
    };
    if section_catalog_version != current_catalog_version
        || last_valid.catalog_content_version != current_catalog_version
    {
        return Ok(None);
    }

    let batch = store
        .open_batch_state(&target)?
        .ok_or(OpenProjectionError::InconsistentState {
            reason: "known fresh Open Section exists without batch state",
        })?;
    let lkg_attempt_id = batch
        .lkg_attempt_id
        .ok_or(OpenProjectionError::InconsistentState {
            reason: "known fresh Open Section exists without an LKG attempt",
        })?;
    let attempt = load_attempt(store, &target, lkg_attempt_id)?;
    let observation = store.open_batch_observation(&lkg_attempt_id)?.ok_or(
        OpenProjectionError::MissingRelatedRow {
            entity: "LKG batch observation",
        },
    )?;
    let expected_refresh_id = derive_open_refresh_observation_id(lkg_attempt_id)?;
    if observation.observation_id != expected_refresh_id
        || last_valid.observation_id != expected_refresh_id
        || observation.catalog_content_version != current_catalog_version.get()
        || observation.observation_sequence != last_valid.observation_sequence.get()
    {
        return Err(OpenProjectionError::InconsistentState {
            reason: "LKG observation identity differs from its committed projection",
        });
    }

    let observed_at =
        section_status
            .freshness
            .observed_at
            .ok_or(OpenProjectionError::InconsistentState {
                reason: "fresh Open Section omitted its observation timestamp",
            })?;
    let fresh_until =
        section_status
            .freshness
            .fresh_until
            .ok_or(OpenProjectionError::InconsistentState {
                reason: "fresh Open Section omitted its freshness deadline",
            })?;
    if observed_at != last_valid.observed_at {
        return Err(OpenProjectionError::InconsistentState {
            reason: "Open Section timestamp differs from its LKG observation",
        });
    }
    let scheduler_lag_milliseconds =
        attempt
            .schedule_lag_ms
            .ok_or(OpenProjectionError::InvalidValue {
                field: "lkg_schedule_lag_ms",
            })?;

    OpenObservationV1::try_new(
        OPEN_CONTRACT_VERSION,
        derive_open_section_observation_id(lkg_attempt_id, section)?,
        expected_refresh_id,
        OpenBatchKey::from(target),
        section.clone(),
        sequence(attempt.attempt_sequence)?,
        current_catalog_version,
        section_status.state,
        observed_at,
        fresh_until,
        scheduler_lag_milliseconds,
        section_status.counter_snapshot.clone(),
    )
    .map(Some)
    .map_err(|_| OpenProjectionError::InconsistentState {
        reason: "projected Open observation target differs from its Section",
    })
}

fn project_sections<S: OpenStatusReadStore>(
    store: &S,
    catalog: OpenCatalogSnapshot,
    current_catalog_version: CatalogContentVersion,
    lkg: Option<&LkgProjection>,
    target_freshness: &OpenFreshnessV1,
    counters: &OpenCounterSnapshotV1,
    scheduler_lag_milliseconds: u64,
) -> Result<Vec<OpenSectionStatusV1>, OpenProjectionError> {
    let target = catalog.target;
    let mut current = BTreeMap::new();
    for value in store.open_section_current(&target)? {
        if value.section.target() != target {
            return Err(OpenProjectionError::InconsistentState {
                reason: "current Open Section belongs to another target",
            });
        }
        let key = value.section.clone();
        if current.insert(key, value).is_some() {
            return Err(OpenProjectionError::InconsistentState {
                reason: "current Open Section identity is duplicated",
            });
        }
    }

    let mut catalog_keys = BTreeSet::new();
    let mut statuses = Vec::with_capacity(catalog.sections.len());
    for section_key in catalog.sections {
        if section_key.target() != target || !catalog_keys.insert(section_key.clone()) {
            return Err(OpenProjectionError::InconsistentState {
                reason: "serving Catalog Section identities are invalid or duplicated",
            });
        }
        let status = match current.remove(&section_key) {
            Some(value) => {
                let Some(lkg) = lkg else {
                    return Err(OpenProjectionError::InconsistentState {
                        reason: "known Open Section exists without an LKG target observation",
                    });
                };
                if value.attempt_id != lkg.attempt_id {
                    return Err(OpenProjectionError::InconsistentState {
                        reason: "known Open Section does not belong to the target LKG attempt",
                    });
                }
                if value.observation_sequence != lkg.point.observation_sequence.get() {
                    return Err(OpenProjectionError::InconsistentState {
                        reason: "known Open Section does not belong to the target LKG observation",
                    });
                }
                if value.catalog_content_version != lkg.point.catalog_content_version.get()
                    || parse_timestamp(&value.observed_at, "section_observed_at")?
                        != lkg.point.observed_at
                {
                    return Err(OpenProjectionError::InconsistentState {
                        reason: "known Open Section markers differ from its LKG observation",
                    });
                }
                let state = match value.state {
                    OpenSectionState::Open => OpenState::Open,
                    OpenSectionState::Closed => OpenState::Closed,
                };
                OpenSectionStatusV1 {
                    contract_version: OPEN_CONTRACT_VERSION,
                    section_key,
                    state,
                    last_observation_id: Some(lkg.point.observation_id),
                    catalog_content_version: Some(content_version(value.catalog_content_version)?),
                    freshness: target_freshness.clone(),
                    scheduler_lag_milliseconds,
                    counter_snapshot: counters.clone(),
                }
            }
            None => OpenSectionStatusV1 {
                contract_version: OPEN_CONTRACT_VERSION,
                section_key,
                state: OpenState::Unknown,
                last_observation_id: None,
                catalog_content_version: Some(current_catalog_version),
                freshness: section_unknown_freshness(lkg.is_some()),
                scheduler_lag_milliseconds,
                counter_snapshot: counters.clone(),
            },
        };
        statuses.push(status);
    }
    Ok(statuses)
}

fn project_lkg<S: OpenStatusReadStore>(
    store: &S,
    target: &TermCampusKey,
    batch: Option<&OpenBatchState>,
    attempt_id: TraceId,
) -> Result<LkgProjection, OpenProjectionError> {
    let attempt = load_attempt(store, target, attempt_id)?;
    if !attempt.classification.is_success() {
        return Err(OpenProjectionError::InconsistentState {
            reason: "LKG attempt is not a valid applied Open response",
        });
    }
    let completed_at = attempt
        .completed_at
        .as_deref()
        .ok_or(OpenProjectionError::InconsistentState {
            reason: "LKG attempt has no completion timestamp",
        })
        .and_then(|value| parse_timestamp(value, "lkg_completed_at"))?;
    let interval = attempt
        .effective_interval_seconds
        .ok_or(OpenProjectionError::InvalidValue {
            field: "lkg_effective_interval_seconds",
        })?;
    let interval = u32::try_from(interval)
        .ok()
        .filter(|value| *value > 0)
        .ok_or(OpenProjectionError::InvalidValue {
            field: "lkg_effective_interval_seconds",
        })?;
    let observation = store.open_batch_observation(&attempt_id)?.ok_or(
        OpenProjectionError::MissingRelatedRow {
            entity: "LKG batch observation",
        },
    )?;
    if observation.target != *target
        || observation.attempt_id != attempt_id
        || !observation.classification.is_success()
    {
        return Err(OpenProjectionError::InconsistentState {
            reason: "LKG batch observation does not match its attempt and target",
        });
    }
    if let Some(batch) = batch
        && (batch.lkg_observation_sequence != Some(observation.observation_sequence)
            || batch.current_catalog_content_version != observation.catalog_content_version)
    {
        return Err(OpenProjectionError::InconsistentState {
            reason: "LKG batch markers differ from the persisted observation",
        });
    }
    let observed_at = parse_timestamp(&observation.observed_at, "lkg_observed_at")?;
    Ok(LkgProjection {
        attempt_id,
        point: OpenObservationPointV1 {
            observation_id: observation.observation_id,
            observation_sequence: sequence(observation.observation_sequence)?,
            catalog_content_version: content_version(observation.catalog_content_version)?,
            observed_at,
            canonical_set_hash: OpenCanonicalSetHash::try_from(observation.canonical_set_sha256)
                .map_err(|_| OpenProjectionError::InvalidValue {
                    field: "lkg_canonical_set_sha256",
                })?,
            state_hash: OpenStateHash::try_from(observation.state_sha256).map_err(|_| {
                OpenProjectionError::InvalidValue {
                    field: "lkg_state_sha256",
                }
            })?,
        },
        freshness_point: LastValidOpenPoint {
            attempt_sequence: sequence(attempt.attempt_sequence)?,
            observed_at,
            completed_at,
            requested_effective_interval_seconds: interval,
        },
    })
}

fn load_attempt<S: OpenStatusReadStore>(
    store: &S,
    target: &TermCampusKey,
    attempt_id: TraceId,
) -> Result<OpenAttemptRecord, OpenProjectionError> {
    let attempt =
        store
            .open_attempt(&attempt_id)?
            .ok_or(OpenProjectionError::MissingRelatedRow {
                entity: "Open attempt",
            })?;
    if attempt.target != *target || attempt.attempt_id != attempt_id {
        return Err(OpenProjectionError::InconsistentState {
            reason: "Open attempt does not match the requested target and identity",
        });
    }
    Ok(attempt)
}

fn project_attempt_point(
    attempt: &OpenAttemptRecord,
) -> Result<OpenAttemptPointV1, OpenProjectionError> {
    let classification = map_classification(attempt.classification);
    let completed_at = parse_optional_timestamp(attempt.completed_at.as_deref(), "completed_at")?;
    if matches!(attempt.classification, OpenAttemptClassification::Started) {
        if completed_at.is_some() {
            return Err(OpenProjectionError::InconsistentState {
                reason: "in-flight Open attempt already has a completion timestamp",
            });
        }
    } else if completed_at.is_none() {
        return Err(OpenProjectionError::InconsistentState {
            reason: "terminal Open attempt has no completion timestamp",
        });
    }
    Ok(OpenAttemptPointV1 {
        attempt_sequence: sequence(attempt.attempt_sequence)?,
        started_at: parse_timestamp(&attempt.started_at, "started_at")?,
        completed_at,
        classification,
        failure_class: project_failure_class(attempt)?,
    })
}

fn project_latest_freshness_point(
    attempt: &OpenAttemptRecord,
) -> Result<Option<LatestOpenAttemptPoint>, OpenProjectionError> {
    let Some(completed_at) = attempt.completed_at.as_deref() else {
        return Ok(None);
    };
    Ok(Some(LatestOpenAttemptPoint {
        attempt_sequence: sequence(attempt.attempt_sequence)?,
        completed_at: parse_timestamp(completed_at, "latest_completed_at")?,
        classification: map_classification(attempt.classification),
    }))
}

fn project_latest_failure(
    batch: Option<&OpenBatchState>,
) -> Result<Option<OpenFailurePointV1>, OpenProjectionError> {
    let Some(batch) = batch else {
        return Ok(None);
    };
    match (
        batch.last_failure_attempt_sequence,
        batch.last_failure_at.as_deref(),
        batch.last_failure_error_code.as_deref(),
        batch.last_failure_http_status,
    ) {
        (None, None, None, None) => Ok(None),
        (Some(attempt_sequence), Some(failed_at), Some(error_code), http_status) => {
            Ok(Some(OpenFailurePointV1 {
                attempt_sequence: sequence(attempt_sequence)?,
                failed_at: parse_timestamp(failed_at, "last_failure_at")?,
                failure_class: map_failure_code(error_code, http_status)?,
            }))
        }
        _ => Err(OpenProjectionError::InconsistentState {
            reason: "latest Open failure snapshot is partial",
        }),
    }
}

const fn map_classification(value: OpenAttemptClassification) -> OpenRefreshClassification {
    match value {
        OpenAttemptClassification::Started => OpenRefreshClassification::InFlight,
        OpenAttemptClassification::ValidApplied => OpenRefreshClassification::ValidApplied,
        OpenAttemptClassification::ValidEmptyNoRows => OpenRefreshClassification::ValidEmptyNoRows,
        OpenAttemptClassification::Failed | OpenAttemptClassification::Interrupted => {
            OpenRefreshClassification::Failed
        }
        OpenAttemptClassification::UnsafeEmpty => OpenRefreshClassification::UnsafeEmpty,
        OpenAttemptClassification::UnsafeZeroIntersection => {
            OpenRefreshClassification::UnsafeZeroIntersection
        }
        OpenAttemptClassification::SuspectPartialSnapshot => {
            OpenRefreshClassification::SuspectPartialSnapshot
        }
        OpenAttemptClassification::StaleCatalogRace => OpenRefreshClassification::StaleCatalogRace,
    }
}

fn project_failure_class(
    attempt: &OpenAttemptRecord,
) -> Result<Option<OpenFailureClass>, OpenProjectionError> {
    if attempt.classification == OpenAttemptClassification::Interrupted {
        return Ok(Some(OpenFailureClass::Persist));
    }
    if attempt.classification != OpenAttemptClassification::Failed {
        return Ok(None);
    }
    let code = attempt
        .error_code
        .as_deref()
        .ok_or(OpenProjectionError::UnknownFailureCode)?;
    Ok(Some(map_failure_code(code, attempt.http.http_status)?))
}

fn map_failure_code(
    code: &str,
    http_status: Option<u16>,
) -> Result<OpenFailureClass, OpenProjectionError> {
    let value = match code {
        "OPEN_TIMEOUT" | "UPSTREAM_TIMEOUT" => OpenFailureClass::Timeout,
        "OPEN_NETWORK" => OpenFailureClass::Network,
        "OPEN_RATE_LIMITED" => OpenFailureClass::Http429,
        "OPEN_FORBIDDEN" => OpenFailureClass::Forbidden,
        "OPEN_RESPONSE_TOO_LARGE" => OpenFailureClass::Oversize,
        "OPEN_CONTENT_DECODING" => OpenFailureClass::SchemaViolation,
        "OPEN_INVALID_CONTENT_TYPE" | "OPEN_INVALID_JSON" => OpenFailureClass::NonJson,
        "OPEN_ROOT_NOT_ARRAY"
        | "OPEN_NON_STRING_VALUE"
        | "OPEN_RESPONSE_METADATA_MISMATCH"
        | "OPEN_INVALID_REDIRECT" => OpenFailureClass::SchemaViolation,
        "OPEN_INVALID_SECTION_INDEX" | "OPEN_RESPONSE_TARGET_MISMATCH" => {
            OpenFailureClass::InvalidValue
        }
        "OPEN_REQUEST_CONSTRUCTION"
        | "OPEN_CATALOG_UNAVAILABLE_AFTER_FETCH"
        | "PROCESS_RESTARTED" => OpenFailureClass::Persist,
        // Withheld-snapshot codes persisted by `finalize_non_applied_attempt`:
        // these are legitimate durable states and must project (a lingering
        // suspect or unsafe latest-failure would otherwise fail the whole
        // status projection until the row is overwritten).
        "UNSAFE_EMPTY_WITH_CATALOG_ROWS" => OpenFailureClass::UnsafeEmpty,
        "UNSAFE_ZERO_INTERSECTION" => OpenFailureClass::UnsafeZeroIntersection,
        "SUSPECT_PARTIAL_SNAPSHOT" => OpenFailureClass::SuspectPartialSnapshot,
        "OPEN_RESPONSE_URL_MISMATCH" | "OPEN_OFF_ORIGIN_REDIRECT" => {
            OpenFailureClass::OffOriginRedirect
        }
        "OPEN_SAME_ORIGIN_REDIRECT" | "OPEN_FATAL_CLIENT_HTTP" => OpenFailureClass::OtherHttp4xx,
        "OPEN_TRANSIENT_HTTP" => match http_status {
            Some(408) => OpenFailureClass::Http408,
            Some(429) => OpenFailureClass::Http429,
            Some(500..=599) => OpenFailureClass::Http5xx,
            Some(400..=499) => OpenFailureClass::OtherHttp4xx,
            _ => return Err(OpenProjectionError::UnknownFailureCode),
        },
        "OPEN_UNSUPPORTED_HTTP" => match http_status {
            Some(408) => OpenFailureClass::Http408,
            Some(429) => OpenFailureClass::Http429,
            Some(500..=599) => OpenFailureClass::Http5xx,
            Some(400..=499) => OpenFailureClass::OtherHttp4xx,
            _ => OpenFailureClass::SchemaViolation,
        },
        _ => return Err(OpenProjectionError::UnknownFailureCode),
    };
    Ok(value)
}

fn project_counters<S: OpenStatusReadStore>(
    store: &S,
    target: &TermCampusKey,
    runtime: &OpenProjectionRuntime,
) -> Result<OpenCounterSnapshotV1, OpenProjectionError> {
    let day_text =
        crate::service::rutgers_day_at(runtime.now).map_err(|_| OpenProjectionError::RutgersDay)?;
    let rutgers_day =
        RutgersDay::try_from(day_text.as_str()).map_err(|_| OpenProjectionError::RutgersDay)?;
    let (run_counts, today_counts) = match runtime.audience {
        OpenCounterAudience::Local { run_id } => (
            Some(to_pull_counts(store.open_run_counters(target, &run_id)?)),
            to_pull_counts(store.open_day_counters(target, &day_text)?),
        ),
        OpenCounterAudience::Public => (
            None,
            to_pull_counts(store.open_service_day_counters(&day_text)?),
        ),
    };
    Ok(OpenCounterSnapshotV1 {
        run_counts,
        today_counts,
        rutgers_day,
        day_timezone: RutgersDayTimezone::AmericaNewYork,
    })
}

const fn to_pull_counts(value: OpenAttemptCounters) -> OpenPullCountsV1 {
    OpenPullCountsV1 {
        attempted: value.attempted,
        succeeded: value.succeeded,
        failed: value.failed,
        empty: value.empty,
    }
}

fn validate_runtime(runtime: &OpenProjectionRuntime) -> Result<(), OpenProjectionError> {
    if runtime.scheduler.requested_general_interval_seconds == 0
        || runtime.scheduler.requested_effective_interval_seconds == 0
    {
        return Err(OpenProjectionError::InvalidValue {
            field: "scheduler_interval_seconds",
        });
    }
    Ok(())
}

fn validate_absent_lkg_state(
    batch: Option<&OpenBatchState>,
    lkg: Option<&LkgProjection>,
) -> Result<(), OpenProjectionError> {
    let Some(batch) = batch else {
        return Ok(());
    };
    if lkg.is_none()
        && (batch.lkg_observation_sequence.is_some()
            || batch.lkg_observed_at.is_some()
            || batch.lkg_canonical_set_sha256.is_some()
            || batch.lkg_state_sha256.is_some())
    {
        return Err(OpenProjectionError::InconsistentState {
            reason: "LKG markers exist without an LKG attempt identity",
        });
    }
    Ok(())
}

fn section_unknown_freshness(target_has_lkg: bool) -> OpenFreshnessV1 {
    OpenFreshnessV1 {
        state: OpenFreshnessState::Unknown,
        observed_at: None,
        fresh_until: None,
        last_known_good_age_seconds: None,
        uncertainty: Some(if target_has_lkg {
            OpenUncertaintyReason::CatalogVersionUnavailable
        } else {
            OpenUncertaintyReason::NeverObserved
        }),
    }
}

fn parse_timestamp(
    value: &str,
    field: &'static str,
) -> Result<OffsetDateTime, OpenProjectionError> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| OpenProjectionError::InvalidTimestamp { field })
}

fn parse_optional_timestamp(
    value: Option<&str>,
    field: &'static str,
) -> Result<Option<OffsetDateTime>, OpenProjectionError> {
    value.map(|value| parse_timestamp(value, field)).transpose()
}

fn sequence(value: u64) -> Result<OpenSequence, OpenProjectionError> {
    OpenSequence::try_from(value).map_err(|_| OpenProjectionError::InvalidValue {
        field: "open_sequence",
    })
}

fn content_version(value: u64) -> Result<CatalogContentVersion, OpenProjectionError> {
    CatalogContentVersion::try_from(value).map_err(|_| OpenProjectionError::InvalidValue {
        field: "catalog_content_version",
    })
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;
    use bcsp_contracts::{
        OpenCircuitState as ContractCircuitState, OpenSchedulerLane, RutgersDayTimezone, SectionKey,
    };
    use bcsp_operational_storage::{OpenCacheStatus, OpenHttpAuditMetadata, OpenRequestLane};

    #[derive(Clone)]
    struct FakeStore {
        catalog: Option<OpenCatalogSnapshot>,
        batch: Option<OpenBatchState>,
        attempts: BTreeMap<TraceId, OpenAttemptRecord>,
        observations: BTreeMap<TraceId, OpenBatchObservation>,
        current: Vec<OpenSectionCurrent>,
        target_day: OpenAttemptCounters,
        service_day: OpenAttemptCounters,
        run: OpenAttemptCounters,
    }

    impl OpenStatusReadStore for FakeStore {
        fn serving_open_catalog_snapshot(
            &mut self,
            _target: &TermCampusKey,
        ) -> StorageResult<Option<OpenCatalogSnapshot>> {
            Ok(self.catalog.clone())
        }

        fn open_batch_state(
            &self,
            _target: &TermCampusKey,
        ) -> StorageResult<Option<OpenBatchState>> {
            Ok(self.batch.clone())
        }

        fn open_attempt(&self, attempt_id: &TraceId) -> StorageResult<Option<OpenAttemptRecord>> {
            Ok(self.attempts.get(attempt_id).cloned())
        }

        fn open_batch_observation(
            &self,
            attempt_id: &TraceId,
        ) -> StorageResult<Option<OpenBatchObservation>> {
            Ok(self.observations.get(attempt_id).cloned())
        }

        fn open_section_current(
            &self,
            _target: &TermCampusKey,
        ) -> StorageResult<Vec<OpenSectionCurrent>> {
            Ok(self.current.clone())
        }

        fn open_day_counters(
            &self,
            _target: &TermCampusKey,
            _rutgers_day: &str,
        ) -> StorageResult<OpenAttemptCounters> {
            Ok(self.target_day)
        }

        fn open_service_day_counters(
            &self,
            _rutgers_day: &str,
        ) -> StorageResult<OpenAttemptCounters> {
            Ok(self.service_day)
        }

        fn open_run_counters(
            &self,
            _target: &TermCampusKey,
            _run_id: &TraceId,
        ) -> StorageResult<OpenAttemptCounters> {
            Ok(self.run)
        }
    }

    fn trace(ordinal: u64) -> TraceId {
        TraceId::from_str(&format!("00000000-0000-4000-8000-{ordinal:012x}")).unwrap()
    }

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(seconds).unwrap()
    }

    fn text(seconds: i64) -> String {
        at(seconds).format(&Rfc3339).unwrap()
    }

    fn target() -> TermCampusKey {
        TermCampusKey::try_new("T2026F", "CAMPUS_A").unwrap()
    }

    fn section(index: &str) -> SectionKey {
        SectionKey::try_new("T2026F", "CAMPUS_A", index).unwrap()
    }

    fn http(
        status: Option<u16>,
        decoded_bytes: Option<u64>,
        etag: Option<&str>,
    ) -> OpenHttpAuditMetadata {
        OpenHttpAuditMetadata {
            http_status: status,
            cache_status: Some(OpenCacheStatus::NotApplicable),
            decoded_bytes,
            decoded_body_sha256: decoded_bytes.map(|_| "c".repeat(64)),
            content_type: decoded_bytes.map(|_| "application/json".to_owned()),
            etag: etag.map(str::to_owned),
            cache_control: None,
            date: None,
            age_seconds: None,
            last_modified: None,
            retry_after: None,
            retry_after_seconds: None,
        }
    }

    fn lkg_attempt() -> OpenAttemptRecord {
        OpenAttemptRecord {
            attempt_id: trace(1),
            run_id: trace(50),
            target: target(),
            attempt_sequence: 1,
            rutgers_day: "1969-12-31".to_owned(),
            captured_catalog_content_version: 1,
            started_at: text(99),
            completed_at: Some(text(100)),
            classification: OpenAttemptClassification::ValidApplied,
            lane: OpenRequestLane::General,
            http: http(Some(200), Some(42), Some("synthetic-etag")),
            canonical_set_sha256: Some("a".repeat(64)),
            state_sha256: Some("b".repeat(64)),
            source_value_count: 1,
            catalog_section_count: 1,
            intersection_count: 1,
            orphan_count: 0,
            duplicate_count: 0,
            requested_interval_seconds: Some(30),
            effective_interval_seconds: Some(30),
            schedule_lag_ms: Some(12),
            lkg_age_ms: None,
            error_code: None,
            diagnostic_token: None,
        }
    }

    fn failed_attempt() -> OpenAttemptRecord {
        OpenAttemptRecord {
            attempt_id: trace(2),
            run_id: trace(50),
            target: target(),
            attempt_sequence: 2,
            rutgers_day: "1969-12-31".to_owned(),
            captured_catalog_content_version: 2,
            started_at: text(104),
            completed_at: Some(text(105)),
            classification: OpenAttemptClassification::Failed,
            lane: OpenRequestLane::ActiveWatch,
            http: http(None, None, None),
            canonical_set_sha256: None,
            state_sha256: None,
            source_value_count: 0,
            catalog_section_count: 2,
            intersection_count: 0,
            orphan_count: 0,
            duplicate_count: 0,
            requested_interval_seconds: Some(3),
            effective_interval_seconds: Some(3),
            schedule_lag_ms: Some(12),
            lkg_age_ms: Some(5_000),
            error_code: Some("OPEN_NETWORK".to_owned()),
            diagnostic_token: None,
        }
    }

    fn lkg_observation() -> OpenBatchObservation {
        OpenBatchObservation {
            attempt_id: trace(1),
            observation_id: trace(101),
            target: target(),
            observation_sequence: 1,
            catalog_content_version: 1,
            observed_at: text(100),
            classification: OpenAttemptClassification::ValidApplied,
            http: http(Some(200), Some(42), Some("synthetic-etag")),
            canonical_set_sha256: "a".repeat(64),
            state_sha256: "b".repeat(64),
            source_value_count: 1,
            catalog_section_count: 1,
            intersection_count: 1,
            orphan_count: 0,
            duplicate_count: 0,
            changed_section_count: 1,
            body_changed: true,
            state_changed: true,
        }
    }

    fn batch(latest: TraceId, latest_sequence: u64) -> OpenBatchState {
        OpenBatchState {
            target: target(),
            last_attempt_sequence: latest_sequence,
            last_observation_sequence: 1,
            current_catalog_content_version: 1,
            lkg_attempt_id: Some(trace(1)),
            lkg_observation_sequence: Some(1),
            lkg_observed_at: Some(text(100)),
            lkg_canonical_set_sha256: Some("a".repeat(64)),
            lkg_state_sha256: Some("b".repeat(64)),
            last_attempt_id: Some(latest),
            last_attempt_at: Some(if latest_sequence == 1 {
                text(99)
            } else {
                text(104)
            }),
            last_success_at: Some(text(100)),
            last_failure_attempt_sequence: (latest_sequence > 1).then_some(2),
            last_failure_at: (latest_sequence > 1).then(|| text(105)),
            last_failure_error_code: (latest_sequence > 1).then(|| "OPEN_NETWORK".to_owned()),
            last_failure_http_status: None,
            last_body_change_at: Some(text(100)),
            last_state_change_at: Some(text(100)),
            requested_interval_seconds: Some(3),
            effective_interval_seconds: Some(3),
            last_schedule_lag_ms: Some(12),
        }
    }

    #[test]
    fn latest_failure_snapshot_survives_a_later_success() {
        let mut state = batch(trace(3), 3);
        state.last_attempt_at = Some(text(110));
        state.last_success_at = Some(text(111));
        state.last_failure_attempt_sequence = Some(2);
        state.last_failure_at = Some(text(105));
        state.last_failure_error_code = Some("OPEN_NETWORK".to_owned());
        state.last_failure_http_status = None;

        let failure = project_latest_failure(Some(&state))
            .expect("valid durable failure snapshot")
            .expect("latest failure");
        assert_eq!(failure.attempt_sequence.get(), 2);
        assert_eq!(failure.failed_at, at(105));
        assert_eq!(failure.failure_class, OpenFailureClass::Network);
    }

    #[test]
    fn withheld_snapshot_failure_codes_project_instead_of_failing_status() {
        // A persisted gate Hold (and the pre-existing unsafe shapes) land in
        // last_failure_error_code via finalize_non_applied_attempt; the
        // projection must classify them, not error out -- an UnknownFailureCode
        // here takes /open/status and watch START down until the row is
        // overwritten, across later successful pulls.
        for (code, expected) in [
            ("SUSPECT_PARTIAL_SNAPSHOT", OpenFailureClass::SuspectPartialSnapshot),
            ("UNSAFE_EMPTY_WITH_CATALOG_ROWS", OpenFailureClass::UnsafeEmpty),
            ("UNSAFE_ZERO_INTERSECTION", OpenFailureClass::UnsafeZeroIntersection),
        ] {
            let mut state = batch(trace(3), 3);
            state.last_attempt_at = Some(text(110));
            state.last_success_at = Some(text(111));
            state.last_failure_attempt_sequence = Some(2);
            state.last_failure_at = Some(text(105));
            state.last_failure_error_code = Some(code.to_owned());
            state.last_failure_http_status = Some(200);

            let failure = project_latest_failure(Some(&state))
                .expect("withheld-snapshot codes must project")
                .expect("latest failure");
            assert_eq!(failure.failure_class, expected, "{code}");
        }
    }

    fn base_store(with_failure: bool) -> FakeStore {
        let lkg = lkg_attempt();
        let mut attempts = BTreeMap::from([(lkg.attempt_id, lkg)]);
        let (latest, latest_sequence) = if with_failure {
            let failed = failed_attempt();
            attempts.insert(failed.attempt_id, failed);
            (trace(2), 2)
        } else {
            (trace(1), 1)
        };
        FakeStore {
            catalog: Some(OpenCatalogSnapshot {
                target: target(),
                content_version: 2,
                sections: vec![section("00001"), section("00002")],
            }),
            batch: Some(batch(latest, latest_sequence)),
            attempts,
            observations: BTreeMap::from([(trace(1), lkg_observation())]),
            current: vec![OpenSectionCurrent {
                section: section("00001"),
                state: OpenSectionState::Open,
                catalog_content_version: 1,
                attempt_id: trace(1),
                observation_sequence: 1,
                observed_at: text(100),
            }],
            target_day: OpenAttemptCounters {
                attempted: 11,
                succeeded: 7,
                failed: 4,
                empty: 2,
            },
            service_day: OpenAttemptCounters {
                attempted: 99,
                succeeded: 70,
                failed: 29,
                empty: 9,
            },
            run: OpenAttemptCounters {
                attempted: 5,
                succeeded: 4,
                failed: 1,
                empty: 1,
            },
        }
    }

    fn runtime(now: i64, audience: OpenCounterAudience) -> OpenProjectionRuntime {
        OpenProjectionRuntime {
            now: at(now),
            audience,
            scheduler: OpenSchedulerStatusV1 {
                lane: OpenSchedulerLane::General,
                requested_general_interval_seconds: 3,
                requested_effective_interval_seconds: 3,
                active_watch_count: 0,
                next_due_at: Some(at(now + 3)),
                in_flight: false,
                scheduler_lag_milliseconds: 12,
                actual_start_to_start_interval_milliseconds: Some(3_012),
                failure_streak: 0,
            },
            circuit: OpenCircuitStatusV1 {
                state: ContractCircuitState::Closed,
                reason: None,
                opened_at: None,
                retry_at: None,
                diagnostic_recheck_required: false,
            },
        }
    }

    #[test]
    fn later_failure_makes_lkg_stale_without_erasing_known_section_state() {
        let mut store = base_store(true);
        let result = project_open_status(
            &mut store,
            &target(),
            &runtime(110, OpenCounterAudience::Public),
        )
        .unwrap();
        assert_eq!(result.refresh.freshness.state, OpenFreshnessState::Stale);
        assert_eq!(
            result.refresh.freshness.uncertainty,
            Some(OpenUncertaintyReason::LatestAttemptFailed)
        );
        assert_eq!(result.sections[0].state, OpenState::Open);
        assert_eq!(
            result.sections[0].freshness.state,
            OpenFreshnessState::Stale
        );
        assert_eq!(result.sections[0].last_observation_id, Some(trace(101)));
    }

    #[test]
    fn current_catalog_section_without_an_open_row_is_unknown_only_for_that_section() {
        let mut store = base_store(false);
        let result = project_open_status(
            &mut store,
            &target(),
            &runtime(110, OpenCounterAudience::Public),
        )
        .unwrap();
        assert_eq!(result.sections[0].state, OpenState::Open);
        assert_eq!(result.sections[1].state, OpenState::Unknown);
        assert_eq!(
            result.sections[1].freshness.state,
            OpenFreshnessState::Unknown
        );
        assert_eq!(
            result.sections[1].freshness.uncertainty,
            Some(OpenUncertaintyReason::CatalogVersionUnavailable)
        );
        assert_eq!(
            result.sections[1]
                .catalog_content_version
                .expect("current version")
                .get(),
            2
        );
    }

    #[test]
    fn freshness_uses_lkg_attempt_interval_not_the_new_scheduler_interval() {
        let mut store = base_store(false);
        let result = project_open_status(
            &mut store,
            &target(),
            &runtime(174, OpenCounterAudience::Public),
        )
        .unwrap();
        assert_eq!(result.refresh.freshness.state, OpenFreshnessState::Fresh);
        assert_eq!(result.refresh.freshness.fresh_until, Some(at(175)));
        assert_eq!(
            result
                .refresh
                .scheduler
                .requested_effective_interval_seconds,
            3
        );
    }

    fn store_with_reconstructable_current_observation() -> FakeStore {
        let mut store = base_store(false);
        store.catalog.as_mut().unwrap().content_version = 1;
        store.catalog.as_mut().unwrap().sections = vec![section("00001"), section("00002")];
        store
            .observations
            .get_mut(&trace(1))
            .unwrap()
            .observation_id = derive_open_refresh_observation_id(trace(1)).unwrap();
        store
    }

    #[test]
    fn current_watch_observation_reuses_committed_identity_and_public_counters() {
        let mut store = store_with_reconstructable_current_observation();
        let observation = project_current_open_observation(
            &mut store,
            &section("00001"),
            &runtime(110, OpenCounterAudience::Public),
        )
        .unwrap()
        .expect("fresh committed Section observation");

        assert_eq!(
            observation.observation_id,
            derive_open_section_observation_id(trace(1), &section("00001")).unwrap()
        );
        assert_eq!(
            observation.refresh_observation_id,
            derive_open_refresh_observation_id(trace(1)).unwrap()
        );
        assert_eq!(observation.pull_sequence.get(), 1);
        assert_eq!(observation.catalog_content_version.get(), 1);
        assert_eq!(observation.state, OpenState::Open);
        assert_eq!(observation.observed_at, at(100));
        assert_eq!(observation.fresh_until, at(175));
        assert_eq!(observation.scheduler_lag_milliseconds, 12);
        assert_eq!(observation.counter_snapshot.run_counts, None);
        assert_eq!(observation.counter_snapshot.today_counts.attempted, 99);
        assert_eq!(
            observation.counter_snapshot.rutgers_day.as_str(),
            "1969-12-31"
        );
        assert_eq!(
            observation.counter_snapshot.day_timezone,
            RutgersDayTimezone::AmericaNewYork
        );
    }

    #[test]
    fn watch_initial_value_is_absent_for_unknown_stale_or_catalog_mismatched_state() {
        let mut unknown = store_with_reconstructable_current_observation();
        assert_eq!(
            project_current_open_observation(
                &mut unknown,
                &section("00002"),
                &runtime(110, OpenCounterAudience::Public),
            )
            .unwrap(),
            None
        );

        for classification in [
            OpenAttemptClassification::Failed,
            OpenAttemptClassification::UnsafeEmpty,
            OpenAttemptClassification::UnsafeZeroIntersection,
            OpenAttemptClassification::StaleCatalogRace,
        ] {
            let mut stale = base_store(true);
            stale.catalog.as_mut().unwrap().content_version = 1;
            stale.attempts.get_mut(&trace(2)).unwrap().classification = classification;
            stale
                .observations
                .get_mut(&trace(1))
                .unwrap()
                .observation_id = derive_open_refresh_observation_id(trace(1)).unwrap();
            assert_eq!(
                project_current_open_observation(
                    &mut stale,
                    &section("00001"),
                    &runtime(110, OpenCounterAudience::Public),
                )
                .unwrap(),
                None,
                "{classification:?} must not open an initial episode"
            );
        }

        let mut catalog_advanced = base_store(false);
        catalog_advanced
            .observations
            .get_mut(&trace(1))
            .unwrap()
            .observation_id = derive_open_refresh_observation_id(trace(1)).unwrap();
        assert_eq!(
            project_current_open_observation(
                &mut catalog_advanced,
                &section("00001"),
                &runtime(110, OpenCounterAudience::Public),
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn unpublished_section_is_distinct_from_a_published_section_without_current_open() {
        let mut store = store_with_reconstructable_current_observation();
        let error = project_current_open_observation(
            &mut store,
            &section("99999"),
            &runtime(110, OpenCounterAudience::Public),
        )
        .unwrap_err();
        assert!(matches!(error, OpenProjectionError::SectionNotPublished));
    }

    #[test]
    fn local_and_public_counter_scopes_are_not_conflated() {
        let mut local_store = base_store(false);
        let local = project_open_status(
            &mut local_store,
            &target(),
            &runtime(110, OpenCounterAudience::Local { run_id: trace(50) }),
        )
        .unwrap();
        assert_eq!(
            local.refresh.counter_snapshot.run_counts.unwrap().attempted,
            5
        );
        assert_eq!(local.refresh.counter_snapshot.today_counts.attempted, 11);

        let mut public_store = base_store(false);
        let public = project_open_status(
            &mut public_store,
            &target(),
            &runtime(110, OpenCounterAudience::Public),
        )
        .unwrap();
        assert_eq!(public.refresh.counter_snapshot.run_counts, None);
        assert_eq!(public.refresh.counter_snapshot.today_counts.attempted, 99);
        assert_eq!(
            public.refresh.counter_snapshot.day_timezone,
            RutgersDayTimezone::AmericaNewYork
        );
        assert_eq!(
            public.refresh.counter_snapshot.rutgers_day.as_str(),
            "1969-12-31"
        );
    }

    #[test]
    fn never_observed_target_and_sections_have_no_invented_known_state() {
        let mut store = base_store(false);
        store.batch = None;
        store.attempts.clear();
        store.observations.clear();
        store.current.clear();
        let result = project_open_status(
            &mut store,
            &target(),
            &runtime(110, OpenCounterAudience::Public),
        )
        .unwrap();
        assert_eq!(result.refresh.freshness.state, OpenFreshnessState::Unknown);
        assert_eq!(
            result.refresh.freshness.uncertainty,
            Some(OpenUncertaintyReason::NeverObserved)
        );
        assert!(result.sections.iter().all(|value| {
            value.state == OpenState::Unknown
                && value.freshness.uncertainty == Some(OpenUncertaintyReason::NeverObserved)
        }));
    }

    #[test]
    fn interrupted_attempt_is_a_failed_public_point_and_invalidates_lkg() {
        let mut store = base_store(true);
        let interrupted = store.attempts.get_mut(&trace(2)).unwrap();
        interrupted.classification = OpenAttemptClassification::Interrupted;
        interrupted.error_code = Some("PROCESS_RESTARTED".to_owned());
        let result = project_open_status(
            &mut store,
            &target(),
            &runtime(110, OpenCounterAudience::Public),
        )
        .unwrap();
        let latest = result.refresh.latest_attempt.unwrap();
        assert_eq!(latest.classification, OpenRefreshClassification::Failed);
        assert_eq!(latest.failure_class, Some(OpenFailureClass::Persist));
        assert_eq!(result.refresh.freshness.state, OpenFreshnessState::Stale);
    }

    #[test]
    fn persisted_failure_codes_project_to_stable_product_classes() {
        for (code, status, expected) in [
            (
                "OPEN_RESPONSE_METADATA_MISMATCH",
                None,
                OpenFailureClass::SchemaViolation,
            ),
            (
                "OPEN_CATALOG_UNAVAILABLE_AFTER_FETCH",
                None,
                OpenFailureClass::Persist,
            ),
            ("OPEN_REQUEST_CONSTRUCTION", None, OpenFailureClass::Persist),
            (
                "OPEN_OFF_ORIGIN_REDIRECT",
                Some(302),
                OpenFailureClass::OffOriginRedirect,
            ),
            (
                "OPEN_SAME_ORIGIN_REDIRECT",
                Some(302),
                OpenFailureClass::OtherHttp4xx,
            ),
            (
                "OPEN_INVALID_REDIRECT",
                Some(302),
                OpenFailureClass::SchemaViolation,
            ),
            (
                "OPEN_UNSUPPORTED_HTTP",
                Some(302),
                OpenFailureClass::SchemaViolation,
            ),
        ] {
            let mut attempt = failed_attempt();
            attempt.error_code = Some(code.to_owned());
            attempt.http.http_status = status;
            assert_eq!(project_failure_class(&attempt).unwrap(), Some(expected));
        }
    }

    #[test]
    fn fake_clock_boundary_remains_inclusive() {
        let mut store = base_store(false);
        let at_boundary = project_open_status(
            &mut store,
            &target(),
            &runtime(175, OpenCounterAudience::Public),
        )
        .unwrap();
        assert_eq!(
            at_boundary.refresh.freshness.state,
            OpenFreshnessState::Fresh
        );

        let mut store = base_store(false);
        let after = project_open_status(
            &mut store,
            &target(),
            &runtime(176, OpenCounterAudience::Public),
        )
        .unwrap();
        assert_eq!(after.refresh.freshness.state, OpenFreshnessState::Stale);
        assert_eq!(
            after.refresh.freshness.uncertainty,
            Some(OpenUncertaintyReason::StaleLastKnownGood)
        );
        assert_eq!(
            after.refresh.freshness.last_known_good_age_seconds,
            Some(u64::try_from((at(176) - at(100)).whole_seconds()).unwrap())
        );
    }
}
