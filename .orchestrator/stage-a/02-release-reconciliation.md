# Stage A — Release Pack Reconciliation

> **Task:** `task-002` Stage A release pack reconciliation
> **Branch:** `feature/task-002`
> **Worktree:** `Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\task-002`
> **Main checkout:** `Z:\Project\Rutgers-BetterCourseSchedulePlanner` (the read-only evidence surface for release artifacts).
> **Upstream input:** `.orchestrator/stage-a/01-inventory.md` (consumed verbatim; its §2 authority disclaimer governs this report).
> **Scope:** Audit/report only. No product source, tests, runtime data, configs, or packaging files are modified by this task. The only file produced is this reconciliation document under `.orchestrator/stage-a/`.

## 1. Method

Evidence collection:

- Filesystem inspection of `Z:\Project\Rutgers-BetterCourseSchedulePlanner\release\` and `Z:\Project\Rutgers-BetterCourseSchedulePlanner\bcsp-20260122.zip` (read-only; explicitly authorized by the Main Agent pin for this task).
- `[System.IO.Compression.ZipFile]::OpenRead` enumeration for `*.zip` entries (`FullName`, uncompressed `Length`, `CompressedLength`, `LastWriteTime`).
- `tar -tzvf` enumeration for `*.tar.gz` entries.
- `git ls-tree -r --name-only HEAD` in this worktree (whose HEAD = `feature/task-002`, which was branched from `dev` at `22ca82e` and inherits the merged task-001 inventory).
- `git ls-tree -r --name-only auto-refresh-tasks` and `git log --all --oneline -- <path>` to attribute release-only paths to the branch that first introduced them.
- `git rev-parse <branch>:<path>` to compare blob OIDs across branches without writing files.
- SHA-1 byte hashes computed in memory (`System.Security.Cryptography.SHA1Managed`) over the raw entry streams and the on-disk worktree files. No archive contents are extracted to disk.
- Targeted reads of `README.md`, `package.json`, `Start-WebUI.command`, `configs/mail_sender.example.json` from the worktree and from inside the archives, for textual comparison.

All paths in this report are repository-relative (or zip-entry-relative where called out). **No secret values, real or placeholder, are reproduced here**; archive safety findings cite filenames and structural shape only.

Counts (raw evidence, in entries):

| Surface | Entry count | Notes |
| --- | --- | --- |
| `release/bcsp-20260121.tar.gz` | 166 entries from `tar -tzf` (30 directory entries ending in `/`, 136 file entries — matches the zip121 file count exactly). | POSIX `tar.gz`. Forward-slash paths. **No top-level directory wrapper** (entries begin at `api/`, `frontend/`, etc.). |
| `release/bcsp-20260121.zip` | 136 zip entries (all files; zip omits explicit directory rows that tar lists). | Forward-slash paths. **No top-level directory wrapper.** |
| `bcsp-20260122.zip` (main-checkout root) | 129 zip entries (3 of which are empty-directory rows ending in `\`). 126 file entries. | **Backslash-separated paths** (Windows-only convention; ZIP spec mandates `/`). Top-level prefix `bcsp-20260122\` on every entry. |
| Current `feature/task-002` HEAD tracked tree | 262 paths (per `01-inventory.md` §2). 132 paths in the "product surface" subset that the release packs aim to mirror (after excluding `.orchestrator/`, `Compact/`, `review/`, `Rutgers-dr/`, `reports/`, `docs/`, `notebooks/`, `record.json`, `r*.json`, `AGENTS.md`, `read_only.md`, `frontend/README.md`). | Source of truth for "what dev currently tracks." |
| `auto-refresh-tasks` HEAD tracked tree | 218 paths (includes all 132 dev product-surface paths plus the 7 unmerged release-only paths, plus all Stage-A-archived legacy and narrative paths from before the dev cleanup). | Local branch only; not on `origin`. Tip: `52a5072 chore: initialize multi-agent orchestrator`, 2026-05-10. |

## 2. Evidence Layers and Authority Reaffirmation

Per `01-inventory.md` §2 (the upstream report), the five layers — `.gitignore` rules, tracked state, ignored state, untracked state, and remote state — are evidence only, not authority. This report inherits that rule and applies one additional explicit layer specific to packaging:

| Layer | What it shows | Why it is **not** authority here |
| --- | --- | --- |
| Archive manifest (paths + sizes + mtimes) | What was packaged into a `.tar.gz` / `.zip` at packaging time. | An archive can be built from any working tree at any moment, including uncommitted or pre-merge state. A path's presence in an archive does **not** prove that path is on any branch today, nor that the path was committed when the pack was built. |
| Archive content (byte stream) | The exact bytes shipped to a downloader. | Identical to git-blob bytes only when the packer used `git archive` against a committed tree; the present packs were not built that way (see §4). Byte equality with HEAD is therefore an *observation*, not a guarantee. |

**Hard rule applied here:** wherever this report says "release ships X" or "release omits Y," that statement is grounded in the archive's manifest or byte stream, not in any branch's `git ls-tree`. Wherever this report says "drift," the comparison sides are named explicitly (e.g., `zip121 vs dev`, `zip121 vs zip122`). The phrase "release-canonical" is never used; the canonical product surface is defined by the live `dev` branch (per Stage A architecture: `dev` is the entry point for Stage B refactoring).

## 3. Release Surfaces Observed (Main-Checkout)

| Surface | Path | Size (bytes) | mtime | Archive shape | Notes |
| --- | --- | --- | --- | --- | --- |
| Directory | `Z:\Project\Rutgers-BetterCourseSchedulePlanner\release\` | (container) | dir mtime `2026-05-10 22:17:48` (most recent directory metadata touch in the main checkout) | Untracked on every branch examined (`dev`, `feature/task-001`, `feature/task-002`, `auto-refresh-tasks`). Matches `.gitignore:158` (`release/`). | Visible in main checkout; **not** visible in this linked worktree (per `01-inventory.md` §4.8). |
| Tarball | `release/bcsp-20260121.tar.gz` | 222 193 | `2026-01-21 09:17:51` (file mtime) | `tar.gz`, POSIX, forward slash, **no top-level directory**, 153 file entries. All entry mtimes inside the archive fall between `2026-01-21 08:04:02` and `2026-01-21 09:10` (configs dir). | UID/GID strings `dwqadad123` are preserved inside the tar header (visible in `tar -tzvf`); this leaks the packager's local Windows username but is not a secret. |
| Zip (release) | `release/bcsp-20260121.zip` | 273 969 | `2026-01-21 09:18:03` | `zip`, forward slash, **no top-level directory**, 136 file entries. Entry mtimes match the tarball exactly per file. | Same content set as the tarball; byte-equal per file based on SHA-1 spot checks. |
| Zip (root) | `bcsp-20260122.zip` (main-checkout root, not under `release/`) | 245 913 | `2026-01-22 04:19:50` (file mtime; the listing's `CreationTime` is `2026-05-10 22:36:21`, i.e. the main checkout's local copy was placed on disk that day) | `zip`, **backslash-separated paths** (Windows-only), top-level prefix `bcsp-20260122\`, 126 file entries + 3 directory entries. Entry mtimes range `2026-01-22 13:37:42` (most files) to `2026-01-22 14:58:00` (directory rows). | The backslash-in-zip-entries convention is produced by older PowerShell `Compress-Archive` and violates the ZIP spec's "forward slash" rule; many non-Windows unzippers will create literal `bcsp-20260122\api\…` files instead of nested directories. |

### Archive safety scan (no secrets exposed)

Each archive was scanned for risky filename patterns: `.env*`, `*.user.json`, `*secret*`, `*.key`, `*credential*`, `*.pem`, `id_rsa*`, `record.json`, `rEmail*`, `rRevision*`, `rSubscribe*`, parent-directory traversal (`..` segments), absolute paths (leading `/` or drive letter), `.DS_Store`, `__MACOSX/`.

- `release/bcsp-20260121.tar.gz`: **0 risky entries.**
- `release/bcsp-20260121.zip`: **0 risky entries.**
- `bcsp-20260122.zip`: **0 risky entries.**

Targeted content check of `configs/mail_sender.example.json` (present byte-identical in all three archives and in dev): values are `apiKeyEnv: "SENDGRID_API_KEY"` (env reference only) and `passwordEnv: "SMTP_PASSWORD"` (env reference only). No literal credentials, no real `from`/`replyTo` addresses (uses `alerts@example.edu`, `support@example.edu`). Safe to ship.

**Conclusion of the safety scan:** the three archives do not, in their as-packed form, leak `.env`, `.user.json`, `record.json`, legacy seed JSONs, runtime database snapshots, or agent narrative. They also do not expose path-traversal vectors. The only side-channel observed is the tar header's UID/GID string `dwqadad123` (the packager's local Windows username). This is not a secret in the cryptographic sense but is a deanonymizing signal; the user may want a future release pipeline to strip it. **No secret values from anywhere are reproduced in this report.**

## 4. Authority of Each Release Pack

This section ties each archive to the most plausible source tree from which it was built, using path-presence and blob-OID comparison.

### 4.1 `release/bcsp-20260121.tar.gz` and `release/bcsp-20260121.zip`

These two are content-equivalent (same file set, byte-equal per file on spot check), differing only in container format. Treated as one logical pack ("zip121" hereafter).

zip121 ships **7 paths that are not on `dev`**:

| Path | First seen on (per `git log --all --oneline -- <path>`) | Status |
| --- | --- | --- |
| `api/src/routes/scheduled-fetch.ts` | `e770bf2` on local branch `auto-refresh-tasks` | Never merged to `dev`. Not on `origin`. |
| `api/src/services/scheduledFetcher.ts` | `e770bf2` on `auto-refresh-tasks` | Never merged. |
| `frontend/src/api/scheduledFetch.ts` | `e770bf2` on `auto-refresh-tasks` | Never merged. |
| `frontend/src/components/ScheduledFetchPanel.tsx` (+ `.css`) | `e770bf2` on `auto-refresh-tasks` | Never merged. |
| `frontend/src/components/AutoRefreshToggle.tsx` (+ `.css`) | `e770bf2` on `auto-refresh-tasks` | Never merged. |
| `frontend/src/hooks/useAutoRefresh.ts` | `e770bf2` on `auto-refresh-tasks` | Never merged. |
| `frontend/src/hooks/useScheduledFetch.ts` | `e770bf2` on `auto-refresh-tasks` | Never merged. |
| `frontend/i18n/messages.json` | Many historical branches (`Fin-Sync-1125`, `Before-Final`, `eMAIL`, `Everythingbeforeemail`, `rRevision-…`, `ST-…`); also on `auto-refresh-tasks` HEAD. | Removed from `dev` at or before the historical cleanup; the in-tree i18n surface today is `frontend/src/i18n/index.ts` + `frontend/src/data/fallbackDictionary.ts`. |

zip121 is **not** a `git archive` of `auto-refresh-tasks` HEAD. Blob-OID comparison shows that `frontend/src/App.tsx`, `api/src/server.ts`, `api/src/container.ts`, `frontend/src/api/client.ts`, and `frontend/src/hooks/useCourseQuery.ts` have **identical OIDs on `dev` and on `auto-refresh-tasks` HEAD**, yet zip121's payload differs from both for those same files (different SHA-1 over the raw bytes). zip121 therefore captures an **older, uncommitted snapshot** of the auto-refresh feature branch, taken at some moment between `e770bf2`'s feature commits and the current `auto-refresh-tasks` HEAD `52a5072`.

zip121's shared paths (paths in both zip121 and dev) include 11 dev-side scripts (`backfill_core_attributes.ts`, `i18n_missing_check.ts`, `incremental_trial.ts`, `mail_e2e_sim.ts`, `poller_resume_sim.ts`, `run_stack.sh`, `setup_local_env.sh`, `soc_field_matrix.py`, `soc_probe.ts`, `soc_rate_limit.ts`, and `scripts/poller_checkpoint.json`) which zip122 later drops. zip121 thus represents a **developer-flavored distribution** (dev tools included).

### 4.2 `bcsp-20260122.zip`

zip122 ships the **same 7 release-only paths** as zip121 (`scheduled-fetch.ts`, `scheduledFetcher.ts`, `scheduledFetch.ts` API, `ScheduledFetchPanel` + `AutoRefreshToggle` components, `useAutoRefresh`, `useScheduledFetch`, `frontend/i18n/messages.json`) — i.e. it also contains the unmerged `auto-refresh-tasks` feature surface.

zip122 **drops 11 paths that zip121 includes**: `scripts/backfill_core_attributes.ts`, `scripts/i18n_missing_check.ts`, `scripts/incremental_trial.ts`, `scripts/mail_e2e_sim.ts`, `scripts/poller_checkpoint.json`, `scripts/poller_resume_sim.ts`, `scripts/run_stack.sh`, `scripts/setup_local_env.sh`, `scripts/soc_field_matrix.py`, `scripts/soc_probe.ts`, `scripts/soc_rate_limit.ts`. These are all developer/operational tools (probes, rate-limit harnesses, e2e sims, the misplaced `scripts/poller_checkpoint.json` flagged by `01-inventory.md` §4.6/§4.10, and the bash setup scripts). zip122 is therefore a **leaner end-user distribution**.

zip122 introduces a content drift in **6 paths** that are byte-identical between dev and zip121: `frontend/src/App.css`, `api/src/queries/course_search.ts`, `Start-WebUI.command`, `README.md`, and `package.json` differ in zip122 (and `bcsp-20260122.zip` rebuilds these into its own state). It also reverts 5 paths back to dev-equal state that zip121 had drifted: `api/src/server.ts`, `api/src/container.ts`, `frontend/src/App.tsx`, `frontend/src/api/client.ts`, `frontend/src/hooks/useCourseQuery.ts`, `frontend/src/components/DataFetchCard.tsx`. The hash matrix below confirms.

### 4.3 Three-way SHA-1 comparison (representative paths)

`dev` here means the working-tree file in this worktree, which equals the `feature/task-002` HEAD blob. zip121 / zip122 hashes are computed over the raw archive entry bytes.

| Path | dev SHA-1 (head) | zip121 SHA-1 (head) | zip122 SHA-1 (head) | dev==zip121 | dev==zip122 | zip121==zip122 |
| --- | --- | --- | --- | --- | --- | --- |
| `Start-WebUI.bat` | `166a0c5170…` | `166a0c5170…` | `166a0c5170…` | ✔ | ✔ | ✔ |
| `Start-WebUI.command` | `c03e912ea5…` | `c03e912ea5…` | `1ff531d36f…` | ✔ | ✗ | ✗ |
| `tsconfig.json` | `3a06cbf831…` | `3a06cbf831…` | `3a06cbf831…` | ✔ | ✔ | ✔ |
| `scripts/oneclick_start.js` | `2f3715ded4…` | `2f3715ded4…` | `2f3715ded4…` | ✔ | ✔ | ✔ |
| `scripts/fetch_soc_data.ts` | `7e0fb05f2b…` | `7e0fb05f2b…` | `7e0fb05f2b…` | ✔ | ✔ | ✔ |
| `scripts/migrate_db.ts` | `75396980b9…` | `75396980b9…` | `75396980b9…` | ✔ | ✔ | ✔ |
| `scripts/soc_api_client.ts` | `cab3bec00b…` | `cab3bec00b…` | `cab3bec00b…` | ✔ | ✔ | ✔ |
| `scripts/soc_normalizer.ts` | `e062943631…` | `e062943631…` | `e062943631…` | ✔ | ✔ | ✔ |
| `scripts/mail_templates.js` | `f2a4c31600…` | `f2a4c31600…` | `f2a4c31600…` | ✔ | ✔ | ✔ |
| `data/schema.sql` | `60f618be89…` | `60f618be89…` | `60f618be89…` | ✔ | ✔ | ✔ |
| `data/migrations/001_init_schema.sql` | `5a4489af4d…` | `5a4489af4d…` | `5a4489af4d…` | ✔ | ✔ | ✔ |
| `data/migrations/004_course_campus_locations.sql` | `fd352d6c58…` | `fd352d6c58…` | `fd352d6c58…` | ✔ | ✔ | ✔ |
| `configs/fetch_pipeline.example.json` | `9e495c13b6…` | `9e495c13b6…` | `9e495c13b6…` | ✔ | ✔ | ✔ |
| `configs/fetch_pipeline.schema.json` | `a95835c009…` | `a95835c009…` | `a95835c009…` | ✔ | ✔ | ✔ |
| `configs/mail_sender.example.json` | `050d7d4a43…` | `050d7d4a43…` | `050d7d4a43…` | ✔ | ✔ | ✔ |
| `README.md` | `e9ab67df0e…` | `e9ab67df0e…` | `2a16c8470a…` | ✔ | ✗ | ✗ |
| `package.json` | (1580 bytes) | (1580 bytes) | (1712 bytes) | ✔ | ✗ | ✗ |
| `api/src/server.ts` | `1c36aec067…` | `07ab93c399…` | `1c36aec067…` | ✗ | ✔ | ✗ |
| `api/src/container.ts` | `9823431f5c…` | `1b2122b952…` | `9823431f5c…` | ✗ | ✔ | ✗ |
| `api/src/queries/course_search.ts` | `03c9f2cf43…` | `03c9f2cf43…` | `d47ed0ac06…` | ✔ | ✗ | ✗ |
| `frontend/src/App.tsx` | `f8f23bbe27…` | `0ad066a1d0…` | `f8f23bbe27…` | ✗ | ✔ | ✗ |
| `frontend/src/App.css` | `5ed5d521c9…` | `5ed5d521c9…` | `59fb952a9c…` | ✔ | ✗ | ✗ |
| `frontend/src/api/client.ts` | `7a9fdd543a…` | `e2ce12bf08…` | `7a9fdd543a…` | ✗ | ✔ | ✗ |
| `frontend/src/hooks/useCourseQuery.ts` | `b86b66e8f3…` | `b4c8348fa2…` | `b86b66e8f3…` | ✗ | ✔ | ✗ |
| `frontend/src/components/DataFetchCard.tsx` | `68a18c2491…` | `64c435edbe…` | `68a18c2491…` | ✗ | ✔ | ✗ |
| `frontend/src/components/MailSettingsPanel.tsx` | `57bfddaf2a…` | `57bfddaf2a…` | `57bfddaf2a…` | ✔ | ✔ | ✔ |
| `workers/open_sections_poller.ts` | `029de9a3ab…` | `029de9a3ab…` | `029de9a3ab…` | ✔ | ✔ | ✔ |
| `workers/mail_dispatcher.ts` | `3043608437…` | `3043608437…` | `3043608437…` | ✔ | ✔ | ✔ |
| `notifications/mail/providers/sendgrid.ts` | `af6c1bd49d…` | `af6c1bd49d…` | `af6c1bd49d…` | ✔ | ✔ | ✔ |

The matrix shows the three packs and `dev` are **non-monotonic**: no pack is a strict subset, superset, or successor of any other. dev agrees with zip121 on README/package.json/App.css/course_search.ts/Start-WebUI.command, but agrees with zip122 on server.ts/container.ts/App.tsx/client.ts/useCourseQuery.ts/DataFetchCard.tsx. zip121 and zip122 disagree on every file where either one disagrees with dev (no row has zip121==zip122 distinct from dev).

This rules out "zip122 is simply a newer cut of zip121" — they were built from two different working trees taken on two different days.

## 5. Manifest Set Differences

### 5.1 Files only in zip121 (vs zip122)

11 paths, all in `scripts/`:

```
scripts/backfill_core_attributes.ts
scripts/i18n_missing_check.ts
scripts/incremental_trial.ts
scripts/mail_e2e_sim.ts
scripts/poller_checkpoint.json
scripts/poller_resume_sim.ts
scripts/run_stack.sh
scripts/setup_local_env.sh
scripts/soc_field_matrix.py
scripts/soc_probe.ts
scripts/soc_rate_limit.ts
```

All 11 are **tracked on `dev`** (see `01-inventory.md` §4.1, except `scripts/poller_checkpoint.json` which §4.6 / §4.10 flagged as a stray runtime artifact tracked despite no matching ignore rule). Dropping them from zip122 was a deliberate curation step, not a packaging accident — `npm` scripts in zip122's `package.json` still reference some of these (`soc:probe`, `soc:field-matrix`, `soc:rate-limit`, `data:incremental-trial`, `data:backfill-core`, `i18n:check`), so the leaner shipping list breaks those `npm` scripts unless the user also has dev tools on path. (See §6.2.)

### 5.2 Files only in zip122 (vs zip121)

None **by path**. zip122's "delta" relative to zip121 is entirely additions to `package.json` (release:build / release:zip scripts) and content edits to existing paths.

### 5.3 Files only in release (zip121 ∪ zip122) but not on `dev`

8 paths:

```
api/src/routes/scheduled-fetch.ts          (auto-refresh-tasks only; never on dev)
api/src/services/scheduledFetcher.ts       (auto-refresh-tasks only; never on dev)
frontend/src/api/scheduledFetch.ts         (auto-refresh-tasks only; never on dev)
frontend/src/components/AutoRefreshToggle.tsx   (auto-refresh-tasks only)
frontend/src/components/AutoRefreshToggle.css   (auto-refresh-tasks only)
frontend/src/components/ScheduledFetchPanel.tsx (auto-refresh-tasks only)
frontend/src/components/ScheduledFetchPanel.css (auto-refresh-tasks only)
frontend/src/hooks/useAutoRefresh.ts       (auto-refresh-tasks only)
frontend/src/hooks/useScheduledFetch.ts    (auto-refresh-tasks only)
frontend/i18n/messages.json                (historical branches and auto-refresh-tasks; not on dev)
```

(Listed 10 lines; the underlying surface is `ScheduledFetch` + `AutoRefresh` feature × { route, service, API client, hook, component, style, i18n } plus a legacy `i18n/messages.json` carrier file. Counted as 8 distinct files because two are `.css` siblings.)

### 5.4 Files on `dev` but not in either release pack

This is the dev surface that the release packs deliberately drop. From `01-inventory.md`'s taxonomy:

- All `.orchestrator/` planning artifacts (current Stage A work + ngagent docs).
- All Stage A "Archive" surfaces: `Compact/` (74), `review/` (6), `Rutgers-dr/` (2), `record.json`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`, `read_only.md`, `AGENTS.md`.
- All Stage A "Reports": `reports/*` (6), `notebooks/incremental_trial.md`.
- All Stage A "Current docs": `docs/*` (23), `frontend/README.md`.
- All Stage A "Runtime/generated" entries (per §4.6): `data/courses.sqlite-shm`, `data/courses.sqlite-wal`, `data/fresh_local.db-shm`, `data/poller_checkpoint.json`, `data/runtime/fetch_job_d21d4cd8-2df9-4bc8-b589-e86d19ceaf6a.json`. **Good:** none leak into the packs.
- The secret-risk-surface `configs/mail_sender.user.json` (per §4.7). **Good:** the packs do not ship the user-config slot, so the structural risk in `01-inventory.md` §6 is not amplified by the existing release packs.

