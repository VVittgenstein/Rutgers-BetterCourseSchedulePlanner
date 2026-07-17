import { useMemo } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';

import type { MeetingDay } from '../state/courseFilters';
import './SchedulePreview.css';

export interface ScheduleMeeting {
  day: MeetingDay;
  startMinutes: number;
  endMinutes: number;
}

export interface ScheduleSection {
  id: string;
  title: string;
  courseCode: string;
  sectionCode: string;
  instructor?: string;
  location?: string;
  color?: string;
  meetings: ScheduleMeeting[];
}

export interface SchedulePreviewProps {
  sections: ScheduleSection[];
  startHour?: number;
  endHour?: number;
  showLegend?: boolean;
}

const DAY_ORDER: MeetingDay[] = ['M', 'T', 'W', 'TH', 'F', 'SA', 'SU'];
const DAY_KEYS = {
  M: 'common.days.short.mon',
  T: 'common.days.short.tue',
  W: 'common.days.short.wed',
  TH: 'common.days.short.thu',
  F: 'common.days.short.fri',
  SA: 'common.days.short.sat',
  SU: 'common.days.short.sun',
} as const;

const FALLBACK_COLORS = ['#7c3aed', '#f97316', '#0ea5e9', '#10b981', '#ec4899', '#14b8a6', '#f59e0b'];

export function SchedulePreview({
  sections,
  startHour = 8,
  endHour = 22,
  showLegend = true,
}: SchedulePreviewProps) {
  const { t } = useTranslation();
  const dayLabels = useMemo(() => createDayLabels(t), [t]);
  const activeDays = getActiveDays(sections) ?? DAY_ORDER.slice(0, 5);
  const hours = buildHourScale(startHour, endHour);
  const totalMinutes = (endHour - startHour) * 60;
  const blocks = flattenMeetings(sections);

  return (
    <section className="schedule-preview">
      <div className="schedule-preview__header">
        <div>
          <p className="schedule-preview__eyebrow">{t('schedule.header.eyebrow')}</p>
          <h3>{t('schedule.header.title')}</h3>
        </div>
        <span className="schedule-preview__count">{t('schedule.header.count', { count: sections.length })}</span>
      </div>

      <div
        className="schedule-preview__grid"
        style={{ gridTemplateColumns: `60px repeat(${activeDays.length}, minmax(0, 1fr))` }}
      >
        <div className="schedule-preview__hours">
          {hours.map((hour) => (
            <span key={hour} className="schedule-preview__hour">
              {formatHour(hour, t)}
            </span>
          ))}
        </div>
        {activeDays.map((day) => (
          <div key={day} className="schedule-preview__day">
            <header className="schedule-preview__day-header">{dayLabels[day]}</header>
            <div className="schedule-preview__day-body">
              {blocks
                .filter((block) => block.meeting.day === day)
                .map((block) => {
                  const position = positionBlock(block.meeting, startHour, totalMinutes);
                  if (!position) return null;
                  const { top, height } = position;
                  return (
                    <article
                      key={block.id}
                      className="schedule-preview__block"
                      style={{ top: `${top}%`, height: `${height}%`, backgroundColor: block.color }}
                    >
                      <p className="schedule-preview__block-code">
                        {block.courseCode}-{block.sectionCode}
                      </p>
                      <p className="schedule-preview__block-title">{block.title}</p>
                      <p className="schedule-preview__block-meta">{formatTimeRange(block.meeting, t)}</p>
                      {block.location && <p className="schedule-preview__block-meta">{block.location}</p>}
                    </article>
                  );
                })}
            </div>
          </div>
        ))}
      </div>

      {showLegend && sections.length > 0 && (
        <footer className="schedule-preview__legend">
          {sections.map((section, index) => (
            <div key={section.id} className="schedule-preview__legend-item">
              <span
                className="schedule-preview__legend-swatch"
                style={{ backgroundColor: section.color ?? FALLBACK_COLORS[index % FALLBACK_COLORS.length] }}
              />
              <div>
                <strong>
                  {section.courseCode}-{section.sectionCode}
                </strong>
                <p>{section.title}</p>
              </div>
            </div>
          ))}
        </footer>
      )}
    </section>
  );
}

const getActiveDays = (sections: ScheduleSection[]): MeetingDay[] | null => {
  const days = new Set<MeetingDay>();
  sections.forEach((section) =>
    section.meetings.forEach((meeting) => {
      days.add(meeting.day);
    }),
  );
  if (!days.size) return null;
  return DAY_ORDER.filter((day) => days.has(day));
};

const buildHourScale = (start: number, end: number): number[] => {
  const hours: number[] = [];
  for (let hour = start; hour <= end; hour += 1) {
    hours.push(hour);
  }
  return hours;
};

interface ScheduleBlock {
  id: string;
  meeting: ScheduleMeeting;
  courseCode: string;
  sectionCode: string;
  title: string;
  location?: string;
  color: string;
}

const flattenMeetings = (sections: ScheduleSection[]): ScheduleBlock[] => {
  const blocks: ScheduleBlock[] = [];
  sections.forEach((section, sectionIndex) => {
    const baseColor = section.color ?? FALLBACK_COLORS[sectionIndex % FALLBACK_COLORS.length];
    section.meetings.forEach((meeting, meetingIndex) => {
      if (meeting.endMinutes <= meeting.startMinutes) return;
      blocks.push({
        id: `${section.id}-${meetingIndex}`,
        meeting,
        courseCode: section.courseCode,
        sectionCode: section.sectionCode,
        title: section.title,
        location: section.location,
        color: baseColor,
      });
    });
  });
  return blocks;
};

const createDayLabels = (t: TFunction): Record<MeetingDay, string> => {
  const labels = {} as Record<MeetingDay, string>;
  DAY_ORDER.forEach((day) => {
    labels[day] = t(DAY_KEYS[day]);
  });
  return labels;
};

const positionBlock = (
  meeting: ScheduleMeeting,
  startHour: number,
  totalMinutes: number,
): { top: number; height: number } | null => {
  const windowStart = startHour * 60;
  const windowEnd = windowStart + totalMinutes;

  if (meeting.endMinutes <= windowStart || meeting.startMinutes >= windowEnd) {
    return null;
  }

  const clampStart = Math.max(meeting.startMinutes, windowStart);
  const clampEnd = Math.min(meeting.endMinutes, windowEnd);
  if (clampEnd <= clampStart) return null;

  const top = ((clampStart - windowStart) / totalMinutes) * 100;
  const height = ((clampEnd - clampStart) / totalMinutes) * 100;
  return { top, height };
};

const formatTimeRange = (meeting: ScheduleMeeting, t: TFunction): string => {
  return `${formatMinutes(meeting.startMinutes, t)} – ${formatMinutes(meeting.endMinutes, t)}`;
};

const formatMinutes = (minutes: number, t: TFunction): string => {
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  const suffix = hours >= 12 ? t('common.time.pm') : t('common.time.am');
  const normalized = ((hours + 11) % 12) + 1;
  return `${normalized}:${mins.toString().padStart(2, '0')} ${suffix}`;
};

const formatHour = (hour: number, t: TFunction): string => {
  const suffix = hour >= 12 ? t('common.time.pm') : t('common.time.am');
  const normalized = ((hour + 11) % 12) + 1;
  return `${normalized}${suffix}`;
};
