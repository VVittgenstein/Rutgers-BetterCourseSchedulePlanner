import fs from 'node:fs/promises';
import path from 'node:path';

import type { FastifyInstance, FastifyReply } from 'fastify';
import { z } from 'zod';

import type { TemplateIssue } from '../../../notifications/mail/template_checker.js';
import { collectTemplateIssues } from '../../../notifications/mail/template_checker.js';
import type { MailSenderConfig, SendGridConfig } from '../../../notifications/mail/types.js';

const emailSchema = z.object({
  email: z.string().trim().email(),
  name: z.string().trim().min(1).optional(),
});

const sendgridInputSchema = z
  .object({
    apiKey: z.string().trim().min(1).optional(),
    apiKeyEnv: z.string().trim().min(1).optional(),
    sandboxMode: z.boolean().optional(),
    categories: z.array(z.string().trim().min(1)).optional(),
    ipPool: z.string().trim().nullable().optional(),
    apiBaseUrl: z.string().trim().optional(),
  });

const testHooksSchema = z.object({
  dryRun: z.boolean().optional(),
  overrideRecipient: z.string().trim().email().nullable().optional(),
});

const updateMailConfigSchema = z.object({
  provider: z.literal('sendgrid'),
  defaultFrom: emailSchema,
  replyTo: emailSchema.optional(),
  sendgrid: sendgridInputSchema,
  testHooks: testHooksSchema.optional(),
});

type MailConfigUpdateBody = z.infer<typeof updateMailConfigSchema>;

type SanitizedSendgridConfig = Omit<SendGridConfig, 'apiKey'> & { apiKeySet: boolean };

type SanitizedMailConfig = Omit<MailSenderConfig, 'providers'> & {
  providers: {
    sendgrid?: SanitizedSendgridConfig;
    smtp?: MailSenderConfig['providers']['smtp'];
  };
};

export async function registerAdminRoutes(app: FastifyInstance) {
  app.get('/admin/mail-config', async (request, reply) => {
    const traceId = String(request.id);
    reply.header('x-trace-id', traceId);

    const paths = getConfigPaths();
    let exampleConfig: MailSenderConfig;
    try {
      exampleConfig = await readConfig(paths.exampleConfigPath);
    } catch (error) {
      request.log.error({ err: error }, 'failed to read mail_sender.example.json');
      return sendError(reply, 500, 'MAIL_CONFIG_EXAMPLE_MISSING', 'mail_sender.example.json is required', traceId);
    }

    const userConfig = await readUserConfig(paths.userConfigPath);
    const config = sanitizeConfig(userConfig ?? exampleConfig);
    const templateIssues = await collectTemplateIssues(userConfig ?? exampleConfig, paths.configDir);
    return {
      config,
      meta: {
        source: userConfig ? 'user' : 'example',
        hasSendgridKey: config.providers.sendgrid?.apiKeySet ?? false,
        path: userConfig ? paths.userConfigPath : paths.exampleConfigPath,
        traceId,
        templateIssues,
      },
    };
  });

  app.put(
    '/admin/mail-config',
    {
      schema: {
        body: updateMailConfigSchema,
      },
    },
    async (request, reply) => {
      const traceId = String(request.id);
      reply.header('x-trace-id', traceId);

      const paths = getConfigPaths();
      let exampleConfig: MailSenderConfig;
      try {
        exampleConfig = await readConfig(paths.exampleConfigPath);
      } catch (error) {
        request.log.error({ err: error }, 'failed to read mail_sender.example.json');
        return sendError(reply, 500, 'MAIL_CONFIG_EXAMPLE_MISSING', 'mail_sender.example.json is required', traceId);
      }

      const existingConfig = await readUserConfig(paths.userConfigPath);
      const body = request.body as MailConfigUpdateBody;
      const mergedConfig = mergeConfig(body, exampleConfig, existingConfig);

      if (!mergedConfig.providers?.sendgrid?.apiKey && !mergedConfig.providers?.sendgrid?.apiKeyEnv) {
        return sendError(reply, 400, 'SENDGRID_KEY_REQUIRED', 'SendGrid apiKey or apiKeyEnv is required', traceId);
      }

      const templateIssues = await collectTemplateIssues(mergedConfig, paths.configDir);
      if (templateIssues.length && mergedConfig.testHooks?.dryRun === false) {
        const summary = summarizeTemplateIssues(templateIssues);
        return sendError(
          reply,
          400,
          'MAIL_TEMPLATES_MISSING',
          `Mail templates missing: ${summary}. Keep dryRun enabled until files are added.`,
          traceId,
          templateIssues.map((issue) => issue.message),
        );
      }

      await fs.mkdir(paths.configDir, { recursive: true });
      await fs.writeFile(paths.userConfigPath, JSON.stringify(mergedConfig, null, 2) + '\n', 'utf8');

      const config = sanitizeConfig(mergedConfig);
      return {
        config,
        meta: {
          source: 'user',
          hasSendgridKey: config.providers.sendgrid?.apiKeySet ?? false,
          path: paths.userConfigPath,
          traceId,
          templateIssues,
        },
      };
    },
  );
}

