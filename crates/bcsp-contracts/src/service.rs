use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::{CatalogContentVersion, CatalogDiscoveryStatusV1, TermCampusKey, TermId, TraceId};

pub const SERVICE_STATUS_CONTRACT_VERSION: u16 = 1;
pub const SERVICE_STATUS_V2_CONTRACT_VERSION: u16 = 2;

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
    /// Targets backed by a current, successful publication.
    pub current_target_count: u64,
    /// Targets whose last usable publication is retained after a failed refresh.
    pub stale_target_count: u64,
    /// Targets without any usable publication.
    pub unavailable_target_count: u64,
    /// Backwards-compatible aggregate of current and stale targets.
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceVisibleTermV2 {
    pub term: TermId,
    pub relative_offset: i8,
    pub discovered: bool,
    pub auto_managed: bool,
    pub manual_pull_allowed: bool,
    pub watchable: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceTermWindowV2 {
    pub current_term: TermId,
    pub next_term: TermId,
    pub visible_terms: Vec<ServiceVisibleTermV2>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceAutomaticTermSummaryV2 {
    pub term: TermId,
    pub ready_target_count: u64,
    pub total_target_count: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServiceOperationStageV2 {
    CatalogFetch,
    CatalogProcess,
    OpenFetch,
    SnapshotPublish,
}

impl ServiceOperationStageV2 {
    pub const ALL: &'static [Self] = &[
        Self::CatalogFetch,
        Self::CatalogProcess,
        Self::OpenFetch,
        Self::SnapshotPublish,
    ];
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceOperationV2 {
    pub target: TermCampusKey,
    pub stage: ServiceOperationStageV2,
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: OffsetDateTime,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServiceSnapshotAvailabilityV2 {
    Unrequested,
    NoCompleteSnapshot,
    Ready,
}

impl ServiceSnapshotAvailabilityV2 {
    pub const ALL: &'static [Self] = &[Self::Unrequested, Self::NoCompleteSnapshot, Self::Ready];
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ServiceWorkStateV2 {
    Idle,
    Queued,
    Running,
    RetryWait,
}

impl ServiceWorkStateV2 {
    pub const ALL: &'static [Self] = &[Self::Idle, Self::Queued, Self::Running, Self::RetryWait];
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceTargetErrorV2 {
    pub code: String,
    pub http_status: Option<u16>,
    pub content_type: Option<String>,
    pub content_encoding: Option<String>,
    pub decoded_bytes: Option<u64>,
    pub error_class: Option<String>,
    pub error_chain: Option<String>,
    pub trace_id: Option<TraceId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceTargetStatusV2 {
    pub target: TermCampusKey,
    pub primary: bool,
    pub snapshot_availability: ServiceSnapshotAvailabilityV2,
    pub work_state: ServiceWorkStateV2,
    pub stage: Option<ServiceOperationStageV2>,
    pub usable: bool,
    pub catalog_content_version: Option<CatalogContentVersion>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_complete_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub next_retry_at: Option<OffsetDateTime>,
    pub error: Option<ServiceTargetErrorV2>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceStatusV2 {
    pub contract_version: u16,
    #[serde(with = "time::serde::rfc3339")]
    pub observed_at: OffsetDateTime,
    pub runtime: ServiceRuntimeV1,
    pub level: ServiceLevelV1,
    pub discovery: CatalogDiscoveryStatusV1,
    pub term_window: ServiceTermWindowV2,
    pub automatic_term_summaries: Vec<ServiceAutomaticTermSummaryV2>,
    pub operations: Vec<ServiceOperationV2>,
    pub targets: Vec<ServiceTargetStatusV2>,
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

    #[test]
    fn v2_target_optional_diagnostics_are_explicit_nulls() {
        let target = ServiceTargetStatusV2 {
            target: TermCampusKey::try_new("72026", "NB").expect("target"),
            primary: true,
            snapshot_availability: ServiceSnapshotAvailabilityV2::Unrequested,
            work_state: ServiceWorkStateV2::Idle,
            stage: None,
            usable: false,
            catalog_content_version: None,
            last_complete_at: None,
            next_retry_at: None,
            error: None,
        };

        assert_eq!(
            serde_json::to_value(target).expect("target"),
            json!({
                "target": {"term": "72026", "campus": "NB"},
                "primary": true,
                "snapshotAvailability": "UNREQUESTED",
                "workState": "IDLE",
                "stage": null,
                "usable": false,
                "catalogContentVersion": null,
                "lastCompleteAt": null,
                "nextRetryAt": null,
                "error": null
            })
        );
    }
}
