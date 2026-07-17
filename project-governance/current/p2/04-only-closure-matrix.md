# P2 ONLY 清除矩阵

## 1. 清除层级

| 层级 | 含义 |
|---|---|
| `REMOVE_FROM_LOCAL_PACKAGE` | 可以仍是内部工具、历史证据或公网delta，但绝不能进入Windows用户制品 |
| `REMOVE_FROM_CURRENT_PRODUCT` | 不得进入当前普通用户runtime、route、UI、schema行为、文档或测试主链 |
| `REMOVE_FROM_REPOSITORY` | 在P7迁移/验证完成后从当前源码树删除；历史由Git/治理记录保留 |
| `MOVE_TO_HISTORICAL_EVIDENCE` | 只保留审计/报告价值，不能被编译、启动或打包 |
| `CARRY_TO_P4` | 不属于本地包，但属于明确公网delta；不是删除 |

P2只作裁决，不执行删除。

## 2. 非目标传递闭包

| ONLY ID / 非目标 | UI | API / protocol | worker | config / secret | schema / data | docs | tests | dependency | startup | package/runtime residue | shared-code保护 | 最终层级 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| ONLY-001 Email/SendGrid/SMTP | `MailSettingsPanel.*`；App挂载；Center/Manager/SubscribeButton email branches；mail i18n/CSS | `/admin/mail-config`；subscription email/contact/verification/token | `mail_dispatcher.ts` | mail example/user config；8 templates；任何key/env | personal contact、verification、notification fanout tables/columns | 3 mail docs；README/notify/subscription/mail段 | admin/mail/provider/retry/template/dispatcher tests | 删除mail专属消费者；generic limiter只有抽取后才可留 | launcher、run/setup脚本的mail import/autostart | 旧dist与三个archives含mail；final source/bundle/package string audit | generic retry/token-bucket可迁移到Rutgers client，但不得携带mail types/provider | product code/assets/docs/tests/config `REMOVE_FROM_REPOSITORY`；历史report可`MOVE_TO_HISTORICAL_EVIDENCE` |
| ONLY-002 Persistent personal active subscription | Center/Manager/SubscribeButton的DB active/contact/token语义 | persistent subscribe/list/unsubscribe；id-only cancel | poller从DB发现active、5min rescan | contact/device/active localStorage keys | subscriptions/events个人active状态 | subscription/notify/open-event旧模型 | subscriptions/poller auto/local claim tests | 不需要账号/contact依赖 | launcher启动poller恢复DB active | local DB跨重启恢复、旧archives | section chooser/list/feedback壳与复合key fixture可EXTRACT | `REMOVE_FROM_CURRENT_PRODUCT`；保护的chooser/key fixture由live-watch/selection替代并通过测试后移除旧实现，不移植被排除语义 |
| ONLY-003 HTTP local notification claim | LocalSound hook的7s poll/polling文案 | `/notifications/local/claim`、device SHA-1、claim即sent | DB fanout生产claim rows | deviceId/enable storage | `open_event_notifications` sent/retry | notify runbook | claim tests | 无新增client polling库 | launcher无直接route但server注册 | dist含claim字符串 | notification DTO字段可EXTRACT为WS event | `REMOVE_FROM_REPOSITORY`，以WS替代 |
| ONLY-004 旧全局Closed→Open/3min dedupe | 旧UI批次一次tone | event/claim没有每观察Open语义 | reminder、bucket、last_notified抑制 | reminder interval | open_events mail/fanout dedupe | open event/notify/subscription旧文字 | poller tests需按模式拆分 | n/a | poller启动 | checkpoint/DB可能保留bucket状态 | 每attempt新RefreshObservation、每valid/safe pull新OpenObservation与transport幂等保留；ONE_SHOT无3min抑制；CONTINUOUS保留per-section episode | 旧通用bucket/reminder `REMOVE_FROM_CURRENT_PRODUCT`；不得误删CONTINUOUS episode |
| ONLY-005 Calendar current runtime | `SchedulePreview.*`、dev mock/playground、calendar i18n | 无route | 无 | 无 | 无 | UI flow/frontend README calendar段 | 无 | 无 | 无 | 当前tree-shaken但仍在src/typecheck | 可抽取历史设计，不进入当前UI | `FUTURE/DEFER`；从current product source/package移出，Git/issue保留 |
| ONLY-006 macOS current support | 无专属React UI | 无 | 无 | 无 | 无 | README/oneclick/quickstart中的Mac宣称 | 无真实Mac测试 | 无 | `Start-WebUI.command`、darwin branch | 三个archives含`.command` | `.bat`的切目录/错误处理不受影响 | `REMOVE_FROM_CURRENT_PRODUCT`和root current source；历史Git保留 |
| ONLY-007 Discord | 当前树允许范围零UI | 零route | 零worker | 零config | 零schema | 只在历史/获批P1记原因 | 零当前test | 零dependency | 零 | 旧Git/Compact历史，不在archives | 无需删除共享通知抽象，因为当前只有WS声音 | 当前repo/runtime/package保持零；`HISTORICAL_EVIDENCE` |
| ONLY-008 Web Push/native App/system notifications | 零 | 零 | 零 | 零 | 零 | 只记录非目标 | 零 | 零 | 零 | 零 | WebSocket普通浏览器能力保留 | 当前runtime/package保持零 |
| ONLY-009 Node/Fastify作为最终runtime | 当前Vite dev依赖launcher | Fastify server/routes/container | TS poller/runner | root package/tsconfig | better-sqlite3访问 | 所有Node启动说明 | 旧Node tests | better-sqlite3/Fastify/Zod/tsx/backend types | npm install、native rebuild、多进程 | 当前本地路径完全可达；旧archives全是源码包 | SQL、contracts、normalizer、tests、errors、poller resilience逐项PORT；React不删除 | 完成行为移植后 `REMOVE_FROM_CURRENT_PRODUCT/REPOSITORY/RUNTIME`；内部历史证据另存 |
| ONLY-010 npm-on-first-run/Vite dev用户路径 | 浏览器连接5174 dev | local API 3333 | child processes | 在线npm、Node22、Build Tools | release目录写DB风险 | README/oneclick | 无clean Windows | root/frontend vendor | `oneclick_start.js` | node_modules/Vite/dist漂移 | `.bat`薄壳、open browser、shutdown/error UX可EXTRACT | `REMOVE_FROM_CURRENT_PRODUCT`；`bcsp-local.exe`替代 |
| ONLY-011 普通用户15–120秒旧toggle/45秒旧cache | archive-only toggle；当前hook cache无UI | query repeat | 无 | 无 | 无 | 历史docs/overlay | 无 | 无 | 无 | remote/archive orphan；current Map cache | 保留新的双刷新：local catalog 1–1440m/Open 1–3600s、public fixed 10m/1s；每attempt新RefreshObservation/计数，valid pull才有OpenObservation/成功时间截点 | named旧toggle/current 45s cache `REMOVE`; archive-only表面`HIST` |
| ONLY-012 Named Compact view | docs候选 | 无 | 无 | 无 | 无 | UI flow | 无 | 无 | 无 | 无 | 正式响应式信息密度保留，不创建旧命名开关 | `REMOVE_FROM_CURRENT_PRODUCT`; 历史docs保留证据 |
| ONLY-013 Share links current功能 | 无production UI；URL helper orphan | 无 | 无 | 无 | 无 | UI flow | 无 | 无 | 无 | helper不进runtime入口 | URL helper可EXTRACT用于导航/direct section URL，但不得提供命名Share功能 | 用户明确`EXCLUDED`；current UI/API/runtime无stub |
| ONLY-014 Waitlist filter/alert | openStatus enum/旧偏好候选 | section stub enum、notifyOn waitlist | poller preference | 无 | raw status可存，但没有可靠、全校统一waitlist字段 | subscription/query旧文字 | old tests部分 | 无 | 无 | 无可靠上游链 | 原始unknown status/detail可保留；不能把Closed或raw hint映射waitlist | 用户明确`EXCLUDED`；current UI/API/runtime去除 |
| ONLY-015 Quiet hours/snooze/paused/suppressed与旧personal history | 无完整当前UI，旧types存在 | preferences/status machine | poller partial checks | contact/active prefs | subscriptions/events个人队列 | subscription docs | old tests | 无 | 无 | DB residue | 必须保护新`LOCAL_ONLY`无contact episode/watch history、prefs与Reset；不得保护旧active/contact/token/fanout | quiet/snooze/paused/suppressed及旧personal history `REMOVE_FROM_CURRENT_PRODUCT/REPOSITORY`；新本地history `REQUIRED` |
| ONLY-016 伪造fallback catalog/dictionary | fallback terms/campuses/subjects让错误看似可用 | filters吞错 | 无 | 静态hardcode | 无 | docs与代码漂移 | 缺错误/空字典tests | 无 | 无 | compiled bundle含过期值 | fallback UI结构/loading skeleton可留；真实值必须来自DB | runtime hardcoded data `REMOVE_FROM_CURRENT_PRODUCT`; dev fixture仅`INTERNAL_TOOLING` |
| ONLY-017 Playground/orphan/dead UI状态 | Playground、mock、unused SubscribeButton、dead `uiStatus/dirtyFields`、missing favicon/title | 无 | 无 | 无 | 无 | frontend README/Playground title | 无frontend tests | unused react-window | 无 | dist旧title/vite.svg | SubscribeButton section CTA、URL helper、mock fixtures可按目标EXTRACT | 无消费者部分`REMOVE`或`INTERNAL_TOOLING`; current package零 |
| ONLY-018 Internal probes/simulations/reports/tests进入用户包 | 无 | 无 | 无 | probe configs | sample/runtime outputs | SOC研究文档、reports | dev tests | dev deps | CLI tools | 历史archives曾带tests/scripts/checkpoint | 作为`INTERNAL_TOOLING/HISTORICAL_EVIDENCE`保留repo；可抽fixture | 全部`REMOVE_FROM_LOCAL_PACKAGE`，不等于删仓库 |
| ONLY-019 Runtime/generated/private residue | 无 | 可能暴露log path | checkpoints | `.secrets`, local/user config | DB/WAL/SHM/log/runtime JSON | 不公开private inventory | artifact regression tests | node_modules | none | `frontend/dist`, node_modules, DB sidecars, checkpoints, oldrelease, CLAUDE/untracked宽泛归档风险 | final dist必须从获批source重建；private只metadata审计 | `REMOVE_FROM_LOCAL_PACKAGE`; positive allowlist+secret scan |
| ONLY-020 旧archives作为当前包 | n/a | n/a | n/a | n/a | REL21含checkpoint | 包内README漂移 | 包含旧tests | 无vendor/exe | Node launchers | REL21/REL22分别125/136类内容，不自包含 | 只提取历史文件清单/失败记忆，不复制payload | `HISTORICAL_EVIDENCE`; 绝不重发/嵌套进新包 |
| ONLY-021 公网部署专属表面混入本地包 | 不分叉普通UI | 公网管理/安全delta待P4 | systemd service/runtime ops | production env/secret | centralized server data生命周期差异 | deployment playbook | public install/capacity tests | Linux deps | Linux setup/run scripts | public package/systemd/Caddy | shared core、React、protocol保留 | `CARRY_TO_P4`; 本地包`REMOVE_FROM_LOCAL_PACKAGE`，不是从产品删除 |
| ONLY-022 普通用户内部crawler/log path/危险full-init | DataFetchCard显示crawler/job log | unauth `/fetch`可写DB | child process runner | temp config/log | 全库DELETE其他scope风险 | README手动fetch | 缺multi-scope回归 | Node spawn | launcher first-fetch错误 | log/runtime JSON | 保护安全双refresh状态/重试、时间截点、local interval和public fixed policy；公网页面刷新不得放大为上游请求 | 内部细节/危险mode `REMOVE_FROM_CURRENT_PRODUCT`; 安全refresh UI `REWRITE` |
| ONLY-023 Saved views旧URL/auto-restore/stub实现方式 | 旧设计把Save与Share/URL并列；无production PresetManager | filter URL parse/restore候选 | 无 | 未定义browser storage/version/quota | 无versioned saved-view schema | `docs/ui_flow_course_list.md`候选 | 无CRUD/migration/package-state tests | 无 | 无 | dirtyFields/dead URL helper可进bundle | 保护共享FilterPanel/chips/FilterSchema与本地`REQUIRED / LOCAL_ONLY` Saved views；删除Share、URL restore、auto-apply、无实现stub，并从public source graph/artifact清除全部Saved views表面 | 旧机制`REMOVE/REWRITE`；由`06`§7.4 LOCAL_ONLY adapter替代；public无入口/API/storage/catalog/bundle |
| ONLY-024 Legacy `keywords/tags` no-op筛选 | 旧preset filter control/state候选 | query builder未发送；URL serializer keys孤立 | 无 | 无 | raw course tags仍可有来源/详情价值 | 历史UI/state声明 | 无端到端query test | generic `TagChip`仍被active-filter chips消费，必须保护 | 无 | orphan preset keys可进入bundle | 关键词真实价值由FLT-C04承担；只删legacy no-op control/state/query/URL keys，保护raw provenance与generic TagChip | legacy preset筛选表面`REMOVE_FROM_CURRENT_PRODUCT/REPOSITORY`；不得按名字机械删除组件/raw字段 |

