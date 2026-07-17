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

use crate::{
    PreparedServingError, PreparedServingRebuildDemand, ProductStorageAccess,
    ScheduledRefreshTarget,
};
use crate::{is_product_campus, product_target_keys};

const ONLINE_CAMPUS_ALIASES: [&str; 3] = ["ONLINE_NB", "ONLINE_NK", "ONLINE_CM"];

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
    publish_discovery_for_refresh_with_prepared(
        storage,
        discovery,
        observation_id,
        started_at,
        completed_at,
        None,
    )
}

pub(crate) fn publish_discovery_for_refresh_with_prepared<S>(
    storage: &S,
    discovery: &DiscoverySnapshot,
    observation_id: TraceId,
    started_at: impl Into<String>,
    completed_at: impl Into<String>,
    prepared: Option<&PreparedServingRebuildDemand>,
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
    let barrier = prepared
        .map(PreparedServingRebuildDemand::begin_publication)
        .transpose()?;
    let outcome = storage
        .lock_operational()
        .map_err(|_| DiscoveryRuntimeError::StorageLock)?
        .apply_discovery_refresh(command)?;
    if let Some(barrier) = barrier {
        barrier.commit();
    }
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

/// Derives the fixed Round 4 product scheduler set from one validated Rutgers discovery.
///
/// One affirmative NB/NK/CM target proves that the term is published. Once published, the term
/// expands to all three product Campuses so Local term-level Pull and automatic current/next
/// refresh never inherit a partial or selector-order-dependent target set.
pub(crate) fn product_refresh_targets(
    discovery: &DiscoverySnapshot,
) -> Result<Vec<ScheduledRefreshTarget>, DiscoveryRuntimeError> {
    let mut registrations = Vec::new();
    for term in &discovery.terms {
        if term.published.value() != Some(&true)
            || !discovery.targets.iter().any(|target| {
                target.key.term() == &term.term_id
                    && is_product_campus(target.key.campus().as_str())
                    && target.enabled.value() == Some(&true)
                    && discovery.campuses.iter().any(|campus| {
                        campus.campus_code == *target.key.campus()
                            && campus.enabled.value() == Some(&true)
                    })
            })
        {
            continue;
        }
        for target in product_target_keys(&term.term_id) {
            let request = OpenSectionsRequest::try_from_persisted_discovery(
                term.source_id.as_str(),
                &term.term_id,
                &target,
                term.year
                    .value()
                    .copied()
                    .and_then(|year| u16::try_from(year).ok()),
                term.term_code.value().map(String::as_str),
            )?;
            registrations.push(ScheduledRefreshTarget::try_new(target, request)?);
        }
    }
    registrations.sort_by(|left, right| left.target().cmp(right.target()));
    Ok(registrations)
}

pub(crate) fn restore_product_refresh_targets<S>(
    storage: &S,
) -> Result<Vec<ScheduledRefreshTarget>, DiscoveryRuntimeError>
where
    S: ProductStorageAccess,
{
    let published = storage
        .lock_operational()
        .map_err(|_| DiscoveryRuntimeError::StorageLock)?
        .published_discovery_snapshot()?;
    product_refresh_targets_from_persisted(&published.snapshot)
}

fn product_refresh_targets_from_persisted(
    discovery: &StoredDiscoverySnapshot,
) -> Result<Vec<ScheduledRefreshTarget>, DiscoveryRuntimeError> {
    let mut registrations = Vec::new();
    for term in &discovery.terms {
        if term.published != Some(true)
            || !discovery.campuses.iter().any(|target| {
                target.target.term() == &term.term_id
                    && is_product_campus(target.target.campus().as_str())
                    && target.enabled == Some(true)
                    && crate::product_scope::stored_presence_is_true(
                        &target.canonical_facts,
                        "campusEnabled",
                    )
                    && crate::product_scope::stored_presence_is_true(
                        &target.canonical_facts,
                        "targetEnabled",
                    )
            })
        {
            continue;
        }
        for target in product_target_keys(&term.term_id) {
            let request = OpenSectionsRequest::try_from_persisted_discovery(
                &term.source_version_id,
                &term.term_id,
                &target,
                term.year,
                term.term_code.as_deref(),
            )?;
            registrations.push(ScheduledRefreshTarget::try_new(target, request)?);
        }
    }
    registrations.sort_by(|left, right| left.target().cmp(right.target()));
    Ok(registrations)
}

fn scheduled_targets(
    discovery: &DiscoverySnapshot,
) -> Result<Vec<ScheduledRefreshTarget>, DiscoveryRuntimeError> {
    let mut registrations = Vec::new();
    for target in &discovery.targets {
        if target.enabled.value() != Some(&true) {
            continue;
        }
        if is_online_campus_alias(target.key.campus().as_str()) {
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
        if is_online_campus_alias(campus.target.campus().as_str()) {
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

pub(crate) fn is_online_campus_alias(campus: &str) -> bool {
    ONLINE_CAMPUS_ALIASES
        .iter()
        .any(|alias| campus.eq_ignore_ascii_case(alias))
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
    #[error(transparent)]
    PreparedServing(#[from] PreparedServingError),
    #[error("discovery target has no matching term: {0:?}")]
    TargetWithoutTerm(TermCampusKey),
    #[error("published discovery contains a duplicate source version")]
    DuplicateSourceVersion,
    #[error("published discovery target and term do not share one source version: {0:?}")]
    TargetSourceOwnershipMismatch(TermCampusKey),
    #[error("published discovery target references an unknown source version")]
    UnknownSourceVersion,
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use bcsp_contracts::TraceId;
    use bcsp_operational_storage::OperationalStorage;
    use bcsp_rutgers_client::{DiscoverySnapshot, SourceProvenance, decode_discovery_payload};

    use super::{
        product_refresh_targets, publish_discovery_for_refresh, restore_product_refresh_targets,
    };

    const DISCOVERY: &[u8] = br#"{
      "sourceVersion":"round-4-product-scope",
      "terms":[
        {"termId":"92026","year":2026,"termCode":"9","published":true},
        {"termId":"12027","year":2027,"termCode":"1","published":false}
      ],
      "campuses":[
        {"campusCode":"NB","enabled":true},
        {"campusCode":"ONLINE_NB","enabled":true},
        {"campusCode":"NWK","enabled":true}
      ],
      "targets":[
        {"termId":"92026","campusCode":"NB","enabled":true},
        {"termId":"92026","campusCode":"ONLINE_NB","enabled":true},
        {"termId":"12027","campusCode":"NWK","enabled":true}
      ],
      "subjects":[]
    }"#;

    fn discovery() -> DiscoverySnapshot {
        DiscoverySnapshot::try_from_raw(
            decode_discovery_payload(DISCOVERY).expect("discovery JSON"),
            SourceProvenance::from_body("ROUND_4_PRODUCT_SCOPE", "2026-07-17T12:00:00Z", DISCOVERY),
        )
        .expect("validated discovery")
    }

    #[test]
    fn one_affirmative_main_campus_expands_to_exactly_nb_nk_cm_and_restores_identically() {
        let discovery = discovery();
        let projected = product_refresh_targets(&discovery).expect("product targets");
        assert_eq!(
            projected
                .iter()
                .map(|target| (
                    target.target().term().as_str(),
                    target.target().campus().as_str()
                ))
                .collect::<Vec<_>>(),
            vec![("92026", "CM"), ("92026", "NB"), ("92026", "NK")],
        );

        let storage = Arc::new(Mutex::new(
            OperationalStorage::open_in_memory().expect("storage"),
        ));
        publish_discovery_for_refresh(
            &storage,
            &discovery,
            "00000000-0000-4000-8000-000000000001"
                .parse::<TraceId>()
                .expect("trace"),
            "2026-07-17T12:00:00Z",
            "2026-07-17T12:00:01Z",
        )
        .expect("publish");
        let restored = restore_product_refresh_targets(&storage).expect("restored targets");
        assert_eq!(
            restored
                .iter()
                .map(|target| target.target().clone())
                .collect::<Vec<_>>(),
            projected
                .iter()
                .map(|target| target.target().clone())
                .collect::<Vec<_>>(),
        );
    }
}
