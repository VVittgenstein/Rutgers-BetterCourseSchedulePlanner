use bcsp_contracts::{
    CatalogModality, CatalogOccurrenceEvidence, CatalogOccurrenceKind, CatalogRequiredness,
    CatalogSubjectCode, CatalogSynchronicity, TraceId,
};
use bcsp_operational_storage::{
    CatalogRefreshCommand, CatalogSnapshot, DiscoveredCampus as StoredDiscoveredCampus,
    DiscoveredSubject as StoredDiscoveredSubject, DiscoveredTerm as StoredDiscoveredTerm,
    DiscoveryRefreshCommand, DiscoverySnapshot as StoredDiscoverySnapshot,
    DiscoverySourceKind as StoredDiscoverySourceKind, DiscoverySourceVersion, ProvenanceEntityKind,
    StoredCourseGroup, StoredCourseVariant, StoredOccurrence, StoredProvenance, StoredSection,
};
use bcsp_rutgers_client::{
    DiscoverySnapshot, DiscoverySourceKind as ClientDiscoverySourceKind, JsonType, Presence,
};
use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;

use crate::{
    DayKnowledge, DuplicateDisposition, NormalizedCatalogTarget, NormalizedCourseGroup,
    NormalizedCourseVariant, NormalizedOccurrence, NormalizedSection, Sha256Hex, TimeKnowledge,
    Weekday,
};

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum SnapshotMappingError {
    #[error("source provenance SHA-256 is invalid")]
    InvalidSourceSha256,
    #[error("source byte count does not fit the operational DTO")]
    SourceBytesOverflow,
    #[error("discovery year is outside the supported unsigned 16-bit range: {0}")]
    InvalidDiscoveryYear(i64),
    #[error("discovery target references a campus absent from its campus dictionary: {0}")]
    MissingDiscoveryCampus(String),
    #[error("discovery entry references an absent source version: {0}")]
    MissingDiscoverySource(String),
    #[error("discovery source ownership is inconsistent across a related entry graph")]
    MixedDiscoverySourceOwnership,
    #[error("discovery source identity does not match its kind and payload hash")]
    InvalidDiscoverySourceIdentity,
    #[error("normalized count does not fit the operational DTO: {0}")]
    CountOverflow(&'static str),
}

pub fn to_catalog_refresh_command(
    normalized: &NormalizedCatalogTarget,
    observation_id: TraceId,
    started_at: impl Into<String>,
    completed_at: impl Into<String>,
) -> Result<CatalogRefreshCommand, SnapshotMappingError> {
    Ok(CatalogRefreshCommand {
        observation_id,
        target: normalized.target.clone(),
        started_at: started_at.into(),
        completed_at: completed_at.into(),
        source_content_sha256: normalized.provenance.body_sha256.clone(),
        semantic_content_sha256: normalized.content_hash.as_str().to_owned(),
        source_bytes: u64::try_from(normalized.provenance.decoded_bytes)
            .map_err(|_| SnapshotMappingError::SourceBytesOverflow)?,
        raw_payload: None,
        snapshot: to_operational_snapshot(normalized)?,
    })
}

pub fn to_operational_discovery_snapshot(
    discovery: &DiscoverySnapshot,
) -> Result<StoredDiscoverySnapshot, SnapshotMappingError> {
    let mut source_ids = std::collections::BTreeSet::new();
    let sources = discovery
        .sources
        .iter()
        .map(|source| {
            let source_sha256 = Sha256Hex::try_new(source.provenance.body_sha256.clone())
                .map_err(|_| SnapshotMappingError::InvalidSourceSha256)?;
            let expected_id = format!(
                "{}:{}",
                discovery_source_id_prefix(source.kind),
                source_sha256.as_str()
            );
            if source.id.as_str() != expected_id {
                return Err(SnapshotMappingError::InvalidDiscoverySourceIdentity);
            }
            source_ids.insert(source.id.as_str());
            Ok(DiscoverySourceVersion {
                source_version_id: source.id.as_str().to_owned(),
                source_kind: stored_discovery_source_kind(source.kind),
                source_identity: source.kind.stable_identity().to_owned(),
                content_sha256: source_sha256.as_str().to_owned(),
                canonical_facts: json!({
                    "schema": "discovery-source-v1",
                    "reportedSourceVersion": presence_fact(&source.source_version),
                }),
                observed_at: source.provenance.observed_at_utc.clone(),
            })
        })
        .collect::<Result<Vec<_>, SnapshotMappingError>>()?;

    let terms = discovery
        .terms
        .iter()
        .map(|term| {
            require_discovery_source(&source_ids, term.source_id.as_str())?;
            let year = match term.year {
                Presence::Value(value) => Some(
                    u16::try_from(value)
                        .map_err(|_| SnapshotMappingError::InvalidDiscoveryYear(value))?,
                ),
                Presence::Missing | Presence::Null | Presence::Malformed(_) => None,
            };
            Ok(StoredDiscoveredTerm {
                term_id: term.term_id.clone(),
                year,
                term_code: known_string(&term.term_code),
                display_name: known_string(&term.display),
                published: known_bool(&term.published),
                canonical_facts: json!({
                    "schema": "discovery-term-v1",
                    "year": presence_fact(&term.year),
                    "termCode": presence_fact(&term.term_code),
                    "display": presence_fact(&term.display),
                    "published": presence_fact(&term.published),
                }),
                source_version_id: term.source_id.as_str().to_owned(),
            })
        })
        .collect::<Result<Vec<_>, SnapshotMappingError>>()?;

    let term_sources = discovery
        .terms
        .iter()
        .map(|term| (&term.term_id, &term.source_id))
        .collect::<std::collections::BTreeMap<_, _>>();
    let campuses_by_code = discovery
        .campuses
        .iter()
        .map(|campus| (&campus.campus_code, campus))
        .collect::<std::collections::BTreeMap<_, _>>();
    let campuses = discovery
        .targets
        .iter()
        .map(|target| {
            require_discovery_source(&source_ids, target.source_id.as_str())?;
            let campus = campuses_by_code.get(target.key.campus()).ok_or_else(|| {
                SnapshotMappingError::MissingDiscoveryCampus(
                    target.key.campus().as_str().to_owned(),
                )
            })?;
            let term_source = term_sources
                .get(target.key.term())
                .ok_or(SnapshotMappingError::MixedDiscoverySourceOwnership)?;
            if campus.source_id != target.source_id || *term_source != &target.source_id {
                return Err(SnapshotMappingError::MixedDiscoverySourceOwnership);
            }
            Ok(StoredDiscoveredCampus {
                target: target.key.clone(),
                display_name: known_string(&campus.display),
                category: known_string(&campus.category),
                enabled: known_bool(&target.enabled),
                canonical_facts: json!({
                    "schema": "discovery-campus-v1",
                    "campusDisplay": presence_fact(&campus.display),
                    "campusCategory": presence_fact(&campus.category),
                    "campusEnabled": presence_fact(&campus.enabled),
                    "targetEnabled": presence_fact(&target.enabled),
                }),
                source_version_id: target.source_id.as_str().to_owned(),
            })
        })
        .collect::<Result<Vec<_>, SnapshotMappingError>>()?;

    let target_sources = discovery
        .targets
        .iter()
        .map(|target| (&target.key, &target.source_id))
        .collect::<std::collections::BTreeMap<_, _>>();
    let subjects = discovery
        .subjects
        .iter()
        .map(|subject| {
            require_discovery_source(&source_ids, subject.source_id.as_str())?;
            if target_sources.get(&subject.target).copied() != Some(&subject.source_id) {
                return Err(SnapshotMappingError::MixedDiscoverySourceOwnership);
            }
            Ok(StoredDiscoveredSubject {
                term_id: subject.target.term().clone(),
                campus_code: subject.target.campus().clone(),
                subject_code: subject.subject_code.as_str().to_owned(),
                display_name: known_string(&subject.display),
                enabled: known_bool(&subject.enabled),
                canonical_facts: json!({
                    "schema": "discovery-subject-v1",
                    "display": presence_fact(&subject.display),
                    "enabled": presence_fact(&subject.enabled),
                }),
                source_version_id: subject.source_id.as_str().to_owned(),
            })
        })
        .collect::<Result<Vec<_>, SnapshotMappingError>>()?;

    Ok(StoredDiscoverySnapshot {
        sources,
        terms,
        campuses,
        subjects,
    })
}

pub fn to_discovery_refresh_command(
    discovery: &DiscoverySnapshot,
    observation_id: TraceId,
    started_at: impl Into<String>,
    completed_at: impl Into<String>,
) -> Result<DiscoveryRefreshCommand, SnapshotMappingError> {
    Ok(DiscoveryRefreshCommand {
        observation_id,
        started_at: started_at.into(),
        completed_at: completed_at.into(),
        snapshot: to_operational_discovery_snapshot(discovery)?,
    })
}

pub fn to_operational_snapshot(
    normalized: &NormalizedCatalogTarget,
) -> Result<CatalogSnapshot, SnapshotMappingError> {
    let source_sha256 = Sha256Hex::try_new(normalized.provenance.body_sha256.clone())
        .map_err(|_| SnapshotMappingError::InvalidSourceSha256)?;
    let mut snapshot = CatalogSnapshot::default();

    for (group_ordinal, group) in normalized.groups.iter().enumerate() {
        let group_entity_key = group_entity_key(group);
        snapshot.course_groups.push(StoredCourseGroup {
            key: group.key.clone(),
            // Group facts contain only aggregate identity/variant membership;
            // no variant is selected as a first/last representative.
            canonical_facts: json!({
                "schema": "catalog-group-v1",
                "courseString": group.key.course_string().as_str(),
                "variantFingerprints": group
                    .variants
                    .iter()
                    .map(|variant| variant.key.fingerprint().as_str())
                    .collect::<Vec<_>>(),
            }),
        });
        snapshot.provenance.push(provenance(
            ProvenanceEntityKind::CourseGroup,
            group_entity_key,
            "canonicalFacts",
            &source_sha256,
            checked_ordinal(group_ordinal, "group ordinal")?,
            json!({"rawCourseCount": group.raw_course_count}),
        ));

        for (variant_ordinal, variant) in group.variants.iter().enumerate() {
            map_variant(
                &mut snapshot,
                variant,
                &source_sha256,
                checked_ordinal(variant_ordinal, "variant ordinal")?,
            )?;
        }
    }
    for audit in &normalized.duplicate_audits {
        let mut raw_record_hashes = audit
            .raw_record_hashes
            .iter()
            .map(Sha256Hex::as_str)
            .collect::<Vec<_>>();
        raw_record_hashes.sort_unstable();
        snapshot.provenance.push(provenance(
            ProvenanceEntityKind::Section,
            audit.section_key.to_string(),
            "duplicateAudit",
            &source_sha256,
            0,
            json!({
                "schema": "catalog-duplicate-audit-v1",
                "rawRecordCount": audit.raw_record_count,
                "rawRecordSha256": raw_record_hashes,
                "semanticSha256": audit.semantic_hash.as_str(),
                "disposition": duplicate_disposition_name(audit.disposition),
                "reason": audit.reason,
            }),
        ));
    }
    Ok(snapshot)
}

fn map_variant(
    snapshot: &mut CatalogSnapshot,
    variant: &NormalizedCourseVariant,
    source_sha256: &Sha256Hex,
    source_ordinal: u32,
) -> Result<(), SnapshotMappingError> {
    let subject_code = known_string(&variant.fields.subject).and_then(|value| {
        CatalogSubjectCode::try_from(value)
            .ok()
            .map(CatalogSubjectCode::into_inner)
    });
    let course_number = known_string(&variant.fields.course_number);
    let title = known_string(&variant.fields.title);
    let description = known_string(&variant.fields.course_description);
    let credits_summary = credits_summary(variant);
    let supplement = known_string(&variant.fields.supplement_code);
    let search_document = search_document(variant);
    let variant_entity_key = variant_entity_key(variant);
    snapshot.course_variants.push(StoredCourseVariant {
        key: variant.key.clone(),
        subject_code,
        course_number,
        title,
        description,
        credits_summary,
        supplement,
        search_document,
        canonical_sha256: variant.canonical_hash.as_str().to_owned(),
        raw_multiplicity: u32::try_from(variant.raw_course_count)
            .map_err(|_| SnapshotMappingError::CountOverflow("variant raw multiplicity"))?,
        canonical_facts: variant.canonical_course_facts.clone(),
    });
    snapshot.provenance.push(provenance(
        ProvenanceEntityKind::CourseVariant,
        variant_entity_key.clone(),
        "canonicalFacts",
        source_sha256,
        source_ordinal,
        json!({
            "canonicalSha256": variant.canonical_hash.as_str(),
            "recordSha256": variant
                .record_hashes
                .iter()
                .map(Sha256Hex::as_str)
                .collect::<Vec<_>>(),
        }),
    ));
    for open_sections in &variant.catalog_open_sections_snapshots {
        snapshot.provenance.push(provenance(
            ProvenanceEntityKind::CourseVariant,
            variant_entity_key.clone(),
            "catalogOpenSectionsSnapshot",
            source_sha256,
            checked_ordinal(open_sections.source_ordinal, "course source ordinal")?,
            json!({
                "schema": "catalog-open-sections-snapshot-v1",
                "catalogOpenSectionsIsSnapshotOnly": true,
                "openSections": presence_fact(&open_sections.value),
                "rawCourseSha256": open_sections.raw_course_hash.as_str(),
            }),
        ));
    }

    for (section_ordinal, section) in variant.sections.iter().enumerate() {
        map_section(
            snapshot,
            &variant.key,
            section,
            source_sha256,
            checked_ordinal(section_ordinal, "section ordinal")?,
        )?;
    }
    Ok(())
}

fn map_section(
    snapshot: &mut CatalogSnapshot,
    variant_key: &bcsp_contracts::CourseVariantKey,
    section: &NormalizedSection,
    source_sha256: &Sha256Hex,
    source_ordinal: u32,
) -> Result<(), SnapshotMappingError> {
    let catalog_status = match section.catalog_open_status {
        Presence::Value(true) => Some("CATALOG_SNAPSHOT_OPEN".to_owned()),
        Presence::Value(false) => Some("CATALOG_SNAPSHOT_CLOSED".to_owned()),
        Presence::Missing | Presence::Null | Presence::Malformed(_) => None,
    };
    let section_entity_key = section.key.to_string();
    let delivery_modality = CatalogModality::from(section.delivery_modality);
    let synchronicity = CatalogSynchronicity::from(section.synchronicity);
    snapshot.sections.push(StoredSection {
        key: section.key.clone(),
        variant_key: variant_key.clone(),
        section_number: known_string(&section.section_number),
        catalog_status,
        section_course_type: known_string(&section.raw_section_course_type),
        delivery_modality: delivery_modality.wire_name().to_owned(),
        synchronicity: synchronicity.wire_name().to_owned(),
        canonical_facts: section.canonical_facts.clone(),
        canonical_sha256: section.semantic_hash.as_str().to_owned(),
    });
    snapshot.provenance.push(provenance(
        ProvenanceEntityKind::Section,
        section_entity_key,
        "canonicalFacts",
        source_sha256,
        source_ordinal,
        json!({
            "recordSha256": section.first_record_hash.as_str(),
            "semanticSha256": section.semantic_hash.as_str(),
            "catalogOpenStatusIsSnapshotOnly": true,
        }),
    ));

    for occurrence in &section.occurrences {
        map_occurrence(snapshot, section, occurrence, source_sha256)?;
    }
    Ok(())
}

fn map_occurrence(
    snapshot: &mut CatalogSnapshot,
    section: &NormalizedSection,
    occurrence: &NormalizedOccurrence,
    source_sha256: &Sha256Hex,
) -> Result<(), SnapshotMappingError> {
    let ordinal = checked_ordinal(occurrence.source_ordinal, "occurrence ordinal")?;
    let occurrence_key = format!("{ordinal:010}-{}", occurrence.raw_hash.as_str());
    let (start_minute, end_minute) = match occurrence.time {
        TimeKnowledge::Known {
            start_minute,
            end_minute,
        } => (Some(start_minute), Some(end_minute)),
        _ => (None, None),
    };
    let modality = CatalogModality::from(occurrence.modality);
    let synchronicity = CatalogSynchronicity::from(occurrence.synchronicity);
    let requiredness = CatalogRequiredness::from(occurrence.requiredness);
    let kind = CatalogOccurrenceKind::from(occurrence.kind);
    let evidence = CatalogOccurrenceEvidence::from(occurrence.evidence);
    snapshot.occurrences.push(StoredOccurrence {
        section_key: section.key.clone(),
        occurrence_key: occurrence_key.clone(),
        ordinal,
        weekday: weekday_text(&occurrence.days),
        start_minute,
        end_minute,
        time_knowledge: time_knowledge_name(occurrence.time).to_owned(),
        requiredness: requiredness.wire_name().to_owned(),
        occurrence_kind: kind.wire_name().to_owned(),
        evidence: evidence.wire_name().to_owned(),
        normalization_reason: occurrence.normalization_reason.to_owned(),
        modality: modality.wire_name().to_owned(),
        synchronicity: synchronicity.wire_name().to_owned(),
        location: known_string(&occurrence.raw_campus_location),
        building: known_string(&occurrence.raw_building_code),
        room: known_string(&occurrence.raw_room_number),
        raw_sha256: occurrence.raw_hash.as_str().to_owned(),
        canonical_facts: occurrence.canonical_facts.clone(),
    });
    snapshot.provenance.push(provenance(
        ProvenanceEntityKind::Occurrence,
        format!("{}/{}", section.key, occurrence_key),
        "canonicalFacts",
        source_sha256,
        ordinal,
        json!({
            "rawSha256": occurrence.raw_hash.as_str(),
            "normalizationReason": occurrence.normalization_reason,
            "evidence": evidence.wire_name(),
        }),
    ));
    Ok(())
}

fn group_entity_key(group: &NormalizedCourseGroup) -> String {
    format!(
        "{}/{}/{}",
        group.key.term(),
        group.key.campus(),
        group.key.course_string()
    )
}

fn variant_entity_key(variant: &NormalizedCourseVariant) -> String {
    format!(
        "{}/{}/{}/{}",
        variant.key.group().term(),
        variant.key.group().campus(),
        variant.key.group().course_string(),
        variant.key.fingerprint()
    )
}

fn provenance(
    entity_kind: ProvenanceEntityKind,
    entity_key: String,
    field_name: &'static str,
    source_sha256: &Sha256Hex,
    source_ordinal: u32,
    detail: Value,
) -> StoredProvenance {
    StoredProvenance {
        entity_kind,
        entity_key,
        field_name: field_name.to_owned(),
        source_sha256: source_sha256.as_str().to_owned(),
        source_ordinal,
        detail,
    }
}

fn credits_summary(variant: &NormalizedCourseVariant) -> Option<String> {
    variant
        .fields
        .credits_object
        .value()
        .and_then(|object| object.get("code"))
        .and_then(Value::as_str)
        .map(str::to_owned)
        .or_else(|| variant.fields.credits.value().map(ToString::to_string))
}

fn search_document(variant: &NormalizedCourseVariant) -> String {
    let mut values = vec![variant.key.group().course_string().as_str().to_owned()];
    for value in [
        &variant.fields.subject,
        &variant.fields.subject_description,
        &variant.fields.course_number,
        &variant.fields.title,
        &variant.fields.expanded_title,
        &variant.fields.course_description,
        &variant.fields.course_notes,
    ] {
        if let Some(value) = value.value().filter(|value| !value.is_empty()) {
            values.push(value.clone());
        }
    }
    values.join("\n")
}

fn known_string(value: &Presence<String>) -> Option<String> {
    value.value().cloned()
}

fn known_bool(value: &Presence<bool>) -> Option<bool> {
    value.value().copied()
}

fn require_discovery_source(
    source_ids: &std::collections::BTreeSet<&str>,
    source_id: &str,
) -> Result<(), SnapshotMappingError> {
    if source_ids.contains(source_id) {
        Ok(())
    } else {
        Err(SnapshotMappingError::MissingDiscoverySource(
            source_id.to_owned(),
        ))
    }
}

const fn discovery_source_id_prefix(kind: ClientDiscoverySourceKind) -> &'static str {
    match kind {
        ClientDiscoverySourceKind::Selector => "selector",
        ClientDiscoverySourceKind::Bootstrap => "bootstrap",
    }
}

