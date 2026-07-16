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
    FilterFieldId, MatchOutcome, MatchReasonCode, OpenAppliedClassification, OpenCircuitReason,
    OpenCircuitState, OpenEpisodeState, OpenFailureClass, OpenFreshnessState,
    OpenRefreshClassification, OpenSchedulerLane, OpenState, OpenUncertaintyReason,
    RutgersDayTimezone, ServiceAvailabilityV1, ServiceIssueComponentV1, ServiceIssueRecoveryV1,
    ServiceIssueSeverityV1, ServiceLevelV1, ServiceOperationPhaseV1, ServiceRuntimeV1,
    WS_PROTOCOL_VERSION, WatchAlertDisposition, WatchContinuousMixerStopReason,
    WatchCueCancellationReason, WatchCueOutcome, WatchNotificationMode, WatchStartRejectionReason,
    WatchStopReason,
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

fn optional_field(name: &str, type_ref: &str) -> ContractField {
    ContractField {
        name: name.to_owned(),
        type_ref: type_ref.to_owned(),
        required: false,
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

fn schema_with_optional_fields(
    id: &str,
    direction: SchemaDirection,
    unknown_fields: UnknownFieldPolicy,
    required_fields: &[(&str, &str)],
    optional_fields: &[(&str, &str)],
) -> ContractSchema {
    ContractSchema {
        id: id.to_owned(),
        direction,
        unknown_fields,
        fields: required_fields
            .iter()
            .map(|(name, type_ref)| field(name, type_ref))
            .chain(
                optional_fields
                    .iter()
                    .map(|(name, type_ref)| optional_field(name, type_ref)),
            )
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

fn serialized_enum_values<T: Serialize>(values: &[T]) -> Vec<String> {
    values
        .iter()
        .map(|value| {
            serde_json::to_value(value)
                .expect("contract enum must serialize")
                .as_str()
                .expect("contract enum must serialize as a string")
                .to_owned()
        })
        .collect()
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
            ScalarConstraint {
                id: "query-contract-version".to_owned(),
                wire_type: "u16".to_owned(),
                exact_bytes: None,
                max_bytes: None,
                pattern: None,
                semantic: Some("only integer 1 is accepted".to_owned()),
            },
            ScalarConstraint {
                id: "open-contract-version".to_owned(),
                wire_type: "u16".to_owned(),
                exact_bytes: None,
                max_bytes: None,
                pattern: None,
                semantic: Some("only integer 1 is accepted".to_owned()),
            },
            ScalarConstraint {
                id: "watch-contract-version".to_owned(),
                wire_type: "u16".to_owned(),
                exact_bytes: None,
                max_bytes: None,
                pattern: None,
                semantic: Some("only integer 1 is accepted".to_owned()),
            },
            ScalarConstraint {
                id: "watch-positive-u64".to_owned(),
                wire_type: "u64".to_owned(),
                exact_bytes: None,
                max_bytes: None,
                pattern: None,
                semantic: Some("positive integer with no product upper limit".to_owned()),
            },
            ScalarConstraint {
                id: "open-sequence".to_owned(),
                wire_type: "u64".to_owned(),
                exact_bytes: None,
                max_bytes: None,
                pattern: None,
                semantic: Some("nonzero target-scoped attempt or observation sequence".to_owned()),
            },
            ScalarConstraint {
                id: "open-http-status-code".to_owned(),
                wire_type: "u16".to_owned(),
                exact_bytes: None,
                max_bytes: None,
                pattern: None,
                semantic: Some(
                    "valid HTTP status code in the inclusive range 100..=599".to_owned(),
                ),
            },
            string_constraint(
                "open-http-header-value",
                None,
                Some(4_096),
                None,
                Some(
                    "nonempty trim-stable unfolded HTTP response-header value containing only visible ASCII or horizontal tabs",
                ),
            ),
            string_constraint(
                "open-decoded-body-sha256",
                Some(SHA256_HEX_BYTES),
                None,
                Some("^[0-9a-f]{64}$"),
                Some("lowercase SHA-256 of the decoded Open response body; raw body excluded"),
            ),
            string_constraint(
                "open-canonical-set-hash",
                Some(SHA256_HEX_BYTES),
                None,
                Some("^[0-9a-f]{64}$"),
                Some("lowercase SHA-256 over the normalized Open index set; raw body excluded"),
            ),
            string_constraint(
                "open-state-hash",
                Some(SHA256_HEX_BYTES),
                None,
                Some("^[0-9a-f]{64}$"),
                Some("lowercase SHA-256 over the batch Catalog intersection state"),
            ),
            string_constraint(
                "rutgers-day",
                Some(10),
                None,
                Some("^[0-9]{4}-[0-9]{2}-[0-9]{2}$"),
                Some("valid calendar date at the America/New_York day boundary"),
            ),
            string_constraint(
                "filter-token",
                None,
                Some(256),
                None,
                Some("normalized nonempty trim-stable control-free UTF-8 filter token"),
            ),
            string_constraint(
                "filter-search-text",
                None,
                Some(512),
                None,
                Some(
                    "1..=32 tokens; each <=128 UTF-8 bytes and containing Unicode alphanumeric; whitespace collapsed; total <=512 bytes",
                ),
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
                    (
                        "coreCodeDictionaries",
                        "$array:$schema:bcsp.catalog.core-code-dictionary.v1",
                    ),
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
            tagged_union_schema(
                "bcsp.catalog.subject-provenance.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                "kind",
                &[
                    (
                        "DISCOVERY",
                        &[("discovery", "$schema:bcsp.catalog.discovery-provenance.v1")],
                    ),
                    (
                        "CATALOG",
                        &[
                            ("contentVersion", "$scalar:catalog-content-version"),
                            ("catalog", "$schema:bcsp.catalog.provenance.v1"),
                        ],
                    ),
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
                    ("provenance", "$schema:bcsp.catalog.subject-provenance.v1"),
                ],
            ),
            schema(
                "bcsp.catalog.core-code-option.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("code", "$primitive:string"),
                    ("description", "$generic:CatalogFieldKnowledge<string>"),
                ],
            ),
            schema(
                "bcsp.catalog.core-code-dictionary.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("target", "$schema:bcsp.identity.term-campus-key.v1"),
                    ("contentVersion", "$scalar:catalog-content-version"),
                    ("provenance", "$schema:bcsp.catalog.provenance.v1"),
                    ("options", "$array:$schema:bcsp.catalog.core-code-option.v1"),
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
            schema(
                "bcsp.open.batch-key.v1",
                SchemaDirection::SharedIdentity,
                UnknownFieldPolicy::Reject,
                &[
                    ("term", "$scalar:term-id"),
                    ("campus", "$scalar:campus-code"),
                ],
            ),
            enum_schema(
                "bcsp.open.refresh-classification.v1",
                OpenRefreshClassification::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            enum_schema(
                "bcsp.open.applied-classification.v1",
                OpenAppliedClassification::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            enum_schema(
                "bcsp.open.failure-class.v1",
                OpenFailureClass::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            enum_schema(
                "bcsp.open.state.v1",
                OpenState::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            enum_schema(
                "bcsp.open.freshness-state.v1",
                OpenFreshnessState::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            enum_schema(
                "bcsp.open.uncertainty-reason.v1",
                OpenUncertaintyReason::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            enum_schema(
                "bcsp.open.scheduler-lane.v1",
                OpenSchedulerLane::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            enum_schema(
                "bcsp.open.circuit-state.v1",
                OpenCircuitState::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            enum_schema(
                "bcsp.open.circuit-reason.v1",
                OpenCircuitReason::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            enum_schema(
                "bcsp.open.rutgers-day-timezone.v1",
                RutgersDayTimezone::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            schema(
                "bcsp.open.pull-counts.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("attempted", "$primitive:u64"),
                    ("succeeded", "$primitive:u64"),
                    ("failed", "$primitive:u64"),
                    ("empty", "$primitive:u64"),
                ],
            ),
            schema_with_optional_fields(
                "bcsp.open.counter-snapshot.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("todayCounts", "$schema:bcsp.open.pull-counts.v1"),
                    ("rutgersDay", "$scalar:rutgers-day"),
                    ("dayTimezone", "$schema:bcsp.open.rutgers-day-timezone.v1"),
                ],
                &[("runCounts", "$schema:bcsp.open.pull-counts.v1")],
            ),
            schema(
                "bcsp.open.freshness.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("state", "$schema:bcsp.open.freshness-state.v1"),
                    ("observedAt", "$optional:$primitive:rfc3339-timestamp"),
                    ("freshUntil", "$optional:$primitive:rfc3339-timestamp"),
                    ("lastKnownGoodAgeSeconds", "$optional:$primitive:u64"),
                    (
                        "uncertainty",
                        "$optional:$schema:bcsp.open.uncertainty-reason.v1",
                    ),
                ],
            ),
            schema(
                "bcsp.open.http-metadata.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("statusCode", "$optional:$scalar:open-http-status-code"),
                    ("decodedBytes", "$optional:$primitive:u64"),
                    (
                        "decodedBodySha256",
                        "$optional:$scalar:open-decoded-body-sha256",
                    ),
                    ("contentType", "$optional:$scalar:open-http-header-value"),
                    ("etag", "$optional:$scalar:open-http-header-value"),
                    ("cacheControl", "$optional:$scalar:open-http-header-value"),
                    ("date", "$optional:$scalar:open-http-header-value"),
                    ("ageSeconds", "$optional:$primitive:u64"),
                    ("lastModified", "$optional:$scalar:open-http-header-value"),
                    ("retryAfter", "$optional:$scalar:open-http-header-value"),
                ],
            ),
            schema(
                "bcsp.open.reconcile-counts.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("canonicalOpenIndexes", "$primitive:u64"),
                    ("duplicateIndexes", "$primitive:u64"),
                    ("catalogSections", "$primitive:u64"),
                    ("matchedOpenSections", "$primitive:u64"),
                    ("closedSections", "$primitive:u64"),
                    ("orphanIndexes", "$primitive:u64"),
                ],
            ),
            schema(
                "bcsp.open.attempt-schedule.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("lane", "$schema:bcsp.open.scheduler-lane.v1"),
                    ("requestedGeneralIntervalSeconds", "$primitive:u32"),
                    ("requestedEffectiveIntervalSeconds", "$primitive:u32"),
                    ("dueAt", "$primitive:rfc3339-timestamp"),
                    ("schedulerLagMilliseconds", "$primitive:u64"),
                    (
                        "actualStartToStartIntervalMilliseconds",
                        "$optional:$primitive:u64",
                    ),
                ],
            ),
            schema(
                "bcsp.open.pull-attempt.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:open-contract-version"),
                    ("attemptId", "$scalar:trace-id"),
                    ("batch", "$schema:bcsp.open.batch-key.v1"),
                    ("attemptSequence", "$scalar:open-sequence"),
                    ("catalogContentVersion", "$scalar:catalog-content-version"),
                    ("startedAt", "$primitive:rfc3339-timestamp"),
                    ("completedAt", "$optional:$primitive:rfc3339-timestamp"),
                    (
                        "classification",
                        "$schema:bcsp.open.refresh-classification.v1",
                    ),
                    (
                        "failureClass",
                        "$optional:$schema:bcsp.open.failure-class.v1",
                    ),
                    ("http", "$schema:bcsp.open.http-metadata.v1"),
                    (
                        "canonicalSetHash",
                        "$optional:$scalar:open-canonical-set-hash",
                    ),
                    ("stateHash", "$optional:$scalar:open-state-hash"),
                    ("counts", "$schema:bcsp.open.reconcile-counts.v1"),
                    ("schedule", "$schema:bcsp.open.attempt-schedule.v1"),
                    ("lastKnownGoodAgeSeconds", "$optional:$primitive:u64"),
                ],
            ),
            schema(
                "bcsp.open.refresh-observation.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:open-contract-version"),
                    ("observationId", "$scalar:trace-id"),
                    ("batch", "$schema:bcsp.open.batch-key.v1"),
                    ("attemptSequence", "$scalar:open-sequence"),
                    ("observationSequence", "$scalar:open-sequence"),
                    ("catalogContentVersion", "$scalar:catalog-content-version"),
                    ("startedAt", "$primitive:rfc3339-timestamp"),
                    ("completedAt", "$primitive:rfc3339-timestamp"),
                    ("observedAt", "$primitive:rfc3339-timestamp"),
                    (
                        "classification",
                        "$schema:bcsp.open.applied-classification.v1",
                    ),
                    ("http", "$schema:bcsp.open.http-metadata.v1"),
                    ("canonicalSetHash", "$scalar:open-canonical-set-hash"),
                    ("stateHash", "$scalar:open-state-hash"),
                    ("counts", "$schema:bcsp.open.reconcile-counts.v1"),
                    ("schedule", "$schema:bcsp.open.attempt-schedule.v1"),
                    ("bodyChanged", "$primitive:bool"),
                    ("stateChanged", "$primitive:bool"),
                    ("lastKnownGoodAgeSeconds", "$primitive:u64"),
                ],
            ),
            schema(
                "bcsp.open.observation.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:open-contract-version"),
                    ("observationId", "$scalar:trace-id"),
                    ("refreshObservationId", "$scalar:trace-id"),
                    ("batch", "$schema:bcsp.open.batch-key.v1"),
                    ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                    ("pullSequence", "$scalar:open-sequence"),
                    ("catalogContentVersion", "$scalar:catalog-content-version"),
                    ("state", "$schema:bcsp.open.state.v1"),
                    ("observedAt", "$primitive:rfc3339-timestamp"),
                    ("freshUntil", "$primitive:rfc3339-timestamp"),
                    ("schedulerLagMilliseconds", "$primitive:u64"),
                    ("counterSnapshot", "$schema:bcsp.open.counter-snapshot.v1"),
                ],
            ),
            schema(
                "bcsp.open.attempt-point.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("attemptSequence", "$scalar:open-sequence"),
                    ("startedAt", "$primitive:rfc3339-timestamp"),
                    ("completedAt", "$optional:$primitive:rfc3339-timestamp"),
                    (
                        "classification",
                        "$schema:bcsp.open.refresh-classification.v1",
                    ),
                    (
                        "failureClass",
                        "$optional:$schema:bcsp.open.failure-class.v1",
                    ),
                ],
            ),
            schema(
                "bcsp.open.failure-point.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("attemptSequence", "$scalar:open-sequence"),
                    ("failedAt", "$primitive:rfc3339-timestamp"),
                    ("failureClass", "$schema:bcsp.open.failure-class.v1"),
                ],
            ),
            schema(
                "bcsp.open.observation-point.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("observationId", "$scalar:trace-id"),
                    ("observationSequence", "$scalar:open-sequence"),
                    ("catalogContentVersion", "$scalar:catalog-content-version"),
                    ("observedAt", "$primitive:rfc3339-timestamp"),
                    ("canonicalSetHash", "$scalar:open-canonical-set-hash"),
                    ("stateHash", "$scalar:open-state-hash"),
                ],
            ),
            schema(
                "bcsp.open.circuit-status.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("state", "$schema:bcsp.open.circuit-state.v1"),
                    ("reason", "$optional:$schema:bcsp.open.circuit-reason.v1"),
                    ("openedAt", "$optional:$primitive:rfc3339-timestamp"),
                    ("retryAt", "$optional:$primitive:rfc3339-timestamp"),
                    ("diagnosticRecheckRequired", "$primitive:bool"),
                ],
            ),
            schema(
                "bcsp.open.scheduler-status.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("lane", "$schema:bcsp.open.scheduler-lane.v1"),
                    ("requestedGeneralIntervalSeconds", "$primitive:u32"),
                    ("requestedEffectiveIntervalSeconds", "$primitive:u32"),
                    ("activeWatchCount", "$primitive:u64"),
                    ("nextDueAt", "$optional:$primitive:rfc3339-timestamp"),
                    ("inFlight", "$primitive:bool"),
                    ("schedulerLagMilliseconds", "$primitive:u64"),
                    (
                        "actualStartToStartIntervalMilliseconds",
                        "$optional:$primitive:u64",
                    ),
                    ("failureStreak", "$primitive:u32"),
                ],
            ),
            schema(
                "bcsp.open.refresh-status.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:open-contract-version"),
                    ("batch", "$schema:bcsp.open.batch-key.v1"),
                    (
                        "catalogContentVersion",
                        "$optional:$scalar:catalog-content-version",
                    ),
                    (
                        "latestAttempt",
                        "$optional:$schema:bcsp.open.attempt-point.v1",
                    ),
                    (
                        "latestFailure",
                        "$optional:$schema:bcsp.open.failure-point.v1",
                    ),
                    (
                        "lastValidObservation",
                        "$optional:$schema:bcsp.open.observation-point.v1",
                    ),
                    ("lastBodyChangeAt", "$optional:$primitive:rfc3339-timestamp"),
                    (
                        "lastStateChangeAt",
                        "$optional:$primitive:rfc3339-timestamp",
                    ),
                    ("freshness", "$schema:bcsp.open.freshness.v1"),
                    ("scheduler", "$schema:bcsp.open.scheduler-status.v1"),
                    ("circuit", "$schema:bcsp.open.circuit-status.v1"),
                    ("counterSnapshot", "$schema:bcsp.open.counter-snapshot.v1"),
                ],
            ),
            schema(
                "bcsp.open.section-status.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:open-contract-version"),
                    ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                    ("state", "$schema:bcsp.open.state.v1"),
                    ("lastObservationId", "$optional:$scalar:trace-id"),
                    (
                        "catalogContentVersion",
                        "$optional:$scalar:catalog-content-version",
                    ),
                    ("freshness", "$schema:bcsp.open.freshness.v1"),
                    ("schedulerLagMilliseconds", "$primitive:u64"),
                    ("counterSnapshot", "$schema:bcsp.open.counter-snapshot.v1"),
                ],
            ),
            schema(
                "bcsp.open.status-request.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("contractVersion", "$scalar:open-contract-version"),
                    ("batch", "$schema:bcsp.open.batch-key.v1"),
                ],
            ),
            schema(
                "bcsp.open.section-status-request.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("contractVersion", "$scalar:open-contract-version"),
                    ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                ],
            ),
            enum_schema(
                "bcsp.query.filter-field-id.v1",
                FilterFieldId::ALL
                    .iter()
                    .map(|value| value.wire_name().to_owned()),
            ),
            enum_schema(
                "bcsp.query.filter-scope.v1",
                ["COURSE", "SECTION"].map(str::to_owned),
            ),
            enum_schema(
                "bcsp.query.filter-value-kind.v1",
                [
                    "TERM_ID",
                    "CAMPUS_CODE_SET",
                    "SUBJECT_CODE_SET",
                    "TEXT_QUERY",
                    "COURSE_NUMBER_SET",
                    "LEVEL_SET",
                    "CREDIT_RANGE",
                    "CORE_CODE_SET",
                    "PREREQUISITE_PRESENCE",
                    "COURSE_LOCATION_SET",
                    "SECTION_INDEX_SET",
                    "SECTION_NUMBER_SET",
                    "OPEN_STATUS_SET",
                    "MODALITY_SET",
                    "SYNCHRONICITY_SET",
                    "INSTRUCTOR_NAME_SET",
                    "AVAILABILITY_WINDOWS",
                    "MEETING_LOCATION_SET",
                    "BUILDING_ROOM",
                    "EXAM_CODE_SET",
                    "PERMISSION_REQUIREMENT",
                    "ELIGIBILITY",
                ]
                .map(str::to_owned),
            ),
            enum_schema(
                "bcsp.query.filter-schema-value.v1",
                [
                    "REQUIRED",
                    "EMPTY_SET",
                    "EMPTY_TEXT",
                    "UNBOUNDED_RANGE",
                    "ANY",
                    "EMPTY_WINDOWS",
                    "EMPTY_COMPOSITE",
                ]
                .map(str::to_owned),
            ),
            enum_schema(
                "bcsp.query.filter-normalization.v1",
                [
                    "CANONICAL_IDENTITY",
                    "TRIM",
                    "TRIM_AND_COLLAPSE_WHITESPACE",
                    "ASCII_UPPERCASE",
                    "SORT_DEDUPLICATE",
                    "CREDIT_HUNDREDTHS",
                    "MINUTE_OF_DAY",
                    "TOKEN_AND",
                ]
                .map(str::to_owned),
            ),
            enum_schema(
                "bcsp.query.filter-validation.v1",
                [
                    "REQUIRED",
                    "DYNAMIC_DICTIONARY",
                    "NONEMPTY_WHEN_ACTIVE",
                    "ORDERED_INCLUSIVE_RANGE",
                    "ORDERED_MINUTE_INTERVAL",
                    "SECTION_INDEX_IDENTITY",
                    "STRUCTURED_ONLY",
                    "MAX32_TEXT_TOKENS",
                    "MAX128_TOKEN_BYTES",
                    "TOKEN_CONTAINS_ALPHANUMERIC",
                ]
                .map(str::to_owned),
            ),
            enum_schema(
                "bcsp.query.filter-query-encoding.v1",
                [
                    "EXACT_ONE",
                    "EXACT_ANY",
                    "TEXT_TOKEN_AND_EXACT_IDENTIFIER_PRIORITY",
                    "INCLUSIVE_RANGE",
                    "EXPLICIT_ANY_ALL",
                    "TERNARY_PRESENCE",
                    "SAME_SECTION_EXACT_ANY",
                    "SAME_SECTION_AVAILABILITY_ALL",
                    "SAME_SECTION_STRUCTURED_DIMENSIONS",
                ]
                .map(str::to_owned),
            ),
            tagged_union_schema(
                "bcsp.query.filter-canonical-neutral.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                "kind",
                &[("REQUIRED", &[]), ("JSON", &[("value", "$primitive:json")])],
            ),
            schema(
                "bcsp.query.filter-field-schema.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("stableId", "$schema:bcsp.query.filter-field-id.v1"),
                    ("requestField", "$primitive:string"),
                    ("scope", "$schema:bcsp.query.filter-scope.v1"),
                    ("valueKind", "$schema:bcsp.query.filter-value-kind.v1"),
                    ("neutral", "$schema:bcsp.query.filter-schema-value.v1"),
                    ("default", "$schema:bcsp.query.filter-schema-value.v1"),
                    (
                        "canonicalNeutral",
                        "$schema:bcsp.query.filter-canonical-neutral.v1",
                    ),
                    (
                        "normalization",
                        "$array:$schema:bcsp.query.filter-normalization.v1",
                    ),
                    (
                        "validation",
                        "$array:$schema:bcsp.query.filter-validation.v1",
                    ),
                    (
                        "queryEncoding",
                        "$schema:bcsp.query.filter-query-encoding.v1",
                    ),
                    ("i18nKey", "$primitive:string"),
                    ("chipOrder", "$primitive:u8"),
                ],
            ),
            schema(
                "bcsp.query.filter-schema.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:query-contract-version"),
                    ("fields", "$array:$schema:bcsp.query.filter-field-schema.v1"),
                ],
            ),
            enum_schema("bcsp.query.set-mode.v1", ["ANY", "ALL"].map(str::to_owned)),
            enum_schema(
                "bcsp.query.prerequisite-filter.v1",
                ["ANY", "HAS", "NONE_REPORTED"].map(str::to_owned),
            ),
            enum_schema(
                "bcsp.query.live-open-state.v1",
                ["OPEN", "CLOSED", "UNKNOWN"].map(str::to_owned),
            ),
            enum_schema(
                "bcsp.query.modality-filter.v1",
                [
                    "ON_CAMPUS_OR_IN_PERSON",
                    "ONLINE",
                    "HYBRID",
                    "OTHER",
                    "UNKNOWN",
                ]
                .map(str::to_owned),
            ),
            enum_schema(
                "bcsp.query.permission-filter.v1",
                ["ANY", "REQUIRED", "NOT_REQUIRED"].map(str::to_owned),
            ),
            enum_schema(
                "bcsp.query.weekday.v1",
                [
                    "MONDAY",
                    "TUESDAY",
                    "WEDNESDAY",
                    "THURSDAY",
                    "FRIDAY",
                    "SATURDAY",
                    "SUNDAY",
                ]
                .map(str::to_owned),
            ),
            schema(
                "bcsp.query.credit-range.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("minimumHundredths", "$optional:$primitive:u32"),
                    ("maximumHundredths", "$optional:$primitive:u32"),
                ],
            ),
            schema(
                "bcsp.query.availability-window.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("weekday", "$schema:bcsp.query.weekday.v1"),
                    ("startMinute", "$primitive:u16"),
                    ("endMinute", "$primitive:u16"),
                ],
            ),
            schema(
                "bcsp.query.core-filter.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("codes", "$array:$scalar:filter-token"),
                    ("mode", "$schema:bcsp.query.set-mode.v1"),
                ],
            ),
            schema(
                "bcsp.query.building-room-filter.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("buildingCodes", "$array:$scalar:filter-token"),
                    ("roomNumbers", "$array:$scalar:filter-token"),
                ],
            ),
            schema(
                "bcsp.query.eligibility-unit-major.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("unitCode", "$scalar:filter-token"),
                    ("majorCode", "$scalar:filter-token"),
                ],
            ),
            schema(
                "bcsp.query.eligibility-filter.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("majorCodes", "$array:$scalar:filter-token"),
                    ("minorCodes", "$array:$scalar:filter-token"),
                    ("honorProgramCodes", "$array:$scalar:filter-token"),
                    ("unitCodes", "$array:$scalar:filter-token"),
                    (
                        "unitMajors",
                        "$array:$schema:bcsp.query.eligibility-unit-major.v1",
                    ),
                ],
            ),
            schema(
                "bcsp.query.normalized-filter-values.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("term", "$scalar:term-id"),
                    ("campuses", "$array:$scalar:campus-code"),
                    ("subjects", "$array:$scalar:catalog-subject-code"),
                    ("text", "$optional:$scalar:filter-search-text"),
                    ("courseNumbers", "$array:$scalar:filter-token"),
                    ("levels", "$array:$scalar:filter-token"),
                    ("credits", "$optional:$schema:bcsp.query.credit-range.v1"),
                    ("core", "$schema:bcsp.query.core-filter.v1"),
                    ("prerequisite", "$schema:bcsp.query.prerequisite-filter.v1"),
                    ("courseLocations", "$array:$scalar:filter-token"),
                    ("sectionIndexes", "$array:$scalar:section-index"),
                    ("sectionNumbers", "$array:$scalar:filter-token"),
                    (
                        "openStatuses",
                        "$array:$schema:bcsp.query.live-open-state.v1",
                    ),
                    ("modalities", "$array:$schema:bcsp.query.modality-filter.v1"),
                    (
                        "synchronicities",
                        "$array:$schema:bcsp.catalog.synchronicity.v1",
                    ),
                    ("instructors", "$array:$scalar:filter-token"),
                    (
                        "availability",
                        "$array:$schema:bcsp.query.availability-window.v1",
                    ),
                    ("meetingLocations", "$array:$scalar:filter-token"),
                    ("buildingRoom", "$schema:bcsp.query.building-room-filter.v1"),
                    ("examCodes", "$array:$scalar:filter-token"),
                    ("permission", "$schema:bcsp.query.permission-filter.v1"),
                    ("eligibility", "$schema:bcsp.query.eligibility-filter.v1"),
                ],
            ),
            schema(
                "bcsp.query.filter-request.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("contractVersion", "$scalar:query-contract-version"),
                    ("values", "$schema:bcsp.query.normalized-filter-values.v1"),
                ],
            ),
            schema(
                "bcsp.query.page-request.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[("page", "$primitive:u32"), ("pageSize", "$primitive:u16")],
            ),
            enum_schema(
                "bcsp.query.sort-direction.v1",
                ["ASCENDING", "DESCENDING"].map(str::to_owned),
            ),
            enum_schema(
                "bcsp.query.course-sort-field.v1",
                ["RELEVANCE", "COURSE_IDENTIFIER", "TITLE"].map(str::to_owned),
            ),
            schema(
                "bcsp.query.course-sort.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("field", "$schema:bcsp.query.course-sort-field.v1"),
                    ("direction", "$schema:bcsp.query.sort-direction.v1"),
                ],
            ),
            enum_schema(
                "bcsp.query.section-sort-field.v1",
                [
                    "SECTION_INDEX",
                    "SECTION_NUMBER",
                    "COURSE_IDENTIFIER",
                    "OPEN_STATUS",
                ]
                .map(str::to_owned),
            ),
            schema(
                "bcsp.query.section-sort.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("field", "$schema:bcsp.query.section-sort-field.v1"),
                    ("direction", "$schema:bcsp.query.sort-direction.v1"),
                ],
            ),
            schema(
                "bcsp.query.course-query-request.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("filters", "$schema:bcsp.query.filter-request.v1"),
                    ("page", "$schema:bcsp.query.page-request.v1"),
                    ("sort", "$schema:bcsp.query.course-sort.v1"),
                ],
            ),
            schema(
                "bcsp.query.section-query-request.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("filters", "$schema:bcsp.query.filter-request.v1"),
                    ("page", "$schema:bcsp.query.page-request.v1"),
                    ("sort", "$schema:bcsp.query.section-sort.v1"),
                ],
            ),
            schema(
                "bcsp.query.course-detail-request.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("contractVersion", "$scalar:query-contract-version"),
                    ("key", "$schema:bcsp.identity.course-group-key.v1"),
                ],
            ),
            schema(
                "bcsp.query.section-detail-request.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("contractVersion", "$scalar:query-contract-version"),
                    ("key", "$schema:bcsp.identity.section-key.v1"),
                ],
            ),
            schema(
                "bcsp.query.live-open-evidence.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("state", "$schema:bcsp.query.live-open-state.v1"),
                    ("observedAt", "$optional:$primitive:rfc3339-timestamp"),
                    ("freshUntil", "$optional:$primitive:rfc3339-timestamp"),
                    ("uncertainty", "$optional:$schema:bcsp.match.reason-code.v1"),
                ],
            ),
            schema(
                "bcsp.query.filter-match.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("fieldId", "$schema:bcsp.query.filter-field-id.v1"),
                    ("explanation", "$schema:bcsp.match.explanation.v1"),
                ],
            ),
            schema(
                "bcsp.query.text-match-evidence.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("exactCourseIdentifier", "$primitive:bool"),
                    ("matchedTokens", "$array:$primitive:string"),
                ],
            ),
            schema(
                "bcsp.query.section-query-item.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("section", "$schema:bcsp.catalog.normalized-section.v1"),
                    (
                        "occurrences",
                        "$array:$schema:bcsp.catalog.normalized-occurrence.v1",
                    ),
                    ("open", "$schema:bcsp.query.live-open-evidence.v1"),
                    ("explanation", "$schema:bcsp.match.explanation.v1"),
                    ("filterMatches", "$array:$schema:bcsp.query.filter-match.v1"),
                ],
            ),
            schema(
                "bcsp.query.section-search-item.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    (
                        "variant",
                        "$schema:bcsp.catalog.normalized-course-variant.v1",
                    ),
                    ("section", "$schema:bcsp.query.section-query-item.v1"),
                    (
                        "courseFilterMatches",
                        "$array:$schema:bcsp.query.filter-match.v1",
                    ),
                    (
                        "textMatch",
                        "$optional:$schema:bcsp.query.text-match-evidence.v1",
                    ),
                ],
            ),
            schema(
                "bcsp.query.course-variant-query-item.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    (
                        "variant",
                        "$schema:bcsp.catalog.normalized-course-variant.v1",
                    ),
                    ("explanation", "$schema:bcsp.match.explanation.v1"),
                    ("filterMatches", "$array:$schema:bcsp.query.filter-match.v1"),
                    (
                        "textMatch",
                        "$optional:$schema:bcsp.query.text-match-evidence.v1",
                    ),
                    (
                        "sections",
                        "$array:$schema:bcsp.query.section-query-item.v1",
                    ),
                ],
            ),
            schema(
                "bcsp.query.course-query-item.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("group", "$schema:bcsp.catalog.normalized-course-group.v1"),
                    ("explanation", "$schema:bcsp.match.explanation.v1"),
                    (
                        "variants",
                        "$array:$schema:bcsp.query.course-variant-query-item.v1",
                    ),
                ],
            ),
            schema(
                "bcsp.query.page-info.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("page", "$primitive:u32"),
                    ("pageSize", "$primitive:u16"),
                    ("total", "$primitive:u64"),
                    ("totalPages", "$primitive:u32"),
                ],
            ),
            schema(
                "bcsp.query.course-query-response.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:query-contract-version"),
                    ("page", "$schema:bcsp.query.page-info.v1"),
                    ("items", "$array:$schema:bcsp.query.course-query-item.v1"),
                ],
            ),
            schema(
                "bcsp.query.section-query-response.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:query-contract-version"),
                    ("page", "$schema:bcsp.query.page-info.v1"),
                    ("items", "$array:$schema:bcsp.query.section-search-item.v1"),
                ],
            ),
            schema(
                "bcsp.query.course-detail-response.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:query-contract-version"),
                    ("course", "$schema:bcsp.query.course-query-item.v1"),
                ],
            ),
            schema(
                "bcsp.query.section-detail-response.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:query-contract-version"),
                    (
                        "variant",
                        "$schema:bcsp.catalog.normalized-course-variant.v1",
                    ),
                    ("section", "$schema:bcsp.query.section-query-item.v1"),
                ],
            ),
            enum_schema(
                "bcsp.watch.notification-mode.v1",
                WatchNotificationMode::ALL.iter().map(|value| {
                    serde_json::to_value(value)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned()
                }),
            ),
            tagged_union_schema(
                "bcsp.watch.continuous-duration.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                "kind",
                &[
                    ("FINITE", &[("seconds", "$scalar:watch-positive-u64")]),
                    ("UNLIMITED", &[]),
                ],
            ),
            schema(
                "bcsp.watch.policy.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    (
                        "notificationMode",
                        "$schema:bcsp.watch.notification-mode.v1",
                    ),
                    ("maxAudible", "$scalar:watch-positive-u64"),
                    (
                        "continuousDuration",
                        "$schema:bcsp.watch.continuous-duration.v1",
                    ),
                ],
            ),
            schema(
                "bcsp.watch.start-item.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                    ("policy", "$schema:bcsp.watch.policy.v1"),
                ],
            ),
            schema(
                "bcsp.watch.active-target.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("activeWatchId", "$scalar:trace-id"),
                    ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                ],
            ),
            schema(
                "bcsp.watch.episode-target.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("activeWatchId", "$scalar:trace-id"),
                    ("episodeId", "$scalar:trace-id"),
                    ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                ],
            ),
            schema(
                "bcsp.watch.alert-target.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("activeWatchId", "$scalar:trace-id"),
                    ("episodeId", "$scalar:trace-id"),
                    ("alertId", "$scalar:trace-id"),
                    ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                ],
            ),
            enum_schema(
                "bcsp.watch.cue-outcome.v1",
                WatchCueOutcome::ALL.iter().map(|value| {
                    serde_json::to_value(value)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned()
                }),
            ),
            schema(
                "bcsp.watch.cue-outcome-report.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                &[
                    ("cueId", "$scalar:trace-id"),
                    ("activeWatchId", "$scalar:trace-id"),
                    ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                    ("outcome", "$schema:bcsp.watch.cue-outcome.v1"),
                    ("reportedAt", "$primitive:rfc3339-timestamp"),
                ],
            ),
            tagged_union_schema(
                "bcsp.watch.client-command.v1",
                SchemaDirection::ClientToServer,
                UnknownFieldPolicy::Reject,
                "type",
                &[
                    (
                        "START_WATCH",
                        &[("items", "$array:$schema:bcsp.watch.start-item.v1")],
                    ),
                    (
                        "STOP_WATCH",
                        &[("watch", "$schema:bcsp.watch.active-target.v1")],
                    ),
                    (
                        "UPDATE_POLICY",
                        &[
                            ("watch", "$schema:bcsp.watch.active-target.v1"),
                            ("policy", "$schema:bcsp.watch.policy.v1"),
                        ],
                    ),
                    (
                        "ACKNOWLEDGE_EPISODE",
                        &[("episode", "$schema:bcsp.watch.episode-target.v1")],
                    ),
                    ("ACKNOWLEDGE_ALL_EPISODES", &[]),
                    (
                        "RESUME_TIMED_OUT_EPISODE",
                        &[("episode", "$schema:bcsp.watch.episode-target.v1")],
                    ),
                    (
                        "RESET_AUDIBLE_COUNT",
                        &[("watch", "$schema:bcsp.watch.active-target.v1")],
                    ),
                    (
                        "REPORT_CUE_OUTCOME",
                        &[("report", "$schema:bcsp.watch.cue-outcome-report.v1")],
                    ),
                    (
                        "DISMISS_ALERT",
                        &[("alert", "$schema:bcsp.watch.alert-target.v1")],
                    ),
                ],
            ),
            enum_schema(
                "bcsp.watch.start-rejection-reason.v1",
                WatchStartRejectionReason::ALL.iter().map(|value| {
                    serde_json::to_value(value)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned()
                }),
            ),
            tagged_union_schema(
                "bcsp.watch.start-item-result.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                "status",
                &[
                    (
                        "ACTIVE",
                        &[
                            ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                            ("activeWatchId", "$scalar:trace-id"),
                            ("startedAt", "$primitive:rfc3339-timestamp"),
                        ],
                    ),
                    (
                        "REJECTED",
                        &[
                            ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                            ("reason", "$schema:bcsp.watch.start-rejection-reason.v1"),
                        ],
                    ),
                ],
            ),
            schema(
                "bcsp.watch.start-result.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:watch-contract-version"),
                    ("items", "$array:$schema:bcsp.watch.start-item-result.v1"),
                    ("activeWatchCount", "$primitive:u8"),
                ],
            ),
            enum_schema(
                "bcsp.watch.stop-reason.v1",
                WatchStopReason::ALL.iter().map(|value| {
                    serde_json::to_value(value)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned()
                }),
            ),
            schema(
                "bcsp.watch.stopped.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:watch-contract-version"),
                    ("activeWatchId", "$scalar:trace-id"),
                    ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                    ("reason", "$schema:bcsp.watch.stop-reason.v1"),
                    ("stoppedAt", "$primitive:rfc3339-timestamp"),
                ],
            ),
            schema(
                "bcsp.watch.open-observation-fanout.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:watch-contract-version"),
                    ("activeWatchId", "$scalar:trace-id"),
                    ("observation", "$schema:bcsp.open.observation.v1"),
                ],
            ),
            enum_schema(
                "bcsp.watch.episode-state.v1",
                OpenEpisodeState::ALL.iter().map(|value| {
                    serde_json::to_value(value)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned()
                }),
            ),
            schema_with_optional_fields(
                "bcsp.watch.open-episode.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:watch-contract-version"),
                    ("episodeId", "$scalar:trace-id"),
                    ("activeWatchId", "$scalar:trace-id"),
                    ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                    ("state", "$schema:bcsp.watch.episode-state.v1"),
                    (
                        "notificationMode",
                        "$schema:bcsp.watch.notification-mode.v1",
                    ),
                    (
                        "continuousDuration",
                        "$schema:bcsp.watch.continuous-duration.v1",
                    ),
                    ("maxAudible", "$scalar:watch-positive-u64"),
                    ("audibleCount", "$primitive:u64"),
                    ("firstObservedAt", "$primitive:rfc3339-timestamp"),
                    ("lastObservedAt", "$primitive:rfc3339-timestamp"),
                    ("observationCount", "$scalar:watch-positive-u64"),
                    ("latestObservationId", "$scalar:trace-id"),
                    ("stateChangedAt", "$primitive:rfc3339-timestamp"),
                ],
                &[("closedAt", "$primitive:rfc3339-timestamp")],
            ),
            enum_schema(
                "bcsp.watch.alert-disposition.v1",
                WatchAlertDisposition::ALL.iter().map(|value| {
                    serde_json::to_value(value)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned()
                }),
            ),
            schema(
                "bcsp.watch.alert.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:watch-contract-version"),
                    ("alertId", "$scalar:trace-id"),
                    ("disposition", "$schema:bcsp.watch.alert-disposition.v1"),
                    ("visible", "$primitive:bool"),
                    ("episode", "$schema:bcsp.watch.open-episode.v1"),
                ],
            ),
            tagged_union_schema(
                "bcsp.watch.audio-trigger.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                "kind",
                &[
                    (
                        "ONE_SHOT_OBSERVATION",
                        &[("observationId", "$scalar:trace-id")],
                    ),
                    ("CONTINUOUS_EPISODE", &[("episodeId", "$scalar:trace-id")]),
                ],
            ),
            schema(
                "bcsp.watch.audio-cue.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("cueId", "$scalar:trace-id"),
                    ("activeWatchId", "$scalar:trace-id"),
                    ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                    ("trigger", "$schema:bcsp.watch.audio-trigger.v1"),
                    ("emittedAt", "$primitive:rfc3339-timestamp"),
                ],
            ),
            enum_schema(
                "bcsp.watch.continuous-mixer-stop-reason.v1",
                WatchContinuousMixerStopReason::ALL.iter().map(|value| {
                    serde_json::to_value(value)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned()
                }),
            ),
            enum_schema(
                "bcsp.watch.cue-cancellation-reason.v1",
                WatchCueCancellationReason::ALL.iter().map(|value| {
                    serde_json::to_value(value)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_owned()
                }),
            ),
            tagged_union_schema(
                "bcsp.watch.audio-disposition.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                "disposition",
                &[
                    (
                        "CUE_REQUESTED",
                        &[("cue", "$schema:bcsp.watch.audio-cue.v1")],
                    ),
                    (
                        "CUE_QUEUED",
                        &[
                            ("activeWatchId", "$scalar:trace-id"),
                            ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                            ("observationId", "$scalar:trace-id"),
                            ("queuedAt", "$primitive:rfc3339-timestamp"),
                        ],
                    ),
                    (
                        "CUE_CANCELLED",
                        &[
                            ("activeWatchId", "$scalar:trace-id"),
                            ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                            ("observationId", "$scalar:trace-id"),
                            ("cueId", "$optional:$scalar:trace-id"),
                            ("reason", "$schema:bcsp.watch.cue-cancellation-reason.v1"),
                            ("cancelledAt", "$primitive:rfc3339-timestamp"),
                        ],
                    ),
                    (
                        "SILENT_MAX_AUDIBLE",
                        &[
                            ("activeWatchId", "$scalar:trace-id"),
                            ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                            ("observationId", "$scalar:trace-id"),
                            ("audibleCount", "$primitive:u64"),
                            ("maxAudible", "$scalar:watch-positive-u64"),
                            ("emittedAt", "$primitive:rfc3339-timestamp"),
                        ],
                    ),
                    (
                        "CONTINUOUS_MIXER_ACTIVE",
                        &[
                            ("episodeIds", "$array:$scalar:trace-id"),
                            ("emittedAt", "$primitive:rfc3339-timestamp"),
                        ],
                    ),
                    (
                        "CONTINUOUS_MIXER_STOPPED",
                        &[
                            (
                                "reason",
                                "$schema:bcsp.watch.continuous-mixer-stop-reason.v1",
                            ),
                            ("emittedAt", "$primitive:rfc3339-timestamp"),
                        ],
                    ),
                ],
            ),
            schema(
                "bcsp.watch.cue-outcome-receipt.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$scalar:watch-contract-version"),
                    ("cueId", "$scalar:trace-id"),
                    ("activeWatchId", "$scalar:trace-id"),
                    ("sectionKey", "$schema:bcsp.identity.section-key.v1"),
                    ("outcome", "$schema:bcsp.watch.cue-outcome.v1"),
                    ("audibleCountConsumed", "$primitive:bool"),
                    ("audibleCount", "$primitive:u64"),
                    ("recordedAt", "$primitive:rfc3339-timestamp"),
                ],
            ),
            tagged_union_schema(
                "bcsp.watch.server-event.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                "type",
                &[
                    (
                        "START_RESULT",
                        &[("result", "$schema:bcsp.watch.start-result.v1")],
                    ),
                    (
                        "WATCH_STOPPED",
                        &[("stopped", "$schema:bcsp.watch.stopped.v1")],
                    ),
                    (
                        "OPEN_OBSERVATION",
                        &[("fanout", "$schema:bcsp.watch.open-observation-fanout.v1")],
                    ),
                    (
                        "EPISODE_UPDATED",
                        &[("episode", "$schema:bcsp.watch.open-episode.v1")],
                    ),
                    ("ALERT_UPDATED", &[("alert", "$schema:bcsp.watch.alert.v1")]),
                    (
                        "AUDIO_DISPOSITION",
                        &[("audio", "$schema:bcsp.watch.audio-disposition.v1")],
                    ),
                    (
                        "CUE_OUTCOME_RECORDED",
                        &[("receipt", "$schema:bcsp.watch.cue-outcome-receipt.v1")],
                    ),
                ],
            ),
            enum_schema(
                "bcsp.service.runtime.v1",
                serialized_enum_values(ServiceRuntimeV1::ALL),
            ),
            enum_schema(
                "bcsp.service.level.v1",
                serialized_enum_values(ServiceLevelV1::ALL),
            ),
            enum_schema(
                "bcsp.service.operation-phase.v1",
                serialized_enum_values(ServiceOperationPhaseV1::ALL),
            ),
            enum_schema(
                "bcsp.service.availability.v1",
                serialized_enum_values(ServiceAvailabilityV1::ALL),
            ),
            enum_schema(
                "bcsp.service.issue-component.v1",
                serialized_enum_values(ServiceIssueComponentV1::ALL),
            ),
            enum_schema(
                "bcsp.service.issue-severity.v1",
                serialized_enum_values(ServiceIssueSeverityV1::ALL),
            ),
            enum_schema(
                "bcsp.service.issue-recovery.v1",
                serialized_enum_values(ServiceIssueRecoveryV1::ALL),
            ),
            schema(
                "bcsp.service.operation.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("phase", "$schema:bcsp.service.operation-phase.v1"),
                    (
                        "target",
                        "$optional:$schema:bcsp.identity.term-campus-key.v1",
                    ),
                    ("startedAt", "$optional:$primitive:rfc3339-timestamp"),
                    ("nextRetryAt", "$optional:$primitive:rfc3339-timestamp"),
                ],
            ),
            schema(
                "bcsp.service.availability-summary.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("totalTargetCount", "$primitive:u64"),
                    ("availableTargetCount", "$primitive:u64"),
                ],
            ),
            schema(
                "bcsp.service.target-status.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("target", "$schema:bcsp.identity.term-campus-key.v1"),
                    ("primary", "$primitive:bool"),
                    (
                        "catalogAvailability",
                        "$schema:bcsp.service.availability.v1",
                    ),
                    (
                        "catalogContentVersion",
                        "$optional:$scalar:catalog-content-version",
                    ),
                    ("openAvailability", "$schema:bcsp.service.availability.v1"),
                    ("searchAvailable", "$primitive:bool"),
                ],
            ),
            schema(
                "bcsp.service.issue.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("component", "$schema:bcsp.service.issue-component.v1"),
                    (
                        "target",
                        "$optional:$schema:bcsp.identity.term-campus-key.v1",
                    ),
                    ("code", "$primitive:string"),
                    ("severity", "$schema:bcsp.service.issue-severity.v1"),
                    ("recovery", "$schema:bcsp.service.issue-recovery.v1"),
                    ("retryAt", "$optional:$primitive:rfc3339-timestamp"),
                ],
            ),
            schema(
                "bcsp.service.status.v1",
                SchemaDirection::ServerToClient,
                UnknownFieldPolicy::Ignore,
                &[
                    ("contractVersion", "$primitive:u16"),
                    ("observedAt", "$primitive:rfc3339-timestamp"),
                    ("runtime", "$schema:bcsp.service.runtime.v1"),
                    ("level", "$schema:bcsp.service.level.v1"),
                    ("operation", "$schema:bcsp.service.operation.v1"),
                    ("discovery", "$schema:bcsp.catalog.discovery-status.v1"),
                    ("catalog", "$schema:bcsp.service.availability-summary.v1"),
                    ("open", "$schema:bcsp.service.availability-summary.v1"),
                    ("targets", "$array:$schema:bcsp.service.target-status.v1"),
                    ("issues", "$array:$schema:bcsp.service.issue.v1"),
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
                    ("CURRENT_REVISION", &[("revision", "$primitive:u64")]),
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
