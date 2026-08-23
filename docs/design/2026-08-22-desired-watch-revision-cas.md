# 期望监控：revision/CAS 共同编辑模型（S2-D3 路线改判）

状态：ACTIVE，**v4**。路线由产品所有者 2026-08-22 裁定：取代 fenced
sequencer，走清单二选一的另一条——**持久化 revision + tombstone + CAS**。
S2-D3 硬门在本设计实现并复审通过前**保持关闭**。

**wire 名冻结**：全篇只用 **`authorityGeneration`**，`resetGeneration` 作废。

**修订史**：v1 五组阻断 → v2 四组阻断 → v3 自查三镜头再挖出 11 项
（幂等身份漏 section、mutationId 派生导致 section 永久锁死、epoch 无分配点/
无持久化/未按代作用域、policy 更新自相矛盾、exactly-once 键在本仓库不存在、
`pendingDisarm` 无 wire 字段、两个 9 门帽不是同一个计数器、前端负控与
§7.5 冲突、**UI 投影规则完全缺失**等）→ **v4 逐条闭合**。

## 0. 唯一要约束的东西：UI 投影

本纲领禁止的失败只有一种：**页面声称某 section 在被监控而实际没有，或
反之**。v1–v3 反复出洞的根因是全文只写服务端机制、从未写这条投影规则。
先定死它：

| `desired` | `armed` | `pendingDisarm` | 用户看到 | Readiness |
| --- | --- | --- | --- | --- |
| 1 | true | false | **监控中** | 绿 |
| 1 | false | false | **准备中**（附失败分类文案） | **非绿**（DEGRADED） |
| 0 | true | true | **停止中** | 非绿 |
| 0 | false | — | 未监控 | — |
| 1 | false | — （分类 = PERMANENT） | 不可达：永久失败由 authority 自身移出 desired（§5.2） | — |

**唯一的绿灯条件**：`desired = 1 ∧ armed = true ∧ ¬pendingDisarm`。
"已提交但未武装"**必须**显示为准备中而非监控中——CAS 提交成功只代表
意图落库，不代表在监控。

## 1. 产品模型（裁定）

- **所有标签页平权编辑**，不存在用户可见的"只有 leader 能改"；
- **`desired_watches` 是唯一真相**；
- **实际监控由服务端按已提交状态统一维护**，不由某个页面"拥有"；
- **leader 只剩一件用户看不见的事**：避免多个页面同时响铃。

## 2. 权威状态

### 2.1 单调量

| 量 | 分配 | 持久 | 比较方式 | 排序对象 |
| --- | --- | --- | --- | --- |
| `authorityGeneration` | Full Reset / 压缩 | metadata 标量 | 直接 | 全表世代 |
| `sectionRevision` | CAS 提交 | **全局计数器**，值存在每行上 | 逐 section 比较 | **desired 状态** |
| `materializationEpoch` | desired 转移 / 连接边沿 / actor 重建 | **全局计数器**，值存在每行上 | 逐 section 比较 | **armed 身份** |
| `attemptToken` | 每次 arm/disarm 尝试 | **不持久**（内存，actor 内单调） | 逐 section 比较 | **单次尝试** |

`sectionRevision` 与 `materializationEpoch` 都由**全局计数器**分配、
**逐 section 比较**——v3 把作用域写成"每 section"是错的，会诱导实现者
做 per-section 计数器，那样 metadata 标量与迁移赋值都无意义。

**三层守卫**（manager 物化器、广播序列化器、前端 store）各自拒绝倒退的：

- desired 侧：`(authorityGeneration, sectionRevision)`
- armed 侧：**`(authorityGeneration, materializationEpoch, attemptToken)`**

armed 侧**必须带 generation**：Full Reset 会把 epoch 计数器归零，若只比
epoch 标量，则重置后所有新事件都被当作倒退丢弃（UI 永远停在重置前的
armed 状态），且**重置前在途的 epoch-1 完成回执会与重置后的 epoch-1
比较相等而被接受，复活一个本该被杀掉的 watch**。

`attemptToken` 解决 **arm-vs-arm**：同一 epoch 下的两次退避重试若共用
epoch，§5.3 的"旧 arm 完成一律丢弃"无法区分它们。它不需要持久化——进程
崩溃后 epoch 本身就会翻新。

### 2.2 desired 表（迁移 10004 重建）

`personal_desired_watches_v1`：

