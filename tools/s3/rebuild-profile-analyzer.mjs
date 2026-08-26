#!/usr/bin/env node
// S3 offline rebuild-cadence analyzer (evidence-only lane).
//
// Pure offline: no network access, no npm dependencies, node: builtins only.
// Every number in the outputs is computed from the inputs; the verdict comes
// exclusively from the A4 gate evaluation.

import { writeFileSync, mkdirSync } from "node:fs";
import { basename, dirname } from "node:path";
import process from "node:process";
import { AnalyzerError, EXIT_OK } from "./lib/errors.mjs";
import { parseArgs, buildNormalizedCommand, USAGE } from "./lib/cli.mjs";
import { sha256File } from "./lib/stable.mjs";
import { ingestNdjson } from "./lib/ingest-ndjson.mjs";
import { ingestSqlite } from "./lib/ingest-sqlite.mjs";
import { buildProvenance } from "./lib/provenance.mjs";
import { segmentWindows, nyLabel, overlapsNyPeak } from "./lib/windows.mjs";
import { buildBrackets } from "./lib/brackets.mjs";
import { analyzeClock } from "./lib/clock.mjs";
import {
  MIN_COMPARISON_BRACKETS,
  fitPhase,
} from "./lib/phase.mjs";
import { compareModels, assessSafeOffset, evaluateGate, commonInformativeSet } from "./lib/gate.mjs";
import { buildJsonReport, computeBracketTotals } from "./lib/report-json.mjs";
import { buildMdReport } from "./lib/report-md.mjs";

