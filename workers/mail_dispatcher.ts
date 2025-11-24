#!/usr/bin/env tsx
import Database from 'better-sqlite3';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { loadMailSenderConfig } from '../notifications/mail/config.js';
import { ReliableMailSender } from '../notifications/mail/retry_policy.js';
import { SendGridMailSender } from '../notifications/mail/providers/sendgrid.js';
import type { MailMessage, ResolvedMailSenderConfig, SendErrorCode } from '../notifications/mail/types.js';

type DeliveryPolicy = {
  maxAttempts: number;
  retryScheduleMs: number[];
};

export type MailDispatcherOptions = {
  sqliteFile: string;
  mailConfigPath: string;
  batchSize: number;
  workerId: string;
  lockTtlSeconds: number;
  delivery: DeliveryPolicy;
  appBaseUrl: string;
  defaultLocale: string;
  idleDelayMs: number;
  runOnce: boolean;
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

type SectionRow = {
  section_id: number;
  course_id: number;
  term_id: string;
  campus_code: string;
  subject_code: string;
  section_number: string | null;
  index_number: string;
  open_status: string | null;
  open_status_updated_at: string | null;
  meeting_mode_summary: string | null;
};

type CourseRow = {
  course_id: number;
  subject_code: string;
  course_number: string;
  course_string: string | null;
  title: string;
};

type MeetingRow = {
  section_id: number;
  meeting_day: string | null;
  start_minutes: number | null;
  end_minutes: number | null;
  campus_abbrev: string | null;
  campus_location_code: string | null;
  campus_location_desc: string | null;
  building_code: string | null;
  room_number: string | null;
};

type MailJob = {
  notificationId: number;
  fanoutAttempts: number;
  dedupeKey: string;
  event: {
    openEventId: number;
    sectionId: number | null;
    termId: string;
    campusCode: string;
    indexNumber: string;
    statusAfter: string | null;
    statusBefore: string | null;
    eventAt: string;
    traceId: string | null;
  };
  subscription: {
    subscriptionId: number;
    status: string;
    contactType: string;
    contactValue: string | null;
    locale: string | null;
    unsubscribeToken: string | null;
    metadata: string | null;
    sectionId: number | null;
    termId: string | null;
    campusCode: string | null;
    indexNumber: string | null;
    lastKnownStatus: string | null;
  };
  payload: Record<string, unknown>;
  section?: SectionRow;
  course?: CourseRow;
  meetings: MeetingRow[];
};

type SendExecutor = Pick<ReliableMailSender, 'send'>;

export class MailDispatcher {
  private readonly now: () => Date;

  constructor(
    private readonly db: Database.Database,
    private readonly sender: SendExecutor,
    private readonly config: ResolvedMailSenderConfig,
    private readonly options: Omit<MailDispatcherOptions, 'sqliteFile' | 'mailConfigPath'>,
    now: () => Date = () => new Date(),
  ) {
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

  private claimBatch(): MailJob[] {
    const expiry = new Date(this.now().getTime() - this.options.lockTtlSeconds * 1000).toISOString();
    const candidateStmt = this.db.prepare(
      `
      SELECT notification_id
      FROM open_event_notifications n
      JOIN subscriptions s ON n.subscription_id = s.subscription_id
      WHERE n.fanout_status = 'pending'
        AND s.contact_type = 'email'
        AND (n.locked_at IS NULL OR n.locked_at < ?)
      ORDER BY notification_id
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
          SELECT 1 FROM subscriptions s WHERE s.subscription_id = open_event_notifications.subscription_id AND s.contact_type = 'email'
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

  private loadJobs(ids: number[]): MailJob[] {
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
          sectionId: row.event_section_id,
          termId: row.event_term_id,
          campusCode: row.event_campus_code,
          indexNumber: row.event_index_number,
          statusAfter: row.event_status_after,
          statusBefore: row.event_status_before,
          eventAt: row.event_at,
          traceId: row.event_trace_id,
        },
        subscription: {
          subscriptionId: row.subscription_id,
          status: row.subscription_status,
          contactType: row.contact_type,
          contactValue: row.contact_value,
          locale: row.subscription_locale,
          unsubscribeToken: row.unsubscribe_token,
          metadata: row.subscription_metadata,
          sectionId: row.subscription_section_id,
          termId: row.subscription_term_id,
          campusCode: row.subscription_campus_code,
          indexNumber: row.subscription_index_number,
          lastKnownStatus: row.last_known_section_status,
        },
        payload: safeParseJson(row.event_payload),
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

  private async handleJob(job: MailJob): Promise<void> {
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

    const message = this.buildMessage(job);
    const sendResult = await this.sender.send(message, { rateLimitKey: 'mail-open-seat' });
    const attempts = job.fanoutAttempts + 1;
    const final = sendResult.finalResult;
    const serializedError = JSON.stringify({ finalResult: final, attempts: sendResult.attempts });

    if (final.status === 'sent') {
      this.persistOutcome(job, {
        fanoutStatus: 'sent',
        attempts,
        error: serializedError,
        subscriptionEvent: { type: 'notify_sent', providerMessageId: final.providerMessageId },
        updateLastNotified: true,
      });
      return;
    }

    if (final.status === 'retryable') {
      const reachedMax = attempts >= this.options.delivery.maxAttempts;
      const delayMs = Math.max(
        final.retryAfterSeconds ? Math.round(final.retryAfterSeconds * 1000) : 0,
        this.options.delivery.retryScheduleMs[Math.min(attempts - 1, this.options.delivery.retryScheduleMs.length - 1)] ??
          0,
      );
      const lockedAt = reachedMax ? null : this.computeRetryLock(delayMs);
      this.persistOutcome(job, {
        fanoutStatus: reachedMax ? 'failed' : 'pending',
        attempts,
        error: serializedError,
        lockedAt,
        subscriptionEvent: reachedMax ? { type: 'notify_failed', error: final.error?.code ?? 'retry_exhausted' } : null,
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

  private validate(job: MailJob): { ok: true } | { ok: false; code: SendErrorCode | 'ineligible'; message: string } {
    if (job.subscription.contactType !== 'email') {
      return { ok: false, code: 'ineligible', message: 'contact type is not email' };
    }
    if (!job.subscription.contactValue) {
      return { ok: false, code: 'invalid_recipient', message: 'missing contact value' };
    }
    if (!['pending', 'active'].includes(job.subscription.status)) {
      return { ok: false, code: 'ineligible', message: `subscription status=${job.subscription.status}` };
    }
    if (job.event.statusAfter && job.event.statusAfter.toUpperCase() !== 'OPEN') {
      return { ok: false, code: 'ineligible', message: `event status ${job.event.statusAfter}` };
    }
    return { ok: true };
  }

  private buildMessage(job: MailJob): MailMessage {
    const locale = chooseLocale(job.subscription.locale, this.config.supportedLocales, this.options.defaultLocale);
    const links = buildLinks(this.options.appBaseUrl, job.subscription.subscriptionId, job.subscription.unsubscribeToken);
    const courseTitle =
      job.course?.title ?? (typeof job.payload.courseTitle === 'string' ? job.payload.courseTitle : 'Course update');
    const courseString = deriveCourseString(job, courseTitle);
    const indexNumber = job.event.indexNumber ?? job.subscription.indexNumber ?? 'TBD';
    const sectionNumber =
      job.section?.section_number ??
      (typeof job.payload.sectionNumber === 'string' ? job.payload.sectionNumber : job.subscription.indexNumber) ??
      'TBD';
    const meetingSummary = buildMeetingSummary(job.meetings);

    return {
      to: { email: job.subscription.contactValue ?? '' },
      locale,
      templateId: 'open-seat',
      templateVersion: 'v1',
      variables: {
        courseTitle,
        courseString,
        sectionIndex: job.event.indexNumber,
        indexNumber,
        sectionNumber,
        meetingSummary,
        campus: job.event.campusCode,
        eventDetectedAt: job.event.eventAt,
        subscriptionId: job.subscription.subscriptionId,
        manageUrl: links.manageUrl,
        unsubscribeUrl: links.unsubscribeUrl ?? '',
      },
      manageUrl: links.manageUrl,
      unsubscribeUrl: links.unsubscribeUrl,
      dedupeKey: job.dedupeKey,
      traceId: job.event.traceId ?? undefined,
      metadata: {
        subscriptionId: job.subscription.subscriptionId,
        openEventId: job.event.openEventId,
        term: job.event.termId,
        campus: job.event.campusCode,
      },
    };
  }

  private isSkippable(code: SendErrorCode | undefined): boolean {
    return (
      code === 'invalid_recipient' ||
      code === 'validation_error' ||
      code === 'template_missing_locale' ||
      code === 'template_variable_missing'
    );
  }

  private computeRetryLock(delayMs: number): string {
    const ttlMs = this.options.lockTtlSeconds * 1000;
    const bounded = Math.min(Math.max(delayMs, 0), ttlMs);
    const lockedAtMs = this.now().getTime() - (ttlMs - bounded) - 1;
    return new Date(lockedAtMs).toISOString();
  }

  private persistOutcome(
    job: MailJob,
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

function chooseLocale(preferred: string | null, supported: string[], fallback: string): string {
  if (preferred && supported.includes(preferred)) return preferred;
  if (supported.includes(fallback)) return fallback;
  return supported[0];
}

function buildLinks(base: string, subscriptionId: number, unsubscribeToken: string | null) {
  const normalized = base.replace(/\/+$/, '');
  const manageUrl = `${normalized}/subscriptions/${subscriptionId}`;
  const unsubscribeUrl = unsubscribeToken
    ? `${normalized}/unsubscribe?token=${encodeURIComponent(unsubscribeToken)}`
    : undefined;
  return { manageUrl, unsubscribeUrl };
}

function buildMeetingSummary(meetings: MeetingRow[]): string {
  if (!meetings.length) return 'TBA';
  return meetings
    .map((meeting) => {
      const day = meeting.meeting_day ?? 'TBA';
      const time =
        meeting.start_minutes !== null && meeting.start_minutes !== undefined && meeting.end_minutes !== null
          ? `${formatMinutes(meeting.start_minutes)}-${formatMinutes(meeting.end_minutes)}`
          : null;
      const locationParts: string[] = [];
      if (meeting.campus_abbrev) locationParts.push(meeting.campus_abbrev);
      else if (meeting.campus_location_code) locationParts.push(meeting.campus_location_code);
      else if (meeting.campus_location_desc) locationParts.push(meeting.campus_location_desc);
      if (meeting.building_code) locationParts.push(meeting.building_code);
      if (meeting.room_number) locationParts.push(meeting.room_number);
      const location = locationParts.length ? locationParts.join(' ') : null;
      return [day, time, location].filter(Boolean).join(' ');
    })
    .join('; ');
}

function formatMinutes(value: number): string {
  const hours = Math.floor(value / 60);
  const minutes = value % 60;
  return `${hours.toString().padStart(2, '0')}:${minutes.toString().padStart(2, '0')}`;
}

function deriveCourseString(job: MailJob, courseTitle: string): string {
  if (job.course?.course_string) return job.course.course_string;
  if (typeof job.payload.courseString === 'string') return job.payload.courseString;
  if (job.course) return `${job.course.subject_code}:${job.course.course_number}`;
  if (job.section) return `${job.section.subject_code}:${job.section.index_number}`;
  if (typeof job.payload.subject === 'string' && typeof job.payload.index === 'string') {
    return `${job.payload.subject}:${job.payload.index}`;
  }
  return courseTitle;
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

function parseArgs(argv: string[]): MailDispatcherOptions {
  const defaults: MailDispatcherOptions = {
    sqliteFile: path.resolve('data', 'local.db'),
    mailConfigPath: path.resolve('configs', 'mail_sender.example.json'),
    batchSize: 25,
    workerId: `mail-worker-${Math.random().toString(16).slice(2, 8)}`,
    lockTtlSeconds: 120,
    delivery: { maxAttempts: 3, retryScheduleMs: [0, 2000, 7000] },
    appBaseUrl: 'http://localhost:3000',
    defaultLocale: 'en-US',
    idleDelayMs: 2000,
    runOnce: false,
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
      case '--mail-config':
        if (!next) throw new Error('Missing value for --mail-config');
        opts.mailConfigPath = path.resolve(next);
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
      case '--max-attempts':
        if (!next) throw new Error('Missing value for --max-attempts');
        opts.delivery.maxAttempts = parseInt(next, 10);
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

async function createSender(config: ResolvedMailSenderConfig): Promise<ReliableMailSender> {
  switch (config.provider) {
    case 'sendgrid':
      return new ReliableMailSender(new SendGridMailSender(config), config);
    default:
      throw new Error(`Unsupported mail provider: ${config.provider}`);
  }
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  const cfg = await loadMailSenderConfig(options.mailConfigPath);
  const sender = await createSender(cfg);
  const db = new Database(options.sqliteFile);
  db.pragma('journal_mode = WAL');
  db.pragma('foreign_keys = ON');

  try {
    const dispatcher = new MailDispatcher(db, sender, cfg, {
      batchSize: options.batchSize,
      workerId: options.workerId,
      lockTtlSeconds: options.lockTtlSeconds,
      delivery: options.delivery,
      appBaseUrl: options.appBaseUrl,
      defaultLocale: options.defaultLocale,
      idleDelayMs: options.idleDelayMs,
      runOnce: options.runOnce,
    });
    await dispatcher.runForever();
  } finally {
    db.close();
  }
}

const isDirectRun = process.argv[1] === fileURLToPath(import.meta.url);
if (isDirectRun) {
  void main().catch((error) => {
    console.error('mail_dispatcher failed:', error);
    process.exit(1);
  });
}