| 列 | 说明 |
| --- | --- |
| `term_id` / `campus_code` / `section_index` | 主键 |
| `desired` | `INTEGER NOT NULL CHECK (desired IN (0,1))`；`0` 即 tombstone |
| `policy_json` | `TEXT NULL`；`CHECK ((desired = 1) = (policy_json IS NOT NULL))`，`CHECK (policy_json IS NULL OR json_valid(policy_json))` |
| `revision` | `INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991)` |
| `materialization_epoch` | `INTEGER NOT NULL`，同上界 CHECK |

`personal_state_metadata_v1` 增三个持久标量（同上界 CHECK）：
`desired_watch_authority_generation`（初值 1）、
`desired_watch_revision_counter`（初值 0）、
`desired_watch_materialization_counter`（初值 0）。

**epoch 的分配点与持久点（v3 缺失）**：desired 转移在 §3.3 步骤 5 的
**同一个 `IMMEDIATE` 事务内** `materialization_epoch = ++counter` 并落列；
非 CAS 触发（连接 `1→0`/`0→1`、actor 重建）由 actor 在**自己的小事务内**
分配并落列。任何时刻 epoch 都是持久的，因此崩溃重建不会重发已用过的值。

### 2.3 tombstone 与 receipt 永不在同一 generation 内被回收

v1 的"256 行上限 + 淘汰最旧 tombstone"**引入 ABA**（GC 后 key 回 rev0，
延迟的 `base=0` 旧 START 又被接受），已废除。

- **tombstone 保留到 Full Reset**；
- **receipt ledger 同样保留到 Full Reset**。注意二者增长律**不同**：
  tombstone 受"曾编辑过多少 section"约束，receipt 随**每次 mutation**
  增长。256 对二者都只是**观测/告警阈值**，不淘汰任何行；receipt 的实际
  增长由人类点击速率约束，可接受；
- **压缩（若将来确需）必须原子**：升代、**清理 desired 表与 receipt
  ledger 两张表**、修复三个计数器，**在同一个 SQLite 事务内完成**；
  **提交之后**才由 actor 广播完整 snapshot。v2/v3 稿的"升代 → 广播 →
  再清理"会让已广播的新代 key 在同代被清回 revision 0。

### 2.4 Full Reset

同一事务内：真删 desired 表与 receipt ledger 全部行、generation +1、
三个计数器归零；提交后由 actor 广播完整双 snapshot（§6.4）。

## 3. 变更身份、幂等与冲突语义

### 3.1 mutationId 是**新铸**的，不是派生的

**`mutationId` 由客户端为每一次用户手势新铸一个 UUIDv4，绝不由内容派生。**

v3 把它写成"绑定到 `(generation, section, 指纹)`"，在"派生"读法下会**永久
锁死一个 section**：

```
9 门满额 → 用户点 START S → LIMIT_EXCEEDED，receipt 保留到 Full Reset
用户停掉别的课，S 现在合法 → 用户再点 START S
→ 同 generation/section/desired/policy/base ⇒ 同派生 id
→ 重放 LIMIT_EXCEEDED，且"不重新 CAS、不 reconcile、不广播"
⇒ S 在本 generation 内永远无法被启动，UI 显示"未监控/超限"而实际只有 8 门
```

因此：**绑定是校验规则，不是派生规则**——同一 `mutationId` 必须始终配同
一个 `(authorityGeneration, section, 指纹)`；不一致即
`MUTATION_ID_CONFLICT`。指纹 = `(section, desired, policy 规范 JSON,
basedOnRevision)` 的 sha256（**v3 漏了 section**，见 §3.4）。

**成功与拒绝结果都必须回显 `mutationId`**。

### 3.2 冲突后必须终止，且服从单调守卫

拒绝结果里回带的 `current` **只用于更新投影，不是重试令牌**：收到冲突的
客户端必须**应用服务器真相并终止该 mutation**；只有用户在看到冲突后的
新界面上做出的**新手势**，才允许以**新 mutationId** 提交。

**但"应用服务器真相"服从 §2.1 的单调守卫**（v3 把它写成无条件，与守卫
直接冲突）：`current` 携带 `(authorityGeneration, revision,
materializationEpoch, armed, pendingDisarm)` 全套；客户端**只有在该元组
不倒退时**才应用。重连后已收到更新 snapshot 的页面，不得被一条迟到的
拒绝回执拉回旧状态。

