# Local watch scaling without widening the public service

Date: 2026-09-02. Status: ACTIVE. Intended release: 0.1.4.

This note supersedes the fixed `9` local-capacity and 521-row response-budget
examples in `2026-08-23-desired-watch-reduced-scope.md`. Its concurrency,
generation, CAS, tombstone, and materialization rules remain in force.

## Incident

A locally rebuilt 0.1.3 executable changed the visible selection limit from 9
to 255. Selecting 40 Sections appeared to work, but monitoring did not:

- `personal_selected_sections_v1.position` still allowed only `0..8`, so the
  tenth full-snapshot write failed. The optimistic UI did not surface that
  persistence failure and continued to display 40.
- Every selection change refreshed every selected Section separately. Forty
  rapid selections could enqueue `1 + ... + 40 = 820` target-wide Open
  projections.
- Each projection held the operational SQLite mutex for roughly one to two
  seconds on the observed database. Canceling the browser request did not
  cancel work already running in `spawn_blocking`, so `/service/status`, watch
  admission, and subsequent refreshes waited behind the abandoned queue.
- Starting and periodically revalidating desired watches also projected the
  same target once per Section.

Rutgers was answering the upstream pulls successfully. This was therefore a
local capacity/persistence and request-amplification design defect, not an
unavoidable 40-watch hardware limit.

## Product boundary

- The Windows local product explicitly opts into 255 selected, desired, and
  active watches.
- The shared/default watch manager and the Linux public product remain capped
  at `MAX_ACTIVE_WATCHES = 9`.
- The watch wire keeps its existing `u8` count domain. Raising the local
  policy does not raise the shared/public admission policy.
- Shared Open batch status is a bounded read optimization, not an entitlement:
  it does not let the public product arm a tenth watch.

Tests must exercise both sides: local 255 succeeds, while an ordinary/default
manager still rejects item 10.

## Persistence and honest UI state

Personal migration 10005 rebuilds `personal_selected_sections_v1` with
`position BETWEEN 0 AND 254`, preserving existing order and rows. Published
migration files remain unchanged.

Selection persistence is a latest-wins single-flight worker. A rapid gesture
writes the first in-flight snapshot and then the newest snapshot, rather than
all intermediate prefixes. A terminal save failure restores the last
server-confirmed selection and shows `SELECTION_SAVE_FAILED`; the UI must not
continue to advertise rows the database rejected.

The local bootstrap and desired-watch decoders accept at most 255 rows. Their
public equivalents continue to use the shared limit of 9.

## One user action, one authority transaction

`PUT /api/v1/local/desired-watch/batch` accepts one local gesture of 1..255
unique Sections. All items compare against the same authority generation and
the per-Section revisions from one page snapshot.

The store validates generation, mutation-ID ownership, every revision, the
post-state desired count, and the post-state tombstone/receipt budgets before
writing an authority row. It then writes every row and one batch receipt in a
single IMMEDIATE SQLite transaction. A conflict or capacity refusal writes no
authority rows; a crash cannot reveal a prefix of a committed batch.

Personal migration 10006 adds a batch receipt ledger. One 255-Section gesture
consumes one receipt, not 255 of the existing 2,048-row budget. Single and
batch mutation IDs share one logical namespace, rotation clears both ledgers,
and Full Reset clears both in the same generation-changing transaction.

After one successful batch commit, the coordinator reconciles once. All new
watches for the same `(term, campus)` are passed to the owner together, so
admission builds that target projection once.

## One target, one Open projection

`POST /api/v1/open/batch-status` accepts 1..510 unique Section identities from
one target. The bound covers a disjoint union of 255 selected and 255 active
Sections. It refreshes and projects the target once, returns the target refresh
state plus the requested published Sections, and omits stale catalog
identities without failing their valid peers. The client treats an omitted
identity like the legacy Section endpoint's non-retryable 404.

Selection-driven telemetry waits for a 300 ms trailing edge, aborts superseded
browser reads, groups Sections by target, and sends target batches
sequentially. This changes the observed 40-click worst case from 820 full
projections to one for a single-target selection.

Desired-watch cold materialization and periodic revalidation use the same
grouped admission path. Expensive admission runs before the socket-state
mutex is acquired, so PING/heartbeat maintenance is not parked behind a
target projection.

## Explicit bounds and release gates

- WebSocket messages remain below the host's 64 KiB frame limit at 255 start
  results.
- Product HTTP request bodies remain below the 1 MiB host limit at the
  510-Section telemetry maximum.
- The largest legal local desired authority and batch mutation response are
  serialized across the full identity, JavaScript-safe numeric, policy,
  materialization, and failure domains. The authority has a 384 KiB budget;
  the larger batch response has a separate 512 KiB budget with additive
  protocol headroom. Responses are never silently truncated or paginated
  because that would lose CAS tombstones.
- Migration tests cover 9-row upgrade preservation, 255-row restart, rejection
  of item 256, coexistence with operational migrations, integrity check,
  rotation, reset, and batch receipt replay/collision.
- Browser acceptance selects and starts 40 real Sections in an isolated copy
  under `C:\Users\YZZ\Desktop\RBCSP`, verifies all 40 materialize, and checks
  that service status and the watch connection remain responsive.

Migrations 10005 and 10006 are forward-only. Once a user's database records
them, an older binary will reject the unknown migration history; release notes
must repeat that rollback constraint.
