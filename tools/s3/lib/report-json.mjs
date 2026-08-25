// Machine JSON assembly. Keys are inserted in the exact schema order; the
// stable serializer emits insertion order, so this module fully determines the
// byte layout. All arrays are sorted by stable keys so input order cannot leak.

import { isoUtcMs, stableStringify } from "./stable.mjs";
import { TOOL_VERSION, SCHEMA_VERSION, isInformative, bracketWidthMs } from "./phase.mjs";

function cmpStr(a, b) {
  return a < b ? -1 : a > b ? 1 : 0;
}

function roundHalfUpSeconds(ms) {
  return Math.floor(ms / 1000 + 0.5);
}

function fitBlock(fit) {
  return {
    informativeCount: fit.informativeCount,
    nonInformativeCount: fit.nonInformativeCount,
    unusableCount: fit.unusableCount,
    maxCoverage: fit.maxCoverage,
    coverageRatio: fit.coverageRatio,
    bestPhaseIntervals: fit.bestPhaseIntervals.map((iv) => ({ startMs: iv.startMs, endMs: iv.endMs })),
    residualBracketIds: fit.residualBracketIds,
  };
}

export function buildJsonReport(ctx) {
  const {
    normalizedCommand,
    inputs,
    targets,
    bracketTotals,
    fits,
    comparison,
    clockSource,
    clockFallback,
    clock,
    safeOffset,
    goGate,
    decision,
  } = ctx;

  // Per-bracket informative flags use the SAME clock the comparison selected
  // (including a client fallback), so a bracket can never show as
  // non-informative in the table while being counted by the comparison.
  const bracketClock = clockSource;

  const inputsJson = [...inputs]
    .sort((a, b) => cmpStr(a.kind, b.kind) || cmpStr(a.id, b.id))
    .map((inp) => {
      const entry = { kind: inp.kind, id: inp.id, sha256: inp.sha256 };
      if (inp.kind === "sqlite") entry.openMode = inp.openMode;
      entry.rows = inp.rows;
      entry.excluded = {
        curlExitCode: inp.excluded.curlExitCode,
        httpStatusNon2xx: inp.excluded.httpStatusNon2xx,
        validationErrors: inp.excluded.validationErrors,
        initialLkg: inp.excluded.initialLkg,
      };
      return entry;
    });

  const targetsJson = [...targets]
    .sort((a, b) => cmpStr(a.targetId, b.targetId))
    .map((target) => ({
      targetId: target.targetId,
      term: target.term,
      year: target.year,
      campus: target.campus,
      windows: [...target.windows]
        .sort((a, b) => cmpStr(a.windowId, b.windowId))
        .map((win) => ({
          windowId: win.windowId,
          utcStart: isoUtcMs(win.utcStartMs),
          utcEnd: isoUtcMs(win.utcEndMs),
          nyLabel: win.nyLabel,
          peakOverlap: win.peakOverlap,
          sampleCount: win.samples.length,
          bracketCount: win.brackets.length,
          brackets: [...win.brackets]
            .sort((a, b) => cmpStr(a.targetId, b.targetId) || cmpStr(a.windowId, b.windowId) || a.changedSeq - b.changedSeq)
            .map((bracket) => ({
              bracketId: bracket.bracketId,
              lowerUtc: isoUtcMs(bracket.clientLowerMs),
              upperUtc: isoUtcMs(bracket.clientUpperMs),
              widthSeconds: roundHalfUpSeconds(bracket.clientWidthMs),
              serverWidthSeconds:
                bracket.serverWidthMs !== null ? roundHalfUpSeconds(bracket.serverWidthMs) : null,
              informative30: isInformative(bracket, 30000, bracketClock),
              informative60: isInformative(bracket, 60000, bracketClock),
            })),
        })),
    }));

  const models = {};
  for (const periodMs of [30000, 60000]) {
    const key = periodMs === 30000 ? "m30" : "m60";
    models[key] = {
      periodSeconds: periodMs / 1000,
      server: clock.status === "unknown" ? null : fitBlock(fits[key].server),
      client: fitBlock(fits[key].client),
    };
  }

  const comparisonJson = {
    clockSource: comparison.clockSource,
    clockFallback,
    commonInformativeCount: comparison.commonInformativeCount,
    maxCoverage30Common: comparison.maxCoverage30Common,
    maxCoverage60Common: comparison.maxCoverage60Common,
    provisionalWinner: comparison.provisionalWinner,
    distinguishable: comparison.distinguishable,
    winner: comparison.winner,
    reason: comparison.reason,
    holdout: {
      mode: comparison.holdout.mode,
      degenerate: comparison.holdout.degenerate,
      groupCount: comparison.holdout.groupCount,
      folds: comparison.holdout.folds.map((f) => ({
        groupId: f.groupId,
        trainCount: f.trainCount,
        testCount: f.testCount,
        test30: f.test30,
        test60: f.test60,
      })),
      consistentM30Win: comparison.holdout.consistentM30Win,
    },
  };

  const clockJson = {
    status: clock.status,
    offsetDistribution:
      clock.offsetDistribution === null
        ? null
        : {
            sampleCount: clock.offsetDistribution.sampleCount,
            minMs: clock.offsetDistribution.minMs,
            p50Ms: clock.offsetDistribution.p50Ms,
            p95Ms: clock.offsetDistribution.p95Ms,
            maxMs: clock.offsetDistribution.maxMs,
          },
    serverDateRegressions: clock.serverDateRegressions,
    serverDateMissingCount: clock.serverDateMissingCount,
  };

  const safeOffsetJson = { identifiable: safeOffset.identifiable };
  if (safeOffset.identifiable) {
    safeOffsetJson.bound = {
      phaseIntervalMs: {
        startMs: safeOffset.bound.phaseIntervalMs.startMs,
        endMs: safeOffset.bound.phaseIntervalMs.endMs,
      },
      maxPositiveJitterMs: safeOffset.bound.maxPositiveJitterMs,
    };
  } else {
    safeOffsetJson.reason = safeOffset.reason;
  }

  const decisionJson = { verdict: decision.verdict };
  if (decision.qualifier) decisionJson.qualifier = decision.qualifier;
  decisionJson.reasons = decision.reasons;

  const root = {
    schemaVersion: SCHEMA_VERSION,
    toolVersion: TOOL_VERSION,
    normalizedCommand,
    inputs: inputsJson,
    targets: targetsJson,
    bracketTotals: {
      total: bracketTotals.total,
      informative30: bracketTotals.informative30,
      informative60: bracketTotals.informative60,
      nonInformative30: bracketTotals.nonInformative30,
      nonInformative60: bracketTotals.nonInformative60,
      serverNonPositiveWidth: bracketTotals.serverNonPositiveWidth,
      clientNonPositiveWidth: bracketTotals.clientNonPositiveWidth,
      noPriorStable: bracketTotals.noPriorStable,
      ageGreaterThanZeroEndpoints: bracketTotals.ageGreaterThanZeroEndpoints,
    },
    models,
    comparison: comparisonJson,
    clock: clockJson,
    safeOffset: safeOffsetJson,
    goGate: goGate.map((g) => ({
      id: g.id,
      requirement: g.requirement,
      satisfied: g.satisfied,
      evidence: g.evidence,
    })),
    decision: decisionJson,
  };

  return stableStringify(root) + "\n";
}

// clockSource is the clock the model comparison selected ("server"/"client"),
// so the headline totals always agree with the comparison's bracket handling.
export function computeBracketTotals(brackets, counters, clockSource) {
  let informative30 = 0;
  let informative60 = 0;
  let nonInformative30 = 0;
  let nonInformative60 = 0;
  for (const bracket of brackets) {
    const width = bracketWidthMs(bracket, clockSource);
    // null: no bounds on this clock. <= 0: corrupt bracket (fail-closed);
    // buildBrackets already rejects these, but never count one as informative.
    if (width === null || width <= 0) continue;
    if (width < 30000) informative30 += 1;
    else nonInformative30 += 1;
    if (width < 60000) informative60 += 1;
    else nonInformative60 += 1;
  }
  return {
    total: brackets.length,
    informative30,
    informative60,
    nonInformative30,
    nonInformative60,
    serverNonPositiveWidth: counters.serverNonPositiveWidth,
    clientNonPositiveWidth: counters.clientNonPositiveWidth,
    noPriorStable: counters.noPriorStable,
    ageGreaterThanZeroEndpoints: counters.ageGreaterThanZeroEndpoints,
  };
}