## 3. Mail闭包的精确文件/符号集合

P7删除/迁移时至少要以本清单做反向引用和制品审计：

- `notifications/mail/**`
- `workers/mail_dispatcher.ts`
- `workers/tests/mail_dispatcher.test.ts`
- `scripts/mail_e2e_sim.ts`
- `scripts/mail_templates.js`
- `configs/mail_sender.example.json`
- ignored `configs/mail_sender.user.json`（只处理路径，不读取/提交正文）
- `configs/templates/email/**`
- `api/src/routes/admin.ts`
- `api/tests/admin_mail_config.test.ts`
- `api/src/server.ts`中的mail route注册
- `api/src/routes/subscriptions.ts`中的email/contact/verification/unsubscribe token
- `frontend/src/api/admin.ts`
- `frontend/src/components/MailSettingsPanel.*`
- `frontend/src/components/SubscriptionCenter.*`、`SubscriptionManager.*`、`SubscribeButton.*`的email分支
- `frontend/src/api/types.ts`的mail/contact类型
- `frontend/i18n/messages.json`的mail/email namespaces
- `data/schema.sql`与migrations的personal contact/mail fanout对象
- `workers/open_sections_poller.ts`的DB notification enqueue
- `scripts/oneclick_start.js`、`run_stack.sh`、`setup_local_env.sh`中的mail路径
- `docs/mail_*`、README、notify/subscription docs中的当前邮件说明
- `reports/mail_worker_latency.md`只移到历史证据，不进入产品/包
- ignored `frontend/dist/**`必须重建，不能逐字符串手改
- 三个旧archives仅保留历史，不作为清理目标或新包输入

