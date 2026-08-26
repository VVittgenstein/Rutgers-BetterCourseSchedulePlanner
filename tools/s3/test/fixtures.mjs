// Synthetic fixture generators for the analyzer test suite.
// Everything is deterministic; temp dirs live under os.tmpdir() and are
// removed by the tests that create them.

import { mkdtempSync, writeFileSync, mkdirSync, existsSync, readFileSync, rmSync } from "node:fs";
import { join, dirname, resolve } from "node:path";
import { tmpdir } from "node:os";
import { spawnSync } from "node:child_process";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath } from "node:url";
import process from "node:process";

export const ANALYZER_PATH = resolve(
  dirname(fileURLToPath(import.meta.url)),
  "..",
  "rebuild-profile-analyzer.mjs",
);

export function makeTmpDir() {
  return mkdtempSync(join(tmpdir(), "s3-analyzer-"));
}

export function cleanup(dir) {
  rmSync(dir, { recursive: true, force: true });
}

function toRfc1123(ms) {
  return new Date(ms).toUTCString();
}

// Emits one full schemaVersion:1 NDJSON row. `sampleDirectory` is set to a
// fixture-local relative string to prove it is never propagated to outputs.
export function sampleRow({
  seq,
  startMs,
  elapsedMs = 200,
  bodySha,
  serverDateMs = null,
  curlExitCode = 0,
  httpStatus = 200,
  validationErrors = [],
  schemaVersion = 1,
  age = null,
  omit = [],
}) {
  const row = {
    schemaVersion,
    sequence: seq,
    sampleId: `fixture_${String(seq).padStart(6, "0")}`,
    requestStartedUtc: new Date(startMs).toISOString(),
    requestEndedUtc: new Date(startMs + elapsedMs).toISOString(),
    requestStartedLocal: new Date(startMs).toISOString(),
    elapsedMilliseconds: elapsedMs,
    curlExitCode,
    httpStatus,
    httpVersion: "1.1",
    remoteIp: "192.0.2.1",
    serverDate: serverDateMs === null ? null : toRfc1123(serverDateMs),
    xServerName: "fixture-backend",
    etag: `etag-${seq % 3}`,
    contentType: "application/json",
    contentEncoding: "gzip",
    declaredContentLength: 1000,
    bodyBytes: 1000,
    age,
    via: null,
    lastModified: null,
    cacheControl: "max-age=30",
    vary: null,
    rawBodySha256: `raw-${bodySha}`,
    decodedBodySha256: bodySha,
    rawArrayCount: 10,
    uniqueIndexCount: 10,
    duplicateCount: 0,
    invalidCount: 0,
    canonicalSetSha256: `canon-${bodySha}`,
    eligibleForVolumeAnalysis: true,
    baselineCount: null,
    ratioToBaseline: null,
    dropCandidate: false,
    validationErrors,
    sampleDirectory: `fixture-samples/row-${seq}`,
  };
  for (const key of omit) delete row[key];
  return row;
}

// Writes run.json + samples.ndjson into dir.
export function makeRunDir(dir, { campus = "NB", year = "2026", term = "9", intervalSeconds = 13, samples, noRunJson = false, uri = null, rawLines = null }) {
  mkdirSync(dir, { recursive: true });
  if (!noRunJson) {
    const resolvedUri =
      uri ??
      `https://classes.example.invalid/soc/api/openSections.json?year=${year}&term=${term}&campus=${campus}`;
    writeFileSync(
      join(dir, "run.json"),
      JSON.stringify({ schemaVersion: 1, uri: resolvedUri, intervalSeconds }, null, 2) + "\n",
      "utf8",
    );
  }
  const lines = rawLines ?? samples.map((s) => JSON.stringify(s));
  writeFileSync(join(dir, "samples.ndjson"), lines.join("\n") + "\n", "utf8");
  return dir;
}

function cyc(value, k) {
  return Array.isArray(value) ? value[k % value.length] : value;
}

