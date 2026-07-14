//! Shared, in-memory watch and episode product core.

#![forbid(unsafe_code)]
#![deny(warnings)]

pub const PACKAGE_BOUNDARY: &str = "bcsp-watch";

mod clock;
mod effect;
mod facade;
mod manager;
mod selection;
mod state;

pub use clock::{WatchClock, WatchInstant};
pub use facade::{
    WatchAction, WatchActionKind, WatchCleanupReason, WatchCleanupReport, WatchDispatch,
    WatchManager, WatchManagerError, WatchPublishOutcome, WatchStartAdmission, WatchStartOutcome,
    WatchTickOutcome,
};
pub use selection::{MAX_SELECTED_SECTIONS, SectionSelection, SelectionChange, SelectionError};

pub fn boundary_marker() -> &'static str {
    let _ = (
        bcsp_contracts::PACKAGE_BOUNDARY,
        bcsp_domain::PACKAGE_BOUNDARY,
        bcsp_open::PACKAGE_BOUNDARY,
    );
    PACKAGE_BOUNDARY
}

mod dependency_contract {
    use serde as _;
    use serde_json as _;
    use thiserror as _;
    use tokio as _;
    use tracing as _;
    use uuid as _;
}

#[cfg(test)]
mod dev_dependency_contract {
    use proptest as _;
}
