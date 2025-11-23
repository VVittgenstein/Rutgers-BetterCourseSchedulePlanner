import type Database from 'better-sqlite3';

export interface FiltersDictionaryResult {
  terms: Array<{ id: string; display: string; active?: boolean }>;
  campuses: Array<{ code: string; display: string; region?: string | null }>;
  campusLocations: Array<{ code: string; description: string; campus?: string | null }>;
  subjects: Array<{ code: string; description: string; school?: string | null; campus?: string | null }>;
  coreCodes: Array<{ code: string; description?: string | null }>;
  levels: string[];
  deliveryMethods: string[];
  instructors: Array<{ id: string; name: string }>;
}

const FALLBACK_CORE_CODES: Array<{ code: string; description: string }> = [
  { code: 'AHO', description: 'Arts and Humanities' },
  { code: 'AHP', description: 'Arts and Humanities' },
  { code: 'AHQ', description: 'Arts and Humanities' },
  { code: 'AHR', description: 'Arts and Humanities' },
  { code: 'CCD', description: 'Contemporary Challenges: Diversity & Difference' },
  { code: 'CCO', description: 'Contemporary Challenges: Our Common Future' },
  { code: 'HST', description: 'Historical Analysis' },
  { code: 'SCL', description: 'Social & Behavioral Sciences' },
  { code: 'NS', description: 'Natural Sciences' },
  { code: 'QQ', description: 'Quantitative & Formal Reasoning' },
  { code: 'QR', description: 'Quantitative Reasoning' },
  { code: 'WCD', description: 'Writing and Communication' },
  { code: 'WCR', description: 'Writing and Communication' },
  { code: 'WC', description: 'Writing and Communication' },
  { code: 'W', description: 'Writing Intensive' },
  { code: 'ITR', description: 'Information Technology & Research' },
  { code: 'CE', description: 'CE' },
  { code: 'ECN', description: 'ECN' },
  { code: 'GVT', description: 'GVT' },
  { code: 'SOEHS', description: 'SOEHS' },
];

export function fetchFiltersDictionary(db: Database.Database): FiltersDictionaryResult {
  const now = Date.now();
  const terms = safeAll(
    () =>
      db
        .prepare(
          `
          SELECT term_id AS id, display_name AS display, start_on AS start, end_on AS "end"
          FROM terms
          ORDER BY start DESC, id DESC
          LIMIT 24
        `,
        )
        .all() as Array<{ id: string; display: string | null; start?: string | null; end?: string | null }>,
  ).map((row) => ({
    id: row.id,
    display: row.display ?? row.id,
    active: isWithinRange(row.start, row.end, now),
  }));

  const campuses = safeAll(
    () =>
      db
        .prepare(
          `
          SELECT campus_code AS code, display_name AS display, region
          FROM campuses
          ORDER BY display_name COLLATE NOCASE ASC
        `,
        )
        .all() as Array<{ code: string; display: string | null; region?: string | null }>,
  ).map((row) => ({
    code: row.code,
    display: row.display ?? row.code,
    region: row.region ?? null,
  }));

  const campusLocations = dedupe(
    safeAll(
      () =>
        db
          .prepare(
            `
            SELECT location_code AS code, COALESCE(location_desc, location_code) AS description, campus_code AS campus
            FROM course_campus_locations
            WHERE location_code IS NOT NULL AND location_code <> ''
            GROUP BY code, description, campus
            ORDER BY description COLLATE NOCASE ASC, code ASC
          `,
          )
          .all() as Array<{ code: string | null; description: string | null; campus?: string | null }>,
    )
      .map((row) => ({
        code: (row.code ?? '').toUpperCase(),
        description: row.description ?? row.code ?? '',
        campus: row.campus ?? null,
      }))
      .filter((row) => row.code.length > 0),
    (row) => `${row.code}|${row.campus ?? ''}`,
  );

  const subjects = safeAll(
    () =>
      db
        .prepare(
          `
          SELECT subject_code AS code, subject_description AS description, school_description AS school, campus_code AS campus
          FROM subjects
          WHERE active IS NULL OR active = 1
          ORDER BY code ASC
          LIMIT 800
        `,
        )
        .all() as Array<{
        code: string;
        description: string | null;
        school?: string | null;
        campus?: string | null;
      }>,
  ).map((row) => ({
    code: row.code,
    description: row.description ?? row.code,
    school: row.school ?? null,
    campus: row.campus ?? null,
  }));

  const coreCodesFromDb = dedupe(
    safeAll(
      () =>
        db
          .prepare(
            `
            SELECT DISTINCT core_code AS code, metadata
            FROM course_core_attributes
            ORDER BY core_code ASC
          `,
          )
          .all() as Array<{ code: string; metadata?: string | null }>,
    ).map((row) => {
      const code = row.code;
      return {
        code,
        description: extractCoreDescription(row.metadata) ?? findFallbackCoreDescription(code) ?? code,
      };
    }),
    (entry) => entry.code,
  );
  const coreCodes = coreCodesFromDb.length ? coreCodesFromDb : fallbackCoreCodes();

  const levels = dedupe(
    safeAll(
      () =>
        db
          .prepare(
            `
            SELECT DISTINCT upper(level) AS level
            FROM courses
            WHERE level IS NOT NULL AND level <> ''
          `,
          )
          .all() as Array<{ level: string | null }>,
    )
      .map((row) => normalizeLevelCode(row.level))
      .filter((level): level is string => Boolean(level)),
    (level) => level,
  );

  const deliveryMethods = dedupe(
    safeAll(
      () =>
        db
          .prepare(
            `
            SELECT DISTINCT lower(delivery_method) AS method
            FROM sections
            WHERE delivery_method IS NOT NULL AND delivery_method <> ''
          `,
          )
          .all() as Array<{ method: string | null }>,
    )
      .map((row) => row.method?.toLowerCase())
      .filter((method): method is string => Boolean(method)),
    (method) => method,
  );

  const instructors = dedupe(
    safeAll(
      () =>
        db
          .prepare(
            `
            SELECT instructor_id AS id, full_name AS name
            FROM instructors
            WHERE full_name IS NOT NULL AND full_name <> ''
            ORDER BY name COLLATE NOCASE ASC
            LIMIT 400
          `,
          )
          .all() as Array<{ id: string; name: string | null }>,
    )
      .map((row) => ({ id: row.id, name: row.name ?? row.id }))
      .filter((row) => row.name.trim().length > 0),
    (row) => row.id,
  );

  const populated = {
    terms: terms.length ? terms : fallbackTerms(),
    campuses: campuses.length ? campuses : fallbackCampuses(),
    campusLocations: campusLocations.length ? campusLocations : fallbackCampusLocations(),
    subjects: subjects.length ? subjects : fallbackSubjects(),
    coreCodes,
    levels: levels.length ? levels : ['UG', 'GR'],
    deliveryMethods: deliveryMethods.length ? deliveryMethods : ['in_person', 'online', 'hybrid'],
    instructors,
  };

  return populated;
}

