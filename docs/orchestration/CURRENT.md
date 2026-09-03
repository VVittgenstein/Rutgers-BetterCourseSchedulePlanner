# RBCSP 当前工作总账与 Codex–实现代理协作协议

状态：**ACTIVE — 当前唯一的工作恢复入口**
最后更新：2026-09-03（America/New_York）
维护者：Codex（orchestrator）
产品决策者：用户
实现者：实现代理

## 0. 本文用途

本文用于在以下情况发生后恢复稳定上下文：

- Codex 对话被多轮压缩；
- 实现代理开启新会话；
- 工作中断后重新开始；
- 分支、设计文档或对话记录之间出现冲突；
- 需要判断“做到了什么、下一步做什么、谁负责什么”。

任何恢复工作的 Codex 实例都必须先：

1. 完整阅读本文；
2. 执行 `git status --short --branch`、`git branch -avv`；
3. 核对本文记录的 commit 与当前本地 ref；
4. 只把已经通过 Codex 独立验收的工作标成完成；
5. 若本文与用户在当前对话中的新决定冲突，以用户最新决定为准，并更新本文。

本文不是历史聊天的替代品；它是历史决定的当前、可执行摘要。

## 1. 决策优先级

发生冲突时，按以下顺序解释需求：

1. 用户在当前对话中的最新明确决定；
2. 本文记录且未被撤回的决定；
3. `docs/conversations/2026-08-23-desired-watch-scope-cut-a7d1b790/conversation.md`
   后半段的最终 scope-cut 裁定；
4. 同一 conversation 中 2026-08-20 的九项总表与开工前最终讨论；
5. `docs/design/2026-08-20-review-package-public.md`；
6. `docs/design/2026-08-20-review-package-local.md`；
7. 其余旧设计、旧 checklist 和实现代理与 Codex 的历史建议。

旧设计中的某一段被后来的产品决定撤回后，不得因为代码已经写了一部分而复活。

## 2. 固定角色

### 2.1 用户：产品负责人和人工转发者

用户负责：

- 决定产品行为、范围和优先级；
- 把 Codex 生成的 **实现任务提示** 原样复制给实现代理；
- 实现代理完成后，把实现代理的原始回报复制回 Codex；
- 对真正会改变产品行为的分歧作最终裁定。

用户不负责：

- 自己整理实现代理与 Codex 的技术分歧；
- 从多轮 finding 中判断哪些是阻断；
- 手工拼装任务说明；
- 替 Codex 做代码验收。

### 2.2 Codex：orchestrator、审查者、验收者

Codex 负责：

- 维护唯一需求基线和本文；
- 规划里程碑与依赖；
- 每次同时生成：
  1. **任务包**（给用户阅读）；
  2. **实现任务提示**（可直接复制给实现代理）；
- 实现代理回报后独立检查真实 Git 状态、commit、diff、调用链和测试；
- 一次性汇总完整 findings；
- 接受、拒绝或要求窄修；
- 为窄修再次同时生成修复任务包和实现任务提示；
- 管理合并、迁移、打包和发布门。

除非用户明确改变分工，Codex 不编写产品源码。Codex 可以进行只读审计、运行测试、
创建隔离测试 worktree，并使用审计子代理；这些子代理不得替代实现代理编写产品代码。

### 2.3 实现代理：唯一实现者

实现代理负责：

- 严格按照 Codex Prompt 修改代码、测试和文档；
- 可以在一个里程碑内创建多个逻辑 commit；
- 使用仓库已配置的维护者 author/committer 身份，不在 commit message 自动添加与外部实现代理或其工具
  供应商有关的 `Co-Authored-By` trailer；
- 不自行扩大范围；
- 不自行重开已冻结的产品设计；
- 不因发现非阻断技术债而顺手重构；
- 完成后提供 commit、文件、测试、已知限制和工作树状态。

实现代理不负责最终验收，也不能自行宣布整个 Stage 完成。

## 3. 实际通信链路

当前没有假定 Codex 能直接向实现代理会话发送消息。普通单 lane 固定链路是：

```text
Codex 同时生成「任务包」和「实现任务提示」
        ↓
用户把实现任务提示原样复制给实现代理
        ↓
实现代理编码、测试、提交并返回原始回报
        ↓
用户把实现代理原始回报复制给 Codex
        ↓
Codex 独立审查仓库，不信任摘要
        ↓
通过：更新本文并进入下一里程碑
驳回：一次性生成完整修复任务包 + 修复 Prompt
```

用户无需重新组织、缩写或解释双方的输出。为了保留证据，应尽量原样转发。

`PARALLEL-WAVE-1` 的批准例外是：Codex 一次生成四份自包含 prompt；用户分别复制给四个实现代理；实现代理
各自在 Codex 已建好的隔离 worktree 工作。用户把四份原始回报都交回后，Codex 才按冻结顺序串行集成和
集中验收。角色不变：实现代理仍只实现，Codex 仍是唯一 orchestrator/验收者。

## 4. 任务包与实现任务提示的固定格式

每次开始实现时，Codex 必须在同一条回复中依次给出两部分。

### 4.1 任务包（给用户与 Codex 留档）

必须包含：

1. 里程碑名称和用户可见结果；
2. 当前基线 commit、目标分支和允许修改的范围；
3. 必须完成的行为；
4. 明确不做的内容；
5. 关键不变量与已知反例；
6. 必须新增或修改的测试；
7. 必须运行的验证命令；
8. commit/工作树要求；
9. 停止条件和需要回报给 Codex 的证据；
10. 发布、迁移和兼容性限制。

### 4.2 实现任务提示（可直接复制）

实现任务提示必须自包含，不能要求实现代理阅读 Codex 当前对话才能理解任务。
它必须包含任务包的全部约束，并明确：

- 先检查真实代码再行动；
- 不扩大范围；
- 不修改用户未授权的产品行为；
- 允许在里程碑内连续工作，不必每个小 commit 等待；
- 完成所有定向测试后再运行里程碑级验证；
- 最后按固定回报格式返回；
- 遇到真正需要产品判断的问题时停止，不得擅自拍板。

## 5. 实现代理完成后的固定回报格式

实现代理的回报应至少包含：

```text
1. Outcome：用户现在能做什么
2. Commits：hash + subject
3. Changed files：按生产/测试/文档分类
4. Contract decisions：实现采用的最终语义
5. Tests run：命令 + 精确通过/失败/ignored 数量
6. Negative proof：哪些测试能打挂旧实现或移除修复后的实现
7. Known limitations：未做或仍受门控的内容
8. Git status：是否干净、是否有无关变更
9. Review range：Codex 应审查的 base..head
```

实现代理的“全部完成”“测试全绿”只是待核验声明。Codex 必须自己读取 Git 和代码。

## 6. Codex Stage 级验收方式

Codex 在完整 Stage 边界集中验收，不再把 M2/M3/M4 之类相邻 slice 分别交付、分别审查。

默认循环：

```text
Stage 任务包 → 实现代理内部多 phase 连续实现 → final boundary 集中重门/一次回报
             → Codex 一轮全 Stage 审查
             → 仅有 blockers 时一轮集中修复 → 最终验收
```

只有 `BLOCKER`（审查严重度 P0/P1、产品行为冲突、权威/安全/误退出、迁移/发布阻断或核心假阳性
测试）才增加轮次。这里的严重度 P1/P2 不得与项目工作流 P1/P2 混用。

Codex 必须：

