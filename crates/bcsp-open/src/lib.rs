//! Shared Open-state reconciliation, freshness, and scheduling policy.

#![forbid(unsafe_code)]
#![deny(warnings)]

mod freshness;
mod gate;
mod policy;
mod projection;
mod reconcile;
mod scheduler;
mod service;

pub use freshness::{
    LastValidOpenPoint, LatestOpenAttemptPoint, OPEN_FRESHNESS_GRACE_SECONDS, OpenFreshnessError,
    project_open_freshness,
};
pub use gate::{
    BaselineEpoch, CandidateEpisode, CatalogSetIdentity, GATE_BASELINE_WINDOW,
    GATE_CONFIRM_MIN_CONSISTENT, GATE_CONFIRM_MIN_SPAN_SECONDS, GATE_CONSISTENCY_DENOMINATOR,
    GATE_DROP_THRESHOLD_DENOMINATOR, GATE_MAX_SAMPLE_GAP_SECONDS, GATE_MIN_ABSOLUTE_DROP,
    GATE_QUARANTINE_PROBE_CAP_SECONDS, GATE_RECOVERY_THRESHOLD_DENOMINATOR, GateDecision,
    GateDecisionKind, GateDisposition, GateRuntime, GateSample, GateState, RestartAttemptSummary,
    TargetGateSet, catalog_section_set_identity_v1, rebuild_after_restart,
};
pub use policy::{
    ACTIVE_WATCH_OPEN_INTERVAL_SECONDS, FATAL_DIAGNOSTIC_COOLDOWN_SECONDS, GeneralOpenInterval,
    LOCAL_DEFAULT_OPEN_INTERVAL_SECONDS, LOCAL_DEFAULT_WATCH_OPEN_INTERVAL_SECONDS,
    LOCAL_MAXIMUM_OPEN_INTERVAL_SECONDS, LOCAL_MAXIMUM_WATCH_OPEN_INTERVAL_SECONDS,
    LOCAL_MINIMUM_OPEN_INTERVAL_SECONDS, LOCAL_MINIMUM_WATCH_OPEN_INTERVAL_SECONDS,
    MISSING_RETRY_AFTER_CIRCUIT_SECONDS, OpenFailureKind, OpenIntervalError, OpenRefreshIntervals,
    OpenRuntimeMode, PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS, PUBLIC_WATCH_OPEN_INTERVAL_SECONDS,
    RetryDirective, RetryMode, TargetRetryClass, WatchOpenInterval, deterministic_jitter_v1,
    retry_directive, target_retry_directive,
};
pub use projection::{
    OpenCounterAudience, OpenProjectionError, OpenProjectionRuntime, OpenStatusReadStore,
    ProjectedOpenStatusV1, project_current_open_observation, project_open_status,
};
pub use reconcile::{
    CatalogOpenBatch, OpenReconcileClassification, OpenReconcilePlan, OpenSetEvidence,
    ReconcileInputError, ReconciledOpenState, SectionOpenUpdate, canonical_open_set_hash_v1,
    reconcile_open_set,
};
pub use scheduler::{
    CompletionSchedule, MonotonicTime, OriginCircuit, OriginDispatch, OriginDispatchHint,
    OriginEdfScheduler, OriginJobKey, OriginJobKind, OriginSchedulerLane, SchedulerError,
    restore_monotonic_due,
};
pub use service::{
    OpenGateRoute, OpenGateWiring, OpenPullClock, OpenPullCommand, OpenPullExecution,
    OpenPullFailure, OpenPullPersistence, OpenPullTerminal, SharedOpenService,
    SharedOpenServiceError, SystemOpenPullClock, rutgers_day_at,
};

pub const PACKAGE_BOUNDARY: &str = "bcsp-open";

pub fn boundary_marker() -> &'static str {
    let _ = (
        bcsp_contracts::PACKAGE_BOUNDARY,
        bcsp_domain::PACKAGE_BOUNDARY,
        bcsp_operational_storage::PACKAGE_BOUNDARY,
        bcsp_rutgers_client::PACKAGE_BOUNDARY,
    );
    PACKAGE_BOUNDARY
}

mod dependency_contract {
    use jiff as _;
    use serde as _;
    use thiserror as _;
    use time as _;
    use tokio as _;
    use tracing as _;
}

#[cfg(test)]
mod dev_dependency_contract {
    use proptest as _;
}
