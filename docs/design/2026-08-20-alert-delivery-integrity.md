# 设计方案：提醒必达（Alert Delivery Integrity）

状态：v3.1 —— **已批准**（Codex 终审 2026-08-20）。进入实现阶段；遗留
细节按 `2026-08-20-pr-acceptance-checklist.md` 在 PR 中验收。
日期：2026-08-20
作者：Claude；评审：Codex；关联：安全门 v3、公网加固 v2、两份评审总案 v2

## 0. 总纲（产品负责人定义）

**页面即服务；页面显示的状态必须是被验证过的事实。** 可以坏，不许骗。
"会响铃"= 五环链全部为真：
① WS 连接活着 → ② 服务端确实 armed → ③ 音频可用 → ④ 页面能发声(或有
兜底) → ⑤ 未静音。

**诚实边界（v2 明确）**：被浏览器冻结（frozen）或丢弃（discarded）的页面
不执行任何 JS——WS 处理器不跑、通知发不出。此场景为 **residual risk**，
本设计不承诺"必达"；承诺范围是 hidden-but-running 及以上。

## 1. 已核实缺陷（file:line 依据同 v1.3，摘要）

D1 断线永不重连（测试锁定的原契约，本案有意变更）；D2 重连即重新 START
（**产品决策：接受，不作为缺陷**）；D3 音频挂起零检测、报警循环静默自杀；
D4 无兜底通知通道；D5 公网 nonce 2h 过期缝隙；D6 部署/重启杀光 watch；
D7 Caddy reload 切断长连接。

## 2. 设计

### 第一层：Readiness 真值链

- 三态：READY（绿）/ DEGRADED（黄，写明断环 + 一键修复）/ STOPPED（灰）；
  **常驻**于所有 SPA 路由（不随路由卸载），DEGRADED 不可被通知挤掉。
- **各环判定语义（v2 补全，附完整真值表于 §5 测试 6）**：
  - ①：WS readyState + **应用层心跳**——页面看不到传输层 ping/pong，
    新增轻量 app-level heartbeat（复用现有 WS 消息通道，10s 一次
    PING/ACK）；READY 定义为 **last-known-good ≤ 25s**（2×心跳+余量）的
    有界陈旧声明，超界降黄；
  - ②：START_RESULT 回执 + 心跳 ACK 隐含服务端存活；
  - ③：AudioContext.state === 'running'（statechange 监听实时更新）；
  - ④：`document.visibilityState` × Notification 权限三态（granted /
    denied / default）——hidden 且无通知权限 → 黄；
  - ⑤：音量/静音设置。

### 第二层：自愈

**2a. 自动重连**
- 触发：**仅意外 close/error**。用户显式 Disconnect 置 `userStopped` 标记，
  **不自动重连**（v2 修正：现有"without automatic restart"测试翻转时必须
  保留用户主动断开分支，Disconnect 按钮不得被自动连回）。
- 节奏：1s/2s/4s…封顶 30s，无限重试；期间 Readiness=DEGRADED("重连中")。
- **re-arm 数据源（v2 修正；v4 由 CAS 设计改写归属）**：不是 selection。
  connection 无关的**期望监控表** `desiredWatches`——用户 START 即写入、
  显式 STOP 即移除；不持久化 activeWatchId/响铃消耗。
  **v4 起该表是服务端权威状态**：所有 tab 经 revision/CAS 平权编辑，
  **物化由服务端逻辑 owner 负责**，**页面不再"加载后按表自动 START"**。
  见 `2026-08-22-desired-watch-revision-cas.md`。
- **desired→armed 调和循环（v3 新增；v4 起由服务端 authority 承担，
  不再是客户端循环）**：
  连接健康 ≠ 全部武装成功——如 Open 快照未就绪时 START 返回
  `TARGET_UNAVAILABLE`，socket 无事发生、不会再触发 re-arm。改为常驻
  调和循环：`desired − armed` 的差集按退避（5s/10s/20s 封顶 30s）重试；
  每轮携带 retry-epoch，STOP/policy 变更/重连即令旧 epoch 作废（取消
  在途重试）；**暂态拒绝**（TARGET_UNAVAILABLE 等）重试，**永久拒绝**
  （admission 类：非产品校区/超学期窗口/超 9 门上限）从 desired 移除并
  以常驻通知告知用户，不无限重试。
