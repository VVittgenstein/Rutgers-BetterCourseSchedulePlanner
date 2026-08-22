//! Public document-session wire contracts (alert-delivery design v3.1,
//! section 2b): the anonymous validate/renew endpoint the public client
//! calls before every WebSocket connection attempt.

use serde::{Deserialize, Serialize};

/// Request body of `POST /api/v1/session/validate` (public target only).
///
/// The contract is frozen by the approved design: `{ nonce: string,
/// locale?: string }`. `locale` follows the same precedence as document
/// issuance -- request body first, `Accept-Language` fallback.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionValidateRequestV1 {
    pub nonce: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

/// Response body of `POST /api/v1/session/validate`.
///
/// Exactly one of the two frozen shapes: `{"valid":true}` when the supplied
/// nonce is still registered (its activity window was touched), or
/// `{"renewed":"<nonce>"}` carrying a freshly issued replacement when it was
/// not. Renew-and-evict is atomic inside the registry lock on the server.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum SessionValidateResponseV1 {
    Valid { valid: bool },
    Renewed { renewed: String },
}

impl SessionValidateResponseV1 {
    pub fn valid() -> Self {
        Self::Valid { valid: true }
    }

    pub fn renewed(nonce: String) -> Self {
        Self::Renewed { renewed: nonce }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_wire_shapes_are_frozen() {
        let request: SessionValidateRequestV1 =
            serde_json::from_str(r#"{"nonce":"abc"}"#).expect("minimal request");
        assert_eq!(request.nonce, "abc");
        assert_eq!(request.locale, None);
        let request: SessionValidateRequestV1 =
            serde_json::from_str(r#"{"nonce":"abc","locale":"zh-CN"}"#).expect("full request");
        assert_eq!(request.locale.as_deref(), Some("zh-CN"));
        assert!(
            serde_json::from_str::<SessionValidateRequestV1>(r#"{"nonce":"abc","extra":1}"#)
                .is_err(),
            "unknown request fields are rejected",
        );

        assert_eq!(
            serde_json::to_string(&SessionValidateResponseV1::valid()).expect("valid"),
            r#"{"valid":true}"#,
        );
        assert_eq!(
            serde_json::to_string(&SessionValidateResponseV1::renewed("n-1".to_owned()))
                .expect("renewed"),
            r#"{"renewed":"n-1"}"#,
        );
    }
}
