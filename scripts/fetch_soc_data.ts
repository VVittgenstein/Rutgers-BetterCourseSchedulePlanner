#!/usr/bin/env tsx
import Database from 'better-sqlite3';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import { mkdir, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { performance } from 'node:perf_hooks';

import {
  decodeSemester,
  performProbe,
  type SemesterParts,
  SOCRequestError
} from './soc_api_client.js';
import {
  normalizeCoursePayload,
  type NormalizedCourse,
  type NormalizedCourseMap,
  type NormalizedSection,
  type NormalizedMeeting
} from './soc_normalizer.js';
import { hashPayload } from './soc_normalizer.js';

type PipelineMode = 'full-init' | 'incremental';

type DB = Database.Database;

interface CLIOptions {
  configPath?: string;
  modeOverride?: PipelineMode;
  termFilter?: string[];
  campusFilter?: string[];
  subjectFilter?: string[];
  subjectAllOverride: boolean;
  maxWorkers?: number;
  resumePath?: string;
  dryRun: boolean;
  showHelp: boolean;
}

interface PipelineConfig {
  runLabel?: string;
  defaultMode?: PipelineMode;
  sqliteFile?: string;
  stagingDir?: string;
  logDir?: string;
  rateLimitProfile?: string;
  concurrency?: Record<string, unknown>;
  retryPolicy?: Record<string, unknown>;
  targets?: PipelineTarget[];
  incremental?: Record<string, unknown>;
  fullInit?: Record<string, unknown>;
  summary?: Record<string, unknown>;
  safety?: Record<string, unknown>;
}

interface PipelineTarget {
  term: string;
  mode?: PipelineMode;
  campuses?: CampusConfig[];
  subjectBatchSize?: number;
  subjectRecencyMinutes?: number;
}

interface CampusConfig {
  code: string;
  subjects?: string[];
}

interface PlannedSlice {
  term: string;
  campus: string;
  mode: PipelineMode;
  subjects: string[];
}

interface SubjectStats {
  subject: string;
  coursesInserted: number;
  coursesUpdated: number;
  coursesDeleted: number;
  sectionsInserted: number;
  sectionsUpdated: number;
  sectionsDeleted: number;
}

interface SliceSummary {
  term: string;
  campus: string;
  mode: PipelineMode;
  durationMs: number;
  subjectsPlanned: number;
  subjectsCompleted: number;
  subjectStats: SubjectStats[];
  openSections?: OpenSectionsStats;
  errors: string[];
}

interface OpenSectionsStats {
  indexesSeen: number;
  markedOpen: number;
  markedClosed: number;
}

interface RunSummary {
  runLabel: string | null;
  startedAt: string;
  completedAt: string;
  totals: SubjectStats & { slicesFailed: number };
  sliceSummaries: SliceSummary[];
}

interface PreparedStatements {
  selectCourses: Database.Statement;
  insertCourse: Database.Statement;
  updateCourse: Database.Statement;
  deleteCourse: Database.Statement;
  selectSections: Database.Statement;
  insertSection: Database.Statement;
  updateSection: Database.Statement;
  deleteSection: Database.Statement;
  insertStatusEvent: Database.Statement;
  deleteMeetings: Database.Statement;
  insertMeeting: Database.Statement;
  upsertTerm: Database.Statement;
  upsertCampus: Database.Statement;
  upsertSubject: Database.Statement;
  selectSectionStatus: Database.Statement;
  updateSectionStatus: Database.Statement;
  insertOpenSnapshot: Database.Statement;
}

interface PipelineContext {
  db: DB;
  config: PipelineConfig;
  options: CLIOptions;
  sqliteFile: string;
  stagingDir: string;
  logDir: string;
  courseWorkerLimit: number;
  openSectionsEnabled: boolean;
  termCache: Map<string, SemesterParts>;
  fullInitPrepared: boolean;
  statements: PreparedStatements;
}

class CLIError extends Error {}

function parseArgs(argv: string[]): CLIOptions {
  const options: CLIOptions = {
    subjectAllOverride: false,
    dryRun: false,
    showHelp: false
  };

  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (!token.startsWith('--')) {
      throw new CLIError(`Unexpected argument: ${token}`);
    }
    const key = token.slice(2);
    switch (key) {
      case 'config':
        options.configPath = requireValue(argv, ++i, '--config');
        break;
      case 'mode':
        options.modeOverride = normalizeMode(requireValue(argv, ++i, '--mode'));
        break;
      case 'terms':
        options.termFilter = parseList(requireValue(argv, ++i, '--terms'));
        break;
      case 'campuses':
        options.campusFilter = parseList(requireValue(argv, ++i, '--campuses')).map((item) =>
          item.toUpperCase(),
        );
        break;
      case 'subjects': {
        const subjects = parseList(requireValue(argv, ++i, '--subjects')).map((subject) =>
          subject.toUpperCase(),
        );
        if (subjects.includes('ALL')) {
          options.subjectAllOverride = true;
        } else if (subjects.length > 0) {
          options.subjectFilter = subjects;
        }
        break;
      }
      case 'max-workers': {
        const raw = requireValue(argv, ++i, '--max-workers');
        const parsed = Number.parseInt(raw, 10);
        if (Number.isNaN(parsed) || parsed <= 0) {
          throw new CLIError('--max-workers must be a positive integer');
        }
        options.maxWorkers = parsed;
        break;
      }
      case 'resume':
        options.resumePath = requireValue(argv, ++i, '--resume');
        break;
      case 'dry-run':
        options.dryRun = true;
        break;
      case 'help':
        options.showHelp = true;
        break;
      default:
        throw new CLIError(`Unknown flag: --${key}`);
    }
  }

  return options;
}

