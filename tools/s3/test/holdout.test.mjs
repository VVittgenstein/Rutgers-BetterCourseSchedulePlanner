import test from "node:test";
import assert from "node:assert/strict";
import { runHoldout } from "../lib/holdout.mjs";
import { compareModels } from "../lib/gate.mjs";
import { fakeBracket } from "./fixtures.mjs";

const MIN = 60000;

// Brackets tightly straddling ticks of a true 30 s grid (phase 0), mixed
// minute parity so 60 s models only ever explain about half.
function thirtySecondGroup({ windowId, baseMs, count }) {
  const brackets = [];
  for (let k = 0; k < count; k += 1) {
    const tick = baseMs + k * 30000;
    brackets.push(
      fakeBracket({ id: `${windowId}#${k}`, lowerMs: tick - 800, upperMs: tick + 700, windowId }),
    );
  }
  return brackets;
}

test("two groups → grouped leave-one-out with per-fold numbers", () => {
  const groupA = thirtySecondGroup({ windowId: "a#w00", baseMs: 100 * MIN, count: 8 });
  const groupB = thirtySecondGroup({ windowId: "b#w00", baseMs: 500 * MIN, count: 6 });
  const holdout = runHoldout([...groupA, ...groupB], "server");
  assert.equal(holdout.mode, "groups");
  assert.equal(holdout.degenerate, false);
  assert.equal(holdout.groupCount, 2);
  assert.equal(holdout.folds.length, 2);
  const foldA = holdout.folds.find((f) => f.groupId.endsWith("a#w00"));
  assert.equal(foldA.trainCount, 6);
  assert.equal(foldA.testCount, 8);
  assert.equal(foldA.test30, 8); // trained phase 0 explains every test bracket
  assert.ok(foldA.test60 <= 4, JSON.stringify(foldA));
  assert.equal(holdout.consistentM30Win, true);
});

test("single group → degenerate half-split, provisional winner NOT promoted", () => {
  const group = thirtySecondGroup({ windowId: "a#w00", baseMs: 100 * MIN, count: 12 });
  const holdout = runHoldout(group, "server");
  assert.equal(holdout.mode, "degenerate");
  assert.equal(holdout.degenerate, true);
  assert.equal(holdout.groupCount, 1);
  assert.equal(holdout.folds.length, 1);

  const cmp = compareModels(group, "server");
  assert.equal(cmp.provisionalWinner, "m30"); // c30 strictly beats c60 here
  assert.equal(cmp.distinguishable, false);
  assert.equal(cmp.winner, null);
  assert.equal(cmp.reason, "holdout-degenerate");
});

test("unstable fixture: winner flips across folds → not distinguishable", () => {
  // Group A: 3 even-minute + 3 odd-half-minute brackets (phase 0 mod 30).
  // Group B: 5 brackets at :45 (phase 15 000 mod 30 000).
  const groupA = [];
  for (let k = 0; k < 6; k += 1) {
    const tick = 100 * MIN + k * 30000;
    groupA.push(fakeBracket({ id: `a#${k}`, lowerMs: tick - 500, upperMs: tick + 500, windowId: "a#w00" }));
  }
  const groupB = [];
  for (let k = 0; k < 5; k += 1) {
    const tick = 500 * MIN + k * MIN + 45000;
    groupB.push(fakeBracket({ id: `b#${k}`, lowerMs: tick - 500, upperMs: tick + 500, windowId: "b#w00" }));
  }
  const all = [...groupA, ...groupB];
  const cmp = compareModels(all, "server");
  assert.ok(cmp.maxCoverage30Common > cmp.maxCoverage60Common, `c30=${cmp.maxCoverage30Common} c60=${cmp.maxCoverage60Common}`);
  assert.equal(cmp.provisionalWinner, "m30");
  assert.equal(cmp.holdout.mode, "groups");
  assert.equal(cmp.holdout.consistentM30Win, false);
  assert.equal(cmp.distinguishable, false);
  assert.equal(cmp.reason, "not-confirmed-by-holdout");
});
