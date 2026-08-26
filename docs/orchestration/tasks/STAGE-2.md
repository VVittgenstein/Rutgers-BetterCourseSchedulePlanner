# Stage 2：S2（含 P1/L1/L2/L3）提醒生命周期完整收口

状态：**ACCEPTED — R1 已验收并集成到 `feat/s2-alert-delivery@6a35c74`**
Prompt 版本：`STAGE-2/v1`
Orchestrator：Codex
实现者：Claude
产品负责人/转发者：用户
预期基线：`feat/s2-alert-delivery@75cefb0`
外部交付：**整个 Stage 完成后一次回报、一次 Codex 集中验收**

## A. Stage 合同

### A1. 本阶段完成后用户得到什么

1. 页面在意外断线、睡眠唤醒或服务重启后自动恢复；恢复过程中不假装仍在监控。
2. 用户主动 Disconnect 后保持停止；只有用户通过现有产品交互明确重新建立连接时才恢复，旧异步工作
   不能把它复活，也不为此新增独立 Resume 产品能力。
3. 公网页面在连接前验证会话票，失效时在后台换新，并让后续 HTTP/WS 使用同一份新票；页面刷新或
   浏览器重启仍按既定公网合同清空页面内监控计划。
4. 公网只恢复本页面生命周期内由用户明确 START、且尚未 STOP/Remove 的 section+policy，不从
   selection 猜测；暂态失败不得被解释成用户撤销，终局结果按既有合同处理。
5. 本地继续以已验收的服务端 desired authority/coordinator 为唯一真相：页面重连只重新接入并读取，
   不发送 legacy START/STOP/UPDATE_POLICY；L1 行为不得倒退。
6. 只有连接、当前真实武装、心跳新鲜度、音频/通知送达能力和用户未静音全部可信时才显示 READY；
   其他情况显示可理解的降级原因与可行恢复动作。
7. AudioContext 被系统挂起时尽力自动恢复；需要手势或恢复失败时诚实降级并提供用户动作。
8. hidden-but-running 或音频实际失败时，可按用户授权使用当前页面级系统通知兜底；
   frozen/discarded 页面仍明确是 residual risk。
9. 本地每个打开的 tab 都证明自己的 presence。最后一个页面离开后显示 60 秒倒计时；页面回来取消，
   到期复验仍无人时才优雅退出 RBCSP。
10. Windows 控制台显示启动、页面、监控、开放警报、Gate、倒计时与退出等有用事件，高频轮询不刷屏，
    也不泄漏会话票或个人数据。

完成声明只能是：

> `S2 + P1 client + L2 + L3 functional implementation complete and ready for Codex review；L1 regression preserved`。
>
> 公网部署仍被 `P2/H4` 阻断。

### A2. 权威输入与冲突顺序

Claude 开始前应读取：

1. `docs/orchestration/CURRENT.md`；
2. `docs/orchestration/STAGE-EXECUTION-PLAN.md`；
3. 本文 A 部分；
4. `docs/orchestration/tasks/M0-M1-001-R4.md` 末尾的 `ACCEPTED` 记录；
5. `docs/design/2026-08-20-alert-delivery-integrity.md` v3.2；
6. `docs/design/2026-08-23-desired-watch-reduced-scope.md` v2；
7. `docs/design/2026-08-20-review-package-public.md` 与 local 总案；
8. `docs/design/2026-08-20-pr-acceptance-checklist.md` 中仍有效的 S2/P1/L2 条目。

冲突优先级（从高到低）：`用户最新决定 → CURRENT → 本 Stage 合同 → Stage 总计划 → alert v3.2
→ reduced-scope v2 → 两份总案/checklist → 已 superseded 旧设计`。

早期 desired projection/leader 方案、旧 `M2-001` 和被划线/标记 superseded 的验收项不能复活。

### A3. 已完成地基：使用并守住，不重新设计

- S1 Gate 和严格 probe 上界；
- L1/reduced desired 的 HTTP/CAS/coordinator/materialization/UI/restart/reset；
- 服务端 app-level PING 与页面 ACK 基础；
- 公网 `/api/v1/session/validate`、session registry、`reserve_ws` lease 与 per-session cap；
- shared secondary WebSocket route seam 与 64 KiB 帽；
- local/public target 分离和现有 capability/architecture verifier。

