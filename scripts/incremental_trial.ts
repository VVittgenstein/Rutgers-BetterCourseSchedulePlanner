#!/usr/bin/env tsx
import crypto from 'node:crypto';
import { performance } from 'node:perf_hooks';
import { decodeSemester, performProbe, SOCRequestError, type SemesterParts } from './soc_api_client.js';

type HashableRecord = Record<string, unknown>;

interface CLIOptions {
  term: string;
  campus: string;
  subjects: string[];
  timeoutMs: number;
}

interface DiffSet {
  added: string[];
  removed: string[];
  updated: string[];
}

interface NormalizedCourse {
  key: string;
  hash: string;
  summary: HashableRecord;
  sections: NormalizedSection[];
}

interface NormalizedSection {
  key: string;
  hash: string;
  courseKey: string;
  summary: HashableRecord;
}

interface SnapshotMaps {
  courses: Map<string, NormalizedCourse>;
  sections: Map<string, NormalizedSection>;
}

interface SubjectTrialResult {
  subject: string;
  totalMs: number;
  fetchMs: number;
  diffMs: number;
  courseCount: number;
  sectionCount: number;
  openSectionEstimate: number;
  courseDiff: DiffSet;
  sectionDiff: DiffSet;
  simulationNotes: string[];
}

class CLIError extends Error {}

function parseArgs(argv: string[]): CLIOptions {
  const defaults: CLIOptions = {
    term: '12024',
    campus: 'NB',
    subjects: ['198', '640', '750'],
    timeoutMs: 15000
  };

  const opts: CLIOptions = { ...defaults };

  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (!token.startsWith('--')) {
      throw new CLIError(`Unexpected argument: ${token}`);
    }
    const key = token.slice(2);
    const value = argv[i + 1];
    switch (key) {
      case 'term':
        if (!value) throw new CLIError('Missing value for --term');
        opts.term = value;
        i += 1;
        break;
      case 'campus':
        if (!value) throw new CLIError('Missing value for --campus');
        opts.campus = value.toUpperCase();
        i += 1;
        break;
      case 'subjects':
        if (!value) throw new CLIError('Missing value for --subjects');
        opts.subjects = value
          .split(',')
          .map((item) => item.trim().toUpperCase())
          .filter(Boolean);
        if (opts.subjects.length === 0) {
          throw new CLIError('At least one subject code is required.');
        }
        i += 1;
        break;
      case 'timeout':
        if (!value) throw new CLIError('Missing value for --timeout');
        opts.timeoutMs = Number.parseInt(value, 10);
        if (Number.isNaN(opts.timeoutMs) || opts.timeoutMs <= 0) {
          throw new CLIError('--timeout must be a positive integer');
        }
        i += 1;
        break;
      case 'help':
        showUsage();
        process.exit(0);
      default:
        throw new CLIError(`Unknown flag: --${key}`);
    }
  }

  return opts;
}

function showUsage(): void {
  console.log(`Incremental strategy dry-run

Usage:
  npm run data:incremental-trial -- [--term 12024] [--campus NB] [--subjects 198,640,750]

Options:
  --term       Semester (e.g. 12024, 92024, FA2024). Default: 12024
  --campus     Campus code (NB, NK, CM ...). Default: NB
  --subjects   Comma list of subject codes. Default: 198,640,750
  --timeout    Request timeout in milliseconds. Default: 15000
  --help       Show this message
`);
}

function hashPayload(payload: HashableRecord): string {
  return crypto.createHash('sha1').update(stableStringify(payload)).digest('hex');
}

function stableStringify(value: unknown): string {
  if (value === null || typeof value !== 'object') {
    return JSON.stringify(value);
  }
  if (Array.isArray(value)) {
    return `[${value.map((item) => stableStringify(item)).join(',')}]`;
  }
  const entries = Object.entries(value as Record<string, unknown>).sort(([a], [b]) =>
    a.localeCompare(b),
  );
  return `{${entries.map(([k, v]) => `${JSON.stringify(k)}:${stableStringify(v)}`).join(',')}}`;
}

function ensureString(value: unknown): string {
  if (typeof value === 'string') return value;
  if (value === undefined || value === null) return '';
  return String(value);
}

function ensureNullable(value: unknown): string | null {
  const str = ensureString(value).trim();
  return str.length === 0 ? null : str;
}

