import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import Database from 'better-sqlite3';

import { createServer } from '../src/server.js';

type ClaimResponse = {
  notifications: Array<{
    notificationId: number;
    term: string;
    campus: string;
    sectionIndex: string;
    courseTitle: string | null;
    eventAt: string;
    dedupeKey: string;
    traceId: string | null;
  }>;
  traceId: string;
  meta?: { version: string; count: number };
};

test('claims pending local notifications and updates bookkeeping', async (t) => {
  const fixture = createDbFixture();
  const restoreEnv = overrideEnv('SQLITE_FILE', fixture.file);
  seedReferenceData(fixture.file);
  const server = await createServer();
  t.after(async () => {
    await server.close();
    restoreEnv();
    fixture.cleanup();
  });

  const deviceId = 'device-test-01';
  const subscriptionId = await createLocalSoundSubscription(server, {
    term: '20251',
    campus: 'NB',
    sectionIndex: '12345',
    deviceId,
  });

  const db = new Database(fixture.file);
  const openEventId = insertOpenEvent(db, {
    index: '12345',
    dedupeKey: 'dedupe-001',
    traceId: 'trace-claim-1',
    eventAt: '2025-02-20T10:00:00Z',
    courseTitle: 'Intro to Testing',
  });
  insertNotification(db, {
    notificationId: 101,
    openEventId,
    subscriptionId,
    dedupeKey: 'dedupe-001',
  });
  db.close();

  const fixedNow = new Date('2025-02-21T12:00:00Z');
  const restoreNow = mockDateNow(fixedNow);
  t.after(restoreNow);

  const response = await server.inject({
    method: 'POST',
    url: '/api/notifications/local/claim',
    payload: { deviceId },
  });

  assert.equal(response.statusCode, 200);
  const body = response.json() as ClaimResponse;
  assert.equal(body.notifications.length, 1);
  assert.equal(body.meta?.count, 1);
  assert.ok(body.traceId);

  const notification = body.notifications[0];
  assert.equal(notification.notificationId, 101);
  assert.equal(notification.term, '20251');
  assert.equal(notification.campus, 'NB');
  assert.equal(notification.sectionIndex, '12345');
  assert.equal(notification.eventAt, '2025-02-20T10:00:00Z');
  assert.equal(notification.dedupeKey, 'dedupe-001');
  assert.equal(notification.courseTitle, 'Intro to Testing');
  assert.equal(notification.traceId, 'trace-claim-1');

  const dbCheck = new Database(fixture.file);
  const notifRow = dbCheck
    .prepare(
      `
      SELECT fanout_status, fanout_attempts, last_attempt_at, locked_by, locked_at
      FROM open_event_notifications
      WHERE notification_id = ?
    `,
    )
    .get(101) as {
      fanout_status: string;
      fanout_attempts: number;
      last_attempt_at: string;
      locked_by: string | null;
      locked_at: string | null;
    };
  assert.equal(notifRow.fanout_status, 'sent');
  assert.equal(notifRow.fanout_attempts, 1);
  assert.equal(notifRow.last_attempt_at, fixedNow.toISOString());
  assert.equal(notifRow.locked_by, null);
  assert.equal(notifRow.locked_at, null);

  const subscriptionRow = dbCheck
    .prepare(
      `
      SELECT last_notified_at, last_known_section_status, updated_at
      FROM subscriptions
      WHERE subscription_id = ?
    `,
    )
    .get(subscriptionId) as {
      last_notified_at: string;
      last_known_section_status: string;
      updated_at: string;
    };
  assert.equal(subscriptionRow.last_notified_at, fixedNow.toISOString());
  assert.equal(subscriptionRow.last_known_section_status, 'OPEN');
  assert.equal(subscriptionRow.updated_at, fixedNow.toISOString());

  const eventRow = dbCheck
    .prepare(
      `
      SELECT event_type, payload
      FROM subscription_events
      WHERE subscription_id = ?
      ORDER BY event_id DESC
      LIMIT 1
    `,
    )
    .get(subscriptionId) as { event_type: string; payload: string };
  assert.equal(eventRow.event_type, 'notify_sent');
  assert.ok(eventRow.payload.includes('"channel":"local_sound"'));
  assert.ok(eventRow.payload.includes('"dedupeKey":"dedupe-001"'));
  dbCheck.close();
});

