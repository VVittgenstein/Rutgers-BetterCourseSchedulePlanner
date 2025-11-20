#!/usr/bin/env tsx
import Database from 'better-sqlite3';
import crypto from 'node:crypto';
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { fileURLToPath } from 'node:url';

import { decodeSemester, performProbe, type SemesterParts, SOCRequestError } from '../scripts/soc_api_client.js';
import { hashPayload } from '../scripts/soc_normalizer.js';

type SubscriptionStatus = 'pending' | 'active' | 'paused' | 'suppressed' | 'unsubscribed';

type Preferences = {
  notifyOn: Array<'open' | 'waitlist'>;
  maxNotifications: number;
  deliveryWindow: {
    startMinutes: number;
    endMinutes: number;
  };
  snoozeUntil: string | null;
  channelMetadata: Record<string, unknown>;
};

export type PollerOptions = {
  term: string;
  campuses: string[];
  intervalMs: number;
  jitter: number;
  sqliteFile: string;
  timeoutMs: number;
  concurrency: number;
  subscriptionChunkSize: number;
  metricsPort: number | null;
  missThreshold: number;
  runOnce: boolean;
  checkpointFile: string;
};

type SectionRow = {
  section_id: number;
  index_number: string;
  is_open: number;
  open_status: string | null;
  open_status_updated_at: string | null;
  section_number: string | null;
  subject_code: string;
  course_title: string;
};

type SubscriptionRow = {
  subscription_id: number;
  status: SubscriptionStatus;
  metadata: string | null;
  last_known_section_status: string | null;
  contact_type: string;
  contact_value: string | null;
};

export type Metrics = {
  pollsTotal: number;
  pollsFailed: number;
  eventsEmitted: number;
  notificationsQueued: number;
  campus: Record<
    string,
    {
      pollsTotal: number;
      pollsFailed: number;
      eventsTotal: number;
      notificationsTotal: number;
      lastDurationMs: number;
      lastOpenCount: number;
    }
  >;
};

type Statements = {
  selectSections: Database.Statement;
  updateSectionStatus: Database.Statement;
  insertStatusEvent: Database.Statement;
  deleteSnapshot: Database.Statement;
  insertSnapshot: Database.Statement;
  selectRecentEvent: Database.Statement;
  insertOpenEvent: Database.Statement;
  selectSubscriptionsPage: Database.Statement;
  insertNotification: Database.Statement;
  updateSubscriptionStatus: Database.Statement;
  resetSubscriptionsForIndex: Database.Statement;
};

export type PollOutcome = {
  opened: number;
  closed: number;
  events: number;
  notifications: number;
  openCount: number;
  snapshotHash: string;
  polledAt: string;
  misses: Map<string, number>;
};

const ACTIVE_STATUSES: SubscriptionStatus[] = ['pending', 'active'];

const defaultPreferences: Preferences = {
  notifyOn: ['open'],
  maxNotifications: 3,
  deliveryWindow: {
    startMinutes: 0,
    endMinutes: 1440,
  },
  snoozeUntil: null,
  channelMetadata: {},
};

type CampusCheckpoint = {
  term: string;
  campus: string;
  lastPollAt: string;
  lastSnapshotHash: string | null;
  openIndexes: number;
  misses: Record<string, number>;
};

type CheckpointFile = {
  version: 1;
  updatedAt: string;
  campuses: Record<string, CampusCheckpoint>;
};

export type CheckpointState = {
  path: string;
  data: CheckpointFile;
};

export type PollerContext = {
  options: PollerOptions;
  term: SemesterParts;
  db: Database.Database;
  statements: Statements;
  missCounters: Map<string, Map<string, number>>;
  metrics: Metrics;
  checkpoint: CheckpointState;
};

class Semaphore {
  private active = 0;
  private queue: Array<() => void> = [];

  constructor(private readonly limit: number) {}

