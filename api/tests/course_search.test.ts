import test from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';

import type { CoursesQuery } from '../src/routes/courses.js';
import { executeCourseSearch } from '../src/queries/course_search.js';

const DAY_MASK = {
  M: 1,
  T: 2,
  W: 4,
  TH: 8,
  F: 16,
  SA: 32,
  SU: 64,
} as const;

test('filters by core code and open status', () => {
  const { db, courses } = seedFixtures();
  const query = buildQuery({
    coreCode: ['WC'],
    hasOpenSection: true,
  });

  const result = executeCourseSearch(db, query);
  assert.equal(result.total, 1);
  assert.equal(result.data.length, 1);
  assert.equal(result.data[0]?.courseId, courses.intro);
  db.close();
});

test('meetingDays uses subset matching with delivery/time window', () => {
  const { db, courses } = seedFixtures();
  const noMondayOnly = executeCourseSearch(db, buildQuery({ meetingDays: ['M'] }));
  assert.equal(noMondayOnly.total, 0);

  const thursdayMatch = executeCourseSearch(
    db,
    buildQuery({
      delivery: ['online'],
      meetingDays: ['TH'],
      meetingStart: 780,
      meetingEnd: 900,
    }),
  );

  assert.equal(thursdayMatch.total, 1);
  assert.equal(thursdayMatch.data[0]?.courseId, courses.systems);
  db.close();
});

test('filters by exam code and returns matching sections', () => {
  const { db, courses } = seedFixtures();
  const query = buildQuery({
    examCode: ['A'],
    include: ['sections'],
  });

  const result = executeCourseSearch(db, query);
  assert.equal(result.total, 1);
  const course = result.data[0];
  assert.ok(course);
  assert.equal(course.courseId, courses.intro);
  assert.equal(course.sections?.length, 1);
  assert.equal(course.sections?.[0]?.indexNumber, '10001');
  db.close();
});

test('supports FTS q search and pagination with ordering', () => {
  const { db, courses } = seedFixtures();
  const common = {
    campus: ['NB', 'NW'],
    subject: ['198', '640'],
    sortBy: 'sectionsOpen' as CoursesQuery['sortBy'],
    pageSize: 1,
  } satisfies Partial<CoursesQuery>;

  const pageOne = executeCourseSearch(
    db,
    buildQuery({ ...common, page: 1, q: 'algorithms' }),
  );
  assert.equal(pageOne.total, 1);
  assert.equal(pageOne.data[0]?.courseId, courses.algorithms);

  const ordered = executeCourseSearch(
    db,
    buildQuery({ ...common, q: undefined, page: 1 }),
  );
  const secondPage = executeCourseSearch(
    db,
    buildQuery({ ...common, q: undefined, page: 2 }),
  );
  assert.equal(ordered.total, 3);
  assert.equal(secondPage.total, 3);
  assert.notEqual(ordered.data[0]?.courseId, secondPage.data[0]?.courseId);
  db.close();
});

test('includes optional summaries and subject metadata', () => {
  const { db, courses } = seedFixtures();
  const query = buildQuery({ include: ['sectionsSummary', 'subjects'] });
  const result = executeCourseSearch(db, query);
  const course = result.data.find((row) => row.courseId === courses.intro);
  assert.ok(course);
  assert.ok(course.subject);
  assert.equal(course.subject?.description, 'Computer Science');
  assert.ok(course.sectionsSummary);
  assert.equal(course.sectionsSummary?.total, 2);
  assert.equal(course.sectionsSummary?.open, 1);
  assert.deepEqual(course.sectionsSummary?.deliveryMethods.sort(), ['hybrid', 'in_person'].sort());
  db.close();
});

function buildQuery(overrides: Partial<CoursesQuery> = {}): CoursesQuery {
  return {
    term: '20241',
    campus: ['NB'],
    subject: ['198'],
    page: 1,
    pageSize: 20,
    ...overrides,
  } as CoursesQuery;
}

