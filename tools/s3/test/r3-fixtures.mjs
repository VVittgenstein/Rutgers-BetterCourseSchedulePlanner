// Fixture builders for the STAGE-5-R3 counterexamples (CE-15..CE-18).
//
// They live beside r2-fixtures.mjs, and for the same reason: the negative-proof
// protocol drives the SAME fixture bytes through the previous analyzer tree,
// and that harness must import the builders without importing (and thereby
// running) the test file.
//
// Everything here is deterministic and offline: no RNG, no clock reads, no
// network, no capture data. Each builder writes run directories under `dir` and
// returns the run names in the order the analyzer should be given them.
//
// Both fixtures are built by EDITING ONE COLUMN FAMILY of an honest capture
// that r2-fixtures.mjs already produces:
//   CE-15 = denseBaseCapture() + regular stride + one constant client shift;
//   CE-16 = straddleSession()  + one constant client shift on the second half.
// Nothing is fabricated: every body hash, every serverDate and the whole row
// order are the honest capture's own.

import { join } from "node:path";
import {
  makeRunDir,
  makeTickSeries,
  renumberRows,
  sampleRow,
  shiftClientClock,
  shiftServerDate,
} from "./fixtures.mjs";
import {
  CAMPUS_SHAPE,
  PEAK_BASE,
  denseBaseCapture,
  honestPeakSession,
  offPeakSession,
  straddleSession,
} from "./r2-fixtures.mjs";

const CAMPUSES = ["NB", "NK", "CM"];
const RUN_NAMES = CAMPUSES.map((c) => `run${c}`);

// ---------------------------------------------------------------------------
// CE-15: CE-10's shape plus the escape hatch it left open. NB is the dense
// base capture; NK is its stride-2 subsample with EVERY client timestamp moved
// +1 ms; CM is its stride-3 subsample moved +2 ms. Bodies, serverDates and row
// order are untouched, so all three streams still describe the same observed
// data — only the client clock of the two copies was translated once.
//
// Under an ABSOLUTE (clientStartMs, bodySha) record key the +1/+2 ms
// translation changes every key, the derived relation finds nothing, and the
// three streams pose as three independent campuses.
// ---------------------------------------------------------------------------

export const TRANSLATED_SUBSAMPLE = {
  NK: { stride: 2, shiftMs: 1 },
  CM: { stride: 3, shiftMs: 2 },
};

export function buildTranslatedSubsampleDerived(dir) {
  const capture = denseBaseCapture();
  makeRunDir(join(dir, "runNB"), { campus: "NB", samples: renumberRows(capture) });
  for (const [campus, { stride, shiftMs }] of Object.entries(TRANSLATED_SUBSAMPLE)) {
    makeRunDir(join(dir, `run${campus}`), {
      campus,
      samples: renumberRows(
        shiftClientClock(
          capture.filter((_, i) => i % stride === 0),
          shiftMs,
        ),
      ),
    });
  }
  return RUN_NAMES;
}

// ---------------------------------------------------------------------------
// CE-16: CE-14's honest straddling session with ONE edit. Each campus runs a
// single server-contiguous 20-tick session from 16:54:59 ET to 17:04:38 ET;
// from tick 10 onward the CLIENT clock jumps forward 11 minutes. serverDate,
// bodies and row order are untouched, so on the server timeline this is still
// one uninterrupted session whose maximum adjacent gap is ~21 s.
//
// The client jump exceeds the window gap rule, so client segmentation mints
// #w00 and #w01. Grouping A4-2's evidence by client windowId then lets ONE
// server-contiguous session supply the off-peak side from its first half and
// the peak side from its second — which is exactly CE-14 with a forged claim
// of independence bolted on.
// ---------------------------------------------------------------------------

// 11 minutes: strictly greater than max(WINDOW_GAP_MIN_MS, 5 x interval) for
// this fixture's intervalSeconds of 13, which is where the client split comes
// from. The tests re-derive that inequality rather than trusting this comment.
export const CLIENT_JUMP_MS = 660000;
// Rows 0..19 are ticks 0..9 (two rows per tick); the jump starts at row 20.
export const CLIENT_JUMP_FROM_ROW = 20;

