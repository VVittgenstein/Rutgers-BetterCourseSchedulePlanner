# P1 Source Register and Provenance Protocol

Status: authoritative source and evidence-format register for the fresh P1

Scope: recovery of the historical requirements for A, the Windows-local release

Governing contract: `.ngagent/.orchestrator/architecture.md`

## 1. Purpose and boundary

This document defines which material the fresh P1 may use, how much authority
each source family has, and how downstream P1 artifacts must record evidence.
It is a register and protocol only. It does not extract A requirements, resolve
product choices, or reuse an earlier P1 outcome.

The evidence universe is closed by default. A source is admissible only when it
is registered below, is a legitimate tracked project/Git source admitted by
Section 5, or is later admitted through the conditional raw-history procedure
in Section 7. Worktree-external sources are byte-pinned, read-only exceptions:
they must never be modified, copied into Git, staged, committed, or packaged.

At creation of this register, all 25 declared external files matched the
SHA-256 values recorded below. The complete mainline log was read in full, not
sampled by keyword. No raw `.codex` or `.claude` session JSONL was opened.

`SRC-*` values below are stable source-register identifiers. They are not the
artifact-local evidence IDs required for substantive claims.

## 2. Access states

| Access state | Meaning |
|---|---|
| `READ_NOW_FULL` | The exact registered file may be read in full now. External files remain read-only and hash-pinned. |
| `READ_NOW_METADATA` | The exact registered index or governance file may be read now for discovery/provenance only. Embedded previews are not substantive evidence. |
| `GIT_READ_NOW` | Read-only Git inspection is allowed under the admissibility and quarantine rules in Section 5. |
| `CONDITIONAL_RAW` | The source is only a candidate. Its message content must remain unopened until the gap-closure gate in Section 7 is satisfied. |
| `PROHIBITED` | The source is outside the evidence universe and must not be opened, quoted, summarized, or used for corroboration. |

The dirty parent checkout is not a general read surface. Its only immediately
readable files are the five exact parent-project exceptions in Section 4.
Tracked project files must be read through the managed worktree. No other
parent-only, untracked, ignored, secret, runtime, or tool-state file is implied
by a directory appearing in a registered path.

## 3. Authority levels

Authority applies to a claim, not merely to the file containing it. A summary
that quotes or characterizes a user does not become direct-user evidence.

| Level | Authority | Treatment |
|---|---|---|
| `L1 PRIMARY_MAINLINE` | Exact user messages (`DIRECT_USER`) and decisions explicitly accepted on the current discussion line (`ACCEPTED_MAINLINE_DECISION`). | Highest authority. If two primary records appear inconsistent, record a `CONFLICT`; do not silently choose one. P1 Review owns the product resolution. |
| `L2 PRIMARY_HISTORY` | Contemporaneous, immutable project/Git records whose provenance is legitimate and not quarantined. | May establish what was requested, planned, committed, or observed at that time. It cannot override later L1 decisions. |
| `L3 HISTORICAL_SUMMARY` | Recovery-corpus narratives, memory documents, old plans, and the unaccepted task-015 candidate matrix. | Leads and corroboration only. Preserve their limitations and seek L1/L2 support for requirements. |
| `L4 IMPLEMENTATION_STATE` | Current or historical code, release behavior, tests, configuration, and runtime-facing documentation. | Can prove implementation state, drift, or absence, but cannot prove user intent. |
| `L5 ANALYTICAL` | Investigator synthesis, inference, conflict framing, and unresolved gaps. | Must be visibly classified as `INFERENCE`, `CONFLICT`, or `UNRESOLVED`; never present it as a user requirement. |

Direct-user evidence and accepted mainline decisions outrank summaries. Code or
release behavior may establish implementation state only. No authority rule
allows P1 to resolve a product ambiguity that belongs to P1 Review.

## 4. Current-mainline and accepted 0A-0C sources

All paths in this section are exact external read-only exceptions. These files
may be read now and must be cited by source ID, exact path, and a precise
message/line/section locator.

