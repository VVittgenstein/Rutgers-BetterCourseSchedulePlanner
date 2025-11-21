import fs from 'node:fs/promises';
import path from 'node:path';

import { z } from 'zod';

const DISCORD_API_BASE = 'https://discord.com/api/v10';

type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
type Logger = Pick<typeof console, 'info' | 'warn' | 'error'>;

type ScheduledTaskResult<T> = { result: T; waitMs: number; startedAt: number };

type RateLimitConfig = { maxPerSecond: number; burst: number; bucketWidthSeconds: number };

type QueueItem<T> = {
  task: () => Promise<T>;
  enqueuedAt: number;
  resolve: (value: ScheduledTaskResult<T>) => void;
  reject: (error: unknown) => void;
};

class RateLimitBucket {
  private readonly capacity: number;
  private readonly ratePerMs: number;
  private tokens: number;
  private lastRefill: number;
  private queue: QueueItem<unknown>[] = [];
  private timer?: NodeJS.Timeout;

  constructor(private readonly config: RateLimitConfig) {
    this.capacity = Math.max(1, Math.min(config.burst, config.maxPerSecond * config.bucketWidthSeconds));
    this.ratePerMs = config.maxPerSecond / 1000;
    this.tokens = this.capacity;
    this.lastRefill = Date.now();
  }

  enqueue<T>(task: () => Promise<T>): Promise<ScheduledTaskResult<T>> {
    const enqueuedAt = Date.now();
    return new Promise<ScheduledTaskResult<T>>((resolve, reject) => {
      this.queue.push({ task, enqueuedAt, resolve, reject });
      this.drain();
    });
  }

  private drain() {
    this.refill();
    if (!this.queue.length) return;

    while (this.tokens >= 1 && this.queue.length) {
      this.tokens -= 1;
      const current = this.queue.shift()!;
      const startedAt = Date.now();
      current
        .task()
        .then((result) => current.resolve({ result, waitMs: startedAt - current.enqueuedAt, startedAt }))
        .catch((error) => current.reject(error));
    }

    if (this.queue.length && !this.timer) {
      const missing = 1 - this.tokens;
      const waitMs = Math.max(1, Math.ceil(missing / this.ratePerMs));
      this.timer = setTimeout(() => {
        this.timer = undefined;
        this.drain();
      }, waitMs);
      if (typeof this.timer.unref === 'function') {
        this.timer.unref();
      }
    }
  }

  private refill() {
    const now = Date.now();
    const elapsedMs = now - this.lastRefill;
    if (elapsedMs <= 0) return;
    this.tokens = Math.min(this.capacity, this.tokens + elapsedMs * this.ratePerMs);
    this.lastRefill = now;
  }
}

class TokenBucketRateLimiter {
  private readonly buckets = new Map<string, RateLimitBucket>();

  constructor(private readonly config: RateLimitConfig) {}

  async schedule<T>(key: string, task: () => Promise<T>): Promise<ScheduledTaskResult<T>> {
    const bucket = this.buckets.get(key) ?? this.createBucket(key);
    return bucket.enqueue(task);
  }

  private createBucket(key: string) {
    const bucket = new RateLimitBucket(this.config);
    this.buckets.set(key, bucket);
    return bucket;
  }
}

const rateLimitSchema = z.object({
  globalPerSecond: z.number().positive().default(20),
  perChannelBurst: z.number().positive().default(5),
  perChannelResetMs: z.number().positive().default(5000),
  maxAttempts: z.number().int().min(1).default(3),
  backoffMs: z.array(z.number().int().nonnegative()).default([0, 2000, 7000]),
  jitter: z.number().min(0).max(1).default(0.25),
});

