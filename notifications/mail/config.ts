import fs from 'node:fs/promises';
import path from 'node:path';

import { z } from 'zod';

import type {
  MailSenderConfig,
  ResolvedMailSenderConfig,
  ResolvedSMTPConfig,
  ResolvedSendGridConfig,
  RetryPolicyConfig,
  RateLimitConfig,
  SMTPConfig,
  SendGridConfig,
  TemplateDefinition,
} from './types.js';

const emailSchema = z.object({
  email: z.string().email(),
  name: z.string().min(1).optional(),
});

const templateSchema = z.object({
  subject: z.record(z.string(), z.string()).optional(),
  html: z.record(z.string(), z.string()),
  text: z.record(z.string(), z.string()).optional(),
  requiredVariables: z.array(z.string()),
});

const timeoutsSchema = z
  .object({
    connectMs: z.number().int().positive().default(4000),
    sendMs: z.number().int().positive().default(10000),
    idleMs: z.number().int().positive().default(60000),
  })
  .default({
    connectMs: 4000,
    sendMs: 10000,
    idleMs: 60000,
  });

const smtpSchema = z.object({
  host: z.string().min(1),
  port: z.number().int().positive(),
  secure: z.boolean(),
  username: z.string().optional(),
  passwordEnv: z.string().optional(),
  poolSize: z.number().int().positive().optional(),
  tls: z.object({ rejectUnauthorized: z.boolean().optional() }).optional(),
});

const sendgridSchema = z
  .object({
    apiKey: z.string().trim().min(1).optional(),
    apiKeyEnv: z.string().trim().min(1).optional(),
    sandboxMode: z.boolean().default(false),
    categories: z.array(z.string()).default([]),
    ipPool: z.string().nullable().optional(),
    apiBaseUrl: z.string().optional(),
  })
  .refine((value) => Boolean(value.apiKey || value.apiKeyEnv), {
    message: 'SendGrid config requires either apiKey or apiKeyEnv',
  });

const rateLimitSchema = z
  .object({
    maxPerSecond: z.number().positive().default(5),
    burst: z.number().positive().default(10),
    bucketWidthSeconds: z.number().positive().default(60),
  })
  .transform<RateLimitConfig>((value) => {
    const burst = Math.max(value.burst, value.maxPerSecond);
    return { ...value, burst };
  });

const retryPolicySchema = z
  .object({
    maxAttempts: z.number().int().min(1).default(3),
    backoffMs: z.array(z.number().int().nonnegative()).default([0, 2000, 7000]),
    jitter: z.number().min(0).max(1).default(0.25),
    retryableErrors: z
      .array(
        z.enum([
          'validation_error',
          'template_missing_locale',
          'template_variable_missing',
          'invalid_recipient',
          'rate_limited',
          'unauthorized',
          'network_error',
          'provider_error',
          'unknown',
        ]),
      )
      .default(['rate_limited', 'network_error', 'provider_error', 'unknown']),
  })
  .transform<RetryPolicyConfig>((value) => ({
    ...value,
    jitter: Number.isFinite(value.jitter) ? value.jitter : 0,
  }));

const configSchema = z.object({
  provider: z.enum(['sendgrid', 'smtp']),
  defaultFrom: emailSchema,
  replyTo: emailSchema.optional(),
  supportedLocales: z.array(z.string()).min(1),
  templateRoot: z.string(),
  templates: z.record(z.string(), templateSchema),
  timeouts: timeoutsSchema,
  rateLimit: rateLimitSchema.default({
    maxPerSecond: 5,
    burst: 10,
    bucketWidthSeconds: 60,
  }),
  retryPolicy: retryPolicySchema.default({
    maxAttempts: 3,
    backoffMs: [0, 2000, 7000],
    jitter: 0.25,
    retryableErrors: ['rate_limited', 'network_error', 'provider_error', 'unknown'],
  }),
  providers: z.object({
    sendgrid: sendgridSchema.optional(),
    smtp: smtpSchema.optional(),
  }),
  logging: z
    .object({
      redactPII: z.boolean().optional(),
      traceHeader: z.string().optional(),
    })
    .default({}),
  testHooks: z
    .object({
      dryRun: z.boolean().optional(),
      overrideRecipient: z.string().email().nullable().optional(),
    })
    .default({}),
});

export async function loadMailSenderConfig(configPath: string, env: NodeJS.ProcessEnv = process.env): Promise<ResolvedMailSenderConfig> {
  const absolutePath = path.resolve(configPath);
  const raw = await fs.readFile(absolutePath, 'utf8');
  const parsedJson = JSON.parse(raw) as unknown;
  return resolveMailSenderConfig(parsedJson, env, path.dirname(absolutePath));
}

