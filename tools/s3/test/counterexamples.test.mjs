// End-to-end counterexamples for the four adjudicated false-GO root causes
// (STAGE-5-R1). Negative-proof protocol: each fixture below, generated with
// these exact parameters, produced `verdict=GO qualifier=none` on analyzer
// v1.1.0 (commit 95827285205d) — verified by extracting that tree and running
// its CLI on the same bytes:
//   CE-1 (A4-1 relabeled copy):        old stdout `verdict=GO qualifier=none brackets=120 distinguishable=true`
//   CE-2 (A4-2 empty peak window):     old stdout `verdict=GO qualifier=none brackets=60 distinguishable=true`
//   CE-3 (A4-5 stray serverDate):      old stdout `verdict=GO qualifier=none brackets=120 distinguishable=true`
//   CE-4 (A4-6 single-target winner):  old stdout `verdict=GO qualifier=none brackets=30 distinguishable=true`
// The CE-5 pair pins the third-round duplicate-content amplification defect,
// which survived until v2.1.0 (commit 6795fed) — same protocol, verified by
// extracting that tree and running its CLI on the same fixture bytes:
//   CE-5 (A4-6 term-relabel copies):   old stdout `verdict=GO qualifier=none brackets=84 distinguishable=true`
//   CE-5b (A4-6 SQLite duplicate):     old stdout `verdict=GO qualifier=none brackets=59 distinguishable=true`
// The CE-6 pair pins the fourth-round time-translation defect, which survived
// until v2.2.0 (commit 34158c9) — same protocol, verified by extracting that
// tree and running its CLI on the same fixture bytes:
//   CE-6 (A4-2 shifted byte-copy):     old stdout `verdict=GO qualifier=none brackets=120 distinguishable=true`
//   CE-6b (A4-2 longer shifted copy):  old stdout `verdict=GO qualifier=none brackets=121 distinguishable=true`
// The CE-7 pair pins the fifth-round single-column translation defect, which
// survived until v2.3.0 (commit 2fe55a8) — same protocol, verified by
// extracting that tree and running its CLI on the same fixture bytes (v2.3.0
// fingerprinted the RAW per-sample serverDelta, so shifting ONE clock column
// changed every serverDelta by a constant, minted a fresh fingerprint, and
// the copy escaped the class before the time-anchor check could run):
//   CE-7 (A4-2 client-only shift):     old stdout `verdict=GO qualifier=none brackets=120 distinguishable=true`
//   CE-7b (A4-1 millisecond nudge):    old stdout `verdict=GO qualifier=none brackets=120 distinguishable=true`
// The CE-8 pair pins the sixth-round serverDate-deletion defect, which
// survived until v2.4.0 (commit c48ce17) — same protocol, verified by
// extracting that tree and running its CLI on the same fixture bytes (v2.4.0
// kept the serverDelta column and its missing-pattern in the merge key, so
// deleting a SINGLE serverDate field from a byte-copy minted a fresh
// fingerprint and the copy escaped the class before the time-anchor check
// could run):
//   CE-8 (A4-1 drop-one relabels):     old stdout `verdict=GO qualifier=none brackets=120 distinguishable=true`
//   CE-8b (A4-2 shift + drop-one):     old stdout `verdict=GO qualifier=none brackets=120 distinguishable=true`
// This file asserts the CURRENT analyzer answers NO_PRODUCTION_CHANGE with the
// specific gate unsatisfied for the specific reason, while the go-gate test
// keeps proving that a genuinely satisfying fixture still reaches GO.

import test from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import { readFileSync } from "node:fs";
import { makeTmpDir, cleanup, makeRunDir, makeTickSeries, makeSqliteDb, sampleRow, runAnalyzer } from "./fixtures.mjs";

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
  // Campus-conflicted streams are barred from ALL evidence, not just from
  // A4-1 coverage: with every stream contested, nothing supports any gate.
  assert.equal(json.provenance.excludedStreamIds.length, 3);
  assert.equal(json.bracketTotals.total, 120);
  assert.equal(json.bracketTotals.excludedFromEvidence, 120);
  assert.equal(json.comparison.commonInformativeCount, 0);
  assert.equal(json.comparison.distinguishable, false);
  assert.deepEqual(
    json.decision.reasons.map((r) => r.split(" ")[0]),
    ["A4-1", "A4-2", "A4-3", "A4-4", "A4-5", "A4-6"],
  );
});

