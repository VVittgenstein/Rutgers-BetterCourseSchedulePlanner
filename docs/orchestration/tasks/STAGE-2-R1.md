# Stage 2 R1：音频真值、presence 退出竞态与通知控制闭环

状态：**ACCEPTED — 已串行回收到 `feat/s2-alert-delivery@6a35c74`**
Prompt 版本：`STAGE-2-R1/v2-parallel`
Orchestrator：Codex
实现者：实现代理
产品负责人/转发者：用户
预期基线：`feat/s2-alert-delivery@553371f`

## A. 窄修任务包

### A1. Codex 验收结论与本轮目标

实现代理已在 `75cefb0..553371f` 完成 Stage 2 的主体实现：shared 自动重连与显式 Disconnect 屏障、
公网换票、当前连接/authority stamp 的 Readiness、页面级 Notification、local presence/60 秒退出、L3
控制台和 target/policy 边界都已接线；S1 与 L1 回归门也通过。

Codex 对该范围的独立结论仍是 `CHANGES_REQUIRED`。只剩四个当前 Stage 的真实阻断：

1. AudioContext 的已知 cue/output failure 可被未重试输出的 unlock 擦掉，并在异步 resume 等待期间保留
   旧 READY；
2. countdown 期间已接入但尚未 HELLO 的 presence socket，可以在 `Exiting` 后注册并被程序随即杀掉；
3. Provider 虽有通知开关 API，生产 UI 没有让用户关闭通知的入口；
4. Notification 的 frontend source/bundle policy self-tests 没有真正进入 CI，release-set/CI-entry 的
   non-archive 判别证据也未闭合。

本轮是 **Stage 2 内的一次集中窄修**，不是新 Stage，不回到 M2/M3 式拆分，也不进入 P2/S4/S3。

### A2. 基线、worktree 与写入纪律

- 独立 worktree：`Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\parallel-wave-1\stage2-r1`；
- 独立分支：`codex/parallel-wave1-stage2-r1`；
- 该分支从包含本任务包的 orchestration-only anchor 建立；产品代码父基线为
  `feat/s2-alert-delivery@553371f8fa449b8c7cb9a88b5f32e179cb1e57c5`；
- 不 reset、rebase、改写既有提交，不 merge/tag/release；
- orchestration anchor 已由 Codex 提交。实现代理不修改或重复提交 `docs/orchestration/**`；
- 两个 `docs/conversations/2026-08-23-*` 目录继续未跟踪，不提交、不移动、不删除；
- 除上述内容外若有与本轮重叠的未知修改，停止并精确回报；
- 本 worktree 仍由单一 writer/integrator 写入和提交。Dynamic workflow 可用于只读调查、测试设计、
  独立复核和隔离验证，不允许多个 agent 并发写同一 worktree。

### A3. 阻断一：音频 READY 必须来自当前可播放证据

当前 `WatchAudioController.unlock()` 会先清掉既有 `audioFailure`，随后只要 context 报 `running` 就直接
返回 READY；它没有重新创建/启动声音节点。因此一旦真实 cue 已经证明 oscillator/gain/start 失败，用户
点击恢复仍会在没有重试输出的情况下被显示成“会提醒你”。

Codex 用未修改源码做了定向运行证据：fake context 为 `running`，一次真实 preview/voice 尝试返回
`FAILED`，同一输出仍失败时再次 unlock 却返回 `READY`。初次 `running` 本身仍按冻结设计作为 ring ③
证据；本轮修的是**已经观测到的失败不能被无证据擦除**，不要求每次 Start 都额外播放测试音。

另一个确定性窗口是：已经 READY 的 context 变成 `suspended`，`heal()` 在等待一个 held `resume()` 前
没有发布 BLOCKED/UNLOCKING。Controller 的实时 getter 已是 BLOCKED，但 Provider 仍持有旧 READY，直到
Promise 结束；若 Promise 挂起，假绿可无限持续。现有测试注释写“BLOCKED then READY”，实际只断言包含
READY，不能证明下降沿。

