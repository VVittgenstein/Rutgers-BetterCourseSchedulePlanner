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
    provenance,
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
    stability:
      comparison.stability === null
        ? null
        : {
            targets: {
              mode: comparison.stability.targets.mode,
              degenerate: comparison.stability.targets.degenerate,
              count: comparison.stability.targets.count,
              folds: comparison.stability.targets.folds.map((f) => ({
                heldOut: f.heldOut,
                distinguishable: f.distinguishable,
                winner: f.winner,
                reason: f.reason,
              })),
              pass: comparison.stability.targets.pass,
            },
            groups: {
              mode: comparison.stability.groups.mode,
              degenerate: comparison.stability.groups.degenerate,
              count: comparison.stability.groups.count,
              folds: comparison.stability.groups.folds.map((f) => ({
                heldOut: f.heldOut,
                distinguishable: f.distinguishable,
                winner: f.winner,
                reason: f.reason,
              })),
              pass: comparison.stability.groups.pass,
            },
            outliers: {
              mode: comparison.stability.outliers.mode,
              residualCount: comparison.stability.outliers.residualCount,
              runs: comparison.stability.outliers.runs.map((r) => ({
                k: r.k,
                removedBracketIds: r.removedBracketIds,
                distinguishable: r.distinguishable,
                winner: r.winner,
                reason: r.reason,
              })),
              note: comparison.stability.outliers.note,
              pass: comparison.stability.outliers.pass,
            },
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
    serverEvidence: {
      sufficient: serverEvidence.sufficient,
      reason: serverEvidence.reason,
      serverCommonCount: serverEvidence.serverCommonCount,
      groupsTotal: serverEvidence.groupsTotal,
      groupsWithServer: serverEvidence.groupsWithServer,
    },
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
    provenance: {
      streams: provenance.streams.map((s) => ({
        streamId: s.streamId,
        targetId: s.targetId,
        campus: s.campus,
        sampleCount: s.sampleCount,
        // false when the stream records no client request end at all (every
        // SQLite stream): the request-end record comparison is suppressed for
        // any pair that includes such a stream.
        clientEndObserved: s.clientEndObserved,
        seriesFingerprint: s.seriesFingerprint,
      })),
      classes: provenance.classes.map((c) => ({
        classId: c.classId,
        campus: c.campus,
        campusConflict: c.campusConflict,
        // Members share captured observation data but disagree about its
        // absolute time, its serverDates, or its request ends (a translated or
        // field-edited copy): the whole family is barred from evidence, like a
        // campus conflict.
        timeConflict: c.timeConflict,
        // Every disagreeing pair in the family, not just the ones on the audit
        // spanning tree; [] when the family is clean.
        timeConflictPairs: c.timeConflictPairs.map((p) => ({
          streamIdA: p.streamIdA,
          streamIdB: p.streamIdB,
          relation: p.relation,
          // Constant client-clock offset at which the two members' shared
          // records line up: 0 means they disagree on some OTHER recorded
          // column (a serverDate, a request end), non-zero means the copy's
          // whole client clock was translated by that many milliseconds.
          offsetMs: p.offsetMs,
        })),
        // relation: how the member joined the family — "representative",
        // "identical", "contained", "overlapping" (a shared contiguous block of
        // canonical entries) or "derived" (reused observation records).
        // relatedTo/matchedCount describe that one attachment edge and are null
        // on the representative.
        members: c.members.map((m) => ({
          streamId: m.streamId,
          relation: m.relation,
          relatedTo: m.relatedTo,
          matchedCount: m.matchedCount,
        })),
      })),
      // Streams barred from all evidence (members of campus-conflicted or
      // time-anchor-conflicted classes); [] when every stream is
      // evidence-eligible.
      excludedStreamIds: provenance.excludedStreamIds,
      // Non-representative members of clean classes (identical/contained
      // observation series) — also barred from all evidence so duplicated
      // data counts once; [] when every clean class has a single member.
      duplicateStreamIds: provenance.duplicateStreamIds,
    },
    bracketTotals: {
      total: bracketTotals.total,
      excludedFromEvidence: bracketTotals.excludedFromEvidence,
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
// excludedFromEvidence: brackets built from conflicted provenance streams
// (campus or time-anchor conflicts) or from duplicate (non-representative)
// members of clean classes — listed in the tables but barred from every
// evidence computation.
export function computeBracketTotals(brackets, counters, clockSource, excludedFromEvidence = 0) {
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
    excludedFromEvidence,
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
