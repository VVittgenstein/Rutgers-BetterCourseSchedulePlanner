# Compact – ST-20251113-act-003-01-worker-contract

## Implemented facts
- Added `docs/mail_worker_contract.md` defining the email worker contract over `open_event_notifications` in SQLite: required fields, locking (`locked_by/locked_at`, default TTL 120s), `fanout_status` lifecycle (`pending|sent|skipped|failed|expired`), and idempotency via `(open_event_id, subscription_id)` plus shared `dedupe_key`.
- Queue payload schema (JSON Schema v2020-12) specified for mail jobs built from `open_event_notifications` → `open_events` → `subscriptions` (+`sections/courses` when available); includes `version`, IDs, `locale`, `recipient`, `template` (`id=open-seat`, version, variables for course/index/meeting summary/links), `event` metadata, `subscription` snapshot, `links`, and `deliveryPolicy` defaults.
- Ack/nack semantics documented: `status=sent` → `fanout_status=sent`; `status=retryable` → keep `pending`, increment attempts, store error and honor `retryAfterSeconds`; terminal failures or ineligible subscriptions → `skipped`/`failed`; dead-letter when attempts exceed `deliveryPolicy.maxAttempts` with manual requeue guidance.
- MailSender invocation expectations: worker passes `dedupeKey`/`traceId`, subscription locales, `templateId=open-seat`, unsubscribe/manage URLs, and metadata `{ subscriptionId, openEventId, term, campus }` to `ReliableMailSender`/`MailSender`.
- Versioning rules captured: additive `MAJOR.MINOR` for job payload; `template.version` for copy/layout; error JSON additive-only to stay parseable.
- `record.json` updated: subtask status set to `done`, timestamps refreshed.

## Interfaces / behavior changes
- Establishes canonical email job shape and delivery semantics that future `mail_dispatcher` implementations must follow, including lock/retry/dead-letter policy and provider dedupe usage.
- Consumers must treat non-email `contact_type` as terminal skip and reuse queue `dedupe_key` as `MailMessage.dedupeKey`.

## Risks / TODO / limits
- Docs-only change; no worker implementation or automated validation yet.
- Delivery defaults (attempts, lock TTL, retry schedule) are specified but not enforced in code.

## Validation
- No tests run; documentation update only.