| Source ID | Family and authority | Access | Exact location | SHA-256 |
|---|---|---|---|---|
| `SRC-MAINLINE-LOG` | Complete current discussion-line export; L1 for exact user messages, L1 for explicit accepted decisions, otherwise classify each message by what it actually is | `READ_NOW_FULL` | `Z:\Project\Rutgers-BetterCourseSchedulePlanner\chat-log-codex-2026-07-10-1ce70862.md` | `76fee2f09567da0afde6fe9f36048c098b6f09f363ba7ad18b8abe5ea8387562` |
| `SRC-DUAL-WORKFLOW` | Accepted workflow and phase/ownership boundary; L1 accepted mainline decision | `READ_NOW_FULL` | `Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\dual-delivery-workflow.md` | `b8edca8f7af19f7986e26c5ebbccb399535e3dcd1fa28c217e2c5cb11eda9b06` |
| `SRC-PUBLIC-WEB-TARGET` | Accepted 0A product target; L1 accepted mainline decision | `READ_NOW_FULL` | `Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\public-web-target.md` | `770a6ee37c475e9fe78fa1cd99106aa57b8acdcdb90f32634aa027611eb49d01` |
| `SRC-DEPLOYMENT-DECISION` | Accepted 0B platform decision; L1 accepted mainline decision for its accepted conclusions | `READ_NOW_FULL` | `Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\deployment-platform-decision.md` | `6c213450c970382efe4ce731c460bce51ef15ff8066e39208a48f26f5c90e1ac` |
| `SRC-SHARED-RUST-ADR` | Accepted 0C architecture direction; L1 accepted mainline decision | `READ_NOW_FULL` | `Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\shared-rust-architecture-decision.md` | `b31c2284fa6ebb698a3e05f7839c0309c69cf89f686f8c4def63438523346946` |

The mainline export is the only approved current conversation export. Its
Codex summaries are not automatically accepted decisions, and the raw session
path named in its header is not thereby authorized for access.

## 5. Legitimate project and Git history

### 5.1 Read boundary

- Read tracked project content from this managed worktree, using
  repository-relative paths. Cite it as `IMPLEMENTATION_OBSERVATION` unless a
  higher-authority source independently establishes intent.
- Use Git read commands for history. A Git citation must contain a full 40-hex
  commit ID, the repository-relative path, and a line/section locator inside
  the blob. A branch name, tag, `HEAD`, or commit subject alone is not an
  immutable claim locator.
- Before citing a current file, inspect its last-changing commit. If the file
  was introduced or changed only by the quarantined lineage in Section 8, it
  is prohibited even though it is reachable from the current branch.
- The parent checkout is dirty. Do not read arbitrary parent-only files, and do
  not treat staging state, ignored files, or runtime directories as evidence.

### 5.2 Admissible anchors

| Source ID | Authority and permitted use | Immutable identity / location |
|---|---|---|
| `SRC-GIT-PUBLIC-BASELINE` | L2/L4 public-project state anchor; implementation/history only | Commit `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4` plus an exact blob path |
| `SRC-GIT-STAGE-A-P` | L2 historical project evidence for the accepted Stage A/P work; individual claims still require exact commits/blobs | Legitimate commits at or before the older P1 pause, reachable in the ancestry ending at `8004637c47e40ee3417b4d74d898124bd4b975f0`, excluding any source caught by Section 8 |
| `SRC-GIT-OLD-P1-PLAN` | L3 historical plan, not a current product decision | Commit `0a61028c91a93906758d41120fd9544ae889cbc7` plus exact blob path |
| `SRC-GIT-TASK015-COLLISION` | L2 historical execution-state evidence only | Commit `342e4502a3466c3e88d1291e5ffff2754e1acc30` plus exact blob path |
| `SRC-GIT-TASK015-PAUSE` | L2 historical execution-state anchor and end of the uncontaminated older baseline | Commit `8004637c47e40ee3417b4d74d898124bd4b975f0` plus exact blob path |
| `SRC-GIT-TASK015-MATRIX` | L3 candidate history only; useful but unmerged and never accepted by provenance review | Commit `5714a8f19481d22691ba799992609e6a5f619d02`, blob `.orchestrator/phase-1/01-release-surface-feature-matrix.md` |
| `SRC-WORKTREE-TRACKED` | L4 observation of current tracked product files, subject to last-change provenance check | Managed worktree `Z:\Project\Rutgers-BetterCourseSchedulePlanner\.ngagent\.worktrees\task-060`, cited by repository-relative path and immutable admissible blob identity |

