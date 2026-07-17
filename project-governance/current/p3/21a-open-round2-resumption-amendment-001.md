# P3 Open Round 2 Resumption Amendment 001

## 1. Frozen authorization

- Status: `FROZEN_ROUND_2_RESUMPTION_AFTER_USER_REVIEW`
- Frozen at: `2026-07-13T12:18:04+08:00`
- Machine amendment: `21b-open-round2-resumption-amendment-001.json`
- Purpose: resume only the cancelled second round of the original frozen Open manifest after the user approved the Rutgers official merged-set/intersection contract and the two-clock rate mapping.

This amendment does not erase or rewrite the historical stop in `10b/10c`. It creates a hash-chained, fail-closed authorization for the 21 unexecuted request IDs already present in `10a`.

## 2. Hash chain

| Artifact | SHA-256 |
|---|---|
| `10a-open-request-manifest.json` | `707705756EEA4D269EDD1822F8529453B1E8C7838E23B1FC843789379B947703` |
| `10c-open-request-stop-001.json` | `141C76D6B20E4CFB31CD556B19AC3E14239BEDF5810B36E2C1F6A1B279316D4B` |
| `11-open-request-ledger.tsv` at amendment freeze | `A6FEF873ABFC511A0BADB71463B772194204015955323D8BB5007AEE28DED9CC` |
| `18a-open-review-decision-001.json` | `73975287BDD4409909EA4C030CA1B63A597CB50A67B074556059AB326CCCFB00` |
| `19-open-cache-and-notification-latency-contract.md` | `17DAC064F3AD0AC426E7C72D75F8080F16EBE1E5E5647EC13FA358048E86E66E` |

## 3. Exact authorization

- Authorized IDs: `OPEN-R2-001` through `OPEN-R2-021`, exactly as defined by `10a`.
- Additional Catalog requests: `0`.
- Additional Open request IDs: `0`.
- Remaining attempt budget: `21`; cumulative hard limit remains `42`.
- Concurrency: `1`; minimum attempt-start interval: `5s`; timeout: `30s`; automatic retries: `0`; decoded response limit: `10 MiB`.
- No cookies, authentication, cache-busting query, burst, capacity probing, or invalid-parameter requests.

The acquisition tool must verify this amendment and the full hash chain before selecting any Round 2 request. The pre-run ledger must be exactly the frozen 21-row successful Round 1 prefix. Each request is appended durably before the next request is eligible.

## 4. Terminal conditions

Immediately stop on 403, 429, any non-2xx, off-origin redirect, non-JSON response, parse failure, non-array root, response limit violation, timeout/network failure, ledger/raw/hash inconsistency, or a newly observed root value that is not a five-digit string. A legal empty array is recorded but never independently interpreted as mass Closed. Raw duplicate values are audited and normalized through set semantics; they do not create Sections.

## 5. Evidence limits

Round 2 can validate cross-round shape, body/ETag/cache observations, transition calculations, complete-batch timing, and the approved merged-set/intersection algorithm. It cannot prove Rutgers' internal publication delay, a hard 30-second end-to-end notification guarantee, or sustained 10-second production capacity. Scheduler cadence, single-flight, coalescing, missed-tick handling, partial failure, and unsafe-empty behavior remain deterministic fake-upstream tests for P7.

