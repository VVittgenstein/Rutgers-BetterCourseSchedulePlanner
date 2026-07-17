use std::collections::BTreeSet;

use bcsp_contracts::{FilterRequestV1, QUERY_CONTRACT_VERSION, filter_schema_v1};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::Deserialize;
use serde_json::{Map as JsonMap, Value as JsonValue, json};

use crate::{
    PersonalStateError, PersonalStateResult, SavedViewContent, SavedViewIncompatibility,
    SavedViewReviewCode, SavedViewReviewReason,
};

pub(crate) const SAVED_VIEW_CODEC_VERSION: u64 = 1;
pub(crate) const SAVED_VIEW_SCHEMA_VERSION: u64 = QUERY_CONTRACT_VERSION.as_u16() as u64;

pub(crate) struct EncodedFilterSnapshot {
    pub schema_version: u64,
    pub raw: JsonValue,
}

pub(crate) fn encode_filter_snapshot(
    filters: &FilterRequestV1,
) -> PersonalStateResult<EncodedFilterSnapshot> {
    if filters.contract_version() != QUERY_CONTRACT_VERSION {
        return Err(PersonalStateError::InvalidFilterSnapshot);
    }
    let request = serde_json::to_value(filters)?;
    let mut values = request
        .get("values")
        .and_then(JsonValue::as_object)
        .cloned()
        .ok_or(PersonalStateError::InvalidFilterSnapshot)?;
    let include_incomplete = values
        .remove("includeIncomplete")
        .and_then(|value| value.as_object().cloned())
        .ok_or(PersonalStateError::InvalidFilterSnapshot)?;
    let schema = filter_schema_v1();
    let mut fields = JsonMap::new();
    for field in &schema.fields {
        let value = values
            .remove(&field.request_field)
            .ok_or(PersonalStateError::InvalidFilterSnapshot)?;
        let value = match field.stable_id.wire_name() {
            "FLT-C09" => json!({
                "value": value,
                "includeIncomplete": include_incomplete.get("prerequisite")
                    .cloned().ok_or(PersonalStateError::InvalidFilterSnapshot)?,
            }),
            "FLT-S04a" => json!({
                "value": value,
                "includeIncomplete": include_incomplete.get("modality")
                    .cloned().ok_or(PersonalStateError::InvalidFilterSnapshot)?,
            }),
            "FLT-S04b" => json!({
                "value": value,
                "includeIncomplete": include_incomplete.get("synchronicity")
                    .cloned().ok_or(PersonalStateError::InvalidFilterSnapshot)?,
            }),
            _ => value,
        };
        fields.insert(field.stable_id.wire_name().to_owned(), value);
    }
    if !values.is_empty() {
        return Err(PersonalStateError::InvalidFilterSnapshot);
    }
    Ok(EncodedFilterSnapshot {
        schema_version: SAVED_VIEW_SCHEMA_VERSION,
        raw: json!({
            "codecVersion": SAVED_VIEW_CODEC_VERSION,
            "schemaVersion": SAVED_VIEW_SCHEMA_VERSION,
            "fields": fields,
        }),
    })
}

pub(crate) fn decode_filter_snapshot(
    raw: JsonValue,
    stored_schema_version: u64,
) -> SavedViewContent {
    match decode_filter_snapshot_inner(&raw, stored_schema_version) {
        Ok(DecodedSnapshot::Compatible(filters)) => SavedViewContent::Compatible { filters },
        Ok(DecodedSnapshot::ReviewRequired(reasons)) => SavedViewContent::ReviewRequired {
            raw_snapshot: raw,
            reasons,
        },
        Err(reason) => SavedViewContent::Incompatible {
            raw_snapshot: raw,
            reason,
        },
    }
}

enum DecodedSnapshot {
    Compatible(Box<FilterRequestV1>),
    ReviewRequired(Vec<SavedViewReviewReason>),
}

