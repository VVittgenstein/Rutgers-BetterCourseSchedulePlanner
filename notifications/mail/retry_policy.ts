import {
  type MailMessage,
  type MailSender,
  type MailSendAttempt,
  type ProviderType,
  type RateLimitConfig,
  type ResolvedMailSenderConfig,
  type RetryPolicyConfig,
  type SendOptions,
  type SendResult,
  type SendWithRetryResult,
} from './types.js';

const DEFAULT_RATE_LIMIT_KEY = 'mail';

type ScheduledTaskResult<T> = { result: T; waitMs: number; startedAt: number };

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

export class TokenBucketRateLimiter {
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

export class ReliableMailSender {
  private readonly limiter: TokenBucketRateLimiter;
  private readonly inflight = new Map<string, Promise<SendWithRetryResult>>();

  constructor(private readonly sender: MailSender, private readonly config: ResolvedMailSenderConfig) {
    this.limiter = new TokenBucketRateLimiter(config.rateLimit);
  }

  async send(message: MailMessage, options: SendOptions = {}): Promise<SendWithRetryResult> {
    const key = message.dedupeKey;
    if (key) {
      const existing = this.inflight.get(key);
      if (existing) return existing;
    }

    const promise = this.execute(message, options).finally(() => {
      if (key) {
        this.inflight.delete(key);
      }
    });

    if (key) {
      this.inflight.set(key, promise);
    }

    return promise;
  }

  private async execute(message: MailMessage, options: SendOptions): Promise<SendWithRetryResult> {
    const attempts: MailSendAttempt[] = [];
    const policy = this.config.retryPolicy;
    const rateLimitKey = options.rateLimitKey ?? DEFAULT_RATE_LIMIT_KEY;
    let finalResult: SendResult | undefined;

    for (let attempt = 1; attempt <= policy.maxAttempts; attempt++) {
      const scheduled = await this.limiter.schedule(rateLimitKey, async () => {
        try {
          return await this.sender.send(message, options);
        } catch (error) {
          return createThrownResult(error, this.config.provider);
        }
      });

      const finishedAt = Date.now();
      const normalizedResult: SendResult = { ...scheduled.result, attempt };

      const attemptLog: MailSendAttempt = {
        attempt,
        startedAt: new Date(scheduled.startedAt).toISOString(),
        finishedAt: new Date(finishedAt).toISOString(),
        durationMs: normalizedResult.durationMs,
        waitMs: scheduled.waitMs,
        result: normalizedResult,
      };
      attempts.push(attemptLog);

      if (normalizedResult.status === 'sent') {
        finalResult = normalizedResult;
        break;
      }

      if (!shouldRetry(normalizedResult, policy, attempt)) {
        finalResult = normalizedResult;
        break;
      }

      const delayMs = computeDelayMs(policy, attempt, normalizedResult);
      attemptLog.nextDelayMs = delayMs;
      if (delayMs > 0) {
        await sleep(delayMs);
      }
    }

    if (!finalResult && attempts.length) {
      finalResult = attempts[attempts.length - 1].result;
    }

    return { finalResult: finalResult ?? createUnknownFailure(this.config.provider), attempts };
  }
}

function shouldRetry(result: SendResult, policy: RetryPolicyConfig, attempt: number): boolean {
  if (attempt >= policy.maxAttempts) return false;
  if (result.status !== 'retryable') return false;
  const code = result.error?.code ?? 'unknown';
  return policy.retryableErrors.includes(code);
}

function computeDelayMs(policy: RetryPolicyConfig, attempt: number, result: SendResult): number {
  const index = Math.min(attempt - 1, policy.backoffMs.length - 1);
  const base = policy.backoffMs[index] ?? 0;
  const retryAfterMs = result.retryAfterSeconds ? Math.round(result.retryAfterSeconds * 1000) : 0;
  const delay = Math.max(base, retryAfterMs);
  const jitter = policy.jitter > 0 && delay > 0 ? Math.round(delay * policy.jitter * Math.random()) : 0;
  return delay + jitter;
}

function sleep(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function createUnknownFailure(provider: ProviderType): SendResult {
  return {
    status: 'failed',
    provider,
    attempt: 1,
    durationMs: 0,
    error: { code: 'unknown', message: 'mail send failed' },
  };
}

function createThrownResult(error: unknown, provider: ProviderType): SendResult {
  const message = error instanceof Error ? error.message : 'mail send failed';
  return {
    status: 'retryable',
    provider,
    attempt: 1,
    durationMs: 0,
    error: { code: 'unknown', message },
  };
}
