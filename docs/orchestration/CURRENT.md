# RBCSP 当前工作总账与 Codex–Claude 协作协议

状态：**ACTIVE — 当前唯一的工作恢复入口**
最后更新：2026-08-24（America/New_York）
维护者：Codex（orchestrator）
产品决策者：用户
实现者：Claude

## 0. 本文用途

本文用于在以下情况发生后恢复稳定上下文：

- Codex 对话被多轮压缩；
- Claude 开启新会话；
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
7. 其余旧设计、旧 checklist 和 Claude/Codex 的历史建议。

旧设计中的某一段被后来的产品决定撤回后，不得因为代码已经写了一部分而复活。

## 2. 固定角色

### 2.1 用户：产品负责人和人工转发者

用户负责：

- 决定产品行为、范围和优先级；
- 把 Codex 生成的 **Claude Prompt** 原样复制给 Claude；
- Claude 完成后，把 Claude 的原始回报复制回 Codex；
- 对真正会改变产品行为的分歧作最终裁定。

用户不负责：

- 自己整理 Claude 与 Codex 的技术分歧；
- 从多轮 finding 中判断哪些是阻断；
- 手工拼装任务说明；
- 替 Codex 做代码验收。

### 2.2 Codex：orchestrator、审查者、验收者

Codex 负责：

- 维护唯一需求基线和本文；
- 规划里程碑与依赖；
- 每次同时生成：
  1. **任务包**（给用户阅读）；
  2. **Claude Prompt**（可直接复制给 Claude）；
- Claude 回报后独立检查真实 Git 状态、commit、diff、调用链和测试；
- 一次性汇总完整 findings；
- 接受、拒绝或要求窄修；
- 为窄修再次同时生成修复任务包和 Claude Prompt；
- 管理合并、迁移、打包和发布门。

除非用户明确改变分工，Codex 不编写产品源码。Codex 可以进行只读审计、运行测试、
创建隔离测试 worktree，并使用审计子代理；这些子代理不得替代 Claude 编写产品代码。

### 2.3 Claude：唯一实现者

Claude 负责：

- 严格按照 Codex Prompt 修改代码、测试和文档；
- 可以在一个里程碑内创建多个逻辑 commit；
- 不自行扩大范围；
- 不自行重开已冻结的产品设计；
- 不因发现非阻断技术债而顺手重构；
- 完成后提供 commit、文件、测试、已知限制和工作树状态。

Claude 不负责最终验收，也不能自行宣布整个 Stage 完成。

## 3. 实际通信链路

当前没有假定 Codex 能直接向 Claude 会话发送消息。固定链路是：

```text
Codex 同时生成「任务包」和「Claude Prompt」
        ↓
用户把 Claude Prompt 原样复制给 Claude
        ↓
Claude 编码、测试、提交并返回原始回报
        ↓
用户把 Claude 原始回报复制给 Codex
        ↓
Codex 独立审查仓库，不信任摘要
        ↓
通过：更新本文并进入下一里程碑
驳回：一次性生成完整修复任务包 + 修复 Prompt
```

用户无需重新组织、缩写或解释双方的输出。为了保留证据，应尽量原样转发。

## 4. 任务包与 Claude Prompt 的固定格式

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

### 4.2 Claude Prompt（可直接复制）

Claude Prompt 必须自包含，不能要求 Claude 阅读 Codex 当前对话才能理解任务。
它必须包含任务包的全部约束，并明确：

- 先检查真实代码再行动；
- 不扩大范围；
- 不修改用户未授权的产品行为；
- 允许在里程碑内连续工作，不必每个小 commit 等待；
- 完成所有定向测试后再运行里程碑级验证；
- 最后按固定回报格式返回；
- 遇到真正需要产品判断的问题时停止，不得擅自拍板。

## 5. Claude 完成后的固定回报格式

Claude 的回报应至少包含：

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

Claude 的“全部完成”“测试全绿”只是待核验声明。Codex 必须自己读取 Git 和代码。

## 6. Codex Stage 级验收方式

