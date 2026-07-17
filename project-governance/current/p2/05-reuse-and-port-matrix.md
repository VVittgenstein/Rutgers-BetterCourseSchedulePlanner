# P2 复用、移植与重写矩阵

## 1. 裁决原则

- 复用比例不是目标；可证明地服务获批产品才是目标。
- Rust目标不使旧TypeScript的算法、SQL、contract、fixtures和tests自动失效。
- `PORT` 表示移植行为/算法/contract，不表示把Node runtime塞入最终包。
- `REWRITE` 必须有具体原因；不能只写“技术栈不同”。
- 本表与 `01` 的158/158文件矩阵、`02`的混合语义矩阵交叉使用。

## 2. React UI 与浏览器层

| REUSE ID / path:symbol | 可复用价值层 | 裁决 | 目标消费者/位置 | 已知缺陷 | 必要修复/不原样复用理由 | 验证方向 |
|---|---|---|---|---|---|---|
| R-UI-001 `frontend/src/utils/classNames.ts` | source utility | `REUSE_AS_IS` | shared React components | 未发现行为缺陷 | 保持纯函数；若设计系统替代可机械迁移 | trivial unit/typecheck |
| R-UI-002 `index.html`、`main.tsx`、i18n bootstrap | source/entry | `REUSE_WITH_FIXES` | formal shared UI entry | Playground title、missing favicon、lang/title不一致、无error boundary | 修正asset/title/lang；增加route/error/bootstrap状态；注入public ephemeral/local persistent user-state adapter | clean build、no-404、`en-US`/`zh-CN`、public reload defaults、local direct-route |
| R-UI-003 `api/client.ts` | HTTP adapter/error contract | `REUSE_WITH_FIXES` | Rust HTTP API client | 无timeout、204/non-JSON、body truthiness、credential/CORS边界窄 | abort/timeout、typed errors、content-type、base URL；WS独立adapter | contract tests/fake server |
| R-UI-004 `FilterPanel.tsx/.css` | UI/state emission/chips/sidebar | `SPLIT` | shared precise filter UI + explicit LOCAL_ONLY Saved views extension slot | 不可达滚动历史、subjects截12、无core ANY/ALL、单window、无Preset UI、层级反向依赖 | 抽contract/chips/sidebar价值；按`06`逐字段重建；普通Reset只清当前filters；Saved manager仅挂入local shell，public不渲染 | field mapping、local Saved CRUD/CAS与Reset作用域、public DOM/route/bundle absence、keyboard/mobile/large dictionary |
| R-UI-005 `courseFilters.ts` | state/query/dirty/URL algorithms | `SPLIT` | shared central FilterSchema + in-memory state/API query/direct section route；LOCAL_ONLY saved snapshot codec/migration | manual dirty keys会漏新字段；browser filter URL会形成隐式Share并违背public reload默认；无tri-state/window/core mode | 使用同一versioned field registry生成shared state/query；local扩展生成canonical saved snapshot/compare/migrate；删除filter URL restore/share keys | query/snapshot property、new-field dirty/migration、public reload default且无Saved codepath、direct section reload |
| R-UI-006 `useCourseQuery.ts` mapping/cache/predicate | mapping/stable key/predicate tests | `SPLIT` | React query adapter；predicate golden fixtures | server分页后client过滤、unknown被丢、200 section cap假阴性、cache无界、instructor拆分错误 | server统一filter；mapping保留raw unknown；catalog-version invalidation | API golden、pagination、TBA/same-section/race |
| R-UI-007 `CourseList.tsx/.css` | course-card/status/pagination shell | `REUSE_WITH_FIXES` | course-centered results | 信息贫化、无direct section、other reasons/uncertain/watch | 按CAP-006/007重构；正式视觉后再保留样式片段 | state/a11y/responsive/direct detail |
| R-UI-008 `App.tsx/.css` | composition/layout/error state | `SPLIT` | shared route shell | mail真实挂载、内部crawler、无router、persistent manager | 移除mail；加入course/section routes；local admin条件能力 | route reachability、artifact denylist、visual |
| R-UI-009 `filters.ts`、`useFiltersDictionary.ts`、fallback shape | dynamic dictionary adapter | `SPLIT` | catalog dictionary UI | 吞错、hardcoded假数据、DB会产`async`而UI/API只收三类、未知枚举静默丢弃 | contract独立；Delivery拆modality/synchronicity并保留raw；error/empty/freshness；dev fixture与runtime分离 | real-data mapping、dictionary version/reliability/error、不丢unknown |
| R-UI-010 `term.ts` | term parse/build algorithm | `REUSE_WITH_FIXES` | term selector/catalog target | 与fallback格式冲突、silent clamp | 仅接受验证过的Rutgers formats，invalid明确错误 | golden term IDs/roundtrip |
| R-UI-011 `LanguageSwitcher.*`、i18n core、messages shared keys | UI/source/asset | `SPLIT` | `en-US`/`zh-CN` shared UI + LOCAL_ONLY Saved views namespace | 中文夹英文、orphan/mail/calendar keys、title未接、无Saved views states、未区分包级locale持久化 | namespace清理；共享key两包一致；Saved CRUD/dirty/incompatible/quota/revision-conflict keys只编入local；raw Rutgers与user view names原文；public每页system detect，local可保存 | i18n checker+locale format+public/local persistence+visual/a11y；public catalog/bundle无Saved keys |
| R-UI-012 `TagChip.*` | UI component | `REUSE_WITH_FIXES` | active filters/reasons | 潜在nested button | 改markup/interaction contract | DOM/keyboard/a11y |
| R-UI-013 `DataFetchCard.*`、fetchJobs API | status/selection/error shell | `SPLIT` | 双refresh freshness/settings | destructive full-init、内部log path、3.5s job polling、race、无时间截点/计数 | 以catalog/Open两个checkpoint、local interval、public fixed policy、run/today计数和safe retry替代crawler job | first-run/ranges/unchanged/partial/multi-scope/counters/error |
| R-UI-014 SubscriptionCenter/Manager/SubscribeButton section部分 | section chooser/list/feedback UI | `SPLIT` | live watch + local history/reset manager | 手输index、orphan、DB全局列表、email/token混合、无9上限 | 从card/detail选择；selected/active分层；public reload empty；local restore/history/Reset但不恢复active | 9-limit、lifecycle、public/local reload、history/reset、a11y |
| R-UI-015 `useLocalSoundNotifications` AudioContext/tone/toast + LocalSoundToggle | audio/toast UI state | `SPLIT` | WS observation/episode consumer | fixed gain/tone、batch一次、claim poll、active持久化、无continuous episode | shared mixer、ONE_SHOT Max audible静默、CONTINUOUS 10m/Unlimited episode/confirm/resume、blocked state、聚合toast | burst/cap-does-not-stop/A-C-D/Closed→Open/timeout/audio failure/browser |
| R-UI-016 `SchedulePreview.*`、dev calendar mock | future algorithm/style/fixture | `DEFER` | future Calendar issue only | orphan、无overlap/TBA/conflict，当前非目标 | 移出current source/runtime；不修成当前功能 | current artifact absence；future另验 |
| R-UI-017 MailSettings/admin client/mail UI branches | UI/API/types/assets | `REMOVE` | 无当前消费者 | 当前明确排除且有credential风险 | Git历史已保存，不保留dead code | source/ref/bundle/string zero |
| R-UI-018 `react-window` direct dependency | dependency | `REMOVE` | 无目标消费者 | 0 imports | pagination不需要预装；未来若有实证再引入 | manifest/lock/unused audit |
| R-UI-019 `courseFilters.dirtyFields` + `docs/ui_flow_course_list.md` Save view/PresetManager | interaction intent/state groundwork | `SPLIT` | LOCAL_ONLY Saved views domain + local service adapter | 只有manual dirty Set和设计文字，无manager/storage/schema/migration；旧文档与Share/URL耦合，Reset作用域未定义 | EXTRACT命名preset、apply、modified意图到local；REWRITE为canonical snapshot比较与versioned local adapter；拆开filter Reset、library delete-all和local user-data Reset；public完整排除，不移植Share/auto-restore | local CRUD/dirty/new-field/migration/quota/filter Reset保库/delete-all保filters/user-data Reset；public source/DOM/route/API/storage/catalog/bundle zero |

