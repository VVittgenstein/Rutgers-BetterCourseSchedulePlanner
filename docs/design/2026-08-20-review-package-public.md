# 评审总案：公网版（Linux 服务器）全部变更

状态：v3 —— **已批准**（Codex 终审 2026-08-20；附录 gate v5 / alert v3.1 / hardening v2）
日期：2026-08-20
性质：按产品切分的评审总案。`[共享]` 条目两份总案均出现，实现只有一份；
本文与附录冲突时以附录为准。

## 审阅指引

公网版完整变更 = 4 项共享 + 3 项公网专属 + 1 项政策修订。
顺序：S1 → S2（含 P1、政策修订）→ P2 → S3/S4 见缝插针。

---

## A. 共享改动（公网侧视角）

### S1. [共享] 快照完整性安全门（附录① v4）

残缺名单（已验真）→ 逐目标基线 + 粘性隔离 + LKG 保留 + 恢复/确认双出口。
v3/v4 关键闭合：Gate 只包裹本可应用的快照（零交集/race 永远优先且非样本）；
隔离专用探测节奏全校区 ≤30s；重启续接仅认哈希全等 + 跨重启 MAX_GAP；
迁移 runner 分段提交 + 事务外关 FK（readback/失败弃连接/可重试）；
PreGateAssessment → GateDecision{Apply/Hold} + **per-target 串行锁**横跨
决策/事务/状态推进（epoch 降级为锁内不变式断言）；目录身份绑 section-set
哈希，serving/candidate 隔离。
公网注意：429 冻结全体的爆炸半径不变（维持固定策略的根本原因）；迁移随
部署流程生效；投影新增独立 SuspectPartial 类。

### S2. [共享] 提醒必达（附录② v3）

五环 Readiness（READY 为 ≤25s 有界陈旧声明，app 层心跳支撑）+ 意外断开
自动重连（显式断开不重连；re-arm 自期望监控表，不复活已停课节）+ 音频
自愈 + 桌面通知兜底（含 cue 失败补发与去重；frozen/discarded 页面为
residual risk，不承诺必达）。
公网注意：断线诱因含部署重启与 Caddy reload——重连是公网生存必需。

### S2-P. 通知政策修订（**产品负责人 2026-08-20 批准**）

现行 deny 政策明含 `browser_notification_api` 等 marker 且前端双扫描器
命中（评审方指出，已核实）——不做解释绕行，正式拆分 SYSTEM_NOTIFICATIONS
家族：新设窄允许项"当前页面级通知"；继续禁止服务端推送/native/
service-worker/web push。同步 frozen marker count 与 semantic SHA、前端
两 verifier expected list、两 manifest、release gate 清单、CI 入口与四层
正反例测试——完整八项同步面见附录② §2 第三层。+1.5 天。

### S3. [共享] 轮询节奏与相位（v2 按评审意见重写）

- **第 0 步（修正）**：`body_changed` 在 `open_batch_observations`
  （0002 L140），非 attempts 表。方法改为**区间删失**：按 target 取
  "前一 unchanged 与首次 changed"请求对形成变化时刻区间，分别拟合 30/60s
  候选周期与相位（既有 2h05m NB 长跑对 :02 方向有支持，但仅一 target
  夜间窗口，不足为据）。
- **统一 `RebuildProfile { period, safe_offset }` / GridAnchor**：覆盖
  全部重锚点（`complete_open_bootstrap`、concurrent open、watch 升降级、
  manual due、外部 adoption、启动恢复），不再是"播种 + retry 修正"两点。
- **失败/退避不吸附网格**：429/origin-pause/5-10-20s 序列原样；仅首次
  成功后回网格。
- **公网固定性保留**：不取消 `PublicIsFixed`，仅按验证结果修订固定值——
  周期 60s → `30/30`；周期 30s → `30/15`。
- **诚实数字**：请求量降 **33–67%**（10→15 为 33.3%）；新健康态最坏等待
  `<30s`/`<15s` 可能慢于旧 `<10s`，"检测不变慢"仅是主枪高置信命中下的
  统计结论。
- **抗抖动/防羊群**：safe_offset 计入本地壁钟偏差与源站正向抖动；
  per-target 稳定小偏移错峰；新增 6 target × 3 并发 × 慢上游测试。

### S4. [共享] 存储微优化 prepare_cached —— **评审已通过**，无变更。

---

## B. 公网专属改动

### P1. 会话票续命与换证（附录② v3 §2b）

WS 心跳续期 nonce（挂机不过期）+ **`POST /api/v1/session/validate`**
换证端点（签新废旧；浏览器无法分辨握手 403，故重连前 single-flight
校验，不做"被拒分支"；请求合同已冻结：状态码矩阵/与首页同桶限流/锁内
原子签新废旧）+ WS 握手 reserve_ws RAII 租约（消除校验-升级间 TOCTOU）
+ 前端 NonceHolder 可变持有。覆盖长离线与服务器
重启（registry 内存态清空）两种作废场景。

### P2. 上线加固（附录③ v2）

配置/代码现在做进仓库，真机验证待部署。v2 要点：
- H4 **补背压**：出站队列改有界 + 慢消费者断开（连接数封顶约束不了
  unbounded queue）；Caddy 每 IP 限流**指明非标准 directive**，采用
  mholt/caddy-ratelimit 模块并写明部署方式，或改为应用层限流；
- H5 **驱逐策略重写**：跟踪 active_ws_count（仅计公网 watch WS，租约制），
  优先淘汰最旧 inactive；
  全部受保护时才 503（容量耗尽；429 专用于限流，与 validate 合同及
  附录③统一）；换证时废弃旧 nonce；
- H1/H3/H7/H8 维持；**H9 与公网组装集成测试 = 重新上线硬门**（推迟但
  必须在上线前完成）。

### P3. 公网生命周期契约（确认保留现状）

刷新/重启浏览器 = 归零；标签页存活 = 自动重连恢复（S2+P1）。

---

## 不做 / 推迟

- 不取消 `PublicIsFixed` 固定性（仅按 S3 验证结果修订固定值）；
- 不做浏览器自动注册；不做 100% 严格校验；不做增量写；
- 推迟（重新上线硬门）：公网组装集成测试、H9 soak。

## 附录

- ① `docs/design/2026-08-20-open-snapshot-integrity-gate.md`（**v5，已批准**）
- ② `docs/design/2026-08-20-alert-delivery-integrity.md`（**v3.1，已批准**）
- ③ `docs/design/2026-08-20-public-prelaunch-hardening.md`（**v2**）
- 证据：`data/open-sections-repro/20260819T2117Z-original-capture/`