如果内部实现需要小幅重构可以自行决定，但不得改变这些已验收合同或把它们重建成另一套架构。

### A4. 必须保持的冻结行为

这些是产品/协议合同，不是建议实现：

- 意外断线按 `1/2/4/8/16/30/30...` 秒基础退避持续恢复；公网合法 `Retry-After` 只能把下一次等待
  延长为基础退避与服务端要求中的较大值；显式 Disconnect/dispose 是硬屏障；
- app PING 的客户端可信输入必须包含正的 safe-integer sequence；READY 的心跳证据最多陈旧 25 秒；
- Readiness 常驻整个 SPA；页面可见性变化、恢复/唤醒等生命周期事件会重新核对真值链，DEGRADED
  不得被普通 toast 或通知遮蔽；
- 公网连接尝试前走既有 validate/renew 合同；换新票只保存在页面内存，不写 storage/cookie/DOM；
- 公网重连计划只在当前页面生命周期存在；本地 desired 则继续持久化并由 coordinator 物化；
- 本地 presence 路径为 `/api/v1/local/presence`，是 local-only、每 tab 常驻且独立于 watch Disconnect；
- presence 只有在固定期限内收到唯一合法的 HELLO/tab identity 后才计数；Ping/Pong 不延长注册期限，
  非法、缺失或重复 HELLO 必须关闭且不泄漏计数；
- presence 计数归零后的产品时间为 60 秒，页面回归优先于到期退出；
- 页面级通知 capability slug 为 `current-page-notification`；只允许当前运行页面调用浏览器 Notification，
  server/native/service-worker/web-push 继续禁止；
- 通知权限只随首次真实 START 的同步用户手势链请求，不能因先 `await` 而丢失手势；应用内设置可关闭；
  hidden、实际 cue 失败或连续重连失败超过 2 分钟时按已批准规则触发并去重；
- 通知政策从当前实际 marker 集合执行“移出三个页面 marker、Rust 四表面继续负控、重新递增版本并重算
  两种摘要”。不得照抄旧基线数字；当前已知动作是 215 减 3，但最终值以实现时真实集合证明；
- 多 tab 不要求实时同步，多个页面都可能响；frozen/discarded 页面不承诺必达；
- 公网刷新/重启浏览器归零；本地关闭最后页面后拆物理 watch、保留 desired，60 秒只决定进程退出；
- L1 暂态装配重试继续受 generation/revision/epoch 约束，STOP/policy/reset 使旧工作失效；永久失败
  停止重试但保留 desired，服务端不代用户 STOP；每 section 至多一份物理 watch，告警扇出全部页面。

### A5. 实现自由度

本文不规定具体类名、内部类型、文件拆分、subagent 数量、commit 数量或逐步测试命令。Claude 应先读取
真实代码和历史，再由 dynamic workflow 根据依赖关系决定方案；允许在不改变 A1/A4 的前提下修正文档中
已经过时的内部实现提示。

合理的工作流应覆盖理解、实现、focused verification、集成、独立合同复核和失败驱动的窄修，但不要求
按本文人为切成固定 phase。并行 agents 默认只做只读调查、测试分析和独立复核；当前权威 checkout 的
产品写入、`git add/commit` 与最终集成由单一 writer/integrator 串行执行。确需并行实现时只能使用基线明确的
隔离副本，并由 integrator 审查后回收，不能让多个 agents 并发写当前 checkout。

### A6. 验收场景

Claude 可以自行组织测试，但最终证据至少要证明以下用户场景：

- 意外 close/error 不会重复排队或并发创建多个连接；持续失败按批准退避恢复，成功后恢复正常节奏；
- Disconnect、dispose 和已 STOP/Remove 的 section 在任何迟到事件后都不复活；现有产品交互可以明确
  重新建立连接，不新增独立 Resume 产品能力；
