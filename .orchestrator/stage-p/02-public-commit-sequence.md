# Stage P · Public Commit Sequence (Design)

> Stage P task-009 design. Authority: this document defines the **construction
> plan** for the `public-main-candidate` branch, in conformance with the
> include/exclude rules established by Stage P task-008
> (`.orchestrator/stage-p/01-public-divergence-and-exposure-policy.md`, §4).
>
> Scope discipline (re-asserted from the task spec): this task **does not**
> construct the branch, cherry-pick or rewrite any commit, modify
> `origin/main`, modify `origin/dev`, delete `far/`, or perform any cutover.
> The only file written by this task is the present `02-public-commit-sequence.md`.
> Product source under `api/`, `frontend/`, `workers/`, `notifications/`,
> `scripts/`, `data/migrations/`, `data/schema.sql`, `configs/`, public
> branches, package manifests, runtime configs, and `far/` are read-only.
>
> Conformance: where this document quotes a rule from task-008 it cites the
> rule ID (`P-INC-*`, `P-EXC-*`, `P-RED-*`). Where it makes a new construction
> decision it labels it `P-BUILD-*` and explains the rationale.

---

## 1. Candidate branch identity

| Field | Value | Evidence |
|---|---|---|
| **Branch name** | `public-main-candidate` | `.orchestrator/architecture.md` §"Stage P Public Branch Architecture", lines 96–98 (verbatim): *"Temporary reviewed candidate branch for replacing/updating main after human cutover approval."* |
| **Base commit** | `98748e1466ec55e93115f651a6a84f84e227daa9` (`98748e1`, `Merge pull request #206 from VVittgenstein/clean-public-artifacts-20260509`) | Current `origin/main` tip (`git rev-parse origin/main` at this audit time). Verified merge base of `origin/main` and `origin/dev` is `2d762179025e68ee05f853c3e7b5e8b43837893c` (`2d76217`); main is 7 commits ahead of the merge base and dev is 25 ahead. |
| **Construction host** | The same local clone hosting `origin/main` and `origin/dev`. Constructor must `git fetch origin` immediately before branching, then `git switch -c public-main-candidate 98748e1466ec55e93115f651a6a84f84e227daa9`. | The local clone is the only host with both branches in scope. The nested `far/Rutgers-BetterCourseSchedulePlanner` clone is **not** used as the host (read-only evidence only, per task-008 §0 and pinned context). |
| **Pushed?** | Not by this task or by the construction task. Pushing the candidate branch to the remote is a separate, human-approved cutover action (task-008 §4.2 P-EXC-12 + `.orchestrator/architecture.md` Stage P §"execution policy" item 4). | — |
| **Default branch change** | Out of scope. | Same citation. |

### 1.1 Source evidence (concrete blobs and refs)

The construction task will reference exactly these objects. Listed here so a
reviewer can verify the plan against the same evidence the construction task
would consume.

| Reference | SHA | Role |
|---|---|---|
| `origin/main` tip | `98748e1466ec55e93115f651a6a84f84e227daa9` | Base commit (P-BUILD-1). |
| `origin/dev` tip | `88c09d8753275c733417fbb237182c5072209aa2` (= `feature/task-008` head at audit time) | Source for the two ignore additions in P-COMMIT-1 and P-COMMIT-2. |
| Merge base | `2d762179025e68ee05f853c3e7b5e8b43837893c` | Audit reference; not a construction input. |
| `origin/main:.gitignore` blob | `b3522eda3134bef75f76c082c183a80e5b4b399a` | Starting `.gitignore`; carried forward as part of the base tree. |
| `origin/dev:.gitignore` blob | `fe8ce7f58e173f1533cc9bc6e3ab07ed947cc283` | Source of the runtime-hygiene additions (P-COMMIT-1) and the `.worktrees/` addition (P-COMMIT-2). |
| `origin/main:README.md` blob | `7763643f0785068413e865aeeb812370bdd6e370` | Public README at base. Carried forward unchanged by this commit sequence. Editorial swap to the dev-side blob (`98885c338db99e0ecbf007147617f2c000b46905`) is **deferred** (task-008 §6, this document §4.3). |
| `origin/main:scripts/poller_checkpoint.json` blob | tracked on main; to be removed from the index by P-COMMIT-1 (`git rm --cached`). No content of this blob is reproduced in this document. | — |

### 1.2 Why build-on-main and not orphan-replay

