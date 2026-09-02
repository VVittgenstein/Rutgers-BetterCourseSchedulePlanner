use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use bcsp_catalog::{ProjectionError, to_normalized_catalog_v1};
use bcsp_contracts::{
    CatalogFieldKnowledge, CatalogFieldPresence, CourseDetailRequestV1, CourseDetailResponseV1,
    CourseQueryRequestV1, CourseQueryResponseV1, DynamicFilterValidationResponseV3, FilterFieldId,
    FilterOptionTargetVersionV2, FilterOptionV2, FilterOptionsFieldV2, FilterOptionsRequestV2,
    FilterOptionsResponseV2, FilterTokenV1, InvalidDynamicFilterValueV3, NormalizedCatalogV1,
    NormalizedFilterValuesV1, QUERY_CONTRACT_VERSION, SectionDetailRequestV1,
    SectionDetailResponseV1, SectionQueryRequestV1, SectionQueryResponseV1, TermCampusKey,
    course_number_band,
};
use bcsp_operational_storage::{CourseTextSearchTokens, OperationalStorage, StorageError};
use bcsp_query::{
    OpenEvidence, QueryEngine, QueryError, TextHit, TextHitPlan, TextHitPlanError,
    TextTargetVersion,
};
use thiserror::Error;
use time::OffsetDateTime;

use crate::{PreparedServingBinding, PreparedServingError, PreparedServingSnapshot};

/// Typed failures at the shared Catalog-query composition boundary.
///
/// In particular, unavailable or stale full-text evidence is never converted
/// into an empty result and never falls back to a substring scan.
#[derive(Debug, Error)]
pub enum SharedQueryError {
    #[error("at least one Catalog target is required")]
    EmptyTargetSet,
    #[error("Catalog target was supplied more than once: {target:?}")]
    DuplicateTarget { target: TermCampusKey },
    #[error("Catalog target {target:?} does not use filter term {filter_term}")]
    FilterTermMismatch {
        target: TermCampusKey,
        filter_term: String,
    },
    #[error("Catalog target campuses do not exactly match the active campus filter")]
    FilterCampusSetMismatch,
    #[error("detail key target is not part of the explicit Catalog target set")]
    DetailTargetMismatch,
    #[error("Catalog target has no published snapshot: {target:?}")]
    TargetNotPublished { target: TermCampusKey },
    #[error(
        "Catalog target {target:?} changed from content version {expected} to {current:?} while the query was prepared"
    )]
    PublicationChanged {
        target: TermCampusKey,
        expected: u64,
        current: Option<u64>,
    },
    #[error("FTS5 is unavailable for Catalog target {target:?}")]
    FtsUnavailable { target: TermCampusKey },
    #[error("normalized text tokens were rejected")]
    InvalidTextTokens {
        #[source]
        source: StorageError,
    },
    #[error("filter option {value:?} is not published for {field:?}")]
    InvalidFilterOption { field: FilterFieldId, value: String },
    #[error("storage failed for Catalog target {target:?}")]
    Storage {
        target: TermCampusKey,
        #[source]
        source: StorageError,
    },
    #[error("published Catalog projection failed for target {target:?}")]
    Projection {
        target: TermCampusKey,
        #[source]
        source: ProjectionError,
    },
    #[error("full-text evidence could not be bound to Catalog versions")]
    TextEvidence {
        #[source]
        source: TextHitPlanError,
    },
    #[error("prepared serving snapshot failed")]
    Prepared {
        #[source]
        source: Box<PreparedServingError>,
    },
    #[error("the shared query engine rejected the request")]
    Query {
        #[source]
        source: QueryError,
    },
}

/// Thin shared composition over operational Catalog storage and the pure
/// query engine. It owns neither refresh scheduling nor live Open polling.
pub struct SharedQueryService<'storage> {
    storage: &'storage mut OperationalStorage,
}

impl<'storage> SharedQueryService<'storage> {
    pub const fn new(storage: &'storage mut OperationalStorage) -> Self {
        Self { storage }
    }

    pub fn course_search(
        &mut self,
        targets: &[TermCampusKey],
        request: &CourseQueryRequestV1,
        now: OffsetDateTime,
        open: &[OpenEvidence],
    ) -> Result<CourseQueryResponseV1, SharedQueryError> {
        let context = prepare_search(self.storage, targets, request.filters.values(), |_| {})?;
        let engine_started = Instant::now();
        let engine = QueryEngine::try_new(&context.catalogs, now, open.iter().cloned())
            .map_err(query_error)?;
        tracing::debug!(
            target: "bcsp_performance",
            phase = "query_engine_build",
            elapsed_us = elapsed_micros(engine_started),
            query_kind = "course",
        );
        engine
            .course_search(request, context.text_hits.as_ref())
            .map_err(query_error)
    }

    pub fn section_search(
        &mut self,
        targets: &[TermCampusKey],
        request: &SectionQueryRequestV1,
        now: OffsetDateTime,
        open: &[OpenEvidence],
    ) -> Result<SectionQueryResponseV1, SharedQueryError> {
        let context = prepare_search(self.storage, targets, request.filters.values(), |_| {})?;
        QueryEngine::try_new(&context.catalogs, now, open.iter().cloned())
            .and_then(|engine| engine.section_search(request, context.text_hits.as_ref()))
            .map_err(query_error)
    }

    pub fn course_detail(
        &mut self,
        targets: &[TermCampusKey],
        request: &CourseDetailRequestV1,
        now: OffsetDateTime,
        open: &[OpenEvidence],
    ) -> Result<CourseDetailResponseV1, SharedQueryError> {
        let targets = validate_detail_targets(targets, &request.key.target())?;
        let catalogs = load_catalogs(self.storage, &targets)?;
        verify_current_versions(self.storage, &catalogs)?;
        QueryEngine::try_new(&catalogs, now, open.iter().cloned())
            .and_then(|engine| engine.course_detail(request))
            .map_err(query_error)
    }

    pub fn section_detail(
        &mut self,
        targets: &[TermCampusKey],
        request: &SectionDetailRequestV1,
        now: OffsetDateTime,
        open: &[OpenEvidence],
    ) -> Result<SectionDetailResponseV1, SharedQueryError> {
        let targets = validate_detail_targets(targets, &request.key.target())?;
        let catalogs = load_catalogs(self.storage, &targets)?;
        verify_current_versions(self.storage, &catalogs)?;
        QueryEngine::try_new(&catalogs, now, open.iter().cloned())
            .and_then(|engine| engine.section_detail(request))
            .map_err(query_error)
    }

    pub fn filter_options(
        &mut self,
        targets: &[TermCampusKey],
        request: &FilterOptionsRequestV2,
    ) -> Result<FilterOptionsResponseV2, SharedQueryError> {
        let targets = canonical_targets(targets)?;
        if targets != request.targets() {
            return Err(SharedQueryError::FilterCampusSetMismatch);
        }
        let catalogs = load_catalogs(self.storage, &targets)?;
        let requested_limit = if request.field == FilterOptionsFieldV2::CourseNumberBand {
            usize::MAX
        } else {
            request.effective_limit()
        };
        let fetch_limit = requested_limit.saturating_add(1);
        let catalog_refs = catalogs.iter().collect::<Vec<_>>();
        let collection_started = Instant::now();
        let mut options = match request.field {
            FilterOptionsFieldV2::Keyword => {
                let mut values = BTreeMap::new();
                for catalog in &catalogs {
                    let terms = self
                        .storage
                        .published_course_fts_terms(
                            &catalog.target,
                            catalog.content_version.get(),
                            request.query.as_deref(),
                            fetch_limit,
                        )
                        .map_err(|source| {
                            map_storage_error(
                                catalog.target.clone(),
                                Some(catalog.content_version.get()),
                                source,
                            )
                        })?;
                    for term in terms {
                        insert_option(&mut values, term.clone(), term);
                    }
                    for group in &catalog.course_groups {
                        let identifier = group.key.course_string().as_str();
                        if query_matches(identifier, request.query.as_deref()) {
                            insert_option(
                                &mut values,
                                identifier.to_owned(),
                                identifier.to_owned(),
                            );
                        }
                    }
                }
                values
            }
            field => collect_catalog_options(&catalog_refs, field, request.query.as_deref()),
        }
        .into_values()
        .collect::<Vec<_>>();
        tracing::debug!(
            target: "bcsp_performance",
            phase = "filter_option_collection",
            elapsed_us = elapsed_micros(collection_started),
            options = options.len(),
        );
        let sort_started = Instant::now();
        if request.field == FilterOptionsFieldV2::CourseNumberBand {
            options.sort_by_key(|option| option.value.parse::<u32>().unwrap_or(u32::MAX));
        } else {
            options.sort_by(|left, right| {
                left.label
                    .to_lowercase()
                    .cmp(&right.label.to_lowercase())
                    .then_with(|| left.value.cmp(&right.value))
            });
        }
        tracing::debug!(
            target: "bcsp_performance",
            phase = "filter_option_sort",
            elapsed_us = elapsed_micros(sort_started),
            options = options.len(),
        );
        let truncated = options.len() > requested_limit;
        options.truncate(requested_limit);
        verify_current_versions(self.storage, &catalogs)?;
        Ok(FilterOptionsResponseV2 {
            contract_version: QUERY_CONTRACT_VERSION,
            field: request.field,
            target_versions: catalogs
                .iter()
                .map(|catalog| FilterOptionTargetVersionV2 {
                    target: catalog.target.clone(),
                    content_version: catalog.content_version,
                })
                .collect(),
            options,
            truncated,
        })
    }

