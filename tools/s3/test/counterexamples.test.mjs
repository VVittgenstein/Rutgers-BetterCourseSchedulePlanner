// End-to-end counterexamples for the four adjudicated false-GO root causes
// (STAGE-5-R1). Negative-proof protocol: each fixture below, generated with
// these exact parameters, produced `verdict=GO qualifier=none` on analyzer
// v1.1.0 (commit 95827285205d) — verified by extracting that tree and running
// its CLI on the same bytes:
//   CE-1 (A4-1 relabeled copy):        old stdout `verdict=GO qualifier=none brackets=120 distinguishable=true`
//   CE-2 (A4-2 empty peak window):     old stdout `verdict=GO qualifier=none brackets=60 distinguishable=true`
//   CE-3 (A4-5 stray serverDate):      old stdout `verdict=GO qualifier=none brackets=120 distinguishable=true`
//   CE-4 (A4-6 single-target winner):  old stdout `verdict=GO qualifier=none brackets=30 distinguishable=true`
// This file asserts the CURRENT analyzer answers NO_PRODUCTION_CHANGE with the
// specific gate unsatisfied for the specific reason, while the go-gate test
// keeps proving that a genuinely satisfying fixture still reaches GO.

import test from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import { readFileSync } from "node:fs";
import { makeTmpDir, cleanup, makeRunDir, makeTickSeries, sampleRow, runAnalyzer } from "./fixtures.mjs";

// Jan 6 2026 is EST (UTC-5): the NY 17:00-18:00 peak is 22:00-23:00 UTC.
const OFF_PEAK_BASE = Date.UTC(2026, 0, 6, 3, 0, 0); // 22:00 ET Jan 5 — off-peak
const PEAK_BASE = Date.UTC(2026, 0, 6, 22, 10, 0); // 17:10 ET Jan 6 — inside peak

// Per-campus geometry used when a fixture needs genuinely independent series
// (mirrors go-gate.test.mjs): distinct bodySha namespace plus distinct
// pre/post/phase, with phases inside the arc overlap so A4-4 still holds.
const CAMPUS_SHAPE = {
  NB: { phaseMs: 0, preMs: 400, postMs: 8600 },
  NK: { phaseMs: 300, preMs: 500, postMs: 8400 },
  CM: { phaseMs: 600, preMs: 800, postMs: 8200 },
};

function gateById(json, id) {
  return json.goGate.find((g) => g.id === id);
}

function analyze(dir, runNames) {
  return runAnalyzer([
    ...runNames.flatMap((name) => ["--ndjson", join(dir, name)]),
    "--out-json", join(dir, "out.json"),
    "--out-md", join(dir, "out.md"),
  ]);
}

test("CE-1 (A4-1): one capture relabeled as NB/NK/CM is one provenance class, not three targets", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // ONE observation series (off-peak + peak sessions, a true 30 s process),
  // written byte-identically into three run dirs whose run.json alone claims
  // three campuses. v1.1.0 keyed A4-1 on the campus label and said GO.
  const samples = [
    ...makeTickSeries({ baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1 }),
    ...makeTickSeries({ baseMs: PEAK_BASE, periodMs: 30000, count: 20, startSeq: 100 }),
  ];
  for (const campus of ["NB", "NK", "CM"]) {
    makeRunDir(join(dir, `run${campus}`), { campus, samples });
  }
  // The observation bytes really are identical across the three inputs.
  const ndjsonBytes = ["NB", "NK", "CM"].map((c) =>
    readFileSync(join(dir, `run${c}`, "samples.ndjson"), "utf8"),
  );
  assert.equal(ndjsonBytes[0], ndjsonBytes[1]);
  assert.equal(ndjsonBytes[0], ndjsonBytes[2]);

  const out = analyze(dir, ["runNB", "runNK", "runCM"]);
  assert.equal(out.code, 0, out.stderr);
  assert.match(out.stdout, /verdict=NO_PRODUCTION_CHANGE qualifier=DATA_REQUIRED/);

  const json = out.json;
  assert.equal(json.provenance.classes.length, 1);
  assert.equal(json.provenance.classes[0].campusConflict, true);
  assert.equal(json.provenance.classes[0].campus, null);
  const a1 = gateById(json, "A4-1");
  assert.equal(a1.satisfied, false);
  assert.match(a1.evidence, /conflicting campus labels/);
  assert.match(a1.evidence, /NB missing; NK missing; CM missing/);
  // Root-cause isolation: every other gate is genuinely satisfied.
  assert.deepEqual(
    json.decision.reasons.map((r) => r.split(" ")[0]),
    ["A4-1"],
  );
});

