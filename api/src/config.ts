import path from 'node:path';

import { z } from 'zod';

const envSchema = z.object({
  NODE_ENV: z.enum(['development', 'test', 'production']).default('development'),
  APP_PORT: z.coerce.number().int().min(0).max(65535).default(3333),
  APP_HOST: z.string().min(1).default('127.0.0.1'),
  LOG_LEVEL: z.enum(['fatal', 'error', 'warn', 'info', 'debug', 'trace']).default('info'),
  SQLITE_FILE: z.string().min(1).default(path.resolve('data', 'local.db')),
});

export type AppConfig = {
  environment: 'development' | 'test' | 'production';
  port: number;
  host: string;
  logLevel: 'fatal' | 'error' | 'warn' | 'info' | 'debug' | 'trace';
  sqliteFile: string;
};

export function loadConfig(env: NodeJS.ProcessEnv = process.env): AppConfig {
  const parsed = envSchema.parse({
    NODE_ENV: env.NODE_ENV,
    APP_PORT: env.APP_PORT ?? env.PORT,
    APP_HOST: env.APP_HOST ?? env.HOST,
    LOG_LEVEL: env.LOG_LEVEL,
    SQLITE_FILE: env.SQLITE_FILE ?? env.SQLITE_PATH,
  });

  return {
    environment: parsed.NODE_ENV,
    port: parsed.APP_PORT,
    host: parsed.APP_HOST,
    logLevel: parsed.LOG_LEVEL,
    sqliteFile: parsed.SQLITE_FILE,
  };
}
