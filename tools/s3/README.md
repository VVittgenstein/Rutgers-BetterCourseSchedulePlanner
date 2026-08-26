# S3 Offline Rebuild-Cadence Analyzer

Evidence-only tooling for Stage 5 / S3: it re-analyzes previously captured
openSections observation data to characterize the origin's rebuild cadence
(30 s vs 60 s grid hypotheses) and evaluates the A4 GO gate. It implements **no
production feature** (no GridAnchor, no RebuildProfile); its only outputs are
the two committed evidence documents under `docs/evidence/`.

- **Zero dependencies**: `node:` builtins only (`node:fs`, `node:sqlite`,
  `node:crypto`, …). Node ≥ 24 (uses `node:sqlite` `DatabaseSync`).
- **Strictly offline**: the tool performs no network access of any kind. Raw
  sample data lives in the gitignored `data/` root and is never copied into the
  repository; outputs carry only basenames, hashes, and counts.

## Usage

```
node tools/s3/rebuild-profile-analyzer.mjs \
  --ndjson <DATA_ROOT>/<runDir>          # repeatable; run dir or samples.ndjson path \
  --sqlite <path/to/db.sqlite>           # repeatable; open_batch_observations source \
  --sqlite-target <target_id>            # optional filter for SQLite targets \
  --out-json docs/evidence/S3-REBUILD-PROFILE.json \
  --out-md   docs/evidence/S3-REBUILD-PROFILE.md
```

`<DATA_ROOT>` is `data/open-sections-repro` under the repo root — local,
gitignored, never committed (only the derived evidence documents under
`docs/evidence/` are committed). Exit codes: `0`
analysis completed (any verdict), `1` usage error, `2` fail-closed structural
error (`E_INPUT_MISSING`, `E_NDJSON_PARSE`, `E_SCHEMA_VERSION`,
`E_MISSING_FIELD`, `E_SEQUENCE_ORDER`, `E_TIME_PARSE`, `E_TIME_REGRESSION`,
`E_SQLITE_SCHEMA`, `E_INPUT_MUTATED`, `E_INTERNAL`). On exit 2 **no output
files are written**.

## Input formats

- **NDJSON**: `samples.ndjson` (schemaVersion 1 rows) plus `run.json` in the
  same directory. `run.json`'s `uri` query (`year`/`term`/`campus`) names the
  target (`soc:<year>:<term>:<campus>`); a missing `run.json` degrades the
  target to `unknown:<input>` and can never satisfy the multi-target gate.
  Rows with non-zero `curlExitCode`, non-2xx `httpStatus`, or non-empty
  `validationErrors` are excluded and counted, never silently dropped.
- **SQLite**: table `open_batch_observations` (see
  `crates/bcsp-operational-storage/migrations/0002_operational_open.sql`),
  opened read-only via an `immutable=1` `file:` URI (verified working on
  node 24). If the URI open fails the tool falls back to a plain
  `readOnly: true` open and records `"openMode": "readonly"` in
  `inputs[]` (the Markdown report explains the caveat). Input files are
  sha256-fingerprinted before analysis and re-verified after; a mismatch is
  fail-closed (`E_INPUT_MUTATED`).

## Bracket semantics

Change detection compares adjacent `decodedBodySha256` values over included
rows (NDJSON) or uses `body_changed` flags with the per-target initial LKG row
excluded (SQLite). **etag is never consulted** — the webfarm serves the same
body under multiple etags.

Each change yields a half-open bracket `(lower, upper]`:

- **Client clock**: `(stable.requestStartedUtc, changed.requestEndedUtc]` —
  the conservative outer envelope (SQLite has a single `observed_at` used for
  both ends).
- **Server clock**: the `Date` header has 1 s precision (truncated). The
  server generated the stable body at some instant ≥ its `Date` second start;
  the change happened strictly after that and at/before the changed body's
  generation instant, which is < its `Date` + 1 s. The upper bound therefore
  gets a **+1 s widening** so the true change instant is always contained.

