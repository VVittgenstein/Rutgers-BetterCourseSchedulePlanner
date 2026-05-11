# Stage A — Current Module Surface and Refactor-Candidate Map

> **Task:** `task-005` Stage A current module surface and refactor-candidate map
> **Branch:** `feature/task-005`
> **Worktree:** `Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\task-005`
> **Main checkout:** `Z:\Project\Rutgers-BetterCourseSchedulePlanner` (`dev` HEAD at the time `task-001` was merged).
> **Scope:** Audit/report only. This document is the **only** file written by this task. No product source, tests, runtime data, configs, package files, or docs outside this file are modified. `api/`, `frontend/`, `workers/`, `notifications/`, `scripts/`, `data/`, `configs/` are read-only here.

## 0. Method and Authority Rules

This report consumes [`.orchestrator/stage-a/01-inventory.md`](./01-inventory.md) as the upstream baseline and preserves its evidence-layer discipline:

- `.gitignore`, **tracked state**, **ignored state**, **untracked state**, and **remote state** are evidence layers, **not authorities**. Each can be wrong or stale.
- "Ignored" is never read as "correctly excluded"; "tracked" is never read as "currently correct"; "absent from this worktree" is never read as "absent from the project."
- Per-file claims cite the **tracked** content of `dev`/`feature/task-005` (which `task-001` already proved are aligned on the surfaces relevant here) and never assume a file's name implies its body matches the surrounding contract docs.

Evidence used in this task:
- `git ls-files <module>/` for tracked file enumeration per module (cross-checked against §3 of `01-inventory.md`).
- Direct reads of route registrars, container, server bootstrap, worker entrypoints, mail stack types/config, scripts, migrations, `data/schema.sql`, top-level `package.json`, and representative `docs/` files.
- Cross-references between code and contract docs (`docs/query_api_contract.md`, `docs/mail_*_contract.md`, `docs/open_event_spec.md`, `docs/fetch_pipeline.md`, `docs/notify_runbook.md`, `docs/subscription_model.md`).
- Spot-verifications by direct re-read of the most consequential claims before publishing them in §3–§5.

Per AC-004, this report **observes** test coverage at a high level. It does **not** add, remove, or modify tests; it does not propose specific test names. "Coverage signal" rows only describe which files exist and what they probably do not cover, never as a Stage A action.

## 1. Module Inventory at a Glance

| Module | Tracked entrypoint(s) | Test directory | Public surface kind | Build/runtime owner |
| --- | --- | --- | --- | --- |
| `api/` | `api/src/server.ts` (+ 8 route files, 2 query helpers, 1 service, 1 plugin, 1 container, 1 config, 1 fastify decorator typing) | `api/tests/` (5 test files) | HTTP (Fastify + zod) on `APP_HOST:APP_PORT` (defaults `127.0.0.1:3333`); decorates `app.container` | invoked by `scripts/oneclick_start.js` via `npm run api:start` (`tsx api/src/server.ts`) |
| `frontend/` | `frontend/src/main.tsx` → `App.tsx`; Vite root `frontend/index.html` | **none tracked** (`git ls-files frontend/` finds no `*.test.*`, no `__tests__/`) | SPA bundle (React 18 + TS, Vite 5); 12 components, 3 hooks, 6 typed API client files | `vite` dev server in oneclick; `vite build` for shipping bundle |
| `workers/` | `workers/mail_dispatcher.ts`, `workers/open_sections_poller.ts` | `workers/tests/` (2 test files) | Long-running CLI processes invoked by `scripts/oneclick_start.js`; also exported for tests | Node + tsx |
| `notifications/mail/` | `notifications/mail/types.ts`, `config.ts`, `template_loader.ts`, `template_checker.ts`, `retry_policy.ts`, `providers/sendgrid.ts` | `notifications/mail/tests/` (4 test files) | Importable library: `MailSender` interface, `ReliableMailSender`, template render/check, config loader | consumed by `workers/mail_dispatcher.ts` and (via `template_checker`) by `api/src/routes/admin.ts` |
| `scripts/` | 13 `*.ts` + 2 `*.js` + 1 `*.py` + 2 `*.sh` (per `01-inventory.md` §4.1) | **none tracked** | Mixed: data pipeline scripts (`fetch_soc_data.ts`, `migrate_db.ts`, `backfill_core_attributes.ts`, `incremental_trial.ts`), SOC probes (`soc_*.ts`, `soc_field_matrix.py`), simulation harnesses (`poller_resume_sim.ts`, `mail_e2e_sim.ts`), oneclick launcher (`oneclick_start.js`), startup helpers (`run_stack.sh`, `setup_local_env.sh`), util libraries used by the api/workers (`soc_api_client.ts`, `soc_normalizer.ts`), and `mail_templates.js` consumed by oneclick. Also contains the stray tracked `scripts/poller_checkpoint.json` flagged in `01-inventory.md` §4.6. | Node/tsx, Python (one-off matrix tool only), POSIX bash (Linux/macOS only) |
| `data/` | `data/schema.sql`, `data/migrations/00{1,2,3,4}_*.sql` | none (SQL has no tracked tests; logic tests live in `api/tests/` and `workers/tests/`) | SQLite schema; runtime DB lives at `data/fresh_local.db` (untracked, generated). Also currently contains tracked runtime junk (see `01-inventory.md` §4.6). | `scripts/migrate_db.ts` applies migrations |
| Mail/startup config + templates (`configs/`) | `configs/fetch_pipeline.{example,schema}.json`, `configs/mail_sender.{example,user}.json`, `configs/templates/email/{open-seat,verification}/{en-US,zh-CN}.{html,txt}` (12 tracked files; **`configs/mail_sender.schema.json` is NOT tracked**) | n/a | Static JSON contracts + Mustache-style email templates | consumed by api admin route, oneclick launcher, mail dispatcher |
| `docs/` | 23 files (per `01-inventory.md` §4.3) — runbooks + contract docs + SOC rate-limit JSON snapshots | n/a | Markdown / CSV / JSON; the **contract** docs (`query_api_contract.md`, `mail_sender_contract.md`, `mail_worker_contract.md`, `open_event_spec.md`, `subscription_model.md`, `fetch_pipeline.md`) are the only place that fixes external/inter-module behavior outside the code itself | versionless; no doc-gen, no link checker, no contract-vs-code lint |

**Tracked file totals consumed by this report:** 262 tracked files in the worktree (matches the `01-inventory.md` count for `feature/task-001`; this branch has only `.orchestrator/stage-a/05-module-surface-map.md` added).

