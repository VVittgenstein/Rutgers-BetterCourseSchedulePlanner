use std::fmt;

use bcsp_contracts::{SystemTraceIdSource, TraceId, TraceIdSource};

use crate::{LocalBootstrapError, LocalInstanceError, LocalRuntimeError};

/// A user-facing startup failure that is safe to show or copy into a support report.
///
/// The underlying error can contain local filesystem paths or an authenticated
/// loopback URL, so it is deliberately not included in this display value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StartupFailureReport {
    trace_id: TraceId,
    summary: &'static str,
}

impl StartupFailureReport {
    pub fn from_error(error: &LocalRuntimeError) -> Self {
        let summary = match error {
            LocalRuntimeError::Bootstrap(LocalBootstrapError::CatalogDerivation(_)) => {
                "RBCSP could not upgrade the course catalog stored in its data folder. Start RBCSP again; if this repeats, rename the data folder next to RBCSP.exe so a fresh catalog can be downloaded."
            }
            LocalRuntimeError::Path(_)
            | LocalRuntimeError::Bootstrap(_)
            | LocalRuntimeError::Surface(_)
            | LocalRuntimeError::Instance(
                LocalInstanceError::CreateDataDirectory(_)
                | LocalInstanceError::CanonicalizeDataDirectory(_)
                | LocalInstanceError::DataDirectoryEscapesPackageRoot
                | LocalInstanceError::InspectLockTarget(_)
                | LocalInstanceError::InvalidLockTarget
                | LocalInstanceError::AcquireLock(_)
                | LocalInstanceError::WriteLockRecord(_),
            ) => {
                "RBCSP could not prepare its local data directory. Check that the extracted package is in a writable folder."
            }
            LocalRuntimeError::Instance(
                LocalInstanceError::ReadLockRecord(_)
                | LocalInstanceError::InvalidLockRecord
                | LocalInstanceError::InvalidBrowserUrl
                | LocalInstanceError::ExistingInstanceUnavailable,
            ) => {
                "RBCSP could not start or reconnect to the existing local instance. Close any stale RBCSP process and try again."
            }
            LocalRuntimeError::OpenBrowser { .. } => {
                "RBCSP started locally, but the browser could not be opened. Check the default-browser setting and try again."
            }
            LocalRuntimeError::Loopback(_)
            | LocalRuntimeError::Watch(_)
            | LocalRuntimeError::Refresh(_)
            | LocalRuntimeError::ShutdownSignal(_)
            | LocalRuntimeError::ShutdownRequestChannelClosed
            | LocalRuntimeError::Instance(LocalInstanceError::LeaseReleased)
            | LocalRuntimeError::RuntimeBuild(_)
            | LocalRuntimeError::InjectedNetworkStart => {
                "RBCSP could not start its local service. Close any stale RBCSP process and try again."
            }
        };
        let mut source = SystemTraceIdSource;
        Self {
            trace_id: source.next_trace_id(),
            summary,
        }
    }

    pub const fn trace_id(&self) -> TraceId {
        self.trace_id
    }

    pub const fn summary(&self) -> &'static str {
        self.summary
    }
}

impl fmt::Display for StartupFailureReport {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}\nTrace ID: {}", self.summary, self.trace_id)
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    #[test]
    fn report_is_actionable_and_does_not_expose_underlying_browser_url() {
        let sensitive_url = "http://127.0.0.1:43123/?session=do-not-print";
        let error = LocalRuntimeError::OpenBrowser {
            url: sensitive_url.to_owned(),
            source: io::Error::other("C:\\Users\\Example\\private-browser.exe"),
        };

        let report = StartupFailureReport::from_error(&error);
        let rendered = report.to_string();

        assert!(rendered.contains("browser could not be opened"));
        assert!(rendered.contains("Trace ID: "));
        assert!(!rendered.contains(sensitive_url));
        assert!(!rendered.contains("Users"));
        assert_eq!(report.trace_id().to_string().len(), 36);
    }

    #[test]
    fn instance_storage_failure_points_to_the_writable_package_folder() {
        let private_path = "C:\\Users\\Example\\RBCSP\\data";
        let error = LocalRuntimeError::Instance(LocalInstanceError::CreateDataDirectory(
            io::Error::new(io::ErrorKind::PermissionDenied, private_path),
        ));

        let rendered = StartupFailureReport::from_error(&error).to_string();

        assert!(rendered.contains("writable folder"));
        assert!(rendered.contains("Trace ID: "));
        assert!(!rendered.contains(private_path));
        assert!(!rendered.contains("stale RBCSP process"));
    }

    #[test]
    fn catalog_derivation_failure_does_not_blame_the_package_folder() {
        let error = LocalRuntimeError::Bootstrap(LocalBootstrapError::CatalogDerivation(
            bcsp_application::RederivationError::Timestamp,
        ));

        let rendered = StartupFailureReport::from_error(&error).to_string();

        assert!(rendered.contains("course catalog"));
        assert!(rendered.contains("Trace ID: "));
        assert!(!rendered.contains("writable folder"));
        assert!(!rendered.contains("stale RBCSP process"));
    }

    #[test]
    fn unavailable_existing_instance_keeps_the_instance_recovery_guidance() {
        let error = LocalRuntimeError::Instance(LocalInstanceError::ExistingInstanceUnavailable);

        let rendered = StartupFailureReport::from_error(&error).to_string();

        assert!(rendered.contains("stale RBCSP process"));
        assert!(rendered.contains("Trace ID: "));
        assert!(!rendered.contains("writable folder"));
    }
}
