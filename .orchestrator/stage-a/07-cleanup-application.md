# Stage A — Cleanup Application (`task-007`)

> **Task:** `task-007` — Stage A apply approved non-product cleanup and repository organization.
> **Branch:** `feature/task-007`
> **Worktree:** `Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\task-007`
> **Depends on:** `task-006` (Stage A final baseline). Inherits the approved non-product cleanup subset from `.orchestrator/stage-a/06-final-baseline.md` §7.
> **Scope:** Apply only the cleanup actions explicitly approved by the Stage A final baseline (`06-final-baseline.md` §7) and pinned by the Main Agent. Do not refactor product runtime behavior. Do not delete historical records or release artifacts. Do not touch the release packs, `frontend/src/dev`, `reports/field_validation_details.mdpart`, optional `.env.example` handling, product code, package manifests, schema, templates, or the external Obsidian vault. Preserve local copies in the task worktree for every `git rm --cached` operation.

## 0. Purpose

This report records the exact, evidence-justified non-product cleanup applied to the repository by `task-007`. It is the bridge from the Stage A audit-only reports (`01..06`) to a cleaned and organized repository state without changing product behavior. Each subsection below cites the Stage A baseline section that justified the change, lists the exact paths touched, and notes the safety checks performed.

## 1. Authority and Pre-flight Checks

Authority for every action below is established by:

- The Stage A final baseline at `.orchestrator/stage-a/06-final-baseline.md` §7.1 (untrack from git tracking), §7.2 (`.gitignore` and configuration-only changes), §7.3 (archive moves), and §7.4 (repository-organization moves).
- The pinned Main Agent context for `task-007` (rendered verbatim in the task spec): "Execute only TaskSpec v2 approved cleanup subset. Treat final baseline §7 as authoritative for cleanup boundaries. Do not touch release packs, frontend/src/dev, reports/field_validation_details.mdpart, optional .env.example handling, product code, package manifests, schema, templates, or the external Obsidian vault. For git rm --cached operations, preserve local copies in the task worktree and do not quote secret-like values."
- The `task-007` acceptance criteria AC-001..AC-009.

Pre-flight checks performed before any change:

1. **Branch.** `git branch --show-current` returned `feature/task-007`; clean tree confirmed by `git status --short --branch` (only the branch header line returned).
2. **Upstream context read.** `.orchestrator/stage-a/06-final-baseline.md` was read end-to-end. The Trusted (§5.1), Stale (§5.2), Historical evidence (§5.3), Unresolved (§5.4), Unsafe (§5.5), and Evidence-only (§5.6) tables, the source-of-truth hierarchy (§4), and the §7 cleanup recommendation set were treated as authoritative for cleanup boundaries.
3. **Secret pre-check.** `configs/mail_sender.user.json` was inspected at `HEAD` and on disk to confirm the file does not contain a real-looking SendGrid API key or password value (AC-003). The check was bounded: the file was opened only enough to confirm the placeholder shape; no value is quoted in this report. The result of the check is recorded in §3.1 below.
4. **`data/refresh_queue.json` non-presence check.** `git ls-files data/refresh_queue.json` returned empty; the file is forward-looking only. The `.gitignore` addition in §4 is preventative, exactly as recommended by `06-final-baseline.md` §7.2 and `04-runtime-config-hygiene.md` §5.2.

No file outside the `task-007` allowlist (`.orchestrator/stage-a/07-cleanup-application.md`, `.gitignore`, the seven AC-002 paths, the AC-005 historical record set, `read_only.md`, `docs/archive/stage-a-legacy/`, `notebooks/incremental_trial.md`, and `reports/incremental_trial.md`) was modified. No path on the `task-007` blocklist (`api/`, `frontend/`, `workers/`, `notifications/`, `package*.json`, `data/schema.sql`, `data/migrations/`, `configs/templates/`, `configs/*.example.json`, `configs/*.schema.json`, `release/`, `bcsp-20260122.zip`) was touched.

## 2. Summary of Changes

The repository changes applied by `task-007` are summarized in the table below. Each row links to the detailed subsection that records the change and to the upstream baseline section that justified it.

