// Window segmentation, server-timeline session assignment, and
// America/New_York peak-overlap classification.

import {
  WINDOW_GAP_MIN_MS,
  WINDOW_GAP_INTERVAL_FACTOR,
  NY_PEAK,
} from "./phase.mjs";

function zeroPad(n, width) {
  return String(n).padStart(width, "0");
}

// Segments one (inputId, targetId) sample stream (already sequence-sorted)
// into windows on client-clock gaps.
export function segmentWindows(samples, intervalSeconds, inputId) {
  const gapMs = Math.max(
    WINDOW_GAP_MIN_MS,
    WINDOW_GAP_INTERVAL_FACTOR * (intervalSeconds ?? 60) * 1000,
  );
  const windows = [];
  let current = null;
  for (const sample of samples) {
    if (
      current === null ||
      sample.clientStartMs - current.samples[current.samples.length - 1].clientStartMs > gapMs
    ) {
      current = {
        windowId: `${inputId}#w${zeroPad(windows.length, 2)}`,
        utcStartMs: sample.clientStartMs,
        utcEndMs: sample.clientEndMs,
        samples: [sample],
      };
      windows.push(current);
    } else {
      current.samples.push(sample);
      current.utcEndMs = sample.clientEndMs;
    }
  }
  return windows;
}

// Server-timeline counterpart of segmentWindows, for ONE (inputId, targetId)
// sample stream in the same sequence order. Returns an array of session
// indices aligned 1:1 with `samples`.
//
// Same gap rule, same threshold and the same strict greater-than comparison as
// segmentWindows — the server-side statement is exactly as strong as the
// client-side one, never weaker. The sample ORDER is never re-derived from the
// Date headers: an edited or skewed serverDate must not be able to reorder
// records.
//
// THE SEAM IS MEASURED WITH ORDER STATISTICS, NOT WITH THE ADJACENT PAIR.
// A candidate boundary at sample i is a claim about two SETS: everything the
// server timeline has already reached, and the earliest point everything from
// i onward reaches. So the gap is measured as
//
//     suffixMin(i) - prefixMax(i)
//        prefixMax(i) = the LATEST Date carried by any dated sample before i
//        suffixMin(i) = the EARLIEST Date carried by any dated sample at or
//                       after i
//
// and never as `serverDateMs[i] - serverDateMs[i-1]`. The adjacent-pair form
// let ONE forged Date MINT a session boundary from either side (CE-17):
//   - drag the last Date of a client window BACKWARDS and the next honest
//     sample's difference is measured from a value the timeline itself
//     contradicts, so the honest sample "jumps forward" past the threshold;
//   - drag the first Date of the next client window FORWARDS past the
//     threshold and it splits on its own, while every later sample rejoins it
//     because their difference from the inflated value is negative.
// Under the order-statistic form neither works: at a seam with two or more
// dated samples on each side, no single edited cell can RAISE the measured
// seam at all — raising suffixMin needs every low Date in the suffix raised,
// and lowering prefixMax needs every high Date in the prefix lowered.
//
// THAT ROBUSTNESS IS IN THE SPLIT DIRECTION ONLY, AND IT IS NOT ENOUGH ON ITS
// OWN. Both statistics have breakdown point ONE in the MERGE direction: a
// single Date placed low anywhere at or after i collapses suffixMin(i), and a
// single Date placed high anywhere before i inflates prefixMax(i), so one cell
// can SUPPRESS a genuine boundary and pool two server-separated sessions.
// A merge is NOT the fail-closed direction. A4-2 counts informative brackets
// PER SESSION against MIN_GROUP_BRACKETS, so pooling two sub-threshold
// sessions manufactures a QUALIFYING one — and off-peak purity survives the
// union of two pure sessions, so the off-peak side pools just as freely as the
// peak side. The merge direction is closed by serverGroupingRobustness()
// below, not by this function.
//
// On a NON-DECREASING server timeline the two forms are identical: prefixMax(i)
// is then serverDateMs[i-1] and suffixMin(i) is serverDateMs[i]. Real captures
// are non-decreasing (local-data.test.mjs asserts it on D1/D2/D3), so this is a
// no-op on honest data. In general suffixMin(i) - prefixMax(i) <= the adjacent
// difference, so this rule splits only where the adjacent-difference rule did.
// That makes the session PARTITION coarser than the adjacent-difference one; it
// does NOT make the A4-2 GATE stricter, because A4-2 is anti-monotone under
// coarsening (see above).
//
// AN ABSENT serverDate IMPOSES NO GROUPING CONSTRAINT AT ALL: its index is
// null, wherever in the stream it sits. It used to INHERIT the running index,
// on the reasoning that a deleted header must not be able to manufacture a
// split; but inheriting is not neutral, it is a vote for the EARLIER session,
// and that vote is exactly a merge. Deleting the first Date of the later client
// window made that window hold both session indices, which unions it with the
// earlier window — one deleted header pooling two genuinely server-separated
// sessions (CE-18, delete direction). Contributing nothing closes it in both
// directions at once:
//   - it cannot manufacture a split, because a boundary is only ever placed at
//     a DATED sample and deleting Dates can neither raise a suffix minimum nor
//     lower a prefix maximum by more than one polling interval per deletion;
//   - it cannot manufacture a merge, because an index that does not exist
//     cannot be shared;
//   - a window with no dated sample left becomes its own group, which buys an
//     attacker nothing: with no server bounds its brackets are non-informative
//     on the server clock, count as no-bounds on both A4-2 sides, and still
//     break that group's off-peak purity.
// It also makes "hold out one Date" and "delete one Date" the SAME operation,
// which is what lets the robustness check below cover deletions as well as
// edits.
// A negative or zero measured seam (webfarm skew) never splits.
export function assignServerSessions(samples, intervalSeconds) {
  return sessionIndicesOf(
    samples.map((s) => s.serverDateMs ?? null),
    sessionGapMs(intervalSeconds),
  );
}

