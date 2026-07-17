# P1 Legacy Capability and Behavior Inventory

> **P1 REMEDIATION EVIDENCE - NOT A P2 FEATURE DECISION**

- Prepared: 2026-07-12
- Owner: Codex execution in the current conversation
- Scope: Deliverable A historical recovery, with A/B-shared product behavior included where the old project did not distinguish delivery form
- Controlling reopen record: `09-p1-reopen-legacy-capability-gap.md`
- Next gate: joint P1 Review

## 1. Purpose and reading rule

The first P1 candidate compressed the old product into labels such as
"course search and filtering." That was not enough. This inventory recovers
the old project's behavior at a level where P2 can later decide what to keep,
remove, redesign, or defer without first rediscovering what existed.

P1 was created because the abandoned task-015-to-025 line and a long project
pause left the User without dependable memory of the original product. This
file reconstructs the historical layer from persistent evidence. It is then
merged in the controlling Deliverable A candidate with the newer discussion
layer, including A/B delivery, shared Rust architecture, current watch/audio
rules, and mandatory UI phases. Historical recovery and newer decisions are
both required P1 inputs; neither may silently replace the other.

An inventory row is not a task and is not automatic v1 membership. It records
one source-backed product behavior, constraint, target, removed behavior,
or known drift point. P2 remains responsible for the all-and-only decision.
For a P2 `REMOVE` decision, absence means dependency-aware removal across the
dedicated UI/API/config/docs/tests/startup/package/runtime chain, not merely
hiding one visible control; shared healthy dependencies must remain intact.

Authority is chronological:

1. direct current User decisions and accepted 0A-0C constraints control when
   they expressly supersede old behavior;
2. original requirement/research records establish historical intent;
3. current code and tests establish implementation state, not User intent;
4. archived AI task narratives establish chronology only and require
   corroboration because they may describe code no longer present; and
5. unmerged branches, release archives, stubs, and orphan components are
   evidence candidates, never proof that a capability worked or must ship.

## 2. Classification vocabulary

| Classification | Meaning in P1 |
|---|---|
| `Current accepted constraint` | A later direct decision already controls this boundary. P2 must respect it unless the User reopens it. |
| `Strong historical requirement candidate` | An old product requirement explicitly asked for the behavior. P2 must adjudicate it; P1 does not silently promote it. |
| `Implemented and exercised` | Present in the current tree and covered by code or tests. This says nothing by itself about final membership. |
| `Implemented but unmounted/orphaned` | Code exists but the ordinary-user path does not reach it, or package contents are not wired into the composition root. |
| `Historical implementation removed` | Immutable Git or archive evidence shows the behavior existed and was later removed or narrowed. |
| `Known stub/drift` | Documentation, UI, route, package, or implementation disagrees or falsely suggests working behavior. |
| `Explicit historical future/non-goal` | Old sources deliberately excluded or deferred the behavior. |
| `Historical quality target` | An old measurable target that remains evidence but is not a current release claim. |

All rows marked `P2 pending` must receive an explicit P2 disposition. Rows
marked `Current boundary` are included so P2 can detect conflicts rather than
accidentally revive superseded behavior.

## 3. Evidence register

