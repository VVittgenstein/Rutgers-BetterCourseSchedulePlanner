# P3 本地一键包完整实现计划

> **FROZEN, AS AMENDED**：Catalog 子门覆盖 Fall 2026 全部 15 个当前 campus，并以 Summer 2026 的 6 个主/线上 scope 作为结构样本，共 21 个成功 payload。Open 两轮 42/42 次请求全部成功；Rutgers官方 term/campus set-membership/intersection、empty/error、双时钟、backoff、observation/counter 与通知前置语义已收口到冻结的 `23a/23b`。Windows 本地存储拓扑已由 2026-07-13 用户批准的 `P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001` 修订为包根相对路径下的单一运行数据库；该修订不改变任何 P3 Catalog/Open 证据或共享 Open 语义。本文件始终不授权 P7 实现。

## 1. 文档状态、权限与解释边界

- 阶段：P3
- 状态：`P3_LOCAL_ONECLICK_PLAN_FROZEN` — NOT P7 AUTHORIZATION
- P6 Review 修订：`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001` — INCORPORATED, P6 REVIEW STILL OPEN
- 交付对象：Windows 本地一键包
- 上游基线：P2 APPROVED — CLOSED
- 当前用途：作为 P4–P6 的本地 `BASELINE_SHARED + LOCAL_ONLY` 输入；不得作为 P7 的启动授权
- 实施边界：本文件不授权修改产品源码、构建包或发布；只有 P6 Review 批准后才可进入 P7 实现

本计划以当前权威 workflow、P2 ALL/ONLY 合同和 P3 Catalog 证据为准。旧 Node/Fastify/worker 代码只提供可移植算法与已知缺陷，不是最终运行时。

所有 Open 依赖统一消费 `23a/23b`；不得再恢复旧的 same-scope orphan hard failure、15秒 poller、两次 miss、Catalog boolean 实时状态或每用户上游轮询。

## 2. 权威输入与已观察事实

### 2.1 输入

1. project-governance/current/single-mainline-delivery-workflow.md
2. project-governance/current/p2/02-file-semantic-matrix.md
3. project-governance/current/p2/03-capability-all-matrix.md
4. project-governance/current/p2/04-only-closure-matrix.md
5. project-governance/current/p2/05-reuse-and-port-matrix.md
6. project-governance/current/p2/06-filter-section-watch-contract.md
7. project-governance/current/p2/07-p2-validation-and-review-gate.md
8. project-governance/current/p3/00–06 Catalog/Delivery/identity 证据产物
9. project-governance/current/p3/10a–22b Open manifest、两轮 evidence、Review、amendment 与 completion 链
10. project-governance/current/p3/23a–23b Shared Open Final Contract

### 2.2 P3 Catalog 事实

| 事实 | 当前观察 | 设计约束 |
|---|---:|---|
| 可用 Catalog payload | 21 | Fall 15 个当前 campus + Summer 6 个结构样本；每个 target 独立 staging、校验、提交和 checkpoint |
| 当前官方 selector 启用 campus | 15 | runtime 不永久硬编码 campus；保存 source/version/freshness |
| Course raw rows | 10,629 | raw provenance 不进入最终包，但规范化必须可追溯 |
| Section raw rows | 22,069 | 不按裸 index 建模 |
| Meeting raw rows | 30,804 | 具体空值分布以 04/06 的机器产物为准，不能自动推断 async |
| Instructor assignments | 19,202 | 只观察到 name，没有稳定 instructor ID |
| term+campus+index 唯一值 | 22,051 | 外部 key 固定为 term+campus+index |
| 同 scope 重复 section keys | 18 | 均语义等价；允许可审计 collapse，冲突时必须拒绝 target |
| 语义冲突重复 keys | 0 | 以后出现非零即 fail closed，不选择“最后一条” |
| term+index 跨 campus 碰撞 | 3,125 | 证明 campus 不能从 key 中删除 |
| 裸 index 跨 scope 碰撞 | 3,344 | 证明裸 index 只可作为用户输入片段 |
| 多 raw object 的 courseString group | 41 | 不能假设一门 course 只有一个 raw object |
| 具有多个真实 variant 的 group | 31 | UI 和 schema 必须显式支持 variant |
| variant 间 section overlap | 0 | 当前未观察到不等于永久不存在；保留冲突检查 |

Fall 2026 的 B、CU、MC、J 在官方启用列表中，但 Catalog 返回合法 JSON 空数组；D 返回 3 个 course、3 个 section。空 target 不能从 campus dictionary 中消失，也不能被解释为“campus 永久无课程”。

### 2.3 P3 Open 事实

| 事实 | 当前观察 | 设计约束 |
|---|---:|---|
| Open attempts | 42/42 HTTP 200 | 两轮、21个scope；所有raw/hash/ledger可复算 |
| Root values | 302,125个五位字符串；非字符串0；raw duplicate 0 | typed array校验后才转set |
| Official intersection | 42/42 PASS | orphan只审计，不造Section |
| Empty arrays | 2 | 均对应空Catalog；empty+非空Catalog仍按UNSAFE_EMPTY处理 |
| Changed raw scope pairs | 14/21 | 两轮自然观察到变化，证明必须产生新observation |
| Changed effective scope pairs | 3/21 | 3 added、8 removed（scope内非去重求和） |
| Cache-Control | 42/42 `max-age=30` | 不是Rutgers更新SLA |
| RTT | median 1,195.597ms；p95 1,501.020ms；max 1,537.168ms | 真实origin默认保持concurrency=1；饱和时显示lag |

### 2.4 字段与未知语义

- Course、section 和 meeting 的候选源已由 04c/06b 建档；字段存在不等于语义已经可靠。
- Meeting day 当前观察到 M/T/W/H/F/S/U 与空值；H 是 Thursday。TH、MTH、TTH 本轮未观察到，仍须用 fixture 保留兼容路径。
- meeting 的 start/end 空值与合法性分布以 04/06 的当前机器产物为准；即使当前未观察到 partial time，parser 仍必须拒绝或标记未来 partial/invalid 值。
- Raw meeting mode 当前观察到 02–29、80–83、90–93 等代码；90/91/92/93 的描述分别提供 Online、Hybrid、Remote Sync、Remote Async 候选证据。非90系列代码不得因“不是 Online”自动变成 On Campus。
- sectionCourseType 当前观察到 T/H/O；P3 已用当前官方 SOC Course Types filter 冻结其 base meaning，并用 05e 证明 184 个 raw Section rows 存在 type/occurrence 冲突，必须进入 UNKNOWN_CONFLICT。
- Instructor 只能按规范化显示名建立当前字典，并标 NAME_ONLY 可靠性；不得伪造稳定 ID。
- Catalog 的 section.openStatus 和 course.openSections 只作为 Catalog 原始字段保存，不能冒充共享 P3 Open 证据中的实时状态。