  acquire(): Promise<void> {
    if (this.active < this.limit) {
      this.active += 1;
      return Promise.resolve();
    }
    return new Promise((resolve) => {
      this.queue.push(resolve);
    });
  }

  release(): void {
    this.active -= 1;
    const next = this.queue.shift();
    if (next) {
      this.active += 1;
      next();
    }
  }

  async run<T>(fn: () => Promise<T>): Promise<T> {
    await this.acquire();
    try {
      return await fn();
    } finally {
      this.release();
    }
  }
}

function parseArgs(argv: string[]): PollerOptions {
  const defaults: PollerOptions = {
    term: '12024',
    campuses: ['NB'],
    intervalMs: 60000,
    jitter: 0.3,
    sqliteFile: path.resolve('data', 'local.db'),
    timeoutMs: 12000,
    concurrency: 3,
    subscriptionChunkSize: 200,
    metricsPort: null,
    missThreshold: 2,
    runOnce: false,
    checkpointFile: path.resolve('scripts', 'poller_checkpoint.json'),
  };

  const options = { ...defaults };

  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (!token.startsWith('--')) {
      throw new Error(`Unexpected argument: ${token}`);
    }
    const key = token.slice(2);
    const next = argv[i + 1];
    switch (key) {
      case 'term':
        if (!next) throw new Error('Missing value for --term');
        options.term = next;
        i += 1;
        break;
      case 'campuses':
        if (!next) throw new Error('Missing value for --campuses');
        options.campuses = parseList(next).map((item) => item.toUpperCase());
        if (options.campuses.length === 0) {
          throw new Error('Provide at least one campus code');
        }
        i += 1;
        break;
      case 'interval':
        if (!next) throw new Error('Missing value for --interval');
        options.intervalMs = Number.parseInt(next, 10) * 1000;
        if (!Number.isFinite(options.intervalMs) || options.intervalMs <= 0) {
          throw new Error('--interval must be a positive number of seconds');
        }
        i += 1;
        break;
      case 'interval-ms':
        if (!next) throw new Error('Missing value for --interval-ms');
        options.intervalMs = Number.parseInt(next, 10);
        if (!Number.isFinite(options.intervalMs) || options.intervalMs <= 0) {
          throw new Error('--interval-ms must be a positive integer');
        }
        i += 1;
        break;
      case 'sqlite':
        if (!next) throw new Error('Missing value for --sqlite');
        options.sqliteFile = path.resolve(next);
        i += 1;
        break;
      case 'timeout':
        if (!next) throw new Error('Missing value for --timeout');
        options.timeoutMs = Number.parseInt(next, 10);
        if (!Number.isFinite(options.timeoutMs) || options.timeoutMs <= 0) {
          throw new Error('--timeout must be a positive integer');
        }
        i += 1;
        break;
      case 'concurrency':
        if (!next) throw new Error('Missing value for --concurrency');
        options.concurrency = Number.parseInt(next, 10);
        if (!Number.isFinite(options.concurrency) || options.concurrency <= 0) {
          throw new Error('--concurrency must be a positive integer');
        }
        i += 1;
        break;
      case 'chunk':
        if (!next) throw new Error('Missing value for --chunk');
        options.subscriptionChunkSize = Number.parseInt(next, 10);
        if (!Number.isFinite(options.subscriptionChunkSize) || options.subscriptionChunkSize <= 0) {
          throw new Error('--chunk must be a positive integer');
        }
        i += 1;
        break;
      case 'metrics-port':
        if (!next) throw new Error('Missing value for --metrics-port');
        options.metricsPort = Number.parseInt(next, 10);
        if (!Number.isFinite(options.metricsPort) || options.metricsPort <= 0) {
          throw new Error('--metrics-port must be a positive integer');
        }
        i += 1;
        break;
      case 'miss-threshold':
        if (!next) throw new Error('Missing value for --miss-threshold');
        options.missThreshold = Number.parseInt(next, 10);
        if (!Number.isFinite(options.missThreshold) || options.missThreshold <= 0) {
          throw new Error('--miss-threshold must be a positive integer');
        }
        i += 1;
        break;
      case 'once':
        options.runOnce = true;
        break;
      case 'checkpoint':
        if (!next) throw new Error('Missing value for --checkpoint');
        options.checkpointFile = path.resolve(next);
        i += 1;
        break;
      case 'help':
        showUsage();
        process.exit(0);
      default:
        throw new Error(`Unknown flag: --${key}`);
    }
  }

  return options;
}

