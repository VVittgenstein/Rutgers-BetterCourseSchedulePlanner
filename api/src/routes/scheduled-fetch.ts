import type { FastifyInstance, FastifyReply } from 'fastify';
import { z } from 'zod';

import { API_VERSION } from './sharedSchemas.js';

const scheduledFetchConfigSchema = z.object({
  enabled: z.boolean(),
  intervalMinutes: z.number().int().min(1).max(30),
  term: z.string(),
  campus: z.string(),
  mode: z.enum(['full-init', 'incremental']),
});

const scheduledFetchStateSchema = z.object({
  config: scheduledFetchConfigSchema,
  status: z.enum(['idle', 'scheduled', 'running']),
  lastRunAt: z.string().nullable(),
  lastRunStatus: z.enum(['success', 'error']).nullable(),
  lastRunMessage: z.string().nullable(),
  nextRunAt: z.string().nullable(),
});

const scheduledFetchResponseSchema = z.object({
  meta: z.object({
    generatedAt: z.string(),
    version: z.string(),
  }),
  data: scheduledFetchStateSchema,
});

const errorResponseSchema = z.object({
  error: z.object({
    code: z.string(),
    message: z.string(),
    traceId: z.string(),
  }),
});

const startScheduledFetchSchema = z.object({
  intervalMinutes: z.coerce.number().int().min(1).max(30).optional(),
  term: z.string().trim().min(4).max(12),
  campus: z
    .string()
    .trim()
    .min(2)
    .max(12)
    .transform((value) => value.toUpperCase()),
  mode: z.enum(['full-init', 'incremental']).default('incremental'),
});

export async function registerScheduledFetchRoutes(app: FastifyInstance) {
  app.get(
    '/scheduled-fetch',
    {
      schema: {
        response: {
          200: scheduledFetchResponseSchema,
        },
      },
    },
    async () => {
      const state = app.container.scheduledFetcher.getState();
      return {
        meta: {
          generatedAt: new Date().toISOString(),
          version: API_VERSION,
        },
        data: state,
      };
    },
  );

  app.post(
    '/scheduled-fetch',
    {
      schema: {
        body: startScheduledFetchSchema,
        response: {
          200: scheduledFetchResponseSchema,
          400: errorResponseSchema,
          500: errorResponseSchema,
        },
      },
    },
    async (request, reply) => {
      const traceId = String(request.id);
      reply.header('x-trace-id', traceId);

      const body = request.body as z.infer<typeof startScheduledFetchSchema>;

      try {
        const state = app.container.scheduledFetcher.start({
          intervalMinutes: body.intervalMinutes,
          term: body.term,
          campus: body.campus,
          mode: body.mode,
        });

        return {
          meta: {
            generatedAt: new Date().toISOString(),
            version: API_VERSION,
          },
          data: state,
        };
      } catch (error) {
        return sendError(
          reply,
          400,
          'SCHEDULED_FETCH_START_FAILED',
          (error as Error).message || 'Failed to start scheduled fetch',
          traceId,
        );
      }
    },
  );

  app.delete(
    '/scheduled-fetch',
    {
      schema: {
        response: {
          200: scheduledFetchResponseSchema,
        },
      },
    },
    async () => {
      const state = app.container.scheduledFetcher.stop();
      return {
        meta: {
          generatedAt: new Date().toISOString(),
          version: API_VERSION,
        },
        data: state,
      };
    },
  );

  app.patch(
    '/scheduled-fetch',
    {
      schema: {
        body: z.object({
          intervalMinutes: z.coerce.number().int().min(1).max(30).optional(),
          term: z.string().trim().min(4).max(12).optional(),
          campus: z
            .string()
            .trim()
            .min(2)
            .max(12)
            .transform((value) => value.toUpperCase())
            .optional(),
          mode: z.enum(['full-init', 'incremental']).optional(),
        }),
        response: {
          200: scheduledFetchResponseSchema,
        },
      },
    },
    async (request) => {
      const body = request.body as Partial<z.infer<typeof startScheduledFetchSchema>>;
      app.container.scheduledFetcher.updateConfig(body);
      const state = app.container.scheduledFetcher.getState();
      return {
        meta: {
          generatedAt: new Date().toISOString(),
          version: API_VERSION,
        },
        data: state,
      };
    },
  );
}

function sendError(reply: FastifyReply, status: number, code: string, message: string, traceId: string) {
  return reply.status(status).send({
    error: {
      code,
      message,
      traceId,
    },
  });
}