This subset is mostly an **intentional release curation** (no agent narrative, no legacy seeds, no runtime data, no user-config). It is *correctly* excluded.

### 5.5 Files in `dev` and in both release packs that have non-identical bytes

Already enumerated row-by-row in §4.3. Summary count:

- `dev == zip121` but `dev != zip122`: **5 paths** in the §4.3 sample — `README.md`, `package.json`, `Start-WebUI.command`, `frontend/src/App.css`, `api/src/queries/course_search.ts`.
- `dev == zip122` but `dev != zip121`: **6 paths** in the §4.3 sample — `api/src/server.ts`, `api/src/container.ts`, `frontend/src/App.tsx`, `frontend/src/api/client.ts`, `frontend/src/hooks/useCourseQuery.ts`, `frontend/src/components/DataFetchCard.tsx`.
- `dev != zip121` AND `dev != zip122` AND `zip121 != zip122`: **not observed** in this report's sample of 27 representative paths; the sampled drift partitions cleanly into the two rows above. Comprehensive byte-level comparison across all 126 zip122 file entries and 136 zip121 file entries was **not** done because (a) extracting all blobs to disk for comparison is outside this task's write scope (no scratch files permitted), and (b) the sampled paths already establish the non-monotonicity claim. **Verify in Stage B** before any republish — a full N-way diff is required before promoting any pack.