function showUsage(): void {
  console.log(`openSections poller

Usage:
  tsx workers/open_sections_poller.ts --term 12024 --campuses NB,NK [--interval 60] [--sqlite data/local.db]

Flags:
  --term <id>            Semester code (e.g. 12024)
  --campuses <list>      Comma list of campus codes (NB,NK,CM)
  --interval <sec>       Base interval between polls in seconds (default: 60)
  --interval-ms <ms>     Base interval in milliseconds (overrides --interval)
  --sqlite <path>        SQLite database file (default: data/local.db)
  --timeout <ms>         HTTP timeout for openSections (default: 12000)
  --concurrency <n>      Max parallel polls (default: 3)
  --chunk <n>            Subscription page size for fan-out (default: 200)
  --metrics-port <port>  Expose Prometheus metrics on /metrics
  --miss-threshold <n>   Consecutive misses before marking Closed (default: 2)
  --checkpoint <path>    Where to persist per-campus poll checkpoints
  --once                 Run a single poll per campus then exit
  --help                 Show this message
`);
}

function parseList(value: string): string[] {
  return value
    .split(',')
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
}

function emptyCheckpoint(pathname: string): CheckpointState {
  return {
    path: pathname,
    data: {
      version: 1,
      updatedAt: new Date().toISOString(),
      campuses: {},
    },
  };
}

