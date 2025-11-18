import { useEffect, useState } from 'react';

import { FilterPanel } from './components/FilterPanel';
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
  const emptyMessage = readyForQuery ? '暂无匹配结果，请调整筛选条件。' : '请先选择学期与校区以加载课程列表。';

  const handlePageChange = (page: number) => {
    setFilters((prev) => ({
      ...prev,
      pagination: { ...prev.pagination, page },
    }));
  };

  return (
    <div className="course-app">
      <div className="course-app__container">
        <aside className="course-app__filters">
          {dictionaryState.dictionary ? (
            <FilterPanel
              state={filters}
              dictionary={dictionaryState.dictionary}
              onStateChange={setFilters}
              onReset={setFilters}
              loading={dictionaryState.status !== 'success'}
            />
          ) : (
            <div className="course-app__filters-placeholder">正在加载筛选字典...</div>
          )}
        </aside>
        <main className="course-app__results">
          {dictionaryState.error && (
            <div className="course-app__alert course-app__alert--warning">
              Filters API 不可用，已切换到离线字典。{' '}
              <button type="button" onClick={dictionaryState.refetch}>
                重试
              </button>
            </div>
          )}
          {filters.openStatus === 'hasWaitlist' && (
            <div className="course-app__alert course-app__alert--info">
              等候名单筛选尚未接入 API，当前结果仅供参考。
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
