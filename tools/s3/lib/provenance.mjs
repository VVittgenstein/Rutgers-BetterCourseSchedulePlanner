// Observation-data provenance: separates target identity (metadata: campus /
// year / term labels from run.json or SQLite target ids) from a fingerprint of
// the observation data itself. A4-1 must count independent DATA, not labels:
// the same capture copied into three run directories with three different
// campus labels is one observation series, not three independent targets — and
// neither are three overlapping slices of it, nor three decimations of it.
//
// Canonical series (per included sample, metadata-free):
//   bodySha \t deltaMs
// where deltaMs is the client-clock step from the previous included sample
// (first line 0). Campus/target labels, input ids, sample ids, sequence
// numbers, run.json content — and BOTH clock columns' absolute values — are
// deliberately excluded. serverDates are not part of the merge key AT ALL:
// neither their values nor their missing-pattern, and neither is the client
// request END. The sixth review round showed why a per-sample clock column must
// not be keyed: with "-" vs number in the canonical text, deleting a SINGLE
// serverDate field from a byte-copy minted a fresh fingerprint that neither
// matched nor was contained — the copy landed in its own "independent" class
// and the time-anchor check never ran, resurrecting copy-and-relabel with a
// one-field deletion. Keying on body content plus client-delta structure alone
// makes the fingerprint invariant to any constant translation of either clock
// column (by hours or by a single millisecond) AND to any edit or deletion of
// serverDates or request ends: every such copy MERGES into the genuine
// capture's family, where the record agreement check below sees the
// disagreement.
//
// PROVENANCE FAMILIES (one family = one body of captured observation data).
// Streams are ordered longest-first (streamId ascending on ties); for every
// ordered pair the first matching relation is recorded as an edge, and the
// families are the CONNECTED COMPONENTS of those edges (union-find), so that
// A~B and B~C put A, B and C in one family even when A and C do not match
// directly. The four relations, in priority order:
//   - identical:   canonical series byte-equal;
//   - contained:   the whole shorter series is a CONTIGUOUS slice of the longer
//                  one (prefix/suffix/middle truncations of a copy);
//   - overlapping: the two share a contiguous block of >= DERIVATION_MIN_BLOCK
//                  interior canonical entries containing at least
//                  DERIVATION_MIN_BLOCK_CHANGES body change(s) — staggered
//                  re-slicing of one capture, even after a joint clock shift;
//   - derived:     the two reuse >= DERIVATION_MIN_RECORDS observation RECORDS,
//                  matched jointly on (clientStartMs, bodySha) — regular or
//                  irregular subsampling, decimation, thinning, reordering.
// A clean family contributes exactly ONE evidence-eligible stream (its
// representative, the longest member with the lowest streamId); every other
// member is a duplicate barred from all evidence. So derived slices and
// subsamples of one capture can no longer multiply campus coverage, comparison
// n, or leave-one-out folds.
//
// Known boundary (by design, documented in the README): what is detected is
// REUSE OF THE SAME OBSERVATION RECORDS. A partition of one capture into
// chunks that share no record and no (body, delta) block reuses nothing —
// every observation is still counted once — and de novo fabrication of body
// content or of the time grid is not detected at all. This is a defence
// against copy/slice/subsample-and-relabel, not forensics against invented
// data, and not a cryptographic capture proof.
//
// Time-anchor and record conflict (fail-closed): the canonical series carries
// no absolute clock and no server or request-end column, so members of one
// family can still DISAGREE about WHEN the shared observation data happened,
// what the server's clock said, or when each request finished. Genuine
// duplicates of one capture (byte copies, SQLite re-ingests, truncations)
// carry the exact recorded fields, so at the aligned samples they agree
// exactly on the absolute client clock AND on every recorded serverDate AND on
// every recorded client request end — value and presence alike. An edge is
// time-aligned only when, at some content-matching alignment, (a) the aligned
// samples' clientStartMs are equal, AND (b) at every aligned sample the two
// serverDates and the two request ends are exactly equal or both absent — a
// different value, a deletion, or an insertion on either side is a
// disagreement. When ANY edge of a family fails, no deterministic choice among
// the conflicting records is safe — the family is flagged timeConflict and the
// analyzer excludes EVERY member from all evidence, exactly like a campus
// conflict. Tolerance is 0 ms on purpose: both ingest paths parse the same
// recorded timestamp strings, so any disagreement means the series was edited.
//
// The request-end comparison is suppressed when either side records no request
// end at all (SQLite ingestion has no such column and sets clientEndMs ==
// clientStartMs); otherwise every honest NDJSON-vs-SQLite duplicate would
// become a time conflict. Zeroing all request ends to dodge the check buys
// nothing: peak/off-peak classification runs on the server clock, so a copy
// with degenerate ends supplies no extra evidence.

