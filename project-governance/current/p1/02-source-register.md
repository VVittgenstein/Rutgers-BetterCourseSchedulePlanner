# P1 来源、安全与证据登记

## 1. 文档状态

- **状态**：P1 Review 已通过；本登记随获批 P1 关闭
- **登记日期**：2026-07-12
- **用途**：说明本次 P1 实际使用了什么、读到什么程度、哪些来源被隔离、每类结论能承担多大证明力
- **当前决定的唯一汇总来源**：`project-governance/current/single-mainline-delivery-workflow.md`，已并入 2026-07-12 P1 Review 澄清

## 2. 证据标签与读取级别

### 2.1 证据标签

| 标签 | 可证明内容 | 不能自动证明的内容 |
|---|---|---|
| `USER_EXPLICIT` | 用户在当前对话或项目原始会话中明确表达的意图/问题 | 代码已经实现、当前外部状态 |
| `CODE` | 某个具体 snapshot 中存在可达实现 | 已运行通过、所有分支一致、应进入最终产品 |
| `TEST` | 测试代码直接规定了行为 | 本轮执行通过、生产端到端通过 |
| `DOC` | 旧产品文档曾这样描述 | 代码事实或用户批准 |
| `COMPACT` | 旧任务 Compact 曾这样总结 | 用户原话、真实运行、当前决定 |
| `GIT` | commit/tree/ref/path metadata 证明的演进或存在性 | 未合并文档的审批权威性 |
| `RELEASE` | 某个旧 archive 中实际包含的内容/metadata | 包可运行、包与 repo 同步、应复用 |
| `RAW_SESSION` | 原始项目会话中保留的直接对话 | Agent 的转述可以升级成用户原话 |
| `INFERENCE` | 多源交叉后可合理推出的解释 | 不能伪装成直接事实 |
| `CONFLICT` | 两个以上允许来源互相不一致 | P1 不擅自选择胜者 |

### 2.2 读取级别

| 级别 | 含义 |
|---|---|
| `FULL` | 全文/全部相关文件逐行读取 |
| `TARGETED` | 只读明确相关文件、行段或结构化字段 |
| `STRUCTURAL` | 只看路径、tree、ref、commit parent/subject、大小、时间等 metadata |
| `PARSED` | 结构化 JSON/JSONL 被解析或定向提取 |
| `HASHED` | 只计算 hash/条目数，不据此解释正文 |
| `NOT_READ` | 未读取正文，也未用于结论 |
| `DISCARDED` | 曾发生边界级误触，但该执行者/结果被隔离，不进入证据链 |

## 3. 安全边界

### 3.1 明确禁区

以下路径只作为禁区名称登记。主线没有有意打开、引用、概括或复用其内容；但SAFE-INC-01的错误`rg`可能机械扫描了`docs/`下的禁区，返回报告仅明确包含旧P1目录的路径/标题行，整个agent结果已丢弃：

| 禁区 ID | 路径/范围 | 本轮处理 |
|---|---|---|
| FORBID-01 | `docs/deliverable-a-windows-local-release-requirements.md` | `SAFE-INC-01 POTENTIAL_TOOL_SCAN`；无报告返回内容，未使用 |
| FORBID-02 | `docs/p1-a-recovery/` | `SAFE-INC-01 TOOL_SCAN/TITLE_MATCHES`；路径/标题级context隔离，未使用 |
| FORBID-03 | `.ngagent/` 中旧 P1 task/report/state/worktree/log/派生物 | `NOT_READ` |
| FORBID-04 | 旧 P1 执行 commit `556afb3cb91fcadc222efef431bca954c7732cbe` 至当前 `dev` 的文件 blob/diff/merge 内容 | `NOT_READ`；仅 metadata 划界 |
| FORBID-05 | `docs/dual-delivery-workflow.md` | `SAFE-INC-01 POTENTIAL_TOOL_SCAN`；无报告返回内容，未使用 |
| FORBID-06 | `docs/public-web-target.md` | `SAFE-INC-01 POTENTIAL_TOOL_SCAN`；无报告返回内容，未使用 |
| FORBID-07 | `docs/deployment-platform-decision.md` | `SAFE-INC-01 POTENTIAL_TOOL_SCAN`；无报告返回内容，未使用 |
| FORBID-08 | `docs/shared-rust-architecture-decision.md` | `SAFE-INC-01 POTENTIAL_TOOL_SCAN`；无报告返回内容，未使用 |
| FORBID-09 | 根目录 `chat-log-codex-2026-07-10-1ce70862*.md` | `NOT_READ` |

### 3.2 Git 安全截止线

| 项目 | 值 |
|---|---|
| 当前 `dev` | `a4b035a586a4b14fc3a75698caf99badce869fd5` |
| 旧 P1 执行线首提交 | `556afb3cb91fcadc222efef431bca954c7732cbe` |
| 安全父提交 | `efe8fd6a5dddb09a2621459860297ae561d8d1ae` |
| 明确有旧产品价值的最后主线提交 | `8004637c47e40ee3417b4d74d898124bd4b975f0` |
| 被隔离提交数量 | `556afb3^..dev` 共 38 个 |

对旧 P1 区域只检查了 commit parent、时间、subject 和 path name。path metadata 证明这 38 个提交没有触碰：

