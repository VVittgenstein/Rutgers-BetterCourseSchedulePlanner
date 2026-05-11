# Compact – ST-20251113-act-003-01-worker-contract (refresh 8)

## Implemented facts
- Authoritative rule reaffirmed: `fanout_attempts` increments once per MailSender attempt (sent/retryable/failed); all failure-only or pre-send variants are superseded. Legacy review blocks kept only for audit.

## Interfaces / behavior changes
- Workers should follow the post-attempt increment rule for backoff/dead-letter calculations; ignore earlier conflicting bullets.

## Risks / TODO / limits
- Docs-only update; worker implementation/enforcement still pending.

## Code Review - ST-20251113-act-003-01 - 2025-11-20T17:12:21Z

---review-start---
Codex Review: Didn't find any major issues. 🎉
---review-end---