- 固定审查范围 `base..head`；
- 区分实现代理声称运行的测试与 Codex 实际复跑的测试；
- 一轮覆盖完整 diff，不一次只报一个 finding；
- 先报告阻断，再列非阻断技术债；
- 对弱测试检查判别力；
- 不因非阻断问题重开整个架构；
- 普通审查严重度 P2/P3 进入 deferred debt，不生成 repair prompt；
- 允许 `ACCEPTED_WITH_DEFERRED_DEBT`，不要求技术债清零；
- 只有 Stage 用户结果闭合后才标为完成。

## 7. 加速规则

旧模式的主要速度问题是：反复重写设计、过细 dormant 切片、每小改都跑全套门、
finding 一条一条往返、用户手工重组消息。新的固定规则：

1. 以完整**产品 Stage**切片，不以 helper、PR slice 或单一能力切片；
2. 实现代理在 Stage 内按依赖顺序做多个内部 phase/commit，phase 之间不中途 handoff；
3. 每 phase 只跑 focused tests；workspace/frontend/architecture 只在 final boundary 集中运行，失败按固定
   transient/受影响复验协议处理；
4. archive、安全扫描、真机/soak 只在相关 Stage final 边界运行；
5. Codex 一轮覆盖整个 Stage diff，findings 一次性分成 blockers 与 deferred；
6. 默认最多一轮集中修复；普通低严重度 finding 不触发修复轮；
7. 不要求每个小修做 revert negative proof，只证明最高风险不变量；
8. final gate 首次出现 unexplained transient 时先保存完整 command/status/output/log；同一 full gate 最多
   再跑一次、可定位的 failing binary/case 最多再跑两次。不复现即记 deferred observation，禁止 10+ 次
   压测式追查；任一次复现则升级并诊断；
9. 没有新的产品决定或真实阻断时，不再发起大设计循环；
10. 用户只因产品选择、外部权限/部署或不可逆动作被打断。
11. 经用户批准可把互相隔离的完整 Stage lane 从同一 anchor 并行实现；产品依赖顺序改为串行集成顺序，
    全量测试和最终审查只在组合 head 做一次。具体以 `PARALLEL-WAVE-1.md` 为准。

详细 Stage 与分级合同见 `docs/orchestration/STAGE-EXECUTION-PLAN.md`。

## 8. 原始任务为什么出现

对话最初从“是否做浏览器自动注册”开始。调查后确认自动替用户提交 WebReg：

- 违反 Rutgers 明示规则；
- 风险是暂停用户注册资格；
- 真正收益集中在很窄场景。

因此自动注册被明确取消。工作重心改为：

> RBCSP 不漏报、不假报、不在已经失效时仍向用户显示“正在监控”。

总原则：

> 页面显示的状态必须是被验证过的事实。可以暂时做不到，但不能假装已经做到。

## 9. 九项总工作

| 代号 | 工作 | 通俗目标 |
|---|---|---|
| S1 | 快照完整性安全门 | Rutgers 发残缺 Open 名单时先扣住，避免批量假关闭和假警报 |
| S2 | 提醒必达 | 连接、实际监控、声音和通知都可信时才显示绿色；断线后自动恢复 |
| S3 | 轮询节奏与相位 | 先测 Rutgers 真实重建周期，再把查询对齐到新数据出现之后 |
| S4 | SQLite `prepare_cached` | 避免同一 INSERT 上万次重复编译，保持行为不变地缩短提交时间 |
| P1 | 公网会话票续命/换证 | 公网页面挂机、票失效或服务器重启后能后台换票并重连 |
| P2 | 公网上线加固 | 补连接上限、背压、systemd/Caddy、部署与 soak 等重新上线硬门 |
| L1 | 本地刷新/重启恢复 | 持久保存“用户想盯哪些课”，页面回来后恢复实际监控 |
| L2 | 本地页面关闭后退出 | 所有页面离开后显示 60 秒倒计时，没人回来则优雅退出程序 |
| L3 | 本地控制台日志 | 在 exe 控制台显示启动、连接、监控、警报、Gate、倒计时和退出 |

## 10. 历史执行顺序的含义

历史文本写的是：

```text
S1 → S2（顺带 P1/L1/L2/L3）→ P2 → S4 → S3
```

这表示**实施依赖和当时的代码接触面**，不是说 P1/L1/L2/L3 是 S2 的小功能，
也不是说完成 S2 就自动完成它们。

- S1 先做：后续所有监控和提醒必须建立在可信 Open 数据上；
- S2 是连接/监控/提醒生命周期的共享主干；
- P1 依赖 S2 的公网重连，所以当时准备同阶段修改；
- L1 依赖 S2 的 re-arm/materialization，所以当时准备同阶段修改；
- L2 依赖页面连接生命周期与额外 WebSocket route seam；
- L3 需要记录 S2/L1/L2 产生的生命周期事件，同行实现最省重复改动；
- P2 在 S2/P1 的会话与连接模型稳定后做；
- S4 独立、低风险；
- S3 必须等真实周期/相位数据，所以最后做。

“顺带”只是历史上的**同阶段打包计划**。这个词容易让人误以为它们是免费的、
可选的或已经完成，今后不再用这个词报告进度。

## 11. 后半段对 S2/L1 的最终范围修正

### 11.1 保留

- 多页面共享服务端同一份 desired 状态；
- 页面刷新后读取最新状态；
- generation/revision/CAS；
- tombstone，防止延迟 START 复活已取消意图；
- receipt，确保同一 mutation 重试时重放原答案；
- 9 门 post-state 上限；
- tombstone/receipt 硬帽与 rotation；
- materialization epoch；
- 程序按 desired 装配真实监控；
- 多个页面同时存在时，每个 section 只有一份物理 watch；
- 告警可扇出到全部页面，因此多个页面都可能响；
- 最后页面离开后拆除物理 watch，但保留 desired；
- 页面回来时重新装配。

### 11.2 取消

- desired 专用 WebSocket；
- desired FULL/DELTA 投影帧；
- 分块状态机；
- effect batch 与双 ACK；
- desired audience/listener registry；
- BroadcastChannel 实时镜像；
- Web Locks/leader tab；
- 只让一个页面响铃的 leader 选举；
- 其他页面无需刷新即可实时看到修改。

### 11.3 最终传输和写入规则

```text
GET /api/v1/local/desired-watch
PUT /api/v1/local/desired-watch
```

- desired 修改走本地 HTTP；
- 告警继续走 `/api/v1/watch`；
- L2 presence 是另一件事，仍可使用 local-only WebSocket；
- CAS 不检查 Catalog、term、campus 或 Open Sections；
- CAS 只执行版本、幂等、资源预算和最多 9 门等意图规则；
- Catalog/term/campus 检查在物理装配阶段进行；
- 装配失败保留 desired，在 UI 显示原因，由用户决定是否删除；
- 服务端不得因装配失败擅自撤销用户意图。

### 11.4 已被覆盖的旧文档内容

恢复上下文时不得把下列旧文本重新当成当前合同：

- `docs/design/2026-08-20-review-package-local.md` 中的 Web Locks leader、
  BroadcastChannel 实时镜像与“只有 leader 响铃”；
- `docs/design/2026-08-20-pr-acceptance-checklist.md` 中依赖 leader 重水合的旧第 7 条；
- feature 分支 `docs/design/2026-08-23-desired-watch-reduced-scope.md` 中仍残留的
  Catalog/term/campus 写入准入、`REJECTED/UNAVAILABLE` 和永久失败自动 retirement；
- 任何要求普通 GET `/api/v1/local/desired-watch` 永久返回 404 的旧测试描述。

这些内容已在对应 Stage 的实现与设计文档中同步；恢复上下文时仍以这里记录的最终裁定为准。

## 12. 当前仓库检查点

记录日期：2026-09-02。

