# P2 文件内语义矩阵

## 1. 读法与闭合规则

- 本矩阵闭合 `01-file-universe.tsv` 中全部 38 个 `MIXED_SEE_02` 文件。
- `R/E/F/I` 分别表示 `REQUIRED / EXCLUDED / FUTURE / INTERNAL_ONLY`。
- 交付归属使用完整名称；`HIST` 表示 `HISTORICAL_EVIDENCE`。
- 每行都是唯一语义去向；不得再用整文件标签覆盖本表。
- 行号是 2026-07-12 P2 冻结工作树的稳定 anchor。实现阶段如代码漂移，必须先重建引用和 hash。

## 2. API、route、protocol 与测试

| ID | path:anchor / 语义单元 | 当前行为 | 目标行为 | 能力 / 处置 / 归属 | inbound / outbound | replacement、清除闭包与验证 |
|---|---|---|---|---|---|---|
| SEM-API-001 | `api/src/routes/notifications.local.ts:7-58` claim DTO、通知字段 | 定义deviceId、limit和DB notification DTO；无法证明event一定是Open | 定义versioned observation/refresh-status：sectionKey、`OPEN/CLOSED/UNKNOWN`、observedAt/freshness及run/today `{attempted,succeeded,failed,empty}` snapshots | R / `PORT` / `BASELINE_SHARED` | server注册；frontend WS types | CLOSED闭合CONTINUOUS episode；failed attempt只发refresh status并保留last-known，不得伪造UNKNOWN/CLOSED |
| SEM-API-002 | `api/src/routes/notifications.local.ts:60-211` HTTP claim | SHA-1 device、事务 claim 后在响应/播放前标 sent | 当前产品没有 HTTP claim；以 live WebSocket fanout 代替 | E / `REMOVE` / `HIST` | `server.ts:16,93`；frontend notifications API/hook | 删除 route、client、tests、schema fanout refs；最终 source/bundle/API route 零 claim 字符串 |
| SEM-API-003 | `api/src/routes/subscriptions.ts:19-69,126-162` section locator 与 preferences parsing | term/campus/index、notifyOn/max/quiet/snooze 混在持久订阅 DTO | 抽取复合section key；Max audible是browser ONE_SHOT策略；quiet/snooze/waitlist与个人active DTO不进入当前版 | R / `EXTRACT` / `BASELINE_SHARED` | WS watch、frontend state types | 移除contact/email/token/quiet/snooze/waitlist/stop-on-cap；本地prefs/history使用独立LOCAL_ONLY模型 |
| SEM-API-004 | `api/src/routes/subscriptions.ts:168-316,382-695` create/resolve/dedupe/unsubscribe | 写SQLite active subscription，可未解析、跨重启恢复、token/id退订 | active只在connection/内存；local另存non-active prefs/history，public每页默认；显式start/stop | E / `REMOVE` / `HIST` | server、poller、frontend/user-state adapters | 删除DB active CRUD/unresolved model；以WS替代；断连清理、public/local persistence测试 |
| SEM-API-005 | `api/src/routes/subscriptions.ts:317-380,636-659` 全局 list 与 id-only cancel | 无认证返回所有 contactValue；递增 ID 可取消任意 channel | 仅当前 connection 的 active/selected view；没有服务器 contact 数据 | E / `REMOVE` / `HIST` | manager UI/tests | 安全回归断言无全局 list、contact、token、id-only route |
| SEM-API-006 | `api/src/routes/subscriptions.ts:153-162,443-457` `maxNotifications` default/range | 默认3、输入1–10且旧poller不消费 | 重命名`Max audible notifications`；每section每次显式watch默认3、任意正整数、仅ONE_SHOT；第N声后只静默不停止watch | R / `REWRITE` / `BASELINE_SHARED` | browser audio policy；WS持续fanout | 与toast可见数分离；显式start或“恢复声音/重置计数”归零；Audio开始失败不计数 |
| SEM-API-007 | `api/src/server.ts:18-82` lifecycle、trace、error envelope | Fastify lifecycle、traceId；5xx/404可能回显内部 message/URL | Rust 服务生命周期、稳定公开错误码、redacted trace/log | R / `PORT` / `BASELINE_SHARED` | all routes、request logging | golden error fixtures；日志/响应不含 secret、contact、内部 path/raw URL |
| SEM-API-008 | `api/src/server.ts:83-95` route registration | 无条件注册 mail、persistent subscription、claim；无 WS/static UI | 注册 search/section/filter/health/local refresh、static React、WS watch；不注册 mail/claim | R / `PORT` / `BASELINE_SHARED` | all route modules | route allowlist/denylist测试；`/admin/mail-config`、claim、旧 subscribe API 均 404 |
| SEM-API-009 | `api/tests/notifications.local.test.ts:1-140` | 固化 claim 即 sent | 只保留 notification event fixture；协议测试改为 WS delivery/ack-free client receipt | I / `REWRITE` / `INTERNAL_TOOLING` | old route/schema | 删除 claim expectations；新增每条 Open、disconnect、reconnect、audio-failure与toast计数测试 |
| SEM-API-010 | `api/tests/subscriptions.test.ts:1-361` | 固化email、持久active、unresolved、无限订阅、id-only取消 | 复用section fixture；重写为复合key、最多9项、live lifecycle、Max audible不停止fanout与包级持久化差异 | I / `REWRITE` / `INTERNAL_TOOLING` | WS/schema/browser adapter | 第10项拒绝、disconnect清理、reload不active、public reload defaults、local history/reset反向回归 |