function main() {
  const args = parseArgs(process.argv.slice(2));
  if (args.help) {
    process.stdout.write(USAGE);
    return EXIT_OK;
  }

  // 1. Ingest all inputs, recording pre-analysis fingerprints.
  const ingests = [];
  for (const p of args.ndjsonPaths) {
    ingests.push({ sourceKind: "ndjson", ...ingestNdjson(p) });
  }
  for (const p of args.sqlitePaths) {
    ingests.push({ sourceKind: "sqlite", ...ingestSqlite(p, args.sqliteTargets) });
  }

  // 2. Build per-(inputId, targetId) sample streams.
  const streams = [];
  for (const ing of ingests) {
    if (ing.sourceKind === "ndjson") {
      streams.push({
        inputId: ing.input.id,
        sourceKind: "ndjson",
        intervalSeconds: ing.intervalSeconds,
        samples: ing.samples,
      });
    } else {
      for (const [targetId, samples] of [...ing.samplesByTarget.entries()].sort((a, b) =>
        a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0,
      )) {
        streams.push({
          inputId: ing.input.id,
          sourceKind: "sqlite",
          intervalSeconds: null,
          samples,
          targetId,
        });
      }
    }
  }

  // 3. Window segmentation + bracket construction.
  const targetsMap = new Map();
  const windowsAll = [];
  const allBrackets = [];
  const counters = {
    changeCount: 0,
    serverNonPositiveWidth: 0,
    clientNonPositiveWidth: 0,
    noPriorStable: 0,
    ageGreaterThanZeroEndpoints: 0,
  };

  for (const stream of streams) {
    if (stream.samples.length === 0) continue;
    const first = stream.samples[0];
    const targetId = first.targetId;
    if (!targetsMap.has(targetId)) {
      targetsMap.set(targetId, {
        targetId,
        term: first.term,
        year: first.year,
        campus: first.campus,
        windows: [],
      });
    }
    const target = targetsMap.get(targetId);
    const windows = segmentWindows(stream.samples, stream.intervalSeconds, stream.inputId);
    for (const win of windows) {
      const { brackets, counters: winCounters } = buildBrackets(win, stream.sourceKind);
      win.nyLabel = nyLabel(win.utcStartMs, win.utcEndMs);
      win.peakOverlap = overlapsNyPeak(win.utcStartMs, win.utcEndMs);
      win.brackets = brackets;
      target.windows.push(win);
      windowsAll.push(win);
      allBrackets.push(...brackets);
      counters.changeCount += winCounters.changeCount;
      counters.serverNonPositiveWidth += winCounters.serverNonPositiveWidth;
      counters.clientNonPositiveWidth += winCounters.clientNonPositiveWidth;
      counters.noPriorStable += winCounters.noPriorStable;
      counters.ageGreaterThanZeroEndpoints += winCounters.ageGreaterThanZeroEndpoints;
    }
  }

  // 3b. Observation-data provenance (metadata-free series fingerprints; the
  // A4-1 gate counts campus coverage per provenance class, not per label).
  const provenance = buildProvenance(streams);

  // 4. Clock analysis over all included samples (per-stream adjacency).
  const clock = analyzeClock(streams.map((s) => s.samples));

  // 5. Fit the four model x clock combinations on all brackets.
  const fits = {};
  for (const periodMs of [30000, 60000]) {
    const key = periodMs === 30000 ? "m30" : "m60";
    fits[key] = {
      server: clock.status === "unknown" ? null : fitPhase(allBrackets, periodMs, "server"),
      client: fitPhase(allBrackets, periodMs, "client"),
    };
  }

  // 6. Clock-source selection, comparison + holdout.
  const serverCommonCount =
    clock.status === "server-date-available"
      ? commonInformativeSet(allBrackets, "server").length
      : 0;
  const useServer = serverCommonCount >= MIN_COMPARISON_BRACKETS;
  const clockSource = useServer ? "server" : "client";
  const clockFallback = !useServer;
  const comparison = compareModels(allBrackets, clockSource);

  // 7. Safe offset + A4 gate.
  const safeOffset = assessSafeOffset(comparison, clock.status);
  const targets = [...targetsMap.values()];
  const { goGate, decision } = evaluateGate({
    targets,
    windowsAll,
    brackets: allBrackets,
    comparison,
    safeOffset,
    clock,
    clockSource,
    provenance,
  });

  // 8. Re-verify all input fingerprints (read-only guarantee) before writing.
  for (const ing of ingests) {
    for (let i = 0; i < ing.files.length; i += 1) {
      const postSha = sha256File(ing.files[i]);
      if (postSha !== ing.fileShas[i]) {
        throw new AnalyzerError(
          "E_INPUT_MUTATED",
          `input ${ing.input.id}: file changed during analysis`,
        );
      }
    }
  }

  // 9. Assemble and write outputs (JSON first, then MD).
  const normalizedCommand = buildNormalizedCommand({
    inputs: ingests.map((i) => i.input),
    sqliteTargets: args.sqliteTargets,
    outJsonBase: basename(args.outJson),
    outMdBase: basename(args.outMd),
  });
  // Totals and per-bracket informative flags are computed on the SAME clock
  // the model comparison selected, so the report tables can never disagree
  // with the comparison about which brackets counted.
  const bracketTotals = computeBracketTotals(allBrackets, counters, clockSource);
  const reportCtx = {
    normalizedCommand,
    inputs: ingests.map((i) => i.input),
    targets,
    provenance,
    windowsAll,
    brackets: allBrackets,
    bracketTotals,
    fits,
    comparison,
    clockSource,
    clockFallback,
    clock,
    safeOffset,
    goGate,
    decision,
  };
  const jsonText = buildJsonReport(reportCtx);
  const mdText = buildMdReport(reportCtx);
  mkdirSync(dirname(args.outJson), { recursive: true });
  writeFileSync(args.outJson, jsonText, "utf8");
  mkdirSync(dirname(args.outMd), { recursive: true });
  writeFileSync(args.outMd, mdText, "utf8");

  process.stdout.write(
    `verdict=${decision.verdict} qualifier=${decision.qualifier ?? "none"} brackets=${bracketTotals.total} distinguishable=${comparison.distinguishable}\n`,
  );
  return EXIT_OK;
}

try {
  process.exitCode = main();
} catch (err) {
  if (err instanceof AnalyzerError) {
    process.stderr.write(`ERROR ${err.code}: ${err.message}\n`);
    if (err.code === "E_USAGE") process.stderr.write(USAGE);
    process.exitCode = err.exitCode;
  } else {
    process.stderr.write(`ERROR E_INTERNAL: ${err && err.stack ? err.stack : String(err)}\n`);
    process.exitCode = 2;
  }
}
