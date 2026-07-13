use std::collections::BTreeSet;
use std::str::FromStr;

use bcsp_contracts::{
    ApiErrorBody, ApiErrorCode, ApiErrorDetail, ApiErrorEnvelope, ContractSchema, CourseGroupKey,
    CourseVariantKey, DetailName, HttpRequestEnvelope, HttpSuccessEnvelope, MatchExplanation,
    MatchOutcome, MatchReason, MatchReasonCode, ReasonField, SectionKey, TermCampusKey, TraceId,
    WsClientEnvelope, WsServerEnvelope, contract_manifest,
};
use serde::Serialize;
use serde_json::Value;

fn schema(id: &str) -> ContractSchema {
    contract_manifest()
        .schemas
        .into_iter()
        .find(|schema| schema.id == id)
        .unwrap_or_else(|| panic!("missing contract schema {id}"))
}

fn object_keys<T: Serialize>(value: &T) -> BTreeSet<String> {
    let Value::Object(object) = serde_json::to_value(value).expect("serialize representative")
    else {
        panic!("representative contract is not an object")
    };
    object.into_iter().map(|(key, _)| key).collect()
}

fn assert_object_binding<T: Serialize>(schema_id: &str, value: &T) {
    let schema = schema(schema_id);
    assert!(schema.discriminator.is_none(), "{schema_id}");
    assert!(schema.variants.is_empty(), "{schema_id}");
    assert!(schema.fields.iter().all(|field| field.required));
    let expected = schema
        .fields
        .into_iter()
        .map(|field| field.name)
        .collect::<BTreeSet<_>>();
    assert_eq!(object_keys(value), expected, "{schema_id}");
}

fn assert_tagged_variant_binding<T: Serialize>(schema_id: &str, tag: &str, value: &T) {
    let schema = schema(schema_id);
    let discriminator = schema
        .discriminator
        .as_deref()
        .expect("tagged union declares a discriminator");
    let variant = schema
        .variants
        .iter()
        .find(|variant| variant.tag_value == tag)
        .unwrap_or_else(|| panic!("missing {schema_id} variant {tag}"));
    assert!(variant.fields.iter().all(|field| field.required));

    let mut expected = variant
        .fields
        .iter()
        .map(|field| field.name.clone())
        .collect::<BTreeSet<_>>();
    expected.insert(discriminator.to_owned());
    assert_eq!(object_keys(value), expected, "{schema_id}:{tag}");

    let serialized = serde_json::to_value(value).expect("serialize tagged representative");
    assert_eq!(serialized[discriminator], Value::String(tag.to_owned()));
}

fn trace_id() -> TraceId {
    TraceId::from_str("00000000-0000-4000-8000-000000000001").expect("valid deterministic trace ID")
}

#[test]
fn manifest_object_fields_are_bound_to_representative_serde_shapes() {
    let target = TermCampusKey::try_new("T2026F", "CAMPUS_A").unwrap();
    let section = SectionKey::try_new("T2026F", "CAMPUS_A", "00001").unwrap();
    let group = CourseGroupKey::try_new("T2026F", "CAMPUS_A", "00:000:001").unwrap();
    let variant =
        CourseVariantKey::try_new(group.clone(), &format!("v1:{}", "a".repeat(64))).unwrap();
    let reason = MatchReason::new(
        MatchReasonCode::MissingReliableData,
        ReasonField::try_from("meeting.location").unwrap(),
    );
    let explanation =
        MatchExplanation::try_new(MatchOutcome::Uncertain, vec![reason.clone()]).unwrap();
    let body =
        ApiErrorBody::new(ApiErrorCode::SelectionLimitExceeded, trace_id()).with_details(vec![
            ApiErrorDetail::Limit {
                name: DetailName::try_from("selected_sections").unwrap(),
                maximum: 9,
            },
        ]);
    let error = ApiErrorEnvelope::new(body.clone());

    assert_object_binding("bcsp.identity.term-campus-key.v1", &target);
    assert_object_binding("bcsp.identity.section-key.v1", &section);
    assert_object_binding("bcsp.identity.course-group-key.v1", &group);
    assert_object_binding("bcsp.identity.course-variant-key.v1", &variant);
    assert_object_binding("bcsp.match.reason.v1", &reason);
    assert_object_binding("bcsp.match.explanation.v1", &explanation);
    assert_object_binding(
        "bcsp.http.request-envelope.v1",
        &HttpRequestEnvelope::new(section.clone()),
    );
    assert_object_binding(
        "bcsp.http.success-envelope.v1",
        &HttpSuccessEnvelope::new(section.clone()),
    );
    assert_object_binding("bcsp.http.error-body.v1", &body);
    assert_object_binding("bcsp.http.error-envelope.v1", &error);
    assert_object_binding(
        "bcsp.ws.client-envelope.v1",
        &WsClientEnvelope::new(trace_id(), section.clone()),
    );
    assert_object_binding(
        "bcsp.ws.server-envelope.v1",
        &WsServerEnvelope::new(trace_id(), section),
    );
}

#[test]
fn error_detail_tagged_union_is_bound_to_every_serde_variant() {
    assert_tagged_variant_binding(
        "bcsp.http.error-detail.v1",
        "INVALID_FIELD",
        &ApiErrorDetail::InvalidField {
            field: DetailName::try_from("section_key").unwrap(),
        },
    );
    assert_tagged_variant_binding(
        "bcsp.http.error-detail.v1",
        "LIMIT",
        &ApiErrorDetail::Limit {
            name: DetailName::try_from("selected_sections").unwrap(),
            maximum: 9,
        },
    );
    assert_tagged_variant_binding(
        "bcsp.http.error-detail.v1",
        "RETRY_AFTER_SECONDS",
        &ApiErrorDetail::RetryAfterSeconds { seconds: 30 },
    );
}

#[test]
fn manifest_enum_values_are_bound_to_actual_serde_names() {
    let cases = [
        (
            "bcsp.error.shared-code.v1",
            ApiErrorCode::ALL
                .iter()
                .map(|value| serde_json::to_value(value).unwrap())
                .collect::<Vec<_>>(),
        ),
        (
            "bcsp.match.outcome.v1",
            MatchOutcome::ALL
                .iter()
                .map(|value| serde_json::to_value(value).unwrap())
                .collect::<Vec<_>>(),
        ),
        (
            "bcsp.match.reason-code.v1",
            MatchReasonCode::ALL
                .iter()
                .map(|value| serde_json::to_value(value).unwrap())
                .collect::<Vec<_>>(),
        ),
    ];

    for (schema_id, actual) in cases {
        let expected = schema(schema_id)
            .enum_values
            .into_iter()
            .map(Value::String)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "{schema_id}");
    }
}