export function loadCheckpointState(pathname: string): CheckpointState {
  if (!pathname) {
    return emptyCheckpoint('');
  }
  if (!fs.existsSync(pathname)) {
    return emptyCheckpoint(pathname);
  }
  try {
    const raw = fs.readFileSync(pathname, 'utf-8');
    const parsed = JSON.parse(raw) as CheckpointFile;
    if (parsed && parsed.version === 1 && parsed.campuses) {
      return {
        path: pathname,
        data: {
          version: 1,
          updatedAt: parsed.updatedAt ?? new Date().toISOString(),
          campuses: parsed.campuses,
        },
      };
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : 'unknown error';
    console.warn(`Failed to read checkpoint file ${pathname}. Starting fresh. Reason: ${message}`);
  }
  return emptyCheckpoint(pathname);
}

export function hydrateMissCountersFromCheckpoint(ctx: PollerContext, campus: string): void {
  const entry = ctx.checkpoint.data.campuses[campus];
  if (!entry) return;
  if (entry.term !== ctx.options.term) return;
  const misses = new Map<string, number>();
  for (const [index, value] of Object.entries(entry.misses ?? {})) {
    const count = Number(value);
    if (Number.isFinite(count) && count > 0) {
      misses.set(index, count);
    }
  }
  if (misses.size > 0) {
    ctx.missCounters.set(campus, misses);
  }
  const campusMetrics = ctx.metrics.campus[campus];
  if (campusMetrics) {
    campusMetrics.lastOpenCount = entry.openIndexes ?? 0;
  }
  const hash = entry.lastSnapshotHash ?? 'none';
  const restoredMisses = misses.size;
  console.log(`[${campus}] restored checkpoint at ${entry.lastPollAt} (hash=${hash}, misses=${restoredMisses})`);
}

export function persistCheckpoint(ctx: PollerContext, campus: string, outcome: PollOutcome): void {
  const entry: CampusCheckpoint = {
    term: ctx.options.term,
    campus,
    lastPollAt: outcome.polledAt,
    lastSnapshotHash: outcome.snapshotHash,
    openIndexes: outcome.openCount,
    misses: Object.fromEntries(outcome.misses),
  };
  ctx.checkpoint.data.campuses[campus] = entry;
  ctx.checkpoint.data.updatedAt = new Date().toISOString();
  try {
    fs.mkdirSync(path.dirname(ctx.checkpoint.path), { recursive: true });
    fs.writeFileSync(ctx.checkpoint.path, JSON.stringify(ctx.checkpoint.data, null, 2));
  } catch (error) {
    const message = error instanceof Error ? error.message : 'unknown error';
    console.error(`Failed to write checkpoint to ${ctx.checkpoint.path}: ${message}`);
  }
}

export function createStatements(db: Database.Database): Statements {
  return {
    selectSections: db.prepare(
      `
      SELECT s.section_id, s.index_number, s.is_open, s.open_status, s.open_status_updated_at,
             s.section_number, s.subject_code, c.title AS course_title
      FROM sections s
      JOIN courses c ON s.course_id = c.course_id
      WHERE s.term_id = ? AND s.campus_code = ?
    `,
    ),
    updateSectionStatus: db.prepare(
      `
      UPDATE sections
      SET is_open = ?, open_status = ?, open_status_updated_at = ?, updated_at = ?
      WHERE section_id = ?
    `,
    ),
    insertStatusEvent: db.prepare(
      `
      INSERT INTO section_status_events (
        section_id, previous_status, current_status, source, snapshot_term, snapshot_campus, snapshot_received_at
      ) VALUES (?, ?, ?, ?, ?, ?, ?)
    `,
    ),
    deleteSnapshot: db.prepare(
      'DELETE FROM open_section_snapshots WHERE term_id = ? AND campus_code = ? AND index_number = ?',
    ),
    insertSnapshot: db.prepare(
      `
      INSERT INTO open_section_snapshots (term_id, campus_code, index_number, seen_open_at, source_hash)
      VALUES (?, ?, ?, ?, ?)
    `,
    ),
    selectRecentEvent: db.prepare(
      `
      SELECT open_event_id FROM open_events
      WHERE dedupe_key = ?
        AND event_at >= ?
      LIMIT 1
    `,
    ),
    insertOpenEvent: db.prepare(
      `
      INSERT INTO open_events (
        section_id, term_id, campus_code, index_number, status_before, status_after,
        seat_delta, event_at, detected_by, snapshot_id, dedupe_key, trace_id, payload
      ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `,
    ),
    selectSubscriptionsPage: db.prepare(
      `
      SELECT subscription_id, status, metadata, last_known_section_status, contact_type, contact_value
      FROM subscriptions
      WHERE term_id = ?
        AND campus_code = ?
        AND index_number = ?
        AND status IN (?, ?)
      ORDER BY subscription_id
      LIMIT ?
      OFFSET ?
    `,
    ),
    insertNotification: db.prepare(
      `
      INSERT OR IGNORE INTO open_event_notifications (
        open_event_id, subscription_id, dedupe_key, fanout_status, fanout_attempts, created_at
      ) VALUES (?, ?, ?, 'pending', 0, ?)
    `,
    ),
    updateSubscriptionStatus: db.prepare(
      `
      UPDATE subscriptions
      SET last_known_section_status = ?, updated_at = ?
      WHERE subscription_id = ?
    `,
    ),
    resetSubscriptionsForIndex: db.prepare(
      `
      UPDATE subscriptions
      SET last_known_section_status = ?, updated_at = ?
      WHERE term_id = ?
        AND campus_code = ?
        AND index_number = ?
        AND status IN ('pending', 'active')
    `,
    ),
  };
}

function normalizeOpenIndexes(body: unknown): string[] {
  if (!Array.isArray(body)) {
    throw new Error('openSections payload must be an array.');
  }
  const set = new Set<string>();
  for (const entry of body) {
    const value = String(entry).trim();
    if (value.length === 0) continue;
    set.add(value);
  }
  return Array.from(set).sort();
}

function jitteredDelay(baseMs: number, jitter: number): number {
  const spread = baseMs * jitter;
  const delta = (Math.random() * 2 - 1) * spread;
  return Math.max(1000, Math.round(baseMs + delta));
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function buildDedupeKey(term: string, campus: string, index: string, status: string, at: Date): string {
  const bucket = Math.floor(at.getTime() / (5 * 60 * 1000));
  return crypto.createHash('sha1').update(`${term}|${campus}|${index}|${status}|${bucket}`).digest('hex');
}

function minutesSinceMidnight(date: Date): number {
  return date.getHours() * 60 + date.getMinutes();
}

function parsePreferences(metadata: string | null): Preferences {
  if (!metadata) return defaultPreferences;
  try {
    const parsed = JSON.parse(metadata) as { preferences?: Partial<Preferences> };
    if (parsed && typeof parsed === 'object' && parsed.preferences) {
      const merged: Preferences = {
        ...defaultPreferences,
        ...parsed.preferences,
        deliveryWindow: {
          ...defaultPreferences.deliveryWindow,
          ...(parsed.preferences.deliveryWindow ?? {}),
        },
        notifyOn: parsed.preferences.notifyOn ?? defaultPreferences.notifyOn,
      };
      return merged;
    }
  } catch {
    // best effort
  }
  return defaultPreferences;
}

function shouldNotify(row: SubscriptionRow, now: Date): boolean {
  if (!ACTIVE_STATUSES.includes(row.status)) return false;
  if (row.last_known_section_status && row.last_known_section_status.toUpperCase() === 'OPEN') return false;
  const prefs = parsePreferences(row.metadata);
  if (!prefs.notifyOn.includes('open')) return false;
  if (prefs.snoozeUntil) {
    const snooze = new Date(prefs.snoozeUntil);
    if (!Number.isNaN(snooze.getTime()) && snooze.getTime() > now.getTime()) {
      return false;
    }
  }
  const minutes = minutesSinceMidnight(now);
  if (minutes < prefs.deliveryWindow.startMinutes || minutes > prefs.deliveryWindow.endMinutes) {
    return false;
  }
  return true;
}

function renderMetrics(metrics: Metrics): string {
  const lines: string[] = [];
  lines.push('# TYPE poller_polls_total counter');
  lines.push('# TYPE poller_poll_failures_total counter');
  lines.push('# TYPE poller_events_emitted_total counter');
  lines.push('# TYPE poller_notifications_queued_total counter');
  lines.push('# TYPE poller_last_duration_ms gauge');
  lines.push('# TYPE poller_last_open_indexes gauge');
  for (const [campus, entry] of Object.entries(metrics.campus)) {
    lines.push(`poller_polls_total{campus="${campus}"} ${entry.pollsTotal}`);
    lines.push(`poller_poll_failures_total{campus="${campus}"} ${entry.pollsFailed}`);
    lines.push(`poller_events_emitted_total{campus="${campus}"} ${entry.eventsTotal}`);
    lines.push(`poller_notifications_queued_total{campus="${campus}"} ${entry.notificationsTotal}`);
    lines.push(`poller_last_duration_ms{campus="${campus}"} ${entry.lastDurationMs.toFixed(0)}`);
    lines.push(`poller_last_open_indexes{campus="${campus}"} ${entry.lastOpenCount}`);
  }
  lines.push(`# last scrape totals`);
  lines.push(`poller_events_emitted_total{campus="all"} ${metrics.eventsEmitted}`);
  lines.push(`poller_notifications_queued_total{campus="all"} ${metrics.notificationsQueued}`);
  lines.push(`poller_polls_total{campus="all"} ${metrics.pollsTotal}`);
  lines.push(`poller_poll_failures_total{campus="all"} ${metrics.pollsFailed}`);
  return lines.join('\n');
}

function startMetricsServer(port: number, metrics: Metrics): http.Server {
  const server = http.createServer((req, res) => {
    if (req.url === '/metrics') {
      const body = renderMetrics(metrics);
      res.writeHead(200, { 'content-type': 'text/plain; version=0.0.4' });
      res.end(body);
      return;
    }
    res.statusCode = 404;
    res.end('not found');
  });
  server.listen(port, () => {
    console.log(`Metrics listening on http://localhost:${port}/metrics`);
  });
  return server;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const term = decodeSemester(options.term);
  const metrics: Metrics = {
    pollsTotal: 0,
    pollsFailed: 0,
    eventsEmitted: 0,
    notificationsQueued: 0,
    campus: {},
  };
  for (const campus of options.campuses) {
    metrics.campus[campus] = {
      pollsTotal: 0,
      pollsFailed: 0,
      eventsTotal: 0,
      notificationsTotal: 0,
      lastDurationMs: 0,
      lastOpenCount: 0,
    };
  }

  const db = new Database(options.sqliteFile, { fileMustExist: true });
  db.pragma('journal_mode = WAL');
  db.pragma('foreign_keys = ON');
  const statements = createStatements(db);
  const semaphore = new Semaphore(options.concurrency);
  const checkpoint = loadCheckpointState(options.checkpointFile);

  const missCounters = new Map<string, Map<string, number>>();
  const shutdownSignals = ['SIGINT', 'SIGTERM'] as const;
  let shuttingDown = false;

  shutdownSignals.forEach((signal) => {
    process.on(signal, () => {
      if (shuttingDown) return;
      shuttingDown = true;
      console.log(`Received ${signal}, shutting down...`);
      db.close();
      process.exit(0);
    });
  });

  const server = options.metricsPort ? startMetricsServer(options.metricsPort, metrics) : null;

  const context: PollerContext = {
    options,
    term,
    db,
    statements,
    missCounters,
    metrics,
    checkpoint,
  };

  for (const campus of options.campuses) {
    hydrateMissCountersFromCheckpoint(context, campus);
  }

  console.log(
    `Starting openSections poller for term=${options.term} campuses=${options.campuses.join(',')} interval=${options.intervalMs}ms (jitter=${options.jitter}) sqlite=${options.sqliteFile} checkpoint=${options.checkpointFile}`,
  );

  await Promise.all(
    options.campuses.map((campus) =>
      pollLoop(context, campus, semaphore).catch((error) => {
        console.error(`Loop for campus ${campus} failed:`, error);
      }),
    ),
  );

  if (server) {
    server.close();
  }
  db.close();
}

async function pollLoop(ctx: PollerContext, campus: string, semaphore: Semaphore): Promise<void> {
  do {
    if (ctx.options.runOnce && ctx.metrics.campus[campus].pollsTotal > 0) {
      break;
    }
    await semaphore.run(async () => {
      await pollOnce(ctx, campus);
    });
    if (ctx.options.runOnce) {
      break;
    }
    const delay = jitteredDelay(ctx.options.intervalMs, ctx.options.jitter);
    await sleep(delay);
  } while (true);
}

async function pollOnce(ctx: PollerContext, campus: string): Promise<void> {
  const started = performance.now();
  ctx.metrics.pollsTotal += 1;
  ctx.metrics.campus[campus].pollsTotal += 1;

  try {
    const response = await performProbe({ campus, endpoint: 'openSections', timeoutMs: ctx.options.timeoutMs }, ctx.term);
    const indexes = normalizeOpenIndexes(response.body);
    const outcome = applySnapshot(ctx, campus, indexes);
    persistCheckpoint(ctx, campus, outcome);
    const durationMs = performance.now() - started;
    const campusEntry = ctx.metrics.campus[campus];
    ctx.metrics.eventsEmitted += outcome.events;
    ctx.metrics.notificationsQueued += outcome.notifications;
    campusEntry.eventsTotal += outcome.events;
    campusEntry.notificationsTotal += outcome.notifications;
    campusEntry.lastDurationMs = durationMs;
    campusEntry.lastOpenCount = outcome.openCount;
    console.log(
      `[${campus}] openSections=${indexes.length} opened=${outcome.opened} closed=${outcome.closed} events=${outcome.events} notifications=${outcome.notifications} durationMs=${durationMs.toFixed(
        0,
      )}`,
    );
  } catch (error) {
    ctx.metrics.pollsFailed += 1;
    ctx.metrics.campus[campus].pollsFailed += 1;
    const durationMs = performance.now() - started;
    ctx.metrics.campus[campus].lastDurationMs = durationMs;
    if (error instanceof SOCRequestError) {
      console.error(
        `[${campus}] openSections failed (${error.requestId}): ${error.message} [${error.kind}] retryHint=${error.retryHint ?? 'n/a'}`,
      );
    } else if (error instanceof Error) {
      console.error(`[${campus}] openSections failed: ${error.message}`);
    } else {
      console.error(`[${campus}] openSections failed due to unknown error`);
    }
  }
}

export function applySnapshot(ctx: PollerContext, campus: string, indexes: string[], now: Date = new Date()): PollOutcome {
  const nowIso = now.toISOString();
  const sourceHash = hashPayload({ term: ctx.options.term, campus, indexes });
  const seen = new Set(indexes);
  const missSet = ctx.missCounters.get(campus) ?? new Map<string, number>();
  const sections = ctx.statements.selectSections.all(ctx.options.term, campus) as SectionRow[];

  const toOpen: SectionRow[] = [];
  const toClose: SectionRow[] = [];

  for (const section of sections) {
    const wasOpen = section.is_open === 1;
    const isOpenNow = seen.has(section.index_number);
    if (isOpenNow) {
      if (!wasOpen) {
        toOpen.push(section);
      }
      missSet.delete(section.index_number);
    } else if (wasOpen) {
      const misses = (missSet.get(section.index_number) ?? 0) + 1;
      if (misses >= ctx.options.missThreshold) {
        toClose.push(section);
        missSet.delete(section.index_number);
      } else {
        missSet.set(section.index_number, misses);
      }
    }
  }
  ctx.missCounters.set(campus, missSet);

  let events = 0;
  let notifications = 0;

  const tx = ctx.db.transaction(() => {
    for (const index of indexes) {
      ctx.statements.deleteSnapshot.run(ctx.options.term, campus, index);
      ctx.statements.insertSnapshot.run(ctx.options.term, campus, index, nowIso, sourceHash);
    }

    for (const section of toOpen) {
      const previous = section.open_status ?? (section.is_open === 1 ? 'OPEN' : 'CLOSED');
      ctx.statements.updateSectionStatus.run(1, 'OPEN', nowIso, nowIso, section.section_id);
      ctx.statements.insertStatusEvent.run(
        section.section_id,
        previous,
        'OPEN',
        'openSections',
        ctx.options.term,
        campus,
        nowIso,
      );
      const eventOutcome = createEventAndFanout(ctx, {
        section,
        campus,
        statusBefore: previous,
        statusAfter: 'OPEN',
        seatDelta: previous === 'OPEN' ? 0 : 1,
        eventAt: nowIso,
        snapshotHash: sourceHash,
      });
      events += eventOutcome.events;
      notifications += eventOutcome.notifications;
    }

    for (const section of toClose) {
      const previous = section.open_status ?? 'OPEN';
      ctx.statements.updateSectionStatus.run(0, 'CLOSED', nowIso, nowIso, section.section_id);
      ctx.statements.insertStatusEvent.run(
        section.section_id,
        previous,
        'CLOSED',
        'openSections',
        ctx.options.term,
        campus,
        nowIso,
      );
      ctx.statements.deleteSnapshot.run(ctx.options.term, campus, section.index_number);
      ctx.statements.resetSubscriptionsForIndex.run(
        'CLOSED',
        nowIso,
        ctx.options.term,
        campus,
        section.index_number,
      );
      const eventOutcome = createEventAndFanout(ctx, {
        section,
        campus,
        statusBefore: previous,
        statusAfter: 'CLOSED',
        seatDelta: -1,
        eventAt: nowIso,
        snapshotHash: sourceHash,
      });
      events += eventOutcome.events;
      notifications += eventOutcome.notifications;
    }
  });

  tx();

  return {
    opened: toOpen.length,
    closed: toClose.length,
    events,
    notifications,
    openCount: indexes.length,
    snapshotHash: sourceHash,
    polledAt: nowIso,
    misses: missSet,
  };
}

function createEventAndFanout(
  ctx: {
    options: PollerOptions;
    db: Database.Database;
    statements: Statements;
  },
  args: {
    section: SectionRow;
    campus: string;
    statusBefore: string;
    statusAfter: string;
    seatDelta: number;
    eventAt: string;
    snapshotHash: string;
  },
): { events: number; notifications: number } {
  const eventTime = new Date(args.eventAt);
  const dedupeKey = buildDedupeKey(ctx.options.term, args.campus, args.section.index_number, args.statusAfter, eventTime);
  const cutoff = new Date(eventTime.getTime() - 5 * 60 * 1000).toISOString();
  const existing = ctx.statements.selectRecentEvent.get(dedupeKey, cutoff) as { open_event_id: number } | undefined;
  if (existing) {
    return { events: 0, notifications: 0 };
  }

  const traceId = crypto.randomUUID();
  const payload = {
    term: ctx.options.term,
    campus: args.campus,
    index: args.section.index_number,
    sectionNumber: args.section.section_number,
    subject: args.section.subject_code,
    courseTitle: args.section.course_title,
    detectedAt: args.eventAt,
    snapshotHash: args.snapshotHash,
  };

  const result = ctx.statements.insertOpenEvent.run(
    args.section.section_id,
    ctx.options.term,
    args.campus,
    args.section.index_number,
    args.statusBefore,
    args.statusAfter,
    args.seatDelta,
    args.eventAt,
    'openSections',
    null,
    dedupeKey,
    traceId,
    JSON.stringify(payload),
  );

  const eventId = Number(result.lastInsertRowid);
  let notifications = 0;

  if (args.statusAfter === 'OPEN') {
    notifications = enqueueNotifications(ctx, {
      campus: args.campus,
      index: args.section.index_number,
      eventId,
      dedupeKey,
      eventAt: args.eventAt,
    });
  }

  return { events: 1, notifications };
}

function enqueueNotifications(
  ctx: {
    options: PollerOptions;
    statements: Statements;
  },
  args: { campus: string; index: string; eventId: number; dedupeKey: string; eventAt: string },
): number {
  let offset = 0;
  let created = 0;
  const now = args.eventAt;
  while (true) {
    const rows = ctx.statements.selectSubscriptionsPage.all(
      ctx.options.term,
      args.campus,
      args.index,
      ACTIVE_STATUSES[0],
      ACTIVE_STATUSES[1],
      ctx.options.subscriptionChunkSize,
      offset,
    ) as SubscriptionRow[];
    if (rows.length === 0) break;
    offset += rows.length;
    for (const sub of rows) {
      if (!shouldNotify(sub, new Date(now))) {
        continue;
      }
      const result = ctx.statements.insertNotification.run(
        args.eventId,
        sub.subscription_id,
        args.dedupeKey,
        now,
      );
      if (result.changes > 0) {
        ctx.statements.updateSubscriptionStatus.run('OPEN', now, sub.subscription_id);
        created += 1;
      }
    }
    if (rows.length < ctx.options.subscriptionChunkSize) {
      break;
    }
  }
  return created;
}

const isDirectRun = process.argv[1] === fileURLToPath(import.meta.url);

if (isDirectRun) {
  void main().catch((error) => {
    console.error('Poller failed to start:', error);
    process.exit(1);
  });
}
