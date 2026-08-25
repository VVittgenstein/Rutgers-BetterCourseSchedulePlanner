# RBCSP 余下四阶段固定执行计划

状态：**ACTIVE — 用户于 2026-08-24 冻结范围与顺序**
Orchestrator：Codex
实现者：Claude
产品负责人：用户
当前产品代码基线：`feat/s2-alert-delivery@75cefb0`

## 1. 唯一任务线

Stage 1 对应 S1，已经完成。余下工作只按以下四个 Stage 推进：

```text
Stage 1 — S1（已完成）
Stage 2 — S2（同时完成 P1、守住已完成 L1、完成 L2/L3 与通知政策修订）
Stage 3 — P2
Stage 4 — S4
Stage 5 — S3
```

顺序固定为：

```text
S2（P1/L1/L2/L3） → P2 → S4 → S3
```

这里的括号表示同阶段交付，不表示 P1/L1/L2/L3 可选、免费或自动完成。L1 已于
`75cefb0` 验收，Stage 2 只守回归、不重新设计。

发布、merge、tag、真实部署不是额外产品 Stage。余下四个 Stage 完成后，Codex 再基于最终 head
准备一次候选/发布门；任何真实外部变更仍需用户另行授权。

## 2. 范围冻结规则

1. 只实现原会话和已批准设计中已经讨论确定的 S2/P1/L1/L2/L3、P2、S4、S3；
2. 不因审查发现“可以更优雅”“未来也许需要”而新增产品能力、持久化、协议、迁移或平台；
3. 当前 Stage 之外的问题只记录到 deferred ledger，除非它直接阻断当前 Stage 的既定用户结果、
   安全/资源边界或可验证性；
4. 不重开已经验收的 S1、L1/reduced desired authority，不复活已取消的 desired WebSocket、实时跨 tab
   同步、leader/Web Locks/BroadcastChannel；
5. 优先使用现有结构和测试接缝。没有现实调用路径、没有失败证据、也不影响主合同的 defensive
   分支，不为它提前扩建架构；
6. 若实现必须改变已冻结的用户行为、wire/数据库权威合同或政策边界，停止并交由用户决定，Claude
   与 Codex 都不得自行扩权。

## 3. 每个 Stage 的固定协作方式

每次只详细编写和发出**当前一个完整 Stage**。未来 Stage 的详细 prompt 在前一 Stage 验收后，按新的
真实 head 重新审计再写，避免提前写死已经漂移的内部实现。

```text
Codex：调查当前代码 → 写完整 Stage 结果/边界/验收合同与可复制 prompt
用户：一次原样粘贴给 Claude
Claude：用一个 dynamic workflow 自主规划、实现、验证和窄修
Claude：整个 Stage 完成后只回报一次
Codex：覆盖整个 Stage diff 集中审查一次
       ├─ 无 blocker → ACCEPTED / ACCEPTED_WITH_DEFERRED_DEBT
       └─ 有 blocker → 最多一份集中 repair 包
```

不再恢复“小 slice 实现一次、全量测试一次、审查一次”的 M2/M3/M4 模式。

## 4. Claude Dynamic Workflow 使用规则

每个 Stage 的可复制 prompt 以人工输入触发词 `ultracode:` 开头，明确要求使用 Claude Code 官方
dynamic workflow。官方说明见 <https://code.claude.com/docs/en/workflows>。

官方最低版本为 Claude Code `2.1.154`；本项目归档记录的当前版本是 `2.1.237`，版本条件满足，
但具体 plan/config 是否启用仍由 Claude 在启动前核对。

工作流的编排由 Claude 根据真实仓库决定：

- Claude 自己决定内部 phase、subagent 数量/分工、文件组织、逻辑 commit 和 focused tests；
- 可并行读取、代码面盘点、测试分析和独立/对抗复核；
- 当前权威 checkout 的产品写入、`git add/commit` 与最终集成由一个 writer/integrator 串行执行；若
  Claude 确需并行实现，只能在基线明确的隔离副本中进行，再由 integrator 审查并回收到权威分支；
- 形成“理解 → 实现 → focused verification → 集成 → 独立合同复核 → 按失败证据窄修”的闭环；
- 不手写或钉死 workflow JavaScript，不追求 agent 数量，不把 Dynamic workflows 与 Agent teams 混用；
- 若当前 Claude Code/version/plan/config 无法使用该正式功能，应在改代码前精确回报，不能假装已经使用。