const configSchema = z.object({
  applicationId: z.string().min(1),
  botTokenEnv: z.string().min(1),
  publicKeyEnv: z.string().optional(),
  defaultMode: z.enum(['channel', 'dm']).default('channel'),
  defaultGuildId: z.string().optional(),
  defaultChannelId: z.string().optional(),
  inviteUrl: z.string().url().optional(),
  dm: z
    .object({
      enabled: z.boolean().default(true),
      requireGuildIds: z.array(z.string()).default([]),
      verificationCommand: z.string().optional(),
      verificationTimeoutSeconds: z.number().int().positive().optional(),
      fallbackChannelId: z.string().optional(),
    })
    .default({ enabled: false, requireGuildIds: [] }),
  mentions: z
    .object({
      roleId: z.string().nullable().optional(),
      allowEveryone: z.boolean().optional(),
    })
    .default({}),
  rateLimit: rateLimitSchema,
  messageTemplate: z
    .object({
      prefix: z.string().optional(),
      statusLine: z.string().optional(),
      meetingLine: z.string().optional(),
      socUrlTemplate: z.string().optional(),
      manageUrlTemplate: z.string().optional(),
      footer: z.string().optional(),
    })
    .default({}),
  logging: z
    .object({
      redactSnowflakes: z.boolean().default(true),
      traceField: z.string().default('traceId'),
    })
    .default({ redactSnowflakes: true, traceField: 'traceId' }),
  testHooks: z
    .object({
      dryRun: z.boolean().optional(),
      overrideChannelId: z.string().nullable().optional(),
      overrideUserId: z.string().nullable().optional(),
    })
    .default({}),
});

export type DiscordBotConfig = z.infer<typeof configSchema>;
export type ResolvedDiscordBotConfig = Omit<DiscordBotConfig, 'botTokenEnv'> & { botToken: string };

export type DiscordTarget = {
  channelId?: string;
  userId?: string;
  guildId?: string;
};

export type DiscordEmbed = {
  title?: string;
  description?: string;
  url?: string;
  color?: number;
  timestamp?: string;
  fields?: Array<{ name: string; value: string; inline?: boolean }>;
  footer?: { text: string };
  author?: { name: string; url?: string; icon_url?: string };
};

export type AllowedMentions = {
  parse?: Array<'roles' | 'users' | 'everyone'>;
  roles?: string[];
  users?: string[];
  repliedUser?: boolean;
};

export type DiscordMessagePayload = {
  content: string;
  embeds?: DiscordEmbed[];
  allowedMentions?: AllowedMentions;
  tts?: boolean;
};

export type DiscordSendRequest = {
  target: DiscordTarget;
  message: DiscordMessagePayload;
  traceId?: string;
  dedupeKey?: string;
};

export type DiscordSendStatus = 'sent' | 'retryable' | 'failed';
export type DiscordSendErrorCode = 'validation_error' | 'rate_limited' | 'unauthorized' | 'not_found' | 'network_error' | 'unknown';

export type DiscordSendResult = {
  status: DiscordSendStatus;
  attempt: number;
  durationMs: number;
  channelId?: string;
  messageId?: string;
  retryAfterSeconds?: number;
  error?: { code: DiscordSendErrorCode; message: string; detail?: unknown; statusCode?: number };
};

export type DiscordSendAttempt = {
  attempt: number;
  startedAt: string;
  finishedAt: string;
  waitMs?: number;
  nextDelayMs?: number;
  result: DiscordSendResult;
};

export type DiscordSendWithRetryResult = {
  finalResult: DiscordSendResult;
  attempts: DiscordSendAttempt[];
};

export async function loadDiscordBotConfig(configPath: string, env: NodeJS.ProcessEnv = process.env): Promise<ResolvedDiscordBotConfig> {
  const absolutePath = path.resolve(configPath);
  const raw = await fs.readFile(absolutePath, 'utf8');
  const parsedJson = JSON.parse(raw) as unknown;
  return resolveDiscordBotConfig(parsedJson, env);
}

export function resolveDiscordBotConfig(rawConfig: unknown, env: NodeJS.ProcessEnv = process.env): ResolvedDiscordBotConfig {
  const parsed = configSchema.parse(rawConfig);
  const token = env[parsed.botTokenEnv];
  if (!token) {
    throw new Error(`Missing Discord bot token env variable: ${parsed.botTokenEnv}`);
  }
  return {
    ...parsed,
    botToken: token,
    logging: parsed.logging,
    testHooks: {
      dryRun: parsed.testHooks.dryRun ?? false,
      overrideChannelId: parsed.testHooks.overrideChannelId ?? null,
      overrideUserId: parsed.testHooks.overrideUserId ?? null,
    },
  };
}

type AttemptResult = Omit<DiscordSendResult, 'attempt'>;

export class DiscordBot {
  private readonly globalLimiter: TokenBucketRateLimiter;
  private readonly channelLimiter: TokenBucketRateLimiter;
  private readonly cacheDmChannels = new Map<string, string>();
  private readonly fetchImpl: FetchLike;
  private readonly logger: Logger;
  private readonly now: () => Date;
  private readonly sleep: (ms: number) => Promise<void>;

