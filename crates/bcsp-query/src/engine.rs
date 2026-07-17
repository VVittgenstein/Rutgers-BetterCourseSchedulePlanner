use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::time::Instant;

use bcsp_contracts::{
    CatalogContentVersion, CatalogFieldKnowledge, CatalogFieldPresence, CourseDetailRequestV1,
    CourseDetailResponseV1, CourseGroupKey, CourseQueryItemV1, CourseQueryRequestV1,
    CourseQueryResponseV1, CourseSortFieldV1, CourseVariantQueryItemV1, FilterMatchV1,
    LiveOpenEvidenceV1, LiveOpenStateV1, MatchOutcome, MatchReasonCode, NormalizedCatalogV1,
    NormalizedFilterValuesV1, PageInfoV1, PageRequestV1, QUERY_CONTRACT_VERSION,
    SectionDetailRequestV1, SectionDetailResponseV1, SectionKey, SectionQueryItemV1,
    SectionQueryRequestV1, SectionQueryResponseV1, SectionSearchItemV1, SectionSortFieldV1,
    SortDirectionV1, TermCampusKey, TextMatchEvidenceV1,
};
use thiserror::Error;
use time::OffsetDateTime;

use crate::predicates::{
    EvaluatedFilters, evaluate_course_filter_matches, evaluate_section_filter_matches,
    matched_explanation, section_filters_active,
};
use crate::{
    CatalogCorpus, CorpusError, PredicateEvaluation, PreparedCatalogCorpus, TextHitPlan, and_all,
    or_active,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpenEvidence {
    pub section_key: SectionKey,
    pub evidence: LiveOpenEvidenceV1,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum QueryError {
    #[error(transparent)]
    InvalidCorpus(#[from] CorpusError),
    #[error("live Open evidence contains the same section more than once")]
    DuplicateOpenEvidence,
    #[error("live Open evidence references a section outside the catalog corpus")]
    ForeignOpenEvidence,
    #[error("filter term differs from the catalog corpus term")]
    FilterTermMismatch,
    #[error("Core code {code} is not present in the selected Catalog targets")]
    UnknownCoreCode { code: String },
    #[error("an active text filter requires a bound text-hit plan")]
    TextEvidenceUnavailable,
    #[error("a text-hit plan was supplied for an inactive text filter")]
    UnexpectedTextEvidence,
    #[error("text-hit plan query differs from the filter query")]
    TextQueryMismatch,
    #[error("text-hit plan targets differ from the participating catalog targets")]
    TextTargetSetMismatch,
    #[error("text-hit plan content version differs from the catalog snapshot")]
    TextContentVersionMismatch,
    #[error("text-hit plan references a variant outside the catalog corpus")]
    ForeignTextHit,
    #[error("prepared request targets do not match the bound prepared generation")]
    PreparedTargetSetMismatch,
    #[error("course was not found")]
    CourseNotFound,
    #[error("section was not found")]
    SectionNotFound,
}

/// Build-once Open evidence index paired with one prepared Catalog corpus.
/// Raw timestamps are retained so request-time freshness remains a function of
/// the caller's exact `now` value.
#[derive(Debug)]
pub struct PreparedOpenOverlay {
    target: TermCampusKey,
    content_version: CatalogContentVersion,
    evidence: Vec<OpenEvidence>,
    index: BTreeMap<u64, OpenIndexBucket>,
}

#[derive(Debug)]
enum OpenIndexBucket {
    One(usize),
    Many(Vec<usize>),
}

impl PreparedOpenOverlay {
    pub fn try_new(
        corpus: &PreparedCatalogCorpus,
        target: &TermCampusKey,
        open: impl IntoIterator<Item = OpenEvidence>,
    ) -> Result<Self, QueryError> {
        let content_version = corpus
            .content_version(target)
            .ok_or(QueryError::PreparedTargetSetMismatch)?;
        let mut evidence = Vec::<OpenEvidence>::new();
        for item in open {
            if item.section_key.term() != target.term()
                || item.section_key.campus() != target.campus()
                || corpus.section(&item.section_key).is_none()
            {
                return Err(QueryError::ForeignOpenEvidence);
            }
            evidence.push(item);
        }
        let index = build_open_index_with_hasher(&evidence, open_key_hash)?;
        Ok(Self {
            target: target.clone(),
            content_version,
            evidence,
            index,
        })
    }

    fn is_bound_to(&self, corpus: &PreparedCatalogCorpus) -> bool {
        corpus.content_version(&self.target) == Some(self.content_version)
    }

    fn get(&self, key: &SectionKey) -> Option<&LiveOpenEvidenceV1> {
        lookup_open_evidence_with_hasher(&self.index, &self.evidence, key, open_key_hash)
    }

    pub fn evidence(&self) -> impl ExactSizeIterator<Item = &OpenEvidence> {
        self.evidence.iter()
    }

    pub fn index_estimated_bytes(&self) -> u64 {
        let mut bytes = usize_to_u64(std::mem::size_of::<Self>())
            .saturating_add(usize_to_u64(self.target.term().as_str().len()))
            .saturating_add(usize_to_u64(self.target.campus().as_str().len()))
            .saturating_add(
                usize_to_u64(self.evidence.capacity())
                    .saturating_mul(usize_to_u64(std::mem::size_of::<OpenEvidence>())),
            )
            .saturating_add(usize_to_u64(self.index.len()).saturating_mul(usize_to_u64(
                std::mem::size_of::<u64>()
                    + std::mem::size_of::<OpenIndexBucket>()
                    + 3 * std::mem::size_of::<usize>(),
            )));
        for item in &self.evidence {
            bytes = bytes
                .saturating_add(usize_to_u64(item.section_key.term().as_str().len()))
                .saturating_add(usize_to_u64(item.section_key.campus().as_str().len()))
                .saturating_add(usize_to_u64(item.section_key.index().as_str().len()));
        }
        for bucket in self.index.values() {
            if let OpenIndexBucket::Many(entities) = bucket {
                bytes = bytes.saturating_add(
                    usize_to_u64(entities.capacity())
                        .saturating_mul(usize_to_u64(std::mem::size_of::<usize>())),
                );
            }
        }
        bytes
    }
}

fn build_open_index_with_hasher(
    evidence: &[OpenEvidence],
    hash: impl Fn(&SectionKey) -> u64,
) -> Result<BTreeMap<u64, OpenIndexBucket>, QueryError> {
    let mut index = BTreeMap::<u64, OpenIndexBucket>::new();
    for (entity, item) in evidence.iter().enumerate() {
        match index.entry(hash(&item.section_key)) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(OpenIndexBucket::One(entity));
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let duplicate = match entry.get() {
                    OpenIndexBucket::One(existing) => {
                        evidence[*existing].section_key == item.section_key
                    }
                    OpenIndexBucket::Many(existing) => existing
                        .iter()
                        .any(|existing| evidence[*existing].section_key == item.section_key),
                };
                if duplicate {
                    return Err(QueryError::DuplicateOpenEvidence);
                }
                match entry.get_mut() {
                    OpenIndexBucket::One(first) => {
                        *entry.get_mut() = OpenIndexBucket::Many(vec![*first, entity]);
                    }
                    OpenIndexBucket::Many(existing) => existing.push(entity),
                }
            }
        }
    }
    Ok(index)
}

fn lookup_open_evidence_with_hasher<'evidence>(
    index: &BTreeMap<u64, OpenIndexBucket>,
    evidence: &'evidence [OpenEvidence],
    key: &SectionKey,
    hash: impl Fn(&SectionKey) -> u64,
) -> Option<&'evidence LiveOpenEvidenceV1> {
    let bucket = index.get(&hash(key))?;
    let entity = match bucket {
        OpenIndexBucket::One(entity) => (evidence[*entity].section_key == *key).then_some(*entity),
        OpenIndexBucket::Many(entities) => entities
            .iter()
            .copied()
            .find(|entity| evidence[*entity].section_key == *key),
    }?;
    Some(&evidence[entity].evidence)
}

