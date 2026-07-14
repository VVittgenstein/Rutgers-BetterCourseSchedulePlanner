use std::ops::{Deref, DerefMut};
use std::sync::{Arc, MutexGuard};

use bcsp_application::{
    FixedRefreshPolicyProvider, OpenRuntimeSnapshotRegistry, ProductStorageAccess,
    ProductStorageLockError, RefreshPolicyError, SharedProductRoutes, SharedRuntimeContext,
    SystemApplicationClock,
};
use bcsp_open::OpenCounterAudience;
use bcsp_operational_storage::OperationalStorage;
use bcsp_public_operations::PublicOperationalStore;

use crate::{SharedPublicOperationalStore, fixed_public_refresh_policy};

#[derive(Clone)]
pub struct PublicProductStorageAccess {
    store: SharedPublicOperationalStore,
}

impl PublicProductStorageAccess {
    pub const fn new(store: SharedPublicOperationalStore) -> Self {
        Self { store }
    }

    pub fn store(&self) -> SharedPublicOperationalStore {
        self.store.clone()
    }
}

pub struct PublicProductStorageGuard<'a>(MutexGuard<'a, PublicOperationalStore>);

impl Deref for PublicProductStorageGuard<'_> {
    type Target = OperationalStorage;

    fn deref(&self) -> &Self::Target {
        self.0.storage()
    }
}

impl DerefMut for PublicProductStorageGuard<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.storage_mut()
    }
}

impl ProductStorageAccess for PublicProductStorageAccess {
    type Guard<'a> = PublicProductStorageGuard<'a>;

    fn lock_operational(&self) -> Result<Self::Guard<'_>, ProductStorageLockError> {
        self.store
            .lock()
            .map(PublicProductStorageGuard)
            .map_err(|_| ProductStorageLockError)
    }
}

pub type PublicProductRoutes = SharedProductRoutes<
    SystemApplicationClock,
    FixedRefreshPolicyProvider,
    PublicProductStorageAccess,
>;

pub fn create_public_product_routes(
    store: SharedPublicOperationalStore,
    open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
) -> Result<PublicProductRoutes, RefreshPolicyError> {
    let runtime = SharedRuntimeContext::new(
        OpenCounterAudience::Public,
        SystemApplicationClock,
        FixedRefreshPolicyProvider::new(fixed_public_refresh_policy()?),
    );
    Ok(SharedProductRoutes::with_storage_access(
        PublicProductStorageAccess::new(store),
        runtime,
        open_runtime,
    ))
}