Codex 在完整 Stage 边界集中验收，不再把 M2/M3/M4 之类相邻 slice 分别交付、分别审查。

默认循环：

```text
Stage 任务包 → Claude 内部多 phase 连续实现 → final boundary 集中重门/一次回报
             → Codex 一轮全 Stage 审查
             → 仅有 blockers 时一轮集中修复 → 最终验收
```

只有 `BLOCKER`（审查严重度 P0/P1、产品行为冲突、权威/安全/误退出、迁移/发布阻断或核心假阳性
测试）才增加轮次。这里的严重度 P1/P2 不得与项目工作流 P1/P2 混用。

Codex 必须：

- 固定审查范围 `base..head`；
- 区分 Claude 声称运行的测试与 Codex 实际复跑的测试；
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
2. Claude 在 Stage 内按依赖顺序做多个内部 phase/commit，phase 之间不中途 handoff；
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

这些内容尚需由 Claude 在正式实现任务中同步修改，但其产品语义已经被用户的最终裁定覆盖。

## 12. 当前仓库检查点

记录日期：2026-08-24。

```text
当前检出：feat/s2-alert-delivery@75cefb0
origin/main：ae65958
S1 分支：feat/s1-snapshot-gate@a4b35bc（已合入 main）
M0-M1 实现基线：feat/s2-alert-delivery@a4f8d22
Claude R4 报告 head：75cefb0
Codex R4 审查范围：b450ed0..75cefb0（全里程碑 a4f8d22..75cefb0）
S2 分支：尚未合并、尚未发布
当前产品源码工作树：无未提交源码；两个 conversation 归档目录未跟踪
Codex orchestration 文档：已记录 R4 ACCEPTED；旧 M2 未实施即作废；当前任务为 STAGE-2
现有 release/0.1.0：sourceCommit=7d5debef（2026-07-15，早于本轮工作）
```

恢复时必须重新核实这些值，不得永久假设它们仍然成立。

## 13. 当前完成度

| 工作 | 状态 | 当前事实 |
|---|---|---|
| S1 | 代码完成并已合入 main；M0 窄修已写入 feature | Gate、三条生产路径、迁移、重启、投影、前端均已接线；`3f4ebb0` 删除 probe 正 jitter；尚无新 release |
| S2 | reduced desired/L1 纵向闭环已验收，仅 feature；完整 S2 未完成 | M1 已证明持久意图、真实物化、重启/reset 与组合并发；自动重连、完整 Readiness、音频、通知仍未做 |
| S3 | 未开始正式实现 | 没有 GridAnchor/RebuildProfile；仅有不足以冻结参数的单校区数据 |
| S4 | 未开始 | 代码中没有 `prepare_cached` |
| P1 | 服务端半边完成，仅 feature；Stage S2 客户端待做 | validate、reserve lease、session 保护完成；公网尚无 mutable nonce holder/validate-before-connect |
| P2 | 大部分未开始 | H5 基本完成、H4 只有 per-session cap；其余重新上线门未完成 |
| L1 | `ACCEPTED`，仅 feature | desired 持久化、GET/PUT、materializer、UI、并发反例与真实包装三生命周期 E2E 均通过；尚未合并/发布 |
| L2 | 只有通用 route seam | 无 presence、60 秒状态机或自动退出 |
| L3 | 未开始 | 无可见业务日志与控制台倒计时 |

## 14. 已发现但尚未处理的关键漂移

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
- `M0-M1-001`、reduced S2-D3 与 L1 已由 Codex 标为 `ACCEPTED`，本地迁移的纵向行为门关闭；这不等于
  已合并/发布，也不等于完整 S2；
- recovery GET 挂起时其他 section 仍使用旧 snapshot 暂为已披露非阻断限制：请求排在同一 queue 后，
  generation/revision CAS 会拒绝 rotation/stale 覆盖，且不会自动重放；
- 下一阶段不再回修 desired；一次完成 `STAGE-2` 中的 transport reconnect、公网 nonce
  客户端、Readiness、提醒恢复与本地生命周期收口。

### P1/S2

