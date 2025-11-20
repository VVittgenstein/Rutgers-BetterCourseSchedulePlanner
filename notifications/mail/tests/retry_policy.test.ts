import assert from 'node:assert/strict';
import test from 'node:test';

import { ReliableMailSender } from '../retry_policy.js';
import type {
  MailMessage,
  MailSender,
  ResolvedMailSenderConfig,
  SendErrorCode,
  SendResult,
} from '../types.js';

test('retries retryable errors until success and captures attempts', async () => {
  const sender = new StubSender([retryable('network_error'), retryable('rate_limited', 0.05), sent()]);
  const reliable = new ReliableMailSender(
    sender,
    baseConfig({
      retryPolicy: {
        maxAttempts: 3,
        backoffMs: [0, 20, 40],
        jitter: 0,
        retryableErrors: ['rate_limited', 'network_error', 'provider_error', 'unknown'],
      },
      rateLimit: { maxPerSecond: 5, burst: 5, bucketWidthSeconds: 1 },
    }),
  );

  const outcome = await reliable.send(baseMessage());
  assert.equal(outcome.finalResult.status, 'sent');
  assert.equal(sender.calls, 3);
  assert.deepEqual(
    outcome.attempts.map((a) => a.result.status),
    ['retryable', 'retryable', 'sent'],
  );
  assert.ok((outcome.attempts[1].nextDelayMs ?? 0) >= 50);
});

test('rate limiter queues requests beyond burst', async () => {
  const sender = new StubSender([sent(), sent(), sent()]);
  const reliable = new ReliableMailSender(
    sender,
    baseConfig({
      rateLimit: { maxPerSecond: 2, burst: 2, bucketWidthSeconds: 1 },
      retryPolicy: { maxAttempts: 1, backoffMs: [0], jitter: 0, retryableErrors: ['rate_limited', 'network_error', 'provider_error', 'unknown'] },
    }),
  );

  const results = await Promise.all([
    reliable.send(baseMessage({ dedupeKey: 'a' })),
    reliable.send(baseMessage({ dedupeKey: 'b' })),
    reliable.send(baseMessage({ dedupeKey: 'c' })),
  ]);

  const waits = results.map((r) => r.attempts[0].waitMs ?? 0);
  assert.ok(Math.max(...waits) >= 400);
});

test('dedupe key collapses concurrent sends', async () => {
  const sender = new StubSender([sent()]);
  const reliable = new ReliableMailSender(sender, baseConfig());

  const [one, two] = await Promise.all([
    reliable.send(baseMessage({ dedupeKey: 'same' })),
    reliable.send(baseMessage({ dedupeKey: 'same' })),
  ]);

  assert.equal(sender.calls, 1);
  assert.equal(one.finalResult.status, 'sent');
  assert.equal(two.finalResult.status, 'sent');
});

function baseMessage(overrides: Partial<MailMessage> = {}): MailMessage {
  return {
    to: { email: 'student@example.edu' },
    locale: 'en-US',
    templateId: 'open-seat',
    variables: {},
    ...overrides,
  };
}

function baseConfig(overrides: Partial<ResolvedMailSenderConfig> = {}): ResolvedMailSenderConfig {
  return {
    provider: 'sendgrid',
    defaultFrom: { email: 'alerts@example.edu' },
    replyTo: undefined,
    supportedLocales: ['en-US'],
    templateRoot: '.',
    templates: {},
    rateLimit: { maxPerSecond: 5, burst: 5, bucketWidthSeconds: 1 },
    retryPolicy: { maxAttempts: 3, backoffMs: [0, 10, 30], jitter: 0, retryableErrors: ['rate_limited', 'network_error', 'provider_error', 'unknown'] },
    timeouts: { connectMs: 1000, sendMs: 1000, idleMs: 1000 },
    providers: { sendgrid: { apiKey: 'sg-key', sandboxMode: true, categories: [], ipPool: null } },
    logging: { redactPII: true, traceHeader: 'X-Trace-Id' },
    testHooks: { dryRun: false, overrideRecipient: null },
    ...overrides,
  };
}

class StubSender implements MailSender {
  public calls = 0;
  constructor(private readonly responses: SendResult[]) {}

  async send(_message: MailMessage): Promise<SendResult> {
    this.calls += 1;
    const next = this.responses.shift();
    if (!next) {
      throw new Error('no stub responses left');
    }
    return { ...next };
  }
}

function retryable(code: SendErrorCode, retryAfterSeconds?: number): SendResult {
  return {
    status: 'retryable',
    provider: 'sendgrid',
    attempt: 1,
    durationMs: 5,
    retryAfterSeconds,
    error: { code, message: code },
  };
}

function sent(): SendResult {
  return {
    status: 'sent',
    provider: 'sendgrid',
    attempt: 1,
    durationMs: 5,
    sentAt: new Date().toISOString(),
  };
}