## 3. Fetch 配置、schema 与 migrations

| ID | path:anchor / 语义单元 | 当前行为 | 目标行为 | 能力 / 处置 / 归属 | inbound / outbound | replacement、清除闭包与验证 |
|---|---|---|---|---|---|---|
| SEM-DAT-001 | `configs/fetch_pipeline.example.json:8-76` targets、timeout、retry/rate shape | 多 term/campus、HTTP timeout、lane/retry 配置；不少字段未被 runner消费 | 复合 target、timeout、有限 retry/backoff 进入 Rust catalog client | R / `PORT` / `BASELINE_SHARED` | fetch script/runner/docs | 每个保留 key 必须有 Rust consumer 和 fake-upstream tests；无 consumer 的 key 不留 |
| SEM-DAT-002 | `configs/fetch_pipeline.example.json:77-107` mode/resume/metrics/recency等 | 多数只声明/打印，形成假开关；full-init可全库清空 | local只暴露catalog 1–1440分钟与Open 1–3600秒；public固定10分钟/1秒；内部策略强类型 | E / `REMOVE` / `HIST` | schema/docs/fetch/poller | 清除无consumer假开关与full-init；保留双refresh checkpoint/counter而非内部log path |
| SEM-DAT-003 | `configs/fetch_pipeline.schema.json` 已有真实消费字段 | JSON schema 描述targets、timeout等 | Rust typed config保持单一真相，并按local/public profile固定刷新政策 | R / `PORT` / `BASELINE_SHARED` | example/launcher/fetch/poller docs | unknown key fail-fast；local range/public fixed值、无静默clamp、真实数据冲突回Review |
| SEM-DAT-004 | `configs/fetch_pipeline.schema.json` resume/queue/recency/metrics等未实现字段 | schema 给出能力表面但执行链缺失 | 不进入当前本地用户配置或 runtime | E / `REMOVE` / `HIST` | docs/example | schema/示例/文档/代码四面零假字段 |
| SEM-DAT-005 | `data/schema.sql:2-241,341-343` term/campus/subject/course/section/meeting/core/status/FTS | 丰富关系模型；subject key、section key、meeting语义和FTS维护有缺陷 | 作为 Rust/SQLite 起点，修复 key、meeting classification、FTS与数据生命周期 | R / `PORT` / `BASELINE_SHARED` | migrate/fetch/query/poller/tests | migration+ingest+query golden tests；真实 ingest 后 FTS可查；open聚合一致 |
| SEM-DAT-006 | `data/schema.sql:116-231` section identity 与 detail fields | `(term,index)`唯一；大量详情列/JSON存在，关联表部分未填 | 内部 surrogate + 唯一 `(term,campus,index)`；逐字段可靠性；支持完整详情 | R / `PORT` / `BASELINE_SHARED` | normalizer/ingest/course query/section stub | collision fixture；detail字段从 ingest→DB→API→UI 逐字段验收 |
| SEM-DAT-007 | `data/schema.sql:177-198` meeting rows | 无 required/optional、exam/special-date、unknown classification | 显式 `occurrence_kind`、`requiredness`、时间知识状态；原始字段保留可追溯 | R / `REWRITE` / `BASELINE_SHARED` | normalizer/query/filter | known/unknown/async/hybrid/optional/exam fixtures；不能凭空把TBA当match |
| SEM-DAT-008 | `data/schema.sql:243-280` subscriptions/events | 持久个人 contact、verification、token、active状态与偏好 | 当前 runtime/schema 不持久个人 active watch | E / `REMOVE` / `HIST` | subscription route/poller/mail/UI/tests | migration必须安全清除/不新建；包、docs、API和测试无个人 subscription 表依赖 |
| SEM-DAT-009 | `data/schema.sql:281-318` snapshot/open_events | 可记录open snapshot与状态事件；event含edge/dedupe形状 | 保留状态/诊断；每attempt新RefreshObservation，valid/safe reconcile才有OpenObservation；ONE_SHOT不按edge抑制，CONTINUOUS用per-section episode | I / `SPLIT` / `BASELINE_SHARED` | ingest/poller/browser episode/history | failure保留last-known且不转episode；transport replay幂等；unsafe empty不Closed；状态/episode/audible分层 |
| SEM-DAT-010 | `data/schema.sql:319-339` open_event_notifications | DB fanout到 email/local_sound、sent/retry | 当前 live WS fanout不落个人队列 | E / `REMOVE` / `HIST` | claim/mail dispatcher/poller | schema、poller、workers、tests、docs、templates完整删除闭包 |
| SEM-DAT-011 | `data/migrations/001_init_schema.sql` course/data objects | 初始数据schema与当前 schema部分重复 | 拆出获批课程/section/meeting/FTS初始迁移 | R / `PORT` / `BASELINE_SHARED` | migrate script | checksum/version migration tests |
| SEM-DAT-012 | `data/migrations/001_init_schema.sql` personal subscription/mail objects | 初始迁移把旧通知模型写入所有新DB | 不进入新本地数据库 | E / `REMOVE` / `HIST` | schema/routes/workers | clean DB table allowlist断言 |
| SEM-DAT-013 | `data/migrations/003_open_events.sql` snapshot/status objects | 加入 open snapshot/event | 仅保留状态与诊断部分 | I / `PORT` / `BASELINE_SHARED` | poller/query | 与 status reconciliation tests 对齐 |
| SEM-DAT-014 | `data/migrations/003_open_events.sql` notification fanout | 加入 DB个人通知队列和dedupe | 不进入当前产品 | E / `REMOVE` / `HIST` | mail/claim/poller | clean DB无fanout table；历史仅留Git证据 |
| SEM-DAT-015 | `data/schema.sql:243-318` 可借鉴事件时间字段，但旧表含个人订阅/fanout语义 | 没有满足新本地history的安全旧schema | 新建`LOCAL_ONLY`无contact episode/watch summary与prefs store；不保存active，Reset可清除 | R / `REWRITE` / `LOCAL_ONLY` | local user-state API/browser history/UI/migrations | 跨启动、每Open更新last/count、Closed→Open新episode、无静默TTL、Reset stop-watch、包内零用户DB |

