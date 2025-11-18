import type { FastifyInstance } from 'fastify';

import { API_VERSION } from './sharedSchemas.js';

export async function registerHealthRoutes(app: FastifyInstance) {
  app.get('/health', async (request) => {
    const dependencies: Record<string, 'up' | 'down'> = { sqlite: 'up' };
    let status: 'ok' | 'degraded' = 'ok';

    try {
      const db = app.container.getDb();
      db.prepare('select 1').get();
    } catch (error) {
      status = 'degraded';
      dependencies.sqlite = 'down';
      request.log.error({ err: error }, 'sqlite health check failed');
    }

    return {
      status,
      dependencies,
      version: API_VERSION,
      generatedAt: new Date().toISOString(),
    };
  });
}
