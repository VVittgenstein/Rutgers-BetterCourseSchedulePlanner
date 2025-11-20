import { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { FilterPanel } from './components/FilterPanel';
import { LanguageSwitcher } from './components/LanguageSwitcher';
import { SchedulePreview } from './components/SchedulePreview';
import type { ScheduleSection } from './components/SchedulePreview';
import type { CourseFilterState } from './state/courseFilters';
import { createInitialCourseFilterState } from './state/courseFilters';
import { useCourseQuery } from './hooks/useCourseQuery';
import { useFiltersDictionary } from './hooks/useFiltersDictionary';
import { CourseList } from './components/CourseList';
import { classNames } from './utils/classNames';
import './App.css';

export function App() {
  const [filters, setFilters] = useState<CourseFilterState>(() => createInitialCourseFilterState());
  const [viewMode, setViewMode] = useState<'list' | 'calendar'>('list');
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

  const scheduleSections = useMemo<ScheduleSection[]>(() => {
    if (!courseQuery.items.length) return [];
    const palette = ['#7c3aed', '#0ea5e9', '#10b981', '#f97316', '#f43f5e', '#2563eb'];
    const sections: ScheduleSection[] = [];
    courseQuery.items.forEach((course, courseIndex) => {
      course.sectionPreviews.forEach((section, sectionIndex) => {
        if (!section.meetings.length) return;
        sections.push({
          id: `${course.id}-${section.id}`,
          title: course.title,
          courseCode: course.code,
          sectionCode: section.sectionNumber ?? section.index,
          instructor: section.instructors[0],
          location:
            section.meetings[0]?.building || section.meetings[0]?.room
              ? `${section.meetings[0]?.building ?? ''} ${section.meetings[0]?.room ?? ''}`.trim()
              : section.meetingCampus ?? '',
          color: palette[(courseIndex + sectionIndex) % palette.length],
          meetings: section.meetings.map((meeting) => ({
            day: meeting.day,
            startMinutes: meeting.startMinutes,
            endMinutes: meeting.endMinutes,
          })),
        });
      });
    });
    return sections;
  }, [courseQuery.items]);

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
          <div className="course-app__view-toggle">
            <button
              type="button"
              className={classNames('course-app__view-btn', viewMode === 'list' && 'course-app__view-btn--active')}
              onClick={() => setViewMode('list')}
            >
              {t('courseList.view.list')}
            </button>
            <button
              type="button"
              className={classNames('course-app__view-btn', viewMode === 'calendar' && 'course-app__view-btn--active')}
              onClick={() => setViewMode('calendar')}
            >
              {t('courseList.view.calendar')}
            </button>
          </div>

          {viewMode === 'list' ? (
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
          ) : (
            <SchedulePreview sections={scheduleSections} />
          )}
        </main>
      </div>
    </div>
  );
}