function sessionGapMs(intervalSeconds) {
  return Math.max(
    WINDOW_GAP_MIN_MS,
    WINDOW_GAP_INTERVAL_FACTOR * (intervalSeconds ?? 60) * 1000,
  );
}

// The rule itself, over a plain array of `number | null` server Dates in record
// order. Split out from assignServerSessions so the robustness check below can
// re-run the EXACT same rule on a held-out Date column — never a paraphrase of
// it.
function sessionIndicesOf(serverMs, gapMs) {
  const n = serverMs.length;
  // suffixMin[i] = the earliest Date at or after i (null when there is none).
  const suffixMin = new Array(n + 1).fill(null);
  for (let i = n - 1; i >= 0; i -= 1) {
    const v = serverMs[i];
    suffixMin[i] =
      v === null
        ? suffixMin[i + 1]
        : suffixMin[i + 1] === null
          ? v
          : Math.min(v, suffixMin[i + 1]);
  }
  const indices = [];
  let current = null;
  let prefixMax = null; // the latest Date carried by any dated sample before i
  for (let i = 0; i < n; i += 1) {
    const v = serverMs[i];
    if (v === null) {
      // No Date, no claim about the timeline: this sample joins no session.
      indices.push(null);
      continue;
    }
    if (current === null) current = 0;
    else if (prefixMax !== null && suffixMin[i] - prefixMax > gapMs) current += 1;
    prefixMax = prefixMax === null ? v : Math.max(prefixMax, v);
    indices.push(current);
  }
  return indices;
}

// The grouping A4-2 actually consumes: buildEvidenceSessions unions two client
// windows of one stream whenever they hold samples carrying the SAME server
// session index. Rendered canonically as one component root per window ordinal
// so that two groupings can be compared for equality.
function windowGroupingSignature(indices, windowOfSample, windowCount) {
  const parent = new Array(windowCount);
  for (let w = 0; w < windowCount; w += 1) parent[w] = w;
  const find = (x) => {
    let root = x;
    while (parent[root] !== root) root = parent[root];
    let cur = x;
    while (parent[cur] !== root) {
      const next = parent[cur];
      parent[cur] = root;
      cur = next;
    }
    return root;
  };
  const union = (x, y) => {
    const rx = find(x);
    const ry = find(y);
    if (rx !== ry) parent[Math.max(rx, ry)] = Math.min(rx, ry);
  };
  const firstHolderOf = new Map();
  for (let i = 0; i < indices.length; i += 1) {
    const idx = indices[i];
    if (idx === null || idx === undefined) continue;
    const w = windowOfSample[i];
    const prev = firstHolderOf.get(idx);
    if (prev === undefined) firstHolderOf.set(idx, w);
    else union(prev, w);
  }
  const roots = new Array(windowCount);
  for (let w = 0; w < windowCount; w += 1) roots[w] = find(w);
  return roots.join(",");
}