`AUTHORITY_UNAVAILABLE` **不携带任何状态字段**，是**非终局**结果：
不落 receipt，同一 `mutationId` **允许原样重试**。它只在 authority 无法
评估时产生（actor 不可用、存储不可用、inbox 关闭）；一切能评估出结论的
情形都必须给终局结果。

### 3.3 CAS 规则（单个 `IMMEDIATE` 事务内，顺序即校验序）

1. `authorityGeneration != current` → `STALE_GENERATION`（**不落
   receipt**，见 §3.4）；
2. receipt ledger 幂等判定（§3.4）；
3. `basedOnRevision != 该 section 当前 revision`（无行为 0）→
   `STALE_REVISION`；
4. **仅当 `desired = true`**：9 门帽（§5.7 单一执行点）→ `LIMIT_EXCEEDED`；
   target 受支持 → `UNSUPPORTED_TARGET`；term 在窗口内 →
   `TERM_OUT_OF_RANGE`；section 已发布**且 S1 gate 已放行** →
   `SECTION_NOT_FOUND`（gate 未放行时**不拒绝**，见下）。
   **`desired = false` 跳过全部准入校验**；
5. 写入、`revision = ++revCounter`、`materialization_epoch = ++epochCounter`
   （仅 desired 值变化时；policy-only 更新不换 epoch，见 §4.3）、写
   receipt、提交。

**`SECTION_NOT_FOUND` 的 gate 依赖（v3 只写在 arm 侧，CAS 侧漏了）**：
S1 完整性 gate 处于 Hold/隔离期间，"section 不见了"可能只是残缺快照。
此时 CAS **不得**给出 `SECTION_NOT_FOUND` 终局拒绝——那会让 §3.2 逼客户端
放弃一个合法请求。改为：**放行提交**（desired 落库），由 arm 侧按
`PENDING_GATE` 暂态重试（§5.2）。

### 3.4 receipt ledger

v3 把"最后一次 mutation"存在 section 行上，无法兑现幂等承诺（满额拒绝后
腾出名额重到会被提交；M2 覆盖 M1 记录后 M1 无法重放）。

新表 `personal_desired_watch_receipts_v1`：

| 列 | 说明 |
| --- | --- |
| `authority_generation` + **`term_id` / `campus_code` / `section_index`** + `mutation_id` | 复合主键（**v3 漏了 section**） |
| `fingerprint` | `TEXT NOT NULL` |
| `outcome_json` | `TEXT NOT NULL CHECK (json_valid(outcome_json))` |

**v3 漏 section 的后果**：同一 `mutationId` 用于两个不同 section 且
`(desired, policy, base)` 相同（两个 rev0 的 START 是最常见情形）时，
指纹相同 → **第二个 section 重放第一个的成功回执**，提交者被告知"已提交"，
而该 section 从未落库、从未武装。

规则：

- **记录范围**：成功 + 终局拒绝（`STALE_REVISION` / `LIMIT_EXCEEDED` /
  `UNSUPPORTED_TARGET` / `TERM_OUT_OF_RANGE` / `SECTION_NOT_FOUND`）；
- **`STALE_GENERATION` 不落 receipt**：它在步骤 1 就被判定，重复请求会被
  同样地再拒一次，receipt 毫无作用；而落进哪个 generation 都错——落旧代
  是永不可达的泄漏行，落当代会烧掉一个当代 id；
- **`MUTATION_ID_CONFLICT` 不插入第二条 receipt**（v3 写"本身也落
  receipt"会撞主键或覆盖原结果，反而毁掉重放保证）：它**由已存在的行
  推导**——同 PK 存在但 `fingerprint` 不符即冲突，直接返回，不写库；
- **重复请求只向提交者重放**，不重新 CAS、不 reconcile、不广播；
- 重放使用**独立帧** `DESIRED_WATCH_REPLAYED`（§6.2），**不是**
  `DESIRED_WATCH_COMMITTED`——后者是广播帧，客户端按权威处理，重放一条
  旧 revision 的 COMMITTED 会与单调守卫打架。

## 4. authority transition actor

### 4.1 唯一执行者

单一本地 actor（单线程 inbox）是下述三步的唯一执行者，按 inbox 顺序逐项
完成：`CAS commit → manager reconcile → event enqueue`。
**新旧由 CAS 判定**，actor 只保证已提交 revision 的**投影顺序**。

### 4.2 必须进入 inbox 的转移

