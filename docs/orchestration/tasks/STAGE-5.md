# Stage 5：S3 离线证据与生产 GO/NO-GO

状态：**DELIVERED — CODEX `CHANGES_REQUIRED`；见 `STAGE-5-R1.md`**
Prompt 版本：`STAGE-5/v1-parallel-evidence`
Orchestrator：Codex
实现者：实现代理
产品代码父基线：`553371f8fa449b8c7cb9a88b5f32e179cb1e57c5`

## A. 任务包

### A1. 当前裁定

现有最强数据只有 92026/NB 的一个夜间 2h05m 窗口、554 个样本。只读初审形成 67 个变化 bracket 后，
30 秒与 60 秒候选都最多解释 65/67，最佳 phase 同样接近整分钟后 0.1–2 秒。其他本地数据库看似有多个
target，但除初始 LKG 外的连续变化仍基本集中在 NB；July 的多 target 数据只有相隔约 78 分钟的两轮，
不能形成稳定→首次变化的窄区间。

因此当前证据只能支持“变化靠近整分钟”，不能区分 30/60 秒，也不能冻结统一 safe offset。Parallel
Wave 1 的 Stage 5 只交付可复查的离线 analyzer、测试和证据报告，并正常以
`NO_PRODUCTION_CHANGE / DATA_REQUIRED` 结束。不得为了完成 Stage 而修改生产 scheduler。

### A2. Worktree 与唯一写入范围

- worktree：`Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\parallel-wave-1\stage5-s3-evidence`；
- branch：`codex/parallel-wave1-stage5-s3-evidence`；
- 从 orchestration-only anchor 建立，产品代码父基线为 `553371f`；
- 只新增 `tools/s3/**` 与 `docs/evidence/S3-REBUILD-PROFILE.{md,json}`；
- 不修改 production crates、schema/migration、policy/default、scheduler/runtime、packaging 或
  `docs/orchestration/**`；
- 不读取并提交 `.cache` 中的用户/QA 数据库，不提交绝对私有路径或原始数据库；
- 不访问 Rutgers、不联网采样、不部署，不 reset/rebase/merge/push/tag/release。

### A3. Analyzer 输入与可复查输出

实现一个无新 npm 依赖的 Node 24 CLI，支持：

1. 现有 `samples.ndjson`/相应 committed capture metadata，以相邻 canonical body hash 形成区间；
2. 用户明确传入的只读 SQLite 路径，从 `open_batch_observations` 按 target 与 observation sequence 读取；
3. synthetic fixture 供 tests 使用。

SQLite 必须 readonly/immutable，不修改输入。每个 target 的初始 `body_changed=true` 只表示建立 LKG，必须
排除；真实变化 bracket 应来自已知稳定观测到下一次首次 `body_changed=true` 的
`(last_unchanged, first_changed]`，不能把所有 completed timestamp 对 60 取模画直方图后冒充区间删失分析。

机器 JSON 和人类 Markdown 至少报告：

- schema/tool version、命令参数和每个输入的 SHA-256 指纹；
- target、term/campus、独立时间窗口、timezone/UTC 边界、样本数；
- initial/invalid/非信息行排除原因和数量；
- 每个有效 interval 的上下界/宽度以及 target/window 分组；
- 30s 与 60s 候选各自的最佳 phase 区间、coverage、residual/outlier 和按 target/window 留出结果；
- server Date、本地请求起止/延迟信息是否存在，时钟误差无法估计时明确标 unknown；
- 正向 jitter/safe-offset 上界是否可识别；
- 机器可判定的 `GO` 或 `NO_PRODUCTION_CHANGE` 与逐项理由。

CLI 输入顺序不得改变结果；相同输入必须产生 byte-stable JSON（生成时间等易变字段不进入机器件）。错误
schema、乱序/重复 sequence、无变化、仅初始 true、时间倒退和宽 interval 都要有明确行为与测试。

### A4. GO 门

未来只有同时满足以下条件才允许另开 production implementation：

- 真实证据覆盖多个 target，至少能独立评价 NB、NK、CM，而不是同一 QA DB 的副本；
- 覆盖多个独立时段，至少含 America/New_York 17:00–18:00 活跃窗口和一个非峰值窗口；
- 30/60 秒模型在按 target/window 留出验证后有一致且明确可区分的胜者，不是当前 65/67 对 65/67；
- 各 target/window 的 phase 与正向 jitter 可由一个统一 safe offset 覆盖；
- 报告能诚实处理服务器 Date 精度、客户端时钟和请求耗时；
- 结论对少量 outlier/单个 target 留出稳定，而不是靠挑样本成立。

