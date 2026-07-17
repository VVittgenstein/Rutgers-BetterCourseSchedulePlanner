# P7.1-010 — Windows local runtime, persistence, history, and configurable clocks

- Task: `P7.1-010`
- Parent: `3466a2644ef5659c272c6a0e49281f70b1f38cb1` (`P7.1-009`)
- Branch: `codex/p7-implementation`
- Next task after PostPush: `P7.1-011`

## Product result

The Windows-local composition root is now an executable-only runtime: no Node.js or npm process is required. It creates `data/rbcsp.sqlite` relative to `RBCSP.exe`, independent of the launch CWD, and fails before networking when the package data directory is not safely writable.

One package-local database host owns the single physical SQLite file. Operational tables and local-only personal migrations remain separate logical domains. First launch creates schema without bundled Catalog or Open rows. Settings, current filters, selection, notification history, migrations, restart behavior, WAL checkpoint, and the personal-data reset allowlist are durable; active watches are connection-bound and never restored.

The runtime provides a random IPv4-loopback HTTP/WS host with exact Host, Origin, and session-nonce checks, bounded bodies, security headers, a single-instance lease, authenticated UI exit, Windows close signals, and ordered watch/server/database shutdown. Shared watch reducer events now persist local episode summaries and actions without persisting connections or active watches. Startup failures show an actionable, path-safe message and random trace ID.

Catalog refresh is configurable from 1–1440 minutes and Open refresh from 3–3600 seconds. The shared runtime reads persisted settings dynamically and projects local run/day counters. Actual Rutgers scheduling and the Open-reconcile-to-watch publisher remain an explicit `P7.1-015` integration-gate responsibility; this task does not claim that end-to-end flow early.

## Verification

- `cargo fmt --all -- --check`: PASS
- `cargo test --workspace --locked --offline`: PASS (the pre-existing opt-in recorded-evidence test remains ignored)
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: PASS
- Rust architecture graph: PASS (`15` members, both binaries, `18/18` public source denies)
- `cargo deny check advisories bans licenses sources`: PASS
- `git diff --check`: PASS

The exact 37-path commit allowlist is recorded in the companion JSON. It contains only normal project source, tests, migrations, assets, lock/graph updates, and these task records. No database, real course/Open data, credential, `.secrets/`, chat log, cache, binary, or protected unrelated workspace path is eligible.