- 公网前端没有 mutable nonce holder；
- 没有调用 `/api/v1/session/validate`；
- 没有共享自动重连/userStopped/退避状态机；公网也没有与 selection 分离的页面内 reconnect plan；
- 心跳未进入 25 秒 Readiness；
- AudioContext 自愈和页面级 Notification 尚未实现；
- 公网 H4 全局容量和字节背压仍是部署阻断。

### P2/L2/L3/S3/S4

- P2 除 H5 与 per-session cap 外基本未开始；
- L2/L3 未开始产品实现；
- S4 为 0%；
- S3 不得在只有 NB 单时段证据时直接修改生产周期。

## 15. 发布和迁移硬门

1. feature 包含本地迁移 10004；一旦数据库升级，旧二进制会因未知迁移拒绝启动；
2. 因此不能发布“只有迁移和地基、没有 writer/UI/E2E”的本地中间构建；
3. 本地发布前必须完成 desired writer、materializer、UI、重启/reset E2E 和回滚说明；
4. 公网发布前必须完成 P1 客户端和 H4 资源边界；
5. 真正重新上线还必须完成 P2 的配置项、真实组装测试和 10 分钟 Caddy+浏览器 soak；
6. “已合入 main”不等于“已发布”；发布状态必须由候选 manifest 的 sourceCommit 证明。

## 16. 当前 Stage

状态：**用户已撤销 M2/M3/M4 分别交付；`M2-001` 未实施即作废并并入
`STAGE-2`，等待用户一次转发给 Claude。**

当前 Stage 2：**S2 提醒生命周期完整收口（同时完成 P1、守住 L1、完成 L2/L3 与通知政策修订）。**

用户可见目标：

- 意外断线、服务重启或长时间离线后，仍存活的页面自动退避重连；
- 显式 Disconnect 绝不被自动连回，用户再次 Start 或明确 Resume 后才恢复；
- 公网页面每次连接前 single-flight validate 当前 nonce，失效时原子换新并只用新票握手；
- 公网页面按页面内 connection intent 重建监控，不从 selection 猜测，也不复活已 STOP 的 section；
- 本地重连继续由服务端 desired coordinator 物化，页面不得发送 legacy START；
- 五环 Readiness、AudioContext 自愈与当前页面级 Notification 一次完成；
- 本地 presence 证明任何浏览 tab 存活；最后页面离开 60 秒后复验再退出；
- 控制台显示关键生命周期事件且不泄漏 nonce/session/请求体。

Claude 使用一个官方 dynamic workflow 自行规划内部 phase、subagent、文件组织和 focused tests。Codex
只冻结用户结果、协议/政策边界、验收场景和 final gates；并行 agents 默认只读，当前权威 checkout 的
产品写入与 Git 提交由单一 integrator 串行完成，不得以“动态”为名扩展 Stage。

建议不包含：

- H4/完整 P2；
- S3/S4。

Stage 通过只代表 S2/P1 client/L2/L3 功能完成；公网 deployment 仍由 H4/P2 阻断。

## 17. 用户冻结的余下四阶段顺序

Stage 1（S1）已完成；M0/M1 也已验收。余下产品工作只允许按以下四个完整 Stage 推进：

```text
Stage 1 — S1（已完成）

Stage 2 — STAGE-2
  S2 + P1 + L1 回归 + L2/L3 + 通知政策修订；一次实现、一次验收

Stage 3 — STAGE-3
  只做 H1–H9/完整 P2 公网资源、配置、组装与部署边界；一次实现、一次验收

Stage 4 — STAGE-4
  只做 prepare_cached 微优化；保持行为不变，不夹带存储重构

Stage 5 — STAGE-5
  先取得跨 target/时段证据，再决定是否实现调度器；证据不足可零 production change 结束
```

发布不是额外产品 Stage；余下四阶段完成后再由 Codex 准备一次候选/发布门，并另行取得外部动作授权。
当前只授权 Stage 2。完整阶段合同、分级政策与停止条件见
`docs/orchestration/STAGE-EXECUTION-PLAN.md`。

## 18. 不得复活的旧工作模式