| 转移 | 说明 |
| --- | --- |
| 用户 mutation | §3.3 |
| `AttachAudience` / `DetachAudience` | **每一次**都入 inbox；连接计数与 `0↔1` 边沿**由 actor 自算** |
| Full Reset confirm | §2.4，同一 barrier |
| generation 轮换 / 压缩 | §2.3 |
| reconcile 重试完成 | 完成回执入 inbox 再广播 |
| 启动恢复 | 进程启动第一件事：读权威 snapshot → 有 audience 则物化 |
| **actor 故障重建** | supervisor 重建；入口 = 启动恢复；**为每个 section 分配新 epoch**；**并向所有现存 audience 广播完整双 snapshot**（v3 缺失，见下） |

**actor 故障重建必须重播 desired（v3 的洞）**：WS pump 活在 host 的
tokio 任务里，不在 actor 线程里，所以 actor 崩溃**不会**触发任何重连。
若重建只"物化"而不重播 desired，下述交错会让两个页面**永久**停在错误
状态：

```
A 提交 M1（START S）→ CAS 提交 rev6 + receipt（事务已 COMMIT）
actor 在 COMMIT 与 event enqueue 之间 panic
supervisor 重建 → 读 snapshot → 武装 S
WS 连接全程存活，无人重连 ⇒ 两个页面永远收不到 rev6
⇒ UI 显示"未监控"，实际正在监控并会响铃
```

同理，**任何"SQLite 已提交但广播未发生"的崩溃**都由重建时的完整双
snapshot 收敛。

### 4.3 epoch 作废规则

**分配新 `materializationEpoch`** 的触发（穷举）：

- 该 section 的 **`desired` 值发生变化**的 CAS 提交（`0→1` 或 `1→0`）；
- Full Reset / generation 轮换；
- 连接 `1→0`（销毁活跃态）与新的 `0→1`（重新物化）；
- actor 故障重建。

**明确不换 epoch**：**policy-only 更新**（`desired` 保持 1，仅
`policy_json` 变化）。它**保留 `activeWatchId` 与 episode**，通过 manager
的 `update_policy` 就地更新；通知模式变更沿用已批准的 `CUE_CANCELLED`
(`NOTIFICATION_MODE_CHANGED`) 语义。

v3 的 §4.3 写"任何 CAS 提交（含 STOP）"换 epoch，而 §5.4 又写"policy
更新保留 activeWatchId（不新起 epoch 的那类更新）"——后者指向一个前者
不承认的空集合。按 v3 字面执行，改一次阈值就会换 `activeWatchId`，
在同一次开放上产生重复响铃。本节即为该矛盾的裁定。

**每次 arm/disarm 尝试**分配新 `attemptToken`（§2.1），旧尝试的完成回执
一律丢弃。已批准设计里的"重连即作废旧 epoch"由连接 `0→1` 覆盖。

### 4.4 AttachAudience 的快照与顺序

- `MATERIALIZATION_SNAPSHOT` **在 `AttachAudience` 这一 inbox 轮次的
  开始处**从 owner map 序列化；
- **同一轮次内**产生的一切物化变更，作为事件**排在该快照之后**发出；
- 新连接必须**同时**收到 `DESIRED_WATCH_SNAPSHOT` 与
  `MATERIALIZATION_SNAPSHOT`——只发其一，页面无法区分"已提交但未武装"
  （§0 的准备中）与"未提交"。

## 5. 服务端拥有的监控与失败模型

### 5.1 逻辑 owner

- 本地引入 connection-independent 的 `LocalWatchOwner`，持有
  `section → { activeWatchId | null, generation, revision,
  materializationEpoch, attemptToken, armed, pendingDisarm, lastFailure }`；
- 浏览器连接只是**编辑者 + 事件 audience**；
- 每个 section 只产生一份 active watch、一份 episode 流、一份 cue；
- **持久提交成功即为真相**。

**owner 在 manager 里是一条合成连接，必须防止被心跳回收**（v3 缺失）：
manager 会清理 `last_touch` 超过 `heartbeat_timeout` 的连接及其全部
watch。owner 连接**必须在维护 tick 上被 touch**，或在 manager 侧显式豁免
过期——否则它拥有的每一份 watch 都会被静默收割。

### 5.2 失败分类（覆盖 manager 全部 disposition）

manager 的实际 disposition 见 `crates/bcsp-watch/src/effect.rs`。逐项：

