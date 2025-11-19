# ST-20251113-act-009-02-subscribe-endpoints

## Confirmed Facts
- Backend now exposes POST /api/subscribe and POST /api/unsubscribe (api/src/routes/subscriptions.ts) with Zod schemas enforcing term/campus/index inputs, channel-specific contact validation, and optional preference/client/discord blocks; both handlers emit trace-aware error envelopes.
- Subscribe flow normalizes contacts (email regex + lowercase, Discord snowflake), checks terms/campuses exist, resolves sections when available, rejects cross-campus conflicts, and deduplicates by (contact_hash, contact_type, term, campus, index) returning existing=true when a prior row is reused.
- Per-IP (10 attempts / 10 minutes), per-contact (max 3 active), and per-section (max 50 active) throttles guard abuse; violations return ate_limited errors.
- Successful creates insert into subscriptions with merged preferences, client/discord metadata, and audit a subscription_events:created row; statuses default to pending (except discord_channel -> ctive), unresolved sections return sectionResolved=false, and responses surface unsubscribe tokens + trace IDs.
- /api/unsubscribe accepts subscription id or unsubscribe token (plus optional contact check), is idempotent, flips status to unsubscribed, clears contact_value, and appends a subscription_events:unsubscribed entry.
- Fastify server registers the new router under /api so these endpoints ship with the existing app container (api/src/server.ts).
- Integration tests cover creation/event logging, duplicate reuse, per-contact throttle, unresolved sections, campus conflicts, unsubscribe redaction, and invalid contact handling using temp SQLite fixtures (api/tests/subscriptions.test.ts).

## Interface / Behavior Changes
- API surface now includes /api/subscribe responses containing subscriptionId/status/requiresVerification/existing/unsubscribeToken/sectionResolved/preferences/traceId and /api/unsubscribe responses { subscriptionId, status: "unsubscribed", previousStatus, traceId }.
- Error payloads for these routes follow { error: { code, message, traceId, details? } }, and success replies set the x-trace-id header for downstream correlation.

## Tests & Status
- 
px tsx --test api/tests/*.test.ts — **failed** to execute because the bundled etter-sqlite3 module targets NODE_MODULE_VERSION 127 while the current Node runtime expects 137; reinstall/rebuild deps before rerunning.

## Risks / TODOs
- Subscription endpoints remain unverified in this workspace until the native module mismatch is resolved and the test suite reruns cleanly.
- IP rate-limit history is in-memory per process; scaling horizontally will require a shared store or upstream rate limiting (unchanged from this implementation).
