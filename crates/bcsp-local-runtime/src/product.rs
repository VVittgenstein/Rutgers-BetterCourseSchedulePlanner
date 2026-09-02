use std::ops::{Deref, DerefMut};
use std::sync::{Arc, Mutex, MutexGuard};

use bcsp_application::{
    OfficialRefreshRuntime, OfficialRefreshRuntimeBuildError, OpenRuntimeSnapshotRegistry,
    PreparedServingRegistry, ProductStorageAccess, ProductStorageLockError, ServiceStatusRegistry,
    SharedProductRoutes, SharedProductStorage, SharedWatchSocket, StorageConnectionInventoryEntry,
    SystemApplicationClock, TargetRefreshDemand,
};
use bcsp_local_user_state::{PersonalStateError, PersonalStateStore, PersonalTransactionState};
use bcsp_open::OpenCounterAudience;
use bcsp_operational_storage::OperationalStorage;

use crate::history::LocalWatchHistorySink;
use crate::{
    DesiredWatchCoordinator, LocalPrimaryDatabase, LocalRefreshPolicyProvider, LocalRuntimeCore,
};

/// How long a local prepared route or personal-surface read waits for a
/// committed publication barrier to clear. The local build keeps the shared
/// default: one user, a large blocking pool, and a rebuild window that the
/// wait is meant to cover with margin.
pub const LOCAL_PREPARED_ADMISSION_WAIT: std::time::Duration =
    bcsp_application::PREPARED_REQUEST_ADMISSION_WAIT;

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

/// Every long-lived SQLite connection the local runtime owns, named.
///
/// This exists for one diagnostic: when the refresh writer's checkpoints
/// stop making progress, `WAL_CHECKPOINT_STARVED` reports the transaction
/// state of each of these so the log names the connection that is holding
/// the read transaction. Every lookup is a `try_lock`: a diagnostic that
/// waited behind a page's mutation would be worse than one that says
/// `LOCKED`.
pub(crate) struct LocalConnectionInventory {
    pub(crate) refresh_storage: SharedProductStorage,
    pub(crate) database: Arc<Mutex<LocalPrimaryDatabase>>,
    pub(crate) mutation_store: Arc<Mutex<PersonalStateStore>>,
    pub(crate) desired_watch: Arc<DesiredWatchCoordinator>,
    pub(crate) history: Arc<LocalWatchHistorySink>,
}

impl LocalConnectionInventory {
    pub(crate) fn inspect(&self) -> Vec<StorageConnectionInventoryEntry> {
        let mut entries = Vec::with_capacity(6);
        entries.push(match self.refresh_storage.try_lock() {
            Ok(storage) => StorageConnectionInventoryEntry::from_state(
                "refresh_operational",
                storage.transaction_state(),
            ),
            Err(_) => locked("refresh_operational"),
        });
        match self.database.try_lock() {
            Ok(database) => {
                entries.push(StorageConnectionInventoryEntry::from_state(
                    "serving_operational",
                    database.operational().transaction_state(),
                ));
                entries.push(personal_entry(
                    "serving_personal",
                    Some(database.personal().transaction_state()),
                ));
            }
            Err(_) => {
                entries.push(locked("serving_operational"));
                entries.push(locked("serving_personal"));
            }
        }
        entries.push(personal_entry(
            "mutation_personal",
            self.mutation_store
                .try_lock()
                .ok()
                .map(|store| store.transaction_state()),
        ));
        entries.push(personal_entry(
            "desired_watch_personal",
            self.desired_watch.store_transaction_state(),
        ));
        entries.push(personal_entry(
            "history_personal",
            self.history.transaction_state().map(Ok),
        ));
        entries
    }
}

fn locked(connection: &'static str) -> StorageConnectionInventoryEntry {
    StorageConnectionInventoryEntry::new(connection, StorageConnectionInventoryEntry::LOCKED)
}

fn personal_entry(
    connection: &'static str,
    state: Option<Result<PersonalTransactionState, PersonalStateError>>,
) -> StorageConnectionInventoryEntry {
    StorageConnectionInventoryEntry::new(
        connection,
        match state {
            Some(Ok(state)) => state.as_str(),
            Some(Err(_)) => StorageConnectionInventoryEntry::ERROR,
            None => StorageConnectionInventoryEntry::LOCKED,
        },
    )
}

/// The refresh writer's storage access: the dedicated refresh connection,
/// plus the inventory of everything else for the starvation diagnostic.
#[derive(Clone)]
pub(crate) struct LocalRefreshStorageAccess {
    refresh_storage: SharedProductStorage,
    inventory: Arc<LocalConnectionInventory>,
}

impl LocalRefreshStorageAccess {
    pub(crate) fn new(inventory: LocalConnectionInventory) -> Self {
        Self {
            refresh_storage: inventory.refresh_storage.clone(),
            inventory: Arc::new(inventory),
        }
    }
}

impl ProductStorageAccess for LocalRefreshStorageAccess {
    type Guard<'a> = MutexGuard<'a, OperationalStorage>;

    fn lock_operational(&self) -> Result<Self::Guard<'_>, ProductStorageLockError> {
        self.refresh_storage
            .lock()
            .map_err(|_| ProductStorageLockError)
    }

    fn connection_inventory(&self) -> Vec<StorageConnectionInventoryEntry> {
        self.inventory.inspect()
    }
}

pub(crate) type LocalProductRoutes = SharedProductRoutes<
    SystemApplicationClock,
    LocalRefreshPolicyProvider,
    LocalProductStorageAccess,
>;

pub(crate) struct LocalProductRefreshResources {
    pub(crate) watch: Arc<SharedWatchSocket>,
    pub(crate) open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
    pub(crate) service_status: Arc<ServiceStatusRegistry>,
    pub(crate) target_refresh_demand: TargetRefreshDemand,
    pub(crate) prepared_serving: Arc<PreparedServingRegistry>,
}

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
    refresh_storage: LocalRefreshStorageAccess,
    runtime: &LocalRuntimeCore,
    resources: LocalProductRefreshResources,
) -> Result<OfficialRefreshRuntime, OfficialRefreshRuntimeBuildError> {
    let OpenCounterAudience::Local { run_id } = runtime.counter_audience() else {
        unreachable!("local runtime always owns a local counter audience");
    };
    // The refresh side reads the policy through the same dedicated
    // connection as the routes: never through LocalPrimaryDatabase, which the
    // prepared rebuild worker must not queue behind.
    OfficialRefreshRuntime::spawn_with_target_refresh_demand_and_prepared(
        refresh_storage,
        runtime.refresh_policy_provider().clone(),
        run_id,
        runtime.counter_audience(),
        resources.watch,
        resources.open_runtime,
        resources.service_status,
        resources.target_refresh_demand,
        resources.prepared_serving,
    )
}