test('claim limit caps batch size and leaves remaining rows pending', async (t) => {
  const fixture = createDbFixture();
  const restoreEnv = overrideEnv('SQLITE_FILE', fixture.file);
  seedReferenceData(fixture.file);
  const server = await createServer();
  t.after(async () => {
    await server.close();
    restoreEnv();
    fixture.cleanup();
  });

  const deviceA = 'device-limit-01';
  const deviceB = 'device-other-02';
  const subA = await createLocalSoundSubscription(server, {
    term: '20251',
    campus: 'NB',
    sectionIndex: '12345',
    deviceId: deviceA,
  });
  const subB = await createLocalSoundSubscription(server, {
    term: '20251',
    campus: 'NB',
    sectionIndex: '12345',
    deviceId: deviceB,
  });

  const db = new Database(fixture.file);
  const event1 = insertOpenEvent(db, {
    index: '12345',
    dedupeKey: 'dedupe-a',
    traceId: 'trace-a',
    eventAt: '2025-02-22T08:00:00Z',
  });
  const event2 = insertOpenEvent(db, {
    index: '12345',
    dedupeKey: 'dedupe-b',
    traceId: 'trace-b',
    eventAt: '2025-02-22T08:01:00Z',
  });
  const event3 = insertOpenEvent(db, {
    index: '12345',
    dedupeKey: 'dedupe-c',
    traceId: 'trace-c',
    eventAt: '2025-02-22T08:02:00Z',
  });
  insertNotification(db, { notificationId: 201, openEventId: event1, subscriptionId: subA, dedupeKey: 'dedupe-a' });
  insertNotification(db, { notificationId: 202, openEventId: event2, subscriptionId: subA, dedupeKey: 'dedupe-b' });
  insertNotification(db, { notificationId: 203, openEventId: event3, subscriptionId: subA, dedupeKey: 'dedupe-c' });
  insertNotification(db, { notificationId: 204, openEventId: event3, subscriptionId: subB, dedupeKey: 'dedupe-c' });
  db.close();

  const fixedNow = new Date('2025-02-22T09:00:00Z');
  const restoreNow = mockDateNow(fixedNow);
  t.after(restoreNow);

  const response = await server.inject({
    method: 'POST',
    url: '/api/notifications/local/claim',
    payload: { deviceId: deviceA, limit: 2 },
  });
  assert.equal(response.statusCode, 200);
  const body = response.json() as ClaimResponse;
  assert.equal(body.notifications.length, 2);
  assert.equal(body.meta?.count, 2);
  assert.deepEqual(
    body.notifications.map((item) => item.notificationId),
    [201, 202],
  );

  const dbCheck = new Database(fixture.file);
  const rows = dbCheck
    .prepare(
      `
      SELECT notification_id, subscription_id, fanout_status, fanout_attempts
      FROM open_event_notifications
      ORDER BY notification_id
    `,
    )
    .all() as Array<{
      notification_id: number;
      subscription_id: number;
      fanout_status: string;
      fanout_attempts: number;
    }>;

  assert.deepEqual(
    rows.map((row) => [row.notification_id, row.fanout_status]),
    [
      [201, 'sent'],
      [202, 'sent'],
      [203, 'pending'],
      [204, 'pending'],
    ],
  );
  assert.equal(rows.find((row) => row.notification_id === 201)?.fanout_attempts, 1);
  assert.equal(rows.find((row) => row.notification_id === 202)?.fanout_attempts, 1);
  assert.equal(rows.find((row) => row.notification_id === 203)?.fanout_attempts, 0);
  assert.equal(rows.find((row) => row.notification_id === 204)?.fanout_attempts, 0);

  const subscriptionRow = dbCheck
    .prepare('SELECT last_notified_at FROM subscriptions WHERE subscription_id = ?')
    .get(subA) as { last_notified_at: string | null };
  assert.equal(subscriptionRow.last_notified_at, fixedNow.toISOString());
  dbCheck.close();
});

