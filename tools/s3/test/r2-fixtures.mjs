// Fixture builders for the STAGE-5-R2 counterexamples (CE-9 .. CE-13) and
// their honest controls.
//
// They live in their own module, not inline in counterexamples.test.mjs, for
// one reason: the negative-proof protocol runs the SAME fixture bytes through
// the previous analyzer tree, and that harness must import the builders without
// importing (and thereby running) the test file.
//
// Everything here is deterministic and offline: no RNG, no clock reads, no
// network, no capture data. Each builder writes run directories under `dir` and
// returns the run names in the order the analyzer should be given them.

import { join } from "node:path";
import { makeRunDir, makeTickSeries, sampleRow, renumberRows } from "./fixtures.mjs";

// Jan 6 2026 is EST (UTC-5): the NY 17:00-18:00 peak is 22:00-23:00 UTC.
export const OFF_PEAK_BASE = Date.UTC(2026, 0, 6, 3, 0, 0); // 22:00 ET Jan 5 — off-peak
export const PEAK_BASE = Date.UTC(2026, 0, 6, 22, 10, 0); // 17:10 ET Jan 6 — inside peak

// Per-campus geometry (identical to go-gate.test.mjs): a distinct bodySha
// namespace plus a distinct pre/post/phase, so three HONEST campus series are
// genuinely independent observation data.
export const CAMPUS_SHAPE = {
  NB: { phaseMs: 0, preMs: 400, postMs: 8600 },
  NK: { phaseMs: 300, preMs: 500, postMs: 8400 },
  CM: { phaseMs: 600, preMs: 800, postMs: 8200 },
};

const CAMPUSES = ["NB", "NK", "CM"];
const RUN_NAMES = CAMPUSES.map((c) => `run${c}`);

// ---------------------------------------------------------------------------
// CE-9: three equal-length, staggered, heavily overlapping contiguous slices of
// ONE capture, relabeled NB/NK/CM.
// ---------------------------------------------------------------------------

// The single honest capture both A2-1 fixtures are cut from: 30 off-peak ticks
// plus 30 peak ticks of a true 30 s process = 120 records, 60 change brackets.
const SLICE_SHAPE = { periodMs: 30000, preMs: 400, postMs: 8600, phaseMs: 0, elapsedMs: 200 };
const SLICE_TICKS_PER_SESSION = 30;
export const SLICE_LEN = 80; // samples (40 ticks)
// Even offsets, so every slice starts on a stable sample and none is a
// contiguous slice of another (they are all the same length).
export const SLICE_OFFSETS = { NB: 0, NK: 20, CM: 40 };

export function sliceBaseCapture() {
  return [
    ...makeTickSeries({
      baseMs: OFF_PEAK_BASE, count: SLICE_TICKS_PER_SESSION, startSeq: 1,
      bodyPrefix: "cap-off-v", ...SLICE_SHAPE,
    }),
    ...makeTickSeries({
      baseMs: PEAK_BASE, count: SLICE_TICKS_PER_SESSION, startSeq: 1000,
      bodyPrefix: "cap-peak-v", ...SLICE_SHAPE,
    }),
  ];
}

export function buildOverlapSlices(dir) {
  const capture = sliceBaseCapture();
  for (const campus of CAMPUSES) {
    const offset = SLICE_OFFSETS[campus];
    makeRunDir(join(dir, `run${campus}`), {
      campus,
      samples: renumberRows(capture.slice(offset, offset + SLICE_LEN)),
    });
  }
  return RUN_NAMES;
}

// ---------------------------------------------------------------------------
// CE-10: one dense capture plus two regular, different-stride decimations of
// it, relabeled NB/NK/CM. NK and CM are each a subsample of NB but NEITHER is a
// subsample of the other, so only the transitive union keeps them in one family.
// ---------------------------------------------------------------------------

// Uniform 3 s polling of a true 30 s change process. The tick phase falls
// inside the FIRST poll gap of every stride, so all three streams resolve one
// bracket per tick and their arcs share a left edge; the widest derived server
// bracket (stride 3 → 10 s) stays well under period/2, keeping the safe offset
// identifiable. Without those properties the baseline analyzer would fail
// A4-4/A4-6 on its own and the fixture would prove nothing.
const DENSE = {
  pollMs: 3000,
  samplesPerSession: 300,
  tickPhaseMs: 1000,
  tickPeriodMs: 30000,
  elapsedMs: 200,
};
export const SUBSAMPLE_STRIDES = { NK: 2, CM: 3 };