## 3. 产品结果与 ONLY 边界

本地包完成后，普通用户应当：

1. 双击 Windows 入口，在无需 Node、Python、npm、数据库工具或管理员权限的情况下打开 WebUI。
2. 从当前、带来源和 freshness 的 Rutgers term/campus 字典选择范围。
3. 使用完整22行筛选合同精确查找 course，并在同一 course variant 内由同一个 section 同时见证全部 section 条件。
4. 默认查看 course-centered 结果，展开 variant 和 MATCH/UNCERTAIN sections；也能独立搜索并直接访问 section 详情。
5. 在本地跨启动保留 filters、selected sections、Saved views、设置和 episode/watch history，但绝不自动恢复 active watch。
6. 明确开始最多9个 section 的 live watch，并按 P2 合同接收 toast 和 WebUI 声音。
7. 在 UI 中区分 Catalog 与 Open 两种刷新、成功时间、最新失败和分类计数。
8. 使用 en-US 或 zh-CN 完成包括错误、Reset、history 和 Saved views 在内的完整流程。

当前包明确不包含 email、SMTP、SendGrid、Discord、Web Push、native/system notifications、macOS launcher、Calendar、Waitlist、Share links、Named Compact view、quiet hours、snooze、持久 active subscription/contact/token、HTTP claim、Node/Fastify 后端运行时或内部证据工具。

## 4. 目标架构

### 4.1 单一共享 Rust 基线

建议在 P7 建立一个 Rust workspace，并通过清晰的 crate/module 边界保持未来 local/public 共享，而不是复制两套业务逻辑：

| 模块 | 责任 | 本地消费者 |
|---|---|---|
| domain | SectionKey、CourseGroup/Variant、三值代数、FilterSchema、RefreshObservation、错误码 | 全部模块 |
| rutgers_client | selector discovery、Catalog/Open HTTP、安全限制、raw DTO、provenance | 双scheduler |
| catalog_normalize | variant 分组、section collapse、occurrence/Delivery 规范化 | Catalog ingest |
| storage | 单一 SQLite migrations、repository、target transaction、FTS、operational/personal logical stores | query/runtime/local entry |
| query | 22筛选、same-section/same-variant witness、理由、排序、分页 | HTTP API |
| refresh_runtime | Catalog/Open target scheduler、single-flight、checkpoint、backoff/circuit、计数 | local entry |
| watch_runtime | WebSocket connection maps 和 fanout | local entry |
| local_user_state | settings、selected、Saved views、history、Reset | HTTP/WS API |
| local_app | loopback server、embedded React assets、startup、browser open、shutdown | bcsp-local.exe |

Rust 运行时方向冻结为：Tokio 异步 runtime、Axum 或等价 typed HTTP/WebSocket 边界、Reqwest+rustls 或等价无系统 OpenSSL 客户端、bundled SQLite+FTS5、Serde typed contracts、structured tracing、SHA-256 provenance。最终库选择仍须在 P6 dependency/license 表中锁定；不得引入第二套 query 或 persistence 实现。

### 4.2 进程与浏览器边界

- 一个 bcsp-local.exe 同时管理 discovery、Catalog、数据库、HTTP/WS 和静态 React 资源。
- 只绑定 loopback；禁止默认监听 0.0.0.0 或 LAN。
- 使用稳定的本地 origin 以支持 section direct URL reload。端口被占用时先验证是否为同一实例；不是则给出明确错误，不静默暴露到其他接口。
- state-changing HTTP 与 WebSocket 必须验证 Origin 和每次启动生成的 session nonce；关闭 CORS，不接受任意网页驱动本地 Reset/watch。
- React product state 以本地服务中的单一数据库为权威；不在 localStorage 中复制 active、history 或 Saved definitions。
- 关闭最后一个窗口不必强制退出后台进程，但托盘/后台行为不在当前合同中；P7 应选择“显式退出入口+进程退出停止全部 watch”的可见语义，不增加隐藏常驻功能。

### 4.3 本地目录

Windows 本地一键包的唯一持久化根是已解压的 package root。运行时必须先取得 `RBCSP.exe` 自身的规范化绝对路径，再以其所在目录为 `<package-root>`；进程当前工作目录（CWD）、启动 `.bat` 的调用目录和浏览器下载目录都不得参与数据路径计算。唯一 live database 固定为：

```text
<package-root>/data/rbcsp.sqlite
```

- `data/` 可由首次启动创建；最终 archive 可以不携带空目录，但不得携带数据库。
- 不得回退到用户 profile、系统目录、临时目录、注册表指向的位置、CWD 或任何第二路径。路径不可解析、目录不可创建、数据库不可读写、不能安全执行 SQLite transaction/WAL/checkpoint 或不能建立完整备份时，必须在任何 Rutgers selector/Catalog/Open 请求前 fail fast，并显示可执行的“请解压到当前用户可写目录”错误。
- 唯一 live database 保存两类逻辑 domain：可由 Rutgers 重建的 Catalog/Open/refresh/checkpoint/diagnostic operational tables，以及不可从 Rutgers 重建的 filters、selected、settings、Saved views、history/ack personal tables。它们共享一个物理 SQLite 文件，但保持 schema/table ownership、repository API 和 Reset 权限边界。
- SQLite WAL/SHM、完整备份和临时文件只能在运行时位于 `<package-root>/data/` 内；不得在其他位置生成隐形持久数据。完整备份是不可直接提供服务的恢复副本，不构成第二个 live datastore。
- 删除整个解压目录即删除程序及其全部本地数据。仅替换/升级程序文件时必须保留既有 `data/`；升级不得覆盖、清空或用 bundled seed 替换数据库。

机器可验证合同：

```text
storage_amendment=P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001
database_relative_path=data/rbcsp.sqlite
path_anchor=RESOLVED_EXECUTABLE_DIRECTORY
current_working_directory_semantics=IGNORED
alternate_storage_fallback=FORBIDDEN
pre_network_writability_gate=REQUIRED
live_database_count=1
logical_domains=OPERATIONAL,PERSONAL
reset_scope=PERSONAL_TABLES_ONLY
archive_database_payloads=FORBIDDEN
first_run_sequence=WRITABILITY_SCHEMA_ONLY_THEN_RUTGERS
upgrade_data_directory=PRESERVE
uninstall_scope=DELETE_UNPACK_ROOT_INCLUDING_DATA
backup_scope=FULL_RBCSP_DATABASE
p7_authorized=FALSE
```

