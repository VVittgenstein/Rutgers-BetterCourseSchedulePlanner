# 期望监控：revision/CAS 共同编辑模型（S2-D3 路线改判）

状态：ACTIVE，**v6**。路线由产品所有者 2026-08-22 裁定：取代 fenced
sequencer，走清单二选一的另一条——**持久化 revision + tombstone + CAS**。
S2-D3 硬门在本设计实现并复审通过前**保持关闭**。

**wire 名冻结**：`authorityGeneration`（`resetGeneration` 作废）。

**修订史**：v1 五组 → v2 四组 → v3 自查十一项 → v4 五组 → v5 自查十五项
→ **v6 逐条闭合**。

## 0. 唯一要约束的东西：UI 投影

禁止的失败只有一种：**页面声称某 section 在被监控而实际没有，或反之**。

### 0.1 物化记录与 `armed`

owner 为每个 section 持有一条**可空的物化记录**：

```
materialized: { generation, revision, epoch, policy, activeWatchId } | null
```

它记录的是**实际生效在 manager 上的那份**，与 desired 侧的
`(authorityGeneration, revision, materializationEpoch, policy)` 是**两组
独立字段**（v5 只有 `materializedRevision`/`materializedPolicy`，缺
generation 与 epoch 的物化孪生，导致 §0.1 的两个比较是自己比自己）。

```
armed(section) ≡ materialized ≠ null
  ∧ materialized.generation = authorityGeneration
  ∧ materialized.revision   = sectionRevision
  ∧ materialized.epoch      = materializationEpoch
  ∧ materialized.policy     = desiredPolicy
```

这满足清单第 5 条的 `desired(policy) == armed(policy)`：policy 已提交但
尚未在 manager 生效 ⇒ 非绿。

### 0.2 三个正交状态位

| 位 | 含义 |
| --- | --- |
| `pendingDisarm` | **本 section 自己**的拆除正在退避重试中；物理 watch 仍活着、仍可能响铃 |
| `blockedOnSlot` | **本 section 的 arm 在等物理槽**（别的 section 的拆除还占着）；本 section 没有任何物理 watch |
| `armed` | §0.1 |

v5 用同一个 `pendingDisarm` 表达"自己在拆"与"在等别人腾槽"，两者在
`ProjectionEntry` 上无法区分，导致 §5.7 承诺的"新 section 显示准备中"
落不到表上。二者现在是**两个字段**。

### 0.3 投影判定（**有序**求值，第一条命中即止）

```
1. pendingDisarm            → desired=0 ? 「停止中」 : 「准备中（旧槽拆除中）」   非绿
2. blockedOnSlot            → 「准备中（等待旧槽释放）」                          非绿
3. desired = 0              → 「未监控」                                          —
4. desired = 1 ∧ armed      → 「监控中」                                          **绿**
5. desired = 1 ∧ ¬armed     → 「准备中」（附 classification 文案）                非绿
```

**必须有序求值**：v5 写成真值表时，STOP 一提交就换 epoch，幸存的物理
watch 因 epoch 不符而 `armed = false`，于是落进"未监控"——而那份 watch
还活着、还会响铃。**投影表自己制造了它要禁止的谎言**。把
`pendingDisarm` 提到第一条即消除该洞。

**唯一绿灯**：第 4 条。

### 0.4 投影必须原子送达

desired 与 materialization 合并为**单一原子信封**
`PROJECTION_UPDATE`（§6.2），前端 reducer 整帧提交，中间态永不渲染。

## 1. 产品模型（裁定）

所有标签页平权编辑；`desired_watches` 是唯一真相；实际监控由服务端统一
维护；leader 只负责"避免多个页面同时响铃"这一件用户看不见的事。

## 2. 权威状态

### 2.1 单调量与守卫

| 量 | 分配 | 持久 | 排序对象 |
| --- | --- | --- | --- |
| `authorityGeneration` | Full Reset / 压缩 | metadata | 全表世代 |
| `sectionRevision` | CAS 提交 | 全局计数器，值落每行 | desired 状态 |
| `materializationEpoch` | desired **值**变化 / 连接边沿 / rotation / actor 重建 | 全局计数器，值落每行 | armed 身份 |
| `attemptToken` | **每次 arm / disarm / policy 尝试** | 不持久（actor 内单调） | 单次尝试 |
| `actorIncarnation` | **每次 actor 启动或重建** | **metadata 标量（持久）** | 帧流化身 |
| `transitionId` | 每个 actor 轮次 | 不持久，**每化身内从 1 连续递增** | 帧顺序 |

**三层守卫**：