修复后的行为必须满足：

- 已知 voice/device/node 失败不能被一个未重试输出的 unlock 清除；恢复 READY 必须来自新的实际输出
  尝试成功，或严格等价且能证明该已知失败已消失的证据；
- 没有已知 cue/output failure 时，继续按冻结设计以 `AudioContext.state === 'running'` 作为 ring ③；
  不新增“每次 Start 强制响一声”的产品行为；
- context 进入 suspended 后，在异步 resume 尚未完成时立即撤销 READY；成功后才恢复，blocked/failed/
  closed 分别保持诚实；
- 恢复不得替用户取消真实 mute；现有 continuous cue、notification fallback 与 Start 行为保持。

最低判别测试：

1. 先在 READY 状态制造一次真实 cue/preview 创建、连接或启动 voice 失败；随后点击真实恢复动作，在输出
   仍失败时不得 READY；
2. 同一路径只有新的实际输出尝试成功后才能 READY；
3. suspend 后用 held resume：Promise 未释放前 Provider/常驻 Readiness 已降级，释放成功后才复绿；
4. 把生产修复回退时，上述测试确定失败。现有“BLOCKED then READY”测试必须断言完整下降/恢复序列，
   不能只检查最终 READY。

具体内部 API、是否复用 preview voice、状态枚举与文件组织由实现代理根据现有结构决定；不要为此新增音频
设备管理平台、持久化或新的产品能力。

### A4. 阻断二：`Exiting` 后任何未完成 HELLO 都不得注册

现有 `connect()` 只在初始接入时检查 `Exiting`。一个 socket 可以在 CountingDown 时被接入但暂不 HELLO；
maintenance tick 随后以 pages=0 把 phase 切为 `Exiting` 并触发 ordered shutdown；该 socket 的合法 HELLO
再落地时，`receive_text()` 仍写入 tab、发送 REGISTERED 并打印页面已打开。shutdown 已不可逆，用户得到的
是“注册成功”后程序退出。

修复必须让 shutdown decision 与 HELLO registration 在同一状态权威下互斥：

- 一旦 phase 已是 `Exiting` 或 presence 已 sealed，任何预先接入但未完成 identity 的 HELLO 都被拒绝，
  不计数、不发送 REGISTERED、不打印 PageOpened，也不能取消已发出的 shutdown；
- 在到期决定真正提交前完成的合法 HELLO 仍优先取消 countdown；
- replacement identity、迟到旧 disconnect、10 秒未注册清理、Ping/Pong 不延长期限、watch Disconnect
  与 presence 独立等现有行为不回退。

最低判别测试使用已有私有 harness/fake countdown 建立无长 sleep 的顺序：已有页面离开 → 新 socket 在
CountingDown 被接入但 withheld HELLO → 到期 tick 进入 Exiting/request_exit → 迟到 HELLO。断言 pages=0、
无 REGISTERED、无 PageOpened、exit 只触发一次。保留“HELLO 在到期决定前落地可取消退出”的正例。

### A5. 阻断三：通知开关必须是用户能操作的生产产品入口

Provider 已实现 `notificationsEnabled`、关闭后清 queue 与 setter，但 `SharedApplication` 没有提供初始设置/
回调，生产 UI 唯一调用只会设为 `true`。现有测试直接向 harness 注入 `false` 或调用 Provider API，证明了
helper，却没有证明用户能在真实界面关闭。

本轮只需完成已批准的最小产品合同：

- 在合适的现有监控/设置界面提供真实可达的开/关控制；
- 关闭后不再请求 permission、不再排队或发出页面通知，并清掉尚未发出的 queue；
- 重新开启沿用既有用户手势/浏览器权限边界；不得后台擅自改浏览器权限；
- 用点击真实生产控件的集成测试证明，而不是直接调用 context/provider 方法。

