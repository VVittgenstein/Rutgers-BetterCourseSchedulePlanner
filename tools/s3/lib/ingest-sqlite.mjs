// SQLite ingestion (open_batch_observations) via node:sqlite, strictly
// readonly. Primary open mode is an immutable file: URI; plain readOnly is the
// documented fallback (recorded in inputs[].openMode).

import { existsSync } from "node:fs";
import { basename } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { AnalyzerError } from "./errors.mjs";
import { sha256File } from "./stable.mjs";
import { parseIsoMs, parseHttpDate } from "./timeparse.mjs";
import { CLIENT_TIME_REGRESSION_TOLERANCE_MS } from "./phase.mjs";

function buildImmutableUri(path) {
  const forward = path.replaceAll("\\", "/");
  const prefixed = forward.startsWith("/") ? forward : `/${forward}`;
  return `file://${encodeURI(prefixed)}?immutable=1`;
}

export function ingestSqlite(path, sqliteTargets) {
  if (!existsSync(path)) {
    throw new AnalyzerError("E_INPUT_MISSING", `sqlite input not found: basename ${basename(path)}`);
  }
  const id = basename(path);
  const preSha = sha256File(path);

  let db = null;
  let openMode = "immutable";
  try {
    db = new DatabaseSync(buildImmutableUri(path), { readOnly: true });
  } catch {
    db = new DatabaseSync(path, { readOnly: true });
    openMode = "readonly";
  }

  let rows;
  try {
    let stmt;
    try {
      stmt = db.prepare(`
        SELECT target_id, observation_sequence, observed_at, response_date,
               http_status, decoded_body_sha256, body_changed, age_seconds, classification
        FROM open_batch_observations
        ORDER BY target_id, observation_sequence
      `);
    } catch (err) {
      throw new AnalyzerError(
        "E_SQLITE_SCHEMA",
        `input ${id}: open_batch_observations not queryable (${err.message})`,
      );
    }
    rows = stmt.all();
  } finally {
    db.close();
  }

  const targetFilter = new Set(sqliteTargets ?? []);
  const excluded = { curlExitCode: 0, httpStatusNon2xx: 0, validationErrors: 0, initialLkg: 0 };
  const samplesByTarget = new Map();
  const prevSeqByTarget = new Map();
  const prevObservedByTarget = new Map();
  const lkgSkippedByTarget = new Set();
  let keptRows = 0;

  for (const row of rows) {
    if (targetFilter.size > 0 && !targetFilter.has(row.target_id)) continue;
    keptRows += 1;
    const targetId = `db:${row.target_id}`;

    const prevSeq = prevSeqByTarget.get(targetId) ?? null;
    if (prevSeq !== null && row.observation_sequence <= prevSeq) {
      throw new AnalyzerError(
        "E_SEQUENCE_ORDER",
        `input ${id} target ${row.target_id}: observation_sequence ${row.observation_sequence} not increasing`,
      );
    }
    prevSeqByTarget.set(targetId, row.observation_sequence);

    const observedMs = parseIsoMs(row.observed_at);
    if (observedMs === null) {
      throw new AnalyzerError(
        "E_TIME_PARSE",
        `input ${id} target ${row.target_id} seq ${row.observation_sequence}: unparseable observed_at`,
      );
    }

    // observed_at monotonicity per target, mirroring the NDJSON client-clock
    // check: a regression beyond the tolerance is fail-closed. Regressions
    // within the tolerance can still produce a non-positive client-width
    // bracket, which buildBrackets rejects and counts.
    const prevObservedMs = prevObservedByTarget.get(targetId) ?? null;
    if (prevObservedMs !== null && observedMs < prevObservedMs - CLIENT_TIME_REGRESSION_TOLERANCE_MS) {
      throw new AnalyzerError(
        "E_TIME_REGRESSION",
        `input ${id} target ${row.target_id} seq ${row.observation_sequence}: observed_at regressed by more than ${CLIENT_TIME_REGRESSION_TOLERANCE_MS} ms`,
      );
    }
    prevObservedByTarget.set(targetId, observedMs);

    if (row.http_status < 200 || row.http_status > 299) {
      excluded.httpStatusNon2xx += 1;
      continue;
    }

    // The first body_changed=1 row per target only establishes LKG; it is not
    // evidence of a change instant and must be excluded from brackets.
    let bodyChangedFlag = row.body_changed;
    if (bodyChangedFlag === 1 && !lkgSkippedByTarget.has(targetId)) {
      lkgSkippedByTarget.add(targetId);
      excluded.initialLkg += 1;
      bodyChangedFlag = 0;
    }

    if (!samplesByTarget.has(targetId)) samplesByTarget.set(targetId, []);
    samplesByTarget.get(targetId).push({
      inputId: id,
      targetId,
      term: null,
      year: null,
      campus: null,
      seq: row.observation_sequence,
      clientStartMs: observedMs,
      clientEndMs: observedMs,
      serverDateMs: parseHttpDate(row.response_date),
      bodySha: row.decoded_body_sha256,
      bodyChangedFlag,
      ageSeconds:
        row.age_seconds === null || row.age_seconds === undefined ? null : Number(row.age_seconds),
    });
  }

  const input = { kind: "sqlite", id, sha256: preSha, openMode, rows: keptRows, excluded };
  return {
    input,
    samplesByTarget,
    exclusions: excluded,
    files: [path],
    fileShas: [preSha],
  };
}
