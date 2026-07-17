# P3 Shared Open Final Contract

## 1. Status and evidence boundary

- Status: `FROZEN_P3_SHARED_OPEN_CONTRACT`
- Applies to: shared local/public domain and scheduler semantics
- Evidence: `10a`–`22b`, including 42/42 successful two-round observations
- Product decision: Rutgers official set membership plus the approved 3/10/30-second two-clock model
- Additional Rutgers requests authorized by this contract: `0`
- Windows local persistence amendment: `P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001` (2026-07-13), incorporated without changing shared Open semantics
- P7 authorization: `FALSE`

The P3 candidate validator passed the complete evidence, hash-chain, traceability, and contract assertions before this promotion. The final validator additionally checks the regenerated notebook/report and total gate. This frozen contract deliberately does not convert low-volume observations into a Rutgers capacity SLA or a hard real-seat-change-to-notification guarantee.

For the Windows local adapter only, every reference to persistence in this document is interpreted through the approved package-relative single-database topology: operational Open tables and personal episode/action tables are logical domains in `<package-root>/data/rbcsp.sqlite`. This amendment changes neither the OpenBatch/join/empty/error/scheduler/observation/notification contract nor the frozen evidence represented by `23b`; therefore `23b` remains unchanged. Incorporating the storage decision is not approval to begin P7.

## 2. Identity and authoritative mapping

1. External Section identity remains `(term,campus,index)`; naked `index` is never a product key.
2. `OpenBatchKey = (term,campus)`. Each initialized Catalog target has one matching Open batch and an independent last-known-good checkpoint. Multiple selected or watched campuses never form a variable cross-campus state batch.
3. The current required source set for an Open batch contains exactly one official `openSections.json?year={year}&term={term}&campus={campus}` URI matching the batch key. The approved “merge arrays then membership” algorithm means: validate every required array, union/deduplicate them inside the same fixed batch, then intersect. With the current one-source set, this reduces to one set. Adding shards or changing required sources requires a reviewed contract revision.
4. Scheduler cycles may finish partially across campuses: each complete safe OpenBatch commits independently. Sets are never unioned across campus keys for state mutation, so status cannot change merely because the user's selected/watched campus collection changes.
5. A successful source array is validated before normalization. Every value must be a five-digit string; duplicates are counted and deduplicated as set input.
6. For the batch, `open_set = union(validated_required_arrays)` and each canonical Catalog Section in that batch is Open iff `open_set.contains(section.index)`.
7. Values not present in that batch's Catalog are orphan audit data only. They never create a Section, change Section identity, or fail an otherwise valid response.
8. Catalog `section.openStatus` and `course.openSections` remain raw snapshot evidence and never override live Open state.
9. A valid batch observation updates all Sections in that target atomically. Another target's failure does not roll it back.

## 3. Complete, empty, partial, and failure semantics

| Input | Classification | State action |
|---|---|---|
| HTTP 2xx, JSON array, all values valid, nonempty | `VALID_APPLIED` | Atomic intersection; absence may produce Closed |
| HTTP 2xx, empty array, Catalog target also empty | `VALID_EMPTY_NO_ROWS` | Commit observation; no Section state rows |
| HTTP 2xx, empty array, Catalog target nonempty | `UNSAFE_EMPTY` | Keep last-known-good; never mass-close |
| Nonempty valid set, nonempty Catalog, zero intersection | `UNSAFE_ZERO_INTERSECTION` | Keep last-known-good; no automatic mass-close |
| Timeout/network/non-2xx/off-origin/non-JSON/oversize/schema/invalid value | `FAILED` | Keep last-known-good |
| Target not yet observed successfully | `UNKNOWN` | Do not infer Closed from Catalog raw fields |

There is no “two misses means Closed” rule. Whole-target zero intersection remains unsafe until a future reviewed real-evidence decision changes it. When a valid batch has at least one intersection match, any other absent Section may close on that same observation. `ETag`, repeated bodies, or elapsed time cannot promote an unsafe zero-intersection response.

An Open attempt captures `catalog_content_version`. At commit, any version drift produces `STALE_CATALOG_RACE`: do not update last-known-good or infer Closed; enqueue one normal coalesced due operation without a new immediate network request. Newly committed Catalog Sections remain UNKNOWN until a later valid Open batch uses that version.

## 4. Approved two-clock scheduler

For each Open target:

- `general_open_refresh_interval_sec`: public fixed `30`; local default `30`, user configurable `3–3600` with invalid input rejected rather than clamped.
- `active_watch_poll_target_sec`: `10` when that exact target contains at least one active watch.
- `requested_effective_interval_sec = min(general_open_refresh_interval_sec, 10)` for watched targets, otherwise the general interval.
- Local value `3` remains a real 3-second requested target; it is not silently changed to 10.
- Per-target single-flight is mandatory. Timer, first load, explicit local refresh, and watch fast lane coalesce into one due request.
- Watch/user counts do not multiply requests for the same target.
- Missed ticks coalesce or skip; there is no catch-up burst.

