# Stage A — Runtime Config and Clean-Checkout Hygiene

> **Task:** `task-004` Stage A runtime config and clean-checkout hygiene audit
> **Branch:** `feature/task-004`
> **Worktree:** `Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\task-004`
> **Upstream input:** `.orchestrator/stage-a/01-inventory.md` (merged into `dev`).
> **Scope:** Audit/report only. No product source, tests, runtime data, configs, package files, or `.gitignore` are modified. The only file written by this task is this document.
> **Authority discipline (inherited from task-001):** `.gitignore`, tracked state, ignored state, untracked state, and remote state are **evidence layers, not authorities**. "Ignored" is not endorsement of correct exclusion. "Tracked" is not endorsement of correct inclusion. "Absent in this worktree" is not absence in the main checkout. Recommendations below are recommendations only; no untrack/cleanup is performed here.

## 1. Method

Evidence collection (all repository-relative, all from within this worktree or via `git` plumbing operating on the shared `.git/` metadata):

- Re-read `.orchestrator/stage-a/01-inventory.md` §2, §4.6, §4.7, §4.8, §6, §9 to preserve the upstream evidence-layer framing and pick up the secret-risk and clean-checkout pointers already enumerated there.
- `git ls-files configs/`, `git ls-files data/`, `git ls-files scripts/` to enumerate tracked content under the three runtime/config-adjacent trees.
- `git status --porcelain --ignored` in this worktree (returned empty — this linked worktree carries no untracked or ignored content; that says nothing about the main checkout).
- `git check-ignore --no-index -v <path>` for each candidate path to record the exact `.gitignore` rule line a path matches (or to confirm no rule matches).
- `git log --diff-filter=A --pretty=…` on the seven flagged tracked-despite-ignore-rule paths to record which historical commit first added each one.
- `git ls-files` filtered by `findstr` for `.env`, `migrations.log`, `staging`, `refresh_queue`, `logs` to confirm what is and is not tracked.
- Content reads of: `Start-WebUI.bat`, `Start-WebUI.command`, `scripts/oneclick_start.js`, `scripts/setup_local_env.sh`, `scripts/run_stack.sh`, `scripts/migrate_db.ts` (first 60 lines), `api/src/config.ts`, `api/src/routes/admin.ts` (focused on mail-config write path), `notifications/mail/config.ts`, `configs/mail_sender.example.json`, `configs/mail_sender.user.json`, `configs/fetch_pipeline.example.json`, `frontend/vite.config.ts`, `frontend/src/api/client.ts`, `data/poller_checkpoint.json`, `scripts/poller_checkpoint.json`, `data/runtime/fetch_job_d21d4cd8-2df9-4bc8-b589-e86d19ceaf6a.json`, `data/schema.sql` (header), `README.md`, `docs/quickstart.md`, `docs/oneclick.md`, `docs/deployment_playbook.md`.
- Pattern grep (full repo): `process\.env\.[A-Z_]+`, `SENDGRID_API_KEY`, `SMTP_PASSWORD`, `data/local\.db|data/courses\.sqlite|data/fresh_local\.db`, `configs/.+\.local\.json`, `Z:\\`.

**No real secret values from any tracked or untracked file are reproduced in this document.** Where a tracked file currently holds a placeholder-shaped credential, that fact is described structurally.

## 2. Evidence-Layer Discipline (Inherited)

This report reuses the five-layer model from `01-inventory.md` §2 verbatim and applies it consistently below:

| Layer | What it shows | Authority caveat applied here |
| --- | --- | --- |
| `.gitignore` rules | Stated intent to exclude paths. | A pattern's existence does **not** prove correct exclusion today (legacy `git add` predating the rule still wins). |
| Tracked state (`git ls-tree`, `git ls-files`) | What HEAD records as committed. | Tracking can encode wrong intent (a runtime artifact may be tracked simply because no one ever ran `git rm --cached`). |
| Ignored state (`git status --ignored`, `git check-ignore`) | What git would currently exclude. | Per-working-tree. Reports a *match*, not an *endorsement*. Already-tracked paths are unaffected. |
| Untracked state | Per-worktree; differs across worktrees and machines. | The main checkout (`Z:\Project\Rutgers-BetterCourseSchedulePlanner`) holds untracked surfaces (release/, archive zip, `node_modules/`, local DB, generated `*.local.json`, `logs/`) that this worktree cannot enumerate. Absence here is not absence there. |
| Remote state (`git ls-remote`, GitHub) | What has been published. | The Context Manifest entry of 2026-05-11 records that the human curated the remote from "everything submitted" to "only the project submitted"; remote presence/absence is selective evidence, not the project's source of truth. |

**Hard rule, repeated for this task's scope:** the words "ignored", "not tracked", and "not in the repo" are never used to mean "correctly excluded" or "absent from the project". Each finding cites the matching rule (when one matches) and separately states whether the rule was structurally appropriate.

