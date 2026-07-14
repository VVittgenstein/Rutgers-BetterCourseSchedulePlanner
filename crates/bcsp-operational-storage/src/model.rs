use bcsp_contracts::{
    CampusCode, CourseGroupKey, CourseVariantKey, SectionKey, TermCampusKey, TermId, TraceId,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
