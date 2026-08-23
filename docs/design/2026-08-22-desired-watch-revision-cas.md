# 期望监控：revision/CAS 共同编辑模型（S2-D3 路线改判）

> **状态：SUPERSEDED（2026-08-23）。**
> 由 `2026-08-23-desired-watch-reduced-scope.md` 取代。
>
> 产品所有者澄清："像 B 站那样"指的是**多页面共同编辑同一份服务端
> 状态**，B 站的语义是"另一个页面操作后本页面需要刷新才看得到"，
> **不要求实时推送**。本文把它读成了"共同编辑 + 实时推送"，并据此建了
> 投影帧流、分块组装、效果批次双向确认、actor 化身编号、听众登记表与
> 响铃 leader 选举——**这些全部作废**。误读是实现方的，不是需求的。
>
> **仍然有效并已实现**的部分：§2.1 的 generation/revision/epoch、
> §2.3 desired 表、§2.4 receipt ledger、§2.5 资源边界、§2.6 rotation、
> §2.7 Full Reset、§3 CAS 五步、§5.5 失败分类、§5.6 系统 CAS、
> §5.6.1 零 audience 暂停、§5.7 两个帽、§7.1/§7.4/§7.5 公网边界。
> 本文保留，是因为上列机制的推导过程写在这里。
>
> **已作废**：§0.3 第 0 条、§0.5、§2.2、§4、§5.2 及其子节、§5.3 的
> effect 相关签名、§5.8、§6.1/§6.2 的 WS 命令与事件、§7.2 的第二条
> WebSocket 路由、§7.6 的 `subscribeProjection`、§9 中依赖上列机制的
> 验收条目、§10、§12 中依赖帧流的门。

状态（历史）：**v7.2（已获批准开工，限 dormant）**。
复审 2026-08-23 裁定：架构与四项核心修复已达实施水位，**不再需要
v7.3/v8 文档循环**；PR5 可立即开工，但**严格限定为 dormant 实现**——
批准开工**不等于**允许单独激活/部署，**S2-D3 不关闭**。
合并前硬验收见 **§12**。路线由产品所有者 2026-08-22 裁定：取代 fenced
sequencer，走 **持久化 revision + tombstone + CAS**。
S2-D3 硬门在本设计实现并复审通过前**保持关闭**。

**wire 名冻结**：`authorityGeneration`；**所有本地 wire variant 一律以
`DESIRED_WATCH_` 开头**（§7.7 孤儿 tag 规则）——因此 v7 的
`SET_DESIRED_WATCH` 更名为 **`DESIRED_WATCH_SET`**，字面与规则一致。

**修订史**：v1 五组 → v2 四组 → v3 自查十一项 → v4 五组 → v5 自查十五项
→ v6 三组 → v7 五组 → v7.1 四组 → **v7.2 窄修逐条闭合**（复审明示无需
v8 大循环；本轮只冻结行为契约，类型/锁/channel/函数签名转入 PR 验收）。
**v6 已获批准且不再重开**：§0.3 的同步态判定顺序、§2.5 六个首发数值、
§7.7 的 16 项数组/去重论证/`212+3=215`。

## 0. 唯一要约束的东西：UI 投影

### 0.1 物化记录与 `armed`

owner 为每个 section 持有一条**可空物化记录**：

```
materialized: { generation, revision, epoch, policy, activeWatchId } | null
```

```
armed(section) ≡ materialized ≠ null
  ∧ materialized.generation = authorityGeneration
  ∧ materialized.revision   = sectionRevision
  ∧ materialized.epoch      = materializationEpoch
  ∧ materialized.policy     = desiredPolicy
```

**`armed` 是派生量，不上 wire**（v6 把它当冗余布尔值发送）。前端按本式
自行计算；服务端内部同式计算。冗余布尔一旦与物化字段不一致，页面就会
盲信一个谎——正是本纲领禁止的。

### 0.2 三个正交状态位

| 位 | 含义 | 上 wire |
| --- | --- | --- |
| `pendingDisarm` | **本 section 自己**的拆除在退避重试中；物理 watch 仍活、仍可能响铃 | 是 |
| `blockedOnSlot` | **本 section 的 arm 在等物理槽**（别的 section 的拆除占着） | 是 |
| `armed` | §0.1 | **否，派生** |

### 0.3 投影判定（**有序**求值，第一条命中即止）

```
0. ¬composite-synced  → 「重连中 / 同步中」，非绿，
   **禁止用缓存 revision 发 mutation**
   （composite-synced ≡ **两条 socket 均 OPEN** ∧ **首个 FULL 已应用**
    ∧ **非 DESYNCED** ∧ **非 ASSEMBLING**；raw OPEN 不等于已同步）
1. pendingDisarm      → desired=0 ? 「停止中」 : 「准备中（旧槽拆除中）」   非绿
2. blockedOnSlot      → 「准备中（等待旧槽释放）」                          非绿
3. desired = 0        → 「未监控」                                          —
4. desired = 1 ∧ armed→ 「监控中」                                          **绿**
5. desired = 1 ∧ ¬armed→「准备中」（附 classification 文案）                非绿
```

第 0 条是 v6 缺失的最高优先级规则：**投影只在帧流可信时才有意义**。
未同步时用缓存 revision 发 mutation 只会撞出无意义的 `STALE_REVISION`
（而按 §3.1 那是终局的，用户手势就此作废）。

### 0.4 状态不变量

```
blockedOnSlot ⇒ desired = 1 ∧ materialized = null ∧ ¬pendingDisarm
```

- `desired = false` 的 CAS **原子清除** `blockedOnSlot`；
- `DESIRED_WATCH_SLOT_RELEASED` 唤醒后**必须重读当前 revision/desired**
  再决定是否 arm——事件到达时用户可能已经改主意了。

### 0.5 投影原子送达

单一信封 `DESIRED_WATCH_PROJECTION_UPDATE`（§6.2），前端 reducer 整帧
提交，中间态永不渲染。

## 1. 产品模型（裁定）

所有标签页平权编辑；`desired_watches` 是唯一真相；实际监控由服务端统一
维护；leader 只负责"避免多个页面同时响铃"这一件用户看不见的事。

## 2. 权威状态与帧流

### 2.1 单调量与守卫

| 量 | 分配 | 持久 | 排序对象 |
| --- | --- | --- | --- |
| `authorityGeneration` | Full Reset / rotation | metadata | 全表世代 |
| `sectionRevision` | CAS 提交 | 全局计数器，落每行 | desired 状态 |
| `materializationEpoch` | desired **值**变化 / 连接边沿 / rotation / actor 重建 | 全局计数器，落每行 | armed 身份 |
| `armAttempt` | 每次 arm / policy 尝试 | 不持久 | 该 section 当前尝试 |
| `disarmOperationId` | 每个 disarm **操作** | 不持久（但被 manager 的 effect batch 持有） | 独立寻址的拆除 |
| `actorIncarnation` | 每次 actor 启动/重建 | **metadata 标量** | 帧流化身 |
| `transitionId` | **每个广播帧**（见下） | 不持久，每化身内**从 1 连续** | 帧顺序 |
| `operationId` | 每个 owner 操作 | 由 manager 的 effect batch 持有 | 副作用幂等 |

**守卫**：desired 侧 `(authorityGeneration, sectionRevision)`；armed 侧
`(authorityGeneration, materializationEpoch, armAttempt)`；帧顺序
`(actorIncarnation, transitionId)`。

