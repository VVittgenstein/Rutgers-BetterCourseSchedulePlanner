import test from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import { buildBrackets } from "../lib/brackets.mjs";
import { ingestNdjson } from "../lib/ingest-ndjson.mjs";
import { segmentWindows } from "../lib/windows.mjs";
import { fitPhase } from "../lib/phase.mjs";
import { makeTmpDir, cleanup, makeRunDir, sampleRow } from "./fixtures.mjs";

const BASE = Date.UTC(2026, 0, 6, 3, 0, 0);

function windowFromNdjson(dir) {
  const { samples, intervalSeconds, input } = ingestNdjson(dir);
  const windows = segmentWindows(samples, intervalSeconds, input.id);
  assert.equal(windows.length, 1);
  return windows[0];
}

test("ndjson bracket bounds: client envelope and +1000 ms server widening", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  const stableStart = BASE + 100; // serverDate floors to BASE
  const changedStart = BASE + 13100; // serverDate floors to BASE + 13000
  makeRunDir(join(dir, "runA"), {
    samples: [
      sampleRow({ seq: 1, startMs: stableStart, elapsedMs: 250, bodySha: "a", serverDateMs: BASE }),
      sampleRow({ seq: 2, startMs: changedStart, elapsedMs: 250, bodySha: "b", serverDateMs: BASE + 13000 }),
    ],
  });
  const { brackets, counters } = buildBrackets(windowFromNdjson(join(dir, "runA")), "ndjson");
  assert.equal(brackets.length, 1);
  assert.equal(counters.changeCount, 1);
  const b = brackets[0];
  assert.equal(b.clientLowerMs, stableStart);
  assert.equal(b.clientUpperMs, changedStart + 250);
  assert.equal(b.serverLowerMs, BASE);
  assert.equal(b.serverUpperMs, BASE + 13000 + 1000, "server upper must be widened by +1000 ms");
  assert.equal(b.serverWidthMs, 14000);
  assert.equal(b.clientWidthMs, changedStart + 250 - stableStart);
});

test("wide bracket lands in nonInformative counts, not coverage", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  makeRunDir(join(dir, "runA"), {
    samples: [
      sampleRow({ seq: 1, startMs: BASE, bodySha: "a", serverDateMs: BASE }),
      // 45 s later: width >= 30 s (non-informative for m30), < 60 s (informative for m60)
      sampleRow({ seq: 2, startMs: BASE + 45000, bodySha: "b", serverDateMs: BASE + 45000 }),
    ],
  });
  const { brackets } = buildBrackets(windowFromNdjson(join(dir, "runA")), "ndjson");
  assert.equal(brackets.length, 1);
  const fit30 = fitPhase(brackets, 30000, "server");
  assert.equal(fit30.informativeCount, 0);
  assert.equal(fit30.nonInformativeCount, 1);
  assert.equal(fit30.maxCoverage, 0);
  const fit60 = fitPhase(brackets, 60000, "server");
  assert.equal(fit60.informativeCount, 1);
  assert.equal(fit60.maxCoverage, 1);
});

test("bracket spanning an excluded row widens across the gap", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  makeRunDir(join(dir, "runA"), {
    samples: [
      sampleRow({ seq: 1, startMs: BASE, bodySha: "a", serverDateMs: BASE }),
      sampleRow({ seq: 2, startMs: BASE + 13000, bodySha: "b", serverDateMs: BASE + 13000, httpStatus: 500 }),
      sampleRow({ seq: 3, startMs: BASE + 26000, bodySha: "b", serverDateMs: BASE + 26000 }),
    ],
  });
  const { brackets } = buildBrackets(windowFromNdjson(join(dir, "runA")), "ndjson");
  assert.equal(brackets.length, 1);
  // adjacency is over included rows: (seq 1, seq 3]
  assert.equal(brackets[0].clientLowerMs, BASE);
  assert.equal(brackets[0].clientUpperMs, BASE + 26000 + 200);
  assert.equal(brackets[0].serverWidthMs, 27000);
});

