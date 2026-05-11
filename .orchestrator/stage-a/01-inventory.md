# Stage A — Repository Inventory and File Taxonomy

> **Task:** `task-001` Stage A repository inventory and file taxonomy
> **Branch:** `feature/task-001`
> **Worktree:** `Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\task-001`
> **Main checkout:** `Z:\Project\Rutgers-BetterCourseSchedulePlanner` (currently on `dev` at `5ffa415`).
> **Scope:** Audit/report only. No product source, tests, package files, runtime data, or local configs are modified by this task. The only file produced is this inventory document under `.orchestrator/stage-a/`.

## 1. Method

Evidence collection used:
- `git worktree list` to enumerate working trees.
- `git ls-tree --name-only dev` and `git ls-tree --name-only feature/task-001` to read tracked top-level entries from each branch's HEAD.
- `git ls-files`, `git ls-files <path>` for per-directory tracked sets.
- `git status --porcelain --ignored` in the task worktree.
- `git diff --stat dev..feature/task-001` to confirm what this task added.
- `git remote -v` and `git ls-remote --heads origin` for remote-state evidence.
- Content reads of representative files where category required value disambiguation (e.g. `configs/mail_sender.user.json`, `record.json`, `.gitignore`).

All file paths in this report are repository-relative. No file contents that look like real secrets were copied into this document; placeholder/test values are described, never reproduced.

Counts (from `git ls-files` in this worktree, after this task added the inventory file):
- Total tracked files: **262** (`.orchestrator/stage-a/01-inventory.md` is the +1 over the `dev` baseline of 261).
- `Compact/`: 74 files; `review/`: 6; `reports/`: 6; `Rutgers-dr/`: 2; `docs/`: 23.
- No tracked files match `*.tar.gz`, `*.zip`, or any path under `release/`.

`git diff --stat dev..feature/task-001` shows the only path-creation on this branch is `.orchestrator/stage-a/01-inventory.md` (plus three small edits to `.orchestrator/{architecture,context_manifest,goals}.md` from the planning commit that established Stage A).

## 2. Evidence Layers and Authority Disclaimer

Per Stage A policy (pinned by the Main Agent), **`.gitignore`, tracked state, ignored state, untracked state, and remote state are evidence layers, not authorities**. Each layer can be wrong or out of sync, and this inventory's classifications cite the layers used for each finding rather than treating any single layer as the source of truth.

The five layers used in this report:

| Layer | What it shows | Why it is **not** authority |
| --- | --- | --- |
| `.gitignore` rules | Stated intent to exclude certain paths. | A pattern can exist while git still tracks a path that was added before the pattern, or before `git rm --cached` was ever run. A pattern's existence does **not** prove the file is correctly excluded today. |
| Tracked state (`git ls-tree`, `git ls-files`) | What HEAD's tree records as committed. | Tracked state is shared across all worktrees of the same repo, but it can include legacy adds that contradict current intent (e.g. runtime/local files captured before the ignore rule). |
| Ignored state (`git status --porcelain --ignored`, `git check-ignore`) | Which untracked files git would currently exclude. | Per-working-tree. Does **not** retroactively untrack already-tracked files. **Today's ignored status is not an endorsement that exclusion is correct or complete** — it is only a description of git's current matching, which may be stale relative to what should be excluded. |
| Untracked state | Per-worktree; differs across worktrees and machines. | Files visible in the **main checkout** as untracked are typically invisible from a linked worktree such as `.worktrees/task-001/`. Absence in this worktree therefore does **not** mean absence in the main checkout. |
| Remote state (`git ls-remote`, GitHub public view) | What has been pushed and what is fetch-visible. | The remote is a **selective-disclosure** surface. The human has explicitly noted a prior cleanup from "everything submitted" to "only the project submitted" (Context Manifest, 2026-05-11). The remote may therefore omit branches/artifacts that the local repo still carries. Remote presence does not mean current; remote absence does not mean nonexistent. |

**Hard rule applied in this report:** the words "ignored," "not tracked," and "not in the repo" are never used to mean "correctly excluded" or "absent from the project." Where this report says a path is ignored, it cites only the matching rule, then separately flags whether that ignore is structurally appropriate. The categorization "keep / archive / delete-candidate / verify" is therefore independent of `.gitignore` status.

## 3. Top-Level Map (main-checkout perspective)

The table below enumerates all top-level entries that the main checkout (`Z:\Project\Rutgers-BetterCourseSchedulePlanner`) contains, reconciled across all five evidence layers above. The "Visible here?" column flags whether each entry is observable from this task worktree.

