# 期望监控：revision/CAS 共同编辑模型（S2-D3 路线改判）

状态：ACTIVE，**v7**。路线由产品所有者 2026-08-22 裁定：取代 fenced
sequencer，走 **持久化 revision + tombstone + CAS**。
S2-D3 硬门在本设计实现并复审通过前**保持关闭**。

**wire 名冻结**：`authorityGeneration`；**所有本地 wire variant 一律带
`DESIRED_WATCH_` 前缀**（§7.7 的孤儿 tag 规则）。

**修订史**：v1 五组 → v2 四组 → v3 自查十一项 → v4 五组 → v5 自查十五项
→ v6 三组 → **v7 逐条闭合**。
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
0. 连接非 OPEN ∨ 尚未收到首个 FULL ∨ 处于 DESYNCED
   → 「重连中 / 同步中」，非绿，**禁止用缓存 revision 发 mutation**
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
| R6 | DESYNCED 时发 `DESIRED_WATCH_REQUEST_FULL_PROJECTION`，**single-flight**；超时按 1s/2s/4s 封顶 30s 退避重试，连续 5 次失败则主动断连重连 |
| R7 | 首个 FULL 亦受 R6 的超时/退避约束 |

**分帧与"每轮至多一帧"的统一**（v6 两处矛盾）：一个 actor 轮次产生
**至多一个投影更新**；若序列化超过 §2.5 的字节预算，则**分块**为多个
wire 帧，**共享同一 `transitionId`**，带 `chunkIndex` / `chunkCount`，
接收方**集齐末块才整体提交**。分块不是多帧，序号不递增。

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
| FULL / DELTA / bootstrap 序列化上限 | 各 **32 KiB** |
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

**actor 故障重建**：`++actorIncarnation` → **枚举 manager 中未确认的
effect batch 并重放交接**（§5.2）→ 向所有现存 audience 重播 FULL。

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
  manager 之前**分配；
- manager **保留该操作的 effect batch**（原 action / episode / cue 的
  ID 与内容）**直到 actor 确认 history 与 audience 双方均已接收**——
  模式参照 manager 既有的 message replay cache；
- actor 重建时**枚举未确认 batch 并重放交接**，用**原 ID**，不新建；
- `ensure` 的 **adopt 必须逐项校验**：同一 owner、同一 section、
  `activeWatchId` **仍驻留 manager**、policy 相同、**没有针对该 ID 的
  disarm 操作**、**没有未确认 effect batch**。任一不满足即不得 adopt。

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
SET_DESIRED_WATCH { section, desired, policy|null,
                    basedOnRevision, authorityGeneration, mutationId }