Two construction strategies were considered. The chosen strategy is
**build-on-main**; the rejected alternative is recorded so future Stage P
work has the rationale.

| Strategy | What it means | Pros | Cons | Decision |
|---|---|---|---|---|
| **Build-on-main** (chosen, `P-BUILD-1`) | Branch from `origin/main` tip (`98748e1`); add sanitized commits on top. | Preserves the seven public-safe commits already on `origin/main` since the merge base (`e770bf2`, `b650d81`, `c55352b`, `6863093`, `45455c1`, `7abd5f1`, `98748e1`). These are themselves task-style PR commits with the human's published identity. The Auto-Refresh / Scheduled Fetch product slice (task-008 §3.1, pinned context) is preserved by construction, not by reapplication. The candidate diverges from `main` by only the two small sanitized additions, making review trivial. | Inherits one residual leak — `scripts/poller_checkpoint.json` is tracked on `main` (audit §3.1). This is corrected by P-COMMIT-1. | **Adopt.** |
| Orphan replay | Start from an orphan commit; replay a synthetic "engineering layers" commit series (e.g. backend, workers, notifications, frontend, scripts, configs, schema, docs, auto-refresh, launchers, README, .gitignore). | Maximum control over the visible history. Eliminates any chance of inheriting a residual leak from `main`. | Loses the seven existing public commits and their authentic identities/dates. Re-publishes the auto-refresh slice as a freshly-authored commit rather than as `e770bf2`, breaking traceability for any external consumer who already pulled `main`. Synthesizes engineering history that did not actually occur in that order, which contradicts the "preserve multiple meaningful public commits" intent of `.orchestrator/architecture.md` Stage P §"branch rules" item 4 — *meaningful* implies authentic. | **Reject.** |

`P-BUILD-1`: the `public-main-candidate` branch is constructed by checking
out `origin/main` at `98748e1` and applying sanitized commits on top. No
history before `98748e1` is rewritten by this task or by the construction
task that consumes it.

---

## 2. Intended commit list

The candidate branch terminates after **two mandatory sanitized commits** on
top of the base. Three further editorial / scope-deferred candidate commits
are documented for completeness; they are **not** part of the mandatory
sequence and are not produced by Stage P unless a separate task is opened.

Visible-history math:
- 7 task-style commits already on `origin/main` since the merge base
  (`e770bf2` … `98748e1`) — preserved by construction.
- + 2 new mandatory sanitized commits (`P-COMMIT-1`, `P-COMMIT-2`).
- = 9 task-style commits visible on the candidate branch since the merge base.

This satisfies the architecture's "preserve multiple meaningful public
commits" requirement without inventing synthetic history.

---

### P-COMMIT-1 · Align public runtime hygiene with dev cleanup decision

