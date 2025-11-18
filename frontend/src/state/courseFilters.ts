/**
 * Shared filter state contract for the course list + calendar views.
 * Mirrors the architecture notes in docs/ui_flow_course_list.md.
 */

export type MeetingDay = 'M' | 'T' | 'W' | 'TH' | 'F' | 'SA' | 'SU';

export type DeliveryMethod = 'in_person' | 'online' | 'hybrid';

export type CourseFilterSortField =
  | 'relevance'
  | 'courseNumber'
  | 'title'
  | 'updated';

export interface MeetingFilter {
  days: MeetingDay[];
  startMinutes?: number;
  endMinutes?: number;
  campusCodes: string[];
  locationKeywords: string[];
}

export interface CourseFilterState {
  term?: string;
  campus?: string;
  subjects: string[];
  queryText: string;
  level: Array<'UG' | 'GR' | 'N/A'>;
  credits: { min?: number; max?: number };
  coreCodes: string[];
  keywords: string[]; // derived quick toggles (permission-only, honors, etc.)
  tags: string[]; // UI preset shortcuts (writing-intensive, STEM, etc.)
  meeting: MeetingFilter;
  instructors: string[];
  delivery: DeliveryMethod[];
  openStatus: 'all' | 'openOnly' | 'hasWaitlist';
  pagination: { page: number; pageSize: number };
  sort: { field: CourseFilterSortField; dir: 'asc' | 'desc' };
  uiStatus: 'idle' | 'loading' | 'error';
  dirtyFields: Set<string>;
}

export const DEFAULT_PAGE_SIZE = 25;

export const createInitialCourseFilterState = (): CourseFilterState => ({
  term: undefined,
  campus: undefined,
  subjects: [],
  queryText: '',
  level: [],
  credits: {},
  coreCodes: [],
  keywords: [],
  tags: [],
  meeting: {
    days: [],
    campusCodes: [],
    locationKeywords: [],
  },
  instructors: [],
  delivery: [],
  openStatus: 'all',
  pagination: { page: 1, pageSize: DEFAULT_PAGE_SIZE },
  sort: { field: 'relevance', dir: 'desc' },
  uiStatus: 'idle',
  dirtyFields: new Set(),
});

const MULTI_VALUE_KEYS = new Set([
  'subject',
  'level',
  'coreCode',
  'delivery',
  'meetingCampus',
  'tag',
  'keyword',
  'instructor',
]);

type PrimitiveParam = string | number | boolean;
type QueryParamValue = PrimitiveParam | PrimitiveParam[];

/**
 * Translate the UI state into the REST-friendly query object used by `/api/courses`.
 */
export const buildCourseQueryParams = (
  state: CourseFilterState,
): Record<string, QueryParamValue> => {
  if (!state.term) {
    throw new Error('CourseFilterState.term is required before querying.');
  }

  const params: Record<string, QueryParamValue> = {
    term: state.term,
    page: state.pagination.page,
    pageSize: state.pagination.pageSize,
  };

  if (state.campus) params.campus = state.campus;
  if (state.subjects.length) params.subject = [...state.subjects];
  if (state.queryText.trim()) params.q = state.queryText.trim();
  if (state.level.length) params.level = [...state.level];
  if (state.coreCodes.length) params.coreCode = [...state.coreCodes];
  if (state.keywords.length) params.keyword = [...state.keywords];
  if (state.tags.length) params.tag = [...state.tags];
  if (state.delivery.length) params.delivery = [...state.delivery];
  if (state.instructors.length) params.instructor = [...state.instructors];

  if (state.credits.min !== undefined) params.creditsMin = state.credits.min;
  if (state.credits.max !== undefined) params.creditsMax = state.credits.max;

  if (state.openStatus === 'openOnly') params.hasOpenSection = true;
  if (state.openStatus === 'hasWaitlist') params.hasWaitlist = true;

  if (state.meeting.days.length) params.meetingDays = state.meeting.days.join('');
  if (state.meeting.startMinutes !== undefined) {
    params.meetingStart = state.meeting.startMinutes;
  }
  if (state.meeting.endMinutes !== undefined) {
    params.meetingEnd = state.meeting.endMinutes;
  }
  if (state.meeting.campusCodes.length) {
    params.meetingCampus = [...state.meeting.campusCodes];
  }
  if (state.meeting.locationKeywords.length) {
    params.meetingLocation = [...state.meeting.locationKeywords];
  }

  if (state.sort.field !== 'relevance' || state.sort.dir !== 'desc') {
    params.sort = `${state.sort.field}:${state.sort.dir}`;
  }

  return params;
};

/**
 * Serializes filter state into the search params strategy defined in docs/ui_flow_course_list.md.
 */