| disposition | 分类 | 处置 |
| --- | --- | --- |
| `RejectedTargetUnavailable`、存储忙、快照未就绪 | **暂态** | `armed = false` + 退避 **5s/10s/20s 封顶 30s** 无限重试；desired 不动 |
| `RejectedSectionNotFound` | **取决于 gate** | S1 gate 放行 → 永久；gate Hold/隔离 → 按 `PENDING_GATE` 暂态重试 |
| `RejectedUnsupportedTarget`、`RejectedTermOutOfRange` | **永久** | §5.5 的系统 CAS 移出 desired + 通知 |
| `RejectedLimit`（`MaxActiveWatches`） | **永久** | 同上。已批准设计把"超 9 门"归入 admission 类永久拒绝 |
| `RejectedDuplicate`（`AlreadyActive`） | **不是失败** | 该 section 已被 owner 持有：直接视为 arm 成功，采用既有 `activeWatchId` |
| disarm 返回 `UnknownWatch` | **不是失败** | 目标已不存在即达成目的，视为 disarm 成功 |

退避与 retry-epoch 的原始出处：
`docs/design/2026-08-20-alert-delivery-integrity.md` **第 57–61 行**
（v3 只引了 214/250，那两行是"永久拒绝移出 desired"的出处）。

### 5.3 disarm 的寻址与失败

- **disarm 按 `section` 寻址**（v3 未定义）：owner 自己解析当前
  `activeWatchId` 再调 manager。**绝不能**用调用方手里的旧 id——
  §4.3 的 epoch 翻新意味着旧 id 早已不是 manager 认识的那个，会得到
  `UnknownWatch`；
- `desired = false`、Full Reset、连接 `1→0` 三种 disarm 失败按同一退避
  重试至成功；
- 重试期间 `pendingDisarm = true`，**对外报告 `armed = true,
  pendingDisarm = true`，绝不谎报已停**（§0 的"停止中"）；
- 旧 attempt 的任何完成回执一律丢弃，不得复活。

### 5.4 activeWatchId 与 episode 生命周期

- `activeWatchId` **可空**，首次 arm 成功前为 `null`；
- **每个新 `materializationEpoch` 分配新 `activeWatchId`**；
  policy-only 更新**不换**（§4.3）；
- **历史身份不变**：持久化的 episode 身份是既有的
  `(section_key, run_id, episode_id)`
  （`crates/bcsp-local-user-state/src/model.rs` 的
  `EpisodeHistoryIdentity`，表 `personal_episode_summaries_v1`）。
  v3 声称的"键仍为 `(activeWatchId, episodeId, observationId)`"**在本仓库
  不存在**，而且 §8 自己禁止个人表出现 `active_watch` 字样。本设计
  **不改动**历史身份；
- cue 的 exactly-once 由 manager 既有的 per-episode audible 计数与
  "owner 是唯一生产者"共同保证——**不依赖任何新持久键**。

### 5.5 永久失败：由 authority 自身走一次系统 CAS

已批准设计（`alert-delivery-integrity.md` 第 214/250 行）的"永久拒绝
（admission 类）→ 移出 desired + 常驻通知"**继续有效，不作产品修订**。
落地方式：

- authority **自行提交一次 `desired = false` 的 CAS**：`basedOnRevision`
  取**读到的当前 revision**，`mutationId` 由服务端新铸（UUIDv4，与客户端
  同一空间，落 receipt），`source = SYSTEM`；
- **`DESIRED_WATCH_COMMITTED.mutationId` 因此始终非空**；`source` 字段
  区分 `USER` / `SYSTEM`；
- **系统 CAS 失败（并发丢失）时重读重试**——§3.2 的"冲突即终止"约束的是
  **客户端**，不约束 authority；authority 是权威，它重试到收敛为止；
- 若该 section 在重读后已被用户自己删除，则目的已达成，不再提交。

### 5.6 零连接暂停（裁定采纳）

- `0→1`：读权威 snapshot 并物化一次（新 epoch）；
- `1→0`：只销毁 active manager 状态，**保留 desired**；
- `ACTIVE_WATCH_STATE_PERSISTENT = false` 继续成立；
- 两个转换都走 §4.2 的同一 inbox。

### 5.7 9 门帽只有一个执行点

v3 有**两个不同的计数器**：CAS 数 `desired = 1` 行，manager 数**该连接**
的 watch 数（`MAX_ACTIVE_WATCHES = 9`）。二者在"disarm 挂起"时必然发散：

