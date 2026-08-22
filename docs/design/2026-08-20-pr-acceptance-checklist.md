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

## S2 部署硬门（Codex S2-PR2 复审裁定，整批部署阻断）

S2-D1. **H4 全局/per-client WS 上限与背压（P2 加固）落地之前，
       `feat/s2-alert-delivery` 分支不得部署**。理由：租约钉死 +
       validate 匿名签发面下，全局签发预算只能限速（rate shaper，
       非容量安全边界——Codex 追认措辞），不能阻止最终填满
       registry；只有 H4 的 WS 全局上限能阻止攻击者把 4096 个会话
       全部钉为不可淘汰（首页/validate 永久 503）。
       **规格冻结（Codex S2-PR2.2 轮要求，H4 实现时为硬验收）**：
       a) global active-WS cap **= 1024**，严格小于 registry 容量
          4096，保留 3072 项可淘汰 headroom（数值待产品追认，
          方向已冻结：cap < 4096 且 headroom ≥ 50%）；
       b) per-client 并发 WS 上限 **= 64**，client 身份与限流器的
          规范化 key 完全一致（IPv4 全址 / IPv6 /64 聚合 /
          v4-mapped 归 IPv4 / 无头归 direct）；
       c) 验收测试：WS 达全局帽时，registry 必须仍可签发新会话
          （被钉死的租约集合 < 容量，首页/validate 不 503）；测试
          直接钉死 leases 与容量的数值不变量（1024/4096 关系）。
       **参数终认（Codex 2026-08-22）**：全局预算 600/10 批准（仅
       rate shaper）；global cap 1024 批准（75% headroom）；
       per-client 64 批准为首发默认值。
       **H4 实现级验收（随参数终认新增）**：
       d) per-client 拒绝需有指标与可调能力（校园 NAT/共享 /64
          可能误拒）；
       e) cap 1024 仅在 H4 同时落实 per-socket 字节帽 + 全局
          outbound 内存预算时成立（1 MiB × 1024 > 主机 ~955 MiB）；
       f) "active-WS" 语义：permit 在 `reserve_ws` 前或同一原子准入
          中取得，计入尚未完成 upgrade 的租约，RAII 贯穿
          upgrade/pump 并覆盖所有失败退出路径；
       g) 发布前确认无仓外旧版 manifest consumer，或为其同步
          `$literal:true` 支持 / 升级 schemaVersion。
S2-D2. XFF 最后一跳取值规则以"Caddy 为公网第一跳、默认覆写
       X-Forwarded-*"为前提冻结；若将来接入 CDN/`trusted_proxies`/
       其他代理链，必须重新冻结取值规则并跑真 Caddy 链路测试。
S2-D3. **期望监控表接线硬门（Codex S2-PR3 复审裁定 B2）**：
       desired-watch 存储 API 为刻意无栅栏的 last-writer-wins（无
       revision、无操作序号、无删除 tombstone），S2-PR3 本身**不可
       独立部署/接线**——在本硬门关闭前，任何生产写入方不得接到
       `upsert_desired_watch` / `remove_desired_watch`。关闭条件
       （S2-PR5 硬验收，二选一已定为后者）：持久化操作序号+tombstone，
       或 **fenced sequencer**——所有 desired-watch 变更（含清空它的
       reset）在 watch-manager 的每命令临界区内按用户意图顺序同步
       下发，覆盖跨连接、leader/retry epoch 与 reset barrier。四个
       反例必须逐一钉死：
       a) STOP 后延迟旧 START 不得重插已停意图；
       b) 新 START 后延迟旧 STOP 不得删除新意图；
       c) 旧 policy 不得覆盖新 policy；
       d) reset 升代后，前代等待中的 upsert 不得重新落库。
       调用方合同已写入 store.rs / lib.rs 文档（SEQUENCING CONTRACT）。
       **PR5 执行语义（Codex 批准 8103168 时冻结，验收按此含义执行）**：
       e) "用户意图顺序"必须由 connection/leader/retry epoch 或
          revision 判定，不能等同于 manager 收包或抢锁顺序；
       f) 四反例测试必须让旧命令在新意图之后**实际到达**；只制造
          enqueue/SQLite 延迟不算通过；
       g) manager 临界区负责 freshness 校验、序号分配和入队线性化，
          不得横跨可能长时间阻塞的 SQLite 操作；
       h) reset 必须是同一 sequencer 的 generation barrier；
       i) 第一个生产调用方必须与 sequencer 和全部 D3 测试**同一 PR
          落地**；任何提前直调重新触发 P1。
       非阻断（PR5 一并做）：Windows 打包测试走完整路径——写入非空
       意图 → 重启恢复且 active=0 → reset 返回删除计数 → 再重启仍空；
       不能只断言空数组。

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

