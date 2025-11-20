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
import { executeCourseSearch } from '../queries/course_search.js';

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
      const db = request.server.container.getDb();
      const startedAt = process.hrtime.bigint();
      const { data, total } = executeCourseSearch(db, query);
      const durationMs = Number(process.hrtime.bigint() - startedAt) / 1_000_000;
      const meta = {
        page: query.page,
        pageSize: query.pageSize,
        total,
        hasNext: query.page * query.pageSize < total,
        generatedAt: new Date().toISOString(),
        version: API_VERSION,
      };
      request.log.info(
        {
          event: 'query.metrics',
          target: 'courses',
          durationMs,
          totalMatching: total,
          totalReturned: data.length,
          page: query.page,
          pageSize: query.pageSize,
          filters: summarizeCourseFilters(query),
        },
        'courses query executed',
      );

      return {
        meta,
        data,
      };
    },
  );
}

function summarizeCourseFilters(query: CoursesQuery) {
  return {
    term: query.term,
    campus: query.campus ?? [],
    subject: query.subject ?? [],
    level: query.level ?? [],
    hasSearchQuery: Boolean(query.q),
    courseNumber: query.courseNumber,
    coreCode: query.coreCode ?? [],
    credits:
      query.creditsMin !== undefined || query.creditsMax !== undefined
        ? { min: query.creditsMin, max: query.creditsMax }
        : undefined,
    delivery: query.delivery ?? [],
    hasOpenSection: query.hasOpenSection,
    meetingDays: query.meetingDays ?? [],
    meetingWindow:
      query.meetingStart !== undefined || query.meetingEnd !== undefined
        ? { start: query.meetingStart, end: query.meetingEnd }
        : undefined,
    instructorProvided: query.instructor ? true : undefined,
    requiresPermission: query.requiresPermission,
    sort: query.sortBy ? { by: query.sortBy, direction: query.sortDir ?? 'asc' } : undefined,
    include: query.include ?? [],
  };
}
