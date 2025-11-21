#!/usr/bin/env tsx
import Database from 'better-sqlite3';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { buildDiscordSend, type CourseRow, type DiscordFanoutJob, type DiscordAdapterOptions, type MeetingRow, type SectionRow } from '../notifications/discord/adapter.js';
import { DiscordBot, loadDiscordBotConfig, type DiscordSendErrorCode, type ResolvedDiscordBotConfig } from '../notifications/discord/bot.js';

type DiscordDispatcherOptions = {
  sqliteFile: string;
  botConfigPath: string;
  batchSize: number;
  workerId: string;
  lockTtlSeconds: number;
  idleDelayMs: number;
  runOnce: boolean;
  appBaseUrl: string;
  defaultLocale: string;
  allowedChannelIds: string[];
};

type NotificationRow = {
  notification_id: number;
  open_event_id: number;
  subscription_id: number;
  dedupe_key: string;
  fanout_attempts: number;
  last_attempt_at: string | null;
  error: string | null;
  // open_events
  event_section_id: number | null;
  event_term_id: string;
  event_campus_code: string;
  event_index_number: string;
  event_status_after: string | null;
  event_status_before: string | null;
  event_seat_delta: number | null;
  event_at: string;
  event_trace_id: string | null;
  event_payload: string | null;
  // subscriptions
  subscription_status: string;
  contact_type: string;
  contact_value: string | null;
  subscription_locale: string | null;
  unsubscribe_token: string | null;
  subscription_metadata: string | null;
  subscription_section_id: number | null;
  subscription_term_id: string | null;
  subscription_campus_code: string | null;
  subscription_index_number: string | null;
  last_known_section_status: string | null;
};

type DiscordJob = DiscordFanoutJob & {
  notificationId: number;
  fanoutAttempts: number;
  section?: SectionRow;
  course?: CourseRow;
  meetings: MeetingRow[];
};

type SendExecutor = Pick<DiscordBot, 'send'>;

export class DiscordDispatcher {
  private readonly adapterOpts: DiscordAdapterOptions;
  private readonly now: () => Date;

  constructor(
    private readonly db: Database.Database,
    private readonly bot: SendExecutor,
    private readonly config: ResolvedDiscordBotConfig,
    private readonly options: Omit<DiscordDispatcherOptions, 'sqliteFile' | 'botConfigPath'>,
    now: () => Date = () => new Date(),
  ) {
    this.adapterOpts = {
      config: this.config,
      appBaseUrl: this.options.appBaseUrl,
      defaultLocale: this.options.defaultLocale,
      allowedChannelIds: this.options.allowedChannelIds,
    };
    this.now = now;
  }

  async runOnce(): Promise<number> {
    const jobs = this.claimBatch();
    for (const job of jobs) {
      await this.handleJob(job);
    }
    return jobs.length;
  }

  async runForever(): Promise<void> {
    while (true) {
      const processed = await this.runOnce();
      if (this.options.runOnce) break;
      const delay = processed > 0 ? 200 : this.options.idleDelayMs;
      await sleep(delay);
    }
  }

  private claimBatch(): DiscordJob[] {
    const expiry = new Date(this.now().getTime() - this.options.lockTtlSeconds * 1000).toISOString();
    const candidateStmt = this.db.prepare(
      `
      SELECT n.notification_id
      FROM open_event_notifications n
      JOIN subscriptions s ON n.subscription_id = s.subscription_id
      WHERE n.fanout_status = 'pending'
        AND s.contact_type IN ('discord_user', 'discord_channel')
        AND (n.locked_at IS NULL OR n.locked_at < ?)
      ORDER BY n.notification_id
      LIMIT ?
    `,
    );
    const candidates = candidateStmt.all(expiry, this.options.batchSize) as Array<{ notification_id: number }>;
    const locked: number[] = [];
    const lockStmt = this.db.prepare(
      `
      UPDATE open_event_notifications
      SET locked_by = ?, locked_at = ?
      WHERE notification_id = ?
        AND fanout_status = 'pending'
        AND (locked_at IS NULL OR locked_at < ?)
        AND EXISTS (
          SELECT 1 FROM subscriptions s WHERE s.subscription_id = open_event_notifications.subscription_id AND s.contact_type IN ('discord_user', 'discord_channel')
        )
    `,
    );
    const lockedAt = this.now().toISOString();
    for (const row of candidates) {
      const result = lockStmt.run(this.options.workerId, lockedAt, row.notification_id, expiry);
      if (result.changes > 0) {
        locked.push(row.notification_id);
      }
    }
    if (locked.length === 0) return [];
    return this.loadJobs(locked);
  }