```
9 门已武装 → 用户 STOP S1 → CAS 提交 desired=0 并广播（UI 显示已停）
           → disarm 进入退避，manager 仍持有 9 份
用户立刻 START S10 → CAS 只数到 8 行 → 通过 → 广播 COMMITTED（UI 显示已监控）
           → actor reconcile → manager 返回 RejectedLimit
```

**裁定：CAS 是唯一执行点**。manager 侧对 **owner 这条合成连接豁免**该
上限（需要 manager 侧一个明确的 API/标志）。理由：authority 已在事务内
基于唯一真相判定，manager 再判一次只会引入第二个真相。

## 6. 契约

### 6.1 命令（本地专有，见 §7）

```
SET_DESIRED_WATCH {
  section, desired, policy | null,
  basedOnRevision, authorityGeneration, mutationId
}
```

### 6.2 结果与广播

```
DESIRED_WATCH_COMMITTED {        // 广播给所有本地 audience
  section, desired, policy, revision, authorityGeneration,
  mutationId, source: USER | SYSTEM
}
DESIRED_WATCH_REPLAYED {         // 只回提交者；receipt 重放，非广播帧
  section, mutationId, outcome
}
DESIRED_WATCH_REJECTED {         // 只回提交者；投影用，非重试令牌
  section, mutationId, reason,
  current { desired, policy, revision, authorityGeneration,
            materializationEpoch, armed, pendingDisarm }
}
AUTHORITY_UNAVAILABLE { mutationId }               // 无状态字段；非终局
DESIRED_WATCH_SNAPSHOT { authorityGeneration, entries[] }
MATERIALIZATION_SNAPSHOT { authorityGeneration, entries[] }
  // entry: section, armed, pendingDisarm, activeWatchId | null,
  //        materializationEpoch, attemptToken, classification | null
DESIRED_WATCH_MATERIALIZED {
  section, authorityGeneration, revision,
  materializationEpoch, attemptToken, activeWatchId, pendingDisarm
}
DESIRED_WATCH_UNARMED {
  section, authorityGeneration, revision,
  materializationEpoch, attemptToken,
  classification: TRANSIENT | PERMANENT | PENDING_GATE, reason
}
```

`pendingDisarm` **必须**出现在 `MATERIALIZATION_SNAPSHOT` 条目与
`DESIRED_WATCH_MATERIALIZED` 上——否则"停止中"与"监控中"在 wire 上逐字节
相同，§0 的投影表无法实现（v3 的洞）。

### 6.3 bootstrap

- `desiredWatchAuthorityGeneration` 是 **snapshot 顶层 required 字段**；
- `desiredWatches` 条目携带 `desired` / `policy | null` / `revision` /
  `materializationEpoch`，**包含 tombstone**；
- bootstrap **不含** materialization 运行态（随连接由
  `MATERIALIZATION_SNAPSHOT` 下发）。

### 6.4 generation 轮换

Full Reset 与任何 generation 轮换必须广播完整
`DESIRED_WATCH_SNAPSHOT` + `MATERIALIZATION_SNAPSHOT`；客户端按 §2.1 的
两个元组单调合并，丢弃任何更旧 generation 的残留。

### 6.5 数值边界与 scalar 归属

三个计数量在 Rust 侧为 `u64`、JS 侧为 `number`，统一收敛到
**`≤ Number.MAX_SAFE_INTEGER`**。三处钉死：SQLite CHECK、**本地专有**
契约 scalar、前端本地专有校验器。**scalar 定义不得放进
`bcsp-contracts`**——该 crate 在公网闭包内（§7.1）。

## 7. 放置、拓扑与公网边界

### 7.1 Rust 放置

`bcsp-contracts` 与 `bcsp-application` **都在公网 Cargo 闭包内**
（`verify-rust-graph.mjs` 自 `bcsp-server` 可达性计算，12 包）。因此
desired 命令、事件与 authority **全部放在本地专有 crate**；共享 host 只
经 **tag 无关的 `WebSocketExtension`** 承载。

### 7.2 secondary 路由：受校验的**路由集合**

现状：host 只有单槽 `secondary_socket`（状态字段、构造器参数、路由注册与
`handle_secondary_socket` 各一处；**按符号名引用，不按行号**——行号已被
证实漂移），而已批准的 presence 已占 `/api/v1/local/presence`。

