use std::collections::BTreeMap;
use std::fmt;

use serde::de::{DeserializeOwned, Error as _, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Number, Value};
use thiserror::Error;

use crate::sha256_hex;

/// Preserves the difference between an absent key, an explicit JSON `null`,
/// and a present value. Empty strings and arrays remain ordinary values.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Presence<T> {
    #[default]
    Missing,
    Null,
    Malformed(MalformedField),
    Value(T),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum JsonType {
    Boolean,
    Number,
    String,
    Array,
    Object,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MalformedField {
    pub json_type: JsonType,
    pub canonical_sha256: String,
}

impl MalformedField {
    fn from_value(value: &Value) -> Self {
        let json_type = match value {
            Value::Bool(_) => JsonType::Boolean,
            Value::Number(_) => JsonType::Number,
            Value::String(_) => JsonType::String,
            Value::Array(_) => JsonType::Array,
            Value::Object(_) => JsonType::Object,
            Value::Null => unreachable!("null has its own Presence variant"),
        };
        let bytes = serde_json::to_vec(value).expect("JSON Value serialization cannot fail");
        Self {
            json_type,
            canonical_sha256: sha256_hex(&bytes),
        }
    }

    fn has_canonical_digest(&self) -> bool {
        self.canonical_sha256.len() == 64
            && self
                .canonical_sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }
}

impl<T> Presence<T> {
    pub const fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }

    pub const fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }

    pub fn as_ref(&self) -> Presence<&T> {
        match self {
            Self::Missing => Presence::Missing,
            Self::Null => Presence::Null,
            Self::Malformed(value) => Presence::Malformed(value.clone()),
            Self::Value(value) => Presence::Value(value),
        }
    }

    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Value(value) => Some(value),
            Self::Missing | Self::Null | Self::Malformed(_) => None,
        }
    }

    pub fn map<U>(self, map: impl FnOnce(T) -> U) -> Presence<U> {
        match self {
            Self::Missing => Presence::Missing,
            Self::Null => Presence::Null,
            Self::Malformed(value) => Presence::Malformed(value),
            Self::Value(value) => Presence::Value(map(value)),
        }
    }
}

impl<'de, T> Deserialize<'de> for Presence<T>
where
    T: DeserializeOwned,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        if value.is_null() {
            return Ok(Self::Null);
        }
        if let Value::Object(object) = &value
            && let Some(envelope) = object.get("$malformed")
        {
            if object.len() != 1 {
                return Err(D::Error::custom(
                    "malformed-field envelope must contain exactly one $malformed member",
                ));
            }
            let malformed: MalformedField = serde_json::from_value(envelope.clone())
                .map_err(|error| D::Error::custom(error.to_string()))?;
            if !malformed.has_canonical_digest() {
                return Err(D::Error::custom(
                    "malformed-field envelope digest must be lowercase SHA-256",
                ));
            }
            return Ok(Self::Malformed(malformed));
        }
        Ok(match serde_json::from_value(value.clone()) {
            Ok(value) => Self::Value(value),
            Err(_) => Self::Malformed(MalformedField::from_value(&value)),
        })
    }
}