export function denseBaseCapture() {
  const rows = [];
  let seq = 1;
  for (const [base, prefix] of [
    [OFF_PEAK_BASE, "dense-off-v"],
    [PEAK_BASE, "dense-peak-v"],
  ]) {
    for (let i = 0; i < DENSE.samplesPerSession; i += 1) {
      const rel = i * DENSE.pollMs;
      const version =
        rel < DENSE.tickPhaseMs
          ? 0
          : Math.floor((rel - DENSE.tickPhaseMs) / DENSE.tickPeriodMs) + 1;
      const t = base + rel;
      rows.push(
        sampleRow({
          seq,
          startMs: t,
          elapsedMs: DENSE.elapsedMs,
          bodySha: `${prefix}${version}`,
          serverDateMs: Math.floor(t / 1000) * 1000,
        }),
      );
      seq += 1;
    }
  }
  return rows;
}

export function buildSubsampleDerived(dir) {
  const capture = denseBaseCapture();
  makeRunDir(join(dir, "runNB"), { campus: "NB", samples: renumberRows(capture) });
  for (const [campus, stride] of Object.entries(SUBSAMPLE_STRIDES)) {
    makeRunDir(join(dir, `run${campus}`), {
      campus,
      samples: renumberRows(capture.filter((_, i) => i % stride === 0)),
    });
  }
  return RUN_NAMES;
}

// ---------------------------------------------------------------------------
// CE-11 / CE-11b / CE-12 / CE-13: the A2-2 peak-classification fixtures. All
// four keep the honest three-campus GO shape and edit exactly one recorded
// field family.
// ---------------------------------------------------------------------------

// Fake-peak session: every request START and every serverDate sits at
// 16:20-16:30 ET (off-peak); only requestEndedUtc is dragged to 17:05 ET.
export const FAKE_START_BASE = Date.UTC(2026, 0, 6, 21, 20, 0); // 16:20 ET — OFF-peak
export const FAKE_END_ANCHOR = Date.UTC(2026, 0, 6, 22, 5, 0); // 17:05 ET — inside peak
// Minimal-edit variant: the session runs 16:49:59-16:59:39 ET and every CHANGED
// row simply carries a 150 s request end. That is the smallest end-extension
// that still drags exactly MIN_GROUP_BRACKETS (5) client envelopes across
// 17:00:00 ET — proof the attack needs no absurd elapsed value, and proof that
// no plausible "reject a long elapsed" heuristic could have closed it.
export const SUBTLE_START_BASE = Date.UTC(2026, 0, 6, 21, 50, 0); // 16:50 ET — OFF-peak
export const SUBTLE_CHANGED_ELAPSED_MS = 150000;

function session(campus, baseMs, extra = {}) {
  return makeTickSeries({
    baseMs,
    periodMs: 30000,
    count: 20,
    bodyPrefix: `${campus}-v`,
    ...CAMPUS_SHAPE[campus],
    ...extra,
  });
}

export function offPeakSession(campus, startSeq = 1) {
  return session(campus, OFF_PEAK_BASE, { startSeq });
}
export function honestPeakSession(campus, startSeq = 100) {
  return session(campus, PEAK_BASE, { startSeq });
}
export function fakePeakSession(campus, startSeq = 100) {
  return session(campus, FAKE_START_BASE, {
    startSeq,
    changedEndMs: (k) => FAKE_END_ANCHOR + k * 1000,
  });
}
// The un-edited twin of fakePeakSession: same session, honest 200 ms ends.
export function offPeakLateSession(campus, startSeq = 100) {
  return session(campus, FAKE_START_BASE, { startSeq });
}
export function subtleFakePeakSession(campus, startSeq = 100) {
  const shape = CAMPUS_SHAPE[campus];
  return session(campus, SUBTLE_START_BASE, {
    startSeq,
    changedEndMs: (k) =>
      SUBTLE_START_BASE + k * 30000 + shape.phaseMs + shape.postMs + SUBTLE_CHANGED_ELAPSED_MS,
  });
}

export function buildFakePeakByRequestEnd(dir) {
  for (const campus of CAMPUSES) {
    makeRunDir(join(dir, `run${campus}`), {
      campus,
      samples: [...offPeakSession(campus), ...fakePeakSession(campus)],
    });
  }
  return RUN_NAMES;
}

