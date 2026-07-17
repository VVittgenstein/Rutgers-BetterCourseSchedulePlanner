use std::collections::BTreeSet;

use bcsp_contracts::{CampusCode, CatalogSubjectCode, TermCampusKey, TermId, TraceId};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};

use crate::migration::sha256_hex;
use crate::storage::{
    i64_to_u64, parse_optional_stored_trace_id, stored_sha256, u64_to_i64,
    validate_diagnostic_token, validate_identifier, validate_optional_text, validate_safe_code,
    validate_sha256, validate_stored_timestamp, validate_timestamp, validate_timestamp_order,
};
use crate::{
    BeginDiscoveryAttemptCommand, DiscoveryAvailability, DiscoveryCounts, DiscoveryObservation,
    DiscoveryPublishOutcome, DiscoveryRefreshCommand, DiscoverySnapshot, DiscoverySourceKind,
    DiscoverySourceVersion, DiscoveryState, DiscoveryStatus, FinishDiscoveryFailureCommand,
    OperationalStorage, PublishedDiscoverySnapshot, RefreshFailureStage, StorageError,
    StorageResult,
};

impl OperationalStorage {
    pub fn begin_discovery_attempt(
        &mut self,
        command: &BeginDiscoveryAttemptCommand,
    ) -> StorageResult<()> {
        validate_timestamp("started_at", &command.started_at)?;
        let observation_id = command.observation_id.to_string();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = transaction
            .query_row(
                "SELECT started_at, status FROM catalog_discovery_observations
                 WHERE observation_id = ?1",
                [&observation_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        match existing {
            None => {
                transaction.execute(
                    "INSERT INTO catalog_discovery_observations
                        (observation_id, started_at, status)
                     VALUES (?1, ?2, 'STARTED')",
                    params![observation_id, command.started_at],
                )?;
            }
            Some((started_at, status)) if status == "STARTED" => {
                if started_at != command.started_at {
                    return Err(StorageError::InvalidCommand {
                        field: "started_at",
                        reason: "must match the timestamp already stored for this observation",
                    });
                }
            }
            Some(_) => {
                return Err(StorageError::ObservationNotStarted(command.observation_id));
            }
        }
        transaction.execute(
            "UPDATE catalog_discovery_state
             SET last_attempt_observation_id = ?1 WHERE singleton = 1",
            [&observation_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn finish_discovery_failure(
        &mut self,
        command: &FinishDiscoveryFailureCommand,
    ) -> StorageResult<()> {
        validate_timestamp("completed_at", &command.completed_at)?;
        validate_safe_code("error_code", &command.error_code)?;
        validate_diagnostic_token(command.diagnostic_token.as_deref())?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let observation_id = command.observation_id.to_string();
        let (started_at, status) = transaction
            .query_row(
                "SELECT started_at, status FROM catalog_discovery_observations
                 WHERE observation_id = ?1",
                [&observation_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?
            .ok_or(StorageError::ObservationNotFound(command.observation_id))?;
        if status != "STARTED" {
            return Err(StorageError::ObservationNotStarted(command.observation_id));
        }
        validate_timestamp_order(&started_at, &command.completed_at)?;
        let updated = transaction.execute(
            "UPDATE catalog_discovery_observations
             SET status = 'FAILED', completed_at = ?2, error_stage = ?3,
                 error_code = ?4, diagnostic_token = ?5
             WHERE observation_id = ?1 AND status = 'STARTED'",
            params![
                observation_id,
                command.completed_at,
                command.stage.as_str(),
                command.error_code,
                command.diagnostic_token,
            ],
        )?;
        if updated != 1 {
            return Err(StorageError::ObservationNotStarted(command.observation_id));
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn apply_discovery_refresh(
        &mut self,
        command: DiscoveryRefreshCommand,
    ) -> StorageResult<DiscoveryPublishOutcome> {
        self.begin_discovery_attempt(&BeginDiscoveryAttemptCommand {
            observation_id: command.observation_id,
            started_at: command.started_at.clone(),
        })?;
        let observation_id = command.observation_id;
        let completed_at = command.completed_at.clone();
        let prepared = match prepare_discovery(command) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.record_discovery_failure_best_effort(
                    &observation_id,
                    &completed_at,
                    RefreshFailureStage::Normalization,
                    "DISCOVERY_PROJECTION_INVALID",
                );
                return Err(error);
            }
        };
        let result = self.apply_prepared_discovery(prepared);
        if result.is_err() {
            self.record_discovery_failure_best_effort(
                &observation_id,
                &completed_at,
                RefreshFailureStage::Publish,
                "DISCOVERY_PUBLISH_FAILED",
            );
        }
        result
    }

    pub fn discovery_state(&self) -> StorageResult<DiscoveryState> {
        load_discovery_state(&self.connection)
    }

    pub fn discovery_observation(
        &self,
        observation_id: &TraceId,
    ) -> StorageResult<Option<DiscoveryObservation>> {
        load_discovery_observation(&self.connection, observation_id)
    }

    pub fn published_discovery_snapshot(&mut self) -> StorageResult<PublishedDiscoverySnapshot> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)?;
        let state = load_discovery_state(&transaction)?;
        let snapshot = load_published_discovery_snapshot(&transaction, &state)?;
        let sources = snapshot.sources.clone();
        let published = PublishedDiscoverySnapshot {
            state,
            sources,
            snapshot,
        };
        transaction.commit()?;
        Ok(published)
    }

    pub fn discovered_targets(&self) -> StorageResult<Vec<TermCampusKey>> {
        let mut statement = self.connection.prepare(
            "SELECT term_id, campus_code FROM catalog_campuses
             ORDER BY term_id, campus_code",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.map(|row| {
            let (term, campus) = row?;
            TermCampusKey::try_new(&term, &campus).map_err(StorageError::from)
        })
        .collect()
    }

    fn apply_prepared_discovery(
        &mut self,
        prepared: PreparedDiscovery,
    ) -> StorageResult<DiscoveryPublishOutcome> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let outcome = apply_discovery_transaction(&transaction, &prepared)?;
        transaction.commit()?;
        Ok(outcome)
    }

    fn record_discovery_failure_best_effort(
        &mut self,
        observation_id: &TraceId,
        completed_at: &str,
        stage: RefreshFailureStage,
        error_code: &str,
    ) {
        let _ = self.finish_discovery_failure(&FinishDiscoveryFailureCommand {
            observation_id: *observation_id,
            completed_at: completed_at.to_owned(),
            stage,
            error_code: error_code.to_owned(),
            diagnostic_token: None,
        });
    }
}

fn load_discovery_observation(
    connection: &Connection,
    observation_id: &TraceId,
) -> StorageResult<Option<DiscoveryObservation>> {
    let observation_id_text = observation_id.to_string();
    let row = connection
        .query_row(
            "SELECT started_at, completed_at, status, semantic_sha256,
                    term_count, campus_count, subject_count,
                    changed, resulting_content_version,
                    error_stage, error_code, diagnostic_token
             FROM catalog_discovery_observations WHERE observation_id = ?1",
            [&observation_id_text],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<bool>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, Option<String>>(11)?,
                ))
            },
        )
        .optional()?;
    let Some(row) = row else {
        return Ok(None);
    };
    let started =
        validate_stored_timestamp("catalog_discovery_observations", "started_at", &row.0)?;
    if let Some(completed_at) = row.1.as_deref() {
        let completed = validate_stored_timestamp(
            "catalog_discovery_observations",
            "completed_at",
            completed_at,
        )?;
        if completed < started {
            return Err(StorageError::InvalidStoredProjection {
                table: "catalog_discovery_observations",
                field: "completed_at",
                reason: "must not precede started_at",
            });
        }
    }
    let status = DiscoveryStatus::parse(&row.2).ok_or(StorageError::UnknownRefreshStatus)?;
    if (status == DiscoveryStatus::Started) != row.1.is_none() {
        return Err(StorageError::InvalidStoredProjection {
            table: "catalog_discovery_observations",
            field: "completed_at",
            reason: "must be absent only while an observation is STARTED",
        });
    }
    if let Some(value) = row.3.as_deref() {
        stored_sha256("catalog_discovery_observations", "semantic_sha256", value)?;
    }
    let mut source_statement = connection.prepare(
        "SELECT v.content_sha256
         FROM catalog_discovery_observation_sources s
         JOIN catalog_discovery_source_versions v
           ON v.source_version_id = s.source_version_id
         WHERE s.observation_id = ?1 ORDER BY v.content_sha256, v.source_version_id",
    )?;
    let source_content_sha256 = source_statement
        .query_map([&observation_id_text], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for digest in &source_content_sha256 {
        stored_sha256(
            "catalog_discovery_source_versions",
            "content_sha256",
            digest,
        )?;
    }
    Ok(Some(DiscoveryObservation {
        observation_id: *observation_id,
        started_at: row.0,
        completed_at: row.1,
        status,
        semantic_sha256: row.3,
        source_content_sha256,
        counts: DiscoveryCounts {
            terms: i64_to_u64(row.4)?,
            campuses: i64_to_u64(row.5)?,
            subjects: i64_to_u64(row.6)?,
        },
        changed: row.7,
        resulting_content_version: row.8.map(i64_to_u64).transpose()?,
        error_stage: row.9,
        error_code: row.10,
        diagnostic_token: row.11,
    }))
}

fn load_discovery_state(connection: &Connection) -> StorageResult<DiscoveryState> {
    let row = connection.query_row(
        "SELECT content_version, semantic_sha256, last_attempt_observation_id,
                last_success_observation_id, last_published_observation_id,
                last_nonempty_observation_id,
                (SELECT count(*) FROM catalog_terms),
                (SELECT count(*) FROM catalog_campuses),
                (SELECT count(*) FROM catalog_subjects)
         FROM catalog_discovery_state WHERE singleton = 1",
        [],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
            ))
        },
    )?;
    let last_attempt_observation_id = parse_optional_stored_trace_id(
        "catalog_discovery_state",
        "last_attempt_observation_id",
        row.2,
    )?;
    let last_success_observation_id = parse_optional_stored_trace_id(
        "catalog_discovery_state",
        "last_success_observation_id",
        row.3,
    )?;
    let last_published_observation_id = parse_optional_stored_trace_id(
        "catalog_discovery_state",
        "last_published_observation_id",
        row.4,
    )?;
    let last_nonempty_observation_id = parse_optional_stored_trace_id(
        "catalog_discovery_state",
        "last_nonempty_observation_id",
        row.5,
    )?;
    let load = |pointer: Option<&TraceId>| -> StorageResult<Option<DiscoveryObservation>> {
        match pointer {
            Some(pointer) => load_discovery_observation(connection, pointer)?
                .ok_or(StorageError::MissingPublishedObservation {
                    scope: "discovery-state pointer",
                })
                .map(Some),
            None => Ok(None),
        }
    };
    let last_attempt = load(last_attempt_observation_id.as_ref())?;
    let last_success = load(last_success_observation_id.as_ref())?;
    let last_published = load(last_published_observation_id.as_ref())?;
    let last_nonempty = load(last_nonempty_observation_id.as_ref())?;
    let content_version = i64_to_u64(row.0)?;
    if let Some(value) = row.1.as_deref() {
        stored_sha256("catalog_discovery_state", "semantic_sha256", value)?;
    }
    if content_version > 0 && (last_success.is_none() || last_published.is_none()) {
        return Err(StorageError::MissingPublishedObservation {
            scope: "discovery publication",
        });
    }
    if content_version > 0 && last_success_observation_id != last_published_observation_id {
        return Err(StorageError::InvalidStoredProjection {
            table: "catalog_discovery_state",
            field: "last_published_observation_id",
            reason: "must identify the latest successful dictionary observation",
        });
    }
    if last_success.as_ref().is_some_and(|observation| {
        !matches!(
            observation.status,
            DiscoveryStatus::AppliedChanged | DiscoveryStatus::AppliedUnchanged
        ) || observation.completed_at.is_none()
            || observation.resulting_content_version != Some(content_version)
            || observation.semantic_sha256 != row.1
    }) {
        return Err(StorageError::InvalidStoredProjection {
            table: "catalog_discovery_state",
            field: "last_success_observation_id",
            reason: "does not point to a completed success for the current dictionary",
        });
    }
    let state_counts = DiscoveryCounts {
        terms: i64_to_u64(row.6)?,
        campuses: i64_to_u64(row.7)?,
        subjects: i64_to_u64(row.8)?,
    };
    if last_success
        .as_ref()
        .is_some_and(|observation| observation.counts != state_counts)
    {
        return Err(StorageError::InvalidStoredProjection {
            table: "catalog_discovery_state",
            field: "entity counts",
            reason: "do not match the latest successful observation",
        });
    }
    if last_nonempty.as_ref().is_some_and(|observation| {
        !matches!(
            observation.status,
            DiscoveryStatus::AppliedChanged | DiscoveryStatus::AppliedUnchanged
        ) || observation.completed_at.is_none()
            || observation.counts.campuses == 0
    }) {
        return Err(StorageError::InvalidStoredProjection {
            table: "catalog_discovery_state",
            field: "last_nonempty_observation_id",
            reason: "does not point to a completed nonempty discovery success",
        });
    }
    let is_stale = last_success_observation_id.is_some()
        && last_attempt_observation_id != last_success_observation_id;
    let availability = if last_success_observation_id.is_none() {
        DiscoveryAvailability::UnavailableNoFirstSuccess
    } else if is_stale {
        DiscoveryAvailability::Stale
    } else {
        DiscoveryAvailability::Fresh
    };
    Ok(DiscoveryState {
        content_version,
        semantic_sha256: row.1,
        last_attempt_observation_id,
        last_success_observation_id,
        last_published_observation_id,
        last_nonempty_observation_id,
        is_stale,
        availability,
        term_count: state_counts.terms,
        campus_count: state_counts.campuses,
        subject_count: state_counts.subjects,
        last_attempt,
        last_success,
        last_published,
        last_nonempty,
    })
}