    /// Revalidates selected dynamic-dictionary values against the currently
    /// published Catalog snapshots without executing a search.
    ///
    /// Saved-view migration uses this application-layer seam so personal-state
    /// storage remains independent of Catalog storage. Callers must treat
    /// publication/storage failures as indeterminate; only
    /// [`SharedQueryError::InvalidFilterOption`] proves a stored selection is
    /// no longer present in the active dictionary.
    pub fn validate_dynamic_filter_options(
        &mut self,
        targets: &[TermCampusKey],
        filters: &NormalizedFilterValuesV1,
    ) -> Result<(), SharedQueryError> {
        let response = self.dynamic_filter_validation(targets, filters)?;
        if let Some(invalid) = response.invalid_values.into_iter().next() {
            return Err(SharedQueryError::InvalidFilterOption {
                field: invalid.field,
                value: invalid.value,
            });
        }
        Ok(())
    }

    /// Returns every invalid dynamic field/value pair against one fixed,
    /// ordered Catalog content-version vector. Invalid selections are data in
    /// the successful response, not transport errors.
    pub fn dynamic_filter_validation(
        &mut self,
        targets: &[TermCampusKey],
        filters: &NormalizedFilterValuesV1,
    ) -> Result<DynamicFilterValidationResponseV3, SharedQueryError> {
        prepare_dynamic_filter_validation(self.storage, targets, filters, |_| {})
    }
}

/// Request-time composition over one immutable prepared generation.
///
/// This is the only optimized serving path. It performs no operational SQLite
/// reads and owns no refresh state; callers bind one `Arc` snapshot before
/// target bookkeeping and keep using it through serialization.
pub struct PreparedQueryService<'snapshot> {
    snapshot: &'snapshot PreparedServingSnapshot,
    forced_open_unavailable: Option<&'snapshot BTreeSet<TermCampusKey>>,
}

struct PreparedSearchContext {
    targets: Vec<TermCampusKey>,
    text_hits: Option<TextHitPlan>,
}

impl<'snapshot> PreparedQueryService<'snapshot> {
    pub const fn new(snapshot: &'snapshot PreparedServingSnapshot) -> Self {
        Self {
            snapshot,
            forced_open_unavailable: None,
        }
    }

    pub const fn with_forced_open_unavailable(
        snapshot: &'snapshot PreparedServingSnapshot,
        targets: &'snapshot BTreeSet<TermCampusKey>,
    ) -> Self {
        Self {
            snapshot,
            forced_open_unavailable: Some(targets),
        }
    }

    pub fn from_binding(binding: &'snapshot PreparedServingBinding) -> Self {
        Self::with_forced_open_unavailable(binding, binding.forced_open_unavailable())
    }

    pub fn course_search(
        &self,
        targets: &[TermCampusKey],
        request: &CourseQueryRequestV1,
        now: OffsetDateTime,
    ) -> Result<CourseQueryResponseV1, SharedQueryError> {
        let PreparedSearchContext { targets, text_hits } =
            self.prepare_search(targets, request.filters.values())?;
        trace_prepared_vectors(self.snapshot, &targets, true)?;
        self.query_engine(&targets, now)?
            .course_search(request, text_hits.as_ref())
            .map_err(query_error)
    }

    pub fn section_search(
        &self,
        targets: &[TermCampusKey],
        request: &SectionQueryRequestV1,
        now: OffsetDateTime,
    ) -> Result<SectionQueryResponseV1, SharedQueryError> {
        let PreparedSearchContext { targets, text_hits } =
            self.prepare_search(targets, request.filters.values())?;
        trace_prepared_vectors(self.snapshot, &targets, true)?;
        self.query_engine(&targets, now)?
            .section_search(request, text_hits.as_ref())
            .map_err(query_error)
    }

    pub fn course_detail(
        &self,
        targets: &[TermCampusKey],
        request: &CourseDetailRequestV1,
        now: OffsetDateTime,
    ) -> Result<CourseDetailResponseV1, SharedQueryError> {
        let targets = validate_detail_targets(targets, &request.key.target())?;
        trace_prepared_vectors(self.snapshot, &targets, true)?;
        self.query_engine(&targets, now)?
            .course_detail(request)
            .map_err(query_error)
    }

    pub fn section_detail(
        &self,
        targets: &[TermCampusKey],
        request: &SectionDetailRequestV1,
        now: OffsetDateTime,
    ) -> Result<SectionDetailResponseV1, SharedQueryError> {
        let targets = validate_detail_targets(targets, &request.key.target())?;
        trace_prepared_vectors(self.snapshot, &targets, true)?;
        self.query_engine(&targets, now)?
            .section_detail(request)
            .map_err(query_error)
    }

    pub fn filter_options(
        &self,
        targets: &[TermCampusKey],
        request: &FilterOptionsRequestV2,
    ) -> Result<FilterOptionsResponseV2, SharedQueryError> {
        let targets = canonical_targets(targets)?;
        if targets != request.targets() {
            return Err(SharedQueryError::FilterCampusSetMismatch);
        }
        let catalogs = self.snapshot.catalogs(&targets).map_err(prepared_error)?;
        let requested_limit = if request.field == FilterOptionsFieldV2::CourseNumberBand {
            usize::MAX
        } else {
            request.effective_limit()
        };
        let mut options = self
            .snapshot
            .prepared_filter_options(&targets, request.field, request.query.as_deref())
            .map_err(prepared_error)?;
        let truncated = options.len() > requested_limit;
        options.truncate(requested_limit);
        trace_prepared_vectors(self.snapshot, &targets, false)?;
        Ok(FilterOptionsResponseV2 {
            contract_version: QUERY_CONTRACT_VERSION,
            field: request.field,
            target_versions: catalogs
                .iter()
                .map(|catalog| FilterOptionTargetVersionV2 {
                    target: catalog.target.clone(),
                    content_version: catalog.content_version,
                })
                .collect(),
            options,
            truncated,
        })
    }

    pub fn dynamic_filter_validation(
        &self,
        targets: &[TermCampusKey],
        filters: &NormalizedFilterValuesV1,
    ) -> Result<DynamicFilterValidationResponseV3, SharedQueryError> {
        let targets = validate_search_targets(targets, filters)?;
        let catalogs = self.snapshot.catalogs(&targets).map_err(prepared_error)?;
        let invalid_values = self
            .snapshot
            .invalid_dynamic_filter_values(&targets, filters)
            .map_err(prepared_error)?;
        trace_prepared_vectors(self.snapshot, &targets, false)?;
        Ok(DynamicFilterValidationResponseV3 {
            contract_version: QUERY_CONTRACT_VERSION,
            target_versions: catalogs
                .iter()
                .map(|catalog| FilterOptionTargetVersionV2 {
                    target: catalog.target.clone(),
                    content_version: catalog.content_version,
                })
                .collect(),
            invalid_values,
        })
    }

    fn prepare_search(
        &self,
        targets: &[TermCampusKey],
        filters: &NormalizedFilterValuesV1,
    ) -> Result<PreparedSearchContext, SharedQueryError> {
        let targets = validate_search_targets(targets, filters)?;
        let invalid_values = self
            .snapshot
            .invalid_dynamic_filter_values(&targets, filters)
            .map_err(prepared_error)?;
        if let Some(invalid) = invalid_values.into_iter().next() {
            return Err(SharedQueryError::InvalidFilterOption {
                field: invalid.field,
                value: invalid.value,
            });
        }
        let text_hits = filters
            .text()
            .map(|query| {
                self.snapshot
                    .text_hit_plan(&targets, query)
                    .map_err(prepared_error)
            })
            .transpose()?;
        Ok(PreparedSearchContext { targets, text_hits })
    }

