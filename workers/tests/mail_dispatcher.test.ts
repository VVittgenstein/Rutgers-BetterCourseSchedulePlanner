import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import Database from 'better-sqlite3';

import { MailDispatcher } from '../mail_dispatcher.js';
import type { MailMessage, ResolvedMailSenderConfig, SendResult, SendWithRetryResult } from '../../notifications/mail/types.js';

class StubSender {
  public readonly messages: MailMessage[] = [];

  constructor(private readonly outcome: SendWithRetryResult) {}

  async send(message: MailMessage): Promise<SendWithRetryResult> {
    this.messages.push(message);
    return this.outcome;
  }
}

const baseConfig: ResolvedMailSenderConfig = {
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

function loadSchema(db: Database.Database) {
  const schemaPath = path.resolve('data', 'schema.sql');
  const contents = fs.readFileSync(schemaPath, 'utf8');
  db.exec(contents);
}

function seedData(db: Database.Database, overrides: { fanoutStatus?: string; notificationId?: number } = {}) {
  const now = '2025-01-01T00:00:00Z';
  const notificationId = overrides.notificationId ?? 1;
  db.exec(`
    INSERT INTO terms (term_id, display_name) VALUES ('12025', 'Spring 2025');
    INSERT INTO campuses (campus_code, display_name, location_code, region) VALUES ('NB', 'New Brunswick', 'NB', 'NJ');
    INSERT INTO subjects (subject_code, school_code, school_description, subject_description, campus_code)
    VALUES ('01:198', '01', 'SAS', 'Computer Science', 'NB');

    INSERT INTO courses (course_id, term_id, campus_code, subject_code, course_number, course_string, title, created_at, updated_at)
    VALUES (1, '12025', 'NB', '01:198', '111', '01:198:111', 'Intro to CS', '${now}', '${now}');

    INSERT INTO sections (section_id, course_id, term_id, campus_code, subject_code, section_number, index_number, open_status, is_open, open_status_updated_at, created_at, updated_at)
    VALUES (10, 1, '12025', 'NB', '01:198', '04', '12345', 'OPEN', 1, '${now}', '${now}', '${now}');

    INSERT INTO section_meetings (section_id, meeting_day, start_minutes, end_minutes, campus_abbrev, campus_location_code, campus_location_desc, building_code, room_number)
    VALUES (10, 'M', 540, 600, 'LIV', 'LIV', 'Livingston', 'TIL', '124');

    INSERT INTO subscriptions (subscription_id, section_id, term_id, campus_code, index_number, contact_type, contact_value, contact_hash, locale, status, is_verified, created_at, updated_at, unsubscribe_token, metadata)
    VALUES (20, 10, '12025', 'NB', '12345', 'email', 'student@example.edu', 'hash', 'en-US', 'active', 1, '${now}', '${now}', 'token-abc', '{}');

    INSERT INTO open_events (open_event_id, section_id, term_id, campus_code, index_number, status_before, status_after, seat_delta, event_at, detected_by, snapshot_id, dedupe_key, trace_id, payload, created_at)
    VALUES (30, 10, '12025', 'NB', '12345', 'CLOSED', 'OPEN', 1, '${now}', 'openSections', NULL, 'dedupe-001', 'trace-001', '{"courseTitle":"Intro to CS","sectionNumber":"04","detectedAt":"${now}"}', '${now}');

    INSERT INTO open_event_notifications (notification_id, open_event_id, subscription_id, dedupe_key, fanout_status, fanout_attempts, created_at)
    VALUES (${notificationId}, 30, 20, 'dedupe-001', '${overrides.fanoutStatus ?? 'pending'}', 0, '${now}');
  `);
}

test('dispatches and marks email notification as sent', async () => {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mail-worker-'));
  const dbPath = path.join(tmpDir, 'local.db');
  const db = new Database(dbPath);
  db.pragma('journal_mode = WAL');
  db.pragma('foreign_keys = ON');
  loadSchema(db);
  seedData(db);

  const fixedNow = new Date('2025-02-01T10:00:00Z');
  const result: SendWithRetryResult = {
    finalResult: {
      status: 'sent',
      provider: 'sendgrid',
      attempt: 1,
      durationMs: 5,
      sentAt: fixedNow.toISOString(),
      providerMessageId: 'msg-42',
    } satisfies SendResult,
    attempts: [
      {
        attempt: 1,
        startedAt: fixedNow.toISOString(),
        finishedAt: fixedNow.toISOString(),
        durationMs: 5,
        result: {
          status: 'sent',
          provider: 'sendgrid',
          attempt: 1,
          durationMs: 5,
          sentAt: fixedNow.toISOString(),
          providerMessageId: 'msg-42',
        },
      },
    ],
  };

  const sender = new StubSender(result);
  const dispatcher = new MailDispatcher(
    db,
    sender,
    baseConfig,
    {
      batchSize: 10,
      workerId: 'worker-test',
      lockTtlSeconds: 120,
      delivery: { maxAttempts: 3, retryScheduleMs: [0, 2000, 7000] },
      appBaseUrl: 'http://localhost:3000',
      defaultLocale: 'en-US',
      idleDelayMs: 10,
      runOnce: true,
    },
    () => fixedNow,
  );

  await dispatcher.runOnce();

  assert.equal(sender.messages.length, 1);
  const message = sender.messages[0];
  assert.equal(message.to.email, 'student@example.edu');
  assert.equal(message.locale, 'en-US');
  assert.equal(message.templateId, 'open-seat');
  assert.equal(message.variables.sectionIndex, '12345');
  assert.ok(String(message.variables.meetingSummary).includes('M 09:00'));
  assert.equal(message.manageUrl, 'http://localhost:3000/subscriptions/20');
  assert.ok(message.unsubscribeUrl?.includes('token-abc'));

  const notification = db
    .prepare('SELECT fanout_status, fanout_attempts, error FROM open_event_notifications WHERE notification_id = 1')
    .get() as { fanout_status: string; fanout_attempts: number; error: string };
  assert.equal(notification.fanout_status, 'sent');
  assert.equal(notification.fanout_attempts, 1);
  assert.ok(notification.error.includes('"status":"sent"'));

  const events = db
    .prepare('SELECT event_type, payload FROM subscription_events WHERE subscription_id = 20')
    .all() as Array<{ event_type: string; payload: string }>;
  assert.equal(events.length, 1);
  assert.equal(events[0].event_type, 'notify_sent');
});

test('leaves notification pending on retryable failure with backoff lock', async () => {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mail-worker-'));
  const dbPath = path.join(tmpDir, 'local.db');
  const db = new Database(dbPath);
  db.pragma('journal_mode = WAL');
  db.pragma('foreign_keys = ON');
  loadSchema(db);
  seedData(db, { notificationId: 2 });

  const fixedNow = new Date('2025-02-02T08:00:00Z');
  const retryable: SendWithRetryResult = {
    finalResult: {
      status: 'retryable',
      provider: 'sendgrid',
      attempt: 1,
      durationMs: 5,
      retryAfterSeconds: 3,
      error: { code: 'network_error', message: 'temporary error' },
    } satisfies SendResult,
    attempts: [
      {
        attempt: 1,
        startedAt: fixedNow.toISOString(),
        finishedAt: fixedNow.toISOString(),
        durationMs: 5,
        result: {
          status: 'retryable',
          provider: 'sendgrid',
          attempt: 1,
          durationMs: 5,
          retryAfterSeconds: 3,
          error: { code: 'network_error', message: 'temporary error' },
        },
      },
    ],
  };

  const sender = new StubSender(retryable);
  const lockTtlSeconds = 120;
  const dispatcher = new MailDispatcher(
    db,
    sender,
    baseConfig,
    {
      batchSize: 10,
      workerId: 'worker-test',
      lockTtlSeconds,
      delivery: { maxAttempts: 3, retryScheduleMs: [0, 2000, 7000] },
      appBaseUrl: 'http://localhost:3000',
      defaultLocale: 'en-US',
      idleDelayMs: 10,
      runOnce: true,
    },
    () => fixedNow,
  );

  await dispatcher.runOnce();

  const row = db
    .prepare('SELECT fanout_status, fanout_attempts, locked_at, last_attempt_at, error FROM open_event_notifications WHERE notification_id = 2')
    .get() as { fanout_status: string; fanout_attempts: number; locked_at: string; last_attempt_at: string; error: string };

  assert.equal(row.fanout_status, 'pending');
  assert.equal(row.fanout_attempts, 1);
  assert.ok(row.error.includes('"retryable"'));

  const lockedAtMs = Date.parse(row.locked_at);
  const expectedReady = fixedNow.getTime() + 3000; // retryAfterSeconds dominates schedule
  const readyAt = lockedAtMs + lockTtlSeconds * 1000;
  assert.ok(Math.abs(readyAt - expectedReady) < 50, `lock encodes retry delay (${readyAt} vs ${expectedReady})`);

  const events = db.prepare('SELECT COUNT(*) as count FROM subscription_events WHERE subscription_id = 20').get() as {
    count: number;
  };
  assert.equal(events.count, 0);
});