impl<T> Serialize for Presence<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Missing | Self::Null => serializer.serialize_none(),
            Self::Malformed(value) => {
                #[derive(Serialize)]
                struct MalformedEnvelope<'a> {
                    #[serde(rename = "$malformed")]
                    value: &'a MalformedField,
                }
                MalformedEnvelope { value }.serialize(serializer)
            }
            Self::Value(value) => value.serialize(serializer),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawCatalogCourse {
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub campus_code: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub campus_locations: Presence<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub core_codes: Presence<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub course_description: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub course_fee: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub course_fee_descr: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub course_notes: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub course_number: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub course_string: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub credits: Presence<Number>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub credits_object: Presence<BTreeMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub expanded_title: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub level: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub main_campus: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub offering_unit_code: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub offering_unit_title: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub open_sections: Presence<i64>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub pre_req_notes: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub school: Presence<BTreeMap<String, Value>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub sections: Presence<Vec<RawCatalogSection>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub subject: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub subject_description: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub subject_group_notes: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub subject_notes: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub supplement_code: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub synopsis_url: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub title: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub unit_notes: Presence<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawCatalogSection {
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub campus_code: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub comments: Presence<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub comments_text: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub cross_listed_sections: Presence<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub cross_listed_sections_text: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub cross_listed_section_type: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub exam_code: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub exam_code_text: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub final_exam: Presence<Value>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub honor_programs: Presence<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub index: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub instructors: Presence<Vec<RawInstructor>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub instructors_text: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub legend_key: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub majors: Presence<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub meeting_times: Presence<Vec<RawMeeting>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub minors: Presence<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub number: Presence<String>,
    /// Rutgers Catalog snapshot only. It is never a live Open observation.
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub open_status: Presence<bool>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub open_status_text: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub open_to_text: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub printed: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub section_campus_locations: Presence<Vec<Value>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub section_course_type: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub section_eligibility: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub section_notes: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub session_date_print_indicator: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub session_dates: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub special_permission_add_code: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub special_permission_add_code_description: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub special_permission_drop_code: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub special_permission_drop_code_description: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub subtitle: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub subtopic: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub unit_majors: Presence<Vec<Value>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawMeeting {
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub ba_class_hours: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub building_code: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub campus_abbrev: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub campus_location: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub campus_name: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub end_time: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub end_time_military: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub meeting_day: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub meeting_mode_code: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub meeting_mode_desc: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub pm_code: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub room_number: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub start_time: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub start_time_military: Presence<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RawInstructor {
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub name: Presence<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceProvenance {
    pub request_id: String,
    pub observed_at_utc: String,
    pub body_sha256: String,
    pub decoded_bytes: usize,
}

impl SourceProvenance {
    pub fn from_body(
        request_id: impl Into<String>,
        observed_at_utc: impl Into<String>,
        body: &[u8],
    ) -> Self {
        Self {
            request_id: request_id.into(),
            observed_at_utc: observed_at_utc.into(),
            body_sha256: sha256_hex(body),
            decoded_bytes: body.len(),
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CatalogDecodeError {
    #[error("Catalog response is not valid typed JSON: {0}")]
    InvalidJson(String),
}

pub fn decode_catalog_payload(body: &[u8]) -> Result<Vec<RawCatalogCourse>, CatalogDecodeError> {
    let decoded: Vec<RawCatalogCourse> = serde_json::from_slice(body)
        .map_err(|error| CatalogDecodeError::InvalidJson(error.to_string()))?;
    let value = decode_strict_json(body)
        .map_err(|error| CatalogDecodeError::InvalidJson(error.to_string()))?;
    reject_reserved_malformed_envelopes(&value)
        .map_err(|error| CatalogDecodeError::InvalidJson(error.to_owned()))?;
    Ok(decoded)
}

struct StrictJson(Value);

impl<'de> Deserialize<'de> for StrictJson {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(StrictJsonVisitor)
    }
}

struct StrictJsonVisitor;

impl<'de> Visitor<'de> for StrictJsonVisitor {
    type Value = StrictJson;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("JSON without duplicate object members")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E> {
        Ok(StrictJson(Value::Bool(value)))
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E> {
        Ok(StrictJson(Value::Number(value.into())))
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E> {
        Ok(StrictJson(Value::Number(value.into())))
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .map(StrictJson)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E> {
        Ok(StrictJson(Value::String(value.to_owned())))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E> {
        Ok(StrictJson(Value::String(value)))
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(StrictJson(Value::Null))
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(StrictJson(Value::Null))
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        StrictJson::deserialize(deserializer)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(StrictJson(value)) = sequence.next_element()? {
            values.push(value);
        }
        Ok(StrictJson(Value::Array(values)))
    }

    fn visit_map<A>(self, mut object: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = serde_json::Map::new();
        while let Some((key, StrictJson(value))) = object.next_entry::<String, StrictJson>()? {
            if values.insert(key.clone(), value).is_some() {
                return Err(A::Error::custom(format!(
                    "duplicate JSON object member: {key}"
                )));
            }
        }
        Ok(StrictJson(Value::Object(values)))
    }
}

pub(crate) fn decode_strict_json(body: &[u8]) -> Result<Value, serde_json::Error> {
    serde_json::from_slice::<StrictJson>(body).map(|value| value.0)
}

pub(crate) fn reject_reserved_malformed_envelopes(value: &Value) -> Result<(), &'static str> {
    match value {
        Value::Array(values) => {
            for value in values {
                reject_reserved_malformed_envelopes(value)?;
            }
        }
        Value::Object(values) => {
            if values.contains_key("$malformed") {
                return Err("upstream payload contains a reserved internal malformed envelope");
            }
            for value in values.values() {
                reject_reserved_malformed_envelopes(value)?;
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{JsonType, Presence, RawCatalogCourse, RawMeeting, decode_catalog_payload};

    #[test]
    fn presence_keeps_missing_null_and_empty_value_distinct() {
        let courses = decode_catalog_payload(
            br#"[
              {"courseString":"SYN:CAT:001","title":null,"courseNotes":"","coreCodes":[],"sections":[]},
              {"courseString":"SYN:CAT:002","sections":[]}
            ]"#,
        )
        .expect("synthetic payload should decode");

        assert_eq!(courses[0].title, Presence::Null);
        assert_eq!(courses[0].course_notes, Presence::Value(String::new()));
        assert_eq!(courses[0].core_codes, Presence::Value(Vec::new()));
        assert_eq!(courses[1].title, Presence::Missing);
    }

    #[test]
    fn serialization_omits_missing_but_retains_null_and_empty() {
        let course: RawCatalogCourse = serde_json::from_str(
            r#"{"courseString":"SYN:CAT:001","title":null,"courseNotes":"","sections":[]}"#,
        )
        .expect("synthetic course should decode");
        let value = serde_json::to_value(course).expect("typed DTO should serialize");
        let object = value
            .as_object()
            .expect("serialized course should be object");

        assert!(!object.contains_key("subject"));
        assert!(object.get("title").is_some_and(serde_json::Value::is_null));
        assert_eq!(
            object.get("courseNotes").and_then(|value| value.as_str()),
            Some("")
        );
    }

    #[test]
    fn root_and_field_types_are_checked() {
        assert!(decode_catalog_payload(br#"{"courseString":"x"}"#).is_err());
        let malformed = decode_catalog_payload(br#"[{"sections":"not-an-array"}]"#)
            .expect("field-level malformed values must survive typed decode");
        assert!(matches!(
            &malformed[0].sections,
            Presence::Malformed(value) if value.json_type == JsonType::String
        ));
    }

    #[test]
    fn real_shape_comment_objects_decode_as_structured_values() {
        let courses = decode_catalog_payload(
            br#"[{
              "courseString":"SYN:CAT:001",
              "sections":[{
                "index":"10001",
                "comments":[{"code":"SYN_CODE","description":"SYNTHETIC COMMENT"}]
              }]
            }]"#,
        )
        .expect("comment objects should be accepted as structured upstream values");
        let Presence::Value(sections) = &courses[0].sections else {
            panic!("sections should be present");
        };
        assert!(matches!(
            &sections[0].comments,
            Presence::Value(values)
                if values == &vec![json!({"code":"SYN_CODE","description":"SYNTHETIC COMMENT"})]
        ));
    }

    #[test]
    fn filter_fields_preserve_five_presence_states_without_raw_malformed_text() {
        let courses = decode_catalog_payload(
            br#"[
              {"courseString":"SYN:CAT:101","sections":[]},
              {"courseString":"SYN:CAT:102","coreCodes":null,"preReqNotes":null,"sections":[]},
              {"courseString":"SYN:CAT:103","coreCodes":[],"preReqNotes":"","sections":[]},
              {"courseString":"SYN:CAT:104","coreCodes":"MALFORMED_SYNTHETIC","preReqNotes":[],"sections":[]},
              {"courseString":"SYN:CAT:105","coreCodes":["SYN_CORE"],"preReqNotes":"SYNTHETIC_REQUIREMENT","sections":[]}
            ]"#,
        )
        .expect("field-level malformed values should not reject the root");

        assert_eq!(courses[0].core_codes, Presence::Missing);
        assert_eq!(courses[1].core_codes, Presence::Null);
        assert_eq!(courses[2].core_codes, Presence::Value(Vec::new()));
        assert!(matches!(
            &courses[3].core_codes,
            Presence::Malformed(value) if value.json_type == JsonType::String && value.canonical_sha256.len() == 64
        ));
        assert!(matches!(&courses[4].core_codes, Presence::Value(values) if values.len() == 1));
        assert!(matches!(
            &courses[3].pre_req_notes,
            Presence::Malformed(value) if value.json_type == JsonType::Array
        ));
    }

    #[test]
    fn malformed_presence_envelopes_round_trip_without_digest_drift() {
        let course: RawCatalogCourse = serde_json::from_value(json!({
            "courseString":"SYN:CAT:901",
            "coreCodes":"MALFORMED_SYNTHETIC",
            "preReqNotes":7,
            "sections":[]
        }))
        .unwrap();
        let course_round_trip: RawCatalogCourse =
            serde_json::from_value(serde_json::to_value(&course).unwrap()).unwrap();
        assert_eq!(course_round_trip, course);

        let meeting: RawMeeting = serde_json::from_value(json!({
            "meetingDay":7,
            "meetingModeCode":["MALFORMED_SYNTHETIC"],
            "startTimeMilitary":{"unexpected":true}
        }))
        .unwrap();
        let meeting_round_trip: RawMeeting =
            serde_json::from_value(serde_json::to_value(&meeting).unwrap()).unwrap();
        assert_eq!(meeting_round_trip, meeting);
    }

    #[test]
    fn malformed_presence_envelopes_reject_noncanonical_or_ambiguous_forms() {
        for value in [
            json!({"$malformed":{"jsonType":"STRING","canonicalSha256":"0"}}),
            json!({"$malformed":{"jsonType":"STRING","canonicalSha256":"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"}}),
            json!({"$malformed":{"jsonType":"STRING","canonicalSha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","extra":true}}),
            json!({"$malformed":{"jsonType":"STRING","canonicalSha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},"extra":true}),
        ] {
            assert!(serde_json::from_value::<Presence<String>>(value).is_err());
        }
    }

    #[test]
    fn upstream_payload_cannot_forge_an_internal_malformed_envelope() {
        let forged = br#"[{
          "courseString":"SYN:CAT:902",
          "preReqNotes":{"$malformed":{"jsonType":"NUMBER","canonicalSha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}},
          "sections":[]
        }]"#;
        assert!(decode_catalog_payload(forged).is_err());
    }

    #[test]
    fn upstream_catalog_duplicate_known_fields_fail_before_value_scanning() {
        for body in [
            br#"[{"courseString":"SYN:CAT:903","courseString":"SYN:CAT:904","sections":[]}]"#
                .as_slice(),
            br#"[{"courseString":"SYN:CAT:903","sections":[{"index":"90001"}],"sections":[]}]"#
                .as_slice(),
        ] {
            assert!(decode_catalog_payload(body).is_err());
        }
    }
}