- desired 侧：`(authorityGeneration, sectionRevision)`
- armed 侧：`(authorityGeneration, materializationEpoch, attemptToken)`
- 帧顺序：**`(actorIncarnation, transitionId)`**

`actorIncarnation` 是 v5 的洞：actor 崩溃**不触发重连**（pump 活在 host
的 tokio 任务里），页面保留 socket 与上次 `transitionId`；重建后计数器
从头开始，**§4.2 强制的 FULL 重播会被当作倒退丢弃**，页面永久停在崩溃前
的投影——正是 §4.2 声称要防的失败。持久化的化身号消除它。

`transitionId` 在每个化身内**连续**递增，客户端据此**检测缺帧**；缺帧时
发 `REQUEST_FULL_PROJECTION`（§6.1）主动恢复。

`attemptToken` 覆盖 **arm / disarm / policy 三类尝试**，且**必须在
manager 副作用发生之前、于同一 actor/manager 临界区内校验**——cue 一旦
响过就收不回来，事后丢回执没用。

### 2.2 desired 表（迁移 10004 重建）

| 列 | 说明 |
| --- | --- |
| `term_id` / `campus_code` / `section_index` | 主键 |
| `desired` | `INTEGER NOT NULL CHECK (desired IN (0,1))`；`0` 即 tombstone |
| `policy_json` | `TEXT NULL`；`CHECK ((desired = 1) = (policy_json IS NOT NULL))`，`CHECK (policy_json IS NULL OR json_valid(policy_json))` |
| `revision` | `INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991)` |
| `materialization_epoch` | `INTEGER NOT NULL`，同上界 |

metadata 增四个持久标量，**四者都带同一上界 CHECK**（v5 漏了
generation 的 CHECK）：`desired_watch_authority_generation`（初值 1）、
`desired_watch_revision_counter`、`desired_watch_materialization_counter`、
`desired_watch_actor_incarnation`（初值 1）。

epoch 在 §3.2 步骤 5 的同一 `IMMEDIATE` 事务内分配并落列（仅 `desired`
**值**变化时）；非 CAS 触发由 actor 在自己的小事务内分配并落列。

### 2.3 receipt ledger

| 列 | 说明 |
| --- | --- |
| `authority_generation` + `mutation_id` | **主键** |
| `term_id` / `campus_code` / `section_index` | 被比较列，**不入主键** |
| `fingerprint` | `(section, desired, policy 规范 JSON, basedOnRevision)` 的 sha256 |
| `outcome_json` | `TEXT NOT NULL CHECK (json_valid(outcome_json))` |

- 命中主键 + section 与 fingerprint 均相同 → **重放**（只回提交者，
  不重新 CAS、不 reconcile、不广播）；
- 命中主键但 section 或 fingerprint 不同 → **`MUTATION_ID_CONFLICT`**，
  **由已存在行推导，不插入第二条**；
- **记录范围**：**仅客户端提交**的成功与终局拒绝（`STALE_REVISION` /
  `LIMIT_EXCEEDED` / `UNSUPPORTED_TARGET` / `TERM_OUT_OF_RANGE` /
  `SECTION_NOT_FOUND`）；
- **`STALE_GENERATION` 不落 receipt**（步骤 1 即判定，落进哪个代都错）；
- **系统 CAS（§5.5）不落 receipt**：它是 authority 内部的，没有任何客户端
  会重放它。v5 让系统 CAS 的每次 `STALE_REVISION` 重试都写一行新 receipt，
  形成一个**绕过路由限流**的无界产出源。

### 2.4 mutationId 的粒度与新铸规则

- **每一个"发出的 section mutation"新铸一个 UUIDv4**——不是每个手势；
  一次批量 START 手势为每个 section 各铸一个；
- **绝不由内容派生**（派生读法会让被拒的 section 在本代内永久无法启动）；
- 系统 CAS 每次重试新铸 id（但不落 receipt，见 §2.3）；
- 绑定是**校验规则**：同一 id 必须始终配同一
  `(generation, section, fingerprint)`。

### 2.5 资源边界（**数值冻结**）

**loopback 协议输入不是可信的人类输入**。三条真实路径：用新 id 无限制造
终局拒绝 receipt；用新 SectionKey 无限制造 tombstone（`desired = false`
跳过全部准入）；tombstone 全部进入 bootstrap 与 FULL 投影，最终撑爆帧帽
**使重连/恢复永久失败**。

冻结（v5 只说"各设硬上限"却一个数都没给）：

