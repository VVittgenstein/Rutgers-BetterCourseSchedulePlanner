use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use bcsp_contracts::{CourseGroupKey, CourseVariantKey, SectionKey, TermCampusKey, TraceId};
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::migration::{apply_migrations, probe_fts5, read_migration_records, sha256_hex};
use crate::open::recover_interrupted_open_attempts;
use crate::{
    BeginRefreshAttemptCommand, CatalogCounts, CatalogRefreshCommand, CatalogSnapshot,
    CourseTextSearchTokens, CourseVariantSearchHit, CourseVariantSearchResult,
    EmptySnapshotDecision, FinishRefreshFailureCommand, MigrationRecord, ProvenanceEntityKind,
    PublishOutcome, PublishedCatalogSnapshot, RefreshFailureStage, RefreshObservation,
    RefreshStatus, StorageError, StorageIntegrityReport, StorageResult, StoredCanonicalFacts,
    StoredCourseGroup, StoredCourseVariant, StoredOccurrence, StoredProvenance, StoredSection,
    TargetState,
};

pub struct OperationalStorage {
    pub(crate) connection: Connection,
    recovery_key: Option<PathBuf>,
}

static ACTIVE_DATABASES: OnceLock<Mutex<BTreeMap<PathBuf, usize>>> = OnceLock::new();

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublishFaultPoint {
    None,
    AfterServingDelete,
    BeforeCommit,
}

struct PreparedCatalogRefresh {
    command: CatalogRefreshCommand,
    semantic_sha256: String,
}

struct StagedMetadata {
    target_id: String,
    semantic_sha256: String,
    completed_at: String,
    counts: CatalogCounts,
}

impl OperationalStorage {
    pub fn open(path: impl AsRef<Path>) -> StorageResult<Self> {
        let path = path.as_ref();
        let connection = Connection::open(path)?;
        let recovery_key = database_recovery_key(path);
        let mut active_databases = ACTIVE_DATABASES
            .get_or_init(|| Mutex::new(BTreeMap::new()))
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let should_recover = match active_databases.get_mut(&recovery_key) {
            Some(active_connections) => {
                *active_connections += 1;
                false
            }
            None => {
                active_databases.insert(recovery_key.clone(), 1);
                true
            }
        };
        let result = Self::initialize(connection, should_recover, Some(recovery_key.clone()));
        if result.is_err() {
            unregister_active_database(&mut active_databases, &recovery_key);
        }
        drop(active_databases);
        result
    }

    pub fn open_in_memory() -> StorageResult<Self> {
        let connection = Connection::open_in_memory()?;
        Self::initialize(connection, true, None)
    }

