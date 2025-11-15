# Compact – T-20251113-act-001-soc-json-scraper-ST-01

## Confirmed
- Added `docs/db/ready_checklist.md` defining the "DB ready" target state per term×campus, covering raw snapshot metadata (`raw_snapshots`, `dataset_versions`, `dataset_version_events`), normalized tables (`courses`, `sections`, `section_meetings`, `open_section_states` etc.) and required indexes/coverage metrics aligned with FR-01/FR-02.
- Documented mandatory validation flow: CLI automation must emit JSON metrics (row counts vs snapshot counts, schema/index/not-null checks) plus manual PRAGMA/count queries and ≥10-course sample comparison recorded in `dataset_version_events`.
- Introduced a persistent readiness log template with a Spring 2026 NB example to capture dataset_version_id, snapshot IDs, fetch command, script hash, counts, and reviewer for each completed build.

## Tests
- Documentation-only change; no automated tests executed.

## Risks / TODO
- Ready checklist references `scripts/db_ready_check.py`; implementation & CI integration remain pending.
- Checklist assumes `openSections` response continues global index behavior; document notes the need to revisit if Rutgers changes API semantics.

## Interfaces / Impact
- Database/Docs teams must consume `docs/db/ready_checklist.md` as the acceptance contract before promoting any term×campus dataset; downstream build scripts should emit the specified JSON payload and update the readiness log after each successful run.

## Code Review - T-20251113-act-001-soc-json-scraper-ST-01 - 2025-11-15T21:57:51Z
Codex Review: Didn't find any major issues. Can't wait for the next one!