| 项 | 值 |
| --- | --- |
| desired 路由 mutation 限流 | 令牌桶 **2/s，突发 60**（与公网 per-client 限流器同一实现模式与参数形状） |
| tombstone 行数硬上限 | **512** |
| receipt 行数硬上限 | **2048** |
| `PROJECTION_UPDATE{FULL}` 序列化字节上限 | **32 KiB**（帧帽 64 KiB 的一半） |
| `PROJECTION_UPDATE{DELTA}` 序列化字节上限 | **32 KiB**；单帧装不下时按 section 切分为多帧，**每帧仍是完整 entry** |
| bootstrap 的 `desiredWatches` 序列化字节上限 | **32 KiB**（bootstrap 是 HTTP GET，不受 WS 帧帽约束，但受同一预算以免恢复路径分叉） |
| rotation 触发阈值 | 任一预算达 **80%** 即触发原子 rotation（§2.6） |

### 2.6 压缩 / generation rotation（原子，且不得丢意图，也不得摧毁健康 watch）

1. `authorityGeneration += 1`；
2. **保留并重写全部 `desired = 1` 行**，分配新代的合法 `revision` 与
   `materialization_epoch`；
3. **只删除** tombstone 与全部 receipt；
4. 修复四个计数器；
5. 以上全部在**同一个 SQLite 事务内**；提交后广播一帧
   `PROJECTION_UPDATE{kind: FULL}`。

**rotation 与 actor 重建换 epoch 时必须走 §5.2 的 adopt 路径**：物理
watch 仍然健康且正确，不得因为 epoch 变了就 `Restarted`（那会结束旧
episode、换 ID、可能重新响铃）。v5 未给例外，等于让一次自动压缩把 9 份
watch 全部拆建一遍。

### 2.7 Full Reset

同一事务内：真删 desired 表与 receipt ledger 全部行、generation +1、
三个计数器归零（`actorIncarnation` **不归零**）；提交后广播
`PROJECTION_UPDATE{kind: FULL}`。

## 3. CAS

### 3.1 冲突后必须终止，且服从单调守卫

`MUTATION_RESULT` **只说结果与原因，不带任何状态**（v5 保留的
`COMMITTED { revision }` 是状态，会诱导客户端拿它当 `basedOnRevision`，
从而在并发下产生虚假 `STALE_REVISION`）。

**`basedOnRevision` 只能取自投影 store**——这是唯一来源，冻结。

客户端收到终局拒绝后**必须终止该 mutation**，**不得自动改号重发**；只有
用户在新界面上做出的**新手势**才允许以**新 id** 提交。（authority 自身的
系统 CAS 不受此约束，见 §5.5。）

`AUTHORITY_UNAVAILABLE` 是**非终局**：不落 receipt，同 id 允许原样重试。

### 3.2 CAS 规则（单个 `IMMEDIATE` 事务内，顺序即校验序）

1. `authorityGeneration != current` → `STALE_GENERATION`（不落 receipt）；
2. receipt ledger 幂等判定（§2.3）；
3. `basedOnRevision != 该 section 当前 revision`（无行为 0）→
   `STALE_REVISION`；
4. **仅当 `desired = true`**：**产品准入帽**（`desired = 1` 行数 < 9）→
   `LIMIT_EXCEEDED`；target 受支持 → `UNSUPPORTED_TARGET`；term 在窗口内
   → `TERM_OUT_OF_RANGE`；section 已发布**且 S1 gate 已放行** →
   `SECTION_NOT_FOUND`（**gate 未放行时不得拒绝**，放行提交、由 arm 侧按
   `PENDING_GATE` 暂态重试）；
   **`desired = false` 跳过全部准入校验**；
5. 写入、`revision = ++revCounter`、`materialization_epoch = ++epochCounter`
   （仅 `desired` **值**变化；policy-only 不换）、写 receipt、提交。

## 4. authority transition actor

### 4.1 帧与轮次

单一本地 actor（单线程 inbox），按 inbox 顺序逐项完成
`CAS commit → manager reconcile → 至多一帧 PROJECTION_UPDATE`。

**每轮至多一帧**（v5 写"恰好一帧"与两条自身路径矛盾：receipt 重放轮次
不广播 = 零帧；§4.4 的"下一帧" = 两帧）。轮次若无投影变化则不发帧，
`transitionId` 也不递增（保持连续性）。

### 4.2 必须进入 inbox 的转移

用户 mutation；`AttachAudience` / `DetachAudience`（**每一次**，连接计数
与 `0↔1` 边沿由 actor 自算）；Full Reset confirm；generation 轮换 / 压缩；
**`SlotReleased`**（物理槽释放，见 §5.7）；reconcile 重试完成回执；
`REQUEST_FULL_PROJECTION`；启动恢复；**actor 故障重建**。

**actor 故障重建必须向所有现存 audience 重播 `kind: FULL`**，并先
`++actorIncarnation`（§2.1）——否则重播帧会被帧顺序守卫当成倒退丢弃。