## 4. 混合产品文档

| ID | path:anchor / 语义单元 | 当前行为 | 目标行为 | 能力 / 处置 / 归属 | inbound / outbound | replacement、清除闭包与验证 |
|---|---|---|---|---|---|---|
| SEM-DOC-001 | `docs/deployment_playbook.md` 本地/WSL开发命令 | Node/npm/Git Bash/WSL运行旧栈 | 不作为普通用户文档；仅抽取故障教训给内部开发 | I / `EXTRACT` / `INTERNAL_TOOLING` | setup/run scripts | 不进入本地包；P3开发文档按Rust重写 |
| SEM-DOC-002 | `docs/deployment_playbook.md` Linux服务/运维意图 | 旧Node部署形状 | 留给P4的 `PUBLIC_DELTA / CARRY_TO_P4`，不在P2决定生产实现 | I / `EXTRACT` / `PUBLIC_DELTA` | Linux scripts | P4重新验证 systemd/Caddy/backup/rollback；本地包不含 |
| SEM-DOC-003 | `docs/notify_runbook.md` open/poller诊断 | 含部分poller操作和指标 | 抽取通用状态、连接、诊断方向 | I / `EXTRACT` / `BASELINE_SHARED` | poller/docs | 新运行手册用live WS与无PII日志重写 |
| SEM-DOC-004 | `docs/notify_runbook.md` persistent queue/mail/HTTP claim | 把DB订阅、claim和mail写成当前路径 | 不进入当前产品文档 | E / `REMOVE` / `HIST` | old routes/workers | 文档、route、schema、worker、frontend、package闭包 |
| SEM-DOC-005 | `docs/open_event_spec.md` status observation/miss/resilience | 提供状态采样和空响应讨论 | 可作poller contract证据，必须加异常空响应/circuit breaker | I / `PORT` / `BASELINE_SHARED` | poller/tests | fake Rutgers 200-empty/error/timeout fixtures |
| SEM-DOC-006 | `docs/open_event_spec.md` edge、3分钟dedupe、fanout | 旧单一Closed→Open/3分钟bucket控制所有提醒 | 旧bucket/last_notified/fanout语义不进入当前产品；新模式分层由SEM-DAT-009与SEM-WATCH-010/012替代 | E / `REMOVE` / `HIST` | poller/browser episode/schema/history | 删除旧doc/runtime语义但保留历史证据；replacement tests覆盖observation/episode/audible/replay |
| SEM-DOC-007 | `docs/subscription_model.md` section target、max=3 | 记录section scope、max默认3 | 重写为connection live watch、ONE_SHOT Max audible静默不停止、CONTINUOUS episode、本地history/public ephemeral | R / `REWRITE` / `BASELINE_SHARED` | API/UI/poller/local store | 与SEM-API-006及`06`§7–10一致 |
| SEM-DOC-008 | `docs/subscription_model.md` contact/email/persistent/quiet/snooze/history/waitlist | 旧个人持久订阅产品 | 旧contact/email/active/quiet/snooze/waitlist/history语义全部排除；新LOCAL_ONLY无身份history由SEM-DAT-015替代 | E / `REMOVE` / `HIST` | schema/API/UI/mail/local store | 旧链final docs/package零残留；replacement不得复用旧身份/active语义 |
| SEM-DOC-009 | `docs/ui_flow_course_list.md` course/filter/detail状态 | course list、筛选、loading/empty等有行为价值 | 重写为course-centered、匹配sections、独立section route和uncertain候选 | R / `REWRITE` / `BASELINE_SHARED` | frontend state/components | UI/API/query/test逐字段映射；真实desktop/mobile验证 |
| SEM-DOC-010 | `docs/ui_flow_course_list.md` Calendar | 周课表进入UI flow | 当前核心不含；只保留future记录 | F / `DEFER` / `FUTURE` | SchedulePreview/i18n | 当前src/runtime/package无Calendar入口或文案 |
| SEM-DOC-011 | `docs/ui_flow_course_list.md` Compact view | 文档候选，无明确当前用户批准 | 不作为当前命名功能；正式UI自行决定响应式信息密度 | E / `REMOVE` / `HIST` | UI docs only | 不出现假开关/死组件 |
| SEM-DOC-012 | `docs/ui_flow_course_list.md` share links | 文档候选、无production闭环 | 用户明确排除命名Share功能；current runtime/package零stub | E / `REMOVE` / `HIST` | URL helper/task history | 只保护direct section route和API内部query构造，不恢复filters或Share UI |
| SEM-DOC-013 | `docs/ui_flow_course_list.md` Save view、PresetManager、unsaved preset changes | 只有架构/交互候选，无production闭环 | 用户已明确Saved views为本地包`REQUIRED / LOCAL_ONLY`；抽取命名preset/apply/dirty交互意图，按`06`§7.4重写 | R / `EXTRACT` / `LOCAL_ONLY` | local FilterPanel/courseFilters/user-state adapter | 不把设计文档冒充实现；与Share/URL restore分离；local CRUD/migration与public capability-absence tests |

