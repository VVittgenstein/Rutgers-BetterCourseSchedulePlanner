# Stage 3 R2：把 H8/H9 的“看起来通过”收紧为实际证据

状态：**READY — 返回原 Stage 3 worktree 做最后一次窄修**

Prompt 版本：`STAGE-3-R2/v1`

基线：`codex/parallel-wave1-stage3-p2@6c7c7cd5c19a550034977bb2ad5ca845e4132648`

## A. 修复任务包

### A1. 本轮为什么仍不能集成

`37176d2..6c7c7cd` 已关闭出站 payload 在 retained queue 前预留、H9 采样完整性以及
CORE/full PASS 命名问题；这些实现和已有测试全部保留。Codex 独立复跑 `bcsp-application` 116、
`bcsp-public-runtime` 52、`bcsp-local-runtime` 44 项测试均通过。

仍有且只有以下三个冻结合同 blocker：

1. SSH preflight 只测试两个 RFC 5737 示例来源地址，未测试真实管理连接。一个只对真实来源生效的
   `Match User root Address ...` 仍可重新打开 root 密码登录并让 preflight PASS；DNS 无法给出真实
   local address 时当前代码也只是 warning；
2. adapted Caddy 检查只找任意通往 `127.0.0.1:8080` 的 proxy，没有把它绑定到
   `BCSP_PUBLIC_ORIGIN` 的 host route。另一个域名的受保护 proxy 可以替真实公网域名冒充 H2 PASS；
3. browser 在 `socket.send(ACK)` 后本地加计数，server 日志只证明“没有看到拒绝”。如果服务端以后静默
   忽略合法 ACK、但任意入站文本仍刷新 transport activity，当前 H9 仍会通过；这不满足 server-side
   acceptance evidence。

这三项都来自既定 `STAGE-3-R1` A3/A4，不是新的平台加固范围。

### A2. 必须完成的行为

#### H8 SSH：评价实际 root connection policy，否则失败

- **本轮没有 Vultr/真实主机，也不取得真实 tuple。** 这里只实现供未来部署使用的只读输入接口、判定逻辑、
  fixture 与 runbook；实现代理不得向用户索要 IP、hostname、SSH 配置、账号、密码或密钥，不得连接任何
  主机。真实 tuple 上的执行证据保持 `PENDING_EXTERNAL_DEPLOYMENT`；
- 不得再把固定 synthetic IP 矩阵当作上线 PASS 证据；
- preflight 必须让 operator 明确提供实际管理连接所需的 root tuple 信息，或采用能同等可靠地评价该
  实际 tuple 的机制。至少要让 `sshd -T -C` 得到真实的 `user/root`、remote host/address、local
  address/port；如果实际条件缺失、无法解析或无法可靠评价，结果必须是 FAIL，不是 warning 后 PASS；
- 如支持多个管理来源，允许重复声明/逐一验证；不得用两个示例地址替代实际来源；
- 对该 tuple 同时判断 `PermitRootLogin`、`PasswordAuthentication`、
  `KbdInteractiveAuthentication` 及实现实际需要的相关控制。keys-only 可通过，任何密码形态可达必须失败；
- CLI 形状、参数命名和内部解析由实现代理决定，但 runbook 与脚本 usage 必须一致，且 preflight 仍只读。

必须有能打挂当前 head 的反例：全局安全，但 `Match User root Address <实际声明来源>` 或实际 host 条件下
重新打开 root/password；当前 synthetic probes 会漏掉，新实现必须失败。缺实际 tuple/local address 也必须
失败。另保留 keys-only 正例。

#### H2 Caddy：保护必须属于真实公网 host route

- 在 `caddy adapt` 的结构化输出上，把检查绑定到 `BCSP_PUBLIC_ORIGIN` 的 canonical host；
- 必须证明这个 host 的 active route 实际到达 `127.0.0.1:8080`，且所有适用于该 host 的这类
  `reverse_proxy` 都带精确 4h `stream_close_delay`；
- 只有其他 host/site 的受保护 proxy、真实 host 只有 static response、真实 host 的某条 8080 proxy
  未保护、注释或其他 upstream 均不得使门通过；
- 不要求建立通用 Caddy 管理器，只需正确处理仓库支持/示例产生的 adapted route 结构并对无法可靠解释的
  结构 fail closed。

必须新增“真实 host 未保护 + 其他 host 的 8080 proxy 已保护仍 FAIL”的判别 fixture；保留真实 host 正例和
既有 comment/other-upstream/all-not-any 反例。

#### H9 ACK：加入服务端接受的正向证据

- 在合法 `HeartbeatAck` 真正通过 protocol/sequence 校验并被服务端接受之后，产生可由本次 soak 读取的
  server-side evidence；不能在 transport 收到任意文本时提前记账；
