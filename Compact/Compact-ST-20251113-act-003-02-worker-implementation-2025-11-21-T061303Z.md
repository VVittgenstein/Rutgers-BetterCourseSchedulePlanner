- Subtask **ST-20251113-act-003-02-worker-implementation** implemented a SQLite-backed mail dispatcher worker and tests.
- New worker **workers/mail_dispatcher.ts**: pulls `open_event_notifications` with expired/absent locks, locks rows, hydrates with `open_events` + `subscriptions` (+ sections/courses/meetings), validates email-eligibility, builds locale-aware `open-seat` mail payload (manage/unsubscribe links, meeting summary, dedupe/trace metadata), calls `ReliableMailSender` (SendGrid adapter wired), and persists outcomes:
  - on sent → `fanout_status=sent`, `fanout_attempts+=1`, error JSON stores final result/attempts, clears lock, writes `subscription_events.notify_sent`, updates `subscriptions.last_known_section_status/last_notified_at`.
  - on retryable → `fanout_status` stays `pending` unless `maxAttempts` reached (then `failed`); lock time encodes backoff using `retryAfterSeconds` vs local schedule; stores attempts JSON; no subscription event unless terminal.
  - on terminal/skippable (invalid recipient, template issues, non-email contact, inactive status, non-OPEN event) → `fanout_status=skipped|failed`, attempts bumped, `notify_failed` event logged.
- CLI args: `--sqlite`, `--mail-config`, `--batch`, `--worker-id`, `--lock-ttl`, `--max-attempts`, `--app-base-url`, `--default-locale`, `--idle-delay`, `--once`; defaults include WAL/foreign_keys, batch=25, lockTtl=120s, delivery retry schedule [0,2s,7s], baseUrl `http://localhost:3000`.
- Template/locale handling: picks subscription locale if supported else default; required variables populated from stored payload + section/course + meetings; meeting summary string derived from `section_meetings` times/location, falls back to `TBA`.
- Tests (`workers/tests/mail_dispatcher.test.ts` run via `npx tsx --test ...`):
  - sent path asserts mail payload contents (locale, manage/unsubscribe links, meeting summary) and DB transitions to `sent` with `notify_sent` event.
  - retryable path asserts `fanout_status` remains `pending`, attempts=1, lock encodes retry delay, and no subscription event emitted.
- Known limits/risks: only SendGrid provider is wired in `createSender`; lock/backoff encoding relies on `locked_at` math (may need alignment with scheduler); app base URL must be configured correctly for unsubscribe/manage links; meeting summary is coarse and may omit richer metadata; no supervisord/runbook or metrics hooks yet beyond underlying MailSender.

## Code Review - ST-20251113-act-003-02-worker-implementation - 2025-11-21T06:25:48Z
Codex Review: Didn't find any major issues. Hooray!
