# S3 Rebuild Cadence Evidence — Verdict: NO_PRODUCTION_CHANGE (DATA_REQUIRED)

This offline analysis of locally captured openSections observation data (kept in the gitignored `data/` root; only these evidence documents are committed) reaches the verdict **NO_PRODUCTION_CHANGE** with qualifier **DATA_REQUIRED** from the A4 gate evaluation below. Unsatisfied gates: A4-1, A4-2, A4-3, A4-4, A4-6. All numbers are computed from interval-censored change brackets; no production behavior is changed by this lane.

- A4-1 unsatisfied: campuses: NB(pc-2e9190844bde) only; NK missing; CM missing
- A4-2 unsatisfied: windows: 3 total; peak/off-peak classified on the server clock; qualifying peak (>=5 informative in-peak brackets): 0; qualifying off-peak (>=5 informative off-peak brackets, none in peak): 1 (window labels: 0 peak-overlapping, 3 off-peak)
- A4-3 unsatisfied: c30=67 c60=67 on 67 common brackets (server clock); reason=equal-coverage-30s-adds-no-explanatory-power; holdout=degenerate
- A4-4 unsatisfied: not identifiable (not-distinguishable)
- A4-6 unsatisfied: not evaluable: no distinguishable winner

## Data overview

| input | kind | rows | excluded | sha256 | target | windows |
|---|---|---:|---:|---|---|---:|
| 20260820T033451393Z-a8a40f98/samples.ndjson | ndjson | 1 | 0 | `ee1ca6fde70f…` | soc:2026:9:NB | 1 |
| 20260820T033527487Z-d92a8a35/samples.ndjson | ndjson | 3 | 0 | `bf6fa4cd5044…` | soc:2026:9:NB | 1 |
| 20260820T033701043Z-5745bc4a/samples.ndjson | ndjson | 554 | 0 | `80e3f8fc16df…` | soc:2026:9:NB | 1 |

## Windows

| windowId | UTC range | America/New_York | peak 17–18 ET? (label) | samples | brackets |
|---|---|---|---|---:|---:|
| 20260820T033451393Z-a8a40f98/samples.ndjson#w00 | 2026-08-20T03:34:51.458Z – 2026-08-20T03:34:51.792Z | 2026-08-19 23:34–23:34 ET | no | 1 | 0 |
| 20260820T033527487Z-d92a8a35/samples.ndjson#w00 | 2026-08-20T03:35:27.541Z – 2026-08-20T03:35:48.086Z | 2026-08-19 23:35–23:35 ET | no | 3 | 0 |
| 20260820T033701043Z-5745bc4a/samples.ndjson#w00 | 2026-08-20T03:37:01.121Z – 2026-08-20T05:42:20.043Z | 2026-08-19 23:37–08-20 01:42 ET | no | 554 | 67 |

## Bracket statistics

- Total change brackets: 67
- Excluded from all evidence (conflicted — campus or time-anchor — or duplicate provenance, see `provenance.excludedStreamIds` / `provenance.duplicateStreamIds`): 0
- Informative (0 < width < period, server clock — the comparison clock): 30 s → 67, 60 s → 67; non-informative: 30 s → 0, 60 s → 0
- Server brackets dropped for non-positive width (webfarm clock skew): 0; brackets rejected entirely for non-positive client/observed_at width (corrupt capture ordering): 0; change rows without a prior stable row: 0; bracket endpoints with `age > 0`: 0
- Client-clock width (s): min 12.585 / p50 13.945 / max 22.016
- Server-clock width incl. +1 s Date widening (s): min 13.000 / p50 15.000 / max 22.000

A bracket whose width reaches or exceeds a candidate period covers the whole phase circle for that period: it is **non-informative** and is excluded from coverage counting rather than silently counted as explained.

## Model comparison