| Path | Tracked? | Visible in this worktree? | Evidence layer notes | Category | Recommendation |
| --- | --- | --- | --- | --- | --- |
| `.git` (file pointer in worktree; directory in main) | n/a | yes (pointer file) | gitdir plumbing. | git plumbing | keep |
| `.gitignore` | yes | yes | Evidence only; the rules listed do not prove correct exclusion (see §2). | repository metadata | keep |
| `.orchestrator/` | yes (this branch adds `stage-a/`) | yes | Newly populated current planning workspace. | current ngagent planning workspace | keep |
| `AGENTS.md` | yes | yes | Orchestrator system prompt entry doc. | current documentation | verify (vs `read_only.md`) |
| `Compact/` (74 files) | yes | yes | Per-act narrative records 2025-11-13..2025-11-23. | historical AI workflow records | archive |
| `README.md` | yes | yes | End-user docs reference a "release" packaging that lives outside the tracked repo (see §3.8). | current documentation | keep |
| `Rutgers-dr/` (2 files) | yes | yes | Deep-research distillations referenced by `record.json`. | historical AI workflow records | archive |
| `Start-WebUI.bat`, `Start-WebUI.command` | yes | yes | Cross-platform launchers shelling into `scripts/oneclick_start.js`. | product source (launchers) | keep |
| `api/` | yes | yes | Fastify backend + tests. | product source | keep |
| `configs/` (mix of `*.example.json`, `*.schema.json`, `*.user.json`, `templates/`) | partial (see §3.7) | yes | Some files are tracked despite matching `configs/*.user.json` ignore rule — illustrates "ignored is not authority." | local/private config + product config schema/templates | verify |
| `data/` (runtime + migrations + schema) | partial (see §3.6) | yes | Several files are tracked despite matching `data/*.sqlite-shm`, `data/*.sqlite-wal`, `data/poller_checkpoint.json`, `data/runtime/` ignores — same pattern as `configs/`. | runtime/generated artifacts + product config (migrations, schema) | verify |
| `docs/` (23 files) | yes | yes | Current developer/operator docs. | current documentation | keep |
| `frontend/` | yes | yes | Vite + React + TS; includes `src/dev/` developer playground. | product source | keep (verify dev-only exclusion) |
| `notebooks/` (1 file) | yes | yes | Singleton `notebooks/incremental_trial.md`. | report misplaced into a stray dir | verify (move to `reports/`) |
| `notifications/` | yes | yes | Mail provider stack + tests. | product source | keep |
| `package-lock.json`, `package.json` | yes | yes | Root workspace manifest. | product source / build manifest | keep |
| `read_only.md` | yes | yes | ngagent main-agent prompt v0.8.0, declared `<!-- ngagent-doc-version: 0.8.0 -->`. | historical AI workflow records (orchestrator doc) | verify (vs `AGENTS.md`) |
| `record.json` (~108 KB) | yes | yes | Legacy task graph; per `AGENTS.md`, the **runtime** record is at `.git/ngagent/`, so the root file is bootstrap legacy. | legacy planning JSON | archive |
| `rEmail.json`, `rRevision.json`, `rSubscribe.json` | yes | yes | Per-initiative task seeds (mail onboarding / filter rewrite / auto-term poller). | legacy planning JSON | archive |
| `reports/` (6 files) | yes | yes | Operational reports (field validation, fresh install, latency, durability). | reports | keep |
| `review/` (6 files) | yes | yes | Past review records 2025-11-13..2025-11-17; includes typo `review-recod-2025-11-14.md`. | historical AI workflow records | archive |
| `scripts/` | yes | yes | Operational + utility scripts. **Includes a tracked `scripts/poller_checkpoint.json` that does not match any ignore rule for `scripts/` — almost certainly a stray runtime artifact.** | product source + 1 stray runtime artifact | keep (untrack the stray; see §3.6) |
| `tsconfig.json` | yes | yes | Root TS project. | product source / build manifest | keep |
| `workers/` | yes | yes | Mail dispatcher + open-sections poller + tests. | product source | keep |
| **`release/`** (main-checkout untracked surface) | **no (not tracked on any branch examined)** | **NO** | Listed as relevant by the task spec and the Main Agent feedback. `.gitignore:158` rule `release/` matches it. **Visible in the main checkout per the Main Agent's evidence list; not visible in this linked worktree because untracked surfaces are per-worktree.** Existence is asserted from main-checkout evidence supplied by the Main Agent, not from filesystem traversal out of this worktree. | release artifact (main-checkout local surface) | verify (reconcile in task 02; "ignored" is not endorsement) |
| **`bcsp-20260122.zip`** (main-checkout root archive) | **no (not tracked on any branch examined)** | **NO** | Listed as relevant by the task spec and the Main Agent feedback. `.gitignore:160` rule `*.zip` matches it. **Visible at the main-checkout root per the Main Agent's evidence list; not visible in this linked worktree.** Name implies a 2026-01-22 packaging. Existence is asserted from main-checkout evidence, not from filesystem traversal. | release artifact (root archive in main checkout) | verify (reconcile in task 02; check for secret/private content; "ignored" is not endorsement) |