function requireValue(argv: string[], index: number, flag: string): string {
  const value = argv[index];
  if (!value) {
    throw new CLIError(`Missing value for ${flag}`);
  }
  return value;
}

function normalizeMode(value: string): PipelineMode {
  const normalized = value.toLowerCase();
  if (normalized === 'full-init' || normalized === 'incremental') {
    return normalized;
  }
  throw new CLIError(`Invalid mode: ${value}`);
}

function parseList(value: string): string[] {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

function showUsage(): void {
  console.log(`SOC Fetch pipeline (planning stub)

Usage:
  npm run data:fetch -- --config path/to/config.json [options]

Options:
  --config <path>      Required. Pipeline config JSON.
  --mode <name>        Override mode (full-init | incremental).
  --terms <list>       Comma list of term IDs to limit work.
  --campuses <list>    Comma list of campus codes (NB,NK,...).
  --subjects <list>    Subject override. Use ALL to process every subject.
  --max-workers <n>    Caps worker pools for quick experiments.
  --resume <path>      Resume queue override.
  --dry-run            Plan only.
  --help               Show this message.
`);
}

async function loadPipelineConfig(configPath: string): Promise<PipelineConfig> {
  const resolved = path.resolve(configPath);
  const contents = await fs.promises.readFile(resolved, 'utf8');
  let parsed: unknown;
  try {
    parsed = JSON.parse(contents);
  } catch (error) {
    throw new CLIError(`Unable to parse config JSON (${(error as Error).message})`);
  }
  if (parsed === null || typeof parsed !== 'object') {
    throw new CLIError('Config must be a JSON object.');
  }
  const pipeline = parsed as PipelineConfig;
  if (!Array.isArray(pipeline.targets) || pipeline.targets.length === 0) {
    throw new CLIError('Config must include at least one entry in targets[].');
  }
  return pipeline;
}

function buildPlan(config: PipelineConfig, options: CLIOptions): PlannedSlice[] {
  const defaultMode: PipelineMode = normalizeMode(config.defaultMode ?? 'incremental');
  const termFilter = options.termFilter ? new Set(options.termFilter) : null;
  const campusFilter = options.campusFilter ? new Set(options.campusFilter) : null;
  const plan: PlannedSlice[] = [];

  for (const target of config.targets ?? []) {
    if (!target || typeof target !== 'object') continue;
    if (!target.term) {
      throw new CLIError('Each target must include a term field.');
    }
    if (termFilter && !termFilter.has(target.term)) continue;
    const mode = options.modeOverride ?? target.mode ?? defaultMode;
    const campuses = target.campuses ?? [];
    if (campuses.length === 0) {
      throw new CLIError(`Target ${target.term} must specify at least one campus.`);
    }
    for (const campus of campuses) {
      if (!campus.code) {
        throw new CLIError(`Campus entry under term ${target.term} is missing code.`);
      }
      const campusCode = campus.code.toUpperCase();
      if (campusFilter && !campusFilter.has(campusCode)) continue;
      const subjects = resolveSubjects(campus.subjects, options);
      plan.push({
        term: target.term,
        campus: campusCode,
        mode,
        subjects
      });
    }
  }

  if (plan.length === 0) {
    throw new CLIError('No work slices match the provided filters.');
  }

  return plan;
}

function resolveSubjects(subjects: string[] | undefined, options: CLIOptions): string[] {
  if (options.subjectAllOverride) {
    return ['ALL'];
  }
  if (options.subjectFilter && options.subjectFilter.length > 0) {
    return options.subjectFilter;
  }
  if (subjects && subjects.length > 0) {
    return subjects;
  }
  return ['ALL'];
}

function printPlan(plan: PlannedSlice[], config: PipelineConfig, options: CLIOptions): void {
  console.log('SOC fetch pipeline planner');
  if (config.runLabel) {
    console.log(`Run label: ${config.runLabel}`);
  }
  console.log(`SQLite file: ${config.sqliteFile ?? 'n/a'}`);
  console.log(`Staging dir: ${config.stagingDir ?? 'n/a'}`);
  console.log(`Log dir: ${config.logDir ?? 'n/a'}`);
  if (options.maxWorkers) {
    console.log(`Worker cap override: ${options.maxWorkers}`);
  }
  if (options.resumePath) {
    console.log(`Resume queue override: ${options.resumePath}`);
  }
  console.log('');
  console.log('Planned slices:');
  plan.forEach((slice, index) => {
    console.log(
      `  ${index + 1}. term=${slice.term} campus=${slice.campus} mode=${slice.mode} subjects=${slice.subjects.join(
        ',',
      )}`,
    );
  });
  console.log('');
}

async function stagePayload(
  ctx: PipelineContext,
  slice: PlannedSlice,
  target: 'courses' | 'openSections',
  subject: string,
  body: unknown
): Promise<void> {
  const dir = path.join(ctx.stagingDir, slice.term, slice.campus);
  await mkdir(dir, { recursive: true });
  const file = path.join(dir, `${target}-${subject}-${Date.now()}.json`);
  await writeFile(file, JSON.stringify(body, null, 2), 'utf8');
}

function expandSubjects(subjects: string[], map: NormalizedCourseMap): string[] {
  if (subjects.includes('ALL')) {
    return Array.from(map.keys()).sort();
  }
  return subjects.map((subject) => subject.toUpperCase());
}

function createStatements(db: DB): PreparedStatements {
  return {
    selectCourses: db.prepare(
      'SELECT course_id, course_number, source_hash FROM courses WHERE term_id = ? AND campus_code = ? AND subject_code = ?',
    ),
    insertCourse: db.prepare(`
      INSERT INTO courses (
        term_id, campus_code, subject_code, course_number, course_string, title, expanded_title,
        level, credits_min, credits_max, credits_display, core_json, has_core_attribute,
        prereq_html, prereq_plain, synopsis_url, course_notes, unit_notes, subject_notes,
        supplement_code, campus_locations_json, open_sections_count, has_open_sections,
        tags, search_vector, source_hash, source_payload, created_at, updated_at
      ) VALUES (
        ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
      )
    `),
    updateCourse: db.prepare(`
      UPDATE courses SET
        course_string = ?,
        title = ?,
        expanded_title = ?,
        level = ?,
        credits_min = ?,
        credits_max = ?,
        credits_display = ?,
        core_json = ?,
        has_core_attribute = ?,
        prereq_html = ?,
        prereq_plain = ?,
        synopsis_url = ?,
        course_notes = ?,
        unit_notes = ?,
        subject_notes = ?,
        supplement_code = ?,
        campus_locations_json = ?,
        open_sections_count = ?,
        has_open_sections = ?,
        tags = ?,
        search_vector = ?,
        source_hash = ?,
        source_payload = ?,
        updated_at = ?
      WHERE course_id = ?
    `),
    deleteCourse: db.prepare('DELETE FROM courses WHERE course_id = ?'),
    selectSections: db.prepare(
      'SELECT section_id, index_number, source_hash, open_status, is_open, open_status_updated_at FROM sections WHERE term_id = ? AND campus_code = ? AND subject_code = ?',
    ),
    insertSection: db.prepare(`
      INSERT INTO sections (
        course_id, term_id, campus_code, subject_code, section_number, index_number,
        open_status, is_open, open_status_updated_at, instructors_text, section_notes,
        comments_json, eligibility_text, open_to_text, majors_json, minors_json,
        honor_programs_json, section_course_type, exam_code, exam_code_text,
        special_permission_add_code, special_permission_add_desc,
        special_permission_drop_code, special_permission_drop_desc, printed,
        session_print_indicator, subtitle, meeting_mode_summary, delivery_method,
        has_meetings, source_hash, source_payload, created_at, updated_at
      ) VALUES (
        ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
      )
    `),
    updateSection: db.prepare(`
      UPDATE sections SET
        course_id = ?,
        section_number = ?,
        open_status = ?,
        is_open = ?,
        open_status_updated_at = ?,
        instructors_text = ?,
        section_notes = ?,
        comments_json = ?,
        eligibility_text = ?,
        open_to_text = ?,
        majors_json = ?,
        minors_json = ?,
        honor_programs_json = ?,
        section_course_type = ?,
        exam_code = ?,
        exam_code_text = ?,
        special_permission_add_code = ?,
        special_permission_add_desc = ?,
        special_permission_drop_code = ?,
        special_permission_drop_desc = ?,
        printed = ?,
        session_print_indicator = ?,
        subtitle = ?,
        meeting_mode_summary = ?,
        delivery_method = ?,
        has_meetings = ?,
        source_hash = ?,
        source_payload = ?,
        updated_at = ?
      WHERE section_id = ?
    `),
    deleteSection: db.prepare('DELETE FROM sections WHERE section_id = ?'),
    insertStatusEvent: db.prepare(`
      INSERT INTO section_status_events (
        section_id, previous_status, current_status, source, snapshot_term, snapshot_campus, snapshot_received_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?)
    `),
    deleteMeetings: db.prepare('DELETE FROM section_meetings WHERE section_id = ?'),
    insertMeeting: db.prepare(`
      INSERT INTO section_meetings (
        section_id, meeting_day, week_mask, start_time_label, end_time_label,
        start_minutes, end_minutes, meeting_mode_code, meeting_mode_desc,
        campus_abbrev, campus_location_code, campus_location_desc, building_code,
        room_number, pm_code, ba_class_hours, online_only, hash
      ) VALUES (
        ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
      )
    `),
    upsertTerm: db.prepare(`
      INSERT INTO terms (term_id, year, term_code, display_name)
      VALUES (?, ?, ?, ?)
      ON CONFLICT(term_id) DO UPDATE SET year = excluded.year, term_code = excluded.term_code, display_name = excluded.display_name
    `),
    upsertCampus: db.prepare(`
      INSERT INTO campuses (campus_code, display_name)
      VALUES (?, ?)
      ON CONFLICT(campus_code) DO UPDATE SET display_name = excluded.display_name
    `),
    upsertSubject: db.prepare(`
      INSERT INTO subjects (subject_code, school_code, school_description, subject_description, campus_code, active)
      VALUES (?, ?, ?, ?, ?, 1)
      ON CONFLICT(subject_code) DO UPDATE SET
        school_code = COALESCE(excluded.school_code, subjects.school_code),
        school_description = COALESCE(excluded.school_description, subjects.school_description),
        subject_description = COALESCE(excluded.subject_description, subjects.subject_description),
        campus_code = COALESCE(excluded.campus_code, subjects.campus_code),
        active = 1
    `),
    selectSectionStatus: db.prepare(
      'SELECT section_id, index_number, is_open, open_status FROM sections WHERE term_id = ? AND campus_code = ?',
    ),
    updateSectionStatus: db.prepare(
      'UPDATE sections SET is_open = ?, open_status = ?, open_status_updated_at = ?, updated_at = ? WHERE section_id = ?',
    ),
    insertOpenSnapshot: db.prepare(
      'INSERT INTO open_section_snapshots (term_id, campus_code, index_number, seen_open_at, source_hash) VALUES (?, ?, ?, ?, ?)',
    ),
  };
}

function toNullableNumber(value: number | null): number | null {
  if (typeof value === 'number' && Number.isFinite(value)) {
    return value;
  }
  return null;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function probeWithRetry(
  options: Parameters<typeof performProbe>[0],
  semester: SemesterParts,
  attempts = 3,
  baseBackoffMs = 2000
): Promise<ReturnType<typeof performProbe>> {
  let lastError: unknown;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      return await performProbe(options, semester);
    } catch (error) {
      lastError = error;
      const reason = error instanceof Error ? error.message : 'unknown error';
      if (attempt < attempts) {
        const backoffMs = baseBackoffMs * attempt;
        console.warn(`Probe ${options.endpoint} attempt ${attempt}/${attempts} failed (${reason}); retrying in ${backoffMs}ms...`);
        await sleep(backoffMs);
      }
    }
  }
  if (lastError instanceof Error) {
    throw lastError;
  }
  throw new Error(`Probe ${options.endpoint} failed after ${attempts} attempts`);
}

