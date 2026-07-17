use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

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
    evaluate_course_filter_matches, evaluate_section_filters, matched_explanation,
    section_filters_active,
};
use crate::{CatalogCorpus, CorpusError, PredicateEvaluation, TextHitPlan, and_all, or_active};

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
    #[error("course was not found")]
    CourseNotFound,
    #[error("section was not found")]
    SectionNotFound,
}

pub struct QueryEngine<'a> {
    corpus: CatalogCorpus<'a>,
    now: OffsetDateTime,
    open: BTreeMap<SectionKey, LiveOpenEvidenceV1>,
}

impl<'a> QueryEngine<'a> {
    pub fn try_new(
        catalogs: &'a [NormalizedCatalogV1],
        now: OffsetDateTime,
        open: impl IntoIterator<Item = OpenEvidence>,
    ) -> Result<Self, QueryError> {
        let corpus = CatalogCorpus::try_new(catalogs)?;
        let mut by_section = BTreeMap::new();
        for item in open {
            if corpus.section(&item.section_key).is_none() {
                return Err(QueryError::ForeignOpenEvidence);
            }
            if by_section.insert(item.section_key, item.evidence).is_some() {
                return Err(QueryError::DuplicateOpenEvidence);
            }
        }
        Ok(Self {
            corpus,
            now,
            open: by_section,
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

        for group in self.corpus.groups() {
            let exact_identifier = exact_course_identifier(filters, &group.key);
            let mut variant_evaluations = Vec::new();
            let mut best_rank = u32::MAX;
            let mut title = None::<&str>;

            for variant in self.corpus.variants_for(group) {
                let (course_evaluation, _) =
                    evaluate_course_filter_matches(&group.key, variant, filters, text_hits);
                let section_witness = if section_filters_active {
                    let sections = self.corpus.sections_for(variant);
                    if sections.is_empty() {
                        PredicateEvaluation::no_match("section.witness")
                    } else {
                        or_active(
                            sections
                                .into_iter()
                                .map(|section| self.section_evaluation(section, filters)),
                        )
                    }
                } else {
                    PredicateEvaluation::matched()
                };
                let variant_evaluation = and_all([course_evaluation.clone(), section_witness]);
                if variant_evaluation.outcome() != MatchOutcome::NoMatch {
                    if let Some(rank) = text_hits.and_then(|plan| plan.rank(&variant.key)) {
                        best_rank = best_rank.min(rank);
                    }
                    if let Some(candidate) =
                        known_string(&variant.title).filter(|value| !value.is_empty())
                        && title.is_none_or(|current| candidate < current)
                    {
                        title = Some(candidate);
                    }
                }
                variant_evaluations.push(variant_evaluation);
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
        }

        candidates.sort_by(|left, right| compare_course(left, right, request));
        let (page, items) = paginate_and_materialize(candidates, request.page, |candidate| {
            self.materialize_course_candidate(candidate, filters, text_hits, section_filters_active)
        });
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
        for section in self.corpus.sections() {
            let variant = self
                .corpus
                .variant(&section.variant_key)
                .expect("validated corpus contains the section variant");
            let (course_evaluation, _) =
                evaluate_course_filter_matches(variant.key.group(), variant, filters, text_hits);
            let section_evaluation = self.section_evaluation(section, filters);
            let overall = and_all([course_evaluation, section_evaluation]);
            if overall.outcome() == MatchOutcome::NoMatch {
                continue;
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
        }

        candidates.sort_by(|left, right| compare_section(left, right, request));
        let (page, items) = paginate_and_materialize(candidates, request.page, |candidate| {
            self.materialize_section_candidate(candidate, filters, text_hits)
        });
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
                let (course_evaluation, course_filter_matches) =
                    evaluate_course_filter_matches(&group.key, variant, filters, text_hits);
                let section_evaluations = self
                    .corpus
                    .sections_for(variant)
                    .into_iter()
                    .map(|section| self.section_item(section, filters))
                    .collect::<Vec<_>>();
                let section_witness = if section_filters_active {
                    if section_evaluations.is_empty() {
                        PredicateEvaluation::no_match("section.witness")
                    } else {
                        or_active(
                            section_evaluations
                                .iter()
                                .map(|(_, evaluation)| evaluation.clone()),
                        )
                    }
                } else {
                    PredicateEvaluation::matched()
                };
                let variant_evaluation = and_all([course_evaluation.clone(), section_witness]);
                if variant_evaluation.outcome() == MatchOutcome::NoMatch {
                    return None;
                }
                let sections = section_evaluations
                    .into_iter()
                    .filter_map(|(mut item, section_evaluation)| {
                        let overall = and_all([course_evaluation.clone(), section_evaluation]);
                        if section_filters_active && overall.outcome() == MatchOutcome::NoMatch {
                            return None;
                        }
                        item.explanation = overall.into_explanation();
                        Some(item)
                    })
                    .collect();
                Some(CourseVariantQueryItemV1 {
                    variant: variant.clone(),
                    explanation: variant_evaluation.into_explanation(),
                    filter_matches: course_filter_matches,
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
        let (course_evaluation, course_filter_matches) =
            evaluate_course_filter_matches(variant.key.group(), variant, filters, text_hits);
        let (mut item, section_evaluation) = self.section_item(section, filters);
        debug_assert_eq!(evaluation, and_all([course_evaluation, section_evaluation]));
        item.explanation = evaluation.into_explanation();
        SectionSearchItemV1 {
            variant: variant.clone(),
            section: item,
            course_filter_matches,
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
    ) -> (SectionQueryItemV1, PredicateEvaluation) {
        let occurrences = self.corpus.known_occurrences_for(section);
        let open = self.open_for(&section.key);
        let (evaluation, filter_matches) =
            evaluate_section_filters(section, occurrences.as_deref(), &open, self.now, filters);
        (
            SectionQueryItemV1 {
                section: section.clone(),
                occurrences: occurrences
                    .unwrap_or_default()
                    .into_iter()
                    .cloned()
                    .collect(),
                open,
                explanation: evaluation.clone().into_explanation(),
                filter_matches,
            },
            evaluation,
        )
    }

    fn section_evaluation(
        &self,
        section: &bcsp_contracts::NormalizedSectionV1,
        filters: &NormalizedFilterValuesV1,
    ) -> PredicateEvaluation {
        let occurrences = self.corpus.known_occurrences_for(section);
        let open = self.open_for(&section.key);
        evaluate_section_filters(section, occurrences.as_deref(), &open, self.now, filters).0
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
        self.open.get(key).cloned().unwrap_or(LiveOpenEvidenceV1 {
            state: LiveOpenStateV1::Unknown,
            observed_at: None,
            fresh_until: None,
            uncertainty: Some(MatchReasonCode::MissingReliableData),
        })
    }

    fn open_state_for(&self, key: &SectionKey) -> LiveOpenStateV1 {
        self.open
            .get(key)
            .map(|evidence| evidence.state)
            .unwrap_or(LiveOpenStateV1::Unknown)
    }

    fn validate_filters(&self, filters: &NormalizedFilterValuesV1) -> Result<(), QueryError> {
        if filters.term().as_str() != self.corpus.term() {
            return Err(QueryError::FilterTermMismatch);
        }
        if !filters.core().codes.is_empty() {
            let authoritative_codes = self
                .corpus
                .groups()
                .filter(|group| {
                    filters.campuses().is_empty() || filters.campuses().contains(group.key.campus())
                })
                .flat_map(|group| self.corpus.variants_for(group))
                .filter_map(|variant| match &variant.core_codes {
                    CatalogFieldKnowledge::Known {
                        presence: CatalogFieldPresence::Present { value },
                    } => Some(value),
                    CatalogFieldKnowledge::Known { .. } | CatalogFieldKnowledge::Unknown { .. } => {
                        None
                    }
                })
                .flatten()
                .map(|code| code.to_ascii_uppercase())
                .collect::<BTreeSet<_>>();
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
            .corpus
            .target_versions()
            .filter(|(target, _)| {
                filters.campuses().is_empty() || filters.campuses().contains(target.campus())
            })
            .map(|(target, version)| (target.clone(), version))
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
            .any(|key| self.corpus.variant(key).is_none())
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
}