Top-level entries that are **not** present in the main checkout (per `git ls-tree dev`, `.gitignore` confirmation, and the main-checkout evidence supplied) and therefore are not separately reconciled in this report:
- `.claude/`, `.worktrees/`, `node_modules/`, `dist/`, `logs/`, `coverage/`, `out/`, `.next/`, `.cache/` — all `.gitignore`-matched; absent in the main checkout per the architecture pin and the supplied relevant-files list.

## 4. Category Taxonomy

### 4.1 Product source (keep)

Implementation code that defines product behavior. Out of scope for Stage A edits.

- Backend API (Fastify + zod, better-sqlite3): `api/src/server.ts`, `api/src/routes/{admin,courses,fetch,filters,health,notifications.local,sections,sharedSchemas,subscriptions}.ts`, `api/src/queries/{course_search,filters}.ts`, `api/src/services/fetchRunner.ts`, `api/src/plugins/requestLogging.ts`, `api/src/types/fastify.d.ts`, `api/src/{config,container}.ts`.
- Frontend (Vite + React + TS): `frontend/index.html`, `frontend/vite.config.ts`, `frontend/src/main.tsx`, `frontend/src/App.tsx`, `frontend/src/components/{FilterPanel,CourseList,DataFetchCard,LanguageSwitcher,LocalSoundToggle,MailSettingsPanel,SchedulePreview,SubscribeButton,SubscriptionCenter,SubscriptionManager,TagChip}.tsx`, `frontend/src/api/*.ts`, `frontend/src/hooks/*.ts`, `frontend/src/state/courseFilters.ts`, `frontend/src/i18n/index.ts`, `frontend/src/utils/*.ts`, `frontend/src/data/fallbackDictionary.ts`, `frontend/src/dev/{ComponentPlayground.tsx,mockData.ts}` (developer playground; in-tree but dev-only — see §4.10).
- Workers (Node TS): `workers/mail_dispatcher.ts`, `workers/open_sections_poller.ts`.
- Notifications (mail provider stack): `notifications/mail/{config,retry_policy,template_checker,template_loader,types}.ts`, `notifications/mail/providers/sendgrid.ts`.
- Scripts (operational + utility): `scripts/{soc_probe.ts,soc_api_client.ts,soc_normalizer.ts,soc_rate_limit.ts,soc_field_matrix.py,fetch_soc_data.ts,migrate_db.ts,incremental_trial.ts,backfill_core_attributes.ts,poller_resume_sim.ts,mail_e2e_sim.ts,mail_templates.js,oneclick_start.js,run_stack.sh,setup_local_env.sh,i18n_missing_check.ts}`.
- Build/lock manifests: root `package.json`, `package-lock.json`, `tsconfig.json` (includes `scripts/`, `api/src/`, `workers/`, `notifications/`); `frontend/package.json`, `frontend/package-lock.json`, `frontend/tsconfig.json`, `frontend/tsconfig.node.json`.
- Launchers: `Start-WebUI.bat` (Windows), `Start-WebUI.command` (POSIX) — both shell into `scripts/oneclick_start.js`.

Rationale: directly referenced by `package.json` scripts (`api:start`, `data:fetch`, etc.) and the UI bundle that ships with the product. Stage A treats these as authoritative current product code and does not edit them.

### 4.2 Tests (keep)

Co-located with their owning module.

- `api/tests/{admin_mail_config,course_search,health,notifications.local,subscriptions}.test.ts`.
- `workers/tests/{mail_dispatcher.test.ts, open_sections_poller.auto.test.ts}`.
- `notifications/mail/tests/{config,provider,retry_policy,template_checker}.test.ts`.