  private loadJobs(ids: number[]): DiscordJob[] {
    if (ids.length === 0) return [];
    const placeholders = ids.map(() => '?').join(', ');
    const rows = this.db
      .prepare(
        `
        SELECT
          n.notification_id,
          n.open_event_id,
          n.subscription_id,
          n.dedupe_key,
          n.fanout_attempts,
          n.last_attempt_at,
          n.error,
          e.section_id AS event_section_id,
          e.term_id AS event_term_id,
          e.campus_code AS event_campus_code,
          e.index_number AS event_index_number,
          e.status_after AS event_status_after,
          e.status_before AS event_status_before,
          e.seat_delta AS event_seat_delta,
          e.event_at,
          e.trace_id AS event_trace_id,
          e.payload AS event_payload,
          s.status AS subscription_status,
          s.contact_type,
          s.contact_value,
          s.locale AS subscription_locale,
          s.unsubscribe_token,
          s.metadata AS subscription_metadata,
          s.section_id AS subscription_section_id,
          s.term_id AS subscription_term_id,
          s.campus_code AS subscription_campus_code,
          s.index_number AS subscription_index_number,
          s.last_known_section_status
        FROM open_event_notifications n
        JOIN open_events e ON n.open_event_id = e.open_event_id
        JOIN subscriptions s ON n.subscription_id = s.subscription_id
        WHERE n.notification_id IN (${placeholders})
      `,
      )
      .all(...ids) as NotificationRow[];

    const sectionIds = new Set<number>();
    for (const row of rows) {
      if (row.event_section_id) sectionIds.add(row.event_section_id);
      if (row.subscription_section_id) sectionIds.add(row.subscription_section_id);
    }
    const sectionMap = this.loadSections(Array.from(sectionIds));
    const courseIds = new Set<number>();
    sectionMap.forEach((value) => courseIds.add(value.course_id));
    const courseMap = this.loadCourses(Array.from(courseIds));
    const meetingMap = this.loadMeetings(Array.from(sectionIds));

    return rows.map((row) => {
      const resolvedSectionId = row.event_section_id ?? row.subscription_section_id ?? undefined;
      const section = resolvedSectionId ? sectionMap.get(resolvedSectionId) : undefined;
      const course = section?.course_id ? courseMap.get(section.course_id) : undefined;
      const meetings = resolvedSectionId ? meetingMap.get(resolvedSectionId) ?? [] : [];
      return {
        notificationId: row.notification_id,
        fanoutAttempts: row.fanout_attempts,
        dedupeKey: row.dedupe_key,
        event: {
          openEventId: row.open_event_id,
          termId: row.event_term_id,
          campusCode: row.event_campus_code,
          indexNumber: row.event_index_number,
          statusAfter: row.event_status_after,
          statusBefore: row.event_status_before,
          seatDelta: row.event_seat_delta,
          eventAt: row.event_at,
          traceId: row.event_trace_id,
          payload: safeParseJson(row.event_payload),
        },
        subscription: {
          subscriptionId: row.subscription_id,
          status: row.subscription_status,
          contactType: row.contact_type,
          contactValue: row.contact_value,
          locale: row.subscription_locale,
          metadata: row.subscription_metadata,
          unsubscribeToken: row.unsubscribe_token,
          sectionId: row.subscription_section_id,
          termId: row.subscription_term_id,
          campusCode: row.subscription_campus_code,
          indexNumber: row.subscription_index_number,
          lastKnownStatus: row.last_known_section_status,
        },
        section,
        course,
        meetings,
      };
    });
  }

