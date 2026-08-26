# Stage 3：P2 公网重新上线加固

状态：**DELIVERED — CODEX `CHANGES_REQUIRED`；见 `STAGE-3-R1.md`**
Prompt 版本：`STAGE-3/v1-parallel`
Orchestrator：Codex
实现者：Claude
产品负责人/转发者：用户
产品代码父基线：`553371f8fa449b8c7cb9a88b5f32e179cb1e57c5`

## A. 任务包

### A1. 用户结果与本 lane 的完成含义

本 Stage 只实现已经批准的 P2/H1–H9 公网加固。目标是让最终公网候选具备确定的连接/内存边界，避免
同版本升级或 Caddy reload 无意义地切断监控，能从崩溃循环恢复，正确处理 Origin 主机名大小写，并有
真实 Linux systemd+Caddy+浏览器 10 分钟验证入口。

当前 Windows 主机没有 WSL、Docker、bash 或 Caddy。因此本 lane 能完成仓内实现、focused tests 和可运行
harness，但不能独立关闭 Linux/真实 Caddy 外部门。Claude 的诚实终态应是
`IMPLEMENTED / PENDING_LINUX_EVIDENCE`，不是 P2 accepted、deployed 或 released。

### A2. Worktree、分支与边界

- worktree：`Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\parallel-wave-1\stage3-p2`；
- branch：`codex/parallel-wave1-stage3-p2`；
- 从包含本任务包的 orchestration-only anchor 建立，产品代码父基线为 `553371f`；
- 只写本 worktree；一个 writer/integrator 串行提交；不修改 `docs/orchestration/**`；
- 不 reset/rebase/merge/push/tag/release，不部署，不改真实 DNS/UFW/SSH/fwupd，不访问 Rutgers；
- 不夹带 S4/S3，不重写已验收的 S1/L1、Stage 2 reconnect/presence/notification 或 session/P1 系统。

主要所有权：

- `crates/bcsp-application` 中公网可选的 outbound/WS seam；
- `crates/bcsp-public-runtime` 的 config、host、session、资源所有者和测试；
- `deploy/public/**`、public ops/runbook/disposable-host；
- public soak/CI 合同和必要的 P2 design/checklist 写回。

已知与 Stage 2 R1 的交叉点是 `.github/workflows/public-ops.yml`。保留 R1 增加的 frontend Notification
policy 执行语义；不要覆盖整份 workflow。不得把 shared `WebSocketExtension` 粗暴全局改型并迫使
`bcsp-local-runtime/src/presence.rs` 跟随迁移；优先增加 public-only bounded path 或等价隔离接缝。

### A3. H1–H3：FD、Caddy reload 与同版本升级

- H1：systemd service 设置 `LimitNOFILE=65536`，验证脚本能 read back；
- H2：Caddy `reverse_proxy` 设置 `stream_close_delay 4h`；runbook 明确 reload 风险和低峰建议；测试必须用
  同一条真实连接穿过 reload，不能让 Stage 2 自动重连替换 socket 后仍算通过；
- H3：同一 release id 的 upgrade 是真正 no-op/liveness check，不 reload/restart，`MainPID` 不变；不同
  release 的既有 ordered upgrade 行为保持。

### A4. H4：公网 WS 容量与出站背压

以下数值和语义在本任务冻结：

- global active public WS cap：`1024`；
- per-client active public WS 默认 cap：`64`，允许环境配置 `1..=1024`；
- 既有 per-session cap：`4`，语义与状态码保持；
- client key 必须复用当前 issuance limiter 的同一规范化：IPv4 全址、IPv6 `/64`、v4-mapped 归 IPv4、
  缺失转发头为 `direct`；不新增第二套近似算法；
- per-socket queued outbound payload budget：`256 KiB`；
- global queued outbound payload budget：`64 MiB`；
- 每次 socket write timeout：`5s`。

资源合同：