Rationale: standard `*.test.ts` naming, included by the root `tsconfig.json`. No `frontend/` tests are tracked. No central `tests/` dir.

### 4.3 Current documentation (keep)

User-facing and developer-facing docs that match shipping behavior:

- `README.md` — bilingual end-user install/usage; references a `release` packaging path that is **not** tracked anywhere on `dev` or `feature/task-001`; the artifact lives outside the tracked repo (see §4.8 release reconciliation pointer).
- `docs/` (23 files): runbooks and contracts. Representative paths: `docs/quickstart.md`, `docs/oneclick.md`, `docs/data_load_runbook.md`, `docs/data_refresh_strategy.md`, `docs/deployment_playbook.md`, `docs/fetch_pipeline.md`, `docs/local_data_model.md`, `docs/query_api_contract.md`, `docs/subscription_model.md`, `docs/open_event_spec.md`, `docs/mail_sender_contract.md`, `docs/mail_sender_usage.md`, `docs/mail_worker_contract.md`, `docs/notify_runbook.md`, `docs/i18n_key_map.md`, `docs/ui_flow_course_list.md`, `docs/soc_api_notes.md`, `docs/soc_rate_limit.md`, `docs/soc_field_matrix.csv`, `docs/soc_rate_limit.latest.json`, `docs/soc_rate_limit.courses_stress.json`, `docs/soc_rate_limit.courses_stress2.json`, `docs/soc_rate_limit.openSections_blitz.json`.
- `.orchestrator/goals.md`, `.orchestrator/architecture.md`, `.orchestrator/context_manifest.md` — current Stage A planning docs (authoritative for this phase).
- `.orchestrator/docs/{main_agent_playbook.md, subagent_runbook.md, eval_and_handoff.md, coverage_manifest.json}` — current ngagent operator docs.
- `frontend/README.md` — frontend-specific.

Rationale: these track the present API/UI/operations and are referenced from `package.json` scripts and runbooks.

### 4.4 Historical AI workflow records (archive)

Past-execution narrative; valuable as evidence in later Stage A passes, but not authoritative for current behavior.

- `Compact/` (74 markdown files). Naming pattern `Compact-ST-YYYYMMDD-<initiative>-<step>-<title>-<ISOZ>.md` covering acts 001–011 plus `soc-api-validation` and `filter-rewrite`. Example: `Compact/Compact-ST-20251113-act-001-01-pipeline-config-2025-11-17-T084726Z.md`. Date range observed: 2025-11-16 .. 2025-11-23.
- `review/` (6 markdown files): `review-record-2025-11-13.md`, `review-recod-2025-11-14.md` (note typo "recod"), `review-record-2025-11-15-T045714Z.md`, `review-record-2025-11-16-T125256Z.md`, `review-record-2025-11-17-T022319Z.md` (~114 KB), `review-record-2025-11-17-T023522Z.md`.
- `Rutgers-dr/` (2 files): `2025-11-11-dr.md` (Chinese-language deep-research distillation referenced by `record.json` as `md_source`), `Patch-dr-2025-11-14.md`.
- `read_only.md` — ngagent main-agent system prompt v0.8.0 (marked `<!-- ngagent-doc-version: 0.8.0 -->`). Listed here because it is duplicative with `AGENTS.md` style content and is not a current planning artifact for this codebase. **Verify** whether `AGENTS.md` and `read_only.md` are intentionally distinct.

Rationale: per `.orchestrator/goals.md`, legacy planning is "evidence, not authority"; per Stage A constraint, "Deleting historical records or release artifacts before explicit approval" is out of scope. Archive means: separate from current-source-of-truth surfaces, retain on-disk under an explicit archive layout (decision deferred to Stage A task 07-cleanup-application). No deletion is proposed here.

### 4.5 Reports (keep / verify naming)

- `reports/` (6 files):
  - `reports/field_validation.md` (~22 KB) — current SOC field validation findings.
  - `reports/field_validation_details.mdpart` — verify the `.mdpart` extension; likely an in-progress markdown fragment.
  - `reports/field_validation_samples.json` (~24 KB).
  - `reports/fresh_install_log.md` — fresh-install run report (Nov 21, 2025).
  - `reports/mail_worker_latency.md`, `reports/poller_durability.md` — performance/durability evidence.
- `notebooks/incremental_trial.md` — single trial note that semantically belongs with `reports/` (matches `reports/` style and timing, dated 2025-11-17). **Verify**: move candidate.