## 5. SQLite、迁移与数据模型

### 5.1 单一数据库与逻辑 domain 分离

本地运行时只能打开 `<package-root>/data/rbcsp.sqlite` 这一份 live SQLite 数据库。物理文件统一，但 ownership 必须按逻辑 domain 分离：

- `OPERATIONAL`：可由上游重建的 term/campus/course/variant/section/occurrence/FTS、Catalog/Open refresh、checkpoint、counter 和诊断事实。
- `PERSONAL`：不可从 Rutgers 重建的 filters、selected、settings、Saved views、本地 history 和 ack。

所有 repository、migration、foreign key、transaction 和 backup 均以同一数据库为边界。Reset 通过受审计事务只清除 `PERSONAL` tables，不删除数据库文件、不清除 `OPERATIONAL` tables，也不借机混入旧 subscriptions/contact/token schema。

### 5.2 Catalog schema

| 对象 | 主键/约束 | 关键字段 |
|---|---|---|
| source_versions | source+version | selector URL/hash/version、observedAt、freshness |
| terms | term_id | year、term_code、display、published/source version |
| campuses | campus_code | display、category、enabled/source version |
| catalog_targets | term_id+campus_code | lifecycle、last attempt/success、content version、row counts |
| catalog_refresh_observations | target+sequence | started/completed、outcome、hash、bytes、changed、error class |
| course_groups | term+campus+course_string | subject、number、group display fields |
| course_variants | group+variant_key | supplement、credit shape、variant label、canonical hash、raw multiplicity |
| courses_core/course_locations | variant+value | structured many-to-many values |
| sections | term+campus+index | variant FK、section number、Catalog raw status、detail fields、canonical hash |
| section_raw_duplicates | target+index+raw_hash | multiplicity、semantic hash、collapse decision |
| occurrences | section+occurrence_key | day/time knowledge、requiredness、kind、mode raw/canonical、location |
| instructors | normalized_name | display name、NAME_ONLY reliability |
| section_instructors | section+ordinal/name | source order与display |
| eligibility/majors/minors/honors | section+type+value | structured code/text+source reliability |
| course_fts | variant/group rowid | searchable document and content version |

Section 外部唯一约束必须是 term+campus+index。内部 surrogate ID 只用于 FK 和性能；任何 API、Saved state、history 或 WebSocket frame 都不得只保存 surrogate 或裸 index。

### 5.3 Open operational schema 与 Catalog version 一致性

Open运行事实写入单一数据库的 `OPERATIONAL` tables，用户 episode/action history 写入同一数据库的 `PERSONAL` tables：

| 对象 | 主键/约束 | 关键字段与保留策略 |
|---|---|---|
| open_targets | term+campus | Catalog content_version、requested/effective/actual interval、next_due、failure streak、circuit、last attempt/valid/state-change、LKG age |
| open_pull_attempts | target+attempt_sequence | started/completed、catalog version at start、HTTP/classification、cache metadata、canonical set hash、bytes、lag；不保存raw body |
| open_refresh_observations | target+observation_sequence | valid applied response、catalog version used、set/state hash、changed、orphan/duplicate、refresh classification |
| section_open_state | term+campus+index | OPEN/CLOSED/UNKNOWN、fresh/stale、target observation、catalog version、last transition |
| open_daily_aggregates | Rutgers date+target | attempted/succeeded/failed/empty及latency/lag rollup |
| open_episode_summaries/actions | user run+SectionKey+episode/action | 见§11.2；属于用户history且不自动TTL |

每target保留当前Rutgers日或最近256条attempt/target-observation明细（取覆盖更多者）；更旧诊断明细汇总成daily aggregate且不留raw body。该有界策略不适用于用户episode/action history。run counters以内存+checkpoint恢复，today counters由当前`America/New_York`日明细/aggregate复算。

OpenPullAttempt开始时捕获Catalog `content_version`。提交事务若版本漂移，分类`STALE_CATALOG_RACE`，不得更新LKG或生成Closed；新Catalog Sections保持UNKNOWN，并按正常single-flight due重新排队，禁止立即追加网络请求。若版本未漂移，OpenRefreshObservation、section_open_state和checkpoint在同一事务提交。Catalog target替换与Open reconcile共享SQLite单写事务边界，不允许旧Section状态孤儿。

### 5.4 Course group 与 variant

P3 已观察到31组真实多 variant course，故采用三级模型：

1. CourseGroupKey = term+campus+courseString。
2. CourseVariantKey = group key + 由所有会影响筛选/详情的 course-level canonical fields 生成的稳定 variant fingerprint。
3. SectionKey = term+campus+index，section 必须关联到一个明确 variant。

归并规则：

- 相同 group 内、variant fingerprint 相同的 raw course objects 合并为一个 variant并并集其不冲突 sections，同时保留raw multiplicity和hash。
- supplement、credits、title或其他筛选/决策字段不同则保留独立 variant，不用“第一条”覆盖。
- 同一 SectionKey 的 raw records只有 semantic hash 相同时才 collapse；P3观察到的18组走此路径。
- 若同一 key 出现不同 semantic hash，或同一 section 落入互相冲突 variants，整个 target normalization失败并保留上个已提交版本。
- Course 查询先按 variant 计算 course predicates，再在该同一 variant 的 sections 中计算 same-section witness；最后对 variants做三值 OR 聚合为 course group 结果。不得让 variant A 的 credits 和 variant B 的 section 时间拼成 MATCH。

### 5.5 Occurrence 与 Delivery

Occurrence 至少保存：raw meeting hash/ordinal、weekday/date、start/end raw与minutes、time knowledge、requiredness、occurrence kind、raw mode code/description、canonical modality/synchronicity、location/building/room、normalization reason。

- 空 day/time 不等于无 meeting，也不等于 async。
- 当前 raw 没有观察到 required/optional 字段时，requiredness 默认为 UNKNOWN_REQUIREDNESS；不能默认为 optional。
- H 正规化为 Thursday；TH 同样支持；组合token用最长合法token parser，不使用逐字符贪婪算法。
- 无效分钟、end不晚于start、partial time进入 INVALID/PARTIAL，不进入确定 MATCH。
- code 90/91/92/93、官方 by-arrangement 规则和 sectionCourseType T/H/O 的 raw→canonical 表已由 P3 Delivery oracle 冻结；实现应从同一 versioned mapping registry 生成，未知新 tuple 走 UNKNOWN 并保留 raw，T/O occurrence conflict 走 UNKNOWN_CONFLICT。
- 对 generic Online，只能得到 ONLINE+UNSPECIFIED；不能用时间存在与否猜 Sync/Async。

