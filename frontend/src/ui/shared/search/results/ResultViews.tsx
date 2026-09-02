import { createContext, useContext, useId, useState, type MouseEvent } from 'react';

import type {
  CatalogFieldKnowledge,
  CourseDetailResponseV1,
  CourseGroupKey,
  CourseQueryItemV1,
  CourseQueryResponseV1,
  CourseVariantQueryItemV1,
  MatchExplanation,
  MatchReasonCode,
  NormalizedCourseVariantV1,
  NormalizedOccurrenceV1,
  PageInfoV1,
  SectionDetailResponseV1,
  SectionKey,
  SectionQueryItemV1,
  SectionQueryResponseV1,
} from '../../product';
import type { MessageKey } from '../../i18n/contract';
import {
  catalogUnknownReasonMessageKeys,
  filterOptionMessageKey,
  matchReasonMessageKeys,
  openStateMessageKeys,
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

/* The variant fingerprint never reaches the UI; it only keys the disclosure map. */
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

/** A catalog value only when it is genuinely reported; never a placeholder sentence. */
function reported<T>(
  field: CatalogFieldKnowledge<T>,
  format: (value: T) => string = String,
): string | null {
  if (field.knowledge !== 'KNOWN' || field.presence.presence !== 'PRESENT') return null;
  const text = format(field.presence.value).trim();
  return text.length === 0 ? null : text;
}

/** A detail-grid value, dropped only when it would render a label with nothing after it. */
function detailText<T>(
  field: CatalogFieldKnowledge<T>,
  i18n: BcspI18nRuntime,
  format: (value: T) => string = String,
): string | null {
  const text = formatKnowledge(field, i18n, format).trim();
  return text.length === 0 ? null : text;
}

function joinParts(parts: readonly (string | null)[], separator: string): string | null {
  const kept = parts.filter((part): part is string => part !== null && part.trim().length > 0);
  return kept.length === 0 ? null : kept.join(separator);
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

const shortWeekdayMessageKeys: Readonly<Record<string, MessageKey>> = {
  MONDAY: 'result.day.monday',
  TUESDAY: 'result.day.tuesday',
  WEDNESDAY: 'result.day.wednesday',
  THURSDAY: 'result.day.thursday',
  FRIDAY: 'result.day.friday',
  SATURDAY: 'result.day.saturday',
  SUNDAY: 'result.day.sunday',
};

const levelMessageKeys: Readonly<Record<string, MessageKey>> = {
  U: 'result.level.undergraduate',
  UNDERGRADUATE: 'result.level.undergraduate',
  G: 'result.level.graduate',
  GRADUATE: 'result.level.graduate',
};

function shortDay(value: string, i18n: BcspI18nRuntime): string {
  const key = shortWeekdayMessageKeys[value.toUpperCase()];
  return key === undefined ? value : i18n.t(key);
}

function levelText(field: CatalogFieldKnowledge<string>, i18n: BcspI18nRuntime): string | null {
  const raw = reported(field);
  if (raw === null) return null;
  const key = levelMessageKeys[raw.toUpperCase()];
  return key === undefined ? raw : i18n.t(key);
}

function formatOccurrenceTime(occurrence: NormalizedOccurrenceV1): string | null {
  if (occurrence.time.knowledge !== 'KNOWN') return null;
  return `${formatMinute(occurrence.time.startMinute)}–${formatMinute(occurrence.time.endMinute)}`;
}

/** days · time · place, with every unreported part simply left out. */
function occurrenceSummary(occurrence: NormalizedOccurrenceV1, i18n: BcspI18nRuntime): string {
  const days = reported(occurrence.days, (values) =>
    values.map((day) => shortDay(day, i18n)).join(' '));
  const place = joinParts([reported(occurrence.building), reported(occurrence.room)], ' ');
  return joinParts([days, formatOccurrenceTime(occurrence), place], ' · ')
    ?? i18n.t('result.meeting_unspecified');
}

function formatDateTime(value: string | null, i18n: BcspI18nRuntime): string {
  if (value === null) return i18n.t('freshness.never_observed');
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp)
    ? i18n.formatDate(timestamp, { dateStyle: 'medium', timeStyle: 'short' })
    : i18n.t('common.invalid_timestamp');
}

function formatClock(value: string | null, i18n: BcspI18nRuntime): string | null {
  if (value === null) return null;
  const timestamp = Date.parse(value);
  return Number.isFinite(timestamp) ? i18n.formatDate(timestamp, { timeStyle: 'short' }) : null;
}

function optionTextForResult(value: string, i18n: BcspI18nRuntime): string {
  const key = filterOptionMessageKey(value);
  return key === undefined ? value : i18n.t(key);
}

/** The headline a student scans: the expanded title only when it says more. */
function variantHeadline(variant: NormalizedCourseVariantV1, fallback: string): string {
  const title = reported(variant.title);
  const expanded = reported(variant.expandedTitle);
  if (expanded !== null && (title === null || (expanded !== title && expanded.length > title.length))) {
    return expanded;
  }
  return title ?? fallback;
}

/** The quiet 12.5px line under a headline: only facts that carry information. */
function variantMetaItems(
  variant: NormalizedCourseVariantV1,
  i18n: BcspI18nRuntime,
  options: { readonly credits: boolean },
): string[] {
  const items: string[] = [];
  const credits = options.credits ? reported(variant.credits, formatCredits) : null;
  if (credits !== null) items.push(i18n.t('result.credits_value', { credits }));
  const subject = reported(variant.subjectDescription);
  if (subject !== null) items.push(subject);
  const level = levelText(variant.level, i18n);
  if (level !== null) items.push(level);
  const core = reported(variant.coreCodes, (codes) => codes.join(', '));
  if (core !== null) items.push(i18n.t('result.core_value', { codes: core }));
  const supplement = reported(variant.supplementCode);
  if (supplement !== null) items.push(i18n.t('result.supplement_value', { code: supplement }));
  return items;
}

function MetaItems({ items }: { readonly items: readonly string[] }) {
  return (
    <>
      {items.map((text, index) => (
        <span className="search-results__meta" key={`${index}-${text}`}>{text}</span>
      ))}
    </>
  );
}

function Fact({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <div className="search-results__fact">
      <span className="search-results__label">{label}</span>
      <span className="search-results__value">{value}</span>
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

/**
 * Live seat evidence goes stale for a whole target at once — the first seconds
 * after launch, or a failed Open fetch. Repeating that on every row buries the
 * one fact the reader needs, so a doubt every visible Section shares is stated
 * once above the list and withdrawn from the rows it would otherwise flood.
 */
const SharedOpenDoubtContext = createContext<MatchReasonCode | null>(null);

function sharedOpenDoubt(sections: readonly SectionQueryItemV1[]): MatchReasonCode | null {
  const first = sections[0]?.open.uncertainty ?? null;
  if (first === null) return null;
  return sections.every((section) => section.open.uncertainty === first) ? first : null;
}

function courseSections(items: readonly CourseQueryItemV1[]): SectionQueryItemV1[] {
  return items.flatMap((item) => item.variants
    .filter(({ explanation }) => explanation.outcome !== 'NO_MATCH')
    .flatMap((variant) => variant.sections
      .filter(({ explanation }) => explanation.outcome !== 'NO_MATCH')));
}

/** States a shared doubt once, in the reader's language, above the results. */
function SharedOpenDoubtNotice({ reason }: { readonly reason: MatchReasonCode | null }) {
  const i18n = useBcspI18n();
  if (reason === null) return null;
  return (
    <p className="search-results__note search-results__note--warn" role="status">
      {i18n.t('result.open_evidence_shared', { reason: i18n.t(matchReasonMessageKeys[reason]) })}
    </p>
  );
}

/** A plain MATCH is the norm, so it earns no pill; only doubt is worth a badge. */
function OutcomeBadge({ explanation }: { readonly explanation: MatchExplanation }) {
  const { t } = useBcspI18n();
  const shared = useContext(SharedOpenDoubtContext);
  if (explanation.outcome === 'MATCH') return null;
  // Doubt the notice above already carries is not repeated on the row.
  if (
    shared !== null
    && explanation.outcome === 'UNCERTAIN'
    && explanation.reasons.length > 0
    && explanation.reasons.every((entry) => entry.code === shared)
  ) {
    return null;
  }
  const uncertain = explanation.outcome === 'UNCERTAIN';
  return (
    <span
      className={`search-results__badge search-results__badge--${uncertain ? 'uncertain' : 'no-match'}`}
    >
      {t(uncertain ? 'result.outcome_uncertain' : 'match.outcome.no_match')}
    </span>
  );
}

function LiveBadge({ section }: { readonly section: SectionQueryItemV1 }) {
  const { t } = useBcspI18n();
  return (
    <span className={`search-results__badge search-results__badge--${section.open.state.toLowerCase()}`}>
      {t(openStateMessageKeys[section.open.state])}
    </span>
  );
}

/** Uncertain evidence reads as one quiet warn line, never a red block. */
function EvidenceNote({ item }: { readonly item: SectionQueryItemV1 }) {
  const i18n = useBcspI18n();
  const shared = useContext(SharedOpenDoubtContext);
  const notes = new Set<string>();
  if (item.explanation.outcome !== 'MATCH') {
    for (const reason of item.explanation.reasons) {
      if (reason.code === shared) continue;
      notes.add(i18n.t(matchReasonMessageKeys[reason.code]));
    }
    const onlyShared = item.explanation.reasons.length > 0
      && item.explanation.reasons.every((entry) => entry.code === shared);
    if (notes.size === 0 && !onlyShared) notes.add(i18n.t('result.outcome_uncertain'));
  }
  if (item.open.uncertainty !== null && item.open.uncertainty !== shared) {
    notes.add(i18n.t(matchReasonMessageKeys[item.open.uncertainty]));
  }
  if (notes.size === 0) return null;
  return (
    <p className="search-results__note search-results__note--warn">{[...notes].join(' · ')}</p>
  );
}

function Occurrences({ occurrences }: { readonly occurrences: readonly NormalizedOccurrenceV1[] }) {
  const i18n = useBcspI18n();
  return (
    <div className="search-results__schedule" aria-label={i18n.t('result.meeting_occurrences')}>
      {occurrences.length === 0 ? (
        <p className="search-results__schedule-line">{i18n.t('result.no_occurrences')}</p>
      ) : occurrences.map((occurrence) => (
        <p
          className="search-results__schedule-line"
          key={`${occurrence.key.section.index}-${occurrence.key.ordinal}`}
        >
          {occurrenceSummary(occurrence, i18n)}
        </p>
      ))}
    </div>
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
  const sectionNumber = reported(item.section.sectionNumber);
  const observed = formatClock(item.open.observedAt, i18n);
  const identity = [
    sectionNumber === null ? null : i18n.t('result.section_number', { number: sectionNumber }),
    optionTextForResult(item.section.deliveryModality, i18n),
    optionTextForResult(item.section.synchronicity, i18n),
    observed === null ? null : i18n.t('result.observed_short', { time: observed }),
  ].filter((text): text is string => text !== null);
  return (
    <article
      className="search-results__section"
      data-match-outcome={item.explanation.outcome}
      data-section-index={item.section.key.index}
    >
      <header className="search-results__section-header">
        <div className="search-results__section-summary">
          <div className="search-results__section-identity">
            <h4 className="search-results__section-title">{item.section.key.index}</h4>
            <LiveBadge section={item} />
          </div>
          <div className="search-results__section-body">
            <div className="search-results__identity">
              <MetaItems items={identity} />
            </div>
            <Occurrences occurrences={item.occurrences} />
          </div>
        </div>
        <div className="search-results__section-actions">
          <div className="search-results__badges">
            <OutcomeBadge explanation={item.explanation} />
          </div>
          <SectionSelectionAction sectionKey={item.section.key} />
          <SectionLink
            onSectionNavigate={onSectionNavigate}
            section={item.section.key}
            sectionHref={sectionHref}
          />
        </div>
      </header>
      <EvidenceNote item={item} />
    </article>
  );
}

function VariantResult({
  cardHeadline,
  expandedSectionDisclosures,
  item,
  ordinal,
  onSectionDisclosureChange,
  sectionHref,
  onSectionNavigate,
}: {
  readonly cardHeadline: string;
  readonly expandedSectionDisclosures?: ReadonlySet<string> | undefined;
  readonly item: CourseVariantQueryItemV1;
  readonly ordinal: number;
  readonly onSectionDisclosureChange?: ((disclosureId: string, expanded: boolean) => void) | undefined;
  readonly sectionHref: (key: SectionKey) => string;
  readonly onSectionNavigate?: ((key: SectionKey) => void) | undefined;
}) {
  const i18n = useBcspI18n();
  const label = i18n.t('result.offering_label', { number: i18n.formatNumber(ordinal) });
  // An offering that carries the card's own name says nothing new; it shows its label instead.
  const headline = variantHeadline(item.variant, label);
  const named = headline !== cardHeadline;
  return (
    <article className="search-results__variant" data-variant-ordinal={ordinal}>
      <header className="search-results__variant-summary">
        <div>
          {named ? <span className="search-results__eyebrow">{label}</span> : null}
          <h3 className="search-results__variant-title">{named ? headline : label}</h3>
          <div className="search-results__identity" aria-label={i18n.t('result.variant_fields')}>
            <MetaItems items={variantMetaItems(item.variant, i18n, { credits: true })} />
          </div>
        </div>
        <div className="search-results__badges">
          <OutcomeBadge explanation={item.explanation} />
        </div>
      </header>
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
  const primary = visibleVariants[0];
  if (primary === undefined) return null;
  // Several genuinely distinct offerings are disclosed as such; one is just the course.
  const several = visibleVariants.length > 1 || item.group.variantKeys.length > 1;
  const courseString = item.group.key.courseString;
  const headline = variantHeadline(primary.variant, courseString);
  const metaItems = [
    ...(several
      ? [i18n.t('result.offerings_count', { count: i18n.formatNumber(visibleVariants.length) })]
      : []),
    ...variantMetaItems(primary.variant, i18n, { credits: !several }),
  ];
  return (
    <article className="search-results__group" data-course-group={courseString}>
      <header className="search-results__group-header">
        <div>
          <h2 className="search-results__group-title">{headline}</h2>
          <div className="search-results__identity">
            <data className="search-results__meta" value={courseString}>{courseString}</data>
            <MetaItems items={metaItems} />
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
      {several ? (
        <div className="search-results__variant-list">
          {visibleVariants.map((variant, index) => (
            <VariantResult
              cardHeadline={headline}
              expandedSectionDisclosures={expandedSectionDisclosures}
              item={variant}
              key={variant.variant.key.fingerprint}
              onSectionDisclosureChange={onSectionDisclosureChange}
              onSectionNavigate={onSectionNavigate}
              ordinal={index + 1}
              sectionHref={sectionHref}
            />
          ))}
        </div>
      ) : (
        <SectionDisclosure
          disclosureId={formatVariantDisclosureId(primary.variant)}
          expandedSectionDisclosures={expandedSectionDisclosures}
          onSectionDisclosureChange={onSectionDisclosureChange}
          onSectionNavigate={onSectionNavigate}
          sectionHref={sectionHref}
          sections={primary.sections.filter(({ explanation }) => explanation.outcome !== 'NO_MATCH')}
        />
      )}
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
  const shared = sharedOpenDoubt(courseSections(visibleItems));
  return (
    <SharedOpenDoubtContext.Provider value={shared}>
    <section className="search-results" aria-label={i18n.t('result.course_results_label')}>
      <SearchResultsStyles />
      <ResultsHeader kind={i18n.t('result.course_results')} page={response.page} />
      <SharedOpenDoubtNotice reason={shared} />
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
    </SharedOpenDoubtContext.Provider>
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
  const shared = sharedOpenDoubt(visibleItems.map(({ section }) => section));
  return (
    <SharedOpenDoubtContext.Provider value={shared}>
    <section className="search-results" aria-label={i18n.t('result.section_results_label')}>
      <SearchResultsStyles />
      <ResultsHeader kind={i18n.t('result.section_results')} page={response.page} />
      <SharedOpenDoubtNotice reason={shared} />
      {visibleItems.length === 0 ? (
        <p className="search-results__empty">{i18n.t('result.no_sections')}</p>
      ) : (
        <div className="search-results__list">
          {visibleItems.map((item) => (
            <article className="search-results__standalone-section" key={formatSectionKey(item.section.section.key)}>
              <header className="search-results__group-header">
                <div>
                  <h2 className="search-results__group-title">
                    {variantHeadline(item.variant, item.variant.key.group.courseString)}
                  </h2>
                  <div className="search-results__identity">
                    <data
                      className="search-results__meta"
                      value={item.variant.key.group.courseString}
                    >
                      {item.variant.key.group.courseString}
                    </data>
                    <MetaItems items={variantMetaItems(item.variant, i18n, { credits: true })} />
                  </div>
                </div>
                <button
                  className="search-results__button"
                  onClick={() => onCourseDetail(item.variant.key.group)}
                  type="button"
                >
                  {i18n.t('result.course_detail')}
                </button>
              </header>
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
    </SharedOpenDoubtContext.Provider>
  );
}

function DetailFields({
  headline,
  variant,
}: {
  readonly headline: string;
  readonly variant: NormalizedCourseVariantV1;
}) {
  const i18n = useBcspI18n();
  const title = reported(variant.title);
  const expandedTitle = reported(variant.expandedTitle);
  const fields: readonly (readonly [string, string | null])[] = [
    [i18n.t('result.title'), title === headline ? null : title],
    [i18n.t('result.expanded_title'), expandedTitle === headline ? null : expandedTitle],
    [i18n.t('course.credits'), detailText(variant.credits, i18n, formatCredits)],
    [i18n.t('result.supplement'), reported(variant.supplementCode)],
    [i18n.t('course.description'), detailText(variant.description, i18n)],
    [i18n.t('result.prerequisite'), detailText(variant.prerequisiteNotes, i18n)],
    [i18n.t('result.prerequisite_state'), optionTextForResult(variant.prerequisiteState, i18n)],
    [i18n.t('result.level'), levelText(variant.level, i18n)],
    [
      i18n.t('course.subject'),
      joinParts([reported(variant.subjectCode), reported(variant.subjectDescription)], ' · '),
    ],
    [
      i18n.t('result.offering_unit'),
      joinParts([reported(variant.offeringUnit), reported(variant.offeringUnitTitle)], ' · '),
    ],
    [i18n.t('course.core_codes'), reported(variant.coreCodes, (codes) => codes.join(', '))],
    [i18n.t('result.campus_locations'), reported(variant.campusLocations, (values) => values.join(', '))],
    [i18n.t('result.synopsis_url'), reported(variant.synopsisUrl)],
  ];
  return (
    <dl className="search-results__field-list">
      {fields.map(([label, value]) => value === null ? null : (
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
  const courseString = course.group.key.courseString;
  const primary = course.variants[0];
  const several = course.variants.length > 1;
  const cardHeadline = primary === undefined
    ? courseString
    : variantHeadline(primary.variant, courseString);
  return (
    <section className="search-results" aria-label={i18n.t('result.course_detail')}>
      <SearchResultsStyles />
      <article className="search-results__detail">
        <header className="search-results__detail-header">
          <div>
            <h1 className="search-results__detail-title">{cardHeadline}</h1>
            <span className="search-results__meta">{formatGroupKey(course.group.key)}</span>
          </div>
          <OutcomeBadge explanation={course.explanation} />
        </header>
        {course.variants.map((variant, index) => {
          const label = i18n.t('result.offering_label', { number: i18n.formatNumber(index + 1) });
          const headline = variantHeadline(variant.variant, label);
          const named = several && headline !== cardHeadline;
          return (
            <article className="search-results__variant" key={variant.variant.key.fingerprint}>
              {several ? (
                <header className="search-results__variant-summary">
                  <div>
                    {named ? <span className="search-results__eyebrow">{label}</span> : null}
                    <h2 className="search-results__variant-title">{named ? headline : label}</h2>
                  </div>
                  <OutcomeBadge explanation={variant.explanation} />
                </header>
              ) : null}
              <DetailFields headline={cardHeadline} variant={variant.variant} />
              <SectionDisclosure
                disclosureId={formatVariantDisclosureId(variant.variant)}
                expandedSectionDisclosures={expandedSectionDisclosures}
                onSectionDisclosureChange={onSectionDisclosureChange}
                onSectionNavigate={onSectionNavigate}
                sectionHref={sectionHref}
                sections={variant.sections}
              />
            </article>
          );
        })}
      </article>
    </section>
  );
}

export function SectionDetailView({ response, sectionHref, onSectionNavigate }: SectionDetailViewProps) {
  const i18n = useBcspI18n();
  const { section, variant } = response;
  const headline = variantHeadline(variant, variant.key.group.courseString);
  return (
    <section className="search-results" aria-label={i18n.t('search.section_detail_title')}>
      <SearchResultsStyles />
      <article className="search-results__detail">
        <header className="search-results__detail-header">
          <div>
            <h1 className="search-results__detail-title">{headline}</h1>
            <span className="search-results__meta">
              {variant.key.group.courseString} · {formatSectionKey(section.section.key)}
            </span>
          </div>
          <OutcomeBadge explanation={section.explanation} />
        </header>
        <DetailFields headline={headline} variant={variant} />
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
        <div className="search-results__live" aria-label={i18n.t('result.live_freshness')}>
          <Fact
            label={i18n.t('result.open_state')}
            value={i18n.t(openStateMessageKeys[section.open.state])}
          />
          <Fact
            label={i18n.t('result.observed_at')}
            value={formatDateTime(section.open.observedAt, i18n)}
          />
          <Fact
            label={i18n.t('freshness.fresh')}
            value={section.open.freshUntil === null
              ? i18n.t('result.no_fresh_until')
              : i18n.t('result.fresh_until', { time: formatDateTime(section.open.freshUntil, i18n) })}
          />
        </div>
        <SectionResult
          item={section}
          onSectionNavigate={onSectionNavigate}
          sectionHref={sectionHref}
        />
      </article>
    </section>
  );
}