```text
当前检出：main（v0.1.4 发布后的编排文档收口）
v0.1.4 产品源码与轻量 tag：379d262da288c0d947629f16e6dbc804c451a17c
GitHub Release：https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases/tag/v0.1.4
v0.1.3 产品源码与轻量 tag：f4117b521aff452717e1b9dde3534d4571b38b76
v0.1.3 Release：https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases/tag/v0.1.3
v0.1.2 产品源码与轻量 tag：76316dd2eedae844bd48fabc75c6b48e343c6d61
v0.1.1 产品源码与轻量 tag：0988dadeeef2db16bbc2e64bc432125674c60325
S1 分支：feat/s1-snapshot-gate@a4b35bc（已合入 main）
M0-M1 实现基线：feat/s2-alert-delivery@a4f8d22
L1/R4 accepted head：75cefb0
实现代理的 Stage 2 主交付 head：553371f；R1 lane head：5af49d9
Codex Stage 2 最终集成 head：6a35c74
Stage 2/3/4/5：均已裁定、串行集成并随 v0.1.1 发布
v0.1.2：真实使用暴露的筛选、可用性、存储与界面缺陷收口，迁移 0007 + derivation stamp 不可回滚
v0.1.3：BY_ARRANGEMENT 同步性取值与上课地点单一语义，迁移 0008 不可回滚
v0.1.4：本地桌面版监看上限 255、批量 desired/telemetry/revalidation 与选择持久化收口；公网/Linux 仍为 9
当前产品源码工作树：无未提交源码；conversation 归档目录保持未跟踪
v0.1.0/v0.1.1/v0.1.2/v0.1.3：tag/Release/资产保持不可变；v0.1.4 为当前 Latest
```

恢复时必须重新核实这些值，不得永久假设它们仍然成立。

## 13. 当前完成度

| 工作 | 状态 | 当前事实 |
|---|---|---|
| S1 | `ACCEPTED`，已合入并随 v0.1.1 发布 | Gate、三条生产路径、迁移、重启、投影、前端均已接线；`3f4ebb0` 删除 probe 正 jitter |
| S2 | `ACCEPTED`，已合入并发布 | R1 四个 blocker 已关闭；reconnect、readiness、audio、notification、presence 与 console 生命周期进入 v0.1.1 |
| S3 | `ACCEPTED_WITH_DEFERRED_DEBT`，已合入 | 离线 analyzer 168/168；CE-15/16 已关、honest GO 保留；当前数据仍 `NO_PRODUCTION_CHANGE / DATA_REQUIRED`，零 production scheduler change |
| S4 | `ACCEPTED`，已合入并发布 | `prepare_cached`、整体回滚与 API 缺席证据成立；architecture 精确冻结 normal cache/dev hooks |
| P1 | 随 Stage 2 `ACCEPTED` 并发布 | mutable in-memory ticket、validate-before-connect、single-flight renew 与页面内 reconnect plan 已通过审查 |
| P2 | `ACCEPTED_WITH_DEFERRED_DEBT`，已合入并发布；CORE evidence PASS | H1–H9 仓内实现、资源边界、Caddy/ACK harness 与真实 600 秒 CORE soak 已收口；生产部署、Rutgers composition 和真实 H8 仍需另行授权 |
| L1 | `ACCEPTED`，已合入并发布 | desired 持久化、GET/PUT、materializer、UI、并发反例与真实包装三生命周期 E2E 均通过 |
| L2 | `ACCEPTED`，已合入并发布 | late HELLO 在 Exiting 后被同锁拒绝；presence/60 秒/ordered shutdown focused tests 通过 |
| L3 | `ACCEPTED`，已合入并发布 | 启动/页面/监控/开放/Gate/倒计时/退出日志已接线 |

## 14. 重要漂移、收口与保留边界

### S1

- `3f4ebb0` 已把 Gate 隔离探测修为严格 `min(30s, watch interval)`，并有可打挂旧 jitter 的反例；
- Codex 在 `107f3e5` 独立复跑相关测试通过；
- N1–N3 三项迁移测试加固是明确非阻断技术债，不得阻塞当前主线。

### S2/L1

- R1–R4 已关闭持续 admission/live drift、rotation/teardown 组合、reset 屏障、terminal/failure stamp、
  真实 batch 单 basis、lost response 撤绿/identity、全局 cutoff、strict codec、五态、late-page active、
  严格 packaging policy 与真实重启物化；
- Codex 已独立验证 `75cefb0`：workspace 746/1 ignored、frontend 254、architecture、三个 diff-check 全绿；
  final-head 12 文件 Windows archive 完成非空物化、两次重启、Full Reset 与再次空重启；
- rotation 现在按 owner live truth 保留 tombstone；STOP committed-before-reconcile + stop fault 时 row、
  pendingDisarm 与原 id 均可达；uncertainty 同步门与 read-only cutoff release 也有真实事件窗口反例；
- `M0-M1-001`、reduced S2-D3 与 L1 已由 Codex 标为 `ACCEPTED`，随后完整 Stage 2 验收、合入并随
  v0.1.1 发布；
- recovery GET 挂起时其他 section 仍使用旧 snapshot 暂为已披露非阻断限制：请求排在同一 queue 后，
  generation/revision CAS 会拒绝 rotation/stale 覆盖，且不会自动重放；
- transport reconnect、公网 nonce 客户端、Readiness、提醒恢复与本地生命周期已在 `STAGE-2` 收口。

### P1/S2

- `75cefb0..553371f` 已实现 shared 退避重连/显式 Disconnect 屏障、公网 mutable ticket +
  validate-before-connect、页面内 reconnect plan、25 秒心跳、authority/connection stamp Readiness、
  AudioContext 自愈、页面 Notification、L2 presence 与 L3 控制台；
- Codex 独立重门和 34/34 security diff inventory 已完成；传统可报告安全 finding 为 0；
- Stage 2 四个阻断已由 `STAGE-2-R1/v2-parallel` 关闭并经 Codex focused tests 验收；
- R1 lane 五个提交已回收为主线 `48bef74..6a35c74`，Stage 2 结论为 ACCEPTED；
- 公网 H4 全局容量和字节背压随后在 Stage 3/P2 完成并随 v0.1.1 发布。

### P2/L2/L3/S3/S4

- P2 final lane `c2e2d2e`、S4 `69c7cb1`、S3 evidence `1dc2219` 已按 B → C → D 回收到
  `feat/s2-alert-delivery`；`5efeaa5` 只把 Stage 2 的两处 late-HELLO 测试机械适配为 `OutboundSender`，锁内
  Exiting guard 与 Notification CI 均保留；
- P2 R3 的 Caddy handler/listener、SSH actual tuple 与单-socket admissions 主合同成立；production jq 的
  27 个 adapted fixtures 已在 Ubuntu CI 真实执行；Linux systemd/Caddy/Chromium 600 秒 CORE soak 通过；
  composition 与真实 H8 tuple 仍是部署期外部门，未开启 Vultr、未部署；
- 原 metric baseline 夹缝在 release 前的 `0988dad` 窄修关闭：基线顺序固定为 admissions → admitted
  permit gauge → ACK，全窗口采样同一 admitted permit gauge；
- S4 production raw SQLite accessor 已删除；statement cache 与整体事务回滚通过，新增依赖/feature 已由
  architecture graph 精确冻结；
- S3 analyzer 只留在 `tools/s3/**`，不会进入 runtime/package。168/168、五类 honest GO 与当前数据
  byte-stability 通过；当前结论仍是不改生产参数。完全不重叠分割、逐样本不规则 jitter、从零伪造及若干
  文案/测试辅助问题保留为明确 deferred；