fn decode_filter_snapshot_inner(
    raw: &JsonValue,
    stored_schema_version: u64,
) -> Result<DecodedSnapshot, SavedViewIncompatibility> {
    let envelope = raw
        .as_object()
        .ok_or(SavedViewIncompatibility::InvalidEnvelope)?;
    if envelope.len() != 3
        || !envelope.contains_key("codecVersion")
        || !envelope.contains_key("schemaVersion")
        || !envelope.contains_key("fields")
    {
        return Err(SavedViewIncompatibility::InvalidEnvelope);
    }
    let codec_version = envelope.get("codecVersion").and_then(JsonValue::as_u64);
    if codec_version != Some(SAVED_VIEW_CODEC_VERSION) {
        return Err(SavedViewIncompatibility::UnsupportedCodecVersion {
            observed: codec_version,
        });
    }
    let schema_version = envelope
        .get("schemaVersion")
        .and_then(JsonValue::as_u64)
        .ok_or(SavedViewIncompatibility::InvalidEnvelope)?;
    if schema_version != stored_schema_version {
        return Err(SavedViewIncompatibility::InvalidEnvelope);
    }
    if schema_version > SAVED_VIEW_SCHEMA_VERSION {
        return Err(SavedViewIncompatibility::FutureSchemaVersion {
            observed: schema_version,
            supported: SAVED_VIEW_SCHEMA_VERSION,
        });
    }
    if schema_version == 1 {
        return decode_v1_filter_snapshot(envelope);
    }
    if schema_version == 2 {
        return decode_v2_filter_snapshot(envelope);
    }
    decode_v3_filter_snapshot(envelope)
        .map(Box::new)
        .map(DecodedSnapshot::Compatible)
}

fn decode_v3_filter_snapshot(
    envelope: &JsonMap<String, JsonValue>,
) -> Result<FilterRequestV1, SavedViewIncompatibility> {
    let fields = envelope
        .get("fields")
        .and_then(JsonValue::as_object)
        .ok_or(SavedViewIncompatibility::InvalidEnvelope)?;
    let schema = filter_schema_v1();
    let known = schema
        .fields
        .iter()
        .map(|field| field.stable_id.wire_name())
        .collect::<BTreeSet<_>>();
    if let Some(unknown) = fields.keys().find(|field| !known.contains(field.as_str())) {
        return Err(SavedViewIncompatibility::UnknownField {
            stable_id: unknown.clone(),
        });
    }

    let mut request_values = JsonMap::new();
    let mut include_incomplete = JsonMap::new();
    for field in &schema.fields {
        let stable_id = field.stable_id.wire_name();
        let default_value = || {
            field.canonical_neutral.json().cloned().ok_or_else(|| {
                SavedViewIncompatibility::MissingRequiredField {
                    stable_id: stable_id.to_owned(),
                }
            })
        };
        let value = match fields.get(stable_id) {
            Some(value) if matches!(stable_id, "FLT-C09" | "FLT-S04a" | "FLT-S04b") => {
                let object = value
                    .as_object()
                    .filter(|object| {
                        object.len() == 2
                            && object.contains_key("value")
                            && object.contains_key("includeIncomplete")
                    })
                    .ok_or(SavedViewIncompatibility::InvalidFieldData)?;
                let include = object
                    .get("includeIncomplete")
                    .and_then(JsonValue::as_bool)
                    .ok_or(SavedViewIncompatibility::InvalidFieldData)?;
                let key = match stable_id {
                    "FLT-C09" => "prerequisite",
                    "FLT-S04a" => "modality",
                    "FLT-S04b" => "synchronicity",
                    _ => unreachable!(),
                };
                include_incomplete.insert(key.to_owned(), JsonValue::Bool(include));
                object
                    .get("value")
                    .cloned()
                    .ok_or(SavedViewIncompatibility::InvalidFieldData)?
            }
            Some(value) => value.clone(),
            None => {
                if let Some(key) = match stable_id {
                    "FLT-C09" => Some("prerequisite"),
                    "FLT-S04a" => Some("modality"),
                    "FLT-S04b" => Some("synchronicity"),
                    _ => None,
                } {
                    include_incomplete.insert(key.to_owned(), JsonValue::Bool(false));
                }
                default_value()?
            }
        };
        request_values.insert(field.request_field.clone(), value);
    }
    request_values.insert(
        "includeIncomplete".to_owned(),
        JsonValue::Object(include_incomplete),
    );
    serde_json::from_value(json!({
        "contractVersion": QUERY_CONTRACT_VERSION.as_u16(),
        "values": request_values,
    }))
    .map_err(|_| SavedViewIncompatibility::InvalidFieldData)
}

fn decode_v2_filter_snapshot(
    envelope: &JsonMap<String, JsonValue>,
) -> Result<DecodedSnapshot, SavedViewIncompatibility> {
    let fields = envelope
        .get("fields")
        .and_then(JsonValue::as_object)
        .ok_or(SavedViewIncompatibility::InvalidEnvelope)?;
    decode_v2_fields(fields)
}

