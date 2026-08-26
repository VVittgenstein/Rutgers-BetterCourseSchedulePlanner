// 30 s vs 60 s model comparison, unified safe-offset assessment, and the A4
// GO gate. The final verdict is always computed from the gate entries; there
// is no shortcut path.

import { internalAssert } from "./errors.mjs";
import {
  MIN_COMPARISON_BRACKETS,
  REQUIRED_CAMPUSES,
  MIN_GROUP_BRACKETS,
  STABILITY_OUTLIER_KS,
  isInformative,
  fitPhase,
  bracketArc,
  arcContains,
  bracketBounds,
} from "./phase.mjs";
import { runHoldout, groupBrackets } from "./holdout.mjs";
import { overlapsNyPeakStrict } from "./windows.mjs";

export {
  TOOL_VERSION,
  SCHEMA_VERSION,
  PERIODS_MS,
  SERVER_DATE_WIDEN_MS,
  CLIENT_TIME_REGRESSION_TOLERANCE_MS,
  SERVER_DATE_REGRESSION_CAVEAT_MS,
  WINDOW_GAP_MIN_MS,
  WINDOW_GAP_INTERVAL_FACTOR,
  MIN_COMPARISON_BRACKETS,
  MIN_GROUP_BRACKETS,
  STABILITY_OUTLIER_KS,
  REQUIRED_CAMPUSES,
  NY_PEAK,
} from "./phase.mjs";

export function commonInformativeSet(brackets, clockSource) {
  // Informative for BOTH periods (width < 30 s implies width < 60 s).
  return brackets.filter((b) => isInformative(b, 30000, clockSource));
}

export function compareModels(brackets, clockSource) {
  const comparisonSet = commonInformativeSet(brackets, clockSource);
  const fit30 = fitPhase(comparisonSet, 30000, clockSource);
  const fit60 = fitPhase(comparisonSet, 60000, clockSource);
  const c30 = fit30.maxCoverage;
  const c60 = fit60.maxCoverage;
  // Honest invariant: every 60 s grid's ticks are contained in the 30 s grid
  // at the same phase mod 30, so coverage(30) >= coverage(60) identically.
  internalAssert(c30 >= c60, `coverage invariant violated: c30=${c30} < c60=${c60}`);

  const holdout = runHoldout(comparisonSet, clockSource);
  const provisionalWinner = c30 > c60 ? "m30" : null;

  let distinguishable = false;
  let winner = null;
  let reason;
  if (comparisonSet.length < MIN_COMPARISON_BRACKETS) {
    reason = "insufficient-informative-brackets";
  } else if (c30 === c60) {
    // Equality is consistent with a true 60 s period but proves neither model.
    reason = "equal-coverage-30s-adds-no-explanatory-power";
  } else if (holdout.degenerate) {
    reason = "holdout-degenerate";
  } else if (!holdout.consistentM30Win) {
    reason = "not-confirmed-by-holdout";
  } else {
    distinguishable = true;
    winner = "m30";
    reason = "m30-strict-win-confirmed-by-holdout";
  }

  return {
    clockSource,
    commonInformativeCount: comparisonSet.length,
    maxCoverage30Common: c30,
    maxCoverage60Common: c60,
    provisionalWinner,
    distinguishable,
    winner,
    reason,
    holdout,
    comparisonSet, // internal; stripped before serialization
  };
}

function intersectIntervalSets(setA, setB, periodMs) {
  // Closed integer intervals on [0, periodMs); each set is a sorted list of
  // non-wrapping runs (wrapped runs already split). Plain 1-D intersection.
  void periodMs;
  const out = [];
  for (const a of setA) {
    for (const b of setB) {
      const start = Math.max(a.startMs, b.startMs);
      const end = Math.min(a.endMs, b.endMs);
      if (start <= end) out.push({ startMs: start, endMs: end });
    }
  }
  out.sort((x, y) => x.startMs - y.startMs);
  return out;
}

