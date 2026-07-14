use std::collections::{BTreeMap, BTreeSet};

use bcsp_contracts::{
    CATALOG_CONTRACT_VERSION, CatalogCommentV1, CatalogContentVersion, CatalogDiagnosticCode,
    CatalogDiscoveryAvailability, CatalogDiscoveryErrorClass, CatalogDiscoveryErrorV1,
    CatalogDiscoveryPointV1, CatalogDiscoveryProvenanceV1, CatalogDiscoveryResponseV1,
    CatalogDiscoverySourceId, CatalogDiscoverySourceKind, CatalogDiscoverySourceV1,
    CatalogDiscoveryStatusV1, CatalogEntityCountsV1, CatalogFieldKnowledge, CatalogFieldPresence,
    CatalogInstructorReliability, CatalogModality, CatalogOccurrenceEvidence,
    CatalogOccurrenceKeyV1, CatalogOccurrenceKind, CatalogOpenStatusProvenance,
    CatalogPayloadDigest, CatalogPrerequisiteState, CatalogProvenanceV1,
    CatalogRefreshCheckpointPointV1, CatalogRefreshCheckpointV1, CatalogRefreshClassification,
    CatalogRefreshErrorClass, CatalogRefreshObservationV1, CatalogRefreshPointV1,
    CatalogRefreshStatusV1, CatalogRequiredness, CatalogSnapshotOpenStatusV1, CatalogSourceKind,
    CatalogSubjectCode, CatalogSubjectV1, CatalogSynchronicity, CatalogTargetV1,
    CatalogTimeKnowledgeV1, CatalogUnitMajorV1, CatalogUnknownReason, NormalizedCatalogV1,
    NormalizedCourseGroupV1, NormalizedCourseVariantV1, NormalizedOccurrenceV1,
    NormalizedSectionV1, TermCampusKey, TraceId,
};
use bcsp_operational_storage::{
    CatalogCounts, DiscoveryAvailability, DiscoveryObservation, DiscoverySourceKind,
    DiscoverySourceVersion, DiscoveryState, DiscoveryStatus, PublishedCatalogSnapshot,
    PublishedDiscoverySnapshot, RefreshObservation, RefreshStatus, StoredCourseGroup,
    StoredCourseVariant, StoredOccurrence, StoredSection, TargetState,
};
use bcsp_rutgers_client::{Presence, RawCatalogCourse, RawCatalogSection, RawMeeting};
use serde::de::DeserializeOwned;
use serde_json::{Map, Number, Value};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::canonical::{
    canonical_course_variant_hash_v1, canonical_occurrence_hash_v1,
    canonical_section_semantic_hash_v1,
};
use crate::delivery::{classify_delivery, normalize_occurrence};
use crate::{DayKnowledge, Weekday, variant_fingerprint_v1};

type CodeListKnowledge = CatalogFieldKnowledge<Vec<String>>;
type MajorAndUnitKnowledge = (CodeListKnowledge, CodeListKnowledge);

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProjectionError {
    #[error("invalid stored Catalog projection at {context}: {reason}")]
    InvalidStoredProjection {
        context: &'static str,
        reason: String,
    },
    #[error("refresh observation {observation_id} is incomplete ({status})")]
    IncompleteRefreshObservation {
        observation_id: TraceId,
        status: &'static str,
    },
}

pub fn to_catalog_discovery_response_v1(
    published: &PublishedDiscoverySnapshot,
    projected_at: OffsetDateTime,
) -> Result<CatalogDiscoveryResponseV1, ProjectionError> {
    validate_discovery_state(&published.state)?;
    if published.sources != published.snapshot.sources {
        return Err(invalid(
            "discovery.sources",
            "top-level sources do not match the published snapshot",
        ));
    }

    let publication = published.state.last_published.as_ref();
    if published.state.content_version > 0 && publication.is_none() {
        return Err(invalid(
            "discovery.state.lastPublished",
            "a nonzero content version requires a published observation",
        ));
    }

    let mut source_versions = BTreeMap::new();
    let mut stable_source_ids = BTreeSet::new();
    let mut sources = Vec::with_capacity(published.sources.len());
    for source in &published.sources {
        if source_versions
            .insert(source.source_version_id.as_str(), source)
            .is_some()
        {
            return Err(invalid(
                "discovery.sources",
                "duplicate source version identity",
            ));
        }
        let projected = project_discovery_source(source)?;
        if !stable_source_ids.insert(projected.source_id.as_str().to_owned()) {
            return Err(invalid(
                "discovery.sources",
                "duplicate stable source identity",
            ));
        }
        sources.push(projected);
    }

    let observation_id = publication.map(|value| value.observation_id);
    let terms = published
        .snapshot
        .terms
        .iter()
        .map(|term| (term.term_id.clone(), term))
        .collect::<BTreeMap<_, _>>();
    if terms.len() != published.snapshot.terms.len() {
        return Err(invalid("discovery.terms", "duplicate term identity"));
    }

    let mut target_source_versions = BTreeMap::new();
    let mut targets = Vec::with_capacity(published.snapshot.campuses.len());
    for campus in &published.snapshot.campuses {
        if target_source_versions
            .insert(campus.target.clone(), campus.source_version_id.clone())
            .is_some()
        {
            return Err(invalid("discovery.targets", "duplicate target identity"));
        }
        let term = terms.get(campus.target.term()).ok_or_else(|| {
            invalid(
                "discovery.targets",
                "campus target references an absent term",
            )
        })?;
        if term.source_version_id != campus.source_version_id {
            return Err(invalid(
                "discovery.targets.provenance",
                "term and campus ownership differ for one target",
            ));
        }
        let observation_id = observation_id.ok_or_else(|| {
            invalid(
                "discovery.targets.provenance",
                "published entities require a published observation",
            )
        })?;
        let source = source_versions
            .get(campus.source_version_id.as_str())
            .ok_or_else(|| {
                invalid(
                    "discovery.targets.provenance",
                    "entity references an absent source version",
                )
            })?;
        require_discovery_schema(&term.canonical_facts, "discovery-term-v1", "term")?;
        require_discovery_schema(&campus.canonical_facts, "discovery-campus-v1", "campus")?;
        let selectable =
            discovery_fact_is_true(&term.canonical_facts, "published", "term.published")?
                && discovery_fact_is_true(
                    &campus.canonical_facts,
                    "campusEnabled",
                    "campus.campusEnabled",
                )?
                && discovery_fact_is_true(
                    &campus.canonical_facts,
                    "targetEnabled",
                    "campus.targetEnabled",
                )?;
        if !selectable {
            continue;
        }
        targets.push(CatalogTargetV1 {
            key: campus.target.clone(),
            term_label: discovery_fact(&term.canonical_facts, "display", "term.display")?,
            campus_label: discovery_fact(
                &campus.canonical_facts,
                "campusDisplay",
                "campus.campusDisplay",
            )?,
            provenance: discovery_provenance(observation_id, source)?,
        });
    }

    let selectable_target_keys = targets
        .iter()
        .map(|target| target.key.clone())
        .collect::<BTreeSet<_>>();
    let mut subject_keys = BTreeSet::new();
    let mut subjects = Vec::with_capacity(published.snapshot.subjects.len());
    for subject in &published.snapshot.subjects {
        let target = TermCampusKey::new(subject.term_id.clone(), subject.campus_code.clone());
        let target_source_version = target_source_versions
            .get(&target)
            .ok_or_else(|| invalid("discovery.subjects", "subject references an absent target"))?;
        if target_source_version != &subject.source_version_id {
            return Err(invalid(
                "discovery.subjects.provenance",
                "subject and target ownership differ",
            ));
        }
        if !subject_keys.insert((target.clone(), subject.subject_code.clone())) {
            return Err(invalid("discovery.subjects", "duplicate subject identity"));
        }
        let observation_id = observation_id.ok_or_else(|| {
            invalid(
                "discovery.subjects.provenance",
                "published entities require a published observation",
            )
        })?;
        let source = source_versions
            .get(subject.source_version_id.as_str())
            .ok_or_else(|| {
                invalid(
                    "discovery.subjects.provenance",
                    "entity references an absent source version",
                )
            })?;
        require_discovery_schema(&subject.canonical_facts, "discovery-subject-v1", "subject")?;
        if !selectable_target_keys.contains(&target)
            || !discovery_fact_is_true(&subject.canonical_facts, "enabled", "subject.enabled")?
        {
            continue;
        }
        subjects.push(CatalogSubjectV1 {
            target,
            code: CatalogSubjectCode::try_from(subject.subject_code.as_str()).map_err(|_| {
                invalid(
                    "discovery.subjects.code",
                    "subject code violates the public contract",
                )
            })?,
            label: discovery_fact(&subject.canonical_facts, "display", "subject.display")?,
            provenance: discovery_provenance(observation_id, source)?,
        });
    }

    validate_discovery_counts(published)?;
    Ok(CatalogDiscoveryResponseV1 {
        contract_version: CATALOG_CONTRACT_VERSION,
        observed_at: projected_at,
        status: discovery_status(&published.state)?,
        sources,
        targets,
        subjects,
    })
}

