# Stage P · Public Divergence and Exposure Policy

> Stage P task-008 audit. Authority: this document defines the **public include/exclude
> policy** that downstream Stage P tasks (public candidate construction, sanitized replay,
> human-approved cutover) must conform to. It does not yet construct the public branch.
>
> Evidence layers: `origin/main`, `origin/dev`, the local nested clone `far/Rutgers-BetterCourseSchedulePlanner`,
> the live working tree, and `.gitignore`/tracked-state observations. As established in
> `.orchestrator/context_manifest.md`, ignore state, tracked state, and remote state are
> evidence layers, not authorities.
>
> Scope discipline: this task is read-only against product code. No source, tests,
> manifests, runtime configs, `main`, `dev`, or `far/` were modified. The only file
> created is this report.

---

## 0. Evidence layers and how they were sourced

| Layer | Role in this audit | Read access used |
|---|---|---|
| `origin/main` (tip `98748e1`) | Public default branch, including the prior 2026-05-09 public-cleanup PRs (#203 / #204 / #206). The authoritative answer to "what is public today." | `git ls-tree`, `git diff`, `git log`, `git cat-file blob` against fetched ref. |
| `origin/dev` (tip `acbf13d`) | Internal working branch, including ngagent orchestration scaffolding and Stage A reconstruction reports. The authoritative answer to "what is the local synthesis today." | Same as above. |
| `far/Rutgers-BetterCourseSchedulePlanner` | User-provided nested clone intended as a redundant local view of `origin/main`. Mentioned as "temporary local evidence for `origin/main`" in the architecture plan. | **Not directly read**: the worktree's own filesystem boundary forbids `../` traversal, and `origin/main` is already fully fetched and queryable in the same repository, so the `far/` clone is informationally redundant for this audit. The `far/` checkout therefore stays untouched and is not added to any public commit. The Stage P plan to delete the local `far/` directory after Stage P remains valid. |
| Live worktree `feature/task-008` | Verified to be a clean checkout of `feature/task-008` (which equals `origin/dev` plus this report-in-progress). No untracked or ignored files present. | `git status --porcelain --ignored=traditional`, `git ls-files --others`. |

> **Evidence note**: the parent repository's working copy may have its own untracked
> state (notably `far/`) that is visible from outside this worktree. That state is
> **outside this audit's filesystem scope**. Public include/exclude rules below are
> defined against tracked Git state on `origin/main` and `origin/dev` plus the explicit
> rules in `.orchestrator/architecture.md` §"Stage P Public Branch Architecture",
> *not* against any worktree-relative on-disk inventory.

---

## 1. Ancestry summary

```
                                     o  acbf13d  feature/task-008 / origin/dev
                                     |          (+ this Stage P report on feature/task-008)
                                    ...  24 internal commits
                                     |          (ngagent init -> Stage A task-001..task-007 -> docs)
                              5ffa415 o
                                     |
            [merge base]      2d76217 o  Merge PR #202 tasks_bugfix
                              /       \
   71f2a2b  tasks_bugfix     o         o   e770bf2  auto-refresh-tasks
                                       |
                              b650d81  o   Merge PR #203 auto-refresh-tasks
                                       |
                              c55352b  o   Clean public repository surface (mail_sender.user.json + README)
                                       |
                              6863093  o   Merge PR #204 clean-public-surface
                                       |
                              45455c1  o   Clean public repository surface (workflow records, runtime files, ...)
                                       |
                              7abd5f1  o   Clean public repository surface (duplicate, .gitignore variant)
                                       |
                              98748e1  o   Merge PR #206 clean-public-artifacts-20260509  --> origin/main tip
```

| Quantity | Value |
|---|---|
| Merge base | `2d762179025e68ee05f853c3e7b5e8b43837893c` (`2d76217 Merge pull request #202 from VVittgenstein/tasks_bugfix`) |
| Commits on `origin/main` not in `origin/dev` | 7 |
| Commits on `origin/dev` not in `origin/main` | 25 |
| Tracked files on `origin/main` | 161 |
| Tracked files on `origin/dev` | 263 |

### 1.1 Commit groupings on `origin/main \ origin/dev` (7 commits)

| Group | Commits | Purpose |
|---|---|---|
| Product feature | `e770bf2` + merge `b650d81` | Adds the **Auto Refresh / Scheduled Fetch** vertical slice (backend `api/src/routes/scheduled-fetch.ts` + `api/src/services/scheduledFetcher.ts`, frontend `frontend/src/api/scheduledFetch.ts` + `AutoRefreshToggle.{tsx,css}` + `ScheduledFetchPanel.{tsx,css}` + `useAutoRefresh.ts` + `useScheduledFetch.ts`). |
| Public-surface cleanup #1 | `c55352b` + merge `6863093` | Deletes `configs/mail_sender.user.json` (a private user config that had been previously committed) and rewrites `README.md` into a shorter Chinese-first beginner guide. |
| Public-surface cleanup #2 | `45455c1` + `7abd5f1` + merge `98748e1` | Bulk delete of historical workflow records (`Compact/`, `review/`, `Rutgers-dr/`, `record.json`, `read_only.md`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`, `auto-refresh-tasks.json`), legacy reports (`reports/*`, `notebooks/*`), and runtime data (`data/courses.sqlite-shm`, `data/courses.sqlite-wal`, `data/fresh_local.db-shm`, `data/poller_checkpoint.json`, `data/runtime/*`); plus `.gitignore` extensions to keep these names out. |

`45455c1` and `7abd5f1` are textually identical content cleanups produced from two
machines (different author identities); both are present on `origin/main` because
`7abd5f1` re-applied the same deletions on top of `45455c1` to also rewrite line
endings inside `.gitignore`. They are not duplicates in the Git data sense (different
trees), but they are duplicate **intents**.

### 1.2 Commit groupings on `origin/dev \ origin/main` (25 commits)

| Group | Approximate commits | Purpose |
|---|---|---|
| ngagent bootstrap | `190ff11`, `ea8a423` | Two `chore: initialize multi-agent orchestrator` commits adding `.orchestrator/`, `AGENTS.md`, `.git/ngagent/` scaffolding metadata mirrored under tracked planning files. |
| Stage A planning | `4fb0e33`, `0c1bd99`, `3164e83`, `5ffa415` | Pure docs: define cleanup plan, tighten review policy, record selective-disclosure risk, treat ignore state as evidence. Touch only `.orchestrator/*.md`. |
| Stage A delivery + handoff | `6421d97`, `84710d5`, `22ca82e`, `8a35984`, `b32a1ef`, `f980271`, `98e0ec6`, `b79acd8`, `296107c`, `4ba8464`, `48c4646`, `cdf0a61`, `7375c27`, `9d79e00`, `6c8a70d` | Seven Stage A delivery tasks (task-001..task-006 reports) plus their merge commits and three handoff doc commits. Each delivery commit touches `.orchestrator/stage-a/*.md` only. |
| Stage A cleanup application | `dbd27ca`, `f5031a1` | Task-007 normal cleanup commit + merge: untracks seven local/runtime artifacts (preserving local copies via `.git/ngagent/local-backups`), adds two ignore patterns, archives historical AI workflow records under `docs/archive/stage-a-legacy/`, replaces root `read_only.md` with a forwarding pointer, and moves `notebooks/incremental_trial.md` to `reports/incremental_trial.md`. |
| Stage A completion + Stage P plan | `0f93e36`, `acbf13d` | Two doc commits adding the Stage A completion record and the Stage P plan to `.orchestrator/context_manifest.md` and `.orchestrator/architecture.md`. |

### 1.3 Why `origin/dev` cannot auto-merge into `origin/main`

The two branches diverged by 7 ahead and 25 ahead from a common base. A raw
fast-forward is impossible by definition (branches diverged), and a raw three-way
merge would generate three classes of conflict and one class of unintended
exposure. They are listed here as **observation only**; resolution is the
responsibility of subsequent Stage P tasks.

| Class | Path(s) | Conflict shape | Why it blocks a clean merge |
|---|---|---|---|
| **C1 · Real text conflict on tracked files** | `README.md` | Both branches modified the same file in incompatible ways: `c55352b` rewrote it from the long bilingual technical README into a compact beginner README; `dev` instead inherited the pre-cleanup `7763643f...` blob through the merge base and never touched it. A three-way merge produces a `<<<<<<<` block over the entire file. | Cannot be resolved automatically; requires a deliberate decision on which README is the "public" README. |
| **C1** | `.gitignore` | Both sides modified `.gitignore` after the merge base in different directions. `main` added bulk ignores for the deleted workflow filenames (`Compact/`, `review/`, `Rutgers-dr/`, `reports/`, `notebooks/`, `record.json`, `read_only.md`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`, `auto-refresh-tasks.json`). `dev` instead added ignores for runtime artifacts that `main` does not yet ignore (`data/refresh_queue.json`, `scripts/poller_checkpoint.json`, `.worktrees/`). The two patches overlap in the same file region. | Mechanical merge will produce conflict hunks on every overlapping line region. |
| **C1** | `docs/data_load_runbook.md`, `docs/data_refresh_strategy.md`, `docs/notify_runbook.md` | `main`'s public cleanup rewrote in-doc references to local validation reports because the underlying files were deleted ("Keep latency reports as local generated artifacts" / "Keep run notes locally rather than committing generated notebooks"). `dev` instead kept references and pointed them at the surviving `reports/*.md` and `notebooks/incremental_trial.md` files (later moved to `reports/incremental_trial.md` in task-007). The two sides modified the same paragraphs in opposite directions. | Mechanical merge will conflict on the same hunks. |
| **C2 · Rename / delete conflict against the same logical content** | `Compact/*.md`, `review/*.md`, `Rutgers-dr/*.md`, `record.json`, `read_only.md`, `rEmail.json`, `rRevision.json`, `rSubscribe.json` | `main` **deleted** these paths (cleanup PR #206). `dev` **moved** the same content into `docs/archive/stage-a-legacy/...` and replaced root `read_only.md` with a 21-line forwarding pointer. Git sees `main`'s tree as "absent at `Compact/...`", `dev`'s tree as "present at `docs/archive/stage-a-legacy/Compact/...`", and the merge base as "present at `Compact/...`". | Three-way merge interprets this as "the other side deleted the file we still have" → it will keep `dev`'s archived copies (re-publishing internal workflow content) unless explicitly resolved otherwise. |
| **C2** | `auto-refresh-tasks.json`, `data/courses.sqlite-shm`, `data/courses.sqlite-wal`, `data/fresh_local.db-shm`, `data/poller_checkpoint.json`, `data/runtime/fetch_job_*.json`, `notebooks/incremental_trial.md`, `reports/field_validation*`, `reports/fresh_install_log.md`, `reports/mail_worker_latency.md`, `reports/poller_durability.md` | Same shape: `main` deleted, `dev` either retained, archived (`notebooks/incremental_trial.md` → `reports/incremental_trial.md`), or never had (the `data/*.sqlite-{shm,wal}` and `data/runtime/*.json` artifacts were untracked by Stage A task-007 and exist only as ignored local files). | Same as above. |
| **C2** | `scripts/poller_checkpoint.json` | Inverted shape: `main` **still tracks** it (blob `e7b0fe8...`); `dev` **untracked** it in task-007 and added it to `.gitignore`. A three-way merge will keep `main`'s tracked copy and ignore the dev-side rule. | Surfaces a runtime checkpoint into public commits even though dev's analysis classified it as runtime-only. |
| **C2** | `configs/mail_sender.user.json` | Inverted shape: this file was previously public on the merge base, was deleted on `main` by `c55352b` as a private config, and never reappeared on `dev`. Not actually a merge conflict today, but recorded here because any future workflow that re-introduces a `*.user.json` file would need the same deletion treatment. | Mentioned for policy completeness, not as a present conflict. |
| **C3 · One-sided product diff** | `api/src/routes/scheduled-fetch.ts`, `api/src/services/scheduledFetcher.ts`, `frontend/src/api/scheduledFetch.ts`, `frontend/src/components/AutoRefreshToggle.{tsx,css}`, `frontend/src/components/ScheduledFetchPanel.{tsx,css}`, `frontend/src/hooks/useAutoRefresh.ts`, `frontend/src/hooks/useScheduledFetch.ts` | The Auto-Refresh / Scheduled-Fetch vertical slice was added to `main` after the merge base (`e770bf2`) and was never propagated to `dev`. A three-way merge would cleanly **add** these files to a merged tree. | Not a conflict in the Git sense, but a real product-surface **divergence** that any sanitized public replay must keep visible. Stage A task-005's module-surface map did not assume this slice exists; downstream Stage B work that touches this surface should treat `main` as authority. |
| **C4 · Internal-only exposure on dev** | `.orchestrator/`, `AGENTS.md`, `docs/archive/stage-a-legacy/` | These paths exist only on `dev` (added after the merge base) and `main` has nothing at the same paths to conflict with. A naive three-way merge will introduce all of them into the merged tree. | Not a "conflict" in the Git sense, but the entire reason a raw `dev → main` merge is unsafe: it would publish internal planning artifacts. |

Cumulative classification: dev cannot auto-merge cleanly into main, and even if it
were force-resolved (e.g. `-X theirs` favoring dev), the result would publish the
C4 internal artifacts and re-introduce the C2 archived workflow records that the
prior public cleanup explicitly removed. **A sanitized replay onto a fresh public
candidate branch is therefore the correct approach**, as already required by
`.orchestrator/architecture.md` §"Stage P Public Branch Architecture".

---

## 2. Path-by-path classification (public vs internal)

The classifications below apply specifically to **the future public branch** (the
"public-main-candidate" envisioned by the architecture). They do not require any
modification of `origin/main` or `origin/dev` today; they are the rules that the
next Stage P task will use when constructing that public branch.

Severity column: **must-include** = mandatory public; **may-include** = public if
adapted; **must-exclude** = never public; **must-redact** = include path but with
content sanitization.

### 2.1 Required by AC-002

| Path | Present on `main`? | Present on `dev`? | Public-branch class | Reason |
|---|---|---|---|---|
| `.orchestrator/` (entire subtree, 12 tracked files on dev) | No | Yes | **must-exclude** | ngagent planning, goals, architecture, context manifest, docs/, memory/, stage-a/, and (after this task) stage-p/ are internal coordination state. They reference internal review policy, model-selection rules, and human cutover gates that have no public consumer. |
| `AGENTS.md` (16,727 bytes, dev-only) | No | Yes | **must-exclude** | ngagent v0.8.0 system prompt for the Main Orchestrator Agent. Describes hard constraints, role boundaries, and runtime state under `.git/ngagent/`. Internal automation contract; not public documentation. |
| `docs/archive/stage-a-legacy/` (entire subtree, ~100 tracked files on dev) | No | Yes | **must-exclude** | Stage A archive of historical AI workflow records: `Compact/Compact-ST-*.md`, `review/review-record-*.md`, `Rutgers-dr/*.md`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`, `record.json`, `read_only.md`. The prior public cleanup (PR #206) explicitly deleted the same content, and Stage A only retained it locally as evidence. Re-exposing it on a public branch would invert the prior selective-disclosure decision. |
| Historical workflow records (the same set, viewed as a logical class regardless of path) | Already deleted by PR #206 | Tracked under `docs/archive/stage-a-legacy/` | **must-exclude** | Same reasoning. Adding an alternative-named archive path on the public branch would have the same exposure effect and is forbidden. |
| Local runtime files: `data/*.db`, `data/*.sqlite`, `data/*.db-shm`, `data/*.db-wal`, `data/*.sqlite-shm`, `data/*.sqlite-wal`, `data/migrations.log`, `data/staging/`, `data/runtime/`, `data/poller_checkpoint.json`, `data/refresh_queue.json`, `scripts/poller_checkpoint.json` | `scripts/poller_checkpoint.json` is **tracked** on main (residual from before cleanup); the rest are not tracked on either branch. | None of them tracked on dev (task-007 untracked `scripts/poller_checkpoint.json` and added it to `.gitignore`). | **must-exclude** (and `scripts/poller_checkpoint.json` must be **explicitly removed** from the public branch even though it is currently tracked on `main`). | Runtime state, mutable per-environment. The dev-side analysis (Stage A task-004) and the public cleanup PR #206 agree on the policy; only the actual tracked-vs-ignored state differs. The public branch must align with both intents. |
| Private configs: `configs/*.local.json`, `configs/*.user.json`, `.env`, `.env.*` (except `.env.example` if added later) | `.env`, `.env.*` ignored on both; `configs/*.user.json` was previously committed (`mail_sender.user.json`) and removed by `c55352b`. | Same. | **must-exclude** | These are per-user secret material by definition. The 2026-05-09 cleanup proves that real values were once committed and had to be redacted. The public-branch `.gitignore` must keep these patterns. No public commit may include any new `*.user.json`, `*.local.json`, or `.env*` file other than `.env.example` (which is currently absent and may be added later by a different task — out of scope here). |
| Release artifacts: `release/`, `*.tar.gz`, `*.zip`, root-level `bcsp-*.zip` / `bcsp-*.tar.gz` | Not tracked. | Not tracked. (Stage A task-002 reconciled local `release/bcsp-20260121.tar.gz`, `release/bcsp-20260121.zip`, and `bcsp-20260122.zip` and concluded none should be trusted as current.) | **must-exclude** from the tracked tree; **must-keep-ignored** in `.gitignore`. | Release packs are derivative artifacts and were partially the surface that motivated the original selective-disclosure cleanup. Stage A's release reconciliation explicitly noted a packager-username side-channel risk; even sanitized release zips should be republished from a release pipeline, not committed. |
| `far/` (the nested clone at `far/Rutgers-BetterCourseSchedulePlanner` in the **parent** working tree) | Not tracked anywhere. | Not tracked anywhere. | **must-exclude** from any public commit; **must-add `far/` to public `.gitignore`** to be safe; **must be deleted from local disk after Stage P**, per the architecture plan. | It is a full nested clone of the same repo. Committing it would double the repository size, embed `.git` directories, and re-publish the very `origin/main` content it mirrors. Pinned context confirms: read-only evidence, do not add far/. |

### 2.2 Additional dev-only paths discovered during this audit (also internal)

| Path | Present on `main`? | Present on `dev`? | Public-branch class | Reason |
|---|---|---|---|---|
| `read_only.md` (root, dev-side 21-line forwarding pointer to `docs/archive/stage-a-legacy/read_only.md` and `.orchestrator/stage-a/06-final-baseline.md` and `.orchestrator/stage-a/07-cleanup-application.md`) | No (deleted by PR #206) | Yes (added by task-007 as a forwarding pointer only) | **must-exclude** | The pointer is internally consistent on dev because both targets exist on dev. On a public branch where neither target exists, the pointer becomes a broken link to the audience's eyes and a metadata leak about internal Stage A naming. The simplest correct policy is to inherit `main`'s decision and not include `read_only.md` at all. |
| `reports/incremental_trial.md`, `reports/field_validation.md`, `reports/field_validation_details.mdpart`, `reports/field_validation_samples.json`, `reports/fresh_install_log.md`, `reports/mail_worker_latency.md`, `reports/poller_durability.md` | No (all deleted by PR #206; `reports/` is in `main`'s `.gitignore`). | Yes (`reports/incremental_trial.md` was moved here by task-007; the others remained tracked through Stage A). | **must-exclude** by default; one-by-one **may-include** review allowed for non-internal evidence files. | The prior public cleanup intentionally deleted these and added `reports/` to `.gitignore`. Public exclusion is the conservative default. Some of these (e.g. `field_validation_samples.json`, `fresh_install_log.md`) embed local timestamps and per-machine paths and should certainly not ship publicly without sanitization. If a future task argues a particular file is genuinely public-facing engineering documentation, it should be promoted into `docs/` with redactions, not included from `reports/`. |
| `data/courses.sqlite-shm`, `data/courses.sqlite-wal`, `data/fresh_local.db-shm` | Not tracked (deleted by PR #206). | Not tracked (already untracked on dev tip; existed only as ignored local artifacts before Stage A). | **must-exclude** (already correctly handled on dev). | Same runtime-state reasoning as 2.1. |
| `data/runtime/fetch_job_*.json`, `data/migrations.log` | Not tracked (deleted by PR #206 / never present on dev). | Not tracked. | **must-exclude** (already correctly handled on both). | Per-job runtime/log artifacts. |

### 2.3 Public-surface paths (must-include)

The following paths exist on `dev` (and on `main` where applicable) and form the
project-facing engineering surface that the public branch must include. Listed
here for completeness so that the next Stage P task has a positive include list.

| Path | Class | Notes |
|---|---|---|
| `api/` (`src/`, `tests/`) | **must-include** | Backend Fastify app + tests. |
| `workers/` (`mail_dispatcher.ts`, `open_sections_poller.ts`, `tests/`) | **must-include** | Background workers + tests. |
| `notifications/mail/` (`config.ts`, `providers/`, `retry_policy.ts`, `template_*.ts`, `tests/`, `types.ts`) | **must-include** | Notification subsystem + tests. |
| `frontend/` (excluding `.cache`, `dist`, build outputs which are already ignored) | **must-include** | UI source, i18n messages, package manifests, vite config. |
| `scripts/` (excluding `scripts/poller_checkpoint.json`) | **must-include** for code; **must-exclude** for the runtime checkpoint JSON. | All `scripts/*.ts`, `scripts/*.js`, `scripts/*.py`, `scripts/*.sh` are project-facing tooling. |
| `configs/` — `fetch_pipeline.example.json`, `fetch_pipeline.schema.json`, `mail_sender.example.json`, `templates/email/**/*` | **must-include** | Example configs (verified to use `example.edu` placeholder addresses) and email templates. Critically, the `configs/*.local.json` and `configs/*.user.json` patterns continue to be `.gitignore`'d. |
| `data/migrations/*.sql`, `data/schema.sql` | **must-include** | Schema and migrations are public engineering artifacts. The mutable `data/*.sqlite` and `data/runtime/*` are not. |
| `docs/` (excluding `docs/archive/stage-a-legacy/`) | **must-include**, possibly **may-include**-with-edits for the three runbooks | Public engineering docs. The three runbooks (`docs/data_load_runbook.md`, `docs/data_refresh_strategy.md`, `docs/notify_runbook.md`) reference paths under `reports/` and `notebooks/` that will not exist on the public branch. The next Stage P task must either (a) restore the `main`-side wording that says "kept locally", or (b) keep the `dev`-side wording but accept that the references resolve to absent paths. Either is an editorial decision; both options are acceptable from an exposure-policy perspective. |
| `package.json`, `package-lock.json`, `frontend/package.json`, `frontend/package-lock.json`, `tsconfig.json`, `frontend/tsconfig.json`, `frontend/tsconfig.node.json`, `frontend/vite.config.ts` | **must-include** | Verified identical between `main` and `dev`. No drift. |
| `Start-WebUI.bat`, `Start-WebUI.command` | **must-include** | One-click launchers; identical between `main` and `dev`. |
| `README.md` | **must-include**, but **must choose** between `main`'s short Chinese-first beginner README and `dev`'s longer bilingual technical README | Editorial decision deferred to the next Stage P task. The exposure-policy default is: prefer the shorter `main`-side README because it represents the explicit prior public-surface decision. |
| `.gitignore` | **must-include**, but **must merge intent** of both sides | Should be the union of: dev-side runtime ignores (`data/refresh_queue.json`, `scripts/poller_checkpoint.json`, `.worktrees/`) plus main-side workflow ignores (`Compact/`, `review/`, `Rutgers-dr/`, `reports/`, `notebooks/`, `record.json`, `read_only.md`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`, `auto-refresh-tasks.json`) plus a new entry for `far/` and a new entry for `.git/ngagent/` (defensive). |

---

## 3. Product-facing differences (AC-003, observation only)

This section enumerates real product-surface differences between `origin/main`
and `origin/dev`. **No product file is modified by this task.** The next Stage P
task can use this list to decide what gets sanitized-replayed onto the public
candidate branch.

### 3.1 On `origin/main` but **not** on `origin/dev`

| Path | Origin commit | Class | Public-branch implication |
|---|---|---|---|
| `api/src/routes/scheduled-fetch.ts` (183 lines) | `e770bf2 auto-refresh-tasks` | Backend route — Auto-Refresh / Scheduled Fetch feature | Real product feature missing on dev. The public branch should keep it (it is already public on `main`). |
| `api/src/services/scheduledFetcher.ts` | `e770bf2` | Backend service for the same feature | Same. |
| `frontend/src/api/scheduledFetch.ts` | `e770bf2` | Frontend API client for the same feature | Same. |
| `frontend/src/components/AutoRefreshToggle.tsx` + `.css` | `e770bf2` | Frontend UI for the same feature | Same. |
| `frontend/src/components/ScheduledFetchPanel.tsx` + `.css` | `e770bf2` | Frontend UI for the same feature | Same. |
| `frontend/src/hooks/useAutoRefresh.ts` | `e770bf2` | Frontend hook for the same feature | Same. |
| `frontend/src/hooks/useScheduledFetch.ts` | `e770bf2` | Frontend hook for the same feature | Same. |
| `scripts/poller_checkpoint.json` | Pre-existing tracked runtime file | Local runtime checkpoint accidentally tracked on `main` | Should be **excluded** from the public branch and added to public `.gitignore` (matching dev's task-007 decision). This is the only file in this row that is product-internal and *should not* be on the public branch. |
| `README.md` (`98885c3...` blob, ~211 lines) | `c55352b` | Public-facing README | Public branch should likely use this version (the explicit prior public-surface decision); `dev` retains the older bilingual technical README. |
| `.gitignore` (`b3522ed...` blob) | `7abd5f1` (and earlier) | Public ignore list | Should be merged with `dev`'s additions (see §2.3 row for `.gitignore`). |
| `docs/data_load_runbook.md`, `docs/data_refresh_strategy.md`, `docs/notify_runbook.md` (the `main` revisions referencing "kept locally") | PR #206 | Public docs | Editorial choice in the next Stage P task. |

### 3.2 On `origin/dev` but **not** on `origin/main`

The dev-only product files separable from internal coordination are listed below.
This excludes `.orchestrator/`, `AGENTS.md`, and `docs/archive/stage-a-legacy/`,
which are covered in §2 as internal-only.

| Path | Origin commit | Class | Public-branch implication |
|---|---|---|---|
| `read_only.md` | `dbd27ca` (task-007) | Forwarding pointer to internal archives | Excluded from public; see §2.2. |
| `reports/field_validation.md`, `reports/field_validation_details.mdpart`, `reports/field_validation_samples.json` | Tracked across Stage A | Local validation evidence | Excluded by default; see §2.2. |
| `reports/fresh_install_log.md`, `reports/mail_worker_latency.md`, `reports/poller_durability.md`, `reports/incremental_trial.md` | Tracked across Stage A; `incremental_trial.md` moved here by task-007 | Local evidence reports | Excluded by default; see §2.2. |
| `.gitignore` (`fe8ce7f...` blob) | task-007 | Internal ignore list | Merge into public `.gitignore`. |
| `docs/data_load_runbook.md`, `docs/data_refresh_strategy.md`, `docs/notify_runbook.md` (the `dev` revisions referencing `reports/...` and `notebooks/incremental_trial.md`) | Pre-Stage-A baseline | Public docs | Editorial choice in the next Stage P task. |

There are **no `dev`-only files under `api/`, `workers/`, `notifications/`,
`frontend/src/` (excluding the dev-only frontend hooks/components for the
scheduled-fetch feature that don't exist), `scripts/` (excluding the deleted
`scripts/poller_checkpoint.json`), `configs/`, `data/migrations/`, or
`data/schema.sql`**. In other words, dev does not introduce any new product
surface that `main` lacks. All real product divergence flows in the
`main → dev` direction (the auto-refresh feature on `main` that dev never picked
up).

---

## 4. Public include / exclude policy (AC-001 authoritative rules)

These rules govern the construction of the public-main-candidate branch by
later Stage P tasks.

### 4.1 Include rules (positive)

P-INC-1. **Product source roots**: include `api/`, `workers/`, `notifications/`,
`frontend/`, `scripts/` (entire trees) **except** the explicit excludes in §4.2.

P-INC-2. **Public configuration surface**: include `configs/fetch_pipeline.example.json`,
`configs/fetch_pipeline.schema.json`, `configs/mail_sender.example.json`,
`configs/templates/email/**/*`. (Verified: `mail_sender.example.json` uses
`example.edu` placeholder addresses on both branches.)

P-INC-3. **Schema and migrations**: include `data/schema.sql` and
`data/migrations/*.sql` only. No other `data/*` files.

P-INC-4. **Public docs**: include all of `docs/` **except** `docs/archive/`.
The three runbooks in §3.1 require an editorial choice in the next Stage P
task; both candidate revisions are acceptable from an exposure-policy
perspective.

P-INC-5. **Top-level metadata**: include `package.json`, `package-lock.json`,
`tsconfig.json`, `Start-WebUI.bat`, `Start-WebUI.command`, `README.md`.

P-INC-6. **Public `.gitignore`**: include a `.gitignore` that is the **union**
of: (a) every entry currently on `origin/main`'s `.gitignore`; (b) the
dev-side additions `data/refresh_queue.json`, `scripts/poller_checkpoint.json`,
`.worktrees/`; (c) two new defensive entries: `far/` and `.git/ngagent/`
(redundant for `.git/` but explicit). The public branch's `.gitignore`
intentionally **keeps** the workflow-name ignores from `origin/main` even
though those filenames may not be present, because they are forward defenses
against accidental re-introduction.

P-INC-7. **Sanitized commit history**: prefer multiple sanitized replay
commits on the public candidate branch (per architecture plan), not a single
"squash everything" commit. Each replay commit may carry a short, redacted,
project-facing subject line. Author identity and committer identity for
replay commits should be the human's published identity, not internal agent
identifiers; this is enforced at construction time, not in this audit.

### 4.2 Exclude rules (negative)

P-EXC-1. **ngagent and orchestrator state**: exclude `.orchestrator/` (entire
subtree) and `AGENTS.md`. The runtime store at `.git/ngagent/` is already
inside `.git/` and therefore not tracked, but the public `.gitignore` will
list it defensively.

P-EXC-2. **Stage A archive**: exclude `docs/archive/stage-a-legacy/` (entire
subtree, ~100 files).

P-EXC-3. **Historical workflow records by name** (defense in depth, in case
they ever reappear at any path): exclude `Compact/`, `review/`, `Rutgers-dr/`,
`record.json`, `read_only.md`, `rEmail.json`, `rRevision.json`,
`rSubscribe.json`, `auto-refresh-tasks.json`, `tasks_bugfix.json`. These are
listed in the public `.gitignore` (P-INC-6) so any future commit will skip
them even if filesystem state changes.

P-EXC-4. **Local runtime / generated state**: exclude `data/*.db`,
`data/*.sqlite`, `data/*.db-{shm,wal}`, `data/*.sqlite-{shm,wal}`,
`data/migrations.log`, `data/staging/`, `data/runtime/`,
`data/poller_checkpoint.json`, `data/refresh_queue.json`,
`scripts/poller_checkpoint.json`, `.cache/`, `.parcel-cache/`, `.next/`,
`.nuxt/`, `dist/`, `build/Release/`, `node_modules/`, `coverage/`,
`*.tsbuildinfo`, `*.log`, `logs/`. Almost all of these are already in both
branches' `.gitignore`; the public `.gitignore` keeps them.

P-EXC-5. **Private configs**: exclude `configs/*.local.json`,
`configs/*.user.json`, `.env`, `.env.*` (with the future allowance of
`.env.example` if a separate task adds one).

P-EXC-6. **Release artifacts**: exclude `release/`, `*.tar.gz`, `*.zip`. The
prior packager-username side-channel observed by Stage A task-002 means even
sanitized release zips must be republished by a release pipeline rather than
committed.

P-EXC-7. **Local reports**: exclude `reports/` (matches `main`'s prior
decision). Individual files may be promoted to `docs/` with redactions only
through an explicit later task; the default is exclude.

P-EXC-8. **Notebooks**: exclude `notebooks/` (matches `main`'s prior
decision; `dev` no longer has it after task-007).

P-EXC-9. **Tooling local state**: exclude `.claude/`, `.worktrees/`,
`.vscode-test`, etc. — already in both branches' `.gitignore`.

P-EXC-10. **The nested clone**: exclude `far/` from the public tree and from
the public `.gitignore`-respected scan; the local `far/` directory is to be
deleted from disk after Stage P, per the architecture plan.

P-EXC-11. **The forwarding pointer**: exclude root-level `read_only.md`. The
dev-side pointer references internal paths that will not exist on the public
branch.

P-EXC-12. **Branches and refs**: this audit defines no rule about which Git
**branches** are visible on the remote; default-branch changes, force-pushes,
and remote branch deletion are explicitly **out of scope** for any executor
and require human cutover approval per the architecture plan.

### 4.3 Redaction rules

P-RED-1. **Author and committer identities** in sanitized replay commits
should be the human's published Git identity. Do not reuse the author /
committer values of internal commits, especially those produced by ngagent
or by review automation. (Implementation: deferred to the construction
task.)

P-RED-2. **Commit subjects** in sanitized replay commits should describe
project-facing engineering changes, not internal task IDs (`task-001..task-007`),
ngagent run identifiers, or executor / reviewer model names. (Implementation:
deferred to the construction task.)

P-RED-3. **No content rewrite** of product source, tests, or schemas is
authorized as part of redaction. If a product file must be rewritten for
public exposure, that is a separate task that must be approved at the
architecture layer first.

---

## 5. Negative confirmations (AC-004)

- No file outside `.orchestrator/stage-p/01-public-divergence-and-exposure-policy.md`
  was created or modified by this task. The worktree's `git status` remained at
  "clean working tree on `feature/task-008`" until this single new file was
  written.
- No source file under `api/`, `frontend/`, `workers/`, `notifications/`, `scripts/`,
  `data/migrations/`, `data/schema.sql`, or `configs/` was edited.
- No package manifest (`package.json`, `package-lock.json`, `frontend/package.json`,
  `frontend/package-lock.json`) was edited.
- No runtime config or example config was edited.
- No commit was created on `origin/main` or `origin/dev`. No push was performed
  to any remote.
- The `far/Rutgers-BetterCourseSchedulePlanner` nested clone was not read,
  written, copied, or referenced in any tracked file beyond this report's
  textual mentions.
- Secret-shaped value scan: a positive-pattern scan for SendGrid live tokens
  (`SG\.[A-Za-z0-9_-]{20,}`), Stripe live keys (`sk_live_...`), AWS access keys
  (`AKIA[0-9A-Z]{16}`), GitHub PATs (`ghp_...`), and Slack tokens
  (`xox[baprs]-...`) returned **zero matches** on both `origin/main` and
  `origin/dev`. The `configs/mail_sender.example.json` blob (identical on both
  branches) uses `example.edu` placeholder addresses. The deleted-on-`main`
  `configs/mail_sender.user.json` is not reachable from either current tip.
  This report names file paths and behaviors only; it deliberately does not
  reproduce any historical secret values.

---

## 6. Handoff to next Stage P task

- This document is the authoritative public include/exclude policy for the
  remainder of Stage P. Subsequent Stage P tasks (public candidate construction,
  sanitized replay, three-runbook editorial decision, README selection,
  human-approved cutover) must conform to §4.
- Open editorial decisions (not blockers for this audit, but blockers before
  cutover): (a) choose `main`'s short `README.md` vs `dev`'s longer bilingual
  `README.md`; (b) choose `main`'s "kept locally" runbook wording vs `dev`'s
  `reports/...` references; (c) decide whether to add a `.env.example` (out of
  scope here).
- Out-of-scope but flagged: the auto-refresh feature on `main` (§3.1) is real
  product surface that any later Stage B work touching scheduled fetching must
  treat as authority over Stage A task-005's module-surface map, which was
  written without knowledge of this slice.
- Stage P does **not** modify `origin/main` or `origin/dev` until explicit
  human cutover approval. The next Stage P task constructs a separate
  `public-main-candidate` branch.