## S2-PR3 复审阻断修复（Codex 2026-08-22 驳回 da5be13 的 2 项 P1）

B1. bootstrap 合同断裂（PR3.1 修复）：Rust 无条件序列化
    `desiredWatches`，而前端 `hasKeys` 为键集全等、snapshot 校验器仍是
    旧字段表——新 payload 即使空列表也 BOOTSTRAP_INVALID。修复：TS
    `DesiredWatch` 类型 + `desiredWatches` 严格校验（元素级
    section/policy 全等键校验 + ≤9 帽）、三处 bootstrap fixtures
    （local-personal / product-bootstrap / local-bootstrap-fixture.mjs）、
    真实新 payload 解析测试（接受非空列表；拒绝旧形状缺键 / 元素混入
    live-watch 键 / 非法 policy / 10 项超帽）；同步非阻断漂移
    `PersonalResetResult.deletedDesiredWatches`。`npm run verify` 此前
    漏检的原因（fixtures 全为旧 payload）由新解析测试补上。HTTP 层
    对称断言（local_runtime.rs reset/bootstrap 处 `deletedDesiredWatches`
    与 `desiredWatches`）已随 PR3.1 补齐；打包 E2E
    `packaging/windows/verify.ps1` 的同型对称断言并入 S2-PR5（随 S2-D3
    测试工作一并做）。
B2. 无栅栏写入的时序反例（裁定为**延后+硬门**路径）：见 S2-D3——
    本提交标记为不可独立接线，fenced sequencer 及四反例测试列为
    S2-PR5 硬验收；PR3.1 将调用方 SEQUENCING CONTRACT 写入
    store.rs/lib.rs 文档。

## 进度

- [x] S1-PR1（gate 决策核心，8e83ee4）——**Codex 已批准 v5.1 迟滞追认**
- [x] S1-PR2（迁移 runner + 0005，0b80b7a）——**Codex 复核通过**
- [x] S1-PR3（接线，93f88d7）——经 PR3.1/PR3.2 修复后闭环
- [x] S1-PR3.1（58d3ba1）——**Codex 批准 B1-B4**；B5 余项见 PR3.2
- [x] S1-PR3.2（8384d45）——**Codex 批准**（B5b + 负 gap；扫描 0 findings）
- [x] S1-PR4（前端展示，bef6d24）——**Codex 批准，S1 全量收口**
      （扫描 0 findings；发布基线含 bef6d24 即解除"PR3 不独立部署"约束）。
- [x] S2-PR1（host 二级 WS seam，74eac23）——**Codex 批准**（携带项 S2a-S2e）
- [x] S2-PR2（validate 合同 + reserve_ws 租约）——经 PR2.1/PR2.2 修复后
      **Codex 批准 81c50b5**（B4 延后至 H4，规格冻结于 S2-D1）
- [x] S2-PR3（L1 期望监控表，da5be13）——驳回 2 P1 后经 PR3.1 修复
      **Codex 批准 8103168**（B1 关闭；B2 → S2-D3 接线硬门 + PR5
      执行语义 e-i；扫描 0 findings）
