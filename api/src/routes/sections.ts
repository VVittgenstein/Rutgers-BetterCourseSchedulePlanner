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

const sectionQuerySchema = paginationSchema(200, 50)
  .extend({
    term: z.string().min(4, 'term is required'),
    campus: stringOrArrayParam,
    subject: stringOrArrayParam,
    courseId: z.coerce.number().int().positive().optional(),
    courseString: z.string().trim().optional(),
    index: z.string().trim().optional(),
    sectionNumber: z.string().trim().optional(),
    openStatus: enumArrayParam(['OPEN', 'CLOSED', 'WAITLIST']),
    isOpen: optionalBooleanParam,
    delivery: enumArrayParam(['in_person', 'online', 'hybrid']),
    meetingDay: stringOrArrayParam,
    meetingStart: optionalMinutesParam,
    meetingEnd: optionalMinutesParam,
    meetingCampus: stringOrArrayParam,
    instructor: z.string().trim().optional(),
    majors: stringOrArrayParam,
    permissionOnly: optionalBooleanParam,
    hasWaitlist: optionalBooleanParam,
    updatedSince: z.string().datetime({ offset: true }).optional(),
    sortBy: z.enum(['index', 'openStatusUpdatedAt', 'meetingStart', 'meetingEnd', 'instructor', 'campus']).optional(),
    sortDir: sortDirectionSchema.optional(),
  })
  .superRefine((value, ctx) => {
    if (
      !value.campus?.length &&
      !value.subject?.length &&
      !value.courseId &&
      !value.courseString &&
      !value.index
    ) {
      ctx.addIssue({
        code: z.ZodIssueCode.custom,
        message: 'Provide at least one of campus/subject/course/index filters to limit results',
        path: ['campus'],
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

export type SectionsQuery = z.infer<typeof sectionQuerySchema>;

export async function registerSectionRoutes(app: FastifyInstance) {
  app.get(
    '/sections',
    {
      schema: {
        querystring: sectionQuerySchema,
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
      const query = request.query as SectionsQuery;
      const startedAt = process.hrtime.bigint();
      const meta = {
        page: query.page,
        pageSize: query.pageSize,
        total: 0,
        hasNext: false,
        generatedAt: new Date().toISOString(),
        version: API_VERSION,
      };
      const durationMs = Number(process.hrtime.bigint() - startedAt) / 1_000_000;
      request.log.info(
        {
          event: 'query.metrics',
          target: 'sections',
          durationMs,
          totalMatching: meta.total,
          totalReturned: 0,
          page: query.page,
          pageSize: query.pageSize,
          filters: summarizeSectionFilters(query),
        },
        'sections query executed',
      );

      return {
        meta,
        data: [],
      };
    },
  );
}

function summarizeSectionFilters(query: SectionsQuery) {
  return {
    term: query.term,
    campus: query.campus ?? [],
    subject: query.subject ?? [],
    courseId: query.courseId,
    courseString: query.courseString,
    index: query.index,
    sectionNumber: query.sectionNumber,
    openStatus: query.openStatus ?? [],
    isOpen: query.isOpen,
    delivery: query.delivery ?? [],
    meetingDay: query.meetingDay ?? [],
    meetingWindow:
      query.meetingStart !== undefined || query.meetingEnd !== undefined
        ? { start: query.meetingStart, end: query.meetingEnd }
        : undefined,
    meetingCampus: query.meetingCampus ?? [],
    instructorProvided: query.instructor ? true : undefined,
    majors: query.majors ?? [],
    permissionOnly: query.permissionOnly,
    hasWaitlist: query.hasWaitlist,
    updatedSince: query.updatedSince,
    sort: query.sortBy ? { by: query.sortBy, direction: query.sortDir ?? 'asc' } : undefined,
  };
}