test("CE-1b (A4-1): a clean tiny series cannot piggyback on an excluded duplicate's windows", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // Piggyback (second R1 review round): one real qualifying NB series is
  // byte-copied into runNK-copy/runCM-copy (one campus-conflicted class), and
  // three trivially distinct 3-bracket tiny series claim NB/NK/CM. On analyzer
  // v2.0.0 (7260da8) the tiny clean classes borrowed the duplicate's
  // qualifying windows through the shared targetId and the run reached
  // `verdict=GO qualifier=none brackets=129` with all six gates satisfied.
  const s1 = [
    ...makeTickSeries({ baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1 }),
    ...makeTickSeries({ baseMs: PEAK_BASE, periodMs: 30000, count: 20, startSeq: 100 }),
  ];
  makeRunDir(join(dir, "runNB"), { campus: "NB", samples: s1 });
  makeRunDir(join(dir, "runNK-copy"), { campus: "NK", samples: s1 });
  makeRunDir(join(dir, "runCM-copy"), { campus: "CM", samples: s1 });
  let seq = 900;
  for (const campus of ["NB", "NK", "CM"]) {
    const tiny = makeTickSeries({
      baseMs: OFF_PEAK_BASE + 7 * 3600 * 1000, periodMs: 31000, count: 3,
      startSeq: seq, bodyPrefix: `${campus}-tiny-`,
    });
    seq += 50;
    makeRunDir(join(dir, `run${campus}-tiny`), { campus, samples: tiny });
  }

  const out = analyze(dir, ["runNB", "runNK-copy", "runCM-copy", "runNB-tiny", "runNK-tiny", "runCM-tiny"]);
  assert.equal(out.code, 0, out.stderr);
  assert.match(out.stdout, /verdict=NO_PRODUCTION_CHANGE qualifier=DATA_REQUIRED/);

  const json = out.json;
  // The duplicated trio is one campus-conflicted class; the tiny series are
  // three clean classes of their own.
  const conflicted = json.provenance.classes.filter((c) => c.campusConflict);
  assert.equal(conflicted.length, 1);
  assert.equal(conflicted[0].members.length, 3);
  assert.equal(json.provenance.classes.length, 4);
  assert.deepEqual(
    json.provenance.excludedStreamIds,
    conflicted[0].members.map((m) => m.streamId).sort(),
  );
  // No clean class qualifies from its own streams (3 brackets < 5), so no
  // campus is covered — the duplicate's windows lend nothing.
  const a1 = gateById(json, "A4-1");
  assert.equal(a1.satisfied, false);
  assert.match(a1.evidence, /campuses: none; NB missing; NK missing; CM missing/);
  // And the excluded brackets do not feed the other gates either: only the
  // 9 tiny brackets remain for the comparison (below the 10-bracket floor).
  assert.equal(json.bracketTotals.total, 129);
  assert.equal(json.bracketTotals.excludedFromEvidence, 120);
  assert.equal(json.comparison.commonInformativeCount, 9);
  assert.equal(json.comparison.distinguishable, false);
  assert.equal(gateById(json, "A4-2").satisfied, false);
  assert.match(gateById(json, "A4-2").evidence, /\(6 excluded: conflicted \(campus or time-anchor\) or duplicate provenance\)/);
  assert.equal(json.decision.verdict, "NO_PRODUCTION_CHANGE");
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

// Shared geometry for the CE-5 pair: the whole 30 s signal lives in ONE
// odd-tick NB capture (2 even + 10 odd ticks per window on the 60 s circle);
// NK and CM are individually 60 s-compatible even-grid series. Without
// duplication this is exactly the CE-4 family: NO-GO via whole-target
// leave-out. The attacks below try to fake target independence by duplicating
// the NB observation data under other labels.
const ce5OddTicks = (base) => [
  ...Array.from({ length: 2 }, (_, i) => base + i * 60000),
  ...Array.from({ length: 10 }, (_, i) => base + (i + 1) * 60000 + 30000),
];
const ce5EvenTicks = (base) => Array.from({ length: 6 }, (_, i) => base + i * 60000);
function ce5NbSamples() {
  return [
    ...makeTickSeries({ ticks: ce5OddTicks(OFF_PEAK_BASE), startSeq: 1, bodyPrefix: "NB-tv" }),
    ...makeTickSeries({ ticks: ce5OddTicks(PEAK_BASE), startSeq: 500, bodyPrefix: "NB-tv" }),
  ];
}

test("CE-5 (A4-1/A4-6): term-relabeled byte-copies of one capture are one target, not three", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // The SAME odd-tick NB capture written byte-identically into three run dirs
  // whose run.json uri differs only in term=9/1/7 (same campus NB — no campus
  // conflict), plus the weak even-grid NK/CM series. On analyzer v2.1.0
  // (6795fed) the three copies stayed evidence-eligible (only campus conflicts
  // were flagged): target-LOO counted 5 targets, three of them one provenance
  // class, and the comparison n was inflated by the copies.
  const nbSamples = ce5NbSamples();
  for (const term of ["9", "1", "7"]) {
    makeRunDir(join(dir, `runNB-t${term}`), { campus: "NB", term, samples: nbSamples });
  }
  makeRunDir(join(dir, "runNK"), {
    campus: "NK",
    samples: makeTickSeries({ ticks: ce5EvenTicks(OFF_PEAK_BASE), startSeq: 1, bodyPrefix: "NK-tv" }),
  });
  makeRunDir(join(dir, "runCM"), {
    campus: "CM",
    samples: makeTickSeries({ ticks: ce5EvenTicks(OFF_PEAK_BASE), startSeq: 1, bodyPrefix: "CM-tv" }),
  });

  const out = analyze(dir, ["runNB-t9", "runNB-t1", "runNB-t7", "runNK", "runCM"]);
  assert.equal(out.code, 0, out.stderr);
  assert.match(out.stdout, /verdict=NO_PRODUCTION_CHANGE qualifier=none/);

  const json = out.json;
  // One provenance class holds all three relabeled copies; the representative
  // (lowest streamId among equal lengths: term 1) alone stays evidence-eligible.
  assert.equal(json.provenance.classes.length, 3);
  const nbClass = json.provenance.classes.find((c) => c.members.length === 3);
  assert.equal(nbClass.campusConflict, false);
  assert.equal(nbClass.campus, "NB");
  assert.deepEqual(
    json.provenance.duplicateStreamIds,
    nbClass.members.filter((m) => m.relation !== "representative").map((m) => m.streamId).sort(),
  );
  assert.equal(json.provenance.duplicateStreamIds.length, 2);
  assert.deepEqual(json.provenance.excludedStreamIds, []);
  // The copies' 48 brackets are barred from evidence: the comparison runs on
  // the same 36 brackets the honest single-copy run would use.
  assert.equal(json.bracketTotals.total, 84);
  assert.equal(json.bracketTotals.excludedFromEvidence, 48);
  assert.equal(json.comparison.commonInformativeCount, 36);
  // A4-1 is honestly satisfied (there really is NB+NK+CM data) — the failure
  // is A4-6: with duplicates carrying no evidence, target-LOO sees 3 targets
  // and the winner collapses when the one real NB series is held out.
  const a1 = gateById(json, "A4-1");
  assert.equal(a1.satisfied, true);
  assert.match(a1.evidence, /2 duplicate stream\(s\) \(identical\/contained observation series\) excluded from evidence/);
  const st = json.comparison.stability;
  assert.equal(st.targets.count, 3);
  assert.equal(st.targets.pass, false);
  const a6 = gateById(json, "A4-6");
  assert.equal(a6.satisfied, false);
  assert.match(a6.evidence, /target-LOO failed: held-out soc:2026:1:NB/);
  assert.deepEqual(
    json.decision.reasons.map((r) => r.split(" ")[0]),
    ["A4-6"],
  );

  // Cross-check: the identical data WITHOUT duplication reaches the same
  // decision for the same reason — duplication changed nothing.
  const single = analyze(dir, ["runNB-t9", "runNK", "runCM"]);
  assert.equal(single.code, 0, single.stderr);
  assert.equal(single.json.comparison.commonInformativeCount, 36);
  assert.deepEqual(
    single.json.decision.reasons.map((r) => r.split(" ")[0]),
    ["A4-6"],
  );
});

