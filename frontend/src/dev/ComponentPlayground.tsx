import { useMemo, useState } from 'react';
import { FilterPanel } from '../components/FilterPanel';
import { SchedulePreview } from '../components/SchedulePreview';
import type { CourseFilterState } from '../state/courseFilters';
import { createInitialCourseFilterState } from '../state/courseFilters';
import { mockDictionary, mockSections } from './mockData';

export function ComponentPlayground() {
  const [filters, setFilters] = useState<CourseFilterState>(() => {
    const base = createInitialCourseFilterState();
    base.term = mockDictionary.terms[0]?.value;
    base.campus = mockDictionary.campuses[0]?.value;
    return base;
  });

  const derivedSections = useMemo(() => {
    return mockSections.filter((section) => {
      if (filters.subjects.length) {
        const matchesSubject = filters.subjects.some((subject) => section.courseCode.startsWith(subject));
        if (!matchesSubject) return false;
      }

      if (filters.meeting.days.length) {
        const hasDay = section.meetings.some((meeting) => filters.meeting.days.includes(meeting.day));
        if (!hasDay) return false;
      }

      if (filters.meeting.startMinutes !== undefined || filters.meeting.endMinutes !== undefined) {
        const matchesWindow = section.meetings.some((meeting) => {
          if (filters.meeting.startMinutes !== undefined && meeting.startMinutes < filters.meeting.startMinutes) {
            return false;
          }
          if (filters.meeting.endMinutes !== undefined && meeting.endMinutes > filters.meeting.endMinutes) {
            return false;
          }
          return true;
        });
        if (!matchesWindow) return false;
      }

      return true;
    });
  }, [filters]);

  return (
    <div className="playground-layout">
      <FilterPanel state={filters} dictionary={mockDictionary} onStateChange={setFilters} onReset={setFilters} />
      <SchedulePreview sections={derivedSections.length ? derivedSections : mockSections} />
    </div>
  );
}
