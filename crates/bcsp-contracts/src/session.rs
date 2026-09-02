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

/// A session nonce proven canonical at construction: the only way to put a
/// `renewed` value on the wire, so a non-canonical nonce is unrepresentable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalSessionNonce(String);

impl CanonicalSessionNonce {
    pub fn try_new(value: String) -> Option<Self> {
        is_canonical_session_nonce(&value).then_some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Response body of `POST /api/v1/session/validate`.
///
/// EXACTLY one of the two frozen shapes: `{"valid":true}` when the supplied
/// nonce is still registered (its activity window was touched), or
/// `{"renewed":"<nonce>"}` carrying a freshly issued replacement when it was
/// not. Renew-and-evict is atomic inside the registry lock on the server.
///
/// Illegal states are unrepresentable: the payload is a private one-of whose
/// only constructors are [`Self::valid`] (literal `true`, no boolean field
/// exists to set) and [`Self::renewed`] (requires a [`CanonicalSessionNonce`]).
/// The decoder is a strict visitor: unknown or duplicate keys, dual-key
/// objects, explicit `null` values, `valid:false`, and non-canonical
/// `renewed` nonces are all rejected -- a `null` never masquerades as an
/// omitted field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionValidateResponseV1 {
    kind: ValidateResponseKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ValidateResponseKind {
    Valid,
    Renewed(CanonicalSessionNonce),
}

impl SessionValidateResponseV1 {
    pub fn valid() -> Self {
        Self {
            kind: ValidateResponseKind::Valid,
        }
    }

    pub fn renewed(nonce: CanonicalSessionNonce) -> Self {
        Self {
            kind: ValidateResponseKind::Renewed(nonce),
        }
    }

    pub fn is_valid(&self) -> bool {
        matches!(self.kind, ValidateResponseKind::Valid)
    }

    pub fn as_renewed(&self) -> Option<&str> {
        match &self.kind {
            ValidateResponseKind::Valid => None,
            ValidateResponseKind::Renewed(nonce) => Some(nonce.as_str()),
        }
    }
}

impl Serialize for SessionValidateResponseV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap as _;
        let mut map = serializer.serialize_map(Some(1))?;
        match &self.kind {
            ValidateResponseKind::Valid => map.serialize_entry("valid", &true)?,
            ValidateResponseKind::Renewed(nonce) => {
                map.serialize_entry("renewed", nonce.as_str())?;
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for SessionValidateResponseV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ResponseVisitor;

        impl<'de> serde::de::Visitor<'de> for ResponseVisitor {
            type Value = SessionValidateResponseV1;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("exactly one of {\"valid\":true} or {\"renewed\":\"<nonce>\"}")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut outcome: Option<ValidateResponseKind> = None;
                while let Some(key) = map.next_key::<String>()? {
                    if outcome.is_some() {
                        return Err(A::Error::custom(
                            "a validate response carries exactly one field",
                        ));
                    }
                    match key.as_str() {
                        "valid" => {
                            // bool, not Option<bool>: an explicit null fails
                            // here instead of imitating an omitted field.
                            let value: bool = map.next_value()?;
                            if !value {
                                return Err(A::Error::custom(
                                    "a validate response never carries valid=false",
                                ));
                            }
                            outcome = Some(ValidateResponseKind::Valid);
                        }
                        "renewed" => {
                            let value: String = map.next_value()?;
                            let nonce = CanonicalSessionNonce::try_new(value).ok_or_else(|| {
                                A::Error::custom("renewed must be a canonical session nonce")
                            })?;
                            outcome = Some(ValidateResponseKind::Renewed(nonce));
                        }
                        unknown => {
                            return Err(A::Error::unknown_field(unknown, &["valid", "renewed"]));
                        }
                    }
                }
                match outcome {
                    Some(kind) => Ok(SessionValidateResponseV1 { kind }),
                    None => Err(A::Error::custom(
                        "a validate response carries exactly one of valid or renewed",
                    )),
                }
            }
        }

        deserializer.deserialize_map(ResponseVisitor)
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
        let canonical = CanonicalSessionNonce::try_new(nonce.to_owned()).expect("canonical");
        assert_eq!(
            serde_json::to_string(&SessionValidateResponseV1::renewed(canonical)).expect("renewed"),
            format!(r#"{{"renewed":"{nonce}"}}"#),
        );
        assert!(
            CanonicalSessionNonce::try_new("not-a-nonce".to_owned()).is_none(),
            "a non-canonical renewed nonce is unrepresentable",
        );
    }

    #[test]
    fn the_response_decoder_enforces_the_exact_one_of() {
        let nonce = "00000000-0000-4000-8000-000000000001";
        let valid = serde_json::from_str::<SessionValidateResponseV1>(r#"{"valid":true}"#)
            .expect("valid decodes");
        assert!(valid.is_valid());
        assert_eq!(valid.as_renewed(), None);
        let renewed = serde_json::from_str::<SessionValidateResponseV1>(&format!(
            r#"{{"renewed":"{nonce}"}}"#
        ))
        .expect("renewed decodes");
        assert_eq!(renewed.as_renewed(), Some(nonce));
        for rejected in [
            r#"{"valid":false}"#.to_owned(),
            format!(r#"{{"valid":true,"renewed":"{nonce}"}}"#),
            // Explicit null is NOT an omitted field: neither half of a
            // dual-key object may hide behind null (reviewer P1).
            r#"{"valid":true,"renewed":null}"#.to_owned(),
            format!(r#"{{"valid":null,"renewed":"{nonce}"}}"#),
            r#"{"valid":null}"#.to_owned(),
            r#"{"renewed":null}"#.to_owned(),
            r#"{}"#.to_owned(),
            r#"{"renewed":"not-a-nonce"}"#.to_owned(),
            format!(r#"{{"renewed":"{nonce}","extra":1}}"#),
            format!(r#"{{"renewed":"{nonce}","renewed":"{nonce}"}}"#),
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
            "00000000-0000-1000-8000-000000000001",  // not v4
            "00000000-0000-4000-c000-000000000001",  // bad variant
            "00000000-0000-4000-8000-00000000000G",  // non-hex
            "00000000-0000-4000-8000-0000000000012", // too long
            "00000000000040008000000000000001",      // no hyphens
            "00000000-0000-4000-8000-00000000001",   // too short
            "A0000000-0000-4000-8000-000000000001",  // uppercase
        ] {
            assert!(!is_canonical_session_nonce(rejected), "{rejected}");
        }
    }
}
