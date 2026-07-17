# Shared Rust Architecture Decision

Status: Accepted for P3-P5 planning; not implementation approval  
Date: 2026-07-11  
Decision owner: Current discussion line (user + Codex)  
Scope: Preflight 0C for A and B

## Context

The project must ultimately ship two products:

- **A:** a Windows-local release that a normal user starts with a BAT file and
  uses through a browser WebUI;
- **B:** a public website that users can open from desktop or mobile without
  installing anything.

A and B must share the ordinary-user experience and most business behavior.
Maintaining unrelated Node and Rust backends, or separate A/B business logic,
would recreate the historical drift between implementation, routes, UI,
configuration, and documentation.

The architecture must also support:

- Windows packaging without requiring users to install Node, npm, Rust, or
  SQLite;
- a small Linux VPS with 1 GB RAM;
- SQLite-backed course catalog search;
- centralized Rutgers ingestion and openSections polling;
- live browser-session subscriptions and audio alerts;
- clear all-and-only boundaries between local-only and public-only features;
- a credible cross-platform release and portfolio story.

## Decision

BCSP will use a **shared-code, dual-entry Rust modular monolith**.

```text
                         React/Vite WebUI
                                |
                    REST + WebSocket contract
                                |
                    Shared Rust application core
        domain / Rutgers / storage / search / realtime / HTTP
                       /                         \
              bcsp-local.exe                 bcsp-server
              Windows A                      Linux B
```

The decision means:

- one React/Vite/TypeScript ordinary-user WebUI;
- one shared Rust workspace and business core;
- two thin composition roots rather than one universal binary with many unsafe
  mode switches;
- one application process per deployment;
- environment-specific routes and capabilities assembled only by the relevant
  entry binary;
- no separate formal Node backend after migration is complete.

The current Node/Fastify implementation remains historical evidence and a
migration reference. It is not the final A/B release backend.

## Technology Direction

| Layer | Selected direction |
|---|---|
| WebUI | React + Vite + TypeScript |
| Backend language | Rust |
| Async runtime | Tokio |
| HTTP/WebSocket framework | Axum |
| Middleware | Tower / tower-http |
| Persistent storage | SQLite |
| Rust database access | SQLx |
| Public reverse proxy/TLS | Caddy |
| Linux process manager | systemd |
| Workspace shape | Cargo workspace with shared crates and two app binaries |
| B deployment | Native Linux binary; Docker optional, not required |

The exact crate count and directory tree are design outputs for P3-P5, not a
preflight requirement. The implementation should preserve clear ownership
boundaries without mechanically creating a crate for every noun.

## Shared Responsibilities

The shared Rust core should own most product behavior:

- course/section domain types and validation;
- Rutgers catalog and openSections clients/parsers;
- course catalog refresh and transactional replacement;
- SQLite schema, migrations, repositories, and search;
- open-status snapshots;
- subscription rules, including the 9-section limit;
- real-time message contracts;
- centralized polling coordination;
- shared REST/WebSocket handlers where exposure is valid for both A and B;
- health models and internal diagnostics contracts.

Rust DTOs should be the contract authority. P3/P4 should design generated or
checked TypeScript types and OpenAPI artifacts so the frontend, backend, tests,
and documentation cannot silently drift.

## Dual Composition Roots

### A: `bcsp-local.exe`

A should assemble only local-release behavior:

- bind to `127.0.0.1` only;
- use a local SQLite database;
- run the local catalog refresher and openSections poller;
- serve the embedded/bundled React UI;
- expose the local WebSocket endpoint;
- allow the local-only catalog refresh configuration;
- open the default browser through the Windows launch flow;
- keep data/config/logs under an appropriate per-user Windows directory.

The Windows release should not require Node, npm, Rust, a separate SQLite
installation, or a separately installed web server.

The target package shape is approximately:

```text
bcsp-windows-x64.zip
|- bcsp-local.exe
|- start.bat
|- README.txt
`- LICENSE
```

`start.bat` is a launcher, not the business application. It should start the
binary, wait for health, and open the browser.

### B: `bcsp-server`

B should assemble only public-server behavior:

- bind to localhost behind Caddy;
- use the server-local SQLite database;
- run one centralized catalog refresher and openSections poll coordinator;
- serve the same ordinary-user React UI;
- expose the public-safe REST and WebSocket surface;
- enforce public rate, origin, payload, and session limits;
- omit local filesystem/configuration routes from the public build;
- run as a dedicated non-root systemd service user after deployment hardening.

The target runtime is:

```text
Internet
   |
   v
Caddy :443
   |
   v
bcsp-server 127.0.0.1:8080
   |- React static assets
   |- REST API
   |- WebSocket
   |- Rutgers poller
   |- SQLite
   `- local-only health/metrics exposure policy
```

The target B package is approximately:

```text
bcsp-linux-x64.tar.gz
|- bcsp-server
|- install.sh
|- bcsp.service
|- Caddyfile
|- bcsp.env.example
|- README.md
`- SHA256SUMS
```

## Data Model

### Persistent Course Catalog

SQLite persists relatively durable catalog/search data:

- terms, subjects, courses, sections, and index numbers;
- instructors, meetings, campus/location, and relationships;
- catalog version and last successful refresh metadata.

Catalog refresh should build and validate staging data before a short atomic
switch. Users must not observe a half-refreshed catalog.

Recommended initial SQLite posture:

- WAL mode;
- foreign keys enabled;
- bounded busy timeout;
- small connection pools appropriate for a single SQLite writer;
- indexed/paginated search;
- single B application instance for v1.

### Ephemeral Open Status

Current open/closed state should primarily remain in memory rather than be
written to SQLite every second. A successful Rutgers result creates a fresh,
immutable status snapshot containing at least a sequence and observation time.

Old cached data must not masquerade as a fresh poll. A failed Rutgers request
must not repeatedly trigger audio from stale Open state.

### Active Subscriptions

Active subscriptions belong to the live browser connection, not a persistent
user account or subscription table:

- at most 9 section keys per live session;
- connection close releases the active subscription state;
- no user account is required;
- the browser may remember UI selections locally, but remembered choices are
  not active monitoring until the user starts a live session.

The final section key may be an index number only if P1/P2 evidence confirms
the required uniqueness. Otherwise it must include term/campus or other
necessary context.

## Polling and Real-Time Delivery

B must have one centralized coordinator, not one Rutgers poller per browser.
Polling must be single-flight per relevant upstream dataset so slow requests do
not overlap. The coordinator should support timeout, retry/backoff for 429/5xx,
and explicit freshness/health reporting.

With active watches, the target cadence is approximately one second, subject to
Rutgers behavior and responsible rate-limit evidence. With no active watches,
the poller may slow down and should perform an immediate poll when the first
session starts watching.

WebSocket is selected over SSE for B v1 because the same live session carries:

- start/stop watching;
- watched-section replacement;
- heartbeat/ping-pong and liveness;
- status/freshness messages;
- reconnect behavior while the page remains active.

The delivery path should prefer a latest-value or lag-aware design. A slow
browser must not accumulate and later play a long queue of stale one-second
snapshots.

For each fresh status snapshot, a session checks its at-most-9 watched sections.
If a watched section is Open, the server sends the status message. The browser
then applies the agreed audio rule. This is deliberately not limited to a
Closed-to-Open transition.

## Capacity Assumption

The initial B target is approximately 50 concurrent active users and at most
450 watched section entries. That fan-out and WebSocket count are small for a
native Rust process. The provisioned 1 vCPU / 1 GB Vultr server is accepted as a
starting hypothesis because:

- Rust/Axum and Caddy have a modest idle footprint;
- current status stays compact and in memory;
- SQLite reads dominate and writes are infrequent;
- the server does not build Rust/React artifacts;
- active subscriptions are not persisted;
- the fan-out calculation is bounded.

P7 must still prove this with load tests. At minimum, tests should cover:

- 50 simultaneous WebSockets;
- 9 sections per session;
- one-second fresh status delivery;
- simultaneous catalog search and catalog refresh;
- upstream slowdown/failure;
- reconnect storms;
- bounded memory and logs over a multi-hour run.

The plan must define a measurable resize trigger rather than assuming 1 GB will
remain sufficient forever.

## Explicitly Outside v1

- email/SMTP/SendGrid reminders;
- user accounts and persistent server-side subscriptions;
- Redis, PostgreSQL, Kafka, or NATS;
- microservices and Kubernetes;
- mandatory Docker deployment;
- multiple B application replicas;
- browser-direct Rutgers polling;
- per-user server pollers;
- Tauri/Electron solely to package the local WebUI;
- one universal binary that can accidentally expose A-only routes in B.

## Safety and All-and-Only Boundary

The two binaries exist to make the product boundary structural rather than
conventional:

- `bcsp-local.exe` registers local-only configuration/launch capabilities;
- `bcsp-server` registers only public-safe routes and operational surfaces;
- shared crates do not decide exposure through scattered `if public_mode`
  checks;
- P2's all-and-only audit remains authoritative over which historical features
  are retained, removed, hidden, or deferred.

P1/P2 may reopen this decision only if recovered evidence exposes a hard
contradiction. They should not reopen it merely because Node is faster to keep
or because a different stack is familiar.

## Phase Ownership

- Current discussion line owns this architecture decision and approval.
- P1 recovers historical A requirements without implementing this ADR.
- P2 audits the all-and-only product surface before code is ported.
- P3 designs A's concrete implementation/migration plan.
- P4 designs B's concrete implementation/deployment plan.
- P5 defines exact shared/A-only/B-only modules and resolves conflicts.
- P6 merges and returns for execution review.
- P7 implements, validates, commits, packages, and load-tests after approval.

This ADR closes the 0C content decision. It does not bypass the P1 or P6 review
gates and does not itself authorize P7.