function toStringArray(value: unknown, extractor: (item: unknown) => string = ensureString): string[] {
  if (!Array.isArray(value)) return [];
  return Array.from(
    new Set(
      value
        .map((item) => extractor(item).trim())
        .filter((item) => item.length > 0),
    ),
  ).sort();
}

function normalizeMeeting(meeting: Record<string, unknown>): Record<string, string> {
  const day = ensureString(meeting.meetingDay ?? meeting.day);
  const start = ensureString(meeting.startTimeMilitary ?? meeting.startTime);
  const end = ensureString(meeting.endTimeMilitary ?? meeting.endTime);
  const campusAbbrev = ensureString(meeting.campusAbbrev ?? meeting.campusLocation);
  const campusName = ensureString(meeting.campusName ?? meeting.buildingCode);
  const mode = ensureString(meeting.meetingModeDesc ?? meeting.meetingModeCode);
  return {
    day,
    start,
    end,
    campusAbbrev,
    campusName,
    mode
  };
}

function normalizeCourses(body: unknown, subject: string): NormalizedCourse[] {
  if (!Array.isArray(body)) return [];
  const normalizedSubject = subject.toUpperCase();
  const matches = body.filter(
    (course) =>
      typeof course === 'object' &&
      course !== null &&
      (course as Record<string, unknown>).subject?.toString().toUpperCase() === normalizedSubject,
  ) as Record<string, unknown>[];

  return matches
    .map((course) => normalizeCourse(course, normalizedSubject))
    .filter((course): course is NormalizedCourse => course !== null);
}

function normalizeCourse(
  course: Record<string, unknown>,
  subject: string,
): NormalizedCourse | null {
  const courseNumber = ensureString(course.courseNumber ?? course.catalogNumber).trim();
  if (!courseNumber) {
    return null;
  }
  const key = `${subject}-${courseNumber}`;
  const campusLocations = Array.isArray(course.campusLocations)
    ? toStringArray(course.campusLocations, (loc) =>
        ensureString((loc as Record<string, unknown>).description ?? loc),
      )
    : toStringArray([course.campusCode ?? course.campus ?? '']);
  const summary: HashableRecord = {
    subject,
    courseNumber,
    title: ensureString(course.title),
    expandedTitle: ensureNullable(course.expandedTitle),
    level: ensureNullable(course.level),
    creditsDisplay: ensureNullable(course.credits ?? (course.creditsObject as Record<string, unknown>)?.description),
    campusLocations,
    openSections: typeof course.openSections === 'number' ? course.openSections : undefined,
    hasCoreAttribute: Array.isArray(course.coreCodes) && course.coreCodes.length > 0,
    prereqNotes: ensureNullable(course.preReqNotes),
    subjectNotes: ensureNullable(course.subjectNotes),
    courseNotes: ensureNullable(course.courseNotes),
    synopsisUrl: ensureNullable(course.synopsisUrl ?? course.syllabusUrl),
    tags: toStringArray(course.tags),
  };

  const sectionsRaw = Array.isArray(course.sections) ? course.sections : [];
  const sections = sectionsRaw
    .map((section) => normalizeSection(section as Record<string, unknown>, key))
    .filter((section): section is NormalizedSection => section !== null);

  const normalized: NormalizedCourse = {
    key,
    summary,
    hash: hashPayload(summary),
    sections,
  };
  return normalized;
}