  private loadSections(ids: number[]): Map<number, SectionRow> {
    const map = new Map<number, SectionRow>();
    if (ids.length === 0) return map;
    const placeholders = ids.map(() => '?').join(', ');
    const rows = this.db
      .prepare(
        `
        SELECT section_id, course_id, term_id, campus_code, subject_code, section_number, index_number,
               open_status, open_status_updated_at, meeting_mode_summary
        FROM sections
        WHERE section_id IN (${placeholders})
      `,
      )
      .all(...ids) as SectionRow[];
    rows.forEach((row) => map.set(row.section_id, row));
    return map;
  }

  private loadCourses(ids: number[]): Map<number, CourseRow> {
    const map = new Map<number, CourseRow>();
    if (ids.length === 0) return map;
    const placeholders = ids.map(() => '?').join(', ');
    const rows = this.db
      .prepare(
        `
        SELECT course_id, subject_code, course_number, course_string, title
        FROM courses
        WHERE course_id IN (${placeholders})
      `,
      )
      .all(...ids) as CourseRow[];
    rows.forEach((row) => map.set(row.course_id, row));
    return map;
  }

  private loadMeetings(ids: number[]): Map<number, MeetingRow[]> {
    const map = new Map<number, MeetingRow[]>();
    if (ids.length === 0) return map;
    const placeholders = ids.map(() => '?').join(', ');
    const rows = this.db
      .prepare(
        `
        SELECT section_id, meeting_day, start_minutes, end_minutes,
               campus_abbrev, campus_location_code, campus_location_desc,
               building_code, room_number
        FROM section_meetings
        WHERE section_id IN (${placeholders})
      `,
      )
      .all(...ids) as MeetingRow[];
    for (const meeting of rows) {
      const list = map.get(meeting.section_id) ?? [];
      list.push(meeting);
      map.set(meeting.section_id, list);
    }
    return map;
  }

  private async handleJob(job: DiscordJob): Promise<void> {
    const eligibility = this.validate(job);
    if (!eligibility.ok) {
      this.persistOutcome(job, {
        fanoutStatus: 'skipped',
        attempts: job.fanoutAttempts + 1,
        error: { code: eligibility.code, message: eligibility.message },
        subscriptionEvent: { type: 'notify_failed', error: eligibility.code },
      });
      return;
    }

    const built = buildDiscordSend(job, this.adapterOpts);
    if (!built.ok) {
      this.persistOutcome(job, {
        fanoutStatus: 'skipped',
        attempts: job.fanoutAttempts + 1,
        error: { code: built.code, message: built.message },
        subscriptionEvent: { type: 'notify_failed', error: built.code },
      });
      return;
    }

    const sendResult = await this.bot.send(built.request);
    const attempts = job.fanoutAttempts + 1;
    const final = sendResult.finalResult;
    const serializedError = JSON.stringify({ finalResult: final, attempts: sendResult.attempts });

    if (final.status === 'sent') {
      this.persistOutcome(job, {
        fanoutStatus: 'sent',
        attempts,
        error: serializedError,
        subscriptionEvent: { type: 'notify_sent', providerMessageId: final.messageId },
        updateLastNotified: true,
      });
      return;
    }

    if (final.status === 'retryable') {
      const maxAttempts = this.config.rateLimit.maxAttempts;
      const exhausted = attempts >= maxAttempts;
      const delayMs = Math.max(
        final.retryAfterSeconds ? Math.round(final.retryAfterSeconds * 1000) : 0,
        this.config.rateLimit.backoffMs[Math.min(attempts - 1, this.config.rateLimit.backoffMs.length - 1)] ?? 0,
      );
      const lockedAt = exhausted ? null : this.computeRetryLock(delayMs);
      this.persistOutcome(job, {
        fanoutStatus: exhausted ? 'failed' : 'pending',
        attempts,
        error: serializedError,
        lockedAt,
        subscriptionEvent: exhausted ? { type: 'notify_failed', error: final.error?.code ?? 'retry_exhausted' } : null,
      });
      return;
    }

    const terminalStatus = this.isSkippable(final.error?.code) ? 'skipped' : 'failed';
    this.persistOutcome(job, {
      fanoutStatus: terminalStatus,
      attempts,
      error: serializedError,
      subscriptionEvent: { type: 'notify_failed', error: final.error?.code ?? 'failed' },
    });
  }