**`transitionId` 只由广播帧消费**（v6 的洞）：Attach 与
`REQUEST_FULL_PROJECTION` 的 FULL 是**定向投递**，若也占用全局序号，
就会出现——A 最后收到 T10；B Attach 只有 B 收到 T11；下一次广播是
T12；**A 误判缺帧并触发无意义重同步**。裁定采用第三种方案：

> **定向 FULL 不消费全局序号**，改为携带
> **`throughTransitionId`** 作为基线：它表示"本快照已包含到该广播序号
> 为止的全部变更"。接收方据此设定自己的 `last`。

`armAttempt` 与 `disarmOperationId` 分离（v6 用单一 `attemptToken`）：
一个 section 上可以**同时**存在"新 epoch 下的 arm 尝试"与"必须继续完成
的、按捕获 id 寻址的旧 disarm 操作"，单一 token 无法同时表达两者。

### 2.2 接收状态机（**冻结**）

| 规则 | 内容 |
| --- | --- |
| R1 | **首帧只能是 FULL**；在此之前处于 SYNCING（§0.3 第 0 条） |
| R2 | DELTA 必须**恰为 `last + 1`**；否则进入 **DESYNCED** |
| R3 | **同化身的 FULL 即使跨 gap 也作为权威替换**，并把 `last` 设为其 `throughTransitionId`（定向）或自身 `transitionId`（广播） |
| R4 | **新化身只接受 FULL**；收到新化身的 DELTA 即 DESYNCED |
| R5 | **FULL 必须删除帧中缺席的本地 entry**（否则删除永远同步不下去） |
| R6 | DESYNCED 时发 `DESIRED_WATCH_REQUEST_FULL_PROJECTION`，**single-flight**；超时按 1s/2s/4s 封顶 30s 退避重试，连续 5 次失败则主动断连重连。**被限流时服务端回 `DESIRED_WATCH_RESYNC_REJECTED { retryAfterSeconds }`（不是静默丢弃）**，客户端下次重试取 `max(退避, retryAfterSeconds)`——否则 R6 的重试时序不可确定 |
| R7 | 首个 FULL 亦受 R6 的超时/退避约束 |

**分帧与"每轮至多一帧"的统一**：一个 actor 轮次产生**至多一个投影
更新**；若序列化超过 §2.5 的**单块**字节帽，则**分块**为多个 wire 帧。
分块不是多帧，`transitionId` 不递增。

**分块规则（冻结）**：

| 规则 | 内容 |
| --- | --- |
| **C0** | **整组原子提交**（v7 有、v7.1 误删，此处恢复并加强）：全部块通过校验、集齐、**按 `chunkIndex` 升序拼接**后**只提交一次**；**部分组绝不触碰 store**。**R5 只对完整 FULL 组的 entry 并集执行一次**——否则 `[A]`、`[B]` 两块会互相把对方删掉 |
| **C0b** | **首块到达即进入 ASSEMBLING**（并入 §0.3 第 0 条：非绿、禁止发 mutation）。**不是等 10s 超时后才降级** |
| C1 | 每个投影更新有一个 **`frameGroupId`（UUIDv4）**；同组各块共享它。不同 `frameGroupId` 的块**一律不得混装** |
| C2 | 块头严格校验：`chunkIndex < chunkCount`、同组 `chunkCount` 一致，且同组 **`actorIncarnation` / `kind` / `transitionId` / `authorityGeneration` / `throughTransitionId`** 五元组全部一致；任一不符 → 丢弃该组并进入 **DESYNCED** |
| C3 | **重复 `chunkIndex`** → 丢弃该组并 DESYNCED |
| C4 | **旧 incarnation 的块**一律丢弃（不影响当前组） |
| C5 | **组装超时 10s**（自首块起）未集齐 → 丢弃该组并 DESYNCED。这是"actor 中途崩溃、末块永不到达"时页面不会**永久保留旧绿态**的唯一保证 |
| C6 | **同时只允许装一个组**。在装期间到达**异 `frameGroupId` 的 DELTA 组** → 丢弃**两者**并 DESYNCED（DELTA 之间无权威性可言，不能择一） |
| C7 | **FULL 组作废在装组**，但**仅当它更新**：`(actorIncarnation, throughTransitionId 或 transitionId)` 不低于在装组；**更旧的同化身 FULL 一律丢弃**，**不得回退 `last`**、**不得作废更新的在装组** |
| **C8** | **每组容量帽**：单组**总字节 ≤ 256 KiB**、**块数 ≤ 16**；任一超出 → 丢弃该组并 DESYNCED。32 KiB 是**单块**帽，**不约束组装内存** |

### 2.3 desired 表（迁移 10004 重建）

| 列 | 说明 |
| --- | --- |
| `term_id` / `campus_code` / `section_index` | 主键 |
| `desired` | `INTEGER NOT NULL CHECK (desired IN (0,1))` |
| `policy_json` | `TEXT NULL`；`CHECK ((desired = 1) = (policy_json IS NOT NULL))`，`CHECK (policy_json IS NULL OR json_valid(policy_json))` |
| `revision` | `INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991)` |
| `materialization_epoch` | `INTEGER NOT NULL`，同上界 |

metadata 四个持久标量，**四者都带同一上界 CHECK**：
`desired_watch_authority_generation`（初值 1）、
`desired_watch_revision_counter`、`desired_watch_materialization_counter`、
`desired_watch_actor_incarnation`（初值 1）。

### 2.4 receipt ledger

主键 `(authority_generation, mutation_id)`；`term_id`/`campus_code`/
`section_index` 与 `fingerprint` 为**被比较列**；`outcome_json` 带
`json_valid` CHECK。

- 主键命中 + section 与 fingerprint 均同 → **重放**（只回提交者，不重新
  CAS、不 reconcile、不广播）；
- 主键命中但 section 或 fingerprint 不同 → **`MUTATION_ID_CONFLICT`**，
  **由已存在行推导，不插第二条**；
- **只记客户端提交**的成功与终局拒绝；**`STALE_GENERATION` 不记**；
  **系统 CAS 不记**（否则是绕过路由限流的无界产出源）。

### 2.5 资源边界（**首发值已获批准**）

| 项 | 值 |
| --- | --- |
| desired 路由 mutation 限流 | 令牌桶 **2/s，突发 60** |
| `DESIRED_WATCH_REQUEST_FULL_PROJECTION` 限流 | **独立**令牌桶 **0.2/s，突发 3**，且同一 audience 的并发请求**合并**为一次序列化 |
| tombstone 行数硬上限 | **512** |
| receipt 行数硬上限 | **2048** |
| FULL / DELTA **单块** wire 帧上限 | 各 **32 KiB**（**单块帽**；逻辑更新可跨块） |
| bootstrap 序列化上限 | **32 KiB**（单次 HTTP 响应，不分块） |
| **单个投影更新组**上限 | **总字节 256 KiB / 块数 16**（§2.2 C8；单块帽不约束组装内存） |
| rotation 触发阈值 | 任一预算达 **80%** |

**执行合同（v6 缺失）**：

- **字节数按完整 UTF-8 信封计算**（含 envelope 与分块头），不是 payload；
- **80% 取整为 `floor(cap * 4 / 5)`**；验收必须覆盖
  `threshold − 1` / `threshold` / `hard cap` 三点；
- **限流拒绝有 wire 结果**：`DESIRED_WATCH_MUTATION_RESULT` 的
  `REJECTED { reason: RATE_LIMITED, retryAfterSeconds }`；它是**非终局**，
  **同 id 允许重试**（与 `AUTHORITY_UNAVAILABLE` 同类，不落 receipt）；