test("CE-5b (A4-1/A4-6): the same capture re-fed as SQLite content counts once, not twice", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // The identical observation series fed once as NDJSON (campus NB) and once
  // as a SQLite database under a different target id. Duplicate detection
  // already merged them into one class on v2.1.0 (6795fed), but both members
  // stayed evidence-eligible: target-LOO counted the pair as 2 independent
  // targets and the comparison double-counted the brackets.
  const nbSamples = ce5NbSamples();
  makeRunDir(join(dir, "runNB"), { campus: "NB", samples: nbSamples });
  makeSqliteDb(join(dir, "dup.sqlite"), {
    targets: [{
      targetId: "batch-NB-mirror",
      observations: nbSamples.map((row, i) => ({
        seq: i + 1,
        observedAtMs: Date.parse(row.requestStartedUtc),
        bodySha: row.decodedBodySha256,
        responseDateMs: row.serverDate === null ? null : Date.parse(row.serverDate),
        bodyChanged: i === 0 ? 0 : nbSamples[i - 1].decodedBodySha256 === row.decodedBodySha256 ? 0 : 1,
      })),
    }],
  });
  makeRunDir(join(dir, "runNK"), {
    campus: "NK",
    samples: makeTickSeries({ ticks: ce5EvenTicks(OFF_PEAK_BASE), startSeq: 1, bodyPrefix: "NK-dv" }),
  });
  makeRunDir(join(dir, "runCM"), {
    campus: "CM",
    samples: makeTickSeries({ ticks: ce5EvenTicks(OFF_PEAK_BASE), startSeq: 1, bodyPrefix: "CM-dv" }),
  });

  const out = runAnalyzer([
    "--ndjson", join(dir, "runNB"),
    "--sqlite", join(dir, "dup.sqlite"),
    "--ndjson", join(dir, "runNK"),
    "--ndjson", join(dir, "runCM"),
    "--out-json", join(dir, "out.json"),
    "--out-md", join(dir, "out.md"),
  ]);
  assert.equal(out.code, 0, out.stderr);
  assert.match(out.stdout, /verdict=NO_PRODUCTION_CHANGE qualifier=none/);

  const json = out.json;
  // The NDJSON and SQLite streams merged into one class; the SQLite stream is
  // the representative (equal length, lowest streamId), the NDJSON copy is the
  // excluded duplicate. The class still carries campus NB from its member.
  const dupClass = json.provenance.classes.find((c) => c.members.length === 2);
  assert.equal(dupClass.campusConflict, false);
  assert.equal(dupClass.campus, "NB");
  assert.equal(dupClass.members[0].relation, "representative");
  assert.match(dupClass.members[0].streamId, /^dup\.sqlite::db:batch-NB-mirror$/);
  assert.deepEqual(json.provenance.duplicateStreamIds, [dupClass.members[1].streamId]);
  assert.equal(json.bracketTotals.excludedFromEvidence, 24);
  // Target-LOO sees 3 targets (mirror, NK, CM) and fails when the one real
  // series is held out — the duplicated content cannot back itself up.
  const st = json.comparison.stability;
  assert.equal(st.targets.count, 3);
  const a6 = gateById(json, "A4-6");
  assert.equal(a6.satisfied, false);
  assert.match(a6.evidence, /target-LOO failed: held-out db:batch-NB-mirror/);
  assert.deepEqual(
    json.decision.reasons.map((r) => r.split(" ")[0]),
    ["A4-6"],
  );
});

