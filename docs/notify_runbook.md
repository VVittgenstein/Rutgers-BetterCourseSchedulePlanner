# Notification runbook (mail channel)

## Scope & components
- Covers open-section polling → `open_events` / `open_event_notifications` queue → mail dispatcher invoking `ReliableMailSender`.
- Applies to local/staging deployments using SQLite + SendGrid (sandbox or real) and the cron-style workers in `workers/open_sections_poller.ts` and `workers/mail_dispatcher.ts`.

## SLO / targets
- Closed→Open end-to-end (event observed to email handed to provider): **avg <30s, worst <60s** when poll interval ≤20–25s and mail send RTT <1s (validated in `reports/mail_worker_latency.md`).
- No duplicate emails per `{term,campus,index,status,bucket_5m}` dedupe key; queue drain succeeds with retry budget (`maxAttempts=3` with `0/2/7s` backoff).
- Error budget: ≤1% of notifications end in `fanout_status in ('failed','skipped')` over a 1h window.

## How to run locally
- **Poller (openSections):**
  - `tsx workers/open_sections_poller.ts --term 12025 --campuses NB --interval 20 --checkpoint data/poller_checkpoint.json --metrics-port 9309`
  - `--interval` is in seconds (use `--interval-ms` for ms). Default jitter is 0.3 and baked in; interval ≤20s keeps worst-case mail latency under 60s while jitter avoids thundering herd.
- **Mail dispatcher:**
  - `tsx workers/mail_dispatcher.ts --sqlite data/local.db --mail-config configs/mail_sender.example.json --batch 50 --idle-delay 2000 --lock-ttl 120`
  - For smoke tests, add `--once`; for long-running, keep WAL enabled (default in script) and monitor log output.
- **Smoke test:** `npx tsx scripts/mail_e2e_sim.ts` to verify queue→send path and dedupe behavior without touching production configs.

## Monitoring & alert hints
- **Poller metrics** (when `--metrics-port` set): scrape `/metrics` for `poller_polls_total`, `poller_poll_failures_total`, `poller_events_emitted_total`, `poller_notifications_queued_total`, `poller_last_duration_ms`.
  - Alert if `poller_poll_failures_total` increments for >5 minutes or `poller_last_duration_ms` grows steadily (hinting at upstream slowness).
- **Queue health (SQLite)**:
  - Pending backlog: `SELECT fanout_status, COUNT(*) FROM open_event_notifications GROUP BY 1;`
  - Stuck locks: `SELECT COUNT(*) FROM open_event_notifications WHERE fanout_status='pending' AND locked_at < datetime('now','-120 seconds');`
  - Retry churn: `SELECT COUNT(*) FROM open_event_notifications WHERE fanout_status='pending' AND fanout_attempts>=3;`
- **Duplicates:** `SELECT dedupe_key, COUNT(*) FROM open_events WHERE event_at > datetime('now','-5 minutes') GROUP BY 1 HAVING COUNT(*)>1;` (should be empty).

## Troubleshooting
- **No emails going out:** check `open_event_notifications` for `pending` rows; if empty, verify poller is emitting events and `subscriptions.last_known_section_status` not already `OPEN`.
- **Stuck locks / crashed worker:** clear stale locks with `UPDATE open_event_notifications SET locked_by=NULL, locked_at=NULL WHERE fanout_status='pending' AND locked_at < datetime('now','-120 seconds');` then rerun dispatcher with `--once` to drain.
- **Provider errors:** `fanout_status='pending'` with `error` JSON containing `"retryable"` → let retries proceed; `"invalid_recipient"` / `"template_variable_missing"` are terminal and will be marked `skipped`.
- **Duplicate sends reported:** confirm reopen happened within 5-minute dedupe bucket (expected suppression). If reopen after that window still duplicating, inspect `dedupe_key` hash inputs (term/campus/index/status/bucket) and ensure poller clock is synced.

## Maintenance / config notes
- Keep poll interval aligned with SLO: 15–25s recommended; higher intervals can breach 60s worst-case latency.
- SendGrid sandbox: set `providers.sendgrid.sandboxMode=true` in `configs/mail_sender.example.json` for dry runs.
- WAL files under `data/` are expected during long runs; avoid deleting while workers are active.