export function buildClientJumpSplitSession(dir) {
  for (const campus of CAMPUSES) {
    const rows = straddleSession(campus);
    const head = rows.slice(0, CLIENT_JUMP_FROM_ROW);
    const tail = shiftClientClock(rows.slice(CLIENT_JUMP_FROM_ROW), CLIENT_JUMP_MS);
    makeRunDir(join(dir, `run${campus}`), { campus, samples: [...head, ...tail] });
  }
  return RUN_NAMES;
}

// ---------------------------------------------------------------------------
// HONEST CONTROL for CE-16, and the tightest one available: byte-for-byte the
// CE-16 fixture EXCEPT that the second half's serverDate moves by the same
// 11 minutes as its client columns. The capture really did pause; the two
// halves really are two independent sessions, separated on BOTH clocks; the
// first is genuinely off-peak and the second genuinely inside the peak hour.
//
// It must keep reaching GO. It differs from CE-16 in exactly one thing — that
// the server clock agrees — so a "fix" keyed on "a client jump is present", on
// "the session crosses 17:00 ET", or on rejecting short inter-session gaps
// would wrongly kill it. 660000 ms is a whole multiple of the 30 s tick period,
// so the change phase is preserved exactly and A4-4/A4-6 stay satisfied: what
// this control tests is the gap rule, not a phase artifact.
// ---------------------------------------------------------------------------

export function buildHonestPausedSession(dir) {
  for (const campus of CAMPUSES) {
    const rows = straddleSession(campus);
    const head = rows.slice(0, CLIENT_JUMP_FROM_ROW);
    const tail = shiftServerDate(
      shiftClientClock(rows.slice(CLIENT_JUMP_FROM_ROW), CLIENT_JUMP_MS),
      CLIENT_JUMP_MS,
    );
    makeRunDir(join(dir, `run${campus}`), { campus, samples: [...head, ...tail] });
  }
  return RUN_NAMES;
}

// ---------------------------------------------------------------------------
// CE-17: CE-16 again, plus the escape hatch the first A2-2 fix left open.
//
// Grouping A4-2's evidence by "client windows that share a server session
// index" only helps while the server session index is itself hard to forge.
// v2.7.0 derived it from the difference between ADJACENT Date headers, and that
// let ONE edited Date manufacture a server-session boundary — placed, of
// course, exactly at the client-window seam, the single position where a server
// split leaves the two client windows in disjoint sessions and the merge never
// happens. Two mirror-image forgeries, one cell each:
//
//   BACKWARDS: drag the LAST Date of #w00 back 700 s. v2.7.0 refused to split
//     on the negative difference but still adopted the regressed value as the
//     reference, so the next (honest) sample's difference was measured as
//     9 s + 700 s = 709 s and split.
//   FORWARDS: drag the FIRST Date of #w01 forward 700 s. That sample split on
//     its own, and every later sample rejoined it because its difference from
//     the inflated reference is negative — so the boundary landed on the seam
//     and nowhere else.
//
// The seam sits at row 21, the STABLE row of tick 10: it ends #w00 without
// being a bracket endpoint inside it, so neither edit corrupts a bracket bound
// and the off-peak purity of the first half survives. (Put the same edit on a
// CHANGED row and the bracket loses its server bounds, purity breaks and the
// run already failed closed — which is why the hole needed its own fixture.)
//
// Everything else is CE-16: bodies, row order and the other 39 Date headers are
// the honest capture's own, and the real adjacent server gaps never exceed 21 s.
// ---------------------------------------------------------------------------

// Row 21 is the stable row of tick 10 — see above. CE-16 cuts one row earlier,
// on a changed row, which is why CE-16 needs no Date edit at all.
export const SEAM_JUMP_FROM_ROW = 21;
// 700 s: comfortably over max(WINDOW_GAP_MIN_MS, 5 x interval) for this
// fixture's intervalSeconds of 13. The tests re-derive that from lib/phase.mjs.
export const SEAM_FORGERY_MS = 700000;

