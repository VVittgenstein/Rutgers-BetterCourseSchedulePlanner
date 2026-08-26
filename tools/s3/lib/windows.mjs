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
// let ONE forged Date mint a session boundary from either side:
//   - drag the last Date of a client window BACKWARDS and the next honest
//     sample's difference is measured from a value the timeline itself
//     contradicts, so the honest sample "jumps forward" past the threshold;
//   - drag the first Date of the next client window FORWARDS past the
//     threshold and it splits on its own, while every later sample rejoins it
//     because their difference from the inflated value is negative.
// Both forgeries put the manufactured boundary exactly at a client-window edge,
// which is the only place it buys a second independent evidence group.
// Under the order-statistic form a single edited Date moves the measured seam
// by at most the distance from the extreme sample to the next one — one polling
// interval — because prefixMax falls back to the second-latest Date and
// suffixMin to the second-earliest. Manufacturing a > gapMs seam therefore
// costs an attacker a bulk rewrite of the whole tail of a window, not one cell.
//
// On a NON-DECREASING server timeline the two forms are identical: prefixMax(i)
// is then serverDateMs[i-1] and suffixMin(i) is serverDateMs[i]. Real captures
// are non-decreasing (local-data.test.mjs asserts it on D1/D2/D3), so this is a
// no-op on honest data. In general suffixMin(i) - prefixMax(i) <= the adjacent
// difference, so the new rule splits only where the old one did: sessions are a
// COARSENING, and A4-2 can only get stricter, never more permissive.
//
// Two degenerate cases, both resolved in the MERGING (fail-closed) direction,
// because a session split is what buys an attacker a second independent
// evidence group:
//   - a sample whose serverDate is absent inherits the current index, so
//     DELETING a Date header can never manufacture a split, and an undated
//     sample sitting across a real gap stays with the EARLIER session (the
//     boundary is only ever placed at a dated sample);
//   - samples before the first observed serverDate carry null — "the server
//     timeline says nothing here" — so they impose no grouping constraint at
//     all. Their brackets have no server bounds, classify as no-bounds, can
//     supply no evidence, and still break their session's off-peak purity, so
//     the null case buys an attacker nothing either.
// A negative or zero measured seam (webfarm skew) never splits.
export function assignServerSessions(samples, intervalSeconds) {
  const gapMs = Math.max(
    WINDOW_GAP_MIN_MS,
    WINDOW_GAP_INTERVAL_FACTOR * (intervalSeconds ?? 60) * 1000,
  );
  const n = samples.length;
  const serverMs = samples.map((s) => s.serverDateMs ?? null);
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
      indices.push(current);
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
