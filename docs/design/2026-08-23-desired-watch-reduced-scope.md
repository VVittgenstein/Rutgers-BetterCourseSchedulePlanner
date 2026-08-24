# 期望监控：缩减范围设计（取代 v7.2 的实时推送模型）

状态：ACTIVE。日期：2026-08-23（v2：2026-08-24，按 M0-M1-001 实现闭环
同步）。
取代 `2026-08-22-desired-watch-revision-cas.md`（该文标记 SUPERSEDED，
保留以说明仍在使用的机制的推导过程）。

**v2 修订摘要**（实现落地后写回，逐条对应用户最终裁定）：

1. §4 删除写入期 Catalog/term/campus 准入与 `REJECTED` / `UNAVAILABLE`
   两个 outcome；CAS 只判 generation、receipt、revision、9 门 post-state
   与两个资源预算；
2. §5 删除"永久失败由服务端自己提交一次 `desired = false`"——**服务端
   永不代用户撤销意图**；
3. §4 的响应合同改写：`REPLAYED` 不是独立状态码，重放答案沿用**原业务
   结果**的状态码；
4. 新增 §4.1 读取投影（GET DTO）与 §4.2 显式响应预算——旧文把
   bootstrap 的 32 KiB 帧预算套到了 authority 读取上，那是两回事；
5. 新增 §5.1 rotation 的 production owner；
6. §8 明确：desired 走 HTTP，**与后续 L2 presence 的 local-only
   WebSocket 是两件事**，不得因为后者存在就把前者搬回 WS。

## 0. 为什么缩

产品所有者 2026-08-22 的裁定里说的是"像 B 站那样"，指的是**多个页面
共同编辑同一份服务端状态**。实现方把它读成了**共同编辑 + 实时推送给
所有页面**，据此建了投影帧流、分块组装、效果批次双向确认、actor 化身
编号、听众登记表与响铃 leader 选举。

2026-08-23 澄清：**B 站的语义是"另一个页面操作后，本页面需要刷新才
看得到"**。不要求实时推送。

这条澄清删掉的正是 v7.2 里最难的部分。**误读是实现方的，不是需求的。**

同轮确认的架构裁定：**浏览器只是展示面，后台程序做真正的解算与拉取；
本地端关掉浏览器即没有响铃通道，因此不响铃是正确行为。**

## 1. 产品模型（不变）

- 所有标签页平权编辑；
- `desired_watches` 是唯一真相；
- 实际监控由**服务端**按已提交的 desired 状态装配，不由某个页面拥有；
- 页面**在加载/刷新时**读到最新状态，不要求推送。

## 2. 保留的机制（已实现，PR5a/5c/5d）

这些机制**与推送无关**，是"两个页面各自提交、不能互相覆盖"本身要求的：

| 机制 | 为什么仍然需要 |
| --- | --- |
| 每 section 的 `revision` + CAS | 两个页面同时改，后到的旧命令不得覆盖新状态 |
| **tombstone** | 删除若直接消失，晚到的 START 会因"读不到行"而被放行，复活已取消的意图 |
| `authorityGeneration` | Full Reset 的屏障：跨代命令一律拒绝 |
| **receipt 账本** | 页面丢了响应后重试，必须重放原答案；否则同一次操作可能先答"否"后答"是" |
| `materializationEpoch` | 区分"改了 policy"与"要重装监控"；policy 编辑不得拆掉正在跑的监控 |
| 准入帽（post-state ≤ 9） | 产品上限；只有 0→1 占额，满员时改 policy 仍可提交 |
| tombstone/receipt 上限 + rotation | 二者只增不减，无上限则数据库无界增长 |

## 3. 删除的机制

| 删除项 | 原因 |
| --- | --- |
| `DESIRED_WATCH_PROJECTION_UPDATE` 帧、FULL/DELTA、分块 C0–C8 | 不推送就没有帧流 |
| 接收状态机、`composite-synced`、ASSEMBLING/DESYNCED | 同上 |
| authority transition actor 的轮次/inbox/化身重建 | 同上 |
| effect batch、history/audience 双 ACK、30s 超时脱钩、拆除独立预算 | 同上 |
| `DESIRED_WATCH_EFFECT` wire wrapper | 同上 |
| `transitionId` | 同上 |
| 响铃 leader 选举 | 见 §6 |
| **专用的第二条 WebSocket 路由** | 改走 HTTP，见 §4 |

