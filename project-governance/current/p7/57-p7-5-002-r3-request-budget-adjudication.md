# P7.5-002-R3 - request-budget adjudication

- Parent: `47f378a557f3e8fb3c3548761150a679dfcff101`
- Adjudicated: `2026-07-15T14:34:55.2833070Z`
- Candidate: `eabdc3b5f4a705d8c22e6941831f55e0bb5b5c2a1c33e648e545f86007cab577`
- Candidate rerun: `false`

The fixed `Open <= N+3` and total `<= 2N+5` limits were test-protocol
guardrails, not Rutgers limits or product requirements. Applying them as a
product acceptance criterion conflicted with the approved live-data behavior:
approximately 30-second Open refresh while a page has real-time demand and
approximately 10-second refresh while a watch is active. The prior request-
budget failure and candidate-retirement decisions are therefore superseded.

The observations in records 56/56a remain valid: Chrome completed every
mandatory real Windows product flow against the exact candidate; the ledger
contained 2 discovery, 15 Catalog, and 23 Open requests over 454.187 seconds;
there was no Rutgers 403 or 429; graceful shutdown, SQLite integrity, cleanup,
and unchanged archive hash all passed. The repeated demanded-target Open pulls
were the configured normal refresh cadence, not browser amplification.

Future live tiers retain the request ledger and 480-second observation window,
but absolute `N+3`/`2N+5` totals are observational rather than pass/fail gates.
Acceptance instead verifies configured cadence, one centralized server poller
across browsers, absence of client-amplified Rutgers traffic, and immediate
stop on 403/429, off-origin redirect, schema anomaly, or unsafe runaway.

No source change, candidate rebuild, or Windows rerun is required.

Gate: `P7_5_WINDOWS_REAL_WORLD_PASS_R3`.
PostPush marker: `P7_5_002_R3_ADJUDICATED_PASS_POST_PUSH`.
Next task: `P7.5-003`.
