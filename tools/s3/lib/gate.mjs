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
  bracketWidthMs,
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

// Unit separator: never present in an inputId, a targetId, or a windowId, so
// `${streamId}<US>${index}` cannot collide across streams.
const SESSION_KEY_SEP = "\u001f";

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

  // (a) whole-target leave-out. Keying on targetId over the comparison set is
  // sound only because duplicate provenance is excluded upstream: a relabeled
  // copy of a capture contributes zero evidence brackets, so its targetId can
  // never appear here as an extra fold. Distinct classes sharing a targetId
  // (repeated genuine captures of one target) are held out together — the
  // whole target leaves, which is the stricter test.
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

function cmpStrGate(a, b) {
  return a < b ? -1 : a > b ? 1 : 0;
}

function zeroPad2(n) {
  return String(n).padStart(2, "0");
}

// A4-2's evidence grouping: EVIDENCE SESSIONS.
//
// A session is a maximal group of one stream's evidence windows that are
// contiguous on the client timeline OR on the server timeline. The client half
// is the window partition itself (segmentWindows already split on client-clock
// gaps); the server half links two windows whenever they share a server
// session index, which assignServerSessions derives from serverDate gaps under
// the SAME max(10 min, 5 x interval) rule, measured across the seam as
// suffixMin - prefixMax so that no single forged Date can mint a boundary.
//
// Independence therefore has to hold on BOTH clocks. That closes two mirror
// forgeries with one rule: a client-clock jump inside one server-contiguous
// session no longer mints two evidence groups (the shared server index links
// the windows back together), and an edited serverDate inside one client
// window never did (the window itself is the link). The third mirror — a
// forged Date at the client-window SEAM, which is the one place a server SPLIT
// would separate the two windows — is closed inside assignServerSessions, and
// its own mirror, a forged Date that SUPPRESSES a genuine boundary and pools
// two server-separated sessions, is closed by serverGroupingRobustness(): the
// analyzer marks every window of a stream whose grouping a single held-out
// Date would change, and A4-2 below refuses those sessions as evidence.
//
// Sessions are unions of WHOLE windows, so the session partition is always a
// COARSENING of the window partition: sessionCount <= evidenceWindowCount.
// COARSER IS NOT STRICTER. A4-2 counts informative brackets per session against
// MIN_GROUP_BRACKETS, so merging two sub-threshold sessions manufactures a
// qualifying one — the peak side directly, and the off-peak side too, since
// the union of two pure off-peak sessions is still pure. A4-2 is therefore
// anti-monotone under coarsening on both sides, and no argument of the form
// "sessions can only get coarser, so nothing that failed can now pass" is
// valid here. Both directions have to be defended explicitly: splits by the
// order-statistic seam, merges by the leave-one-out grouping check.
// The anti-lockout risk in the other direction — genuinely separated windows
// merging — is disproved by the ~19 h two-session control.
//
// Scope is the STREAM (`${inputId}::${targetId}`), not the windowId: windowIds
// are `${inputId}#wNN` and collide across the several targets of one SQLite
// input.
export function buildEvidenceSessions(evidenceWindows) {
  const ordered = [...evidenceWindows].sort(
    (a, b) => cmpStrGate(a.streamId, b.streamId) || cmpStrGate(a.windowId, b.windowId),
  );
  const parent = ordered.map((_, i) => i);
  const find = (x) => {
    let root = x;
    while (parent[root] !== root) root = parent[root];
    let cur = x;
    while (parent[cur] !== root) {
      const next = parent[cur];
      parent[cur] = root;
      cur = next;
    }
    return root;
  };
  const union = (x, y) => {
    const rx = find(x);
    const ry = find(y);
    if (rx !== ry) parent[Math.max(rx, ry)] = Math.min(rx, ry);
  };
  // `${streamId}` and the index are joined by a character no id can contain.
  const firstHolderOf = new Map();
  for (let i = 0; i < ordered.length; i += 1) {
    for (const idx of ordered[i].serverSessionIndices ?? []) {
      const key = `${ordered[i].streamId}${SESSION_KEY_SEP}${idx}`;
      const prev = firstHolderOf.get(key);
      if (prev === undefined) firstHolderOf.set(key, i);
      else union(prev, i);
    }
  }
  const components = new Map(); // root rank -> [rank, ...] ascending
  for (let i = 0; i < ordered.length; i += 1) {
    const root = find(i);
    if (!components.has(root)) components.set(root, []);
    components.get(root).push(i);
  }
  const sessions = [];
  const perStreamCount = new Map();
  // Components are keyed by their MINIMUM rank, and `ordered` is sorted by
  // (streamId, windowId), so walking roots ascending numbers each stream's
  // sessions in ascending order of their first client window.
  for (const [, ranks] of [...components.entries()].sort((a, b) => a[0] - b[0])) {
    const windows = ranks.map((r) => ordered[r]);
    const streamIds = [...new Set(windows.map((w) => w.streamId))];
    internalAssert(streamIds.length === 1, "an evidence session must not span streams");
    const streamId = streamIds[0];
    const n = perStreamCount.get(streamId) ?? 0;
    perStreamCount.set(streamId, n + 1);
    sessions.push({
      sessionId: `${streamId}#s${zeroPad2(n)}`,
      streamId,
      windowIds: windows.map((w) => w.windowId),
      // Whole windows in windowId order; each window's brackets are already in
      // changedSeq order, so the concatenation is deterministic.
      brackets: windows.flatMap((w) => w.brackets),
      // True when one held-out serverDate would have regrouped this stream's
      // windows (serverGroupingRobustness, set per window by the analyzer):
      // the session boundaries around this session are decided by a single
      // Date header rather than by the server timeline.
      serverGroupingAmbiguous: windows.some((w) => w.serverGroupingAmbiguous === true),
    });
  }
  internalAssert(
    sessions.reduce((acc, sess) => acc + sess.windowIds.length, 0) === evidenceWindows.length,
    "every evidence window must land in exactly one evidence session",
  );
  return sessions;
}

