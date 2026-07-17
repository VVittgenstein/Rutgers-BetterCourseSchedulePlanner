# P1 旧 RBCSP 行为级能力库存

## 1. 文档状态与读法

- **状态**：P1 Review 已通过；本库存是获批 P1 产品记忆的一部分
- **对象**：旧 RBCSP 的意图、实现、测试、发布、删除和漂移；不是最终产品 backlog
- **时间范围**：2025-11 初始开发至 2026-06 恢复现场，并以 2026-07-12 当前工作树和远端只读状态作交叉核验
- **当前目标的覆盖关系**：见 `04-current-decision-overlay.md`
- **冲突集中登记**：见 `05-conflict-and-supersession-ledger.md`

本库存刻意不沿用旧 task-015 矩阵中的 `complete / recover / repair / remove / defer / unclear` 作为当前结论。该矩阵是 2026-05 的未合并、未通过 review 的历史候选，只能帮助发现能力域。

### 1.1 受控状态原子与组合语法

状态单元格只使用下列原子；多个原子以` / `组合。组合描述多个同时成立的事实，不形成新的隐含状态。代码标识（例如`NOT EXISTS`、`ON DELETE`、`TH`）和版本轴不属于状态。

| 状态原子 | 含义 |
|---|---|
| `IMPLEMENTED` | 在允许snapshot中有行为实现 |
| `PARTIAL` | 只完成部分链路或语义 |
| `ABSENT` | 在已调查snapshot中未发现该能力 |
| `STUB` | 有契约/表面，但核心行为固定为空或假实现 |
| `ORPHANED` | 源码存在，但生产入口没有引用/注册 |
| `DECLARED_ONLY` | 配置、类型或文档声明存在，执行逻辑缺失 |
| `DOCUMENTED` | 只由旧产品文档描述 |
| `RELEASED_SOURCE` | 某旧archive中存在源码/资源，不表示可运行 |
| `DELETED_HISTORY` | 旧Git/Compact证明曾存在，后续源码面消失 |
| `DRIFT` | snapshot、文档、release或行为互相不一致 |
| `IMPLEMENTATION_GAP` | 目标/表面存在，但真实链路缺失或破坏行为 |
| `BROKEN_CHAIN` | 多个已存在环节之间没有闭环 |
| `PATH_METADATA_ONLY` | 只使用路径/Git metadata，不使用正文 |
| `UNKNOWN` | 证据不足，P1不补猜 |
| `P2_UNRESOLVED` | 事实已恢复，但当前all-and-only去向留给P2 |
| `CODE` | 源码直接证据限定词 |
| `TEST_CODE` | 测试代码存在；不等于本轮执行通过 |
| `COMPACT_CLAIM` | Compact Agent总结限定词 |
| `USER_EXPLICIT` | 用户直接表达限定词 |
| `REPORT` | 历史运行报告限定词，不升级成当前测试 |
| `REMOTE` | 仅远端snapshot限定词 |
| `CONFLICT` | 证据/语义直接冲突 |
| `RISK` | 已识别风险，尚未量化 |
| `SECURITY_RISK` | 涉及安全边界的风险 |
| `LOSS_WINDOW` | 存在确认后未完成用户行为的丢失窗口 |
| `HIGH_RISK` | 可能破坏核心用户数据/路径的高风险缺口 |
| `ASSUMPTION` | 行为依赖未显式保证的运行假设 |

### 1.2 版本轴

- `CMP`：74 份 2025-11 Compact；全部是 `COMPACT` 级总结。
- `REL21`：`bcsp-20260121.zip` / `.tar.gz`，同一批 136 个文件的两个容器格式。
- `REL22`：根目录 `bcsp-20260122.zip`，125 个文件。
- `CW`：2026-07-12 当前工作树中允许读取的产品源码/测试。
- `GIT-HIST`：旧 P1 污染截止线之前或之外的旧 RBCSP Git 历史。
- `REMOTE`：当前公开 `origin/main` / GitHub 可见产品面。
- `RAW`：项目原始 `.codex` / `.claude` 会话中的用户原话。

## 2. 用户目标与产品身份

| ID | 恢复出的旧意图/行为 | 历史状态与证据 | 未决说明 |
|---|---|---|---|
| LEG-ID-01 | RBCSP 是 Rutgers SOC 课程数据抓取、搜索、筛选与空位提醒工具。 | `README.md:3-5`；旧 Phase 1 主路径；`CODE` 覆盖 ingest→SQLite→Fastify→React→poller。 | 产品身份稳定，但当前双交付与 Rust 架构属于新层。 |
| LEG-ID-02 | 旧 2026-05 Phase 1 的目标是“完整本地 release”，不是 MVP，也不是当时的云部署。 | `GIT-HIST 0a61028:.orchestrator/phase-1/00-plan.md:21-33`；`SRC-RAW-004`。 | 这是旧方向，不覆盖当前“公网包以及部署 + 本地一键包”。 |
| LEG-ID-03 | 旧核心路径被表述为：解压/设置→启动→抓取/载入 SOC→搜索/筛选→查看 section→订阅→轮询→本地提醒→管理/删除订阅。 | `GIT-HIST 5714a8f:.orchestrator/phase-1/01-release-surface-feature-matrix.md:8-21`，未合并历史候选；`SRC-RAW-004`交叉支持。 | 当前 active watch 与提醒语义已经改变。 |
| LEG-ID-04 | 使用文档必须照顾完全没有电脑和代码经验的用户。 | `SRC-RAW-001`定向会话第5条用户消息，明确要求完全重写 README，使零经验用户可逐步使用。 | 该可用性目标与当前 Windows 普通用户一键包方向相容。 |
| LEG-ID-05 | 用户明确认为旧项目“能 work 但并不好”，且怀疑 repo、release、实现、设计和工作流发生不同步。 | `SRC-RAW-003`定向会话第338、409条相关记录；用户要求“清洗、剥离、整理和重构”。 | 这是为何不能把任何旧包或单一分支当作全局真相的直接依据。 |
| LEG-ID-06 | RBCSP 建立的直接原因，是原 CSP 筛选条件不足、性能和定位效率低，用户难以快速找到真正想要的课程；目标是以低门槛改善绝大多数学生的 CSP 使用体验。 | `SRC-CUR-005`，用户在 P1 Review 再次明确说明产品起因。 | BCSP 是 CSP 的体验增强工具，不以替代 CSP 为目标。 |
| LEG-ID-07 | macOS 曾是明确支持目标，但旧史料未证明后来正式取消。 | `SRC-RAW-002`证明用户无Mac经验且要求修好Mac启动；`SRC-RAW-004`中2026-05-12原话仍希望Win/Mac均可直接使用；允许历史窄查未发现“停止支持”决定。 | 当前取消macOS是`SRC-CUR-005`新增决定，不能伪称旧材料已经记录。 |