    fn initialize(
        mut connection: Connection,
        recover_interrupted: bool,
        recovery_key: Option<PathBuf>,
    ) -> StorageResult<Self> {
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        connection.busy_timeout(Duration::from_secs(5))?;
        // File-backed runtimes use a dedicated serving connection alongside the
        // refresh writer. WAL keeps already-published Catalog data readable while
        // a large replacement snapshot is staged and committed atomically.
        if recovery_key.is_some() {
            connection.pragma_update(None, "journal_mode", "WAL")?;
            let journal_mode = connection
                .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))?;
            if !journal_mode.eq_ignore_ascii_case("wal") {
                return Err(StorageError::SqliteConfiguration("journal_mode"));
            }
        }
        probe_fts5(&connection)?;
        apply_migrations(&mut connection)?;
        if recover_interrupted {
            recover_interrupted_refreshes(&mut connection)?;
            recover_interrupted_open_attempts(&mut connection)?;
        }
        Ok(Self {
            connection,
            recovery_key,
        })
    }

    pub fn migration_records(&self) -> StorageResult<Vec<MigrationRecord>> {
        read_migration_records(&self.connection)
    }

    pub fn operational_table_names(&self) -> StorageResult<Vec<String>> {
        let mut statement = self.connection.prepare(
            "SELECT name FROM sqlite_schema
             WHERE type IN ('table', 'view')
               AND name NOT LIKE 'sqlite_%'
               AND name NOT LIKE 'catalog_course_fts_%'
             ORDER BY name",
        )?;
        let rows = statement.query_map([], |row| row.get(0))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn begin_refresh_attempt(
        &mut self,
        command: &BeginRefreshAttemptCommand,
    ) -> StorageResult<()> {
        validate_timestamp("started_at", &command.started_at)?;
        validate_optional_sha256(
            "source_content_sha256",
            command.source_content_sha256.as_deref(),
        )?;
        let source_bytes = optional_u64_to_i64(command.source_bytes)?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        ensure_target(&transaction, &command.target, &command.started_at)?;
        insert_or_update_started_observation(
            &transaction,
            command.observation_id,
            &command.target,
            &command.started_at,
            command.source_content_sha256.as_deref(),
            source_bytes,
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn finish_refresh_failure(
        &mut self,
        command: &FinishRefreshFailureCommand,
    ) -> StorageResult<()> {
        validate_timestamp("completed_at", &command.completed_at)?;
        validate_optional_sha256(
            "source_content_sha256",
            command.source_content_sha256.as_deref(),
        )?;
        validate_safe_code("error_code", &command.error_code)?;
        validate_diagnostic_token(command.diagnostic_token.as_deref())?;
        let source_bytes = optional_u64_to_i64(command.source_bytes)?;

        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let observation_id = command.observation_id.to_string();
        let (started_at, status, stored_source_content_sha256, stored_source_bytes) = transaction
            .query_row(
                "SELECT started_at, status, source_content_sha256, source_bytes
                 FROM catalog_refresh_observations
                 WHERE observation_id = ?1",
                [&observation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<i64>>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or(StorageError::ObservationNotFound(command.observation_id))?;
        if status != "STARTED" && status != "STAGED" {
            return Err(StorageError::ObservationNotStarted(command.observation_id));
        }
        validate_timestamp_order(&started_at, &command.completed_at)?;
        validate_source_metadata_compatibility(
            stored_source_content_sha256.as_deref(),
            command.source_content_sha256.as_deref(),
            stored_source_bytes,
            source_bytes,
        )?;
        clear_staging(&transaction, command.observation_id)?;
        transaction.execute(
            "UPDATE catalog_refresh_observations
             SET status = 'FAILED', completed_at = ?2,
                 source_content_sha256 = COALESCE(source_content_sha256, ?3),
                 source_bytes = COALESCE(source_bytes, ?4),
                 error_stage = ?5, error_code = ?6, diagnostic_token = ?7
             WHERE observation_id = ?1",
            params![
                observation_id,
                command.completed_at,
                command.source_content_sha256,
                source_bytes,
                command.stage.as_str(),
                command.error_code,
                command.diagnostic_token,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn apply_catalog_refresh(
        &mut self,
        command: CatalogRefreshCommand,
        empty_decision: EmptySnapshotDecision,
    ) -> StorageResult<PublishOutcome> {
        self.ensure_catalog_attempt_started(&command)?;
        let completed_at = command.completed_at.clone();
        let observation_id = command.observation_id;
        let prepared = match prepare_catalog_refresh(command) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.record_failure_best_effort(
                    &observation_id,
                    &completed_at,
                    RefreshFailureStage::Normalization,
                    "STORAGE_PROJECTION_INVALID",
                );
                return Err(error);
            }
        };
        let result = self.stage_and_publish(prepared, empty_decision, PublishFaultPoint::None);
        if result.is_err() {
            self.record_failure_best_effort(
                &observation_id,
                &completed_at,
                RefreshFailureStage::Publish,
                "STORAGE_PUBLISH_FAILED",
            );
        }
        result
    }

    pub fn stage_catalog_refresh(&mut self, command: CatalogRefreshCommand) -> StorageResult<()> {
        self.ensure_catalog_attempt_started(&command)?;
        let completed_at = command.completed_at.clone();
        let observation_id = command.observation_id;
        let prepared = match prepare_catalog_refresh(command) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.record_failure_best_effort(
                    &observation_id,
                    &completed_at,
                    RefreshFailureStage::Normalization,
                    "STORAGE_PROJECTION_INVALID",
                );
                return Err(error);
            }
        };
        let result = (|| {
            let transaction = self
                .connection
                .transaction_with_behavior(TransactionBehavior::Immediate)?;
            stage_prepared_catalog_refresh(&transaction, &prepared)?;
            transaction.commit()?;
            Ok(())
        })();
        if result.is_err() {
            self.record_failure_best_effort(
                &observation_id,
                &completed_at,
                RefreshFailureStage::Publish,
                "STORAGE_STAGE_FAILED",
            );
        }
        result
    }

    pub fn publish_staged_refresh(
        &mut self,
        observation_id: &TraceId,
        empty_decision: EmptySnapshotDecision,
    ) -> StorageResult<PublishOutcome> {
        let observation_id_text = observation_id.to_string();
        let completed_at = self
            .connection
            .query_row(
                "SELECT completed_at FROM catalog_refresh_observations
                 WHERE observation_id = ?1 AND status = 'STAGED'",
                [&observation_id_text],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten()
            .ok_or(StorageError::ObservationNotStaged(*observation_id))?;
        let result = self.publish_staged_with_fault(
            *observation_id,
            empty_decision,
            PublishFaultPoint::None,
        );
        if result.is_err() {
            self.record_failure_best_effort(
                observation_id,
                &completed_at,
                RefreshFailureStage::Publish,
                "STORAGE_PUBLISH_FAILED",
            );
        }
        result
    }

    pub fn target_state(&self, target: &TermCampusKey) -> StorageResult<Option<TargetState>> {
        load_target_state(&self.connection, target)
    }

    pub fn published_catalog_snapshot(
        &mut self,
        target: &TermCampusKey,
    ) -> StorageResult<Option<PublishedCatalogSnapshot>> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)?;
        let Some(state) = load_target_state(&transaction, target)? else {
            transaction.commit()?;
            return Ok(None);
        };
        if state.current_content_version == 0 {
            transaction.commit()?;
            return Ok(None);
        }
        let publication =
            state
                .last_published
                .ok_or(StorageError::MissingPublishedObservation {
                    scope: "catalog publication",
                })?;
        if publication.resulting_content_version != Some(state.current_content_version) {
            return Err(StorageError::InconsistentPublishedVersion {
                entity_kind: "publication observation",
            });
        }
        let snapshot = load_catalog_snapshot(&transaction, target, state.current_content_version)?;
        if snapshot.counts() != state.counts {
            return Err(StorageError::InvalidStoredProjection {
                table: "catalog_targets",
                field: "entity counts",
                reason: "do not match published serving rows",
            });
        }
        let recomputed_semantic_sha256 = catalog_content_sha256_v1(target, &snapshot)?;
        if state.accepted_semantic_sha256.as_deref() != Some(recomputed_semantic_sha256.as_str()) {
            return Err(StorageError::InvalidStoredProjection {
                table: "catalog_targets",
                field: "accepted_semantic_hash",
                reason: "does not match published serving rows",
            });
        }
        let published = PublishedCatalogSnapshot {
            target: target.clone(),
            content_version: state.current_content_version,
            publication,
            snapshot,
        };
        transaction.commit()?;
        Ok(Some(published))
    }

    pub fn refresh_observation(
        &self,
        observation_id: &TraceId,
    ) -> StorageResult<Option<RefreshObservation>> {
        load_refresh_observation(&self.connection, observation_id)
    }

    pub fn serving_section_keys(&self, target: &TermCampusKey) -> StorageResult<Vec<SectionKey>> {
        let mut statement = self.connection.prepare(
            "SELECT section_index FROM catalog_sections
             WHERE target_id = ?1 ORDER BY section_index",
        )?;
        let indexes = statement.query_map([target_id(target)], |row| row.get::<_, String>(0))?;
        indexes
            .map(|index| {
                let index = index?;
                SectionKey::try_new(target.term().as_str(), target.campus().as_str(), &index)
                    .map_err(StorageError::from)
            })
            .collect()
    }

    pub fn sections(&self, target: &TermCampusKey) -> StorageResult<Vec<StoredSection>> {
        let mut statement = self.connection.prepare(
            "SELECT section_index, course_string, fingerprint, section_number,
                    catalog_status, section_course_type, delivery_modality,
                    synchronicity, canonical_facts_json, canonical_sha256
             FROM catalog_sections WHERE target_id = ?1
             ORDER BY section_index",
        )?;
        let rows = statement.query_map([target_id(target)], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
            ))
        })?;
        rows.map(|row| {
            let row = row?;
            let group =
                CourseGroupKey::try_new(target.term().as_str(), target.campus().as_str(), &row.1)?;
            Ok(StoredSection {
                key: SectionKey::try_new(target.term().as_str(), target.campus().as_str(), &row.0)?,
                variant_key: CourseVariantKey::try_new(group, &row.2)?,
                section_number: row.3,
                catalog_status: row.4,
                section_course_type: row.5,
                delivery_modality: row.6,
                synchronicity: row.7,
                canonical_facts: serde_json::from_str(&row.8)?,
                canonical_sha256: row.9,
            })
        })
        .collect()
    }

    pub fn canonical_facts(
        &self,
        target: &TermCampusKey,
    ) -> StorageResult<Vec<StoredCanonicalFacts>> {
        let mut statement = self.connection.prepare(
            "SELECT entity_kind, entity_key, canonical_facts_json FROM (
                 SELECT 'COURSE_GROUP' AS entity_kind, course_string AS entity_key,
                        canonical_facts_json
                 FROM catalog_course_groups WHERE target_id = ?1
                 UNION ALL
                 SELECT 'COURSE_VARIANT', course_string || '/' || fingerprint,
                        canonical_facts_json
                 FROM catalog_course_variants WHERE target_id = ?1
                 UNION ALL
                 SELECT 'SECTION', section_index, canonical_facts_json
                 FROM catalog_sections WHERE target_id = ?1
                 UNION ALL
                 SELECT 'OCCURRENCE', section_index || '/' || occurrence_key,
                        canonical_facts_json
                 FROM catalog_occurrences WHERE target_id = ?1
             ) ORDER BY entity_kind, entity_key",
        )?;
        let rows = statement.query_map([target_id(target)], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (kind, entity_key, json) = row?;
            let entity_kind = match kind.as_str() {
                "COURSE_GROUP" => crate::ProvenanceEntityKind::CourseGroup,
                "COURSE_VARIANT" => crate::ProvenanceEntityKind::CourseVariant,
                "SECTION" => crate::ProvenanceEntityKind::Section,
                "OCCURRENCE" => crate::ProvenanceEntityKind::Occurrence,
                _ => {
                    return Err(StorageError::InvalidCommand {
                        field: "entity_kind",
                        reason: "stored canonical fact kind is invalid",
                    });
                }
            };
            Ok(StoredCanonicalFacts {
                entity_kind,
                entity_key,
                canonical_facts: serde_json::from_str(&json)?,
            })
        })
        .collect()
    }

    pub fn course_variants(
        &self,
        target: &TermCampusKey,
    ) -> StorageResult<Vec<StoredCourseVariant>> {
        let mut statement = self.connection.prepare(
            "SELECT course_string, fingerprint, subject_code, course_number, title,
                    description, credits_summary, supplement, search_document,
                    canonical_sha256, raw_multiplicity, canonical_facts_json
             FROM catalog_course_variants WHERE target_id = ?1
             ORDER BY course_string, fingerprint",
        )?;
        let rows = statement.query_map([target_id(target)], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, String>(11)?,
            ))
        })?;
        rows.map(|row| {
            let row = row?;
            let group =
                CourseGroupKey::try_new(target.term().as_str(), target.campus().as_str(), &row.0)?;
            Ok(StoredCourseVariant {
                key: CourseVariantKey::try_new(group, &row.1)?,
                subject_code: row.2,
                course_number: row.3,
                title: row.4,
                description: row.5,
                credits_summary: row.6,
                supplement: row.7,
                search_document: row.8,
                canonical_sha256: row.9,
                raw_multiplicity: u32::try_from(row.10)
                    .map_err(|_| StorageError::StoredIntegerOutOfRange)?,
                canonical_facts: serde_json::from_str(&row.11)?,
            })
        })
        .collect()
    }

    pub fn search_course_variants(
        &mut self,
        target: &TermCampusKey,
        content_version: u64,
        tokens: &CourseTextSearchTokens,
    ) -> StorageResult<CourseVariantSearchResult> {
        if content_version == 0 {
            return Err(StorageError::InvalidCommand {
                field: "content_version",
                reason: "must identify a published catalog version",
            });
        }
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Deferred)?;
        let current_version = transaction
            .query_row(
                "SELECT current_content_version FROM catalog_targets WHERE target_id = ?1",
                [target_id(target)],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map(i64_to_u64)
            .transpose()?;
        let Some(current_version) = current_version.filter(|version| *version > 0) else {
            return Err(StorageError::CatalogTargetNotPublished);
        };
        if current_version != content_version {
            return Err(StorageError::CatalogContentVersionMismatch {
                requested: content_version,
                current: current_version,
            });
        }

        let fts_query = fts5_literal_and_query(tokens);
        let ordered_keys = {
            let mut statement = transaction.prepare(
                "SELECT f.course_string, f.fingerprint
                 FROM catalog_course_fts AS f
                 JOIN catalog_course_variants AS v
                   ON v.target_id = f.target_id
                  AND v.course_string = f.course_string
                  AND v.fingerprint = f.fingerprint
                 JOIN catalog_targets AS t ON t.target_id = f.target_id
                 WHERE catalog_course_fts MATCH ?1
                   AND f.target_id = ?2
                   AND f.content_version = ?3
                   AND v.content_version = ?3
                   AND t.current_content_version = ?3
                 ORDER BY bm25(catalog_course_fts),
                          f.course_string COLLATE BINARY,
                          f.fingerprint COLLATE BINARY",
            )?;
            let rows = statement.query_map(
                params![fts_query, target_id(target), u64_to_i64(content_version)?],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )?;
            rows.map(|row| {
                let (course_string, fingerprint) = row?;
                let group = CourseGroupKey::try_new(
                    target.term().as_str(),
                    target.campus().as_str(),
                    &course_string,
                )?;
                CourseVariantKey::try_new(group, &fingerprint).map_err(StorageError::from)
            })
            .collect::<StorageResult<Vec<_>>>()?
        };
        let hits = ordered_keys
            .into_iter()
            .enumerate()
            .map(|(rank, key)| {
                Ok(CourseVariantSearchHit {
                    key,
                    fts_rank: u32::try_from(rank)
                        .map_err(|_| StorageError::StoredIntegerOutOfRange)?,
                })
            })
            .collect::<StorageResult<Vec<_>>>()?;
        transaction.commit()?;
        Ok(CourseVariantSearchResult {
            target: target.clone(),
            content_version,
            hits,
        })
    }

    pub fn integrity_report(
        &self,
        target: &TermCampusKey,
    ) -> StorageResult<StorageIntegrityReport> {
        let target_id = target_id(target);
        let foreign_key_violations = self.connection.query_row(
            "SELECT count(*) FROM pragma_foreign_key_check",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        let missing_fts_rows = self.connection.query_row(
            "SELECT count(*) FROM catalog_course_variants v
             WHERE v.target_id = ?1 AND NOT EXISTS (
                 SELECT 1 FROM catalog_course_fts f
                 WHERE f.target_id = v.target_id
                   AND f.course_string = v.course_string
                   AND f.fingerprint = v.fingerprint
             )",
            [&target_id],
            |row| row.get::<_, i64>(0),
        )?;
        let orphan_fts_rows = self.connection.query_row(
            "SELECT count(*) FROM catalog_course_fts f
             WHERE f.target_id = ?1 AND NOT EXISTS (
                 SELECT 1 FROM catalog_course_variants v
                 WHERE v.target_id = f.target_id
                   AND v.course_string = f.course_string
                   AND v.fingerprint = f.fingerprint
             )",
            [&target_id],
            |row| row.get::<_, i64>(0),
        )?;
        let duplicate_fts_rows = self.connection.query_row(
            "SELECT COALESCE(sum(excess), 0) FROM (
                 SELECT count(*) - 1 AS excess
                 FROM catalog_course_fts
                 WHERE target_id = ?1
                 GROUP BY target_id, course_string, fingerprint
                 HAVING count(*) > 1
             )",
            [&target_id],
            |row| row.get::<_, i64>(0),
        )?;
        let version_mismatched_fts_rows = self.connection.query_row(
            "SELECT count(*) FROM catalog_course_fts f
             JOIN catalog_course_variants v
               ON v.target_id = f.target_id
              AND v.course_string = f.course_string
              AND v.fingerprint = f.fingerprint
             WHERE f.target_id = ?1 AND f.content_version != v.content_version",
            [&target_id],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(StorageIntegrityReport {
            foreign_key_violations: i64_to_u64(foreign_key_violations)?,
            missing_fts_rows: i64_to_u64(missing_fts_rows)?,
            orphan_fts_rows: i64_to_u64(orphan_fts_rows)?,
            duplicate_fts_rows: i64_to_u64(duplicate_fts_rows)?,
            version_mismatched_fts_rows: i64_to_u64(version_mismatched_fts_rows)?,
        })
    }

    pub fn staged_payload_count(&self) -> StorageResult<u64> {
        let count = self.connection.query_row(
            "SELECT count(*) FROM catalog_staging_payloads",
            [],
            |row| row.get::<_, i64>(0),
        )?;
        i64_to_u64(count)
    }

    fn ensure_catalog_attempt_started(
        &mut self,
        command: &CatalogRefreshCommand,
    ) -> StorageResult<()> {
        self.begin_refresh_attempt(&BeginRefreshAttemptCommand {
            observation_id: command.observation_id,
            target: command.target.clone(),
            started_at: command.started_at.clone(),
            source_content_sha256: Some(command.source_content_sha256.clone()),
            source_bytes: Some(command.source_bytes),
        })
    }

    fn stage_and_publish(
        &mut self,
        prepared: PreparedCatalogRefresh,
        empty_decision: EmptySnapshotDecision,
        fault: PublishFaultPoint,
    ) -> StorageResult<PublishOutcome> {
        let observation_id = prepared.command.observation_id;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        stage_prepared_catalog_refresh(&transaction, &prepared)?;
        let outcome = publish_staged(&transaction, observation_id, empty_decision, fault)?;
        transaction.commit()?;
        Ok(outcome)
    }

    fn publish_staged_with_fault(
        &mut self,
        observation_id: TraceId,
        empty_decision: EmptySnapshotDecision,
        fault: PublishFaultPoint,
    ) -> StorageResult<PublishOutcome> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)?;
        let outcome = publish_staged(&transaction, observation_id, empty_decision, fault)?;
        transaction.commit()?;
        Ok(outcome)
    }

    fn record_failure_best_effort(
        &mut self,
        observation_id: &TraceId,
        completed_at: &str,
        stage: RefreshFailureStage,
        error_code: &str,
    ) {
        let _ = self.finish_refresh_failure(&FinishRefreshFailureCommand {
            observation_id: *observation_id,
            completed_at: completed_at.to_owned(),
            stage,
            source_content_sha256: None,
            source_bytes: None,
            error_code: error_code.to_owned(),
            diagnostic_token: None,
        });
    }
}