// Is there enough SERVER-clock evidence to base production conclusions on?
// A stray serverDate on some unrelated sample is not evidence: the comparison
// itself must run on the server clock (no client fallback) and every
// qualifying (target, window) group must have its own server-clock brackets.
export function assessServerClockEvidence({ brackets, clock, clockFallback }) {
  if (clock.status !== "server-date-available") {
    return {
      sufficient: false,
      reason: "server-date-absent",
      serverCommonCount: 0,
      groupsTotal: 0,
      groupsWithServer: 0,
    };
  }
  const serverCommonCount = commonInformativeSet(brackets, "server").length;
  if (clockFallback === true) {
    return {
      sufficient: false,
      reason: "client-clock-fallback",
      serverCommonCount,
      groupsTotal: 0,
      groupsWithServer: 0,
    };
  }
  const clientGroups = groupBrackets(commonInformativeSet(brackets, "client"));
  const groupsTotal = clientGroups.length;
  if (groupsTotal === 0) {
    return {
      sufficient: false,
      reason: "no-qualifying-groups",
      serverCommonCount,
      groupsTotal: 0,
      groupsWithServer: 0,
    };
  }
  let groupsWithServer = 0;
  for (const group of clientGroups) {
    const serverInformative = group.brackets.filter((b) =>
      isInformative(b, 30000, "server"),
    ).length;
    if (serverInformative >= MIN_GROUP_BRACKETS) groupsWithServer += 1;
  }
  if (groupsWithServer < groupsTotal) {
    return {
      sufficient: false,
      reason: "groups-missing-server-evidence",
      serverCommonCount,
      groupsTotal,
      groupsWithServer,
    };
  }
  return { sufficient: true, reason: null, serverCommonCount, groupsTotal, groupsWithServer };
}

export function assessSafeOffset(comparison, clockStatus, serverEvidence) {
  if (clockStatus === "unknown") {
    return { identifiable: false, reason: "clock-unknown" };
  }
  if (serverEvidence.sufficient !== true) {
    return {
      identifiable: false,
      reason: `server-clock-evidence-insufficient:${serverEvidence.reason}`,
    };
  }
  if (!comparison.distinguishable) {
    return { identifiable: false, reason: "not-distinguishable" };
  }
  if (comparison.holdout.degenerate) {
    return { identifiable: false, reason: "holdout-degenerate" };
  }

  const periodMs = comparison.winner === "m30" ? 30000 : 60000;
  const clockSource = comparison.clockSource;
  const groups = groupBrackets(comparison.comparisonSet);

  // Per-group best-phase intervals for the winning period must share a
  // non-empty circular intersection.
  let consensus = null;
  for (const group of groups) {
    const fit = fitPhase(group.brackets, periodMs, clockSource);
    const intervals = fit.bestPhaseIntervals;
    consensus = consensus === null ? intervals : intersectIntervalSets(consensus, intervals, periodMs);
    if (consensus.length === 0) {
      return { identifiable: false, reason: "phase-intervals-disjoint-across-groups" };
    }
  }
  if (consensus === null || consensus.length === 0) {
    return { identifiable: false, reason: "phase-intervals-disjoint-across-groups" };
  }

  // Positive jitter bound: distance from the latest grid tick (< upper) at the
  // consensus phase to each bracket upper bound must stay < period/2.
  const phiMs = consensus[0].startMs;
  let maxPositiveJitterMs = 0;
  for (const bracket of comparison.comparisonSet) {
    const { upperMs } = bracketBounds(bracket, clockSource);
    let jitter = (((upperMs - phiMs) % periodMs) + periodMs) % periodMs;
    if (jitter === 0) jitter = periodMs;
    if (jitter > maxPositiveJitterMs) maxPositiveJitterMs = jitter;
  }
  if (maxPositiveJitterMs >= periodMs / 2) {
    return { identifiable: false, reason: "jitter-unbounded" };
  }

  return {
    identifiable: true,
    bound: {
      phaseIntervalMs: { startMs: consensus[0].startMs, endMs: consensus[0].endMs },
      maxPositiveJitterMs,
    },
  };
}