function normalizeSection(
  section: Record<string, unknown>,
  courseKey: string,
): NormalizedSection | null {
  const indexNumber = ensureString(section.index ?? section.indexNumber).trim();
  if (!indexNumber) {
    return null;
  }

  const instructors = Array.isArray(section.instructors)
    ? toStringArray(section.instructors, (item) =>
        ensureString((item as Record<string, unknown>).name ?? item),
      )
    : [];

  const comments = Array.isArray(section.comments)
    ? toStringArray(section.comments, (item) =>
        ensureString((item as Record<string, unknown>).comments ?? item),
      )
    : [];

  const meetings = Array.isArray(section.meetingTimes)
    ? (section.meetingTimes as Record<string, unknown>[])
        .map((meeting) => normalizeMeeting(meeting))
        .sort((a, b) => {
          const dayDiff = a.day.localeCompare(b.day);
          if (dayDiff !== 0) return dayDiff;
          return a.start.localeCompare(b.start);
        })
    : [];

  const summary: HashableRecord = {
    courseKey,
    sectionNumber: ensureString(section.number ?? section.sectionNumber),
    indexNumber,
    openStatus: ensureString(section.openStatusText ?? section.openStatus ?? ''),
    isOpen: Boolean(section.openStatus),
    instructors,
    instructorsText: ensureNullable(section.instructorsText),
    sectionNotes: ensureNullable(section.sectionNotes),
    subtitle: ensureNullable(section.subtitle),
    eligibility: ensureNullable(section.sectionEligibility),
    campus: ensureNullable(section.campus ?? section.campusCode),
    deliveryMethod: ensureNullable(section.meetingModeSummary ?? section.meetingMode ?? section.deliveryMode),
    majors: toStringArray(section.majors, (item) =>
      ensureString((item as Record<string, unknown>).code ?? item),
    ),
    minors: toStringArray(section.minors, (item) =>
      ensureString((item as Record<string, unknown>).code ?? item),
    ),
    honorPrograms: toStringArray(section.honorPrograms, (item) =>
      ensureString((item as Record<string, unknown>).code ?? item),
    ),
    meetingModes: meetings,
    comments,
  };

  return {
    key: indexNumber,
    courseKey,
    summary,
    hash: hashPayload(summary),
  };
}

function buildSnapshot(courses: NormalizedCourse[]): SnapshotMaps {
  const courseMap = new Map<string, NormalizedCourse>();
  const sectionMap = new Map<string, NormalizedSection>();

  for (const course of courses) {
    courseMap.set(course.key, course);
    for (const section of course.sections) {
      sectionMap.set(section.key, section);
    }
  }
  return { courses: courseMap, sections: sectionMap };
}

function ghostCourse(baseKey: string, suffix: number): NormalizedCourse {
  const summary: HashableRecord = {
    subject: 'GHOST',
    courseNumber: `${baseKey}-LEGACY-${suffix}`,
    title: `Legacy snapshot for ${baseKey}`,
    deleted: true,
  };
  return {
    key: `GHOST-${baseKey}-${suffix}`,
    summary,
    hash: hashPayload(summary),
    sections: [],
  };
}

function ghostSection(courseKey: string, suffix: number): NormalizedSection {
  const summary: HashableRecord = {
    courseKey,
    sectionNumber: `9${suffix}`,
    indexNumber: `999${suffix}`,
    openStatus: 'CLOSED',
    isOpen: false,
    ghost: true,
  };
  return {
    key: `GHOST-${courseKey}-${suffix}`,
    courseKey,
    summary,
    hash: hashPayload(summary),
  };
}