async function readUserConfig(userConfigPath: string): Promise<MailSenderConfig | null> {
  try {
    return await readConfig(userConfigPath);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') {
      return null;
    }
    throw error;
  }
}

async function readConfig(filePath: string): Promise<MailSenderConfig> {
  const raw = await fs.readFile(filePath, 'utf8');
  return JSON.parse(raw) as MailSenderConfig;
}

function mergeConfig(
  payload: MailConfigUpdateBody,
  exampleConfig: MailSenderConfig,
  existingConfig: MailSenderConfig | null,
): MailSenderConfig {
  const baseConfig = existingConfig ?? exampleConfig;
  const previousSendgrid = baseConfig.providers?.sendgrid;
  const { apiKey: incomingApiKey, apiKeyEnv: incomingApiKeyEnv, ...restSendgrid } = payload.sendgrid;

  const apiKeyEnv = incomingApiKeyEnv ?? previousSendgrid?.apiKeyEnv ?? exampleConfig.providers?.sendgrid?.apiKeyEnv;
  const apiKey = incomingApiKey ?? (incomingApiKeyEnv ? undefined : previousSendgrid?.apiKey);

  const sendgrid: SendGridConfig = {
    ...exampleConfig.providers?.sendgrid,
    ...previousSendgrid,
    ...restSendgrid,
    apiKey,
    apiKeyEnv,
  };

  return {
    provider: payload.provider,
    defaultFrom: payload.defaultFrom,
    replyTo: payload.replyTo ?? baseConfig.replyTo,
    supportedLocales: baseConfig.supportedLocales ?? exampleConfig.supportedLocales,
    templateRoot: baseConfig.templateRoot ?? exampleConfig.templateRoot,
    templates: baseConfig.templates ?? exampleConfig.templates,
    rateLimit: baseConfig.rateLimit ?? exampleConfig.rateLimit,
    retryPolicy: baseConfig.retryPolicy ?? exampleConfig.retryPolicy,
    timeouts: baseConfig.timeouts ?? exampleConfig.timeouts,
    providers: {
      sendgrid,
      smtp: baseConfig.providers?.smtp ?? exampleConfig.providers?.smtp,
    },
    logging: baseConfig.logging ?? exampleConfig.logging,
    testHooks: {
      dryRun: payload.testHooks?.dryRun ?? baseConfig.testHooks?.dryRun ?? exampleConfig.testHooks?.dryRun ?? false,
      overrideRecipient:
        payload.testHooks?.overrideRecipient ??
        baseConfig.testHooks?.overrideRecipient ??
        exampleConfig.testHooks?.overrideRecipient ??
        null,
    },
  };
}

function sanitizeConfig(config: MailSenderConfig): SanitizedMailConfig {
  const providers = config.providers ?? {};
  const sendgrid = providers.sendgrid;
  const { apiKey: _apiKey, ...restSendgrid } = sendgrid ?? {};
  const apiKeySet = Boolean(sendgrid?.apiKey || sendgrid?.apiKeyEnv);

  return {
    ...config,
    providers: {
      ...providers,
      sendgrid: sendgrid ? { ...restSendgrid, apiKeySet } : undefined,
    },
  };
}

function getConfigPaths() {
  const configDir = process.env.MAIL_CONFIG_DIR ? path.resolve(process.env.MAIL_CONFIG_DIR) : path.resolve('configs');
  return {
    configDir,
    userConfigPath: path.join(configDir, 'mail_sender.user.json'),
    exampleConfigPath: path.join(configDir, 'mail_sender.example.json'),
  };
}

function sendError(
  reply: FastifyReply,
  statusCode: number,
  code: string,
  message: string,
  traceId: string,
  details?: string[],
) {
  return reply.status(statusCode).send({
    error: {
      code,
      message,
      traceId,
      details,
    },
  });
}

function summarizeTemplateIssues(issues: TemplateIssue[]): string {
  if (issues.length === 0) return '';
  const head = issues.slice(0, 2).map((issue) => issue.path);
  const remaining = issues.length - head.length;
  return `${head.join(', ')}${remaining > 0 ? ` (+${remaining} more)` : ''}`;
}