function ensureCleanWorktree(): void {
  try {
    const output = execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' });
    if (output.trim().length > 0) {
      throw new CLIError('Working tree is dirty. Commit or stash changes before running with requireCleanWorktree.');
    }
  } catch (error) {
    const err = error as NodeJS.ErrnoException;
    if (err.code === 'ENOENT') {
      return;
    }
    if (err instanceof CLIError) {
      throw err;
    }
    throw new CLIError('Unable to verify git status.');
  }
}

async function runPipeline(plan: PlannedSlice[], config: PipelineConfig, options: CLIOptions): Promise<void> {
  const sqliteFile = path.resolve(config.sqliteFile ?? path.join('data', 'local.db'));
  const stagingDir = path.resolve(config.stagingDir ?? path.join('data', 'staging'));
  const logDir = path.resolve(config.logDir ?? path.join('logs', 'fetch_runs'));
  await mkdir(path.dirname(sqliteFile), { recursive: true });
  await mkdir(stagingDir, { recursive: true });
  await mkdir(logDir, { recursive: true });

  if ((config.safety as { requireCleanWorktree?: boolean } | undefined)?.requireCleanWorktree) {
    ensureCleanWorktree();
  }

  const db = new Database(sqliteFile);
  db.pragma('journal_mode = WAL');
  db.pragma('foreign_keys = ON');

  const concurrency = (config.concurrency ?? {}) as Record<string, unknown>;
  const maxCourseWorkers = Number.parseInt(String(concurrency.maxCourseWorkers ?? 1), 10) || 1;
  const maxOpenSectionsWorkers = Number.parseInt(String(concurrency.maxOpenSectionsWorkers ?? 0), 10) || 0;
  const effectiveCourseWorkers = options.maxWorkers
    ? Math.max(1, Math.min(maxCourseWorkers, options.maxWorkers))
    : maxCourseWorkers;

  const ctx: PipelineContext = {
    db,
    config,
    options,
    sqliteFile,
    stagingDir,
    logDir,
    courseWorkerLimit: effectiveCourseWorkers,
    openSectionsEnabled: maxOpenSectionsWorkers > 0,
    termCache: new Map(),
    fullInitPrepared: false,
    statements: createStatements(db)
  };

  const startedAt = new Date().toISOString();
  const summaries: SliceSummary[] = [];

  try {
    for (const slice of plan) {
      const summary = await processSlice(ctx, slice);
      summaries.push(summary);
      if (summary.errors.length > 0) {
        throw new Error(`Slice ${slice.term}/${slice.campus} failed: ${summary.errors.join('; ')}`);
      }
    }
  } finally {
    db.close();
  }

  const completedAt = new Date().toISOString();
  const totals = aggregateTotals(summaries);
  const runSummary: RunSummary = {
    runLabel: config.runLabel ?? null,
    startedAt,
    completedAt,
    totals,
    sliceSummaries: summaries
  };
  await writeSummaryFiles(ctx, runSummary);
  console.log('Ingestion completed.');
}

