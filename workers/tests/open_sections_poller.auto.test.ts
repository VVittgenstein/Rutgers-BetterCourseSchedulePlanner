import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import Database from 'better-sqlite3';

import {
  createStatements,
  discoverSubscriptionTargets,
  loadCheckpointState,
  parseArgs,
  syncTargetLoops,
  type Metrics,
  type PollerContext,
  type PollTarget,
  type PollerOptions,
} from '../open_sections_poller.js';
import { decodeSemester, performProbe } from '../../scripts/soc_api_client.js';

function makeBaseOptions(overrides: Partial<PollerOptions> = {}): PollerOptions {
  return {
    openReminderIntervalMs: 3 * 60 * 1000,
    termsMode: 'auto',
    terms: [],
    campuses: [],
    intervalMs: 15000,
    refreshIntervalMs: 5 * 60 * 1000,
    jitter: 0.3,
    sqliteFile: path.resolve('data', 'local.db'),
    timeoutMs: 12000,
    concurrency: 1,
    subscriptionChunkSize: 50,
    metricsPort: null,
    missThreshold: 2,
    runOnce: false,
    checkpointFile: path.resolve('scripts', 'poller_checkpoint.json'),
    ...overrides,
  };
}

function buildAutoContext(tempDir: string, allowlist: string[] = []): { ctx: PollerContext; db: Database.Database } {
  const dbPath = path.join(tempDir, 'local.db');
  const db = new Database(dbPath);
  db.pragma('journal_mode = WAL');
  db.exec(fs.readFileSync(path.resolve('data', 'schema.sql'), 'utf-8'));

  const options: PollerOptions = makeBaseOptions({
    campuses: allowlist,
    sqliteFile: dbPath,
    checkpointFile: path.join(tempDir, 'checkpoint.json'),
  });

  const metrics: Metrics = { pollsTotal: 0, pollsFailed: 0, eventsEmitted: 0, notificationsQueued: 0, targets: {} };
  const ctx: PollerContext = {
    options,
    db,
    statements: createStatements(db),
    probeFn: performProbe,
    missCounters: new Map(),
    metrics,
    checkpoint: loadCheckpointState(options.checkpointFile),
    datasetStatus: new Map(),
  };
  return { ctx, db };
}

function seedSubscription(db: Database.Database, term: string, campus: string, status: string, idx = '10001') {
  db.prepare(
    `
    INSERT INTO subscriptions (term_id, campus_code, index_number, contact_type, contact_value, contact_hash, status, created_at, updated_at)
    VALUES (?, ?, ?, 'email', ?, ?, ?, datetime('now'), datetime('now'))
  `,
  ).run(term, campus, idx, `${campus}@example.edu`, `${term}-${campus}-${idx}`, status);
}

test('parseArgs defaults to auto mode with refresh interval', () => {
  const opts = parseArgs([]);
  assert.equal(opts.termsMode, 'auto');
  assert.deepEqual(opts.campuses, []);
  assert.equal(opts.refreshIntervalMs, 5 * 60 * 1000);
  assert.equal(opts.intervalMs, 15000);
});

test('parseArgs preserves legacy campus default for explicit term', () => {
  const opts = parseArgs(['--term', '12024']);
  assert.equal(opts.termsMode, 'explicit');
  assert.deepEqual(opts.terms, ['12024']);
  assert.deepEqual(opts.campuses, ['NB']);
});

test('parseArgs rejects sub-minute refresh interval', () => {
  assert.throws(() => parseArgs(['--refresh-interval-mins', '0']), /at least 1 minute/);
});