## 5. Frontend manifest、composition、API/types 与 i18n

| ID | path:anchor / 语义单元 | 当前行为 | 目标行为 | 能力 / 处置 / 归属 | inbound / outbound | replacement、清除闭包与验证 |
|---|---|---|---|---|---|---|
| SEM-FE-001 | `frontend/package.json:11-17` React/i18n | 有真实源码消费者 | 保留共享React与至少`en-US`/`zh-CN` i18n | R / `REUSE_WITH_FIXES` / `BASELINE_SHARED` | Vite/src | clean build、license/dependency audit、locale/key parity |
| SEM-FE-002 | `frontend/package.json:16,21` `react-window`及types | 零源码import | 当前分页结果无证明消费者，不保留预装依赖 | I / `REMOVE` / `INTERNAL_TOOLING` | lock only | package+lock同步删除；依赖消费者为0 |
| SEM-FE-003 | `frontend/package.json:6-9,18-24` Vite/TS toolchain | build存在，无test/lint；launcher仅跑dev | 保留构建工具，补contract/unit/e2e/a11y入口 | I / `REUSE_WITH_FIXES` / `INTERNAL_TOOLING` | vite config/tsconfig | clean install/typecheck/build/test在P7执行；vendor不进包 |
| SEM-FE-004 | `frontend/package-lock.json` React/i18n/Vite图 | 锁定所需依赖 | 随manifest保留并可复现 | I / `REUSE_WITH_FIXES` / `INTERNAL_TOOLING` | npm | lock与manifest一致，无mail/unused dependency |
| SEM-FE-005 | `frontend/package-lock.json` react-window subtree | 仅由无消费者直接依赖引入 | 删除 | I / `REMOVE` / `INTERNAL_TOOLING` | package manifest | lock审计证明无残留 |
| SEM-FE-006 | `frontend/i18n/messages.json` search/filter/course/section/language/common keys | 中英结构有价值，部分英文硬编码、未消费key | 固定`en-US`/`zh-CN`，补齐共享course/section/uncertain/watch/audio/refresh/reset全状态；Rutgers raw不翻译；本地Saved views namespace由SEM-FE-023隔离 | R / `REUSE_WITH_FIXES` / `BASELINE_SHARED` | i18next/types/components/docs | key parity、locale格式、public system detect/local persistence、missing/orphan、visual tests；public无Saved views keys |
| SEM-FE-007 | `frontend/i18n/messages.json:256-293,369-425,699-736,812-868` email/mail keys | 真实bundle可达 | 删除当前catalog命名空间 | E / `REMOVE` / `HIST` | Mail UI/subscription UI | source/bundle string audit零SendGrid/SMTP/email UI key |
| SEM-FE-008 | `frontend/i18n/messages.json:215-218,426-432,658-661,869-875` Calendar/schedule keys | future文案 | 移出当前runtime catalog；future只保留治理记录 | F / `DEFER` / `FUTURE` | SchedulePreview/README | 当前bundle无Calendar表面 |
| SEM-FE-009 | `frontend/i18n/messages.json` fetch/crawler/log keys | 暴露内部job/log path | 重写为双refresh时间截点、attempted/succeeded/failed/empty、local run/today/public today计数 | R / `REWRITE` / `BASELINE_SHARED` | DataFetch/App/index | 不显示内部log path；fresh/error/empty/unchanged/partial/counter/时区状态验证 |
| SEM-FE-010 | `frontend/src/App.tsx:17-121` shared shell/search/language/course | 唯一页面壳；无router、section detail | 保留composition思想，增加course/section routes和完整状态 | R / `REUSE_WITH_FIXES` / `BASELINE_SHARED` | main/components/hooks | direct URL reload、course-centered、section search/detail、responsive/a11y |
| SEM-FE-011 | `frontend/src/App.tsx:77-84` DataFetch card | 普通UI显示crawler管理 | 显示catalog/Open两种checkpoint和计数；local可配interval/safe retry，public固定且用户刷新不触发上游 | R / `REWRITE` / `BASELINE_SHARED` | DataFetch/fetch/status APIs | local ranges、public fixed、每attempt新refresh结果、valid pull新status、multi-scope、failure freshness/counters |
| SEM-FE-012 | `frontend/src/App.tsx:12,122-125` MailSettings | 生产入口直接挂载mail UI | 删除 | E / `REMOVE` / `HIST` | MailSettings/admin/i18n/CSS | final App import/render和bundle route/style零残留 |
| SEM-FE-013 | `frontend/src/App.tsx:13-14,96-100,124` subscription components | 持久subscribe/manager与mail混合 | connection-scoped watch manager + package-specific user-state adapter；local history/Reset，public每页默认 | R / `REWRITE` / `BASELINE_SHARED` | Center/Manager/hook/local API | 最多9、explicit start/stop、public reload empty、local selected/history restore但not active、Reset stop all |
| SEM-FE-014 | `frontend/src/api/notifications.ts` HTTP claim | POST claim | 删除协议；新WS adapter | E / `REMOVE` / `HIST` | sound hook/client | route/client/bundle零claim；WS protocol tests |
| SEM-FE-015 | `frontend/src/api/subscriptions.ts` persistent HTTP CRUD | subscribe/unsubscribe/list | 删除旧协议；watch start/stop走同一WS connection | E / `REMOVE` / `HIST` | three subscription components | replacement contract frame tests |
| SEM-FE-016 | `frontend/src/api/types.ts:1-133` course/filter/fetch types | 丰富course rows；meeting缺unknown/requiredness；fetch job暴露path；delivery三类会丢async | 扩展tri-state/detail、Delivery两轴/raw provenance和RefreshObservation/counters；去内部path | R / `REUSE_WITH_FIXES` / `BASELINE_SHARED` | hooks/components/APIs | Rust generated/golden/real-data contract；unknown/other/unspecified不静默丢弃 |
| SEM-FE-017 | `frontend/src/api/types.ts:135-223` persistent subscription/local claim | email/contact/quiet/snooze/claim DTO | 以selected/active、OpenObservation/OpenEpisode、Max audible、local history/reset DTO替代 | R / `REWRITE` / `BASELINE_SHARED` | old UI/hook/local adapter | type-level禁止email/contact/token/persistent active；public bundle无history persistence adapter |
| SEM-FE-018 | `frontend/src/api/types.ts:230-290` mail admin types | SendGrid/SMTP config | 删除 | E / `REMOVE` / `HIST` | admin/MailSettings | TS refs和bundle零mail config |
| SEM-FE-019 | `frontend/src/App.tsx`与subscription组件缺少包级用户状态边界 | 当前browser storage会让public/local语义漂移 | 新建ephemeral public current-state adapter和LOCAL_ONLY prefs/history/saved-view store；Reset显式stop并清user data；public adapter不得暴露Saved views | R / `REWRITE` / `BASELINE_SHARED` | bootstrap/watch/history/settings/local Saved views | public current defaults且Saved capability零存在；local restart/Reset；active永不恢复 |
| SEM-FE-020 | `frontend/i18n/messages.json` Playground keys与`index.html` Playground title | orphan/dev命名进入正式产品表面 | 从current catalog/document/bundle删除；正式title由SEM-FE-009替代 | E / `REMOVE` / `HIST` | App/index/build | source与compiled bundle零Playground产品命名 |
| SEM-FE-021 | `FilterPanel.tsx/.css` header Reset、active chips与旧sticky/responsive layout | 有可用筛选反馈骨架，但无Save/PresetManager UI | 抽取共享active chips、普通filter Reset与sidebar/drawer信息架构；预留显式LOCAL_ONLY扩展边界但public不渲染入口 | R / `EXTRACT` / `BASELINE_SHARED` | courseFilters/App/TagChip/i18n | desktop/mobile/keyboard/a11y；普通Reset只清当前filters；public DOM/route/bundle无Saved入口 |
| SEM-FE-022 | 当前frontend缺失Saved views manager | 旧设计无真实CRUD/storage，不能直接复用 | 新建LOCAL_ONLY manager：save/apply/rename/update/duplicate/delete/delete-all、clean/modified/incompatible/revision-conflict；普通Reset保库、delete-all保当前filters | R / `REWRITE` / `LOCAL_ONLY` | local shell/FilterSchema/user-state API | CRUD/CAS/keyboard/a11y；public source graph、DOM、route、bundle零Saved功能 |
| SEM-FE-023 | 当前i18n缺失Saved views完整状态 | 旧messages没有可验收的双语Saved manager contract | 新建LOCAL_ONLY `en-US`/`zh-CN` Saved views namespace；view name与Rutgers raw保持原文 | R / `REWRITE` / `LOCAL_ONLY` | local Saved manager/error states | key parity、quota/conflict/incompatible/reset文案；public compiled catalog零该namespace |

