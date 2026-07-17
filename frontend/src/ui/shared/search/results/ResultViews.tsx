import { useId, useState, type MouseEvent } from 'react';

import type {
  CatalogFieldKnowledge,
  CourseDetailResponseV1,
  CourseGroupKey,
  CourseQueryItemV1,
  CourseQueryResponseV1,
  CourseVariantQueryItemV1,
  FilterMatchV1,
  MatchExplanation,
  NormalizedCourseVariantV1,
  NormalizedOccurrenceV1,
  PageInfoV1,
  SectionDetailResponseV1,
  SectionKey,
  SectionQueryItemV1,
  SectionQueryResponseV1,
} from '../../product';
import {
  catalogUnknownReasonMessageKeys,
  filterOptionMessageKey,
  openStateMessageKeys,
  openUncertaintyMessageKeys,
} from '../../i18n/presenter';
import { useBcspI18n, type BcspI18nRuntime } from '../../i18n/runtime';
import { SectionSelectionAction } from '../../watch';
import { SearchResultsStyles } from './styles';

export interface ResultNavigationProps {
  readonly onCourseDetail: (key: CourseGroupKey) => void;
  readonly sectionHref: (key: SectionKey) => string;
  readonly onSectionNavigate?: ((key: SectionKey) => void) | undefined;
}

export interface SectionDisclosureStateProps {
  readonly expandedSectionDisclosures?: ReadonlySet<string> | undefined;
  readonly onSectionDisclosureChange?: ((disclosureId: string, expanded: boolean) => void) | undefined;
}

export interface CourseResultsViewProps extends ResultNavigationProps, SectionDisclosureStateProps {
  readonly response: CourseQueryResponseV1;
  readonly onPageChange: (page: number) => void;
}

export interface SectionResultsViewProps extends ResultNavigationProps {
  readonly response: SectionQueryResponseV1;
  readonly onPageChange: (page: number) => void;
}

export interface CourseDetailViewProps extends SectionDisclosureStateProps {
  readonly response: CourseDetailResponseV1;
  readonly sectionHref: (key: SectionKey) => string;
  readonly onSectionNavigate?: ((key: SectionKey) => void) | undefined;
}

export interface SectionDetailViewProps {
  readonly response: SectionDetailResponseV1;
  readonly sectionHref: (key: SectionKey) => string;
  readonly onSectionNavigate?: ((key: SectionKey) => void) | undefined;
}

function formatGroupKey(key: CourseGroupKey): string {
  return `${key.courseString} / ${key.term} / ${key.campus}`;
}

function formatSectionKey(key: SectionKey): string {
  return `${key.index} / ${key.term} / ${key.campus}`;
}

function formatVariantDisclosureId(variant: NormalizedCourseVariantV1): string {
  const { group, fingerprint } = variant.key;
  return `${group.term}\u0000${group.campus}\u0000${group.courseString}\u0000${fingerprint}`;
}

function formatKnowledge<T>(
  field: CatalogFieldKnowledge<T>,
  i18n: BcspI18nRuntime,
  format: (value: T) => string = String,
): string {
  if (field.knowledge === 'UNKNOWN') {
    return `${i18n.t('common.unknown')} — ${i18n.t(catalogUnknownReasonMessageKeys[field.reason])}`;
  }
  if (field.presence.presence === 'ABSENT') return i18n.t('common.not_reported');
  if (field.presence.presence === 'EXPLICIT_NULL') return i18n.t('common.explicit_null');
  return format(field.presence.value);
}

function formatArray(
  field: CatalogFieldKnowledge<readonly string[]>,
  i18n: BcspI18nRuntime,
  formatItem: (value: string) => string = String,
): string {
  return formatKnowledge(
    field,
    i18n,
    (value) => value.length === 0
      ? i18n.t('common.not_reported')
      : value.map(formatItem).join(', '),
  );
}

function formatCredits(value: string): string {
  const encoded = /^(\d+)_(\d+)$/u.exec(value.trim());
  if (encoded === null) return value;
  const whole = encoded[1];
  const fraction = encoded[2]?.replace(/0+$/u, '');
  return fraction === undefined || fraction.length === 0 ? (whole ?? value) : `${whole}.${fraction}`;
}

