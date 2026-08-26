// Observation-data provenance: separates target identity (metadata: campus /
// year / term labels from run.json or SQLite target ids) from a fingerprint of
// the observation data itself. A4-1 must count independent DATA, not labels:
// the same capture copied into three run directories with three different
// campus labels is one observation series, not three independent targets.
//
// Canonical series (per included sample, metadata-free):
//   bodySha \t deltaMs \t serverDeltaNorm
// where deltaMs is the client-clock step from the previous included sample
// (first line 0) and serverDeltaNorm is the per-sample server-vs-client delta
// (serverDateMs - clientStartMs) RE-BASED to the series' first sample that has
// a serverDate ("-" when serverDate is absent). Campus/target labels, input
// ids, sample ids, sequence numbers, and run.json content are all deliberately
// excluded.
//
// The canonical form is invariant to a CONSTANT shift of EITHER clock column
// (and hence to a joint shift of both): deltaMs already ignores a constant
// client shift, and re-basing serverDelta ignores the constant it picks up
// when only one column moves. This is deliberate: a copy of a capture whose
// client timestamps were translated (serverDate untouched), whose serverDates
// were translated (client untouched), or which was jointly translated — by
// hours or by a single millisecond — still MERGES into the genuine capture's
// class, where the time-anchor check below can see the disagreement. Keying
// on the raw per-sample serverDelta instead would let a 1 ms client-side
// nudge mint a fresh fingerprint and escape the class entirely (the fifth
// review round's false GO).
//
// Equivalence (one provenance class):
//   - identical: canonical series byte-equal, or
//   - contained: after normalizing its first delta to 0 and re-basing its
//     serverDelta within the compared slice, the whole series is a CONTIGUOUS
//     slice of the class representative's series (prefix/suffix/middle
//     truncations of a copy).
// Known boundary (by design, documented in the README): derived series such as
// every-other-sample subsampling, interleaving/reordering, or any edit of the
// delta structure are NOT detected as equivalent — this gate defends against
// copy-and-relabel and copy-and-translate, it is not forensics against de novo
// fabrication.
//
// Time-anchor conflict (fail-closed): the canonical series is deliberately
// translation-invariant, so members of one class can still DISAGREE about
// WHEN the shared observation data happened — e.g. a byte-copy of an off-peak
// capture shifted into the 17:00-18:00 ET peak hour, whether the shift moved
// both clock columns or only one. Genuine duplicates of one capture (byte
// copies, SQLite re-ingests, truncations) carry the exact recorded timestamp
// strings, so at the aligned samples they agree exactly on BOTH absolute
// clocks. A member is time-aligned with its representative only when, at some
// content-matching alignment offset, (a) its first sample's clientStartMs
// equals the representative's clientStartMs there, AND (b) its per-sample
// serverDelta equals the representative's at the aligned positions (i.e. the
// re-basing constant is zero — the absolute serverDates agree too; vacuous
// when the member carries no serverDate at all). When any member fails both
// checks at every offset, no deterministic choice among the conflicting
// timelines is safe — the class is flagged timeConflict and the analyzer
// excludes EVERY member from all evidence, exactly like a campus conflict.
// Tolerance is 0 ms on purpose: both ingest paths parse the same recorded
// timestamp strings, so any disagreement means the series was translated.

import { sha256Text } from "./stable.mjs";

// entries[k]: { bodySha, deltaMs, rawServerDelta: number|null }.
function canonicalEntries(samples) {
  const entries = [];
  let prevStartMs = null;
  for (const s of samples) {
    entries.push({
      bodySha: s.bodySha,
      deltaMs: prevStartMs === null ? 0 : s.clientStartMs - prevStartMs,
      rawServerDelta: s.serverDateMs === null ? null : s.serverDateMs - s.clientStartMs,
    });
    prevStartMs = s.clientStartMs;
  }
  return entries;
}

// First non-null rawServerDelta (the re-basing constant), or null when the
// series carries no serverDate at all.
function serverBaseOf(entries) {
  for (const e of entries) {
    if (e.rawServerDelta !== null) return e.rawServerDelta;
  }
  return null;
}

function fingerprintOf(entries) {
  const base = serverBaseOf(entries);
  return sha256Text(
    entries
      .map(
        (e) =>
          `${e.bodySha}\t${e.deltaMs}\t${e.rawServerDelta === null ? "-" : String(e.rawServerDelta - base)}`,
      )
      .join("\n"),
  );
}

// All alignments at which series A is a contiguous slice of series B (first
// delta of the slice normalized to 0, matching A's own first delta of 0;
// serverDelta compared re-based within the slice, so a slice whose server
// column was shifted by a constant still matches — its non-zero shift is then
// caught by the time-anchor check). Every alignment is returned (not just the
// first) so the time-anchor check can accept a genuine truncation whose
// content happens to align at more than one position: it agrees in absolute
// time at its true offset. Each result: { offset, serverShiftMs } where
// serverShiftMs = B's rawServerDelta minus A's at the aligned samples (null
// when A has no serverDate anywhere — nothing recorded to compare).
function containmentAlignments(a, b) {
  const alignments = [];
  if (a.length === 0 || a.length > b.length) return alignments;
  for (let i = 0; i + a.length <= b.length; i += 1) {
    let match = true;
    let serverShiftMs = null;
    for (let k = 0; k < a.length; k += 1) {
      const x = a[k];
      const y = b[i + k];
      if (x.bodySha !== y.bodySha || (x.rawServerDelta === null) !== (y.rawServerDelta === null)) {
        match = false;
        break;
      }
      // The slice's first delta is normalized to 0 (= x.deltaMs by construction).
      if (k > 0 && x.deltaMs !== y.deltaMs) {
        match = false;
        break;
      }
      if (x.rawServerDelta !== null) {
        const shift = y.rawServerDelta - x.rawServerDelta;
        if (serverShiftMs === null) {
          serverShiftMs = shift;
        } else if (shift !== serverShiftMs) {
          match = false;
          break;
        }
      }
    }
    if (match) alignments.push({ offset: i, serverShiftMs });
  }
  return alignments;
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
      // Absolute anchors (NOT part of the fingerprint): used only for the
      // time-anchor agreement check between class members.
      startTimes: stream.samples.map((s) => s.clientStartMs),
      firstStartMs: first.clientStartMs,
      serverBase: serverBaseOf(entries),
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
          // Identical fingerprints share the whole delta series, so the
          // member is time-aligned iff the first sample's absolute client
          // time matches AND the serverDelta re-basing constants match (the
          // absolute serverDates then agree everywhere; vacuously true when
          // neither series carries a serverDate — the fingerprint's "-"
          // pattern guarantees both sides agree on absence).
          timeAligned:
            stream.firstStartMs === cls.representative.firstStartMs &&
            stream.serverBase === cls.representative.serverBase,
        });
        placed = true;
        break;
      }
      const alignments = containmentAlignments(stream.entries, cls.representative.entries);
      if (alignments.length > 0) {
        cls.members.push({
          stream,
          relation: "contained",
          // A genuine truncation agrees with the representative's absolute
          // clocks at (at least) its true alignment offset; a copy translated
          // in either clock column agrees at none.
          timeAligned: alignments.some(
            (al) =>
              cls.representative.startTimes[al.offset] === stream.firstStartMs &&
              (al.serverShiftMs === null || al.serverShiftMs === 0),
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
