# P7.4-001 — Final integration verification and release-input freeze

- Task: `P7.4-001`
- Parent: `6346fcbe56d1c02a2182299b4d89a2d8e2344aa3`
- Branch: `codex/p7-implementation`
- Release version: `0.1.0`
- Package count: exactly `2`
- Package build in this task: no

## Integrated product result

The real Vite UI is now embedded in both Rust delivery roots. The Windows local
runtime serves the local composition, API, and WebSocket from one loopback
origin. The Linux public runtime serves the public composition from the service
binary without an external web-assets directory. Release builds fail closed if
the verified frontend distribution is absent.

The frozen package contract is
`packaging/release-inputs.json`:

- `rbcsp-windows-x86_64-0.1.0.zip`, with no pre-created database or course data;
- `rbcsp-linux-x86_64-0.1.0.tar.gz`, with embedded public UI and no `share/`
  web-assets surface.

Both packages carry the approved ISC license and
`Copyright (c) 2026 VVittgenstein` through the repository `LICENSE` input.

## Verification report

| Product gate | Result |
|---|---|
| Frontend architecture guard | PASS 82/82 |
| React product/component tests | PASS 100/100 |
| TypeScript plus local/public production builds | PASS |
| Public DOM/route/i18n/bundle boundary | PASS 72/72 |
| Rust workspace all targets, with real Rutgers disabled only by exact CI switch | PASS |
| Rust format and clippy with warnings denied | PASS |
| Rust advisories, dependency bans, licenses, and sources | PASS |
| Rust workspace graph and public zero-surface guards | PASS |
| Local release composition root | PASS |
| Public release composition root | PASS |
| Real Chrome → embedded local UI/API/WebSocket/deep-link smoke | PASS 1/1 |
| Public runtime with real frontend distribution | PASS 24/24 |
| Prior post-polish browser matrix | PASS 10/10 |

The Rust suite covers unit, property, fake-upstream integration, capacity,
fresh-schema, migration, and transaction rollback behavior. The public
operations rehearsal covers install, upgrade, automatic rollback, explicit
rollback, service checks, and SQLite integrity on disposable Ubuntu. Its
PostPush run is required to succeed on this exact commit before the next task.

## Trace closure

All implementation-level P3–P5 rows consumed by P7.1–P7.3 are closed. Of the
106 P5 verification rows, 86 are closed by the integrated suites above. The 20
artifact/environment rows are intentionally owned by the package and
real-world tasks, not claimed complete here:

- `P5-V067`–`P5-V068`;
- `P5-V071`–`P5-V076`;
- `P5-V081`–`P5-V082`;
- `P5-V097`–`P5-V106`.

They are frozen to `P7.4-002` through `P7.5-005`. No GitHub Release, production
deployment, DNS, Cloudflare, certificate, or production-traffic mutation is
performed by this task.

Gate: `P7_4_RELEASE_INPUTS_FROZEN_LOCAL_PASS`.
