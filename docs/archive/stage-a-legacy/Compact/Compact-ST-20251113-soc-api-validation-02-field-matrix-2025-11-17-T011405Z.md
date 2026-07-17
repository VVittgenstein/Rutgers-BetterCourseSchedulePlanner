## Confirmed
- Added `scripts/soc_field_matrix.py`, a Python runner that downloads each `term+campus` payload once, caches responses, aggregates course/section/meeting/instructor field presence, annotates FR-01/FR-02 mappings (plus manual entries for null fields), and emits `docs/soc_field_matrix.csv`.
- Script executed all 2 terms × 3 campuses batches, reused payloads for 5 representative subjects with U/G hints (42 combos) and logged subject-level course/section counts; total coverage measured 13,322 courses, 32,249 sections, 45,884 meeting rows, 23,863 open indexes.
- `docs/soc_field_matrix.csv` now holds 78 scoped rows with presence ratios, FR mapping tags, and notes (e.g., `credits` 89% vs `creditsObject` 100%, `offeringUnitTitle` always null, `openSections` exposes only index strings, no capacity/notes).
- `docs/soc_api_notes.md` gained a “Field matrix batch” section summarizing the runner command, dataset sizing table per campus/term, FR coverage snapshot, openSections limitations, and subject-level sampling facts; also documents that SOC ignores `subject` and `level` params.
- `package.json` includes a new npm script `soc:field-matrix` that runs the Python generator; docs instruct using this command.
- `record.json` marks `ST-20251113-soc-api-validation-02-field-matrix` as `done` with updated timestamp and clears the blocking status on the subsequent rate-limit subtask.

## Interfaces / Behavior Changes
- New CLI entry `npm run soc:field-matrix` depends on a local Python 3 interpreter and network access; downstream automation can call it to refresh the CSV artifact.
- Generated artifact `docs/soc_field_matrix.csv` plus expanded guidance in `docs/soc_api_notes.md` serve as the field matrix referenced by acceptance criteria.

## Risks / Limits / TODOs
- SOC endpoints still omit seat capacity, waitlist size, and openSection metadata beyond index strings; the CSV and notes flag these as “missing,” so alerting must continue relying solely on state transitions.
- Script assumes Rutgers SOC responses remain compatible and that repeated downloads are acceptable; no retry/backoff is implemented beyond Python urllib defaults.
- `offeringUnitTitle` remains null across the dataset; full department names must continue to come from `school` or subject dictionaries.

## Testing
- `python3 scripts/soc_field_matrix.py`
