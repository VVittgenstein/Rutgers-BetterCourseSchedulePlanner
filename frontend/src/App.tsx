import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { FilterPanel } from './components/FilterPanel';
import { LanguageSwitcher } from './components/LanguageSwitcher';
import type { CourseFilterState } from './state/courseFilters';
import { createInitialCourseFilterState } from './state/courseFilters';
import { useCourseQuery } from './hooks/useCourseQuery';
import { useFiltersDictionary } from './hooks/useFiltersDictionary';
import { CourseList } from './components/CourseList';
import './App.css';

export function App() {
  const [filters, setFilters] = useState<CourseFilterState>(() => createInitialCourseFilterState());
  const dictionaryState = useFiltersDictionary();
  const dictionaryReady = Boolean(dictionaryState.dictionary);
  const { t } = useTranslation();

  useEffect(() => {
    if (!dictionaryState.dictionary) return;
    setFilters((prev) => {
      let changed = false;
      const next: CourseFilterState = { ...prev };
      if (!prev.term && dictionaryState.dictionary?.terms[0]) {
        next.term = dictionaryState.dictionary.terms[0].value;
        changed = true;
      }
      if (!prev.campus && dictionaryState.dictionary?.campuses[0]) {
        next.campus = dictionaryState.dictionary.campuses[0].value;
        changed = true;
      }
      return changed ? next : prev;
    });
  }, [dictionaryState.dictionary]);

  const courseQuery = useCourseQuery(filters, { enabled: dictionaryReady });
  const readyForQuery = Boolean(filters.term && (filters.campus || filters.subjects.length));
  const emptyMessage = readyForQuery ? t('app.shell.empty.ready') : t('app.shell.empty.missingFilters');

  const handlePageChange = (page: number) => {
    setFilters((prev) => ({
      ...prev,
      pagination: { ...prev.pagination, page },
    }));
  };

  return (
    <div className="course-app">
      <div className="course-app__toolbar">
        <LanguageSwitcher />
      </div>
      <div className="course-app__container">
        <aside className="course-app__filters">
          {dictionaryState.dictionary ? (
            <FilterPanel
              state={filters}
              dictionary={dictionaryState.dictionary}
              onStateChange={setFilters}
              onReset={setFilters}
              loading={dictionaryState.status === 'loading' && !dictionaryState.dictionary}
            />
          ) : (
            <div className="course-app__filters-placeholder">{t('app.shell.loadingDictionary')}</div>
          )}
        </aside>
        <main className="course-app__results">
          {dictionaryState.error && (
            <div className="course-app__alert course-app__alert--warning">
              {t('app.shell.fallbackAlert')}{' '}
              <button type="button" onClick={dictionaryState.refetch}>
                {t('common.actions.retry')}
              </button>
            </div>
          )}
          {filters.openStatus === 'hasWaitlist' && (
            <div className="course-app__alert course-app__alert--info">
              {t('app.shell.waitlistNotice')}
            </div>
          )}
          <CourseList
            items={courseQuery.items}
            meta={courseQuery.meta}
            isLoading={courseQuery.isLoading || (!dictionaryReady && courseQuery.status === 'idle')}
            isFetching={courseQuery.isFetching}
            error={courseQuery.error}
            onPageChange={handlePageChange}
            onRetry={courseQuery.refetch}
            emptyState={emptyMessage}
          />
        </main>
      </div>
    </div>
  );
}