`desired_watch_actor_incarnation` 列**保留但当前无写入方**（迁移 10004
已应用，不为删列再发一次迁移）；rotation 仍不得改它。

## 4. 传输：HTTP，与既有个人状态一致

本地个人状态（设置、选课、保存视图）已经全部走 HTTP：
`PUT /api/v1/local/settings`、`PUT /api/v1/local/selection` 等。
期望监控照抄同一模式：

```
GET /api/v1/local/desired-watch
  → 200 { protocolVersion, data: DesiredWatchStateV1 }

PUT /api/v1/local/desired-watch
  body: { protocolVersion, payload: {
            contractVersion, section, policy|null,
            basedOnRevision, authorityGeneration, mutationId } }
  → { protocolVersion, data: {
        contractVersion, outcome, replayed, authorityGeneration,
        currentRevision|null, maximum|null, committed|null, state|null } }
```

`policy` 本身就是"要不要监控"：`Some` 表示要看，`null` 表示停止。**不再
另发一个 `desired` 布尔**——两个字段可以互相矛盾，而矛盾时页面会相信
其中一个。

状态码由**业务结果**决定：

| outcome | 状态 | 终局 | 说明 |
| --- | --- | --- | --- |
| `COMMITTED` | 200 | — | `state` 携带提交后的完整权威读取 |
| `STALE_GENERATION` | 409 | 是 | body 带当前 generation |
| `STALE_REVISION` | 409 | 是 | body 带该 section 当前 revision |
| `MUTATION_ID_CONFLICT` | 409 | 是 | 同 id 承载了不同命令 |
| `LIMIT_EXCEEDED` | 409 | 是 | body 带 `maximum = 9` |
| `AUTHORITY_FULL` | 503 | **否** | 未写入、未记 receipt，同 id 可重试 |

**`replayed` 是一个布尔字段，不是一种 outcome。** 重放的答案沿用**被
重放的原业务结果**的状态码：一次被拒的提交在重放时仍然是 409。若因为
外层写着"REPLAYED"就一律回 200，丢了响应的页面会以为被拒的命令成功了
——而这正是 receipt 账本存在的理由。

协议层失败（body 无法解析、缺少 Origin/session）沿用本地 API 既有的
`400` / `403` 类型化错误信封；它们不是关于用户意图的答案。

**不新增 WebSocket 路由。** 告警仍走既有的 `/api/v1/watch`；对 desired
路径发起 upgrade 只会命中 HTTP 处理器，永远拿不到 101。

### 4.1 读取投影（`DesiredWatchStateV1`）

```
{ contractVersion, authorityGeneration, entries: [ {
    section, policy|null, revision, materializationEpoch,
    materialized: { authorityGeneration, revision,
                    materializationEpoch, policy, activeWatchId } | null,
    pendingDisarm, blockedOnSlot,
    failure: { classification: TRANSIENT|PERMANENT,
               reason, retryScheduled } | null
  } ] }
```

`entries` **含 tombstone**：`policy = null` 就是 tombstone，它持有该
section 被删除时的 revision。

**`armed` 不上 wire**（§7）。前端按 §7 的四元组自行判定，服务端内部同式
判定；wire 上没有任何一个布尔能与这些字段矛盾。

### 4.2 响应预算（取代旧文的 32 KiB 说法）

旧文写"受 bootstrap 的 32 KiB 预算约束"是**错的**：32 KiB 是 v7.2 的
**单个投影帧**上限，与这个 HTTP 读取无关。

authority 读取使用**独立的显式预算 256 KiB**（沿用 §2 已冻结的逻辑状态
上界），并以**最大合法状态**（9 desired + 512 tombstone = 521 行）序列化
测试证明不超限。预算是用来**证明**的，不是用来截断的：