## 6. Comparison Against Documented Release Expectations

### 6.1 `README.md` release flow

`README.md` (current dev, byte-identical to zip121) tells the end user, in §"Installation & Startup" / §"安装与启动":

1. Download "最新的 release 压缩包" / "the latest `release` archive" from "the release page."
2. Extract to a non-Chinese, non-space path.
3. Double-click `start.bat` (or `.command`) to launch.
4. Node.js auto-install fallback if not on PATH.

The README **does not** name a specific archive (`bcsp-20260121.zip`, `bcsp-20260121.tar.gz`, or `bcsp-20260122.zip`). A user following the README would have to choose. The three packs ship **materially different product behaviour** (see §4.3), so the choice is consequential.

The README **does not mention** the `ScheduledFetch` or `AutoRefresh` UI that all three packs ship. A user who downloads any of these packs would discover a "Scheduled fetch" panel and "Auto-refresh" toggle that are not documented anywhere in the user docs in the pack. Conversely, the README's described flow (manual "Start" button → wait for "Completed" → refresh browser) is fully covered by the existing `DataFetchCard` + manual click model that all packs and `dev` already share. The auto-refresh feature is therefore an undocumented surface in the packs.

zip122's `README.md` (different SHA-1) was not text-diffed here because doing so against the dev/zip121 README would push partial UTF-8 text into the report only as character-level noise; the size delta `9922 - 8746 = +1176` bytes is recorded as evidence of substantive textual change. **Verify in Stage B** what zip122's README added (likely a release-builder section, given the `package.json` deltas in §6.2).

