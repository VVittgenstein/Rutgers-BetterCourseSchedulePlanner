# 期望监控：revision/CAS 共同编辑模型（S2-D3 路线改判）

状态：ACTIVE，**v2**（Codex 驳回 v1 的 5 组设计阻断，本稿逐条闭合）。
路线由产品所有者 2026-08-22 裁定：取代 fenced sequencer，走清单二选一的
另一条——**持久化 revision + tombstone + CAS**。
S2-D3 硬门在本设计实现并复审通过前**保持关闭**。

## 0. 为什么改判（证据）

S2-PR5 曾按 fenced sequencer 实现并被自查三镜头**实跑复现**两项 P1：

1. `epoch`（连接注册序）压过 `sequence`，且 `last_by_section` 只在 reset
   时清除，于是**只要某 section 被较新连接写过，任何较旧但仍存活的连接
   对它的写入就被永久静默丢弃**。复现：B 标签页给 S 上过 watch 后关闭，
   仅存的 A 标签页里用户点 START —— manager 正常准入且会响铃，意图却没
   落库。**活跃 watch 与持久意图反向错位**。
2. 反向去掉 epoch（纯到达序）后，反例 (a)"STOP 后延迟旧 START"必然失败。

根因是**模型错配**而非缺陷：栅栏假设单写者，产品模型是多页面平权编辑。
两条同时存活的连接之间，服务端在字节流上无法区分"迟到旧帧"与"新点击"，
判定信息必须由客户端携带——`basedOnRevision` 正是它。

## 1. 产品模型（裁定）

- **所有标签页平权编辑**，不存在用户可见的"只有 leader 能改"；
- **`desired_watches` 是唯一真相**，页面显示的是它的投影；
- **实际监控由服务端按已提交状态统一维护**，不由某个页面"拥有"；
- **leader 只剩一件用户看不见的事**：避免多个页面同时响铃。

```
页面 A ─┐
        ├─→ authority（CAS 提交）→ 物化监控 → 广播回所有页面
页面 B ─┘
```

## 2. 权威状态：generation + revision + tombstone

单调键是二元组 **`(authorityGeneration, sectionRevision)`**，所有应用点
（authority、manager 物化器、广播、前端 store）一律按它单调合并，
**拒绝倒退**。

`personal_desired_watches_v1`（迁移 10004 重建）：

| 列 | 说明 |
| --- | --- |
| `term_id` / `campus_code` / `section_index` | 主键 |
| `desired` | `INTEGER NOT NULL CHECK (desired IN (0,1))`；`0` 即 tombstone |
| `policy_json` | `TEXT NULL`；`CHECK ((desired = 1) = (policy_json IS NOT NULL))`，且 `CHECK (policy_json IS NULL OR json_valid(policy_json))` |
| `revision` | `INTEGER NOT NULL CHECK (revision BETWEEN 1 AND 9007199254740991)` |
| `last_mutation_id` | `TEXT NULL`，幂等键（§3） |
| `last_mutation_fingerprint` | `TEXT NULL`，与 id 绑定的请求指纹 |
| `last_mutation_outcome` | `TEXT NULL`，原提交结果（幂等重放用） |

`personal_state_metadata_v1` 增两个持久标量：
`desired_watch_authority_generation`（初值 1）与
`desired_watch_revision_counter`（初值 0），二者均带同一上界 CHECK。

### 2.1 tombstone 永不在同一 generation 内被回收（P1-1 闭合）

v1 的"256 行上限 + 淘汰最旧 tombstone"**引入 ABA**，已废除：

```
A 的首个 START 延迟（base=0）
B START → r1
B STOP  → tombstone r2
GC 淘汰 tombstone → 该 key 回到 rev0
A 的旧 START(base=0) 被接受   ← 击穿反例 a，且 last_mutation_id 一并丢失
```

**裁定采纳**：tombstone **保留到 Full Reset**。256 仅作为**观测/告警
阈值**，不触发任何淘汰。若将来确需硬压缩，唯一允许的做法是：原子提升
`authorityGeneration` → 使所有旧命令失效 → 广播完整 snapshot → 再批量
清理；**任何情况下都不得让单个 key 在同一 generation 内回到 revision 0**。

