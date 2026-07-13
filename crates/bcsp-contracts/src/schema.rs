use serde::{Deserialize, Serialize};

use crate::identity::{
    CAMPUS_MAX_BYTES, COURSE_STRING_MAX_BYTES, FINGERPRINT_PREFIX, SECTION_INDEX_WIDTH,
    SHA256_HEX_BYTES, TERM_MAX_BYTES,
};
use crate::{
    API_PROTOCOL_VERSION, ApiErrorCode, MatchOutcome, MatchReasonCode, WS_PROTOCOL_VERSION,
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