fn load_published_discovery_snapshot(
    connection: &Connection,
    state: &DiscoveryState,
) -> StorageResult<DiscoverySnapshot> {
    if state.content_version == 0 {
        if state.term_count != 0 || state.campus_count != 0 || state.subject_count != 0 {
            return Err(StorageError::InconsistentPublishedVersion {
                entity_kind: "discovery dictionary",
            });
        }
        return Ok(DiscoverySnapshot::default());
    }
    let observation_id =
        state
            .last_success_observation_id
            .ok_or(StorageError::MissingPublishedObservation {
                scope: "discovery success",
            })?;
    let observation_id_text = observation_id.to_string();
    let mut source_statement = connection.prepare(
        "SELECT v.source_version_id, v.source_kind, v.source_identity,
                v.content_sha256, v.canonical_facts_json, v.observed_at
         FROM catalog_discovery_observation_sources os
         JOIN catalog_discovery_source_versions v
           ON v.source_version_id = os.source_version_id
         WHERE os.observation_id = ?1
         ORDER BY v.source_version_id, v.source_kind, v.source_identity",
    )?;
    let sources = source_statement
        .query_map([&observation_id_text], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?
        .map(|row| {
            let row = row?;
            if validate_identifier("source_version_id", &row.0, 128).is_err()
                || validate_identifier("source_identity", &row.2, 256).is_err()
                || validate_sha256("source.content_sha256", &row.3).is_err()
            {
                return Err(StorageError::InvalidStoredProjection {
                    table: "catalog_discovery_source_versions",
                    field: "source identity or digest",
                    reason: "violates the persisted source contract",
                });
            }
            validate_stored_timestamp("catalog_discovery_source_versions", "observed_at", &row.5)?;
            let source_kind = match row.1.as_str() {
                "SELECTOR" => DiscoverySourceKind::Selector,
                "BOOTSTRAP" => DiscoverySourceKind::Bootstrap,
                _ => {
                    return Err(StorageError::InvalidStoredProjection {
                        table: "catalog_discovery_source_versions",
                        field: "source_kind",
                        reason: "is not a supported source kind",
                    });
                }
            };
            let (expected_identity, expected_version_id) = match source_kind {
                DiscoverySourceKind::Selector => {
                    ("RUTGERS_SELECTOR", format!("selector:{}", row.3))
                }
                DiscoverySourceKind::Bootstrap => {
                    ("RBCSP_BOOTSTRAP", format!("bootstrap:{}", row.3))
                }
            };
            if row.2 != expected_identity || row.0 != expected_version_id {
                return Err(StorageError::InvalidStoredProjection {
                    table: "catalog_discovery_source_versions",
                    field: "source identity",
                    reason: "kind, stable identity, digest, and version ID do not agree",
                });
            }
            Ok(DiscoverySourceVersion {
                source_version_id: row.0,
                source_kind,
                source_identity: row.2,
                content_sha256: row.3,
                canonical_facts: serde_json::from_str(&row.4)?,
                observed_at: row.5,
            })
        })
        .collect::<StorageResult<Vec<_>>>()?;
    if sources.is_empty() {
        return Err(StorageError::InvalidStoredSourceOwnership {
            entity_kind: "discovery sources",
        });
    }
    let source_ids = sources
        .iter()
        .map(|source| source.source_version_id.as_str())
        .collect::<BTreeSet<_>>();
    let version = u64_to_i64(state.content_version)?;

    let mut term_statement = connection.prepare(
        "SELECT term_id, year, term_code, display_name, published,
                canonical_facts_json, source_version_id, content_version
         FROM catalog_terms ORDER BY term_id",
    )?;
    let terms = term_statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<bool>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })?
        .map(|row| {
            let row = row?;
            if row.7 != version {
                return Err(StorageError::InconsistentPublishedVersion {
                    entity_kind: "discovery term",
                });
            }
            if !source_ids.contains(row.6.as_str()) {
                return Err(StorageError::InvalidStoredSourceOwnership {
                    entity_kind: "discovery term",
                });
            }
            Ok(crate::DiscoveredTerm {
                term_id: TermId::try_from(row.0.as_str())?,
                year: row.1.map(u16::try_from).transpose().map_err(|_| {
                    StorageError::InvalidStoredProjection {
                        table: "catalog_terms",
                        field: "year",
                        reason: "is outside the supported range",
                    }
                })?,
                term_code: row.2,
                display_name: row.3,
                published: row.4,
                canonical_facts: serde_json::from_str(&row.5)?,
                source_version_id: row.6,
            })
        })
        .collect::<StorageResult<Vec<_>>>()?;

    let mut campus_statement = connection.prepare(
        "SELECT term_id, campus_code, display_name, category, enabled,
                canonical_facts_json, source_version_id, content_version
         FROM catalog_campuses ORDER BY term_id, campus_code",
    )?;
    let campuses = campus_statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<bool>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })?
        .map(|row| {
            let row = row?;
            if row.7 != version {
                return Err(StorageError::InconsistentPublishedVersion {
                    entity_kind: "discovery campus",
                });
            }
            if !source_ids.contains(row.6.as_str()) {
                return Err(StorageError::InvalidStoredSourceOwnership {
                    entity_kind: "discovery campus",
                });
            }
            Ok(crate::DiscoveredCampus {
                target: TermCampusKey::try_new(&row.0, &row.1)?,
                display_name: row.2,
                category: row.3,
                enabled: row.4,
                canonical_facts: serde_json::from_str(&row.5)?,
                source_version_id: row.6,
            })
        })
        .collect::<StorageResult<Vec<_>>>()?;

    let mut subject_statement = connection.prepare(
        "SELECT term_id, campus_code, subject_code, display_name, enabled,
                canonical_facts_json, source_version_id, content_version
         FROM catalog_subjects ORDER BY term_id, campus_code, subject_code",
    )?;
    let subjects = subject_statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<bool>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
            ))
        })?
        .map(|row| {
            let row = row?;
            if row.7 != version {
                return Err(StorageError::InconsistentPublishedVersion {
                    entity_kind: "discovery subject",
                });
            }
            if !source_ids.contains(row.6.as_str()) {
                return Err(StorageError::InvalidStoredSourceOwnership {
                    entity_kind: "discovery subject",
                });
            }
            CatalogSubjectCode::try_from(row.2.as_str()).map_err(|_| {
                StorageError::InvalidStoredProjection {
                    table: "catalog_subjects",
                    field: "subject_code",
                    reason: "violates the shared subject contract",
                }
            })?;
            Ok(crate::DiscoveredSubject {
                term_id: TermId::try_from(row.0.as_str())?,
                campus_code: CampusCode::try_from(row.1.as_str())?,
                subject_code: row.2,
                display_name: row.3,
                enabled: row.4,
                canonical_facts: serde_json::from_str(&row.5)?,
                source_version_id: row.6,
            })
        })
        .collect::<StorageResult<Vec<_>>>()?;
    let term_ids = terms
        .iter()
        .map(|term| term.term_id.clone())
        .collect::<BTreeSet<_>>();
    let campus_targets = campuses
        .iter()
        .map(|campus| campus.target.clone())
        .collect::<BTreeSet<_>>();
    for campus in &campuses {
        if !term_ids.contains(campus.target.term()) {
            return Err(StorageError::OrphanedStoredEntity {
                entity_kind: "discovery campus",
            });
        }
    }
    for subject in &subjects {
        let target = TermCampusKey::new(subject.term_id.clone(), subject.campus_code.clone());
        if !campus_targets.contains(&target) {
            return Err(StorageError::OrphanedStoredEntity {
                entity_kind: "discovery subject",
            });
        }
    }
    if terms.len() as u64 != state.term_count
        || campuses.len() as u64 != state.campus_count
        || subjects.len() as u64 != state.subject_count
    {
        return Err(StorageError::InvalidStoredProjection {
            table: "catalog_discovery_state",
            field: "entity counts",
            reason: "do not match published discovery rows",
        });
    }
    Ok(DiscoverySnapshot {
        sources,
        terms,
        campuses,
        subjects,
    })
}