### 4.3 epoch 作废规则

**换新 epoch**：该 section 的 `desired` **值**变化的 CAS 提交；Full Reset；
generation 轮换；连接 `1→0` 与新的 `0→1`；actor 故障重建。
其中 **rotation 与重建走 adopt**（§2.6、§5.2），不重建物理 watch。

**不换 epoch**：**policy-only 更新**——保留 `activeWatchId` 与 episode，
经 §5.2 的 `applyPolicy` 就地更新。

### 4.4 AttachAudience 的顺序

`PROJECTION_UPDATE{kind: FULL}` 在该轮次**开始处**从 owner map 序列化，
**它就是该轮次的那一帧**；该轮触发的任何物化变更由 actor **自行入队一条
后续 inbox 项**（`MaterializeDue`），在其后的轮次发出各自的 DELTA。
v5 写"排在其后的下一帧"既违反每轮一帧，又没有任何东西调度那个"下一帧"。

## 5. 服务端拥有的监控与失败模型

### 5.1 逻辑 owner

`LocalWatchOwner` 持有：

```
section → {
  generation, revision, materializationEpoch,     // desired 侧
  materialized: {...} | null,                     // §0.1
  attemptToken, armed, pendingDisarm, blockedOnSlot, lastFailure
}
```

owner 在 manager 中是一条**内部合成连接**：新增
**`ConnectionKind::Owner`**，**不可由 wire 请求构造**；**豁免心跳过期**，
**不豁免物理 watch 帽**。

### 5.2 owner API（三个，全部按 `(generation, epoch, attemptToken)` 幂等）

- **`ensure(section, generation, epoch, policy, attemptToken)`**
  - 同 `(generation, epoch)` 重试 → **收养**原有 `activeWatchId`，
    **零 manager 副作用**；
  - **新 epoch 但物理 watch 仍健康**（rotation / actor 重建）→ **adopt**：
    只把物化记录重新盖章为新 `(generation, epoch)`，**不走 `Restarted`**；
  - **新 epoch 且需要替换**（desired 值变化后的重新武装）→ 才允许
    `Restarted` 语义；
- **`applyPolicy(section, generation, epoch, policy, attemptToken)`**
  （v5 缺失）：policy-only 路径。v5 只有 `ensure` 且它在同
  `(generation, epoch)` 下"零副作用"，于是 policy-only 更新**永远无法生效**，
  `materialized.policy` 永不前进，section 永久非绿——一个活锁；
- **`disarm(section, targetActiveWatchId, epoch, attemptToken)`**：
  见 §5.4。

`RejectedDuplicate` 在本模型下**不可达**：owner 逐 section 调用，不批量
提交。（它在 manager 里只表示同一批 START 内的重复 section；**已持有的
section 实际走 `Restarted`**，会换 ID、结束旧 episode、可能重新响铃——
这正是必须有 adopt 路径的原因。）

### 5.3 失败分类

| disposition | 分类 | 处置 |
| --- | --- | --- |
| `RejectedTargetUnavailable`、存储忙、快照未就绪 | **暂态** | 退避 **5s/10s/20s 封顶 30s** 无限重试；desired 不动 |
| `RejectedSectionNotFound` | **取决于 gate** | gate 放行 → 永久；gate Hold/隔离 → `PENDING_GATE` 暂态 |
| `RejectedUnsupportedTarget`、`RejectedTermOutOfRange` | **永久** | §5.5 系统 CAS 移出 desired + 通知 |
| `RejectedLimit`（owner 连接）**且存在 pendingDisarm** | **预期路径，非告警** | 置 `blockedOnSlot = true`，等 `SlotReleased`（§5.7） |
| `RejectedLimit`（owner 连接）**且无 pendingDisarm** | **不变量告警** | 说明 CAS 帽与物理帽已失配，上报；仍**不**删除 desired |
| disarm 返回 `UnknownWatch` | **不是失败** | 目标已不存在即达成目的 |

**产品帽与物理帽是两类事件，不可混淆**：CAS 步骤 4 的 `LIMIT_EXCEEDED`
是**面向用户的永久拒绝**（与已批准设计"超 9 门属 admission 类永久拒绝"
一致）；owner 连接上的 `RejectedLimit` 是**内部物理槽条件**，绝不触发
系统 CAS 删除 desired。该区分需写回
`docs/design/2026-08-20-alert-delivery-integrity.md`（§10）。

退避与 retry-epoch 出处：该文档**第 57–61 行**；"永久拒绝移出 desired +
通知"在 **214–215 / 250 行**。

### 5.4 disarm

