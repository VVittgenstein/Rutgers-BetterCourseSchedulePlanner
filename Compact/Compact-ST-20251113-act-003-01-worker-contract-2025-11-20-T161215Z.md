# Compact – ST-20251113-act-003-01-worker-contract (refresh)

## Implemented facts
- `docs/mail_worker_contract.md` refined: `fanout_attempts` increments exactly once per completed MailSender attempt (sent/retryable/failed), not during lock acquisition, preventing double-count.
- Retry scheduling now uses `retryScheduleMs[fanout_attempts-1]` after the attempt bump, keeping the intended `[0, 2000, 7000]` cadence with `maxAttempts` limits.
- Ack/nack bullets updated accordingly while keeping prior contract elements (queue fields, locking with `locked_by/locked_at`, dedupe via `dedupe_key`, payload schema, versioning) intact.

## Interfaces / behavior changes
- Workers must apply post-attempt increment semantics when persisting results and compute the next backoff based on the incremented attempt count to avoid premature dead-lettering.

## Risks / TODO / limits
- Documentation-only change; no code enforces the clarified attempt handling or retry index.
- Worker implementation still pending; defaults (lock TTL, retry schedule) remain unenforced.

## Code Review - ST-20251113-act-003-01 - 2025-11-20T16:31:23Z

---review-start---
docs/mail_worker_contract.md
Comment on lines +148 to +150


## Ack/Nack and retry semantics
- **Ack (sent)**: when `MailSender.send` (wrapped by `ReliableMailSender`) returns `status="sent"`, set `fanout_status='sent'`, store the final attempt in `error` (`{provider, providerMessageId, status}`), clear `locked_by/locked_at`, and append `subscription_events.notify_sent` with the section status snapshot.
- **Retryable nack**: when `status="retryable"` and `error.code` is retryable, keep `fanout_status='pending'`, increment `fanout_attempts`, update `last_attempt_at`, store the last attempt (with `retryAfterSeconds` when present) in `error`, clear the lock. Outer worker schedules the next pick using `retryScheduleMs[attempt-1]` capped by `lockTtlSeconds`.


@chatgpt-codex-connector
chatgpt-codex-connector bot

P1 Badge Avoid double-counting fanout attempts

The contract tells workers to bump fanout_attempts before dispatching each job (line 8) and then increment it again on retryable nacks (line 150), which causes every failed delivery to count two attempts. With the documented defaults (maxAttempts=3, retryScheduleMs=[0,2000,7000]), the first failure records attempts=2 and schedules the 2 s backoff instead of the initial 0 ms, and the second failure pushes attempts to 4, triggering dead-letter after only two real sends. Workers following this spec will exhaust the queue early and skip the intended retry cadence.
---review-end---
