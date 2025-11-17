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

## Code Review - ST-20251113-soc-api-validation-02-field-matrix - 2025-11-17T01:40:13Z

Codex Review
BetterCourseSchedulePlanner/record.json

Lines 147 to 151 in 9a1f13b

 "dependencies": [ 
   "ST-20251113-soc-api-validation-01-probe", 
   "ST-20251113-soc-api-validation-02-field-matrix" 
 ], 
 "blocked": true, 

P2 Badge Unblock rate-limit subtask once prerequisite is done
The rate-limit follow-up subtask (ST-20251113-soc-api-validation-03-limit-profile) remains blocked: true even though its only dependency (ST-20251113-soc-api-validation-02-field-matrix) is now marked done in the same record. With blocked still set, any tooling that skips blocked items will continue to ignore the rate-limit work despite the dependency having completed. Consider flipping blocked to false and clearing the blocked_by entry so the task can proceed.---review-end---


Codex Review: Didn't find any major issues. Hooray!
