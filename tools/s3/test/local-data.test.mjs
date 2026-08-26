// End-to-end run against the real capture data, which is LOCAL and gitignored
// (data/open-sections-repro under the repo root) — it is never committed; only
// the derived evidence documents under docs/evidence/ are.
//
// Data-root resolution (first hit wins; the test must NOT skip because of a
// checkout layout when the data exists):
//   1. $BCSP_OPEN_SECTIONS_REPRO_DIR — when set it is used as-is, nothing
//      else is probed;
//   2. `git rev-parse --show-toplevel` → <top>/data/open-sections-repro
//      (plain repo-root checkout);
//   3. `git rev-parse --git-common-dir` → <common>/.. /data/open-sections-repro
//      (worktree checkout falling back to the shared main repo root).
// Only when no candidate exists (e.g. CI without the local data) does the test
// skip, and the skip message says exactly what was probed.
//
// Assertions use no literal numbers beyond the pre-registered verdict/flags —
// everything else is recomputed independently inside this test.

import test from "node:test";
import assert from "node:assert/strict";
import { join, resolve, dirname, isAbsolute } from "node:path";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { spawnSync } from "node:child_process";
import process from "node:process";
import { makeTmpDir, cleanup, runAnalyzer } from "./fixtures.mjs";

const testDir = dirname(fileURLToPath(import.meta.url));

const RUN_DIRS = [
  "20260820T033701043Z-5745bc4a",
  "20260820T033527487Z-d92a8a35",
  "20260820T033451393Z-a8a40f98",
];
const D1 = RUN_DIRS[0];

function gitOut(args) {
  const res = spawnSync("git", ["-C", testDir, ...args], { encoding: "utf8" });
  if (res.status !== 0 || typeof res.stdout !== "string") return null;
  const out = res.stdout.trim();
  return out.length > 0 ? out : null;
}

function resolveDataRoot() {
  const envDir = process.env.BCSP_OPEN_SECTIONS_REPRO_DIR;
  if (envDir) {
    // Explicit override: used as-is, no further probing.
    const candidates = [{ source: "BCSP_OPEN_SECTIONS_REPRO_DIR", dir: envDir }];
    return { root: existsSync(envDir) ? envDir : null, candidates };
  }
  const candidates = [];
  const top = gitOut(["rev-parse", "--show-toplevel"]);
  if (top !== null) {
    candidates.push({
      source: "git rev-parse --show-toplevel",
      dir: join(resolve(top), "data", "open-sections-repro"),
    });
  }
  const common = gitOut(["rev-parse", "--git-common-dir"]);
  if (common !== null) {
    const commonAbs = isAbsolute(common) ? common : resolve(testDir, common);
    candidates.push({
      source: "git rev-parse --git-common-dir",
      dir: join(resolve(commonAbs, ".."), "data", "open-sections-repro"),
    });
  }
  for (const candidate of candidates) {
    if (existsSync(candidate.dir)) return { root: candidate.dir, candidates };
  }
  return { root: null, candidates };
}

function includedRows(ndjsonPath) {
  return readFileSync(ndjsonPath, "utf8")
    .split("\n")
    .filter((line) => line.length > 0)
    .map((line) => JSON.parse(line))
    .filter(
      (row) =>
        row.curlExitCode === 0 &&
        row.httpStatus >= 200 &&
        row.httpStatus <= 299 &&
        (!Array.isArray(row.validationErrors) || row.validationErrors.length === 0),
    );
}

function bruteForceMaxCoverage(arcs, periodMs) {
  // O(n^2) arc-stabbing: evaluate coverage at every arc endpoint.
  const contains = (arc, p) =>
    arc.start <= arc.end ? p >= arc.start && p <= arc.end : p >= arc.start || p <= arc.end;
  let best = 0;
  for (const probe of arcs.flatMap((a) => [a.start, a.end])) {
    const covered = arcs.filter((a) => contains(a, probe)).length;
    if (covered > best) best = covered;
  }
  void periodMs;
  return best;
}