function formatMinute(value: number): string {
  const hours = Math.floor(value / 60).toString().padStart(2, '0');
  const minutes = (value % 60).toString().padStart(2, '0');
  return `${hours}:${minutes}`;
}

function formatOccurrenceTime(occurrence: NormalizedOccurrenceV1, i18n: BcspI18nRuntime): string {
  if (occurrence.time.knowledge !== 'KNOWN') return i18n.t('common.unknown');
  return `${formatMinute(occurrence.time.startMinute)}–${formatMinute(occurrence.time.endMinute)}`;
}

function reasonText(explanation: MatchExplanation, i18n: BcspI18nRuntime): string {
  if (explanation.reasons.length === 0) return i18n.t('common.none');
  return explanation.reasons.map((reason) => `${reason.field}: ${reason.code}`).join('; ');
}

function formatDateTime(value: string | null, i18n: BcspI18nRuntime): string {
  if (value === null) return i18n.t('freshness.never_observed');
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp)
    ? i18n.formatDate(timestamp, { dateStyle: 'medium', timeStyle: 'short' })
    : i18n.t('common.invalid_timestamp');
}

function optionTextForResult(value: string, i18n: BcspI18nRuntime): string {
  const key = filterOptionMessageKey(value);
  return key === undefined ? value : i18n.t(key);
}

function Fact({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <div className="search-results__fact">
      <span className="search-results__label">{label}</span>
      <span className="search-results__value">{value}</span>
    </div>
  );
}

function VariantFacts({ variant }: { readonly variant: NormalizedCourseVariantV1 }) {
  const i18n = useBcspI18n();
  return (
    <div className="search-results__variant-facts" aria-label={i18n.t('result.variant_fields')}>
      <Fact label={i18n.t('result.title')} value={formatKnowledge(variant.title, i18n)} />
      <Fact label={i18n.t('course.credits')} value={formatKnowledge(variant.credits, i18n, formatCredits)} />
      <Fact label={i18n.t('result.supplement')} value={formatKnowledge(variant.supplementCode, i18n)} />
    </div>
  );
}

function SectionLink({
  section,
  sectionHref,
  onSectionNavigate,
}: {
  readonly section: SectionKey;
  readonly sectionHref: (key: SectionKey) => string;
  readonly onSectionNavigate?: ((key: SectionKey) => void) | undefined;
}) {
  const { t } = useBcspI18n();
  function navigate(event: MouseEvent<HTMLAnchorElement>) {
    if (
      onSectionNavigate === undefined
      || event.button !== 0
      || event.metaKey
      || event.ctrlKey
      || event.shiftKey
      || event.altKey
    ) return;
    event.preventDefault();
    onSectionNavigate(section);
  }

  return (
    <a
      className="search-results__section-link"
      href={sectionHref(section)}
      onClick={navigate}
    >
      {t('result.open_section')}
    </a>
  );
}

function OutcomeBadge({ explanation }: { readonly explanation: MatchExplanation }) {
  const modifier = explanation.outcome === 'MATCH'
    ? 'match'
    : explanation.outcome === 'UNCERTAIN'
      ? 'uncertain'
      : 'no-match';
  return (
    <span className={`search-results__badge search-results__badge--${modifier}`}>
      {explanation.outcome}
    </span>
  );
}

function LiveBadge({ section }: { readonly section: SectionQueryItemV1 }) {
  const { t } = useBcspI18n();
  return (
    <span className={`search-results__badge search-results__badge--${section.open.state.toLowerCase()}`}>
      {t('result.live_state', { state: section.open.state })}
    </span>
  );
}

