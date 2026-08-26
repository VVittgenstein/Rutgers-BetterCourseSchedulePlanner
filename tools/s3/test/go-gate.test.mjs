// Gate-positive path: verdict=GO must be reachable when all six A4 gates are
// genuinely satisfied — NB+NK+CM targets, peak and off-peak windows produced
// by gap-based splitting inside each input, a strict 30 s coverage win
// confirmed by a non-degenerate holdout, an identifiable safe offset, and
// stability under single-group leave-out.

import test from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import { makeTmpDir, cleanup, makeRunDir, makeTickSeries, runAnalyzer } from "./fixtures.mjs";

// Jan 6 2026 is EST (UTC-5): the NY 17:00-18:00 peak is 22:00-23:00 UTC.
const OFF_PEAK_BASE = Date.UTC(2026, 0, 6, 3, 0, 0); // 22:00 ET Jan 5 — off-peak
const PEAK_BASE = Date.UTC(2026, 0, 6, 22, 10, 0); // 17:10 ET Jan 6 — inside peak

// Per-campus series shapes: each campus gets its own bodySha namespace and its
// own pre/post/phase geometry so the three observation series are genuinely
// independent under the provenance fingerprint (a relabeled copy would merge
// into one class and fail A4-1). Phases stay within the arc overlap and every
// stable sample still floors to the second below its tick, so per-group phase
// intervals keep a common intersection (A4-4).
const CAMPUS_SHAPE = {
  NB: { phaseMs: 0, preMs: 400, postMs: 8600 },
  NK: { phaseMs: 300, preMs: 500, postMs: 8400 },
  CM: { phaseMs: 600, preMs: 800, postMs: 8200 },
};

function makeCampusRun(dir, campus) {
  // One input stream with two sessions ~19 h apart: gap-based segmentation
  // must split it into #w00 (off-peak) and #w01 (peak). Both sessions are a
  // true 30 s process (20 ticks each → 20 brackets per window).
  const shape = CAMPUS_SHAPE[campus];
  const offPeak = makeTickSeries({
    baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1,
    bodyPrefix: `${campus}-v`, ...shape,
  });
  const peak = makeTickSeries({
    baseMs: PEAK_BASE, periodMs: 30000, count: 20, startSeq: 100,
    bodyPrefix: `${campus}-v`, ...shape,
  });
  makeRunDir(dir, { campus, samples: [...offPeak, ...peak] });
}