  constructor(
    private readonly config: ResolvedDiscordBotConfig,
    deps: { fetch?: FetchLike; logger?: Logger; now?: () => Date; sleep?: (ms: number) => Promise<void> } = {},
  ) {
    this.fetchImpl = deps.fetch ?? fetch;
    this.logger = deps.logger ?? console;
    this.now = deps.now ?? (() => new Date());
    this.sleep = deps.sleep ?? ((ms: number) => new Promise((resolve) => setTimeout(resolve, ms)));

    this.globalLimiter = new TokenBucketRateLimiter({
      maxPerSecond: config.rateLimit.globalPerSecond,
      burst: Math.max(config.rateLimit.globalPerSecond, config.rateLimit.globalPerSecond * 2),
      bucketWidthSeconds: 1,
    });
    this.channelLimiter = new TokenBucketRateLimiter({
      maxPerSecond: config.rateLimit.perChannelBurst / Math.max(1, config.rateLimit.perChannelResetMs / 1000),
      burst: config.rateLimit.perChannelBurst,
      bucketWidthSeconds: Math.max(1, config.rateLimit.perChannelResetMs / 1000),
    });
  }

  private readonly inflight = new Map<string, Promise<DiscordSendWithRetryResult>>();

  async send(request: DiscordSendRequest): Promise<DiscordSendWithRetryResult> {
    const dedupeKey = request.dedupeKey;
    if (dedupeKey) {
      const existing = this.inflight.get(dedupeKey);
      if (existing) return existing;
    }

    const promise = this.sendWithRetry(request).finally(() => {
      if (dedupeKey) {
        this.inflight.delete(dedupeKey);
      }
    });

    if (dedupeKey) {
      this.inflight.set(dedupeKey, promise);
    }

    return promise;
  }

  private async sendWithRetry(request: DiscordSendRequest): Promise<DiscordSendWithRetryResult> {
    const attempts: DiscordSendAttempt[] = [];
    const effectiveTarget = this.applyTestHooks(request.target);
    const channelResolution = await this.resolveChannel(effectiveTarget, request.traceId);
    const message = this.applyMessageDefaults(request.message);

    let finalResult: DiscordSendResult | undefined;
    for (let attempt = 1; attempt <= this.config.rateLimit.maxAttempts; attempt++) {
      const scheduled = await this.schedule(channelResolution.channelId, () => this.performSend(channelResolution.channelId, message, request.traceId));
      const finishedAt = this.now().getTime();
      const result: DiscordSendResult = { ...scheduled.result, attempt };

      const attemptLog: DiscordSendAttempt = {
        attempt,
        startedAt: new Date(scheduled.startedAt).toISOString(),
        finishedAt: new Date(finishedAt).toISOString(),
        waitMs: scheduled.waitMs,
        result,
      };
      attempts.push(attemptLog);

      if (result.status === 'sent') {
        finalResult = result;
        break;
      }

      if (!this.shouldRetry(result, attempt)) {
        finalResult = result;
        break;
      }

      const delayMs = this.computeDelay(attempt, result);
      attemptLog.nextDelayMs = delayMs;
      this.log('warn', 'Retrying Discord send after backoff', {
        attempt,
        delayMs,
        retryAfterSeconds: result.retryAfterSeconds ?? null,
        channelId: this.redact(channelResolution.channelId),
        traceId: request.traceId,
      });
      if (delayMs > 0) {
        await this.sleep(delayMs);
      }
    }

    if (!finalResult && attempts.length) {
      finalResult = attempts[attempts.length - 1].result;
    }

    return { finalResult: finalResult ?? this.unknownFailure(), attempts };
  }