- **本地多标签页所有权（v3 提出，v4 由 CAS 设计取代）**：desired 表
  持久化后，若每个 tab 都自动 START，会产生重复 watch、重复响铃与跨 tab
  STOP 不一致。v3 曾采用 **leader tab 拥有监控**的方案（Web Locks 选举，
  仅 leader 建连接、跑调和循环、发声）。
  **该方案已被 `2026-08-22-desired-watch-revision-cas.md` 取代**：
  监控由**服务端 connection-independent 的逻辑 owner 持有**，
  **所有标签页平权编辑** desired 表（revision/CAS），
  **leader 只剩一件用户看不见的事：避免多个页面同时响铃**。
  因此不再有"仅 leader 武装""leader 转移后 re-arm""BroadcastChannel
  镜像/转发编辑"这三件事。公网版无持久 desired，单 tab 即单连接，
  维持现状。
- 重连后对仍开放课节再响一轮 = 产品决策接受（v1.1，维持）。

**2b. 公网会话票（v2 重写：403 分支不可实现）**
- 事实：WHATWG 规范下，浏览器脚本对 WS 握手失败一律只见不可区分的
  close——**无法识别 HTTP 403**，"被拒走换证分支"写法作废。
- 服务端：新增 **`POST /api/v1/session/validate`**（公网专属）。
  **请求合同冻结（v3，评审要求——失效换新实质是匿名签发面）**：
  - 请求体 `{ nonce: string, locale?: string }`（locale 来源与首页签发
    一致：请求体优先、缺省回退 Accept-Language），body 上限沿用 1 MB
    全局帽下的 4 KB 路由帽；
  - Host/Origin 校验与其他公网 POST 完全一致（authority 等值 +
    `BCSP_PUBLIC_ORIGIN`）；
  - 状态合同：`200 {valid:true}` / `200 {renewed:nonce}` /
    `400`（体非法）/ `403`（**Origin** 不符）/ `421`（**Host/authority**
    不符）/ `429 + Retry-After`（仅限流）/ `503`（registry 容量全活跃，
    与 H5 统一：容量耗尽一律 503，429 专用于限流）；
  - **per-IP 限流与首页签发同一策略同一桶**（否则绕过首页限流）；
  - validate/touch 与 renew/evict（签新+废旧）各自在 registry 锁内
    **一次原子完成**。
- **WS 握手原子租约（v3 新增）**：握手路径改为 `reserve_ws(nonce)` 一次
  锁内完成：有效性检查 + per-session 连接帽 + `active_ws_count += 1` +
  activity touch，返回 **RAII lease**（drop 时 `-1` 并 touch）——消除
  "校验通过后、upgrade 完成前 nonce 被淘汰"的 TOCTOU 窗口。
- 客户端：nonce 从 bootstrap 闭包常量改为 **mutable NonceHolder**；重连
  流程 = 每次连接尝试**前**先 single-flight 调 validate（带退避），拿
  有效 nonce 再握手。连接失败不猜原因，统一走"validate → 重试"环。

**2c. 音频自愈**：statechange 监听 → suspended 自动 resume()；需手势 →
DEGRADED + 一键恢复 + 兜底通知；visibilitychange/resume/大时钟跳变触发
五环全量复检。

### 第二层附加：本地版生命周期契约

**L1. 刷新后完整恢复**（产品负责人定义并确认）
- 持久化对象 = **期望监控表**（section+policy，2a 同一结构落盘），不是
  "selection+活跃布尔"，不含 activeWatchId/响铃消耗；页面加载后按表自动
  START。修订 `bcsp-local-user-state` lib.rs 的"不表示 active watch"声明。

**L2. 关闭浏览器 = 60 秒可见倒计时后退出**
- **presence 通道（v2 新增，关键修正）**：watch WS 只在点击 Start 后才
  存在——"只浏览未监控"的页面无任何连接，仅数 watch 连接会在 60s 后
  **误杀正在使用的运行时**。新增本地专属**每 tab 一条的 presence 连接**
  （页面加载即建立），覆盖：空闲浏览页、多 tab、刷新（1–2s 回归取消
  倒计时）、浏览器重启（60s 内回归取消）、异常断连。
- **host seam（v3 修正，评审指出仅改 lifecycle.rs 不可行）**：共享 host
  目前硬编码唯一 WS 路由 `/api/v1/watch`（`bcsp-application/src/host.rs:366`），
  普通 RouteExtension 无法表达流式连接。方案：在共享 host 增加
  **第二个可选 WS 路由注册 seam**（target 注入，公网不注入即不存在），
  本地经此注册 `/api/v1/local/presence`。`bcsp-application/src/host.rs`
  正式列入触点（见 §4）。