test("all six A4 gates satisfiable → verdict GO (never hardcoded NO)", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  for (const campus of ["NB", "NK", "CM"]) {
    makeCampusRun(join(dir, `run${campus}`), campus);
  }
  const out = runAnalyzer([
    "--ndjson", join(dir, "runNB"),
    "--ndjson", join(dir, "runNK"),
    "--ndjson", join(dir, "runCM"),
    "--out-json", join(dir, "out.json"),
    "--out-md", join(dir, "out.md"),
  ]);
  assert.equal(out.code, 0, out.stderr);
  assert.match(out.stdout, /verdict=GO qualifier=none/);

  const json = out.json;
  assert.equal(json.decision.verdict, "GO");
  assert.equal(json.decision.qualifier, undefined);
  assert.deepEqual(json.decision.reasons, []);
  for (const gate of json.goGate) {
    assert.equal(gate.satisfied, true, `${gate.id} must be satisfied: ${gate.evidence}`);
  }
  assert.equal(json.goGate.length, 6);

  // A4-1: all three campuses evaluable from three independent provenance classes.
  const a1 = json.goGate.find((g) => g.id === "A4-1");
  for (const campus of ["NB", "NK", "CM"]) {
    assert.match(a1.evidence, new RegExp(`${campus}\\(pc-[0-9a-f]{12}\\)`));
  }
  assert.match(a1.evidence, /from 3 provenance classes/);
  assert.equal(json.provenance.classes.length, 3);
  const classCampuses = json.provenance.classes.map((c) => c.campus).sort();
  assert.deepEqual(classCampuses, ["CM", "NB", "NK"]);
  for (const cls of json.provenance.classes) {
    assert.equal(cls.campusConflict, false);
    assert.equal(cls.timeConflict, false);
    assert.equal(cls.members.length, 1);
    assert.equal(cls.members[0].relation, "representative");
  }
  assert.equal(json.provenance.streams.length, 3);
  assert.equal(
    new Set(json.provenance.streams.map((s) => s.seriesFingerprint)).size,
    3,
    "the three campus series must have distinct observation fingerprints",
  );
  // Nothing is contested: every stream is evidence-eligible.
  assert.deepEqual(json.provenance.excludedStreamIds, []);
  assert.deepEqual(json.provenance.duplicateStreamIds, []);
  assert.equal(json.bracketTotals.excludedFromEvidence, 0);

  // Window segmentation: each of the 3 inputs split into #w00 + #w01.
  const windows = json.targets.flatMap((tgt) => tgt.windows);
  assert.equal(windows.length, 6);
  assert.equal(windows.filter((w) => w.windowId.endsWith("#w01")).length, 3);
  assert.equal(windows.filter((w) => w.peakOverlap).length, 3);
  for (const win of windows) {
    assert.equal(win.bracketCount, 20);
    assert.equal(win.peakOverlap, win.windowId.endsWith("#w01"), win.windowId);
  }

  // nyLabel formatting on the peak window (17:09:59.6 ET start floors to 17:09).
  const peakWin = windows.find((w) => w.peakOverlap);
  assert.equal(peakWin.nyLabel, "2026-01-06 17:09–17:19 ET");
  const a2 = json.goGate.find((g) => g.id === "A4-2");
  assert.equal(
    a2.evidence,
    "windows: 6 total; qualifying peak (>=5 informative in-peak brackets): 3; qualifying off-peak (>=5 informative brackets): 3 (raw: 3 peak, 3 off-peak)",
  );

  // A4-3: strict win via a non-degenerate 6-group holdout.
  assert.equal(json.comparison.distinguishable, true);
  assert.equal(json.comparison.winner, "m30");
  assert.ok(json.comparison.maxCoverage30Common > json.comparison.maxCoverage60Common);
  assert.equal(json.comparison.holdout.mode, "groups");
  assert.equal(json.comparison.holdout.degenerate, false);
  assert.equal(json.comparison.holdout.groupCount, 6);
  assert.equal(json.comparison.holdout.folds.length, 6);

  // A4-5: the comparison runs on the server clock with server evidence in
  // every qualifying group.
  const a5 = json.goGate.find((g) => g.id === "A4-5");
  assert.match(a5.evidence, /qualifying groups with server evidence 6\/6/);
  assert.equal(json.clock.serverEvidence.sufficient, true);
  assert.equal(json.clock.serverEvidence.reason, null);

  // A4-4 / A4-6 via their gate rows are asserted satisfied above; spot-check.
  assert.equal(json.safeOffset.identifiable, true);
  const a6 = json.goGate.find((g) => g.id === "A4-6");
  assert.match(a6.evidence, /stable: target-LOO 3\/3, group-LOO 6\/6/);

  // A4-6 triple stability, each reported separately.
  const st = json.comparison.stability;
  assert.equal(st.targets.mode, "target-loo");
  assert.equal(st.targets.degenerate, false);
  assert.equal(st.targets.count, 3);
  assert.equal(st.targets.folds.length, 3);
  assert.equal(st.targets.pass, true);
  for (const fold of st.targets.folds) {
    assert.equal(fold.distinguishable, true);
    assert.equal(fold.winner, "m30");
  }
  assert.equal(st.groups.mode, "group-loo");
  assert.equal(st.groups.count, 6);
  assert.equal(st.groups.pass, true);
  assert.equal(st.outliers.mode, "residual-topk");
  // Every bracket of this fixture is explained by the winning fit: outlier
  // removal is honestly vacuous and says so.
  assert.equal(st.outliers.residualCount, 0);
  assert.deepEqual(st.outliers.runs, []);
  assert.equal(st.outliers.note, "no-residuals");
  assert.equal(st.outliers.pass, true);
  assert.match(out.md, /no residual brackets under the winning fit/);

  // The MD headline renders the GO verdict without a qualifier.
  assert.match(out.md, /^# S3 Rebuild Cadence Evidence — Verdict: GO\n/);
  assert.match(out.md, /All A4 gates are satisfied by the current evidence\./);
});
