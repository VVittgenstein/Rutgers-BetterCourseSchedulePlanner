import { useMemo } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import { FixedSizeList as VirtualList } from 'react-window';

import type { CourseQueryMeta, CourseResultItem } from '../hooks/useCourseQuery';
import type { ApiError } from '../api/client';
import './CourseList.css';

const ROW_HEIGHT = 168;
const VIRTUAL_THRESHOLD = 16;
const MIN_ROWS = 4;
const MAX_ROWS = 8;

export interface CourseListProps {
  items: CourseResultItem[];
  meta: CourseQueryMeta | null;
  isLoading: boolean;
  isFetching: boolean;
  error?: ApiError | null;
  onPageChange?: (page: number) => void;
  onRetry?: () => void;
  emptyState?: string;
}

export function CourseList({
  items,
  meta,
  isLoading,
  isFetching,
  error,
  onPageChange,
  onRetry,
  emptyState,
}: CourseListProps) {
  const { t, i18n } = useTranslation();
  const numberFormatter = useMemo(() => {
    const locale = i18n.language?.startsWith('zh') ? 'zh-CN' : 'en-US';
    return new Intl.NumberFormat(locale);
  }, [i18n.language]);
  const formatNumber = (value: number) => numberFormatter.format(value);
  const total = meta?.total ?? 0;
  const page = meta?.page ?? 1;
  const pageSize = meta?.pageSize ?? (items.length || 25);
  const rangeStart = total > 0 ? (page - 1) * pageSize + 1 : 0;
  const rangeEnd = total > 0 ? Math.min(rangeStart + items.length - 1, total) : 0;
  const totalPages = Math.max(1, Math.ceil((total || pageSize) / pageSize));
  const resolvedEmptyState = emptyState ?? t('courseList.empty.default');

  const renderBody = () => {
    if (isLoading) {
      return <SkeletonList />;
    }
    if (error) {
      return <ErrorState error={error} onRetry={onRetry} t={t} />;
    }
    if (!items.length) {
      return <EmptyState message={resolvedEmptyState} />;
    }
    return renderVirtualizedList(items, t);
  };

  const handlePrev = () => {
    if (page <= 1 || !onPageChange) return;
    onPageChange(page - 1);
  };
  const handleNext = () => {
    if (!meta?.hasNext || !onPageChange) return;
    onPageChange(page + 1);
  };

  return (
    <section className="course-list">
      <header className="course-list__header">
        <div>
          <p className="course-list__eyebrow">{t('courseList.header.eyebrow')}</p>
          <h2>
            {isLoading
              ? t('courseList.header.loading')
              : t('courseList.header.count', { countLabel: formatNumber(total) })}
          </h2>
          {total > 0 && (
            <p className="course-list__summary">
              {t('courseList.header.range', {
                start: formatNumber(rangeStart),
                end: formatNumber(rangeEnd),
                page: formatNumber(page),
              })}
            </p>
          )}
        </div>
        {isFetching && !isLoading && (
          <span className="course-list__badge">{t('courseList.header.refreshing')}</span>
        )}
      </header>

      {renderBody()}

      <footer className="course-list__footer">
        <div className="course-list__footer-meta">
          {total > 0 ? (
            <span>
              {t('courseList.footer.pagination', {
                pages: formatNumber(totalPages),
                pageSize: formatNumber(pageSize),
              })}
            </span>
          ) : (
            <span>{t('courseList.footer.none')}</span>
          )}
        </div>
        <div className="course-list__pagination">
          <button type="button" onClick={handlePrev} disabled={page <= 1 || isLoading}>
            {t('courseList.pagination.prev')}
          </button>
          <span>
            {t('courseList.footer.pageLabel', {
              page: formatNumber(page),
              pages: formatNumber(totalPages),
            })}
          </span>
          <button type="button" onClick={handleNext} disabled={!meta?.hasNext || isLoading}>
            {t('courseList.pagination.next')}
          </button>
        </div>
      </footer>
    </section>
  );
}

function renderVirtualizedList(items: CourseResultItem[], t: TFunction) {
  if (items.length <= VIRTUAL_THRESHOLD) {
    return (
      <div className="course-list__rows">
        {items.map((item) => (
          <CourseRow key={item.id} course={item} t={t} />
        ))}
      </div>
    );
  }

  const rowsToShow = Math.min(MAX_ROWS, Math.max(MIN_ROWS, Math.min(items.length, MAX_ROWS)));
  const height = rowsToShow * ROW_HEIGHT;

  return (
    <VirtualList
      height={height}
      width="100%"
      itemCount={items.length}
      itemSize={ROW_HEIGHT}
      className="course-list__virtual"
    >
      {({ index, style }) => {
        const course = items[index];
        return (
          <div style={style}>
            <CourseRow course={course} t={t} />
          </div>
        );
      }}
    </VirtualList>
  );
}