// IS THIS STREAM'S EVIDENCE GROUPING ESTABLISHED BY THE SERVER TIMELINE, OR BY
// ONE Date CELL?
//
// assignServerSessions is robust against a forged split and forgeable in the
// merge direction, and no amount of further hardening of the SEAM STATISTIC
// fixes that: max/min, second-largest/second-smallest, a trimmed mean — each is
// one number, and one cell can always push a single number in one of the two
// directions. So the merge direction is closed by a different kind of statement
// — a LEAVE-ONE-OUT stability test on the object A4-2 actually consumes.
//
// Recompute the window grouping once per dated sample, each time with that ONE
// sample's Date held out — which, since an absent Date imposes no grouping
// constraint, is literally the same stream an attacker would produce by
// DELETING that header. If any of those groupings differs from the grouping
// computed with every Date present, then a single Date header decides how this
// stream's windows are grouped into independent evidence sessions and the
// server timeline has NOT established that grouping. The caller then voids the
// stream's A4-2 evidence, which can only remove evidence: a void never turns a
// NO-GO into a GO.
//
// WHAT THIS BUYS, stated as a theorem, because loose "one forged cell" claims
// are exactly where the previous rounds went wrong:
//
//   Let G* be the grouping of the honest stream and G_j the grouping of the
//   honest stream with cell j held out. Suppose the honest capture is 1-ROBUST,
//   i.e. G_j = G* for every j. Let an attacker replace cell j with any value
//   whatsoever, giving observed grouping G. Holding cell j out of the FORGED
//   stream leaves exactly the honest remaining cells, so this check computes
//   G_j = G*. Hence either G = G* — the forgery changed nothing A4-2 can see —
//   or G != G* = G_j and the check fires. No single-cell serverDate edit can
//   change the evidence grouping of a 1-robust capture without being flagged,
//   in EITHER direction, split or merge.
//
// Honest captures are 1-robust with enormous margin. Their sessions are either
// one continuous run — hold out any one Date and the run is still continuous —
// or runs separated by hours, where holding out one Date leaves the separation
// hours wide. Holding out the Date immediately before or after a real gap moves
// the boundary to the next dated sample, which lies in the SAME client window,
// so the window grouping does not move. D1/D2/D3 produce ONE window per stream,
// where the grouping is the trivial singleton and the check cannot fire at all.
//
// It does NOT claim robustness against a BULK rewrite, which stays deferred per
// A3: an attacker who rewrites a whole window tail changes the grouping in a
// way that survives every single-cell hold-out — and an honest 11-minute pause
// moved on both clocks is exactly such a rewrite and is required to stay GO.
//
// `windowOfSample[i]` is the client-window ordinal of `samples[i]` and
// `windowCount` is the number of client windows in the stream. Cost is
// O(n * (n + windowCount)) with an early exit on the first disagreement.
export function serverGroupingRobustness(samples, intervalSeconds, windowOfSample, windowCount) {
  const gapMs = sessionGapMs(intervalSeconds);
  const serverMs = samples.map((s) => s.serverDateMs ?? null);
  const base = windowGroupingSignature(
    sessionIndicesOf(serverMs, gapMs),
    windowOfSample,
    windowCount,
  );
  const heldOut = [...serverMs];
  for (let j = 0; j < serverMs.length; j += 1) {
    // An ABSENT Date already makes no claim, so holding it out changes nothing.
    if (serverMs[j] === null) continue;
    heldOut[j] = null;
    const alt = windowGroupingSignature(
      sessionIndicesOf(heldOut, gapMs),
      windowOfSample,
      windowCount,
    );
    heldOut[j] = serverMs[j];
    if (alt !== base) {
      return { robust: false, decidedByIndex: j, grouping: base, heldOutGrouping: alt };
    }
  }
  return { robust: true, decidedByIndex: null, grouping: base, heldOutGrouping: null };
}