pub fn to_normalized_catalog_v1(
    published: &PublishedCatalogSnapshot,
) -> Result<NormalizedCatalogV1, ProjectionError> {
    validate_catalog_publication(published)?;
    let target = &published.target;
    let snapshot = &published.snapshot;

    let mut group_keys = BTreeSet::new();
    for group in &snapshot.course_groups {
        if group.key.target() != *target || !group_keys.insert(group.key.clone()) {
            return Err(invalid(
                "catalog.courseGroups",
                "group target mismatch or duplicate identity",
            ));
        }
    }
    let mut variant_keys = BTreeSet::new();
    for variant in &snapshot.course_variants {
        if variant.key.group().target() != *target
            || !group_keys.contains(variant.key.group())
            || !variant_keys.insert(variant.key.clone())
        {
            return Err(invalid(
                "catalog.courseVariants",
                "variant target mismatch, orphan, or duplicate identity",
            ));
        }
    }
    let mut section_keys = BTreeSet::new();
    for section in &snapshot.sections {
        if section.key.target() != *target
            || !variant_keys.contains(&section.variant_key)
            || !section_keys.insert(section.key.clone())
        {
            return Err(invalid(
                "catalog.sections",
                "section target mismatch, orphan, or duplicate identity",
            ));
        }
    }
    let mut occurrence_keys = BTreeSet::new();
    for occurrence in &snapshot.occurrences {
        if !section_keys.contains(&occurrence.section_key)
            || !occurrence_keys.insert((occurrence.section_key.clone(), occurrence.ordinal))
        {
            return Err(invalid(
                "catalog.occurrences",
                "occurrence is orphaned or has a duplicate ordinal",
            ));
        }
    }

    let mut course_groups = Vec::with_capacity(snapshot.course_groups.len());
    for group in &snapshot.course_groups {
        let keys = snapshot
            .course_variants
            .iter()
            .filter(|variant| variant.key.group() == &group.key)
            .map(|variant| variant.key.clone())
            .collect::<Vec<_>>();
        validate_group_facts(group, &keys)?;
        course_groups.push(NormalizedCourseGroupV1 {
            key: group.key.clone(),
            variant_keys: keys,
        });
    }

    let mut course_variants = Vec::with_capacity(snapshot.course_variants.len());
    for variant in &snapshot.course_variants {
        let section_keys = snapshot
            .sections
            .iter()
            .filter(|section| section.variant_key == variant.key)
            .map(|section| section.key.clone())
            .collect::<Vec<_>>();
        course_variants.push(project_variant(variant, section_keys)?);
    }

    let mut sections = Vec::with_capacity(snapshot.sections.len());
    for section in &snapshot.sections {
        let occurrences = snapshot
            .occurrences
            .iter()
            .filter(|occurrence| occurrence.section_key == section.key)
            .collect::<Vec<_>>();
        sections.push(project_section(section, &occurrences)?);
    }
    let occurrences = snapshot
        .occurrences
        .iter()
        .map(project_occurrence)
        .collect::<Result<Vec<_>, _>>()?;

    let publication = &published.publication;
    let observed_at = parse_timestamp(
        "catalog.publication.completedAt",
        publication.completed_at.as_deref().ok_or_else(|| {
            invalid(
                "catalog.publication.completedAt",
                "published observation is incomplete",
            )
        })?,
    )?;
    let payload_digest = required_digest(
        "catalog.publication.sourceContentSha256",
        publication.source_content_sha256.as_deref(),
    )?;
    Ok(NormalizedCatalogV1 {
        contract_version: CATALOG_CONTRACT_VERSION,
        target: target.clone(),
        content_version: content_version("catalog.contentVersion", published.content_version)?,
        provenance: CatalogProvenanceV1 {
            observation_id: publication.observation_id,
            source: CatalogSourceKind::RutgersCatalog,
            target: target.clone(),
            observed_at,
            payload_digest,
        },
        course_groups,
        course_variants,
        sections,
        occurrences,
    })
}

pub fn to_catalog_refresh_observation_v1(
    observation: &RefreshObservation,
) -> Result<CatalogRefreshObservationV1, ProjectionError> {
    let classification = refresh_classification(observation.status);
    if matches!(
        observation.status,
        RefreshStatus::Started | RefreshStatus::Staged
    ) {
        return Err(ProjectionError::IncompleteRefreshObservation {
            observation_id: observation.observation_id,
            status: refresh_status_name(observation.status),
        });
    }
    let finished_at = parse_timestamp(
        "refresh.completedAt",
        observation.completed_at.as_deref().ok_or_else(|| {
            invalid(
                "refresh.completedAt",
                "completed observation has no completion time",
            )
        })?,
    )?;
    let error_class = match observation.status {
        RefreshStatus::Failed => Some(refresh_failure_class(
            observation
                .error_stage
                .as_deref()
                .ok_or_else(|| invalid("refresh.errorStage", "failed observation has no stage"))?,
        )?),
        _ => {
            if observation.error_stage.is_some() || observation.error_code.is_some() {
                return Err(invalid(
                    "refresh.error",
                    "nonfailed observation contains failure metadata",
                ));
            }
            None
        }
    };
    if observation.status == RefreshStatus::Failed {
        diagnostic_code(
            "refresh.errorCode",
            observation.error_code.as_deref().ok_or_else(|| {
                invalid("refresh.errorCode", "failed observation has no error code")
            })?,
        )?;
    }
    let partial_reasons = if observation.status == RefreshStatus::EmptySuspectRetained {
        vec![CatalogUnknownReason::SparseEvidence]
    } else {
        Vec::new()
    };
    Ok(CatalogRefreshObservationV1 {
        contract_version: CATALOG_CONTRACT_VERSION,
        observation_id: observation.observation_id,
        target: observation.target.clone(),
        started_at: parse_timestamp("refresh.startedAt", &observation.started_at)?,
        finished_at,
        classification,
        content_version: optional_content_version(
            "refresh.resultingContentVersion",
            observation.resulting_content_version,
        )?,
        payload_digest: optional_digest(
            "refresh.sourceContentSha256",
            observation.source_content_sha256.as_deref(),
        )?,
        counts: entity_counts(observation.counts),
        error_class,
        partial_reasons,
    })
}

pub fn to_catalog_refresh_checkpoint_v1(
    state: &TargetState,
) -> Result<CatalogRefreshCheckpointV1, ProjectionError> {
    validate_target_state(state)?;
    let latest = state.last_attempt.as_ref().ok_or_else(|| {
        invalid(
            "refreshCheckpoint.latestAttempt",
            "target state has no latest attempt",
        )
    })?;
    Ok(CatalogRefreshCheckpointV1 {
        contract_version: CATALOG_CONTRACT_VERSION,
        target: state.target.clone(),
        latest_attempt: checkpoint_point(latest)?,
        last_success: state
            .last_success
            .as_ref()
            .map(checkpoint_point)
            .transpose()?,
        last_published: state
            .last_published
            .as_ref()
            .map(checkpoint_point)
            .transpose()?,
        last_nonempty: state
            .last_nonempty
            .as_ref()
            .map(checkpoint_point)
            .transpose()?,
        pending_empty: state
            .pending_empty
            .as_ref()
            .map(checkpoint_point)
            .transpose()?,
    })
}

pub fn to_catalog_refresh_status_v1(
    state: &TargetState,
) -> Result<CatalogRefreshStatusV1, ProjectionError> {
    validate_target_state(state)?;
    let latest = state.last_attempt.as_ref().ok_or_else(|| {
        invalid(
            "refreshStatus.latestAttempt",
            "target state has no latest attempt",
        )
    })?;
    Ok(CatalogRefreshStatusV1 {
        contract_version: CATALOG_CONTRACT_VERSION,
        target: state.target.clone(),
        latest_attempt: refresh_point(latest)?,
        last_success: state.last_success.as_ref().map(refresh_point).transpose()?,
        last_published: state
            .last_published
            .as_ref()
            .map(refresh_point)
            .transpose()?,
        last_nonempty: state
            .last_nonempty
            .as_ref()
            .map(refresh_point)
            .transpose()?,
        pending_empty: state
            .pending_empty
            .as_ref()
            .map(refresh_point)
            .transpose()?,
    })
}

fn project_discovery_source(
    source: &DiscoverySourceVersion,
) -> Result<CatalogDiscoverySourceV1, ProjectionError> {
    let source_kind = discovery_source_kind(source.source_kind);
    let expected_identity = match source.source_kind {
        DiscoverySourceKind::Selector => "RUTGERS_SELECTOR",
        DiscoverySourceKind::Bootstrap => "RBCSP_BOOTSTRAP",
    };
    let expected_prefix = match source.source_kind {
        DiscoverySourceKind::Selector => "selector:",
        DiscoverySourceKind::Bootstrap => "bootstrap:",
    };
    if source.source_identity != expected_identity
        || source.source_version_id != format!("{expected_prefix}{}", source.content_sha256)
    {
        return Err(invalid(
            "discovery.sources.identity",
            "source kind, stable identity, version, and digest disagree",
        ));
    }
    require_discovery_schema(&source.canonical_facts, "discovery-source-v1", "source")?;
    Ok(CatalogDiscoverySourceV1 {
        source_id: CatalogDiscoverySourceId::try_from(source.source_identity.as_str()).map_err(
            |_| {
                invalid(
                    "discovery.sources.sourceId",
                    "stable source identity violates the public contract",
                )
            },
        )?,
        source_kind,
        source_version: discovery_fact(
            &source.canonical_facts,
            "reportedSourceVersion",
            "source.reportedSourceVersion",
        )?,
        payload_digest: digest("discovery.sources.payloadDigest", &source.content_sha256)?,
        observed_at: parse_timestamp("discovery.sources.observedAt", &source.observed_at)?,
    })
}

