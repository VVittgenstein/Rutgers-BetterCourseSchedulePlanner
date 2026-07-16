use bcsp_contracts::{TermCampusKey, TraceId};
use bcsp_operational_storage::{
    BeginRefreshAttemptCommand, CatalogFailureAudit, FinishRefreshFailureCommand,
    OperationalStorage, RefreshFailureStage, StorageError,
};

const STARTED: &str = "2026-07-17T00:00:00Z";
const COMPLETED: &str = "2026-07-17T00:00:01Z";

fn trace() -> TraceId {
    "123e4567-e89b-42d3-a456-426614174000"
        .parse()
        .expect("canonical UUIDv4")
}

fn begin(storage: &mut OperationalStorage) {
    storage
        .begin_refresh_attempt(&BeginRefreshAttemptCommand {
            observation_id: trace(),
            target: TermCampusKey::try_new("72026", "NB").expect("target"),
            started_at: STARTED.to_owned(),
            source_content_sha256: None,
            source_bytes: None,
        })
        .expect("begin Catalog attempt");
}

fn failure() -> FinishRefreshFailureCommand {
    FinishRefreshFailureCommand {
        observation_id: trace(),
        completed_at: COMPLETED.to_owned(),
        stage: RefreshFailureStage::Transport,
        source_content_sha256: None,
        source_bytes: Some(1_337),
        error_code: "CATALOG_UPSTREAM_TRANSPORT".to_owned(),
        diagnostic_token: None,
    }
}

#[test]
fn failed_catalog_attempt_atomically_persists_bounded_http_audit() {
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    begin(&mut storage);
    let audit = CatalogFailureAudit {
        http_status: Some(503),
        content_type: Some("application/json".to_owned()),
        content_encoding: Some("gzip".to_owned()),
        decoded_bytes: Some(1_337),
        error_chain: Some("Catalog upstream returned non-success HTTP status 503".to_owned()),
    };

    storage
        .finish_refresh_failure_with_audit(&failure(), Some(&audit))
        .expect("finish with audit");

    assert_eq!(
        storage.catalog_failure_audit(&trace()).expect("read audit"),
        Some(audit)
    );
    assert_eq!(
        storage
            .refresh_observation(&trace())
            .expect("read observation")
            .expect("observation")
            .error_code
            .as_deref(),
        Some("CATALOG_UPSTREAM_TRANSPORT")
    );
}

#[test]
fn unsafe_or_unbounded_error_detail_is_rejected_before_failure_is_committed() {
    let mut storage = OperationalStorage::open_in_memory().expect("storage");
    begin(&mut storage);
    let unsafe_audit = CatalogFailureAudit {
        error_chain: Some("safe prefix\r\nAuthorization: secret".to_owned()),
        ..CatalogFailureAudit::default()
    };

    assert!(matches!(
        storage.finish_refresh_failure_with_audit(&failure(), Some(&unsafe_audit)),
        Err(StorageError::InvalidCommand {
            field: "error_chain",
            ..
        })
    ));
    assert!(
        storage
            .catalog_failure_audit(&trace())
            .expect("read audit")
            .is_none()
    );
}
