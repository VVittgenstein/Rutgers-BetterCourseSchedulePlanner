use bcsp_contracts::{
    API_PROTOCOL_VERSION, ApiErrorCode, ApiErrorEnvelope, ContractManifest, HttpSuccessEnvelope,
    SectionKey, WS_PROTOCOL_VERSION, WsServerEnvelope, contract_manifest,
};

#[test]
fn checked_in_v1_manifest_matches_the_generated_contract() {
    let checked_in: ContractManifest =
        serde_json::from_str(include_str!("golden/contract-manifest-v1.json")).unwrap();
    assert_eq!(checked_in, contract_manifest());
    assert_eq!(checked_in.api_protocol_version, 1);
    assert_eq!(checked_in.ws_protocol_version, 1);
    assert_eq!(API_PROTOCOL_VERSION, WS_PROTOCOL_VERSION);
}

#[test]
fn server_responses_allow_additive_fields_for_v1_consumers() {
    let response = r#"{
      "protocolVersion": 1,
      "data": {"term":"T2026F","campus":"CAMPUS_A","index":"00001"},
      "futureServerMetadata": {"safe": true}
    }"#;
    let decoded: HttpSuccessEnvelope<SectionKey> = serde_json::from_str(response).unwrap();
    assert_eq!(decoded.data().index().as_str(), "00001");

    let error = r#"{
      "protocolVersion": 1,
      "error": {
        "code": "SELECTION_LIMIT_EXCEEDED",
        "messageKey": "error.selection_limit_exceeded",
        "traceId": "00000000-0000-4000-8000-000000000001",
        "details": [{
          "kind": "LIMIT",
          "name": "selected_sections",
          "maximum": 9,
          "futureDetailMetadata": true
        }],
        "futureErrorMetadata": {"safe": true}
      },
      "futureEnvelopeMetadata": {"safe": true}
    }"#;
    let decoded: ApiErrorEnvelope = serde_json::from_str(error).unwrap();
    assert_eq!(decoded.error().code(), ApiErrorCode::SelectionLimitExceeded);

    let websocket = r#"{
      "protocolVersion": 1,
      "messageId": "00000000-0000-4000-8000-000000000001",
      "payload": {"term":"T2026F","campus":"CAMPUS_A","index":"00001"},
      "futureServerMetadata": {"safe": true}
    }"#;
    let decoded: WsServerEnvelope<SectionKey> = serde_json::from_str(websocket).unwrap();
    assert_eq!(decoded.payload().index().as_str(), "00001");
}

#[test]
fn every_schema_id_is_unique_and_versioned() {
    let manifest = contract_manifest();
    let mut ids = manifest
        .schemas
        .iter()
        .map(|schema| schema.id.as_str())
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), manifest.schemas.len());
    assert!(ids.iter().all(|id| id.ends_with(".v1")));
}

#[test]
fn every_manifest_reference_resolves_and_names_are_unique() {
    let manifest = contract_manifest();

    let mut scalar_ids = manifest
        .scalar_constraints
        .iter()
        .map(|constraint| constraint.id.as_str())
        .collect::<Vec<_>>();
    let scalar_count = scalar_ids.len();
    scalar_ids.sort_unstable();
    scalar_ids.dedup();
    assert_eq!(scalar_ids.len(), scalar_count);

    let schema_ids = manifest
        .schemas
        .iter()
        .map(|schema| schema.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let scalar_ids = scalar_ids
        .into_iter()
        .collect::<std::collections::BTreeSet<_>>();

    for schema in &manifest.schemas {
        if !schema.enum_values.is_empty() {
            assert!(schema.fields.is_empty(), "{}", schema.id);
            assert!(schema.discriminator.is_none(), "{}", schema.id);
            assert!(schema.variants.is_empty(), "{}", schema.id);
        }
        match schema.discriminator.as_deref() {
            Some(discriminator) => {
                assert!(schema.fields.is_empty(), "{}", schema.id);
                assert!(!schema.variants.is_empty(), "{}", schema.id);
                assert!(
                    schema.variants.iter().all(|variant| variant
                        .fields
                        .iter()
                        .all(|field| field.name != discriminator)),
                    "{}",
                    schema.id
                );
            }
            None => assert!(schema.variants.is_empty(), "{}", schema.id),
        }

        let mut field_names = schema
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>();
        let field_count = field_names.len();
        field_names.sort_unstable();
        field_names.dedup();
        assert_eq!(field_names.len(), field_count, "{}", schema.id);

        let mut tags = schema
            .variants
            .iter()
            .map(|variant| variant.tag_value.as_str())
            .collect::<Vec<_>>();
        let tag_count = tags.len();
        tags.sort_unstable();
        tags.dedup();
        assert_eq!(tags.len(), tag_count, "{}", schema.id);

        for variant in &schema.variants {
            let names = variant
                .fields
                .iter()
                .map(|field| field.name.as_str())
                .collect::<Vec<_>>();
            let unique = names
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>();
            assert_eq!(
                unique.len(),
                names.len(),
                "{}:{}",
                schema.id,
                variant.tag_value
            );
        }

        for field in schema
            .fields
            .iter()
            .chain(schema.variants.iter().flat_map(|variant| &variant.fields))
        {
            if let Some(reference) = field.type_ref.strip_prefix("$schema:") {
                assert!(schema_ids.contains(reference), "{reference}");
            } else if let Some(reference) = field.type_ref.strip_prefix("$array:$schema:") {
                assert!(schema_ids.contains(reference), "{reference}");
            } else if let Some(reference) = field.type_ref.strip_prefix("$scalar:") {
                assert!(scalar_ids.contains(reference), "{reference}");
            } else if field.type_ref.starts_with("$optional:$schema:") {
                let reference = field.type_ref.trim_start_matches("$optional:$schema:");
                assert!(schema_ids.contains(reference), "{reference}");
            } else if field.type_ref.starts_with("$optional:$scalar:") {
                let reference = field.type_ref.trim_start_matches("$optional:$scalar:");
                assert!(scalar_ids.contains(reference), "{reference}");
            } else {
                assert!(
                    field.type_ref.starts_with("$generic:")
                        || matches!(
                            field.type_ref.as_str(),
                            "$primitive:bool"
                                | "$primitive:rfc3339-timestamp"
                                | "$primitive:string"
                                | "$primitive:u16"
                                | "$primitive:u32"
                                | "$primitive:u64"
                        ),
                    "unsupported type reference {}",
                    field.type_ref
                );
            }
        }
    }
}
