use bcsp_contracts::{
    CampusCode, CourseGroupKey, CourseVariantKey, SectionKey, TermCampusKey, TermId, TraceId,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const MAX_COURSE_TEXT_SEARCH_TOKENS: usize = 32;
const MAX_COURSE_TEXT_SEARCH_TOKEN_BYTES: usize = 128;
const MAX_COURSE_TEXT_SEARCH_BYTES: usize = 512;

/// Validated literal terms for a course full-text search.
///
/// This type deliberately cannot contain raw FTS5 query syntax. Storage quotes every item as an
/// FTS5 literal and combines the items with `AND` before executing `MATCH`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CourseTextSearchTokens {
    tokens: Vec<String>,
}

impl CourseTextSearchTokens {
    pub fn try_new<I, S>(tokens: I) -> crate::StorageResult<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut validated = Vec::new();
        let mut total_bytes = 0_usize;
        for token in tokens {
            if validated.len() == MAX_COURSE_TEXT_SEARCH_TOKENS {
                return Err(crate::StorageError::InvalidCommand {
                    field: "course_text_search_tokens",
                    reason: "must contain between 1 and 32 literal tokens",
                });
            }
            let token = token.as_ref().trim();
            if token.is_empty() {
                return Err(crate::StorageError::InvalidCommand {
                    field: "course_text_search_tokens",
                    reason: "tokens must be non-empty after trimming",
                });
            }
            if token.len() > MAX_COURSE_TEXT_SEARCH_TOKEN_BYTES {
                return Err(crate::StorageError::InvalidCommand {
                    field: "course_text_search_tokens",
                    reason: "each token must be at most 128 UTF-8 bytes",
                });
            }
            if token.chars().any(char::is_control) {
                return Err(crate::StorageError::InvalidCommand {
                    field: "course_text_search_tokens",
                    reason: "tokens must not contain control characters",
                });
            }
            if !token.chars().any(char::is_alphanumeric) {
                return Err(crate::StorageError::InvalidCommand {
                    field: "course_text_search_tokens",
                    reason: "tokens must contain at least one Unicode letter or number",
                });
            }
            total_bytes = total_bytes
                .checked_add(token.len())
                .ok_or(crate::StorageError::StoredIntegerOutOfRange)?;
            if total_bytes > MAX_COURSE_TEXT_SEARCH_BYTES {
                return Err(crate::StorageError::InvalidCommand {
                    field: "course_text_search_tokens",
                    reason: "combined token text must be at most 512 UTF-8 bytes",
                });
            }
            validated.push(token.to_owned());
        }
        if validated.is_empty() {
            return Err(crate::StorageError::InvalidCommand {
                field: "course_text_search_tokens",
                reason: "must contain between 1 and 32 literal tokens",
            });
        }
        Ok(Self { tokens: validated })
    }

    pub fn len(&self) -> usize {
        self.tokens.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tokens.is_empty()
    }

    pub fn as_slice(&self) -> &[String] {
        &self.tokens
    }
}