The older task-015 matrix can suggest where to look, but a claim taken from it
remains `HISTORICAL_SUMMARY` unless independently supported. Its failed review
must be recorded in confidence/contradiction notes. Tracked legacy material
such as `docs/archive/stage-a-legacy/**` is not the same as
`.ngagent/archives/**`; it may be considered only as low-authority historical
or implementation evidence after normal provenance checks.

## 6. Declared recovery corpus

Every file below is an exact external read-only exception. The common root is
`Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current`.
Recovery narratives are L3 summaries, not direct user evidence. Index and
governance files are discovery/provenance metadata and do not establish A
requirements by themselves.

### 6.1 Recovery narratives

| Source ID | Access | Exact location | SHA-256 |
|---|---|---|---|
| `SRC-REC-00-STATUS` | `READ_NOW_FULL` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\00-status.md` | `f8b2b4c088a468d492ffb1b1427a64153a3009f53e6277a0cbc92c5b021e2f51` |
| `SRC-REC-01-SITE-MODEL` | `READ_NOW_FULL` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\01-site-model.md` | `4f2cdc9e447741476f93419fc31bfa33a33980a4bf7a09c2e4ad8defbcb9c384` |
| `SRC-REC-02-SOURCE-INVENTORY` | `READ_NOW_FULL` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\02-source-inventory.md` | `6b886cfbf1ae9f30db2f57c644626fcf1f157d91caad0e9a231289536178db64` |
| `SRC-REC-03-CODEX-SESSIONS` | `READ_NOW_FULL` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\03-codex-sessions.md` | `8e91fcd8ef2c7f88af031e6a08d802a38042f747aa2133bbdd4e3f84ca05c440` |
| `SRC-REC-04-CLAUDE-SESSIONS` | `READ_NOW_FULL` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\04-claude-sessions.md` | `99a83db67e00031d7be09e88e741676b6890c6ea6985f1645cb08805aa99aa84` |
| `SRC-REC-05-PROJECT-FILES` | `READ_NOW_FULL` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\05-project-files.md` | `f09a3bb81e247bad97387b36971adc586c66e229b0149ef865b92b102dcd89be` |
| `SRC-REC-06-TIMELINE` | `READ_NOW_FULL` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\06-timeline.md` | `106e71d0fefb85b4e4194d0bb168e7831adf458c5e92827b50c087d94cc5999c` |
| `SRC-REC-07-CONFLICTS` | `READ_NOW_FULL` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\07-conflicts-and-corrections.md` | `3272b24b4b3c3eb0e1389a0c8496bb310532af352bc5ae14a466310c456ecdf8` |
| `SRC-REC-08-CURRENT-STATE` | `READ_NOW_FULL` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\08-current-state.md` | `8a8a6d3d27e8442c7e3266f6af1d4df8347d6f7328dac619d9f605053853ece3` |
| `SRC-REC-09-NEXT-STEPS` | `READ_NOW_FULL` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\09-next-steps.md` | `128e96a2291f428c23946ebcfe3a4f55375e824a629ef946a589e54afb422350` |
| `SRC-REC-10-CRITICAL-DIALOGUES` | `READ_NOW_FULL` | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\10-critical-dialogues.md` | `3a1d7146e9b85b90f3d4ebd47dd729560b4f0094debd58959668375031917b14` |

### 6.2 Candidate indexes and recovery governance

| Source ID | Access and purpose | Exact location | SHA-256 |
|---|---|---|---|
| `SRC-REC-CLAUDE-INDEX-JSON` | `READ_NOW_METADATA`; structured Claude candidate inventory | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\claude-session-candidate-index.json` | `e3a36e1058ff7e514a7c7354a73bb8d462c8ae7011242784ac8ebaeb0db76e1f` |
| `SRC-REC-CLAUDE-INDEX-MD` | `READ_NOW_METADATA`; human-readable Claude candidate inventory | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\claude-session-candidate-index.md` | `14f1ee099e7b4a552f96f95b037bd505f72af6beee7c0d3aa24c98484af09175` |
| `SRC-REC-CODEX-INDEX-JSON` | `READ_NOW_METADATA`; structured Codex candidate inventory | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\codex-session-candidate-index.json` | `ea8d4a04bd22516b54b587f3fe17cef92e2c43079ff6f97ba47aae871d95a593` |
| `SRC-REC-CODEX-INDEX-MD` | `READ_NOW_METADATA`; human-readable Codex candidate inventory | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\codex-session-candidate-index.md` | `bc3c92800cd8ab318c77be972fb4e989cf534d07f9cfb58199d6a2261bbccd11` |
| `SRC-REC-FORBIDDEN-ACTIONS` | `READ_NOW_METADATA`; recovery access restrictions | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\forbidden-actions.md` | `e15be4d7057759579c9433a1ac796ddf890f5ef4805671fa0d81895ec0335cc6` |
| `SRC-REC-MANIFEST` | `READ_NOW_METADATA`; recovery scope/count manifest | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\manifest.json` | `8801ae61febc8d1bee833a1eae6dfae4c842fb07a1e4e75396e60c2c37bb00ad` |
| `SRC-REC-READ-LOG` | `READ_NOW_METADATA`; recovery provenance log | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\read-log.jsonl` | `7e593d605efc63b86587e115f15f903b826d0a44b0cf15294a9a201bef59aba9` |
| `SRC-REC-SESSION-INDEX-JSON` | `READ_NOW_METADATA`; combined Codex/Claude candidate inventory | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\session-candidate-index.json` | `c9e7272c87296bf07d2d0e54aa5d682361b0cb65ac937b0ae2a32c717ac5da43` |
| `SRC-REC-SESSION-INDEX-MD` | `READ_NOW_METADATA`; combined index summary | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\session-candidate-index.md` | `46eecb8242f48e587f140831c65118e8b157bc2e035531b912c45acf6bbed715` |