- **在发起时捕获** `(section, targetActiveWatchId, epoch)`，退避重试
  **始终针对被捕获的那个 id**，**绝不在执行时重新解析**。v5 写"执行时
  由 owner 解析当前 id"，在 §0.3 第 1 条明确合法的"STOP → 退避中 → 用户
  又 START"下，会把**用户当前想要的那份新 watch 拆掉**；
- epoch 变化**取消在途的 arm 尝试**，但**不取消在途的 disarm**——旧
  watch 仍必须死；
- 三种 disarm（`desired = false`、Full Reset、连接 `1→0`）失败按同一退避
  重试至成功；期间 `pendingDisarm = true`（§0.3 第 1 条），**绝不谎报
  已停**；
- 成功后置 `pendingDisarm = false` 并向 actor 投递 **`SlotReleased`**；
- `activeWatchId` 可空；每个"需要替换"的新 epoch 换新；adopt 与
  policy-only 不换；
- **历史身份不变**：仍是既有的 `(section_key, run_id, episode_id)`；
- cue 的 exactly-once 由 manager 既有 per-episode audible 计数 + owner 是
  唯一生产者 + `attemptToken` 的**前置**校验共同保证。

### 5.5 永久失败：authority 自身的系统 CAS

authority 提交一次 `desired = false` 的 CAS，`basedOnRevision` 取读到的
当前值，每次重试新铸 id，**不落 receipt**（§2.3），`source = SYSTEM`；
并发丢失时**重读重试**（§3.1 的"冲突即终止"只约束客户端）；若重读后该
section 已被用户删除，目的已达成，不再提交。

### 5.6 零连接暂停

`0→1` 读权威 snapshot 物化一次（新 epoch，走 adopt/ensure）；`1→0` 只
销毁 active manager 状态、保留 desired；
`ACTIVE_WATCH_STATE_PERSISTENT = false` 继续成立。

### 5.7 两个帽是两个不变量

- **CAS 是唯一的产品准入帽**：`desired = 1` 行数 ≤ 9；
- **manager 保留独立物理帽**，owner 连接**不豁免**——否则
  `9 armed → 9 pendingDisarm → 再 START 9 个` 会得到 18、27、36 份物理
  watch；
- 旧槽仍被 pending disarm 占用时，新 section 的 CAS 照常成功，物化推迟，
  `blockedOnSlot = true`，UI 显示**准备中（等待旧槽释放）**；
- 槽释放时 §5.4 投递 **`SlotReleased`**，actor 立刻重试被阻塞的 arm——
  **不依赖 5–30s 的退避**（v5 让一次普通换课要等最多 30s）。

## 6. 契约

### 6.1 命令（本地专有）

```
SET_DESIRED_WATCH {
  section, desired, policy | null,
  basedOnRevision, authorityGeneration, mutationId
}
REQUEST_FULL_PROJECTION { }        // 客户端检测到缺帧后主动重同步
```

### 6.2 事件

```
PROJECTION_UPDATE {
  actorIncarnation, transitionId, authorityGeneration,
  kind: FULL | DELTA,
  entries: [ ProjectionEntry ]
}

ProjectionEntry {
  section,
  desired, policy | null, revision, materializationEpoch,   // desired 侧
  materialized: { generation, revision, epoch, policy,      // §0.1；可空
                  activeWatchId } | null,
  attemptToken, armed, pendingDisarm, blockedOnSlot,
  classification: TRANSIENT | PERMANENT | PENDING_GATE | null,
  reason | null
}

MUTATION_RESULT {                  // 单播给提交者；**不含任何状态**
  mutationId,
  outcome: COMMITTED | REJECTED { reason } | REPLAYED { outcome }
}

AUTHORITY_UNAVAILABLE { mutationId }   // 单播；非终局；无状态字段
```

`reason` ∈ `STALE_GENERATION` / `STALE_REVISION` / `MUTATION_ID_CONFLICT` /
`LIMIT_EXCEEDED` / `UNSUPPORTED_TARGET` / `TERM_OUT_OF_RANGE` /
`SECTION_NOT_FOUND`。

### 6.3 bootstrap

`desiredWatchAuthorityGeneration` 为 snapshot **顶层 required 字段**；
`desiredWatches` 条目携带 `desired` / `policy | null` / `revision` /
`materializationEpoch`，**含 tombstone**，受 §2.5 的 32 KiB 预算；
bootstrap **不含** materialization 运行态（随连接由第一帧
`PROJECTION_UPDATE{FULL}` 下发）。

### 6.4 数值边界与 scalar 归属

