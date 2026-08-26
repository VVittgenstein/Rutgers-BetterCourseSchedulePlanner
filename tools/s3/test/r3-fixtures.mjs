// Fixture builders for the STAGE-5-R3 counterexamples (CE-15, CE-16).
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
  renumberRows,
  shiftClientClock,
  shiftServerDate,
} from "./fixtures.mjs";
import { denseBaseCapture, straddleSession } from "./r2-fixtures.mjs";

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
