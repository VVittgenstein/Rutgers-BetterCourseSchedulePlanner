import type { AppContainer } from '../container.js';

declare module 'fastify' {
  interface FastifyInstance {
    container: AppContainer;
  }
}
