use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use bcsp_application::{RederivationError, rederive_stored_delivery_now};
use bcsp_local_user_state::{PersonalStateError, PersonalStateStore};
use bcsp_operational_storage::OperationalStorage;
use thiserror::Error;

use crate::path::{LOCAL_DATA_DIRECTORY_NAME, LocalRuntimePaths};

static NEXT_PROBE_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LocalRuntimeState {
    pub active_watch_count: u64,
}

pub struct OperationalGate {
    paths: LocalRuntimePaths,
    database: Arc<Mutex<LocalPrimaryDatabase>>,
    refresh_storage: Arc<Mutex<OperationalStorage>>,
}

/// The single package-local SQLite host.
///
/// Operational and personal repositories keep separate SQL ownership while this
/// host guarantees one physical package-relative database. The product-serving
/// and refresh connections are intentionally distinct: SQLite WAL preserves the
/// last published snapshot for UI reads while a replacement snapshot is written.
pub struct LocalPrimaryDatabase {
    operational: OperationalStorage,
    personal: PersonalStateStore,
}

impl LocalPrimaryDatabase {
    pub const fn operational(&self) -> &OperationalStorage {
        &self.operational
    }

    pub fn operational_mut(&mut self) -> &mut OperationalStorage {
        &mut self.operational
    }

    pub const fn personal(&self) -> &PersonalStateStore {
        &self.personal
    }

    pub fn personal_mut(&mut self) -> &mut PersonalStateStore {
        &mut self.personal
    }
}

impl OperationalGate {
    pub fn open(paths: LocalRuntimePaths) -> Result<Self, LocalBootstrapError> {
        let canonical_data = prepare_data_directory(&paths)?;
        verify_data_directory_writable(paths.data_directory())?;
        verify_database_target_before_open(paths.database(), &canonical_data)?;
        let mut operational = OperationalStorage::open(paths.database())
            .map_err(LocalBootstrapError::OperationalStorage)?;
        // Stored derived delivery columns must match this binary's derivation
        // rule before any prepared-serving build or refresh runtime reads
        // them; the projection rejects rows written under another rule.
        rederive_stored_delivery_now(&mut operational)
            .map_err(LocalBootstrapError::CatalogDerivation)?;
        let personal = PersonalStateStore::open(paths.database())
            .map_err(LocalBootstrapError::PersonalState)?;
        let refresh_storage = OperationalStorage::open(paths.database())
            .map_err(LocalBootstrapError::OperationalStorage)?;
        verify_database_target_after_open(paths.database(), &canonical_data)?;
        Ok(Self {
            paths,
            database: Arc::new(Mutex::new(LocalPrimaryDatabase {
                operational,
                personal,
            })),
            refresh_storage: Arc::new(Mutex::new(refresh_storage)),
        })
    }

    pub const fn paths(&self) -> &LocalRuntimePaths {
        &self.paths
    }

    pub fn database(&self) -> Arc<Mutex<LocalPrimaryDatabase>> {
        self.database.clone()
    }

    pub fn refresh_storage(&self) -> Arc<Mutex<OperationalStorage>> {
        self.refresh_storage.clone()
    }
}

fn prepare_data_directory(paths: &LocalRuntimePaths) -> Result<PathBuf, LocalBootstrapError> {
    fs::create_dir_all(paths.data_directory()).map_err(LocalBootstrapError::CreateDataDirectory)?;
    let canonical_data = paths
        .data_directory()
        .canonicalize()
        .map_err(LocalBootstrapError::CanonicalizeDataDirectory)?;
    if canonical_data.parent() != Some(paths.package_root())
        || canonical_data.file_name().and_then(|name| name.to_str())
            != Some(LOCAL_DATA_DIRECTORY_NAME)
    {
        return Err(LocalBootstrapError::DataDirectoryEscapesPackageRoot {
            expected: paths.data_directory().to_path_buf(),
            actual: canonical_data,
        });
    }
    Ok(canonical_data)
}

fn verify_database_target_before_open(
    database: &Path,
    canonical_data: &Path,
) -> Result<(), LocalBootstrapError> {
    let metadata = match fs::symlink_metadata(database) {
        Ok(metadata) => metadata,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(source) => return Err(LocalBootstrapError::InspectDatabaseTarget(source)),
    };

    if !metadata.file_type().is_file() {
        return Err(LocalBootstrapError::InvalidDatabaseTarget(
            database.to_path_buf(),
        ));
    }

    verify_canonical_database_parent(database, canonical_data)
}

fn verify_database_target_after_open(
    database: &Path,
    canonical_data: &Path,
) -> Result<(), LocalBootstrapError> {
    verify_canonical_database_parent(database, canonical_data)
}

fn verify_canonical_database_parent(
    database: &Path,
    canonical_data: &Path,
) -> Result<(), LocalBootstrapError> {
    let canonical_database = database
        .canonicalize()
        .map_err(LocalBootstrapError::CanonicalizeDatabase)?;
    if canonical_database.parent() != Some(canonical_data) || !canonical_database.is_file() {
        return Err(LocalBootstrapError::DatabaseEscapesDataDirectory {
            expected: database.to_path_buf(),
            actual: canonical_database,
        });
    }
    Ok(())
}

fn verify_data_directory_writable(data_directory: &Path) -> Result<(), LocalBootstrapError> {
    let id = NEXT_PROBE_ID.fetch_add(1, Ordering::Relaxed);
    let probe = data_directory.join(format!(".rbcsp-write-probe-{}-{id}", std::process::id()));
    let result = (|| -> std::io::Result<()> {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&probe)?;
        file.write_all(b"RBCSP_LOCAL_STORAGE_PROBE\n")?;
        file.flush()?;
        file.sync_all()?;
        drop(file);
        fs::remove_file(&probe)?;
        Ok(())
    })();
    if let Err(source) = result {
        let _ = fs::remove_file(&probe);
        return Err(LocalBootstrapError::WriteProbe {
            path: probe,
            source,
        });
    }
    Ok(())
}

#[derive(Debug, Error)]
pub enum LocalBootstrapError {
    #[error("failed to create the package-local data directory")]
    CreateDataDirectory(#[source] std::io::Error),
    #[error("failed to canonicalize the package-local data directory")]
    CanonicalizeDataDirectory(#[source] std::io::Error),
    #[error("the data directory escaped the package root: expected {expected}, got {actual}")]
    DataDirectoryEscapesPackageRoot { expected: PathBuf, actual: PathBuf },
    #[error("the package-local data directory is not writable: {path}")]
    WriteProbe {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to inspect the package-local database target")]
    InspectDatabaseTarget(#[source] std::io::Error),
    #[error("the package-local database target is not a regular file: {0}")]
    InvalidDatabaseTarget(PathBuf),
    #[error("failed to canonicalize the package-local database")]
    CanonicalizeDatabase(#[source] std::io::Error),
    #[error(
        "the database escaped its package-local data directory: expected {expected}, got {actual}"
    )]
    DatabaseEscapesDataDirectory { expected: PathBuf, actual: PathBuf },
    #[error("failed to open operational storage")]
    OperationalStorage(#[source] bcsp_operational_storage::StorageError),
    #[error("failed to re-derive the stored catalog delivery columns")]
    CatalogDerivation(#[source] RederivationError),
    #[error("failed to open personal state in the package-local database")]
    PersonalState(#[source] PersonalStateError),
}