因此"某 key 的当前 revision"只有两种来源：行上的 `revision`，或"本
generation 内从未被编辑过"的 `0`。tombstone 存在即保证前者。

### 2.2 Full Reset

Full Reset 真删全部行（tombstone 也是个人数据），并**原子地**：
generation +1、revision 计数器归零、广播完整 snapshot（§6.4）。
跨 generation 的迟到命令由 `STALE_GENERATION` 拒绝。

## 3. 变更身份与冲突语义

### 3.1 mutationId 绑定（P1-4）

`mutationId` 绑定到 **`(authorityGeneration, section, 请求指纹)`**，
指纹 = `(desired, policy 的规范 JSON, basedOnRevision)` 的 sha256。

| 情形 | 结果 |
| --- | --- |
| 同 id + 同指纹 | 返回**原提交结果**：不增 revision、不重复 manager 效果、不重复广播 |
| 同 id + 不同指纹 | `MUTATION_ID_CONFLICT` |
| 新 id | 正常走 §3.3 |

`mutationId` 在**成功与拒绝结果里都必须回显**，页面据此把结果与自己的
待决操作对上。

### 3.2 冲突后必须终止，不得改号重发（P1-1）

v1 写的"`STALE_REVISION` 后换成 `currentRevision` 重发"会把**刚被拒绝
的旧 START 再次合法化**，等于绕过反例 a。改为：

> 拒绝结果里回带的当前真相**只用于更新投影**，它**不是重试令牌**。
> 收到冲突的一方必须**应用服务器真相并终止该 mutation**。只有用户在
> **看到冲突后的新界面上**做出的**新操作**，才允许以**新的 mutationId**
> 提交。

`AUTHORITY_UNAVAILABLE` **不携带任何状态字段**，不得伪造"当前真相"。

### 3.3 CAS 规则（单个 `IMMEDIATE` 事务内，顺序即校验序）

1. `resetGeneration != current` → `STALE_GENERATION`；
2. mutationId 幂等判定（§3.1）；
3. `basedOnRevision != 该 section 当前 revision`（无行为 0）→
   `STALE_REVISION`；
4. **仅当 `desired = true`** 时依次校验：9 门帽（只数 `desired = 1`
   的行）→ `LIMIT_EXCEEDED`；target 受支持 → `UNSUPPORTED_TARGET`；
   term 在窗口内 → `TERM_OUT_OF_RANGE`；section 已发布 →
   `SECTION_NOT_FOUND`；
   **`desired = false` 跳过全部准入校验**——过期/不受支持的 section
   也必须允许删除，否则用户会被永久卡住；
5. 写入、`revision = ++counter`、记录 mutation 三元组、提交。

## 4. 提交顺序：authority transition actor（P1-2 闭合）

v1 只保证了 SQLite 串行，**投影顺序未被约束**：

```
START 提交 r1 → 暂停
STOP  提交 r2 → manager STOP、广播 r2
START 恢复    → manager START、广播 r1     ← 存储是 STOP，manager/UI 是 START
```

改为**单一 authority transition actor**（本地专有、单线程 inbox），
它是下述三步的**唯一**执行者，按 inbox 顺序逐项完成：

```
CAS commit  →  manager reconcile  →  event enqueue
```

进入同一 inbox 的转移**必须**包括：用户 mutation、**Full Reset**
（清单 h 改写为此）、以及**连接 0→1 / 1→0 转换**（§5.2）。

这不是被否决的 connection-epoch sequencer：**新旧由 CAS 判定**，actor
只保证已提交 revision 的**投影顺序**。

**纵深防御**：即便 actor 存在，manager 物化器、广播序列化器、前端 store
三处应用点仍各自**拒绝倒退的 `(authorityGeneration, sectionRevision)`**。

## 5. 服务端拥有的监控（P1-3 闭合）

现有 `WatchManager` 的 watch、dispatch、disconnect 全部是
connection-owned / unicast，无法表示"服务端统一拥有的一份监控"。选某个
tab 当 owner 会在它退出时停表；每 tab 各起一份会重复 episode、history
与响铃。冻结如下：