- 可采用安全聚合 metric/counter 或等价确定性证据，不修改既有 wire contract；不得记录 session、nonce、
  section 或用户标识；
- soak 在开始前取基线、结束后取终值，并把本次 server-accepted delta 与浏览器实际发送/期望的 ACK
  关联。读取失败、counter 倒退或不足必须失败；
- 正证必须绑定本次 service invocation/soak。保留当前 journal 可读性、启动锚、无 rejected frame、完整
  采样窗口和 CORE/full PASS 规则；
- 新增能打挂当前实现的判别测试：浏览器发送 ACK 但服务端接收逻辑被静默绕过时，H9 analyzer/harness
  必须失败。合法 ACK 增长、畸形/旧 sequence 不增长要有 focused 证据。

### A3. 明确不做

- 不重写已经成立的 `OutboundSender`、容量常量、write deadline、session registry 或 H1–H7；
- 不修 manual workflow 任意 SHA、candidate trust、UFW app-profile、Ping 单独 deadline 判别器或其他
  deferred；
- 不执行真实 DNS/UFW/SSH/Caddy/systemd 修改，不跑 600 秒 soak，不访问 Rutgers；
- 不要求当前用户创建/开启 Vultr，不获取或保存任何真实 SSH 地址、凭据或密钥；
- 不碰 S4、S3 analyzer、Stage 2 产品语义；
- shared sender 若继续机械触及 local presence，必须保留主线已经验收的 Exiting/late-HELLO 行为，并在
  回报中列出 integration note；
- 不修改 `docs/orchestration/**`。

### A4. 工作方式与验证

使用外部实现工具的官方 dynamic workflow。先并行做三项只读根因调查和反例设计，再由唯一 writer 串行修改，
最后做一次覆盖三项的对抗复核；不要在每个小 finding 后停下来等用户。

只跑受影响的 Rust、ops/static/self-tests、parser、shell syntax 和 `git diff --check`。不得跑 workspace、
完整 frontend、archive、真实 soak 或新的全仓 security scan。已有 focused tests不得删除或放宽。

完成后一次回报：Outcome；commits；三个 blocker 的机制与判别证据；changed files；精确 tests；negative
proof；保留的 external Linux/H8/H9 gates；known limitations；Git status；review range
`6c7c7cd..HEAD`。不得宣布 P2 accepted/deployed/released。

## B. 实现任务提示（请用户从下一行开始原样复制）

```text
ultracode: 请继续使用外部实现工具的官方 dynamic workflow，只在
Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\parallel-wave-1\stage3-p2 的
codex/parallel-wave1-stage3-p2@6c7c7cd5c19a550034977bb2ad5ca845e4132648 上完成 STAGE-3-R2/v1。
不要写主 checkout，不 reset/rebase/merge/push/tag/release，不修改 docs/orchestration/**，不访问 Rutgers，
不执行真实 DNS/UFW/SSH/Caddy/systemd 改动或 600 秒 soak。

当前没有开启 Vultr，也没有真实主机。本轮只实现未来 preflight 接收实际 SSH connection tuple 的只读
接口、判定逻辑、fixture 和 runbook；不得向用户索要真实 IP/hostname/SSH 配置/账号/密码/密钥，不得连接
任何主机。真实 tuple 的执行证据继续标为 PENDING_EXTERNAL_DEPLOYMENT，不能用 synthetic fixture 冒充。

先完整阅读主 checkout 的
Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\orchestration\tasks\STAGE-3-R2.md A 部分。
本轮只关闭三个仍可 false PASS 的冻结合同：SSH 必须评价 operator 提供的实际 root connection tuple，信息
不足或无法可靠评价时 FAIL，不能再用两个 synthetic IP 冒充；adapted Caddy 必须把带 4h delay、通往
127.0.0.1:8080 的 active proxy 绑定到 BCSP_PUBLIC_ORIGIN 的真实 host route，其他 host 不能代替；H9
必须取得合法 HeartbeatAck 被 server 真正接受后的正向、与本次 soak 关联的证据，浏览器 send 加计数和
“日志里没看到拒绝”都不够。

内部 CLI/API/metric 组织由你自主决定；保持 wire、H1-H7、OutboundSender/容量与现有 H9 完整采样、
journal、CORE/full PASS 修复不变。为三项分别新增能打挂 6c7c7cd 的精确反例，保留所有旧测试。不要修任意
SHA/candidate trust、UFW、fairness、Ping 单测等 deferred，不碰 S4/S3/Stage 2。若 shared sender 文件与
Stage 2 presence 有机械交叉，只做类型兼容并明确提醒 Codex 集成时保留 Exiting/late-HELLO guard。

只跑 A4 focused gates 和 diff-check，不跑全仓重门/archive/soak/security scan。完成后按 A4 要求一次性
回报；P2 的接受、集成和外部 Linux evidence 仍由 Codex 决定。
```