- L2/L3 随 Stage 2 accepted 并在组合 workspace/frontend 门中回归通过。

## 15. 发布和迁移硬门

1. 本地个人状态迁移现已推进到 10006（含 10004/10005/10006）；一旦数据库升级，旧二进制会因未知迁移拒绝启动；
2. 因此不能发布“只有迁移和地基、没有 writer/UI/E2E”的本地中间构建；
3. 本地发布前必须完成 desired writer、materializer、UI、重启/reset E2E 和回滚说明；
4. 公网发布前必须完成 P1 客户端和 H4 资源边界；
5. 真正重新上线还必须完成 P2 的配置项、真实组装测试和 10 分钟 Caddy+浏览器 soak；
6. “已合入 main”不等于“已发布”；发布状态必须由候选 manifest 的 sourceCommit 证明。

## 16. 当前 Parallel Wave

状态：**Parallel Wave 1 与 v0.1.1–v0.1.4 发布均已完成。v0.1.4 收口了本地大批量监看时暴露的
选择持久化、请求放大和服务锁竞争。Stage 2 → P2 → S4 → S3 evidence 已串行回收；
Windows/Linux 同源归档、联合验证与 Linux 600 秒 CORE soak 全部通过。**

Stage 2（已完成）：**S2 提醒生命周期完整收口（同时完成 P1、守住 L1、完成 L2/L3 与通知政策修订）。**

Stage 2 主体已经实现的用户结果：

- 意外断线、服务重启或长时间离线后，仍存活的页面自动退避重连；
- 显式 Disconnect 绝不被自动连回，只有用户通过现有连接交互明确恢复；不新增独立 Resume 能力；
- 公网页面每次连接前 single-flight validate 当前 nonce，失效时原子换新并只用新票握手；
- 公网页面按页面内 connection intent 重建监控，不从 selection 猜测，也不复活已 STOP 的 section；
- 本地重连继续由服务端 desired coordinator 物化，页面不得发送 legacy START；
- 五环 Readiness、AudioContext 自愈与当前页面级 Notification 已接线，音频真值的假绿阻断已关闭；
- 本地 presence 证明任何浏览 tab 存活；最后页面离开 60 秒后复验再退出；pre-admitted late HELLO 在
  Exiting 后由同一把锁拒绝；
- 控制台显示关键生命周期事件且不泄漏 nonce/session/请求体。

Stage 2 R1 已关闭：音频输出证据/held resume 撤绿、Exiting 后 late HELLO、真实通知开关、frontend CI
与 release-set/CI-entry non-archive policy 证据。Codex 独立 focused tests 全绿，五个提交已串行回收。

R1 明确不包含：

- H4/完整 P2；
- S3/S4。

最终 lane 裁定：

- Stage 3/P2 R3：`ACCEPTED_WITH_DEFERRED_DEBT`；Linux CORE evidence PASS，部署期 composition/H8 pending；
- Stage 4/S4 R1：`ACCEPTED`；
- Stage 5/S3 evidence R3：`ACCEPTED_WITH_DEFERRED_DEBT`，结论为
  `NO_PRODUCTION_CHANGE / DATA_REQUIRED`。

最终 source 独立门：workspace 全绿（一个既有 real-browser ignored）；Rust architecture/source boundary 全绿；
frontend guard 92、Vitest 340、typecheck/local/public builds 全绿；S3 168；ops/soak self-test、release-set
12 capability 全绿；preflight 在 Linux CI 为 12 pass/53 refusal、零 skip；正式 600 秒 CORE
soak 59/59 ACK、20 个样本、forced reload 后同一连接继续。

## 17. 用户冻结并已完成的四阶段顺序

Stage 1（S1）与 M0/M1 已完成；用户冻结的四个完整 Stage 也已按下列顺序验收并进入 v0.1.1：

```text
Stage 1 — S1（已完成）

Stage 2 — STAGE-2（已完成）
  S2 + P1 + L1 回归 + L2/L3 + 通知政策修订；一次实现、一次验收

Stage 3 — STAGE-3（已完成；部署期外部门保留）
  只做 H1–H9/完整 P2 公网资源、配置、组装与部署边界；一次实现、一次验收

Stage 4 — STAGE-4（已完成）
  只做 prepare_cached 微优化；保持行为不变，不夹带存储重构

Stage 5 — STAGE-5（已完成；结论为零 production change）
  先取得跨 target/时段证据，再决定是否实现调度器；证据不足可零 production change 结束
```

发布不是额外产品 Stage；四阶段完成后 Codex 已完成一次 v0.1.1 候选/发布门。Vultr 生产部署、真实 H8
主机操作与 Rutgers composition 从未被这次发布授权。完整阶段合同、分级政策与停止条件见
`docs/orchestration/STAGE-EXECUTION-PLAN.md`。

## 18. 不得复活的旧工作模式

- 不让实现代理同时定义需求、扩展架构、实现并自我验收；
- 不让用户自己拼任务 prompt；
- 不每个 helper/测试修复都停下来进行完整往返；
- 不为每个小 commit 重跑重型安全扫描；
- 不先做大量 dormant 地基而长期没有用户可用结果；
- 不用“Stage 快完成”“地基完成”等模糊说法替代真实用户行为；
- 不凭实现代理与 Codex 的摘要判断完成，必须看 commit、diff、调用链和测试；
- 不把 scope cut 已取消的 leader、实时投影或 desired WebSocket 重新加入；
- 不在没有跨 target 数据前实施 S3；
- 不发布迁移已升级但产品路径未闭合的本地构建。

## 19. 变更日志

### 2026-09-03 — 筛选条件按适用组成部分施加约束

- FLT-S04b 只聚合课节的线上/远程组成部分；纯线下课不因同步方式被排除，混合课也不再把
  线下固定时段错误算进线上 `MIXED`；
- FLT-S07 只约束需要到场的实体时段；纯在线课不因所选子校区被排除，混合课的线上时段不参与地点判断；
- 明确不匹配仍优先于缺失证据，`includeIncomplete` 只能接纳真正的 `UNCERTAIN`，不能放行已证明的反例；
- 真实 `92026 / NB` 快照重放用户组合后，完整条件与移除地点均为 18 门课程 / 42 个课节，证明地点条件
  不再错误排除这些纯在线结果；另以真实 Hybrid 和纯线下课节验证两个条件的适用范围；
- 中英文界面补充适用范围和空结果诊断的独立单项测试说明；当前受版本控制的文件中不再保留外部实现
  工具的品牌归因文字，本机未跟踪的工作目录与用户对话归档保持原样；
- workspace、frontend、Clippy、fmt、Rust architecture/self-tests 与真实 SQLite 隔离实例验证通过；本轮不改
  公网运行时的 9 个监看上限，也不发布新版本。

### 2026-09-02 — v0.1.4 已发布，本地 255 个监看与大批量请求放大收口

- 用户在自行把本地可见上限改为 255 后选择 40 个课节，界面显示 40、实际始终无法监看并反复报告
  服务中断。排查确认不是不可克服的机器性能上限，而是三处设计叠加：个人状态表仍以
  `position BETWEEN 0 AND 8` 拒绝第十项以后写入，前端保留乐观状态并吞掉失败；连续 40 次选择形成
  `1 + ... + 40 = 820` 个逐项状态请求；每个请求和周期 revalidation 都重复完整 target projection，
  且 projection 占用 operational SQLite 全局互斥，已取消的浏览器请求仍在 blocking worker 中排队；