function CourseRow({ course, t }: { course: CourseResultItem; t: TFunction }) {
  const deliveryLabels = course.sections.deliveryMethods.map((method) =>
    t(`courseCard.tags.delivery.${method}`, { defaultValue: method.toUpperCase() }),
  );
  const updatedLabel = formatRelativeTime(course.updatedAt, t);

  return (
    <article className="course-card">
      <header className="course-card__header">
        <div>
          <p className="course-card__eyebrow">
            {course.code} · {course.campusCode}
          </p>
          <h3>{course.title}</h3>
          {course.subtitle && <p className="course-card__subtitle">{course.subtitle}</p>}
        </div>
        <div className="course-card__badges">
          {course.hasOpenSections && (
            <span className="course-card__badge course-card__badge--success">{t('courseCard.badges.open')}</span>
          )}
          <span className="course-card__badge">{course.level ?? t('courseCard.badges.levelFallback')}</span>
        </div>
      </header>

      <div className="course-card__meta">
        <MetaItem label={t('courseCard.meta.credits')} value={formatCredits(course.credits, t)} />
        <MetaItem label={t('courseCard.meta.sections')} value={`${course.sections.open}/${course.sections.total}`} />
        <MetaItem label={t('courseCard.meta.subject')} value={course.subjectDescription ?? course.subjectCode} />
      </div>

      <div className="course-card__tags">
        {deliveryLabels.length ? (
          deliveryLabels.map((label, index) => (
            <span key={`${label}-${index}`} className="course-card__tag">
              {label}
            </span>
          ))
        ) : (
          <span className="course-card__tag course-card__tag--muted">{t('courseCard.tags.deliveryFallback')}</span>
        )}
      </div>

      {course.prerequisites && (
        <p className="course-card__prereq">
          <strong>{t('courseCard.details.prerequisites')}</strong> {course.prerequisites}
        </p>
      )}

      <footer className="course-card__footer">
        <span>
          {updatedLabel
            ? t('courseCard.details.updated', { time: updatedLabel })
            : t('courseCard.details.updatedRecent')}
        </span>
        <span>{t('courseCard.details.term', { term: course.termId })}</span>
      </footer>
    </article>
  );
}

function MetaItem({ label, value }: { label: string; value: string }) {
  return (
    <div className="course-card__meta-item">
      <p>{label}</p>
      <strong>{value}</strong>
    </div>
  );
}

function formatCredits(credits: CourseResultItem['credits'], t: TFunction) {
  if (credits.display) return credits.display;
  if (typeof credits.min === 'number' && typeof credits.max === 'number') {
    if (credits.min === credits.max) return `${credits.min}`;
    return `${credits.min}–${credits.max}`;
  }
  if (typeof credits.min === 'number') return `${credits.min}`;
  if (typeof credits.max === 'number') return `${credits.max}`;
  return t('courseCard.badges.levelFallback');
}

function formatRelativeTime(timestamp: string | null | undefined, t: TFunction): string | null {
  if (!timestamp) return null;
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return null;
  const diffMs = Date.now() - date.getTime();
  if (diffMs < 60_000) return t('common.time.relative.justNow');
  if (diffMs < 3_600_000) {
    const minutes = Math.max(1, Math.round(diffMs / 60_000));
    return t('common.time.relative.minutes', { count: minutes });
  }
  if (diffMs < 86_400_000) {
    const hours = Math.max(1, Math.round(diffMs / 3_600_000));
    return t('common.time.relative.hours', { count: hours });
  }
  const days = Math.max(1, Math.round(diffMs / 86_400_000));
  return t('common.time.relative.days', { count: days });
}

function SkeletonList() {
  return (
    <div className="course-list__rows">
      {Array.from({ length: 5 }).map((_, index) => (
        <div key={index} className="course-card course-card--skeleton">
          <div className="course-card__skeleton-line" />
          <div className="course-card__skeleton-line course-card__skeleton-line--short" />
          <div className="course-card__skeleton-line" />
        </div>
      ))}
    </div>
  );
}

function EmptyState({ message }: { message: string }) {
  return (
    <div className="course-list__empty">
      <p>{message}</p>
    </div>
  );
}

function ErrorState({ error, onRetry, t }: { error: ApiError; onRetry?: () => void; t: TFunction }) {
  return (
    <div className="course-list__empty course-list__empty--error">
      <p>{error.message}</p>
      {error.traceId && <p className="course-list__hint">{t('courseList.error.traceId', { id: error.traceId })}</p>}
      {onRetry && (
        <button type="button" onClick={onRetry}>
          {t('courseList.error.retry')}
        </button>
      )}
    </div>
  );
}
