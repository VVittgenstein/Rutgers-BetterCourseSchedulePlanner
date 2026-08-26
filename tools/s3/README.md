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
copy is byte-identical (CE-6) or extended by one tick so it wins the
representative slot by length (CE-6b) — while the go-gate test keeps proving
a genuinely satisfying fixture still reaches GO.

Provenance semantics: a class whose members carry conflicting campus labels
— or conflicting **absolute time anchors** (the same canonical series claimed
at two different times, i.e. a time-translated copy; `timeConflict` in the
JSON) — contributes nothing to A4-1, and its member streams are **excluded
from all evidence** (fits, comparison, server-clock evidence, safe offset,
every gate); they stay listed in the descriptive tables, flagged by
`provenance.excludedStreamIds` and `bracketTotals.excludedFromEvidence`.
Genuine duplicates of one capture carry the exact recorded timestamps, so
class members must agree exactly on the absolute client time at their aligned
samples; any disagreement means someone translated the series, and no
deterministic representative choice among disagreeing timelines is safe — so
nobody in the class counts. In a clean (non-conflicted) class, only the
**representative** stream (the longest member; streamId ascending on ties) is
evidence-eligible: identical or contained duplicates are excluded from all
evidence the same way and listed in `provenance.duplicateStreamIds`, so
duplicated observation data counts exactly once no matter how it is relabeled
— a capture copied under other term/target ids, or re-fed through the SQLite
path, cannot inflate the comparison n, multiply whole-target leave-out folds,
or widen campus coverage. A clean class covers its campus only when one of
its OWN evidence-eligible streams (i.e. the representative) has a window with
≥ 5 informative brackets — qualification never travels through a shared
targetId.

Provenance boundary (documented on purpose): A4-1 merges observation series
that are identical or contiguous slices of one another after removing
metadata; derived series (subsampling, interleaving, edited deltas) are NOT
detected — the gate defends against copy-and-relabel and copy-and-translate,
it is not forensics against de novo fabrication. Byte-identical streams under
the SAME campus label and the SAME absolute times merge without conflict into
one class whose representative alone stays evidence-eligible; they cannot
widen campus coverage or add evidence.