- **rotation 保留 `actorIncarnation`**；**Full Reset 只增 generation、
  清零 revision 与 materialization 计数器**，
  **不得在同一化身内重置 `transitionId`**。

### 2.6 rotation（原子；不丢意图；不摧毁健康 watch）

同一个 SQLite 事务内：generation +1 → **保留并重写全部 `desired = 1`
行**并分配新代 `revision`/`materialization_epoch` → **只删** tombstone 与
receipt → 修复计数器。提交后广播一帧 FULL。
**换 epoch 走 §5.2 的 adopt 路径**，不得 `Restarted`。

### 2.7 Full Reset

同一事务内真删两表全部行、generation +1、revision 与 materialization
计数器归零（`actorIncarnation` 与 `transitionId` **不动**）；提交后广播
一帧 FULL。

## 3. CAS

### 3.1 冲突后必须终止

`DESIRED_WATCH_MUTATION_RESULT` **不带任何状态**；
**`basedOnRevision` 只能取自投影 store**（冻结）。

客户端收到**终局**拒绝后必须终止该 mutation，**不得自动改号重发**；
只有用户的**新手势**才允许以**新 id** 提交。
（`AUTHORITY_UNAVAILABLE` 与 `RATE_LIMITED` 是**非终局**，同 id 重试。）

### 3.2 CAS 规则（单 `IMMEDIATE` 事务，顺序即校验序）

1. `authorityGeneration != current` → `STALE_GENERATION`（不记 receipt）；
2. receipt 幂等判定；
3. `basedOnRevision != 当前 revision`（无行为 0）→ `STALE_REVISION`；
4. **仅当 `desired = true`**：
   **产品准入帽按 post-state 判定——提交后 `desired = 1` 的行数 ≤ 9**
   （v6 写"当前 count < 9"，会让**满 9 门时的 policy-only 更新被误拒**；
   只有 `0→1` 才占额）；
   target 受支持 / term 在窗口内 / section 已发布**且 S1 gate 已放行**；
   **gate 未放行时不得拒绝**——放行提交，由 arm 侧按 `PENDING_GATE`
   暂态重试；
   **`desired = false` 跳过全部准入校验**；
5. 写入、`revision = ++revCounter`、`materialization_epoch = ++epochCounter`
   （仅 `desired` **值**变化）、写 receipt、提交。

## 4. authority transition actor

### 4.1 帧与轮次

单线程 inbox，按序完成 `CAS commit → manager reconcile → 至多一个投影
更新`（可分块，§2.2）。无变化则不发帧，`transitionId` 不递增。

### 4.2 inbox 转移

用户 mutation；`AttachAudience`/`DetachAudience`；Full Reset confirm；
rotation；**`SlotReleased`**；**周期 reconcile**（§5.8）；reconcile 重试
完成回执；`REQUEST_FULL_PROJECTION`；启动恢复；**actor 故障重建**。

**actor 故障重建（顺序已按复审更正——v7 写反了）**：
`++actorIncarnation` → **先**向所有现存 audience 投递**新化身的 FULL**
（否则按 R4"新化身只接受 FULL"，随后重放的 effect 会被全部丢弃）→
**再**按**原顺序**重放未确认 effect batch（§5.2）。
**history 侧独立恢复**，不依赖 audience 的进度。

**audience registry 必须活在 actor 之外**（v6 的洞）：持久化身号只解决
排序，**不能让重建后的 actor 凭空找回仍然存活的 pump**。registry 由
host 侧持有（与 socket 生命周期同寿），actor 重建时从中读取当前
audience 集合；每条 audience 记录带**化身戳**，重建后统一标记为
"需 FULL"。

### 4.3 epoch 作废

**换新 epoch**：`desired` **值**变化的 CAS；Full Reset；rotation；连接
`1→0` 与新 `0→1`；actor 重建。其中 **rotation 与重建走 adopt**。
**不换 epoch**：policy-only 更新（经 §5.2 的 `applyPolicy`）。

### 4.4 AttachAudience

定向 FULL 在该轮次开始处从 owner map 序列化，**携带
`throughTransitionId`、不消费全局序号**；该轮触发的物化变更由 actor
**自行入队** `MaterializeDue`，在其后的轮次以广播 DELTA 发出。

## 5. 服务端拥有的监控与失败模型

### 5.1 逻辑 owner

`LocalWatchOwner` 持有
`section → { generation, revision, materializationEpoch, materialized|null,
armAttempt, pendingDisarm, blockedOnSlot, lastFailure }`，外加一组
**独立的 disarm 操作**（按捕获的 `activeWatchId` 寻址）。

owner 在 manager 中是内部合成连接 **`ConnectionKind::Owner`**，
**不可由 wire 请求构造**；**豁免心跳过期**，**不豁免物理 watch 帽**。

### 5.2 owner 操作身份与 effect batch（v6 的核心洞）

**freshness token ≠ effect idempotency**。反例：

```
ensure(G,E) 已在 manager 建出 activeWatchId / episode / cue
actor 在 history 与 audience 交接之前崩溃
重建后按"同 (G,E) → adopt、零副作用"处理
⇒ 投影恢复了，但那一份 cue 与 history 永久丢失
⇒ 若用户重新 START，manager 会走 Restarted，产生全新 ID/episode/cue
```

因此：

- 每个 owner 操作分配**稳定的 `operationId`**（UUIDv4），**在调用
  manager 之前**分配；每个 batch 有稳定的 **`effectBatchId`**，其中
  effect **有序**且带下标；
- manager **保留该 effect batch**（原 action / episode / cue 的 ID 与
  内容）**直到 actor 确认 history 与 audience 两侧**——模式参照 manager
  既有的 message replay cache；
- actor 重建时按 §4.2 的顺序**用原 ID 重放**，不新建；
- `ensure` 的 **adopt 必须逐项校验**：同一 owner、同一 section、
  `activeWatchId` **仍驻留 manager**、policy 相同、**没有针对该 ID 的
  disarm 操作**、**没有未确认 effect batch**。任一不满足即不得 adopt；
- **存在未确认 batch 时只能走 `PENDING_HANDOFF → replay`，
  绝不允许落入 `Restarted`**。

#### 5.2.1 "确认"的可执行定义（v7 只说"双确认"，没说什么算确认）

现状：history 接缝返回 `void`、SQLite 失败只记日志；audience 侧的
"发送成功"只到达无界发送队列，**并不代表浏览器收到**。因此冻结：

| 面 | ACK 的确切含义 |
| --- | --- |
| **history** | **SQLite 事务已提交**，或写入被判定为 **`AlreadyPresent`**（幂等重放命中既有行）。除此之外一律**不算**已确认，按退避重试 |
| **audience** | 客户端显式回 **`DESIRED_WATCH_EFFECT_ACK { effectBatchId }`**。入队/`send()` 返回**不算**确认 |

- **必需 audience 集合** = **batch 创建时刻**已注册的 audience 集合；
  期间 **detach 的 audience 从必需集合中移除**（视为已满足）；集合变空
  则**仅凭 history ACK** 即可确认；
- **leader 语义与 ACK 无关**：leader 只决定**是否播放声音**，不决定
  是否 ACK，也不得截断事件；
- **客户端先按 `(effectBatchId, effectIndex)` 去重再发声**，随后 ACK。
  重放到达时若已去重命中，**仍须 ACK**（否则重放永不收敛）；