Generic limiter/retry思路只有在去除全部mail types、provider、template、credential语义，并获得Rutgers client明确消费者后，才可作为`EXTRACT/PORT`保留。

## 4. Package 正向 allowlist 原则

最终本地一键包只能通过正向manifest生成，至少允许：

- `bcsp-local.exe`及确有运行消费者的必要动态库；
- 编译后的正式React静态assets；
- 薄 `Start-WebUI.bat`；
- 必需且无secret的默认配置/schema；
- quickstart、troubleshooting、release notes、license、checksums。

默认拒绝：源码、tests、reports、Compact/治理材料、chat-log、`.git`、`.ngagent`、`.secrets`、private inventory、node_modules、DB/WAL/SHM、runtime JSON/log/checkpoint、local/user config、旧archives、mail/calendar/macOS表面、内部probe/simulator。

## 5. ONLY 完成结论

- 已登记 **24** 个非目标/隔离域。
- 每个域都有UI/API/worker/config/schema/docs/tests/deps/startup/package去向或明确“当前零表面”的负证据。
- 邮件、持久active、HTTP claim、Node用户runtime完成了传递闭包；不会只隐藏按钮。
- `PUBLIC_DELTA`明确携带到P4，没有误删。
- shared-code保护已逐项指出，避免删除mail/persistence时误伤generic retry、section选择、本地episode history、UI列表、query/normalizer与双刷新等健康价值。