function bumpOneServerDate(rows, index, deltaMs) {
  return rows.map((row, i) => (i === index ? shiftServerDate([row], deltaMs)[0] : { ...row }));
}

// direction: "backwards" edits the last Date of #w00, "forwards" the first Date
// of #w01. Exactly ONE serverDate cell differs from the CE-16 shape either way.
export function buildForgedServerSeam(dir, direction) {
  for (const campus of CAMPUSES) {
    const rows = straddleSession(campus);
    let head = rows.slice(0, SEAM_JUMP_FROM_ROW);
    let tail = shiftClientClock(rows.slice(SEAM_JUMP_FROM_ROW), CLIENT_JUMP_MS);
    if (direction === "backwards") {
      head = bumpOneServerDate(head, head.length - 1, -SEAM_FORGERY_MS);
    } else if (direction === "forwards") {
      tail = bumpOneServerDate(tail, 0, SEAM_FORGERY_MS);
    } else {
      throw new Error(`buildForgedServerSeam: unknown direction ${direction}`);
    }
    makeRunDir(join(dir, `run${campus}`), { campus, samples: [...head, ...tail] });
  }
  return RUN_NAMES;
}

// ---------------------------------------------------------------------------
// CE-18: the MIRROR of CE-17, and the hole CE-17's own fix opened.
//
// CE-17 hardened the seam against a forged SPLIT by measuring it with order
// statistics — earliest Date at or after the seam minus latest Date before it.
// Both of those statistics have breakdown point ONE in the other direction: a
// single Date placed low anywhere in the suffix collapses the minimum, and a
// single Date placed high anywhere in the prefix inflates the maximum. So one
// cell could no longer create a boundary but could still SUPPRESS one, pooling
// two genuinely server-separated sessions into one.
//
// A merge is not the fail-closed direction. A4-2 counts informative brackets
// PER SESSION against MIN_GROUP_BRACKETS, so two honest chunks holding four
// in-peak brackets each — neither of which qualifies — pool into one session
// holding seven, which does.
//
// The fixture is three honest pieces of the tree's own honest capture, in one
// run per campus:
//   - offPeakSession(campus): the honest 20-tick off-peak session, which
//     supplies A4-2's off-peak side exactly as it does in the honest control;
//   - two honest 4-tick peak chunks, 25 minutes apart on BOTH clocks. 25 min is
//     far over max(10 min, 5 x 13 s), so the client windowing and every server
//     seam rule agree they are two sessions. Four ticks each is BELOW
//     MIN_GROUP_BRACKETS, so honestly neither chunk can supply the peak side.
// Nothing is fabricated: bodies, row order, request columns and every other
// Date header are the generator's own, and the honest mode edits nothing.
//
// The two forged modes each move exactly ONE serverDate cell by the same
// 25 minutes, in a position that is NOT adjacent to the seam — which is what
// makes them strictly worse than CE-17's, since the adjacent-pair rule v2.7.0
// used still refused both:
//   merge-backwards: the LAST Date of the later chunk dragged back into the
//     earlier chunk's range, collapsing the suffix minimum at the seam;
//   merge-forwards:  the FIRST Date of the earlier chunk dragged forward into
//     the later chunk's range, inflating the prefix maximum at the seam.
// ---------------------------------------------------------------------------

// 25 minutes: over max(WINDOW_GAP_MIN_MS, 5 x interval) for this fixture's
// intervalSeconds of 13 on BOTH clocks. The tests re-derive that from
// lib/phase.mjs rather than trusting this comment.
export const POOLED_CHUNK_GAP_MS = 1500000;
// Ticks per peak chunk. Each tick yields one change bracket, so 4 is one below
// MIN_GROUP_BRACKETS = 5: the pooling is the ONLY thing that could qualify the
// peak side.
export const POOLED_CHUNK_TICKS = 4;