Non-positive server widths (webfarm clock skew) drop the server bounds and are
counted (`serverNonPositiveWidth`); the client bounds remain usable. A
non-positive **client** width — a tolerated small client-clock step, a
corrupted `requestEndedUtc`, or a SQLite `observed_at` tie/regression within
the tolerance — instead rejects the **whole** bracket and is counted
(`clientNonPositiveWidth`): the pair ordering itself is suspect, so not even
the server bounds are kept, and such a bracket is never credited as
informative coverage. Client/`observed_at` timestamps must be monotone per
target within a 2 s tolerance; larger regressions are fail-closed
(`E_TIME_REGRESSION`) for NDJSON and SQLite alike. Per-bracket informative
flags and the `bracketTotals` block are computed on the **same clock the model
comparison selected** (server unless it fell back to client), so the evidence
tables can never disagree with the comparison about which brackets counted.

## Phase model: arc coverage, not histograms

Every bracket maps to an arc on the period circle `[0, P)`; because bounds are
integer ms, `(l, u]` equals the closed integer interval `[l+1, u]` (mod `P`,
wrapping). The best phase is the maximal arc-coverage region, found by an exact
event sweep over arc endpoints. A bracket whose width ≥ `P` covers the whole
circle: it is **non-informative** and is excluded from coverage counting
instead of silently inflating it.

A `timestamp % period` histogram of detection times is **invalid** for this
problem: detection timestamps confound the sampling cadence with the change
phase (the test suite contains a fixture where the histogram argmax lands 17 s
away from the true phase that the arc model recovers exactly).

## The 30 vs 60 identity and the tie rule

Every 60 s grid's ticks are a subset of the 30 s grid's ticks at the same phase
mod 30, so `maxCoverage(30) ≥ maxCoverage(60)` holds identically (asserted at
runtime). Consequences:

- Equal coverage can **never** select a winner: it only shows the 30 s grid
  adds no explanatory power — consistent with a true 60 s period but proof of
  neither model.
- Only a strict `c30 > c60` win, confirmed in **every** fold of a
  non-degenerate per-(target, window) holdout, makes the comparison
  distinguishable; a single-group half-split holdout is labeled degenerate and
  never promoted to a winner.

## Determinism

Identical input sets produce **byte-identical** outputs regardless of CLI
argument order: inputs, targets, windows, and brackets are all sorted by stable
keys, the normalized command is rebuilt from sorted input basenames, and the
outputs contain no timestamps, absolute paths, hostnames, or `sampleDirectory`
values. The JSON serializer rejects non-finite numbers; the only non-integer
values are 4-decimal coverage ratios.

## Tests

```
node --test "tools/s3/test/*.test.mjs"
```

The suite is offline and self-contained (fixtures under the OS temp dir). The
local-data test runs end-to-end against the real capture directories (local
and gitignored — never committed), independently recounting D1's brackets and
brute-forcing the arc coverage. It locates the data root in this order, first
hit wins:

1. `BCSP_OPEN_SECTIONS_REPRO_DIR` (used as-is when set; nothing else is
   probed);
2. `git rev-parse --show-toplevel` → `<top>/data/open-sections-repro` (plain
   repo-root checkout);
3. `git rev-parse --git-common-dir` → the shared main repo root's
   `data/open-sections-repro` (worktree checkouts).

When the data exists at any candidate the test always runs — a checkout
layout is never a reason to skip. Only when no candidate holds the data
(e.g. CI) does it skip, and the skip message lists exactly what was probed.

