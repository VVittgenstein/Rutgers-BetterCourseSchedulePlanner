# Stage 3 R1：真正的出站预算与不撒谎的 H8/H9 门

状态：**READY — 返回原 Stage 3 worktree 集中窄修**
Prompt 版本：`STAGE-3-R1/v1`
基线：`codex/parallel-wave1-stage3-p2@37176d2c487ecd0366f8d7a50e85cad686f77b01`

## A. 修复任务包

### A1. Codex 结论

Stage 3 的 global/per-client permit、session 组合、Origin/Host、systemd/Caddy 基础配置和大部分 ops 实现
成立，但当前不能集成。只修以下五个根因，不重写 P2：

1. public pump 把 `mpsc::UnboundedSender<String>` 交给 extension，String 已进入无界 channel 后才做
   256 KiB/64 MiB reservation；报告中“入队即预留”与真实代码相反；
2. operator Caddy preflight 只 grep 原始文本，注释或别的 site/block 也能让 H2 PASS；
3. `sshd -T` 未绑定 root connection tuple，conditional `Match` 可使 H8 对错误的 effective policy PASS；
4. H9 sampler 静默丢失败读取、只需极少样本；`journalctl` 失败也被当作“没有拒绝 ACK”；
5. `composition=SKIPPED` 时仍打印并由 workflow 接受 `P2_H9_PUBLIC_SOAK_PASS`。

Codex Security diff scan `9391c9bd-32ce-483c-9fde-d454e11c2826` 对 16/16 source inventory 完成审查：
H2/H8 两项为 high-confidence low-severity findings；unaccounted sender 因实际远程 DoS 幅度未量化而在
security 报告中 deferred，但它仍直接违反本 Stage 的资源合同，所以是产品/发布 blocker。

### A2. 出站预算必须在 retained queue 之前生效

- public 路径的每个 outbound payload 在进入任何 retained channel/queue 之前，必须同时取得 per-socket 与
  global reservation；失败只断该连接；
- 不得保留一个先存完整 `String`、后由 pump `try_recv` 才记账的无界中转 channel；
- local watch/presence 的既有行为不变。可以引入 target-neutral sender abstraction、public-only extension
  seam 或等价设计；内部类型由 Claude 决定；
- 若共享 sender API 必须机械影响 `bcsp-local-runtime/presence.rs`，不要重写 presence 语义，并在回报中
  精确列出，Codex 会在已集成 Stage 2 R1 的 head 上手工合并；
- permit、reservation、write timeout、三类 disconnect metric 和 healthy-socket isolation 保持；
- 判别测试必须走真实 public `SharedWatchSocket` producer，而不只用可随意 burst 的 fake extension；held
  writer/并发 dispatch 下，实际 retained payload 与 gauge 都不得先越预算再被发现。

单帧构造期间的临时 String 不是队列；本轮不建立全进程 allocator 平台。目标只是把批准的 queued-payload
合同做真。

### A3. H2/H8 preflight 必须验证实际生效配置

- H2：对 operator-selected Caddyfile 检查 active public `reverse_proxy` 的 adapted/结构化语义；仅在注释、
  其他 site、其他 block 出现 `stream_close_delay 4h` 必须失败；
- H8：不得从无 `-C` 的全局 `sshd -T` 输出宣称 root login 已安全。采用明确 root connection tuple，或在
  conditional policy 无法可靠评价时 fail closed。同时评价该 tuple 实际相关的 password/root controls；
- 新增精确正反 fixture。不要顺手做完整 SSH/Caddy 管理平台。

### A4. H9 evidence 必须完整且命名诚实

- 600s/30s sampler 对读取失败 fail closed，并证明样本覆盖整个窗口、允许合理调度抖动但不能只凭 6 个/
  1 个样本 PASS；缺中段、缺尾段、命令失败都必须失败；
- `journalctl`/日志读取本身失败必须失败；日志范围绑定本次 service/soak，不能用旧日志；
- 浏览器的 `socket.send(ACK)` 不能单独冒充服务器接受，保留或改进可验证的 server-side acceptance 证据；
- core 600s soak 可以有明确 `...CORE_PASS` 标记；只有完整 composition 成功时才允许
  `P2_H9_PUBLIC_SOAK_PASS`。workflow/job/summary 不得把 `composition=SKIPPED` 称为完整 H9；
- 当前没有 Rutgers 网络授权，所以 focused/CI self-test 只证明 core/harness；完整 composition 继续
  `PENDING_LINUX_EVIDENCE / EXTERNAL_AUTHORIZATION`，不要联网运行。

### A5. 明确不修

- manual workflow 能选择任意 SHA、root harness source 已信任 candidate：二者都要求已有 workflow/operator
  权限，当前为明确 trust prerequisite，不扩建签名/ancestry 平台；
- bounded pump fairness、单独 Ping deadline 判别器、UFW app-profile、美化/重命名、真实 H8 主机动作；
- S4/S3、Stage 2、session/P1 重写、Redis/CDN/多实例。

### A6. 验证与回报

只跑受影响 Rust、ops/static/self-tests、parser 和 diff-check；不跑 workspace/full frontend/archive/600s
soak/security scan。每个 blocker 要有能打挂 `37176d2` 的正反例。一次回报 commits、机制、tests、
negative proof、冲突提示、Git status 和 `37176d2..<head>`。

## B. Claude Prompt（请用户从下一行开始原样复制）

ultracode: 请继续使用 Claude Code 官方 dynamic workflow，只在
`Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\parallel-wave-1\stage3-p2` 的
`codex/parallel-wave1-stage3-p2@37176d2c487ecd0366f8d7a50e85cad686f77b01` 上完成
`STAGE-3-R1/v1`。不要写主 checkout，不 reset/rebase/merge，不修改 `docs/orchestration/**`。

先完整阅读主 checkout 的
`Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\orchestration\tasks\STAGE-3-R1.md` A 部分。Codex 已完成
全 diff 审查，只修 A1 的五个根因：public outbound 必须在任何 retained queue 前取得 256KiB/64MiB 预算，
不能再把完整 String 先放进 UnboundedSender；operator Caddyfile 必须按 active public proxy 语义验证；SSH
必须验证实际 root conditional policy 或 fail closed；H9 样本/日志读取必须覆盖完整窗口并 fail closed；
composition 跳过时只能报 core/pending，绝不能打印完整 H9 PASS。

内部 sender/seam、Caddy/SSH 解析方式和测试组织由你自主决定。若 shared sender API 机械触及 local
presence，只做兼容适配并明确回报，不能覆盖 Codex 已在另一条线上验收的 Exiting/late-HELLO 语义。严格
遵守 A5，不修任意 SHA/candidate trust、fairness 或其他 deferred。只跑 A6 focused tests，不跑全仓重门、
archive、soak，不访问 Rutgers。完成后按 A6 一次性回报；P2 仍由 Codex 验收。