test("CE-2 (A4-2): an isolated zero-change peak-time sample is not peak evidence", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // Three genuinely independent campus series, each with a rich off-peak
  // window (20 brackets) plus a SINGLE peak-time sample: its window has zero
  // brackets. v1.1.0 counted the window's existence and said GO.
  for (const campus of ["NB", "NK", "CM"]) {
    const shape = CAMPUS_SHAPE[campus];
    const offPeak = makeTickSeries({
      baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1,
      bodyPrefix: `${campus}-v`, ...shape,
    });
    const peakLoner = sampleRow({
      seq: 100,
      startMs: PEAK_BASE,
      bodySha: `${campus}-v20`,
      serverDateMs: PEAK_BASE,
    });
    makeRunDir(join(dir, `run${campus}`), { campus, samples: [...offPeak, peakLoner] });
  }
  const out = analyze(dir, ["runNB", "runNK", "runCM"]);
  assert.equal(out.code, 0, out.stderr);
  assert.match(out.stdout, /verdict=NO_PRODUCTION_CHANGE qualifier=DATA_REQUIRED/);

  const json = out.json;
  const a2 = gateById(json, "A4-2");
  assert.equal(a2.satisfied, false);
  assert.equal(
    a2.evidence,
    "windows: 6 total; qualifying peak (>=5 informative in-peak brackets): 0; qualifying off-peak (>=5 informative brackets): 3 (raw: 3 peak, 3 off-peak)",
  );
  assert.deepEqual(
    json.decision.reasons.map((r) => r.split(" ")[0]),
    ["A4-2"],
  );
});

test("CE-2b (A4-2): a window ending exactly at 17:00:00.000 ET has zero peak evidence", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // Boundary graze (second R1 review round): each campus has a rich off-peak
  // session plus a 20-bracket session whose LAST sample sits exactly at the
  // 17:00:00.000 ET boundary instant. The window's closed-interval label says
  // peakOverlap=true (measure-zero touch), but not one bracket overlaps the
  // peak hour with positive measure. Analyzer v2.0.0 (7260da8) counted the
  // window's own informative brackets and said verdict=GO qualifier=none.
  const PRE_PEAK_END = Date.UTC(2026, 0, 6, 22, 0, 0); // == 17:00:00.000 ET
  for (const campus of ["NB", "NK", "CM"]) {
    const shape = CAMPUS_SHAPE[campus];
    const offPeak = makeTickSeries({
      baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1,
      bodyPrefix: `${campus}-v`, ...shape,
    });
    const graze = makeTickSeries({
      baseMs: PRE_PEAK_END - 20 * 30000 - 60000, periodMs: 30000, count: 20,
      startSeq: 300, bodyPrefix: `${campus}-p`, ...shape,
    });
    graze.push(
      sampleRow({ seq: 400, startMs: PRE_PEAK_END, elapsedMs: 0, bodySha: `${campus}-p20`, serverDateMs: PRE_PEAK_END }),
    );
    makeRunDir(join(dir, `run${campus}`), { campus, samples: [...offPeak, ...graze] });
  }
  const out = analyze(dir, ["runNB", "runNK", "runCM"]);
  assert.equal(out.code, 0, out.stderr);
  assert.match(out.stdout, /verdict=NO_PRODUCTION_CHANGE qualifier=DATA_REQUIRED/);

  const json = out.json;
  // The bait: the grazing windows ARE labeled peak-overlapping (closed touch)…
  const windows = json.targets.flatMap((tgt) => tgt.windows);
  assert.equal(windows.filter((w) => w.peakOverlap).length, 3);
  // …but the gate demands brackets inside the hour, and there are none.
  const a2 = gateById(json, "A4-2");
  assert.equal(a2.satisfied, false);
  assert.match(a2.evidence, /qualifying peak \(>=5 informative in-peak brackets\): 0;/);
  assert.deepEqual(
    json.decision.reasons.map((r) => r.split(" ")[0]),
    ["A4-2"],
  );
});