// Hand-crafted change series with exactly one change bracket per tick.
// Tick T_k = baseMs + k*periodMs + phaseMs (or ticks[k] when an explicit tick
// array is given). Emits per k a stable sample at T_k - pre (body
// {bodyPrefix}{k}) and a changed sample at T_k + post (body {bodyPrefix}{k+1});
// serverDate = floor(sampleTime / 1000) s (1 s truncation) + serverOffsetMs,
// unless noServerDate.
//
// changedEndMs(k), when given, overrides the CHANGED row's absolute
// requestEndedUtc while leaving requestStartedUtc, decodedBodySha256 and
// serverDate untouched — the single field the A4-2 counterexamples edit.
// serverOffsetMs skews the recorded server clock away from the client clock,
// which is the mirror edit. Both default to the identity, so every existing
// caller emits byte-identical rows.
export function makeTickSeries({
  baseMs,
  periodMs,
  count,
  preMs = 400,
  postMs = 8600,
  phaseMs = 0,
  startSeq = 1,
  noServerDate = false,
  elapsedMs = 200,
  bodyPrefix = "v",
  ticks = null,
  changedEndMs = null,
  serverOffsetMs = 0,
}) {
  const rows = [];
  let seq = startSeq;
  const n = ticks !== null ? ticks.length : count;
  for (let k = 0; k < n; k += 1) {
    const tick = ticks !== null ? ticks[k] : baseMs + k * periodMs + phaseMs;
    const pair = [
      [-cyc(preMs, k), `${bodyPrefix}${k}`],
      [cyc(postMs, k), `${bodyPrefix}${k + 1}`],
    ];
    for (let role = 0; role < pair.length; role += 1) {
      const [offset, body] = pair[role];
      const t = tick + offset;
      const endMs = role === 1 && changedEndMs !== null ? changedEndMs(k) : t + elapsedMs;
      rows.push(
        sampleRow({
          seq,
          startMs: t,
          elapsedMs: endMs - t,
          bodySha: body,
          serverDateMs: noServerDate ? null : Math.floor(t / 1000) * 1000 + serverOffsetMs,
        }),
      );
      seq += 1;
    }
  }
  return rows;
}

// Re-stamps sequence/sampleId so a derived export looks like a fresh capture.
// EVERY other field — body hashes, both clock columns, elapsed, headers — stays
// the byte-identical reused observation record. This is what makes the A2-1
// fixtures reuse rather than fabricate.
export function renumberRows(rows) {
  return rows.map((row, i) => ({
    ...row,
    sequence: i + 1,
    sampleId: `fixture_${String(i + 1).padStart(6, "0")}`,
  }));
}

// A copy of `rows` with ONLY requestEndedUtc (and the elapsedMilliseconds that
// mirrors it) rewritten; requestStartedUtc, decodedBodySha256 and serverDate
// keep their exact recorded values.
export function rewriteRequestEnds(rows, endForRow) {
  return rows.map((row, i) => {
    const startMs = Date.parse(row.requestStartedUtc);
    const endMs = endForRow(i, startMs, row);
    return {
      ...row,
      requestEndedUtc: new Date(endMs).toISOString(),
      elapsedMilliseconds: endMs - startMs,
    };
  });
}

// A copy of `rows` whose CLIENT clock columns are translated by a single
// constant deltaMs: requestStartedUtc, requestEndedUtc and requestStartedLocal
// move together, so elapsedMilliseconds is unchanged. decodedBodySha256,
// rawBodySha256, canonicalSetSha256, serverDate, sequence and every other
// recorded field keep their exact captured values. This is a pure client-clock
// translation of a genuine capture — the A2-1 edit — and nothing else.
export function shiftClientClock(rows, deltaMs) {
  return rows.map((row) => {
    const shifted = { ...row };
    for (const key of ["requestStartedUtc", "requestEndedUtc", "requestStartedLocal"]) {
      if (typeof row[key] !== "string") continue;
      shifted[key] = new Date(Date.parse(row[key]) + deltaMs).toISOString();
    }
    return shifted;
  });
}

// A copy of `rows` whose recorded server `Date` header is translated by a
// single constant deltaMs. Every client column, every body hash and the row
// order keep their captured values. Composed with shiftClientClock this models
// a GENUINE capture pause: both clocks really moved, together.
export function shiftServerDate(rows, deltaMs) {
  return rows.map((row) => {
    if (typeof row.serverDate !== "string") return { ...row };
    return { ...row, serverDate: new Date(Date.parse(row.serverDate) + deltaMs).toUTCString() };
  });
}