const NY_PARTS_FMT = new Intl.DateTimeFormat("en-CA", {
  timeZone: NY_PEAK.timeZone,
  year: "numeric",
  month: "2-digit",
  day: "2-digit",
  hour: "2-digit",
  minute: "2-digit",
  second: "2-digit",
  hour12: false,
});

function nyParts(utcMs) {
  const parts = {};
  for (const { type, value } of NY_PARTS_FMT.formatToParts(new Date(utcMs))) {
    parts[type] = value;
  }
  // Intl can render hour 24 for midnight with some locales/options; normalize.
  if (parts.hour === "24") parts.hour = "00";
  return parts;
}

export function nyLabel(utcStartMs, utcEndMs) {
  const s = nyParts(utcStartMs);
  const e = nyParts(utcEndMs);
  const sameDay = s.year === e.year && s.month === e.month && s.day === e.day;
  const startStr = `${s.year}-${s.month}-${s.day} ${s.hour}:${s.minute}`;
  const endStr = sameDay ? `${e.hour}:${e.minute}` : `${e.month}-${e.day} ${e.hour}:${e.minute}`;
  return `${startStr}–${endStr} ET`;
}

// UTC offset (ms to ADD to a NY wall-clock-as-UTC reading to get real UTC):
// derived by probing Intl at the given instant — DST-safe.
function nyOffsetMsAt(utcMs) {
  const p = nyParts(utcMs);
  const asUtc = Date.UTC(
    Number(p.year),
    Number(p.month) - 1,
    Number(p.day),
    Number(p.hour),
    Number(p.minute),
    Number(p.second),
  );
  return utcMs - asUtc; // e.g. EDT: utc = wall + 4h → offset = +4h... (wall < utc)
}

// UTC offset valid for the given NY calendar day's late afternoon (DST-safe:
// transitions happen at 02:00 local, far from the 17:00-18:00 peak).
function offsetForDayAfternoon(year, month, day) {
  const wallAsUtc = Date.UTC(year, month - 1, day, 17, 30, 0);
  // Fixed-point refinement of the offset, starting from a 5 h (EST) guess.
  let guess = wallAsUtc + 5 * 3600000;
  for (let i = 0; i < 3; i += 1) {
    guess = wallAsUtc + nyOffsetMsAt(guess);
  }
  return nyOffsetMsAt(guess);
}

function nyPeakOverlapImpl(utcStartMs, utcEndMs, strict) {
  // Iterate each NY calendar day touched by the window (pad one day each side
  // to be safe around offset boundaries).
  const dayMs = 24 * 3600 * 1000;
  const seen = new Set();
  for (let probe = utcStartMs - dayMs; probe <= utcEndMs + dayMs; probe += dayMs) {
    const p = nyParts(probe);
    const dayKey = `${p.year}-${p.month}-${p.day}`;
    if (seen.has(dayKey)) continue;
    seen.add(dayKey);
    const y = Number(p.year);
    const m = Number(p.month);
    const d = Number(p.day);
    const offset = offsetForDayAfternoon(y, m, d);
    const peakStartUtc = Date.UTC(y, m - 1, d, NY_PEAK.startHour, 0, 0) + offset;
    const peakEndUtc = Date.UTC(y, m - 1, d, NY_PEAK.endHour, 0, 0) + offset;
    const hit = strict
      ? peakStartUtc < utcEndMs && peakEndUtc > utcStartMs
      : peakStartUtc <= utcEndMs && peakEndUtc >= utcStartMs;
    if (hit) return true;
  }
  return false;
}

// True iff [utcStartMs, utcEndMs] intersects any America/New_York
// 17:00:00–18:00:00 local interval (closed: a boundary touch counts). Used for
// window LABELING only — never as peak evidence.
export function overlapsNyPeak(utcStartMs, utcEndMs) {
  return nyPeakOverlapImpl(utcStartMs, utcEndMs, false);
}

// True iff the intersection has POSITIVE measure: an interval that merely
// touches 17:00:00.000 or 18:00:00.000 ET at a single instant contains zero
// peak observation time and is NOT peak evidence. This is the check the A4-2
// gate applies to individual change brackets.
export function overlapsNyPeakStrict(utcStartMs, utcEndMs) {
  return nyPeakOverlapImpl(utcStartMs, utcEndMs, true);
}
