# ST-20251113-act-009-02-subscribe-endpoints (update)

## Confirmed Facts
- esolveClientIp now skips client-provided IPs entirely: the subscribe handler passes only Fastify's equest.ip plus x-forwarded-for, and the builder stores that resolved value in metadata so per-IP throttling can’t be bypassed via payload spoofing (api/src/routes/subscriptions.ts).
- /api/unsubscribe enforces unsubscribeToken or (subscriptionId + contactValue) at validation time; the handler still cross-checks normalized contacts and the happy-path test now supplies the contact value explicitly (api/src/routes/subscriptions.ts, api/tests/subscriptions.test.ts).
- Added a regression test that ensures ID-only unsubscribe attempts fail with VALIDATION_FAILED, guarding against future gaps (api/tests/subscriptions.test.ts).

## Tests & Status
- 
px tsx --test api/tests/subscriptions.test.ts — still blocked by the existing etter-sqlite3 NODE_MODULE_VERSION mismatch (module built for 127, runtime expects 137).

## Risks / TODOs
- All new auth/abuse protections remain unvalidated locally until etter-sqlite3 is rebuilt for the current Node version and the suite can run.
- Multi-instance rate limiting is still in-memory per process; shared-state enforcement is out of scope for this change.

## Code Review - ST-20251113-act-009-02-subscribe-endpoints - 2025-11-19T050826Z

---review-start---
#1
api/src/routes/subscriptions.ts
Comment on lines +390 to +394


function resolveClientIp(bodyIp: string | undefined, requestIp: string, forwarded?: string | string[]) {
  if (bodyIp) {
    return bodyIp;
  }
  if (typeof forwarded === 'string') {


@chatgpt-codex-connector
chatgpt-codex-connector bot

P1 Badge Rate limit trusts client-provided IP address

The per-IP throttle can be bypassed because resolveClientIp prefers the clientContext.ip value supplied in the request body before using x-forwarded-for or the actual request.ip, and the subscribe handler feeds that value into allowIpAttempt. A malicious client can set clientContext.ip to a new random string on each POST, resetting the counter and effectively disabling the IP_MAX_ATTEMPTS guard for abusive traffic.

#2
api/src/routes/subscriptions.ts
Comment on lines +88 to +92


    unsubscribeToken: z.string().trim().min(16).optional(),
    contactValue: z.string().trim().min(3).max(256).optional(),
    reason: z.string().trim().min(3).max(64).optional(),
  })
  .refine((value) => Boolean(value.subscriptionId || value.unsubscribeToken), {


@chatgpt-codex-connector
chatgpt-codex-connector bot

P1 Badge Unsubscribe allows unauthenticated cancellation by ID

The unsubscribe payload accepts a bare subscriptionId without requiring the secret unsubscribeToken, and the handler proceeds to cancel the row if it exists. Because subscription IDs are sequential, any caller can guess an ID and force another user’s subscription into unsubscribed status without owning the token or proving contact ownership, contrary to the documented expectation that anonymous flows use the token for authorization.
---review-end---
