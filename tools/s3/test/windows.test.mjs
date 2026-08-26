import test from "node:test";
import assert from "node:assert/strict";
import {
  segmentWindows,
  assignServerSessions,
  clockOffsetMedianMs,
  seamServerAdvanceMs,
  nyLabel,
  overlapsNyPeak,
  overlapsNyPeakStrict,
} from "../lib/windows.mjs";
import { WINDOW_GAP_MIN_MS } from "../lib/phase.mjs";

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

function sv(startMs, serverMs) {
  return { clientStartMs: startMs, clientEndMs: startMs + 200, serverDateMs: serverMs };
}

test("assignServerSessions splits on server gaps > max(10 min, 5x interval)", () => {
  // interval 13 s -> the 600 000 ms floor applies. The CLIENT column is
  // deliberately kept gapless here: what is under test is the server timeline.
  const samples = [
    sv(T0, T0),
    sv(T0 + 13000, T0 + 13000),
    sv(T0 + 26000, T0 + 26000),
    sv(T0 + 39000, T0 + 26000 + 700000),
    sv(T0 + 52000, T0 + 26000 + 713000),
  ];
  assert.deepEqual(assignServerSessions(samples, 13), [0, 0, 0, 1, 1]);
  // Exactly at the threshold does not split (strict greater-than), one ms over
  // does — the same boundary segmentWindows uses.
  const atThreshold = [sv(T0, T0), sv(T0 + 1000, T0 + WINDOW_GAP_MIN_MS)];
  const overThreshold = [sv(T0, T0), sv(T0 + 1000, T0 + WINDOW_GAP_MIN_MS + 1)];
  assert.deepEqual(assignServerSessions(atThreshold, 13), [0, 0]);
  assert.deepEqual(assignServerSessions(overThreshold, 13), [0, 1]);
});

test("assignServerSessions scales the gap threshold with 5x the interval", () => {
  const samples = [sv(T0, T0), sv(T0 + 1000, T0 + 700000)];
  assert.deepEqual(assignServerSessions(samples, 13), [0, 1]);
  // interval 200 s -> threshold 1 000 000 ms > the floor: no split.
  assert.deepEqual(assignServerSessions(samples, 200), [0, 0]);
  // null interval falls back to 60 s -> 300 000 < floor -> floor applies.
  assert.deepEqual(assignServerSessions(samples, null), [0, 1]);
});

test("assignServerSessions fails closed on missing and non-monotonic serverDates", () => {
  // An ABSENT Date header imposes no grouping constraint at all, wherever it
  // sits: index null, joins no session. It used to INHERIT the running index
  // so that a deleted header could not manufacture a split — but inheriting is
  // a vote for the EARLIER session, i.e. a merge, and deleting the first Date
  // of the later client window made that window carry both indices and pooled
  // it with the earlier one (CE-18, delete direction). Contributing nothing
  // cannot mint a split either: a boundary is only ever placed at a dated
  // sample.
  const holed = [
    sv(T0, T0),
    sv(T0 + 13000, null),
    sv(T0 + 26000, null),
    sv(T0 + 39000, T0 + 39000),
  ];
  assert.deepEqual(assignServerSessions(holed, 13), [0, null, null, 0]);
  // A hole that spans a real server gap still splits at the next dated sample,
  // and the undated sample joins neither side.
  const holedAcrossGap = [
    sv(T0, T0),
    sv(T0 + 13000, null),
    sv(T0 + 26000, T0 + 700000),
  ];
  assert.deepEqual(assignServerSessions(holedAcrossGap, 13), [0, null, 1]);
  // Samples before the first observed Date are the same case, and always were:
  // the server timeline says nothing there.
  const lateFirstDate = [sv(T0, null), sv(T0 + 13000, null), sv(T0 + 26000, T0 + 26000)];
  assert.deepEqual(assignServerSessions(lateFirstDate, 13), [null, null, 0]);
  // No Date anywhere: every index is null and sessions collapse to the client
  // windows, i.e. exactly the pre-session behavior.
  assert.deepEqual(assignServerSessions([sv(T0, null), sv(T0 + 1, null)], 13), [null, null]);
  // Webfarm skew (a negative or zero server difference) never splits.
  const skewed = [sv(T0, T0 + 5000), sv(T0 + 13000, T0), sv(T0 + 26000, T0)];
  assert.deepEqual(assignServerSessions(skewed, 13), [0, 0, 0]);
  assert.deepEqual(assignServerSessions([], 13), []);
});