| model | clock | informative | non-informative | unusable | max coverage | ratio | best phase interval(s) |
|---|---|---:|---:|---:|---:|---:|---|
| m30 | server | 67 | 0 | 0 | 67 | 1 | [00:00.001, 00:01.000] |
| m30 | client | 67 | 0 | 0 | 67 | 1 | [00:00.945, 00:01.493] |
| m60 | server | 67 | 0 | 0 | 67 | 1 | [00:00.001, 00:01.000] |
| m60 | client | 67 | 0 | 0 | 67 | 1 | [00:00.945, 00:01.493] |

Comparison on the 67 brackets informative for both periods (server clock): max coverage 30 s = **67**, 60 s = **67**.

max-coverage(30) ≥ max-coverage(60) holds identically — the ticks of any 60 s grid at phase φ are a subset of the 30 s grid's ticks at phase φ mod 30 — so equality can never select a winner: it only shows the 30 s grid adds no explanatory power, which is *consistent with* a true 60 s period but proves neither model. Holdout mode: **degenerate** (degenerate single-group half-split; NOT multi-window validation). Result: distinguishable = **false**, winner = **none**, reason = `equal-coverage-30s-adds-no-explanatory-power`.

### Stability (A4-6)

Not evaluable: no distinguishable winner.

## A4 gate

| id | requirement | satisfied | evidence |
|---|---|---|---|
| A4-1 | Multi-target evidence: at least NB, NK, CM independently evaluable from independent data provenance | no | campuses: NB(pc-2e9190844bde) only; NK missing; CM missing |
| A4-2 | Multiple independent time windows including America/New_York 17:00-18:00 peak and one off-peak window, each with qualifying informative brackets; peak and off-peak evidence only from brackets whose own comparison-clock bounds fall in that regime, and the off-peak window must hold no peak-hour bracket at all, so one straddling session cannot supply both sides | no | windows: 3 total; peak/off-peak classified on the server clock; qualifying peak (>=5 informative in-peak brackets): 0; qualifying off-peak (>=5 informative off-peak brackets, none in peak): 1 (window labels: 0 peak-overlapping, 3 off-peak) |
| A4-3 | 30s vs 60s model distinguishable with consistent winner under per-(target,window) holdout | no | c30=67 c60=67 on 67 common brackets (server clock); reason=equal-coverage-30s-adds-no-explanatory-power; holdout=degenerate |
| A4-4 | Unified safe offset covers phase and positive jitter across all targets/windows | no | not identifiable (not-distinguishable) |
| A4-5 | Report honestly handles server Date precision, client clock, and request latency; production conclusions rest on server-clock evidence | yes | server clock used; serverDate on 558 samples; qualifying groups with server evidence 1/1; +1s quantization widening applied; client-vs-server offset p50=-752 ms |
| A4-6 | Conclusions stable under whole-target leave-out, (target,window) group leave-out, and deterministic outlier removal | no | not evaluable: no distinguishable winner |

## Clock and caveats

- **Server `Date` precision**: 1 s (truncated). Every server-clock bracket upper bound is widened by +1 s so the true change instant is conservatively contained; widths quoted above include this widening.
- **Client vs server offset** (server second midpoint minus client request midpoint, 558 samples): min -6176 ms / p50 -752 ms / p95 -297 ms / max 309 ms. This mixes clock offset with request latency; it is reported, never used to correct timestamps.
- **Server Date regressions** (adjacent samples, > 1 s backwards): 0; samples missing serverDate: 0. The webfarm rotates multiple backends (`X-Server-Name`), so small skew between backends is expected and is why non-positive-width server brackets are dropped (0 here).
- **Server-clock evidence coverage**: sufficient = **true**; server-informative comparison brackets: 67; qualifying groups with server evidence: 1/1. Production GO and any safe offset require the comparison itself to run on the server clock with server brackets covering every qualifying group; a client-clock fallback fails these gates closed.
- **Caching**: `cache-control: max-age=30` means an intermediary cache could quantize observations; bracket endpoints with `age > 0`: 0.
- **etag is not a change signal**: the same body is served with multiple etags across backends; change detection uses `decodedBodySha256` only, and etag is never consulted.
- **Timestamp semantics**: NDJSON rows carry client-side `requestStartedUtc`/`requestEndedUtc` (bracket = (stable requestStart, changed requestEnd]); SQLite rows carry a single `observed_at`, used for both bracket endpoints, so SQLite client-clock brackets are narrower than the true envelope by up to one request duration. Client/observed_at timestamps must be monotone per target within a 2 s tolerance (fail-closed beyond it); a change pair whose client width is still non-positive indicates corrupt capture ordering and is rejected entirely and counted (0 here), never treated as informative.
- **Method**: brackets are interval-censored arcs on the period circle; the best phase is the maximum arc-coverage region. A `timestamp % period` histogram of detection times is invalid for this purpose (it confounds sampling cadence with change phase) and is not used.