export function buildSubtleFakePeakByRequestEnd(dir) {
  for (const campus of CAMPUSES) {
    makeRunDir(join(dir, `run${campus}`), {
      campus,
      samples: [...offPeakSession(campus), ...subtleFakePeakSession(campus)],
    });
  }
  return RUN_NAMES;
}

// CE-12: two NB streams of the SAME off-peak capture differing ONLY in
// requestEndedUtc (the edited one dragged into the peak hour), named so the
// edited copy sorts first and wins the representative tie-break. NK and CM are
// honest off-peak only, so the peak side of A4-2 can only come from the edit.
export function buildRepresentativeHijack(dir) {
  makeRunDir(join(dir, "nb-a-edited"), { campus: "NB", samples: fakePeakSession("NB", 1) });
  makeRunDir(join(dir, "nb-b-honest"), { campus: "NB", samples: offPeakLateSession("NB", 1) });
  makeRunDir(join(dir, "runNK"), { campus: "NK", samples: offPeakSession("NK") });
  makeRunDir(join(dir, "runCM"), { campus: "CM", samples: offPeakSession("CM") });
  return ["nb-a-edited", "nb-b-honest", "runNK", "runCM"];
}

// CE-13: the mirror hole on the OFF-PEAK side. Each campus gets (a) a session
// whose client timestamps read 16:10 ET but whose serverDate — the clock the
// comparison runs on — says 17:10 ET, and (b) an honest 17:40 ET peak session.
// Every bit of server evidence in the run lies inside the peak hour.
export const FAKE_OFFPEAK_CLIENT_BASE = Date.UTC(2026, 0, 6, 21, 10, 0); // 16:10 ET client
export const HONEST_LATE_PEAK_BASE = Date.UTC(2026, 0, 6, 22, 40, 0); // 17:40 ET both clocks

export function buildFakeOffPeakByClientClock(dir) {
  for (const campus of CAMPUSES) {
    const fakeOffPeak = session(campus, FAKE_OFFPEAK_CLIENT_BASE, {
      startSeq: 1,
      serverOffsetMs: 3600000, // the server says +1 h: 17:10 ET, inside the peak
    });
    const honestPeak = makeTickSeries({
      baseMs: HONEST_LATE_PEAK_BASE,
      periodMs: 30000,
      count: 20,
      startSeq: 100,
      bodyPrefix: `${campus}-w`,
      ...CAMPUS_SHAPE[campus],
    });
    makeRunDir(join(dir, `run${campus}`), { campus, samples: [...fakeOffPeak, ...honestPeak] });
  }
  return RUN_NAMES;
}

// ---------------------------------------------------------------------------
// CE-14: ONE wall-clock session per campus, straddling 17:00 ET. Nothing here
// is forged — every body, request start, request end and serverDate is honest,
// and both regimes are real ON THE SERVER CLOCK. The point is that a single
// 10-minute session is a single window: it must not satisfy the peak side and
// the off-peak side of A4-2 by itself. v2.5.0 refused this shape because its
// off-peak side demanded a window whose whole envelope lay outside the peak
// hour; a per-bracket rule without the purity clause accepts it.
// ---------------------------------------------------------------------------

// 16:55 ET; 20 ticks of 30 s run the session to 17:04:38 ET, so the first ten
// change brackets close before 17:00 ET and the last ten inside the peak hour.
export const STRADDLE_BASE = Date.UTC(2026, 0, 6, 21, 55, 0);

export function straddleSession(campus, startSeq = 1) {
  return session(campus, STRADDLE_BASE, { startSeq });
}

export function buildStraddlingSingleSession(dir) {
  for (const campus of CAMPUSES) {
    makeRunDir(join(dir, `run${campus}`), { campus, samples: straddleSession(campus) });
  }
  return RUN_NAMES;
}

// ---------------------------------------------------------------------------
// Honest controls. Both MUST keep reaching verdict=GO: they are the guard
// against "fixing" the gates by wiring them permanently shut.
// ---------------------------------------------------------------------------

// Three genuinely independent captures: disjoint bodySha namespaces, different
// tick geometry, zero shared records, zero shared (body, delta) blocks.
export function buildHonestControl(dir) {
  for (const campus of CAMPUSES) {
    makeRunDir(join(dir, `run${campus}`), {
      campus,
      samples: [...offPeakSession(campus), ...honestPeakSession(campus)],
    });
  }
  return RUN_NAMES;
}
