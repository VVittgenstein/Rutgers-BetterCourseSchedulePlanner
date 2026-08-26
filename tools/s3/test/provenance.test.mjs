// Unit tests for observation-data provenance families (A4-1's data layer).
// The canonical series is metadata-free: identical, contiguously-contained,
// overlapping, and subsample-derived observation series merge into one family
// regardless of campus labels, input ids, or file names, and the merge is
// closed transitively. Known boundary (documented, asserted below): what is
// detected is REUSE of the same observation records — de novo fabrication of
// body content or of the time grid is not, and neither is a partition that
// reuses nothing.

import test from "node:test";
import assert from "node:assert/strict";
import { buildProvenance } from "../lib/provenance.mjs";
import {
  DERIVATION_MIN_RECORDS,
  DERIVATION_MIN_RECORDS_SHIFTED,
} from "../lib/phase.mjs";

function mkSample({ t, bodySha, serverDeltaMs = -600, endDeltaMs = 200, targetId = "soc:2026:9:NB", campus = "NB" }) {
  return {
    clientStartMs: t,
    // Normalized samples always carry a request end; the undefined branch stays
    // a fail-closed guard for streams that record none.
    clientEndMs: endDeltaMs === null ? undefined : t + endDeltaMs,
    serverDateMs: serverDeltaMs === null ? null : t + serverDeltaMs,
    bodySha,
    targetId,
    campus,
  };
}