| Field | Value |
|---|---|
| **Purpose** | Untrack the single residual runtime checkpoint that survived the prior public-cleanup PRs (`scripts/poller_checkpoint.json`, tracked at base) and merge the two runtime-related ignore additions from `origin/dev`'s `.gitignore` into the public `.gitignore`. Brings the public branch into alignment with the runtime-hygiene policy reached by Stage A task-007 on the dev side. |
| **Suggested subject** | `chore: untrack stray runtime checkpoint and tighten runtime ignores` |
| **Suggested body** | One paragraph noting that `scripts/poller_checkpoint.json` is a per-environment poller state file, not a tracked artifact, and that `data/refresh_queue.json` is the symmetric per-environment queue state. No reference to task IDs, run IDs, executor names, or reviewer names (per task-008 §4.3 `P-RED-2`). |
| **Author / committer identity** | The human's published Git identity (the same identity that authored `c55352b`, `45455c1`, `7abd5f1`, etc.). Not an ngagent or executor identifier. Per task-008 §4.3 `P-RED-1`. |
| **Included paths (write)** | `.gitignore` — append two new lines under the existing "Local database artifacts" block, in this order: `data/refresh_queue.json`, `scripts/poller_checkpoint.json`. The block already on `main` (e.g. `data/poller_checkpoint.json`, `data/runtime/`, `data/staging/`) is left untouched. |
| **Included paths (remove from index)** | `scripts/poller_checkpoint.json` — removed via `git rm --cached scripts/poller_checkpoint.json` so the local file is preserved as ignored. No `git rm -f`; no on-disk delete. |
| **Excluded paths in this commit** | All other paths. Specifically, no edits to: `api/**`, `frontend/**`, `workers/**`, `notifications/**`, `scripts/**` (other than the index removal above), `configs/**`, `data/migrations/**`, `data/schema.sql`, `docs/**`, `package.json`, `package-lock.json`, `frontend/package.json`, `frontend/package-lock.json`, `tsconfig.json`, `frontend/tsconfig.json`, `Start-WebUI.bat`, `Start-WebUI.command`, `README.md`. No new files under `.orchestrator/`, `AGENTS.md`, `docs/archive/`, `reports/`, `notebooks/`, `release/`, `far/`. |
| **Rationale (preserves visible task-style activity)** | This is a real, narrowly-scoped engineering cleanup commit — exactly the shape of `45455c1` and `7abd5f1` already on `main`. It documents a single hygiene decision that is genuinely missing from `main` (the residual checkpoint), and it does so without re-publishing dev's internal Stage A reports. The change is auditable in 3 lines of diff. |
| **Source evidence** | (a) The blob `e7b0fe8…` is tracked at `origin/main:scripts/poller_checkpoint.json`; (b) `origin/dev:.gitignore` (blob `fe8ce7f…`) ignores both `scripts/poller_checkpoint.json` and `data/refresh_queue.json`; (c) Stage A task-007 (`.orchestrator/stage-a/07-cleanup-application.md`) documented these as the deliberate runtime-hygiene decisions. Citations: task-008 audit §1.2 row C2 (inverted shape) and §2.1 row "Local runtime files". |
| **Conformance to task-008 policy** | Satisfies `P-EXC-4` (local runtime / generated state — exclude). Satisfies `P-INC-6` clauses (b) and (c) for the dev-side ignore additions. Does **not** add any of the optional defensive ignores from §4.2 — those are deferred to P-COMMIT-2. |

---

### P-COMMIT-2 · Add defensive ignores for nested clone, ngagent state, and worktree root

| Field | Value |
|---|---|
| **Purpose** | Add three forward-defense ignore rules to the public `.gitignore` so that the directories enumerated in task-008 §4.1 `P-INC-6` clause (c) and §4.2 `P-EXC-10`, `P-EXC-9` cannot reach a future public commit even if they appear locally. None of the three is currently tracked on any branch. |
| **Suggested subject** | `chore: add defensive ignores for nested clone and local agent scaffolding` |
| **Suggested body** | One paragraph noting that `far/` is a same-repo nested clone used only as local evidence, `.git/ngagent/` is local agent state stored inside the Git directory, and `.worktrees/` is the root for local multi-worktree development. No reference to task IDs, run IDs, executor names, or reviewer names. |
| **Author / committer identity** | The human's published Git identity (`P-RED-1`). |
| **Included paths (write)** | `.gitignore` — append three new lines under a new comment block titled e.g. `# Local agent scaffolding and nested clones`, in this order: `far/`, `.git/ngagent/`, `.worktrees/`. The `.git/ngagent/` entry is technically redundant (everything inside `.git/` is already excluded from tracking) but is included explicitly as a defense against any worktree-relative tooling that scans relative paths. |
| **Included paths (remove from index)** | None. No path is currently tracked under `far/`, `.git/ngagent/`, or `.worktrees/` on either `origin/main` or `origin/dev`; no `git rm --cached` is required. |
| **Excluded paths in this commit** | All paths other than `.gitignore`. Same blocklist as P-COMMIT-1. |
| **Rationale (preserves visible task-style activity)** | A focused "defensive ignore" commit with a clear engineering message is a normal hygiene PR and matches the shape of `7abd5f1`. It deliberately does **not** combine with P-COMMIT-1 so that reviewers can verify the runtime-hygiene change (which removes an existing tracked file) separately from the forward-defense change (which adds rules for paths nobody tracks). |
| **Source evidence** | (a) The pinned Main-Agent context for this task: *"Public .gitignore should merge main and dev intent and add defensive `far/` and `.git/ngagent/` ignores."*; (b) task-008 §4.1 `P-INC-6` clause (c); (c) task-008 §4.2 `P-EXC-9` for `.worktrees/`; (d) `.orchestrator/context_manifest.md` Known Gotcha [2026-05-11] noting `far/` is read-only evidence to be deleted post-Stage-P. |
| **Conformance to task-008 policy** | Satisfies `P-INC-6` (c). Satisfies `P-EXC-9` for `.worktrees/`. Satisfies `P-EXC-10` for `far/` (defensive-ignore aspect; deletion of the local `far/` directory is **explicitly out of scope of this task and of the construction task that consumes this plan** — see §3 below). |

