import type { ChangeEvent } from 'react';
import { useMemo, useState } from 'react';
import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import type {
  CourseFilterState,
  DeliveryMethod,
  MeetingDay,
} from '../state/courseFilters';
import { createInitialCourseFilterState } from '../state/courseFilters';
import { TagChip } from './TagChip';
import { classNames } from '../utils/classNames';
import './FilterPanel.css';

type LevelValue = CourseFilterState['level'][number];

export interface FilterOption {
  label: string;
  value: string;
  description?: string;
  badge?: string;
}

export interface SubjectOption extends FilterOption {
  school?: string;
}

export interface LevelOption {
  label: string;
  value: LevelValue;
  description?: string;
}

export interface DeliveryOption {
  label: string;
  value: DeliveryMethod;
  description?: string;
}

export interface FiltersDictionary {
  terms: FilterOption[];
  campuses: FilterOption[];
  subjects: SubjectOption[];
  levels: LevelOption[];
  deliveries: DeliveryOption[];
  tags: FilterOption[];
  coreCodes: FilterOption[];
  instructors: FilterOption[];
}

export interface FilterPanelProps {
  state: CourseFilterState;
  dictionary: FiltersDictionary;
  onStateChange: (next: CourseFilterState) => void;
  onReset?: (next: CourseFilterState) => void;
  loading?: boolean;
}

const MEETING_DAY_ORDER: MeetingDay[] = ['M', 'T', 'W', 'TH', 'F', 'SA', 'SU'];

const MEETING_DAY_KEYS = {
  M: 'common.days.short.mon',
  T: 'common.days.short.tue',
  W: 'common.days.short.wed',
  TH: 'common.days.short.thu',
  F: 'common.days.short.fri',
  SA: 'common.days.short.sat',
  SU: 'common.days.short.sun',
} as const;

const toggleValue = <T,>(list: T[], value: T): T[] =>
  list.includes(value) ? list.filter((entry) => entry !== value) : [...list, value];

const minutesToTimeInput = (minutes?: number): string => {
  if (minutes === undefined) return '';
  const normalized = Math.max(0, Math.min(minutes, 23 * 60 + 59));
  const hours = Math.floor(normalized / 60)
    .toString()
    .padStart(2, '0');
  const mins = (normalized % 60).toString().padStart(2, '0');
  return `${hours}:${mins}`;
};

const timeInputToMinutes = (value: string): number | undefined => {
  if (!value) return undefined;
  const [hours, mins] = value.split(':').map((token) => Number(token));
  if (Number.isNaN(hours) || Number.isNaN(mins)) return undefined;
  return hours * 60 + mins;
};