export const serializeCourseFilters = (state: CourseFilterState): URLSearchParams => {
  const params = new URLSearchParams();
  const appendAll = (key: string, values: Array<string | number | boolean>) => {
    values.forEach((entry) => params.append(key, String(entry)));
  };

  if (state.term) params.set('term', state.term);
  if (state.campus) params.set('campus', state.campus);
  if (state.subjects.length) appendAll('subject', state.subjects);
  if (state.queryText.trim()) params.set('q', state.queryText.trim());
  if (state.level.length) appendAll('level', state.level);
  if (state.coreCodes.length) appendAll('coreCode', state.coreCodes);
  if (state.keywords.length) appendAll('keyword', state.keywords);
  if (state.tags.length) appendAll('tag', state.tags);
  if (state.delivery.length) appendAll('delivery', state.delivery);
  if (state.instructors.length) appendAll('instructor', state.instructors);
  if (state.credits.min !== undefined) params.set('creditsMin', String(state.credits.min));
  if (state.credits.max !== undefined) params.set('creditsMax', String(state.credits.max));
  if (state.openStatus === 'openOnly') params.set('hasOpenSection', 'true');
  if (state.openStatus === 'hasWaitlist') params.set('hasWaitlist', 'true');
  if (state.meeting.days.length) params.set('meetingDays', state.meeting.days.join(''));
  if (state.meeting.startMinutes !== undefined) {
    params.set('meetingStart', String(state.meeting.startMinutes));
  }
  if (state.meeting.endMinutes !== undefined) {
    params.set('meetingEnd', String(state.meeting.endMinutes));
  }
  if (state.meeting.campusCodes.length) appendAll('meetingCampus', state.meeting.campusCodes);
  if (state.meeting.locationKeywords.length) {
    appendAll('meetingLocation', state.meeting.locationKeywords);
  }

  if (state.pagination.page > 1) params.set('page', String(state.pagination.page));
  if (state.pagination.pageSize !== DEFAULT_PAGE_SIZE) {
    params.set('pageSize', String(state.pagination.pageSize));
  }

  if (state.sort.field !== 'relevance' || state.sort.dir !== 'desc') {
    params.set('sort', `${state.sort.field}:${state.sort.dir}`);
  }

  return params;
};

/**
 * Parses URLSearchParams back into a state snapshot. Unknown params are ignored.
 */
export const parseCourseFiltersFromSearch = (
  search: string,
  baseState: CourseFilterState = createInitialCourseFilterState(),
): CourseFilterState => {
  const params = new URLSearchParams(search.startsWith('?') ? search.slice(1) : search);
  const state: CourseFilterState = {
    ...baseState,
    subjects: [],
    level: [],
    coreCodes: [],
    keywords: [],
    tags: [],
    meeting: {
      days: [],
      startMinutes: undefined,
      endMinutes: undefined,
      campusCodes: [],
      locationKeywords: [],
    },
    instructors: [],
    delivery: [],
    pagination: { ...baseState.pagination },
    dirtyFields: new Set(baseState.dirtyFields),
  };

  params.forEach((value, key) => {
    if (MULTI_VALUE_KEYS.has(key)) {
      switch (key) {
        case 'subject':
          state.subjects.push(value);
          break;
        case 'level':
          state.level.push(value as CourseFilterState['level'][number]);
          break;
        case 'coreCode':
          state.coreCodes.push(value);
          break;
        case 'delivery':
          state.delivery.push(value as DeliveryMethod);
          break;
        case 'meetingCampus':
          state.meeting.campusCodes.push(value);
          break;
        case 'tag':
          state.tags.push(value);
          break;
        case 'keyword':
          state.keywords.push(value);
          break;
        case 'instructor':
          state.instructors.push(value);
          break;
      }
      return;
    }

    switch (key) {
      case 'term':
        state.term = value;
        break;
      case 'campus':
        state.campus = value;
        break;
      case 'q':
        state.queryText = value;
        break;
      case 'creditsMin':
        state.credits.min = Number(value);
        break;
      case 'creditsMax':
        state.credits.max = Number(value);
        break;
      case 'meetingDays':
        state.meeting.days = parseMeetingDays(value);
        break;
      case 'meetingStart':
        state.meeting.startMinutes = Number(value);
        break;
      case 'meetingEnd':
        state.meeting.endMinutes = Number(value);
        break;
      case 'meetingLocation':
        state.meeting.locationKeywords.push(value);
        break;
      case 'hasOpenSection':
        if (value === 'true') state.openStatus = 'openOnly';
        break;
      case 'hasWaitlist':
        if (value === 'true') state.openStatus = 'hasWaitlist';
        break;
      case 'page':
        state.pagination.page = Number(value);
        break;
      case 'pageSize':
        state.pagination.pageSize = Number(value);
        break;
      case 'sort': {
        const [field, dir] = value.split(':');
        if (field) {
          state.sort.field = field as CourseFilterSortField;
        }
        if (dir === 'asc' || dir === 'desc') {
          state.sort.dir = dir;
        }
        break;
      }
      default:
        break;
    }
  });

  return state;
};

const parseMeetingDays = (raw: string): MeetingDay[] => {
  const upper = raw.toUpperCase();
  const tokens: MeetingDay[] = [];
  for (let i = 0; i < upper.length; i += 1) {
    const char = upper[i];
    if (char === ',' || char.trim() === '') continue;
    const double = upper.slice(i, i + 2);
    if (double === 'TH' || double === 'SA' || double === 'SU') {
      tokens.push(double as MeetingDay);
      i += 1;
      continue;
    }
    if (char === 'M' || char === 'T' || char === 'W' || char === 'F') {
      tokens.push(char as MeetingDay);
      continue;
    }
  }
  return tokens;
};
