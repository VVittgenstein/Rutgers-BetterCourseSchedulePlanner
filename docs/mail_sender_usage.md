# Mail sender usage, retry, and rate limits

This runbook shows how to wrap provider adapters with the reliability layer added in `notifications/mail/retry_policy.ts`, configure backoff/limits, and read the attempt logs surfaced to workers.

## Quick start (code)
```ts
import { loadMailSenderConfig } from '../notifications/mail/config.js';
import { SendGridMailSender } from '../notifications/mail/providers/sendgrid.js';
import { ReliableMailSender } from '../notifications/mail/retry_policy.js';

const cfg = await loadMailSenderConfig('configs/mail_sender.example.json');
const sender = new ReliableMailSender(new SendGridMailSender(cfg), cfg);

const outcome = await sender.send(
  {
    to: { email: 'student@example.edu' },
    locale: 'en-US',
    templateId: 'open-seat',
    variables: { courseTitle: 'Calculus I', courseString: 'MATH-101', manageUrl: 'https://...' },
    dedupeKey: 'open-2025:0900',
    traceId: 'trace-mail-1',
  },
  { rateLimitKey: 'mail-open-seat' },
);

console.log(outcome.finalResult.status);        // sent | retryable | failed
console.log(outcome.attempts[0].waitMs);        // time spent in rate-limit queue
console.log(outcome.attempts[0].nextDelayMs);   // backoff applied before the next retry
```

## Behavior
- **Token-bucket rate limit**: `rateLimit.maxPerSecond` is the refill rate; `burst` is bucket capacity; `bucketWidthSeconds` caps how much can accumulate after long idle periods. Requests beyond capacity are queued (wait time reported as `attempt.waitMs`).
- **Retry/backoff**: `retryPolicy.maxAttempts` caps total tries. Backoff values come from `retryPolicy.backoffMs` (indexed per attempt) and are maxed against provider `retryAfterSeconds`; `retryPolicy.jitter` adds random delay to avoid thundering herds. Only `SendResult.status === "retryable"` with `error.code` in `retryPolicy.retryableErrors` are retried.
- **Idempotency**: calls sharing `message.dedupeKey` reuse the same in-flight promise to avoid duplicate sends while a retry loop is running.
- **Attempt log**: every `ReliableMailSender.send` returns `{ finalResult, attempts[] }` where each attempt includes timestamps, queue wait, provider result, and the scheduled delay before the next retry.

## Recommended config (see `configs/mail_sender.example.json`)
- Rate limit: `maxPerSecond: 5`, `burst: 10`, `bucketWidthSeconds: 60` (safe for free SendGrid/SMTP tiers; tighten if provider quota is lower).
- Retry policy: `maxAttempts: 3`, `backoffMs: [0, 2000, 7000]`, `jitter: 0.25`, `retryableErrors: ["rate_limited", "network_error", "provider_error", "unknown"]`.
- Keep `logging.redactPII=true` and set `testHooks.dryRun/overrideRecipient` when exercising staging environments.

## Troubleshooting
- **Hit rate limits**: lower `maxPerSecond` or `burst`, assign distinct `rateLimitKey` buckets for noisy workflows, and honor `retryAfterSeconds` surfaced in attempts.
- **Repeated `unknown` errors**: verify API keys/password env vars, template locales/variables, and connectivity; non-retryable errors will exit early with `status="failed"`.
- **Growing queue**: inspect `attempt.waitMs` to estimate delay per message; increase capacity or spread sends across providers if the queue dominates delivery latency.
