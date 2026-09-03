# Stage 5 R2：关闭最后两条可构造的 evidence 假 GO

状态：**READY — 返回原 Stage 5 worktree 做最后一次窄修**

Prompt 版本：`STAGE-5-R2/v1`

基线：`codex/parallel-wave1-stage5-s3-evidence@b4e93f7a009c4ab603b94e427d882cb132e0c29a`

## A. 修复任务包

### A1. 已经成立的部分

`9582728..b4e93f7` 已真正关闭原 R1 的空 peak window、client-clock GO、整 target/group/outlier 留出以及
整份复制/重标等主要反例。Codex 在最终 head 独立复跑 analyzer 为 96/96、0 skip；当前 NB evidence 仍是
`NO_PRODUCTION_CHANGE / DATA_REQUIRED`，真实 GO fixture 仍可达。

本轮不重写 analyzer，只关闭审计后剩下的两条具体假 GO。

### A2. 必须关闭的两条路径

#### 1. 同一 capture 的派生切片/规则抽样不得增加 campus provenance 覆盖

当前 provenance 只合并“较短 series 整体连续包含于一个代表”的情况。把同一 capture 的 peak/off-peak
session 切成三条等长、错位但大量重叠的范围，分别重标 NB/NK/CM，三条互不完整包含，仍会形成三个 clean
class 并可能通过 A4-1 至 A4-6。当前代码还明确把规则 subsample/cadence 变体列为已知边界。

修复目标不是检测任意伪造数据，也不是建立密码学采集证明；只需让同一批真实 observation/hash records 的
明显派生复用不能冒充独立 target evidence：

- 大量重叠的连续切片必须并入同一 provenance family，或 fail closed 地从独立 campus 覆盖中排除；
- 一条是另一条的规则 subsequence/subsample、且 observation/hash 与时间格可证明来自同一 series 时，也
  不能增加覆盖；
- union/transitive 情况要稳定：A 与 B、B 与 C 同源时，不能因 A/C 不直接包含而得到两个独立 class；
- 不得仅靠 run.json campus/term/文件名/输入格式区分；NDJSON/SQLite 同源规则继续成立；
- 真正无 record reuse、无可证明派生关系的独立 synthetic NB/NK/CM GO fixture 必须继续通过。

至少新增两条端到端判别 fixture：三份等长错位重叠切片；规则 subsample/派生 series。它们在当前 head 可
构成三个 campus evidence，新实现必须 NO-GO/冲突；另保留 honest GO control。

#### 2. server-clock GO 的 peak/off-peak 分类必须使用 server evidence

当前 A4-2 用 client outer envelope 的 `clientUpperMs` 判 peak，而 comparison/A4-5 已要求 server clock。
因此可保持 body、client start、serverDate 全在 off-peak，只把 `requestEndedUtc` 延长到 17:00–18:00 ET，
让 off-peak server bracket 被算成 peak；provenance agreement 又没有比较 client end。

必须做到：

- 任何可能进入 production GO 的 peak/off-peak qualifying bracket，都按参与 comparison 的 server-clock
  bounds/时间归类；只有 client envelope 跨过峰值不能贡献 server peak evidence；
- 缺少可用 server bounds 的 bracket 不得补齐 A4-2 的 production peak/off-peak 门；
- provenance record/time agreement 同时覆盖 client start、client end 与 serverDate 的存在性和值，不能让
  只改 request end 的副本仍作为 clean representative；
- Date 的 1 秒保守加宽、现有 clock caveat/safe-offset 语义保持，边界归类必须确定且 byte-stable。

新增端到端反例：三 campus honest GO 形状中，所谓 peak session 的 server evidence 实际仍在 off-peak，只有
`requestEndedUtc` 被延长进入 peak；当前 head 可 GO，新实现必须因 peak evidence 不足 NO-GO。真实 server
peak control 仍 GO。

### A3. 保持不变与明确不做

- A4-6 whole-target、group 和 residual-top-k 实现不再重写；零 residual 的普通呈现问题不阻断；
- 不尝试防御从零伪造全新 body/timestamp 的攻击，不建立签名、在线采集或可信硬件；
- 不修改 production crate、scheduler、policy、storage schema、CI 或 `docs/orchestration/**`；
- 不访问 Rutgers，不读取未授权数据，不修 README polish、N1–N3 或其他 deferred；
- 继续生成 byte-stable JSON/MD，当前本地数据的全部实测数字与 NO-GO 结论不得被“修成”GO；
- 结构性坏输入仍 fail closed 且不产出文件。

### A4. 工作方式与验证

使用外部实现工具的官方 dynamic workflow。先让独立调查 agent 分别构造两条反例和 honest control，再由唯一
writer 修改 provenance/peak classification，最后用独立复核员尝试绕过；不要为普通 minor finding再开
修复轮。

运行全部 analyzer tests、两条新 end-to-end negative proof、当前本地 data 重生成与 byte comparison、
`git diff --check`。严格离线；不跑 workspace/frontend/archive/security scan。

完成后一次回报：Outcome；commits；新 provenance/peak 语义；两条反例在旧/新实现上的结果；测试精确
数量；current-data 数字；GO control；changed files；deferred；Git status；review range
`b4e93f7..HEAD`。不得宣布 production scheduler 已实现或 Stage 5 已 accepted。

## B. 实现任务提示（请用户从下一行开始原样复制）

```text
ultracode: 请继续使用外部实现工具的官方 dynamic workflow，只在
Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\parallel-wave-1\stage5-s3-evidence 的
codex/parallel-wave1-stage5-s3-evidence@b4e93f7a009c4ab603b94e427d882cb132e0c29a 上完成 STAGE-5-R2/v1。
不要写主 checkout，不 reset/rebase/merge/push/tag/release，不修改 docs/orchestration/**，严格离线，不访问
Rutgers。

先完整阅读主 checkout 的
Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\orchestration\tasks\STAGE-5-R2.md A 部分。
本轮只关闭两条剩余假 GO：同一 capture 的等长错位重叠切片或可证明的规则 subsample/派生 series，不能因
重标 NB/NK/CM 增加独立 provenance 覆盖；server-clock production GO 的 peak/off-peak 必须按 server
bracket evidence 分类，不能只延长 requestEndedUtc 就把 off-peak 数据伪造成 peak，同时 provenance
agreement 要覆盖 client start/end 和 serverDate。

内部算法由你自主决定，但要处理 transitive provenance family，保留真正独立的三-campus GO control，且
不要扩成任意数据伪造检测、签名或在线采集平台。A4-6 和其余已关闭 R1 合同不重写，不改任何 production
crate/scheduler/CI，不顺手修 minor/deferred。

为两条路径各增加能打挂 b4e93f7 的 end-to-end 反例，运行完整 analyzer focused tests、当前本地数据重生成
和 byte-stability/diff-check；不跑全仓重门/archive/security scan。完成后按 A4 一次性回报；Stage 5 的
最终接受与是否实施生产调度器仍由 Codex 决定。
```
