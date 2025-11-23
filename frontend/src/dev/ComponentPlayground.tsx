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

      const requireDays = filters.meeting.days.length > 0;
      const requireStart = filters.meeting.startMinutes !== undefined;
      const requireEnd = filters.meeting.endMinutes !== undefined;

      if ((requireDays || requireStart || requireEnd) && !section.meetings.length) {
        return false;
      }

      if (requireDays) {
        const allowedDays = new Set(filters.meeting.days);
        const hasOutlierDay = section.meetings.some((meeting) => !allowedDays.has(meeting.day));
        if (hasOutlierDay) return false;
      }

      if (requireStart || requireEnd) {
        const matchesWindow = section.meetings.every((meeting) => {
          if (requireStart && meeting.startMinutes < (filters.meeting.startMinutes ?? 0)) {
            return false;
          }
          if (requireEnd && meeting.endMinutes > (filters.meeting.endMinutes ?? Infinity)) {
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
