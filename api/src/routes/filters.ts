import type { FastifyInstance } from 'fastify';

import { fetchFiltersDictionary } from '../queries/filters.js';
import { API_VERSION } from './sharedSchemas.js';

export async function registerFilterRoutes(app: FastifyInstance) {
  app.get('/filters', async (request) => {
    const db = request.server.container.getDb();
    const dictionary = fetchFiltersDictionary(db);

    return {
      meta: {
        generatedAt: new Date().toISOString(),
        version: API_VERSION,
      },
      data: dictionary,
    };
  });
}