enum EngineCorpus<'a> {
    Built(CatalogCorpus<'a>),
    Prepared(&'a PreparedCatalogCorpus),
}

impl EngineCorpus<'_> {
    fn term(&self) -> &str {
        match self {
            Self::Built(corpus) => corpus.term(),
            Self::Prepared(corpus) => corpus.term(),
        }
    }

    fn for_each_group<'corpus>(
        &'corpus self,
        active_targets: &[TermCampusKey],
        mut visitor: impl FnMut(&'corpus bcsp_contracts::NormalizedCourseGroupV1),
    ) {
        match self {
            Self::Built(corpus) => corpus.groups().for_each(&mut visitor),
            Self::Prepared(corpus) => {
                corpus.for_each_group_in_targets(active_targets, visitor);
            }
        }
    }

    fn group(&self, key: &CourseGroupKey) -> Option<&bcsp_contracts::NormalizedCourseGroupV1> {
        match self {
            Self::Built(corpus) => corpus.group(key),
            Self::Prepared(corpus) => corpus.group(key),
        }
    }

    fn variants_for(
        &self,
        group: &bcsp_contracts::NormalizedCourseGroupV1,
    ) -> Vec<&bcsp_contracts::NormalizedCourseVariantV1> {
        match self {
            Self::Built(corpus) => corpus.variants_for(group),
            Self::Prepared(corpus) => corpus.variants_for(group),
        }
    }

    fn variant(
        &self,
        key: &bcsp_contracts::CourseVariantKey,
    ) -> Option<&bcsp_contracts::NormalizedCourseVariantV1> {
        match self {
            Self::Built(corpus) => corpus.variant(key),
            Self::Prepared(corpus) => corpus.variant(key),
        }
    }

    fn for_each_section<'corpus>(
        &'corpus self,
        active_targets: &[TermCampusKey],
        mut visitor: impl FnMut(&'corpus bcsp_contracts::NormalizedSectionV1),
    ) {
        match self {
            Self::Built(corpus) => corpus.sections().for_each(&mut visitor),
            Self::Prepared(corpus) => {
                corpus.for_each_section_in_targets(active_targets, visitor);
            }
        }
    }

    fn section(&self, key: &SectionKey) -> Option<&bcsp_contracts::NormalizedSectionV1> {
        match self {
            Self::Built(corpus) => corpus.section(key),
            Self::Prepared(corpus) => corpus.section(key),
        }
    }

    fn sections_for(
        &self,
        variant: &bcsp_contracts::NormalizedCourseVariantV1,
    ) -> Vec<&bcsp_contracts::NormalizedSectionV1> {
        match self {
            Self::Built(corpus) => corpus.sections_for(variant),
            Self::Prepared(corpus) => corpus.sections_for(variant),
        }
    }

    fn known_occurrences_for(
        &self,
        section: &bcsp_contracts::NormalizedSectionV1,
    ) -> Option<Vec<&bcsp_contracts::NormalizedOccurrenceV1>> {
        match self {
            Self::Built(corpus) => corpus.known_occurrences_for(section),
            Self::Prepared(corpus) => corpus.known_occurrences_for(section),
        }
    }

    fn target_versions(&self) -> Vec<(&TermCampusKey, CatalogContentVersion)> {
        match self {
            Self::Built(corpus) => corpus.target_versions().collect(),
            Self::Prepared(corpus) => corpus.target_versions().collect(),
        }
    }
}

