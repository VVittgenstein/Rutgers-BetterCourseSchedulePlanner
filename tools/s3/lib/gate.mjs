// 30 s vs 60 s model comparison, unified safe-offset assessment, and the A4
// GO gate. The final verdict is always computed from the gate entries; there
// is no shortcut path.

import { internalAssert } from "./errors.mjs";
import {
  MIN_COMPARISON_BRACKETS,
  REQUIRED_CAMPUSES,
  MIN_GROUP_BRACKETS,
  isInformative,
  fitPhase,
  bracketArc,
  arcContains,
  bracketBounds,
} from "./phase.mjs";
import { runHoldout, groupBrackets } from "./holdout.mjs";

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

export function assessSafeOffset(comparison, clockStatus) {
  if (clockStatus === "unknown") {
    return { identifiable: false, reason: "clock-unknown" };
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

// ctx: {
//   targets: [{ targetId, campus, windows: [{ windowId, peakOverlap, brackets }] }],
//   windowsAll: [{ windowId, peakOverlap }],
//   brackets, comparison, safeOffset, clock, clockSource
// }
export function evaluateGate(ctx) {
  const gate = [];

  // A4-1: multi-target evidence.
  const qualifyingByCampus = new Map();
  for (const target of ctx.targets) {
    if (target.targetId.startsWith("unknown:")) continue;
    const qualifies = target.windows.some(
      (w) =>
        w.brackets.filter((b) => isInformative(b, 30000, ctx.clockSource)).length >=
        MIN_GROUP_BRACKETS,
    );
    if (qualifies && target.campus !== null) {
      if (!qualifyingByCampus.has(target.campus)) qualifyingByCampus.set(target.campus, []);
      qualifyingByCampus.get(target.campus).push(target.targetId);
    }
  }
  const missingCampuses = REQUIRED_CAMPUSES.filter((c) => !qualifyingByCampus.has(c));
  const qualifyingIds = [...qualifyingByCampus.values()].flat().sort();
  const a1Satisfied = missingCampuses.length === 0;
  const a1Evidence = a1Satisfied
    ? `targets: ${qualifyingIds.join(", ")}`
    : `targets: ${qualifyingIds.length > 0 ? qualifyingIds.join(", ") + " only" : "none"}${missingCampuses
        .map((c) => `; ${c} missing`)
        .join("")}`;
  gate.push({
    id: "A4-1",
    requirement: "Multi-target evidence: at least NB, NK, CM independently evaluable",
    satisfied: a1Satisfied,
    evidence: a1Evidence,
  });

  // A4-2: multiple independent time windows incl. NY peak and off-peak.
  const totalWindows = ctx.windowsAll.length;
  const peakWindows = ctx.windowsAll.filter((w) => w.peakOverlap).length;
  const offPeakWindows = totalWindows - peakWindows;
  const a2Satisfied = totalWindows >= 2 && peakWindows >= 1 && offPeakWindows >= 1;
  gate.push({
    id: "A4-2",
    requirement:
      "Multiple independent time windows including America/New_York 17:00-18:00 peak and one off-peak window",
    satisfied: a2Satisfied,
    evidence: `windows: ${totalWindows} total, ${peakWindows} peak-overlapping, ${offPeakWindows} off-peak`,
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

  // A4-5: honest clock handling — structurally satisfiable by the tool itself.
  const a5Satisfied = ctx.clock.status === "server-date-available";
  gate.push({
    id: "A4-5",
    requirement: "Report honestly handles server Date precision, client clock, and request latency",
    satisfied: a5Satisfied,
    evidence: a5Satisfied
      ? `serverDate present on ${ctx.clock.offsetDistribution.sampleCount} samples; +1s quantization widening applied; client-vs-server offset p50=${ctx.clock.offsetDistribution.p50Ms} ms`
      : "serverDate absent; client-clock-only",
  });

  // A4-6: stability under leave-out of any single group.
  let a6Satisfied = false;
  let a6Evidence = "not evaluable: no distinguishable winner";
  if (cmp.distinguishable === true) {
    const groups = groupBrackets(cmp.comparisonSet);
    const stable = groups.every((group) => {
      const remaining = ctx.brackets.filter(
        (b) => `${b.targetId}/${b.windowId}` !== group.groupId,
      );
      const rerun = compareModels(remaining, ctx.clockSource);
      return rerun.distinguishable === true && rerun.winner === cmp.winner;
    });
    a6Satisfied = stable;
    a6Evidence = stable
      ? `winner ${cmp.winner} unchanged under leave-out of each of ${groups.length} groups`
      : "winner not stable under single-group leave-out";
  }
  gate.push({
    id: "A4-6",
    requirement: "Conclusions stable under leave-out of outliers / single target",
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
