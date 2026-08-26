# Stage 3 R3：让 Caddy 与 ACK 证据只计算真实可达路径

状态：**ACCEPTED_WITH_DEFERRED_DEBT — 已串行集成；PENDING_LINUX_EVIDENCE**

Prompt 版本：`STAGE-3-R3/v1`

基线：`codex/parallel-wave1-stage3-p2@df79aa4fab0f0e29fdb17fc946a234b5e1a85734`

## A. 修复任务包

### A1. 已经通过、不得重写

Codex 已独立核对并复跑：`bcsp-application` 117、`bcsp-public-runtime` 52、preflight 无-jq 路径、
soak self-test 与 deploy contracts 均通过。以下 R1/R2 合同成立并冻结：

- public outbound 在任何 retained queue 前取得 per-socket/global 预算；
- actual `--admin-source` root tuple 的解析、`sshd -T -C`、UseDNS/listener/password controls 与 fail-closed；
- HeartbeatAck 只有在 known connection 上命中服务端已发出且未确认的 fresh sequence 才增加 accepted counter；
- H9 完整采样、invocation journal、CORE/full PASS 命名；
- H1–H7、容量数值、wire/session、Stage 2 行为。

SSH 主合同已 `fixed`。本轮不再设计 SSH，也没有 Vultr/真实主机；不得向用户索取地址或凭据，不连接任何
主机。单一 synthetic arbitrary-source control probe 的宽泛 PASS 措辞记为非阻断技术债，不得借此扩建通用
SSH policy analyzer。

### A2. 只修两个根因

#### 1. H2：Caddy active path 必须同时服从 route 与 handler 顺序

当前 jq 只会在 route 之间遇到终止响应时截断；进入一个 route 后却枚举全部 `.handle[]`。因此同一个
route 的 handler chain 为：

```text
static_response 200
reverse_proxy 127.0.0.1:8080 + stream_close_delay 4h
```

时，Caddy 实际在 `static_response` 停止，不会调用后面的 proxy，preflight 却把不可达 proxy 算作 active
并返回 OK。另一个同类归属错误是：两个 server 都监听 origin port，但监听地址不同；仅另一个接口上的
protected proxy 不能替真实公网接口的 static/unprotected route 证明 H2。

必须做到：

- 按 Caddy handler middleware chain 的实际顺序解释同一路由；已知会终止且不调用 next 的 handler 之后，
  任何 proxy 都不可作为证据；普通继续型 middleware 之前的 proxy 仍可达；
- 对未知 handler 的 next/terminal 语义 fail closed，不猜它会继续；保留现有 route-order、host、port、
  wildcard、negated matcher、handle_errors、alias upstream 与全 service proxy 义务；
- 同一 origin port 上有多个可能承接公网 host 的 server/listen address 时，必须绑定实际 DNS/local listener，
  或要求每个可能承接该 host 的 server 都满足合同；无法唯一归属时 fail closed。不得把不同 listen address
  的 server 扁平合并后以“其中一个有保护”通过；
- 支持仓库 `Caddyfile.example` 真实 adapted 形状；不建立通用 Caddy 解释器。

必须新增能打挂 `df79aa4` 的 fixture：

1. 同 route 内 unconditional `static_response` 在 protected proxy 之前 → FAIL；
2. 同 route 内普通 continuing middleware 在 protected proxy 之前 → PASS；
3. 同 port 的另一 listen address 有 protected proxy，而公网可能到达的 listener 只有 static/unprotected →
   FAIL；所有可能 listener 都正确保护的 control → PASS。

本机没有 jq 是已知环境限制。fixture 必须真实调用 production jq 程序的既有 Linux CI 路径；本地可继续
明确 SKIP，但不得用另写的镜像算法冒充 jq 已执行。Codex 将静态审查 jq，并把首次 Linux 实跑保留在既定
CI/external evidence 门。

#### 2. H9：聚合 accepted counter 必须证明只属于这一条 soak socket

accepted counter 的服务端计数点正确，但它是全进程聚合。当前 connection samples 只要求 gauge `>=1`，
甚至允许 `[1,1,2,1]`；因此目标 socket 的 ACK 全被忽略时，另一条 socket 可以贡献相同 ACK 数，聚合
delta、browser tally 与现有 analyzer 仍全部通过。

在不记录身份、不修改 wire 的前提下，建立 soak-exclusive 证据。推荐的最小合同是：

