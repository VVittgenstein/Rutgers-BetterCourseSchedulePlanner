use std::collections::{BTreeMap, BTreeSet};

use bcsp_catalog::{ProjectionError, to_normalized_catalog_v1};
use bcsp_contracts::{
    CatalogFieldKnowledge, CatalogFieldPresence, CourseDetailRequestV1, CourseDetailResponseV1,
    CourseQueryRequestV1, CourseQueryResponseV1, FilterOptionTargetVersionV2, FilterOptionV2,
    FilterOptionsFieldV2, FilterOptionsRequestV2, FilterOptionsResponseV2, FilterTokenV1,
    NormalizedCatalogV1, NormalizedFilterValuesV1, QUERY_CONTRACT_VERSION, SectionDetailRequestV1,
    SectionDetailResponseV1, SectionQueryRequestV1, SectionQueryResponseV1, TermCampusKey,
};
use bcsp_operational_storage::{CourseTextSearchTokens, OperationalStorage, StorageError};
use bcsp_query::{
    OpenEvidence, QueryEngine, QueryError, TextHit, TextHitPlan, TextHitPlanError,
    TextTargetVersion,
};
use thiserror::Error;
use time::OffsetDateTime;

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
    InvalidFilterOption {
        field: FilterOptionsFieldV2,
        value: String,
    },
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
        QueryEngine::try_new(&context.catalogs, now, open.iter().cloned())
            .and_then(|engine| engine.course_search(request, context.text_hits.as_ref()))
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
        let requested_limit = request.effective_limit();
        let fetch_limit = requested_limit.saturating_add(1);
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
            field => collect_catalog_options(&catalogs, field, request.query.as_deref()),
        }
        .into_values()
        .collect::<Vec<_>>();
        options.sort_by(|left, right| {
            left.label
                .to_lowercase()
                .cmp(&right.label.to_lowercase())
                .then_with(|| left.value.cmp(&right.value))
        });
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
        let targets = validate_search_targets(targets, filters)?;
        let catalogs = load_catalogs(self.storage, &targets)?;
        validate_dynamic_filter_values(self.storage, &catalogs, filters)?;
        verify_current_versions(self.storage, &catalogs)
    }
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

    validate_dynamic_filter_values(storage, &catalogs, filters)?;

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
                let result = storage
                    .search_course_variants(&catalog.target, catalog.content_version.get(), &tokens)
                    .map_err(|source| {
                        map_storage_error(
                            catalog.target.clone(),
                            Some(catalog.content_version.get()),
                            source,
                        )
                    })?;
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
    targets
        .iter()
        .map(|target| {
            let published = storage
                .published_catalog_snapshot(target)
                .map_err(|source| map_storage_error(target.clone(), None, source))?
                .ok_or_else(|| SharedQueryError::TargetNotPublished {
                    target: target.clone(),
                })?;
            to_normalized_catalog_v1(&published).map_err(|source| SharedQueryError::Projection {
                target: target.clone(),
                source,
            })
        })
        .collect()
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

fn known_value<T>(knowledge: &CatalogFieldKnowledge<T>) -> Option<&T> {
    match knowledge {
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Present { value },
        } => Some(value),
        CatalogFieldKnowledge::Known { .. } | CatalogFieldKnowledge::Unknown { .. } => None,
    }
}

fn insert_option(options: &mut BTreeMap<String, FilterOptionV2>, value: String, label: String) {
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

fn collect_catalog_options(
    catalogs: &[NormalizedCatalogV1],
    field: FilterOptionsFieldV2,
    query: Option<&str>,
) -> BTreeMap<String, FilterOptionV2> {
    let mut options = BTreeMap::new();
    match field {
        FilterOptionsFieldV2::Keyword => {}
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

fn validate_tokens(
    options: &BTreeMap<String, FilterOptionV2>,
    values: &[FilterTokenV1],
    field: FilterOptionsFieldV2,
) -> Result<(), SharedQueryError> {
    for value in values {
        if !contains_selected_option(options, value.as_str()) {
            return Err(SharedQueryError::InvalidFilterOption {
                field,
                value: value.as_str().to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_dynamic_filter_values(
    storage: &mut OperationalStorage,
    catalogs: &[NormalizedCatalogV1],
    filters: &NormalizedFilterValuesV1,
) -> Result<(), SharedQueryError> {
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
                let terms = storage
                    .published_course_fts_terms(
                        &catalog.target,
                        catalog.content_version.get(),
                        Some(keyword),
                        100,
                    )
                    .map_err(|source| {
                        map_storage_error(
                            catalog.target.clone(),
                            Some(catalog.content_version.get()),
                            source,
                        )
                    })?;
                found |= terms.iter().any(|term| term.eq_ignore_ascii_case(keyword));
                if found {
                    break;
                }
            }
            if !found {
                return Err(SharedQueryError::InvalidFilterOption {
                    field: FilterOptionsFieldV2::Keyword,
                    value: keyword.clone(),
                });
            }
        }
    }

    validate_tokens(
        &collect_catalog_options(catalogs, FilterOptionsFieldV2::CourseLevel, None),
        filters.levels(),
        FilterOptionsFieldV2::CourseLevel,
    )?;
    validate_tokens(
        &collect_catalog_options(catalogs, FilterOptionsFieldV2::Instructor, None),
        filters.instructors(),
        FilterOptionsFieldV2::Instructor,
    )?;
    validate_tokens(
        &collect_catalog_options(catalogs, FilterOptionsFieldV2::MeetingLocation, None),
        &filters.meeting_locations().locations,
        FilterOptionsFieldV2::MeetingLocation,
    )?;
    validate_tokens(
        &collect_catalog_options(catalogs, FilterOptionsFieldV2::ExamCode, None),
        filters.exam_codes(),
        FilterOptionsFieldV2::ExamCode,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use bcsp_catalog::{normalize_target, to_catalog_refresh_command};
    use bcsp_contracts::{
        CampusCode, CourseDetailRequestV1, CourseQueryRequestV1, CourseSortV1,
        FilterOptionsFieldV2, FilterOptionsRequestV2, FilterRequestV1, FilterSearchTextV1,
        FilterTokenV1, FilterValuesInputV1, NormalizedFilterValuesV1, PageRequestV1,
        QUERY_CONTRACT_VERSION, SectionDetailRequestV1, SectionKey, SectionQueryRequestV1,
        SectionSortV1, TermId, TraceId,
    };
    use bcsp_operational_storage::{EmptySnapshotDecision, OperationalStorage, PublishOutcome};
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
                field: FilterOptionsFieldV2::Instructor,
                ref value,
            } if value == "Removed Instructor"
        ));
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
}