## 3. Bootstrap Map — What a Clean Checkout Needs Before the App Can Run

The launcher and helper scripts each instantiate a slightly different bootstrap. The table below reconciles the three documented entry points against the artifacts they require, the templates they expect to copy from, and the env variables they read.

| Entry point | Tracked launcher | Calls | Generates/expects | Notes |
| --- | --- | --- | --- | --- |
| `Start-WebUI.bat` (Windows) | yes (`Start-WebUI.bat:13`) | `node scripts/oneclick_start.js` | Node 22+ on PATH; `node_modules/` (root + `frontend/`); `configs/fetch_pipeline.local.json` (auto-copied from `configs/fetch_pipeline.example.json` at `scripts/oneclick_start.js:188-194`); SQLite DB at `data/fresh_local.db` by default (`scripts/oneclick_start.js:40`); `data/poller_checkpoint.json` (auto-created at `scripts/oneclick_start.js:494`); `configs/mail_sender.user.json` (consumed only if present, can be created by the WebUI — see §6). | Only entry point intended for end-users in `README.md`. Pre-checks `node` on PATH and opens the Node.js download page if missing. |
| `Start-WebUI.command` (macOS/Linux GUI launcher) | yes (`Start-WebUI.command:16`) | same `scripts/oneclick_start.js` | same as above | Symmetric POSIX path. |
| `scripts/setup_local_env.sh` (developer bootstrap) | yes | `npm install` (root + `frontend/`); migrations; optional `data:fetch` | `configs/fetch_pipeline.local.json` (copies from example if missing, `setup_local_env.sh:55-127, 254`); `configs/mail_sender.local.json` (copies from `configs/mail_sender.example.json` at `setup_local_env.sh:254`); `.env.local` and `frontend/.env.local` (written from inline heredoc at `setup_local_env.sh:143-163`); `data/local.db` is the documented default in this path; `data/migrations.log` is appended by `npm run db:migrate` per `docs/deployment_playbook.md:57`. | Documented in `docs/quickstart.md` and `docs/deployment_playbook.md` as the dev path. |
| `scripts/run_stack.sh` (developer runner) | yes | starts API + frontend + poller (+ optional mail dispatcher) | Expects an already-present DB (`run_stack.sh:380-384` exits if missing); writes per-component log files under `logs/run_stack/`. | Not a bootstrap script — it asserts setup has already happened. |
| Direct `npm run db:migrate` / `npm run data:fetch` / `npm run api:start` | n/a | the underlying TS scripts (`scripts/migrate_db.ts`, `scripts/fetch_soc_data.ts`, `api/src/server.ts`) | Each script reads its own default. | See §4 for the path-default divergence this entry point exposes. |

### 3.1 Required-to-generate artifacts (clean-checkout perspective)

Items below are **not** tracked. A clean checkout must produce each one before (or during) the relevant command can succeed. The right-hand column states whether the producing path is fully covered by `.gitignore`.