- **容量与背压**：未确认 batch **至多 64 个 / 合计 256 KiB**；达帽即
  **背压**（暂停新的 arm，`lastFailure` 分类为 TRANSIENT）；
- **GC 条件（精确）**：

  ```
  historyAck ∧ (每个 required audience 均已 ACK 或已 detach)
  ```

- **ACK deadline 与超时 detach（v7.1 的活性洞）**：仅有容量帽只解决
  内存无界，**不解决活性**——冻结或恶意页面可以保持连接却永不 ACK，
  最终把新 arm 全部堵死。因此每个 batch 对每个 required audience 设
  **ACK deadline 30s**；超时即**把该 audience 判为 detach**（移出
  required set 并触发一次 Detach 流程），batch 据此推进；
- **拆除路径必须有独立且有界的容量**：`STOP`、Full Reset、
  `1→0` 的 disarm 各自走**独立预算**（每类 16 个 / 64 KiB），
  **普通 effect backlog 不得阻塞物理拆除**——否则"停不下来"会变成
  比"没响"更糟的谎。

#### 5.2.2 effect 的 wire 形态（v7.1 只有客户端 ACK，服务端没带字段）

§5.2.1 要求客户端按 `(effectBatchId, effectIndex)` 去重，但 v7.1 的
服务端事件合同**不携带这些字段**。补**本地专有 wrapper**（**不进
`bcsp-contracts`**，marker 计数不变）：

```
DESIRED_WATCH_EFFECT {
  audienceBindingId,          // §7.6.1 的 pair 身份
  effectBatchId,
  effectSequence,             // batch 之间的全序
  effectIndex, effectCount,   // batch 内的序与总数
  projectionFence,            // 见 §7.6.1：应用前必须已达到的投影位置
  watchEvent                  // 被包裹的既有 watch 事件
}
```

**客户端集齐整个 batch 并按 `effectIndex` 顺序处理完之后才 ACK**；
命中去重仍须 ACK（§5.2.1）。

### 5.3 owner API（三个）

- **`ensure(section, generation, epoch, policy, armAttempt, operationId)`**
  - 同 `(generation, epoch)` 重试 → 收养原 `activeWatchId`，零新副作用；
  - **新 epoch 但物理 watch 健康**（rotation / 重建）→ **adopt**：仅重盖
    物化记录的 `(generation, epoch)`，**不走 `Restarted`**；
  - **新 epoch 且需替换** → 才允许 `Restarted` 语义；
- **`applyPolicy(section, generation, epoch, sectionRevision, policy,
  armAttempt, operationId)`**
  - **按 `sectionRevision` 作稳定操作键**；
  - **未武装 / `blockedOnSlot` 时只更新 desired 侧**，后续 `ensure`
    自然采用最新 policy；
  - **已武装时 compare-and-apply**；**同 revision 重试不得再写一次
    history**；
- **`disarm(section, targetActiveWatchId, epoch, disarmOperationId)`**
  —— 见 §5.4。

### 5.4 disarm

- **发起时捕获** `(section, targetActiveWatchId, epoch)`，退避重试**始终
  针对被捕获的那个 id**，**绝不在执行时重新解析**；
- epoch 变化**取消在途 arm 尝试**，**不取消在途 disarm**（旧 watch 仍须
  死）；
- 三种 disarm 失败按 **5s/10s/20s 封顶 30s** 重试至成功；期间
  `pendingDisarm = true`，**绝不谎报已停**；
- 成功后清 `pendingDisarm` 并投递 **`SlotReleased`**；
- `UnknownWatch` **视为成功**；
- **历史身份不变**：仍是 `(section_key, run_id, episode_id)`。

### 5.5 失败分类

| disposition | 分类 | 处置 |
| --- | --- | --- |
| `RejectedTargetUnavailable`、存储忙、快照未就绪 | 暂态 | 5/10/20 封顶 30s 无限重试；desired 不动 |
| `RejectedSectionNotFound` | 取决于 gate | 放行→永久；Hold/隔离→`PENDING_GATE` 暂态 |
| `RejectedUnsupportedTarget`、`RejectedTermOutOfRange` | 永久 | §5.6 系统 CAS 移出 desired + 通知 |
| `RejectedLimit`（owner）**且有 pendingDisarm** | 预期路径，不告警 | 置 `blockedOnSlot`，等 `SlotReleased` |
| `RejectedLimit`（owner）**且无 pendingDisarm** | 不变量告警 | 上报；仍**不**删除 desired |
| disarm `UnknownWatch` | 不是失败 | 已达成目的 |

**两类事件不可混淆**：CAS 的 `LIMIT_EXCEEDED` 是**面向用户的永久拒绝**
（与已批准设计"超 9 门属 admission 类"一致）；owner 上的
`RejectedLimit` 是**内部物理槽条件**。

### 5.6 永久失败：系统 CAS

提交一次 `desired = false`，`basedOnRevision` 取读到的当前值，每次重试
**新铸 id 但不记 receipt**，`source = SYSTEM`；并发丢失时重读重试；
若该 section 已被用户删除则不再提交。

### 5.6.1 零 audience 暂停（既有产品裁定，v7 误删，此处恢复）

- **`1→0`（最后一个 audience 断开）**：**拆除全部物理 watch**（走
  §5.4 的 disarm，捕获各自 `activeWatchId`），**保留 desired**；
- **`0→1`（首个 audience 接入）**：**从权威 snapshot 重新物化一次**
  （新 epoch，走 `ensure`/adopt）；
- `ACTIVE_WATCH_STATE_PERSISTENT = false` 继续成立：
  `activeWatchId` 与 episode 下次重新生成；
- 两个转换都走 §4.2 的同一 inbox。

### 5.7 两个帽

CAS 是唯一**产品准入帽**（post-state ≤ 9）；manager 保留**物理帽**且
owner **不豁免**（否则 `9 armed → 9 pendingDisarm → 再 START 9 个` 会得到
18、27、36 份）。旧槽被占时 CAS 照常成功、物化推迟、`blockedOnSlot`。

### 5.8 活性不依赖单一事件（v6 的洞）

`SlotReleased` **只是唤醒提示**，不是唯一活性来源。actor 另有**周期
reconcile**（30s）重新推导 `desired` 与 `materialized` 的差集并**重新
发现空槽**；重建后的首轮恢复同样执行一次。事件丢失不得造成永久停滞。

## 6. 契约（全部带 `DESIRED_WATCH_` 前缀）

### 6.1 命令

```
DESIRED_WATCH_SET { section, desired, policy|null,
                    basedOnRevision, authorityGeneration, mutationId }
DESIRED_WATCH_REQUEST_FULL_PROJECTION { }
DESIRED_WATCH_EFFECT_ACK { effectBatchId }          // §5.2.1 audience ACK
```

### 6.2 事件