Rationale: these are factual measurement artifacts referenced by recent operational decisions. Treated separately from "Historical AI workflow records" because they document product behavior rather than agent process.

### 4.6 Runtime / generated artifacts (verify; many should leave tracking)

These paths are **tracked** despite matching `.gitignore` patterns. Per the authority disclaimer in §2, the fact that a `.gitignore` rule exists for a path does not retroactively untrack it; git keeps already-added paths even when later ignored. This category therefore exists precisely because **today's ignored status is not an endorsement of correct exclusion**.

- `data/courses.sqlite-shm`, `data/courses.sqlite-wal` (matches `data/*.sqlite-shm`, `data/*.sqlite-wal` — `.gitignore:146-147`).
- `data/fresh_local.db-shm` (matches `data/*.db-shm` — `.gitignore:144`).
- `data/poller_checkpoint.json` (matches `data/poller_checkpoint.json` — `.gitignore:151`). Body shows term `12026`, campus `NB`, last poll `2025-11-25T18:36:50Z`.
- `data/runtime/fetch_job_d21d4cd8-2df9-4bc8-b589-e86d19ceaf6a.json` (matches `data/runtime/` — `.gitignore:150`).
- `scripts/poller_checkpoint.json` — **separate copy** at `scripts/`. **No `.gitignore` rule matches this path** — yet the file is clearly a stray runtime checkpoint (body shows older state: term `12024`, last poll `2024-10-01`). This is the inverse of the data/ pattern: a runtime artifact that is **not** ignored, illustrating again that `.gitignore` is not authority over what is appropriate to track.

Adjacent product-config artifacts that should NOT be reclassified:
- `data/schema.sql` and `data/migrations/{001_init_schema.sql,002_relax_section_index_scope.sql,003_open_events.sql,004_course_campus_locations.sql}` — these are product-source SQL, not runtime data; keep.

Rationale: recommendation is to remove the runtime files from tracking (without deletion of local copies) in the Stage A cleanup task `07-cleanup-application.md`, after the runtime/config hygiene report (`04-runtime-config-hygiene.md`) confirms intended local-state lifecycle.

### 4.7 Local / private config (verify; secret-risk surface)

- `configs/mail_sender.example.json` — sandbox/template, uses `apiKeyEnv: "SENDGRID_API_KEY"` and `passwordEnv: "SMTP_PASSWORD"` (env-only). Safe to keep.
- `configs/mail_sender.user.json` — **tracked despite** `.gitignore:155` listing `configs/*.user.json`. The file currently contains a placeholder-style SendGrid API key (the value matches the literal placeholder used in product docs, not a functional SendGrid key); reply-to is `help@demo.test`; SMTP password uses `passwordEnv` (env reference, no value). **Risk classification:** structural — the user-local-config slot is currently checked in, which means any future write of a real key into this file would be auto-committed. **No real secret values are repeated here.** Same evidence-vs-authority story as §4.6: an existing `.gitignore` rule does not retroactively exclude an already-tracked file.
- `configs/fetch_pipeline.example.json`, `configs/fetch_pipeline.schema.json` — pipeline contract/template, safe.
- `configs/templates/email/open-seat/{en-US,zh-CN}.{html,txt}` and `configs/templates/email/verification/{en-US,zh-CN}.{html,txt}` — keep; product email templates.

Additional secret-risk surfaces audited (placeholder-only, no real values present):
- `.env`, `.env.*` are `.gitignore`-matched (`.gitignore:69-71`, with `!.env.example` allowlisted); no `.env*` file is tracked on either branch.
- No tracked file ends in `.user.json` other than the one called out above.
- No tracked file matches `*.tar.gz` or `*.zip`.

Recommendation: Stage A cleanup task should (a) remove `configs/mail_sender.user.json` from tracking and (b) replace it with documentation that it is generated at first run; this is a verify item until task 04 finalizes the hygiene report.

### 4.8 Release / archive artifacts (main-checkout local surfaces — verify drift)

This section explicitly accounts for release/archive surfaces visible in the **main checkout** (`Z:\Project\Rutgers-BetterCourseSchedulePlanner`). Per §2's authority disclaimer, absence from this task worktree does **not** establish absence from the main checkout, and `.gitignore` matching does **not** establish correct exclusion.

