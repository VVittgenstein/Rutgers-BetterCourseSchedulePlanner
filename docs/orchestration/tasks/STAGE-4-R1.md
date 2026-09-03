# Stage 4 R1：移除生产 raw SQLite 测试后门

状态：**ACCEPTED — 已串行集成并通过组合门**
Prompt 版本：`STAGE-4-R1/v1`
基线：`codex/parallel-wave1-stage4-s4@283a8feeee734abf61af564457d28f1b08e7102f`

## A. 修复任务包

`prepare_cached` 生产改动、事务回滚和 61 项 focused tests 均通过；唯一 blocker 是
`OperationalStorage::raw_connection_for_tests()` 以正常 `pub` API 编进生产库。`#[doc(hidden)]` 只隐藏文档，
任何依赖 crate 仍可执行任意 SQL/PRAGMA，绕过 storage 写入不变量。

修复必须：

- 从正常 production API/构建中彻底移除 raw writable `Connection` accessor；
- 保留 prepare-count 判别力：旧逐行 execute 为 `2N`、普通 prepare-per-commit 为 2、正确 cached 为 1；
- 优先把 authorizer/cache probe 移入 `open.rs` 的私有 `#[cfg(test)]` unit seam，或使用同等严格且 production
  build 不存在的 test-only 机制；
- integration rollback 测试和 `prepare_cached` 实现不回退；不改 SQL、算法、依赖版本或性能范围；
- 新增静态/编译证据，证明正常依赖方无法取得 raw connection。

只跑 operational-storage focused tests、必要 API/static check 和 diff-check。不要处理 trigger 生命周期、
BTreeMap 测试耦合或其他 deferred，不跑全仓门。最后返回 commits、测试、判别证据、API absence 和
`283a8fe..<head>`。

## B. 实现任务提示（请用户从下一行开始原样复制）

ultracode: 请继续使用外部实现工具提供的最小充分 dynamic workflow，只在
`Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\parallel-wave-1\stage4-s4` 的
`codex/parallel-wave1-stage4-s4@283a8feeee734abf61af564457d28f1b08e7102f` 上完成 `STAGE-4-R1/v1`。
不要写主 checkout，不 reset/rebase/merge，不修改 `docs/orchestration/**`。

阅读主 checkout 的 `docs/orchestration/tasks/STAGE-4-R1.md` A 部分。唯一修复是删除 production 编译的
`raw_connection_for_tests()` 可写数据库 API，同时用真正 test-only 的私有 seam 保留三向 prepare-count
判别和 rollback 证明。不要改 `prepare_cached` 算法、SQL、依赖或其他存储代码。只跑 focused tests 和
diff-check，完成后按 A 部分一次性回报。
