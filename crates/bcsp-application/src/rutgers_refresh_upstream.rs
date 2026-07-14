use std::collections::BTreeSet;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use bcsp_contracts::{SystemTraceIdSource, TermCampusKey, TraceIdSource};
use bcsp_rutgers_client::{
    CatalogClientBuildError, CatalogFailure, CatalogRequest, CatalogTransportError,
    OpenClientBuildError, OpenSectionsFailure, OpenSectionsRequest, OpenSectionsResponse,
    RutgersCatalogClient, RutgersOpenSectionsClient,
};
use thiserror::Error;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::{CatalogPullFailure, CatalogPullResponse, RefreshFuture, RefreshUpstream};

/// The latest selector-confirmed Catalog targets published by discovery.
///
/// Discovery owns replacement of this set. Catalog pulls take a short read lock only after the
/// response has been decoded, so each response is classified against a current, complete
/// discovery membership rather than a startup allowlist.
#[derive(Debug, Default)]
pub struct SelectorTargetMembership {
    targets: RwLock<BTreeSet<TermCampusKey>>,
}

impl SelectorTargetMembership {
    pub fn new() -> Self {
        Self::default()
    }

    /// Atomically replace the complete discovery membership and return its new size.
    pub fn replace<I>(&self, targets: I) -> usize
    where
        I: IntoIterator<Item = TermCampusKey>,
    {
        let targets = targets.into_iter().collect::<BTreeSet<_>>();
        let size = targets.len();
        *self.write_targets() = targets;
        size
    }

    /// Apply one incremental discovery membership change.
    ///
    /// The return value is true only when the membership changed.
    pub fn update(&self, target: TermCampusKey, confirmed: bool) -> bool {
        let mut targets = self.write_targets();
        if confirmed {
            targets.insert(target)
        } else {
            targets.remove(&target)
        }
    }

    pub fn contains(&self, target: &TermCampusKey) -> bool {
        self.read_targets().contains(target)
    }

    pub fn len(&self) -> usize {
        self.read_targets().len()
    }

    pub fn is_empty(&self) -> bool {
        self.read_targets().is_empty()
    }

    pub fn snapshot(&self) -> BTreeSet<TermCampusKey> {
        self.read_targets().clone()
    }

    fn read_targets(&self) -> RwLockReadGuard<'_, BTreeSet<TermCampusKey>> {
        self.targets
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn write_targets(&self) -> RwLockWriteGuard<'_, BTreeSet<TermCampusKey>> {
        self.targets
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[derive(Debug, Error)]
pub enum RutgersRefreshUpstreamBuildError {
    #[error("the official Rutgers Catalog client could not be built")]
    Catalog(#[source] CatalogClientBuildError),
    #[error("the official Rutgers Open Sections client could not be built")]
    Open(#[source] OpenClientBuildError),
}

/// Production implementation of the shared refresh seam.
///
/// Both transports are fixed-origin, bounded Rutgers clients. Tests inject a different
/// [`RefreshUpstream`] into the coordinator and therefore never need to redirect this adapter to
/// a non-production endpoint.
pub struct RutgersRefreshUpstream {
    catalog: RutgersCatalogClient,
    open: RutgersOpenSectionsClient,
    selector_targets: Arc<SelectorTargetMembership>,
}

impl RutgersRefreshUpstream {
    pub fn new_official(
        selector_targets: Arc<SelectorTargetMembership>,
    ) -> Result<Self, RutgersRefreshUpstreamBuildError> {
        let catalog = RutgersCatalogClient::new_official()
            .map_err(RutgersRefreshUpstreamBuildError::Catalog)?;
        let open = RutgersOpenSectionsClient::new_official()
            .map_err(RutgersRefreshUpstreamBuildError::Open)?;
        Ok(Self {
            catalog,
            open,
            selector_targets,
        })
    }

    pub fn selector_targets(&self) -> &Arc<SelectorTargetMembership> {
        &self.selector_targets
    }
}

impl RefreshUpstream for RutgersRefreshUpstream {
    fn fetch_catalog<'a>(
        &'a self,
        target: &'a TermCampusKey,
    ) -> RefreshFuture<'a, Result<CatalogPullResponse, CatalogPullFailure>> {
        Box::pin(async move {
            let request = catalog_request(target)?;
            let (request_id, observed_at_utc) = catalog_request_metadata()?;
            let response = self
                .catalog
                .fetch(&request, &request_id, &observed_at_utc)
                .await
                .map_err(map_catalog_failure)?;
            let selector_confirms_target = self.selector_targets.contains(response.target());
            let (target, courses, provenance) = response.into_normalization_parts();
            Ok(CatalogPullResponse {
                target,
                courses,
                provenance,
                selector_confirms_target,
            })
        })
    }

    fn fetch_open<'a>(
        &'a self,
        request: OpenSectionsRequest,
    ) -> RefreshFuture<'a, Result<OpenSectionsResponse, OpenSectionsFailure>> {
        Box::pin(async move { self.open.fetch(&request).await })
    }
}

fn catalog_request(target: &TermCampusKey) -> Result<CatalogRequest, CatalogPullFailure> {
    CatalogRequest::try_from_target(target.clone()).map_err(|_| CatalogPullFailure::Schema)
}