function createDbFixture() {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'local-claim-'));
  const file = path.join(directory, 'db.sqlite');
  const db = new Database(file);
  db.pragma('journal_mode = WAL');
  db.pragma('foreign_keys = ON');
  const schemaPath = path.resolve('data', 'schema.sql');
  const schema = fs.readFileSync(schemaPath, 'utf8');
  db.exec(schema);
  db.close();
  return {
    file,
    cleanup: () => fs.rmSync(directory, { recursive: true, force: true }),
  };
}

function seedReferenceData(file: string) {
  const db = new Database(file);
  db.prepare("INSERT INTO terms (term_id, display_name) VALUES ('20251', 'Spring 2025')").run();
  db
    .prepare("INSERT INTO campuses (campus_code, display_name, location_code, region) VALUES ('NB', 'New Brunswick', 'NB', 'NJ')")
    .run();
  db.close();
}

async function createLocalSoundSubscription(
  server: Awaited<ReturnType<typeof createServer>>,
  args: { term: string; campus: string; sectionIndex: string; deviceId: string },
) {
  const response = await server.inject({
    method: 'POST',
    url: '/api/subscribe',
    payload: {
      term: args.term,
      campus: args.campus,
      sectionIndex: args.sectionIndex,
      contactType: 'local_sound',
      contactValue: args.deviceId,
    },
  });
  assert.equal(response.statusCode, 201);
  const body = response.json() as { subscriptionId: number };
  return body.subscriptionId;
}

function insertOpenEvent(
  db: Database.Database,
  args: { index: string; dedupeKey: string; traceId: string; eventAt: string; courseTitle?: string },
) {
  const now = args.eventAt;
  const result = db
    .prepare(
      `
      INSERT INTO open_events (
        section_id,
        term_id,
        campus_code,
        index_number,
        status_before,
        status_after,
        seat_delta,
        event_at,
        detected_by,
        snapshot_id,
        dedupe_key,
        trace_id,
        payload,
        created_at
      )
      VALUES (NULL, '20251', 'NB', ?, 'CLOSED', 'OPEN', 1, ?, 'test', NULL, ?, ?, ?, ?)
    `,
    )
    .run(args.index, args.eventAt, args.dedupeKey, args.traceId, buildPayload(args.courseTitle), now);
  return Number(result.lastInsertRowid);
}

function insertNotification(
  db: Database.Database,
  args: { notificationId: number; openEventId: number; subscriptionId: number; dedupeKey: string },
) {
  const createdAt = '2025-02-20T00:00:00Z';
  db.prepare(
    `
    INSERT INTO open_event_notifications (
      notification_id,
      open_event_id,
      subscription_id,
      dedupe_key,
      fanout_status,
      fanout_attempts,
      created_at
    )
    VALUES (?, ?, ?, ?, 'pending', 0, ?)
  `,
  ).run(args.notificationId, args.openEventId, args.subscriptionId, args.dedupeKey, createdAt);
}

function buildPayload(courseTitle?: string) {
  if (!courseTitle) return null;
  return JSON.stringify({ courseTitle });
}

function overrideEnv(key: string, value: string) {
  const previous = process.env[key];
  process.env[key] = value;
  return () => {
    if (previous === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = previous;
    }
  };
}

function mockDateNow(now: Date) {
  const original = Date.now;
  Date.now = () => now.getTime();
  return () => {
    Date.now = original;
  };
}
