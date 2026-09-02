use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bcsp_application::{
    RefreshPolicy, RefreshPolicyProvider, RefreshPolicyReadError, SharedRuntimeContext,
    SystemApplicationClock,
};
use bcsp_contracts::{SystemTraceIdSource, TraceIdSource};
use bcsp_local_user_state::{PersonalStateError, PersonalStateStore};
use bcsp_open::{
    GeneralOpenInterval, OpenCounterAudience, OpenRefreshIntervals, WatchOpenInterval,
};

/// Reads the refresh policy from its OWN SQLite connection.
///
/// Every product route, the personal surface and watch admission serialize on
/// the `LocalPrimaryDatabase` mutex, and a single Open-status projection can
/// hold it for the better part of a second. The refresh coordinator, the
/// prepared rebuild worker and the Open-status routes all read the policy
/// before doing their work; giving that read a dedicated connection means
/// none of them queue behind the HTTP surface, and -- the reason this exists
/// -- the prepared rebuild that clears a committed publication barrier no
/// longer waits on the very mutex the parked requests will need next.
///
/// The read stays live rather than cached: a settings write through ANY
/// connection (the settings route, a user-data reset, a test writing through
/// the primary store) is visible on the next call.
#[derive(Clone)]
pub struct LocalRefreshPolicyProvider {
    store: Arc<Mutex<PersonalStateStore>>,
}

impl LocalRefreshPolicyProvider {
    /// Opens the dedicated policy connection against the package database.
    pub fn open(database: impl AsRef<Path>) -> Result<Self, PersonalStateError> {
        PersonalStateStore::open(database).map(Self::new)
    }

    pub fn new(store: PersonalStateStore) -> Self {
        Self {
            store: Arc::new(Mutex::new(store)),
        }
    }
}

impl RefreshPolicyProvider for LocalRefreshPolicyProvider {
    fn refresh_policy(&self) -> Result<RefreshPolicy, RefreshPolicyReadError> {
        let settings = self
            .store
            .lock()
            .map_err(|_| RefreshPolicyReadError)?
            .settings()
            .map_err(|_| RefreshPolicyReadError)?
            .value;
        let catalog_interval =
            Duration::from_secs(u64::from(settings.catalog_refresh_minutes.get()) * 60);
        let open_interval =
            GeneralOpenInterval::local(u32::from(settings.open_refresh_seconds.get()))
                .map_err(|_| RefreshPolicyReadError)?;
        let watch_interval =
            WatchOpenInterval::local(u32::from(settings.watch_fast_lane_seconds.get()))
                .map_err(|_| RefreshPolicyReadError)?;
        RefreshPolicy::try_new_with_intervals(
            catalog_interval,
            OpenRefreshIntervals::new(open_interval, watch_interval),
        )
        .map_err(|_| RefreshPolicyReadError)
    }
}

pub type LocalRuntimeCore =
    SharedRuntimeContext<SystemApplicationClock, LocalRefreshPolicyProvider>;

pub fn create_local_runtime_core(policy: LocalRefreshPolicyProvider) -> LocalRuntimeCore {
    let mut ids = SystemTraceIdSource;
    SharedRuntimeContext::new(
        OpenCounterAudience::Local {
            run_id: ids.next_trace_id(),
        },
        SystemApplicationClock,
        policy,
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use bcsp_application::RefreshPolicyProvider;
    use bcsp_local_user_state::{
        CatalogRefreshMinutes, LocalSettings, OpenRefreshSeconds, SettingsRevision,
        WatchFastLaneSeconds,
    };

    use super::LocalRefreshPolicyProvider;
    use crate::{LocalRuntimePaths, OperationalGate};

    #[test]
    fn refresh_policy_does_not_block_while_the_primary_database_is_locked() {
        let temp = tempfile::tempdir().expect("temp package");
        let root = temp.path().join("RBCSP");
        fs::create_dir_all(&root).expect("package root");
        let executable = root.join("RBCSP.exe");
        fs::write(&executable, b"test executable").expect("fake executable");
        let paths = LocalRuntimePaths::from_executable(&executable).expect("package paths");
        let gate = OperationalGate::open(paths).expect("operational gate");
        let provider =
            LocalRefreshPolicyProvider::open(gate.paths().database()).expect("policy store");

        let database = gate.database();
        let mut primary = database.lock().expect("hold the primary database");
        let edited = LocalSettings {
            catalog_refresh_minutes: CatalogRefreshMinutes::try_from(1).unwrap(),
            open_refresh_seconds: OpenRefreshSeconds::try_from(3).unwrap(),
            watch_fast_lane_seconds: WatchFastLaneSeconds::try_from(60).unwrap(),
            ..LocalSettings::default()
        };
        let state_revision = primary.personal().user_state_revision().unwrap();
        primary
            .personal_mut()
            .compare_and_swap_settings(state_revision, SettingsRevision::ZERO, &edited)
            .expect("write settings through the primary connection");

        // The primary mutex stays held for the whole read: the provider must
        // neither wait for it nor serve a stale value written before it.
        let (sender, receiver) = mpsc::channel();
        let reader = std::thread::spawn(move || {
            let started = Instant::now();
            let policy = provider.refresh_policy();
            sender
                .send((policy, started.elapsed()))
                .expect("publish the policy read");
        });
        let (policy, elapsed) = receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("refresh_policy() must not queue behind LocalPrimaryDatabase");
        reader.join().expect("policy reader thread");
        let policy = policy.expect("live policy");
        assert_eq!(policy.catalog_interval(), Duration::from_secs(60));
        assert_eq!(
            policy.effective_open_interval(false),
            Duration::from_secs(3)
        );
        assert!(
            elapsed < Duration::from_millis(500),
            "policy read took {elapsed:?} while the primary mutex was held"
        );
        drop(primary);
    }
}