// Triple stability assessment behind A4-6 (only meaningful for a
// distinguishable comparison): the winner must survive (a) whole-TARGET
// leave-out, (b) (target, window) group leave-out, and (c) deterministic
// removal of the top-k most-residual brackets. Each check is a bounded rerun
// of compareModels on the reduced bracket set and is reported separately —
// a group is never presented as a target.
export function assessStability({ brackets, comparison, clockSource }) {
  const cmp = comparison;
  const keepsWinner = (rerun) => rerun.distinguishable === true && rerun.winner === cmp.winner;

  // (a) whole-target leave-out.
  const targetIds = [...new Set(cmp.comparisonSet.map((b) => b.targetId))].sort();
  let targets;
  if (targetIds.length < 2) {
    targets = { mode: "target-loo", degenerate: true, count: targetIds.length, folds: [], pass: false };
  } else {
    const folds = targetIds.map((targetId) => {
      const rerun = compareModels(
        brackets.filter((b) => b.targetId !== targetId),
        clockSource,
      );
      return {
        heldOut: targetId,
        distinguishable: rerun.distinguishable,
        winner: rerun.winner,
        reason: rerun.reason,
      };
    });
    targets = {
      mode: "target-loo",
      degenerate: false,
      count: targetIds.length,
      folds,
      pass: folds.every((f) => f.distinguishable === true && f.winner === cmp.winner),
    };
  }

  // (b) (target, window) group leave-out.
  const groupList = groupBrackets(cmp.comparisonSet);
  let groups;
  if (groupList.length < 2) {
    groups = { mode: "group-loo", degenerate: true, count: groupList.length, folds: [], pass: false };
  } else {
    const folds = groupList.map((group) => {
      const rerun = compareModels(
        brackets.filter((b) => `${b.targetId}/${b.windowId}` !== group.groupId),
        clockSource,
      );
      return {
        heldOut: group.groupId,
        distinguishable: rerun.distinguishable,
        winner: rerun.winner,
        reason: rerun.reason,
      };
    });
    groups = {
      mode: "group-loo",
      degenerate: false,
      count: groupList.length,
      folds,
      pass: folds.every((f) => f.distinguishable === true && f.winner === cmp.winner),
    };
  }

  // (c) deterministic outlier sensitivity: rank the residual brackets of the
  // winning fit by circular distance from the best phase (desc, bracketId asc
  // tiebreak) and rerun with the top-k removed.
  const periodMs = cmp.winner === "m30" ? 30000 : 60000;
  const fit = fitPhase(cmp.comparisonSet, periodMs, clockSource);
  const phiMs = fit.bestPhaseIntervals.length > 0 ? fit.bestPhaseIntervals[0].startMs : 0;
  const residualIds = new Set(fit.residualBracketIds);
  const cdist = (a, b) => {
    const d = Math.abs(a - b);
    return Math.min(d, periodMs - d);
  };
  const ranked = cmp.comparisonSet
    .filter((b) => residualIds.has(b.bracketId))
    .map((b) => {
      const arc = bracketArc(b, periodMs, clockSource);
      return {
        bracketId: b.bracketId,
        d: Math.min(cdist(phiMs, arc.startMs), cdist(phiMs, arc.endMs)),
      };
    })
    .sort((x, y) => y.d - x.d || (x.bracketId < y.bracketId ? -1 : x.bracketId > y.bracketId ? 1 : 0));
  const residualCount = ranked.length;
  const runs = [];
  let outliersPass = true;
  for (const k of STABILITY_OUTLIER_KS) {
    if (k > residualCount) continue;
    const removedBracketIds = ranked.slice(0, k).map((r) => r.bracketId);
    const removedSet = new Set(removedBracketIds);
    const rerun = compareModels(
      brackets.filter((b) => !removedSet.has(b.bracketId)),
      clockSource,
    );
    runs.push({
      k,
      removedBracketIds,
      distinguishable: rerun.distinguishable,
      winner: rerun.winner,
      reason: rerun.reason,
    });
    if (!keepsWinner(rerun)) outliersPass = false;
  }
  const outliers = {
    mode: "residual-topk",
    residualCount,
    runs,
    note: residualCount === 0 ? "no-residuals" : null,
    pass: outliersPass,
  };

  return { targets, groups, outliers };
}

