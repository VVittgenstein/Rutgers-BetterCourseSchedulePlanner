# Compact – T-20251113-act-001-soc-json-scraper-ST-02

## Confirmed
- Added `docs/data_pipeline/init_flow.md`, splitting the SOC API → SQLite initialization into Stage A–F (queue expansion, scheduler fetch, snapshot登记, staging normalize, diff/promote, ready checklist) with Mermaid + ASCII fallback so the orchestration logic is unambiguous.
- Each stage now lists concrete inputs, actions, outputs/artifacts (`data/raw/*.json.gz`, `manifest.jsonl`, `raw_snapshots`, `staging_*`, `dataset_versions`, `ready_report_*`) and concurrency guarantees (term×campus parallelism vs. global serial phases), giving the implementation exact sequencing and persistence expectations.
- Document defines component boundaries: `scripts/fetch_soc_data.ts` orchestrates CLI (`--terms`, `--campuses`, `--mode`, `--max-workers`, `--inline-blob`, `--resume-run`), reuses `scripts/fetch_soc_samples.py` for HTTP/GZIP + manifest logging, and calls `scripts/migrate_db.ts` for normalization/diff/promotion, with required logs/metrics captured in `logs/fetch_soc_data/*.jsonl` and `dataset_version_events`.
- Failure/retry nodes and observability hooks are enumerated (network retry backoff, staging validation, promote rollback, alerts on repeated failures), ensuring downstream operators know where to add alarms.

## Tests
- Documentation-only change; no automated tests executed.

## Risks / TODO
- Scheduler concurrency controls, resume-run state machine, and ready checklist automation still need implementation per the documented contract.
- CI/observability wiring from `logs/*.jsonl` and `dataset_version_events` to real alerting systems remains open.

## Interfaces / Impact
- Data-pipeline/build scripts must align with the Stage A–F contract; `scripts/fetch_soc_data.ts` becomes the authoritative orchestrator entry point with the documented CLI and artifact outputs.
- Database tooling (`scripts/migrate_db.ts`) must expose the normalize/diff/promote operations exactly as staged, so canonical tables and `dataset_versions` match the workflow; ready reports become required outputs before downstream consumers rely on a term×campus dataset.

## Code Review - T-20251113-act-001-soc-json-scraper-ST-02 - 2025-11-15T22:49:47Z
Codex Review: Didn't find any major issues. What shall we delve into next?