fn decode_v2_fields(
    fields: &JsonMap<String, JsonValue>,
) -> Result<DecodedSnapshot, SavedViewIncompatibility> {
    let schema = filter_schema_v1();
    let known = schema
        .fields
        .iter()
        .map(|field| field.stable_id.wire_name())
        .collect::<BTreeSet<_>>();
    if let Some(unknown) = fields.keys().find(|field| !known.contains(field.as_str())) {
        return Err(SavedViewIncompatibility::UnknownField {
            stable_id: unknown.clone(),
        });
    }

    let mut reasons = Vec::new();
    for (stable_id, neutral) in [
        ("FLT-C05", json!([])),
        ("FLT-C09", json!("ANY")),
        ("FLT-S04a", json!([])),
        ("FLT-S04b", json!([])),
    ] {
        if let Some(value) = fields.get(stable_id) {
            let structurally_valid = match stable_id {
                "FLT-C09" => value.is_string(),
                _ => value.is_array(),
            };
            if !structurally_valid {
                return Err(SavedViewIncompatibility::InvalidFieldData);
            }
            if value != &neutral {
                reasons.push(SavedViewReviewReason {
                    stable_id: stable_id.to_owned(),
                    code: SavedViewReviewCode::ActiveLegacyFilter,
                });
            }
        }
    }
    if let Some(campuses) = fields.get("FLT-C02") {
        let campuses = campuses
            .as_array()
            .ok_or(SavedViewIncompatibility::InvalidFieldData)?;
        if campuses.iter().any(|campus| {
            campus
                .as_str()
                .is_none_or(|campus| !matches!(campus, "NB" | "NK" | "CM"))
        }) {
            reasons.push(SavedViewReviewReason {
                stable_id: "FLT-C02".to_owned(),
                code: SavedViewReviewCode::UnsupportedCampus,
            });
        }
    }
    if !reasons.is_empty() {
        return Ok(DecodedSnapshot::ReviewRequired(reasons));
    }

    let mut request_values = JsonMap::new();
    for field in &schema.fields {
        let stable_id = field.stable_id.wire_name();
        let value = if stable_id == "FLT-C05" {
            json!([])
        } else {
            match fields.get(stable_id) {
                Some(value) => value.clone(),
                None => field.canonical_neutral.json().cloned().ok_or_else(|| {
                    SavedViewIncompatibility::MissingRequiredField {
                        stable_id: stable_id.to_owned(),
                    }
                })?,
            }
        };
        request_values.insert(field.request_field.clone(), value);
    }
    request_values.insert(
        "includeIncomplete".to_owned(),
        json!({"prerequisite": false, "modality": false, "synchronicity": false}),
    );
    let filters = serde_json::from_value(json!({
        "contractVersion": QUERY_CONTRACT_VERSION.as_u16(),
        "values": request_values,
    }))
    .map_err(|_| SavedViewIncompatibility::InvalidFieldData)?;
    Ok(DecodedSnapshot::Compatible(Box::new(filters)))
}

fn decode_v1_filter_snapshot(
    envelope: &JsonMap<String, JsonValue>,
) -> Result<DecodedSnapshot, SavedViewIncompatibility> {
    let fields = envelope
        .get("fields")
        .and_then(JsonValue::as_object)
        .ok_or(SavedViewIncompatibility::InvalidEnvelope)?;
    const REMOVED: [(&str, &str); 4] = [
        ("FLT-C10", "array"),
        ("FLT-S02", "array"),
        ("FLT-S08", "buildingRoom"),
        ("FLT-S11", "eligibility"),
    ];
    let neutral_building = json!({"buildingCodes": [], "roomNumbers": []});
    let neutral_eligibility = json!({
        "majorCodes": [],
        "minorCodes": [],
        "honorProgramCodes": [],
        "unitCodes": [],
        "unitMajors": []
    });
    for (stable_id, kind) in REMOVED {
        let Some(value) = fields.get(stable_id) else {
            continue;
        };
        let neutral = match kind {
            "array" => value.as_array().is_some_and(Vec::is_empty),
            "buildingRoom" => value == &neutral_building,
            "eligibility" => value == &neutral_eligibility,
            _ => false,
        };
        if !neutral {
            return Err(SavedViewIncompatibility::UnknownField {
                stable_id: stable_id.to_owned(),
            });
        }
    }

    let schema = filter_schema_v1();
    let active_ids = schema
        .fields
        .iter()
        .map(|field| field.stable_id.wire_name())
        .collect::<BTreeSet<_>>();
    let removed_ids = REMOVED
        .iter()
        .map(|(stable_id, _)| *stable_id)
        .collect::<BTreeSet<_>>();
    if let Some(unknown) = fields
        .keys()
        .find(|field| !active_ids.contains(field.as_str()) && !removed_ids.contains(field.as_str()))
    {
        return Err(SavedViewIncompatibility::UnknownField {
            stable_id: unknown.clone(),
        });
    }

    let mut migrated_fields = JsonMap::new();
    for field in &schema.fields {
        let stable_id = field.stable_id.wire_name();
        let value = match fields.get(stable_id) {
            Some(value) if stable_id == "FLT-C04" => match value {
                JsonValue::Null => json!([]),
                JsonValue::String(text) => json!(
                    text.split_whitespace()
                        .map(str::to_owned)
                        .collect::<Vec<_>>()
                ),
                _ => return Err(SavedViewIncompatibility::InvalidFieldData),
            },
            Some(value) if stable_id == "FLT-S07" => json!({
                "locations": value,
                "mode": "ANY_MEETING"
            }),
            Some(value) => value.clone(),
            None => field.canonical_neutral.json().cloned().ok_or_else(|| {
                SavedViewIncompatibility::MissingRequiredField {
                    stable_id: stable_id.to_owned(),
                }
            })?,
        };
        migrated_fields.insert(stable_id.to_owned(), value);
    }
    decode_v2_fields(&migrated_fields)
}

