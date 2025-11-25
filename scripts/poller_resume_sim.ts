#!/usr/bin/env tsx
import Database from 'better-sqlite3';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { decodeSemester, performProbe } from './soc_api_client.js';
import {
  applySnapshot,
  createStatements,
  hydrateMissCountersFromCheckpoint,
  loadCheckpointState,
  persistCheckpoint,
  type PollerContext,
  type PollerOptions,
  type Metrics,
  type PollTarget,
} from '../workers/open_sections_poller.js';

type SimulationTotals = {
  opened: number;
  closed: number;
  events: number;
  notifications: number;
};

const CAMPUS = 'NB';
const TERM = '12024';
const CHECKPOINT_FILE = process.env.CHECKPOINT_FILE
  ? path.resolve(process.env.CHECKPOINT_FILE)
  : path.resolve('scripts', 'poller_checkpoint.json');
const BASE_TIME = new Date('2024-10-01T10:00:00.000Z');
const RESTART_AFTER_TICK = 80;
const TOTAL_TICKS = 120; // minute granularity -> 2 hours

function applyMigrations(db: Database.Database) {
  const baseSchema = fs.readFileSync(path.resolve('data/migrations/001_init_schema.sql'), 'utf-8');
  const openEventSchema = fs.readFileSync(path.resolve('data/migrations/003_open_events.sql'), 'utf-8');
  db.exec(baseSchema);
  db.exec(openEventSchema);
}

function seedData(db: Database.Database) {
  const now = new Date().toISOString();
  db.prepare('INSERT OR IGNORE INTO campuses (campus_code, display_name) VALUES (?, ?)').run(CAMPUS, 'New Brunswick');
  db.prepare('INSERT OR IGNORE INTO terms (term_id, term_code, display_name) VALUES (?, ?, ?)').run(TERM, '1', 'Spring 2024');
  db.prepare(
    'INSERT OR IGNORE INTO subjects (subject_code, campus_code, subject_description) VALUES (?, ?, ?)',
  ).run('CS', CAMPUS, 'Computer Science');

  const courseResult = db
    .prepare(
      `
      INSERT INTO courses (term_id, campus_code, subject_code, course_number, title, created_at, updated_at)
      VALUES (?, ?, ?, ?, ?, ?, ?)
    `,
    )
    .run(TERM, CAMPUS, 'CS', '101', 'Simulated Course', now, now);
  const courseId = Number(courseResult.lastInsertRowid);

  const sectionResult = db
    .prepare(
      `
      INSERT INTO sections (
        course_id, term_id, campus_code, subject_code, section_number, index_number,
        open_status, is_open, open_status_updated_at, created_at, updated_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `,
    )
    .run(courseId, TERM, CAMPUS, 'CS', '01', '00001', 'CLOSED', 0, now, now, now);
  const sectionId = Number(sectionResult.lastInsertRowid);

  db.prepare(
    `
    INSERT INTO subscriptions (
      section_id, term_id, campus_code, index_number, contact_type, contact_value,
      contact_hash, status, created_at, updated_at, last_known_section_status
    ) VALUES (?, ?, ?, ?, 'email', ?, ?, 'active', ?, ?, 'CLOSED')
  `,
  ).run(sectionId, TERM, CAMPUS, '00001', 'sim@example.com', 'hash-sim', now, now);
}

function buildTarget(): PollTarget {
  return {
    termId: TERM,
    campus: CAMPUS,
    decodedTerm: decodeSemester(TERM),
  };
}

function buildContext(dbPath: string): { ctx: PollerContext; target: PollTarget } {
  const options: PollerOptions = {
    termsMode: 'explicit',
    terms: [TERM],
    campuses: [CAMPUS],
    intervalMs: 15000,
    refreshIntervalMs: 5 * 60 * 1000,
    openReminderIntervalMs: 3 * 60 * 1000,
    jitter: 0,
    sqliteFile: dbPath,
    timeoutMs: 8000,
    concurrency: 1,
    subscriptionChunkSize: 50,
    metricsPort: null,
    missThreshold: 2,
    runOnce: false,
    checkpointFile: CHECKPOINT_FILE,
  };
  const metrics: Metrics = {
    pollsTotal: 0,
    pollsFailed: 0,
    eventsEmitted: 0,
    notificationsQueued: 0,
    targets: {},
  };

  const db = new Database(dbPath);
  db.pragma('journal_mode = WAL');
  db.pragma('foreign_keys = ON');

  const ctx: PollerContext = {
    options,
    db,
    statements: createStatements(db),
    probeFn: performProbe,
    missCounters: new Map(),
    metrics,
    checkpoint: loadCheckpointState(CHECKPOINT_FILE),
    datasetStatus: new Map(),
  };
  const target = buildTarget();

  hydrateMissCountersFromCheckpoint(ctx, target);

  return { ctx, target };
}

function openIndexesForTick(tick: number): string[] {
  if (tick < 30) return [];
  if (tick < 80) return ['00001']; // goes open after 30m, stays open
  if (tick < 82) return []; // two missing heartbeats should close
  if (tick < 100) return []; // remains closed for a while
  return ['00001']; // re-opens for the last stretch
}

function runSimulation() {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'poller-sim-'));
  const dbPath = path.join(tempDir, 'local.db');
  try {
    const db = new Database(dbPath);
    db.pragma('journal_mode = WAL');
    db.pragma('foreign_keys = ON');
    applyMigrations(db);
    seedData(db);
    db.close();

    let { ctx, target } = buildContext(dbPath);
    const totals: SimulationTotals = { opened: 0, closed: 0, events: 0, notifications: 0 };
    const timeline: string[] = [];

    for (let tick = 0; tick < TOTAL_TICKS; tick += 1) {
      if (tick === RESTART_AFTER_TICK + 1) {
        ctx.db.close();
        ({ ctx, target } = buildContext(dbPath));
        timeline.push(`restart at tick=${tick}, restored misses=${ctx.missCounters.get(`${TERM}|${CAMPUS}`)?.size ?? 0}`);
      }

      const now = new Date(BASE_TIME.getTime() + tick * 60 * 1000);
      const indexes = openIndexesForTick(tick);
      const outcome = applySnapshot(ctx, target, indexes, now);
      persistCheckpoint(ctx, target, outcome);

      totals.opened += outcome.opened;
      totals.closed += outcome.closed;
      totals.events += outcome.events;
      totals.notifications += outcome.notifications;

      if (outcome.opened > 0 || outcome.closed > 0 || outcome.events > 0 || outcome.notifications > 0) {
        timeline.push(
          `${now.toISOString()} tick=${tick} open=${indexes.length} opened=${outcome.opened} closed=${outcome.closed} events=${outcome.events} notifications=${outcome.notifications}`,
        );
      }
    }

    ctx.db.close();

    console.log('Durability simulation complete:');
    console.log(`  checkpoint file: ${CHECKPOINT_FILE}`);
    console.log(
      `  totals -> opened=${totals.opened} closed=${totals.closed} events=${totals.events} notifications=${totals.notifications}`,
    );
    console.log('  timeline (events only):');
    console.log(timeline.map((line) => `    ${line}`).join('\n'));
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
}

runSimulation();
