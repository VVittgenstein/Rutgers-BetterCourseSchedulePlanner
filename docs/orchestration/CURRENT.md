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

## 6. Codex 验收方式

Codex 在里程碑边界集中验收，不逐小 commit 打断 Claude。

默认循环：

```text
任务包 → Claude 完整实现 → Codex 一轮集中审查
       → Claude 一轮集中修复 → Codex 最终验收
```

只有发现新的 P0/P1、产品行为冲突或迁移/发布阻断时才增加轮次。

Codex 必须：

- 固定审查范围 `base..head`；
- 区分 Claude 声称运行的测试与 Codex 实际复跑的测试；
- 一轮覆盖完整 diff，不一次只报一个 finding；
- 先报告阻断，再列非阻断技术债；
- 对弱测试检查判别力；
- 不因非阻断问题重开整个架构；
- 只有用户可见纵向结果闭合后才把里程碑标为完成。

## 7. 加速规则

旧模式的主要速度问题是：反复重写设计、过细 dormant 切片、每小改都跑全套门、
finding 一条一条往返、用户手工重组消息。新的固定规则：

1. 以**用户可用里程碑**切片，不以单个类型或 helper 切片；
2. Claude 可在一个任务包内连续完成多个内部 commit；
3. Codex 在 Claude 编写期间并行准备验收表和反例；
4. findings 一次性汇总；
5. 默认只允许一轮集中修复；
6. 开发中跑定向测试，里程碑边界跑 workspace/frontend/architecture；
7. 只有激活、合并或发布边界才跑打包 E2E 与重型安全扫描；
8. 非阻断技术债记录，不抢占当前主线；
9. 没有新的产品决定或真实阻断时，不再发起大设计循环；
10. 用户只因产品选择被打断。

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
当前检出：feat/s2-alert-delivery@2cd54e2
origin/main：ae65958
S1 分支：feat/s1-snapshot-gate@a4b35bc（已合入 main）
M0-M1 实现基线：feat/s2-alert-delivery@a4f8d22
Claude R2 报告 head：2cd54e2
Codex R2 审查范围：c50499f..2cd54e2（全里程碑 a4f8d22..2cd54e2）
S2 分支：尚未合并、尚未发布
当前产品源码工作树：无未提交源码；两个 conversation 归档目录未跟踪
Codex orchestration 文档：已记录 R2 CHANGES_REQUIRED，并生成 R3 组合反例收口任务
现有 release/0.1.0：sourceCommit=7d5debef（2026-07-15，早于本轮工作）
```

恢复时必须重新核实这些值，不得永久假设它们仍然成立。

## 13. 当前完成度

| 工作 | 状态 | 当前事实 |
|---|---|---|
| S1 | 代码完成并已合入 main；M0 窄修已写入 feature | Gate、三条生产路径、迁移、重启、投影、前端均已接线；`3f4ebb0` 删除 probe 正 jitter；尚无新 release |
| S2 | desired 纵向闭环与 R2 已实现但验收仍未通过，仅 feature | 真实 archive 已证明重启物化/reset；仍有 STOP×rotation、failure stamp、真实批量/不确定 mutation 与 close-read 竞态；自动重连、完整 Readiness、音频、通知仍未做 |
| S3 | 未开始正式实现 | 没有 GridAnchor/RebuildProfile；仅有不足以冻结参数的单校区数据 |
| S4 | 未开始 | 代码中没有 `prepare_cached` |
| P1 | 服务端半边完成，仅 feature | validate、reserve lease、session 保护完成；浏览器不会调用或自动重连 |
| P2 | 大部分未开始 | H5 基本完成、H4 只有 per-session cap；其余重新上线门未完成 |
| L1 | 纵向代码已落地但未通过验收 | desired 可持久化、GET/PUT、materializer、UI 与真实包装 E2E 已存在；`M0-M1-001-R3` 接受前不得称可发布或完成 |
| L2 | 只有通用 route seam | 无 presence、60 秒状态机或自动退出 |
| L3 | 未开始 | 无可见业务日志与控制台倒计时 |

## 14. 已发现但尚未处理的关键漂移

### S1

- `3f4ebb0` 已把 Gate 隔离探测修为严格 `min(30s, watch interval)`，并有可打挂旧 jitter 的反例；
- Codex 在 `107f3e5` 独立复跑相关测试通过；
- N1–N3 三项迁移测试加固是明确非阻断技术债，不得阻塞当前主线。

### S2/L1

- R1/R2 已实质关闭持续 admission/live drift、receipt-only rotation、reset 屏障与 incomplete 503、
  teardown id、普通/cross-caller terminal stamp、单条 gesture、raw NUL、五态、late-page active 和真实包装物化；
- Codex 已独立验证 `2cd54e2`：workspace 739/1 ignored、frontend 239、architecture、三个 diff-check 全绿；
  真实 12 文件 Windows archive 再次完成普通重启 attach/materialize、Full Reset 与第三次空重启；
- STOP 在 receipt threshold−1 写 tombstone 后立即触发 rotation；rotation 清掉该 row。健康 STOP 的
  COMMITTED body 因此被本地 strict codec 拒绝；stop fault 时内部虽保存 id，GET/desk 却完全看不见它；
- `failure` 没有 generation/revision/epoch stamp。旧 PERMANENT failure 会穿过 stuck teardown、STOP 与
  新 START，永久阻止新的 intent 装配；
- 单条 `setSectionIntent` 已捕获 gesture snapshot，但真实 Start selected 与 Apply policy 逐项 await 后
  再捕获，仍会把后续 section 静默 rebase 到跨 tab 新 revision/rotation generation；
- PUT outcome 不确定时 recovery GET 保持 READY，旧 snapshot 仍可绿/可移除。START 已提交但 response
  丢失时可删除唯一管理行；STOP response 丢失时旧 green 可无限保留；
- CLOSED/ERROR 只记录当时 snapshot 已知 running rows；关闭前 issued 的 held GET 可在关闭后返回 RUNNING
  并复活绿色；
- `intentSaved` 只保留 policy 非空，未覆盖 pendingDisarm/materialized tombstone；读取失败仍可隐藏 teardown；
- Windows policy helper 已比较 600/601，但仍接受 extra keys、600.4 与数字字符串；checkpoint seam
  无条件进入生产公开 API；
- `M0-M1-001-R3` 关闭前，S2-D3、M1 和迁移发布门保持开放。

### P1/S2

- 公网前端没有 mutable nonce holder；
- 没有调用 `/api/v1/session/validate`；
- 没有自动重连；
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

## 16. 当前拟定的下一里程碑

状态：**`M0-M1-001-R2` 已交付但 Codex 判定 `CHANGES_REQUIRED`；组合反例最终收口任务
`M0-M1-001-R3` 已生成，等待用户转发给 Claude。**

当前里程碑不扩展范围：**先把 reduced S2/L1 纵向闭环修到可信、可打包验收。**

用户可见目标：

- 本地页面能读取和提交“我想盯哪些课”；
- 程序按已提交 desired 装配真实监控；
- 页面刷新/程序重启后恢复；
- 页面显示 desired、准备、已装配或失败的真实状态；
- 多页面共享状态但不实时推送；
- Full Reset 后再次重启仍为空。

R3 必须包含：

1. STOP×rotation 保留 stopping tombstone，并冻结合法缺行的 0/0 committed shape；
2. failure 绑定 authority stamp，新 intent 不继承旧 PERMANENT 结论；
3. Start selected/Apply policy 整个点击共用一个 snapshot；
4. mutation 不确定时立即撤销旧 READY，并保留可能已启动的管理 identity；
5. CLOSED/ERROR 建立全局 response cutoff，关闭前在途 GET 不可复绿；
6. policy verifier 严格 keys/types，checkpoint hook 不进入生产公开 API；
7. 从 final head 重新构建一次候选并集中跑全量门；
8. 保持公网 desired 零表面，不发布中间构建。

建议不包含：

- 共享自动重连；
- P1 浏览器换票；
- 完整五环 Readiness；
- AudioContext 自愈；
- 页面级 Notification；
- L2/L3；
- 完整 P2；
- S3/S4。

上述后续工作仍在总案内，只是不属于第一个里程碑。

## 17. 当前推荐的后续顺序

为避免“顺带”造成歧义，今后使用明确里程碑：

```text
M0：S1 30 秒窄修 + 最终合同同步
M1：reduced S2/L1 本地 desired 纵向闭环
M2：共享自动重连 + P1 浏览器换票
M3：完整 Readiness + 音频自愈 + 页面级通知
M4：L2 presence/60 秒退出 + L3 控制台日志
M5：P2 公网加固（H4 可与 M1–M3 并行准备）
M6：S4 prepare_cached
M7：S3-PR0 数据分析 → 参数冻结 → 调度器实现
M8：最终 Windows/Linux 候选、打包、soak、发布
```

M0 与 M1 已按用户批准合并执行；当前只做 M1 的 R3 收口，不进入 M2。

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
Active task id: M0-M1-001-R3
Milestone: M0 S1 窄修与合同同步 + M1 reduced S2/L1 本地 desired 纵向闭环
Claude prompt issued: YES — 由用户复制转发
Prompt/version: M0-M1-001-R3/v1
Expected base: feat/s2-alert-delivery@2cd54e2
Claude reported head: pending
Codex review range: pending (expected 2cd54e2..<head>; full a4f8d22..<head>)
Codex verdict: CHANGES_REQUIRED
Blocking findings: STOP×rotation row 消失；failure 未带 stamp；真实 batch gesture rebase；uncertain mutation 仍 READY；close/read 复绿；policy shape/API seam 残余
Prior repair: M0-M1-001-R2/v1 at 2cd54e2 — CHANGES_REQUIRED
Repair task: M0-M1-001-R3/v1
Repair expected base: feat/s2-alert-delivery@2cd54e2
Next authorized action: 用户将 docs/orchestration/tasks/M0-M1-001-R3.md 中的 Claude Prompt 原样转发给 Claude
```

验收结论只允许使用：

- `ACCEPTED`
- `CHANGES_REQUIRED`
- `BLOCKED`（仅真实外部/产品决策阻断）
- `NOT_STARTED`
- `IN_PROGRESS`
- `PENDING_REVIEW`

未经 Codex 更新本表为 `ACCEPTED`，Claude 的完成声明不得推进里程碑状态。
