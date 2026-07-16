use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{CatalogContentVersion, CatalogDiscoveryStatusV1, TermCampusKey};

pub const SERVICE_STATUS_CONTRACT_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServiceRuntimeV1 {
    Local,
    Public,
}

impl ServiceRuntimeV1 {
    pub const ALL: &'static [Self] = &[Self::Local, Self::Public];
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServiceLevelV1 {
    Initializing,
    PartiallyReady,
    Ready,
    Degraded,
    Error,
}

impl ServiceLevelV1 {
    pub const ALL: &'static [Self] = &[
        Self::Initializing,
        Self::PartiallyReady,
        Self::Ready,
        Self::Degraded,
        Self::Error,
    ];
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServiceOperationPhaseV1 {
    Starting,
    Discovering,
    CatalogFetch,
    CatalogProcess,
    CatalogPublish,
    OpenFetch,
    Idle,
    RetryWait,
    Stopped,
}

impl ServiceOperationPhaseV1 {
    pub const ALL: &'static [Self] = &[
        Self::Starting,
        Self::Discovering,
        Self::CatalogFetch,
        Self::CatalogProcess,
        Self::CatalogPublish,
        Self::OpenFetch,
        Self::Idle,
        Self::RetryWait,
        Self::Stopped,
    ];
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceOperationV1 {
    pub phase: ServiceOperationPhaseV1,
    pub target: Option<TermCampusKey>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub started_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub next_retry_at: Option<OffsetDateTime>,
}

impl ServiceOperationV1 {
    pub const fn starting() -> Self {
        Self {
            phase: ServiceOperationPhaseV1::Starting,
            target: None,
            started_at: None,
            next_retry_at: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServiceAvailabilityV1 {
    Unavailable,
    Current,
    Stale,
}

impl ServiceAvailabilityV1 {
    pub const ALL: &'static [Self] = &[Self::Unavailable, Self::Current, Self::Stale];
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceTargetStatusV1 {
    pub target: TermCampusKey,
    pub primary: bool,
    pub catalog_availability: ServiceAvailabilityV1,
    pub catalog_content_version: Option<CatalogContentVersion>,
    pub open_availability: ServiceAvailabilityV1,
    pub search_available: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServiceIssueComponentV1 {
    Discovery,
    Catalog,
    Open,
    Scheduler,
    Storage,
}

impl ServiceIssueComponentV1 {
    pub const ALL: &'static [Self] = &[
        Self::Discovery,
        Self::Catalog,
        Self::Open,
        Self::Scheduler,
        Self::Storage,
    ];
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServiceIssueSeverityV1 {
    Degraded,
    Blocking,
}

impl ServiceIssueSeverityV1 {
    pub const ALL: &'static [Self] = &[Self::Degraded, Self::Blocking];
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServiceIssueRecoveryV1 {
    AutomaticRetry,
    UserActionRequired,
}

impl ServiceIssueRecoveryV1 {
    pub const ALL: &'static [Self] = &[Self::AutomaticRetry, Self::UserActionRequired];
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceIssueV1 {
    pub component: ServiceIssueComponentV1,
    pub target: Option<TermCampusKey>,
    pub code: String,
    pub severity: ServiceIssueSeverityV1,
    pub recovery: ServiceIssueRecoveryV1,
    #[serde(with = "time::serde::rfc3339::option")]
    pub retry_at: Option<OffsetDateTime>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceAvailabilitySummaryV1 {
    pub total_target_count: u64,
    pub available_target_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatusV1 {
    pub contract_version: u16,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    pub runtime: ServiceRuntimeV1,
    pub level: ServiceLevelV1,
    pub operation: ServiceOperationV1,
    pub discovery: CatalogDiscoveryStatusV1,
    pub catalog: ServiceAvailabilitySummaryV1,
    pub open: ServiceAvailabilitySummaryV1,
    pub targets: Vec<ServiceTargetStatusV1>,
    pub issues: Vec<ServiceIssueV1>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn optional_operation_fields_are_explicit_nulls() {
        assert_eq!(
            serde_json::to_value(ServiceOperationV1::starting()).expect("operation"),
            json!({
                "phase": "STARTING",
                "target": null,
                "startedAt": null,
                "nextRetryAt": null
            })
        );
    }
}
