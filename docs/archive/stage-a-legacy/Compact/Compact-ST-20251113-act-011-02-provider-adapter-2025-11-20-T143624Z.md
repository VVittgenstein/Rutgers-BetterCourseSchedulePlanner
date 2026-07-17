# Compact – ST-20251113-act-011-02-provider-adapter

## Implemented facts
- Added `notifications/mail/types.ts` defining MailSender contract (payload/result/error enums, provider types, template/config shapes) aligned with mail sender contract doc.
- `notifications/mail/config.ts` loads JSON configs with zod validation, resolves env-referenced secrets for SendGrid/SMTP, enforces supportedLocales/template locale coverage, and normalizes defaults for logging/testHooks/timeouts/templateRoot.
- `notifications/mail/template_loader.ts` renders subject/html/text from template files with variable interpolation + HTML escape, auto-fills locale/unsubscribe/manage vars, and surfaces structured TemplateError codes (validation_error/template_missing_locale/template_variable_missing/unknown).
- Implemented SendGrid provider adapter (`notifications/mail/providers/sendgrid.ts`): validates addresses, honors defaultFrom/replyTo and overrideRecipient/dryRun test hooks, maps dedupe/trace/templateVersion/metadata to SendGrid headers/custom_args, builds attachments, handles sandboxMode/categories/ipPool, respects timeouts, and classifies responses (202 sent; 429 rate_limited with retryAfter; 5xx provider_error retryable; 401/403 unauthorized failed; other 4xx provider_error failed; timeouts/network as retryable).
- Added node:test suite (`notifications/mail/tests/provider.test.ts`) with stub SendGrid server covering payload mapping + template rendering, rate limit/5xx classification, unauthorized + timeout handling, template validation failures, and dryRun/overrideRecipient behavior. Test run: `npx tsx --test notifications/mail/tests/provider.test.ts` (pass).
- `tsconfig.json` now includes `notifications/**/*.ts` so types are compiled.
- `api/src/routes/subscriptions.ts` updated zod `z.record` usage to two-argument form (`z.record(z.string(), z.unknown())`) to align with zod v4 expectations.

## Interfaces / behavior changes
- New MailSender TypeScript types + config loader/renderer introduce normalized payload/result semantics for downstream workers; SendGrid adapter available for use. Config must supply provider sections + env vars as enforced by loader.
- Subscription API schema change is internal (zod record signature fix); no external contract change but prevents runtime schema errors with current zod version.

## Risks / TODO / limits
- Only SendGrid adapter implemented; SMTP adapter and integration into polling/notification worker still pending.
- Template files referenced in config examples not yet present; renderer assumes existing files under `templateRoot`.
- Rate limiting/retry orchestration beyond per-send classification not implemented here (relies on callers).

## Validation
- Tests: `npx tsx --test notifications/mail/tests/provider.test.ts` (pass).

## Code Review - ST-20251113-act-011-02-provider-adapter - 2025-11-20T14:48:23Z
Codex Review: Didn't find any major issues. Keep them coming!
