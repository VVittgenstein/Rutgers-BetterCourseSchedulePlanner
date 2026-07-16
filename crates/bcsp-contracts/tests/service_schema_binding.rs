use std::collections::BTreeSet;

use bcsp_contracts::{
    CatalogContentVersion, CatalogDiscoveryAvailability, CatalogDiscoveryStatusV1, ContractSchema,
    SERVICE_STATUS_CONTRACT_VERSION, ServiceAvailabilitySummaryV1, ServiceAvailabilityV1,
    ServiceIssueComponentV1, ServiceIssueRecoveryV1, ServiceIssueSeverityV1, ServiceIssueV1,
    ServiceLevelV1, ServiceOperationPhaseV1, ServiceOperationV1, ServiceRuntimeV1, ServiceStatusV1,
    ServiceTargetStatusV1, TermCampusKey, contract_manifest,
};
use serde::Serialize;
use serde_json::Value;
use time::OffsetDateTime;

fn schema(id: &str) -> ContractSchema {
    contract_manifest()
        .schemas
        .into_iter()
        .find(|schema| schema.id == id)
        .unwrap_or_else(|| panic!("missing contract schema {id}"))
}

fn assert_object_binding<T: Serialize>(schema_id: &str, value: &T) {
    let schema = schema(schema_id);
    let Value::Object(object) = serde_json::to_value(value).expect("representative") else {
        panic!("representative contract is not an object")
    };
    let expected = schema
        .fields
        .into_iter()
        .map(|field| field.name)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        object
            .into_iter()
            .map(|(key, _)| key)
            .collect::<BTreeSet<_>>(),
        expected,
    );
}

fn enum_values<T: Serialize>(values: &[T]) -> Vec<Value> {
    values
        .iter()
        .map(|value| serde_json::to_value(value).expect("enum"))
        .collect()
}

#[test]
fn service_status_schema_is_bound_to_wire_shapes_and_enum_values() {
    let target = TermCampusKey::try_new("92026", "NB").expect("target");
    let operation = ServiceOperationV1 {
        phase: ServiceOperationPhaseV1::RetryWait,
        target: Some(target.clone()),
        started_at: Some(OffsetDateTime::UNIX_EPOCH),
        next_retry_at: Some(OffsetDateTime::UNIX_EPOCH),
    };
    let target_status = ServiceTargetStatusV1 {
        target: target.clone(),
        primary: true,
        catalog_availability: ServiceAvailabilityV1::Stale,
        catalog_content_version: Some(CatalogContentVersion::try_from(1).expect("version")),
        open_availability: ServiceAvailabilityV1::Unavailable,
        search_available: true,
    };
    let issue = ServiceIssueV1 {
        component: ServiceIssueComponentV1::Catalog,
        target: Some(target),
        code: "CATALOG_UPSTREAM_TRANSPORT".to_owned(),
        severity: ServiceIssueSeverityV1::Degraded,
        recovery: ServiceIssueRecoveryV1::AutomaticRetry,
        retry_at: Some(OffsetDateTime::UNIX_EPOCH),
    };
    let summary = ServiceAvailabilitySummaryV1 {
        total_target_count: 1,
        available_target_count: 1,
    };
    let status = ServiceStatusV1 {
        contract_version: SERVICE_STATUS_CONTRACT_VERSION,
        observed_at: OffsetDateTime::UNIX_EPOCH,
        runtime: ServiceRuntimeV1::Local,
        level: ServiceLevelV1::Degraded,
        operation: operation.clone(),
        discovery: CatalogDiscoveryStatusV1 {
            availability: CatalogDiscoveryAvailability::Current,
            latest_attempt: None,
            last_success: None,
            is_stale: false,
            error: None,
        },
        catalog: summary,
        open: summary,
        targets: vec![target_status.clone()],
        issues: vec![issue.clone()],
    };

    assert_object_binding("bcsp.service.operation.v1", &operation);
    assert_object_binding("bcsp.service.availability-summary.v1", &summary);
    assert_object_binding("bcsp.service.target-status.v1", &target_status);
    assert_object_binding("bcsp.service.issue.v1", &issue);
    assert_object_binding("bcsp.service.status.v1", &status);

    let cases = [
        (
            "bcsp.service.runtime.v1",
            enum_values(ServiceRuntimeV1::ALL),
        ),
        ("bcsp.service.level.v1", enum_values(ServiceLevelV1::ALL)),
        (
            "bcsp.service.operation-phase.v1",
            enum_values(ServiceOperationPhaseV1::ALL),
        ),
        (
            "bcsp.service.availability.v1",
            enum_values(ServiceAvailabilityV1::ALL),
        ),
        (
            "bcsp.service.issue-component.v1",
            enum_values(ServiceIssueComponentV1::ALL),
        ),
        (
            "bcsp.service.issue-severity.v1",
            enum_values(ServiceIssueSeverityV1::ALL),
        ),
        (
            "bcsp.service.issue-recovery.v1",
            enum_values(ServiceIssueRecoveryV1::ALL),
        ),
    ];
    for (schema_id, actual) in cases {
        let expected = schema(schema_id)
            .enum_values
            .into_iter()
            .map(Value::String)
            .collect::<Vec<_>>();
        assert_eq!(actual, expected, "{schema_id}");
    }
}
