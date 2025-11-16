## Subtask
- ID: ST-20251113-soc-api-validation-01-probe
- Scope: build a minimal Rutgers SOC probe CLI that unifies `courses.json` / `openSections` calls, surfaces request metrics, and records sample outputs for ≥3 term/campus combos.

## Confirmed Implementation Facts
1. Added npm/TypeScript tooling (`package.json`, `package-lock.json`, `tsconfig.json`) with `npm run soc:probe` wired to execute `scripts/soc_probe.ts` via `tsx`.
2. `scripts/soc_probe.ts` parses CLI flags (`term`, `campus`, `subject`, `endpoint`, `samples`, `timeout`, `level`), normalizes semester inputs into SOC `year` + `term` parameters, issues a single fetch to `https://classes.rutgers.edu/soc/api/<endpoint>.json`, and prints status code, latency, decoded size, and filtered sample rows.
3. The probe logs structured JSON errors (request id, endpoint, retry hint) for HTTP errors, JSON parse failures, network issues, and timeouts before exiting non-zero.
4. `docs/soc_api_notes.md` documents how to run the CLI plus three recorded scenarios (Spring24 NB subject 198, Fall24 NK subject 640, Summer24 CM openSections) with request ids, dataset sizes, and sample outputs.
5. `record.json` marks this subtask as `done`, clears blockers, and sets `updated_at` to `2025-11-16T00:00:00Z` for traceability.

## Interface / Behavior Changes
- New CLI command `npm run soc:probe` is now part of the developer workflow; it accepts Rutgers semester aliases (`12024`, `FA2024`, etc.), emits human-readable summaries, and can be reused by downstream scripts for multi-term probing.
- New artifact `docs/soc_api_notes.md` serves as the canonical reference for SOC probe usage and captured samples.

## Risks / Limits / TODOs
- The probe currently runs a single HTTP request per invocation; batching/concurrency control for larger sweeps remains TBD.
- Subject filtering is performed only client-side (per SOC behavior); no server-side filtering guarantees beyond what the API returns.
- Rate-limit profiling and wider field coverage are explicitly deferred to subtasks ST-20251113-soc-api-validation-02-field-matrix and ST-20251113-soc-api-validation-03-limit-profile.

## Self-tests
- `npm run soc:probe -- --term 12024 --campus NB --subject 198 --samples 2` (courses endpoint, success)
- `npm run soc:probe -- --term 92024 --campus NK --subject 640 --samples 2` (courses endpoint, success)
- `npm run soc:probe -- --term 72024 --campus CM --endpoint openSections --samples 5` (openSections endpoint, success)
