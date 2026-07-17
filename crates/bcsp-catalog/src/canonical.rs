use serde_json::{Map, Value};

use crate::model::Sha256Hex;

const RAW_COURSE_PREIMAGE_V1: &[u8] = b"bcsp-catalog/raw-course/v1\0";
const RAW_SECTION_PREIMAGE_V1: &[u8] = b"bcsp-catalog/raw-section/v1\0";
const SECTION_SEMANTIC_PREIMAGE_V1: &[u8] = b"bcsp-catalog/section-semantic/v1\0";
const OCCURRENCE_PREIMAGE_V1: &[u8] = b"bcsp-catalog/occurrence/v1\0";
const COURSE_VARIANT_PREIMAGE_V1: &[u8] = b"bcsp-catalog/course-variant/v1\0";
const TARGET_CONTENT_PREIMAGE_V1: &[u8] = b"bcsp-catalog/target-content/v1\0";

#[derive(Clone, Copy)]
enum CanonicalProfile {
    Exact,
    CourseSemantic,
    SectionSemantic,
}

pub(crate) fn canonical_raw_course_hash_v1(value: &Value) -> Result<Sha256Hex, serde_json::Error> {
    let value = canonicalize(value, CanonicalProfile::Exact, &mut Vec::new());
    hash_tagged(RAW_COURSE_PREIMAGE_V1, &value)
}

pub(crate) fn canonical_raw_section_hash_v1(value: &Value) -> Result<Sha256Hex, serde_json::Error> {
    let value = canonicalize(value, CanonicalProfile::Exact, &mut Vec::new());
    hash_tagged(RAW_SECTION_PREIMAGE_V1, &value)
}

pub(crate) fn canonical_section_semantic_hash_v1(
    value: &Value,
) -> Result<Sha256Hex, serde_json::Error> {
    let value = canonical_section_semantic_value_v1(value);
    hash_tagged(SECTION_SEMANTIC_PREIMAGE_V1, &value)
}

pub(crate) fn canonical_section_semantic_value_v1(value: &Value) -> Value {
    canonicalize(value, CanonicalProfile::SectionSemantic, &mut Vec::new())
}

pub(crate) fn canonical_occurrence_hash_v1(value: &Value) -> Result<Sha256Hex, serde_json::Error> {
    let value = canonicalize(value, CanonicalProfile::Exact, &mut Vec::new());
    hash_tagged(OCCURRENCE_PREIMAGE_V1, &value)
}

pub(crate) fn canonical_course_semantic_value_v1(value: &Value) -> Value {
    canonicalize(value, CanonicalProfile::CourseSemantic, &mut Vec::new())
}

pub(crate) fn canonical_course_variant_hash_v1(
    value: &Value,
) -> Result<Sha256Hex, serde_json::Error> {
    let canonical = canonicalize(value, CanonicalProfile::Exact, &mut Vec::new());
    hash_tagged(COURSE_VARIANT_PREIMAGE_V1, &canonical)
}

pub(crate) fn canonical_target_content_hash_v1(
    value: &Value,
) -> Result<Sha256Hex, serde_json::Error> {
    let canonical = canonicalize(value, CanonicalProfile::Exact, &mut Vec::new());
    hash_tagged(TARGET_CONTENT_PREIMAGE_V1, &canonical)
}

fn hash_tagged(prefix: &[u8], value: &Value) -> Result<Sha256Hex, serde_json::Error> {
    let value = serde_json::to_vec(value)?;
    let mut preimage = Vec::with_capacity(prefix.len() + value.len());
    preimage.extend_from_slice(prefix);
    preimage.extend_from_slice(&value);
    Ok(Sha256Hex::from_digest_bytes(&preimage))
}