| Artifact | Producer | `.gitignore` coverage | Clean-checkout status |
| --- | --- | --- | --- |
| `node_modules/` (root) | `npm install` (auto by `oneclick_start.js` or `setup_local_env.sh`) | yes — `.gitignore:41` `node_modules/` | OK. |
| `frontend/node_modules/` | `npm install` in `frontend/` | yes — same rule, applies under any directory | OK. |
| `configs/fetch_pipeline.local.json` | `oneclick_start.js:188-194` copies from `configs/fetch_pipeline.example.json`; `setup_local_env.sh:254` copies from same example. | yes — `.gitignore:154` `configs/*.local.json` | Template present (`configs/fetch_pipeline.example.json` tracked). OK. |
| `configs/mail_sender.local.json` | `setup_local_env.sh:254` copies from `configs/mail_sender.example.json`. The launcher does **not** auto-create this — it inspects `configs/mail_sender.user.json` first and falls back to `.local.json` only if it exists. | yes — same `configs/*.local.json` rule | Template present (`configs/mail_sender.example.json` tracked). OK. |
| `configs/mail_sender.user.json` | WebUI: `api/src/routes/admin.ts:121` writes this file when the user clicks "Save Email Settings" in the UI (path computed at `admin.ts:215-222`). Also the launcher's preferred config source (`oneclick_start.js:28`). | yes — `.gitignore:155` `configs/*.user.json` | **NOT OK on this repo.** The path is currently *tracked* (see §5.1) — the next save in the WebUI will rewrite a tracked file with a real SendGrid key body. Recommendation in §7. |
| `data/local.db` *or* `data/fresh_local.db` *or* `data/courses.sqlite` | `npm run db:migrate` creates the DB; `npm run data:fetch` populates it. Which path is created depends on entry point (see §4). | yes — `.gitignore:142-143` `data/*.db`, `data/*.sqlite` | Template absent (none required — `data/schema.sql` + `data/migrations/*.sql` are tracked product source). OK. |
| `data/poller_checkpoint.json` | `workers/open_sections_poller.ts` writes it; `oneclick_start.js:494` ensures the parent dir exists. | yes — `.gitignore:151` `data/poller_checkpoint.json` | **NOT OK on this repo.** Path is *tracked* (see §5.1). Will reappear as a modified file the first time the poller runs after clone. |
| `data/runtime/<job-id>.json` | `data:fetch` writes per-run fetch-pipeline-config snapshots. | yes — `.gitignore:150` `data/runtime/` | **NOT OK on this repo.** One snapshot (`data/runtime/fetch_job_d21d4cd8-…json`) is *tracked* with a stale absolute path (see §5.1, §5.2). |
| `data/migrations.log` | `npm run db:migrate` appends to it. | yes — `.gitignore:148` | Not tracked. OK. |
| `data/staging/` | `data:fetch` writes intermediate dumps; `setup_local_env.sh:267` pre-creates the dir. | yes — `.gitignore:149` `data/staging/` | Not tracked. OK. |
| `data/refresh_queue.json` | Referenced as `incremental.resumeQueueFile` in `configs/fetch_pipeline.example.json:79`. Produced by the incremental fetcher. | **NO** — there is no `.gitignore` rule that matches `data/refresh_queue.json` (`git check-ignore --no-index -v` returns no match). | **NOT OK structurally.** If/when the incremental fetcher runs and writes this file, it will land **untracked but unignored**. A subsequent `git add .` would commit it. Recommendation in §7. |
| `logs/fetch_runs/summary_latest.{log,json}` | `fetch_soc_data.ts` writes via `summary.writeText`/`summary.writeJson` in fetch config. | yes — `.gitignore:2` `logs` and `.gitignore:3` `*.log` | Not tracked. OK. |
| `logs/run_stack/<component>.log` | `scripts/run_stack.sh:269` writes one log file per component. | yes — same rules | Not tracked. OK. |
| `.env.local`, `frontend/.env.local` | `scripts/setup_local_env.sh:143-163` writes from inline templates. | yes — `.gitignore:69-71` `.env`, `.env.*`, `!.env.example` | **Partial gap.** The `.env.example` allowlist exists, but **no `.env.example` file is tracked** in the repo. The dev path therefore has a tracked allowlist for a template that does not exist; the documented dev `.env.local` shape lives only inside `setup_local_env.sh` and `docs/deployment_playbook.md:30-44`. Not a clean-checkout blocker (the launcher writes the file itself), but a documentation/template inconsistency. |

### 3.2 Required-to-exist tracked templates

All four templates the bootstrap scripts copy from are tracked and present in this worktree:

- `configs/fetch_pipeline.example.json` (`configs/fetch_pipeline.schema.json` is the matching schema and is also tracked).
- `configs/mail_sender.example.json` (uses `apiKeyEnv: "SENDGRID_API_KEY"` and `passwordEnv: "SMTP_PASSWORD"` — env references only; safe shape).
- `data/schema.sql` (raw schema text — not used by `migrate_db.ts`, but kept for reference) and `data/migrations/00{1,2,3,4}_*.sql` (the canonical migration sources).
- `configs/templates/email/{open-seat,verification}/*.html|.txt` (email body templates referenced by the mail config).

There is **no tracked `.env.example`**. The `.gitignore:71` allowlist is forward-looking only.

### 3.3 Required-to-be-on-PATH prerequisites

- Node.js 22+ (`oneclick_start.js:9` `MIN_NODE_MAJOR = 22`; `docs/quickstart.md:6` says "Node.js 22+").
- npm (bundled with Node).
- `python3` is referenced as **optional** for sanity checks (`docs/quickstart.md:8`, `docs/deployment_playbook.md:21`) and by one tracked script: `scripts/soc_field_matrix.py` (executed via `npm run soc:field-matrix`).
- On Windows, the launcher detects WSL paths and warns (`scripts/oneclick_start.js:42-45`), but does not require WSL.
- Outbound HTTPS to `classes.rutgers.edu` (data fetch) and SendGrid (`docs/deployment_playbook.md:22`).
- For native module rebuilds (`better-sqlite3`), Windows users may need "Microsoft C++ Build Tools" — `oneclick_start.js:360-362` raises a specific error message recommending this.

None of these external prerequisites are checked into the repo and none can be — they are end-user responsibilities. The launchers do degrade gracefully (open the Node.js download page if `node` is missing; force-reinstall `better-sqlite3` if its native binary mismatches).

## 4. Path-Default Divergence (Bootstrap Blocker Class)

Three competing canonical SQLite paths exist in tracked sources; the actually-used path depends on the entry point and overrides. This is not a bug per se (env vars and CLI flags reconcile in practice), but it is a clean-checkout footgun and explains why the repo currently carries SHM/WAL stragglers for *two different* DB filenames.