## 3. API、query、data 与运行算法

| REUSE ID / path:symbol | 可复用价值层 | 裁决 | 目标消费者/位置 | 已知缺陷 | 必要修复/不原样复用理由 | 验证方向 |
|---|---|---|---|---|---|---|
| R-BE-001 `api/src/config.ts` | config contract | `PORT` | Rust local/server adapters | 旧loopback/Node paths，无shared/local/public层次 | typed config、local user vs public fixed policy | config defaults/invalid/secret tests |
| R-BE-002 `container.ts` | SQLite lifecycle/WAL/FK | `PORT` | Rust DB service | lazy-open未统一迁移/schema version | startup migration、explicit lifecycle/concurrency | readiness/upgrade/shutdown |
| R-BE-003 request logging + server error envelope | contract/observability | `PORT` | Rust HTTP/WS boundary | raw URL/5xx/internal path泄露 | stable error codes/redaction/PII rules | security golden/log scan |
| R-BE-004 `sharedSchemas.ts` | validation/pagination normalizers | `PORT` | Rust request DTOs + generated TS types | course/section naming漂移、弱unknown处理 | single schema/field contract | serialization/property tests |
| R-BE-005 course route + `course_search.ts` | SQL/query/parameter binding/sort/same-section skeleton | `PORT` | Rust query repository | time仅EXISTS、core OR-only、course aggregate stale、FTS ingest断链、preview cap | tri-state/same-section/availability；FTS真实维护；reason output | golden fixtures/real ingest/perf |
| R-BE-006 filters query | dynamic dictionary SQL | `PORT` | Rust dictionary service | 吞错、假fallback、silent truncation、exam/async漂移 | typed values/source/version/truncation/error | DB fixtures/error/large dictionary |
| R-BE-007 sections route schema | field discovery/validation hints | `EXTRACT` | section search/detail contract | handler永久空、course/section predicate命名不一 | 只取有目标/数据支持字段；共享predicate | search/detail contract tests |
| R-BE-008 sections handler | none beyond stub surface | `REWRITE` | Rust section repository/routes | 固定0结果，没有single detail | 无安全可复用实现 | direct URL/list/detail/notfound |
| R-BE-009 health route/tests | readiness/error contract | `PORT` | Rust health/readiness | 只查6表，无freshness/poller/WS，可能泄错 | component health + public redaction | degraded matrix |
| R-BE-010 fetch route/runner | job state/mutex/error ideas | `SPLIT` | catalog scheduler/control | unauth写DB/log、Node spawn、global job、danger full-init | integrate Rust worker；safe scopes；local可safe retry，public user不得触发上游；不返回path | concurrency/first-run/ranges/multi-scope/partial |
| R-BE-011 subscriptions route/preferences | composite key与限制fixture | `SPLIT` | WS watch manager + browser policy | persistent contact/email/security漏洞/无限项；旧max会停watch | keep composite target/9-selection limit；Max audible默认3且无产品上限只在browser ONE_SHOT；remove CRUD/stop-on-cap | WS/lifecycle/selection-limit/cap-does-not-stop/security |
| R-BE-012 notification claim DTO | event fields | `EXTRACT` | versioned WS observation/refresh-status frames | 无status observation/episode区分、claim at-most-once loss | `observationId`、`sectionKey`、`OPEN/CLOSED/UNKNOWN`、`observedAt`、freshness、pull sequence及run/today `{attempted,succeeded,failed,empty}` snapshots；episode留在watch层 | contract fixture/replay/failure-no-transition/Closed-closes-episode/unchanged-pull |
| R-BE-013 HTTP claim route | protocol/runtime | `REMOVE` | 无 | 与WS目标冲突、claim-before-play loss | 无需兼容current product | denylist/ref/package audit |
| R-BE-014 mail admin/worker/provider/templates | mail product | `REMOVE` | 无当前消费者 | 明文key、可配base exfiltration、未鉴权、broken links/locale | current product无mail，不修旧链 | complete closure scan |
| R-BE-015 mail `retry_policy.ts` generic limiter | generic algorithm | `EXTRACT` | Rutgers client limiter only | 与mail types耦合、无queue limit/cancel/shutdown/bucket cleanup | 完全去mail语义后才可PORT；否则删除 | limiter unit/load/shutdown |

