# P7.2-004 — Local/public composition, i18n, and accessibility

- Task: `P7.2-004`
- Parent: `692db05bcc2e560352206bb1f13ac9b6e088eb83`
- Branch: `codex/p7-implementation`
- Skills used: `$industrial-brutalist-ui`, `$design-taste-frontend`
- Next task after PostPush: `P7.3-001`

## Product result

The Windows-local composition now adds Saved Views, History, and Settings to the
shared course-search and Watch experience. Saved Views support create, rename,
apply, and delete; History displays the latest 50 search summaries; Settings
persist locale, refresh cadence, notification policy, and volume. The three reset
actions retain distinct boundaries for filters, Saved Views, and all local user
data. Local personal pages remain usable when Catalog bootstrap fails, and the
Watch provider remains mounted while navigating among local routes.

The public composition continues to expose only shared search and Watch
capabilities. It starts from fresh in-memory state after reload and contains no
local routes, navigation, source modules, localized strings, or compiled bundle
markers. Shared Filter, Result, and Watch surfaces now render in both `en-US` and
`zh-CN`.

## Verification

- Frontend guard: PASS 82/82
- Vitest product/component suite: PASS 89/89, including live Settings-to-Watch volume sync
- TypeScript and local/public production builds: PASS
- Public DOM/route/i18n/bundle boundary: PASS 72/72
- Focused Windows route and public Rust boundary checks: PASS
- Real-Chrome target × locale × viewport matrix: PASS 8/8
- axe, keyboard navigation, contrast, and horizontal overflow: PASS
- Public reload returns to defaults: PASS

The eight browser snapshots are under
`project-governance/current/p7/evidence/p7-2-004/`.

Gate: `P7_2_INTEGRATED_VISUAL_PASS`
