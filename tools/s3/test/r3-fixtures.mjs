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

// ---------------------------------------------------------------------------
// CE-20: CE-16 again, plus the escape hatch v2.8.0 left open — the one the
// STAGE-5-R3 A2-2 charter is actually about.
//
// v2.8.0 merged the two client windows of a jumped session because they shared
// a SERVER session index, and assignServerSessions derives that index from
// `suffixMin - prefixMax` across the seam. Deleting a wide enough band of Date
// headers around the seam removes both order statistics from the region and
// widens the measured seam for free — no Date is edited, only dropped, and
// deletion is a primitive this tree already treats as in scope (CE-8, and
// CE-18's merge-delete mode). With the band chosen so the surviving Dates of
// the first window are all off-peak and the surviving Dates of the second are
// all inside the peak hour, ONE server-contiguous 40-minute capture reached a
// six-gate GO on v2.8.0 while every Date it used as evidence stayed honest.
//
// The geometry is CE-14's straddle stretched to 80 ticks (16:40-17:20 ET), for
// one reason: a deleted band has to be WIDER than max(10 min, 5 x interval)
// before it can widen the seam past the threshold at all, and a 20-tick session
// is only ten minutes long.
//
// Modes:
//   "honest"       no edits — the twin, one client window per campus;
//   "delete"       the band only, no jump — still one client window;
//   "jump"         the CE-16 client jump only, no deletions;
//   "jump-delete"  the escape: jump AND band;
//   "two-jumps"    jump at the band's start AND at its end, so the undated band
//                  becomes a client window of its own that no longer touches
//                  either dated window's seam;
//   "rewrite"      the A3-deferred boundary: jump, band, and the surviving
//                  Dates of the second half moved by the same 11 minutes, i.e.
//                  a capture that really is indistinguishable from an honest
//                  pause. It is expected to reach GO.
// ---------------------------------------------------------------------------

// 16:40 ET. 80 ticks of 30 s run to 17:19:30 ET, so tick 40 lands exactly on
// 17:00 ET: ticks 0..39 close before the peak hour and ticks 40..79 inside it.
export const LONG_STRADDLE_BASE = Date.UTC(2026, 0, 6, 21, 40, 0);
export const LONG_STRADDLE_TICKS = 80;
// The jump starts at tick 40 (row 80) — the peak boundary, which is what makes
// the first client window purely off-peak.
export const LONG_JUMP_FROM_TICK = 40;
// Ticks 40..60 inclusive: 21 ticks = 42 rows = 10.5 minutes of Date headers,
// the smallest band wider than WINDOW_GAP_MIN_MS at this 30 s cadence.
export const DELETED_BAND_FROM_TICK = 40;
export const DELETED_BAND_TO_TICK = 60;

function longStraddleRows(campus) {
  return makeTickSeries({
    baseMs: LONG_STRADDLE_BASE,
    periodMs: 30000,
    count: LONG_STRADDLE_TICKS,
    startSeq: 1,
    bodyPrefix: `${campus}-ls-`,
    ...CAMPUS_SHAPE[campus],
  });
}

// Drops the `serverDate` of every row belonging to ticks [from, to]. Nothing
// else changes: the rows stay, in order, with their bodies and both client
// columns.
function dropServerDates(rows, from, to) {
  return rows.map((row, i) => {
    const tick = Math.floor(i / 2);
    return tick >= from && tick <= to ? { ...row, serverDate: null } : { ...row };
  });
}

export function deleteBandRows(campus, mode) {
  const cut = LONG_JUMP_FROM_TICK * 2;
  const bandEndRow = (DELETED_BAND_TO_TICK + 1) * 2;
  const drop = mode === "jump" ? false : mode !== "honest";
  const rows = drop
    ? dropServerDates(longStraddleRows(campus), DELETED_BAND_FROM_TICK, DELETED_BAND_TO_TICK)
    : longStraddleRows(campus);
  if (mode === "honest" || mode === "delete") return rows;
  if (mode === "two-jumps") {
    return [
      ...rows.slice(0, cut),
      ...shiftClientClock(rows.slice(cut, bandEndRow), CLIENT_JUMP_MS),
      ...shiftClientClock(rows.slice(bandEndRow), 2 * CLIENT_JUMP_MS),
    ];
  }
  const tail = shiftClientClock(rows.slice(cut), CLIENT_JUMP_MS);
  if (mode !== "rewrite") return [...rows.slice(0, cut), ...tail];
  // "rewrite": every Date the second window still carries moves with its client
  // columns, which is what makes the result an honest pause rather than a lie.
  const stillDated = bandEndRow - cut;
  return [
    ...rows.slice(0, cut),
    ...tail.slice(0, stillDated),
    ...shiftServerDate(tail.slice(stillDated), CLIENT_JUMP_MS),
  ];
}

