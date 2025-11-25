import crypto from 'node:crypto';
import type { FastifyInstance, FastifyReply } from 'fastify';
import { z } from 'zod';

import { API_VERSION } from './sharedSchemas.js';

const ELIGIBLE_STATUSES = ['pending', 'active'] as const;
const CLAIM_DEFAULT_LIMIT = 20;
const CLAIM_MAX_LIMIT = 50;
const LOCAL_DEVICE_ID_REGEX = /^[a-zA-Z0-9:_-]+$/;

const claimPayloadSchema = z.object({
  deviceId: z.string().trim().min(6).max(128),
  limit: z.coerce.number().int().min(1).max(CLAIM_MAX_LIMIT).optional(),
});

const errorResponseSchema = z.object({
  error: z.object({
    code: z.string(),
    message: z.string(),
    traceId: z.string(),
    details: z.record(z.string(), z.unknown()).optional(),
  }),
});

const notificationSchema = z.object({
  notificationId: z.number().int(),
  term: z.string(),
  campus: z.string(),
  sectionIndex: z.string(),
  courseTitle: z.string().nullable(),
  eventAt: z.string(),
  dedupeKey: z.string(),
  traceId: z.string().nullable(),
});

const claimResponseSchema = z.object({
  notifications: z.array(notificationSchema),
  traceId: z.string(),
  meta: z.object({ version: z.string(), count: z.number().int() }).optional(),
});

type ClaimPayload = z.infer<typeof claimPayloadSchema>;

type PendingNotificationRow = {
  notification_id: number;
  open_event_id: number;
  subscription_id: number;
  dedupe_key: string;
  event_term_id: string;
  event_campus_code: string;
  event_index_number: string;
  event_status_after: string | null;
  event_at: string;
  event_trace_id: string | null;
  event_payload: string | null;
  last_known_section_status: string | null;
};

export async function registerLocalNotificationRoutes(app: FastifyInstance) {
  app.post(
    '/notifications/local/claim',
    {
      schema: {
        body: claimPayloadSchema,
        response: {
          200: claimResponseSchema,
          400: errorResponseSchema,
        },
      },
    },
    async (request, reply) => {
      const traceId = String(request.id);
      reply.header('x-trace-id', traceId);

      const body = request.body as ClaimPayload;
      const deviceId = normalizeDeviceId(body.deviceId);
      if (!deviceId) {
        return sendError(reply, 400, 'INVALID_DEVICE_ID', 'deviceId is invalid or empty', traceId);
      }

      const limit = body.limit ?? CLAIM_DEFAULT_LIMIT;
      const db = request.server.container.getDb();
      const nowIso = new Date().toISOString();
      const rows = claimNotifications(db, {
        deviceHash: sha1(deviceId),
        limit,
        now: nowIso,
      });

      const notifications = rows.map((row) => ({
        notificationId: row.notification_id,
        term: row.event_term_id,
        campus: row.event_campus_code,
        sectionIndex: row.event_index_number,
        courseTitle: deriveCourseTitle(row.event_payload),
        eventAt: row.event_at,
        dedupeKey: row.dedupe_key,
        traceId: row.event_trace_id,
      }));

      return reply.status(200).send({
        notifications,
        traceId,
        meta: {
          version: API_VERSION,
          count: notifications.length,
        },
      });
    },
  );
}

function claimNotifications(
  db: any,
  args: {
    deviceHash: string;
    limit: number;
    now: string;
  },
): PendingNotificationRow[] {
  const selectPending = db.prepare(
    `
    SELECT
      n.notification_id,
      n.open_event_id,
      n.subscription_id,
      n.dedupe_key,
      e.term_id AS event_term_id,
      e.campus_code AS event_campus_code,
      e.index_number AS event_index_number,
      e.status_after AS event_status_after,
      e.event_at,
      e.trace_id AS event_trace_id,
      e.payload AS event_payload,
      s.last_known_section_status
    FROM open_event_notifications n
    JOIN subscriptions s ON n.subscription_id = s.subscription_id
    JOIN open_events e ON n.open_event_id = e.open_event_id
    WHERE n.fanout_status = 'pending'
      AND s.contact_type = 'local_sound'
      AND s.contact_hash = ?
      AND s.status IN (${ELIGIBLE_STATUSES.map(() => '?').join(', ')})
    ORDER BY n.notification_id
    LIMIT ?
  `,
  );

  const updateNotification = db.prepare(
    `
    UPDATE open_event_notifications
    SET fanout_status = 'sent',
        fanout_attempts = fanout_attempts + 1,
        last_attempt_at = ?,
        locked_by = NULL,
        locked_at = NULL,
        error = NULL
    WHERE notification_id = ?
      AND fanout_status = 'pending'
  `,
  );

  const updateSubscription = db.prepare(
    `
    UPDATE subscriptions
    SET last_known_section_status = ?,
        last_notified_at = ?,
        updated_at = ?
    WHERE subscription_id = ?
  `,
  );

  const insertEvent = db.prepare(
    `
    INSERT INTO subscription_events (subscription_id, event_type, section_status_snapshot, payload, created_at)
    VALUES (?, 'notify_sent', ?, ?, ?)
  `,
  );

  const tx = db.transaction((deviceHash: string, limit: number) => {
    const rows = selectPending.all(deviceHash, ...ELIGIBLE_STATUSES, limit) as PendingNotificationRow[];
    if (!rows.length) {
      return [] as PendingNotificationRow[];
    }

    const claimed: PendingNotificationRow[] = [];
    for (const row of rows) {
      const statusSnapshot = row.event_status_after ?? row.last_known_section_status ?? 'OPEN';
      const result = updateNotification.run(args.now, row.notification_id);
      if (result.changes > 0) {
        updateSubscription.run(statusSnapshot, args.now, args.now, row.subscription_id);
        insertEvent.run(
          row.subscription_id,
          statusSnapshot,
          JSON.stringify({
            channel: 'local_sound',
            openEventId: row.open_event_id,
            dedupeKey: row.dedupe_key,
            traceId: row.event_trace_id,
          }),
          args.now,
        );
        claimed.push(row);
      }
    }

    return claimed;
  });

  return tx(args.deviceHash, args.limit);
}

function deriveCourseTitle(payload: string | null): string | null {
  if (!payload) return null;
  try {
    const parsed = JSON.parse(payload) as { courseTitle?: unknown };
    const value = parsed?.courseTitle;
    if (typeof value === 'string' && value.trim().length > 0) {
      return value;
    }
  } catch {
    // best-effort parse
  }
  return null;
}

function normalizeDeviceId(raw: string): string | null {
  const trimmed = raw.trim();
  if (trimmed.length < 6 || trimmed.length > 128) {
    return null;
  }
  if (!LOCAL_DEVICE_ID_REGEX.test(trimmed)) {
    return null;
  }
  return trimmed.toLowerCase();
}

function sha1(value: string): string {
  return crypto.createHash('sha1').update(value.toLowerCase()).digest('hex');
}

function sendError(reply: FastifyReply, statusCode: number, code: string, message: string, traceId: string) {
  return reply.status(statusCode).send({
    error: {
      code,
      message,
      traceId,
    },
  });
}