**裁定：改为受校验的 secondary 路由集合**，不合并为单条 local-control
socket。理由：presence 关心"哪些 tab 可见、谁响铃"，desired audience
关心"哪些连接在编辑与收事件"，语义不同；合并会迫使 desired 复用 presence
已冻结的 HELLO/期限语义并耦合失败面。本地是 loopback 单用户，两条 WS 无
容量压力；公网结构上没有这些路由。

实现要求（v3 只写了"改成 Vec"，不足以工作）：

- `HostState` 需要 **`path → Arc<dyn WebSocketExtension>` 映射**，
  `handle_secondary_socket` 按 `request.uri().path()` 选择——单槽字段读法
  在多路由下无法选中正确 extension；
- `SecondaryWebSocketRoute::new` 目前**不可失败**，需改为 fallible 以承载
  S2a 路径校验；`LoopbackServerError` 需新增变体；
- **集合内路径唯一性校验是必需项而非美观项**：axum 的 `Router::route`
  在重复路径上 **panic**；
- 路径冻结：presence `/api/v1/local/presence`、desired
  `/api/v1/local/desired-watch`。

### 7.3 既有携带项绑定为硬验收

- **S2c**：共享 `serve_websocket` 补 **64 KiB** 帧/消息上限；
- **S2b**：未注入 → **404**，集合化后**逐路由**钉死。

### 7.4 N-g 措辞更正

"公网完全不触碰 manager"在现有 pump 下不可达（pump 在解码**之前**调用
`transport_activity`）。N-g 改述为：

> 公网严格解码器把 desired tag 当未知命令拒绝：**不进入命令路由、不改变
> watch 状态、不产生 dispatch/sink/reply**。

### 7.5 旧命令去留

| | `START_WATCH` / `STOP_WATCH` / `UPDATE_POLICY` |
| --- | --- |
| 公网 | **保留**现有 ephemeral、connection-owned 语义，不变 |
| 本地 | **外部协议拒绝/退役** |
| 内部 | 物化器通过 **Rust API** 调用 manager |

**`SharedWatchSocket` 目前不暴露任何 arm/disarm/update Rust API**，需新增
一组 **tag 无关**（命名不得出现 desired 语义）的方法；它位于公网闭包且在
API 审计面上，命名与文档需按公网中立措辞。

### 7.6 前端拆分走 runtime port，而不是"共享 provider 不再发"

v3 写"共享 provider 不再发这三条"**与 §7.5 直接冲突且不可实现**：
`LiveWatchProvider` 是这三条命令的**唯一**发送方，而它被**两个 target
共用**（`entry.public.tsx → PublicCompositionRoot → SharedApplication →
LiveWatchProvider`）。剥掉发送会让公网失去启停能力。

真正存在的接缝是注入的 **`ProductRuntimePort`**（provider 通过
`runtime.watch.send` 发送，端口由各 target 的 bootstrap 构造）。因此：

- **本地的 runtime port 把这三个意图路由到 desired-watch CAS 通道**；
  公网的 port 继续发 wire 命令；
- 共享 provider 的形状不变；
- 负控三件套：本地专有 import 图约束、capability manifest、
  **public bundle 断言**——public 产物中不得出现任何 desired marker。

### 7.7 public marker 计账（v3 未言明）

`public-source-deny.json` 现有 **18 行 capability**，且
`EXPECTED_CAPABILITY_COUNT = 18`、`PUBLIC_SOURCE_MARKER_COUNT = 212`
与两个 SHA-256 摘要分别硬编码在 `verify-public-rust-zero-surface.mjs`、
`verify-rust-graph.mjs`、`frontend/tools/verify-import-graph.mjs`。

**裁定：不新增 capability 行**（新增会变成 19/19，与 N-g 的"保持
18/18"自相矛盾），改为**在既有 `PERSISTENT_ACTIVE_WATCH` 行上追加
marker**；随之必须同步：`markerSetVersion`、两处 212 常量、两个 SHA-256
摘要。N-g 的措辞相应为"**capability 行保持 18/18**，marker 数与摘要按规则
更新"。

## 8. 迁移 10004 与同步面

- 重建 `personal_desired_watches_v1`（§2.2），新建
  `personal_desired_watch_receipts_v1`（§3.4）；
- 10003 遗留行升级为 `desired = 1`，按 `(term, campus, index)` 升序从 1
  起分配 `revision` 与 `materialization_epoch`；
- metadata 写入 generation = 1、两个计数器 = 已分配最大值；
- 迁移在既有 personal runner 的**单事务 + 可选 Rust after-hook** 模式内
  完成（0002 已有先例）。