### 5.6 迁移

- 使用单调 schema version、命名 migration、SHA-256 checksum 和单事务升级。
- 首次启动先通过§4.3可写门并创建只含 schema/migration metadata 的空数据库；提交 schema 后才允许开始 Rutgers discovery/Catalog/Open 获取。不得从 archive seed、示例、fixture 或预装真实课程/Open 数据。
- 既有数据库升级前先做兼容性检查，并用 SQLite 一致性备份机制创建整个 `rbcsp.sqlite` 的完整恢复副本；不得只备份 PERSONAL tables。备份成功后才允许在原 live database 内执行 migration；checksum漂移或备份失败均拒绝启动并提供诊断。
- OPERATIONAL 与 PERSONAL migration 由各自模块声明、按一个总版本图排序，并在同一数据库事务边界内应用；不得创建第二个数据库来模拟独立迁移。
- 不迁移旧 email/contact/token/active subscription；检测到旧键时只按明确 allowlist导入无害设置，并记录清理结果。
- PERSONAL migration必须覆盖 FilterSchema、Saved definitions、applied association和history summary；未知新版本数据不降级覆盖。

## 6. Discovery、Catalog scheduler 与安全 ingest

### 6.1 Scope discovery

- 官方 SOC selector/bootstrap是 term/campus dictionary 来源，保存 source URL、version/hash、observedAt 和最后成功时间。
- Fall 2026 的 15 个当前 campus 与 Summer 2026 的 6 个结构 scope 只作为回归 fixture，不作为永久 allowlist。
- Discovery失败时保留last-known dictionary并显示 stale/error；首次安装无last-known时显示初始化失败，不展示假fallback。
- 当前 term 的全部官方有效campus进入默认初始化队列；用户选择其他已发布term时初始化该term scope。空 course campus仍保留在dictionary中。
- Discovery refresh不得与10分钟 Catalog refresh混成一个无界请求风暴；使用独立低频策略和全局上游 limiter。

### 6.2 Catalog scheduler

- 默认10分钟；用户可设置1–1440分钟，非法值明确拒绝，不静默clamp。
- 每个 target 单独 next_due；全局串行或经证据批准的极低并发，错峰执行，避免所有campus同一时刻突发。
- 同 target single-flight；定时、首次初始化和UI“立即刷新”合并为一个请求。
- 每次 attempt 无论成功、失败或无变化都写一条 RefreshObservation。
- UI触发立即刷新受cooldown；连续点击只共用同一个结果。
- 一个 target 失败不回滚其他 target，也不阻塞其查询；批次显示 PARTIAL。

### 6.3 HTTP 与 staging

- 只允许官方 HTTPS origin、GET、已验证query；关闭跨origin redirect。
- 设置明确 timeout、decoded/wire body上限、JSON Content-Type/root-array校验和SHA-256。
- retry只针对明确瞬态类别，并经过全局 limiter/cooldown；403/429/schema/HTML/size错误立即开路并显示，不自动重试。
- Catalog完整body只可暂存到同一 `rbcsp.sqlite` 的 OPERATIONAL ingest staging tables，并依靠SQLite transaction/fsync形成crash-safe边界，再解析；不得为staging创建第二个数据库或包根外文件。Open raw body继续按`23a/23b`禁止产品持久化。解析和规范化不得直接修改live serving tables。
- 在同一数据库内生成target-scoped staging tables，完成类型、key、duplicate、variant、row-count、FTS document和FK校验后，在一个事务中替换该target并清理暂存body。
- canonical content hash未变时不重写所有行，但仍提交新的success checkpoint和时间戳；变更时递增target content version并使相关查询cache失效。

### 6.4 Catalog 空数组安全

P3已观察到官方有效campus返回空数组，因此空不能一律当HTTP失败：

- 首次初始化且scope由当前官方selector确认时，可记录 EMPTY_VALID、安装零行target并保持campus可选。
- 已有非空target突然返回空时，先记录 EMPTY_SUSPECT/PARTIAL并保留last-known内容；不得一次请求就删除全部课程。
- 只有后续正常调度的独立成功空观察满足冻结的确认策略后才能提交为空；不得通过立即burst主动“确认”。
- UI同时显示最后成功非空checkpoint与最新空attempt，直到安全决策完成。
- 该策略只适用于 Catalog；Open 空响应按 `23a` 区分 `VALID_EMPTY_NO_ROWS` 与 `UNSAFE_EMPTY_KEEP_LAST_KNOWN_GOOD`。

## 7. 22行 FilterSchema 与查询计划

### 7.1 单一 FilterSchema

Rust domain 和 TypeScript UI 从同一versioned registry生成或用golden contract互验。每项登记 stable ID、type、neutral/default、normalizer、validator、query encoder、Saved-view codec/migration、chip/i18n metadata与tests。禁止在 UI、query、Saved manager 各维护一份字段列表。

所有 predicate返回 MATCH、UNCERTAIN 或 NO_MATCH，并携带stable reason code。不同维度AND，同维度默认OR，Core显式ANY/ALL。Server统一计算filters、variant witness、section reasons、total、pagination和sorting；前端不在分页后删除结果。

### 7.2 字段落地矩阵