`authorityGeneration` / `sectionRevision` / `materializationEpoch` /
**`attemptToken`** / **`transitionId`** / **`actorIncarnation`** 六个量
全部收敛到 **`≤ Number.MAX_SAFE_INTEGER`**（v5 漏了后三个）。三处钉死：
SQLite CHECK（持久的四个）、**本地专有**契约 scalar、前端本地专有校验器。
**不得放进 `bcsp-contracts`**（公网闭包内）。

## 7. 放置、拓扑与公网边界

### 7.1 Rust 放置

`bcsp-contracts` 与 `bcsp-application` **都在公网 Cargo 闭包内**。desired
命令、事件与 authority **全部放在本地专有 crate**；共享 host 只经 **tag
无关**的 `WebSocketExtension` 承载。

### 7.2 secondary 路由：受校验的**路由集合**

host 现为单槽 `secondary_socket`（状态字段、构造器参数、路由注册与
handler 各一处；**按符号名引用**——行号已被证实漂移）。presence 已占
`/api/v1/local/presence`。

- `HostState` 需要 **`path → Arc<dyn WebSocketExtension>` 映射**，
  handler 按 `request.uri().path()` 选择；
- `SecondaryWebSocketRoute::new` 改为 **fallible**（承载 S2a 路径校验），
  `LoopbackServerError` 新增变体；
- **集合内路径唯一性校验是必需项**：axum `Router::route` 在重复路径上
  **panic**；
- 路径冻结：presence `/api/v1/local/presence`、desired
  `/api/v1/local/desired-watch`。

### 7.3 既有携带项绑定为硬验收

**S2c**：共享 `serve_websocket` 补 **64 KiB** 帧/消息上限；
**S2b**：未注入 → **404**，集合化后**逐路由**钉死。

### 7.4 N-g 措辞

公网严格解码器把 desired tag 当未知命令拒绝：**不进入命令路由、不改变
watch 状态、不产生 dispatch/sink/reply**。

### 7.5 旧命令去留 + 服务端准入过滤器

公网保留现有 ephemeral 语义；**本地外部协议拒绝**；内部经 Rust API。
**新增中立 manager API 本身不拒绝外部旧命令**——本地必须有一个**显式的
服务端 command-admission filter**：本地 watch socket 收到这三条 tag 时
直接拒绝并计数，不进入 `route_command`。

### 7.6 前端接缝：target-neutral 意图/投影 port

`ProductRuntimePort.watch` 现为 wire 专用的 `WatchClientPort`，共享
`LiveWatchProvider` 自己构造 START/STOP/UPDATE_POLICY，并且**还消费
`state` / `subscribeState` / `connect` / `disconnect`**（连接态是清单第 5
条"当前 connection/retry epoch 下"的判定输入，**不能丢**）。

冻结端口形状：

```
watch: {
  // 意图：返回终局结果，使 MUTATION_RESULT 能到达 UI（v5 的洞）
  setDesired(section, desired, policy): Promise<DesiredOutcome>
  //   DesiredOutcome = { status: COMMITTED | REJECTED | UNAVAILABLE,
  //                      reason?: <§6.2 的 reason> }
  subscribeProjection(listener): unsubscribe     // §0.3 五态
  // 连接态：清单第 5 条需要
  readonly state; subscribeState(listener); connect(); disconnect()
  // episode 命令：两 target 相同，**六条全列**
  episodes: { acknowledgeEpisode, acknowledgeAllEpisodes,
              resumeTimedOutEpisode, resetAudibleCount,
              reportCueOutcome, dismissAlert }
}
```

- **公网 adapter**：`setDesired` → 旧 wire 命令；`DesiredOutcome` 由
  `START_RESULT` 的 rejection 映射；投影由 `START_RESULT` /
  `WATCH_STOPPED` 合成（ephemeral：generation/revision 取退化值，
  `pendingDisarm` / `blockedOnSlot` 恒 false）；
- **本地 adapter**：`setDesired` → 逐 section 新铸 UUID、**从投影 store
  取 base revision**（§3.1）→ `SET_DESIRED_WATCH`；投影来自
  `PROJECTION_UPDATE`；缺帧时发 `REQUEST_FULL_PROJECTION`。

共享 provider 只消费投影与连接态，不再构造 desired 类 wire 命令。

### 7.7 public marker 计账（**冻结到数组顺序**）

`public-source-deny.json` 现有 **18 行 capability**，
`PERSISTENT_ACTIVE_WATCH` 行 **13 个 marker**，`markerSetVersion = 1`，
marker 总数 **212**。

**不新增 capability 行**（会变 19/19，与"保持 18"矛盾）。在
`PERSISTENT_ACTIVE_WATCH` 行追加 **3 个** marker——不是 v5 写的 6 个：
匹配是**规范化后的子串包含**，`desired_watch` 已经吞掉
`desired_watches` / `desired_watch_authority` / `desired_watch_receipt`，
那三个是死条目。