- `dc78afb` 将**仅本地桌面版**的 selection/desired/active 上限推进到 255；共享/公网默认值、公共 API、
  公网 UI 与 Linux runtime 的硬上限保持 9。个人迁移 10005 重建 selection 位置约束为 0..254；
  10006 增加 batch receipt。`PUT /api/v1/local/desired-watch/batch` 把一个 1..255 项用户手势放进一个
  `BEGIN IMMEDIATE` 事务，rows/revisions/tombstones/receipt 全成或全退，一次手势只占一条 receipt；
- 新增共享 additive `POST /api/v1/open/batch-status`，前端对 settled gesture 做 300ms trailing debounce，
  按 term/campus 顺序分组请求；materialization 与周期 revalidation 也按 target 批量 admission，一次 target
  只 projection 一次。昂贵 admission 移到 WebSocket 状态锁之外，PING 不再被锁内 projection 卡住；
  running/stopping authority 在 CLOSED/ERROR 时仍可信，只有明确 STOP 才移除；
- selection 持久化改为 single-flight/latest-wins；失败回滚到最后确认状态并显示
  `SELECTION_SAVE_FAILED`。声音次数与有限时长在 Rust 构造/反序列化及 UI 同时限制为 JavaScript safe
  integer，避免“服务端写入成功、客户端永远无法 bootstrap”的毒化状态；最大合法 domain 的 authority
  与 batch response 分别在 384 KiB / 512 KiB 预算内通过；
- 独立门：`cargo test --workspace`、Clippy `-D warnings`、rustfmt、前端 31 文件 474 项 + typecheck +
  local/public build、Rust graph 与 public zero-surface actual/self-test、release contract self-test 全绿。
  Chrome 使用 0.1.3 数据库副本经迁移后从真实课程页选择 40 门，40/40 desired 全部 materialized，
  failure/pending/blocked 均为 0；刷新后 527ms 恢复 40/40。40 门运行期间密集探测 `/service/status` 60 次，
  0 失败，median 3.7ms、p95 897.9ms、max 1039.6ms，应用日志 0 条 error/panic/lock/timeout；
- 首个 Linux iteration run `33711896240` 在 actual architecture gate 抓到
  `bcsp-local-user-state` 新增的 dev dependency 未写入 graph spec；`379d262` 只补记
  `internalDev: ['bcsp-operational-storage']`，`bcsp-server --edges all` 仍不含任何 local-only 包，公网闭包
  仍为 12。新 run `33712248807` 构建/验证通过；
- 正式 source/tag 为 `379d262da288c0d947629f16e6dbc804c451a17c`。Windows 12 文件，SHA-256
  `005b1f5813f87dab4c9e4aa248ad4808c1a673915a9dc07208c6b7aed358f0fe`；Linux 22 文件，SHA-256
  `104064baac82640e568a14d226014d3bf7fdd779ab5dec061472f5d3e7a4e2f9`；联合门核对 172 个共享组件、
  10 个前端组件、12 项共享前端能力和相同 source epoch。GitHub 上传后重新下载，两份哈希保持一致；
- v0.1.4 Release：`https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases/tag/v0.1.4`。
  本轮只发布 Linux 包，不连接/部署 Vultr，不改 DNS/UFW/SSH/Caddy/systemd，不重跑 600 秒公网 soak。
  个人迁移 10005/10006 仍为单向；升级前必须停止 RBCSP 并备份完整 `data` 目录。

### 2026-09-02 — v0.1.3 已发布，按 Rutgers 口径命名"时间另行约定"并收紧上课地点

- 用户逐条推敲 v0.1.2 的筛选语义，提出两点：勾"同步 + 异步"应当就是同步与异步的并集，
  以及"都线下了还有什么未知"。第二问指向真缺陷；
- 依用户要求核对 Rutgers 官方 Schedule of Classes 的实现（`soc_utils.js`/`soc_app.js`）：
  `isByArrangementMeetingTime` 判 `baClassHours == "B"`，`isOnlineOrRemoteMeetingTime` 判
  `campusLocation` 为 `O`/`T`，二者同时成立才显示 "Asynchronous content"，否则显示
  "Hours by arrangement"。本产品的 `by_arrangement` 判据与官方一致，无需改动；缺陷在另一头：
  非线上的那一类被归入 `Synchronicity::Unknown`，界面显示"未知"，比官方口径更少信息；
- 派生 v3 为该类时段引入 `BY_ARRANGEMENT`（reason `OFFICIAL_BY_ARRANGEMENT_ONSITE`）。
  92026/NB 的 4,575 个"线下 · 未知"课节中 4,461 个取得官方说法，114 个（既有固定时段又有
  另行约定时段，或时间字段非法）仍为未知；全库 5,979 个时段带上新 reason。该取值不可勾选，
  在筛选中的行为与原 `UNKNOWN` 完全相同，**不改变任何筛选结果**；
- 四张目录表的 `synchronicity` CHECK 早于该取值，写入直接触发约束失败。迁移 0008 按 0005 的
  十二步形式重建 `catalog_sections`/`catalog_occurrences` 与两张 staging 表，仅放宽该 CHECK。
  迁移账本因此使升级单向：0.1.2 二进制遇到 id=8 会 `UnknownMigration` 失败关闭；
- FLT-S07 原先只要有一次课命中就放行，选 College Avenue 会返回 78 个还需跑另一实体校区的课节。
  改为"所有需要到场的上课都必须在所选地点内"，线上/远程时段因不产生通勤要求而跳过（与 FLT-S06
  跳过异步时段同一原则），完全无需到场的课节按自身地点判定。College Avenue 由 1,592 变为 1,514；
  "子校区匹配方式"下拉随第二种行为一并移除，`mode` 字段保留仅为让已存筛选状态继续反序列化；
- 界面文案纠正：可上课时间说明原先陈述的是与引擎相反的量词；核心课程体系条件摘要用逗号拼接
  含逗号的字典标签（"WCr · Writing and Communication, Revision"）而读作多出一个代码，改为只列代码
  并本地化 ANY/ALL；"完整数据显示"复选框的字面含义与功能相反，改为"包含数据不完整的记录"；
  中文 `混合` 同时用于授课方式 HYBRID 与同步方式 MIXED，拆为"线上线下混合"/"同步异步混合"；
  课程详情接口不接收筛选条件却列出被筛掉的课节并标 MATCH，改为明示本页不应用搜索条件；
- 语义裁定（产品所有者）：相邻时间段不合并、上课时间待定一律放行、学分保持完全包含——
  三项均维持现状，理由记于 `docs/design/2026-09-02-filter-semantics-and-by-arrangement.md`；
- 验证：Rust 全量 852 项、前端 31 文件 459 项、类型检查与双目标构建全过；用户 452 MB 真实数据库
  副本上迁移 0008 + 派生 v3 重算 5.5 秒 / 6 个 target。Windows 发布门 PASS（12 文件、两次重启、
  `NON_EMPTY_MATERIALIZED`），archive `fad1ca8f71c230be4a8e0a0511989381f90393ac4b16ee8e192b06082e1cb477`；
- 发布过程记录一次流程事故：打包门要求检出连未跟踪文件都干净，正确解法是本仓库既有的
  `git worktree add --detach` 分离检出（v0.1.2 即如此）。本轮先尝试临时移走未跟踪的 conversation
  归档，且移动命令把 `--untracked-files=all` 列出的**文件**当作**目录**平铺，导致同名文件互相覆盖。
  已改用分离 worktree 完成打包；6 份归档由仍存的会话转录重建，
  `rbcsp-design-pr-reviews-d0d9c360` 与 `parallel-stage-release-orchestration-ab0cc48a`
  因源转录已不在 `旧实现工具的本地转录目录` 且从未被 git 跟踪而永久丢失。

### 2026-09-02 — v0.1.2 已发布，真实使用缺陷收口