// ---- CE-6 pair: time-translated duplicates (fourth round) ------------------
// The canonical provenance series is deliberately translation-invariant, so a
// byte-copy of a capture shifted by a whole number of seconds still merges
// into the genuine capture's class. On v2.2.0 (34158c9) the class then chose a
// single representative (longest member, streamId ascending on ties) and only
// barred the OTHERS — letting the attacker elect the FABRICATED peak-hour
// timeline as the sole evidence-eligible member and supply the only qualifying
// A4-2 peak evidence. Now such a class is a time-anchor conflict: every member
// is excluded from all evidence, like a campus conflict.

// The exact per-campus tick shift used by both CE-6 fixtures: a joint client+
// server translation by a whole number of seconds (here: whole minutes), which
// preserves the canonical delta series byte-for-byte.
function ce6ShiftRows(rows, shiftMs, startSeq = 1) {
  return rows.map((row, i) => sampleRow({
    seq: startSeq + i,
    startMs: Date.parse(row.requestStartedUtc) + shiftMs,
    elapsedMs: row.elapsedMilliseconds,
    bodySha: row.decodedBodySha256,
    serverDateMs: row.serverDate === null ? null : Date.parse(row.serverDate) + shiftMs,
  }));
}

// Rich but OFF-PEAK-ONLY NK/CM runs (two off-peak sessions each, 3 h apart):
// in the honest CE-6 world nobody has peak data, so A4-2 must stay unsatisfied.
function ce6MakeOffPeakOnly(dir) {
  for (const c of ["NK", "CM"]) {
    makeRunDir(join(dir, `run${c}`), {
      campus: c,
      samples: [
        ...makeTickSeries({ baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1, bodyPrefix: `${c}-jv`, ...CAMPUS_SHAPE[c] }),
        ...makeTickSeries({ baseMs: OFF_PEAK_BASE + 3 * 3600 * 1000, periodMs: 30000, count: 20, startSeq: 500, bodyPrefix: `${c}-jw`, ...CAMPUS_SHAPE[c] }),
      ],
    });
  }
}