export function FilterPanel({
  state,
  dictionary,
  onStateChange,
  onReset,
  loading = false,
}: FilterPanelProps) {
  const { t } = useTranslation();
  const [subjectQuery, setSubjectQuery] = useState('');

  const subjectLookup = useMemo(() => buildLookup(dictionary.subjects), [dictionary.subjects]);
  const levelLookup = useMemo(() => buildLookup(dictionary.levels), [dictionary.levels]);
  const deliveryLookup = useMemo(() => buildLookup(dictionary.deliveries), [dictionary.deliveries]);
  const tagLookup = useMemo(() => buildLookup(dictionary.tags), [dictionary.tags]);
  const coreLookup = useMemo(() => buildLookup(dictionary.coreCodes), [dictionary.coreCodes]);
  const meetingDayLabels = useMemo(() => createMeetingDayLabels(t), [t]);

  const emitState = (
    partial: Partial<CourseFilterState>,
    dirtyKey?: string,
    options: { resetPage?: boolean } = { resetPage: true },
  ) => {
    const dirty = new Set(state.dirtyFields);
    if (dirtyKey) dirty.add(dirtyKey);
    const shouldResetPage = options.resetPage ?? true;
    const nextPagination = partial.pagination
      ? partial.pagination
      : shouldResetPage
        ? { ...state.pagination, page: 1 }
        : { ...state.pagination };
    onStateChange({
      ...state,
      ...partial,
      pagination: nextPagination,
      dirtyFields: dirty,
    });
  };

  const filteredSubjects = useMemo(() => {
    if (!subjectQuery.trim()) return dictionary.subjects;
    const terms = subjectQuery.toLowerCase().split(/\s+/).filter(Boolean);
    return dictionary.subjects.filter((subject) => {
      const haystack = `${subject.school ?? ''} ${subject.label} ${subject.value}`.toLowerCase();
      return terms.every((term) => haystack.includes(term));
    });
  }, [dictionary.subjects, subjectQuery]);

  const trimmedSubjects = filteredSubjects.slice(0, 12);

  const chips = buildFilterChips({
    state,
    subjectLookup,
    levelLookup,
    deliveryLookup,
    tagLookup,
    coreLookup,
    emitState,
    meetingDayLabels,
    t,
  });

  const handleMeetingDayToggle = (day: MeetingDay) => {
    emitState(
      {
        meeting: {
          ...state.meeting,
          days: toggleValue(state.meeting.days, day),
        },
      },
      'meeting.days',
    );
  };

  const handleMeetingTimeChange = (field: 'startMinutes' | 'endMinutes', value: string) => {
    emitState(
      {
        meeting: {
          ...state.meeting,
          [field]: timeInputToMinutes(value),
        },
      },
      'meeting.time',
    );
  };

  const clearMeeting = () => {
    emitState(
      {
        meeting: { days: [], startMinutes: undefined, endMinutes: undefined },
      },
      'meeting',
    );
  };

  const clearAll = () => {
    const base = createInitialCourseFilterState();
    base.term = state.term;
    base.campus = state.campus;
    onReset ? onReset(base) : onStateChange(base);
  };

  const handleOpenStatus = (next: CourseFilterState['openStatus']) => {
    emitState({ openStatus: next }, 'openStatus');
  };

  const handleQueryChange = (event: ChangeEvent<HTMLInputElement>) => {
    emitState(
      {
        queryText: event.target.value,
        pagination: { ...state.pagination, page: 1 },
      },
      'queryText',
      { resetPage: false },
    );
  };

  return (
    <aside className="filter-panel">
      <header className="filter-panel__header">
        <div>
          <p className="filter-panel__eyebrow">{t('filters.header.eyebrow')}</p>
          <h2 className="filter-panel__title">{t('filters.header.title')}</h2>
          <p className="filter-panel__subtitle">{t('filters.header.subtitle')}</p>
        </div>
        <button type="button" className="filter-panel__reset" onClick={clearAll} disabled={loading}>
          {t('filters.header.reset')}
        </button>
      </header>

      {chips.length > 0 && (
        <div className="filter-panel__chips">
          {chips.map((chip) => (
            <TagChip key={chip.id} label={chip.label} value={chip.value} tone={chip.tone} onRemove={chip.onRemove} />
          ))}
        </div>
      )}

      <section className="filter-panel__section">
        <div className="filter-panel__section-heading">
          <h3>{t('filters.sections.basic.title')}</h3>
          {loading && <span className="filter-panel__badge">{t('filters.status.loadingBadge')}</span>}
        </div>
        <div className="filter-panel__grid">
          <label className="filter-panel__control">
            <span>{t('filters.sections.basic.term.label')}</span>
            <select
              value={state.term ?? ''}
              onChange={(event) => emitState({ term: event.target.value || undefined }, 'term')}
            >
              <option value="" disabled>
                {t('filters.sections.basic.term.placeholder')}
              </option>
              {dictionary.terms.map((term) => (
                <option key={term.value} value={term.value}>
                  {term.label}
                </option>
              ))}
            </select>
          </label>

          <label className="filter-panel__control">
            <span>{t('filters.sections.basic.campus.label')}</span>
            <select
              value={state.campus ?? ''}
              onChange={(event) => emitState({ campus: event.target.value || undefined }, 'campus')}
            >
              <option value="">{t('filters.sections.basic.campus.all')}</option>
              {dictionary.campuses.map((campus) => (
                <option key={campus.value} value={campus.value}>
                  {campus.label}
                </option>
              ))}
            </select>
          </label>
        </div>

        <label className="filter-panel__control">
          <span>{t('filters.sections.basic.keyword.label')}</span>
          <input
            type="text"
            placeholder={t('filters.sections.basic.keyword.placeholder')}
            value={state.queryText}
            onChange={handleQueryChange}
          />
        </label>

        <div className="filter-panel__open-status">
          <span>{t('filters.sections.basic.openStatus.label')}</span>
          <div className="filter-panel__pill-group">
            <button
              type="button"
              className={classNames(
                'filter-panel__pill',
                state.openStatus === 'all' && 'filter-panel__pill--active',
              )}
              onClick={() => handleOpenStatus('all')}
            >
              {t('filters.sections.basic.openStatus.all')}
            </button>
            <button
              type="button"
              className={classNames(
                'filter-panel__pill',
                state.openStatus === 'openOnly' && 'filter-panel__pill--active',
              )}
              onClick={() => handleOpenStatus('openOnly')}
            >
              {t('filters.sections.basic.openStatus.openOnly')}
            </button>
            <button
              type="button"
              className={classNames(
                'filter-panel__pill',
                state.openStatus === 'hasWaitlist' && 'filter-panel__pill--active',
              )}
              onClick={() => handleOpenStatus('hasWaitlist')}
            >
              {t('filters.sections.basic.openStatus.waitlist')}
            </button>
          </div>
        </div>
      </section>

      <section className="filter-panel__section">
        <div className="filter-panel__section-heading">
          <h3>{t('filters.sections.subjects.title')}</h3>
          <button type="button" className="filter-panel__clear-btn" onClick={() => emitState({ subjects: [] }, 'subjects')}>
            {t('filters.sections.subjects.clear')}
          </button>
        </div>

        <label className="filter-panel__control">
          <span>{t('filters.sections.subjects.search.label')}</span>
          <input
            type="search"
            placeholder={t('filters.sections.subjects.search.placeholder')}
            value={subjectQuery}
            onChange={(event) => setSubjectQuery(event.target.value)}
          />
        </label>

        <div className="filter-panel__checkboxes">
          {trimmedSubjects.map((subject) => (
            <label key={subject.value}>
              <input
                type="checkbox"
                checked={state.subjects.includes(subject.value)}
                onChange={() => emitState({ subjects: toggleValue(state.subjects, subject.value) }, 'subjects')}
              />
              <span>
                {subject.label} <small>{subject.value}</small>
              </span>
            </label>
          ))}
          {filteredSubjects.length > trimmedSubjects.length && (
            <span className="filter-panel__hint">
              {t('filters.sections.subjects.truncatedHint', {
                current: trimmedSubjects.length,
                total: filteredSubjects.length,
              })}
            </span>
          )}
        </div>
      </section>

      <section className="filter-panel__section">
        <div className="filter-panel__section-heading">
          <h3>{t('filters.sections.meeting.title')}</h3>
          <button type="button" className="filter-panel__clear-btn" onClick={clearMeeting}>
            {t('filters.sections.meeting.clear')}
          </button>
        </div>

        <div className="filter-panel__days">
          {MEETING_DAY_ORDER.map((day) => (
            <button
              key={day}
              type="button"
              className={classNames(
                'filter-panel__day',
                state.meeting.days.includes(day) && 'filter-panel__day--selected',
              )}
              onClick={() => handleMeetingDayToggle(day)}
            >
              {meetingDayLabels[day]}
            </button>
          ))}
        </div>

        <div className="filter-panel__grid">
          <label className="filter-panel__control">
            <span>{t('filters.sections.meeting.start')}</span>
            <input
              type="time"
              value={minutesToTimeInput(state.meeting.startMinutes)}
              onChange={(event) => handleMeetingTimeChange('startMinutes', event.target.value)}
            />
          </label>
          <label className="filter-panel__control">
            <span>{t('filters.sections.meeting.end')}</span>
            <input
              type="time"
              value={minutesToTimeInput(state.meeting.endMinutes)}
              onChange={(event) => handleMeetingTimeChange('endMinutes', event.target.value)}
            />
          </label>
        </div>
      </section>

      <section className="filter-panel__section">
        <div className="filter-panel__section-heading">
          <h3>{t('filters.sections.meta.title')}</h3>
          <button
            type="button"
            className="filter-panel__clear-btn"
            onClick={() => emitState({ delivery: [], level: [] }, 'meta')}
          >
            {t('filters.sections.meta.clear')}
          </button>
        </div>

        <div className="filter-panel__subgrid">
          <div>
            <p className="filter-panel__label">{t('filters.sections.meta.delivery')}</p>
            <div className="filter-panel__checkboxes">
              {dictionary.deliveries.map((delivery) => (
                <label key={delivery.value}>
                  <input
                    type="checkbox"
                    checked={state.delivery.includes(delivery.value)}
                    onChange={() => emitState({ delivery: toggleValue(state.delivery, delivery.value) }, 'delivery')}
                  />
                  <span>{delivery.label}</span>
                </label>
              ))}
            </div>
          </div>
          <div>
            <p className="filter-panel__label">{t('filters.sections.meta.level')}</p>
            <div className="filter-panel__checkboxes">
              {dictionary.levels.map((level) => (
                <label key={level.value}>
                  <input
                    type="checkbox"
                    checked={state.level.includes(level.value)}
                    onChange={() => emitState({ level: toggleValue(state.level, level.value) }, 'level')}
                  />
                  <span>{level.label}</span>
                </label>
              ))}
            </div>
          </div>
        </div>
      </section>

      {dictionary.tags.length > 0 && (
        <section className="filter-panel__section">
          <div className="filter-panel__section-heading">
            <h3>{t('filters.sections.tags.title')}</h3>
            <button type="button" className="filter-panel__clear-btn" onClick={() => emitState({ tags: [] }, 'tags')}>
              {t('filters.sections.tags.clear')}
            </button>
          </div>

          <div className="filter-panel__chip-grid">
            {dictionary.tags.map((tag) => {
              const isActive = state.tags.includes(tag.value);
              return (
                <TagChip
                  key={tag.value}
                  label={tag.label}
                  tone={isActive ? 'success' : 'default'}
                  active={isActive}
                  onClick={() => emitState({ tags: toggleValue(state.tags, tag.value) }, 'tags')}
                />
              );
            })}
          </div>
        </section>
      )}
    </aside>
  );
}