## 4. SOC、normalization、ingest、schema 与 poller

| REUSE ID / path:symbol | 可复用价值层 | 裁决 | 目标消费者/位置 | 已知缺陷 | 必要修复/不原样复用理由 | 验证方向 |
|---|---|---|---|---|---|---|
| R-DAT-001 `soc_api_client.ts` | endpoint/term/timeout/error/retry-hint | `PORT` | Rust Rutgers client | 旧rate证据过期、无response size/circuit policy | current policy deferred P4/P7；fake server resilience | 429/5xx/timeout/invalid/size |
| R-DAT-002 `soc_normalizer.ts` | data model/field extraction/stable hash | `PORT` | Rust normalization core | H/TH parser、invalid time、async from no meetings、boolean status、bare index、core variants | raw provenance、tri-state occurrence、safe key、strict validation | golden raw fixtures/differential |
| R-DAT-003 `backfill_core_attributes.ts` core variants | algorithm | `MERGE` | normalizer/ingest migration | separate drifted implementation | merge supported `coreCode/code/core_code` into single normalizer | core fixtures/idempotency |
| R-DAT-004 `fetch_soc_data.ts` | transactional/staging/upsert/hash/summary/open reconcile | `SPLIT` | Rust catalog ingest | full-init global delete、fake config、FTS/joins incomplete、empty payload risk | target-scoped transaction、FTS and details complete、invalid payload abort | multi-target/real ingest/rollback |
| R-DAT-005 `migrate_db.ts` | named migrations/checksum/transaction | `PORT` | Rust migration runner | dry-run仍写DB/log；schema drift | true read-only plan、version table、upgrade/rollback | clean/upgrade/checksum/dry-run |
| R-DAT-006 schema course/section/meeting/core/FTS | schema | `SPLIT` | shared SQLite schema | subject/section key、missing occurrence/raw Delivery语义、unfilled tables、FTS disconnect | only required fields；`(term,campus,index)`；modality/synchronicity+raw provenance；source reliability | real ingest+migration+query chain |
| R-DAT-007 schema personal subscriptions/fanout | schema | `REMOVE` | 无旧模型消费者 | 与live/nonpersistent active模型冲突 | 删除contact/token/active/fanout；不得误删新LOCAL_ONLY episode history store | clean DB allowlist + history migration isolation |
| R-DAT-008 `open_sections_poller.ts` target coalesce/poll/miss/metrics/checkpoint | algorithm/runtime | `SPLIT` | Rust centralized poller | target来自persistent subscriptions、5min delay、empty mass close、non-atomic checkpoint、metrics bind all、无每pull可审计结果 | target来自已批准catalog/service scope而非watch ref-count；safe empty/circuit、single-flight、atomic observation/counters；local 1–3600s/public fixed1s | no-watch status freshness、multi-client no-extra-poll、fake upstream/restart/today+run counts/unchanged |
| R-DAT-009 poller edge/reminder/dedupe/fanout | product semantics | `REWRITE` | Open observation broadcaster + browser episode consumer | 3min suppression、DB mail/local queue、last_notified | 每次有效pull新observation并direct live broadcast；ONE_SHOT audible cap仅browser；CONTINUOUS按Closed→Open episode | persistent-open/replay/mode split/count |
| R-DAT-010 course-search/health/poller tests | tests/fixtures | `SPLIT` | Rust unit/integration suite | artificial FTS seed、missing core poller paths | preserve good fixtures, add full chain/unknown/safety | deterministic unified test entry |
| R-DAT-011 subscription/claim/mail tests | tests | `SPLIT` | live WS/security regression | 固化被排除行为与漏洞 | section fixtures保留；期待行为反转 | deny old routes + new lifecycle |
| R-DAT-012 new local user-state/history/saved-view store | product contract；无安全旧schema可复用 | `REWRITE` | `LOCAL_ONLY` prefs、episode/watch history、Saved views与Reset adapter | 旧subscriptions/events含contact、token、active恢复与fanout，且无saved-view schema | 新建无身份episode summary + versioned saved definitions；Reset stop watches后清user data；无静默TTL/eviction | restart/history/saved CRUD+migration/reset/no-active-restore/package exclusion |

