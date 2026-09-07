//! Shared, in-memory watch and episode product core.

#![forbid(unsafe_code)]
#![deny(warnings)]

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
