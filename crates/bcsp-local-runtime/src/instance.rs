use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use thiserror::Error;

use crate::{LOCAL_DATA_DIRECTORY_NAME, LocalRuntimePaths};

pub const LOCAL_INSTANCE_LOCK_FILE_NAME: &str = "rbcsp.instance.lock";

const MAX_LOCK_CONTENT_BYTES: u64 = 128;
const HEALTH_CONNECT_TIMEOUT: Duration = Duration::from_millis(250);
const HEALTH_IO_TIMEOUT: Duration = Duration::from_millis(500);
const DISCOVERY_RETRY_INTERVAL: Duration = Duration::from_millis(50);

/// The result of atomically claiming the package-local process slot.
///
/// A secondary process gets no storage or scheduler handles. It can only read
/// and health-check the URL published by the process that owns the lock.
pub enum LocalInstanceClaim {
    Primary(PrimaryInstanceLease),
    Secondary(ExistingLocalInstance),
}

impl LocalInstanceClaim {
    pub fn acquire(paths: &LocalRuntimePaths) -> Result<Self, LocalInstanceError> {
        let lock_path = checked_lock_path(paths)?;
        match try_acquire_primary(&lock_path)? {
            Some(file) => Ok(Self::Primary(PrimaryInstanceLease {
                file: Some(file),
                lock_path,
            })),
            None => Ok(Self::Secondary(ExistingLocalInstance { lock_path })),
        }
    }
}

/// An OS-owned first-instance lease.
///
/// On Windows, the open handle denies other writers and the operating system
/// releases that ownership if the process crashes. The file itself is only a
/// small rendezvous record and is not the source of ownership.
pub struct PrimaryInstanceLease {
    file: Option<File>,
    lock_path: PathBuf,
}

impl PrimaryInstanceLease {
    pub fn publish_browser_url(
        &mut self,
        browser_url: &str,
    ) -> Result<LoopbackBrowserUrl, LocalInstanceError> {
        let browser_url = LoopbackBrowserUrl::parse(browser_url)?;
        let file = self
            .file
            .as_mut()
            .ok_or(LocalInstanceError::LeaseReleased)?;
        file.set_len(0)
            .map_err(LocalInstanceError::WriteLockRecord)?;
        file.write_all(browser_url.as_str().as_bytes())
            .and_then(|()| file.write_all(b"\n"))
            .and_then(|()| file.flush())
            .and_then(|()| file.sync_all())
            .map_err(LocalInstanceError::WriteLockRecord)?;
        Ok(browser_url)
    }

    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }
}