- 用户以 0.1.1 正式包在自己机器上使用后报告三件事：按条件筛选“一节课都没有刷出来”、界面混乱、
  反复出现“无法完成本地更改”。逐条复现并沿用户要求扩查了全部 16 个筛选项的每一个可选值；
- 对照真实 Fall 2026 目录（92026/NB，4,391 门课 / 11,976 个课节）逐项探测发现四个筛选缺陷：
  Rutgers `meetingModeCode 90` 一律被判为 `UNSPECIFIED`，全库只有 1 个 `ASYNC`、0 个 `MIXED`；
  可用时间窗与子校区 `ALL_REQUIRED_MEETINGS` 因 `UNKNOWN_REQUIREDNESS` 永不排除，任何取值都返回全集；
  `AHo/AHp/AHq/AHr/WCd/WCr` 六个核心码因请求端大写、字典端保留原大小写而被判 `INVALID_FILTER_OPTION`；
- 新推导规则：`90` + 已知时间 → `SYNC`（`ONLINE_SCHEDULED_SYNCHRONOUS`），`90` + `baClassHours=B` 且
  无时间 → `ASYNC`（`ONLINE_BY_ARRANGEMENT_ASYNCHRONOUS`），其余保持 `UNSPECIFIED`；可靠 occurrence
  同时出现 `SYNC` 与 `ASYNC` 的 section 归为 `MIXED`。真实数据由此变为 ASYNC 1,152 / SYNC 238 /
  MIXED 29；投影侧强校验会拒绝旧行，因此迁移 0007 增加 per-target `catalog_derivation_state`，
  本地与公网 runtime 在开库后、任何投影之前原地重算并打戳；
- 可用性缺陷：每次 Open 提交都关闭 prepared-serving 准入，实测 100ms 节奏连续请求中约 20% 返回
  `500`（本地 bootstrap）且单次停顿 3.5–4.5 秒。改为有界等待（本地 5s / 公网 2s），超时后返回
  `503 STORAGE_BUSY`；refresh policy 改用独立 personal 连接，不再与服务连接争锁；
- 存储缺陷：`\\?\\` 前缀路径让 SQLite 走 UNC 分支，WAL-index 锁记账竞态泄漏一个读锁，
  `nBackfill` 恒为 0，实测 WAL 一小时到 3 GB、主库 92 分钟从 173 MB 涨到 867 MB；同时 Open 诊断按
  Rutgers 日保留导致当天永不清理。已规范化路径、给所有写连接设 `journal_size_limit`、由 refresh
  runtime 在 executor 之外做检查点、并改为按 attempt 数分批清理；
- 前端：监看台 `/api/v1/open/status` 请求体误带 index 被服务端拒收，且两处读取未落终态标记导致
  资源永远停在“正在读取”；本地个人状态的任何失败（含写入成功后的刷新失败）都显示同一句话；
  搜索页每次启动丢弃已存学期/校区；空结果不解释原因；结果卡片把 64 位 fingerprint、重复三遍的
  课程名、空列与英文 MATCH 徽章摆在最显眼处。以上全部修复；
- 界面按用户裁定整体重做为 quiet catalog 设计系统：56px 单行 app bar、卡片式结果、四组筛选栏
  （常驻已选条件与搜索动作）、完整深色调色板、选项网格铺满整行、内层滚动条悬停才出现；
- 验证：workspace 849 项 / 前端 457 项测试通过，clippy、rustfmt、Rust 与前端架构图、公网零表面门
  全绿；Windows 12 文件包通过桌面/移动真实浏览器验收、非空 desired-watch 与两次重启 + Full Reset；
  Linux 22 文件包由 run `33663375119` 在 ubuntu-24.04 上同源构建并验证；双平台联合 release-set
  验证通过（172 shared components、12 shared frontend capabilities）；
- 用真实 867 MB 的 0.1.1 数据库做升级预演：启动重算 8 秒完成，两分钟内 967 次 bootstrap 全部
  `200`（中位 16ms），WAL 稳定在 11 MB，用户原始筛选条件返回 4 门课；
- Windows `rbcsp-windows-x86_64-0.1.2.zip` 为 12 文件，SHA-256
  `fcb42e7cfb0de2603262f9c3016f1a694c269b82e916c32ea9a4203fdf8dc3df`；Linux
  `rbcsp-linux-x86_64-0.1.2.tar.gz` 为 22 文件，SHA-256
  `a7c2afd625805a161f0d1aa4df99d3356db560fa3cbf26311f5c6ca7251c7516`；正式 GitHub Release 为
  `https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases/tag/v0.1.2`；
- 本轮未运行 600 秒公网 CORE soak（Linux 包由 iteration job 构建并验证），未连接 Vultr，未修改
  DNS/UFW/SSH/Caddy/systemd 生产主机；旧 `v0.1.0`/`v0.1.1` tag、Release 与资产未变；
- 按仓库既定归因要求，本轮 7 个提交仍全部由
  `VVittgenstein <158061732+VVittgenstein@users.noreply.github.com>` author/commit，不带任何
  与外部实现代理或其工具供应商有关的 co-author trailer。

### 2026-08-26 — v0.1.1 已发布，归因与双平台 release 收口

- 以 `v0.1.0` 为不可变边界完成 message-only 历史改写：186 个映射提交中 161 个提交消息移除
  与外部实现代理或其工具供应商有关的 co-author trailer；父拓扑、tree、author/committer 与真实日期逐项一致，
  最终可达历史中相关 attribution 为 0；远端 `main` 与清理候选随后安全前移，未 squash；
- 首个 release-prep head 的 Linux CORE soak 暴露 Ubuntu 24.04 仓库只提供 Caddy 2.6.2，无法解析
  `stream_close_delay`。最终 release source `0988dadeeef2db16bbc2e64bc432125674c60325` 窄修为官方
  Caddy v2.11.4 固定资产 + SHA-256 校验，并把 H9 归属证据改为 admissions baseline → admitted permit
  gauge → ACK baseline、全窗口采样 admitted permit gauge；独立复核无 blocker/major；
- GitHub push contracts、tag contracts、Linux package run
  `33024587187` 与 600 秒 CORE soak `33024588480` 全绿；soak 同一 socket 完成 59/59 ACK、一次
  admission、20 个内存/连接样本，中点 forced reload 后继续，最终标记
  `P2_H9_PUBLIC_SOAK_CORE_PASS`；
- Windows `rbcsp-windows-x86_64-0.1.1.zip` 为 12 文件，SHA-256
  `a3a23bafdcc42dc3f88d6ad90a0775b92bbdf951e0af712648927e5219daaa73`；release verifier 通过
  桌面/移动真实浏览器、非空 desired-watch、两次重启与 Full Reset；
- Linux `rbcsp-linux-x86_64-0.1.1.tar.gz` 为 22 文件，SHA-256
  `77546e4615b148963d2de92f75a0628c3323ae679066b88edfd79ed756b7e2ac`；双平台联合 release-set
  验证通过；正式 GitHub Release 为
  `https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases/tag/v0.1.1`；
- Release 发布后从 GitHub 重新下载两份资产并复算同一哈希；旧 `v0.1.0` tag、Release 359451298、
  正文、资产 ID/大小/时间/哈希全部未变；没有连接 Vultr，没有修改 DNS/UFW/SSH/Caddy/systemd
  生产主机，没有运行需真实 Rutgers 授权的 composition 层或真实 H8 主机操作。

### 2026-08-26 — 授权 v0.1.1 发布与 GitHub 归因收口

- 用户批准把已经集成的四条 lane 作为 `v0.1.1` 同时发布 Windows local 与 Linux public 包；仍不连接或
  部署 Vultr；