export function buildDeleteBandSplitSession(dir, mode) {
  for (const campus of CAMPUSES) {
    makeRunDir(join(dir, `run${campus}`), { campus, samples: deleteBandRows(campus, mode) });
  }
  return RUN_NAMES;
}

// ---------------------------------------------------------------------------
// CE-21: CE-20 again, plus the escape hatch v2.9.0's seam-corroboration check
// left open — a DECOY client window.
//
// v2.9.0 corroborated a client-window seam from the two windows' median
// (serverDate - requestStart). That reads a statistic OF A WINDOW, and the
// windows are drawn by the CLIENT clock, which is the attacker's. A window with
// ONE dated sample has a median with breakdown point ONE — and minting one is
// free, because the client clock may be stepped as often as you like: step at
// the peak boundary (CE-16's own primitive), step again ONE SAMPLE later, and
// the sample in between becomes a client window of its own. Move that sample's
// Date with its own client column and the decoy looks exactly like an honest
// one-poll pause: it corroborates the seam behind it, so the pure off-peak half
// stands alone as an "independent" session, while the seam in front of it stays
// uncorroborated and merely merges the decoy into the peak half — which costs
// the attacker nothing, since the peak half qualifies on its own window.
//
// Price: ONE Date cell out of 160. The 43 deleted Dates are free (CE-20 already
// established that), no body, no row order and no other Date is touched, and
// the two windows that actually carry the peak and off-peak evidence keep every
// Date they have. Measured on the bytes this builder writes:
//   frozen baseline 2c7b53a87471  verdict=GO qualifier=none brackets=237
//   v2.9.0 (06edcf0)             verdict=GO qualifier=none brackets=237, six gates
//
// Modes:
//   "honest"    no edits at all — one client window per campus;
//   "attack"    the decoy's Date moved with its client column, band deleted;
//   "no-forge"  byte-identical to "attack" except that the decoy's Date is NOT
//               moved. That single cell is the whole price, and v2.9.0 already
//               refused this one.
// ---------------------------------------------------------------------------

// The decoy is the first row of the peak half (tick 40, row 80): stepping the
// client clock again at row 81 leaves it alone in its own client window.
export const DECOY_ROW = LONG_JUMP_FROM_TICK * 2;
// Rows 81..123 = ticks 40.5..61 — the same free band CE-20 uses, shifted one
// row later so it starts after the decoy.
export const DECOY_BAND_FROM_ROW = DECOY_ROW + 1;
export const DECOY_BAND_TO_ROW = (DELETED_BAND_TO_TICK + 1) * 2 + 1;

export function decoyWindowRows(campus, mode) {
  let rows = longStraddleRows(campus).map((row) => ({ ...row }));
  if (mode === "honest") return rows;
  if (mode === "attack") {
    rows[DECOY_ROW] = shiftServerDate([rows[DECOY_ROW]], CLIENT_JUMP_MS)[0];
  } else if (mode !== "no-forge") {
    throw new Error(`decoyWindowRows: unknown mode ${mode}`);
  }
  rows = rows.map((row, i) =>
    i >= DECOY_BAND_FROM_ROW && i <= DECOY_BAND_TO_ROW ? { ...row, serverDate: null } : row,
  );
  return [
    ...rows.slice(0, DECOY_ROW),
    ...shiftClientClock(rows.slice(DECOY_ROW, DECOY_ROW + 1), CLIENT_JUMP_MS),
    ...shiftClientClock(rows.slice(DECOY_ROW + 1), 2 * CLIENT_JUMP_MS),
  ];
}

export function buildDecoyWindowSplitSession(dir, mode) {
  for (const campus of CAMPUSES) {
    makeRunDir(join(dir, `run${campus}`), { campus, samples: decoyWindowRows(campus, mode) });
  }
  return RUN_NAMES;
}