| ID | 数据/索引 | 判定与实现 | 验收重点 |
|---|---|---|---|
| FLT-C01 Term | catalog_targets/terms | 必选exact；dictionary无source/freshness则阻止查询 | term version、invalid、stale |
| FLT-C02 Campus | target scope/campuses | 多选OR；空为当前term全部已初始化/可用campus | 15-campus、合法空scope、无fallback |
| FLT-C03 Subject | variant.subject+dictionary | code exact，多选OR | 大字典、source version |
| FLT-C04 Text | FTS5 per variant/group | token AND；exact courseString优先排序 | 真实ingest后可查、Unicode、无post-filter |
| FLT-C05 Course number | normalized variant number | 多值exact OR；保留前导零规则 | exact identifier、unknown |
| FLT-C06 Level | raw explicit level | dictionary exact OR；推导值必须带derived provenance | raw/unknown/new value |
| FLT-C07 Credits | creditsObject canonical range | inclusive min/max；BA/arranged/invalid为UNCERTAIN | variable、decimal、边界 |
| FLT-C08 Core | normalized structured rows | selected codes ANY/ALL，空raw array为已知无core，missing/malformed为unknown | ANY/ALL property tests |
| FLT-C09 Prerequisite | key presence+preReqNotes | present nonempty=HAS；present empty=NONE_REPORTED；missing/malformed=UNKNOWN | empty与unknown分开 |
| FLT-C10 Course location | structured course_locations | exact OR；missing/malformed为UNCERTAIN | code/description/source scope |
| FLT-S01 Index | SectionKey | 输入index结合term/campus exact OR；歧义要求补context | 跨campus collision、直接URL |
| FLT-S02 Section number | sections.section_number | exact OR | 字母/数字格式、unknown |
| FLT-S03 Open status | canonical section OpenObservation state | `fresh_until=last_valid+2×requested_effective+15s`；仅FRESH OPEN/CLOSED给确定MATCH/NO_MATCH；无LKG、超窗或LKG后出现failure/unsafe/catalog-race均为UNCERTAIN，同时显示last-known+age/reason；Catalog raw status不参与 | intersection、LKG、fake-clock stale truth table |
| FLT-S04a Modality | sectionCourseType+occurrence conflict check | T/H/O base 映射为ON_CAMPUS_OR_IN_PERSON/HYBRID/ONLINE；冲突为UNKNOWN_CONFLICT | 不用“非online”推断；184个当前raw冲突行不得被掩盖；新值不丢 |
| FLT-S04b Synchronicity | occurrence Delivery oracle | SYNC/ASYNC/MIXED/UNSPECIFIED/UNKNOWN，与modality正交 | 90/92/93、generic online |
| FLT-S05 Instructor | section_instructors NAME_ONLY | normalized display name exact OR；无stable ID不得伪造 | 同名、无教师、字典版本 |
| FLT-S06 Availability | occurrences + interval indexes | 每个potential-required occurrence完整落入同weekday某窗口；严格三值规则 | H/TH、多窗口、TBA、invalid |
| FLT-S07 Meeting location | occurrence location | exact OR | missing=UNCERTAIN、hybrid多个location |
| FLT-S08 Building/room | occurrence building/room | exact OR；分别标准化但same-section聚合 | 稀疏值、code大小写 |
| FLT-S09 Exam | examCode/text+future occurrence kind | exact raw dictionary；code不臆造exam时间 | finalExam null、unknown code |
| FLT-S10 Permission | add permission code | any/required/not-required；present null与missing分开 | code dictionary、unknown |
| FLT-S11 Eligibility | majors/minors/honors/unitMajors/eligibility/openTo | 子维度内OR、子维度间AND；structured优先，无法解释text为UNCERTAIN | sparse、mixed text/structured |

### 7.3 Availability SQL/执行模型

复杂三值和same-section witness不得依赖一个“任意 meeting EXISTS”SQL。建议流程：

1. SQL用term/campus/course predicates和可索引section粗筛缩小集合。
2. 对候选section在query service中运行同一个纯函数 predicate evaluator，或用等价三值SQL CTE；course查询与独立section查询必须复用它。
3. 每个section输出overall result与每字段reason；每个variant对sections做OR；course group对variants做OR。
4. 分页必须发生在最终course group result之后；可以用临时结果表/CTE两阶段查询，不能先LIMIT再过滤。
5. 建立真实Catalog ingest的performance fixture，P7在目标Windows机器验证可接受延迟和内存。

### 7.4 FTS5

- FTS document至少包含courseString、subject code/description、course number、title、expanded title和经合同允许的description/notes。
- 一variant一FTS row，关联group key和content version；group结果去重聚合。
- 使用事务内显式delete+insert或可靠trigger，target replacement后验证FTS row count和孤儿为0。
- exact Rutgers identifier parser在FTS前运行并提高排序，不用字符串contains代替exact。
- FTS unavailable是显式服务错误，不退回硬编码或部分LIKE假结果。

## 8. HTTP API、直接访问与错误合同

逻辑surface如下；P6可统一最终路径命名，但不得缩减行为：

| Surface | 行为 |
|---|---|
| bootstrap/status | app version、locale defaults、Catalog/Open freshness、target状态、schema versions |
| dictionaries | term/campus/subject/level/core/location/instructor/exam/permission values+source/version |
| course search | FilterSchema request、MATCH/UNCERTAIN groups、variants、matching/uncertain sections、reasons、total/page |
| section search | 同一section predicates、稳定排序、reasons、pagination |
| section detail | SectionKey direct lookup、完整detail/provenance、selected/watch state |
| catalog refresh | local-only immediate request，single-flight/cooldown，返回同一observation引用 |
| Open status/refresh | target checkpoint、requested/effective/actual cadence、lag/circuit、run/today counters；local-only立即刷新进入同一EDF/single-flight且不绕过backoff |
| settings/selection | revisioned local user state；第10项明确拒绝 |
| Saved views | LOCAL_ONLY CRUD/apply/duplicate/CAS/delete-all |
| history | LOCAL_ONLY分页、episode/detail/action查询、显式删除 |
| Reset | LOCAL_ONLY确认token、停止watch后事务清user state |
| WebSocket watch | versioned start/stop/state/refresh/observation frames；只在valid reconcile后fanout |

统一错误envelope包含stable code、localized-message key、trace ID和可安全展示的details；不返回绝对路径、SQL、raw upstream body或secret。404 section、invalid filter、stale revision、quota、audio blocked和upstream unavailable必须可区分。

## 9. React 正式产品壳与 i18n

### 9.1 路由与信息架构

- /：course-centered搜索和筛选。
- /sections：独立section搜索。
- /sections/:term/:campus/:index：可刷新、可复制、可直接访问的section详情。
- /saved-views：LOCAL_ONLY manager，也可作为筛选侧栏子面板实现；不得形成Share URL filter restore。
- /history：LOCAL_ONLY episode/watch history。
- /settings：语言、声音、双refresh、本地数据Reset。

Course card先显示group identity和group级摘要，再显示variant chips/subcards。每个variant保留credits/supplement/title等差异，并在其下列MATCH、UNCERTAIN和“其他sections”。默认展开MATCH；UNCERTAIN独立区域必须解释原因。variant不能被隐藏为重复课程，也不能让用户误以为不同credits属于同一section。

Section summary与详情按P2字段清单展示。所有unknown/TBA/NAME_ONLY/来源freshness用产品解释文案，不暴露内部raw JSON；高级provenance可显示raw code/description但保持Rutgers原文。

