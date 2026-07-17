use std::collections::BTreeSet;

use bcsp_contracts::{FilterRequestV1, QUERY_CONTRACT_VERSION, filter_schema_v1};
use rusqlite::{OptionalExtension, Transaction, params};
use serde::Deserialize;
use serde_json::{Map as JsonMap, Value as JsonValue, json};

use crate::{PersonalStateError, PersonalStateResult, SavedViewContent, SavedViewIncompatibility};

pub(crate) const SAVED_VIEW_CODEC_VERSION: u64 = 1;
pub(crate) const SAVED_VIEW_SCHEMA_VERSION: u64 = QUERY_CONTRACT_VERSION.as_u16() as u64;

pub(crate) struct EncodedFilterSnapshot {
    pub schema_version: u64,
    pub raw: JsonValue,
}

pub(crate) fn encode_filter_snapshot(
    filters: &FilterRequestV1,
) -> PersonalStateResult<EncodedFilterSnapshot> {
    let request = serde_json::to_value(filters)?;
    let mut values = request
        .get("values")
        .and_then(JsonValue::as_object)
        .cloned()
        .ok_or(PersonalStateError::InvalidFilterSnapshot)?;
    let schema = filter_schema_v1();
    let mut fields = JsonMap::new();
    for field in &schema.fields {
        let value = values
            .remove(&field.request_field)
            .ok_or(PersonalStateError::InvalidFilterSnapshot)?;
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
    match decode_compatible_filter_snapshot(&raw, stored_schema_version) {
        Ok(filters) => SavedViewContent::Compatible {
            filters: Box::new(filters),
        },
        Err(reason) => SavedViewContent::Incompatible {
            raw_snapshot: raw,
            reason,
        },
    }
}

fn decode_compatible_filter_snapshot(
    raw: &JsonValue,
    stored_schema_version: u64,
) -> Result<FilterRequestV1, SavedViewIncompatibility> {
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
    for field in &schema.fields {
        let value = match fields.get(field.stable_id.wire_name()) {
            Some(value) => value.clone(),
            None => field.canonical_neutral.json().cloned().ok_or_else(|| {
                SavedViewIncompatibility::MissingRequiredField {
                    stable_id: field.stable_id.wire_name().to_owned(),
                }
            })?,
        };
        request_values.insert(field.request_field.clone(), value);
    }
    serde_json::from_value(json!({
        "contractVersion": QUERY_CONTRACT_VERSION.as_u16(),
        "values": request_values,
    }))
    .map_err(|_| SavedViewIncompatibility::InvalidFieldData)
}

fn decode_v1_filter_snapshot(
    envelope: &JsonMap<String, JsonValue>,
) -> Result<FilterRequestV1, SavedViewIncompatibility> {
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

    let mut request_values = JsonMap::new();
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
        request_values.insert(field.request_field.clone(), value);
    }
    serde_json::from_value(json!({
        "contractVersion": QUERY_CONTRACT_VERSION.as_u16(),
        "values": request_values,
    }))
    .map_err(|_| SavedViewIncompatibility::InvalidFieldData)
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
        filters: FilterRequestV1,
    }
    let current = serde_json::from_value::<LegacyCurrentFilters>(legacy)?;
    let encoded = encode_filter_snapshot(&current.filters)?;
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

#[cfg(test)]
mod tests {
    use bcsp_contracts::{MeetingLocationMatchModeV2, QueryContractVersion};

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
    fn neutral_v1_removed_fields_migrate_to_v2_without_broadening() {
        let content = decode_filter_snapshot(legacy_v1_snapshot(), 1);
        let filters = content.filters().expect("neutral V1 snapshot migrates");
        assert_eq!(filters.contract_version(), QueryContractVersion::V2);
        assert!(filters.values().keywords().is_none());
        assert!(filters.values().meeting_locations().locations.is_empty());
        assert_eq!(
            filters.values().meeting_locations().mode,
            MeetingLocationMatchModeV2::AnyMeeting
        );
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
