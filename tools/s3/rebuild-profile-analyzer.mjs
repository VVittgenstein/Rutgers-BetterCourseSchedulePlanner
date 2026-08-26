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
import {
  compareModels,
  assessSafeOffset,
  assessServerClockEvidence,
  evaluateGate,
  commonInformativeSet,
} from "./lib/gate.mjs";
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
    // Stream identity matches provenance.mjs: `${inputId}::${targetId}`.
    const streamId = `${stream.inputId}::${targetId}`;
    const windows = segmentWindows(stream.samples, stream.intervalSeconds, stream.inputId);
    for (const win of windows) {
      const { brackets, counters: winCounters } = buildBrackets(win, stream.sourceKind);
      win.streamId = streamId;
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

  // 3c. Two provenance-based exclusions bar streams from ALL evidence below —
  // model fits, the clock-source selection, the comparison, server-clock
  // evidence, the safe offset, and every A4 gate:
  //   (1) every stream of a CONFLICTED class — conflicting campus labels (the
  //       copy-and-relabel attack) or conflicting absolute time anchors (the
  //       copy-and-translate attack: the same canonical series claimed at two
  //       different times, e.g. an off-peak capture shifted into the peak
  //       hour). Contested observations cannot lend brackets, windows, or
  //       wins to anything; picking a representative among disagreeing
  //       timelines would let the attacker choose WHICH absolute timeline
  //       counts, so nobody counts;
  //   (2) every NON-representative member of a clean class (identical or
  //       contained observation series agreeing on campus and time):
  //       duplicated observation data counts exactly ONCE, through the class
  //       representative. A capture copied under a different targetId (term
  //       relabel) or re-fed through the SQLite path therefore cannot inflate
  //       the comparison n, multiply target-LOO folds, or widen campus
  //       coverage — evidence and stability are effectively counted per
  //       provenance class.
  // Excluded streams stay listed in the descriptive tables, flagged via
  // provenance.excludedStreamIds / provenance.duplicateStreamIds and
  // bracketTotals.excludedFromEvidence.
  const excludedStreamIds = provenance.classes
    .filter((cls) => cls.campusConflict || cls.timeConflict)
    .flatMap((cls) => cls.members.map((m) => m.streamId))
    .sort();
  const duplicateStreamIds = provenance.classes
    .filter((cls) => !cls.campusConflict && !cls.timeConflict)
    .flatMap((cls) =>
      cls.members.filter((m) => m.relation !== "representative").map((m) => m.streamId),
    )
    .sort();
  provenance.excludedStreamIds = excludedStreamIds;
  provenance.duplicateStreamIds = duplicateStreamIds;
  const excludedStreams = new Set([...excludedStreamIds, ...duplicateStreamIds]);
  for (const win of windowsAll) {
    win.excluded = excludedStreams.has(win.streamId);
  }
  const evidenceBrackets = windowsAll
    .filter((win) => !win.excluded)
    .flatMap((win) => win.brackets);

  // 4. Clock analysis over all included samples (per-stream adjacency). This
  // is descriptive diagnostics; gate sufficiency (A4-5) is decided by
  // assessServerClockEvidence over the evidence brackets only.
  const clock = analyzeClock(streams.map((s) => s.samples));

  // 5. Fit the four model x clock combinations on the evidence brackets.
  const fits = {};
  for (const periodMs of [30000, 60000]) {
    const key = periodMs === 30000 ? "m30" : "m60";
    fits[key] = {
      server: clock.status === "unknown" ? null : fitPhase(evidenceBrackets, periodMs, "server"),
      client: fitPhase(evidenceBrackets, periodMs, "client"),
    };
  }

  // 6. Clock-source selection, comparison + holdout.
  const serverCommonCount =
    clock.status === "server-date-available"
      ? commonInformativeSet(evidenceBrackets, "server").length
      : 0;
  const useServer = serverCommonCount >= MIN_COMPARISON_BRACKETS;
  const clockSource = useServer ? "server" : "client";
  const clockFallback = !useServer;
  const comparison = compareModels(evidenceBrackets, clockSource);

  // 6b. Server-clock evidence sufficiency (production gates fail closed on a
  // client-clock fallback or on qualifying groups without server brackets).
  const serverEvidence = assessServerClockEvidence({
    brackets: evidenceBrackets,
    clock,
    clockFallback,
  });

  // 7. Safe offset + A4 gate.
  const safeOffset = assessSafeOffset(comparison, clock.status, serverEvidence);
  const targets = [...targetsMap.values()];
  const { goGate, decision } = evaluateGate({
    windowsAll,
    brackets: evidenceBrackets,
    comparison,
    safeOffset,
    clock,
    clockSource,
    provenance,
    serverEvidence,
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
  // with the comparison about which brackets counted. Totals describe every
  // built bracket; excludedFromEvidence says how many of them were barred
  // from all evidence by conflicted (campus or time-anchor) or duplicate
  // provenance.
  const bracketTotals = computeBracketTotals(
    allBrackets,
    counters,
    clockSource,
    allBrackets.length - evidenceBrackets.length,
  );
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
    serverEvidence,
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
