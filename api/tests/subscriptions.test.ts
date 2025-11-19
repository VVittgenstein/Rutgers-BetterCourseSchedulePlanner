import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import Database from 'better-sqlite3';

import { createServer } from '../src/server.js';

test('subscribe persists new row, resolves section, and logs an event', async (t) => {
  const fixture = createSubscriptionFixture();
  const restoreEnv = overrideEnv('SQLITE_FILE', fixture.file);
  const server = await createServer();
  t.after(async () => {
    await server.close();
    restoreEnv();
    fixture.cleanup();
  });

  const response = await server.inject({
    method: 'POST',
    url: '/api/subscribe',
    payload: {
      term: '20251',
      campus: 'nb',
      sectionIndex: '12345',
      contactType: 'email',
      contactValue: ' Student@example.edu ',
      preferences: {
        maxNotifications: 4,
      },
    },
  });

  assert.equal(response.statusCode, 201);
  const body = response.json();
  assert.equal(body.sectionResolved, true);
  assert.equal(body.requiresVerification, true);
  assert.equal(body.existing, false);
  assert.equal(body.preferences.maxNotifications, 4);
  assert.match(body.unsubscribeToken, /^[a-f0-9]{32}$/);

  const db = new Database(fixture.file);
  const row = db.prepare('SELECT * FROM subscriptions WHERE subscription_id = ?').get(body.subscriptionId) as {
    contact_value: string;
    status: string;
    section_id: number;
  };
  assert.equal(row.contact_value, 'student@example.edu');
  assert.equal(row.status, 'pending');
  assert.equal(row.section_id, 1);

  const event = db
    .prepare('SELECT event_type, payload FROM subscription_events WHERE subscription_id = ?')
    .get(body.subscriptionId) as { event_type: string; payload: string };
  assert.equal(event.event_type, 'created');
  assert.ok(event.payload.includes('api'));
  db.close();
});

test('duplicate subscribe requests reuse the existing record', async (t) => {
  const fixture = createSubscriptionFixture();
  const restoreEnv = overrideEnv('SQLITE_FILE', fixture.file);
  const server = await createServer();
  t.after(async () => {
    await server.close();
    restoreEnv();
    fixture.cleanup();
  });

  const first = await server.inject({
    method: 'POST',
    url: '/api/subscribe',
    payload: {
      term: '20251',
      campus: 'NB',
      sectionIndex: '12345',
      contactType: 'email',
      contactValue: 'subscriber@example.edu',
    },
  });
  assert.equal(first.statusCode, 201);
  const firstBody = first.json();

  const second = await server.inject({
    method: 'POST',
    url: '/api/subscribe',
    payload: {
      term: '20251',
      campus: 'NB',
      sectionIndex: '12345',
      contactType: 'email',
      contactValue: 'subscriber@example.edu',
    },
  });
  assert.equal(second.statusCode, 200);
  const secondBody = second.json();
  assert.equal(secondBody.subscriptionId, firstBody.subscriptionId);
  assert.equal(secondBody.existing, true);
  assert.equal(secondBody.requiresVerification, true);

  const db = new Database(fixture.file);
  const rows = db.prepare('SELECT COUNT(*) as count FROM subscriptions').get() as { count: number };
  assert.equal(rows.count, 1);
  db.close();
});

test('contact limit prevents more than three active subscriptions per contact', async (t) => {
  const fixture = createSubscriptionFixture();
  const restoreEnv = overrideEnv('SQLITE_FILE', fixture.file);
  seedSection(fixture.file, { sectionId: 2, index: '11111' });
  seedSection(fixture.file, { sectionId: 3, index: '22222' });
  seedSection(fixture.file, { sectionId: 4, index: '33333' });
  const server = await createServer();
  t.after(async () => {
    await server.close();
    restoreEnv();
    fixture.cleanup();
  });

  const payloadBase = {
    term: '20251',
    campus: 'NB',
    contactType: 'email',
    contactValue: 'limit@example.edu',
  };

  const first = await server.inject({
    method: 'POST',
    url: '/api/subscribe',
    payload: { ...payloadBase, sectionIndex: '12345' },
  });
  assert.equal(first.statusCode, 201);
  const second = await server.inject({
    method: 'POST',
    url: '/api/subscribe',
    payload: { ...payloadBase, sectionIndex: '11111' },
  });
  assert.equal(second.statusCode, 201);
  const third = await server.inject({
    method: 'POST',
    url: '/api/subscribe',
    payload: { ...payloadBase, sectionIndex: '22222' },
  });
  assert.equal(third.statusCode, 201);

  const blocked = await server.inject({
    method: 'POST',
    url: '/api/subscribe',
    payload: { ...payloadBase, sectionIndex: '33333' },
  });
  assert.equal(blocked.statusCode, 400);
  const error = blocked.json();
  assert.equal(error.error.code, 'rate_limited');
});

test('subscribe accepts unresolved sections but reports sectionResolved false', async (t) => {
  const fixture = createSubscriptionFixture();
  const restoreEnv = overrideEnv('SQLITE_FILE', fixture.file);
  const server = await createServer();
  t.after(async () => {
    await server.close();
    restoreEnv();
    fixture.cleanup();
  });

  const response = await server.inject({
    method: 'POST',
    url: '/api/subscribe',
    payload: {
      term: '20251',
      campus: 'NB',
      sectionIndex: '99999',
      contactType: 'email',
      contactValue: 'unresolved@example.edu',
    },
  });
  assert.equal(response.statusCode, 201);
  const body = response.json();
  assert.equal(body.sectionResolved, false);
});

