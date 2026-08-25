import test from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import { makeTmpDir, cleanup, makeRunDir, makeTickSeries, runAnalyzer } from "./fixtures.mjs";

const BASE_A = Date.UTC(2026, 0, 6, 3, 0, 0);
const BASE_B = Date.UTC(2026, 0, 6, 9, 0, 0);

function runTwoGroups(dir, periodMs, opts = {}) {
  makeRunDir(join(dir, "runA"), {
    samples: makeTickSeries({ baseMs: BASE_A, periodMs, count: 20, ...opts }),
  });
  makeRunDir(join(dir, "runB"), {
    samples: makeTickSeries({ baseMs: BASE_B, periodMs, count: 20, ...opts }),
  });
  return runAnalyzer([
    "--ndjson", join(dir, "runA"),
    "--ndjson", join(dir, "runB"),
    "--out-json", join(dir, "out.json"),
    "--out-md", join(dir, "out.md"),
  ]);
}

test("distinguishable fixture: true 30 s process wins with consistent holdout", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  const out = runTwoGroups(dir, 30000);
  assert.equal(out.code, 0, out.stderr);
  const cmp = out.json.comparison;
  assert.equal(cmp.clockSource, "server");
  assert.ok(cmp.maxCoverage30Common > cmp.maxCoverage60Common, `c30=${cmp.maxCoverage30Common} c60=${cmp.maxCoverage60Common}`);
  assert.equal(cmp.holdout.mode, "groups");
  assert.equal(cmp.holdout.degenerate, false);
  assert.equal(cmp.holdout.groupCount, 2);
  assert.equal(cmp.holdout.consistentM30Win, true);
  assert.equal(cmp.distinguishable, true);
  assert.equal(cmp.winner, "m30");
  for (const fold of cmp.holdout.folds) {
    assert.ok(fold.test30 > fold.test60, JSON.stringify(fold));
  }
});

test("tie fixture: true 60 s process yields equal coverage and no winner", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  const out = runTwoGroups(dir, 60000);
  assert.equal(out.code, 0, out.stderr);
  const cmp = out.json.comparison;
  assert.equal(cmp.maxCoverage30Common, cmp.maxCoverage60Common);
  assert.equal(cmp.distinguishable, false);
  assert.equal(cmp.winner, null);
  assert.equal(cmp.provisionalWinner, null);
  assert.equal(cmp.reason, "equal-coverage-30s-adds-no-explanatory-power");
});

test("histogram of detection timestamps misleads; arc coverage does not", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // True 60 s process at phase 0; every detection happens 17.6 s after the tick.
  const rows = makeTickSeries({ baseMs: BASE_A, periodMs: 60000, count: 20, preMs: 400, postMs: 17600 });
  makeRunDir(join(dir, "runA"), { samples: rows });
  const out = runAnalyzer([
    "--ndjson", join(dir, "runA"),
    "--out-json", join(dir, "out.json"),
    "--out-md", join(dir, "out.md"),
  ]);
  assert.equal(out.code, 0, out.stderr);

  // Naive (invalid) method: histogram of detection timestamps mod 60 s.
  const hist = new Array(60).fill(0);
  for (let i = 1; i < rows.length; i += 1) {
    if (rows[i].decodedBodySha256 !== rows[i - 1].decodedBodySha256) {
      const detectionMs = Date.parse(rows[i].requestStartedUtc);
      hist[Math.floor((detectionMs % 60000) / 1000)] += 1;
    }
  }
  const argmaxBucket = hist.indexOf(Math.max(...hist));
  assert.notEqual(argmaxBucket, 0, "naive histogram argmax must NOT contain the true phase 0");
  assert.equal(argmaxBucket, 17);

  // The analyzer's interval-censored arc model does contain the true phase 0.
  const m60 = out.json.models.m60.server;
  assert.ok(
    m60.bestPhaseIntervals.some((iv) => iv.startMs <= 0 && iv.endMs >= 0),
    `phase 0 must lie in ${JSON.stringify(m60.bestPhaseIntervals)}`,
  );
});

test("insufficient brackets → not distinguishable regardless of coverage", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  makeRunDir(join(dir, "runA"), {
    samples: makeTickSeries({ baseMs: BASE_A, periodMs: 30000, count: 4 }),
  });
  const out = runAnalyzer([
    "--ndjson", join(dir, "runA"),
    "--out-json", join(dir, "out.json"),
    "--out-md", join(dir, "out.md"),
  ]);
  assert.equal(out.code, 0, out.stderr);
  assert.equal(out.json.comparison.commonInformativeCount, 4);
  assert.equal(out.json.comparison.distinguishable, false);
  assert.equal(out.json.comparison.reason, "insufficient-informative-brackets");
});