struct PreparedDiscovery {
    command: DiscoveryRefreshCommand,
    semantic_sha256: String,
}

fn prepare_discovery(mut command: DiscoveryRefreshCommand) -> StorageResult<PreparedDiscovery> {
    validate_timestamp("started_at", &command.started_at)?;
    validate_timestamp("completed_at", &command.completed_at)?;
    validate_timestamp_order(&command.started_at, &command.completed_at)?;
    canonicalize_and_validate_discovery(&mut command.snapshot)?;

    let semantic_sha256 = discovery_content_sha256_v1(&command.snapshot)?;
    Ok(PreparedDiscovery {
        command,
        semantic_sha256,
    })
}

pub fn discovery_content_sha256_v1(snapshot: &DiscoverySnapshot) -> StorageResult<String> {
    let mut term_rows = snapshot.terms.iter().collect::<Vec<_>>();
    term_rows.sort_by(|left, right| left.term_id.cmp(&right.term_id));
    let terms = term_rows
        .into_iter()
        .map(|term| {
            (
                &term.term_id,
                term.year,
                &term.term_code,
                &term.display_name,
                term.published,
                &term.canonical_facts,
            )
        })
        .collect::<Vec<_>>();
    let mut campus_rows = snapshot.campuses.iter().collect::<Vec<_>>();
    campus_rows.sort_by(|left, right| left.target.cmp(&right.target));
    let campuses = campus_rows
        .into_iter()
        .map(|campus| {
            (
                &campus.target,
                &campus.display_name,
                &campus.category,
                campus.enabled,
                &campus.canonical_facts,
            )
        })
        .collect::<Vec<_>>();
    let mut subject_rows = snapshot.subjects.iter().collect::<Vec<_>>();
    subject_rows.sort_by(|left, right| {
        (&left.term_id, &left.campus_code, &left.subject_code).cmp(&(
            &right.term_id,
            &right.campus_code,
            &right.subject_code,
        ))
    });
    let subjects = subject_rows
        .into_iter()
        .map(|subject| {
            (
                &subject.term_id,
                &subject.campus_code,
                &subject.subject_code,
                &subject.display_name,
                subject.enabled,
                &subject.canonical_facts,
            )
        })
        .collect::<Vec<_>>();
    let value = serde_json::json!({
        "schema": "discovery-content-v1",
        "terms": terms,
        "campuses": campuses,
        "subjects": subjects,
    });
    Ok(sha256_hex(&serde_json::to_vec(&value)?))
}

