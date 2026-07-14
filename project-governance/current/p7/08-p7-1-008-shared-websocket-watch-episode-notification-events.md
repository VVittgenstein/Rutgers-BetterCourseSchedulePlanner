# P7.1-008 — Shared WebSocket watch, episode, and notification events

- Status: `COMPLETE / PASS`
- Parent: `62a5d45dda7804b44f8aa5de787ddd5375c8a92e`
- Branch: `codex/p7-implementation`
- Verified: `2026-07-14T11:57:25.9129560Z`

## Product delivered

- Strict v1 watch commands and additive server-event contracts, schemas, and wire goldens.
- Connection-bound in-memory watch management with nine-watch capacity, per-item Catalog admission, message replay, heartbeat/disconnect/process cleanup, and no active-state restoration.
- One shared demand per Section across browser connections, committed Open observation fanout, monotonic Section watermarks, and race-safe initial snapshots.
- UNKNOWN/CLOSED/OPEN episode reduction, acknowledgement, timeout/resume, rearm, alert lifecycle, ONE_SHOT cue ordering/caps, CONTINUOUS mixing, and validated cue outcomes.
- Durable Open-to-watch handoff emits only committed Section observations and does not persist watch, episode, alert, or cue state.

## Verification

| Gate | Result |
|---|---|
| Watch contracts | `13 new; 86 package tests total` |
| Shared watch core | `18 passed` |
| Shared Open | `46 passed` |
| Operational storage | `45 passed` |
| Workspace fmt/check/test/clippy | `PASS` |
| Architecture graph + self-test | `PASS` |
| Cargo deny advisories/bans/licenses/sources | `PASS` |
| Product review | `PASS after observation-watermark and START-admission fixes` |

## Boundary

This task does not add a runtime WebSocket listener, UI, WebAudio implementation, package, deployment, release, or production traffic change. It made no live Rutgers request, contains no real course body or preinstalled database, and persists no active user watch state.
