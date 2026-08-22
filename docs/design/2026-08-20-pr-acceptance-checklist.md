# 实现阶段 PR 验收清单（设计评审遗留细节）

状态：ACTIVE。来源：Codex 四审共识——以下条目**不阻断设计**，但每条都是
对应实现 PR 的硬验收标准，缺一不合并。
日期：2026-08-20

## Gate（S1 实现 PR）

1. §8.7 并发测试按强串行语义实现：A 阻塞于取锁前 / 断言 B 持锁期间 A
   确实阻塞；不得出现"A 已持 Confirm 而 B 先提交"的（不可能的）前置。
2. `QuarantineRecover` 的**完整转移表**与测试补全（v4 文档部分转移仍以
   v2 表述为参照；实现以 gate v5 §4 状态机为准，逐转移建测试）。
3. candidate 双 runtime 的钉死测试（A→B 残缺首拉 Hold/不发布/零 fanout）
   与原子提升测试（gate v5 §4）。

## 提醒必达（S2 实现 PR）

4. 状态码全仓一致：`403` = Origin 不符、`421` = Host/authority 不符；
   容量耗尽一律 `503`、`429` 专用于限流（文档已统一，代码对齐）。
5. Readiness 的第②环必须验证**当前 connection/retry epoch 下**
   `desired(policy) == armed(policy)`——不是"连接活着"就绿。
6. app 层心跳**不得依赖 hidden-tab 会被节流的客户端定时器**：服务端驱动
   PING（WS 消息事件在后台标签页照常触发），页面在消息处理器内被动 ACK。
7. leader tab 接管后从**持久期望监控表**重水合（不信任 BroadcastChannel
   缓存）；BroadcastChannel 消息带 revision/ACK，或明确持久表为唯一真相。

## 加固（P2 实现 PR）

8. H4 出站定界**不得照抄"256 帧"**：64 KiB 帧上限下 256 帧 ≈ 16 MiB/
   socket；按**字节预算**（如 1 MiB/socket）+ 全局内存预算定界，并附
   慢消费者压测。

## 非阻断加固（Codex S1-PR2 复核建议，择机补做）

N1. 0005 升级测试增加 37 列 sentinel 值逐列前后比对；
N2. 索引与 FK 定义的精确快照断言（非仅计数）；
N3. FK 恢复失败的 fault seam 注入测试。

## S2 各 PR 验收携带项（Codex S2-PR1 批准时明确，L2 slice 硬验收）

S2a. `SecondaryWebSocketRoute::new` 路径校验：绝对、精确、无参数/通配符/
     查询片段、不等于内建 `/api/v1/watch`（防维护任务启动后 Axum panic）；
S2b. 本地集成测试直接钉死"未注入 → 404"（现有单测只钉到 fallback，
     404 是本地 extension 的行为而非 seam trait 保证）；
S2c. presence 上线前给共享 `serve_websocket` 补 WS 帧/消息大小帽
     （公网 host 已有 64KB，共享 host 目前无帽）；
S2d. presence 首帧 HELLO 承载 tab 身份：注册期限不受 Ping/Pong 延长；
     非法/重复首帧 → 关闭连接且计数回滚，逐一钉死；
S2e. `on_upgrade` pump 未纳入 `LoopbackServer` 所有权属基线既有
     shutdown 债务——L2 流程以"presence count=0 → 等 60s"为序，不受
     影响；记录在案，不新增义务。

## 通用

9. 每个 PR 的验收段引用本清单对应条目编号；条目完成后在本文件勾除。

## S1-PR3 复审阻断修复（Codex 2026-08-21 驳回的 5 项 P1，全部在 PR3.1 修复）

B1. 生产 persistence wrapper 转发 LKG/history 读取——trait 默认值删除
    （改为必须实现），`ShortLockOpenPersistence` 显式经 `with_storage`
    转发；端到端验收测试钉死（重启后 candidate 播种自持久 LKG）。
B2. unseeded runtime 首次成功即建立 baseline（seed 随 decision 的
    next_state 走 commit-before-advance）；单元 + 端到端钉死。
B3. `SUSPECT_PARTIAL_SNAPSHOT` 进入 failure 投影模型：`OpenFailureClass`
    新增 3 个 wire 值（连同**预存同型缺陷** `UNSAFE_EMPTY` /
    `UNSAFE_ZERO_INTERSECTION`——原码全仓无映射，一次即永久打挂
    `/open/status`）；golden +3；持久化 Hold→projection 集成断言在
    端到端测试内。
B4. 重启重建逐对校验历史样本 `MAX_GAP`（0 <= newer-older <= 120s）；
    单元钉死（越界截断 / 恰在界内保留）。
B5. candidate 历史隔离 + 精确 section-set identity 绑定：迁移 0006 新增
    `gate_catalog_set_identity` 列，gated commit 写入，summaries 查询排除
    `candidate_catalog_observation_id IS NOT NULL` 并回传 identity；重建
    要求逐行 identity 精确匹配（不匹配即断 run）。存储 + 单元钉死。
N4（原非阻断 candidate 容量）：candidate map 淘汰被替换者，容量钉为 1。
N5（原非阻断直连 API）：`with_parts` 改为**每 target 默认自带**
    `TargetWorkflowControl`（官方 runtime 的 attach 覆盖之）——直连
    coordinator 不再存在 gate 静默脱钩；这也使集成测试线束真实带 gate。

B5b（PR3.1 复审残留，PR3.2 修复）：已发布 candidate 的 breaker 语义——
    summaries 查询按"对 serving 状态做了什么"区分 candidate 行：未发布
    （Hold/unsafe/失败/中断）继续隐藏；成功发布（VALID_APPLIED /
    VALID_EMPTY_NO_ROWS，发布与提交同事务，行存在即已提升）保留为
    非-suspect breaker。评审者反例（同 identity、发布前后同 hash、
    全 gap≤120s）在 gate 单元 + 存储层双钉死：重建只继承发布后的
    一个 suspect。
N6（原非阻断负 gap）：episode 全部时间边统一 `(0..=120s).contains`——
    活跃续用负 gap 重新锚定；重启 newest 来自未来则拒绝续用（LKG 地板
    仍设防）。单元钉死。

## 进度

- [x] S1-PR1（gate 决策核心，8e83ee4）——**Codex 已批准 v5.1 迟滞追认**
- [x] S1-PR2（迁移 runner + 0005，0b80b7a）——**Codex 复核通过**
- [x] S1-PR3（接线，93f88d7）——经 PR3.1/PR3.2 修复后闭环
- [x] S1-PR3.1（58d3ba1）——**Codex 批准 B1-B4**；B5 余项见 PR3.2
- [x] S1-PR3.2（8384d45）——**Codex 批准**（B5b + 负 gap；扫描 0 findings）
- [x] S1-PR4（前端展示，bef6d24）——**Codex 批准，S1 全量收口**
      （扫描 0 findings；发布基线含 bef6d24 即解除"PR3 不独立部署"约束）。