function FilterWitness({
  explanation,
  matches,
}: {
  readonly explanation: MatchExplanation;
  readonly matches: readonly FilterMatchV1[];
}) {
  const i18n = useBcspI18n();
  const fallbackReasons = explanation.reasons;
  return (
    <section
      className={`search-results__witness${explanation.outcome === 'UNCERTAIN' ? ' search-results__witness--uncertain' : ''}`}
      aria-label={i18n.t('result.same_section_witness')}
    >
      <strong className="search-results__witness-title">{i18n.t('result.witness_title')}</strong>
      <ul className="search-results__witness-list">
        {matches.map((match, index) => (
          <li className="search-results__witness-item" key={`${match.fieldId}-${index}`}>
            <span className="search-results__label">
              {match.fieldId} · {match.explanation.outcome}
            </span>
            <span>{reasonText(match.explanation, i18n)}</span>
          </li>
        ))}
        {matches.length === 0 && fallbackReasons.map((reason, index) => (
          <li className="search-results__witness-item" key={`${reason.field}-${reason.code}-${index}`}>
            <span className="search-results__label">{i18n.t('result.reason')}</span>
            <span>{reason.field}: {reason.code}</span>
          </li>
        ))}
        {matches.length === 0 && fallbackReasons.length === 0 ? (
          <li className="search-results__witness-item">
            <span className="search-results__label">{i18n.t('result.witness')}</span>
            <span>{i18n.t('result.match_confirmed')}</span>
          </li>
        ) : null}
      </ul>
    </section>
  );
}