fn discovery_provenance(
    observation_id: TraceId,
    source: &DiscoverySourceVersion,
) -> Result<CatalogDiscoveryProvenanceV1, ProjectionError> {
    Ok(CatalogDiscoveryProvenanceV1 {
        observation_id,
        source_id: CatalogDiscoverySourceId::try_from(source.source_identity.as_str()).map_err(
            |_| {
                invalid(
                    "discovery.provenance.sourceId",
                    "stable source identity violates the public contract",
                )
            },
        )?,
        source_kind: discovery_source_kind(source.source_kind),
        observed_at: parse_timestamp("discovery.provenance.observedAt", &source.observed_at)?,
        payload_digest: digest("discovery.provenance.payloadDigest", &source.content_sha256)?,
    })
}

fn discovery_status(state: &DiscoveryState) -> Result<CatalogDiscoveryStatusV1, ProjectionError> {
    let error = match state.last_attempt.as_ref() {
        Some(observation) if observation.status == DiscoveryStatus::Failed => {
            Some(CatalogDiscoveryErrorV1 {
                class: discovery_failure_class(observation.error_stage.as_deref().ok_or_else(
                    || {
                        invalid(
                            "discovery.status.errorStage",
                            "failed attempt has no failure stage",
                        )
                    },
                )?)?,
                code: diagnostic_code(
                    "discovery.status.errorCode",
                    observation.error_code.as_deref().ok_or_else(|| {
                        invalid(
                            "discovery.status.errorCode",
                            "failed attempt has no error code",
                        )
                    })?,
                )?,
            })
        }
        _ => None,
    };
    Ok(CatalogDiscoveryStatusV1 {
        availability: match state.availability {
            DiscoveryAvailability::Fresh => CatalogDiscoveryAvailability::Current,
            DiscoveryAvailability::Stale => CatalogDiscoveryAvailability::StaleLastSuccess,
            DiscoveryAvailability::UnavailableNoFirstSuccess => {
                CatalogDiscoveryAvailability::UnavailableNoFirstSuccess
            }
        },
        latest_attempt: state
            .last_attempt
            .as_ref()
            .map(discovery_point)
            .transpose()?,
        last_success: state
            .last_success
            .as_ref()
            .map(discovery_point)
            .transpose()?,
        is_stale: state.is_stale,
        error,
    })
}

fn discovery_point(
    observation: &DiscoveryObservation,
) -> Result<CatalogDiscoveryPointV1, ProjectionError> {
    Ok(CatalogDiscoveryPointV1 {
        observation_id: observation.observation_id,
        observed_at: parse_timestamp(
            "discovery.observation.observedAt",
            observation
                .completed_at
                .as_deref()
                .unwrap_or(&observation.started_at),
        )?,
        content_version: optional_content_version(
            "discovery.observation.contentVersion",
            observation.resulting_content_version,
        )?,
    })
}

fn project_variant(
    stored: &StoredCourseVariant,
    section_keys: Vec<bcsp_contracts::SectionKey>,
) -> Result<NormalizedCourseVariantV1, ProjectionError> {
    let raw: RawCatalogCourse = decode_facts(
        "catalog.courseVariant.canonicalFacts",
        &stored.canonical_facts,
    )?;
    if raw.course_string.value().map(String::as_str)
        != Some(stored.key.group().course_string().as_str())
    {
        return Err(invalid(
            "catalog.courseVariant.courseString",
            "canonical identity does not match the stored key",
        ));
    }
    let canonical_hash =
        canonical_course_variant_hash_v1(&stored.canonical_facts).map_err(|error| {
            invalid(
                "catalog.courseVariant.canonicalSha256",
                format!("cannot hash canonical facts: {error}"),
            )
        })?;
    if canonical_hash.as_str() != stored.canonical_sha256 {
        return Err(invalid(
            "catalog.courseVariant.canonicalSha256",
            "canonical digest does not match canonical facts",
        ));
    }
    let fingerprint = variant_fingerprint_v1(&raw).map_err(|_| {
        invalid(
            "catalog.courseVariant.fingerprint",
            "cannot derive the frozen variant fingerprint from canonical facts",
        )
    })?;
    if fingerprint != stored.key.fingerprint().as_str() {
        return Err(invalid(
            "catalog.courseVariant.fingerprint",
            "canonical identity facts do not match the stored fingerprint",
        ));
    }

    let prerequisite_notes = string_knowledge(&raw.pre_req_notes)?;
    let prerequisite_state = match &raw.pre_req_notes {
        Presence::Value(value) if value.is_empty() => CatalogPrerequisiteState::NoneReported,
        Presence::Value(_) => CatalogPrerequisiteState::Has,
        Presence::Missing | Presence::Null | Presence::Malformed(_) => {
            CatalogPrerequisiteState::Unknown
        }
    };
    Ok(NormalizedCourseVariantV1 {
        key: stored.key.clone(),
        title: string_knowledge(&raw.title)?,
        expanded_title: string_knowledge(&raw.expanded_title)?,
        description: string_knowledge(&raw.course_description)?,
        notes: string_knowledge(&raw.course_notes)?,
        subject_group_notes: string_knowledge(&raw.subject_group_notes)?,
        subject_notes: string_knowledge(&raw.subject_notes)?,
        unit_notes: string_knowledge(&raw.unit_notes)?,
        synopsis_url: string_knowledge(&raw.synopsis_url)?,
        prerequisite_notes,
        prerequisite_state,
        credits: credits_knowledge(&raw)?,
        level: string_knowledge(&raw.level)?,
        subject_code: string_knowledge(&raw.subject)?,
        subject_description: string_knowledge(&raw.subject_description)?,
        course_number: string_knowledge(&raw.course_number)?,
        supplement_code: string_knowledge(&raw.supplement_code)?,
        school_code: object_string_field_knowledge(&raw.school, "code")?,
        offering_unit: string_knowledge(&raw.offering_unit_code)?,
        offering_unit_title: string_knowledge(&raw.offering_unit_title)?,
        core_codes: value_array_knowledge(&raw.core_codes, array_string)?,
        campus_locations: value_array_knowledge(&raw.campus_locations, campus_location_code)?,
        section_keys,
    })
}