function peakChunk(campus, baseMs, startSeq, prefix) {
  return makeTickSeries({
    baseMs,
    periodMs: 30000,
    count: POOLED_CHUNK_TICKS,
    startSeq,
    bodyPrefix: `${campus}-${prefix}-`,
    ...CAMPUS_SHAPE[campus],
  });
}

// mode: "honest" (no edit at all — the legitimate NO-GO baseline),
// "merge-backwards" / "merge-forwards" (exactly one serverDate cell moved), or
// "merge-delete" (exactly one serverDate cell REMOVED — the third mirror: an
// undated sample that inherits the running session index votes for the earlier
// session, which unions the two client windows just as effectively as a moved
// Date. CE-8's primitive applied to CE-18's gate).
export function buildPooledPeakChunks(dir, mode) {
  for (const campus of CAMPUSES) {
    let first = peakChunk(campus, PEAK_BASE, 100, "p1");
    let second = peakChunk(campus, PEAK_BASE + POOLED_CHUNK_GAP_MS, 500, "p2");
    if (mode === "merge-backwards") {
      second = bumpOneServerDate(second, second.length - 1, -POOLED_CHUNK_GAP_MS);
    } else if (mode === "merge-forwards") {
      first = bumpOneServerDate(first, 0, POOLED_CHUNK_GAP_MS);
    } else if (mode === "merge-delete") {
      second = second.map((row, i) => (i === 0 ? { ...row, serverDate: null } : { ...row }));
    } else if (mode !== "honest") {
      throw new Error(`buildPooledPeakChunks: unknown mode ${mode}`);
    }
    makeRunDir(join(dir, `run${campus}`), {
      campus,
      samples: [...offPeakSession(campus), ...first, ...second],
    });
  }
  return RUN_NAMES;
}

// The row index, inside one run, of the single edited cell — used by the test
// to prove the fixture differs from its honest twin in exactly one place.
export function pooledForgedRowIndex(mode) {
  const offPeakRows = 40; // offPeakSession: 20 ticks x 2 rows
  const chunkRows = POOLED_CHUNK_TICKS * 2;
  if (mode === "merge-backwards") return offPeakRows + 2 * chunkRows - 1;
  if (mode === "merge-forwards") return offPeakRows;
  if (mode === "merge-delete") return offPeakRows + chunkRows;
  throw new Error(`pooledForgedRowIndex: unknown mode ${mode}`);
}

// ---------------------------------------------------------------------------
// CE-19: CE-18's pooling forgery again, with k edited Date cells instead of 1.
//
// v2.7.1/v2.7.2 answered CE-18 with a leave-one-out check on the window
// GROUPING: recompute it with each dated sample's Date held out, and void the
// stream's evidence if any hold-out regroups the windows. That closes k = 1 by
// construction and NOTHING else. Two low Dates in the later chunk keep the
// suffix minimum collapsed under every single hold-out, so the grouping is
// stable, the check stays silent, and the pooled session qualifies the peak
// side with brackets no single client window ever held.
//
// Measured on d6ce282 (v2.7.2), on the bytes this builder writes:
//   k = 1  -> NO_PRODUCTION_CHANGE (the leave-one-out check fires)
//   k = 2  -> GO qualifier=none brackets=84 distinguishable=true
//   k = 3  -> GO ...
//   k = 8  -> GO ...
// while the frozen A1 baseline 2c7b53a87471 refuses all four (4 in-peak
// brackets per client window, one below MIN_GROUP_BRACKETS). Accepting what the
// frozen baseline refuses is the violation; the fixture pins it closed.
//
// Every edited cell moves by the same real gap the two chunks are apart, so the
// forged Dates are values the capture itself produced elsewhere.
// ---------------------------------------------------------------------------

export function buildPooledPeakChunksMultiCell(dir, cellCount) {
  for (const campus of CAMPUSES) {
    const first = peakChunk(campus, PEAK_BASE, 100, "p1");
    const second = peakChunk(campus, PEAK_BASE + POOLED_CHUNK_GAP_MS, 500, "p2").map((row, i) =>
      i < cellCount ? shiftServerDate([row], -POOLED_CHUNK_GAP_MS)[0] : { ...row },
    );
    makeRunDir(join(dir, `run${campus}`), {
      campus,
      samples: [...offPeakSession(campus), ...first, ...second],
    });
  }
  return RUN_NAMES;
}