## 3. 安装、启动、本地运行与打包

| ID | 行为级能力 | 状态与可验证行为 | 证据/漂移 |
|---|---|---|---|
| LEG-RUN-01 | Windows `.bat` 启动入口。 | `IMPLEMENTED / RELEASED_SOURCE`：切换到脚本目录，检查 `node`，缺失时打开 Node 下载页，再执行 `scripts/oneclick_start.js`。 | `Start-WebUI.bat:1-20`。它不是内嵌 runtime。 |
| LEG-RUN-02 | macOS/Linux shell `.command` 入口。 | `IMPLEMENTED / RELEASED_SOURCE`：检查 Node，调用同一 JS launcher。 | `Start-WebUI.command:1-21`。REL21 使用 CRLF；REL22 改为 LF，但 ZIP 未保存 Unix executable metadata。 |
| LEG-RUN-03 | 启动器要求 Node 22+。 | `IMPLEMENTED`：低版本直接退出；根和 frontend 依赖缺失时在线运行 `npm install --force`。 | `scripts/oneclick_start.js:9,110-118,269-365`。旧“一键”依赖网络、npm 和 native module 构建。 |
| LEG-RUN-04 | `better-sqlite3` 平台二进制修复。 | `IMPLEMENTED`：尝试 require，失败则删模块重装，再失败时删除整个 root `node_modules` 重装；最终提示安装 Microsoft C++ Build Tools。 | `scripts/oneclick_start.js:305-364`。普通用户失败成本很高。 |
| LEG-RUN-05 | 首次配置与本地路径写入。 | `IMPLEMENTED`：从 example 复制 `fetch_pipeline.local.json`，解析 DB/term/campus，写回绝对 SQLite 路径。 | `scripts/oneclick_start.js:20-29,187-242`；写入位置与 release 目录耦合。 |
| LEG-RUN-06 | 数据库迁移与首次抓取。 | `DRIFT / IMPLEMENTATION_GAP`：launcher先迁移创建DB，再以DB是否存在判断是否full fetch；因此默认会跳过首次抓取。 | `scripts/oneclick_start.js:367-390`；`README.md:89-102`后来引导用户在WebUI手动抓取。 |
| LEG-RUN-07 | 多进程本地栈。 | `IMPLEMENTED`：启动 Fastify `127.0.0.1:3333`、Vite dev `127.0.0.1:5174`、openSections poller、可选 mail dispatcher，1.5 秒后开浏览器；任一子进程退出会终止整个栈。 | `scripts/oneclick_start.js:392-552`。旧包运行的是 dev server 而非静态 production frontend。 |
| LEG-RUN-08 | 关闭启动窗口停止服务。 | `IMPLEMENTED`：捕获 SIGINT/SIGTERM 并 kill children；文档明确要求窗口保持打开。 | `scripts/oneclick_start.js:409-419,441-444,549-563`；`docs/oneclick.md:10-12`。 |
| LEG-RUN-09 | Linux/WSL setup 与 run-stack 脚本。 | `IMPLEMENTED / COMPACT_CLAIM`：旧脚本可准备 env、安装依赖、迁移、full-init、启动并记录日志。 | `scripts/setup_local_env.sh`、`scripts/run_stack.sh`、`docs/quickstart.md`；`Compact-ST-20251113-act-006-03-fresh-run-2025-11-21-T192107Z.md`声称 WSL 上 4,506 courses / 11,467 sections。 |
| LEG-PKG-01 | 2026-01-21 本地源码包。 | `RELEASED_SOURCE`：zip/tar 各 136 文件，无统一顶层目录；含 React/Fastify/SQLite/poller/mail/launchers/tests 和运行 checkpoint。 | SHA-256 见 `02-source-register.md`；包内容来自含 auto-refresh 的不同工作树快照。 |
| LEG-PKG-02 | 2026-01-22 修订包。 | `RELEASED_SOURCE / DRIFT`：125 文件，删除 11 个开发/模拟脚本，但 `package.json` 仍暴露若干已不存在的命令，并引用历史中不存在的 build script。 | `SRC-REL-022Z`的archive entry与`package.json`直接核验；`SRC-GIT-003`中的release reconciliation作历史交叉。 |
| LEG-PKG-03 | 主 release 内容应以产品文件为主。 | `USER_EXPLICIT`：原始项目会话明确要求 release 不要包含全部工程过程文件。 | `SRC-RAW-006`中定向 session `6a3d7772…`。 |
| LEG-PKG-04 | macOS 双击启动曾真实失败。 | `USER_EXPLICIT`：用户朋友在 Mac 点击 `.command` 后未成功运行；REL21 的 CRLF 是可证缺陷，REL22 权限仍未获真实验证。 | `SRC-RAW-002`定向会话第5条用户消息；`SRC-REL-021Z`与`SRC-REL-022Z`的line-ending/external-attribute metadata。 |
| LEG-PKG-05 | 旧仓库链接和包身份发生漂移。 | `DRIFT`：README/package metadata 指向 `VVittgenstein/BetterCourseSchedulePlanner`，实际远端是 `Rutgers-BetterCourseSchedulePlanner`。 | `package.json:23-33`；release README。 |

## 4. Rutgers SOC 获取、规范化与本地数据

