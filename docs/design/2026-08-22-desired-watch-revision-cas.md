# 期望监控：revision/CAS 共同编辑模型（S2-D3 路线改判）

状态：ACTIVE（产品所有者 2026-08-22 裁定）。取代 S2-D3 原先选定的
fenced sequencer 路线，改走清单里二选一的另一条：**持久化 revision +
tombstone + CAS**。

## 0. 为什么改判（证据）

S2-PR5 曾按 fenced sequencer 实现：`(generation, epoch, sequence)` 全序，
`epoch` 在连接注册时分配。三镜头对抗验证以**实跑复现**打回两项 P1：

1. `epoch` 压过 `sequence` 且 `last_by_section` 只在 reset 时清除，于是
   **只要某 section 被较新连接写过，任何较旧但仍存活的连接对它的写入就被
   永久静默丢弃**。复现：B 标签页（epoch 2）给 S 上过 watch 后关闭，仅存
   的 A 标签页（epoch 1）里用户点 START —— manager 正常准入
   （`connection_watches == [S]`，会真的响铃），意图却没落库。**活跃
   watch 与持久意图反向错位**，直接违反"人认为是 active 的时候就一定要
   active"。
2. 反向去掉 epoch（纯到达序）后，反例 (a)"STOP 后延迟旧 START"必然失败。

根因：服务端在**两条同时存活**的连接之间，无法区分"迟到的旧帧"与"用户
刚在那个标签页做的新操作"——两者在字节流上同形。注册序只是 leader 的
错误代理（Web Locks 的 leader 可以回落到更早的连接）。

**结论**：判定信息必须由客户端携带。`basedOnRevision` 正是它：用户的新
点击一定基于页面**已经看到**的最新 revision，迟到的旧帧一定基于更旧的
revision。fenced sequencer 适合单写者；本产品要的是多页面平权编辑。

## 1. 产品模型（裁定）

- **所有标签页平权编辑**，不存在"只有 leader 能改"的可见规则；
- **`desired_watches` 是唯一真相**，页面显示的是它的投影；
- **实际监控由服务端按已提交的 desired 状态统一维护**，不再由某个页面
  "拥有"一个 watch；
- **leader 只剩一件用户看不见的事**：避免多个页面同时响铃。它不限制编辑。

```
页面 A ─┐
        ├─→ 服务端 desired_watches（CAS 提交）→ 实际监控 → 广播回所有页面
页面 B ─┘
```

## 2. 存储（迁移 10004）

`personal_desired_watches_v1` 由"只存活跃意图"改为"**带 revision 的
当前真相 + tombstone**"：

| 列 | 说明 |
| --- | --- |
| `term_id` / `campus_code` / `section_index` | 主键，不变 |
| `desired` | `INTEGER NOT NULL CHECK (desired IN (0,1))`；`0` 即 tombstone |
| `policy_json` | `TEXT NULL`，`CHECK (desired = 0) = (policy_json IS NULL)` |
| `revision` | `INTEGER NOT NULL`，该 section 最后一次变更的修订号 |
| `last_mutation_id` | `TEXT NULL`，幂等键（重试去重） |

- **tombstone 是必需的**：删除若直接消失，迟到的旧 START 就没有可比较的
  revision，会复活已删除的意图（用户裁定第 4 点）。
- **revision 分配**：`personal_state_metadata_v1` 持久化一个
  `desired_watch_revision` 单调计数器，在同一 `IMMEDIATE` 事务里 +1。
  无行的 section 视为 `revision = 0`。
- **9 门帽**只统计 `desired = 1` 的行；tombstone 不占额。
- **reset**：完整重置**删除**全部行（tombstone 也是个人数据，必须真删），
  因此单靠 revision 无法防住跨 reset 的迟到命令 —— 由
  `reset_generation` 承担（见 §3）。reset 时 generation +1、revision
  计数器归零。

### 待决（需裁定）
- **tombstone 增长边界**：`desired = 1` 有 9 帽，tombstone 只受"用户历史
  上监控过多少 section"约束。建议：总行数上限（例如 256），超限时按
  `revision` 升序淘汰最旧的 tombstone（永不淘汰 `desired = 1`）。淘汰后
  该 section 回落到 `revision = 0`，等价于"从未编辑过"——迟到旧帧因
  `basedOnRevision != 0` 仍被拒。

## 3. 命令与事件（契约）

新增客户端命令（严格解码）：

```
SET_DESIRED_WATCH {
  section:           SectionKey,
  desired:           bool,
  policy:            WatchPolicyV1 | null,   // desired=true 时必填
  basedOnRevision:   u64,
  resetGeneration:   u64,
  mutationId:        TraceId,
}
```

新增服务端事件（增量）：

