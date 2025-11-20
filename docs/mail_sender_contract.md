# Mail Sender Contract

Defines the abstraction that fan-out workers use to deliver email notifications for course opening events. The goal is to swap providers (SendGrid API or SMTP) without code changes, enforce consistent payloads/templates, and return structured results that make retries deterministic.

## Scope and responsibilities
- Provide a single `MailSender` interface that hides provider details and normalizes success vs retryable vs permanent failures.
- Normalize message payloads: locale-aware templates, safely interpolated variables (course name, index, subscription info), unsubscribe/manage links, and trace metadata.
- Honor deployment-level config for rate limits, retries, timeouts, and template roots so operators can tune without rebuilds.

## Interfaces (TypeScript)
```ts
export interface MailSender {
  send(message: MailMessage, options?: SendOptions): Promise<SendResult>;
  verify?(options?: VerifyOptions): Promise<VerifyResult>; // optional health check
}

export interface MailMessage {
  to: { email: string; name?: string };
  from?: { email: string; name?: string }; // defaultFrom is taken from config when omitted
  replyTo?: { email: string; name?: string };
  locale: string; // e.g., "en-US", "zh-CN"; validated against supportedLocales
  templateId: string; // logical name such as "open-seat" or "verification"
  templateVersion?: string; // optional for backwards-compatible template evolution
  subject?: string; // optional when template supplies localized subject
  variables: Record<string, string | number | boolean | null>; // used for template interpolation
  textBody?: string; // optional override for SMTP/plain flows
  htmlBody?: string; // optional override when template pre-rendered upstream
  unsubscribeUrl?: string;
  manageUrl?: string;
  attachments?: Array<{ filename: string; mimeType: string; contentBase64: string }>;
  dedupeKey?: string; // matches open_event_notifications.dedupe_key when present
  traceId?: string; // cascades to provider headers for observability
  metadata?: { subscriptionId?: number; openEventId?: number; term?: string; campus?: string };
}

export interface SendOptions {
  timeoutMs?: number; // defaults to config.timeouts.sendMs
  providerOverride?: "sendgrid" | "smtp";
  rateLimitKey?: string; // optional bucket key; defaults to "mail"
  dryRun?: boolean; // skip real send but return success with fake providerMessageId
}

export type SendStatus = "sent" | "retryable" | "failed";
export type SendErrorCode =
  | "validation_error"
  | "template_missing_locale"
  | "template_variable_missing"
  | "invalid_recipient"
  | "rate_limited"
  | "unauthorized"
  | "network_error"
  | "provider_error"
  | "unknown";

export interface SendResult {
  status: SendStatus;
  provider: "sendgrid" | "smtp";
  providerMessageId?: string;
  attempt: number;
  durationMs: number;
  sentAt?: string; // ISO timestamp
  retryAfterSeconds?: number; // honored when status === "retryable"
  error?: { code: SendErrorCode; message: string; detail?: unknown };
}
```

Interface semantics:
- `send` resolves with `status="sent"` on provider ack; recoverable errors surface as `status="retryable"` with `retryAfterSeconds` populated; permanent issues (bad recipient, missing template locale) use `status="failed"` and `error.code` so the worker can mark the notification as skipped.
- Implementations should avoid throwing except for programmer errors; operational conditions are expressed through `SendResult`.
- `verify` is optional and may perform a lightweight auth/ping for readiness probes; it inherits timeout settings from config when the caller does not supply `VerifyOptions`.

### Payload validation rules
- `locale` must exist in `config.supportedLocales`; otherwise return `validation_error`.
- `templateId` must map to a template entry and include the requested locale; missing locale files return `template_missing_locale`.
- `variables` must satisfy the template-required keys (see tests below); missing keys return `template_variable_missing`.
- Recipients are lowercased and trimmed; invalid email format returns `invalid_recipient`.
- `dedupeKey` and `traceId` pass through to provider headers when supported (e.g., SendGrid `X-SMTPAPI`, SMTP `X-Trace-Id`).

