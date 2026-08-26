//! Shared operational SQLite persistence for Catalog discovery and normalized serving data.
//!
//! This crate owns target-neutral write projections and crash-safe publication. It deliberately
//! does not depend on `bcsp-catalog`: normalizers submit independent DTOs built from the shared
//! identities in `bcsp-contracts`.
//!
//! # Write-path encapsulation
//!
//! The single writable SQLite connection inside [`OperationalStorage`] is sealed: every write
//! goes through the typed command API, so a dependent crate can never run raw SQL or `PRAGMA`s
//! that would bypass the storage write invariants. There is no accessor that hands out the raw
//! connection:
//!
//! ```compile_fail,E0599
//! use bcsp_operational_storage::OperationalStorage;
//!
//! fn hijack(storage: &OperationalStorage) -> &rusqlite::Connection {
//!     storage.raw_connection_for_tests()
//! }
//! ```
//!
//! and the connection field itself is private outside the crate:
//!
//! ```compile_fail,E0616
//! use bcsp_operational_storage::OperationalStorage;
//!
//! fn hijack(storage: &OperationalStorage) -> &rusqlite::Connection {
//!     &storage.connection
//! }
//! ```

#![forbid(unsafe_code)]
#![deny(warnings)]

mod discovery;
mod error;
mod migration;
mod migration_bundle;
mod model;
mod open;
mod storage;

pub use discovery::discovery_content_sha256_v1;
pub use error::{StorageError, StorageResult};
pub use model::{
    BeginDiscoveryAttemptCommand, BeginRefreshAttemptCommand, CatalogCounts, CatalogFailureAudit,
    CatalogRefreshCommand, CatalogSnapshot, CourseFtsCorpusSignature, CourseTextSearchTokens,
    CourseVariantSearchHit, CourseVariantSearchResult, DiscoveredCampus, DiscoveredSubject,
    DiscoveredTerm, DiscoveryAvailability, DiscoveryCounts, DiscoveryObservation,
    DiscoveryPublishOutcome, DiscoveryRefreshCommand, DiscoverySnapshot, DiscoverySourceKind,
    DiscoverySourceVersion, DiscoveryState, DiscoveryStatus, EmptySnapshotDecision,
    FinishDiscoveryFailureCommand, FinishRefreshFailureCommand, InitialEmptyProof, MigrationRecord,
    ProvenanceEntityKind, PublishOutcome, PublishedCatalogSnapshot, PublishedCourseFtsDocument,
    PublishedDiscoverySnapshot, RawStagingPayload, RefreshFailureStage, RefreshObservation,
    RefreshStatus, StorageIntegrityReport, StoredCanonicalFacts, StoredCourseGroup,
    StoredCourseVariant, StoredOccurrence, StoredProvenance, StoredSection, TargetState,
};
pub use open::{
    BeginOpenPullAttemptCommand, CatalogCandidateOpenSnapshot, CompleteSnapshotCommitOutcome,
    CompleteTargetSnapshotState, FinishOpenPullFailureCommand, FinishOpenPullSuccessCommand,
    OpenAttemptClassification, OpenAttemptCounters, OpenAttemptRecord, OpenBatchObservation,
    OpenGateAttemptSummary,
    OpenBatchState, OpenCacheStatus, OpenCatalogSnapshot, OpenCircuitState, OpenCommitOutcome,
    OpenHttpAuditMetadata, OpenObservationCommit, OpenOriginState, OpenRequestLane,
    OpenRetentionReport, OpenScheduleState, OpenSectionCurrent, OpenSectionEvent, OpenSectionState,
    derive_open_refresh_observation_id, derive_open_section_observation_id,
};
pub use storage::{
    OperationalStorage, OperationalStorageInterruptHandle, catalog_content_sha256_v1,
};

pub const PACKAGE_BOUNDARY: &str = "bcsp-operational-storage";

pub fn boundary_marker() -> &'static str {
    let _ = (
        bcsp_contracts::PACKAGE_BOUNDARY,
        bcsp_domain::PACKAGE_BOUNDARY,
    );
    PACKAGE_BOUNDARY
}

mod dependency_contract {
    use include_dir as _;
    use rusqlite as _;
    use serde as _;
    use serde_json as _;
    use sha2 as _;
    use thiserror as _;
    use time as _;
    use tracing as _;
}

#[cfg(test)]
mod dev_dependency_contract {
    use tempfile as _;
}
