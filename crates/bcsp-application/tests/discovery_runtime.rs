use std::str::FromStr;
use std::sync::{Arc, Mutex};

use bcsp_application::{publish_discovery_for_refresh, restore_refresh_targets};
use bcsp_contracts::{TermCampusKey, TraceId};
use bcsp_operational_storage::OperationalStorage;
use bcsp_rutgers_client::{DiscoverySnapshot, SourceProvenance, decode_discovery_payload};
use tempfile::TempDir;

const DISCOVERY_BODY: &[u8] = br#"{
  "sourceVersion":"synthetic-clean-restart-v1",
  "terms":[
    {"termId":"92026","year":2026,"termCode":"9","published":true},
    {"termId":"12027","year":2027,"termCode":"1","published":false}
  ],
  "campuses":[
    {"campusCode":"NB","enabled":true},
    {"campusCode":"CAMDEN","enabled":true},
    {"campusCode":"NWK","enabled":true}
  ],
  "targets":[
    {"termId":"92026","campusCode":"NB","enabled":true},
    {"termId":"92026","campusCode":"CAMDEN","enabled":false},
    {"termId":"12027","campusCode":"NWK","enabled":true}
  ],
  "subjects":[]
}"#;

fn trace(value: u64) -> TraceId {
    TraceId::from_str(&format!("00000000-0000-4000-8000-{value:012x}")).expect("synthetic trace ID")
}

#[test]
fn published_discovery_restores_strict_open_targets_after_sqlite_reopen() {
    let directory = TempDir::new().expect("temporary directory");
    let database = directory.path().join("clean-restart.sqlite");
    let storage = Arc::new(Mutex::new(
        OperationalStorage::open(&database).expect("initial operational storage"),
    ));
    let discovery = DiscoverySnapshot::try_from_raw(
        decode_discovery_payload(DISCOVERY_BODY).expect("synthetic discovery payload"),
        SourceProvenance::from_body(
            "SYNTHETIC_CLEAN_RESTART",
            "2030-01-01T00:00:00Z",
            DISCOVERY_BODY,
        ),
    )
    .expect("synthetic discovery snapshot");

    let initially_published = publish_discovery_for_refresh(
        &storage,
        &discovery,
        trace(1),
        "2030-01-01T00:00:00Z",
        "2030-01-01T00:00:01Z",
    )
    .expect("publish discovery");
    assert_eq!(initially_published.targets.len(), 1);
    drop(initially_published);
    drop(storage);

    let reopened = Arc::new(Mutex::new(
        OperationalStorage::open(&database).expect("reopened operational storage"),
    ));
    let restored = restore_refresh_targets(&reopened).expect("restore scheduler targets");

    assert_eq!(restored.len(), 1);
    let registration = &restored[0];
    assert_eq!(
        registration.target(),
        &TermCampusKey::try_new("92026", "NB").expect("target")
    );
    assert_eq!(registration.open_request().year(), 2026);
    assert_eq!(registration.open_request().term_code(), 9);
    assert_eq!(
        registration.open_request().target().target(),
        registration.target().clone()
    );
    assert!(
        registration
            .open_request()
            .discovery_source_id()
            .as_str()
            .starts_with("selector:")
    );
}
