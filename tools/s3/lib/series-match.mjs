// Pure sequence matchers over canonical observation series.
//
// These are the CONTENT-ONLY predicates behind provenance families: none of
// them reads a campus label, a target id, an input id, a file name, a
// sequence number, or a source kind, and none of them decides policy. They
// answer three questions about two canonical entry lists:
//
//   containmentAlignments  — is A's whole content a contiguous slice of B's?
//   longestCommonBlocks    — which contiguous stretches do A and B share?
//   sharedRecordAlignment  — which individual observation RECORDS do A and B
//                            reuse, in order, at the SAME absolute client
//                            instants?
//   bestShiftedRecordAlignment — same question, but allowing ONE constant
//                            client-clock offset shared by every matched
//                            record.
//
// The first two key on (bodySha, deltaMs), so they are invariant under any
// translation of either clock: a slice whose timestamps were shifted still
// matches, and the record-agreement layer in provenance.mjs then judges the
// shift. The third keys on (clientStartMs, bodySha) jointly, so it survives
// any re-cadencing (decimation, thinning, reordering) that reuses the actual
// records — and it never fires on two honest captures, which agree on neither
// millisecond-resolution request starts nor, jointly, on body hashes.
//
// Neither key alone would do. bodySha alone is dominated by stale runs (a real
// capture can hold the same body for minutes on end, and two honest captures of
// the same target legitimately see the same bodies); clientStartMs alone merges
// honest captures that merely ran at the same wall-clock times.
//
// The fourth matcher generalizes the third by exactly one degree of freedom: a
// single constant offset applied to the WHOLE stream. It keys on
// (bodySha, clientStartMs + offset) for one offset shared by every matched
// record, so a regular subsample whose client clock was translated once still
// matches, while a per-sample jitter — where no single offset explains more
// than a coincidence's worth of records — does not. The offset is an exact
// integer equality, never a tolerance window: widening it is what would reach
// into the deliberately-deferred jitter case.
//
// Complexity of the offset search: P = sum over bodies of n_body * m_body
// candidate pairs, bounded by |a| * |b| and in practice far below it (measured
// on the 554-sample local capture against itself: P = 8202 against n^2 =
// 306916). No cap is applied — a cap could only lose real matches.

// All alignment offsets at which series A's CONTENT (bodySha + client-delta
// structure, first delta of the slice normalized to 0, matching A's own first
// delta of 0) is a contiguous slice of series B. serverDates play no role in
// the match — a copy whose server column was edited or emptied still aligns
// here and is then judged by the record agreement check. Every offset is
// returned (not just the first) so the time-anchor check can accept a genuine
// truncation whose content happens to align at more than one position: it
// agrees in absolute time at its true offset.
export function containmentAlignments(a, b) {
  const offsets = [];
  if (a.length === 0 || a.length > b.length) return offsets;
  for (let i = 0; i + a.length <= b.length; i += 1) {
    let match = true;
    for (let k = 0; k < a.length; k += 1) {
      const x = a[k];
      const y = b[i + k];
      if (x.bodySha !== y.bodySha) {
        match = false;
        break;
      }
      // The slice's first delta is normalized to 0 (= x.deltaMs by construction).
      if (k > 0 && x.deltaMs !== y.deltaMs) {
        match = false;
        break;
      }
    }
    if (match) offsets.push(i);
  }
  return offsets;
}

// Every MAXIMAL contiguous block on which two INTERIOR entry lists agree in
// (bodySha, deltaMs), with length >= minLength. "Interior" means entry 0 has
// been dropped by the caller: entry 0 carries the synthetic delta 0 that makes
// a canonical series translation-invariant, so it matches everything and must
// never anchor a block. Returned as {length, aStart, bStart} indices INTO THE
// INTERIOR LISTS, sorted by (aStart asc, bStart asc) for byte-stable output.
//
// Exact algorithm (two-row common-suffix DP on integer lengths): row[j] holds
// the length of the common suffix ending at aInterior[i-1] / bInterior[j-1].
// A run is emitted when it cannot be extended one step to the right, which is
// precisely the maximality condition.
export function longestCommonBlocks(aInterior, bInterior, minLength) {
  const n = aInterior.length;
  const m = bInterior.length;
  const blocks = [];
  if (minLength <= 0 || n < minLength || m < minLength) return blocks;
  let prev = new Int32Array(m + 1);
  let cur = new Int32Array(m + 1);
  for (let i = 1; i <= n; i += 1) {
    const ai = aInterior[i - 1];
    cur[0] = 0;
    for (let j = 1; j <= m; j += 1) {
      const bj = bInterior[j - 1];
      if (ai.bodySha === bj.bodySha && ai.deltaMs === bj.deltaMs) {
        const len = prev[j - 1] + 1;
        cur[j] = len;
        if (len >= minLength) {
          const extendable =
            i < n &&
            j < m &&
            aInterior[i].bodySha === bInterior[j].bodySha &&
            aInterior[i].deltaMs === bInterior[j].deltaMs;
          if (!extendable) blocks.push({ length: len, aStart: i - len, bStart: j - len });
        }
      } else {
        cur[j] = 0;
      }
    }
    const swap = prev;
    prev = cur;
    cur = swap;
  }
  blocks.sort((x, y) => x.aStart - y.aStart || x.bStart - y.bStart || x.length - y.length);
  return blocks;
}