| ID | 行为级能力 | 状态与可验证行为 | 证据/漂移 |
|---|---|---|---|
| LEG-SOC-01 | Rutgers `courses.json` 与 `openSections` 探测。 | `IMPLEMENTED / COMPACT_CLAIM`：probe 记录 request id、延迟、大小、结构化错误；term alias 被解析。 | `scripts/soc_probe.ts`、`scripts/soc_api_client.ts`；`Compact-ST-20251113-soc-api-validation-01-probe-2025-11-16-T133825Z.md`。 |
| LEG-SOC-02 | 多term、多campus、不同学段数据。 | `IMPLEMENTED / COMPACT_CLAIM`：字段矩阵覆盖2 term × 3 campus；旧统计为13,322 courses / 32,249 sections。配置目标可列多个term/campus。 | `Compact-ST-20251113-soc-api-validation-02-field-matrix-2025-11-17-T013508Z.md:2-16`；`configs/fetch_pipeline.example.json:41-76`。统计不是当前Rutgers数量。 |
| LEG-SOC-03 | 上游字段限制。 | `COMPACT_CLAIM`：SOC courses endpoint 忽略某些 subject/level 参数；`openSections`只返回 index 列表，不提供容量或 waitlist。 | `Compact-ST-20251113-soc-api-validation-02-field-matrix-2025-11-17-T013508Z.md:2-16`；需要靠本地过滤和 courses 数据关联。 |
| LEG-SOC-04 | 限流、重试和请求节流。 | `PARTIAL / DECLARED_ONLY / COMPACT_CLAIM`：probe/配置/文档定义lane间隔、并发、retryable status和backoff，历史压力样本未见429；当前runner未执行大部分profile字段，实际固定最多3次线性backoff。 | `configs/fetch_pipeline.example.json:8-40`、`docs/soc_rate_limit.md`、`scripts/fetch_soc_data.ts:681-704`。 |
| LEG-SOC-05 | Rutgers meeting day编码。 | `DRIFT / CODE / REPORT`：历史真实对照样本多次出现周四`H`；normalizer与frontend只识别`TH`，会产生空week mask或丢meeting。 | `reports/field_validation.md:25,65,104,169`；`scripts/soc_normalizer.ts:109-117,343-365`；`frontend/src/hooks/useCourseQuery.ts:385-393`。当前SOC须重探。 |
| LEG-DAT-01 | SQLite 关系模型。 | `IMPLEMENTED / RELEASED_SOURCE`：terms、campuses、subjects、courses、course locations/core、sections、instructors、meetings、populations、crosslistings、status events、subscriptions、snapshots、open events/notifications、FTS。 | `data/schema.sql`、`data/migrations/001_init_schema.sql`至`data/migrations/004_course_campus_locations.sql`；`Compact-ST-20251113-act-007-01-entity-design-2025-11-17-T050547Z.md`。 |
| LEG-DAT-02 | Course 复合唯一性。 | `IMPLEMENTED`：`term + campus + subject + course_number`。 | `data/schema.sql:73-74`。 |
| LEG-DAT-03 | Section/index 唯一性。 | `IMPLEMENTED / DRIFT`：迁移后主要唯一约束是 `term + index_number`；部分 API/订阅逻辑继续携带 campus。 | `data/schema.sql:158-159`、`data/migrations/002_relax_section_index_scope.sql`；不能证明 index 跨 term/campus 全局唯一。 |
| LEG-DAT-04 | Full-init与incremental数据抓取。 | `PARTIAL / IMPLEMENTED`：可按模式/term/campus/subject运行，full-init先truncate，incremental不truncate，并有staging、summary和固定重试；持久队列、recency、完整并发/降级和真正resume未实现。 | `scripts/fetch_soc_data.ts:309-359,681-704,839-940`对比`docs/fetch_pipeline.md:60-89`。 |
| LEG-DAT-05 | 规范化和hash差异。 | `PARTIAL / IMPLEMENTED`：课程/section规范化、source hash/payload、变化更新、meeting重建、core/location同步存在；正常ingest不写instructors joins、populations、crosslistings或FTS。 | `scripts/soc_normalizer.ts`、`scripts/fetch_soc_data.ts:411-557,946-1323`对比`docs/local_data_model.md:61-67,134-239,305-350`。 |
| LEG-DAT-06 | openSections reconciliation。 | `IMPLEMENTED`：按 term/campus 获取 index 集合，更新 section open/closed 并记录 status event/snapshot。 | `scripts/fetch_soc_data.ts:1338-1374`。上游空数组语义在历史中冲突。 |
| LEG-DAT-07 | FTS 搜索文档。 | `PARTIAL / IMPLEMENTATION_GAP / BROKEN_CHAIN`：SQLite建立FTS5，`q` 查询依赖它；正常 `scripts/fetch_soc_data.ts`没有insert/rebuild，只有course-search test fixture手工填表。因此fresh ingest数据库中的关键词查询可能基本无结果。 | `data/schema.sql:341-343`；`api/src/queries/course_search.ts:157-169`；唯一insert在`api/tests/course_search.test.ts:445`；`docs/local_data_model.md:305-350`却声称ingest后刷新。 |
| LEG-DAT-08 | WAL 与迁移。 | `IMPLEMENTED`：migration runner、WAL；fresh run/恢复/备份文档存在。 | `scripts/migrate_db.ts`、`scripts/fetch_soc_data.ts:726-738`、`docs/data_load_runbook.md`。 |
| LEG-DAT-09 | 自动/计划刷新源码表面。 | `REMOTE / RELEASED_SOURCE / DRIFT`：公开main/部分release有server scheduled-fetch service/route（1–30分钟、full-init/incremental）和browser auto-refresh toggle（约15–120秒重新查询本服务），但CW没有这些文件。 | `REMOTE e770bf2`、`b650d81`及对应tree；`SRC-REL-021Z`/`SRC-REL-022Z`；两者是不同层：SOC目录抓取与浏览器query refresh。 |
| LEG-DAT-10 | 自动/计划刷新生产接线。 | `ORPHANED / DRIFT / REMOTE / RELEASED_SOURCE`：App不引用browser toggle/panel，server不注册route，container无scheduledFetcher；定义无生产消费者。 | `SRC-GIT-006`的`origin/main`/`dev`/`feature/task-015` tree及`SRC-REL-021Z`/`SRC-REL-022Z`中的`frontend/src/App.tsx`、`api/src/server.ts`、`api/src/container.ts`对照。 |
| LEG-DAT-11 | Fetch pipeline配置面。 | `DECLARED_ONLY / DRIFT`：rate profile、retry policy、resume、并发、recency、migrations、rebuildFts、metrics等大多只定义/打印/转boolean。 | `scripts/fetch_soc_data.ts:36-51,144,373-374,681-704,743-756`对比`configs/fetch_pipeline.example.json`、`configs/fetch_pipeline.schema.json`、`docs/fetch_pipeline.md`；实际retry固定3次线性backoff。 |
| LEG-DAT-12 | WebUI抓取的破坏范围。 | `IMPLEMENTATION_GAP / HIGH_RISK`：DataFetchCard不传mode，POST默认`full-init`；runner在probe前全库DELETE配置表。会清其他term/campus；resolved subscription FK还可能使truncate失败。 | `frontend/src/components/DataFetchCard.tsx:127-133`、`api/src/routes/fetch.ts:62-74`、`configs/fetch_pipeline.example.json:84-94`、`scripts/fetch_soc_data.ts:839-843,925-940`、`data/schema.sql:246-280`。 |