- 公网 valid、renewed、限流/暂时失败、非法响应和服务重启场景不会混用旧票，也不会并发重复换票；
- 公网 `Retry-After` 会延长而不会缩短基础退避；
- 公网活 WS 不因两小时 TTL 失效；lease 释放后重新遵循既有活动窗口；
- 公网只重建真实页面内监控计划；本地重连不发送 legacy lifecycle command；
- 五环任一环失效都撤绿；最后一个合法 PING 后即使再无 UI/网络事件，超过 25 秒也会降级；
- armed 证据属于当前连接/authority epoch 和当前 policy，旧 START/旧 authority 不能点绿；
- 音频 running/suspended/blocked/closed 及用户恢复动作均诚实反映；
- hidden、cue 失败、重连超过 2 分钟、通知权限拒绝/默认/允许、首次 START 手势、设置关闭及去重场景
  符合已批准边界；
- local idle page、多 tab、刷新、60 秒内浏览器重开、异常 presence 断线、回归×到期竞态均不会误退出；
- mounted tab 的 presence 意外断线会独立恢复；真正消失的连接不形成永久 ghost，旧连接的迟到断开
  不会删除 replacement；watch Disconnect 不影响 presence；
- presence 缺失/非法/重复 HELLO 以及只发 Ping/Pong 的半连接不会计数或延长注册期限；
- 最后页面确实离开超过 60 秒后走既有 ordered graceful shutdown，desired 仍可供下次启动恢复；
- 控制台包含约定的关键生命周期事件；倒计时按 60→0 输出、回归后记录取消且旧倒计时停止；关键用户
  文本至少随既有 en-US/zh-CN locale 变化；INFO 不刷高频 polling/tick，且不包含 nonce/session header/
  敏感 URL/完整请求体；
- local/public manifest、frontend source/bundle、Rust 四表面、release gate、CI 入口和两类摘要同步；
  声明 slug 的页面通知正例放行，server/native/service-worker/web-push 反例继续拒绝；
- public 构建没有 local presence/desired 持久权威表面，local 构建没有新增公网票逻辑；
- S1 和已验收 L1/reduced desired 的现有行为全量回归。

测试应优先使用仓库已有 fake clock、held promise、fake socket/audio 与 host/runtime integration 接缝；
不为了测试理论不可达分支而新增大架构。

### A7. 不做与允许延期

本 Stage 不做：

- H4/完整 P2、Caddy 10 分钟 soak、真实部署；
- S4、S3；
- N1–N3；
- desired WebSocket、leader/Web Locks/BroadcastChannel、跨 tab 实时同步或只响一次；
- public 持久 watch、resume cursor；
- 浏览器自动注册、resumeHint/跨重连去重；重连后仍开放课节再提醒一轮维持已接受行为；
- native/server/service-worker/web-push 或 frozen/discarded 页面必达；
- 3 秒轮询、100% 严格名单校验、会话级三声响铃额度改动或增量 `open_section_current`；
- 新的日志平台、日志文件轮转、遥测系统；
- 与本阶段用户结果无关的广泛重构、提前资源优化或“未来可能用到”的 abstraction。

普通低严重度清理、理论 defensive case、按固定预算仍不复现的 transient 可以作为 deferred debt 带走。
只有 A1/A4 主合同、安全/target 边界、presence 误退出、新无界资源或核心假阳性测试才阻断。

### A8. 验证与交付边界

开发过程中由 Claude 选择 focused tests。所有功能集成后，在最终 head 集中运行：

```text
cargo test --workspace
node tools/architecture/verify-rust-graph.mjs
node tools/architecture/verify-public-rust-zero-surface.mjs
cd frontend && npm run verify
git diff --check 75cefb0..<head>
git diff --check a4f8d22..<head>
```

另外运行 Notification policy/verifier/release-set/CI-entry 的非 archive self-tests，以及足以证明 A6 的
本地进程/host 和公网 composition 集成测试。此 Stage 不要求构建正式 archive、
不跑真实 Caddy soak、不访问 Rutgers/外网、不安装工具、不部署、不 merge/tag/release。最终候选在余下四阶段结束后
统一准备，避免每个阶段重复打包。

