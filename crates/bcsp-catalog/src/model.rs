use bcsp_contracts::{CourseGroupKey, CourseVariantKey, SectionKey, TermCampusKey};
use bcsp_rutgers_client::{Presence, SourceProvenance};
use serde_json::{Number, Value};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
#[serde(transparent)]
pub struct Sha256Hex(String);

impl Sha256Hex {
    pub fn try_new(value: String) -> Result<Self, Sha256HexError> {
        if value.len() == 64
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
        {
            Ok(Self(value))
        } else {
            Err(Sha256HexError)
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn from_digest_bytes(bytes: &[u8]) -> Self {
        Self(bcsp_rutgers_client::sha256_hex(bytes))
    }
}

impl std::fmt::Display for Sha256Hex {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
#[error("SHA-256 digest must be exactly 64 lowercase hexadecimal characters")]
pub struct Sha256HexError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CollectionPresence {
    Missing,
    Null,
    Malformed,
    PresentEmpty,
    PresentNonEmpty,
}

impl<T> From<&Presence<Vec<T>>> for CollectionPresence {
    fn from(value: &Presence<Vec<T>>) -> Self {
        match value {
            Presence::Missing => Self::Missing,
            Presence::Null => Self::Null,
            Presence::Malformed(_) => Self::Malformed,
            Presence::Value(values) if values.is_empty() => Self::PresentEmpty,
            Presence::Value(_) => Self::PresentNonEmpty,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WeekdaySet(Vec<Weekday>);

impl WeekdaySet {
    pub(crate) fn new(mut weekdays: Vec<Weekday>) -> Self {
        weekdays.sort();
        weekdays.dedup();
        Self(weekdays)
    }

    pub fn as_slice(&self) -> &[Weekday] {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DayKnowledge {
    Missing,
    Null,
    Empty,
    Known(WeekdaySet),
    Invalid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeKnowledge {
    Missing,
    Null,
    Empty,
    Partial,
    Invalid,
    Known { start_minute: u16, end_minute: u16 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Requiredness {
    UnknownRequiredness,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OccurrenceKind {
    Scheduled,
    ByArrangement,
    Unspecified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OccurrenceEvidence {
    None,
    Physical,
    Remote,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OccurrenceModality {
    OnCampusOrInPerson,
    Hybrid,
    Online,
    Unknown,
    UnknownConflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeliveryModality {
    OnCampusOrInPerson,
    Hybrid,
    Online,
    Other,
    Unknown,
    UnknownConflict,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Synchronicity {
    Synchronous,
    Asynchronous,
    Mixed,
    Unspecified,
    Unknown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InstructorReliability {
    NameOnly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedInstructor {
    pub name: Presence<String>,
    pub reliability: InstructorReliability,
    pub canonical_facts: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedOccurrence {
    pub source_ordinal: usize,
    pub raw_hash: Sha256Hex,
    pub raw_day: Presence<String>,
    pub days: DayKnowledge,
    pub raw_start_time_military: Presence<String>,
    pub raw_end_time_military: Presence<String>,
    pub time: TimeKnowledge,
    pub requiredness: Requiredness,
    pub kind: OccurrenceKind,
    pub raw_mode_code: Presence<String>,
    pub raw_mode_description: Presence<String>,
    pub modality: OccurrenceModality,
    pub synchronicity: Synchronicity,
    pub evidence: OccurrenceEvidence,
    pub raw_campus_location: Presence<String>,
    pub raw_building_code: Presence<String>,
    pub raw_room_number: Presence<String>,
    pub normalization_reason: &'static str,
    pub canonical_facts: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CourseVariantFields {
    pub supplement_code: Presence<String>,
    pub credits: Presence<Number>,
    pub credits_object: Presence<std::collections::BTreeMap<String, Value>>,
    pub title: Presence<String>,
    pub expanded_title: Presence<String>,
    pub level: Presence<String>,
    pub offering_unit_code: Presence<String>,
    pub subject: Presence<String>,
    pub subject_description: Presence<String>,
    pub course_number: Presence<String>,
    pub course_description: Presence<String>,
    pub course_notes: Presence<String>,
    pub prerequisite_notes: Presence<String>,
    pub core_codes: Presence<Vec<Value>>,
    pub campus_locations: Presence<Vec<Value>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CatalogOpenSectionsSnapshot {
    pub source_ordinal: usize,
    pub value: Presence<i64>,
    pub raw_course_hash: Sha256Hex,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedSection {
    pub key: SectionKey,
    pub section_number: Presence<String>,
    /// A Catalog snapshot fact only; never interpreted as current Open state.
    pub catalog_open_status: Presence<bool>,
    pub raw_section_course_type: Presence<String>,
    pub delivery_modality: DeliveryModality,
    pub synchronicity: Synchronicity,
    pub occurrence_source: CollectionPresence,
    pub occurrences: Vec<NormalizedOccurrence>,
    pub instructor_source: CollectionPresence,
    pub instructors: Vec<NormalizedInstructor>,
    pub semantic_hash: Sha256Hex,
    pub first_record_hash: Sha256Hex,
    pub canonical_facts: Value,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedCourseVariant {
    pub key: CourseVariantKey,
    pub fields: CourseVariantFields,
    pub raw_course_count: usize,
    pub record_hashes: Vec<Sha256Hex>,
    pub canonical_hash: Sha256Hex,
    pub canonical_course_facts: Value,
    pub catalog_open_sections_snapshots: Vec<CatalogOpenSectionsSnapshot>,
    pub sections: Vec<NormalizedSection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedCourseGroup {
    pub key: CourseGroupKey,
    pub raw_course_count: usize,
    pub variants: Vec<NormalizedCourseVariant>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DuplicateDisposition {
    AuditedEquivalentCollapse,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DuplicateAudit {
    pub section_key: SectionKey,
    pub raw_record_count: usize,
    pub raw_record_hashes: Vec<Sha256Hex>,
    pub semantic_hash: Sha256Hex,
    pub disposition: DuplicateDisposition,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CatalogMetrics {
    pub raw_courses: usize,
    pub raw_sections: usize,
    pub normalized_sections: usize,
    pub raw_meetings: usize,
    pub raw_instructor_assignments: usize,
    pub equivalent_duplicate_keys: usize,
    pub delivery_conflict_raw_sections: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NormalizedCatalogTarget {
    pub target: TermCampusKey,
    pub groups: Vec<NormalizedCourseGroup>,
    pub duplicate_audits: Vec<DuplicateAudit>,
    pub content_hash: Sha256Hex,
    pub provenance: SourceProvenance,
    pub metrics: CatalogMetrics,
}

impl NormalizedCatalogTarget {
    pub fn is_empty(&self) -> bool {
        self.metrics.raw_courses == 0
    }

    pub fn sections(&self) -> impl Iterator<Item = &NormalizedSection> {
        self.groups
            .iter()
            .flat_map(|group| &group.variants)
            .flat_map(|variant| &variant.sections)
    }
}

impl From<DeliveryModality> for bcsp_contracts::CatalogModality {
    fn from(value: DeliveryModality) -> Self {
        match value {
            DeliveryModality::OnCampusOrInPerson => Self::OnCampusOrInPerson,
            DeliveryModality::Hybrid => Self::Hybrid,
            DeliveryModality::Online => Self::Online,
            DeliveryModality::Other => Self::Other,
            DeliveryModality::Unknown => Self::Unknown,
            DeliveryModality::UnknownConflict => Self::UnknownConflict,
        }
    }
}

impl From<OccurrenceModality> for bcsp_contracts::CatalogModality {
    fn from(value: OccurrenceModality) -> Self {
        match value {
            OccurrenceModality::OnCampusOrInPerson => Self::OnCampusOrInPerson,
            OccurrenceModality::Hybrid => Self::Hybrid,
            OccurrenceModality::Online => Self::Online,
            OccurrenceModality::Unknown => Self::Unknown,
            OccurrenceModality::UnknownConflict => Self::UnknownConflict,
        }
    }
}

impl From<Synchronicity> for bcsp_contracts::CatalogSynchronicity {
    fn from(value: Synchronicity) -> Self {
        match value {
            Synchronicity::Synchronous => Self::Sync,
            Synchronicity::Asynchronous => Self::Async,
            Synchronicity::Mixed => Self::Mixed,
            Synchronicity::Unspecified => Self::Unspecified,
            Synchronicity::Unknown => Self::Unknown,
        }
    }
}

impl From<OccurrenceKind> for bcsp_contracts::CatalogOccurrenceKind {
    fn from(value: OccurrenceKind) -> Self {
        match value {
            OccurrenceKind::Scheduled => Self::Scheduled,
            OccurrenceKind::ByArrangement => Self::ByArrangement,
            OccurrenceKind::Unspecified => Self::Unspecified,
        }
    }
}

impl From<OccurrenceEvidence> for bcsp_contracts::CatalogOccurrenceEvidence {
    fn from(value: OccurrenceEvidence) -> Self {
        match value {
            OccurrenceEvidence::None => Self::None,
            OccurrenceEvidence::Physical => Self::Physical,
            OccurrenceEvidence::Remote => Self::Remote,
        }
    }
}

impl From<Requiredness> for bcsp_contracts::CatalogRequiredness {
    fn from(value: Requiredness) -> Self {
        match value {
            Requiredness::UnknownRequiredness => Self::UnknownRequiredness,
        }
    }
}

impl From<TimeKnowledge> for bcsp_contracts::CatalogTimeKnowledgeV1 {
    fn from(value: TimeKnowledge) -> Self {
        match value {
            TimeKnowledge::Missing => Self::Missing,
            TimeKnowledge::Null => Self::ExplicitNull,
            TimeKnowledge::Empty => Self::Empty,
            TimeKnowledge::Partial => Self::Partial,
            TimeKnowledge::Invalid => Self::Invalid,
            TimeKnowledge::Known {
                start_minute,
                end_minute,
            } => Self::Known {
                start_minute,
                end_minute,
            },
        }
    }
}
