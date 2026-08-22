//! Public document-session wire contracts (alert-delivery design v3.1,
//! section 2b): the anonymous validate/renew endpoint the public client
//! calls before every WebSocket connection attempt.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

/// Whether a string is a canonical public document-session nonce: the exact
/// shape the manifest's `session-nonce` scalar freezes (36-character
/// lowercase hyphenated RFC 4122 random UUID v4). The server rejects
/// non-canonical nonces with 400 instead of treating them as renewable.
pub fn is_canonical_session_nonce(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 36 {
        return false;
    }
    for (index, byte) in bytes.iter().enumerate() {
        match index {
            8 | 13 | 18 | 23 => {
                if *byte != b'-' {
                    return false;
                }
            }
            14 => {
                if *byte != b'4' {
                    return false;
                }
            }
            19 => {
                if !matches!(byte, b'8' | b'9' | b'a' | b'b') {
                    return false;
                }
            }
            _ => {
                if !matches!(byte, b'0'..=b'9' | b'a'..=b'f') {
                    return false;
                }
            }
        }
    }
    true
}

/// Request body of `POST /api/v1/session/validate` (public target only).
///
/// The contract is frozen by the approved design: `{ nonce: string,
/// locale?: string }`. `locale` may be OMITTED entirely and follows the same
/// precedence as document issuance -- request body first, `Accept-Language`
/// fallback. The nonce must satisfy [`is_canonical_session_nonce`]; the
/// server enforces it with 400.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SessionValidateRequestV1 {
    pub nonce: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
}

/// Response body of `POST /api/v1/session/validate`.
///
/// EXACTLY one of the two frozen shapes: `{"valid":true}` when the supplied
/// nonce is still registered (its activity window was touched), or
/// `{"renewed":"<nonce>"}` carrying a freshly issued replacement when it was
/// not. Renew-and-evict is atomic inside the registry lock on the server.
/// The decoder enforces the one-of strictly: dual-key objects, unknown
/// fields, and `{"valid":false}` are all rejected.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
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

impl<'de> Deserialize<'de> for SessionValidateResponseV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            #[serde(default)]
            valid: Option<bool>,
            #[serde(default)]
            renewed: Option<String>,
        }

        let wire = Wire::deserialize(deserializer)?;
        match (wire.valid, wire.renewed) {
            (Some(true), None) => Ok(Self::Valid { valid: true }),
            (Some(false), None) => Err(D::Error::custom(
                "a validate response never carries valid=false",
            )),
            (None, Some(renewed)) => {
                if !is_canonical_session_nonce(&renewed) {
                    return Err(D::Error::custom(
                        "renewed must be a canonical session nonce",
                    ));
                }
                Ok(Self::Renewed { renewed })
            }
            (Some(_), Some(_)) | (None, None) => Err(D::Error::custom(
                "a validate response carries exactly one of valid or renewed",
            )),
        }
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
        let nonce = "00000000-0000-4000-8000-000000000001";
        assert_eq!(
            serde_json::to_string(&SessionValidateResponseV1::renewed(nonce.to_owned()))
                .expect("renewed"),
            format!(r#"{{"renewed":"{nonce}"}}"#),
        );
    }

    #[test]
    fn the_response_decoder_enforces_the_exact_one_of() {
        let nonce = "00000000-0000-4000-8000-000000000001";
        assert_eq!(
            serde_json::from_str::<SessionValidateResponseV1>(r#"{"valid":true}"#)
                .expect("valid decodes"),
            SessionValidateResponseV1::valid(),
        );
        assert_eq!(
            serde_json::from_str::<SessionValidateResponseV1>(&format!(
                r#"{{"renewed":"{nonce}"}}"#
            ))
            .expect("renewed decodes"),
            SessionValidateResponseV1::renewed(nonce.to_owned()),
        );
        for rejected in [
            r#"{"valid":false}"#.to_owned(),
            format!(r#"{{"valid":true,"renewed":"{nonce}"}}"#),
            r#"{}"#.to_owned(),
            r#"{"renewed":"not-a-nonce"}"#.to_owned(),
            format!(r#"{{"renewed":"{nonce}","extra":1}}"#),
        ] {
            assert!(
                serde_json::from_str::<SessionValidateResponseV1>(&rejected).is_err(),
                "{rejected}",
            );
        }
    }

    #[test]
    fn canonical_session_nonce_validation_is_exact() {
        assert!(is_canonical_session_nonce(
            "00000000-0000-4000-8000-000000000001"
        ));
        assert!(is_canonical_session_nonce(
            "a1b2c3d4-e5f6-4a7b-9c8d-0123456789ab"
        ));
        for rejected in [
            "",
            "abc",
            "00000000-0000-1000-8000-000000000001",     // not v4
            "00000000-0000-4000-c000-000000000001",     // bad variant
            "00000000-0000-4000-8000-00000000000G",     // non-hex
            "00000000-0000-4000-8000-0000000000012",    // too long
            "00000000000040008000000000000001",         // no hyphens
            "00000000-0000-4000-8000-00000000001",      // too short
            "A0000000-0000-4000-8000-000000000001",     // uppercase
        ] {
            assert!(!is_canonical_session_nonce(rejected), "{rejected}");
        }
    }
}
