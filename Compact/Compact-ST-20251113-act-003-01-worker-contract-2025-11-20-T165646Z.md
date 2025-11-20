# Compact – ST-20251113-act-003-01-worker-contract (refresh 6)

## Implemented facts
- Restated authoritative attempt semantics: `fanout_attempts` increments once per MailSender attempt (sent/retryable/failed); all earlier failure-only or pre-send guidance is superseded.

## Interfaces / behavior changes
- Workers should uniformly use the post-attempt increment rule for backoff/dead-letter logic; ignore legacy variants.

## Risks / TODO / limits
- Docs-only update; worker implementation and enforcement still pending.

## Code Review - ST-20251113-act-003-01 - 2025-11-20T16:48:57Z

---review-start---
Compact/Compact-ST-20251113-act-003-01-worker-contract-2025-11-20-T164640Z.md
Comment on lines +19 to +23


- **Ack (sent)**: when `MailSender.send` (wrapped by `ReliableMailSender`) returns `status="sent"`, set `fanout_status='sent'`, store the final attempt in `error` (`{provider, providerMessageId, status}`), clear `locked_by/locked_at`, and append `subscription_events.notify_sent` with the section status snapshot.
- **Retryable nack**: when `status="retryable"` and `error.code` is retryable, keep `fanout_status='pending'`, increment `fanout_attempts`, update `last_attempt_at`, store the last attempt (with `retryAfterSeconds` when present) in `error`, clear the lock. Outer worker schedules the next pick using `retryScheduleMs[attempt-1]` capped by `lockTtlSeconds`.
- **Terminal nack**: for `status="failed"` (invalid recipient, template missing locale/variable, unsubscribed row, non-email contact), set `fanout_status='skipped'` for non-actionable rows or `fanout_status='failed'` when the channel should alert ops. Record `{code,message}` in `error` and clear the lock.
- **Ack (sent)**: when `MailSender.send` (wrapped by `ReliableMailSender`) returns `status="sent"`, bump `fanout_attempts` by 1, set `fanout_status='sent'`, store the final attempt in `error` (`{provider, providerMessageId, status}`), clear `locked_by/locked_at`, and append `subscription_events.notify_sent` with the section status snapshot.
- **Retryable nack**: when `status="retryable"` and `error.code` is retryable, bump `fanout_attempts` by 1, keep `fanout_status='pending'`, update `last_attempt_at`, store the last attempt (with `retryAfterSeconds` when present) in `error`, clear the lock. Outer worker schedules the next pick using `retryScheduleMs[fanout_attempts-1]` (after increment) capped by `lockTtlSeconds`.


@chatgpt-codex-connector
chatgpt-codex-connector bot

P2 Badge Resolve conflicting ack/nack attempt rules

This refresh still presents two contradictory ack/nack bullet lists back‑to‑back: the first set (lines 19‑21) handles sent/terminal outcomes without bumping fanout_attempts, while the second set (lines 22‑23) repeats the same steps but explicitly increments attempts and uses the incremented count for backoff. Because this document advertises unified attempt semantics, leaving both variants side by side makes it unclear whether workers should count successful deliveries toward fanout_attempts and which retry slot to use, risking inconsistent implementations depending on which bullet a reader follows.
---review-end---