- `api/**`
- `frontend/**`
- `scripts/**`
- `workers/**`
- `notifications/**`
- `configs/**`
- `data/schema.sql`
- `data/migrations/**`

因此当前工作树中的上述产品源码可被独立调查；这不构成读取旧 P1 内容。旧 P1 相关 feature refs `task-016` 至 `task-067` 也整体隔离，不与 2026-05 的旧产品计划 task-016～025 混同。

### 3.3 边界事件披露

| 事件 ID | 发生了什么 | 暴露级别 | 处置 | 对结论的影响 |
|---|---|---|---|---|
| SAFE-INC-01 | 第一位源码盘点 subagent 的一次 `rg` 标题扫描写错排除 glob；该工具因此机械扫描了禁区文件，并向该 subagent context 返回若干路径/标题匹配行。 | 工具扫描发生；进入模型context的是路径/标题匹配行，未报告返回其他正文。 | 立即中止该 agent；其全部调查结果 `DISCARDED`；重新启动clean-room agent，只允许逐项白名单路径，禁止对`docs/`根执行glob/递归/全文搜索。 | 最终材料不使用该agent任何发现。P1不能声称“零扫描/零触碰”，只能声称“误扫返回的标题级上下文被隔离，未进入主线结论”。 |
| SAFE-INC-02 | clean-room白名单最初写成了过宽的 `configs/**`，使该agent对被Git忽略的本地 `configs/mail_sender.user.json` 做了secret-shape分类。 | 该agent报告“非占位secret-shaped”；值没有显示、复制或返回主线。主线仅用Git metadata确认路径存在且ignored/untracked。 | 私有配置正文和该shape分类从证据链排除；停止内容检查；来源白名单收窄为tracked examples/schemas/templates；本轮不修改/删除/显示该文件。 | P1只能登记“ignored本地配置路径存在、内容未知”；后续secret/artifact audit须覆盖。不得再声称其含有效或非占位secret。 |

用户设置禁区的目的，是防止偷懒复用和上下文污染。上述处置遵循该目的，而不是只做形式上的路径回避。

2026-07-12 P1 Review 中，用户明确接受 `SAFE-INC-01` 与 `SAFE-INC-02` 的隔离处置。两起事件仍永久保留在证据登记中，但不再阻止 P1 关闭；无需因这两项事件重做调查。

## 4. 实际来源登记

### 4.1 当前权威层

| 来源 ID | 来源 | 读取级别 | 证据角色 | 备注 |
|---|---|---|---|---|
| SRC-CUR-001 | `project-governance/current/single-mainline-delivery-workflow.md` | `FULL` | 当前产品、架构、双任务双包、阶段与审核门的唯一权威汇总 | P1 启动时 SHA-256 为 `EF199DAC55CD9D919E2CC3DABEFD80709B626A0B24DF8CA0A3AB369465E52B56`；并入获批 Review 澄清后的 SHA-256 为 `B26451BB6A05C6E4330B9F710F38B4F3735E56FD7A444715385B4026A0F6D5A0`。 |
| SRC-CUR-002 | 当前对话中用户 2026-07-12 的明确指示 | `FULL` | `USER_EXPLICIT` | 禁止旧 P1 是为防止复用/污染；所有旧 RBCSP 一手材料在范围内；Compact 特别重要；正式进入 P1。 |
| SRC-CUR-003 | `project-governance/current/p1/00-p1-charter.md` | `FULL` | 本次 P1 的执行边界 | 不替代工作流中的产品决定。 |
| SRC-CUR-004 | `project-governance/current/p1/01-preflight-baseline.md` | `FULL` | 启动时工作树、branch、HEAD 和 source-root 基线 | 保护既有 dirty worktree。 |
| SRC-CUR-005 | 当前对话中用户 2026-07-12 的 P1 Review 批准与八项回答 | `FULL` | `USER_EXPLICIT`：接受两起事件、批准P1；确认产品起因、严格meeting规则、course-centered与独立section路径、Calendar历史与future定位、当前取消macOS、subscription management及toast/max notifications、Discord删除原因和低门槛目标 | 新当前决定已汇总进`SRC-CUR-001`；历史澄清直接进入获批P1记忆。 |

### 4.2 Compact：独立完整证据域

| 来源 ID | 来源 | 读取级别 | 证据角色 | 限制 |
|---|---|---|---|---|
| SRC-CMP-001 | `docs/archive/stage-a-legacy/Compact/` 全部 74 个文件 | `FULL`，逐行 | `COMPACT`：恢复 2025-11 的设计、实施、review、测试声称和变更轨迹 | 全部是 Agent 任务总结；其中 “confirmed/implemented/tests pass” 不自动升级为 `CODE/TEST/USER_EXPLICIT`。 |

完整覆盖分组：

| Compact 组 | 文件数 | 恢复主题 |
|---|---:|---|
| `soc-api-validation` | 4 | SOC probe、字段矩阵、限流 |
| `act-001` | 4 | 抓取 pipeline、ingest、数据验证 |
| `act-002` | 8 | React 筛选架构、组件、API 集成、frontend MVP |
| `act-003` | 12 | mail worker contract、实现、E2E |
| `act-004` | 8 | Discord strategy/sender/integration |
| `act-005` | 4 | 中英 copy audit 与 i18n |
| `act-006` | 3 | 本地部署、自动化脚本、fresh run |
| `act-007` | 9 | SQLite entity、migration、incremental strategy |
| `act-008` | 5 | Fastify API contract、filter engine、hardening |
| `act-009` | 5 | 持久 subscription model/API/frontend |
| `act-010` | 6 | open event、poller、resume/checkpoint |
| `act-011` | 3 | MailSender/SendGrid/retry |
| `filter-rewrite` | 3 | 2025-11-23 筛选面重写 |
| **合计** | **74** | **全部逐行读取** |

