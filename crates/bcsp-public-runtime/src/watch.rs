use std::sync::Arc;

use bcsp_application::{NoopWatchDispatchSink, SharedWatchSocket, WatchAdmissionSource};
use bcsp_contracts::{
    OpenCircuitState, OpenCircuitStatusV1, OpenSchedulerLane, OpenSchedulerStatusV1, SectionKey,
};
use bcsp_open::{
    OpenCounterAudience, OpenProjectionError, OpenProjectionRuntime,
    project_current_open_observation,
};
use bcsp_watch::{WatchManagerError, WatchStartAdmission};
use time::OffsetDateTime;

use crate::{
    PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS, PUBLIC_WATCHED_OPEN_INTERVAL_SECONDS,
    SharedPublicOperationalStore,
};

struct PublicWatchAdmission {
    store: SharedPublicOperationalStore,
}

impl WatchAdmissionSource for PublicWatchAdmission {
    fn admission_for(&self, section: &SectionKey) -> WatchStartAdmission {
        let Ok(mut store) = self.store.lock() else {
            return WatchStartAdmission::TargetUnavailable;
        };
        let runtime = watch_projection_runtime(OffsetDateTime::now_utc());
        admission_from_projection(project_current_open_observation(
            store.storage_mut(),
            section,
            &runtime,
        ))
    }
}

fn watch_projection_runtime(now: OffsetDateTime) -> OpenProjectionRuntime {
    OpenProjectionRuntime {
        now,
        audience: OpenCounterAudience::Public,
        scheduler: OpenSchedulerStatusV1 {
            lane: OpenSchedulerLane::ActiveWatch,
            requested_general_interval_seconds: PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS,
            requested_effective_interval_seconds: PUBLIC_WATCHED_OPEN_INTERVAL_SECONDS,
            active_watch_count: 1,
            next_due_at: None,
            in_flight: false,
            scheduler_lag_milliseconds: 0,
            actual_start_to_start_interval_milliseconds: None,
            failure_streak: 0,
        },
        circuit: OpenCircuitStatusV1 {
            state: OpenCircuitState::Closed,
            reason: None,
            opened_at: None,
            retry_at: None,
            diagnostic_recheck_required: false,
        },
    }
}

fn admission_from_projection(
    projection: Result<Option<bcsp_contracts::OpenObservationV1>, OpenProjectionError>,
) -> WatchStartAdmission {
    match projection {
        Ok(observation) => WatchStartAdmission::admitted(observation),
        Err(OpenProjectionError::SectionNotPublished) => WatchStartAdmission::SectionNotFound,
        Err(_) => WatchStartAdmission::TargetUnavailable,
    }
}

pub fn create_public_watch_socket(
    store: SharedPublicOperationalStore,
) -> Result<Arc<SharedWatchSocket>, WatchManagerError> {
    let admission: Arc<dyn WatchAdmissionSource> = Arc::new(PublicWatchAdmission { store });
    Ok(Arc::new(SharedWatchSocket::try_new(
        admission,
        Arc::new(NoopWatchDispatchSink),
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_keeps_missing_current_distinct_from_missing_section_or_target() {
        assert_eq!(
            admission_from_projection(Ok(None)),
            WatchStartAdmission::admitted(None)
        );
        assert_eq!(
            admission_from_projection(Err(OpenProjectionError::SectionNotPublished)),
            WatchStartAdmission::SectionNotFound
        );
        assert_eq!(
            admission_from_projection(Err(OpenProjectionError::TargetNotPublished)),
            WatchStartAdmission::TargetUnavailable
        );
    }

    #[test]
    fn watch_projection_uses_fixed_public_intervals_and_service_counter_scope() {
        let runtime = watch_projection_runtime(OffsetDateTime::UNIX_EPOCH);
        assert_eq!(runtime.audience, OpenCounterAudience::Public);
        assert_eq!(
            runtime.scheduler.requested_general_interval_seconds,
            PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS
        );
        assert_eq!(
            runtime.scheduler.requested_effective_interval_seconds,
            PUBLIC_WATCHED_OPEN_INTERVAL_SECONDS
        );
    }
}