test("CE-2c (A4-2): a peak-time loner merged into a pre-peak session is still not peak evidence", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // Merged loner (second R1 review round): the adjudicated root-cause-#2
  // shape — an isolated zero-change peak-time sample — placed ~5.5 min after
  // a rich pre-peak session so the gap-based segmentation MERGES it into that
  // window (gap < 10 min). The merged window genuinely overlaps the peak and
  // has 20 informative brackets, yet none of them lies inside 17:00-18:00 ET.
  // Analyzer v2.0.0 (7260da8) said verdict=GO qualifier=none on these bytes.
  const PRE_PEAK_BASE = Date.UTC(2026, 0, 6, 21, 50, 0); // last tick 16:59:30 ET
  const LONER_AT = Date.UTC(2026, 0, 6, 22, 5, 0); // 17:05:00 ET
  for (const campus of ["NB", "NK", "CM"]) {
    const shape = CAMPUS_SHAPE[campus];
    const offPeak = makeTickSeries({
      baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1,
      bodyPrefix: `${campus}-v`, ...shape,
    });
    const prePeak = makeTickSeries({
      baseMs: PRE_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 100,
      bodyPrefix: `${campus}-p`, ...shape,
    });
    const loner = sampleRow({ seq: 300, startMs: LONER_AT, bodySha: `${campus}-p20`, serverDateMs: LONER_AT });
    makeRunDir(join(dir, `run${campus}`), { campus, samples: [...offPeak, ...prePeak, loner] });
  }
  const out = analyze(dir, ["runNB", "runNK", "runCM"]);
  assert.equal(out.code, 0, out.stderr);
  assert.match(out.stdout, /verdict=NO_PRODUCTION_CHANGE qualifier=DATA_REQUIRED/);

  const json = out.json;
  // The loner really merged: 2 windows per input (not 3), and the merged one
  // overlaps the peak with 20 brackets.
  const windows = json.targets.flatMap((tgt) => tgt.windows);
  assert.equal(windows.length, 6);
  const mergedPeak = windows.filter((w) => w.peakOverlap);
  assert.equal(mergedPeak.length, 3);
  for (const win of mergedPeak) assert.equal(win.bracketCount, 20);
  const a2 = gateById(json, "A4-2");
  assert.equal(a2.satisfied, false);
  assert.match(a2.evidence, /qualifying peak \(>=5 informative in-peak brackets\): 0;/);
  assert.deepEqual(
    json.decision.reasons.map((r) => r.split(" ")[0]),
    ["A4-2"],
  );
});

test("CE-3 (A4-5): one stray serverDate does not license client-clock production conclusions", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // Three independent campus series with NO serverDate anywhere that matters:
  // every bracket has null server bounds, so the comparison silently falls
  // back to the client clock. One trailing unrelated NB sample carries a
  // serverDate (its own single-sample window, zero brackets); v1.1.0's A4-5
  // looked only at the global clock status and said GO.
  for (const campus of ["NB", "NK", "CM"]) {
    const shape = CAMPUS_SHAPE[campus];
    const samples = [
      ...makeTickSeries({
        baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1,
        bodyPrefix: `${campus}-v`, noServerDate: true, ...shape,
      }),
      ...makeTickSeries({
        baseMs: PEAK_BASE, periodMs: 30000, count: 20, startSeq: 100,
        bodyPrefix: `${campus}-v`, noServerDate: true, ...shape,
      }),
    ];
    if (campus === "NB") {
      samples.push(
        sampleRow({
          seq: 500,
          startMs: PEAK_BASE + 2 * 3600 * 1000, // 19:10 ET — its own off-peak window
          bodySha: "NB-v20",
          serverDateMs: PEAK_BASE + 2 * 3600 * 1000,
        }),
      );
    }
    makeRunDir(join(dir, `run${campus}`), { campus, samples });
  }
  const out = analyze(dir, ["runNB", "runNK", "runCM"]);
  assert.equal(out.code, 0, out.stderr);
  assert.match(out.stdout, /verdict=NO_PRODUCTION_CHANGE qualifier=none/);

  const json = out.json;
  // The bait: global clock status IS server-date-available (1 stray sample).
  assert.equal(json.clock.status, "server-date-available");
  assert.equal(json.clock.offsetDistribution.sampleCount, 1);
  // But no bracket has server bounds and the comparison fell back to client.
  assert.equal(json.models.m30.server.informativeCount, 0);
  assert.equal(json.comparison.clockSource, "client");
  assert.equal(json.comparison.clockFallback, true);
  assert.equal(json.clock.serverEvidence.sufficient, false);
  assert.equal(json.clock.serverEvidence.reason, "client-clock-fallback");

  const a5 = gateById(json, "A4-5");
  assert.equal(a5.satisfied, false);
  assert.equal(
    a5.evidence,
    "client-clock fallback: only 0 server-informative comparison brackets (< 10)",
  );
  assert.equal(json.safeOffset.identifiable, false);
  assert.equal(json.safeOffset.reason, "server-clock-evidence-insufficient:client-clock-fallback");
  const a4 = gateById(json, "A4-4");
  assert.equal(a4.satisfied, false);
  assert.deepEqual(
    json.decision.reasons.map((r) => r.split(" ")[0]),
    ["A4-4", "A4-5"],
  );
});