| Default | Where it appears | Evidence |
| --- | --- | --- |
| `data/local.db` | `scripts/migrate_db.ts:28`; `scripts/setup_local_env.sh:6`; `scripts/run_stack.sh:7`; `api/src/config.ts:10` (`SQLITE_FILE` default); `workers/open_sections_poller.ts:397, 406`; `docs/quickstart.md`, `docs/deployment_playbook.md`, `docs/notify_runbook.md`, `docs/query_api_contract.md`, `docs/data_refresh_strategy.md`. | The dev path's documented default. |
| `data/fresh_local.db` | `scripts/oneclick_start.js:40, 44, 203`; `reports/fresh_install_log.md:3, 6, 11`; `Compact/Compact-ST-…act-006-03-fresh-run-2025-11-21-T192107Z.md:14, 17, 22`. | The end-user one-click launcher's default. |
| `data/courses.sqlite` | `configs/fetch_pipeline.example.json:5` (`sqliteFile`); `scripts/backfill_core_attributes.ts:14`; `reports/field_validation.md`; `docs/fetch_pipeline.md:61`; `docs/data_load_runbook.md:5, 12, 26`. | The fetch pipeline's documented default; also the historical default. |

Operational consequence (consumed downstream into Stage A task 05 / 06):

- A user who follows `README.md` (release-pack double-click) gets `data/fresh_local.db`.
- A developer who follows `docs/quickstart.md` (manual `setup_local_env.sh`) gets `data/local.db`.
- A developer who runs `npm run data:fetch` directly without `--config configs/fetch_pipeline.local.json` will use the example's `data/courses.sqlite`.
- The currently tracked SHM/WAL artifacts (`data/courses.sqlite-shm`, `data/courses.sqlite-wal`, `data/fresh_local.db-shm`) are historical witnesses to two different DB names that were used on the source machine, then committed.

This is the runtime evidence that `data/` was at some point treated as part of the project, **not** as per-developer local state. The fix is documentation-and-untrack discipline; see §7.

## 5. Tracked-Despite-Ignored vs Unignored-Despite-Runtime-Shape

Per AC-003, this section enumerates the inverse pattern in both directions. Each entry cites the `.gitignore` rule that does or does not match, the commit that first added the path to tracking, and the structural risk.

### 5.1 Tracked despite a matching `.gitignore` rule (ignore is not endorsement of exclusion — the file is still in the index)

Verified via `git check-ignore --no-index -v <path>` (returns the matching rule even for tracked paths since `--no-index` ignores the index state).

| Path | Matching rule | First added | Local shape (no secret values) |
| --- | --- | --- | --- |
| `configs/mail_sender.user.json` | `.gitignore:155` `configs/*.user.json` | commit `f6b10e2` (2025-11-26, `Fin-Sync-1125`) | Mail config slot. `providers.sendgrid.apiKey` is currently a placeholder string (matches the literal placeholder shape used in product docs, not a functional key). `replyTo.email` is `help@demo.test` (synthetic domain). `testHooks.dryRun: false`. **Risk: structural — the WebUI save endpoint (`api/src/routes/admin.ts:121`) writes this exact path; the next user save will replace the placeholder with a real `SG.…` token and the tracked diff will contain a real secret.** No values are reproduced here. |
| `data/poller_checkpoint.json` | `.gitignore:151` `data/poller_checkpoint.json` | commit `fe48fbb` (2025-11-23, `rRevision-ST-20251122-filter-rewrite-02-api-schema-query`) | Live poller checkpoint. Shape: `{ version, updatedAt, campuses: { "12026\|NB": { term, campus, lastPollAt, lastSnapshotHash, openIndexes: int, misses: {} } } }`. Snapshot from `2025-11-25T18:36:50Z`. **Risk: bootstrap blocker — first poller run on a clean checkout will rewrite the snapshot hash and timestamp for whatever term/campus the user chose, producing an immediate dirty working tree.** |
| `data/runtime/fetch_job_d21d4cd8-2df9-4bc8-b589-e86d19ceaf6a.json` | `.gitignore:150` `data/runtime/` | commit `e4bc155` (2025-11-23, `Everythingbeforeemail`) | A frozen copy of a `fetch_pipeline.local.json`-shaped config used for one fetch run. Contains target `{ term: 92025, campuses: [NB], subjects: [ALL] }`. **Carries a stale absolute Windows path `Z:\Project\BetterCourseSchedulePlanner\data\courses.sqlite` (note the repo name is missing the `Rutgers-` prefix it carries today at `Z:\Project\Rutgers-BetterCourseSchedulePlanner`). This is the only stale-absolute-path occurrence in tracked sources.** Risk: machine-specific path checked into git; no functional impact (the file is never read by the runtime — it is a write-only run-log artifact), but it is residue. |
| `data/courses.sqlite-shm` | `.gitignore:146` `data/*.sqlite-shm` | commit `60a45a4` (2025-11-23, `sYNC-1123`) | SQLite WAL shared-memory file. Binary. No companion `data/courses.sqlite` in tracked state — the actual DB was correctly excluded; only the sidecar slipped in. |
| `data/courses.sqlite-wal` | `.gitignore:147` `data/*.sqlite-wal` | same commit `60a45a4` | SQLite write-ahead log. Binary. Same shape: orphaned sidecar without the parent DB. |
| `data/fresh_local.db-shm` | `.gitignore:144` `data/*.db-shm` | commit `fe48fbb` (2025-11-23) | SQLite WAL shared-memory file for the `fresh_local.db` instance. The matching `.db` and `.db-wal` are correctly absent from tracking — only the shm slipped in. |

