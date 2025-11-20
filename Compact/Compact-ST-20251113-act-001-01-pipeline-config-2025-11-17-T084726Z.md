### Subtask ST-20251113-act-001-01-pipeline-config

**Confirmed facts**
- Added `docs/fetch_pipeline.md` describing the `npm run data:fetch` CLI flags, runtime overrides, hidden debug switches, config structure, rate-limit-aligned batching/retry logic, and separate runbooks for `full-init` vs `incremental` flows.
- Added `configs/fetch_pipeline.example.json` showing the declarative pipeline schema (run metadata, concurrency caps, retry/backoff profile, target term/campus matrix, per-mode settings, summary outputs, safety toggles) that downstream tooling must obey.
- `record.json` now marks this subtask done and links both artifacts as deliverables.

**Interface / behavior implications**
- Introduces a stable CLI contract for `scripts/fetch_soc_data.ts`, including required `--config` flag semantics and overrides for terms/campuses/subjects/worker caps.
- Establishes expected config keys (e.g., `concurrency.*`, `retryPolicy.*`, `targets[].mode`, `incremental.resumeQueueFile`, `fullInit.truncateTables`), so future code must parse/validate these names exactly to remain compatible with the documentation.

**Risks / limits / TODOs**
- No executable changes or tests yet; ingestion script still needs to honor the documented contract and enforce schema validation (risk of drift until implementation catches up).
- Rate-limit assertions depend on `docs/soc_rate_limit.md` dated 2025-11-17; if Rutgers changes behavior, both doc and config defaults need a refresh.

**Self-test status**
- Tests not run (documentation/config-only change).
