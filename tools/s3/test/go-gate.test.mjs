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

function makeCampusRun(dir, campus) {
  // One input stream with two sessions ~19 h apart: gap-based segmentation
  // must split it into #w00 (off-peak) and #w01 (peak). Both sessions are a
  // true 30 s process (20 ticks each → 20 brackets per window).
  const offPeak = makeTickSeries({ baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1 });
  const peak = makeTickSeries({ baseMs: PEAK_BASE, periodMs: 30000, count: 20, startSeq: 100 });
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

  // A4-1: all three campuses evaluable.
  const a1 = json.goGate.find((g) => g.id === "A4-1");
  for (const campus of ["NB", "NK", "CM"]) {
    assert.match(a1.evidence, new RegExp(`soc:2026:9:${campus}`));
  }

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
  assert.equal(a2.evidence, "windows: 6 total, 3 peak-overlapping, 3 off-peak");

  // A4-3: strict win via a non-degenerate 6-group holdout.
  assert.equal(json.comparison.distinguishable, true);
  assert.equal(json.comparison.winner, "m30");
  assert.ok(json.comparison.maxCoverage30Common > json.comparison.maxCoverage60Common);
  assert.equal(json.comparison.holdout.mode, "groups");
  assert.equal(json.comparison.holdout.degenerate, false);
  assert.equal(json.comparison.holdout.groupCount, 6);
  assert.equal(json.comparison.holdout.folds.length, 6);

  // A4-4 / A4-6 via their gate rows are asserted satisfied above; spot-check.
  assert.equal(json.safeOffset.identifiable, true);
  const a6 = json.goGate.find((g) => g.id === "A4-6");
  assert.match(a6.evidence, /unchanged under leave-out of each of 6 groups/);

  // The MD headline renders the GO verdict without a qualifier.
  assert.match(out.md, /^# S3 Rebuild Cadence Evidence — Verdict: GO\n/);
  assert.match(out.md, /All A4 gates are satisfied by the current evidence\./);
});