import { sha256Text } from "./stable.mjs";
import {
  DERIVATION_MIN_BLOCK,
  DERIVATION_MIN_BLOCK_CHANGES,
  DERIVATION_MIN_RECORDS,
} from "./phase.mjs";
import {
  containmentAlignments,
  longestCommonBlocks,
  sharedRecordAlignment,
} from "./series-match.mjs";

// First match wins; lower number = stronger evidence of the same data.
const RELATION_PRIORITY = { identical: 0, contained: 1, overlapping: 2, derived: 3 };

// entries[k]: { bodySha, deltaMs, rawServerDelta: number|null, endDeltaMs: number|null }.
function canonicalEntries(samples) {
  const entries = [];
  let prevStartMs = null;
  for (const s of samples) {
    entries.push({
      bodySha: s.bodySha,
      deltaMs: prevStartMs === null ? 0 : s.clientStartMs - prevStartMs,
      rawServerDelta: s.serverDateMs === null ? null : s.serverDateMs - s.clientStartMs,
      endDeltaMs: Number.isFinite(s.clientEndMs) ? s.clientEndMs - s.clientStartMs : null,
    });
    prevStartMs = s.clientStartMs;
  }
  return entries;
}

function fingerprintOf(entries) {
  return sha256Text(entries.map((e) => `${e.bodySha}\t${e.deltaMs}`).join("\n"));
}

// Exact agreement on every RECORDED per-sample field that the fingerprint
// deliberately omits. Both columns are deltas relative to clientStartMs and the
// callers only consult them where the absolute client starts already agree at
// the aligned samples, so equal deltas mean equal absolute serverDates and
// equal absolute request ends (null === null covers agreement on absence, null
// vs number is a presence disagreement).
function fieldsAgreeAt(ea, eb, endComparable) {
  if (ea.rawServerDelta !== eb.rawServerDelta) return false;
  if (endComparable && ea.endDeltaMs !== eb.endDeltaMs) return false;
  return true;
}

function recordFieldsAgree(entriesA, entriesB, offsetA, offsetB, length, endComparable) {
  for (let k = 0; k < length; k += 1) {
    if (!fieldsAgreeAt(entriesA[offsetA + k], entriesB[offsetB + k], endComparable)) return false;
  }
  return true;
}

function distinctBodyCount(entries, start, length) {
  const seen = new Set();
  for (let k = 0; k < length; k += 1) seen.add(entries[start + k].bodySha);
  return seen.size;
}

function cmpStr(a, b) {
  return a < b ? -1 : a > b ? 1 : 0;
}