impl Drop for OperationalStorage {
    fn drop(&mut self) {
        let Some(recovery_key) = self.recovery_key.as_ref() else {
            return;
        };
        let Some(active_databases) = ACTIVE_DATABASES.get() else {
            return;
        };
        let mut active_databases = active_databases
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        unregister_active_database(&mut active_databases, recovery_key);
    }
}

fn database_recovery_key(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| {
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map(|directory| directory.join(path))
                .unwrap_or_else(|_| path.to_path_buf())
        }
    })
}

fn unregister_active_database(
    active_databases: &mut BTreeMap<PathBuf, usize>,
    recovery_key: &Path,
) {
    let Some(active_connections) = active_databases.get_mut(recovery_key) else {
        return;
    };
    *active_connections -= 1;
    if *active_connections == 0 {
        active_databases.remove(recovery_key);
    }
}

pub(crate) fn parse_stored_trace_id(
    table: &'static str,
    column: &'static str,
    value: &str,
) -> StorageResult<TraceId> {
    value
        .parse()
        .map_err(|_| StorageError::InvalidStoredTraceId { table, column })
}

pub(crate) fn parse_optional_stored_trace_id(
    table: &'static str,
    column: &'static str,
    value: Option<String>,
) -> StorageResult<Option<TraceId>> {
    value
        .as_deref()
        .map(|value| parse_stored_trace_id(table, column, value))
        .transpose()
}

pub(crate) fn validate_stored_timestamp(
    table: &'static str,
    column: &'static str,
    value: &str,
) -> StorageResult<OffsetDateTime> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|_| StorageError::InvalidStoredTimestamp { table, column })
}

fn load_refresh_observation(
    connection: &Connection,
    observation_id: &TraceId,
) -> StorageResult<Option<RefreshObservation>> {
    let observation_id_text = observation_id.to_string();
    let row = connection
        .query_row(
            "SELECT t.term_id, t.campus_code, o.attempt_sequence,
                    o.started_at, o.completed_at,
                    o.status, o.source_content_sha256, o.semantic_sha256, o.source_bytes,
                    o.group_count, o.variant_count, o.section_count, o.occurrence_count,
                    o.changed, o.resulting_content_version, o.retained_observation_id,
                    o.error_stage, o.error_code, o.diagnostic_token
             FROM catalog_refresh_observations o
             JOIN catalog_targets t ON t.target_id = o.target_id
             WHERE o.observation_id = ?1",
            [&observation_id_text],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, Option<String>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<i64>>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, i64>(11)?,
                    row.get::<_, i64>(12)?,
                    row.get::<_, Option<bool>>(13)?,
                    row.get::<_, Option<i64>>(14)?,
                    row.get::<_, Option<String>>(15)?,
                    row.get::<_, Option<String>>(16)?,
                    row.get::<_, Option<String>>(17)?,
                    row.get::<_, Option<String>>(18)?,
                ))
            },
        )
        .optional()?;
    let Some(row) = row else {
        return Ok(None);
    };
    let started = validate_stored_timestamp("catalog_refresh_observations", "started_at", &row.3)?;
    if let Some(completed_at) = row.4.as_deref() {
        let completed = validate_stored_timestamp(
            "catalog_refresh_observations",
            "completed_at",
            completed_at,
        )?;
        if completed < started {
            return Err(StorageError::InvalidStoredProjection {
                table: "catalog_refresh_observations",
                field: "completed_at",
                reason: "must not precede started_at",
            });
        }
    }
    let status = RefreshStatus::parse(&row.5).ok_or(StorageError::UnknownRefreshStatus)?;
    if (status == RefreshStatus::Started) != row.4.is_none() {
        return Err(StorageError::InvalidStoredProjection {
            table: "catalog_refresh_observations",
            field: "completed_at",
            reason: "must be absent only while an observation is STARTED",
        });
    }
    if let Some(value) = row.6.as_deref() {
        stored_sha256(
            "catalog_refresh_observations",
            "source_content_sha256",
            value,
        )?;
    }
    if let Some(value) = row.7.as_deref() {
        stored_sha256("catalog_refresh_observations", "semantic_sha256", value)?;
    }
    Ok(Some(RefreshObservation {
        observation_id: *observation_id,
        target: TermCampusKey::try_new(&row.0, &row.1)?,
        attempt_sequence: i64_to_u64(row.2)?,
        started_at: row.3,
        completed_at: row.4,
        status,
        source_content_sha256: row.6,
        semantic_sha256: row.7,
        source_bytes: row.8.map(i64_to_u64).transpose()?,
        counts: CatalogCounts {
            course_groups: i64_to_u64(row.9)?,
            course_variants: i64_to_u64(row.10)?,
            sections: i64_to_u64(row.11)?,
            occurrences: i64_to_u64(row.12)?,
        },
        changed: row.13,
        resulting_content_version: row.14.map(i64_to_u64).transpose()?,
        retained_observation_id: parse_optional_stored_trace_id(
            "catalog_refresh_observations",
            "retained_observation_id",
            row.15,
        )?,
        error_stage: row.16,
        error_code: row.17,
        diagnostic_token: row.18,
    }))
}

fn load_target_state(
    connection: &Connection,
    target: &TermCampusKey,
) -> StorageResult<Option<TargetState>> {
    let row = connection
        .query_row(
            "SELECT current_content_version, last_attempt_sequence,
                    accepted_semantic_hash,
                    group_count, variant_count, section_count, occurrence_count,
                    last_attempt_observation_id, last_success_observation_id,
                    last_published_observation_id, last_nonempty_observation_id,
                    pending_empty_observation_id
             FROM catalog_targets WHERE target_id = ?1",
            [target_id(target)],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
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
    let last_attempt_observation_id =
        parse_optional_stored_trace_id("catalog_targets", "last_attempt_observation_id", row.7)?;
    let last_success_observation_id =
        parse_optional_stored_trace_id("catalog_targets", "last_success_observation_id", row.8)?;
    let last_published_observation_id =
        parse_optional_stored_trace_id("catalog_targets", "last_published_observation_id", row.9)?;
    let last_nonempty_observation_id =
        parse_optional_stored_trace_id("catalog_targets", "last_nonempty_observation_id", row.10)?;
    let pending_empty_observation_id =
        parse_optional_stored_trace_id("catalog_targets", "pending_empty_observation_id", row.11)?;
    let load = |pointer: Option<&TraceId>| -> StorageResult<Option<RefreshObservation>> {
        match pointer {
            Some(pointer) => load_refresh_observation(connection, pointer)?
                .ok_or(StorageError::MissingPublishedObservation {
                    scope: "target-state pointer",
                })
                .map(Some),
            None => Ok(None),
        }
    };
    let last_attempt = load(last_attempt_observation_id.as_ref())?;
    let last_success = load(last_success_observation_id.as_ref())?;
    let last_published = load(last_published_observation_id.as_ref())?;
    let last_nonempty = load(last_nonempty_observation_id.as_ref())?;
    let pending_empty = load(pending_empty_observation_id.as_ref())?;
    for observation in [
        last_attempt.as_ref(),
        last_success.as_ref(),
        last_published.as_ref(),
        last_nonempty.as_ref(),
        pending_empty.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        if observation.target != *target {
            return Err(StorageError::InvalidStoredProjection {
                table: "catalog_targets",
                field: "observation pointer",
                reason: "points outside the target",
            });
        }
    }
    let current_content_version = i64_to_u64(row.0)?;
    if let Some(value) = row.2.as_deref() {
        stored_sha256("catalog_targets", "accepted_semantic_hash", value)?;
    }
    if current_content_version > 0 && last_published.is_none() {
        return Err(StorageError::MissingPublishedObservation {
            scope: "catalog publication",
        });
    }
    let last_attempt_sequence = i64_to_u64(row.1)?;
    if (last_attempt_sequence == 0) != last_attempt.is_none() {
        return Err(StorageError::InvalidStoredProjection {
            table: "catalog_targets",
            field: "last_attempt_observation_id",
            reason: "must agree with whether an attempt sequence exists",
        });
    }
    if last_attempt
        .as_ref()
        .is_some_and(|observation| observation.attempt_sequence != last_attempt_sequence)
    {
        return Err(StorageError::InvalidStoredProjection {
            table: "catalog_targets",
            field: "last_attempt_sequence",
            reason: "does not match the latest attempt observation",
        });
    }
    if last_success.as_ref().is_some_and(|observation| {
        !matches!(
            observation.status,
            RefreshStatus::AppliedChanged
                | RefreshStatus::AppliedUnchanged
                | RefreshStatus::EmptyValidInitial
                | RefreshStatus::EmptySuspectRetained
        ) || observation.completed_at.is_none()
            || observation.resulting_content_version != Some(current_content_version)
    }) {
        return Err(StorageError::InvalidStoredProjection {
            table: "catalog_targets",
            field: "last_success_observation_id",
            reason: "does not point to a completed successful response for the current version",
        });
    }
    if last_published.as_ref().is_some_and(|observation| {
        !matches!(
            observation.status,
            RefreshStatus::AppliedChanged
                | RefreshStatus::AppliedUnchanged
                | RefreshStatus::EmptyValidInitial
        ) || observation.completed_at.is_none()
            || observation.resulting_content_version != Some(current_content_version)
    }) {
        return Err(StorageError::InvalidStoredProjection {
            table: "catalog_targets",
            field: "last_published_observation_id",
            reason: "does not point to a completed publication for the current version",
        });
    }
    if last_nonempty.as_ref().is_some_and(|observation| {
        !matches!(
            observation.status,
            RefreshStatus::AppliedChanged | RefreshStatus::AppliedUnchanged
        ) || observation.counts == CatalogCounts::default()
            || observation.completed_at.is_none()
    }) {
        return Err(StorageError::InvalidStoredProjection {
            table: "catalog_targets",
            field: "last_nonempty_observation_id",
            reason: "does not point to a completed nonempty publication",
        });
    }
    if pending_empty.as_ref().is_some_and(|observation| {
        observation.status != RefreshStatus::EmptySuspectRetained
            || observation.completed_at.is_none()
            || observation.resulting_content_version != Some(current_content_version)
    }) {
        return Err(StorageError::InvalidStoredProjection {
            table: "catalog_targets",
            field: "pending_empty_observation_id",
            reason: "does not point to retained suspect-empty evidence",
        });
    }
    Ok(Some(TargetState {
        target: target.clone(),
        last_attempt_sequence,
        current_content_version,
        accepted_semantic_sha256: row.2,
        counts: CatalogCounts {
            course_groups: i64_to_u64(row.3)?,
            course_variants: i64_to_u64(row.4)?,
            sections: i64_to_u64(row.5)?,
            occurrences: i64_to_u64(row.6)?,
        },
        last_attempt_observation_id,
        last_success_observation_id,
        last_published_observation_id,
        last_nonempty_observation_id,
        pending_empty_observation_id,
        last_attempt,
        last_success,
        last_published,
        last_nonempty,
        pending_empty,
    }))
}

fn require_published_version(
    entity_kind: &'static str,
    actual: i64,
    expected: u64,
) -> StorageResult<()> {
    if i64_to_u64(actual)? != expected {
        return Err(StorageError::InconsistentPublishedVersion { entity_kind });
    }
    Ok(())
}

fn parse_provenance_kind(value: &str) -> StorageResult<ProvenanceEntityKind> {
    match value {
        "COURSE_GROUP" => Ok(ProvenanceEntityKind::CourseGroup),
        "COURSE_VARIANT" => Ok(ProvenanceEntityKind::CourseVariant),
        "SECTION" => Ok(ProvenanceEntityKind::Section),
        "OCCURRENCE" => Ok(ProvenanceEntityKind::Occurrence),
        _ => Err(StorageError::InvalidStoredProjection {
            table: "catalog_provenance",
            field: "entity_kind",
            reason: "is not a supported provenance entity kind",
        }),
    }
}

fn stored_wire_token(
    table: &'static str,
    field: &'static str,
    value: &str,
    allowed: &[&str],
) -> StorageResult<()> {
    if !allowed.contains(&value) {
        return Err(StorageError::InvalidStoredProjection {
            table,
            field,
            reason: "is not an exact shared-contract wire value",
        });
    }
    Ok(())
}

pub(crate) fn stored_sha256(
    table: &'static str,
    field: &'static str,
    value: &str,
) -> StorageResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(StorageError::InvalidStoredProjection {
            table,
            field,
            reason: "is not canonical lowercase SHA-256 hex",
        });
    }
    Ok(())
}