fn canonicalize_and_validate_discovery(snapshot: &mut DiscoverySnapshot) -> StorageResult<()> {
    snapshot.sources.sort_by(|left, right| {
        (
            &left.source_version_id,
            left.source_kind,
            &left.source_identity,
        )
            .cmp(&(
                &right.source_version_id,
                right.source_kind,
                &right.source_identity,
            ))
    });
    snapshot
        .terms
        .sort_by(|left, right| left.term_id.cmp(&right.term_id));
    snapshot
        .campuses
        .sort_by(|left, right| left.target.cmp(&right.target));
    snapshot.subjects.sort_by(|left, right| {
        (&left.term_id, &left.campus_code, &left.subject_code).cmp(&(
            &right.term_id,
            &right.campus_code,
            &right.subject_code,
        ))
    });
    if snapshot.sources.is_empty() {
        return Err(StorageError::InvalidCommand {
            field: "discovery.sources",
            reason: "an accepted discovery snapshot requires source provenance",
        });
    }

    let mut source_ids = BTreeSet::new();
    for source in &snapshot.sources {
        validate_identifier("source_version_id", &source.source_version_id, 128)?;
        validate_identifier("source_identity", &source.source_identity, 256)?;
        validate_sha256("source.content_sha256", &source.content_sha256)?;
        serde_json::to_string(&source.canonical_facts)?;
        validate_timestamp("source.observed_at", &source.observed_at)?;
        let (expected_identity, expected_version_id) = match source.source_kind {
            DiscoverySourceKind::Selector => (
                "RUTGERS_SELECTOR",
                format!("selector:{}", source.content_sha256),
            ),
            DiscoverySourceKind::Bootstrap => (
                "RBCSP_BOOTSTRAP",
                format!("bootstrap:{}", source.content_sha256),
            ),
        };
        if source.source_identity != expected_identity
            || source.source_version_id != expected_version_id
        {
            return Err(StorageError::InvalidCommand {
                field: "discovery.sources",
                reason: "source kind, stable identity, digest, and version ID must agree",
            });
        }
        if !source_ids.insert(source.source_version_id.clone()) {
            return Err(StorageError::InvalidCommand {
                field: "discovery.sources",
                reason: "contains a duplicate source version identity",
            });
        }
    }

    let mut term_ids = BTreeSet::new();
    for term in &snapshot.terms {
        if !source_ids.contains(&term.source_version_id) {
            return Err(missing_source("terms"));
        }
        if let Some(term_code) = &term.term_code {
            validate_identifier("term_code", term_code, 32)?;
        }
        validate_optional_text("term.display_name", term.display_name.as_deref(), 512)?;
        serde_json::to_string(&term.canonical_facts)?;
        if term.year.is_some_and(|year| year < 1900) || !term_ids.insert(term.term_id.clone()) {
            return Err(StorageError::InvalidCommand {
                field: "discovery.terms",
                reason: "contains an invalid year or duplicate term",
            });
        }
    }

    let mut campus_keys = BTreeSet::new();
    for campus in &snapshot.campuses {
        if !term_ids.contains(campus.target.term()) {
            return Err(StorageError::InvalidCommand {
                field: "discovery.campuses",
                reason: "references an absent term",
            });
        }
        if !source_ids.contains(&campus.source_version_id) {
            return Err(missing_source("campuses"));
        }
        validate_optional_text("campus.display_name", campus.display_name.as_deref(), 512)?;
        serde_json::to_string(&campus.canonical_facts)?;
        if !campus_keys.insert(campus.target.clone()) {
            return Err(StorageError::InvalidCommand {
                field: "discovery.campuses",
                reason: "contains a duplicate dynamic campus",
            });
        }
    }

    let mut subject_keys = BTreeSet::new();
    for subject in &snapshot.subjects {
        let target = TermCampusKey::new(subject.term_id.clone(), subject.campus_code.clone());
        if !campus_keys.contains(&target) {
            return Err(StorageError::InvalidCommand {
                field: "discovery.subjects",
                reason: "references an absent term/campus",
            });
        }
        if !source_ids.contains(&subject.source_version_id) {
            return Err(missing_source("subjects"));
        }
        CatalogSubjectCode::try_from(subject.subject_code.as_str()).map_err(|_| {
            StorageError::InvalidCommand {
                field: "subject_code",
                reason: "must satisfy the shared dynamic Catalog subject contract",
            }
        })?;
        validate_optional_text("subject.display_name", subject.display_name.as_deref(), 512)?;
        serde_json::to_string(&subject.canonical_facts)?;
        if !subject_keys.insert((target, subject.subject_code.clone())) {
            return Err(StorageError::InvalidCommand {
                field: "discovery.subjects",
                reason: "contains a duplicate subject",
            });
        }
    }
    Ok(())
}