function simulatePreviousSnapshot(
  courses: NormalizedCourse[],
  scenarioIndex: number,
): { snapshot: SnapshotMaps; notes: string[] } {
  const cloned = courses.map((course) => ({
    key: course.key,
    hash: course.hash,
    summary: JSON.parse(JSON.stringify(course.summary)) as HashableRecord,
    sections: course.sections.map((section) => ({
      key: section.key,
      hash: section.hash,
      courseKey: section.courseKey,
      summary: JSON.parse(JSON.stringify(section.summary)) as HashableRecord,
    })),
  }));

  const notes: string[] = [];
  if (cloned.length === 0) {
    notes.push('No courses returned for subject; snapshot is empty.');
    return { snapshot: buildSnapshot(cloned), notes };
  }

  const scenario = scenarioIndex % 3;
  const firstCourse = cloned[0];

  switch (scenario) {
    case 0: {
      if (cloned.length > 1) {
        const removed = cloned.pop();
        if (removed) {
          notes.push(`Removed ${removed.key} from previous snapshot to simulate a new course addition.`);
        }
      }
      if (firstCourse) {
        firstCourse.summary.title = `${firstCourse.summary.title ?? 'Course'} (legacy)`;
        firstCourse.hash = hashPayload(firstCourse.summary);
        notes.push(`Marked ${firstCourse.key} as legacy title to simulate a course update.`);
        if (firstCourse.sections.length > 0) {
          const primary = firstCourse.sections[0];
          primary.summary.openStatus =
            primary.summary.openStatus === 'OPEN' ? 'CLOSED' : 'OPEN';
          primary.hash = hashPayload(primary.summary);
          notes.push(`Flipped open status for section ${primary.key}.`);
          if (firstCourse.sections.length > 1) {
            const removedSection = firstCourse.sections.pop();
            if (removedSection) {
              notes.push(
                `Dropped section ${removedSection.key} from previous snapshot to simulate section insertion.`,
              );
            }
          }
        }
      }
      break;
    }
    case 1: {
      const ghost = ghostCourse(firstCourse.key, scenarioIndex);
      cloned.push(ghost);
      notes.push(`Inserted ghost course ${ghost.key} to simulate a later deletion.`);
      if (firstCourse.sections.length > 0) {
        const removed = firstCourse.sections.shift();
        if (removed) {
          notes.push(
            `Removed earliest section ${removed.key} from previous snapshot to simulate an added section in the new payload.`,
          );
        }
      }
      if (firstCourse.sections.length > 0) {
        const mutateTarget = firstCourse.sections[0];
        mutateTarget.summary.comments = [
          ...(Array.isArray(mutateTarget.summary.comments)
            ? (mutateTarget.summary.comments as string[])
            : []),
          'legacy annotation',
        ];
        mutateTarget.hash = hashPayload(mutateTarget.summary);
        notes.push(`Annotated section ${mutateTarget.key} comments to force a section update.`);
      }
      break;
    }
    case 2: {
      if (firstCourse.sections.length > 0) {
        const primary = firstCourse.sections[0];
        primary.summary.meetingModes = [
          ...(Array.isArray(primary.summary.meetingModes)
            ? (primary.summary.meetingModes as Record<string, string>[])
            : []),
          {
            day: 'HY',
            start: '0000',
            end: '0000',
            campusAbbrev: 'ASY',
            campusName: 'Asynchronous',
            mode: 'ONLINE',
          },
        ];
        primary.hash = hashPayload(primary.summary);
        notes.push(`Extended meeting schedule for section ${primary.key} to mimic meeting edits.`);
      }
      const ghost = ghostSection(firstCourse.key, scenarioIndex);
      firstCourse.sections.push(ghost);
      notes.push(`Added ghost section ${ghost.key} so new payload treats it as deleted.`);
      if (cloned.length > 1) {
        const removedCourse = cloned.shift();
        if (removedCourse) {
          notes.push(`Dropped ${removedCourse.key} entirely to simulate course addition.`);
        }
      }
      break;
    }
    default:
      break;
  }

  return { snapshot: buildSnapshot(cloned), notes };
}

function diffMaps(prev: Map<string, NormalizedCourse | NormalizedSection>, next: Map<string, NormalizedCourse | NormalizedSection>): DiffSet {
  const added: string[] = [];
  const updated: string[] = [];
  const removed: string[] = [];

  for (const [key, nextValue] of next.entries()) {
    const prevValue = prev.get(key);
    if (!prevValue) {
      added.push(key);
      continue;
    }
    if (prevValue.hash !== nextValue.hash) {
      updated.push(key);
    }
  }

  for (const key of prev.keys()) {
    if (!next.has(key)) {
      removed.push(key);
    }
  }

  return { added, updated, removed };
}

async function runSubjectTrial(
  subject: string,
  semester: SemesterParts,
  campus: string,
  timeoutMs: number,
  scenarioIndex: number,
): Promise<SubjectTrialResult> {
  const started = performance.now();
  const requestStarted = performance.now();
  const probe = await performProbe(
    {
      campus,
      subject,
      endpoint: 'courses',
      timeoutMs,
    },
    semester,
  );
  const fetchMs = performance.now() - requestStarted;
  const normalized = normalizeCourses(probe.body, subject);
  const nextSnapshot = buildSnapshot(normalized);
  const diffStarted = performance.now();
  const { snapshot: prevSnapshot, notes } = simulatePreviousSnapshot(normalized, scenarioIndex);
  const courseDiff = diffMaps(prevSnapshot.courses, nextSnapshot.courses);
  const sectionDiff = diffMaps(prevSnapshot.sections, nextSnapshot.sections);
  const diffMs = performance.now() - diffStarted;
  const totalMs = performance.now() - started;
  const sectionValues = Array.from(nextSnapshot.sections.values());
  const openSectionEstimate = sectionValues.filter((section) =>
    String(section.summary.openStatus ?? '').toUpperCase().includes('OPEN'),
  ).length;

  return {
    subject,
    totalMs,
    fetchMs,
    diffMs,
    courseCount: nextSnapshot.courses.size,
    sectionCount: nextSnapshot.sections.size,
    openSectionEstimate,
    courseDiff,
    sectionDiff,
    simulationNotes: notes,
  };
}

