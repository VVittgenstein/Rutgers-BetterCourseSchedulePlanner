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