---

### Editorial / deferred candidates (not part of the mandatory sequence)

The following three candidate commits are documented so the construction
task can refer to them if a follow-up plan opens them. They are **not**
produced by Stage P unless a separate task is opened, and the construction
task that consumes this plan should not invent any of them on its own.

#### P-DEFERRED-A · README selection

- Purpose (if opened): replace the public `README.md` blob `7763643f…`
  (current `origin/main`) with the dev-side `98885c33…` blob, or vice
  versa.
- Status: **deferred**. Task-008 §6 explicitly classifies this as an open
  editorial decision. The exposure-policy default is to keep the current
  `origin/main` README because it is the existing public-surface decision;
  swapping it requires an explicit later task. Note: task-008 §1.1 described
  `c55352b` as rewriting the README into "a shorter Chinese-first beginner
  guide"; the actual `c55352b` diff is +14 / −5 lines and does **not**
  rewrite the file. The dev-side blob is the more compact Chinese-first
  beginner guide; the main-side blob is the longer bilingual technical
  document. This is a labeling correction; it does not change the editorial
  deferral.
- Construction-task behavior: **do nothing**. The base commit's `README.md`
  is carried forward as-is.

#### P-DEFERRED-B · Runbook wording

- Purpose (if opened): replace `docs/data_load_runbook.md`,
  `docs/data_refresh_strategy.md`, `docs/notify_runbook.md` with the
  dev-side blobs that reference concrete `reports/*` and
  `notebooks/incremental_trial.md` filenames.
- Status: **deferred**. Task-008 §6 classifies this as an open editorial
  decision. Note: dev's runbook revisions reference paths under `reports/`
  (which is ignored on the public branch per `P-EXC-7`) and under
  `notebooks/` (which is ignored per `P-EXC-8`). Adopting dev's wording on
  the public branch would produce documentation that points at paths
  guaranteed to be absent. Main's "kept locally" wording is consistent with
  the public branch's ignore rules and is therefore the safer default.
- Construction-task behavior: **do nothing**. The base commit's three runbook
  blobs are carried forward as-is.

#### P-DEFERRED-C · `.env.example`

- Purpose (if opened): add a tracked `.env.example` file enumerating safe
  default environment variable names with empty / placeholder values.
- Status: **deferred** and **out of Stage P scope**. Task-008 §2.1 row
  "Private configs" and §6 both explicitly defer this to a separate task.
- Construction-task behavior: **do nothing**.

---

## 3. Path-class handling rules

This section answers AC-003 explicitly: how each class of path must be
handled by the construction task that consumes this plan. The rules below
are construction-task rules. They are **not** executed by this task.

### 3.1 `origin/main` product-facing changes (preserve as-is)

| Surface | Source on base | Handling |
|---|---|---|
| Auto-Refresh / Scheduled Fetch slice — `api/src/routes/scheduled-fetch.ts`, `api/src/services/scheduledFetcher.ts`, `frontend/src/api/scheduledFetch.ts`, `frontend/src/components/AutoRefreshToggle.{tsx,css}`, `frontend/src/components/ScheduledFetchPanel.{tsx,css}`, `frontend/src/hooks/useAutoRefresh.ts`, `frontend/src/hooks/useScheduledFetch.ts` | Already on the base commit (`98748e1`), introduced by `e770bf2 auto-refresh-tasks` and merged by `b650d81`. | **Preserve unchanged.** The pinned Main-Agent context for this task requires preserving the slice. P-COMMIT-1 and P-COMMIT-2 do not touch any of these paths. |
| Backend modules — `api/src/**` (excluding the auto-refresh files above) | On base. | **Preserve unchanged.** |
| Workers — `workers/mail_dispatcher.ts`, `workers/open_sections_poller.ts`, `workers/tests/**` | On base. | **Preserve unchanged.** |
| Notifications — `notifications/mail/**` | On base. | **Preserve unchanged.** |
| Frontend — `frontend/src/**` (other than auto-refresh files above), `frontend/i18n/**`, `frontend/package.json`, `frontend/package-lock.json`, `frontend/tsconfig*.json`, `frontend/vite.config.ts`, `frontend/index.html`, `frontend/README.md` | On base. Verified identical between `main` and `dev` for the manifests (task-008 §2.3). | **Preserve unchanged.** |
| Scripts (TypeScript / JavaScript / Python / shell, *.ts / *.js / *.py / *.sh) | On base. | **Preserve unchanged.** Only `scripts/poller_checkpoint.json` is removed from the index, by P-COMMIT-1. |
| Top-level metadata — `package.json`, `package-lock.json`, `tsconfig.json`, `Start-WebUI.bat`, `Start-WebUI.command` | On base. Manifests verified identical between `main` and `dev` (task-008 §2.3). | **Preserve unchanged.** |
| Schema and migrations — `data/schema.sql`, `data/migrations/*.sql` | On base. | **Preserve unchanged.** |
| Public configuration surface — `configs/fetch_pipeline.example.json`, `configs/fetch_pipeline.schema.json`, `configs/mail_sender.example.json`, `configs/templates/email/**` | On base. Placeholder verified (`example.edu`) on both branches (task-008 §2.3, §5). | **Preserve unchanged.** |