## Configuration model
Operators provide deployment settings via `configs/mail_sender.example.json` (to be copied/templated per environment). Fields:

| Field | Notes |
| --- | --- |
| `provider` | Default provider: `"sendgrid"` or `"smtp"`. Worker may override per-message. |
| `defaultFrom` | `{ email, name }` used when `MailMessage.from` is absent. |
| `replyTo` | Optional `{ email, name }` applied globally unless overridden. |
| `supportedLocales` | Array of locales the template loader must enforce. |
| `templateRoot` | Base directory for templates (HTML/text/subject files per locale). |
| `templates` | Map of template ids → `{ subject?: Record<locale,string>, html: Record<locale,string>, text?: Record<locale,string>, requiredVariables: string[] }`. |
| `rateLimit` | `{ maxPerSecond, burst, bucketWidthSeconds }` shared across providers. |
| `retryPolicy` | `{ maxAttempts, backoffMs, jitter, retryableErrors }`, where `retryableErrors` maps `SendErrorCode` to retry behavior. |
| `timeouts` | `{ connectMs, sendMs, idleMs }` enforced by adapters. |
| `providers.sendgrid` | `{ apiKeyEnv, sandboxMode, categories, ipPool? }`. `apiKeyEnv` points to an environment variable holding the key. |
| `providers.smtp` | `{ host, port, secure, username, passwordEnv, poolSize, tls?: { rejectUnauthorized } }`. Secrets referenced via `passwordEnv`. |
| `logging` | `{ redactPII, traceHeader }` controls log redaction and trace header name. |
| `testHooks` | `{ dryRun, overrideRecipient }` used by CLI/test scripts to avoid accidental sends. |

See `configs/mail_sender.example.json` for a concrete sample with both providers configured.

## Security and operational requirements
- **Secret handling**: Never store API keys or SMTP passwords in git. Config files reference environment variable names (`apiKeyEnv`, `passwordEnv`); loaders must read from `process.env` and reject startup if missing.
- **Transport security**: SMTP defaults to STARTTLS (`secure=false` + `port=587`) with optional enforcement via `tls.rejectUnauthorized=true`. SendGrid uses HTTPS by default.
- **Timeouts and connection pooling**: Enforce `timeouts.connectMs/sendMs` per attempt; SMTP adapters should cap `poolSize` and evict idle connections at `idleMs` to avoid hanging sockets.
- **PII hygiene**: Logs must redact full email addresses; include only hashed or truncated values plus `traceId`. Template rendering should escape variables to prevent HTML injection.
- **Compliance**: Always attach `unsubscribeUrl` and `manageUrl` when provided; default From/Reply-To must align with verified domains (SPF/DKIM).

## Validation and tests
- **Config validation**: Loaders validate that the selected provider exists, required env vars are present, and `supportedLocales` cover every template locale entry. Fail fast on startup.
- **Template completeness**: A unit test asserts that each template id provides `html` (and optional `text`) for all `supportedLocales` and that `requiredVariables` are present in fixtures; missing locales/keys surface as `template_missing_locale` or `template_variable_missing`.
- **Provider adapters**: Contract tests stub SendGrid/SMTP endpoints and assert mapping of `MailMessage` → provider payload, including headers (`traceId`, `dedupeKey`) and retry classification (429/5xx as `retryable`, 4xx as `failed`).
- **Integration smoke**: A CLI/script can run in `dryRun` or `overrideRecipient` mode to validate connectivity without hitting real recipients; it must honor `testHooks` and timeouts.
- **Observability**: Adapters emit structured logs `{ traceId, provider, status, attempt, durationMs, error }` and expose counters for sent/retry/fail to feed dashboards.

With this contract, downstream workers can render locale-aware templates, choose providers dynamically, and recover gracefully from transient provider issues without changing business logic.
