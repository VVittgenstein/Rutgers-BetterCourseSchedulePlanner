# Project Timeline / 项目时间线

> Append-only chronological record of all significant project events.
> Status legend: ✅ Success | ❌ Failed | ⚠️ Partial/Pivoted | 🚧 In Progress
> Detailed logs live under `docs/sessions/`. Internal Stage A/P reports under `.orchestrator/stage-{a,p}/`.

---

## 2026-05-11: Stage A Phase 1 — Repository Inventory ✅ (task-001)

**背景 / Context:** Stage A 启动: 在任何重构前, 先把仓库现状作为独立证据层重新清点, 不假设任何来源是权威.

**做了什么 / What was done:**
- Inventoried repository contents by category: product source, tests, current docs, historical AI workflow records, generated/runtime artifacts, local-only configs, release artifacts, unknowns.
- Wrote `.orchestrator/stage-a/01-inventory.md` framing `.gitignore` / tracked / ignored / untracked / remote as **evidence layers, not authorities**.

**关键决策 / Key decisions:**
- Stage A execution = audit-first, report-only by default (no source code changes).
- Active task system = ngagent (legacy `record.json` is evidence, not authority).
- Review must use Opus 4.7 Max only; no model fallback.

**结果 / Result:** Inventory established as foundation for tasks 002-006. Flagged `release/`, `bcsp-20260122.zip`, legacy JSON, runtime/config artifacts for downstream reconciliation.

**相关文档 / Related:** `.orchestrator/stage-a/01-inventory.md`

---

## 2026-05-11: Stage A Phase 2 — Release Reconciliation ✅ (task-002)

**背景:** 用户怀疑 release pack 与仓库存在 drift, 需要独立比对.

**做了什么:**
- Inspected `release/bcsp-20260121.tar.gz`, `release/bcsp-20260121.zip`, `bcsp-20260122.zip`.
- Compared against current repository contents.
- Wrote `.orchestrator/stage-a/02-release-reconciliation.md`.

**关键决策:**
- **No existing release pack should be trusted as current.** All require full reconciliation if to be used.

**结果:** Review passed. Noted low-risk count inconsistencies and a literal packager-username side-channel that final baseline must handle.

**教训 / Lesson:** Release archives drift from repo state silently — never auto-trust.

**相关文档:** `.orchestrator/stage-a/02-release-reconciliation.md`

---

## 2026-05-11: Stage A Phase 3 — Record Reconciliation ✅ (task-003)

