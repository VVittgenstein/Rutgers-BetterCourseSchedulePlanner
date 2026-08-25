import test from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import { assessSafeOffset } from "../lib/gate.mjs";
import { makeTmpDir, cleanup, makeRunDir, makeTickSeries, runAnalyzer, fakeBracket } from "./fixtures.mjs";

const MIN = 60000;
const BASE_A = Date.UTC(2026, 0, 6, 3, 0, 0);
const BASE_B = Date.UTC(2026, 0, 6, 9, 0, 0);

test("identifiable fixture: distinguishable + overlapping phases + bounded jitter", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  makeRunDir(join(dir, "runA"), {
    samples: makeTickSeries({ baseMs: BASE_A, periodMs: 30000, count: 20 }),
  });
  makeRunDir(join(dir, "runB"), {
    samples: makeTickSeries({ baseMs: BASE_B, periodMs: 30000, count: 20 }),
  });
  const out = runAnalyzer([
    "--ndjson", join(dir, "runA"),
    "--ndjson", join(dir, "runB"),
    "--out-json", join(dir, "out.json"),
    "--out-md", join(dir, "out.md"),
  ]);
  assert.equal(out.code, 0, out.stderr);
  assert.equal(out.json.comparison.distinguishable, true);
  assert.equal(out.json.safeOffset.identifiable, true);
  assert.equal(out.json.safeOffset.reason, undefined);
  const bound = out.json.safeOffset.bound;
  // postMs 8600 → changed serverDate floors to +8000, +1 s widening → jitter 9000 ms < 15000
  assert.equal(bound.maxPositiveJitterMs, 9000);
  assert.ok(bound.phaseIntervalMs.startMs <= bound.phaseIntervalMs.endMs);
  const a4 = out.json.goGate.find((g) => g.id === "A4-4");
  assert.equal(a4.satisfied, true);
});

function fakeComparison(comparisonSet) {
  return {
    clockSource: "server",
    distinguishable: true,
    winner: "m30",
    comparisonSet,
    holdout: { degenerate: false },
  };
}

test("disjoint per-group phase intervals → phase-intervals-disjoint-across-groups", () => {
  const groupA = [];
  const groupB = [];
  for (let k = 0; k < 5; k += 1) {
    const tickA = 100 * MIN + k * 30000; // phase 0
    groupA.push(fakeBracket({ id: `a#${k}`, lowerMs: tickA - 400, upperMs: tickA + 400, windowId: "a#w00" }));
    const tickB = 500 * MIN + k * 30000 + 9000; // phase 9 000
    groupB.push(fakeBracket({ id: `b#${k}`, lowerMs: tickB - 400, upperMs: tickB + 400, windowId: "b#w00" }));
  }
  const res = assessSafeOffset(fakeComparison([...groupA, ...groupB]), "server-date-available");
  assert.equal(res.identifiable, false);
  assert.equal(res.reason, "phase-intervals-disjoint-across-groups");
});

test("unbounded positive jitter → jitter-unbounded", () => {
  const groupA = [];
  const groupB = [];
  for (let k = 0; k < 5; k += 1) {
    const tickA = 100 * MIN + k * 30000;
    // wide right tail: upper 16 s past the tick (>= period/2 after consensus phase)
    groupA.push(fakeBracket({ id: `a#${k}`, lowerMs: tickA - 400, upperMs: tickA + 16000, windowId: "a#w00" }));
    const tickB = 500 * MIN + k * 30000;
    groupB.push(fakeBracket({ id: `b#${k}`, lowerMs: tickB - 400, upperMs: tickB + 16000, windowId: "b#w00" }));
  }
  const res = assessSafeOffset(fakeComparison([...groupA, ...groupB]), "server-date-available");
  assert.equal(res.identifiable, false);
  assert.equal(res.reason, "jitter-unbounded");
});

test("not distinguishable / degenerate holdout reasons take precedence", () => {
  const set = [fakeBracket({ id: "x", lowerMs: 0, upperMs: 1000 })];
  assert.deepEqual(assessSafeOffset({ ...fakeComparison(set), distinguishable: false }, "server-date-available"), {
    identifiable: false,
    reason: "not-distinguishable",
  });
  assert.deepEqual(
    assessSafeOffset(
      { ...fakeComparison(set), holdout: { degenerate: true } },
      "server-date-available",
    ),
    { identifiable: false, reason: "holdout-degenerate" },
  );
  assert.deepEqual(assessSafeOffset(fakeComparison(set), "unknown"), {
    identifiable: false,
    reason: "clock-unknown",
  });
});
