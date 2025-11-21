import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import Database from 'better-sqlite3';

import type { DiscordSendWithRetryResult, ResolvedDiscordBotConfig } from '../../notifications/discord/bot.js';
import { DiscordDispatcher } from '../discord_dispatcher.js';

class StubBot {
  public readonly requests: any[] = [];
  constructor(private readonly outcome: DiscordSendWithRetryResult) {}

  async send(request: any): Promise<DiscordSendWithRetryResult> {
    this.requests.push(request);
    return this.outcome;
  }
}

const baseConfig: ResolvedDiscordBotConfig = {
  applicationId: 'app-123',
  botTokenEnv: 'DISCORD_BOT_TOKEN',
  botToken: 'token-abc',
  defaultMode: 'channel',
  defaultChannelId: 'chan-default',
  dm: { enabled: true, requireGuildIds: [], verificationCommand: undefined, verificationTimeoutSeconds: undefined, fallbackChannelId: undefined },
  mentions: { roleId: null, allowEveryone: false },
  rateLimit: { globalPerSecond: 10, perChannelBurst: 2, perChannelResetMs: 5000, maxAttempts: 3, backoffMs: [0, 2000, 5000], jitter: 0 },
  messageTemplate: {
    prefix: ':mega:',
    statusLine: '{{courseTitle}} {{indexNumber}} {{statusAfter}}',
    meetingLine: 'When: {{meetingSummary}}',
    footer: 'Manage: {{manageUrl}}',
  },
  logging: { redactSnowflakes: false, traceField: 'traceId' },
  testHooks: { dryRun: true, overrideChannelId: null, overrideUserId: null },
};

function loadSchema(db: Database.Database) {
  const schemaPath = path.resolve('data', 'schema.sql');
  const contents = fs.readFileSync(schemaPath, 'utf8');
  db.exec(contents);
}

