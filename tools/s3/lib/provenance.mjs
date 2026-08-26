// Observation-data provenance: separates target identity (metadata: campus /
// year / term labels from run.json or SQLite target ids) from a fingerprint of
// the observation data itself. A4-1 must count independent DATA, not labels:
// the same capture copied into three run directories with three different
// campus labels is one observation series, not three independent targets.
//
// Canonical series (per included sample, metadata-free):
//   bodySha \t deltaMs
// where deltaMs is the client-clock step from the previous included sample
// (first line 0). Campus/target labels, input ids, sample ids, sequence
// numbers, run.json content — and BOTH clock columns' absolute values — are
// deliberately excluded. serverDates are not part of the merge key AT ALL:
// neither their values nor their missing-pattern. The sixth review round
// showed why the missing-pattern must not be keyed either: with "-" vs number
// in the canonical text, deleting a SINGLE serverDate field from a byte-copy
// minted a fresh fingerprint that neither matched nor was contained — the
// copy landed in its own "independent" class and the time-anchor check never
// ran, resurrecting copy-and-relabel with a one-field deletion. Keying on
// body content plus client-delta structure alone makes the fingerprint
// invariant to any constant translation of either clock column (by hours or
// by a single millisecond) AND to any edit or deletion of serverDates: every
// such copy MERGES into the genuine capture's class, where the record
// agreement check below sees the disagreement.
//
// Equivalence (one provenance class):
//   - identical: canonical series byte-equal, or
//   - contained: after normalizing its first delta to 0, the whole series is
//     a CONTIGUOUS slice of the class representative's series
//     (prefix/suffix/middle truncations of a copy).
// Known boundary (by design, documented in the README): derived series that
// edit the BODY content or the CLIENT delta structure — every-other-sample
// subsampling, interleaving/reordering, edited client deltas, invented or
// removed samples — are NOT detected as equivalent. This gate defends against
// copy-and-relabel and copy-and-edit of the clock columns (translations of
// either clock, serverDate edits, serverDate deletions); it is not forensics
// against de novo fabrication of observation content.
//
// Time-anchor conflict (fail-closed): the canonical series carries no
// absolute clock and no server column, so members of one class can still
// DISAGREE about WHEN the shared observation data happened or what the
// server's clock said. Genuine duplicates of one capture (byte copies,
// SQLite re-ingests, truncations) carry the exact recorded fields, so at the
// aligned samples they agree exactly on the absolute client clock AND on
// every recorded serverDate — value and presence alike. A member is
// time-aligned with its representative only when, at some content-matching
// alignment offset, (a) its first sample's clientStartMs equals the
// representative's clientStartMs there, AND (b) at every aligned sample the
// two serverDates are exactly equal or both absent — a different value, a
// deletion, or an insertion on either side is a disagreement. When any member
// fails both checks at every offset, no deterministic choice among the
// conflicting records is safe — the class is flagged timeConflict and the
// analyzer excludes EVERY member from all evidence, exactly like a campus
// conflict. Tolerance is 0 ms on purpose: both ingest paths parse the same
// recorded timestamp strings, so any disagreement means the series was
// edited.

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

function fingerprintOf(entries) {
  return sha256Text(entries.map((e) => `${e.bodySha}\t${e.deltaMs}`).join("\n"));
}

// serverDates agree exactly — same presence and, when present, same value —
// at every aligned sample of A against B starting at `offset`. rawServerDelta
// is serverDateMs - clientStartMs; the caller only consults this where the
// absolute client clocks already agree at the aligned samples, so equal
// deltas mean equal absolute serverDates (null === null covers agreement on
// absence, null vs number is a presence disagreement).
function serverDatesAgree(a, b, offset) {
  for (let k = 0; k < a.length; k += 1) {
    if (a[k].rawServerDelta !== b[offset + k].rawServerDelta) return false;
  }
  return true;
}

// All alignment offsets at which series A's CONTENT (bodySha + client-delta
// structure, first delta of the slice normalized to 0, matching A's own first
// delta of 0) is a contiguous slice of series B. serverDates play no role in
// the match — a copy whose server column was edited or emptied still aligns
// here and is then judged by the record agreement check. Every offset is
// returned (not just the first) so the time-anchor check can accept a genuine
// truncation whose content happens to align at more than one position: it
// agrees in absolute time at its true offset.
function containmentAlignments(a, b) {
  const offsets = [];
  if (a.length === 0 || a.length > b.length) return offsets;
  for (let i = 0; i + a.length <= b.length; i += 1) {
    let match = true;
    for (let k = 0; k < a.length; k += 1) {
      const x = a[k];
      const y = b[i + k];
      if (x.bodySha !== y.bodySha) {
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
      // Absolute anchors (NOT part of the fingerprint): used only for the
      // time-anchor agreement check between class members.
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
          // Identical fingerprints share the whole content series, so the
          // member is time-aligned iff the first sample's absolute client
          // time matches AND every serverDate agrees exactly (value and
          // presence; vacuously true when neither series carries a serverDate
          // anywhere). A copy that translated a clock, edited a serverDate,
          // or deleted one fails here and conflicts the class.
          timeAligned:
            stream.firstStartMs === cls.representative.firstStartMs &&
            serverDatesAgree(stream.entries, cls.representative.entries, 0),
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
          // client clock and recorded serverDates at (at least) its true
          // alignment offset; a copy translated in either clock column or
          // with an edited/deleted serverDate agrees at none.
          timeAligned: alignments.some(
            (offset) =>
              cls.representative.startTimes[offset] === stream.firstStartMs &&
              serverDatesAgree(stream.entries, cls.representative.entries, offset),
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
      // observed or what the server's clock recorded: no representative
      // choice among the records is safe, so the analyzer excludes every
      // member from evidence (like campusConflict).
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
