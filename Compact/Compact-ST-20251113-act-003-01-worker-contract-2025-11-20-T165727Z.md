# Compact – ST-20251113-act-003-01-worker-contract (refresh 7)

## Implemented facts
- Clarified in `docs/mail_worker_contract.md` that the authoritative rule supersedes all legacy/pre-send attempt semantics (including earlier drafts/compacts); `fanout_attempts` increments exactly once per MailSender attempt (sent/retryable/failed).

## Interfaces / behavior changes
- Channel workers must ignore prior conflicting guidance and use the unified post-attempt increment for backoff/dead-letter calculations.

## Risks / TODO / limits
- Documentation update only; worker implementation/enforcement still pending.

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

## Code Review - ST-20251113-act-003-01 - 2025-11-20T16:58:29Z

---review-start---
Compact/Compact-ST-20251113-act-003-01-worker-contract-2025-11-20-T165646Z.md
Comment on lines +10 to +24


P2 Badge Missing review content

The latest compact refresh omits the third review thread that flagged duplicated ack/nack semantics. Without appending that review verbatim, the chain of feedback is incomplete and downstream review tooling cannot track the unresolved issue. Please add the prior review block as done in earlier compacts so the history remains intact.
---review-end---