All six rows above are evidence that **`.gitignore` matching does not equal correct exclusion**. Six paths matched by ignore rules are nonetheless in the index because they predate the rule, or because the rule was added after the original `git add`. Untracking them is a `git rm --cached` decision deferred to Stage A task 07 (`07-cleanup-application.md`).

### 5.2 Unignored despite a runtime / local-state shape (the inverse pattern)

| Path | Has any `.gitignore` rule? | First added | Local shape |
| --- | --- | --- | --- |
| `scripts/poller_checkpoint.json` | **No.** `git check-ignore --no-index -v scripts/poller_checkpoint.json` returns no match (`::` line confirms non-match). The repo's `data/poller_checkpoint.json` rule is path-anchored and does not match the `scripts/` location. | commit `3e92660` (2025-11-20, `ST-20251113-act-010-03-resume-tests`) | A second poller checkpoint sitting under `scripts/`. Older snapshot: `lastPollAt: 2024-10-01T11:59:00.000Z`, `term: 12024`, `lastSnapshotHash` differs from the `data/` copy. **Inverse risk: the file looks like product source by location, but it is identical in shape to the runtime checkpoint that `data/poller_checkpoint.json` carries. It is almost certainly a stray runtime write to the wrong directory (likely from a test or resume-sim run where the cwd was `scripts/`), and it is structurally a runtime artifact but currently a *committed product file*.** No ignore rule will pick it up because of where it lives, even if the data/ rule is preserved exactly as written. |
| `data/refresh_queue.json` (not present locally, referenced by `configs/fetch_pipeline.example.json:79`) | **No.** `git check-ignore --no-index -v data/refresh_queue.json` returns no match. The closest siblings are `data/migrations.log`, `data/staging/`, `data/poller_checkpoint.json`, `data/runtime/` — none cover this name. | n/a (not tracked, not yet generated locally) | If/when an incremental fetch runs against `configs/fetch_pipeline.local.json` (which inherits this field unchanged from the example), the fetcher writes `data/refresh_queue.json`. It will land untracked and **unignored**. A subsequent `git add .` (or a tooling sweep) would commit a runtime queue file. **Inverse risk: future-tense — a runtime artifact whose path is documented in tracked config but is not covered by any ignore rule.** |

These two rows are evidence that **ignore-rule absence does not equal "correctly tracked"** any more than ignore-rule presence equals "correctly excluded". The two evidence layers are independent of intent.

### 5.3 Cleanly tracked product config that *looks* runtime-ish (do not reclassify)

For completeness: a few paths under `data/` and `configs/` are correctly tracked even though they share the directory with the runtime artifacts above. They should not be flagged for untracking.

- `data/schema.sql` — DDL reference; product source. Not consumed by `migrate_db.ts` (which reads `data/migrations/*.sql`), but kept as a human-readable schema view.
- `data/migrations/001_init_schema.sql`, `data/migrations/002_relax_section_index_scope.sql`, `data/migrations/003_open_events.sql`, `data/migrations/004_course_campus_locations.sql` — canonical migration source. Hash-pinned by `migrate_db.ts`.
- `configs/fetch_pipeline.example.json`, `configs/fetch_pipeline.schema.json` — pipeline template + schema. Both safe.
- `configs/mail_sender.example.json` — env-only credential references; safe.
- `configs/templates/email/{open-seat,verification}/{en-US,zh-CN}.{html,txt}` — product email bodies.

None of these match any ignore rule, and none should.

## 6. Secret-Risk Surfaces (no values exposed)

This section is the runtime/config-hygiene complement to `01-inventory.md` §6. It focuses on *operational* secret-leak vectors, not on a static read of every config.