fn canonicalize(value: &Value, profile: CanonicalProfile, path: &mut Vec<String>) -> Value {
    match value {
        Value::Object(object) => {
            let mut result = Map::new();
            let mut entries = object.iter().collect::<Vec<_>>();
            entries.sort_by_key(|(left, _)| *left);
            for (key, value) in entries {
                if matches!(profile, CanonicalProfile::SectionSemantic)
                    && path.is_empty()
                    && derived_text_has_trustworthy_structured_source(object, key)
                {
                    continue;
                }
                path.push(key.clone());
                result.insert(key.clone(), canonicalize(value, profile, path));
                path.pop();
            }
            Value::Object(result)
        }
        Value::Array(values) => {
            let mut values = values
                .iter()
                .map(|value| canonicalize(value, profile, path))
                .collect::<Vec<_>>();
            if is_known_set_array(profile, path) {
                values.sort_by_key(|value| {
                    serde_json::to_string(value).expect("JSON Value serialization cannot fail")
                });
            }
            Value::Array(values)
        }
        Value::String(value) if is_known_semantic_string(profile, path) => {
            Value::String(value.trim().to_owned())
        }
        _ => value.clone(),
    }
}

fn derived_text_has_trustworthy_structured_source(object: &Map<String, Value>, key: &str) -> bool {
    let structured = match key {
        "commentsText" => "comments",
        "instructorsText" => "instructors",
        "crossListedSectionsText" => "crossListedSections",
        _ => return false,
    };
    let Some(Value::String(derived_text)) = object.get(key) else {
        return false;
    };
    let Some(Value::Array(values)) = object.get(structured) else {
        return false;
    };
    if values.is_empty() {
        return derived_text.trim().is_empty();
    }
    match structured {
        "comments" => values.iter().all(valid_comment_entry),
        "instructors" => values.iter().all(|value| {
            value
                .as_object()
                .and_then(|object| object.get("name"))
                .and_then(Value::as_str)
                .is_some_and(|name| !name.trim().is_empty())
        }),
        "crossListedSections" => values.iter().all(valid_cross_list_entry),
        _ => false,
    }
}

fn valid_cross_list_entry(value: &Value) -> bool {
    value.as_str().is_some_and(|code| !code.trim().is_empty())
        || value
            .as_object()
            .and_then(|object| object.get("code"))
            .and_then(Value::as_str)
            .is_some_and(|code| !code.trim().is_empty())
}

fn is_known_set_array(profile: CanonicalProfile, path: &[String]) -> bool {
    let Some(field) = path.last().map(String::as_str) else {
        return false;
    };
    match profile {
        CanonicalProfile::Exact => false,
        CanonicalProfile::CourseSemantic => {
            path.len() == 1 && matches!(field, "campusLocations" | "coreCodes")
        }
        CanonicalProfile::SectionSemantic => {
            path.len() == 1
                && matches!(
                    field,
                    "comments"
                        | "crossListedSections"
                        | "honorPrograms"
                        | "instructors"
                        | "majors"
                        | "minors"
                        | "sectionCampusLocations"
                        | "unitMajors"
                )
        }
    }
}

fn is_known_semantic_string(profile: CanonicalProfile, path: &[String]) -> bool {
    let Some(field) = path.last().map(String::as_str) else {
        return false;
    };
    match profile {
        CanonicalProfile::Exact => false,
        CanonicalProfile::CourseSemantic => {
            (path.len() == 1
                && matches!(
                    field,
                    "campusCode"
                        | "courseDescription"
                        | "courseFee"
                        | "courseFeeDescr"
                        | "courseNotes"
                        | "courseNumber"
                        | "courseString"
                        | "expandedTitle"
                        | "level"
                        | "mainCampus"
                        | "offeringUnitCode"
                        | "offeringUnitTitle"
                        | "preReqNotes"
                        | "subject"
                        | "subjectDescription"
                        | "subjectGroupNotes"
                        | "subjectNotes"
                        | "supplementCode"
                        | "synopsisUrl"
                        | "title"
                        | "unitNotes"
                ))
                || matches_known_nested_code(path, &["campusLocations", "creditsObject", "school"])
                || path == ["coreCodes"]
        }
        CanonicalProfile::SectionSemantic => {
            (path.len() == 1
                && matches!(
                    field,
                    "campusCode"
                        | "commentsText"
                        | "crossListedSectionsText"
                        | "crossListedSectionType"
                        | "examCode"
                        | "examCodeText"
                        | "index"
                        | "instructorsText"
                        | "legendKey"
                        | "number"
                        | "openStatusText"
                        | "openToText"
                        | "printed"
                        | "sectionCourseType"
                        | "sectionEligibility"
                        | "sectionNotes"
                        | "sessionDatePrintIndicator"
                        | "sessionDates"
                        | "specialPermissionAddCode"
                        | "specialPermissionAddCodeDescription"
                        | "specialPermissionDropCode"
                        | "specialPermissionDropCodeDescription"
                        | "subtitle"
                        | "subtopic"
                ))
                || path == ["comments"]
                || (path.len() == 2
                    && path[0] == "comments"
                    && matches!(path[1].as_str(), "code" | "description"))
                || path == ["instructors", "name"]
                || (path.len() == 2
                    && path[0] == "unitMajors"
                    && matches!(path[1].as_str(), "unitCode" | "majorCode"))
                || path == ["sectionCampusLocations", "description"]
                || matches_known_nested_code(
                    path,
                    &[
                        "crossListedSections",
                        "honorPrograms",
                        "majors",
                        "minors",
                        "sectionCampusLocations",
                        "unitMajors",
                    ],
                )
        }
    }
}