- 当前新增的 175 个提交全部由 `VVittgenstein <158061732+VVittgenstein@users.noreply.github.com>`
  author/commit，但 151 个带自动添加的外部实现代理署名 `Co-Authored-By`；已公开 `main` 另有 10 个同类
  trailer；
- 用户明确授权以旧 `v0.1.0` tag 为不可变边界，先备份，再仅删除边界之后与外部实现代理或其工具供应商
  有关的 co-author trailer；每个提交的 tree、父拓扑、author/committer、真实日期和独立粒度必须保留，禁止
  squash 或为贡献日历伪造日期；
- 清理候选必须先跑 GitHub CI；通过后才允许 `force-with-lease` 替换远端 `main`。旧 `v0.1.0` tag、
  Release 和资产保持原样；`v0.1.1` 的 Windows/Linux 包必须从清理后的同一个最终 `main` 提交重建并
  联合验证；
- 发布准备同时修正版本源、根 README、Windows 随包 README/QUICKSTART 和 Linux operator runbook。
  归因改写不改变产品代码或用户行为。

### 2026-08-26 — Parallel Wave 1 收口与 Windows local 候选

- Codex 按冻结顺序把 P2、S4、S3 evidence 集成到 `feat/s2-alert-delivery`，组合产品/发布源码提交为
  `e2f9af31526095178b0e1df5106113261c7de74a`；
- 组合 head 的 workspace、frontend、architecture、public-ops 静态门、release-set self-test 与 S3 analyzer
  168 项测试全部通过；Stage 3/P2 以 `ACCEPTED_WITH_DEFERRED_DEBT + PENDING_LINUX_EVIDENCE`
  收口，Stage 4/S4 接受，Stage 5/S3 evidence 以 `NO_PRODUCTION_CHANGE / DATA_REQUIRED` 收口；
- 从该提交的 clean detached worktree、锁定且离线的工具链构建 Windows local 0.1.0 候选；归档恰好
  12 个文件，SHA-256 为 `ac70b8d8efa4ab43f478fbf0559c71f76550e6229aee3273f0798b7973c75714`；
- release verifier 在 Unicode 路径中通过桌面/移动真实浏览器验收，并完成 desired-watch 非空播种、两次重启、
  同 section 重新物化和 Full Reset 闭环；结果为 `NON_EMPTY_MATERIALIZED`，section
  `92026/NB/10001`；
- 候选只保存在 `.cache/final-release-e2f9af3/release/rbcsp-windows-x86_64-0.1.0.zip`；没有连接
  Vultr、没有部署、没有 tag、没有上传，也没有运行 Linux/systemd/Caddy 600 秒 soak 或真实 H8 主机操作。

### 2026-08-25 — Parallel Wave 1 首轮集中审查与三 lane 窄修

- 四个实现代理分别交付 R1、P2、S4、S3 evidence；分支/head/status 与回报一致；
- Codex 独立复跑 R1 presence 16、frontend focused 81、policy 92、release self-test；结论 ACCEPTED，
  cherry-pick 为 `48bef74..6a35c74`；
- S4 operational-storage 61 项、S3 analyzer 58 项独立全绿，但源码审查发现关键测试/决策门假阳性；
- P2 Codex Security diff scan `9391c9bd-32ce-483c-9fde-d454e11c2826` 覆盖 16/16 inventory，报告 2 项
  high-confidence/low-severity H2/H8 preflight finding，并 deferred unaccounted outbound 的安全严重度；
- 产品验收仍把 unaccounted outbound、H9 不完整 PASS 视为发布 blocker；manual arbitrary SHA/trusted
  candidate、fairness 等低优先项明确不修；
- Stage 4 只移除 production raw SQLite test seam；Stage 5 只收紧四个 false-GO 根因；不重开产品架构，
  不运行 full gates/archive/soak。

### 2026-08-25 — Parallel Wave 1：隔离并行实现、串行集成

- 用户认为按 Stage 依次实现和依次审查仍过慢，批准多个实现代理通过 worktree 并行工作；
- Codex 对 P2、S4、S3 的真实代码路径、参数缺口、外部环境和交叉文件做了并行只读调查；
- 冻结四个 lane：Stage 2 R1、Stage 3/P2、Stage 4/S4、Stage 5/S3 evidence；每个 lane 有独立 branch、
  worktree、任务包与 `ultracode:` prompt；
- P2 新冻结 per-socket 256 KiB、global outbound 64 MiB、write timeout 5s；保留 global 1024、per-client 64、
  per-session 4；本机无法运行 Linux systemd+Caddy 600s H9，因此只允许报告 pending evidence；
- S4 确认唯一热点为 operational storage 成功提交循环，rusqlite 只在该 crate 窄开 cache feature；
- S3 现有 NB 数据下 30s/60s 都约解释 65/67，证据不足，当前只授权离线 analyzer 与
  `NO_PRODUCTION_CHANGE / DATA_REQUIRED`；
- 并行 worktree 只跑 focused tests；workspace/frontend/archive/security/soak 留给 Codex 串行组合 head，
  避免已观测到的 Vitest/Cargo 资源争抢。

### 2026-08-25 — Stage 2 主交付独立验收与 R1 窄修

- 实现代理在 `75cefb0..553371f` 一次实现 Stage 2 主体；Codex 固定同一范围检查 55 个 changed files，
  独立复跑 workspace、frontend、architecture/self-tests、diff-check 与 PowerShell parser；
- final-head Rust、architecture 和串行 frontend verify 全绿；第一次 frontend 与 Rust 并跑时出现 20 个
  Vitest worker 启动超时、无断言 failure，按固定预算串行复跑一次即通过，记环境资源观察；
- Codex Security scan `4b394b71-967d-42bf-b8ac-8186c7cb5d22` 完成 34/34 inventory，封存 0 个可报告
  finding；
- 独立审查确认四个 Stage blocker：audio known-failure 被无证据擦除与 held resume 假绿、presence pre-admitted
  late HELLO 越过 Exiting、生产 UI 缺通知关闭入口、frontend Notification policy tests 未进入 CI 且
  release-set/CI-entry non-archive self-test 不完整；
- 结论为 `CHANGES_REQUIRED`，生成唯一一轮 `STAGE-2-R1/v1`；其余低严重度/防御性候选明确延期，不进入
  P2/S4/S3，不构建 archive、不重复全量安全扫描。

### 2026-08-24 — 改为 Stage 级推进并允许非关键问题延期

- 用户明确撤销 M2、M3、M4 分别实现/分别验收的慢速方式；
- `M2-001` 在尚未实施前标为 `SUPERSEDED BEFORE IMPLEMENTATION`，其内容与原 M3/M4 一并进入
  `STAGE-2`；
- 用户最终冻结余下四个产品 Stage：`S2（P1/L1/L2/L3）→ P2 → S4 → S3`；不再把 P2/S4 合并，
  release 也不冒充第五个产品 Stage；
- 每个 Stage 由 Codex 在真实起点上写一份完整计划，用户一次转发；实现代理使用官方 dynamic workflow
  自主编排内部工作并只在 Stage final 回报一次；
- Prompt 只钉死范围、用户结果、边界、验收场景和 final gates，不预先规定内部类型/文件/agent 数；
- 拒绝过早优化：当前 Stage 之外、无现实路径/失败证据且不影响主合同的问题默认延期；
- 只有会造成错误成功状态、用户失控/意图丢失、安全或资源边界破坏、误退出、迁移/发布危险的
  `BLOCKER` 阻断；普通审查严重度 P2/P3 默认记入 deferred debt，可用
  `ACCEPTED_WITH_DEFERRED_DEBT` 通过；