test("CE-6 (A4-2): a time-translated byte-copy cannot supply the only peak evidence", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // Genuine NB capture: off-peak only. The attack adds a byte-copy translated
  // into the 17:00-18:00 ET peak hour, relabeled term=1 so its streamId sorts
  // BEFORE the genuine term=9 stream and wins the v2.2.0 representative
  // tie-break among equal lengths.
  const nbOff = makeTickSeries({ baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1, bodyPrefix: "NB-jv", ...CAMPUS_SHAPE.NB });
  makeRunDir(join(dir, "runNB-t9"), { campus: "NB", term: "9", samples: nbOff });
  makeRunDir(join(dir, "runNB-t1"), {
    campus: "NB",
    term: "1",
    samples: ce6ShiftRows(nbOff, PEAK_BASE - OFF_PEAK_BASE),
  });
  ce6MakeOffPeakOnly(dir);

  const out = analyze(dir, ["runNB-t1", "runNB-t9", "runNK", "runCM"]);
  assert.equal(out.code, 0, out.stderr);
  assert.match(out.stdout, /verdict=NO_PRODUCTION_CHANGE/);

  const json = out.json;
  // The two NB streams merge (identical canonical series) but disagree about
  // WHEN the data happened: time-anchor conflict, nobody counts.
  const nbClass = json.provenance.classes.find((c) => c.members.length === 2);
  assert.equal(nbClass.timeConflict, true);
  assert.equal(nbClass.campusConflict, false);
  assert.deepEqual(
    json.provenance.excludedStreamIds,
    nbClass.members.map((m) => m.streamId).sort(),
  );
  assert.deepEqual(json.provenance.duplicateStreamIds, []);
  // The fabricated peak window is gone: zero qualifying peak evidence, and NB
  // has no evidence-eligible class left, so A4-1 fails too.
  const a2 = gateById(json, "A4-2");
  assert.equal(a2.satisfied, false);
  assert.match(a2.evidence, /qualifying peak \(>=\d+ informative in-peak brackets\): 0/);
  const a1 = gateById(json, "A4-1");
  assert.equal(a1.satisfied, false);
  assert.match(a1.evidence, /conflicting absolute time anchors \(time-translated or serverDate-edited duplicate observation series\)/);

  // Honest run (no shifted copy): same A4-2 failure — the copy changed nothing.
  const honest = analyze(dir, ["runNB-t9", "runNK", "runCM"]);
  assert.equal(honest.code, 0, honest.stderr);
  assert.match(honest.stdout, /verdict=NO_PRODUCTION_CHANGE/);
  assert.equal(gateById(honest.json, "A4-2").satisfied, false);
});

test("CE-6b (A4-2): a LONGER shifted copy cannot win the representative slot by length", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // Same attack without the naming tie-break: the shifted copy appends one
  // extra tick continuing the body chain (NB-kv20 -> NB-kv21), so the genuine
  // capture becomes a CONTAINED prefix and the fake is the longest member —
  // the v2.2.0 representative regardless of stream naming.
  const nbOff = makeTickSeries({ baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1, bodyPrefix: "NB-kv", ...CAMPUS_SHAPE.NB });
  makeRunDir(join(dir, "runNB-a"), { campus: "NB", term: "9", samples: nbOff });
  const shifted = ce6ShiftRows(nbOff, PEAK_BASE - OFF_PEAK_BASE);
  const tick20 = PEAK_BASE + 20 * 30000;
  const extra = [
    sampleRow({ seq: 41, startMs: tick20 - CAMPUS_SHAPE.NB.preMs, bodySha: "NB-kv20", serverDateMs: Math.floor((tick20 - CAMPUS_SHAPE.NB.preMs) / 1000) * 1000 }),
    sampleRow({ seq: 42, startMs: tick20 + CAMPUS_SHAPE.NB.postMs, bodySha: "NB-kv21", serverDateMs: Math.floor((tick20 + CAMPUS_SHAPE.NB.postMs) / 1000) * 1000 }),
  ];
  makeRunDir(join(dir, "runNB-z"), { campus: "NB", term: "1", samples: [...shifted, ...extra] });
  for (const c of ["NK", "CM"]) {
    makeRunDir(join(dir, `run${c}`), {
      campus: c,
      samples: [
        ...makeTickSeries({ baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1, bodyPrefix: `${c}-kv`, ...CAMPUS_SHAPE[c] }),
        ...makeTickSeries({ baseMs: OFF_PEAK_BASE + 3 * 3600 * 1000, periodMs: 30000, count: 20, startSeq: 500, bodyPrefix: `${c}-kw`, ...CAMPUS_SHAPE[c] }),
      ],
    });
  }

  const out = analyze(dir, ["runNB-a", "runNB-z", "runNK", "runCM"]);
  assert.equal(out.code, 0, out.stderr);
  assert.match(out.stdout, /verdict=NO_PRODUCTION_CHANGE/);

  const json = out.json;
  const nbClass = json.provenance.classes.find((c) => c.members.length === 2);
  // The fake IS the representative (longest) and the genuine capture is
  // contained — but their absolute times disagree at every alignment offset,
  // so the whole class is time-conflicted and excluded.
  assert.match(nbClass.members[0].streamId, /runNB-z/);
  assert.equal(nbClass.members[1].relation, "contained");
  assert.equal(nbClass.timeConflict, true);
  assert.deepEqual(
    json.provenance.excludedStreamIds,
    nbClass.members.map((m) => m.streamId).sort(),
  );
  assert.deepEqual(json.provenance.duplicateStreamIds, []);
  const a2 = gateById(json, "A4-2");
  assert.equal(a2.satisfied, false);
  assert.match(a2.evidence, /qualifying peak \(>=\d+ informative in-peak brackets\): 0/);
  assert.equal(gateById(json, "A4-1").satisfied, false);
});