fn load_catalog_snapshot(
    connection: &Connection,
    target: &TermCampusKey,
    content_version: u64,
) -> StorageResult<CatalogSnapshot> {
    let target_id = target_id(target);
    let mut group_statement = connection.prepare(
        "SELECT course_string, canonical_facts_json, content_version
         FROM catalog_course_groups WHERE target_id = ?1 ORDER BY course_string",
    )?;
    let course_groups = group_statement
        .query_map([&target_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .map(|row| {
            let (course_string, canonical_facts, version) = row?;
            require_published_version("course group", version, content_version)?;
            Ok(StoredCourseGroup {
                key: CourseGroupKey::try_new(
                    target.term().as_str(),
                    target.campus().as_str(),
                    &course_string,
                )?,
                canonical_facts: serde_json::from_str(&canonical_facts)?,
            })
        })
        .collect::<StorageResult<Vec<_>>>()?;

    let mut variant_statement = connection.prepare(
        "SELECT course_string, fingerprint, subject_code, course_number, title,
                description, credits_summary, supplement, search_document,
                canonical_sha256, raw_multiplicity, canonical_facts_json, content_version
         FROM catalog_course_variants WHERE target_id = ?1
         ORDER BY course_string, fingerprint",
    )?;
    let course_variants = variant_statement
        .query_map([&target_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, i64>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, i64>(12)?,
            ))
        })?
        .map(|row| {
            let row = row?;
            require_published_version("course variant", row.12, content_version)?;
            stored_sha256("catalog_course_variants", "canonical_sha256", &row.9)?;
            let group =
                CourseGroupKey::try_new(target.term().as_str(), target.campus().as_str(), &row.0)?;
            Ok(StoredCourseVariant {
                key: CourseVariantKey::try_new(group, &row.1)?,
                subject_code: row.2,
                course_number: row.3,
                title: row.4,
                description: row.5,
                credits_summary: row.6,
                supplement: row.7,
                search_document: row.8,
                canonical_sha256: row.9,
                raw_multiplicity: u32::try_from(row.10)
                    .map_err(|_| StorageError::StoredIntegerOutOfRange)?,
                canonical_facts: serde_json::from_str(&row.11)?,
            })
        })
        .collect::<StorageResult<Vec<_>>>()?;

    let mut section_statement = connection.prepare(
        "SELECT section_index, course_string, fingerprint, section_number,
                catalog_status, section_course_type, delivery_modality, synchronicity,
                canonical_facts_json, canonical_sha256, content_version
         FROM catalog_sections WHERE target_id = ?1 ORDER BY section_index",
    )?;
    let sections = section_statement
        .query_map([&target_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, i64>(10)?,
            ))
        })?
        .map(|row| {
            let row = row?;
            require_published_version("section", row.10, content_version)?;
            stored_sha256("catalog_sections", "canonical_sha256", &row.9)?;
            stored_wire_token(
                "catalog_sections",
                "delivery_modality",
                &row.6,
                MODALITY_WIRE_VALUES,
            )?;
            stored_wire_token(
                "catalog_sections",
                "synchronicity",
                &row.7,
                SYNCHRONICITY_WIRE_VALUES,
            )?;
            let group =
                CourseGroupKey::try_new(target.term().as_str(), target.campus().as_str(), &row.1)?;
            Ok(StoredSection {
                key: SectionKey::try_new(target.term().as_str(), target.campus().as_str(), &row.0)?,
                variant_key: CourseVariantKey::try_new(group, &row.2)?,
                section_number: row.3,
                catalog_status: row.4,
                section_course_type: row.5,
                delivery_modality: row.6,
                synchronicity: row.7,
                canonical_facts: serde_json::from_str(&row.8)?,
                canonical_sha256: row.9,
            })
        })
        .collect::<StorageResult<Vec<_>>>()?;

    let mut occurrence_statement = connection.prepare(
        "SELECT section_index, occurrence_key, ordinal, weekday, start_minute, end_minute,
                time_knowledge, requiredness, occurrence_kind, modality, synchronicity,
                evidence, normalization_reason, location, building, room, raw_sha256,
                canonical_facts_json, content_version
         FROM catalog_occurrences WHERE target_id = ?1
         ORDER BY section_index, occurrence_key",
    )?;
    let occurrences = occurrence_statement
        .query_map([&target_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<i64>>(4)?,
                row.get::<_, Option<i64>>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, String>(9)?,
                row.get::<_, String>(10)?,
                row.get::<_, String>(11)?,
                row.get::<_, String>(12)?,
                row.get::<_, Option<String>>(13)?,
                row.get::<_, Option<String>>(14)?,
                row.get::<_, Option<String>>(15)?,
                row.get::<_, String>(16)?,
                row.get::<_, String>(17)?,
                row.get::<_, i64>(18)?,
            ))
        })?
        .map(|row| {
            let row = row?;
            require_published_version("occurrence", row.18, content_version)?;
            stored_sha256("catalog_occurrences", "raw_sha256", &row.16)?;
            stored_wire_token(
                "catalog_occurrences",
                "time_knowledge",
                &row.6,
                TIME_KNOWLEDGE_WIRE_VALUES,
            )?;
            stored_wire_token(
                "catalog_occurrences",
                "requiredness",
                &row.7,
                REQUIREDNESS_WIRE_VALUES,
            )?;
            stored_wire_token(
                "catalog_occurrences",
                "occurrence_kind",
                &row.8,
                OCCURRENCE_KIND_WIRE_VALUES,
            )?;
            stored_wire_token(
                "catalog_occurrences",
                "modality",
                &row.9,
                MODALITY_WIRE_VALUES,
            )?;
            stored_wire_token(
                "catalog_occurrences",
                "synchronicity",
                &row.10,
                SYNCHRONICITY_WIRE_VALUES,
            )?;
            stored_wire_token(
                "catalog_occurrences",
                "evidence",
                &row.11,
                OCCURRENCE_EVIDENCE_WIRE_VALUES,
            )?;
            if validate_safe_code("normalization_reason", &row.12).is_err() {
                return Err(StorageError::InvalidStoredProjection {
                    table: "catalog_occurrences",
                    field: "normalization_reason",
                    reason: "is not a bounded stable diagnostic code",
                });
            }
            let occurrence = StoredOccurrence {
                section_key: SectionKey::try_new(
                    target.term().as_str(),
                    target.campus().as_str(),
                    &row.0,
                )?,
                occurrence_key: row.1,
                ordinal: u32::try_from(row.2).map_err(|_| StorageError::StoredIntegerOutOfRange)?,
                weekday: row.3,
                start_minute: row.4.map(u16::try_from).transpose().map_err(|_| {
                    StorageError::InvalidStoredProjection {
                        table: "catalog_occurrences",
                        field: "start_minute",
                        reason: "is outside the supported minute range",
                    }
                })?,
                end_minute: row.5.map(u16::try_from).transpose().map_err(|_| {
                    StorageError::InvalidStoredProjection {
                        table: "catalog_occurrences",
                        field: "end_minute",
                        reason: "is outside the supported minute range",
                    }
                })?,
                time_knowledge: row.6,
                requiredness: row.7,
                occurrence_kind: row.8,
                modality: row.9,
                synchronicity: row.10,
                evidence: row.11,
                normalization_reason: row.12,
                location: row.13,
                building: row.14,
                room: row.15,
                raw_sha256: row.16,
                canonical_facts: serde_json::from_str(&row.17)?,
            };
            if validate_occurrence_time_projection(&occurrence).is_err() {
                return Err(StorageError::InvalidStoredProjection {
                    table: "catalog_occurrences",
                    field: "time projection",
                    reason: "is inconsistent with time knowledge",
                });
            }
            Ok(occurrence)
        })
        .collect::<StorageResult<Vec<_>>>()?;

    let mut provenance_statement = connection.prepare(
        "SELECT entity_kind, entity_key, field_name, source_sha256, source_ordinal,
                detail_json, content_version
         FROM catalog_provenance WHERE target_id = ?1
         ORDER BY entity_kind, entity_key, field_name, source_ordinal",
    )?;
    let provenance = provenance_statement
        .query_map([&target_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })?
        .map(|row| {
            let row = row?;
            require_published_version("provenance", row.6, content_version)?;
            stored_sha256("catalog_provenance", "source_sha256", &row.3)?;
            Ok(StoredProvenance {
                entity_kind: parse_provenance_kind(&row.0)?,
                entity_key: row.1,
                field_name: row.2,
                source_sha256: row.3,
                source_ordinal: u32::try_from(row.4)
                    .map_err(|_| StorageError::StoredIntegerOutOfRange)?,
                detail: serde_json::from_str(&row.5)?,
            })
        })
        .collect::<StorageResult<Vec<_>>>()?;

    let snapshot = CatalogSnapshot {
        course_groups,
        course_variants,
        sections,
        occurrences,
        provenance,
    };
    validate_loaded_catalog_ownership(&snapshot)?;
    Ok(snapshot)
}

fn validate_loaded_catalog_ownership(snapshot: &CatalogSnapshot) -> StorageResult<()> {
    let groups = snapshot
        .course_groups
        .iter()
        .map(|group| group.key.clone())
        .collect::<BTreeSet<_>>();
    let variants = snapshot
        .course_variants
        .iter()
        .map(|variant| variant.key.clone())
        .collect::<BTreeSet<_>>();
    let sections = snapshot
        .sections
        .iter()
        .map(|section| section.key.clone())
        .collect::<BTreeSet<_>>();
    for variant in &snapshot.course_variants {
        if !groups.contains(variant.key.group()) {
            return Err(StorageError::OrphanedStoredEntity {
                entity_kind: "course variant",
            });
        }
    }
    for section in &snapshot.sections {
        if !variants.contains(&section.variant_key) {
            return Err(StorageError::OrphanedStoredEntity {
                entity_kind: "section",
            });
        }
    }
    for occurrence in &snapshot.occurrences {
        if !sections.contains(&occurrence.section_key) {
            return Err(StorageError::OrphanedStoredEntity {
                entity_kind: "occurrence",
            });
        }
    }
    let group_entities = snapshot
        .course_groups
        .iter()
        .map(|group| {
            format!(
                "{}/{}/{}",
                group.key.term(),
                group.key.campus(),
                group.key.course_string()
            )
        })
        .collect::<BTreeSet<_>>();
    let variant_entities = snapshot
        .course_variants
        .iter()
        .map(|variant| {
            format!(
                "{}/{}/{}/{}",
                variant.key.group().term(),
                variant.key.group().campus(),
                variant.key.group().course_string(),
                variant.key.fingerprint()
            )
        })
        .collect::<BTreeSet<_>>();
    let section_entities = snapshot
        .sections
        .iter()
        .map(|section| section.key.to_string())
        .collect::<BTreeSet<_>>();
    let occurrence_entities = snapshot
        .occurrences
        .iter()
        .map(|occurrence| format!("{}/{}", occurrence.section_key, occurrence.occurrence_key))
        .collect::<BTreeSet<_>>();
    for provenance in &snapshot.provenance {
        let owned = match provenance.entity_kind {
            ProvenanceEntityKind::CourseGroup => group_entities.contains(&provenance.entity_key),
            ProvenanceEntityKind::CourseVariant => {
                variant_entities.contains(&provenance.entity_key)
            }
            ProvenanceEntityKind::Section => section_entities.contains(&provenance.entity_key),
            ProvenanceEntityKind::Occurrence => {
                occurrence_entities.contains(&provenance.entity_key)
            }
        };
        if !owned {
            return Err(StorageError::OrphanedStoredEntity {
                entity_kind: "provenance",
            });
        }
    }
    Ok(())
}