- 不分页——被截断的读取与完整读取在前端看来完全一样，缺失的 tombstone
  会被当成"从未存在的行"，下一条针对它的命令就会带着
  `basedOnRevision = 0` 被放行，复活用户已取消的意图；
- 不丢 tombstone——同上；
- 证明失败即**构建失败**，不是请求失败。

## 5. 装配：连上时按已提交状态装监控

- 页面建立 watch WebSocket 连接时，服务端读 `desired = 1` 的行，
  为每一行装配一份监控（服务端持有，不绑定到发起的那个页面）；
- 最后一个连接断开 → 拆除全部物理监控，**保留 desired 状态**；
- `PUT` 提交成功后，服务端**先 reconcile 再回响应**——因此提交页面拿到
  的 `state` 描述的是"现在实际在跑什么"，不是"刚才请求了什么"；
- **不广播**，其他页面下次读取时看到。

物化记录的四元组 `(generation, revision, epoch, policy)` 与 authority
一致时才算 armed。policy-only 编辑走**就地更新**、rotation 走**收养**，
都不重启健康的 watch：重启会结束 episode 并对一个用户已经被告知过的
开放课节再响一次，而用户只是改了响多大声。

失败分类沿用 v7.2 §5.5 的**暂态/永久划分**，但处置改写：

| 分类 | 例子 | 处置 |
| --- | --- | --- |
| 暂态 | 快照未就绪、gate 未放行、物理槽被占、连接关闭中 | 有界退避重试（5/10/20 封顶 30s），desired 不动 |
| 永久 | 非产品校区、学期超窗、目录不发布该 section | **停止重试**，desired **保留**，在读取投影中暴露原因 |

**服务端永不代用户撤销意图。** v7.2 的"永久失败由服务端自己提交一次
`desired = false`"已被用户最终裁定删除：该 section 下学期可能重新发布，
而一条自己消失的行会让用户看到更短的列表且没有任何解释。运行时报告
它做不到什么，STOP 仍然是用户的决定。

每次重试都带**发起时的 (generation, revision, epoch) 戳**，并在触发时
重新读取 authority：戳不再匹配的重试直接丢弃而不是套用。否则一条在
"用户已 STOP"之后触发的重试会复活该 section，一条在"用户改了 policy"
之后触发的重试会把新意图压在旧退避后面。

### 5.1 rotation 的 production owner

rotation 由 **coordinator 在提交路径上**触发（预算达 80% 阈值时），
不由存储层自作主张，也不由"谁先发现谁来做"。理由：rotation 抬 generation
并重编号所有存活行，每个页面都必须重读；把这个决定留给随机的调用方会让
一次阈值穿越产生多次 rotation。

只有写入会让预算增长，所以在提交路径上检查就够了；维护 tick 不做
rotation。

## 6. 两个页面同时开着时的响铃

两个页面都连着 watch WebSocket 时，告警**扇出到全部连接**，因此两个
页面都会响。这是诚实的行为（都在监控、都响），不是错位。

v7.2 的 leader 选举是为了"只响一次"，属于体验优化，**不在本范围内**。
若将来觉得吵，再单独做。

## 7. UI 投影（简化）

```
1. pendingDisarm      → desired=0 ? 「停止中」 : 「准备中」   非绿
2. blockedOnSlot      → 「准备中（等待旧槽释放）」            非绿
3. desired = 0        → 「未监控」                            —
4. desired = 1 ∧ armed→ 「监控中」                            **绿**
5. desired = 1 ∧ ¬armed→「准备中」                            非绿
```

v7.2 的第 0 条（`¬composite-synced`）随帧流一并删除。HTTP 读取要么成功
要么失败，没有"部分同步"这个中间态；读取失败时页面显示读取失败，不得
拿旧数据冒充当前状态。

`armed` 仍是派生量，**不上 wire**：

```
armed ≡ materialized ≠ null
  ∧ materialized.generation = authorityGeneration
  ∧ materialized.revision   = sectionRevision
  ∧ materialized.epoch      = materializationEpoch
  ∧ materialized.policy     = desiredPolicy
```

## 8. 公网边界（强化）