Candidate-index fields such as session ID, path, timestamps, CWD, counts, and
match type may be used to select a future candidate. Any embedded user or
assistant preview is a discovery aid only and must not be promoted into an
evidence claim. The underlying raw file remains `CONDITIONAL_RAW`.

## 7. Conditional raw-history candidates

The following path families are not approved read-now sources:

| Candidate family | Access | Location pattern |
|---|---|---|
| Codex live sessions | `CONDITIONAL_RAW` | `Z:\.codex\sessions\**\*.jsonl` |
| Codex archived sessions | `CONDITIONAL_RAW` | `Z:\.codex\archived_sessions\**\*.jsonl` |
| Claude sessions/history | `CONDITIONAL_RAW` | Exact `.jsonl` candidates beneath `Z:\.claude\**`, as named by the registered indexes |

Merely appearing in an index, recovery summary, or exported-log header does
not grant permission to open a raw file. Raw history is a last-mile gap-closure
source, not a parallel evidence pass.

A raw candidate may be opened only after all of these conditions are met:

1. The full mainline pass, legitimate project/Git pass, and declared recovery
   pass have been completed first.
2. A downstream artifact records a stable gap ID and the existing evidence IDs
   that failed to answer it. The gap must concern a material A requirement,
   provenance question, or contradiction; general curiosity is insufficient.
3. The candidate indexes identify one exact session file likely to close that
   named gap. The request records the exact path, session ID, intended
   message/line or minimal time window, expected relevance, and privacy risk.