test("assignServerSessions measures the seam with order statistics, so ONE forged Date cannot mint a split", () => {
  // The seam at i is `earliest Date at or after i` minus `latest Date before
  // i`, never `serverDateMs[i] - serverDateMs[i - 1]`. Both mirror forgeries
  // below reach the adjacent-difference threshold and neither reaches this one.

  // BACKWARDS: the sample that ENDS a group carries a Date dragged 700 s back.
  // The adjacent reading measured the NEXT (honest) sample against that bogus
  // value — 52 000 - (39 000 - 700 000) = 713 000 — and split. prefixMax simply
  // ignores the regression and falls back to the previous honest Date.
  const draggedBack = [
    sv(T0, T0),
    sv(T0 + 13000, T0 + 13000),
    sv(T0 + 26000, T0 + 26000),
    sv(T0 + 39000, T0 + 39000 - 700000),
    sv(T0 + 52000, T0 + 52000),
  ];
  assert.equal(draggedBack[4].serverDateMs - draggedBack[3].serverDateMs, 713000);
  assert.deepEqual(assignServerSessions(draggedBack, 13), [0, 0, 0, 0, 0]);

  // FORWARDS: the sample that would START the new group carries a Date dragged
  // 700 s forward. It cleared the adjacent threshold on its own, and every
  // later sample rejoined it because their difference from the inflated value
  // is negative — so the boundary landed exactly where the forger put it.
  // suffixMin ignores the spike and reports the next honest Date instead.
  const spikedForward = [
    sv(T0, T0),
    sv(T0 + 13000, T0 + 13000),
    sv(T0 + 26000, T0 + 26000 + 700000),
    sv(T0 + 39000, T0 + 39000),
    sv(T0 + 52000, T0 + 52000),
  ];
  assert.equal(spikedForward[2].serverDateMs - spikedForward[1].serverDateMs, 713000);
  assert.deepEqual(assignServerSessions(spikedForward, 13), [0, 0, 0, 0, 0]);

  // A GENUINE separation still splits when a lone spike sits inside the later
  // session: suffixMin is the honest first Date of that session, not the spike.
  const genuineWithSpike = [
    sv(T0, T0),
    sv(T0 + 13000, T0 + 13000),
    sv(T0 + 26000, T0 + 700000),
    sv(T0 + 39000, T0 + 700000 + 900000),
    sv(T0 + 52000, T0 + 713000),
  ];
  assert.deepEqual(assignServerSessions(genuineWithSpike, 13), [0, 0, 1, 1, 1]);

  // COARSENING, stated rather than implied. A Date that falls back into the
  // earlier range AFTER a real gap collapses suffixMin, so this rule reads the
  // two sides as one session. That is a MERGE, and one cell is enough to buy
  // it — breakdown point ONE in this direction, permanently. This function is
  // deliberately NOT the defence against it: A4-2 requires a side to be carried
  // by ONE constituent client window, so a merged session's counts are the best
  // single window's counts and pooling manufactures nothing. What is asserted
  // here is only the reading itself; counterexamples.test.mjs pins the gate
  // consequence for k = 1, 2, 3 and 8 edited cells.
  const genuineWithLateRegression = [
    sv(T0, T0),
    sv(T0 + 13000, T0 + 13000),
    sv(T0 + 26000, T0 + 700000),
    sv(T0 + 39000, T0 + 26000),
  ];
  assert.deepEqual(assignServerSessions(genuineWithLateRegression, 13), [0, 0, 0, 0]);

  // On a NON-DECREASING timeline the two readings coincide exactly, which is
  // why this change is a no-op on every honest capture.
  const monotone = [
    sv(T0, T0),
    sv(T0 + 13000, T0 + 13000),
    sv(T0 + 26000, T0 + 26000 + 700000),
    sv(T0 + 39000, T0 + 39000 + 700000),
  ];
  assert.deepEqual(assignServerSessions(monotone, 13), [0, 0, 1, 1]);
});