pub(crate) fn migrate_legacy_current_filters(
    transaction: &Transaction<'_>,
) -> PersonalStateResult<()> {
    let settings_json = transaction
        .query_row(
            "SELECT settings_json FROM personal_settings_v1 WHERE singleton_id = 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(settings_json) = settings_json else {
        return Ok(());
    };
    let mut settings = serde_json::from_str::<JsonValue>(&settings_json)?;
    let object = settings
        .as_object_mut()
        .ok_or(PersonalStateError::InvalidFilterSnapshot)?;
    let legacy = object.remove("currentFilters");
    transaction.execute(
        "UPDATE personal_settings_v1 SET settings_json = ?1 WHERE singleton_id = 1",
        [serde_json::to_string(&settings)?],
    )?;
    let Some(legacy) = legacy.filter(|value| !value.is_null()) else {
        return Ok(());
    };
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct LegacyCurrentFilters {
        association: crate::FilterAssociation,
        filters: JsonValue,
    }
    let current = serde_json::from_value::<LegacyCurrentFilters>(legacy)?;
    let encoded = encode_legacy_request_snapshot(current.filters)?;
    transaction.execute(
        "INSERT INTO personal_current_filters_v1(
            singleton_id, revision, has_value, association_json, schema_version, snapshot_json
         ) VALUES (1, 1, 1, ?1, ?2, ?3)",
        params![
            serde_json::to_string(&current.association)?,
            i64::try_from(encoded.schema_version)
                .map_err(|_| PersonalStateError::RevisionOverflow)?,
            serde_json::to_string(&encoded.raw)?,
        ],
    )?;
    Ok(())
}

/// Converts the pre-snapshot `settings.currentFilters.filters` request into
/// the stable-field snapshot envelope used by the dedicated Local table.
/// Query V1/V2 are accepted only here, as persisted-state migration input;
/// every new Local mutation still goes through `encode_filter_snapshot` and
/// therefore requires the active V3 request contract.
fn encode_legacy_request_snapshot(raw: JsonValue) -> PersonalStateResult<EncodedFilterSnapshot> {
    let object = raw
        .as_object()
        .filter(|object| {
            object.len() == 2
                && object.contains_key("contractVersion")
                && object.contains_key("values")
        })
        .ok_or(PersonalStateError::InvalidFilterSnapshot)?;
    let version = object
        .get("contractVersion")
        .and_then(JsonValue::as_u64)
        .ok_or(PersonalStateError::InvalidFilterSnapshot)?;
    if version == SAVED_VIEW_SCHEMA_VERSION {
        let filters = serde_json::from_value::<FilterRequestV1>(raw)?;
        return encode_filter_snapshot(&filters);
    }
    if !matches!(version, 1 | 2) {
        return Err(PersonalStateError::InvalidFilterSnapshot);
    }
    let values = object
        .get("values")
        .and_then(JsonValue::as_object)
        .ok_or(PersonalStateError::InvalidFilterSnapshot)?;
    let mut fields = JsonMap::new();
    for field in filter_schema_v1().fields {
        let source = match (version, field.stable_id.wire_name()) {
            (1, "FLT-C04") => "text",
            (1 | 2, "FLT-C05") => "courseNumbers",
            _ => field.request_field.as_str(),
        };
        let value = values
            .get(source)
            .cloned()
            .ok_or(PersonalStateError::InvalidFilterSnapshot)?;
        fields.insert(field.stable_id.wire_name().to_owned(), value);
    }
    if version == 1 {
        for (stable_id, source) in [
            ("FLT-C10", "courseLocations"),
            ("FLT-S02", "sectionNumbers"),
            ("FLT-S08", "buildingRoom"),
            ("FLT-S11", "eligibility"),
        ] {
            fields.insert(
                stable_id.to_owned(),
                values
                    .get(source)
                    .cloned()
                    .ok_or(PersonalStateError::InvalidFilterSnapshot)?,
            );
        }
    }
    let expected_field_count = if version == 1 { 22 } else { 18 };
    if values.len() != expected_field_count {
        return Err(PersonalStateError::InvalidFilterSnapshot);
    }
    Ok(EncodedFilterSnapshot {
        schema_version: version,
        raw: json!({
            "codecVersion": SAVED_VIEW_CODEC_VERSION,
            "schemaVersion": version,
            "fields": fields,
        }),
    })
}

