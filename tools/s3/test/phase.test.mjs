import test from "node:test";
import assert from "node:assert/strict";
import { fitPhase, sweepArcs, arcContains } from "../lib/phase.mjs";
import { compareModels } from "../lib/gate.mjs";
import { fakeBracket } from "./fixtures.mjs";

const MIN = 60000;

test("true phase lies inside bestPhaseIntervals for a synthetic 30 s process", () => {
  // Change instants at exactly phase 7000 on a 30 s grid; brackets straddle them.
  const brackets = [];
  for (let k = 0; k < 12; k += 1) {
    const tick = 10 * MIN + k * 30000 + 7000;
    const lower = tick - 1000 - (k % 5) * 700;
    const upper = tick + 500 + (k % 7) * 900;
    brackets.push(fakeBracket({ id: `b${k}`, lowerMs: lower, upperMs: upper }));
  }
  const fit = fitPhase(brackets, 30000, "server");
  assert.equal(fit.maxCoverage, 12);
  assert.equal(fit.informativeCount, 12);
  const phase = 7000;
  assert.ok(
    fit.bestPhaseIntervals.some((iv) => arcContains({ startMs: iv.startMs, endMs: iv.endMs }, phase, 30000)),
    `phase ${phase} must lie in ${JSON.stringify(fit.bestPhaseIntervals)}`,
  );
  assert.equal(fit.residualBracketIds.length, fit.informativeCount - fit.maxCoverage);
});

test("wrap-around arc across 0 is handled exactly", () => {
  // (29 500, 30 800] on a 30 s circle → closed [29501, 29999] ∪ [0, 800]
  const brackets = [fakeBracket({ id: "wrap", lowerMs: 29500, upperMs: 30800 })];
  const fit = fitPhase(brackets, 30000, "server");
  assert.equal(fit.maxCoverage, 1);
  assert.deepEqual(fit.bestPhaseIntervals, [
    { startMs: 0, endMs: 800 },
    { startMs: 29501, endMs: 29999 },
  ]);
  // point 29500 itself is NOT covered (half-open lower bound)
  const cover29500 = fit.bestPhaseIntervals.some((iv) => 29500 >= iv.startMs && 29500 <= iv.endMs);
  assert.equal(cover29500, false);
});

test("sweepArcs counts overlap exactly at the seam", () => {
  const arcs = [
    { startMs: 29001, endMs: 500 }, // wrapped
    { startMs: 0, endMs: 1000 },
    { startMs: 200, endMs: 700 },
  ];
  const { maxCoverage, bestPhaseIntervals } = sweepArcs(arcs, 30000);
  assert.equal(maxCoverage, 3);
  assert.deepEqual(bestPhaseIntervals, [{ startMs: 200, endMs: 500 }]);
});

test("residual invariant holds with outliers", () => {
  const brackets = [];
  for (let k = 0; k < 9; k += 1) {
    const tick = 10 * MIN + k * 30000;
    brackets.push(fakeBracket({ id: `b${k}`, lowerMs: tick - 900, upperMs: tick + 900 }));
  }
  // one bracket that misses the common phase entirely
  brackets.push(fakeBracket({ id: "outlier", lowerMs: 10 * MIN + 9 * 30000 + 9000, upperMs: 10 * MIN + 9 * 30000 + 11000 }));
  const fit = fitPhase(brackets, 30000, "server");
  assert.equal(fit.maxCoverage, 9);
  assert.deepEqual(fit.residualBracketIds, ["outlier"]);
  assert.equal(fit.residualBracketIds.length, fit.informativeCount - fit.maxCoverage);
});

test("runtime invariant c30 >= c60 (compareModels does not throw)", () => {
  const brackets = [];
  for (let k = 0; k < 20; k += 1) {
    const tick = 10 * MIN + k * 30000;
    brackets.push(fakeBracket({ id: `b${k}`, lowerMs: tick - 700 - (k % 3) * 400, upperMs: tick + 600 + (k % 4) * 500 }));
  }
  const cmp = compareModels(brackets, "server");
  assert.ok(cmp.maxCoverage30Common >= cmp.maxCoverage60Common);
});
