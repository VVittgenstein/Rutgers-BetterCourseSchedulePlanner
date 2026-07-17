# P7-REPAIR-002 - live subject hydration

- Parent task: `P7-REPAIR-001`
- Trigger: real Catalog data completed after the first UI discovery read
- Result required: `P7_REPAIR_002_PASS`

## Product repair

The shared UI now polls only its local operational discovery endpoint while a
ready/current selector has targets but no published subjects. The schema stays
stable and READY does not flicker. Once the backend atomically exposes the
latest-term subject dictionary, an untouched search form moves to a populated
target in that same latest Rutgers term.

The switch aborts any old search/detail request, clears pending focus and stale
results, and cannot overwrite a user's edited filters. Unmount cancels the
single-flight poll and timer. No browser-side Rutgers request is introduced.

## Acceptance

- latest Rutgers term selection and stable synthetic fallback are covered;
- polling, single-flight behavior, hydration, stop and cleanup are covered;
- a deferred old search is explicitly aborted and cannot backfill results;
- shared TypeScript checks and frontend tests pass.

No package build, Rutgers request, Vultr mutation, release publication, or
production mutation is authorized. Next task: `P7-REPAIR-003`.
