# Data Refresh Strategy

_Last updated: 2025-11-17_

This document describes how the Rutgers SOC payloads are ingested into the local SQLite schema defined in `data/schema.sql`, how incremental deltas are detected, and when a forced full rebuild is required. It complements the entity notes in `docs/local_data_model.md` by focusing on operational mechanics (queues, hashing, cleanup and tooling).

## Objectives
- Keep the SQLite database as the single source of truth for FR-01/FR-02 filtering and FR-04 notifications.
- Allow frequent, low-risk refreshes on laptops or small servers by only touching rows that changed between SOC snapshots.
- Preserve provenance (raw payload + deterministic `source_hash`) so that diffs are auditable and replays are idempotent.
- Avoid blocking UI/API readers: all refresh work is wrapped in short WAL transactions scoped to a `term+campus+subject` slice.

## Source slices and scheduling
| Slice | Purpose | Driver | Suggested cadence |
| --- | --- | --- | --- |
| `courses.json` per `term+campus+subject` | Structural data (courses, sections, meetings, instructors) | Batch worker | Full refresh nightly for active terms; incremental loop every 30–60 min during add/drop. |
| `openSections` per `term+campus` | Fast vacancy deltas | Poller (lightweight) | Every 60–120 s per campus; writes into `open_section_snapshots` + updates `sections.is_open`. |

Workers share a queue keyed by `{term, campus, subject}` so that a heavy subject cannot starve others. The queue is persisted (e.g., `data/refresh_queue.json`) to resume after crashes.

## Change detection per entity

| Entity | Natural key | Incremental signal | Notes |
| --- | --- | --- | --- |
| `terms`, `campuses`, `subjects` | IDs from SOC | `INSERT ... ON CONFLICT DO UPDATE` | `subjects.active` flipped to `0` when a subject disappears from the latest payload. |
| `courses` | `term_id + campus_code + subject_code + course_number` | `courses.source_hash` (SHA1 of normalized course fields) | `source_hash` excludes section arrays so open-seat chatter does not churn course rows. |
| `sections` | `term_id + index_number` | `sections.source_hash` | When the hash changes, `sections.updated_at` is refreshed and open-status transitions generate entries in `section_status_events`. |
| `section_meetings` | `(section_id, hash)` | Derived `hash` per meeting row | We rebuild the entire set for a section when its hash changes (child tables stay tiny). |
| `section_instructors`, `section_populations`, `section_crosslistings` | `(section_id, instructor_id/code)` | Compare deterministic arrays | Simple `DELETE + INSERT` scoped to the changed section. |
| `subscriptions`, `open_section_snapshots` | `index_number + contact` / `term+campus+index` | Not touched by course refresh | Section refresh updates `sections.last_known_section_status` so notification logic stays consistent. |

The normalized JSON used for hashing is described inline within `scripts/incremental_trial.ts` and will be reused by the production ingest once the same helpers are moved under `src`. Hashing uses sorted keys (`stableStringify`) to avoid false positives.

## Refresh pipeline
1. **Build worklist** – For each active term (config-driven), enqueue `campus × subject` combos last seen >N minutes ago or explicitly invalidated (e.g., after schema migration). A queue item carries `term_id`, `campus_code`, `subject_code`, retry count and the `courses.json` ETag if known.
2. **Fetch + stage** – Call `performProbe({endpoint:'courses'})` with per-lane throttling (see `docs/soc_rate_limit.md`). The worker writes raw JSON to `data/staging/<term>/<campus>/<subject>.json` for troubleshooting and loads the parsed rows into in-memory objects.
3. **Compute hashes** – Normalize courses and sections (same logic the incremental trial script uses) and fill two temp tables:
   ```sql
   CREATE TEMP TABLE staging_courses (..., source_hash TEXT, source_payload TEXT);
   CREATE TEMP TABLE staging_sections (..., source_hash TEXT, source_payload TEXT);
   ```
   Each staging row includes `term_id`, `campus_code`, `subject_code`, `course_number`, `index_number` plus derived helpers (`has_core_attribute`, `meeting_mode_summary`, etc.).