且 verifier **要求行内 marker 按序数序排列**，所以必须**插入**而非
追加。冻结**结果数组**（16 项）：

```
active_watch_persistence, active_watch_store, authority_generation,
auto_rewatch, automatic_rewatch, desired_watch, materialization_epoch,
persisted_active_watch, persistent_active_watch, restore_subscription,
restore_watch, restored_watch, subscription_restoration,
watch_rehydration, watch_repository, watch_storage
```

**新总数 212 + 3 = 215**。同步项：

- `markerSetVersion` **1 → 2**，三处：policy 文件、Rust verifier、
  frontend verifier；
- **Rust 侧 whole-document SHA** 与 **frontend 侧 rows-only SHA** 重算
  （两者都对**数组顺序**敏感，故上面冻结了顺序）；
- `frontend/tools/verify-import-graph.test.mjs` 的硬编码
  **18/212 → 18/215**；
- **五组校验面、八个断言面各补正反例**：Rust graph（1）、Rust zero
  surface 的 SOURCE/API/STORAGE/PACKAGE（4）、frontend import graph（1）、
  target build（1）、public DOM/bundle（1）；
- **local capability manifest 增精确 slug `persistent-desired-watches`**，
  **public manifest 必须不含**。

## 8. 迁移 10004 与同步面

- 重建 `personal_desired_watches_v1`（§2.2），新建
  `personal_desired_watch_receipts_v1`（§2.3）；
- 10003 遗留行升级为 `desired = 1`，按 `(term, campus, index)` 升序从 1
  起分配 `revision` 与 `materialization_epoch`；
- metadata 写入 generation = 1、`actorIncarnation` = 1、两个计数器 =
  已分配最大值；
- 在既有 personal runner 的**单事务 + 可选 Rust after-hook** 模式内完成。

**同步面**：两个 allowlist、`PersonalTableCounts`、`PersonalResetResult`、
schema 子串守卫（新表不得含 connection/active_watch 字样）、Rust
`PersonalStateSnapshot` 与 `DesiredWatch` 形状、`bcsp-local-runtime` 的
bootstrap 编码，**以及 `frontend/src/ui/local/personal/contracts.ts`**：

1. `desiredWatches.length <= 9` 必须改为"**`desired = 1` 的条目数 ≤ 9**"
   ——保留 tombstone 后第 10 个 tombstone 会让整个 bootstrap 解析失败；
2. `hasKeys` 是**键集全等**检查：新增顶层
   `desiredWatchAuthorityGeneration` 会打挂 `isPersonalStateSnapshot`，
   条目新增字段会打挂 `isDesiredWatch`；
3. 同步三处 bootstrap fixtures（S2-PR3.1 的先例）。

## 9. 验收清单（自包含，A-01..A-36）

**CAS 与身份**
- A-01 STOP 后延迟旧 START → `STALE_REVISION`，不得重插已停意图；
- A-02 新 START 后延迟旧 STOP → 对称；
- A-03 旧 policy 不得覆盖新 policy；
- A-04 reset 升代后前代写入 → `STALE_GENERATION`；
- A-05 上述四例中**旧命令必须实际迟到**，不得只模拟入队/SQLite 延迟；
- A-06 幂等三态：同 id 同 (section, fingerprint) 重放；同 id 异 section
  或异 fingerprint → `MUTATION_ID_CONFLICT` 且**不插入第二行**；
- A-07 `STALE_GENERATION` 不落 receipt；**系统 CAS 不落 receipt**；
- A-08 `AUTHORITY_UNAVAILABLE` 不落 receipt、同 id 重试可成功；
- A-09 终局拒绝后原因消失，**新手势新 id** 必须能成功；
- A-10 **客户端收到终局拒绝后不得自动改号重发**（A-09 的反向；
  与 A-11 的方向相反，是最易实现反的一处）；
- A-11 系统 CAS 重试每次新铸 id；
- A-12 receipt 主键为 `(generation, mutationId)`，section 为被比较列；
- A-13 `basedOnRevision` 只取自投影 store；`MUTATION_RESULT` 不带状态；

**顺序与帧流**
- A-14 每个 actor 轮次**至多一帧**；无变化则不发帧且 `transitionId`
  不递增；
- A-15 前端 reducer **整帧提交**，任何中间态都不得渲染；
- A-16 actor 故障重建：`++actorIncarnation` 且向现存 audience 广播
  `kind: FULL`；"COMMIT 后 panic"交错下所有页面收敛；
