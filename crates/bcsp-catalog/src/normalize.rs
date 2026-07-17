use std::collections::{BTreeMap, BTreeSet};

use bcsp_contracts::{
    CourseGroupKey, CourseString, CourseVariantKey, SectionKey, TermCampusKey, VariantFingerprint,
};
use bcsp_rutgers_client::{
    Presence, RawCatalogCourse, RawCatalogSection, RawInstructor, SourceProvenance,
};
use serde::Serialize;
use serde_json::{Map, Value, json};
use thiserror::Error;

use crate::canonical::{
    canonical_course_semantic_value_v1, canonical_course_variant_hash_v1,
    canonical_raw_course_hash_v1, canonical_raw_section_hash_v1,
    canonical_section_semantic_hash_v1, canonical_section_semantic_value_v1,
    canonical_target_content_hash_v1,
};
use crate::delivery::{classify_delivery, normalize_occurrence};
use crate::model::{
    CatalogMetrics, CatalogOpenSectionsSnapshot, CollectionPresence, CourseVariantFields,
    DeliveryModality, DuplicateAudit, DuplicateDisposition, InstructorReliability,
    NormalizedCatalogTarget, NormalizedCourseGroup, NormalizedCourseVariant, NormalizedInstructor,
    NormalizedSection, Sha256Hex,
};

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum NormalizationError {
    #[error("required Catalog field is absent or null: {0}")]
    MissingRequiredField(&'static str),
    #[error("required Catalog field is malformed: {0}")]
    MalformedRequiredField(&'static str),
    #[error("Catalog identity is invalid: {0}")]
    InvalidIdentity(String),
    #[error("same variant fingerprint has conflicting course facts: {key:?}")]
    ConflictingVariantFacts { key: CourseVariantKey },
    #[error("same SectionKey has conflicting semantic facts: {key}")]
    ConflictingDuplicate { key: SectionKey },
    #[error("same SectionKey is attached to multiple variants: {key}")]
    SectionAcrossVariants { key: SectionKey },
    #[error("Catalog canonicalization failed: {0}")]
    Canonicalization(String),
}

struct GroupAccumulator {
    key: CourseGroupKey,
    raw_course_count: usize,
    variants: BTreeMap<CourseVariantKey, VariantAccumulator>,
}

struct VariantAccumulator {
    key: CourseVariantKey,
    fields: CourseVariantFields,
    raw_course_count: usize,
    record_hashes: BTreeSet<Sha256Hex>,
    canonical_hash: Sha256Hex,
    canonical_course_facts: Value,
    catalog_open_sections_snapshots: Vec<CatalogOpenSectionsSnapshot>,
    sections: BTreeMap<SectionKey, NormalizedSection>,
}

struct SectionSeen {
    variant_key: CourseVariantKey,
    semantic_hash: Sha256Hex,
    representative_record_hash: Sha256Hex,
    raw_record_count: usize,
    raw_record_hashes: BTreeSet<Sha256Hex>,
}

struct SectionResult {
    section: NormalizedSection,
    raw_meetings: usize,
    raw_instructors: usize,
}

pub fn normalize_target(
    target: TermCampusKey,
    courses: Vec<RawCatalogCourse>,
    provenance: SourceProvenance,
) -> Result<NormalizedCatalogTarget, NormalizationError> {
    let mut metrics = CatalogMetrics {
        raw_courses: courses.len(),
        ..CatalogMetrics::default()
    };
    let mut groups = BTreeMap::<CourseGroupKey, GroupAccumulator>::new();
    let mut seen_sections = BTreeMap::<SectionKey, SectionSeen>::new();

    for (course_ordinal, course) in courses.into_iter().enumerate() {
        validate_optional_campus_shape(&course.campus_code, "course.campusCode")?;
        let course_string: CourseString = required(&course.course_string, "course.courseString")?
            .clone()
            .try_into()
            .map_err(|error: bcsp_contracts::IdentityError| {
                NormalizationError::InvalidIdentity(error.to_string())
            })?;
        let group_key = CourseGroupKey::new(
            target.term().clone(),
            target.campus().clone(),
            course_string,
        );
        let fingerprint = variant_fingerprint_v1(&course)?;
        let fingerprint: VariantFingerprint =
            fingerprint
                .try_into()
                .map_err(|error: bcsp_contracts::IdentityError| {
                    NormalizationError::InvalidIdentity(error.to_string())
                })?;
        let variant_key = CourseVariantKey::new(group_key.clone(), fingerprint);
        let raw_course_value = to_value(&course)?;
        let record_hash =
            canonical_raw_course_hash_v1(&raw_course_value).map_err(canonical_error)?;
        let canonical_course_facts = canonical_course_facts(&raw_course_value);
        let canonical_hash =
            canonical_course_variant_hash_v1(&canonical_course_facts).map_err(canonical_error)?;
        let canonical_course: RawCatalogCourse =
            serde_json::from_value(canonical_course_facts.clone()).map_err(canonical_error)?;
        let fields = course_fields(&canonical_course);

        let group = groups
            .entry(group_key.clone())
            .or_insert_with(|| GroupAccumulator {
                key: group_key,
                raw_course_count: 0,
                variants: BTreeMap::new(),
            });
        group.raw_course_count += 1;
        let new_variant = !group.variants.contains_key(&variant_key);
        let variant = group
            .variants
            .entry(variant_key.clone())
            .or_insert_with(|| VariantAccumulator {
                key: variant_key.clone(),
                fields,
                raw_course_count: 0,
                record_hashes: BTreeSet::new(),
                canonical_hash: canonical_hash.clone(),
                canonical_course_facts: canonical_course_facts.clone(),
                catalog_open_sections_snapshots: Vec::new(),
                sections: BTreeMap::new(),
            });
        if !new_variant {
            if !merge_course_variant_facts(
                &mut variant.canonical_course_facts,
                &canonical_course_facts,
            ) {
                return Err(NormalizationError::ConflictingVariantFacts { key: variant_key });
            }
            variant.canonical_hash =
                canonical_course_variant_hash_v1(&variant.canonical_course_facts)
                    .map_err(canonical_error)?;
            let merged_course: RawCatalogCourse =
                serde_json::from_value(variant.canonical_course_facts.clone())
                    .map_err(canonical_error)?;
            variant.fields = course_fields(&merged_course);
        }
        variant.raw_course_count += 1;
        variant.record_hashes.insert(record_hash.clone());
        variant
            .catalog_open_sections_snapshots
            .push(CatalogOpenSectionsSnapshot {
                source_ordinal: course_ordinal,
                value: course.open_sections.clone(),
                raw_course_hash: record_hash,
            });

        let raw_sections = match &course.sections {
            Presence::Value(raw_sections) => raw_sections,
            Presence::Missing | Presence::Null => {
                return Err(NormalizationError::MissingRequiredField("course.sections"));
            }
            Presence::Malformed(_) => {
                return Err(NormalizationError::MalformedRequiredField(
                    "course.sections",
                ));
            }
        };
        for raw_section in raw_sections {
            metrics.raw_sections += 1;
            let normalized = normalize_section(&target, raw_section)?;
            metrics.raw_meetings += normalized.raw_meetings;
            metrics.raw_instructor_assignments += normalized.raw_instructors;
            if normalized.section.delivery_modality == DeliveryModality::UnknownConflict {
                metrics.delivery_conflict_raw_sections += 1;
            }

            let section_key = normalized.section.key.clone();
            match seen_sections.get_mut(&section_key) {
                Some(seen) if seen.variant_key != variant.key => {
                    return Err(NormalizationError::SectionAcrossVariants { key: section_key });
                }
                Some(seen) if seen.semantic_hash != normalized.section.semantic_hash => {
                    return Err(NormalizationError::ConflictingDuplicate { key: section_key });
                }
                Some(seen) => {
                    let representative_record_hash = normalized.section.first_record_hash.clone();
                    seen.raw_record_count += 1;
                    seen.raw_record_hashes
                        .insert(representative_record_hash.clone());
                    if representative_record_hash < seen.representative_record_hash {
                        seen.representative_record_hash = representative_record_hash;
                        variant.sections.insert(section_key, normalized.section);
                    }
                }
                None => {
                    let mut raw_record_hashes = BTreeSet::new();
                    raw_record_hashes.insert(normalized.section.first_record_hash.clone());
                    seen_sections.insert(
                        section_key.clone(),
                        SectionSeen {
                            variant_key: variant.key.clone(),
                            semantic_hash: normalized.section.semantic_hash.clone(),
                            representative_record_hash: normalized
                                .section
                                .first_record_hash
                                .clone(),
                            raw_record_count: 1,
                            raw_record_hashes,
                        },
                    );
                    variant.sections.insert(section_key, normalized.section);
                }
            }
        }
    }

    let mut normalized_groups = Vec::with_capacity(groups.len());
    for group in groups.into_values() {
        let mut variants = Vec::with_capacity(group.variants.len());
        for variant in group.variants.into_values() {
            variants.push(NormalizedCourseVariant {
                key: variant.key,
                fields: variant.fields,
                raw_course_count: variant.raw_course_count,
                record_hashes: variant.record_hashes.into_iter().collect(),
                canonical_hash: variant.canonical_hash,
                canonical_course_facts: variant.canonical_course_facts,
                catalog_open_sections_snapshots: variant.catalog_open_sections_snapshots,
                sections: variant.sections.into_values().collect(),
            });
        }
        normalized_groups.push(NormalizedCourseGroup {
            key: group.key,
            raw_course_count: group.raw_course_count,
            variants,
        });
    }

    let duplicate_audits = seen_sections
        .iter()
        .filter(|(_, seen)| seen.raw_record_count > 1)
        .map(|(section_key, seen)| DuplicateAudit {
            section_key: section_key.clone(),
            raw_record_count: seen.raw_record_count,
            raw_record_hashes: seen.raw_record_hashes.iter().cloned().collect(),
            semantic_hash: seen.semantic_hash.clone(),
            disposition: DuplicateDisposition::AuditedEquivalentCollapse,
            reason: "ALL_SECTION_SEMANTIC_HASHES_EQUAL",
        })
        .collect::<Vec<_>>();
    metrics.equivalent_duplicate_keys = duplicate_audits.len();
    metrics.normalized_sections = seen_sections.len();
    let content_hash = content_hash(&target, &normalized_groups)?;

    Ok(NormalizedCatalogTarget {
        target,
        groups: normalized_groups,
        duplicate_audits,
        content_hash,
        provenance,
        metrics,
    })
}

pub fn variant_fingerprint_v1(course: &RawCatalogCourse) -> Result<String, NormalizationError> {
    // V1 is the frozen offering signature observed in P3. Other course facts
    // remain canonical content and can advance the content version without
    // needlessly changing the stable variant key.
    let raw_course = to_value(course)?;
    let semantic_course = canonical_course_semantic_value_v1(&raw_course);
    let value = course_variant_identity_facts(&semantic_course);
    let digest = canonical_course_variant_hash_v1(&value).map_err(canonical_error)?;
    Ok(format!("v1:{digest}"))
}

fn normalize_section(
    target: &TermCampusKey,
    raw: &RawCatalogSection,
) -> Result<SectionResult, NormalizationError> {
    validate_optional_campus_shape(&raw.campus_code, "section.campusCode")?;
    let index = required(&raw.index, "section.index")?;
    let key = SectionKey::try_new(target.term().as_str(), target.campus().as_str(), index)
        .map_err(|error| NormalizationError::InvalidIdentity(error.to_string()))?;
    let canonical_facts = to_value(raw)?;
    let semantic_facts = canonical_section_semantic_value_v1(&canonical_facts);
    let semantic_raw: RawCatalogSection =
        serde_json::from_value(semantic_facts).map_err(canonical_error)?;
    let first_record_hash =
        canonical_raw_section_hash_v1(&canonical_facts).map_err(canonical_error)?;
    let semantic_hash =
        canonical_section_semantic_hash_v1(&canonical_facts).map_err(canonical_error)?;
    let occurrence_source = CollectionPresence::from(&raw.meeting_times);
    let mut occurrences = Vec::new();
    if let Presence::Value(raw_occurrences) = &raw.meeting_times {
        for (ordinal, raw_occurrence) in raw_occurrences.iter().enumerate() {
            occurrences
                .push(normalize_occurrence(raw_occurrence, ordinal).map_err(canonical_error)?);
        }
    }
    let instructor_source = CollectionPresence::from(&semantic_raw.instructors);
    let mut instructors = Vec::new();
    if let Presence::Value(raw_instructors) = &semantic_raw.instructors {
        for raw_instructor in raw_instructors {
            instructors.push(normalize_instructor(raw_instructor)?);
        }
    }
    let (delivery_modality, synchronicity) =
        classify_delivery(&semantic_raw.section_course_type, &occurrences);

    Ok(SectionResult {
        raw_meetings: occurrences.len(),
        raw_instructors: instructors.len(),
        section: NormalizedSection {
            key,
            section_number: semantic_raw.number,
            catalog_open_status: semantic_raw.open_status,
            raw_section_course_type: semantic_raw.section_course_type,
            delivery_modality,
            synchronicity,
            occurrence_source,
            occurrences,
            instructor_source,
            instructors,
            semantic_hash,
            first_record_hash,
            canonical_facts,
        },
    })
}

fn normalize_instructor(raw: &RawInstructor) -> Result<NormalizedInstructor, NormalizationError> {
    Ok(NormalizedInstructor {
        name: raw.name.clone(),
        reliability: InstructorReliability::NameOnly,
        canonical_facts: to_value(raw)?,
    })
}

fn course_fields(course: &RawCatalogCourse) -> CourseVariantFields {
    CourseVariantFields {
        supplement_code: course.supplement_code.clone(),
        credits: course.credits.clone(),
        credits_object: course.credits_object.clone(),
        title: course.title.clone(),
        expanded_title: course.expanded_title.clone(),
        level: course.level.clone(),
        offering_unit_code: course.offering_unit_code.clone(),
        subject: course.subject.clone(),
        subject_description: course.subject_description.clone(),
        course_number: course.course_number.clone(),
        course_description: course.course_description.clone(),
        course_notes: course.course_notes.clone(),
        prerequisite_notes: course.pre_req_notes.clone(),
        core_codes: course.core_codes.clone(),
        campus_locations: course.campus_locations.clone(),
    }
}

fn canonical_course_facts(raw: &Value) -> Value {
    let mut raw = raw.clone();
    if let Value::Object(object) = &mut raw {
        object.remove("sections");
        object.remove("openSections");
        // Campus filtering and identity come from the request selector. These
        // scalar scope hints can legitimately differ inside one selector and
        // are not user-facing course-location facts.
        object.remove("campusCode");
        object.remove("mainCampus");
    }
    canonical_course_semantic_value_v1(&raw)
}

fn course_variant_identity_facts(course: &Value) -> Value {
    const FIELDS: [&str; 6] = [
        "supplementCode",
        "creditsObject",
        "title",
        "expandedTitle",
        "level",
        "offeringUnitCode",
    ];

    let mut identity = Map::new();
    if let Value::Object(course) = course {
        for field in FIELDS {
            if let Some(value) = course.get(field) {
                identity.insert(field.to_owned(), value.clone());
            }
        }
    }
    Value::Object(identity)
}

fn merge_course_variant_facts(existing: &mut Value, incoming: &Value) -> bool {
    let (Some(existing_object), Some(incoming_object)) =
        (existing.as_object(), incoming.as_object())
    else {
        return false;
    };
    let mut existing_base = existing_object.clone();
    let mut incoming_base = incoming_object.clone();
    let existing_locations = existing_base.remove("campusLocations");
    let incoming_locations = incoming_base.remove("campusLocations");
    if existing_base != incoming_base {
        return false;
    }

    let merged_locations = match (existing_locations, incoming_locations) {
        (None, None) => None,
        (Some(left), Some(right)) if left == right => Some(left),
        (Some(Value::Array(mut left)), Some(Value::Array(right))) => {
            left.extend(right);
            left.sort_by_key(|value| {
                serde_json::to_string(value).expect("JSON Value serialization cannot fail")
            });
            left.dedup();
            Some(Value::Array(left))
        }
        _ => return false,
    };

    let Some(existing_object) = existing.as_object_mut() else {
        return false;
    };
    if let Some(locations) = merged_locations {
        existing_object.insert("campusLocations".to_owned(), locations);
    } else {
        existing_object.remove("campusLocations");
    }
    true
}

fn content_hash(
    target: &TermCampusKey,
    groups: &[NormalizedCourseGroup],
) -> Result<Sha256Hex, NormalizationError> {
    let groups = groups
        .iter()
        .map(|group| {
            let variants = group
                .variants
                .iter()
                .map(|variant| {
                    let sections = variant
                        .sections
                        .iter()
                        .map(|section| {
                            json!({
                                "term": section.key.term().as_str(),
                                "campus": section.key.campus().as_str(),
                                "index": section.key.index().as_str(),
                                "semanticHash": section.semantic_hash,
                            })
                        })
                        .collect::<Vec<_>>();
                    json!({
                        "fingerprint": variant.key.fingerprint().as_str(),
                        "canonicalHash": variant.canonical_hash,
                        "sections": sections,
                    })
                })
                .collect::<Vec<_>>();
            json!({
                "courseString": group.key.course_string().as_str(),
                "variants": variants,
            })
        })
        .collect::<Vec<_>>();
    canonical_target_content_hash_v1(&json!({
        "schema": "catalog-content-v1",
        "target": {
            "term": target.term().as_str(),
            "campus": target.campus().as_str(),
        },
        "groups": groups,
    }))
    .map_err(canonical_error)
}

fn required<'a, T>(
    value: &'a Presence<T>,
    field: &'static str,
) -> Result<&'a T, NormalizationError> {
    match value {
        Presence::Value(value) => Ok(value),
        Presence::Missing | Presence::Null => Err(NormalizationError::MissingRequiredField(field)),
        Presence::Malformed(_) => Err(NormalizationError::MalformedRequiredField(field)),
    }
}

fn validate_optional_campus_shape(
    value: &Presence<String>,
    field: &'static str,
) -> Result<(), NormalizationError> {
    if matches!(value, Presence::Malformed(_)) {
        return Err(NormalizationError::MalformedRequiredField(field));
    }
    Ok(())
}

fn to_value<T>(value: &T) -> Result<Value, NormalizationError>
where
    T: Serialize,
{
    serde_json::to_value(value).map_err(canonical_error)
}

fn canonical_error(error: serde_json::Error) -> NormalizationError {
    NormalizationError::Canonicalization(error.to_string())
}