enum EngineOpen<'a> {
    Built(BTreeMap<SectionKey, LiveOpenEvidenceV1>),
    Prepared(Vec<&'a PreparedOpenOverlay>),
}

pub struct QueryEngine<'a> {
    corpus: EngineCorpus<'a>,
    active_targets: Vec<TermCampusKey>,
    forced_open_unavailable: Vec<TermCampusKey>,
    now: OffsetDateTime,
    open: EngineOpen<'a>,
}

impl<'a> QueryEngine<'a> {
    pub fn try_new(
        catalogs: &'a [NormalizedCatalogV1],
        now: OffsetDateTime,
        open: impl IntoIterator<Item = OpenEvidence>,
    ) -> Result<Self, QueryError> {
        let catalogs = catalogs.iter().collect::<Vec<_>>();
        Self::try_new_from_refs(&catalogs, now, open)
    }

    pub fn try_new_from_refs(
        catalogs: &[&'a NormalizedCatalogV1],
        now: OffsetDateTime,
        open: impl IntoIterator<Item = OpenEvidence>,
    ) -> Result<Self, QueryError> {
        let corpus_started = Instant::now();
        let corpus = CatalogCorpus::try_new_from_refs(catalogs)?;
        tracing::debug!(
            target: "bcsp_performance",
            phase = "corpus_build",
            elapsed_us = elapsed_micros(corpus_started),
            catalogs = catalogs.len(),
        );
        let overlay_started = Instant::now();
        let mut by_section = BTreeMap::new();
        for item in open {
            if corpus.section(&item.section_key).is_none() {
                return Err(QueryError::ForeignOpenEvidence);
            }
            if by_section.insert(item.section_key, item.evidence).is_some() {
                return Err(QueryError::DuplicateOpenEvidence);
            }
        }
        tracing::debug!(
            target: "bcsp_performance",
            phase = "open_overlay_build",
            elapsed_us = elapsed_micros(overlay_started),
            sections = by_section.len(),
        );
        let active_targets = corpus
            .target_versions()
            .map(|(target, _)| target.clone())
            .collect();
        Ok(Self {
            corpus: EngineCorpus::Built(corpus),
            active_targets,
            forced_open_unavailable: Vec::new(),
            now,
            open: EngineOpen::Built(by_section),
        })
    }

    /// Binds one request to a build-once prepared term corpus and Open overlay.
    /// Only the explicit target vector participates in Search, Detail and text
    /// evidence validation even though the reusable corpus may contain other
    /// ready campuses for the same term.
    pub fn try_new_prepared(
        corpus: &'a PreparedCatalogCorpus,
        active_targets: &[TermCampusKey],
        forced_open_unavailable: &[TermCampusKey],
        now: OffsetDateTime,
        open: &[&'a PreparedOpenOverlay],
    ) -> Result<Self, QueryError> {
        if active_targets.is_empty() {
            return Err(QueryError::PreparedTargetSetMismatch);
        }
        let mut unique = BTreeSet::new();
        for target in active_targets {
            if target.term().as_str() != corpus.term()
                || !corpus.contains_target(target)
                || !unique.insert(target.clone())
            {
                return Err(QueryError::PreparedTargetSetMismatch);
            }
        }
        let mut open_by_target = BTreeMap::new();
        for overlay in open {
            if !overlay.is_bound_to(corpus)
                || open_by_target
                    .insert(overlay.target.clone(), *overlay)
                    .is_some()
            {
                return Err(QueryError::PreparedTargetSetMismatch);
            }
        }
        if open_by_target.keys().ne(unique.iter()) {
            return Err(QueryError::PreparedTargetSetMismatch);
        }
        if forced_open_unavailable
            .iter()
            .any(|target| !unique.contains(target))
        {
            return Err(QueryError::PreparedTargetSetMismatch);
        }
        Ok(Self {
            corpus: EngineCorpus::Prepared(corpus),
            active_targets: unique.into_iter().collect(),
            forced_open_unavailable: forced_open_unavailable.to_vec(),
            now,
            open: EngineOpen::Prepared(open_by_target.into_values().collect()),
        })
    }

    pub fn course_search(
        &self,
        request: &CourseQueryRequestV1,
        text_hits: Option<&TextHitPlan>,
    ) -> Result<CourseQueryResponseV1, QueryError> {
        let filters = request.filters.values();
        self.validate_filters(filters)?;
        self.validate_text_plan(filters, text_hits)?;
        let section_filters_active = section_filters_active(filters);
        let mut candidates = Vec::new();
        let predicate_started = Instant::now();

        self.corpus.for_each_group(&self.active_targets, |group| {
            if !self.group_is_active(&group.key) {
                return;
            }
            let exact_identifier = exact_course_identifier(filters, &group.key);
            let mut variant_evaluations = Vec::new();
            let mut best_rank = u32::MAX;
            let mut title = None::<&str>;

            for variant in self.corpus.variants_for(group) {
                let course =
                    evaluate_course_filter_matches(&group.key, variant, filters, text_hits);
                let (section_witness, section_admitted) = if section_filters_active {
                    let sections = self.corpus.sections_for(variant);
                    let admitted = sections
                        .into_iter()
                        .map(|section| self.section_evaluation(section, filters))
                        .filter(|evaluated| evaluated.admitted)
                        .map(|evaluated| evaluated.evaluation)
                        .collect::<Vec<_>>();
                    if admitted.is_empty() {
                        (PredicateEvaluation::no_match("section.witness"), false)
                    } else {
                        (or_active(admitted), true)
                    }
                } else {
                    (PredicateEvaluation::matched(), true)
                };
                let variant_evaluation = and_all([course.evaluation.clone(), section_witness]);
                if course.admitted && section_admitted {
                    if let Some(rank) = text_hits.and_then(|plan| plan.rank(&variant.key)) {
                        best_rank = best_rank.min(rank);
                    }
                    if let Some(candidate) =
                        known_string(&variant.title).filter(|value| !value.is_empty())
                        && title.is_none_or(|current| candidate < current)
                    {
                        title = Some(candidate);
                    }
                    variant_evaluations.push(variant_evaluation);
                }
            }

            let group_evaluation = or_nonempty(variant_evaluations, "variant.witness");
            if group_evaluation.outcome() != MatchOutcome::NoMatch {
                candidates.push(CourseCandidate {
                    group,
                    evaluation: group_evaluation,
                    exact_identifier,
                    best_rank,
                    title,
                });
            }
        });
        tracing::debug!(
            target: "bcsp_performance",
            phase = "predicate",
            elapsed_us = elapsed_micros(predicate_started),
            candidates = candidates.len(),
            query_kind = "course",
        );

        let sort_started = Instant::now();
        candidates.sort_by(|left, right| compare_course(left, right, request));
        tracing::debug!(
            target: "bcsp_performance",
            phase = "sort",
            elapsed_us = elapsed_micros(sort_started),
            candidates = candidates.len(),
            query_kind = "course",
        );
        let pagination_started = Instant::now();
        let (page, candidates) = paginate(candidates, request.page);
        tracing::debug!(
            target: "bcsp_performance",
            phase = "pagination",
            elapsed_us = elapsed_micros(pagination_started),
            page_items = candidates.len(),
            query_kind = "course",
        );
        let materialization_started = Instant::now();
        let items = candidates
            .into_iter()
            .map(|candidate| {
                self.materialize_course_candidate(
                    candidate,
                    filters,
                    text_hits,
                    section_filters_active,
                )
            })
            .collect();
        tracing::debug!(
            target: "bcsp_performance",
            phase = "materialization",
            elapsed_us = elapsed_micros(materialization_started),
            query_kind = "course",
        );
        Ok(CourseQueryResponseV1 {
            contract_version: QUERY_CONTRACT_VERSION,
            page,
            items,
        })
    }

    pub fn section_search(
        &self,
        request: &SectionQueryRequestV1,
        text_hits: Option<&TextHitPlan>,
    ) -> Result<SectionQueryResponseV1, QueryError> {
        let filters = request.filters.values();
        self.validate_filters(filters)?;
        self.validate_text_plan(filters, text_hits)?;
        let mut candidates = Vec::new();
        let predicate_started = Instant::now();
        self.corpus
            .for_each_section(&self.active_targets, |section| {
                if !self.section_is_active(&section.key) {
                    return;
                }
                let variant = self
                    .corpus
                    .variant(&section.variant_key)
                    .expect("validated corpus contains the section variant");
                let course = evaluate_course_filter_matches(
                    variant.key.group(),
                    variant,
                    filters,
                    text_hits,
                );
                let section_evaluation = self.section_evaluation(section, filters);
                let overall = and_all([
                    course.evaluation.clone(),
                    section_evaluation.evaluation.clone(),
                ]);
                if !course.admitted || !section_evaluation.admitted {
                    return;
                }
                candidates.push(SectionCandidate {
                    section,
                    variant,
                    evaluation: overall,
                    section_number: known_string(&section.section_number).unwrap_or_default(),
                    exact_identifier: exact_course_identifier(filters, variant.key.group()),
                    text_rank: text_hits
                        .and_then(|plan| plan.rank(&variant.key))
                        .unwrap_or(u32::MAX),
                    open_state: self.open_state_for(&section.key),
                });
            });
        tracing::debug!(
            target: "bcsp_performance",
            phase = "predicate",
            elapsed_us = elapsed_micros(predicate_started),
            candidates = candidates.len(),
            query_kind = "section",
        );

        let sort_started = Instant::now();
        candidates.sort_by(|left, right| compare_section(left, right, request));
        tracing::debug!(
            target: "bcsp_performance",
            phase = "sort",
            elapsed_us = elapsed_micros(sort_started),
            candidates = candidates.len(),
            query_kind = "section",
        );
        let pagination_started = Instant::now();
        let (page, candidates) = paginate(candidates, request.page);
        tracing::debug!(
            target: "bcsp_performance",
            phase = "pagination",
            elapsed_us = elapsed_micros(pagination_started),
            page_items = candidates.len(),
            query_kind = "section",
        );
        let materialization_started = Instant::now();
        let items = candidates
            .into_iter()
            .map(|candidate| self.materialize_section_candidate(candidate, filters, text_hits))
            .collect();
        tracing::debug!(
            target: "bcsp_performance",
            phase = "materialization",
            elapsed_us = elapsed_micros(materialization_started),
            query_kind = "section",
        );
        Ok(SectionQueryResponseV1 {
            contract_version: QUERY_CONTRACT_VERSION,
            page,
            items,
        })
    }

    fn materialize_course_candidate(
        &self,
        candidate: CourseCandidate<'a>,
        filters: &NormalizedFilterValuesV1,
        text_hits: Option<&TextHitPlan>,
        section_filters_active: bool,
    ) -> CourseQueryItemV1 {
        let CourseCandidate {
            group,
            evaluation,
            exact_identifier,
            ..
        } = candidate;
        let variants = self
            .corpus
            .variants_for(group)
            .into_iter()
            .filter_map(|variant| {
                let course =
                    evaluate_course_filter_matches(&group.key, variant, filters, text_hits);
                let section_evaluations = self
                    .corpus
                    .sections_for(variant)
                    .into_iter()
                    .map(|section| self.section_item(section, filters))
                    .collect::<Vec<_>>();
                let admitted_section_evaluations = section_evaluations
                    .iter()
                    .filter(|(_, evaluated)| evaluated.admitted)
                    .map(|(_, evaluated)| evaluated.evaluation.clone())
                    .collect::<Vec<_>>();
                let (section_witness, section_admitted) = if section_filters_active {
                    if admitted_section_evaluations.is_empty() {
                        (PredicateEvaluation::no_match("section.witness"), false)
                    } else {
                        (or_active(admitted_section_evaluations), true)
                    }
                } else {
                    (PredicateEvaluation::matched(), true)
                };
                let variant_evaluation = and_all([course.evaluation.clone(), section_witness]);
                if !course.admitted || !section_admitted {
                    return None;
                }
                let sections = section_evaluations
                    .into_iter()
                    .filter_map(|(mut item, section_evaluation)| {
                        if section_filters_active && !section_evaluation.admitted {
                            return None;
                        }
                        let overall =
                            and_all([course.evaluation.clone(), section_evaluation.evaluation]);
                        item.explanation = overall.into_explanation();
                        Some(item)
                    })
                    .collect();
                Some(CourseVariantQueryItemV1 {
                    variant: variant.clone(),
                    explanation: variant_evaluation.into_explanation(),
                    filter_matches: course.filter_matches,
                    text_match: filters.text().and_then(|text| {
                        text_hits.and_then(|plan| plan.rank(&variant.key)).map(|_| {
                            TextMatchEvidenceV1 {
                                exact_course_identifier: exact_identifier,
                                matched_tokens: text.tokens().to_vec(),
                            }
                        })
                    }),
                    sections,
                })
            })
            .collect();
        CourseQueryItemV1 {
            group: group.clone(),
            explanation: evaluation.into_explanation(),
            variants,
        }
    }

    fn materialize_section_candidate(
        &self,
        candidate: SectionCandidate<'a>,
        filters: &NormalizedFilterValuesV1,
        text_hits: Option<&TextHitPlan>,
    ) -> SectionSearchItemV1 {
        let SectionCandidate {
            section,
            variant,
            evaluation,
            exact_identifier,
            ..
        } = candidate;
        let course =
            evaluate_course_filter_matches(variant.key.group(), variant, filters, text_hits);
        let (mut item, section_evaluation) = self.section_item(section, filters);
        debug_assert!(course.admitted && section_evaluation.admitted);
        debug_assert_eq!(
            evaluation,
            and_all([course.evaluation, section_evaluation.evaluation])
        );
        item.explanation = evaluation.into_explanation();
        SectionSearchItemV1 {
            variant: variant.clone(),
            section: item,
            course_filter_matches: course.filter_matches,
            text_match: filters.text().and_then(|text| {
                text_hits
                    .and_then(|plan| plan.rank(&variant.key))
                    .map(|_| TextMatchEvidenceV1 {
                        exact_course_identifier: exact_identifier,
                        matched_tokens: text.tokens().to_vec(),
                    })
            }),
        }
    }

    pub fn course_detail(
        &self,
        request: &CourseDetailRequestV1,
    ) -> Result<CourseDetailResponseV1, QueryError> {
        if !self.group_is_active(&request.key) {
            return Err(QueryError::CourseNotFound);
        }
        let group = self
            .corpus
            .group(&request.key)
            .ok_or(QueryError::CourseNotFound)?;
        let variants = self
            .corpus
            .variants_for(group)
            .into_iter()
            .map(|variant| {
                let sections = self
                    .corpus
                    .sections_for(variant)
                    .into_iter()
                    .map(|section| self.unfiltered_section_item(section))
                    .collect();
                CourseVariantQueryItemV1 {
                    variant: variant.clone(),
                    explanation: matched_explanation(),
                    filter_matches: Vec::new(),
                    text_match: None,
                    sections,
                }
            })
            .collect();
        Ok(CourseDetailResponseV1 {
            contract_version: QUERY_CONTRACT_VERSION,
            course: CourseQueryItemV1 {
                group: group.clone(),
                explanation: matched_explanation(),
                variants,
            },
        })
    }

    pub fn section_detail(
        &self,
        request: &SectionDetailRequestV1,
    ) -> Result<SectionDetailResponseV1, QueryError> {
        if !self.section_is_active(&request.key) {
            return Err(QueryError::SectionNotFound);
        }
        let section = self
            .corpus
            .section(&request.key)
            .ok_or(QueryError::SectionNotFound)?;
        let variant = self
            .corpus
            .variant(&section.variant_key)
            .expect("validated corpus contains the section variant");
        Ok(SectionDetailResponseV1 {
            contract_version: QUERY_CONTRACT_VERSION,
            variant: variant.clone(),
            section: self.unfiltered_section_item(section),
        })
    }

    fn section_item(
        &self,
        section: &bcsp_contracts::NormalizedSectionV1,
        filters: &NormalizedFilterValuesV1,
    ) -> (SectionQueryItemV1, EvaluatedFilters) {
        let occurrences = self.corpus.known_occurrences_for(section);
        let open = self.open_for(&section.key);
        let evaluated = evaluate_section_filter_matches(
            section,
            occurrences.as_deref(),
            &open,
            self.now,
            filters,
        );
        (
            SectionQueryItemV1 {
                section: section.clone(),
                occurrences: occurrences
                    .unwrap_or_default()
                    .into_iter()
                    .cloned()
                    .collect(),
                open,
                explanation: evaluated.evaluation.clone().into_explanation(),
                filter_matches: evaluated.filter_matches.clone(),
            },
            evaluated,
        )
    }

    fn section_evaluation(
        &self,
        section: &bcsp_contracts::NormalizedSectionV1,
        filters: &NormalizedFilterValuesV1,
    ) -> EvaluatedFilters {
        let occurrences = self.corpus.known_occurrences_for(section);
        let open = self.open_for(&section.key);
        evaluate_section_filter_matches(section, occurrences.as_deref(), &open, self.now, filters)
    }

    fn unfiltered_section_item(
        &self,
        section: &bcsp_contracts::NormalizedSectionV1,
    ) -> SectionQueryItemV1 {
        SectionQueryItemV1 {
            section: section.clone(),
            occurrences: self
                .corpus
                .known_occurrences_for(section)
                .unwrap_or_default()
                .into_iter()
                .cloned()
                .collect(),
            open: self.open_for(&section.key),
            explanation: matched_explanation(),
            filter_matches: Vec::<FilterMatchV1>::new(),
        }
    }

    fn open_for(&self, key: &SectionKey) -> LiveOpenEvidenceV1 {
        let (mut evidence, prepared) = match &self.open {
            EngineOpen::Built(open) => (open.get(key).cloned(), false),
            EngineOpen::Prepared(open) => (
                open.iter()
                    .find(|overlay| {
                        overlay.target.term() == key.term()
                            && overlay.target.campus() == key.campus()
                    })
                    .and_then(|overlay| overlay.get(key))
                    .cloned(),
                true,
            ),
        };
        let Some(mut evidence) = evidence.take() else {
            return LiveOpenEvidenceV1 {
                state: LiveOpenStateV1::Unknown,
                observed_at: None,
                fresh_until: None,
                uncertainty: Some(MatchReasonCode::MissingReliableData),
            };
        };
        let forced = self.section_open_is_forced_unavailable(key);
        let stale_without_prior_uncertainty = evidence.uncertainty.is_none()
            && !(evidence.observed_at.is_some()
                && evidence
                    .fresh_until
                    .is_some_and(|fresh_until| self.now <= fresh_until));
        if prepared && (forced || stale_without_prior_uncertainty) {
            evidence.uncertainty = Some(MatchReasonCode::SourceUnavailable);
        }
        evidence
    }

    fn open_state_for(&self, key: &SectionKey) -> LiveOpenStateV1 {
        match &self.open {
            EngineOpen::Built(open) => open.get(key),
            EngineOpen::Prepared(open) => open
                .iter()
                .find(|overlay| {
                    overlay.target.term() == key.term() && overlay.target.campus() == key.campus()
                })
                .and_then(|overlay| overlay.get(key)),
        }
        .map(|evidence| evidence.state)
        .unwrap_or(LiveOpenStateV1::Unknown)
    }

    fn identity_is_active(&self, term: &str, campus: &str) -> bool {
        self.active_targets
            .iter()
            .any(|target| target.term().as_str() == term && target.campus().as_str() == campus)
    }

    fn group_is_active(&self, key: &CourseGroupKey) -> bool {
        self.identity_is_active(key.term().as_str(), key.campus().as_str())
    }

    fn section_is_active(&self, key: &SectionKey) -> bool {
        self.identity_is_active(key.term().as_str(), key.campus().as_str())
    }

    fn section_open_is_forced_unavailable(&self, key: &SectionKey) -> bool {
        self.forced_open_unavailable
            .iter()
            .any(|target| target.term() == key.term() && target.campus() == key.campus())
    }

    fn validate_filters(&self, filters: &NormalizedFilterValuesV1) -> Result<(), QueryError> {
        if filters.term().as_str() != self.corpus.term() {
            return Err(QueryError::FilterTermMismatch);
        }
        if !filters.core().codes.is_empty() {
            let mut authoritative_codes = BTreeSet::new();
            self.corpus.for_each_group(&self.active_targets, |group| {
                if !self.group_is_active(&group.key)
                    || (!filters.campuses().is_empty()
                        && !filters.campuses().contains(group.key.campus()))
                {
                    return;
                }
                for variant in self.corpus.variants_for(group) {
                    if let CatalogFieldKnowledge::Known {
                        presence: CatalogFieldPresence::Present { value },
                    } = &variant.core_codes
                    {
                        authoritative_codes
                            .extend(value.iter().map(|code| code.to_ascii_uppercase()));
                    }
                }
            });
            if let Some(code) = filters
                .core()
                .codes
                .iter()
                .find(|code| !authoritative_codes.contains(code.as_str()))
            {
                return Err(QueryError::UnknownCoreCode {
                    code: code.as_str().to_owned(),
                });
            }
        }
        Ok(())
    }

    fn validate_text_plan(
        &self,
        filters: &NormalizedFilterValuesV1,
        plan: Option<&TextHitPlan>,
    ) -> Result<(), QueryError> {
        let (Some(query), Some(plan)) = (filters.text(), plan) else {
            return match (filters.text(), plan) {
                (Some(_), None) => Err(QueryError::TextEvidenceUnavailable),
                (None, Some(_)) => Err(QueryError::UnexpectedTextEvidence),
                (None, None) => Ok(()),
                (Some(_), Some(_)) => unreachable!(),
            };
        };
        if query != plan.query() {
            return Err(QueryError::TextQueryMismatch);
        }

        let expected = self
            .active_targets
            .iter()
            .map(|target| {
                (
                    target.clone(),
                    self.corpus
                        .target_versions()
                        .into_iter()
                        .find_map(|(candidate, version)| (candidate == target).then_some(version))
                        .expect("active target belongs to the bound corpus"),
                )
            })
            .collect::<BTreeMap<TermCampusKey, CatalogContentVersion>>();
        let actual = plan
            .target_versions()
            .map(|(target, version)| (target.clone(), version))
            .collect::<BTreeMap<_, _>>();
        if expected.keys().ne(actual.keys()) {
            return Err(QueryError::TextTargetSetMismatch);
        }
        if expected != actual {
            return Err(QueryError::TextContentVersionMismatch);
        }
        if plan
            .hit_keys()
            .any(|key| !self.group_is_active(key.group()) || self.corpus.variant(key).is_none())
        {
            return Err(QueryError::ForeignTextHit);
        }
        Ok(())
    }
}

struct CourseCandidate<'a> {
    group: &'a bcsp_contracts::NormalizedCourseGroupV1,
    evaluation: PredicateEvaluation,
    exact_identifier: bool,
    best_rank: u32,
    title: Option<&'a str>,
}

struct SectionCandidate<'a> {
    section: &'a bcsp_contracts::NormalizedSectionV1,
    variant: &'a bcsp_contracts::NormalizedCourseVariantV1,
    evaluation: PredicateEvaluation,
    section_number: &'a str,
    exact_identifier: bool,
    text_rank: u32,
    open_state: LiveOpenStateV1,
}

