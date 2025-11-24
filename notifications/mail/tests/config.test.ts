import assert from 'node:assert/strict';
import test from 'node:test';

import { resolveMailSenderConfig } from '../config.js';
import type { MailSenderConfig } from '../types.js';

test('prefers inline SendGrid apiKey over environment variable', () => {
  const resolved = resolveMailSenderConfig(
    baseConfig({
      providers: { sendgrid: { apiKey: 'inline-key', apiKeyEnv: 'SENDGRID_KEY' } },
    }),
    { SENDGRID_KEY: 'env-key' },
  );

  assert.equal(resolved.providers.sendgrid?.apiKey, 'inline-key');
  assert.equal(resolved.providers.sendgrid?.apiKeyEnv, 'SENDGRID_KEY');
});

test('falls back to apiKeyEnv when inline key is absent', () => {
  const resolved = resolveMailSenderConfig(
    baseConfig({
      providers: { sendgrid: { apiKeyEnv: 'SENDGRID_KEY' } },
    }),
    { SENDGRID_KEY: 'env-key' },
  );

  assert.equal(resolved.providers.sendgrid?.apiKey, 'env-key');
});

test('throws when apiKeyEnv is set but environment variable is missing', () => {
  assert.throws(
    () =>
      resolveMailSenderConfig(
        baseConfig({
          providers: { sendgrid: { apiKeyEnv: 'SENDGRID_KEY' } },
        }),
        {},
      ),
    /Missing SendGrid API key env variable: SENDGRID_KEY/,
  );
});

test('throws when neither apiKey nor apiKeyEnv is provided', () => {
  assert.throws(
    () =>
      resolveMailSenderConfig(
        baseConfig({
          providers: { sendgrid: {} },
        }),
        {},
      ),
    /SendGrid config requires either apiKey or apiKeyEnv/,
  );
});

function baseConfig(overrides: Partial<MailSenderConfig> = {}): MailSenderConfig {
  const merged: MailSenderConfig = {
    provider: 'sendgrid',
    defaultFrom: { email: 'alerts@example.edu', name: 'Course Alerts' },
    supportedLocales: ['en-US'],
    templateRoot: '.',
    templates: {
      'open-seat': {
        html: { 'en-US': 'open-seat/en-US.html' },
        requiredVariables: ['courseTitle'],
      },
    },
    timeouts: { connectMs: 500, sendMs: 500, idleMs: 500 },
    providers: {
      sendgrid: { sandboxMode: false, categories: [], ipPool: null },
    },
    ...overrides,
    providers: {
      sendgrid: {
        sandboxMode: false,
        categories: [],
        ipPool: null,
        ...(overrides.providers?.sendgrid ?? {}),
      },
      ...overrides.providers,
    },
  };

  return merged;
}