原设计没有要求把该开关写入 SQLite、cookie 或 localStorage。本轮不得自行新增数据库字段、迁移或新的
跨页面同步；只实现当前页面生命周期内可用的应用内控制即可。

### A6. 阻断四：通知能力政策必须真正由 CI 与 non-archive self-test 执行

当前 source/bundle/manifest 和 Rust 四表面的实现本身通过，但 `.github/workflows/public-ops.yml` 新步骤只
运行两项 Rust architecture 测试，没有安装/运行 frontend `test:guard`，其注释却声称 policy tests 已进入
CI。未来 frontend Notification 越界可在这条 workflow 中静默通过。

修复必须：

- 让 frontend Notification source/bundle/manifest 的正反 policy tests 在现有 CI workflow 中实际运行；
- 添加或扩展最小 non-archive 判别测试，证明 CI entry 被移除会失败；
- 同样证明 release-set 需要 `current-page-notification`：有 slug 通过、缺 slug 失败；PowerShell 仅 parser
  clean 不是该语义证据；
- server/native/service-worker/web-push 负控、212 marker、markerSetVersion 3、两摘要与当前 manifest
  不得漂移，除非修复本身证明真实集合确需变化。

采用 workflow 静态合同测试、可复用 policy helper 或等价的最小方案均可；不要为 CI 新建平台、发布流或
正式 archive。

### A7. 明确延期，禁止借窄修扩展

以下不触发本轮修改：

- P2/H4、global/per-client WS 容量、共享 outbound byte budget、Caddy/systemd、soak、部署；
- S4、S3、N1–N3、desired 架构、leader/BroadcastChannel/跨 tab 实时同步；
- public `UPDATE_POLICY` 无确认而暂时 PREPARING、console UTC、10 秒 HELLO 常量、tracing subscriber
  先占用的已披露限制；
- recovery GET 挂起时其他 section 使用旧 snapshot；
- 外部撤销 Notification permission 的 hidden-page 极窄缓存窗口；
- notification ledger 的长期小量集合增长、local presence 的额外防御性连接帽、first-party
  `Retry-After` 的超大值 clamp、HELLO deadline 最多一个 maintenance tick 的边界余量。

这些项目要么已有后续 Stage、要么缺少现实攻击/失败强度、要么会引入新产品取舍。除非修复 A3–A6 时
自然消失且无需新抽象，不得顺手处理。

### A8. 验证与速度纪律

本 lane 只跑四个根因的 focused tests、A6 的 CI-entry/release-set non-archive self-tests、PowerShell parser
check，以及 `git diff --check`。不要在并行 worktree 中运行 `cargo test --workspace`、完整
`frontend npm run verify`、archive、全仓 architecture/security 门或 soak；这些重门由 Codex 在四个 lane
串行集成后只跑一次，避免多个 worktree 争抢 CPU、端口和缓存。

不要运行真实 Caddy/Rutgers，不联网安装，不部署、merge、tag 或 release。

若 full gate 首次出现无断言的 worker/环境 transient，保存完整输出后最多串行复跑一次；可定位 case 最多
复跑两次。不得十余次刷绿。

### A9. 最终一次性回报

一次性返回：Outcome；commits；A3–A6 每项的生产闭环和真实控件/竞态证据；changed files；focused/final
tests 精确结果；能打挂 `553371f` 的判别证据；deferred debt；Git status；`553371f..<head>`、
`75cefb0..<head>` 与 `a4f8d22..<head>`。不要自行宣布 Stage 2 accepted 或进入 Stage 3。

## B. 实现任务提示（请用户从下一行开始原样复制）

ultracode: 请使用外部实现工具的官方 dynamic workflow，只在
`Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\parallel-wave-1\stage2-r1` 的
`codex/parallel-wave1-stage2-r1` 分支上一次性完成 `STAGE-2-R1/v2-parallel`。该 worktree 从 Codex 的
orchestration-only anchor 建立，产品代码父基线是
`553371f8fa449b8c7cb9a88b5f32e179cb1e57c5`。这是 Stage 2 内唯一一轮集中窄修，不是新 Stage；不要逐
finding 停下来 handoff，也不要在主 checkout 写入。