## 5. 查询、筛选、课程与 section 信息

| ID | 行为级能力 | 状态与可验证行为 | 证据/漂移 |
|---|---|---|---|
| LEG-QRY-01 | `GET /api/courses` 主课程浏览接口。 | `IMPLEMENTED / TEST_CODE`：Zod validation、分页、统计、可选 section expansion、错误 envelope、查询 metrics。 | `api/src/routes/courses.ts`、`api/src/queries/course_search.ts`、`api/tests/course_search.test.ts`。 |
| LEG-QRY-02 | 基础限定。 | `IMPLEMENTED`：term 必填，且至少 campus 或 subject 限制结果集；支持多 campus/subject。 | `docs/query_api_contract.md:50-76` 与 route schema。 |
| LEG-QRY-03 | 课程级筛选。 | `PARTIAL / IMPLEMENTED`：keyword/q、level、credits、core、prereq、campus location；course number只有sort/result，无专用filter，号码文本最多依赖断链FTS q。 | `api/src/routes/courses.ts:15-38`、`api/src/queries/course_search.ts:124-386`、`frontend/src/state/courseFilters.ts:25-43`。 |
| LEG-QRY-04 | Section关联筛选。 | `PARTIAL / IMPLEMENTED`：当前course route可按delivery、open-only、exam、meeting day/time/location筛；instructor/permission/waitlist只在数据面、历史或空sections schema。 | `api/src/routes/courses.ts:23,27-33`、`api/src/queries/course_search.ts:204-277`、`api/src/routes/sections.ts:14-37,85-115`。 |
| LEG-QRY-05 | 严格 meeting subset 语义。 | `PARTIAL / CONFLICT / USER_EXPLICIT`：day filter用 `NOT EXISTS` 排除任一越界/未知日期，符合严格subset；time filter却只要求存在一条meeting满足start/end。前端又按“所有meetings在时间窗内”二次过滤。P1 Review确认目标是用户可用时间：整个section的每个已知必修meeting都须完整落入同星期可用窗口，任一不符即排除。 | `api/src/queries/course_search.ts:212-277,633-679`；`frontend/src/hooks/useCourseQuery.ts:438-466`；`docs/ui_flow_course_list.md:97,106`；`SRC-CUR-005`。异步/TBA/hybrid/optional/exam仍待P2/P3定义。 |
| LEG-QRY-06 | 排序、分页、FTS。 | `IMPLEMENTED / TEST_CODE`：课程号/标题/学分/open count/updated排序、page/pageSize、FTS keyword。 | `docs/query_api_contract.md:67-80`、`api/tests/course_search.test.ts`；真实ingest缺FTS见LEG-DAT-07。 |
| LEG-QRY-07 | 可选section信息内嵌。 | `IMPLEMENTED`：`include=sectionsSummary,subjects,sections`并限制每课section preview数。 | `frontend/src/state/courseFilters.ts:85-94`；`docs/query_api_contract.md:71-72,96-149`。 |
| LEG-QRY-08 | `GET /api/filters`字典。 | `PARTIAL / IMPLEMENTED`：从SQLite返回terms、campuses、locations、subjects、core、levels、delivery、instructors并fallback；不返回examCodes。 | `api/src/queries/filters.ts:3-12,37-213`。 |
| LEG-QRY-09 | 独立 `GET /api/sections`。 | `STUB`：完整 query schema、日志和 200 response contract 存在，但 handler 固定 `total=0`、`data=[]`。 | `api/src/routes/sections.ts:14-116`；旧文档把它写成真实 detail API，形成虚假表面。 |
| LEG-QRY-10 | 筛选面多次扩张/收缩。 | `DRIFT`：2025-11-20曾有index/section/status/instructor/permission/location；11-23删除多项并加exam；2026-01又出现campusLocations。 | `Compact-T-20251113-act-002-frontend-filter-mvp-2025-11-20-T023714Z.md:1-18`、`Compact-ST-20251122-filter-rewrite-01-frontend-state-ui-2025-11-23-T062853Z.md:3-23`、CW state`:25-43`。 |
| LEG-QRY-11 | URL serialize/parse 筛选状态。 | `ORPHANED`：helper 实现存在，但`SRC-REL-021Z`、`SRC-REL-022Z`和CW的生产`frontend/src/App.tsx`均未调用。 | `frontend/src/state/courseFilters.ts:148-299`与各snapshot的`frontend/src/App.tsx`对照。 |
| LEG-QRY-12 | Section信息广度。 | `PARTIAL / DECLARED_ONLY / IMPLEMENTATION_GAP`：大量字段在schema/query contract；正常ingest不填部分关联表，CourseList只显示极少字段，独立endpoint又是stub。 | `data/schema.sql:117-231`、`docs/query_api_contract.md:96-208`、`scripts/fetch_soc_data.ts:943-1323`、`frontend/src/components/CourseList.tsx:125-183`。 |
| LEG-QRY-13 | Filter dictionary与真实值。 | `DRIFT`：API不返回examCodes；normalizer产生`async`但route/frontend只接受三类并丢弃；fallback编码与历史SOC不同。 | `api/src/queries/filters.ts:3-12,202-213`、`frontend/src/api/filters.ts:76-82,120-142`、`scripts/soc_normalizer.ts:329-340`、`frontend/src/data/fallbackDictionary.ts:3-12`。 |

