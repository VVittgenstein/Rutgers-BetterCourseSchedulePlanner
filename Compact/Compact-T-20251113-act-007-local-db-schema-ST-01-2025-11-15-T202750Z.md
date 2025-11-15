# Compact – T-20251113-act-007-local-db-schema-ST-01

## Confirmed facts
- `scripts/fetch_soc_samples.py` automates Spring 2026 SOC pulls (defaults `year=2026, term=1, campuses=NB,NK,CM`), streams both `courses.json` and `openSections.json` per campus with gzip support, validates by parsing JSON, tallies course/section counts, and persists payloads under `data/raw/spring-2026-<endpoint>-<campus>.json` plus a consolidated metadata log (`data/raw/spring-2026-metadata.json`).
- Metadata entries record URL, headers (`Cache-Control`, `ETag`, `Content-Encoding`), byte sizes, SHA256, and derived counts (`record_count`, `section_count`, `distinct_subjects`), giving downstream jobs structured provenance for each snapshot.
- `docs/db/sample_notes.md` documents the fetch command/time (2025-11-15T20:25:26Z UTC), NB/NK/CM coverage metrics, file sizes, and qualitative observations (e.g., `courseDescription` empty everywhere, `synopsisUrl` coverage uneven, `openSections` returns identical 13,780 indexes regardless of campus).
- Record entry `T-20251113-act-007-local-db-schema-ST-01` now marked `done`, unblocked, with `updated_at=2025-11-15T20:26:33Z` to reflect the completed snapshot deliverables.

## Interfaces / behavior impacts
- New CLI script (`scripts/fetch_soc_samples.py`) is the canonical way to regenerate future SOC snapshots; consuming tools should depend on its metadata contract instead of hardcoded file names.
- Downstream schema/analysis steps must read `data/raw/spring-2026-metadata.json` and `docs/db/sample_notes.md` for authoritative counts and coverage notes rather than duplicating heuristics.
- Observed `openSections` response ignoring campus implies notification/DB logic must continue filtering by `sections[*].index` joined against campus metadata; this behavior is now an explicit documented assumption.

## Risks / TODOs
- `openSections` returning a campus-agnostic list is fragile; need monitoring (e.g., hash comparison) in future fetches to detect when Rutgers changes the behavior.
- Lack of `courseDescription` content means UI/schema must rely on `synopsisUrl` or other sources for course summaries; additional enrichment may be required.
- Script currently exits on first HTTP/URL error; future hardening (retry/backoff, per-endpoint logging) may be desirable if SOC starts flaking.

## Test evidence
- `python3 scripts/fetch_soc_samples.py` (default arguments) – executed successfully, producing NB/NK/CM course and openSections files plus metadata; stderr logs confirm record counts and file paths.