function safeAll<T>(fn: () => T[]): T[] {
  try {
    return fn();
  } catch {
    return [];
  }
}

function dedupe<T>(values: T[], key: (value: T) => string): T[] {
  const seen = new Set<string>();
  const result: T[] = [];
  for (const value of values) {
    const k = key(value);
    if (seen.has(k)) continue;
    seen.add(k);
    result.push(value);
  }
  return result;
}

function isWithinRange(start: string | null | undefined, end: string | null | undefined, nowMs: number): boolean {
  const startMs = start ? Date.parse(start) : null;
  const endMs = end ? Date.parse(end) : null;
  if (startMs && endMs) return nowMs >= startMs && nowMs <= endMs;
  if (startMs) return nowMs >= startMs;
  if (endMs) return nowMs <= endMs;
  return true;
}

function extractCoreDescription(metadata: string | null | undefined) {
  if (!metadata) return null;
  try {
    const parsed = JSON.parse(metadata) as { description?: string; title?: string };
    return parsed.description ?? parsed.title ?? null;
  } catch {
    return null;
  }
}

function normalizeLevelCode(level: string | null | undefined): string | null {
  if (!level) return null;
  const upper = level.toUpperCase();
  if (upper === 'U' || upper === 'UG' || upper === 'UNDERGRADUATE') return 'UG';
  if (upper === 'G' || upper === 'GR' || upper === 'GRADUATE') return 'GR';
  if (upper === 'N/A' || upper === 'NA' || upper === 'OTHER') return 'N/A';
  return upper;
}

function findFallbackCoreDescription(code: string | null | undefined): string | undefined {
  if (!code) return undefined;
  const normalized = code.toUpperCase();
  const match = FALLBACK_CORE_CODES.find((entry) => entry.code === normalized);
  return match?.description;
}

function fallbackCoreCodes(): FiltersDictionaryResult['coreCodes'] {
  return FALLBACK_CORE_CODES.map((entry) => ({
    code: entry.code,
    description: entry.description,
  }));
}

function fallbackTerms(): FiltersDictionaryResult['terms'] {
  return [
    { id: '2024FA', display: 'Fall 2024', active: true },
    { id: '2025SP', display: 'Spring 2025', active: false },
  ];
}

function fallbackCampuses(): FiltersDictionaryResult['campuses'] {
  return [
    { code: 'NB', display: 'New Brunswick', region: 'Central' },
    { code: 'NWK', display: 'Newark', region: 'North' },
    { code: 'CAM', display: 'Camden', region: 'South' },
  ];
}

function fallbackCampusLocations(): FiltersDictionaryResult['campusLocations'] {
  return [
    { code: '1', description: 'College Avenue', campus: 'NB' },
    { code: '2', description: 'Busch', campus: 'NB' },
    { code: '3', description: 'Livingston', campus: 'NB' },
    { code: '4', description: 'Cook/Douglass', campus: 'NB' },
    { code: '5', description: 'Downtown New Brunswick', campus: 'NB' },
    { code: 'Z', description: 'Off campus', campus: 'NB' },
    { code: 'S', description: 'Study Abroad', campus: 'NB' },
    { code: 'O', description: 'O', campus: 'NB' },
    { code: 'NA', description: 'N/A', campus: 'NB' },
  ];
}

function fallbackSubjects(): FiltersDictionaryResult['subjects'] {
  return [
    { code: '01:198', description: 'Computer Science', school: 'SAS', campus: 'NB' },
    { code: '01:640', description: 'Mathematics', school: 'SAS', campus: 'NB' },
    { code: '01:960', description: 'Statistics', school: 'SAS', campus: 'NB' },
    { code: '14:332', description: 'Electrical Engineering', school: 'SOE', campus: 'NB' },
    { code: '21:198', description: 'CS (Newark)', school: 'NCS', campus: 'NWK' },
    { code: '50:198', description: 'CS (Camden)', school: 'CCAS', campus: 'CAM' },
  ];
}