任何覆盖不足、模型近似同分、跨 target/window 冲突、safe offset 无稳定上界或时钟误差无法控制，都必须
`NO_PRODUCTION_CHANGE / DATA_REQUIRED`。本 lane 已知会走该路径；不把 “no change” 当失败。

### A5. Tests 与验证纪律

用 `node:test` 和临时 synthetic NDJSON/SQLite 覆盖：

- 初始 true 被排除、稳定→变化 bracket 正确、多个 target/window 分组；
- 相位落在 interval 内的计算、30/60 可区分与不可区分样本；
- holdout、outlier、unknown clock、safe-offset 可识别/不可识别；
- 输入顺序稳定、JSON byte-stable、指纹变化；
- malformed schema/time/sequence 的 fail-closed 行为；
- 当前 committed NB 数据产生预期的 `NO_PRODUCTION_CHANGE`，且报告明确两模型无法区分。

lane 内只跑 analyzer tests、对现有数据的离线命令和 `git diff --check`。不跑 workspace/frontend/archive/
soak/security scan，不访问网络。不要为了 analyzer 引入依赖或分析平台。

### A6. 未来采样不是本任务

若以后用户另行授权，推荐的独立采样约束是：只对 Rutgers Open Sections endpoint 做匿名只读 GET；term
92026 的 NB/NK/CM；不登录、不写入、不绕限流；一次仅采一个 target，总速率不快于每 12 秒一请求；
13s±1s 节奏，至少 6h、最多 24h，必须覆盖 17:00–18:00 ET。6h 已满足 GO 门可早停，24h 仍不足即停止。

这些约束只是 future plan。当前实现代理没有在线授权，不编写/运行隐藏采集，不使用当前时间去碰 Rutgers。

### A7. 回报格式

一次返回：Outcome；commits；新增 files；CLI contract；当前数据 coverage/30s/60s/safe-offset 精确结果；
tests；为何是 `NO_PRODUCTION_CHANGE / DATA_REQUIRED`；未来所缺 evidence；Git status；从 orchestration anchor
与 `553371f` 的 review range。不要实现或承诺 GridAnchor/RebuildProfile。

## B. 实现任务提示（请用户从下一行开始原样复制）

ultracode: 请使用外部实现工具的官方 dynamic workflow，只在
`Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\parallel-wave-1\stage5-s3-evidence` 的
`codex/parallel-wave1-stage5-s3-evidence` 分支上完成 `STAGE-5/v1-parallel-evidence`。该 worktree 从 Codex
的 orchestration-only anchor 建立，产品代码父基线是
`553371f8fa449b8c7cb9a88b5f32e179cb1e57c5`。不要写主 checkout。

先核对 branch/head/status，然后完整阅读 `docs/orchestration/PARALLEL-WAVE-1.md` 与
`docs/orchestration/tasks/STAGE-5.md` 的 A 部分。使用 dynamic workflow 自主安排最小只读调查、analyzer/
test 实现、对抗 fixture 和报告复核；一个 writer 串行写本 worktree。不得修改 `docs/orchestration/**`，
不得 reset/rebase/merge/push/tag/release。

本任务是 evidence-only。只新增 `tools/s3/**` 和 `docs/evidence/S3-REBUILD-PROFILE.{md,json}`；用 Node 24
标准能力读取 committed NDJSON，并支持显式传入的 readonly/immutable
`open_batch_observations` SQLite。排除每 target 初始 LKG，按稳定观测→首次变化形成区间删失 bracket；拟合
30/60 秒 phase、coverage/residual/holdout 和 safe-offset 可识别性，输出带输入 SHA-256 的 byte-stable JSON
与人类报告。不能用 timestamp%60 直方图代替 bracket 模型。

当前已知只有 92026/NB 一个夜间主窗口，30s/60s 都约解释 65/67，不能区分。你必须让工具独立复算和报告
真实结果，但除非 A4 每一项在现有 committed 数据上真实成立，否则结论固定为
`NO_PRODUCTION_CHANGE / DATA_REQUIRED`。不要改 scheduler、policy/default、runtime、storage、schema 或
packaging，不实现 GridAnchor/RebuildProfile，不读取/提交 `.cache` 私有数据库。

严格离线：不访问 Rutgers、不联网采样。用 synthetic NDJSON/SQLite 覆盖 interval、两模型可区分/同分、
holdout、unknown clock、safe-offset、determinism 和 malformed inputs；再对 committed NB 数据运行一次。只跑
analyzer tests和 diff-check，不跑全仓重门。最后按 A7 一次性回报；零 production change 是本 lane 的正确
成功结果。
