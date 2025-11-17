## Confirmed
- `scripts/soc_field_matrix.py` implements the SOC field-matrix runner: downloads each term+campus payload once, caches responses, aggregates course/section/meeting/instructor field presence, tags FR-01/FR-02 coverage (including manual handling for null-only fields), and writes `docs/soc_field_matrix.csv`.
- Batch run covered 2 terms × 3 campuses, then reused the payloads for 5 representative subjects with U/G hints (42 combos). Captured totals: 13,322 courses, 32,249 sections, 45,884 meeting rows, 23,863 open section indexes.
- `docs/soc_field_matrix.csv` now lists 78 scoped fields with presence ratios, FR mapping labels, sample values, and notes (e.g., `credits` 89% vs `creditsObject` 100%, `offeringUnitTitle` always null, `openSections` indexes only).
- `docs/soc_api_notes.md` documents the new runner, coverage stats per campus/term, FR coverage snapshot, openSections limitations, and subject-level sampling observations; also records that SOC ignores `subject`/`level` query params.
- Added npm script `soc:field-matrix` (Python 3 dependency) so automation can regenerate the CSV in one command.
- `record.json` marks subtask ST-20251113-soc-api-validation-02-field-matrix as done and unblocks the follow-up rate-limit subtask (blocked=false, cleared `blocked_by`).

## Interfaces / Behavior Changes
- New CLI entry `npm run soc:field-matrix` plus artifact `docs/soc_field_matrix.csv` and expanded `docs/soc_api_notes.md` constitute the field matrix deliverable referenced by acceptance criteria.
- Downstream planners can now schedule ST-03 (rate-limit profiling) because the blocker flag was cleared.

## Risks / Limits / TODOs
- SOC APIs still omit capacity, waitlist size, and openSection metadata beyond index strings; notifications must rely solely on status changes captured via `courses.json` + `openSections`.
- Runner assumes SOC payload format remains stable and performs single-shot downloads without retry/backoff besides urllib defaults.
- `offeringUnitTitle` is always null; use `school.description` / subject dictionaries for human-readable department names.

## Testing
- `python3 scripts/soc_field_matrix.py`
