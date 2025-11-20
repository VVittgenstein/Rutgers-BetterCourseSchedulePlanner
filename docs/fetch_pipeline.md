# SOC Fetch Pipeline

_Last updated: 2025-11-17_

`scripts/fetch_soc_data.ts` is the single runner for Rutgers SOC ingestion. It hides the raw SOC endpoints (`courses.json`, `openSections`) behind a declarative config file and wraps every write in SQLite transactions so UI/API consumers always read consistent rows. This note defines the CLI, configuration surface, batching strategy, and how the runner differentiates between a first-time initialization and day-2 incremental refreshes.

> **Status**: `npm run data:fetch` currently validates configs and prints the execution plan only. The actual fetch/execution loop is implemented under `ST-20251113-act-001-02-ingest-impl`; once that lands the same CLI + config remain valid.

## CLI entry point
```bash
npm run data:fetch -- \
  --config configs/fetch_pipeline.example.json \
  --mode incremental \
  --terms 12024 \
  --campuses NB,NK \
  --subjects 198,640,750 \
  --dry-run
```

| Flag | Purpose |
| --- | --- |
| `--config <path>` | Load pipeline definition (see `configs/fetch_pipeline.example.json`). Required unless every flag is supplied inline. |
| `--mode <full-init|incremental>` | Override the config-level default. Determines batching + database behaviors described below. |
| `--terms <list>` | Comma separated override of term IDs from the config `targets` array. Keeps CLI short when iterating on a single term. |
| `--campuses <list>` | Same override for campuses. Applies per term; e.g. `--campuses NB,NK`. |
| `--subjects <list|ALL>` | Filters the subject queue for incremental runs. `ALL` can force a full scan without editing the config file. |
| `--max-workers <n>` | Caps both course and openSections workers at runtime. Useful when the operator is on a constrained laptop. |
| `--resume <path>` | Reload a serialized work queue (defaults to the config `incremental.resumeQueueFile`). |
| `--dry-run` | Parse config and plan batches without touching the database; prints the execution graph for smoke testing. |
| `--help` | Output effective config, inferred rate-limit profile, and exit. |

The CLI also exposes hidden debugging switches (`--dump-requests`, `--keep-staging`) which bubble up raw payloads to `data/staging/...` for post-mortems. They stay undocumented in `--help` but are covered here to ensure the config example makes sense.

## Configuration layout
`configs/fetch_pipeline.example.json` shows the expected structure (excerpt below). The config schema used by editors and CI lives at `configs/fetch_pipeline.schema.json`.

```json
{
  "$schema": "./fetch_pipeline.schema.json",
  "defaultMode": "incremental",
  "rateLimitProfile": "docs/soc_rate_limit.latest.json",
  "concurrency": {
    "maxCourseWorkers": 3,
    "courseRequestIntervalMs": 600,
    "maxOpenSectionsWorkers": 10,
    "openSectionsIntervalMs": 250
  },
  "retryPolicy": {
    "maxAttempts": 4,
    "backoffMs": [0, 3000, 7000, 15000],
    "downgradedProfile": { "maxCourseWorkers": 1, "courseRequestIntervalMs": 1200 }
  },
  "targets": [
    { "term": "12024", "mode": "full-init", "campuses": [{"code": "NB", "subjects": ["ALL"]}] },
    { "term": "92024", "mode": "incremental", "campuses": [{"code": "NB", "subjects": ["198", "640", "750"]}] }
  ]
}
```

Key sections:
- **Top-level metadata** – `runLabel`, `defaultMode`, file paths for SQLite (`data/courses.sqlite`), staging dumps (`data/staging`) and logs (`logs/fetch_runs`). The runner persists human + JSON summaries according to `summary.writeText` / `summary.writeJson` and guards against accidental writes when `safety.requireCleanWorktree` is true.
- **Work matrix (`targets`)** – The declarative plan describing which term/campus pairs should run and whether each uses `full-init` or `incremental` semantics. Every entry can further narrow subjects, batch sizes and recency windows (`subjectBatchSize`, `subjectRecencyMinutes`).
- **Concurrency section** – Mirrors the SOC rate-limit recommendations: course pulls default to 3 workers with a 600 ms gap (≈3.3 req/s), while openSections polls may scale to 10 workers @ 250 ms (≈40 req/s theoretical). Extra guardrails limit simultaneous campuses and subject workers so laptops do not exhaust RAM.
- **Retry policy** – Operators set the exponential schedule plus a downgraded profile. After the second failure the runner swaps to the low-frequency profile (`1×1200 ms` for courses, `5×500 ms` for openSections) and only returns to the nominal profile after 5 clean pulls.
- **Mode-specific settings** – The `fullInit` block controls destructive actions (`truncateTables`, `rebuildFts`, `prerunMigrations`), while `incremental` defines how long a subject stays eligible (`subjectRecencyMinutes`), how many retries to tolerate, and where the queue is checkpointed.
- **Safety + observability** – `safety.dryRun` toggles “plan only”, and `summary.emitMetrics` controls lightweight stats the API layer can scrape.