| Evidence ID | Authority and exact locator | What was inspected |
|---|---|---|
| `LC-E001` | Current accepted records: `docs/public-web-target.md`; `docs/shared-rust-architecture-decision.md`; `docs/dual-delivery-workflow.md`; and `docs/deliverable-a-windows-local-release-requirements.md`, sections 3-5 | Current A/B parity, Windows-local composition, Rust/shared-WebUI direction, watch/audio rules, ten-minute catalog refresh, phase gates, and package constraints. |
| `LC-E002` | `docs/archive/stage-a-legacy/read_only.md`, lines 14-36, 52-68, 75-120, and 218-242 | Original goal, non-goals, FR-01 through FR-08, NFR-01 through NFR-06, notification ambiguity, free-time ambiguity, and campus/level scope question. |
| `LC-E003` | `docs/archive/stage-a-legacy/Rutgers-dr/2025-11-11-dr.md`, lines 12-36 and 160-177 | Deep-research statement of advanced filtering, target users, product boundary, simple-time-filter MVP, and later conflict/ICS ideas. |
| `LC-E004` | `docs/archive/stage-a-legacy/Rutgers-dr/Patch-dr-2025-11-14.md`, lines 27-57 | Historical pivot from static/cloud assumptions to local SQLite/API while retaining multidimensional filters and week view. |
| `LC-E005` | `docs/archive/stage-a-legacy/README.md`, lines 3-9, 22-29, and 31-48 | Archived planning/Compact files are evidence-only, stale statuses cannot be imported as current state, and some narratives describe code absent from the present tree. |
| `LC-E006` | `docs/archive/stage-a-legacy/record.json`, `project_context`, tasks `T-20251113-act-001` through `T-20251113-act-011`, and their named subtasks/artifacts; companion seed records `rRevision.json`, `rSubscribe.json`, and `rEmail.json` | Historical task registry for data, filters, API, i18n, subscriptions, poller, notifications, and deployment; parent/subtask status inconsistencies were retained rather than normalized. |
| `LC-E007` | `docs/archive/stage-a-legacy/Compact/Compact-ST-20251122-filter-rewrite-01-frontend-state-ui-2025-11-23-T062853Z.md`, lines 4-19 | Final filter rewrite's retained fields, exact multi-value/day/time behavior, removed fields, active chips, URL helpers, and reported checks. |
| `LC-E008` | `docs/archive/stage-a-legacy/Compact/Compact-ST-20251122-filter-rewrite-02-api-schema-query-2025-11-23-T082002Z.md`, complete compact | API/query rewrite, field composition, meeting semantics, exam support, and tests. |
| `LC-E009` | `docs/archive/stage-a-legacy/Compact/Compact-ST-20251122-filter-rewrite-03-data-pipeline-dicts-2025-11-23-T110512Z.md`, complete compact | Dictionary/data-pipeline changes and filter-option provenance. |
| `LC-E010` | `frontend/src/state/courseFilters.ts`, lines 6-43, 45-132, and 148-319; SHA-256 `b27361945fdb3e080e3bdfdc3e24264326611172d4fa7fd646d9d7d44969ad60` | Current filter state, multi-value keys, query serialization, URL serialization/parsing, pagination, and sorting. |
| `LC-E011` | `frontend/src/components/FilterPanel.tsx`, controls at lines 248-553 and chip/reset behavior at lines 605-740 | Current mounted filter controls, search-within-subjects, active chips, group clearing, and reset behavior. |
| `LC-E012` | `docs/query_api_contract.md`, lines 9-10 and 44-80; `api/src/routes/courses.ts`, lines 15-62 and 88-119; `api/src/queries/course_search.ts`, lines 125-280 and 612-785 | Current course-query contract, validation, AND/OR composition, same-section/meeting predicates, SQL implementation, and returned section/meeting fields. |
| `LC-E013` | `api/tests/course_search.test.ts`, lines 18-110, especially lines 32-43 and 52-91 | Tests for combined meeting-day/delivery/time filters, core/open, exam, multi-campus/multi-subject, sorting, pagination, and includes. |
| `LC-E014` | Immutable pre-rewrite Git at commit `d845a09`: `frontend/src/state/courseFilters.ts` blob `aa75a6ac63a40d634980c689f156e11309490005`; `frontend/src/components/FilterPanel.tsx` blob `bf56fab86af6db1539f4f1dc2f3adf8c9b6989ce` | Removed historical filters: course number, Index, section number, instructors, explicit statuses/waitlist, meeting campus, building/room, permission, tags, and keywords. |
| `LC-E015` | `docs/ui_flow_course_list.md`, lines 1-64, 66-119, and later responsive/state sections | Planned List/Calendar workspace, URL synchronization, presets/share, section drawer, caching/cancellation, strict meeting semantics, trace/freshness, responsive layout, and reduced motion. |
| `LC-E016` | `frontend/src/App.tsx`, complete mounted component tree | Mounted ordinary-user surface: language, fetch, filters, subscriptions, mail settings, subscription manager, and course list; schedule, subscribe-button, and refresh components are not mounted. |
| `LC-E017` | `frontend/src/components/CourseList.tsx`, complete file; `frontend/src/hooks/useCourseQuery.ts`, lines 90-247 and 273-465 | Current result fields, loading/error/empty/pagination states, 200 ms debounce, cancellation, cache, section transformation, and client-side section coherence. |
| `LC-E018` | `frontend/src/components/SchedulePreview.tsx`, complete file; only caller in `frontend/src/dev/ComponentPlayground.tsx` | Implemented weekly meeting visualization with location/legend, but only in the development playground. |
| `LC-E019` | `docs/local_data_model.md`; `docs/fetch_pipeline.md`, lines 20-95; `docs/data_refresh_strategy.md`, lines 8-79; migrations and ingest scripts named there | Course/section/meeting/instructor model, SQLite source of truth, full and incremental loads, transactional staging, hashes, WAL, retry, and recovery. |
| `LC-E020` | `docs/soc_api_notes.md`, lines 76-100; `docs/soc_field_matrix.csv`; `scripts/soc_normalizer.ts`; `scripts/fetch_soc_data.ts` | Rutgers field availability, normalization, and the absence of reliable capacity/waitlist-count fields. |
| `LC-E021` | `frontend/src/components/DataFetchCard.tsx`, lines 27-235; `api/src/routes/fetch.ts`; `api/src/services/fetchRunner.ts` | Mounted manual year/season/campus fetch, job polling, status, log path, and dictionary refresh. |
| `LC-E022` | `api/src/queries/filters.ts`, lines 4-210 and 271-314; `frontend/src/hooks/useFiltersDictionary.ts`; `frontend/src/data/fallbackDictionary.ts` | Dynamic/fallback terms, campuses, campus locations, subjects, Core, levels, delivery, and instructors; backend omits exam codes while frontend falls back. |
| `LC-E023` | Commit `e770bf2`; branch `auto-refresh-tasks`; blobs `25821a71004fec16313a8cb435d3938dc801e88a`, `49c4bdbf1718fcf55a043821649ce6f4810ec497`, and `2bf755c413e192d2a84fd6801bba6ccf8d05c777` | Scheduled-fetch service/route and browser auto-refresh controls exist in branch/package history but are not registered or mounted in the inspected composition roots. |
| `LC-E024` | `api/src/routes/subscriptions.ts`, lines 1-691; `frontend/src/components/SubscriptionCenter.tsx`; `frontend/src/components/SubscriptionManager.tsx`; `docs/subscription_model.md` | Old persistent email/local-sound subscriptions, idempotent create, list/manage/unsubscribe, tokens, and remembered contact/device behavior. |
| `LC-E025` | `frontend/src/hooks/useLocalSoundNotifications.ts`, lines 9-261; `api/src/routes/notifications.local.ts`, lines 62-207; `frontend/src/components/LocalSoundToggle.tsx` | Old browser-local claim polling, 7 s interval, 15 s error backoff, WebAudio tone, toast queue, local device/enabled storage, and audio-unlock path. |
| `LC-E026` | `docs/open_event_spec.md`; `workers/open_sections_poller.ts` (complete file, SHA-256 `e7a16a74cc1ea0a05bcd9d2d9db502da9da7a0df25e00c46d1e437d89de9ad46`); poller tests and resume simulation | Old transition-driven open events, deduplication, active-subscription discovery, retry/outage handling, checkpoint, and restart behavior. |
| `LC-E027` | `frontend/src/i18n/index.ts`, lines 24-67; `frontend/i18n/messages.json`; `scripts/i18n_missing_check.ts`, lines 43-78; archived i18n compacts | English/Chinese switch, local persistence, HTML `lang` synchronization, and missing-key validation. |
| `LC-E028` | `Start-WebUI.bat`, lines 1-13; `scripts/oneclick_start.js`, lines 110-184, 270-360, and 442-550; `docs/oneclick.md` | Current launcher requires Node, may install/rebuild dependencies and require C++ Build Tools, starts multiple old processes, and opens the browser. |
| `LC-E029` | `api/src/routes/health.ts`, lines 27-81; `api/src/plugins/requestLogging.ts`; course-route query metrics; archived API-hardening compact | Health/readiness, request IDs/logging, trace IDs, query metrics, and schema checks. |
| `LC-E030` | Local release artifacts: `release/bcsp-20260121.zip` SHA-256 `f62f14d2cee0de4bd90931e37808141fb45df970e39cff7baab78e9a999a9a50`; `release/bcsp-20260121.tar.gz` SHA-256 `827d2ef1f59357780ac70a92f489a92246d5ec4bc1ede2bef2ec1cbfa63951ad`; `bcsp-20260122.zip` SHA-256 `48e976ef9b2efcbfb692f6cc119c790bf7d4d3e9a361c464e38f61a89a5afad1` | Divergent historical package layouts/content, including scheduled-refresh files that are not wired into package App/server/container roots. |
| `LC-E031` | `Z:/resume-from-main-machine/Rutgers-BetterCourseSchedulePlanner/current/02-source-inventory.md`, `05-project-files.md`, `08-current-state.md`, and `10-critical-dialogues.md` | Recovered main-machine snapshot confirms complete-local-release direction, source history, package/stub questions, and that the snapshot itself is a 20-file recovery index rather than a second product checkout. |
| `LC-E032` | Unmerged commit `5714a8f`, `.orchestrator/phase-1/01-release-surface-feature-matrix.md`; recovery record documents failed provenance/review | Broad task-015 matrix is discovery evidence only and cannot decide feature membership. |
| `LC-E033` | `api/src/routes/sections.ts`, lines 14-60 and 85-114; SHA-256 `af356d9ba9dd01ca38a975139842bc6eaf1263fde8e9087f19f0f9dfd77fdac5` | A detailed advanced section-query schema validates input, then always returns `total: 0` and `data: []`. |
| `LC-E034` | Current `README.md`, course filter documentation around lines 149-162; origin/main README blob `7763643f0785068413e865aeeb812370bdd6e370` | Public/current documentation advertises simultaneous course filtering and is useful for claim-versus-implementation comparison, not as sole intent evidence. |

## 4. Capability inventory