- global/per-client permit 在 session `reserve_ws` 之前或同一准入步骤取得，计入尚未完成 upgrade 的连接；
- 所有 permit 与 byte reservation 都由 RAII 贯穿 upgrade、pump、正常关闭、错误和取消路径，不泄漏；
- Text/Ping/Pong/Close 等实际 socket writes 均受 timeout；慢消费者只断自己的连接；
- byte budget 达限不得退化为另一个可重复增长的无界 control/close queue；
- 全局 cap 拒绝返回 503；per-client 容量拒绝也返回 503，并用独立 metric/reason 区分；既有 per-session
  403 不改；429 继续只表示 issuance rate limit；
- 达到 1024 global cap 时仍能签发新 session，直接钉死 `1024 < 4096` 和 3072 registry headroom；
- 64 KiB inbound frame/message 帽保持，不把“256 frames”旧示例复活为内存合同；
- 只把 per-client cap 做环境可调，拒绝为所有常量建立配置平台。

具体 channel/sink 类型、模块划分和 metric 命名由 Claude 按现有架构决定，但必须能用 deterministic tests
证明 byte accounting、释放、单次 overload 处理和 timeout。

### A5. H5–H7：保留 session、恢复策略与 Origin

- H5 已有 `active_ws_count`、inactive-only eviction、all-leased 503、validate 原子换证和 issuance 限流；
  只补 H4 组合回归与 runbook，不重写 registry/session；
- H6：systemd 使用 `StartLimitIntervalSec=0` 与 `MemoryHigh=700M`，runbook 写清监控/恢复；disposable-host
  能以六次 `SIGKILL` 证明仍恢复；
- H7：外部 origin authority 的 DNS host 比较大小写不敏感。应在配置规范化边界保存 canonical authority，
  不放宽 scheme、port、path 或其他 Origin/Host 校验；用大写配置/请求组合判别。

### A6. H8–H9：仓内上线合同与真实证据门

H8 本 lane 只完成 repository-owned preflight、env schema/template、runbook 和 verify 脚本。不得猜真实域名，
不得操作 DNS、80/443、防火墙、root SSH、fwupd 或远端主机。这些仍需用户另行授权。

H9 要实现可在干净 Linux disposable runner 上执行的候选门：

- 从同一 final commit 构建/验证 Linux archive；
- 真实 binary + systemd + Caddy + real browser，而不是内存替身；
- 同一条 WS 连满 600 秒，无 close/error/替换连接；
- 收到并 ACK 约 50 个有效 application PING；
- 中途一次真实 `caddy reload` 后 connection identity 与 service `MainPID` 不变；
- 采集连接 metric 和每 30 秒 `MemoryCurrent`；所有样本低于 700 MiB，末三样本相对首三样本增长不超过
  32 MiB；
- public START→服务端实际 watch→fixture event→浏览器观察的完整 composition 仍成立。

本机不能运行该门。Claude 应实现脚本/CI 手动入口、做 parser/static/focused 验证，并明确列出在 Linux
runner 上的精确待执行命令。不得把 54 秒测试、mock proxy 或自动重连冒充 10 分钟 H9。

### A7. 判别测试和 lane 内验证

至少覆盖：

1. global/per-client/per-session 三层各自达到边界和释放后再准入；
2. upgrade 前、upgrade 失败、pump error/cancel/normal close 均释放 permit；
3. per-socket/global byte budget 达限只牺牲慢连接，恢复预算，且无第二无界队列；
4. held writer 超过 5 秒被断开；正常 alert/PING 不受影响；
5. global 1024 全满时 registry 仍签票；H5 eviction/renew 回归保持；
6. exact client-key 规范化与 per-client override/拒绝 metric；
7. Origin host case 正例及 scheme/port 负例；
8. same-release `MainPID` 不变、Caddy directive、systemd 常量、H9 脚本的静态/单元合同。

