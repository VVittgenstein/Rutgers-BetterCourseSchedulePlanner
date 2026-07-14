//! Linux-public ownership of the service-wide operational SQLite database.

#![forbid(unsafe_code)]
#![deny(warnings)]

use std::path::{Path, PathBuf};

use bcsp_operational_storage::{OpenAttemptCounters, OperationalStorage, StorageError};
use thiserror::Error;

pub const PACKAGE_BOUNDARY: &str = "bcsp-public-operations";
pub const PUBLIC_STATE_ROOT: &str = "/var/lib/bcsp";
pub const PUBLIC_DATABASE_FILENAME: &str = "rbcsp.sqlite";
pub const PUBLIC_DATABASE_PATH: &str = "/var/lib/bcsp/rbcsp.sqlite";

/// The one service-owned operational database used by `bcsp-server`.
///
/// The wrapper creates only the state directory and the shared operational
/// schema. It has no dependency on, migration for, or repository representing
/// local personal state.
pub struct PublicOperationalStore {
    state_root: PathBuf,
    database_path: PathBuf,
    storage: OperationalStorage,
}

impl PublicOperationalStore {
    /// Opens the canonical production database without consulting the process
    /// working directory, environment variables, or a fallback location.
    pub fn open_production() -> Result<Self, PublicOperationsError> {
        Self::open_for_state_root(PUBLIC_STATE_ROOT)
    }

    /// Opens a public operational database below an explicit absolute state root.
    ///
    /// Production uses [`PUBLIC_STATE_ROOT`]. The explicit form lets disposable
    /// hosts and tests exercise first-start and restart behavior without writing
    /// to the production location. It deliberately rejects relative paths so a
    /// changed working directory can never redirect service state.
    pub fn open_for_state_root(
        state_root: impl AsRef<Path>,
    ) -> Result<Self, PublicOperationsError> {
        let state_root = state_root.as_ref();
        if !state_root.is_absolute() {
            return Err(PublicOperationsError::RelativeStateRoot);
        }

        std::fs::create_dir_all(state_root)
            .map_err(|source| PublicOperationsError::CreateStateRoot { source })?;
        let state_root = state_root.to_path_buf();
        let database_path = state_root.join(PUBLIC_DATABASE_FILENAME);
        let storage = OperationalStorage::open(&database_path)
            .map_err(|source| PublicOperationsError::OpenDatabase { source })?;

        Ok(Self {
            state_root,
            database_path,
            storage,
        })
    }

    pub fn state_root(&self) -> &Path {
        &self.state_root
    }

    pub fn database_path(&self) -> &Path {
        &self.database_path
    }

    pub const fn storage(&self) -> &OperationalStorage {
        &self.storage
    }

    pub const fn storage_mut(&mut self) -> &mut OperationalStorage {
        &mut self.storage
    }

    /// Reads the durable, service-wide aggregate for one Rutgers calendar day.
    /// This intentionally does not project per-process run counters as public
    /// product history.
    pub fn service_day_counters(
        &self,
        rutgers_day: &str,
    ) -> Result<OpenAttemptCounters, PublicOperationsError> {
        self.storage
            .open_service_day_counters(rutgers_day)
            .map_err(|source| PublicOperationsError::ReadServiceState { source })
    }
}

/// Startup/read failures with safe user- and log-facing messages.
///
/// The original error remains available through `Error::source` for controlled
/// diagnostics, while `Display` never includes a state path, SQL, or stored data.
#[derive(Debug, Error)]
pub enum PublicOperationsError {
    #[error("public operational state root must be an absolute path")]
    RelativeStateRoot,
    #[error("public operational state root could not be created")]
    CreateStateRoot {
        #[source]
        source: std::io::Error,
    },
    #[error("public operational database could not be opened")]
    OpenDatabase {
        #[source]
        source: StorageError,
    },
    #[error("public operational service state could not be read")]
    ReadServiceState {
        #[source]
        source: StorageError,
    },
}

impl PublicOperationsError {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::RelativeStateRoot => "PUBLIC_STATE_ROOT_INVALID",
            Self::CreateStateRoot { .. } => "PUBLIC_STATE_ROOT_UNAVAILABLE",
            Self::OpenDatabase { .. } => "PUBLIC_OPERATIONAL_DATABASE_UNAVAILABLE",
            Self::ReadServiceState { .. } => "PUBLIC_OPERATIONAL_STATE_UNAVAILABLE",
        }
    }
}

pub fn boundary_marker() -> &'static str {
    let _ = bcsp_operational_storage::PACKAGE_BOUNDARY;
    PACKAGE_BOUNDARY
}

mod dependency_contract {
    use rusqlite as _;
    use tracing as _;
}
