# Stage 5 R3：让派生关系与独立窗口使用同一证据时钟

状态：**ACCEPTED_WITH_DEFERRED_DEBT — 已串行集成；NO_PRODUCTION_CHANGE / DATA_REQUIRED**

Prompt 版本：`STAGE-5-R3/v1`

基线：`codex/parallel-wave1-stage5-s3-evidence@2c7b53a874714f57c0f905fbc0abeecfb325b387`

## A. 修复任务包

### A1. 已经通过、不得重写

Codex 独立复跑最终 head：119/119、0 skip；当前 NB evidence byte-identical，仍为
`NO_PRODUCTION_CHANGE / DATA_REQUIRED`。以下合同成立并冻结：

- identical/contained/overlapping 与绝对时间未平移的 regular subsample family；
- transitive union、全 family edge 的 client start/end/serverDate agreement；
- server-clock bracket peak/off-peak、off-peak purity、CE-9～CE-14；
- whole-target/group/residual stability、server evidence、honest GO control；
- 零 production scheduler change、离线/只读/byte-stable 输出。

本轮不扩成任意伪造检测。完全不重叠分割、逐样本不规则抖动、从零伪造继续 deferred。

### A2. 只修两条已实测 GO 的反例

#### 1. 常量平移后的规则 subsample 仍属于同一派生 family

在 `2c7b53a` 上实测：NB 使用 dense base；NK 使用同一 capture 的 stride-2 subsample，并把所有 client
start/end `+1 ms`；CM 使用 stride-3 并整体 `+2 ms`；body、serverDate、顺序均不变。输出仍为三个 clean
class，六门全真：

```text
verdict=GO qualifier=none brackets=180 distinguishable=true
```

根因是 derived record key 先要求绝对 `clientStartMs` 相等；常量平移后关联根本没有建立，因此绝对 client
start/end/serverDate agreement 没机会把它判为 conflict。

必须做到：

- 规则 subsample/decimation 的关系识别对单一常量 client-clock 平移保持不变；可以基于 body/order 与相对
  时间格或等价可证明关系，不得先要求绝对 client start 相等；
- 关系建立后，现有绝对 client start/end/serverDate agreement 继续以 0ms 容差运行，因此 `+1/+2ms`
  派生副本应进入同一 family 并成为 timeConflict/无独立 campus coverage；
- 只需覆盖可证明的规则抽样 + 单一常量平移；逐样本任意 jitter、完全不重叠 partition 与新造 body 保持
  deferred；
- 真正独立、不同 observation namespace 的三 campus honest GO 继续通过。

新增端到端判别 fixture：上述 stride-2 `+1ms`、stride-3 `+2ms` 在旧 head GO、新实现 NO-GO；未平移
subsample、transitive family 和 honest control 保留。

#### 2. server 连续的一次 session 不能被 client clock jump 拆成两个独立窗口

在 `2c7b53a` 上实测：每 campus 只有一段 serverDate 连续的 20-tick session，真实 server 时间跨纽约
16:55–17:05；仅把后半 client start/end 向前跳 11 分钟。client gap 把它切成 w00/w01，A4-2 又分别用
server brackets 数出 off-peak/peak，最终六门全真：

```text
verdict=GO qualifier=none brackets=60 distinguishable=true
```

生产 GO 已要求 server clock，所以“独立窗口/session”也必须由 server timeline 证明：

- 在 A4-2 evidence 层，若多个 client-segmented windows 的 server timeline 仍按既有 session gap 规则连续，
  必须合并为同一个 evidence session，或因 client/server clock discontinuity fail closed；
- 一个 server-contiguous session 跨 17:00 不得同时补齐 peak 与 off-peak；CE-14 purity 不得只依赖 client
  生成的 `windowId`；
- 真正由 server timeline 分隔的 peak/off-peak sessions（例如相隔约 19 小时的 honest control）仍可分别
  贡献；展示用 client window label 可以保持，不必重构 ingest/UI；
- 使用既有 `max(10 min, 5×interval)` 或证据强度不低于它的 server gap 规则，Date 量化与 bounds 处理保持
  确定、byte-stable。

新增端到端判别 fixture：server 连续 + client 后半跳 11 分钟在旧 head GO、新实现 NO-GO；真正 server
分隔的两 session control 仍 GO。

### A3. 明确不做

- 不重写已通过的 provenance family、A4-6、safe offset、server peak/purity 或全部 analyzer；
- 不检测任意逐样本抖动、完全不重叠三分割、全新伪造/签名/可信采集；
- 不修改 production crate、scheduler、storage、CI、依赖或 `docs/orchestration/**`；
- 不联网、不访问 Rutgers、不采样、不跑 workspace/frontend/archive/security scan；
- 当前数据的数字与 NO-GO 结论不得因修复发生无解释漂移。

### A4. 工作方式、验证与回报

使用 Claude Code 官方 dynamic workflow。两个调查 agent 分别构造 translated-subsample 与
server-contiguous/client-jump fixture，唯一 writer 实现，独立复核员只攻击这两条；普通 minor finding
记录后结束，不再扩轮。

运行完整 analyzer tests、两条旧/新 end-to-end negative proof、当前本地 data 重生成/byte comparison、
honest GO controls 与 `git diff --check`。严格离线。

一次回报：commits；两个反例旧/新 stdout；机制；测试数；current-data hashes/数字；GO controls；changed
files；deferred；Git status；review range `2c7b53a..HEAD`。不得宣布 production scheduler 已实现或 Stage 5
accepted。

## B. Claude Prompt（请用户从下一行开始原样复制）

```text
ultracode: 请继续使用 Claude Code 官方 dynamic workflow，只在
Z:\Project\Rutgers-BetterCourseSchedulePlanner\.worktrees\parallel-wave-1\stage5-s3-evidence 的
codex/parallel-wave1-stage5-s3-evidence@2c7b53a874714f57c0f905fbc0abeecfb325b387 上完成 STAGE-5-R3/v1。
不要写主 checkout，不 reset/rebase/merge/push/tag/release，不修改 docs/orchestration/**，严格离线，不访问
Rutgers。

先完整阅读主 checkout 的
Z:\Project\Rutgers-BetterCourseSchedulePlanner\docs\orchestration\tasks\STAGE-5-R3.md A 部分。
现有 119 项、provenance union、server peak/purity、A4-6、当前 NO-GO 与 honest GO controls 都保留。本轮只修
两条在 2c7b53a 上已实测为 GO 的精确反例：同一 capture 的 stride-2/stride-3 规则 subsample 在 client
start/end 整体平移 +1/+2ms 后仍必须被识别为同一派生 family，再由绝对字段 agreement 判 conflict；同一段
serverDate 连续、跨 17:00 的 session 不能因 client 时钟向前跳 11 分钟而被切成两个独立 evidence windows
并同时补齐 peak/off-peak。

内部算法由你自主决定，但派生识别只需对规则抽样+单一常量平移不变；production GO 的窗口独立性必须由
server timeline 证明。保留真正 server 分隔约 19 小时的两-session GO 与独立三-campus GO。不要扩到逐样本
jitter、完全不重叠分割、任意伪造/签名平台，不重写其他 gate，不改 production/CI。

为两条路径增加能打挂 2c7b53a 的 end-to-end 反例，运行完整 analyzer tests、current-data 重生成与 byte
comparison、GO controls、diff-check；不跑全仓重门/archive/security scan。完成后按 A4 一次性回报；
Stage 5 的最终接受与生产调度器决定仍由 Codex 作出。
```