## 6. Subscription、toast 与声音 UI

| ID | path:anchor / 语义单元 | 当前行为 | 目标行为 | 能力 / 处置 / 归属 | inbound / outbound | replacement、清除闭包与验证 |
|---|---|---|---|---|---|---|
| SEM-WATCH-001 | `LocalSoundToggle.tsx:39-49` + CSS toggle/status | 可启停本地poll；状态壳有价值 | 变为active watch声音设置，显示连接/audio状态 | R / `REUSE_WITH_FIXES` / `BASELINE_SHARED` | Center/sound hook/i18n | keyboard、screen reader、blocked audio、disconnect states |
| SEM-WATCH-002 | `LocalSoundToggle.tsx:21-29` polling文案和相关CSS | 暴露错误HTTP polling架构 | 删除；使用connecting/active/stopped/error | E / `REMOVE` / `HIST` | i18n/hook | source/bundle无polling claim文案 |
| SEM-WATCH-003 | `LocalSoundToggle.tsx:85-118` toast markup/styles | 一批最多5个toast，可手动关 | 新Open episode产生alert；同episode后续观察更新last/time count；显示section与mode状态 | R / `REUSE_WITH_FIXES` / `BASELINE_SHARED` | sound hook/history list | Max audible与visible queue分离；每秒不堆栈但observation不丢；a11y live region tests |
| SEM-WATCH-004 | `SubscribeButton.tsx:26-102,166-216` section/feedback壳 + CSS | orphan组件，可记一部分contact | 从course/section context添加/移除selected section；不手输任意index | R / `EXTRACT` / `BASELINE_SHARED` | currently no inbound | 集成到card/detail；复合key；第10项明确拒绝 |
| SEM-WATCH-005 | `SubscribeButton.tsx:103-165,217-337` email/token/persistent flow + CSS | email-only submit、token storage、unsubscribe | 删除 | E / `REMOVE` / `HIST` | API/types/storage/i18n | storage/API/CSS/i18n/bundle闭包 |
| SEM-WATCH-006 | `SubscriptionCenter.tsx` section selection/feedback/local sound shell + CSS | 手输index、另开claim轮询 | 重写为selected≤9、批量/逐项start-stop、connection、Max audible、continuous duration/confirm | R / `REWRITE` / `BASELINE_SHARED` | WS/sound/history adapter | route context、9-limit、explicit start、public/local selection、A/C/D tests |
| SEM-WATCH-007 | `SubscriptionCenter.tsx:22-123,157-217` email contact branches + CSS | 默认email、contact type持久化 | 删除 | E / `REMOVE` / `HIST` | storage/API/types/i18n | localStorage migration清除旧email/token keys |
| SEM-WATCH-008 | `SubscriptionManager.tsx:10-160` list/refresh/remove shell + CSS | mount拉全局DB active rows | 复用列表交互；数据改为当前selected/connection active、episode/静默状态与local history | R / `REWRITE` / `BASELINE_SHARED` | WS/types/local store | selected vs active、disconnect、cap silent-not-stop、confirm/resume、Reset tests |
| SEM-WATCH-009 | `SubscriptionManager.tsx:95-128` email channel展示 + CSS modifier | 渲染email/local_sound | 只有WebUI live watch | E / `REMOVE` / `HIST` | i18n/types | source/style/bundle零email channel |
| SEM-WATCH-010 | `useLocalSoundNotifications.ts:111-175` AudioContext、oscillator、toast state | 固定gain/0.35s；一个batch只响一次 | 抽取audio unlock/shared mixer；ONE_SHOT逐observation至cap，CONTINUOUS按episode loop/confirm/timeout | R / `REWRITE` / `BASELINE_SHARED` | LocalSoundToggle/WS observation | burst、cap静默、10m/Unlimited、A/C/D、Closed→Open、audio failure browser tests |
| SEM-WATCH-011 | `useLocalSoundNotifications.ts:7-11,58-102,177-223` device、enabled storage、7s claim | reload自动恢复active；HTTP poll/退避 | 旧device/enabled/claim/active persistence整段删除；新package-state adapters由SEM-FE-019替代 | E / `REMOVE` / `HIST` | user-state adapters/WS | 旧storage key migration清除；public reload、local restore与Reset由replacement tests覆盖 |
| SEM-WATCH-012 | `useLocalSoundNotifications.ts:161-173` batch播放 | 多条通知只触发一次tone | ONE_SHOT每条可发声Open observation独立trigger；CONTINUOUS同episode只维持一个shared alarm | R / `REWRITE` / `BASELINE_SHARED` | WS observation/episode state | N observations=N one-shots至cap；同episode确认后不重响；新D立即响 |