4. The exact file is added as a hash-pinned external read-only source in the
   applicable TaskSpec before access. Directory-wide authorization is not
   allowed.
5. Access is minimized to the justified file and locator. Unrelated messages,
   raw tool output, secrets, provider inventory, and other conversations are
   neither copied nor summarized.
6. Any resulting claim receives the normal evidence fields and classification.
   If the targeted read does not close the gap, retain `UNRESOLVED`; do not
   expand into a broad session crawl.

The decision and result belong in
`docs/p1-a-recovery/05-targeted-history-gap-closure.md`. If no justified gap
exists, that artifact must say that no raw history was opened.

## 8. Quarantine and explicit exclusions

The following are `PROHIBITED` for every P1 evidence task, even when a path or
commit is locally reachable:

1. **All NGAT archives:** `.ngagent/archives/**` in any checkout or worktree.
   Archive install records, task artifacts, reports, transcripts, and copied
   outcomes are not admissible evidence.
2. **The archived runaway-P1 lineage:** every prior-runtime task from
   `task-016` through `task-060`, inclusive, plus every attempt, worktree,
   branch, commit, merge, completion/review/eval/merge artifact, ledger,
   replacement/remediation task, and generated outcome belonging to that
   lineage. This is a lineage rule, not an ID-only rule: the fresh active
   `task-060` is a different runtime task, and this register is its output—not
   evidence for A.
3. **Reachable runaway merges:** the following first-parent merges are named
   explicitly so their reachability from the fresh branch cannot be mistaken
   for admissibility:
   `7b80345cf525a365adb09d1f9fa4e59221ef536c` (old task-017),
   `7f599eb4ae6a15c93b73b6abedb09ba426d7f7ff` (old task-019),
   `afbced896214abac4025e29281cd555b00e7a5b7` (old task-050),
   `28e9213bdfceff86547df262d88cb096546a7371` (old task-051),
   `def384358c7859a20aff9e78b99c87e3adcef404` (old task-052),
   `53fa8929a4a20aa451a8c7d7a0ef50354e88ef45` (old task-053),
   `d6118d5dff2f8b4042b3637a88019fdaedb04aa6` (old task-054),
   `52718fa4ae9f64ee5143e1895c4d8b0cb4e69b49` (old task-055),
   `a2682475a637ea9b3c2860e17b9665b3906810e6` (old task-056), and
   `b6d6a65b179bbc07bac0829cd1d0c89684b15b71` (old task-057).
   Their side commits and blobs are equally prohibited. Old task IDs without a
   merge remain prohibited as attempts/outcomes.
4. **The old root requirements document:**
   `deliverable-a-windows-local-release-requirements.md` at repository root,
   including every historical blob and the runaway commits that populated it.
   It must not be quoted, diffed, summarized, or used as a checklist. The fresh
   candidate later produced at
   `docs/deliverable-a-windows-local-release-requirements.md` is a new P1
   output and likewise cannot serve as evidence for itself.
5. **Runtime and private material:** `.ngagent/**` except the governing
   planning contract supplied to the task, `.git/ngagent/**`, `.secrets/**`,
   `configs/*.local.json`, SSH material, provider inventory, tokens, billing
   data, raw tool output, and unrelated conversations.

No source may be rehabilitated by paraphrasing a prohibited artifact into a
new file. If an admissible recovery summary mentions a quarantined outcome,
only the fact that the summary reports a conflict or quarantine may be used;
the prohibited outcome's substantive claims remain unusable.

## 9. Required evidence record

Every substantive claim in downstream P1 evidence artifacts must contain all
of the following fields from the governing Evidence Contract:

| Required field | Rule |
|---|---|
| **Evidence ID** | Stable and artifact-local. Use a file-specific prefix plus a zero-padded sequence (for example, `ML-E001`). Never renumber or reuse an ID after publication. |
| **Classification** | Exactly one value from the closed vocabulary below. |
| **Source** | Exact registered source path/ID, or a full immutable Git commit identity plus repository-relative blob path. |
| **Locator** | Precise line range, mainline message number and role, Markdown section, JSON pointer/JSONL line, or commit/blob locator. File-only citations are insufficient. |
| **Quotation or faithful paraphrase** | Concise text representing what the source actually says or demonstrates. Mark paraphrases as paraphrases; do not turn summaries into quotations. |
| **Relevance to A** | Explain how the claim bears on A's description, goal, constraint, purpose, expectation, or provenance. |
| **Confidence / contradiction notes** | Record uncertainty, source limits, supersession, failed provenance, or conflicting evidence where needed. Use an explicit “none noted” when the tabular schema requires a value and no qualification is needed. |

The classification vocabulary is closed and case-sensitive:

- `DIRECT_USER`
- `ACCEPTED_MAINLINE_DECISION`
- `HISTORICAL_SUMMARY`
- `IMPLEMENTATION_OBSERVATION`
- `INFERENCE`
- `CONFLICT`
- `UNRESOLVED`

Classification rules:

- Use `DIRECT_USER` only for an exact user-authored record with an exact
  message/line locator.
- Use `ACCEPTED_MAINLINE_DECISION` only when the current discussion line
  explicitly accepted the decision or an accepted 0A-0C record states it.
- Use `HISTORICAL_SUMMARY` for recovery narratives, memory documents, old
  plans, and the candidate task-015 matrix, even when they describe a user.
- Use `IMPLEMENTATION_OBSERVATION` for code, configuration, tests, packages,
  or behavior. It proves state, not intent.
- Use `INFERENCE` when the investigator connects facts not directly stated by
  a source. Name the supporting evidence IDs and keep confidence explicit.
- Use `CONFLICT` when admissible sources disagree. Cite every side; do not
  silently collapse the conflict by authority or chronology.
- Use `UNRESOLVED` when the available admissible evidence does not support a
  conclusion. An unresolved item is not permission to guess.

## 10. Collection and provenance procedure

Downstream P1 evidence tasks must follow this order:

1. **Verify and register.** Recompute the raw-byte SHA-256 of every external
   file before use. A mismatch fails closed and is reported; it is never
   normalized or silently replaced.
2. **Read by authority order.** Process `SRC-MAINLINE-LOG` completely, then the
   accepted 0A-0C records, legitimate tracked/Git history, and the declared
   recovery corpus. Do not use keyword sampling as a substitute for the full
   mainline pass.
3. **Capture atomic claims.** Give each independently testable claim one stable
   evidence ID and all fields in Section 9. Split compound statements when
   their classifications, sources, or confidence differ.
4. **Preserve provenance.** Quote sparingly and accurately; otherwise use a
   faithful paraphrase. Keep source identity and locator beside the claim, not
   in an uncoupled bibliography.
5. **Apply authority without erasing disagreement.** L1 evidence outranks
   summaries, but a material contradiction still receives a `CONFLICT` record
   for P1 Review. Historical or implementation evidence cannot create a new
   user requirement.
6. **Enforce quarantine.** Check source lineage before citation. Reachability
   from the active branch, duplication in a recovery summary, or apparent
   usefulness does not make a prohibited source admissible.
7. **Close gaps conditionally.** Only after the first three evidence passes may
   Section 7 authorize one exact raw-session candidate. Otherwise leave raw
   history unopened.
8. **Keep artifacts safe and bounded.** Do not include secrets, provider data,
   raw tool output, unrelated conversation content, or external-source copies.
   Durable evidence belongs in the assigned Markdown artifact; completion
   reports should only summarize the work.

This register is the format authority for
`02-mainline-evidence.md`, `03-project-history-evidence.md`,
`04-recovery-corpus-evidence.md`, and the conditional
`05-targeted-history-gap-closure.md`. The later candidate, audit, correction,
and stop-gate artifacts must preserve the same evidence IDs and provenance
links. P1 ends at its review handoff; nothing in this protocol authorizes P2.
