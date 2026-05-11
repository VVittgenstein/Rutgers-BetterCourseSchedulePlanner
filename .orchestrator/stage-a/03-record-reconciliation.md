# Stage A — Legacy Record and Workflow Reconciliation

> **Task:** `task-003` Stage A legacy record and workflow reconciliation
> **Branch:** `feature/task-003`
> **Worktree:** `Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\task-003`
> **Depends on:** `task-001` (`.orchestrator/stage-a/01-inventory.md`, already merged)
> **Scope:** Audit/report only. The only file produced is this reconciliation document under `.orchestrator/stage-a/`. No product source, tests, runtime data, configs, package files, or legacy record files are modified.

## 1. Method

This task reconciles the legacy AI-workflow record families against the *actual* state of the current tracked repository on `dev` and `feature/task-003`. Evidence collected:

- `git ls-files` for the full tracked set on this worktree (262 files, identical to `dev` plus this report).
- Direct content reads of `record.json`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`, `read_only.md`, `AGENTS.md`, the six `review/` records, twelve sampled `Compact/` records (Compact-ST-* across acts 001–011 and the two later seed initiatives), `Rutgers-dr/2025-11-11-dr.md`, `Rutgers-dr/Patch-dr-2025-11-14.md`, and `reports/field_validation.md`.
- Directory inventories of `Compact/` (74 files), `review/` (6 files), `reports/` (6 files), `Rutgers-dr/` (2 files).
- A read-only listing of the Obsidian prompt folder `D:\Document\Obsidian\Adrian\Prompt\BetterCourseSchedulePlanner` and content reads of `Meta-Prompt.md`, `4b-dr.md`, `5b-mb.md`, `6b-json-v3.md`, `7b-dr-patch.md`, `9-code.md`, `11-compact.md`, `14-com.md`, `14a-compact.md`, `15a-update.md`, `GIT-Clone.md`, `未命名.md`. The vault sits outside the worktree and is treated as external evidence, not a write target.
- Cross-reference of every `artifacts` path listed in each legacy task/subtask against `git ls-files` to determine which artifacts actually exist today.

All file paths in this report are repository-relative unless explicitly noted. No real secrets are quoted; placeholder/test values are described rather than reproduced.

## 2. Evidence Layers and Authority Disclaimer

Stage A policy from `task-001` is preserved: **`.gitignore`, tracked state, ignored state, untracked state, and remote state are evidence layers, not authorities.** This reconciliation adds three more evidence layers that are equally non-authoritative on their own:

| Layer | What it shows | Why it is **not** authority |
| --- | --- | --- |
| Legacy planning JSON (`record.json`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`) | Self-reported task graphs and statuses authored by an earlier AI workflow. | The status field of any task/subtask reflects whatever the last agent run wrote; it can lag behind, leap ahead of, or directly contradict implemented code. |
| `Compact/` markdown files | Per-subtask narrative snapshots authored by an AI compaction step (template `11-compact.md`), often followed by a Code-Review trailer (`## Code Review - <SubtaskID> - <TS>`). | Compacts describe *what an agent said it did at one point in time*. They can outlive the code they describe (e.g. work later reverted) and can also be authored without the change ever reaching this branch. They are useful corroboration, not proof. |
| `review/` markdown files | Git unified diffs of `record.json` mutations from 2025-11-13 to 2025-11-17. | Six snapshots only; chain has gaps (see §6.4) and stops well before `record.json` itself stopped being edited. They are a partial, week-long forensic trail, not a complete audit log. |
| `Rutgers-dr/` documents | Hand-authored deep-research and patch-DR markdown that seeded the planning JSON. | Historical narrative; explicitly superseded in some places by later decisions captured in `record.json.decisions` and by the `Patch-dr-2025-11-14.md` architecture pivot. |
| `reports/` markdown / JSON | Measurement artifacts referenced by terminal subtasks (field validation, fresh install, mail latency, poller durability). | They prove a measurement happened; they do not prove that the planning record describing them is still current. |
| `read_only.md` | A project-level DR distillation (`file: 2025-11-11-dr.md`, `version: 0.1`, `derived_from: "Deep Research (raw notes in context)"`). | Labelled `read_only` by intent; in fact, see §7.1, several of its decisions and Action Seeds were subsequently *overridden* by `record.json.decisions` after `Patch-dr-2025-11-14.md`, but the file itself was not updated. The name implies authority that the file no longer carries. |
| `AGENTS.md` | The ngagent main-agent system prompt (`<!-- ngagent-doc-version: 0.8.0 -->`). | Current orchestrator entry doc for the *new* ngagent workflow. It is current for ngagent governance, but it is **not** authoritative for the legacy `record.json`/Compact pipeline that pre-dates ngagent. |
| Obsidian prompt vault (out-of-repo, on `D:`) | Personal prompt templates that drove the legacy workflow (DR → distillation → JSON → Compact → review → update). | External evidence outside the repo; explains the *shape* of the legacy records but holds no authority over either the repo or the current ngagent setup. The vault folder is itself named with the project's *old* name (`BetterCourseSchedulePlanner`, no `Rutgers-` prefix). |

**Hard rule applied here, mirroring §2 of `01-inventory.md`:** the words "done," "todo," "in_progress," and "in_review" inside any legacy record are *self-report by an earlier agent*, never proof of current code state. Every status finding in this report cites either (a) the presence/absence of the referenced `artifacts` paths in current `git ls-files`, or (b) the content of a corroborating Compact/review file, never the legacy record's status field alone.

## 3. Record Family Inventory

| Family | Tracked? | Files / size | Date span | Authoring source |
| --- | --- | --- | --- | --- |
| `record.json` | yes (root) | 1 file, ~108 KB, 2,390 lines | `generated_at: 2025-11-13`; per-task `updated_at` 2025-11-13 .. 2025-11-21 | Obsidian `6b-json-v3.md` initially; then patched via `7b-dr-patch.md` / `15a-update.md` cycles; legacy planning JSON. AGENTS.md notes runtime ngagent state lives at `.git/ngagent/`, so the root copy is **legacy bootstrap, not the live ngagent record**. |
| `rEmail.json` | yes (root) | 1 file, 253 lines | `generated_at: 2025-11-22`; `updated_at: 2025-11-24T12:00:00Z` | Seed for `T-20251122-mail-onboarding-no-code`; `md_source: rEmail-plan.md` (md source not tracked). |
| `rRevision.json` | yes (root) | 1 file, 170 lines | `generated_at: 2025-11-22` (`md_source: 2025-11-22-rRevision.md`, not tracked) | Seed for `T-20251122-filter-rewrite` (11-item filter rewrite). |
| `rSubscribe.json` | yes (root) | 1 file, 144 lines | `generated_at: 2025-11-24` (`md_source: rSubscribe-plan.md`, not tracked) | Seed for `T-20251124-auto-term-poller`. |
| `Compact/` | yes | 74 markdown files | 2025-11-16 .. 2025-11-23 (UTC) | Generated by Obsidian template `11-compact.md`; subsequent reviews appended via `14-com.md` / `14a-compact.md` (`## Code Review - <SubtaskID> - <TS>` trailer). |
| `review/` | yes | 6 markdown files | 2025-11-13 .. 2025-11-17 | Each file is a `diff --git a/record.json b/record.json` snapshot; one filename typo (`review-recod-2025-11-14.md`). |
| `Rutgers-dr/` | yes | 2 markdown files | `2025-11-11-dr.md` (deep research, Chinese), `Patch-dr-2025-11-14.md` (architecture-pivot patch DR). | Hand-authored DR; Obsidian template `4b-dr.md` is the corresponding generation prompt. |
| `reports/` | yes | 6 files | 2025-11-17 .. 2025-11-21 (per record.json `updated_at` of terminal subtasks) | Measurement outputs from terminal subtasks (field validation, fresh install, mail latency, poller durability). |
| `read_only.md` | yes (root) | 1 file, 432 lines, `file: 2025-11-11-dr.md`, `version: 0.1`, `date: 2025-11-13T00:11:00` | 2025-11-13 | Obsidian template `5b-mb.md` — DR distillation; intended to seed `6b-json-v3.md` (`record.json`). |
| `AGENTS.md` | yes (root) | 1 file, 337 lines, `<!-- ngagent-doc-version: 0.8.0 -->` | Current | ngagent orchestrator system prompt; current reference, not legacy. |
| `D:\Document\Obsidian\Adrian\Prompt\BetterCourseSchedulePlanner` (out-of-repo) | n/a (external) | 13 markdown files (`Meta-Prompt.md`, `4b-dr.md`, `5b-mb.md`, `6b-json-v3.md`, `7b-dr-patch.md`, `7b-review.md`, `9-code.md`, `11-compact.md`, `14-com.md`, `14a-compact.md`, `15a-update.md`, `GIT-Clone.md`, `未命名.md`) | 2025-11-11 .. 2026-01-13 (mtimes) | Hand-curated personal Obsidian vault; the legacy workflow's prompt library. Lives outside the repo; not subject to git policy. |