为固定本轮Compact corpus，按“文件名升序，每行`filename|FILE_SHA256_UPPER_HEX`，UTF-8编码，记录间用LF连接且末尾无LF，再取SHA-256大写hex”的manifest digest为：

`C60BAD39567DEDC9E0FD70DA408450034496F98E2EE2E335AF5DFE7D2C18BE62`

该 digest 只证明本轮 corpus 快照，不赋予 Compact 额外权威性。

### 4.3 当前工作树：产品源码、测试和允许文档

Clean-room 重查实际覆盖：131 个被枚举的非文档路径，其中 TypeScript/TSX 80 个；测试文件 11 个、共 45 个 `test(...)` 定义；24 份逐项白名单旧产品文档完成读取。该执行者没有对 `docs/` 根执行目录级枚举、glob 或递归全文搜索，也没有读取 Compact、Git blob 或旧 P1。`SAFE-INC-02` 所涉及的 ignored 私有配置不作为有效来源，不能用来证明任何产品或secret事实。

| 来源 ID | 白名单 | 读取级别 | 覆盖 |
|---|---|---|---|
| SRC-CODE-001 | `README.md`, `CLAUDE.md`, `read_only.md`, `package.json`, `tsconfig.json`, `Start-WebUI.bat`, `Start-WebUI.command` | `FULL/TARGETED` | 产品身份、启动、文档与 test script |
| SRC-CODE-002 | `api/**` | `TARGETED` | Fastify server/routes、course/section/filter/subscription/fetch/local-notification/admin/health contract 与实现 |
| SRC-CODE-003 | `frontend/**` | `TARGETED` | React App、filter state、课程/section UI、fetch、subscription、本地声音、邮件、i18n、orphan components |
| SRC-CODE-004 | `scripts/**`, `workers/**`, `notifications/**` | `TARGETED` | SOC ingest、one-click、poller、mail/local notification、migration/checkpoint |
| SRC-CODE-005 | tracked `configs/*.example.json`, `configs/*.schema.json`, `configs/templates/**`, `data/schema.sql`, `data/migrations/**` | `FULL/TARGETED` | 公开配置样例/模板、SQLite schema 与 4 个 migration；ignored user/local config正文不属于来源 |
| SRC-CODE-006 | `api/tests/**`, `workers/tests/**`, `notifications/mail/tests/**` | `FULL/TARGETED` | 11个测试文件、45个test定义；本轮未执行 |
| SRC-CODE-007 | `reports/**` | `TARGETED` | 历史 fresh install、field validation、incremental、poller/mail evidence |
| SRC-DOC-001 | 逐项白名单旧产品文档：query/data/SOC/fetch/refresh/load/subscription/open-event/mail/notify/oneclick/quickstart/deployment/UI/i18n/TIMELINE/DIGEST/registry/sessions | `FULL/TARGETED` | 旧契约、runbook、历史记忆与文档漂移 | clean-room 调查禁止从 `docs/` 根枚举；不含 Compact 和禁区。 |

这些来源证明的是当前工作树 snapshot。它不自动代表公开 main、旧 release 或最终目标。

### 4.4 恢复目录

| 来源 ID | 来源 | 读取级别 | 证据角色 | 限制 |
|---|---|---|---|---|
| SRC-REC-001 | `Z:\resume-from-main-machine\Rutgers-BetterCourseSchedulePlanner\current\00-status.md` 至 `10-critical-dialogues.md` | `FULL` | 2026-06-13 的恢复索引、时间线、冲突和 session 路由 | Agent 二手综合，不是用户原话；其中明确承认当时未深读 release。 |
| SRC-REC-002 | 同目录 `forbidden-actions.md`, `manifest.json`, `read-log.jsonl` | `FULL/PARSED` | 恢复现场边界、读取日志和 corpus metadata | 只用于来源路由和交叉检查。 |
| SRC-REC-003 | 三份 session candidate index Markdown | `FULL` | Codex/Claude 候选会话索引 | 索引内容不自动等于会话正文。 |
| SRC-REC-004 | 三份 candidate index JSON | `PARSED` | 6,018 个 JSONL 的结构化索引；159 个候选（Codex 95、Claude 64） | 只定向选择 RBCSP 会话，不做个人历史无差别扫描。 |

恢复目录共 20 个文件完成上述读取/解析。

### 4.5 旧 release

| 来源 ID | 制品 | 读取级别 | 条目 | SHA-256 | 证据角色 |
|---|---|---|---:|---|---|
| SRC-REL-021Z | `release/bcsp-20260121.zip` | `STRUCTURAL + TARGETED FULL` | 136 files | `F62F14D2CEE0DE4BD90931E37808141FB45DF970E39CFF7BAAB78E9A999A9A50` | 2026-01-21 实际源码包内容/metadata |
| SRC-REL-021T | `release/bcsp-20260121.tar.gz` | `STRUCTURAL + TARGETED FULL` | 136 files + dirs | `827D2EF1F59357780AC70A92F489A92246D5EC4BC1EDE2BEF2EC1CBFA63951AD` | 与 zip 同批内容的 tar 容器、Unix metadata 对照 |
| SRC-REL-022Z | `bcsp-20260122.zip` | `STRUCTURAL + TARGETED FULL` | 125 files | `48E976EF9B2EFCBFB692F6CC119C790BF7D4D3E9A361C464E38F61A89A5AFAD1` | 2026-01-22 修订包、脚本/权限/命令漂移 |