    fn query_engine(
        &self,
        targets: &[TermCampusKey],
        now: OffsetDateTime,
    ) -> Result<QueryEngine<'snapshot>, SharedQueryError> {
        let first = targets.first().ok_or(SharedQueryError::EmptyTargetSet)?;
        let corpus = self.snapshot.query_corpus(first).map_err(prepared_error)?;
        let open = self
            .snapshot
            .query_open_overlays(targets)
            .map_err(prepared_error)?;
        let forced = self
            .forced_open_unavailable
            .map(|forced| {
                targets
                    .iter()
                    .filter(|target| forced.contains(*target))
                    .cloned()
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        QueryEngine::try_new_prepared(corpus, targets, &forced, now, &open).map_err(query_error)
    }
}

fn trace_prepared_vectors(
    snapshot: &PreparedServingSnapshot,
    targets: &[TermCampusKey],
    include_open: bool,
) -> Result<(), SharedQueryError> {
    let bind_started = Instant::now();
    let catalog_vector = snapshot.catalog_vector(targets).map_err(prepared_error)?;
    if include_open {
        let open_vector = snapshot.open_vector(targets).map_err(prepared_error)?;
        tracing::debug!(
            target: "bcsp_performance",
            phase = "prepared_snapshot_bind",
            elapsed_us = elapsed_micros(bind_started),
            catalog_vector = ?catalog_vector,
            open_vector = ?open_vector,
        );
    } else {
        tracing::debug!(
            target: "bcsp_performance",
            phase = "prepared_snapshot_bind",
            elapsed_us = elapsed_micros(bind_started),
            catalog_vector = ?catalog_vector,
        );
    }
    Ok(())
}

fn prepared_error(source: PreparedServingError) -> SharedQueryError {
    SharedQueryError::Prepared {
        source: Box::new(source),
    }
}

fn prepare_dynamic_filter_validation(
    storage: &mut OperationalStorage,
    targets: &[TermCampusKey],
    filters: &NormalizedFilterValuesV1,
    after_validation: impl FnOnce(&mut OperationalStorage),
) -> Result<DynamicFilterValidationResponseV3, SharedQueryError> {
    let targets = validate_search_targets(targets, filters)?;
    let catalogs = load_catalogs(storage, &targets)?;
    let validation_started = Instant::now();
    let invalid_values = collect_invalid_dynamic_filter_values(storage, &catalogs, filters)?;
    tracing::debug!(
        target: "bcsp_performance",
        phase = "dynamic_filter_validation",
        elapsed_us = elapsed_micros(validation_started),
        route = "exact_dynamic_filter_validation",
    );

    // This seam makes the Catalog-vector invariant directly testable. The
    // production route supplies a no-op; any publication that races the
    // validation is caught before a response can expose mixed-version facts.
    after_validation(storage);
    verify_current_versions(storage, &catalogs)?;
    Ok(DynamicFilterValidationResponseV3 {
        contract_version: QUERY_CONTRACT_VERSION,
        target_versions: catalogs
            .iter()
            .map(|catalog| FilterOptionTargetVersionV2 {
                target: catalog.target.clone(),
                content_version: catalog.content_version,
            })
            .collect(),
        invalid_values,
    })
}

struct SearchContext {
    catalogs: Vec<NormalizedCatalogV1>,
    text_hits: Option<TextHitPlan>,
}

fn prepare_search(
    storage: &mut OperationalStorage,
    targets: &[TermCampusKey],
    filters: &NormalizedFilterValuesV1,
    after_load: impl FnOnce(&mut OperationalStorage),
) -> Result<SearchContext, SharedQueryError> {
    let targets = validate_search_targets(targets, filters)?;
    let catalogs = load_catalogs(storage, &targets)?;

    let validation_started = Instant::now();
    validate_dynamic_filter_values(storage, &catalogs, filters)?;
    tracing::debug!(
        target: "bcsp_performance",
        phase = "dynamic_filter_validation",
        elapsed_us = elapsed_micros(validation_started),
    );

    // This seam keeps the version-race behavior directly testable. Production
    // callers always supply the no-op closure through the public methods.
    after_load(storage);

    let text_hits = filters
        .text()
        .map(|query| {
            let tokens = CourseTextSearchTokens::try_new(query.tokens())
                .map_err(|source| SharedQueryError::InvalidTextTokens { source })?;
            let mut bindings = Vec::with_capacity(catalogs.len());
            let mut hits = Vec::new();
            for catalog in &catalogs {
                let search_started = Instant::now();
                let result = storage
                    .search_course_variants(&catalog.target, catalog.content_version.get(), &tokens)
                    .map_err(|source| {
                        map_storage_error(
                            catalog.target.clone(),
                            Some(catalog.content_version.get()),
                            source,
                        )
                    })?;
                tracing::debug!(
                    target: "bcsp_performance",
                    phase = "sqlite_text_search",
                    elapsed_us = elapsed_micros(search_started),
                    target_key = ?catalog.target,
                );
                if result.target != catalog.target
                    || result.content_version != catalog.content_version.get()
                {
                    return Err(SharedQueryError::PublicationChanged {
                        target: catalog.target.clone(),
                        expected: catalog.content_version.get(),
                        current: Some(result.content_version),
                    });
                }
                bindings.push(TextTargetVersion {
                    target: catalog.target.clone(),
                    content_version: catalog.content_version,
                });
                hits.extend(result.hits.into_iter().map(|hit| TextHit {
                    variant_key: hit.key,
                    fts_rank: hit.fts_rank,
                }));
            }
            TextHitPlan::try_new(query, bindings, hits)
                .map_err(|source| SharedQueryError::TextEvidence { source })
        })
        .transpose()?;

    // A publication can change after snapshot projection or between target
    // searches. Rechecking every binding makes that preparation fail closed.
    verify_current_versions(storage, &catalogs)?;
    Ok(SearchContext {
        catalogs,
        text_hits,
    })
}

fn validate_search_targets(
    targets: &[TermCampusKey],
    filters: &NormalizedFilterValuesV1,
) -> Result<Vec<TermCampusKey>, SharedQueryError> {
    let targets = canonical_targets(targets)?;
    for target in &targets {
        if target.term() != filters.term() {
            return Err(SharedQueryError::FilterTermMismatch {
                target: target.clone(),
                filter_term: filters.term().as_str().to_owned(),
            });
        }
    }
    if !filters.campuses().is_empty() {
        let actual = targets
            .iter()
            .map(|target| target.campus().clone())
            .collect::<BTreeSet<_>>();
        let expected = filters.campuses().iter().cloned().collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(SharedQueryError::FilterCampusSetMismatch);
        }
    }
    Ok(targets)
}

fn validate_detail_targets(
    targets: &[TermCampusKey],
    detail_target: &TermCampusKey,
) -> Result<Vec<TermCampusKey>, SharedQueryError> {
    let targets = canonical_targets(targets)?;
    if !targets.contains(detail_target) {
        return Err(SharedQueryError::DetailTargetMismatch);
    }
    Ok(targets)
}

fn canonical_targets(targets: &[TermCampusKey]) -> Result<Vec<TermCampusKey>, SharedQueryError> {
    if targets.is_empty() {
        return Err(SharedQueryError::EmptyTargetSet);
    }
    let mut unique = BTreeSet::new();
    for target in targets {
        if !unique.insert(target.clone()) {
            return Err(SharedQueryError::DuplicateTarget {
                target: target.clone(),
            });
        }
    }
    Ok(unique.into_iter().collect())
}

fn load_catalogs(
    storage: &mut OperationalStorage,
    targets: &[TermCampusKey],
) -> Result<Vec<NormalizedCatalogV1>, SharedQueryError> {
    let mut catalogs = Vec::with_capacity(targets.len());
    for target in targets {
        let load_started = Instant::now();
        let published = storage
            .published_catalog_snapshot(target)
            .map_err(|source| map_storage_error(target.clone(), None, source))?
            .ok_or_else(|| SharedQueryError::TargetNotPublished {
                target: target.clone(),
            })?;
        tracing::debug!(
            target: "bcsp_performance",
            phase = "sqlite_catalog_load",
            elapsed_us = elapsed_micros(load_started),
            target_key = ?target,
        );
        let projection_started = Instant::now();
        let catalog = to_normalized_catalog_v1(&published).map_err(|source| {
            SharedQueryError::Projection {
                target: target.clone(),
                source,
            }
        })?;
        tracing::debug!(
            target: "bcsp_performance",
            phase = "catalog_projection",
            elapsed_us = elapsed_micros(projection_started),
            target_key = ?target,
            groups = catalog.course_groups.len(),
            variants = catalog.course_variants.len(),
            sections = catalog.sections.len(),
        );
        catalogs.push(catalog);
    }
    Ok(catalogs)
}

fn elapsed_micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn verify_current_versions(
    storage: &OperationalStorage,
    catalogs: &[NormalizedCatalogV1],
) -> Result<(), SharedQueryError> {
    for catalog in catalogs {
        let state = storage
            .target_state(&catalog.target)
            .map_err(|source| map_storage_error(catalog.target.clone(), None, source))?;
        let current = state
            .as_ref()
            .map(|state| state.current_content_version)
            .filter(|version| *version != 0);
        if current != Some(catalog.content_version.get()) {
            return Err(SharedQueryError::PublicationChanged {
                target: catalog.target.clone(),
                expected: catalog.content_version.get(),
                current,
            });
        }
    }
    Ok(())
}

fn map_storage_error(
    target: TermCampusKey,
    expected: Option<u64>,
    source: StorageError,
) -> SharedQueryError {
    match source {
        StorageError::Fts5Unavailable => SharedQueryError::FtsUnavailable { target },
        StorageError::CatalogTargetNotPublished => SharedQueryError::TargetNotPublished { target },
        StorageError::CatalogContentVersionMismatch { requested, current } => {
            SharedQueryError::PublicationChanged {
                target,
                expected: expected.unwrap_or(requested),
                current: Some(current),
            }
        }
        source => SharedQueryError::Storage { target, source },
    }
}

fn query_error(source: QueryError) -> SharedQueryError {
    SharedQueryError::Query { source }
}

pub(crate) fn known_value<T>(knowledge: &CatalogFieldKnowledge<T>) -> Option<&T> {
    match knowledge {
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Present { value },
        } => Some(value),
        CatalogFieldKnowledge::Known { .. } | CatalogFieldKnowledge::Unknown { .. } => None,
    }
}

pub(crate) fn insert_option(
    options: &mut BTreeMap<String, FilterOptionV2>,
    value: String,
    label: String,
) {
    let key = value.to_lowercase();
    options
        .entry(key)
        .and_modify(|existing| {
            if label.len() > existing.label.len() {
                existing.label.clone_from(&label);
            }
        })
        .or_insert(FilterOptionV2 { value, label });
}

fn query_matches(value: &str, query: Option<&str>) -> bool {
    query.is_none_or(|query| value.to_lowercase().contains(&query.to_lowercase()))
}

