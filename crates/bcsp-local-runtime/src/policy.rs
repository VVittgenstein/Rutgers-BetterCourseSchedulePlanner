use std::sync::{Arc, Mutex};
use std::time::Duration;

use bcsp_application::{
    RefreshPolicy, RefreshPolicyProvider, RefreshPolicyReadError, SharedRuntimeContext,
    SystemApplicationClock,
};
use bcsp_contracts::{SystemTraceIdSource, TraceIdSource};
use bcsp_open::{GeneralOpenInterval, OpenCounterAudience};

use crate::LocalPrimaryDatabase;

#[derive(Clone)]
pub struct LocalRefreshPolicyProvider {
    database: Arc<Mutex<LocalPrimaryDatabase>>,
}

impl LocalRefreshPolicyProvider {
    pub const fn new(database: Arc<Mutex<LocalPrimaryDatabase>>) -> Self {
        Self { database }
    }
}

impl RefreshPolicyProvider for LocalRefreshPolicyProvider {
    fn refresh_policy(&self) -> Result<RefreshPolicy, RefreshPolicyReadError> {
        let settings = self
            .database
            .lock()
            .map_err(|_| RefreshPolicyReadError)?
            .personal()
            .settings()
            .map_err(|_| RefreshPolicyReadError)?
            .value;
        let catalog_interval =
            Duration::from_secs(u64::from(settings.catalog_refresh_minutes.get()) * 60);
        let open_interval =
            GeneralOpenInterval::local(u32::from(settings.open_refresh_seconds.get()))
                .map_err(|_| RefreshPolicyReadError)?;
        RefreshPolicy::try_new(catalog_interval, open_interval).map_err(|_| RefreshPolicyReadError)
    }
}

pub type LocalRuntimeCore =
    SharedRuntimeContext<SystemApplicationClock, LocalRefreshPolicyProvider>;

pub fn create_local_runtime_core(database: Arc<Mutex<LocalPrimaryDatabase>>) -> LocalRuntimeCore {
    let mut ids = SystemTraceIdSource;
    SharedRuntimeContext::new(
        OpenCounterAudience::Local {
            run_id: ids.next_trace_id(),
        },
        SystemApplicationClock,
        LocalRefreshPolicyProvider::new(database),
    )
}
