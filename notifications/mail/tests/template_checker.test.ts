import assert from 'node:assert/strict';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { collectTemplateIssues } from '../template_checker.js';
import type { MailSenderConfig } from '../types.js';

const BASE_CONFIG: MailSenderConfig = {
  provider: 'sendgrid',
  defaultFrom: { email: 'alerts@example.edu', name: 'Course Alerts' },
  supportedLocales: ['en-US'],
  templateRoot: 'templates/email',
  templates: {
    'open-seat': {
      html: { 'en-US': 'open-seat/en-US.html' },
      text: { 'en-US': 'open-seat/en-US.txt' },
      requiredVariables: ['courseString'],
    },
  },
  rateLimit: { maxPerSecond: 5, burst: 10, bucketWidthSeconds: 60 },
  retryPolicy: { maxAttempts: 1, backoffMs: [0], jitter: 0, retryableErrors: ['unknown'] },
  timeouts: { connectMs: 1000, sendMs: 1000, idleMs: 1000 },
  providers: { sendgrid: { apiKey: 'SG.fake' } },
  logging: { redactPII: true, traceHeader: 'X-Trace-Id' },
  testHooks: { dryRun: true, overrideRecipient: null },
};

test('collectTemplateIssues returns no issues when files exist', async (t) => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'template-checker-ok-'));
  t.after(() => fs.rm(root, { recursive: true, force: true }));

  const configDir = path.join(root, 'configs');
  const templateRoot = path.join(configDir, 'templates', 'email', 'open-seat');
  await fs.mkdir(templateRoot, { recursive: true });
  await fs.writeFile(path.join(templateRoot, 'en-US.html'), '<p>ok</p>', 'utf8');
  await fs.writeFile(path.join(templateRoot, 'en-US.txt'), 'ok', 'utf8');

  const issues = await collectTemplateIssues(BASE_CONFIG, configDir);
  assert.equal(issues.length, 0);
});

test('collectTemplateIssues reports missing files with resolved paths', async (t) => {
  const root = await fs.mkdtemp(path.join(os.tmpdir(), 'template-checker-missing-'));
  t.after(() => fs.rm(root, { recursive: true, force: true }));

  const configDir = path.join(root, 'configs');
  const templateRoot = path.join(configDir, 'templates', 'email', 'open-seat');
  await fs.mkdir(templateRoot, { recursive: true });
  await fs.writeFile(path.join(templateRoot, 'en-US.html'), '<p>ok</p>', 'utf8');

  const issues = await collectTemplateIssues(BASE_CONFIG, configDir);
  assert.equal(issues.length, 1);
  assert.equal(issues[0]?.templateId, 'open-seat');
  assert.equal(issues[0]?.locale, 'en-US');
  assert.equal(issues[0]?.kind, 'text');
  assert.ok(issues[0]?.path.endsWith(path.join('open-seat', 'en-US.txt')));
});