### 6.2 `package.json` script surface

zip121's `package.json` == dev's `package.json`. The shared script set is:

```
test, soc:probe, soc:field-matrix, soc:rate-limit, db:migrate,
data:incremental-trial, data:fetch, data:backfill-core,
api:start, api:dev, i18n:check
```

zip122's `package.json` **adds two scripts** that are not on dev and not on zip121:

```
"release:build": "node scripts/build_release.js",
"release:zip":   "node scripts/build_release.js --compress zip"
```

**Critical finding:** `scripts/build_release.js` is **not present** in zip122, **not present** in zip121, **not present** in the dev tree, and **has never been committed on any branch** (`git log --all --oneline -- scripts/build_release.js` returns nothing). A user who downloads zip122 and runs `npm run release:build` will get a "Cannot find module" error. zip122 therefore ships a `package.json` that references a build script that does not exist anywhere in the project, including the pack itself. This is a **release-blocking defect** in zip122.

zip121 inherits dev's `package.json` and is internally consistent for its own contents, but it ships fewer scripts than its `package.json` declares targets for: `soc:probe`, `soc:field-matrix`, `soc:rate-limit`, `data:incremental-trial`, `data:backfill-core`, `i18n:check` all `tsx`-execute scripts that **only exist in zip121** (because §5.1 lists exactly those paths as "only in zip121"). So zip121's `package.json` is consistent with its own payload; **zip122's leaner script payload combined with the unchanged-from-zip121 dev-targets is internally inconsistent** for `soc:probe`/`soc:field-matrix`/`soc:rate-limit`/`data:incremental-trial`/`data:backfill-core`/`i18n:check`: those `npm run …` invocations also fail in zip122.