### 5.1 逻辑 owner

- 本地引入 **connection-independent 的逻辑 owner**（`LocalWatchOwner`），
  它持有 `section → { activeWatchId, generation, revision, armed }`；
- 浏览器连接只是**编辑者 + 事件 audience**，不拥有任何 watch；
- **每个 section 只产生一份** active watch、一份 episode 流、一份 cue；
- **持久提交成功即为真相**。manager 暂时 arm 失败时，标记该 section
  `armed = false` 并由**常驻 reconciler** 按退避重试；**绝不回滚
  desired**；
- 物化事件是**本地专有**的、携带 `(generation, revision, activeWatchId)`
  的新事件，**不复用** connection-scoped 的 `START_RESULT`。

### 5.2 零连接暂停（裁定采纳）

- `0→1`：读取权威 snapshot 并**物化一次**；
- `1→0`：只销毁 active manager 状态，**保留 desired**；
- `activeWatchId` / episode 下次重新生成，
  `ACTIVE_WATCH_STATE_PERSISTENT = false` 继续成立；
- 两个转换都走 §4 的同一 inbox。

## 6. 契约（P1-4 闭合）

### 6.1 命令（本地专有，见 §7）

```
SET_DESIRED_WATCH {
  section, desired, policy | null,
  basedOnRevision, resetGeneration, mutationId
}
```

### 6.2 结果与广播

```
DESIRED_WATCH_COMMITTED {            // 广播给所有本地连接
  section, desired, policy, revision, authorityGeneration, mutationId
}
DESIRED_WATCH_REJECTED {             // 只回提交者；投影用，非重试令牌
  section, mutationId,
  reason: STALE_GENERATION | STALE_REVISION | MUTATION_ID_CONFLICT
        | LIMIT_EXCEEDED | UNSUPPORTED_TARGET | TERM_OUT_OF_RANGE
        | SECTION_NOT_FOUND,
  current: { desired, policy, revision, authorityGeneration }
}
AUTHORITY_UNAVAILABLE { mutationId } // 无任何状态字段
DESIRED_WATCH_SNAPSHOT {             // generation 轮换 / Full Reset
  authorityGeneration, entries[]
}
DESIRED_WATCH_MATERIALIZED { section, generation, revision, activeWatchId }
DESIRED_WATCH_UNARMED     { section, generation, revision, reason }
```

### 6.3 bootstrap

- `desiredWatchAuthorityGeneration` 是 **snapshot 顶层 required 字段**
  （否则空列表或 Full Reset 之后页面无法初始化）；
- `desiredWatches` 条目携带 `desired` / `policy | null` / `revision`，
  **包含 tombstone**——页面要编辑一个自己删过的 section，必须能拿到它的
  `revision` 才能发出合法的 `basedOnRevision`。

### 6.4 generation 轮换

Full Reset 与任何未来的 generation 轮换**必须广播完整
`DESIRED_WATCH_SNAPSHOT`**；客户端按 `(generation, revision)` 元组单调
合并，并**丢弃**任何更旧 generation 的残留状态。

### 6.5 数值边界

`revision` 与 `authorityGeneration` 在 Rust 侧为 `u64`，在 JS 侧为
`number`。统一收敛到 **`≤ Number.MAX_SAFE_INTEGER`(9007199254740991)**：
SQLite CHECK、契约 scalar 与前端校验器三处同时钉死。（备选方案"改十进制
字符串"未采用，理由是它会污染所有比较点。）

## 7. 放置与公网边界（P1-5 闭合）

v1 把 `SET_DESIRED_WATCH` 与 `DesiredWatchAuthority` 放进共享
`bcsp-contracts` / `bcsp-application`——**这两个 crate 都在公网 Cargo
闭包内**，等于把本地个人状态语义带进公网闭包。"解析后再拒绝"不足以支撑
`PUBLIC_RUST_ZERO_SURFACE`。改为：