Catalog and Open queues share one real-origin limiter with maximum concurrency `1`, the only concurrency mode directly aligned with the serial P3 evidence. All work uses earliest absolute due time/deadline first; an active-watch Open job wins only an exact tie. After a job finishes, its next absolute due advances, so overdue general Open and Catalog jobs cannot be starved. Fake-clock tests must prove bounded progress for every lane. If the queue cannot meet a requested interval, the product records and displays scheduler lag and actual start-to-start interval; it does not mislabel the requested value as achieved.

Theoretical requested rate is `sum(1 / requested_effective_interval_sec)` across active targets, while actual rate is bounded by shared single-origin serialization and response duration. For current Fall's 15 targets, demand is 0.5 QPS at 30s, 1.1 QPS for nine distinct 10s watched targets plus six at 30s, 1.5 QPS if all fifteen are watched, and 5 QPS if all are locally set to 3s. Observed p95 with concurrency one implies about 0.666 requests/s, so saturation and visible lag are expected in the larger cases. The setting is a target cadence, not a promise under saturation. P7 must validate no starvation and honest lag with a fake upstream; it must not pressure-test Rutgers. Raising real-origin concurrency above one requires a new reviewed evidence decision.

## 5. Transport, retry, and circuit policy

- HTTPS GET to the official allowlisted origin only; no cookies/auth, cache-busting query, or off-origin redirects.
- Connect timeout: `5s`; total attempt timeout: `15s`; decoded body limit: `10 MiB`.
- Automatic immediate retries per attempt: `0`.
- Network/408/5xx, `UNSAFE_EMPTY`, and `UNSAFE_ZERO_INTERSECTION`: keep last-known-good and use backoff steps `30s, 60s, 120s, 240s, 480s, 600s` (the step, not the total delay, is capped at 600s); `retry_delay = max(requested_effective_interval, backoff[n]) + deterministic_jitter`, where jitter is fixed in `[0, 10%]` of the selected delay. Thus a local 3600s cadence still yields a delay of at least 3600s before jitter. Failure never polls faster than the requested cadence. Success resets the failure streak.
- 429: honor a valid `Retry-After`; otherwise open an origin-wide `15m` circuit. No other target bypasses that circuit.
- 403, off-origin redirect, content/schema/value/size violation: open an origin-wide fail-closed circuit and require an explicit diagnostic recheck after a minimum `60s` cooldown; no automatic loop.
- A user-configured cadence never bypasses backoff or a circuit. UI exposes requested cadence, effective due time, scheduler lag, and circuit reason.
- The product sends ordinary full GET requests. Conditional GET/304 behavior is not part of P3 because it was not observed or validated. `ETag` is stored only as response audit metadata.

## 6. Observations, timestamps, and counters

Every actual request start creates an `OpenPullAttempt`. Every valid applied response, including an unchanged body or `VALID_EMPTY_NO_ROWS`, creates a target-level `OpenRefreshObservation` and advances the target checkpoint. Reconcile then derives one section-level `OpenObservation` for each watched Section in that batch, even when its OPEN state is unchanged.

Required fields include target, attempt/observation sequence, Catalog content version, started/completed/observed timestamps, outcome/classification, HTTP/cache metadata, canonical set hash, state hash, orphan/duplicate counts, scheduler lane, requested/effective interval, lag, and last-known-good age. `body_changed` uses the canonical set/body hash; `state_changed` uses the target intersection state hash. `ETag` never marks either change and never triggers an episode or cue.

Operational retention is bounded: per target keep the current Rutgers day and the most recent 256 detailed attempts/target observations, whichever covers more; older detail rolls into daily aggregates without raw bodies. In the Windows local adapter, episode/action history remains in the `PERSONAL` logical tables of `<package-root>/data/rbcsp.sqlite` under its separate no-silent-TTL product contract; operational retention remains in the same file's `OPERATIONAL` logical tables. Public persistence remains defined by its adapter contract. Product storage never keeps raw `openSections` bodies.

Required UI/status timestamps:

- last attempt;
- last valid observation;
- last body change;
- last state change;
- latest failure and last-known-good age.

Counters are target-request counters, never Section/browser/WebSocket counters:

- `attempted`: every request start;
- `succeeded`: every valid applied response;
- `failed`: transport/HTTP/validation/unsafe-empty application failure;
- `empty`: every HTTP 2xx empty array, orthogonal to succeeded/failed.