| Surface | Evidence layer | Status | Notes |
| --- | --- | --- | --- |
| `release/` (main-checkout untracked dir) | (a) Main-checkout evidence: present (per the Main Agent's relevant-files list and feedback). (b) `.gitignore:158` rule `release/`. (c) Tracked state: not tracked on `dev` or `feature/task-001` (`git ls-tree --name-only dev`). (d) Remote: not exposed (would be filtered by the `release/` ignore even before push). | Visible in main checkout; not visible in this worktree; not tracked; not in remote. | **The ignore rule is not endorsement.** Whether `release/` should remain a per-developer local folder, be archived, or be deleted depends on its content (release packs? source copy? secrets?) — that determination belongs to `02-release-reconciliation.md`. The reconciliation task must enumerate the directory's actual contents against `README.md`'s release-flow expectation. |
| `bcsp-20260122.zip` (main-checkout root archive) | (a) Main-checkout evidence: present (per the Main Agent's relevant-files list and feedback). (b) `.gitignore:160` rule `*.zip`. (c) Tracked state: not tracked on `dev` or `feature/task-001`. (d) Remote: not exposed. | Visible in main checkout; not visible in this worktree; not tracked; not in remote. | Name implies a 2026-01-22 (YYYYMMDD) packaging of the project. **The ignore rule is not endorsement.** The zip may contain build output, a release snapshot, or a personal backup — verifying that distinction (and whether it contains private/secret data) is a `02-release-reconciliation.md` action. This inventory only confirms the surface exists and is unaccounted for. |
| `README.md` "release" download flow | Tracked state: `README.md` references a release-archive workflow. Tracked state: no release artifact is in the tree. | Drift between docs and tracked repo. | Documented as **expected drift** because release artifacts live outside the tracked repo by design (`.gitignore:157-160` is intent-consistent here), but the *current* contents of the local `release/` directory and `bcsp-20260122.zip` are unknown to this report and must be reconciled separately. |

**Why this section exists separately:** the previous version of this inventory said "No `release/` directory in this worktree" and used that to imply the surface was simply absent. That phrasing conflates worktree absence with project absence. The corrected framing above treats worktree visibility, tracked state, ignore matching, and main-checkout existence as four separate facts.

### 4.9 Legacy planning JSON (archive)

- `record.json` (~108 KB) — `project_context.brief` matches `Rutgers-dr/2025-11-11-dr.md`; structure is a task graph (`tasks → subtasks`) annotated with statuses, citations, and artifact paths. Many entries are marked `status: "done"` but the orchestrator now uses ngagent (`.git/ngagent/`), so `record.json` is historical. The repository's `AGENTS.md` explicitly says `record.json` is managed by ngagent under `.git/ngagent/` — the **root-level file is the legacy bootstrap**, not the runtime one.
- `rEmail.json` — task seed for the no-code email enablement initiative (`T-20251122-mail-onboarding-no-code`).
- `rRevision.json` — task seed for the 11-item filter rewrite (`T-20251122-filter-rewrite`).
- `rSubscribe.json` — task seed for auto-discovering term/campus in the poller (`T-20251124-auto-term-poller`).

Rationale: separate from agent-execution narrative (Compact/review) because these are seed plans, not run logs. Stage A task `03-record-reconciliation.md` is the right venue to map these to actual file changes and current `record.json` state.

### 4.10 Unknowns / verify

- `notebooks/incremental_trial.md` — singleton in a `notebooks/` folder; semantically a report (matches `reports/` style and date). No other notebook content exists; the directory itself looks accidental. Verify whether to fold into `reports/`.
- `frontend/src/dev/{ComponentPlayground.tsx, mockData.ts}` — a developer playground inside product source. Verify whether the build excludes it for production bundling.
- `read_only.md` vs `AGENTS.md` — both are tracked orchestrator system prompts. `AGENTS.md` is the entry doc; `read_only.md` is the v0.8.0 main-agent prompt body. Verify whether one is a stale duplicate.
- `reports/field_validation_details.mdpart` — non-standard `.mdpart` extension. Verify if a renamed `.md` is intended.
- Naming typo `review/review-recod-2025-11-14.md` ("recod" → "record"). Verify rename safety against external references in `Compact/` / `record.json`.
- `scripts/poller_checkpoint.json` — a runtime checkpoint tracked under `scripts/` with **no** matching ignore rule. Inverse of the §4.6 / §4.7 pattern; reinforces that `.gitignore` is not a complete description of what *should* be excluded.

## 5. Keep / Archive / Delete-Candidate / Verify Summary

| Category | Keep | Archive | Delete-candidate (untrack only) | Verify |
| --- | --- | --- | --- | --- |
| Product source / tests / launchers / build manifests | `api/`, `frontend/`, `workers/`, `notifications/`, `scripts/` (minus stray runtime), `Start-WebUI.bat`, `Start-WebUI.command`, `package.json`, `package-lock.json`, `tsconfig.json`, `frontend/package*.json`, `frontend/tsconfig*.json`, `frontend/vite.config.ts`, `frontend/index.html` | — | — | `frontend/src/dev/*` (production exclusion) |
| Current docs | `README.md`, `docs/*`, `.orchestrator/{goals,architecture,context_manifest}.md`, `.orchestrator/docs/*`, `frontend/README.md` | — | — | `AGENTS.md` vs `read_only.md` |
| Reports | `reports/*` | — | — | `reports/field_validation_details.mdpart`, `notebooks/incremental_trial.md` (move) |
| Historical AI workflow | — | `Compact/*`, `review/*`, `Rutgers-dr/*` | — | `review/review-recod-2025-11-14.md` (rename), `read_only.md` (duplicate?) |
| Legacy planning JSON | — | `record.json`, `rEmail.json`, `rRevision.json`, `rSubscribe.json` | — | Map each entry to current files in Stage A task 03 |
| Runtime / generated artifacts (tracked despite ignore rules) | — | — | Untrack: `data/courses.sqlite-shm`, `data/courses.sqlite-wal`, `data/fresh_local.db-shm`, `data/poller_checkpoint.json`, `data/runtime/fetch_job_d21d4cd8-2df9-4bc8-b589-e86d19ceaf6a.json`, `scripts/poller_checkpoint.json` | All of the same paths — confirm in hygiene report |
| Local / private config | `configs/mail_sender.example.json`, `configs/fetch_pipeline.{example,schema}.json`, `configs/templates/email/**` | — | Untrack: `configs/mail_sender.user.json` (secret-risk surface) | Confirm replacement is generated at first run |
| Release / archive (main-checkout local) | — | — | — | Reconcile `release/` and `bcsp-20260122.zip` in Stage A task 02 |

"Delete-candidate" in this report means *delete-from-git-tracking*, not delete-from-disk. No file deletions are proposed here; that decision lives in the Stage A `07-cleanup-application.md` task after the hygiene and handoff reports. **Items in the "Delete-candidate" column are flagged because they are *tracked* in a way that contradicts evident intent, not because they are currently ignored** — the two evidence layers are independent.

## 6. Secret-Risk Surfaces (no values exposed)

- `configs/mail_sender.user.json` — a `*.user.json` config slot is currently tracked despite the `configs/*.user.json` ignore rule. The current value in this slot is a placeholder string, not a real key (SendGrid keys begin with `SG.` and continue with a real opaque token; the present value matches the literal placeholder used in product docs). Risk is **structural** (next overwrite will commit a real key), not present-value disclosure.
- `configs/mail_sender.example.json` — uses `apiKeyEnv` / `passwordEnv` references only; safe.
- `.env` / `.env.*` — `.gitignore`-matched; not present in this worktree on either branch; no leakage path observed. (Note: matching by `.gitignore` is evidence of intent, not proof of correct exclusion if a `.env` were ever force-added.)
- `data/poller_checkpoint.json`, `data/runtime/fetch_job_*.json` — operational checkpoints with internal hashes, term/campus codes, and counters; no personally identifying information observed, but they are clean-checkout pollution.
- `scripts/poller_checkpoint.json` — same shape, older snapshot; reinforces "stray copy" hypothesis.
- `release/` and `bcsp-20260122.zip` (main-checkout local surfaces, not visible in this worktree) — content is unknown to this inventory; **task 02 must verify they do not contain private keys, user credentials, or PII before any release/publish action**. Their `.gitignore` match means they will not accidentally be pushed by `git push`, but it does **not** mean they are safe; secrets-on-disk are still a leakage risk for any process that reads the working tree (CI tarballs, support uploads, screen-share captures).

No real secret values from any of the above are reproduced anywhere in this report.

## 7. Remote (Selective-Disclosure) Evidence

`git remote -v`: origin → `https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner`.

`git ls-remote --heads origin` shows many branches (`Before-Final`, `CRLF`, `Everythingbeforeemail`, `Fin-Sync-1125`, `New-Subtask`, multiple `RealST-…` and `ST-…` branches, `Reorder-Subtask`, `boli`, `eMAIL`, `dev`, `main`, etc.). The Context Manifest entry of 2026-05-11 notes that the human recalls a prior cleanup from "everything submitted" toward "only the project submitted," so today's remote-visible branch set may already be a curated subset of the historical state.

**Authority caveat:** the remote is treated as *evidence about what was published*, not as a complete record of what exists locally or what is current. Specifically:
- A path absent from the remote is **not** proven absent locally (untracked + ignored paths never went to the remote).
- A path present on the remote is **not** proven to match the local tree (the human may have cleaned local state after pushing, or vice versa).
- The branch list itself is evidence of past push activity, not of current canonical state — `dev` (where the main checkout currently sits) and `feature/task-001` (this worktree) are the only branches Stage A should treat as live working surfaces.

This section is intentionally short; the remote is one of five evidence layers, not the authority.

## 8. Constraint Compliance Check

- **Write scope:** only `.orchestrator/stage-a/01-inventory.md` is written by this task. `api/`, `frontend/`, `workers/`, `notifications/`, `scripts/`, `data/`, `configs/` are read-only.
- **No product edits:** no source, test, or runtime file is modified.
- **No deletions:** no files are removed from disk or from git tracking; all "delete-candidate" labels point forward to Stage A task 04/07 for the actual untracking decision.
- **Evidence-backed:** every classification line cites a concrete path, a `.gitignore` rule line number, a `git ls-files`/`git ls-tree` observation, or a content snippet read during this task. Where a fact about the main checkout is asserted but not observable from this worktree, the assertion is explicitly attributed to the Main Agent's relevant-files list / feedback rather than to filesystem traversal.
- **Authority discipline:** §2 establishes that `.gitignore`, tracked state, ignored state, untracked state, and remote state are evidence layers, not authorities, and the rest of the document applies that rule. The phrase "ignored is not endorsement" is used wherever a path is `.gitignore`-matched.
- **Secret discipline:** no real keys, tokens, or passwords are quoted; only structural risks are described.
- **Filesystem discipline:** no path outside `Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\task-001\` is read from disk; main-checkout facts come from `git` queries (which operate on the shared `.git/` metadata) and from the Main Agent's pinned relevant-files list / feedback.

## 9. Hand-Off

This document is the baseline input for the rest of Stage A:

- `02-release-reconciliation.md` consumes §4.8 (release/archive surfaces in the main checkout) and §6 (secret-risk implications of unverified archive contents). Must enumerate the actual contents of `release/` and `bcsp-20260122.zip` from the main checkout, then compare against `README.md`'s release-flow expectation.
- `03-record-reconciliation.md` consumes §4.9 (legacy JSON inventory) and §4.4 (historical workflow records).
- `04-runtime-config-hygiene.md` consumes §4.6 (runtime artifacts) and §4.7 (local-config / secret-risk surfaces), including the "delete-candidate" list. Must apply the §2 authority disclaimer when deciding untrack actions.
- `05-stage-b-handoff.md` and `06-final-baseline.md` consume §5 "Verify" rows.
- `07-cleanup-application.md` is the only Stage A task that may untrack files; it must cite §4.6 / §4.7 / §4.8 evidence and explicitly justify each untrack against the five evidence layers, not against `.gitignore` alone.

## 10. Acceptance Criteria Coverage Map

| AC | Requirement | Where addressed |
| --- | --- | --- |
| AC-001 | Cover top-level directories plus root files visible from the main checkout across tracked, ignored, and untracked evidence; identify keep/archive/delete-candidate/verify; cite representative paths and rationale; flag secret-risk and local-only surfaces without exposing values. | §3 (top-level map, main-checkout perspective), §4.1–§4.10 (per-category citations), §5 (summary table), §6 (secret-risk surfaces with no values). |
| AC-002 | Treat `.gitignore`, tracked state, ignored state, untracked state, and remote state as evidence only — not authority. Explicitly state that current ignored status may be inaccurate; do not imply that ignored artifacts are correctly excluded merely because they are ignored today. | §2 (evidence-layer table and "ignored is not endorsement" rule); reinforced inline in §4.6, §4.7, §4.8, §6, §7, §8. |
| AC-003 | Account for local release and archive surfaces visible in the main checkout such as `release/` and `bcsp-20260122.zip` when present, or explain absence using main-checkout evidence. | §3 (top-level map rows for `release/` and `bcsp-20260122.zip`); §4.8 (dedicated release/archive section with four evidence-layer columns); §6 (secret-risk implications); §9 (hand-off to task 02 with explicit content enumeration). |