- browser 启动前 active public watch connection gauge 必须为 0；
- 暴露匿名、单调、process-lifetime 的 public watch connection admissions counter；soak 前后 delta 必须
  精确为 1；
- judged window 的每个 active connection sample 必须精确为 1，不再是 `>=1`；
- accepted ACK delta 继续与本次 browser report 双向核对，并保持现有一帧在途容差；
- 任何 gauge/counter 读取失败、倒退、额外 admission/reconnect 或第二条并发 socket 都 FAIL；
- 不能加入 session/nonce/section/connection id 标签，不能让 metrics 泄漏身份。

可采用等价、证据强度不低于上述条件的匿名机制。必须有判别 self-test：目标 socket accepted=0、第二条
socket 恰好补足 aggregate ACK 时仍 FAIL；单一 socket、一次 admission、ACK 正常则 PASS。

### A3. 明确不做

- 不重写 SSH actual-tuple、accepted ACK 服务端 fresh-sequence 判定、OutboundSender、容量/session/wire；
- 不处理 arbitrary workflow SHA、candidate trust、UFW profile、任意 Caddy plugin/结构、通用 SSH 来源证明；
- 不碰 S4、S3 analyzer、frontend/Stage 2 产品语义或 `docs/orchestration/**`；
- 不执行真实 DNS/UFW/SSH/Caddy/systemd 操作，不开启 Vultr，不跑 600 秒 soak，不访问 Rutgers；
- 不把 jq/Caddy/Linux 外部门虚报为本机已执行。

### A4. 工作方式、验证与回报

使用 Claude Code 官方 dynamic workflow。先由两个调查 agent 分别钉死 handler/listener 与 soak-exclusive
反例，唯一 writer 串行修改，再由独立复核员只攻击这两项；普通 minor finding 一律记录，不再扩轮。

只跑受影响 Rust、preflight/self-test/static/parser/shell syntax、deploy contracts 与 `git diff --check`。
不跑 workspace、完整 frontend、archive、600 秒 soak或新的全仓 security scan。

一次回报：commits；两个根因的旧/新判别；精确 tests；jq/Linux 哪些实际执行/哪些 pending；changed
files；保留合同；deferred；Git status；review range `df79aa4..HEAD`。不得宣布 P2 accepted/deployed/released。

## B. Claude Prompt（请用户从下一行开始原样复制）

```text
ultracode: 请继续使用 Claude Code 官方 dynamic workflow，只在
Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\parallel-wave-1\stage3-p2 的
codex/parallel-wave1-stage3-p2@df79aa4fab0f0e29fdb17fc946a234b5e1a85734 上完成 STAGE-3-R3/v1。
不要写主 checkout，不 reset/rebase/merge/push/tag/release，不修改 docs/orchestration/**，不访问 Rutgers，
不连接任何主机，不执行真实 DNS/UFW/SSH/Caddy/systemd 操作或 600 秒 soak。

先完整阅读主 checkout 的
Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\orchestration\tasks\STAGE-3-R3.md A 部分。
SSH actual --admin-source 合同、服务端 fresh issued ACK 计数、OutboundSender/容量、H1-H7 与既有采样/日志/
CORE 命名都已经通过，不得重写。本轮只修两个精确 false PASS：Caddy 判定必须服从同一 route 内 handler
chain 的终止顺序，并且同 origin port 的不同 listen address 不能互相冒充公网 host 的受保护路径；H9 的
全进程 accepted-ACK delta 必须证明 judged window 只有这一次 public watch admission 和这一条 active
socket，第二条 socket 不能替目标连接补 ACK 数。

按 A2 增加能打挂 df79aa4 的同-route static-before-proxy、同-port other-listener 和 second-socket
aggregate 反例，并保留健康 controls。内部 jq/metric 结构由你自主决定，但未知 Caddy handler 语义要
fail closed，metrics 不得带身份标签，wire 不变。本机没有 jq/真实 Linux 时继续诚实标 PENDING，不得用
镜像算法冒充 production jq 已运行。不要修 SSH control-probe 措辞、任意 SHA/UFW/其他 deferred，不碰
S4/S3/Stage 2。

只跑 A4 focused gates 和 diff-check，不跑全仓重门/archive/soak/security scan。完成后按 A4 一次性回报；
最终验收、集成和外部 evidence 仍由 Codex 决定。
```