```
DESIRED_WATCH_COMMITTED {                    // 广播给所有连接
  section, desired, policy, revision, resetGeneration
}
DESIRED_WATCH_REJECTED {                     // 只回提交者
  section, reason: STALE_REVISION | STALE_GENERATION | LIMIT_EXCEEDED
        | UNSUPPORTED_TARGET | TERM_OUT_OF_RANGE | SECTION_NOT_FOUND,
  currentRevision, currentDesired, currentPolicy, currentResetGeneration
}
```

拒绝事件**必须回带当前真相**，这样页面无需再查一次就能重放用户意图
（`basedOnRevision` 换成 `currentRevision` 后重发）。

**CAS 规则**（单个 `IMMEDIATE` 事务内）：
1. `resetGeneration != current` → `STALE_GENERATION`；
2. `mutationId == last_mutation_id` → 幂等成功，回当前状态（重试安全）；
3. `basedOnRevision != current_revision_of_section` → `STALE_REVISION`
   （无行时 current 为 0）；
4. `desired = true` 且该 section 尚非 desired 且已有 9 行 desired →
   `LIMIT_EXCEEDED`；
5. 否则写入、`revision = ++counter`、提交、广播。

## 4. 执行路径（本仓库接缝）

CAS **必须**返回结果给提交页面，所以它不能是 fire-and-forget 的 sink
投递；同时它是 SQLite 事务，**不得**在 `SharedWatchSocket` 的互斥锁内执行。

```
receive_text
  └─ route_command: SET_DESIRED_WATCH
       └─ 释放 socket 锁 → DesiredWatchAuthority::commit(...)   [阻塞工作线程]
            ├─ 本地：SQLite CAS 事务（bcsp-local-user-state）
            └─ 公网：一律拒绝（公网无个人存储，见 §6）
       └─ 重取锁 → 依据提交结果同步 WatchManager（起/停监控）
                 → 广播 DESIRED_WATCH_COMMITTED 给全部连接
```

`DesiredWatchAuthority` 是新的 trait 接缝，与既有 `WatchAdmissionSource` /
`WatchDispatchSink` 同层。既有的 `LocalWatchHistorySink` 单 worker 线程
继续只负责 episode 历史（fire-and-forget 语义对历史是对的）。

### 待决（需裁定）
- **START_WATCH / STOP_WATCH / UPDATE_POLICY 的去留**：它们目前既是
  "用户意图"又是"取得 activeWatchId"的路径。建议：用户面路径全部改走
  `SET_DESIRED_WATCH`，三条旧命令保留为 manager 内部效果的载体
  （`START_RESULT` 等事件不变），或在 S3 前废弃。需裁定。
- **零连接时是否继续监控**：现有不变量
  `ACTIVE_WATCH_STATE_PERSISTENT = false`（重启不复活活跃 watch）。建议
  保持：desired 状态是"应当监控什么"的权威，manager 在**至少有一个连接**
  时据此物化 watch；零连接时无人可提醒，监控暂停。这与"服务端统一维护"
  不冲突（不再由某个页面拥有），但需明确裁定。

## 5. 前端（PR6/PR7）

- 每个标签页持有它见过的 `revision` / `resetGeneration`，发命令时带上；
- 收到 `DESIRED_WATCH_COMMITTED` 即更新本地投影（无论是不是自己发的）；
- 收到 `DESIRED_WATCH_REJECTED` 时：若是 `STALE_REVISION`，用回带的当前
  真相更新投影，再决定是否以新 revision 重放用户意图（用户仍想要的话）；
- bootstrap 的 `desiredWatches` 增加 `revision` / `resetGeneration`；
- leader（Web Locks）**只**决定谁播放声音，不参与编辑。

## 6. 边界

- 期望监控是**本地专有**概念：公网 target 无个人存储，
  `SET_DESIRED_WATCH` 在公网一律拒绝，`PUBLIC_RUST_ZERO_SURFACE` 校验器
  继续为权威约束；
- 一条命令一个 section：批量原子性不是需求，逐条 CAS 更简单且失败面更小；
- 重放的 START（manager 的 per-connection 重放缓存）不得再次提交意图——
  在新模型下由 `mutationId` 幂等天然覆盖。

## 7. 与清单的关系

本文件关闭 S2-D3 的路线选择，并**取代**其中 e–i 五条执行语义（那五条是
针对 fenced sequencer 写的）。新的验收反例见 §3 的 CAS 规则，四个原始
反例在本模型下的形态：

| 原反例 | 本模型下的表现 |
| --- | --- |
| a) STOP 后延迟旧 START | 旧 START 的 `basedOnRevision` 落后于 STOP 提交后的 revision → `STALE_REVISION` 拒绝 |
| b) 新 START 后延迟旧 STOP | 同上，对称 |
| c) 旧 policy 覆盖新 policy | 同上，policy 变更也走 CAS |
| d) reset 升代后前代写入 | `resetGeneration` 不匹配 → `STALE_GENERATION` 拒绝 |