fn project_section(
    stored: &StoredSection,
    occurrences: &[&StoredOccurrence],
) -> Result<NormalizedSectionV1, ProjectionError> {
    let raw: RawCatalogSection =
        decode_facts("catalog.section.canonicalFacts", &stored.canonical_facts)?;
    if raw.index.value().map(String::as_str) != Some(stored.key.index().as_str()) {
        return Err(invalid(
            "catalog.section.index",
            "canonical identity does not match the stored key",
        ));
    }
    let semantic_hash =
        canonical_section_semantic_hash_v1(&stored.canonical_facts).map_err(|error| {
            invalid(
                "catalog.section.canonicalSha256",
                format!("cannot hash canonical facts: {error}"),
            )
        })?;
    if semantic_hash.as_str() != stored.canonical_sha256 {
        return Err(invalid(
            "catalog.section.canonicalSha256",
            "semantic digest does not match canonical facts",
        ));
    }
    if raw.number.value().map(|value| value.trim().to_owned()) != stored.section_number {
        return Err(invalid(
            "catalog.section.number",
            "flattened value does not match canonical facts",
        ));
    }
    if raw
        .section_course_type
        .value()
        .map(|value| value.trim().to_owned())
        != stored.section_course_type
    {
        return Err(invalid(
            "catalog.section.sectionCourseType",
            "flattened value does not match canonical facts",
        ));
    }
    let expected_status = match raw.open_status {
        Presence::Value(true) => Some("CATALOG_SNAPSHOT_OPEN"),
        Presence::Value(false) => Some("CATALOG_SNAPSHOT_CLOSED"),
        Presence::Missing | Presence::Null | Presence::Malformed(_) => None,
    };
    if stored.catalog_status.as_deref() != expected_status {
        return Err(invalid(
            "catalog.section.catalogStatus",
            "flattened value does not match canonical facts",
        ));
    }

    let normalized_occurrences = match &raw.meeting_times {
        Presence::Value(values) => values
            .iter()
            .enumerate()
            .map(|(ordinal, value)| normalize_occurrence(value, ordinal))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| {
                invalid(
                    "catalog.section.meetingTimes",
                    format!("cannot normalize canonical occurrences: {error}"),
                )
            })?,
        Presence::Missing | Presence::Null | Presence::Malformed(_) => Vec::new(),
    };
    if normalized_occurrences.len() != occurrences.len() {
        return Err(invalid(
            "catalog.section.meetingTimes",
            "canonical occurrence count does not match stored rows",
        ));
    }
    let canonical_meetings = match stored.canonical_facts.get("meetingTimes") {
        Some(Value::Array(values)) => values.as_slice(),
        None | Some(Value::Null) | Some(Value::Object(_)) if occurrences.is_empty() => &[],
        _ => {
            return Err(invalid(
                "catalog.section.meetingTimes",
                "canonical meeting collection shape is invalid",
            ));
        }
    };
    for expected in &normalized_occurrences {
        let ordinal = u32::try_from(expected.source_ordinal).map_err(|_| {
            invalid(
                "catalog.section.meetingTimes",
                "occurrence ordinal does not fit the public contract",
            )
        })?;
        let actual = occurrences
            .iter()
            .find(|occurrence| occurrence.ordinal == ordinal)
            .ok_or_else(|| {
                invalid(
                    "catalog.section.meetingTimes",
                    "canonical occurrence has no stored row",
                )
            })?;
        if canonical_meetings.get(expected.source_ordinal) != Some(&actual.canonical_facts) {
            return Err(invalid(
                "catalog.section.meetingTimes",
                "stored occurrence facts differ from their section source",
            ));
        }
    }
    let (delivery_modality, synchronicity) =
        classify_delivery(&raw.section_course_type, &normalized_occurrences);
    let expected_delivery = CatalogModality::from(delivery_modality);
    let expected_synchronicity = CatalogSynchronicity::from(synchronicity);
    if stored.delivery_modality != expected_delivery.wire_name()
        || stored.synchronicity != expected_synchronicity.wire_name()
    {
        return Err(invalid(
            "catalog.section.delivery",
            "flattened delivery classification does not match canonical facts",
        ));
    }

    let instructors = instructors_knowledge(&raw.instructors)?;
    let (major_codes, unit_codes) = majors_knowledge(&raw.majors)?;
    let comments = comments_knowledge(&raw.comments)?;
    let comments_text = comments_text_knowledge(&raw.comments, &raw.comments_text, &comments)?;
    let cross_listed_sections_text = cross_listed_sections_text_knowledge(
        &raw.cross_listed_sections,
        &raw.cross_listed_sections_text,
    )?;
    let mut occurrence_keys = occurrences
        .iter()
        .map(|occurrence| CatalogOccurrenceKeyV1 {
            section: stored.key.clone(),
            ordinal: occurrence.ordinal,
        })
        .collect::<Vec<_>>();
    occurrence_keys.sort_by_key(|key| key.ordinal);
    let occurrence_keys = match &raw.meeting_times {
        Presence::Missing => CatalogFieldKnowledge::absent(),
        Presence::Null => CatalogFieldKnowledge::explicit_null(),
        Presence::Malformed(malformed) => {
            validate_malformed_digest(malformed.canonical_sha256.as_str())?;
            CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed)
        }
        Presence::Value(_) => CatalogFieldKnowledge::present(occurrence_keys),
    };
    Ok(NormalizedSectionV1 {
        key: stored.key.clone(),
        variant_key: stored.variant_key.clone(),
        section_number: semantic_string_knowledge(&raw.number)?,
        subtitle: semantic_string_knowledge(&raw.subtitle)?,
        subtopic: semantic_string_knowledge(&raw.subtopic)?,
        section_notes: semantic_string_knowledge(&raw.section_notes)?,
        session_dates: semantic_string_knowledge(&raw.session_dates)?,
        session_date_print_indicator: semantic_string_knowledge(&raw.session_date_print_indicator)?,
        comments,
        comments_text,
        cross_listed_sections_text,
        cross_listed_section_type: semantic_string_knowledge(&raw.cross_listed_section_type)?,
        instructors,
        instructor_reliability: CatalogInstructorReliability::NameOnly,
        raw_section_course_type: semantic_string_knowledge(&raw.section_course_type)?,
        delivery_modality: expected_delivery,
        synchronicity: expected_synchronicity,
        exam_code: semantic_string_knowledge(&raw.exam_code)?,
        exam_code_text: semantic_string_knowledge(&raw.exam_code_text)?,
        special_permission_add_code: semantic_string_knowledge(&raw.special_permission_add_code)?,
        special_permission_add_description: semantic_string_knowledge(
            &raw.special_permission_add_code_description,
        )?,
        special_permission_drop_code: semantic_string_knowledge(&raw.special_permission_drop_code)?,
        special_permission_drop_description: semantic_string_knowledge(
            &raw.special_permission_drop_code_description,
        )?,
        major_codes,
        unit_codes,
        minor_codes: value_array_knowledge(&raw.minors, structured_code)?,
        honor_program_codes: value_array_knowledge(&raw.honor_programs, structured_code)?,
        unit_majors: unit_majors_knowledge(&raw.unit_majors)?,
        eligibility_text: semantic_string_knowledge(&raw.section_eligibility)?,
        open_to_text: semantic_string_knowledge(&raw.open_to_text)?,
        catalog_open_status: CatalogSnapshotOpenStatusV1 {
            value: bool_knowledge(&raw.open_status)?,
            provenance: CatalogOpenStatusProvenance::CatalogSnapshotOnly,
        },
        occurrence_keys,
    })
}

fn project_occurrence(
    stored: &StoredOccurrence,
) -> Result<NormalizedOccurrenceV1, ProjectionError> {
    let raw: RawMeeting =
        decode_facts("catalog.occurrence.canonicalFacts", &stored.canonical_facts)?;
    let normalized = normalize_occurrence(
        &raw,
        usize::try_from(stored.ordinal).map_err(|_| {
            invalid(
                "catalog.occurrence.ordinal",
                "ordinal does not fit the normalization model",
            )
        })?,
    )
    .map_err(|error| {
        invalid(
            "catalog.occurrence.canonicalFacts",
            format!("cannot normalize canonical facts: {error}"),
        )
    })?;
    let raw_hash = canonical_occurrence_hash_v1(&stored.canonical_facts).map_err(|error| {
        invalid(
            "catalog.occurrence.rawSha256",
            format!("cannot hash canonical facts: {error}"),
        )
    })?;
    let expected_key = format!("{:010}-{raw_hash}", stored.ordinal);
    let expected_modality = CatalogModality::from(normalized.modality);
    let expected_synchronicity = CatalogSynchronicity::from(normalized.synchronicity);
    let expected_requiredness = CatalogRequiredness::from(normalized.requiredness);
    let expected_kind = CatalogOccurrenceKind::from(normalized.kind);
    let expected_evidence = CatalogOccurrenceEvidence::from(normalized.evidence);
    let (expected_start, expected_end) = match normalized.time {
        crate::TimeKnowledge::Known {
            start_minute,
            end_minute,
        } => (Some(start_minute), Some(end_minute)),
        _ => (None, None),
    };
    if stored.occurrence_key != expected_key
        || stored.raw_sha256 != raw_hash.as_str()
        || stored.weekday != weekday_text(&normalized.days)
        || stored.start_minute != expected_start
        || stored.end_minute != expected_end
        || stored.time_knowledge != time_knowledge_name(normalized.time)
        || stored.requiredness != expected_requiredness.wire_name()
        || stored.occurrence_kind != expected_kind.wire_name()
        || stored.evidence != expected_evidence.wire_name()
        || stored.normalization_reason != normalized.normalization_reason
        || stored.modality != expected_modality.wire_name()
        || stored.synchronicity != expected_synchronicity.wire_name()
        || stored.location != raw.campus_location.value().cloned()
        || stored.building != raw.building_code.value().cloned()
        || stored.room != raw.room_number.value().cloned()
    {
        return Err(invalid(
            "catalog.occurrence",
            "flattened row does not match canonical normalization",
        ));
    }

    Ok(NormalizedOccurrenceV1 {
        key: CatalogOccurrenceKeyV1 {
            section: stored.section_key.clone(),
            ordinal: stored.ordinal,
        },
        raw_code: string_knowledge(&raw.meeting_mode_code)?,
        raw_description: string_knowledge(&raw.meeting_mode_desc)?,
        modality: normalized_enum_knowledge(&raw.meeting_mode_code, expected_modality)?,
        synchronicity: normalized_enum_knowledge(&raw.meeting_mode_code, expected_synchronicity)?,
        raw_day: string_knowledge(&raw.meeting_day)?,
        days: days_knowledge(&normalized.days),
        raw_start_time: string_knowledge(&raw.start_time_military)?,
        raw_end_time: string_knowledge(&raw.end_time_military)?,
        time: CatalogTimeKnowledgeV1::from(normalized.time),
        start_date: CatalogFieldKnowledge::unknown(CatalogUnknownReason::NotObserved),
        end_date: CatalogFieldKnowledge::unknown(CatalogUnknownReason::NotObserved),
        campus: string_knowledge(&raw.campus_location)?,
        campus_name: string_knowledge(&raw.campus_name)?,
        building: string_knowledge(&raw.building_code)?,
        room: string_knowledge(&raw.room_number)?,
        requiredness: expected_requiredness,
        kind: expected_kind,
        evidence: expected_evidence,
        normalization_reason: diagnostic_code(
            "catalog.occurrence.normalizationReason",
            &stored.normalization_reason,
        )?,
    })
}

fn validate_group_facts(
    stored: &StoredCourseGroup,
    variant_keys: &[bcsp_contracts::CourseVariantKey],
) -> Result<(), ProjectionError> {
    let object = object(
        "catalog.courseGroup.canonicalFacts",
        &stored.canonical_facts,
    )?;
    if object.get("schema").and_then(Value::as_str) != Some("catalog-group-v1")
        || object.get("courseString").and_then(Value::as_str)
            != Some(stored.key.course_string().as_str())
    {
        return Err(invalid(
            "catalog.courseGroup.canonicalFacts",
            "schema or identity is invalid",
        ));
    }
    let fingerprints = object.get("variantFingerprints").cloned().ok_or_else(|| {
        invalid(
            "catalog.courseGroup.canonicalFacts",
            "variant fingerprint list is missing",
        )
    })?;
    let fingerprints: Vec<String> = serde_json::from_value(fingerprints).map_err(|error| {
        invalid(
            "catalog.courseGroup.canonicalFacts",
            format!("invalid variant fingerprint list: {error}"),
        )
    })?;
    let expected = variant_keys
        .iter()
        .map(|key| key.fingerprint().as_str().to_owned())
        .collect::<Vec<_>>();
    if fingerprints != expected {
        return Err(invalid(
            "catalog.courseGroup.canonicalFacts",
            "variant fingerprint list does not match stored variants",
        ));
    }
    Ok(())
}