fn prepare_catalog_refresh(
    mut command: CatalogRefreshCommand,
) -> StorageResult<PreparedCatalogRefresh> {
    validate_timestamp("started_at", &command.started_at)?;
    validate_timestamp("completed_at", &command.completed_at)?;
    validate_timestamp_order(&command.started_at, &command.completed_at)?;
    validate_sha256("source_content_sha256", &command.source_content_sha256)?;
    validate_sha256("semantic_content_sha256", &command.semantic_content_sha256)?;
    u64_to_i64(command.source_bytes)?;
    if let Some(payload) = &command.raw_payload {
        validate_sha256("raw_payload.sha256", &payload.sha256)?;
        if payload.sha256 != command.source_content_sha256
            || sha256_hex(&payload.bytes) != payload.sha256
            || payload.bytes.len() as u64 != command.source_bytes
        {
            return Err(StorageError::InvalidCommand {
                field: "raw_payload",
                reason: "payload byte count and SHA-256 must match the source metadata",
            });
        }
    }
    canonicalize_and_validate_snapshot(&command.target, &mut command.snapshot)?;
    let recomputed = catalog_content_sha256_v1(&command.target, &command.snapshot)?;
    if recomputed != command.semantic_content_sha256 {
        return Err(StorageError::InvalidCommand {
            field: "semantic_content_sha256",
            reason: "does not match the catalog-content-v1 projection",
        });
    }
    Ok(PreparedCatalogRefresh {
        semantic_sha256: command.semantic_content_sha256.clone(),
        command,
    })
}

pub fn catalog_content_sha256_v1(
    target: &TermCampusKey,
    snapshot: &CatalogSnapshot,
) -> StorageResult<String> {
    let mut variants_by_group = BTreeMap::new();
    for variant in &snapshot.course_variants {
        variants_by_group
            .entry(variant.key.group().clone())
            .or_insert_with(Vec::new)
            .push(variant);
    }
    for variants in variants_by_group.values_mut() {
        variants.sort_by(|left, right| left.key.cmp(&right.key));
    }
    let mut sections_by_variant = BTreeMap::new();
    for section in &snapshot.sections {
        sections_by_variant
            .entry(section.variant_key.clone())
            .or_insert_with(Vec::new)
            .push(section);
    }
    for sections in sections_by_variant.values_mut() {
        sections.sort_by(|left, right| left.key.cmp(&right.key));
    }

    let mut group_rows = snapshot.course_groups.iter().collect::<Vec<_>>();
    group_rows.sort_by(|left, right| left.key.cmp(&right.key));
    let groups = group_rows
        .into_iter()
        .map(|group| {
            let variants = variants_by_group
                .get(&group.key)
                .into_iter()
                .flatten()
                .map(|variant| {
                    let sections = sections_by_variant
                        .get(&variant.key)
                        .into_iter()
                        .flatten()
                        .map(|section| {
                            serde_json::json!({
                                "term": section.key.term().as_str(),
                                "campus": section.key.campus().as_str(),
                                "index": section.key.index().as_str(),
                                "semanticHash": section.canonical_sha256,
                            })
                        })
                        .collect::<Vec<_>>();
                    serde_json::json!({
                        "fingerprint": variant.key.fingerprint().as_str(),
                        "canonicalHash": variant.canonical_sha256,
                        "sections": sections,
                    })
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "courseString": group.key.course_string().as_str(),
                "variants": variants,
            })
        })
        .collect::<Vec<_>>();
    let value = serde_json::json!({
        "schema": "catalog-content-v1",
        "target": {
            "term": target.term().as_str(),
            "campus": target.campus().as_str(),
        },
        "groups": groups,
    });
    let canonical_json = serde_json::to_vec(&value)?;
    let mut preimage =
        Vec::with_capacity(CATALOG_TARGET_CONTENT_PREIMAGE_V1.len() + canonical_json.len());
    preimage.extend_from_slice(CATALOG_TARGET_CONTENT_PREIMAGE_V1);
    preimage.extend_from_slice(&canonical_json);
    Ok(sha256_hex(&preimage))
}

const CATALOG_TARGET_CONTENT_PREIMAGE_V1: &[u8] = b"bcsp-catalog/target-content/v1\0";

const TIME_KNOWLEDGE_WIRE_VALUES: &[&str] = &[
    "MISSING",
    "EXPLICIT_NULL",
    "EMPTY",
    "PARTIAL",
    "INVALID",
    "KNOWN",
];
const REQUIREDNESS_WIRE_VALUES: &[&str] = &["REQUIRED", "OPTIONAL", "UNKNOWN_REQUIREDNESS"];
const OCCURRENCE_KIND_WIRE_VALUES: &[&str] = &["SCHEDULED", "BY_ARRANGEMENT", "UNSPECIFIED"];
const OCCURRENCE_EVIDENCE_WIRE_VALUES: &[&str] = &["NONE", "PHYSICAL", "REMOTE"];
const MODALITY_WIRE_VALUES: &[&str] = &[
    "ON_CAMPUS_OR_IN_PERSON",
    "HYBRID",
    "ONLINE",
    "OTHER",
    "UNKNOWN",
    "UNKNOWN_CONFLICT",
];
const SYNCHRONICITY_WIRE_VALUES: &[&str] = &["SYNC", "ASYNC", "UNSPECIFIED", "UNKNOWN", "MIXED"];

fn validate_wire_token(field: &'static str, value: &str, allowed: &[&str]) -> StorageResult<()> {
    if !allowed.contains(&value) {
        return Err(StorageError::InvalidCommand {
            field,
            reason: "must be an exact shared-contract wire value",
        });
    }
    Ok(())
}

fn validate_occurrence_time_projection(occurrence: &crate::StoredOccurrence) -> StorageResult<()> {
    match (
        occurrence.time_knowledge.as_str(),
        occurrence.start_minute,
        occurrence.end_minute,
    ) {
        ("KNOWN", Some(start), Some(end)) if start < 1_440 && end <= 1_440 && end > start => Ok(()),
        ("KNOWN", _, _) => Err(StorageError::InvalidCommand {
            field: "occurrence.time_knowledge",
            reason: "KNOWN requires an ordered projected minute range within one day",
        }),
        (_, None, None) => Ok(()),
        _ => Err(StorageError::InvalidCommand {
            field: "occurrence.time_knowledge",
            reason: "non-KNOWN time states must not expose minute projections",
        }),
    }
}

fn canonicalize_and_validate_snapshot(
    target: &TermCampusKey,
    snapshot: &mut CatalogSnapshot,
) -> StorageResult<()> {
    snapshot
        .course_groups
        .sort_by(|left, right| left.key.cmp(&right.key));
    snapshot
        .course_variants
        .sort_by(|left, right| left.key.cmp(&right.key));
    snapshot
        .sections
        .sort_by(|left, right| left.key.cmp(&right.key));
    snapshot.occurrences.sort_by(|left, right| {
        (&left.section_key, &left.occurrence_key).cmp(&(&right.section_key, &right.occurrence_key))
    });
    snapshot.provenance.sort_by(|left, right| {
        (
            left.entity_kind,
            &left.entity_key,
            &left.field_name,
            left.source_ordinal,
        )
            .cmp(&(
                right.entity_kind,
                &right.entity_key,
                &right.field_name,
                right.source_ordinal,
            ))
    });

    let mut group_keys = BTreeSet::new();
    for group in &snapshot.course_groups {
        validate_group_target(target, &group.key)?;
        serde_json::to_string(&group.canonical_facts)?;
        if !group_keys.insert(group.key.clone()) {
            return Err(duplicate_error("course_groups"));
        }
    }

    let mut variant_keys = BTreeSet::new();
    for variant in &snapshot.course_variants {
        validate_group_target(target, variant.key.group())?;
        if !group_keys.contains(variant.key.group()) {
            return Err(orphan_error("course_variants"));
        }
        validate_sha256("variant.canonical_sha256", &variant.canonical_sha256)?;
        validate_optional_text("subject_code", variant.subject_code.as_deref(), 128)?;
        validate_optional_text("course_number", variant.course_number.as_deref(), 128)?;
        validate_optional_text("title", variant.title.as_deref(), 4096)?;
        validate_optional_text("description", variant.description.as_deref(), 65_536)?;
        validate_text(
            "variant.search_document",
            &variant.search_document,
            262_144,
            true,
        )?;
        serde_json::to_string(&variant.canonical_facts)?;
        if variant.raw_multiplicity == 0 {
            return Err(StorageError::InvalidCommand {
                field: "variant.raw_multiplicity",
                reason: "must be at least one",
            });
        }
        if !variant_keys.insert(variant.key.clone()) {
            return Err(duplicate_error("course_variants"));
        }
    }

    let mut section_keys = BTreeSet::new();
    for section in &snapshot.sections {
        validate_section_target(target, &section.key)?;
        validate_group_target(target, section.variant_key.group())?;
        if !variant_keys.contains(&section.variant_key) {
            return Err(orphan_error("sections"));
        }
        validate_wire_token(
            "section.delivery_modality",
            &section.delivery_modality,
            MODALITY_WIRE_VALUES,
        )?;
        validate_wire_token(
            "section.synchronicity",
            &section.synchronicity,
            SYNCHRONICITY_WIRE_VALUES,
        )?;
        validate_sha256("section.canonical_sha256", &section.canonical_sha256)?;
        serde_json::to_string(&section.canonical_facts)?;
        if !section_keys.insert(section.key.clone()) {
            return Err(duplicate_error("sections"));
        }
    }

    let mut occurrence_keys = BTreeSet::new();
    for occurrence in &snapshot.occurrences {
        validate_section_target(target, &occurrence.section_key)?;
        if !section_keys.contains(&occurrence.section_key) {
            return Err(orphan_error("occurrences"));
        }
        validate_identifier("occurrence_key", &occurrence.occurrence_key, 256)?;
        validate_sha256("occurrence.raw_sha256", &occurrence.raw_sha256)?;
        validate_wire_token(
            "occurrence.time_knowledge",
            &occurrence.time_knowledge,
            TIME_KNOWLEDGE_WIRE_VALUES,
        )?;
        validate_wire_token(
            "occurrence.requiredness",
            &occurrence.requiredness,
            REQUIREDNESS_WIRE_VALUES,
        )?;
        validate_wire_token(
            "occurrence.occurrence_kind",
            &occurrence.occurrence_kind,
            OCCURRENCE_KIND_WIRE_VALUES,
        )?;
        validate_wire_token(
            "occurrence.evidence",
            &occurrence.evidence,
            OCCURRENCE_EVIDENCE_WIRE_VALUES,
        )?;
        validate_safe_code(
            "occurrence.normalization_reason",
            &occurrence.normalization_reason,
        )?;
        validate_wire_token(
            "occurrence.modality",
            &occurrence.modality,
            MODALITY_WIRE_VALUES,
        )?;
        validate_wire_token(
            "occurrence.synchronicity",
            &occurrence.synchronicity,
            SYNCHRONICITY_WIRE_VALUES,
        )?;
        validate_occurrence_time_projection(occurrence)?;
        serde_json::to_string(&occurrence.canonical_facts)?;
        let key = (
            occurrence.section_key.clone(),
            occurrence.occurrence_key.clone(),
        );
        if !occurrence_keys.insert(key) {
            return Err(duplicate_error("occurrences"));
        }
    }

    let mut provenance_keys = BTreeSet::new();
    for provenance in &snapshot.provenance {
        validate_identifier("provenance.entity_key", &provenance.entity_key, 512)?;
        validate_identifier("provenance.field_name", &provenance.field_name, 256)?;
        validate_sha256("provenance.source_sha256", &provenance.source_sha256)?;
        serde_json::to_string(&provenance.detail)?;
        let key = (
            provenance.entity_kind,
            provenance.entity_key.clone(),
            provenance.field_name.clone(),
            provenance.source_ordinal,
        );
        if !provenance_keys.insert(key) {
            return Err(duplicate_error("provenance"));
        }
    }
    Ok(())
}

