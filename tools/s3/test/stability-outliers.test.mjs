// Non-vacuous coverage for the A4-6 outlier-sensitivity check. Every other
// fixture that reaches assessStability has residualCount 0 (the honestly
// vacuous path); the tests here force actual top-k removal runs and pin:
//   - the deterministic ranking (circular distance desc, bracketId asc tie),
//   - the k > residualCount skip,
//   - the winner-kept AND winner-lost outcomes,
//   - the JSON stability.outliers.runs shape and the MD rendering of
//     non-empty runs (both the pass and the FAIL line).

import test from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import {
  makeTmpDir,
  cleanup,
  makeRunDir,
  makeTickSeries,
  sampleRow,
  runAnalyzer,
  fakeBracket,
} from "./fixtures.mjs";
import { compareModels, assessStability } from "../lib/gate.mjs";

// Jan 6 2026 is EST (UTC-5): the NY 17:00-18:00 peak is 22:00-23:00 UTC.
const OFF_PEAK_BASE = Date.UTC(2026, 0, 6, 3, 0, 0); // 22:00 ET Jan 5 — off-peak
const PEAK_BASE = Date.UTC(2026, 0, 6, 22, 10, 0); // 17:10 ET Jan 6 — inside peak

const CAMPUS_SHAPE = {
  NB: { phaseMs: 0, preMs: 400, postMs: 8600 },
  NK: { phaseMs: 300, preMs: 500, postMs: 8400 },
  CM: { phaseMs: 600, preMs: 800, postMs: 8200 },
};

