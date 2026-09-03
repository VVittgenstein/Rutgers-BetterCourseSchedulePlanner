# Stage 4：S4 SQLite `prepare_cached` 微优化

状态：**DELIVERED — CODEX `CHANGES_REQUIRED`；见 `STAGE-4-R1.md`**
Prompt 版本：`STAGE-4/v1-parallel`
Orchestrator：Codex
实现者：实现代理
产品代码父基线：`553371f8fa449b8c7cb9a88b5f32e179cb1e57c5`

## A. 任务包

### A1. 唯一目标

成功 Open pull 提交当前镜像时，代码先删除一个 target 的 `open_section_current`，再对全部 Catalog section
逐行执行同一条 INSERT。当前每行调用 `transaction.execute`，等价于每次重新 prepare；历史 11,933 行场景
约从 105 ms 降至 28.5 ms 的证据说明值得修，但该毫秒值只作方向性证据。

本 Stage 只把这一条 INSERT 改为一次 `prepare_cached` 后重复 execute。全量 `DELETE + INSERT` 算法、事务、
排序、数据、事件和错误行为全部不变。

### A2. Worktree、允许文件和依赖

- worktree：`Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\parallel-wave-1\stage4-s4`；
- branch：`codex/parallel-wave1-stage4-s4`；
- 产品代码父基线 `553371f`；只写本 worktree，不修改 `docs/orchestration/**`；
- 预期生产热点：`crates/bcsp-operational-storage/src/open.rs` 的
  `finish_open_pull_success_transaction`；普通 serving 与 candidate publish 已共用它；
- `rusqlite 0.40.1` 当前未启用 `cache`。只在 `bcsp-operational-storage` 自己的依赖声明窄加
  `features = ["cache"]`；不要打开 workspace 全局默认 features；
- 允许相应最小 `Cargo.lock` 更新，以及为稳定测试在该 crate dev dependency 窄开 `hooks`；
- 预计只改 operational-storage 的 `open.rs`、crate manifest、`Cargo.lock` 和相关 `open_storage` 测试。

不 reset/rebase/merge/push/tag/release，不写主 checkout。若 Cargo 需要取得该已声明 feature 的锁定 registry
依赖，这是正常构建步骤；不安装额外工具或引入 benchmark 框架。

### A3. 行为与事务不变量

- 保留 target 级 DELETE 和所有行的完整重建，不改增量/upsert/批量拼 SQL；
- statement 在循环外 `prepare_cached`，循环内参数、七个绑定值、BTreeMap 顺序和 `?` 传播不变；
- statement 生命周期应在后续 transaction 操作前结束并归还连接缓存；不新增 global/static cache owner；
- DELETE、全部 INSERT、observation、attempt、batch/counter 更新仍在同一 immediate transaction；
- prepare 失败、第一行后任一插入失败、后续状态更新失败都必须整体回滚；
- unsafe/gate hold/early return 不触碰 current mirror；
- 不改变 LKG、attempt/batch identity、observation sequence、hash、changed count、projection、event fanout 或
  `OpenCommitOutcome`；
- 普通成功与 candidate publish 两条入口继续共用一处实现。

### A4. 判别测试

优先使用不依赖墙钟的机制测试证明同一 SQL 不再按行编译。已确认可用的方案是 rusqlite test-only hooks 的
SQLite authorizer：它只在 statement prepare 时观察 `open_section_current` INSERT。

建议在清空 statement cache 后，对同一连接连续完成两次多行成功提交，并断言 prepare count 为 1：旧
`transaction.execute` 为 `2N`，只把普通 `prepare` 移到循环外为 2，正确 `prepare_cached` 为 1。若实现代理
找到同等稳定、能同时区分这三种实现且不靠毫秒的现有接缝，可采用等价方案并说明。

同时强化既有 rollback 测试：让第一行先成功、第二个 section 的 trigger 再失败，断言事务回到完整旧 LKG，
attempt 仍是既定失败状态，observation/batch pointer/部分新行均不泄漏。继续复用已有 ordinary、candidate、
gate-hold 和 projection 回归，不复制整套场景。

### A5. 性能证据与验证纪律

- CI 硬门是语义等价和 prepare 次数，不设置 28/50/105 ms 绝对阈值；
- 可用同一 11,933-row release fixture 对 base/head warm-up 后多轮运行，报告 median/range，失败不由绝对时间
  决定；不要引入 Criterion 或长期 benchmark 基础设施；
- lane 内只跑 `bcsp-operational-storage` 的 focused unit/integration tests、依赖/许可的窄检查和
  `git diff --check`；不跑 workspace、完整 frontend、archive、soak 或全仓安全扫描；
- 若首次依赖解析失败，保存精确错误并处理真实 manifest/lock 原因，不用反复刷命令。

### A6. 明确不做

- 不做 incremental mirror、upsert、schema/migration/index 或 cache-capacity 调优；
- 不缓存其他 SQL，不清理存储层，不重构 transaction API；
- 不改轮询周期、S3 scheduler/freshness、runtime、watch、frontend、deployment 或 packaging；
- 不因理论微优化扩展范围。

### A7. 回报格式

一次返回：Outcome；commits；changed files；旧/新 prepare 机制；事务等价证据；focused tests 精确结果；
信息性性能数据（若运行）；依赖/lock 变化；known limitations；Git status；从 orchestration anchor 与
`553371f` 的 review range。不要宣布整个组合 Stage accepted。

## B. 实现任务提示（请用户从下一行开始原样复制）

ultracode: 请使用外部实现工具的官方 dynamic workflow，只在
`Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\parallel-wave-1\stage4-s4` 的
`codex/parallel-wave1-stage4-s4` 分支上完成 `STAGE-4/v1-parallel`。该 worktree 从 Codex 的
orchestration-only anchor 建立，产品代码父基线是
`553371f8fa449b8c7cb9a88b5f32e179cb1e57c5`。不要写主 checkout。

先核对 branch/head/status，然后完整阅读 `docs/orchestration/PARALLEL-WAVE-1.md` 与
`docs/orchestration/tasks/STAGE-4.md` 的 A 部分。Codex 钉死唯一用户结果和事务不变量；你用 dynamic
workflow 自主安排最小调查、实现、focused tests、独立判别复核和逻辑 commit。一个 writer 串行写本
worktree，不修改 `docs/orchestration/**`，不 reset/rebase/merge/push/tag/release。

只修改 `finish_open_pull_success_transaction` 的重复 INSERT：保持 DELETE + 全量 INSERT + 其余状态更新仍在
同一 transaction，把同一 INSERT 在循环外 `prepare_cached` 并循环 execute。只在
`bcsp-operational-storage` 窄启 rusqlite `cache` feature，允许必要的最小 lock 更新；不要打开全 workspace
默认 features。不得改增量写、upsert、schema/migration/index、其他 SQL、scheduler、runtime 或部署。

用不依赖毫秒的测试证明旧实现每行 prepare 会失败、普通 prepare-per-commit 也不够、cached statement 可
跨连续提交复用；优先考虑 test-only SQLite authorizer，等价稳定方案也可。再证明第一行成功、第二行失败时
整个 transaction 回滚且旧 LKG/attempt/observation/batch/current mirror 合同不变。性能运行只报告
median/range，不设绝对 CI 时间门，不引入 benchmark 平台。

并行期间只跑 operational-storage focused tests、窄依赖/许可检查和 diff-check；不跑 workspace、frontend、
archive、soak 或全仓扫描。完成后按 A7 一次性回报，并提示 `Cargo.lock` 需要 Codex 在 P2 组合 head 上重新
解析，不能机械选冲突一侧。
