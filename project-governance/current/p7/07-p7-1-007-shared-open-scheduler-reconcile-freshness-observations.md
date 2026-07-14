# P7.1-007 — Shared Open scheduler, reconcile, freshness, and observations

- Status: `COMPLETE / PASS`
- Parent: `b6f95a7777de23292da4bebf4fb801c5888406cf`
- Branch: `codex/p7-implementation`
- Verified: `2026-07-14T10:29:45.173Z`

## Product delivered

- Discovery-bound official Rutgers `openSections.json` requests with exact `(term,campus)` ownership, strict payload parsing, a 10 MiB decoded limit, no credentials, redirects, or automatic request retries.
- One shared-origin EDF scheduler with concurrency one, per-target single-flight/coalescing, 3/10/30/3600-second behavior, no catch-up bursts, deterministic backoff, Retry-After, and fatal diagnostic circuits.
- Atomic Catalog-version-bound reconcile with LKG retention, safe empty and zero-intersection handling, honest Catalog-race counts, orphan/duplicate audit, and no mass-close failure path.
- Durable attempt, target-observation, watched-Section event, counter, freshness, lag, change-time, latest-failure, circuit, and bounded-retention state without raw response bodies or course seed data.
- Complete redacted HTTP/cache audit metadata, including decoded body hash and actual cache headers; corrupt content decoding is fatal rather than transient.
- Current watch membership is sampled after the network request and immediately before commit. Active watch demand remains in memory and is not restored from SQLite.
- Shared target and Section status projections preserve stale known state, UNKNOWN, LKG age, latest failure, and exact uncertainty reasons.

## Verification

| Gate | Result |
|---|---|
| Contracts | `73 passed` |
| Rutgers client | `29 passed` |
| Operational storage | `45 passed` |
| Shared Open | `44 passed` |
| Workspace fmt/test/clippy | `PASS` |
| Architecture graph + self-test | `PASS` |
| Cargo deny advisories/bans/licenses/sources | `PASS` |
| Bounded product re-review | `PASS; no remaining blocker` |

## Boundary

This task does not implement the connection-bound watch/episode reducer, WebSocket endpoint, UI, WebAudio, packaging, deployment, release publication, or production traffic changes. It made no live Rutgers request and contains no real Rutgers course body or preinstalled database.
