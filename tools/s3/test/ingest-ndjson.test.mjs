import test from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import { parseIsoMs, parseHttpDate } from "../lib/timeparse.mjs";
import { makeTmpDir, cleanup, makeRunDir, makeTickSeries, sampleRow, runAnalyzer } from "./fixtures.mjs";

const BASE = Date.UTC(2026, 0, 6, 3, 0, 0);

test("parseIsoMs handles sub-ms fractions and offsets", () => {
  assert.equal(
    parseIsoMs("2026-08-20T03:37:01.1211177+00:00"),
    Date.parse("2026-08-20T03:37:01.121Z"),
  );
  assert.equal(
    parseIsoMs("2026-08-19T23:37:01.1211177-04:00"),
    Date.parse("2026-08-20T03:37:01.121Z"),
  );
  assert.equal(parseIsoMs("2026-08-20T03:37:01Z"), Date.parse("2026-08-20T03:37:01.000Z"));
  assert.equal(parseIsoMs("2026-08-20T03:37:01.5Z"), Date.parse("2026-08-20T03:37:01.500Z"));
  assert.equal(parseIsoMs("garbage"), null);
});

test("parseHttpDate handles IMF-fixdate and rejects garbage", () => {
  assert.equal(parseHttpDate("Thu, 20 Aug 2026 03:37:00 GMT"), Date.parse("2026-08-20T03:37:00Z"));
  assert.equal(parseHttpDate("not a date"), null);
  assert.equal(parseHttpDate(null), null);
});

test("run.json parsing yields soc target id", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  makeRunDir(join(dir, "runA"), {
    campus: "NB",
    samples: makeTickSeries({ baseMs: BASE, periodMs: 30000, count: 6 }),
  });
  const out = runAnalyzer([
    "--ndjson", join(dir, "runA"),
    "--out-json", join(dir, "out.json"),
    "--out-md", join(dir, "out.md"),
  ]);
  assert.equal(out.code, 0, out.stderr);
  assert.equal(out.json.targets.length, 1);
  assert.equal(out.json.targets[0].targetId, "soc:2026:9:NB");
  assert.equal(out.json.targets[0].campus, "NB");
  assert.equal(out.json.targets[0].term, "9");
});

test("missing run.json degrades to unknown target and A4-1 evidence reflects it", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  makeRunDir(join(dir, "runA"), {
    noRunJson: true,
    samples: makeTickSeries({ baseMs: BASE, periodMs: 30000, count: 6 }),
  });
  const out = runAnalyzer([
    "--ndjson", join(dir, "runA"),
    "--out-json", join(dir, "out.json"),
    "--out-md", join(dir, "out.md"),
  ]);
  assert.equal(out.code, 0, out.stderr);
  assert.equal(out.json.targets.length, 1);
  assert.ok(out.json.targets[0].targetId.startsWith("unknown:"), out.json.targets[0].targetId);
  assert.equal(out.json.targets[0].campus, null);
  const a1 = out.json.goGate.find((g) => g.id === "A4-1");
  assert.equal(a1.satisfied, false);
  assert.match(a1.evidence, /NB missing/);
});

test("row-level exclusions are counted per reason, first match wins", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  const rows = makeTickSeries({ baseMs: BASE, periodMs: 30000, count: 4 });
  let seq = rows.length;
  const mk = (opts) => sampleRow({ seq: (seq += 1), startMs: BASE + 200000 + seq * 1000, bodySha: "v4", ...opts });
  rows.push(mk({ curlExitCode: 7, httpStatus: 500, validationErrors: ["both set: curl wins"] }));
  rows.push(mk({ httpStatus: 503 }));
  rows.push(mk({ validationErrors: ["bad row"] }));
  makeRunDir(join(dir, "runA"), { samples: rows });
  const out = runAnalyzer([
    "--ndjson", join(dir, "runA"),
    "--out-json", join(dir, "out.json"),
    "--out-md", join(dir, "out.md"),
  ]);
  assert.equal(out.code, 0, out.stderr);
  const input = out.json.inputs[0];
  assert.equal(input.rows, rows.length);
  assert.deepEqual(input.excluded, {
    curlExitCode: 1,
    httpStatusNon2xx: 1,
    validationErrors: 1,
    initialLkg: 0,
  });
});