  private validate(job: DiscordJob): { ok: true } | { ok: false; code: DiscordSendErrorCode | 'ineligible'; message: string } {
    const contactType = job.subscription.contactType;
    if (contactType !== 'discord_user' && contactType !== 'discord_channel') {
      return { ok: false, code: 'ineligible', message: 'contact type is not discord' };
    }
    if (!['pending', 'active'].includes(job.subscription.status)) {
      return { ok: false, code: 'ineligible', message: `subscription status=${job.subscription.status}` };
    }
    if (job.event.statusAfter && job.event.statusAfter.toUpperCase() !== 'OPEN') {
      return { ok: false, code: 'ineligible', message: `event status ${job.event.statusAfter}` };
    }
    return { ok: true };
  }

  private isSkippable(code: DiscordSendErrorCode | undefined): boolean {
    return code === 'validation_error' || code === 'unauthorized' || code === 'not_found';
  }

  private computeRetryLock(delayMs: number): string {
    const ttlMs = this.options.lockTtlSeconds * 1000;
    const bounded = Math.min(Math.max(delayMs, 0), ttlMs);
    const lockedAtMs = this.now().getTime() - (ttlMs - bounded) - 1;
    return new Date(lockedAtMs).toISOString();
  }

  private persistOutcome(
    job: DiscordJob,
    input: {
      fanoutStatus: 'pending' | 'sent' | 'failed' | 'skipped';
      attempts: number;
      error: string | { code: string; message: string };
      lockedAt?: string | null;
      subscriptionEvent: { type: 'notify_sent' | 'notify_failed'; error?: string; providerMessageId?: string } | null;
      updateLastNotified?: boolean;
    },
  ) {
    const nowIso = this.now().toISOString();
    const errorText = typeof input.error === 'string' ? input.error : JSON.stringify(input.error);
    const statusSnapshot = job.section?.open_status ?? job.subscription.lastKnownStatus ?? job.event.statusAfter ?? null;
    const lockedAt = input.lockedAt === undefined ? null : input.lockedAt;

    const updateNotification = this.db.prepare(
      `
      UPDATE open_event_notifications
      SET fanout_status = ?, fanout_attempts = ?, last_attempt_at = ?, locked_by = NULL, locked_at = ?, error = ?
      WHERE notification_id = ?
    `,
    );

    const updateSubscription = this.db.prepare(
      `
      UPDATE subscriptions
      SET last_known_section_status = ?, last_notified_at = ?, updated_at = ?
      WHERE subscription_id = ?
    `,
    );

    const insertEvent = this.db.prepare(
      `
      INSERT INTO subscription_events (subscription_id, event_type, section_status_snapshot, payload, created_at)
      VALUES (?, ?, ?, ?, ?)
    `,
    );

    const tx = this.db.transaction(() => {
      updateNotification.run(input.fanoutStatus, input.attempts, nowIso, lockedAt, errorText, job.notificationId);
      if (input.updateLastNotified) {
        updateSubscription.run('OPEN', nowIso, nowIso, job.subscription.subscriptionId);
      }
      if (input.subscriptionEvent) {
        insertEvent.run(
          job.subscription.subscriptionId,
          input.subscriptionEvent.type,
          statusSnapshot,
          JSON.stringify({
            openEventId: job.event.openEventId,
            dedupeKey: job.dedupeKey,
            traceId: job.event.traceId,
            error: input.subscriptionEvent.error,
            providerMessageId: input.subscriptionEvent.providerMessageId,
          }),
          nowIso,
        );
      }
    });

    tx();
  }
}