DESIRED_WATCH_REQUEST_FULL_PROJECTION { }
```

### 6.2 事件

```
DESIRED_WATCH_PROJECTION_UPDATE {
  actorIncarnation, authorityGeneration,
  kind: FULL | DELTA,
  transitionId | null,          // 广播帧有；定向 FULL 为 null
  throughTransitionId | null,   // 定向 FULL 有
  chunkIndex, chunkCount,       // 分块；同一 transitionId
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
DESIRED_WATCH_SLOT_RELEASED { section }              // 内部；不上 wire
```

`reason` ∈ `STALE_GENERATION` / `STALE_REVISION` / `MUTATION_ID_CONFLICT` /
`LIMIT_EXCEEDED` / `UNSUPPORTED_TARGET` / `TERM_OUT_OF_RANGE` /
`SECTION_NOT_FOUND` / **`RATE_LIMITED`**。
**终局集合** = 前七个；`RATE_LIMITED` 与 `AUTHORITY_UNAVAILABLE` **非终局**。

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
  //   handle.result: Promise<{ status: COMMITTED | REJECTED, reason? }>
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
- **公网 adapter 必须定义 STOP 与 UPDATE_POLICY 的完成语义**：现行
  `UPDATE_POLICY` **没有明确的成功 ACK**，仅映射 `START_RESULT` 不够。
  公网 adapter 以"命令已发出且连接仍 OPEN"为乐观完成，并在
  `WATCH_STOPPED` / 后续 `EPISODE_UPDATED` 的 policy 字段上做事后校正；
  该退化语义须在端口文档中明示，避免本地/公网行为被误认为等价。

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

## 9. 验收清单（自包含，A-01..A-44）

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
- A-14 **`RATE_LIMITED` 与 `AUTHORITY_UNAVAILABLE` 为非终局、同 id 可
  重试且不记 receipt**；终局七项反之；

**帧流与接收机**
- A-15 每轮**至多一个投影更新**；无变化不发帧且序号不递增；
- A-16 **分块**帧共享 `transitionId`、集齐末块才提交；
- A-17 **定向 FULL 不消费全局序号**，携带 `throughTransitionId`；
  "A 收 T10 / B Attach / 下一广播 T12"下 **A 不得误判缺帧**；
- A-18 接收状态机 R1–R7 逐条钉死，含 **R5：FULL 必须删除帧中缺席的
  本地 entry**；
- A-19 新化身只接受 FULL；重建 FULL **不得**被帧顺序守卫丢弃；
- A-20 `REQUEST_FULL_PROJECTION` **独立限流 + 合并**；小请求不得触发
  无界序列化；
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
- A-31 **effect batch 幂等**：manager 建出 ID/episode/cue 后 actor 崩溃，
  重建后**用原 ID 重放交接**，cue 与 history **不得丢失**，且**不得**
  产生第二份；
- A-32 `applyPolicy` 使 policy-only 更新真正生效；**同 revision 重试不
  重复写 history**；未武装/blocked 时只更新 desired 侧；
- A-33 **`armAttempt` 与 `disarmOperationId` 分离**：同一 section 上
  "新 epoch 的 arm"与"旧捕获 ID 的 disarm"可并存且互不作废；
- A-34 rotation 与 actor 重建换 epoch 时走 **adopt**，健康 watch 不得
  `Restarted`（不结束 episode / 不换 ID / 不重新响铃）；
- A-35 一般暂态 arm 失败不回滚 desired，重试后转 armed；
- A-36 两个帽两个不变量：物理 watch 不得超帽；`SlotReleased` 到达后
  立即 arm；
- A-37 **周期 reconcile 是独立活性来源**：丢掉 `SlotReleased` 后仍能在
  一个周期内自愈；
- A-38 owner 的 `RejectedLimit` 两种情形分别为"预期路径/不变量告警"，
  两者都不删 desired；
- A-39 `ConnectionKind::Owner` 不可由 wire 构造；豁免心跳不豁免物理帽；
- A-40 `disarm` 用**发起时捕获**的 ID；"STOP → 退避中 → 用户又 START"
  下不得拆掉新武装的 watch；
- A-41 **满 9 门时的 policy-only 更新必须成功**（post-state 判定）；
  只有 `0→1` 占额；

**端口与边界**
- A-42 端口 `subscribeEvents` 承载 observation/episode/alert/audio/cue
  receipt；缺它则警报与声音消失（负例）；
- A-43 `setDesired` 返回 handle；非终局由 adapter **以同一 mutationId**
  内部重试且 Promise 保持 pending；公网 adapter 的 STOP/UPDATE_POLICY
  完成语义按 §7.6 的退化定义并在端口文档明示；
- A-44 §2.5 六值 + 执行合同（UTF-8 整信封计数、`floor(cap*4/5)` 与
  三点边界、`RATE_LIMITED` 的 wire 结果与 `Retry-After`、rotation 保留
  incarnation、Full Reset 不重置 `transitionId`）；marker：18 行 / 215 项 /
  数组顺序 / 两个摘要 / `markerSetVersion 2` / 测试常量 / 八个断言面
  正反例 / local manifest 含 `persistent-desired-watches` 且 public 不含 /
  **孤儿 tag 泄漏负例**；本地旧三命令被服务端 admission filter 拒绝。

## 10. 已裁定 + 写回义务

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
