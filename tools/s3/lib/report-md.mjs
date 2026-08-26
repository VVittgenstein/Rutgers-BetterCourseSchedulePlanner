// Human Markdown report assembly. Deterministic: content derives only from
// analysis results (no timestamps, no absolute paths, no hostnames).

import { isoUtcMs } from "./stable.mjs";

function cmpStr(a, b) {
  return a < b ? -1 : a > b ? 1 : 0;
}

function fmtOffset(ms) {
  const totalSeconds = Math.floor(ms / 1000);
  const mm = String(Math.floor(totalSeconds / 60)).padStart(2, "0");
  const ss = String(totalSeconds % 60).padStart(2, "0");
  const mmm = String(ms % 1000).padStart(3, "0");
  return `${mm}:${ss}.${mmm}`;
}

function fmtIntervals(intervals) {
  if (intervals.length === 0) return "—";
  return intervals.map((iv) => `[${fmtOffset(iv.startMs)}, ${fmtOffset(iv.endMs)}]`).join(", ");
}

function fmtSeconds(ms) {
  return (ms / 1000).toFixed(3);
}

function widthStats(values) {
  if (values.length === 0) return null;
  const sorted = [...values].sort((a, b) => a - b);
  const p50 = sorted[Math.max(0, Math.ceil(0.5 * sorted.length) - 1)];
  return { min: sorted[0], p50, max: sorted[sorted.length - 1] };
}

