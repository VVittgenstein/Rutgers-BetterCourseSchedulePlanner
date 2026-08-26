// Unit tests for observation-data provenance classes (A4-1's data layer).
// The canonical series is metadata-free: identical or contiguously-contained
// observation series merge into one class regardless of campus labels, input
// ids, or file names. Known boundary (documented, asserted below): derived
// series such as every-other-sample subsampling change the delta structure
// and are NOT merged — this defends against copy-and-relabel, not forensics.

import test from "node:test";
import assert from "node:assert/strict";
import { buildProvenance } from "../lib/provenance.mjs";

function mkSample({ t, bodySha, serverDeltaMs = -600, targetId = "soc:2026:9:NB", campus = "NB" }) {
  return {
    clientStartMs: t,
    serverDateMs: serverDeltaMs === null ? null : t + serverDeltaMs,
    bodySha,
    targetId,
    campus,
  };
}

// A simple 6-sample series: bodies a0..a5, 20 s apart.
function mkSeries({ baseMs = 1_000_000, targetId = "soc:2026:9:NB", campus = "NB", prefix = "a", n = 6, stepMs = 20000 }) {
  const samples = [];
  for (let k = 0; k < n; k += 1) {
    samples.push(mkSample({ t: baseMs + k * stepMs, bodySha: `${prefix}${k}`, targetId, campus }));
  }
  return samples;
}

test("identical series (relabeled copy) merge into one class", () => {
  const samples = mkSeries({});
  const p = buildProvenance([
    { inputId: "runNB/samples.ndjson", samples },
    {
      inputId: "runNK/samples.ndjson",
      samples: samples.map((s) => ({ ...s, targetId: "soc:2026:9:NK", campus: "NK" })),
    },
  ]);
  assert.equal(p.streams.length, 2);
  assert.equal(p.streams[0].seriesFingerprint, p.streams[1].seriesFingerprint);
  assert.equal(p.classes.length, 1);
  const cls = p.classes[0];
  assert.equal(cls.members.length, 2);
  assert.equal(cls.members[0].relation, "representative");
  assert.equal(cls.members[1].relation, "identical");
  // Conflicting campus labels on one observation series → no campus credit.
  assert.equal(cls.campusConflict, true);
  assert.equal(cls.campus, null);
});

test("identical series under the SAME campus label is one class without conflict", () => {
  const samples = mkSeries({});
  const p = buildProvenance([
    { inputId: "runA/samples.ndjson", samples },
    { inputId: "runB/samples.ndjson", samples },
  ]);
  assert.equal(p.classes.length, 1);
  assert.equal(p.classes[0].campusConflict, false);
  // Byte-copies carry the exact same timestamps: no time-anchor conflict.
  assert.equal(p.classes[0].timeConflict, false);
  assert.equal(p.classes[0].campus, "NB");
});

test("a pure joint time translation still merges — and flags a time-anchor conflict", () => {
  const p = buildProvenance([
    { inputId: "runA/samples.ndjson", samples: mkSeries({ baseMs: 1_000_000 }) },
    { inputId: "runB/samples.ndjson", samples: mkSeries({ baseMs: 9_000_000 }) },
  ]);
  assert.equal(p.classes.length, 1);
  // The canonical series is translation-invariant, so the copies merge; but
  // the members disagree about WHEN the shared data happened, so the class
  // is time-conflicted and the analyzer bars every member from evidence.
  assert.equal(p.classes[0].timeConflict, true);
  assert.equal(p.classes[0].campusConflict, false);
});

test("a time-translated copy that is one tick LONGER (contained genuine) is a time-anchor conflict", () => {
  // n5 geometry: the genuine capture is a contained prefix of a shifted copy
  // that wins the representative slot by length. The genuine member matches
  // the representative's content at offset 0 but not its absolute times.
  const genuine = mkSeries({ baseMs: 1_000_000, n: 6 });
  const shiftMs = 8_000_000;
  const shifted = genuine.map((s) => ({
    ...s,
    clientStartMs: s.clientStartMs + shiftMs,
    serverDateMs: s.serverDateMs === null ? null : s.serverDateMs + shiftMs,
  }));
  const extra = mkSample({ t: 1_000_000 + shiftMs + 6 * 20000, bodySha: "a6x" });
  const p = buildProvenance([
    { inputId: "runA/samples.ndjson", samples: genuine },
    { inputId: "runZ/samples.ndjson", samples: [...shifted, extra] },
  ]);
  assert.equal(p.classes.length, 1);
  assert.equal(p.classes[0].members[0].streamId, "runZ/samples.ndjson::soc:2026:9:NB");
  assert.equal(p.classes[0].members[1].relation, "contained");
  assert.equal(p.classes[0].timeConflict, true);
});