test("CE-4 (A4-6): a winner carried by one target does not survive whole-target leave-out", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // NB carries ALL the 30 s evidence: two windows whose ticks sit mostly on
  // odd 30 s positions (1 even + 9 odd per window on the 60 s circle). NK and
  // CM are individually 60 s-compatible (5 even ticks each). Group leave-out
  // keeps the winner (the other NB window remains), so v1.1.0 said GO; the
  // whole-NB-target leave-out collapses to equal coverage.
  const nbTicks = (base) => [
    base,
    ...Array.from({ length: 9 }, (_, i) => base + (i + 1) * 60000 + 30000),
  ];
  const evenTicks = (base) => Array.from({ length: 5 }, (_, i) => base + i * 60000);
  makeRunDir(join(dir, "runNB"), {
    campus: "NB",
    samples: [
      ...makeTickSeries({ ticks: nbTicks(OFF_PEAK_BASE), startSeq: 1, bodyPrefix: "NB-v" }),
      ...makeTickSeries({ ticks: nbTicks(PEAK_BASE), startSeq: 100, bodyPrefix: "NB-v" }),
    ],
  });
  makeRunDir(join(dir, "runNK"), {
    campus: "NK",
    samples: makeTickSeries({ ticks: evenTicks(OFF_PEAK_BASE), startSeq: 1, bodyPrefix: "NK-v" }),
  });
  makeRunDir(join(dir, "runCM"), {
    campus: "CM",
    samples: makeTickSeries({ ticks: evenTicks(OFF_PEAK_BASE), startSeq: 1, bodyPrefix: "CM-v" }),
  });

  const out = analyze(dir, ["runNB", "runNK", "runCM"]);
  assert.equal(out.code, 0, out.stderr);
  assert.match(out.stdout, /verdict=NO_PRODUCTION_CHANGE qualifier=none/);

  const json = out.json;
  // The full-data comparison and the group-level checks all look fine…
  assert.equal(json.comparison.distinguishable, true);
  assert.equal(json.comparison.winner, "m30");
  const st = json.comparison.stability;
  assert.equal(st.groups.pass, true);
  assert.equal(st.groups.count, 4);
  assert.equal(st.outliers.pass, true);
  // …but the winner is carried entirely by NB.
  assert.equal(st.targets.degenerate, false);
  assert.equal(st.targets.count, 3);
  assert.equal(st.targets.pass, false);
  const nbFold = st.targets.folds.find((f) => f.heldOut === "soc:2026:9:NB");
  assert.equal(nbFold.distinguishable, false);
  assert.equal(nbFold.reason, "equal-coverage-30s-adds-no-explanatory-power");

  const a6 = gateById(json, "A4-6");
  assert.equal(a6.satisfied, false);
  assert.match(a6.evidence, /target-LOO failed: held-out soc:2026:9:NB/);
  assert.deepEqual(
    json.decision.reasons.map((r) => r.split(" ")[0]),
    ["A4-6"],
  );

  // Cross-check: the analyzer itself, run on NK+CM alone, reports the honest
  // equal-coverage tie the held-out fold predicts.
  const out2 = runAnalyzer([
    "--ndjson", join(dir, "runNK"),
    "--ndjson", join(dir, "runCM"),
    "--out-json", join(dir, "out2.json"),
    "--out-md", join(dir, "out2.md"),
  ]);
  assert.equal(out2.code, 0, out2.stderr);
  assert.equal(out2.json.comparison.reason, "equal-coverage-30s-adds-no-explanatory-power");
  assert.equal(out2.json.comparison.distinguishable, false);
});