并行 lane 只运行受影响 crate/tests、Node/shell/PowerShell parser/static self-tests 和
`git diff --check`。不要运行 workspace、完整 frontend verify、archive build、10 分钟 soak或全仓安全扫描；
这些由 Codex 串行集成后执行。若必须下载正常 Cargo 依赖可按仓库锁定流程进行，但不得安装外部系统工具
或联网部署。

### A8. 明确延期

- 真实 H8 主机操作和 H9 Linux evidence；
- Redis、多实例协调、CDN/trusted-proxy 新模型、autoscaling；
- session 持久化、新 WS 协议、Caddy 第三方 rate-limit module；
- prelaunch 设计中与 H1–H9 无关的 112–120 行低优先杂项；
- S4/S3、N1–N3、任何 UI 美化或本地 desired/presence 重构。

### A9. 回报格式

一次返回：Outcome；commits；H1–H9 对照表；H4 资源所有权/失败路径；changed files；focused tests 精确
结果；判别力；`PENDING_LINUX_EVIDENCE` 清单和待执行命令；known limitations；Git status；从
orchestration anchor 与 `553371f` 的 review range。不要宣布 P2 accepted/deployed/released。

## B. Claude Prompt（请用户从下一行开始原样复制）

ultracode: 请使用 Claude Code 官方 dynamic workflow，只在
`Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\parallel-wave-1\stage3-p2` 的
`codex/parallel-wave1-stage3-p2` 分支上完成 `STAGE-3/v1-parallel`。该 worktree 从 Codex 的
orchestration-only anchor 建立，产品代码父基线是
`553371f8fa449b8c7cb9a88b5f32e179cb1e57c5`。不要写主 checkout。

先核对 branch/head/status，再完整阅读：

1. `docs/orchestration/PARALLEL-WAVE-1.md`；
2. `docs/orchestration/tasks/STAGE-3.md` 的 A 部分；
3. `docs/design/2026-08-20-public-prelaunch-hardening.md`；
4. `docs/design/2026-08-20-review-package-public.md`；
5. `docs/design/2026-08-20-pr-acceptance-checklist.md` 的 P2/H4/S2-D1 部分。

Codex 钉死产品结果、H1–H9、H4 数值/资源语义、外部门和 lane 所有权；你使用 dynamic workflow 自主安排
内部 phase、只读调查 agents、实现结构、focused tests 和逻辑 commits。一个 writer/integrator 串行写本
worktree。不得 reset/rebase/merge/push/tag/release，不修改 `docs/orchestration/**`，不访问 Rutgers，不部署
或改 DNS/UFW/SSH/fwupd。

完成 H1 `LimitNOFILE=65536`；H2 `stream_close_delay 4h` 与同一 socket 穿 reload；H3 same-release 真 no-op/
PID 不变；H4 global 1024、per-client 默认64可调1..1024、保留 per-session4、复用 exact client key、
per-socket 256KiB/global 64MiB outbound payload、所有 write 5s timeout、permit/byte RAII、拒绝 metric；
H5 只补回归不重写；H6 `StartLimitIntervalSec=0`/`MemoryHigh=700M`；H7 authority host 大小写规范化且
不放宽其他 Origin；H8 只做仓内 preflight/runbook；H9 实现真实 Linux systemd+Caddy+browser 600s harness。

避免把 shared WebSocket extension 全局改型：public bounded outbound 应与 local presence 隔离。保留 R1 在
`.github/workflows/public-ops.yml` 的 frontend Notification policy 语义，发生文本冲突只在回报中提示 Codex。
不要夹带 S4/S3、session 重写、Redis/CDN/多实例、第三方 Caddy rate-limit 或其他防御性优化。

按 A7 写能打挂旧实现/删掉资源门后的 deterministic tests；并行期间只跑 focused/static/parser/diff-check，
不跑 workspace、完整 frontend verify、archive、全仓安全扫描或 soak。本机缺 Linux 环境时不要安装或伪造，
把结果明确标为 `IMPLEMENTED / PENDING_LINUX_EVIDENCE`，列出 final integration head 在 Linux runner 必须执行
的精确命令。最后按 A9 一次性回报。
