import test from "node:test";
import assert from "node:assert/strict";
import { segmentWindows, nyLabel, overlapsNyPeak, overlapsNyPeakStrict } from "../lib/windows.mjs";

function mk(startMs, endMs = startMs + 200) {
  return { clientStartMs: startMs, clientEndMs: endMs };
}

const T0 = Date.UTC(2026, 0, 6, 3, 0, 0);

test("segmentWindows splits one stream on gaps > max(10 min, 5x interval)", () => {
  // interval 13 s → threshold is the 600 000 ms floor
  const samples = [mk(T0), mk(T0 + 13000), mk(T0 + 26000), mk(T0 + 26000 + 700000), mk(T0 + 26000 + 713000)];
  const windows = segmentWindows(samples, 13, "runA/samples.ndjson");
  assert.equal(windows.length, 2);
  assert.equal(windows[0].windowId, "runA/samples.ndjson#w00");
  assert.equal(windows[1].windowId, "runA/samples.ndjson#w01");
  assert.equal(windows[0].samples.length, 3);
  assert.equal(windows[1].samples.length, 2);
  assert.equal(windows[0].utcStartMs, T0);
  assert.equal(windows[0].utcEndMs, T0 + 26000 + 200);
  assert.equal(windows[1].utcStartMs, T0 + 26000 + 700000);
});

test("segmentWindows keeps one window when the gap is under the threshold", () => {
  const samples = [mk(T0), mk(T0 + 13000), mk(T0 + 13000 + 599000)];
  const windows = segmentWindows(samples, 13, "runA/samples.ndjson");
  assert.equal(windows.length, 1);
  assert.equal(windows[0].samples.length, 3);
});

test("segmentWindows scales the gap threshold with 5x the interval", () => {
  // interval 200 s → threshold 1 000 000 ms > the 600 000 floor: a 700 s gap
  // splits at interval 13 but must NOT split at interval 200.
  const samples = [mk(T0), mk(T0 + 700000)];
  assert.equal(segmentWindows(samples, 13, "in").length, 2);
  assert.equal(segmentWindows(samples, 200, "in").length, 1);
  // null interval falls back to 60 s → 300 000 < floor → floor applies → split
  assert.equal(segmentWindows(samples, null, "in").length, 2);
});

test("nyLabel: overnight D1-shaped window crosses the NY calendar day", () => {
  const start = Date.UTC(2026, 7, 20, 3, 37, 1); // 2026-08-19 23:37 EDT
  const end = Date.UTC(2026, 7, 20, 5, 42, 20); // 2026-08-20 01:42 EDT
  assert.equal(nyLabel(start, end), "2026-08-19 23:37–08-20 01:42 ET");
});

test("nyLabel: same NY calendar day omits the second date", () => {
  const start = Date.UTC(2026, 0, 6, 15, 0, 0); // 10:00 EST
  const end = Date.UTC(2026, 0, 6, 16, 30, 0); // 11:30 EST
  assert.equal(nyLabel(start, end), "2026-01-06 10:00–11:30 ET");
});

test("overlapsNyPeak: EDT summer — peak is 21:00-22:00 UTC", () => {
  const day = [2026, 7, 20]; // Aug 20 2026, EDT (UTC-4)
  const utc = (h, m, s = 0) => Date.UTC(day[0], day[1], day[2], h, m, s);
  assert.equal(overlapsNyPeak(utc(21, 30), utc(21, 40)), true, "17:30-17:40 ET inside peak");
  assert.equal(overlapsNyPeak(utc(20, 0), utc(20, 59, 59)), false, "ends 16:59:59 ET, before peak");
  assert.equal(overlapsNyPeak(utc(22, 0, 1), utc(23, 0)), false, "starts 18:00:01 ET, after peak");
  assert.equal(overlapsNyPeak(utc(19, 0), utc(21, 0)), true, "touching 17:00:00 ET exactly counts");
  assert.equal(overlapsNyPeak(utc(20, 30), utc(23, 30)), true, "spanning the whole peak");
});

test("overlapsNyPeak: EST winter — peak is 22:00-23:00 UTC", () => {
  const utc = (h, m) => Date.UTC(2026, 0, 6, h, m, 0);
  assert.equal(overlapsNyPeak(utc(22, 30), utc(22, 40)), true, "17:30-17:40 ET inside peak");
  assert.equal(overlapsNyPeak(utc(21, 30), utc(21, 59)), false, "16:30-16:59 ET (would be peak in EDT)");
});

test("overlapsNyPeak: DST boundary days use that day's afternoon offset", () => {
  // 2026-03-08: DST starts at 02:00 local, so the afternoon is EDT.
  assert.equal(overlapsNyPeak(Date.UTC(2026, 2, 8, 21, 30, 0), Date.UTC(2026, 2, 8, 21, 45, 0)), true);
  assert.equal(overlapsNyPeak(Date.UTC(2026, 2, 8, 22, 30, 0), Date.UTC(2026, 2, 8, 22, 45, 0)), false);
  // 2026-11-01: DST ends at 02:00 local, so the afternoon is EST.
  assert.equal(overlapsNyPeak(Date.UTC(2026, 10, 1, 22, 30, 0), Date.UTC(2026, 10, 1, 22, 45, 0)), true);
  assert.equal(overlapsNyPeak(Date.UTC(2026, 10, 1, 21, 30, 0), Date.UTC(2026, 10, 1, 21, 45, 0)), false);
});

test("overlapsNyPeakStrict: a measure-zero boundary touch is labeled peak but is not peak evidence", () => {
  const peakStart = Date.UTC(2026, 0, 6, 22, 0, 0); // 17:00:00.000 EST
  const peakEnd = Date.UTC(2026, 0, 6, 23, 0, 0); // 18:00:00.000 EST
  // Ends exactly at 17:00:00.000: closed labeling says peak, strict says no.
  assert.equal(overlapsNyPeak(peakStart - 600000, peakStart), true);
  assert.equal(overlapsNyPeakStrict(peakStart - 600000, peakStart), false);
  // Starts exactly at 18:00:00.000: same split.
  assert.equal(overlapsNyPeak(peakEnd, peakEnd + 600000), true);
  assert.equal(overlapsNyPeakStrict(peakEnd, peakEnd + 600000), false);
  // One millisecond of true overlap flips strict to true.
  assert.equal(overlapsNyPeakStrict(peakStart - 600000, peakStart + 1), true);
  assert.equal(overlapsNyPeakStrict(peakEnd - 1, peakEnd + 600000), true);
  // An interval fully inside the hour is peak evidence under both.
  assert.equal(overlapsNyPeakStrict(peakStart + 60000, peakStart + 120000), true);
});

test("overlapsNyPeak: overnight and multi-day windows", () => {
  // D1's actual overnight window: 23:37 ET → 01:42 ET, nowhere near 17:00-18:00.
  assert.equal(overlapsNyPeak(Date.UTC(2026, 7, 20, 3, 37, 1), Date.UTC(2026, 7, 20, 5, 42, 20)), false);
  // A window longer than 24 h necessarily crosses a peak hour.
  assert.equal(overlapsNyPeak(Date.UTC(2026, 7, 20, 0, 0, 0), Date.UTC(2026, 7, 21, 2, 0, 0)), true);
});
