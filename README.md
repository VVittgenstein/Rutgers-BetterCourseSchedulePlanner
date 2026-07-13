# Rutgers Better Course Schedule Planner

RBCSP is a course-centered Rutgers search, filtering, section-status, and live-watch application. The repository is currently in the P7 implementation phase; no end-user release candidate has been published yet.

## Delivery model

The project has one shared implementation and two delivery targets:

- `bcsp-local`: the Windows local composition root. A later P7 task packages it as `RBCSP.exe` with package-relative runtime data created on first start.
- `bcsp-server`: the Linux public composition root. A later P7 task packages it with service-owned runtime state and public operations assets.

Both entries consume the same shared Rust domain, Catalog, query, Open, watch, API, and operational-storage boundaries. Target-specific behavior is confined to narrow local or public adapters; it is not selected with a shared runtime mode or mutually exclusive business feature.

The frontend similarly has one shared React source graph with explicit local and public composition roots. Node.js and npm are pinned build/test tools only and are not part of either final runtime package.

## Current engineering status

P7 tasks are delivered as individually validated commits. The current architecture work establishes:

- one Cargo workspace with explicit shared, local-only, public-only, and binary packages;
- independent `bcsp-local` and `bcsp-server` dependency closures;
- explicit local and public frontend entries;
- fail-closed Rust and TypeScript import/reachability guards.

Product domain behavior, storage migrations, Rutgers adapters, the complete UI, packaging, and real-world E2E validation will be implemented and accepted by their later owner tasks. Until those gates pass, this branch should be treated as development source rather than an installable release.

## Legacy migration inputs

The older Node/Fastify backend, workers, launchers, and previous frontend source remain temporarily in the repository as frozen migration inputs. They are excluded from the active P7 target graph and are not supported release or runtime entrypoints. Their valuable contracts and tests are migrated by the responsible P7 tasks before final cleanup.

## Data and security boundary

The repository and final packages must not contain credentials, `.secrets/`, personal information, prebuilt databases, SQLite sidecars, seeds, or real Rutgers Catalog/Open payloads. Runtime databases are created on first start by the target runtime; production deployment remains a separate post-P7 authorization.

## License

ISC — Copyright (c) 2026 VVittgenstein