**同步面（v3 只列了一半）**：两个 allowlist、`PersonalTableCounts`、
`PersonalResetResult`、schema 子串守卫（新表同样不得含
connection/active_watch 字样）、Rust `PersonalStateSnapshot` 与
`DesiredWatch` 形状、`bcsp-local-runtime` 的 bootstrap 编码，**以及
`frontend/src/ui/local/personal/contracts.ts`**——后者有三处硬阻断：

1. `desiredWatches.length <= 9` 的帽：§2.3 保留 tombstone 到 Full Reset，
   **第 10 个 tombstone 就会让整个 bootstrap 解析失败**。该帽必须改为
   "`desired = 1` 的条目数 ≤ 9"，总条目数不设产品帽；
2. `hasKeys` 是**键集全等**检查：新增顶层
   `desiredWatchAuthorityGeneration` 会打挂 `isPersonalStateSnapshot`，
   每条目新增 `desired`/`revision`/`materializationEpoch` 会打挂
   `isDesiredWatch`；
3. 上述改动需同步三处 bootstrap fixtures（S2-PR3.1 的先例）。

## 9. 验收映射

清单 S2-D3 的 a–d 继续有效；e–i 中 **f、i 继续有效**，**h 改写为"reset
进入同一 authority transition barrier"**，e、g 改写。

| 条目 | 本模型下的形态 |
| --- | --- |
| a) STOP 后延迟旧 START | `basedOnRevision` 落后 → `STALE_REVISION`；tombstone 永不回收 |
| b) 新 START 后延迟旧 STOP | 同上，对称 |
| c) 旧 policy 覆盖新 policy | 同上 |
| d) reset 升代后前代写入 | `STALE_GENERATION` |
| e) 顺序基准 | 改写：客户端携带的 `(generation, revision)` 经 CAS 判定 |
| f) 旧命令必须实际迟到 | 继续有效 |
| g) 临界区不得横跨 SQLite | 改写：CAS 在 socket 锁之外的 authority actor 内 |
| h) reset barrier | 改写：reset 进入同一 authority transition barrier |
| i) 首个生产调用方与全部反例测试同 PR | 继续有效 |

验收项 N-a..N-n（v2/v3）保留，v4 新增：

- **N-o**：UI 投影表（§0）逐行钉死，尤其"desired=1 ∧ armed=false ⇒
  准备中、非绿"与"pendingDisarm ⇒ 停止中"；
- **N-p**：同一 mutationId 用于两个不同 section ⇒ `MUTATION_ID_CONFLICT`，
  不得重放另一 section 的回执；
- **N-q**：终局拒绝后原因消失（腾出名额 / gate 放行），用户**新手势**
  （新 id）必须能成功——即 §3.1 的"新铸"规则；
- **N-r**：actor 故障重建向现存 audience 广播完整双 snapshot；
  "COMMIT 后 panic"交错下两页面收敛到 rev6；
- **N-s**：Full Reset 后 epoch 计数器归零，armed 侧仍按
  `(generation, epoch, attempt)` 正确排序；重置前在途的 epoch-1 完成
  回执不得复活 watch；
- **N-t**：`RejectedDuplicate` 视为成功、disarm `UnknownWatch` 视为成功；
- **N-u**：9 门帽单一执行点——"STOP 挂起 + 立刻 START 第 10 门"不得出现
  UI 显示已监控而 manager 拒绝；
- **N-v**：owner 合成连接不被心跳回收；
- **N-w**：policy-only 更新不换 `activeWatchId`、不重复响铃；
- **N-x**：迟到的 `DESIRED_WATCH_REJECTED` 不得把已前进的前端投影拉回
  （§3.2 服从单调守卫）。

## 10. 仍待裁定 / 开放

- **打包 E2E 非空路径门保持开放**。本地专有 desired 路由落地后，冒烟可
  直连提交一次 CAS；但**要验 `desired = true`，冒烟环境必须先播种一个
  已发布 section**，否则准入直接拒，只有 `desired = false` 的弱覆盖。
  播种方案属独立工作项，请裁定优先级。
- **已批准设计的测试 1c（"leader 关闭 → 另一 tab 接管并 re-arm"）应声明
  被本设计取代**：本模型下 watch 由服务端 connection-independent 持有，
  leader 转移不触发任何 re-arm（§1）。请确认后在该文档标注。