## 2. Cross-Module Boot Map

`scripts/oneclick_start.js` is the single seam that wires the four runnable surfaces together. The trace below is taken from a direct read of `scripts/oneclick_start.js` (not from doc claims), so it can be used as a ground-truth reference for drift checks against `docs/oneclick.md` and `docs/deployment_playbook.md`.

```
Start-WebUI.bat (Win) | Start-WebUI.command (POSIX)
        │
        ▼
scripts/oneclick_start.js   ── reads:
        │                       configs/fetch_pipeline.local.json (created from .example.json on first run)
        │                       configs/mail_sender.user.json, .local.json   (via scripts/mail_templates.js)
        │                       data/poller_checkpoint.json                  (creates parent dir)
        │
        ├── ensureNodeVersion()             (>= MIN_NODE_MAJOR = 22)
        ├── ensureDependencies()            (npm install at root + frontend; better-sqlite3 binary probe)
        ├── prepareDatabase()               (db:migrate via npm; conditional first-run full-init fetch)
        │
        ├── spawn 'api'                     (npm run api:start  →  tsx api/src/server.ts)
        │       env: APP_PORT, APP_HOST=127.0.0.1, SQLITE_FILE=<dbPath>
        │
        ├── spawn 'frontend'                (npm run dev -- --host 127.0.0.1 --port 5174)
        │       env: VITE_API_BASE_URL=/api, VITE_API_PROXY_TARGET=http://localhost:${API_PORT}
        │
        ├── if !CSP_SKIP_POLLER:
        │     spawn 'open_sections_poller'  (npx tsx workers/open_sections_poller.ts
        │                                    --terms <auto|explicit> --sqlite <dbPath> --interval
        │                                    --checkpoint data/poller_checkpoint.json [--campuses ...])
        │
        ├── inspectMailConfig() decides:
        │     if usable: spawn 'mail_dispatcher'
        │                (npx tsx workers/mail_dispatcher.ts
        │                 --sqlite <dbPath> --mail-config <user|local>.json
        │                 --batch CSP_MAIL_BATCH (default 25)
        │                 --app-base-url <APP_BASE_URL>)
        │     else:      log a localized reason ("dryRun=true", "templates missing",
        │                "API key missing", "config invalid")
        │
        └── openBrowser(http://localhost:5174)
```

Key implications for the surface map:
- **The runtime composition is encoded only in JS**, not in `docs/oneclick.md` or `docs/deployment_playbook.md`. The launcher hardcodes the relationship between API port `3333`, frontend port `5174`, frontend proxy, and poller/mail child processes. There is no declarative service descriptor.
- **API never directly talks to workers**; the only contract between them is the shared SQLite database file path (`SQLITE_FILE`/`--sqlite`) and the tables defined by `data/migrations/*.sql`. The api admin route does, however, import directly from `notifications/mail/template_checker.ts` (see §3.1.4).
- **`data/poller_checkpoint.json` is the canonical checkpoint path used by oneclick.** `scripts/poller_checkpoint.json` (the tracked stray called out in `01-inventory.md` §4.6/§4.10) is **not referenced** by `oneclick_start.js`; this confirms the stray-copy hypothesis.
- **Mail dispatch is opt-in by data, not by flag.** Oneclick decides whether to spawn the dispatcher purely from `configs/mail_sender.user.json` / `mail_sender.local.json` shape (apiKey + dryRun + templates). This means a missing key silently disables real notifications without changing the API's `/admin/mail-config` UX.

## 3. Module-by-Module Surface Summary

For each module: tracked entrypoint(s), public contract surface, hidden coupling signals, and test-coverage observation. **Hidden coupling** means dependencies that are real but not visible in the public surface (magic strings, module-load side effects, cross-module imports that bypass the documented contract).

### 3.1 API (`api/`)

**Tracked surface:** `api/src/server.ts` bootstraps Fastify, applies the zod type provider, decorates `app.container` (built by `api/src/container.ts`), registers `plugins/requestLogging.ts`, then registers 8 route modules under the `/api` prefix (with `admin` and `notifications.local` mounted alongside per file).

**Public HTTP surface (per direct read of each route file):**

| File | Routes | Purpose |
| --- | --- | --- |
| `routes/health.ts` | `GET /api/health`, `GET /api/ready` | Liveness + readiness (checks for `REQUIRED_SCHEMA_TABLES` rows). |
| `routes/courses.ts` | `GET /api/courses` | Course search; calls `queries/course_search.ts`. Uses `paginationSchema(100, 20)` + many filter schemas from `sharedSchemas.ts`. |
| `routes/sections.ts` | `GET /api/sections` | **STUB.** Full zod schema (`paginationSchema(200, 50)` + 17 query params with cross-validation), but handler body returns `data: []`, `total: 0` (`sections.ts:88–114`). Verified by direct read in §1 prep work for this task. |
| `routes/filters.ts` | `GET /api/filters` | Dictionary of valid terms/campuses/subjects/core codes/delivery; calls `queries/filters.ts`. |
| `routes/subscriptions.ts` | `POST /api/subscribe`, `POST /api/unsubscribe`, `GET /api/subscriptions` | Subscription lifecycle. Defines its own zod schemas. Inline SQL via `db.transaction()` (no queries layer). |
| `routes/fetch.ts` | `GET /api/fetch`, `POST /api/fetch` | Triggers/polls a data fetch job spawned by `services/fetchRunner.ts`. Uses module-level mutable `activeJob` state — see §3.1.4. |
| `routes/admin.ts` | `GET /admin/mail-config`, `PUT /admin/mail-config` | Read/write the mail-sender config file (sanitizes apiKey on read). Imports `template_checker` from `notifications/mail/`. Reads `process.env.MAIL_CONFIG_DIR` in handler body. |
| `routes/notifications.local.ts` | `POST /api/notifications/local/claim` | Lets a browser-local "sound notification" client claim pending notifications for its device hash. Inline SQL with a `db.transaction()` over four prepared statements. |

**Database access pattern:** `request.server.container.getDb()` returns a per-process singleton `better-sqlite3` `Database` lazily opened from `AppConfig.sqliteFile`. Routes call it directly and use either inline SQL (subscriptions, health, notifications.local, admin) or the `queries/` layer (courses, filters). No repository layer; no ORM. Two routes nest `db.transaction(...)` themselves; the rest are autocommit per `prepare().run()`.