- 当前授权任务改为 `STAGE-2/v1`，产品代码基线仍为 `75cefb0`。

### 2026-08-24 — M0-M1-001-R4 最终验收

- 实现代理在 `b450ed0..75cefb0` 完成 R4；
- Codex 独立复核 committed-before-reconcile 交错、同一 render turn uncertainty 门与 READ-only cutoff
  release，三个判别测试和 production 机制均成立；
- workspace 746/1 ignored、frontend 254、architecture、diff-check 与 final-head Windows archive 全绿；
- 两次构建获显式豁免：第二次是最终 head 的完整重建，没有污染正式 release；一次未保留日志且未复现的
  workspace HTTP 失败记为 unexplained transient 观察项，不冒充已诊断 flake；
- `M0-M1-001`、reduced S2-D3 与 L1 结论为 `ACCEPTED`；当时生成的 `M2-001` 后因用户改为
  Stage 级推进而在未实施前作废。

### 2026-08-24 — M0-M1-001-R3 独立验收

- 实现代理在 `2cd54e2..b450ed0` 完成 R3；
- Codex 独立复跑 workspace/frontend/architecture/diff-check 与最终 Windows archive，普通门及非空
  三生命周期 E2E 全部通过；
- 安全差异扫描 `27364bc1-56fd-4f0e-9a3b-b14e69c6c7c7` 覆盖 7/7 authoritative production files，
  传统可报告 finding 为 0；
- 新组合交错发现 STOP committed-before-reconcile × rotation，且确认 `b450ed0` 声称的同步 uncertainty
  map 修复没有进入 production；另有 PUT 解除 cutoff 的窄合同差异；
- R3 仍为 `CHANGES_REQUIRED`，生成一次性 `M0-M1-001-R4`，不扩展到 M2。

### 2026-08-24 — M0-M1-001-R2 独立验收

- 实现代理在 `c50499f..2cd54e2` 完成 R2；
- Codex 独立复跑 workspace/frontend/architecture/diff-check 与真实 Windows archive，普通门均通过；
- 安全差异扫描 `39c6bf4b-fda3-46a7-afe7-05394bd6921f` 覆盖 9/9 authoritative files，
  传统可报告 finding 为 0；
- 组合反例发现 STOP×rotation、stale failure、真实批量 gesture、uncertain mutation 与 close/read race；
- R2 仍为 `CHANGES_REQUIRED`，生成一次性 `M0-M1-001-R3`，不扩展到 M2。

### 2026-08-24 — M0-M1-001-R1 独立验收

- 实现代理在 `107f3e5..c50499f` 完成 R1；
- Codex 独立复跑 workspace/frontend/architecture/diff-check，并复验真实 Windows archive：
  12 文件、两次重启、desired 成功 attach/materialize、reset 后为空；
- 完成 16/16 authoritative changed files 的安全差异扫描
  `14918661-76ef-4f40-847b-2f8addc08ec7`，传统可报告 finding 为 0；
- 通过未覆盖组合反例发现 teardown identity、reset fault、terminal restamp、前端 gesture snapshot、
  immediate fail-closed、saved-row identity、五态标签与验收门判别力残余；
- R1 结论为 `CHANGES_REQUIRED`，生成一次性 `M0-M1-001-R2`，不扩展到 M2。

### 2026-08-24 — M0-M1-001 独立验收

- 实现代理在 `a4f8d22..107f3e5` 落地 S1 窄修和 desired-watch 纵向实现；
- Codex 独立复跑 workspace、frontend 和 architecture 门；
- Codex 使用本机已有锁定工具真实构建并验证 Windows 候选，确认当前脚本能通过但没有证明重启物化；
- 完成 28 个自动 inventory 文件加 1 个 binary-classified TypeScript 源码的安全差异扫描；
- 安全扫描封存为 0 个传统可报告漏洞；4 个同用户/本地正确性候选被安全策略排除，仍作为产品阻断；
- 验收结论为 `CHANGES_REQUIRED`，生成 `M0-M1-001-R1` 集中修复任务。

### 2026-08-23 — 初始版本

- 记录 Codex orchestrator / 用户转发 / 外部实现代理（implementer）的真实通信方式；
- 解释九项总案与历史执行顺序；
- 合并后半段 desired-watch scope cut；
- 记录 `main@ae65958` 与 `feat/s2-alert-delivery@a4f8d22` 检查点；
- 记录九项完成度、迁移/发布门和下一里程碑建议；
- 尚未向实现代理发出正式实现任务。

## 20. 新工作模式任务账本

此表只记录采用本文协作协议之后的任务。每次 Codex 发出任务包、收到实现代理回报、
作出验收结论或批准进入下一里程碑时都必须更新。

```text
Active wave: PARALLEL-WAVE-1 COMPLETE
Accepted/integrated: Stage 2; STAGE-3-R3/P2; STAGE-4-R1/S4; STAGE-5-R3/S3 evidence
Published v0.1.1 product source/tag: 0988dadeeef2db16bbc2e64bc432125674c60325
Published v0.1.4 product source/tag: 379d262da288c0d947629f16e6dbc804c451a17c
Integration order completed: P2 → S4 → S3 evidence
Integration-only commits: 5efeaa5 (presence sender); 882f230 (architecture feature snapshot)
Heavy gates: final source push/tag contracts, Windows verifier, Linux verifier, joint release-set and 600s CORE soak PASS
Windows v0.1.1 release: 12 files; SHA256 a3a23bafdcc42dc3f88d6ad90a0775b92bbdf951e0af712648927e5219daaa73; NON_EMPTY_MATERIALIZED after two restarts
Linux v0.1.1 release: 22 files; SHA256 77546e4615b148963d2de92f75a0628c3323ae679066b88edfd79ed756b7e2ac
Windows v0.1.4 release: 12 files; SHA256 005b1f5813f87dab4c9e4aa248ad4808c1a673915a9dc07208c6b7aed358f0fe; Chrome 40/40 and packaged NON_EMPTY_MATERIALIZED PASS
Linux v0.1.4 release: 22 files; SHA256 104064baac82640e568a14d226014d3bf7fdd779ab5dec061472f5d3e7a4e2f9; public watch limit remains 9
External gates: P2 Linux/systemd/Caddy 600s CORE PASS; full Rutgers composition and real H8 deployment remain separately authorized
S3 production verdict remains: NO_PRODUCTION_CHANGE / DATA_REQUIRED
Codex current verdict: Stage 2 ACCEPTED; Stage 3/P2 ACCEPTED_WITH_DEFERRED_DEBT (CORE evidence PASS; deployment-only evidence pending); Stage 4 ACCEPTED; Stage 5 ACCEPTED_WITH_DEFERRED_DEBT / NO_PRODUCTION_CHANGE
Prior milestone: M0-M1-001-R4/v1 at 75cefb0 — ACCEPTED
Superseded task: M2-001/v1 — SUPERSEDED BEFORE IMPLEMENTATION
Next authorized action: NONE — scoped filter fix integrated after validation; a new release, production deployment, or Rutgers composition requires new explicit authorization
```

验收结论只允许使用：

- `ACCEPTED`
- `ACCEPTED_WITH_DEFERRED_DEBT`
- `CHANGES_REQUIRED`
- `BLOCKED`（仅真实外部/产品决策阻断）
- `NOT_STARTED`
- `IN_PROGRESS`
- `PENDING_REVIEW`

未经 Codex 更新本表为 `ACCEPTED` 或 `ACCEPTED_WITH_DEFERRED_DEBT`，实现代理的完成声明不得推进 Stage 状态。
