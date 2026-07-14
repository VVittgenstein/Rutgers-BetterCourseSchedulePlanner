use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard};

use bcsp_application::{
    NoopCoordinatorStatusSink, OfficialRefreshRuntime, OfficialRefreshRuntimeBuildError,
    OpenRuntimeSnapshotRegistry, ProductStorageAccess, ProductStorageLockError,
    SharedProductRoutes, SharedWatchSocket, SystemApplicationClock, TargetRefreshDemand,
};
use bcsp_open::OpenCounterAudience;
use bcsp_operational_storage::OperationalStorage;

use crate::{LocalPrimaryDatabase, LocalRefreshPolicyProvider, LocalRuntimeCore};

#[derive(Clone)]
pub(crate) struct LocalProductStorageAccess {
    database: Arc<Mutex<LocalPrimaryDatabase>>,
}

impl LocalProductStorageAccess {
    pub(crate) const fn new(database: Arc<Mutex<LocalPrimaryDatabase>>) -> Self {
        Self { database }
    }
}

pub(crate) struct LocalProductStorageGuard<'a>(MutexGuard<'a, LocalPrimaryDatabase>);

impl Deref for LocalProductStorageGuard<'_> {
    type Target = OperationalStorage;

    fn deref(&self) -> &Self::Target {
        self.0.operational()
    }
}

impl DerefMut for LocalProductStorageGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.operational_mut()
    }
}

impl ProductStorageAccess for LocalProductStorageAccess {
    type Guard<'a> = LocalProductStorageGuard<'a>;

    fn lock_operational(&self) -> Result<Self::Guard<'_>, ProductStorageLockError> {
        self.database
            .lock()
            .map(LocalProductStorageGuard)
            .map_err(|_| ProductStorageLockError)
    }
}

pub(crate) type LocalProductRoutes = SharedProductRoutes<
    SystemApplicationClock,
    LocalRefreshPolicyProvider,
    LocalProductStorageAccess,
>;

pub(crate) fn create_local_product_routes(
    database: Arc<Mutex<LocalPrimaryDatabase>>,
    runtime: LocalRuntimeCore,
    open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
) -> LocalProductRoutes {
    SharedProductRoutes::with_storage_access(
        LocalProductStorageAccess::new(database),
        runtime,
        open_runtime,
    )
}

pub(crate) fn start_local_product_refresh(
    database: Arc<Mutex<LocalPrimaryDatabase>>,
    runtime: &LocalRuntimeCore,
    watch: Arc<SharedWatchSocket>,
    open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
    target_refresh_demand: TargetRefreshDemand,
) -> Result<OfficialRefreshRuntime, OfficialRefreshRuntimeBuildError> {
    let OpenCounterAudience::Local { run_id } = runtime.counter_audience() else {
        unreachable!("local runtime always owns a local counter audience");
    };
    OfficialRefreshRuntime::spawn_with_target_refresh_demand(
        LocalProductStorageAccess::new(database.clone()),
        LocalRefreshPolicyProvider::new(database),
        run_id,
        runtime.counter_audience(),
        watch,
        open_runtime,
        Arc::new(NoopCoordinatorStatusSink),
        target_refresh_demand,
    )
}
