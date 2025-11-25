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
  openReminderIntervalMs: number;
  termsMode: 'auto' | 'explicit';
  terms: string[];
  campuses: string[];
  intervalMs: number;
  refreshIntervalMs: number;
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
  last_notified_at: string | null;
};

export type Metrics = {
  pollsTotal: number;
  pollsFailed: number;
  eventsEmitted: number;
  notificationsQueued: number;
  targets: Record<
    string,
    {
      term: string;
      campus: string;
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
  countSectionsForTarget: Database.Statement;
  selectSections: Database.Statement;
  updateSectionStatus: Database.Statement;
  insertStatusEvent: Database.Statement;
  deleteSnapshot: Database.Statement;
  insertSnapshot: Database.Statement;
  selectRecentEvent: Database.Statement;
  insertOpenEvent: Database.Statement;
  hasActiveSubscription: Database.Statement;
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
const TARGET_KEY_SEPARATOR = '|';

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

const OPEN_REMINDER_INTERVAL_MS = 3 * 60 * 1000;

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

type DatasetStatus = 'ready' | 'missing';

export type CheckpointState = {
  path: string;
  data: CheckpointFile;
};

export type PollTarget = {
  termId: string;
  campus: string;
  decodedTerm: SemesterParts;
};

export type PollerContext = {
  options: PollerOptions;
  db: Database.Database;
  statements: Statements;
  probeFn: typeof performProbe;
  missCounters: Map<string, Map<string, number>>;
  metrics: Metrics;
  checkpoint: CheckpointState;
  datasetStatus: Map<string, DatasetStatus>;
};

function makeTargetKey(term: string, campus: string): string {
  return `${term}${TARGET_KEY_SEPARATOR}${campus}`;
}

function targetLabel(target: PollTarget): string {
  return `${target.termId}/${target.campus}`;
}

function ensureMetricsTarget(metrics: Metrics, target: PollTarget): void {
  const key = makeTargetKey(target.termId, target.campus);
  if (!metrics.targets[key]) {
    metrics.targets[key] = {
      term: target.termId,
      campus: target.campus,
      pollsTotal: 0,
      pollsFailed: 0,
      eventsTotal: 0,
      notificationsTotal: 0,
      lastDurationMs: 0,
      lastOpenCount: 0,
    };
  }
}

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

export function parseArgs(argv: string[]): PollerOptions {
  const defaults: PollerOptions = {
    openReminderIntervalMs: OPEN_REMINDER_INTERVAL_MS,
    termsMode: 'auto',
    terms: [],
    campuses: [],
    intervalMs: 15000,
    refreshIntervalMs: 5 * 60 * 1000,
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

  const options: PollerOptions = { ...defaults };
  let campusesProvided = false;
  let termsProvided = false;

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
        if (termsProvided) throw new Error('Only one of --term/--terms may be provided');
        options.termsMode = 'explicit';
        options.terms = [next];
        termsProvided = true;
        i += 1;
        break;
      case 'terms':
        if (!next) throw new Error('Missing value for --terms');
        if (termsProvided) throw new Error('Only one of --term/--terms may be provided');
        if (next.toLowerCase() === 'auto') {
          options.termsMode = 'auto';
          options.terms = [];
        } else {
          const terms = parseList(next);
          if (terms.length === 0) {
            throw new Error('Provide at least one term for --terms');
          }
          options.termsMode = 'explicit';
          options.terms = terms;
        }
        termsProvided = true;
        i += 1;
        break;
      case 'campuses':
        if (!next) throw new Error('Missing value for --campuses');
        options.campuses = parseList(next).map((item) => item.toUpperCase());
        if (options.campuses.length === 0) {
          throw new Error('Provide at least one campus code');
        }
        campusesProvided = true;
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
      case 'refresh-interval-mins':
        if (!next) throw new Error('Missing value for --refresh-interval-mins');
        const mins = Number.parseInt(next, 10);
        if (!Number.isFinite(mins) || mins < 1) {
          throw new Error('--refresh-interval-mins must be at least 1 minute');
        }
        options.refreshIntervalMs = mins * 60 * 1000;
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

  if (options.termsMode === 'explicit') {
    if (options.terms.length === 0) {
      throw new Error('Provide at least one term via --terms or --term');
    }
    options.terms = Array.from(new Set(options.terms));
    if (!campusesProvided && options.campuses.length === 0) {
      options.campuses = ['NB'];
    }
    if (options.campuses.length === 0) {
      throw new Error('Provide at least one campus code');
    }
  }

  return options;
}

function showUsage(): void {
  console.log(`openSections poller

Usage:
  tsx workers/open_sections_poller.ts [--terms auto|12024,12026] [--campuses NB,NK] [--interval 15] [--sqlite data/local.db]

Flags:
  --terms <auto|list>          Terms to poll; use auto to read active subscriptions (default: auto)
  --term <id>                  Alias for --terms <id> (backwards compatibility)
  --campuses <list>            Comma list of campus codes; acts as allowlist in auto mode
  --refresh-interval-mins <m>  How often to rescan subscriptions in auto mode (default: 5)
  --interval <sec>             Base interval between polls in seconds (default: 15)
  --interval-ms <ms>           Base interval in milliseconds (overrides --interval)
  --sqlite <path>              SQLite database file (default: data/local.db)
  --timeout <ms>               HTTP timeout for openSections (default: 12000)
  --concurrency <n>            Max parallel polls (default: 3)
  --chunk <n>                  Subscription page size for fan-out (default: 200)
  --metrics-port <port>        Expose Prometheus metrics on /metrics
  --miss-threshold <n>         Consecutive misses before marking Closed (default: 2)
  --checkpoint <path>          Where to persist per-target poll checkpoints
  --once                       Run a single poll per campus then exit
  --help                       Show this message
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
    const campuses = parsed?.campuses;
    const validCampuses = campuses && typeof campuses === 'object' && !Array.isArray(campuses) ? campuses : null;
    if (parsed && parsed.version === 1 && validCampuses) {
      return {
        path: pathname,
        data: {
          version: 1,
          updatedAt: parsed.updatedAt ?? new Date().toISOString(),
          campuses: validCampuses as Record<string, CampusCheckpoint>,
        },
      };
    }
    console.warn(
      `Checkpoint file ${pathname} is missing expected campuses map; discarding corrupted state and starting fresh.`,
    );
  } catch (error) {
    const message = error instanceof Error ? error.message : 'unknown error';
    console.warn(`Failed to read checkpoint file ${pathname}. Starting fresh. Reason: ${message}`);
  }
  return emptyCheckpoint(pathname);
}

export function hydrateMissCountersFromCheckpoint(ctx: PollerContext, target: PollTarget): void {
  const key = makeTargetKey(target.termId, target.campus);
  const entry =
    ctx.checkpoint.data.campuses[key] ?? ctx.checkpoint.data.campuses[target.campus];
  if (!entry) return;
  if (entry.term !== target.termId) return;
  const misses = new Map<string, number>();
  for (const [index, value] of Object.entries(entry.misses ?? {})) {
    const count = Number(value);
    if (Number.isFinite(count) && count > 0) {
      misses.set(index, count);
    }
  }
  if (misses.size > 0) {
    ctx.missCounters.set(key, misses);
  }
  const campusMetrics = ctx.metrics.targets[key];
  if (campusMetrics) {
    campusMetrics.lastOpenCount = entry.openIndexes ?? 0;
  }
  const hash = entry.lastSnapshotHash ?? 'none';
  const restoredMisses = misses.size;
  console.log(
    `[${target.termId}/${target.campus}] restored checkpoint at ${entry.lastPollAt} (hash=${hash}, misses=${restoredMisses})`,
  );
}

export function persistCheckpoint(ctx: PollerContext, target: PollTarget, outcome: PollOutcome): void {
  const entry: CampusCheckpoint = {
    term: target.termId,
    campus: target.campus,
    lastPollAt: outcome.polledAt,
    lastSnapshotHash: outcome.snapshotHash,
    openIndexes: outcome.openCount,
    misses: Object.fromEntries(outcome.misses),
  };
  const key = makeTargetKey(target.termId, target.campus);
  ctx.checkpoint.data.campuses[key] = entry;
  const legacyKey = target.campus;
  const legacyEntry = ctx.checkpoint.data.campuses[legacyKey];
  if (legacyKey !== key && legacyEntry?.term === target.termId) {
    delete ctx.checkpoint.data.campuses[legacyKey];
  }
  ctx.checkpoint.data.updatedAt = new Date().toISOString();
  try {
    fs.mkdirSync(path.dirname(ctx.checkpoint.path), { recursive: true });
    fs.writeFileSync(ctx.checkpoint.path, JSON.stringify(ctx.checkpoint.data, null, 2));
  } catch (error) {
    const message = error instanceof Error ? error.message : 'unknown error';
    console.error(`Failed to write checkpoint to ${ctx.checkpoint.path}: ${message}`);
  }
}

function persistMissingCheckpoint(ctx: PollerContext, target: PollTarget): void {
  const key = makeTargetKey(target.termId, target.campus);
  const misses = ctx.missCounters.get(key) ?? new Map<string, number>();
  const outcome: PollOutcome = {
    opened: 0,
    closed: 0,
    events: 0,
    notifications: 0,
    openCount: 0,
    snapshotHash: 'missing-data',
    polledAt: new Date().toISOString(),
    misses,
  };
  persistCheckpoint(ctx, target, outcome);
}

function setDatasetStatus(ctx: PollerContext, target: PollTarget, status: DatasetStatus): void {
  const key = makeTargetKey(target.termId, target.campus);
  const previous = ctx.datasetStatus.get(key);
  if (previous === status) return;
  ctx.datasetStatus.set(key, status);
  if (status === 'missing') {
    console.warn(
      `[${targetLabel(target)}] no sections found locally; fetch course data for this term/campus before polling.`,
    );
    persistMissingCheckpoint(ctx, target);
  } else if (previous === 'missing') {
    console.log(`[${targetLabel(target)}] detected sections data; resuming polls.`);
  }
}

export function ensureSectionsData(ctx: PollerContext, target: PollTarget): boolean {
  const row = ctx.statements.countSectionsForTarget.get(target.termId, target.campus) as
    | { total?: number; count?: number }
    | undefined;
  const total = Number(row?.total ?? row?.count ?? 0);
  if (!Number.isFinite(total) || total <= 0) {
    setDatasetStatus(ctx, target, 'missing');
    return false;
  }
  setDatasetStatus(ctx, target, 'ready');
  return true;
}

export function createStatements(db: Database.Database): Statements {
  return {
    countSectionsForTarget: db.prepare(
      `
      SELECT COUNT(*) AS total
      FROM sections
      WHERE term_id = ?
        AND campus_code = ?
    `,
    ),
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
    hasActiveSubscription: db.prepare(
      `
      SELECT 1
      FROM subscriptions
      WHERE term_id = ?
        AND campus_code = ?
        AND index_number = ?
        AND status IN (?, ?)
      LIMIT 1
    `,
    ),
    selectSubscriptionsPage: db.prepare(
      `
      SELECT subscription_id, status, metadata, last_known_section_status, contact_type, contact_value, last_notified_at
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
  const bucket = Math.floor(at.getTime() / (3 * 60 * 1000));
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

function shouldNotify(row: SubscriptionRow, now: Date, reminderIntervalMs: number): boolean {
  if (!ACTIVE_STATUSES.includes(row.status)) return false;
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
  if (row.last_notified_at) {
    const last = new Date(row.last_notified_at);
    if (!Number.isNaN(last.getTime()) && now.getTime() - last.getTime() < reminderIntervalMs) {
      return false;
    }
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
  for (const entry of Object.values(metrics.targets)) {
    const labels = `term="${entry.term}",campus="${entry.campus}"`;
    lines.push(`poller_polls_total{${labels}} ${entry.pollsTotal}`);
    lines.push(`poller_poll_failures_total{${labels}} ${entry.pollsFailed}`);
    lines.push(`poller_events_emitted_total{${labels}} ${entry.eventsTotal}`);
    lines.push(`poller_notifications_queued_total{${labels}} ${entry.notificationsTotal}`);
    lines.push(`poller_last_duration_ms{${labels}} ${entry.lastDurationMs.toFixed(0)}`);
    lines.push(`poller_last_open_indexes{${labels}} ${entry.lastOpenCount}`);
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

type LoopHandle = {
  target: PollTarget;
  stop: () => void;
  done: Promise<void>;
};

export function resolveExplicitTargets(options: PollerOptions): PollTarget[] {
  const targets: PollTarget[] = [];
  for (const termId of options.terms) {
    const decodedTerm = decodeSemester(termId);
    for (const campus of options.campuses) {
      targets.push({ termId, campus, decodedTerm });
    }
  }
  return targets.sort((a, b) =>
    a.termId === b.termId ? a.campus.localeCompare(b.campus) : a.termId.localeCompare(b.termId),
  );
}

export function discoverSubscriptionTargets(ctx: PollerContext): PollTarget[] {
  const rows = ctx.db
    .prepare(
      `
      SELECT term_id AS termId, campus_code AS campus
      FROM subscriptions
      WHERE status IN (?, ?)
        AND term_id IS NOT NULL
        AND campus_code IS NOT NULL
      GROUP BY term_id, campus_code
    `,
    )
    .all(ACTIVE_STATUSES[0], ACTIVE_STATUSES[1]) as Array<{ termId: string; campus: string }>;

  const allowCampuses = ctx.options.campuses;
  const targets: PollTarget[] = [];
  for (const row of rows) {
    const termId = String(row.termId ?? '').trim();
    const campus = String(row.campus ?? '').trim().toUpperCase();
    if (!termId || !campus) continue;
    if (allowCampuses.length > 0 && !allowCampuses.includes(campus)) continue;
    const key = makeTargetKey(termId, campus);
    if (targets.some((target) => makeTargetKey(target.termId, target.campus) === key)) {
      continue;
    }
    try {
      targets.push({ termId, campus, decodedTerm: decodeSemester(termId) });
    } catch (error) {
      const message = error instanceof Error ? error.message : 'invalid term';
      console.warn(`Skipping subscription target ${termId}/${campus}: ${message}`);
    }
  }

  return targets.sort((a, b) =>
    a.termId === b.termId ? a.campus.localeCompare(b.campus) : a.termId.localeCompare(b.termId),
  );
}

function startTargetLoop(
  ctx: PollerContext,
  target: PollTarget,
  semaphore: Semaphore,
  shouldContinue: () => boolean,
): LoopHandle {
  let active = true;
  const stop = () => {
    active = false;
  };
  const done = pollLoop(ctx, target, semaphore, () => active && shouldContinue()).catch((error) => {
    console.error(`[${targetLabel(target)}] loop failed:`, error);
  });
  return { target, stop, done };
}

export function syncTargetLoops(
  ctx: PollerContext,
  semaphore: Semaphore,
  running: Map<string, LoopHandle>,
  desiredTargets: PollTarget[],
  shouldContinue: () => boolean,
  startLoop: typeof startTargetLoop = startTargetLoop,
): void {
  const desiredKeys = new Set<string>();
  for (const target of desiredTargets) {
    const key = makeTargetKey(target.termId, target.campus);
    desiredKeys.add(key);
    if (running.has(key)) continue;
    ensureMetricsTarget(ctx.metrics, target);
    hydrateMissCountersFromCheckpoint(ctx, target);
    const handle = startLoop(ctx, target, semaphore, shouldContinue);
    running.set(key, handle);
    void handle.done.finally(() => {
      if (running.get(key) === handle) {
        running.delete(key);
      }
    });
    console.log(
      `Started loop for ${targetLabel(target)} interval=${ctx.options.intervalMs}ms jitter=${ctx.options.jitter}`,
    );
  }

  for (const [key, handle] of running.entries()) {
    if (!desiredKeys.has(key)) {
      handle.stop();
      console.log(`Stopping loop for ${targetLabel(handle.target)} (no longer matched)`);
    }
  }
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const metrics: Metrics = {
    pollsTotal: 0,
    pollsFailed: 0,
    eventsEmitted: 0,
    notificationsQueued: 0,
    targets: {},
  };

  const db = new Database(options.sqliteFile, { fileMustExist: true });
  db.pragma('journal_mode = WAL');
  db.pragma('foreign_keys = ON');
  const statements = createStatements(db);
  const semaphore = new Semaphore(options.concurrency);
  const checkpoint = loadCheckpointState(options.checkpointFile);

  const missCounters = new Map<string, Map<string, number>>();
  const shutdownSignals = ['SIGINT', 'SIGTERM'] as const;
  let shuttingDown = false;

  const server = options.metricsPort ? startMetricsServer(options.metricsPort, metrics) : null;
  const runningLoops = new Map<string, LoopHandle>();

  const stopAll = () => {
    runningLoops.forEach((loop) => loop.stop());
  };

  shutdownSignals.forEach((signal) => {
    process.on(signal, () => {
      if (shuttingDown) return;
      shuttingDown = true;
      console.log(`Received ${signal}, shutting down...`);
      stopAll();
    });
  });

  const context: PollerContext = {
    options,
    db,
    statements,
    probeFn: performProbe,
    missCounters,
    metrics,
    checkpoint,
    datasetStatus: new Map(),
  };

  const resolveTargets =
    options.termsMode === 'auto' ? () => discoverSubscriptionTargets(context) : () => resolveExplicitTargets(options);

  const initialTargets = resolveTargets();
  if (options.termsMode === 'explicit' && initialTargets.length === 0) {
    throw new Error('No term/campus targets resolved. Check --terms/--campuses.');
  }

  if (options.termsMode === 'auto') {
    const campusFilter = options.campuses.length > 0 ? options.campuses.join(',') : 'all';
    console.log(
      `Starting openSections poller in auto mode (campus allowlist: ${campusFilter}) refresh=${Math.round(
        options.refreshIntervalMs / 60000,
      )}m interval=${options.intervalMs}ms sqlite=${options.sqliteFile} checkpoint=${options.checkpointFile}`,
    );
    if (initialTargets.length === 0) {
      console.warn(
        `No active subscriptions found. Will idle and retry in ${Math.round(options.refreshIntervalMs / 60000)}m.`,
      );
    }
  } else {
    console.log(
      `Starting openSections poller for terms=${options.terms.join(',')} campuses=${options.campuses.join(',')} interval=${options.intervalMs}ms (jitter=${options.jitter}) sqlite=${options.sqliteFile} checkpoint=${options.checkpointFile}`,
    );
  }

  syncTargetLoops(context, semaphore, runningLoops, initialTargets, () => !shuttingDown);

  if (options.runOnce) {
    await Promise.all([...runningLoops.values()].map((loop) => loop.done));
    if (server) {
      server.close();
    }
    db.close();
    return;
  }

  if (options.termsMode === 'auto') {
    while (!shuttingDown) {
      await sleep(options.refreshIntervalMs);
      if (shuttingDown) break;
      const targets = resolveTargets();
      syncTargetLoops(context, semaphore, runningLoops, targets, () => !shuttingDown);
      if (targets.length === 0) {
        console.warn(
          `No active subscriptions detected; next refresh in ${Math.round(options.refreshIntervalMs / 60000)}m.`,
        );
      } else {
        console.log(`Monitoring ${targets.length} targets: ${targets.map((t) => targetLabel(t)).join(', ')}`);
      }
    }
  } else {
    await Promise.all([...runningLoops.values()].map((loop) => loop.done));
  }

  stopAll();
  await Promise.all([...runningLoops.values()].map((loop) => loop.done));

  if (server) {
    server.close();
  }
  db.close();
}

async function pollLoop(
  ctx: PollerContext,
  target: PollTarget,
  semaphore: Semaphore,
  shouldContinue: () => boolean,
): Promise<void> {
  const key = makeTargetKey(target.termId, target.campus);
  do {
    if (!shouldContinue()) break;
    if (ctx.options.runOnce && (ctx.metrics.targets[key]?.pollsTotal ?? 0) > 0) {
      break;
    }
    await semaphore.run(async () => {
      if (shouldContinue()) {
        await pollOnce(ctx, target);
      }
    });
    if (ctx.options.runOnce) {
      break;
    }
    const delay = jitteredDelay(ctx.options.intervalMs, ctx.options.jitter);
    await sleep(delay);
  } while (shouldContinue());
}

async function pollOnce(ctx: PollerContext, target: PollTarget): Promise<void> {
  const started = performance.now();
  ensureMetricsTarget(ctx.metrics, target);
  const key = makeTargetKey(target.termId, target.campus);
  if (!ensureSectionsData(ctx, target)) {
    return;
  }
  ctx.metrics.pollsTotal += 1;
  ctx.metrics.targets[key].pollsTotal += 1;

  try {
    const response = await ctx.probeFn(
      { campus: target.campus, endpoint: 'openSections', timeoutMs: ctx.options.timeoutMs },
      target.decodedTerm,
    );
    const indexes = normalizeOpenIndexes(response.body);
    const outcome = applySnapshot(ctx, target, indexes);
    persistCheckpoint(ctx, target, outcome);
    const durationMs = performance.now() - started;
    const targetMetrics = ctx.metrics.targets[key];
    ctx.metrics.eventsEmitted += outcome.events;
    ctx.metrics.notificationsQueued += outcome.notifications;
    targetMetrics.eventsTotal += outcome.events;
    targetMetrics.notificationsTotal += outcome.notifications;
    targetMetrics.lastDurationMs = durationMs;
    targetMetrics.lastOpenCount = outcome.openCount;
    console.log(
      `[${targetLabel(target)}] openSections=${indexes.length} opened=${outcome.opened} closed=${outcome.closed} events=${outcome.events} notifications=${outcome.notifications} durationMs=${durationMs.toFixed(
        0,
      )}`,
    );
  } catch (error) {
    ctx.metrics.pollsFailed += 1;
    ctx.metrics.targets[key].pollsFailed += 1;
    const durationMs = performance.now() - started;
    ctx.metrics.targets[key].lastDurationMs = durationMs;
    if (error instanceof SOCRequestError) {
      console.error(
        `[${targetLabel(target)}] openSections failed (${error.requestId}): ${error.message} [${error.kind}] retryHint=${error.retryHint ?? 'n/a'}`,
      );
    } else if (error instanceof Error) {
      console.error(`[${targetLabel(target)}] openSections failed: ${error.message}`);
    } else {
      console.error(`[${targetLabel(target)}] openSections failed due to unknown error`);
    }
  }
}

export function applySnapshot(
  ctx: PollerContext,
  target: PollTarget,
  indexes: string[],
  now: Date = new Date(),
): PollOutcome {
  const nowIso = now.toISOString();
  const sourceHash = hashPayload({ term: target.termId, campus: target.campus, indexes });
  const seen = new Set(indexes);
  const missKey = makeTargetKey(target.termId, target.campus);
  const missSet = ctx.missCounters.get(missKey) ?? new Map<string, number>();
  const sections = ctx.statements.selectSections.all(target.termId, target.campus) as SectionRow[];

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
  ctx.missCounters.set(missKey, missSet);

  let events = 0;
  let notifications = 0;

  const tx = ctx.db.transaction(() => {
    for (const index of indexes) {
      ctx.statements.deleteSnapshot.run(target.termId, target.campus, index);
      ctx.statements.insertSnapshot.run(target.termId, target.campus, index, nowIso, sourceHash);
    }

    for (const section of toOpen) {
      const previous = section.open_status ?? (section.is_open === 1 ? 'OPEN' : 'CLOSED');
      ctx.statements.updateSectionStatus.run(1, 'OPEN', nowIso, nowIso, section.section_id);
      ctx.statements.insertStatusEvent.run(
        section.section_id,
        previous,
        'OPEN',
        'openSections',
        target.termId,
        target.campus,
        nowIso,
      );
      const eventOutcome = createEventAndFanout(ctx, target.termId, {
        section,
        campus: target.campus,
        statusBefore: previous,
        statusAfter: 'OPEN',
        seatDelta: previous === 'OPEN' ? 0 : 1,
        eventAt: nowIso,
        snapshotHash: sourceHash,
      });
      events += eventOutcome.events;
      notifications += eventOutcome.notifications;
    }

    // Emit periodic reminders for sections that are already open (deduped to one event per 3-minute bucket).
    for (const section of sections) {
      if (section.is_open !== 1) continue;
      if (!seen.has(section.index_number)) continue;
      const eventOutcome = createEventAndFanout(ctx, target.termId, {
        section,
        campus: target.campus,
        statusBefore: section.open_status ?? 'OPEN',
        statusAfter: 'OPEN',
        seatDelta: 0,
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
        target.termId,
        target.campus,
        nowIso,
      );
      ctx.statements.deleteSnapshot.run(target.termId, target.campus, section.index_number);
      ctx.statements.resetSubscriptionsForIndex.run('CLOSED', nowIso, target.termId, target.campus, section.index_number);
      const eventOutcome = createEventAndFanout(ctx, target.termId, {
        section,
        campus: target.campus,
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
  term: string,
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
  if (args.statusAfter === 'OPEN') {
    const hasActiveSub = ctx.statements.hasActiveSubscription.get(
      term,
      args.campus,
      args.section.index_number,
      ACTIVE_STATUSES[0],
      ACTIVE_STATUSES[1],
    ) as { 1: number } | undefined;
    if (!hasActiveSub) {
      return { events: 0, notifications: 0 };
    }
  }
  const dedupeKey = buildDedupeKey(term, args.campus, args.section.index_number, args.statusAfter, eventTime);
  const cutoff = new Date(eventTime.getTime() - 3 * 60 * 1000).toISOString();
  const existing = ctx.statements.selectRecentEvent.get(dedupeKey, cutoff) as { open_event_id: number } | undefined;
  if (existing) {
    return { events: 0, notifications: 0 };
  }

  const traceId = crypto.randomUUID();
  const payload = {
    term,
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
    term,
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
    notifications = enqueueNotifications(ctx, term, {
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
  term: string,
  args: { campus: string; index: string; eventId: number; dedupeKey: string; eventAt: string },
): number {
  let offset = 0;
  let created = 0;
  const now = args.eventAt;
  while (true) {
    const rows = ctx.statements.selectSubscriptionsPage.all(
      term,
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
      if (!shouldNotify(sub, new Date(now), ctx.options.openReminderIntervalMs)) {
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
