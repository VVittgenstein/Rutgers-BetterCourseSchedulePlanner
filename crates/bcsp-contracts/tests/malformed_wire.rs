use std::collections::BTreeSet;
use std::str::FromStr;

use bcsp_contracts::{
    API_PROTOCOL_VERSION, ApiErrorBody, ApiErrorCode, CatalogContentVersion, CatalogDiagnosticCode,
    CatalogDiscoveryRequestV1, CatalogDiscoverySourceId, CatalogPayloadDigest, CatalogSubjectCode,
    ContractDecodeError, CourseGroupKey, CourseVariantKey, HttpRequestEnvelope, MatchExplanation,
    ProtocolVersion, SectionKey, TraceId, TraceIdError, TraceIdSource, WS_PROTOCOL_VERSION,
    WsClientEnvelope, decode_versioned_envelope_json,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SectionPayload {
    section_key: SectionKey,
}

#[test]
fn malformed_identities_fail_closed() {
    let cases = [
        r#"{"term":"T2026F","campus":"campus_a","index":"00001"}"#,
        r#"{"term":"T2026F","campus":"CAMPUS/A","index":"00001"}"#,
        r#"{"term":"T2026F","campus":"CAMPUS_A","index":"00A01"}"#,
        r#"{"term":"T2026F","campus":"CAMPUS_A","index":"00001","surrogate":7}"#,
        r#"{"campus":"CAMPUS_A","index":"00001"}"#,
    ];
    for json in cases {
        assert!(serde_json::from_str::<SectionKey>(json).is_err(), "{json}");
    }
}

#[test]
fn malformed_catalog_scalars_and_strict_requests_fail_closed() {
    assert!(serde_json::from_str::<CatalogContentVersion>("0").is_err());
    assert!(
        serde_json::from_str::<CatalogPayloadDigest>(&format!("\"{}\"", "A".repeat(64))).is_err()
    );
    assert!(CatalogSubjectCode::try_from(" SYN:SUBJECT").is_err());
    assert!(CatalogSubjectCode::try_from("SYN:SUBJECT\n").is_err());
    assert!(CatalogDiscoverySourceId::try_from("https://source.invalid").is_err());
    assert!(CatalogDiagnosticCode::try_from("parser detail leaked").is_err());
    assert!(CatalogDiagnosticCode::try_from("SAFE_CODE\nforged").is_err());
    assert!(
        serde_json::from_str::<CatalogDiscoveryRequestV1>(
            r#"{"contractVersion":1,"futureMutationSemantics":true}"#,
        )
        .is_err()
    );
}

#[test]
fn duplicate_wire_fields_fail_closed() {
    let section = r#"{
      "term": "T2026F",
      "term": "T2027S",
      "campus": "CAMPUS_A",
      "index": "00001"
    }"#;
    assert!(serde_json::from_str::<SectionKey>(section).is_err());

    let request = r#"{
      "protocolVersion": 1,
      "protocolVersion": 1,
      "payload": {"sectionKey":{"term":"T2026F","campus":"CAMPUS_A","index":"00001"}}
    }"#;
    assert!(serde_json::from_str::<HttpRequestEnvelope<SectionPayload>>(request).is_err());
}

#[test]
fn course_variant_fingerprint_is_versioned_lowercase_sha256() {
    let group = CourseGroupKey::try_new("T2026F", "CAMPUS_A", "00:000:001").unwrap();
    for invalid in [
        "a",
        &format!("v1:{}", "a".repeat(63)),
        &format!("v1:{}", "A".repeat(64)),
        &format!("v2:{}", "a".repeat(64)),
    ] {
        assert!(CourseVariantKey::try_new(group.clone(), invalid).is_err());
    }
}

#[test]
fn unsupported_or_malformed_protocol_versions_are_rejected() {
    assert!(ProtocolVersion::try_from(0).is_err());
    assert!(ProtocolVersion::try_from(2).is_err());
    assert!(serde_json::from_str::<ProtocolVersion>("65536").is_err());
    assert_eq!(API_PROTOCOL_VERSION, WS_PROTOCOL_VERSION);
}

#[test]
fn versioned_decoder_distinguishes_unsupported_from_malformed() {
    let valid = br#"{
      "protocolVersion": 1,
      "payload": {"sectionKey":{"term":"T2026F","campus":"CAMPUS_A","index":"00001"}}
    }"#;
    let decoded =
        decode_versioned_envelope_json::<HttpRequestEnvelope<SectionPayload>>(valid).unwrap();
    assert_eq!(decoded.payload().section_key.index().as_str(), "00001");

    let unsupported = br#"{
      "protocolVersion": 2,
      "payload": {"sectionKey":{"term":"T2026F","campus":"CAMPUS_A","index":"00001"}}
    }"#;
    let error = decode_versioned_envelope_json::<HttpRequestEnvelope<SectionPayload>>(unsupported)
        .unwrap_err();
    assert_eq!(error.code(), ApiErrorCode::UnsupportedProtocolVersion);
    assert_eq!(error.observed_protocol_version(), Some(2));
    assert_eq!(error.to_string(), "unsupported protocol version");

    let malformed = br#"{
      "protocolVersion": "1",
      "payload": {"sectionKey":{"term":"T2026F","campus":"CAMPUS_A","index":"00001"}}
    }"#;
    let error = decode_versioned_envelope_json::<HttpRequestEnvelope<SectionPayload>>(malformed)
        .unwrap_err();
    assert_eq!(error, ContractDecodeError::MalformedRequest);

    let missing = br#"{
      "payload": {"sectionKey":{"term":"T2026F","campus":"CAMPUS_A","index":"00001"}}
    }"#;
    let error =
        decode_versioned_envelope_json::<HttpRequestEnvelope<SectionPayload>>(missing).unwrap_err();
    assert_eq!(error, ContractDecodeError::MalformedRequest);
}