#[cfg(test)]
mod tests {
    use bcsp_contracts::{
        MeetingLocationMatchModeV2, NormalizedFilterValuesV1, QueryContractVersion,
    };

    use super::*;

    fn legacy_v1_snapshot() -> JsonValue {
        let mut fields = JsonMap::new();
        for field in filter_schema_v1().fields {
            let value = if field.stable_id.wire_name() == "FLT-C01" {
                json!("T2026F")
            } else {
                field.canonical_neutral.into_json().unwrap()
            };
            fields.insert(field.stable_id.wire_name().to_owned(), value);
        }
        fields.insert("FLT-C04".to_owned(), JsonValue::Null);
        fields.insert("FLT-S07".to_owned(), json!([]));
        fields.insert("FLT-C10".to_owned(), json!([]));
        fields.insert("FLT-S02".to_owned(), json!([]));
        fields.insert(
            "FLT-S08".to_owned(),
            json!({"buildingCodes": [], "roomNumbers": []}),
        );
        fields.insert(
            "FLT-S11".to_owned(),
            json!({
                "majorCodes": [],
                "minorCodes": [],
                "honorProgramCodes": [],
                "unitCodes": [],
                "unitMajors": []
            }),
        );
        json!({"codecVersion": 1, "schemaVersion": 1, "fields": fields})
    }

    #[test]
    fn neutral_v1_removed_fields_migrate_to_v3_without_broadening() {
        let content = decode_filter_snapshot(legacy_v1_snapshot(), 1);
        let filters = content.filters().expect("neutral V1 snapshot migrates");
        assert_eq!(filters.contract_version(), QueryContractVersion::V3);
        assert!(filters.values().keywords().is_none());
        assert!(filters.values().meeting_locations().locations.is_empty());
        assert_eq!(
            filters.values().meeting_locations().mode,
            MeetingLocationMatchModeV2::AnyMeeting
        );
    }

    fn neutral_v2_snapshot() -> JsonValue {
        let mut fields = JsonMap::new();
        for field in filter_schema_v1().fields {
            let value = if field.stable_id.wire_name() == "FLT-C01" {
                json!("92026")
            } else if field.stable_id.wire_name() == "FLT-C02" {
                json!(["NB"])
            } else {
                field.canonical_neutral.into_json().unwrap()
            };
            fields.insert(field.stable_id.wire_name().to_owned(), value);
        }
        json!({"codecVersion": 1, "schemaVersion": 2, "fields": fields})
    }

    fn neutral_v2_request() -> JsonValue {
        let filters = FilterRequestV1::new(NormalizedFilterValuesV1::for_term(
            bcsp_contracts::TermId::try_from("92026").unwrap(),
        ));
        let mut request = serde_json::to_value(filters).unwrap();
        request["contractVersion"] = json!(2);
        let values = request["values"].as_object_mut().unwrap();
        values.remove("includeIncomplete").unwrap();
        let bands = values.remove("courseNumberBands").unwrap();
        values.insert("courseNumbers".to_owned(), bands);
        request
    }