test("prefix, suffix, and middle slices of a copy are 'contained' in the class", () => {
  const full = mkSeries({ n: 8 });
  const prefixSlice = full.slice(0, 4);
  const middleSlice = full.slice(2, 6);
  const suffixSlice = full.slice(5);
  const p = buildProvenance([
    { inputId: "full/samples.ndjson", samples: full },
    { inputId: "pre/samples.ndjson", samples: prefixSlice },
    { inputId: "mid/samples.ndjson", samples: middleSlice },
    { inputId: "suf/samples.ndjson", samples: suffixSlice },
  ]);
  assert.equal(p.classes.length, 1);
  const relations = p.classes[0].members.map((m) => m.relation);
  assert.deepEqual(relations, ["representative", "contained", "contained", "contained"]);
  // The longest stream is the representative regardless of input order.
  assert.equal(p.classes[0].members[0].streamId, "full/samples.ndjson::soc:2026:9:NB");
  // Genuine truncations keep the original absolute times: no time conflict.
  assert.equal(p.classes[0].timeConflict, false);
});

test("different delta structure (different cadence/phase geometry) does NOT merge", () => {
  const p = buildProvenance([
    { inputId: "runA/samples.ndjson", samples: mkSeries({ stepMs: 20000 }) },
    { inputId: "runB/samples.ndjson", samples: mkSeries({ stepMs: 21000 }) },
  ]);
  assert.equal(p.classes.length, 2);
});

test("different bodySha series does NOT merge", () => {
  const p = buildProvenance([
    { inputId: "runA/samples.ndjson", samples: mkSeries({ prefix: "a" }) },
    { inputId: "runB/samples.ndjson", samples: mkSeries({ prefix: "b" }) },
  ]);
  assert.equal(p.classes.length, 2);
});

test("KNOWN BOUNDARY: every-other-sample subsampling is not detected as derived", () => {
  const full = mkSeries({ n: 8 });
  const everyOther = full.filter((_, i) => i % 2 === 0);
  const p = buildProvenance([
    { inputId: "full/samples.ndjson", samples: full },
    { inputId: "sub/samples.ndjson", samples: everyOther },
  ]);
  // The subsample's deltas are doubled: not a contiguous slice, so it forms
  // its own class. This is the documented limit of the provenance check.
  assert.equal(p.classes.length, 2);
});

test("serverDate presence is part of the canonical series", () => {
  const withServer = mkSeries({});
  const withoutServer = withServer.map((s) => ({ ...s, serverDateMs: null }));
  const p = buildProvenance([
    { inputId: "runA/samples.ndjson", samples: withServer },
    { inputId: "runB/samples.ndjson", samples: withoutServer },
  ]);
  assert.equal(p.classes.length, 2);
});

test("cross-format duplicate (NDJSON vs SQLite normalized samples) merges when timestamps agree", () => {
  // SQLite streams normalize observed_at into clientStartMs, so a database
  // holding the same observation series as an NDJSON run has the same
  // canonical form — a duplicated SQLite copy cannot add a provenance class.
  const ndjson = mkSeries({});
  const sqlite = ndjson.map((s) => ({
    ...s,
    targetId: "db:soc-2026-9-NB",
    campus: null,
  }));
  const p = buildProvenance([
    { inputId: "run/samples.ndjson", samples: ndjson },
    { inputId: "capture.sqlite", samples: sqlite },
  ]);
  assert.equal(p.classes.length, 1);
  // One member carries a campus, the other none: no conflict, campus kept.
  assert.equal(p.classes[0].campusConflict, false);
  assert.equal(p.classes[0].timeConflict, false);
  assert.equal(p.classes[0].campus, "NB");
});

test("empty streams carry no observation data and are skipped", () => {
  const p = buildProvenance([
    { inputId: "empty/samples.ndjson", samples: [] },
    { inputId: "run/samples.ndjson", samples: mkSeries({}) },
  ]);
  assert.equal(p.streams.length, 1);
  assert.equal(p.classes.length, 1);
});
