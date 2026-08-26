# Parallel Wave 1：四个隔离实现 lane 与串行集成合同

状态：**FIRST DELIVERY REVIEWED — Stage 2 R1 accepted；Stage 3/4/5 集中窄修**
日期：2026-08-25
Orchestrator：Codex
实现者：四个相互隔离的 Claude 会话
产品代码父基线：`553371f8fa449b8c7cb9a88b5f32e179cb1e57c5`

## 1. 为什么并行、什么不并行

用户已批准把余下工作从“完成一个 Stage、审一次、再开始下一个”改为：

```text
四个 Claude 在四个 worktree 并行实现
                 ↓
Codex 按冻结顺序串行集成
                 ↓
只在组合 head 跑一次重门并做一次集中审查
```

并行的是读取、实现、focused tests 和 lane 内提交；不并行的是共享工作树写入、全量测试、最终集成、
验收和发布。产品依赖顺序仍是：

```text
Stage 2 R1 → Stage 3 / P2 → Stage 4 / S4 → Stage 5 / S3 evidence
```

这不是允许后续 Stage 覆盖前一 Stage 的合同。每个 lane 都从同一个 orchestration-only anchor 出发，
最终由 Codex 按上述顺序审查并回收到 `feat/s2-alert-delivery`。

## 2. Worktree 与分支

| Lane | Worktree | Branch | 任务包 |
|---|---|---|---|
| A | `.worktrees/parallel-wave-1/stage2-r1` | `codex/parallel-wave1-stage2-r1` | `tasks/STAGE-2-R1.md` |
| B | `.worktrees/parallel-wave-1/stage3-p2` | `codex/parallel-wave1-stage3-p2` | `tasks/STAGE-3.md` |
| C | `.worktrees/parallel-wave-1/stage4-s4` | `codex/parallel-wave1-stage4-s4` | `tasks/STAGE-4.md` |
| D | `.worktrees/parallel-wave-1/stage5-s3-evidence` | `codex/parallel-wave1-stage5-s3-evidence` | `tasks/STAGE-5.md` |

所有路径都相对于仓库根目录
`Z:\Project\Rutgers-BetterCourseSchedulePlanner`。Claude 必须只写自己被分配的 worktree，不写主 checkout，
不创建更多产品分支，不 merge/rebase/push/tag/release。

## 3. 写入所有权

### Lane A — Stage 2 R1

只关闭四个已裁定 blocker：音频真值、Exiting 后 late HELLO、生产通知开关、Notification policy CI/自检。
主要所有权是 Stage 2 相应 frontend、local presence 和 CI policy 文件。

### Lane B — Stage 3 / P2

拥有公网资源准入、public-only outbound 背压、public runtime/config/session、`deploy/public/**`、public ops
验证与 Linux/soak harness。它不得把 shared WebSocket extension 全局改型并迫使 local presence 迁移；应保留
本地既有 unbounded/internal 路径，新增公网可选的 bounded seam 或等价隔离实现。

### Lane C — Stage 4 / S4

只拥有 operational storage 的一个 INSERT 热点、该 crate 的窄依赖 feature、对应测试和锁文件变化。不碰
调度、runtime、部署或其他 SQL。

### Lane D — Stage 5 / S3 evidence

只新增离线 analyzer、测试和 evidence 报告。现有数据不足以 GO，因此本 wave 不改生产 scheduler、默认值、
policy、runtime 或 storage schema；不访问 Rutgers。

## 4. 已知交叉点

1. Lane A 与 B 都可能修改 `.github/workflows/public-ops.yml`。A 先集成；B 的提交在组合 head 上重放，保留
   A 的 frontend Notification policy 步骤，并追加 P2 所需 Linux/public 门。
2. Lane A 的 `crates/bcsp-local-runtime/src/presence.rs` 与 B 的 shared WebSocket 工作存在潜在语义交叉。
   B 必须使用 public-only bounded path，避免修改 local presence API；若仍发生冲突，由 Codex 以 A 的
   Exiting/HELLO 语义为准手工集成。