// ctx: {
//   windowsAll: [{ windowId, streamId, peakOverlap, excluded, brackets }],
//       // ALL windows; excluded=true marks conflicted provenance (campus
//       // labels or absolute time anchors) or a duplicate
//       // (non-representative) member of a clean class
//   brackets,  // evidence brackets only (excluded streams already removed)
//   comparison, safeOffset, clock, clockSource,
//   provenance: buildProvenance() result, serverEvidence: assessServerClockEvidence() result
// }
export function evaluateGate(ctx) {
  const gate = [];
  const evidenceWindows = ctx.windowsAll.filter((w) => w.excluded !== true);

  // A4-1: multi-target evidence counted by independent observation-data
  // provenance, not by campus labels. A provenance class contributes at most
  // one campus, and only when one of its OWN member streams has a qualifying
  // window (>= MIN_GROUP_BRACKETS informative brackets in a window built from
  // that stream) — an evidentially worthless but clean series can never
  // piggyback on qualifying windows that belong to another stream merely
  // sharing its targetId. A class whose members carry conflicting campus
  // labels (the copy-and-relabel attack) or conflicting absolute time anchors
  // (the copy-and-translate / serverDate-edit attacks) contributes nothing, and its member
  // streams are excluded from all evidence upstream. Non-representative
  // members of clean classes (identical/contained duplicates, e.g. the same
  // capture relabeled to another term or re-fed as SQLite) are also excluded
  // upstream, so only the representative's windows can qualify a class and
  // duplicated data never multiplies evidence.
  const streamQualifies = new Set();
  for (const w of evidenceWindows) {
    const informative = w.brackets.filter((b) => isInformative(b, 30000, ctx.clockSource)).length;
    if (informative >= MIN_GROUP_BRACKETS) streamQualifies.add(w.streamId);
  }
  const coveredBy = new Map(); // campus -> classId (first covering class, classId asc)
  let conflictClassCount = 0;
  let excludedStreamCount = 0;
  let timeConflictClassCount = 0;
  let timeConflictStreamCount = 0;
  for (const cls of ctx.provenance.classes) {
    if (cls.campusConflict || cls.timeConflict) {
      if (cls.campusConflict) {
        conflictClassCount += 1;
        excludedStreamCount += cls.members.length;
      }
      // A time-anchor conflict (the same canonical series claimed at two
      // different absolute times, or with edited/deleted serverDates) voids
      // the whole class even when every member agrees on the campus label:
      // choosing a representative among disagreeing records would let an
      // edited copy supply the only peak/off-peak/clock evidence.
      if (cls.timeConflict && !cls.campusConflict) {
        timeConflictClassCount += 1;
        timeConflictStreamCount += cls.members.length;
      }
      continue;
    }
    if (cls.campus === null || !REQUIRED_CAMPUSES.includes(cls.campus)) continue;
    const qualifies = cls.members.some((m) => streamQualifies.has(m.streamId));
    if (qualifies && !coveredBy.has(cls.campus)) coveredBy.set(cls.campus, cls.classId);
  }
  const missingCampuses = REQUIRED_CAMPUSES.filter((c) => !coveredBy.has(c));
  const coveredList = REQUIRED_CAMPUSES.filter((c) => coveredBy.has(c)).map(
    (c) => `${c}(${coveredBy.get(c)})`,
  );
  const conflictSuffix =
    (conflictClassCount > 0
      ? `; ${conflictClassCount} class(es) with conflicting campus labels ignored and their ${excludedStreamCount} stream(s) excluded from all evidence`
      : "") +
    (timeConflictClassCount > 0
      ? `; ${timeConflictClassCount} class(es) with conflicting absolute time anchors (time-translated or serverDate-edited duplicate observation series) ignored and their ${timeConflictStreamCount} stream(s) excluded from all evidence`
      : "");
  let duplicateStreamCount = 0;
  let derivedMergeCount = 0;
  for (const cls of ctx.provenance.classes) {
    if (!cls.campusConflict && !cls.timeConflict) duplicateStreamCount += cls.members.length - 1;
    // Members that joined their family as a re-slice or a subsample of another
    // member rather than as a whole-series copy — reported so the reader can
    // see that a derivation, not a byte-copy, was what stopped counting twice.
    derivedMergeCount += cls.members.filter(
      (m) => m.relation === "overlapping" || m.relation === "derived",
    ).length;
  }
  const duplicateSuffix =
    (duplicateStreamCount > 0
      ? `; ${duplicateStreamCount} duplicate stream(s) (identical, contained, overlapping, or subsampled observation series) excluded from evidence — duplicates never add target coverage`
      : "") +
    (derivedMergeCount > 0
      ? `; ${derivedMergeCount} stream(s) merged into an existing provenance family as overlapping slices or subsampled derivations`
      : "");
  const a1Satisfied = missingCampuses.length === 0;
  const a1Evidence = a1Satisfied
    ? `campuses: ${coveredList.join(", ")} from ${ctx.provenance.classes.length} provenance classes${conflictSuffix}${duplicateSuffix}`
    : `campuses: ${coveredList.length > 0 ? coveredList.join(", ") + " only" : "none"}${missingCampuses
        .map((c) => `; ${c} missing`)
        .join("")}${conflictSuffix}${duplicateSuffix}`;
  gate.push({
    id: "A4-1",
    requirement:
      "Multi-target evidence: at least NB, NK, CM independently evaluable from independent data provenance",
    satisfied: a1Satisfied,
    evidence: a1Evidence,
  });

  // A4-2: multiple independent time windows incl. NY peak and off-peak — each
  // side must have at least one QUALIFYING EVIDENCE SESSION (>=
  // MIN_GROUP_BRACKETS informative brackets of its own on the comparison
  // clock), and the two sides may never be the same session.
  //
  // The grouping unit is the SESSION, not the client window. Client windows
  // are cut on client-clock gaps alone, so jumping the client clock forward
  // inside ONE server-contiguous session split it into #w00 and #w01 and let
  // that single session supply the off-peak side from its first half and the
  // peak side from its second — CE-14's shape with a forged claim of
  // independence bolted on, and every bracket bound involved genuinely
  // server-derived. Production GO already requires the server clock, so
  // "independent session" must be provable on the server timeline too:
  // buildEvidenceSessions merges windows that are contiguous on EITHER clock
  // under the same max(10 min, 5 x interval) rule. The client windowId
  // survives as the display label in the tables.
  //
  // A SESSION WHOSE GROUPING RESTS ON ONE Date HEADER SUPPLIES NO EVIDENCE.
  // The mirror of the manufactured split is the manufactured MERGE: pooling
  // two genuinely server-separated sessions, each holding fewer than
  // MIN_GROUP_BRACKETS brackets on its side, into one that clears the
  // threshold. Purity does not stop it either — the union of two pure
  // off-peak sessions is pure. Because every seam statistic is one number that
  // one cell can push, the defence is the leave-one-out check the analyzer
  // runs per stream (serverGroupingRobustness): if holding out any single
  // dated sample's Date would regroup the stream's windows, the server
  // timeline has not established the grouping, and neither the peak nor the
  // off-peak side may count those sessions. Voiding only ever REMOVES
  // evidence, so this can never turn a NO-GO into a GO.
  //
  // BOTH sides are classified per BRACKET on the COMPARISON clock — the same
  // bounds the model comparison, the safe offset and A4-5 consume. Whenever a
  // production GO is reachable that clock is the server clock, so a peak or
  // off-peak claim always rests on the server's own Date evidence.
  //
  // The previous rule classified peak membership from the CLIENT outer
  // envelope (stable requestStart, changed requestEnd] and the off-peak side
  // from the window's client-envelope label. Both were forgeable without
  // touching a single body, request start or serverDate: extending only
  // `requestEndedUtc` dragged an off-peak bracket's client envelope across
  // 17:00 ET and manufactured peak evidence, while a client clock running an
  // hour slow labelled a wholly in-peak server window as off-peak. The
  // client-vs-server offset is NOT "orders of magnitude below the peak span"
  // when an attacker chooses it.
  //
  // A bracket with no usable bounds on the comparison clock classifies as
  // neither side (fail closed — it must never fill in a production peak or
  // off-peak gap). On the server clock that state is unreachable inside
  // `informativeBrackets` because isInformative(_, _, "server") already
  // requires both server bounds; the branch stays as a guard.
  //
  // The off-peak side additionally demands a PURE off-peak window: every one
  // of that window's brackets must classify off-peak on the comparison clock.
  // Without the purity clause a SINGLE session straddling 17:00 ET satisfies
  // both sides by itself — its early brackets off-peak, its late ones in the
  // peak hour — and a per-bracket rule would then be strictly WEAKER than the
  // window-label rule it replaced, which required an off-peak window whose
  // whole span sat outside the peak hour. Purity is checked over ALL of the
  // window's brackets, not only the informative ones, so a straddling session
  // cannot buy purity by making its peak-side brackets too wide to be
  // informative or by dropping their `Date` headers.
  //
  // The peak side needs no mirror clause: brackets whose own comparison-clock
  // bounds fall inside the peak hour ARE peak evidence wherever the rest of
  // their window sits. The two qualifying sets are disjoint by construction
  // (a qualifying peak window holds >= MIN_GROUP_BRACKETS in-peak brackets, so
  // it is never pure) — asserted below rather than left to a count of ids.
  const bracketPeakState = (bracket, clockSource) => {
    const { lowerMs, upperMs } = bracketBounds(bracket, clockSource);
    if (lowerMs === null || upperMs === null) return "no-bounds";
    return overlapsNyPeakStrict(lowerMs, upperMs) ? "in-peak" : "off-peak";
  };
  const sessions = buildEvidenceSessions(evidenceWindows);
  const informativeBrackets = (sess) =>
    sess.brackets.filter((b) => isInformative(b, 30000, ctx.clockSource));
  const inPeakInformativeCount = (sess) =>
    informativeBrackets(sess).filter((b) => bracketPeakState(b, ctx.clockSource) === "in-peak")
      .length;
  const offPeakInformativeCount = (sess) =>
    informativeBrackets(sess).filter((b) => bracketPeakState(b, ctx.clockSource) === "off-peak")
      .length;
  const isPureOffPeakSession = (sess) =>
    sess.brackets.every((b) => bracketPeakState(b, ctx.clockSource) === "off-peak");
  const totalWindows = ctx.windowsAll.length;
  const excludedWindows = totalWindows - evidenceWindows.length;
  // Window-level peakOverlap is the client-envelope LABEL rendered in the
  // tables; it is reported for orientation and never used as evidence.
  const peakWindows = evidenceWindows.filter((w) => w.peakOverlap).length;
  const offPeakWindows = evidenceWindows.length - peakWindows;
  const groupingEstablished = (sess) => sess.serverGroupingAmbiguous !== true;
  const qualifyingPeakSessions = sessions.filter(
    (sess) => groupingEstablished(sess) && inPeakInformativeCount(sess) >= MIN_GROUP_BRACKETS,
  );
  const qualifyingOffPeakSessions = sessions.filter(
    (sess) =>
      groupingEstablished(sess) &&
      isPureOffPeakSession(sess) &&
      offPeakInformativeCount(sess) >= MIN_GROUP_BRACKETS,
  );
  // Sessions refused because one held-out Date header would have regrouped
  // their stream's windows. Disclosed for the same reason the straddle count
  // is: a reader must never have to guess why a session with enough brackets
  // did not count.
  const ambiguousGroupingSessions = sessions.filter((sess) => !groupingEstablished(sess)).length;
  // Sessions the purity clause refuses: enough off-peak brackets to qualify,
  // but they also observed the peak hour (or time the comparison clock cannot
  // place). Disclosed so a reader never has to guess why a session with plenty
  // of off-peak brackets did not count.
  const straddlingOffPeakSessions = sessions.filter(
    (sess) => !isPureOffPeakSession(sess) && offPeakInformativeCount(sess) >= MIN_GROUP_BRACKETS,
  ).length;
  const qualifyingPeakIds = new Set(qualifyingPeakSessions.map((sess) => sess.sessionId));
  const qualifyingOffPeakIds = new Set(qualifyingOffPeakSessions.map((sess) => sess.sessionId));
  internalAssert(
    qualifyingOffPeakSessions.every((sess) => !qualifyingPeakIds.has(sess.sessionId)),
    "A4-2 qualifying peak and off-peak session sets must be disjoint",
  );
  const a2Satisfied = qualifyingPeakSessions.length >= 1 && qualifyingOffPeakSessions.length >= 1;
  // Reported to the JSON/MD layer, computed once here so the tables and the
  // gate string can never disagree about the grouping.
  const evidenceSessions = sessions.map((sess) => ({
    sessionId: sess.sessionId,
    streamId: sess.streamId,
    windowIds: [...sess.windowIds],
    bracketCount: sess.brackets.length,
    informativeCount: informativeBrackets(sess).length,
    inPeakInformativeCount: inPeakInformativeCount(sess),
    offPeakInformativeCount: offPeakInformativeCount(sess),
    pureOffPeak: isPureOffPeakSession(sess),
    qualifiesPeak: qualifyingPeakIds.has(sess.sessionId),
    qualifiesOffPeak: qualifyingOffPeakIds.has(sess.sessionId),
    serverGroupingAmbiguous: sess.serverGroupingAmbiguous === true,
  }));
  // Honest accounting of what the server-clock requirement drops: brackets the
  // client clock would have called informative but that carry no usable server
  // bounds, so they can qualify neither side.
  const noServerBoundsInformative =
    ctx.clockSource === "server"
      ? evidenceWindows
          .flatMap((w) => w.brackets)
          .filter((b) => isInformative(b, 30000, "client") && bracketWidthMs(b, "server") === null)
          .length
      : 0;
  const excludedNote =
    excludedWindows > 0
      ? ` (${excludedWindows} excluded: conflicted (campus or time-anchor) or duplicate provenance)`
      : "";
  const mergedWindowCount = evidenceWindows.length - sessions.length;
  const mergedNote =
    mergedWindowCount > 0
      ? `; ${mergedWindowCount} client window(s) merged into a session with another client window`
      : "";
  const noServerBoundsNote =
    noServerBoundsInformative > 0
      ? `; ${noServerBoundsInformative} client-informative bracket(s) had no usable server bounds and could not qualify peak or off-peak evidence`
      : "";
  const ambiguousGroupingNote =
    ambiguousGroupingSessions > 0
      ? `; ${ambiguousGroupingSessions} session(s) supplied no evidence because holding out ONE serverDate header regroups their stream's windows — the server timeline does not establish those session boundaries`
      : "";
  const straddleNote =
    straddlingOffPeakSessions > 0
      ? `; ${straddlingOffPeakSessions} session(s) with >=${MIN_GROUP_BRACKETS} off-peak brackets also hold peak-hour or unclassifiable brackets and cannot supply off-peak evidence`
      : "";
  gate.push({
    id: "A4-2",
    requirement:
      "Multiple independent time windows including America/New_York 17:00-18:00 peak and one off-peak window, each with qualifying informative brackets; evidence is grouped into sessions independent on BOTH the client and the server timeline, and a session counts only when no single held-out serverDate header would regroup its stream's windows; peak and off-peak evidence only from brackets whose own comparison-clock bounds fall in that regime, and the off-peak session must hold no peak-hour bracket at all, so neither one straddling session nor a client-clock jump inside one server-contiguous session can supply both sides",
    satisfied: a2Satisfied,
    evidence: `windows: ${totalWindows} total${excludedNote}; evidence sessions: ${sessions.length} from ${evidenceWindows.length} evidence window(s), grouped on the server timeline${mergedNote}; peak/off-peak classified on the ${ctx.clockSource} clock; qualifying peak sessions (>=${MIN_GROUP_BRACKETS} informative in-peak brackets): ${qualifyingPeakSessions.length}; qualifying off-peak sessions (>=${MIN_GROUP_BRACKETS} informative off-peak brackets, none in peak): ${qualifyingOffPeakSessions.length} (window labels: ${peakWindows} peak-overlapping, ${offPeakWindows} off-peak)${noServerBoundsNote}${straddleNote}${ambiguousGroupingNote}`,
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

  return { goGate: gate, decision, evidenceSessions };
}

// Diagnostic helper referenced by tests: an arc-membership check identical to
// what coverage uses (exported so tests can brute-force validate the sweep).
export function arcContainsPhase(bracket, periodMs, clockSource, phaseMs) {
  return arcContains(bracketArc(bracket, periodMs, clockSource), phaseMs, periodMs);
}