// ---------------------------------------------------------------------------
// CE-22: the same decoy trick at a SECOND geometry, for two Date cells and no
// deleted band at all — proof that CE-21 is a mechanism, not a knife edge.
//
// One honest 30 s polling capture per campus with two hiccups in it:
//   A  40 ticks 16:40:00-16:59:30 ET
//   X   1 tick  17:05:00 ET          (the decoy)
//   B  18 ticks 17:11:00-17:19:30 ET
// The largest adjacent serverDate gap of the HONEST capture is 351 s, well
// under max(10 min, 5 x interval), so honestly this is ONE server-contiguous
// session straddling 17:00 ET — CE-14's forbidden shape.
//
// The attack steps the client clock 5 minutes at X and 5 minutes again at B, so
// X becomes a client window of its own; then it DELETES X's first Date and
// MOVES X's second Date forward by the same 5 minutes as X's client column.
// Two cells out of 59 per capture. Each half alone is refused, and so is the
// unedited twin — the combination is the escape.
//
// Modes: "honest" (no edit), "attack", "delete-only", "forge-only".
// ---------------------------------------------------------------------------

export const DECOY2_JUMP_MS = 300000;
const DECOY2_A_BASE = Date.UTC(2026, 0, 6, 21, 40, 0); // 16:40 ET
const DECOY2_X_BASE = Date.UTC(2026, 0, 6, 22, 5, 0); // 17:05 ET
const DECOY2_B_BASE = Date.UTC(2026, 0, 6, 22, 11, 0); // 17:11 ET
export const DECOY2_A_TICKS = 40;
export const DECOY2_B_TICKS = 18;

export function twoCellDecoyRows(campus, mode) {
  const shape = CAMPUS_SHAPE[campus];
  const chunk = (baseMs, count, startSeq) =>
    makeTickSeries({
      baseMs,
      periodMs: 30000,
      count,
      startSeq,
      bodyPrefix: `${campus}-d2-`,
      ...shape,
    });
  const a = chunk(DECOY2_A_BASE, DECOY2_A_TICKS, 1);
  let x = chunk(DECOY2_X_BASE, 1, 200).map((row) => ({ ...row }));
  const b = chunk(DECOY2_B_BASE, DECOY2_B_TICKS, 300);
  if (mode === "honest") return [...a, ...x, ...b];
  if (mode === "attack" || mode === "delete-only") {
    x = x.map((row, i) => (i === 0 ? { ...row, serverDate: null } : row));
  }
  if (mode === "attack" || mode === "forge-only") {
    x = x.map((row, i) => (i === 1 ? shiftServerDate([row], DECOY2_JUMP_MS)[0] : row));
  }
  if (!["attack", "delete-only", "forge-only"].includes(mode)) {
    throw new Error(`twoCellDecoyRows: unknown mode ${mode}`);
  }
  return [
    ...a,
    ...shiftClientClock(x, DECOY2_JUMP_MS),
    ...shiftClientClock(b, 2 * DECOY2_JUMP_MS),
  ];
}

export function buildTwoCellDecoySplit(dir, mode) {
  for (const campus of CAMPUSES) {
    makeRunDir(join(dir, `run${campus}`), { campus, samples: twoCellDecoyRows(campus, mode) });
  }
  return RUN_NAMES;
}