pub(crate) fn collect_catalog_options(
    catalogs: &[&NormalizedCatalogV1],
    field: FilterOptionsFieldV2,
    query: Option<&str>,
) -> BTreeMap<String, FilterOptionV2> {
    let mut options = BTreeMap::new();
    match field {
        FilterOptionsFieldV2::Keyword => {}
        FilterOptionsFieldV2::CourseNumberBand => {
            for variant in catalogs.iter().flat_map(|catalog| &catalog.course_variants) {
                if let Some(value) = known_value(&variant.course_number)
                    && let Some(band) = course_number_band(value)
                {
                    let upper = band + 99;
                    insert_option(
                        &mut options,
                        band.to_string(),
                        format!("{band:03}-{upper:03}"),
                    );
                }
            }
        }
        FilterOptionsFieldV2::CourseLevel => {
            for variant in catalogs.iter().flat_map(|catalog| &catalog.course_variants) {
                if let Some(value) = known_value(&variant.level) {
                    let value = value.trim().to_ascii_uppercase();
                    if !value.is_empty() {
                        let label = match value.as_str() {
                            "U" => "U · Undergraduate".to_owned(),
                            "G" => "G · Graduate".to_owned(),
                            _ => value.clone(),
                        };
                        insert_option(&mut options, value, label);
                    }
                }
            }
        }
        FilterOptionsFieldV2::Instructor => {
            for section in catalogs.iter().flat_map(|catalog| &catalog.sections) {
                if let Some(values) = known_value(&section.instructors) {
                    for value in values {
                        let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
                        if !normalized.is_empty() && query_matches(&normalized, query) {
                            insert_option(&mut options, normalized.clone(), normalized);
                        }
                    }
                }
            }
        }
        FilterOptionsFieldV2::MeetingLocation => {
            for occurrence in catalogs.iter().flat_map(|catalog| &catalog.occurrences) {
                let code = known_value(&occurrence.campus)
                    .map(|value| value.trim().to_ascii_uppercase())
                    .filter(|value| !value.is_empty());
                let name = known_value(&occurrence.campus_name)
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty());
                match (code, name) {
                    (Some(code), Some(name)) => {
                        insert_option(&mut options, code.clone(), format!("{code} · {name}"));
                    }
                    (Some(code), None) => insert_option(&mut options, code.clone(), code),
                    (None, Some(name)) => {
                        insert_option(&mut options, name.to_ascii_uppercase(), name)
                    }
                    (None, None) => {}
                }
            }
        }
        FilterOptionsFieldV2::ExamCode => {
            for section in catalogs.iter().flat_map(|catalog| &catalog.sections) {
                let code = known_value(&section.exam_code)
                    .map(|value| value.trim().to_ascii_uppercase())
                    .filter(|value| !value.is_empty());
                let description = known_value(&section.exam_code_text)
                    .map(|value| value.trim().to_owned())
                    .filter(|value| !value.is_empty());
                match (code, description) {
                    (Some(code), Some(description)) => insert_option(
                        &mut options,
                        code.clone(),
                        format!("{code} · {description}"),
                    ),
                    (Some(code), None) => insert_option(&mut options, code.clone(), code),
                    (None, Some(description)) => {
                        insert_option(&mut options, description.to_ascii_uppercase(), description)
                    }
                    (None, None) => {}
                }
            }
        }
    }
    options
}

fn contains_selected_option(options: &BTreeMap<String, FilterOptionV2>, selected: &str) -> bool {
    options.contains_key(&selected.to_lowercase())
}

fn collect_invalid_tokens(
    options: &BTreeMap<String, FilterOptionV2>,
    values: &[FilterTokenV1],
    field: FilterFieldId,
) -> Vec<InvalidDynamicFilterValueV3> {
    values
        .iter()
        .filter(|value| !contains_selected_option(options, value.as_str()))
        .map(|value| InvalidDynamicFilterValueV3 {
            field,
            value: value.as_str().to_owned(),
        })
        .collect()
}

fn validate_dynamic_filter_values(
    storage: &mut OperationalStorage,
    catalogs: &[NormalizedCatalogV1],
    filters: &NormalizedFilterValuesV1,
) -> Result<(), SharedQueryError> {
    if let Some(invalid) = collect_invalid_dynamic_filter_values(storage, catalogs, filters)?
        .into_iter()
        .next()
    {
        return Err(SharedQueryError::InvalidFilterOption {
            field: invalid.field,
            value: invalid.value,
        });
    }
    Ok(())
}

fn collect_invalid_dynamic_filter_values(
    storage: &mut OperationalStorage,
    catalogs: &[NormalizedCatalogV1],
    filters: &NormalizedFilterValuesV1,
) -> Result<Vec<InvalidDynamicFilterValueV3>, SharedQueryError> {
    let catalog_refs = catalogs.iter().collect::<Vec<_>>();
    collect_invalid_dynamic_filter_values_with(&catalog_refs, filters, |catalog, keyword| {
        storage
            .published_course_fts_term_exists(
                &catalog.target,
                catalog.content_version.get(),
                keyword,
            )
            .map_err(|source| {
                map_storage_error(
                    catalog.target.clone(),
                    Some(catalog.content_version.get()),
                    source,
                )
            })
    })
}