## 6. React UI、数据获取体验与 i18n

| ID | 行为级能力 | 状态与可验证行为 | 证据/漂移 |
|---|---|---|---|
| LEG-UI-01 | React/Vite 单页外壳。 | `IMPLEMENTED / RELEASED_SOURCE`：App 组合数据抓取、筛选、课程列表、订阅、本地声音、邮件设置等区域。 | CW、`SRC-REL-021Z`、`SRC-REL-022Z`中的`frontend/src/App.tsx`；旧 UI 是功能面参考，不是当前正式设计。 |
| LEG-UI-02 | 当前课程列表和section展开。 | `PARTIAL / IMPLEMENTED`：production CourseList只展示课程代码、campus、title及section index/open；没有丰富详情、订阅CTA或virtualization。 | `frontend/src/components/CourseList.tsx:125-183`对比`docs/ui_flow_course_list.md`。 |
| LEG-UI-03 | FilterPanel。 | `IMPLEMENTED / USER_EXPLICIT`：多类控件与active chips存在；用户曾报告面板不能独立滚动，底部条件不可达。 | `frontend/src/components/FilterPanel.tsx:247-559,580-746`、`.css`；SRC-RAW-006 session`69294bc7…`。 |
| LEG-UI-04 | 数据抓取 UI。 | `IMPLEMENTED`：选择年份/学期/campus，POST 单一后台 fetch job，约 3.5 秒轮询 job 状态；成功后刷新字典并更新筛选条件。 | `frontend/src/components/DataFetchCard.tsx:93-240`；`api/src/routes/fetch.ts:82-170`。 |
| LEG-UI-05 | 全局单一fetch job。 | `IMPLEMENTED / RISK`：`fetchRunner`使用模块级active job，限制并发但也形成进程内全局状态。 | `api/src/services/fetchRunner.ts:115-267`。公网多用户语义未设计。 |
| LEG-UI-06 | SchedulePreview/calendar。 | `ORPHANED / DOCUMENTED / USER_EXPLICIT`：组件与早期toggle历史存在；production App不引用，只有dev playground引用。P1 Review确认Calendar是用户过去明确要求、因早期Agent和项目经验限制未完成的能力；不是Agent自行扩张。 | `frontend/src/components/SchedulePreview.tsx`、`frontend/src/dev/ComponentPlayground.tsx`、`Compact-T-20251113-act-002-frontend-filter-mvp-2025-11-20-T023714Z.md:1-18`、`SRC-CUR-005`。当前仅作future feature。 |
| LEG-UI-07 | Compact view、saved views、share links。 | `DOCUMENTED / UNKNOWN`：出现在`docs/ui_flow_course_list.md`与未审核task-015候选，未形成完整production能力。 | `docs/ui_flow_course_list.md`、`5714a8f:.orchestrator/phase-1/01-release-surface-feature-matrix.md:120-130`。 |
| LEG-UI-08 | 中英i18n。 | `IMPLEMENTED / COMPACT_CLAIM`：messages/fallback、语言切换、localStorage、`html lang`、missing-key checker。 | `frontend/i18n/messages.json`、`frontend/src/i18n/index.ts:1-35`、`Compact-ST-20251113-act-005-03-language-toggle-2025-11-18-T192254Z.md:4-16`。 |
| LEG-UI-09 | loading/empty/error/disabled状态。 | `PARTIAL / DRIFT`：组件有局部状态，旧Phase1要求完整状态面；没有全UI一致完成证据。 | `frontend/src/App.tsx`及components；`0a61028:.orchestrator/phase-1/00-plan.md:21-33`。 |
| LEG-UI-10 | 桌面与移动体验。 | `PARTIAL / UNKNOWN`：旧CSS有响应式痕迹，未找到完整真实设备矩阵和正式视觉验收。 | `frontend/src/**/*.css`、旧release；不能称移动端完成。 |
| LEG-UI-11 | Fetch后课程查询刷新。 | `IMPLEMENTATION_GAP`：成功后刷新字典/重设范围，但query有45秒同key缓存且无显式refetch。 | `frontend/src/hooks/useCourseQuery.ts:89-247`、`frontend/src/components/DataFetchCard.tsx:115-125`、`README.md:139-147`。 |
| LEG-UI-12 | 产品壳身份。 | `DRIFT`：`frontend/index.html`仍引用不存在的 `/vite.svg`，标题是 `BCSP Filter Playground`；frontend README仍描述mock component playground。 | `frontend/index.html:5-7`、`frontend/README.md:1-18`。 |

## 7. 持久订阅、openSections 与旧事件模型