fn compare_course(
    left: &CourseCandidate<'_>,
    right: &CourseCandidate<'_>,
    request: &CourseQueryRequestV1,
) -> Ordering {
    outcome_order(left.evaluation.outcome())
        .cmp(&outcome_order(right.evaluation.outcome()))
        .then_with(|| right.exact_identifier.cmp(&left.exact_identifier))
        .then_with(|| match request.sort.field {
            CourseSortFieldV1::Relevance => directed(
                relevance(left.best_rank).cmp(&relevance(right.best_rank)),
                request.sort.direction,
            ),
            CourseSortFieldV1::CourseIdentifier => directed(
                compare_course_identifier(&left.group.key, &right.group.key),
                request.sort.direction,
            ),
            CourseSortFieldV1::Title => {
                compare_titles(&left.title, &right.title, request.sort.direction)
            }
        })
        .then_with(|| left.group.key.cmp(&right.group.key))
}

fn compare_section(
    left: &SectionCandidate<'_>,
    right: &SectionCandidate<'_>,
    request: &SectionQueryRequestV1,
) -> Ordering {
    outcome_order(left.evaluation.outcome())
        .cmp(&outcome_order(right.evaluation.outcome()))
        .then_with(|| right.exact_identifier.cmp(&left.exact_identifier))
        .then_with(|| {
            let ordering = match request.sort.field {
                SectionSortFieldV1::SectionIndex => {
                    left.section.key.index().cmp(right.section.key.index())
                }
                SectionSortFieldV1::SectionNumber => left.section_number.cmp(right.section_number),
                SectionSortFieldV1::CourseIdentifier => {
                    compare_course_identifier(left.variant.key.group(), right.variant.key.group())
                }
                SectionSortFieldV1::OpenStatus => {
                    open_order(left.open_state).cmp(&open_order(right.open_state))
                }
            };
            directed(ordering, request.sort.direction)
        })
        .then_with(|| relevance(right.text_rank).cmp(&relevance(left.text_rank)))
        .then_with(|| left.section.key.cmp(&right.section.key))
}