Counts above are observations from the worktree (`git ls-files` and direct `ls`) and from per-record `updated_at` fields, *not* assertions of current correctness.

## 4. Legacy → Current Mapping (record.json)

`record.json` records ten top-level tasks (`T-20251113-soc-api-validation` and `T-20251113-act-001 .. -011` minus `-012`) plus thirty-one subtasks. Per-task mapping is below; each row is reconciled three ways: parent status, subtasks-claimed-done, and presence of artifact paths in current `git ls-files`.

Legend: **✓** = artifact present on `dev`/`feature/task-003`; **✗** = artifact missing; **rename** = same intent, different path. Status column is the *legacy claim*, with a parenthesized note when it conflicts with code.

### 4.1 SOC API research

| Legacy id | Title | Legacy status | Current files (`✓`/`✗`) | Reconciliation |
| --- | --- | --- | --- | --- |
| `T-20251113-soc-api-validation` | 验证 Rutgers SOC API 参数、限流与字段行为 | `done` | docs/soc_api_notes.md ✓, docs/soc_field_matrix.csv ✓, docs/soc_rate_limit.md ✓, docs/soc_rate_limit.{latest,courses_stress,courses_stress2,openSections_blitz}.json ✓ | Consistent: research outputs present; backed by Compact records 2025-11-16 .. 2025-11-17 and review-2025-11-17-T023522Z chain. |
| ↳ `ST-…-01-probe` | SOC 探针脚本与最小调用验证 | `done` | scripts/soc_probe.ts ✓, docs/soc_api_notes.md ✓ | Consistent. |
| ↳ `ST-…-02-field-matrix` | SOC 字段跑批与字段矩阵 | `done` | scripts/soc_field_matrix.py ✓, docs/soc_field_matrix.csv ✓ | Consistent. |
| ↳ `ST-…-03-limit-profile` | 限流与错误码画像 | `done` | scripts/soc_rate_limit.ts ✓, scripts/soc_api_client.ts ✓, docs/soc_rate_limit*.json ✓ | Consistent. |

### 4.2 Local DB schema