| Surface | Risk class | Evidence | Why it matters for a clean checkout |
| --- | --- | --- | --- |
| `configs/mail_sender.user.json` (tracked; see §5.1) | **Active write target for a real secret.** | `api/src/routes/admin.ts:121` writes this exact path via `fs.writeFile(paths.userConfigPath, JSON.stringify(mergedConfig, null, 2) + '\n', 'utf8')`. The path is computed by `getConfigPaths()` at `admin.ts:215-222` using `MAIL_CONFIG_DIR` env (defaulting to `configs/`). | A user who completes the WebUI "Save Email Settings" flow against the current `dev` checkout writes a real SendGrid API key into a tracked file. The next `git status` will show `configs/mail_sender.user.json` as modified with real-secret diff content. The user is one `git add` away from committing the key. **Highest-priority structural risk in this report.** |
| `configs/mail_sender.example.json` | Safe — env-only. | Reads `apiKeyEnv: "SENDGRID_API_KEY"` and `passwordEnv: "SMTP_PASSWORD"`; `notifications/mail/config.ts:190-211` enforces that a SendGrid section provide either `apiKey` or `apiKeyEnv`, and `apiKey` is *not* present in the example. | No value disclosure path. Safe to keep tracked. |
| `.env` / `.env.*` | No file currently tracked. Allowlist `!.env.example` exists but has no tracked file behind it. | `.gitignore:69-71`; `git ls-files` shows no tracked `.env*`. `docs/deployment_playbook.md:30-44` documents the expected `.env.local` shape inline (heredoc); `scripts/setup_local_env.sh:143-163` writes both `.env.local` files from inline templates. | The dev path generates env files at first run rather than copying from a template. Recommendation: see §7 — track a `.env.example` so the rule has the artifact it allowlists, and so the launcher heredoc has a single source of truth. |
| `data/poller_checkpoint.json` (tracked; see §5.1) | Low — internal-only data. | Shape is `{version, updatedAt, campuses: {…lastSnapshotHash…}}`. No user identifiers; only term/campus codes and SHA-1 snapshot hashes. | Not a credential leak. It is a runtime-residue surface that makes every clean clone start with an immediately-dirty working tree. |
| `data/runtime/fetch_job_<uuid>.json` (one tracked; see §5.1) | Low — local-path leak only. | Contains the absolute Windows path of the source machine's prior DB location. No tokens. | Not a credential leak. Exposes one developer's local filesystem layout (path leak, machine-fingerprintable). |
| `scripts/poller_checkpoint.json` (unignored; see §5.2) | Low — internal-only data. | Same shape as `data/poller_checkpoint.json`; older snapshot. | Not a credential leak. Same runtime-residue category. |
| `release/`, `bcsp-20260122.zip` (main-checkout local; not visible in this worktree) | **Unknown content — verify in task 02.** | `01-inventory.md` §4.8, §6 already flagged this. `.gitignore:158, 160` match — so the surfaces won't accidentally be pushed by `git push` — but secrets-on-disk are still a leak risk for any process that reads the working tree (CI tarballs, support uploads, screen captures). | Not in this report's write scope; it is task 02's verify target. Reproduced here as a cross-reference. |

**No real secret values from any file in this table are reproduced anywhere in this document.**

## 7. Recommendations (Recommendations Only — No Files Modified)

Per the task spec, this section produces **recommendations only**. No untrack, gitignore edit, or packaging change is performed by this task. Each recommendation is keyed to evidence above and is queued for the relevant downstream Stage A task.

### 7.1 Untrack runtime artifacts (queue for `07-cleanup-application.md`)

Untrack-from-git-tracking (i.e. `git rm --cached <path>`) — no on-disk deletion proposed. Each path is matched by an existing `.gitignore` rule, so untracking is sufficient; no rule change is required for these six.

| Path | Justification (cite §5.1) | Existing rule preserves intent after untrack |
| --- | --- | --- |
| `configs/mail_sender.user.json` | Highest priority. WebUI actively writes this path with real SendGrid keys; current tracked state turns a normal user flow into a secret-leak path. The example file already provides the template (§3.2). | `.gitignore:155` `configs/*.user.json` |
| `data/poller_checkpoint.json` | Pure runtime state; rewritten on every poller run. Recreated automatically by the launcher (`oneclick_start.js:494`). | `.gitignore:151` `data/poller_checkpoint.json` |
| `data/runtime/fetch_job_d21d4cd8-2df9-4bc8-b589-e86d19ceaf6a.json` | Per-run write-only artifact. Also leaks a stale absolute Windows path (`Z:\Project\BetterCourseSchedulePlanner\data\courses.sqlite`) referencing a now-renamed repo. | `.gitignore:150` `data/runtime/` |
| `data/courses.sqlite-shm` | SQLite sidecar; orphaned (no parent `.sqlite` is tracked). Regenerated by SQLite at runtime. | `.gitignore:146` `data/*.sqlite-shm` |
| `data/courses.sqlite-wal` | Same; sidecar without parent. Regenerated by SQLite at runtime. | `.gitignore:147` `data/*.sqlite-wal` |
| `data/fresh_local.db-shm` | Same; orphaned `.db-shm` without the parent `.db`. | `.gitignore:144` `data/*.db-shm` |

### 7.2 Reclassify a tracked path that has no ignore coverage (queue for `07-cleanup-application.md`)