function seedData(db: Database.Database, overrides: { contactType?: string; contactValue?: string; notificationId?: number } = {}) {
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
    VALUES (20, 10, '12025', 'NB', '12345', '${overrides.contactType ?? 'discord_channel'}', '${overrides.contactValue ?? 'chan-42'}', 'hash', 'en-US', 'active', 1, '${now}', '${now}', 'token-abc', '{}');

    INSERT INTO open_events (open_event_id, section_id, term_id, campus_code, index_number, status_before, status_after, seat_delta, event_at, detected_by, snapshot_id, dedupe_key, trace_id, payload, created_at)
    VALUES (30, 10, '12025', 'NB', '12345', 'CLOSED', 'OPEN', 1, '${now}', 'openSections', NULL, 'dedupe-001', 'trace-001', '{"courseTitle":"Intro to CS","sectionNumber":"04","seatDelta":1}', '${now}');

    INSERT INTO open_event_notifications (notification_id, open_event_id, subscription_id, dedupe_key, fanout_status, fanout_attempts, created_at)
    VALUES (${notificationId}, 30, 20, 'dedupe-001', 'pending', 0, '${now}');
  `);
}

test('dispatches Discord notification and marks as sent', async () => {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'discord-worker-'));
  const dbPath = path.join(tmpDir, 'local.db');
  const db = new Database(dbPath);
  db.pragma('journal_mode = WAL');
  db.pragma('foreign_keys = ON');
  loadSchema(db);
  seedData(db);

  const fixedNow = new Date('2025-02-01T10:00:00Z');
  const result: DiscordSendWithRetryResult = {
    finalResult: { status: 'sent', attempt: 1, durationMs: 5, channelId: 'chan-42', messageId: 'msg-42' },
    attempts: [
      {
        attempt: 1,
        startedAt: fixedNow.toISOString(),
        finishedAt: fixedNow.toISOString(),
        result: { status: 'sent', attempt: 1, durationMs: 5, channelId: 'chan-42', messageId: 'msg-42' },
      },
    ],
  };

  const bot = new StubBot(result);
  const dispatcher = new DiscordDispatcher(
    db,
    bot,
    baseConfig,
    {
      batchSize: 10,
      workerId: 'worker-test',
      lockTtlSeconds: 120,
      idleDelayMs: 10,
      runOnce: true,
      appBaseUrl: 'http://localhost:3000',
      defaultLocale: 'en-US',
      allowedChannelIds: [],
    },
    () => fixedNow,
  );

  await dispatcher.runOnce();

  assert.equal(bot.requests.length, 1);
  const req = bot.requests[0];
  assert.equal(req.target.channelId, 'chan-42');
  assert.equal(req.dedupeKey, 'dedupe-001');
  assert.ok(String(req.message.content).includes('Intro to CS'));
  assert.ok(String(req.message.content).includes('12345'));
  assert.ok(String(req.message.content).includes('Manage: http://localhost:3000/subscriptions/20'));

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

test('keeps notification pending on retryable failure with lock', async () => {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'discord-worker-'));
  const dbPath = path.join(tmpDir, 'local.db');
  const db = new Database(dbPath);
  db.pragma('journal_mode = WAL');
  db.pragma('foreign_keys = ON');
  loadSchema(db);
  seedData(db, { notificationId: 2 });

  const fixedNow = new Date('2025-02-02T08:00:00Z');
  const retryable: DiscordSendWithRetryResult = {
    finalResult: {
      status: 'retryable',
      attempt: 1,
      durationMs: 5,
      retryAfterSeconds: 3,
      error: { code: 'rate_limited', message: 'too many' },
    },
    attempts: [
      {
        attempt: 1,
        startedAt: fixedNow.toISOString(),
        finishedAt: fixedNow.toISOString(),
        result: {
          status: 'retryable',
          attempt: 1,
          durationMs: 5,
          retryAfterSeconds: 3,
          error: { code: 'rate_limited', message: 'too many' },
        },
      },
    ],
  };

  const bot = new StubBot(retryable);
  const lockTtlSeconds = 120;
  const dispatcher = new DiscordDispatcher(
    db,
    bot,
    baseConfig,
    {
      batchSize: 10,
      workerId: 'worker-test',
      lockTtlSeconds,
      idleDelayMs: 10,
      runOnce: true,
      appBaseUrl: 'http://localhost:3000',
      defaultLocale: 'en-US',
      allowedChannelIds: [],
    },
    () => fixedNow,
  );

  await dispatcher.runOnce();

  const row = db
    .prepare('SELECT fanout_status, fanout_attempts, locked_at, last_attempt_at, error FROM open_event_notifications WHERE notification_id = 2')
    .get() as { fanout_status: string; fanout_attempts: number; locked_at: string; last_attempt_at: string; error: string };
  assert.equal(row.fanout_status, 'pending');
  assert.equal(row.fanout_attempts, 1);
  assert.ok(row.error.includes('"status":"retryable"'));

  const lockedAtMs = new Date(row.locked_at).getTime();
  const nowMs = fixedNow.getTime();
  const expectedDelayMs = 3000; // retryAfterSeconds dominates
  const expectedLockedAtMs = nowMs - (lockTtlSeconds * 1000 - expectedDelayMs) - 1;
  assert.ok(Math.abs(lockedAtMs - expectedLockedAtMs) < 50);
});

test('skips disallowed channel targets', async () => {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'discord-worker-'));
  const dbPath = path.join(tmpDir, 'local.db');
  const db = new Database(dbPath);
  db.pragma('journal_mode = WAL');
  db.pragma('foreign_keys = ON');
  loadSchema(db);
  seedData(db, { contactValue: 'chan-disallowed' });

  const fixedNow = new Date('2025-02-03T10:00:00Z');
  const sent: DiscordSendWithRetryResult = {
    finalResult: { status: 'sent', attempt: 1, durationMs: 5, channelId: 'chan-disallowed', messageId: 'msg-99' },
    attempts: [
      {
        attempt: 1,
        startedAt: fixedNow.toISOString(),
        finishedAt: fixedNow.toISOString(),
        result: { status: 'sent', attempt: 1, durationMs: 5, channelId: 'chan-disallowed', messageId: 'msg-99' },
      },
    ],
  };

  const bot = new StubBot(sent);
  const dispatcher = new DiscordDispatcher(
    db,
    bot,
    baseConfig,
    {
      batchSize: 10,
      workerId: 'worker-test',
      lockTtlSeconds: 120,
      idleDelayMs: 10,
      runOnce: true,
      appBaseUrl: 'http://localhost:3000',
      defaultLocale: 'en-US',
      allowedChannelIds: ['chan-allowed'],
    },
    () => fixedNow,
  );

  await dispatcher.runOnce();

  assert.equal(bot.requests.length, 0);
  const notification = db
    .prepare('SELECT fanout_status, fanout_attempts, error FROM open_event_notifications WHERE notification_id = 1')
    .get() as { fanout_status: string; fanout_attempts: number; error: string };
  assert.equal(notification.fanout_status, 'skipped');
  assert.equal(notification.fanout_attempts, 1);
  assert.ok(notification.error.includes('channel_blocked'));
});