- A-17 **帧顺序守卫为 `(actorIncarnation, transitionId)`**：重建后的
  FULL 帧**不得**被当作倒退丢弃；
- A-18 缺帧检测：`transitionId` 每化身内连续；客户端缺帧后发
  `REQUEST_FULL_PROJECTION` 并恢复；
- A-19 `AttachAudience` 的 FULL 帧在该轮次开始处序列化并**就是该轮的
  那一帧**；同轮触发的物化变更经自入队的后续轮次发出；
- A-20 Full Reset 后 epoch 计数器归零，armed 侧仍按
  `(generation, epoch, attemptToken)` 正确排序；重置前在途的 epoch-1
  完成回执不得复活 watch；
- A-21 `attemptToken` 在 manager 副作用**之前**校验（不得先响铃再丢
  回执），且覆盖 arm/disarm/**policy** 三类尝试；

**投影语义**
- A-22 §0.3 的**有序**判定逐条钉死；特别是 STOP 提交后 `armed` 变
  false 时**不得**落到"未监控"；
- A-23 §0.1 严格 `armed`：policy 已提交但未在 manager 生效 ⇒ **非绿**；
  且四个比较项在 `ProjectionEntry` 上**可计算**；
- A-24 `pendingDisarm`（自己在拆）与 `blockedOnSlot`（等别人腾槽）是
  **两个字段**，各自的 UI 文案不同；
- A-25 迟到的终局拒绝不得把已前进的前端投影拉回；
- A-26 前端 store 丢弃更旧 generation 的残留状态；

**物化与资源**
- A-27 `ensure` 按 `(generation, epoch)` 幂等：同 key 重试**收养**原
  `activeWatchId`、零副作用、无重复响铃；
- A-28 **adopt 路径**：rotation 与 actor 重建换 epoch 时，健康的物理
  watch **不得**被 `Restarted`（不得结束 episode / 换 ID / 重新响铃）；
- A-29 **`applyPolicy` 使 policy-only 更新真正生效**：
  `materialized.policy` 前进、section 转绿（v5 在此活锁）；
- A-30 **一般暂态 arm 失败不回滚 desired**，reconciler 重试后转 armed；
- A-31 两个帽两个不变量：9 armed → 9 STOP 全 pending → 再 START 9 个，
  物理 watch **不得**超过帽；新 section `blockedOnSlot`，**`SlotReleased`
  到达后立即 arm**，不等退避；
- A-32 owner 的 `RejectedLimit`：有 pendingDisarm 时是**预期路径不告警**、
  无 pendingDisarm 时是**不变量告警**；两者都**不**删除 desired；
- A-33 owner 合成连接豁免心跳过期但**不豁免**物理帽；
  `ConnectionKind::Owner` 不可由 wire 构造；
- A-34 `disarm` 用**发起时捕获**的 `activeWatchId`；"STOP → 退避中 →
  用户又 START"下**不得**拆掉新武装的 watch；
- A-35 `desired = false` 对过期/不受支持 section 仍成功；永久失败经系统
  CAS 移出 desired 并通知；`SECTION_NOT_FOUND` 在 gate 未放行时按暂态；

**边界与预算**
- A-36 §2.5 的**六个冻结数值**逐项生效（2/s·60 突发、512、2048、
  32 KiB×3、80% 触发）；rotation **保留全部 `desired = 1` 行**、只清
  tombstone 与 receipt；公网：desired tag 被拒且不进入命令路由/不改
  watch 状态/不产生 dispatch/sink/reply；capability 行保持 18、marker
  总数 215、数组顺序与两个摘要同步；local manifest 含
  `persistent-desired-watches`、public 不含；本地旧三命令被服务端
  admission filter 拒绝。

## 10. 已裁定 + 写回义务

- **打包冒烟播种：P1 release gate**。必须在**首个生产 writer / 整批发布
  之前**完成；使用**确定性的 PUBLISHED section fixture**并确保 S1 gate
  放行；`desired = false` 的 tombstone 提交**不能**替代 true-path 验收。
- **写回 `2026-08-20-alert-delivery-integrity.md` 两条**：
  1. 测试 1c 取代但**翻译**：leader 关闭且仍有 audience 时，
     `activeWatchId` / `materializationEpoch` / manager watch 计数**均
     不变**，**无 re-arm、无重复 episode**；新 leader **只**接管音频；
     跨 tab 的 STOP 仍收敛；
  2. **9 门帽的两类事件区分**（§5.3）：面向用户的 CAS `LIMIT_EXCEEDED`
     仍属 admission 类永久拒绝；owner 连接上的物理 `RejectedLimit` 是
     内部槽条件，**不**移除 desired。