4. **Apply inside a transaction**:
   - Upsert reference rows (terms/campuses/subjects) first.
   - `INSERT INTO courses ... ON CONFLICT (...) DO UPDATE SET ... WHERE excluded.source_hash <> courses.source_hash`. When the hash differs we update mutable columns, refresh `updated_at`, and replace `source_payload`.
   - `INSERT INTO sections ... ON CONFLICT (term_id, index_number) DO UPDATE` with the same hash guard. Before updating a section we capture `is_open` + `open_status` so we can append to `section_status_events` if it flips from closed→open or vice versa.
   - Replace child tables for the touched sections (meetings, instructors, populations, crosslistings) by deleting the old rows for those section IDs and bulk inserting the staged ones.
5. **Prune missing rows** – Anything present in the database but absent from the latest staging set for that slice is deleted:
   ```sql
   DELETE FROM sections
   WHERE term_id = :term
     AND campus_code = :campus
     AND subject_code = :subject
     AND index_number NOT IN (SELECT index_number FROM staging_sections);

   DELETE FROM courses
   WHERE term_id = :term
     AND campus_code = :campus
     AND subject_code = :subject
     AND course_number NOT IN (SELECT course_number FROM staging_courses);
   ```
   Because `sections` references `courses.course_id`, deletions cascade into `section_*` tables thanks to `ON DELETE CASCADE`.
6. **Post-processing** – Refresh `course_search_fts` for rows whose `course_id` appeared in staging, vacuum the WAL if file size grew >128 MB, and log batch stats: counts, durations, hash mismatches and HTTP latency.

All steps run inside WAL transactions shorter than one second for typical subjects. Readers only see committed rows, and staging tables disappear when the transaction closes.

## Observability and tooling
- `npm run data:incremental-trial -- --term 12024 --campus NB --subjects 198,640,750` replays the hashing + diff logic against live SOC data. The script adds simulated legacy rows so we can verify insert/update/delete paths without waiting for a real SOC change. See `notebooks/incremental_trial.md` for the latest run.
- `data/migrations.log` captures schema changes coming from `npm run db:migrate` so refresh jobs know whether to invalidate cached queue entries.
- Each refresh job logs JSON snippets with `term/campus/subject`, counts of `added/updated/deleted` rows, and the max request latency. These logs are tailed by health dashboards in later milestones.

## Full rebuild triggers and runbook
Certain events require discarding the incremental queue and re-ingesting all slices:

1. **Schema-breaking migration** – e.g., changing constraints or adding NOT NULL columns. _Runbook_: `npm run db:migrate`, back up `data/local.db`, then for each active term enqueue every `campus×subject`. Delete `course_search_fts` and rebuild via full refresh.
2. **Hash bugs or ingest drift** – If a bug produced incorrect `source_hash` values (detected via audit scripts), incremental updates might skip rows. _Runbook_: drop/rename `courses.source_hash` (or reset to NULL) and re-run a full refresh so hashes are recomputed.
3. **SOC API structural change** – If `courses.json` adds/removes nested fields the normalizer depends on, hashes may oscillate. _Runbook_: freeze refreshers, update the normalizer (and `docs/local_data_model.md`), then perform a full refresh once the new mapping ships.
4. **Stale queue / long outage** – If refreshers fall behind more than one academic week (e.g., laptop asleep), we prefer a fresh full run to avoid reconciling thousands of deletions.
5. **Corrupted local.db** – Detected via failed WAL checkpoint or pragma integrity check. _Runbook_: `sqlite3 data/local.db ".backup data/local.db.bak"` then recreate the DB from migrations followed by a full refresh.

During a rebuild, notifications remain paused (no `openSections` polling) so we do not send alerts based on partially-ingested data.

## Next steps
- Promote the normalizer and hashing helpers from `scripts/incremental_trial.ts` into the production ingest to avoid divergence.
- Add a lightweight SQLite view that exposes recent `section_status_events` so health checks can confirm the pipeline is alive.
- Automate queue persistence (JSON + checksum) so laptop restarts resume the last unfinished slice.
