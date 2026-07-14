use std::collections::BTreeSet;

use bcsp_catalog::{SnapshotMappingError, to_discovery_refresh_command};
use bcsp_contracts::{TermCampusKey, TraceId};
use bcsp_operational_storage::{
    BeginDiscoveryAttemptCommand, DiscoveryPublishOutcome,
    DiscoverySnapshot as StoredDiscoverySnapshot, FinishDiscoveryFailureCommand,
    RefreshFailureStage, StorageError,
};
use bcsp_rutgers_client::{DiscoverySnapshot, OpenSectionsRequest, OpenSectionsRequestError};
use thiserror::Error;

use crate::{ProductStorageAccess, ScheduledRefreshTarget};

/// A validated discovery publication and the exact Rutgers Open registrations derived from it.
/// Both product entrypoints use this one conversion so target identity cannot drift between them.
#[derive(Clone, Debug)]
pub struct PublishedRefreshTargets {
    pub outcome: DiscoveryPublishOutcome,
    pub targets: Vec<ScheduledRefreshTarget>,
}

pub fn publish_discovery_for_refresh<S>(
    storage: &S,
    discovery: &DiscoverySnapshot,
    observation_id: TraceId,
    started_at: impl Into<String>,
    completed_at: impl Into<String>,
) -> Result<PublishedRefreshTargets, DiscoveryRuntimeError>
where
    S: ProductStorageAccess,
{
    let targets = scheduled_targets(discovery)?;
    let command = to_discovery_refresh_command(
        discovery,
        observation_id,
        started_at.into(),
        completed_at.into(),
    )?;
    let outcome = storage
        .lock_operational()
        .map_err(|_| DiscoveryRuntimeError::StorageLock)?
        .apply_discovery_refresh(command)?;
    Ok(PublishedRefreshTargets { outcome, targets })
}

pub fn record_discovery_transport_failure<S>(
    storage: &S,
    observation_id: TraceId,
    started_at: impl Into<String>,
    completed_at: impl Into<String>,
    error_code: impl Into<String>,
) -> Result<(), DiscoveryRuntimeError>
where
    S: ProductStorageAccess,
{
    let mut storage = storage
        .lock_operational()
        .map_err(|_| DiscoveryRuntimeError::StorageLock)?;
    storage.begin_discovery_attempt(&BeginDiscoveryAttemptCommand {
        observation_id,
        started_at: started_at.into(),
    })?;
    storage.finish_discovery_failure(&FinishDiscoveryFailureCommand {
        observation_id,
        completed_at: completed_at.into(),
        stage: RefreshFailureStage::Transport,
        error_code: error_code.into(),
        diagnostic_token: None,
    })?;
    Ok(())
}

/// Restores the exact Open scheduler registrations from the last published discovery snapshot.
///
/// This is the clean-restart path used when the process has durable discovery LKG. Loading the
/// published snapshot revalidates SQLite's content version and source references; this conversion
/// additionally requires exact enabled/published flags, term coordinates, and same-source
/// ownership before any upstream request can be reconstructed.
pub fn restore_refresh_targets<S>(
    storage: &S,
) -> Result<Vec<ScheduledRefreshTarget>, DiscoveryRuntimeError>
where
    S: ProductStorageAccess,
{
    let published = storage
        .lock_operational()
        .map_err(|_| DiscoveryRuntimeError::StorageLock)?
        .published_discovery_snapshot()?;
    scheduled_targets_from_persisted(&published.snapshot)
}

fn scheduled_targets(
    discovery: &DiscoverySnapshot,
) -> Result<Vec<ScheduledRefreshTarget>, DiscoveryRuntimeError> {
    let mut registrations = Vec::new();
    for target in &discovery.targets {
        if target.enabled.value() != Some(&true) {
            continue;
        }
        let Some(term) = discovery
            .terms
            .iter()
            .find(|term| term.term_id == *target.key.term())
        else {
            return Err(DiscoveryRuntimeError::TargetWithoutTerm(target.key.clone()));
        };
        if term.published.value() != Some(&true) {
            continue;
        }
        let request = OpenSectionsRequest::try_from_discovery(term, target)?;
        registrations.push(ScheduledRefreshTarget::try_new(
            target.key.clone(),
            request,
        )?);
    }
    registrations.sort_by(|left, right| left.target().cmp(right.target()));
    Ok(registrations)
}

fn scheduled_targets_from_persisted(
    discovery: &StoredDiscoverySnapshot,
) -> Result<Vec<ScheduledRefreshTarget>, DiscoveryRuntimeError> {
    let mut source_ids = BTreeSet::new();
    for source in &discovery.sources {
        if !source_ids.insert(source.source_version_id.as_str()) {
            return Err(DiscoveryRuntimeError::DuplicateSourceVersion);
        }
    }

    let mut registrations = Vec::new();
    for campus in &discovery.campuses {
        if campus.enabled != Some(true) {
            continue;
        }
        let Some(term) = discovery
            .terms
            .iter()
            .find(|term| term.term_id == *campus.target.term())
        else {
            return Err(DiscoveryRuntimeError::TargetWithoutTerm(
                campus.target.clone(),
            ));
        };
        if term.published != Some(true) {
            continue;
        }
        if term.source_version_id != campus.source_version_id {
            return Err(DiscoveryRuntimeError::TargetSourceOwnershipMismatch(
                campus.target.clone(),
            ));
        }
        if !source_ids.contains(term.source_version_id.as_str()) {
            return Err(DiscoveryRuntimeError::UnknownSourceVersion);
        }
        let request = OpenSectionsRequest::try_from_persisted_discovery(
            &term.source_version_id,
            &term.term_id,
            &campus.target,
            term.year,
            term.term_code.as_deref(),
        )?;
        registrations.push(ScheduledRefreshTarget::try_new(
            campus.target.clone(),
            request,
        )?);
    }
    registrations.sort_by(|left, right| left.target().cmp(right.target()));
    Ok(registrations)
}

#[derive(Debug, Error)]
pub enum DiscoveryRuntimeError {
    #[error("operational storage is unavailable")]
    StorageLock,
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Mapping(#[from] SnapshotMappingError),
    #[error(transparent)]
    OpenRequest(#[from] OpenSectionsRequestError),
    #[error(transparent)]
    Coordinator(#[from] crate::CoordinatorError),
    #[error("discovery target has no matching term: {0:?}")]
    TargetWithoutTerm(TermCampusKey),
    #[error("published discovery contains a duplicate source version")]
    DuplicateSourceVersion,
    #[error("published discovery target and term do not share one source version: {0:?}")]
    TargetSourceOwnershipMismatch(TermCampusKey),
    #[error("published discovery target references an unknown source version")]
    UnknownSourceVersion,
}
