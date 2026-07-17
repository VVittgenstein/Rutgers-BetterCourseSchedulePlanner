# P7.2-002 — Course-centered search filters and Section flows

- Task: `P7.2-002`
- Parent: `7100851a6549cc25b671f673081afcd1f262cbfd`
- Branch: `codex/p7-implementation`
- Skills used: `$industrial-brutalist-ui`, `$design-taste-frontend`
- Next task after PostPush: `P7.2-003`

## Product result

The shared local/public UI now exposes all 22 typed FilterSchema fields, a
course-centered search with explicit variants and matching Sections, an
independent Section search, inline Course detail, and reload-safe Section detail
URLs. Course and same-Section constraints are grouped into a responsive
accordion; active filters remain visible and the groups collapse after submit so
results are immediately reachable on mobile.

Searches run only after explicit submit, abort superseded requests, preserve
typed one-based pagination, and render distinct idle, loading, validation,
error, valid-empty, and ready states. Result evidence keeps MATCH/UNCERTAIN,
same-Section witnesses, occurrences, live Open observation time, and freshness
explicit. Invalid values identify, reveal, and focus the affected filter without
issuing an API request. Direct Section links preserve modified-click browser
behavior. No fallback or preloaded course data is introduced.

The local runtime serves the same embedded SPA shell at `/sections` and safe
`/sections/:term/:campus/:index` paths while continuing to reject malformed,
encoded, extra-depth, API, and non-GET paths. A minimal same-origin History
router avoids importing unrelated platform or persistent-storage capabilities
into the public bundle.

## Verification

- Frontend guard: PASS 80/80
- Vitest product/component suite: PASS 56/56
- TypeScript and local/public production builds: PASS
- Public DOM/route/i18n/bundle boundary: PASS 72/72
- Local Rust runtime: PASS 8 unit + 12 integration + 3 single-instance tests
- Real Chrome Course/Section/detail snapshots: PASS 5/5
- Desktop/mobile horizontal overflow: PASS none

Snapshots are under `project-governance/current/p7/evidence/p7-2-002/`.
