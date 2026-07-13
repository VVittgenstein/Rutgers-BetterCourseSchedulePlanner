use std::str::FromStr;

use bcsp_contracts::{
    ApiErrorBody, ApiErrorCode, ApiErrorDetail, ApiErrorEnvelope, CourseGroupKey, CourseVariantKey,
    DetailName, HttpRequestEnvelope, HttpSuccessEnvelope, MatchExplanation, MatchOutcome,
    MatchReason, MatchReasonCode, ReasonField, SectionKey, TraceId, WsClientEnvelope,
    WsServerEnvelope, contract_manifest,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct IdentityPayload {
    section_key: SectionKey,
    course_group_key: CourseGroupKey,
    course_variant_key: CourseVariantKey,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum WatchCommandKind {
    Start,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct WatchCommand {
    kind: WatchCommandKind,
    section_keys: Vec<SectionKey>,
}

fn section_key() -> SectionKey {
    SectionKey::try_new("T2026F", "CAMPUS_A", "00001").expect("valid synthetic section key")
}

fn group_key() -> CourseGroupKey {
    CourseGroupKey::try_new("T2026F", "CAMPUS_A", "00:000:001")
        .expect("valid synthetic course group key")
}

fn variant_key() -> CourseVariantKey {
    CourseVariantKey::try_new(group_key(), &format!("v1:{}", "a".repeat(64)))
        .expect("valid synthetic variant key")
}

fn trace_id() -> TraceId {
    TraceId::from_str("00000000-0000-4000-8000-000000000001").expect("valid deterministic trace id")
}

fn identity_payload() -> IdentityPayload {
    IdentityPayload {
        section_key: section_key(),
        course_group_key: group_key(),
        course_variant_key: variant_key(),
    }
}

fn match_explanation() -> MatchExplanation {
    MatchExplanation::try_new(
        MatchOutcome::Uncertain,
        vec![MatchReason::new(
            MatchReasonCode::MissingReliableData,
            ReasonField::try_from("meeting.location").expect("valid stable field name"),
        )],
    )
    .expect("valid uncertainty explanation")
}

fn pretty<T: Serialize>(value: &T) -> String {
    format!(
        "{}\n",
        serde_json::to_string_pretty(value).expect("serialize golden value")
    )
}

fn assert_golden<T>(value: &T, expected: &str)
where
    T: Serialize,
{
    assert_eq!(pretty(value), expected);
}

#[test]
fn section_key_wire_is_exact_and_round_trips() {
    let value = section_key();
    let golden = include_str!("golden/section-key-v1.json");
    assert_golden(&value, golden);
    assert_eq!(serde_json::from_str::<SectionKey>(golden).unwrap(), value);
}

#[test]
fn identity_payload_wire_is_exact_and_round_trips() {
    let value = identity_payload();
    let golden = include_str!("golden/identity-payload-v1.json");
    assert_golden(&value, golden);
    assert_eq!(
        serde_json::from_str::<IdentityPayload>(golden).unwrap(),
        value
    );
}

#[test]
fn http_request_wire_is_exact_and_round_trips() {
    let value = HttpRequestEnvelope::new(identity_payload());
    let golden = include_str!("golden/http-request-v1.json");
    assert_golden(&value, golden);
    assert_eq!(
        serde_json::from_str::<HttpRequestEnvelope<IdentityPayload>>(golden).unwrap(),
        value
    );
}

#[test]
fn http_success_wire_is_exact_and_round_trips() {
    let value = HttpSuccessEnvelope::new(identity_payload());
    let golden = include_str!("golden/http-success-v1.json");
    assert_golden(&value, golden);
    assert_eq!(
        serde_json::from_str::<HttpSuccessEnvelope<IdentityPayload>>(golden).unwrap(),
        value
    );
}

#[test]
fn http_error_wire_derives_message_key_from_code() {
    let value = ApiErrorEnvelope::new(
        ApiErrorBody::new(ApiErrorCode::SelectionLimitExceeded, trace_id()).with_details(vec![
            ApiErrorDetail::Limit {
                name: DetailName::try_from("selected_sections").unwrap(),
                maximum: 9,
            },
        ]),
    );
    let golden = include_str!("golden/http-error-v1.json");
    assert_golden(&value, golden);
    assert_eq!(
        serde_json::from_str::<ApiErrorEnvelope>(golden).unwrap(),
        value
    );
}

#[test]
fn websocket_client_wire_is_exact_and_round_trips() {
    let value = WsClientEnvelope::new(
        trace_id(),
        WatchCommand {
            kind: WatchCommandKind::Start,
            section_keys: vec![section_key()],
        },
    );
    let golden = include_str!("golden/ws-client-envelope-v1.json");
    assert_golden(&value, golden);
    assert_eq!(
        serde_json::from_str::<WsClientEnvelope<WatchCommand>>(golden).unwrap(),
        value
    );
}

#[test]
fn websocket_server_wire_is_exact_and_round_trips() {
    let value = WsServerEnvelope::new(
        trace_id(),
        WatchCommand {
            kind: WatchCommandKind::Start,
            section_keys: vec![section_key()],
        },
    );
    let golden = include_str!("golden/ws-server-envelope-v1.json");
    assert_golden(&value, golden);
    assert_eq!(
        serde_json::from_str::<WsServerEnvelope<WatchCommand>>(golden).unwrap(),
        value
    );
}

#[test]
fn match_explanation_wire_is_exact_and_round_trips() {
    let value = match_explanation();
    let golden = include_str!("golden/match-explanation-v1.json");
    assert_golden(&value, golden);
    assert_eq!(
        serde_json::from_str::<MatchExplanation>(golden).unwrap(),
        value
    );
}

#[test]
fn contract_manifest_is_an_exact_generation_input() {
    let value = contract_manifest();
    let golden = include_str!("golden/contract-manifest-v1.json");
    assert_golden(&value, golden);
    assert_eq!(
        serde_json::from_str::<bcsp_contracts::ContractManifest>(golden).unwrap(),
        value
    );
}