const fn stored_discovery_source_kind(
    kind: ClientDiscoverySourceKind,
) -> StoredDiscoverySourceKind {
    match kind {
        ClientDiscoverySourceKind::Selector => StoredDiscoverySourceKind::Selector,
        ClientDiscoverySourceKind::Bootstrap => StoredDiscoverySourceKind::Bootstrap,
    }
}

const fn duplicate_disposition_name(value: DuplicateDisposition) -> &'static str {
    match value {
        DuplicateDisposition::AuditedEquivalentCollapse => "AUDITED_EQUIVALENT_COLLAPSE",
    }
}

fn presence_fact<T>(presence: &Presence<T>) -> Value
where
    T: Serialize,
{
    match presence {
        Presence::Missing => json!({"state": "MISSING"}),
        Presence::Null => json!({"state": "EXPLICIT_NULL"}),
        Presence::Malformed(malformed) => json!({
            "state": "MALFORMED",
            "jsonType": malformed_json_type(malformed.json_type),
            "canonicalSha256": malformed.canonical_sha256,
        }),
        Presence::Value(value) => json!({"state": "VALUE", "value": value}),
    }
}

const fn malformed_json_type(json_type: JsonType) -> &'static str {
    match json_type {
        JsonType::Boolean => "BOOLEAN",
        JsonType::Number => "NUMBER",
        JsonType::String => "STRING",
        JsonType::Array => "ARRAY",
        JsonType::Object => "OBJECT",
    }
}

fn weekday_text(days: &DayKnowledge) -> Option<String> {
    let DayKnowledge::Known(days) = days else {
        return None;
    };
    Some(
        days.as_slice()
            .iter()
            .map(|day| match day {
                Weekday::Monday => "M",
                Weekday::Tuesday => "T",
                Weekday::Wednesday => "W",
                Weekday::Thursday => "H",
                Weekday::Friday => "F",
                Weekday::Saturday => "S",
                Weekday::Sunday => "U",
            })
            .collect::<Vec<_>>()
            .join(","),
    )
}

const fn time_knowledge_name(value: TimeKnowledge) -> &'static str {
    match value {
        TimeKnowledge::Missing => "MISSING",
        TimeKnowledge::Null => "EXPLICIT_NULL",
        TimeKnowledge::Empty => "EMPTY",
        TimeKnowledge::Partial => "PARTIAL",
        TimeKnowledge::Invalid => "INVALID",
        TimeKnowledge::Known { .. } => "KNOWN",
    }
}

fn checked_ordinal(value: usize, field: &'static str) -> Result<u32, SnapshotMappingError> {
    u32::try_from(value).map_err(|_| SnapshotMappingError::CountOverflow(field))
}