fn missing_source(field: &'static str) -> StorageError {
    StorageError::InvalidCommand {
        field,
        reason: "references an absent source version",
    }
}

fn refresh_discovery_source_ownership(
    transaction: &Transaction<'_>,
    snapshot: &DiscoverySnapshot,
    content_version: u64,
) -> StorageResult<()> {
    let version = u64_to_i64(content_version)?;
    let counts = transaction.query_row(
        "SELECT (SELECT count(*) FROM catalog_terms),
                (SELECT count(*) FROM catalog_campuses),
                (SELECT count(*) FROM catalog_subjects)",
        [],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        },
    )?;
    if i64_to_u64(counts.0)? != snapshot.terms.len() as u64
        || i64_to_u64(counts.1)? != snapshot.campuses.len() as u64
        || i64_to_u64(counts.2)? != snapshot.subjects.len() as u64
    {
        return Err(StorageError::InvalidStoredSourceOwnership {
            entity_kind: "discovery dictionary",
        });
    }
    for term in &snapshot.terms {
        let updated = transaction.execute(
            "UPDATE catalog_terms SET source_version_id = ?1
             WHERE term_id = ?2 AND content_version = ?3",
            params![term.source_version_id, term.term_id.as_str(), version],
        )?;
        if updated != 1 {
            return Err(StorageError::InvalidStoredSourceOwnership {
                entity_kind: "term",
            });
        }
    }
    for campus in &snapshot.campuses {
        let updated = transaction.execute(
            "UPDATE catalog_campuses SET source_version_id = ?1
             WHERE term_id = ?2 AND campus_code = ?3 AND content_version = ?4",
            params![
                campus.source_version_id,
                campus.target.term().as_str(),
                campus.target.campus().as_str(),
                version,
            ],
        )?;
        if updated != 1 {
            return Err(StorageError::InvalidStoredSourceOwnership {
                entity_kind: "campus",
            });
        }
    }
    for subject in &snapshot.subjects {
        let updated = transaction.execute(
            "UPDATE catalog_subjects SET source_version_id = ?1
             WHERE term_id = ?2 AND campus_code = ?3 AND subject_code = ?4
               AND content_version = ?5",
            params![
                subject.source_version_id,
                subject.term_id.as_str(),
                subject.campus_code.as_str(),
                subject.subject_code,
                version,
            ],
        )?;
        if updated != 1 {
            return Err(StorageError::InvalidStoredSourceOwnership {
                entity_kind: "subject",
            });
        }
    }
    Ok(())
}