- 不让 Claude 同时定义需求、扩展架构、实现并自我验收；
- 不让用户自己拼任务 prompt；
- 不每个 helper/测试修复都停下来进行完整往返；
- 不为每个小 commit 重跑重型安全扫描；
- 不先做大量 dormant 地基而长期没有用户可用结果；
- 不用“Stage 快完成”“地基完成”等模糊说法替代真实用户行为；
- 不凭 Claude/Codex 的摘要判断完成，必须看 commit、diff、调用链和测试；
- 不把 scope cut 已取消的 leader、实时投影或 desired WebSocket 重新加入；
- 不在没有跨 target 数据前实施 S3；
- 不发布迁移已升级但产品路径未闭合的本地构建。

## 19. 变更日志

### 2026-08-24 — 改为 Stage 级推进并允许非关键问题延期

- 用户明确撤销 M2、M3、M4 分别实现/分别验收的慢速方式；
- `M2-001` 在尚未实施前标为 `SUPERSEDED BEFORE IMPLEMENTATION`，其内容与原 M3/M4 一并进入
  `STAGE-2`；
- 用户最终冻结余下四个产品 Stage：`S2（P1/L1/L2/L3）→ P2 → S4 → S3`；不再把 P2/S4 合并，
  release 也不冒充第五个产品 Stage；
- 每个 Stage 由 Codex 在真实起点上写一份完整计划，用户一次转发；Claude 使用官方 dynamic workflow
  自主编排内部工作并只在 Stage final 回报一次；
- Prompt 只钉死范围、用户结果、边界、验收场景和 final gates，不预先规定内部类型/文件/agent 数；
- 拒绝过早优化：当前 Stage 之外、无现实路径/失败证据且不影响主合同的问题默认延期；
- 只有会造成错误成功状态、用户失控/意图丢失、安全或资源边界破坏、误退出、迁移/发布危险的
  `BLOCKER` 阻断；普通审查严重度 P2/P3 默认记入 deferred debt，可用
  `ACCEPTED_WITH_DEFERRED_DEBT` 通过；
- 当前授权任务改为 `STAGE-2/v1`，产品代码基线仍为 `75cefb0`。

### 2026-08-24 — M0-M1-001-R4 最终验收

- Claude 在 `b450ed0..75cefb0` 完成 R4；
- Codex 独立复核 committed-before-reconcile 交错、同一 render turn uncertainty 门与 READ-only cutoff
  release，三个判别测试和 production 机制均成立；
- workspace 746/1 ignored、frontend 254、architecture、diff-check 与 final-head Windows archive 全绿；
- 两次构建获显式豁免：第二次是最终 head 的完整重建，没有污染正式 release；一次未保留日志且未复现的
  workspace HTTP 失败记为 unexplained transient 观察项，不冒充已诊断 flake；
- `M0-M1-001`、reduced S2-D3 与 L1 结论为 `ACCEPTED`；当时生成的 `M2-001` 后因用户改为
  Stage 级推进而在未实施前作废。

### 2026-08-24 — M0-M1-001-R3 独立验收

- Claude 在 `2cd54e2..b450ed0` 完成 R3；
- Codex 独立复跑 workspace/frontend/architecture/diff-check 与最终 Windows archive，普通门及非空
  三生命周期 E2E 全部通过；
- 安全差异扫描 `27364bc1-56fd-4f0e-9a3b-b14e69c6c7c7` 覆盖 7/7 authoritative production files，
  传统可报告 finding 为 0；
- 新组合交错发现 STOP committed-before-reconcile × rotation，且确认 `b450ed0` 声称的同步 uncertainty
  map 修复没有进入 production；另有 PUT 解除 cutoff 的窄合同差异；
- R3 仍为 `CHANGES_REQUIRED`，生成一次性 `M0-M1-001-R4`，不扩展到 M2。

### 2026-08-24 — M0-M1-001-R2 独立验收

