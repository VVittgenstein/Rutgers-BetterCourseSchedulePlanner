use std::sync::{Arc, Mutex};

use bcsp_application::{SharedWatchSocket, WatchAdmissionSource};
use bcsp_contracts::SectionKey;
use bcsp_watch::{WatchManagerError, WatchStartAdmission};

use crate::{LocalPrimaryDatabase, history::LocalWatchHistorySink};

struct LocalWatchAdmission {
    database: Arc<Mutex<LocalPrimaryDatabase>>,
}

impl WatchAdmissionSource for LocalWatchAdmission {
    fn admission_for(&self, section: &SectionKey) -> WatchStartAdmission {
        let Ok(database) = self.database.lock() else {
            return WatchStartAdmission::TargetUnavailable;
        };
        let target = section.target();
        let published = match database.operational().target_state(&target) {
            Ok(Some(state)) => state.current_content_version > 0,
            Ok(None) | Err(_) => false,
        };
        if !published {
            return WatchStartAdmission::TargetUnavailable;
        }
        match database.operational().serving_section_keys(&target) {
            Ok(sections) if sections.contains(section) => WatchStartAdmission::admitted(None),
            Ok(_) => WatchStartAdmission::SectionNotFound,
            Err(_) => WatchStartAdmission::TargetUnavailable,
        }
    }
}

pub fn create_local_watch_socket(
    database: Arc<Mutex<LocalPrimaryDatabase>>,
) -> Result<Arc<SharedWatchSocket>, WatchManagerError> {
    let admission: Arc<dyn WatchAdmissionSource> = Arc::new(LocalWatchAdmission {
        database: database.clone(),
    });
    Ok(Arc::new(SharedWatchSocket::try_new(
        admission,
        Arc::new(LocalWatchHistorySink::new(database)),
    )?))
}