  private async performSend(channelId: string, message: DiscordMessagePayload, traceId?: string): Promise<AttemptResult> {
    if (this.config.testHooks.dryRun) {
      this.log('info', 'Discord send dry-run', { channelId: this.redact(channelId), traceId });
      return { status: 'sent', durationMs: 0, channelId, messageId: 'dry-run' };
    }

    const startedAt = this.now().getTime();
    try {
      const response = await this.fetchImpl(new URL(`/channels/${channelId}/messages`, DISCORD_API_BASE), {
        method: 'POST',
        headers: {
          Authorization: `Bot ${this.config.botToken}`,
          'Content-Type': 'application/json',
        },
        body: JSON.stringify(this.toDiscordPayload(message)),
      });

      const durationMs = this.now().getTime() - startedAt;
      const body = await this.safeJson(response);

      if (response.status === 429) {
        const retryAfterSeconds = this.extractRetryAfterSeconds(response, body);
        this.log('warn', 'Discord rate limited', {
          channelId: this.redact(channelId),
          retryAfterSeconds,
          traceId,
          status: response.status,
        });
        return {
          status: 'retryable',
          durationMs,
          channelId,
          retryAfterSeconds,
          error: { code: 'rate_limited', message: body?.message ?? 'rate limited', detail: body, statusCode: response.status },
        };
      }

      if (response.ok) {
        this.log('info', 'Discord message sent', {
          channelId: this.redact(channelId),
          messageId: this.redact(body?.id),
          traceId,
          durationMs,
        });
        return { status: 'sent', durationMs, channelId, messageId: body?.id };
      }

      const result = normalizeError(response.status, body);
      this.log('error', 'Discord send failed', {
        channelId: this.redact(channelId),
        traceId,
        status: response.status,
        error: result.error?.message,
      });
      return { ...result, durationMs, channelId };
    } catch (error) {
      const durationMs = this.now().getTime() - startedAt;
      this.log('error', 'Discord send threw', { channelId: this.redact(channelId), traceId, error });
      return {
        status: 'retryable',
        durationMs,
        channelId,
        error: { code: 'network_error', message: error instanceof Error ? error.message : 'network error' },
      };
    }
  }

  private async schedule(channelId: string, task: () => Promise<AttemptResult>): Promise<ScheduledTaskResult<AttemptResult>> {
    const global = await this.globalLimiter.schedule('global', () => this.channelLimiter.schedule(channelId, task));
    const inner = global.result as ScheduledTaskResult<AttemptResult>;
    return {
      result: inner.result,
      waitMs: (global.waitMs ?? 0) + (inner.waitMs ?? 0),
      startedAt: inner.startedAt,
    };
  }

  private async resolveChannel(target: DiscordTarget, traceId?: string): Promise<{ channelId: string }> {
    const overrideChannel = this.config.testHooks.overrideChannelId;
    if (overrideChannel) {
      return { channelId: overrideChannel };
    }

    if (target.channelId) {
      return { channelId: target.channelId };
    }

    if (target.userId && this.config.dm.enabled) {
      try {
        const dmChannel = await this.ensureDmChannel(target.userId);
        return { channelId: dmChannel };
      } catch (error) {
        this.log('warn', 'Failed to open DM channel, attempting fallback', {
          userId: this.redact(target.userId),
          traceId,
          error,
        });
      }
    }

    const fallback = this.config.dm.fallbackChannelId ?? this.config.defaultChannelId;
    if (fallback) {
      return { channelId: fallback };
    }

    throw new Error('No channelId resolved for Discord message');
  }

  private async ensureDmChannel(userId: string): Promise<string> {
    const cached = this.cacheDmChannels.get(userId);
    if (cached) return cached;

    const response = await this.fetchImpl(new URL('/users/@me/channels', DISCORD_API_BASE), {
      method: 'POST',
      headers: {
        Authorization: `Bot ${this.config.botToken}`,
        'Content-Type': 'application/json',
      },
      body: JSON.stringify({ recipient_id: userId }),
    });

    const body = await this.safeJson(response);
    if (!response.ok) {
      throw new Error(body?.message ?? `Failed to create DM channel (status ${response.status})`);
    }

    const channelId = body?.id as string | undefined;
    if (!channelId) {
      throw new Error('DM creation returned no channel id');
    }

    this.cacheDmChannels.set(userId, channelId);
    return channelId;
  }

  private applyTestHooks(target: DiscordTarget): DiscordTarget {
    const channelId = this.config.testHooks.overrideChannelId ?? target.channelId;
    const userId = this.config.testHooks.overrideUserId ?? target.userId;
    return { ...target, channelId, userId };
  }

  private applyMessageDefaults(message: DiscordMessagePayload): DiscordMessagePayload {
    const mentions = this.buildAllowedMentions(message.allowedMentions);
    return { ...message, allowedMentions: mentions };
  }

  private buildAllowedMentions(input?: AllowedMentions): AllowedMentions {
    const allowEveryone = this.config.mentions.allowEveryone ?? false;
    const roleId = this.config.mentions.roleId ?? undefined;
    const allowedParse: Array<'roles' | 'users' | 'everyone'> = [];
    if (allowEveryone) {
      allowedParse.push('everyone');
    }
    const allowed: AllowedMentions = {
      parse: input?.parse ?? allowedParse,
      roles: roleId ? [roleId] : input?.roles,
      users: input?.users,
      repliedUser: input?.repliedUser ?? false,
    };
    return allowed;
  }

