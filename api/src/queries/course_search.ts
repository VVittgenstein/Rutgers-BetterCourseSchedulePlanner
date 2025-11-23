import type Database from 'better-sqlite3';

import type { CoursesQuery } from '../routes/courses.js';

type CourseInclude = 'sectionsSummary' | 'subjects' | 'sections';

export interface CourseSearchSubject {
  code: string;
  description: string | null;
  schoolCode: string | null;
  schoolDescription: string | null;
}

export interface CourseSectionsSummary {
  total: number;
  open: number;
  deliveryMethods: string[];
}

export interface CourseSectionMeeting {
  meetingDay: string | null;
  startMinutes: number | null;
  endMinutes: number | null;
  campus?: string | null;
  building?: string | null;
  room?: string | null;
}

export interface CourseSectionRow {
  sectionId: number;
  indexNumber: string;
  sectionNumber: string | null;
  openStatus: string | null;
  isOpen: boolean;
  deliveryMethod: string | null;
  campusCode: string | null;
  meetingCampus?: string | null;
  instructorsText?: string | null;
  meetingModeSummary?: string | null;
  meetings: CourseSectionMeeting[];
}

export interface CourseSearchRow {
  courseId: number;
  termId: string;
  campusCode: string;
  subjectCode: string;
  courseNumber: string;
  courseString: string | null;
  title: string;
  expandedTitle: string | null;
  level: string | null;
  creditsMin: number | null;
  creditsMax: number | null;
  creditsDisplay: string | null;
  coreAttributes: unknown;
  hasOpenSections: boolean;
  sectionsOpen: number;
  updatedAt: string | null;
  prerequisites: string | null;
  subject?: CourseSearchSubject;
  sectionsSummary?: CourseSectionsSummary;
  sections?: CourseSectionRow[];
}

export interface CourseSearchResult {
  data: CourseSearchRow[];
  total: number;
}

const VALID_DELIVERY = new Set(['in_person', 'online', 'hybrid']);
const VALID_INCLUDES: CourseInclude[] = ['sectionsSummary', 'subjects', 'sections'];
const SECTION_PREVIEW_LIMIT = 200;

const DAY_BITMASK: Record<string, number> = {
  M: 1,
  T: 2,
  W: 4,
  TH: 8,
  F: 16,
  SA: 32,
  SU: 64,
};
const DAY_MASK_ALL = Object.values(DAY_BITMASK).reduce((mask, value) => mask | value, 0);

const SORT_COLUMNS = {
  subject: 'c.subject_code',
  courseNumber: 'c.course_number',
  title: 'lower(c.title)',
  credits: 'COALESCE(c.credits_min, c.credits_max, 0)',
  sectionsOpen: 'COALESCE(c.open_sections_count, 0)',
  updatedAt: "COALESCE(c.updated_at, c.created_at, '')",
} as const;

const SORT_DEFAULT_DIRECTION: Partial<Record<keyof typeof SORT_COLUMNS, 'asc' | 'desc'>> = {
  sectionsOpen: 'desc',
  updatedAt: 'desc',
};

class SqlBinder {
  #counter = 0;
  #params: Record<string, unknown> = {};

  bind(value: unknown) {
    const name = `p${this.#counter++}`;
    this.#params[name] = value;
    return `@${name}`;
  }