Dynamic workflow 会显著增加 token/时间；本项目要求**最小充分并行**，不是无限 fan-out。

## 5. 缺陷分级与“拒绝过早优化”

### BLOCKER

仅以下问题阻断 Stage：

- 主场景会假绿、漏恢复、复活已 STOP/Disconnect 的监控、丢失用户意图或唯一控制入口；
- session/nonce/Origin/Host/target/local-public 边界被破坏；
- L2 会在仍有页面时误退出，或页面全部离开后永不执行已承诺的退出；
- 当前 Stage 新增无界连接、任务或内存增长；
- 数据/迁移损坏、发布证据与实际 head 不一致；
- 关键测试是假阳性，不能证明本 Stage 的用户结果；
- 破坏已经验收的 S1 或 L1/reduced desired 行为。

### DEFERRED

默认记录后继续：

- 普通命名、helper 重复、代码组织、注释和测试样式；
- 正常产品路径不可达、没有真实触发证据的理论 defensive case；
- 不影响本阶段用户结果的重构、性能美化或“为未来准备”的抽象；
- N1–N3；
- 按固定预算仍无法复现的单次 transient；
- 明确属于后续 Stage 的工作。

Codex 可以给出 `ACCEPTED_WITH_DEFERRED_DEBT`。不得为了技术债清零生成连续修复轮。

以下是已接受行为/残余风险，不创建 deferred repair：跨 tab 修改在刷新后可见；多个 tab 都可能响；
frozen/discarded 页面不执行 JS，因此不承诺提醒必达。

## 6. 四个 Stage 的固定产品边界

### Stage 2 — `STAGE-2`

完成：

- S2 意外断线自动恢复、诚实五环 Readiness、音频自愈与当前页面级通知；
- P1 公网浏览器换票与页面生命周期内的监控恢复；
- 已完成 L1 的回归保护；
- L2 local-only 页面 presence、60 秒可见倒计时和优雅退出；
- L3 最小完整控制台生命周期日志；
- 已批准的当前页面通知政策/capability 同步。

不做：H4/完整 P2、S4、S3、发布/部署；P2/H4 仍阻断公网部署。

### Stage 3 — `STAGE-3`

只完成已批准的公网重新上线加固 P2：H1–H9、H4 资源/背压边界、真实 public composition、配置、
runbook 与相应 soak。H5 已有部分只做差距补齐和回归，不借机重写 session 系统。

不夹带 S4、S3 或新产品能力。具体任务包在 Stage 2 验收后的真实 head 上另写。

### Stage 4 — `STAGE-4`

只做已批准的 SQLite `prepare_cached` 微优化，保持行为完全不变，不做增量写、不重构存储模型、
不顺手清理无关查询。以等价性测试和合理的性能证据验收，不设易抖动的绝对毫秒 CI 门。

### Stage 5 — `STAGE-5`

S3 必须 evidence-first：先取得多个 target/时段的区间删失证据，再决定是否冻结 30/60 秒周期和
safe offset。证据不足时，以 `NO_PRODUCTION_CHANGE / DEFERRED` 正常结束，不猜参数、不为了“完成四阶段”
强改生产调度。证据充分才实现已批准的统一网格/重锚行为。

## 7. 验证频率

- Stage 内部：Claude 自主选择 focused tests，失败即修，避免每个 phase 跑全量门；
- Stage final：workspace、frontend verify、architecture 与该 Stage 专项集成门各运行一次；
- package/soak：只在 Stage 产品结果确实需要真实组装证明时运行；不为每个 commit 构建 archive；
- security review：由 Codex 在涉及 session、资源或发布边界的 Stage final diff 上集中做；
- negative proof：只要求最高风险合同具有判别力，不逐 helper 机械 revert；
- transient：保存首次完整证据，同一 full gate 最多再跑一次、可定位 case 最多再跑两次；仍不复现则
  记录而不继续烧时间。

## 8. 当前状态

| 项目 | 状态 |
|---|---|
| S1 | 已完成 |
| L1 / reduced desired | `ACCEPTED` at `75cefb0`，只回归 |
| 旧 `M2-001` | `SUPERSEDED BEFORE IMPLEMENTATION` |
| 当前 Stage | `STAGE-2/v1`，等待用户转发 |
| 后续固定顺序 | `STAGE-3 (P2)` → `STAGE-4 (S4)` → `STAGE-5 (S3)` |