fn validate_catalog_publication(
    published: &PublishedCatalogSnapshot,
) -> Result<(), ProjectionError> {
    let publication = &published.publication;
    if publication.target != published.target {
        return Err(invalid(
            "catalog.publication.target",
            "publication target differs from snapshot target",
        ));
    }
    if !matches!(
        publication.status,
        RefreshStatus::AppliedChanged
            | RefreshStatus::AppliedUnchanged
            | RefreshStatus::EmptyValidInitial
    ) {
        return Err(invalid(
            "catalog.publication.status",
            "snapshot is not backed by a publishing observation",
        ));
    }
    if publication.resulting_content_version != Some(published.content_version) {
        return Err(invalid(
            "catalog.publication.contentVersion",
            "publication version differs from snapshot version",
        ));
    }
    if publication.counts != published.snapshot.counts() {
        return Err(invalid(
            "catalog.publication.counts",
            "publication counts differ from snapshot rows",
        ));
    }
    required_digest(
        "catalog.publication.sourceContentSha256",
        publication.source_content_sha256.as_deref(),
    )?;
    for provenance in &published.snapshot.provenance {
        digest("catalog.provenance.sourceSha256", &provenance.source_sha256)?;
    }
    to_catalog_refresh_observation_v1(publication)?;
    Ok(())
}

fn validate_discovery_state(state: &DiscoveryState) -> Result<(), ProjectionError> {
    validate_marker(
        "discovery.state.lastAttempt",
        state.last_attempt_observation_id.as_ref(),
        state
            .last_attempt
            .as_ref()
            .map(|value| &value.observation_id),
    )?;
    validate_marker(
        "discovery.state.lastSuccess",
        state.last_success_observation_id.as_ref(),
        state
            .last_success
            .as_ref()
            .map(|value| &value.observation_id),
    )?;
    validate_marker(
        "discovery.state.lastPublished",
        state.last_published_observation_id.as_ref(),
        state
            .last_published
            .as_ref()
            .map(|value| &value.observation_id),
    )?;
    validate_marker(
        "discovery.state.lastNonempty",
        state.last_nonempty_observation_id.as_ref(),
        state
            .last_nonempty
            .as_ref()
            .map(|value| &value.observation_id),
    )?;
    if state.is_stale != (state.availability == DiscoveryAvailability::Stale) {
        return Err(invalid(
            "discovery.state.availability",
            "stale flag and availability disagree",
        ));
    }
    if state.availability == DiscoveryAvailability::UnavailableNoFirstSuccess
        && (state.last_success.is_some()
            || state.last_published.is_some()
            || state.content_version != 0)
    {
        return Err(invalid(
            "discovery.state.availability",
            "no-first-success state contains published data",
        ));
    }
    if state.availability != DiscoveryAvailability::UnavailableNoFirstSuccess
        && (state.last_success.is_none()
            || state.last_published.is_none()
            || state.content_version == 0)
    {
        return Err(invalid(
            "discovery.state.availability",
            "available state lacks successful published data",
        ));
    }
    if let Some(last_success) = &state.last_success
        && !matches!(
            last_success.status,
            DiscoveryStatus::AppliedChanged | DiscoveryStatus::AppliedUnchanged
        )
    {
        return Err(invalid(
            "discovery.state.lastSuccess",
            "success marker points to a nonsuccess observation",
        ));
    }
    Ok(())
}

fn validate_discovery_counts(
    published: &PublishedDiscoverySnapshot,
) -> Result<(), ProjectionError> {
    if published.state.term_count != published.snapshot.terms.len() as u64
        || published.state.campus_count != published.snapshot.campuses.len() as u64
        || published.state.subject_count != published.snapshot.subjects.len() as u64
    {
        return Err(invalid(
            "discovery.state.counts",
            "state counts differ from published rows",
        ));
    }
    Ok(())
}

fn validate_target_state(state: &TargetState) -> Result<(), ProjectionError> {
    validate_marker(
        "refreshState.lastAttempt",
        state.last_attempt_observation_id.as_ref(),
        state
            .last_attempt
            .as_ref()
            .map(|value| &value.observation_id),
    )?;
    validate_marker(
        "refreshState.lastSuccess",
        state.last_success_observation_id.as_ref(),
        state
            .last_success
            .as_ref()
            .map(|value| &value.observation_id),
    )?;
    validate_marker(
        "refreshState.lastPublished",
        state.last_published_observation_id.as_ref(),
        state
            .last_published
            .as_ref()
            .map(|value| &value.observation_id),
    )?;
    validate_marker(
        "refreshState.lastNonempty",
        state.last_nonempty_observation_id.as_ref(),
        state
            .last_nonempty
            .as_ref()
            .map(|value| &value.observation_id),
    )?;
    validate_marker(
        "refreshState.pendingEmpty",
        state.pending_empty_observation_id.as_ref(),
        state
            .pending_empty
            .as_ref()
            .map(|value| &value.observation_id),
    )?;
    for observation in [
        state.last_attempt.as_ref(),
        state.last_success.as_ref(),
        state.last_published.as_ref(),
        state.last_nonempty.as_ref(),
        state.pending_empty.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        if observation.target != state.target {
            return Err(invalid(
                "refreshState.observation",
                "marker points outside the target",
            ));
        }
    }
    if state
        .last_attempt
        .as_ref()
        .is_some_and(|value| value.attempt_sequence != state.last_attempt_sequence)
    {
        return Err(invalid(
            "refreshState.lastAttemptSequence",
            "sequence differs from the latest attempt",
        ));
    }
    if state
        .last_success
        .as_ref()
        .is_some_and(|value| !is_refresh_success(value.status))
        || state.last_published.as_ref().is_some_and(|value| {
            !matches!(
                value.status,
                RefreshStatus::AppliedChanged
                    | RefreshStatus::AppliedUnchanged
                    | RefreshStatus::EmptyValidInitial
            )
        })
        || state.last_nonempty.as_ref().is_some_and(|value| {
            !matches!(
                value.status,
                RefreshStatus::AppliedChanged | RefreshStatus::AppliedUnchanged
            ) || value.counts == CatalogCounts::default()
        })
        || state
            .pending_empty
            .as_ref()
            .is_some_and(|value| value.status != RefreshStatus::EmptySuspectRetained)
    {
        return Err(invalid(
            "refreshState.markers",
            "marker classification violates the refresh chain",
        ));
    }
    Ok(())
}

fn refresh_point(
    observation: &RefreshObservation,
) -> Result<CatalogRefreshPointV1, ProjectionError> {
    Ok(CatalogRefreshPointV1 {
        observation_id: observation.observation_id,
        observed_at: refresh_point_time(observation)?,
        classification: refresh_classification(observation.status),
        content_version: optional_content_version(
            "refreshPoint.contentVersion",
            observation.resulting_content_version,
        )?,
    })
}

fn checkpoint_point(
    observation: &RefreshObservation,
) -> Result<CatalogRefreshCheckpointPointV1, ProjectionError> {
    Ok(CatalogRefreshCheckpointPointV1 {
        observation_id: observation.observation_id,
        observed_at: refresh_point_time(observation)?,
        classification: refresh_classification(observation.status),
        content_version: optional_content_version(
            "refreshCheckpointPoint.contentVersion",
            observation.resulting_content_version,
        )?,
    })
}

fn refresh_point_time(observation: &RefreshObservation) -> Result<OffsetDateTime, ProjectionError> {
    parse_timestamp(
        "refreshPoint.observedAt",
        observation
            .completed_at
            .as_deref()
            .unwrap_or(&observation.started_at),
    )
}

fn string_knowledge(
    value: &Presence<String>,
) -> Result<CatalogFieldKnowledge<String>, ProjectionError> {
    presence_knowledge(value, Clone::clone)
}

fn semantic_string_knowledge(
    value: &Presence<String>,
) -> Result<CatalogFieldKnowledge<String>, ProjectionError> {
    presence_knowledge(value, |value| value.trim().to_owned())
}

fn bool_knowledge(value: &Presence<bool>) -> Result<CatalogFieldKnowledge<bool>, ProjectionError> {
    presence_knowledge(value, |value| *value)
}

fn normalized_enum_knowledge<T: Copy>(
    source: &Presence<String>,
    value: T,
) -> Result<CatalogFieldKnowledge<T>, ProjectionError> {
    match source {
        Presence::Missing => Ok(CatalogFieldKnowledge::absent()),
        Presence::Null => Ok(CatalogFieldKnowledge::explicit_null()),
        Presence::Malformed(malformed) => {
            validate_malformed_digest(malformed.canonical_sha256.as_str())?;
            Ok(CatalogFieldKnowledge::unknown(
                CatalogUnknownReason::Malformed,
            ))
        }
        Presence::Value(_) => Ok(CatalogFieldKnowledge::present(value)),
    }
}