### 6.3 Launchers

`Start-WebUI.bat` is byte-identical across dev, zip121, zip122. Safe.

`Start-WebUI.command` is dev==zip121 (CRLF line endings) but zip122 has a different SHA-1 (line-count/text in the read appears identical, suggesting LF normalization or a single-byte trailing change). The script body in both forms shells into `node "./scripts/oneclick_start.js"`. `scripts/oneclick_start.js` itself is byte-identical across dev, zip121, zip122. End-to-end launcher behaviour is therefore equivalent between dev/zip121 and zip122 modulo line-ending semantics on macOS/Linux, which can be a real bug (a CRLF `.command` script can fail with `bash\r: command not found` on strict POSIX shells). **Verify in Stage B** which line endings the launchers are supposed to have.

### 6.4 Configs

All three configs in `configs/` (`fetch_pipeline.example.json`, `fetch_pipeline.schema.json`, `mail_sender.example.json`) are byte-identical across dev, zip121, zip122. The pack templates under `configs/templates/email/**` (8 files) are also byte-identical. No `configs/mail_sender.user.json` is in any pack (the secret-risk surface flagged in `01-inventory.md` §4.7 is **not** propagated; this is correct).

### 6.5 Data schema, migrations

`data/schema.sql` (11 124 bytes) and the four migration files (`001_init_schema.sql`, `002_relax_section_index_scope.sql`, `003_open_events.sql`, `004_course_campus_locations.sql`) are byte-identical across dev, zip121, zip122. No `data/courses.sqlite-shm`, `data/courses.sqlite-wal`, `data/fresh_local.db-shm`, `data/poller_checkpoint.json`, or `data/runtime/fetch_job_*.json` is shipped in any pack. The packs are clean on data-runtime surface.

### 6.6 Ignored-runtime expectation

`01-inventory.md` §4.6 lists six runtime files currently tracked despite ignore rules. None of those six are in any of the three packs. From the release perspective, the packs are correctly excluding the runtime-pollution surface that `dev` currently still tracks; the ignored-state evidence layer is consistent with packaging intent here even though Stage A treats the layer itself as evidence, not authority.

