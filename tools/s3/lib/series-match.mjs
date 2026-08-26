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
//                            reuse, in order?
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
  const pairs = [];
  if (aKeys.length === 0 || bKeys.length === 0) return pairs;
  const index = new Map();
  for (let i = 0; i < aKeys.length; i += 1) {
    const key = aKeys[i];
    const bucket = index.get(key);
    if (bucket === undefined) index.set(key, [i]);
    else bucket.push(i);
  }
  let minA = 0;
  for (let j = 0; j < bKeys.length; j += 1) {
    const bucket = index.get(bKeys[j]);
    if (bucket === undefined) continue;
    // Smallest index in the bucket that is >= minA (buckets are ascending).
    let lo = 0;
    let hi = bucket.length;
    while (lo < hi) {
      const mid = (lo + hi) >> 1;
      if (bucket[mid] < minA) lo = mid + 1;
      else hi = mid;
    }
    if (lo === bucket.length) continue;
    const ia = bucket[lo];
    pairs.push([ia, j]);
    minA = ia + 1;
  }
  return pairs;
}
