# Data Load Runbook

_Last updated: 2025-11-17_

This runbook captures the operator steps for initializing and refreshing the Rutgers SOC dataset in `data/courses.sqlite`. It complements `docs/fetch_pipeline.md` (CLI/config surface) and `docs/data_refresh_strategy.md` (architecture) by focusing on the concrete commands, required environment, verification, and failure recovery.

## Prerequisites
| Requirement | Details |
| --- | --- |
| Node.js | v22.x (the repo is currently using `/mnt/d/Software/Nodejs/node.exe`). Run `npm install` once per environment. |
| Python 3 | v3.12+ for validation helpers (`reports/field_validation_samples.json`) and light SQLite checks. |
| SQLite files | `data/courses.sqlite` is created by the fetcher; `data/schema.sql` + migrations live in the repo. Ensure the workspace is writable. |
| Network access | Outbound HTTPS to `https://classes.rutgers.edu/soc/api`. Throttling is handled by the fetcher, but VPN/firewall rules must allow the traffic. |
| Config | Copy or symlink `configs/fetch_pipeline.example.json` to a local variant and update `targets`, `sqliteFile`, and optional `safety` settings. |

## Key files & commands
- `scripts/fetch_soc_data.ts` — ingestion CLI (`npm run data:fetch -- <flags>`).
- `scripts/migrate_db.ts` — runs schema migrations (`npm run db:migrate`).
- `logs/fetch_runs/summary_latest.json|.log` — rolling execution summaries.
- `data/staging/...` — optional raw payload dumps when `--dry-run` is disabled and staging is enabled.
- `reports/field_validation.md` / `reports/field_validation_samples.json` — verification artifacts for spot-checking SQLite vs SOC.

## Pre-flight checks
1. `npm install` (once per machine) and `npm run db:migrate` to ensure the schema matches `data/schema.sql`.
2. Copy `configs/fetch_pipeline.example.json` to `configs/fetch_pipeline.local.json` (or another path) and adjust:
   - `sqliteFile`: usually `data/courses.sqlite`.
   - `targets`: list of `{ term, mode, campuses[] }`.
   - `concurrency`/`retryPolicy`: align with the latest `docs/soc_rate_limit.md`.
3. Confirm disk space (`du -sh data`) and permissions for `data/` and `logs/`.
4. Optional: `npm run data:fetch -- --config <file> --dry-run` to preview the execution plan.

## First-time initialization checklist (~10–15 minutes for one campus/term)
- [ ] **Install & migrate** (3 min). `npm install && npm run db:migrate`. Output: `data/courses.sqlite` created (≈4 MB before ingest) and `data/migrations.log` updated.
- [ ] **Prepare config** (2 min). Copy `configs/fetch_pipeline.example.json`, set `mode` to `full-init`, and list the target term/campus pairs (e.g. `12024/NB`).
- [ ] **Run fetcher** (5–10 min depending on number of subjects). Example:  
  `npm run data:fetch -- --config configs/fetch_pipeline.local.json --mode full-init --terms 12024 --campuses NB`  
  Output: `logs/fetch_runs/summary_latest.{log,json}`, raw payloads under `data/staging/<term>/<campus>/`.
- [ ] **Verify database counts** (1 min).  
  `python3 - <<'PY'\nimport sqlite3\nconn = sqlite3.connect('data/courses.sqlite')\nfor table in ('courses','sections'):\n    cur = conn.execute(f'SELECT COUNT(*) FROM {table} WHERE term_id=?', ('12024',))\n    print(table, cur.fetchone()[0])\nPY`
- [ ] **Spot-check data fidelity** (optional, 5 min). Regenerate `reports/field_validation_samples.json`/`reports/field_validation.md` and confirm SOC vs SQLite parity before the API and notification services start reading the DB.

## Routine incremental update checklist (~2–3 minutes per subject batch)
- [ ] **Confirm config** (1 min). Ensure the incremental queue/filters (`incremental.subjectRecencyMinutes`, `targets`) contain the desired slices.
- [ ] **Execute incremental pull** (≤2 min for a handful of subjects). Example:  
  `npm run data:fetch -- --config configs/fetch_pipeline.local.json --mode incremental --terms 12024 --campuses NB --subjects 198`  
  Output: updated `logs/fetch_runs/summary_latest.*`, `data/courses.sqlite` mutated in-place.
- [ ] **Review summary + errors** (1 min).  
  `python3 - <<'PY'\nimport json\nfrom pathlib import Path\nsummary = json.loads(Path('logs/fetch_runs/summary_latest.json').read_text())\nprint(summary['totals'])\nprint('errors:', summary['sliceSummaries'][0]['errors'] if summary['sliceSummaries'] else [])\nPY`
- [ ] **Optional validation** (2 min). If schema or normalizer changed, rerun the sampling helper to refresh `reports/field_validation*.md`.

## Verification & monitoring
- **Success indicators**: 
  - `logs/fetch_runs/summary_latest.log` ends with `Finished <term>/<campus>` and `errors=[]`.
  - `logs/fetch_runs/summary_latest.json` totals show non-zero `coursesInserted`/`sectionsInserted` or expected updates.
  - `sqlite3 data/courses.sqlite 'PRAGMA integrity_check;'` returns `ok`.
- **Field-level checks**: Run the Python sampler in `reports/field_validation.md` to prove SOC vs SQLite parity after any major change.
- **Disk usage**: Monitor `du -sh data` and prune `data/staging` if it grows past the allotted quota (staging can be disabled via config after initial validation).

## Common errors & mitigations
| Symptom | Likely cause | Mitigation |
| --- | --- | --- |
| `SOCRequestError` with HTTP 429/5xx | SOC rate limit or transient outage | Retry with lower concurrency (`--max-workers 1`, update config `concurrency`) or wait 2–3 minutes before re-running. |
| `SQLITE_BUSY` / database locked | Another process (UI/API) holding a long-lived connection | Pause consumers, rerun with `--dry-run` to confirm queue, then execute when DB is free. |
| `EACCES` or permission denied on `data/` | Workspace or sqlite file not writable | Fix filesystem permissions or run from a writable directory (see harness docs). |
| Gaps in `openSections` stats | `openSections` worker failed after courses fetch | Re-run `npm run data:fetch ... --mode incremental` for the affected campus; the script retries the snapshot if previous step succeeded. |
| Missing `core_codes` / campus entries | SOC returned unexpected payload shape | Inspect `data/staging/<term>/<campus>/courses.all.json`, update `scripts/soc_normalizer.ts`, rerun full-init once patched. |

## References
- `docs/fetch_pipeline.md` — CLI/config deep dive.
- `docs/data_refresh_strategy.md` — hashing + incremental logic.
- `docs/local_data_model.md` — schema definitions.
- `docs/soc_rate_limit.md` — throughput guardrails that inform the `concurrency` block.
- `reports/field_validation.md` — latest SOC vs SQLite sampling evidence.
