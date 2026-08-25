// End-to-end run against the real (gitignored, local-only) capture data.
// Skips cleanly when the data is not present; asserts no literal numbers
// beyond the pre-registered verdict/flags — everything else is recomputed
// independently inside this test.

import test from "node:test";
import assert from "node:assert/strict";
import { join, resolve, dirname } from "node:path";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import process from "node:process";
import { makeTmpDir, cleanup, runAnalyzer } from "./fixtures.mjs";

const worktreeRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const dataRoot =
  process.env.BCSP_OPEN_SECTIONS_REPRO_DIR ??
  resolve(worktreeRoot, "../../../data/open-sections-repro");

const RUN_DIRS = [
  "20260820T033701043Z-5745bc4a",
  "20260820T033527487Z-d92a8a35",
  "20260820T033451393Z-a8a40f98",
];
const D1 = RUN_DIRS[0];

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

test("committed data: verdict, gates, and independent recomputation", (t) => {
  if (!RUN_DIRS.every((d) => existsSync(join(dataRoot, d, "samples.ndjson")))) {
    t.skip(`open-sections-repro data not present: ${dataRoot}`);
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