export function resolveMailSenderConfig(
  rawConfig: unknown,
  env: NodeJS.ProcessEnv = process.env,
  baseDir: string = process.cwd(),
): ResolvedMailSenderConfig {
  const parsed = configSchema.parse(rawConfig);
  const logging = {
    redactPII: parsed.logging.redactPII ?? true,
    traceHeader: parsed.logging.traceHeader ?? 'X-Trace-Id',
  };
  const testHooks = {
    dryRun: parsed.testHooks.dryRun ?? false,
    overrideRecipient: parsed.testHooks.overrideRecipient ?? null,
  };

  validateTemplateLocales(parsed.templates, parsed.supportedLocales);
  validateDefaultProvider(parsed.provider, parsed.providers);

  const resolvedSendgrid = resolveSendgrid(parsed.providers.sendgrid, env);
  const resolvedSmtp = resolveSmtp(parsed.providers.smtp, env, {
    requirePassword: parsed.provider === 'smtp',
  });

  const resolved: ResolvedMailSenderConfig = {
    provider: parsed.provider,
    defaultFrom: parsed.defaultFrom,
    replyTo: parsed.replyTo,
    supportedLocales: parsed.supportedLocales,
    templateRoot: path.isAbsolute(parsed.templateRoot) ? parsed.templateRoot : path.resolve(baseDir, parsed.templateRoot),
    templates: parsed.templates,
    rateLimit: parsed.rateLimit,
    retryPolicy: parsed.retryPolicy,
    timeouts: parsed.timeouts,
    providers: {
      sendgrid: resolvedSendgrid,
      smtp: resolvedSmtp,
    },
    logging,
    testHooks,
  };

  return resolved;
}

function resolveSendgrid(config: SendGridConfig | undefined, env: NodeJS.ProcessEnv): ResolvedSendGridConfig | undefined {
  if (!config) return undefined;
  const { apiKey, apiKeyEnv, ...rest } = config;
  if (apiKey) {
    return { ...rest, apiKey, apiKeyEnv };
  }

  if (!apiKeyEnv) {
    throw new Error('Missing SendGrid API key: provide providers.sendgrid.apiKey or apiKeyEnv');
  }

  const envApiKey = env[apiKeyEnv];
  if (!envApiKey) {
    throw new Error(`Missing SendGrid API key env variable: ${apiKeyEnv}`);
  }

  return {
    ...rest,
    apiKey: envApiKey,
    apiKeyEnv,
  };
}

function resolveSmtp(
  config: SMTPConfig | undefined,
  env: NodeJS.ProcessEnv,
  options: { requirePassword?: boolean } = {},
): ResolvedSMTPConfig | undefined {
  if (!config) return undefined;
  const { requirePassword = false } = options;
  const { passwordEnv, ...rest } = config;
  const password = passwordEnv ? env[passwordEnv] : undefined;

  if (passwordEnv && rest.username && !password && requirePassword) {
    throw new Error(`Missing SMTP password env variable: ${passwordEnv}`);
  }

  return {
    ...rest,
    password,
  };
}

function validateDefaultProvider(provider: MailSenderConfig['provider'], providers: MailSenderConfig['providers']) {
  if (provider === 'sendgrid' && !providers.sendgrid) {
    throw new Error('SendGrid is configured as default provider but providers.sendgrid is missing');
  }
  if (provider === 'smtp' && !providers.smtp) {
    throw new Error('SMTP is configured as default provider but providers.smtp is missing');
  }
}

function validateTemplateLocales(templates: Record<string, TemplateDefinition>, supportedLocales: string[]) {
  const unsupportedLocales: string[] = [];
  const missingLocales: string[] = [];

  for (const [templateId, tpl] of Object.entries(templates)) {
    for (const locale of Object.keys(tpl.html)) {
      if (!supportedLocales.includes(locale)) {
        unsupportedLocales.push(`${templateId}:${locale}`);
      }
    }
    for (const locale of supportedLocales) {
      if (!tpl.html[locale]) {
        missingLocales.push(`${templateId}:${locale}`);
      }
      if (tpl.text && !(locale in tpl.text)) {
        missingLocales.push(`${templateId}:${locale}:text`);
      }
      if (tpl.subject && !(locale in tpl.subject)) {
        missingLocales.push(`${templateId}:${locale}:subject`);
      }
    }
  }

  if (unsupportedLocales.length) {
    throw new Error(`Template locales not listed in supportedLocales: ${unsupportedLocales.join(', ')}`);
  }

  if (missingLocales.length) {
    throw new Error(`Templates missing required locales: ${missingLocales.join(', ')}`);
  }
}
