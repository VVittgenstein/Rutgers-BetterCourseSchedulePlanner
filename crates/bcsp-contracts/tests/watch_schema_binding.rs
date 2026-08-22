use std::collections::BTreeSet;

use bcsp_contracts::{
    ContractSchema, OpenEpisodeState, SchemaDirection, UnknownFieldPolicy, WatchAlertDisposition,
    WatchCueOutcome, WatchNotificationMode, WatchStartRejectionReason, WatchStopReason,
    contract_manifest,
};
use serde::Serialize;

fn schema(id: &str) -> ContractSchema {
    contract_manifest()
        .schemas
        .into_iter()
        .find(|schema| schema.id == id)
        .unwrap_or_else(|| panic!("missing schema {id}"))
}

fn wire_values<T: Copy + Serialize>(values: &[T]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| {
            serde_json::to_value(value)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect()
}

#[test]
fn watch_enum_schemas_bind_to_wire_names() {
    for (id, expected) in [
        (
            "bcsp.watch.notification-mode.v1",
            wire_values(WatchNotificationMode::ALL),
        ),
        (
            "bcsp.watch.episode-state.v1",
            wire_values(OpenEpisodeState::ALL),
        ),
        (
            "bcsp.watch.alert-disposition.v1",
            wire_values(WatchAlertDisposition::ALL),
        ),
        (
            "bcsp.watch.cue-outcome.v1",
            wire_values(WatchCueOutcome::ALL),
        ),
        (
            "bcsp.watch.start-rejection-reason.v1",
            wire_values(WatchStartRejectionReason::ALL),
        ),
        (
            "bcsp.watch.stop-reason.v1",
            wire_values(WatchStopReason::ALL),
        ),
    ] {
        assert_eq!(
            schema(id).enum_values.into_iter().collect::<BTreeSet<_>>(),
            expected
        );
    }
}

#[test]
fn client_is_strict_server_is_additive_and_fanout_reuses_open_observation() {
    let client = schema("bcsp.watch.client-command.v1");
    assert_eq!(client.direction, SchemaDirection::ClientToServer);
    assert_eq!(client.unknown_fields, UnknownFieldPolicy::Reject);
    assert_eq!(client.discriminator.as_deref(), Some("type"));
    assert_eq!(client.variants.len(), 10);

    let server = schema("bcsp.watch.server-event.v1");
    assert_eq!(server.direction, SchemaDirection::ServerToClient);
    assert_eq!(server.unknown_fields, UnknownFieldPolicy::Ignore);
    assert_eq!(server.variants.len(), 8);

    let fanout = schema("bcsp.watch.open-observation-fanout.v1");
    let fields = fanout
        .fields
        .into_iter()
        .map(|field| (field.name, field.type_ref))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        fields,
        BTreeSet::from([
            ("activeWatchId".to_owned(), "$scalar:trace-id".to_owned(),),
            (
                "contractVersion".to_owned(),
                "$scalar:watch-contract-version".to_owned(),
            ),
            (
                "observation".to_owned(),
                "$schema:bcsp.open.observation.v1".to_owned(),
            ),
        ])
    );
}