function seedFixtures() {
  const db = setupDb();
  insertSubject(db, '198', 'Computer Science');
  insertSubject(db, '640', 'Mathematics');

  const intro = insertCourse(db, {
    subjectCode: '198',
    courseNumber: '111',
    title: 'Intro to CS',
    coreJson: JSON.stringify(['WC']),
    openSectionsCount: 1,
    hasOpenSections: 1,
    creditsMin: 4,
    creditsMax: 4,
  });
  insertCoreAttribute(db, intro, 'WC');
  seedFtsRow(db, { courseId: intro, campusCode: 'NB', termId: '20241', document: 'Intro computing fundamentals' });
  const introSectionOpen = insertSection(db, {
    courseId: intro,
    indexNumber: '10001',
    isOpen: 1,
    deliveryMethod: 'in_person',
    instructorsText: 'Jamie Lin',
    examCode: 'A',
  });
  insertMeeting(db, introSectionOpen, ['M', 'W'], 600, 690);
  const introSectionClosed = insertSection(db, {
    courseId: intro,
    indexNumber: '10002',
    isOpen: 0,
    deliveryMethod: 'hybrid',
    instructorsText: 'Jamie Lin',
    examCode: 'B',
  });
  insertMeeting(db, introSectionClosed, ['F'], 900, 1000);

  const systems = insertCourse(db, {
    subjectCode: '198',
    courseNumber: '205',
    title: 'Systems Programming',
    coreJson: JSON.stringify(['QL']),
    openSectionsCount: 0,
    hasOpenSections: 0,
  });
  insertCoreAttribute(db, systems, 'QL');
  seedFtsRow(db, { courseId: systems, campusCode: 'NB', termId: '20241', document: 'Systems and networking' });
  const systemsSection = insertSection(db, {
    courseId: systems,
    indexNumber: '20001',
    isOpen: 0,
    deliveryMethod: 'online',
    instructorsText: 'Ann Smith',
    examCode: 'C',
    specialPermissionAdd: 'SP',
  });
  insertMeeting(db, systemsSection, ['TH'], 800, 900);

  const algorithms = insertCourse(db, {
    campusCode: 'NW',
    subjectCode: '640',
    courseNumber: '152',
    title: 'Algorithms',
    openSectionsCount: 1,
    hasOpenSections: 1,
  });
  seedFtsRow(db, { courseId: algorithms, campusCode: 'NW', termId: '20241', document: 'Advanced algorithms and data structures' });
  const algorithmsSection = insertSection(db, {
    courseId: algorithms,
    campusCode: 'NW',
    subjectCode: '640',
    indexNumber: '30001',
    isOpen: 1,
    deliveryMethod: 'in_person',
    instructorsText: 'Brian Doe',
    examCode: 'D',
  });
  insertMeeting(db, algorithmsSection, ['T'], 540, 600);

  return { db, courses: { intro, systems, algorithms } };
}

function setupDb() {
  const db = new Database(':memory:');
  db.exec(`
    CREATE TABLE courses (
      course_id INTEGER PRIMARY KEY AUTOINCREMENT,
      term_id TEXT NOT NULL,
      campus_code TEXT NOT NULL,
      subject_code TEXT NOT NULL,
      course_number TEXT NOT NULL,
      course_string TEXT,
      title TEXT NOT NULL,
      expanded_title TEXT,
      level TEXT,
      credits_min REAL,
      credits_max REAL,
      credits_display TEXT,
      core_json TEXT,
      has_open_sections INTEGER DEFAULT 0,
      open_sections_count INTEGER DEFAULT 0,
      updated_at TEXT,
      prereq_plain TEXT,
      created_at TEXT DEFAULT CURRENT_TIMESTAMP
    );

    CREATE TABLE course_core_attributes (
      course_id INTEGER,
      term_id TEXT,
      core_code TEXT
    );

    CREATE TABLE subjects (
      subject_code TEXT PRIMARY KEY,
      subject_description TEXT,
      school_code TEXT,
      school_description TEXT
    );

    CREATE TABLE sections (
      section_id INTEGER PRIMARY KEY AUTOINCREMENT,
      course_id INTEGER NOT NULL,
      term_id TEXT NOT NULL,
      campus_code TEXT NOT NULL,
      subject_code TEXT NOT NULL,
      section_number TEXT,
      index_number TEXT,
      open_status TEXT,
      delivery_method TEXT,
      is_open INTEGER DEFAULT 0,
      instructors_text TEXT,
      meeting_mode_summary TEXT,
      exam_code TEXT,
      exam_code_text TEXT,
      special_permission_add_code TEXT,
      special_permission_drop_code TEXT
    );

    CREATE TABLE section_meetings (
      meeting_id INTEGER PRIMARY KEY AUTOINCREMENT,
      section_id INTEGER NOT NULL,
      week_mask INTEGER,
      start_minutes INTEGER,
      end_minutes INTEGER,
      meeting_day TEXT,
      campus_abbrev TEXT,
      campus_location_code TEXT,
      campus_location_desc TEXT,
      building_code TEXT,
      room_number TEXT
    );

    CREATE VIRTUAL TABLE course_search_fts USING fts5(term_id, campus_code, course_id, section_id, document);
  `);
  return db;
}