Local exposes run and Rutgers-day totals; public exposes service-wide Rutgers-day totals. The day boundary is `America/New_York` and must be labeled. Resetting local user data does not rewrite diagnostic request history unless a separate maintenance action explicitly does so.

Freshness is computed, not claimed from Rutgers: `fresh_until = last_valid_completed + 2 * requested_effective_interval + 15s`. Any later FAILED, UNSAFE, or STALE_CATALOG_RACE attempt immediately makes the last-known value stale. With no last-known-good the state is UNKNOWN. FLT-S03 returns definite MATCH/NO_MATCH only for fresh OPEN/CLOSED; UNKNOWN or stale last-known OPEN/CLOSED returns UNCERTAIN while the UI still displays last-known value, age, and reason.

## 7. Reconcile, episodes, and audio precondition

- First successful observation initializes OPEN/CLOSED for that target; before it, state is UNKNOWN.
- A reliable `CLOSED → OPEN` transition creates a new OpenEpisode.
- If the user explicitly starts a watch while the current reliable state is already Open, create one initial OpenEpisode for that user action.
- If watch starts from UNKNOWN, the first reliable Open observation may create the initial episode once.
- A confirmed/timed-out OpenEpisode cannot cause CONTINUOUS to re-ring while the Section remains Open. A reliable Closed observation must occur before a later Open can create another transition episode. Independently, ONE_SHOT consumes every distinct valid section-level `OpenObservation` while Open and attempts one cue until Max audible is reached; it does not create a new episode for unchanged Open.
- `UNSAFE_EMPTY`, partial/failed attempts, or stale checkpoints never create Closed and never re-arm an episode.
- Only a valid section-level OpenObservation may fan out through WebSocket and feed audio logic.

ONE_SHOT and CONTINUOUS semantics, Max audible default `3` with no product upper limit, confirmation, resume, shared bounded mixer, and no persistent active watch remain as frozen in P2/P3. Watch fanout cannot precede state reconciliation.

## 8. Latency contract

`D_true = U + C + P + B + F` remains the only valid end-to-end model. Rutgers publication delay `U` is unknown; cache/representation delay `C` is not controlled by BCSP; scheduler phase/queue `P` and request/batch work `B` are observable. Server reconcile/WebSocket fanout has an engineering target of `<=1s`; browser audio-start is included only when the page is connected, foreground-capable, and AudioContext has already been unlocked. Autoplay-blocked or suspended browsers report a visible failure instead of pretending to meet audio latency.

The UI says “BCSP first observed Open,” not the actual seat-change time. The product does not promise that a real Rutgers seat change always produces a notification within 30 seconds. It must report actual observation freshness and scheduler lag so users can distinguish configured intent from observed behavior.

## 9. P7 mandatory fake-upstream tests

At minimum:

1. 3/10/30/3600 interval boundaries; invalid input rejection; watched/unwatched lane changes.
2. Same-target timer/reload/manual/watch coalescing and one in-flight request.
3. Max-nine local watched Sections across one and multiple batches; shared Catalog/Open EDF, no starvation, honest lag, and no catch-up burst.
4. Fixed per-campus OpenBatch membership; multiple selected campuses never alter another batch; future multi-array same-batch fixture merges only required sources.
5. HTTP 200 unchanged/changed/nonempty/empty; duplicate set input; orphan audit; no orphan-created Section; ETag-change/body-same and body-change/state-same.
6. Empty+empty Catalog valid; empty+nonempty Catalog unsafe; nonempty zero-intersection remains unsafe without mass-close; first-run UNKNOWN; partial/timeout/5xx/403/429/schema/oversize/off-origin.
7. Catalog content-version race, new Section UNKNOWN, local recompute/coalesced due, and no stale-version Closed.
8. Backoff never accelerates a 3600s cadence; Retry-After, origin circuit, unsafe-empty streak, recovery, and last-known-good age.
9. Every-attempt counters and every-valid target/Section observation cardinality across `America/New_York` midnight; bounded detail retention and daily rollup.
10. Fresh/stale/UNKNOWN FLT-S03 truth table with fake clock.
11. Atomic target reconcile; CLOSED→OPEN; initial already-Open watch; UNKNOWN→OPEN; failed/unsafe observations never close or re-arm.
12. ONE_SHOT cues on distinct valid Open Section observations until cap while CONTINUOUS does not re-ring within a confirmed episode.
13. WebSocket replay/idempotency; disconnect cleanup; watch count does not amplify upstream requests.
14. Observation-to-fanout/audio `<=1s` under approved conditions; browser autoplay failure remains visible and does not corrupt counters.

Passing fake-upstream tests validates BCSP behavior, not Rutgers capacity or freshness. Any implementation that requires concurrent real-origin Open requests, cache-busting, browser-direct polling, or a second Open contract must stop for Review.