fn collect_invalid_dynamic_filter_values_with(
    catalogs: &[&NormalizedCatalogV1],
    filters: &NormalizedFilterValuesV1,
    mut keyword_exists: impl FnMut(&NormalizedCatalogV1, &str) -> Result<bool, SharedQueryError>,
) -> Result<Vec<InvalidDynamicFilterValueV3>, SharedQueryError> {
    let mut invalid_values = Vec::new();
    if let Some(keywords) = filters.keywords() {
        for keyword in keywords.tokens() {
            let mut found = false;
            for catalog in catalogs {
                if catalog.course_groups.iter().any(|group| {
                    group
                        .key
                        .course_string()
                        .as_str()
                        .eq_ignore_ascii_case(keyword)
                }) {
                    found = true;
                    break;
                }
                found = keyword_exists(catalog, keyword)?;
                if found {
                    break;
                }
            }
            if !found {
                invalid_values.push(InvalidDynamicFilterValueV3 {
                    field: FilterFieldId::CourseText,
                    value: keyword.clone(),
                });
            }
        }
    }

    let subject_options = catalogs
        .iter()
        .flat_map(|catalog| &catalog.course_variants)
        .filter_map(|variant| known_value(&variant.subject_code))
        .map(|value| value.as_str())
        .collect::<BTreeSet<_>>();
    invalid_values.extend(
        filters
            .subjects()
            .iter()
            .filter(|value| !subject_options.contains(value.as_str()))
            .map(|value| InvalidDynamicFilterValueV3 {
                field: FilterFieldId::CourseSubject,
                value: value.as_str().to_owned(),
            }),
    );

    invalid_values.extend(collect_invalid_tokens(
        &collect_catalog_options(catalogs, FilterOptionsFieldV2::CourseLevel, None),
        filters.levels(),
        FilterFieldId::CourseLevel,
    ));
    let band_options =
        collect_catalog_options(catalogs, FilterOptionsFieldV2::CourseNumberBand, None);
    for band in filters.course_number_bands() {
        if !contains_selected_option(&band_options, &band.to_string()) {
            invalid_values.push(InvalidDynamicFilterValueV3 {
                field: FilterFieldId::CourseNumberBand,
                value: band.to_string(),
            });
        }
    }

    // Core codes are mixed-case in the feed ("AHo") while a request's codes
    // are canonical uppercase; the engine compares them case-insensitively,
    // so the validator must too.
    let core_options = catalogs
        .iter()
        .flat_map(|catalog| &catalog.course_variants)
        .filter_map(|variant| known_value(&variant.core_codes))
        .flatten()
        .map(|value| value.to_ascii_uppercase())
        .collect::<BTreeSet<_>>();
    invalid_values.extend(
        filters
            .core()
            .codes
            .iter()
            .filter(|value| !core_options.contains(value.as_str()))
            .map(|value| InvalidDynamicFilterValueV3 {
                field: FilterFieldId::CourseCoreCode,
                value: value.as_str().to_owned(),
            }),
    );

    invalid_values.extend(collect_invalid_tokens(
        &collect_catalog_options(catalogs, FilterOptionsFieldV2::Instructor, None),
        filters.instructors(),
        FilterFieldId::SectionInstructor,
    ));
    invalid_values.extend(collect_invalid_tokens(
        &collect_catalog_options(catalogs, FilterOptionsFieldV2::MeetingLocation, None),
        &filters.meeting_locations().locations,
        FilterFieldId::SectionMeetingLocation,
    ));
    invalid_values.extend(collect_invalid_tokens(
        &collect_catalog_options(catalogs, FilterOptionsFieldV2::ExamCode, None),
        filters.exam_codes(),
        FilterFieldId::SectionExam,
    ));
    invalid_values.sort();
    invalid_values.dedup();
    Ok(invalid_values)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration as StdDuration;

    use bcsp_catalog::{normalize_target, to_catalog_refresh_command};
    use bcsp_contracts::{
        CampusCode, CatalogSubjectCode, CourseDetailRequestV1, CourseQueryRequestV1, CourseSortV1,
        FilterFieldId, FilterOptionsFieldV2, FilterOptionsRequestV2, FilterRequestV1,
        FilterSearchTextV1, FilterTokenV1, FilterValuesInputV1, MatchOutcome,
        NormalizedFilterValuesV1, PageRequestV1, PrerequisiteFilterV1, QUERY_CONTRACT_VERSION,
        SectionDetailRequestV1, SectionKey, SectionQueryRequestV1, SectionSortV1, TermId, TraceId,
        UserModalityV3, UserSynchronicityV3,
    };
    use bcsp_open::{GeneralOpenInterval, OpenCounterAudience};
    use bcsp_operational_storage::{
        BeginOpenPullAttemptCommand, EmptySnapshotDecision, FinishOpenPullSuccessCommand,
        OpenCacheStatus, OpenHttpAuditMetadata, OpenRequestLane, OperationalStorage,
        PublishOutcome,
    };
    use bcsp_rutgers_client::{SourceProvenance, decode_catalog_payload};
    use serde_json::{Value, json};
    use time::OffsetDateTime;

    use super::*;

    const STARTED: &str = "2030-01-01T00:00:00Z";
    const COMPLETED: &str = "2030-01-01T00:00:01Z";

    fn target(campus: &str) -> TermCampusKey {
        TermCampusKey::try_new("92026", campus).expect("synthetic target")
    }

    fn trace(suffix: u8) -> TraceId {
        format!("00000000-0000-4000-8000-{suffix:012x}")
            .parse()
            .expect("synthetic trace ID")
    }

    fn raw_course(
        campus: &str,
        course_string: &str,
        subject: &str,
        course_number: &str,
        title: &str,
        index: &str,
    ) -> Value {
        json!({
            "campusCode": campus,
            "courseString": course_string,
            "subject": subject,
            "courseNumber": course_number,
            "title": title,
            "level": "U",
            "sections": [{
                "campusCode": campus,
                "index": index,
                "number": "01",
                "sectionCourseType": "LECTURE",
                "openStatus": true,
                "meetingTimes": [],
                "instructors": [{"name": "Pat Smith"}],
                "examCode": "F",
                "examCodeText": "Final exam"
            }]
        })
    }

    fn publish(
        storage: &mut OperationalStorage,
        target: &TermCampusKey,
        courses: Vec<Value>,
        observation: u8,
    ) {
        let body = serde_json::to_vec(&courses).expect("synthetic Catalog body");
        let normalized = normalize_target(
            target.clone(),
            decode_catalog_payload(&body).expect("decode synthetic Catalog"),
            SourceProvenance::from_body("SYNTHETIC_APPLICATION_QUERY", STARTED, &body),
        )
        .expect("normalize synthetic Catalog");
        let outcome = storage
            .apply_catalog_refresh(
                to_catalog_refresh_command(&normalized, trace(observation), STARTED, COMPLETED)
                    .expect("map synthetic Catalog"),
                EmptySnapshotDecision::AcceptNonEmptyOrUnchangedEmpty,
                bcsp_catalog::CATALOG_DERIVATION_VERSION,
            )
            .expect("publish synthetic Catalog");
        assert!(matches!(outcome, PublishOutcome::AppliedChanged { .. }));
    }

    fn filters(text: &str) -> NormalizedFilterValuesV1 {
        let mut input =
            FilterValuesInputV1::for_term(TermId::try_from("92026").expect("synthetic term"));
        input.text = Some(FilterSearchTextV1::try_from(text).expect("synthetic search text"));
        NormalizedFilterValuesV1::try_new(input).expect("synthetic filters")
    }

    fn course_request(text: &str, page: u32, page_size: u16) -> CourseQueryRequestV1 {
        CourseQueryRequestV1 {
            filters: FilterRequestV1::new(filters(text)),
            page: PageRequestV1::try_new(page, page_size).expect("synthetic page"),
            sort: CourseSortV1::default(),
        }
    }

    fn section_request(text: &str) -> SectionQueryRequestV1 {
        SectionQueryRequestV1 {
            filters: FilterRequestV1::new(filters(text)),
            page: PageRequestV1::default(),
            sort: SectionSortV1::default(),
        }
    }

    fn fixture_storage() -> (OperationalStorage, TermCampusKey, TermCampusKey) {
        let mut storage = OperationalStorage::open_in_memory().expect("in-memory storage");
        let nb = target("NB");
        let nwk = target("NWK");
        publish(
            &mut storage,
            &nb,
            vec![
                raw_course(
                    "NB",
                    "01:198:111",
                    "198",
                    "111",
                    "Computer Science Foundations",
                    "10001",
                ),
                raw_course(
                    "NB",
                    "01:198:112",
                    "198",
                    "112",
                    "01 198 111 Companion",
                    "10002",
                ),
                raw_course("NB", "01:960:100", "960", "100", "Statistics", "10003"),
            ],
            1,
        );
        publish(
            &mut storage,
            &nwk,
            vec![raw_course(
                "NWK",
                "21:198:101",
                "198",
                "101",
                "Computer Science Newark",
                "20001",
            )],
            2,
        );
        (storage, nb, nwk)
    }

    fn publish_open_for_prepared(
        storage: &mut OperationalStorage,
        target: &TermCampusKey,
        attempt_suffix: u8,
    ) {
        let catalog_content_version = storage
            .target_state(target)
            .expect("load fixture Catalog state")
            .expect("fixture Catalog state")
            .current_content_version;
        let section =
            SectionKey::try_new(target.term().as_str(), target.campus().as_str(), "10001")
                .expect("fixture section");
        storage
            .begin_open_pull_attempt(&BeginOpenPullAttemptCommand {
                attempt_id: trace(attempt_suffix),
                run_id: trace(attempt_suffix.saturating_add(1)),
                target: target.clone(),
                captured_catalog_content_version: catalog_content_version,
                rutgers_day: "2026-07-17".to_owned(),
                started_at: STARTED.to_owned(),
                lane: OpenRequestLane::ActiveWatch,
                requested_interval_seconds: Some(30),
                effective_interval_seconds: Some(10),
                schedule_lag_ms: Some(0),
            })
            .expect("begin Open fixture");
        storage
            .finish_open_pull_success(FinishOpenPullSuccessCommand {
                gate_hold: false,
                gate_catalog_set_identity: None,
                attempt_id: trace(attempt_suffix),
                completed_at: COMPLETED.to_owned(),
                open_sections: vec![section],
                source_value_count: 1,
                watched_sections: Vec::new(),
                http: OpenHttpAuditMetadata {
                    http_status: Some(200),
                    cache_status: Some(OpenCacheStatus::Miss),
                    decoded_bytes: Some(2),
                    decoded_body_sha256: Some("a".repeat(64)),
                    content_type: Some("application/json".to_owned()),
                    etag: None,
                    cache_control: None,
                    date: None,
                    age_seconds: None,
                    last_modified: None,
                    retry_after: None,
                    retry_after_seconds: None,
                },
            })
            .expect("finish Open fixture");
    }

    #[test]
    fn filter_options_use_published_fts_and_case_insensitive_instructor_substrings() {
        let (mut storage, nb, _) = fixture_storage();
        let mut service = SharedQueryService::new(&mut storage);
        for query in ["smi", "Smi", "SMI"] {
            let mut request = FilterOptionsRequestV2 {
                contract_version: QUERY_CONTRACT_VERSION,
                term: TermId::try_from("92026").unwrap(),
                campuses: vec![CampusCode::try_from("NB").unwrap()],
                field: FilterOptionsFieldV2::Instructor,
                query: Some(query.to_owned()),
                limit: None,
            };
            request.validate().unwrap();
            let response = service
                .filter_options(std::slice::from_ref(&nb), &request)
                .expect("published instructor options");
            assert_eq!(response.options.len(), 1);
            assert_eq!(response.options[0].value, "Pat Smith");
        }

        let mut request = FilterOptionsRequestV2 {
            contract_version: QUERY_CONTRACT_VERSION,
            term: TermId::try_from("92026").unwrap(),
            campuses: vec![CampusCode::try_from("NB").unwrap()],
            field: FilterOptionsFieldV2::Keyword,
            query: Some("comp".to_owned()),
            limit: Some(10),
        };
        request.validate().unwrap();
        let response = service
            .filter_options(&[nb], &request)
            .expect("published FTS terms");
        assert!(
            response
                .options
                .iter()
                .any(|option| option.value == "computer")
        );
    }

    #[test]
    fn course_number_band_options_cover_the_complete_catalog_and_bind_its_version() {
        let mut storage = OperationalStorage::open_in_memory().expect("in-memory storage");
        let nb = target("NB");
        publish(
            &mut storage,
            &nb,
            vec![
                raw_course("NB", "01:198:001", "198", "001", "Zero band", "10001"),
                raw_course("NB", "01:198:099", "198", "099", "Zero band high", "10002"),
                raw_course("NB", "01:198:100", "198", "100", "One band", "10003"),
                raw_course("NB", "01:198:499", "198", "499", "Four band", "10004"),
            ],
            9,
        );
        let mut request = FilterOptionsRequestV2 {
            contract_version: QUERY_CONTRACT_VERSION,
            term: TermId::try_from("92026").unwrap(),
            campuses: vec![CampusCode::try_from("NB").unwrap()],
            field: FilterOptionsFieldV2::CourseNumberBand,
            query: None,
            limit: Some(1),
        };
        request.validate().unwrap();
        let response = SharedQueryService::new(&mut storage)
            .filter_options(std::slice::from_ref(&nb), &request)
            .expect("complete dynamic bands");
        assert_eq!(
            response
                .options
                .iter()
                .map(|option| option.value.as_str())
                .collect::<Vec<_>>(),
            ["0", "100", "400"]
        );
        assert!(!response.truncated);
        assert_eq!(response.target_versions.len(), 1);
        assert_eq!(response.target_versions[0].target, nb);
        assert_eq!(response.target_versions[0].content_version.get(), 1);
    }

    #[test]
    fn dynamic_filter_revalidation_distinguishes_published_and_removed_values() {
        let (mut storage, nb, _) = fixture_storage();
        let mut input =
            FilterValuesInputV1::for_term(TermId::try_from("92026").expect("synthetic term"));
        input.campuses = vec![CampusCode::try_from("NB").expect("synthetic campus")];
        input.instructors = vec![FilterTokenV1::try_from("Pat Smith").expect("valid token")];
        let valid = NormalizedFilterValuesV1::try_new(input.clone()).expect("valid filters");
        SharedQueryService::new(&mut storage)
            .validate_dynamic_filter_options(std::slice::from_ref(&nb), &valid)
            .expect("published instructor remains valid");

        input.instructors =
            vec![FilterTokenV1::try_from("Removed Instructor").expect("valid token")];
        let invalid = NormalizedFilterValuesV1::try_new(input).expect("valid filters");
        let error = SharedQueryService::new(&mut storage)
            .validate_dynamic_filter_options(std::slice::from_ref(&nb), &invalid)
            .expect_err("removed instructor must be rejected");
        assert!(matches!(
            error,
            SharedQueryError::InvalidFilterOption {
                field: FilterFieldId::SectionInstructor,
                ref value,
            } if value == "Removed Instructor"
        ));
    }

    #[test]
    fn exact_dynamic_validation_is_independent_of_keyword_and_instructor_option_windows() {
        let mut storage = OperationalStorage::open_in_memory().expect("in-memory storage");
        let nb = target("NB");
        let mut courses = Vec::new();
        for ordinal in 0..=101_u32 {
            let exact = ordinal == 101;
            let mut course = raw_course(
                "NB",
                &format!("01:198:{ordinal:03}"),
                "198",
                &format!("{ordinal:03}"),
                if exact {
                    "zneedle".to_owned()
                } else {
                    format!("a{ordinal:03}zneedle")
                }
                .as_str(),
                &format!("{ordinal:05}"),
            );
            course["sections"][0]["instructors"] = json!([{
                "name": if exact {
                    "Zed Needle".to_owned()
                } else {
                    format!("A{ordinal:03} Zed Needle")
                }
            }]);
            courses.push(course);
        }
        publish(&mut storage, &nb, courses, 10);

        let limited_terms = storage
            .published_course_fts_terms(&nb, 1, Some("zneedle"), 100)
            .expect("legacy option window");
        assert_eq!(limited_terms.len(), 100);
        assert!(!limited_terms.iter().any(|term| term == "zneedle"));
        let mut instructor_request = FilterOptionsRequestV2 {
            contract_version: QUERY_CONTRACT_VERSION,
            term: TermId::try_from("92026").unwrap(),
            campuses: vec![CampusCode::try_from("NB").unwrap()],
            field: FilterOptionsFieldV2::Instructor,
            query: Some("Zed Needle".to_owned()),
            limit: Some(100),
        };
        instructor_request.validate().unwrap();
        let limited_instructors = SharedQueryService::new(&mut storage)
            .filter_options(std::slice::from_ref(&nb), &instructor_request)
            .expect("legacy instructor option window");
        assert!(limited_instructors.truncated);
        assert!(
            !limited_instructors
                .options
                .iter()
                .any(|option| option.value == "Zed Needle")
        );

        let mut input = FilterValuesInputV1::for_term(TermId::try_from("92026").unwrap());
        input.campuses = vec![CampusCode::try_from("NB").unwrap()];
        input.text = Some(FilterSearchTextV1::try_from("zneedle").unwrap());
        input.instructors = vec![FilterTokenV1::try_from("Zed Needle").unwrap()];
        let filters = NormalizedFilterValuesV1::try_new(input).unwrap();
        let validation = SharedQueryService::new(&mut storage)
            .dynamic_filter_validation(std::slice::from_ref(&nb), &filters)
            .expect("exact validation is not option discovery");
        assert!(validation.invalid_values.is_empty());
        assert_eq!(validation.target_versions.len(), 1);
        assert_eq!(validation.target_versions[0].target, nb);
        assert_eq!(validation.target_versions[0].content_version.get(), 1);
    }

    #[test]
    fn real_operational_fts_composes_all_query_surfaces_and_prioritizes_exact_identifier() {
        let (mut storage, nb, nwk) = fixture_storage();
        let targets = vec![nwk.clone(), nb.clone()];
        let mut service = SharedQueryService::new(&mut storage);

        let exact = service
            .course_search(
                &targets,
                &course_request("01:198:111", 1, 20),
                OffsetDateTime::UNIX_EPOCH,
                &[],
            )
            .expect("real FTS course search");
        assert_eq!(exact.page.total, 2, "both literal phrase hits survive FTS");
        assert_eq!(
            exact.items[0].group.key.course_string().as_str(),
            "01:198:111",
            "exact Rutgers identifier outranks a non-exact FTS phrase hit"
        );
        assert!(
            exact.items[0].variants[0]
                .text_match
                .as_ref()
                .expect("text evidence")
                .exact_course_identifier
        );

        // Empty C02 means all explicitly supplied targets. Filtering happens
        // before total/page, so the second page retains the two-hit total.
        let paged = service
            .course_search(
                &targets,
                &course_request("Computer Science", 2, 1),
                OffsetDateTime::UNIX_EPOCH,
                &[],
            )
            .expect("multi-target paged course search");
        assert_eq!(paged.page.total, 2);
        assert_eq!(paged.page.total_pages, 2);
        assert_eq!(paged.items.len(), 1);

        let sections = service
            .section_search(
                &targets,
                &section_request("Computer Science"),
                OffsetDateTime::UNIX_EPOCH,
                &[],
            )
            .expect("independent Section search");
        assert_eq!(sections.page.total, 2);
        assert_eq!(sections.items.len(), 2);
        assert!(sections.items.iter().all(|item| {
            item.variant
                .key
                .group()
                .course_string()
                .as_str()
                .contains("198")
        }));

        let course_key = exact.items[0].group.key.clone();
        let detail = service
            .course_detail(
                &targets,
                &CourseDetailRequestV1::new(course_key.clone()),
                OffsetDateTime::UNIX_EPOCH,
                &[],
            )
            .expect("direct Course detail");
        assert_eq!(detail.course.group.key, course_key);

        let section_key =
            SectionKey::try_new("92026", "NB", "10001").expect("synthetic section key");
        let detail = service
            .section_detail(
                &targets,
                &SectionDetailRequestV1::new(section_key.clone()),
                OffsetDateTime::UNIX_EPOCH,
                &[],
            )
            .expect("direct Section detail");
        assert_eq!(detail.section.section.key, section_key);
    }

    #[test]
    fn publication_replacement_between_projection_and_fts_fails_closed() {
        let mut storage = OperationalStorage::open_in_memory().expect("in-memory storage");
        let scope = target("NB");
        publish(
            &mut storage,
            &scope,
            vec![raw_course(
                "NB",
                "01:198:111",
                "198",
                "111",
                "Computer Science Before",
                "10001",
            )],
            10,
        );
        let values = filters("Computer Science");
        let error = prepare_search(
            &mut storage,
            std::slice::from_ref(&scope),
            &values,
            |storage| {
                publish(
                    storage,
                    &scope,
                    vec![raw_course(
                        "NB",
                        "01:198:111",
                        "198",
                        "111",
                        "Computer Science After",
                        "10001",
                    )],
                    11,
                );
            },
        )
        .err()
        .expect("replacement must not produce stale or empty results");
        assert!(matches!(
            error,
            SharedQueryError::PublicationChanged {
                expected: 1,
                current: Some(2),
                ..
            }
        ));
    }

    #[test]
    fn dynamic_validation_rejects_a_catalog_publication_race_instead_of_mixing_versions() {
        let mut storage = OperationalStorage::open_in_memory().expect("in-memory storage");
        let scope = target("NB");
        publish(
            &mut storage,
            &scope,
            vec![raw_course(
                "NB",
                "01:198:111",
                "198",
                "111",
                "Before",
                "10001",
            )],
            20,
        );
        let mut input = FilterValuesInputV1::for_term(TermId::try_from("92026").unwrap());
        input.campuses = vec![CampusCode::try_from("NB").unwrap()];
        input.subjects = vec![CatalogSubjectCode::try_from("198").unwrap()];
        let filters = NormalizedFilterValuesV1::try_new(input).unwrap();

        let error = prepare_dynamic_filter_validation(
            &mut storage,
            std::slice::from_ref(&scope),
            &filters,
            |storage| {
                publish(
                    storage,
                    &scope,
                    vec![raw_course(
                        "NB",
                        "01:960:100",
                        "960",
                        "100",
                        "After",
                        "10002",
                    )],
                    21,
                );
            },
        )
        .expect_err("a changed Catalog vector must fail closed");
        assert!(matches!(
            error,
            SharedQueryError::PublicationChanged {
                expected: 1,
                current: Some(2),
                ..
            }
        ));
    }

    #[test]
    fn target_validation_and_unpublished_targets_are_typed_failures() {
        let (mut storage, nb, nwk) = fixture_storage();
        let request = course_request("Computer Science", 1, 20);
        let error = SharedQueryService::new(&mut storage)
            .course_search(
                &[nb.clone(), nb.clone()],
                &request,
                OffsetDateTime::UNIX_EPOCH,
                &[],
            )
            .expect_err("duplicate targets fail");
        assert!(matches!(error, SharedQueryError::DuplicateTarget { .. }));

        let missing = target("CAMDEN");
        let error = SharedQueryService::new(&mut storage)
            .course_search(
                &[nb, nwk, missing.clone()],
                &request,
                OffsetDateTime::UNIX_EPOCH,
                &[],
            )
            .expect_err("unpublished target must not look empty");
        assert!(matches!(
            error,
            SharedQueryError::TargetNotPublished { target } if target == missing
        ));
    }

    #[test]
    fn prepared_snapshot_matches_reference_options_validation_search_and_details() {
        let (mut storage, nb, _) = fixture_storage();
        let mut known_match = raw_course(
            "NB",
            "01:198:211",
            "198",
            "211",
            "Computer Science known match",
            "10001",
        );
        known_match["preReqNotes"] = json!("01:198:110");
        known_match["sections"][0]["sectionCourseType"] = json!("O");
        known_match["sections"][0]["meetingTimes"] = json!([{
            "meetingModeCode": "92",
            "meetingDay": "M",
            "startTimeMilitary": "0900",
            "endTimeMilitary": "1020",
            "campusLocation": "O"
        }]);
        let uncertain = raw_course(
            "NB",
            "01:198:212",
            "198",
            "212",
            "Computer Science uncertain",
            "10002",
        );
        let mut prerequisite_mismatch = raw_course(
            "NB",
            "01:198:213",
            "198",
            "213",
            "Computer Science prerequisite mismatch",
            "10003",
        );
        prerequisite_mismatch["preReqNotes"] = json!("");
        prerequisite_mismatch["sections"][0]["sectionCourseType"] = json!("O");
        prerequisite_mismatch["sections"][0]["meetingTimes"] =
            known_match["sections"][0]["meetingTimes"].clone();
        let mut modality_mismatch = raw_course(
            "NB",
            "01:198:214",
            "198",
            "214",
            "Computer Science modality mismatch",
            "10004",
        );
        modality_mismatch["preReqNotes"] = json!("01:198:110");
        modality_mismatch["sections"][0]["sectionCourseType"] = json!("T");
        modality_mismatch["sections"][0]["meetingTimes"] = json!([{
            "meetingModeCode": "02",
            "meetingDay": "M",
            "startTimeMilitary": "0900",
            "endTimeMilitary": "1020",
            "campusLocation": "NB"
        }]);
        let mut synchronicity_mismatch = raw_course(
            "NB",
            "01:198:215",
            "198",
            "215",
            "Computer Science synchronicity mismatch",
            "10005",
        );
        synchronicity_mismatch["preReqNotes"] = json!("01:198:110");
        synchronicity_mismatch["sections"][0]["sectionCourseType"] = json!("O");
        synchronicity_mismatch["sections"][0]["meetingTimes"] = json!([{
            "meetingModeCode": "93",
            "campusLocation": "O"
        }]);
        publish(
            &mut storage,
            &nb,
            vec![
                known_match,
                uncertain,
                prerequisite_mismatch,
                modality_mismatch,
                synchronicity_mismatch,
            ],
            4,
        );
        let cm = target("CM");
        publish(
            &mut storage,
            &cm,
            vec![raw_course(
                "CM",
                "50:198:211",
                "198",
                "211",
                "Computer Science Camden α",
                "10001",
            )],
            3,
        );
        publish_open_for_prepared(&mut storage, &nb, 90);
        publish_open_for_prepared(&mut storage, &cm, 92);
        let now = OffsetDateTime::from_unix_timestamp(1_784_289_602).expect("fixed 2026-07-17");
        let policy = crate::RefreshPolicy::try_new(
            StdDuration::from_secs(600),
            GeneralOpenInterval::public(),
        )
        .expect("policy");
        let runtime = crate::SharedRuntimeContext::new(
            OpenCounterAudience::Public,
            move || now,
            crate::FixedRefreshPolicyProvider::new(policy),
        );
        let open_runtime = Arc::new(crate::OpenRuntimeSnapshotRegistry::default());
        let snapshot = crate::build_prepared_serving_snapshot(
            &mut storage,
            &runtime,
            &open_runtime,
            bcsp_contracts::ServiceRuntimeV1::Local,
        )
        .expect("prepared snapshot");
        let prepared = PreparedQueryService::new(&snapshot);

        for (query, should_match) in [("Α", false), ("α", true)] {
            let mut request = FilterOptionsRequestV2 {
                contract_version: QUERY_CONTRACT_VERSION,
                term: TermId::try_from("92026").unwrap(),
                campuses: vec![CampusCode::try_from("CM").unwrap()],
                field: FilterOptionsFieldV2::Keyword,
                query: Some(query.to_owned()),
                limit: Some(100),
            };
            request.validate().unwrap();
            let reference = SharedQueryService::new(&mut storage)
                .filter_options(std::slice::from_ref(&cm), &request)
                .expect("reference Greek keyword options");
            let optimized = prepared
                .filter_options(std::slice::from_ref(&cm), &request)
                .expect("prepared Greek keyword options");
            assert_eq!(
                serde_json::to_value(&optimized).unwrap(),
                serde_json::to_value(&reference).unwrap(),
                "Greek keyword query {query}"
            );
            assert_eq!(
                optimized.options.iter().any(|option| option.value == "α"),
                should_match,
                "SQLite lower() is ASCII-only for FTS vocabulary filtering"
            );

            let mut input = FilterValuesInputV1::for_term(TermId::try_from("92026").unwrap());
            input.campuses = vec![CampusCode::try_from("CM").unwrap()];
            input.text = Some(FilterSearchTextV1::try_from(query).unwrap());
            let filters = NormalizedFilterValuesV1::try_new(input).unwrap();
            let reference = SharedQueryService::new(&mut storage)
                .dynamic_filter_validation(std::slice::from_ref(&cm), &filters)
                .expect("reference Greek exact validation");
            let optimized = prepared
                .dynamic_filter_validation(std::slice::from_ref(&cm), &filters)
                .expect("prepared Greek exact validation");
            assert_eq!(
                serde_json::to_value(&optimized).unwrap(),
                serde_json::to_value(&reference).unwrap(),
                "Greek exact query {query}"
            );
            assert_eq!(optimized.invalid_values.is_empty(), should_match);
        }

        for (field, query) in [
            (FilterOptionsFieldV2::Keyword, Some("comp".to_owned())),
            (FilterOptionsFieldV2::CourseNumberBand, None),
            (FilterOptionsFieldV2::CourseLevel, None),
            (FilterOptionsFieldV2::Instructor, Some("smi".to_owned())),
            (FilterOptionsFieldV2::MeetingLocation, None),
            (FilterOptionsFieldV2::ExamCode, None),
        ] {
            let mut request = FilterOptionsRequestV2 {
                contract_version: QUERY_CONTRACT_VERSION,
                term: TermId::try_from("92026").unwrap(),
                campuses: vec![CampusCode::try_from("NB").unwrap()],
                field,
                query,
                limit: Some(100),
            };
            request.validate().unwrap();
            let reference = SharedQueryService::new(&mut storage)
                .filter_options(std::slice::from_ref(&nb), &request)
                .expect("reference options");
            let optimized = prepared
                .filter_options(std::slice::from_ref(&nb), &request)
                .expect("prepared options");
            assert_eq!(
                serde_json::to_value(optimized).unwrap(),
                serde_json::to_value(reference).unwrap(),
                "field {field:?}"
            );
        }

        let mut input = FilterValuesInputV1::for_term(TermId::try_from("92026").unwrap());
        input.campuses = vec![CampusCode::try_from("NB").unwrap()];
        input.text = Some(FilterSearchTextV1::try_from("computer").unwrap());
        input.instructors = vec![FilterTokenV1::try_from("Pat Smith").unwrap()];
        let filters = NormalizedFilterValuesV1::try_new(input).unwrap();
        let reference_validation = SharedQueryService::new(&mut storage)
            .dynamic_filter_validation(std::slice::from_ref(&nb), &filters)
            .expect("reference validation");
        let prepared_validation = prepared
            .dynamic_filter_validation(std::slice::from_ref(&nb), &filters)
            .expect("prepared validation");
        assert_eq!(
            serde_json::to_value(prepared_validation).unwrap(),
            serde_json::to_value(reference_validation).unwrap()
        );

        let request = course_request("Computer Science", 1, 20);
        let open = snapshot
            .open_evidence(std::slice::from_ref(&nb), now)
            .expect("bound Open evidence");

        let mut assert_tri_state_parity =
            |input: FilterValuesInputV1, label: &str, expected_total: u64| {
                let request = CourseQueryRequestV1 {
                    filters: FilterRequestV1::new(
                        NormalizedFilterValuesV1::try_new(input).expect("tri-state filters"),
                    ),
                    page: PageRequestV1::default(),
                    sort: CourseSortV1::default(),
                };
                let reference = SharedQueryService::new(&mut storage)
                    .course_search(std::slice::from_ref(&nb), &request, now, &open)
                    .expect("reference tri-state search");
                let optimized = prepared
                    .course_search(std::slice::from_ref(&nb), &request, now)
                    .expect("prepared tri-state search");
                assert_eq!(
                    serde_json::to_value(&optimized).unwrap(),
                    serde_json::to_value(&reference).unwrap(),
                    "tri-state parity: {label}"
                );
                assert_eq!(optimized.page.total, expected_total, "{label}");
                optimized
            };

        for (field, strict_total, inclusive_total) in [
            (FilterFieldId::CoursePrerequisite, 3, 4),
            (FilterFieldId::SectionModality, 3, 4),
            (FilterFieldId::SectionSynchronicity, 3, 4),
        ] {
            for (include, expected_total) in [(false, strict_total), (true, inclusive_total)] {
                let mut input = FilterValuesInputV1::for_term(TermId::try_from("92026").unwrap());
                input.campuses = vec![CampusCode::try_from("NB").unwrap()];
                match field {
                    FilterFieldId::CoursePrerequisite => {
                        input.prerequisite = PrerequisiteFilterV1::Has;
                        input.include_incomplete.prerequisite = include;
                    }
                    FilterFieldId::SectionModality => {
                        input.modalities = vec![UserModalityV3::Online];
                        input.include_incomplete.modality = include;
                    }
                    FilterFieldId::SectionSynchronicity => {
                        input.synchronicities = vec![UserSynchronicityV3::Sync];
                        input.include_incomplete.synchronicity = include;
                    }
                    _ => unreachable!(),
                }
                let response = assert_tri_state_parity(
                    input,
                    &format!("{field:?}/include={include}"),
                    expected_total,
                );
                if include {
                    let uncertain = response
                        .items
                        .iter()
                        .find(|item| item.group.key.course_string().as_str() == "01:198:212")
                        .expect("the field-specific switch admits the uncertain Course");
                    assert_eq!(uncertain.explanation.outcome(), MatchOutcome::Uncertain);
                    assert!(uncertain.variants.iter().flat_map(|variant| &variant.sections).any(
                        |section| section.explanation.outcome() == MatchOutcome::Uncertain
                    ));
                }
            }
        }

        let request_for_all_axes = |prerequisite: bool, modality: bool, synchronicity: bool| {
            let mut input = FilterValuesInputV1::for_term(TermId::try_from("92026").unwrap());
            input.campuses = vec![CampusCode::try_from("NB").unwrap()];
            input.prerequisite = PrerequisiteFilterV1::Has;
            input.modalities = vec![UserModalityV3::Online];
            input.synchronicities = vec![UserSynchronicityV3::Sync];
            input.include_incomplete.prerequisite = prerequisite;
            input.include_incomplete.modality = modality;
            input.include_incomplete.synchronicity = synchronicity;
            input
        };
        for flags in [
            (false, false, false),
            (true, false, false),
            (true, true, false),
        ] {
            assert_tri_state_parity(
                request_for_all_axes(flags.0, flags.1, flags.2),
                &format!("all axes/flags={flags:?}"),
                1,
            );
        }
        let inclusive = assert_tri_state_parity(
            request_for_all_axes(true, true, true),
            "all axes/all incomplete enabled",
            2,
        );
        assert!(inclusive.items.iter().all(|item| {
            !matches!(
                item.group.key.course_string().as_str(),
                "01:198:213" | "01:198:214" | "01:198:215"
            )
        }));
        let uncertain = inclusive
            .items
            .iter()
            .find(|item| item.group.key.course_string().as_str() == "01:198:212")
            .expect("all three switches admit the multi-field uncertain Course");
        assert_eq!(uncertain.explanation.outcome(), MatchOutcome::Uncertain);
        for field in [
            FilterFieldId::CoursePrerequisite,
            FilterFieldId::SectionModality,
            FilterFieldId::SectionSynchronicity,
        ] {
            let has_reason = uncertain.variants.iter().any(|variant| {
                variant
                    .filter_matches
                    .iter()
                    .chain(
                        variant
                            .sections
                            .iter()
                            .flat_map(|section| section.filter_matches.iter()),
                    )
                    .any(|entry| {
                        entry.field_id == field
                            && entry.explanation.outcome() == MatchOutcome::Uncertain
                            && !entry.explanation.reasons().is_empty()
                    })
            });
            assert!(has_reason, "materialized uncertainty reason for {field:?}");
        }

        let reference_search = SharedQueryService::new(&mut storage)
            .course_search(std::slice::from_ref(&nb), &request, now, &open)
            .expect("reference search");
        let prepared_search = prepared
            .course_search(std::slice::from_ref(&nb), &request, now)
            .expect("prepared search");
        assert_eq!(
            serde_json::to_value(&prepared_search).unwrap(),
            serde_json::to_value(&reference_search).unwrap()
        );

        let course_key = reference_search.items[0].group.key.clone();
        let course_detail_request = CourseDetailRequestV1::new(course_key);
        let reference_detail = SharedQueryService::new(&mut storage)
            .course_detail(
                std::slice::from_ref(&nb),
                &course_detail_request,
                now,
                &open,
            )
            .expect("reference detail");
        let prepared_detail = prepared
            .course_detail(std::slice::from_ref(&nb), &course_detail_request, now)
            .expect("prepared detail");
        assert_eq!(
            serde_json::to_value(prepared_detail).unwrap(),
            serde_json::to_value(reference_detail).unwrap()
        );

        // Feed the optimized surface a deliberately reversed two-target list.
        // It must use the same canonical target vector as the reference path
        // for Catalog, Open, Search, Section and Detail composition.
        let reversed_targets = vec![nb.clone(), cm.clone()];
        let canonical_targets = vec![cm.clone(), nb.clone()];
        let open = snapshot
            .open_evidence(&canonical_targets, now)
            .expect("canonical multi-target Open evidence");
        let course_request = course_request("Computer Science", 1, 20);
        let reference_courses = SharedQueryService::new(&mut storage)
            .course_search(&canonical_targets, &course_request, now, &open)
            .expect("reference multi-target Course search");
        let prepared_courses = prepared
            .course_search(&reversed_targets, &course_request, now)
            .expect("prepared reversed multi-target Course search");
        assert_eq!(
            serde_json::to_value(&prepared_courses).unwrap(),
            serde_json::to_value(&reference_courses).unwrap()
        );

        let section_request = section_request("Computer Science");
        let reference_sections = SharedQueryService::new(&mut storage)
            .section_search(&canonical_targets, &section_request, now, &open)
            .expect("reference multi-target Section search");
        let prepared_sections = prepared
            .section_search(&reversed_targets, &section_request, now)
            .expect("prepared reversed multi-target Section search");
        assert_eq!(
            serde_json::to_value(&prepared_sections).unwrap(),
            serde_json::to_value(&reference_sections).unwrap()
        );

        let cm_course = reference_courses
            .items
            .iter()
            .find(|course| course.group.key.target() == cm)
            .expect("CM Course result")
            .group
            .key
            .clone();
        let course_detail_request = CourseDetailRequestV1::new(cm_course);
        let reference_course_detail = SharedQueryService::new(&mut storage)
            .course_detail(&canonical_targets, &course_detail_request, now, &open)
            .expect("reference CM Course detail");
        let prepared_course_detail = prepared
            .course_detail(&reversed_targets, &course_detail_request, now)
            .expect("prepared CM Course detail");
        assert_eq!(
            serde_json::to_value(prepared_course_detail).unwrap(),
            serde_json::to_value(reference_course_detail).unwrap()
        );

        let cm_section = SectionKey::try_new("92026", "CM", "10001").unwrap();
        let section_detail_request = SectionDetailRequestV1::new(cm_section);
        let reference_section_detail = SharedQueryService::new(&mut storage)
            .section_detail(&canonical_targets, &section_detail_request, now, &open)
            .expect("reference CM Section detail");
        let prepared_section_detail = prepared
            .section_detail(&reversed_targets, &section_detail_request, now)
            .expect("prepared CM Section detail");
        assert_eq!(
            serde_json::to_value(prepared_section_detail).unwrap(),
            serde_json::to_value(reference_section_detail).unwrap()
        );
    }

    #[test]
    fn online_by_arrangement_sections_match_async_without_the_incomplete_switch() {
        // P1: the real Rutgers online shape (sectionCourseType O, generic mode
        // 90, "hours by arrangement", no day/time) must be a confident ASYNC
        // match; before derivation v2 it was UNSPECIFIED and the ONLINE+ASYNC
        // filter returned nothing unless includeIncomplete.synchronicity was set.
        let (mut storage, nb, _) = fixture_storage();
        let mut by_arrangement = raw_course(
            "NB",
            "01:198:216",
            "198",
            "216",
            "Computer Science online by arrangement",
            "10006",
        );
        by_arrangement["sections"][0]["sectionCourseType"] = json!("O");
        by_arrangement["sections"][0]["meetingTimes"] = json!([{
            "meetingModeCode": "90",
            "meetingDay": "",
            "startTimeMilitary": "",
            "endTimeMilitary": "",
            "baClassHours": "B",
            "campusLocation": "O"
        }]);
        let mut scheduled = raw_course(
            "NB",
            "01:198:217",
            "198",
            "217",
            "Computer Science online scheduled",
            "10007",
        );
        scheduled["sections"][0]["sectionCourseType"] = json!("O");
        scheduled["sections"][0]["meetingTimes"] = json!([{
            "meetingModeCode": "90",
            "meetingDay": "M",
            "startTimeMilitary": "0900",
            "endTimeMilitary": "1020",
            "campusLocation": "O"
        }]);
        let uncertain = raw_course(
            "NB",
            "01:198:218",
            "198",
            "218",
            "Computer Science uncertain",
            "10008",
        );
        publish(
            &mut storage,
            &nb,
            vec![by_arrangement, scheduled, uncertain],
            5,
        );

        let search = |storage: &mut OperationalStorage,
                      synchronicity: UserSynchronicityV3,
                      include_incomplete: bool| {
            let mut input = FilterValuesInputV1::for_term(TermId::try_from("92026").unwrap());
            input.campuses = vec![CampusCode::try_from("NB").unwrap()];
            input.modalities = vec![UserModalityV3::Online];
            input.synchronicities = vec![synchronicity];
            input.include_incomplete.synchronicity = include_incomplete;
            let request = CourseQueryRequestV1 {
                filters: FilterRequestV1::new(
                    NormalizedFilterValuesV1::try_new(input).expect("filters"),
                ),
                page: PageRequestV1::default(),
                sort: CourseSortV1::default(),
            };
            let response = SharedQueryService::new(storage)
                .course_search(
                    std::slice::from_ref(&nb),
                    &request,
                    OffsetDateTime::UNIX_EPOCH,
                    &[],
                )
                .expect("course search");
            response
                .items
                .iter()
                .map(|item| item.group.key.course_string().as_str().to_owned())
                .collect::<Vec<_>>()
        };

        assert_eq!(
            search(&mut storage, UserSynchronicityV3::Async, false),
            vec!["01:198:216".to_owned()],
            "ONLINE + ASYNC matches the by-arrangement section without includeIncomplete"
        );
        assert_eq!(
            search(&mut storage, UserSynchronicityV3::Sync, false),
            vec!["01:198:217".to_owned()],
            "the same section is excluded from ONLINE + SYNC"
        );
        assert!(search(&mut storage, UserSynchronicityV3::Mixed, false).is_empty());
        assert_eq!(
            search(&mut storage, UserSynchronicityV3::Async, true),
            vec!["01:198:216".to_owned()],
            "the incomplete switch adds nothing: every online section is now confident"
        );
    }
}