```
DESIRED_WATCH_PROJECTION_UPDATE {
  actorIncarnation, authorityGeneration,
  kind: FULL | DELTA,
  transitionId | null,          // 广播帧有；定向 FULL 为 null
  throughTransitionId | null,   // 定向 FULL 有
  frameGroupId, chunkIndex, chunkCount,   // §2.2 C0–C8
  entries: [ DesiredWatchProjectionEntry ]
}

DesiredWatchProjectionEntry {
  section,
  desired, policy|null, revision, materializationEpoch,
  materialized: { generation, revision, epoch, policy, activeWatchId }|null,
  pendingDisarm, blockedOnSlot,
  classification: TRANSIENT|PERMANENT|PENDING_GATE|null, reason|null
}
// armed 不上 wire：由 §0.1 派生

DESIRED_WATCH_MUTATION_RESULT {   // 单播；不含任何状态
  mutationId,
  outcome: COMMITTED
         | REJECTED { reason, retryAfterSeconds? }
         | REPLAYED { outcome }
}

DESIRED_WATCH_AUTHORITY_UNAVAILABLE { mutationId }   // 非终局
DESIRED_WATCH_RESYNC_REJECTED { retryAfterSeconds }  // §2.2 R6 限流结果
DESIRED_WATCH_SLOT_RELEASED { section }              // 内部；不上 wire
```

`reason` ∈ `STALE_GENERATION` / `STALE_REVISION` / `MUTATION_ID_CONFLICT` /
`LIMIT_EXCEEDED` / `UNSUPPORTED_TARGET` / `TERM_OUT_OF_RANGE` /
`SECTION_NOT_FOUND` / **`RATE_LIMITED`**。
**两个概念必须分开（v7 把它们混为一谈，导致 A-14 与 A-06/A-07 冲突）**：

| 概念 | 含义 | 集合 |
| --- | --- | --- |
| **终局性** | 决定**客户端是否必须终止该 mutation** | **终局** = 上列七项；**非终局** = `RATE_LIMITED`、`AUTHORITY_UNAVAILABLE`（同 id 可重试） |
| **是否记 receipt** | 决定**重复请求能否重放** | **记**：成功、`STALE_REVISION`、`LIMIT_EXCEEDED`、`UNSUPPORTED_TARGET`、`TERM_OUT_OF_RANGE`、`SECTION_NOT_FOUND`；**不记**：`STALE_GENERATION`（步骤 1 即判定）、`MUTATION_ID_CONFLICT`（由既有行推导，不插第二条）、两个非终局项、系统 CAS |

即：**终局 ≠ 记 receipt**。`STALE_GENERATION` 与
`MUTATION_ID_CONFLICT` 都是终局但都不写库。

### 6.3 bootstrap

`desiredWatchAuthorityGeneration` 为 snapshot **顶层 required 字段**；
`desiredWatches` 条目携带 `desired`/`policy|null`/`revision`/
`materializationEpoch`，**含 tombstone**，受 32 KiB 预算；
**不含** materialization 运行态（随连接由首帧 FULL 下发）。

### 6.4 数值边界与 scalar 归属

`authorityGeneration`/`sectionRevision`/`materializationEpoch`/
`armAttempt`/`transitionId`/`actorIncarnation` 六个量收敛到
**`≤ Number.MAX_SAFE_INTEGER`**；三处钉死（SQLite CHECK / **本地专有**
契约 scalar / 前端本地专有校验器）。**不得放进 `bcsp-contracts`**。

## 7. 放置、拓扑与公网边界

### 7.1 Rust 放置

`bcsp-contracts` 与 `bcsp-application` **都在公网闭包内**；desired 命令、
事件与 authority **全部放本地专有 crate**，共享 host 只经 tag 无关的
`WebSocketExtension` 承载。

### 7.2 secondary 路由：受校验的路由集合

`HostState` 需 `path → Arc<dyn WebSocketExtension>` 映射；
`SecondaryWebSocketRoute::new` 改 **fallible**（承载 S2a 校验），
`LoopbackServerError` 新增变体；**集合内路径唯一性校验必需**（axum
`Router::route` 重复路径会 **panic**）。路径冻结：
presence `/api/v1/local/presence`、desired `/api/v1/local/desired-watch`。

### 7.3 携带项绑定

**S2c** 共享 `serve_websocket` 补 **64 KiB** 帧/消息上限；
**S2b** 未注入 → **404**，**逐路由**钉死。

### 7.4 N-g 措辞

公网严格解码器把 desired tag 当未知命令拒绝：**不进入命令路由、不改变
watch 状态、不产生 dispatch/sink/reply**。

### 7.5 旧命令 + 服务端准入过滤器

公网保留 ephemeral 语义；**本地外部协议拒绝**；内部经 Rust API。
本地必须有**显式的服务端 command-admission filter**（新增中立 manager
API 本身不拒绝外部旧命令）。

### 7.6 target-neutral 端口（冻结）

```
watch: {
  setDesired(section, desired, policy): DesiredMutationHandle
  //   handle.result: Promise<{ status: COMMITTED | REJECTED
  //                                   | SENT_UNCONFIRMED, reason? }>
  //   非终局（UNAVAILABLE / RATE_LIMITED）由 adapter **以同一 mutationId**
  //   内部重试，Promise 保持 pending；handle 暴露 retry 状态供 UI 显示
  subscribeProjection(listener): unsubscribe      // §0.3 五态 + 第 0 条
  subscribeEvents(listener): unsubscribe          // ← v6 缺失且承重
  //   observation / episode / alert / audioDisposition / cueReceipt
  readonly state; subscribeState(listener); connect(); disconnect()
  episodes: { acknowledgeEpisode, acknowledgeAllEpisodes,
              resumeTimedOutEpisode, resetAudibleCount,
              reportCueOutcome, dismissAlert }
}
```

- **`subscribeEvents` 是承重面**：只有 `subscribeProjection` 会让**警报
  与声音事件整体消失**（provider 仍需 observation/episode/alert/audio/
  cue receipt）；
- **`UNAVAILABLE` 不是终局**，不能作为 Promise 的终值返回（v6 把它列进
  "终局返回"）：改为 handle + 同 id 内部重试；
- **公网 STOP / UPDATE_POLICY 不得报告为 `COMMITTED`**（v7 的
  "乐观完成"被复审明确否决）。理由：socket OPEN + `send()` 只证明
  浏览器**接受了发送**；`STOP` 的 `UnknownWatch` 现只写日志；
  `UPDATE_POLICY` 在无 episode 时即使成功也没有任何后续事件——于是会
  **永久返回一个无法验证的 `COMMITTED`**。
  裁定采纳可接受方案之二：公网 adapter 对这两条返回
  **`SENT_UNCONFIRMED`**，端口 outcome 枚举相应扩为
  `COMMITTED | REJECTED | SENT_UNCONFIRMED`；**UI 不得据此显示权威
  成功**。文案冻结为 **"已发送，协议不提供确认"**——不是"等待确认"，
  因为公网 wire 上**根本不会再来**任何确认。若将来公网 wire 补上 STOP/UPDATE 的
  成功/拒绝 ACK，可再收敛为 `COMMITTED`。

### 7.6.1 本地 episode 控制与事件通道（v7 只列了端口方法，没落线）

**裁定：继续双 socket**，各自职责冻结如下。

| 本地 socket | 职责 |
| --- | --- |
| `/api/v1/watch`（既有） | **audience 事件通道 + episode 控制 + 应用层心跳**（S2-PR4 不变） |
| `/api/v1/local/desired-watch` | **desired CAS 命令 + 投影帧 + `DESIRED_WATCH_EFFECT_ACK`** |

规定：

- **episode 命令路由**：`ACKNOWLEDGE_EPISODE` /
  `RESUME_TIMED_OUT_EPISODE` / `RESET_AUDIBLE_COUNT` /
  `REPORT_CUE_OUTCOME` / `DISMISS_ALERT` **五条按 `activeWatchId`
  定向路由**到 synthetic owner；**`ACKNOWLEDGE_ALL_EPISODES` 不带
  `activeWatchId`**，按 **synthetic owner 全域执行**；
