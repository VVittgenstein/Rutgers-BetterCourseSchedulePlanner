# P2 能力 ALL 矩阵

## 1. 读法

本表回答两个问题：

1. 每个当前 `REQUIRED` 用户/运行能力是否映射到完整交付链；
2. 当前旧代码在哪一环断裂，以及可复用单元如何进入后续设计。

`MAPPED_GAP` 表示交付链已被 P2 完整定位，但旧产品尚未实现目标；P2是审计阶段，因此不能把它误写成产品已完成。所有实现缺口进入P3计划输入。

## 2. 用户能力与完整交付链

| CAP ID / requirement | 用户场景 | UI | API / protocol | query / data / schema | worker / runtime | config | tests | docs | startup / package | P2完整性结论 |
|---|---|---|---|---|---|---|---|---|---|---|
| CAP-001 / CUR-DEL-04 | 普通Windows用户下载、解压、双击即用 | 启动/初始化/端口/错误/停止状态 | local HTTP+WS由`bcsp-local.exe`托管 | SQLite明确数据目录和迁移 | 单进程Rust模块化单体 | 本地用户设置，secret不入包 | clean Windows、无Node、空格/Unicode路径、重复启动/停止 | quickstart/troubleshooting/release notes | `.bat -> bcsp-local.exe`；自包含Windows archive | `MAPPED_GAP`：旧`.bat`壳可复用；Node/npm/native/Vite dev链重写 |
| CAP-002 / CUR-UX-01～05 | 使用同一正式产品壳找课，不被内部管理工具干扰 | React desktop/responsive、loading/empty/error/disabled、a11y | static assets + stable public errors | catalog freshness/status | local entry serve UI | locale/audio preference | frontend unit/contract/e2e/a11y/visual | product overview与非目标 | compiled static UI进入local package | `MAPPED_GAP`：旧App壳/i18n可修复复用；Playground/mail/crawler壳清除 |
| CAP-003 / CUR-DATA-01～02 | 首次获得课程数据，并独立刷新课程信息与Open信息 | 两个时间截点、错误/重试、本地interval设置、Open pull计数 | 每attempt RefreshObservation；valid pull才有WS OpenObservation；catalog version | catalog target staging；Open状态安全reconcile；valid pull即使无变化也留新状态结果，failure保留last-known | Rust catalog scheduler + centralized Open poller | local catalog 10m(1–1440m)、Open 1s(1–3600s)；public固定10m/1s | first-run、429/5xx/empty、multi-term preservation、unchanged checkpoint、failure-no-transition、run/today计数、partial | data lifecycle、计数/时区与包差异 | runtime自行管理，无Node子进程/log path泄露 | `MAPPED_GAP`：SOC client/normalizer/ingest/migration高价值PORT；旧manual full-init、45s cache与破坏性刷新链重写 |
| CAP-004 / FLT-C01～C10 | 通过term/campus/subject、课程号、关键词、level、credits、core、prereq、location精准找course | 逐字段控件、active chips、core ANY/ALL、clear/reset | shared course query contract | indexed course predicates、FTS真实维护、三值可靠性 | Rust query service | dictionary来自当前DB，带version | 每字段、组合、FTS真实ingest、unknown、pagination | filter contract | UI+API+DB均进入包 | `MAPPED_GAP`：旧FilterPanel/state/query/tests可拆复用；course number/core mode/可靠性补齐 |
| CAP-005 / CUR-QRY-01 + FLT-S01～S11 | 以整个section为单位组合open、delivery modality/synchronicity、教师、availability、地点、exam、permission和eligibility | section filters、availability多星期多窗口、uncertain区域；On Campus/Online/Hybrid与Sync/Async两轴 | course和section共用同一predicate contract | 同一section EXISTS；MATCH/UNCERTAIN/NO_MATCH真值表；raw→canonical provenance | Rust query service | 无假fallback | same-section反例、三值AND/OR、requiredness-unknown窗外、generic Online unspecified、TBA/hybrid/optional/exam、raw不丢 | `06` + query API docs | 同一实现进入包 | `MAPPED_GAP`：旧SQL保留same-section骨架；当前delivery跨normalizer/API/UI漂移，time EXISTS、client post-filter与unknown丢失必须重写并经过真实数据门 |
| CAP-006 / CUR-UX-06 | 默认course-centered，展开匹配sections | course card、匹配/uncertain计数、show others + reasons | course results含匹配sections和理由 | server统一filter/total/page | query runtime | page size内部受控 | card状态、分页一致、other reasons | UI flow | compiled UI/API | `MAPPED_GAP`：旧CourseList壳可修复；贫化信息和前端后过滤清除 |
| CAP-007 / CUR-UX-07 | 独立搜索、直接访问并打开完整section详情 | section search、可刷新URL、详情/CTA | list/search + single detail contract；稳定复合key | section/detail字段从ingest到DB完整可达 | query runtime | n/a | direct URL、reload、not-found、字段/unknown、course展开一致 | section contract | router/static fallback/API进入包 | `MAPPED_GAP`：旧route是stub，只可抽schema候选，handler和UI重写 |
| CAP-008 / CUR-WATCH-01～03,09 | 从course/section选择最多9项并管理 | selected列表、逐项/批量start-stop、selected vs active | WS start/stop/state frames | `(term,campus,index)`验证；本地保存selection，公网新页面为空 | connection/section双向内存map | Max audible配置与包级非active偏好策略 | 第10项、invalid key、start/stop、public reload empty、local selection restore | watch contract | WS与UI同一runtime | `MAPPED_GAP`：旧管理UI壳可EXTRACT；手输/email/persistent active CRUD全部改写 |
| CAP-009 / CUR-WATCH-04～08 | 页面/连接有效才active；断线立即失效且不自动恢复 | connecting/active/stopped/lost状态 | heartbeat、close cleanup；transport可重连但不rewatch | server不持久个人active | live connection manager | 不保存active flag | disconnect/timeout/restart/reload/not-auto-active | lifecycle docs | 无subscription DB/runtime residue | `MAPPED_GAP`：旧DB active/5min discovery/`localSoundEnabled`与目标相反，REMOVE |
| CAP-010 / CUR-WATCH-10 | 每个Open episode可见；ONE_SHOT以Max audible限制每section声音但不停止watch | 聚合toast/alert、audible count/max、silent-cap、恢复声音 | 每valid/safely-reconciled pull新observation；failure只更新refresh status；fanout不接收stop-on-cap | per-section active-session audible count；episode summary | WS始终fanout有效observations，browser应用声音上限 | `Max audible notifications`默认3、任意正整数、无产品上限 | 第N声后N+1静默但观察/toast/history继续、failure不转episode、独立sections、restart/reset-count、audio failure | `06` §9 | 无旧DB notification queue | `MAPPED_GAP`：旧5 visible toast与Max拆开；旧stop-on-cap语义删除 |
| CAP-011 / CUR-SND-01～05 | WebUI发通用声音；选择一声或持续闹钟，并在UI辨认section | volume、mode、10m/Unlimited、per-section/confirm-all、test/resume、audio blocked | consume observation + episode state | local episode/watch history；public仅session state | browser AudioContext shared mixer | public每页默认；local保存偏好但不保存active | burst、A/C/D episode、Closed→Open、timeout/ack/resume、mute/autoplay | `06` §10 | 浏览器UI制品 | `MAPPED_GAP`：旧AudioContext/tone壳可EXTRACT；batch-once/fixed gain/HTTP poll与每poll重响重写 |
| CAP-012 / CUR-SND-03～04 | ONE_SHOT在section持续Open时按每次观察发声至cap；CONTINUOUS同episode确认后不重响 | observation count、episode状态、静默原因 | 每次有效pull新observationId；exact replay幂等 | OpenObservation/OpenEpisode/AudibleNotification分层 | centralized poller → live watchers；browser mode policy | cadence由CAP-003固定；声音策略不改变poll | persistent-open one-shot、continuous episode、replay、multi-client single poll | poller/event contract | shared core | `MAPPED_GAP`：旧edge/3min bucket仅从ONE_SHOT产品语义移除；CONTINUOUS采用明确Closed→Open episode门 |
| CAP-013 / CUR-UX-02 | 查看可靠Open/Closed、每次观察时间和拉取统计 | card/detail/current status、课程/Open时间截点、public today与local run/today计数 | course/section/status/refresh frame一致 | section状态单一真相；course aggregate一致；attempted/succeeded/failed/empty分开 | poller reconciliation + durable counters | miss/empty安全策略；`America/New_York`日界线 | open→closed、持续open、异常empty、failure freshness、restart counters、course aggregate一致 | status/refresh data docs | shared runtime | `MAPPED_GAP`：旧poller造成course aggregate陈旧且没有可审计refresh observations/counters；必须修复 |
| CAP-014 / current product memory | 至少`en-US`与`zh-CN`普通用户体验一致 | language switch、`html lang`、localized全状态 | error code由前端翻译 | message catalog不含mail/current Calendar残留；Rutgers raw原文 | UI runtime | public按system每页初始化不持久；local可保存且Reset清除；fallback en-US | key parity、missing/orphan、locale/date/time/a11y/visual、public/local persistence | EN/zh-CN quickstart与行为说明 | catalog编译进UI | `MAPPED_GAP`：i18next结构复用；旧中文夹英文/Playground/mail keys清理 |
| CAP-015 / CUR-SEC-01～03 | 本地使用不泄露secret、PII、内部path | 无mail key/contact/log path UI | redacted errors/logs；仅loopback local admin | private/runtime数据不进source/package | local service最小暴露 | secret/env边界；positive package allowlist | secret scan、artifact audit、route denylist、log redaction | security/troubleshooting | package无secret/private inventory/runtime checkpoint | `MAPPED_GAP`：旧mail admin/contact/list/rawURL/log path为高风险ONLY闭包 |
| CAP-016 / data baseline | 数据库可升级、可恢复且不因刷新损坏其他scope | 数据错误/恢复提示 | readiness/version状态 | versioned migrations、WAL、transaction/staging、safe keys/FTS | Rust DB lifecycle | local data path/retention | migration checksum/upgrade/rollback/multi-scope | local data model | DB在用户数据目录，不写release source tree | `MAPPED_GAP`：旧schema/migration algorithms可PORT；persistent subscription FK与dry-run写入等修复 |
| CAP-017 / CUR-UX-04, UI gates | 正式桌面/响应式UI完整可用 | 全状态、keyboard、screen reader、reduced motion、mobile/responsive | stable contracts | n/a | React runtime | user preferences | unit/e2e/a11y/visual matrix | UI behavior docs | compiled production assets，不运行Vite dev | `MAPPED_GAP`：P7.2/P7.3实现；P2定位旧组件/样式可用与缺口 |
| CAP-018 / governance + P7.4 input | 获得干净、可重复的本地一键包 | package version/about/error | n/a | empty initial DB或明确seed policy | self-contained exe/static assets | config sample无secret | reproducible build、manifest allowlist、package content、clean machine | quickstart/troubleshooting/release notes/checksums | 仅Windows archive；严格不是第三包 | `MAPPED_GAP`：当前无Cargo/builder/allowlist/audit；旧archives仅历史证据 |
| CAP-019 / 2026-07-13 Review | 本地跨启动保留非active设置与Open episode/watch history，并可恢复公网默认 | history列表/详情、删除、Reset确认；public当前filters/settings/history不持久 | local-only user-data/history API；Reset先stop all watches | episode summary、prefs与migration；无contact/token/active | local store adapter；public ephemeral current-state adapter | local user-data dir；无静默TTL；public无Saved definitions例外 | local restart/history、public reload defaults、Reset scope/active stop、migration | persistence/reset contract | local data进入用户目录且不进入release archive | `MAPPED_GAP`：旧personal subscription/history schema不得复活；需新建无身份、无active恢复的LOCAL_ONLY模型 |
| CAP-020 / 2026-07-13 Saved views | 本地用户保存、比较和复用复杂筛选组合，且未来新增字段可自动纳入；公网不提供 | LOCAL_ONLY Saved views manager、save/apply/rename/update/duplicate/delete/delete-all、modified/incompatible/conflict状态；普通筛选Reset明确保库；public无入口 | local revisioned typed definition + local user-data API；public无endpoint | shared versioned `FilterSchema`/stable field IDs；LOCAL_ONLY canonical filter snapshot、migration/CAS；不保存sort/result/watch/audio | local persistent store；public无adapter/storage | 无产品数量上限；quota/conflict显式；public capability denylist | local CRUD/revision conflict、page1 apply、dirty normalization、new-field registry、migration/incompatible、filter Reset保库/delete-all保当前filters/local user-data Reset、no URL/share；public DOM/route/API/storage/catalog/bundle absence | `06` §7.3–7.4 + local EN/zh-CN behavior；public文档不宣称该能力 | definitions不进release；public无Saved views代码/数据 | `MAPPED_GAP`：旧UI只有Save/PresetManager设计与manual dirtyFields骨架，无真实持久化实现；交互思路EXTRACT到LOCAL_ONLY，schema/storage重写 |

