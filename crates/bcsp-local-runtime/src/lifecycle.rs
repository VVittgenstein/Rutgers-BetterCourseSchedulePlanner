use std::future::Future;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use bcsp_application::{
    LoopbackServer, LoopbackServerError, OfficialRefreshRuntime, OfficialRefreshRuntimeBuildError,
    OpenRuntimeSnapshotRegistry, RouteExtension, SessionNonce, SharedWatchSocket,
    TargetRefreshDemand, WebSocketExtension, spawn_loopback_server_with_socket,
};
use thiserror::Error;
use tokio::sync::watch;

use crate::{
    ExistingLocalInstance, LocalBootstrapError, LocalInstanceClaim, LocalInstanceError,
    LocalPathError, LocalRouteExtension, LocalRuntimeCore, LocalRuntimePaths, LocalRuntimeState,
    LocalSurfaceFailure, OperationalGate, PersonalSurface, PrimaryInstanceLease,
    create_local_runtime_core, create_local_watch_socket,
    product::{create_local_product_routes, start_local_product_refresh},
};

const EXISTING_INSTANCE_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone)]
struct LocalShutdownTrigger {
    requested: watch::Sender<bool>,
}

impl LocalShutdownTrigger {
    fn request(&self) {
        self.requested.send_replace(true);
    }
}

struct LocalShutdownListener {
    requested: watch::Receiver<bool>,
}

impl LocalShutdownListener {
    async fn wait(&mut self) -> bool {
        if *self.requested.borrow_and_update() {
            return true;
        }
        while self.requested.changed().await.is_ok() {
            if *self.requested.borrow_and_update() {
                return true;
            }
        }
        false
    }
}

fn local_shutdown_channel() -> (LocalShutdownTrigger, LocalShutdownListener) {
    let (requested, receiver) = watch::channel(false);
    (
        LocalShutdownTrigger { requested },
        LocalShutdownListener {
            requested: receiver,
        },
    )
}

pub struct PreparedLocalRuntime {
    operational: OperationalGate,
    core: LocalRuntimeCore,
    open_runtime: Arc<OpenRuntimeSnapshotRegistry>,
    watch: Arc<SharedWatchSocket>,
    target_refresh_demand: TargetRefreshDemand,
    extension: Arc<LocalRouteExtension>,
    nonce: SessionNonce,
    shutdown_requests: LocalShutdownListener,
}

impl PreparedLocalRuntime {
    pub fn open(paths: LocalRuntimePaths) -> Result<Self, LocalRuntimeError> {
        let operational = OperationalGate::open(paths)?;
        let database = operational.database();
        let core = create_local_runtime_core(database.clone());
        let open_runtime = Arc::new(OpenRuntimeSnapshotRegistry::default());
        let target_refresh_demand = TargetRefreshDemand::default();
        let product_routes: Arc<dyn RouteExtension> = Arc::new(
            create_local_product_routes(database.clone(), core.clone(), open_runtime.clone())
                .with_target_refresh_demand(target_refresh_demand.clone()),
        );
        let watch =
            create_local_watch_socket(database.clone(), core.clone(), open_runtime.clone())?;
        let personal = PersonalSurface::new(database, watch.clone());
        let nonce = SessionNonce::generate();
        let (shutdown_trigger, shutdown_requests) = local_shutdown_channel();
        let extension = Arc::new(LocalRouteExtension::with_product_routes(
            nonce.clone(),
            Box::new(personal),
            move || shutdown_trigger.request(),
            product_routes,
        ));
        Ok(Self {
            operational,
            core,
            open_runtime,
            watch,
            target_refresh_demand,
            extension,
            nonce,
            shutdown_requests,
        })
    }

    pub fn from_executable(executable: impl AsRef<Path>) -> Result<Self, LocalRuntimeError> {
        Self::open(LocalRuntimePaths::from_executable(executable)?)
    }

    pub fn for_current_executable() -> Result<Self, LocalRuntimeError> {
        Self::open(LocalRuntimePaths::for_current_executable()?)
    }

    pub const fn paths(&self) -> &LocalRuntimePaths {
        self.operational.paths()
    }

    pub fn state(&self) -> LocalRuntimeState {
        LocalRuntimeState {
            active_watch_count: u64::try_from(self.watch.total_active_watch_count())
                .unwrap_or(u64::MAX),
        }
    }

    pub const fn operational(&self) -> &OperationalGate {
        &self.operational
    }

    pub fn operational_mut(&mut self) -> &mut OperationalGate {
        &mut self.operational
    }

