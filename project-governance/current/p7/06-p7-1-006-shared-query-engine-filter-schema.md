# P7.1-006 — Shared query engine, FilterSchema, and 22 filters

- Status: `COMPLETE / PASS`
- Parent: `0890396edc8532ede71d066388aa80a64dfddffa`
- Branch: `codex/p7-implementation`
- Verified: `2026-07-14T08:56:00.544Z`

## Product delivered

- One versioned 22-row `FilterSchema` and strict normalized request contracts.
- Course-centered search plus independent Section search and direct Course/Section detail.
- All ten course/context and twelve Section predicates with `MATCH / NO_MATCH / UNCERTAIN` explanations.
- Same-variant and same-Section witnesses, including same-occurrence building/room matching.
- Exact credit ranges, multi-window availability, orthogonal modality/synchronicity, structured eligibility, permission, exam, instructor, and injected live-Open evidence.
- SQLite FTS5 token-AND search bound to exact target content versions; no `LIKE` or empty-result fallback.
- Filter-before-total/page behavior, exact course-identifier priority, deterministic sorting, and stable pagination.
- Self-contained UI responses: Section search includes its course variant, occurrences, ten course-filter results, twelve Section-filter results, and text-match evidence.

## Verification

| Gate | Result |
|---|---|
| Contracts | `56 passed` |
| Operational storage | `31 passed` |
| Query | `24 passed` |
| Application composition | `3 passed` |
| Workspace fmt/check/test/clippy | `PASS` |
| Architecture graph + self-test | `PASS` |
| Cargo deny advisories/bans/licenses/sources | `PASS` |
| Independent product review | `PASS; no remaining blocker` |

## Boundary

This task does not implement the Open scheduler/reconcile loop, watch/WebSocket behavior, UI, runtime packaging, deployment, release publication, or production changes. It contains no real Rutgers course body or preinstalled course database.
