import test from "node:test";
import assert from "node:assert/strict";
import { join } from "node:path";
import { readFileSync, writeFileSync, mkdirSync, cpSync } from "node:fs";
import { makeTmpDir, cleanup, makeRunDir, makeTickSeries, runAnalyzer } from "./fixtures.mjs";

const BASE_A = Date.UTC(2026, 0, 6, 3, 0, 0);
const BASE_B = Date.UTC(2026, 0, 6, 9, 0, 0);

function makeTwoRuns(dir) {
  makeRunDir(join(dir, "runA"), {
    samples: makeTickSeries({ baseMs: BASE_A, periodMs: 30000, count: 10 }),
  });
  makeRunDir(join(dir, "runB"), {
    samples: makeTickSeries({ baseMs: BASE_B, periodMs: 60000, count: 8 }),
  });
}

test("argument order does not change a single output byte", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  makeTwoRuns(dir);
  // Same output basenames in different directories: only the argument order
  // (and output location) differs between the two invocations.
  const out1 = runAnalyzer([
    "--ndjson", join(dir, "runA"),
    "--ndjson", join(dir, "runB"),
    "--out-json", join(dir, "d1", "out.json"),
    "--out-md", join(dir, "d1", "out.md"),
  ]);
  const out2 = runAnalyzer([
    "--ndjson", join(dir, "runB"),
    "--ndjson", join(dir, "runA"),
    "--out-json", join(dir, "d2", "out.json"),
    "--out-md", join(dir, "d2", "out.md"),
  ]);
  assert.equal(out1.code, 0, out1.stderr);
  assert.equal(out2.code, 0, out2.stderr);
  assert.equal(
    readFileSync(join(dir, "d1", "out.json"), "utf8"),
    readFileSync(join(dir, "d2", "out.json"), "utf8"),
  );
  assert.equal(
    readFileSync(join(dir, "d1", "out.md"), "utf8"),
    readFileSync(join(dir, "d2", "out.md"), "utf8"),
  );
});

test("flipping one input byte changes the input fingerprint", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  makeTwoRuns(dir);
  const out1 = runAnalyzer([
    "--ndjson", join(dir, "runA"),
    "--out-json", join(dir, "o1.json"),
    "--out-md", join(dir, "o1.md"),
  ]);
  const copyDir = join(dir, "runA-copy");
  mkdirSync(copyDir, { recursive: true });
  cpSync(join(dir, "runA"), copyDir, { recursive: true });
  const ndjsonPath = join(copyDir, "samples.ndjson");
  // flip one byte inside a sha string (stays valid NDJSON)
  const text = readFileSync(ndjsonPath, "utf8");
  const mutated = text.replace('"decodedBodySha256":"v0"', '"decodedBodySha256":"w0"');
  assert.notEqual(mutated, text);
  writeFileSync(ndjsonPath, mutated, "utf8");
  const out2 = runAnalyzer([
    "--ndjson", copyDir,
    "--out-json", join(dir, "o2.json"),
    "--out-md", join(dir, "o2.md"),
  ]);
  assert.equal(out1.code, 0, out1.stderr);
  assert.equal(out2.code, 0, out2.stderr);
  assert.notEqual(out1.json.inputs[0].sha256, out2.json.inputs[0].sha256);
});

test("outputs never leak absolute paths, hostnames, or sampleDirectory", (t) => {
  const dir = makeTmpDir();
  t.after(() => cleanup(dir));
  makeTwoRuns(dir);
  const out = runAnalyzer([
    "--ndjson", join(dir, "runA"),
    "--ndjson", join(dir, "runB"),
    "--out-json", join(dir, "o.json"),
    "--out-md", join(dir, "o.md"),
  ]);
  assert.equal(out.code, 0, out.stderr);
  for (const content of [out.jsonText, out.md]) {
    assert.doesNotMatch(content, /Z:\\|Z:\//);
    assert.doesNotMatch(content, /DESKTOP-/);
    assert.doesNotMatch(content, /Users[\\/]/);
    assert.doesNotMatch(content, /sampleDirectory/);
    assert.doesNotMatch(content, /fixture-samples/);
    // the tmp fixture root itself must not appear
    assert.ok(!content.includes(dir), "tmp dir path leaked into output");
  }
});