核验包括 archive entry、路径分隔、line ending/external attributes、package scripts、startup、API/UI/poller/notification 关键源码和测试存在性。没有解压覆盖工作树，也没有运行旧包。

### 4.6 Git 与旧 task-015

| 来源 ID | 来源 | 读取级别 | 证据角色 | 限制 |
|---|---|---|---|---|
| SRC-GIT-001 | 旧 P1 截止线之前/之外的 commit graph、refs、tree/path metadata | `STRUCTURAL` | 分支谱系、功能出现/删除、release 与 public/internal 分裂 | 旧 P1 后代只做 metadata 隔离。 |
| SRC-GIT-002 | `0a61028c91a93906758d41120fd9544ae889cbc7` | `FULL`（允许路径） | 2026-05 旧 Phase 1 本地 release plan | 旧方向，非当前决定。 |
| SRC-GIT-003 | `feature/task-015@5714a8f19481d22691ba799992609e6a5f619d02` | `FULL`（允许旧 RBCSP 文档） | 能力域、模块图、release reconciliation、Stage A/P 历史候选 | 未合并、未 review；分类不继承。 |
| SRC-GIT-004 | `342e4502a3466c3e88d1291e5ffff2754e1acc30`, `8004637c47e40ee3417b4d74d898124bd4b975f0` | `FULL/TARGETED`（允许历史） | task-015 中断原因和执行平面污染 | 证明旧矩阵未被验收，不证明其内容正确。 |
| SRC-GIT-005 | 2025-11 至 2026-01 旧产品 commits | `STRUCTURAL/TARGETED FULL` | SOC、SQLite、API、React、subscription、poller、mail、Discord、one-click、local sound、auto-refresh 的演进 | 只读取旧产品相关 blob。 |
| SRC-GIT-006 | local `main`, `dev`, `auto-refresh-tasks`, `feature/task-015`, `origin/main` tree 对照 | `STRUCTURAL` | 证明当前存在多个不同产品 snapshot | 不能选择其中任一为最终产品。 |

关键 refs：

| Ref | SHA | 事实 |
|---|---|---|
| local `main` | `2d762179…` | 2026-01-18，未含 auto-refresh |
| `feature/task-015` | `5714a8f…` | 未合并旧矩阵，未含 auto-refresh |
| `auto-refresh-tasks` | `52a5072b…` | 含 `e770bf2`，另有编排初始化 |
| `origin/main` | `9c93170c…` | 当前公开产品树，含 scheduled-fetch/auto-refresh |
| `dev` | `a4b035a…` | 当前内部线；旧 P1 文档区被隔离，产品源码路径未被旧 P1 commit 修改 |

### 4.7 GitHub 公开面

| 来源 ID | 来源 | 读取级别 | 证据角色 | 本轮状态 |
|---|---|---|---|---|
| SRC-GH-001 | GitHub connector：`VVittgenstein/Rutgers-BetterCourseSchedulePlanner` repo metadata/default branch | `STRUCTURAL` | 当前公开 repo、default `main`、repo id、可见 refs | 只读 |
| SRC-GH-002 | GitHub connector：public README、`package.json` | `FULL` | 当前公开文档/metadata 漂移 | 只读 |
| SRC-GH-003 | GitHub connector：commit/PR 搜索 | `TARGETED` | filters、subscription、mail、refresh 等历史 PR 系列 | 只读；以 GitHub 当前索引为准 |
| SRC-GH-004 | 本地 tracking tree 与 connector 交叉 | `STRUCTURAL` | `origin/main=9c93170…`，公开树含 auto-refresh；只见 main，无 remote task-015 | live `git ls-remote` 本轮网络超时，因此不把命令失败伪装成实时核验成功。 |

本轮没有修改 issue、PR、release、branch 或远端。

### 4.8 项目原始会话