## 5. Startup、build、docs、reports 与制品

| REUSE ID / path/surface | 可复用价值层 | 裁决 | 目标消费者/位置 | 已知缺陷 | 必要修复/不原样复用理由 | 验证方向 |
|---|---|---|---|---|---|---|
| R-RUN-001 `Start-WebUI.bat` | user entry/error shell | `REUSE_WITH_FIXES` | local package thin launcher | checks/opens Node download；calls JS | only locate/launch exe, error/pause/single instance | clean Windows no Node |
| R-RUN-002 `Start-WebUI.command` | historical launcher | `REMOVE` | none current | macOS current non-target/unverified | Git history keeps evidence | package/source/docs absence |
| R-RUN-003 `oneclick_start.js` | supervision/open browser/path/error lessons | `PORT` | `bcsp-local.exe` | Node/npm/native repair/Vite dev/mail/first-fetch bug | port only proven UX; integrate single Rust process | startup/failure/stop/path/port |
| R-RUN-004 root package/lock/tsconfig | dependency/command evidence | `SPLIT` | migration tooling only, then Rust/React build | test fixed fail、wrong repo、no build/package、Node backend graph | no final backend Node runtime; consumer-by-consumer closure | dependency/build/package audit |
| R-RUN-005 frontend package/lock/Vite/TS | build toolchain | `REUSE_WITH_FIXES` | shared React build | no tests/lint/WS proxy; unused dep | clean build/test configs, static assets served by Rust | clean install/typecheck/build/test |
| R-RUN-006 `.gitignore` | packaging/security guard | `REUSE_WITH_FIXES` | repo hygiene | ignore不是positive allowlist，broad archives仍可误包 | add explicit manifest/audit; keep private/runtime ignores | status/artifact/secret tests |
| R-RUN-007 query/data/fetch/i18n docs | contract/history | `SPLIT` | current technical docs | code/doc drift、fake config、term错误 | 从获批contract与真实数据门重建；固定双refresh、两轴Delivery、`en-US`/`zh-CN`、包级persistence差异 | docs-code link/fixture provenance/check |
| R-RUN-008 README/oneclick/quickstart | user docs | `REWRITE` | local package docs | Node/macOS/mail/manual refresh/wrong repo claims | zero-experience Windows instructions | clean-machine doc walkthrough |
| R-RUN-009 deployment docs/Linux scripts | ops lessons | `EXTRACT` | P4 public delta | oldNode/local/WSL与production混合 | do not place in local package or decide P4 now | P4 revalidation |
| R-RUN-010 SOC probe/rate/field tools/docs | internal tooling/evidence | `REUSE_WITH_FIXES` | upstream validation toolkit | 2025 rate evidence stale；scripts may write reports | isolate outputs/temp；never user package | P4/P7 controlled validation |
| R-RUN-011 reports/field samples | evidence/fixture source | `EXTRACT` | deterministic test fixtures + history | random/stale/环境报告不是当前通过 | extract minimal redacted fixtures; retain reports as history | fixture hash/provenance |
| R-RUN-012 old archives | directory metadata/failure memory | `OUT_OF_SCOPE/HISTORICAL_EVIDENCE` | audit only | divergent trees、mail/tests/checkpoint、no exe/vendor/docs | do not extract/copy/release again | archive hash/listing only |
| R-RUN-013 generated/runtime/vendor/private paths | build/runtime state | `REMOVE_FROM_PACKAGE` | regenerated runtime only | stale dist含mail、node_modules、DB/checkpoint/log/local config/secrets | positive allowlist; final dist rebuild; secret metadata only | artifact manifest/secret/string scan |