- **owner 产生的事件扇出给全部 audience**（不再 unicast 给发起连接）；
- **leader 只决定是否播放声音，不得截断事件**：所有 audience 都收到
  完整事件流，是否发声由 leader 决定；
- **本地 admission filter 只拒绝旧三条** `START_WATCH` /
  `STOP_WATCH` / `UPDATE_POLICY`，其余命令照常；
#### 7.6.1.1 audience 绑定与跨流因果序（v7.1 的洞）

FULL 走 desired socket、effect 走 watch socket，**两条 TCP 流之间不存在
"先发即先到"的保证**。冻结：

- **`audienceBindingId`**：把两条连接绑成**同一 tab 的同一连接代**的
  稳定身份。客户端在两条 socket 上各出示一次；服务端仅在**两半都出示
  且匹配**时成对；
- **`AttachAudience` 只在"两半已成对 **且** 首个 FULL 已被客户端应用"
  之后发生**（客户端以 projection-ready ACK 告知）——在此之前该 tab
  不进入任何 batch 的 required set；
- **任一半断开即 pair 失效**，并**只产生一次 Detach**（不得两半各触发
  一次）；
- **ACK 校验**：`DESIRED_WATCH_EFFECT_ACK` 必须校验该 audience
  **确实属于该 batch 的 required set**，否则忽略并计数；
- **effect 携带 `projectionFence`**（§5.2.2）：客户端在对应的
  FULL/DELTA 应用之前**必须缓存该 effect**，不得提前处理；服务端亦可
  选择在收到 projection-ready ACK 之后才投递该 fence 之后的 effect。
  二者取其一，**实现时冻结为前者 + 服务端不主动提前投递**；
- **一个 tab 的同步态是 composite 的**（§0.3 第 0 条）：两条 socket
  OPEN + 首 FULL 已应用 + 非 DESYNCED/ASSEMBLING。**raw OPEN 不算**。

### 7.7 public marker 计账（**冻结**）

不新增 capability 行（保持 **18**）。`PERSISTENT_ACTIVE_WATCH` 行由 13
项增至 **16 项**，**按序数序插入**，冻结结果数组：

```
active_watch_persistence, active_watch_store, authority_generation,
auto_rewatch, automatic_rewatch, desired_watch, materialization_epoch,
persisted_active_watch, persistent_active_watch, restore_subscription,
restore_watch, restored_watch, subscription_restoration,
watch_rehydration, watch_repository, watch_storage
```

marker 总数 **212 + 3 = 215**。冻结摘要（复审方按该数组算得，实现时
必须复算一致）：

- Rust whole-document SHA：
  `20B38DE008B28197879D536D85E44A0D32EE88BF5A25ED24C15B9CE396DBEA0D`
- Frontend rows-only SHA：
  `F78B7BB698BBED4F146DFE1AF7248A312A036F457D2D0809B4DD6FBC46219989`

同步项：`markerSetVersion` **1 → 2**（policy / Rust verifier / frontend
verifier 三处）；`verify-import-graph.test.mjs` 的 **18/212 → 18/215**；
**五组、八个断言面各补正反例**（Rust graph 1；Rust zero surface
SOURCE/API/STORAGE/PACKAGE 4；frontend import graph 1；target build 1；
public DOM/bundle 1）；local capability manifest 增精确 slug
**`persistent-desired-watches`**，public manifest **必须不含**。

**孤儿 tag 规则（v6 的边界洞）**：`REQUEST_FULL_PROJECTION` /
`PROJECTION_UPDATE` / `MUTATION_RESULT` 这些名字**本身不含**任何 marker，
会从负控里漏出去。裁定：**所有本地 wire variant 一律加
`DESIRED_WATCH_` 前缀**（已落实于 §6），并补一条**"孤儿 tag 泄漏"负例**
——公网源码中出现任何本地 wire variant 名即失败。该规则**不改变 215**。

## 8. 迁移 10004 与同步面

- 重建 `personal_desired_watches_v1`，新建
  `personal_desired_watch_receipts_v1`；
- 10003 遗留行升级为 `desired = 1`，按 `(term, campus, index)` 升序从 1
  起分配 `revision` 与 `materialization_epoch`；
- metadata 写入 generation = 1、`actorIncarnation` = 1、两计数器 =
  已分配最大值；
- 在既有 personal runner 的**单事务 + 可选 Rust after-hook** 模式内完成。

**同步面**：两个 allowlist、`PersonalTableCounts`、`PersonalResetResult`、
schema 子串守卫、Rust `PersonalStateSnapshot`/`DesiredWatch` 形状、
`bcsp-local-runtime` bootstrap 编码，**以及
`frontend/src/ui/local/personal/contracts.ts`**：

1. `desiredWatches.length <= 9` 改为"**`desired = 1` 的条目数 ≤ 9**"；
2. `hasKeys` 是**键集全等**检查，新增顶层字段与条目字段都会打挂它；
3. 同步三处 bootstrap fixtures。

## 9. 验收清单（自包含，**A-01..A-53**，连续编号）

**CAS 与身份**
- A-01 STOP 后延迟旧 START → `STALE_REVISION`；
- A-02 新 START 后延迟旧 STOP → 对称；
- A-03 旧 policy 不得覆盖新 policy；
- A-04 reset 升代后前代写入 → `STALE_GENERATION`；
- A-05 四例中**旧命令必须实际迟到**；
- A-06 幂等三态；同 id 异 section/异 fingerprint → 冲突且**不插第二行**；
- A-07 `STALE_GENERATION` 与**系统 CAS** 均不记 receipt；
- A-08 `AUTHORITY_UNAVAILABLE` 不记 receipt、同 id 重试可成功；
- A-09 终局拒绝后原因消失，**新手势新 id** 必须能成功；
- A-10 **客户端收到终局拒绝后不得自动改号重发**；
- A-11 系统 CAS 重试每次新铸 id；
- A-12 receipt 主键为 `(generation, mutationId)`；
- A-13 `basedOnRevision` 只取自投影 store；`MUTATION_RESULT` 不带状态；
- A-14 **终局性与"是否记 receipt"是两张表**（§6.2）：非终局 =
  `RATE_LIMITED` / `AUTHORITY_UNAVAILABLE`（同 id 可重试、不记）；
  终局七项中 **`STALE_GENERATION` 与 `MUTATION_ID_CONFLICT` 也不记**
  （分别由步骤 1 判定、由既有行推导）。逐项钉死，不得把两张表合并；

**帧流与接收机**
- A-15 每轮**至多一个投影更新**；无变化不发帧且序号不递增；
- A-16 **分块规则 C0–C8 逐条钉死**：整组原子提交与 ASSEMBLING 降级
  （C0/C0b，另见 A-46）、`frameGroupId` 不混组、块头五元组严格校验、
  重复块索引、旧 incarnation 块、**10s 组装超时**（"actor 中途崩溃、
  末块永不到达"时页面**不得永久保留旧绿态**）、**同时只装一组且异
  DELTA 组两者皆弃**（C6）、**更旧的同化身 FULL 不回退 `last`/不作废
  更新在装组**（C7，另见 A-47）、每组 256 KiB/16 块（C8）；
- A-17 **定向 FULL 不消费全局序号**，携带 `throughTransitionId`；
  "A 收 T10 → B Attach（定向 FULL，不占号）→ **下一次广播必须是 T11**"，
  A 按 R2 正常应用、**不得判 gap**（v7 的 A-17 写成 T12，与 R2 直接
  冲突，此处更正）；