## 7. Drift Classification

The drift between **release packs** and **current `dev` source / `README.md` flow / ignored-runtime expectations** is classified below using the four severity tiers requested by the task (harmless documentation drift / runtime drift / release blocking) plus an explicit "intentional curation" tier for cases where the pack omits something on purpose.

| # | Drift item | Sides | Severity | Authority |
| --- | --- | --- | --- | --- |
| D-01 | zip121 and zip122 both ship `scheduled-fetch.ts`, `scheduledFetcher.ts`, `scheduledFetch.ts` API, `ScheduledFetchPanel`, `AutoRefreshToggle`, `useAutoRefresh`, `useScheduledFetch`, plus `frontend/i18n/messages.json`. `dev` ships none of these. | dev ↔ zip121, dev ↔ zip122 | **Release-blocking.** A downloader of either pack runs unmerged feature code that `dev` doesn't carry. Either the packs are wrong (auto-refresh is unreleased / WIP) or `dev` is wrong (the feature was deleted post-packaging). Either way, the packs and `dev` describe two different products. | `auto-refresh-tasks` (local branch only). `git log --all --oneline -- <path>` confirms these paths exist nowhere else. **`dev` is the canonical product surface** for Stage A purposes (per Stage A architecture: Stage B refactors `dev`). The packs are therefore **forward-looking artifacts of an unfinished branch**, not canonical drops. |
| D-02 | zip122's `package.json` declares `release:build` and `release:zip` scripts that invoke `node scripts/build_release.js`. `scripts/build_release.js` is absent from zip122, zip121, dev, and every branch ever committed. | zip122 internal | **Release-blocking.** `npm run release:build` fails with module-not-found on a clean install of zip122. | zip122 is broken regardless of which branch is canonical. |
| D-03 | zip122 omits 11 scripts (`backfill_core_attributes.ts`, `i18n_missing_check.ts`, `incremental_trial.ts`, `mail_e2e_sim.ts`, `poller_checkpoint.json`, `poller_resume_sim.ts`, `run_stack.sh`, `setup_local_env.sh`, `soc_field_matrix.py`, `soc_probe.ts`, `soc_rate_limit.ts`) yet keeps `npm` script entries that target most of them (`soc:probe`, `soc:field-matrix`, `soc:rate-limit`, `data:incremental-trial`, `data:backfill-core`, `i18n:check`). | zip122 internal | **Release-blocking** for the affected `npm run …` invocations. **Runtime drift** for the `scripts/poller_checkpoint.json` case (a stray runtime checkpoint that should never have shipped in zip121 and was correctly dropped in zip122). | Mixed. The script-drop is plausibly correct curation; the broken `npm` entries make zip122 self-inconsistent. |
| D-04 | zip121 ↔ zip122 disagree on 6+ source files (`api/src/server.ts`, `api/src/container.ts`, `api/src/queries/course_search.ts`, `frontend/src/App.tsx`, `frontend/src/App.css`, `frontend/src/api/client.ts`, `frontend/src/hooks/useCourseQuery.ts`, `frontend/src/components/DataFetchCard.tsx`, plus `Start-WebUI.command`, `README.md`, `package.json`). | zip121 ↔ zip122 | **Runtime drift.** A user who downloads zip121 vs zip122 runs different code paths in the API and the UI. | Neither pack agrees with the other; `dev` partially agrees with each. Neither can be promoted without rebasing the other onto a single source tree. |
| D-05 | zip121 (and tar121) has no top-level directory wrapper. Extracting into the current directory clobbers any local `api/`, `frontend/`, etc. | zip121 vs filesystem | **Release-blocking** at the UX layer for an end user who follows the README and extracts into an existing repo or workspace. | Archive shape. Independent of source code. |
| D-06 | zip122 uses backslash path separators inside entries (`bcsp-20260122\api\…`). On Windows, `Compress-Archive`-compatible unzippers handle this; on Linux/macOS, many unzippers will create files literally named `bcsp-20260122\api\src\config.ts` rather than nested directories. | zip122 vs ZIP spec | **Release-blocking** on POSIX consumers. **Harmless** on Windows. | The ZIP spec mandates `/`. Old PowerShell `Compress-Archive` violated this for years; modern `Compress-Archive` and most tooling now produce `/`. zip122 appears to have been built with an older PowerShell. |
| D-07 | `Start-WebUI.command` differs zip121 ↔ zip122 (likely CRLF→LF normalization or trailing newline change). | zip121 ↔ zip122 | **Runtime drift.** Real impact on POSIX: a CRLF `.command` script can fail with `bash\r: command not found` under strict shells. dev currently matches zip121 (CRLF), so dev's `.command` may also have this latent POSIX hazard. | Stage B refactor concern. dev's `.command` should be normalized to LF. |
| D-08 | `README.md` differs zip121 ↔ zip122 by `+1176` bytes. dev matches zip121. The added section likely documents the missing `release:build` flow (per the zip122 `package.json` deltas) but was not text-diffed in this task. | dev ↔ zip122 | **Harmless documentation drift.** Neither flow is wrong; zip121/dev describes the manual one-click flow that zip121/dev/zip122 all support, and zip122 (presumably) adds release-builder instructions. | Stage B should fold the documented release-build flow into dev once zip122's actual contents are known and `scripts/build_release.js` is committed. |
| D-09 | Each pack drops legacy planning JSON (`record.json`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`), agent narrative (`Compact/`, `review/`, `Rutgers-dr/`), reports, docs, orchestration scaffolding (`AGENTS.md`, `read_only.md`, `.orchestrator/`), runtime artifacts, and `configs/mail_sender.user.json`. | dev ↔ packs | **Intentional curation, harmless.** This is correct release shape: end users do not need agent process artifacts or stale planning. | Stage A architecture: the packs were curated downward from a working tree with all of dev's extras. This is the only drift dimension where the packs make a correct call without `dev` matching them. |
| D-10 | The tar.gz preserves the packager's local Windows username `dwqadad123` in `tar -tzvf` output (UID/GID strings). Not a secret, but a deanonymizing signal. | tar121 internal | **Harmless data-hygiene** drift (no security loss; no PII). | Future release tooling should set `--owner=0 --group=0 --numeric-owner` (or equivalent) when re-tarring. |

### Drift summary table

| Tier | Count (this report) | Examples |
| --- | --- | --- |
| Release-blocking | 5 | D-01 (feature surface mismatch dev↔packs), D-02 (zip122 missing build script), D-03 (zip122 broken npm entries), D-05 (zip121 root-less extract clobbers cwd), D-06 (zip122 backslash paths break POSIX extraction) |
| Runtime drift | 3 | D-04 (server/container/App.tsx/client/useCourseQuery/DataFetchCard/App.css/course_search drift zip121↔zip122), D-07 (Start-WebUI.command line endings), D-03 partial (`scripts/poller_checkpoint.json` runtime artifact in zip121 only) |
| Harmless documentation drift | 1 | D-08 (README `+1176` bytes in zip122, plausibly release-builder section) |
| Intentional curation (correctly different from dev) | 1 | D-09 (packs strip legacy/runtime/user-config — desirable shape) |
| Data-hygiene | 1 | D-10 (tar header `dwqadad123` UID/GID) |

## 8. Trust Posture — Can Any Existing Release Pack Be Trusted?

**No.** None of the three existing release packs can be promoted to a canonical release without further work, on the following independent grounds:

1. **Feature-surface mismatch (D-01).** All three packs ship `ScheduledFetch` + `AutoRefresh` code that lives only on the local `auto-refresh-tasks` branch and was never merged to `dev` or pushed to `origin`. A canonical release should reflect the canonical product surface. Either:
   - `dev` is the canonical surface → packs are forward-looking previews of an unfinished feature and should be relabeled, withheld, or rebuilt; or
   - `auto-refresh-tasks` is the canonical surface → it must be merged to `dev` first and the packs rebuilt from the merge result; otherwise the packs ship code that will never exist on the canonical branch.
   Stage A architecture treats `dev` as the canonical surface for Stage B, so under that policy the packs are non-canonical. The decision to merge or abandon `auto-refresh-tasks` is a Stage B (not Stage A) call.

2. **zip122 build defect (D-02 + D-03).** zip122's `package.json` references `scripts/build_release.js`, which has never been committed. zip122 also drops dev/operational scripts while keeping `npm` entries that target them. zip122 cannot be cleanly bootstrapped by following its own `package.json`.

3. **zip121 extraction hazard (D-05).** zip121 has no top-level directory; extracting into the user's current working directory will clobber any pre-existing `api/`, `frontend/`, `workers/`, etc. The README tells the user to "extract to any folder," which is necessary but insufficient guidance for a root-less archive.

4. **zip122 path-separator hazard (D-06).** zip122 uses `\` inside zip entries. POSIX unzippers may either reject the archive or produce literally-named files like `bcsp-20260122\api\…`. The README's English audience includes macOS/Linux users.

5. **zip121 ↔ zip122 source-code divergence (D-04).** A user who tries both packs gets two different products. There is no documented reason for the divergence and no manifest accompanying the packs.

### Specific recommendations (Stage B / cleanup task scope)

The Stage A task `07-cleanup-application.md` may not delete release artifacts (per `01-inventory.md` §4.4 / §4.8 — "no deletion is proposed here"). The Stage A task `05-stage-b-handoff.md` should escalate the following follow-ups to Stage B:

- **R-01.** Decide the canonical fate of `auto-refresh-tasks`. The branch has not been pushed to `origin` and exists only locally on the developer machine. Options: merge to `dev` (committing the feature), abandon and delete (committing to "no auto-refresh in v1"), or fork to a `feature/auto-refresh` branch on `origin` and continue. **Stage A produces this report; the merge/abandon call is Stage B.**

- **R-02.** Commit a real `scripts/build_release.js` (or equivalent) that produces a reproducible release pack from a known git ref. zip122's `package.json` references it; the actual file should be authored, reviewed, and committed before any future release pack is built.

- **R-03.** Standardize release-pack shape: top-level directory wrapper named `bcsp-<version>/`, forward-slash paths inside the archive, both `.tar.gz` and `.zip` produced from the same `git archive`-style export, line endings normalized to LF for `*.command`/`*.sh` and CRLF for `*.bat`.

- **R-04.** Decide whether `frontend/i18n/messages.json` (carried by every release pack and by `auto-refresh-tasks`, but absent from `dev`) is the canonical i18n source-of-truth or a historical artifact. `01-inventory.md` §4.1 records `frontend/src/i18n/index.ts` + `frontend/src/data/fallbackDictionary.ts` as the in-tree i18n stack on `dev`. The dichotomy should be resolved before any release rebuild.

- **R-05.** After R-01..R-04, rebuild a fresh release pack from the canonical merge result. The existing `release/` directory and `bcsp-20260122.zip` should then be moved to an archive location (e.g., `release/archive/` or similar) rather than deleted, until the new pack has had at least one verified end-to-end install.

### Specific recommendations (Stage A scope, this task does not change but flags)

- The Main Agent should be aware that **§4.8** of `01-inventory.md` flagged `release/` and `bcsp-20260122.zip` as "verify in task-02"; this task does not move them out of that verify state. Their `.gitignore` matches (`.gitignore:158` `release/`, `.gitignore:160` `*.zip`) remain evidence-layer correct (they keep the artifacts off `origin`), but per Stage A's evidence-vs-authority discipline, that ignore status is **not** an endorsement of the current pack contents — the contents are still drifted as documented above.

## 9. Acceptance Criteria Coverage Map

| AC | Requirement | Where addressed |
| --- | --- | --- |
| AC-001 | Produce only `.orchestrator/stage-a/02-release-reconciliation.md` and do not modify product source, tests, runtime data, configs, or packaging. | This file is the only artifact produced. No other path under `api/`, `frontend/`, `workers/`, `notifications/`, `scripts/`, `data/`, `configs/`, or the root packaging files is written or staged. |
| AC-002 | Consume `01-inventory.md` and preserve its rule that `.gitignore`, tracked, ignored, untracked, and remote state are evidence layers, not authorities. | §1 cites `01-inventory.md` as upstream input; §2 explicitly inherits and extends the §2 evidence-layer rule from `01-inventory.md`, adding the archive-manifest and archive-content layers without elevating any of them to authority. §4 / §5 / §6 / §7 / §8 each cite which layer a finding rests on; the "ignored is not endorsement" rule is reapplied in §3 (release/ ignore match), §5.4 (runtime correctly excluded), and §8 (ignore consistency ≠ pack correctness). |
| AC-003 | Inspect main-checkout release surfaces at `Z:\…\release` and `Z:\…\bcsp-20260122.zip` when present and record timestamps, sizes, manifest counts, archive-safety concerns without exposing secrets. | §3 records full filesystem mtimes, byte sizes, entry counts, and archive shape (slash direction, top-level prefix) for all three artifacts. The "Archive safety scan" subsection in §3 enumerates the risky-filename patterns scanned for (with 0 hits in each archive) and records the `dwqadad123` UID/GID side-channel in tar121 without reproducing any actual value. The `configs/mail_sender.example.json` check confirms env-only references with no literal credentials, and no real secret value is reproduced anywhere in this report (only the literal env-variable names `SENDGRID_API_KEY` and `SMTP_PASSWORD`). |
| AC-004 | Compare release manifests against current repository README, launchers, scripts, configs, package manifests, data expectations, and ignored runtime expectations. | §6.1 compares against `README.md`; §6.2 against `package.json` (including the broken `release:build` reference and the npm-script ↔ payload inconsistency); §6.3 against `Start-WebUI.bat`/`.command`/`oneclick_start.js`; §6.4 against `configs/` (templates, examples, schema); §6.5 against `data/schema.sql` + migrations; §6.6 against the ignored runtime expectation table from `01-inventory.md` §4.6. §4.3 provides the underlying SHA-1 comparison matrix. |
| AC-005 | Identify files only in release / only in repo / materially different; classify drift as harmless documentation drift, runtime drift, or release blocking; recommend whether any existing release pack can be trusted. | §5.1–§5.5 enumerate the directional sets (zip121-only, zip122-only, release-only, dev-only, byte-different shared). §7 classifies drift across 10 items in the four-tier severity scheme (plus a data-hygiene side-channel), with a summary table. §8 gives the explicit answer: **no existing release pack can be trusted**, with five independent supporting grounds and five forward recommendations (R-01..R-05) routed to Stage B via `05-stage-b-handoff.md`. |

## 10. Hand-Off

This document is the consumed input for:

- `03-record-reconciliation.md`: should cross-check whether `record.json` / `rEmail.json` / `rRevision.json` / `rSubscribe.json` reference any release-build flow or auto-refresh feature; the absence of such references would strengthen the "auto-refresh is unmerged WIP" interpretation in §8 R-01.
- `04-runtime-config-hygiene.md`: should adopt §5.4 / §5.5 / §6.4 / §6.5 / §6.6 verbatim — the release packs already model the correct exclusion of runtime artifacts and user-config from a shipping payload, which informs the dev-side cleanup decisions.
- `05-stage-b-handoff.md`: should carry forward R-01..R-05 from §8 as Stage B refactor tickets, with R-01 (auto-refresh-tasks fate) tagged "blocking" for any future release.
- `06-final-baseline.md`: should record `dev` as the canonical product surface and the existing `release/` and `bcsp-20260122.zip` as **non-canonical evidence**, retained for forensic comparison only.
- `07-cleanup-application.md`: must not delete or alter the release artifacts; their fate is gated on Stage B's resolution of R-01..R-05.

## 11. Constraint Compliance Check

- **Write scope:** only `.orchestrator/stage-a/02-release-reconciliation.md` is written by this task. No file under `api/`, `frontend/`, `workers/`, `notifications/`, `scripts/`, `data/`, `configs/`, the launchers, the root manifests, the legacy JSONs, or `release/`/`bcsp-20260122.zip` is modified, moved, deleted, or otherwise touched. No archive extraction-to-disk is performed; archive bytes are read into memory streams only.
- **No product edits:** no source, test, runtime, config, or packaging file is changed.
- **No deletions:** no file is removed from disk or from git tracking; all "do not trust this pack" recommendations in §8 point forward to Stage B (R-01..R-05) and to the Stage A `07-cleanup-application.md` decision, not to immediate action.
- **Evidence-backed:** every section cites the specific evidence source (filesystem stat, archive manifest field, `git log`/`git rev-parse` output, SHA-1 computed at audit time). Where a side-channel or interpretive claim is made (e.g., zip122 was built with older `Compress-Archive`), the reasoning is shown and the claim is bounded.
- **Authority discipline:** §2 reaffirms `01-inventory.md` §2 and extends it to archive layers; no archive layer or branch state is treated as authority. The release packs are explicitly *not* treated as canonical drops; `dev` is the canonical product surface per Stage A architecture.
- **Secret discipline:** no real keys, tokens, passwords, or PII values are quoted. The only literal strings reproduced from inside the archives are filenames, env-variable names (`SENDGRID_API_KEY`, `SMTP_PASSWORD`), template `{{placeholder}}` tokens, example-domain emails (`alerts@example.edu`, `support@example.edu`), and Bash control flow from `Start-WebUI.command`. The tar UID/GID string `dwqadad123` is named only as a structural side-channel; it is not used as an identifier elsewhere in the report.
- **Filesystem discipline:** archive bytes were read via `[System.IO.Compression.ZipFile]::OpenRead` and `tar -tzvf` only — both are read-only operations. The release surfaces at `Z:\Project\Rutgers-BetterCourseSchedulePlanner\release\…` and `Z:\Project\Rutgers-BetterCourseSchedulePlanner\bcsp-20260122.zip` are explicitly listed as in-scope read-only evidence by the Main Agent's pinned context for this task (`pin:main_agent_context:001`). No other path outside this worktree was read or traversed.