// ---------------------------------------------------------------------------
// BOUNDARY (A3b): the CHEAPEST forged split that still reaches GO, built so the
// price can be counted instead of argued.
//
// One client jump at the peak boundary, and in the second window every Date is
// dropped except the `keptTicks` ticks that carry its peak evidence. Of those,
// `forgedTicks` move with the client column. Sweeping forgedTicks from 0 to
// keptTicks measures the price directly, and it is not "more than half": the
// server-session index is an ORDER STATISTIC (suffixMin - prefixMax), so a
// single surviving honest Date after the seam collapses suffixMin and pulls the
// two windows back into one server session, whatever the medians say. Every
// Date the far window still carries has to move.
//
// Measured on these bytes, live tree, keptTicks = MIN_GROUP_BRACKETS = 5:
//   forged 0, 2, 4, 6, 8 cells -> NO_PRODUCTION_CHANGE (A4-2 the sole refusal)
//   forged 10 cells            -> GO, all six gates
// and with fewer kept ticks the far window cannot reach MIN_GROUP_BRACKETS at
// all: 2, 3, 4 kept ticks all fully forged -> NO-GO. So the minimum is
// 2 * MIN_GROUP_BRACKETS = 10 Date cells per capture at this geometry (one
// stable and one changed endpoint per bracket), and a geometry where every poll
// sees a change could not go below MIN_GROUP_BRACKETS + 1 = 6.
//
// At 10 forged cells the result is a capture whose second window moved on BOTH
// clocks — indistinguishable from a real 11-minute pause, which is legitimate
// evidence. It reaches GO on the frozen baseline, on v2.9.0 and here, and stays
// deferred per A3.
// ---------------------------------------------------------------------------

export const MINIMAL_SPLIT_KEPT_TICKS = 5;

export function minimalForgedSplitRows(campus, forgedTicks, keptTicks = MINIMAL_SPLIT_KEPT_TICKS) {
  const rows = longStraddleRows(campus).map((row, i) => {
    if (i < DECOY_ROW) return { ...row };
    const tickIndex = Math.floor(i / 2) - LONG_JUMP_FROM_TICK;
    if (tickIndex >= keptTicks) return { ...row, serverDate: null };
    return tickIndex < forgedTicks ? shiftServerDate([row], CLIENT_JUMP_MS)[0] : { ...row };
  });
  return [
    ...rows.slice(0, DECOY_ROW),
    ...shiftClientClock(rows.slice(DECOY_ROW), CLIENT_JUMP_MS),
  ];
}

export function buildMinimalForgedSplit(dir, forgedTicks, keptTicks = MINIMAL_SPLIT_KEPT_TICKS) {
  for (const campus of CAMPUSES) {
    makeRunDir(join(dir, `run${campus}`), {
      campus,
      samples: minimalForgedSplitRows(campus, forgedTicks, keptTicks),
    });
  }
  return RUN_NAMES;
}

// ---------------------------------------------------------------------------
// CONTROL (R4): an honest capture with a SHORT window in the middle of it.
//
// The anti-lockout control for this round's rule, and the fixture that decides
// between the two candidate fixes for CE-21. Per campus, three honest windows on
// BOTH clocks, nothing edited anywhere:
//   #w00  the 20-tick off-peak session   (22:00 ET the previous evening);
//   #w01  ONE tick at 12:00 ET           — a stub: two rows, two Dates, one
//         bracket, far too little to qualify any A4-2 side;
//   #w02  the 20-tick peak session       (17:10 ET).
// Every gap is real on both clocks, so every seam is corroborated and the three
// windows stay three evidence sessions: the off-peak side comes from #w00 and
// the peak side from #w02, exactly as in the honest control. GO.
//
// A POPULATION FLOOR on the corroborating window — "a window with fewer than F
// dated samples cannot corroborate its seams, so they fail closed" — was the
// obvious answer to CE-21 and it FAILS HERE for any F > 2: the stub then
// corroborates neither of its seams, merges with BOTH neighbours, and the single
// resulting session holds peak-hour brackets, so it is not pure and the off-peak
// side dies. An honest capture would lose its evidence because one of its
// windows was short. Voiding on a DEMONSTRATED clock step instead costs this
// fixture nothing, because it has no step to demonstrate.
// ---------------------------------------------------------------------------

// 12:00 ET Jan 6 — between the off-peak session and the peak session on both
// clocks, and comfortably outside the peak hour.
export const SHORT_WINDOW_BASE = Date.UTC(2026, 0, 6, 17, 0, 0);

export function buildHonestShortWindowControl(dir) {
  for (const campus of CAMPUSES) {
    const stub = makeTickSeries({
      baseMs: SHORT_WINDOW_BASE,
      periodMs: 30000,
      count: 1,
      startSeq: 50,
      bodyPrefix: `${campus}-stub-`,
      ...CAMPUS_SHAPE[campus],
    });
    makeRunDir(join(dir, `run${campus}`), {
      campus,
      samples: [...offPeakSession(campus), ...stub, ...honestPeakSession(campus)],
    });
  }
  return RUN_NAMES;
}