fn stage_prepared_catalog_refresh(
    transaction: &Transaction<'_>,
    prepared: &PreparedCatalogRefresh,
) -> StorageResult<()> {
    let command = &prepared.command;
    let observation_id = command.observation_id.to_string();
    let target_id = target_id(&command.target);
    let counts = command.snapshot.counts();
    let persisted_started_at = transaction
        .query_row(
            "SELECT started_at FROM catalog_refresh_observations
             WHERE observation_id = ?1 AND target_id = ?2 AND status = 'STARTED'",
            params![observation_id, target_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .ok_or(StorageError::ObservationNotStarted(command.observation_id))?;
    validate_timestamp_order(&persisted_started_at, &command.completed_at)?;
    clear_staging(transaction, command.observation_id)?;
    let updated = transaction.execute(
        "UPDATE catalog_refresh_observations
         SET status = 'STAGED', completed_at = ?2,
             source_content_sha256 = ?3, semantic_sha256 = ?4, source_bytes = ?5,
             group_count = ?6, variant_count = ?7, section_count = ?8,
             occurrence_count = ?9, changed = NULL, resulting_content_version = NULL,
             retained_observation_id = NULL, error_stage = NULL, error_code = NULL,
             diagnostic_token = NULL
         WHERE observation_id = ?1 AND target_id = ?10 AND status = 'STARTED'",
        params![
            observation_id,
            command.completed_at,
            command.source_content_sha256,
            prepared.semantic_sha256,
            u64_to_i64(command.source_bytes)?,
            u64_to_i64(counts.course_groups)?,
            u64_to_i64(counts.course_variants)?,
            u64_to_i64(counts.sections)?,
            u64_to_i64(counts.occurrences)?,
            target_id,
        ],
    )?;
    if updated != 1 {
        return Err(StorageError::ObservationNotStarted(command.observation_id));
    }

    if let Some(payload) = &command.raw_payload {
        transaction.execute(
            "INSERT INTO catalog_staging_payloads
                (observation_id, target_id, sha256, byte_count, encoded_body)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                observation_id,
                target_id,
                payload.sha256,
                u64_to_i64(payload.bytes.len() as u64)?,
                payload.bytes,
            ],
        )?;
    }

    for group in &command.snapshot.course_groups {
        let canonical_facts_json = serde_json::to_string(&group.canonical_facts)?;
        transaction.execute(
            "INSERT INTO catalog_staging_course_groups
                (observation_id, target_id, course_string, canonical_facts_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                observation_id,
                target_id,
                group.key.course_string().as_str(),
                canonical_facts_json,
            ],
        )?;
    }
    for variant in &command.snapshot.course_variants {
        let canonical_facts_json = serde_json::to_string(&variant.canonical_facts)?;
        transaction.execute(
            "INSERT INTO catalog_staging_course_variants
                (observation_id, target_id, course_string, fingerprint,
                 subject_code, course_number, title, description, credits_summary,
                 supplement, search_document, canonical_sha256, raw_multiplicity,
                 canonical_facts_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                observation_id,
                target_id,
                variant.key.group().course_string().as_str(),
                variant.key.fingerprint().as_str(),
                &variant.subject_code,
                &variant.course_number,
                &variant.title,
                &variant.description,
                &variant.credits_summary,
                &variant.supplement,
                &variant.search_document,
                &variant.canonical_sha256,
                i64::from(variant.raw_multiplicity),
                canonical_facts_json,
            ],
        )?;
    }
    for section in &command.snapshot.sections {
        let canonical_facts_json = serde_json::to_string(&section.canonical_facts)?;
        transaction.execute(
            "INSERT INTO catalog_staging_sections
                (observation_id, target_id, section_index, course_string, fingerprint,
                 section_number, catalog_status, section_course_type, delivery_modality,
                 synchronicity, canonical_facts_json, canonical_sha256)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                observation_id,
                target_id,
                section.key.index().as_str(),
                section.variant_key.group().course_string().as_str(),
                section.variant_key.fingerprint().as_str(),
                &section.section_number,
                &section.catalog_status,
                &section.section_course_type,
                &section.delivery_modality,
                &section.synchronicity,
                canonical_facts_json,
                &section.canonical_sha256,
            ],
        )?;
    }
    for occurrence in &command.snapshot.occurrences {
        let canonical_facts_json = serde_json::to_string(&occurrence.canonical_facts)?;
        transaction.execute(
            "INSERT INTO catalog_staging_occurrences
                (observation_id, target_id, section_index, occurrence_key, ordinal,
                 weekday, start_minute, end_minute, time_knowledge, requiredness,
                 occurrence_kind, modality, synchronicity, evidence, normalization_reason,
                 location, building, room, raw_sha256, canonical_facts_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                     ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                observation_id,
                target_id,
                occurrence.section_key.index().as_str(),
                &occurrence.occurrence_key,
                i64::from(occurrence.ordinal),
                &occurrence.weekday,
                occurrence.start_minute.map(i64::from),
                occurrence.end_minute.map(i64::from),
                &occurrence.time_knowledge,
                &occurrence.requiredness,
                &occurrence.occurrence_kind,
                &occurrence.modality,
                &occurrence.synchronicity,
                &occurrence.evidence,
                &occurrence.normalization_reason,
                &occurrence.location,
                &occurrence.building,
                &occurrence.room,
                &occurrence.raw_sha256,
                canonical_facts_json,
            ],
        )?;
    }
    for provenance in &command.snapshot.provenance {
        let detail_json = serde_json::to_string(&provenance.detail)?;
        transaction.execute(
            "INSERT INTO catalog_staging_provenance
                (observation_id, target_id, entity_kind, entity_key, field_name,
                 source_sha256, source_ordinal, detail_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                observation_id,
                target_id,
                provenance.entity_kind.as_str(),
                &provenance.entity_key,
                &provenance.field_name,
                &provenance.source_sha256,
                i64::from(provenance.source_ordinal),
                detail_json,
            ],
        )?;
    }
    Ok(())
}

fn publish_staged(
    transaction: &Transaction<'_>,
    observation_id: TraceId,
    empty_decision: EmptySnapshotDecision,
    fault: PublishFaultPoint,
) -> StorageResult<PublishOutcome> {
    let observation_id_text = observation_id.to_string();
    let metadata = load_staged_metadata(transaction, observation_id)?;
    let target_row = transaction.query_row(
        "SELECT current_content_version, accepted_semantic_hash,
                group_count, variant_count, section_count, occurrence_count,
                last_published_observation_id
         FROM catalog_targets WHERE target_id = ?1",
        [&metadata.target_id],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, Option<String>>(6)?,
            ))
        },
    )?;
    let current_version = i64_to_u64(target_row.0)?;
    let retained_counts = CatalogCounts {
        course_groups: i64_to_u64(target_row.2)?,
        course_variants: i64_to_u64(target_row.3)?,
        sections: i64_to_u64(target_row.4)?,
        occurrences: i64_to_u64(target_row.5)?,
    };
    let incoming_empty = metadata.counts == CatalogCounts::default();
    let baseline_nonempty = retained_counts != CatalogCounts::default();

    if incoming_empty && current_version > 0 && baseline_nonempty {
        transaction.execute(
            "UPDATE catalog_refresh_observations
             SET status = 'EMPTY_SUSPECT_RETAINED', changed = 0,
                 resulting_content_version = ?2, retained_observation_id = ?3
             WHERE observation_id = ?1",
            params![
                observation_id_text,
                u64_to_i64(current_version)?,
                target_row.6
            ],
        )?;
        transaction.execute(
            "UPDATE catalog_targets
             SET updated_at = ?2, last_success_observation_id = ?1,
                 pending_empty_observation_id = ?1
             WHERE target_id = ?3",
            params![
                observation_id_text,
                metadata.completed_at,
                metadata.target_id
            ],
        )?;
        clear_staging(transaction, observation_id)?;
        return Ok(PublishOutcome::SuspectEmptyRetained {
            content_version: current_version,
            retained_observation_id: parse_optional_stored_trace_id(
                "catalog_targets",
                "last_published_observation_id",
                target_row.6,
            )?,
            retained_counts,
        });
    }

    if !incoming_empty && empty_decision != EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty {
        return Err(StorageError::InvalidCommand {
            field: "empty_decision",
            reason: "non-empty snapshots require the non-empty publication decision",
        });
    }

    let changed =
        current_version == 0 || target_row.1.as_deref() != Some(metadata.semantic_sha256.as_str());
    let initial_selector_membership_requested = incoming_empty
        && current_version == 0
        && empty_decision
            == EmptySnapshotDecision::AcceptInitialSelectorConfirmedEmpty(
                crate::InitialEmptyProof::CurrentSelectorMembership,
            );
    let initial_valid_empty = initial_selector_membership_requested
        && current_selector_contains_target(transaction, &metadata.target_id)?;
    if incoming_empty && current_version == 0 && !initial_valid_empty {
        return Err(StorageError::InvalidCommand {
            field: "empty_decision",
            reason: "a first valid empty requires current persisted selector membership",
        });
    }
    if incoming_empty
        && current_version > 0
        && !baseline_nonempty
        && empty_decision != EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty
    {
        return Err(StorageError::InvalidCommand {
            field: "empty_decision",
            reason: "an established empty target accepts only an unchanged empty decision",
        });
    }
    if incoming_empty && current_version > 0 && changed {
        return Err(StorageError::InvalidCommand {
            field: "empty_decision",
            reason: "an established empty target accepts only an unchanged empty snapshot",
        });
    }
    if !changed {
        write_checkpoint(transaction, observation_id, &metadata, current_version)?;
        transaction.execute(
            "UPDATE catalog_refresh_observations
             SET status = 'APPLIED_UNCHANGED', changed = 0,
                 resulting_content_version = ?2
             WHERE observation_id = ?1",
            params![observation_id_text, u64_to_i64(current_version)?],
        )?;
        update_target_after_publish(transaction, observation_id, &metadata, current_version)?;
        clear_staging(transaction, observation_id)?;
        return Ok(PublishOutcome::AppliedUnchanged {
            content_version: current_version,
            counts: metadata.counts,
        });
    }

    let next_version = current_version
        .checked_add(1)
        .ok_or(StorageError::StoredIntegerOutOfRange)?;
    transaction.execute(
        "DELETE FROM catalog_course_fts WHERE target_id = ?1",
        [&metadata.target_id],
    )?;
    transaction.execute(
        "DELETE FROM catalog_provenance WHERE target_id = ?1",
        [&metadata.target_id],
    )?;
    transaction.execute(
        "DELETE FROM catalog_course_groups WHERE target_id = ?1",
        [&metadata.target_id],
    )?;
    if fault == PublishFaultPoint::AfterServingDelete {
        return Err(StorageError::InjectedFault("AFTER_SERVING_DELETE"));
    }
    insert_serving_rows(transaction, observation_id, next_version)?;
    verify_fts_integrity(transaction, &metadata.target_id)?;
    write_checkpoint(transaction, observation_id, &metadata, next_version)?;
    transaction.execute(
        "UPDATE catalog_refresh_observations
         SET status = ?3, changed = 1,
             resulting_content_version = ?2
         WHERE observation_id = ?1",
        params![
            observation_id_text,
            u64_to_i64(next_version)?,
            if initial_valid_empty {
                "EMPTY_VALID_INITIAL"
            } else {
                "APPLIED_CHANGED"
            },
        ],
    )?;
    update_target_after_publish(transaction, observation_id, &metadata, next_version)?;
    clear_staging(transaction, observation_id)?;
    if fault == PublishFaultPoint::BeforeCommit {
        return Err(StorageError::InjectedFault("BEFORE_COMMIT"));
    }
    if initial_valid_empty {
        Ok(PublishOutcome::InitialValidEmpty {
            content_version: next_version,
        })
    } else {
        Ok(PublishOutcome::AppliedChanged {
            content_version: next_version,
            counts: metadata.counts,
        })
    }
}

