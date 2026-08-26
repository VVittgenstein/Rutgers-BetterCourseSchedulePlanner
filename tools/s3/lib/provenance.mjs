// Observation-data provenance: separates target identity (metadata: campus /
// year / term labels from run.json or SQLite target ids) from a fingerprint of
// the observation data itself. A4-1 must count independent DATA, not labels:
// the same capture copied into three run directories with three different
// campus labels is one observation series, not three independent targets.
//
// Canonical series (per included sample, metadata-free):
//   bodySha \t deltaMs \t serverDeltaMs
// where deltaMs is the client-clock step from the previous included sample
// (first line 0) and serverDeltaMs = serverDateMs - clientStartMs ("-" when
// serverDate is absent). Campus/target labels, input ids, sample ids,
// sequence numbers, and run.json content are all deliberately excluded.
//
// Equivalence (one provenance class):
//   - identical: canonical series byte-equal, or
//   - contained: after normalizing its first delta to 0, the whole series is
//     a CONTIGUOUS slice of the class representative's series (prefix/suffix/
//     middle truncations of a copy).
// Known boundary (by design, documented in the README): derived series such as
// every-other-sample subsampling, interleaving/reordering, or any edit of the
// delta structure are NOT detected as equivalent — this gate defends against
// copy-and-relabel, it is not forensics against arbitrary fabrication. A pure
// joint time translation of client and server clocks preserves the canonical
// series and therefore still merges (fail-closed: fewer independent classes).

import { sha256Text } from "./stable.mjs";

function canonicalEntries(samples) {
  const entries = [];
  let prevStartMs = null;
  for (const s of samples) {
    entries.push({
      bodySha: s.bodySha,
      deltaMs: prevStartMs === null ? 0 : s.clientStartMs - prevStartMs,
      serverDelta: s.serverDateMs === null ? "-" : String(s.serverDateMs - s.clientStartMs),
    });
    prevStartMs = s.clientStartMs;
  }
  return entries;
}

function fingerprintOf(entries) {
  return sha256Text(entries.map((e) => `${e.bodySha}\t${e.deltaMs}\t${e.serverDelta}`).join("\n"));
}

// Is series A a contiguous slice of series B (first delta of the slice
// normalized to 0, matching A's own first delta of 0)?
function isContainedIn(a, b) {
  if (a.length === 0 || a.length > b.length) return false;
  for (let i = 0; i + a.length <= b.length; i += 1) {
    let match = true;
    for (let k = 0; k < a.length; k += 1) {
      const x = a[k];
      const y = b[i + k];
      if (x.bodySha !== y.bodySha || x.serverDelta !== y.serverDelta) {
        match = false;
        break;
      }
      // The slice's first delta is normalized to 0 (= x.deltaMs by construction).
      if (k > 0 && x.deltaMs !== y.deltaMs) {
        match = false;
        break;
      }
    }
    if (match) return true;
  }
  return false;
}

function cmpStr(a, b) {
  return a < b ? -1 : a > b ? 1 : 0;
}

// streams: [{ inputId, samples }] as built by the CLI (one per (input, target)
// pair; NDJSON inputs carry one stream). Empty streams carry no observation
// data and are skipped. targetId/campus are read from the samples themselves.
export function buildProvenance(streams) {
  const enriched = [];
  for (const stream of streams) {
    if (stream.samples.length === 0) continue;
    const first = stream.samples[0];
    const entries = canonicalEntries(stream.samples);
    enriched.push({
      streamId: `${stream.inputId}::${first.targetId}`,
      targetId: first.targetId,
      campus: first.campus,
      sampleCount: stream.samples.length,
      entries,
      seriesFingerprint: fingerprintOf(entries),
    });
  }

  // Longest-first so every class representative is at least as long as any
  // later member; ties broken by streamId for determinism.
  const ordered = [...enriched].sort(
    (a, b) => b.sampleCount - a.sampleCount || cmpStr(a.streamId, b.streamId),
  );

  const classes = [];
  for (const stream of ordered) {
    let placed = false;
    for (const cls of classes) {
      if (stream.seriesFingerprint === cls.representative.seriesFingerprint) {
        cls.members.push({ stream, relation: "identical" });
        placed = true;
        break;
      }
      if (isContainedIn(stream.entries, cls.representative.entries)) {
        cls.members.push({ stream, relation: "contained" });
        placed = true;
        break;
      }
    }
    if (!placed) {
      classes.push({
        representative: stream,
        members: [{ stream, relation: "representative" }],
      });
    }
  }

  const classesOut = classes
    .map((cls) => {
      const campuses = [
        ...new Set(cls.members.map((m) => m.stream.campus).filter((c) => c !== null)),
      ];
      const campusConflict = campuses.length > 1;
      return {
        classId: `pc-${cls.representative.seriesFingerprint.slice(0, 12)}`,
        campus: campusConflict || campuses.length === 0 ? null : campuses[0],
        campusConflict,
        members: [...cls.members]
          .sort(
            (a, b) =>
              (a.relation === "representative" ? 0 : 1) - (b.relation === "representative" ? 0 : 1) ||
              cmpStr(a.stream.streamId, b.stream.streamId),
          )
          .map((m) => ({ streamId: m.stream.streamId, relation: m.relation })),
      };
    })
    .sort((a, b) => cmpStr(a.classId, b.classId));

  const streamsOut = enriched
    .map((s) => ({
      streamId: s.streamId,
      targetId: s.targetId,
      campus: s.campus,
      sampleCount: s.sampleCount,
      seriesFingerprint: s.seriesFingerprint,
    }))
    .sort((a, b) => cmpStr(a.streamId, b.streamId));

  return { streams: streamsOut, classes: classesOut };
}