The counterexamples test file pins the adjudicated false-GO fixtures to
NO_PRODUCTION_CHANGE: the four STAGE-5-R1 root causes (relabeled provenance,
empty peak window, stray serverDate over a client-clock fallback,
single-target winner) plus the second-round shapes — a window grazing
17:00:00.000 ET at a single instant (CE-2b), an isolated peak-time sample
merged into a pre-peak session by the gap rule (CE-2c), a clean tiny
series piggybacking on an excluded duplicate's windows (CE-1b), and
duplicate-content target relabeling — one capture claimed under three terms
(CE-5) or re-fed through the SQLite path (CE-5b) — and time-translated
byte-copies of one capture shifted into the peak hour, whether the shifted
copy is byte-identical (CE-6), extended by one tick so it wins the
representative slot by length (CE-6b), shifted in the client column only
(CE-7), or nudged by a single millisecond under three relabels (CE-7b) — and
serverDate deletions on a copy: relabeled copies that each delete one
serverDate field (CE-8), and a client-shifted peak copy that also deletes one
serverDate (CE-8b) — and the STAGE-5-R2 shapes: three equal-length,
staggered, overlapping slices of one capture relabeled NB/NK/CM (CE-9), one
capture plus two different-stride decimations of it (CE-10), a peak session
claimed only by extending `requestEndedUtc` while every body, request start
and `Date` stays off-peak (CE-11, and CE-11b at the minimal 150 s edit), a
copy that edited only `requestEndedUtc` and so won the representative slot
(CE-12), and a client clock running an hour slow that labeled a wholly in-peak
server window off-peak (CE-13) — while the go-gate test and the R2 honest
control keep proving a genuinely satisfying fixture still reaches GO.

Provenance semantics: streams are grouped into **families** — one family per
body of captured observation data — and a family whose members carry
conflicting campus labels, or conflicting **records** (`timeConflict` in the
JSON: the same captured data claimed at two different times, or carrying
edited/deleted `Date` headers or request ends), contributes nothing to A4-1 and
its member streams are **excluded from all evidence** (fits, comparison,
server-clock evidence, safe offset, every gate); they stay listed in the
descriptive tables, flagged by `provenance.excludedStreamIds` and
`bracketTotals.excludedFromEvidence`.

Two streams join the same family under the first of four relations that holds
(reported per member as `relation`, with `relatedTo` and `matchedCount`):

| relation | holds when | catches |
|---|---|---|
| `identical` | the canonical series are byte-equal | whole-series copies |
| `contained` | the shorter series is a contiguous slice of the longer | truncated copies |
| `overlapping` | they share a contiguous block of ≥ 4 interior canonical entries containing ≥ 1 body change | staggered re-slicing of one capture |
| `derived` | they reuse ≥ 3 observation **records** at the same client-clock instants, **or ≥ 6 records under one single constant client-clock offset**, matched jointly on `(clientStartMs + offset, bodySha)` | subsampling, decimation, thinning, reordering — including a copy whose whole client clock was translated |

Family membership is closed **transitively** (union-find): if A relates to B
and B to C, all three are one family even when A and C do not match directly —
the case two differently-strided decimations of one capture hit.

The canonical series (and so the fingerprint) is the observation CONTENT alone
— the bodySha sequence plus the client-clock delta structure. Absolute times,
the whole serverDate column (values and missing-pattern alike) and the client
request END are deliberately NOT part of the merge key, so a copy whose clocks
were translated (either column, jointly, by hours or by a single millisecond),
whose serverDates were edited or deleted (one field or all), or whose
`requestEndedUtc` alone was rewritten still merges into the genuine capture's
family instead of minting a fresh fingerprint.

Genuine duplicates of one capture carry the exact recorded fields, so family
members must agree exactly on the absolute client start time, on every recorded
serverDate, AND on every recorded client request end — value and presence — at
their aligned samples. The request-end comparison is suppressed only when a
stream records no request end at all (`clientEndObserved: false`, i.e. every
SQLite stream, whose ingestion has no such column), which is what keeps honest
cross-format duplicates clean. Any disagreement on **any** pair in the family —
not merely the ones on the reported attachment tree; see `timeConflictPairs` —
means someone edited the series, and no deterministic representative choice
among disagreeing records is safe, so nobody in the family counts.