function Occurrences({ occurrences }: { readonly occurrences: readonly NormalizedOccurrenceV1[] }) {
  const i18n = useBcspI18n();
  return (
    <section className="search-results__occurrences" aria-label={i18n.t('result.meeting_occurrences')}>
      <strong className="search-results__occurrence-title">
        {i18n.t('result.occurrences_count', { count: i18n.formatNumber(occurrences.length) })}
      </strong>
      {occurrences.length === 0 ? (
        <p className="search-results__value">{i18n.t('result.no_occurrences')}</p>
      ) : (
        <ol className="search-results__occurrence-list">
          {occurrences.map((occurrence) => (
            <li
              className="search-results__occurrence"
              key={`${occurrence.key.section.index}-${occurrence.key.ordinal}`}
            >
              <span className="search-results__label">
                {i18n.t('result.occurrence', { number: i18n.formatNumber(occurrence.key.ordinal) })}
              </span>
              <span>
                {formatArray(occurrence.days, i18n)} · {formatOccurrenceTime(occurrence, i18n)} ·{' '}
                {formatKnowledge(occurrence.building, i18n)} {formatKnowledge(occurrence.room, i18n)}
              </span>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

function SectionResult({
  item,
  sectionHref,
  onSectionNavigate,
}: {
  readonly item: SectionQueryItemV1;
  readonly sectionHref: (key: SectionKey) => string;
  readonly onSectionNavigate?: ((key: SectionKey) => void) | undefined;
}) {
  const i18n = useBcspI18n();
  return (
    <article
      className="search-results__section"
      data-match-outcome={item.explanation.outcome}
      data-section-index={item.section.key.index}
    >
      <header className="search-results__section-header">
        <div>
          <span className="search-results__eyebrow">{i18n.t('result.section_identity')}</span>
          <h4 className="search-results__section-title">
            {formatKnowledge(item.section.sectionNumber, i18n)} · {item.section.key.index}
          </h4>
          <div className="search-results__identity">
            <data className="search-results__meta" value={item.section.key.term}>{item.section.key.term}</data>
            <data className="search-results__meta" value={item.section.key.campus}>{item.section.key.campus}</data>
            <span className="search-results__meta">
              {i18n.t(filterOptionMessageKey(item.section.deliveryModality) ?? 'common.unknown')}
            </span>
            <span className="search-results__meta">
              {i18n.t(filterOptionMessageKey(item.section.synchronicity) ?? 'common.unknown')}
            </span>
          </div>
        </div>
        <div className="search-results__section-actions">
          <div className="search-results__badges">
            <OutcomeBadge explanation={item.explanation} />
            <LiveBadge section={item} />
          </div>
          <SectionSelectionAction sectionKey={item.section.key} />
          <SectionLink
            onSectionNavigate={onSectionNavigate}
            section={item.section.key}
            sectionHref={sectionHref}
          />
        </div>
      </header>
      <div className="search-results__live" aria-label={i18n.t('result.live_freshness')}>
        <Fact label={i18n.t('result.open_state')} value={i18n.t(openStateMessageKeys[item.open.state])} />
        <Fact label={i18n.t('result.observed_at')} value={formatDateTime(item.open.observedAt, i18n)} />
        <Fact
          label={i18n.t('freshness.fresh')}
          value={item.open.freshUntil === null
            ? i18n.t('result.no_fresh_until')
            : i18n.t('result.fresh_until', { time: formatDateTime(item.open.freshUntil, i18n) })}
        />
      </div>
      {item.open.uncertainty === null ? null : (
        <p className="search-results__meta">
          {i18n.t('result.open_uncertainty', {
            reason: item.open.uncertainty,
          })}
        </p>
      )}
      <FilterWitness explanation={item.explanation} matches={item.filterMatches} />
      <Occurrences occurrences={item.occurrences} />
    </article>
  );
}

function VariantResult({
  expandedSectionDisclosures,
  item,
  onSectionDisclosureChange,
  sectionHref,
  onSectionNavigate,
}: {
  readonly expandedSectionDisclosures?: ReadonlySet<string> | undefined;
  readonly item: CourseVariantQueryItemV1;
  readonly onSectionDisclosureChange?: ((disclosureId: string, expanded: boolean) => void) | undefined;
  readonly sectionHref: (key: SectionKey) => string;
  readonly onSectionNavigate?: ((key: SectionKey) => void) | undefined;
}) {
  const i18n = useBcspI18n();
  return (
    <article className="search-results__variant" data-variant-fingerprint={item.variant.key.fingerprint}>
      <header className="search-results__variant-summary">
        <div>
          <span className="search-results__eyebrow">
            {i18n.t('result.variant', { fingerprint: item.variant.key.fingerprint })}
          </span>
          <h3 className="search-results__variant-title">{formatKnowledge(item.variant.title, i18n)}</h3>
        </div>
        <div className="search-results__badges">
          <OutcomeBadge explanation={item.explanation} />
        </div>
      </header>
      <VariantFacts variant={item.variant} />
      <SectionDisclosure
        disclosureId={formatVariantDisclosureId(item.variant)}
        expandedSectionDisclosures={expandedSectionDisclosures}
        onSectionDisclosureChange={onSectionDisclosureChange}
        onSectionNavigate={onSectionNavigate}
        sectionHref={sectionHref}
        sections={item.sections.filter(({ explanation }) => explanation.outcome !== 'NO_MATCH')}
      />
    </article>
  );
}

function SectionDisclosure({
  disclosureId,
  expandedSectionDisclosures,
  onSectionDisclosureChange,
  sections,
  sectionHref,
  onSectionNavigate,
}: {
  readonly disclosureId: string;
  readonly expandedSectionDisclosures?: ReadonlySet<string> | undefined;
  readonly onSectionDisclosureChange?: ((disclosureId: string, expanded: boolean) => void) | undefined;
  readonly sections: readonly SectionQueryItemV1[];
  readonly sectionHref: (key: SectionKey) => string;
  readonly onSectionNavigate?: ((key: SectionKey) => void) | undefined;
}) {
  const i18n = useBcspI18n();
  const contentId = useId();
  const [localExpanded, setLocalExpanded] = useState(false);
  const [mounted, setMounted] = useState(false);
  const controlled = expandedSectionDisclosures !== undefined
    && onSectionDisclosureChange !== undefined;
  const expanded = controlled
    ? expandedSectionDisclosures.has(disclosureId)
    : localExpanded;
  const reflectOpen = (open: boolean) => {
    if (controlled) onSectionDisclosureChange(disclosureId, open);
    else setLocalExpanded(open);
    if (open) setMounted(true);
  };
  return (
    <details
      className="search-results__section-disclosure"
      onToggle={(event) => {
        if (event.currentTarget.open !== expanded) reflectOpen(event.currentTarget.open);
      }}
      open={expanded}
    >
      <summary
        aria-controls={mounted || expanded ? contentId : undefined}
        className="search-results__section-disclosure-summary"
        onClick={(event) => {
          event.preventDefault();
          reflectOpen(!expanded);
        }}
      >
        <span>{i18n.t(expanded ? 'result.sections_collapse' : 'result.sections_expand', {
          count: i18n.formatNumber(sections.length),
        })}</span>
        <span aria-hidden="true" className="search-results__section-disclosure-action">
          {expanded ? '−' : '+'}
        </span>
      </summary>
      {mounted || expanded ? (
        <div className="search-results__section-list" id={contentId}>
          {sections.map((section) => (
            <SectionResult
              item={section}
              key={formatSectionKey(section.section.key)}
              onSectionNavigate={onSectionNavigate}
              sectionHref={sectionHref}
            />
          ))}
        </div>
      ) : null}
    </details>
  );
}

function CourseGroupResult({
  expandedSectionDisclosures,
  item,
  onCourseDetail,
  onSectionDisclosureChange,
  sectionHref,
  onSectionNavigate,
}: {
  readonly expandedSectionDisclosures?: ReadonlySet<string> | undefined;
  readonly item: CourseQueryItemV1;
  readonly onCourseDetail: (key: CourseGroupKey) => void;
  readonly onSectionDisclosureChange?: ((disclosureId: string, expanded: boolean) => void) | undefined;
  readonly sectionHref: (key: SectionKey) => string;
  readonly onSectionNavigate?: ((key: SectionKey) => void) | undefined;
}) {
  const i18n = useBcspI18n();
  const visibleVariants = item.variants.filter(({ explanation }) => explanation.outcome !== 'NO_MATCH');
  if (visibleVariants.length === 0) return null;
  return (
    <article className="search-results__group" data-course-group={item.group.key.courseString}>
      <header className="search-results__group-header">
        <div>
          <span className="search-results__eyebrow">{i18n.t('result.course_group')}</span>
          <h2 className="search-results__group-title">{item.group.key.courseString}</h2>
          <div className="search-results__identity">
            <span className="search-results__meta">{i18n.t('common.term')} {item.group.key.term}</span>
            <span className="search-results__meta">{i18n.t('common.campus')} {item.group.key.campus}</span>
            <span className="search-results__meta">
              {i18n.t('result.explicit_variants', { count: i18n.formatNumber(visibleVariants.length) })}
            </span>
          </div>
        </div>
        <button
          className="search-results__button"
          onClick={() => onCourseDetail(item.group.key)}
          type="button"
        >
          {i18n.t('result.course_detail')}
        </button>
      </header>
      <div className="search-results__variant-list">
        {visibleVariants.map((variant) => (
          <VariantResult
            expandedSectionDisclosures={expandedSectionDisclosures}
            item={variant}
            key={variant.variant.key.fingerprint}
            onSectionDisclosureChange={onSectionDisclosureChange}
            onSectionNavigate={onSectionNavigate}
            sectionHref={sectionHref}
          />
        ))}
      </div>
    </article>
  );
}

function Pagination({ page, onPageChange }: {
  readonly page: PageInfoV1;
  readonly onPageChange: (page: number) => void;
}) {
  const i18n = useBcspI18n();
  const hasPrevious = page.page > 1;
  const hasNext = page.page < page.totalPages;
  return (
    <nav className="search-results__pagination" aria-label={i18n.t('result.pages')}>
      <span className="search-results__page-label">
        {i18n.t('result.page_summary', {
          count: i18n.formatNumber(page.total),
          page: i18n.formatNumber(page.page),
          pages: i18n.formatNumber(page.totalPages || 1),
        })}
      </span>
      <div className="search-results__pagination-actions">
        <button
          className="search-results__button"
          disabled={!hasPrevious}
          onClick={() => onPageChange(page.page - 1)}
          type="button"
        >
          {i18n.t('common.previous')}
        </button>
        <button
          className="search-results__button"
          disabled={!hasNext}
          onClick={() => onPageChange(page.page + 1)}
          type="button"
        >
          {i18n.t('common.next')}
        </button>
      </div>
    </nav>
  );
}

function ResultsHeader({ kind, page }: { readonly kind: string; readonly page: PageInfoV1 }) {
  const i18n = useBcspI18n();
  return (
    <header className="search-results__header">
      <div>
        <span className="search-results__eyebrow">{i18n.t('result.search_output')}</span>
        <h1 className="search-results__heading">{kind}</h1>
      </div>
      <data className="search-results__count" value={page.total}>{i18n.formatNumber(page.total)}</data>
    </header>
  );
}

export function CourseResultsView({
  expandedSectionDisclosures,
  response,
  onCourseDetail,
  onSectionDisclosureChange,
  sectionHref,
  onSectionNavigate,
  onPageChange,
}: CourseResultsViewProps) {
  const i18n = useBcspI18n();
  const visibleItems = response.items.filter((item) =>
    item.variants.some(({ explanation }) => explanation.outcome !== 'NO_MATCH'));
  return (
    <section className="search-results" aria-label={i18n.t('result.course_results_label')}>
      <SearchResultsStyles />
      <ResultsHeader kind={i18n.t('result.course_results')} page={response.page} />
      {visibleItems.length === 0 ? (
        <p className="search-results__empty">{i18n.t('result.no_courses')}</p>
      ) : (
        <div className="search-results__list">
          {visibleItems.map((item) => (
            <CourseGroupResult
              expandedSectionDisclosures={expandedSectionDisclosures}
              item={item}
              key={formatGroupKey(item.group.key)}
              onCourseDetail={onCourseDetail}
              onSectionDisclosureChange={onSectionDisclosureChange}
              onSectionNavigate={onSectionNavigate}
              sectionHref={sectionHref}
            />
          ))}
        </div>
      )}
      <Pagination onPageChange={onPageChange} page={response.page} />
    </section>
  );
}

export function SectionResultsView({
  response,
  onCourseDetail,
  sectionHref,
  onSectionNavigate,
  onPageChange,
}: SectionResultsViewProps) {
  const i18n = useBcspI18n();
  const visibleItems = response.items.filter(({ section }) => section.explanation.outcome !== 'NO_MATCH');
  return (
    <section className="search-results" aria-label={i18n.t('result.section_results_label')}>
      <SearchResultsStyles />
      <ResultsHeader kind={i18n.t('result.section_results')} page={response.page} />
      {visibleItems.length === 0 ? (
        <p className="search-results__empty">{i18n.t('result.no_sections')}</p>
      ) : (
        <div className="search-results__list">
          {visibleItems.map((item) => (
            <article className="search-results__standalone-section" key={formatSectionKey(item.section.section.key)}>
              <header className="search-results__group-header">
                <div>
                  <span className="search-results__eyebrow">{i18n.t('result.course_variant')}</span>
                  <h2 className="search-results__group-title">
                    {item.variant.key.group.courseString} · {formatKnowledge(item.variant.title, i18n)}
                  </h2>
                  <span className="search-results__meta">
                    {i18n.t('result.variant', { fingerprint: item.variant.key.fingerprint })}
                  </span>
                </div>
                <button
                  className="search-results__button"
                  onClick={() => onCourseDetail(item.variant.key.group)}
                  type="button"
                >
                  {i18n.t('result.course_detail')}
                </button>
              </header>
              <VariantFacts variant={item.variant} />
              <SectionResult
                item={item.section}
                onSectionNavigate={onSectionNavigate}
                sectionHref={sectionHref}
              />
            </article>
          ))}
        </div>
      )}
      <Pagination onPageChange={onPageChange} page={response.page} />
    </section>
  );
}

function DetailFields({ variant }: { readonly variant: NormalizedCourseVariantV1 }) {
  const i18n = useBcspI18n();
  const fields = [
    [i18n.t('result.expanded_title'), formatKnowledge(variant.expandedTitle, i18n)],
    [i18n.t('course.credits'), formatKnowledge(variant.credits, i18n, formatCredits)],
    [i18n.t('result.supplement'), formatKnowledge(variant.supplementCode, i18n)],
    [i18n.t('course.description'), formatKnowledge(variant.description, i18n)],
    [i18n.t('result.prerequisite'), formatKnowledge(variant.prerequisiteNotes, i18n)],
    [i18n.t('result.prerequisite_state'), optionTextForResult(variant.prerequisiteState, i18n)],
    [i18n.t('result.level'), formatKnowledge(variant.level, i18n)],
    [i18n.t('course.subject'), `${formatKnowledge(variant.subjectCode, i18n)} · ${formatKnowledge(variant.subjectDescription, i18n)}`],
    [i18n.t('result.offering_unit'), `${formatKnowledge(variant.offeringUnit, i18n)} · ${formatKnowledge(variant.offeringUnitTitle, i18n)}`],
    [i18n.t('course.core_codes'), formatArray(variant.coreCodes, i18n)],
    [i18n.t('result.campus_locations'), formatArray(variant.campusLocations, i18n)],
    [i18n.t('result.synopsis_url'), formatKnowledge(variant.synopsisUrl, i18n)],
  ] as const;
  return (
    <dl className="search-results__field-list">
      {fields.map(([label, value]) => (
        <div className="search-results__field" key={label}>
          <dt className="search-results__label">{label}</dt>
          <dd className="search-results__value">{value}</dd>
        </div>
      ))}
    </dl>
  );
}

export function CourseDetailView({
  expandedSectionDisclosures,
  onSectionDisclosureChange,
  onSectionNavigate,
  response,
  sectionHref,
}: CourseDetailViewProps) {
  const i18n = useBcspI18n();
  const course = response.course;
  return (
    <section className="search-results" aria-label={i18n.t('result.course_detail')}>
      <SearchResultsStyles />
      <article className="search-results__detail">
        <header className="search-results__detail-header">
          <div>
            <span className="search-results__eyebrow">{i18n.t('result.course_detail')}</span>
            <h1 className="search-results__detail-title">{course.group.key.courseString}</h1>
            <span className="search-results__meta">{formatGroupKey(course.group.key)}</span>
          </div>
          <OutcomeBadge explanation={course.explanation} />
        </header>
        {course.variants.map((variant) => (
          <article className="search-results__variant" key={variant.variant.key.fingerprint}>
            <header className="search-results__variant-summary">
              <div>
                <span className="search-results__eyebrow">
                  {i18n.t('result.variant', { fingerprint: variant.variant.key.fingerprint })}
                </span>
                <h2 className="search-results__variant-title">{formatKnowledge(variant.variant.title, i18n)}</h2>
              </div>
              <OutcomeBadge explanation={variant.explanation} />
            </header>
            <DetailFields variant={variant.variant} />
            <SectionDisclosure
              disclosureId={formatVariantDisclosureId(variant.variant)}
              expandedSectionDisclosures={expandedSectionDisclosures}
              onSectionDisclosureChange={onSectionDisclosureChange}
              onSectionNavigate={onSectionNavigate}
              sectionHref={sectionHref}
              sections={variant.sections}
            />
          </article>
        ))}
      </article>
    </section>
  );
}

export function SectionDetailView({ response, sectionHref, onSectionNavigate }: SectionDetailViewProps) {
  const i18n = useBcspI18n();
  const { section, variant } = response;
  return (
    <section className="search-results" aria-label={i18n.t('search.section_detail_title')}>
      <SearchResultsStyles />
      <article className="search-results__detail">
        <header className="search-results__detail-header">
          <div>
            <span className="search-results__eyebrow">{i18n.t('search.section_detail_title')}</span>
            <h1 className="search-results__detail-title">
              {variant.key.group.courseString} · {section.section.key.index}
            </h1>
            <span className="search-results__meta">{formatSectionKey(section.section.key)}</span>
          </div>
          <OutcomeBadge explanation={section.explanation} />
        </header>
        <DetailFields variant={variant} />
        <dl className="search-results__field-list">
          <div className="search-results__field">
            <dt className="search-results__label">{i18n.t('result.section_subtitle')}</dt>
            <dd className="search-results__value">{formatKnowledge(section.section.subtitle, i18n)}</dd>
          </div>
          <div className="search-results__field">
            <dt className="search-results__label">{i18n.t('result.instructors')}</dt>
            <dd className="search-results__value">{formatArray(section.section.instructors, i18n)}</dd>
          </div>
          <div className="search-results__field">
            <dt className="search-results__label">{i18n.t('section.exam_code')}</dt>
            <dd className="search-results__value">
              {formatKnowledge(section.section.examCode, i18n)} · {formatKnowledge(section.section.examCodeText, i18n)}
            </dd>
          </div>
          <div className="search-results__field search-results__field--full-row">
            <dt className="search-results__label">{i18n.t('result.permission_add')}</dt>
            <dd className="search-results__value">
              {formatKnowledge(section.section.specialPermissionAddDescription, i18n)}
            </dd>
          </div>
        </dl>
        <SectionResult
          item={section}
          onSectionNavigate={onSectionNavigate}
          sectionHref={sectionHref}
        />
      </article>
    </section>
  );
}