  cloneParams() {
    return { ...this.#params };
  }
}

interface SectionFilterInput {
  deliveries: string[];
  examCodes: string[];
  meetingMask?: number;
  meetingStart?: number;
  meetingEnd?: number;
  meetingCampuses: string[];
}

export function executeCourseSearch(db: Database.Database, query: CoursesQuery): CourseSearchResult {
  const binder = new SqlBinder();
  const filters: string[] = [];
  filters.push(`c.term_id = ${binder.bind(query.term)}`);

  const campusFilter = normalizeStringList(query.campus, (value) => value.toUpperCase());
  if (campusFilter.length) {
    filters.push(buildInClause('c.campus_code', campusFilter, binder));
  }

  const campusLocations = normalizeStringList(query.campusLocation, (value) => value.toUpperCase());
  if (campusLocations.length) {
    filters.push(`
      EXISTS (
        SELECT 1
        FROM course_campus_locations ccl
        WHERE ccl.course_id = c.course_id
          AND ccl.term_id = c.term_id
          AND ccl.campus_code = c.campus_code
          AND ${buildInClause('ccl.location_code', campusLocations, binder)}
      )
    `);
  }

  const subjects = normalizeSubjectCodes(query.subject);
  if (subjects.length) {
    filters.push(buildInClause('c.subject_code', subjects, binder));
  }

  const levels = normalizeLevelFilters(query.level);
  if (levels.length) {
    filters.push(buildInClause('c.level', levels, binder));
  }

  const ftsQuery = buildFtsQuery(query.q);
  if (ftsQuery) {
    filters.push(`
      EXISTS (
        SELECT 1
        FROM course_search_fts fts
        WHERE fts.course_id = c.course_id
          AND fts.term_id = c.term_id
          AND fts.campus_code = c.campus_code
          AND fts.document MATCH ${binder.bind(ftsQuery)}
      )
    `);
  }

  const coreCodes = normalizeStringList(query.coreCode, (value) => value.toUpperCase());
  if (coreCodes.length) {
    filters.push(`
      EXISTS (
        SELECT 1
        FROM course_core_attributes cca
        WHERE cca.course_id = c.course_id
          AND ${buildInClause('upper(cca.core_code)', coreCodes, binder)}
      )
    `);
  }

  if (typeof query.creditsMin === 'number') {
    filters.push(`c.credits_min IS NOT NULL AND c.credits_min >= ${binder.bind(query.creditsMin)}`);
  }

  if (typeof query.creditsMax === 'number') {
    filters.push(`c.credits_max IS NOT NULL AND c.credits_max <= ${binder.bind(query.creditsMax)}`);
  }

  if (typeof query.hasPrerequisite === 'boolean') {
    if (query.hasPrerequisite) {
      filters.push(`c.prereq_plain IS NOT NULL AND c.prereq_plain <> ''`);
    } else {
      filters.push(`(c.prereq_plain IS NULL OR c.prereq_plain = '')`);
    }
  }

  if (typeof query.hasOpenSection === 'boolean') {
    filters.push(`c.has_open_sections = ${binder.bind(query.hasOpenSection ? 1 : 0)}`);
  }

  const deliveries = normalizeDeliveryList(query.delivery);
  const examCodes = normalizeExamCodeList(query.examCode);
  const meetingMask = buildMeetingMask(query.meetingDays);
  const meetingStart = typeof query.meetingStart === 'number' ? query.meetingStart : undefined;
  const meetingEnd = typeof query.meetingEnd === 'number' ? query.meetingEnd : undefined;
  const meetingCampuses = normalizeStringList(query.meetingCampus, (value) => value.toUpperCase());
  const outsideDayMask = typeof meetingMask === 'number' ? DAY_MASK_ALL & ~meetingMask : undefined;

  const meetingClauses: string[] = [];
  if (meetingMask !== undefined) {
    meetingClauses.push(`sm.week_mask IS NOT NULL AND sm.week_mask > 0`);
    meetingClauses.push(`(sm.week_mask & ${binder.bind(outsideDayMask ?? 0)}) = 0`);
  }
  if (meetingStart !== undefined) {
    meetingClauses.push(`sm.start_minutes IS NOT NULL AND sm.start_minutes >= ${binder.bind(meetingStart)}`);
  }
  if (meetingEnd !== undefined) {
    meetingClauses.push(`sm.end_minutes IS NOT NULL AND sm.end_minutes <= ${binder.bind(meetingEnd)}`);
  }

  const meetingLocationClauses: string[] = [];
  if (meetingCampuses.length) {
    meetingLocationClauses.push(
      buildInClause('upper(COALESCE(sm.campus_abbrev, sm.campus_location_code))', meetingCampuses, binder),
    );
  }

  const sectionConditions: string[] = [];
  if (deliveries.length) {
    sectionConditions.push(buildInClause('s_filter.delivery_method', deliveries, binder));
  }
  if (examCodes.length) {
    sectionConditions.push(buildInClause('upper(s_filter.exam_code)', examCodes, binder));
  }
  if (outsideDayMask !== undefined) {
    sectionConditions.push(`
      NOT EXISTS (
        SELECT 1
        FROM section_meetings sm_all
        WHERE sm_all.section_id = s_filter.section_id
          AND sm_all.week_mask IS NOT NULL
          AND sm_all.week_mask > 0
          AND (sm_all.week_mask & ${binder.bind(outsideDayMask)}) != 0
      )
    `);
  }

  const sectionFilter: SectionFilterInput = {
    deliveries,
    examCodes,
    meetingMask,
    meetingStart,
    meetingEnd,
    meetingCampuses,
  };

  const needsMeetingJoin = meetingClauses.length > 0 || meetingLocationClauses.length > 0;
  if (sectionConditions.length || meetingClauses.length || meetingLocationClauses.length) {
    const joined = [...sectionConditions, ...meetingClauses, ...meetingLocationClauses];
    const joinMeetings = needsMeetingJoin ? 'LEFT JOIN section_meetings sm ON sm.section_id = s_filter.section_id' : '';
    const conditionSql = joined.length ? ` AND ${joined.join(' AND ')}` : '';
    filters.push(`
      EXISTS (
        SELECT 1
        FROM sections s_filter
        ${joinMeetings}
        WHERE s_filter.course_id = c.course_id${conditionSql}
      )
    `);
  }

  const whereClause = filters.join(' AND ');

  const filterParams = binder.cloneParams();
  const totalRow = db
    .prepare(`SELECT COUNT(*) as total FROM courses c WHERE ${whereClause}`)
    .get(filterParams) as { total: number } | undefined;
  const total = typeof totalRow?.total === 'number' ? totalRow.total : 0;

  const { column: sortColumn, direction } = resolveSort(query.sortBy, query.sortDir);
  const ordering = `${sortColumn} ${direction}, c.subject_code ASC, c.course_number ASC, c.course_id ASC`;
  const selectSql = `
    SELECT
      c.course_id AS courseId,
      c.term_id AS termId,
      c.campus_code AS campusCode,
      c.subject_code AS subjectCode,
      c.course_number AS courseNumber,
      c.course_string AS courseString,
      c.title AS title,
      c.expanded_title AS expandedTitle,
      c.level AS level,
      c.credits_min AS creditsMin,
      c.credits_max AS creditsMax,
      c.credits_display AS creditsDisplay,
      c.core_json AS coreJson,
      c.has_open_sections AS hasOpenSections,
      c.open_sections_count AS sectionsOpen,
      c.updated_at AS updatedAt,
      c.prereq_plain AS prerequisites,
      subj.subject_description AS subjectDescription,
      subj.school_code AS schoolCode,
      subj.school_description AS schoolDescription
    FROM courses c
    LEFT JOIN subjects subj ON subj.subject_code = c.subject_code
    WHERE ${whereClause}
    ORDER BY ${ordering}
    LIMIT @limit OFFSET @offset
  `;

  const offset = (query.page - 1) * query.pageSize;
  const rows = db
    .prepare(selectSql)
    .all({ ...filterParams, limit: query.pageSize, offset }) as Array<{
      courseId: number;
      termId: string;
      campusCode: string;
      subjectCode: string;
      courseNumber: string;
      courseString: string | null;
      title: string;
      expandedTitle: string | null;
      level: string | null;
      creditsMin: number | null;
      creditsMax: number | null;
      creditsDisplay: string | null;
      coreJson: string | null;
      hasOpenSections: number | null;
      sectionsOpen: number | null;
      updatedAt: string | null;
      prerequisites: string | null;
      subjectDescription: string | null;
      schoolCode: string | null;
      schoolDescription: string | null;
    }>;

  const includeSet = buildIncludeSet(query.include);
  const mappedRows: CourseSearchRow[] = rows.map((row) => {
    const courseRow: CourseSearchRow = {
      courseId: row.courseId,
      termId: row.termId,
      campusCode: row.campusCode,
      subjectCode: row.subjectCode,
      courseNumber: row.courseNumber,
      courseString: row.courseString,
      title: row.title,
      expandedTitle: row.expandedTitle,
      level: row.level,
      creditsMin: row.creditsMin,
      creditsMax: row.creditsMax,
      creditsDisplay: row.creditsDisplay,
      coreAttributes: safeJson(row.coreJson),
      hasOpenSections: row.hasOpenSections === 1,
      sectionsOpen: typeof row.sectionsOpen === 'number' ? row.sectionsOpen : 0,
      updatedAt: row.updatedAt,
      prerequisites: row.prerequisites,
    };

    if (includeSet.has('subjects')) {
      courseRow.subject = {
        code: row.subjectCode,
        description: row.subjectDescription,
        schoolCode: row.schoolCode,
        schoolDescription: row.schoolDescription,
      };
    }

    return courseRow;
  });

  if (includeSet.has('sectionsSummary')) {
    attachSectionsSummary(db, mappedRows);
  }

  if (includeSet.has('sections')) {
    const previewLimit = typeof query.sectionsLimit === 'number' ? query.sectionsLimit : SECTION_PREVIEW_LIMIT;
    attachSectionsPreview(db, mappedRows, previewLimit, sectionFilter);
  }

  return { data: mappedRows, total };
}

function buildInClause(column: string, values: string[], binder: SqlBinder) {
  if (!values.length) {
    return '1 = 1';
  }
  const placeholders = values.map((value) => binder.bind(value));
  return `${column} IN (${placeholders.join(', ')})`;
}

function normalizeStringList(
  values: string[] | string | undefined,
  transform: (value: string) => string | undefined,
): string[] {
  if (!values) {
    return [];
  }
  const asArray = Array.isArray(values)
    ? values
    : values
        .split(',')
        .map((token) => token.trim())
        .filter(Boolean);
  if (!asArray.length) {
    return [];
  }
  const dedup = new Set<string>();
  for (const raw of asArray) {
    const transformed = transform(raw.trim());
    if (!transformed) {
      continue;
    }
    dedup.add(transformed);
  }
  return Array.from(dedup);
}

function normalizeSubjectCodes(values: string[] | undefined) {
  return normalizeStringList(values, (value) => {
    if (!value) {
      return undefined;
    }
    const normalized = value.replace(/\s+/g, '');
    const tokens = normalized.split(':');
    const last = tokens[tokens.length - 1]?.trim();
    if (!last) {
      return undefined;
    }
    return last.toUpperCase();
  });
}

function normalizeDeliveryList(values: string[] | undefined) {
  return normalizeStringList(values, (value) => {
    const normalized = value.toLowerCase();
    return VALID_DELIVERY.has(normalized) ? normalized : undefined;
  });
}

function normalizeExamCodeList(values: string[] | undefined) {
  return normalizeStringList(values, (value) => {
    const cleaned = value.replace(/[^a-zA-Z]/g, '').toUpperCase();
    return cleaned ? cleaned : undefined;
  });
}

function normalizeLevelFilters(values: string[] | string | undefined): string[] {
  if (!values) {
    return [];
  }
  const dedup = new Set<string>();
  const list = Array.isArray(values)
    ? values
    : values
        .split(',')
        .map((token) => token.trim())
        .filter(Boolean);

  for (const raw of list) {
    const upper = raw.toUpperCase();
    if (upper === 'UG' || upper === 'U' || upper === 'UNDERGRADUATE') {
      dedup.add('UG');
      dedup.add('U');
      continue;
    }
    if (upper === 'GR' || upper === 'G' || upper === 'GRADUATE' || upper === 'GRAD') {
      dedup.add('GR');
      dedup.add('G');
      continue;
    }
    if (upper === 'N/A' || upper === 'NA' || upper === 'OTHER') {
      dedup.add('N/A');
      continue;
    }
    dedup.add(upper);
  }

  return Array.from(dedup);
}

function buildFtsQuery(input: string | undefined) {
  if (!input) {
    return undefined;
  }
  const tokens = input
    .split(/\s+/)
    .map((token) => token.replace(/[^a-zA-Z0-9]/g, '').toLowerCase())
    .filter(Boolean)
    .map((token) => `${token}*`);
  if (!tokens.length) {
    return undefined;
  }
  return tokens.join(' ');
}

function buildMeetingMask(values: string[] | undefined) {
  if (!values || !values.length) {
    return undefined;
  }
  let mask = 0;
  for (const raw of values) {
    const cleaned = raw.replace(/[^a-zA-Z]/g, '').toUpperCase();
    let index = 0;
    while (index < cleaned.length) {
      const pair = cleaned.slice(index, index + 2);
      if (pair === 'TH' || pair === 'SA' || pair === 'SU') {
        mask |= DAY_BITMASK[pair];
        index += 2;
        continue;
      }
      const char = cleaned[index];
      if (char === 'R') {
        mask |= DAY_BITMASK.TH;
      } else if (char === 'U') {
        mask |= DAY_BITMASK.SU;
      } else if (char === 'S') {
        mask |= DAY_BITMASK.SA;
      } else if (DAY_BITMASK[char]) {
        mask |= DAY_BITMASK[char];
      }
      index += 1;
    }
  }
  return mask || undefined;
}

function resolveSort(sortBy: CoursesQuery['sortBy'], sortDir: CoursesQuery['sortDir']) {
  const sortKey = (sortBy && sortBy in SORT_COLUMNS ? sortBy : 'subject') as keyof typeof SORT_COLUMNS;
  const defaultDir = SORT_DEFAULT_DIRECTION[sortKey] ?? 'asc';
  const requested = typeof sortDir === 'string' ? sortDir : defaultDir;
  const normalizedDir = requested.toLowerCase() === 'desc' ? 'DESC' : 'ASC';
  return {
    column: SORT_COLUMNS[sortKey],
    direction: normalizedDir,
  };
}

function safeJson(payload: string | null) {
  if (!payload) {
    return null;
  }
  try {
    return JSON.parse(payload);
  } catch {
    return null;
  }
}

function buildIncludeSet(include: string[] | undefined) {
  if (!include || !include.length) {
    return new Set<CourseInclude>();
  }
  const normalized = include.map((value) => value.trim()).filter((value): value is CourseInclude => {
    return (VALID_INCLUDES as string[]).includes(value);
  });
  return new Set(normalized);
}

function attachSectionsSummary(db: Database.Database, rows: CourseSearchRow[]) {
  if (!rows.length) {
    return;
  }

  const courseIds = rows.map((row) => row.courseId);
  const placeholders = courseIds.map((_, index) => `@course${index}`);
  const params = Object.fromEntries(courseIds.map((id, index) => [`course${index}`, id]));

  const summaryRows = db
    .prepare(`
      SELECT
        s.course_id AS courseId,
        COUNT(*) AS totalSections,
        SUM(CASE WHEN s.is_open = 1 THEN 1 ELSE 0 END) AS openSections,
        GROUP_CONCAT(DISTINCT s.delivery_method) AS deliveryMethods
      FROM sections s
      WHERE s.course_id IN (${placeholders.join(', ')})
      GROUP BY s.course_id
    `)
    .all(params) as Array<{
      courseId: number;
      totalSections: number | null;
      openSections: number | null;
      deliveryMethods: string | null;
    }>;

  const summaryMap = new Map(summaryRows.map((row) => [row.courseId, row]));
  for (const row of rows) {
    const summary = summaryMap.get(row.courseId);
    const delivery = summary?.deliveryMethods
      ? Array.from(new Set(summary.deliveryMethods.split(',').filter(Boolean)))
      : [];
    row.sectionsSummary = {
      total: summary?.totalSections ?? 0,
      open: summary?.openSections ?? 0,
      deliveryMethods: delivery,
    };
  }
}

function attachSectionsPreview(
  db: Database.Database,
  rows: CourseSearchRow[],
  limitPerCourse: number,
  filter?: SectionFilterInput,
) {
  if (!rows.length) {
    return;
  }

  const safeLimit = Math.max(1, Math.min(limitPerCourse || SECTION_PREVIEW_LIMIT, 300));
  const courseIds = rows.map((row) => row.courseId);
  const placeholders = courseIds.map((_, index) => `@course${index}`);
  const baseParams = Object.fromEntries(courseIds.map((id, index) => [`course${index}`, id]));

  const binder = new SqlBinder();
  const filterClauses: string[] = [];

  if (filter?.deliveries?.length) {
    filterClauses.push(buildInClause('s.delivery_method', filter.deliveries, binder));
  }
  if (filter?.examCodes?.length) {
    filterClauses.push(buildInClause('upper(s.exam_code)', filter.examCodes, binder));
  }

  const outsideDayMask = typeof filter?.meetingMask === 'number' ? DAY_MASK_ALL & ~filter.meetingMask : undefined;
  if (outsideDayMask !== undefined) {
    filterClauses.push(`
      NOT EXISTS (
        SELECT 1
        FROM section_meetings sm_all
        WHERE sm_all.section_id = s.section_id
          AND sm_all.week_mask IS NOT NULL
          AND sm_all.week_mask > 0
          AND (sm_all.week_mask & ${binder.bind(outsideDayMask)}) != 0
      )
    `);
  }

  const meetingClauses: string[] = [];
  const meetingLocationClauses: string[] = [];
  if (filter?.meetingMask !== undefined) {
    meetingClauses.push(`sm.week_mask IS NOT NULL AND sm.week_mask > 0`);
    meetingClauses.push(`(sm.week_mask & ${binder.bind(outsideDayMask ?? 0)}) = 0`);
  }
  if (filter?.meetingStart !== undefined) {
    meetingClauses.push(`sm.start_minutes IS NOT NULL AND sm.start_minutes >= ${binder.bind(filter.meetingStart)}`);
  }
  if (filter?.meetingEnd !== undefined) {
    meetingClauses.push(`sm.end_minutes IS NOT NULL AND sm.end_minutes <= ${binder.bind(filter.meetingEnd)}`);
  }
  if (filter?.meetingCampuses?.length) {
    meetingLocationClauses.push(
      buildInClause('upper(COALESCE(sm.campus_abbrev, sm.campus_location_code))', filter.meetingCampuses, binder),
    );
  }

  if (meetingClauses.length || meetingLocationClauses.length) {
    const combined = [...meetingClauses, ...meetingLocationClauses].join(' AND ');
    filterClauses.push(`
      EXISTS (
        SELECT 1
        FROM section_meetings sm
        WHERE sm.section_id = s.section_id
        ${combined ? ` AND ${combined}` : ''}
      )
    `);
  }

  const whereClause = filterClauses.length ? ` AND ${filterClauses.join(' AND ')}` : '';
  const params = { ...baseParams, ...binder.cloneParams() };

  const rawSections = db
    .prepare(`
      SELECT DISTINCT
        s.section_id AS sectionId,
        s.course_id AS courseId,
        s.index_number AS indexNumber,
        s.section_number AS sectionNumber,
        s.open_status AS openStatus,
        s.is_open AS isOpen,
        s.delivery_method AS deliveryMethod,
        s.campus_code AS campusCode,
        s.instructors_text AS instructorsText,
        s.meeting_mode_summary AS meetingModeSummary
      FROM sections s
      WHERE s.course_id IN (${placeholders.join(', ')})${whereClause}
      ORDER BY s.course_id ASC, s.is_open DESC, s.index_number ASC
    `)
    .all(params) as Array<{
      sectionId: number;
      courseId: number;
      indexNumber: string;
      sectionNumber: string | null;
      openStatus: string | null;
      isOpen: number | null;
      deliveryMethod: string | null;
      campusCode: string | null;
      instructorsText: string | null;
      meetingModeSummary: string | null;
    }>;

  const sectionsByCourse = new Map<number, CourseSectionRow[]>();
  for (const section of rawSections) {
    const list = sectionsByCourse.get(section.courseId) ?? [];
    if (list.length >= safeLimit) {
      sectionsByCourse.set(section.courseId, list);
      continue;
    }
    list.push({
      sectionId: section.sectionId,
      indexNumber: section.indexNumber,
      sectionNumber: section.sectionNumber,
      openStatus: section.openStatus,
      isOpen: section.isOpen === 1,
      deliveryMethod: section.deliveryMethod,
      campusCode: section.campusCode,
      instructorsText: section.instructorsText ?? undefined,
      meetingModeSummary: section.meetingModeSummary ?? undefined,
      meetings: [],
    });
    sectionsByCourse.set(section.courseId, list);
  }

  const selectedSectionIds = Array.from(sectionsByCourse.values()).flatMap((list) => list.map((item) => item.sectionId));
  if (selectedSectionIds.length) {
    const meetingPlaceholders = selectedSectionIds.map((_, index) => `@section${index}`);
    const meetingParams = Object.fromEntries(selectedSectionIds.map((id, index) => [`section${index}`, id]));
    const meetingRows = db
      .prepare(`
        SELECT
          sm.section_id AS sectionId,
          sm.meeting_day AS meetingDay,
          sm.start_minutes AS startMinutes,
          sm.end_minutes AS endMinutes,
          sm.campus_abbrev AS meetingCampus,
          sm.campus_location_code AS meetingCampusCode,
          sm.campus_location_desc AS meetingCampusDesc,
          sm.building_code AS buildingCode,
          sm.room_number AS roomNumber
        FROM section_meetings sm
        WHERE sm.section_id IN (${meetingPlaceholders.join(', ')})
      `)
      .all(meetingParams) as Array<{
      sectionId: number;
      meetingDay: string | null;
      startMinutes: number | null;
      endMinutes: number | null;
      meetingCampus: string | null;
      meetingCampusCode: string | null;
      meetingCampusDesc: string | null;
      buildingCode: string | null;
      roomNumber: string | null;
    }>;

    const meetingMap = new Map<number, CourseSectionMeeting[]>();
    for (const meeting of meetingRows) {
      const list = meetingMap.get(meeting.sectionId) ?? [];
      list.push({
        meetingDay: meeting.meetingDay,
        startMinutes: meeting.startMinutes,
        endMinutes: meeting.endMinutes,
        campus: meeting.meetingCampus ?? meeting.meetingCampusCode ?? meeting.meetingCampusDesc,
        building: meeting.buildingCode,
        room: meeting.roomNumber,
      });
      meetingMap.set(meeting.sectionId, list);
    }

    for (const [, sectionList] of sectionsByCourse) {
      sectionList.forEach((section) => {
        section.meetings = meetingMap.get(section.sectionId) ?? [];
        if (!section.meetingCampus && section.meetings.length) {
          section.meetingCampus = section.meetings.find((meeting) => meeting.campus)?.campus ?? null;
        }
      });
    }
  }

  for (const row of rows) {
    row.sections = sectionsByCourse.get(row.courseId) ?? [];
  }
}