fn matches_known_nested_code(path: &[String], parents: &[&str]) -> bool {
    path.len() == 2 && path[1] == "code" && parents.contains(&path[0].as_str())
}

fn valid_comment_entry(value: &Value) -> bool {
    if value
        .as_str()
        .is_some_and(|comment| !comment.trim().is_empty())
    {
        return true;
    }
    let Some(object) = value.as_object() else {
        return false;
    };
    ["code", "description"].iter().any(|field| {
        object
            .get(*field)
            .and_then(Value::as_str)
            .is_some_and(|text| !text.trim().is_empty())
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        canonical_course_semantic_value_v1, canonical_course_variant_hash_v1,
        canonical_occurrence_hash_v1, canonical_raw_course_hash_v1, canonical_raw_section_hash_v1,
        canonical_section_semantic_hash_v1, canonical_target_content_hash_v1,
    };

    #[test]
    fn semantic_hash_ignores_array_order_and_derived_text() {
        let left = json!({"comments":[" A ","B"],"commentsText":"A B"});
        let right = json!({"comments":["B","A"],"commentsText":"B A"});
        assert_eq!(
            canonical_section_semantic_hash_v1(&left).expect("hash should serialize"),
            canonical_section_semantic_hash_v1(&right).expect("hash should serialize")
        );
    }

    #[test]
    fn semantic_hash_handles_real_shape_comment_objects_as_a_set() {
        let left = json!({
            "comments":[
                {"code":" A ","description":" First "},
                {"code":"B","description":"Second"}
            ],
            "commentsText":"First Second"
        });
        let right = json!({
            "comments":[
                {"code":"B","description":"Second"},
                {"code":"A","description":"First"}
            ],
            "commentsText":"Second First"
        });
        assert_eq!(
            canonical_section_semantic_hash_v1(&left).unwrap(),
            canonical_section_semantic_hash_v1(&right).unwrap()
        );
    }

    #[test]
    fn derived_text_is_retained_without_a_trustworthy_structured_array() {
        for (left, right) in [
            (
                json!({"commentsText":"SYNTHETIC_A"}),
                json!({"commentsText":"SYNTHETIC_B"}),
            ),
            (
                json!({"comments":null,"commentsText":"SYNTHETIC_A"}),
                json!({"comments":null,"commentsText":"SYNTHETIC_B"}),
            ),
            (
                json!({"comments":"MALFORMED_SYNTHETIC","commentsText":"SYNTHETIC_A"}),
                json!({"comments":"MALFORMED_SYNTHETIC","commentsText":"SYNTHETIC_B"}),
            ),
            (
                json!({"comments":[],"commentsText":"SYNTHETIC_A"}),
                json!({"comments":[],"commentsText":"SYNTHETIC_B"}),
            ),
            (
                json!({"comments":[""],"commentsText":"SYNTHETIC_A"}),
                json!({"comments":[""],"commentsText":"SYNTHETIC_B"}),
            ),
            (
                json!({"instructors":[{"name":{"$malformed":{"jsonType":"NUMBER","canonicalSha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}}],"instructorsText":"SYNTHETIC_A"}),
                json!({"instructors":[{"name":{"$malformed":{"jsonType":"NUMBER","canonicalSha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}}}],"instructorsText":"SYNTHETIC_B"}),
            ),
            (
                json!({"crossListedSections":[{}],"crossListedSectionsText":"SYNTHETIC_A"}),
                json!({"crossListedSections":[{}],"crossListedSectionsText":"SYNTHETIC_B"}),
            ),
        ] {
            assert_ne!(
                canonical_section_semantic_hash_v1(&left).unwrap(),
                canonical_section_semantic_hash_v1(&right).unwrap()
            );
        }
    }

    #[test]
    fn unknown_nested_field_name_collisions_preserve_order_and_whitespace() {
        let section_order_left = json!({"future":{"comments":["B","A"]}});
        let section_order_right = json!({"future":{"comments":["A","B"]}});
        assert_ne!(
            canonical_section_semantic_hash_v1(&section_order_left).unwrap(),
            canonical_section_semantic_hash_v1(&section_order_right).unwrap()
        );

        let section_space_left = json!({"future":{"title":" SYNTHETIC "}});
        let section_space_right = json!({"future":{"title":"SYNTHETIC"}});
        assert_ne!(
            canonical_section_semantic_hash_v1(&section_space_left).unwrap(),
            canonical_section_semantic_hash_v1(&section_space_right).unwrap()
        );

        let course_left = json!({"future":{"coreCodes":["B","A"],"title":" SYNTHETIC "}});
        let course_right = json!({"future":{"coreCodes":["A","B"],"title":"SYNTHETIC"}});
        assert_ne!(
            canonical_course_semantic_value_v1(&course_left),
            canonical_course_semantic_value_v1(&course_right)
        );
    }

    #[test]
    fn meeting_order_and_raw_tuple_whitespace_are_semantic_content() {
        let ordered = json!({
            "meetingTimes": [
                {"meetingModeCode":"92","meetingDay":"M"},
                {"meetingModeCode":"93","meetingDay":"T"}
            ]
        });
        let reordered = json!({
            "meetingTimes": [
                {"meetingModeCode":"93","meetingDay":"T"},
                {"meetingModeCode":"92","meetingDay":"M"}
            ]
        });
        assert_ne!(
            canonical_section_semantic_hash_v1(&ordered).unwrap(),
            canonical_section_semantic_hash_v1(&reordered).unwrap()
        );

        let padded = json!({"meetingTimes":[{"meetingModeCode":" 92 "}]});
        let trimmed = json!({"meetingTimes":[{"meetingModeCode":"92"}]});
        assert_ne!(
            canonical_section_semantic_hash_v1(&padded).unwrap(),
            canonical_section_semantic_hash_v1(&trimmed).unwrap()
        );
    }

    #[test]
    fn v1_hash_domains_have_stable_synthetic_goldens() {
        let fixture = json!({
            "z": [" B ", "A"],
            "a": " synthetic ",
            "commentsText": "derived"
        });
        assert_eq!(
            canonical_raw_course_hash_v1(&fixture).unwrap().as_str(),
            "c669fa99c5c84a6b2a982da87112100086123865d23dc403226a03ce509bc27c"
        );
        assert_eq!(
            canonical_raw_section_hash_v1(&fixture).unwrap().as_str(),
            "c0acc9f453fe0eb41bc3e7d7d7f2f711b8ac7173bd7ede71f70205a323fb0635"
        );
        assert_eq!(
            canonical_section_semantic_hash_v1(&fixture)
                .unwrap()
                .as_str(),
            "14266143009a1553d22af83ac365977fe2fd118e6601ee9f2ae40b190580526f"
        );
        assert_eq!(
            canonical_occurrence_hash_v1(&fixture).unwrap().as_str(),
            "94db9f85ffcf0cb4bcdf92c4ca4df599ff21c25c123a6c5deb024a1ddd67c183"
        );
        assert_eq!(
            canonical_course_variant_hash_v1(&fixture).unwrap().as_str(),
            "7161547ee7d8db9f52e3c0786ad5fe4753de2dbecf49f801877eb40fb4948370"
        );
        assert_eq!(
            canonical_target_content_hash_v1(&fixture).unwrap().as_str(),
            "d365c106750d1b106ab38f85b5fefc7e5f167943c174ad1750aba2cc3624bb01"
        );
    }
}