| Legacy id | Title | Legacy status | Current files | Reconciliation |
| --- | --- | --- | --- | --- |
| `T-20251113-act-007-local-db-schema` | 设计本地数据库模式与增量更新机制 | `done` | data/schema.sql ✓, data/migrations/001_init_schema.sql ✓, data/migrations/002_relax_section_index_scope.sql ✓, scripts/migrate_db.ts ✓; **docs/db_schema.md ✗** (referenced as artifact, never created — actual file is `docs/local_data_model.md` ✓) | Consistent in spirit; one artifact-path drift (`docs/db_schema.md` vs the `local_data_model.md` actually shipped per the `-01-entity-design` subtask). |
| ↳ `ST-…-01-entity-design` | 本地课程实体模型设计 | `done` | docs/local_data_model.md ✓ | Consistent. |
| ↳ `ST-…-02-migration-tooling` | SQLite 迁移与初始化机制 | `done` | scripts/migrate_db.ts ✓, data/migrations/*.sql ✓ | Consistent. |
| ↳ `ST-…-03-incremental-strategy` | 增量更新策略验证 | `done` | docs/data_refresh_strategy.md ✓, notebooks/incremental_trial.md ✓, scripts/incremental_trial.ts ✓ | Consistent. (01-inventory §4.5 already flagged `notebooks/` as a move-candidate into `reports/`.) |

### 4.3 SOC data ingestion

| Legacy id | Title | Legacy status | Current files | Reconciliation |
| --- | --- | --- | --- | --- |
| `T-20251113-act-001-soc-json-scraper` | 封装 Rutgers SOC 数据抓取与本地数据库初始化脚本 | `done` | scripts/fetch_soc_data.ts ✓; **`data/courses.sqlite` ✗ as tracked** (file is intentionally ignored — `.gitignore:142`; runtime artifact, not a tracked artifact); per-evidence-layer rule (§2 of 01-inventory), absence-from-tracking is **not** authority to delete the listing — it just means the artifact is a local runtime file, not a deliverable. | Consistent. Note: the task id (`-soc-json-scraper`) is *naming inertia* — the task was re-scoped after `Patch-dr-2025-11-14.md` from "static JSON scraper" to "local DB initializer," but the slug retained the old wording. |
| ↳ `ST-…-01-pipeline-config` | 抓取流程接口与配置确定 | `done` | docs/fetch_pipeline.md ✓, configs/fetch_pipeline.example.json ✓ | Consistent. |
| ↳ `ST-…-02-ingest-impl` | SOC 拉取与写入实现 | `done` | scripts/fetch_soc_data.ts ✓, scripts/soc_normalizer.ts ✓; **`logs/fetch_runs/*.log` ✗** (not tracked; `logs/` `.gitignore`-matched at line 136 of 01-inventory's inventory). | Consistent code-wise; the `logs/` artifact path was always a runtime path, not a deliverable — listing it is harmless. |
| ↳ `ST-…-03-data-verification` | 字段覆盖验证与初始化 runbook | `done` | docs/data_load_runbook.md ✓, reports/field_validation.md ✓ | Consistent. |

### 4.4 Local query API

| Legacy id | Title | Legacy status | Current files | Reconciliation |
| --- | --- | --- | --- | --- |
| `T-20251113-act-008-local-query-api` | 实现本地课程查询 API 服务 | `done` | **Parent `artifacts` field still lists placeholder paths `services/api/server.ts`, `services/api/routes/courses.ts`, `tests/api/courses.test.ts` ✗** that do NOT match current layout. Actual files: api/src/server.ts ✓, api/src/routes/courses.ts ✓, api/tests/course_search.test.ts ✓. Subtask-level artifacts are correctly written; the *parent-level* `artifacts` array was never updated when the project moved from `services/api/` to `api/src/` during `act-008-01-contract`. | **Stale at parent level, consistent at subtask level.** Same drift recurs at `act-009/-010/-011/-003/-005` parents — see §6.3. |
| ↳ `ST-…-01-contract` | 课程查询接口契约与服务骨架 | `done` | api/src/{config,container}.ts ✓, api/src/plugins/requestLogging.ts ✓, api/src/routes/{sharedSchemas,courses,sections,filters,health}.ts ✓, api/src/types/fastify.d.ts ✓, api/src/server.ts ✓, docs/query_api_contract.md ✓ | Consistent. |
| ↳ `ST-…-02-filter-engine` | 本地查询与筛选实现 | `done` | api/src/queries/course_search.ts ✓, api/tests/course_search.test.ts ✓ | Consistent. |
| ↳ `ST-…-03-api-hardening` | 错误处理、健康检查与日志 | `done` | api/src/server.ts ✓, api/src/routes/health.ts ✓, api/tests/health.test.ts ✓ | Consistent. |

### 4.5 Frontend filter MVP

| Legacy id | Title | Legacy status | Current files | Reconciliation |
| --- | --- | --- | --- | --- |
| `T-20251113-act-002-frontend-filter-mvp` | 实现基于本地查询 API 的课程列表与多维筛选（MVP） | **`todo`** (parent) | frontend/src/components/{FilterPanel,FilterPanel.css,TagChip,TagChip.css,SchedulePreview,SchedulePreview.css,CourseList}.tsx ✓, frontend/src/state/courseFilters.ts ✓, frontend/src/hooks/useCourseQuery.ts ✓, frontend/src/dev/{ComponentPlayground.tsx,mockData.ts} ✓, docs/ui_flow_course_list.md ✓; **parent `artifacts` field lists `src/pages/courses.tsx`, `src/components/filters/*`, `src/components/calendar-view/*`, `src/api/hooks/useCourses.ts` — none exist under those paths.** | **Stale: parent says `todo`, but all three subtasks below are `done` and the FE filter MVP has shipped per code.** Same naming-pre-refactor pattern as 4.4 — parent `artifacts` paths were never reconciled to the chosen `frontend/src/...` layout. The 2025-11-22 filter rewrite (see §5.2) modified these exact files. |
| ↳ `ST-…-01-ui-architecture` | 课程列表信息架构与状态规划 | `done` | docs/ui_flow_course_list.md ✓, frontend/src/state/courseFilters.ts ✓ | Consistent. |
| ↳ `ST-…-02-filter-components` | 筛选组件与日历视图 MVP | `done` | frontend/src/components/{FilterPanel,TagChip,SchedulePreview}.{tsx,css} ✓, frontend/src/dev/* ✓ | Consistent (the `frontend/src/dev/*` developer playground is the `verify` row in 01-inventory §4.10). |
| ↳ `ST-…-03-api-integration` | 接入查询接口并优化交互 | `done` | frontend/src/hooks/useCourseQuery.ts ✓, frontend/src/components/CourseList.tsx ✓ | Consistent. |

### 4.6 i18n setup

| Legacy id | Title | Legacy status | Current files | Reconciliation |
| --- | --- | --- | --- | --- |
| `T-20251113-act-005-frontend-i18n-setup` | 搭建前端 i18n 架构并完成中英文界面 | **`todo`** (parent) | frontend/src/i18n/index.ts ✓, frontend/src/components/LanguageSwitcher.tsx ✓, scripts/i18n_missing_check.ts ✓, docs/i18n_key_map.md ✓, frontend/i18n/messages.json (tracked; per 01-inventory). **Parent `artifacts` lists `src/i18n/index.ts`, `locales/en/*.json`, `locales/zh/*.json` ✗** — wrong paths (no `locales/` directory; i18n lives in `frontend/i18n/messages.json` and `frontend/src/i18n/`). | **Stale: parent says `todo`, all 3 subtasks are `done`, code ships.** Same parent-artifacts drift. |
| ↳ `ST-…-01-copy-audit` | 文案盘点与 i18n key 规划 | `done` | docs/i18n_key_map.md ✓, frontend/i18n/messages.json ✓ | Consistent. |
| ↳ `ST-…-02-i18n-integration` | i18n 库接入与页面改造 | `done` | frontend/src/i18n/index.ts ✓, frontend/src/{main,App}.tsx ✓, frontend/src/components/* ✓, frontend/i18n/messages.json ✓ | Consistent. (Subtask references `frontend/src/i18n/types.d.ts` which does not appear in `git ls-files` — likely deleted later; minor evidence drift, not a contradiction.) |
| ↳ `ST-…-03-language-toggle` | 语言切换 UI 与缺失检测 | `done` | frontend/src/components/LanguageSwitcher.tsx ✓, scripts/i18n_missing_check.ts ✓ | Consistent. |

### 4.7 Subscriptions

| Legacy id | Title | Legacy status | Current files | Reconciliation |
| --- | --- | --- | --- | --- |
| `T-20251113-act-009-subscription-management` | 实现订阅管理接口与本地存储 | **`todo`** (parent) | api/src/routes/subscriptions.ts ✓, api/tests/subscriptions.test.ts ✓, frontend/src/components/{SubscribeButton,SubscriptionCenter,SubscriptionManager}.tsx ✓, frontend/src/api/subscriptions.ts ✓, docs/subscription_model.md ✓. **Parent `artifacts` lists `src/components/subscription/*`, `services/api/routes/subscriptions.ts`, `db/subscriptions.sql` ✗** — wrong paths. | **Stale: parent says `todo`, all 3 subtasks are `done`, code ships.** Same parent-artifacts drift. |
| ↳ `ST-…-01-subscription-model` | 订阅数据结构与接口契约 | `done` | docs/subscription_model.md ✓ | Consistent. |
| ↳ `ST-…-02-subscribe-endpoints` | 订阅/退订接口与写入逻辑实现 | `done` | api/src/routes/subscriptions.ts ✓, api/tests/subscriptions.test.ts ✓ | Consistent. |
| ↳ `ST-…-03-frontend-flow` | 前端订阅流与端到端验证 | `done` | frontend/src/components/SubscribeButton.{tsx,css} ✓, frontend/src/components/CourseList.tsx ✓, frontend/src/api/subscriptions.ts ✓ | Consistent. |

### 4.8 Open-sections poller

| Legacy id | Title | Legacy status | Current files | Reconciliation |
| --- | --- | --- | --- | --- |
| `T-20251113-act-010-local-polling-notify` | 本地空位轮询与通知调度服务 | **`todo` + `blocked: true` (blocked_by DEP-001 = SOC API "unknown")** | workers/open_sections_poller.ts ✓, workers/tests/open_sections_poller.auto.test.ts ✓, data/migrations/003_open_events.sql ✓, scripts/poller_resume_sim.ts ✓, scripts/poller_checkpoint.json ✓ (01-inventory §4.6 flags this as a stray runtime artifact), reports/poller_durability.md ✓, docs/open_event_spec.md ✓. **Parent `artifacts` lists `services/notifier/poller.ts`, `services/notifier/event_store.ts`, `tests/notifier/poller.test.ts` ✗** — wrong paths. | **Stale + contradictory: blocked=true on an API marked "unknown," yet the poller ships, runs, and has a durability report.** The block has been overtaken by reality. |
| ↳ `ST-…-01-event-spec` | 开放事件模型与轮询策略设计 | `done` | docs/open_event_spec.md ✓, data/migrations/003_open_events.sql ✓ | Consistent. |
| ↳ `ST-…-02-polling-worker` | openSections 轮询 worker 实现 | `done` | workers/open_sections_poller.ts ✓, data/migrations/003_open_events.sql ✓ | Consistent. (`logs/poller/*.log` listed but `logs/` is `.gitignore`-matched.) |
| ↳ `ST-…-03-resume-tests` | 长运行与重启恢复验证 | `done` | reports/poller_durability.md ✓, scripts/poller_checkpoint.json ✓ | Consistent. |

### 4.9 Mail sender stack

| Legacy id | Title | Legacy status | Current files | Reconciliation |
| --- | --- | --- | --- | --- |
| `T-20251113-act-011-mail-sender-module` | 邮件发送模块多平台支持 | **`todo`** (parent) | notifications/mail/{config,retry_policy,template_loader,template_checker,types}.ts ✓, notifications/mail/providers/sendgrid.ts ✓, notifications/mail/tests/{config,provider,retry_policy,template_checker}.test.ts ✓, configs/mail_sender.example.json ✓, docs/{mail_sender_contract,mail_sender_usage}.md ✓. **Parent `artifacts` lists `services/notify/mail_sender.ts`, `config/email.example.json`, `templates/email/*` ✗** — wrong paths. Actual templates live under `configs/templates/email/{open-seat,verification}/{en-US,zh-CN}.{html,txt}` ✓. | **Stale: parent says `todo`, all 3 subtasks are `done`, full mail stack ships, including templates and tests.** Same parent-artifacts drift. |
| ↳ `ST-…-01-mail-interface` | 邮件发送接口与配置基线 | `done` | docs/mail_sender_contract.md ✓, configs/mail_sender.example.json ✓ | Consistent. |
| ↳ `ST-…-02-provider-adapter` | 首个邮件 Provider 适配层 | `done` | notifications/mail/{types,config,template_loader}.ts ✓, notifications/mail/providers/sendgrid.ts ✓, notifications/mail/tests/provider.test.ts ✓ | Consistent (subtask explicitly lists `sendgrid.ts`; earlier comments mention SMTP as alternative — actual ship is SendGrid-only, no SMTP provider; SMTP is supported only via the user.json's `passwordEnv` SMTP path described in 01-inventory §4.7). |
| ↳ `ST-…-03-retry-tuning` | 失败重试与速率限制策略 | `done` | notifications/mail/retry_policy.ts ✓, notifications/mail/tests/retry_policy.test.ts ✓, docs/mail_sender_usage.md ✓ | Consistent. |

### 4.10 Mail-notify worker

| Legacy id | Title | Legacy status | Current files | Reconciliation |
| --- | --- | --- | --- | --- |
| `T-20251113-act-003-mail-notify-function` | 实现课程空位邮件通知功能（本地定时任务） | **`todo` + `blocked: true` (blocked_by DEP-001 + DEP-002)** | workers/mail_dispatcher.ts ✓, workers/tests/mail_dispatcher.test.ts ✓, docs/{mail_worker_contract,notify_runbook}.md ✓, scripts/mail_e2e_sim.ts ✓, reports/mail_worker_latency.md ✓. **Parent `artifacts` lists `services/notifier/email_worker.ts`, `templates/email/*`, `config/email.json` ✗** — wrong paths. | **Stale + contradictory: blocked=true, parent=todo, but all 3 subtasks are `done` and a latency report exists.** Block has been overtaken. |
| ↳ `ST-…-01-worker-contract` | 邮件通知 worker 契约 | `done` | docs/mail_worker_contract.md ✓ | Consistent. |
| ↳ `ST-…-02-worker-implementation` | 邮件 worker 实现 | `done` | workers/mail_dispatcher.ts ✓, workers/tests/mail_dispatcher.test.ts ✓ | Consistent. |
| ↳ `ST-…-03-end-to-end-validation` | 端到端延迟与幂等验证 | `done` | scripts/mail_e2e_sim.ts ✓, reports/mail_worker_latency.md ✓, docs/notify_runbook.md ✓ | Consistent. |

### 4.11 Discord channel — the strongest contradiction

| Legacy id | Title | Legacy status | Current files | Reconciliation |
| --- | --- | --- | --- | --- |
| `T-20251113-act-004-discord-notify-channel` | 实现 Discord 通知通道并评估私信与频道策略 | **`todo` + `blocked: true`** (parent) | **No Discord-related files exist on `dev` or `feature/task-003`.** `git ls-files \| grep -iE "discord"` returns zero matches. No `notifications/discord/`, no `workers/discord_dispatcher.ts`, no `docs/discord_strategy.md` / `discord_runbook.md`, no `configs/discord_bot.example.json`, no `reports/discord_channel_validation.md`. | **Contradictory in two directions at once.** Parent says blocked/todo. All three subtasks below claim `status: "done"` with explicit `artifacts` paths and corroborating Compact records — but those `artifacts` are entirely absent from the current tree. |
| ↳ `ST-…-01-strategy` | Discord 通知策略与配置 | `done` | docs/discord_strategy.md ✗, configs/discord_bot.example.json ✗ | **Stale/contradictory.** No corresponding Compact file (none of the 74 `Compact/` entries match `act-004-01`; 01-inventory's 74-file census confirms this) — so even the Compact narrative is missing for the "strategy" subtask. |
| ↳ `ST-…-02-bot-sending` | Discord Bot 发送能力 | `done` | notifications/discord/bot.ts ✗, notifications/discord/tests/bot.test.ts ✗, scripts/discord_send_test.ts ✗ | **Stale/contradictory.** Three Compact files exist (`Compact-ST-20251113-act-004-02-bot-sending-2025-11-21-T{105707,110000,112050}Z.md`) and quote substantive implementation details (`DiscordBot.send`, dedupeKey, 429 retry, token-bucket queue). Code reviewed and "done" but **not present** in current `dev`. |
| ↳ `ST-…-03-event-integration` | 事件流接入与推荐策略 | `done` | notifications/discord/adapter.ts ✗, workers/discord_dispatcher.ts ✗, workers/tests/discord_dispatcher.test.ts ✗, docs/discord_runbook.md ✗, reports/discord_channel_validation.md ✗ | **Stale/contradictory.** Three Compact files exist; one contains a code-review trailer with literal `workers/discord_dispatcher.ts` line numbers (lines +318..+336). The work clearly existed at some point — but is **not** in current `dev`. |

**Interpretation:** the Discord work appears to have been implemented, compacted, code-reviewed (Codex Compact ER trailers prove this), and then either deleted or never merged onto `dev`. Either way, the artefact for Stage A is the same: **subtask status `done` is not authority; the artifact path check on `git ls-files` is.** This is the clearest single case that justifies the §2 rule. It is also a candidate for §8.3 Stage-B handling: either reinstate from the deleted commits (if recoverable from history) or formally remove the task tree. The Compact records remain valuable as documentation of what *was* designed.

### 4.12 Deployment / one-click

| Legacy id | Title | Legacy status | Current files | Reconciliation |
| --- | --- | --- | --- | --- |
| `T-20251113-act-006-deploy-docs-cicd` | 编写并验证本地一键部署文档与辅助脚本 | `done` | README.md ✓, `.env.example` ✗ (not tracked — `.env*` is `.gitignore`-matched with `!.env.example` allowlisted per 01-inventory §4.7; the file **was listed as an artifact** but is absent both from `git ls-files` and from the worktree), scripts/{setup_local_env.sh,run_stack.sh,oneclick_start.js} ✓, docs/{deployment_playbook,quickstart,oneclick}.md ✓, Start-WebUI.{bat,command} ✓, reports/fresh_install_log.md ✓. | **Mostly consistent**, with one artefact drift (`.env.example` listed but never created; `.gitignore` allowlists the name but no file exists). |
| ↳ `ST-…-01-deploy-playbook` | 从零部署步骤梳理 | `done` | docs/deployment_playbook.md ✓ | Consistent. |
| ↳ `ST-…-02-automation-scripts` | 部署流程脚本化 | `done` | scripts/{setup_local_env.sh,run_stack.sh} ✓ | Consistent. |
| ↳ `ST-…-03-fresh-run` | 新手视角试跑与 README 迭代 | `done` | README.md ✓, docs/quickstart.md ✓, reports/fresh_install_log.md ✓ | Consistent. |

### 4.13 Summary of stale / contradictory states in `record.json`

| Category | Tasks |
| --- | --- |
| **Stale parent status (subtasks done, parent still `todo`)** | `act-002`, `act-005`, `act-009`, `act-011` (4 tasks) |
| **Stale parent status with `blocked:true` overtaken by shipped code** | `act-003`, `act-010` (2 tasks) |
| **Contradictory: subtasks claim `done` with artifacts that don't exist** | `act-004` (1 task, all 3 subtasks) |
| **Naming inertia in IDs** | `T-20251113-act-001-soc-json-scraper` (re-scoped to local DB), `D-20251113-arch-static-frontend-b` (decision is *against* static frontend), `read_only.md` (no longer authoritative — see §7.1) |
| **Parent-level `artifacts` paths point to a pre-refactor layout** | `act-002`, `act-003`, `act-005`, `act-008`, `act-009`, `act-010`, `act-011` (7 tasks; the post-`Patch-dr-2025-11-14.md` move from `services/notifier/`, `services/api/`, `services/notify/`, `src/components/`, `locales/`, etc. to `api/src/`, `workers/`, `notifications/`, `frontend/src/`, `frontend/i18n/` was reflected at subtask level but never reflected at parent level) |
| **Artifacts listed but never created** | `T-20251113-act-006`'s `.env.example`; `T-20251113-act-007`'s `docs/db_schema.md` (actual file is `docs/local_data_model.md`); `T-20251113-act-001-02`'s `logs/fetch_runs/*.log` (ignored runtime path); `T-20251113-act-010-02`'s `logs/poller/*.log` (ignored runtime path) |

## 5. Legacy → Current Mapping (seed initiatives)

### 5.1 `rEmail.json` — mail onboarding (T-20251122-mail-onboarding-no-code)

Parent task and all five subtasks are marked `done`. All artifact paths exist in `git ls-files`.

| Subtask | Status | Current files |
| --- | --- | --- |
| `ST-20251122-mail-config-direct-key` | `done` | notifications/mail/config.ts ✓, notifications/mail/types.ts ✓, notifications/mail/tests/config.test.ts ✓, docs/mail_sender_contract.md ✓ |
| `ST-20251122-mail-admin-api` | `done` | api/src/routes/admin.ts ✓, api/tests/admin_mail_config.test.ts ✓, configs/mail_sender.user.json ✓ (01-inventory §4.7 flags this as a structural secret-risk surface) |
| `ST-20251122-mail-frontend-settings` | `done` | frontend/src/components/MailSettingsPanel.{tsx,css} ✓, frontend/src/api/admin.ts ✓, frontend/src/api/types.ts ✓, frontend/src/api/client.ts ✓ |
| `ST-20251122-mail-launcher-integration` | `done` | scripts/run_stack.sh ✓, scripts/oneclick_start.js ✓ |
| `ST-20251122-mail-template-check` | `done` | notifications/mail/template_checker.ts ✓, notifications/mail/tests/template_checker.test.ts ✓, api/src/routes/admin.ts ✓, api/tests/admin_mail_config.test.ts ✓, scripts/mail_templates.js ✓ |

**Reconciliation:** `rEmail.json` is the **closest legacy record to ground truth**. All claimed `done` subtasks correspond to existing tracked files. The duplicated `artifacts` field (the JSON has the field twice — lines 43 and 57 — with the second being more complete) is a JSON oddity that does not block reading but should be flagged. The plan's stated risk that `configs/mail_sender.user.json` would be the slot for the next saved key is precisely the structural-secret-risk surface 01-inventory §4.7 / §6 identified — `rEmail.json` is therefore *evidence that the structural risk was a deliberate design choice*, not an accident.

### 5.2 `rRevision.json` — 11-item filter rewrite (T-20251122-filter-rewrite)

Parent task: `status: todo`. Subtask statuses are mixed; reconciliation against current files and against the matching `Compact-ST-20251122-filter-rewrite-*` records yields the following:

| Subtask | Legacy status | Compact present? | Current files | Reconciliation |
| --- | --- | --- | --- | --- |
| `ST-20251122-filter-rewrite-01-frontend-state-ui` | `todo` | **Yes** — `Compact-ST-20251122-filter-rewrite-01-frontend-state-ui-2025-11-23-T062853Z.md` | frontend/src/state/courseFilters.ts ✓, frontend/src/components/FilterPanel.tsx ✓, frontend/src/hooks/useCourseQuery.ts ✓, frontend/src/dev/ComponentPlayground.tsx ✓, frontend/src/data/fallbackDictionary.ts ✓, frontend/src/api/{filters,types}.ts ✓, frontend/i18n/messages.json ✓ — *all the touched files exist with the post-rewrite shape described in the Compact (subset-only meeting days, openStatus all/openOnly, examCodes block, no waitlist/instructor/permission)*. | **Stale: should be at least `in_review`/`done`.** The Compact records the FE state/UI slimming, the build self-test (`npm run -C frontend build`) passing, and even lists the explicit interface-changes — yet the JSON still says `todo`. |
| `ST-20251122-filter-rewrite-02-api-schema-query` | `in_review` | **Yes** — `Compact-…-02-…-T082002Z.md`, with a `## Code Review - … - 2025-11-23T08:42:26Z` trailer "Didn't find any major issues. Swish!" | api/src/routes/courses.ts ✓ (examCode added, deprecated params dropped), api/src/queries/course_search.ts ✓ (`examCode` filter, subset semantics), api/tests/course_search.test.ts ✓ (new coverage). | **Stale: review trailer is `pass`; status should be `done`.** |
| `ST-20251122-filter-rewrite-03-data-pipeline-dicts` | `done` | **Yes** — `Compact-…-03-…-T110512Z.md`, with `## Code Review - … - 2025-11-23T11:20:44Z` trailer "Didn't find any major issues. You're on a roll." | scripts/fetch_soc_data.ts ✓, scripts/backfill_core_attributes.ts ✓ (new helper, `npm run data:backfill-core`), api/src/queries/course_search.ts ✓ (case-insensitive core match), api/src/queries/filters.ts ✓, frontend/src/data/fallbackDictionary.ts ✓. Compact reports 1,646 `course_core_attributes` rows backfilled on `data/courses.sqlite`. | Consistent. |
| `ST-20251122-filter-rewrite-04-docs-i18n` | `todo` | **No** Compact entry exists. | i18n strings already refreshed in `frontend/i18n/messages.json` (per Compact-01); `docs/query_api_contract.md` exists ✓ — but Stage A cannot verify from inside `git ls-files` whether the doc *was* updated for the new `examCode`/subset semantics without reading the doc itself. | **Indeterminate.** Likely partial done (i18n side of the work is in code) and partial pending (doc updates). Status `todo` is plausibly correct for the doc half. |

**Reconciliation:** Three of four subtasks are *more* done than `rRevision.json` claims; the parent task's `status: todo` is misleading. Stage B should not propagate any `todo` from this file without first checking the Compact + file evidence.

### 5.3 `rSubscribe.json` — auto-term poller (T-20251124-auto-term-poller)

Parent task: `status: in_progress`. All three subtasks: `status: done`.

| Subtask | Legacy status | Current files | Reconciliation |
| --- | --- | --- | --- |
| `ST-20251124-auto-mode-cli` | `done` | workers/open_sections_poller.ts ✓, workers/tests/open_sections_poller.auto.test.ts ✓ | Consistent. |
| `ST-20251124-dynamic-loops` | `done` | workers/open_sections_poller.ts ✓, workers/tests/open_sections_poller.auto.test.ts ✓ | Consistent. |
| `ST-20251124-launcher-docs` | `done` | scripts/run_stack.sh ✓, scripts/oneclick_start.js ✓, docs/{quickstart,oneclick,deployment_playbook}.md ✓ | Consistent. |

**Reconciliation:** **Stale parent status** — all three subtasks done, no remaining work indicated, but parent still flagged `in_progress`. The `workers/tests/open_sections_poller.auto.test.ts` (note the `.auto` suffix) and the test name confirm the auto-mode work shipped.

## 6. Cross-Cutting Contradictions

### 6.1 Architecture pivot recorded in `record.json` but not in `read_only.md`

`read_only.md` (the project's DR distillation, dated 2025-11-13) states:

- **D1**: "采用方案 B：前端静态站点 + 云函数/定时任务通知" (static frontend + cloud-function notifications, *not* a database backend).
- **D3**: "筛选逻辑尽可能全部在客户端完成" (do filtering in the browser, no complex query API).
- **ACT-001**: "封装 Rutgers SOC 数据抓取与静态 JSON 生成脚本" (static JSON output, *not* a local DB).
- **ACT-002**: "实现基于静态 JSON 的课程列表与多维筛选前端（MVP）".
- **ACT-003**: "设计并上线邮件通知云函数（基于 SOC openSections）".
- **DEP-004**: "静态前端托管平台（GitHub Pages / Netlify 等）".

`record.json` (2025-11-13, then patched after `Rutgers-dr/Patch-dr-2025-11-14.md`) records the opposite via four explicit decisions in `record.json.decisions`:

- **D-20251113-arch-static-frontend-b**: "v1.x 阶段采用'本地 SQLite 数据库 + 轻量 API 服务 + 前端 SPA'的一体化架构，而不是继续依赖纯静态前端加载大 JSON 与云函数通知。" — the decision *ID* still carries `static-frontend-b` as **naming inertia**, but the body chose the opposite of what the slug suggests.
- **D-20251113-client-side-filter**: "课程筛选与排序主要由嵌入式数据库与本地查询 API 承担，前端仅负责构建条件并展示结果，不再预加载全量数据."
- Task titles: `T-20251113-act-001 = "封装 Rutgers SOC 数据抓取与本地数据库初始化脚本"`, `T-20251113-act-002 = "实现基于本地查询 API 的课程列表与多维筛选（MVP）"`, `T-20251113-act-003 = "实现课程空位邮件通知功能（本地定时任务）"`.
- **DEP-004 in record.json**: "本地运行环境/容器（Node.js + SQLite + 定时任务）" — same DEP-ID as `read_only.md`, but a *different dependency*.

**Implication:** `read_only.md` is named "read-only" by intent but is *not* the current source of truth. Its D1/D3/ACT-001/-002/-003/DEP-004 are **all superseded** by `record.json.decisions` + `Rutgers-dr/Patch-dr-2025-11-14.md`. Anyone reading `read_only.md` in isolation would be misled. Stage B must treat `read_only.md` as historical evidence, not as a current decision register.

### 6.2 `read_only.md` misclassified by `01-inventory.md`

`01-inventory.md` §4.4 lists `read_only.md` under "Historical AI workflow records" with the rationale "ngagent main-agent system prompt v0.8.0 (marked `<!-- ngagent-doc-version: 0.8.0 -->`)" and the verify row "Verify whether `AGENTS.md` and `read_only.md` are intentionally distinct." Reality:

| Field | `read_only.md` (line 1–5) | `AGENTS.md` (line 1) |
| --- | --- | --- |
| First line | `## file: {{2025-11-11-dr.md}}` | `<!-- ngagent-doc-version: 0.8.0 -->` |
| Frontmatter | `project: "{{BetterCourseSchedulePlanner}}"`, `version: 0.1`, `derived_from: "Deep Research (raw notes in context)"` | `# Multi-Agent SWE Orchestrator — System Prompt`, ngagent v0.8.0 |
| Body | YAML Action Seeds, External Dependencies, FR/NFR catalogue from the 2025-11-11 DR | Main-agent prompt: hard constraints, dispatching Sub-agents, plan buckets, retry framework, escalation rules |
| Grep `ngagent-doc-version` | 0 hits | 1 hit (line 1) |

**`read_only.md` is not the ngagent main-agent prompt.** It is the project's DR distillation produced by Obsidian template `5b-mb.md` to seed `record.json` (via `6b-json-v3.md`). The two files are *not* duplicates; they belong to two entirely different workflows (legacy DR → JSON pipeline vs. current ngagent orchestrator). The 01-inventory verify row should resolve as: **distinct files, distinct workflows, neither is a duplicate of the other.**

(This errata does not invalidate `01-inventory.md` — its evidence-layer rule, its release/archive reconciliation pointer, its runtime/config flags, and its overall keep/archive taxonomy remain authoritative for Stage A. Only the one-line characterisation of `read_only.md` in §4.4 and §5 needs to be re-read as "DR distillation" instead of "ngagent main-agent prompt v0.8.0," and the verify row should be marked resolved.)

### 6.3 Parent-level `artifacts` paths are pre-refactor in `record.json`

After `Patch-dr-2025-11-14.md` flipped the architecture from "static + cloud functions" to "local DB + local API + local workers," the implementation moved from a `services/`-flavoured layout into the present `api/src/`, `workers/`, `notifications/`, `frontend/src/` layout. Subtask-level `artifacts` arrays were *consistently* updated as each Compact landed; **parent-level `artifacts` arrays were not.** Seven parents carry pre-refactor placeholder paths (act-002, -003, -005, -008, -009, -010, -011 — see §4.13). They are *not* useful as a path catalogue. Stage B should derive task-to-file mapping from subtask-level artifacts only, or rebuild a fresh mapping directly from `git ls-files` plus the file/AC matrix.

### 6.4 `review/` chain is incomplete and stops early

Blob index chain across the six files:

| File | from..to | Chain continuous? |
| --- | --- | --- |
| `review-record-2025-11-13.md` | `af485ee..5c4002c` | start |
| `review-recod-2025-11-14.md` | `5c4002c..5b7c3d9` | ✓ continues |
| `review-record-2025-11-15-T045714Z.md` | `4b865bc..30b5c1f` | **gap** (skipped 5b7c3d9→4b865bc) |
| `review-record-2025-11-16-T125256Z.md` | `30cf574..41c33c7` | **gap** (skipped 30b5c1f→30cf574) |
| `review-record-2025-11-17-T022319Z.md` | `e9fe2df..3b69daf` | **gap** (skipped 41c33c7→e9fe2df) |
| `review-record-2025-11-17-T023522Z.md` | `3b69daf..1c4b86e` | ✓ continues |

The chain has at least three gaps — `record.json` was edited *between* review snapshots without those edits being captured. Also, `record.json.updated_at` markers go up to **2025-11-21T19:48:20Z** (act-006), well past the last review file (2025-11-17). The `review/` corpus is therefore a *partial week-long* forensic trail, not an audit log. The filename `review-recod-2025-11-14.md` is a typo (`recod` → `record`) flagged separately in 01-inventory §4.10.

### 6.5 Naming drift: `BetterCourseSchedulePlanner` vs `Rutgers-BetterCourseSchedulePlanner`

`git remote -v` shows the canonical remote URL as `https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner.git` (per 01-inventory §7). However, the project's *old* name (no `Rutgers-` prefix) survives in several places:

| File | Reference | Stale? |
| --- | --- | --- |
| `package.json` | `repository.url`, `bugs.url`, `homepage` | **Yes** — points to a URL that does not match the current remote. |
| `Start-WebUI.bat`, `Start-WebUI.command` | "Starting BetterCourseSchedulePlanner (web UI)..." | User-visible string drift; cosmetic. |
| `read_only.md` line 2 | `project: "{{BetterCourseSchedulePlanner}}"` | Templated placeholder never updated. |
| `Compact/Compact-ST-20251113-soc-api-validation-02-field-matrix-2025-11-17-T013508Z.md`, `Compact/Compact-ST-20251113-act-009-01-subscription-model-…`, `Compact/Compact-ST-20251113-act-010-02-polling-worker-…` | Inline string mentions | Historical evidence; appropriate to leave alone. |
| `reports/field_validation.md` | Header reference | Historical. |
| Obsidian `GIT-Clone.md` | `git clone https://github.com/VVittgenstein/BetterCourseSchedulePlanner.git` | **Yes** — old URL; out-of-repo, but used by the human to bootstrap the workflow. |
| Obsidian folder name | `D:\Document\Obsidian\Adrian\Prompt\BetterCourseSchedulePlanner\` | Cosmetic; outside repo. |

Stage A is *report-only* — it does **not** rename anything. This drift is recorded here as evidence; Stage B's runtime/config-hygiene (`04-`) and Stage-B-handoff (`05-`) phases should decide whether to update `package.json` / launcher strings to match the current remote name. The Obsidian vault is out of scope.

### 6.6 `record.json` is bootstrap-legacy, not ngagent runtime state

Per `AGENTS.md` (hard constraint #2): "You MUST use `ngagent` for task and project state. Never manually edit runtime state (`record.json` is managed by ngagent under `.git/ngagent/`)…" The runtime ngagent state therefore lives under `.git/ngagent/tasks/<task-id>/{spec.v1.json, attempt-NNN.json, completion-NNN.json, review-NNN.json, …}` and `.git/ngagent/events.jsonl`. The root-level `record.json` (2,390 lines) is **not** that runtime state — it is the *legacy* JSON that the human's pre-ngagent workflow used to maintain by hand and via Obsidian prompts. Stage B must not confuse the two surfaces; merging the legacy `record.json` into ngagent's runtime state would re-introduce all the contradictions inventoried above.

### 6.7 Obsidian vault is missing `rRevision.json` from its prompt library

`D:\Document\Obsidian\Adrian\Prompt\BetterCourseSchedulePlanner\9-code.md` enumerates the coding-task entry-point prompts and lists only `record.json`, `rEmail.json`, `rSubscribe.json` — **not** `rRevision.json`. Yet `rRevision.json` exists in the repo (`generated_at: 2025-11-22`). This is consistent with `rRevision.json` having been added on 2025-11-22 *after* the Obsidian prompt library was last touched at this template. It is evidence (not authority) that the legacy workflow's tooling was already drifting from the legacy planning JSON it produced.

## 7. Errata vs. `01-inventory.md`

| Inventory line | Original phrasing | Correction supported by §6.2 |
| --- | --- | --- |
| §3 row for `read_only.md` | "ngagent main-agent prompt v0.8.0, declared `<!-- ngagent-doc-version: 0.8.0 -->`" | `read_only.md` does **not** contain the `<!-- ngagent-doc-version: 0.8.0 -->` marker (0 grep hits). It is the project's DR distillation (`file: 2025-11-11-dr.md`, `version: 0.1`, generated by Obsidian template `5b-mb.md`). The 0.8.0 marker belongs to `AGENTS.md` only. |
| §4.4 row for `read_only.md` | "ngagent main-agent system prompt v0.8.0 (marked `<!-- ngagent-doc-version: 0.8.0 -->`). Listed here because it is duplicative with `AGENTS.md` style content and is not a current planning artifact for this codebase. **Verify** whether `AGENTS.md` and `read_only.md` are intentionally distinct." | Resolved here: the two files are **intentionally distinct**, address different audiences/workflows, and contain different content. `read_only.md` belongs under "DR distillation / project decisions, stale post-Patch-DR" rather than "ngagent prompt duplicate". `AGENTS.md` is the current ngagent doc; it is **not** legacy. |
| §5 row for `read_only.md` | Same wording as §4.4 (verify against `AGENTS.md`). | Same resolution. |
| §4.4 / §5 row for `Rutgers-dr/` | "Chinese-language deep-research distillation referenced by `record.json` as `md_source`" | Confirmed. `record.json.md_source: "2025-11-11-dr.md"` ✓; `record.json.facts[0]` says "本 JSON 基于 2025-11-11-dr 研究结论整理". |

The above errata are scoped to one row of `01-inventory.md` and do not alter its broader taxonomy or its evidence-layer rule.

## 8. Classification and Stage-B Handling

### 8.1 Classification per record family

| Family | Classification | Recommendation for Stage B |
| --- | --- | --- |
| `record.json` (root) | **Historical evidence + stale + contradictory.** 4 stale parents (act-002, -005, -009, -011); 2 stale + blocked parents (act-003, -010) with shipped code; 1 contradictory tree (act-004 — all 3 subtasks `done` with missing artifacts); 7 parents with pre-refactor `artifacts` paths; 1 misleading decision-id slug; never live ngagent state. | **Archive** under a clearly named historical path (decision deferred to `07-cleanup-application.md`). Do **not** import any `status` field into ngagent runtime state. Build any task list for Stage B refactor candidates by reading code first, then cross-referencing Compact files for narrative context. |
| `rEmail.json` (root) | **Historical evidence.** All 5 subtasks `done`; all artifact paths present. Closest legacy record to ground truth. | **Archive.** Useful as the canonical summary of the no-code mail-onboarding feature; do not edit (it is also a record of *why* `configs/mail_sender.user.json` is structurally a secret-risk surface — see 01-inventory §4.7). |
| `rRevision.json` (root) | **Stale.** Three of four subtasks more done than claimed; parent `todo`. | **Archive.** Stage B should derive the filter-rewrite ground truth from the three matching `Compact-ST-20251122-filter-rewrite-*` records plus `git ls-files`, not from this JSON's status field. The fourth subtask (docs/i18n) may legitimately be partly outstanding — recheck `docs/query_api_contract.md` against the new `examCode` + subset semantics before declaring complete. |
| `rSubscribe.json` (root) | **Stale.** All subtasks `done`; parent still `in_progress`. | **Archive.** Auto-term-poller code lives under `workers/open_sections_poller.ts` and `workers/tests/open_sections_poller.auto.test.ts`; treat as shipped. |
| `Compact/` (74 files) | **Historical evidence.** More reliable than `record.json` for "what an agent claimed it did" because each Compact is dated and often carries a Code-Review trailer. Some Compacts (act-004-02, -03) describe code that is **not in current `dev`** — the Compacts are evidence of design and review activity, **not** evidence of merged code. | **Archive.** When reconciling any specific task in Stage B, prefer Compact narrative + `git ls-files` check over `record.json` status. Cross-reference per task ID. Do not merge Compacts into product source; they are append-only history. |
| `review/` (6 files) | **Historical evidence (partial).** Chain has gaps; stops at 2025-11-17 while `record.json` was edited through 2025-11-21. Filename typo on `review-recod-2025-11-14.md`. | **Archive.** Forensic only — useful for reconstructing how `record.json` evolved during the week of 2025-11-13..17. Not authoritative for current state. Rename of the typo file is a `07-cleanup-application.md` decision; check external references first (none in `Compact/` reference the typo, per inspection). |
| `Rutgers-dr/` (2 files) | **Historical evidence.** `2025-11-11-dr.md` is upstream of `read_only.md` and `record.json`. `Patch-dr-2025-11-14.md` is upstream of the architecture pivot recorded in `record.json.decisions`. | **Archive.** Keep as the immutable narrative explaining *why* the project moved from static + cloud functions to local DB + local services. Reference from Stage B handoff (`05-`). |
| `reports/` (6 files) | **Current reference.** Measurement outputs from terminal subtasks (act-001-03, -003-03, -006-03, -010-03). | **Keep as current reference.** They prove the corresponding subtasks shipped. The `reports/field_validation_details.mdpart` extension oddity and the `notebooks/incremental_trial.md` location are 01-inventory §4.5 verify items, not record-reconciliation items. |
| `read_only.md` (root) | **Stale + contradictory.** A *DR distillation* (not an ngagent prompt — see §6.2 / §7) whose D1/D3/ACT-001/-002/-003/DEP-004 were overridden by `Patch-dr-2025-11-14.md` and never re-rendered. | **Archive as DR distillation.** Do **not** treat as a current decision register. If Stage B needs an authoritative seed of project decisions, derive it from `record.json.decisions` + `Patch-dr-2025-11-14.md` + the relevant `docs/` runbooks. The 01-inventory verify row "AGENTS.md vs read_only.md" can be marked **resolved: distinct files, distinct workflows**. |
| `AGENTS.md` (root) | **Current reference.** ngagent main-agent system prompt v0.8.0. Not legacy; not duplicate of `read_only.md`. | **Keep as current reference.** Stage B's review-gate and dispatch protocols read from this file. |
| Obsidian prompt vault (`D:\Document\Obsidian\Adrian\Prompt\BetterCourseSchedulePlanner\`) | **External historical evidence.** Personal Obsidian vault holding the meta-prompts of the legacy DR→JSON→Compact→Review→Update workflow. `GIT-Clone.md` references the old repo name; `9-code.md` predates `rRevision.json`. | **Out of scope for Stage A writes.** Treat as the *explanation* for legacy record shapes (filename conventions for `Compact/`, `review/`, `record.json` schema, etc.). No change. Stage B should not rely on it as a source of current truth. |

### 8.2 What Stage B should and should not do with these records

**Do:**
1. Use `Compact/` plus `git ls-files` as the primary reconciler of "did task X ship". Statuses in legacy JSON are not authority (§2 hard rule).
2. Preserve all four legacy planning JSON files plus `Compact/`, `review/`, `Rutgers-dr/`, and `read_only.md` *on disk*. They are useful evidence for understanding the project's history and for justifying decisions in `05-stage-b-handoff.md`.
3. Record any cleanup (file moves, renames, untracking) in `07-cleanup-application.md` only; do not edit the records themselves during this phase. The legacy JSON's structure (project_context / tasks / decisions / facts / risks / external_dependencies) is itself a tracking surface — modifying it midway through Stage A would conflate evidence with output.
4. When migrating any work item into ngagent runtime state under `.git/ngagent/`, treat the legacy `record.json` task as a *spec input*, not a *status input*. Re-derive AC, dependencies, and current artifact paths from code + Compact narrative.
5. Cite `record.json.decisions` + `Patch-dr-2025-11-14.md` (not `read_only.md`) when documenting current product architecture in Stage B.

**Do not:**
1. Do **not** import any of the seven stale `todo` / `in_progress` / `in_review` statuses into ngagent without verifying against the artifact list first. In every case checked here, the legacy status under-reports completion (except for `act-004`, where it over-reports).
2. Do **not** rebuild `act-004` Discord from the subtask `artifacts` paths alone. Either recover the missing files from git history if they exist on an unmerged branch, or formally retire the Discord channel as out of scope for the current product. Stage B handoff (`05-`) should make this a named decision.
3. Do **not** treat `read_only.md` as a read-only single source of truth. Its name is misleading after the Patch-DR.
4. Do **not** modify `record.json` to "fix" the statuses during Stage A. Stage A is audit-first; fixing statuses would destroy the evidence layer that this reconciliation depends on. Any reconciled status set must live in ngagent runtime state under `.git/ngagent/`, not in the legacy file.
5. Do **not** propagate naming-drift fixes (`BetterCourseSchedulePlanner` → `Rutgers-BetterCourseSchedulePlanner`) into legacy records, Compact files, review files, or Obsidian. Those are evidence layers; rewriting them would also destroy their value. Stage B may update the *current* `package.json` and launcher strings if it deems that part of product cleanup, but that decision lives in `07-cleanup-application.md`.

### 8.3 Specific items to surface in `05-stage-b-handoff.md`

1. **`act-004-discord-notify-channel`** — three Compacts but no shipped code on `dev`. Verify whether the Discord work exists on a different branch (the remote enumerates many; see 01-inventory §7) before declaring it lost. If lost, retire the task tree explicitly.
2. **`rRevision.json` filter-rewrite-04-docs-i18n** — verify `docs/query_api_contract.md` and `docs/ui_flow_course_list.md` against the new `examCode` + meeting-day-subset semantics. Only docs-update may remain.
3. **`read_only.md` archival path** — once moved, place a one-line forwarding pointer (in `07-cleanup-application.md`) so future readers see that `read_only.md` is a v0.1 DR distillation superseded by `Patch-dr-2025-11-14.md` and `record.json.decisions`.
4. **`package.json` remote URLs** vs `git remote -v` (see §6.5) — Stage B should decide whether to update `package.json` `repository` / `bugs` / `homepage` to match the current remote name.
5. **Parent-level `record.json.artifacts`** — if `record.json` is kept for any forward purpose at all (it should not be), regenerate parent-level artifact arrays from subtask arrays + `git ls-files`. Otherwise leave them as the historical record that they are.

## 9. Constraint Compliance Check

- **Write scope:** the only file produced by this task is `.orchestrator/stage-a/03-record-reconciliation.md`. No file in `api/`, `frontend/`, `workers/`, `notifications/`, `scripts/`, `data/`, `configs/` is modified. No legacy record file (`record.json`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`, `Compact/`, `review/`, `Rutgers-dr/`, `read_only.md`, `AGENTS.md`) is modified.
- **Evidence-backed:** every status claim cites either an artifact-path check against `git ls-files`, a referenced Compact filename, a `review/` blob index, or a quoted `record.json` field. Where this report differs from a legacy claim, the legacy claim is explicitly quoted before the difference is described.
- **Authority discipline:** §2 establishes that legacy planning JSON, Compact narratives, review diffs, DR documents, reports, `read_only.md`, `AGENTS.md`, and the Obsidian vault are all evidence layers, not authorities. The §2 rule from `01-inventory.md` (gitignore / tracked / ignored / untracked / remote as evidence-only) is preserved verbatim in spirit and re-applied here.
- **No secrets exposed:** no real API keys, tokens, or passwords from `configs/mail_sender.user.json`, `.env.*`, or any other secret-risk surface are reproduced; only structural risk is described, consistent with 01-inventory §6.
- **Filesystem discipline:** the only out-of-worktree path read is `D:\Document\Obsidian\Adrian\Prompt\BetterCourseSchedulePlanner\` — an *external* personal Obsidian vault explicitly listed by the Main Agent's pinned context and in the task's relevant-files list. No content from other worktrees of the same repo, no traversal of the main checkout's root, and no `../` access to repo-internal paths outside this worktree.
- **Obsidian-vault discipline:** content from the vault is described in summary only (filenames, prompt purpose, presence/absence of references). No sensitive material is copied; no edit is proposed. The vault's `GIT-Clone.md` URL drift is recorded as evidence in §6.5, not acted upon.

## 10. Acceptance Criteria Coverage Map

| AC | Requirement | Where addressed |
| --- | --- | --- |
| AC-001 | Produce only `.orchestrator/stage-a/03-record-reconciliation.md`; do not modify product source, tests, runtime data, configs, package files, or legacy record files. | This file is the sole write. §9 reaffirms the write-scope discipline. |
| AC-002 | Consume `.orchestrator/stage-a/01-inventory.md` and preserve its rule that gitignore / tracked / ignored / untracked / remote state are evidence layers, not authorities. | §1 cites 01-inventory; §2 re-states the rule and *extends* it to legacy-record evidence layers; §7 lists errata against 01-inventory without invalidating its evidence-layer rule. |
| AC-003 | Reconcile `record.json`, `rEmail.json`, `rRevision.json`, `rSubscribe.json`, `Compact`, `review`, `reports`, `Rutgers-dr`, `read_only.md`, `AGENTS.md`, and the Obsidian prompt folder as evidence, not authority. | §3 inventories each family; §4–§5 map legacy entries to current code; §6 catalogs cross-cutting contradictions; §7 corrects 01-inventory's classification of `read_only.md`; §8.1 classifies each family. Obsidian vault is treated as external evidence in §3, §6.5, §6.7, §8.1. |
| AC-004 | Map major legacy tasks and subtasks to current files where possible and identify stale `todo` or `in_progress` statuses that conflict with implemented code or later reports. | §4 maps all of `record.json`'s ten parent tasks and thirty-one subtasks; §5 maps the three later seed initiatives. §4.13 summarises the stale/contradictory states (4 stale parents, 2 stale+blocked parents, 1 contradictory tree, 7 pre-refactor artifact lists, 4 artefacts-never-created). §5.2/§5.3 cover `rRevision.json`/`rSubscribe.json` staleness. |
| AC-005 | Classify each record family as current reference, historical evidence, stale, contradictory, or archive candidate and recommend how Stage B should use or ignore it. | §8.1 classification table covers every family; §8.2 lists Do/Don't for Stage B; §8.3 names specific items to surface in `05-stage-b-handoff.md`. |