// ---- CE-7 pair: single-column time translations (fifth round) --------------
// v2.3.0's canonical series keyed on the RAW per-sample serverDelta
// (serverDateMs - clientStartMs). Translating ONE clock column — client
// timestamps only, or serverDates only, by hours or by 1 ms — changes every
// serverDelta by a constant, so the copy got a fresh fingerprint, formed its
// own "independent" clean class, and the time-anchor check never ran. The
// canonical series now carries no server column at all (bodySha + client
// deltas only), so such copies merge into the genuine capture's class, where
// the time-anchor check — client anchor AND exact per-sample serverDate
// agreement — voids the class.

// Client-only translation: requestStartedUtc shifted, serverDate UNCHANGED.
function ce7ShiftClientOnly(rows, shiftMs, startSeq = 1) {
  return rows.map((row, i) => sampleRow({
    seq: startSeq + i,
    startMs: Date.parse(row.requestStartedUtc) + shiftMs,
    elapsedMs: row.elapsedMilliseconds,
    bodySha: row.decodedBodySha256,
    serverDateMs: row.serverDate === null ? null : Date.parse(row.serverDate),
  }));
}

test("CE-7 (A4-2): a CLIENT-ONLY shifted byte-copy cannot supply the only peak evidence", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // CE-6 geometry, but the copy's shift touches only the client column: the
  // genuine NB capture is off-peak only, and the attack adds a copy whose
  // client timestamps are translated into the 17:00-18:00 ET peak hour while
  // the serverDates keep the original off-peak values. On v2.3.0 the copy's
  // raw serverDeltas all differed by the shift constant, its fingerprint was
  // fresh, and the fake class supplied both NB's A4-1 coverage and the ONLY
  // qualifying A4-2 peak window.
  const nbOff = makeTickSeries({ baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1, bodyPrefix: "NB-jv", ...CAMPUS_SHAPE.NB });
  makeRunDir(join(dir, "runNB-t9"), { campus: "NB", term: "9", samples: nbOff });
  makeRunDir(join(dir, "runNB-t1"), {
    campus: "NB",
    term: "1",
    samples: ce7ShiftClientOnly(nbOff, PEAK_BASE - OFF_PEAK_BASE),
  });
  ce6MakeOffPeakOnly(dir);

  const out = analyze(dir, ["runNB-t1", "runNB-t9", "runNK", "runCM"]);
  assert.equal(out.code, 0, out.stderr);
  assert.match(out.stdout, /verdict=NO_PRODUCTION_CHANGE/);

  const json = out.json;
  // The two NB streams merge despite the changed serverDelta column (the
  // canonical form is shift-invariant per column) and disagree about WHEN:
  // time-anchor conflict, nobody counts.
  const nbClass = json.provenance.classes.find((c) => c.members.length === 2);
  assert.equal(nbClass.timeConflict, true);
  assert.equal(nbClass.campusConflict, false);
  assert.deepEqual(
    json.provenance.excludedStreamIds,
    nbClass.members.map((m) => m.streamId).sort(),
  );
  assert.deepEqual(json.provenance.duplicateStreamIds, []);
  const a2 = gateById(json, "A4-2");
  assert.equal(a2.satisfied, false);
  assert.match(a2.evidence, /qualifying peak \(>=\d+ informative in-peak brackets\): 0/);
  const a1 = gateById(json, "A4-1");
  assert.equal(a1.satisfied, false);
  assert.match(a1.evidence, /NB missing/);
  assert.match(a1.evidence, /conflicting absolute time anchors \(time-translated or serverDate-edited duplicate observation series\)/);

  // Honest run (no shifted copy): same A4-2 failure — the copy changed nothing.
  const honest = analyze(dir, ["runNB-t9", "runNK", "runCM"]);
  assert.equal(honest.code, 0, honest.stderr);
  assert.match(honest.stdout, /verdict=NO_PRODUCTION_CHANGE/);
  assert.equal(gateById(honest.json, "A4-2").satisfied, false);
});