### 3.2 Dev-only project files (handling)

These exist only on `origin/dev` and must NOT be added to the candidate
branch by the construction task.

| Path / class | Source on dev | Handling |
|---|---|---|
| `.orchestrator/` (entire subtree, 12+ tracked files on dev as of task-008 audit, +1 from task-008 itself, +1 from this task) | Dev only. | **Do not include.** Construction must not `git checkout origin/dev -- .orchestrator/...` or otherwise resurrect these paths. Per `P-EXC-1`. |
| `AGENTS.md` (root, 16,727 bytes) | Dev only. | **Do not include.** Per `P-EXC-1`. |
| `docs/archive/stage-a-legacy/` (entire subtree, ~100 files) | Dev only. | **Do not include.** Per `P-EXC-2` and `P-EXC-3`. |
| `read_only.md` (root, 21-line forwarding pointer on dev) | Dev only (added by `dbd27ca`). | **Do not include.** Per `P-EXC-11`. The base commit's tree already lacks this path because PR #206 deleted it. |
| `reports/incremental_trial.md`, `reports/field_validation.md`, `reports/field_validation_details.mdpart`, `reports/field_validation_samples.json`, `reports/fresh_install_log.md`, `reports/mail_worker_latency.md`, `reports/poller_durability.md` | Dev only (some moved by task-007, some pre-Stage-A). | **Do not include.** Per `P-EXC-7`. The base commit's tree already lacks these paths because PR #206 deleted the directory and added `reports/` to `.gitignore`. |
| `notebooks/incremental_trial.md` (pre-task-007 dev path) | Dev only, then moved to `reports/incremental_trial.md` by task-007. | **Do not include.** Per `P-EXC-8`. The base commit's tree already lacks `notebooks/`. |
| `frontend/src/dev/ComponentPlayground.tsx`, `frontend/src/dev/mockData.ts` | Present on both branches (frontend dev surfaces); base already includes them. | **Preserve unchanged** as part of the carried-forward base tree. They are inside `frontend/src/dev/` which task-007 left tracked. No construction action required. |
| Dev-side runbook revisions (`docs/data_load_runbook.md`, `docs/data_refresh_strategy.md`, `docs/notify_runbook.md` with `reports/...` references) | Dev only. | **Do not include.** See `P-DEFERRED-B`. Base's runbook blobs are carried forward unchanged. |
| Dev-side `README.md` (blob `98885c33…`) | Dev only. | **Do not include.** See `P-DEFERRED-A`. Base's README blob is carried forward unchanged. |
| Dev-side `.gitignore` additions for the runtime-hygiene patterns (`data/refresh_queue.json`, `scripts/poller_checkpoint.json`) | Dev only. | **Include**, but applied as a fresh sanitized commit (P-COMMIT-1), not by cherry-picking dev's commit. |
| Dev-side `.gitignore` addition for `.worktrees/` | Dev only. | **Include**, applied via P-COMMIT-2 alongside the new `far/` and `.git/ngagent/` defensive entries. |
| Dev-side `.gitignore` removals of the historical workflow-name ignores (`Compact/`, `review/`, etc.) | Dev removed these because dev archived the files. | **Do not adopt.** The public branch must keep main's workflow-name ignores (`P-EXC-3` is satisfied by the existing ignore lines on the base). |

### 3.3 Docs (handling)

