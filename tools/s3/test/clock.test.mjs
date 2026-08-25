import test from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import { analyzeClock } from "../lib/clock.mjs";
import { makeTmpDir, cleanup, makeRunDir, makeTickSeries, runAnalyzer } from "./fixtures.mjs";

const BASE = Date.UTC(2026, 0, 6, 3, 0, 0);

test("offset distribution matches hand-computed values", () => {
  const mk = (startMs, endMs, serverDateMs) => ({ clientStartMs: startMs, clientEndMs: endMs, serverDateMs });
  const stream = [
    mk(1000, 1200, 1000),   // offset = 1500 - 1100 = 400
    mk(20000, 20400, 19000), // offset = 19500 - 20200 = -700
    mk(40000, 40200, 40000), // offset = 40500 - 40100 = 400
    mk(60000, 60300, 59000), // offset = 59500 - 60150 = -650
    mk(80000, 80100, null),  // missing serverDate
  ];
  const clock = analyzeClock([stream]);
  assert.equal(clock.status, "server-date-available");
  assert.equal(clock.serverDateMissingCount, 1);
  assert.deepEqual(clock.offsetDistribution, {
    sampleCount: 4,
    minMs: -700,
    p50Ms: -650, // nearest-rank p50 of [-700, -650, 400, 400]
    p95Ms: 400,
    maxMs: 400,
  });
  assert.equal(clock.serverDateRegressions, 0);
});

test("serverDate regressions > 1 s are counted as caveat, not fatal", () => {
  const mk = (startMs, serverDateMs) => ({ clientStartMs: startMs, clientEndMs: startMs + 100, serverDateMs });
  const clock = analyzeClock([[mk(1000, 1000), mk(15000, 12000), mk(30000, 11000), mk(45000, 45000)]]);
  // 15000→12000 is a 3 s regression? prev 1000→12000 rises; 12000→11000 falls 1 s (not > 1 s); no...
  // regressions: (1000→12000 no), (12000→11000 fall 1000 ms, not > 1000), (11000→45000 no) → 0
  assert.equal(clock.serverDateRegressions, 0);
  const clock2 = analyzeClock([[mk(1000, 20000), mk(15000, 12000)]]);
  assert.equal(clock2.serverDateRegressions, 1); // 20000→12000 falls 8 s
});

test("serverDate absent everywhere → unknown clock, null server models, clock-unknown safe offset", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  makeRunDir(join(dir, "runA"), {
    samples: makeTickSeries({ baseMs: BASE, periodMs: 30000, count: 12, noServerDate: true }),
  });
  const out = runAnalyzer([
    "--ndjson", join(dir, "runA"),
    "--out-json", join(dir, "out.json"),
    "--out-md", join(dir, "out.md"),
  ]);
  assert.equal(out.code, 0, out.stderr);
  assert.equal(out.json.clock.status, "unknown");
  assert.equal(out.json.clock.offsetDistribution, null);
  assert.equal(out.json.models.m30.server, null);
  assert.equal(out.json.models.m60.server, null);
  assert.equal(out.json.comparison.clockSource, "client");
  assert.equal(out.json.comparison.clockFallback, true);
  assert.equal(out.json.safeOffset.identifiable, false);
  assert.equal(out.json.safeOffset.reason, "clock-unknown");
  const a5 = out.json.goGate.find((g) => g.id === "A4-5");
  assert.equal(a5.satisfied, false);
  assert.equal(a5.evidence, "serverDate absent; client-clock-only");
});