fn presence_knowledge<T, U>(
    value: &Presence<T>,
    project: impl FnOnce(&T) -> U,
) -> Result<CatalogFieldKnowledge<U>, ProjectionError> {
    match value {
        Presence::Missing => Ok(CatalogFieldKnowledge::absent()),
        Presence::Null => Ok(CatalogFieldKnowledge::explicit_null()),
        Presence::Malformed(malformed) => {
            validate_malformed_digest(malformed.canonical_sha256.as_str())?;
            Ok(CatalogFieldKnowledge::unknown(
                CatalogUnknownReason::Malformed,
            ))
        }
        Presence::Value(value) => Ok(CatalogFieldKnowledge::present(project(value))),
    }
}

fn object_string_field_knowledge(
    object: &Presence<BTreeMap<String, Value>>,
    field: &str,
) -> Result<CatalogFieldKnowledge<String>, ProjectionError> {
    match object {
        Presence::Missing => Ok(CatalogFieldKnowledge::absent()),
        Presence::Null => Ok(CatalogFieldKnowledge::explicit_null()),
        Presence::Malformed(malformed) => {
            validate_malformed_digest(malformed.canonical_sha256.as_str())?;
            Ok(CatalogFieldKnowledge::unknown(
                CatalogUnknownReason::Malformed,
            ))
        }
        Presence::Value(object) => match object.get(field) {
            None => Ok(CatalogFieldKnowledge::absent()),
            Some(Value::Null) => Ok(CatalogFieldKnowledge::explicit_null()),
            Some(Value::String(value)) => Ok(CatalogFieldKnowledge::present(value.clone())),
            Some(_) => Ok(CatalogFieldKnowledge::unknown(
                CatalogUnknownReason::Malformed,
            )),
        },
    }
}

fn credits_knowledge(
    raw: &RawCatalogCourse,
) -> Result<CatalogFieldKnowledge<String>, ProjectionError> {
    match &raw.credits_object {
        Presence::Value(object) if object.contains_key("code") => {
            object_string_field_knowledge(&raw.credits_object, "code")
        }
        Presence::Value(_) | Presence::Missing => number_string_knowledge(&raw.credits),
        Presence::Null => Ok(CatalogFieldKnowledge::explicit_null()),
        Presence::Malformed(malformed) => {
            validate_malformed_digest(malformed.canonical_sha256.as_str())?;
            Ok(CatalogFieldKnowledge::unknown(
                CatalogUnknownReason::Malformed,
            ))
        }
    }
}

fn number_string_knowledge(
    value: &Presence<Number>,
) -> Result<CatalogFieldKnowledge<String>, ProjectionError> {
    presence_knowledge(value, ToString::to_string)
}

fn value_array_knowledge(
    value: &Presence<Vec<Value>>,
    item: fn(&Value) -> Option<String>,
) -> Result<CatalogFieldKnowledge<Vec<String>>, ProjectionError> {
    match value {
        Presence::Missing => Ok(CatalogFieldKnowledge::absent()),
        Presence::Null => Ok(CatalogFieldKnowledge::explicit_null()),
        Presence::Malformed(malformed) => {
            validate_malformed_digest(malformed.canonical_sha256.as_str())?;
            Ok(CatalogFieldKnowledge::unknown(
                CatalogUnknownReason::Malformed,
            ))
        }
        Presence::Value(values) => {
            let projected = values.iter().map(item).collect::<Option<Vec<_>>>();
            Ok(match projected {
                Some(mut values) => {
                    values.sort();
                    values.dedup();
                    CatalogFieldKnowledge::present(values)
                }
                None => CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed),
            })
        }
    }
}

fn comments_knowledge(
    value: &Presence<Vec<Value>>,
) -> Result<CatalogFieldKnowledge<Vec<CatalogCommentV1>>, ProjectionError> {
    match value {
        Presence::Missing => Ok(CatalogFieldKnowledge::absent()),
        Presence::Null => Ok(CatalogFieldKnowledge::explicit_null()),
        Presence::Malformed(malformed) => {
            validate_malformed_digest(malformed.canonical_sha256.as_str())?;
            Ok(CatalogFieldKnowledge::unknown(
                CatalogUnknownReason::Malformed,
            ))
        }
        Presence::Value(values) => {
            let mut comments = Vec::with_capacity(values.len());
            for value in values {
                let Some(comment) = project_comment(value) else {
                    return Ok(CatalogFieldKnowledge::unknown(
                        CatalogUnknownReason::Malformed,
                    ));
                };
                comments.push(comment);
            }
            comments.sort_by_cached_key(|comment| {
                serde_json::to_string(comment)
                    .expect("typed Catalog comments always serialize to JSON")
            });
            comments.dedup();
            Ok(CatalogFieldKnowledge::present(comments))
        }
    }
}

fn comments_text_knowledge(
    structured: &Presence<Vec<Value>>,
    derived: &Presence<String>,
    comments: &CatalogFieldKnowledge<Vec<CatalogCommentV1>>,
) -> Result<CatalogFieldKnowledge<String>, ProjectionError> {
    let Presence::Value(values) = structured else {
        return semantic_string_knowledge(derived);
    };
    let structured_is_authoritative = match derived {
        Presence::Missing => values.iter().all(valid_comment_value),
        Presence::Value(derived_text) if values.is_empty() => derived_text.trim().is_empty(),
        Presence::Value(_) => values.iter().all(valid_comment_value),
        Presence::Null | Presence::Malformed(_) => false,
    };
    if !structured_is_authoritative {
        return semantic_string_knowledge(derived);
    }
    let CatalogFieldKnowledge::Known {
        presence: CatalogFieldPresence::Present { value: comments },
    } = comments
    else {
        return Err(invalid(
            "catalog.section.comments",
            "trusted structured comments did not produce a typed projection",
        ));
    };
    let text = comments
        .iter()
        .filter_map(|comment| {
            known_nonempty_text(&comment.description).or_else(|| known_nonempty_text(&comment.code))
        })
        .collect::<Vec<_>>()
        .join("; ");
    Ok(CatalogFieldKnowledge::present(text))
}

fn cross_listed_sections_text_knowledge(
    structured: &Presence<Vec<Value>>,
    derived: &Presence<String>,
) -> Result<CatalogFieldKnowledge<String>, ProjectionError> {
    let Presence::Value(values) = structured else {
        return semantic_string_knowledge(derived);
    };
    let structured_is_authoritative = match derived {
        Presence::Missing => values.iter().all(valid_cross_listing_value),
        Presence::Value(derived_text) if values.is_empty() => derived_text.trim().is_empty(),
        Presence::Value(_) => values.iter().all(valid_cross_listing_value),
        Presence::Null | Presence::Malformed(_) => false,
    };
    if !structured_is_authoritative {
        return semantic_string_knowledge(derived);
    }
    let mut codes = values
        .iter()
        .filter_map(cross_listing_code_value)
        .collect::<Vec<_>>();
    codes.sort();
    codes.dedup();
    Ok(CatalogFieldKnowledge::present(codes.join(", ")))
}

fn valid_cross_listing_value(value: &Value) -> bool {
    value.as_str().is_some_and(|value| !value.trim().is_empty())
        || value
            .as_object()
            .and_then(|object| object.get("code"))
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
}

fn cross_listing_code_value(value: &Value) -> Option<String> {
    value
        .as_str()
        .or_else(|| {
            value
                .as_object()
                .and_then(|object| object.get("code"))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn valid_comment_value(value: &Value) -> bool {
    if value.as_str().is_some_and(|value| !value.trim().is_empty()) {
        return true;
    }
    value.as_object().is_some_and(|object| {
        ["code", "description"].iter().any(|field| {
            object
                .get(*field)
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty())
        })
    })
}

fn project_comment(value: &Value) -> Option<CatalogCommentV1> {
    if let Some(description) = value.as_str() {
        let description = description.trim();
        return (!description.is_empty()).then(|| CatalogCommentV1 {
            code: CatalogFieldKnowledge::absent(),
            description: CatalogFieldKnowledge::present(description.to_owned()),
        });
    }
    let object = value.as_object()?;
    let code = json_string_field_knowledge(object, "code");
    let description = json_string_field_knowledge(object, "description");
    (knowledge_has_nonempty_text(&code) || knowledge_has_nonempty_text(&description))
        .then_some(CatalogCommentV1 { code, description })
}

fn json_string_field_knowledge(
    object: &Map<String, Value>,
    field: &str,
) -> CatalogFieldKnowledge<String> {
    match object.get(field) {
        None => CatalogFieldKnowledge::absent(),
        Some(Value::Null) => CatalogFieldKnowledge::explicit_null(),
        Some(Value::String(value)) => CatalogFieldKnowledge::present(value.trim().to_owned()),
        Some(_) => CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed),
    }
}

fn knowledge_has_nonempty_text(value: &CatalogFieldKnowledge<String>) -> bool {
    matches!(
        value,
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Present { value }
        } if !value.is_empty()
    )
}

fn known_nonempty_text(value: &CatalogFieldKnowledge<String>) -> Option<&str> {
    match value {
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Present { value },
        } if !value.is_empty() => Some(value.as_str()),
        _ => None,
    }
}