// LCG-seeded generic change process (spec Section 17): body version counts
// change instants (tick + jitter) at or before the server generation time.
export function makeChangeProcess({
  periodMs,
  phaseMs,
  jitterMs = 0,
  sampleIntervalMs,
  startMs,
  count,
  seed = 1,
  elapsedMs = 200,
}) {
  let state = seed >>> 0;
  const lcg = () => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state / 2 ** 32;
  };
  const spanMs = count * sampleIntervalMs + 2 * periodMs;
  const firstTick = Math.floor((startMs - periodMs - phaseMs) / periodMs) * periodMs + phaseMs;
  const changeInstants = [];
  for (let t = firstTick; t <= startMs + spanMs; t += periodMs) {
    changeInstants.push(t + Math.floor(lcg() * (jitterMs + 1)));
  }
  const rows = [];
  for (let k = 0; k < count; k += 1) {
    const t = startMs + k * sampleIntervalMs;
    const genMs = t + Math.floor(elapsedMs / 2);
    const version = changeInstants.filter((c) => c <= genMs).length;
    rows.push(
      sampleRow({
        seq: k + 1,
        startMs: t,
        elapsedMs,
        bodySha: `v${version}`,
        serverDateMs: Math.floor(genMs / 1000) * 1000,
      }),
    );
  }
  return rows;
}

// Creates a schema-compatible open_batch_observations SQLite db.
export function makeSqliteDb(dbPath, { targets }) {
  mkdirSync(dirname(dbPath), { recursive: true });
  const db = new DatabaseSync(dbPath);
  db.exec(`
    CREATE TABLE open_batch_observations (
      attempt_id INTEGER PRIMARY KEY,
      observation_id TEXT NOT NULL,
      target_id TEXT NOT NULL,
      observation_sequence INTEGER NOT NULL,
      catalog_content_version TEXT,
      observed_at TEXT NOT NULL,
      classification TEXT NOT NULL,
      http_status INTEGER NOT NULL,
      decoded_body_sha256 TEXT NOT NULL,
      response_date TEXT,
      age_seconds INTEGER,
      canonical_set_sha256 TEXT,
      state_sha256 TEXT,
      changed_section_count INTEGER,
      body_changed INTEGER NOT NULL,
      state_changed INTEGER NOT NULL,
      UNIQUE (target_id, observation_sequence)
    );
  `);
  const insert = db.prepare(`
    INSERT INTO open_batch_observations
      (observation_id, target_id, observation_sequence, catalog_content_version,
       observed_at, classification, http_status, decoded_body_sha256, response_date,
       age_seconds, canonical_set_sha256, state_sha256, changed_section_count, body_changed, state_changed)
    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
  `);
  for (const target of targets) {
    for (const obs of target.observations) {
      insert.run(
        `obs-${target.targetId}-${obs.seq}`,
        target.targetId,
        obs.seq,
        "ccv-1",
        new Date(obs.observedAtMs).toISOString(),
        obs.classification ?? "VALID_APPLIED",
        obs.httpStatus ?? 200,
        obs.bodySha,
        obs.responseDateMs === null || obs.responseDateMs === undefined
          ? null
          : toRfc1123(obs.responseDateMs),
        obs.ageSeconds ?? null,
        `canon-${obs.bodySha}`,
        `state-${obs.bodySha}`,
        0,
        obs.bodyChanged,
        0,
      );
    }
  }
  db.close();
  return dbPath;
}

// Spawns the analyzer offline with absolute paths; parses outputs when present.
export function runAnalyzer(args) {
  const res = spawnSync(process.execPath, [ANALYZER_PATH, ...args], { encoding: "utf8" });
  const out = { code: res.status, stdout: res.stdout ?? "", stderr: res.stderr ?? "", json: null, md: null };
  const jsonIdx = args.indexOf("--out-json");
  const mdIdx = args.indexOf("--out-md");
  if (jsonIdx !== -1 && existsSync(args[jsonIdx + 1])) {
    out.jsonText = readFileSync(args[jsonIdx + 1], "utf8");
    out.json = JSON.parse(out.jsonText);
  }
  if (mdIdx !== -1 && existsSync(args[mdIdx + 1])) {
    out.md = readFileSync(args[mdIdx + 1], "utf8");
  }
  return out;
}

// Minimal fabricated bracket for unit-level phase/holdout/safe-offset tests.
export function fakeBracket({
  id,
  lowerMs,
  upperMs,
  targetId = "soc:2026:9:NB",
  windowId = "fixture#w00",
}) {
  return {
    bracketId: id,
    windowId,
    targetId,
    stableSeq: 0,
    changedSeq: 0,
    clientLowerMs: lowerMs,
    clientUpperMs: upperMs,
    serverLowerMs: lowerMs,
    serverUpperMs: upperMs,
    clientWidthMs: upperMs - lowerMs,
    serverWidthMs: upperMs - lowerMs,
  };
}