## 7. Filter state 混合语义

| ID | path:anchor / 语义单元 | 当前行为 | 目标行为 | 能力 / 处置 / 归属 | inbound / outbound | replacement、清除闭包与验证 |
|---|---|---|---|---|---|---|
| SEM-FLT-001 | `courseFilters.ts:18-43` current filter state | course与section字段混合；单一全局time range；delivery仅三类且会丢async；无core mode/uncertain | 采用`06`逐字段、Delivery modality/synchronicity两轴、same-section、availability windows和tri-state contract | R / `REUSE_WITH_FIXES` / `BASELINE_SHARED` | App/FilterPanel/query hook | serializer/query/UI/real-data fixtures一一对应；raw/other/unspecified不丢 |
| SEM-FLT-002 | `courseFilters.ts:84-145` REST query builder | 重复key、多字段有价值；meetingDays拼接、section fields不全 | 保留序列化价值，生成共享contract query；不得client后过滤破坏分页 | R / `REWRITE` / `BASELINE_SHARED` | useCourseQuery/API | property/golden tests；same-section witness由server统一执行 |
| SEM-FLT-003 | `courseFilters.ts:151-299` URL serialize/parse | orphan、校验不足；会让公网reload恢复filters并形成隐式Share表面 | 拆分：保留API内部query serialization与direct section route构造；删除browser filter URL restore/share keys；本地Saved views只走local store | I / `SPLIT` / `BASELINE_SHARED` | currently no inbound | public reload必须默认filters且无Saved capability；direct section reload可达；两包都无browser-URL save/share/filter-state roundtrip |
| SEM-FLT-004 | `courseFilters.ts:1-4` calendar view注释与`uiStatus` | 旧Calendar耦合和无消费者UI状态 | 删除未消费状态/旧文档引用；UI loading/error由query machine承载 | E / `REMOVE` / `HIST` | FilterPanel/dev | type/ref搜索零dead field；不得误删SEM-FLT-005本地Saved views价值 |
| SEM-FLT-005 | `courseFilters.ts` `dirtyFields`与`FilterPanel.emitState` dirty-key调用 | 已追踪部分改动但无PresetManager，且手工key会漏掉以后新增筛选 | 在本地扩展中用共享versioned FilterSchema生成canonical snapshot并与last saved/applied view比较，得到clean/modified；迁移/unknown字段可解释 | R / `REWRITE` / `LOCAL_ONLY` | local FilterPanel/SavedViews manager/storage；shared FilterSchema | 任一注册filter变化都标modified；apply/save变clean；new-field/migration/property tests；public source graph无dirty association |

