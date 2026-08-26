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
// series and therefore still merges.
//
// Time-anchor conflict (fail-closed): the canonical series is deliberately
// translation-invariant, so members of one class can still DISAGREE about
// WHEN the shared observation data happened — e.g. a byte-copy of an off-peak
// capture shifted by a whole number of seconds into the 17:00-18:00 ET peak
// hour. Genuine duplicates of one capture (byte copies, SQLite re-ingests,
// truncations) carry the exact recorded timestamps, so their absolute
// client-clock times agree exactly at the aligned samples. When any member's
// time anchor (clientStartMs of its first sample, compared at the alignment
// offset inside the representative's series) differs from the
// representative's, no deterministic choice among the conflicting timelines
// is safe — the class is flagged timeConflict and the analyzer excludes EVERY
// member from all evidence, exactly like a campus conflict. Tolerance is 0 ms
// on purpose: both ingest paths parse the same recorded timestamp strings, so
// any disagreement means the series was translated.

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

// All offsets at which series A is a contiguous slice of series B (first
// delta of the slice normalized to 0, matching A's own first delta of 0).
// Every offset is returned (not just the first) so the time-anchor check can
// accept a genuine truncation whose content happens to align at more than one
// position: it agrees in absolute time at its true offset.
function containmentOffsets(a, b) {
  const offsets = [];
  if (a.length === 0 || a.length > b.length) return offsets;
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
    if (match) offsets.push(i);
  }
  return offsets;
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
      // Absolute client-clock times (NOT part of the fingerprint): used only
      // for the time-anchor agreement check between class members.
      startTimes: stream.samples.map((s) => s.clientStartMs),
      firstStartMs: first.clientStartMs,
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
        cls.members.push({
          stream,
          relation: "identical",
          // Identical fingerprints share the whole delta series, so agreement
          // of the first sample's absolute time means every sample agrees.
          timeAligned: stream.firstStartMs === cls.representative.firstStartMs,
        });
        placed = true;
        break;
      }
      const offsets = containmentOffsets(stream.entries, cls.representative.entries);
      if (offsets.length > 0) {
        cls.members.push({
          stream,
          relation: "contained",
          // A genuine truncation agrees with the representative's absolute
          // time at (at least) its true alignment offset; a time-translated
          // copy agrees at none.
          timeAligned: offsets.some(
            (i) => cls.representative.startTimes[i] === stream.firstStartMs,
          ),
        });
        placed = true;
        break;
      }
    }
    if (!placed) {
      classes.push({
        representative: stream,
        members: [{ stream, relation: "representative", timeAligned: true }],
      });
    }
  }

  const classesOut = classes
    .map((cls) => {
      const campuses = [
        ...new Set(cls.members.map((m) => m.stream.campus).filter((c) => c !== null)),
      ];
      const campusConflict = campuses.length > 1;
      // Members that share canonical content but disagree about WHEN it was
      // observed: no representative choice among the timelines is safe, so
      // the analyzer excludes every member from evidence (like campusConflict).
      const timeConflict = cls.members.some((m) => m.timeAligned === false);
      return {
        classId: `pc-${cls.representative.seriesFingerprint.slice(0, 12)}`,
        campus: campusConflict || campuses.length === 0 ? null : campuses[0],
        campusConflict,
        timeConflict,
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