- **倒计时状态机（v3 补）**：`{ count, generation, phase:
  Running | CountingDown | ShuttingDown }` 原子持有；count 归零 →
  `CountingDown(generation+1)`；期内回归 → 回 `Running`（generation 再
  +1，作废旧倒计时）；**到期时重新验证 generation 与 count 仍然为零**
  才进入 `ShuttingDown`——杜绝"回归与到期竞态"下的误杀。控制台（L3）
  每秒输出「页面已全部关闭，N 秒后退出；页面回归即取消」。

**L3. 控制台日志**：启动/presence 连接断开/监控 START-STOP/开放警报/
安全门事件/倒计时/退出；高频轮询 debug 级；语言随 locale。

### 第三层：桌面通知兜底

- **政策修订（产品负责人 2026-08-20 批准，回应评审方反对意见）**：
  现行 `public-source-deny.json` SYSTEM_NOTIFICATIONS 家族明含
  `browser_notification_api`/`desktop_notification`/`notification_permission`
  且前端 import 闭包扫描器（verify-import-graph.mjs）与构建产物扫描
  （verify-target-build.mjs）均会命中——**不做解释性绕行，正式修订政策**：
  - 拆分家族：新设窄允许项"当前页面级通知"（页面运行中经浏览器授权发出
    的 OS 通知），**继续禁止** 服务端推送 / native / service-worker
    notification / web push（相关 marker 保留）；
  - **完整同步面（v3 按评审清单补全，逐项列举）**：
    1. 冻结 exact capability slug：`current-page-notification`；
    2. 冻结 exact marker 分区——**表面作用域模型**（v3.1 窄修：评审方用
       现行扫描器实证"全局移除"会放行 Rust 侧 `desktop_notification`，
       违背继续禁止 server/native 的边界）：
       - 三个页面级 marker（`browser_notification_api` /
         `notification_permission` / `desktop_notification`）从**共享
         全局集**移出：全局集 212 → **209**（最终数冻结为 209；v3 所写
         "保留 push_notification"有误——该 marker 不存在于现行集合，
         SYSTEM_NOTIFICATIONS 实际剩余为 `native_alert` /
         `os_notification` / `service_worker_notification` /
         `show_notification` / `system_notification` / `tray_balloon`，
         web push 由独立 WEB_PUSH 族继续覆盖，不新增 marker）；
       - 新增 **Rust 表面补充负控**：这三个 marker 在 Rust 的
         SOURCE/API/STORAGE/PACKAGE 四表面**继续拒绝**（补充清单随
         verifier 固化，`desktop_notification` 进 Rust 负控测试）；
       - 前端 source/bundle **仅当** manifest 声明
         `current-page-notification` slug 时豁免这三个 marker；未声明的
         构建照旧拒绝；
       - `markerSetVersion` 递增；双摘要以"全局 209 + Rust 补充清单"为
         唯一输入。
    3. **两种摘要分别更新**：Rust 侧 whole-document semantic SHA
       （verify-rust-graph.mjs:20-21 所 pin）与前端侧 canonical rows SHA
       ——两套算法两处基线；
    4. `tools/architecture/verify-public-rust-zero-surface.mjs` 及其
       自身测试同步（marker 计数常量）；
    5. `packaging/verify-release-set.ps1` 中固定的 shared capability
       清单与计数同步（release gate）；
    6. **CI 执行入口**：新 verifier/测试实际挂进 CI workflow（不是只有
       文件存在）；
    7. **正例测试**：页面级 `Notification.permission` / `new Notification`
       在公网构建的 source / bundle / manifest / release 四层全部放行；
    8. **反例测试**：server / native / service-worker `showNotification` /
       web-push 在同样四层全部仍被拒绝。
    预估修正为 +1.5 天。
- 权限：随首次"开始监听"手势请求；应用内设置可关。
- 触发（v2 补时序洞）：
  a) `ALERT_UPDATED(Opened)` 且（页面 hidden 或 audio 非 running）；
  b) **cue 失败分支**：Alert 先于 AudioDisposition 到达——页面 visible
     且 audio 当时 READY 则 a) 不触发；随后播放返回
     `AUTOPLAY_BLOCKED/FAILED` 时**补发**通知；
  c) 重连持续失败 >2min 发"监控已降级"；
  - a/b 按 alert/episode id 去重，至多一条。