**Shared schemas (`sharedSchemas.ts`):** exports `API_VERSION = 'v1'`, `paginationSchema`, `sortDirectionSchema`, `optionalBooleanParam`, `minutesParam`, `optionalMinutesParam`, `stringOrArrayParam`, `enumArrayParam`. Used by `courses.ts` and `sections.ts` in nearly identical ways (sections duplicates the courses-style pagination/filter shape; see §3.1.4). `filters.ts`, `fetch.ts`, and `health.ts` use only `API_VERSION`. `subscriptions.ts`, `admin.ts`, and `notifications.local.ts` define their own schemas.

**Container shape (`container.ts`):** `AppContainer = { config: AppConfig; getDb(): Database.Database; close(): void }`. No other services are injected. `requestLoggingPlugin` is a hook-style plugin (`onRequest`/`onResponse`) and is not part of the container.

#### 3.1.1 Hidden coupling — internal to api/

- **Stub route in a real-looking schema.** `routes/sections.ts` ships a fully-validated query schema that promises pagination, filtering, sorting, and a meeting-time window — but the handler returns an empty result set. Any frontend feature that "checks the box" by hitting `/api/sections` will appear to work in dev/test (200 OK, valid schema) and silently return nothing in production.
- **Duplicated filter surface across `courses.ts` and `sections.ts`.** Both define very similar `paginationSchema + campus/subject/delivery/meeting*` zod blocks. Any future filter change has to be touched in two places.
- **`fetchRunner.ts` carries module-level mutable state.** `services/fetchRunner.ts` keeps `activeJob` at module scope; `startFetchJob` and `getActiveFetchJob` read/write it. Two concurrent `POST /api/fetch` calls race here — there is no lock and no rejection-when-busy on the runner side beyond what the route enforces. This is a structural concurrency hazard.
- **Admin and subscriptions endpoints are auth-free.** `GET /api/subscriptions` returns active subscriptions without a user context; `GET/PUT /admin/mail-config` has no guard. The current launcher binds to `127.0.0.1` (oneclick `APP_HOST='127.0.0.1'`), so this is local-only by deployment, not by code. Anything that publishes the API on a public interface inherits an unauthenticated admin surface.
- **`process.env` reads inside route handlers.** `admin.ts` reads `MAIL_CONFIG_DIR` at request time and defaults it to `'configs'` — i.e., the config file path is not in `AppConfig`. This makes the route's behavior depend on env at every call.
- **Hardcoded magic strings for season/term codes** in `routes/fetch.ts` (`SEASON_CODES`, `TERM_ALIASES`); these duplicate logic that also lives in scripts (`scripts/soc_api_client.ts::decodeSemester`) and in the workers, so term normalization rules can drift between layers.

#### 3.1.2 Hidden coupling — across module boundaries

- **`api/src/routes/admin.ts` imports `notifications/mail/template_checker.ts` directly** (see §3.4). The admin route therefore does live disk reads against `configs/templates/email/**` on every config write/read. This is the **only** non-test code path where `api/` reaches into `notifications/mail/`.
- **Schema validation is the only real interface between the API and the DB.** Tables are referenced by string literals in route bodies (`SELECT term_id FROM terms`, `campus_code FROM campuses`, etc.). The "contract" between `api/` and `data/migrations/*` is enforced only by `routes/health.ts::REQUIRED_SCHEMA_TABLES` and by sqlite blowing up at runtime if a column is missing.

#### 3.1.3 Test coverage signal — api/

Tracked in `api/tests/`: `admin_mail_config.test.ts`, `course_search.test.ts`, `health.test.ts`, `notifications.local.test.ts`, `subscriptions.test.ts`.

Observed gaps at a high level (no tests are added or proposed; this is descriptive only):
- No `sections.test.ts`. Because the route is a stub today, the absence of a test means the stub will not be flagged by CI when sections-table data finally arrives.
- No `filters.test.ts`. The `queries/filters.ts` helper is exercised only indirectly via course search.
- No `fetch.test.ts`. The `fetchRunner` concurrency story (§3.1.1) is unguarded.
- The query-layer tests (`course_search.test.ts`) exercise the queries directly, not through the Fastify handler — schema validation and error mapping are observed at the route level only via the other tests.

#### 3.1.4 Refactor surface signals — api/

(See §5 for the priority table.)

- `routes/sections.ts` stub vs. shipped schema (high evidence: direct read).
- `services/fetchRunner.ts` module-level `activeJob` (high evidence: direct read).
- `routes/admin.ts` reading env mid-handler and importing `template_checker` synchronously across module boundary.
- Duplicated zod surface between `routes/courses.ts` and `routes/sections.ts`.

### 3.2 Frontend (`frontend/`)