## Reproduction

Offline, zero-dependency (Node ≥ 24). `<DATA_ROOT>` is the gitignored capture root `data/open-sections-repro`; raw sample data is never committed.

```
node tools/s3/rebuild-profile-analyzer.mjs \
  --ndjson <DATA_ROOT>/20260820T033451393Z-a8a40f98 \
  --ndjson <DATA_ROOT>/20260820T033527487Z-d92a8a35 \
  --ndjson <DATA_ROOT>/20260820T033701043Z-5745bc4a \
  --out-json docs/evidence/S3-REBUILD-PROFILE.json \
  --out-md docs/evidence/S3-REBUILD-PROFILE.md
```

Normalized command (order-independent): `rebuild-profile-analyzer --ndjson 20260820T033451393Z-a8a40f98/samples.ndjson --ndjson 20260820T033527487Z-d92a8a35/samples.ndjson --ndjson 20260820T033701043Z-5745bc4a/samples.ndjson --out-json S3-REBUILD-PROFILE.json --out-md S3-REBUILD-PROFILE.md`

## Input fingerprints

| input | kind | sha256 |
|---|---|---|
| 20260820T033451393Z-a8a40f98/samples.ndjson | ndjson | `ee1ca6fde70fa277ab9e3dbdc5eb14b658fc7982e7c9a391865f065025fce171` |
| 20260820T033527487Z-d92a8a35/samples.ndjson | ndjson | `bf6fa4cd50444c6c23c69909e8b291c4a6fd4aa9d874cce3c39dd5f6cd444ee3` |
| 20260820T033701043Z-5745bc4a/samples.ndjson | ndjson | `80e3f8fc16df15d14f729478ce939e78da7880aa44284fc128c6c3c1f2eb1eb2` |

For NDJSON inputs the fingerprint is sha256 over `sha256(samples.ndjson) + "\n" + sha256(run.json)` (or the samples hash alone when run.json is absent); for SQLite it is the file hash, re-verified unchanged after the analysis (read-only access).

## Missing evidence for a future GO

- Independent capture runs for the missing campuses (NK and/or CM alongside NB), each with enough change brackets per window for holdout grouping.
- At least one America/New_York 17:00–18:00 peak window and one independent off-peak window, each with ≥ 5 informative brackets of its own (an empty or single-sample window is metadata, not peak evidence) — peak and off-peak are classified from the server-clock bounds of individual brackets, not from the window label.
- Enough brackets across ≥ 2 (target, window) groups for non-degenerate holdout, and a strict, holdout-consistent coverage win before any winner can be declared.
- A winning model whose per-group phase intervals intersect and whose positive jitter is bounded, so one safe offset can be frozen.
- Stability of the winner under all three checks: whole-target leave-out, (target, window) group leave-out, and deterministic top-k outlier removal.

Future captures must follow the A6 sampling constraints as the capture plan. This lane holds **no online authorization** and performed **no network access**; it only re-analyzed previously captured, gitignored local data.