// WHAT ASSIGNING SESSIONS DOES AND DOES NOT ESTABLISH, stated exactly, because
// the previous two rounds both overclaimed here.
//
// SPLIT direction (an attacker manufacturing a session boundary): the seam is
// an order statistic over two SETS, so raising it needs every low Date in the
// suffix raised or every high Date in the prefix lowered. The breakdown point
// is the number of dated samples on the smaller side of the seam, not one.
//
// MERGE direction (an attacker suppressing a genuine boundary): breakdown point
// ONE, and no seam statistic can fix that — a single number is always pushable
// by a single cell. v2.7.1/v2.7.2 bolted a leave-one-out grouping check on top;
// it refused honest captures whose genuine seam landed within one polling gap
// of the threshold and it still admitted every two-cell pooling forgery, so it
// was removed. The merge direction is instead harmless by construction: A4-2
// takes each side's count from the BEST SINGLE constituent client window, so a
// merged session can never count more than its best part.
//
// HOLD-OUT IS NOT DELETION IN GENERAL. Now that an absent Date imposes no
// grouping constraint, holding out one cell and deleting it produce the same
// index array — but that equivalence is per-cell only. It does NOT extend to
// "any k-cell rewrite behaves like k deletions": deletions can only widen a
// measured seam (raise suffixMin, lower prefixMax), while an edit can move it
// either way, and two edited cells already reach a grouping no single deletion
// reaches. The test below pins both halves.
test("assignServerSessions: deletion widens a seam, and two cells beat one", () => {
  const twoRealSessions = [
    sv(T0, T0),
    sv(T0 + 13000, T0 + 13000),
    sv(T0 + 26000, T0 + 700000),
    sv(T0 + 39000, T0 + 713000),
  ];
  assert.deepEqual(assignServerSessions(twoRealSessions, 13), [0, 0, 1, 1]);

  // Deleting the first Date of the later session moves the boundary one sample
  // later; the separation itself only grows. Deletion is a widening operation.
  const deletedFirstOfSecond = twoRealSessions.map((s, i) =>
    i === 2 ? sv(s.clientStartMs, null) : s,
  );
  assert.deepEqual(assignServerSessions(deletedFirstOfSecond, 13), [0, 0, null, 1]);

  // ONE edited cell can suppress the boundary entirely — the merge direction's
  // breakdown point of one, asserted rather than argued.
  const oneCellMerge = twoRealSessions.map((s, i) =>
    i === 3 ? sv(s.clientStartMs, T0 + 26000) : s,
  );
  assert.deepEqual(assignServerSessions(oneCellMerge, 13), [0, 0, 0, 0]);

  // ...and it survives holding out any single cell, which is precisely why a
  // leave-one-out check on the GROUPING cannot be the defence: hold out the one
  // edited cell here and the boundary comes back, but with TWO edited cells
  // (below) every single hold-out still shows the merged grouping.
  const twoCellMerge = twoRealSessions.map((s, i) =>
    i >= 2 ? sv(s.clientStartMs, T0 + 26000 + (i - 2) * 1000) : s,
  );
  assert.deepEqual(assignServerSessions(twoCellMerge, 13), [0, 0, 0, 0]);
  for (let j = 0; j < twoCellMerge.length; j += 1) {
    const held = twoCellMerge.map((s, i) => (i === j ? sv(s.clientStartMs, null) : s));
    const idx = assignServerSessions(held, 13).filter((v) => v !== null);
    assert.deepEqual(new Set(idx), new Set([0]), `hold-out ${j} still reads one session`);
  }

  // A stream with no Date anywhere imposes no grouping at all, and the empty
  // stream is not a special case.
  assert.deepEqual(assignServerSessions([sv(T0, null), sv(T0 + 13000, null)], 13), [null, null]);
  assert.deepEqual(assignServerSessions([], 13), []);

  // A regression that moves no boundary (webfarm skew inside one session) is
  // simply read as one session; it is not an error condition.
  const skewedRun = [sv(T0, T0 + 5000), sv(T0 + 13000, T0), sv(T0 + 26000, T0 + 26000)];
  assert.deepEqual(assignServerSessions(skewedRun, 13), [0, 0, 0]);
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

test("clockOffsetMedianMs is the LOWER median over dated samples only", () => {
  assert.equal(clockOffsetMedianMs([]), null);
  assert.equal(clockOffsetMedianMs([sv(T0, null), sv(T0 + 1000, null)]), null);
  // Undated samples are skipped, not counted as zero.
  assert.equal(clockOffsetMedianMs([sv(T0, T0 - 600), sv(T0 + 1000, null)]), -600);
  // Odd count: the middle value. Even count: the LOWER of the two middles, so
  // the result is always one of the observed offsets and never a half-integer.
  const withOffsets = (offsets) => offsets.map((o, i) => sv(T0 + i * 1000, T0 + i * 1000 + o));
  assert.equal(clockOffsetMedianMs(withOffsets([-600, -100, -900])), -600);
  assert.equal(clockOffsetMedianMs(withOffsets([-600, -100, -900, -50])), -600);
  // Robustness is the point: a minority of wildly edited Dates cannot move it.
  const contaminated = [];
  for (let i = 0; i < 11; i += 1) {
    contaminated.push(sv(i * 1000, i * 1000 - 600 - (i < 5 ? 660000 : 0)));
  }
  assert.equal(clockOffsetMedianMs(contaminated), -600, "5 of 11 edited cells move nothing");
  contaminated[5] = sv(5000, 5000 - 600 - 660000);
  assert.equal(clockOffsetMedianMs(contaminated), -660600, "6 of 11 do");
});

test("seamServerAdvanceMs cancels a client-only jump exactly and passes a genuine pause", () => {
  const win = (firstStart, lastStart, offset) => ({
    utcStartMs: firstStart,
    lastClientStartMs: lastStart,
    clockOffsetMedianMs: offset,
  });
  const JUMP = 660000;
  // Honest: 21 s of real time between the two windows' extreme samples, same
  // offset on both sides.
  assert.equal(seamServerAdvanceMs(win(T0, T0 + 21000, -600), win(T0 + 42000, T0 + 60000, -600)), 21000);
  // A genuine pause moves both clocks: the client advance IS the server advance.
  assert.equal(
    seamServerAdvanceMs(win(T0, T0 + 21000, -600), win(T0 + 42000 + JUMP, T0 + 60000, -600)),
    21000 + JUMP,
  );
  // A client-only jump of J inflates the client advance by J and drags the
  // second window's median by exactly -J. The two cancel identically, for every
  // J: what is left is the 21 s that really passed.
  for (const j of [WINDOW_GAP_MIN_MS + 1, JUMP, 3 * JUMP]) {
    assert.equal(
      seamServerAdvanceMs(win(T0, T0 + 21000, -600), win(T0 + 42000 + j, T0 + 60000, -600 - j)),
      21000,
      `jump ${j}`,
    );
  }
  // A window with no dated sample makes no claim, so the seam is unmeasurable
  // — the caller treats null as "not corroborated" and merges.
  assert.equal(seamServerAdvanceMs(win(T0, T0 + 21000, -600), win(T0 + 42000, T0 + 60000, null)), null);
  assert.equal(seamServerAdvanceMs(win(T0, T0 + 21000, null), win(T0 + 42000, T0 + 60000, -600)), null);
});

test("seamServerAdvanceMs is blind to DELETED Dates around the seam", () => {
  // The CE-20 shape in miniature: the client jumps 11 minutes and every Date in
  // a band around the seam is dropped. Dropping changes which samples enter a
  // median, not where the median sits, so the measured advance is unchanged.
  const JUMP = 660000;
  const honestOffsets = new Array(20).fill(-600);
  const before = honestOffsets.map((o, i) => sv(T0 + i * 30000, T0 + i * 30000 + o));
  const afterAll = honestOffsets.map((o, i) =>
    sv(T0 + (20 + i) * 30000 + JUMP, T0 + (20 + i) * 30000 + o),
  );
  // Drop the first twelve Dates of the second window.
  const afterHoled = afterAll.map((s, i) => (i < 12 ? sv(s.clientStartMs, null) : s));
  const mkWin = (rows) => ({
    utcStartMs: rows[0].clientStartMs,
    lastClientStartMs: rows[rows.length - 1].clientStartMs,
    clockOffsetMedianMs: clockOffsetMedianMs(rows),
  });
  const full = seamServerAdvanceMs(mkWin(before), mkWin(afterAll));
  const holed = seamServerAdvanceMs(mkWin(before), mkWin(afterHoled));
  assert.equal(full, 30000);
  assert.equal(holed, 30000, "deleting Dates does not move the corroborated advance");
  assert.ok(full <= WINDOW_GAP_MIN_MS, "and it stays under the threshold, so the seam merges");
});