test("CE-7b (A4-1): millisecond-nudged relabeled copies of one capture are one class, not three targets", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // The adjudicated root cause 4 (CE-1: copy-and-relabel) resurrected with a
  // 1 ms edit: ONE capture (off-peak + peak sessions) copied into three run
  // dirs relabeled NB/NK/CM, with the copies' client timestamps nudged by
  // +1 ms / +2 ms and serverDates untouched. On v2.3.0 the nudges minted
  // three distinct fingerprints — three "independent" clean classes — and the
  // run reached GO with all six gates.
  const series = [
    ...makeTickSeries({ baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1 }),
    ...makeTickSeries({ baseMs: PEAK_BASE, periodMs: 30000, count: 20, startSeq: 100 }),
  ];
  makeRunDir(join(dir, "runNB"), { campus: "NB", samples: series });
  makeRunDir(join(dir, "runNK"), { campus: "NK", samples: ce7ShiftClientOnly(series, 1) });
  makeRunDir(join(dir, "runCM"), { campus: "CM", samples: ce7ShiftClientOnly(series, 2) });

  const out = analyze(dir, ["runNB", "runNK", "runCM"]);
  assert.equal(out.code, 0, out.stderr);
  assert.match(out.stdout, /verdict=NO_PRODUCTION_CHANGE qualifier=DATA_REQUIRED/);

  const json = out.json;
  // All three streams are ONE provenance class again — conflicted on BOTH
  // axes: three campus labels on one series, and three disagreeing timelines.
  assert.equal(json.provenance.classes.length, 1);
  assert.equal(json.provenance.classes[0].campusConflict, true);
  assert.equal(json.provenance.classes[0].timeConflict, true);
  assert.equal(json.provenance.classes[0].campus, null);
  assert.equal(json.provenance.excludedStreamIds.length, 3);
  assert.deepEqual(json.provenance.duplicateStreamIds, []);
  const a1 = gateById(json, "A4-1");
  assert.equal(a1.satisfied, false);
  assert.match(a1.evidence, /NB missing; NK missing; CM missing/);
  // With every stream contested, nothing supports any gate.
  assert.equal(json.bracketTotals.total, 120);
  assert.equal(json.bracketTotals.excludedFromEvidence, 120);
  assert.equal(json.comparison.commonInformativeCount, 0);
  assert.deepEqual(
    json.decision.reasons.map((r) => r.split(" ")[0]),
    ["A4-1", "A4-2", "A4-3", "A4-4", "A4-5", "A4-6"],
  );
});

// ---- CE-8 pair: serverDate deletions on a copy (sixth round) ---------------
// v2.4.0's canonical series still carried the serverDelta column (re-based)
// and its missing-pattern ("-" vs number per sample), and containment
// rejected any presence mismatch. Deleting a SINGLE serverDate field from a
// byte-copy therefore minted a fresh fingerprint that neither matched nor was
// contained — the copy landed in its own "independent" clean class and the
// time-anchor conflict machinery never saw it. The canonical series now
// carries no server column at all, so any serverDate deletion or edit on a
// copy merges into the genuine capture's class, where the exact per-sample
// serverDate agreement check (value AND presence) voids it.

// A copy of `rows` with the serverDate at index `dropIdx` deleted and nothing
// else touched (client timestamps, bodies, and every other serverDate keep
// the exact recorded values).
function ce8DropOne(rows, dropIdx, startSeq = 1) {
  return rows.map((row, i) => sampleRow({
    seq: startSeq + i,
    startMs: Date.parse(row.requestStartedUtc),
    elapsedMs: row.elapsedMilliseconds,
    bodySha: row.decodedBodySha256,
    serverDateMs:
      i === dropIdx || row.serverDate === null ? null : Date.parse(row.serverDate),
  }));
}