test("end-to-end: one residual bracket → a real k=1 removal run that keeps the winner (and GO)", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // The go-gate GO fixture plus ONE extra change pair in NB's off-peak
  // window whose arc ([12001, 13000] on the 30 s circle, server clock) does
  // not contain the best phase: exactly one residual bracket under the
  // winning fit, while every gate still holds (its positive jitter stays
  // below period/2). k=1 must remove precisely that bracket and keep the
  // winner; k=2 exceeds residualCount and must be skipped.
  for (const campus of ["NB", "NK", "CM"]) {
    const shape = CAMPUS_SHAPE[campus];
    const offPeak = makeTickSeries({
      baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1,
      bodyPrefix: `${campus}-v`, ...shape,
    });
    const peak = makeTickSeries({
      baseMs: PEAK_BASE, periodMs: 30000, count: 20, startSeq: 100,
      bodyPrefix: `${campus}-v`, ...shape,
    });
    const samples = [...offPeak, ...peak];
    if (campus === "NB") {
      // Stable + changed pair ~35 s after the last off-peak sample (same
      // window: gap < 10 min), landing at 30 s-phase ≈ 12-13 s.
      const stableAt = OFF_PEAK_BASE + 612000;
      const changedAt = OFF_PEAK_BASE + 612800;
      samples.splice(
        40,
        0,
        sampleRow({ seq: 50, startMs: stableAt, bodySha: "NB-v20", serverDateMs: stableAt }),
        sampleRow({
          seq: 51,
          startMs: changedAt,
          bodySha: "NB-v21",
          serverDateMs: Math.floor(changedAt / 1000) * 1000,
        }),
      );
    }
    makeRunDir(join(dir, `run${campus}`), { campus, samples });
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
  for (const gate of json.goGate) {
    assert.equal(gate.satisfied, true, `${gate.id} must be satisfied: ${gate.evidence}`);
  }
  const st = json.comparison.stability;
  const residualId = "soc:2026:9:NB/runNB/samples.ndjson#w00/#51";
  assert.equal(st.outliers.mode, "residual-topk");
  assert.equal(st.outliers.residualCount, 1);
  assert.equal(st.outliers.note, null);
  // Exactly one run: k=1 removes the residual; k=2 > residualCount is skipped.
  assert.equal(st.outliers.runs.length, 1);
  assert.deepEqual(st.outliers.runs[0], {
    k: 1,
    removedBracketIds: [residualId],
    distinguishable: true,
    winner: "m30",
    reason: "m30-strict-win-confirmed-by-holdout",
  });
  assert.equal(st.outliers.pass, true);
  // The winning fit itself agrees on which bracket was left unexplained.
  const clockKey = json.comparison.clockSource;
  assert.deepEqual(json.models.m30[clockKey].residualBracketIds, [residualId]);
  const a6 = json.goGate.find((g) => g.id === "A4-6");
  assert.match(a6.evidence, /outlier top-k \(k∈\{1,2\}, 1 residuals\) winner unchanged/);
  // MD renders the non-empty run, not the vacuous sentence.
  assert.match(
    out.md,
    /- \*\*Outlier sensitivity\*\* \(residual top-k, 1 residuals\): pass — k=1 removed \[soc:2026:9:NB\/runNB\/samples\.ndjson#w00\/#51\] → m30/,
  );
  assert.doesNotMatch(out.md, /no residual brackets under the winning fit/);
});

test("end-to-end: removing the top residual can lose the winner → outliers FAIL is reported", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // A 10-bracket comparison set (exactly MIN_COMPARISON_BRACKETS) with one
  // residual: two 5-bracket windows, nine ticks on the 30 s grid and one tick
  // displaced by +10 s. The full comparison is distinguishable (c30=9 > c60,
  // holdout consistent across the two groups), but removing the top-1
  // residual drops the set below the comparison floor: the winner is lost
  // and the outlier check must FAIL — in JSON and in the MD rendering.
  const g1 = [0, 90000, 150000, 210000, 270000].map((d) => OFF_PEAK_BASE + d);
  const g2 = [0, 90000, 150000, 210000, 280000].map((d) => PEAK_BASE + d);
  makeRunDir(join(dir, "runNB"), {
    campus: "NB",
    samples: [
      ...makeTickSeries({ ticks: g1, startSeq: 1, bodyPrefix: "NB-v" }),
      ...makeTickSeries({ ticks: g2, startSeq: 100, bodyPrefix: "NB-w" }),
    ],
  });
  const out = runAnalyzer([
    "--ndjson", join(dir, "runNB"),
    "--out-json", join(dir, "out.json"),
    "--out-md", join(dir, "out.md"),
  ]);
  assert.equal(out.code, 0, out.stderr);

  const json = out.json;
  assert.equal(json.comparison.distinguishable, true);
  assert.equal(json.comparison.winner, "m30");
  assert.equal(json.comparison.commonInformativeCount, 10);
  const st = json.comparison.stability;
  const residualId = "soc:2026:9:NB/runNB/samples.ndjson#w01/#109";
  assert.equal(st.outliers.residualCount, 1);
  assert.equal(st.outliers.runs.length, 1);
  assert.deepEqual(st.outliers.runs[0], {
    k: 1,
    removedBracketIds: [residualId],
    distinguishable: false,
    winner: null,
    reason: "insufficient-informative-brackets",
  });
  assert.equal(st.outliers.pass, false);
  // A4-6 is unsatisfied (target-LOO is degenerate here too — single target —
  // but the outlier verdict must stand on its own in JSON and MD).
  assert.equal(json.goGate.find((g) => g.id === "A4-6").satisfied, false);
  assert.match(
    out.md,
    /- \*\*Outlier sensitivity\*\* \(residual top-k, 1 residuals\): FAIL — k=1 removed \[soc:2026:9:NB\/runNB\/samples\.ndjson#w01\/#109\] → insufficient-informative-brackets/,
  );
});

test("unit: residual ranking is circular-distance desc with bracketId asc tiebreak", () => {
  // Twelve covering brackets (arc [401, 600], phases split 6/6 across the two
  // 60 s parities) in two groups, plus three residuals in group w0:
  //   r-c: arc [10001, 10200] → distance 9600
  //   r-b: arc [15001, 15200] → distance 14600
  //   r-a: arc [15601, 15801] → distance 14600 (tie with r-b)
  // Ranked order must be [r-a, r-b, r-c]: distance desc, then bracketId asc.
  const brackets = [];
  for (let k = 0; k < 12; k += 1) {
    brackets.push(
      fakeBracket({
        id: `c${String(k).padStart(2, "0")}`,
        lowerMs: k * 30000 + 400,
        upperMs: k * 30000 + 600,
        windowId: k < 6 ? "fx#w0" : "fx#w1",
      }),
    );
  }
  const residual = (id, lowerMs, upperMs) =>
    fakeBracket({ id, lowerMs, upperMs, windowId: "fx#w0" });
  brackets.push(residual("r-c", 600000 + 10000, 600000 + 10200));
  brackets.push(residual("r-b", 600000 + 15000, 600000 + 15200));
  brackets.push(residual("r-a", 600000 + 15600, 600000 + 15801));

  const comparison = compareModels(brackets, "client");
  assert.equal(comparison.distinguishable, true);
  assert.equal(comparison.winner, "m30");
  assert.equal(comparison.maxCoverage30Common, 12);
  assert.equal(comparison.maxCoverage60Common, 6);

  const st = assessStability({ brackets, comparison, clockSource: "client" });
  assert.equal(st.outliers.residualCount, 3);
  assert.equal(st.outliers.runs.length, 2);
  // k=1 removes the top-ranked residual; k=2 the top two — in ranked order.
  assert.deepEqual(st.outliers.runs[0].removedBracketIds, ["r-a"]);
  assert.deepEqual(st.outliers.runs[1].removedBracketIds, ["r-a", "r-b"]);
  for (const run of st.outliers.runs) {
    assert.equal(run.distinguishable, true);
    assert.equal(run.winner, "m30");
  }
  assert.equal(st.outliers.pass, true);
  assert.equal(st.outliers.note, null);
});