### 4.1 Product purpose and boundaries

| ID | Level | Recovered capability or behavior | State and chronology | P1 classification / P2 gate | Evidence |
|---|---|---|---|---|---|
| `LEG-A-001` | Product | Browse Rutgers SOC course offerings and monitor section openings in one product. | Original product goal; still consistent with current A. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E003`, `LC-E031` |
| `LEG-A-002` | User | Serve ordinary users, not only developers who know GitHub or the source tree. | Old one-click intent; later strengthened into unpack-and-BAT Windows A. | Current accepted constraint | `LC-E001`, `LC-E002` |
| `LEG-A-003` | Release | Deliver a complete but honest product: working/documented claims only; remove, hide, or defer false surfaces. | Historical Phase 1 purpose, reaffirmed by current User decisions. | Current accepted constraint | `LC-E001`, `LC-E031`, `LC-E032` |
| `LEG-A-004` | Identity | No login, account system, Rutgers NetID, or student-number collection. | Explicit old long-term constraint; current active-session model also needs no account. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E003` |
| `LEG-A-005` | Registration | Do not automate WebReg enrollment/drop; users manually use the displayed Index in Rutgers systems. | Explicit old non-goal. | Explicit historical non-goal; P2 must preserve or explicitly reopen | `LC-E002`, `LC-E003` |
| `LEG-A-006` | Planning | No degree planning, graduation audit, or complex prerequisite validation; displaying prerequisite/Core information is allowed. | Explicit old non-goal. | Explicit historical non-goal; P2 pending | `LC-E002`, `LC-E003` |
| `LEG-A-007` | Data authority | Use public Rutgers SOC information as authority; do not bypass access controls or simulate a WebReg session. | Original public-data boundary; compatible with current shared Rutgers client. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E003`, `LC-E020` |
| `LEG-A-008` | Scope | Select and browse multiple academic terms. | Old requirement and current data/filter model. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E006`, `LC-E010`, `LC-E019` |
| `LEG-A-009` | Scope | Select and browse multiple Rutgers campuses. | Old requirement/current implementation; exact campus coverage was historically unresolved. | Strong historical requirement candidate; P2 must set supported coverage | `LC-E002`, `LC-E010`, `LC-E022` |
| `LEG-A-010` | Scope | Represent undergraduate, graduate, and unknown/N/A levels. | Current filter supports UG/GR/N/A; old MVP campus/graduate breadth was an open question. | Historical/current candidate; P2 pending | `LC-E002`, `LC-E010`, `LC-E022` |
| `LEG-A-011` | Language | Switch the whole static UI between English and Chinese while preserving Rutgers course data in its source language. | Explicit FR-07; implemented and checked. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E027` |
| `LEG-A-012` | Form factor | Provide a usable browser UI on desktop and mobile. | Historical responsive design and current B purpose; final UI is deferred to P7.2/P7.3. | Strong historical/current candidate; P2 pending | `LC-E001`, `LC-E015` |
| `LEG-A-013` | Distribution | Be open source and straightforward to deploy/run. | Original cloud one-click shape was superseded; current A specifically requires an unpackable Windows BAT package. | Current A packaging boundary; old hosting mechanics superseded | `LC-E001`, `LC-E002`, `LC-E004` |
| `LEG-A-014` | Privacy | Avoid unnecessary personal data, tracking cookies, and secrets in client/repository/release artifacts. | Old privacy NFR plus current credential boundary. | Strong historical/current constraint; P2 pending | `LC-E001`, `LC-E002`, `LC-E024` |
| `LEG-A-015` | Notifications | Email/SMTP/SendGrid and Discord were old notification candidates. A/B v1 now have browser audio only, and email is future GitHub-tracked work. | Direct current decision supersedes old FR-04/FR-05 channels. | Current accepted exclusion; do not revive in P2 v1 | `LC-E001`, `LC-E002`, `LC-E024`, `LC-E026` |
| `LEG-A-016` | A/B parity | A and B use one ordinary-user WebUI and shared core behavior where deployment allows; local configuration/runtime surfaces may differ. | Accepted after the old project. | Current accepted constraint | `LC-E001` |

### 4.2 Catalog, data, and refresh

| ID | Level | Recovered capability or behavior | State and chronology | P1 classification / P2 gate | Evidence |
|---|---|---|---|---|---|
| `LEG-A-017` | Catalog | Choose term and campus before acquiring or querying a catalog. | Old FR-01; current fetch and filter UI implement the selection. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E010`, `LC-E021` |
| `LEG-A-018` | Catalog | Show the complete course and section set for the selected scope rather than a hand-curated subset; the old numeric target was coverage of at least 95% of official course fields. | Old FR-01 and field-coverage success criterion. | Strong historical requirement candidate; P2 must define completeness validation and whether the old percentage remains meaningful | `LC-E002`, `LC-E019`, `LC-E020` |
| `LEG-A-019` | Course | Preserve title, subject/course code and number, credits, school/department, Core attributes, prerequisites, level, and cross-list metadata when Rutgers supplies them. | Explicit old field list; current model stores a substantial but not necessarily complete subset. | Strong historical requirement candidate; field-by-field P2 audit required | `LC-E002`, `LC-E019`, `LC-E020` |
| `LEG-A-020` | Section | Preserve Index, section number, open status, delivery method, exam code, permission codes, instructors, and section-level campus where available. | Explicit old field list/current schema and query previews. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E012`, `LC-E019`, `LC-E020` |
| `LEG-A-021` | Meeting | Store structured meeting day, start/end minutes, campus/location, building, and room for each section meeting. | Required by old time/calendar behavior and implemented in the data/query model. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E012`, `LC-E019` |
| `LEG-A-022` | Instructor | Preserve instructor identities/names and section relationships. | Old display/filter requirement and current data dictionary/model. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E019`, `LC-E022` |
| `LEG-A-023` | Cross-list | Preserve and expose cross-list relationships or enough source data to identify them. | Explicit FR-02 candidate; current product behavior was not proven in the mounted UI. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E019`, `LC-E020` |
| `LEG-A-024` | Data limit | Do not claim seat capacity or waitlist counts when Rutgers public responses do not reliably provide them. | Field investigation contradicts early assumption that all availability fields existed. | Known source limitation; P2 must define truthful display | `LC-E002`, `LC-E020` |
| `LEG-A-025` | Persistence | Use local SQLite as the searchable source of truth for A's catalog. | Historical architecture pivot and current accepted Rust/SQLite direction. | Current accepted constraint | `LC-E001`, `LC-E004`, `LC-E019` |
| `LEG-A-026` | Persistence | Create and upgrade schema through repeatable, versioned, non-destructive migrations. | Historical tasks/current migrations. | Implemented historical candidate; P2/P3 pending | `LC-E006`, `LC-E019` |
| `LEG-A-027` | Refresh | Support a full initial catalog load. | Historical pipeline and current manual fetch path. | Strong historical requirement candidate; P2 pending | `LC-E006`, `LC-E019`, `LC-E021` |
| `LEG-A-028` | Refresh | Support incremental updates rather than requiring every refresh to rebuild all data. | Historical strategy implemented with hashes/upserts; exact future Rust design is open. | Strong historical implementation candidate; P2/P3 pending | `LC-E006`, `LC-E019` |
| `LEG-A-029` | Refresh | Stage and validate new catalog data before atomic replacement; retain a usable last successful catalog on failure. | Historical transactional design; later current target explicitly requires atomic replacement. | Current accepted outcome; mechanics later | `LC-E001`, `LC-E019` |
| `LEG-A-030` | Refresh | Configure multiple term/campus targets for acquisition. | Old fetch-pipeline behavior; current mounted card is one manual selection at a time. | Historical candidate with present-surface mismatch; P2 pending | `LC-E006`, `LC-E019`, `LC-E021` |
| `LEG-A-031` | Refresh | Apply rate discipline, timeout, retry, and backoff to Rutgers requests. | Historical probes/pipeline and current accepted upstream-failure behavior. | Current accepted outcome; constants later | `LC-E001`, `LC-E019`, `LC-E020` |
| `LEG-A-032` | Refresh | Use content hashes/diffs/upserts to identify additions, changes, and removals. | Historical implementation detail supporting incremental refresh. | Implemented historical candidate; P3 may redesign | `LC-E006`, `LC-E019` |
| `LEG-A-033` | Recovery | Resume long-running poll/fetch work from checkpoints after interruption. | Old poller implementation and tests. | Historical implementation candidate; P2/P3 pending | `LC-E006`, `LC-E026` |
| `LEG-A-034` | Local UI | Allow a local user to start a fetch for year/season/campus and see running/success/error state plus the log path. | Mounted current A-only admin-like surface. | Implemented current surface; P2 must keep, redesign, or hide | `LC-E016`, `LC-E021` |
| `LEG-A-035` | Dictionary | Bootstrap filter choices from catalog-derived terms, campuses, campus locations, subjects, Core codes, levels, delivery methods, and instructors. | Current API with fallback values. | Implemented historical candidate; P2 pending | `LC-E015`, `LC-E022` |
| `LEG-A-036` | Dictionary | Fall back gracefully when dictionary tables are unavailable or empty. | Current API/frontend behavior. | Implemented behavior; P2 must audit whether fallback can become stale/false | `LC-E022` |
| `LEG-A-037` | Dictionary | Provide exam-code choices from authoritative/current data. | Frontend expects exam choices, backend dictionary omits them, and fallback masks the mismatch. | Known drift; P2 must repair or remove claim | `LC-E010`, `LC-E022` |
| `LEG-A-038` | Search storage | Maintain full-text-search material for course keyword queries. | Current SQLite/FTS query path. | Implemented and exercised; P2 pending | `LC-E012`, `LC-E013`, `LC-E019` |
| `LEG-A-039` | Query | Return stable paginated and sortable results for large catalogs. | Old NFR/current API and tests. | Implemented and exercised; P2 pending | `LC-E002`, `LC-E010`, `LC-E012`, `LC-E013` |
| `LEG-A-040` | Catalog cadence | Refresh the course directory every ten minutes by default; only A exposes local configurability. | Later current decision supersedes older cadence proposals. | Current accepted constraint | `LC-E001` |
| `LEG-A-041` | Scheduling | Run scheduled server/local catalog fetches. | Service/route exists in branch/package history but is not registered in inspected roots. | Implemented but orphaned; P2 must not assume it works | `LC-E023`, `LC-E030` |
| `LEG-A-042` | Browser refresh | Let a visible browser periodically refresh displayed query results at a stored 15-120 second interval. | Historical branch code, not mounted; this is distinct from authoritative catalog/status polling. | Implemented but orphaned; P2 pending | `LC-E023` |
| `LEG-A-043` | Freshness UI | Show last synchronization time/data version and disable dependent controls when dictionary/catalog freshness is unavailable. | Planned in UI architecture; only portions of health/error handling exist. | Strong historical design candidate; implementation incomplete | `LC-E015`, `LC-E017`, `LC-E029` |