- A-18 接收状态机 R1–R7 逐条钉死，含 **R5：FULL 必须删除帧中缺席的
  本地 entry**；
- A-19 新化身只接受 FULL；重建 FULL **不得**被帧顺序守卫丢弃；
- A-20 `REQUEST_FULL_PROJECTION` **独立限流 + 合并**；小请求不得触发
  无界序列化；**被限流时回 `DESIRED_WATCH_RESYNC_REJECTED`（非静默
  丢弃）**，客户端下次重试取 `max(退避, retryAfterSeconds)`；
- A-21 首个/请求的 FULL **超时 + single-flight + 退避**，连续失败后断连；
- A-22 **audience registry 活在 actor 之外**：actor 重建后能找回仍存活
  的 pump 并重播 FULL；

**投影语义**
- A-23 §0.3 **有序**判定逐条钉死，含**第 0 条**（未同步 → 重连中/同步
  中、非绿、禁止用缓存 revision 发 mutation）；
- A-24 §0.1 严格 `armed` 且**由前端派生**；wire 上不得出现 `armed` 布尔；
- A-25 `pendingDisarm` 与 `blockedOnSlot` 是两个字段，文案不同；
- A-26 **不变量 `blockedOnSlot ⇒ desired=1 ∧ materialized=null ∧
  ¬pendingDisarm`**；`desired=false` 原子清除它；`SlotReleased` 唤醒后
  **重读**当前 revision/desired；
- A-27 迟到的终局拒绝不得把已前进的投影拉回；
- A-28 前端 store 丢弃更旧 generation 的残留；

**物化、幂等与活性**
- A-29 `ensure` 同 `(generation, epoch)` 重试收养原 ID、零副作用；
- A-30 **adopt 前置校验五项**（同 owner/section、ID 仍驻留、policy 同、
  无针对该 ID 的 disarm、无未确认 effect batch）逐项钉死；
- A-31 **effect batch 幂等与可执行确认**（§5.2.1）：manager 建出
  ID/episode/cue 后 actor 崩溃，重建后**用原 ID 重放交接**，cue 与
  history 不得丢失、不得产生第二份；**history ACK = SQLite commit 或
  `AlreadyPresent`**（日志不算）；**audience ACK = 客户端显式
  `DESIRED_WATCH_EFFECT_ACK`**（入队/`send()` 不算）；detach 的
  audience 移出必需集合、集合空则仅凭 history ACK；客户端**先按
  `(effectBatchId, effectIndex)` 去重再发声且仍须 ACK**；
  **未确认 batch 只能 `PENDING_HANDOFF → replay`，绝不落入
  `Restarted`**；容量 64 个 / 256 KiB、达帽背压、**回收仅在 history
  ACK 之后**；
- A-32 **重建顺序**：audience **先**收新化身 FULL、**再**按原顺序重放
  effect（顺序反了会被 R4 全部丢弃）；history 独立恢复；
- A-33 `applyPolicy` 使 policy-only 更新真正生效；**同 revision 重试不
  重复写 history**；未武装/blocked 时只更新 desired 侧；
- A-34 **`armAttempt` 与 `disarmOperationId` 分离**：同一 section 上
  "新 epoch 的 arm"与"旧捕获 ID 的 disarm"可并存且互不作废；
- A-35 rotation 与 actor 重建换 epoch 时走 **adopt**，健康 watch 不得
  `Restarted`（不结束 episode / 不换 ID / 不重新响铃）；
- A-36 一般暂态 arm 失败不回滚 desired，重试后转 armed；
- A-37 两个帽两个不变量：物理 watch 不得超帽；`SlotReleased` 到达后
  立即 arm；
- A-38 **周期 reconcile 是独立活性来源**：丢掉 `SlotReleased` 后仍能在
  一个周期内自愈；
- A-39 owner 的 `RejectedLimit` 两种情形分别为"预期路径/不变量告警"，
  两者都不删 desired；
- A-40 `ConnectionKind::Owner` 不可由 wire 构造；豁免心跳不豁免物理帽；
- A-41 `disarm` 用**发起时捕获**的 ID；"STOP → 退避中 → 用户又 START"
  下不得拆掉新武装的 watch；
- A-42 **满 9 门时的 policy-only 更新必须成功**（post-state 判定）；
  只有 `0→1` 占额；

**端口与边界**
- A-43 **本地 episode 通道落线**（§7.6.1）：六条 episode 命令按
  `activeWatchId` 路由到 synthetic owner；**owner 事件扇出给全部
  audience**（非 unicast）；**leader 只决定是否发声、不截断事件**；
  本地 admission filter **只**拒绝旧三条；两条 socket 都 OPEN 才算已
  同步。端口 `subscribeEvents` 承载 observation/episode/alert/audio/
  cue receipt——缺它则警报与声音整体消失（负例）；
- A-44 `setDesired` 返回 handle；非终局由 adapter **以同一 mutationId**
  内部重试且 Promise 保持 pending；**公网 STOP/UPDATE_POLICY 必须返回
  `SENT_UNCONFIRMED`，绝不得报告 `COMMITTED`**，UI 不得据此显示权威
  成功（负例：断言公网这两条不会产生 `COMMITTED`）；
- A-45 §2.5 六值 + 执行合同（UTF-8 整信封计数、`floor(cap*4/5)` 与
  三点边界、`RATE_LIMITED` 的 wire 结果与 `Retry-After`、rotation 保留
  incarnation、Full Reset 不重置 `transitionId`）；marker：18 行 / 215 项 /
  数组顺序 / 两个摘要 / `markerSetVersion 2` / 测试常量 / 八个断言面
  正反例 / local manifest 含 `persistent-desired-watches` 且 public 不含 /
  **孤儿 tag 泄漏负例**；本地旧三命令被服务端 admission filter 拒绝。
- A-46 **零 audience 暂停**（§5.6.1）：`1→0` 拆除全部物理 watch 并保留
  desired；`0→1` 从权威 snapshot 重新物化（新 epoch）；`activeWatchId`
  与 episode 重新生成，`ACTIVE_WATCH_STATE_PERSISTENT` 仍为 false。
  **最终验收须覆盖半开 pair、在途旧 disarm、以及容量已满三种情形**；
- A-47 **整组原子提交**（C0/C0b）：全部块校验+集齐+按 `chunkIndex` 升序
  拼接后**只提交一次**；**部分组绝不触碰 store**；**R5 只对完整 FULL 组
  的 entry 并集执行一次**（负例：`[A]`/`[B]` 两块互删）；**首块到达
  即 ASSEMBLING、非绿、禁 mutation**（不是等 10s 超时才降级）；
- A-48 **同组五元组一致性**（C2 含 `authorityGeneration` 与
  `throughTransitionId`）；**同时只装一个组**，在装期间异
  `frameGroupId` 的 DELTA 组到达 → **两者皆弃并 DESYNCED**（C6）；
  **更旧的同化身 FULL 不得回退 `last`、不得作废更新的在装组**（C7）；
  **每组 256 KiB / 16 块**上限（C8，单块帽不约束组装内存）；
- A-49 **effect 上 wire**（§5.2.2）：`DESIRED_WATCH_EFFECT` 携带
  `audienceBindingId` / `effectBatchId` / `effectSequence` /
  `effectIndex` / `effectCount` / `projectionFence` / `watchEvent`；
  **客户端集齐整个 batch 并按 `effectIndex` 顺序处理完之后才 ACK**；
  wrapper **不进 `bcsp-contracts`**、marker 计数不变；