// A simple 6-sample series: bodies a0..a5, 20 s apart.
function mkSeries({ baseMs = 1_000_000, targetId = "soc:2026:9:NB", campus = "NB", prefix = "a", n = 6, stepMs = 20000, endDeltaMs = 200 }) {
  const samples = [];
  for (let k = 0; k < n; k += 1) {
    samples.push(
      mkSample({ t: baseMs + k * stepMs, bodySha: `${prefix}${k}`, targetId, campus, endDeltaMs }),
    );
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

test("a CLIENT-ONLY constant shift (serverDate untouched) still merges — time-anchor conflict", () => {
  // Fifth review round: shifting only the client column changes every raw
  // serverDelta by the same constant. The canonical series re-bases the
  // serverDelta column, so the copy still merges — and its client anchor
  // disagrees, so the class is voided.
  const genuine = mkSeries({ baseMs: 1_000_000 });
  const shifted = genuine.map((s) => ({ ...s, clientStartMs: s.clientStartMs + 8_000_000 }));
  const p = buildProvenance([
    { inputId: "runA/samples.ndjson", samples: genuine },
    { inputId: "runB/samples.ndjson", samples: shifted },
  ]);
  assert.equal(p.classes.length, 1);
  assert.equal(p.classes[0].timeConflict, true);
  assert.equal(p.classes[0].campusConflict, false);
});

test("a SERVER-ONLY constant shift (client untouched) still merges — time-anchor conflict", () => {
  // The mirror attack: client anchors agree, but the copy claims the server
  // observed everything at different absolute times. The server anchor check
  // catches the non-zero re-basing constant.
  const genuine = mkSeries({ baseMs: 1_000_000 });
  const shifted = genuine.map((s) => ({ ...s, serverDateMs: s.serverDateMs + 3_600_000 }));
  const p = buildProvenance([
    { inputId: "runA/samples.ndjson", samples: genuine },
    { inputId: "runB/samples.ndjson", samples: shifted },
  ]);
  assert.equal(p.classes.length, 1);
  assert.equal(p.classes[0].timeConflict, true);
});

test("a 1 ms client nudge still merges — time-anchor conflict", () => {
  // The copy-and-relabel resurrection: +1 ms on the client column must not
  // mint a fresh fingerprint. Tolerance is 0 ms: genuine duplicates parse the
  // same recorded timestamp strings and agree exactly.
  const genuine = mkSeries({ baseMs: 1_000_000 });
  const nudged = genuine.map((s) => ({ ...s, clientStartMs: s.clientStartMs + 1 }));
  const p = buildProvenance([
    { inputId: "runA/samples.ndjson", samples: genuine },
    { inputId: "runB/samples.ndjson", samples: nudged },
  ]);
  assert.equal(p.classes.length, 1);
  assert.equal(p.classes[0].timeConflict, true);
});

test("a client-only-shifted copy that is one tick LONGER (contained genuine) is a time-anchor conflict", () => {
  // CE-6b geometry with a single-column shift: the genuine capture is a
  // contained slice of the longer shifted fake. Content matches at offset 0,
  // but neither the client anchor nor the server re-basing constant agrees.
  const genuine = mkSeries({ baseMs: 1_000_000, n: 6 });
  const shiftMs = 8_000_000;
  const shifted = genuine.map((s) => ({ ...s, clientStartMs: s.clientStartMs + shiftMs }));
  const extra = {
    ...mkSample({ t: 1_000_000 + shiftMs + 6 * 20000, bodySha: "a6y" }),
    serverDateMs: 1_000_000 + 6 * 20000 - 600, // server column stays un-shifted
  };
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

test("an every-other-sample subsample merges as a DERIVED member of the source family", () => {
  const full = mkSeries({ n: 8 });
  const everyOther = full.filter((_, i) => i % 2 === 0);
  const p = buildProvenance([
    { inputId: "full/samples.ndjson", samples: full },
    { inputId: "sub/samples.ndjson", samples: everyOther },
  ]);
  // The subsample's deltas are doubled, so it is neither identical nor a
  // contiguous slice - but it reuses the source's actual observation records,
  // which is what the derived relation keys on.
  assert.equal(p.classes.length, 1);
  assert.equal(p.classes[0].members[0].streamId, "full/samples.ndjson::soc:2026:9:NB");
  assert.equal(p.classes[0].members[1].relation, "derived");
  assert.equal(p.classes[0].members[1].relatedTo, "full/samples.ndjson::soc:2026:9:NB");
  assert.equal(p.classes[0].members[1].matchedCount, 4);
  // Genuine records, genuinely reused: no record disagreement.
  assert.equal(p.classes[0].timeConflict, false);
  assert.deepEqual(p.classes[0].timeConflictPairs, []);
});

test("a constant-shifted every-other subsample merges DERIVED and is a time conflict", () => {
  // STAGE-5-R3 A2-1, at unit scale: the same regular subsample as the test
  // above, with its WHOLE client clock moved +1 ms. Under the old absolute
  // (clientStartMs, bodySha) record key every key changed and the copy posed
  // as an independent capture. It must now merge on the constant offset and
  // then FAIL the absolute anchor check, which is still exactly 0 ms.
  const full = mkSeries({ n: 14 });
  const shifted = full
    .filter((_, i) => i % 2 === 0)
    .map((s) => ({ ...s, clientStartMs: s.clientStartMs + 1, clientEndMs: s.clientEndMs + 1 }));
  // The pre-fix rule provably finds nothing: no shared absolute record key.
  const fullKeys = new Set(full.map((s) => `${s.clientStartMs}|${s.bodySha}`));
  assert.equal(shifted.filter((s) => fullKeys.has(`${s.clientStartMs}|${s.bodySha}`)).length, 0);
  // ...while every shifted record is one of the source's, offset by exactly 1.
  assert.equal(
    shifted.filter((s) => fullKeys.has(`${s.clientStartMs - 1}|${s.bodySha}`)).length,
    shifted.length,
  );
  assert.ok(shifted.length >= DERIVATION_MIN_RECORDS_SHIFTED);

  const p = buildProvenance([
    { inputId: "src/samples.ndjson", samples: full },
    { inputId: "shifted/samples.ndjson", samples: shifted.map((s) => ({ ...s, campus: "NK" })) },
  ]);
  assert.equal(p.classes.length, 1);
  assert.equal(p.classes[0].members[1].relation, "derived");
  assert.equal(p.classes[0].members[1].matchedCount, shifted.length);
  assert.equal(p.classes[0].timeConflict, true);
  assert.equal(p.classes[0].campusConflict, true);
  assert.equal(p.classes[0].timeConflictPairs.length, 1);
  assert.equal(p.classes[0].timeConflictPairs[0].relation, "derived");
  assert.equal(p.classes[0].timeConflictPairs[0].offsetMs, 1);
});

test("ANTI-LOCKOUT: two honest captures sharing bodies but no single offset stay independent", () => {
  // The false-positive risk the shifted threshold has to survive. Two honest
  // captures of ONE origin, so they legitimately observe the SAME body
  // sequence, polled at 13 s and 17 s from start instants 500 ms apart. They
  // never share an absolute instant (every candidate offset is 500 mod 1000)
  // and no single constant offset explains more than a handful of records.
  const EPOCH = 1_000_000_000;
  const bodyAt = (t) => `srv${Math.floor((t - EPOCH) / 30000)}`;
  const capture = (stepMs, startOffsetMs, n) => {
    const out = [];
    for (let i = 0; i < n; i += 1) {
      const t = EPOCH + startOffsetMs + i * stepMs;
      out.push(mkSample({ t, bodySha: bodyAt(t) }));
    }
    return out;
  };
  const a = capture(13000, 0, 60);
  const b = capture(17000, 500, 45);
  // Heavy body sharing is the point: this is NOT a disjoint-namespace control.
  const aBodies = new Set(a.map((s) => s.bodySha));
  assert.ok([...new Set(b.map((s) => s.bodySha))].filter((x) => aBodies.has(x)).length >= 20);
  // Recomputed inline: the largest same-body constant-offset bucket, which is
  // an UPPER bound on any alignment the matcher could return.
  const buckets = new Map();
  for (const y of b) {
    for (const x of a) {
      if (x.bodySha !== y.bodySha) continue;
      const d = y.clientStartMs - x.clientStartMs;
      buckets.set(d, (buckets.get(d) ?? 0) + 1);
    }
  }
  const largest = Math.max(...buckets.values());
  assert.ok(largest < DERIVATION_MIN_RECORDS_SHIFTED, `largest offset bucket ${largest}`);
  assert.equal(buckets.get(0), undefined, "no shared absolute instant at all");

  const p = buildProvenance([
    { inputId: "capA/samples.ndjson", samples: a },
    { inputId: "capB/samples.ndjson", samples: b },
  ]);
  assert.equal(p.classes.length, 2);
  for (const cls of p.classes) {
    assert.equal(cls.members.length, 1);
    assert.equal(cls.timeConflict, false);
  }
});

test("DEFERRED BOUNDARY: per-sample jitter does NOT merge (invariance is to ONE offset)", () => {
  // The documented edge of the model (STAGE-5-R3 A2-1/A3: per-sample irregular
  // jitter stays deferred). Every subsampled record carries its OWN offset, so
  // the offsets never coincide and no bucket reaches the shifted threshold.
  // The matcher cannot drift into this case: the offset match is exact integer
  // equality, never a tolerance window.
  const full = mkSeries({ n: 30 });
  const jittered = full
    .filter((_, i) => i % 2 === 0)
    .map((s, k) => {
      const d = 1 + ((k * 3) % 7);
      return { ...s, clientStartMs: s.clientStartMs + d, clientEndMs: s.clientEndMs + d };
    });
  const buckets = new Map();
  for (const y of jittered) {
    for (const x of full) {
      if (x.bodySha !== y.bodySha) continue;
      const d = y.clientStartMs - x.clientStartMs;
      buckets.set(d, (buckets.get(d) ?? 0) + 1);
    }
  }
  const largest = Math.max(...buckets.values());
  // Deliberately recorded: the modal bucket DOES clear the offset-0 threshold.
  // Reusing DERIVATION_MIN_RECORDS at non-zero offsets would have swept this
  // deferred case in; DERIVATION_MIN_RECORDS_SHIFTED is what keeps it out.
  assert.ok(largest >= DERIVATION_MIN_RECORDS, `largest offset bucket ${largest}`);
  assert.ok(largest < DERIVATION_MIN_RECORDS_SHIFTED, `largest offset bucket ${largest}`);
  const p = buildProvenance([
    { inputId: "src/samples.ndjson", samples: full },
    { inputId: "jit/samples.ndjson", samples: jittered.map((s) => ({ ...s, campus: "NK" })) },
  ]);
  assert.equal(p.classes.length, 2);
});

test("three staggered overlapping slices of one capture are ONE family", () => {
  // Equal-length slices: none is contained in another, and each pair shares a
  // long contiguous run of (bodySha, clientDelta) entries spanning changes.
  const full = mkSeries({ n: 20 });
  const slices = [full.slice(0, 12), full.slice(4, 16), full.slice(8, 20)];
  const p = buildProvenance(
    slices.map((samples, i) => ({
      inputId: `slice${i}/samples.ndjson`,
      samples: samples.map((s) => ({ ...s, campus: ["NB", "NK", "CM"][i] })),
    })),
  );
  assert.equal(p.classes.length, 1);
  assert.deepEqual(
    p.classes[0].members.map((m) => m.relation),
    ["representative", "overlapping", "overlapping"],
  );
  for (const m of p.classes[0].members.slice(1)) {
    assert.notEqual(m.relatedTo, null);
    assert.ok(m.matchedCount >= 4, `matchedCount ${m.matchedCount}`);
  }
  // Three campus labels on one body of captured data: no coverage at all.
  assert.equal(p.classes[0].campusConflict, true);
  assert.equal(p.classes[0].campus, null);
  assert.equal(p.classes[0].timeConflict, false);
});

test("A~B and B~C but A NOT~C still collapses to ONE family (transitive union)", () => {
  // Two disjoint, phase-staggered subsamples of one source: each reuses the
  // source's records, neither reuses the other's. Only the transitive closure
  // keeps them from posing as two independent captures.
  const full = mkSeries({ n: 12 });
  const even = full.filter((_, i) => i % 2 === 0);
  const odd = full.filter((_, i) => i % 2 === 1);
  const evenKeys = new Set(even.map((s) => `${s.clientStartMs}\t${s.bodySha}`));
  assert.equal(odd.filter((s) => evenKeys.has(`${s.clientStartMs}\t${s.bodySha}`)).length, 0);
  const p = buildProvenance([
    { inputId: "src/samples.ndjson", samples: full },
    { inputId: "even/samples.ndjson", samples: even.map((s) => ({ ...s, campus: "NK" })) },
    { inputId: "odd/samples.ndjson", samples: odd.map((s) => ({ ...s, campus: "CM" })) },
  ]);
  assert.equal(p.classes.length, 1);
  assert.equal(p.classes[0].members.length, 3);
  assert.equal(p.classes[0].campusConflict, true);
  // Without the source stream the two halves are genuinely unrelated data.
  const split = buildProvenance([
    { inputId: "even/samples.ndjson", samples: even },
    { inputId: "odd/samples.ndjson", samples: odd },
  ]);
  assert.equal(split.classes.length, 2);
});

test("a family whose NON-tree edge disagrees is still a time conflict", () => {
  // src-A and src-B are clean; A-B disagrees on the recorded request ends at
  // the samples they share. Judging only the spanning tree would miss it.
  const full = mkSeries({ n: 12 });
  const even = full.filter((_, i) => i % 2 === 0);
  const evenEdited = even.map((s) => ({ ...s, clientEndMs: s.clientEndMs + 5000 }));
  const p = buildProvenance([
    { inputId: "src/samples.ndjson", samples: full },
    { inputId: "a-even/samples.ndjson", samples: even },
    { inputId: "b-even/samples.ndjson", samples: evenEdited },
  ]);
  assert.equal(p.classes.length, 1);
  assert.equal(p.classes[0].timeConflict, true);
  assert.ok(p.classes[0].timeConflictPairs.length >= 1);
});

test("a request-end-only edited copy merges and is a record conflict", () => {
  // The A2-2 provenance hole: the fingerprint is content-only, so a copy that
  // rewrote ONLY requestEndedUtc keeps the same fingerprint and merges - and
  // the record agreement check (which now covers the client END) voids it,
  // instead of letting the edited copy win the representative slot.
  const genuine = mkSeries({});
  const endEdited = genuine.map((s) => ({ ...s, clientEndMs: s.clientEndMs + 150000 }));
  const p = buildProvenance([
    { inputId: "runA/samples.ndjson", samples: genuine },
    { inputId: "runB/samples.ndjson", samples: endEdited },
  ]);
  assert.equal(p.classes.length, 1);
  assert.equal(p.classes[0].timeConflict, true);
  assert.equal(p.classes[0].campusConflict, false);
});

test("clientEndObserved false on one side suppresses the request-end comparison", () => {
  // A SQLite stream records no request end (clientEndMs === clientStartMs).
  // Comparing it against an NDJSON stream's real ends would flag every honest
  // cross-format duplicate, so the end check is suppressed for such a pair.
  const ndjson = mkSeries({});
  const sqlite = ndjson.map((s) => ({
    ...s,
    clientEndMs: s.clientStartMs,
    targetId: "db:soc-2026-9-NB",
    campus: null,
  }));
  const p = buildProvenance([
    { inputId: "run/samples.ndjson", samples: ndjson },
    { inputId: "capture.sqlite", samples: sqlite },
  ]);
  assert.equal(
    p.streams.find((x) => x.streamId.startsWith("capture.sqlite")).clientEndObserved,
    false,
  );
  assert.equal(p.streams.find((x) => x.streamId.startsWith("run/")).clientEndObserved, true);
  assert.equal(p.classes.length, 1);
  assert.equal(p.classes[0].timeConflict, false);
});

test("a copy with ONE serverDate deleted still merges — time-anchor conflict", () => {
  // Sixth review round: serverDates are NOT part of the merge key (neither
  // values nor missing-pattern). Deleting a single serverDate field from a
  // byte-copy must not mint a fresh fingerprint and escape the class; it
  // merges, and the record disagreement (presence mismatch at the aligned
  // sample) voids the class.
  const genuine = mkSeries({});
  const oneDropped = genuine.map((s, i) => (i === 3 ? { ...s, serverDateMs: null } : s));
  const p = buildProvenance([
    { inputId: "runA/samples.ndjson", samples: genuine },
    { inputId: "runB/samples.ndjson", samples: oneDropped },
  ]);
  assert.equal(p.classes.length, 1);
  assert.equal(p.classes[0].timeConflict, true);
  assert.equal(p.classes[0].campusConflict, false);
});

test("a copy with ALL serverDates deleted still merges — time-anchor conflict", () => {
  const withServer = mkSeries({});
  const withoutServer = withServer.map((s) => ({ ...s, serverDateMs: null }));
  const p = buildProvenance([
    { inputId: "runA/samples.ndjson", samples: withServer },
    { inputId: "runB/samples.ndjson", samples: withoutServer },
  ]);
  assert.equal(p.classes.length, 1);
  assert.equal(p.classes[0].timeConflict, true);
});

test("a copy with ONE serverDate edited by 1 ms still merges — time-anchor conflict", () => {
  // A per-sample server edit changes one rawServerDelta: under a merge key
  // that carried the serverDelta column this too minted a fresh fingerprint.
  const genuine = mkSeries({});
  const edited = genuine.map((s, i) => (i === 2 ? { ...s, serverDateMs: s.serverDateMs + 1 } : s));
  const p = buildProvenance([
    { inputId: "runA/samples.ndjson", samples: genuine },
    { inputId: "runB/samples.ndjson", samples: edited },
  ]);
  assert.equal(p.classes.length, 1);
  assert.equal(p.classes[0].timeConflict, true);
});

test("two serverDate-free byte-copies at the same absolute times merge WITHOUT conflict", () => {
  // Agreement on absence is agreement: honest duplicates of a capture that
  // never recorded serverDates must not be flagged.
  const bare = mkSeries({}).map((s) => ({ ...s, serverDateMs: null }));
  const p = buildProvenance([
    { inputId: "runA/samples.ndjson", samples: bare },
    { inputId: "runB/samples.ndjson", samples: bare },
  ]);
  assert.equal(p.classes.length, 1);
  assert.equal(p.classes[0].timeConflict, false);
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