fn instructors_knowledge(
    value: &Presence<Vec<bcsp_rutgers_client::RawInstructor>>,
) -> Result<CatalogFieldKnowledge<Vec<String>>, ProjectionError> {
    match value {
        Presence::Missing => Ok(CatalogFieldKnowledge::absent()),
        Presence::Null => Ok(CatalogFieldKnowledge::explicit_null()),
        Presence::Malformed(malformed) => {
            validate_malformed_digest(malformed.canonical_sha256.as_str())?;
            Ok(CatalogFieldKnowledge::unknown(
                CatalogUnknownReason::Malformed,
            ))
        }
        Presence::Value(values) => {
            let mut names = Vec::with_capacity(values.len());
            for instructor in values {
                match &instructor.name {
                    Presence::Value(name) if !name.trim().is_empty() => {
                        names.push(name.trim().to_owned());
                    }
                    Presence::Value(_) | Presence::Missing | Presence::Null => {
                        return Ok(CatalogFieldKnowledge::unknown(
                            CatalogUnknownReason::SparseEvidence,
                        ));
                    }
                    Presence::Malformed(malformed) => {
                        validate_malformed_digest(malformed.canonical_sha256.as_str())?;
                        return Ok(CatalogFieldKnowledge::unknown(
                            CatalogUnknownReason::Malformed,
                        ));
                    }
                }
            }
            names.sort();
            Ok(CatalogFieldKnowledge::present(names))
        }
    }
}

fn majors_knowledge(
    value: &Presence<Vec<Value>>,
) -> Result<MajorAndUnitKnowledge, ProjectionError> {
    let malformed = || {
        (
            CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed),
            CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed),
        )
    };
    match value {
        Presence::Missing => Ok((
            CatalogFieldKnowledge::absent(),
            CatalogFieldKnowledge::absent(),
        )),
        Presence::Null => Ok((
            CatalogFieldKnowledge::explicit_null(),
            CatalogFieldKnowledge::explicit_null(),
        )),
        Presence::Malformed(value) => {
            validate_malformed_digest(value.canonical_sha256.as_str())?;
            Ok(malformed())
        }
        Presence::Value(values) => {
            let mut major_codes = Vec::new();
            let mut unit_codes = Vec::new();
            for value in values {
                let Some(object) = value.as_object() else {
                    return Ok(malformed());
                };
                let Some(code) = object
                    .get("code")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|code| !code.is_empty())
                else {
                    return Ok(malformed());
                };
                match (
                    object.get("isMajorCode").and_then(Value::as_bool),
                    object.get("isUnitCode").and_then(Value::as_bool),
                ) {
                    (Some(true), Some(false)) => major_codes.push(code.to_owned()),
                    (Some(false), Some(true)) => unit_codes.push(code.to_owned()),
                    _ => return Ok(malformed()),
                }
            }
            major_codes.sort();
            major_codes.dedup();
            unit_codes.sort();
            unit_codes.dedup();
            Ok((
                CatalogFieldKnowledge::present(major_codes),
                CatalogFieldKnowledge::present(unit_codes),
            ))
        }
    }
}

fn unit_majors_knowledge(
    value: &Presence<Vec<Value>>,
) -> Result<CatalogFieldKnowledge<Vec<CatalogUnitMajorV1>>, ProjectionError> {
    match value {
        Presence::Missing => Ok(CatalogFieldKnowledge::absent()),
        Presence::Null => Ok(CatalogFieldKnowledge::explicit_null()),
        Presence::Malformed(value) => {
            validate_malformed_digest(value.canonical_sha256.as_str())?;
            Ok(CatalogFieldKnowledge::unknown(
                CatalogUnknownReason::Malformed,
            ))
        }
        Presence::Value(values) => {
            let mut projected = Vec::with_capacity(values.len());
            for value in values {
                let Some(object) = value.as_object() else {
                    return Ok(CatalogFieldKnowledge::unknown(
                        CatalogUnknownReason::Malformed,
                    ));
                };
                let Some(unit_code) = object
                    .get("unitCode")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|code| !code.is_empty())
                else {
                    return Ok(CatalogFieldKnowledge::unknown(
                        CatalogUnknownReason::Malformed,
                    ));
                };
                let Some(major_code) = object
                    .get("majorCode")
                    .and_then(Value::as_str)
                    .map(str::trim)
                else {
                    return Ok(CatalogFieldKnowledge::unknown(
                        CatalogUnknownReason::Malformed,
                    ));
                };
                projected.push(CatalogUnitMajorV1 {
                    unit_code: unit_code.to_owned(),
                    major_code: major_code.to_owned(),
                });
            }
            projected.sort();
            projected.dedup();
            Ok(CatalogFieldKnowledge::present(projected))
        }
    }
}

fn days_knowledge(days: &DayKnowledge) -> CatalogFieldKnowledge<Vec<String>> {
    match days {
        DayKnowledge::Missing => CatalogFieldKnowledge::absent(),
        DayKnowledge::Null => CatalogFieldKnowledge::explicit_null(),
        DayKnowledge::Empty => CatalogFieldKnowledge::present(Vec::new()),
        DayKnowledge::Invalid => CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed),
        DayKnowledge::Known(days) => CatalogFieldKnowledge::present(
            days.as_slice()
                .iter()
                .map(|day| weekday_code(*day).to_owned())
                .collect(),
        ),
    }
}

fn discovery_fact<T: DeserializeOwned>(
    facts: &Value,
    field: &str,
    context: &'static str,
) -> Result<CatalogFieldKnowledge<T>, ProjectionError> {
    let envelope = object(
        context,
        object(context, facts)?
            .get(field)
            .ok_or_else(|| invalid(context, "required five-state fact is missing"))?,
    )?;
    let state = envelope
        .get("state")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid(context, "five-state fact has no state"))?;
    match state {
        "MISSING" => {
            require_keys(context, envelope, &["state"])?;
            Ok(CatalogFieldKnowledge::absent())
        }
        "EXPLICIT_NULL" => {
            require_keys(context, envelope, &["state"])?;
            Ok(CatalogFieldKnowledge::explicit_null())
        }
        "MALFORMED" => {
            require_keys(context, envelope, &["canonicalSha256", "jsonType", "state"])?;
            match envelope.get("jsonType").and_then(Value::as_str) {
                Some("BOOLEAN" | "NUMBER" | "STRING" | "ARRAY" | "OBJECT") => {}
                _ => return Err(invalid(context, "malformed fact has an invalid JSON type")),
            }
            let digest = envelope
                .get("canonicalSha256")
                .and_then(Value::as_str)
                .ok_or_else(|| invalid(context, "malformed fact has no digest"))?;
            validate_malformed_digest(digest)?;
            Ok(CatalogFieldKnowledge::unknown(
                CatalogUnknownReason::Malformed,
            ))
        }
        "VALUE" => {
            require_keys(context, envelope, &["state", "value"])?;
            let value = envelope
                .get("value")
                .cloned()
                .ok_or_else(|| invalid(context, "value fact has no value"))?;
            let value = serde_json::from_value(value).map_err(|error| {
                invalid(context, format!("fact value has the wrong type: {error}"))
            })?;
            Ok(CatalogFieldKnowledge::present(value))
        }
        _ => Err(invalid(context, "unknown five-state fact state")),
    }
}

fn discovery_fact_is_true(
    facts: &Value,
    field: &str,
    context: &'static str,
) -> Result<bool, ProjectionError> {
    Ok(matches!(
        discovery_fact::<bool>(facts, field, context)?,
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Present { value: true }
        }
    ))
}

fn require_discovery_schema(
    facts: &Value,
    schema: &'static str,
    context: &'static str,
) -> Result<(), ProjectionError> {
    if object(context, facts)?
        .get("schema")
        .and_then(Value::as_str)
        == Some(schema)
    {
        Ok(())
    } else {
        Err(invalid(context, format!("expected schema {schema}")))
    }
}

fn require_keys(
    context: &'static str,
    object: &Map<String, Value>,
    expected: &[&str],
) -> Result<(), ProjectionError> {
    let actual = object.keys().map(String::as_str).collect::<BTreeSet<_>>();
    let expected = expected.iter().copied().collect::<BTreeSet<_>>();
    if actual == expected {
        Ok(())
    } else {
        Err(invalid(context, "five-state envelope shape is invalid"))
    }
}

fn decode_facts<T: DeserializeOwned>(
    context: &'static str,
    facts: &Value,
) -> Result<T, ProjectionError> {
    if !facts.is_object() {
        return Err(invalid(context, "canonical facts are not an object"));
    }
    validate_internal_malformed_envelopes(facts)?;
    serde_json::from_value(facts.clone())
        .map_err(|error| invalid(context, format!("cannot decode canonical facts: {error}")))
}