In a clean (non-conflicted) family, only the **representative** stream (the
longest member; streamId ascending on ties) is evidence-eligible: identical,
contained, overlapping and derived members alike are excluded from all evidence
and listed in `provenance.duplicateStreamIds`, so captured observation data
counts exactly once no matter how it is relabeled, re-sliced, or thinned — a
capture copied under other term/target ids, re-fed through the SQLite path, cut
into overlapping windows, or decimated cannot inflate the comparison n,
multiply whole-target leave-out folds, or widen campus coverage. A clean family
covers its campus only when one of its OWN evidence-eligible streams (i.e. the
representative) has a window with ≥ 5 informative brackets — qualification
never travels through a shared targetId.

Provenance boundary (documented on purpose): what A4-1 detects is **reuse of
the same observation records, up to ONE constant translation of the whole
client clock**. Whole-series copies, truncations, overlapping re-slices, and
regular or irregular subsampling/decimation/thinning of one capture all merge,
as do copies that touch only the clocks — a translation of either column, an
edited or deleted serverDate (one or all), an extended request end — which
merge and are then voided by the record conflict.

Invariance boundary of the record match, stated exactly: derived-record
detection is invariant under **one single constant translation of the client
clock applied to the whole stream** (and, being body/time based, under any
re-cadencing: stride, decimation, thinning, reordering). It is **not**
invariant under per-sample jitter, per-segment offsets, or any non-constant
time edit — the offsets then fail to coincide, no bucket reaches the
threshold, and the streams stay independent families. Nothing about that is
approximate: the offset match is an exact integer equality, never a tolerance
window, so the rule cannot drift into the jitter case. Fully disjoint
partitions and de-novo fabrication remain outside the model, exactly as before.
Crucially, making the RELATION shift-invariant is what lets the record
agreement check stay **absolute at 0 ms**: a translated copy now joins the
family and is immediately flagged `timeConflict`, which bars every member from
all evidence.

What is NOT detected: **de novo fabrication** and **per-sample time edits**. A
series whose body content or whose time grid was invented from scratch is
independent data as far as this tool can tell, and neither is a copy whose
timestamps were nudged sample by sample (or segment by segment) rather than by
one constant offset: with a different offset per record no single offset
explains more than a coincidence's worth of them. No cryptographic capture
proof, signature, online collection, or trusted-hardware attestation is
attempted. There is one further
residual, stated rather than hidden: a partition of one capture into chunks
that share no observation record and no `(bodySha, clientDelta)` block with
each other reuses nothing — every observation is still counted exactly once,
and the disjoint chunks cover disjoint wall-clock time — so the remaining fraud
there is the campus label alone, which is the de-novo boundary above.

The thresholds (4 interior entries with ≥ 1 body change; 3 reused records at
offset 0; 6 reused records at any non-zero offset) are calibrated on the real
capture data, not guessed: inside the 554-sample local capture the longest
repeated `(bodySha, clientDelta)` block at any non-zero self-offset is 1, and
the three genuinely independent local captures share **zero** observation
records and zero body hashes with one another (re-measured by the local-data
test on every run).

The **shifted** record threshold is higher than the offset-0 one for a reason,
and it is argued twice over. The absolute rule tests exactly one offset; the
shift-invariant rule ranges over every offset that any same-body pair produces
— 7362 distinct non-zero offsets inside that one 554-sample capture, from 8202
candidate pairs — so the accidental ceiling has to be re-measured rather than
inherited. It was: the largest set of records a single **non-zero** constant
offset can align inside that capture is **3** (at ±13405 ms, ±27127 ms and
±28068 ms), and none reaches 6; between the three local captures there is no
candidate offset at all, because they share no body hash. 6 is twice the
measured ceiling, and the local-data test recomputes it on every run so a
future data set cannot quietly invalidate it. The higher threshold also costs
nothing: a window or a session needs ≥ 5 informative brackets to qualify, and 5
brackets need ≥ 6 samples, so a reuse of ≤ 5 records could never have qualified
anything anyway. Two rules deliberately NOT
adopted: a bodySha-only match (a real capture holds one body for minutes at a
time, and two honest captures of one target legitimately see the same bodies),
and a timestamp-only match (honest campus captures overlap in wall-clock time
by construction).