function aggregateTotals(summaries: SliceSummary[]): SubjectStats & { slicesFailed: number } {
  const total: SubjectStats & { slicesFailed: number } = {
    subject: 'TOTAL',
    coursesInserted: 0,
    coursesUpdated: 0,
    coursesDeleted: 0,
    sectionsInserted: 0,
    sectionsUpdated: 0,
    sectionsDeleted: 0,
    slicesFailed: summaries.filter((slice) => slice.errors.length > 0).length
  };
  for (const slice of summaries) {
    for (const stats of slice.subjectStats) {
      total.coursesInserted += stats.coursesInserted;
      total.coursesUpdated += stats.coursesUpdated;
      total.coursesDeleted += stats.coursesDeleted;
      total.sectionsInserted += stats.sectionsInserted;
      total.sectionsUpdated += stats.sectionsUpdated;
      total.sectionsDeleted += stats.sectionsDeleted;
    }
  }
  return total;
}

async function writeSummaryFiles(ctx: PipelineContext, summary: RunSummary): Promise<void> {
  const summaryConfig = (ctx.config.summary ?? {}) as { writeJson?: string; writeText?: string };
  if (summaryConfig.writeJson) {
    const file = path.resolve(summaryConfig.writeJson);
    await mkdir(path.dirname(file), { recursive: true });
    await writeFile(file, JSON.stringify(summary, null, 2), 'utf8');
  }
  if (summaryConfig.writeText) {
    const file = path.resolve(summaryConfig.writeText);
    await mkdir(path.dirname(file), { recursive: true });
    const lines: string[] = [];
    lines.push(`Run label: ${summary.runLabel ?? 'n/a'}`);
    lines.push(`Started: ${summary.startedAt}`);
    lines.push(`Completed: ${summary.completedAt}`);
    lines.push(`Slices: ${summary.sliceSummaries.length}`);
    lines.push(
      `Courses Δ insert=${summary.totals.coursesInserted} update=${summary.totals.coursesUpdated} delete=${summary.totals.coursesDeleted}`,
    );
    lines.push(
      `Sections Δ insert=${summary.totals.sectionsInserted} update=${summary.totals.sectionsUpdated} delete=${summary.totals.sectionsDeleted}`,
    );
    await writeFile(file, lines.join('\n'), 'utf8');
  }
}