| Path | Handling |
|---|---|
| `docs/` (entire subtree) on base | **Preserve unchanged.** Base already excludes `docs/archive/stage-a-legacy/` because that directory was never on main. |
| `docs/archive/` | **Do not include** at any sub-path. Per `P-EXC-2`. |
| `docs/data_load_runbook.md`, `docs/data_refresh_strategy.md`, `docs/notify_runbook.md` | **Preserve base wording.** See `P-DEFERRED-B`. |
| Future public docs (e.g. a `docs/CONTRIBUTING.md` or `docs/security.md`) | Out of scope for this commit sequence. May be added by later tasks; must follow the same include/exclude rules. |

### 3.4 Reports, notebooks, release artifacts (handling)

| Path / class | Handling |
|---|---|
| `reports/` (top-level) | **Do not include.** Per `P-EXC-7`. Base already excludes it (PR #206 deleted, `.gitignore` covers). |
| `notebooks/` (top-level) | **Do not include.** Per `P-EXC-8`. Base already excludes it. |
| `release/` (top-level) and `*.tar.gz`, `*.zip` | **Do not include.** Per `P-EXC-6`. Base already ignores. Release-pack republication is a release-pipeline concern, not a Git-commit concern. |
| Root-level archives previously seen on the local main checkout (`bcsp-20260121.tar.gz`, `bcsp-20260121.zip`, `bcsp-20260122.zip`, etc., per Stage A task-002) | **Do not include.** Already covered by the `*.tar.gz` / `*.zip` rules in the base `.gitignore`. |

### 3.5 Ignore rules (merged intent — already enumerated in §2)

The final candidate `.gitignore` (after P-COMMIT-1 and P-COMMIT-2) is the
**union** of:

- All entries from `origin/main:.gitignore` (blob `b3522eda…`) — carried
  forward by the base commit. This includes the workflow-name ignores
  (`Compact/`, `review/`, `Rutgers-dr/`, `reports/`, `notebooks/`,
  `record.json`, `read_only.md`, `rEmail.json`, `rRevision.json`,
  `rSubscribe.json`, `auto-refresh-tasks.json`, `tasks_bugfix.json`) which
  are kept as forward defenses even though the files are not present.
- Two runtime-hygiene entries added by P-COMMIT-1: `data/refresh_queue.json`,
  `scripts/poller_checkpoint.json`.
- Three defensive entries added by P-COMMIT-2: `far/`, `.git/ngagent/`,
  `.worktrees/`.

The candidate `.gitignore` therefore satisfies `P-INC-6` in full and does
not need any further edit before cutover.

### 3.6 Identity rules for sanitized commits

Per task-008 §4.3:

- `P-RED-1` is satisfied by both P-COMMIT-1 and P-COMMIT-2 using the human's
  published Git identity (`VVittgenstein <adrianyuanzhengze@gmail.com>` — the
  same identity that authored `c55352b`, `45455c1`, `7abd5f1`, `98748e1`, and
  `e770bf2`). The construction task must `unset` or re-configure any local
  `user.email` / `user.name` overrides that would otherwise inject an
  ngagent / executor identity.
- `P-RED-2` is satisfied by the suggested subjects in §2 (no task IDs, no
  run IDs, no model names, no reviewer names).
- `P-RED-3` is automatically satisfied because P-COMMIT-1 and P-COMMIT-2
  modify no product source, tests, or schemas.

---

## 4. Explicit out-of-scope actions

Re-stated for AC-004 and for the construction task that consumes this plan.

1. **No cutover.** This task does not switch `origin/main`'s default branch
   to `public-main-candidate`, does not delete or rename `origin/main`, does
   not push the candidate to `origin`, and does not force-push anywhere.
   Cutover is a human-approved action per `.orchestrator/architecture.md`
   Stage P §"execution policy" item 4. The construction task that consumes
   this plan **also** does not perform cutover; cutover is a separate later
   step gated on human approval and on GPT-5.5 xhigh review of the
   constructed candidate.

2. **No deletion of `far/`.** The local nested clone at
   `far/Rutgers-BetterCourseSchedulePlanner` is **not** removed by this
   task. Per `.orchestrator/context_manifest.md` (Known Gotcha
   [2026-05-11]) and `.orchestrator/architecture.md` Stage P §"branch
   rules" item 5, the local `far/` directory is to be deleted after Stage
   P **completes**, i.e. after construction, review, and human cutover —
   none of which are performed by this task or by the immediate downstream
   construction task. P-COMMIT-2 adds `far/` to the public `.gitignore` as
   a forward defense; this is the only `far/`-related action of this task,
   and it touches only one `.gitignore` line, not any file under `far/`.

3. **No branch construction.** This task does not execute `git switch -c
   public-main-candidate`, does not author P-COMMIT-1 or P-COMMIT-2, and
   does not produce any Git object beyond the present file. Construction
   is a separate later task (the next Stage P task after this one).

4. **No modification of `origin/main` or `origin/dev`.** Neither branch is
   force-pushed, fast-forwarded, rebased, reset, or rewritten by this
   task. Both branches retain the SHAs cited in §1.

5. **No modification of product source, tests, package manifests, runtime
   configs, or `far/`.** Re-stated below in §5.

6. **No release-pack republication.** Even sanitized release zips are out
   of scope; they are a release-pipeline concern (`P-EXC-6`).

7. **No README, runbook, or `.env.example` edits.** See `P-DEFERRED-A`,
   `P-DEFERRED-B`, `P-DEFERRED-C`.

---

## 5. Negative confirmations (AC-005)

Re-asserted scope discipline:

- The only file written by this task is
  `.orchestrator/stage-p/02-public-commit-sequence.md` (the present file).
- No file under `api/`, `frontend/`, `workers/`, `notifications/`,
  `scripts/`, `data/migrations/`, `data/schema.sql`, or `configs/` was
  modified.
- No package manifest (`package.json`, `package-lock.json`,
  `frontend/package.json`, `frontend/package-lock.json`) was modified.
- No runtime config or example config was modified.
- `.gitignore` was **not** modified by this task. (P-COMMIT-1 and P-COMMIT-2
  describe edits to `.gitignore` that will be made by the **construction
  task**, not by this task.)
- No commit was created on `origin/main`, on `origin/dev`, or on any public
  branch. No `public-main-candidate` branch was created. No push was
  performed to any remote.
- No path under `far/` was read, written, copied, or referenced.
- `scripts/poller_checkpoint.json` was not removed from the index by this
  task. (P-COMMIT-1 describes that removal as a construction-task action.)
- No secret-shaped value was reproduced in this document. The scan-pattern
  list from task-008 §5 is not repeated here; the construction task should
  re-run the same pattern scan against the constructed candidate tree
  before cutover, but that is a construction-time concern.

---

## 6. Handoff to construction (next Stage P task)

The next Stage P task consumes this plan and produces the actual
`public-main-candidate` branch. That task is expected to:

1. `git fetch origin` to ensure `98748e1` and `88c09d8` are local.
2. `git switch -c public-main-candidate 98748e1466ec55e93115f651a6a84f84e227daa9`.
3. Author P-COMMIT-1 exactly as §2 specifies — single `.gitignore` diff
   (two appended lines) plus `git rm --cached scripts/poller_checkpoint.json`.
4. Author P-COMMIT-2 exactly as §2 specifies — single `.gitignore` diff
   (three appended lines under a new comment block).
5. Re-run the secret-shaped-value scan from task-008 §5 against the final
   candidate tree and append the result to the construction task's report.
6. Run a tree-equivalence check between the final candidate tree and
   `origin/main`'s tree minus `scripts/poller_checkpoint.json` plus the
   five new `.gitignore` lines; nothing else should differ.
7. **Stop.** Do not push, do not open a PR, do not change the default
   branch, do not delete `origin/main`, do not delete `far/`. Emit a
   completion report and yield to review (GPT-5.5 xhigh per the Stage P
   execution policy).

The construction task's reviewer must verify:

- The candidate branch's commit count since `98748e1` is exactly 2.
- The two new commits have the human's published identity.
- The two new commits' subject lines contain no internal task IDs, run
  IDs, model names, or reviewer names.
- The candidate tree matches §3.1's "preserve unchanged" list exactly
  (file-by-file SHA equality with `origin/main`'s tree, except for the
  one removed `scripts/poller_checkpoint.json` and the five added
  `.gitignore` lines).
- The candidate tree contains nothing under `.orchestrator/`, `AGENTS.md`,
  `docs/archive/`, `reports/`, `notebooks/`, `far/`, or any `*.user.json`
  / `*.local.json` / `.env*` (except a future `.env.example` if one is
  added by a separate task).

Cutover decisions (default-branch change, force-push, remote branch
deletion, deletion of local `far/`) remain explicitly human-gated and are
not delegated to either the construction task or its reviewer.