### 9.2 UI 状态

每个主要surface实现 loading、first-run initializing、partial target progress、empty valid、empty suspect、stale、latest-attempt failed、offline/connection lost、validation error和ready。不得用fallback课程或假字典填充错误状态。

Open badge、Open filter、checkpoint/counters、watch、toast和audio面板必须完整实现23a的UNKNOWN、FRESH、stale LKG、unsafe、catalog-race、circuit与lag状态；stale/UNKNOWN筛选必须显示UNCERTAIN，不得把requested interval显示成已实现的actual interval。

### 9.3 i18n

- canonical locales：en-US、zh-CN；fallback en-US。
- 本地首次按系统/浏览器检测，用户选择写入单一数据库的 PERSONAL settings table；Reset后重新检测。
- html lang、日期、数字、plural、America/New_York计数日界说明与aria label同步。
- filters、reason codes、variants、Saved views、history、Reset、双refresh、watch/audio和全部错误必须key parity。
- Rutgers course title、教师、地点、代码和用户Saved view名称保持原文，不机器翻译。
- 构建门运行missing/orphan/key-parity检查；旧mail/calendar/macOS文案不进入bundle。

## 10. LOCAL_ONLY Saved views

SavedViewDefinition严格采用P2模型：id、name、schemaVersion、revision、canonical filter snapshot、createdAt、updatedAt。

- PERSONAL Saved views tables使用稳定UUID、casefolded unique name和monotonic revision。
- Create/duplicate生成新id；rename/update/delete携带expected revision并用CAS事务；冲突返回当前revision，不last-write-wins。
- Snapshot只包含FilterSchema登记的term/campus和22筛选值；不保存sort、page、result、selected、active、audio、refresh、language、history或ack。
- Apply后page=1并建立appliedViewId/appliedRevision。clean/modified按canonical snapshot比较，不维护手工dirtyFields。
- Rename不改变snapshot；update必须显式。删除当前view或delete-all保留current filters并解除association。
- 普通filter Reset保留library；delete-all只删library；只有安装级本地用户数据Reset同时删除library。
- Schema migration由FilterSchema驱动；future missing field补当时neutral值，unknown/removed field保留raw并标INCOMPATIBLE，禁止发宽松假query。
- 不设产品数量上限、不静默淘汰；SQLite/disk/quota失败明确反馈。
- 不实现URL restore、Share、cloud/account sync、default auto-apply、import/export。

## 11. LOCAL_ONLY settings、history 与 Reset

### 11.1 持久与非持久边界

跨启动保存：current filters+association、最多9个selected SectionKeys、language override、volume、sound mode/duration、Max audible设置、Catalog/Open interval、Saved definitions和episode/action history。Open target checkpoint与有界诊断attempt属于可重建运行数据，不属于user history。

绝不跨启动保存：active watch flag、connection ID、heartbeat、当前audible count、未确认alarm、AudioContext状态。恢复selected只恢复候选列表，UI明确显示“未开始watch”。

### 11.2 History

History 的 episode 输入只接受23a批准的 valid OpenObservation；表结构按以下语义锁定。

- episode summary按SectionKey+local run+episode sequence唯一。
- 保存first/last Open observed、observationCount、mode、successful audible count、ack/timed-out/closed时间和原因。
- 同episode每次Open更新summary，不每秒插入一条无限raw history。
- watch start/stop、reset audible、resume、confirm、confirm-all为有意义action rows。
- 不保存contact、token、账号、connection ID，也不从history恢复active。
- 无自动TTL和静默淘汰；提供分页、按term/section/time筛选、显式删除单项/全部。容量通过聚合、索引和可见诊断管理。

### 11.3 Reset

“重置本地用户数据”使用二次确认和短期confirm token：

1. 广播stop并清理全部当前process active watches、episode mixer和WS映射。
2. 在单一数据库事务中只删除 PERSONAL domain 的filters、association、selected、Saved library、settings overrides、history和acks。
3. 重置内存store和浏览器当前state，语言重新按系统检测，Catalog/Open普通刷新interval回到10分钟/30秒。
4. 不删除 `rbcsp.sqlite` 文件、OPERATIONAL tables、应用或 operational diagnostics。
5. 返回新的user-state revision；所有打开页面收到reset event并丢弃旧revision。

Reset 必须先停止 Open watch/episode，再执行 PERSONAL-domain 事务；Open checkpoint、上游诊断attempt/daily aggregate与Catalog数据不属于本地用户数据Reset范围，普通用户Reset不得伪造或倒退today counters。

## 12. 双 scheduler、WebSocket、watch 与声音

### 12.1 Catalog scheduler

Catalog scheduler按第6节冻结：local默认10分钟、范围1–1440分钟、per-target single-flight、全局错峰、每attempt新RefreshObservation、成功与最新失败分显、无变化也更新时间截点。

### 12.2 Open scheduler

> 状态：FROZEN BY `23a/23b-shared-open-final-contract`

- `OpenBatchKey=(term,campus)`；当前每batch固定一个同key官方URI。若未来同batch有多个required arrays，必须全部成功后在batch内union；不同campus从不为状态变更做动态union。外部SectionKey保持`(term,campus,index)`，scheduler cycle允许成功batch独立提交。
- local普通刷新默认30秒、范围3–3600秒；target存在active watch时目标10秒；requested effective取`min(general,10)`，local 3秒不被静默clamp。
- per-target single-flight；timer/首次加载/本地手动刷新/watch lane合并；missed tick coalesce/skip且不追赶。
- Catalog/Open共享真实Rutgers origin `max_concurrency=1`；全部工作按absolute earliest-due-first，同deadline才watch优先，必须通过fake clock证明无饥饿。同target的用户数和watch数不放大请求；饱和时记录并显示actual interval与scheduler lag。
- Connect timeout 5秒、attempt总timeout 15秒、decoded上限10MiB、立即retry 0。
- network/408/5xx、UNSAFE_EMPTY、UNSAFE_ZERO_INTERSECTION退避30/60/120/240/480/600秒；实际retry delay取`max(requested effective, backoff)+0–10%确定性正jitter`。429遵守Retry-After，否则origin 15分钟circuit；403/off-origin/content/schema/value/size异常fail closed，最少60秒后才允许显式诊断recheck。
- nonempty valid response转canonical set并对Catalog Sections做membership；orphan审计不造Section，duplicate审计后set去重。非空Catalog但intersection为0属于UNSAFE_ZERO_INTERSECTION，保留LKG且不得靠重复miss自动mass-close。
- empty+empty Catalog=`VALID_EMPTY_NO_ROWS`；empty+非空Catalog=`UNSAFE_EMPTY`并保留last-known；失败、catalog version race同样不以absence转Closed。
- 每请求记录OpenPullAttempt；每valid response即使unchanged也产生target级OpenRefreshObservation，并为该batch每个watched Section派生section级OpenObservation。run+Rutgers-day统计attempted/succeeded/failed/empty，empty为正交维度；日界`America/New_York`。
- body_changed只看canonical set/body hash，state_changed只看intersection state hash；ETag仅审计，不触发episode、cue或changed。
- UI分别显示last attempt、last valid observation、last body/state change、latest failure、last-known age、requested/effective/actual cadence与circuit reason。
- 真实变化到通知只按`U+C+P+B+F`分段；仅valid observation→fanout/audio冻结为≤1秒工程目标。