## 8. Root manifests、launcher 与 poller

| ID | path:anchor / 语义单元 | 当前行为 | 目标行为 | 能力 / 处置 / 归属 | inbound / outbound | replacement、清除闭包与验证 |
|---|---|---|---|---|---|---|
| SEM-RUN-001 | `package.json:10-21` scripts | Node CLI入口；test固定失败；无build/package | 保留算法验证的迁移期参考；最终由Cargo/前端build/测试/包manifest替代 | I / `REWRITE` / `INTERNAL_TOOLING` | launcher/dev | P3定义，不在P2写Cargo；P7统一入口必须通过 |
| SEM-RUN-002 | `package.json:40-45` better-sqlite3/Fastify/Zod | 当前Node runtime直接消费者 | 行为、SQL、validation移植；这些包不进入最终runtime | I / `REMOVE` / `INTERNAL_TOOLING` | api/scripts/workers | 每个消费者在reuse矩阵有目标；最终dependency/package audit零backend Node deps |
| SEM-RUN-003 | `package.json` repository metadata | 指向旧错误repo | 修正公开metadata | I / `REWRITE` / `INTERNAL_TOOLING` | npm/docs | public content review |
| SEM-RUN-004 | `package-lock.json` backend/runtime graph | 锁定旧Node backend/mail消费者 | 迁移期只作可复算证据；最终不进入本地包 | I / `REMOVE` / `INTERNAL_TOOLING` | root package | dependency consumer closure；无孤立package |
| SEM-RUN-005 | `tsconfig.json` backend/scripts include | 不含frontend；无法形成全repo验证 | 迁移期内部工具；最终Rust+frontend验证分离 | I / `SPLIT` / `INTERNAL_TOOLING` | root TypeScript | P3测试计划承接；本地用户包不含 |
| SEM-RUN-006 | `oneclick_start.js:110-242,367-419,422-435,549-563` entry/config/supervision/open-browser/error | 有用户路径、配置、child shutdown、open browser价值；首次fetch判断错误 | 将可用行为移植进`bcsp-local.exe`及薄`.bat`；首次数据状态安全明确 | R / `PORT` / `LOCAL_ONLY` | `.bat/.command`、npm scripts | clean Windows无Node、重复启动、port冲突、路径/Unicode、stop/restart tests |
| SEM-RUN-007 | `oneclick_start.js:269-365,469-515` npm install/native repair/Fastify/Vite dev多进程 | 需要Node/network/C++工具，运行dev server | 不进入最终一键包 | E / `REMOVE` / `HIST` | package/frontend/workers | package content和clean-machine测试证明无Node/npm/native build |
| SEM-RUN-008 | `oneclick_start.js:7,17-29,520-547` mail检查/dispatcher | mail真实启动链 | 删除 | E / `REMOVE` / `HIST` | mail scripts/config/worker | launcher source/bundle/package无mail refs |
| SEM-POLL-001 | `open_sections_poller.ts:394-610,812-1050` targets、checkpoint、poll loop、metrics | 有target coalesce、resume、miss阈值；target来自持久订阅且5分钟rescan | 复用poll/resilience/metrics；target来自已批准catalog/service scope而非live watch；local 1–3600s/public fixed1s；每attempt写refresh checkpoint/counter | R / `PORT` / `BASELINE_SHARED` | SOC client/DB/WS/query | 无watch仍刷新Open状态；多watch不增加Rutgers poll；valid pull才更新section status；atomic state；run/today/restart tests |
| SEM-POLL-002 | `open_sections_poller.ts:1099-1249` status reconcile、edge/reminder/bucket | 只在edge/3分钟生成并去重 | 每个attempt产生RefreshObservation；只有valid/safely-reconciled pull产生per-section observationId；ONE_SHOT/CONTINUOUS由consumer分层 | R / `REWRITE` / `BASELINE_SHARED` | DB/fanout/browser | 持续Open连续observation、failure保留last-known且不转episode、exact replay幂等、unsafe empty不Closed、mode split |
| SEM-POLL-003 | `open_sections_poller.ts:706-748,1225-1332` persistent subscription preference/fanout | 查DB active、quiet/snooze/last_notified，写notification队列 | 删除个人持久状态/DB fanout；直接广播所有live observations；不得应用Max audible或停止watch | E / `REMOVE` / `HIST` | subscriptions/mail/claim/schema/browser | DB/API/worker/docs/tests闭包；fanout持续、内存watch cleanup、local history独立consumer |
| SEM-POLL-004 | `open_sections_poller.ts:1086-1198` empty/miss handling | 200空数组连续两次可把全部open标closed | 错误、异常空、真实零open必须区分；circuit breaker/可信阈值 | R / `PORT` / `BASELINE_SHARED` | SOC response/status DB | fake upstream empty/error/recovery tests；不得大规模误关 |
| SEM-POLL-005 | `open_sections_poller.ts:584-589,1131-1186` section status update | 只更新sections，不同步course denormalized count | 查询从同一source计算或事务同步course aggregate | R / `REWRITE` / `BASELINE_SHARED` | course query | poll后course filter/summary/detail一致性测试 |
| SEM-POLL-006 | `open_sections_poller.auto.test.ts` target/missing/checkpoint fixtures | 有target coalesce/start-stop/missing dataset价值 | 抽取为Rust poller resilience tests | I / `PORT` / `INTERNAL_TOOLING` | poller/SOC client | 保留稳定fixture，移除DB persistent subscription前提 |
| SEM-POLL-007 | `open_sections_poller.auto.test.ts` active DB discovery/persistent resume assertions | 固化旧个人active模型 | 删除并反向测试断连不恢复active | E / `REWRITE` / `INTERNAL_TOOLING` | schema/subscription route | process restart后selected可由browser保存但server active为空 |

## 9. 覆盖结论

- CSS 混合文件的精确归属：
  - `frontend/src/components/LocalSoundToggle.css` → `SEM-WATCH-001`～`003`
  - `frontend/src/components/SubscribeButton.css` → `SEM-WATCH-004`～`005`
  - `frontend/src/components/SubscriptionCenter.css` → `SEM-WATCH-006`～`007`
  - `frontend/src/components/SubscriptionManager.css` → `SEM-WATCH-008`～`009`
- `MIXED_SEE_02` 文件：**38/38 已下钻**。
- 本矩阵语义单元：**91**，每项均有三轴结论、目标消费者/替代面和验证方向。
- 没有使用“技术栈不同”作为整文件重写理由；React utility、UI壳、SQL/query、normalizer、migration、poller resilience、测试fixture均保留了独立复用途径。
- 没有把 `PUBLIC_DELTA` 当删除：Linux运维表面只携带到 P4，不进入当前本地包。