export function buildMdReport(ctx) {
  const {
    normalizedCommand,
    inputs,
    targets,
    windowsAll,
    brackets,
    bracketTotals,
    fits,
    comparison,
    clockSource,
    clockFallback,
    clock,
    serverEvidence,
    safeOffset,
    goGate,
    decision,
  } = ctx;

  const lines = [];
  const push = (s = "") => lines.push(s);

  // 1. Headline + summary (rendered from decision, never hardcoded).
  const headline = decision.qualifier
    ? `Verdict: ${decision.verdict} (${decision.qualifier})`
    : `Verdict: ${decision.verdict}`;
  push(`# S3 Rebuild Cadence Evidence — ${headline}`);
  push();
  const reasonsPart =
    decision.reasons.length > 0
      ? ` Unsatisfied gates: ${decision.reasons.map((r) => r.split(" unsatisfied:")[0]).join(", ")}.`
      : "";
  push(
    `This offline analysis of locally captured openSections observation data (kept in the gitignored \`data/\` root; only these evidence documents are committed) reaches the verdict **${decision.verdict}**${
      decision.qualifier ? ` with qualifier **${decision.qualifier}**` : ""
    } from the A4 gate evaluation below.${reasonsPart} All numbers are computed from interval-censored change brackets; no production behavior is changed by this lane.`,
  );
  if (decision.reasons.length > 0) {
    push();
    for (const reason of decision.reasons) {
      push(`- ${reason}`);
    }
  }
  push();

  // 2. Data overview.
  push(`## Data overview`);
  push();
  push(`| input | kind | rows | excluded | sha256 | target | windows |`);
  push(`|---|---|---:|---:|---|---|---:|`);
  const sortedInputs = [...inputs].sort((a, b) => cmpStr(a.kind, b.kind) || cmpStr(a.id, b.id));
  for (const inp of sortedInputs) {
    const excludedTotal =
      inp.excluded.curlExitCode +
      inp.excluded.httpStatusNon2xx +
      inp.excluded.validationErrors +
      inp.excluded.initialLkg;
    const inputTargets = targets.filter((t) => t.windows.some((w) => w.windowId.startsWith(`${inp.id}#`)));
    const targetNames = inputTargets.map((t) => t.targetId).sort().join(", ") || "—";
    const windowCount = inputTargets.reduce(
      (acc, t) => acc + t.windows.filter((w) => w.windowId.startsWith(`${inp.id}#`)).length,
      0,
    );
    push(
      `| ${inp.id} | ${inp.kind} | ${inp.rows} | ${excludedTotal} | \`${inp.sha256.slice(0, 12)}…\` | ${targetNames} | ${windowCount} |`,
    );
  }
  push();

  // 3. Windows.
  push(`## Windows`);
  push();
  push(`| windowId | UTC range | America/New_York | peak 17–18 ET? | samples | brackets |`);
  push(`|---|---|---|---|---:|---:|`);
  for (const win of [...windowsAll].sort((a, b) => cmpStr(a.windowId, b.windowId))) {
    push(
      `| ${win.windowId} | ${isoUtcMs(win.utcStartMs)} – ${isoUtcMs(win.utcEndMs)} | ${win.nyLabel} | ${
        win.peakOverlap ? "yes" : "no"
      } | ${win.samples.length} | ${win.brackets.length} |`,
    );
  }
  push();

  // 4. Bracket statistics.
  push(`## Bracket statistics`);
  push();
  push(`- Total change brackets: ${bracketTotals.total}`);
  push(
    `- Excluded from all evidence (campus-conflicted provenance, see \`provenance.excludedStreamIds\`): ${bracketTotals.excludedFromEvidence}`,
  );
  push(
    `- Informative (0 < width < period, ${clockSource} clock — the comparison clock): 30 s → ${bracketTotals.informative30}, 60 s → ${bracketTotals.informative60}; non-informative: 30 s → ${bracketTotals.nonInformative30}, 60 s → ${bracketTotals.nonInformative60}`,
  );
  push(
    `- Server brackets dropped for non-positive width (webfarm clock skew): ${bracketTotals.serverNonPositiveWidth}; brackets rejected entirely for non-positive client/observed_at width (corrupt capture ordering): ${bracketTotals.clientNonPositiveWidth}; change rows without a prior stable row: ${bracketTotals.noPriorStable}; bracket endpoints with \`age > 0\`: ${bracketTotals.ageGreaterThanZeroEndpoints}`,
  );
  const clientWidths = brackets.map((b) => b.clientWidthMs);
  const serverWidths = brackets.filter((b) => b.serverWidthMs !== null).map((b) => b.serverWidthMs);
  const cw = widthStats(clientWidths);
  const sw = widthStats(serverWidths);
  if (cw) {
    push(
      `- Client-clock width (s): min ${fmtSeconds(cw.min)} / p50 ${fmtSeconds(cw.p50)} / max ${fmtSeconds(cw.max)}`,
    );
  }
  if (sw) {
    push(
      `- Server-clock width incl. +1 s Date widening (s): min ${fmtSeconds(sw.min)} / p50 ${fmtSeconds(sw.p50)} / max ${fmtSeconds(sw.max)}`,
    );
  }
  push();
  push(
    `A bracket whose width reaches or exceeds a candidate period covers the whole phase circle for that period: it is **non-informative** and is excluded from coverage counting rather than silently counted as explained.`,
  );
  push();

  // 5. Model comparison.
  push(`## Model comparison`);
  push();
  push(`| model | clock | informative | non-informative | unusable | max coverage | ratio | best phase interval(s) |`);
  push(`|---|---|---:|---:|---:|---:|---:|---|`);
  for (const modelKey of ["m30", "m60"]) {
    for (const clockKey of ["server", "client"]) {
      const fit = fits[modelKey][clockKey];
      if (fit === null || clock.status === "unknown" && clockKey === "server") {
        push(`| ${modelKey} | ${clockKey} | — | — | — | — | — | (no server Date available) |`);
        continue;
      }
      push(
        `| ${modelKey} | ${clockKey} | ${fit.informativeCount} | ${fit.nonInformativeCount} | ${fit.unusableCount} | ${fit.maxCoverage} | ${
          fit.coverageRatio === null ? "—" : fit.coverageRatio
        } | ${fmtIntervals(fit.bestPhaseIntervals)} |`,
      );
    }
  }
  push();
  push(
    `Comparison on the ${comparison.commonInformativeCount} brackets informative for both periods (${comparison.clockSource} clock${clockFallback ? ", client fallback" : ""}): max coverage 30 s = **${comparison.maxCoverage30Common}**, 60 s = **${comparison.maxCoverage60Common}**.`,
  );
  push();
  push(
    `max-coverage(30) ≥ max-coverage(60) holds identically — the ticks of any 60 s grid at phase φ are a subset of the 30 s grid's ticks at phase φ mod 30 — so equality can never select a winner: it only shows the 30 s grid adds no explanatory power, which is *consistent with* a true 60 s period but proves neither model. Holdout mode: **${comparison.holdout.mode}**${
      comparison.holdout.degenerate
        ? " (degenerate single-group half-split; NOT multi-window validation)"
        : ""
    }. Result: distinguishable = **${comparison.distinguishable}**, winner = **${comparison.winner ?? "none"}**, reason = \`${comparison.reason}\`.`,
  );
  push();

  // 5b. Stability (A4-6): the three checks are reported separately — a
  // (target, window) group is never presented as a target.
  push(`### Stability (A4-6)`);
  push();
  const st = comparison.stability;
  if (st === null) {
    push(`Not evaluable: no distinguishable winner.`);
  } else {
    const foldStr = (f) => `held-out ${f.heldOut} → ${f.distinguishable ? f.winner : f.reason}`;
    if (st.targets.degenerate) {
      push(
        `- **Whole-target leave-out**: degenerate — single target in the comparison set; NOT satisfied.`,
      );
    } else {
      push(
        `- **Whole-target leave-out** (${st.targets.count} folds): ${st.targets.pass ? "pass" : "FAIL"} — ${st.targets.folds.map(foldStr).join("; ")}`,
      );
    }
    if (st.groups.degenerate) {
      push(
        `- **Group leave-out**: degenerate — single (target, window) group in the comparison set; NOT satisfied.`,
      );
    } else {
      push(
        `- **Group leave-out** (${st.groups.count} folds): ${st.groups.pass ? "pass" : "FAIL"} — ${st.groups.folds.map(foldStr).join("; ")}`,
      );
    }
    if (st.outliers.residualCount === 0) {
      push(
        `- **Outlier sensitivity** (residual top-k): pass (vacuous) — no residual brackets under the winning fit; removal has nothing to act on.`,
      );
    } else {
      push(
        `- **Outlier sensitivity** (residual top-k, ${st.outliers.residualCount} residuals): ${st.outliers.pass ? "pass" : "FAIL"} — ${st.outliers.runs
          .map((r) => `k=${r.k} removed [${r.removedBracketIds.join(", ")}] → ${r.distinguishable ? r.winner : r.reason}`)
          .join("; ")}`,
      );
    }
  }
  push();

  // 6. A4 gate.
  push(`## A4 gate`);
  push();
  push(`| id | requirement | satisfied | evidence |`);
  push(`|---|---|---|---|`);
  for (const g of goGate) {
    push(`| ${g.id} | ${g.requirement} | ${g.satisfied ? "yes" : "no"} | ${g.evidence} |`);
  }
  push();

  // 7. Clock & caveats.
  push(`## Clock and caveats`);
  push();
  push(
    `- **Server \`Date\` precision**: 1 s (truncated). Every server-clock bracket upper bound is widened by +1 s so the true change instant is conservatively contained; widths quoted above include this widening.`,
  );
  if (clock.offsetDistribution) {
    const d = clock.offsetDistribution;
    push(
      `- **Client vs server offset** (server second midpoint minus client request midpoint, ${d.sampleCount} samples): min ${d.minMs} ms / p50 ${d.p50Ms} ms / p95 ${d.p95Ms} ms / max ${d.maxMs} ms. This mixes clock offset with request latency; it is reported, never used to correct timestamps.`,
    );
  } else {
    push(`- **Client vs server offset**: no server Date available; all phase results are client-clock only.`);
  }
  push(
    `- **Server Date regressions** (adjacent samples, > 1 s backwards): ${clock.serverDateRegressions}; samples missing serverDate: ${clock.serverDateMissingCount}. The webfarm rotates multiple backends (\`X-Server-Name\`), so small skew between backends is expected and is why non-positive-width server brackets are dropped (${bracketTotals.serverNonPositiveWidth} here).`,
  );
  push(
    `- **Server-clock evidence coverage**: sufficient = **${serverEvidence.sufficient}**${
      serverEvidence.reason !== null ? ` (reason: \`${serverEvidence.reason}\`)` : ""
    }; server-informative comparison brackets: ${serverEvidence.serverCommonCount}; qualifying groups with server evidence: ${serverEvidence.groupsWithServer}/${serverEvidence.groupsTotal}. Production GO and any safe offset require the comparison itself to run on the server clock with server brackets covering every qualifying group; a client-clock fallback fails these gates closed.`,
  );
  push(
    `- **Caching**: \`cache-control: max-age=30\` means an intermediary cache could quantize observations; bracket endpoints with \`age > 0\`: ${bracketTotals.ageGreaterThanZeroEndpoints}.`,
  );
  push(
    `- **etag is not a change signal**: the same body is served with multiple etags across backends; change detection uses \`decodedBodySha256\` only, and etag is never consulted.`,
  );
  push(
    `- **Timestamp semantics**: NDJSON rows carry client-side \`requestStartedUtc\`/\`requestEndedUtc\` (bracket = (stable requestStart, changed requestEnd]); SQLite rows carry a single \`observed_at\`, used for both bracket endpoints, so SQLite client-clock brackets are narrower than the true envelope by up to one request duration. Client/observed_at timestamps must be monotone per target within a 2 s tolerance (fail-closed beyond it); a change pair whose client width is still non-positive indicates corrupt capture ordering and is rejected entirely and counted (${bracketTotals.clientNonPositiveWidth} here), never treated as informative.`,
  );
  push(
    `- **Method**: brackets are interval-censored arcs on the period circle; the best phase is the maximum arc-coverage region. A \`timestamp % period\` histogram of detection times is invalid for this purpose (it confounds sampling cadence with change phase) and is not used.`,
  );
  push();

  // 8. Reproduction.
  push(`## Reproduction`);
  push();
  push(
    `Offline, zero-dependency (Node ≥ 24). \`<DATA_ROOT>\` is the gitignored capture root \`data/open-sections-repro\`; raw sample data is never committed.`,
  );
  push();
  push("```");
  const repro = ["node tools/s3/rebuild-profile-analyzer.mjs"];
  for (const inp of sortedInputs) {
    if (inp.kind === "ndjson") {
      const runDir = inp.id.endsWith("/samples.ndjson")
        ? inp.id.slice(0, -"/samples.ndjson".length)
        : inp.id;
      repro.push(`  --ndjson <DATA_ROOT>/${runDir}`);
    } else {
      repro.push(`  --sqlite <DATA_ROOT>/${inp.id}`);
    }
  }
  repro.push(`  --out-json docs/evidence/S3-REBUILD-PROFILE.json`);
  repro.push(`  --out-md docs/evidence/S3-REBUILD-PROFILE.md`);
  push(repro.join(" \\\n"));
  push("```");
  push();
  push(`Normalized command (order-independent): \`${normalizedCommand}\``);
  push();

  // 9. Input fingerprints.
  push(`## Input fingerprints`);
  push();
  push(`| input | kind | sha256 |`);
  push(`|---|---|---|`);
  for (const inp of sortedInputs) {
    push(`| ${inp.id} | ${inp.kind} | \`${inp.sha256}\` |`);
  }
  push();
  push(
    `For NDJSON inputs the fingerprint is sha256 over \`sha256(samples.ndjson) + "\\n" + sha256(run.json)\` (or the samples hash alone when run.json is absent); for SQLite it is the file hash, re-verified unchanged after the analysis (read-only access).`,
  );
  push();

  // 10. Missing evidence for a future GO.
  push(`## Missing evidence for a future GO`);
  push();
  const unsatisfied = goGate.filter((g) => !g.satisfied).map((g) => g.id);
  if (unsatisfied.length === 0) {
    push(`All A4 gates are satisfied by the current evidence.`);
  } else {
    if (unsatisfied.includes("A4-1")) {
      push(
        `- Independent capture runs for the missing campuses (NK and/or CM alongside NB), each with enough change brackets per window for holdout grouping.`,
      );
    }
    if (unsatisfied.includes("A4-2")) {
      push(
        `- At least one America/New_York 17:00–18:00 peak window and one independent off-peak window, each with ≥ 5 informative brackets of its own (an empty or single-sample window is metadata, not peak evidence).`,
      );
    }
    if (unsatisfied.includes("A4-3")) {
      push(
        `- Enough brackets across ≥ 2 (target, window) groups for non-degenerate holdout, and a strict, holdout-consistent coverage win before any winner can be declared.`,
      );
    }
    if (unsatisfied.includes("A4-4")) {
      push(`- A winning model whose per-group phase intervals intersect and whose positive jitter is bounded, so one safe offset can be frozen.`);
    }
    if (unsatisfied.includes("A4-5")) {
      push(
        `- Captures that retain the server \`Date\` header with enough server-clock brackets to cover every qualifying group, so production conclusions never rest on a client-clock fallback.`,
      );
    }
    if (unsatisfied.includes("A4-6")) {
      push(
        `- Stability of the winner under all three checks: whole-target leave-out, (target, window) group leave-out, and deterministic top-k outlier removal.`,
      );
    }
    push();
    push(
      `Future captures must follow the A6 sampling constraints as the capture plan. This lane holds **no online authorization** and performed **no network access**; it only re-analyzed previously captured, gitignored local data.`,
    );
  }
  push();

  return lines.join("\n");
}