type ChipDescriptor = {
  id: string;
  label: string;
  value: string;
  tone: 'default' | 'info' | 'success' | 'warning';
  onRemove: () => void;
};

const createMeetingDayLabels = (t: TFunction): Record<MeetingDay, string> => {
  const labels = {} as Record<MeetingDay, string>;
  MEETING_DAY_ORDER.forEach((day) => {
    labels[day] = t(MEETING_DAY_KEYS[day]);
  });
  return labels;
};

const buildFilterChips = ({
  state,
  subjectLookup,
  levelLookup,
  deliveryLookup,
  tagLookup,
  coreLookup,
  emitState,
  meetingDayLabels,
  t,
}: {
  state: CourseFilterState;
  subjectLookup: Map<string, SubjectOption>;
  levelLookup: Map<string, LevelOption>;
  deliveryLookup: Map<string, DeliveryOption>;
  tagLookup: Map<string, FilterOption>;
  coreLookup: Map<string, FilterOption>;
  emitState: (partial: Partial<CourseFilterState>, dirtyKey?: string) => void;
  meetingDayLabels: Record<MeetingDay, string>;
  t: TFunction;
}): ChipDescriptor[] => {
  const chips: ChipDescriptor[] = [];
  const chipLabels = {
    keyword: t('filters.chips.keyword'),
    subject: t('filters.chips.subject'),
    level: t('filters.chips.level'),
    delivery: t('filters.chips.delivery'),
    tag: t('filters.chips.tag'),
    core: t('filters.chips.core'),
    meeting: t('filters.chips.meeting'),
    openOnly: t('filters.chips.openOnly'),
    hasWaitlist: t('filters.chips.hasWaitlist'),
  };
  if (state.queryText.trim()) {
    chips.push({
      id: 'query',
      label: chipLabels.keyword,
      value: state.queryText.trim(),
      tone: 'info',
      onRemove: () => emitState({ queryText: '' }, 'queryText'),
    });
  }

  state.subjects.forEach((subject) => {
    const option = subjectLookup.get(subject);
    chips.push({
      id: `subject:${subject}`,
      label: option ? option.label : chipLabels.subject,
      value: subject,
      tone: 'default',
      onRemove: () =>
        emitState({ subjects: state.subjects.filter((entry) => entry !== subject) }, 'subjects'),
    });
  });

  state.level.forEach((level) => {
    const option = levelLookup.get(level);
    chips.push({
      id: `level:${level}`,
      label: chipLabels.level,
      value: option ? option.label : level,
      tone: 'success',
      onRemove: () => emitState({ level: state.level.filter((entry) => entry !== level) }, 'level'),
    });
  });

  state.delivery.forEach((delivery) => {
    const option = deliveryLookup.get(delivery);
    chips.push({
      id: `delivery:${delivery}`,
      label: chipLabels.delivery,
      value: option ? option.label : delivery,
      tone: 'info',
      onRemove: () =>
        emitState({ delivery: state.delivery.filter((entry) => entry !== delivery) }, 'delivery'),
    });
  });

  state.tags.forEach((tag) => {
    const option = tagLookup.get(tag);
    chips.push({
      id: `tag:${tag}`,
      label: chipLabels.tag,
      value: option ? option.label : tag,
      tone: 'success',
      onRemove: () => emitState({ tags: state.tags.filter((entry) => entry !== tag) }, 'tags'),
    });
  });

  state.coreCodes.forEach((core) => {
    const option = coreLookup.get(core);
    chips.push({
      id: `core:${core}`,
      label: option ? option.label : chipLabels.core,
      value: core,
      tone: 'warning',
      onRemove: () =>
        emitState({ coreCodes: state.coreCodes.filter((entry) => entry !== core) }, 'coreCodes'),
    });
  });

  if (
    state.meeting.days.length ||
    state.meeting.startMinutes !== undefined ||
    state.meeting.endMinutes !== undefined
  ) {
    chips.push({
      id: 'meeting',
      label: chipLabels.meeting,
      value: formatMeetingChip(state, t, meetingDayLabels),
      tone: 'info',
      onRemove: () =>
        emitState({ meeting: { days: [], startMinutes: undefined, endMinutes: undefined } }, 'meeting'),
    });
  }

  if (state.openStatus === 'openOnly') {
    chips.push({
      id: 'openOnly',
      label: chipLabels.openOnly,
      value: '',
      tone: 'success',
      onRemove: () => emitState({ openStatus: 'all' }, 'openStatus'),
    });
  }

  if (state.openStatus === 'hasWaitlist') {
    chips.push({
      id: 'hasWaitlist',
      label: chipLabels.hasWaitlist,
      value: '',
      tone: 'success',
      onRemove: () => emitState({ openStatus: 'all' }, 'openStatus'),
    });
  }

  return chips;
};