fn current_selector_contains_target(
    transaction: &Transaction<'_>,
    target_id: &str,
) -> StorageResult<bool> {
    transaction
        .query_row(
            "SELECT EXISTS(
                 SELECT 1
                 FROM catalog_targets t
                 JOIN catalog_campuses c
                   ON c.term_id = t.term_id AND c.campus_code = t.campus_code
                 JOIN catalog_discovery_source_versions s
                   ON s.source_version_id = c.source_version_id
                 WHERE t.target_id = ?1 AND s.source_kind = 'SELECTOR'
             )",
            [target_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(Into::into)
}

fn load_staged_metadata(
    transaction: &Transaction<'_>,
    observation_id: TraceId,
) -> StorageResult<StagedMetadata> {
    let observation_id_text = observation_id.to_string();
    let row = transaction
        .query_row(
            "SELECT target_id, semantic_sha256, completed_at,
                    group_count, variant_count, section_count, occurrence_count
             FROM catalog_refresh_observations
             WHERE observation_id = ?1 AND status = 'STAGED'",
            [&observation_id_text],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()?
        .ok_or(StorageError::ObservationNotStaged(observation_id))?;
    Ok(StagedMetadata {
        target_id: row.0,
        semantic_sha256: row.1,
        completed_at: row.2,
        counts: CatalogCounts {
            course_groups: i64_to_u64(row.3)?,
            course_variants: i64_to_u64(row.4)?,
            sections: i64_to_u64(row.5)?,
            occurrences: i64_to_u64(row.6)?,
        },
    })
}

fn insert_serving_rows(
    transaction: &Transaction<'_>,
    observation_id: TraceId,
    content_version: u64,
) -> StorageResult<()> {
    let observation_id = observation_id.to_string();
    let version = u64_to_i64(content_version)?;
    transaction.execute(
        "INSERT INTO catalog_course_groups
            (target_id, course_string, canonical_facts_json, content_version)
         SELECT target_id, course_string, canonical_facts_json, ?2
         FROM catalog_staging_course_groups WHERE observation_id = ?1",
        params![observation_id, version],
    )?;
    transaction.execute(
        "INSERT INTO catalog_course_variants
            (target_id, course_string, fingerprint, subject_code, course_number,
             title, description, credits_summary, supplement, search_document,
             canonical_sha256, raw_multiplicity, canonical_facts_json, content_version)
         SELECT target_id, course_string, fingerprint, subject_code, course_number,
                title, description, credits_summary, supplement, search_document,
                canonical_sha256, raw_multiplicity, canonical_facts_json, ?2
         FROM catalog_staging_course_variants WHERE observation_id = ?1",
        params![observation_id, version],
    )?;
    transaction.execute(
        "INSERT INTO catalog_sections
            (target_id, section_index, course_string, fingerprint, section_number,
             catalog_status, section_course_type, delivery_modality, synchronicity,
             canonical_facts_json, canonical_sha256, content_version)
         SELECT target_id, section_index, course_string, fingerprint, section_number,
                catalog_status, section_course_type, delivery_modality, synchronicity,
                canonical_facts_json, canonical_sha256, ?2
         FROM catalog_staging_sections WHERE observation_id = ?1",
        params![observation_id, version],
    )?;
    transaction.execute(
        "INSERT INTO catalog_occurrences
            (target_id, section_index, occurrence_key, ordinal, weekday, start_minute,
             end_minute, time_knowledge, requiredness, occurrence_kind, modality,
             synchronicity, evidence, normalization_reason, location, building, room,
             raw_sha256, canonical_facts_json, content_version)
         SELECT target_id, section_index, occurrence_key, ordinal, weekday, start_minute,
                end_minute, time_knowledge, requiredness, occurrence_kind, modality,
                synchronicity, evidence, normalization_reason, location, building, room,
                raw_sha256, canonical_facts_json, ?2
         FROM catalog_staging_occurrences WHERE observation_id = ?1",
        params![observation_id, version],
    )?;
    transaction.execute(
        "INSERT INTO catalog_provenance
            (target_id, entity_kind, entity_key, field_name, source_sha256,
             source_ordinal, detail_json, content_version)
         SELECT target_id, entity_kind, entity_key, field_name, source_sha256,
                source_ordinal, detail_json, ?2
         FROM catalog_staging_provenance WHERE observation_id = ?1",
        params![observation_id, version],
    )?;
    transaction.execute(
        "INSERT INTO catalog_course_fts
            (target_id, course_string, fingerprint, content_version, document)
         SELECT target_id, course_string, fingerprint, ?2, search_document
         FROM catalog_staging_course_variants WHERE observation_id = ?1",
        params![observation_id, version],
    )?;
    Ok(())
}

fn write_checkpoint(
    transaction: &Transaction<'_>,
    observation_id: TraceId,
    metadata: &StagedMetadata,
    content_version: u64,
) -> StorageResult<()> {
    let observation_id = observation_id.to_string();
    transaction.execute(
        "INSERT INTO catalog_refresh_checkpoints
            (target_id, content_version, semantic_sha256, observation_id, accepted_at,
             group_count, variant_count, section_count, occurrence_count)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
         ON CONFLICT(target_id) DO UPDATE SET
             content_version = excluded.content_version,
             semantic_sha256 = excluded.semantic_sha256,
             observation_id = excluded.observation_id,
             accepted_at = excluded.accepted_at,
             group_count = excluded.group_count,
             variant_count = excluded.variant_count,
             section_count = excluded.section_count,
             occurrence_count = excluded.occurrence_count",
        params![
            metadata.target_id,
            u64_to_i64(content_version)?,
            metadata.semantic_sha256,
            observation_id,
            metadata.completed_at,
            u64_to_i64(metadata.counts.course_groups)?,
            u64_to_i64(metadata.counts.course_variants)?,
            u64_to_i64(metadata.counts.sections)?,
            u64_to_i64(metadata.counts.occurrences)?,
        ],
    )?;
    Ok(())
}

fn update_target_after_publish(
    transaction: &Transaction<'_>,
    observation_id: TraceId,
    metadata: &StagedMetadata,
    content_version: u64,
) -> StorageResult<()> {
    let observation_id = observation_id.to_string();
    transaction.execute(
        "UPDATE catalog_targets
         SET updated_at = ?2,
             current_content_version = ?3,
             accepted_semantic_hash = ?4,
             group_count = ?5,
             variant_count = ?6,
             section_count = ?7,
             occurrence_count = ?8,
             last_success_observation_id = ?1,
             last_published_observation_id = ?1,
             last_nonempty_observation_id = CASE WHEN ?5 > 0
                 THEN ?1 ELSE last_nonempty_observation_id END,
             pending_empty_observation_id = NULL
         WHERE target_id = ?9",
        params![
            observation_id,
            metadata.completed_at,
            u64_to_i64(content_version)?,
            metadata.semantic_sha256,
            u64_to_i64(metadata.counts.course_groups)?,
            u64_to_i64(metadata.counts.course_variants)?,
            u64_to_i64(metadata.counts.sections)?,
            u64_to_i64(metadata.counts.occurrences)?,
            metadata.target_id,
        ],
    )?;
    Ok(())
}

fn verify_fts_integrity(transaction: &Transaction<'_>, target_id: &str) -> StorageResult<()> {
    let anomalies = transaction.query_row(
        "SELECT
            (SELECT count(*) FROM catalog_course_variants v
             WHERE v.target_id = ?1 AND NOT EXISTS (
                SELECT 1 FROM catalog_course_fts f
                WHERE f.target_id = v.target_id
                  AND f.course_string = v.course_string
                  AND f.fingerprint = v.fingerprint)),
            (SELECT count(*) FROM catalog_course_fts f
             WHERE f.target_id = ?1 AND NOT EXISTS (
                SELECT 1 FROM catalog_course_variants v
                WHERE v.target_id = f.target_id
                  AND v.course_string = f.course_string
                  AND v.fingerprint = f.fingerprint)),
            (SELECT count(*) FROM (
                SELECT 1 FROM catalog_course_fts
                WHERE target_id = ?1
                GROUP BY target_id, course_string, fingerprint
                HAVING count(*) > 1)),
            (SELECT count(*) FROM catalog_course_fts f
             JOIN catalog_course_variants v
               ON v.target_id = f.target_id
              AND v.course_string = f.course_string
              AND v.fingerprint = f.fingerprint
             WHERE f.target_id = ?1 AND f.content_version != v.content_version)",
        [target_id],
        |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        },
    )?;
    if anomalies != (0, 0, 0, 0) {
        return Err(StorageError::InvalidCommand {
            field: "catalog_course_fts",
            reason: "FTS rows do not have one-to-one parity with serving variants",
        });
    }
    Ok(())
}

fn ensure_target(
    transaction: &Transaction<'_>,
    target: &TermCampusKey,
    timestamp: &str,
) -> StorageResult<()> {
    transaction.execute(
        "INSERT INTO catalog_targets
            (target_id, term_id, campus_code, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?4)
         ON CONFLICT(target_id) DO NOTHING",
        params![
            target_id(target),
            target.term().as_str(),
            target.campus().as_str(),
            timestamp,
        ],
    )?;
    Ok(())
}

fn insert_or_update_started_observation(
    transaction: &Transaction<'_>,
    observation_id: TraceId,
    target: &TermCampusKey,
    started_at: &str,
    source_content_sha256: Option<&str>,
    source_bytes: Option<i64>,
) -> StorageResult<()> {
    let observation_id_text = observation_id.to_string();
    let expected_target_id = target_id(target);
    let existing = transaction
        .query_row(
            "SELECT target_id, status, attempt_sequence, started_at,
                    source_content_sha256, source_bytes
             FROM catalog_refresh_observations
             WHERE observation_id = ?1",
            [&observation_id_text],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<i64>>(5)?,
                ))
            },
        )
        .optional()?;
    match existing {
        None => {
            let updated = transaction.execute(
                "UPDATE catalog_targets
                 SET last_attempt_sequence = last_attempt_sequence + 1,
                     updated_at = ?2, last_attempt_observation_id = ?1
                 WHERE target_id = ?3",
                params![observation_id_text, started_at, expected_target_id],
            )?;
            if updated != 1 {
                return Err(StorageError::ObservationNotFound(observation_id));
            }
            let attempt_sequence = transaction.query_row(
                "SELECT last_attempt_sequence FROM catalog_targets WHERE target_id = ?1",
                [&expected_target_id],
                |row| row.get::<_, i64>(0),
            )?;
            if attempt_sequence <= 0 {
                return Err(StorageError::StoredIntegerOutOfRange);
            }
            transaction.execute(
                "INSERT INTO catalog_refresh_observations
                    (observation_id, target_id, attempt_sequence, started_at, status,
                     source_content_sha256, source_bytes)
                 VALUES (?1, ?2, ?3, ?4, 'STARTED', ?5, ?6)",
                params![
                    observation_id_text,
                    expected_target_id,
                    attempt_sequence,
                    started_at,
                    source_content_sha256,
                    source_bytes,
                ],
            )?;
        }
        Some((
            actual_target_id,
            status,
            attempt_sequence,
            persisted_started_at,
            stored_source_content_sha256,
            stored_source_bytes,
        )) => {
            if actual_target_id != expected_target_id || status != "STARTED" {
                return Err(StorageError::ObservationNotStarted(observation_id));
            }
            if attempt_sequence <= 0 {
                return Err(StorageError::StoredIntegerOutOfRange);
            }
            if persisted_started_at != started_at {
                return Err(StorageError::InvalidCommand {
                    field: "started_at",
                    reason: "must match the timestamp already stored for this observation",
                });
            }
            validate_source_metadata_compatibility(
                stored_source_content_sha256.as_deref(),
                source_content_sha256,
                stored_source_bytes,
                source_bytes,
            )?;
            transaction.execute(
                "UPDATE catalog_refresh_observations
                 SET source_content_sha256 = COALESCE(source_content_sha256, ?2),
                     source_bytes = COALESCE(source_bytes, ?3)
                 WHERE observation_id = ?1",
                params![observation_id_text, source_content_sha256, source_bytes],
            )?;
        }
    }
    Ok(())
}