- desired 命令、事件与 authority **全部放在本地专有 crate**；
- 载体是 **S2-PR1 已建成的 `SecondaryWebSocketRoute` 接缝**上的一条
  **本地专有 WS 路由**（拟 `/api/v1/local/desired-watch`）。公网 host
  结构上从不构造该路由，公网严格解码器把这些 tag 当**未知命令拒绝**，
  且**不触碰 manager/sink**；
- 该通道后续可与 L2 presence 复用同一条本地 socket。

### 7.1 旧命令去留（裁定）

| | `START_WATCH` / `STOP_WATCH` / `UPDATE_POLICY` |
| --- | --- |
| 公网 | **保留**现有 ephemeral、connection-owned 语义，不变 |
| 本地 | **外部协议拒绝/退役**——否则可绕过 CAS |
| 内部 | manager 通过 **Rust API** 被物化器调用，**不把外部 wire command 当"内部载体"** |

本地页面对 episode 的操作（`ACKNOWLEDGE_*` / `DISMISS_ALERT` /
`REPORT_CUE_OUTCOME` / `HEARTBEAT_ACK`）**不属于** desired 状态，继续走
既有 watch WS 不变。

## 8. 迁移 10004

- 重建 `personal_desired_watches_v1` 为 §2 的形态；
- 10003 遗留行（语义上全是"活跃意图"）升级为 `desired = 1`，按
  `(term, campus, index)` 升序从 1 起分配 `revision`，mutation 三列为
  `NULL`；
- metadata 写入 `desired_watch_authority_generation = 1` 与
  `desired_watch_revision_counter = <已分配的最大值>`；
- 两个 allowlist、`PersonalTableCounts`、`PersonalResetResult`、
  schema 子串守卫按既有规则同步。

## 9. 验收映射（与清单的关系）

清单 S2-D3 的 a–d 四反例继续有效；e–i **不整体作废**：

| 清单条目 | 本模型下的形态 |
| --- | --- |
| a) STOP 后延迟旧 START | `basedOnRevision` 落后 → `STALE_REVISION`；且 tombstone 永不回收，key 不会回到 rev0 |
| b) 新 START 后延迟旧 STOP | 同上，对称 |
| c) 旧 policy 覆盖新 policy | 同上，policy 变更同样走 CAS |
| d) reset 升代后前代写入 | `resetGeneration` 不匹配 → `STALE_GENERATION` |
| e) 顺序基准 | **改写**：新旧由 CAS 的 `(generation, revision)` 判定（客户端携带），不再由服务端 epoch 判定 |
| f) 旧命令必须**实际迟到** | **继续有效**，四反例测试照此构造 |
| g) 临界区不得横跨 SQLite | **改写**：CAS 在 socket 锁之外的 authority actor 内执行 |
| h) reset 是同一 sequencer 的 barrier | **改写为**：reset 进入同一 **authority transition barrier**（§4） |
| i) 首个生产调用方与全部反例测试同 PR | **继续有效** |

新增验收项（本轮 Codex 阻断转化）：

- N-a：投影顺序测试——CAS 提交乱序恢复时，manager 与广播仍按 revision 序；
- N-b：幂等三态测试（同 id 同指纹 / 同 id 异指纹 / 新 id）；
- N-c：冲突终止测试——被拒 mutation 不得被自动改号重发；
- N-d：generation 轮换后完整 snapshot 广播 + 客户端丢弃旧 generation；
- N-e：arm 失败不回滚 desired，reconciler 重试后转为 armed；
- N-f：`desired = false` 对过期/不受支持 section 仍成功；
- N-g：公网严格解码器拒绝 desired 命令且不触碰 manager/sink；
  `PUBLIC_RUST_ZERO_SURFACE` 保持 18/18。

## 10. 仍待裁定

- 打包 E2E 的非空路径（写入 → 重启恢复 → reset 删除计数 → 再重启为空）
  需要打包冒烟 run 具备可提交的 desired 写入路径。本地专有 WS 路由落地
  后可从 PowerShell 直连该路由提交一次 CAS，届时该门可关闭；在此之前
  **保持开放**（Codex 本轮已明确）。
