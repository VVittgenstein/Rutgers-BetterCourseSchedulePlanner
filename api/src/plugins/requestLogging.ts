import fp from 'fastify-plugin';
import type { FastifyPluginAsync } from 'fastify';

const startTimeSymbol = Symbol('requestStartMs');

export const requestLoggingPlugin: FastifyPluginAsync = fp(async (app) => {
  app.addHook('onRequest', async (request) => {
    (request as typeof request & { [startTimeSymbol]?: number })[startTimeSymbol] = Date.now();
    request.log.info({ reqId: request.id, method: request.method, url: request.url }, 'request received');
  });

  app.addHook('onResponse', async (request, reply) => {
    const startedAt = (request as typeof request & { [startTimeSymbol]?: number })[startTimeSymbol];
    const durationMs = typeof startedAt === 'number' ? Date.now() - startedAt : undefined;
    request.log.info(
      {
        reqId: request.id,
        statusCode: reply.statusCode,
        durationMs,
      },
      'request completed',
    );
  });
});