/// One immutable, version-bound source row for a prepared serving FTS index.
///
/// The document is exported only while constructing a serving snapshot. Product
/// requests query the prepared index and never return this source text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedCourseFtsDocument {
    pub key: CourseVariantKey,
    pub content_version: u64,
    pub document: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CourseFtsCorpusSignature {
    pub row_count: u64,
    pub document_bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CourseVariantSearchHit {
    pub key: CourseVariantKey,
    /// Zero-based position after FTS5 relevance ordering and stable identity tie-breaks.
    ///
    /// This is an ordinal, not a truncated BM25 score. The query layer can put an exact Rutgers
    /// identifier ahead of non-exact hits while retaining this order within each priority class.
    pub fts_rank: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CourseVariantSearchResult {
    pub target: TermCampusKey,
    pub content_version: u64,
    pub hits: Vec<CourseVariantSearchHit>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredCourseGroup {
    pub key: CourseGroupKey,
    pub canonical_facts: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredCourseVariant {
    pub key: CourseVariantKey,
    pub subject_code: Option<String>,
    pub course_number: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub credits_summary: Option<String>,
    pub supplement: Option<String>,
    pub search_document: String,
    pub canonical_sha256: String,
    pub raw_multiplicity: u32,
    pub canonical_facts: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredSection {
    pub key: SectionKey,
    pub variant_key: CourseVariantKey,
    pub section_number: Option<String>,
    pub catalog_status: Option<String>,
    pub section_course_type: Option<String>,
    pub delivery_modality: String,
    pub synchronicity: String,
    pub canonical_facts: Value,
    pub canonical_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredOccurrence {
    pub section_key: SectionKey,
    pub occurrence_key: String,
    pub ordinal: u32,
    pub weekday: Option<String>,
    pub start_minute: Option<u16>,
    pub end_minute: Option<u16>,
    pub time_knowledge: String,
    pub requiredness: String,
    pub occurrence_kind: String,
    pub evidence: String,
    pub normalization_reason: String,
    pub modality: String,
    pub synchronicity: String,
    pub location: Option<String>,
    pub building: Option<String>,
    pub room: Option<String>,
    pub raw_sha256: String,
    pub canonical_facts: Value,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ProvenanceEntityKind {
    CourseGroup,
    CourseVariant,
    Section,
    Occurrence,
}

impl ProvenanceEntityKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CourseGroup => "COURSE_GROUP",
            Self::CourseVariant => "COURSE_VARIANT",
            Self::Section => "SECTION",
            Self::Occurrence => "OCCURRENCE",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredProvenance {
    pub entity_kind: ProvenanceEntityKind,
    pub entity_key: String,
    pub field_name: String,
    pub source_sha256: String,
    pub source_ordinal: u32,
    pub detail: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StoredCanonicalFacts {
    pub entity_kind: ProvenanceEntityKind,
    pub entity_key: String,
    pub canonical_facts: Value,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogSnapshot {
    pub course_groups: Vec<StoredCourseGroup>,
    pub course_variants: Vec<StoredCourseVariant>,
    pub sections: Vec<StoredSection>,
    pub occurrences: Vec<StoredOccurrence>,
    pub provenance: Vec<StoredProvenance>,
}

impl CatalogSnapshot {
    pub fn is_empty(&self) -> bool {
        self.course_groups.is_empty()
            && self.course_variants.is_empty()
            && self.sections.is_empty()
            && self.occurrences.is_empty()
    }

    pub fn counts(&self) -> CatalogCounts {
        CatalogCounts {
            course_groups: self.course_groups.len() as u64,
            course_variants: self.course_variants.len() as u64,
            sections: self.sections.len() as u64,
            occurrences: self.occurrences.len() as u64,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedCatalogSnapshot {
    pub target: TermCampusKey,
    pub content_version: u64,
    pub publication: RefreshObservation,
    pub snapshot: CatalogSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RawStagingPayload {
    pub sha256: String,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeginRefreshAttemptCommand {
    pub observation_id: TraceId,
    pub target: TermCampusKey,
    pub started_at: String,
    pub source_content_sha256: Option<String>,
    pub source_bytes: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshFailureStage {
    Transport,
    Schema,
    Normalization,
    Publish,
}

impl RefreshFailureStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Transport => "TRANSPORT",
            Self::Schema => "SCHEMA",
            Self::Normalization => "NORMALIZATION",
            Self::Publish => "PUBLISH",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinishRefreshFailureCommand {
    pub observation_id: TraceId,
    pub completed_at: String,
    pub stage: RefreshFailureStage,
    pub source_content_sha256: Option<String>,
    pub source_bytes: Option<u64>,
    pub error_code: String,
    pub diagnostic_token: Option<String>,
}

/// Bounded, body-free HTTP evidence associated with a failed Catalog observation.
///
/// The transport is responsible for redacting and truncating `error_chain`; storage enforces the
/// same durable boundary so raw upstream bodies, credentials, and unbounded error strings cannot
/// enter the operational database accidentally.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CatalogFailureAudit {
    pub http_status: Option<u16>,
    pub content_type: Option<String>,
    pub content_encoding: Option<String>,
    pub decoded_bytes: Option<u64>,
    pub error_chain: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogRefreshCommand {
    pub observation_id: TraceId,
    pub target: TermCampusKey,
    pub started_at: String,
    pub completed_at: String,
    pub source_content_sha256: String,
    pub semantic_content_sha256: String,
    pub source_bytes: u64,
    pub raw_payload: Option<RawStagingPayload>,
    pub snapshot: CatalogSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmptySnapshotDecision {
    AcceptNonEmptyOrUnchangedEmpty,
    AcceptInitialSelectorConfirmedEmpty(InitialEmptyProof),
    RetainLastKnownGood,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InitialEmptyProof {
    CurrentSelectorMembership,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PublishOutcome {
    AppliedChanged {
        content_version: u64,
        counts: CatalogCounts,
    },
    AppliedUnchanged {
        content_version: u64,
        counts: CatalogCounts,
    },
    InitialValidEmpty {
        content_version: u64,
    },
    SuspectEmptyRetained {
        content_version: u64,
        retained_observation_id: Option<TraceId>,
        retained_counts: CatalogCounts,
    },
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CatalogCounts {
    pub course_groups: u64,
    pub course_variants: u64,
    pub sections: u64,
    pub occurrences: u64,
}

/// One `catalog_targets` row: the target and the content version it serves.
///
/// Enumerates every target row, including targets that are absent from the
/// current selector and targets that have never published (`current_content_version == 0`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogTargetVersion {
    pub target: TermCampusKey,
    pub current_content_version: u64,
}

/// In-place replacement of the derived delivery columns of one published target.
///
/// The rewrite touches only columns that are recomputed from stored canonical facts:
/// it never changes content versions, canonical hashes, accepted semantic hashes,
/// checkpoints, counts, FTS, staging or Open state. Rows omitted from the lists keep
/// their stored values; the derivation stamp is written even when both lists are empty.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogDeliveryRewrite {
    pub derivation_version: u32,
    pub stamped_at: String,
    pub sections: Vec<SectionDeliveryRewrite>,
    pub occurrences: Vec<OccurrenceDeliveryRewrite>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SectionDeliveryRewrite {
    pub key: SectionKey,
    pub delivery_modality: String,
    pub synchronicity: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OccurrenceDeliveryRewrite {
    pub section_key: SectionKey,
    pub occurrence_key: String,
    pub occurrence_kind: String,
    pub modality: String,
    pub synchronicity: String,
    pub evidence: String,
    pub normalization_reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogDeliveryRewriteReport {
    pub target: TermCampusKey,
    pub derivation_version: u32,
    pub sections_rewritten: u64,
    pub occurrences_rewritten: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetState {
    pub target: TermCampusKey,
    pub last_attempt_sequence: u64,
    pub current_content_version: u64,
    pub accepted_semantic_sha256: Option<String>,
    pub counts: CatalogCounts,
    pub last_attempt_observation_id: Option<TraceId>,
    pub last_success_observation_id: Option<TraceId>,
    pub last_published_observation_id: Option<TraceId>,
    pub last_nonempty_observation_id: Option<TraceId>,
    pub pending_empty_observation_id: Option<TraceId>,
    pub last_attempt: Option<RefreshObservation>,
    pub last_success: Option<RefreshObservation>,
    pub last_published: Option<RefreshObservation>,
    pub last_nonempty: Option<RefreshObservation>,
    pub pending_empty: Option<RefreshObservation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RefreshStatus {
    Started,
    Staged,
    AppliedChanged,
    AppliedUnchanged,
    EmptyValidInitial,
    EmptySuspectRetained,
    Failed,
    Interrupted,
}

impl RefreshStatus {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "STARTED" => Some(Self::Started),
            "STAGED" => Some(Self::Staged),
            "APPLIED_CHANGED" => Some(Self::AppliedChanged),
            "APPLIED_UNCHANGED" => Some(Self::AppliedUnchanged),
            "EMPTY_VALID_INITIAL" => Some(Self::EmptyValidInitial),
            "EMPTY_SUSPECT_RETAINED" => Some(Self::EmptySuspectRetained),
            "FAILED" => Some(Self::Failed),
            "INTERRUPTED" => Some(Self::Interrupted),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RefreshObservation {
    pub observation_id: TraceId,
    pub target: TermCampusKey,
    pub attempt_sequence: u64,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub status: RefreshStatus,
    pub source_content_sha256: Option<String>,
    pub semantic_sha256: Option<String>,
    pub source_bytes: Option<u64>,
    pub counts: CatalogCounts,
    pub changed: Option<bool>,
    pub resulting_content_version: Option<u64>,
    pub retained_observation_id: Option<TraceId>,
    pub error_stage: Option<String>,
    pub error_code: Option<String>,
    pub diagnostic_token: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscoverySourceKind {
    Selector,
    Bootstrap,
}

impl DiscoverySourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Selector => "SELECTOR",
            Self::Bootstrap => "BOOTSTRAP",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoverySourceVersion {
    pub source_version_id: String,
    pub source_kind: DiscoverySourceKind,
    pub source_identity: String,
    pub content_sha256: String,
    /// Versioned, privacy-safe source metadata with five-state presence facts.
    /// Raw selector content is never retained here.
    pub canonical_facts: Value,
    /// First observation time for this immutable content-addressed source version.
    pub observed_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveredTerm {
    pub term_id: TermId,
    pub year: Option<u16>,
    pub term_code: Option<String>,
    pub display_name: Option<String>,
    pub published: Option<bool>,
    pub canonical_facts: Value,
    pub source_version_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveredCampus {
    pub target: TermCampusKey,
    pub display_name: Option<String>,
    pub category: Option<String>,
    pub enabled: Option<bool>,
    pub canonical_facts: Value,
    pub source_version_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoveredSubject {
    pub term_id: TermId,
    pub campus_code: CampusCode,
    pub subject_code: String,
    pub display_name: Option<String>,
    pub enabled: Option<bool>,
    pub canonical_facts: Value,
    pub source_version_id: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoverySnapshot {
    pub sources: Vec<DiscoverySourceVersion>,
    pub terms: Vec<DiscoveredTerm>,
    pub campuses: Vec<DiscoveredCampus>,
    pub subjects: Vec<DiscoveredSubject>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublishedDiscoverySnapshot {
    pub state: DiscoveryState,
    pub sources: Vec<DiscoverySourceVersion>,
    pub snapshot: DiscoverySnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BeginDiscoveryAttemptCommand {
    pub observation_id: TraceId,
    pub started_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinishDiscoveryFailureCommand {
    pub observation_id: TraceId,
    pub completed_at: String,
    pub stage: RefreshFailureStage,
    pub error_code: String,
    pub diagnostic_token: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryRefreshCommand {
    pub observation_id: TraceId,
    pub started_at: String,
    pub completed_at: String,
    pub snapshot: DiscoverySnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryPublishOutcome {
    AppliedChanged { content_version: u64 },
    AppliedUnchanged { content_version: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryState {
    pub content_version: u64,
    pub semantic_sha256: Option<String>,
    pub last_attempt_observation_id: Option<TraceId>,
    pub last_success_observation_id: Option<TraceId>,
    pub last_published_observation_id: Option<TraceId>,
    pub last_nonempty_observation_id: Option<TraceId>,
    pub is_stale: bool,
    pub availability: DiscoveryAvailability,
    pub term_count: u64,
    pub campus_count: u64,
    pub subject_count: u64,
    pub last_attempt: Option<DiscoveryObservation>,
    pub last_success: Option<DiscoveryObservation>,
    pub last_published: Option<DiscoveryObservation>,
    pub last_nonempty: Option<DiscoveryObservation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryAvailability {
    UnavailableNoFirstSuccess,
    Fresh,
    Stale,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryStatus {
    Started,
    AppliedChanged,
    AppliedUnchanged,
    Failed,
}

impl DiscoveryStatus {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "STARTED" => Some(Self::Started),
            "APPLIED_CHANGED" => Some(Self::AppliedChanged),
            "APPLIED_UNCHANGED" => Some(Self::AppliedUnchanged),
            "FAILED" => Some(Self::Failed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryObservation {
    pub observation_id: TraceId,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub status: DiscoveryStatus,
    pub semantic_sha256: Option<String>,
    pub source_content_sha256: Vec<String>,
    pub counts: DiscoveryCounts,
    pub changed: Option<bool>,
    pub resulting_content_version: Option<u64>,
    pub error_stage: Option<String>,
    pub error_code: Option<String>,
    pub diagnostic_token: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DiscoveryCounts {
    pub terms: u64,
    pub campuses: u64,
    pub subjects: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationRecord {
    pub migration_id: u32,
    pub name: String,
    pub sha256: String,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct StorageIntegrityReport {
    pub foreign_key_violations: u64,
    pub missing_fts_rows: u64,
    pub orphan_fts_rows: u64,
    pub duplicate_fts_rows: u64,
    pub version_mismatched_fts_rows: u64,
}

impl StorageIntegrityReport {
    pub const fn is_clean(self) -> bool {
        self.foreign_key_violations == 0
            && self.missing_fts_rows == 0
            && self.orphan_fts_rows == 0
            && self.duplicate_fts_rows == 0
            && self.version_mismatched_fts_rows == 0
    }
}

/// The WAL checkpoint modes SQLite offers, exposed as a typed command so a
/// dependent never has to run a `PRAGMA` against the sealed connection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WalCheckpointMode {
    /// Backfills as many frames as it can without waiting for anyone. Never
    /// blocks; a reader older than the newest frame caps the progress.
    Passive,
    /// Waits for writers (up to the connection's busy timeout) and backfills
    /// every frame; readers may keep the log from being reset.
    Full,
    /// `Full`, then waits for readers so the next writer restarts the log
    /// from the beginning.
    Restart,
    /// `Restart`, then truncates the log file to zero bytes.
    Truncate,
}

impl WalCheckpointMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Passive => "PASSIVE",
            Self::Full => "FULL",
            Self::Restart => "RESTART",
            Self::Truncate => "TRUNCATE",
        }
    }
}

/// What `PRAGMA wal_checkpoint` reported.
///
/// `log_frames` is the size of the log in frames and `checkpointed_frames`
/// how many of them have been copied into the main database. A report whose
/// two counters are equal after a small write proves that no connection is
/// holding a stale read transaction against the file.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct WalCheckpointReport {
    pub busy: bool,
    pub log_frames: u64,
    pub checkpointed_frames: u64,
}

impl WalCheckpointReport {
    /// Every frame of the log has been backfilled into the main database.
    pub const fn is_complete(self) -> bool {
        self.checkpointed_frames == self.log_frames
    }
}

/// The transaction the connection is in right now, per `sqlite3_txn_state`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageTransactionState {
    None,
    Read,
    Write,
}

impl StorageTransactionState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "NONE",
            Self::Read => "READ",
            Self::Write => "WRITE",
        }
    }
}
