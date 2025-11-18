import { FixedSizeList as VirtualList } from 'react-window';

import type { CourseQueryMeta, CourseResultItem } from '../hooks/useCourseQuery';
import type { ApiError } from '../api/client';
import './CourseList.css';

const ROW_HEIGHT = 168;
const VIRTUAL_THRESHOLD = 16;
const MIN_ROWS = 4;
const MAX_ROWS = 8;

const DELIVERY_LABELS: Record<string, string> = {
  in_person: 'In Person',
  online: 'Online',
  hybrid: 'Hybrid',
};

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
  emptyState = '没有符合条件的课程，请尝试修改筛选条件。',
}: CourseListProps) {
  const total = meta?.total ?? 0;
  const page = meta?.page ?? 1;
  const pageSize = meta?.pageSize ?? (items.length || 25);
  const rangeStart = total > 0 ? (page - 1) * pageSize + 1 : 0;
  const rangeEnd = total > 0 ? Math.min(rangeStart + items.length - 1, total) : 0;

  const renderBody = () => {
    if (isLoading) {
      return <SkeletonList />;
    }
    if (error) {
      return <ErrorState error={error} onRetry={onRetry} />;
    }
    if (!items.length) {
      return <EmptyState message={emptyState} />;
    }
    return renderVirtualizedList(items);
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
          <p className="course-list__eyebrow">Results</p>
          <h2>{isLoading ? '加载课程中…' : `${total.toLocaleString('en-US')} courses`}</h2>
          {total > 0 && (
            <p className="course-list__summary">
              Showing {rangeStart.toLocaleString()} - {rangeEnd.toLocaleString()} · Page {page}
            </p>
          )}
        </div>
        {isFetching && !isLoading && <span className="course-list__badge">Refreshing…</span>}
      </header>

      {renderBody()}

      <footer className="course-list__footer">
        <div className="course-list__footer-meta">
          {total > 0 ? (
            <span>
              {Math.ceil(total / pageSize)} pages · {pageSize} per page
            </span>
          ) : (
            <span>No pagination available</span>
          )}
        </div>
        <div className="course-list__pagination">
          <button type="button" onClick={handlePrev} disabled={page <= 1 || isLoading}>
            Previous
          </button>
          <span>
            Page {page} / {Math.max(1, Math.ceil(total / pageSize))}
          </span>
          <button type="button" onClick={handleNext} disabled={!meta?.hasNext || isLoading}>
            Next
          </button>
        </div>
      </footer>
    </section>
  );
}

function renderVirtualizedList(items: CourseResultItem[]) {
  if (items.length <= VIRTUAL_THRESHOLD) {
    return (
      <div className="course-list__rows">
        {items.map((item) => (
          <CourseRow key={item.id} course={item} />
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
            <CourseRow course={course} />
          </div>
        );
      }}
    </VirtualList>
  );
}

function CourseRow({ course }: { course: CourseResultItem }) {
  const deliveryLabels = course.sections.deliveryMethods.map((method) => DELIVERY_LABELS[method] ?? method.toUpperCase());
  const updatedLabel = formatRelativeTime(course.updatedAt);

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
          {course.hasOpenSections && <span className="course-card__badge course-card__badge--success">Open</span>}
          <span className="course-card__badge">{course.level ?? 'N/A'}</span>
        </div>
      </header>

      <div className="course-card__meta">
        <MetaItem label="Credits" value={formatCredits(course.credits)} />
        <MetaItem label="Sections" value={`${course.sections.open}/${course.sections.total}`} />
        <MetaItem label="Subject" value={course.subjectDescription ?? course.subjectCode} />
      </div>

      <div className="course-card__tags">
        {deliveryLabels.length ? (
          deliveryLabels.map((label, index) => (
            <span key={`${label}-${index}`} className="course-card__tag">
              {label}
            </span>
          ))
        ) : (
          <span className="course-card__tag course-card__tag--muted">Delivery TBD</span>
        )}
      </div>

      {course.prerequisites && (
        <p className="course-card__prereq">
          <strong>Prerequisites:</strong> {course.prerequisites}
        </p>
      )}

      <footer className="course-card__footer">
        <span>{updatedLabel ? `Updated ${updatedLabel}` : 'Updated recently'}</span>
        <span>Term {course.termId}</span>
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

function formatCredits(credits: CourseResultItem['credits']) {
  if (credits.display) return credits.display;
  if (typeof credits.min === 'number' && typeof credits.max === 'number') {
    if (credits.min === credits.max) return `${credits.min}`;
    return `${credits.min}–${credits.max}`;
  }
  if (typeof credits.min === 'number') return `${credits.min}`;
  if (typeof credits.max === 'number') return `${credits.max}`;
  return 'N/A';
}

function formatRelativeTime(timestamp?: string | null): string | null {
  if (!timestamp) return null;
  const date = new Date(timestamp);
  if (Number.isNaN(date.getTime())) return null;
  const diffMs = Date.now() - date.getTime();
  if (diffMs < 60_000) return 'just now';
  if (diffMs < 3_600_000) {
    const minutes = Math.max(1, Math.round(diffMs / 60_000));
    return `${minutes}m ago`;
  }
  if (diffMs < 86_400_000) {
    const hours = Math.max(1, Math.round(diffMs / 3_600_000));
    return `${hours}h ago`;
  }
  const days = Math.max(1, Math.round(diffMs / 86_400_000));
  return `${days}d ago`;
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

function ErrorState({ error, onRetry }: { error: ApiError; onRetry?: () => void }) {
  return (
    <div className="course-list__empty course-list__empty--error">
      <p>{error.message}</p>
      {error.traceId && <p className="course-list__hint">Trace ID: {error.traceId}</p>}
      {onRetry && (
        <button type="button" onClick={onRetry}>
          Retry
        </button>
      )}
    </div>
  );
}