fn validate_internal_malformed_envelopes(value: &Value) -> Result<(), ProjectionError> {
    match value {
        Value::Array(values) => {
            for value in values {
                validate_internal_malformed_envelopes(value)?;
            }
        }
        Value::Object(values) => {
            if let Some(envelope) = values.get("$malformed") {
                if values.len() != 1 {
                    return Err(invalid(
                        "canonicalFacts.$malformed",
                        "reserved malformed envelope has sibling fields",
                    ));
                }
                let envelope = object("canonicalFacts.$malformed", envelope)?;
                require_keys(
                    "canonicalFacts.$malformed",
                    envelope,
                    &["canonicalSha256", "jsonType"],
                )?;
                match envelope.get("jsonType").and_then(Value::as_str) {
                    Some("BOOLEAN" | "NUMBER" | "STRING" | "ARRAY" | "OBJECT") => {}
                    _ => {
                        return Err(invalid(
                            "canonicalFacts.$malformed.jsonType",
                            "unknown malformed JSON type",
                        ));
                    }
                }
                validate_malformed_digest(
                    envelope
                        .get("canonicalSha256")
                        .and_then(Value::as_str)
                        .ok_or_else(|| {
                            invalid(
                                "canonicalFacts.$malformed.canonicalSha256",
                                "malformed digest is not a string",
                            )
                        })?,
                )?;
            } else {
                for value in values.values() {
                    validate_internal_malformed_envelopes(value)?;
                }
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

fn object<'a>(
    context: &'static str,
    value: &'a Value,
) -> Result<&'a Map<String, Value>, ProjectionError> {
    value
        .as_object()
        .ok_or_else(|| invalid(context, "expected a JSON object"))
}

fn array_string(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

fn campus_location_code(value: &Value) -> Option<String> {
    value
        .as_object()
        .and_then(|object| object.get("code"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn structured_code(value: &Value) -> Option<String> {
    value
        .as_str()
        .or_else(|| {
            value
                .as_object()
                .and_then(|object| object.get("code"))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

fn validate_marker(
    context: &'static str,
    marker: Option<&TraceId>,
    observation: Option<&TraceId>,
) -> Result<(), ProjectionError> {
    if marker == observation {
        Ok(())
    } else {
        Err(invalid(
            context,
            "marker ID and loaded observation disagree",
        ))
    }
}

fn validate_malformed_digest(value: &str) -> Result<(), ProjectionError> {
    digest("canonicalFacts.$malformed.canonicalSha256", value).map(|_| ())
}

fn parse_timestamp(context: &'static str, value: &str) -> Result<OffsetDateTime, ProjectionError> {
    OffsetDateTime::parse(value, &Rfc3339)
        .map_err(|error| invalid(context, format!("invalid RFC 3339 timestamp: {error}")))
}

fn digest(context: &'static str, value: &str) -> Result<CatalogPayloadDigest, ProjectionError> {
    CatalogPayloadDigest::try_from(value)
        .map_err(|_| invalid(context, "invalid lowercase SHA-256 digest"))
}

fn required_digest(
    context: &'static str,
    value: Option<&str>,
) -> Result<CatalogPayloadDigest, ProjectionError> {
    digest(
        context,
        value.ok_or_else(|| invalid(context, "required payload digest is missing"))?,
    )
}

fn optional_digest(
    context: &'static str,
    value: Option<&str>,
) -> Result<Option<CatalogPayloadDigest>, ProjectionError> {
    value.map(|value| digest(context, value)).transpose()
}

fn diagnostic_code(
    context: &'static str,
    value: &str,
) -> Result<CatalogDiagnosticCode, ProjectionError> {
    CatalogDiagnosticCode::try_from(value)
        .map_err(|_| invalid(context, "diagnostic code violates the public contract"))
}

fn content_version(
    context: &'static str,
    value: u64,
) -> Result<CatalogContentVersion, ProjectionError> {
    CatalogContentVersion::try_from(value)
        .map_err(|_| invalid(context, "published content version must be nonzero"))
}

fn optional_content_version(
    context: &'static str,
    value: Option<u64>,
) -> Result<Option<CatalogContentVersion>, ProjectionError> {
    value
        .map(|value| content_version(context, value))
        .transpose()
}

const fn discovery_source_kind(value: DiscoverySourceKind) -> CatalogDiscoverySourceKind {
    match value {
        DiscoverySourceKind::Selector => CatalogDiscoverySourceKind::Selector,
        DiscoverySourceKind::Bootstrap => CatalogDiscoverySourceKind::Bootstrap,
    }
}

fn discovery_failure_class(stage: &str) -> Result<CatalogDiscoveryErrorClass, ProjectionError> {
    match stage {
        "TRANSPORT" => Ok(CatalogDiscoveryErrorClass::Transport),
        "SCHEMA" => Ok(CatalogDiscoveryErrorClass::Decode),
        "NORMALIZATION" => Ok(CatalogDiscoveryErrorClass::Validate),
        "PUBLISH" => Ok(CatalogDiscoveryErrorClass::Persist),
        _ => Err(invalid(
            "discovery.status.errorStage",
            "unknown storage failure stage",
        )),
    }
}

fn refresh_failure_class(stage: &str) -> Result<CatalogRefreshErrorClass, ProjectionError> {
    match stage {
        "TRANSPORT" => Ok(CatalogRefreshErrorClass::Transport),
        "SCHEMA" => Ok(CatalogRefreshErrorClass::Decode),
        "NORMALIZATION" => Ok(CatalogRefreshErrorClass::Normalize),
        "PUBLISH" => Ok(CatalogRefreshErrorClass::Persist),
        _ => Err(invalid(
            "refresh.errorStage",
            "unknown storage failure stage",
        )),
    }
}

const fn refresh_classification(status: RefreshStatus) -> CatalogRefreshClassification {
    match status {
        RefreshStatus::Started => CatalogRefreshClassification::Started,
        RefreshStatus::Staged => CatalogRefreshClassification::Staged,
        RefreshStatus::AppliedChanged => CatalogRefreshClassification::AppliedChanged,
        RefreshStatus::AppliedUnchanged => CatalogRefreshClassification::AppliedUnchanged,
        RefreshStatus::EmptyValidInitial => CatalogRefreshClassification::EmptyValidInitial,
        RefreshStatus::EmptySuspectRetained => CatalogRefreshClassification::EmptySuspect,
        RefreshStatus::Failed => CatalogRefreshClassification::Error,
        RefreshStatus::Interrupted => CatalogRefreshClassification::Interrupted,
    }
}

const fn refresh_status_name(status: RefreshStatus) -> &'static str {
    match status {
        RefreshStatus::Started => "STARTED",
        RefreshStatus::Staged => "STAGED",
        RefreshStatus::AppliedChanged => "APPLIED_CHANGED",
        RefreshStatus::AppliedUnchanged => "APPLIED_UNCHANGED",
        RefreshStatus::EmptyValidInitial => "EMPTY_VALID_INITIAL",
        RefreshStatus::EmptySuspectRetained => "EMPTY_SUSPECT_RETAINED",
        RefreshStatus::Failed => "FAILED",
        RefreshStatus::Interrupted => "INTERRUPTED",
    }
}

const fn is_refresh_success(status: RefreshStatus) -> bool {
    matches!(
        status,
        RefreshStatus::AppliedChanged
            | RefreshStatus::AppliedUnchanged
            | RefreshStatus::EmptyValidInitial
            | RefreshStatus::EmptySuspectRetained
    )
}

const fn entity_counts(counts: CatalogCounts) -> CatalogEntityCountsV1 {
    CatalogEntityCountsV1 {
        course_groups: counts.course_groups,
        course_variants: counts.course_variants,
        sections: counts.sections,
        occurrences: counts.occurrences,
    }
}

fn weekday_text(days: &DayKnowledge) -> Option<String> {
    let DayKnowledge::Known(days) = days else {
        return None;
    };
    Some(
        days.as_slice()
            .iter()
            .map(|day| weekday_code(*day))
            .collect::<Vec<_>>()
            .join(","),
    )
}

const fn weekday_code(day: Weekday) -> &'static str {
    match day {
        Weekday::Monday => "M",
        Weekday::Tuesday => "T",
        Weekday::Wednesday => "W",
        Weekday::Thursday => "H",
        Weekday::Friday => "F",
        Weekday::Saturday => "S",
        Weekday::Sunday => "U",
    }
}

const fn time_knowledge_name(value: crate::TimeKnowledge) -> &'static str {
    match value {
        crate::TimeKnowledge::Missing => "MISSING",
        crate::TimeKnowledge::Null => "EXPLICIT_NULL",
        crate::TimeKnowledge::Empty => "EMPTY",
        crate::TimeKnowledge::Partial => "PARTIAL",
        crate::TimeKnowledge::Invalid => "INVALID",
        crate::TimeKnowledge::Known { .. } => "KNOWN",
    }
}

fn invalid(context: &'static str, reason: impl Into<String>) -> ProjectionError {
    ProjectionError::InvalidStoredProjection {
        context,
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use bcsp_contracts::{CatalogFieldKnowledge, CatalogUnknownReason};
    use bcsp_rutgers_client::{Presence, RawInstructor};
    use serde_json::json;

    use super::instructors_knowledge;

    #[test]
    fn nested_instructor_presence_is_sparse_or_malformed_collection_evidence() {
        for value in [json!([{}]), json!([{"name":null}]), json!([{"name":""}])] {
            let value: Presence<Vec<RawInstructor>> = serde_json::from_value(value).unwrap();
            assert_eq!(
                instructors_knowledge(&value).unwrap(),
                CatalogFieldKnowledge::unknown(CatalogUnknownReason::SparseEvidence)
            );
        }

        let malformed: Presence<Vec<RawInstructor>> =
            serde_json::from_value(json!([{"name":7}])).unwrap();
        assert_eq!(
            instructors_knowledge(&malformed).unwrap(),
            CatalogFieldKnowledge::unknown(CatalogUnknownReason::Malformed)
        );

        let empty: Presence<Vec<RawInstructor>> = serde_json::from_value(json!([])).unwrap();
        assert_eq!(
            instructors_knowledge(&empty).unwrap(),
            CatalogFieldKnowledge::present(Vec::new())
        );
    }
}