test("CE-8 (A4-1): relabeled copies that each drop one serverDate are one class, not three targets", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // CE-1 resurrected with a one-FIELD deletion instead of a timestamp edit:
  // ONE capture (off-peak + peak sessions) copied into three run dirs
  // relabeled NB/NK/CM with ZERO timestamp edits; copies 2 and 3 each delete
  // one serverDate at a different sample index. On v2.4.0 the distinct
  // missing-patterns minted three distinct fingerprints — three "independent"
  // clean classes — and the run reached GO with all six gates.
  const series = [
    ...makeTickSeries({ baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1 }),
    ...makeTickSeries({ baseMs: PEAK_BASE, periodMs: 30000, count: 20, startSeq: 100 }),
  ];
  makeRunDir(join(dir, "runNB"), { campus: "NB", samples: series });
  makeRunDir(join(dir, "runNK"), { campus: "NK", samples: ce8DropOne(series, 5) });
  makeRunDir(join(dir, "runCM"), { campus: "CM", samples: ce8DropOne(series, 7) });

  const out = analyze(dir, ["runNB", "runNK", "runCM"]);
  assert.equal(out.code, 0, out.stderr);
  assert.match(out.stdout, /verdict=NO_PRODUCTION_CHANGE qualifier=DATA_REQUIRED/);

  const json = out.json;
  // All three streams are ONE provenance class conflicted on BOTH axes: three
  // campus labels on one series, and disagreeing serverDate records.
  assert.equal(json.provenance.classes.length, 1);
  assert.equal(json.provenance.classes[0].campusConflict, true);
  assert.equal(json.provenance.classes[0].timeConflict, true);
  assert.equal(json.provenance.classes[0].campus, null);
  assert.equal(json.provenance.excludedStreamIds.length, 3);
  assert.deepEqual(json.provenance.duplicateStreamIds, []);
  const a1 = gateById(json, "A4-1");
  assert.equal(a1.satisfied, false);
  assert.match(a1.evidence, /NB missing; NK missing; CM missing/);
  // With every stream contested, nothing supports any gate.
  assert.equal(json.bracketTotals.total, 120);
  assert.equal(json.bracketTotals.excludedFromEvidence, 120);
  assert.equal(json.comparison.commonInformativeCount, 0);
  assert.deepEqual(
    json.decision.reasons.map((r) => r.split(" ")[0]),
    ["A4-1", "A4-2", "A4-3", "A4-4", "A4-5", "A4-6"],
  );
});

test("CE-8b (A4-2): a client-shifted copy that also drops one serverDate cannot supply the only peak evidence", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  // CE-7 geometry where the client-shifted peak copy ALSO deletes the
  // serverDate at sample index 10. On v2.4.0 that presence mismatch kept the
  // copy out of the genuine capture's class; it kept 19/20 serverDates,
  // supplied NB's A4-1 coverage AND the only qualifying A4-2 peak window
  // (the wall-clock peak test uses client bounds), passed A4-5 with 6/6
  // qualifying groups, and the run reached GO.
  const nbOff = makeTickSeries({ baseMs: OFF_PEAK_BASE, periodMs: 30000, count: 20, startSeq: 1, bodyPrefix: "NB-jv", ...CAMPUS_SHAPE.NB });
  makeRunDir(join(dir, "runNB-t9"), { campus: "NB", term: "9", samples: nbOff });
  makeRunDir(join(dir, "runNB-t1"), {
    campus: "NB",
    term: "1",
    samples: ce8DropOne(ce7ShiftClientOnly(nbOff, PEAK_BASE - OFF_PEAK_BASE), 10),
  });
  ce6MakeOffPeakOnly(dir);

  const out = analyze(dir, ["runNB-t1", "runNB-t9", "runNK", "runCM"]);
  assert.equal(out.code, 0, out.stderr);
  assert.match(out.stdout, /verdict=NO_PRODUCTION_CHANGE/);

  const json = out.json;
  // The two NB streams merge despite the deleted serverDate (the canonical
  // form carries no server column) and disagree on the client anchor AND the
  // server record: time-anchor conflict, nobody counts.
  const nbClass = json.provenance.classes.find((c) => c.members.length === 2);
  assert.equal(nbClass.timeConflict, true);
  assert.equal(nbClass.campusConflict, false);
  assert.deepEqual(
    json.provenance.excludedStreamIds,
    nbClass.members.map((m) => m.streamId).sort(),
  );
  assert.deepEqual(json.provenance.duplicateStreamIds, []);
  const a2 = gateById(json, "A4-2");
  assert.equal(a2.satisfied, false);
  assert.match(a2.evidence, /qualifying peak \(>=\d+ informative in-peak brackets\): 0/);
  const a1 = gateById(json, "A4-1");
  assert.equal(a1.satisfied, false);
  assert.match(a1.evidence, /NB missing/);
  assert.match(a1.evidence, /conflicting absolute time anchors \(time-translated or serverDate-edited duplicate observation series\)/);

  // Honest run (no fabricated copy): same A4-2 failure — the copy changed nothing.
  const honest = analyze(dir, ["runNB-t9", "runNK", "runCM"]);
  assert.equal(honest.code, 0, honest.stderr);
  assert.match(honest.stdout, /verdict=NO_PRODUCTION_CHANGE/);
  assert.equal(gateById(honest.json, "A4-2").satisfied, false);
});
