// Circular arc-coverage phase model.
//
// Each change bracket (lower, upper] (integer ms, half-open) maps to an arc on
// the circle [0, P). Because all bounds are integer ms, (lower, upper] equals
// the closed integer interval [lower+1, upper]; the arc is that interval mod P,
// wrapping when needed. Coverage is a step function whose breakpoints are arc
// endpoints, so an exact event sweep finds the maximum coverage and every
// maximal phase run. No timestamp-modulo histogram is ever used: a histogram of
// detection timestamps confounds the sampling cadence with the change phase,
// while interval-censored arcs do not.

import { AnalyzerError, internalAssert } from "./errors.mjs";
import { fmtRatio } from "./stable.mjs";

// Shared constants (single source of truth; gate.mjs re-exports them).
export const TOOL_VERSION = "2.8.0";
export const SCHEMA_VERSION = 6;
export const PERIODS_MS = [30000, 60000];
export const SERVER_DATE_WIDEN_MS = 1000;
export const CLIENT_TIME_REGRESSION_TOLERANCE_MS = 2000;
export const SERVER_DATE_REGRESSION_CAVEAT_MS = 1000;
export const WINDOW_GAP_MIN_MS = 600000;
export const WINDOW_GAP_INTERVAL_FACTOR = 5;
export const MIN_COMPARISON_BRACKETS = 10;
export const MIN_GROUP_BRACKETS = 5;
// Deterministic outlier-sensitivity: remove the top-k most-residual brackets
// (k from this list, capped at the residual count) and require the winner to
// survive each removal.
export const STABILITY_OUTLIER_KS = [1, 2];
export const REQUIRED_CAMPUSES = ["NB", "NK", "CM"];
// Provenance FAMILY thresholds (provenance.mjs / series-match.mjs). They decide
// when two observation series are the same captured data reused, not two
// independent captures.
//
// DERIVATION_MIN_BLOCK counts matching INTERIOR canonical entries, i.e. 4
// matches means 5 consecutive samples agreeing on 5 body hashes AND on 4
// exactly-equal millisecond client steps. Measured on the real local capture
// (554 samples): the longest such repeat at any non-zero self-offset is 1, and
// across the three independent local captures it is 0 — a factor-of-4 margin
// over an empirical maximum of 1. An attacker cannot go below it and still
// reuse enough of a session to qualify a window (MIN_GROUP_BRACKETS = 5).
export const DERIVATION_MIN_BLOCK = 4;
// …and that shared block must contain at least one BODY CHANGE. The one
// plausible accidental long block is a stale run under an exactly-scheduled
// poller (constant delta, one repeated body hash); such a block yields no
// change bracket on either side, so requiring a change costs nothing defensively
// and removes the whole coincidence class.
export const DERIVATION_MIN_BLOCK_CHANGES = 1;
// Reused (clientStartMs, bodySha) observation records. Two independent captures
// never collide on even one (measured: 0 shared record keys between every pair
// of the three local captures, and 0 between the disjoint halves of one of
// them); a stream sharing fewer than 3 could not reach MIN_GROUP_BRACKETS
// anyway, so 3 buys robustness without giving up any detection.
export const DERIVATION_MIN_RECORDS = 3;
// The same reuse counted at a NON-ZERO constant client-clock offset. The
// absolute rule above tests exactly ONE offset; the shift-invariant rule
// searches every offset that any same-body pair produces (7363 distinct
// offsets inside the 554-sample local capture alone), so its threshold has to
// be re-argued rather than inherited.
//
// Two independent grounds for 6:
//   - Empirical ceiling. On the real local capture compared with itself — the
//     only real body-repetition structure available, and the friendliest case
//     for an accidental match — the largest non-zero-offset candidate bucket
//     holds 3 pairs (offsets ±13405 ms, ±27127 ms, ±28068 ms); none reaches 6.
//     Between the three independent local captures there is no bucket at all:
//     they share zero decodedBodySha256 values pairwise. 6 is 2x the measured
//     accidental ceiling.
//   - Zero detection cost. A stream must supply MIN_GROUP_BRACKETS = 5
//     informative brackets to qualify a window or a session, and 5 brackets
//     need at least 6 samples (each bracket is an adjacent-pair body change).
//     A reuse of <= 5 records can therefore qualify nothing, so the higher
//     shifted threshold forfeits no detection whatsoever.
//
// What it deliberately does NOT catch is per-sample jitter: with a different
// offset per sample the offsets do not coincide, every bucket collapses toward
// size 1, and the streams stay independent families. That is the documented,
// contract-deferred boundary — and the rule cannot drift into it, because the
// offset match is an exact integer equality with no tolerance window.
export const DERIVATION_MIN_RECORDS_SHIFTED = 6;
export const NY_PEAK = { startHour: 17, endHour: 18, timeZone: "America/New_York" };

