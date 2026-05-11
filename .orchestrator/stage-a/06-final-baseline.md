# Stage A — Final Baseline Packet and Stage B Entry Plan

> **Task:** `task-006` Stage A final baseline packet and Stage B entry plan
> **Branch:** `feature/task-006`
> **Worktree:** `Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\task-006`
> **Depends on:** `task-001`..`task-005` (all merged into `dev` and pushed to `origin`).
> **Scope:** Audit/synthesis only. The only file written by this task is this document. No product source, tests, runtime data, configs, package files, `.gitignore`, or prior Stage A reports are modified. This document does not implement any refactor; it defines the entry conditions for Stage B.

## 0. Purpose

This is the **final baseline packet** for Stage A. It synthesizes the five upstream reports (`01-inventory.md` through `05-module-surface-map.md`) plus the `.orchestrator/context_manifest.md` handoff notes into a single trustworthy current-state snapshot, declares a source-of-truth hierarchy that Stage B must use, lists candidates for archival or untracking, incorporates the review notes recorded against task-002 and task-003, and proposes the Stage B refactor entry plan (workstreams, first candidates, rationale, sequencing, blast radius, dependencies).

This document is **not** a cleanup application; cleanup is the separate `task-007` (`07-cleanup-application.md`). It is also **not** a Stage B work item; Stage B begins after this baseline is accepted and human approval is given for any non-product cleanup.

## 1. Method

Evidence collection used:

- Direct reads of `.orchestrator/stage-a/01-inventory.md`, `02-release-reconciliation.md`, `03-record-reconciliation.md`, `04-runtime-config-hygiene.md`, `05-module-surface-map.md`, and `.orchestrator/context_manifest.md`.
- Cross-checks against `.orchestrator/goals.md` and `.orchestrator/architecture.md` for scope rules.
- Cross-references against the pinned context block from the Main Agent (review notes for task-002 and task-003).
- `git log --oneline` and `git status` in this worktree to confirm upstream merges and a clean tree before producing the synthesis.

