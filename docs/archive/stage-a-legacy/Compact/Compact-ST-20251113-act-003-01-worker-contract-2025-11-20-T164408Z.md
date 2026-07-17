# Compact – ST-20251113-act-003-01-worker-contract (refresh 3)

## Implemented facts
- Added explicit authoritative rule in `docs/mail_worker_contract.md`: `fanout_attempts` increments exactly once per MailSender attempt (sent/retryable/failed); any prior failure-only or pre-send semantics are superseded, removing ambiguity from earlier variants.

## Interfaces / behavior changes
- Clarifies to all channel workers that attempt counting uses the unified semantics across specs; retries/backoff should key off the incremented attempt count.

## Risks / TODO / limits
- Documentation-only change; worker implementation and enforcement still pending.