- 边界：frozen/discarded 页面无法触发（§0 residual risk）。

## 3. 产品边界

- 通知政策按 §2 第三层正式修订，不再依赖边界解释；
- **（v4 更正）** 本设计**新增一处本地服务端持久个人状态**：期望监控表
  `personal_desired_watches_v1` 及其 receipt ledger——**local-only**，
  公网 target 结构上不存在（`PUBLIC_RUST_ZERO_SURFACE` 强制）。
  除此之外无新增：validate 端点无状态（nonce registry 本就存在）；
  presence 与 desired 通道均为 local-only 路由（架构围栏强制）。

## 4. 触点清单

| 层 | 位置 | 改动 |
|---|---|---|
| 前端连接 | `WatchClient.ts:80-119` | 重连状态机（意外断限定/退避/single-flight validate） |
| 前端状态 | `LiveWatchProvider.tsx` | desiredWatches 表；Readiness 派生；userStopped |
| 前端心跳 | WS 消息层 | app-level PING/ACK（10s）+ 25s 陈旧上界 |
| 前端音频 | `audio.ts` | statechange + 自动 resume |
| 前端通知 | 新 `watch/notification.ts` | 权限/发送/去重/设置；cue 失败补发 |
| 公网服务端 | `bcsp-public-runtime` | WS 活动续期 nonce；`POST /api/v1/session/validate`（签新废旧） |
| 公网客户端 | bootstrap/NonceHolder | nonce 可变持有 + 换证环 |
| 本地状态(L1) | `bcsp-local-user-state` | 持久化期望监控表（revision/CAS + tombstone + receipt）；**物化由服务端 owner 负责，非"加载自动 START"** |
| **共享 host seam** | `bcsp-application/src/host.rs:366`（v3 新增触点） | 第二个可选 WS 路由注册 seam（target 注入制） |
| 本地 presence(L2) | `bcsp-local-runtime`（经 seam 注册 `/api/v1/local/presence`）+ 前端页面级接入 | 每 tab presence 连接；count+generation+phase 状态机 → 60s 倒计时 → 到期复验 → 退出 |
| 本地多 tab | 前端（Web Locks）+ 服务端 authority | 所有 tab 平权编辑 desired（CAS）；监控由服务端逻辑 owner 持有；leader **只**决定谁播放声音 |
| 本地 WS 路由 | 共享 host seam | **受校验的路由集合**（非单条）：presence `/api/v1/local/presence` 与 desired `/api/v1/local/desired-watch` |
| 本地日志(L3) | `bcsp-local-runtime`（tracing stdout） | 关键事件 + 倒计时 |
| 政策 | `public-source-deny.json`、两 verifier、两 manifest、相关测试 | 通知家族拆分（批准的修订） |
| 测试翻转 | `live-watch-provider.test.tsx:860-932` | 改为"意外断自动重连、显式断不重连" |

## 5. 测试计划

1. 意外断 → 退避重连 → 按期望监控表 re-arm（已手动 STOP 的课**不**复活）；
   显式 Disconnect → 不重连；
1b. **调和循环（v3；v4 归属服务端 authority）**：物化健康但 arm 返回
   TARGET_UNAVAILABLE → 退避重试至武装成功；永久拒绝（admission 类）→
   移出 desired + 常驻通知；STOP/policy 变更作废在途 retry-epoch。
   **9 门帽自 v4 起是两类事件，不可混淆**：CAS 上面向用户的
   `LIMIT_EXCEEDED` 仍属 admission 类**永久拒绝**；而服务端 owner 连接
   上的物理 `RejectedLimit` 只是**内部物理槽条件**（旧槽尚未拆完），
   **不得**移除 desired；
1c. **多 tab（v3 表述已被 v4 取代，此处为翻译版）**：双 tab 同 desired
   表 → **单一响铃**；**leader 关闭且仍有 audience 时：不发生 re-arm，
   `activeWatchId` / `materializationEpoch` / manager watch 计数均
   不变，无重复 episode**，新 leader **只**接管音频职责；跨 tab STOP
   仍然收敛；
2. 公网换证：nonce 失效 → validate 换新（旧 nonce 废弃）→ 重连成功；
   validate single-flight 并发只发一次；服务器重启（registry 清空）→
   自动换证恢复；WS 心跳续期使挂机 nonce 不过期；