Byte-identical streams under the SAME campus label and the SAME absolute
times merge without conflict into one family whose representative alone stays
evidence-eligible; they cannot widen campus coverage or add evidence.

## A4-2: peak and off-peak are independent server-clock evidence

Both sides of A4-2 are decided **per bracket on the comparison clock** — the
same bounds the model comparison, the safe offset and A4-5 consume, which is
the server clock whenever a production conclusion is reachable — and the
brackets are grouped into **evidence sessions**, not client windows.

An evidence session is a maximal group of one stream's evidence windows that
are contiguous on the client timeline **or** on the server timeline, under the
same `max(10 min, 5 x intervalSeconds)` gap rule the client windowing uses.
Independence must hold on **both** clocks. Client windows are cut on
client-clock gaps alone, so jumping the client clock forward mid-session used
to split ONE server-contiguous session into two "independent" windows — the
first supplying the off-peak side, the second the peak side, every bound
genuinely server-derived and the claim of independence the only forgery.
Because a production GO already rests on the server clock, a claim of two
independent sessions has to rest on it too. The mirror hole is closed by the
same rule: an edited serverDate inside one client window still leaves one
session, because the client window itself is the link.

Sessions are unions of **whole** windows, so the session partition is always a
coarsening of the window partition and A4-2 is uniformly at least as strict as
the window rule it replaced — nothing that failed before can pass now. Missing
`Date` headers resolve toward merging (fail-closed: a deleted header cannot
manufacture a split, and samples before the first observed `Date` impose no
grouping at all, while their brackets stay unplaceable and still void their
session's purity). A stream with no `Date` anywhere falls back to exactly its
client windows and cannot reach GO regardless, because A4-5 fails closed on a
client-clock fallback. The grouping is reported in `evidenceSessions` (JSON)
and in the `### Evidence sessions (server timeline)` table (Markdown).

A session qualifies on the peak side when ≥ 5 of its informative brackets have
comparison-clock bounds intersecting 17:00–18:00 America/New_York with
**positive measure** (a single-instant boundary touch is not peak evidence).
It qualifies on the off-peak side when ≥ 5 of its informative brackets fall
outside the peak hour **and every one of its brackets does** — informative or
not. That purity clause is what keeps the per-bracket rule from being weaker
than the window-label rule it replaced: without it a single session straddling
17:00 ET would satisfy both sides on its own, its early brackets off-peak and
its late ones in peak, and "multiple independent time windows" would reduce to
one ten-minute capture. Carried by the session rather than the window, the same
clause is now what refuses the client-clock-jump shape too — the two are one
theorem. Purity is checked over *all* of a session's brackets so that a
straddling session cannot buy it by making its peak-side brackets too wide to
be informative, or by dropping their `Date` headers. The peak side needs no
mirror clause — brackets whose own comparison-clock bounds lie in the peak hour
are peak evidence wherever the rest of their session sits — and the two
qualifying sets are therefore disjoint by construction: the peak and off-peak
evidence always come from **different sessions**. Any session with enough
off-peak brackets that the purity clause refuses is reported in the gate
evidence, so a reader never has to guess why it did not count.

The server upper bound carries its usual +1 s `Date` widening before the test,
which can only move a bracket toward the peak side — the conservative
direction, since the widened upper really is the true upper bound on the change
instant. A bracket without usable bounds on the comparison clock qualifies
neither side (fail closed, never filled in from the client envelope), is
counted in the gate evidence, and also costs its session off-peak purity.

The `peak 17–18 ET? (label)` column in the Markdown report and
`targets[].windows[].peakOverlap` in the JSON are the window's **client
envelope** label, and the `windowId` itself is a client-side display label. They are description only and are never gate evidence: a
capture can extend `requestEndedUtc` to drag that label across 17:00 ET, or run
its client clock an hour slow to drag it off, without touching a single body,
request start, or `Date` header.