| ID | 行为级能力 | 状态与可验证行为 | 证据/漂移 |
|---|---|---|---|
| LEG-SUB-01 | 创建持久 section subscription。 | `IMPLEMENTED / TEST_CODE`：term、campus、5 位 index；channel 为 email 或 local_sound；服务端写 SQLite。 | `api/src/routes/subscriptions.ts:19-69,169-274`；subscriptions tests。 |
| LEG-SUB-02 | 未解析section也可订阅。 | `IMPLEMENTED / TEST_CODE`：DB暂无section时接受并存`section_id=NULL`；没有section_id backfill。以后poller只是按term/campus/index功能匹配。 | `api/src/routes/subscriptions.ts:168-274,478-505`、`api/tests/subscriptions.test.ts`。 |
| LEG-SUB-03 | 订阅去重。 | `IMPLEMENTED`：contact hash/type + term + campus + section index 的 active 状态组合复用。 | `api/src/routes/subscriptions.ts:508-535`。 |
| LEG-SUB-04 | 旧subscription状态机。 | `PARTIAL / DECLARED_ONLY / COMPACT_CLAIM`：schema/type/Compact定义pending/active/paused/suppressed/unsubscribed；route实际创建active+verified，只实现active→unsubscribed，其他转换未实现。 | `data/schema.sql:246-280`、`api/src/routes/subscriptions.ts:168-274,664-695`、`Compact-ST-20251113-act-009-01-subscription-model-2025-11-18-T214047Z.md:4-16`。 |
| LEG-SUB-05 | 偏好。 | `PARTIAL / IMPLEMENTED / USER_EXPLICIT`：deliveryWindow、snooze、notifyOn=open部分生效；max、waitlist、自动状态和quiet queue未闭环。P1 Review确认Max notifications是用户明确提出的subscription management需求。 | `api/src/routes/subscriptions.ts:19-47,153-162`、`workers/open_sections_poller.ts:728-748`对比`docs/subscription_model.md:45-49`；`SRC-CUR-005`。其新live模型语义仍待P2/P3定义。 |
| LEG-SUB-06 | 管理/取消订阅 UI。 | `IMPLEMENTED`：旧UI加载所有 active rows，显示 term/campus/index/course/channel，可取消。 | `frontend/src/components/SubscriptionManager.tsx:19-157`。当前必须有subscription management由CUR-WATCH-09/CON-013承载；这不证明旧持久UI符合新live模型。 |
| LEG-SUB-07 | 本地信任模型。 | `IMPLEMENTED / SECURITY_RISK`：列表无auth返回contactValue；数字ID可取消email/local_sound任意channel。 | `api/src/routes/subscriptions.ts:71-81,317-380,636-659`；`api/tests/subscriptions.test.ts:326`测试名仅称local，route风险是全channel。 |
| LEG-SUB-08 | 邮件verification。 | `STUB / BROKEN_CHAIN`：创建即active+verified；模板存在，无verification endpoint/worker。 | `api/src/routes/subscriptions.ts:168-274,538-601`、`configs/templates/email/verification/**`。 |
| LEG-POLL-01 | 集中于本机 worker 的 `openSections` poller。 | `IMPLEMENTED / TEST_CODE`：默认 15 秒、±30% jitter、并发 3、miss threshold 2；可指定 term/campus 或 auto discover。 | `workers/open_sections_poller.ts:226-237,394-413`。 |
| LEG-POLL-02 | Auto discovery。 | `IMPLEMENTED / TEST_CODE`：每约 5 分钟扫描 active subscriptions 获得 term/campus；无本地 section data 时跳过并告警，之后可恢复。 | `workers/open_sections_poller.ts:812-820,949-994`；`workers/tests/open_sections_poller.auto.test.ts`的6组测试定义。 |
| LEG-POLL-03 | Checkpoint/resume。 | `IMPLEMENTED / TEST_CODE`：每 target 保存 last poll/hash/miss 等到 JSON，启动恢复；旧包曾把 checkpoint 打进制品。 | `workers/open_sections_poller.ts:425-517`、`workers/tests/open_sections_poller.auto.test.ts`；`SRC-REL-021Z`中的runtime-state entry。 |
| LEG-POLL-04 | Closed→Open event。 | `IMPLEMENTED`：检测开闭变化、写 open_events、建立 subscription fan-out rows。 | `workers/open_sections_poller.ts:1099-1155,1225-1332`；`data/migrations/003_open_events.sql`。 |
| LEG-POLL-05 | 持续 Open reminder。 | `IMPLEMENTED / DRIFT`：后期 release 不是只报边沿；已 Open 的 section 每 3 分钟 bucket 可再产生 reminder，并用 recent event/last_notified_at 限制。 | `workers/open_sections_poller.ts:1156-1171,1225-1332`。 |
| LEG-POLL-06 | 空 payload 处理。 | `CONFLICT`：早期 spec 把 campus-wide empty 当软故障并不关闭；后续 Compact/release 方向要求空列表也参与 closure，防止永久卡 Open。 | `Compact-ST-20251113-act-010-01-event-spec-2025-11-20-T070509Z.md`对比`Compact-ST-20251113-act-010-02-polling-worker-2025-11-20-T090911Z.md`及`workers/open_sections_poller.ts`；具体release行为须按对应archive代码解释。 |
| LEG-POLL-07 | 历史 cadence 多次变化。 | `DRIFT`：Compact spec 45–75 秒；另有 ≤20 秒 runbook；release 15 秒；data-refresh 文档 60–120 秒；持续 reminder 3 分钟。 | `Compact-ST-20251113-act-010-01-event-spec-2025-11-20-T070509Z.md`、`docs/open_event_spec.md`、`docs/data_refresh_strategy.md`、`workers/open_sections_poller.ts`及`SRC-REL-021Z`/`SRC-REL-022Z`对应代码。 |
| LEG-POLL-08 | Snapshot/event retention。 | `PARTIAL`：ingest追加snapshot，poller按index替换；文档要求30天清理，源码无retention job。 | `scripts/fetch_soc_data.ts:1372-1374`、`workers/open_sections_poller.ts:575-610`、`docs/open_event_spec.md:54`。 |

## 8. 本地声音、邮件与 Discord