| 来源 ID | 原始会话 | 读取级别 | 直接贡献 |
|---|---|---|---|
| SRC-RAW-001 | `Z:\.codex\sessions\2026\01\21\...019be0f0-d27d-7511-8725-01026d6e9f25.jsonl` | `TARGETED/PARSED` | 用户要求 README 让完全零经验用户也能逐步使用。 |
| SRC-RAW-002 | `Z:\.codex\sessions\2026\01\22\...019be701-4756-7532-a53d-fcfb87692d56.jsonl` | `TARGETED/PARSED` | 第5条用户消息报告朋友Mac双击`.command`后未启动；原文件第161/173行表明用户没有Mac经验但仍要求修成Mac可直接使用。 |
| SRC-RAW-003 | `Z:\.codex\sessions\2026\05\10\...019e14e4-6b01-7791-8350-8ee8829845bc.jsonl` | `TARGETED/PARSED` | 用户明确说旧开发流不成熟、修改仓促、repo/release 可能不同步、实现/设计较低级，并要求清洗/剥离/整理/重构。 |
| SRC-RAW-004 | `Z:\.codex\sessions\2026\05\12\...019e1ad0-48f6-7153-a0ae-51dd99a342e8.jsonl` 及允许的导出 `docs/chat-log-2026-05-12T06-12Z.md` | `FULL` | 完整本地release目标、all-and-only含义、UI/UX重建、核心路径、task-015停止；原会话第875行（892重复）明确“只有Windows环境”但仍希望Win/Mac均可直接使用。 |
| SRC-RAW-005 | 同日 task-015 attempt 原始 Codex sessions `019e1b8b…`, `019e1bff…`, `019e1c13…` | `STRUCTURAL/TARGETED` | collision、retry、prompt 未提交的执行历史 | 产品内容不由这些 worker session决定。 |
| SRC-RAW-006 | `Z:\.claude\projects\Z--Project-Rutgers-BetterCourseSchedulePlanner\sessions-index.json` 定向 session `69294bc7…`, `6a3d7772…` | `TARGETED` | DB 相对路径、严格 meeting filter、FilterPanel 独立滚动、release 只含主产品文件等用户反馈。 |

只定向读取 RBCSP 项目会话；没有对个人 `.codex/.claude` 历史作无差别内容扫描。

## 5. 产品域覆盖矩阵

| 产品域 | Compact | Code/Test | Git | Release | Raw/User | 当前决定 | 覆盖结论 |
|---|---:|---:|---:|---:|---:|---:|---|
| 产品身份/普通用户路径 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 已覆盖 |
| Windows/macOS 启动 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 已覆盖，含 Mac 失败 |
| 打包/release 漂移 | — | ✓ | ✓ | ✓ | ✓ | ✓ | 已覆盖三制品 |
| SOC/限流/字段 | ✓ | ✓ | ✓ | ✓ | — | ✓ | 已覆盖，当前外部限制仍待后续核验 |
| SQLite/schema/migration | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 已覆盖 |
| full/incremental/refresh | ✓ | ✓ | ✓ | ✓ | — | ✓ | 已覆盖，存在分支分裂 |
| 课程查询/筛选 | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 已覆盖，含严格 meeting 语义 |
| section 信息/API | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 已覆盖，独立 route 为 stub |
| React UI/UX/i18n | ✓ | ✓ | ✓ | ✓ | ✓ | ✓ | 已覆盖，旧正式完成度未知 |
| subscriptions | ✓ | ✓ | ✓ | ✓ | — | ✓ | 已覆盖，旧持久模型被当前 live watch 重构 |
| openSections/poller/event | ✓ | ✓ | ✓ | ✓ | — | ✓ | 已覆盖，cadence/empty/dedupe 冲突明确 |
| 本地声音 | — | ✓ | ✓ | ✓ | — | ✓ | 已覆盖，旧 HTTP claim 与当前 WebSocket 冲突明确 |
| mail/SendGrid/SMTP | ✓ | ✓ | ✓ | ✓ | — | ✓ | 已覆盖，当前明确不进版本 |
| Discord | ✓ | — | ✓ | — | — | ✓ | 已覆盖曾实现后删除；原因未知 |
| 健康/日志/测试 | ✓ | ✓ | ✓ | ✓ | — | ✓ | 已覆盖，不能宣称整体 tests pass |
| 公网平台/Rust/双包 | — | — | — | — | ✓ | ✓ | 属当前新增目标，不要求旧实现证明 |

## 6. 来源限制

1. Compact 的全部 74 份内容已完整读，但它们仍是 Agent 总结。
2. 当前工作树、公开 main、内部 dev、task-015 和三个 release 不是同一 tree。
3. 本轮没有运行会写 DB、logs、config、checkpoint 或依赖目录的产品测试；因此只登记测试代码存在，不声称本轮通过。
4. `git ls-remote` 的一次实时网络调用超时；GitHub connector 和本地 tracking tree已提供公开 main 交叉证据，但最终发布前仍应重新核验远端、tags、releases。
5. 旧 P1 正文被禁止，因此无法也不需要拿它来“补齐”任何结论。
6. `D:\Document\Obsidian\Adrian\Prompt\BetterCourseSchedulePlanner` 在本机不存在；用户提到的早期资料只能由其他一手来源交叉恢复，不能伪装为已读。
7. SAFE-INC-01 必须保留在最终审计中；不得把“已隔离污染结果”改写为“从未发生过工具扫描/误触”。用户已在P1 Review接受该处置。
8. SAFE-INC-02 必须保留；ignored私有配置的内容、key有效性和secret状态均视为未知，本次只保留路径metadata。用户已在P1 Review接受该处置。

## 7. 精确可复核清单

### 7.1 24份逐项白名单旧产品文档

以下是clean-room允许并报告完成读取的精确文档清单；除这些路径和独立Compact域外，不允许从`docs/`根扩张：

```text
docs/query_api_contract.md
docs/local_data_model.md
docs/soc_api_notes.md
docs/soc_rate_limit.md
docs/fetch_pipeline.md
docs/data_refresh_strategy.md
docs/data_load_runbook.md
docs/subscription_model.md
docs/open_event_spec.md
docs/mail_worker_contract.md
docs/mail_sender_contract.md
docs/mail_sender_usage.md
docs/notify_runbook.md
docs/oneclick.md
docs/quickstart.md
docs/deployment_playbook.md
docs/ui_flow_course_list.md
docs/i18n_key_map.md
docs/TIMELINE.md
docs/DIGEST.md
docs/registry.json
docs/sessions/README.md
docs/sessions/2026-05-11-pm-init-setup.md
docs/sessions/2026-05-12-public-remote-closeout-savepoint.md
```