test('section conflict returns 409 with canonical campus info', async (t) => {
  const fixture = createSubscriptionFixture();
  seedSection(fixture.file, { sectionId: 5, index: '77777', campus: 'NK' });
  const restoreEnv = overrideEnv('SQLITE_FILE', fixture.file);
  const server = await createServer();
  t.after(async () => {
    await server.close();
    restoreEnv();
    fixture.cleanup();
  });

  const response = await server.inject({
    method: 'POST',
    url: '/api/subscribe',
    payload: {
      term: '20251',
      campus: 'NB',
      sectionIndex: '77777',
      contactType: 'email',
      contactValue: 'conflict@example.edu',
    },
  });
  assert.equal(response.statusCode, 409);
  const body = response.json();
  assert.equal(body.error.code, 'section_conflict');
  assert.equal(body.error.details.campus, 'NK');
});

test('unsubscribe transitions status and redacts contact value', async (t) => {
  const fixture = createSubscriptionFixture();
  const restoreEnv = overrideEnv('SQLITE_FILE', fixture.file);
  const server = await createServer();
  t.after(async () => {
    await server.close();
    restoreEnv();
    fixture.cleanup();
  });

  const subscribe = await server.inject({
    method: 'POST',
    url: '/api/subscribe',
    payload: {
      term: '20251',
      campus: 'NB',
      sectionIndex: '12345',
      contactType: 'email',
      contactValue: 'unsubscribe@example.edu',
    },
  });
  const { subscriptionId } = subscribe.json();

  const response = await server.inject({
    method: 'POST',
    url: '/api/unsubscribe',
    payload: {
      subscriptionId,
      reason: 'user_request',
    },
  });
  assert.equal(response.statusCode, 200);
  const body = response.json();
  assert.equal(body.status, 'unsubscribed');
  assert.equal(body.previousStatus, 'pending');

  const db = new Database(fixture.file);
  const row = db.prepare('SELECT status, contact_value FROM subscriptions WHERE subscription_id = ?').get(subscriptionId) as {
    status: string;
    contact_value: string;
  };
  assert.equal(row.status, 'unsubscribed');
  assert.equal(row.contact_value, '');
  const event = db
    .prepare('SELECT event_type, payload FROM subscription_events WHERE subscription_id = ? ORDER BY event_id DESC LIMIT 1')
    .get(subscriptionId) as { event_type: string; payload: string };
  assert.equal(event.event_type, 'unsubscribed');
  assert.ok(event.payload.includes('user_request'));
  db.close();
});

test('invalid email contact is rejected with validation error', async (t) => {
  const fixture = createSubscriptionFixture();
  const restoreEnv = overrideEnv('SQLITE_FILE', fixture.file);
  const server = await createServer();
  t.after(async () => {
    await server.close();
    restoreEnv();
    fixture.cleanup();
  });

  const response = await server.inject({
    method: 'POST',
    url: '/api/subscribe',
    payload: {
      term: '20251',
      campus: 'NB',
      sectionIndex: '12345',
      contactType: 'email',
      contactValue: 'invalid-email',
    },
  });

  assert.equal(response.statusCode, 400);
  const body = response.json();
  assert.equal(body.error.code, 'invalid_contact');
});

function createSubscriptionFixture() {
  const directory = mkdtempSync(path.join(os.tmpdir(), 'subscriptions-api-'));
  const file = path.join(directory, 'db.sqlite');
  const db = new Database(file);
  db.exec('PRAGMA journal_mode = WAL;');
  db.exec('PRAGMA foreign_keys = OFF;');
  db.exec(`
    CREATE TABLE terms (term_id TEXT PRIMARY KEY);
    CREATE TABLE campuses (campus_code TEXT PRIMARY KEY);
    CREATE TABLE sections (
      section_id INTEGER PRIMARY KEY AUTOINCREMENT,
      term_id TEXT NOT NULL,
      campus_code TEXT NOT NULL,
      index_number TEXT NOT NULL
    );
    CREATE TABLE subscriptions (
      subscription_id INTEGER PRIMARY KEY AUTOINCREMENT,
      section_id INTEGER,
      term_id TEXT NOT NULL,
      campus_code TEXT NOT NULL,
      index_number TEXT NOT NULL,
      contact_type TEXT NOT NULL,
      contact_value TEXT,
      contact_hash TEXT NOT NULL,
      locale TEXT,
      status TEXT NOT NULL,
      is_verified INTEGER DEFAULT 0,
      created_at TEXT,
      updated_at TEXT,
      last_known_section_status TEXT,
      unsubscribe_token TEXT,
      metadata TEXT
    );
    CREATE TABLE subscription_events (
      event_id INTEGER PRIMARY KEY AUTOINCREMENT,
      subscription_id INTEGER NOT NULL,
      event_type TEXT NOT NULL,
      section_status_snapshot TEXT,
      payload TEXT,
      created_at TEXT
    );
  `);
  db.prepare('INSERT INTO terms (term_id) VALUES (?)').run('20251');
  db.prepare('INSERT INTO campuses (campus_code) VALUES (?)').run('NB');
  db.prepare('INSERT INTO campuses (campus_code) VALUES (?)').run('NK');
  db.prepare('INSERT INTO sections (section_id, term_id, campus_code, index_number) VALUES (?, ?, ?, ?)').run(
    1,
    '20251',
    'NB',
    '12345',
  );
  db.close();

  return {
    file,
    cleanup: () => rmSync(directory, { recursive: true, force: true }),
  };
}

function seedSection(
  file: string,
  opts: {
    sectionId: number;
    index: string;
    campus?: string;
  },
) {
  const db = new Database(file);
  db
    .prepare('INSERT INTO sections (section_id, term_id, campus_code, index_number) VALUES (?, ?, ?, ?)')
    .run(opts.sectionId, '20251', opts.campus ?? 'NB', opts.index);
  db.close();
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
