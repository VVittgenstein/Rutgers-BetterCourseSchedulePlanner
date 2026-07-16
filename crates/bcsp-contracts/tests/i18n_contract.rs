use std::collections::BTreeSet;

use bcsp_contracts::{ApiErrorCode, MatchReasonCode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct I18nContract {
    canonical_locales: Vec<String>,
    fallback: String,
    api_errors: Vec<MessagePair>,
    match_reasons: Vec<MessagePair>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MessagePair {
    wire: String,
    message_key: String,
}

fn expected_contract() -> I18nContract {
    I18nContract {
        canonical_locales: vec!["en-US".to_owned(), "zh-CN".to_owned()],
        fallback: "en-US".to_owned(),
        api_errors: ApiErrorCode::ALL
            .iter()
            .map(|code| MessagePair {
                wire: code.wire_name().to_owned(),
                message_key: code.message_key().to_owned(),
            })
            .collect(),
        match_reasons: MatchReasonCode::ALL
            .iter()
            .map(|reason| MessagePair {
                wire: reason.wire_name().to_owned(),
                message_key: reason.message_key().to_owned(),
            })
            .collect(),
    }
}

fn pretty<T: Serialize>(value: &T) -> String {
    format!("{}\n", serde_json::to_string_pretty(value).unwrap())
}

fn canonical_golden(expected: &str) -> String {
    expected.replace("\r\n", "\n")
}

fn assert_unique_pairs(label: &str, pairs: &[MessagePair]) {
    let wires = pairs
        .iter()
        .map(|pair| pair.wire.as_str())
        .collect::<BTreeSet<_>>();
    let message_keys = pairs
        .iter()
        .map(|pair| pair.message_key.as_str())
        .collect::<BTreeSet<_>>();

    assert_eq!(wires.len(), pairs.len(), "duplicate {label} wire name");
    assert_eq!(
        message_keys.len(),
        pairs.len(),
        "duplicate {label} message key"
    );
}

#[test]
fn i18n_contract_golden_locks_locales_error_keys_and_match_reason_keys() {
    let golden = include_str!("golden/i18n-contract-v1.json");
    let expected = expected_contract();

    assert_eq!(expected.canonical_locales, ["en-US", "zh-CN"]);
    assert_eq!(expected.fallback, "en-US");
    assert_eq!(expected.api_errors.len(), 14);
    assert_eq!(expected.match_reasons.len(), 7);
    assert_unique_pairs("API error", &expected.api_errors);
    assert_unique_pairs("match reason", &expected.match_reasons);

    assert_eq!(pretty(&expected), canonical_golden(golden));
    assert_eq!(
        serde_json::from_str::<I18nContract>(golden).unwrap(),
        expected
    );
}
