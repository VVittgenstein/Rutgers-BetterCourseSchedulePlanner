# P7.1-015 — Functional integration gate before visual implementation

- Task: `P7.1-015`
- Parent: `d9135f32c8f6f8c2c88b9959122f95eb78640801`
- Branch: `codex/p7-implementation`
- Next task after PostPush: `P7.2-001`

## Product result

Local and public development entries now use the same typed product routes,
Rutgers discovery/catalog/open refresh chain, persisted catalog projection,
query engine, section status, and WebSocket watch protocol. The runtime refreshes
the current discovered term automatically and loads older published targets only
when requested. Subject options are derived from the published catalog when the
selector does not provide them.

The fake-upstream integration suite exercises discovery, all eight product API
routes, search and filtering, course and section details, open-section linkage,
freshness/lag/counts, two WebSocket clients, watch admission and notification,
clean database restart, and request de-duplication. It performs no real Rutgers,
release, deployment, or production operation.

## Verification

- Rust format, workspace tests, and Clippy with warnings denied: PASS
- Frontend guard 80/80, tests 27/27, typecheck, local/public builds: PASS
- Rust dependency graph and public zero-surface checks: PASS
- Fake-upstream functional E2E and clean-restart tests: PASS
- Real-world E2E remains its separately approved later sub-phase

The task is complete only when the pushed `Public operations rehearsal`
workflow passes for the exact commit.
