use std::sync::{Arc, Mutex};

use bcsp_application::{
    OpenRuntimeSnapshotRegistry, SharedWatchSocket, WatchAdmissionSource, is_product_campus,
};
use bcsp_contracts::SectionKey;
use bcsp_domain::{RutgersTermWindow, RutgersTermWindowScope};
use bcsp_local_user_state::PersonalStateStore;
use bcsp_open::{OpenProjectionError, project_current_open_observation};
use bcsp_watch::{WatchManagerError, WatchStartAdmission};
use time::OffsetDateTime;

use crate::{LocalPrimaryDatabase, LocalRuntimeCore, history::LocalWatchHistorySink};

struct LocalWatchAdmission {
    database: Arc<Mutex<LocalPrimaryDatabase>>,
    runtime: LocalRuntimeCore,
    open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
}

impl WatchAdmissionSource for LocalWatchAdmission {
    fn admission_for(&self, section: &SectionKey) -> WatchStartAdmission {
        if !self.target_supported(section) {
            return WatchStartAdmission::UnsupportedTarget;
        }
        if !self.term_in_range(section) {
            return WatchStartAdmission::TermOutOfRange;
        }
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

    fn target_supported(&self, section: &SectionKey) -> bool {
        is_product_campus(section.campus().as_str())
    }

    fn term_in_range(&self, section: &SectionKey) -> bool {
        watch_term_in_range(section)
    }
}

#[cfg(test)]
fn watch_target_supported(section: &SectionKey) -> bool {
    is_product_campus(section.campus().as_str())
}

fn watch_term_in_range(section: &SectionKey) -> bool {
    RutgersTermWindow::at(OffsetDateTime::now_utc(), RutgersTermWindowScope::Public).is_ok_and(
        |window| section.term() == window.current_term() || section.term() == window.next_term(),
    )
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
    history_store: PersonalStateStore,
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
        Arc::new(LocalWatchHistorySink::new(history_store)),
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bcsp_contracts::SectionKey;

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

    #[test]
    fn watch_targets_are_limited_to_the_three_product_campuses() {
        for campus in ["NB", "NK", "CM"] {
            let section = SectionKey::try_new("92026", campus, "12345").expect("section");
            assert!(watch_target_supported(&section), "{campus}");
        }
        for campus in ["NWK", "CAM", "ONLINE_NB"] {
            let section = SectionKey::try_new("92026", campus, "12345").expect("section");
            assert!(!watch_target_supported(&section), "{campus}");
        }
    }
}