// ---------------------------------------------------------------------------
// CONTROL (R3c): the honest capture the leave-one-out check refused.
//
// Four client windows per campus, every one of them honest:
//   #w00  the 20-tick off-peak session (22:00 ET the previous evening);
//   #w01  the 20-tick peak session     (17:10 ET);
//   #w02  a 20-tick late off-peak chunk (18:30 ET), closed by ONE extra stable
//         poll that was SLOW — the response Date lands SLOW_POLL_MS after the
//         request start, which is what a slow response records. It repeats the
//         body the previous sample already carried and is not a bracket
//         endpoint, so it changes no bracket bound;
//   #w03  a 20-tick chunk after a GENUINE pause.
//
// The pause is real on both clocks. Because the last poll before it was slow,
// the client gap (measured between request starts) is SLOW_POLL_MS LONGER than
// the server-measured seam (between Date headers) — routine, and enough to put
// the seam just under the session-gap threshold while the client gap sits just
// over it. So the two late chunks are one evidence session, which is what the
// server timeline actually says, and #w00 and #w01 are untouched: the off-peak
// and peak sides are supplied by two windows separated by 19 hours and 70
// minutes respectively.
//
// v2.7.1/v2.7.2 answered NO_PRODUCTION_CHANGE on these bytes: the near-threshold
// seam made one hold-out regroup the stream, and the void is STREAM-WIDE, so it
// took #w00 and #w01 with it. The frozen A1 baseline 2c7b53a87471 answers GO,
// and so must any successor.
// ---------------------------------------------------------------------------

// One slow poll: 4 s from request start to the server's Date. Small enough to
// be unremarkable, large enough to exceed the 1 s Date truncation.
export const SLOW_POLL_MS = 4000;
// 18:30 ET — comfortably after the peak hour, so #w02/#w03 are pure off-peak.
export const LATE_OFFPEAK_BASE = Date.UTC(2026, 0, 6, 23, 30, 0);
// Client gap across the genuine pause. Above WINDOW_GAP_MIN_MS (so the client
// really does split) and within SLOW_POLL_MS of it (so the server-measured seam
// lands just below). The test re-derives both inequalities from lib/phase.mjs.
export const NEAR_THRESHOLD_CLIENT_GAP_MS = 602000;

export function buildHonestNearThresholdSeam(dir) {
  for (const campus of CAMPUSES) {
    const shape = CAMPUS_SHAPE[campus];
    const late = makeTickSeries({
      baseMs: LATE_OFFPEAK_BASE,
      periodMs: 30000,
      count: 20,
      startSeq: 900,
      bodyPrefix: `${campus}-l1-`,
      ...shape,
    });
    const lastRow = late[late.length - 1];
    const slowStartMs = Date.parse(lastRow.requestStartedUtc) + 20000;
    late.push(
      sampleRow({
        seq: 1200,
        startMs: slowStartMs,
        elapsedMs: SLOW_POLL_MS + 200,
        bodySha: lastRow.decodedBodySha256,
        serverDateMs: Math.floor((slowStartMs + SLOW_POLL_MS) / 1000) * 1000,
      }),
    );
    // Place the resumed chunk so its FIRST request start sits exactly
    // NEAR_THRESHOLD_CLIENT_GAP_MS after the slow poll's request start.
    const resumedBase =
      slowStartMs + NEAR_THRESHOLD_CLIENT_GAP_MS - shape.phaseMs + shape.preMs;
    const resumed = makeTickSeries({
      baseMs: resumedBase,
      periodMs: 30000,
      count: 20,
      startSeq: 1300,
      bodyPrefix: `${campus}-l2-`,
      ...shape,
    });
    makeRunDir(join(dir, `run${campus}`), {
      campus,
      samples: [...offPeakSession(campus), ...honestPeakSession(campus), ...late, ...resumed],
    });
  }
  return RUN_NAMES;
}