## 3. 内部必需质量能力

| CAP ID | 能力 | 源码/算法候选 | 当前缺口 | 后续验证方向 | 归属 |
|---|---|---|---|---|---|
| CAP-I01 | Rutgers client错误分类、timeout、retry/backoff | `scripts/soc_api_client.ts` | 当前profile多为假配置、无响应大小/完整rate策略 | fake upstream + current upstream受控验证 | `BASELINE_SHARED` |
| CAP-I02 | 字段normalization与稳定hash | `soc_normalizer.ts`、backfill兼容逻辑 | H/TH、时间校验、async推断、boolean status、section key | golden fixtures + raw provenance | `BASELINE_SHARED` |
| CAP-I03 | health/readiness | health route/tests | 表集合不全，无freshness/poller/WS，暴露内部错误 | readiness状态矩阵 | `BASELINE_SHARED` |
| CAP-I04 | request/poller metrics与redacted logs | logging plugin、poller metrics | raw URL、metrics绑定全部接口、无统一采集/rotation | security/log/metrics tests | `BASELINE_SHARED`；公网运维细节`PUBLIC_DELTA` |
| CAP-I05 | deterministic test入口 | 11个旧Node tests、reports/fixtures | root test固定失败、frontend零tests、多个核心分支未覆盖 | P3设计统一Rust+frontend+integration入口 | `INTERNAL_TOOLING` |
| CAP-I06 | dependency consumer closure | package manifests/import graph | unused react-window；Node backend/mail依赖与最终runtime混合 | dependency allowlist、unused/duplicate/license audit | `INTERNAL_TOOLING` |
| CAP-I07 | package/runtime residue guard | `.gitignore`、旧archive失败记忆 | 没有正向manifest；旧checkpoint/mail/tests曾入包 | manifest diff、secret/string/content audit | `INTERNAL_TOOLING` |
| CAP-I08 | 真实Rutgers数据证据门 | SOC probe/field tools、历史raw fixtures与独立profile脚本 | 当前Delivery/API/UI漂移、H/TH、FTS/instructor/eligibility/permission与open join均未由届时真实数据闭合 | 受控低请求量manifest、raw hash/profile、modality/synchronicity oracle、collision/join、empty/error fixtures；冲突即回Review | `INTERNAL_TOOLING`；结论供`BASELINE_SHARED`/`PUBLIC_DELTA`消费 |

## 4. Required 能力的双向核验

### 4.1 需求没有孤岛

- 20 项普通用户/交付能力全部具有 UI、protocol/data/runtime、test、docs、startup/package 去向。
- 8 项内部质量能力全部具有源码候选、缺口和验证消费者。
- 任何当前缺失实现均标为 `MAPPED_GAP`，没有用stub、schema、fixture或旧文档冒充完成。

### 4.2 Runtime/package表面均有需求或清除依据

- course/search/filter/data/health/React/i18n/Windows入口、本地history/reset与本地Saved views：映射到 CAP-001～020。
- persistent subscription、HTTP claim、mail、Calendar、macOS、Playground、旧Node运行时：映射到 `04-only-closure-matrix.md`，没有无依据保留。
- probes/reports/dev fixtures：只归 `INTERNAL_TOOLING/HISTORICAL_EVIDENCE`，禁止进入用户包。
- Linux脚本/部署文档：`PUBLIC_DELTA / CARRY_TO_P4`，不混入本地包也不从整个产品误删。

因此 P2 的 `ALL` 审计链已闭合；它证明“后续必须实现和验证什么”已经完整，不证明 P7 产品已经实现。