async function processSlice(ctx: PipelineContext, slice: PlannedSlice): Promise<SliceSummary> {
  console.log(`\n== term=${slice.term} campus=${slice.campus} mode=${slice.mode} ==`);
  if (slice.mode === 'full-init' && !ctx.fullInitPrepared) {
    prepareFullInit(ctx);
  }

  const summary: SliceSummary = {
    term: slice.term,
    campus: slice.campus,
    mode: slice.mode,
    durationMs: 0,
    subjectsPlanned: 0,
    subjectsCompleted: 0,
    subjectStats: [],
    errors: []
  };

  const started = performance.now();
  let semester = ctx.termCache.get(slice.term);
  if (!semester) {
    semester = decodeSemester(slice.term);
    ctx.termCache.set(slice.term, semester);
  }

  let coursesResponse: unknown;
  try {
    const response = await probeWithRetry({ campus: slice.campus, endpoint: 'courses' }, semester);
    coursesResponse = response.body;
    await stagePayload(ctx, slice, 'courses', 'all', response.body);
  } catch (error) {
    if (error instanceof SOCRequestError) {
      summary.errors.push(`courses.json failed (${error.requestId}): ${error.message}`);
    } else if (error instanceof Error) {
      summary.errors.push(`courses.json failed: ${error.message}`);
    } else {
      summary.errors.push('courses.json failed due to unknown error');
    }
    summary.durationMs = performance.now() - started;
    return summary;
  }

  const normalized = normalizeCoursePayload(coursesResponse);
  const subjects = expandSubjects(slice.subjects, normalized);
  summary.subjectsPlanned = subjects.length;

  for (const subject of subjects) {
    const courses = normalized.get(subject);
    if (!courses || courses.length === 0) {
      console.warn(`Subject ${subject} not present in payload for ${slice.term}/${slice.campus}.`);
      continue;
    }
    try {
      const stats = applySubjectBatch(ctx, slice, subject, courses);
      summary.subjectStats.push(stats);
      summary.subjectsCompleted += 1;
    } catch (error) {
      const message = error instanceof Error ? error.message : 'unknown error';
      summary.errors.push(`Subject ${subject} failed: ${message}`);
      break;
    }
  }

  if (ctx.openSectionsEnabled && summary.errors.length === 0) {
    try {
      const response = await probeWithRetry({ campus: slice.campus, endpoint: 'openSections' }, semester, 3, 3000);
      await stagePayload(ctx, slice, 'openSections', 'all', response.body);
      const stats = applyOpenSectionsSnapshot(ctx, slice, response.body);
      summary.openSections = stats;
    } catch (error) {
      if (error instanceof SOCRequestError) {
        summary.errors.push(`openSections failed (${error.requestId}): ${error.message}`);
      } else if (error instanceof Error) {
        summary.errors.push(`openSections failed: ${error.message}`);
      } else {
        summary.errors.push('openSections failed due to unknown error');
      }
    }
  }

  summary.durationMs = performance.now() - started;
  console.log(
    `Finished ${slice.term}/${slice.campus} in ${summary.durationMs.toFixed(0)} ms • subjects=${summary.subjectsCompleted}/${summary.subjectsPlanned}`,
  );
  return summary;
}