  private toDiscordPayload(message: DiscordMessagePayload): Record<string, unknown> {
    const payload: Record<string, unknown> = {
      content: message.content,
      tts: message.tts ?? false,
      embeds: message.embeds,
      allowed_mentions: this.transformAllowedMentions(message.allowedMentions),
    };
    return pruneUndefined(payload);
  }

  private transformAllowedMentions(mentions?: AllowedMentions) {
    if (!mentions) return undefined;
    return pruneUndefined({
      parse: mentions.parse,
      roles: mentions.roles,
      users: mentions.users,
      replied_user: mentions.repliedUser,
    });
  }

  private computeDelay(attempt: number, result: AttemptResult): number {
    const idx = Math.min(attempt - 1, this.config.rateLimit.backoffMs.length - 1);
    const base = this.config.rateLimit.backoffMs[idx] ?? 0;
    const retryAfterMs = result.retryAfterSeconds ? Math.round(result.retryAfterSeconds * 1000) : 0;
    const chosen = Math.max(base, retryAfterMs);
    if (!chosen) return 0;
    const jitter = this.config.rateLimit.jitter > 0 ? Math.round(chosen * this.config.rateLimit.jitter * Math.random()) : 0;
    return chosen + jitter;
  }

  private shouldRetry(result: DiscordSendResult, attempt: number): boolean {
    if (attempt >= this.config.rateLimit.maxAttempts) return false;
    return result.status === 'retryable';
  }

  private extractRetryAfterSeconds(response: Response, body: unknown): number | undefined {
    const headerValue = response.headers.get('Retry-After');
    const retryAfterHeader = headerValue ? Number(headerValue) : undefined;
    if (Number.isFinite(retryAfterHeader)) {
      return retryAfterHeader;
    }
    if (body && typeof body === 'object' && 'retry_after' in body) {
      const value = (body as { retry_after?: number }).retry_after;
      if (typeof value === 'number') return value;
    }
    return undefined;
  }

  private async safeJson(response: Response): Promise<any> {
    try {
      return await response.json();
    } catch {
      return null;
    }
  }

  private log(level: keyof Logger, message: string, meta?: Record<string, unknown>) {
    const payload = meta ? this.redactPayload(meta) : undefined;
    this.logger[level](message, payload);
  }

  private redact(value: string | number | null | undefined): string | null {
    if (!value) return null;
    if (!this.config.logging.redactSnowflakes) return String(value);
    const str = String(value);
    if (str.length <= 4) return '****';
    return `${'*'.repeat(Math.max(0, str.length - 4))}${str.slice(-4)}`;
  }

  private redactPayload(obj: Record<string, unknown>): Record<string, unknown> {
    const output: Record<string, unknown> = {};
    for (const [key, value] of Object.entries(obj)) {
      if (value === null || value === undefined) continue;
      if (typeof value === 'string' && key.toLowerCase().includes('id')) {
        output[key] = this.redact(value);
      } else {
        output[key] = value;
      }
    }
    return output;
  }

  private unknownFailure(): DiscordSendResult {
    return { status: 'failed', attempt: 0, durationMs: 0, error: { code: 'unknown', message: 'No attempts performed' } };
  }
}

function pruneUndefined<T extends Record<string, unknown>>(value: T): T {
  const output: Record<string, unknown> = {};
  for (const [key, val] of Object.entries(value)) {
    if (val === undefined) continue;
    output[key] = val;
  }
  return output as T;
}

function normalizeError(status: number, body: any): AttemptResult {
  if (status === 401 || status === 403) {
    return {
      status: 'failed',
      durationMs: 0,
      error: { code: 'unauthorized', message: body?.message ?? 'unauthorized', detail: body, statusCode: status },
    };
  }
  if (status === 404) {
    return {
      status: 'failed',
      durationMs: 0,
      error: { code: 'not_found', message: body?.message ?? 'channel or user not found', detail: body, statusCode: status },
    };
  }
  if (status >= 500) {
    return {
      status: 'retryable',
      durationMs: 0,
      error: { code: 'network_error', message: body?.message ?? 'server error', detail: body, statusCode: status },
    };
  }
  return {
    status: 'failed',
    durationMs: 0,
    error: { code: 'validation_error', message: body?.message ?? 'bad request', detail: body, statusCode: status },
  };
}
