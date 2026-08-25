import test from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import { existsSync } from "node:fs";
import { makeTmpDir, cleanup, makeRunDir, sampleRow, runAnalyzer } from "./fixtures.mjs";

const BASE = Date.UTC(2026, 0, 6, 3, 0, 0);

function expectFailClosed(t, name, { samples, rawLines }, code) {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  makeRunDir(join(dir, "runA"), { samples, rawLines });
  const outJson = join(dir, "out.json");
  const outMd = join(dir, "out.md");
  const out = runAnalyzer(["--ndjson", join(dir, "runA"), "--out-json", outJson, "--out-md", outMd]);
  assert.equal(out.code, 2, `${name}: expected exit 2, stderr=${out.stderr}`);
  assert.match(out.stderr, new RegExp(`ERROR ${code}`), name);
  assert.equal(existsSync(outJson), false, `${name}: no JSON output may be written`);
  assert.equal(existsSync(outMd), false, `${name}: no MD output may be written`);
}

test("schemaVersion 2 → E_SCHEMA_VERSION, no outputs", (t) => {
  expectFailClosed(t, "schema", {
    samples: [
      sampleRow({ seq: 1, startMs: BASE, bodySha: "a", serverDateMs: BASE }),
      sampleRow({ seq: 2, startMs: BASE + 13000, bodySha: "a", serverDateMs: BASE, schemaVersion: 2 }),
    ],
  }, "E_SCHEMA_VERSION");
});

test("missing requestStartedUtc → E_MISSING_FIELD, no outputs", (t) => {
  expectFailClosed(t, "missing-field", {
    samples: [
      sampleRow({ seq: 1, startMs: BASE, bodySha: "a", serverDateMs: BASE }),
      sampleRow({ seq: 2, startMs: BASE + 13000, bodySha: "a", serverDateMs: BASE, omit: ["requestStartedUtc"] }),
    ],
  }, "E_MISSING_FIELD");
});

test("duplicate sequence → E_SEQUENCE_ORDER, no outputs", (t) => {
  expectFailClosed(t, "dup-seq", {
    samples: [
      sampleRow({ seq: 3, startMs: BASE, bodySha: "a", serverDateMs: BASE }),
      sampleRow({ seq: 3, startMs: BASE + 13000, bodySha: "a", serverDateMs: BASE }),
    ],
  }, "E_SEQUENCE_ORDER");
});

test("regressing sequence → E_SEQUENCE_ORDER, no outputs", (t) => {
  expectFailClosed(t, "regress-seq", {
    samples: [
      sampleRow({ seq: 5, startMs: BASE, bodySha: "a", serverDateMs: BASE }),
      sampleRow({ seq: 4, startMs: BASE + 13000, bodySha: "a", serverDateMs: BASE }),
    ],
  }, "E_SEQUENCE_ORDER");
});

test("client time regression > 2 s → E_TIME_REGRESSION, no outputs", (t) => {
  expectFailClosed(t, "time-regress", {
    samples: [
      sampleRow({ seq: 1, startMs: BASE, bodySha: "a", serverDateMs: BASE }),
      sampleRow({ seq: 2, startMs: BASE - 3000, bodySha: "a", serverDateMs: BASE }),
    ],
  }, "E_TIME_REGRESSION");
});

test("broken JSON line → E_NDJSON_PARSE, no outputs", (t) => {
  expectFailClosed(t, "bad-json", {
    rawLines: [
      JSON.stringify(sampleRow({ seq: 1, startMs: BASE, bodySha: "a", serverDateMs: BASE })),
      "{not valid json",
    ],
  }, "E_NDJSON_PARSE");
});

test("missing samples.ndjson → E_INPUT_MISSING, exit 2", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  const out = runAnalyzer([
    "--ndjson", join(dir, "does-not-exist"),
    "--out-json", join(dir, "out.json"),
    "--out-md", join(dir, "out.md"),
  ]);
  assert.equal(out.code, 2);
  assert.match(out.stderr, /ERROR E_INPUT_MISSING/);
});

test("usage errors exit 1", () => {
  const out = runAnalyzer(["--nope"]);
  assert.equal(out.code, 1);
  assert.match(out.stderr, /ERROR E_USAGE/);
  const out2 = runAnalyzer([]);
  assert.equal(out2.code, 1);
});
