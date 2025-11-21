import assert from 'node:assert/strict';
import test from 'node:test';

import { DiscordBot, resolveDiscordBotConfig, type DiscordSendRequest } from '../bot.js';

const baseConfig = {
  applicationId: '123456',
  botTokenEnv: 'DISCORD_BOT_TOKEN',
  defaultMode: 'channel' as const,
  defaultChannelId: 'chan-default',
  dm: { enabled: true, requireGuildIds: [], fallbackChannelId: 'chan-fallback' },
  mentions: { roleId: null, allowEveryone: false },
  rateLimit: { globalPerSecond: 10, perChannelBurst: 2, perChannelResetMs: 5000, maxAttempts: 3, backoffMs: [0, 1000, 2000], jitter: 0 },
  messageTemplate: {},
  logging: { redactSnowflakes: false, traceField: 'traceId' },
  testHooks: {},
};

function createBot(overrides: Partial<typeof baseConfig> = {}, fetchImpl?: typeof fetch, sleepSpy?: (ms: number) => Promise<void>) {
  process.env.DISCORD_BOT_TOKEN = 'token-abc';
  const resolved = resolveDiscordBotConfig({ ...baseConfig, ...overrides }, process.env);
  return new DiscordBot(resolved, {
    fetch: fetchImpl,
    now: () => new Date('2025-01-01T00:00:00Z'),
    sleep: sleepSpy,
  });
}

test('sends message to default channel', async () => {
  const calls: Array<{ url: string; body: string }> = [];
  const fetchStub: typeof fetch = async (url, init) => {
    calls.push({ url: String(url), body: String(init?.body) });
    return new Response(JSON.stringify({ id: 'msg-123' }), { status: 200, headers: { 'Content-Type': 'application/json' } });
  };

  const bot = createBot({}, fetchStub);
  const request: DiscordSendRequest = { target: { channelId: undefined }, message: { content: 'hello world' }, traceId: 'trace-1' };
  const result = await bot.send(request);

  assert.equal(result.finalResult.status, 'sent');
  assert.equal(result.finalResult.messageId, 'msg-123');
  assert.equal(calls.length, 1);
  assert.ok(calls[0].url.endsWith('/channels/chan-default/messages'));
  const parsed = JSON.parse(calls[0].body);
  assert.equal(parsed.content, 'hello world');
});

test('retries when rate limited and succeeds', async () => {
  const calls: string[] = [];
  const sleepCalls: number[] = [];
  const fetchStub: typeof fetch = async (url, init) => {
    calls.push(String(url));
    if (calls.length === 1) {
      return new Response(JSON.stringify({ message: 'rate limited', retry_after: 1 }), {
        status: 429,
        headers: { 'Retry-After': '1' },
      });
    }
    return new Response(JSON.stringify({ id: 'msg-200' }), { status: 200, headers: { 'Content-Type': 'application/json' } });
  };
  const bot = createBot({}, fetchStub, async (ms) => {
    sleepCalls.push(ms);
  });

  const result = await bot.send({ target: { channelId: 'chan-default' }, message: { content: 'rate limit test' } });

  assert.equal(result.finalResult.status, 'sent');
  assert.equal(result.attempts.length, 2);
  assert.equal(result.attempts[0].result.status, 'retryable');
  assert.deepEqual(sleepCalls, [1000]);
  assert.ok(calls.every((url) => url.endsWith('/channels/chan-default/messages')));
});

test('opens DM channel when userId provided', async () => {
  const calls: string[] = [];
  const fetchStub: typeof fetch = async (url, init) => {
    calls.push(String(url));
    if (String(url).endsWith('/users/@me/channels')) {
      return new Response(JSON.stringify({ id: 'dm-777' }), { status: 200, headers: { 'Content-Type': 'application/json' } });
    }
    return new Response(JSON.stringify({ id: 'msg-dm' }), { status: 200, headers: { 'Content-Type': 'application/json' } });
  };

  const bot = createBot({}, fetchStub);
  const result = await bot.send({ target: { userId: 'user-9' }, message: { content: 'private hello' }, traceId: 'trace-dm' });

  assert.equal(result.finalResult.status, 'sent');
  assert.equal(result.finalResult.channelId, 'dm-777');
  assert.equal(calls.length, 2);
  assert.ok(calls[0].endsWith('/users/@me/channels'));
  assert.ok(calls[1].endsWith('/channels/dm-777/messages'));
});

test('dedupeKey prevents concurrent duplicate sends', async () => {
  let callCount = 0;
  const fetchStub: typeof fetch = async (url, init) => {
    callCount += 1;
    return new Response(JSON.stringify({ id: 'msg-dup' }), { status: 200, headers: { 'Content-Type': 'application/json' } });
  };
  const bot = createBot({}, fetchStub);
  const request: DiscordSendRequest = { target: { channelId: 'chan-default' }, message: { content: 'hello' }, dedupeKey: 'evt-1' };

  const [r1, r2] = await Promise.all([bot.send(request), bot.send(request)]);

  assert.equal(callCount, 1);
  assert.equal(r1.finalResult.messageId, 'msg-dup');
  assert.equal(r2.finalResult.messageId, 'msg-dup');
  assert.equal(r1.finalResult.status, 'sent');
  assert.equal(r2.finalResult.status, 'sent');
});
