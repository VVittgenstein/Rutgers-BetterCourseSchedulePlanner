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
// THAT ROBUSTNESS IS IN THE SPLIT DIRECTION ONLY, and the exact statement is:
// to RAISE the measured seam at index i an attacker must raise every dated
// sample at or after i that sits below the target, or lower every dated sample
// before i that sits above it — so the breakdown point in the split direction
// is the number of dated samples on the smaller side of the seam, not one.
//
// A DELETED Date is not covered by that count: it is not raised, it is removed
// from both order statistics, and removing every dated sample in a band around
// a seam widens the measured seam for free. That hole is closed one level up,
// by `seamServerAdvanceMs` below, which corroborates a client-window seam from
// the two windows' MEDIAN clock offsets instead of from the Dates at the seam.
//
// In the MERGE direction both statistics still have breakdown point ONE: a
// single Date placed low anywhere at or after i collapses suffixMin(i), and a
// single Date placed high anywhere before i inflates prefixMax(i), so one cell
// can SUPPRESS a genuine boundary and pool two server-separated sessions into
// one. That is deliberate and safe, but ONLY because of how A4-2 consumes the
// grouping: a session qualifies a side only when EVERY constituent client
// window clears MIN_GROUP_BRACKETS on that side by itself (gate.mjs). Pooling
// can therefore never manufacture evidence — the per-window minimum of a union
// is at most the minimum of either part — so the merge direction is the
// fail-closed one and needs no separate defence here. An earlier revision
// (v2.7.1/v2.7.2) instead summed brackets over the session and bolted a
// leave-one-out grouping check on top; that check refused honest captures whose
// genuine seam happened to land within one polling gap of the threshold, and
// still admitted any two-cell pooling forgery. It is gone.
//
// On a NON-DECREASING server timeline the two forms are identical: prefixMax(i)
// is then serverDateMs[i-1] and suffixMin(i) is serverDateMs[i]. Real captures
// are non-decreasing (local-data.test.mjs asserts it on D1/D2/D3), so this is a
// no-op on honest data. In general suffixMin(i) - prefixMax(i) <= the adjacent
// difference, so this rule splits only where the adjacent-difference rule did:
// the session PARTITION is coarser than the adjacent-difference one, and under
// the per-window rule above coarser is never more permissive.
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
//   - it cannot manufacture a merge, because an index that does not exist
//     cannot be shared;
//   - it CAN move a boundary, and enough deletions on one side of a seam can
//     raise the measured seam past the threshold and so manufacture a SPLIT.
//     Deletion is therefore NOT enough on its own to reach two independent
//     evidence sessions: the seam a deletion widens still has to survive the
//     SEAM CORROBORATION check below, which does not read the Dates at the
//     seam at all;
//   - a window with no dated sample left becomes its own group; it also loses
//     every server bracket bound it had, so its brackets are non-informative on
//     the server clock and it can qualify neither A4-2 side.
// A negative or zero measured seam (webfarm skew) never splits.
export function assignServerSessions(samples, intervalSeconds) {
  return sessionIndicesOf(
    samples.map((s) => s.serverDateMs ?? null),
    sessionGapMs(intervalSeconds),
  );
}

export function sessionGapMs(intervalSeconds) {
  return Math.max(
    WINDOW_GAP_MIN_MS,
    WINDOW_GAP_INTERVAL_FACTOR * (intervalSeconds ?? 60) * 1000,
  );
}

// The robust client-vs-server clock offset of one window: the LOWER median of
// (serverDateMs - clientStartMs) over its dated samples, or null when it has
// none. Integer ms in, integer ms out; the lower median keeps it deterministic
// and byte-stable for even sample counts.
//
// A median rather than a mean or an extreme on purpose. On honest data the
// spread of this quantity is response latency plus the Date header's 1 s
// truncation — single-digit seconds (measured on the local captures D1/D2/D3:
// min -6176 ms, p50 -752 ms, max +309 ms, i.e. a 6.5 s total spread across 558
// samples). Any statistic would separate that from a ten-minute step, but only
// a median makes moving the statistic cost the attacker more than half of the
// window's Dates.
export function clockOffsetMedianMs(samples) {
  const offsets = [];
  for (const s of samples) {
    const server = s.serverDateMs ?? null;
    if (server === null || s.clientStartMs === null || s.clientStartMs === undefined) continue;
    offsets.push(server - s.clientStartMs);
  }
  if (offsets.length === 0) return null;
  offsets.sort((a, b) => a - b);
  return offsets[(offsets.length - 1) >> 1];
}

// SEAM CORROBORATION: how much time the SERVER timeline says passed across the
// seam between two client windows of one stream, measured WITHOUT reading the
// Dates at the seam.
//
// `segmentWindows` cuts on the client clock alone, so a client-window seam is
// only ever a CLAIM that `clientAdvanceMs` of real time passed. Each window
// carries its own robust offset m = median(serverDate - clientStart), so the
// server time of a sample is (clientStart + m) and the server-side advance
// across the seam is
//
//     serverAdvanceMs = clientAdvanceMs + (mNext - mPrev)
//
// with clientAdvanceMs measured start-to-start, exactly as segmentWindows
// measures the client gap it split on. Two consequences, both exact:
//
//   - A GENUINE PAUSE moves both clocks together, so mNext === mPrev and
//     serverAdvanceMs === clientAdvanceMs. Every honest control keeps its
//     boundary: the ~19 h two-session control, the genuine 11-minute pause, and
//     the near-threshold seam whose 4 s slow poll leaves the medians equal.
//   - A CLIENT-ONLY JUMP of J ms inflates clientAdvanceMs by J and moves mNext
//     by exactly -J, so serverAdvanceMs collapses back to the real advance and
//     the boundary fails. There is no threshold slack to tune against: the two
//     terms cancel identically, whatever J is.
//
// This is the check that closes the deletion+jump split. Deleting a band of
// Dates around the seam widens `suffixMin - prefixMax` and so buys a session
// boundary out of assignServerSessions — but it cannot touch the medians of the
// surviving Dates on either side, and those still testify that the client clock,
// not the world, is what moved. To keep such a split alive the attacker must
// move more than HALF of one window's offsets, and those same Dates are the
// bracket bounds its peak/off-peak evidence rests on: a window needs
// MIN_GROUP_BRACKETS server-informative brackets to qualify a side, i.e. at
// least ten dated samples, so the cheapest forgery is a coordinated rewrite of
// six or more of the very Dates it is claiming as evidence. That is the
// both-clocks bulk rewrite A3 defers — and it is observationally identical to a
// genuine capture pause, which is legitimate evidence.
//
// Returns null when either window has no dated sample: an absent Date makes no
// claim, so it can neither corroborate nor refute (such a window has no server
// bracket bounds either, so it qualifies no A4-2 side).
export function seamServerAdvanceMs(prevWindow, nextWindow) {
  const mPrev = prevWindow.clockOffsetMedianMs ?? null;
  const mNext = nextWindow.clockOffsetMedianMs ?? null;
  if (mPrev === null || mNext === null) return null;
  const from = prevWindow.lastClientStartMs;
  const to = nextWindow.utcStartMs;
  if (typeof from !== "number" || typeof to !== "number") return null;
  return to - from + (mNext - mPrev);
}

// The rule itself, over a plain array of `number | null` server Dates in record
// order.
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