3. Lane B 与 C 可能同时更新 `Cargo.lock`。集成 C 时在组合 head 重新解析最小 lock diff，不能机械选择
   ours/theirs。
4. Lane D 的 `tools/s3/**` 与 `docs/evidence/**` 应与其他 lane 零产品代码重叠。

Claude 不通过修改别的 lane 文件来“提前解决冲突”。不确定时保留自己 lane 的最小实现并在回报中列出
integration note。

## 5. 测试资源纪律

并行期间每个 Claude 只跑自己根因的 focused tests、静态检查和 `git diff --check`。以下重门不得在多个
worktree 同时运行：

- `cargo test --workspace`；
- 完整 `frontend npm run verify`；
- Windows/Linux archive build；
- 全仓 architecture/security 扫描；
- real-browser/Caddy soak。

原因是本机已经观测到并发 Cargo 与 Vitest 导致 worker 启动超时，而且多个 runtime 测试会争抢端口、
临时目录和共享缓存。Codex 串行集成后只在最终组合 head 运行一次适用的完整门。

Lane B 在当前 Windows 主机无法完成真实 Linux systemd+Caddy 10 分钟 soak；它负责实现可运行 harness 和
仓内合同，回报必须写 `PENDING_LINUX_EVIDENCE`，不得用 mock 或较短等待冒充 H9。Lane D 不运行任何在线
采样。

## 6. 集成与验收

Codex 收齐四个 Claude 回报后：

1. 核对每个 worktree 的 branch、base、提交、dirty/untracked 状态和实际 diff；
2. 独立审查每个 lane 是否越权或遗漏主合同；
3. 按 A → B → C → D 的顺序把已通过的提交回收到主集成分支；
4. 只在组合 head 解决上述已知交叉点，不顺手重构；
5. 跑一次 workspace、frontend、architecture、依赖/ops 和 Stage 专项目；
6. 对公网 session/资源/发布边界做一次组合 diff 安全审查；
7. 普通 P2/P3 review finding 记录为 deferred，只有既定 blocker 才要求集中窄修；
8. 将 H9 与真实 H8 外部动作明确留作 Linux/部署授权门，不虚报 accepted/released。

Stage 5 的正常成功结果可以是 `NO_PRODUCTION_CHANGE / DATA_REQUIRED`。这不阻止安全功能代码集成，也不
授权日后自行采集 Rutgers 数据。

## 7. 停止条件

任何 Claude 只有在以下情况停止并向用户回报，而不是自行扩权：

- worktree/branch/base 不匹配；
- 发现不属于自己 lane 的未知修改且无法隔离；
- 必须改变冻结的产品行为、wire/schema 或资源常量才能继续；
- 需要真实部署、DNS、防火墙、SSH、联网采样或不可逆外部操作；
- focused test 揭示前一 Stage 的主合同确实不成立。

缺少 Linux/Caddy 环境、S3 数据不足、普通代码质量建议不是扩展范围的理由；分别按已定义的
`PENDING_LINUX_EVIDENCE`、`NO_PRODUCTION_CHANGE` 和 deferred 处理。

## 8. 首轮 Codex 裁定（2026-08-25）

| Lane | Head | 裁定 | 下一步 |
|---|---|---|---|
| A / Stage 2 R1 | `5af49d9` | `ACCEPTED`；已回收为主线 `6a35c74` | 无 |
| B / Stage 3 | `37176d2` | `CHANGES_REQUIRED` | `tasks/STAGE-3-R1.md` |
| C / Stage 4 | `283a8fe` | `CHANGES_REQUIRED` | `tasks/STAGE-4-R1.md` |
| D / Stage 5 | `9582728` | `CHANGES_REQUIRED` | `tasks/STAGE-5-R1.md` |

三条 repair 继续使用原 worktree，可并行；不得另起 full gate。普通 finding 已按 deferred 政策过滤，repair
只含资源合同、发布证据、数据库写入边界和 evidence GO 门的真实阻断。