对失败先保存 command/status/output；同一 full gate 最多再跑一次，可定位 case 最多再跑两次。若复现则
诊断修复；仍不复现则如实列入 deferred observation，不做十余次无证据复跑。

### A9. 允许提前停止的真实阻断

- 当前 branch/head 不符或有与本 Stage 重叠的未知源码修改；
- dynamic workflows 因 version/plan/config 实际不可用；
- 需要改变 A1/A4 的用户行为、wire/数据库权威合同或通知政策边界；
- 完成 Stage 必须提前实施 P2/H4 或需要联网、安装、部署、权限/不可逆外部动作；
- 仓库已有测试/实现证明已批准设计在当前技术条件下不可兑现。

普通编译错误、测试失败、同范围竞态、文件组织和低严重度 finding 不是停止理由。

### A10. 最终一次性回报

最终回报保持简洁但可验收：用户现在能做什么；final head 与逻辑 commits；主要设计取舍；按 A6 汇总的
场景证据；focused/final tests 的精确结果；最高风险合同的判别证据；deferred debt/residual risks；
Git status 与 `75cefb0..<head>` review range。不要自行宣布公网可部署、项目完成或 Stage 被 Codex 接受。

## B. Claude Prompt（请用户从下一行开始原样复制）

ultracode: 请使用 Claude Code 官方的 dynamic workflow，把 RBCSP 的整个
`STAGE-2/v1` 作为一个完整交付单元实施、集成并验证。

仓库是 `Z:\Project\Rutgers-BetterCourseSchedulePlanner`，预期产品代码基线是
`feat/s2-alert-delivery@75cefb0`。Codex 负责范围、最终审查和验收；你负责实际实现。先核对 Git、当前代码、
已有测试和历史，再完整阅读：

- `docs/orchestration/CURRENT.md`；
- `docs/orchestration/STAGE-EXECUTION-PLAN.md`；
- `docs/orchestration/tasks/STAGE-2.md` 的 A 部分；
- `docs/orchestration/tasks/M0-M1-001-R4.md` 末尾 ACCEPTED 记录；
- A2 指定的已批准设计。

请由你根据真实依赖设计并运行 workflow。本文钉死的是范围、用户结果、不变量、验收场景、最终 gates 和
停止条件；内部 phase、subagent 数量/分工、文件与类型组织、逻辑 commits、focused-test 组合由你动态决定，
并随实现证据调整。并行 agents 默认只做只读调查、测试分析和独立复核；当前权威 checkout 的产品写入、
`git add/commit` 与最终集成由一个 writer/integrator 串行执行。确需并行实现时使用基线明确的隔离副本，
再由 integrator 审查回收。形成“理解 → 实现 → focused verification → 集成 → 独立合同复核 → 按失败证据
窄修”的闭环，不在内部阶段间向用户/Codex handoff，整个 Stage 完成后只回报一次。

范围只包括剩余 S2、P1 浏览器端、已完成 L1 的回归保护、L2、L3 和已批准的当前页面通知政策修订。
不要重开 desired/CAS/coordinator，除非新 Stage 改动直接造成回归；不要实施 H4/完整 P2、S4、S3、发布、
leader/实时跨 tab、public 持久 watch、native/service-worker/web-push 或其他未讨论功能。拒绝过早优化：
没有现实路径和失败证据、也不影响本 Stage 主合同的问题，记录为 deferred debt 后继续。

Codex 已知工作树包含本轮 orchestration 文档以及两个必须保持未跟踪的
`docs/conversations/2026-08-23-*` 目录；先精确列出并与 CURRENT 对照。orchestration 文档保留原意并纳入合适
的逻辑 commit，conversation 目录不提交、不移动、不删除。其他与本 Stage 重叠的未知修改才是 blocker；
不得 reset/rebase/改写既有历史或覆盖用户文件。

按 A1/A4 实现，按 A6 自主设计具有判别力的测试，最后按 A8 只集中跑一次重门。不要在本 Stage 重复构建
正式 archive，不访问 Rutgers/外网，不安装工具，不部署、merge、tag 或发布。普通低严重度问题、样式清理、
理论 defensive case 和按预算不复现的 transient 不阻断；只有 A9 的真实情况才提前停止。