fn validate_source_metadata_compatibility(
    stored_source_content_sha256: Option<&str>,
    proposed_source_content_sha256: Option<&str>,
    stored_source_bytes: Option<i64>,
    proposed_source_bytes: Option<i64>,
) -> StorageResult<()> {
    if matches!(
        (stored_source_content_sha256, proposed_source_content_sha256),
        (Some(stored), Some(proposed)) if stored != proposed
    ) {
        return Err(StorageError::InvalidCommand {
            field: "source_content_sha256",
            reason: "must match immutable metadata already stored for this observation",
        });
    }
    if matches!(
        (stored_source_bytes, proposed_source_bytes),
        (Some(stored), Some(proposed)) if stored != proposed
    ) {
        return Err(StorageError::InvalidCommand {
            field: "source_bytes",
            reason: "must match immutable metadata already stored for this observation",
        });
    }
    Ok(())
}

fn clear_staging(transaction: &Transaction<'_>, observation_id: TraceId) -> StorageResult<()> {
    let observation_id = observation_id.to_string();
    transaction.execute(
        "DELETE FROM catalog_staging_provenance WHERE observation_id = ?1",
        [&observation_id],
    )?;
    transaction.execute(
        "DELETE FROM catalog_staging_course_groups WHERE observation_id = ?1",
        [&observation_id],
    )?;
    transaction.execute(
        "DELETE FROM catalog_staging_payloads WHERE observation_id = ?1",
        [&observation_id],
    )?;
    Ok(())
}

fn recover_interrupted_refreshes(connection: &mut Connection) -> StorageResult<()> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    transaction.execute(
        "UPDATE catalog_discovery_observations
         SET status = 'FAILED',
             completed_at = COALESCE(completed_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
             error_stage = 'TRANSPORT', error_code = 'PROCESS_INTERRUPTED',
             diagnostic_token = NULL
         WHERE status = 'STARTED'",
        [],
    )?;
    transaction.execute(
        "DELETE FROM catalog_staging_provenance
         WHERE observation_id IN (
             SELECT observation_id FROM catalog_refresh_observations
             WHERE status IN ('STARTED', 'STAGED'))",
        [],
    )?;
    transaction.execute(
        "DELETE FROM catalog_staging_course_groups
         WHERE observation_id IN (
             SELECT observation_id FROM catalog_refresh_observations
             WHERE status IN ('STARTED', 'STAGED'))",
        [],
    )?;
    transaction.execute(
        "DELETE FROM catalog_staging_payloads
         WHERE observation_id IN (
             SELECT observation_id FROM catalog_refresh_observations
             WHERE status IN ('STARTED', 'STAGED'))",
        [],
    )?;
    transaction.execute(
        "UPDATE catalog_refresh_observations
         SET status = 'INTERRUPTED',
             completed_at = COALESCE(completed_at, strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
             error_stage = 'PUBLISH', error_code = 'PROCESS_INTERRUPTED',
             diagnostic_token = NULL
         WHERE status IN ('STARTED', 'STAGED')",
        [],
    )?;
    transaction.commit()?;
    Ok(())
}

fn target_id(target: &TermCampusKey) -> String {
    format!("{}/{}", target.term(), target.campus())
}

fn fts5_literal_and_query(tokens: &CourseTextSearchTokens) -> String {
    tokens
        .as_slice()
        .iter()
        .map(|token| format!("\"{}\"", token.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" AND ")
}

fn validate_group_target(target: &TermCampusKey, group: &CourseGroupKey) -> StorageResult<()> {
    if group.term() != target.term() || group.campus() != target.campus() {
        return Err(StorageError::InvalidCommand {
            field: "course_group.key",
            reason: "identity is outside the refresh target",
        });
    }
    Ok(())
}

fn validate_section_target(target: &TermCampusKey, section: &SectionKey) -> StorageResult<()> {
    if section.term() != target.term() || section.campus() != target.campus() {
        return Err(StorageError::InvalidCommand {
            field: "section.key",
            reason: "identity is outside the refresh target",
        });
    }
    Ok(())
}

fn duplicate_error(field: &'static str) -> StorageError {
    StorageError::InvalidCommand {
        field,
        reason: "contains a duplicate stable identity",
    }
}

fn orphan_error(field: &'static str) -> StorageError {
    StorageError::InvalidCommand {
        field,
        reason: "contains a row whose normalized parent is absent",
    }
}

pub(crate) fn validate_identifier(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> StorageResult<()> {
    if value.is_empty()
        || value.len() > max_bytes
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':' | b'/')
        })
    {
        return Err(StorageError::InvalidCommand {
            field,
            reason: "must be a bounded stable ASCII identifier",
        });
    }
    Ok(())
}

pub(crate) fn validate_safe_code(field: &'static str, value: &str) -> StorageResult<()> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Err(StorageError::InvalidCommand {
            field,
            reason: "must be an uppercase stable code of at most 64 bytes",
        });
    }
    Ok(())
}

pub(crate) fn validate_diagnostic_token(value: Option<&str>) -> StorageResult<()> {
    if let Some(value) = value
        && (value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':')
            }))
    {
        return Err(StorageError::InvalidCommand {
            field: "diagnostic_token",
            reason: "must be a bounded redacted token, never raw upstream detail",
        });
    }
    Ok(())
}

pub(crate) fn validate_text(
    field: &'static str,
    value: &str,
    max_bytes: usize,
    allow_empty: bool,
) -> StorageResult<()> {
    if (!allow_empty && value.is_empty()) || value.len() > max_bytes || value.contains('\0') {
        return Err(StorageError::InvalidCommand {
            field,
            reason: "text violates the empty, size, or NUL boundary",
        });
    }
    Ok(())
}

pub(crate) fn validate_optional_text(
    field: &'static str,
    value: Option<&str>,
    max_bytes: usize,
) -> StorageResult<()> {
    if let Some(value) = value {
        validate_text(field, value, max_bytes, true)?;
    }
    Ok(())
}

pub(crate) fn validate_sha256(field: &'static str, value: &str) -> StorageResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(StorageError::InvalidCommand {
            field,
            reason: "must be canonical lowercase SHA-256 hex",
        });
    }
    Ok(())
}

fn validate_optional_sha256(field: &'static str, value: Option<&str>) -> StorageResult<()> {
    if let Some(value) = value {
        validate_sha256(field, value)?;
    }
    Ok(())
}

pub(crate) fn validate_timestamp(field: &'static str, value: &str) -> StorageResult<()> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|_| StorageError::InvalidCommand {
        field,
        reason: "must be an RFC 3339 timestamp",
    })?;
    Ok(())
}

pub(crate) fn validate_timestamp_order(started_at: &str, completed_at: &str) -> StorageResult<()> {
    let started =
        OffsetDateTime::parse(started_at, &Rfc3339).map_err(|_| StorageError::InvalidCommand {
            field: "started_at",
            reason: "must be an RFC 3339 timestamp",
        })?;
    let completed = OffsetDateTime::parse(completed_at, &Rfc3339).map_err(|_| {
        StorageError::InvalidCommand {
            field: "completed_at",
            reason: "must be an RFC 3339 timestamp",
        }
    })?;
    if completed < started {
        return Err(StorageError::InvalidCommand {
            field: "completed_at",
            reason: "must not precede started_at",
        });
    }
    Ok(())
}

pub(crate) fn u64_to_i64(value: u64) -> StorageResult<i64> {
    i64::try_from(value).map_err(|_| StorageError::StoredIntegerOutOfRange)
}

fn optional_u64_to_i64(value: Option<u64>) -> StorageResult<Option<i64>> {
    value.map(u64_to_i64).transpose()
}

pub(crate) fn i64_to_u64(value: i64) -> StorageResult<u64> {
    u64::try_from(value).map_err(|_| StorageError::StoredIntegerOutOfRange)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn fts5_query_builder_quotes_every_literal_and_doubles_embedded_quotes() {
        let tokens = CourseTextSearchTokens::try_new(["alpha", "say\"hello", "OR"])
            .expect("valid literal tokens");
        assert_eq!(
            fts5_literal_and_query(&tokens),
            "\"alpha\" AND \"say\"\"hello\" AND \"OR\""
        );
    }

    #[test]
    fn file_backed_storage_uses_wal_for_serving_reads_during_a_write() {
        let temp = TempDir::new().expect("temporary database directory");
        let path = temp.path().join("operational.sqlite");
        let mut writer = OperationalStorage::open(&path).expect("writer storage");
        let reader = OperationalStorage::open(&path).expect("serving storage");

        let writer_mode = writer
            .connection
            .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))
            .expect("writer journal mode");
        let reader_mode = reader
            .connection
            .pragma_query_value(None, "journal_mode", |row| row.get::<_, String>(0))
            .expect("reader journal mode");
        assert!(writer_mode.eq_ignore_ascii_case("wal"));
        assert!(reader_mode.eq_ignore_ascii_case("wal"));

        let transaction = writer
            .connection
            .transaction_with_behavior(TransactionBehavior::Exclusive)
            .expect("exclusive writer transaction");
        transaction
            .execute(
                "UPDATE bcsp_operational_migrations SET name = name WHERE migration_id = 1",
                [],
            )
            .expect("hold a real write transaction");

        assert!(
            !reader
                .migration_records()
                .expect("serving read must not wait for writer commit")
                .is_empty()
        );
        transaction.rollback().expect("rollback synthetic write");
    }
}