// ctx: {
//   targets: [{ targetId, campus, windows: [{ windowId, peakOverlap, brackets }] }],
//   windowsAll: [{ windowId, peakOverlap }],
//   brackets, comparison, safeOffset, clock, clockSource,
//   provenance: buildProvenance() result, serverEvidence: assessServerClockEvidence() result
// }
export function evaluateGate(ctx) {
  const gate = [];

  // A4-1: multi-target evidence counted by independent observation-data
  // provenance, not by campus labels. A provenance class contributes at most
  // one campus; a class whose members carry conflicting campus labels (the
  // copy-and-relabel attack) contributes nothing.
  const targetQualifies = new Map();
  for (const target of ctx.targets) {
    targetQualifies.set(
      target.targetId,
      target.windows.some(
        (w) =>
          w.brackets.filter((b) => isInformative(b, 30000, ctx.clockSource)).length >=
          MIN_GROUP_BRACKETS,
      ),
    );
  }
  const targetByStreamId = new Map(ctx.provenance.streams.map((s) => [s.streamId, s.targetId]));
  const coveredBy = new Map(); // campus -> classId (first covering class, classId asc)
  let conflictClassCount = 0;
  for (const cls of ctx.provenance.classes) {
    if (cls.campusConflict) {
      conflictClassCount += 1;
      continue;
    }
    if (cls.campus === null || !REQUIRED_CAMPUSES.includes(cls.campus)) continue;
    const qualifies = cls.members.some(
      (m) => targetQualifies.get(targetByStreamId.get(m.streamId)) === true,
    );
    if (qualifies && !coveredBy.has(cls.campus)) coveredBy.set(cls.campus, cls.classId);
  }
  const missingCampuses = REQUIRED_CAMPUSES.filter((c) => !coveredBy.has(c));
  const coveredList = REQUIRED_CAMPUSES.filter((c) => coveredBy.has(c)).map(
    (c) => `${c}(${coveredBy.get(c)})`,
  );
  const conflictSuffix =
    conflictClassCount > 0
      ? `; ${conflictClassCount} class(es) with conflicting campus labels ignored`
      : "";
  const a1Satisfied = missingCampuses.length === 0;
  const a1Evidence = a1Satisfied
    ? `campuses: ${coveredList.join(", ")} from ${ctx.provenance.classes.length} provenance classes${conflictSuffix}`
    : `campuses: ${coveredList.length > 0 ? coveredList.join(", ") + " only" : "none"}${missingCampuses
        .map((c) => `; ${c} missing`)
        .join("")}${conflictSuffix}`;
  gate.push({
    id: "A4-1",
    requirement:
      "Multi-target evidence: at least NB, NK, CM independently evaluable from independent data provenance",
    satisfied: a1Satisfied,
    evidence: a1Evidence,
  });

  // A4-2: multiple independent time windows incl. NY peak and off-peak — each
  // side must have at least one QUALIFYING window (>= MIN_GROUP_BRACKETS
  // informative brackets of its own on the comparison clock). Peak evidence is
  // counted at the BRACKET level: a peak-qualifying window needs >=
  // MIN_GROUP_BRACKETS informative brackets whose own client-time interval
  // overlaps 17:00-18:00 ET with positive measure. A window that merely
  // touches the boundary instant, or that merged an isolated zero-change
  // peak-time sample into a rich pre-peak session, carries zero in-peak
  // brackets and cannot satisfy the peak side. (Client bounds are used for the
  // wall-clock peak test: they always exist, and the client-vs-server offset
  // is orders of magnitude below the one-hour peak span.)
  const informativeBrackets = (w) => w.brackets.filter((b) => isInformative(b, 30000, ctx.clockSource));
  const inPeakInformativeCount = (w) =>
    informativeBrackets(w).filter((b) => overlapsNyPeakStrict(b.clientLowerMs, b.clientUpperMs))
      .length;
  const totalWindows = ctx.windowsAll.length;
  const peakWindows = ctx.windowsAll.filter((w) => w.peakOverlap).length;
  const offPeakWindows = totalWindows - peakWindows;
  const qualifyingPeak = ctx.windowsAll.filter(
    (w) => inPeakInformativeCount(w) >= MIN_GROUP_BRACKETS,
  ).length;
  const qualifyingOffPeak = ctx.windowsAll.filter(
    (w) => !w.peakOverlap && informativeBrackets(w).length >= MIN_GROUP_BRACKETS,
  ).length;
  const a2Satisfied = qualifyingPeak >= 1 && qualifyingOffPeak >= 1;
  gate.push({
    id: "A4-2",
    requirement:
      "Multiple independent time windows including America/New_York 17:00-18:00 peak and one off-peak window, each with qualifying informative brackets; peak evidence only from brackets overlapping the peak hour itself",
    satisfied: a2Satisfied,
    evidence: `windows: ${totalWindows} total; qualifying peak (>=${MIN_GROUP_BRACKETS} informative in-peak brackets): ${qualifyingPeak}; qualifying off-peak (>=${MIN_GROUP_BRACKETS} informative brackets): ${qualifyingOffPeak} (raw: ${peakWindows} peak, ${offPeakWindows} off-peak)`,
  });

  // A4-3: distinguishable winner under holdout.
  const cmp = ctx.comparison;
  gate.push({
    id: "A4-3",
    requirement: "30s vs 60s model distinguishable with consistent winner under per-(target,window) holdout",
    satisfied: cmp.distinguishable === true,
    evidence: `c30=${cmp.maxCoverage30Common} c60=${cmp.maxCoverage60Common} on ${cmp.commonInformativeCount} common brackets (${cmp.clockSource} clock); reason=${cmp.reason}; holdout=${cmp.holdout.mode}`,
  });

  // A4-4: unified safe offset.
  const so = ctx.safeOffset;
  gate.push({
    id: "A4-4",
    requirement: "Unified safe offset covers phase and positive jitter across all targets/windows",
    satisfied: so.identifiable === true,
    evidence: so.identifiable
      ? `phase interval [${so.bound.phaseIntervalMs.startMs},${so.bound.phaseIntervalMs.endMs}] ms, max positive jitter ${so.bound.maxPositiveJitterMs} ms`
      : `not identifiable (${so.reason})`,
  });

  // A4-5: production conclusions must rest on server-clock evidence — the
  // comparison itself on the server clock, with server brackets covering every
  // qualifying group. A stray serverDate on an unrelated sample changes nothing.
  const se = ctx.serverEvidence;
  const a5Satisfied = se.sufficient === true;
  let a5Evidence;
  if (a5Satisfied) {
    a5Evidence = `server clock used; serverDate on ${ctx.clock.offsetDistribution.sampleCount} samples; qualifying groups with server evidence ${se.groupsWithServer}/${se.groupsTotal}; +1s quantization widening applied; client-vs-server offset p50=${ctx.clock.offsetDistribution.p50Ms} ms`;
  } else if (se.reason === "server-date-absent") {
    a5Evidence = "serverDate absent; client-clock-only";
  } else if (se.reason === "client-clock-fallback") {
    a5Evidence = `client-clock fallback: only ${se.serverCommonCount} server-informative comparison brackets (< ${MIN_COMPARISON_BRACKETS})`;
  } else if (se.reason === "no-qualifying-groups") {
    a5Evidence = "no qualifying groups";
  } else {
    a5Evidence = `server evidence missing in ${se.groupsTotal - se.groupsWithServer}/${se.groupsTotal} qualifying groups`;
  }
  gate.push({
    id: "A4-5",
    requirement:
      "Report honestly handles server Date precision, client clock, and request latency; production conclusions rest on server-clock evidence",
    satisfied: a5Satisfied,
    evidence: a5Evidence,
  });

  // A4-6: triple stability — whole-target leave-out, group leave-out, and
  // deterministic outlier removal must EACH keep the winner.
  let a6Satisfied = false;
  let a6Evidence = "not evaluable: no distinguishable winner";
  let stability = null;
  if (cmp.distinguishable === true) {
    stability = assessStability({
      brackets: ctx.brackets,
      comparison: cmp,
      clockSource: ctx.clockSource,
    });
    a6Satisfied = stability.targets.pass && stability.groups.pass && stability.outliers.pass;
    if (a6Satisfied) {
      a6Evidence = `stable: target-LOO ${stability.targets.count}/${stability.targets.count}, group-LOO ${stability.groups.count}/${stability.groups.count}, outlier top-k (k∈{${STABILITY_OUTLIER_KS.join(",")}}, ${stability.outliers.residualCount} residuals) winner unchanged`;
    } else if (stability.targets.degenerate) {
      a6Evidence = "target-LOO degenerate: single target in comparison set";
    } else if (!stability.targets.pass) {
      const fold = stability.targets.folds.find(
        (f) => !(f.distinguishable === true && f.winner === cmp.winner),
      );
      a6Evidence = `target-LOO failed: held-out ${fold.heldOut} → ${fold.reason}`;
    } else if (stability.groups.degenerate) {
      a6Evidence = "group-LOO degenerate: single (target,window) group in comparison set";
    } else if (!stability.groups.pass) {
      const fold = stability.groups.folds.find(
        (f) => !(f.distinguishable === true && f.winner === cmp.winner),
      );
      a6Evidence = `group-LOO failed: held-out ${fold.heldOut} → ${fold.reason}`;
    } else {
      const run = stability.outliers.runs.find(
        (r) => !(r.distinguishable === true && r.winner === cmp.winner),
      );
      a6Evidence = `outlier removal failed: k=${run.k} → ${run.reason}`;
    }
  }
  // Exposed to the report layer as comparison.stability (null when the
  // comparison is not distinguishable).
  cmp.stability = stability;
  gate.push({
    id: "A4-6",
    requirement:
      "Conclusions stable under whole-target leave-out, (target,window) group leave-out, and deterministic outlier removal",
    satisfied: a6Satisfied,
    evidence: a6Evidence,
  });

  const unsatisfied = gate.filter((g) => !g.satisfied);
  const verdict = unsatisfied.length === 0 ? "GO" : "NO_PRODUCTION_CHANGE";
  const dataRequired = gate
    .filter((g) => ["A4-1", "A4-2", "A4-3"].includes(g.id))
    .some((g) => !g.satisfied);

  const decision = { verdict };
  if (verdict !== "GO" && dataRequired) decision.qualifier = "DATA_REQUIRED";
  decision.reasons = unsatisfied.map((g) => `${g.id} unsatisfied: ${g.evidence}`);

  return { goGate: gate, decision };
}

// Diagnostic helper referenced by tests: an arc-membership check identical to
// what coverage uses (exported so tests can brute-force validate the sweep).
export function arcContainsPhase(bracket, periodMs, clockSource, phaseMs) {
  return arcContains(bracketArc(bracket, periodMs, clockSource), phaseMs, periodMs);
}