impl Drop for PrimaryInstanceLease {
    fn drop(&mut self) {
        drop(self.file.take());
        let _ = fs::remove_file(&self.lock_path);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExistingLocalInstance {
    lock_path: PathBuf,
}

impl ExistingLocalInstance {
    pub fn wait_for_healthy_browser_url(
        &self,
        timeout: Duration,
    ) -> Result<LoopbackBrowserUrl, LocalInstanceError> {
        let started = Instant::now();
        loop {
            if let Ok(browser_url) = read_browser_url(&self.lock_path)
                && browser_url.is_healthy()
            {
                return Ok(browser_url);
            }
            if started.elapsed() >= timeout {
                return Err(LocalInstanceError::ExistingInstanceUnavailable);
            }
            thread::sleep(DISCOVERY_RETRY_INTERVAL.min(timeout.saturating_sub(started.elapsed())));
        }
    }

    pub fn lock_path(&self) -> &Path {
        &self.lock_path
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopbackBrowserUrl {
    value: String,
    port: u16,
}

impl LoopbackBrowserUrl {
    pub fn parse(value: &str) -> Result<Self, LocalInstanceError> {
        let port_text = value
            .strip_prefix("http://127.0.0.1:")
            .and_then(|value| value.strip_suffix('/'))
            .ok_or(LocalInstanceError::InvalidBrowserUrl)?;
        if port_text.is_empty()
            || !port_text.bytes().all(|value| value.is_ascii_digit())
            || (port_text.len() > 1 && port_text.starts_with('0'))
        {
            return Err(LocalInstanceError::InvalidBrowserUrl);
        }
        let port = port_text
            .parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or(LocalInstanceError::InvalidBrowserUrl)?;
        if value != format!("http://127.0.0.1:{port}/") {
            return Err(LocalInstanceError::InvalidBrowserUrl);
        }
        Ok(Self {
            value: value.to_owned(),
            port,
        })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub const fn port(&self) -> u16 {
        self.port
    }

    fn is_healthy(&self) -> bool {
        let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, self.port);
        let Ok(mut stream) = TcpStream::connect_timeout(&address.into(), HEALTH_CONNECT_TIMEOUT)
        else {
            return false;
        };
        if stream.set_read_timeout(Some(HEALTH_IO_TIMEOUT)).is_err()
            || stream.set_write_timeout(Some(HEALTH_IO_TIMEOUT)).is_err()
        {
            return false;
        }
        let request = format!(
            "GET / HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nConnection: close\r\n\r\n",
            self.port
        );
        if stream.write_all(request.as_bytes()).is_err() || stream.flush().is_err() {
            return false;
        }
        let mut prefix = [0_u8; 17];
        let mut read = 0;
        while read < prefix.len() {
            match stream.read(&mut prefix[read..]) {
                Ok(0) => break,
                Ok(count) => read += count,
                Err(_) => return false,
            }
        }
        prefix.get(..12) == Some(b"HTTP/1.1 200")
    }
}

impl std::fmt::Display for LoopbackBrowserUrl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

fn checked_lock_path(paths: &LocalRuntimePaths) -> Result<PathBuf, LocalInstanceError> {
    fs::create_dir_all(paths.data_directory()).map_err(LocalInstanceError::CreateDataDirectory)?;
    let canonical_data = paths
        .data_directory()
        .canonicalize()
        .map_err(LocalInstanceError::CanonicalizeDataDirectory)?;
    if canonical_data.parent() != Some(paths.package_root())
        || canonical_data.file_name().and_then(|name| name.to_str())
            != Some(LOCAL_DATA_DIRECTORY_NAME)
    {
        return Err(LocalInstanceError::DataDirectoryEscapesPackageRoot);
    }
    let lock_path = canonical_data.join(LOCAL_INSTANCE_LOCK_FILE_NAME);
    match fs::symlink_metadata(&lock_path) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            Err(LocalInstanceError::InvalidLockTarget)
        }
        Ok(_) => Ok(lock_path),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(lock_path),
        Err(source) => Err(LocalInstanceError::InspectLockTarget(source)),
    }
}

#[cfg(windows)]
fn try_acquire_primary(lock_path: &Path) -> Result<Option<File>, LocalInstanceError> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const ERROR_SHARING_VIOLATION: i32 = 32;
    const ERROR_LOCK_VIOLATION: i32 = 33;

    let result = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .share_mode(FILE_SHARE_READ)
        .open(lock_path);
    match result {
        Ok(file) => {
            file.set_len(0)
                .map_err(LocalInstanceError::WriteLockRecord)?;
            file.sync_all()
                .map_err(LocalInstanceError::WriteLockRecord)?;
            Ok(Some(file))
        }
        Err(source)
            if matches!(
                source.raw_os_error(),
                Some(ERROR_SHARING_VIOLATION) | Some(ERROR_LOCK_VIOLATION)
            ) =>
        {
            Ok(None)
        }
        Err(source) => Err(LocalInstanceError::AcquireLock(source)),
    }
}

#[cfg(not(windows))]
fn try_acquire_primary(lock_path: &Path) -> Result<Option<File>, LocalInstanceError> {
    match OpenOptions::new()
        .create_new(true)
        .read(true)
        .write(true)
        .open(lock_path)
    {
        Ok(file) => Ok(Some(file)),
        Err(source) if source.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
        Err(source) => Err(LocalInstanceError::AcquireLock(source)),
    }
}

fn read_browser_url(lock_path: &Path) -> Result<LoopbackBrowserUrl, LocalInstanceError> {
    let mut file = open_lock_record_for_read(lock_path)?;
    let mut bytes = Vec::new();
    std::io::Read::by_ref(&mut file)
        .take(MAX_LOCK_CONTENT_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(LocalInstanceError::ReadLockRecord)?;
    if bytes.len() as u64 > MAX_LOCK_CONTENT_BYTES
        || bytes.last() != Some(&b'\n')
        || bytes[..bytes.len().saturating_sub(1)].contains(&b'\n')
        || bytes.contains(&b'\r')
    {
        return Err(LocalInstanceError::InvalidLockRecord);
    }
    bytes.pop();
    let value = String::from_utf8(bytes).map_err(|_| LocalInstanceError::InvalidLockRecord)?;
    LoopbackBrowserUrl::parse(&value)
}

#[cfg(windows)]
fn open_lock_record_for_read(lock_path: &Path) -> Result<File, LocalInstanceError> {
    use std::os::windows::fs::OpenOptionsExt;

    const FILE_SHARE_READ: u32 = 0x0000_0001;
    const FILE_SHARE_WRITE: u32 = 0x0000_0002;

    OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(lock_path)
        .map_err(LocalInstanceError::ReadLockRecord)
}

#[cfg(not(windows))]
fn open_lock_record_for_read(lock_path: &Path) -> Result<File, LocalInstanceError> {
    File::open(lock_path).map_err(LocalInstanceError::ReadLockRecord)
}

#[derive(Debug, Error)]
pub enum LocalInstanceError {
    #[error("failed to create the package-local data directory for the instance lock")]
    CreateDataDirectory(#[source] std::io::Error),
    #[error("failed to canonicalize the package-local data directory for the instance lock")]
    CanonicalizeDataDirectory(#[source] std::io::Error),
    #[error("the instance lock data directory escaped the package root")]
    DataDirectoryEscapesPackageRoot,
    #[error("failed to inspect the package-local instance lock")]
    InspectLockTarget(#[source] std::io::Error),
    #[error("the package-local instance lock target is not a regular file")]
    InvalidLockTarget,
    #[error("failed to acquire the package-local instance lock")]
    AcquireLock(#[source] std::io::Error),
    #[error("the primary instance lease was already released")]
    LeaseReleased,
    #[error("failed to write the package-local instance record")]
    WriteLockRecord(#[source] std::io::Error),
    #[error("failed to read the package-local instance record")]
    ReadLockRecord(#[source] std::io::Error),
    #[error("the package-local instance record is malformed")]
    InvalidLockRecord,
    #[error("the published browser URL is not a canonical IPv4 loopback URL")]
    InvalidBrowserUrl,
    #[error("the existing local instance did not publish a healthy browser URL")]
    ExistingInstanceUnavailable,
}