const buildLookup = <T extends { value: string }>(list: T[]): Map<string, T> => {
  const map = new Map<string, T>();
  list.forEach((item) => map.set(item.value, item));
  return map;
};

const formatMeetingChip = (
  state: CourseFilterState,
  t: TFunction,
  meetingDayLabels: Record<MeetingDay, string>,
): string => {
  const segments: string[] = [];
  if (state.meeting.days.length) {
    segments.push(state.meeting.days.map((day) => meetingDayLabels[day]).join('/'));
  }
  if (state.meeting.startMinutes !== undefined || state.meeting.endMinutes !== undefined) {
    const start =
      state.meeting.startMinutes !== undefined
        ? minutesToHuman(state.meeting.startMinutes, t)
        : t('filters.chips.timePlaceholder.start');
    const end =
      state.meeting.endMinutes !== undefined
        ? minutesToHuman(state.meeting.endMinutes, t)
        : t('filters.chips.timePlaceholder.end');
    segments.push(`${start}-${end}`);
  }
  return segments.join(' · ') || t('filters.chips.meetingFallback');
};

const minutesToHuman = (minutes: number, t: TFunction): string => {
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  const suffix = hours >= 12 ? t('common.time.pm') : t('common.time.am');
  const normalizedHours = ((hours + 11) % 12) + 1;
  return `${normalizedHours}:${mins.toString().padStart(2, '0')} ${suffix}`;
};
