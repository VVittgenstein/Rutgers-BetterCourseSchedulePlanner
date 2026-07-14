use std::sync::{Arc, Mutex};

use bcsp_application::{OpenRuntimeSnapshotRegistry, SharedWatchSocket, WatchAdmissionSource};
use bcsp_contracts::SectionKey;
use bcsp_open::{OpenProjectionError, project_current_open_observation};
use bcsp_watch::{WatchManagerError, WatchStartAdmission};

use crate::{LocalPrimaryDatabase, LocalRuntimeCore, history::LocalWatchHistorySink};

struct LocalWatchAdmission {
    database: Arc<Mutex<LocalPrimaryDatabase>>,
    runtime: LocalRuntimeCore,
    open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
}

impl WatchAdmissionSource for LocalWatchAdmission {
    fn admission_for(&self, section: &SectionKey) -> WatchStartAdmission {
        let target = section.target();
        let Ok(snapshot) = self.open_runtime.snapshot(&target) else {
            return WatchStartAdmission::TargetUnavailable;
        };
        let Ok(runtime) = self.runtime.projection_runtime(&snapshot) else {
            return WatchStartAdmission::TargetUnavailable;
        };
        let Ok(mut database) = self.database.lock() else {
            return WatchStartAdmission::TargetUnavailable;
        };
        admission_from_projection(project_current_open_observation(
            database.operational_mut(),
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

pub fn create_local_watch_socket(
    database: Arc<Mutex<LocalPrimaryDatabase>>,
    runtime: LocalRuntimeCore,
    open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
) -> Result<Arc<SharedWatchSocket>, WatchManagerError> {
    let admission: Arc<dyn WatchAdmissionSource> = Arc::new(LocalWatchAdmission {
        database: database.clone(),
        runtime,
        open_runtime,
    });
    Ok(Arc::new(SharedWatchSocket::try_new(
        admission,
        Arc::new(LocalWatchHistorySink::new(database)),
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admission_distinguishes_missing_section_and_missing_target() {
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
}