| Path | Justification | Recommendation |
| --- | --- | --- |
| `scripts/poller_checkpoint.json` | Runtime shape, but committed under `scripts/` where no rule reaches it. Provenance (§5.2) shows it was added by a resume-tests act, consistent with a stray cwd. The "production" location is `data/poller_checkpoint.json`. | Untrack (`git rm --cached`). **Then** — and only then — propose either (a) widening the ignore rule (`.gitignore` recommendation: append `scripts/poller_checkpoint.json` or change the data-anchored rule to `**/poller_checkpoint.json`), or (b) renaming the in-tree usage so the file is regenerated in `data/`. The rule change is a `.gitignore` edit and therefore explicitly outside this task's write scope; the proposal is recorded here as input to task 07. |

### 7.3 Pre-empt a future unignored runtime artifact (queue for `07-cleanup-application.md`)

| Path | Justification | Recommendation |
| --- | --- | --- |
| `data/refresh_queue.json` (not yet present in this checkout) | Referenced by `configs/fetch_pipeline.example.json:79` as `incremental.resumeQueueFile`. The fetcher will write it on incremental runs. No `.gitignore` rule covers it (`git check-ignore --no-index -v` returns no match). | Recommend a `.gitignore` line such as `data/refresh_queue.json` or `data/*queue.json`. The rule edit is outside this task's scope; recorded as input to task 07. Note: this is a forward-looking recommendation; nothing is currently broken because the file does not yet exist in tracked or untracked form. |

### 7.4 Documentation / packaging hygiene (queue for `05-stage-b-handoff.md` or `06-final-baseline.md`)

