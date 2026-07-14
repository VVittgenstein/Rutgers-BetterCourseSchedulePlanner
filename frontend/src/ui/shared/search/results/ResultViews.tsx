import type { MouseEvent } from 'react';

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
import { SectionSelectionAction } from '../../watch';
import { SearchResultsStyles } from './styles';

export interface ResultNavigationProps {
  readonly onCourseDetail: (key: CourseGroupKey) => void;
  readonly sectionHref: (key: SectionKey) => string;
  readonly onSectionNavigate?: ((key: SectionKey) => void) | undefined;
}

export interface CourseResultsViewProps extends ResultNavigationProps {
  readonly response: CourseQueryResponseV1;
  readonly onPageChange: (page: number) => void;
}

export interface SectionResultsViewProps extends ResultNavigationProps {
  readonly response: SectionQueryResponseV1;
  readonly onPageChange: (page: number) => void;
}

export interface CourseDetailViewProps {
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

function formatKnowledge<T>(
  field: CatalogFieldKnowledge<T>,
  format: (value: T) => string = String,
): string {
  if (field.knowledge === 'UNKNOWN') return `Unknown — ${field.reason}`;
  if (field.presence.presence === 'ABSENT') return 'Not reported';
  if (field.presence.presence === 'EXPLICIT_NULL') return 'Explicitly null';
  return format(field.presence.value);
}

function formatArray(field: CatalogFieldKnowledge<readonly string[]>): string {
  return formatKnowledge(field, (value) => value.length === 0 ? 'None reported' : value.join(', '));
}

function formatMinute(value: number): string {
  const hours = Math.floor(value / 60).toString().padStart(2, '0');
  const minutes = (value % 60).toString().padStart(2, '0');
  return `${hours}:${minutes}`;
}

function formatOccurrenceTime(occurrence: NormalizedOccurrenceV1): string {
  if (occurrence.time.knowledge !== 'KNOWN') return `Unknown — ${occurrence.time.knowledge}`;
  return `${formatMinute(occurrence.time.startMinute)}–${formatMinute(occurrence.time.endMinute)}`;
}

function reasonText(explanation: MatchExplanation): string {
  if (explanation.reasons.length === 0) return 'No uncertainty reasons';
  return explanation.reasons.map((reason) => `${reason.field}: ${reason.code}`).join('; ');
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
  return (
    <div className="search-results__variant-facts" aria-label="Variant-defining fields">
      <Fact label="Title" value={formatKnowledge(variant.title)} />
      <Fact label="Credits" value={formatKnowledge(variant.credits)} />
      <Fact label="Supplement" value={formatKnowledge(variant.supplementCode)} />
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
      Open section
    </a>
  );
}

function OutcomeBadge({ explanation }: { readonly explanation: MatchExplanation }) {
  const modifier = explanation.outcome === 'MATCH' ? 'match' : 'uncertain';
  return (
    <span className={`search-results__badge search-results__badge--${modifier}`}>
      {explanation.outcome}
    </span>
  );
}

function LiveBadge({ section }: { readonly section: SectionQueryItemV1 }) {
  return (
    <span className={`search-results__badge search-results__badge--${section.open.state.toLowerCase()}`}>
      Live {section.open.state}
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
  const fallbackReasons = explanation.reasons;
  return (
    <section
      className={`search-results__witness${explanation.outcome === 'UNCERTAIN' ? ' search-results__witness--uncertain' : ''}`}
      aria-label="Same-section filter witness"
    >
      <strong className="search-results__witness-title">Same-section witness</strong>
      <ul className="search-results__witness-list">
        {matches.map((match, index) => (
          <li className="search-results__witness-item" key={`${match.fieldId}-${index}`}>
            <span className="search-results__label">{match.fieldId} · {match.explanation.outcome}</span>
            <span>{reasonText(match.explanation)}</span>
          </li>
        ))}
        {matches.length === 0 && fallbackReasons.map((reason, index) => (
          <li className="search-results__witness-item" key={`${reason.field}-${reason.code}-${index}`}>
            <span className="search-results__label">Result reason</span>
            <span>{reason.field}: {reason.code}</span>
          </li>
        ))}
        {matches.length === 0 && fallbackReasons.length === 0 ? (
          <li className="search-results__witness-item">
            <span className="search-results__label">Result witness</span>
            <span>Match confirmed; no section-scoped filter reason was required.</span>
          </li>
        ) : null}
      </ul>
    </section>
  );
}

function Occurrences({ occurrences }: { readonly occurrences: readonly NormalizedOccurrenceV1[] }) {
  return (
    <section className="search-results__occurrences" aria-label="Meeting occurrences">
      <strong className="search-results__occurrence-title">Occurrences · {occurrences.length}</strong>
      {occurrences.length === 0 ? (
        <p className="search-results__value">No meeting occurrences reported.</p>
      ) : (
        <ol className="search-results__occurrence-list">
          {occurrences.map((occurrence) => (
            <li
              className="search-results__occurrence"
              key={`${occurrence.key.section.index}-${occurrence.key.ordinal}`}
            >
              <span className="search-results__label">Occurrence {occurrence.key.ordinal}</span>
              <span>
                {formatArray(occurrence.days)} · {formatOccurrenceTime(occurrence)} ·{' '}
                {formatKnowledge(occurrence.building)} {formatKnowledge(occurrence.room)}
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
  return (
    <article
      className="search-results__section"
      data-match-outcome={item.explanation.outcome}
      data-section-index={item.section.key.index}
    >
      <header className="search-results__section-header">
        <div>
          <span className="search-results__eyebrow">Section identity</span>
          <h4 className="search-results__section-title">
            {formatKnowledge(item.section.sectionNumber)} · {item.section.key.index}
          </h4>
          <div className="search-results__identity">
            <data className="search-results__meta" value={item.section.key.term}>{item.section.key.term}</data>
            <data className="search-results__meta" value={item.section.key.campus}>{item.section.key.campus}</data>
            <span className="search-results__meta">{item.section.deliveryModality}</span>
            <span className="search-results__meta">{item.section.synchronicity}</span>
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
      <div className="search-results__live" aria-label="Live open freshness">
        <Fact label="Open state" value={item.open.state} />
        <Fact label="Observed at" value={item.open.observedAt ?? 'Never observed'} />
        <Fact
          label="Freshness"
          value={item.open.freshUntil === null ? 'No fresh-until time' : `Fresh until ${item.open.freshUntil}`}
        />
      </div>
      {item.open.uncertainty === null ? null : (
        <p className="search-results__meta">Open uncertainty · {item.open.uncertainty}</p>
      )}
      <FilterWitness explanation={item.explanation} matches={item.filterMatches} />
      <Occurrences occurrences={item.occurrences} />
    </article>
  );
}

function VariantResult({
  item,
  sectionHref,
  onSectionNavigate,
}: {
  readonly item: CourseVariantQueryItemV1;
  readonly sectionHref: (key: SectionKey) => string;
  readonly onSectionNavigate?: ((key: SectionKey) => void) | undefined;
}) {
  return (
    <details className="search-results__variant" data-variant-fingerprint={item.variant.key.fingerprint} open>
      <summary className="search-results__variant-summary">
        <div>
          <span className="search-results__eyebrow">Variant · {item.variant.key.fingerprint}</span>
          <h3 className="search-results__variant-title">{formatKnowledge(item.variant.title)}</h3>
        </div>
        <div className="search-results__badges">
          <OutcomeBadge explanation={item.explanation} />
          <span className="search-results__badge">{item.sections.length} sections</span>
        </div>
      </summary>
      <VariantFacts variant={item.variant} />
      <div className="search-results__section-list">
        {item.sections.map((section) => (
          <SectionResult
            item={section}
            key={formatSectionKey(section.section.key)}
            onSectionNavigate={onSectionNavigate}
            sectionHref={sectionHref}
          />
        ))}
      </div>
    </details>
  );
}

function CourseGroupResult({
  item,
  onCourseDetail,
  sectionHref,
  onSectionNavigate,
}: {
  readonly item: CourseQueryItemV1;
  readonly onCourseDetail: (key: CourseGroupKey) => void;
  readonly sectionHref: (key: SectionKey) => string;
  readonly onSectionNavigate?: ((key: SectionKey) => void) | undefined;
}) {
  return (
    <article className="search-results__group" data-course-group={item.group.key.courseString}>
      <header className="search-results__group-header">
        <div>
          <span className="search-results__eyebrow">Course group</span>
          <h2 className="search-results__group-title">{item.group.key.courseString}</h2>
          <div className="search-results__identity">
            <span className="search-results__meta">Term {item.group.key.term}</span>
            <span className="search-results__meta">Campus {item.group.key.campus}</span>
            <span className="search-results__meta">{item.variants.length} explicit variants</span>
          </div>
        </div>
        <button
          className="search-results__button"
          onClick={() => onCourseDetail(item.group.key)}
          type="button"
        >
          Course detail
        </button>
      </header>
      <div className="search-results__variant-list">
        {item.variants.map((variant) => (
          <VariantResult
            item={variant}
            key={variant.variant.key.fingerprint}
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
  const hasPrevious = page.page > 1;
  const hasNext = page.page < page.totalPages;
  return (
    <nav className="search-results__pagination" aria-label="Search result pages">
      <span className="search-results__page-label">
        Page {page.page} / {page.totalPages || 1} · {page.total} results
      </span>
      <div className="search-results__pagination-actions">
        <button
          className="search-results__button"
          disabled={!hasPrevious}
          onClick={() => onPageChange(page.page - 1)}
          type="button"
        >
          Previous
        </button>
        <button
          className="search-results__button"
          disabled={!hasNext}
          onClick={() => onPageChange(page.page + 1)}
          type="button"
        >
          Next
        </button>
      </div>
    </nav>
  );
}

function ResultsHeader({ kind, page }: { readonly kind: string; readonly page: PageInfoV1 }) {
  return (
    <header className="search-results__header">
      <div>
        <span className="search-results__eyebrow">Search output</span>
        <h1 className="search-results__heading">{kind}</h1>
      </div>
      <data className="search-results__count" value={page.total}>{page.total}</data>
    </header>
  );
}

export function CourseResultsView({
  response,
  onCourseDetail,
  sectionHref,
  onSectionNavigate,
  onPageChange,
}: CourseResultsViewProps) {
  return (
    <section className="search-results" aria-label="Course search results">
      <SearchResultsStyles />
      <ResultsHeader kind="Course results" page={response.page} />
      {response.items.length === 0 ? (
        <p className="search-results__empty">No courses match the active filter set.</p>
      ) : (
        <div className="search-results__list">
          {response.items.map((item) => (
            <CourseGroupResult
              item={item}
              key={formatGroupKey(item.group.key)}
              onCourseDetail={onCourseDetail}
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
  return (
    <section className="search-results" aria-label="Section search results">
      <SearchResultsStyles />
      <ResultsHeader kind="Section results" page={response.page} />
      {response.items.length === 0 ? (
        <p className="search-results__empty">No sections match the active filter set.</p>
      ) : (
        <div className="search-results__list">
          {response.items.map((item) => (
            <article className="search-results__standalone-section" key={formatSectionKey(item.section.section.key)}>
              <header className="search-results__group-header">
                <div>
                  <span className="search-results__eyebrow">Course variant</span>
                  <h2 className="search-results__group-title">
                    {item.variant.key.group.courseString} · {formatKnowledge(item.variant.title)}
                  </h2>
                  <span className="search-results__meta">Variant {item.variant.key.fingerprint}</span>
                </div>
                <button
                  className="search-results__button"
                  onClick={() => onCourseDetail(item.variant.key.group)}
                  type="button"
                >
                  Course detail
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
  const fields = [
    ['Expanded title', formatKnowledge(variant.expandedTitle)],
    ['Credits', formatKnowledge(variant.credits)],
    ['Supplement', formatKnowledge(variant.supplementCode)],
    ['Description', formatKnowledge(variant.description)],
    ['Prerequisite', formatKnowledge(variant.prerequisiteNotes)],
    ['Prerequisite state', variant.prerequisiteState],
    ['Level', formatKnowledge(variant.level)],
    ['Subject', `${formatKnowledge(variant.subjectCode)} · ${formatKnowledge(variant.subjectDescription)}`],
    ['Offering unit', `${formatKnowledge(variant.offeringUnit)} · ${formatKnowledge(variant.offeringUnitTitle)}`],
    ['Core codes', formatArray(variant.coreCodes)],
    ['Campus locations', formatArray(variant.campusLocations)],
    ['Synopsis URL', formatKnowledge(variant.synopsisUrl)],
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

export function CourseDetailView({ response, sectionHref, onSectionNavigate }: CourseDetailViewProps) {
  const course = response.course;
  return (
    <section className="search-results" aria-label="Course detail">
      <SearchResultsStyles />
      <article className="search-results__detail">
        <header className="search-results__detail-header">
          <div>
            <span className="search-results__eyebrow">Course detail</span>
            <h1 className="search-results__detail-title">{course.group.key.courseString}</h1>
            <span className="search-results__meta">{formatGroupKey(course.group.key)}</span>
          </div>
          <OutcomeBadge explanation={course.explanation} />
        </header>
        {course.variants.map((variant) => (
          <details className="search-results__variant" key={variant.variant.key.fingerprint} open>
            <summary className="search-results__variant-summary">
              <div>
                <span className="search-results__eyebrow">Variant {variant.variant.key.fingerprint}</span>
                <h2 className="search-results__variant-title">{formatKnowledge(variant.variant.title)}</h2>
              </div>
              <OutcomeBadge explanation={variant.explanation} />
            </summary>
            <DetailFields variant={variant.variant} />
            {variant.sections.map((section) => (
              <SectionResult
                item={section}
                key={formatSectionKey(section.section.key)}
                onSectionNavigate={onSectionNavigate}
                sectionHref={sectionHref}
              />
            ))}
          </details>
        ))}
      </article>
    </section>
  );
}

export function SectionDetailView({ response, sectionHref, onSectionNavigate }: SectionDetailViewProps) {
  const { section, variant } = response;
  return (
    <section className="search-results" aria-label="Section detail">
      <SearchResultsStyles />
      <article className="search-results__detail">
        <header className="search-results__detail-header">
          <div>
            <span className="search-results__eyebrow">Section detail</span>
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
            <dt className="search-results__label">Section subtitle</dt>
            <dd className="search-results__value">{formatKnowledge(section.section.subtitle)}</dd>
          </div>
          <div className="search-results__field">
            <dt className="search-results__label">Instructors</dt>
            <dd className="search-results__value">{formatArray(section.section.instructors)}</dd>
          </div>
          <div className="search-results__field">
            <dt className="search-results__label">Exam code</dt>
            <dd className="search-results__value">
              {formatKnowledge(section.section.examCode)} · {formatKnowledge(section.section.examCodeText)}
            </dd>
          </div>
          <div className="search-results__field">
            <dt className="search-results__label">Permission to add</dt>
            <dd className="search-results__value">
              {formatKnowledge(section.section.specialPermissionAddDescription)}
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
