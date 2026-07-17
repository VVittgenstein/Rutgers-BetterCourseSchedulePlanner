import assert from 'node:assert/strict';
import { once } from 'node:events';
import { createServer, type IncomingMessage, type ServerResponse } from 'node:http';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { resolveMailSenderConfig } from '../config.js';
import type { MailMessage, MailSenderConfig } from '../types.js';
import { SendGridMailSender } from '../providers/sendgrid.js';

type RecordedRequest = {
  method?: string;
  path?: string;
  headers: IncomingMessage['headers'];
  body: string;
};

test('SendGrid adapter sends payload with rendered templates', async () => {
  const templates = createTemplateFixtures();
  const mock = await startMockSendgridServer({
    status: 202,
    headers: { 'x-message-id': 'msg-123' },
  });

  try {
    const sender = new SendGridMailSender(
      resolveMailSenderConfig(
        baseConfig(templates.root, {
          providers: {
            sendgrid: {
              apiKeyEnv: 'SENDGRID_KEY',
              sandboxMode: false,
              apiBaseUrl: mock.url,
              categories: ['alerts'],
            },
          },
        }),
        { SENDGRID_KEY: 'sg.test.key' },
      ),
    );

    const message: MailMessage = {
      to: { email: 'student@example.edu', name: 'Student' },
      locale: 'en-US',
      templateId: 'open-seat',
      variables: { courseTitle: 'Calculus I', courseString: 'MATH-101', manageUrl: 'https://example.com/manage' },
      dedupeKey: 'dedupe-123',
      traceId: 'trace-1',
    };

    const result = await sender.send(message);

    assert.equal(result.status, 'sent');
    assert.equal(result.providerMessageId, 'msg-123');
    assert.ok(result.durationMs >= 0);
    assert.ok(result.sentAt);

    const recorded = mock.requests[0];
    assert.equal(recorded.method, 'POST');
    assert.equal(recorded.path, '/v3/mail/send');
    assert.equal(recorded.headers.authorization, 'Bearer sg.test.key');

    const payload = JSON.parse(recorded.body) as Record<string, unknown>;
    const personalization = (payload.personalizations as Array<Record<string, unknown>>)[0];
    const to = (personalization.to as Array<Record<string, string>>)[0];
    const content = payload.content as Array<{ type: string; value: string }>;

    assert.equal(to.email, 'student@example.edu');
    assert.equal((payload.from as { email: string }).email, 'alerts@example.edu');
    assert.equal(personalization.subject, 'Seat opened for Calculus I');
    assert.equal(personalization.headers?.['x-trace-id'], 'trace-1');
    assert.equal(personalization.custom_args?.dedupe_key, 'dedupe-123');
    assert.equal(content.find((c) => c.type === 'text/plain')?.value, 'Hello Calculus I');
    assert.match(content.find((c) => c.type === 'text/html')?.value ?? '', /Calculus I/);
  } finally {
    await mock.close();
    templates.cleanup();
  }
});

test('classifies rate limits and server errors', async () => {
  const templates = createTemplateFixtures();
  const mock = await startMockSendgridServer({
    statusSequence: [429, 500],
    headers: { 'Retry-After': '12' },
  });

  try {
    const sender = new SendGridMailSender(
      resolveMailSenderConfig(
        baseConfig(templates.root, {
          providers: { sendgrid: { apiKeyEnv: 'SENDGRID_KEY', apiBaseUrl: mock.url } },
        }),
        { SENDGRID_KEY: 'sg.test.key' },
      ),
    );

    const message: MailMessage = {
      to: { email: 'student@example.edu' },
      locale: 'en-US',
      templateId: 'open-seat',
      variables: { courseTitle: 'Physics' },
    };

    const rateLimited = await sender.send(message);
    assert.equal(rateLimited.status, 'retryable');
    assert.equal(rateLimited.error?.code, 'rate_limited');
    assert.equal(rateLimited.retryAfterSeconds, 12);

    const serverError = await sender.send(message);
    assert.equal(serverError.status, 'retryable');
    assert.equal(serverError.error?.code, 'provider_error');
  } finally {
    await mock.close();
    templates.cleanup();
  }
});

