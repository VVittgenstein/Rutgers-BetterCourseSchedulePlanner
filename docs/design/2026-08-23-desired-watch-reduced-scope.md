# 期望监控：缩减范围设计（取代 v7.2 的实时推送模型）

状态：ACTIVE。日期：2026-08-23。
取代 `2026-08-22-desired-watch-revision-cas.md`（该文标记 SUPERSEDED，
保留以说明仍在使用的机制的推导过程）。

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
  → { protocolVersion, data: { authorityGeneration, entries: [...] } }

PUT /api/v1/local/desired-watch
  body: { protocolVersion, payload: {
            section, desired, policy|null,
            basedOnRevision, authorityGeneration, mutationId } }
  → 200 { outcome: COMMITTED | REPLAYED, revision, materializationEpoch }
  → 409 { outcome: STALE_GENERATION | STALE_REVISION | MUTATION_ID_CONFLICT
                 | LIMIT_EXCEEDED | REJECTED, ... }
  → 503 { outcome: UNAVAILABLE | AUTHORITY_FULL }   // 非终局，同 id 可重试
```

`entries` 含 tombstone（前端据 revision 发下一条命令），受 bootstrap 的
32 KiB 预算约束；最大合法状态已由 §2 的上限定为 **9 + 512 = 521 行**。

**不新增 WebSocket 路由。** 告警仍走既有的 `/api/v1/watch`。

## 5. 装配：连上时按已提交状态装监控

- 页面建立 watch WebSocket 连接时，服务端读 `desired = 1` 的行，
  为每一行装配一份监控（服务端持有，不绑定到发起的那个页面）；
- 最后一个连接断开 → 拆除全部物理监控，**保留 desired 状态**；
- `PUT` 提交成功后，服务端立即按新状态装配或拆除——**不广播**，其他
  页面下次读取时看到。

失败分类沿用 v7.2 §5.5：暂态退避重试、永久由服务端自己提交一次
`desired = false`（不记 receipt）、gate 未放行按暂态处理。

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

## 8. 公网边界（不变）

desired 的类型与路由**只存在于本地专有 crate**；`bcsp-contracts` 与
`bcsp-application` 都在公网闭包内，不得出现 desired 相关类型。公网严格
解码器把未知命令拒绝：不进入命令路由、不改 watch 状态、不产生 dispatch。

改走 HTTP 后，本地路由注册在 `LocalRouteExtension` 内，与 settings /
selection 同一处，**不触及共享 host 的路由表**——公网边界比 v7.2 的
第二条 WebSocket 方案更容易证明。

## 9. 验收

1. CAS 五步顺序、receipt 幂等、tombstone 反例、准入帽 post-state、
   epoch 保留规则 —— **已实现并钉死**（PR5a/5c/5d）。
2. 两个上限、80% 阈值 409/1638、fail-closed、原子 rotation ——
   **已实现并钉死**（PR5d）。
3. HTTP 路由：每种 outcome 的状态码与响应体；同 id 重试重放；
   跨代拒绝。
4. 装配：连上时按 desired 装配；最后一个连接断开时拆除且保留 desired；
   提交后立即生效。
5. 前端：读取、显示五态、提交、失败可见。
6. 打包端到端：写入非空意图 → 重启 → 恢复且能装配 → reset 返回删除
   计数 → 再重启仍空。

## 10. 与 S2-D3 的关系

S2-D3 要求"首个生产调用方必须与全部反例测试同一 PR 落地"。本范围下，
反例测试已随 PR5a/5c/5d 落地并通过复审，因此接线 PR 需携带的是 §9 的
第 3–6 项。**S2-D3 在 §9 全部通过后关闭。**