先核对 Git 与工作树，然后完整阅读：

1. `docs/orchestration/CURRENT.md`；
2. `docs/orchestration/STAGE-EXECUTION-PLAN.md`；
3. `docs/orchestration/tasks/STAGE-2.md` 末尾的 Codex 独立验收记录；
4. `docs/orchestration/tasks/STAGE-2-R1.md` 的 A 部分。

Codex 冻结本轮结果与边界，你根据真实依赖用 dynamic workflow 自主安排内部 phase、只读调查 agents、
focused tests、文件/类型组织和逻辑 commits。本 worktree 只能由一个 writer/integrator 串行写入。不得
reset/rebase/改写旧历史；不得修改 `docs/orchestration/**`。两个 `docs/conversations/2026-08-23-*` 目录
若可见也保持未跟踪，不提交、不移动、不删除；发现其他未知重叠修改才是 blocker。

本轮只关闭四个根因：

1. 音频已知 voice/device/node failure 不能被一个未重试输出的 unlock 擦掉；真实恢复动作只有在新的输出
   尝试成功或严格等价证据出现后才能复绿。没有已知 failure 时仍按冻结设计以 AudioContext `running`
   作为 ring ③，不新增每次 Start 强制测试音。context 进入 suspended 后，held resume 未完成期间立即
   撤销 READY，成功后才恢复；不要替用户取消 mute。
2. presence shutdown decision 与 HELLO registration 必须互斥。socket 可在 CountingDown 时预先接入，
   但若 tick 已进入 Exiting/request_exit，迟到 HELLO 必须不计数、不发 REGISTERED、不打印 PageOpened；
   到期决定前已完成的合法 HELLO 仍取消 countdown。
3. 把现有 notification enabled 状态接到真实生产 UI，让用户可以开/关。关闭后不再 ask/queue/show 并清
   pending queue；用点击真实控件的测试证明，不能只向 Provider 注入 false。不要新增数据库迁移、storage
   持久化或跨 tab 同步。
4. 完成 Notification policy 执行闭环：frontend source/bundle/manifest 正反 self-tests 必须实际进入现有
   CI；增加最小 non-archive 判别证据，移除 CI entry 会失败，release-set 缺
   `current-page-notification` 会失败、有它会通过。parser clean 不能替代语义测试。

判别测试至少覆盖：已观测到的 voice 创建/启动失败不能由无输出重试的 unlock 清掉，新的输出成功后才
READY；held resume 期间常驻 Readiness 已降级；pre-admitted/withheld HELLO 在 Exiting 后被拒；真实 UI
关闭通知后不 ask/queue/show；CI/release policy 负例能打挂。保留既有 reconnect、P1、L1、Readiness、
presence、L3、target boundary 与 policy 正反例。

严格遵守 R1 A7：不要顺手做 P2/H4、S4/S3、desired 重构、permission 轮询、通知 ledger/连接帽、
Retry-After 防御性 clamp 或其他 deferred。只跑 A8 指定的 lane-focused tests 与 non-archive policy
self-tests；完整重门留给 Codex 集成。不要构建 archive，不联网、部署、merge、tag 或 release，也不要
重复重型安全扫描。

完成后按 A9 一次性回报，并明确 review ranges。Stage 2 是否 accepted 仍由 Codex 独立决定。

## C. Codex 独立验收（2026-08-25）

- 审查范围：`002f50d..5af49d9`；
- 四项 blocker 的生产路径与判别测试均成立；
- Codex 独立复跑：presence 16/16、四个前端 focused 文件 81/81、frontend policy 92/92、
  release-set SelfTest PASS；
- 普通 polish/deferred 项不触发修复；
- 结论：`ACCEPTED`，五个提交按原顺序 cherry-pick 为 `48bef74..6a35c74`。