### 7.2 Compact逐文件manifest

下列74行就是用于计算SRC-CMP-001 corpus digest的原始manifest：

```text
Compact-ST-20251113-act-001-01-pipeline-config-2025-11-17-T084726Z.md|EBFFB596D5567E96DE1F37F966ACBB73747A5F4010132601F81EC6EDD7A08B86
Compact-ST-20251113-act-001-01-pipeline-config-2025-11-17-T091916Z.md|57545D7FBB254AB91ECC50D46D2163EEF203CB67EBC7975B19DE5DDFA054B2FB
Compact-ST-20251113-act-001-02-ingest-impl-2025-11-17-T101012Z.md|9BC40B7CED3E8040E69A3F23B349F538612116DE0847D7E000F379DCC7A8717A
Compact-ST-20251113-act-001-03-data-verification-2025-11-17-T104201Z.md|E3DA0B3108BEF1712DCD806DBCE628ED1F9F9AD75C128CC527C7755132A182DB
Compact-ST-20251113-act-002-01-ui-architecture-2025-11-18-T123742Z.md|A56E0BB639A47BFC3554FF707986704BB1F951B58C925D3D1ACC4A41849CA21D
Compact-ST-20251113-act-002-01-ui-architecture-2025-11-18-T124836Z.md|EC5AE3186B7932D318AB408C9A1E727E11114A54BCC62F662BD1E32BD1FBEA14
Compact-ST-20251113-act-002-02-filter-components-2025-11-18-T130603Z.md|3C21EC52F552BA5A47F180ED8E6AD457BC71F09C5FCB6C92F6B21B7382791819
Compact-ST-20251113-act-002-02-filter-components-2025-11-18-T131627Z.md|BB4EAE4956EFE2B558E3258910089D2C79E655DAC436DA3CD251DEC31F07793A
Compact-ST-20251113-act-002-03-api-integration-2025-11-18-T142029Z.md|32FAE516299F27EDF93F63AB5A7124FA94C59AEE690D3896ECEE5841A098B9F6
Compact-ST-20251113-act-002-03-api-integration-2025-11-18-T143124Z.md|02F4BC4531A04425E6FBDAF74433F4EFAE894559E8EF679E213EC0479465055C
Compact-ST-20251113-act-003-01-worker-contract-2025-11-20-T155929Z.md|3C7E4425530C30E3BBD2B4464E04FE3D32F725B135656B0F2CE39E8E6E4F8024
Compact-ST-20251113-act-003-01-worker-contract-2025-11-20-T161215Z.md|2E1370539C74FF8D8F9B8008E3D35DAA8C0246F1B0EA249568E2720D8594C3C8
Compact-ST-20251113-act-003-01-worker-contract-2025-11-20-T163622Z.md|38E0DCE1E20D62979138AE857F6F9DFDCD158772D05DA534CE83BE5373D6842E
Compact-ST-20251113-act-003-01-worker-contract-2025-11-20-T164408Z.md|E8E01934367B3C3F1767392594F6F799D887FA9141FB1BD3A7F28F5D6786AEEC
Compact-ST-20251113-act-003-01-worker-contract-2025-11-20-T164640Z.md|E2A64F7CFFD7351CEC127E94064F636D90F2F86D4BB98492676E26A6472799C5
Compact-ST-20251113-act-003-01-worker-contract-2025-11-20-T165546Z.md|B194625670449B350179D59F22ADF94079E98D8526ED5D33382921D87F5EABF7
Compact-ST-20251113-act-003-01-worker-contract-2025-11-20-T165646Z.md|C3DB0253D1BBD1CE13A1F53E7D3A67B29396F391CD04DF32A697C7AD8B50292F
Compact-ST-20251113-act-003-01-worker-contract-2025-11-20-T165727Z.md|2DC9ADE5E31ED6CCA5FE93AEF431554026AFF23DA56761907A460A8B93DDBDDB
Compact-ST-20251113-act-003-01-worker-contract-2025-11-20-T171316Z.md|67B0001571B3A05EBF70E673904B38D3442118117B6E601476C28AE7EDCD80DD
Compact-ST-20251113-act-003-02-worker-implementation-2025-11-21-T061303Z.md|52099A963563F53FB91B0026E4BDC7DF8A892C90AA026EB5E3B08B54F0E55625
Compact-ST-20251113-act-003-03-end-to-end-validation-2025-11-21-T073814Z.md|A7218E282FC8AE2D7909FD6119D6DC436E6109AC32282A068780C4F4FD1E1B25
Compact-ST-20251113-act-003-03-end-to-end-validation-2025-11-21-T082428Z.md|478266C9E3ECEFDBC6AAAF86A25663A5D7C9ED7A2E767B118FC033BF0E3D8CE5
Compact-ST-20251113-act-004-01-strategy-2025-11-21-T101540Z.md|C1BC964B0EC531A31B1F552D8EA67DCC1F74A4F0570774BD3BF9E7DB2E73DD89
Compact-ST-20251113-act-004-01-strategy-2025-11-21-T102608Z.md|858DEF8AEADFBE0CA7EE0EEA7357C01D814E2D88D582C5599224E0CAED065F1F
Compact-ST-20251113-act-004-02-bot-sending-2025-11-21-T105707Z.md|F7D537C70968C03C75E4835F6F95B6192D8B6EB37328037D2A921B0A9FF8760E
Compact-ST-20251113-act-004-02-bot-sending-2025-11-21-T110000Z.md|5B3B0BA42E156A8DC0307B61630E6A5B6AADE460E601F2E5DA09EAD7475955A1
Compact-ST-20251113-act-004-02-bot-sending-2025-11-21-T112050Z.md|E49483EF935E925E24481092F1DA7A04BD6439BAD5F88499C6E72D2F36D7AA69
Compact-ST-20251113-act-004-03-event-integration-2025-11-21-T115350Z.md|40A316797A30DCEE3BA609AADC1C99129CBCA717659A4FE1921B27526C3CA416
Compact-ST-20251113-act-004-03-event-integration-2025-11-21-T124952Z.md|C0A9078109BD71A4853708FED17365086A8CF719C306AEC6BB35F6453AD72F0B
Compact-ST-20251113-act-004-03-event-integration-2025-11-21-T130957Z.md|060017260009D6B0611B1508556B0DD1F2261530DA1D0B4A4C0A77E2C2FBD7A5
Compact-ST-20251113-act-005-01-copy-audit-2025-11-18-T173204Z.md|75F4F42968204AA452F14879D40A6E2300CE618865EDD114B0C355DDE4941BC0
Compact-ST-20251113-act-005-01-copy-audit-2025-11-18-T180657Z.md|020EC4E80BF42B7B9F7D5814ACD99874C50A017EFE0FAFBF66BE82715759C1AB
Compact-ST-20251113-act-005-02-i18n-integration-2025-11-18-T185009Z.md|E1C0F12391BEA713BC2B5848B9BA35B5197AFFE320E0B2D2260775642A03482F
Compact-ST-20251113-act-005-03-language-toggle-2025-11-18-T192254Z.md|85262824417761B9F9ECC419A35D7A4543A291A73F0E09D25379C7AFD3184B13
Compact-ST-20251113-act-006-01-deploy-playbook-2025-11-21-T134648Z.md|D6D3F3C2D59D6A1455A32DC26A7C65F7EC47D5197869D99D9EEAE5847D6ABE78
Compact-ST-20251113-act-006-02-automation-scripts-2025-11-21-T144200Z.md|287C068C13B0B26909D35430A5456E8BCC37F4E442C694C526BFE4F3A103740D
Compact-ST-20251113-act-006-03-fresh-run-2025-11-21-T192107Z.md|7FB4EF26A0BC0977ED8172A5F9E81288F92A83B00FD650D9C9BE499329036C7F
Compact-ST-20251113-act-007-01-entity-design-2025-11-17-T040925Z.md|0D843F41A599A41DE72C46E63822D53A7CC3E3E3D849D4E762059CBCE45AE866
Compact-ST-20251113-act-007-01-entity-design-2025-11-17-T044754Z.md|64F238C983E7C9106351363809FEE130FD24BC1D19FD546813B0BD3DABFACCCF
Compact-ST-20251113-act-007-01-entity-design-2025-11-17-T050547Z.md|42032AE632B221694F5BDAEB17B75141E527B8E6DDEF574C20C15102E1867BFC
Compact-ST-20251113-act-007-02-migration-tooling-2025-11-17-T053943Z.md|E7FECECDBCC93AF6B65FC7FD79F6679B0B093DADFAC864028ABD76C192E242C4
Compact-ST-20251113-act-007-02-migration-tooling-2025-11-17-T055927Z.md|C7BD84766C23B326B8ECABFF675945EA3CE9541787E9955F9BE50F145469F908
Compact-ST-20251113-act-007-02-migration-tooling-2025-11-17-T062555Z.md|BF882925E36D6683BCDC2C4D58BE438460D9A566BA6D7238CCBF010E2B4E33DE
Compact-ST-20251113-act-007-02-migration-tooling-2025-11-17-T063625Z.md|410781988BE486AC1E38C6304FA7AA71068BF2C3DD46778EDB05C8ED8744762C
Compact-ST-20251113-act-007-03-incremental-strategy-2025-11-17-T082525Z.md|C7AB6C6176C971B6D038761719813F1F7B249AF2141FC08296AB0116EE11F3B6
Compact-ST-20251113-act-007-03-incremental-strategy-2025-11-17-T083615Z.md|06F7A329C0DA1EC1342C37178BD58949F5D885B36D625F457458F178D4BC0ED9
Compact-ST-20251113-act-008-01-contract-2025-11-18-T023627Z.md|655F4661A6F2FE596E9A08FB517F851F6B37CCE2FBBD12281ECFDFD910B2CDB2
Compact-ST-20251113-act-008-01-contract-2025-11-18-T030058Z.md|A2393272496839994B07A92B7FAE64636DDB1CBF3C6B64B54309F5E7694F43A8
Compact-ST-20251113-act-008-02-filter-engine-2025-11-18-T035146Z.md|56E9CDE670519C65D5FAF49D551D64A6B524491510AA76788D76448BC0E19A3E
Compact-ST-20251113-act-008-02-filter-engine-2025-11-18-T040307Z.md|50BC947E46E5976C4C290654C914663C59940B25B367E3BAC3170FEEA7EB0C20
Compact-ST-20251113-act-008-03-api-hardening-2025-11-18-T042735Z.md|F89C2FBCC526BF6930D133095B0CD4320B8EA62D8A675B987DD6C85368234A73
Compact-ST-20251113-act-009-01-subscription-model-2025-11-18-T214047Z.md|19E8F1545BB5AAB9AFD7B5AF6347161327224B790C0B8506DA030096183302F3
Compact-ST-20251113-act-009-02-subscribe-endpoints-2025-11-19-T012202Z.md|FAC4E7A1A62C7A1D46437031077735DF5B35AFE08CC00271D47D1F2904ACBCED
Compact-ST-20251113-act-009-02-subscribe-endpoints-2025-11-19-T050717Z.md|EE6DAFC9CDE7CC571751BB1D44C490BD1E2C9CA91F573B1843AB5329B4C57E60
Compact-ST-20251113-act-009-02-subscribe-endpoints-2025-11-19-T052121Z.md|29DB3A8E8ADC6C7A04CBEB2E3DD5C5A53E1FA0302336BB257E9A9E239E93099B
Compact-ST-20251113-act-009-03-frontend-flow-2025-11-20-T060040Z.md|B6B5B38A8E04D4937A14C71E7EE50CA954DBFFCB612FA8C646749DCAA3AD0E14
Compact-ST-20251113-act-010-01-event-spec-2025-11-20-T070509Z.md|E5A566AD9BC45A9D83C0CA525FC9B961AD523AEB320FCBFBA7A363DD92A1C6CA
Compact-ST-20251113-act-010-02-polling-worker-2025-11-20-T085849Z.md|6F3586109B05E0CA0385D73A16EC59942FAA75A1264D60AE1211AE95C3B49FCA
Compact-ST-20251113-act-010-02-polling-worker-2025-11-20-T090911Z.md|52A0F0C06E77DF93EA06D318F0408F56A249771F449FD0E728376D3075FC1982
Compact-ST-20251113-act-010-03-resume-tests-2025-11-20-T094119Z.md|870A62FE0A95BDBF78538846C77173E1CFBC81EC9BAD6F199AF067E64090B27D
Compact-ST-20251113-act-010-03-resume-tests-2025-11-20-T095818Z.md|3E9D99E4861617F44B0B0782081F1E8A478EE48095DC20D05D08BC609BE55E02
Compact-ST-20251113-act-010-03-resume-tests-2025-11-20-T102410Z.md|B9243A1074937A20620B30F6EC904DBE378F5D294C7B0EE9BD9EE16EAF20BD60
Compact-ST-20251113-act-011-01-mail-interface-2025-11-20-T134509Z.md|18BF3C47F599DE89634F84297D2ACFA146FBE94D824C76C7184BCCF5104DF10E
Compact-ST-20251113-act-011-02-provider-adapter-2025-11-20-T143624Z.md|F273E0FCD3B592899987DD2FFB41EF64B4A8497BC0FE0785699E867D203DC430
Compact-ST-20251113-act-011-03-retry-tuning-2025-11-20-T152057Z.md|51E2949DDDA2899F81F91E72AC0C1FE6B249B88A76E1E50EC228CD7E8EB93345
Compact-ST-20251113-soc-api-validation-01-probe-2025-11-16-T133825Z.md|412054680CA431DEB00FA52368E244C5B7DE55445D57206BB332279516C5854B
Compact-ST-20251113-soc-api-validation-02-field-matrix-2025-11-17-T011405Z.md|3EDEBF2253E2E4A748B717EEAE2859639480E2EE04BAC467C901DF5D1CBE93C1
Compact-ST-20251113-soc-api-validation-02-field-matrix-2025-11-17-T013508Z.md|BD94B87EA2549DF4454B30277A5DA25AE883E064AFCC5296679ADFBAAF206194
Compact-ST-20251113-soc-api-validation-03-limit-profile-2025-11-17-T015611Z.md|7B111AA15BAF6880EB559BCB2E7DC8E92E2E5DCD744561BD2BB2C7ECCF9A674C
Compact-ST-20251122-filter-rewrite-01-frontend-state-ui-2025-11-23-T062853Z.md|FC4986718DF5982BE7A9CA9B9D6F5A6AD2C764BC9CC4228E6F298737AE88F2A6
Compact-ST-20251122-filter-rewrite-02-api-schema-query-2025-11-23-T082002Z.md|5943CF0FF7BE1B159BF0BE274A589E26540B077A04A1088A8C105B38F0DE5644
Compact-ST-20251122-filter-rewrite-03-data-pipeline-dicts-2025-11-23-T110512Z.md|E0E0CDE5151AEA6B55B7B4368F2D78735F750A7C73840B36C77D437E5E618C00
Compact-T-20251113-act-002-frontend-filter-mvp-2025-11-20-T023714Z.md|387ED4F0A22508E77DFCE040E4EB6B6A5676F6E85CFDF688F2BFB8C306635F25
Compact-T-20251113-act-002-frontend-filter-mvp-2025-11-20-T030356Z.md|B3E2B15A498DC2A6BAA42447EB2D44D2DB39B6E707DBE273A97B6F657EA62AA7
```