### 4.3 Course and section filtering

Filter rows are deliberately separate. "Supports filtering" does not preserve
multi-select behavior, cross-field composition, meeting semantics, or the
features that were removed during the 2025 rewrite.

| ID | Level | Recovered capability or behavior | State and chronology | P1 classification / P2 gate | Evidence |
|---|---|---|---|---|---|
| `LEG-A-044` | Course | Search by free text across course title/code and indexed course text. | Old FR-02/current FTS query and mounted keyword field. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E010`, `LC-E011`, `LC-E012` |
| `LEG-A-045` | Course | Filter by required academic term. | Current query refuses to run without a term. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E010`, `LC-E012` |
| `LEG-A-046` | Course | Filter by campus. | Historical requirement/current state and API. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E010`, `LC-E012` |
| `LEG-A-047` | Course/location | Multi-select campus locations within a campus. | Added/retained in final rewrite and current UI/API. | Implemented current candidate; P2 pending | `LC-E007`, `LC-E010`, `LC-E011`, `LC-E012` |
| `LEG-A-048` | Course | Search and multi-select subjects/departments. | Current UI searches subject options; API treats values as one field group. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E010`, `LC-E011`, `LC-E012` |
| `LEG-A-049` | Course | Multi-select course level (UG/GR/N/A). | Current state/UI/API. | Implemented current candidate; P2 pending | `LC-E007`, `LC-E010`, `LC-E011`, `LC-E012` |
| `LEG-A-050` | Course | Filter by minimum and/or maximum credits. | Explicit FR-02/current range implementation. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E010`, `LC-E011`, `LC-E012` |
| `LEG-A-051` | Course | Multi-select Core Curriculum codes. | Explicit FR-02/current implementation. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E010`, `LC-E011`, `LC-E012`, `LC-E013` |
| `LEG-A-052` | Section/course | Multi-select exam codes so a course qualifies through matching sections. | Retained in rewrite/current implementation and tests. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E007`, `LC-E010`, `LC-E012`, `LC-E013` |
| `LEG-A-053` | Course | Filter prerequisite state as any, has prerequisite, or none. | Current tri-state implementation. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E010`, `LC-E011`, `LC-E012` |
| `LEG-A-054` | Section/course | Multi-select delivery methods (in-person, online, hybrid). | Explicit FR-02/current implementation. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E010`, `LC-E011`, `LC-E012`, `LC-E013` |
| `LEG-A-055` | Section/course | Switch between all courses and courses having at least one open matching section. | Current two-state filter replaced older explicit waitlist/status controls. | Implemented current candidate; P2 pending | `LC-E007`, `LC-E010`, `LC-E011`, `LC-E012` |
| `LEG-A-056` | Meeting/course | Select one or more meeting days from Monday through Sunday. | Explicit FR-02/current implementation. | Strong historical requirement candidate; P2 pending | `LC-E002`, `LC-E010`, `LC-E011`, `LC-E012` |
| `LEG-A-057` | Meeting/course | Apply strict day-subset semantics: a qualifying section cannot have another structured meeting on an unselected day. | Documented and implemented in SQL and client-side section filtering. | Implemented and exercised; P2 must explicitly retain or redesign semantics | `LC-E007`, `LC-E012`, `LC-E013`, `LC-E017` |
| `LEG-A-058` | Meeting/course | Filter by start/end window such that each qualifying structured meeting falls inside the requested bounds. | Old FR-02/current query and client behavior. | Strong historical requirement candidate; P2 must confirm exact inclusive/unknown-time semantics | `LC-E002`, `LC-E007`, `LC-E010`, `LC-E012`, `LC-E017` |
| `LEG-A-059` | Query semantics | Combine different filter fields with logical AND. | Explicit contract and SQL `filters.join(' AND ')`. | Implemented and exercised; P2 pending | `LC-E012`, `LC-E013` |
| `LEG-A-060` | Query semantics | Treat multiple selected values within a field as alternatives (logical OR/SQL `IN`). | Explicit contract and current normalizers. | Implemented and exercised; P2 pending | `LC-E010`, `LC-E012`, `LC-E013` |
| `LEG-A-061` | Query semantics | Require section-level predicates to be satisfied by a coherent matching section rather than by unrelated sections of one course. | Current `EXISTS`/section-preview filtering prevents cross-section false positives for implemented predicates. | Implemented behavior; P2 must preserve or deliberately redefine | `LC-E012`, `LC-E017` |
| `LEG-A-062` | Query semantics | Support simultaneous combinations such as meeting day + delivery + time, not only one active filter at a time. | Core user concern; directly exercised by tests. | Strong historical requirement, implemented and tested; P2 pending | `LC-E002`, `LC-E013`, `LC-E034` |
| `LEG-A-063` | Filter UI | Show every active filter as a removable chip or equally legible active-state summary. | Current FilterPanel behavior. | Implemented current candidate; P2/UI design pending | `LC-E007`, `LC-E011` |
| `LEG-A-064` | Filter UI | Remove a single active filter without resetting unrelated filters. | Current chip behavior. | Implemented current candidate; P2/UI design pending | `LC-E011` |
| `LEG-A-065` | Filter UI | Clear a filter group and reset the complete filter state. | Current panel contains both group clearing and reset behavior. | Implemented current candidate; P2/UI design pending | `LC-E011` |
| `LEG-A-066` | Validation | Reject missing required term, invalid enum/day values, impossible page/range values, and reversed time windows. | Current route schemas and UI/query contract. | Implemented candidate; P2/API contract pending | `LC-E012`, `LC-E033` |
| `LEG-A-067` | Interaction | Debounce course-query input by 200 ms. | Current hook implementation; not a historical product promise. | Implementation observation; P3/P7 may redesign | `LC-E017` |
| `LEG-A-068` | Interaction | Cancel an obsolete in-flight query when filters change. | Current hook and historical UI design. | Implemented behavior; P3/P7 may redesign | `LC-E015`, `LC-E017` |
| `LEG-A-069` | Interaction | Cache results by stable query key for a freshness window. | Current hook and historical UI design. | Implemented behavior; P3/P7 may redesign | `LC-E015`, `LC-E017` |
| `LEG-A-070` | Sharing/state | Serialize and parse multi-value filters, meeting window, page, and sort in URL query parameters. | Helper implementation exists, but no current App caller was found. | Implemented but unmounted/orphaned; P2 pending | `LC-E010`, `LC-E015` |
| `LEG-A-071` | Query | Select page/page size and expose total result count. | Current state/API/test behavior. | Implemented and exercised; P2 pending | `LC-E010`, `LC-E012`, `LC-E013`, `LC-E017` |
| `LEG-A-072` | Query | Sort by supported fields/directions with deterministic tie-breaking. | Current state/API/test behavior. | Implemented and exercised; P2 must choose exposed sort set | `LC-E010`, `LC-E012`, `LC-E013` |
| `LEG-A-073` | Course | Filter by exact/partial course number independently of free-text search. | Present before final rewrite and explicitly removed from current state/UI. | Historical implementation removed; P2 must adjudicate | `LC-E002`, `LC-E014` |
| `LEG-A-074` | Section | Filter by Index and section number. | Explicit FR-02 and pre-rewrite UI; removed from the final course-filter surface. | Strong historical requirement plus removed implementation; P2 must adjudicate | `LC-E002`, `LC-E014` |
| `LEG-A-075` | Section | Search and multi-select instructors. | Explicit FR-02/pre-rewrite UI and dictionary; removed from current FilterPanel state. | Strong historical requirement plus removed implementation; P2 must adjudicate | `LC-E002`, `LC-E014`, `LC-E022` |
| `LEG-A-076` | Section | Multi-select explicit Open, Waitlist, and Closed states, including a waitlist shortcut. | Explicit FR-02/pre-rewrite UI; narrowed to all/open-only. | Strong historical requirement plus removed implementation; P2 must adjudicate | `LC-E002`, `LC-E014` |
| `LEG-A-077` | Meeting | Multi-select meeting-campus values distinct from the course-campus scope. | Pre-rewrite UI/API query support; removed from current filter state, although query internals still have meeting-campus handling. | Historical implementation partially removed/drifting; P2 must adjudicate | `LC-E012`, `LC-E014` |
| `LEG-A-078` | Meeting | Filter by building and room. | Explicit FR-02/pre-rewrite UI; removed from current filter surface. | Strong historical requirement plus removed implementation; P2 must adjudicate | `LC-E002`, `LC-E014` |
| `LEG-A-079` | Section | Filter whether add/drop permission is required. | Pre-rewrite UI and section schema fields; removed from current course filters. | Historical implementation removed; P2 must adjudicate | `LC-E014`, `LC-E033` |
| `LEG-A-080` | Course/section | Apply tags and derived quick-keyword shortcuts such as honors or writing/STEM groupings. | Pre-rewrite state/UI; source authority and exact semantics were weak and the rewrite removed them. | Historical implementation removed; P2 must require authoritative definitions before any revival | `LC-E014` |
| `LEG-A-081` | Course | Filter by school/college or department beyond subject selection. | Explicit FR-02 wording; mounted current surface exposes subject, not a separate school selector. | Strong historical requirement candidate with implementation gap; P2 must adjudicate | `LC-E002`, `LC-E011`, `LC-E022` |
| `LEG-A-082` | Course | Filter cross-listed courses. | Explicit FR-02; no mounted current control or exercised current contract was found. | Strong historical requirement candidate with implementation gap; P2 must adjudicate | `LC-E002`, `LC-E011`, `LC-E012` |
| `LEG-A-083` | Section API | Query sections directly with course/index/status/instructor/permission/delivery/meeting/location/pagination/sort filters. | Detailed route schema exists, but there is no result query. | Known stub/drift; P2 must recover or remove/demote | `LC-E033` |

### 4.4 Results, schedule, language, and accessibility

| ID | Level | Recovered capability or behavior | State and chronology | P1 classification / P2 gate | Evidence |
|---|---|---|---|---|---|
| `LEG-A-084` | Course UI | List course code, campus, title, and open/all-closed summary. | Mounted current CourseList. | Implemented current candidate; P2 pending | `LC-E016`, `LC-E017` |
| `LEG-A-085` | Section UI | Show section preview rows with Index and current status. | Mounted current CourseList, but substantially sparser than old FR-01. | Implemented but incomplete relative to historical requirement; P2 pending | `LC-E002`, `LC-E017` |
| `LEG-A-086` | Detail UI | Let users inspect the full available course/section fields needed for registration and schedule judgment: credits, department, Index, section, instructor, status, meetings/location, Core, prerequisites, delivery, and exam information. | Explicit FR-01; current list does not expose all fields in its ordinary path. | Strong historical requirement candidate with current gap; P2 must define final field surface | `LC-E002`, `LC-E012`, `LC-E017` |
| `LEG-A-087` | Result UI | Show loading skeletons without collapsing the result layout. | Current CourseList and historical UI architecture. | Implemented current candidate; P2/UI pending | `LC-E015`, `LC-E017` |
| `LEG-A-088` | Result UI | Show API errors with retry and trace information. | Current CourseList/useCourseQuery/API trace path. | Implemented current candidate; P2/UI pending | `LC-E017`, `LC-E029` |
| `LEG-A-089` | Result UI | Show a distinct no-results/empty state. | Current CourseList. | Implemented current candidate; P2/UI pending | `LC-E017` |
| `LEG-A-090` | Result UI | Navigate paginated result pages while preserving filters. | Current state/query/list behavior. | Implemented current candidate; P2/UI pending | `LC-E010`, `LC-E013`, `LC-E017` |
| `LEG-A-091` | Schedule UI | Switch to a weekly/calendar visualization of meetings matching the current filters. | Explicit FR-03 and historical architecture. | Strong historical requirement candidate; P2 must adjudicate | `LC-E002`, `LC-E003`, `LC-E015` |
| `LEG-A-092` | Schedule UI | Render meeting blocks with day/time/location and a legend. | `SchedulePreview` implements this but is used only in the developer playground. | Implemented but unmounted/orphaned; P2 pending | `LC-E018` |
| `LEG-A-093` | Schedule UI | Keep List and Calendar views on one shared result/cache state. | Historical architecture only; ordinary App does not mount the calendar view. | Strong historical design candidate; P2/P3 pending | `LC-E015`, `LC-E016` |
| `LEG-A-094` | Detail UI | Open a pinned section drawer/panel from list/calendar and place the watch action there. | Historical UI architecture; not proven in current App. | Historical design candidate; P2/UI pending | `LC-E015`, `LC-E016` |
| `LEG-A-095` | Saved state | Save and reload local filter presets/views. | Historical architecture and later release-surface question; no mounted implementation proven. | Historical candidate; P2 pending | `LC-E015`, `LC-E031` |
| `LEG-A-096` | Sharing | Copy a share link that restores filter state. | Historical architecture plus orphan URL helpers. | Historical candidate/partial implementation; P2 pending | `LC-E010`, `LC-E015`, `LC-E031` |
| `LEG-A-097` | Result UI | Offer a compact result view. | Named in historical release questions, but no verified mounted implementation. | Weak historical candidate; P2 pending | `LC-E031`, `LC-E032` |
| `LEG-A-098` | Scheduling | Detect schedule conflicts between selected/visible sections. | Explicitly described as later work in deep research, not MVP. | Explicit historical future/non-v1 candidate; P2 pending | `LC-E003`, `LC-E015` |
| `LEG-A-099` | Scheduling | Export selected schedule data to ICS. | Explicitly described as later work, not MVP. | Explicit historical future/non-v1 candidate; P2 pending | `LC-E003` |
| `LEG-A-100` | Scheduling | Find courses by drawing/selecting free time or by non-conflict semantics. | FR-03 deferred this; Q-05 left semantics unresolved. | Explicit historical future candidate; P2 must not claim without semantic decision | `LC-E002`, `LC-E003` |
| `LEG-A-101` | Performance UI | Virtualize a large course list. | Historical architecture/compact reported it; current CourseList renders rows directly. | Historical implementation drift; P2/P3 pending | `LC-E005`, `LC-E015`, `LC-E017` |
| `LEG-A-102` | Responsive UI | Use a fixed desktop filter column and a mobile drawer/collapsed control while preventing filter-result layout jumps. | Historical UI architecture; final design intentionally deferred to P7.2/P7.3. | Strong historical UX candidate; P2/UI pending | `LC-E001`, `LC-E015` |
| `LEG-A-103` | Accessibility | Respect `prefers-reduced-motion` when switching result/schedule states. | Historical UI requirement; no current implementation was found. | Historical requirement candidate with implementation gap; P2/UI pending | `LC-E015`, `LC-E016` |
| `LEG-A-104` | Accessibility | Use semantic/ARIA state for controls, loading, errors, status, keyboard, and touch operation. | Current components have partial ARIA; final completeness is unproven. | Historical/current quality candidate; P2/P7 validation pending | `LC-E011`, `LC-E017`, `LC-E021` |
| `LEG-A-105` | Language | Persist language choice, update `html.lang`, and fail checks for missing translated keys. | Implemented current i18n behavior. | Implemented and exercised; P2 pending | `LC-E027` |

### 4.5 Watching, polling, and notification behavior

| ID | Level | Recovered capability or behavior | State and chronology | P1 classification / P2 gate | Evidence |
|---|---|---|---|---|---|
| `LEG-A-106` | Watch | Select a specific section, historically identified by Index, and request opening monitoring from list/detail UI. | Explicit FR-04; a SubscribeButton component exists but is not mounted in the ordinary App, and the current exact key remains unresolved. | Strong historical requirement candidate; P2 must resolve identity and final entry point | `LC-E001`, `LC-E002`, `LC-E016`, `LC-E024` |
| `LEG-A-107` | Watch | Persist subscription records in SQLite with contact/channel state. | Implemented old architecture; directly conflicts with current connection-memory watch model. | Historical implementation superseded for v1 | `LC-E001`, `LC-E024` |
| `LEG-A-108` | Watch | Treat repeated creation of the same subscription idempotently/reuse an existing record. | Implemented old endpoint behavior. | Historical implementation superseded or redesign candidate | `LC-E024` |
| `LEG-A-109` | Watch | List active subscriptions and cancel/unsubscribe by ID or token. | Old FR-06 and mounted SubscriptionManager. Current v1 uses connection-scoped active watches. | Strong historical requirement plus superseded mechanism; P2 must translate user outcome or remove surface | `LC-E001`, `LC-E002`, `LC-E024` |
| `LEG-A-110` | Watch | Remember contact type/value and local-sound device identity in browser local storage. | Old mounted UI/hook behavior. Email/contact memory is obsolete; selection memory may remain inactive convenience. | Historical implementation partly superseded; P2 pending | `LC-E001`, `LC-E024`, `LC-E025` |
| `LEG-A-111` | Poller | Discover term/campus work from active persistent subscriptions and optional allowlists. | Old poller/launcher behavior. Current shared-core design instead centralizes active connection watches. | Historical implementation superseded or redesign candidate | `LC-E001`, `LC-E026`, `LC-E028` |
| `LEG-A-112` | Poller | Detect Closed-to-Open transitions and create durable open events. | Old FR-05/event model. Current accepted rule does not require a transition. | Historical behavior explicitly superseded | `LC-E001`, `LC-E002`, `LC-E026` |
| `LEG-A-113` | Poller | Deduplicate one notification per open event/channel. | Old FR-05 and event model. Current rule requires every fresh Open message and no debounce. | Historical behavior explicitly superseded | `LC-E001`, `LC-E002`, `LC-E026` |
| `LEG-A-114` | Poller | Checkpoint state and resume without replaying already-processed transitions. | Implemented old worker/restart tests. | Historical implementation candidate; current latest-value design may replace it | `LC-E026` |
| `LEG-A-115` | Poller | Retry/recover from Rutgers outage or invalid response while logging health/freshness. | Old worker and current accepted upstream behavior align at outcome level. | Current accepted outcome; mechanics later | `LC-E001`, `LC-E026`, `LC-E029` |
| `LEG-A-116` | Local notification | Claim pending local-sound notifications from a local API and mark them delivered. | Implemented old DB/event architecture. | Historical implementation superseded by WebSocket direction | `LC-E024`, `LC-E025` |
| `LEG-A-117` | Browser notification | Poll the local claim API every 7 seconds, use 15-second error backoff, and store enabled/device state. | Implemented old hook; incompatible with near-one-second WebSocket direction. | Historical implementation superseded | `LC-E001`, `LC-E025` |
| `LEG-A-118` | Browser audio | Play a fixed WebAudio oscillator tone, maintain a small toast list, and provide an audio-unlock path. | Implemented old hook; lacks accepted final controls/semantics. | Partial implementation observation; P2/P7 redesign pending | `LC-E025` |
| `LEG-A-119` | Notification channel | Send email through SendGrid with verification, localized templates, settings/configuration, retry policy, and a mail worker. | Large old implementation exists, but current User removed all v1 mail surfaces. | Current accepted exclusion; future GitHub-tracked work only | `LC-E001`, `LC-E006`, `LC-E028` |
| `LEG-A-120` | Notification channel | Send Discord bot messages. | Historical compacts describe it; current tree does not establish an ordinary working Discord surface. | Current v1 exclusion / historical-only candidate | `LC-E001`, `LC-E005`, `LC-E006` |
| `LEG-A-121` | Watch session | User explicitly starts watching from an open WebUI; remembered selections alone are inactive. | Later current decision replaces durable subscriptions. | Current accepted constraint | `LC-E001` |
| `LEG-A-122` | Watch session | Limit one live session to nine watched sections. | Later current decision. | Current accepted constraint | `LC-E001` |
| `LEG-A-123` | Watch session | Keep active watch state only in live connection memory and release it on confirmed close/disconnect/liveness expiry. | Later current decision; exact heartbeat/replacement semantics remain open. | Current accepted constraint; P3-P5 mechanics pending | `LC-E001` |
| `LEG-A-124` | Realtime | Use WebSocket for live watch updates and fresh status delivery. | Later current architecture decision. | Current accepted constraint | `LC-E001` |
| `LEG-A-125` | Alert semantics | Every fresh received Open status for a watched section causes audio, even if it was already Open. | Direct current correction supersedes transition-only logic. | Current accepted constraint | `LC-E001` |
| `LEG-A-126` | Alert semantics | Do not debounce fresh Open alerts. | Direct current decision. | Current accepted constraint | `LC-E001` |
| `LEG-A-127` | Audio | Expose a volume control. | Direct current decision; absent from old fixed-gain hook. | Current accepted constraint; implementation missing | `LC-E001`, `LC-E025` |
| `LEG-A-128` | Audio | Let the user choose one-shot or continuous playback. | Direct current decision; absent from old hook. | Current accepted constraint; implementation missing | `LC-E001`, `LC-E025` |
| `LEG-A-129` | Browser limit | Do not promise sound while the tab/browser is closed, locked, background-suspended, or disconnected. | Direct current accepted limitation. | Current accepted constraint | `LC-E001` |
| `LEG-A-130` | Realtime | Prefer latest-value/lag-aware delivery so a slow/reconnected browser does not replay a backlog of stale alerts. | Current target synthesis from fresh-message semantics. | Current accepted outcome; wire mechanics pending | `LC-E001` |
| `LEG-A-131` | Polling | Slow status polling when no watches exist and poll immediately when the first watch starts. | Current accepted direction; exact intervals require upstream evidence. | Current accepted direction; P3/P7 validation pending | `LC-E001` |
| `LEG-A-132` | Latency | Aim for roughly one second from status update to alert, without claiming an unmeasured hard guarantee. | Current decision supersedes old 30/60-second mail target. | Current accepted aspiration; P7 must measure | `LC-E001`, `LC-E002` |
| `LEG-A-133` | Identity | Verify the stable unique section key; do not assume Index alone across all supported term/campus scopes. | Current unresolved question against old Index-centric model. | Mandatory P2/P3 decision | `LC-E001`, `LC-E002`, `LC-E024` |

### 4.6 Runtime, diagnostics, packaging, and quality

| ID | Level | Recovered capability or behavior | State and chronology | P1 classification / P2 gate | Evidence |
|---|---|---|---|---|---|
| `LEG-A-134` | Diagnostics | Expose a health endpoint that reports application/dependency status. | Current API implementation. | Implemented behavior; final Rust equivalent required if kept | `LC-E029` |
| `LEG-A-135` | Diagnostics | Expose readiness that fails when SQLite or required schema is unavailable. | Current API implementation. | Implemented behavior; final Rust equivalent required if kept | `LC-E029` |
| `LEG-A-136` | Diagnostics | Attach request/trace IDs to API errors and logs so UI errors can be investigated. | Current API/UI and historical architecture. | Implemented behavior; P2/P3 pending | `LC-E015`, `LC-E017`, `LC-E029` |
| `LEG-A-137` | Diagnostics | Record course-query duration and summarized filters without leaking secrets. | Current route metrics/logging. | Implemented behavior; P2/P3 pending | `LC-E012`, `LC-E029` |
| `LEG-A-138` | Local security | Bind the local service only to loopback. | Later current architecture decision. | Current accepted constraint | `LC-E001` |
| `LEG-A-139` | Startup drift | `Start-WebUI.bat` currently exits when Node is absent and opens the Node download page. | Contradicts current ordinary-user package requirement. | Known drift; P2/P3/P7 must remove from release behavior | `LC-E001`, `LC-E028` |
| `LEG-A-140` | Startup drift | Current one-click startup installs npm dependencies and rebuilds native `better-sqlite3` when needed. | Developer-oriented behavior packaged as one-click. | Known drift; not acceptable proof of final package | `LC-E028` |
| `LEG-A-141` | Startup drift | Current startup may require Microsoft C++ Build Tools to recover a native dependency. | Directly violates no-toolchain ordinary-user requirement. | Known drift; must not ship as A v1 prerequisite | `LC-E001`, `LC-E028` |
| `LEG-A-142` | Packaging | Ship a Windows x64 archive that runs through BAT without separately installed Node/npm/Rust/SQLite/compiler/web server. | Later current accepted packaging contract. | Current accepted constraint | `LC-E001` |
| `LEG-A-143` | Filesystem | Store mutable data, user configuration, and logs under an appropriate per-user Windows location, not beside source/application files. | Later accepted outcome; exact path/migration policy remains open. | Current accepted constraint; P3 design pending | `LC-E001` |
| `LEG-A-144` | Startup | Wait for a healthy local service before opening the browser and report actionable startup failure/log location. | Current target outcome; old launcher opens on a timer. | Current accepted outcome; P3/P7 validation pending | `LC-E001`, `LC-E028`, `LC-E029` |
| `LEG-A-145` | Package integrity | Historical archives differ in root layout, source content, and scheduled-refresh files. | No old archive is canonical proof of release readiness. | Known package drift; P2/P7 must audit a new package | `LC-E030` |
| `LEG-A-146` | Package integrity | Scheduled-refresh source can be present in an archive while absent from App/server/container wiring. | Concrete false-surface risk. | Known orphan/drift; do not advertise without integration tests | `LC-E023`, `LC-E030` |
| `LEG-A-147` | Validation | Test A on a clean Windows machine by unpacking and starting through the BAT entry. | Later accepted package validation. | Current accepted constraint | `LC-E001`, `LC-E028` |
| `LEG-A-148` | Validation | Exercise functional, API contract, browser, package, security/secret, and capacity checks against the final artifact. | Current workflow requirement. | Current accepted constraint | `LC-E001` |
| `LEG-A-149` | Performance | Historical targets were <3 s first catalog load, <0.5 s combined-filter feedback at about 1,000 courses, and at least 200 concurrent browsing users in the old static design. | Architecture assumptions changed; numbers were never reaccepted for A. | Historical quality targets; P2/P6 must replace or validate before claims | `LC-E002`, `LC-E003` |
| `LEG-A-150` | Quality | Historical target requested lint/tests and at least 80% core-logic coverage. | Old governance target; current coverage was not established by P1. | Historical quality target; P2/P6 pending | `LC-E002` |
| `LEG-A-151` | Documentation | Ship truthful quickstart/run/deployment documentation and do not advertise a route, view, channel, or prerequisite that the artifact does not support. | Original one-click goal and current all-and-only principle. | Current accepted constraint | `LC-E001`, `LC-E002`, `LC-E031` |
| `LEG-A-152` | Surface honesty | Remove, hide, or explicitly defer empty `/api/sections`, mail configuration, orphan controls, stale launch instructions, and any other false UI/docs/config surface. | Central Phase 1 correction rule; exact all-and-only list is P2. | Current accepted constraint; P2 must enumerate | `LC-E001`, `LC-E016`, `LC-E028`, `LC-E033` |
| `LEG-A-153` | Architecture | Implement the final shared core as a Rust modular monolith with separate Windows-local and Linux-public composition roots, one React UI, SQLite, centralized polling, WebSocket, and B-side Caddy/systemd. | Later 0C decision supersedes Node/Fastify deployment mechanics without erasing product behaviors. | Current accepted constraint; decomposition belongs to P3-P5 | `LC-E001` |
| `LEG-A-154` | Cost | Historical operations target was free or very low monthly cost, including an example scale below 5,000 active durable subscriptions. | Belonged to the old cloud/mail architecture; current B selected a paid Vultr VM and current A is local. | Historical quality target only; not a current capacity/cost promise | `LC-E002` |
| `LEG-A-155` | Usability | Historical success criterion proposed at least 80% satisfaction in a small usability test of time visualization. | No reviewed result proves this target was tested. | Historical quality target; P6 may replace it with a defined usability protocol | `LC-E002` |
| `LEG-A-156` | Deployment | Historical one-click criterion proposed that an uninvolved developer could deploy an instance in under one hour without modifying code. | Original GitHub Pages/cloud-function shape was superseded by A/B delivery decisions. | Historical quality target; current A clean-machine and B deployment-package tests replace the old mechanics | `LC-E002`, `LC-E001` |
| `LEG-A-157` | Notification quality | Historical mail/Discord transition model targeted average delivery below 30 seconds, maximum below 60 seconds, and no missed opening events. | Current browser-audio/WebSocket model instead has a near-one-second aspiration and different trigger semantics. | Historical quality target explicitly superseded as a release claim | `LC-E002`, `LC-E001` |
| `LEG-A-158` | Platform | An older local-release statement expected ordinary unpack-and-use on Windows and macOS. | Current Deliverable A is Windows-local; B provides the no-install browser path for other platforms. | Historical breadth superseded for A v1; preserve chronology | `LC-E001`, `LC-E031` |
| `LEG-A-159` | Notification boundary | SMS and additional notification channels were explicitly outside the old MVP. | Current v1 narrows further to active-WebUI browser audio only. | Explicit historical non-goal plus current exclusion | `LC-E002`, `LC-E001` |

## 5. Chronology and contradiction ledger

| Topic | Earlier state | Later/current state | P1 consequence |
|---|---|---|---|
| Architecture | Static/client filtering plus cloud notification proposal, then Node/Fastify local SQLite/API. | Shared Rust core, React WebUI, separate Windows/Linux composition roots. | Preserve behavior evidence; do not preserve obsolete implementation as a requirement. |
| Filters | Broad FR-02 and a pre-rewrite implementation included Index, instructor, status/waitlist, location, permission, tags, and more. | Final rewrite retained a narrower set with precise multi-select and meeting semantics. | Inventory both retained and removed fields. P2 must adjudicate each, not treat the rewrite as automatic product approval. |
| Schedule | Week/calendar view was an explicit old requirement and a component was implemented. | Component is only mounted in the development playground; ordinary App has no Calendar tab. | Treat as a strong historical candidate and an orphaned implementation, not a working release feature. |
| Section detail | Old requirements demand complete section information; architecture described a section drawer/API. | Course previews are sparse and standalone `/api/sections` returns no data. | User outcome remains a candidate; route/interface choice is later. Stub must not be advertised. |
| Subscriptions | Durable email/Discord/local-sound subscriptions, transition events, dedupe, manage/unsubscribe. | Active browser session, max nine watches, connection memory, WebSocket, every fresh Open, no email/Discord. | Current direct rules supersede old mechanics, while section selection/manage UX outcomes still require P2 review. |
| Refresh | Manual fetch, incremental pipeline, old scheduled/auto-refresh branch, multiple historical cadences. | Catalog default ten minutes, configurable only in A; open status has a separate near-one-second aspiration. | Do not conflate catalog refresh, displayed-query refresh, and openSections status polling. |
| Startup | BAT delegates to Node script, dependency installation, native rebuild, multiple processes. | Final A must be an unpack-and-BAT no-runtime package. | Old launcher is drift evidence and a regression case, not a release baseline. |
| Release archives | 2026 archives contain divergent trees and orphan scheduled-refresh files. | No audited current A package exists. | Build a new artifact later; never infer capability membership from archive presence. |

## 6. P1 coverage conclusion

This inventory contains 159 behavior-level rows across product boundaries,
catalog/data, filters, results/schedule, watches/alerts, and release/runtime
quality. It explicitly includes the user's examples: multi-value filtering,
day/time filtering, and simultaneous combination semantics. It also preserves
removed filters, orphan views, stubs, old notification mechanics, and current
superseding decisions so P2 can make an all-and-only decision without losing
chronology.

No row in this file authorizes P2 or implementation. The next action after
single-line validation is the joint P1 Review.
