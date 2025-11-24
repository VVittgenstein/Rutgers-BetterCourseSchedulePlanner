#!/usr/bin/env tsx
import Database from 'better-sqlite3';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { performance } from 'node:perf_hooks';

import { decodeSemester, performProbe } from './soc_api_client.js';
import { MailDispatcher } from '../workers/mail_dispatcher.js';
import {
  applySnapshot,
  createStatements,
  loadCheckpointState,
  type Metrics,
  type PollerContext,
  type PollerOptions,
  type PollTarget,
} from '../workers/open_sections_poller.js';
import type { MailMessage, ResolvedMailSenderConfig, SendWithRetryResult } from '../notifications/mail/types.js';

type SimulationResult = {
  detectionToSendMs: number;
  bestCaseMs: number;
  avgCaseMs: number;
  worstCaseMs: number;
  messages: MailMessage[];
  openOutcome: { events: number; notifications: number };
  reopenOutcome: { events: number; notifications: number };
};

const mailConfig: ResolvedMailSenderConfig = {
  provider: 'sendgrid',
  defaultFrom: { email: 'alerts@example.edu' },
  replyTo: undefined,
  supportedLocales: ['en-US'],
  templateRoot: '',
  templates: { 'open-seat': { html: { 'en-US': 'noop' }, requiredVariables: [] } },
  rateLimit: { maxPerSecond: 5, burst: 10, bucketWidthSeconds: 60 },
  retryPolicy: { maxAttempts: 3, backoffMs: [0, 10, 20], jitter: 0, retryableErrors: ['unknown', 'network_error'] },
  timeouts: { connectMs: 1000, sendMs: 1000, idleMs: 1000 },
  providers: { sendgrid: { apiKey: 'test', sandboxMode: true, categories: [], ipPool: null } },
  logging: { redactPII: true, traceHeader: 'x-trace-id' },
  testHooks: { dryRun: false, overrideRecipient: null },
};

class TimedSender {
  public readonly messages: MailMessage[] = [];
  private counter = 0;

  constructor(private readonly delayMs: number) {}

  async send(message: MailMessage): Promise<SendWithRetryResult> {
    const startedWall = performance.now();
    await sleep(this.delayMs);
    const finishedWall = performance.now();

    this.messages.push(message);
    this.counter += 1;

    const startedAt = new Date();
    const finishedAt = new Date(startedAt.getTime() + Math.round(finishedWall - startedWall));

    const durationMs = finishedWall - startedWall;
    return {
      finalResult: {
        status: 'sent',
        provider: 'sendgrid',
        providerMessageId: `stub-${this.counter}`,
        attempt: 1,
        durationMs,
        sentAt: finishedAt.toISOString(),
      },
      attempts: [
        {
          attempt: 1,
          startedAt: startedAt.toISOString(),
          finishedAt: finishedAt.toISOString(),
          durationMs,
          result: {
            status: 'sent',
            provider: 'sendgrid',
            providerMessageId: `stub-${this.counter}`,
            attempt: 1,
            durationMs,
            sentAt: finishedAt.toISOString(),
          },
        },
      ],
    };
  }
}

function loadSchema(db: Database.Database) {
  const schemaPath = path.resolve('data', 'schema.sql');
  const contents = fs.readFileSync(schemaPath, 'utf8');
  db.exec(contents);
}

function seedData(db: Database.Database, campus: string, term: string, nowIso: string, recipients: string[]) {
  db.prepare('INSERT OR IGNORE INTO campuses (campus_code, display_name) VALUES (?, ?)').run(campus, 'Sim Campus');
  db.prepare('INSERT OR IGNORE INTO terms (term_id, term_code, display_name) VALUES (?, ?, ?)').run(term, '1', term);
  db.prepare(
    'INSERT OR IGNORE INTO subjects (subject_code, campus_code, subject_description) VALUES (?, ?, ?)',
  ).run('CS', campus, 'Computer Science');

  const courseResult = db
    .prepare(
      `
      INSERT INTO courses (term_id, campus_code, subject_code, course_number, course_string, title, created_at, updated_at)
      VALUES (?, ?, ?, ?, ?, ?, ?, ?)
    `,
    )
    .run(term, campus, 'CS', '101', 'CS:101', 'Simulated Course', nowIso, nowIso);
  const courseId = Number(courseResult.lastInsertRowid);

  const sectionResult = db
    .prepare(
      `
      INSERT INTO sections (course_id, term_id, campus_code, subject_code, section_number, index_number, open_status, is_open, open_status_updated_at, created_at, updated_at)
      VALUES (?, ?, ?, ?, ?, ?, 'CLOSED', 0, ?, ?, ?)
    `,
    )
    .run(courseId, term, campus, 'CS', '01', '12345', nowIso, nowIso, nowIso);
  const sectionId = Number(sectionResult.lastInsertRowid);

  db.prepare(
    `
    INSERT INTO section_meetings (section_id, meeting_day, start_minutes, end_minutes, campus_abbrev, campus_location_code, campus_location_desc, building_code, room_number)
    VALUES (?, 'M', 540, 590, 'LIV', 'LIV', 'Livingston', 'TIL', '124')
  `,
  ).run(sectionId);

  const insertSub = db.prepare(
    `
    INSERT INTO subscriptions (
      section_id, term_id, campus_code, index_number, contact_type, contact_value, contact_hash,
      locale, status, is_verified, created_at, updated_at, last_known_section_status, unsubscribe_token, metadata
    ) VALUES (?, ?, ?, ?, 'email', ?, ?, 'en-US', 'active', 1, ?, ?, 'CLOSED', ?, '{}')
  `,
  );

  recipients.forEach((email, idx) => {
    const token = `token-${idx + 1}`;
    insertSub.run(sectionId, term, campus, '12345', email, `hash-${idx}`, nowIso, nowIso, token);
  });
}

