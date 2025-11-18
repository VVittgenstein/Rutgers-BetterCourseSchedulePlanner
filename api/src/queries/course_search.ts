import type Database from 'better-sqlite3';

import type { CoursesQuery } from '../routes/courses.js';

type CourseInclude = 'sectionsSummary' | 'subjects';

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
}

export interface CourseSearchResult {
  data: CourseSearchRow[];
  total: number;
}

const VALID_DELIVERY = new Set(['in_person', 'online', 'hybrid']);
const VALID_INCLUDES: CourseInclude[] = ['sectionsSummary', 'subjects'];

const DAY_BITMASK: Record<string, number> = {
  M: 1,
  T: 2,
  W: 4,
  TH: 8,
  F: 16,
  SA: 32,
  SU: 64,
};

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

export function executeCourseSearch(db: Database.Database, query: CoursesQuery): CourseSearchResult {
  const binder = new SqlBinder();
  const filters: string[] = [];
  filters.push(`c.term_id = ${binder.bind(query.term)}`);

  const campusFilter = normalizeStringList(query.campus, (value) => value.toUpperCase());
  if (campusFilter.length) {
    filters.push(buildInClause('c.campus_code', campusFilter, binder));
  }

  const subjects = normalizeSubjectCodes(query.subject);
  if (subjects.length) {
    filters.push(buildInClause('c.subject_code', subjects, binder));
  }

  const levels = normalizeStringList(query.level, (value) => value.toUpperCase());
  if (levels.length) {
    filters.push(buildInClause('c.level', levels, binder));
  }

  if (query.courseNumber?.trim()) {
    filters.push(`c.course_number = ${binder.bind(query.courseNumber.trim())}`);
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
          AND ${buildInClause('cca.core_code', coreCodes, binder)}
      )
    `);
  }

  if (typeof query.creditsMin === 'number') {
    filters.push(`c.credits_min IS NOT NULL AND c.credits_min >= ${binder.bind(query.creditsMin)}`);
  }

  if (typeof query.creditsMax === 'number') {
    filters.push(`c.credits_max IS NOT NULL AND c.credits_max <= ${binder.bind(query.creditsMax)}`);
  }

  const deliveries = normalizeDeliveryList(query.delivery);
  if (deliveries.length) {
    filters.push(`
      EXISTS (
        SELECT 1
        FROM sections s_delivery
        WHERE s_delivery.course_id = c.course_id
          AND ${buildInClause('s_delivery.delivery_method', deliveries, binder)}
      )
    `);
  }

  if (typeof query.hasOpenSection === 'boolean') {
    filters.push(`c.has_open_sections = ${binder.bind(query.hasOpenSection ? 1 : 0)}`);
  }

  const meetingMask = buildMeetingMask(query.meetingDays);
  const meetingStart = typeof query.meetingStart === 'number' ? query.meetingStart : undefined;
  const meetingEnd = typeof query.meetingEnd === 'number' ? query.meetingEnd : undefined;

  if (meetingMask !== undefined || meetingStart !== undefined || meetingEnd !== undefined) {
    const meetingClauses: string[] = [];
    if (meetingMask !== undefined) {
      meetingClauses.push(`sm.week_mask IS NOT NULL AND (sm.week_mask & ${binder.bind(meetingMask)}) != 0`);
    }
    if (meetingStart !== undefined) {
      meetingClauses.push(`sm.start_minutes IS NOT NULL AND sm.start_minutes >= ${binder.bind(meetingStart)}`);
    }
    if (meetingEnd !== undefined) {
      meetingClauses.push(`sm.end_minutes IS NOT NULL AND sm.end_minutes <= ${binder.bind(meetingEnd)}`);
    }

    const meetingFilter = meetingClauses.length ? ` AND ${meetingClauses.join(' AND ')}` : '';
    filters.push(`
      EXISTS (
        SELECT 1
        FROM sections s_meeting
        JOIN section_meetings sm ON sm.section_id = s_meeting.section_id
        WHERE s_meeting.course_id = c.course_id${meetingFilter}
      )
    `);
  }

  if (query.instructor?.trim()) {
    const instructorNeedle = query.instructor.trim().toLowerCase();
    filters.push(`
      EXISTS (
        SELECT 1
        FROM sections s_instructor
        WHERE s_instructor.course_id = c.course_id
          AND s_instructor.instructors_text IS NOT NULL
          AND s_instructor.instructors_text <> ''
          AND instr(lower(s_instructor.instructors_text), ${binder.bind(instructorNeedle)}) > 0
      )
    `);
  }

  if (typeof query.requiresPermission === 'boolean') {
    if (query.requiresPermission) {
      filters.push(`
        EXISTS (
          SELECT 1
          FROM sections s_perm
          WHERE s_perm.course_id = c.course_id
            AND (
              (s_perm.special_permission_add_code IS NOT NULL AND s_perm.special_permission_add_code <> '') OR
              (s_perm.special_permission_drop_code IS NOT NULL AND s_perm.special_permission_drop_code <> '')
            )
        )
      `);
    } else {
      filters.push(`
        EXISTS (
          SELECT 1
          FROM sections s_perm
          WHERE s_perm.course_id = c.course_id
            AND (
              (s_perm.special_permission_add_code IS NULL OR s_perm.special_permission_add_code = '') AND
              (s_perm.special_permission_drop_code IS NULL OR s_perm.special_permission_drop_code = '')
            )
        )
      `);
    }
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
  values: string[] | undefined,
  transform: (value: string) => string | undefined,
): string[] {
  if (!values || !values.length) {
    return [];
  }
  const dedup = new Set<string>();
  for (const raw of values) {
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
