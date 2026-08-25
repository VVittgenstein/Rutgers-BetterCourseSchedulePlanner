// NDJSON (samples.ndjson + run.json) ingestion into normalized samples.

import { existsSync, readFileSync, statSync } from "node:fs";
import { basename, dirname, join } from "node:path";
import { AnalyzerError } from "./errors.mjs";
import { sha256File, sha256Text } from "./stable.mjs";
import { parseIsoMs, parseHttpDate } from "./timeparse.mjs";
import { CLIENT_TIME_REGRESSION_TOLERANCE_MS } from "./phase.mjs";

function resolveNdjsonPath(rawPath) {
  if (!existsSync(rawPath)) {
    throw new AnalyzerError("E_INPUT_MISSING", `ndjson input not found: basename ${basename(rawPath)}`);
  }
  const stat = statSync(rawPath);
  const ndjsonPath = stat.isDirectory() ? join(rawPath, "samples.ndjson") : rawPath;
  const runJsonPath = join(dirname(ndjsonPath), "run.json");
  if (!existsSync(ndjsonPath)) {
    throw new AnalyzerError(
      "E_INPUT_MISSING",
      `samples.ndjson missing in run directory ${basename(rawPath)}`,
    );
  }
  return { ndjsonPath, runJsonPath: existsSync(runJsonPath) ? runJsonPath : null };
}

function parseRunJson(runJsonPath, id) {
  const raw = readFileSync(runJsonPath, "utf8");
  let run;
  try {
    run = JSON.parse(raw);
  } catch {
    throw new AnalyzerError("E_NDJSON_PARSE", `run.json unparseable for input ${id}`);
  }
  let year = null;
  let term = null;
  let campus = null;
  if (typeof run.uri === "string") {
    try {
      const url = new URL(run.uri);
      year = url.searchParams.get("year");
      term = url.searchParams.get("term");
      campus = url.searchParams.get("campus");
    } catch {
      // leave nulls; degrades to unknown target
    }
  }
  const intervalSeconds =
    typeof run.intervalSeconds === "number" && Number.isFinite(run.intervalSeconds)
      ? run.intervalSeconds
      : null;
  return { year, term, campus, intervalSeconds };
}

const REQUIRED_FIELDS = [
  "sequence",
  "requestStartedUtc",
  "requestEndedUtc",
  "curlExitCode",
  "httpStatus",
  "validationErrors",
];

export function ingestNdjson(rawPath) {
  const { ndjsonPath, runJsonPath } = resolveNdjsonPath(rawPath);
  const id = `${basename(dirname(ndjsonPath))}/samples.ndjson`;

  const ndjsonSha = sha256File(ndjsonPath);
  const runJsonSha = runJsonPath ? sha256File(runJsonPath) : null;
  const fingerprint = runJsonSha ? sha256Text(`${ndjsonSha}\n${runJsonSha}`) : ndjsonSha;

  let year = null;
  let term = null;
  let campus = null;
  let intervalSeconds = null;
  if (runJsonPath) {
    ({ year, term, campus, intervalSeconds } = parseRunJson(runJsonPath, id));
  }
  const hasTarget = year !== null && term !== null && campus !== null;
  const targetId = hasTarget ? `soc:${year}:${term}:${campus}` : `unknown:${id}`;
  if (!hasTarget) {
    year = null;
    term = null;
    campus = null;
  }

  const text = readFileSync(ndjsonPath, "utf8");
  const lines = text.split("\n");
  if (lines.length > 0 && lines[lines.length - 1] === "") {
    lines.pop();
  }

  const excluded = { curlExitCode: 0, httpStatusNon2xx: 0, validationErrors: 0, initialLkg: 0 };
  const samples = [];
  let rows = 0;
  let prevSequence = null;
  let prevStartMs = null;

  lines.forEach((line, index) => {
    const lineNo = index + 1;
    let row;
    try {
      row = JSON.parse(line);
    } catch {
      throw new AnalyzerError("E_NDJSON_PARSE", `input ${id} line ${lineNo}: invalid JSON`);
    }
    rows += 1;
    if (row.schemaVersion !== 1) {
      throw new AnalyzerError(
        "E_SCHEMA_VERSION",
        `input ${id} line ${lineNo}: schemaVersion ${row.schemaVersion} (expected 1)`,
      );
    }
    for (const field of REQUIRED_FIELDS) {
      if (row[field] === undefined || row[field] === null) {
        throw new AnalyzerError("E_MISSING_FIELD", `input ${id} line ${lineNo}: missing ${field}`);
      }
    }
    if (prevSequence !== null && row.sequence <= prevSequence) {
      throw new AnalyzerError(
        "E_SEQUENCE_ORDER",
        `input ${id} line ${lineNo}: sequence ${row.sequence} not increasing (prev ${prevSequence})`,
      );
    }
    prevSequence = row.sequence;

    const clientStartMs = parseIsoMs(row.requestStartedUtc);
    const clientEndMs = parseIsoMs(row.requestEndedUtc);
    if (clientStartMs === null || clientEndMs === null) {
      throw new AnalyzerError("E_TIME_PARSE", `input ${id} line ${lineNo}: unparseable request timestamps`);
    }
    if (prevStartMs !== null && clientStartMs < prevStartMs - CLIENT_TIME_REGRESSION_TOLERANCE_MS) {
      throw new AnalyzerError(
        "E_TIME_REGRESSION",
        `input ${id} line ${lineNo}: requestStartedUtc regressed by more than ${CLIENT_TIME_REGRESSION_TOLERANCE_MS} ms`,
      );
    }
    prevStartMs = clientStartMs;

    // Row-level exclusion: counted, not fatal; first match wins the counter.
    if (row.curlExitCode !== 0) {
      excluded.curlExitCode += 1;
      return;
    }
    if (row.httpStatus < 200 || row.httpStatus > 299) {
      excluded.httpStatusNon2xx += 1;
      return;
    }
    if (Array.isArray(row.validationErrors) && row.validationErrors.length > 0) {
      excluded.validationErrors += 1;
      return;
    }
    if (typeof row.decodedBodySha256 !== "string" || row.decodedBodySha256.length === 0) {
      throw new AnalyzerError(
        "E_MISSING_FIELD",
        `input ${id} line ${lineNo}: included row lacks decodedBodySha256`,
      );
    }

    const ageNum = row.age === null || row.age === undefined ? null : Number(row.age);
    samples.push({
      inputId: id,
      targetId,
      term,
      year,
      campus,
      seq: row.sequence,
      clientStartMs,
      clientEndMs,
      serverDateMs: parseHttpDate(row.serverDate),
      bodySha: row.decodedBodySha256,
      bodyChangedFlag: null,
      ageSeconds: Number.isFinite(ageNum) ? ageNum : null,
    });
  });

  const input = { kind: "ndjson", id, sha256: fingerprint, rows, excluded };
  return {
    input,
    samples,
    exclusions: excluded,
    intervalSeconds,
    // for post-analysis mutation check
    files: runJsonPath ? [ndjsonPath, runJsonPath] : [ndjsonPath],
    fileShas: runJsonPath ? [ndjsonSha, runJsonSha] : [ndjsonSha],
  };
}