// The relation between a LONGER-OR-EQUAL stream `a` and a stream `b`, or null
// when the two carry independent observation data. Content-only predicates
// decide the relation; the recorded clock columns decide only timeAligned.
function relateStreams(a, b) {
  const endComparable = a.clientEndObserved && b.clientEndObserved;

  if (a.seriesFingerprint === b.seriesFingerprint) {
    return {
      relation: "identical",
      matchedCount: b.sampleCount,
      // Identical fingerprints share the whole content series, so the pair is
      // time-aligned iff the first sample's absolute client time matches AND
      // every serverDate and request end agrees exactly (value and presence).
      timeAligned:
        b.firstStartMs === a.firstStartMs &&
        recordFieldsAgree(b.entries, a.entries, 0, 0, b.entries.length, endComparable),
    };
  }

  const alignments = containmentAlignments(b.entries, a.entries);
  if (alignments.length > 0) {
    return {
      relation: "contained",
      matchedCount: b.sampleCount,
      // A genuine truncation agrees with the longer series' absolute client
      // clock and recorded columns at (at least) its true alignment offset.
      timeAligned: alignments.some(
        (offset) =>
          a.startTimes[offset] === b.firstStartMs &&
          recordFieldsAgree(b.entries, a.entries, 0, offset, b.entries.length, endComparable),
      ),
    };
  }

  // Overlapping re-slicing of one capture: a shared contiguous run of interior
  // entries that spans at least one body change. Shift-invariant, so a slice
  // whose clocks were translated still matches — and then fails timeAligned.
  const blocks = longestCommonBlocks(a.interior, b.interior, DERIVATION_MIN_BLOCK).filter(
    (blk) =>
      distinctBodyCount(a.entries, blk.aStart + 1, blk.length) >=
      1 + DERIVATION_MIN_BLOCK_CHANGES,
  );
  if (blocks.length > 0) {
    let matchedCount = 0;
    for (const blk of blocks) if (blk.length > matchedCount) matchedCount = blk.length;
    return {
      relation: "overlapping",
      matchedCount,
      // Equal deltas across the block make start equality at the block's first
      // aligned sample imply it at every aligned sample.
      timeAligned: blocks.some(
        (blk) =>
          a.startTimes[blk.aStart + 1] === b.startTimes[blk.bStart + 1] &&
          recordFieldsAgree(
            b.entries,
            a.entries,
            blk.bStart + 1,
            blk.aStart + 1,
            blk.length,
            endComparable,
          ),
      ),
    };
  }

  // Subsampled / decimated / thinned re-export: the two reuse actual
  // observation records. Absolute-time based, so it survives any re-cadencing
  // that destroys the delta structure — the case `overlapping` cannot see.
  const pairs = sharedRecordAlignment(a.recordKeys, b.recordKeys);
  if (pairs.length >= DERIVATION_MIN_RECORDS) {
    return {
      relation: "derived",
      matchedCount: pairs.length,
      // Client starts and bodies agree by construction of the match key; the
      // remaining recorded columns still have to.
      timeAligned: pairs.every(([ia, ib]) =>
        fieldsAgreeAt(a.entries[ia], b.entries[ib], endComparable),
      ),
    };
  }

  return null;
}

function findRoot(parent, x) {
  let root = x;
  while (parent[root] !== root) root = parent[root];
  let cur = x;
  while (parent[cur] !== root) {
    const next = parent[cur];
    parent[cur] = root;
    cur = next;
  }
  return root;
}