// The greedy left-to-right walk shared by both record matchers. Consumes a
// pair list ordered by (ib asc, ia asc) and returns the strictly-increasing
// alignment it induces: for each ib in order, take the first still-available
// ia, then advance the floor. Extracted verbatim from the walk
// sharedRecordAlignment performed inline, so both matchers report the same
// count for the same candidate set.
function greedyIncreasingPairs(pairsByBAsc) {
  const pairs = [];
  let minA = 0;
  let i = 0;
  while (i < pairsByBAsc.length) {
    const ib = pairsByBAsc[i][1];
    let taken = false;
    while (i < pairsByBAsc.length && pairsByBAsc[i][1] === ib) {
      if (!taken && pairsByBAsc[i][0] >= minA) {
        pairs.push([pairsByBAsc[i][0], ib]);
        minA = pairsByBAsc[i][0] + 1;
        taken = true;
      }
      i += 1;
    }
  }
  return pairs;
}

// The in-order set of observation RECORDS that two streams reuse, as index
// pairs [indexInA, indexInB], strictly increasing on both sides. Record keys
// are opaque strings supplied by the caller (provenance.mjs builds them from
// clientStartMs and bodySha jointly). Millisecond-resolution client starts make
// the key buckets singletons in practice, so the greedy left-to-right walk
// below is the optimal alignment; when a key does repeat inside one stream the
// walk still returns a valid strictly-increasing alignment, and the count it
// reports is a LOWER bound on reuse — the fail-closed direction for a threshold
// that triggers exclusion.
export function sharedRecordAlignment(aKeys, bKeys) {
  if (aKeys.length === 0 || bKeys.length === 0) return [];
  const index = new Map();
  for (let i = 0; i < aKeys.length; i += 1) {
    const bucket = index.get(aKeys[i]);
    if (bucket === undefined) index.set(aKeys[i], [i]);
    else bucket.push(i);
  }
  const ordered = [];
  for (let j = 0; j < bKeys.length; j += 1) {
    const bucket = index.get(bKeys[j]);
    if (bucket === undefined) continue;
    // Buckets are built in ascending ia, and j ascends, so the concatenation
    // is already in (ib asc, ia asc) order — exactly what the walk wants.
    for (const ia of bucket) ordered.push([ia, j]);
  }
  return greedyIncreasingPairs(ordered);
}

// The best SHIFT-INVARIANT record alignment between two streams: the single
// constant client-clock offset `offsetMs` at which the largest strictly
// increasing set of same-body records lines up, or null when no offset clears
// its threshold.
//
//   a, b: { times: number[], bodies: string[] } — parallel arrays in stream
//   order (provenance.mjs passes clientStartMs and bodySha).
//
// Two thresholds, because the two cases are not equally cheap to hit by
// accident. `minPairsAtZero` governs offset 0, where exactly ONE offset is
// ever tested and the key is the joint (clientStartMs, bodySha) key the older
// rule used — so this branch reproduces that rule exactly. `minPairsShifted`
// governs every non-zero offset, where the search ranges over every offset any
// same-body pair produces, and must therefore be re-argued against the
// accidental ceiling (see phase.mjs).
//
// Selection is by an EXPLICIT comparator — (alignment length desc, |offset|
// asc, offset asc) — never by Map iteration order, which is insertion order
// and would leak the argument order into the result. The |offset| tie-break
// makes offset 0 win every tie, which is what keeps every previously-detected
// derivation bit-identical.
export function bestShiftedRecordAlignment(a, b, minPairsAtZero, minPairsShifted) {
  if (a.times.length === 0 || b.times.length === 0) return null;
  const byBody = new Map();
  for (let ia = 0; ia < a.times.length; ia += 1) {
    const bucket = byBody.get(a.bodies[ia]);
    if (bucket === undefined) byBody.set(a.bodies[ia], [ia]);
    else bucket.push(ia);
  }
  // offset -> candidate pairs, accumulated in (ib asc, ia asc) order because
  // ib ascends in the outer loop and each body bucket ascends in ia.
  const byOffset = new Map();
  for (let ib = 0; ib < b.times.length; ib += 1) {
    const candidates = byBody.get(b.bodies[ib]);
    if (candidates === undefined) continue;
    for (const ia of candidates) {
      const delta = b.times[ib] - a.times[ia];
      const bucket = byOffset.get(delta);
      if (bucket === undefined) byOffset.set(delta, [[ia, ib]]);
      else bucket.push([ia, ib]);
    }
  }
  const survivors = [];
  for (const [delta, candidatePairs] of byOffset) {
    const threshold = delta === 0 ? minPairsAtZero : minPairsShifted;
    // Cheap pre-filter: the greedy alignment can only be shorter than the raw
    // candidate list, so a bucket below the threshold can never survive it.
    if (candidatePairs.length < threshold) continue;
    const pairs = greedyIncreasingPairs(candidatePairs);
    if (pairs.length < threshold) continue;
    survivors.push({ delta, pairs });
  }
  if (survivors.length === 0) return null;
  survivors.sort(
    (x, y) =>
      y.pairs.length - x.pairs.length ||
      Math.abs(x.delta) - Math.abs(y.delta) ||
      x.delta - y.delta,
  );
  return { offsetMs: survivors[0].delta, pairs: survivors[0].pairs };
}