test("local data: verdict, gates, and independent recomputation", (t) => {
  const { root: dataRoot, candidates } = resolveDataRoot();
  if (dataRoot === null || !RUN_DIRS.every((d) => existsSync(join(dataRoot, d, "samples.ndjson")))) {
    const probed = candidates.map((c) => `${c.source} → ${c.dir}`).join("; ");
    t.skip(
      `local open-sections-repro data not present (never committed; set BCSP_OPEN_SECTIONS_REPRO_DIR to point at it). Probed: ${probed || "no candidates resolvable"}`,
    );
    return;
  }
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  const out = runAnalyzer([
    ...RUN_DIRS.flatMap((d) => ["--ndjson", join(dataRoot, d)]),
    "--out-json", join(dir, "out.json"),
    "--out-md", join(dir, "out.md"),
  ]);
  assert.equal(out.code, 0, out.stderr);
  const json = out.json;

  // Pre-registered outcome flags (never literal numbers).
  assert.equal(json.decision.verdict, "NO_PRODUCTION_CHANGE");
  assert.equal(json.decision.qualifier, "DATA_REQUIRED");
  assert.equal(json.comparison.distinguishable, false);
  assert.equal(json.goGate.find((g) => g.id === "A4-1").satisfied, false);
  assert.equal(json.goGate.find((g) => g.id === "A4-2").satisfied, false);
  // The comparison is not distinguishable, so stability is honestly not
  // evaluable rather than reported as trivially passing.
  assert.equal(json.comparison.stability, null);

  // Server-clock evidence covers the qualifying groups: A4-5 holds even
  // though the data gates keep the overall verdict at NO_PRODUCTION_CHANGE.
  assert.equal(json.goGate.find((g) => g.id === "A4-5").satisfied, true);
  assert.equal(json.clock.serverEvidence.sufficient, true);

  // All three inputs are NB captures: whatever way the provenance classes
  // merge (short runs may or may not be contained in the long one — both are
  // legitimate), no class may claim a campus other than NB or a conflict.
  assert.ok(json.provenance.classes.length >= 1);
  for (const cls of json.provenance.classes) {
    assert.equal(cls.campusConflict, false);
    assert.equal(cls.campus, "NB");
  }

  // Independent recount of D1 brackets: adjacent decodedBodySha256 diff over
  // included rows (minimal inline reader, no analyzer code).
  const rows = includedRows(join(dataRoot, D1, "samples.ndjson"));
  let recount = 0;
  for (let i = 1; i < rows.length; i += 1) {
    if (rows[i].decodedBodySha256 !== rows[i - 1].decodedBodySha256) recount += 1;
  }
  const d1Windows = json.targets
    .flatMap((tgt) => tgt.windows)
    .filter((w) => w.windowId.startsWith(`${D1}/samples.ndjson#`));
  const d1BracketCount = d1Windows.reduce((acc, w) => acc + w.bracketCount, 0);
  assert.equal(d1BracketCount, recount, "analyzer D1 bracket count must equal the independent recount");

  // Independent re-derivation of max coverage on the common comparison set
  // (server clock, +1000 ms upper widening, half-open (l, u] → closed [l+1, u]).
  const serverBrackets = [];
  for (let i = 1; i < rows.length; i += 1) {
    if (rows[i].decodedBodySha256 === rows[i - 1].decodedBodySha256) continue;
    const lo = Date.parse(rows[i - 1].serverDate);
    const hi = Date.parse(rows[i].serverDate) + 1000;
    if (!Number.isFinite(lo) || !Number.isFinite(hi) || hi - lo <= 0) continue;
    serverBrackets.push({ lo, hi });
  }
  for (const periodMs of [30000, 60000]) {
    const arcs = serverBrackets
      .filter((b) => b.hi - b.lo < 30000) // common comparison set: informative for BOTH periods
      .map((b) => ({
        start: (((b.lo + 1) % periodMs) + periodMs) % periodMs,
        end: ((b.hi % periodMs) + periodMs) % periodMs,
      }));
    const brute = bruteForceMaxCoverage(arcs, periodMs);
    const analyzerValue =
      periodMs === 30000 ? json.comparison.maxCoverage30Common : json.comparison.maxCoverage60Common;
    assert.equal(brute, analyzerValue, `brute-force coverage mismatch for period ${periodMs}`);
  }

  // Sanity: comparison ran on the server clock for this data.
  assert.equal(json.comparison.clockSource, "server");
  assert.equal(json.clock.status, "server-date-available");
});