2b. **validate 合同（v3）**：状态码矩阵逐项（400/403/421/429+Retry-After/
   503）；per-IP 限流与首页同桶；renew 原子签新废旧；
2c. **reserve_ws 租约（v3）**：校验后-upgrade 前并发淘汰 → 握手仍成功
   （租约持有）；lease drop → 计数递减且 touch；per-session 帽在租约内
   强制；
3. 音频 suspended → 自动 resume；被拒 → DEGRADED + 兜底；
4. 通知触发矩阵：hidden+opened / visible+cue-failure 补发 / 去重 /
   权限 denied 时 Readiness 如实降级；
5. L1：监控中刷新 → 期望监控表恢复，**由服务端 owner 物化**（不是页面
   自动 START）；已 STOP 的不恢复；
6. Readiness 完整真值表：五环 × 各失效态逐一断言，UI 永不虚绿；
   心跳陈旧 >25s 降黄；跨 SPA 路由常驻；
7. L2：仅浏览页（无 watch）存活 → 不倒计时；全部页面关闭 → 倒计时 →
   60s 到期**复验 generation+count** → 退出；到期瞬间页面回归（竞态）→
   不退出；刷新/浏览器重启 60s 内回归 → 取消；
8. 政策修订（v3 扩展为 §2 第三层同步面的正反例全集）：四层（source/
   bundle/manifest/release gate）正例放行 + 反例拒绝；两种摘要一致；
   CI 入口实际执行。

## 6. 工作量

重连+desiredWatches+调和循环 2–2.5 天；多 tab leader 0.5–1 天；心跳+
Readiness+音频 1.5–2 天；通知+cue 补发 1 天；政策修订 1.5 天；公网
validate 合同+租约+NonceHolder 1.5 天；L1 0.5–1 天；L2 host seam+
presence+状态机 2 天；L3 0.5–1 天。合计 **11–13.5 天**。顺序在安全门之后。

---

### v2 → v3 变更记录（2026-08-20，回应 Codex 三审 4 项阻断 + 政策同步遗漏）

1. desired→armed 常驻调和循环（暂态退避/永久拒绝移除并通知/retry-epoch
   作废）；本地多 tab 采用 Web Locks leader 方案；
2. validate 请求合同冻结（体/locale/Host-Origin/体帽/状态码矩阵/与首页
   同桶限流/锁内原子）；WS 握手 reserve_ws RAII 租约消除 TOCTOU；
3. presence 依赖共享 host 新增第二 WS 路由 seam（按符号名引用
   `spawn_loopback_server_with_socket` / `handle_secondary_socket`，
   **行号已漂移，勿按行号引用**）。**v4 起该 seam 承载不止一条路由**：
   改为**受校验的路由集合**（presence `/api/v1/local/presence` 与
   desired `/api/v1/local/desired-watch`），见
   `2026-08-22-desired-watch-revision-cas.md` §7.2；
   倒计时改 count+generation+phase 状态机，到期复验；
4. 政策同步面补全为八项清单（slug/marker 分区/markerSetVersion、双摘要、
   rust verifier+测试、release gate 清单、CI 入口、四层正反例）；
5. 加固 H5 措辞修正：`active_ws_count` 仅计公网 watch WS（presence 为
   local-only，见加固文档）。

### v1.3 → v2 变更记录（2026-08-20，回应 Codex 初审阻断）

1. 403 分支作废 → validate/renew 端点 + NonceHolder + single-flight
   （握手失败浏览器不可分辨）；
2. 恢复对象改为 connection 无关的期望监控表（防复活已停课节）；L1 持久化
   对象同步改为该表；
3. 重连限定意外断开；显式 Disconnect 不自动重连；
4. 通知补 cue 失败分支 + alert/episode 去重（时序洞）；
5. L2 新增每 tab presence 通道（防误杀空闲页面）；
6. 应用层心跳 + READY 定义为 ≤25s 有界陈旧声明；Readiness 常驻全路由 +
   完整真值表测试；
7. frozen/discarded 页面明确列为 residual risk，撤回该场景"必达"表述；
8. 通知政策由"边界解释"改为**正式修订**（产品负责人批准），列全同步项。

（v1→v1.1：撤 resumeHint，重连即重新 START；v1.1→v1.2：L1/L2/L3 生命
周期契约；v1.2→v1.3：L2 60s 可见倒计时 + L3 控制台日志。历史详情见
git 与对话记录。曾发生针对 L1 的未授权编辑并伪造决策署名，已恢复留痕。）
