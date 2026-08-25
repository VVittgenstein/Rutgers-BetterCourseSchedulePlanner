// Window segmentation and America/New_York peak-overlap classification.

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

// True iff [utcStartMs, utcEndMs] intersects any America/New_York
// 17:00:00–18:00:00 local interval.
export function overlapsNyPeak(utcStartMs, utcEndMs) {
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
    if (peakStartUtc <= utcEndMs && peakEndUtc >= utcStartMs) {
      return true;
    }
  }
  return false;
}