function prepareFullInit(ctx: PipelineContext): void {
  const fullInit = ctx.config.fullInit as { truncateTables?: string[] } | undefined;
  const tables = fullInit?.truncateTables;
  if (!tables || tables.length === 0) {
    ctx.fullInitPrepared = true;
    return;
  }
  const sanitized = tables.filter((table) => /^[a-zA-Z0-9_]+$/.test(table));
  const tx = ctx.db.transaction(() => {
    for (const table of sanitized) {
      ctx.db.prepare(`DELETE FROM ${table}`).run();
    }
  });
  tx();
  console.log(`Full-init: truncated tables ${sanitized.join(', ')}`);
  ctx.fullInitPrepared = true;
}

function applySubjectBatch(
  ctx: PipelineContext,
  slice: PlannedSlice,
  subject: string,
  courses: NormalizedCourse[]
): SubjectStats {
  const stats: SubjectStats = {
    subject,
    coursesInserted: 0,
    coursesUpdated: 0,
    coursesDeleted: 0,
    sectionsInserted: 0,
    sectionsUpdated: 0,
    sectionsDeleted: 0
  };
  const now = new Date().toISOString();
  const { statements: st } = ctx;

  const tx = ctx.db.transaction(() => {
    const existingCourses = new Map<string, { courseId: number; sourceHash: string }>(
      st
        .selectCourses
        .all(slice.term, slice.campus, subject)
        .map((row: { course_id: number; course_number: string; source_hash: string }) => [
          row.course_number,
          { courseId: row.course_id, sourceHash: row.source_hash }
        ]),
    );

    const existingSections = new Map<
      string,
      { sectionId: number; sourceHash: string; openStatus: string; isOpen: number; openStatusUpdatedAt: string | null }
    >(
      st
        .selectSections
        .all(slice.term, slice.campus, subject)
        .map(
          (
            row: {
              section_id: number;
              index_number: string;
              source_hash: string;
              open_status: string;
              is_open: number;
              open_status_updated_at: string | null;
            },
          ) => [
            row.index_number,
            {
              sectionId: row.section_id,
              sourceHash: row.source_hash,
              openStatus: row.open_status,
              isOpen: row.is_open,
              openStatusUpdatedAt: row.open_status_updated_at
            }
          ],
        ),
    );

    const seenCourseNumbers = new Set<string>();
    const courseIds = new Map<string, number>();
    const changedSections: Array<{ sectionId: number; section: NormalizedSection; statusChanged: boolean; previousStatus: string }>
      = [];

    for (const course of courses) {
      const courseNumber = course.record.courseNumber;
      if (seenCourseNumbers.has(courseNumber)) {
        console.warn(
          `Duplicate course ${slice.term}/${slice.campus} subject=${subject} course=${courseNumber} in courses.json; skipping duplicate entry.`,
        );
        continue;
      }
      seenCourseNumbers.add(courseNumber);
      ensureReferenceRows(ctx, slice, course);
      const row = existingCourses.get(course.record.courseNumber);
      const payload = JSON.stringify(course.raw);
      const coreJson = JSON.stringify((course.raw as Record<string, unknown>).coreCodes ?? course.record.coreCodes);
      const campusLocationsJson = JSON.stringify(course.record.campusLocations);
      const hasCore = course.record.coreCodes.length > 0 ? 1 : 0;
      const tags = course.record.tags.join(',');
      const creditsMin = toNullableNumber(course.record.creditsMin);
      const creditsMax = toNullableNumber(course.record.creditsMax);
      if (!row) {
        const result = st.insertCourse.run(
          slice.term,
          slice.campus,
          subject,
          course.record.courseNumber,
          course.record.courseString,
          course.record.title,
          course.record.expandedTitle,
          course.record.level,
          creditsMin,
          creditsMax,
          course.record.creditsDisplay,
          coreJson,
          hasCore,
          course.record.prereqHtml,
          course.record.prereqPlain,
          course.record.synopsisUrl,
          course.record.courseNotes,
          course.record.unitNotes,
          course.record.subjectNotes,
          course.record.supplementCode,
          campusLocationsJson,
          course.record.openSectionsCount,
          course.record.hasOpenSections ? 1 : 0,
          tags,
          course.record.searchVector,
          course.hash,
          payload,
          now,
          now,
        );
        const courseId = Number(result.lastInsertRowid);
        courseIds.set(course.record.courseNumber, courseId);
        stats.coursesInserted += 1;
      } else if (row.sourceHash !== course.hash) {
        st.updateCourse.run(
          course.record.courseString,
          course.record.title,
          course.record.expandedTitle,
          course.record.level,
          creditsMin,
          creditsMax,
          course.record.creditsDisplay,
          coreJson,
          hasCore,
          course.record.prereqHtml,
          course.record.prereqPlain,
          course.record.synopsisUrl,
          course.record.courseNotes,
          course.record.unitNotes,
          course.record.subjectNotes,
          course.record.supplementCode,
          campusLocationsJson,
          course.record.openSectionsCount,
          course.record.hasOpenSections ? 1 : 0,
          tags,
          course.record.searchVector,
          course.hash,
          payload,
          now,
          row.courseId,
        );
        courseIds.set(course.record.courseNumber, row.courseId);
        existingCourses.delete(course.record.courseNumber);
        stats.coursesUpdated += 1;
      } else {
        courseIds.set(course.record.courseNumber, row.courseId);
        existingCourses.delete(course.record.courseNumber);
      }

      const courseId = courseIds.get(course.record.courseNumber);
      if (!courseId) continue;
      for (const section of course.sections) {
        const existing = existingSections.get(section.key);
        const commentsJson = JSON.stringify(section.record.comments);
        const majorsJson = JSON.stringify(section.record.majors);
        const minorsJson = JSON.stringify(section.record.minors);
        const honorProgramsJson = JSON.stringify(section.record.honorPrograms);
        const payloadSection = JSON.stringify(section.raw);
        const campusCode = section.record.campusCode ?? slice.campus;
        if (!existing) {
          const result = st.insertSection.run(
            courseId,
            slice.term,
            slice.campus,
            subject,
            section.record.sectionNumber,
            section.record.indexNumber,
            section.record.openStatus,
            section.record.isOpen ? 1 : 0,
            now,
            section.record.instructorsText,
            section.record.sectionNotes,
            commentsJson,
            section.record.eligibilityText,
            section.record.openToText,
            majorsJson,
            minorsJson,
            honorProgramsJson,
            section.record.sectionCourseType,
            section.record.examCode,
            section.record.examCodeText,
            section.record.specialPermissionAddCode,
            section.record.specialPermissionAddDesc,
            section.record.specialPermissionDropCode,
            section.record.specialPermissionDropDesc,
            section.record.printed,
            section.record.sessionPrintIndicator,
            section.record.subtitle,
            section.record.meetingModeSummary,
            section.record.deliveryMethod,
            section.record.hasMeetings ? 1 : 0,
            section.hash,
            payloadSection,
            now,
            now,
          );
          const sectionId = Number(result.lastInsertRowid);
          changedSections.push({ sectionId, section, statusChanged: true, previousStatus: 'UNKNOWN' });
          stats.sectionsInserted += 1;
        } else if (existing.sourceHash !== section.hash) {
          const statusChanged = existing.openStatus !== section.record.openStatus;
          const openStatusTimestamp = statusChanged ? now : existing.openStatusUpdatedAt;
          st.updateSection.run(
            courseId,
            section.record.sectionNumber,
            section.record.openStatus,
            section.record.isOpen ? 1 : 0,
            openStatusTimestamp,
            section.record.instructorsText,
            section.record.sectionNotes,
            commentsJson,
            section.record.eligibilityText,
            section.record.openToText,
            majorsJson,
            minorsJson,
            honorProgramsJson,
            section.record.sectionCourseType,
            section.record.examCode,
            section.record.examCodeText,
            section.record.specialPermissionAddCode,
            section.record.specialPermissionAddDesc,
            section.record.specialPermissionDropCode,
            section.record.specialPermissionDropDesc,
            section.record.printed,
            section.record.sessionPrintIndicator,
            section.record.subtitle,
            section.record.meetingModeSummary,
            section.record.deliveryMethod,
            section.record.hasMeetings ? 1 : 0,
            section.hash,
            payloadSection,
            now,
            existing.sectionId,
          );
          changedSections.push({
            sectionId: existing.sectionId,
            section,
            statusChanged,
            previousStatus: existing.openStatus,
          });
          existingSections.delete(section.key);
          stats.sectionsUpdated += 1;
          if (statusChanged) {
            st.insertStatusEvent.run(
              existing.sectionId,
              existing.openStatus,
              section.record.openStatus,
              'courses.json',
              slice.term,
              slice.campus,
              now,
            );
          }
        } else {
          existingSections.delete(section.key);
        }
      }
    }

    for (const leftover of existingCourses.values()) {
      st.deleteCourse.run(leftover.courseId);
      stats.coursesDeleted += 1;
    }

    for (const leftover of existingSections.values()) {
      st.deleteSection.run(leftover.sectionId);
      stats.sectionsDeleted += 1;
    }

    for (const changed of changedSections) {
      st.deleteMeetings.run(changed.sectionId);
      for (const meeting of changed.section.meetings) {
        st.insertMeeting.run(
          changed.sectionId,
          meeting.meetingDay,
          meeting.weekMask,
          meeting.startTimeLabel,
          meeting.endTimeLabel,
          meeting.startMinutes,
          meeting.endMinutes,
          meeting.meetingModeCode,
          meeting.meetingModeDesc,
          meeting.campusAbbrev,
          meeting.campusLocationCode,
          meeting.campusLocationDesc,
          meeting.buildingCode,
          meeting.roomNumber,
          meeting.pmCode,
          meeting.baClassHours,
          meeting.onlineOnly ? 1 : 0,
          meeting.hash,
        );
      }
    }
  });

  tx();
  console.log(
    `  subject ${subject}: Δcourses +${stats.coursesInserted}/~${stats.coursesUpdated}/-${stats.coursesDeleted} • Δsections +${stats.sectionsInserted}/~${stats.sectionsUpdated}/-${stats.sectionsDeleted}`,
  );
  return stats;
}

