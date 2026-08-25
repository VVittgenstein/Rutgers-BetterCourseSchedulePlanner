// Change-bracket construction for NDJSON and SQLite sample streams.
//
// Semantics (half-open (lower, upper]): the server generated the stable body
// at some instant >= its Date-header second start and >= the client request
// start; the change happened strictly AFTER that instant and AT/BEFORE the
// changed body's generation instant, which is < its Date + 1 s and <= the
// client request end. Server upper bounds therefore get +SERVER_DATE_WIDEN_MS
// to cover the 1 s Date truncation; client bounds use the conservative outer
// envelope (stable requestStart, changed requestEnd].
//
// etag is deliberately never consulted: the same body maps to multiple etags
// on this webfarm, so etag is not a change signal (the field is not even
// carried into normalized samples).

import { SERVER_DATE_WIDEN_MS, PERIODS_MS, bracketWidthMs } from "./phase.mjs";

// Returns a bracket, or null when the pair is rejected (counted in counters).
function makeBracket(window, targetId, stable, changed, counters) {
  if (changed.clientEndMs - stable.clientStartMs <= 0) {
    // Non-positive client/observed_at width: the capture's own row ordering is
    // corrupt for this pair (a tolerated client-clock step, a corrupted
    // requestEndedUtc, or a SQLite observed_at tie/regression within the
    // ingestion tolerance). The pairing itself is untrustworthy, so the whole
    // bracket is rejected — the server bounds derive from the same suspect
    // pair and are not kept either. Counted, never silently credited as
    // informative coverage (mirrors the serverNonPositiveWidth treatment).
    counters.clientNonPositiveWidth += 1;
    return null;
  }
  let serverLowerMs = stable.serverDateMs;
  let serverUpperMs = changed.serverDateMs !== null ? changed.serverDateMs + SERVER_DATE_WIDEN_MS : null;
  if (serverLowerMs === null || serverUpperMs === null) {
    serverLowerMs = null;
    serverUpperMs = null;
  } else if (serverUpperMs - serverLowerMs <= 0) {
    // Webfarm clock skew produced a degenerate server bracket; keep client bounds.
    counters.serverNonPositiveWidth += 1;
    serverLowerMs = null;
    serverUpperMs = null;
  }
  const bracket = {
    bracketId: `${targetId}/${window.windowId}/#${changed.seq}`,
    windowId: window.windowId,
    targetId,
    stableSeq: stable.seq,
    changedSeq: changed.seq,
    clientLowerMs: stable.clientStartMs,
    clientUpperMs: changed.clientEndMs,
    serverLowerMs,
    serverUpperMs,
  };
  bracket.clientWidthMs = bracket.clientUpperMs - bracket.clientLowerMs;
  bracket.serverWidthMs =
    serverLowerMs !== null && serverUpperMs !== null ? serverUpperMs - serverLowerMs : null;
  for (const periodMs of PERIODS_MS) {
    const key = periodMs / 1000;
    const sw = bracketWidthMs(bracket, "server");
    const cw = bracketWidthMs(bracket, "client");
    bracket[`informative${key}Server`] = sw !== null && sw < periodMs;
    bracket[`informative${key}Client`] = cw !== null && cw < periodMs;
  }
  if (stable.ageSeconds !== null && stable.ageSeconds > 0) counters.ageGreaterThanZeroEndpoints += 1;
  if (changed.ageSeconds !== null && changed.ageSeconds > 0) counters.ageGreaterThanZeroEndpoints += 1;
  counters.changeCount += 1;
  return bracket;
}

// Builds change brackets for one window.
// sourceKind: "ndjson" (adjacent decodedBodySha256 diff over included rows;
// first row is the LKG baseline) or "sqlite" (body_changed flags; the caller
// has already dropped the per-target initial LKG row).
export function buildBrackets(window, sourceKind) {
  const counters = {
    changeCount: 0,
    serverNonPositiveWidth: 0,
    clientNonPositiveWidth: 0,
    noPriorStable: 0,
    ageGreaterThanZeroEndpoints: 0,
  };
  const brackets = [];
  const samples = window.samples;
  if (sourceKind === "ndjson") {
    for (let i = 1; i < samples.length; i += 1) {
      const a = samples[i - 1];
      const b = samples[i];
      if (b.bodySha !== a.bodySha) {
        const bracket = makeBracket(window, b.targetId, a, b, counters);
        if (bracket !== null) brackets.push(bracket);
      }
    }
  } else if (sourceKind === "sqlite") {
    for (let i = 0; i < samples.length; i += 1) {
      const row = samples[i];
      if (row.bodyChangedFlag !== 1) continue;
      if (i === 0) {
        // change row with no preceding stable row inside this window
        counters.noPriorStable += 1;
        continue;
      }
      const bracket = makeBracket(window, row.targetId, samples[i - 1], row, counters);
      if (bracket !== null) brackets.push(bracket);
    }
  }
  return { brackets, counters };
}