// streams: [{ inputId, samples }] as built by the CLI (one per (input, target)
// pair; NDJSON inputs carry one stream). Empty streams carry no observation
// data and are skipped. targetId/campus are read from the samples themselves.
export function buildProvenance(streams) {
  const enriched = [];
  for (const stream of streams) {
    if (stream.samples.length === 0) continue;
    const first = stream.samples[0];
    const entries = canonicalEntries(stream.samples);
    enriched.push({
      streamId: `${stream.inputId}::${first.targetId}`,
      targetId: first.targetId,
      campus: first.campus,
      sampleCount: stream.samples.length,
      entries,
      // Entry 0's delta is the synthetic 0 that makes the series
      // translation-invariant: it matches everything, so it must never anchor
      // a shared block.
      interior: entries.slice(1),
      // Absolute anchors (NOT part of the fingerprint): used only for the
      // record/time agreement checks and for the record-reuse match key.
      startTimes: stream.samples.map((s) => s.clientStartMs),
      recordKeys: stream.samples.map((s) => `${s.clientStartMs}\t${s.bodySha}`),
      firstStartMs: first.clientStartMs,
      // SQLite ingestion carries no request end and sets clientEndMs ==
      // clientStartMs; such a stream records no end to compare.
      clientEndObserved: entries.some((e) => e.endDeltaMs !== null && e.endDeltaMs !== 0),
      seriesFingerprint: fingerprintOf(entries),
    });
  }

  // Longest-first so every family representative is at least as long as any
  // other member; ties broken by streamId for determinism. A stream's position
  // in this order is its RANK and is the only ordering any step below uses, so
  // the result never depends on the argument order.
  const ordered = [...enriched].sort(
    (a, b) => b.sampleCount - a.sampleCount || cmpStr(a.streamId, b.streamId),
  );

  // 1. All pairwise relation edges, in ascending (rank a, rank b) order.
  const edges = [];
  const parent = ordered.map((_, i) => i);
  for (let ra = 0; ra < ordered.length; ra += 1) {
    for (let rb = ra + 1; rb < ordered.length; rb += 1) {
      const rel = relateStreams(ordered[ra], ordered[rb]);
      if (rel === null) continue;
      edges.push({ ra, rb, ...rel });
      // 2. Transitive closure: A~B and B~C put A, B, C in ONE family even when
      // A and C do not match directly (the union case a subsample pair hits).
      const rootA = findRoot(parent, ra);
      const rootB = findRoot(parent, rb);
      if (rootA !== rootB) parent[Math.max(rootA, rootB)] = Math.min(rootA, rootB);
    }
  }

  // 3. Families = connected components, keyed by their minimum rank (= the
  // representative: longest, streamId ascending on ties).
  const familyMembers = new Map(); // rootRank -> [rank, ...] ascending
  for (let r = 0; r < ordered.length; r += 1) {
    const root = findRoot(parent, r);
    if (!familyMembers.has(root)) familyMembers.set(root, []);
    familyMembers.get(root).push(r);
  }

  const classes = [];
  for (const [root, ranks] of [...familyMembers.entries()].sort((a, b) => a[0] - b[0])) {
    const memberSet = new Set(ranks);
    const familyEdges = edges.filter((e) => memberSet.has(e.ra) && memberSet.has(e.rb));
    // 4. Attachment (deterministic audit spanning tree): every non-
    // representative member is described by exactly ONE edge to an already
    // attached member, chosen by (rank of the newly attached member asc,
    // relation priority asc, rank of the anchor asc).
    const attachment = new Map(); // rank -> { relation, relatedTo, matchedCount }
    const attached = new Set([root]);
    while (attached.size < ranks.length) {
      let best = null;
      for (const e of familyEdges) {
        const aIn = attached.has(e.ra);
        const bIn = attached.has(e.rb);
        if (aIn === bIn) continue;
        const newRank = aIn ? e.rb : e.ra;
        const anchorRank = aIn ? e.ra : e.rb;
        const key = [newRank, RELATION_PRIORITY[e.relation], anchorRank];
        if (
          best === null ||
          key[0] < best.key[0] ||
          (key[0] === best.key[0] &&
            (key[1] < best.key[1] || (key[1] === best.key[1] && key[2] < best.key[2])))
        ) {
          best = { key, newRank, anchorRank, edge: e };
        }
      }
      // The family is a connected component, so an edge always exists.
      attached.add(best.newRank);
      attachment.set(best.newRank, {
        relation: best.edge.relation,
        relatedTo: ordered[best.anchorRank].streamId,
        matchedCount: best.edge.matchedCount,
      });
    }

    const campuses = [...new Set(ranks.map((r) => ordered[r].campus).filter((c) => c !== null))];
    const campusConflict = campuses.length > 1;
    // A record/time disagreement on ANY edge of the family voids it — not just
    // on the spanning-tree edges, so an attacker cannot arrange a clean
    // attachment path around a conflicting pair.
    const conflictEdges = familyEdges.filter((e) => e.timeAligned === false);
    const timeConflict = conflictEdges.length > 0;
    classes.push({
      classId: `pc-${ordered[root].seriesFingerprint.slice(0, 12)}`,
      campus: campusConflict || campuses.length === 0 ? null : campuses[0],
      campusConflict,
      timeConflict,
      timeConflictPairs: conflictEdges
        .map((e) => ({
          streamIdA: ordered[e.ra].streamId,
          streamIdB: ordered[e.rb].streamId,
          relation: e.relation,
        }))
        .sort((x, y) => cmpStr(x.streamIdA, y.streamIdA) || cmpStr(x.streamIdB, y.streamIdB)),
      members: ranks
        .map((r) => ({
          streamId: ordered[r].streamId,
          relation: r === root ? "representative" : attachment.get(r).relation,
          relatedTo: r === root ? null : attachment.get(r).relatedTo,
          matchedCount: r === root ? null : attachment.get(r).matchedCount,
        }))
        .sort(
          (a, b) =>
            (a.relation === "representative" ? 0 : 1) -
              (b.relation === "representative" ? 0 : 1) || cmpStr(a.streamId, b.streamId),
        ),
    });
  }
  classes.sort((a, b) => cmpStr(a.classId, b.classId));

  const streamsOut = enriched
    .map((s) => ({
      streamId: s.streamId,
      targetId: s.targetId,
      campus: s.campus,
      sampleCount: s.sampleCount,
      clientEndObserved: s.clientEndObserved,
      seriesFingerprint: s.seriesFingerprint,
    }))
    .sort((a, b) => cmpStr(a.streamId, b.streamId));

  return { streams: streamsOut, classes };
}