function buildContext(
  dbPath: string,
  options: { term: string; campus: string; intervalMs: number; jitter: number },
): { ctx: PollerContext; target: PollTarget } {
  const pollerOptions: PollerOptions = {
    termsMode: 'explicit',
    terms: [options.term],
    campuses: [options.campus],
    intervalMs: options.intervalMs,
    refreshIntervalMs: 5 * 60 * 1000,
    jitter: options.jitter,
    sqliteFile: dbPath,
    timeoutMs: 8000,
    concurrency: 1,
    subscriptionChunkSize: 200,
    metricsPort: null,
    missThreshold: 2,
    runOnce: true,
    checkpointFile: path.join(path.dirname(dbPath), 'checkpoint.json'),
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
    options: pollerOptions,
    db,
    statements: createStatements(db),
    probeFn: performProbe,
    missCounters: new Map(),
    metrics,
    checkpoint: loadCheckpointState(pollerOptions.checkpointFile),
    datasetStatus: new Map(),
  };
  const target: PollTarget = { termId: options.term, campus: options.campus, decodedTerm: decodeSemester(options.term) };

  return { ctx, target };
}

async function simulate() {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mail-e2e-'));
  const dbPath = path.join(tempDir, 'local.db');
  const term = '12025';
  const campus = 'NB';
  const pollIntervalMs = 20000;
  const jitter = 0.2;
  const nowIso = '2025-02-01T10:00:00.000Z';

  try {
    const db = new Database(dbPath);
    db.pragma('journal_mode = WAL');
    db.pragma('foreign_keys = ON');
    loadSchema(db);
    seedData(db, campus, term, nowIso, [
      'alice@example.edu',
      'bob@example.edu',
      'charlie@example.edu',
    ]);
    db.close();

    const { ctx, target } = buildContext(dbPath, { term, campus, intervalMs: pollIntervalMs, jitter });
    const sender = new TimedSender(250);
    const dispatcher = new MailDispatcher(ctx.db, sender, mailConfig, {
      batchSize: 25,
      workerId: 'mail-e2e',
      lockTtlSeconds: 120,
      delivery: { maxAttempts: 3, retryScheduleMs: [0, 2000, 7000] },
      appBaseUrl: 'http://localhost:3000',
      defaultLocale: 'en-US',
      idleDelayMs: 50,
      runOnce: true,
    });

    const detectionStart = performance.now();
    const openAt = new Date('2025-02-01T10:01:05.000Z');
    const openOutcome = applySnapshot(ctx, target, ['12345'], openAt);

    await dispatcher.runOnce();

    const dispatchFinished = performance.now();
    const detectionToSendMs = dispatchFinished - detectionStart;

    // Force a close then a reopen within the same 5-minute bucket to confirm dedupe.
    applySnapshot(ctx, target, [], new Date(openAt.getTime() + 20000));
    applySnapshot(ctx, target, [], new Date(openAt.getTime() + 40000));
    const reopenOutcome = applySnapshot(ctx, target, ['12345'], new Date(openAt.getTime() + 2 * 60 * 1000));
    await dispatcher.runOnce();

    const bestCaseMs = detectionToSendMs;
    const avgCaseMs = pollIntervalMs / 2 + detectionToSendMs;
    const worstCaseMs = pollIntervalMs + detectionToSendMs;

    ctx.db.close();

    const result: SimulationResult = {
      detectionToSendMs,
      bestCaseMs,
      avgCaseMs,
      worstCaseMs,
      messages: sender.messages,
      openOutcome: { events: openOutcome.events, notifications: openOutcome.notifications },
      reopenOutcome: { events: reopenOutcome.events, notifications: reopenOutcome.notifications },
    };

    console.log(JSON.stringify(result, null, 2));
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

void simulate().catch((error) => {
  console.error('mail_e2e_sim failed:', error);
  process.exit(1);
});