**Tracked surface:** `frontend/index.html` → `frontend/src/main.tsx` → `frontend/src/App.tsx`. Vite + React 18 + TS. No external state library: i18next provides translation, `react-window` is in `package.json` but the components actually rendering rows in `CourseList.tsx` use plain React (no virtualization is mounted in `App.tsx`'s tree as currently wired).

**Verified by direct read:** `frontend/src/App.tsx` mounts `LanguageSwitcher`, `DataFetchCard`, `FilterPanel`, `SubscriptionCenter`, `CourseList`, `MailSettingsPanel`, `SubscriptionManager`. No Context provider, no Redux/Zustand, no error boundary, no router. App-level state is one `useState<CourseFilterState>` plus the dictionaries returned by `useFiltersDictionary()` and `useCourseQuery()`.

**Public surface (from a code/runtime perspective):**

- **What the bundle renders:** the SPA expects to be served behind a proxy that exposes the API as `/api/*` (`VITE_API_BASE_URL=/api`, set by oneclick).
- **What the bundle calls:** `frontend/src/api/{client,admin,filters,fetchJobs,notifications,subscriptions,types}.ts`. All endpoints used are HTTP only; there is no websocket or SSE.
- **What the bundle persists:** `localStorage` keys prefixed `bcsp:` — locale, `bcsp:localSoundDeviceId`, `bcsp:localSoundEnabled`, `bcsp:subscriptionContact`, `bcsp:subscriptionContactType`.

**Endpoints hit (per the api client modules and hooks):**

| Frontend caller | Backend route |
| --- | --- |
| `useCourseQuery` / `useFiltersDictionary` | `/api/courses`, `/api/filters` |
| `DataFetchCard` (+ hooks) | `/api/fetch` (GET + POST) |
| `MailSettingsPanel` | `/admin/mail-config` (GET + PUT) |
| `SubscribeButton`, `SubscriptionManager` | `/api/subscribe`, `/api/unsubscribe`, `/api/subscriptions` |
| `useLocalSoundNotifications` | `/api/notifications/local/claim` |

Notably, **no current frontend caller hits `/api/sections`** — which is consistent with that route being a stub (§3.1).

#### 3.2.1 Hidden coupling — internal to frontend/

- **`useCourseQuery.ts` is a tall hook.** Per a direct content sweep, it combines an in-memory debounced cache, the URL parameter builder, the row transformer, and meeting/section filter normalization. That's at least four responsibilities living together and means any change to how the backend shapes a course row touches the same file as caching behavior. (Refactor candidate, blast radius local to the hook + its consumers.)
- **Type duplication between `api/` and `frontend/`.** Course/section row shapes are re-declared in `frontend/src/api/types.ts` and again transformed in `useCourseQuery.ts`. The backend's `sharedSchemas.ts` does not export TypeScript types that the frontend can consume — the two layers carry parallel definitions.
- **Magic strings concentrated in two places.** Endpoints are written as string literals in each `frontend/src/api/*.ts` file (no shared route constants); localStorage keys are inline in each consumer.
- **Hardcoded values that look like config.** `FALLBACK_CAMPUSES` in `DataFetchCard.tsx`, `LEVEL_LABELS` and `DELIVERY_LABELS` in the filters layer, `sectionsLimit: 200` and the include list `['sectionsSummary', 'subjects', 'sections']` in `state/courseFilters.ts`. These define the user-visible shape of the search UI but are not in `configs/`.
- **The dev playground (`src/dev/ComponentPlayground.tsx`, `src/dev/mockData.ts`) is not imported by product code.** Verified: `grep "from ['\"]./dev/" frontend/src` returns no matches. Vite will still include it if anything ever imports it; today nothing does, so it's effectively dead from a routing perspective but still part of the source tree (`01-inventory.md` §4.10 already flagged this as a verify item).

#### 3.2.2 Hidden coupling — across module boundaries

- **The proxy contract is the only thing that aligns `/api` between frontend and backend.** It is set by `VITE_API_PROXY_TARGET` and `VITE_API_BASE_URL` in `oneclick_start.js`, not in any shared config or doc. Changing the API port in oneclick is the only place this contract is expressed.
- **No frontend code references `workers/` or `notifications/`** — the only path is the HTTP API. This is a healthy boundary.

#### 3.2.3 Test coverage signal — frontend/

`git ls-files frontend/` produces **zero** test files (no `*.test.*`, no `*.spec.*`, no `__tests__/`). `frontend/package.json` has no `test` script. There is no Vitest/Playwright/Jest configured. The entire UI surface — including the localStorage state machine and the dev playground that ships in the source tree — has no automated coverage.

#### 3.2.4 Refactor surface signals — frontend/

- `useCourseQuery.ts` size/responsibility split.
- Centralizing API endpoint strings and localStorage keys (single source of truth shared with `api/`).
- Dev playground exclusion or relocation (Vite build inclusion verify).
- Optional: error boundary at App level.

### 3.3 Workers (`workers/`)

**Tracked surface:** two single-file long-running processes.

`workers/mail_dispatcher.ts`:
- CLI entry; also exports `MailDispatcher` class for tests.
- CLI args (per oneclick spawn + tests): `--sqlite`, `--mail-config`, `--batch`, `--app-base-url`. Internal defaults: `batchSize=25`, `lockTtlSeconds=120`, `delivery.maxAttempts=3`, `delivery.retryScheduleMs=[0,2000,7000]`, `idleDelayMs=2000`.
- Reads from `open_event_notifications`, `subscriptions`, `open_events`, `sections`, `courses`, `section_meetings`; writes `subscription_events` and updates `open_event_notifications` / `subscriptions.last_known_section_status`.
- Calls `ReliableMailSender.send()` (from `notifications/mail/retry_policy.ts`) wrapping `SendGridMailSender` from `notifications/mail/providers/sendgrid.ts`.

`workers/open_sections_poller.ts`:
- CLI entry; also exports helpers for tests (`parseArgs`, `discoverSubscriptionTargets`, `syncTargetLoops`).
- CLI args (per oneclick spawn + tests): `--terms` (or `auto`), `--campuses`, `--sqlite`, `--interval`, `--checkpoint`, plus internal defaults `refreshIntervalMs=5min`, `jitter=0.3`, `concurrency=3`, `subscriptionChunkSize=200`, `missThreshold=2`, optional `metricsPort` Prometheus endpoint.
- Reads/writes `sections`, `subscriptions`, `open_events`, `open_event_notifications`, `section_status_events`, `open_section_snapshots`; calls `performProbe()` from `scripts/soc_api_client.ts` against `https://classes.rutgers.edu/soc/api`.

**Hidden coupling — internal to workers/:**
- **Coordinate-only coupling.** The two workers never import each other. They share state only via the database tables defined in `data/migrations/003_open_events.sql` (and the rest of the schema). The contract between them is implicit: poller inserts `open_event_notifications`; dispatcher consumes them. No TypeScript interface fixes this contract.
- **Locking is in-table, untyped.** `subscriptions.last_known_section_status`, lock columns on `open_event_notifications`, and a `lockTtlSeconds=120` constant in `mail_dispatcher.ts` together implement a lease pattern, but neither the schema nor a contract doc names this protocol explicitly.
- **Duplicate constants.** `sleep()` is redefined in both workers; `ACTIVE_STATUSES`, dedupe-bucket constants (`3 * 60 * 1000` ms in both `OPEN_REMINDER_INTERVAL_MS` and the dedupe-key bucket), and locale-fallback logic appear in multiple files.
- **Worker file size.** `open_sections_poller.ts` is the largest file in the codebase (low-thousands of lines on direct count); `mail_dispatcher.ts` is several hundred lines. Both bundle multiple responsibilities (CLI parsing, DB I/O, business logic, metrics, retry).

**Hidden coupling — across module boundaries:**
- **Workers import `notifications/mail/`** (provider, retry policy, template loader, config loader, types). That is the intended boundary.
- **Workers import `scripts/soc_api_client.ts`** (the poller, for `performProbe`). This means the SOC HTTP client lives in `scripts/` but is also part of the worker runtime; refactoring its location requires updating worker imports.
- **Workers do not import `api/` or `frontend/`.** Healthy.

**Test coverage signal — workers/:**
- `workers/tests/mail_dispatcher.test.ts` and `workers/tests/open_sections_poller.auto.test.ts` exist.
- Coverage is shallow at a high level: dispatcher tests appear to cover one success path and one retry; poller tests focus on `parseArgs`, `discoverSubscriptionTargets`, and `syncTargetLoops` plus a "skip when sections data missing" case. The actual poll loop (jitter, checkpoint hydration/persistence, miss-counter, snapshot dedup, fan-out chunking) is not directly covered by named test cases. No tests are added or proposed here; this is observation only.

### 3.4 Mail / Notifications (`notifications/mail/`)

**Tracked surface (per `git ls-files notifications/`):** `types.ts`, `config.ts`, `template_loader.ts`, `template_checker.ts`, `retry_policy.ts`, `providers/sendgrid.ts`, plus four tracked test files.

**Public surface:**

- `types.ts`: `MailMessage`, `MailSender` interface (`send(message, options?) => Promise<SendResult>`), `SendResult` (status `sent|retryable|failed`), `SendErrorCode` union, `SendOptions`, `MailSenderConfig` / `ResolvedMailSenderConfig`, `MailSendAttempt`, `SendWithRetryResult`, `TemplateDefinition`, `RateLimitConfig`, `RetryPolicyConfig`. Includes a typed `SMTPConfig` shape even though no SMTP provider is implemented.
- `config.ts`: `loadMailSenderConfig(path)`, `resolveMailSenderConfig(raw, env, baseDir)`. Resolves env-referenced API keys (e.g. `apiKeyEnv: "SENDGRID_API_KEY"`), validates locales/template variables, applies defaults.
- `template_loader.ts`: `renderMessageContent(config, message)` (lazy/on-send); Mustache-style `{{var}}` interpolation; throws typed `TemplateError`.
- `template_checker.ts`: `collectTemplateIssues(config, baseDir)` — startup validation that walks the template tree and reports missing/invalid files; consumed by `api/src/routes/admin.ts` and by tests.
- `retry_policy.ts`: `ReliableMailSender` (wraps any `MailSender`; adds token-bucket rate limit + exponential backoff with jitter); `TokenBucketRateLimiter`.
- `providers/sendgrid.ts`: `SendGridMailSender implements MailSender`; HTTP POST to `/v3/mail/send`; maps 4xx/429/5xx/timeout to `SendErrorCode`.

**Public contract — the interface other modules consume:**
- `mail_dispatcher.ts` instantiates `new SendGridMailSender(resolvedConfig)` and wraps it in `new ReliableMailSender(...)`. The provider is hardcoded by `if (config.provider === 'sendgrid') { ... }`; there is no provider registry. SMTP types exist but there is no SMTP provider implementation.
- `api/src/routes/admin.ts` uses `template_checker` to surface "templates missing" issues in the `/admin/mail-config` response.

**Hidden coupling — internal to notifications/mail/:**
- **Double validation of templates.** `config.ts` validates template locales and required variables at config load; `template_loader.ts` re-validates and re-reads at send time. Defensive but duplicative — a refactor that consolidates this needs to think about API admin's startup-vs-runtime expectations.
- **Locale fallback is permissive.** `chooseLocale()` in the dispatcher picks the first available locale if preferred/fallback are both missing. Safe but can mask misconfig (a real refactor should decide whether to fail loudly).
- **Retry/rate-limit defaults live in two places.** `notifications/mail/config.ts` carries defaults; `workers/mail_dispatcher.ts` overrides some of them with CLI defaults. Either source can be authoritative if the other forgets a field.

**Hidden coupling — across module boundaries:**
- `notifications/mail/` does **not** import `api/`, `workers/`, or `scripts/`. Healthy library shape.
- The api admin route imports `template_checker` — see §3.1.2.
- The dispatcher imports from `notifications/mail/`; this is the intended direction.

**Test coverage signal — notifications/mail/:**
Tests cover config loading + env resolution, SendGrid provider payloads (including rate limit/timeout cases), retry policy (incl. token bucket), and `template_checker` happy/missing paths. Direct file reads via `template_loader.ts` are not covered by a dedicated test file; HTML escaping and locale-fallback edge cases in the loader are inferred via the dispatcher tests but not isolated. The schema-vs-data assumption in `configs/mail_sender.example.json` is not directly validated by a tracked schema file (see "missing schema" note below).

**Missing schema artifact:** `git ls-files configs/` does not list `configs/mail_sender.schema.json`, even though the analogous `configs/fetch_pipeline.schema.json` is tracked. `01-inventory.md` §4.7 already flagged `configs/mail_sender.user.json` as a tracked-while-ignored anomaly; this report adds the observation that the **mail config has no companion schema file**, so the only validation is the TypeScript zod/runtime in `notifications/mail/config.ts`.

### 3.5 Data Pipeline + Schema (`data/`, `scripts/`)

**Tracked surface:** `data/schema.sql` plus migrations `001_init_schema.sql`, `002_relax_section_index_scope.sql`, `003_open_events.sql`, `004_course_campus_locations.sql`. `scripts/migrate_db.ts` applies migrations and tracks them in `schema_migrations` with checksums.

**Tables (high level, from `data/schema.sql`):** reference (`terms`, `campuses`, `subjects`, `instructors`); course-side (`courses`, `course_core_attributes`, `course_campus_locations`); section-side (`sections`, `section_instructors`, `section_meetings`, `section_populations`, `section_crosslistings`, `section_status_events`); subscription-side (`subscriptions`, `subscription_events`); open-events (`open_section_snapshots`, `open_events`, `open_event_notifications`); FTS (`course_search_fts` virtual, porter tokenizer).

**Pipeline orchestration (from `scripts/fetch_soc_data.ts` + `scripts/soc_api_client.ts` + `configs/fetch_pipeline.example.json`):**
- `fetch_soc_data.ts` is the only place that runs the SOC ingestion; called via `npm run data:fetch`. Reads `configs/fetch_pipeline.local.json` (created from `.example.json` by oneclick) for targets, throttling, retry policy.
- `soc_api_client.ts` is the HTTP layer (`courses.json`, `openSections.json`). Exports `decodeSemester`, `performProbe`, `SOCRequestError`.
- `soc_normalizer.ts` maps raw SOC payloads to schema columns.
- `backfill_core_attributes.ts` derives `course_core_attributes` from already-loaded `courses.core_json` and sets `has_core_attribute` — must run after `data:fetch` for that column to be current.
- `incremental_trial.ts` is a measurement harness, not a runtime path; it dry-runs a sample of the same pipeline and diffs against local snapshots.

**Hidden coupling:**
- **Rate limiting is config-driven only.** `configs/fetch_pipeline.{example,schema}.json` is where intervals/concurrency/retry live; there is no client-side enforcement in `soc_api_client.ts` itself. `scripts/soc_rate_limit.ts` is a *measurement* script that produces `docs/soc_rate_limit.*.json` snapshots used to *tune* the config. The poller imports `performProbe` and inherits no rate limit unless it constructs one itself.
- **The poller bypasses `fetch_soc_data.ts` and goes direct to the SOC client.** `workers/open_sections_poller.ts` calls `performProbe` for openSections directly; it does not reuse the pipeline's `targets` config. Rate-limit policy for the poller is therefore the poller's own jitter/interval, not the pipeline config.
- **`scripts/soc_field_matrix.py` is the only Python file in the repo** (`docs/soc_field_matrix.csv` is its output). It is not referenced from any TypeScript module and is a one-off audit tool.
- **Migrations vs `schema.sql`.** `schema.sql` is the canonical final state; migrations are cumulative. The two are not auto-checked against each other; a divergence (e.g., adding a column in a migration without updating `schema.sql`) would not be flagged by CI today. The `schema_migrations` table provides idempotency, but there is no test that verifies "apply all migrations to an empty DB ⇒ identical schema to `data/schema.sql`."

**Test coverage signal — data/, scripts/:**
- No `data/` or `scripts/` test directory tracked.
- Risky surfaces without tests at this layer (descriptive only, no action proposed): migration checksum drift in `migrate_db.ts`, malformed payload handling in `soc_normalizer.ts`, dedupe-key computation in `incremental_trial.ts`, and the simulation harnesses themselves (`poller_resume_sim.ts`, `mail_e2e_sim.ts`) which are never run by CI.

### 3.6 Startup / Ops Scripts (`scripts/`, root launchers, `configs/`)

**Tracked entrypoints:**
- `Start-WebUI.bat`, `Start-WebUI.command` → shell into `node scripts/oneclick_start.js`.
- `scripts/oneclick_start.js` is the orchestrator described in §2 above.
- `scripts/run_stack.sh` and `scripts/setup_local_env.sh` provide POSIX-only equivalents for bash users.
- `scripts/mail_templates.js` exposes `evaluateMailConfig` and `summarizeIssues`, consumed by `oneclick_start.js` to decide whether the dispatcher should be spawned.

**Hidden coupling:**
- **Two parallel boot paths.** `Start-WebUI.*` → `oneclick_start.js` (cross-platform) and `run_stack.sh`/`setup_local_env.sh` (POSIX). The two paths can drift: oneclick handles Windows-specific better-sqlite3 rebuilds, mail config detection, and per-platform DB path sanitization; the bash scripts do not. Windows users effectively must use oneclick; Linux/macOS users have both.
- **Oneclick knows about every runtime tier.** `oneclick_start.js` spawns API, frontend, poller, and (conditionally) mail dispatcher. It also reads/writes `configs/fetch_pipeline.local.json`, decides DB path, and reads two mail config files. There is no service registry — the composition is a literal `spawn(...)` per child in `main()`.
- **Mail config evaluation is JS, not the TS notifications stack.** `scripts/mail_templates.js` is a separate evaluator parallel to `notifications/mail/config.ts` — it has to keep up with the same template/locale rules but doesn't import them. Risk of drift between launcher-side validation and worker-side validation.
- **`scripts/poller_checkpoint.json` is tracked but unused by oneclick.** `oneclick_start.js` writes the checkpoint at `data/poller_checkpoint.json`. The `scripts/` copy is leftover (`01-inventory.md` §4.6 / §4.10) — confirmed here by a direct read of the launcher.
- **Single point of operational truth is `package.json` scripts.** `data:fetch`, `data:backfill-core`, `db:migrate`, `data:incremental-trial`, `api:start`, `api:dev`, `i18n:check`, `soc:*` are all there. No `test` script is wired (the `test` script just prints an error). `frontend/package.json` has only `dev/build/preview`.

**Test coverage signal — scripts/:** none tracked. The launcher itself, the SOC normalizer, and the simulation harnesses are not covered.

### 3.7 Docs (`docs/`, `.orchestrator/`)

**Tracked surface:** 23 files in `docs/` (per `01-inventory.md` §4.3). Five sub-shapes:
- **Contract docs:** `query_api_contract.md`, `mail_sender_contract.md`, `mail_sender_usage.md`, `mail_worker_contract.md`, `open_event_spec.md`, `subscription_model.md` — these are the only place that fixes external/cross-module behavior outside the code.
- **Runbooks:** `quickstart.md`, `oneclick.md`, `data_load_runbook.md`, `data_refresh_strategy.md`, `deployment_playbook.md`, `notify_runbook.md`.
- **Pipeline / model docs:** `fetch_pipeline.md`, `local_data_model.md`.
- **SOC measurement evidence:** `soc_api_notes.md`, `soc_rate_limit.md`, `soc_field_matrix.csv`, plus four `soc_rate_limit*.json` snapshots.
- **UI / i18n:** `ui_flow_course_list.md`, `i18n_key_map.md`.

**Hidden coupling — docs vs. code:**
- **No doc-versioning or lint.** Nothing checks that `query_api_contract.md`'s route list matches `api/src/server.ts`'s registered route set. The `/api/sections` route, for example, is a stub in code but is presumably documented as if functional (verify in Stage B).
- **Contract docs are duplicated by code.** `mail_sender_contract.md` describes `MailSender`/`MailMessage` shapes; `notifications/mail/types.ts` defines them. If one changes without the other, only a careful reviewer catches it.
- **No subscription-UI doc, no mail-settings-UI doc, no component playbook.** `docs/ui_flow_course_list.md` is the only UI flow document. Subscription, mail settings, and the in-tree dev playground have no companion docs.
- **`docs/i18n_key_map.md` is the spec for `scripts/i18n_missing_check.ts`.** The script enforces key parity between `en-US` and `zh-CN`; the doc is the human-readable description.
- **SOC rate-limit JSON snapshots in `docs/`** are measurement artifacts, not docs. They live in `docs/` because `scripts/soc_rate_limit.ts` writes there; that placement is a small layering smell (data in docs) — flag, not block.

**Test coverage signal — docs/:** n/a. There is no link checker, no markdown lint, no doc-vs-code consistency check tracked in `package.json` scripts.

## 4. Contract-vs-Code Drift (Read-Only Spot Checks)

This section records likely divergences between contract docs and code, based on the direct reads done for §3. It does not fix them — it lists them so Stage B can pick them up in order. Where a "verify in Stage B" tag appears, the divergence is plausible but not proven; the report respects the evidence-layer rule from `01-inventory.md` §2 (treat the doc as evidence about intended behavior, not authority).

| Area | Doc evidence | Code evidence | Likely drift |
| --- | --- | --- | --- |
| Sections API | `docs/query_api_contract.md` discusses `/api/sections` as a real endpoint with filters and pagination. | `routes/sections.ts` handler returns empty data and `total: 0` regardless of input. | **High-confidence drift**: doc promises behavior the code does not implement. Stage B must either implement or document as deferred. |
| Mail provider set | `docs/mail_sender_contract.md` describes a `MailSender` interface; `notifications/mail/types.ts` defines `SMTPConfig`. | Only `SendGridMailSender` is implemented; `mail_dispatcher.ts` hardcodes the SendGrid provider switch. | **Confirm in Stage B** whether SMTP is a future feature or dead typings. |
| Mail config schema | `configs/fetch_pipeline.schema.json` is tracked; there is no analogous `configs/mail_sender.schema.json`. | `notifications/mail/config.ts` is the only validator. | Drift between two analogous config families (one schema-described, one TS-only). Decide one canonical pattern in Stage B. |
| Poller observability | `docs/notify_runbook.md` covers the notification path. | `open_sections_poller.ts` optionally exposes a `/metrics` Prometheus endpoint; the JSON checkpoint format has a `version` field but no documented upgrade path. | Stage B should document the metrics names and the checkpoint v1 format (or, conversely, decide to drop one). |
| Launcher composition | `docs/oneclick.md` + `docs/deployment_playbook.md` describe the start flow. | `scripts/oneclick_start.js` encodes the exact child process matrix, ports, env vars, and mail-decision logic. | Stage B should add a short "what oneclick actually spawns" appendix or a generated diagram; today the runtime composition exists only in code. |
| Frontend ↔ API types | `api/src/routes/sharedSchemas.ts` and `notifications/mail/types.ts` are TypeScript. | `frontend/src/api/types.ts` re-declares the relevant row shapes. | Drift waiting to happen; mitigated only by code review. Stage B may consider a shared package or a `tsc --build` cross-reference. |

## 5. Refactor-Candidate Table (Stage B Entry Conditions)

Priorities are relative within Stage A scope only. **Evidence** cites the §3 sub-section that established the finding (and where applicable a file:line range; line numbers come from direct reads done for this task). **Blast radius** describes which other modules a refactor in this area touches today. **Stage B sequencing** is a suggested ordering, not a prescription — Stage B is free to reorder once it has its own plan.

| # | Candidate | Evidence | Priority | Blast radius | Suggested Stage B sequence | Why this sequence |
| --- | --- | --- | --- | --- | --- | --- |
| R-01 | Reconcile `/api/sections` stub against `docs/query_api_contract.md` | §3.1, `routes/sections.ts:14–116`, §4 | **High** | api/, docs/, (eventually) frontend/ if/when frontend starts calling it | **First** | Doc/code disagreement on a real endpoint; a Stage B branch that pretends sections works will silently regress. Pin behavior either by implementing or by demoting in docs before any downstream work touches the search surface. |
| R-02 | Make `fetchRunner.activeJob` concurrency-safe | §3.1.1, `services/fetchRunner.ts` (module-level mutable state) | **High** | api/ only at runtime; tests in `api/tests/` | **First** (alongside R-01) | A live race in production-shaped code (`POST /api/fetch` twice). Cheap to contain to the api module. Doing this early prevents Stage B from having to remove a half-built workaround later. |
| R-03 | Add an auth gate (or explicit "loopback-only" declaration) for `/admin/*` and `GET /api/subscriptions` | §3.1.1, `routes/admin.ts`, `routes/subscriptions.ts` | **High** | api/, possibly frontend/ (MailSettingsPanel), docs/ | **Second** | Today it's safe because oneclick binds to 127.0.0.1; the next packaging that doesn't (a docker compose, a cloud run) inherits an open admin endpoint. Decide before any deployment change. |
| R-04 | Lift environment + config reads out of route handlers (`admin.ts` `MAIL_CONFIG_DIR`) into `AppConfig` | §3.1.1, `routes/admin.ts:216` | Medium | api/, configs/ (semantic only) | Second | Sets up R-03 well (auth + config can be wired through the container at once). Small change. |
| R-05 | Stop spawning the SOC poller against `scripts/soc_api_client.ts`; either lift that into `notifications/`-style library or into a `services/` namespace shared by api/workers | §3.5, §3.3 | Medium | workers/, scripts/, tsconfig | Second/Third | Aligns the import direction with the existing `notifications/mail/` pattern. Touches packaging but not behavior. Doing this before R-08 makes worker tests easier to wire. |
| R-06 | Centralize mail/template config validation in **one** path (`notifications/mail/config.ts`) and have `scripts/mail_templates.js` import or re-export instead of re-implementing | §3.4, §3.6 | Medium | scripts/, notifications/mail/, oneclick boot decision | Third | Two validators are how silent drift starts. Best done after R-04 lifts admin's env reads, so the validator gets one caller per surface. |
| R-07 | Unify duplicated zod surface between `routes/courses.ts` and `routes/sections.ts` (or, post-R-01, fold sections into courses if sections-as-stub is the chosen state) | §3.1.4 | Medium | api/ only | Third | Order matters: R-01 decides whether sections is a real route. Then deduping is mechanical. |
| R-08 | Split `useCourseQuery.ts` into cache hook + transformer + query builder; consider centralizing endpoint strings and `bcsp:*` localStorage keys | §3.2.1 | Medium | frontend/ only | Third/Fourth | High touch, low cross-module reach. Defer until R-01 stabilizes the courses/sections response shape. |
| R-09 | Add a schema file for `configs/mail_sender.*` to mirror `configs/fetch_pipeline.schema.json` | §3.4 | Medium | configs/, notifications/mail/, admin docs | Fourth | Pair this with R-06 so the schema and the resolver are co-designed. |
| R-10 | Untrack runtime artifacts and the secret-risk surface flagged in `01-inventory.md` §4.6 / §4.7 (`data/courses.sqlite-{shm,wal}`, `data/poller_checkpoint.json`, `data/runtime/*`, `scripts/poller_checkpoint.json`, `configs/mail_sender.user.json`) | `01-inventory.md` §4.6/§4.7, this report §3.6 (oneclick uses `data/poller_checkpoint.json` only) | High (security/repo hygiene), but **belongs to Stage A task 07-cleanup-application**, not Stage B | data/, configs/, scripts/, ignore rules | **Stage A cleanup task, not Stage B** | Listed here so Stage B can assume these are gone by the time it starts. Already scoped to `07-cleanup-application.md`. |
| R-11 | Document checkpoint-v1 format and mail dispatcher's lease/lock protocol in a shared spec under `docs/` | §3.3, §3.7 | Low | docs/ only | Late Stage B (or earliest Stage C) | Doc-only; can run in parallel with any other R-* item. |
| R-12 | Centralize SOC `decodeSemester`/season-code logic (currently duplicated between `routes/fetch.ts` and `scripts/soc_api_client.ts` and re-encoded by `oneclick_start.js`'s default `'12024'`) | §3.1.1, §3.5, §3.6 | Low | api/, scripts/, oneclick | Late Stage B | Small, isolated. Wait until R-05 has chosen a home for the SOC client. |
| R-13 | Decide the fate of `frontend/src/dev/*` (move to a non-bundled location, gate via a Vite env, or delete) and `scripts/soc_field_matrix.py` (move to an audit area, since it has no runtime consumer) | §3.2.1, §3.5 | Low | frontend/, scripts/ | Anytime | These are dead-from-routing or one-off-audit code. Cheap to move; mostly a cleanliness signal. |

**Sequencing summary** (what Stage B should plan to do first vs. later):
1. **Stabilize current contracts** (R-01 sections, R-02 fetchRunner race, R-03 admin/subscriptions auth or loopback declaration).
2. **Disentangle cross-module imports** (R-04 admin env, R-05 SOC client placement, R-06 mail config single-source).
3. **Deduplicate within boundaries** (R-07 courses/sections schemas, R-08 frontend hook split, R-09 mail config schema).
4. **Docs + cleanup follow-ups** (R-11 docs, R-12 season-code centralization, R-13 dead-code relocation).

R-10 is explicitly **not** Stage B's job; it is the Stage A cleanup task. The list above includes it only so Stage B inherits a known-clean baseline.

## 6. Constraint Compliance Check

- **AC-001 — Output discipline.** The only file written by this task is `.orchestrator/stage-a/05-module-surface-map.md`. No product source, tests, runtime data, configs, package files, or documentation outside this report is modified. `api/`, `frontend/`, `workers/`, `notifications/`, `scripts/`, `data/`, `configs/` are read-only here. Compliance verified before commit by `git diff` against `dev` (only this file appears).
- **AC-002 — Evidence-layer discipline preserved.** §0 restates the `01-inventory.md` §2 rule that `.gitignore`, tracked, ignored, untracked, and remote state are evidence, not authority. The "ignored is not endorsement" rule is reused inline whenever this report cites an ignore match (e.g. §3.4 mail config schema absence; §3.6 stray `scripts/poller_checkpoint.json` confirmed not used by oneclick). No claim assumes "ignored ⇒ correctly excluded" or "absent here ⇒ absent in the project."
- **AC-003 — All required surfaces summarized.** API §3.1, frontend §3.2, data pipeline §3.5, workers §3.3, mail §3.4, startup scripts §3.6, docs §3.7. Each section names current public contracts and likely hidden coupling.
- **AC-004 — Test coverage observed, not changed.** Each module section includes a "test coverage signal" paragraph that describes what tracked tests exist and what kinds of behavior they likely do not cover. No tests are added, removed, renamed, or proposed by name. The report does not state "Stage B should add test X"; it only flags risk areas.
- **AC-005 — Refactor-candidate table.** §5 lists 13 candidates (R-01..R-13), each with evidence pointer, priority, blast radius, suggested sequencing, and a one-line rationale for the sequence.
- **Secret discipline.** No real keys, tokens, or passwords are quoted. The only secret-risk surface mentioned (`configs/mail_sender.user.json`) is cited via `01-inventory.md` §4.7 without reproducing the value, and is explicitly assigned to the Stage A cleanup task (R-10), not Stage B.
- **Filesystem discipline.** No path outside the worktree (`Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\task-005\`) is read from disk. All cross-checkout facts come from git plumbing (which uses the shared `.git/` metadata) or from `01-inventory.md`'s pinned findings.

## 7. Hand-Off to Stage B

Stage B planning should consume this report alongside `.orchestrator/stage-a/01-inventory.md` and (when produced) `02..04` and `06`. Specifically:

- Stage B's first plan should treat **R-01, R-02, R-03** as preconditions, not items to compete with the rest of the backlog.
- The boot map in §2 is the runtime contract any Stage B refactor must preserve until explicitly renegotiated.
- The drift table in §4 is the minimum doc/code reconciliation set; the cleanup task (`07-cleanup-application.md`) and the final baseline (`06-final-baseline.md`) should both reference it before any Stage B branch lands.
- The "hidden coupling — across module boundaries" notes in §3.1.2, §3.3, §3.4, §3.6 enumerate every cross-module import path observed by this audit. Stage B should not assume additional safe seams beyond these without re-running a similar surface read.

## 8. Acceptance Criteria Coverage Map

| AC | Requirement | Where addressed |
| --- | --- | --- |
| AC-001 | Produce only `.orchestrator/stage-a/05-module-surface-map.md` and do not modify product source, tests, runtime data, configs, package files, or documentation outside the output report. | §0 (scope statement); §6 (compliance check); enforced at commit. |
| AC-002 | Consume `01-inventory.md` and preserve its rule that `.gitignore`, tracked, ignored, untracked, and remote state are evidence layers, not authorities. | §0 (method and authority rules); reused inline in §3.4 (mail config schema absence), §3.6 (stray checkpoint), §6 (compliance check). |
| AC-003 | Summarize api, frontend, data pipeline, workers, mail, startup scripts, and docs surfaces with current public contracts and likely hidden coupling. | §3.1 (api), §3.2 (frontend), §3.5 (data pipeline), §3.3 (workers), §3.4 (mail), §3.6 (startup/ops scripts), §3.7 (docs); plus §2 boot map and §4 contract drift. |
| AC-004 | Note test coverage presence and gaps at a high level without adding or modifying tests. | §3.1.3, §3.2.3, §3.3 ("Test coverage signal — workers/"), §3.4 ("Test coverage signal — notifications/mail/"), §3.5 ("Test coverage signal — data/, scripts/"), §3.6 ("Test coverage signal — scripts/"), §3.7 ("Test coverage signal — docs/"). No tests added or modified. |
| AC-005 | Produce a refactor candidate table with evidence, priority, expected blast radius, and suggested Stage B sequencing. | §5 (R-01..R-13 table with evidence pointers, priority, blast radius, sequence column, and one-line rationale per row); §7 (Stage B hand-off ordering). |