export function bracketBounds(bracket, clockSource) {
  if (clockSource === "server") {
    return { lowerMs: bracket.serverLowerMs, upperMs: bracket.serverUpperMs };
  }
  if (clockSource === "client") {
    return { lowerMs: bracket.clientLowerMs, upperMs: bracket.clientUpperMs };
  }
  throw new AnalyzerError("E_INTERNAL", `unknown clockSource ${clockSource}`);
}

export function bracketWidthMs(bracket, clockSource) {
  const { lowerMs, upperMs } = bracketBounds(bracket, clockSource);
  if (lowerMs === null || upperMs === null) return null;
  return upperMs - lowerMs;
}

export function isInformative(bracket, periodMs, clockSource) {
  const width = bracketWidthMs(bracket, clockSource);
  // width <= 0 is a corrupt bracket, not a narrow one: it must never count as
  // informative (a negative-width "arc" would cover almost the whole circle).
  return width !== null && width > 0 && width < periodMs;
}

function mod(n, m) {
  return ((n % m) + m) % m;
}

// Closed integer arc of a bracket on [0, P): {startMs, endMs}, wrapping when
// endMs < startMs. Only valid for informative brackets (width < P).
export function bracketArc(bracket, periodMs, clockSource) {
  const { lowerMs, upperMs } = bracketBounds(bracket, clockSource);
  internalAssert(lowerMs !== null && upperMs !== null, "arc requested for unusable bracket");
  internalAssert(upperMs - lowerMs > 0, "arc requested for non-positive-width bracket");
  internalAssert(upperMs - lowerMs < periodMs, "arc requested for non-informative bracket");
  return { startMs: mod(lowerMs + 1, periodMs), endMs: mod(upperMs, periodMs) };
}

export function arcContains(arc, phaseMs, periodMs) {
  void periodMs;
  if (arc.startMs <= arc.endMs) {
    return phaseMs >= arc.startMs && phaseMs <= arc.endMs;
  }
  return phaseMs >= arc.startMs || phaseMs <= arc.endMs;
}

// Unroll a (possibly wrapped) closed circular interval into linear closed
// integer intervals within [0, P-1].
function unroll(arc, periodMs) {
  if (arc.startMs <= arc.endMs) {
    return [[arc.startMs, arc.endMs]];
  }
  return [
    [arc.startMs, periodMs - 1],
    [0, arc.endMs],
  ];
}