    pub fn nonce(&self) -> &SessionNonce {
        &self.nonce
    }

    pub fn route_extension(&self) -> Arc<LocalRouteExtension> {
        self.extension.clone()
    }

    pub fn watch_socket(&self) -> Arc<SharedWatchSocket> {
        self.watch.clone()
    }

    pub const fn core(&self) -> &LocalRuntimeCore {
        &self.core
    }

    pub fn open_runtime(&self) -> Arc<OpenRuntimeSnapshotRegistry> {
        self.open_runtime.clone()
    }

    pub async fn start(self) -> Result<RunningLocalRuntime, LocalRuntimeError> {
        let refresh = start_local_product_refresh(
            self.operational.refresh_storage(),
            self.operational.database(),
            &self.core,
            self.watch.clone(),
            self.open_runtime.clone(),
            self.target_refresh_demand.clone(),
        )?;
        let extension: Arc<dyn RouteExtension> = self.extension.clone();
        let socket: Arc<dyn WebSocketExtension> = self.watch.clone();
        let server =
            spawn_loopback_server_with_socket(extension, socket, self.nonce.clone()).await?;
        Ok(RunningLocalRuntime {
            prepared: self,
            server,
            refresh,
        })
    }
}

pub async fn prepare_and_start_with<T, F, Fut>(
    paths: LocalRuntimePaths,
    start_network: F,
) -> Result<(PreparedLocalRuntime, T), LocalRuntimeError>
where
    F: FnOnce(Arc<dyn RouteExtension>, SessionNonce) -> Fut,
    Fut: Future<Output = Result<T, LocalRuntimeError>>,
{
    let prepared = PreparedLocalRuntime::open(paths)?;
    let extension: Arc<dyn RouteExtension> = prepared.extension.clone();
    let started = start_network(extension, prepared.nonce.clone()).await?;
    Ok((prepared, started))
}

pub struct RunningLocalRuntime {
    prepared: PreparedLocalRuntime,
    server: LoopbackServer,
    refresh: OfficialRefreshRuntime,
}

impl RunningLocalRuntime {
    pub fn browser_url(&self) -> String {
        self.server.browser_url()
    }

    pub fn origin(&self) -> &str {
        self.server.origin()
    }

    pub fn nonce(&self) -> &SessionNonce {
        self.server.nonce()
    }

    pub const fn prepared(&self) -> &PreparedLocalRuntime {
        &self.prepared
    }

    /// Waits until the authenticated local UI asks this runtime to exit.
    ///
    /// The request only signals intent. Call [`Self::shutdown`] afterwards so
    /// watch transport, the HTTP server, and SQLite are closed in order.
    pub async fn wait_for_local_exit_request(&mut self) -> Result<(), LocalRuntimeError> {
        if self.prepared.shutdown_requests.wait().await {
            Ok(())
        } else {
            Err(LocalRuntimeError::ShutdownRequestChannelClosed)
        }
    }

    pub async fn shutdown(self) -> Result<(), LocalRuntimeError> {
        let Self {
            prepared,
            server,
            refresh,
        } = self;
        refresh.shutdown().await;
        prepared.watch.stop();
        server.shutdown().await?;
        prepared.extension.checkpoint_wal()?;
        Ok(())
    }
}

pub async fn run() -> Result<(), LocalRuntimeError> {
    let paths = LocalRuntimePaths::for_current_executable()?;
    match LocalInstanceClaim::acquire(&paths)? {
        LocalInstanceClaim::Primary(lease) => run_primary(paths, lease).await,
        LocalInstanceClaim::Secondary(instance) => reuse_existing(instance),
    }
}