## 6. Direct dependency closure

| Dependency | 当前消费者 | P2去向 | 最终消费者/验证 |
|---|---|---|---|
| `react`, `react-dom` | React UI | retain | formal shared UI; build/e2e |
| `i18next`, `react-i18next` | i18n bootstrap/components | retain with fixes | bilingual UI; key parity |
| `react-window`, types | none | remove | none；manifest/lock zero |
| Vite/React plugin/TypeScript/frontend types | frontend build | retain internal only | clean build/typecheck，不入runtime vendor |
| `better-sqlite3` | Node API/scripts/workers/tests | port behavior then remove | Rust SQLite crate choice属于P3；final package zeroNode addon |
| Fastify/plugin/type-provider | Node API | port contract then remove | Rust HTTP/WS stack choice属于P3 |
| Zod | Node routes/config | port schemas; frontend若无consumer则remove root | Rust typed validation + TS contract |
| `tsx`, root TypeScript/@types | Node tools/tests | migration/internal then remove or isolate | final backend/test plan由P3定；不进用户包 |

## 7. Reuse 完成结论

- 每个 `REUSE_AS_IS / REUSE_WITH_FIXES / EXTRACT / PORT / REWRITE` 类别都有目标消费者、已知缺陷、修复边界和验证方向。
- 每个 `REWRITE` 都有具体行为、安全或架构闭环理由：stub、错误筛选、错误生命周期、危险数据刷新、协议冲突或无法满足自包含分发；没有只因“TypeScript→Rust”而整项目重写。
- 邮件、持久active和HTTP claim没有目标消费者，按完整闭包删除；generic limiter、section UI壳和DTO字段只有脱离被排除语义后才能抽取。
- Saved views旧UI只复用sticky filter/sidebar、active chips、普通筛选Reset、命名preset与modified意图；旧历史不存在PresetManager CRUD/storage，manual dirtyFields与URL/Share耦合必须重写为共享FilterSchema上的LOCAL_ONLY revisioned adapter。普通筛选Reset、library delete-all与本地用户数据Reset必须是三个不混用的作用域；公网不得携带Saved views入口、API、storage、文案或bundle表面。
- old release、reports、Compact和Git只提供证据，不作为源码复用捷径。
