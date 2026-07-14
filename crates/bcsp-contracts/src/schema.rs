use serde::{Deserialize, Serialize};

use crate::identity::{
    CAMPUS_MAX_BYTES, COURSE_STRING_MAX_BYTES, FINGERPRINT_PREFIX, SECTION_INDEX_WIDTH,
    SHA256_HEX_BYTES, TERM_MAX_BYTES,
};
use crate::{
    API_PROTOCOL_VERSION, ApiErrorCode, CatalogDiscoveryAvailability, CatalogDiscoveryErrorClass,
    CatalogDiscoverySourceKind, CatalogInstructorReliability, CatalogModality,
    CatalogOccurrenceEvidence, CatalogOccurrenceKind, CatalogOpenStatusProvenance,
    CatalogPrerequisiteState, CatalogRefreshClassification, CatalogRefreshErrorClass,
    CatalogRequiredness, CatalogSourceKind, CatalogSynchronicity, CatalogUnknownReason,
    MatchOutcome, MatchReasonCode, WS_PROTOCOL_VERSION,
};

pub const CONTRACT_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SchemaDirection {
    SharedIdentity,
    ClientToServer,
    ServerToClient,
    Bidirectional,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UnknownFieldPolicy {
    Reject,
    Ignore,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScalarConstraint {
    pub id: String,
    pub wire_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exact_bytes: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContractField {
    pub name: String,
    pub type_ref: String,
    pub required: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContractVariant {
    pub tag_value: String,
    pub fields: Vec<ContractField>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContractSchema {
    pub id: String,
    pub direction: SchemaDirection,
    pub unknown_fields: UnknownFieldPolicy,
    pub fields: Vec<ContractField>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discriminator: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<ContractVariant>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContractManifest {
    pub schema_version: u16,
    pub api_protocol_version: u16,
    pub ws_protocol_version: u16,
    pub scalar_constraints: Vec<ScalarConstraint>,
    pub schemas: Vec<ContractSchema>,
}

fn field(name: &str, type_ref: &str) -> ContractField {
    ContractField {
        name: name.to_owned(),
        type_ref: type_ref.to_owned(),
        required: true,
    }
}

fn schema(
    id: &str,
    direction: SchemaDirection,
    unknown_fields: UnknownFieldPolicy,
    fields: &[(&str, &str)],
) -> ContractSchema {
    ContractSchema {
        id: id.to_owned(),
        direction,
        unknown_fields,
        fields: fields
            .iter()
            .map(|(name, type_ref)| field(name, type_ref))
            .collect(),
        enum_values: Vec::new(),
        discriminator: None,
        variants: Vec::new(),
    }
}

fn enum_schema(id: &str, values: impl IntoIterator<Item = String>) -> ContractSchema {
    ContractSchema {
        id: id.to_owned(),
        direction: SchemaDirection::Bidirectional,
        unknown_fields: UnknownFieldPolicy::Reject,
        fields: Vec::new(),
        enum_values: values.into_iter().collect(),
        discriminator: None,
        variants: Vec::new(),
    }
}

fn tagged_union_schema(
    id: &str,
    direction: SchemaDirection,
    unknown_fields: UnknownFieldPolicy,
    discriminator: &str,
    variants: &[(&str, &[(&str, &str)])],
) -> ContractSchema {
    ContractSchema {
        id: id.to_owned(),
        direction,
        unknown_fields,
        fields: Vec::new(),
        enum_values: Vec::new(),
        discriminator: Some(discriminator.to_owned()),
        variants: variants
            .iter()
            .map(|(tag_value, fields)| ContractVariant {
                tag_value: (*tag_value).to_owned(),
                fields: fields
                    .iter()
                    .map(|(name, type_ref)| field(name, type_ref))
                    .collect(),
            })
            .collect(),
    }
}

fn string_constraint(
    id: &str,
    exact_bytes: Option<usize>,
    max_bytes: Option<usize>,
    pattern: Option<&str>,
    semantic: Option<&str>,
) -> ScalarConstraint {
    ScalarConstraint {
        id: id.to_owned(),
        wire_type: "string".to_owned(),
        exact_bytes: exact_bytes.map(|value| value as u16),
        max_bytes: max_bytes.map(|value| value as u16),
        pattern: pattern.map(str::to_owned),
        semantic: semantic.map(str::to_owned),
    }
}

pub fn contract_manifest() -> ContractManifest {
    let error_codes = ApiErrorCode::ALL
        .iter()
        .map(|code| code.wire_name().to_owned());
    let match_outcomes = MatchOutcome::ALL
        .iter()
        .map(|outcome| outcome.wire_name().to_owned());
    let match_reasons = MatchReasonCode::ALL
        .iter()
        .map(|reason| reason.wire_name().to_owned());

    ContractManifest {
        schema_version: CONTRACT_SCHEMA_VERSION,
        api_protocol_version: API_PROTOCOL_VERSION.as_u16(),
        ws_protocol_version: WS_PROTOCOL_VERSION.as_u16(),
        scalar_constraints: vec![
            string_constraint(
                "term-id",
                None,
                Some(TERM_MAX_BYTES),
                Some("^[A-Z0-9_-]+$"),
                Some("dynamic selector identity; no trim or case rewrite"),
            ),
            string_constraint(
                "campus-code",
                None,
                Some(CAMPUS_MAX_BYTES),
                Some("^[A-Z0-9_]+$"),
                Some("dynamic selector identity; no fixed campus allowlist"),
            ),
            string_constraint(
                "section-index",
                Some(SECTION_INDEX_WIDTH),
                None,
                Some("^[0-9]{5}$"),
                Some("leading zeroes are identity-significant"),
            ),
            string_constraint(
                "course-string",
                None,
                Some(COURSE_STRING_MAX_BYTES),
                Some("^[A-Za-z0-9:._-]+$"),
                Some("opaque source identifier; no parsing or case rewrite"),
            ),
            string_constraint(
                "variant-fingerprint",
                Some(FINGERPRINT_PREFIX.len() + SHA256_HEX_BYTES),
                None,
                Some("^v1:[0-9a-f]{64}$"),
                Some("algorithm-versioned canonical course-field fingerprint"),
            ),
            string_constraint(
                "reason-field",
                None,
                Some(64),
                Some("^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$"),
                Some("stable filter or domain field identifier"),
            ),
            string_constraint(
                "detail-name",
                None,
                Some(64),
                Some("^[a-z0-9][a-z0-9._-]{0,63}$"),
                Some("safe public error-detail field or limit identifier"),
            ),
            string_constraint(
                "message-key",
                None,
                Some(64),
                Some("^error\\.[a-z0-9._-]+$"),
                Some("stable translation key derived from the typed error code"),
            ),
            string_constraint(
                "trace-id",
                Some(36),
                None,
                Some("^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$"),
                Some("canonical lowercase RFC 4122 random UUID v4; injected source"),
            ),
            ScalarConstraint {
                id: "protocol-version".to_owned(),
                wire_type: "u16".to_owned(),
                exact_bytes: None,
                max_bytes: None,
                pattern: None,
                semantic: Some("only integer 1 is accepted".to_owned()),
            },
            ScalarConstraint {
                id: "catalog-contract-version".to_owned(),
                wire_type: "u16".to_owned(),
                exact_bytes: None,
                max_bytes: None,
                pattern: None,
                semantic: Some("only integer 1 is accepted".to_owned()),
            },
            ScalarConstraint {
                id: "catalog-content-version".to_owned(),
                wire_type: "u64".to_owned(),
                exact_bytes: None,
                max_bytes: None,
                pattern: None,
                semantic: Some(
                    "monotonic target-scoped published content version; zero is invalid".to_owned(),
                ),
            },
            string_constraint(
                "catalog-payload-digest",
                Some(SHA256_HEX_BYTES),
                None,
                Some("^[0-9a-f]{64}$"),
                Some("lowercase SHA-256 of the observed upstream payload; raw body excluded"),
            ),
            string_constraint(
                "catalog-subject-code",
                None,
                Some(64),
                None,
                Some(
                    "opaque dynamic subject identity; nonempty, trim-stable, control-free UTF-8; no compiled allowlist or rewrite",
                ),
            ),
            string_constraint(
                "catalog-discovery-source-id",
                None,
                Some(64),
                Some("^[A-Z0-9_.:-]+$"),
                Some("stable public discovery source identity; not an upstream URL"),
            ),
            string_constraint(
                "catalog-diagnostic-code",
                None,
                Some(64),
                Some("^[A-Z0-9_]+$"),
                Some("stable redacted diagnostic code; never parser or upstream detail"),
            ),
        ],
        schemas: vec![
            schema(
                "bcsp.identity.term-campus-key.v1",
                SchemaDirection::SharedIdentity,
                UnknownFieldPolicy::Reject,
                &[
                    ("term", "$scalar:term-id"),
                    ("campus", "$scalar:campus-code"),
                ],
            ),
            schema(
                "bcsp.identity.section-key.v1",
                SchemaDirection::SharedIdentity,
                UnknownFieldPolicy::Reject,
                &[
                    ("term", "$scalar:term-id"),
                    ("campus", "$scalar:campus-code"),
                    ("index", "$scalar:section-index"),
                ],
            ),
            schema(
                "bcsp.identity.course-group-key.v1",
                SchemaDirection::SharedIdentity,
                UnknownFieldPolicy::Reject,
                &[
                    ("term", "$scalar:term-id"),
                    ("campus", "$scalar:campus-code"),
                    ("courseString", "$scalar:course-string"),
                ],
            ),
            schema(
                "bcsp.identity.course-variant-key.v1",
                SchemaDirection::SharedIdentity,
                UnknownFieldPolicy::Reject,
                &[
                    ("group", "$schema:bcsp.identity.course-group-key.v1"),
                    ("fingerprint", "$scalar:variant-fingerprint"),
                ],
            ),
            schema(
                "bcsp.catalog.discovery-request.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[("contractVersion", "$scalar:catalog-contract-version")],
            ),
            enum_schema(
                "bcsp.catalog.unknown-reason.v1",
                CatalogUnknownReason::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            tagged_union_schema(
                "bcsp.catalog.field-presence.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                "presence",
                &[
                    ("PRESENT", &[("value", "$generic:T")]),
                    ("EXPLICIT_NULL", &[]),
                    ("ABSENT", &[]),
                ],
            ),
            tagged_union_schema(
                "bcsp.catalog.field-knowledge.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                "knowledge",
                &[
                    ("KNOWN", &[("presence", "$generic:CatalogFieldPresence<T>")]),
                    (
                        "UNKNOWN",
                        &[("reason", "$schema:bcsp.catalog.unknown-reason.v1")],
                    ),
                ],
            ),
            schema(
                "bcsp.catalog.target.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("key", "$schema:bcsp.identity.term-campus-key.v1"),
                    ("termLabel", "$generic:CatalogFieldKnowledge<string>"),
                    ("campusLabel", "$generic:CatalogFieldKnowledge<string>"),
                    ("provenance", "$schema:bcsp.catalog.discovery-provenance.v1"),
                ],
            ),
            schema(
                "bcsp.catalog.discovery-response.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:catalog-contract-version"),
                    ("observedAt", "$primitive:rfc3339-timestamp"),
                    ("status", "$schema:bcsp.catalog.discovery-status.v1"),
                    ("sources", "$array:$schema:bcsp.catalog.discovery-source.v1"),
                    ("targets", "$array:$schema:bcsp.catalog.target.v1"),
                    ("subjects", "$array:$schema:bcsp.catalog.subject.v1"),
                ],
            ),
            enum_schema(
                "bcsp.catalog.discovery-source-kind.v1",
                CatalogDiscoverySourceKind::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            schema(
                "bcsp.catalog.discovery-source.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("sourceId", "$scalar:catalog-discovery-source-id"),
                    (
                        "sourceKind",
                        "$schema:bcsp.catalog.discovery-source-kind.v1",
                    ),
                    ("sourceVersion", "$generic:CatalogFieldKnowledge<string>"),
                    ("payloadDigest", "$scalar:catalog-payload-digest"),
                    ("observedAt", "$primitive:rfc3339-timestamp"),
                ],
            ),
            enum_schema(
                "bcsp.catalog.discovery-availability.v1",
                CatalogDiscoveryAvailability::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            enum_schema(
                "bcsp.catalog.discovery-error-class.v1",
                CatalogDiscoveryErrorClass::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            schema(
                "bcsp.catalog.discovery-error.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("class", "$schema:bcsp.catalog.discovery-error-class.v1"),
                    ("code", "$scalar:catalog-diagnostic-code"),
                ],
            ),
            schema(
                "bcsp.catalog.discovery-point.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("observationId", "$scalar:trace-id"),
                    ("observedAt", "$primitive:rfc3339-timestamp"),
                    (
                        "contentVersion",
                        "$optional:$scalar:catalog-content-version",
                    ),
                ],
            ),
            schema(
                "bcsp.catalog.discovery-status.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    (
                        "availability",
                        "$schema:bcsp.catalog.discovery-availability.v1",
                    ),
                    (
                        "latestAttempt",
                        "$optional:$schema:bcsp.catalog.discovery-point.v1",
                    ),
                    (
                        "lastSuccess",
                        "$optional:$schema:bcsp.catalog.discovery-point.v1",
                    ),
                    ("isStale", "$primitive:bool"),
                    ("error", "$optional:$schema:bcsp.catalog.discovery-error.v1"),
                ],
            ),
            schema(
                "bcsp.catalog.discovery-provenance.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("observationId", "$scalar:trace-id"),
                    ("sourceId", "$scalar:catalog-discovery-source-id"),
                    (
                        "sourceKind",
                        "$schema:bcsp.catalog.discovery-source-kind.v1",
                    ),
                    ("observedAt", "$primitive:rfc3339-timestamp"),
                    ("payloadDigest", "$scalar:catalog-payload-digest"),
                ],
            ),
            enum_schema(
                "bcsp.catalog.source-kind.v1",
                CatalogSourceKind::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            schema(
                "bcsp.catalog.provenance.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("observationId", "$scalar:trace-id"),
                    ("source", "$schema:bcsp.catalog.source-kind.v1"),
                    ("target", "$schema:bcsp.identity.term-campus-key.v1"),
                    ("observedAt", "$primitive:rfc3339-timestamp"),
                    ("payloadDigest", "$scalar:catalog-payload-digest"),
                ],
            ),
            schema(
                "bcsp.catalog.subject.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("target", "$schema:bcsp.identity.term-campus-key.v1"),
                    ("code", "$scalar:catalog-subject-code"),
                    ("label", "$generic:CatalogFieldKnowledge<string>"),
                    ("provenance", "$schema:bcsp.catalog.discovery-provenance.v1"),
                ],
            ),
            schema(
                "bcsp.catalog.occurrence-key.v1",
                SchemaDirection::SharedIdentity,
                UnknownFieldPolicy::Reject,
                &[
                    ("section", "$schema:bcsp.identity.section-key.v1"),
                    ("ordinal", "$primitive:u32"),
                ],
            ),
            schema(
                "bcsp.catalog.normalized-course-group.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("key", "$schema:bcsp.identity.course-group-key.v1"),
                    (
                        "variantKeys",
                        "$array:$schema:bcsp.identity.course-variant-key.v1",
                    ),
                ],
            ),
            enum_schema(
                "bcsp.catalog.prerequisite-state.v1",
                CatalogPrerequisiteState::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            schema(
                "bcsp.catalog.normalized-course-variant.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("key", "$schema:bcsp.identity.course-variant-key.v1"),
                    ("title", "$generic:CatalogFieldKnowledge<string>"),
                    ("expandedTitle", "$generic:CatalogFieldKnowledge<string>"),
                    ("description", "$generic:CatalogFieldKnowledge<string>"),
                    ("notes", "$generic:CatalogFieldKnowledge<string>"),
                    (
                        "subjectGroupNotes",
                        "$generic:CatalogFieldKnowledge<string>",
                    ),
                    ("subjectNotes", "$generic:CatalogFieldKnowledge<string>"),
                    ("unitNotes", "$generic:CatalogFieldKnowledge<string>"),
                    ("synopsisUrl", "$generic:CatalogFieldKnowledge<string>"),
                    (
                        "prerequisiteNotes",
                        "$generic:CatalogFieldKnowledge<string>",
                    ),
                    (
                        "prerequisiteState",
                        "$schema:bcsp.catalog.prerequisite-state.v1",
                    ),
                    ("credits", "$generic:CatalogFieldKnowledge<string>"),
                    ("level", "$generic:CatalogFieldKnowledge<string>"),
                    ("subjectCode", "$generic:CatalogFieldKnowledge<string>"),
                    (
                        "subjectDescription",
                        "$generic:CatalogFieldKnowledge<string>",
                    ),
                    ("courseNumber", "$generic:CatalogFieldKnowledge<string>"),
                    ("supplementCode", "$generic:CatalogFieldKnowledge<string>"),
                    ("schoolCode", "$generic:CatalogFieldKnowledge<string>"),
                    ("offeringUnit", "$generic:CatalogFieldKnowledge<string>"),
                    (
                        "offeringUnitTitle",
                        "$generic:CatalogFieldKnowledge<string>",
                    ),
                    ("coreCodes", "$generic:CatalogFieldKnowledge<array<string>>"),
                    (
                        "campusLocations",
                        "$generic:CatalogFieldKnowledge<array<string>>",
                    ),
                    ("sectionKeys", "$array:$schema:bcsp.identity.section-key.v1"),
                ],
            ),
            enum_schema(
                "bcsp.catalog.open-status-provenance.v1",
                CatalogOpenStatusProvenance::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            schema(
                "bcsp.catalog.snapshot-open-status.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("value", "$generic:CatalogFieldKnowledge<bool>"),
                    (
                        "provenance",
                        "$schema:bcsp.catalog.open-status-provenance.v1",
                    ),
                ],
            ),
            enum_schema(
                "bcsp.catalog.instructor-reliability.v1",
                CatalogInstructorReliability::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            schema(
                "bcsp.catalog.unit-major.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("unitCode", "$primitive:string"),
                    ("majorCode", "$primitive:string"),
                ],
            ),
            schema(
                "bcsp.catalog.comment.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("code", "$generic:CatalogFieldKnowledge<string>"),
                    ("description", "$generic:CatalogFieldKnowledge<string>"),
                ],
            ),
            schema(
                "bcsp.catalog.normalized-section.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("key", "$schema:bcsp.identity.section-key.v1"),
                    ("variantKey", "$schema:bcsp.identity.course-variant-key.v1"),
                    ("sectionNumber", "$generic:CatalogFieldKnowledge<string>"),
                    ("subtitle", "$generic:CatalogFieldKnowledge<string>"),
                    ("subtopic", "$generic:CatalogFieldKnowledge<string>"),
                    ("sectionNotes", "$generic:CatalogFieldKnowledge<string>"),
                    ("sessionDates", "$generic:CatalogFieldKnowledge<string>"),
                    (
                        "sessionDatePrintIndicator",
                        "$generic:CatalogFieldKnowledge<string>",
                    ),
                    (
                        "comments",
                        "$generic:CatalogFieldKnowledge<array<CatalogCommentV1>>",
                    ),
                    ("commentsText", "$generic:CatalogFieldKnowledge<string>"),
                    (
                        "crossListedSectionsText",
                        "$generic:CatalogFieldKnowledge<string>",
                    ),
                    (
                        "crossListedSectionType",
                        "$generic:CatalogFieldKnowledge<string>",
                    ),
                    (
                        "instructors",
                        "$generic:CatalogFieldKnowledge<array<string>>",
                    ),
                    (
                        "instructorReliability",
                        "$schema:bcsp.catalog.instructor-reliability.v1",
                    ),
                    (
                        "rawSectionCourseType",
                        "$generic:CatalogFieldKnowledge<string>",
                    ),
                    ("deliveryModality", "$schema:bcsp.catalog.modality.v1"),
                    ("synchronicity", "$schema:bcsp.catalog.synchronicity.v1"),
                    ("examCode", "$generic:CatalogFieldKnowledge<string>"),
                    ("examCodeText", "$generic:CatalogFieldKnowledge<string>"),
                    (
                        "specialPermissionAddCode",
                        "$generic:CatalogFieldKnowledge<string>",
                    ),
                    (
                        "specialPermissionAddDescription",
                        "$generic:CatalogFieldKnowledge<string>",
                    ),
                    (
                        "specialPermissionDropCode",
                        "$generic:CatalogFieldKnowledge<string>",
                    ),
                    (
                        "specialPermissionDropDescription",
                        "$generic:CatalogFieldKnowledge<string>",
                    ),
                    (
                        "majorCodes",
                        "$generic:CatalogFieldKnowledge<array<string>>",
                    ),
                    ("unitCodes", "$generic:CatalogFieldKnowledge<array<string>>"),
                    (
                        "minorCodes",
                        "$generic:CatalogFieldKnowledge<array<string>>",
                    ),
                    (
                        "honorProgramCodes",
                        "$generic:CatalogFieldKnowledge<array<string>>",
                    ),
                    (
                        "unitMajors",
                        "$generic:CatalogFieldKnowledge<array<CatalogUnitMajorV1>>",
                    ),
                    ("eligibilityText", "$generic:CatalogFieldKnowledge<string>"),
                    ("openToText", "$generic:CatalogFieldKnowledge<string>"),
                    (
                        "catalogOpenStatus",
                        "$schema:bcsp.catalog.snapshot-open-status.v1",
                    ),
                    (
                        "occurrenceKeys",
                        "$generic:CatalogFieldKnowledge<array<CatalogOccurrenceKeyV1>>",
                    ),
                ],
            ),
            enum_schema(
                "bcsp.catalog.modality.v1",
                CatalogModality::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            enum_schema(
                "bcsp.catalog.synchronicity.v1",
                CatalogSynchronicity::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            enum_schema(
                "bcsp.catalog.occurrence-kind.v1",
                CatalogOccurrenceKind::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            enum_schema(
                "bcsp.catalog.occurrence-evidence.v1",
                CatalogOccurrenceEvidence::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            enum_schema(
                "bcsp.catalog.requiredness.v1",
                CatalogRequiredness::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            tagged_union_schema(
                "bcsp.catalog.time-knowledge.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                "knowledge",
                &[
                    ("MISSING", &[]),
                    ("EXPLICIT_NULL", &[]),
                    ("EMPTY", &[]),
                    ("PARTIAL", &[]),
                    ("INVALID", &[]),
                    (
                        "KNOWN",
                        &[
                            ("startMinute", "$primitive:u16"),
                            ("endMinute", "$primitive:u16"),
                        ],
                    ),
                ],
            ),
            schema(
                "bcsp.catalog.normalized-occurrence.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("key", "$schema:bcsp.catalog.occurrence-key.v1"),
                    ("rawCode", "$generic:CatalogFieldKnowledge<string>"),
                    ("rawDescription", "$generic:CatalogFieldKnowledge<string>"),
                    (
                        "modality",
                        "$generic:CatalogFieldKnowledge<CatalogModality>",
                    ),
                    (
                        "synchronicity",
                        "$generic:CatalogFieldKnowledge<CatalogSynchronicity>",
                    ),
                    ("rawDay", "$generic:CatalogFieldKnowledge<string>"),
                    ("days", "$generic:CatalogFieldKnowledge<array<string>>"),
                    ("rawStartTime", "$generic:CatalogFieldKnowledge<string>"),
                    ("rawEndTime", "$generic:CatalogFieldKnowledge<string>"),
                    ("time", "$schema:bcsp.catalog.time-knowledge.v1"),
                    ("startDate", "$generic:CatalogFieldKnowledge<string>"),
                    ("endDate", "$generic:CatalogFieldKnowledge<string>"),
                    ("campus", "$generic:CatalogFieldKnowledge<string>"),
                    ("campusName", "$generic:CatalogFieldKnowledge<string>"),
                    ("building", "$generic:CatalogFieldKnowledge<string>"),
                    ("room", "$generic:CatalogFieldKnowledge<string>"),
                    ("requiredness", "$schema:bcsp.catalog.requiredness.v1"),
                    ("kind", "$schema:bcsp.catalog.occurrence-kind.v1"),
                    ("evidence", "$schema:bcsp.catalog.occurrence-evidence.v1"),
                    ("normalizationReason", "$scalar:catalog-diagnostic-code"),
                ],
            ),
            schema(
                "bcsp.catalog.normalized-catalog.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:catalog-contract-version"),
                    ("target", "$schema:bcsp.identity.term-campus-key.v1"),
                    ("contentVersion", "$scalar:catalog-content-version"),
                    ("provenance", "$schema:bcsp.catalog.provenance.v1"),
                    (
                        "courseGroups",
                        "$array:$schema:bcsp.catalog.normalized-course-group.v1",
                    ),
                    (
                        "courseVariants",
                        "$array:$schema:bcsp.catalog.normalized-course-variant.v1",
                    ),
                    (
                        "sections",
                        "$array:$schema:bcsp.catalog.normalized-section.v1",
                    ),
                    (
                        "occurrences",
                        "$array:$schema:bcsp.catalog.normalized-occurrence.v1",
                    ),
                ],
            ),
            enum_schema(
                "bcsp.catalog.refresh-classification.v1",
                CatalogRefreshClassification::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            enum_schema(
                "bcsp.catalog.refresh-error-class.v1",
                CatalogRefreshErrorClass::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            schema(
                "bcsp.catalog.entity-counts.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("courseGroups", "$primitive:u64"),
                    ("courseVariants", "$primitive:u64"),
                    ("sections", "$primitive:u64"),
                    ("occurrences", "$primitive:u64"),
                ],
            ),
            schema(
                "bcsp.catalog.refresh-observation.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:catalog-contract-version"),
                    ("observationId", "$scalar:trace-id"),
                    ("target", "$schema:bcsp.identity.term-campus-key.v1"),
                    ("startedAt", "$primitive:rfc3339-timestamp"),
                    ("finishedAt", "$primitive:rfc3339-timestamp"),
                    (
                        "classification",
                        "$schema:bcsp.catalog.refresh-classification.v1",
                    ),
                    (
                        "contentVersion",
                        "$optional:$scalar:catalog-content-version",
                    ),
                    ("payloadDigest", "$optional:$scalar:catalog-payload-digest"),
                    ("counts", "$schema:bcsp.catalog.entity-counts.v1"),
                    (
                        "errorClass",
                        "$optional:$schema:bcsp.catalog.refresh-error-class.v1",
                    ),
                    (
                        "partialReasons",
                        "$array:$schema:bcsp.catalog.unknown-reason.v1",
                    ),
                ],
            ),
            schema(
                "bcsp.catalog.refresh-point.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("observationId", "$scalar:trace-id"),
                    ("observedAt", "$primitive:rfc3339-timestamp"),
                    (
                        "classification",
                        "$schema:bcsp.catalog.refresh-classification.v1",
                    ),
                    (
                        "contentVersion",
                        "$optional:$scalar:catalog-content-version",
                    ),
                ],
            ),
            schema(
                "bcsp.catalog.refresh-checkpoint-point.v1",
                SchemaDirection::Bidirectional,
                UnknownFieldPolicy::Reject,
                &[
                    ("observationId", "$scalar:trace-id"),
                    ("observedAt", "$primitive:rfc3339-timestamp"),
                    (
                        "classification",
                        "$schema:bcsp.catalog.refresh-classification.v1",
                    ),
                    (
                        "contentVersion",
                        "$optional:$scalar:catalog-content-version",
                    ),
                ],
            ),
            schema(
                "bcsp.catalog.refresh-checkpoint.v1",
                SchemaDirection::Bidirectional,
                UnknownFieldPolicy::Reject,
                &[
                    ("contractVersion", "$scalar:catalog-contract-version"),
                    ("target", "$schema:bcsp.identity.term-campus-key.v1"),
                    (
                        "latestAttempt",
                        "$schema:bcsp.catalog.refresh-checkpoint-point.v1",
                    ),
                    (
                        "lastSuccess",
                        "$optional:$schema:bcsp.catalog.refresh-checkpoint-point.v1",
                    ),
                    (
                        "lastPublished",
                        "$optional:$schema:bcsp.catalog.refresh-checkpoint-point.v1",
                    ),
                    (
                        "lastNonempty",
                        "$optional:$schema:bcsp.catalog.refresh-checkpoint-point.v1",
                    ),
                    (
                        "pendingEmpty",
                        "$optional:$schema:bcsp.catalog.refresh-checkpoint-point.v1",
                    ),
                ],
            ),
            schema(
                "bcsp.catalog.refresh-status.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:catalog-contract-version"),
                    ("target", "$schema:bcsp.identity.term-campus-key.v1"),
                    ("latestAttempt", "$schema:bcsp.catalog.refresh-point.v1"),
                    (
                        "lastSuccess",
                        "$optional:$schema:bcsp.catalog.refresh-point.v1",
                    ),
                    (
                        "lastPublished",
                        "$optional:$schema:bcsp.catalog.refresh-point.v1",
                    ),
                    (
                        "lastNonempty",
                        "$optional:$schema:bcsp.catalog.refresh-point.v1",
                    ),
                    (
                        "pendingEmpty",
                        "$optional:$schema:bcsp.catalog.refresh-point.v1",
                    ),
                ],
            ),
            enum_schema("bcsp.match.outcome.v1", match_outcomes),
            enum_schema("bcsp.match.reason-code.v1", match_reasons),
            schema(
                "bcsp.match.reason.v1",
                SchemaDirection::Bidirectional,
                UnknownFieldPolicy::Reject,
                &[
                    ("code", "$schema:bcsp.match.reason-code.v1"),
                    ("field", "$scalar:reason-field"),
                ],
            ),
            schema(
                "bcsp.match.explanation.v1",
                SchemaDirection::Bidirectional,
                UnknownFieldPolicy::Reject,
                &[
                    ("outcome", "$schema:bcsp.match.outcome.v1"),
                    ("reasons", "$array:$schema:bcsp.match.reason.v1"),
                ],
            ),
            enum_schema("bcsp.error.shared-code.v1", error_codes),
            schema(
                "bcsp.http.request-envelope.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("protocolVersion", "$scalar:protocol-version"),
                    ("payload", "$generic:T"),
                ],
            ),
            schema(
                "bcsp.http.success-envelope.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("protocolVersion", "$scalar:protocol-version"),
                    ("data", "$generic:T"),
                ],
            ),
            schema(
                "bcsp.http.error-body.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("code", "$schema:bcsp.error.shared-code.v1"),
                    ("messageKey", "$scalar:message-key"),
                    ("traceId", "$scalar:trace-id"),
                    ("details", "$array:$schema:bcsp.http.error-detail.v1"),
                ],
            ),
            tagged_union_schema(
                "bcsp.http.error-detail.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                "kind",
                &[
                    ("INVALID_FIELD", &[("field", "$scalar:detail-name")]),
                    (
                        "LIMIT",
                        &[
                            ("name", "$scalar:detail-name"),
                            ("maximum", "$primitive:u32"),
                        ],
                    ),
                    ("RETRY_AFTER_SECONDS", &[("seconds", "$primitive:u32")]),
                ],
            ),
            schema(
                "bcsp.http.error-envelope.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("protocolVersion", "$scalar:protocol-version"),
                    ("error", "$schema:bcsp.http.error-body.v1"),
                ],
            ),
            schema(
                "bcsp.ws.client-envelope.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("protocolVersion", "$scalar:protocol-version"),
                    ("messageId", "$scalar:trace-id"),
                    ("payload", "$generic:T"),
                ],
            ),
            schema(
                "bcsp.ws.server-envelope.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("protocolVersion", "$scalar:protocol-version"),
                    ("messageId", "$scalar:trace-id"),
                    ("payload", "$generic:T"),
                ],
            ),
        ],
    }
}