| # | Change | Mechanism | Files touched | Baseline §        | This report § |
| --- | --- | --- | --- | --- | --- |
| 1 | Untrack seven local runtime/config artifacts from git tracking; preserve local copies on disk | `git rm --cached` only | 7 paths (see §3) | §7.1 | §3 |
| 2 | Add `.gitignore` coverage for `scripts/poller_checkpoint.json` and `data/refresh_queue.json` | Two new lines in `.gitignore` | `.gitignore` | §7.2 | §4 |
| 3 | Relocate historical AI-workflow evidence under `docs/archive/stage-a-legacy/` with an archive index, leaving a forwarding pointer at the root for `read_only.md` | `git mv` for 4 JSON files and 3 directories (82 files); duplicate + edit for `read_only.md`; new `README.md` index | 89 entries (see §5) | §7.3 | §5 |
| 4 | Move `notebooks/incremental_trial.md` into `reports/` | `git mv` | 1 path | §7.4 | §6 |
| 5 | Produce this cleanup-application report | `Write` | `.orchestrator/stage-a/07-cleanup-application.md` | §11 (hand-off) | §0–§9 |

Total tracked-tree delta produced by this task (per `git status --short` against the pre-flight clean state):

- 2 Added files (`docs/archive/stage-a-legacy/README.md`, `docs/archive/stage-a-legacy/read_only.md`).
- 7 staged-as-deleted index entries (the AC-002 untrack set), each with the on-disk file preserved in the working tree.
- 2 Modified files (`.gitignore`, root `read_only.md`).
- 87 staged renames (4 root JSON + 1 notebook + 82 historical-record directory entries).
- Plus this report itself, which appears as an Added file once staged at commit time (§8).

No content edit was applied to any file inside the renamed historical-record set; the rename diff stat (`git diff --cached --stat -M --diff-filter=R`) shows `0 insertions(+), 0 deletions(-)` across all 87 renames.

## 3. Untrack From Git Tracking (AC-002, AC-003)

### 3.1 Secret pre-check on `configs/mail_sender.user.json` (AC-003)

Before staging the untrack, the on-disk file was inspected only enough to confirm placeholder shape. The structural shape was confirmed to match a deliberate placeholder, not a real credential:

- The `providers.sendgrid.apiKeyEnv` field is an environment-variable name, not a secret value. It references the same env name used by product docs.
- The `providers.sendgrid.apiKey` field is present but its length and prefix-only-`SG.<short-token>` shape is incompatible with a real, freshly-issued SendGrid token (real tokens are far longer and have a structured suffix). The literal value is intentionally not quoted in this report per the pinned Main Agent rule.
- The `providers.smtp.passwordEnv` field is an environment-variable name only; no `password` field carries a literal value.
- The `defaultFrom.email` and `replyTo.email` values use RFC-reserved example domains (`example.edu` and `demo.test`) consistent with documentation placeholders.

**Result:** the placeholder does not look like a real key or password. Per AC-003, the untrack was allowed to proceed. **No real-looking secret was found, so the AC-003 block condition did not trigger.**

If at any future time the file's on-disk content is replaced by a real SendGrid token (most plausibly via the WebUI's `PUT /admin/mail-config` write path described in `04-runtime-config-hygiene.md` §6), the file will remain untracked because of the existing `.gitignore:155` `configs/*.user.json` rule plus the untrack landed by this task. A second-line defence against the historical risk (any prior `git add` predating the untrack) is a separate Stage B concern and is explicitly out of scope here per `06-final-baseline.md` §7.5 "do not push any history rewrite without a separate Stage B decision."

### 3.2 The seven `git rm --cached` paths

Each row below names the path, the existing `.gitignore` rule that matches it (so that, once untracked, it stays untracked), and the upstream evidence that this is a runtime/local artifact rather than product source.