### 12.3 WebSocket 与 active watch

> 状态：FROZEN SHARED CONTRACT

- selected最多9；第10项明确拒绝。
- start需要用户手势，逐SectionKey返回active/rejected。
- server维护connection→sections和section→connections内存映射。
- stop/socket close/heartbeat timeout/process exit立即清理；transport重连不自动rewatch。
- active不写SQLite；本地重启只恢复selected。
- frames带protocolVersion、section OpenObservation ID、target refresh ID、SectionKey、status、observedAt、freshness、pullSequence和counter snapshots。
- 精确frame replay按observationId幂等；不同pull不得被吞。

### 12.4 Toast、ONE_SHOT 与 CONTINUOUS

> 状态：FROZEN PRODUCT CONTRACT

浏览器消费 P3 Review 批准的 OpenObservation；P2 交互语义如下：

- 新episode产生可感知alert；同episode更新lastObservedAt/count而不无限堆toast。
- ONE_SHOT对每个不同且valid的section OpenObservation在其状态为Open时排队一次cue，直至每section Max audible（默认3、任意正整数、无产品上限）；unchanged Open不新建episode。
- 成功开始cue才计数；volume0、autoplay blocked或开始前失败不计。
- 达Max只静默声音，不停止watch、toast、状态或history；显式restart/reset count归零。
- CONTINUOUS只由新OpenEpisode启动，默认10分钟、支持有限正时长或UNLIMITED，不受Max audible限制；已确认episode中的unchanged Open observation不重响。
- A/C/D多section共享一个bounded mixer；逐项/全部确认；同episode确认或timeout后不因持续Open重响；可靠Closed→Open才新建episode。
- 通用WebUI声音，不朗读课程名；AudioContext错误明确显示且不丢其他反馈。
- mixer禁止无界并行音轨，支持reduced motion不等于静音。

## 13. Windows 本地一键打包

### 13.1 Build 与运行制品

- 目标：64-bit Windows，MSVC toolchain；是否另做ARM64由P6/P7证据决定，不在当前假定。
- React/Vite仅是build-time工具；dist静态资源嵌入Rust exe或进入严格allowlist只读assets目录。
- bundled SQLite包含并验证FTS5，不要求用户安装DLL/SQLite。
- TLS使用随exe可用的rustls/webpki或等价方案，不依赖用户OpenSSL。
- 不需要管理员权限、不写注册表系统区、不安装service、不修改PATH或防火墙。

建议用户包正向allowlist：

1. RBCSP.exe
2. Start-RBCSP.bat 薄入口，只定位并启动exe
3. README.en-US.md 与 README.zh-CN.md
4. LICENSE/THIRD-PARTY-NOTICES
5. 可选的明确版本/manifest文件

`data/` 由首次运行创建，或只作为不含文件的空目录项存在。archive中禁止任何 `.sqlite`、`.sqlite-wal`、`.sqlite-shm`、`.db`、数据库seed/backup、Catalog/Open raw body、真实课程/Open派生数据、logs、checkpoint和staging；同样禁止 node.exe、npm、node_modules、Python、源码、测试、P3 raw、mail模板、secret、旧release、macOS `.command`和内部probe。发布前必须同时执行正向allowlist与上述denylist，不能仅靠文件名抽查。

### 13.2 Launcher 与生命周期

- 双击bat或exe均可启动；bat不下载依赖、不联网修复、不调用JS。
- 单实例锁；已有健康实例时只打开浏览器，不启动第二套scheduler。
- readiness成功后才打开WebUI；migration/discovery/port错误在可读窗口给出trace ID，并仅在数据库可用时写入 OPERATIONAL diagnostic tables；不得为错误日志另建持久目录。
- 启动顺序固定为：解析exe根目录 → 通过 `data/` 可写/备份能力门 → 创建或迁移schema → 启动本地HTTP/WS → 请求Rutgers。存储门失败时不得先发任何Rutgers请求，也不得换用其他目录继续运行。
- Ctrl+C、UI显式退出和Windows关闭走graceful shutdown：停止scheduler、关闭WS、flush checkpoint、checkpoint WAL。
- 崩溃恢复不自动恢复active watch；同库未完成staging与DB integrity在下次启动检查。

### 13.3 Clean-machine验收

在无Node/Python/Git/开发工具的干净Windows账户上验证：解压路径含空格和Unicode；从不同CWD分别通过`.bat`与exe启动仍只使用`<package-root>/data/rbcsp.sqlite`；只读/不可写解压目录在任何Rutgers请求前fail fast且不产生fallback数据；archive无DB/WAL/SHM和真实Catalog/Open数据；首次启动先产生schema-only DB再取数；浏览器打开与首次真实Catalog初始化；重启持久状态但active不恢复；direct section URL reload；Reset只清PERSONAL tables；完整DB备份与升级migration保留`data/`；离线/上游失败；删除整个解压目录后没有散落在其他目录的应用数据。

## 14. 实施顺序与依赖门

| 顺序 | P7工作包 | 输入门 | 输出 |
|---:|---|---|---|
| 1 | Rust workspace、contracts、error/i18n keys | P6批准 | 可编译骨架与generated/golden types |
| 2 | 单一数据库的OPERATIONAL/PERSONAL migrations | schema/variant/identity冻结 | clean+upgrade+full-backup DB tests |
| 3 | discovery、Catalog client、normalizer、target ingest、FTS | P3 Catalog gate冻结 | 真实fixture全链 |
| 4 | FilterSchema、22 predicates、course/variant/section query | Delivery/meeting oracle冻结 | golden/property/perf tests |
| 5 | HTTP API、React course/section UI、i18n | query contract稳定 | 正式非Open产品壳 |
| 6 | LOCAL_ONLY Saved views、settings、selected、Reset | PERSONAL schema稳定 | persistence/CAS/reset tests |
| 7 | Open scheduler、WS、history、toast/audio | P3 `23a/23b` + P6批准 | 完整 live flow |
| 8 | Windows launcher、artifact allowlist、clean-machine | 全部功能门通过 | 本地一键包候选 |