- A-50 **audience 绑定与跨流因果序**（§7.6.1.1）：`audienceBindingId`
  成对；**`AttachAudience` 只在两半成对且首个 FULL 已应用之后**；任一半
  断开 → pair 失效且**只产生一次 Detach**；`EFFECT_ACK` 校验该 audience
  **确属该 batch 的 required set**；effect 按 `projectionFence` **缓存至
  对应投影帧应用后**才处理；
- A-51 **ACK 活性**：每个 required audience 的 **ACK deadline 30s**，
  超时**判为 detach** 并推进 batch（负例：一个永不 ACK 的连接**不得**
  堵死所有新 arm）；**GC 条件恰为
  `historyAck ∧ (每个 required audience 已 ACK 或已 detach)`**；
- A-52 **拆除路径独立预算**：`STOP` / Full Reset / `1→0` 的 disarm 各自
  **16 个 / 64 KiB**；**普通 effect backlog 达帽时物理拆除仍须畅通**；
- A-53 **composite 同步态**（§0.3 第 0 条）：两条 socket OPEN + 首 FULL
  已应用 + 非 DESYNCED + 非 ASSEMBLING；**raw OPEN 不得被当作已同步**。

## 10. PR5 / PR6 切分与激活（复审裁定）

**PR5 不是可独立启用件**——三种单边启用都会直接打挂产品：

| 单边启用 | 后果 |
| --- | --- |
| PR5 先启用 admission filter | 旧前端仍发 START/STOP/UPDATE → **监控直接失效** |
| PR6 先启用 | 旧后端 desired route 为 **404** → **永远不同步** |
| PR5 先要求 `EFFECT_ACK` | 旧前端永不 ACK → 最终触发**背压** |

**裁定**：PR5 以**完全 dormant** 的形态合并——

- **不注册** desired route；
- **不启用** 本地 admission filter；
- **不启动** owner materialization；
- **不关闭** S2-D3。

**生产激活必须与 PR6 原子发布**，或引入**明确的协议版本 / 能力握手**
（二选一须在 PR6 前冻结）。

**release-gate E2E 必须覆盖**：新旧四种组合（旧前端×旧后端、旧×新、
新×旧、新×新）以及**单边 socket flap**（desired 断而 watch 未断、
反之）。

## 12. PR5 合并硬验收（复审 2026-08-23 转入 PR 级，不再另发设计稿）

**G1 消除"首 FULL / Attach"循环**。§4.4 是 Attach 后发 FULL，
§7.6.1.1 又要求首 FULL 应用后才 Attach——实现必须走**两阶段**：

```
UNPAIRED
  → PAIRED_SYNCING   （注册 pump；不进 required set；发送 FULL）
  → PROJECTION_READY （客户端回执）
  → ACTIVE / AttachAudience
```

本地专有 bind/HELLO 与 **`DESIRED_WATCH_PROJECTION_READY`** wire 必须
落地；**旧连接代的 READY/ACK 一律无效**。

**G2 冻结可比较的 projection fence**：

```
{ authorityGeneration, actorIncarnation, throughTransitionId }
```

广播帧以**自身 `transitionId`** 作为 through 值。**actor 重建时保留
batch/event ID，但把 wrapper 的 fence 重基到新 FULL 的位置**；客户端
仅在**当前位置满足 fence 后**才释放 effect。

**G3 effect 边界矩阵**：

- `audienceEffects.isEmpty ⇒ requiredAudienceSet = ∅`；
- **history-only batch 只等 history ACK**；
- 两侧都无 effect ⇒ **不创建 batch，或立即完成**；
- **ACK deadline 自"fence 已满足且完整 batch 首次实际交付"起算**，
  **不是** 自 batch 创建起算；
- **ACK 超时必须原子地**：失效 binding、关闭/作废两半连接、产生**一次**
  Detach，并使 UI **离开 composite-synced**。

**G4 history 故障时禁止重新武装**。仅有 16/64 KiB 的 teardown reserve
仍可能被反复 `0→1→0` 耗尽。**任何持续未 ACK 的 history 故障都必须
暂停新的 arm/policy 物化**；**disarm 继续使用保留预算**；history 恢复
并清账后再重新物化。

**G5 dormant 必须覆盖 migration 与 bootstrap**。PR5 含 10004 与 Rust
snapshot 变形，而旧前端对 bootstrap/`DesiredWatch` 使用**严格键集**
校验。PR5-only 构建必须满足其一：

- **不执行/不激活** 10004 与新 bootstrap；**或**
- 数据库可先升级，但**继续输出与 protocol v1 逐键相同的旧 bootstrap
  兼容视图**。

**dormant 负控必须证明当前 UI 行为不变**；route / filter / owner /
新 bootstrap 的**生产激活**仍须与 PR6 原子发布或经明确能力握手。

**G6 帧帽证明与小勘误**：

- 用**最大合法状态**证明 FULL ≤ **256 KiB / 16 块**；**证明失败即阻断
  PR 合并**，并须补 producer 侧恢复路径；
- §2.5 与 C8 的表述冲突**以 C8 为准**（已改）；
- `ACKNOWLEDGE_ALL_EPISODES` 无 `activeWatchId`：**五条定向 + ACK-all
  全域**（已改）；
- alert 触点表的单一"第二 WS 路由"残留随 PR 文档同步（非阻断）。

## 11. 已裁定 + 写回义务

- **打包冒烟播种：P1 release gate**——必须在**首个生产 writer / 整批
  发布之前**完成；确定性 PUBLISHED section fixture 且 S1 gate 放行；
  `desired = false` 的 tombstone 提交**不能**替代 true-path 验收。
- **写回范围（v6 只写了一个文件，不足）**：
  1. `2026-08-20-alert-delivery-integrity.md`：**leader 模型**、
     **触点表**、**测试 1 与 1c**——1c 改为"leader 转移**不 re-arm**，
     `activeWatchId`/`materializationEpoch`/manager watch 计数**均不变**，
     **无重复 episode**，新 leader **只**接管音频，跨 tab STOP 仍收敛"；
     并补记 **9 门帽两类事件的区分**（CAS `LIMIT_EXCEEDED` 属 admission
     类永久拒绝；owner `RejectedLimit` 是内部物理槽条件，不移除 desired）；
  2. **验收清单条目 7**（leader 重水合表述需按"服务端持有、leader 只管
     音频"改写）；
  3. `2026-08-20-review-package-local.md` 的 **leader 所有权**段落
     （"仅 leader 武装监控与发声"已被本设计取代）；
  4. **清单尾部两处漂移**：`S2-D3` 里"路由落地即可关闭打包门"的说法与
     本节的 **P1 release gate** 裁定冲突，须改写；进度行仍写 v5，须更新。
     （1–4 已于 v7 执行完毕。）
  5. **v7.1 补做的三处残留**（复审指出 v7 只完成了一部分）：
     a) alert 文档仍写 **desired→armed 调和循环归客户端**、
        **页面加载后自动 START**——两者均已转由**服务端 authority**
        承担，须改写；
     b) alert 文档 §3 产品边界仍称 **"本设计无服务端持久个人状态新增"**
        ——desired 表落地后该断言不成立，须改写；
     c) `review-package-local.md` L1 仍写**页面加载后按表自动 START**
        ——改为服务端按已提交 desired 物化，页面只是编辑者与 audience。
