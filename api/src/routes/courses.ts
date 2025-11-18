import type { FastifyInstance } from 'fastify';
import { z } from 'zod';

import {
  API_VERSION,
  enumArrayParam,
  optionalBooleanParam,
  optionalMinutesParam,
  paginationSchema,
  sortDirectionSchema,
  stringOrArrayParam,
} from './sharedSchemas.js';

const courseQuerySchema = paginationSchema(100, 20)
  .extend({
    term: z.string().min(4, 'term is required'),
    campus: stringOrArrayParam,
    subject: stringOrArrayParam,
    q: z.string().trim().min(2).optional(),
    level: stringOrArrayParam,
    courseNumber: z.string().trim().optional(),
    coreCode: stringOrArrayParam,
    creditsMin: z.coerce.number().int().min(0).max(20).optional(),
    creditsMax: z.coerce.number().int().min(0).max(20).optional(),
    delivery: enumArrayParam(['in_person', 'online', 'hybrid']),
    hasOpenSection: optionalBooleanParam,
    meetingDays: stringOrArrayParam,
    meetingStart: optionalMinutesParam,
    meetingEnd: optionalMinutesParam,
    instructor: z.string().trim().optional(),
    requiresPermission: optionalBooleanParam,
    sortBy: z.enum(['subject', 'courseNumber', 'title', 'credits', 'sectionsOpen', 'updatedAt']).optional(),
    sortDir: sortDirectionSchema.optional(),
    include: stringOrArrayParam,
  })
  .superRefine((value, ctx) => {
    if (!value.campus?.length && !value.subject?.length) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        message: 'At least one of campus or subject is required',
        path: ['campus'],
      });
    }

    if (value.creditsMin !== undefined && value.creditsMax !== undefined && value.creditsMin > value.creditsMax) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        message: 'creditsMin must not exceed creditsMax',
        path: ['creditsMin'],
      });
    }

    if (value.meetingStart !== undefined && value.meetingEnd !== undefined && value.meetingStart > value.meetingEnd) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        message: 'meetingStart must be <= meetingEnd',
        path: ['meetingStart'],
      });
    }
  });

export type CoursesQuery = z.infer<typeof courseQuerySchema>;

export async function registerCourseRoutes(app: FastifyInstance) {
  app.get(
    '/courses',
    {
      schema: {
        querystring: courseQuerySchema,
        response: {
          200: z.object({
            meta: z.object({
              page: z.number().int(),
              pageSize: z.number().int(),
              total: z.number().int(),
              hasNext: z.boolean(),
              generatedAt: z.string(),
              version: z.string(),
            }),
            data: z.array(z.record(z.string(), z.unknown())),
          }),
        },
      },
    },
    async (request) => {
      const query = request.query as CoursesQuery;
      const meta = {
        page: query.page,
        pageSize: query.pageSize,
        total: 0,
        hasNext: false,
        generatedAt: new Date().toISOString(),
        version: API_VERSION,
      };

      // Persistence integration happens in ST-20251113-act-008-02-filter-engine.
      return {
        meta,
        data: [],
      };
    },
  );
}