#[test]
fn request_and_client_control_envelopes_reject_unknown_fields() {
    let request = r#"{
      "protocolVersion": 1,
      "payload": {"sectionKey":{"term":"T2026F","campus":"CAMPUS_A","index":"00001"}},
      "ignored": true
    }"#;
    assert!(serde_json::from_str::<HttpRequestEnvelope<SectionPayload>>(request).is_err());

    let websocket = r#"{
      "protocolVersion": 1,
      "messageId": "00000000-0000-4000-8000-000000000001",
      "payload": {"sectionKey":{"term":"T2026F","campus":"CAMPUS_A","index":"00001"}},
      "ignored": true
    }"#;
    assert!(serde_json::from_str::<WsClientEnvelope<SectionPayload>>(websocket).is_err());
}

#[test]
fn error_code_and_message_key_cannot_diverge() {
    let invalid = r#"{
      "code": "SECTION_NOT_FOUND",
      "messageKey": "error.internal",
      "traceId": "00000000-0000-4000-8000-000000000001",
      "details": []
    }"#;
    assert!(serde_json::from_str::<ApiErrorBody>(invalid).is_err());

    let missing_details = r#"{
      "code": "SECTION_NOT_FOUND",
      "messageKey": "error.section_not_found",
      "traceId": "00000000-0000-4000-8000-000000000001"
    }"#;
    assert!(serde_json::from_str::<ApiErrorBody>(missing_details).is_err());

    let codes = ApiErrorCode::ALL
        .iter()
        .map(|code| code.wire_name())
        .collect::<BTreeSet<_>>();
    let keys = ApiErrorCode::ALL
        .iter()
        .map(|code| code.message_key())
        .collect::<BTreeSet<_>>();
    assert_eq!(codes.len(), ApiErrorCode::ALL.len());
    assert_eq!(keys.len(), ApiErrorCode::ALL.len());

    for code in ApiErrorCode::ALL {
        let serialized = serde_json::to_string(code).unwrap();
        assert_eq!(serialized, format!("\"{}\"", code.wire_name()));
        assert_eq!(
            serde_json::from_str::<ApiErrorCode>(&serialized).unwrap(),
            *code
        );
    }
    assert!(serde_json::from_str::<ApiErrorCode>(r#""FUTURE_ERROR""#).is_err());
}

#[test]
fn generic_decoder_redacts_parser_detail_and_rejects_trailing_data() {
    let error: ContractDecodeError =
        decode_versioned_envelope_json::<HttpRequestEnvelope<SectionPayload>>(
        br#"{"protocolVersion":1,"payload":{"sectionKey":{"term":"T2026F","campus":"CAMPUS_A","index":"00001"}}} trailing"#,
    ).unwrap_err();
    assert_eq!(error.code(), ApiErrorCode::MalformedRequest);
    assert_eq!(error.to_string(), "malformed typed contract payload");
}

#[test]
fn match_explanation_deserialization_cannot_bypass_invariants() {
    let invalid = [
        r#"{"outcome":"MATCH","reasons":[{"code":"UNKNOWN_VALUE","field":"meeting.location"}]}"#,
        r#"{"outcome":"UNCERTAIN","reasons":[]}"#,
        r#"{"outcome":"UNCERTAIN","reasons":[{"code":"KNOWN_MISMATCH","field":"meeting.location"}]}"#,
        r#"{"outcome":"NO_MATCH","reasons":[{"code":"TBA","field":"meeting.location"}]}"#,
        r#"{"outcome":"FUTURE","reasons":[]}"#,
        r#"{"outcome":"UNCERTAIN","reasons":[{"code":"TBA","field":"../unsafe"}]}"#,
        r#"{"outcome":"UNCERTAIN","reasons":[{"code":"TBA","field":"."}]}"#,
        r#"{"outcome":"MATCH","reasons":[],"ignored":true}"#,
    ];
    for json in invalid {
        assert!(
            serde_json::from_str::<MatchExplanation>(json).is_err(),
            "{json}"
        );
    }
}

#[test]
fn trace_ids_require_canonical_privacy_safe_uuid_v4() {
    let invalid = [
        "00000000-0000-0000-0000-000000000000",
        "f47ac10b-58cc-11cf-a447-001122334455",
        "00000000-0000-4000-0000-000000000001",
        "00000000000040008000000000000001",
        "00000000-0000-4000-8000-00000000000A",
        "{00000000-0000-4000-8000-000000000001}",
        "00000000-0000-4000-8000-000000000001\nforged",
    ];
    for value in invalid {
        assert!(TraceId::from_str(value).is_err(), "{value:?}");
    }

    assert_eq!(
        TraceId::from_str("00000000000040008000000000000001").unwrap_err(),
        TraceIdError::NonCanonical
    );
    assert_eq!(
        TraceId::from_str("00000000-0000-0000-0000-000000000000").unwrap_err(),
        TraceIdError::NotRandomV4
    );
}

struct FixedTraceIdSource {
    value: TraceId,
}

impl TraceIdSource for FixedTraceIdSource {
    fn next_trace_id(&mut self) -> TraceId {
        self.value
    }
}

#[test]
fn trace_id_generation_is_injected_and_deterministic_in_tests() {
    let expected = TraceId::from_str("00000000-0000-4000-8000-000000000001").unwrap();
    let mut source = FixedTraceIdSource { value: expected };
    assert_eq!(source.next_trace_id(), expected);
    assert_eq!(source.next_trace_id(), expected);
}
