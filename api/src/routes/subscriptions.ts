import type { FastifyInstance, FastifyReply } from 'fastify';
import crypto from 'node:crypto';
import { z } from 'zod';

import { API_VERSION } from './sharedSchemas.js';

const EMAIL_REGEX =
  /^(?:[a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+|"[^"]+")@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9-]+)+$/;
const DISCORD_SNOWFLAKE = /^[0-9]{17,19}$/;
const DEFAULT_LOCALE = 'en-US';
const CONTACT_ACTIVE_LIMIT = 3;
const SECTION_ACTIVE_LIMIT = 50;
const IP_WINDOW_MS = 10 * 60 * 1000;
const IP_MAX_ATTEMPTS = 10;

const ACTIVE_STATUSES = ['pending', 'active'] as const;

type ContactType = 'email' | 'discord_user' | 'discord_channel';
type SubscriptionStatus = 'pending' | 'active' | 'paused' | 'suppressed' | 'unsubscribed';

const preferencesInputSchema = z
  .object({
    notifyOn: z.array(z.enum(['open', 'waitlist'])).min(1).max(2).optional(),
    maxNotifications: z.coerce.number().int().min(1).max(10).optional(),
    deliveryWindow: z
      .object({
        startMinutes: z.coerce.number().int().min(0).max(1440),
        endMinutes: z.coerce.number().int().min(0).max(1440),
      })
      .optional()
      .refine((value) => !value || value.startMinutes <= value.endMinutes, {
        message: 'deliveryWindow.startMinutes must be <= deliveryWindow.endMinutes',
        path: ['startMinutes'],
      }),
    snoozeUntil: z.string().datetime({ offset: true }).nullable().optional(),
    channelMetadata: z.record(z.unknown()).optional(),
  })
  .optional();

const preferencesSchema = z.object({
  notifyOn: z.array(z.enum(['open', 'waitlist'])).min(1),
  maxNotifications: z.number().int().min(1),
  deliveryWindow: z.object({
    startMinutes: z.number().int(),
    endMinutes: z.number().int(),
  }),
  snoozeUntil: z.string().nullable(),
  channelMetadata: z.record(z.unknown()),
});

const subscribePayloadSchema = z.object({
  term: z.string().trim().min(4).max(10),
  campus: z
    .string()
    .trim()
    .min(2)
    .max(5)
    .transform((value) => value.toUpperCase()),
  sectionIndex: z
    .string()
    .trim()
    .regex(/^\d{5}$/),
  contactType: z.enum(['email', 'discord_user', 'discord_channel']),
  contactValue: z.string().trim().min(3).max(256),
  locale: z
    .string()
    .trim()
    .regex(/^[A-Za-z]{2,3}(?:[-_][A-Za-z]{2,3})?$/)
    .optional(),
  preferences: preferencesInputSchema,
  clientContext: z
    .object({
      ip: z.string().trim().min(3).max(64).optional(),
      userAgent: z.string().trim().min(3).max(512).optional(),
    })
    .optional(),
  discord: z
    .object({
      guildId: z.string().trim().min(1).max(32).optional(),
      channelId: z.string().trim().min(1).max(32).optional(),
    })
    .optional(),
});

const unsubscribePayloadSchema = z
  .object({
    subscriptionId: z.coerce.number().int().positive().optional(),
    unsubscribeToken: z.string().trim().min(16).optional(),
    contactValue: z.string().trim().min(3).max(256).optional(),
    reason: z.string().trim().min(3).max(64).optional(),
  })
  .refine((value) => Boolean(value.unsubscribeToken || (value.subscriptionId && value.contactValue)), {
    message: 'Provide unsubscribeToken or subscriptionId with matching contactValue',
    path: ['subscriptionId'],
  });

const errorResponseSchema = z.object({
  error: z.object({
    code: z.string(),
    message: z.string(),
    traceId: z.string(),
    details: z.record(z.string(), z.unknown()).optional(),
  }),
});

const subscribeResponseSchema = z.object({
  subscriptionId: z.number().int(),
  status: z.string(),
  requiresVerification: z.boolean(),
  existing: z.boolean(),
  unsubscribeToken: z.string().nullable(),
  term: z.string(),
  campus: z.string(),
  sectionIndex: z.string(),
  sectionResolved: z.boolean(),
  preferences: preferencesSchema,
  traceId: z.string(),
});

const unsubscribeResponseSchema = z.object({
  subscriptionId: z.number().int(),
  status: z.literal('unsubscribed'),
  previousStatus: z.string(),
  traceId: z.string(),
});

type SubscribePayload = z.infer<typeof subscribePayloadSchema>;
type UnsubscribePayload = z.infer<typeof unsubscribePayloadSchema>;
type Preferences = z.infer<typeof preferencesSchema>;

type SubscriptionRow = {
  subscription_id: number;
  section_id: number | null;
  term_id: string;
  campus_code: string;
  index_number: string;
  contact_type: ContactType;
  contact_hash: string;
  contact_value: string | null;
  locale: string | null;
  status: SubscriptionStatus;
  is_verified: number;
  unsubscribe_token: string | null;
  metadata: string | null;
  last_known_section_status: string | null;
};

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

const ipAttempts = new Map<string, number[]>();

export async function registerSubscriptionRoutes(app: FastifyInstance) {
  app.post(
    '/subscribe',
    {
      schema: {
        body: subscribePayloadSchema,
        response: {
          200: subscribeResponseSchema,
          201: subscribeResponseSchema,
          400: errorResponseSchema,
          404: errorResponseSchema,
          409: errorResponseSchema,
        },
      },
    },
    async (request, reply) => {
      const body = request.body as SubscribePayload;
      const traceId = String(request.id);
      reply.header('x-trace-id', traceId);

      const clientIp = resolveClientIp(request.ip, request.headers['x-forwarded-for']);
      if (!allowIpAttempt(clientIp)) {
        return sendError(reply, 400, 'rate_limited', 'Too many subscribe attempts from this IP', traceId);
      }

      const contact = normalizeContact(body.contactType, body.contactValue);
      if (!contact) {
        return sendError(reply, 400, 'invalid_contact', 'Contact value is not valid for the chosen channel', traceId);
      }

      const db = request.server.container.getDb();
      if (!lookupTerm(db, body.term)) {
        return sendError(reply, 404, 'section_not_found', `Unknown term ${body.term}`, traceId);
      }
      if (!lookupCampus(db, body.campus)) {
        return sendError(reply, 404, 'section_not_found', `Unknown campus ${body.campus}`, traceId);
      }

      const section = findSection(db, body.term, body.campus, body.sectionIndex);
      if (!section) {
        const conflicting = findConflictingSection(db, body.term, body.sectionIndex);
        if (conflicting && conflicting.campus_code !== body.campus) {
          return sendError(
            reply,
            409,
            'section_conflict',
            `Section ${body.sectionIndex} belongs to campus ${conflicting.campus_code}`,
            traceId,
            {
              campus: conflicting.campus_code,
              term: conflicting.term_id,
            },
          );
        }
      }

      const existing = findExistingSubscription(db, {
        contactHash: contact.hash,
        contactType: body.contactType,
        term: body.term,
        campus: body.campus,
        sectionIndex: body.sectionIndex,
      });
      if (existing) {
        const response = formatSubscriptionResponse(existing, traceId, true);
        request.log.info({ subscriptionId: existing.subscription_id, traceId }, 'subscription.reused');
        return reply.status(200).send(response);
      }

      if (!checkContactLimit(db, contact.hash, body.contactType)) {
        return sendError(
          reply,
          400,
          'rate_limited',
          'Contact reached the maximum number of active subscriptions',
          traceId,
        );
      }

      if (section?.section_id && !checkSectionLimit(db, section.section_id)) {
        return sendError(reply, 400, 'rate_limited', 'Section reached the maximum number of subscriptions', traceId);
      }

      const now = new Date().toISOString();
      const status: SubscriptionStatus = body.contactType === 'discord_channel' ? 'active' : 'pending';
      const preferences = mergePreferences(body.preferences);
      const unsubscribeToken = body.contactType === 'discord_channel' ? null : crypto.randomBytes(16).toString('hex');
      const metadata = buildMetadata(preferences, {
        client: buildClientContext(body.clientContext, clientIp, request.headers['user-agent']),
        discord: body.contactType.startsWith('discord') ? body.discord : undefined,
      });

      const subscriptionId = createSubscription(db, {
        sectionId: section?.section_id ?? null,
        body,
        contact,
        now,
        status,
        unsubscribeToken,
        metadata,
      });

      request.log.info(
        {
          event: 'subscription.created',
          subscriptionId,
          contactType: body.contactType,
          sectionResolved: Boolean(section?.section_id),
          version: API_VERSION,
        },
        'subscription created',
      );

      const payload = {
        subscriptionId,
        status,
        requiresVerification: status === 'pending',
        existing: false,
        unsubscribeToken,
        term: body.term,
        campus: body.campus,
        sectionIndex: body.sectionIndex,
        sectionResolved: Boolean(section?.section_id),
        preferences,
        traceId,
      };

      return reply.status(201).send(payload);
    },
  );

  app.post(
    '/unsubscribe',
    {
      schema: {
        body: unsubscribePayloadSchema,
        response: {
          200: unsubscribeResponseSchema,
          400: errorResponseSchema,
          404: errorResponseSchema,
        },
      },
    },
    async (request, reply) => {
      const body = request.body as UnsubscribePayload;
      const traceId = String(request.id);
      reply.header('x-trace-id', traceId);
      const db = request.server.container.getDb();

      const row = findSubscriptionForUnsubscribe(db, body);
      if (!row) {
        return sendError(reply, 404, 'subscription_not_found', 'Subscription not found', traceId);
      }

      if (body.contactValue) {
        const normalized = normalizeContact(row.contact_type, body.contactValue);
        if (!normalized || normalized.hash !== row.contact_hash) {
          return sendError(reply, 400, 'contact_mismatch', 'Contact value does not match subscription', traceId);
        }
      }

      const response = {
        subscriptionId: row.subscription_id,
        status: 'unsubscribed' as const,
        previousStatus: row.status,
        traceId,
      };

      if (row.status === 'unsubscribed') {
        return reply.status(200).send(response);
      }

      cancelSubscription(db, row.subscription_id, body.reason ?? 'user_request', row.last_known_section_status);
      request.log.info({ subscriptionId: row.subscription_id, traceId }, 'subscription.unsubscribed');
      return reply.status(200).send(response);
    },
  );
}

function sendError(
  reply: FastifyReply,
  statusCode: number,
  code: string,
  message: string,
  traceId: string,
  details?: Record<string, unknown>,
) {
  return reply.status(statusCode).send({
    error: {
      code,
      message,
      traceId,
      ...(details ? { details } : {}),
    },
  });
}

function normalizeContact(contactType: ContactType, rawValue: string) {
  const trimmed = rawValue.trim();
  if (contactType === 'email') {
    const match = trimmed.match(/<([^>]+)>/);
    const email = match ? match[1] : trimmed;
    const normalized = email.toLowerCase();
    if (!EMAIL_REGEX.test(normalized)) {
      return null;
    }
    return {
      value: normalized,
      hash: sha1(normalized),
    };
  }

  if (contactType === 'discord_user' || contactType === 'discord_channel') {
    if (!DISCORD_SNOWFLAKE.test(trimmed)) {
      return null;
    }
    return {
      value: trimmed,
      hash: sha1(trimmed),
    };
  }

  return null;
}

function sha1(value: string) {
  return crypto.createHash('sha1').update(value.toLowerCase()).digest('hex');
}

function resolveClientIp(requestIp: string, forwarded?: string | string[]) {
  if (typeof forwarded === 'string' && forwarded.length) {
    const [first] = forwarded.split(',').map((token) => token.trim());
    if (first) {
      return first;
    }
  }
  if (Array.isArray(forwarded) && forwarded.length) {
    return forwarded[0] ?? requestIp;
  }
  return requestIp;
}

function allowIpAttempt(ip: string | undefined) {
  if (!ip) {
    return true;
  }
  const now = Date.now();
  const history = ipAttempts.get(ip) ?? [];
  const nextHistory = history.filter((timestamp) => now - timestamp < IP_WINDOW_MS);
  nextHistory.push(now);
  ipAttempts.set(ip, nextHistory);
  return nextHistory.length <= IP_MAX_ATTEMPTS;
}

function mergePreferences(preferences?: z.infer<typeof preferencesInputSchema>): Preferences {
  if (!preferences) {
    return defaultPreferences;
  }
  const notifyOn = preferences.notifyOn?.length ? preferences.notifyOn : defaultPreferences.notifyOn;
  const deliveryWindow = preferences.deliveryWindow
    ? {
        startMinutes:
          preferences.deliveryWindow.startMinutes ?? defaultPreferences.deliveryWindow.startMinutes,
        endMinutes: preferences.deliveryWindow.endMinutes ?? defaultPreferences.deliveryWindow.endMinutes,
      }
    : defaultPreferences.deliveryWindow;

  return {
    notifyOn,
    maxNotifications: preferences.maxNotifications ?? defaultPreferences.maxNotifications,
    deliveryWindow,
    snoozeUntil:
      preferences.snoozeUntil !== undefined ? preferences.snoozeUntil : defaultPreferences.snoozeUntil,
    channelMetadata: preferences.channelMetadata ?? defaultPreferences.channelMetadata,
  };
}

function buildClientContext(
  clientContext: SubscribePayload['clientContext'],
  resolvedIp: string | undefined,
  userAgentHeader?: string,
) {
  const ip = resolvedIp ?? clientContext?.ip;
  const userAgent = clientContext?.userAgent ?? (typeof userAgentHeader === 'string' ? userAgentHeader : undefined);
  if (!ip && !userAgent) {
    return undefined;
  }
  return {
    ...(ip ? { ip } : {}),
    ...(userAgent ? { userAgent } : {}),
  };
}

function buildMetadata(
  preferences: Preferences,
  extras: {
    client?: { ip?: string; userAgent?: string };
    discord?: SubscribePayload['discord'];
  },
) {
  const metadata: Record<string, unknown> = {
    preferences,
  };
  if (extras.client) {
    metadata.client = extras.client;
  }
  if (extras.discord) {
    metadata.discord = extras.discord;
  }
  return JSON.stringify(metadata);
}

function lookupTerm(db: any, term: string) {
  const row = db.prepare('SELECT term_id FROM terms WHERE term_id = ?').get(term) as { term_id: string } | undefined;
  return Boolean(row);
}

function lookupCampus(db: any, campus: string) {
  const row = db.prepare('SELECT campus_code FROM campuses WHERE campus_code = ?').get(campus) as
    | { campus_code: string }
    | undefined;
  return Boolean(row);
}

function findSection(db: any, term: string, campus: string, sectionIndex: string) {
  return db
    .prepare(
      `
        SELECT section_id, term_id, campus_code, index_number
        FROM sections
        WHERE term_id = ?
          AND campus_code = ?
          AND index_number = ?
      `,
    )
    .get(term, campus, sectionIndex) as
    | { section_id: number; term_id: string; campus_code: string; index_number: string }
    | undefined;
}

function findConflictingSection(db: any, term: string, sectionIndex: string) {
  return db
    .prepare(
      `
        SELECT term_id, campus_code
        FROM sections
        WHERE term_id = ?
          AND index_number = ?
        LIMIT 1
      `,
    )
    .get(term, sectionIndex) as { term_id: string; campus_code: string } | undefined;
}

function findExistingSubscription(
  db: any,
  args: {
    contactHash: string;
    contactType: ContactType;
    term: string;
    campus: string;
    sectionIndex: string;
  },
) {
  return db
    .prepare(
      `
        SELECT *
        FROM subscriptions
        WHERE contact_hash = ?
          AND contact_type = ?
          AND term_id = ?
          AND campus_code = ?
          AND index_number = ?
          AND status IN (${ACTIVE_STATUSES.map(() => '?').join(', ')})
        ORDER BY subscription_id DESC
        LIMIT 1
      `,
    )
    .get(args.contactHash, args.contactType, args.term, args.campus, args.sectionIndex, ...ACTIVE_STATUSES) as
    | SubscriptionRow
    | undefined;
}

function checkContactLimit(db: any, contactHash: string, contactType: ContactType) {
  const row = db
    .prepare(
      `
        SELECT COUNT(*) as count
        FROM subscriptions
        WHERE contact_hash = ?
          AND contact_type = ?
          AND status IN (${ACTIVE_STATUSES.map(() => '?').join(', ')})
      `,
    )
    .get(contactHash, contactType, ...ACTIVE_STATUSES) as { count: number };
  return row.count < CONTACT_ACTIVE_LIMIT;
}

function checkSectionLimit(db: any, sectionId: number) {
  const row = db
    .prepare(
      `
        SELECT COUNT(*) as count
        FROM subscriptions
        WHERE section_id = ?
          AND status IN (${ACTIVE_STATUSES.map(() => '?').join(', ')})
      `,
    )
    .get(sectionId, ...ACTIVE_STATUSES) as { count: number };
  return row.count < SECTION_ACTIVE_LIMIT;
}

function createSubscription(
  db: any,
  args: {
    sectionId: number | null;
    body: SubscribePayload;
    contact: { value: string; hash: string };
    now: string;
    status: SubscriptionStatus;
    unsubscribeToken: string | null;
    metadata: string;
  },
) {
  const insert = db.prepare(
    `
      INSERT INTO subscriptions (
        section_id,
        term_id,
        campus_code,
        index_number,
        contact_type,
        contact_value,
        contact_hash,
        locale,
        status,
        is_verified,
        created_at,
        updated_at,
        unsubscribe_token,
        metadata
      )
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `,
  );

  const insertEvent = db.prepare(
    `
      INSERT INTO subscription_events (subscription_id, event_type, payload, created_at)
      VALUES (?, ?, ?, ?)
    `,
  );

  const transaction = db.transaction(() => {
    const result = insert.run(
      args.sectionId,
      args.body.term,
      args.body.campus,
      args.body.sectionIndex,
      args.body.contactType,
      args.contact.value,
      args.contact.hash,
      args.body.locale ?? DEFAULT_LOCALE,
      args.status,
      args.status === 'active' ? 1 : 0,
      args.now,
      args.now,
      args.unsubscribeToken,
      args.metadata,
    );
    const subscriptionId = Number(result.lastInsertRowid);
    insertEvent.run(subscriptionId, 'created', JSON.stringify({ via: 'api' }), args.now);
    return subscriptionId;
  });

  return transaction();
}

function formatSubscriptionResponse(row: SubscriptionRow, traceId: string, existing: boolean) {
  const preferences = readPreferences(row.metadata);
  return {
    subscriptionId: row.subscription_id,
    status: row.status,
    requiresVerification: row.contact_type !== 'discord_channel' && row.status === 'pending',
    existing,
    unsubscribeToken: row.unsubscribe_token ?? null,
    term: row.term_id,
    campus: row.campus_code,
    sectionIndex: row.index_number,
    sectionResolved: Boolean(row.section_id),
    preferences,
    traceId,
  };
}

function readPreferences(metadata: string | null): Preferences {
  if (!metadata) {
    return defaultPreferences;
  }
  try {
    const parsed = JSON.parse(metadata) as { preferences?: Preferences };
    if (parsed.preferences) {
      return mergePreferences(parsed.preferences);
    }
  } catch {
    // best-effort parse
  }
  return defaultPreferences;
}

function findSubscriptionForUnsubscribe(db: any, body: UnsubscribePayload) {
  if (body.subscriptionId) {
    return db
      .prepare(
        `
          SELECT *
          FROM subscriptions
          WHERE subscription_id = ?
        `,
      )
      .get(body.subscriptionId) as SubscriptionRow | undefined;
  }

  if (body.unsubscribeToken) {
    return db
      .prepare(
        `
          SELECT *
          FROM subscriptions
          WHERE unsubscribe_token = ?
        `,
      )
      .get(body.unsubscribeToken) as SubscriptionRow | undefined;
  }

  return undefined;
}

function cancelSubscription(db: any, subscriptionId: number, reason: string, statusSnapshot: string | null) {
  const now = new Date().toISOString();
  const update = db.prepare(
    `
      UPDATE subscriptions
      SET status = 'unsubscribed',
          is_verified = 0,
          contact_value = '',
          updated_at = ?
      WHERE subscription_id = ?
    `,
  );

  const insertEvent = db.prepare(
    `
      INSERT INTO subscription_events (
        subscription_id,
        event_type,
        section_status_snapshot,
        payload,
        created_at
      ) VALUES (?, 'unsubscribed', ?, ?, ?)
    `,
  );

  const tx = db.transaction(() => {
    update.run(now, subscriptionId);
    insertEvent.run(subscriptionId, statusSnapshot, JSON.stringify({ reason }), now);
  });

  tx();
}
