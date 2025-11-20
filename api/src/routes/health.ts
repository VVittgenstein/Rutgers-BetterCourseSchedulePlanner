import type Database from 'better-sqlite3';
import type { FastifyInstance, FastifyRequest } from 'fastify';

import { API_VERSION } from './sharedSchemas.js';

export const REQUIRED_SCHEMA_TABLES = [
  'courses',
  'sections',
  'course_core_attributes',
  'course_search_fts',
  'section_meetings',
  'subjects',
] as const;

type ReadinessChecks = {
  sqlite: {
    status: 'up' | 'down';
    message?: string;
  };
  tables: {
    status: 'up' | 'down';
    missing: string[];
  };
};

export async function registerHealthRoutes(app: FastifyInstance) {
  app.get('/health', async (request) => {
    const checks = runReadinessChecks(request);
    const dependencies: Record<string, 'up' | 'down'> = {
      sqlite: checks.sqlite.status,
      schema: checks.tables.status,
    };
    const status: 'ok' | 'degraded' = Object.values(dependencies).every((value) => value === 'up') ? 'ok' : 'degraded';

    return {
      status,
      dependencies,
      version: API_VERSION,
      generatedAt: new Date().toISOString(),
    };
  });

  app.get('/ready', async (request, reply) => {
    const checks = runReadinessChecks(request);
    const status: 'ready' | 'not_ready' = checks.sqlite.status === 'up' && checks.tables.status === 'up' ? 'ready' : 'not_ready';
    const payload = {
      status,
      checks,
      version: API_VERSION,
      generatedAt: new Date().toISOString(),
    };

    if (status !== 'ready') {
      return reply.status(503).send(payload);
    }

    return payload;
  });
}

function runReadinessChecks(request: FastifyRequest): ReadinessChecks {
  const checks: ReadinessChecks = {
    sqlite: { status: 'up' },
    tables: { status: 'up', missing: [] },
  };

  try {
    const db = request.server.container.getDb();
    db.prepare('select 1').get();
    const missingTables = findMissingTables(db);
    if (missingTables.length) {
      checks.tables.status = 'down';
      checks.tables.missing = missingTables;
      request.log.error({ missingTables }, 'required sqlite tables missing');
    }
  } catch (error) {
    checks.sqlite.status = 'down';
    checks.sqlite.message = error instanceof Error ? error.message : 'Unknown sqlite error';
    request.log.error({ err: error }, 'sqlite readiness probe failed');
    checks.tables.status = 'down';
    checks.tables.missing = [...REQUIRED_SCHEMA_TABLES];
  }

  return checks;
}

function findMissingTables(db: Database.Database) {
  const placeholders = REQUIRED_SCHEMA_TABLES.map(() => '?').join(', ');
  const rows = db
    .prepare(
      `
        SELECT name
        FROM sqlite_schema
        WHERE type = 'table'
          AND name IN (${placeholders})
      `,
    )
    .all(...REQUIRED_SCHEMA_TABLES);

  const existing = new Set((rows as { name: string }[]).map((row) => row.name));
  return REQUIRED_SCHEMA_TABLES.filter((table) => !existing.has(table));
}
