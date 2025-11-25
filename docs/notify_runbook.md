# Notification runbook (mail channel)

## Scope & components
- Covers open-section polling → `open_events` / `open_event_notifications` queue → mail dispatcher invoking `ReliableMailSender`.
- Applies to local/staging deployments using SQLite + SendGrid (sandbox or real) and the cron-style workers in `workers/open_sections_poller.ts` and `workers/mail_dispatcher.ts`.
- Local sound pull channel uses the same queue: browser POSTs to `/api/notifications/local/claim` with `{ deviceId, limit?<=50 }` to claim `contact_type='local_sound'` rows. Claims flip `fanout_status` to `sent`, bump `fanout_attempts`, and update `subscriptions.last_notified_at`.

## SLO / targets
- Closed→Open end-to-end (event observed to email handed to provider): **avg <30s, worst <60s** when poll interval ≤15–20s and mail send RTT <1s (validated in `reports/mail_worker_latency.md`).
- No duplicate emails per `{term,campus,index,status,bucket_3m}` dedupe key; queue drain succeeds with retry budget (`maxAttempts=3` with `0/2/7s` backoff).
- Error budget: ≤1% of notifications end in `fanout_status in ('failed','skipped')` over a 1h window.

## How to run locally
- **Poller (openSections):**
  - `tsx workers/open_sections_poller.ts --term 12025 --campuses NB --interval 15 --checkpoint data/poller_checkpoint.json --metrics-port 9309`
  - `--interval` is in seconds (use `--interval-ms` for ms). Default jitter is 0.3 and baked in; interval ≤15s keeps worst-case mail latency under 60s while jitter avoids thundering herd.
- **Mail dispatcher:**
  - `tsx workers/mail_dispatcher.ts --sqlite data/local.db --mail-config configs/mail_sender.example.json --batch 50 --idle-delay 2000 --lock-ttl 120`
  - For smoke tests, add `--once`; for long-running, keep WAL enabled (default in script) and monitor log output.
- **Smoke test:** `npx tsx scripts/mail_e2e_sim.ts` to verify queue→send path and dedupe behavior without touching production configs.

## Local sound channel (browser pull)
- **Prereqs:** Keep the subscriptions page open/unmuted; first user click may be required to wake `AudioContext` if autoplay is blocked. Sound + email can coexist; they share the queue but fan out independently per `contact_type`.
- **Enable in UI:** In Subscription Center pick **Sound**, note the device ID (stored locally), then toggle **Enable sound** to start 5–10s polling. If you see “Click to enable sound,” press it once to resume audio.
- **API contract:** `POST /api/notifications/local/claim` with `{"deviceId":"<local-id>","limit":<=50}` → returns `{ notifications[], traceId, meta }` and marks matching rows as `sent` while updating `subscriptions.last_notified_at`.
- **Manual validation:**
  1. Create a sound subscription in the UI (or POST `/api/subscribe` with `contactType=local_sound`, `contactValue=<deviceId>`).
  2. Allow the poller to enqueue a row (or insert a test `open_event_notifications` row for that subscription).
  3. Run `curl -XPOST http://127.0.0.1:3333/api/notifications/local/claim -H 'content-type: application/json' -d '{"deviceId":"<deviceId>","limit":2}'` and expect `200` + `meta.count` > 0; confirm `fanout_status` flips to `sent` and `last_notified_at` is updated.
  4. With the front-end open, toggle **Enable sound** and confirm a short chime + toast; if silent, click “Click to enable sound” to resume the audio context.
- **Mail channel sanity check:** Local sound leaves `mail_dispatcher` untouched; run `npx tsx scripts/mail_e2e_sim.ts` after changes to confirm email delivery still succeeds.


## Active subscriptions & channel labels
- `GET /api/subscriptions` returns `contactType` (`email` | `local_sound`) per row; the UI falls back to `email` if absent to stay compatible with older data.
- Active list shows a channel pill (“Email”/“Sound”) beside each subscription; cancel/unsubscribe continues to use the same flow for both channels.
- Mixed-channel regression: create one email + one sound subscription, call `curl http://127.0.0.1:3333/api/subscriptions` to verify both `contactType` values, then open Subscription Manager to confirm the badges render and both entries can be removed without errors.

## Monitoring & alert hints
- **Poller metrics** (when `--metrics-port` set): scrape `/metrics` for `poller_polls_total`, `poller_poll_failures_total`, `poller_events_emitted_total`, `poller_notifications_queued_total`, `poller_last_duration_ms`.
  - Alert if `poller_poll_failures_total` increments for >5 minutes or `poller_last_duration_ms` grows steadily (hinting at upstream slowness).
- **Queue health (SQLite)**:
  - Pending backlog: `SELECT fanout_status, COUNT(*) FROM open_event_notifications GROUP BY 1;`
  - Stuck locks: `SELECT COUNT(*) FROM open_event_notifications WHERE fanout_status='pending' AND locked_at < datetime('now','-120 seconds');`
  - Retry churn: `SELECT COUNT(*) FROM open_event_notifications WHERE fanout_status='pending' AND fanout_attempts>=3;`
- **Duplicates:** `SELECT dedupe_key, COUNT(*) FROM open_events WHERE event_at > datetime('now','-3 minutes') GROUP BY 1 HAVING COUNT(*)>1;` (should be empty).

## Troubleshooting
- **No emails going out:** check `open_event_notifications` for `pending` rows; if empty, verify poller is emitting events and `subscriptions.last_known_section_status` not already `OPEN`.
- **Stuck locks / crashed worker:** clear stale locks with `UPDATE open_event_notifications SET locked_by=NULL, locked_at=NULL WHERE fanout_status='pending' AND locked_at < datetime('now','-120 seconds');` then rerun dispatcher with `--once` to drain.
- **Provider errors:** `fanout_status='pending'` with `error` JSON containing `"retryable"` → let retries proceed; `"invalid_recipient"` / `"template_variable_missing"` are terminal and will be marked `skipped`.
- **Duplicate sends reported:** confirm reopen happened within 3-minute dedupe bucket (expected suppression). If reopen after that window still duplicating, inspect `dedupe_key` hash inputs (term/campus/index/status/bucket) and ensure poller clock is synced.
- **No chime for sound channel:** keep the tab active/unmuted, accept the browser autoplay prompt, and check devtools for `AudioContext` being `suspended`. Re-run the `curl` claim above to verify queue state; if empty, wait for the poller or seed a pending row.

## Maintenance / config notes
- Keep poll interval aligned with SLO: 10–20s recommended; higher intervals can breach 60s worst-case latency.
- SendGrid sandbox: set `providers.sendgrid.sandboxMode=true` in `configs/mail_sender.example.json` for dry runs.
- WAL files under `data/` are expected during long runs; avoid deleting while workers are active.
