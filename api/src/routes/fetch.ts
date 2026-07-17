import type { FastifyInstance, FastifyReply } from 'fastify';
import { z } from 'zod';

import { API_VERSION } from './sharedSchemas.js';
import { getActiveFetchJob, startFetchJob, type FetchJob } from '../services/fetchRunner.js';

type TermSeason = 'winter' | 'spring' | 'summer' | 'fall';

const SEASON_CODES: Record<TermSeason, string> = {
  winter: '0',
  spring: '1',
  summer: '7',
  fall: '9',
};

const TERM_ALIASES: Record<string, TermSeason> = {
  W: 'winter',
  WI: 'winter',
  WINTER: 'winter',
  S: 'spring',
  SP: 'spring',
  SPR: 'spring',
  SPRING: 'spring',
  SU: 'summer',
  SUM: 'summer',
  SUMMER: 'summer',
  F: 'fall',
  FA: 'fall',
  FALL: 'fall',
};

const fetchJobSchema = z.object({
  id: z.string(),
  term: z.string(),
  campus: z.string(),
  mode: z.enum(['full-init', 'incremental']),
  status: z.enum(['running', 'success', 'error']),
  startedAt: z.string(),
  finishedAt: z.string().nullable(),
  message: z.string().optional(),
  logFile: z.string().optional(),
});

const fetchStatusResponseSchema = z.object({
  meta: z.object({
    generatedAt: z.string(),
    version: z.string(),
  }),
  data: z.object({
    job: fetchJobSchema.nullable(),
  }),
});

const errorResponseSchema = z.object({
  error: z.object({
    code: z.string(),
    message: z.string(),
    traceId: z.string(),
  }),
});

const fetchRequestSchema = z
  .object({
    year: z.coerce.number().int().min(2015).max(2100).optional(),
    season: z.enum(['winter', 'spring', 'summer', 'fall']).optional(),
    term: z.string().trim().min(4).max(12).optional(),
    campus: z
      .string()
      .trim()
      .min(2)
      .max(12)
      .transform((value) => value.toUpperCase()),
    mode: z.enum(['full-init', 'incremental']).default('full-init'),
  })
  .refine((value) => Boolean(value.term || (value.year && value.season)), {
    message: 'Provide term or year+season',
    path: ['term'],
  });

type FetchRequest = z.infer<typeof fetchRequestSchema>;

export async function registerFetchRoutes(app: FastifyInstance) {
  app.get(
    '/fetch',
    {
      schema: {
        response: {
          200: fetchStatusResponseSchema,
        },
      },
    },
    async () => {
      return {
        meta: {
          generatedAt: new Date().toISOString(),
          version: API_VERSION,
        },
        data: {
          job: getActiveFetchJob(),
        },
      };
    },
  );

  app.post(
    '/fetch',
    {
      schema: {
        body: fetchRequestSchema,
        response: {
          200: fetchStatusResponseSchema,
          202: fetchStatusResponseSchema,
          400: errorResponseSchema,
          409: errorResponseSchema,
          500: errorResponseSchema,
        },
      },
    },
    async (request, reply) => {
      const traceId = String(request.id);
      reply.header('x-trace-id', traceId);

      const body = request.body as FetchRequest;
      let term: string;
      try {
        term = resolveTerm(body);
      } catch (error) {
        return sendError(reply, 400, 'INVALID_TERM', (error as Error).message, traceId);
      }

      const active = getActiveFetchJob();
      if (active && active.status === 'running') {
        return sendError(
          reply,
          409,
          'FETCH_IN_PROGRESS',
          `Fetch already running for term ${active.term} (${active.campus}).`,
          traceId,
        );
      }

      let job: FetchJob;
      try {
        job = startFetchJob({
          term,
          campus: body.campus,
          mode: body.mode,
          sqliteFile: app.container.config.sqliteFile,
          logger: request.log,
        });
      } catch (error) {
        return sendError(
          reply,
          500,
          'FETCH_START_FAILED',
          (error as Error).message || 'Failed to start fetch',
          traceId,
        );
      }

      reply.status(202);
      return {
        meta: {
          generatedAt: new Date().toISOString(),
          version: API_VERSION,
        },
        data: { job },
      };
    },
  );
}

function resolveTerm(body: FetchRequest): string {
  if (body.term) {
    return normalizeTerm(body.term);
  }
  if (body.year && body.season) {
    return `${SEASON_CODES[body.season]}${body.year}`;
  }
  throw new Error('Provide term or year+season');
}

function normalizeTerm(raw: string): string {
  const stripped = raw.replace(/[-_\s]/g, '').toUpperCase();
  const fiveDigit = stripped.match(/^([0179])(\d{4})$/);
  if (fiveDigit) {
    const [, termCode, year] = fiveDigit;
    return `${termCode}${year}`;
  }

  const swapped = stripped.match(/^(\d{4})([0179])$/);
  if (swapped) {
    const [, year, termCode] = swapped;
    return `${termCode}${year}`;
  }

  const aliasMatch = stripped.match(/^([A-Z]+)(\d{4})$/);
  if (aliasMatch) {
    const [, alias, year] = aliasMatch;
    const season = TERM_ALIASES[alias];
    if (!season) {
      throw new Error(`Unrecognized term alias: ${alias}`);
    }
    return `${SEASON_CODES[season]}${year}`;
  }

  throw new Error('Unable to parse term. Try formats like 12024 or FA2024.');
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
