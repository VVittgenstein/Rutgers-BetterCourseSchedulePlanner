# Compact – ST-20251113-act-003-01-worker-contract (refresh 5)

## Implemented facts
- Clarified `docs/mail_worker_contract.md`: authoritative rule now explicitly states that any prior failure-only or pre-send `fanout_attempts` semantics (including earlier drafts/compacts) are superseded; attempts increment exactly once per MailSender attempt (sent/retryable/failed).

## Interfaces / behavior changes
- Workers should ignore legacy attempt-count guidance and rely solely on the unified post-attempt increment for backoff/dead-letter logic.

## Risks / TODO / limits
- Documentation-only; worker implementation/enforcement still pending.