    #[test]
    fn statically_safe_v2_snapshot_migrates_to_strict_v3_neutral_controls() {
        let content = decode_filter_snapshot(neutral_v2_snapshot(), 2);
        let filters = content.filters().expect("safe V2 snapshot migrates");
        assert_eq!(filters.contract_version(), QueryContractVersion::V3);
        assert!(filters.values().course_number_bands().is_empty());
        assert_eq!(
            filters.values().include_incomplete(),
            bcsp_contracts::IncludeIncompleteV3::default()
        );
    }

    #[test]
    fn pre_snapshot_v2_request_is_only_adapted_by_local_persistence_migration() {
        let encoded = encode_legacy_request_snapshot(neutral_v2_request()).unwrap();
        assert_eq!(encoded.schema_version, 2);
        let content = decode_filter_snapshot(encoded.raw, encoded.schema_version);
        let filters = content.filters().expect("neutral V2 request migrates");
        assert_eq!(filters.contract_version(), QueryContractVersion::V3);

        let mut active = neutral_v2_request();
        active["values"]["courseNumbers"] = json!(["198"]);
        let encoded = encode_legacy_request_snapshot(active).unwrap();
        assert!(matches!(
            decode_filter_snapshot(encoded.raw, encoded.schema_version),
            SavedViewContent::ReviewRequired { reasons, .. }
                if reasons == vec![SavedViewReviewReason {
                    stable_id: "FLT-C05".to_owned(),
                    code: SavedViewReviewCode::ActiveLegacyFilter,
                }]
        ));
    }

    #[test]
    fn active_v2_axes_and_removed_campus_preserve_raw_as_review_required() {
        for (stable_id, value, code) in [
            (
                "FLT-C05",
                json!(["111"]),
                SavedViewReviewCode::ActiveLegacyFilter,
            ),
            (
                "FLT-C09",
                json!("HAS"),
                SavedViewReviewCode::ActiveLegacyFilter,
            ),
            (
                "FLT-S04a",
                json!(["UNKNOWN"]),
                SavedViewReviewCode::ActiveLegacyFilter,
            ),
            (
                "FLT-S04b",
                json!(["UNSPECIFIED"]),
                SavedViewReviewCode::ActiveLegacyFilter,
            ),
            (
                "FLT-C02",
                json!(["NB", "ONLINE_NB"]),
                SavedViewReviewCode::UnsupportedCampus,
            ),
        ] {
            let mut raw = neutral_v2_snapshot();
            raw["fields"][stable_id] = value;
            let content = decode_filter_snapshot(raw.clone(), 2);
            assert_eq!(
                content,
                SavedViewContent::ReviewRequired {
                    raw_snapshot: raw,
                    reasons: vec![SavedViewReviewReason {
                        stable_id: stable_id.to_owned(),
                        code,
                    }],
                },
                "{stable_id}"
            );
        }
    }

    #[test]
    fn v3_snapshot_round_trip_keeps_each_incomplete_switch_independent() {
        let mut input = bcsp_contracts::FilterValuesInputV1::for_term(
            bcsp_contracts::TermId::try_from("92026").unwrap(),
        );
        input.include_incomplete.prerequisite = true;
        input.include_incomplete.modality = false;
        input.include_incomplete.synchronicity = true;
        let filters =
            FilterRequestV1::new(bcsp_contracts::NormalizedFilterValuesV1::try_new(input).unwrap());
        let encoded = encode_filter_snapshot(&filters).unwrap();
        let decoded = decode_filter_snapshot(encoded.raw, encoded.schema_version);
        assert_eq!(decoded.filters(), Some(&filters));
    }

    #[test]
    fn new_local_snapshots_reject_non_v3_request_envelopes() {
        let filters = FilterRequestV1::new(bcsp_contracts::NormalizedFilterValuesV1::for_term(
            bcsp_contracts::TermId::try_from("92026").unwrap(),
        ));
        let mut raw = serde_json::to_value(filters).unwrap();
        raw["contractVersion"] = json!(2);
        let legacy_shaped = serde_json::from_value::<FilterRequestV1>(raw).unwrap();
        assert!(matches!(
            encode_filter_snapshot(&legacy_shaped),
            Err(PersonalStateError::InvalidFilterSnapshot)
        ));
    }

    #[test]
    fn active_removed_v1_field_is_preserved_as_incompatible() {
        let mut raw = legacy_v1_snapshot();
        raw["fields"]["FLT-C10"] = json!(["BUSCH"]);
        let content = decode_filter_snapshot(raw, 1);
        assert!(matches!(
            content.incompatibility(),
            Some(SavedViewIncompatibility::UnknownField { stable_id })
                if stable_id == "FLT-C10"
        ));
    }
}