function printSubjectResult(result: SubjectTrialResult): void {
  console.log(`\nSubject ${result.subject}`);
  console.log(
    `  snapshot: courses=${result.courseCount} sections=${result.sectionCount} (est. open sections=${result.openSectionEstimate})`,
  );
  console.log(
    `  timings: fetch=${result.fetchMs.toFixed(1)} ms diff=${result.diffMs.toFixed(
      1,
    )} ms total=${result.totalMs.toFixed(1)} ms`,
  );
  console.log(
    `  Δ courses: +${result.courseDiff.added.length} / -${result.courseDiff.removed.length} / ~${result.courseDiff.updated.length}`,
  );
  console.log(
    `  Δ sections: +${result.sectionDiff.added.length} / -${result.sectionDiff.removed.length} / ~${result.sectionDiff.updated.length}`,
  );
  if (result.simulationNotes.length > 0) {
    console.log('  simulation:');
    result.simulationNotes.forEach((note) => console.log(`    • ${note}`));
  }
}

function printSummary(results: SubjectTrialResult[]): void {
  const totalCourses = results.reduce((sum, item) => sum + item.courseCount, 0);
  const totalSections = results.reduce((sum, item) => sum + item.sectionCount, 0);
  const totalOpen = results.reduce((sum, item) => sum + item.openSectionEstimate, 0);
  const combinedCourses = {
    added: results.reduce((sum, item) => sum + item.courseDiff.added.length, 0),
    removed: results.reduce((sum, item) => sum + item.courseDiff.removed.length, 0),
    updated: results.reduce((sum, item) => sum + item.courseDiff.updated.length, 0),
  };
  const combinedSections = {
    added: results.reduce((sum, item) => sum + item.sectionDiff.added.length, 0),
    removed: results.reduce((sum, item) => sum + item.sectionDiff.removed.length, 0),
    updated: results.reduce((sum, item) => sum + item.sectionDiff.updated.length, 0),
  };
  const totalRuntime = results.reduce((sum, item) => sum + item.totalMs, 0);
  console.log('\nAggregate summary');
  console.log(
    `  processed subjects=${results.length} courses=${totalCourses} sections=${totalSections} (~${totalOpen} open)`,
  );
  console.log(
    `  Δ courses: +${combinedCourses.added} / -${combinedCourses.removed} / ~${combinedCourses.updated}`,
  );
  console.log(
    `  Δ sections: +${combinedSections.added} / -${combinedSections.removed} / ~${combinedSections.updated}`,
  );
  console.log(`  total runtime ~${totalRuntime.toFixed(1)} ms`);
}

async function main(): Promise<void> {
  try {
    const opts = parseArgs(process.argv.slice(2));
    if (opts.subjects.length === 0) {
      throw new CLIError('Please provide at least one subject code via --subjects.');
    }
    let semester: SemesterParts;
    try {
      semester = decodeSemester(opts.term);
    } catch (error) {
      throw new CLIError((error as Error).message);
    }
    const results: SubjectTrialResult[] = [];
    for (let i = 0; i < opts.subjects.length; i += 1) {
      const subject = opts.subjects[i];
      try {
        const result = await runSubjectTrial(subject, semester, opts.campus, opts.timeoutMs, i);
        results.push(result);
        printSubjectResult(result);
      } catch (error) {
        if (error instanceof SOCRequestError) {
          console.error(
            `Subject ${subject} failed (${error.requestId}): ${error.message} ${
              error.retryHint ?? ''
            }`,
          );
        } else if (error instanceof Error) {
          console.error(`Subject ${subject} failed: ${error.message}`);
        } else {
          console.error(`Subject ${subject} failed due to unknown error.`);
        }
      }
    }
    if (results.length > 0) {
      printSummary(results);
    } else {
      console.log('No successful subject trials were recorded.');
    }
  } catch (error) {
    if (error instanceof CLIError) {
      console.error(`Argument error: ${error.message}`);
      process.exit(1);
    }
    if (error instanceof Error) {
      console.error(`Unexpected error: ${error.message}`);
    } else {
      console.error('Unexpected fatal error.');
    }
    process.exit(1);
  }
}

main();