function safeParseJson(text: string | null): Record<string, unknown> {
  if (!text) return {};
  try {
    const parsed = JSON.parse(text) as unknown;
    return typeof parsed === 'object' && parsed ? (parsed as Record<string, unknown>) : {};
  } catch {
    return {};
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function parseArgs(argv: string[]): DiscordDispatcherOptions {
  const defaults: DiscordDispatcherOptions = {
    sqliteFile: path.resolve('data', 'local.db'),
    botConfigPath: path.resolve('configs', 'discord_bot.example.json'),
    batchSize: 25,
    workerId: `discord-worker-${Math.random().toString(16).slice(2, 8)}`,
    lockTtlSeconds: 120,
    idleDelayMs: 2000,
    runOnce: false,
    appBaseUrl: 'http://localhost:3000',
    defaultLocale: 'en-US',
    allowedChannelIds: [],
  };

  const opts = { ...defaults };
  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    const next = argv[i + 1];
    switch (token) {
      case '--sqlite':
        if (!next) throw new Error('Missing value for --sqlite');
        opts.sqliteFile = path.resolve(next);
        i += 1;
        break;
      case '--bot-config':
        if (!next) throw new Error('Missing value for --bot-config');
        opts.botConfigPath = path.resolve(next);
        i += 1;
        break;
      case '--batch':
        if (!next) throw new Error('Missing value for --batch');
        opts.batchSize = parseInt(next, 10);
        i += 1;
        break;
      case '--worker-id':
        if (!next) throw new Error('Missing value for --worker-id');
        opts.workerId = next;
        i += 1;
        break;
      case '--lock-ttl':
        if (!next) throw new Error('Missing value for --lock-ttl');
        opts.lockTtlSeconds = parseInt(next, 10);
        i += 1;
        break;
      case '--app-base-url':
        if (!next) throw new Error('Missing value for --app-base-url');
        opts.appBaseUrl = next;
        i += 1;
        break;
      case '--default-locale':
        if (!next) throw new Error('Missing value for --default-locale');
        opts.defaultLocale = next;
        i += 1;
        break;
      case '--allow-channel':
        if (!next) throw new Error('Missing value for --allow-channel');
        opts.allowedChannelIds.push(next);
        i += 1;
        break;
      case '--idle-delay':
        if (!next) throw new Error('Missing value for --idle-delay');
        opts.idleDelayMs = parseInt(next, 10);
        i += 1;
        break;
      case '--once':
        opts.runOnce = true;
        break;
      default:
        throw new Error(`Unknown argument: ${token}`);
    }
  }

  return opts;
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const cfg = await loadDiscordBotConfig(options.botConfigPath);
  const bot = new DiscordBot(cfg);
  const db = new Database(options.sqliteFile);
  db.pragma('journal_mode = WAL');
  db.pragma('foreign_keys = ON');

  try {
    const dispatcher = new DiscordDispatcher(
      db,
      bot,
      cfg,
      {
        batchSize: options.batchSize,
        workerId: options.workerId,
        lockTtlSeconds: options.lockTtlSeconds,
        idleDelayMs: options.idleDelayMs,
        runOnce: options.runOnce,
        appBaseUrl: options.appBaseUrl,
        defaultLocale: options.defaultLocale,
        allowedChannelIds: options.allowedChannelIds,
      },
    );
    await dispatcher.runForever();
  } finally {
    db.close();
  }
}

const isDirectRun = process.argv[1] === fileURLToPath(import.meta.url);
if (isDirectRun) {
  void main().catch((error) => {
    console.error('discord_dispatcher failed:', error);
    process.exit(1);
  });
}
