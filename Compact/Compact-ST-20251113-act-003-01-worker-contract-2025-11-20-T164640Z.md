# Compact – ST-20251113-act-003-01-worker-contract (refresh 4)

## Implemented facts
- Reiterated authoritative attempt semantics across specs: `fanout_attempts` increments once per MailSender attempt (sent/retryable/failed); no pre-send increment or failure-only counting remains.

## Interfaces / behavior changes
- Workers should rely on the unified attempt-count rule for backoff/dead-letter calculations; previous contradictory guidance removed.

## Risks / TODO / limits
- Docs only; worker code still pending to enforce/back this contract.

## Code Review - ST-20251113-act-003-01 - 2025-11-20T16:48:57Z

---review-start---
Compact/Compact-ST-20251113-act-003-01-worker-contract-2025-11-20-T163622Z.md
Comment on lines +18 to +22


- **Ack (sent)**: when `MailSender.send` (wrapped by `ReliableMailSender`) returns `status="sent"`, set `fanout_status='sent'`, store the final attempt in `error` (`{provider, providerMessageId, status}`), clear `locked_by/locked_at`, and append `subscription_events.notify_sent` with the section status snapshot.
- **Retryable nack**: when `status="retryable"` and `error.code` is retryable, keep `fanout_status='pending'`, increment `fanout_attempts`, update `last_attempt_at`, store the last attempt (with `retryAfterSeconds` when present) in `error`, clear the lock. Outer worker schedules the next pick using `retryScheduleMs[attempt-1]` capped by `lockTtlSeconds`.
- **Terminal nack**: for `status="failed"` (invalid recipient, template missing locale/variable, unsubscribed row, non-email contact), set `fanout_status='skipped'` for non-actionable rows or `fanout_status='failed'` when the channel should alert ops. Record `{code,message}` in `error` and clear the lock.
- **Ack (sent)**: when `MailSender.send` (wrapped by `ReliableMailSender`) returns `status="sent"`, bump `fanout_attempts` by 1, set `fanout_status='sent'`, store the final attempt in `error` (`{provider, providerMessageId, status}`), clear `locked_by/locked_at`, and append `subscription_events.notify_sent` with the section status snapshot.
- **Retryable nack**: when `status="retryable"` and `error.code` is retryable, bump `fanout_attempts` by 1, keep `fanout_status='pending'`, update `last_attempt_at`, store the last attempt (with `retryAfterSeconds` when present) in `error`, clear the lock. Outer worker schedules the next pick using `retryScheduleMs[fanout_attempts-1]` (after increment) capped by `lockTtlSeconds`.


@chatgpt-codex-connector
chatgpt-codex-connector bot
1 minute ago
P2 Badge Resolve duplicated ack/nack semantics

The new Compact refresh still lists two different ack/nack variants back‑to‑back: lines 18‑20 describe handling without bumping fanout_attempts on a successful or terminal send, while lines 21‑22 repeat the same steps but with an explicit increment. Because this document is supposed to reconcile the worker contracts, the conflicting instructions leave it unclear whether workers should increment attempts on a successful delivery (which drives backoff slots and dead‑letter thresholds), risking divergent implementations depending on which bullet the reader follows.
---review-end---
