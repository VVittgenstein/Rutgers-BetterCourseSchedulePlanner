# Incremental Strategy Trial – 2025-11-17

Objective: validate the hashing/diff workflow on ≥3 subjects before wiring it into the production ingest. The run exercises simulated add/update/delete paths without mutating the real database.

## Command
```
npm run data:incremental-trial -- --term 12024 --campus NB --subjects 198,640,750
```

The script reuses `performProbe` (same network stack as `soc:probe`), normalizes each `courses.json` payload, then mutates the “previous snapshot” to mimic legacy data:
- Subject 198 ➜ drop one course, flip a section’s `openStatus`, and remove another section.
- Subject 640 ➜ inject a ghost course (later deletion) and remove the earliest section.
- Subject 750 ➜ drop one course, extend a section’s meeting block, and add a ghost section (later deletion).

## Results

| Subject | Courses | Sections | Δ Courses (+/−/~) | Δ Sections (+/−/~) | fetch/diff/total (ms) | Simulation notes |
| --- | --- | --- | --- | --- | --- | --- |
| 198 | 66 | 627 | 1 / 0 / 1 | 2 / 0 / 1 | 305 / 3 / 332 | removed 198-684, flipped 06561, removed 06562 |
| 640 | 82 | 429 | 0 / 1 / 0 | 1 / 0 / 0 | 152 / 2 / 164 | ghost course inserted, deleted section 08045 |
| 750 | 59 | 313 | 1 / 0 / 0 | 1 / 0 / 0 | 193 / 2 / 204 | ghost section added, dropped 750-106 |

Aggregate:
- Courses processed: 207 (≈699 ms cumulative runtime). Δ totals: +2 added / −1 removed / ~1 updated.
- Sections processed: 1,369 (≈825 estimated open). Δ totals: +4 added / −0 removed / ~1 updated.

The sub-second diff phase (≤3.4 ms) confirms the staging tables + hashing approach stay lightweight per subject. Average payload sizes roughly match expectations from `docs/soc_api_notes.md`.

## Follow-ups
- Promote the normalizer used here into the ingest job so course/section hashing stays consistent.
- Emit structured metrics (`added`, `removed`, `updated`, timings) to a log sink so production runs can be compared against this baseline.