// Exact circular sweep over closed integer intervals.
// Returns { maxCoverage, bestPhaseIntervals } where bestPhaseIntervals is the
// list of maximal closed integer runs on [0, P) at max coverage, sorted by
// startMs. A run crossing 0/P appears as two entries (wrap-split).
export function sweepArcs(arcs, periodMs) {
  if (arcs.length === 0) {
    return { maxCoverage: 0, bestPhaseIntervals: [] };
  }
  // Events at integer positions: +1 at start, -1 at end+1.
  const deltas = new Map();
  const bump = (pos, d) => deltas.set(pos, (deltas.get(pos) ?? 0) + d);
  for (const arc of arcs) {
    for (const [start, end] of unroll(arc, periodMs)) {
      bump(start, +1);
      if (end + 1 < periodMs) {
        bump(end + 1, -1);
      }
      // A piece ending at P-1 closes at the circle seam: the linear walk over
      // [0, P-1] ends there anyway, so no closing event is needed (emitting one
      // at 0 would wrongly cancel coverage at the seam).
    }
  }
  const positions = [...deltas.keys()].sort((a, b) => a - b);
  // Walk the circle once to establish the coverage entering position 0:
  // sum of all +1 whose interval covers 0 is already accounted by the
  // unrolling (wrapped tails emit [0, end]), so coverage before the first
  // breakpoint is the running sum starting at 0.
  let coverage = 0;
  let maxCoverage = 0;
  const segments = []; // { startMs, endMs, coverage } closed segments
  for (let i = 0; i < positions.length; i += 1) {
    coverage += deltas.get(positions[i]);
    const segStart = positions[i];
    const segEnd = (i + 1 < positions.length ? positions[i + 1] : periodMs) - 1;
    if (segStart <= segEnd) {
      segments.push({ startMs: segStart, endMs: segEnd, coverage });
      if (coverage > maxCoverage) maxCoverage = coverage;
    }
  }
  // If position 0 had no event, nothing covers 0: every unrolled piece that
  // covers 0 starts at 0 and would have emitted a +1 event there. The gap
  // [0, positions[0]-1] therefore has coverage 0.
  if (positions[0] !== 0) {
    segments.unshift({ startMs: 0, endMs: positions[0] - 1, coverage: 0 });
  }
  // Collect maximal runs at max coverage, merging adjacent segments.
  const runs = [];
  for (const seg of segments) {
    if (seg.coverage !== maxCoverage) continue;
    const last = runs[runs.length - 1];
    if (last && last.endMs + 1 === seg.startMs) {
      last.endMs = seg.endMs;
    } else {
      runs.push({ startMs: seg.startMs, endMs: seg.endMs });
    }
  }
  runs.sort((a, b) => a.startMs - b.startMs);
  return { maxCoverage, bestPhaseIntervals: runs };
}

// Fits the phase model for one period and clock source.
export function fitPhase(brackets, periodMs, clockSource) {
  const usable = [];
  let unusableCount = 0;
  let nonInformativeCount = 0;
  for (const bracket of brackets) {
    const width = bracketWidthMs(bracket, clockSource);
    if (width === null || width <= 0) {
      // null: bounds unavailable on this clock. <= 0: corrupt (fail-closed) —
      // buildBrackets already rejects these, but any bracket reaching this
      // point is still excluded rather than credited.
      unusableCount += 1;
    } else if (width >= periodMs) {
      // Wide brackets cover the full circle: non-informative, excluded from
      // coverage counting, never silently credited.
      nonInformativeCount += 1;
    } else {
      usable.push(bracket);
    }
  }
  const arcs = usable.map((b) => bracketArc(b, periodMs, clockSource));
  const { maxCoverage, bestPhaseIntervals } = sweepArcs(arcs, periodMs);
  const informativeCount = usable.length;

  let residualBracketIds = [];
  if (bestPhaseIntervals.length > 0) {
    const phiMs = bestPhaseIntervals[0].startMs;
    residualBracketIds = usable
      .filter((b, i) => !arcContains(arcs[i], phiMs, periodMs))
      .map((b) => b.bracketId)
      .sort();
    internalAssert(
      residualBracketIds.length === informativeCount - maxCoverage,
      "residual count must equal informativeCount - maxCoverage",
    );
  }

  return {
    periodSeconds: periodMs / 1000,
    clockSource,
    informativeCount,
    nonInformativeCount,
    unusableCount,
    maxCoverage,
    coverageRatio: informativeCount > 0 ? fmtRatio(maxCoverage / informativeCount) : null,
    bestPhaseIntervals,
    residualBracketIds,
  };
}

// Coverage of a single phase point over a bracket list (used by holdout).
export function coverageAtPhase(brackets, periodMs, clockSource, phaseMs) {
  let covered = 0;
  for (const bracket of brackets) {
    if (!isInformative(bracket, periodMs, clockSource)) continue;
    if (arcContains(bracketArc(bracket, periodMs, clockSource), phaseMs, periodMs)) {
      covered += 1;
    }
  }
  return covered;
}