| ID | 行为级能力 | 状态与可验证行为 | 证据/漂移 |
|---|---|---|---|
| LEG-SND-01 | 浏览器本地声音开关与 device id。 | `IMPLEMENTED`：device id 与 enabled 写 localStorage；用户动作创建/恢复 AudioContext。 | `frontend/src/hooks/useLocalSoundNotifications.ts:7-102,111-155,225-252`。 |
| LEG-SND-02 | HTTP claim 轮询。 | `IMPLEMENTED / TEST_CODE`：默认每 7 秒 POST claim，错误后 15 秒退避；不是 WebSocket。 | `frontend/src/hooks/useLocalSoundNotifications.ts:68-84,177-223`；`api/tests/notifications.local.test.ts`。 |
| LEG-SND-03 | 旧播放语义。 | `IMPLEMENTED / USER_EXPLICIT`：一个非空批次不论含几条 notification，只播放一次 0.35 秒固定音；最多显示 5 个 toast。P1 Review确认WebUI toast是用户明确提出的subscription management需求。 | `frontend/src/hooks/useLocalSoundNotifications.ts:9-11,135-175`；`SRC-CUR-005`。旧固定批次/数量语义不自动成为当前contract。 |
| LEG-SND-04 | Claim的原子确认。 | `IMPLEMENTED / TEST_CODE / LOSS_WINDOW`：API响应前标sent，浏览器之后才播放；autoplay/tab/播放失败不重投，多tab竞抢。 | `api/src/routes/notifications.local.ts:114-211`、`frontend/src/hooks/useLocalSoundNotifications.ts:135-223`；丢失结论为CODE+INFERENCE。 |
| LEG-MAIL-01 | SendGrid provider。 | `IMPLEMENTED / TEST_CODE / COMPACT_CLAIM`：request payload、错误分类、retry policy、token bucket、模板。 | `notifications/mail/providers/sendgrid.ts`、`notifications/mail/tests/provider.test.ts`、`notifications/mail/tests/retry_policy.test.ts`；`Compact-ST-20251113-act-011-02-provider-adapter-2025-11-20-T143624Z.md`与`Compact-ST-20251113-act-011-03-retry-tuning-2025-11-20-T152057Z.md`。外部真实 SendGrid E2E 未证明。 |
| LEG-MAIL-02 | Mail dispatcher。 | `IMPLEMENTED / TEST_CODE / COMPACT_CLAIM`：从 fan-out 队列 claim，渲染中英模板，发送/重试/记录结果。 | `workers/mail_dispatcher.ts`、`workers/tests/mail_dispatcher.test.ts`；`Compact-ST-20251113-act-003-02-worker-implementation-2025-11-21-T061303Z.md`。 |
| LEG-MAIL-03 | 邮件设置UI/API。 | `IMPLEMENTED / TEST_CODE / SECURITY_RISK`：WebUI提交From/key/dryRun；API明文写user config，response隐藏key。 | `frontend/src/components/MailSettingsPanel.tsx`、`api/src/routes/admin.ts:79-221`、`api/tests/admin_mail_config.test.ts:61-101`。 |
| LEG-MAIL-04 | SMTP。 | `DECLARED_ONLY`：类型/示例提到SMTP，但provider只有SendGrid。 | `notifications/mail/types.ts`、`notifications/mail/config.ts`、`notifications/mail/providers/sendgrid.ts`。 |
| LEG-MAIL-05 | 邮件管理/退订链接。 | `BROKEN_CHAIN`：worker生成manage/unsubscribe URL，frontend无router/URL处理。 | `workers/mail_dispatcher.ts:556-563`、`frontend/src/App.tsx`。 |
| LEG-MAIL-06 | 邮件locale映射。 | `DRIFT`：frontend提交 `en`/`zh`，worker只精确匹配 `en-US`/`zh-CN`，否则fallback，中文订阅可能回退英文。 | `frontend/src/components/SubscriptionCenter.tsx:103-111`、`workers/mail_dispatcher.ts:550-553`。 |
| LEG-DISC-01 | Discord 通知。 | `DELETED_HISTORY / COMPACT_CLAIM / USER_EXPLICIT`：Compact 声称 channel/DM sender、retry、dedupe、dispatcher 和 stub tests 已实现；Git 证明相关提交后又整体删除；2026-01 release 无 Discord 文件。P1 Review确认Discord由用户主动要求删除，原因是降低系统复杂度。 | `Compact-ST-20251113-act-004-01-strategy-2025-11-21-T102608Z.md`、`Compact-ST-20251113-act-004-02-bot-sending-2025-11-21-T112050Z.md`、`Compact-ST-20251113-act-004-03-event-integration-2025-11-21-T130957Z.md`；GIT commits `5da6721`, `8eff5c0`, `6e9992f`, deletion `d24bfa5`,`9283ba1`；`SRC-CUR-005`。 |

## 9. API 健康、日志、测试与运行安全

| ID | 行为级能力 | 状态与可验证行为 | 证据/漂移 |
|---|---|---|---|
| LEG-OPS-01 | `/health` 与 `/ready`。 | `IMPLEMENTED / TEST_CODE`：进程健康、SQLite/schema readiness/dependencies。 | `api/src/routes/health.ts`、`api/tests/health.test.ts`。 |
| LEG-OPS-02 | 请求日志与 trace id。 | `IMPLEMENTED`：request logging plugin、错误 envelope/trace id、course/section query metrics。 | `api/src/plugins/requestLogging.ts`、routes。 |
| LEG-OPS-03 | Poller 指标。 | `IMPLEMENTED`：poll/failure/event/notification count、duration、open index gauge，文本输出。 | `workers/open_sections_poller.ts:753-772`。 |
| LEG-OPS-04 | Fetch summary与报告。 | `IMPLEMENTED / DOCUMENTED`：text/JSON summary与staging存在；历史reports覆盖field/fresh/incremental/poller/mail。 | `scripts/fetch_soc_data.ts:780-835`、`reports/field_validation.md`、`reports/fresh_install_log.md`、`reports/incremental_trial.md`、`reports/poller_durability.md`、`reports/mail_worker_latency.md`。 |
| LEG-OPS-05 | 根验证入口。 | `BROKEN_CHAIN`：root`npm test`固定“no test specified”并exit 1。 | `package.json:10-21`。 |
| LEG-OPS-06 | 分散测试代码。 | `TEST_CODE`：11个文件、45个test定义；course search 5、health 3、subscriptions 10、local notifications 2、admin mail 3、mail config 4、provider 5、retry 3、template 2、dispatcher 2、poller 6。 | 本轮未执行；覆盖不含真实SOC/SendGrid、浏览器E2E、launcher、fresh-machine、sections route、多meeting time、poller snapshot/reminder/empty/dedupe完整语义。 |
| LEG-OPS-07 | Frontend自动化测试。 | `ABSENT`：clean-room在当前frontend和允许旧snapshot中未发现生产测试套件。 | `frontend/package.json:6-24`无test/lint script；clean-room文件清单。 |
| LEG-OPS-08 | TypeScript/命令一致性。 | `DRIFT / COMPACT_CLAIM`：Compact 多次记录 repo-wide typecheck 或 native ABI 问题；REL22 package commands 指向已删除脚本。 | `Compact-ST-20251113-act-006-03-fresh-run-2025-11-21-T192107Z.md`及`Compact-ST-20251113-act-003-03-end-to-end-validation-2025-11-21-T082428Z.md`；`SRC-REL-022Z`的`package.json`与archive entries。 |
| LEG-OPS-09 | 白名单文档与代码一致性。 | `DRIFT`：query文档称服务read-only、DB缺失拒绝启动、filters返回examCodes；实际有mutating routes、DB lazy-open、examCodes缺失。local data model又把term code `7/9` 的Summer/Fall写反。 | `docs/query_api_contract.md`对比`api/src/server.ts`、`api/src/container.ts`、`api/src/queries/filters.ts`；`docs/local_data_model.md:38`对比`api/src/routes/fetch.ts:9-14`。 |
| LEG-OPS-10 | Poller metrics/checkpoint可靠性。 | `PARTIAL`：可选Prometheus文本endpoint存在，但listen未显式绑定loopback；checkpoint直接写文件而非atomic replace；没有统一采集、告警、backlog monitor或log rotation。 | `workers/open_sections_poller.ts:425-517,751-790`；历史`reports/poller_durability.md`不能替代实时证据。 |
| LEG-SEC-01 | 旧服务安全边界依赖loopback。 | `IMPLEMENTED / ASSUMPTION`：API/Vite默认127.0.0.1；admin/subscription/fetch无auth。 | `api/src/config.ts:5-35`、launcher、routes；不能直接迁移公网。 |
| LEG-SEC-02 | Runtime state进入release。 | `RELEASED_SOURCE / IMPLEMENTATION_GAP`：REL21含poller checkpoint；远端后续`f819d3c`才取消跟踪并加强ignore。 | SRC-REL-021Z/021T archive listing、REMOTE commit metadata。 |
| LEG-SEC-03 | 本机ignored邮件配置路径。 | `PATH_METADATA_ONLY`：路径存在且ignored/untracked；正文、key有效性、secret状态未知。 | `git status --ignored`显示`!! configs/mail_sender.user.json`；SAFE-INC-02。 |