| Path | Existing `.gitignore` rule | Why this is local/runtime, not product | Evidence |
| --- | --- | --- | --- |
| `configs/mail_sender.user.json` | `.gitignore:155` `configs/*.user.json` | The `*.user.json` suffix is the local-secrets slot for `configs/`; the WebUI write-path targets this exact path. | `01-inventory.md` §4.7, §6; `04-runtime-config-hygiene.md` §5.1, §6; `06-final-baseline.md` §5.5 row 1. |
| `data/poller_checkpoint.json` | `.gitignore:151` `data/poller_checkpoint.json` | Pure runtime checkpoint rewritten on every poller run. | `01-inventory.md` §4.6; `04-runtime-config-hygiene.md` §5.1, §7.1; `06-final-baseline.md` §5.5 row 2. |
| `data/runtime/fetch_job_d21d4cd8-2df9-4bc8-b589-e86d19ceaf6a.json` | `.gitignore:150` `data/runtime/` | Per-run write-only fetch-job snapshot; also embeds a stale absolute Windows path referencing the pre-rename repo. | `01-inventory.md` §4.6; `04-runtime-config-hygiene.md` §5.1; `06-final-baseline.md` §5.5 row 3. |
| `data/courses.sqlite-shm` | `.gitignore:146` `data/*.sqlite-shm` | Orphaned SQLite sidecar regenerated by SQLite at runtime. | `04-runtime-config-hygiene.md` §5.1; `06-final-baseline.md` §5.5 row 4. |
| `data/courses.sqlite-wal` | `.gitignore:147` `data/*.sqlite-wal` | Orphaned SQLite sidecar regenerated by SQLite at runtime. | Same as above. |
| `data/fresh_local.db-shm` | `.gitignore:144` `data/*.db-shm` | Orphaned SQLite sidecar regenerated by SQLite at runtime. | Same as above. |
| `scripts/poller_checkpoint.json` | none (until §4 below) | Inverse case: runtime artifact under the wrong directory; canonical checkpoint lives at `data/poller_checkpoint.json` and oneclick uses only that path. | `01-inventory.md` §4.6, §4.10; `04-runtime-config-hygiene.md` §5.2, §7.2; `06-final-baseline.md` §5.5 row 5. |

The single command used was:

```text
git rm --cached \
  configs/mail_sender.user.json \
  data/poller_checkpoint.json \
  data/runtime/fetch_job_d21d4cd8-2df9-4bc8-b589-e86d19ceaf6a.json \
  data/courses.sqlite-shm \
  data/courses.sqlite-wal \
  data/fresh_local.db-shm \
  scripts/poller_checkpoint.json
```

Verification: the post-command `ls` of each path returned the same path back, confirming the on-disk copy was preserved in the working tree exactly as required by the pinned Main Agent rule ("preserve local copies in the task worktree"). The pre-`.gitignore`-edit `git status` showed `scripts/poller_checkpoint.json` as `??` (untracked); after §4 below, that line disappeared because the new rule covers the path.

### 3.3 Not in this set

These adjacent items are deliberately not untracked by `task-007` and the reasons are pinned by `06-final-baseline.md` §7.5:

- `release/bcsp-20260121.tar.gz`, `release/bcsp-20260121.zip`, `bcsp-20260122.zip` — already covered by `.gitignore:158-160` (and not tracked in this branch's `git ls-files`); their archival disposition is explicitly deferred per pinned Main Agent rule and AC-006. No `git rm --cached` is required.
- `frontend/src/dev/*` — on the blocklist for this task (within `frontend/`). Deferred to Stage B per `05-module-surface-map.md` §5 R-13.
- `reports/field_validation_details.mdpart` — explicitly deferred by AC-006.
- `data/schema.sql`, `data/migrations/**`, `configs/templates/**`, `configs/*.example.json`, `configs/*.schema.json` — product config and templates, on the blocklist.

## 4. `.gitignore` Update (AC-004)

Two lines were added after the existing `data/poller_checkpoint.json` rule and before the "Local configs generated from examples" comment block:

```
+data/refresh_queue.json
+scripts/poller_checkpoint.json
```

Both additions are exactly what `06-final-baseline.md` §7.2 recommended:

- `data/refresh_queue.json` — forward-looking; referenced by `configs/fetch_pipeline.example.json:79` per `04-runtime-config-hygiene.md` §5.2; the incremental fetcher would create this file at runtime and a future `git add .` could commit a runtime queue. Adding the rule preventatively closes the gap.
- `scripts/poller_checkpoint.json` — paired with the §3.2 untrack of the same path. This closes the inverse case noted by `01-inventory.md` §4.10: a runtime-shaped file at a path that no existing rule reached.

Per AC-004, the optional `.env.example` allowlist (`.gitignore:71` `!.env.example`) was deliberately left untouched. That decision belongs to Stage B (see `06-final-baseline.md` §7.2, third row, and §9 row 5). No other change was made to `.gitignore`.

Verification: post-edit `git diff -- .gitignore` shows exactly two added lines and no other changes (lines `+data/refresh_queue.json` and `+scripts/poller_checkpoint.json` inserted between line 151 `data/poller_checkpoint.json` and line 152 — the blank line preceding the `# Local configs generated from examples` block).

## 5. Archive Historical AI-Workflow Records (AC-005)

`06-final-baseline.md` §7.3 lists the historical-record set that should be relocated to a clearly-named archive while preserving every byte. `task-007` chose `docs/archive/stage-a-legacy/` as the archive path, consistent with the existing `docs/` convention and matching the `task-007` write-scope allowlist.

### 5.1 Items archived

| Original path | Archive path | Kind | Files moved |
| --- | --- | --- | --- |
| `record.json` | `docs/archive/stage-a-legacy/record.json` | Legacy planning JSON (parent task graph, 2,390 lines) | 1 |
| `rEmail.json` | `docs/archive/stage-a-legacy/rEmail.json` | Legacy planning JSON (mail-onboarding seed) | 1 |
| `rRevision.json` | `docs/archive/stage-a-legacy/rRevision.json` | Legacy planning JSON (filter-rewrite seed) | 1 |
| `rSubscribe.json` | `docs/archive/stage-a-legacy/rSubscribe.json` | Legacy planning JSON (auto-term poller seed) | 1 |
| `Compact/` | `docs/archive/stage-a-legacy/Compact/` | Per-subtask narrative + Code-Review trailers authored by an earlier AI workflow | 74 |
| `review/` | `docs/archive/stage-a-legacy/review/` | `diff --git a/record.json b/record.json` snapshots, 2025-11-13..17 (includes the cosmetic filename typo `review-recod-2025-11-14.md`) | 6 |
| `Rutgers-dr/` | `docs/archive/stage-a-legacy/Rutgers-dr/` | Hand-authored DR documents: `2025-11-11-dr.md` and `Patch-dr-2025-11-14.md` | 2 |
| `read_only.md` (root) | `docs/archive/stage-a-legacy/read_only.md` (copy) | DR distillation `5b-mb.md` output v0.1, 2025-11-13 | 1 (copied; root file retained as forwarding pointer) |

The mechanism for the 4 root JSON files and the 3 historical directories was `git mv`, which preserves rename detection and yields zero-line diffs (verified by `git diff --cached --stat -M --diff-filter=R`, which reported `0 insertions(+), 0 deletions(-)` across all 87 renames in this task).

For `read_only.md` the mechanism was different. AC-005 requires both **an archived copy** of `read_only.md` and **a forwarding pointer at the root path**. The applied procedure was:

1. `cp read_only.md docs/archive/stage-a-legacy/read_only.md`; `git add docs/archive/stage-a-legacy/read_only.md` — produces the archive copy as a new tracked file with byte-identical content.
2. Rewrite the root `read_only.md` with a short forwarding pointer that names the archive copy, names the documents that supersede the v0.1 content (`Rutgers-dr/Patch-dr-2025-11-14.md` (now at `docs/archive/stage-a-legacy/Rutgers-dr/Patch-dr-2025-11-14.md`), the `record.json.decisions` block (now at `docs/archive/stage-a-legacy/record.json`)), and names `.orchestrator/stage-a/06-final-baseline.md` and this report as the authoritative current Stage A documents.

This preserves the historical filename `read_only.md` at the root so existing external references resolve, while making the document's non-authoritative status explicit.

### 5.2 Archive index file

A new file at `docs/archive/stage-a-legacy/README.md` was added as the archive index. The README explicitly:

- Names every original-to-archive path mapping with file counts (the same table as §5.1 above).
- Cites the Stage A baseline sections that justified each move.
- States that no file content inside this archive was edited during the move.
- States that release artifacts are intentionally not in this archive (their disposition is deferred).
- States that `record.json` / `r*.json` status fields were not "corrected" — correcting them would destroy the evidence layer the Stage A baseline depends on.
- Cites the external Obsidian vault as out of scope for any in-repo change.

The README is the AC-005 "archive index" requirement and is the single canonical pointer for anyone reading `docs/archive/stage-a-legacy/` cold.

### 5.3 Verification

- `git status --short` counts the archive moves as 87 R-prefixed (renamed) entries: 4 root JSON + 1 notebook + 6 `review/` + 2 `Rutgers-dr/` + 74 `Compact/`. The `notebooks → reports` move (§6 below) accounts for the 88th rename when counted overall.
- `git diff --cached --stat -M --diff-filter=R` reports `0 insertions(+), 0 deletions(-)` across all renames — no content drift.
- The root `read_only.md` shows as `M` (modified) and `docs/archive/stage-a-legacy/read_only.md` shows as `A` (added). The archive copy is byte-identical to the pre-edit root file; the root file is now a short pointer.
- `docs/archive/stage-a-legacy/README.md` shows as `A` (added).

### 5.4 What was deliberately not archived

Per the pinned Main Agent rule and `06-final-baseline.md` §7.5:

- `release/bcsp-20260121.tar.gz`, `release/bcsp-20260121.zip`, `bcsp-20260122.zip` — release-pack archival is explicitly deferred.
- The Obsidian vault under `D:\Document\Obsidian\Adrian\Prompt\BetterCourseSchedulePlanner\` — external, out of repo scope. Not copied into this archive.
- Prior Stage A reports (`.orchestrator/stage-a/01..06.md`) — these are current planning surface (Tier T1 per `06-final-baseline.md` §4), not historical evidence.

## 6. Repository-Organization Move (AC-006)

Per `06-final-baseline.md` §7.4, only one cosmetic move from that section was applied by this task:

- `notebooks/incremental_trial.md` → `reports/incremental_trial.md` (via `git mv`).

This is the move recommended by `01-inventory.md` §4.10 and `06-final-baseline.md` §7.4 row 1: `notebooks/` carried a single file that is semantically a report (matches the existing `reports/` family of `field_validation*.{md,json}`, `mail_worker_latency.md`, `poller_durability.md`, `fresh_install_log.md`). Moving it consolidates the report family into one directory; no naming collision exists in `reports/`.

Verification: `git diff --cached --stat -M -- notebooks/incremental_trial.md reports/incremental_trial.md` reports `0 insertions(+), 0 deletions(-)`. The empty `notebooks/` directory is left in the working tree; git does not track empty directories, so it does not appear in `git status`. A future commit that wishes to remove the empty directory may do so without further evidence; this task does not delete it because that would be a working-tree change outside the rename diff and outside the strict "preserve local copies" rule that applied to the AC-002 untrack set.

### 6.1 What was deliberately not moved

Per AC-006 explicit deferrals and the pinned Main Agent rule:

- `reports/field_validation_details.mdpart` — non-standard extension; rename/finalize decision deferred. The file is left exactly as it was.
- `frontend/src/dev/*` — on the blocklist for this task and deferred to Stage B (R-13 in `05-module-surface-map.md` §5).
- Release pack disposition — explicitly deferred.
- `review/review-recod-2025-11-14.md` — the cosmetic filename typo (`recod` → `record`) is preserved through the archive rename without correction. The rename safety check in `03-record-reconciliation.md` §6.4 already confirmed no external references to the typo file exist in `Compact/`. Correcting the typo is a separate cosmetic decision and was not approved for `task-007` (it is not enumerated in `06-final-baseline.md` §7.4 with a §7-table approval, only as an unresolved item in §5.4 + §7.4 with explicit "approval gating: requires explicit human approval"; no such approval was provided in the task spec).

## 7. Constraint Compliance Check (Acceptance Criteria Coverage)

| AC | Requirement | Where addressed | Status |
| --- | --- | --- | --- |
| AC-001 | Produce `.orchestrator/stage-a/07-cleanup-application.md` describing every change and citing Stage A evidence | This document, §0 through §9, with explicit citations to `06-final-baseline.md` §§3.3, 4, 5.1–5.6, 7.1–7.5, and 9 in every change subsection | Done |
| AC-002 | Untrack only the seven approved local runtime artifacts without deleting local copies | §3.2 (seven paths and the single `git rm --cached` command); §3 verification | Done — `git status --short` lists exactly 7 `D` entries; `ls` of each path on disk returned the same path back |
| AC-003 | Before untracking `configs/mail_sender.user.json` confirm no real SendGrid key or password is present; do not quote values; block if a real secret is found | §3.1 (structural confirmation by env-var name pattern + placeholder shape; no values quoted); the block condition did not trigger because no real-looking secret was present | Done — proceeded only after confirming placeholder shape |
| AC-004 | Update `.gitignore` only for `scripts/poller_checkpoint.json` and `data/refresh_queue.json`; do not decide optional `.env.example` handling | §4 (the exact two added lines; the diff hunk attached); `.env.example` allowlist line 71 was not touched | Done |
| AC-005 | Move historical AI-workflow evidence into `docs/archive/stage-a-legacy` with an archive index; leave root `read_only.md` as a forwarding pointer | §5.1 (eight items moved), §5.2 (archive README), §5.3 (verification), §5.4 (deliberate non-archive items); root `read_only.md` rewritten as a forwarding pointer in §5.1 step 2 | Done |
| AC-006 | Apply only safe cosmetic organization from §7.4: move `notebooks/incremental_trial.md` into `reports/`. Defer `reports/field_validation_details.mdpart`, `frontend/src/dev`, and release pack disposition | §6 (one move); §6.1 (deferrals); §3.3 (release-pack non-touch) | Done |
| AC-007 | Do not delete historical records or release artifacts from disk | §3.2 verification (on-disk copies preserved for all seven untracked paths); §5 uses `git mv`/`cp`, not `rm`; §6 uses `git mv`, not `rm`; §5.4 / §3.3 / §6.1 list every release/archive surface that was deliberately not touched | Done |
| AC-008 | Do not modify product source, tests, package manifests, database schema, runtime config templates, or prior Stage A reports | All blocklisted paths (`api/`, `frontend/`, `workers/`, `notifications/`, `scripts/` aside from the `git rm --cached` line on `scripts/poller_checkpoint.json`, `package*.json`, `data/schema.sql`, `data/migrations/`, `configs/templates/`, `configs/*.example.json`, `configs/*.schema.json`) are unchanged; `.orchestrator/stage-a/01..06.md` are unchanged | Done — `git status --short` contains no path under any blocklisted root |
| AC-009 | Leave git status containing only intentional Stage A cleanup changes | §8 (commit shape) and §2 (summary tally: 2 A, 7 D, 2 M, 87 R, plus this report itself) — every entry in the staged tree is enumerated in §2 / §3 / §4 / §5 / §6 above | Done after §8 staging |

No acceptance criterion is left unmet. No criterion is unverifiable; every row in §7 is supported by either a direct `git` observation or an explicit citation to `06-final-baseline.md`.

## 8. Commit Shape

This cleanup application produces a **single normal task commit** on `feature/task-007`, consistent with the architecture rule "one delivery task, one normal task commit" pinned in `.orchestrator/architecture.md`. The commit groups:

- The 7 staged `git rm --cached` entries (AC-002, §3).
- The 1 `.gitignore` modification (AC-004, §4).
- The 87 renames into / inside `docs/archive/stage-a-legacy/` (AC-005, §5) and one rename `notebooks/incremental_trial.md → reports/incremental_trial.md` (AC-006, §6).
- The 2 newly-added files at `docs/archive/stage-a-legacy/README.md` and `docs/archive/stage-a-legacy/read_only.md` (AC-005, §5.1 step 1 and §5.2).
- The modified root `read_only.md` (AC-005, §5.1 step 2).
- This report at `.orchestrator/stage-a/07-cleanup-application.md` (AC-001).

The commit message follows the conventional shape used by upstream Stage A delivery commits (`docs(task-NNN): …` for report-only tasks; this task additionally touches `.gitignore` and applies renames, so the commit is captured as `chore(task-007): apply approved Stage A non-product cleanup`). No history rewrite, no force push, no `--amend`, no `--no-verify`. Merging into `dev` and pushing to `origin` are the Main Agent's responsibility after `ngagent review task-007` passes.

## 9. Hand-Off

- **Reviewer (`ngagent review task-007`).** This document is the single canonical record of what `task-007` did. Each subsection (§3, §4, §5, §6) cites the upstream baseline section that authorized the change. The §7 table is the AC-by-AC compliance map. Reviewers may verify each row independently: §2 via `git diff --cached --stat -M`; §3 via `git ls-files | grep -E "<paths>"`; §4 via `git diff -- .gitignore`; §5 via `git diff --cached --stat -M --diff-filter=R`; §6 via `git diff --cached -- notebooks/incremental_trial.md reports/incremental_trial.md`.
- **Stage B precondition P-01 (`06-final-baseline.md` §8.1).** This task is precisely the P-01 work item. Once accepted and merged into `dev`, Stage B's precondition is satisfied. The Stage B candidates B-01..B-19 enumerated in `06-final-baseline.md` §8.3 may then proceed in the sequence proposed by `06-final-baseline.md` §8.4. None of the Stage A cleanup gaps listed at `06-final-baseline.md` §8.5 ("Out-of-scope-for-Stage-B reminders") are reopened by Stage B if `task-007` lands as described here.
- **Deferred items.** The following items remain explicitly unresolved and require separate human approval before any future task touches them: `reports/field_validation_details.mdpart` finalize/rename; `frontend/src/dev/*` disposition (Stage B R-13); release-pack archival or rebuild (Stage B W-D); `.env.example` allowlist resolution (Stage B); `review/review-recod-2025-11-14.md` typo fix; SQLite canonical-path decision (Stage B); the broader history-rewrite question for `configs/mail_sender.user.json` (out of scope for Stage A entirely per `06-final-baseline.md` §7.5).
- **Context manifest update.** The Main Agent should append a short upstream summary for `task-007` after the merge, matching the format established for `task-001`..`task-006`. The merge-time gotcha recorded for upstream tasks in `.orchestrator/context_manifest.md` (post-merge staged-deletion of the newly-merged `.orchestrator/stage-a/<NN>-*.md`) should be expected for `07-cleanup-application.md` as well, and the same `git restore --source HEAD --staged --worktree .orchestrator/stage-a/07-cleanup-application.md` mitigation applies.

## 10. Notes on Secret Discipline and Filesystem Discipline

- **Secret discipline.** No real keys, tokens, passwords, or PII were quoted in this report. The placeholder shape of `configs/mail_sender.user.json` is described structurally in §3.1, without values. The tar UID/GID side-channel string flagged in `02-release-reconciliation.md` §3 / D-10 is not reproduced here, consistent with `06-final-baseline.md` §6.1 (b) and the pinned Main Agent rule.
- **Filesystem discipline.** No path outside `Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\task-007\` was read, written, or staged. The external Obsidian vault was not opened. The other worktrees and the root checkout were not accessed. `git add .` / `git add -A` were never used; every staging step named specific paths (`git rm --cached <paths>`, `git mv <src> <dst>`, `git add docs/archive/stage-a-legacy/README.md`, `git add docs/archive/stage-a-legacy/read_only.md`, `git add read_only.md`, `git add .gitignore`).
- **Authority discipline.** Every change is authorized by exactly one of: `06-final-baseline.md` §7.1, §7.2, §7.3, §7.4; the pinned Main Agent context for `task-007`; or AC-001..AC-009. No change was authorized by inference from a NULL-tier signal (per `06-final-baseline.md` §4): `.gitignore` matching alone, ignored state alone, untracked state alone, and remote branch state alone were never sufficient grounds for a change.