若 dynamic workflows 不可用，在修改产品代码前回报实际 version/plan/config 原因，不要静默改称普通
subagent 流程。完成后按 A10 一次性回报；S2 是否 accepted 由 Codex 独立决定。

## C. Codex Stage 2 独立验收记录（2026-08-25）

- Claude 交付 head：`553371f8fa449b8c7cb9a88b5f32e179cb1e57c5`；审查范围：
  `75cefb0a27b85495d9b5a6e2c7f93b5590b3ee95..553371f8fa449b8c7cb9a88b5f32e179cb1e57c5`；
- 结论：`CHANGES_REQUIRED`。Stage 2 主体已落地，但 A1/A4 的音频真值、presence 回归优先、应用内通知
  关闭入口和 A6/A8 policy 执行闭环仍有四个阻断；不得进入 Stage 3；
- 已确认通过：shared 意外断线退避与显式 Disconnect 屏障、公网页面内 ticket validate/renew 与
  reconnect plan、本地 desired/L1 回归、25 秒 heartbeat 与 authority/connection stamp、页面级
  Notification 基础、local presence/60 秒倒计时、L3 控制台和 local/public target 边界；
- 音频阻断：`unlock()` 清掉已知 output failure 后，只凭 context `running` 返回 READY。Codex 定向运行
  得到 `failedCue=FAILED, falseRecovery=READY`，即失败仍存在却被无输出重试的恢复动作擦掉；
  `heal()` 等待 held resume 前也不发布下降状态，Provider 可无限保留旧 READY；现有测试注释声称
  “BLOCKED then READY”，实际只断言最终 READY；
- presence 阻断：socket 在 CountingDown 被接入、withhold HELLO，tick 先进入 Exiting/request_exit 后，
  迟到 HELLO 仍会写 tab、发送 REGISTERED 并报告 PageOpened；ordered shutdown 已不可取消，仍打开的页面
  会被退出；现有测试只覆盖 Exiting 后新 connect，没有覆盖预先接入的 socket；
- 通知控制阻断：Provider 有 `setNotificationsEnabled(false)`，但生产 UI 没有调用入口；测试通过直接注入
  false/调用 Provider API 绕开了缺失的用户交互；
- policy 阻断：`.github/workflows/public-ops.yml` 的新步骤只执行两项 Rust policy tests；全仓唯一 workflow
  没有安装/执行 frontend `test:guard`。release-set 仅增加 required slug，Claude 只做 parser check，未完成
  A8 要求的 non-archive 正反 self-test；
- Codex final-head 独立重门：`cargo test --workspace` exit 0；两项 Rust architecture verifier 与两项
  verifier self-test 全绿；串行 `frontend npm run verify` 为 guard 90、Vitest 333/333、typecheck、local/public
  build 全绿；两个 diff-check 和三份 PowerShell parser check 全绿；
- 第一次 frontend full gate 与 workspace 并发运行时，Vitest 有 20 个 fork worker 启动超时，未出现断言
  failure（已有 9 files / 58 tests pass）；按 A8 预算在 Rust 完成后串行复跑一次即全绿，记录为环境资源
  争用观察，不构成代码阻断；
- Codex Security diff scan `4b394b71-967d-42bf-b8ac-8186c7cb5d22` 覆盖 34/34 inventory，封存为 0 个
  可报告安全 finding。若干 localhost/self-only correctness/resource 候选按安全策略排除，但其中音频假绿与
  presence 误退出仍按产品合同阻断；
- 不要求本轮处理：P2/H4、S4/S3、N1–N3、permission 极窄外部撤权窗口、notification ledger 长期小量增长、
  local presence 防御性连接帽、first-party Retry-After 超大值 clamp、已披露 console/UPDATE_POLICY 等债务；
- 已生成一次集中窄修任务 `docs/orchestration/tasks/STAGE-2-R1.md`。本轮不构建 archive、不重跑完整安全
  扫描、不扩大产品范围。
