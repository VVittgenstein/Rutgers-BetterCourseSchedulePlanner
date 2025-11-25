import Fastify, { type FastifyError } from 'fastify';
import { ZodError } from 'zod';
import { ZodTypeProvider, validatorCompiler, serializerCompiler } from 'fastify-type-provider-zod';
import { fileURLToPath } from 'node:url';

import { loadConfig } from './config.js';
import { buildContainer } from './container.js';
import { requestLoggingPlugin } from './plugins/requestLogging.js';
import { registerHealthRoutes } from './routes/health.js';
import { registerCourseRoutes } from './routes/courses.js';
import { registerSectionRoutes } from './routes/sections.js';
import { registerFilterRoutes } from './routes/filters.js';
import { registerSubscriptionRoutes } from './routes/subscriptions.js';
import { registerFetchRoutes } from './routes/fetch.js';
import { registerAdminRoutes } from './routes/admin.js';
import { registerLocalNotificationRoutes } from './routes/notifications.local.js';

export async function createServer() {
  const config = loadConfig();
  const container = buildContainer({ config });

  const app = Fastify({
    logger: {
      level: config.logLevel,
    },
  })
    .withTypeProvider<ZodTypeProvider>();

  app.setValidatorCompiler(validatorCompiler);
  app.setSerializerCompiler(serializerCompiler);

  app.decorate('container', container);

  app.setErrorHandler((error: unknown, request, reply) => {
    const traceId = String(request.id);
    if (error instanceof ZodError) {
      const details = error.issues.map((issue) => `${issue.path.join('.') || 'root'}: ${issue.message}`);
      reply.header('x-trace-id', traceId);
      return reply.status(400).send({
        error: {
          code: 'VALIDATION_FAILED',
          message: 'One or more parameters are invalid',
          details,
          traceId,
        },
      });
    }

    const fastifyError = error as FastifyError;
    const statusCode = typeof fastifyError.statusCode === 'number' ? fastifyError.statusCode : 500;
    const normalizedStatus = statusCode >= 400 && statusCode < 600 ? statusCode : 500;
    const message =
      typeof fastifyError.message === 'string' && fastifyError.message.length > 0
        ? fastifyError.message
        : 'Unexpected error';
    request.log.error({ err: fastifyError }, 'unhandled error');
    reply.header('x-trace-id', traceId);
    return reply.status(normalizedStatus).send({
      error: {
        code: normalizedStatus >= 500 ? 'INTERNAL_ERROR' : 'BAD_REQUEST',
        message,
        traceId,
      },
    });
  });

  app.setNotFoundHandler((request, reply) => {
    const traceId = String(request.id);
    reply.header('x-trace-id', traceId);
    return reply.status(404).send({
      error: {
        code: 'NOT_FOUND',
        message: `Route ${request.raw.url ?? request.url} not found`,
        traceId,
      },
    });
  });

  app.addHook('onClose', async () => {
    container.close();
  });

  await app.register(requestLoggingPlugin);
  await app.register(async (router) => {
    await registerHealthRoutes(router);
    await registerCourseRoutes(router);
    await registerSectionRoutes(router);
    await registerFilterRoutes(router);
    await registerSubscriptionRoutes(router);
    await registerLocalNotificationRoutes(router);
    await registerFetchRoutes(router);
    await registerAdminRoutes(router);
  }, { prefix: '/api' });

  return app;
}

async function start() {
  const app = await createServer();
  const { config } = app.container;
  try {
    await app.listen({ port: config.port, host: config.host });
    app.log.info(`Listening on http://${config.host}:${config.port}`);
  } catch (error) {
    app.log.error({ err: error }, 'Failed to start server');
    process.exit(1);
  }
}

const isEntryPoint = process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];

if (isEntryPoint) {
  start();
}
