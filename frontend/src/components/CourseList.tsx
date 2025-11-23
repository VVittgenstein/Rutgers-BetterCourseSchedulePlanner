import { useMemo } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';

import type { CourseQueryMeta, CourseResultItem } from '../hooks/useCourseQuery';
import type { ApiError } from '../api/client';
import { classNames } from '../utils/classNames';
import './CourseList.css';

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
  const total = meta?.total ?? items.length;
  const page = meta?.page ?? 1;
  const pageSize = meta?.pageSize ?? (items.length || 25);
  const totalPages = Math.max(1, Math.ceil((total || pageSize) / pageSize));
  const displayCount = items.length;
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
    return (
      <div className="course-list__rows">
        {items.map((item) => (
          <CourseRow key={item.id} course={item} t={t} />
        ))}
      </div>
    );
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
              : t('courseList.header.count', { countLabel: formatNumber(displayCount) })}
          </h2>
          <p className="course-list__summary">
            {t('courseList.header.simpleRange', {
              page: formatNumber(page),
              pages: formatNumber(totalPages),
              total: formatNumber(total),
            })}
          </p>
        </div>
        {isFetching && !isLoading && (
          <span className="course-list__badge">{t('courseList.header.refreshing')}</span>
        )}
      </header>

      {renderBody()}

      <footer className="course-list__footer">
        <div className="course-list__footer-meta">
          <span>
            {t('courseList.footer.pagination', {
              pages: formatNumber(totalPages),
              pageSize: formatNumber(pageSize),
            })}
          </span>
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

function CourseRow({ course, t }: { course: CourseResultItem; t: TFunction }) {
  const hasOpen = course.sectionPreviews.some((section) => section.isOpen);
  return (
    <article className="course-card">
      <header className="course-card__header">
        <div>
          <p className="course-card__eyebrow">
            {course.code} · {course.campusCode}
          </p>
          <h3>{course.title}</h3>
        </div>
        <span
          className={classNames(
            'course-card__status',
            hasOpen ? 'course-card__status--open' : 'course-card__status--closed',
          )}
        >
          {hasOpen ? t('courseCard.sections.anyOpen') : t('courseCard.sections.allClosed')}
        </span>
      </header>

      <SectionList sections={course.sectionPreviews} t={t} />
    </article>
  );
}

function SectionList({ sections, t }: { sections: CourseResultItem['sectionPreviews']; t: TFunction }) {
  if (!sections.length) {
    return (
      <div className="course-card__sections course-card__sections--empty">
        <p>{t('courseCard.sections.empty')}</p>
      </div>
    );
  }

  return (
    <div className="course-card__sections">
      <div className="course-card__sections-head">
        <span>{t('courseCard.sections.index')}</span>
        <span>{t('courseCard.sections.status')}</span>
      </div>
      <div className="course-card__sections-body">
        {sections.map((section) => (
          <div key={section.id} className="course-card__section-row">
            <span className="course-card__section-index">{section.index}</span>
            <span
              className={classNames(
                'course-card__section-status',
                section.isOpen ? 'course-card__section-status--open' : 'course-card__section-status--closed',
              )}
            >
              {section.isOpen ? t('courseCard.sections.statusOpen') : t('courseCard.sections.statusClosed')}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
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