async fn run_primary(
    paths: LocalRuntimePaths,
    mut lease: PrimaryInstanceLease,
) -> Result<(), LocalRuntimeError> {
    let prepared = PreparedLocalRuntime::open(paths)?;
    let mut running = prepared.start().await?;
    let browser_url = running.browser_url();
    if let Err(source) = lease.publish_browser_url(&browser_url) {
        let _ = running.shutdown().await;
        return Err(source.into());
    }
    if let Err(source) = open::that(&browser_url) {
        let _ = running.shutdown().await;
        return Err(LocalRuntimeError::OpenBrowser {
            url: browser_url,
            source,
        });
    }
    let shutdown_request = wait_for_shutdown_request(&mut running).await;
    let shutdown = running.shutdown().await;
    match (shutdown_request, shutdown) {
        (_, Err(error)) => Err(error),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

#[cfg(not(windows))]
async fn wait_for_shutdown_request(
    running: &mut RunningLocalRuntime,
) -> Result<(), LocalRuntimeError> {
    tokio::select! {
        request = running.wait_for_local_exit_request() => request,
        signal = tokio::signal::ctrl_c() => signal.map_err(LocalRuntimeError::ShutdownSignal),
    }
}

#[cfg(windows)]
async fn wait_for_shutdown_request(
    running: &mut RunningLocalRuntime,
) -> Result<(), LocalRuntimeError> {
    let mut signals =
        WindowsShutdownSignals::install().map_err(LocalRuntimeError::ShutdownSignal)?;
    tokio::select! {
        request = running.wait_for_local_exit_request() => request,
        () = signals.recv() => Ok(()),
    }
}

#[cfg(windows)]
struct WindowsShutdownSignals {
    ctrl_c: tokio::signal::windows::CtrlC,
    ctrl_break: tokio::signal::windows::CtrlBreak,
    ctrl_close: tokio::signal::windows::CtrlClose,
    ctrl_logoff: tokio::signal::windows::CtrlLogoff,
    ctrl_shutdown: tokio::signal::windows::CtrlShutdown,
}

#[cfg(windows)]
impl WindowsShutdownSignals {
    fn install() -> Result<Self, std::io::Error> {
        Ok(Self {
            ctrl_c: tokio::signal::windows::ctrl_c()?,
            ctrl_break: tokio::signal::windows::ctrl_break()?,
            ctrl_close: tokio::signal::windows::ctrl_close()?,
            ctrl_logoff: tokio::signal::windows::ctrl_logoff()?,
            ctrl_shutdown: tokio::signal::windows::ctrl_shutdown()?,
        })
    }

    async fn recv(&mut self) {
        tokio::select! {
            _ = self.ctrl_c.recv() => {}
            _ = self.ctrl_break.recv() => {}
            _ = self.ctrl_close.recv() => {}
            _ = self.ctrl_logoff.recv() => {}
            _ = self.ctrl_shutdown.recv() => {}
        }
    }
}

pub fn run_blocking() -> Result<(), LocalRuntimeError> {
    let paths = LocalRuntimePaths::for_current_executable()?;
    match LocalInstanceClaim::acquire(&paths)? {
        LocalInstanceClaim::Secondary(instance) => reuse_existing(instance),
        LocalInstanceClaim::Primary(lease) => tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .map_err(LocalRuntimeError::RuntimeBuild)?
            .block_on(run_primary(paths, lease)),
    }
}

fn reuse_existing(instance: ExistingLocalInstance) -> Result<(), LocalRuntimeError> {
    let browser_url = instance.wait_for_healthy_browser_url(EXISTING_INSTANCE_DISCOVERY_TIMEOUT)?;
    open::that(browser_url.as_str()).map_err(|source| LocalRuntimeError::OpenBrowser {
        url: browser_url.to_string(),
        source,
    })
}

#[derive(Debug, Error)]
pub enum LocalRuntimeError {
    #[error(transparent)]
    Path(#[from] LocalPathError),
    #[error(transparent)]
    Bootstrap(#[from] LocalBootstrapError),
    #[error(transparent)]
    Instance(#[from] LocalInstanceError),
    #[error(transparent)]
    Loopback(#[from] LoopbackServerError),
    #[error("failed to initialize shared watch state")]
    Watch(#[from] bcsp_watch::WatchManagerError),
    #[error("failed to initialize official Rutgers refresh clients")]
    Refresh(#[from] OfficialRefreshRuntimeBuildError),
    #[error("failed to open the local browser URL {url}")]
    OpenBrowser {
        url: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to install or receive the shutdown signal")]
    ShutdownSignal(#[source] std::io::Error),
    #[error("the local UI shutdown request channel closed unexpectedly")]
    ShutdownRequestChannelClosed,
    #[error("failed to initialize the local async runtime")]
    RuntimeBuild(#[source] std::io::Error),
    #[error("failed to checkpoint local SQLite state")]
    Surface(#[from] LocalSurfaceFailure),
    #[error("injected network startup failed")]
    InjectedNetworkStart,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_request_is_not_lost_when_sent_before_the_waiter_starts() {
        let (trigger, mut listener) = local_shutdown_channel();
        trigger.request();
        assert!(listener.wait().await);
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn all_windows_console_shutdown_listeners_can_be_installed() {
        let _signals = WindowsShutdownSignals::install().unwrap();
    }
}
