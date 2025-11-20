import type { FastifyInstance } from 'fastify';

import { API_VERSION } from './sharedSchemas.js';

export async function registerFilterRoutes(app: FastifyInstance) {
  app.get('/filters', async () => {
    return {
      meta: {
        generatedAt: new Date().toISOString(),
        version: API_VERSION,
      },
      data: {
        terms: [],
        campuses: [],
        subjects: [],
        coreCodes: [],
        levels: ['UG', 'GR'],
        deliveryMethods: ['in_person', 'online', 'hybrid'],
      },
    };
  });
}