不得为了并行实现旧 poller 兼容层；第7项必须直接实现23a合同，并先用fake-upstream完成失败/容量/episode测试。

## 15. 验证策略

### 15.1 Rust/domain/storage

- 三值AND/OR/ANY/ALL property tests。
- SectionKey跨term/campus collision、18组等价duplicate、future conflicting duplicate fail-closed。
- 41个multi-object course group与31个variant fixture；same-variant/same-section反例。
- H/TH/MTH/TTH、M/T/W/H/F/S/U、empty/partial/invalid time、requiredness unknown、async/hybrid。
- target-scoped rollback、unchanged hash、nonempty→empty suspect、multi-target partial。
- FTS row parity、真实ingest可查、migration/checksum/WAL恢复、升级前完整DB备份。
- exe路径锚定、跨CWD同一数据库、不可写目录pre-network fail-fast、零fallback、首启schema-only后取数、升级保留`data/`、删除package root即删除全部本地数据。
- Saved CAS、schema migration/incompatible、Reset只清PERSONAL且OPERATIONAL不变、active零持久。

### 15.2 API/React

- 22字段serialization/query golden；server total/page/reasons一致。
- course group/variant展示、独立section搜索、direct URL reload、not-found。
- loading/first-run/empty/partial/stale/error/disabled全状态。
- en-US/zh-CN key parity、格式、raw原文、keyboard/screen-reader/reduced-motion。
- Saved views完整CRUD/dirty/revision/quota；filter Reset/delete-all/user Reset三作用域。
- 第10 selected拒绝；local重启恢复selected但active=false。

### 15.3 Open相关

> 全部消费 `23a/23b`，不得对 Rutgers 做压力测试

- fake upstream覆盖OpenPullAttempt/OpenRefreshObservation/section OpenObservation基数、valid reconcile、unsafe empty、unsafe zero-intersection、Catalog version race、failure保留last-known、unchanged observation。
- 多连接同target不增加上游请求、Catalog/Open共享concurrency=1、EDF无饥饿、scheduler lag、disconnect cleanup、transport replay。
- ONE_SHOT第N声后静默不停止；CONTINUOUS Closed→Open、A/C/D、confirm、timeout/resume、UNLIMITED。
- run/today counters、America/New_York日界/DST、detail retention/daily rollup、restart、audio blocked。
- 3/10/30/3600边界、max-nine本地watch、多target饱和、coalesce/no-catch-up、backoff不加速3600秒cadence、circuit用fake upstream验证；真实42条evidence只验证shape/join/transition，不冒充容量SLA。
- FLT-S03 fresh/stale/UNKNOWN fake-clock真值；ETag变/body不变、body变/state不变均不误触发episode。

### 15.4 Artifact/security

- 正向manifest diff、secret/string扫描、mail/claim/macOS/Node denylist，以及DB/WAL/SHM/backup/真实Catalog/Open数据零命中。
- loopback-only、Origin/session nonce、CSRF/WS、path traversal、static fallback和error redaction。
- dependency consumer/license/unused audit；用户包SBOM或第三方notice。
- clean Windows walkthrough和hash/reproducibility记录。

## 16. P2能力追踪

| P2能力 | 本计划位置 | 主要验收 |
|---|---|---|
| 自包含Windows入口 | §4、§13 | clean machine、无Node |
| Catalog双刷新中的Catalog侧 | §6、§12.1 | interval、single-flight、checkpoint |
| 22筛选+三值+same section | §7 | golden/property/反例 |
| course-centered+section direct | §5.4、§8、§9 | variant、direct reload |
| 复合SectionKey | §5.2、§5.4 | collision/duplicate |
| SQLite/FTS | §5、§7.4 | migration、真实ingest、parity |
| LOCAL_ONLY Saved views | §10 | CRUD/CAS/migration/reset |
| LOCAL_ONLY persistence/history/reset | §11 | restart/no-active/reset scope |
| React+i18n | §9 | en-US/zh-CN/a11y |
| Open/watch/audio | §12、23a | intersection/LKG、3/10/30、counters、episode/audio fake-upstream全链 |
| ONLY闭包与包 | §3、§13、§15.4 | allowlist/denylist |

## 17. 风险、停止条件与实现期 Review

P3 Review必须停止而不是冻结，如果：

- P3 Delivery/meeting oracle无法把现有raw安全映射到P2 canonical或unknown语义；
- course variant fingerprint会合并影响筛选/详情的不同事实；
- equivalent duplicate判定无法复算，或发现semantic conflict；
- 某个REQUIRED筛选没有可信source且无法用UNCERTAIN安全表达；
- 03 evidence register、hash/provenance和notebook不能互相复算；
- 实现计划需要恢复任何P2明确排除的旧链。

P3已经冻结Catalog target/hash、SectionKey、duplicate collapse、course group/variant、22筛选以及共享Open合同。P7若发现实现必须提高真实origin concurrency、放宽empty/error、恢复第二套Open状态或无法在fake-upstream满足valid observation→fanout/audio目标，必须停止回Review；不得静默改合同。

## 18. P3计划冻结完成条件

本计划冻结要求由09验证门复算以下条件：

1. 请求ledger、raw hashes、evidence register与notebook可复算；
2. Delivery/meeting oracle和unknown规则获得Review；
3. composite section collapse与course variant模型有最小fixtures；
4. 22行字段逐项有source、predicate、unknown和test；
5. Catalog empty/transaction/FTS方案闭合；
6. LOCAL_ONLY Saved/history/reset和Windows package闭合，并满足`P6-REVIEW-LOCAL-STORAGE-AMENDMENT-001`的包根相对单库、首启无预装数据、完整备份与零fallback合同；
7. Open两轮42条证据、官方intersection、empty/error、3/10/30、backoff/circuit、observation/counters与P7 fake-upstream门均闭合；
8. 09记录Catalog与Open总门结果，且active P3产物不存在旧RATE待确认或Open阻塞标签。

本文件已在候选验证通过后冻结；`09`总门、traceability、notebook与report须继续由最终validator联合复算。任何后续语义修改都必须重新打开P3 Review，不得静默覆盖本计划。