test('discoverSubscriptionTargets collects active subscriptions and applies campus filter', () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'poller-auto-'));
  try {
    const { ctx, db } = buildAutoContext(tempDir);
    seedSubscription(db, '12025', 'NB', 'active', '11111');
    seedSubscription(db, '12026', 'NK', 'pending', '22222');
    seedSubscription(db, '12026', 'NK', 'paused', '33333'); // ignored

    const allTargets = discoverSubscriptionTargets(ctx);
    assert.deepEqual(
      allTargets.map((t) => `${t.termId}/${t.campus}`),
      ['12025/NB', '12026/NK'],
    );

    ctx.options.campuses = ['NB'];
    const filtered = discoverSubscriptionTargets(ctx);
    assert.deepEqual(
      filtered.map((t) => `${t.termId}/${t.campus}`),
      ['12025/NB'],
    );
    db.close();
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

test('syncTargetLoops starts and stops targets based on desired set', async () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'poller-loops-'));
  try {
    const { ctx, db } = buildAutoContext(tempDir);
    const semaphore = { run: async <T>(fn: () => Promise<T>) => fn() } as unknown as any;
    const running = new Map<string, { target: PollTarget; stop: () => void; done: Promise<void> }>();
    const started: string[] = [];
    const stopped: string[] = [];
    const resolvers: Record<string, () => void> = {};

    const starter = (_ctx: PollerContext, target: PollTarget) => {
      const key = `${target.termId}/${target.campus}`;
      started.push(key);
      let resolveDone: (() => void) | null = null;
      const done = new Promise<void>((resolve) => {
        resolveDone = resolve;
      });
      resolvers[key] = () => resolveDone?.();
      const handle = {
        target,
        stop: () => {
          stopped.push(key);
          resolveDone?.();
        },
        done,
      };
      return handle;
    };

    const targetA: PollTarget = { termId: '12025', campus: 'NB', decodedTerm: decodeSemester('12025') };
    const targetB: PollTarget = { termId: '12026', campus: 'NK', decodedTerm: decodeSemester('12026') };

    syncTargetLoops(ctx, semaphore, running, [targetA, targetB], () => true, starter);
    const handleA = running.get('12025|NB');
    const handleB = running.get('12026|NK');
    assert.ok(handleA);
    assert.ok(handleB);
    assert.deepEqual(started, ['12025/NB', '12026/NK']);

    syncTargetLoops(ctx, semaphore, running, [targetB], () => true, starter);
    assert.deepEqual(stopped, ['12025/NB']);

    resolvers['12025/NB']?.();
    await handleA?.done;
    assert.equal(running.has('12025|NB'), false);
    assert.equal(running.has('12026|NK'), true);

    resolvers['12026/NK']?.();
    await handleB?.done;
    assert.equal(running.size, 0);

    db.close();
  } finally {
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});

test('skips polling when sections data is missing and emits a warning', async () => {
  const tempDir = fs.mkdtempSync(path.join(os.tmpdir(), 'poller-missing-'));
  const warnings: string[] = [];
  const originalWarn = console.warn;
  try {
    const { ctx, db } = buildAutoContext(tempDir);
    ctx.options.runOnce = true;
    let probes = 0;
    ctx.probeFn = async () => {
      probes += 1;
      return { body: [], status: 200 } as any;
    };

    const target: PollTarget = { termId: '12025', campus: 'NB', decodedTerm: decodeSemester('12025') };
    const running = new Map<string, { target: PollTarget; stop: () => void; done: Promise<void> }>();
    const semaphore = { run: async <T>(fn: () => Promise<T>) => fn() } as unknown as any;

    console.warn = (...args: unknown[]) => {
      warnings.push(args.join(' '));
    };

    syncTargetLoops(ctx, semaphore, running, [target], () => true);
    const handles = [...running.values()];
    await Promise.all(handles.map((loop) => loop.done));

    assert.equal(probes, 0);
    assert.equal(ctx.metrics.pollsTotal, 0);
    assert.equal(warnings.length, 1);
    assert.ok(warnings[0].includes('no sections'));

    const checkpointRaw = fs.readFileSync(ctx.options.checkpointFile, 'utf-8');
    const checkpoint = JSON.parse(checkpointRaw) as { campuses: Record<string, { lastSnapshotHash: string }> };
    const entry = checkpoint.campuses['12025|NB'];
    assert.ok(entry);
    assert.equal(entry.lastSnapshotHash, 'missing-data');

    db.close();
  } finally {
    console.warn = originalWarn;
    fs.rmSync(tempDir, { recursive: true, force: true });
  }
});