function insertSubject(db: Database.Database, code: string, description: string) {
  db.prepare(
    `INSERT INTO subjects(subject_code, subject_description, school_code, school_description)
     VALUES (?, ?, '01', 'Arts and Sciences')
     ON CONFLICT(subject_code) DO UPDATE SET subject_description=excluded.subject_description`,
  ).run(code, description);
}

function insertCourse(
  db: Database.Database,
  {
    termId = '20241',
    campusCode = 'NB',
    subjectCode = '198',
    courseNumber = '000',
    courseString = null,
    title = 'Course',
    expandedTitle = null,
    level = 'UG',
    creditsMin = 3,
    creditsMax = 3,
    creditsDisplay = '3',
    coreJson = null,
    hasOpenSections = 0,
    openSectionsCount = 0,
  }: {
    termId?: string;
    campusCode?: string;
    subjectCode?: string;
    courseNumber?: string;
    courseString?: string | null;
    title?: string;
    expandedTitle?: string | null;
    level?: string | null;
    creditsMin?: number | null;
    creditsMax?: number | null;
    creditsDisplay?: string | null;
    coreJson?: string | null;
    hasOpenSections?: number;
    openSectionsCount?: number;
  },
) {
  const info = db
    .prepare(
      `INSERT INTO courses (
        term_id, campus_code, subject_code, course_number, course_string, title, expanded_title,
        level, credits_min, credits_max, credits_display, core_json, has_open_sections,
        open_sections_count, updated_at, prereq_plain
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now'), NULL)
      `,
    )
    .run(
      termId,
      campusCode,
      subjectCode,
      courseNumber,
      courseString,
      title,
      expandedTitle,
      level,
      creditsMin,
      creditsMax,
      creditsDisplay,
      coreJson,
      hasOpenSections,
      openSectionsCount,
    );
  return Number(info.lastInsertRowid);
}

function insertCoreAttribute(db: Database.Database, courseId: number, coreCode: string) {
  db.prepare('INSERT INTO course_core_attributes(course_id, term_id, core_code) VALUES (?, ?, ?)').run(
    courseId,
    '20241',
    coreCode,
  );
}

function insertSection(
  db: Database.Database,
  {
    courseId,
    termId = '20241',
    campusCode = 'NB',
    subjectCode = '198',
    sectionNumber = '01',
    indexNumber = '10000',
    openStatus = null,
    deliveryMethod = 'in_person',
    isOpen = 0,
    instructorsText = 'TBA',
    examCode = null,
    specialPermissionAdd = null,
    specialPermissionDrop = null,
  }: {
    courseId: number;
    termId?: string;
    campusCode?: string;
    subjectCode?: string;
    sectionNumber?: string;
    indexNumber?: string;
    openStatus?: string | null;
    deliveryMethod?: string;
    isOpen?: number;
    instructorsText?: string;
    examCode?: string | null;
    specialPermissionAdd?: string | null;
    specialPermissionDrop?: string | null;
  },
) {
  const info = db
    .prepare(
      `INSERT INTO sections (
        course_id, term_id, campus_code, subject_code, section_number, index_number,
        open_status, delivery_method, is_open, instructors_text, exam_code, special_permission_add_code, special_permission_drop_code
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      `,
    )
    .run(
      courseId,
      termId,
      campusCode,
      subjectCode,
      sectionNumber,
      indexNumber,
      openStatus ?? (isOpen ? 'OPEN' : 'CLOSED'),
      deliveryMethod,
      isOpen,
      instructorsText,
      examCode,
      specialPermissionAdd,
      specialPermissionDrop,
    );
  return Number(info.lastInsertRowid);
}

function insertMeeting(
  db: Database.Database,
  sectionId: number,
  days: Array<keyof typeof DAY_MASK>,
  start: number,
  end: number,
) {
  const mask = days.reduce((value, day) => value | DAY_MASK[day], 0);
  const meetingDay = days.join('');
  db.prepare(
    'INSERT INTO section_meetings(section_id, week_mask, start_minutes, end_minutes, meeting_day, campus_abbrev, campus_location_code, campus_location_desc, building_code, room_number) VALUES (?, ?, ?, ?, ?, NULL, NULL, NULL, NULL, NULL)',
  ).run(sectionId, mask, start, end, meetingDay);
}

function seedFtsRow(
  db: Database.Database,
  {
    courseId,
    campusCode,
    termId,
    document,
  }: {
    courseId: number;
    campusCode: string;
    termId: string;
    document: string;
  },
) {
  db.prepare('INSERT INTO course_search_fts(term_id, campus_code, course_id, section_id, document) VALUES (?, ?, ?, NULL, ?)').run(
    termId,
    campusCode,
    courseId,
    document,
  );
}
