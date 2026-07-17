import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { createServer } from '../src/server.js';

const EXAMPLE_CONFIG = {
  provider: 'sendgrid' as const,
  defaultFrom: { email: 'alerts@example.edu', name: 'Course Alerts' },
  replyTo: { email: 'support@example.edu', name: 'Alerts Support' },
  supportedLocales: ['en-US'],
  templateRoot: 'templates/email',
  templates: {
    verification: {
      html: { 'en-US': 'verification/en-US.html' },
      text: { 'en-US': 'verification/en-US.txt' },
      requiredVariables: ['verificationUrl'],
    },
  },
  rateLimit: { maxPerSecond: 3, burst: 6, bucketWidthSeconds: 60 },
  retryPolicy: { maxAttempts: 2, backoffMs: [0, 2000], jitter: 0.1, retryableErrors: ['network_error'] },
  timeouts: { connectMs: 3000, sendMs: 8000, idleMs: 60000 },
  providers: {
    sendgrid: {
      apiKeyEnv: 'SENDGRID_API_KEY',
      sandboxMode: true,
      categories: ['course-alerts'],
      ipPool: null,
    },
  },
  logging: { redactPII: true, traceHeader: 'X-Trace-Id' },
  testHooks: { dryRun: true, overrideRecipient: null },
};

test('GET /api/admin/mail-config returns sanitized config using example defaults', async (t) => {
  const fixture = createTempMailConfig();
  const restoreEnv = overrideEnv('MAIL_CONFIG_DIR', fixture.configDir);
  const restoreSqlite = overrideEnv('SQLITE_FILE', path.join(fixture.root, 'db.sqlite'));
  const server = await createServer();
  t.after(async () => {
    await server.close();
    restoreEnv();
    restoreSqlite();
    fixture.cleanup();
  });

  const response = await server.inject({ method: 'GET', url: '/api/admin/mail-config' });
  assert.equal(response.statusCode, 200);
  const payload = response.json() as any;
  assert.equal(payload.meta.source, 'example');
  assert.equal(payload.config.provider, 'sendgrid');
  assert.deepEqual(payload.config.supportedLocales, EXAMPLE_CONFIG.supportedLocales);
  assert.ok(payload.config.providers.sendgrid);
  assert.equal(payload.config.providers.sendgrid.apiKeySet, true);
  assert.equal(Object.prototype.hasOwnProperty.call(payload.config.providers.sendgrid, 'apiKey'), false);
  assert.equal(payload.meta.templateIssues.length, 0);
});

test('PUT /api/admin/mail-config writes user file and redacts apiKey in response', async (t) => {
  const fixture = createTempMailConfig();
  const restoreEnv = overrideEnv('MAIL_CONFIG_DIR', fixture.configDir);
  const restoreSqlite = overrideEnv('SQLITE_FILE', path.join(fixture.root, 'db.sqlite'));
  const server = await createServer();
  t.after(async () => {
    await server.close();
    restoreEnv();
    restoreSqlite();
    fixture.cleanup();
  });

  const payload = {
    provider: 'sendgrid' as const,
    defaultFrom: { email: 'alerts@demo.test', name: 'Demo Alerts' },
    replyTo: { email: 'help@demo.test', name: 'Help Desk' },
    sendgrid: { apiKey: 'SG.fake-key', sandboxMode: false, categories: ['demo'], ipPool: null },
    testHooks: { dryRun: false },
  };

  const putResponse = await server.inject({
    method: 'PUT',
    url: '/api/admin/mail-config',
    payload,
  });
  assert.equal(putResponse.statusCode, 200);
  const putPayload = putResponse.json() as any;
  assert.equal(putPayload.meta.source, 'user');
  assert.equal(putPayload.config.defaultFrom.email, payload.defaultFrom.email);
  assert.equal(putPayload.config.testHooks.dryRun, false);
  assert.equal(Object.prototype.hasOwnProperty.call(putPayload.config.providers.sendgrid, 'apiKey'), false);
  assert.equal(putPayload.config.providers.sendgrid.apiKeySet, true);
  assert.deepEqual(putPayload.config.supportedLocales, EXAMPLE_CONFIG.supportedLocales);
  assert.equal(putPayload.meta.templateIssues.length, 0);

  const savedConfig = readJson(path.join(fixture.configDir, 'mail_sender.user.json'));
  assert.equal(savedConfig.defaultFrom.email, payload.defaultFrom.email);
  assert.equal(savedConfig.providers.sendgrid.apiKey, payload.sendgrid.apiKey);
  assert.deepEqual(savedConfig.templates, EXAMPLE_CONFIG.templates);
  assert.deepEqual(savedConfig.rateLimit, EXAMPLE_CONFIG.rateLimit);
});

test('PUT /api/admin/mail-config rejects when templates are missing and dryRun=false', async (t) => {
  const fixture = createTempMailConfig();
  const restoreEnv = overrideEnv('MAIL_CONFIG_DIR', fixture.configDir);
  const restoreSqlite = overrideEnv('SQLITE_FILE', path.join(fixture.root, 'db.sqlite'));
  const server = await createServer();
  t.after(async () => {
    await server.close();
    restoreEnv();
    restoreSqlite();
    fixture.cleanup();
  });

  const missingHtml = path.join(fixture.configDir, 'templates', 'email', 'open-seat', 'en-US.html');
  fs.rmSync(missingHtml);

  const payload = {
    provider: 'sendgrid' as const,
    defaultFrom: { email: 'alerts@demo.test', name: 'Demo Alerts' },
    replyTo: { email: 'help@demo.test', name: 'Help Desk' },
    sendgrid: { apiKey: 'SG.fake-key', sandboxMode: false, categories: ['demo'], ipPool: null },
    testHooks: { dryRun: false },
  };

  const putResponse = await server.inject({
    method: 'PUT',
    url: '/api/admin/mail-config',
    payload,
  });

  assert.equal(putResponse.statusCode, 400);
  const body = putResponse.json() as any;
  assert.equal(body.error.code, 'MAIL_TEMPLATES_MISSING');
  assert.match(body.error.message, /open-seat/);
});

function createTempMailConfig() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'admin-mail-config-'));
  const configDir = path.join(root, 'configs');
  fs.mkdirSync(configDir, { recursive: true });
  fs.writeFileSync(path.join(configDir, 'mail_sender.example.json'), JSON.stringify(EXAMPLE_CONFIG, null, 2));
  writeTemplateFixtures(configDir);

  return {
    root,
    configDir,
    cleanup: () => fs.rmSync(root, { recursive: true, force: true }),
  };
}

function writeTemplateFixtures(configDir: string) {
  const base = path.join(configDir, 'templates', 'email');
  fs.mkdirSync(path.join(base, 'open-seat'), { recursive: true });
  fs.mkdirSync(path.join(base, 'verification'), { recursive: true });
  fs.writeFileSync(path.join(base, 'open-seat', 'en-US.html'), '<p>open seat</p>');
  fs.writeFileSync(path.join(base, 'open-seat', 'en-US.txt'), 'open seat');
  fs.writeFileSync(path.join(base, 'verification', 'en-US.html'), '<p>verify</p>');
  fs.writeFileSync(path.join(base, 'verification', 'en-US.txt'), 'verify');
}

function overrideEnv(key: string, value: string) {
  const previous = process.env[key];
  process.env[key] = value;
  return () => {
    if (previous === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = previous;
    }
  };
}

function readJson(filePath: string) {
  const raw = fs.readFileSync(filePath, 'utf8');
  return JSON.parse(raw) as any;
}
