// Client vs server clock analysis.
//
// serverDate has 1 s precision (truncated). We compare the server second's
// midpoint (serverDateMs + 500) against the client request midpoint; the
// result mixes true clock offset with request latency and truncation, which
// is why it is reported as a distribution and never used to "correct" data.

import { SERVER_DATE_REGRESSION_CAVEAT_MS } from "./phase.mjs";

function nearestRank(sortedValues, p) {
  // Nearest-rank percentile on a sorted array (p in [0,1]).
  const n = sortedValues.length;
  const rank = Math.max(1, Math.ceil(p * n));
  return sortedValues[rank - 1];
}

// samplesByStream: array of sample arrays, each one (inputId, targetId) stream
// in sequence order (regressions are only meaningful within a stream).
export function analyzeClock(samplesByStream) {
  const offsets = [];
  let serverDateMissingCount = 0;
  let serverDateRegressions = 0;

  for (const stream of samplesByStream) {
    let prevServerMs = null;
    for (const sample of stream) {
      if (sample.serverDateMs === null) {
        serverDateMissingCount += 1;
        continue;
      }
      const clientMidMs = Math.round((sample.clientStartMs + sample.clientEndMs) / 2);
      offsets.push(sample.serverDateMs + 500 - clientMidMs);
      if (
        prevServerMs !== null &&
        prevServerMs - sample.serverDateMs > SERVER_DATE_REGRESSION_CAVEAT_MS
      ) {
        serverDateRegressions += 1;
      }
      prevServerMs = sample.serverDateMs;
    }
  }

  if (offsets.length === 0) {
    return {
      status: "unknown",
      offsetDistribution: null,
      serverDateRegressions: 0,
      serverDateMissingCount,
    };
  }

  offsets.sort((a, b) => a - b);
  return {
    status: "server-date-available",
    offsetDistribution: {
      sampleCount: offsets.length,
      minMs: offsets[0],
      p50Ms: nearestRank(offsets, 0.5),
      p95Ms: nearestRank(offsets, 0.95),
      maxMs: offsets[offsets.length - 1],
    },
    serverDateRegressions,
    serverDateMissingCount,
  };
}