| Item | Justification | Recommendation |
| --- | --- | --- |
| `.env.example` is allowlisted but missing | `.gitignore:71` `!.env.example` is forward-looking permission; no template file exists. The `.env.local` shape lives in `scripts/setup_local_env.sh:143-163` and `docs/deployment_playbook.md:30-44` only. | Document the situation; if the team wants a single source of truth, track a sanitized `.env.example` (no secrets). Choice is a Stage B / final-baseline call. |
| Three competing DB-path defaults (§4) | `data/local.db`, `data/fresh_local.db`, `data/courses.sqlite` all appear as defaults in tracked sources. The runtime SHM/WAL stragglers in §5.1 are direct evidence that more than one of these was used on the source machine and committed. | Pick one canonical path (the launcher's `data/fresh_local.db` is the end-user-facing default; `data/local.db` is the dev-doc default; `data/courses.sqlite` is the example-config default). Reconcile in a Stage B doc/code pass. **Out of scope for this task.** |
| Stale absolute Windows path in `data/runtime/fetch_job_d21d4cd8-…json` | Absolute path `Z:\Project\BetterCourseSchedulePlanner\…` references the pre-rename repo. | Covered by §7.1 (untrack the file). After untracking, no fix is needed — future fetch-job snapshots will live untracked under `data/runtime/`. |
| `README.md` describes a release pack flow but Start-WebUI lives at the repo root | The end-user-facing `README.md` (§1) tells a user to "download the release archive" and "find the one-click script (e.g., `start.bat`)", but the actually shipping launchers in this checkout are `Start-WebUI.bat` / `Start-WebUI.command`. This is a release-reconciliation question (`02-release-reconciliation.md`'s domain), not a runtime-hygiene one. | Cross-reference only. |

### 7.5 Things this task explicitly **does not** recommend

- Do not delete `data/`, `configs/`, or `scripts/` files from disk. All untrack recommendations in §7.1–§7.3 are `git rm --cached` only.
- Do not rewrite history to remove the placeholder/synthetic content currently in `configs/mail_sender.user.json`. The current value is structurally a placeholder, not a real key (no value is reproduced here). History rewriting is a Stage B / dedicated remediation decision and would also need `02-release-reconciliation.md`'s remote evidence before being safe to push.
- Do not edit `.gitignore` in this task — the proposed rule additions in §7.2 and §7.3 are explicit inputs to task 07.

## 8. Constraint Compliance Check

- **AC-001 — Write scope:** the only file produced by this task is `.orchestrator/stage-a/04-runtime-config-hygiene.md`. No product source, tests, runtime data, configs, package files, or `.gitignore` is modified. The seven blocklist roots (`api/`, `frontend/`, `workers/`, `notifications/`, `scripts/`, `data/`, `configs/`) are read-only here.
- **AC-002 — Upstream input consumed:** §1, §2 cite `.orchestrator/stage-a/01-inventory.md`. §5 reuses the `01-inventory.md` §4.6, §4.7 enumeration and extends it with first-add commits, runtime shapes, and the inverse `scripts/poller_checkpoint.json` and `data/refresh_queue.json` cases. The "ignored is not endorsement" rule from `01-inventory.md` §2 is applied verbatim throughout — see the table headings in §3.1, §5.1, §5.2, the explicit caveat in §3, and the lead-in to §7.
- **AC-003 — Ignored surfaces enumerated without disclosing values; tracked-despite-ignore and unignored-despite-runtime cases stated:** §3.1 (runtime artifacts column-flagged for ignore coverage), §5.1 (six tracked-despite-ignore rows with rule-line citations), §5.2 (two unignored-despite-runtime-shape rows, one current and one forward-looking), §6 (secret-risk surfaces, all described structurally with **no value reproduction**).
- **AC-004 — Clean-checkout DB/config/startup prerequisites assessed:** §3 (bootstrap map per entry point), §3.1 (per-artifact generation source), §3.2 (tracked templates available), §3.3 (external prerequisites), §4 (path-default divergence as a clean-checkout footgun), §6 (the mail-config write path as the highest-priority secret risk).
- **AC-005 — Stale absolute paths, runtime residue, bootstrap blockers identified; recommendations only:** §4 (path-default divergence — runtime residue evidence), §5.1 (stale Windows absolute path in `data/runtime/…json`), §5.2 (unignored runtime artifact and future-tense `data/refresh_queue.json`), §7 (all recommendations explicitly bounded to `07-cleanup-application.md` / Stage B; no file cleanup applied here).
- **Secret discipline:** no real SendGrid API keys, SMTP passwords, OAuth tokens, or PII are reproduced. The `configs/mail_sender.user.json` placeholder value is described as "placeholder-shaped" and is **not quoted**.
- **Filesystem discipline:** every file read in this task is repository-relative under `Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\task-004\`. Main-checkout-only surfaces (`release/`, `bcsp-20260122.zip`) are referenced only via §6, sourced from `01-inventory.md` §4.8 and the pinned relevant-files list, **not** by filesystem traversal outside this worktree.
- **Evidence-layer authority discipline:** the words "ignored", "not tracked", and "not in the repo" appear in this document only with explicit qualification, never as standalone judgements of correctness. §5.1 and §5.2 are written as the affirmative inverse: each case names the rule that matched (or did not match) *and* names the structural condition independently.

## 9. Hand-Off

This document is the runtime/config-hygiene input for the rest of Stage A:

- `05-stage-b-handoff.md` consumes §3, §4, §7.4: the path-default reconciliation, the `.env.example` situation, and the `README.md` ↔ launcher naming question are Stage B candidates because they touch product code/docs.
- `06-final-baseline.md` consumes §3.1, §3.2, §3.3 (the "what a clean checkout actually needs" map) and §8 (constraint coverage) as evidence that Stage A's runtime/config layer has been characterised.
- `07-cleanup-application.md` is the only Stage A task that may apply repository-altering actions and inherits **all** of §7.1, §7.2, §7.3 as its explicit input list. Per `01-inventory.md` §9 / §8, it must justify each untrack against the five evidence layers — this report supplies the layer-by-layer evidence (matching ignore rule, first-add commit, runtime shape, secret risk class) needed to do so without re-deriving.

## 10. Acceptance Criteria Coverage Map

| AC | Requirement | Where addressed |
| --- | --- | --- |
| AC-001 | Produce only `.orchestrator/stage-a/04-runtime-config-hygiene.md`; do not modify product source, tests, runtime data, configs, package files, or `.gitignore`. | §1 (write scope statement); §8 (constraint compliance check); this is the only file written by the task. |
| AC-002 | Consume `.orchestrator/stage-a/01-inventory.md` and preserve its rule that `.gitignore`, tracked, ignored, untracked, and remote states are evidence layers, not authorities. | §1 (method cites the inventory); §2 (evidence-layer table replicates the inventory's authority disclaimer); §5 (tracked-despite-ignore and unignored-despite-runtime split structurally enforces the rule); §7.5 (explicitly refuses to edit `.gitignore` or rewrite history on the basis of evidence alone). |
| AC-003 | Document ignored local data/config/release/runtime surfaces without revealing secret values; state which are tracked despite ignore rules or unignored despite runtime shape. | §3.1 (table column "matching `.gitignore` rule"); §5.1 (tracked-despite-ignored, six rows); §5.2 (unignored-despite-runtime-shape, two rows); §6 (secret-risk surfaces, all values withheld). |
| AC-004 | Check whether required DB config / template / startup prerequisites are present, generated, or missing for a clean checkout. | §3 (bootstrap map per entry point); §3.1 (per-artifact: producer, ignore coverage, current state); §3.2 (tracked templates present); §3.3 (external prerequisites); §4 (path-default divergence — concrete clean-checkout failure modes). |
| AC-005 | Identify stale absolute paths, runtime residue, and bootstrap blockers; recommend `.gitignore`, packaging, or untrack changes as recommendations only. | §4 (path-default divergence — direct runtime residue evidence); §5.1 (stale absolute path in `data/runtime/…json`); §5.2 (future-tense `data/refresh_queue.json`); §7 (all recommendations explicitly bounded, no on-disk cleanup performed); §7.5 (refusals — what this task does *not* do). |