**背景:** Legacy planning JSON (`record.json`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`) and historical workflow records (`Compact/`, `review/`, Obsidian notes) needed classification.

**做了什么:**
- Mapped legacy records to actual files / current implementation.
- Classified each record as authoritative / evidence / archive / stale / contradictory.
- Wrote `.orchestrator/stage-a/03-record-reconciliation.md`.

**结果:** Most legacy records classified as **archive-oriented evidence** with stale or contradictory status fields. Review passed with cosmetic notes (bad section refs, less explicit `reports/` enumeration).

**相关文档:** `.orchestrator/stage-a/03-record-reconciliation.md`

---

## 2026-05-11: Stage A Phase 4 — Runtime/Config Hygiene ✅ (task-004)

**背景:** Identify runtime/config surfaces, secret-risk areas, and clean-checkout blockers without exposing secret values.

**做了什么:**
- Audited `data/`, `configs/`, ignored files, generated files, startup scripts.
- Wrote `.orchestrator/stage-a/04-runtime-config-hygiene.md`.

**关键发现 / Key findings:**
- Tracked local config/runtime artifacts despite ignore rules.
- `scripts/poller_checkpoint.json` is runtime-shaped but unignored.
- Competing DB defaults across modules.
- Missing tracked `.env.example`.
- Stale absolute runtime paths in some files.

**结果:** All findings recorded as recommendations; actual application gated to task-007.

**相关文档:** `.orchestrator/stage-a/04-runtime-config-hygiene.md`

---

## 2026-05-11: Stage A Phase 5 — Module Surface Map ✅ (task-005)

**背景:** Build a Stage B refactor candidate list with evidence, priority, blast radius.

**做了什么:**
- Mapped product module surfaces and identified Stage B refactor candidates.
- Wrote `.orchestrator/stage-a/05-module-surface-map.md`.

**关键发现:**
- API route gaps, frontend test gaps, data pipeline/runtime coupling, mail/config risk, startup/bootstrap issues, docs/source drift.

**结果:** Review passed with **no findings**. Provides the input for any future Stage B planning.

**相关文档:** `.orchestrator/stage-a/05-module-surface-map.md`

---

## 2026-05-11: Stage A Phase 6 — Final Baseline ✅ (task-006)

**背景:** Consolidate Stage A reports into a single source-of-truth hierarchy and gate cleanup proposals before any code/file movement.

**做了什么:**
- Declared Stage B source-of-truth hierarchy across reports 01-05.
- Gated cleanup application via §7 of the baseline doc.
- Wrote `.orchestrator/stage-a/06-final-baseline.md`.

**关键决策:**
- Approved-for-task-007 cleanup subset: untrack local/runtime artifacts (preserve local copies), add ignore coverage for `scripts/poller_checkpoint.json` + `data/refresh_queue.json`, archive historical AI workflow records under a clear archive path.
- Deferred: release pack disposition, optional `.env.example`, `frontend/src/dev`, `reports/field_validation_details.mdpart`.

**相关文档:** `.orchestrator/stage-a/06-final-baseline.md`

---

## 2026-05-11: Stage A Phase 7 — Cleanup Application ✅ (task-007)

**背景:** Apply only the explicitly approved non-product cleanup from task-006.

**做了什么 (merge `f5031a1`):**
- Untracked 7 local/runtime artifacts (preserved as ignored local files).
- Added ignore coverage for `scripts/poller_checkpoint.json` and `data/refresh_queue.json`.
- Moved legacy AI workflow evidence into `docs/archive/stage-a-legacy/`.
- Left root `read_only.md` as forwarding pointer.
- Moved `notebooks/incremental_trial.md` → `reports/incremental_trial.md`.
- Wrote `.orchestrator/stage-a/07-cleanup-application.md`.

**结果:** Stage A complete. Repository organized at the documentation/history/repo-structure layer; product behavior unchanged.

**教训:** `ngagent merge task-007` left main worktree/index staged with a large inverse of the cleanup move. `git restore --source=HEAD --staged --worktree -- .` realigned tracked files; local artifacts restored from `.git/ngagent/local-backups/task-007-20260511-044954`.

**相关文档:** `.orchestrator/stage-a/07-cleanup-application.md`

---

## 2026-05-11: Stage A Complete — Stage P Opens ✅

**背景:** Stage A delivery tasks 001-007 all merged into local `dev`. Local repository is cleaned/separated/organized at the documentation layer; product behavior intentionally unchanged. Stage B (refactor) deferred.

**Pivot:** Stage P opened: synchronize the cleaned local project into a sanitized public `origin/main` without exposing ngagent/orchestrator internals, while preserving multiple normal commits for visible contribution activity.

**关键决策:**
- Public branch = sanitized replay, NOT raw `dev` -> `main` merge.
- Stage P delivery executors = Claude Opus 4.7 Max; reviewers = GPT-5.5 with xhigh reasoning. No fallback.
- All cutover actions (replace `main`, force push, default branch, remote branch deletion) require explicit human approval.

---

## 2026-05-11: Stage P Phase 1 — Public Divergence & Exposure Policy ✅ (task-008)

**背景:** Confirm whether raw `dev` -> `main` merge is safe.

**做了什么:**
- Compared `origin/main` against local `dev`.
- Wrote `.orchestrator/stage-p/01-public-divergence-and-exposure-policy.md`.

**关键发现:**
- `main` has real Auto Refresh / Scheduled Fetch product files **missing from `dev`**.
- `dev` has internal `.orchestrator/`, `AGENTS.md`, `docs/archive/stage-a-legacy/` artifacts that **must not be public**.
- Conclusion: raw merge is unsafe; sanitized replay required.

**结果:** Public candidate construction must use sanitized replay, preserve `main`'s product feature, exclude internal/runtime/private/history surfaces, merge `.gitignore` intent from both branches.

**相关文档:** `.orchestrator/stage-p/01-public-divergence-and-exposure-policy.md`

---

## 2026-05-11: Stage P Phase 2 — Public Commit Sequence ✅ (task-009)

**背景:** Design the exact commit sequence for the public candidate branch.

**做了什么:**
- Started from `origin/main` tip `98748e1466ec55e93115f651a6a84f84e227daa9`.
- Designed plan to preserve existing public Auto Refresh/Scheduled Fetch history + add 2 sanitized hygiene commits.
- Wrote `.orchestrator/stage-p/02-public-commit-sequence.md`.

**关键决策:**
- Commit 1: untrack `scripts/poller_checkpoint.json` + add runtime ignore rules.
- Commit 2: add defensive ignores for `far/`, `.git/ngagent/`, `.worktrees/`.
- Deferred: README/runbook swaps and `.env.example`.

**相关文档:** `.orchestrator/stage-p/02-public-commit-sequence.md`

---

## 2026-05-11: Stage P Phase 3 — Public Candidate Build ✅ (task-010)

**背景:** Build and push the `public-main-candidate` branch for review.

**做了什么:**
- Built `public-main-candidate` at `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4`.
- Pushed to `origin/public-main-candidate` (non-default name).
- 2 commits ahead of `origin/main@98748e1`: `f819d3c` (untrack + runtime ignores), `9c93170` (defensive ignores).
- Wrote `.orchestrator/stage-p/03-public-candidate-build.md`.

**结果:** GPT-5.5 review found **no excluded internal/private/runtime-history paths** and **no secret-shaped matches** in the candidate tree. `origin/main` and `far/` untouched.

**教训 / Lesson:**
- Needed human-approved ngagent gate recovery after 3 dispatches. Early CompletionReports used schema-hostile values (`null` for string fields). Recovery prompt must specify "string fields must be strings, not `null`".

**相关文档:** `.orchestrator/stage-p/03-public-candidate-build.md`

---

## 2026-05-11: Stage P Phase 4 — Public Cutover & Local Cleanup ✅ (task-011)

**背景:** After explicit human approval, replace `origin/main` with the reviewed `public-main-candidate`.

**做了什么:**
- Non-force single-ref push from `origin/public-main-candidate` -> `origin/main` at `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4`.
- `origin/dev` left at `2fab439b9e8e77e45969405508273b53f95029f5`; no remote branches deleted in this task.
- Public `main` tree matches reviewed candidate (160 tracked files, no forbidden internal path matches).
- Deleted local temporary `far/` directory after resolved-path verification.
- Wrote `.orchestrator/stage-p/04-public-cutover-and-local-cleanup.md`.

**相关文档:** `.orchestrator/stage-p/04-public-cutover-and-local-cleanup.md`

---

## 2026-05-11: Stage P Phase 5 — Remote Surface Audit ✅ (task-012)

**背景:** After main cutover, classify remaining remote refs / tags / GitHub Releases for closeout.

**做了什么:**
- Audited remote: confirmed **147 branches, 8 tags, 1 GitHub Release** (`id=264993969`, `tag_name=Release-0122`, display `Release-0121`).
- Classified `origin/main@9c93170` as the only keep ref.
- Wrote `.orchestrator/stage-p/05-public-remote-surface-audit.md`.

**关键决策:**
- Recommend deleting all non-main branches and all stale tags.
- `public-main-candidate` + GitHub Release = explicit human-confirmation items.
- After `origin/dev` deletion, local `dev` must NOT be pushed back to public `origin`.

**教训:** GitHub CLI not installed; deleting GitHub Release object may require authenticated API/tooling beyond plain git tag deletion.

**相关文档:** `.orchestrator/stage-p/05-public-remote-surface-audit.md`

---

## 2026-05-11: Stage P Phase 6 — Public Branch Closeout ✅ (task-013)

**背景:** After explicit human approval and GPT-5.5 review, delete all non-main remote branches.

**做了什么:**
- Deleted **all 146 non-main remote branches** from `origin`, including `dev` and `public-main-candidate`.
- Zero failures.
- Final remote heads = exactly `refs/heads/main` at `9c93170c5dc8e3b767312b4877d87ee0d2ce19e4`.
- No tags or GitHub Release objects modified.
- Wrote `.orchestrator/stage-p/06-public-branch-closeout.md`.

**相关文档:** `.orchestrator/stage-p/06-public-branch-closeout.md`

---

## 2026-05-11: Stage P Phase 7 — Public Tags & Releases Closeout ✅ (task-014)

**背景:** Final remote surface closeout — delete the 8 stale tags and the 1 GitHub Release object.

**做了什么:**
- After explicit human approval and GPT-5.5 review:
  - Deleted GitHub Release `id=264993969` (`tag_name=Release-0122`, display `Release-0121`).
  - Deleted all 8 stale remote tags.
- Final public remote state: **1 head (`refs/heads/main` at `9c93170c5...`), 0 tags, 0 GitHub Releases.**
- No branch refs or product files modified.
- Wrote `.orchestrator/stage-p/07-public-tags-releases-closeout.md`.

**结果:** Stage P complete. Public surface fully closed down to intended state.

**相关文档:** `.orchestrator/stage-p/07-public-tags-releases-closeout.md`

---

## 2026-05-11: Project Memory System Initialized ✅

**背景 / Context:** With Stage A and Stage P both complete, set up the three-layer project memory system (`/pm-init`) so future sessions have a stable, auto-loaded context dashboard distinct from the heavyweight `.orchestrator/` Stage A/P artifacts.

**做了什么 / What was done:**
- Created `CLAUDE.md` at repo root (current state dashboard, bilingual).
- Created `docs/TIMELINE.md` (this file) with full backfill of Stage A + Stage P task history.
- Created `docs/DIGEST.md` + `docs/registry.json` (empty distillation layer for `/pm-save`).
- Created `docs/sessions/README.md` and first session file `docs/sessions/2026-05-11-pm-init-setup.md`.

**关键决策 / Key decisions:**
- Memory docs language = **bilingual (中/EN)**.
- TIMELINE backfill = **full** (one entry per task 001-014).
- Next phase = **undecided** (per human: "现在不用决定").

**结果 / Result:** Three-layer system in place. CLAUDE.md auto-loads on session start; failed experiments will not pollute it; all history preserved under TIMELINE + sessions/ + .orchestrator/stage-{a,p}/.

**相关文档:** `docs/sessions/2026-05-11-pm-init-setup.md`

---

## 2026-05-12: Phase 1 - Stage A/P Public Remote Closeout Savepoint ✅

**背景:** 用户触发 `$pm-save`，要求把本次长会话保存到三层记忆系统中。本次保存覆盖 Stage A 清洗/剥离/整理、Stage P public remote closeout，以及用户关于 Git/GitHub/dev/PR/release 的关键澄清。

**做了什么:**
- 运行 4 个 pm-save 工作单元：事实提取、决策链提取、关键原文提取、产物验证。
- 创建 `docs/sessions/2026-05-12-public-remote-closeout-savepoint.md`，记录用户三点历史判断、Stage A/B 拆分、ngagent 执行策略、public/internal 分离、remote closeout 全过程。
- 验证最终 public remote state：仅 `refs/heads/main@9c93170c5dc8e3b767312b4877d87ee0d2ce19e4`，0 tags，0 GitHub Releases。

**关键决策:**
- Public GitHub 目标不是 raw `dev` 合并，而是 sanitized `main` + 删除所有公开 construction refs（用户确认，agent 执行）。
- Local `dev` 继续作为内部协调线；不得 `git push origin dev`、`git push --all` 或 `git push --mirror`（agent 提醒，用户接受）。
- 下一阶段仍未决定；Stage B refactor / maintenance / new features 需另行确认（用户暂未选择）。

**结果:** Stage A/P 已形成可回放保存点；远端公开表面已收口为 clean `main`，memory docs 记录了完整上下文和后续风险。

**教训/发现:** GitHub public surface 包括 branches、tags、releases 和 assets；只更新 `main` 不能消除 `dev had recent pushes` / PR prompt。当前 memory docs 是本地未跟踪文件，是否纳入内部 `dev` 提交需另行决定。

**相关文档:**
- `docs/sessions/2026-05-12-public-remote-closeout-savepoint.md`

---

## Current Status (2026-05-12)

**Active Work / 当前工作:** None — Stage A and Stage P fully closed out. Latest savepoint recorded in `docs/sessions/2026-05-12-public-remote-closeout-savepoint.md`.

**Next Milestones / 下一里程碑:**
- Decide next phase: Stage B refactor (per `05-module-surface-map.md`) / new features / maintenance only.
- Decide whether memory docs should remain local-only for now or be committed on internal `dev`.

**Public State / 公共状态:** `origin/main` only (1 commit `9c93170c5...`), 0 tags, 0 releases.

**Local State / 本地状态:** `dev` is internal coordination branch; do NOT push to `origin`. Memory docs currently live as local untracked files unless separately committed.

---

*Status legend: ✅ Success | ❌ Failed | ⚠️ Partial/Pivoted | 🚧 In Progress. Detailed logs in `docs/sessions/`. Stage A/P internal reports under `.orchestrator/stage-{a,p}/`.*