fn apply_discovery_transaction(
    transaction: &Transaction<'_>,
    prepared: &PreparedDiscovery,
) -> StorageResult<DiscoveryPublishOutcome> {
    let command = &prepared.command;
    let observation_id = command.observation_id.to_string();
    let (started_at, status) = transaction
        .query_row(
            "SELECT started_at, status FROM catalog_discovery_observations
             WHERE observation_id = ?1",
            [&observation_id],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .ok_or(StorageError::ObservationNotFound(command.observation_id))?;
    if status != "STARTED" {
        return Err(StorageError::ObservationNotStarted(command.observation_id));
    }
    validate_timestamp_order(&started_at, &command.completed_at)?;

    for source in &command.snapshot.sources {
        transaction.execute(
            "INSERT INTO catalog_discovery_source_versions
                (source_version_id, source_kind, source_identity, content_sha256,
                 canonical_facts_json, observed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(source_version_id) DO NOTHING",
            params![
                source.source_version_id,
                source.source_kind.as_str(),
                source.source_identity,
                source.content_sha256,
                serde_json::to_string(&source.canonical_facts)?,
                source.observed_at,
            ],
        )?;
        let stored = transaction.query_row(
            "SELECT source_kind, source_identity, content_sha256, canonical_facts_json
             FROM catalog_discovery_source_versions WHERE source_version_id = ?1",
            [&source.source_version_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )?;
        if stored
            != (
                source.source_kind.as_str().to_owned(),
                source.source_identity.clone(),
                source.content_sha256.clone(),
                serde_json::to_string(&source.canonical_facts)?,
            )
        {
            return Err(StorageError::InvalidCommand {
                field: "source_version_id",
                reason: "an existing source version has different immutable provenance",
            });
        }
        transaction.execute(
            "INSERT INTO catalog_discovery_observation_sources
                (observation_id, source_version_id) VALUES (?1, ?2)",
            params![observation_id, source.source_version_id],
        )?;
    }

    let current = transaction.query_row(
        "SELECT content_version, semantic_sha256
         FROM catalog_discovery_state WHERE singleton = 1",
        [],
        |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<String>>(1)?)),
    )?;
    let current_version = i64_to_u64(current.0)?;
    let changed =
        current_version == 0 || current.1.as_deref() != Some(prepared.semantic_sha256.as_str());
    let resulting_version = if changed {
        current_version
            .checked_add(1)
            .ok_or(StorageError::StoredIntegerOutOfRange)?
    } else {
        current_version
    };

    if changed {
        transaction.execute("DELETE FROM catalog_terms", [])?;
        for term in &command.snapshot.terms {
            transaction.execute(
                "INSERT INTO catalog_terms
                    (term_id, year, term_code, display_name, published, canonical_facts_json,
                     source_version_id, content_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    term.term_id.as_str(),
                    term.year.map(i64::from),
                    term.term_code,
                    term.display_name,
                    term.published,
                    serde_json::to_string(&term.canonical_facts)?,
                    term.source_version_id,
                    u64_to_i64(resulting_version)?,
                ],
            )?;
        }
        for campus in &command.snapshot.campuses {
            transaction.execute(
                "INSERT INTO catalog_campuses
                    (term_id, campus_code, display_name, category, enabled,
                     canonical_facts_json, source_version_id, content_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    campus.target.term().as_str(),
                    campus.target.campus().as_str(),
                    campus.display_name,
                    campus.category,
                    campus.enabled,
                    serde_json::to_string(&campus.canonical_facts)?,
                    campus.source_version_id,
                    u64_to_i64(resulting_version)?,
                ],
            )?;
        }
        for subject in &command.snapshot.subjects {
            transaction.execute(
                "INSERT INTO catalog_subjects
                    (term_id, campus_code, subject_code, display_name,
                     enabled, canonical_facts_json, source_version_id, content_version)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    subject.term_id.as_str(),
                    subject.campus_code.as_str(),
                    subject.subject_code,
                    subject.display_name,
                    subject.enabled,
                    serde_json::to_string(&subject.canonical_facts)?,
                    subject.source_version_id,
                    u64_to_i64(resulting_version)?,
                ],
            )?;
        }
    } else {
        refresh_discovery_source_ownership(transaction, &command.snapshot, resulting_version)?;
    }

    let status = if changed {
        "APPLIED_CHANGED"
    } else {
        "APPLIED_UNCHANGED"
    };
    transaction.execute(
        "UPDATE catalog_discovery_observations
         SET completed_at = ?2, status = ?3, semantic_sha256 = ?4,
             changed = ?5, resulting_content_version = ?6,
             term_count = ?7, campus_count = ?8, subject_count = ?9,
             error_stage = NULL, error_code = NULL, diagnostic_token = NULL
         WHERE observation_id = ?1",
        params![
            observation_id,
            command.completed_at,
            status,
            prepared.semantic_sha256,
            changed,
            u64_to_i64(resulting_version)?,
            u64_to_i64(command.snapshot.terms.len() as u64)?,
            u64_to_i64(command.snapshot.campuses.len() as u64)?,
            u64_to_i64(command.snapshot.subjects.len() as u64)?,
        ],
    )?;
    transaction.execute(
        "UPDATE catalog_discovery_state
         SET content_version = ?2, semantic_sha256 = ?3,
             last_success_observation_id = ?1,
             last_published_observation_id = ?1,
             last_nonempty_observation_id = CASE WHEN ?4 > 0
                 THEN ?1 ELSE last_nonempty_observation_id END
         WHERE singleton = 1",
        params![
            observation_id,
            u64_to_i64(resulting_version)?,
            prepared.semantic_sha256,
            u64_to_i64(command.snapshot.campuses.len() as u64)?,
        ],
    )?;

    Ok(if changed {
        DiscoveryPublishOutcome::AppliedChanged {
            content_version: resulting_version,
        }
    } else {
        DiscoveryPublishOutcome::AppliedUnchanged {
            content_version: resulting_version,
        }
    })
}