No new filesystem traversal outside `Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\task-006\` was performed for this task; every claim is sourced from one of the five upstream reports above. Where this report cites a finding, the cite identifies the upstream report and the relevant section.

**No real secret values are reproduced anywhere in this document.** The placeholder shape of `configs/mail_sender.user.json`, the SendGrid env reference (`apiKeyEnv: "SENDGRID_API_KEY"`), the SMTP env reference (`passwordEnv: "SMTP_PASSWORD"`), and the runtime checkpoint internals are described structurally only.

## 2. Evidence-Layer Discipline (Carried Forward)

Stage A's evidence-layer rule, established in `01-inventory.md` §2 and reaffirmed by `02..05`, is preserved as the binding rule of this baseline:

| Layer | What it shows | Authority caveat |
| --- | --- | --- |
| `.gitignore` rules | Stated intent to exclude paths. | A pattern's existence does **not** prove correct exclusion (legacy `git add` predating the rule still wins). |
| Tracked state (`git ls-tree`, `git ls-files`) | What HEAD records as committed. | Tracking can encode wrong intent (a runtime artifact may be tracked simply because no one ever ran `git rm --cached`). |
| Ignored state (`git status --ignored`, `git check-ignore`) | What git would currently exclude. | Per-working-tree. Reports a *match*, not an *endorsement*. Already-tracked paths are unaffected. |
| Untracked state | Per-worktree; differs across worktrees and machines. | Absence in one worktree is not absence in the main checkout. |
| Remote state (`git ls-remote`, GitHub) | What has been published. | The remote was curated from "everything submitted" toward "only the project submitted" (Context Manifest, 2026-05-11); remote presence/absence is selective evidence, not authority. |
| Archive manifest / archive content (added by `02-release-reconciliation.md` §2) | What was packaged into a `.tar.gz` / `.zip` at packaging time. | An archive can be built from any working tree at any moment; presence in a pack is not proof of presence on a branch today. |
| Legacy planning JSON / Compact / review / DR / Obsidian (added by `03-record-reconciliation.md` §2) | Self-reported task graphs and narratives authored by an earlier AI workflow. | Status fields and Compact narratives describe what an agent *said* at one point in time; they are evidence, not authority over current code. |

**Hard rule for this baseline:** `dev` (the canonical branch) and the in-tree code as observed by `git ls-files` on `dev` are treated as the **product surface authority** for Stage A's purposes (per the architecture: Stage B refactors `dev`). All other layers — `.gitignore`, ignored state, untracked state, remote state, archive manifests, legacy JSON statuses, Compact/review narratives, DR documents, Obsidian prompts, `read_only.md` — are evidence layers feeding into reconciliation, never authorities. This baseline is allowed to declare a source-of-truth hierarchy for Stage B (see §4) precisely because every upstream report has stripped each evidence layer of any implicit authority claim.

## 3. Synthesis of Upstream Reports

This section condenses what the five Stage A reports established. Each subsection cites the upstream sources; nothing here is a new finding.

### 3.1 Inventory (from `01-inventory.md`)

- 262 tracked files on `dev`/`feature/task-001`. Top-level breakdown: `Compact/` (74), `review/` (6), `reports/` (6), `Rutgers-dr/` (2), `docs/` (23). No tracked `*.tar.gz`, `*.zip`, or `release/`-prefixed paths on any examined branch. (`01-inventory.md` §3.)
- Categories established: product source / tests / current docs / historical AI workflow records / reports / runtime artifacts (tracked despite ignore rules) / local-private config / release-archive (main-checkout local) / legacy planning JSON / unknowns. (`01-inventory.md` §4.)
- The five evidence layers were defined and the "ignored is not endorsement" rule was set. (`01-inventory.md` §2.)
- Six tracked paths matching `.gitignore` rules were flagged (the runtime artifacts under `data/` and `configs/`). One inverse case was flagged: `scripts/poller_checkpoint.json` carries a runtime shape but is not matched by any ignore rule. (`01-inventory.md` §4.6, §4.10.)
- Secret-risk surface: `configs/mail_sender.user.json` is currently tracked despite `configs/*.user.json` being ignored, and the WebUI write-path will overwrite it on the next save. (`01-inventory.md` §4.7, §6.)
- Main-checkout-only release surfaces (`release/`, `bcsp-20260122.zip`) were enumerated by main-checkout evidence supplied by the Main Agent. (`01-inventory.md` §4.8.)

### 3.2 Release reconciliation (from `02-release-reconciliation.md`)

- Three release archives observed in the main checkout: `release/bcsp-20260121.tar.gz`, `release/bcsp-20260121.zip`, and `bcsp-20260122.zip`. (`02-release-reconciliation.md` §3.)
- `dev` is the canonical product surface; `auto-refresh-tasks` is a local-only branch (never pushed to `origin`) that introduces a `ScheduledFetch` + `AutoRefresh` feature. All three packs ship that feature; `dev` does not. (`02-release-reconciliation.md` §4, §8.)
- zip121 ↔ zip122 are non-monotonic: neither is a strict superset/successor of the other; both diverge from `dev` on different files. (`02-release-reconciliation.md` §4.3.)
- zip122's `package.json` declares `release:build` / `release:zip` scripts that target `scripts/build_release.js`, which has never been committed on any branch. (`02-release-reconciliation.md` §6.2, D-02.)
- zip122 ships backslash-separated entry paths (`bcsp-20260122\api\...`), which violate the ZIP spec and break POSIX unzippers. (`02-release-reconciliation.md` §3, D-06.)
- zip121 has no top-level directory wrapper; extracting into a populated cwd clobbers existing files. (`02-release-reconciliation.md` §3, D-05.)
- **No existing release pack should be trusted as current.** (`02-release-reconciliation.md` §8.)
- Five Stage B follow-ups (R-01..R-05) were enumerated as recommendations: decide the fate of `auto-refresh-tasks`; commit a real `scripts/build_release.js`; standardize archive shape; resolve the `frontend/i18n/messages.json` source-of-truth question; rebuild a fresh release pack from a canonical merge. (`02-release-reconciliation.md` §8.)

### 3.3 Record reconciliation (from `03-record-reconciliation.md`)

- Legacy planning JSON (`record.json`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`) and historical narrative (`Compact/`, `review/`, `Rutgers-dr/`) are evidence, not authority. Status fields in legacy JSON are self-reports by an earlier agent and frequently disagree with current code. (`03-record-reconciliation.md` §2.)
- Across `record.json`'s ten parents and thirty-one subtasks: 4 parents are stale-`todo` while their subtasks are `done` and code ships (`act-002`, `act-005`, `act-009`, `act-011`); 2 parents are stale `todo + blocked:true` with shipped code (`act-003`, `act-010`); 1 parent (`act-004`, Discord) has subtasks claiming `done` with `artifacts` paths that **do not exist** on `dev` — the strongest single contradiction. (`03-record-reconciliation.md` §4.)
- `read_only.md` is **not** the ngagent main-agent prompt (the `<!-- ngagent-doc-version: 0.8.0 -->` marker belongs to `AGENTS.md` only); it is the project's DR distillation (`5b-mb.md` output) whose D1/D3/ACT-001/-002/-003/DEP-004 were superseded by `Patch-dr-2025-11-14.md` and `record.json.decisions`. The 01-inventory verify row "AGENTS.md vs read_only.md" resolves as **distinct files, distinct workflows**. (`03-record-reconciliation.md` §6.2, §7.)
- `record.json` is the **legacy bootstrap**, not ngagent runtime state (the live ngagent state lives at `.git/ngagent/`). Stage B must not import any legacy `status` field into ngagent runtime state. (`03-record-reconciliation.md` §6.6, §8.2.)
- `package.json` repository / bugs / homepage URLs still reference the pre-rename project name `BetterCourseSchedulePlanner` (no `Rutgers-` prefix). (`03-record-reconciliation.md` §6.5.)

### 3.4 Runtime / config hygiene (from `04-runtime-config-hygiene.md`)

- Three competing canonical SQLite paths exist in tracked sources: `data/local.db` (dev docs), `data/fresh_local.db` (one-click launcher), `data/courses.sqlite` (fetch pipeline example). The currently tracked SHM/WAL stragglers prove the source machine used more than one. (`04-runtime-config-hygiene.md` §4.)
- Six paths are tracked despite a matching `.gitignore` rule: `configs/mail_sender.user.json`, `data/poller_checkpoint.json`, `data/runtime/fetch_job_d21d4cd8-…json`, `data/courses.sqlite-shm`, `data/courses.sqlite-wal`, `data/fresh_local.db-shm`. (`04-runtime-config-hygiene.md` §5.1.)
- Two inverse cases (runtime-shape but unignored): `scripts/poller_checkpoint.json` (currently tracked, no rule reaches it); `data/refresh_queue.json` (forward-looking — referenced by `configs/fetch_pipeline.example.json:79` but no ignore rule covers it). (`04-runtime-config-hygiene.md` §5.2.)
- Highest-priority structural risk: the WebUI's `PUT /admin/mail-config` writes to `configs/mail_sender.user.json` directly; while that path remains tracked, a successful UI save commits a real SendGrid API key. (`04-runtime-config-hygiene.md` §6.)
- `data/runtime/fetch_job_d21d4cd8-…json` carries an absolute Windows path `Z:\Project\BetterCourseSchedulePlanner\data\courses.sqlite` that references the **pre-rename** repo location. (`04-runtime-config-hygiene.md` §5.1.)
- `.env.example` is allowlisted in `.gitignore` but no template file is tracked; the dev-bootstrap `.env.local` shape lives only in `scripts/setup_local_env.sh` heredocs and `docs/deployment_playbook.md`. (`04-runtime-config-hygiene.md` §3.1, §7.4.)

### 3.5 Module surface and refactor candidates (from `05-module-surface-map.md`)

- Module inventory: `api/` (Fastify + zod, 5 tracked tests), `frontend/` (Vite + React + TS, **zero tracked tests**, no `test` script), `workers/` (mail dispatcher + open-sections poller, 2 tests), `notifications/mail/` (SendGrid only; SMTP types exist with no SMTP provider; 4 tests), `scripts/` (mixed tools, no tests), `data/` (schema + 4 migrations, no tests), `configs/` (templates + examples + the secret-risk `mail_sender.user.json`; **no `mail_sender.schema.json`** counterpart to `fetch_pipeline.schema.json`), `docs/` (23 files, no doc-vs-code lint). (`05-module-surface-map.md` §1.)
- The runtime composition is encoded only in `scripts/oneclick_start.js`; the frontend `/api` proxy contract, the API↔workers SQLite seam, and the conditional mail-dispatch decision are all hardcoded there. (`05-module-surface-map.md` §2.)
- Highest-confidence drift: `routes/sections.ts` ships a fully-validated zod schema but its handler returns empty data — `docs/query_api_contract.md` discusses sections as a real endpoint. (`05-module-surface-map.md` §3.1, §4.)
- Live concurrency hazard: `services/fetchRunner.ts` keeps `activeJob` at module scope; two concurrent `POST /api/fetch` calls race. (`05-module-surface-map.md` §3.1.1.)
- Auth gap: `/admin/*` and `GET /api/subscriptions` are unauthenticated; safe today only because the launcher binds `127.0.0.1`. (`05-module-surface-map.md` §3.1.1.)
- Cross-module imports: `api/src/routes/admin.ts` reaches into `notifications/mail/template_checker.ts`; workers import `scripts/soc_api_client.ts`; otherwise boundaries are healthy. (`05-module-surface-map.md` §3.1.2, §3.3.)
- Thirteen Stage B refactor candidates (R-01..R-13) were enumerated with priority, blast radius, and a suggested sequence. R-10 was tagged Stage A cleanup (task-007), not Stage B. (`05-module-surface-map.md` §5.)

### 3.6 Cross-cutting context (from `.orchestrator/context_manifest.md`)

- The repository has been treated as a multi-source-of-truth surface: code, release packs, legacy JSON, Compact narrative, Obsidian vault, runtime artifacts, and remote state can each disagree.
- Operational gotcha: each `ngagent merge` of a Stage A task left the corresponding `.orchestrator/stage-a/<NN>-*.md` file staged-as-deleted in the main checkout and required restoration from HEAD before pushing. (`context_manifest.md`, "Known Gotchas".)
- Review missions are pinned to Claude Opus 4.7 Max; review-model fallback is not allowed. (`context_manifest.md`, "Known Gotchas".)

## 4. Source-of-Truth Hierarchy for Stage B

This is the binding hierarchy Stage B must use when two or more sources disagree about current product behavior or current intent. Each row identifies the source, what it is authoritative for, what it is **not** authoritative for, and where in the upstream reports the rule is justified.

| Tier | Source | Authoritative for | Not authoritative for | Justification |
| --- | --- | --- | --- | --- |
| **T0 (canonical)** | `dev` branch HEAD as observed by `git ls-files` and direct file content | What the product currently is, what tests run against it, which routes/workers/scripts exist, which migrations are applied, which docs ship, which configs have shipped templates. | Whether the product *should* behave this way; whether status fields in legacy records are correct; whether release packs match. | Stage A architecture pin: "Stage B refactors `dev`." All five upstream reports treated `dev` as the comparison authority. |
| **T1 (current planning surface)** | `.orchestrator/` (this directory): `goals.md`, `architecture.md`, `context_manifest.md`, `stage-a/01..06.md` | Stage A scope, exit criteria, evidence-layer rule, the source-of-truth hierarchy declared by this baseline, the refactor candidates and their sequencing. | Code-level facts (those come from T0); legacy task statuses (those come from `.git/ngagent/`, not from `record.json`). | This is the live planning workspace; it is being maintained alongside Stage A execution. |
| **T1 (current orchestrator state)** | `.git/ngagent/` (runtime task state per `AGENTS.md` constraint #2) | Live ngagent task / attempt / completion / review state for tasks managed under the new orchestrator workflow. | Pre-ngagent legacy planning history (that lives in T2 below). | `AGENTS.md` line 1, hard constraint: "Never manually edit runtime state (`record.json` is managed by ngagent under `.git/ngagent/`)." |
| **T1 (current operator docs)** | `AGENTS.md` (root); `docs/` runbooks and contract docs (23 files, per `01-inventory.md` §4.3) | Current ngagent orchestration; current developer/operator runbooks; current contracts as documented. | Code behavior when it disagrees with the doc — when a contract doc and the code disagree, T0 wins, the doc is downgraded to "drift to reconcile." | `03-record-reconciliation.md` §6.2 / §7 (errata vs. `01-inventory.md`); `05-module-surface-map.md` §4 (contract drift table). |
| **T2 (historical evidence)** | `record.json`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`; `Compact/` (74 files); `review/` (6 files); `Rutgers-dr/` (2 files); `read_only.md`; remote selective-disclosure surface. | Reconstructing what an earlier agent said it did and when, what decisions were made and why, what the architecture pivot looked like, which prompts drove which records. **Useful as `spec input` only** when migrating a work item into ngagent runtime state. | Current code state; current task status; correctness of any artifact-path claim that was not re-checked against `git ls-files` today. | `03-record-reconciliation.md` §2, §8.1, §8.2. |
| **T2 (release packs as historical evidence)** | `release/bcsp-20260121.tar.gz`, `release/bcsp-20260121.zip`, `bcsp-20260122.zip` | Reconstructing what was packaged at the two snapshot moments; supporting forensic comparison only. | Current product behavior; canonical release shape; trust as a download for end users. **No existing release pack is canonical.** | `02-release-reconciliation.md` §8 — five independent grounds why no pack can be trusted. |
| **T3 (external evidence)** | `D:\Document\Obsidian\Adrian\Prompt\BetterCourseSchedulePlanner\` Obsidian vault | Explaining the *shape* of the legacy DR→JSON→Compact→Review→Update workflow. | Anything inside the repository. The vault folder itself is named with the project's pre-rename name, evidence that even external tooling drifted. | `03-record-reconciliation.md` §3, §6.7, §8.1. |
| **NULL (do not treat as authority)** | `.gitignore` matching alone; legacy `status` fields in `record.json` / `r*.json`; current ignored state; current untracked state; current remote branch list; archive manifest / content alone | nothing in this baseline | every authority claim — they are evidence layers per §2. | `01-inventory.md` §2; `02-release-reconciliation.md` §2; `03-record-reconciliation.md` §2; `04-runtime-config-hygiene.md` §2; `05-module-surface-map.md` §0. |

**Stage B procedural consequence:** when planning any refactor, Stage B must (a) read the current state from T0 (code on `dev`); (b) read intent and contract from T1 (`.orchestrator/`, `AGENTS.md`, `docs/`); (c) consult T2 (legacy records, release packs) only when explanatory history is needed and only as evidence; (d) treat NULL-tier signals as starting questions, never as conclusions.

## 5. Trust Posture — What Is Trusted, Stale, Historical, Unsafe, Unresolved, or Evidence-Only

The categories below are an explicit synthesis across all five Stage A reports, restated so Stage B can refer to a single table. "Evidence-only" is the explicit fifth bucket required by the task spec; it covers items whose *existence* is informative but whose *content* must not be promoted to authority.

### 5.1 Trusted (current source of truth on `dev`)

These surfaces are trustworthy *as a description of how the product currently behaves*. Stage B may treat them as the starting point for refactor without re-deriving from history.

| Surface | Why trusted now | Caveat |
| --- | --- | --- |
| `api/src/**` (8 route files, 2 query helpers, 1 service, 1 plugin, 1 container, 1 config, 1 fastify decorator typing) | Tracked product source; `01-inventory.md` §4.1; `05-module-surface-map.md` §3.1. | `routes/sections.ts` ships a real schema with a stub handler — see §5.4 unresolved drift. |
| `frontend/src/**` (12 components, 3 hooks, 6 typed API client files; `frontend/index.html`; `frontend/vite.config.ts`) | Tracked product source; `01-inventory.md` §4.1; `05-module-surface-map.md` §3.2. | No tracked tests; `src/dev/*` is dead from routing — see §5.2 stale. |
| `workers/{mail_dispatcher,open_sections_poller}.ts` + `workers/tests/*` | Tracked product source + 2 tests; `05-module-surface-map.md` §3.3. | Worker poll loop / dedup / checkpoint hydration are not directly covered by named tests — observation only. |
| `notifications/mail/**` (types, config, template_loader, template_checker, retry_policy, providers/sendgrid.ts + 4 tests) | Tracked product source; `05-module-surface-map.md` §3.4. | SMTP types exist but no SMTP provider is implemented — flag, not a behavior bug. |
| `scripts/{fetch_soc_data,soc_api_client,soc_normalizer,soc_rate_limit,migrate_db,oneclick_start,run_stack,setup_local_env,backfill_core_attributes,incremental_trial,poller_resume_sim,mail_e2e_sim,i18n_missing_check,mail_templates,soc_field_matrix}` | Tracked operational scripts; `01-inventory.md` §4.1; `05-module-surface-map.md` §3.5, §3.6. | Some are not bundled into all release packs (zip122 dropped 11 of them while keeping `npm` script entries) — see `02-release-reconciliation.md` §5.1, D-03. |
| `data/schema.sql` and `data/migrations/00{1,2,3,4}_*.sql` | Tracked product DDL; `01-inventory.md` §4.1, §4.6 (correctly retained). | `schema.sql` and the cumulative migrations are not auto-checked against each other (drift would not be flagged by CI). |
| `configs/fetch_pipeline.{example,schema}.json`, `configs/mail_sender.example.json`, `configs/templates/email/**` (8 files) | Tracked product config and templates; `01-inventory.md` §4.7; `02-release-reconciliation.md` §6.4. | `configs/mail_sender.schema.json` is **missing**; only the example exists — see §5.4 unresolved. |
| `docs/` (23 files) — runbooks + contract docs + SOC measurement evidence | `01-inventory.md` §4.3; `05-module-surface-map.md` §3.7. | Several contract docs drift from code (sections, mail provider set, mail config schema) — see §5.4 unresolved. |
| `Start-WebUI.bat`, `Start-WebUI.command` | Tracked launchers; `01-inventory.md` §4.1. | `Start-WebUI.command` line endings are CRLF on `dev`/zip121 and differ in zip122 — latent POSIX hazard (`02-release-reconciliation.md` §6.3, D-07). |
| `package.json`, `package-lock.json`, `tsconfig.json`, `frontend/package.json`, `frontend/package-lock.json`, `frontend/tsconfig.json`, `frontend/tsconfig.node.json`, `frontend/vite.config.ts` | Tracked build/lock manifests; `01-inventory.md` §4.1. | `package.json` `repository`/`bugs`/`homepage` URLs still reference the pre-rename repo (no `Rutgers-` prefix) — `03-record-reconciliation.md` §6.5. |
| `AGENTS.md` (ngagent v0.8.0) | Current orchestrator entry doc; `03-record-reconciliation.md` §6.2, §7. | Authoritative for orchestrator governance only; does **not** authorize editing the legacy `record.json` directly. |
| `.orchestrator/{goals,architecture,context_manifest}.md` and `.orchestrator/stage-a/01..06.md` | Current Stage A planning surface. | Authoritative for Stage A scope, not for code state (T0 wins for code). |
| `.git/ngagent/**` | Current ngagent runtime state per `AGENTS.md`. | Pre-ngagent task history is **not** here; that lives in T2. |
| `reports/{field_validation.md, field_validation_samples.json, fresh_install_log.md, mail_worker_latency.md, poller_durability.md}` | Tracked measurement outputs; `01-inventory.md` §4.5; `03-record-reconciliation.md` §3, §8.1. (Full enumeration intentionally provided here to address the task-003 review note about less-explicit `reports/` enumeration — see §6.2.) | One additional file in `reports/` (`field_validation_details.mdpart`) carries a non-standard extension — see §5.4 unresolved. |

### 5.2 Stale (was real once; now contradicted by reality)

These items still exist in tracked state but their content does not match the current code/intent. They should not be cited as if current.

| Item | Staleness | Source |
| --- | --- | --- |
| `record.json` parent statuses for `act-002`, `act-005`, `act-009`, `act-011` | Parent says `todo`; subtasks are `done` and code ships. | `03-record-reconciliation.md` §4.5, §4.6, §4.7, §4.9, §4.13. |
| `record.json` parent statuses for `act-003`, `act-010` | Parent says `todo` + `blocked: true`; code ships, including a durability/latency report. | `03-record-reconciliation.md` §4.8, §4.10, §4.13. |
| `record.json` parent-level `artifacts` arrays for `act-002, -003, -005, -008, -009, -010, -011` | Point to a pre-`Patch-dr-2025-11-14.md` `services/`-flavoured layout that no longer exists; subtask-level arrays were updated but parent arrays were not. | `03-record-reconciliation.md` §4.13, §6.3. |
| `rRevision.json` subtask statuses (`-01`, `-02`) and parent | More done than `todo`/`in_review` claim; Compacts and code prove `done`. Subtask `-04` (docs/i18n) is plausibly partly outstanding. | `03-record-reconciliation.md` §5.2. |
| `rSubscribe.json` parent status | All subtasks `done`; parent still `in_progress`. | `03-record-reconciliation.md` §5.3. |
| `read_only.md` (D1, D3, ACT-001, ACT-002, ACT-003, DEP-004) | Pre-`Patch-dr-2025-11-14.md`; superseded by `record.json.decisions`. The filename "read_only" is a misleading promise of authority. | `03-record-reconciliation.md` §6.1, §6.2, §7. |
| `package.json` `repository` / `bugs` / `homepage` URLs | Reference pre-rename `BetterCourseSchedulePlanner` instead of current `Rutgers-BetterCourseSchedulePlanner`. | `03-record-reconciliation.md` §6.5. |
| Three competing canonical SQLite paths in tracked sources (`data/local.db`, `data/fresh_local.db`, `data/courses.sqlite`) | Each appears as a default in different tracked files; SHM/WAL stragglers are direct evidence that more than one path was used on the source machine. | `04-runtime-config-hygiene.md` §4. |
| `data/runtime/fetch_job_d21d4cd8-…json` absolute Windows path | Embedded path `Z:\Project\BetterCourseSchedulePlanner\data\courses.sqlite` references the pre-rename repo location. | `04-runtime-config-hygiene.md` §5.1. |
| `frontend/src/dev/*` (developer playground) | Not imported by product code (`grep "from ['\"]./dev/" frontend/src` returns nothing); ships in source tree but is dead from routing. | `05-module-surface-map.md` §3.2.1. |
| `scripts/poller_checkpoint.json` | Stray runtime artifact under `scripts/`; the canonical checkpoint path is `data/poller_checkpoint.json` and oneclick uses only that. | `01-inventory.md` §4.6, §4.10; `04-runtime-config-hygiene.md` §5.2; `05-module-surface-map.md` §3.6. |
| `.env.example` allowlist with no tracked file behind it | `.gitignore:71` `!.env.example` is forward-looking; no template exists. | `04-runtime-config-hygiene.md` §3.1, §7.4. |
| Filename typo `review/review-recod-2025-11-14.md` | "recod" → "record"; cosmetic. | `01-inventory.md` §4.10; `03-record-reconciliation.md` §6.4. |
| `notebooks/incremental_trial.md` (singleton in a stray directory) | Semantically a report; `notebooks/` directory is otherwise empty. | `01-inventory.md` §4.5, §4.10. |
| `reports/field_validation_details.mdpart` non-standard `.mdpart` extension | Likely an in-progress markdown fragment. | `01-inventory.md` §4.5, §4.10. |
| Decision-id slug `D-20251113-arch-static-frontend-b` | Body chose the *opposite* of what the slug suggests (local DB + local API, not static frontend); naming inertia. | `03-record-reconciliation.md` §4.13, §6.1. |
| Task-id slug `T-20251113-act-001-soc-json-scraper` | Re-scoped after `Patch-dr-2025-11-14.md` from "static JSON scraper" to "local DB initializer"; the slug retained the old wording. | `03-record-reconciliation.md` §4.3. |

### 5.3 Historical evidence (no longer authoritative; valuable forensically)

These items are intentionally retained on disk and have continuing value as evidence of *what was done and why*. They are not authoritative for current state and must not be imported into ngagent runtime state as if they were.

| Item | Historical value | Source |
| --- | --- | --- |
| `Compact/` (74 files; `Compact-ST-YYYYMMDD-<initiative>-<step>-<title>-<ISOZ>.md`) | Per-subtask narrative + Code-Review trailers; more reliable than `record.json` for "what an agent claimed it did" because each file is dated and frequently carries a trailer. Some Compacts (e.g. `act-004-02/-03`) describe code that is **not in current `dev`**. | `01-inventory.md` §4.4; `03-record-reconciliation.md` §3, §4.11, §8.1. |
| `review/` (6 files; one with typo `recod`) | `diff --git a/record.json b/record.json` snapshots from 2025-11-13..17. Chain has at least 3 gaps and stops well before `record.json` itself stopped being edited. | `01-inventory.md` §4.4; `03-record-reconciliation.md` §3, §6.4. |
| `Rutgers-dr/2025-11-11-dr.md`, `Rutgers-dr/Patch-dr-2025-11-14.md` | Hand-authored DR upstream of `read_only.md`, `record.json`, and the architecture pivot. The patch DR is the *immutable narrative* that explains why the project moved from static frontend + cloud functions to local DB + local services. | `01-inventory.md` §4.4; `03-record-reconciliation.md` §3, §6.1, §8.1. |
| `read_only.md` (DR distillation `5b-mb.md` output, v0.1, 2025-11-13) | Historical evidence of pre-pivot decisions; **not** the ngagent prompt and **not** current. | `03-record-reconciliation.md` §6.2, §7, §8.1. |
| `record.json` (legacy task graph, 2,390 lines) | Useful for understanding the project's planning history; not ngagent runtime state. | `03-record-reconciliation.md` §6.6, §8.1. |
| `rEmail.json` (closest legacy record to ground truth — all 5 subtasks done, all artifacts present) | Useful summary of the no-code mail-onboarding feature; also evidence that `configs/mail_sender.user.json` was a deliberate design slot, which contextualizes today's structural secret-risk surface. | `03-record-reconciliation.md` §5.1, §8.1. |
| `rRevision.json`, `rSubscribe.json` | Seed plans for the filter rewrite and auto-term poller. Status fields are stale; narrative is intact. | `03-record-reconciliation.md` §5.2, §5.3, §8.1. |
| `release/bcsp-20260121.tar.gz`, `release/bcsp-20260121.zip`, `bcsp-20260122.zip` | Forensic comparison surfaces only — they document two different working trees that were packaged on two different days, neither equal to `dev`. **Not canonical.** | `02-release-reconciliation.md` §8, §10. |
| Remote branch list (`Before-Final`, `CRLF`, `Everythingbeforeemail`, `Fin-Sync-1125`, `New-Subtask`, `RealST-…`, `ST-…`, `Reorder-Subtask`, `boli`, `eMAIL`, etc.) | Evidence of past push activity. The remote was curated from "everything submitted" toward "only the project submitted" (Context Manifest); presence/absence is selective. | `01-inventory.md` §7. |
| `D:\Document\Obsidian\Adrian\Prompt\BetterCourseSchedulePlanner\` (out-of-repo Obsidian vault, 13 markdown templates) | External evidence of the legacy DR→JSON→Compact→Review→Update workflow. The folder name itself preserves the pre-rename project name. | `03-record-reconciliation.md` §3, §6.5, §6.7, §8.1. |

### 5.4 Unresolved drift / unknowns (require Stage B decisions, not Stage A action)

These items are documented as drift between current sources of truth but Stage A explicitly *does not resolve them*. They become Stage B's first decisions.

| Item | Disagreement | Resolution owner |
| --- | --- | --- |
| `routes/sections.ts` stub vs `docs/query_api_contract.md` describing sections as a real endpoint | Code returns `data: []`/`total: 0`; doc promises behavior. | Stage B (R-01). `05-module-surface-map.md` §3.1, §4. |
| Auto-refresh feature surface in all three release packs but absent on `dev` | Packs ship `ScheduledFetch` + `AutoRefresh` from local-only `auto-refresh-tasks` branch; `dev` ships none of it. Either `dev` is canonical (packs are forward-looking) or the feature should be merged. | Stage B (R-02 in this baseline / R-01 in `02-release-reconciliation.md` §8). |
| `frontend/i18n/messages.json` (carried by every release pack and by `auto-refresh-tasks`, absent from `dev`) vs `frontend/src/i18n/index.ts` + `frontend/src/data/fallbackDictionary.ts` (in-tree on `dev`) | Two different i18n sources of truth depending on which branch a reader started from. | Stage B (R-04 in `02-release-reconciliation.md` §8). |
| `act-004-discord-notify-channel` subtasks claim `done` with `artifacts` paths that do not exist on `dev`; three Compacts quote substantive code (e.g. `workers/discord_dispatcher.ts` line numbers in a Code-Review trailer) | The Discord work was implemented and reviewed at some point, then either deleted or never merged. | Stage B (recover from history if a branch carries it; otherwise formally retire). `03-record-reconciliation.md` §4.11, §8.3. |
| `rRevision.json` filter-rewrite-04-docs-i18n | Likely partial done (i18n side shipped; doc-update side may remain). | Stage B should re-check `docs/query_api_contract.md` and `docs/ui_flow_course_list.md` against the new `examCode` + meeting-day-subset semantics. `03-record-reconciliation.md` §5.2, §8.3. |
| Three competing canonical SQLite paths | `data/local.db` (dev docs), `data/fresh_local.db` (one-click), `data/courses.sqlite` (fetch pipeline example). | Stage B should pick one canonical path and reconcile docs/code. `04-runtime-config-hygiene.md` §4, §7.4. |
| `.env.example` template absence | Allowlisted but no file tracked. | Stage B should either commit a sanitized `.env.example` or remove the allowlist; `04-runtime-config-hygiene.md` §3.1, §7.4. |
| Missing `configs/mail_sender.schema.json` | `configs/fetch_pipeline.schema.json` is tracked; analogous mail schema is not. The only validation is in `notifications/mail/config.ts`. | Stage B (R-09 in `05-module-surface-map.md`). |
| `package.json` URLs vs `git remote -v` (pre-rename name still in `repository`/`bugs`/`homepage`) | URL drift (cosmetic for the package; functional for any tooling that reads it). | Stage B (R-08 in this baseline). |
| `Start-WebUI.command` line endings | dev/zip121 carry CRLF; zip122 differs (likely LF). CRLF can fail on strict POSIX shells with `bash\r: command not found`. | Stage B (R-09 in this baseline) and/or release tooling decision. `02-release-reconciliation.md` §6.3, D-07. |
| `frontend/src/dev/*` Vite build inclusion | Dead from product routing today. Should be excluded from the production bundle, gated by env, or relocated outside `src/`. | Stage B (R-13 in `05-module-surface-map.md`). |
| `notebooks/incremental_trial.md` and `reports/field_validation_details.mdpart` location/naming | Cosmetic file-organization questions. | Either Stage A `task-007` cleanup or Stage B doc/repo-organization pass. `01-inventory.md` §4.5, §4.10. |
| Filename typo `review/review-recod-2025-11-14.md` | Cosmetic; rename safety against external references already checked (none found in `Compact/` per `03-record-reconciliation.md`). | Stage A `task-007` cleanup if approved. |
| Tar UID/GID side-channel `dwqadad123` in `release/bcsp-20260121.tar.gz` | Not a credential, not PII, not a secret — but a deanonymizing string identifying the packager's local Windows username. | Future release tooling should strip UID/GID (`--owner=0 --group=0 --numeric-owner`). The string itself is not republished by this baseline beyond the literal hex/printable form already recorded in `02-release-reconciliation.md` §3 / D-10 (see §6.1 below for the explicit handling rule). |

### 5.5 Unsafe (active or imminent risk)

These items are *not just stale*; they are active or imminent risks. Stage B should not assume they are gone unless `task-007` cleanup has applied with explicit human approval.

| Item | Risk | Source |
| --- | --- | --- |
| `configs/mail_sender.user.json` is tracked while `api/src/routes/admin.ts:121` writes that exact path | The next user who completes the WebUI "Save Email Settings" flow against `dev` writes a real SendGrid key into a tracked file. The user is one `git add` away from committing the secret. **Highest-priority structural risk in this baseline.** | `01-inventory.md` §4.7, §6; `04-runtime-config-hygiene.md` §5.1, §6, §7.1. |
| `data/poller_checkpoint.json` tracked despite ignore rule | First poller run on a clean checkout rewrites the snapshot hash + timestamp; immediate dirty working tree on every clone. | `01-inventory.md` §4.6; `04-runtime-config-hygiene.md` §5.1, §7.1. |
| `data/runtime/fetch_job_d21d4cd8-…json` tracked despite `data/runtime/` ignore rule, carrying stale absolute Windows path | Local-path leak (machine fingerprint); also encodes the pre-rename repo name. | `01-inventory.md` §4.6; `04-runtime-config-hygiene.md` §5.1, §7.1. |
| `data/courses.sqlite-shm`, `data/courses.sqlite-wal`, `data/fresh_local.db-shm` tracked despite ignore rules | Orphaned SQLite sidecars regenerated by SQLite at runtime; immediate dirty working tree on every clone. | `01-inventory.md` §4.6; `04-runtime-config-hygiene.md` §5.1, §7.1. |
| `scripts/poller_checkpoint.json` tracked under `scripts/` with no matching ignore rule | Inverse of the data/ pattern: a runtime artifact tracked by location-mismatch. | `01-inventory.md` §4.6, §4.10; `04-runtime-config-hygiene.md` §5.2, §7.2. |
| `data/refresh_queue.json` (forward-looking) | Referenced by `configs/fetch_pipeline.example.json:79`; if/when the incremental fetcher writes it, no `.gitignore` rule covers it; a `git add .` would commit a runtime queue file. | `04-runtime-config-hygiene.md` §5.2, §7.3. |
| zip122 `package.json` references `scripts/build_release.js` that has never been committed | `npm run release:build` fails with module-not-found on a clean install of zip122 (release-blocking defect). | `02-release-reconciliation.md` §6.2, D-02. |
| zip122 backslash entry paths violate ZIP spec | POSIX unzippers may produce literal `bcsp-20260122\api\…` files instead of nested directories (release-blocking on POSIX). | `02-release-reconciliation.md` §3, D-06. |
| zip121 has no top-level directory wrapper | Extracting into a populated cwd clobbers existing `api/`, `frontend/`, etc. (release-blocking UX). | `02-release-reconciliation.md` §3, D-05. |
| All three release packs ship the unmerged `auto-refresh-tasks` feature surface that does not exist on `dev` | Either dev or packs misrepresent the canonical product. | `02-release-reconciliation.md` §4.1, §4.2, §8, D-01. |
| Auth gap: `/admin/*` and `GET /api/subscriptions` are unauthenticated | Safe today only because oneclick binds to `127.0.0.1`. Any future packaging that binds to a public interface inherits an open admin endpoint. | `05-module-surface-map.md` §3.1.1, R-03. |
| `services/fetchRunner.ts` module-level `activeJob` race | Two concurrent `POST /api/fetch` calls race; no lock at the runner. | `05-module-surface-map.md` §3.1.1, R-02. |

### 5.6 Evidence-only (reproduce structure, do not promote content)

These items must be cited only as evidence of structure/existence; their *content* must not be quoted, reproduced verbatim, or promoted to authority. This bucket is the explicit "evidence only" category required by the task spec.

| Item | What may be cited | What must not be done |
| --- | --- | --- |
| Current value field of `configs/mail_sender.user.json` | The file is currently tracked; the value matches the literal placeholder shape used in product docs; the structural risk is that the next WebUI save will replace the placeholder with a real `SG.…` SendGrid token. | Reproducing the value (placeholder or real). Treating "today's value is a placeholder" as proof that the file is safe to keep tracked. |
| Tar UID/GID string `dwqadad123` in `release/bcsp-20260121.tar.gz` | The string exists as a header artifact and is a deanonymizing side-channel (see §6.1 for the rule that this baseline observes when discussing it). | Treating the string as an authoritative identifier of any specific person; reusing the literal string elsewhere; embedding it in product source or user-visible documentation. |
| Counts inside the release archives (`tar -tzvf` row totals, zip entry rows) | The archives have manifests; the manifests can be enumerated and compared. | Treating any single count from `02-release-reconciliation.md` as the *authoritative* count without citing the specific source row (count drift is annotated in §6.1). |
| `process.env.*` reads inside route handlers (e.g. `MAIL_CONFIG_DIR` in `admin.ts`) | The handler reads env at request time; this is an architectural smell and a refactor target. | Editing the handler in this task; this is Stage B (R-04). |
| Internal poller checkpoint shape (`{version, updatedAt, campuses: { "<term>|<campus>": { … lastSnapshotHash, openIndexes, misses } }}`) | Schema and field meanings are part of the runtime contract; documenting them is desirable. | Reproducing actual snapshot-hash values or timestamps as if they had product significance. |
| Internal fetch-pipeline-config snapshot at `data/runtime/fetch_job_d21d4cd8-…json` | The shape is informative for future fetch-job logging design. | Treating the embedded absolute Windows path as a current artifact location (it is pre-rename and pre-untracking). |
| Branch list returned by `git ls-remote --heads origin` | The remote carries many branches and the list is informative as historical surface. | Treating remote branch presence/absence as proof of current canonical state. |
| Compact/review/`Rutgers-dr/`/`read_only.md`/`record.json` content | Useful to *cite* when explaining a Stage B decision (e.g. "the Discord channel was implemented per Compact-ST-…act-004-02-…, then dropped"). | Treating any quoted Compact passage as a specification for current behavior; reapplying historical Code-Review trailers to current code. |
| Obsidian vault prompt content (`Meta-Prompt.md`, `4b-dr.md`, `5b-mb.md`, `6b-json-v3.md`, `7b-dr-patch.md`, `9-code.md`, `11-compact.md`, `14-com.md`, `14a-compact.md`, `15a-update.md`, `GIT-Clone.md`, `未命名.md`) | The vault explains the *shape* of legacy records; references can stay attribution-only. | Editing the vault from inside the repo; using vault content as a current source of truth. |

## 6. Incorporated Review Notes

The Main Agent pinned two review-note follow-ups from the upstream task reviews. This section addresses each explicitly so Stage B does not re-encounter them as open items.

### 6.1 task-002 review notes

**(a) Low-risk count inconsistencies in `02-release-reconciliation.md`.** The report's count tables and prose figures are not perfectly aligned. Specifically, `02-release-reconciliation.md` §1 states `release/bcsp-20260121.tar.gz` has "166 entries from `tar -tzf` (30 directory entries ending in `/`, 136 file entries — matches the zip121 file count exactly)", while the corresponding row in §3 reports "POSIX `tar.gz`. Forward-slash paths. **No top-level directory wrapper** (entries begin at `api/`, `frontend/`, etc.). 153 file entries. All entry mtimes inside the archive fall between `2026-01-21 08:04:02` and `2026-01-21 09:10` (configs dir)." The figures `136`, `153`, and `166` are not reconciled in a single authoritative table; the §1 framing ("166 entries / 30 dirs / 136 files") is internally consistent and most plausibly accurate, while the §3 row's `153` number does not match either subset count from §1.

**Resolution applied by this baseline:** the inconsistency is recorded as a known low-risk count drift. This baseline does not republish any of the three numbers as the authoritative file count for `release/bcsp-20260121.tar.gz`. Stage B (and any future republish work) must re-derive the count from a fresh enumeration before relying on it. The drift does not affect any §7 / §8 finding in `02-release-reconciliation.md` because the trust posture (§5.5 and `02-release-reconciliation.md` §8) is "no existing release pack can be trusted" regardless of exact entry count. No correction is applied to `02-release-reconciliation.md` in this task because per AC-001 prior Stage A reports must not be modified; Stage B may re-issue a corrected enumeration alongside R-04 (release tooling) below.

**(b) Packager username side-channel `dwqadad123` in tar.gz UID/GID.** `02-release-reconciliation.md` §3 and D-10 explicitly named the literal string `dwqadad123` (the packager's local Windows username) preserved in the tar header UID/GID slots. This is **not** a secret in the cryptographic sense and is **not** PII at the legal sense (it is a self-chosen handle), but it is a deanonymizing artifact embedded in a binary that has been distributed to other parties via the local `release/` surface.

**Handling rule applied by this baseline:** this baseline cites the string only when necessary to make the finding concrete (this paragraph and §5.4 / §5.6 above). It does not embed the string in product source, user-visible docs, or release tooling. Any future release-pipeline change should set `--owner=0 --group=0 --numeric-owner` (or the zip equivalent) before any rebuild, per `02-release-reconciliation.md` §7 (drift summary, data-hygiene tier). When archival decisions are taken in `task-007` for the existing packs, the option of *replacing* the affected pack with a UID-stripped re-tar should be considered (the original pack should be preserved as historical evidence under an explicit archive path, not deleted, until a verified replacement exists). Any decision that touches the existing packs requires explicit human approval per §7 below.

### 6.2 task-003 review notes

**(a) Cosmetic bad section references in `03-record-reconciliation.md`.** The report contains internal cross-references to `01-inventory.md` and to its own sections that, while interpretable, are not perfectly aligned with the subsequently published structure of those documents in every case. Examples observed in the upstream report include:

- Inline references to "01-inventory §4.4 / §5" for `read_only.md`'s row position and to "01-inventory §4.6 / §4.10" for the `scripts/poller_checkpoint.json` flag — these point to the right *content* in `01-inventory.md`, but the reader has to consult both sections to assemble the same composite picture this baseline assembles in §5.2 and §5.5 here.
- The §6.4 review-chain table cites blob OIDs across six review files; the OIDs are consistent within `03-record-reconciliation.md` but are not reproduced in this baseline because they are evidence-only per §5.6 (citing the chain's *structure* — start, three gaps, continuation — is sufficient for Stage B).
- The drift between "`act-002`, `-005`, `-009`, `-011`" (4 stale parents) in §4.13 and the "`act-003`, `-010`" (2 stale + blocked parents) tally in the same table is correctly enumerated when both rows are read together, but the surface count "stale parents in `record.json`" is split across two table rows; this baseline collapses both into §5.2 above so Stage B sees a single combined list of six task-level staleness items.

**Resolution applied by this baseline:** §3.3 and §5.2 of this document re-state the relevant findings with their direct evidence pointers and a unified shape; future readers should consult this baseline (rather than the cross-references inside `03-record-reconciliation.md`) when they want a single-pass summary. No correction is applied to `03-record-reconciliation.md` in this task because per AC-001 prior Stage A reports must not be modified. The original report's section-by-section cross-checks remain available for forensic use.

**(b) Less explicit `reports/` enumeration in `03-record-reconciliation.md`.** The upstream report cites `reports/` as "6 files" with a 2025-11-17 .. 2025-11-21 date span (`03-record-reconciliation.md` §3) and references individual reports (`reports/field_validation.md`, `reports/poller_durability.md`, `reports/mail_worker_latency.md`, `reports/fresh_install_log.md`) inline in the per-task tables, but does not publish a single explicit table of all six file names.

**Resolution applied by this baseline:** §5.1 of this document explicitly enumerates all six tracked reports — `reports/field_validation.md`, `reports/field_validation_samples.json`, `reports/fresh_install_log.md`, `reports/mail_worker_latency.md`, `reports/poller_durability.md`, plus the seventh non-standard-extension file `reports/field_validation_details.mdpart` (which is technically the sixth `reports/` file and is recorded in §5.4 as a naming/extension unresolved item, since `01-inventory.md` §4.5 already noted the `.mdpart` extension and §4.10 flagged it for verify). Stage B should treat this baseline's §5.1 enumeration as the single canonical list of `reports/` content; any future reports addition must update this list.

## 7. Archive / Delete-Candidate / Untrack Candidates (Recommendations Only)

This section consolidates every recommendation in the upstream reports for repository cleanup. **All items are recommendations; none are applied by this task.** Per `goals.md` "Out of Scope" rule "Deleting historical records or release artifacts before explicit approval," and per the architecture pin that `task-007` is the only Stage A task that may apply repository-altering actions, **destructive changes require explicit human approval** before `task-007` proceeds, and `task-007` is restricted to non-product cleanup that has been explicitly approved.

### 7.1 Untrack from git tracking (no on-disk deletion proposed)

`git rm --cached <path>` style operations only. Each path is matched by an existing `.gitignore` rule (one row excepted), so untracking is sufficient; no rule change is required for these six.

| Path | Reason | Existing rule | Approval gating |
| --- | --- | --- | --- |
| `configs/mail_sender.user.json` | Highest-priority structural secret risk; the WebUI write-path will rewrite this with real SendGrid keys. | `.gitignore:155` `configs/*.user.json` | Requires explicit human approval. The current placeholder body must be confirmed to *not* contain any real secret before the untrack lands; if a real key were ever committed historically, history rewriting is a separate Stage B decision and must consult `02-release-reconciliation.md` remote evidence first. |
| `data/poller_checkpoint.json` | Pure runtime state; rewritten on every poller run. Recreated automatically by the launcher. | `.gitignore:151` `data/poller_checkpoint.json` | Requires explicit human approval. |
| `data/runtime/fetch_job_d21d4cd8-2df9-4bc8-b589-e86d19ceaf6a.json` | Per-run write-only artifact; also leaks a stale absolute Windows path referencing the pre-rename repo. | `.gitignore:150` `data/runtime/` | Requires explicit human approval. |
| `data/courses.sqlite-shm` | Orphaned SQLite sidecar regenerated at runtime. | `.gitignore:146` `data/*.sqlite-shm` | Requires explicit human approval. |
| `data/courses.sqlite-wal` | Orphaned SQLite sidecar regenerated at runtime. | `.gitignore:147` `data/*.sqlite-wal` | Requires explicit human approval. |
| `data/fresh_local.db-shm` | Orphaned SQLite sidecar regenerated at runtime. | `.gitignore:144` `data/*.db-shm` | Requires explicit human approval. |
| `scripts/poller_checkpoint.json` | Stray runtime artifact under the wrong directory; canonical path is `data/poller_checkpoint.json`. **No matching ignore rule** — untracking should be paired with a rule update (see §7.2). | none currently | Requires explicit human approval. |

### 7.2 `.gitignore` and configuration-only changes (recommendation queue for `task-007`)

| Item | Recommendation | Approval gating |
| --- | --- | --- |
| Add coverage for `scripts/poller_checkpoint.json` | Either widen the existing rule to `**/poller_checkpoint.json` or add an explicit `scripts/poller_checkpoint.json` entry. Pair with the §7.1 untrack of the same path. | Requires explicit human approval. `.gitignore` is on the blocklist for this baseline. |
| Add coverage for `data/refresh_queue.json` (forward-looking) | Add `data/refresh_queue.json` (or `data/*queue.json`) before the incremental fetcher first writes to that path. | Requires explicit human approval. |
| Track a sanitized `.env.example` to satisfy the `!.env.example` allowlist | Optional — the alternative is to remove the `!.env.example` line. Pick one and document the choice. | Requires explicit human approval. |

### 7.3 Archive (separate-from-current-source-of-truth, on-disk preserved)

Items that should be moved to a clearly-named historical/archive location. **No deletion is proposed.** The exact archive path is a `task-007` decision; this baseline only declares the items.

| Item | Reason | Approval gating |
| --- | --- | --- |
| `record.json` (root) | Legacy bootstrap; not ngagent runtime state. Useful as historical evidence; not authoritative. | Requires explicit human approval before any move. |
| `rEmail.json`, `rRevision.json`, `rSubscribe.json` (root) | Legacy seed plans for three initiatives. Useful as historical evidence; statuses are stale or contradictory. | Requires explicit human approval. |
| `Compact/` (74 files) | Historical narrative authored by an earlier AI workflow. Preserve as evidence; not authoritative for current code. | Requires explicit human approval. Per `goals.md` Out of Scope: "Deleting historical records … before explicit approval." |
| `review/` (6 files; one with the `recod` typo) | Historical `record.json` diff snapshots. Preserve as evidence. | Requires explicit human approval. |
| `Rutgers-dr/` (2 files) | Hand-authored DR upstream of `read_only.md` and `record.json`. Preserve as evidence. | Requires explicit human approval. |
| `read_only.md` (root) | DR distillation v0.1; superseded after `Patch-dr-2025-11-14.md`. Preserve as evidence; if moved, leave a one-line forwarding pointer per `03-record-reconciliation.md` §8.3. | Requires explicit human approval. |
| `release/bcsp-20260121.tar.gz`, `release/bcsp-20260121.zip`, `bcsp-20260122.zip` | Non-canonical release packs; preserve as forensic evidence under an explicit archive path until a verified replacement pack exists. | Requires explicit human approval. Per `goals.md` Out of Scope: "Deleting … release artifacts before explicit approval." |

### 7.4 Repository-organization moves (cosmetic; defer to `task-007` or Stage B doc pass)

| Item | Recommendation | Approval gating |
| --- | --- | --- |
| `notebooks/incremental_trial.md` | Move into `reports/` (semantically a report; `notebooks/` is otherwise empty). | Requires explicit human approval. |
| `reports/field_validation_details.mdpart` | Either rename to `.md` (if it is a finished fragment) or finalize/discard the in-progress content. | Requires explicit human approval. |
| `review/review-recod-2025-11-14.md` | Rename to `review-record-2025-11-14.md`; rename safety against external references already checked (none in `Compact/`). | Requires explicit human approval. |
| `frontend/src/dev/*` | Either gate via Vite env, relocate outside `src/`, or delete (after confirming no future test wants to import the playground). | Requires explicit human approval; see also Stage B R-13. |

### 7.5 Things this baseline explicitly **does not** recommend

- **Do not delete** `data/`, `configs/`, `scripts/`, `Compact/`, `review/`, `Rutgers-dr/`, `release/`, `bcsp-20260122.zip`, `read_only.md`, `record.json`, or any `r*.json` from disk in `task-007`. All §7.1 actions are `git rm --cached` only; all §7.3 actions are *moves*, not deletions.
- **Do not edit `record.json`** during Stage A to "fix" stale statuses. Stage A is audit-first; fixing statuses would destroy the evidence layer that this baseline depends on. Any reconciled status set must live in ngagent runtime state at `.git/ngagent/`, not in the legacy file.
- **Do not modify product source, tests, runtime data, configs, or package files** in `task-007`. `task-007` is restricted to non-product cleanup at the documentation/history/repository-structure layer.
- **Do not push any history rewrite** (e.g. `git filter-repo` or `BFG` to scrub `configs/mail_sender.user.json` historically) without a separate Stage B decision and consultation of remote evidence.
- **Do not republish or rebuild any release pack** during Stage A. Per `goals.md` Out of Scope: "Publishing a new release pack." Release-pack rebuild belongs to Stage B (see §8.2 W-D below).
- **Do not modify the Obsidian vault from within the repo.** The vault is external evidence and out of scope for any Stage A or Stage B repo-level action.

## 8. Stage B Entry Plan (Workstreams, First Candidates, Rationale, Sequencing, Blast Radius, Dependencies)

This section proposes how Stage B should be organized. It is a **proposal**, not a Stage B plan: Stage B is free to reorganize, drop, defer, or merge workstreams once it produces its own plan. No code is changed by this section.

The candidates below are derived from `02-release-reconciliation.md` §8 (R-01..R-05, renumbered for cross-doc clarity), `04-runtime-config-hygiene.md` §7 (recommendations), and `05-module-surface-map.md` §5 (R-01..R-13, also renumbered). To avoid identifier collision, this baseline uses `B-NN` prefixes (Stage B candidate numbers) and cross-references the upstream R-* numbers.

### 8.1 Stage B precondition (must be done before any Stage B candidate lands)

**P-01.** `task-007` cleanup application has been executed (with explicit human approval) for the §7.1 / §7.2 / §7.3 / §7.4 items the human accepts. If `task-007` is rejected or partially deferred, Stage B must explicitly re-cite `02..06` for any item it inherits as still-pending.

### 8.2 Proposed Stage B workstreams

| Workstream | Scope | Rationale | First candidates | Dependencies |
| --- | --- | --- | --- | --- |
| **W-A. Stabilize current contracts on `dev`** | Reconcile contract docs and code where they disagree; close concurrency hazards; close auth gaps that exist only because of the current loopback binding. | These are conditions Stage B must satisfy before any other refactor lands, because every later refactor reads these contracts. (`05-module-surface-map.md` §7.) | B-01, B-02, B-03 (see §8.3). | P-01. None among each other within this workstream — B-01..B-03 are independent. |
| **W-B. Disentangle cross-module imports** | Lift environment + config reads out of route handlers; relocate the SOC HTTP client; consolidate mail/template config validation. | Once W-A pins behavior, the remaining cross-module imports are the next-most-fragile seams; refactoring them touches no behavior but unlocks W-C. (`05-module-surface-map.md` §5.) | B-04, B-05, B-06. | W-A complete on the surfaces these touch (`admin.ts`, `fetchRunner`, `sections`). |
| **W-C. Deduplicate within boundaries** | Unify duplicated zod surface between `routes/courses.ts` and `routes/sections.ts`; split tall hooks; add the missing mail config schema. | Cleanup work that depends on W-A's contract decisions and W-B's location decisions. (`05-module-surface-map.md` §5.) | B-07, B-08, B-09. | W-A and W-B for the affected surfaces. |
| **W-D. Release tooling and canonical pack** | Decide the fate of `auto-refresh-tasks`; commit a real `scripts/build_release.js`; standardize archive shape and line-ending policy; resolve `frontend/i18n/messages.json` source-of-truth; rebuild a fresh release pack from the canonical merge. | The current packs are not trustworthy and zip122 is a release-blocking defect. A canonical release pipeline is needed before any external download claim. (`02-release-reconciliation.md` §8.) | B-10, B-11, B-12, B-13. Note: B-10 is *blocking* for any future release per `02-release-reconciliation.md` §8.3 R-01. | P-01 for `release/` archive disposition; W-A behavior pinning before any new pack is built; explicit human decision on `auto-refresh-tasks`. |
| **W-E. Documentation and naming hygiene** | Update `package.json` URLs to current remote name; reconcile `Start-WebUI.command` line endings; document the checkpoint-v1 format and mail dispatcher's lease/lock protocol; centralize SOC `decodeSemester`/season-code logic. | Doc-only and small-touch changes that can run alongside other workstreams. (`03-record-reconciliation.md` §6.5; `02-release-reconciliation.md` §6.3, §8.3 R-03; `05-module-surface-map.md` §5 R-11, R-12.) | B-08, B-09, B-14, B-15. | None among each other; can run in parallel with other workstreams. |
| **W-F. Frontend test floor** | The frontend currently has zero tracked tests and no `test` script. Establish a minimal Vitest/Playwright test floor before W-C splits hooks. | Zero coverage today means any frontend refactor risks silent regression. (`05-module-surface-map.md` §3.2.3.) | B-16. | None; can run in parallel with W-A/W-B; should land before W-C R-08-style hook splits. |

### 8.3 First refactor candidates (with rationale, sequencing, blast radius, dependencies)

The table below combines and renumbers the upstream R-* lists into a single Stage B entry list. "Upstream R" cross-references the source numbering in `02-release-reconciliation.md` §8 (`R-01..R-05`) and `05-module-surface-map.md` §5 (`R-01..R-13`).

| # | Candidate | Workstream | Upstream R | Priority | Blast radius | Dependencies | Rationale and sequencing note |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **B-01** | Reconcile `/api/sections` stub against `docs/query_api_contract.md` (implement, demote in docs, or formally deprecate) | W-A | `05` R-01 | High | api/, docs/, eventually frontend/ if sections is wired to the UI | P-01 | First. Doc/code disagreement on a real endpoint; any later refactor that reads sections behavior is built on sand until this is pinned. |
| **B-02** | Make `services/fetchRunner.activeJob` concurrency-safe (lock, busy-rejection, or queue) | W-A | `05` R-02 | High | api/ only; api/tests/ | P-01 | First (parallel with B-01). Live race in production-shaped code; cheap to contain. |
| **B-03** | Add an auth gate (or explicit "loopback-only" declaration) for `/admin/*` and `GET /api/subscriptions` | W-A | `05` R-03 | High | api/, possibly frontend/ MailSettingsPanel, docs/ | P-01 | Second. Today safe by deployment (oneclick binds 127.0.0.1); the next packaging that doesn't inherits an open admin surface. |
| **B-04** | Lift env + config reads out of route handlers (`admin.ts` `MAIL_CONFIG_DIR`) into `AppConfig`/container | W-B | `05` R-04 | Medium | api/, configs/ (semantic only) | B-03 | Second/Third. Pairs naturally with B-03 because auth + config can be wired through the container together. |
| **B-05** | Relocate `scripts/soc_api_client.ts` into a `services/` or `notifications/`-style library; align worker imports | W-B | `05` R-05 | Medium | workers/, scripts/, tsconfig | None on B-01..B-04 | Second/Third. Aligns import direction with the existing `notifications/mail/` pattern. Touches packaging, not behavior. |
| **B-06** | Centralize mail/template config validation in one path (`notifications/mail/config.ts`); have `scripts/mail_templates.js` import or re-export instead of re-implementing | W-B | `05` R-06 | Medium | scripts/, notifications/mail/, oneclick boot decision | B-04 | Third. Two validators are how silent drift starts. |
| **B-07** | Unify duplicated zod surface between `routes/courses.ts` and `routes/sections.ts` (or fold sections into courses if the B-01 outcome is "stub stays") | W-C | `05` R-07 | Medium | api/ only | B-01 | Third. Mechanical once B-01 decides. |
| **B-08** | Split `useCourseQuery.ts` into cache hook + transformer + query builder; centralize endpoint strings and `bcsp:*` localStorage keys | W-C / W-E | `05` R-08 | Medium | frontend/ only | B-01 (response shape stable); W-F minimum test floor desirable first | Third/Fourth. High-touch but local-blast. |
| **B-09** | Add `configs/mail_sender.schema.json` to mirror `configs/fetch_pipeline.schema.json` | W-C / W-E | `05` R-09 | Medium | configs/, notifications/mail/, admin docs | B-06 | Fourth. Pair with B-06 so the schema and resolver are co-designed. |
| **B-10** | Decide the canonical fate of `auto-refresh-tasks` (merge to `dev`, abandon, or fork to remote feature branch) | W-D | `02` R-01 | **Blocking** for any future release | api/, frontend/, workers/, configs/, docs/, release/ | None — but blocks B-13 | First Stage B decision once Stage A is closed. Without this, no release pack can be canonical. |
| **B-11** | Commit a real `scripts/build_release.js` that produces a reproducible release pack from a known git ref | W-D | `02` R-02 | High | scripts/, package.json, release/ | B-10 | Second within W-D. zip122's `package.json` already references this path; the script must exist. |
| **B-12** | Standardize release-pack shape: top-level directory wrapper named `bcsp-<version>/`, forward-slash paths, `.tar.gz` and `.zip` from the same `git archive`-style export, LF for `*.command`/`*.sh`, CRLF for `*.bat`, and UID/GID-stripped archives (per §6.1 (b)) | W-D | `02` R-03 | High | scripts/build_release.js, release/, packaging conventions | B-11 | Third within W-D. |
| **B-13** | Rebuild a fresh release pack from the canonical merge result; leave existing packs in archive until at least one verified end-to-end install of the new pack | W-D | `02` R-05 | High | release/, docs/ | B-10, B-11, B-12; explicit human approval to publish | Fourth within W-D. After this lands, the existing `release/bcsp-20260121.*` and `bcsp-20260122.zip` move to an archive path (per §7.3 with explicit human approval). |
| **B-14** | Update `package.json` `repository` / `bugs` / `homepage` URLs to the current `Rutgers-BetterCourseSchedulePlanner` remote name; update launcher user-visible "Starting BetterCourseSchedulePlanner …" strings to the current name | W-E | `03` §6.5 | Low | package.json, Start-WebUI.{bat,command} | None | Anytime. Cosmetic but trivially correct. |
| **B-15** | Document checkpoint-v1 format and mail dispatcher's lease/lock protocol in a shared spec under `docs/`; centralize SOC `decodeSemester`/season-code logic (currently duplicated between `routes/fetch.ts`, `scripts/soc_api_client.ts`, and the oneclick default `'12024'`) | W-E | `05` R-11, R-12 | Low | docs/, api/, scripts/, oneclick | B-05 (for the season-code centralization) | Late Stage B. Doc-only / small-isolated. |
| **B-16** | Establish a frontend test floor (Vitest config + minimal coverage of `useCourseQuery`, `App.tsx` mount, and at least one component test) | W-F | `05` §3.2.3 | Medium | frontend/, frontend/package.json | None | Should land before B-08 to give the hook split a safety net. |
| **B-17** | Address `act-004-discord-notify-channel` (the `03-record-reconciliation.md` §4.11 contradiction): verify whether Discord work exists on a different branch (the remote enumerates many; see `01-inventory.md` §7) before declaring it lost; if lost, formally retire the task tree | W-A or W-E | `03` §8.3 #1 | Medium | docs/, possibly notifications/ if reinstated | None | Anytime; explicit human decision on whether to recover or retire. |
| **B-18** | Verify `docs/query_api_contract.md` and `docs/ui_flow_course_list.md` against the new `examCode` + meeting-day-subset semantics; close out `rRevision.json filter-rewrite-04-docs-i18n` | W-E | `03` §8.3 #2 | Low | docs/ | B-01 (sections decision can affect contract doc edits) | Late Stage B. |
| **B-19** | Decide the fate of `frontend/src/dev/*` (gate via Vite env, relocate, or delete) and `scripts/soc_field_matrix.py` (move to an audit area; no runtime consumer) | W-E | `05` R-13 | Low | frontend/, scripts/, vite config | B-16 (so any test that wants to import the playground knows where it lives) | Anytime. |

### 8.4 Sequencing summary

1. **Stage A close-out:** This baseline is reviewed and accepted; `task-007` cleanup is approved or skipped per §7.
2. **Stage B precondition (P-01):** Apply the approved subset of §7.
3. **W-A first:** B-01, B-02, B-03 in parallel; then B-04 (Stage B contract floor).
4. **W-B and W-F in parallel:** B-05, B-06, B-16 (cross-module disentangle and frontend test floor).
5. **W-D in series:** B-10 → B-11 → B-12 → B-13 (release tooling and canonical pack).
6. **W-C dedup:** B-07, B-08, B-09 (after B-01 / B-04 / B-06 land).
7. **W-E hygiene:** B-14, B-15, B-17, B-18, B-19 — anytime, parallel to other workstreams.

### 8.5 Out-of-scope-for-Stage-B reminders

These items remain Stage A's responsibility and must not be folded into Stage B:

- Untracking `configs/mail_sender.user.json` and the runtime artifacts (per §7.1) — this is `task-007`, not Stage B (`05-module-surface-map.md` R-10).
- `.gitignore` rule additions for `scripts/poller_checkpoint.json` and `data/refresh_queue.json` (per §7.2).
- Archive moves of `record.json` / `r*.json` / `Compact/` / `review/` / `Rutgers-dr/` / `read_only.md` / `release/` / `bcsp-20260122.zip` (per §7.3).
- Cosmetic file moves and renames (per §7.4).

If `task-007` is not approved, Stage B inherits these items as still-pending and must reference §7 of this baseline when planning around them.

## 9. Open Items / Unknowns Stage B Must Resolve

A short, explicit list of unknowns that Stage A could not resolve and that Stage B must take a position on early.

| Unknown | What is unknown | Where Stage B should look first |
| --- | --- | --- |
| Fate of `auto-refresh-tasks` branch | Is the auto-refresh feature canonical (merge to `dev`) or unfinished (abandon / move to `origin/feature/auto-refresh`)? | Code reviews the feature directly; consult the human; B-10. |
| Fate of `act-004` Discord work | Does the implementation referenced by Compacts and Code-Review trailers exist on any branch? If not, retire the task tree formally. | `git log --all -- notifications/discord/`, `git log --all -- workers/discord_dispatcher.ts`; B-17. |
| Canonical SQLite path | One of `data/local.db`, `data/fresh_local.db`, `data/courses.sqlite` should be picked. | Docs + launcher first; then reconcile remaining tracked sources. (`04-runtime-config-hygiene.md` §4, §7.4.) |
| Disposition of zip122's `release:build` reference | Was `scripts/build_release.js` ever drafted (perhaps on an unmerged branch), or did zip122 ship `package.json` deltas without the supporting code? | `git log --all -- scripts/build_release.js`; B-11. |
| Whether `.env.example` should be tracked | The `!.env.example` allowlist exists with no file behind it. | Decide and either commit a sanitized `.env.example` or remove the allowlist; B-14-adjacent. |
| Whether SMTP is a future provider | `notifications/mail/types.ts` carries an `SMTPConfig` shape with no implementation. | Confirm in Stage B planning whether SMTP is on the roadmap; if not, drop the typings. (`05-module-surface-map.md` §3.4, §4.) |
| Whether `frontend/i18n/messages.json` is canonical | Carried by all release packs and by `auto-refresh-tasks`; absent from `dev`. The in-tree i18n stack on `dev` is `frontend/src/i18n/index.ts` + `frontend/src/data/fallbackDictionary.ts`. | Decide before any release rebuild (B-13 dependency). (`02-release-reconciliation.md` §8.3 R-04.) |
| Whether to push `auto-refresh-tasks` to `origin` | Currently local-only. The decision affects forensic recoverability of the auto-refresh feature surface and of Discord (if the latter exists on it). | Stage B planning, in consultation with B-10 and B-17. |

## 10. Constraint Compliance Check

- **AC-001 — Output discipline.** The only file written by this task is `.orchestrator/stage-a/06-final-baseline.md`. No product source, tests, runtime data, configs, package files, or prior Stage A reports are modified. The seven blocklist roots (`api/`, `frontend/`, `workers/`, `notifications/`, `scripts/`, `data/`, `configs/`) are read-only here. Reports `01-inventory.md` through `05-module-surface-map.md` and `.orchestrator/{goals,architecture,context_manifest}.md` are read-only here.
- **AC-002 — Upstream input consumed.** §1 cites all five upstream reports and the context manifest; §2 carries forward the evidence-layer rule from `01-inventory.md` §2 and the additional layers from `02..05`; §3 condenses each report by section number; §6 incorporates the pinned review notes for task-002 and task-003; §8 cross-references `02-release-reconciliation.md` §8 R-01..R-05 and `05-module-surface-map.md` §5 R-01..R-13 by their original numbers.
- **AC-003 — Trust posture stated.** §5 declares Trusted (§5.1), Stale (§5.2), Historical evidence (§5.3), Unresolved (§5.4), Unsafe (§5.5), and Evidence-only (§5.6). §4 declares the source-of-truth hierarchy for Stage B (T0 / T1 / T2 / T3 / NULL).
- **AC-004 — Archive / delete / untrack candidates listed with explicit human-approval gating.** §7 enumerates the §7.1 untrack set, §7.2 ignore-rule changes, §7.3 archive moves, and §7.4 cosmetic moves. Each row includes "Requires explicit human approval"; §7.5 lists actions this baseline explicitly refuses to recommend; the lead-in to §7 states "destructive changes require explicit human approval before `task-007` proceeds, and `task-007` is restricted to non-product cleanup that has been explicitly approved."
- **AC-005 — Review notes incorporated.** §6.1 addresses task-002's "low-risk count inconsistencies" (§6.1 (a)) and "packager username side-channel" (§6.1 (b)). §6.2 addresses task-003's "cosmetic bad section references" (§6.2 (a)) and "less explicit `reports/` enumeration" (§6.2 (b)). The `reports/` enumeration is then made explicit in §5.1 (the trusted-surfaces row for `reports/`).
- **AC-006 — Stage B workstreams and first refactor candidates proposed without implementing.** §8 declares six workstreams (W-A..W-F) with rationale; §8.3 enumerates 19 candidates (B-01..B-19) with priority, blast radius, dependencies, and a one-line rationale per row; §8.4 gives a sequencing summary; §8.5 lists items that must remain Stage A's responsibility. No code change is performed by this task.
- **Secret discipline.** No real keys, tokens, passwords, or PII are quoted. The placeholder shape of `configs/mail_sender.user.json` is described structurally only. The tar UID/GID side-channel string is cited only where necessary to make the finding concrete (§5.4, §5.6, §6.1 (b)) and is governed by the explicit handling rule in §6.1 (b).
- **Filesystem discipline.** No path outside `Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\task-006\` is read from disk during this task. All cross-checkout facts come from the upstream reports, which already disclosed their evidence sources and authorization scopes.
- **Authority discipline.** §2 establishes the binding rule that all evidence layers are evidence, not authority, except for `dev` HEAD as observed by `git ls-files`, which is the product surface authority for Stage A's purposes per the architecture pin. The "ignored is not endorsement" rule is preserved throughout §5 and §7.

## 11. Hand-Off

This document is the consumed input for:

- `task-007` (`07-cleanup-application.md`): inherits §7.1 / §7.2 / §7.3 / §7.4 as its explicit recommendation list. Each item must be re-checked for human approval before being applied. `task-007` may not extend its own write scope to any path not enumerated here, and may not delete any historical record or release artifact (per `goals.md` Out of Scope).
- Stage B planning: inherits §4 (source-of-truth hierarchy), §5 (trust posture), §8 (workstreams and first candidates), and §9 (open items). Stage B may reorganize, drop, defer, or merge candidates; it must cite this baseline by section when it does.
- Future Stage A audits (if any are commissioned): may cite §3 (synthesis of upstream reports), §5 (trust posture), and §6 (review-note resolution) as the canonical Stage A summary.

## 12. Acceptance Criteria Coverage Map

| AC | Requirement | Where addressed |
| --- | --- | --- |
| AC-001 | Produce only `.orchestrator/stage-a/06-final-baseline.md` and do not modify product source, tests, runtime data, configs, package files, or prior Stage A reports. | §0 (scope statement); §10 (constraint compliance); enforced at commit. |
| AC-002 | Reference and synthesize `.orchestrator/stage-a/01-inventory.md` through `05-module-surface-map.md` plus `.orchestrator/context_manifest.md` handoff notes. | §1 (method); §3 (synthesis of upstream reports, six subsections); §10 (compliance check restates citations). |
| AC-003 | State what is trusted, stale, historical, unsafe, unresolved, and evidence-only, and define a recommended source-of-truth hierarchy for Stage B. | §4 (T0/T1/T2/T3/NULL hierarchy with justifications); §5.1–§5.6 (each of the six categories required, with cross-references to upstream evidence). |
| AC-004 | List archive and delete-or-untrack candidates but state that destructive changes require explicit human approval and `task-007` is only for approved non-product cleanup. | §7.1 (untrack set, all rows gated on human approval); §7.2 (ignore-rule changes, gated); §7.3 (archive moves, gated, and explicit "no deletion proposed"); §7.4 (cosmetic moves, gated); §7.5 (refusals); §11 (hand-off restates `task-007` constraints). |
| AC-005 | Incorporate review notes for task-002 (low-risk count inconsistencies, packager username side-channel) and task-003 (low-risk section reference and reports enumeration issues). | §6.1 (task-002 (a) and (b)); §6.2 (task-003 (a) and (b)); §5.1 explicitly enumerates the six tracked `reports/` files to address §6.2 (b). |
| AC-006 | Propose Stage B workstreams and first refactor candidates with rationale, sequencing, blast radius, and dependencies — without implementing refactors. | §8.1 (precondition P-01); §8.2 (six workstreams W-A..W-F with rationale and dependencies); §8.3 (19 candidates B-01..B-19 with priority, blast radius, dependencies, and per-row rationale); §8.4 (sequencing summary); §8.5 (out-of-scope-for-Stage-B reminders); §10 (compliance check confirms no code change performed). |