fn compare_titles(
    left: &Option<&str>,
    right: &Option<&str>,
    direction: SortDirectionV1,
) -> Ordering {
    match (left, right) {
        (Some(left), Some(right)) => directed(left.cmp(right), direction),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn compare_course_identifier(left: &CourseGroupKey, right: &CourseGroupKey) -> Ordering {
    left.course_string()
        .cmp(right.course_string())
        .then_with(|| left.campus().cmp(right.campus()))
        .then_with(|| left.term().cmp(right.term()))
}

const fn outcome_order(value: MatchOutcome) -> u8 {
    match value {
        MatchOutcome::Match => 0,
        MatchOutcome::Uncertain => 1,
        MatchOutcome::NoMatch => 2,
    }
}

const fn open_order(value: LiveOpenStateV1) -> u8 {
    match value {
        LiveOpenStateV1::Closed => 0,
        LiveOpenStateV1::Unknown => 1,
        LiveOpenStateV1::Open => 2,
    }
}

const fn relevance(rank: u32) -> u32 {
    u32::MAX - rank
}

const fn directed(ordering: Ordering, direction: SortDirectionV1) -> Ordering {
    match direction {
        SortDirectionV1::Ascending => ordering,
        SortDirectionV1::Descending => ordering.reverse(),
    }
}

fn paginate<T>(mut values: Vec<T>, request: PageRequestV1) -> (PageInfoV1, Vec<T>) {
    let total = values.len() as u64;
    let page_size = u64::from(request.page_size());
    let total_pages = if total == 0 {
        0
    } else {
        total.div_ceil(page_size) as u32
    };
    let start = u64::from(request.page() - 1).saturating_mul(page_size);
    let items = if start >= total {
        Vec::new()
    } else {
        let end = total.min(start + page_size);
        values.drain(start as usize..end as usize).collect()
    };
    (
        PageInfoV1 {
            page: request.page(),
            page_size: request.page_size(),
            total,
            total_pages,
        },
        items,
    )
}

fn elapsed_micros(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_micros()).unwrap_or(u64::MAX)
}

fn open_key_hash(key: &SectionKey) -> u64 {
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish()
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
fn paginate_and_materialize<T, U>(
    values: Vec<T>,
    request: PageRequestV1,
    materialize: impl FnMut(T) -> U,
) -> (PageInfoV1, Vec<U>) {
    let (page, values) = paginate(values, request);
    (page, values.into_iter().map(materialize).collect())
}

fn exact_course_identifier(filters: &NormalizedFilterValuesV1, key: &CourseGroupKey) -> bool {
    filters.text().is_some_and(|text| {
        text.tokens().len() == 1
            && text
                .as_str()
                .eq_ignore_ascii_case(key.course_string().as_str())
    })
}

fn known_string(value: &CatalogFieldKnowledge<String>) -> Option<&str> {
    match value {
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Present { value },
        } => Some(value),
        _ => None,
    }
}

fn or_nonempty(
    values: Vec<PredicateEvaluation>,
    empty_reason: &'static str,
) -> PredicateEvaluation {
    if values.is_empty() {
        PredicateEvaluation::no_match(empty_reason)
    } else {
        or_active(values)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    fn constant_open_hash(_: &SectionKey) -> u64 {
        11
    }

    fn open_evidence(index: &str) -> OpenEvidence {
        OpenEvidence {
            section_key: SectionKey::try_new("92026", "NB", index).expect("section key"),
            evidence: LiveOpenEvidenceV1 {
                state: LiveOpenStateV1::Open,
                observed_at: None,
                fresh_until: None,
                uncertainty: None,
            },
        }
    }

    #[test]
    fn pagination_invokes_materialization_only_for_the_selected_large_result_page() {
        let materialized = Cell::new(0_u32);
        let request = PageRequestV1::try_new(137, 25).expect("valid page request");
        let (page, items) =
            paginate_and_materialize((0_u32..8_037).collect(), request, |candidate| {
                materialized.set(materialized.get() + 1);
                candidate
            });

        assert_eq!(page.total, 8_037);
        assert_eq!(page.total_pages, 322);
        assert_eq!(materialized.get(), 25);
        assert_eq!(items, (3_400_u32..3_425).collect::<Vec<_>>());
    }

    #[test]
    fn open_index_collision_keeps_distinct_keys_and_rejects_exact_duplicates() {
        let evidence = vec![open_evidence("10001"), open_evidence("10002")];
        let index = build_open_index_with_hasher(&evidence, constant_open_hash)
            .expect("distinct colliding keys");
        assert!(matches!(index.get(&11), Some(OpenIndexBucket::Many(values)) if values == &[0, 1]));
        for item in &evidence {
            assert_eq!(
                lookup_open_evidence_with_hasher(
                    &index,
                    &evidence,
                    &item.section_key,
                    constant_open_hash,
                ),
                Some(&item.evidence)
            );
        }
        let missing = SectionKey::try_new("92026", "NB", "10003").expect("missing key");
        assert_eq!(
            lookup_open_evidence_with_hasher(&index, &evidence, &missing, constant_open_hash,),
            None
        );

        let duplicates = vec![evidence[0].clone(), evidence[0].clone()];
        assert!(matches!(
            build_open_index_with_hasher(&duplicates, constant_open_hash),
            Err(QueryError::DuplicateOpenEvidence)
        ));
    }
}