test("serverDate skew producing non-positive width -> client-only bracket + counter", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  makeRunDir(join(dir, "runA"), {
    samples: [
      // stable backend clock runs 3 s ahead: serverLower > serverUpper
      sampleRow({ seq: 1, startMs: BASE, bodySha: "a", serverDateMs: BASE + 3000 }),
      sampleRow({ seq: 2, startMs: BASE + 1000, bodySha: "b", serverDateMs: BASE + 1000 }),
    ],
  });
  const { brackets, counters } = buildBrackets(windowFromNdjson(join(dir, "runA")), "ndjson");
  assert.equal(brackets.length, 1);
  assert.equal(counters.serverNonPositiveWidth, 1);
  assert.equal(brackets[0].serverLowerMs, null);
  assert.equal(brackets[0].serverUpperMs, null);
  assert.equal(brackets[0].serverWidthMs, null);
  assert.equal(brackets[0].clientLowerMs, BASE);
  assert.equal(brackets[0].clientUpperMs, BASE + 1200);
  const fit = fitPhase(brackets, 30000, "server");
  assert.equal(fit.unusableCount, 1);
  assert.equal(fit.informativeCount, 0);
});

test("tolerated client-clock step producing negative width -> bracket rejected and counted", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  makeRunDir(join(dir, "runA"), {
    samples: [
      sampleRow({ seq: 1, startMs: BASE, elapsedMs: 100, bodySha: "a", serverDateMs: BASE }),
      // client clock steps back 1.9 s (inside the 2 s ingestion tolerance);
      // with 100 ms elapsed the client envelope is (BASE, BASE - 1800]: width -1800.
      sampleRow({ seq: 2, startMs: BASE - 1900, elapsedMs: 100, bodySha: "b", serverDateMs: BASE + 13000 }),
    ],
  });
  const { brackets, counters } = buildBrackets(windowFromNdjson(join(dir, "runA")), "ndjson");
  // The whole bracket is rejected — the seemingly valid server bounds derive
  // from the same corrupt pair and must not survive on their own.
  assert.equal(brackets.length, 0);
  assert.equal(counters.clientNonPositiveWidth, 1);
  assert.equal(counters.changeCount, 0);
});

test("corrupted requestEndedUtc < requestStartedUtc -> bracket rejected and counted", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  makeRunDir(join(dir, "runA"), {
    samples: [
      sampleRow({ seq: 1, startMs: BASE, elapsedMs: 100, bodySha: "a", serverDateMs: BASE }),
      // requestEndedUtc 50 s BEFORE requestStartedUtc of the stable row
      sampleRow({ seq: 2, startMs: BASE + 100, elapsedMs: -50100, bodySha: "b", serverDateMs: BASE + 13000 }),
    ],
  });
  const { brackets, counters } = buildBrackets(windowFromNdjson(join(dir, "runA")), "ndjson");
  assert.equal(brackets.length, 0);
  assert.equal(counters.clientNonPositiveWidth, 1);
});

test("sqlite path: change rows bracket against nearest preceding row", () => {
  const mkSample = (seq, tMs, body, changed) => ({
    inputId: "obs.sqlite",
    targetId: "db:t1",
    term: null,
    year: null,
    campus: null,
    seq,
    clientStartMs: tMs,
    clientEndMs: tMs,
    serverDateMs: Math.floor(tMs / 1000) * 1000,
    bodySha: body,
    bodyChangedFlag: changed,
    ageSeconds: null,
  });
  const window = {
    windowId: "obs.sqlite#w00",
    utcStartMs: BASE,
    utcEndMs: BASE + 40000,
    samples: [
      mkSample(1, BASE + 100, "a", 0),
      mkSample(2, BASE + 13100, "a", 0),
      mkSample(3, BASE + 26100, "b", 1),
    ],
  };
  const { brackets, counters } = buildBrackets(window, "sqlite");
  assert.equal(brackets.length, 1);
  assert.equal(counters.noPriorStable, 0);
  const b = brackets[0];
  assert.equal(b.clientLowerMs, BASE + 13100);
  assert.equal(b.clientUpperMs, BASE + 26100);
  assert.equal(b.serverLowerMs, BASE + 13000);
  assert.equal(b.serverUpperMs, BASE + 26000 + 1000);

  // change row first in window → noPriorStable
  const window2 = { ...window, samples: [mkSample(1, BASE + 100, "b", 1), mkSample(2, BASE + 13100, "b", 0)] };
  const res2 = buildBrackets(window2, "sqlite");
  assert.equal(res2.brackets.length, 0);
  assert.equal(res2.counters.noPriorStable, 1);
});