## 10. Git、分支与产品面漂移

| ID | 事实 | 证据与意义 |
|---|---|---|
| LEG-GIT-01 | 本地旧 `main`、内部 `dev`、`feature/task-015`、公开 `origin/main` 是不同产品树。 | 当前 refs：local main `2d76217`；task-015 `5714a8f`；origin/main `9c93170`。不能把“当前仓库”当作单一快照。 |
| LEG-GIT-02 | 公开 `origin/main` 含 auto-refresh/scheduled-fetch源码表面，而内部 `dev` 和 task-015 不含。 | tree检查及GIT commits `e770bf2`,`b650d81`；公开树的App/server/container也未将其生产接线。 |
| LEG-GIT-03 | task-015 能力矩阵未合并。 | `5714a8f` 只在 `feature/task-015`；父为 `0a61028`；不在 dev/origin main 祖先链。内容有发现价值，无审批权威性。 |
| LEG-GIT-04 | task-015 中断是执行平面污染/提交问题，而不是已完成产品 review。 | `342e450` 记录跨项目 tmux collision；`8004637` 记录 turn-count mismatch 与 prompt 未提交；三次 dispatch 后暂停。 |
| LEG-GIT-05 | 2026-07-11 旧 P1 执行线必须隔离。 | 首提交 `556afb3`，安全父 `efe8fd6`；从该点至当前 dev 的内容均未用于本库存。只使用 metadata 划界。 |
| LEG-GIT-06 | 当前远端只见 `main`，未见 tags；历史 tags/Release 曾在 Stage P 删除。 | connector + local refs；remote live `ls-remote` 本轮网络超时，故远端 tag 状态以 connector/本地 tracking 与旧 Stage P 交叉，不能夸大为实时全验证。 |

## 11. 测试证据的正确解读

调查中可以确认“测试代码存在”的范围包括：

- course search：core/open、meeting subset、exam、FTS/pagination、includes；
- health/ready；
- subscription 创建、去重、unresolved、取消和本地 id-only 行为；
- local notification claim；
- poller auto discovery、missing data、checkpoint；
- mail dispatcher；
- SendGrid provider、retry、config 与模板检查。

但不能据此写成“旧产品整体通过测试”，原因是：

1. 根 `npm test` 明确失败；
2. frontend 没有已发现的自动化测试面；
3. Compact 记录过 Node 缺失、`better-sqlite3` ABI、repo-wide typecheck 等环境问题；
4. 多数邮件/Discord 验证使用 stub，不证明真实外部联调；
5. 本轮 P1 为保护工作树，没有运行可能写数据库、日志、config 或 checkpoint 的测试；
6. release 包、内部 dev 和公开 main 不是同一 tree。

因此每个结论必须保持四层分离：`实现存在`、`测试代码存在`、`Compact 声称运行`、`真实环境通过未知`。

## 12. P1 Review 已解决项与剩余未知项

### 12.1 用户在 P1 Review 已解决

1. 独立section路径是必要用户契约：默认UI以course为中心并展开sections，但section也必须可独立搜索、独立访问并打开完整详情页。
2. 严格meeting时间规则是明确用户意图：整个section的每个已知必修meeting都必须完整落入对应星期的用户可用窗口。
3. Calendar是用户历史明确需求，不是Agent扩张；当前不进入核心版本，仅作为future feature。
4. 当前不支持macOS，理由是缺少测试设备并降低开发/分发复杂度；允许旧史料未找到更早的正式取消决定。
5. 当前必须有subscription management；WebUI toast与Max notifications是用户明确要求。
6. Discord由用户主动要求删除，原因是降低系统复杂度。
7. BCSP用于优化CSP的筛选、定位效率与低门槛体验，不以替代CSP为目标。

### 12.2 仍保留为 `UNKNOWN` 或 `P2_UNRESOLVED`

1. 最终筛选字段全集，尤其是历史上被删除又部分回来的字段。
2. 严格时间规则下async、TBA/unknown、hybrid、optional meeting、exam和特殊日期的处理。
3. Compact view、saved views、share links 的历史来源和当前去向。
4. 旧server scheduled fetch在当前10分钟目录策略下迁移哪些行为；另行审计browser query auto-refresh/cache UX。
5. 2026-01 release 中未提交工作树改动的精确来源。
6. subscription history、quiet hours、snooze等旧偏好，以及toast/Max notifications在新live watch中的精确计数、上限动作与重连语义。
7. 旧数据 schema 和 composite key 在 Rust 共享核心中的最终形态。
8. 当前 Rutgers 限流、实际端到端延迟与目标容量。
9. 当前“持续提醒”的停止/确认、持续时长、重复Open重触发和多section并发语义（CON-053）。

这些剩余问题不是 P1 漏项；它们是获批 P1 明确保留给 P2/P3/P4/P7 的裁决或验证点。