desired 的类型与路由**只存在于本地专有 crate**；`bcsp-contracts` 与
`bcsp-application` 都在公网闭包内，不得出现 desired 相关类型。公网严格
解码器把未知命令拒绝：不进入命令路由、不改 watch 状态、不产生 dispatch。

改走 HTTP 后，本地路由注册在 `LocalRouteExtension` 内，与 settings /
selection 同一处，**不触及共享 host 的路由表**——公网边界比 v7.2 的
第二条 WebSocket 方案更容易证明。

**v2 新增：该边界现在是被强制的，不只是被遵守的。** 公网 SOURCE 负控
新增三个 marker——`desired_watch`、`authority_generation`、
`materialization_epoch`——PERSISTENT_ACTIVE_WATCH 由 13 项增至 16 项，
全局集 212 → 215，`markerSetVersion` 1 → 2，两个摘要重算。Rust 四表面
与前端 source/bundle 均覆盖；**前端 shared 属于公网闭包**，所以停在
shared 里的辅助代码泄漏得和写在 public 里一模一样，前端负控对此有专门
反例。

因此共享前端里的端口用的是**目标中立词汇**（"standing watch intent"），
不是本地存储的词汇：把某一个 target 的能力名字写进另一个 target 也会
编译的模块，本身就是泄漏。

**desired HTTP 与 L2 presence 是两件事。** L2 的页面存活探测仍然会用一条
local-only WebSocket（`/api/v1/local/presence`，共享 host 的受校验二级
路由集合），那条路由的存在**不构成**把 desired 搬回 WebSocket 的理由：
desired 是"读了才知道"的权威状态，presence 是"断了才知道"的连接事实。

## 9. 验收

1. CAS 五步顺序、receipt 幂等、tombstone 反例、准入帽 post-state、
   epoch 保留规则 —— **已实现并钉死**（PR5a/5c/5d）。
2. 两个上限、80% 阈值 409/1638、fail-closed、原子 rotation ——
   **已实现并钉死**（PR5d）。
3. HTTP 路由：每种 outcome 的状态码与响应体；同 id 重试重放；
   跨代拒绝。
4. 装配：连上时按 desired 装配；最后一个连接断开时拆除且保留 desired；
   提交后立即生效；每 section 至多一份物理 watch，告警扇出到全部页面。
5. 前端：读取、显示五态、提交、失败可见；只有四元组全等才显示"监控中"；
   读取失败不保留旧绿态；409 后重读且不自动重放手势。
6. 打包端到端：写入非空意图 → 重启 → 恢复且能装配 → reset 返回删除
   计数 → 再重启仍空。**已在 `packaging/windows/verify.ps1` 落地**，
   同型断言另有 `crates/bcsp-local-runtime/tests/local_runtime.rs` 的
   三生命周期 HTTP 版本，随 `cargo test --workspace` 常跑。
7. 公网零表面：三个 marker 的 Rust 四表面 + 前端 source/bundle 负控。

### 9.1 迁移 10004 与发布约束

- 10004 重建 `personal_desired_watches_v1` 并新建
  `personal_desired_watch_receipts_v1`；一旦用户数据库被升级，**旧二进制
  会因未知迁移拒绝启动**，即该升级**不可回滚**；
- 因此不得发布"只有迁移和地基、没有 writer/UI/E2E"的中间构建：本设计
  §9 的第 3–6 项全部闭合后才可发布；
- receipt 的 `outcome_json` 只有三种形状（`COMMITTED`、`STALE_REVISION`、
  `LIMIT_EXCEEDED`）。v7.2 曾有的 `REJECTED` 形状随写入期准入一并删除；
  由于 CAS 写入器在本轮之前**从未有过生产调用方**，任何已发布构建都
  不可能写出过 `REJECTED` 行，删除它不会孤立用户磁盘上的任何记录。

## 10. 与 S2-D3 的关系

S2-D3 要求"首个生产调用方必须与全部反例测试同一 PR 落地"。本范围下，
反例测试已随 PR5a/5c/5d 落地并通过复审，因此接线 PR 需携带的是 §9 的
第 3–6 项。**S2-D3 在 §9 全部通过后关闭。**