## Batching, retries, and rate-limit alignment
The pull behavior follows the measurements documented in `docs/soc_rate_limit.md` (2025-11-17 sweep) so we remain comfortably under Rutgers’ informal limits:

1. **courses.json batches** – The runner groups `term×campus` combos three at a time (`maxCourseWorkers=3`) with 600 ms gaps, reproducing the 3.39 req/s “sweet spot” observed in the rate-limit study. Payloads larger than~20 MB benefit from sequential gzip decode, so increasing workers past three only lengthens latency. When a run switches to `full-init`, the worker pool still throttles to this profile after each transaction commits.
2. **openSections polling** – Incremental mode optionally attaches an openSections heartbeat with up to 10 workers @ 250 ms gaps. This stays below the self-imposed <25 req/s steady-state cited in the rate-limit document while keeping vacancy freshness under a second per campus. Catch-up bursts are supported by temporarily raising `maxOpenSectionsWorkers` in config; after any failure the CLI reverts to the downgraded 5 workers @ 500 ms profile for at least one minute.
3. **Retry / backoff policy** – `retryPolicy.maxAttempts` counts logical attempts per slice. Every retry multiplies the delay by two (matching the `backoffMs` array) and introduces ±30 % jitter to avoid lockstep retries during outages. All retryable scenarios (HTTP 408/429/5xx plus client timeouts) mark the slice as “dirty”, persist the retry count to `data/refresh_queue.json`, and downgrade concurrency once two failures occur within 5 minutes.
4. **Rollback & staging** – Regardless of mode, each slice writes raw payloads to `data/staging/<term>/<campus>/<subject>.json` and only publishes normalized rows inside a single WAL transaction. On fatal errors the transaction is rolled back, the staging file is retained (unless `--keep-staging=false`), and the slice is re-queued with its retry metadata. Operators can force deletion of staging data via `--keep-staging=0` to save disk.

Because the rate-limit captures already stress-tested 12×150 ms and 32×50 ms scenarios, the runner validates at startup that requested worker counts do not exceed those stress values; otherwise it refuses to start with a clear log message.

## Mode-specific flows
### Full initialization (`full-init`)
1. **Pre-flight** – Ensure `npm run db:migrate` (from `ST-20251113-act-007-02-migration-tooling`) has run and the SQLite file is at the latest schema. The CLI refuses to start if pending migrations exist.
2. **Queue composition** – Expand every `term×campus` entry from the config (subjects default to `ALL`). Each slice is marked `forceRefresh=true` so deletions are enabled.
3. **Database prep** – Optional `truncateTables` executes in dependency order (child tables first) followed by WAL checkpointing. When `fullInit.rebuildFts` is true we also rebuild `course_search_fts` immediately after the first commit to avoid long-running vacuum later.
4. **Fetch + apply** – Courses are fetched using the 3-worker profile; sections/instructors child tables are bulk rebuilt, and `pruneMissingRows` is enabled so anything absent from the current SOC payload is deleted.
5. **Post actions** – After all slices succeed we vacuum the DB, recompute SQLite stats, and emit the summary file (counts of inserted/updated/deleted courses/sections, duration, errors). Notifications stay paused until summaries confirm zero pending slices.

### Incremental updates (`incremental`)
1. **Queue seed** – Load `data/refresh_queue.json` (if present) and merge with freshly-detected slices: new `term×campus` combos from config plus recently-updated subjects (within `subjectRecencyMinutes`). Operators can override subjects via CLI to hotfix a single subject.
2. **Selective pruning** – Incremental mode never truncates tables. Instead, we compute source hashes (see `docs/data_refresh_strategy.md`) and update only rows whose hash changed. Deletions occur only when the SOC payload explicitly omits a section that previously existed for the same subject.
3. **OpenSections tie-in** – After a section gets upserted, the heartbeat job optionally runs (if `maxOpenSectionsWorkers>0`) to capture fresh vacancy snapshots. Failures or rate-limit downgrades in this poller don’t block the course ingest but are surfaced in the summary file under `openSections.status`.
4. **Resume + checkpoint** – Every successful slice resets its retry counter and removes itself from `data/refresh_queue.json`; partial queues are flushed every minute so laptops can pause mid-run without losing progress. If a migration occurs mid-loop the CLI halts and asks the operator to re-run `full-init`.

## Run outputs and validation
- `logs/fetch_runs/summary_latest.json` (see config) captures inserted/updated/deleted counts per slice, elapsed ms, retry counts, and the worker profile actually used. A rolling text log mirrors the JSON for quick CLI inspection.
- Staging dumps remain until `--keep-staging=0` or a nightly cleanup removes files older than 7 days.
- After either mode finishes, operators are expected to spot-check ≥10 courses against the live SOC site, verifying FR-01/FR-02 field coverage before enabling downstream consumers.

With this doc + example config, the ingestion implementation team has the complete contract needed to wire up CLI parsing, config validation, and operational guardrails for both the initial bootstrap and ongoing refreshes.
