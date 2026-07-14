use std::sync::Arc;

use bcsp_application::{
    FixedRefreshPolicyProvider, NoopWatchDispatchSink, OpenRuntimeSnapshotRegistry,
    SharedRuntimeContext, SharedWatchSocket, SystemApplicationClock, WatchAdmissionSource,
};
use bcsp_contracts::SectionKey;
use bcsp_open::{OpenCounterAudience, OpenProjectionError, project_current_open_observation};
use bcsp_watch::{WatchManagerError, WatchStartAdmission};

use crate::{SharedPublicOperationalStore, fixed_public_refresh_policy};

struct PublicWatchAdmission {
    store: SharedPublicOperationalStore,
    runtime: SharedRuntimeContext<SystemApplicationClock, FixedRefreshPolicyProvider>,
    open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
}

impl WatchAdmissionSource for PublicWatchAdmission {
    fn admission_for(&self, section: &SectionKey) -> WatchStartAdmission {
        let target = section.target();
        let Ok(snapshot) = self.open_runtime.snapshot(&target) else {
            return WatchStartAdmission::TargetUnavailable;
        };
        let Ok(runtime) = self.runtime.projection_runtime(&snapshot) else {
            return WatchStartAdmission::TargetUnavailable;
        };
        let Ok(mut store) = self.store.lock() else {
            return WatchStartAdmission::TargetUnavailable;
        };
        admission_from_projection(project_current_open_observation(
            store.storage_mut(),
            section,
            &runtime,
        ))
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
    open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
) -> Result<Arc<SharedWatchSocket>, WatchManagerError> {
    let runtime = SharedRuntimeContext::new(
        OpenCounterAudience::Public,
        SystemApplicationClock,
        FixedRefreshPolicyProvider::new(
            fixed_public_refresh_policy().expect("fixed public refresh policy is valid"),
        ),
    );
    let admission: Arc<dyn WatchAdmissionSource> = Arc::new(PublicWatchAdmission {
        store,
        runtime,
        open_runtime,
    });
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
        let runtime = SharedRuntimeContext::new(
            OpenCounterAudience::Public,
            SystemApplicationClock,
            FixedRefreshPolicyProvider::new(
                fixed_public_refresh_policy().expect("fixed public policy"),
            ),
        )
        .projection_runtime(&Default::default())
        .expect("public watch projection");
        assert_eq!(runtime.audience, OpenCounterAudience::Public);
        assert_eq!(
            runtime.scheduler.requested_general_interval_seconds,
            crate::PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS
        );
        assert_eq!(
            runtime.scheduler.requested_effective_interval_seconds,
            crate::PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS
        );
    }
}