test('treats unauthorized as failed and aborts on timeout', async () => {
  const templates = createTemplateFixtures();
  const mock = await startMockSendgridServer({
    statusSequence: [401],
  });

  const slowMock = await startMockSendgridServer({
    delayMs: 200,
    status: 202,
  });

  try {
    const sender = new SendGridMailSender(
      resolveMailSenderConfig(
        baseConfig(templates.root, {
          providers: { sendgrid: { apiKeyEnv: 'SENDGRID_KEY', apiBaseUrl: mock.url } },
        }),
        { SENDGRID_KEY: 'sg.test.key' },
      ),
    );

    const message: MailMessage = {
      to: { email: 'student@example.edu' },
      locale: 'en-US',
      templateId: 'open-seat',
      variables: { courseTitle: 'Biology' },
    };

    const unauthorized = await sender.send(message);
    assert.equal(unauthorized.status, 'failed');
    assert.equal(unauthorized.error?.code, 'unauthorized');

    const slowSender = new SendGridMailSender(
      resolveMailSenderConfig(
        baseConfig(templates.root, {
          providers: { sendgrid: { apiKeyEnv: 'SENDGRID_KEY', apiBaseUrl: slowMock.url } },
        }),
        { SENDGRID_KEY: 'sg.test.key' },
      ),
    );
    const timedOut = await slowSender.send(message, { timeoutMs: 50 });
    assert.equal(timedOut.status, 'retryable');
    assert.equal(timedOut.error?.code, 'network_error');
  } finally {
    await mock.close();
    await slowMock.close();
    templates.cleanup();
  }
});

test('fails fast when template variables or locale missing', async () => {
  const templates = createTemplateFixtures();
  const mock = await startMockSendgridServer({ status: 202 });

  try {
    const sender = new SendGridMailSender(
      resolveMailSenderConfig(
        baseConfig(templates.root, {
          providers: { sendgrid: { apiKeyEnv: 'SENDGRID_KEY', apiBaseUrl: mock.url } },
        }),
        { SENDGRID_KEY: 'sg.test.key' },
      ),
    );

    const missingVar: MailMessage = {
      to: { email: 'student@example.edu' },
      locale: 'en-US',
      templateId: 'open-seat',
      variables: {},
    };

    const missingLocale: MailMessage = {
      to: { email: 'student@example.edu' },
      locale: 'de-DE',
      templateId: 'open-seat',
      variables: { courseTitle: 'Chemistry' },
    };

    const resultMissingVar = await sender.send(missingVar);
    assert.equal(resultMissingVar.status, 'failed');
    assert.equal(resultMissingVar.error?.code, 'template_variable_missing');
    assert.equal(mock.requests.length, 0);

    const resultMissingLocale = await sender.send(missingLocale);
    assert.equal(resultMissingLocale.status, 'failed');
    assert.equal(resultMissingLocale.error?.code, 'validation_error');
    assert.equal(mock.requests.length, 0);
  } finally {
    await mock.close();
    templates.cleanup();
  }
});

test('honors override recipient test hook and dryRun option', async () => {
  const templates = createTemplateFixtures();
  const mock = await startMockSendgridServer({ status: 202 });
  try {
    const sender = new SendGridMailSender(
      resolveMailSenderConfig(
        baseConfig(templates.root, {
          providers: { sendgrid: { apiKeyEnv: 'SENDGRID_KEY', apiBaseUrl: mock.url, sandboxMode: false } },
          testHooks: { overrideRecipient: 'override@example.edu', dryRun: false },
        }),
        { SENDGRID_KEY: 'sg.test.key' },
      ),
    );

    const message: MailMessage = {
      to: { email: 'student@example.edu', name: 'Student' },
      locale: 'en-US',
      templateId: 'open-seat',
      variables: { courseTitle: 'Philosophy' },
    };

    const dryRunResult = await sender.send(message, { dryRun: true });
    assert.equal(dryRunResult.status, 'sent');
    assert.equal(dryRunResult.providerMessageId, 'dry-run');
    assert.equal(mock.requests.length, 0);

    const realSend = await sender.send(message);
    assert.equal(realSend.status, 'sent');
    const destEmail = JSON.parse(mock.requests[0].body).personalizations[0].to[0].email;
    assert.equal(destEmail, 'override@example.edu');
  } finally {
    await mock.close();
    templates.cleanup();
  }
});

