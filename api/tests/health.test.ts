import test from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';
import os from 'node:os';
import path from 'node:path';
import { mkdtempSync, rmSync } from 'node:fs';

import { createServer } from '../src/server.js';
import { REQUIRED_SCHEMA_TABLES } from '../src/routes/health.js';

const TABLE_DDL: Record<(typeof REQUIRED_SCHEMA_TABLES)[number], string> = {
  courses: 'CREATE TABLE courses (course_id INTEGER PRIMARY KEY, term_id TEXT)',
  sections: 'CREATE TABLE sections (section_id INTEGER PRIMARY KEY, course_id INTEGER)',
  course_core_attributes: 'CREATE TABLE course_core_attributes (course_id INTEGER, core_code TEXT)',
  course_search_fts: 'CREATE TABLE course_search_fts (course_id INTEGER)',
  section_meetings: 'CREATE TABLE section_meetings (section_id INTEGER, start_minutes INTEGER)',
  subjects: 'CREATE TABLE subjects (code TEXT PRIMARY KEY)',
};

test('ready endpoint reports healthy schema', async (t) => {
  const fixture = createTempSqlite();
  const restoreEnv = overrideEnv('SQLITE_FILE', fixture.file);
  const server = await createServer();
  t.after(async () => {
    await server.close();
    restoreEnv();
    fixture.cleanup();
  });

  const response = await server.inject({ method: 'GET', url: '/api/ready' });
  assert.equal(response.statusCode, 200);
  const payload = response.json();
  assert.equal(payload.status, 'ready');
  assert.equal(payload.checks.sqlite.status, 'up');
  assert.equal(payload.checks.tables.status, 'up');
});

test('ready endpoint surfaces missing tables and uses 503', async (t) => {
  const fixture = createTempSqlite(['sections']);
  const restoreEnv = overrideEnv('SQLITE_FILE', fixture.file);
  const server = await createServer();
  t.after(async () => {
    await server.close();
    restoreEnv();
    fixture.cleanup();
  });

  const response = await server.inject({ method: 'GET', url: '/api/ready' });
  assert.equal(response.statusCode, 503);
  const payload = response.json();
  assert.equal(payload.status, 'not_ready');
  assert.equal(payload.checks.sqlite.status, 'up');
  assert.equal(payload.checks.tables.status, 'down');
  assert.ok(payload.checks.tables.missing.includes('sections'));
});

test('validation errors include trace id in header and payload', async (t) => {
  const fixture = createTempSqlite();
  const restoreEnv = overrideEnv('SQLITE_FILE', fixture.file);
  const server = await createServer();
  t.after(async () => {
    await server.close();
    restoreEnv();
    fixture.cleanup();
  });

  const response = await server.inject({
    method: 'GET',
    url: '/api/courses?term=20251',
  });
  assert.equal(response.statusCode, 400);
  const payload = response.json();
  assert.ok(payload.error.traceId);
  const headerTraceId = response.headers['x-trace-id'] as string | undefined;
  assert.ok(headerTraceId);
  assert.equal(payload.error.traceId, headerTraceId);
});

function createTempSqlite(skip: string[] = []) {
  const directory = mkdtempSync(path.join(os.tmpdir(), 'query-api-health-'));
  const file = path.join(directory, 'db.sqlite');
  const db = new Database(file);
  for (const table of REQUIRED_SCHEMA_TABLES) {
    if (skip.includes(table)) {
      continue;
    }
    db.prepare(TABLE_DDL[table]).run();
  }
  db.close();
  return {
    file,
    cleanup: () => {
      rmSync(directory, { recursive: true, force: true });
    },
  };
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
