// Unit tests for the pure sequence matchers behind provenance families.
// These functions decide nothing on their own — provenance.mjs applies the
// thresholds and the record-agreement policy — so what is pinned here is
// exactness: which blocks are found, that they are maximal, that the result is
// order-stable, and that the record alignment stays strictly increasing.

import test from "node:test";
import assert from "node:assert/strict";
import {
  containmentAlignments,
  longestCommonBlocks,
  sharedRecordAlignment,
} from "../lib/series-match.mjs";
import { DERIVATION_MIN_BLOCK } from "../lib/phase.mjs";

// Canonical entries carry more fields; the matchers only read these two.
function ent(bodySha, deltaMs) {
  return { bodySha, deltaMs };
}
function series(bodies, deltaMs = 1000) {
  return bodies.map((b) => ent(b, deltaMs));
}

test("containmentAlignments finds every offset of a contiguous slice", () => {
  const b = [ent("x", 0), ent("a", 1000), ent("b", 2000), ent("c", 1000), ent("a", 1000), ent("b", 2000)];
  // The slice's own first delta is normalized to 0 and is not compared.
  const a = [ent("a", 0), ent("b", 2000)];
  assert.deepEqual(containmentAlignments(a, b), [1, 4]);
  assert.deepEqual(containmentAlignments(b, a), []);
  assert.deepEqual(containmentAlignments([], b), []);
});

test("longestCommonBlocks: a block at the threshold is found, one below is not", () => {
  const shared = [ent("p", 500), ent("q", 500), ent("r", 500), ent("s", 500)];
  const a = [ent("z", 900), ...shared, ent("y", 900)];
  const b = [ent("w", 700), ent("v", 700), ...shared];
  const found = longestCommonBlocks(a, b, DERIVATION_MIN_BLOCK);
  assert.equal(DERIVATION_MIN_BLOCK, 4);
  assert.deepEqual(found, [{ length: 4, aStart: 1, bStart: 2 }]);
  // One entry shorter and the same threshold rejects it.
  const a3 = [ent("z", 900), ...shared.slice(0, 3), ent("y", 900)];
  const b3 = [ent("w", 700), ...shared.slice(0, 3)];
  assert.deepEqual(longestCommonBlocks(a3, b3, DERIVATION_MIN_BLOCK), []);
  assert.deepEqual(longestCommonBlocks(a3, b3, 3), [{ length: 3, aStart: 1, bStart: 1 }]);
});

test("longestCommonBlocks needs BOTH the body and the delta to agree", () => {
  const bodies = ["a", "b", "c", "d", "e"];
  const sameBodiesOtherCadence = bodies.map((x) => ent(x, 2000));
  assert.deepEqual(longestCommonBlocks(series(bodies), sameBodiesOtherCadence, 4), []);
  const sameCadenceOtherBodies = bodies.map((x) => ent(`${x}2`, 1000));
  assert.deepEqual(longestCommonBlocks(series(bodies), sameCadenceOtherBodies, 4), []);
  assert.equal(longestCommonBlocks(series(bodies), series(bodies), 4).length, 1);
});

test("longestCommonBlocks returns MAXIMAL blocks, sorted and argument-order stable", () => {
  // Two separated shared runs, so the DP must emit two maximal blocks rather
  // than one merged or several nested ones.
  const run1 = [ent("a", 100), ent("b", 100), ent("c", 100), ent("d", 100), ent("e", 100)];
  const run2 = [ent("m", 300), ent("n", 300), ent("o", 300), ent("p", 300)];
  const a = [...run1, ent("gap", 999), ...run2];
  const b = [ent("head", 555), ...run1, ent("other", 111), ent("more", 222), ...run2];
  const blocks = longestCommonBlocks(a, b, 4);
  assert.deepEqual(blocks, [
    { length: 5, aStart: 0, bStart: 1 },
    { length: 4, aStart: 6, bStart: 8 },
  ]);
  // Same blocks with the arguments swapped (roles of aStart/bStart exchanged).
  const swapped = longestCommonBlocks(b, a, 4);
  assert.deepEqual(
    swapped.map((x) => ({ length: x.length, aStart: x.bStart, bStart: x.aStart })),
    blocks,
  );
  // Sorted by (aStart, bStart).
  for (let i = 1; i < blocks.length; i += 1) {
    assert.ok(
      blocks[i - 1].aStart < blocks[i].aStart ||
        (blocks[i - 1].aStart === blocks[i].aStart && blocks[i - 1].bStart <= blocks[i].bStart),
    );
  }
});

test("longestCommonBlocks: an all-stale block IS returned (the change rule lives in provenance)", () => {
  // A constant-body, constant-cadence run is exactly the accidental coincidence
  // provenance.mjs rejects with DERIVATION_MIN_BLOCK_CHANGES. The matcher is
  // deliberately dumb about it, so the policy stays in one place.
  const stale = series(["same", "same", "same", "same", "same"]);
  const blocks = longestCommonBlocks(stale, stale, 4);
  assert.ok(blocks.length >= 1);
  assert.equal(new Set(stale.map((e) => e.bodySha)).size, 1);
});

test("longestCommonBlocks skips work when either side is shorter than the threshold", () => {
  assert.deepEqual(longestCommonBlocks(series(["a", "b"]), series(["a", "b", "c", "d"]), 4), []);
  assert.deepEqual(longestCommonBlocks([], [], 4), []);
  assert.deepEqual(longestCommonBlocks(series(["a"]), series(["a"]), 0), []);
});

test("sharedRecordAlignment is strictly increasing on both sides", () => {
  const a = ["k0", "k1", "k2", "k3", "k4", "k5"];
  const b = ["k1", "k3", "k5"];
  const pairs = sharedRecordAlignment(a, b);
  assert.deepEqual(pairs, [[1, 0], [3, 1], [5, 2]]);
  for (let i = 1; i < pairs.length; i += 1) {
    assert.ok(pairs[i][0] > pairs[i - 1][0]);
    assert.ok(pairs[i][1] > pairs[i - 1][1]);
  }
});

test("sharedRecordAlignment ignores reordering-only overlap beyond one match per index", () => {
  // A stream whose records were re-emitted out of order still matches the ones
  // it can align in order; the count is a lower bound, never an overcount.
  const a = ["k0", "k1", "k2", "k3"];
  const b = ["k3", "k2", "k1", "k0"];
  const pairs = sharedRecordAlignment(a, b);
  assert.equal(pairs.length, 1);
  assert.deepEqual(pairs, [[3, 0]]);
});

test("sharedRecordAlignment tolerates one edited record instead of dissolving the match", () => {
  const a = ["k0", "k1", "k2", "k3", "k4"];
  const edited = ["k0", "k1", "EDITED", "k3", "k4"];
  assert.equal(sharedRecordAlignment(a, edited).length, 4);
  assert.deepEqual(sharedRecordAlignment(a, []), []);
  assert.deepEqual(sharedRecordAlignment([], a), []);
});

test("sharedRecordAlignment finds nothing between disjoint key spaces", () => {
  assert.deepEqual(sharedRecordAlignment(["a", "b", "c"], ["x", "y", "z"]), []);
});
