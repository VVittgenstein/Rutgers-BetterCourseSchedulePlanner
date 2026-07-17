# P7.1-012 — Linux public runtime, ephemeral sessions, fixed clocks, and service state

- Task: `P7.1-012`
- Parent: `eb939b65ddc7ac60d98901bea7dee9e4ac39917d`
- Branch: `codex/p7-implementation`
- Next task after PostPush: `P7.1-013`

## Product result

`bcsp-server` now starts the Linux-public Axum runtime on fixed loopback `127.0.0.1:8080`, validates one HTTPS external origin, opens the fixed service database `/var/lib/bcsp/rbcsp.sqlite` before binding, and exposes liveness, truthful readiness, aggregate metrics, document bootstrap, and the shared WebSocket transport.

Every top-level document load receives a new in-memory CSPRNG nonce and browser-language default. Filters, selections, active watches, alerts, audio settings, and locale are reset for a new page and are never written to cookies, browser storage, or personal tables. The bounded nonce registry evicts the least-recently-used document instead of allowing a page-load burst to hold the service unavailable for the session TTL.

Catalog/Open requested cadence is fixed at `600/30/10` seconds and has no user input. Browser loads, navigation, queries, and status reads only consume shared service state; the host exposes no path that starts Rutgers work. Readiness distinguishes stored LKG from a running scheduler, projects Fresh/Stale/Unknown truth through the shared Open projection, and reports circuit, backoff, lag, overload, Rutgers-day service counters, active watches, and WebSocket connection count without personal labels.

Watch START now reconstructs the current committed Section observation with the same deterministic observation IDs and shared freshness rules. A later failure, unsafe result, Catalog race, expired LKG, unknown Section value, or Catalog-version mismatch never opens an initial episode.

HTTP mutations require exact Host, Origin, and live document nonce. WebSocket upgrade additionally requires the fixed `bcsp.v1` subprotocol. Responses are no-store with strict CSP and no CORS. HSTS remains intentionally absent until the P7.1-014 HTTPS/Caddy validation boundary. Shutdown stops watches and has a bounded HTTP drain.

## Deliberate integration boundary

The production composition still injects `NoPublicProductRoutes` and does not start the Catalog/Open scheduler. Therefore a fresh `bcsp-server` remains live but not ready and shared search/detail APIs return 404. P7.1-015 owns wiring the already-built shared query, scheduler, Rutgers client, and observation publisher for both entries. This task does not claim the functional integration gate early.

## Verification

- `cargo fmt --all -- --check`: PASS
- `cargo test --workspace --locked --offline`: PASS
- post-adjustment `cargo test -p bcsp-public-runtime --locked --offline`: PASS (`18/18`)
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: PASS
- post-adjustment strict public-runtime/server Clippy: PASS
- Rust architecture graph and self-test: PASS (`15` members, `18/18` public SOURCE denies)
- `cargo deny check advisories bans licenses sources`: PASS
- task diff check: PASS

No Rutgers request, real course data, database artifact, credential, `.secrets/`, release, package, Vultr, DNS, Cloudflare, certificate, or production mutation was used or added.
