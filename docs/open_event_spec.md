# Open Event Model & Polling Strategy

_Last updated: 2025-11-20_

## Scope and goals
- Define the contract between the `openSections` poller, the local SQLite store, and downstream notification channels.
- Describe the event payload (`open_event`) that workers emit when a section transitions into an open state, including dedup keys and linkage to subscriptions.
- Lock in a polling cadence, backoff rules, and oscillation handling that respect the SOC rate-limit profile (`docs/soc_rate_limit.md`) while keeping freshness under one minute per campus.

## Canonical event shape
An `open_event` is produced when the poller observes `Closed/Wait → Open` for a section. The same shape is used both for storage and for the in-memory queue handed to channel senders.

| Field | Type | Notes |
| --- | --- | --- |
| `open_event_id` (PK) | INTEGER | Surrogate key in SQLite; optional when represented in memory. |
| `section_id` | INTEGER | FK to `sections.section_id` when the join succeeds; `NULL` for unresolved indexes (fallback to term/campus/index). |
| `term_id` | TEXT | Copied from subscription or section row to allow lookups without a join. |
| `campus_code` | TEXT | Copied from section. |
| `index_number` | TEXT | The SOC index that appeared in `openSections`. |
| `subscription_id` | INTEGER | Nullable; populated in the fan-out stage when a concrete subscription is attached. |
| `status_before` | TEXT | `Closed/Wait/Open/Unknown` snapshot from `sections.last_known_section_status` or prior snapshot. |
| `status_after` | TEXT | Usually `Open`; `Closed` events are recorded for bookkeeping but never fan out. |
| `seat_delta` | INTEGER | `+1` for reopen/open, `-1` for explicit close, `0` when the delta is unknown (e.g., duplicate heartbeat). |
| `event_at` | TEXT | ISO timestamp when the transition was first detected. |
| `detected_by` | TEXT | Enum: `open_sections`, `section_status_event`, `manual`. |
| `snapshot_id` | INTEGER | FK to `open_section_snapshots.snapshot_id` when derived from the poller. |
| `dedupe_key` | TEXT | `sha1(term|campus|index|status_after|bucket_3m)`; prevents duplicate sends within the same freshness window. |
| `trace_id` | TEXT | Correlates with poller logs and downstream sender traces. |
| `payload` | TEXT (JSON) | Channel-agnostic context (course title, meeting summary) materialized at detection time to avoid stale joins. |

Two storage surfaces keep the lifecycle simple:
- `open_events` (append-only) holds the section-level transitions above.
- `open_event_notifications` links events to subs for delivery: \
  `open_event_id`, `subscription_id`, `fanout_status (pending|sent|skipped|failed|expired)`, `fanout_attempts`, `last_attempt_at`, `locked_by`, `locked_at`, `error`.

## Polling cadence and ordering
The cadence mirrors the `openSections` profile in `docs/soc_rate_limit.md` (safe at 30–50 req/s but self-throttled for politeness):
- **Default loop**: Each active `{term, campus}` is polled every **≈10–20 seconds** (15 s target + ±30% jitter). A round-robin queue prevents a single campus from starving others.
- **Concurrency**: Up to **10 workers** with **250 ms gaps** between requests (≈30 req/s observed). Term or campus count simply controls queue length; workers keep the per-campus freshness <1 s when bursts are needed.
- **Backoff**: On the first non-2xx, pause 15 s then resume at **5 workers / 500 ms**. After 5 consecutive successes the loop ramps back to the default profile. 429/5xx also clear inflight locks before rescheduling.
- **Source of truth**: Poller writes every heartbeat into `open_section_snapshots` with `seen_open_at` and `source_hash`; course ingest continues to write `section_status_events` so the notification loop can reconcile when the heartbeat misses an update.

## Lifecycle and oscillation handling
1. **Detect** – Poller compares the latest `openSections` list to `open_section_snapshots` and the last `section_status_events` row for the same section: \
   - New index or reappearance after `closed_at` → emit `status_before=Closed`, `status_after=Open`, `seat_delta=+1`. \
   - Index missing for ≥2 consecutive polls (≈20–40 seconds with current interval) → record `Closed` housekeeping event with `seat_delta=-1` to reset state but do not notify.
2. **Deduplicate** – Skip creating a new `open_event` if the same `dedupe_key` exists in the last 3 minutes. This absorbs CDN jitter and prevents repeat sends until a real close is observed.
3. **Fan-out** – Build `open_event_notifications` by joining `open_events.status_after=Open` with `subscriptions` where `status IN ('active','pending')` and `last_known_section_status <> 'Open'`. Respect per-subscription quiet hours (`deliveryWindow`), `maxNotifications`, and `snoozeUntil`. Populate `subscription_id` and carry `dedupe_key` forward for channel deduping (`bucket_3m`).
4. **Send** – Channel workers pick `fanout_status=pending`, mark a short-lived lock, call the channel sender once, then: \
   - On success → increment `fanout_attempts` by 1, update `fanout_status=sent`, bump `subscriptions.last_known_section_status='Open'`, append a `subscription_events.notify_sent` row with the section status snapshot, clear the lock. \
   - On retryable failure → increment `fanout_attempts` by 1, keep `pending`, update `last_attempt_at`, persist the error for backoff scheduling (see mail worker contract), clear the lock. \
   - On terminal failure or ineligible subscription → increment `fanout_attempts` by 1, set `fanout_status=skipped/failed`, record the error, clear the lock. \
   `fanout_attempts` counts outer queue tries (one per MailSender attempt) and drives retry schedules defined per worker contract.
5. **Reset and archive** – When `Closed` is recorded (via heartbeat miss or course ingest), set `subscriptions.last_known_section_status='Closed'` so a future reopen triggers fresh events. Expire archived `open_events` and `open_event_notifications` after 30 days to keep the DB small while retaining traceability.

### Oscillation rules
- Rapid `Open→Closed→Open` oscillations generate **one** notification until a `Closed` event clears the dedupe window (two missed heartbeats or explicit `section_status_events` flip). A reopened section after that point triggers a new `dedupe_key`.
- `seat_delta` stays `0` when the upstream keeps the section open across consecutive polls; notifications only fire on the first `+1` edge.
- If `openSections` returns temporarily empty for a campus, treat it as a soft outage: record no `Closed` events, pause 30 s, and retry with the downgraded profile.

With this contract, the poller, event store, and channel workers share a deterministic handshake: only real `Closed→Open` edges reach users, jitter is absorbed by snapshot deduping, and every send is traceable via `open_event_id` + `subscription_id`.