fn catalog_request_metadata() -> Result<(String, String), CatalogPullFailure> {
    let mut source = SystemTraceIdSource;
    let request_id = source.next_trace_id().to_string();
    let observed_at_utc = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| CatalogPullFailure::Schema)?;
    Ok((request_id, observed_at_utc))
}

fn map_catalog_failure(failure: CatalogFailure) -> CatalogPullFailure {
    map_catalog_transport_error(failure.kind())
}

fn map_catalog_transport_error(error: &CatalogTransportError) -> CatalogPullFailure {
    match error {
        CatalogTransportError::Timeout | CatalogTransportError::Network => {
            CatalogPullFailure::Transport
        }
        CatalogTransportError::NonSuccessHttp { status: 429 } => {
            CatalogPullFailure::RateLimited { retry_after: None }
        }
        CatalogTransportError::NonSuccessHttp { status }
            if matches!(*status, 408 | 425 | 500..=599) =>
        {
            CatalogPullFailure::Transport
        }
        CatalogTransportError::RequestConstruction
        | CatalogTransportError::ResponseUrlMismatch
        | CatalogTransportError::Redirect { .. }
        | CatalogTransportError::NonSuccessHttp { .. }
        | CatalogTransportError::InvalidContentType
        | CatalogTransportError::ResponseTooLarge
        | CatalogTransportError::ContentDecoding
        | CatalogTransportError::InvalidJson => CatalogPullFailure::Schema,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use bcsp_contracts::{TermCampusKey, TraceId};
    use bcsp_rutgers_client::{CatalogTransportError, RedirectScope};
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

    use super::{
        SelectorTargetMembership, catalog_request, catalog_request_metadata,
        map_catalog_transport_error,
    };
    use crate::CatalogPullFailure;

    fn target(term: &str, campus: &str) -> TermCampusKey {
        TermCampusKey::try_new(term, campus).expect("contract-valid target")
    }

    #[test]
    fn selector_membership_replaces_and_updates_complete_target_keys() {
        let membership = SelectorTargetMembership::new();
        let nb = target("92026", "NB");
        let nwk = target("92026", "NWK");
        let later_term = target("12027", "NB");

        assert!(membership.is_empty());
        assert_eq!(membership.replace([nb.clone(), nwk.clone()]), 2);
        assert!(membership.contains(&nb));
        assert!(membership.contains(&nwk));
        assert!(!membership.contains(&later_term));

        assert!(membership.update(later_term.clone(), true));
        assert!(!membership.update(later_term.clone(), true));
        assert!(membership.update(nwk.clone(), false));
        assert!(!membership.update(nwk, false));
        assert_eq!(membership.len(), 2);
        assert_eq!(membership.snapshot(), BTreeSet::from([nb, later_term]));
    }

    #[test]
    fn catalog_request_strictly_decodes_only_rutgers_term_identities() {
        let winter = catalog_request(&target("02027", "NB")).expect("Rutgers Winter target");
        assert_eq!(winter.year(), 2027);
        assert_eq!(winter.term_code(), 0);

        assert_eq!(
            catalog_request(&target("22026", "NB")),
            Err(CatalogPullFailure::Schema)
        );
        assert_eq!(
            catalog_request(&target("2026", "NB")),
            Err(CatalogPullFailure::Schema)
        );
    }

    #[test]
    fn catalog_failure_mapping_separates_retryable_origin_and_protocol_failures() {
        assert_eq!(
            map_catalog_transport_error(&CatalogTransportError::Timeout),
            CatalogPullFailure::Transport
        );
        assert_eq!(
            map_catalog_transport_error(&CatalogTransportError::NonSuccessHttp { status: 503 }),
            CatalogPullFailure::Transport
        );
        assert_eq!(
            map_catalog_transport_error(&CatalogTransportError::NonSuccessHttp { status: 429 }),
            CatalogPullFailure::RateLimited { retry_after: None }
        );
        assert_eq!(
            map_catalog_transport_error(&CatalogTransportError::InvalidJson),
            CatalogPullFailure::Schema
        );
        assert_eq!(
            map_catalog_transport_error(&CatalogTransportError::Redirect {
                status: 302,
                scope: RedirectScope::OffOrigin,
            }),
            CatalogPullFailure::Schema
        );
        assert_eq!(
            map_catalog_transport_error(&CatalogTransportError::NonSuccessHttp { status: 403 }),
            CatalogPullFailure::Schema
        );
    }

    #[test]
    fn catalog_metadata_uses_random_v4_ids_and_valid_utc_timestamps() {
        let (first_id, first_observed_at) =
            catalog_request_metadata().expect("system request metadata");
        let (second_id, second_observed_at) =
            catalog_request_metadata().expect("system request metadata");

        let _: TraceId = first_id.parse().expect("canonical random UUID v4");
        let _: TraceId = second_id.parse().expect("canonical random UUID v4");
        assert_ne!(first_id, second_id);
        OffsetDateTime::parse(&first_observed_at, &Rfc3339).expect("RFC 3339 UTC timestamp");
        OffsetDateTime::parse(&second_observed_at, &Rfc3339).expect("RFC 3339 UTC timestamp");
        assert!(first_observed_at.ends_with('Z'));
        assert!(second_observed_at.ends_with('Z'));
    }
}
