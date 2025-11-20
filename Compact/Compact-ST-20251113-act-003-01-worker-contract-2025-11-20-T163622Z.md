# Compact – ST-20251113-act-003-01-worker-contract (refresh 2)

## Implemented facts
- Reconciled specs: `docs/open_event_spec.md` now matches `mail_worker_contract` semantics—`fanout_attempts` increments once per MailSender attempt (success/retryable/terminal), locks cleared after each, and retries/backoff driven by worker contract; no pre-send increment.

## Interfaces / behavior changes
- Channel workers must treat `fanout_attempts` as “outer queue attempt count” aligned across specs; retry scheduling/backoff should use the post-attempt count per mail worker contract.

## Risks / TODO / limits
- Documentation-only; no enforcement in code and channel worker implementation still pending.

## Code Review - ST-20251113-act-003-01 - 2025-11-20T16:36:52Z

---review-start---
docs/mail_worker_contract.md
Comment on lines 148 to +151
## Ack/Nack and retry semantics
- **Ack (sent)**: when `MailSender.send` (wrapped by `ReliableMailSender`) returns `status="sent"`, set `fanout_status='sent'`, store the final attempt in `error` (`{provider, providerMessageId, status}`), clear `locked_by/locked_at`, and append `subscription_events.notify_sent` with the section status snapshot.
- **Retryable nack**: when `status="retryable"` and `error.code` is retryable, keep `fanout_status='pending'`, increment `fanout_attempts`, update `last_attempt_at`, store the last attempt (with `retryAfterSeconds` when present) in `error`, clear the lock. Outer worker schedules the next pick using `retryScheduleMs[attempt-1]` capped by `lockTtlSeconds`.
- **Terminal nack**: for `status="failed"` (invalid recipient, template missing locale/variable, unsubscribed row, non-email contact), set `fanout_status='skipped'` for non-actionable rows or `fanout_status='failed'` when the channel should alert ops. Record `{code,message}` in `error` and clear the lock.
- **Ack (sent)**: when `MailSender.send` (wrapped by `ReliableMailSender`) returns `status="sent"`, bump `fanout_attempts` by 1, set `fanout_status='sent'`, store the final attempt in `error` (`{provider, providerMessageId, status}`), clear `locked_by/locked_at`, and append `subscription_events.notify_sent` with the section status snapshot.
- **Retryable nack**: when `status="retryable"` and `error.code` is retryable, bump `fanout_attempts` by 1, keep `fanout_status='pending'`, update `last_attempt_at`, store the last attempt (with `retryAfterSeconds` when present) in `error`, clear the lock. Outer worker schedules the next pick using `retryScheduleMs[fanout_attempts-1]` (after increment) capped by `lockTtlSeconds`.
- **Terminal nack**: for `status="failed"` (invalid recipient, template missing locale/variable, unsubscribed row, non-email contact), bump `fanout_attempts` by 1, set `fanout_status='skipped'` for non-actionable rows or `fanout_status='failed'` when the channel should alert ops. Record `{code,message}` in `error` and clear the lock.
@chatgpt-codex-connector
chatgpt-codex-connector bot
6 minutes ago
P2 Badge Align fanout_attempts semantics across specs

These lines now state that fanout_attempts is incremented only after a MailSender attempt completes and is bumped even on a successful send, with retries scheduled using retryScheduleMs[fanout_attempts-1]. docs/open_event_spec.md (lines 49‑52) still documents the prior behavior (increment before send and only on failure), leaving the two contracts contradictory about when attempts are counted and which backoff slot applies. This inconsistency makes it unclear how many tries a worker should record before dead-lettering and how to compute the next delay; please reconcile the two specs or note which one is authoritative.
---review-end---
