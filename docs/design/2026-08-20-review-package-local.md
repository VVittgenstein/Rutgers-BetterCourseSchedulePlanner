# 评审总案：本地版（Windows）全部变更

状态：v3 —— **已批准**（Codex 终审 2026-08-20；附录 gate v5 / alert v3.1）
日期：2026-08-20
性质：按产品切分的评审总案。`[共享]` 条目两份总案均出现，实现只有一份；
本文与附录冲突时以附录为准。

## 审阅指引

本地版完整变更 = 4 项共享 + 3 项本地专属 + 1 项共享政策修订。
顺序：S1 → S2（含 L1/L2/L3、政策修订）→ S3/S4 见缝插针。

---

## A. 共享改动（本地侧视角）

### S1. [共享] 快照完整性安全门（附录① v4）

机制同公网版总案 S1（v4 已闭合三审阻断：串行锁提交、目录身份哈希、
跨重启 MAX_GAP、迁移 runner 分段），不重复。本地注意：
- 用户可调间隔（watch 3–60s）下隔离探测节奏 = min(30s, watch 间隔)，
  三校区确认路径均可达；
- 迁移 0005 在用户机器上执行：runner 需先支持"事务前关 FK"（否则两个
  CASCADE 子表会被静默清空且检查不出）；升级测试三件套（父子行数/级联
  保全/阴性检出）标准从严；
- 假警报在本地还会污染个人历史，安全门同时保护实时与历史。

### S2. [共享] 提醒必达（附录② v3）

同公网版总案 S2：五环 Readiness（app 层心跳、≤25s 有界陈旧）、意外断开
自动重连（显式 Disconnect 不重连；re-arm 自**期望监控表**——不复活已
手动停止的课）、音频自愈、桌面通知兜底（cue 失败补发 + 去重；
frozen/discarded 页面为 residual risk）。v3 增 desired→armed 调和循环
（暂态拒绝退避重试、永久拒绝移除并通知）。
本地注意：断线诱因仅三种（RBCSP 进程重启、睡眠唤醒、标签页冻结/丢弃），
与路由器/Wi-Fi 无关（本机回环）。

### S2-P. [共享] 通知政策修订（产品负责人批准）

同公网版总案 S2-P：正式拆分 SYSTEM_NOTIFICATIONS 家族，新设"当前页面级
通知"窄允许项，同步两 verifier/两 manifest/marker 计数与 SHA/测试。
本地 slug 登记进 `capabilities.local.json`。远期本地专属的 Rust 原生
通知/托盘不在本案。完整八项同步面（含 release gate/CI/四层正反例）
见附录② §2 第三层。

### S3. [共享] 轮询节奏与相位（v2 按评审意见重写，细则同公网版总案 S3）

第 0 步区间删失法测真实周期与相位（`body_changed` 在
`open_batch_observations`）；统一 RebuildProfile/GridAnchor 覆盖全部重锚
点；失败/退避不吸附网格。本地专属注意：
- 默认 watch 间隔改为验证周期 ÷ 2（60s→30s 或 30s→15s）；
- **"整除即稳定"以验证周期 P 为准**（P=30 时 20s 不稳定），文档化；
- **既有用户显式设置不按值覆盖**（无法区分"旧默认 10s"与"用户自选
  10s"，一律不动，仅新装/重置采用新默认）；
- 默认 10s→30s 使 freshness 上界从 35s 放宽到 75s
  （`freshness.rs:48` 公式 2×interval+15s）——**作为显式契约变化**
  单独测试与记录。

### S4. [共享] 存储微优化 prepare_cached —— **评审已通过**。本地收益：
3s 极限轮询下提交开销 105ms→28ms。

---

## B. 本地专属改动（附录② v3 §2 生命周期契约）

契约总述：**记忆放哪，页面就是什么**。本地记忆在硬盘，页面可死而复生；
关闭页面 = 用户明确表达停止。

### L1. 刷新后完整恢复

持久化 **期望监控表**（section+policy；v2 修正：不是"selection+活跃
布尔"，不含 activeWatchId/响铃消耗）；已手动 STOP 的课不复活。有意修订
`bcsp-local-user-state` 的原设计声明。
**（v4 由 CAS 设计改写）**：不再是"页面加载后按表自动 START"——该表是
**服务端权威状态**，**由服务端逻辑 owner 按已提交的 desired 物化**；
页面只是**编辑者（revision/CAS）与事件 audience**。见
`2026-08-22-desired-watch-revision-cas.md`。
**多标签页所有权（v3 提出，v4 由 CAS 设计取代）**：监控改由**服务端
connection-independent 的逻辑 owner** 持有，**所有 tab 平权编辑**
desired 表（revision/CAS，持久表为唯一真相）；Web Locks 选出的 leader
**只**决定谁播放声音，转移时**不触发 re-arm**、`activeWatchId` 与
watch 计数不变。仍然杜绝重复 watch/重复响铃/跨 tab STOP 不一致，但不再
依赖 BroadcastChannel 镜像与转发编辑。见
`2026-08-22-desired-watch-revision-cas.md`。

### L2. 关闭浏览器 = 60 秒可见倒计时后退出

- **presence 通道（v2 关键修正）**：watch 连接只在点击 Start 后存在，
  仅数它会在 60s 后**误杀只浏览未监控的页面**。新增本地专属每 tab 一条
  presence 连接（页面加载即建立；经共享 host 新增的**第二 WS 路由
  seam** 注册为 local-only 的 `/api/v1/local/presence`——公网不注入即
  不存在），生命周期计数以此为准；
- 计数归零 → `count+generation+phase` 状态机进入倒计时，控制台每秒输出
  「页面已全部关闭，N 秒后退出；页面回归即取消」→ **到期复验**
  generation 与 count 后才优雅退出（杜绝回归/到期竞态误杀）；
- 覆盖场景：空闲浏览页不倒计时；刷新 1–2s 回归取消；浏览器重启 60s 内
  回归取消、超时视同关闭（重新启动后 L1 恢复）；异常断连同关闭。

### L3. 控制台日志

启动信息 / presence 连接与断开（含计数）/ 监控 START-STOP / 开放警报 /
安全门事件 / 退出倒计时 / 优雅退出；高频轮询 debug 级不刷屏；语言随
locale 设置。

### 本地生命周期全景

| 用户动作 | 系统行为 |
|---|---|
| 刷新页面 | presence 秒回 → 取消倒计时 → 重连 → L1 恢复监控 |
| 重启浏览器 | 60s 内回归 → 同上；超时 → 视同关闭退出，重启后 L1 恢复 |
| 关闭浏览器 | 控制台倒计时 60s → 优雅退出，数据完好在盘 |
| 重新双击启动 | 进程起 → 开页面 → L1 按期望监控表恢复 |
| 电脑睡眠唤醒 | 页面自动重连（S2），监控恢复 |
| 进程崩溃/升级 | 页面退避重连直至进程回归，自动 re-arm |

---

## 不做（已定决策）

浏览器自动注册；响铃额度改动（会话级 3 声为设计意图）；增量写
`open_section_current`；按值覆盖用户已有间隔设置。

## 附录

- ① `docs/design/2026-08-20-open-snapshot-integrity-gate.md`（**v5，已批准**）
- ② `docs/design/2026-08-20-alert-delivery-integrity.md`（**v3.1，已批准**）
- 证据：`data/open-sections-repro/20260819T2117Z-original-capture/`