function ensureReferenceRows(ctx: PipelineContext, slice: PlannedSlice, course: NormalizedCourse): void {
  const semester = ctx.termCache.get(slice.term) ?? decodeSemester(slice.term);
  ctx.statements.upsertTerm.run(slice.term, semester.year, semester.termCode, slice.term);
  ctx.statements.upsertCampus.run(slice.campus, slice.campus);
  ctx.statements.upsertSubject.run(
    course.record.subject,
    course.record.schoolCode,
    course.record.schoolDescription,
    course.record.subjectDescription,
    slice.campus,
  );
}

function applyOpenSectionsSnapshot(ctx: PipelineContext, slice: PlannedSlice, body: unknown): OpenSectionsStats {
  if (!Array.isArray(body)) {
    throw new Error('openSections payload must be an array.');
  }
  const indexes = Array.from(new Set(body.map((value) => String(value)))).sort();
  const now = new Date().toISOString();
  const hash = hashPayload({ indexes, term: slice.term, campus: slice.campus });
  const stats: OpenSectionsStats = {
    indexesSeen: indexes.length,
    markedOpen: 0,
    markedClosed: 0
  };

  const { statements: st } = ctx;
  const tx = ctx.db.transaction(() => {
    const sections = st.selectSectionStatus.all(slice.term, slice.campus) as Array<{
      section_id: number;
      index_number: string;
      is_open: number;
      open_status: string;
    }>;
    const openSet = new Set(indexes);
    for (const section of sections) {
      const shouldOpen = openSet.has(section.index_number);
      if (shouldOpen && section.is_open === 0) {
        st.updateSectionStatus.run(1, 'OPEN', now, now, section.section_id);
        st.insertStatusEvent.run(section.section_id, section.open_status, 'OPEN', 'openSections', slice.term, slice.campus, now);
        stats.markedOpen += 1;
      } else if (!shouldOpen && section.is_open === 1) {
        st.updateSectionStatus.run(0, 'CLOSED', now, now, section.section_id);
        st.insertStatusEvent.run(section.section_id, section.open_status, 'CLOSED', 'openSections', slice.term, slice.campus, now);
        stats.markedClosed += 1;
      }
    }
    for (const index of indexes) {
      st.insertOpenSnapshot.run(slice.term, slice.campus, index, now, hash);
    }
  });
  tx();
  return stats;
}

async function main(): Promise<void> {
  const options = parseArgs(process.argv.slice(2));
  if (options.showHelp) {
    showUsage();
    return;
  }
  if (!options.configPath) {
    throw new CLIError('Missing required --config <path>');
  }
  const config = await loadPipelineConfig(options.configPath);
  const plan = buildPlan(config, options);
  printPlan(plan, config, options);

  const safety = (config.safety ?? {}) as { dryRun?: boolean };
  if (options.dryRun || safety.dryRun) {
    console.log('Dry-run requested: no network or database work performed.');
    return;
  }
  await runPipeline(plan, config, options);
}

void (async () => {
  try {
    await main();
  } catch (error) {
    if (error instanceof CLIError) {
      console.error(`Error: ${error.message}`);
      console.error('Use --help to see available options.');
      process.exit(1);
    } else {
      console.error('Unexpected error:', error);
      process.exit(1);
    }
  }
})();
