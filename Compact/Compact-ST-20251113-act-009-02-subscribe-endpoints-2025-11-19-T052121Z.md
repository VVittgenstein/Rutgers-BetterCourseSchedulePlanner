# ST-20251113-act-009-02-subscribe-endpoints (post-review)

## Confirmed Facts
- Unsubscribe lookups now prefer unsubscribeToken when present, falling back to subscriptionId only if no token is provided, so dummy tokens can’t bypass contact proof (api/src/routes/subscriptions.ts).
- Added regression coverage ensuring id+fake-token payloads are rejected (404) while valid token-only requests succeed, preventing future priority regressions (api/tests/subscriptions.test.ts).

## Tests & Status
- 
px tsx --test api/tests/subscriptions.test.ts remains blocked: etter-sqlite3 binary was compiled for NODE_MODULE_VERSION 127 but this Node runtime expects 137 (needs reinstall/rebuild before validation).

## Risks / TODOs
- Token-first logic still can’t be verified locally until etter-sqlite3 is rebuilt for the current Node version and tests can run.

## Code Review - ST-20251113-act-009-02-subscribe-endpoints - 2025-11-19T052147Z

---review-start---
api/src/routes/subscriptions.ts
Comment on lines +92 to 95


  .refine((value) => Boolean(value.unsubscribeToken || (value.subscriptionId && value.contactValue)), {
    message: 'Provide unsubscribeToken or subscriptionId with matching contactValue',
    path: ['subscriptionId'],
  });


@chatgpt-codex-connector
chatgpt-codex-connector bot

P1 Badge Unsubscribe contact proof bypassed by dummy token

The new refine rule treats any non-empty unsubscribeToken as sufficient, so a payload that includes subscriptionId and a random 16+ char token but omits contactValue still passes validation. The handler then calls findSubscriptionForUnsubscribe which prioritizes subscriptionId over the token (lines 672‑683), meaning the token is ignored and the contact hash check is skipped. An attacker who knows or guesses an ID can still cancel another user’s subscription without the secret token or matching contact by simply adding a placeholder token to the request.
---review-end---
