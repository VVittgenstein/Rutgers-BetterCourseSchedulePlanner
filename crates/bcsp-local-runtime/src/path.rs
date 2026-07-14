use std::path::{Path, PathBuf};

use thiserror::Error;

pub const LOCAL_DATA_DIRECTORY_NAME: &str = "data";
pub const LOCAL_DATABASE_FILE_NAME: &str = "rbcsp.sqlite";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalRuntimePaths {
    executable: PathBuf,
    package_root: PathBuf,
    data_directory: PathBuf,
    database: PathBuf,
}

impl LocalRuntimePaths {
    pub fn from_executable(executable: impl AsRef<Path>) -> Result<Self, LocalPathError> {
        let executable = executable
            .as_ref()
            .canonicalize()
            .map_err(LocalPathError::CanonicalizeExecutable)?;
        if !executable.is_file() {
            return Err(LocalPathError::ExecutableIsNotFile(executable));
        }
        let package_root = executable
            .parent()
            .ok_or_else(|| LocalPathError::MissingPackageRoot(executable.clone()))?
            .to_path_buf();
        let data_directory = package_root.join(LOCAL_DATA_DIRECTORY_NAME);
        let database = data_directory.join(LOCAL_DATABASE_FILE_NAME);
        Ok(Self {
            executable,
            package_root,
            data_directory,
            database,
        })
    }

    pub fn for_current_executable() -> Result<Self, LocalPathError> {
        let executable = std::env::current_exe().map_err(LocalPathError::CurrentExecutable)?;
        Self::from_executable(executable)
    }

    pub fn executable(&self) -> &Path {
        &self.executable
    }

    pub fn package_root(&self) -> &Path {
        &self.package_root
    }

    pub fn data_directory(&self) -> &Path {
        &self.data_directory
    }

    pub fn database(&self) -> &Path {
        &self.database
    }
}

#[derive(Debug, Error)]
pub enum LocalPathError {
    #[error("failed to resolve the current executable")]
    CurrentExecutable(#[source] std::io::Error),
    #[error("failed to canonicalize the executable path")]
    CanonicalizeExecutable(#[source] std::io::Error),
    #[error("the executable path is not a file: {0}")]
    ExecutableIsNotFile(PathBuf),
    #[error("the executable path has no package root: {0}")]
    MissingPackageRoot(PathBuf),
}
