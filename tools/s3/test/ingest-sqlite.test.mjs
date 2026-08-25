import test from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import { sha256File } from "../lib/stable.mjs";
import { makeTmpDir, cleanup, makeSqliteDb, runAnalyzer } from "./fixtures.mjs";

const BASE = Date.UTC(2026, 0, 6, 3, 0, 0);

function obsSeries(targetId, { count = 8, changeEvery = 3 } = {}) {
  // First body_changed=1 row is LKG establishment; later ones are real changes.
  const observations = [];
  let version = 0;
  for (let i = 0; i < count; i += 1) {
    const changed = i % changeEvery === 0; // i=0 is the LKG row
    if (changed) version += 1;
    observations.push({
      seq: i + 1,
      observedAtMs: BASE + i * 13000,
      responseDateMs: Math.floor((BASE + i * 13000) / 1000) * 1000,
      bodySha: `v${version}`,
      bodyChanged: changed ? 1 : 0,
    });
  }
  return { targetId, observations };
}

test("sqlite immutable open, initialLkg exclusion, db unchanged by full run", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  const dbPath = join(dir, "obs.sqlite");
  makeSqliteDb(dbPath, { targets: [obsSeries("92026:NB", { count: 9, changeEvery: 4 })] });
  const preSha = sha256File(dbPath);
  const out = runAnalyzer([
    "--sqlite", dbPath,
    "--out-json", join(dir, "out.json"),
    "--out-md", join(dir, "out.md"),
  ]);
  assert.equal(out.code, 0, out.stderr);
  assert.equal(sha256File(dbPath), preSha, "db file must be byte-identical after a full analyzer run");

  const input = out.json.inputs[0];
  assert.equal(input.kind, "sqlite");
  assert.equal(input.openMode, "immutable");
  assert.equal(input.sha256, preSha);
  assert.equal(input.excluded.initialLkg, 1);

  assert.equal(out.json.targets.length, 1);
  assert.equal(out.json.targets[0].targetId, "db:92026:NB");
  // changes at i=0 (LKG, excluded), i=4, i=8 → 2 brackets
  assert.equal(out.json.bracketTotals.total, 2);
});

test("--sqlite-target filters targets", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  const dbPath = join(dir, "obs.sqlite");
  makeSqliteDb(dbPath, {
    targets: [obsSeries("92026:NB"), obsSeries("92026:NK")],
  });
  const out = runAnalyzer([
    "--sqlite", dbPath,
    "--sqlite-target", "92026:NK",
    "--out-json", join(dir, "out.json"),
    "--out-md", join(dir, "out.md"),
  ]);
  assert.equal(out.code, 0, out.stderr);
  assert.equal(out.json.targets.length, 1);
  assert.equal(out.json.targets[0].targetId, "db:92026:NK");
  assert.match(out.json.normalizedCommand, /--sqlite-target 92026:NK/);
});
