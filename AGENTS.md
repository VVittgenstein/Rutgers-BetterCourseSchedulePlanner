# RBCSP repository guide

This file is a navigation entry point. It does not change the collaboration
roles, product decisions, or release authorization recorded in
[CURRENT.md](docs/orchestration/CURRENT.md). Current user instructions take precedence.

## Read progressively

1. Read [README.md](README.md) for products and development commands.
2. Find the affected area in the table below, then read its implementation and
   nearest behavior tests. Trace callers before changing an exported API.
3. Consult CURRENT.md for product decisions and active work. When recovering an
   orchestration session, follow its recovery protocol, including reading it fully.
4. Open an individual design or task document when it explains the affected
   invariant. Conversation archives are historical evidence, not default context.

The [2026-09-07 architecture audit](docs/design/2026-09-07-agent-friendly-architecture-audit.md)
contains baseline evidence, refactoring stages, and the ARCH-001 implementation
record. Check the recorded status before treating a proposed stage as implemented.

## Where to work

| Change | Implementation entry | Closest verification |
|---|---|---|
| Wire shape / input validation | `crates/bcsp-contracts/src/` and `frontend/src/ui/shared/product/contracts/` | Rust contract, malformed-wire and golden tests; frontend product-client/contracts tests |
| Filter meaning / matching | `crates/bcsp-query/src/predicates.rs`, `evaluation.rs`, `engine.rs`; `bcsp-domain` | `crates/bcsp-query/tests/query_engine.rs`; domain truth tables |
| Rutgers decoding / normalization | `bcsp-rutgers-client`, `bcsp-catalog` | Their catalog, normalization, projection and rederive tests |
| Query serving / refresh | `bcsp-application/src/product_routes.rs`, `query_service.rs`, `prepared_serving.rs`, `refresh_runtime.rs` | Application product-route, query, publication and refresh tests |
| Operational database | `bcsp-operational-storage` | Storage, migration, rollback and WAL tests |
| Open snapshot integrity | `bcsp-open/src/gate.rs`, `service.rs`, `projection.rs` | Gate replay and Open service/projection tests |
| Watch lifecycle / transport | `bcsp-watch`; `bcsp-application/src/watch_socket.rs` | Watch admission/acceptance and socket tests |
| Local desired state / lifecycle | `bcsp-local-user-state`; `bcsp-local-runtime/src/desired.rs`, `personal.rs`, `presence.rs` | Desired-watch, personal-state, local-runtime and presence tests |
| Public sessions / capacity | `bcsp-public-runtime`; `bcsp-public-operations` | Public runtime/session/capacity tests and public boundary gates |
| Search UI | `frontend/src/ui/shared/search/` | Filter, query-scope, search and course-workspace tests |
| Watch UI | `frontend/src/ui/shared/watch/`; local adapters under `ui/local/` | Live-watch, desired-watch, readiness, recovery and audio tests |
| Builds / product boundaries | `frontend/build/`, `frontend/tools/`, `tools/architecture/` | Actual guards, guard self-tests, both frontend builds |
| Packaging / deployment | `packaging/`, `deploy/public/` | Relevant package/ops verifiers; Linux host checks require Linux |
| Offline cadence research | `tools/s3/` | Its README and `node --test tools/s3/test/*.test.mjs` |

## Preserve these boundaries

- Shared frontend imports shared code; local and public compositions inject
  their capabilities. Shared/public code must not import local personal state.
- Local desired state is authoritative and durable; public watches belong to
  sessions. Preserve CAS, receipts, tombstones, and explicit disconnect semantics.
- Missing evidence stays unknown. Preserve snapshot publication barriers,
  freshness rules, complete section identities, and filter witness semantics.
- Preserve existing migration SQL bytes and IDs. Data migrations and contract
  revisions require their own compatibility work; a cleanup does not imply either.
- Test fixtures must not require a second production implementation. Keep clocks,
  transports, and storage seams where they enable observable behavior tests.

## Validate proportionately

Run the affected tests while iterating. At an implementation boundary, run
`cargo test --workspace --locked`, Clippy with the README flags, `cargo fmt --all -- --check`,
and `npm --prefix frontend run verify` as applicable. Boundary changes also require
both actual Rust architecture scripts and their self-tests under `tools/architecture/`.
Build frontend assets before tests that specifically validate embedded frontend output.

Do not use test counts or file length alone to justify deletion. Separate dead
code, weak assertions, duplicate fixtures, and necessary product boundaries.
Prefer completing one existing responsibility over adding another forwarding layer.