function baseConfig(templateRoot: string, overrides: Partial<MailSenderConfig> = {}): MailSenderConfig {
  const merged: MailSenderConfig = {
    provider: 'sendgrid',
    defaultFrom: { email: 'alerts@example.edu', name: 'Course Alerts' },
    supportedLocales: ['en-US'],
    templateRoot,
    templates: {
      'open-seat': {
        subject: { 'en-US': 'Seat opened for {{courseTitle}}' },
        html: { 'en-US': 'open-seat/en-US.html' },
        text: { 'en-US': 'open-seat/en-US.txt' },
        requiredVariables: ['courseTitle'],
      },
    },
    timeouts: { connectMs: 500, sendMs: 500, idleMs: 1000 },
    providers: {
      sendgrid: {
        apiKeyEnv: 'SENDGRID_KEY',
        sandboxMode: false,
        categories: [],
        ipPool: null,
      },
    },
    logging: { traceHeader: 'x-trace-id', redactPII: true },
    ...overrides,
    providers: {
      ...{
        sendgrid: {
          apiKeyEnv: 'SENDGRID_KEY',
          sandboxMode: false,
          categories: [],
          ipPool: null,
        },
      },
      ...overrides.providers,
    },
  };
  return merged;
}

function createTemplateFixtures() {
  const root = mkdtempSync(path.join(os.tmpdir(), 'mail-templates-'));
  const openSeatDir = path.join(root, 'open-seat');
  mkdirSync(openSeatDir, { recursive: true });
  writeFileSync(path.join(openSeatDir, 'en-US.html'), '<p>Hello {{courseTitle}}</p>', 'utf8');
  writeFileSync(path.join(openSeatDir, 'en-US.txt'), 'Hello {{courseTitle}}', 'utf8');

  return {
    root,
    cleanup: () => rmSync(root, { recursive: true, force: true }),
  };
}

async function startMockSendgridServer(opts: {
  status?: number;
  statusSequence?: number[];
  headers?: Record<string, string>;
  delayMs?: number;
}) {
  const requests: RecordedRequest[] = [];
  const statusSequence = [...(opts.statusSequence ?? [])];
  const server = createServer(async (req: IncomingMessage, res: ServerResponse) => {
    const bodyChunks: Buffer[] = [];
    req.on('data', (chunk) => bodyChunks.push(chunk as Buffer));
    req.on('end', async () => {
      const body = Buffer.concat(bodyChunks).toString();
      requests.push({ method: req.method, path: req.url ?? '', headers: req.headers, body });
      const status = statusSequence.length ? statusSequence.shift() ?? opts.status ?? 202 : opts.status ?? 202;
      if (opts.delayMs) {
        await new Promise((resolve) => setTimeout(resolve, opts.delayMs));
      }
      res.writeHead(status, opts.headers);
      res.end(status >= 400 ? 'error' : 'ok');
    });
  });

  server.keepAliveTimeout = 1000;
  server.headersTimeout = 2000;

  const listenPromise = once(server, 'listening');
  server.listen(0, '127.0.0.1');
  await listenPromise;
  const address = server.address();
  if (!address || typeof address === 'string') {
    throw new Error('failed to bind mock server');
  }
  const url = `http://127.0.0.1:${address.port}`;
  return {
    url,
    requests,
    close: async () => {
      await new Promise<void>((resolve, reject) => {
        server.close((err) => {
          if (err) reject(err);
          else resolve();
        });
      });
    },
  };
}