- Claude 在 `c50499f..2cd54e2` 完成 R2；
- Codex 独立复跑 workspace/frontend/architecture/diff-check 与真实 Windows archive，普通门均通过；
- 安全差异扫描 `39c6bf4b-fda3-46a7-afe7-05394bd6921f` 覆盖 9/9 authoritative files，
  传统可报告 finding 为 0；
- 组合反例发现 STOP×rotation、stale failure、真实批量 gesture、uncertain mutation 与 close/read race；
- R2 仍为 `CHANGES_REQUIRED`，生成一次性 `M0-M1-001-R3`，不扩展到 M2。

### 2026-08-24 — M0-M1-001-R1 独立验收

- Claude 在 `107f3e5..c50499f` 完成 R1；
- Codex 独立复跑 workspace/frontend/architecture/diff-check，并复验真实 Windows archive：
  12 文件、两次重启、desired 成功 attach/materialize、reset 后为空；
- 完成 16/16 authoritative changed files 的安全差异扫描
  `14918661-76ef-4f40-847b-2f8addc08ec7`，传统可报告 finding 为 0；
- 通过未覆盖组合反例发现 teardown identity、reset fault、terminal restamp、前端 gesture snapshot、
  immediate fail-closed、saved-row identity、五态标签与验收门判别力残余；
- R1 结论为 `CHANGES_REQUIRED`，生成一次性 `M0-M1-001-R2`，不扩展到 M2。

### 2026-08-24 — M0-M1-001 独立验收

- Claude 在 `a4f8d22..107f3e5` 落地 S1 窄修和 desired-watch 纵向实现；
- Codex 独立复跑 workspace、frontend 和 architecture 门；
- Codex 使用本机已有锁定工具真实构建并验证 Windows 候选，确认当前脚本能通过但没有证明重启物化；
- 完成 28 个自动 inventory 文件加 1 个 binary-classified TypeScript 源码的安全差异扫描；
- 安全扫描封存为 0 个传统可报告漏洞；4 个同用户/本地正确性候选被安全策略排除，仍作为产品阻断；
- 验收结论为 `CHANGES_REQUIRED`，生成 `M0-M1-001-R1` 集中修复任务。

### 2026-08-23 — 初始版本

- 记录 Codex orchestrator / 用户转发 / Claude implementer 的真实通信方式；
- 解释九项总案与历史执行顺序；
- 合并后半段 desired-watch scope cut；
- 记录 `main@ae65958` 与 `feat/s2-alert-delivery@a4f8d22` 检查点；
- 记录九项完成度、迁移/发布门和下一里程碑建议；
- 尚未向 Claude 发出正式实现任务。

## 20. 新工作模式任务账本

此表只记录采用本文协作协议之后的任务。每次 Codex 发出任务包、收到 Claude 回报、
作出验收结论或批准进入下一里程碑时都必须更新。

```text
Active task id: STAGE-2
Milestone: Stage 2 complete S2 + P1/L1/L2/L3 closure
Claude prompt prepared: YES
Claude prompt forwarded: NO — 等待用户复制转发
Prompt/version: STAGE-2/v1
Expected base: feat/s2-alert-delivery@75cefb0
Claude reported head: pending
Codex review range: pending (expected 75cefb0..<head>)
Codex verdict: NOT_STARTED
Blocking findings: none; Stage work not yet implemented
Prior milestone: M0-M1-001-R4/v1 at 75cefb0 — ACCEPTED
Superseded task: M2-001/v1 — SUPERSEDED BEFORE IMPLEMENTATION
Active task: STAGE-2/v1
Next authorized action: 用户将 docs/orchestration/tasks/STAGE-2.md 中的 Claude Prompt 原样转发给 Claude
```

验收结论只允许使用：

- `ACCEPTED`
- `ACCEPTED_WITH_DEFERRED_DEBT`
- `CHANGES_REQUIRED`
- `BLOCKED`（仅真实外部/产品决策阻断）
- `NOT_STARTED`
- `IN_PROGRESS`
- `PENDING_REVIEW`

未经 Codex 更新本表为 `ACCEPTED` 或 `ACCEPTED_WITH_DEFERRED_DEBT`，Claude 的完成声明不得推进 Stage 状态。
