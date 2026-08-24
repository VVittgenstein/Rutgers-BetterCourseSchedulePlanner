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
6. ~~app 层心跳**不得依赖 hidden-tab 会被节流的客户端定时器**：服务端~~
   ~~驱动 PING，页面在消息处理器内被动 ACK。~~ **S2-PR4 已关闭
   （56a9f70，Codex 批准）**；25s Readiness 判定仍属 PR6。
7. ~~leader tab 接管后从持久期望监控表重水合~~ **（v4 CAS 设计改写，
   2026-08-23 范围缩减后撤销，M0-M1-001 已实现替代物）**：
   **leader / Web Locks / BroadcastChannel 与"单一原子投影帧"全部作废，
   不得复活。** 持久 desired 表仍是唯一真相，所有 tab 经 CAS 平权编辑，
   但编辑走**本地 HTTP** 而不是投影帧流；监控由服务端 coordinator 的
   owner 连接持有，每 section 一份，告警**扇出给全部页面**（两个页面都
   会响，因为两个确实都在监控）。跨 tab 不实时同步：刷新后可见。
   见 `2026-08-23-desired-watch-reduced-scope.md`。

## 加固（P2 实现 PR）

8. H4 出站定界**不得照抄"256 帧"**：64 KiB 帧上限下 256 帧 ≈ 16 MiB/
   socket；按**字节预算**（如 1 MiB/socket）+ 全局内存预算定界，并附
   慢消费者压测。**同属 H4（Codex S2-PR4 批准时确认，非该差分引入）**：
   既有 pump 的出站通道无界且无 write timeout。

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
S2f（Codex S2-PR4 批准时新增，**PR6 硬验收**）：`WatchClient` 对 PING
     目前是浅层运行时解码。把 PING 计入绿色 Readiness 之前，必须要求
     `sequence` 为**正的 safe integer**，否则可信服务端的异常帧会造成
     假 contact。PR4 不回修。
S2g（同轮）：PING 的 10–10.25s 只是**正常 250ms ticker 下的入队节拍**；
     传输阻塞时实际到达可更晚——PR6 必须按 **25s 上界**降级，不得假设
     节拍即到达。

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
       **路线改判（产品所有者 2026-08-22 裁定）：本硬门改走二选一里的
       另一条——持久化 revision + tombstone + CAS**，不再走 fenced
       sequencer。理由与实跑证据见
       `docs/design/2026-08-22-desired-watch-revision-cas.md` §0：
       fenced sequencer 隐含"单写者"，而产品模型是**所有标签页平权
       编辑**；按注册序当 epoch 会造成"较旧但仍存活的连接的真实用户
       操作被静默丢弃"，即活跃 watch 与持久意图反向错位（已复现）。
       a–d 四反例继续有效。e–i **不整体作废**（Codex 复审 e53f525 时
       更正 v1 的说法）：**f（旧命令必须实际迟到）与 i（首个生产调用方
       与全部反例测试同 PR）继续有效**；**h 改写为"reset 进入同一
       authority transition barrier"**；e 与 g 改写——新旧由客户端携带的
       `(generation, revision)` 经 CAS 判定，CAS 在 socket 锁之外的
       authority actor 内执行。逐条映射见设计文档 §9。
       **v1 设计稿被 Codex 驳回的 5 组阻断已在 v2 闭合**：① tombstone
       淘汰重新引入 ABA（改为保留到 Full Reset，256 仅作告警阈值）+
       冲突后必须终止而非改号重发；② CAS 顺序未延伸到 manager 与广播
       （新增单一 authority transition actor，reset 与 0↔1 连接转换同入
       其 inbox，另加三处应用点拒绝倒退元组）；③ 现有 WatchManager 无法
       表示服务端统一拥有的监控（引入 connection-independent 逻辑 owner
       + 常驻 reconciler，arm 失败不回滚 desired）；④ mutation/bootstrap/
       reset 合同闭合（mutationId 绑定指纹与三态幂等、generation 为
       bootstrap 顶层必填、轮换广播完整 snapshot、desired=false 免准入、
       数值收敛到 MAX_SAFE_INTEGER）；⑤ 公网边界（desired 命令/事件/
       authority 全部移入本地专有 crate，走 S2-PR1 的 SecondaryWebSocket
       接缝，公网当未知命令拒绝）。
       **v2 设计稿的 4 组阻断已在 v3 闭合**：① per-row mutation 三列
       无法兑现幂等（改为 generation-scoped **receipt ledger**，记成功
       与全部**终局**拒绝，重复请求只向提交者重放、不 reconcile 不广播；
       `AUTHORITY_UNAVAILABLE` 明确为**非终局、不落 receipt、同 id 可
       重试**）；② actor 覆盖面补全（**每次 Attach/DetachAudience 入
       inbox 且由 actor 自算连接数与 0↔1 边沿**、启动恢复、actor 故障
       重建、generation 轮换、reset confirm、reconcile 重试完成），并新增
       **`materializationEpoch`**——`(G,R)` 排序 desired、epoch 排序
       armed，二者不可互替；压缩改为**同一 SQLite 事务内升代+清理+修计数**，
       提交后才广播；③ arm/disarm 失败模型闭合（暂态 5/10/20 封顶 30s
       不动 desired；永久由 authority **自身走一次 CAS** 移出 desired
       并通知——与已批准设计第 214/250 行一致，**不构成产品语义修订**；
       `SECTION_NOT_FOUND` 只有在 S1 gate 放行时才算永久；disarm 失败
       重试 + 旧 arm 完成防复活 + `pendingDisarm` 不谎报；
       `activeWatchId` 可空、每新 epoch 换新；ACK/DISMISS/CUE **不是**
       "既有 WS 不变"，需本地适配层按 `activeWatchId` 路由并向全体
       audience 扇出）；④ 拓扑抉择——host 单槽改为**受校验的 secondary
       路由集合**（逐项 S2a 校验 + 集合内路径唯一性），presence 与
       desired 各占一条本地路由；N-g 措辞按"不进入命令路由/不改 watch
       状态/不产生 dispatch/sink/reply"更正；绑定 S2b 逐路由 404 与
       S2c 64 KiB 帽为硬验收；新增前端负控（共享 provider 现仍在
       `LiveWatchProvider.tsx:412/818/823` 发那三条命令，须拆到本地专有
       模块 + import 图/manifest/public bundle 三重负控）。
       **v3 自查三镜头再挖出 11 项，v4 逐条闭合**，其中三项是根子：
       ⓐ **UI 投影规则完全缺失**——全文只写服务端机制，从未定义
       `(desired, armed, pendingDisarm)` 到"用户看到什么"的映射，而那
       恰是本纲领唯一要约束的东西。v4 §0 定死投影表，**唯一绿灯条件为
       `desired=1 ∧ armed=true ∧ ¬pendingDisarm`**，"已提交但未武装"
       必须显示为准备中；ⓑ **`mutationId` 的派生读法会永久锁死一个
       section**（满额被拒 → 名额腾出 → 同手势派生同 id → 永远重放
       LIMIT_EXCEEDED）——改为**每次用户手势新铸 UUIDv4**，绑定降级为
       校验规则；ⓒ **epoch 无分配点/无持久化/未按代作用域**——改为在
       CAS 同事务内分配并落列，armed 侧守卫比较
       `(generation, epoch, attemptToken)`，并新增不持久的
       `attemptToken` 解决 arm-vs-arm。
       其余闭合项：receipt ledger 主键补 section（否则跨 section 重放
       他人回执）、`MUTATION_ID_CONFLICT` 由已有行推导而非二次插入、
       `STALE_GENERATION` 不落 receipt、重放走独立帧
       `DESIRED_WATCH_REPLAYED`、policy-only 更新不换 epoch/不换
       `activeWatchId`、放弃本仓库不存在的 exactly-once 键改为沿用既有
       `(section_key, run_id, episode_id)`、`pendingDisarm` 补进 wire、
       disarm 按 section 寻址、失败分类覆盖 manager 全部 disposition、
       **9 门帽单一执行点**（CAS 判定，manager 对 owner 合成连接豁免）、
       owner 合成连接需防心跳回收、系统 CAS 的 id/base/重试语义、
       actor 故障重建须向现存 audience 重播完整双 snapshot、
       前端拆分改走 `ProductRuntimePort`（共享 provider 为两 target 共用，
       不能"不再发"）、public marker 计账（不新增 capability 行，改在
       `PERSISTENT_ACTIVE_WATCH` 追加并同步 212 常量与两个摘要）、
       `contracts.ts` 的 9 门帽与 `hasKeys` 全等检查三处硬阻断。
       **v4 又被驳回 5 组，v5 逐条闭合**：① receipt 主键与"同 id 换
       section 必须冲突"正面矛盾（section 在主键里会插第二行）——改为
       `PK(generation, mutationId)`、section/fingerprint 为被比较列；
       mutationId 粒度改为**每个发出的 section mutation 一个**（一次批量
       START 手势含多 section），系统 CAS 每次重试新铸；② §0 投影未达
       可批准版——`armed` 严格定义为"generation/revision/policy/epoch
       四项全等"（对齐清单第 5 条 `desired(policy)==armed(policy)`），
       补上 `desired=1 ∧ pendingDisarm` 这一合法状态，并把两帧投影
       合并为**单一原子信封 `PROJECTION_UPDATE`**（原方案在两帧之间
       可渲染出表外状态）；③ **manager 全豁免被否**——两个帽是两个
       不变量而非两个真相，全豁免会让物理 watch 9→18→27 无限翻倍；
       改为 CAS 管产品准入、manager 保留物理帽，旧槽未释放时新 section
       显示"准备中（等待旧槽释放）"，owner 的 `RejectedLimit` 是暂态
       告警而**非**删除 desired 的触发器；并新增按 `(generation, epoch)`
       幂等的 owner `ensure`（已持有 section 实际走 `Restarted` 会换
       ID、结束旧 episode、重复响铃），`attemptToken` 必须在 manager
       副作用**之前**校验；④ tombstone/receipt 无资源边界——**loopback
       输入不是可信人类输入**，可无限制造 receipt/tombstone 直至 FULL
       snapshot 超 64 KiB 使恢复永久失败；新增 mutation 限流 + 行数/字节
       硬预算 + 达预算前原子 rotation，且 rotation 必须**保留全部
       `desired=1` 行**、只清 tombstone 与 receipt；⑤ marker 同步清单
       闭合（追加 6 个冻结 marker，212→218，`markerSetVersion` 1→2
       三处同步，两个 SHA 重算，`verify-import-graph.test.mjs` 的
       18/212 硬编码，六个校验面正反例，local manifest 增
       `persistent-desired-watches`、public 不含），并新增**本地旧三
       命令的服务端 admission filter**（中立 manager API 本身不拒绝外部
       旧命令）。前端接缝改为 target-neutral 的意图/投影 port
       （`ProductRuntimePort.watch` 现为 wire 专用，共享 provider 自己
       构造命令，无法逐 section 铸 id/取 base revision/处理双 socket）。
       验收清单重编为**自包含的 A-01..A-32**（v4 声称保留的 N-h..N-n
       从未存在）。
       **v5 自查三镜头再挖出 15 项，v6 逐条闭合**，其中最严重的一条是
       v5 自己制造的：严格 `armed` 定义让"停止中"一行**永远不可达**
       ——STOP 一提交就换 epoch，幸存的物理 watch 因 epoch 不符而
       `armed=false`，于是落进"未监控"，而那份 watch 还活着还会响铃，
       **投影表自己造出了它要禁止的谎言**；v6 改为**有序求值**并把
       `pendingDisarm` 提到第一条。其余：`armed` 的四个比较项在 v5
       中不可计算（缺 generation/epoch 的物化孪生）→ 引入可空
       `materialized` 记录；`pendingDisarm` 一词被用于两种互斥语义
       → 拆为 `pendingDisarm` 与 `blockedOnSlot` 两个字段；
       "每轮恰好一帧"与自身两条路径矛盾 → 改为**至多一帧**且
       AttachAudience 的后续变更由 actor **自入队**；帧顺序守卫加
       **持久 `actorIncarnation`**（否则 actor 重建后强制重播的 FULL
       帧会被当作倒退丢弃，页面永久停在崩溃前投影）；`disarm` 改为
       **发起时捕获 id**（v5 执行时重解析会把用户刚 START 的新 watch
       拆掉）；新增 **`applyPolicy`** owner API（v5 只有 `ensure` 且
       同 key 零副作用，policy-only 更新永远无法生效、section 永久非绿
       ——一个活锁）；rotation 与 actor 重建换 epoch 时新增 **adopt
       路径**（否则一次自动压缩会把 9 份健康 watch 全部 `Restarted`、
       结束 episode、可能重新响铃）；`setDesired` 改为返回终局结果
       （v5 的端口没有任何通道能把 `MUTATION_RESULT` 送到 UI，撞满
       9 门帽在界面上毫无反应），端口补回 `state`/`connect` 等连接态
       与全部六条 episode 命令；新增 `REQUEST_FULL_PROJECTION` 与
       `transitionId` 连续性以支持**缺帧检测与恢复**；新增
       `SlotReleased` 事件（否则一次普通换课要等最多 30s 退避）；
       系统 CAS **不落 receipt**（v5 让它成为绕过限流的无界产出源）；
       §2.5 的**六个数值全部冻结**（2/s·60、512、2048、32 KiB×3、80%）；
       marker 由 6 个改为 **3 个**（规范化子串匹配下另三个是死条目）且
       **必须按序数序插入**、冻结到数组顺序，总数 **212→215**。
       验收清单扩至 **A-01..A-36**，补回 v5 漏掉的"客户端不得自动改号
       重发"与"一般暂态 arm 失败不回滚 desired"两项。
       **v6 又被驳回 3 组（§0.3 顺序、§2.5 六值、§7.7 数组/去重/215
       已获批准且不再重开），v7 逐条闭合**：① **帧流不可靠恢复**——
       补 §0.3 最高优先级第 0 条（未同步 ⇒ 重连中/同步中、非绿、
       **禁止用缓存 revision 发 mutation**）、`blockedOnSlot` 三项
       不变量、**定向 FULL 不消费全局 `transitionId`** 而携带
       `throughTransitionId`（否则 B Attach 会让 A 误判缺帧）、冻结
       接收状态机 R1–R7（含"FULL 必须删除帧中缺席的本地 entry"）、
       FULL 超时/single-flight/退避、`REQUEST_FULL_PROJECTION` 独立
       限流与合并、**audience registry 必须活在 actor 之外**、分块与
       "每轮至多一帧"统一为**共享 `transitionId` 的分块**、
       **`armed` 改为前端派生不上 wire**；② **owner 副作用幂等**——
       freshness token ≠ effect idempotency，新增稳定 `operationId` 与
       **manager 侧 effect batch**（原 action/episode/cue ID 保留到
       history 与 audience 双确认，actor 重建时用**原 ID 重放交接**），
       adopt 增五项前置校验，`applyPolicy` 按 `sectionRevision` 作
       稳定键且同 revision 重试不重复写 history，**`armAttempt` 与
       `disarmOperationId` 拆分**，`SlotReleased` 降为唤醒提示并新增
       **30s 周期 reconcile** 作独立活性源，**9 门帽改按 post-state
       判定**（否则满门时 policy-only 更新被误拒）；③ **端口两个承重
       面**——新增 `subscribeEvents`（否则警报与声音整体消失）、
       `setDesired` 返回 handle 且非终局由 adapter **以同一 mutationId**
       内部重试、公网 adapter 明示 STOP/UPDATE_POLICY 的退化完成语义。
       另：`RATE_LIMITED` 入 wire 且为非终局；§2.5 补执行合同
       （UTF-8 整信封计数、`floor(cap*4/5)` 三点边界、Retry-After、
       rotation 保留 incarnation、Full Reset 不重置 `transitionId`）；
       **所有本地 wire variant 统一加 `DESIRED_WATCH_` 前缀**并补
       "孤儿 tag 泄漏"负例（原名不含任何 marker，会从负控漏出）；
       两个 SHA 由复审方按冻结数组算出并写入设计文档。
       **v7 被驳回 5 组，v7.1 窄修闭合**（复审明示无需 v8 大循环，本轮
       只冻结行为契约，类型/锁/channel/签名转入 PR 验收）：
       ① **帧流**补分块组身份 `frameGroupId`、块头严格校验、**10s 组装
       超时**（防"actor 中途崩溃、末块永不到达"时页面永久保留旧绿态）、
       旧化身块丢弃、新 FULL 作废在装组；**A-17 更正为"下一广播是 T11
       且不得判 gap"**（原写 T12，与 R2 直接冲突）；32 KiB 明确为
       **单块 wire 帽**而非逻辑更新总帽；限流回
       `DESIRED_WATCH_RESYNC_REJECTED { retryAfterSeconds }`（非静默
       丢弃），否则 R6 重试时序不可确定；
       ② **effect batch 的"确认"给出可执行定义**——history ACK =
       **SQLite commit 或 `AlreadyPresent`**（现接缝返回 void、失败只
       记日志，不算）；audience ACK = 客户端显式
       **`DESIRED_WATCH_EFFECT_ACK`**（入队/`send()` 不算，那只到无界
       发送队列）；必需集合按 batch 创建时刻取、detach 视为满足、集合空
       则仅凭 history；客户端按 `(effectBatchId, effectIndex)` **先去重
       再发声且仍须 ACK**；未确认 batch **只能 `PENDING_HANDOFF →
       replay`，绝不落入 `Restarted`**；容量 64 个/256 KiB + 背压，
       **回收须待 history ACK**；**重建顺序更正为"先投 新化身 FULL、
       再按原顺序重放 effect"**（v7 写反了，按 R4 会被全部丢弃）；
       ③ **本地 episode 通道落线**（继续双 socket：既有 watch socket 承
       事件+六条 episode 命令+心跳，desired socket 承 CAS+投影+effect
       ACK；命令按 `activeWatchId` 路由到 synthetic owner；**owner 事件
       扇出给全部 audience**；**leader 只决定是否发声、不截断事件**；
       admission filter **只**拒旧三条；两条 socket 皆 OPEN 才算已同步）；
       ④ **公网 STOP/UPDATE_POLICY 改为 `SENT_UNCONFIRMED`**——明确
       禁止报告 `COMMITTED`（socket OPEN + `send()` 只证明浏览器接受了
       发送；STOP 的 `UnknownWatch` 只写日志；UPDATE 无 episode 时即使
       成功也无后续事件，会永久返回无法验证的成功）；
       ⑤ 恢复 v7 **误删**的零 audience 暂停语义（`1→0` 拆除物理 watch
       并保留 desired、`0→1` 从权威 snapshot 重新物化）。
       另：**终局性与"是否记 receipt"分为两张表**（A-14 与 A-06/A-07
       冲突的根因——`STALE_GENERATION` 与 `MUTATION_ID_CONFLICT` 都是
       终局但都不写库）；`SET_DESIRED_WATCH` 更名 **`DESIRED_WATCH_SET`**
       使前缀规则字面成立；补做三处跨文档残留（alert 的客户端调和循环与
       页面自动 START、"本设计无服务端持久个人状态新增"断言、
       review-package-local 的页面自动 START）。
       **v7.1 被驳回 4 组，v7.2 窄修闭合**：
       ① **分块的"整组原子提交"在 v7.1 重写 §2.2 时被误删**——恢复并
       加强为 C0/C0b：全部块校验+集齐+按索引拼接后**只提交一次**、
       **部分组绝不触碰 store**、**R5 只对完整 FULL 组的并集执行一次**
       （否则 `[A]`/`[B]` 两块互删）、**首块到达即 ASSEMBLING**（不是
       等 10s 超时才降级）；C2 同组五元组补 `authorityGeneration` 与
       `throughTransitionId`；C6 同时只装一组、异 DELTA 组到达则两者
       皆弃；C7 更旧的同化身 FULL 不得回退 `last`/不得作废更新的在装组；
       C8 每组 **256 KiB / 16 块**（单块帽不约束组装内存）；
       ② **effect 真正上 wire**——原稿要客户端按
       `(effectBatchId, effectIndex)` 去重，服务端事件却不带这些字段；
       新增**本地专有** `DESIRED_WATCH_EFFECT` wrapper（
       `audienceBindingId`/`effectBatchId`/`effectSequence`/
       `effectIndex`/`effectCount`/`projectionFence`/`watchEvent`），
       客户端**集齐并按序处理完整 batch 才 ACK**；wrapper 不进
       `bcsp-contracts`，marker 计数不变；
       ③ **双 socket 的绑定与跨流因果序**——FULL 走 desired、effect 走
       watch，两条 TCP 流之间**没有"先发即先到"**；新增
       `audienceBindingId` 成对、`AttachAudience` 须待两半成对**且首个
       FULL 已应用**、任一半断开 pair 失效且**只产生一次 Detach**、
       `EFFECT_ACK` 校验 required set 归属、effect 按 `projectionFence`
       缓存;**同步态改为 composite**（两 socket OPEN + 首 FULL 已应用 +
       非 DESYNCED + 非 ASSEMBLING），raw OPEN 不算；
       ④ **非 ACK audience 可永久钉死系统**——容量帽只解决内存不解决
       活性；新增**每 audience 30s ACK deadline + 超时判 detach**、
       精确 GC 条件 `historyAck ∧ (每个 required audience 已 ACK 或已
       detach)`、以及 **STOP/Full Reset/`1→0` disarm 的独立预算**
       （16 个 / 64 KiB），普通 effect backlog **不得阻塞物理拆除**。
       **PR5/PR6 切分裁定**（设计文档新增 §10）：PR5 **不是可独立启用
       件**（filter 先启用→旧前端监控失效；PR6 先启用→desired route
       404 永不同步；先要求 EFFECT_ACK→旧前端永不 ACK 触发背压），
       故 **PR5 以完全 dormant 形态合并**（不注册路由/不启用 filter/
       不启动 owner 物化/不关闭 S2-D3），**激活必须与 PR6 原子发布**或
       引入明确的协议版本-能力握手；release-gate E2E 须覆盖新旧四组合
       与单边 socket flap。
       另：公网文案改为**"已发送，协议不提供确认"**（不是"等待确认"，
       公网 wire 上根本不会再来确认）；验收清单**连续重编为
       A-01..A-53**（原 A-31b 与错位的 A-45 已消除）；alert 文档补做
       触点表 L1、测试 5 的"加载自动 START"与"第二 WS 路由"单数表述。
       **§10 两项已裁定**：打包冒烟播种为 **P1 release gate**（首个生产
       writer/整批发布前必须完成，需确定性 PUBLISHED fixture 且 S1 gate
       放行，`desired=false` 不可替代）；已批准设计测试 1c **取代但需
       翻译**（leader 关闭且仍有 audience 时 activeWatchId/epoch/watch
       计数不变、无 re-arm、无重复 episode，新 leader 只接管音频，跨 tab
       STOP 仍收敛），须写回 alert-delivery-integrity.md。
       本硬门在 CAS 路线落地并复审通过前保持关闭。
       **打包 E2E 非空路径门保持开放，且已被裁定为 P1 release gate**
       （Codex 2026-08-23）：必须在**首个生产 writer / 整批发布之前**
       完成，需**确定性的 PUBLISHED section fixture** 且 S1 gate 放行；
       `desired = false` 的 tombstone 提交**不能**替代 true-path 验收。
       （早前"路由落地即可关闭该门"的说法作废——路由只是使其**可达**。）

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

## S2-PR5b（host 受校验二级路由集合，dormant）

设计依据 §7.2/§7.3。本片**不注册任何 desired/presence 路由**，只把承载
能力做出来，S2-D3 保持开放。

落地：
- `SecondaryWebSocketRoute::new` 改 **fallible**，承载 S2a 路径校验
  （绝对 / 无空段 / 无 `.`、`..` / 无 query、fragment / 仅 RFC 3986
  unreserved 字面量——`{param}`、`*rest`、`:kind`、`%2D` 全部落入
  NotLiteral / 不得等于内建 `/api/v1/watch`）；新增
  `LoopbackServerError::SecondaryRoutePath { path, rejection }` 与
  `SecondaryRoutePathRejection`。
- `spawn_loopback_server_with_sockets` 收 **一组** 路由；`HostState` 改
  `Arc<BTreeMap<&'static str, Arc<dyn WebSocketExtension>>>`；**集合内
  路径唯一性在 bind 之前**校验（axum `Router::route` 重复路径会 panic，
  且失败不得泄漏已绑定端口）。
- **S2c**：新增共享 `shared_websocket_upgrade()`，把 64 KiB 帧/消息上限
  与子协议 offer 收敛到唯一一处；公网 host 删除自带常量改用它，本地
  loopback host 两条 WS 路由同时获得该上限。
- **S2b**：`bcsp-local-runtime` 冻结两条本地路径常量，集成测试对**每条
  路径**钉死"未注入 → 404"（WS 升级形与普通 GET 形各一），并同时钉死
  内建 `/api/v1/watch` 仍为 101。

S2-PR5b.1（Codex 驳回 `a77d9e0` 的 1 项 P2 + 1 项 P3）：
- **P2**：64 KiB 测试只发单个 FIN text frame，因此**只证明了 frame cap**
  ——误删 `max_message_size` 仍全绿。补**分片消息**判别测试：
  `32 KiB Text(FIN=0) + 32 KiB Continuation(FIN=1)` 合计恰好 64 KiB →
  交付一次；`32 KiB + (32 KiB+1)` 每帧均低于帽、合计超限 → 断连且零交付。
  **实测：仅删 `max_message_size` 时该测试 3/3 FAILED,单帧测试仍 ok。**
  同时补对称缺口——单帧测试同样不能证明 `max_frame_size`（只有
  `max_message_size` 时 65537 字节单帧也会在重组处被拒）。新增**声明长度**
  测试：宣称 1 MiB 只发 8 字节 → 必须在**帧头**即拒绝而不缓冲。
  **实测：仅删 `max_frame_size` 时该测试 2/2 FAILED,恢复 5/5 ok。**
  （1 MiB 而非更大：axum 默认 frame cap 为 16 MiB,更大的声明会被默认值
  拦下,证明不了本 host 设置的帽。）
- **P3（第一轮:欠读）**：`websocket_handshake` 只调用一次 `read`,body 与
  headers 同批到达是 TCP 不保证的。改为读满 headers 再按 `content-length`
  读满 body；101 无 body 即在头终止符处停止（连接保持,不能读到 EOF）。
- **P3（第二轮:超读,Codex 批准 `3aa4935` 时提出）**：同一次 `read` 也可能
  把响应之后的字节一并带回——101 之后服务端**立即**发的心跳 Ping
  `0x89 0x00` 不是合法 UTF-8,`String::from_utf8` 会偶发 panic。退出前
  `response.truncate(header_end + length)`；helper 随后丢弃 socket,无需
  保留 remainder。**已确定性复现:握手后强制延迟 100ms 使 Ping 与 headers
  同批到达,去掉 truncate 即 `FromUtf8Error{ ..., 137, 0 }`(valid_up_to
  199);加回 truncate 3/3 通过。**

三点披露：
1. 64 KiB 上限是**入站**约束（tungstenite 仅在 `read_message_frame`
   校验），出站帧不受限；§2/§6 的 chunk 预算是另一回事。这是本地 watch
   路由上的**真实行为变化**（此前本地无帽，公网已有同值帽）。
2. 共享 pump 在 extension **丢弃 outbound sender** 时立即结束连接
   （`outbound_messages.recv()` 返回 `None` → break）。既有行为，正确，
   但会让"只读"测试 socket 自己拆掉连接——测试替身必须持有 sender。
3. `tokio::time::interval` 首拍立即触发，因此每条连接建立后服务端**立刻**
   发一帧 Ping；判定"对端是否关闭"必须走帧而不能只看首字节。

## S2-PR5c（CAS 写入器，dormant）

设计依据 §3.2（五步顺序）、§5.5/§5.7（两个帽）、§5.6（系统 CAS）。
本片只落**存储层写入器**，无调用方、无路由、无接线，S2-D3 保持开放。

落地：
- `commit_desired_watch_mutation(&command, admission)`：单条 `IMMEDIATE`
  事务内按冻结顺序求值——① generation ② receipt 幂等 ③ revision CAS
  ④ **post-state 准入帽 + 准入判定** ⑤ 写入。
  **【已撤回】** 初版此处写"任何拒绝都不留痕"，与 §2.4 / §6.2 冻结的
  receipt 表直接冲突，见下方 PR5c.1。
- **准入帽按 post-state 判定**：仅 `0→1` 占额，因此**满 9 门时的
  policy-only 更新照常提交**。v6 的"当前 count < 9"会误拒它——满员的
  监控列表将变成不可编辑。
- **`DesiredWatchLimitExceeded` 不再是孤儿**：它作为**错误**是错的形状
  （把产品裁决和 SQLite 故障混在同一个 `Result` 里），已删除；改为
  outcome `LimitExceeded { maximum }`。新增错误
  `DesiredWatchCounterExhausted`(计数器触达 `Number.MAX_SAFE_INTEGER`)。
- **receipt 幂等**：同 `(generation, mutationId)` 且指纹相同 → 重放原
  结果（`AlreadyApplied`），**不写第二行**；指纹或 section 不同 →
  `MutationIdConflict`，**由既有行派生**。指纹 = SHA-256(section ‖
  basedOnRevision ‖ desired ‖ policy_json)。
- **epoch 规则**：revision 每次提交都前进；`materialization_epoch`
  仅在 `desired` **值**变化（或行不存在）时新铸——policy-only 编辑保留
  已武装 watch 的身份，这正是"调整"与"重启"的区别。
- **STOP 完全跳过准入**：一个事后变得不受支持的 section 仍必须停得掉。
  `PendingGate` **提交**而非拒绝（§3.2 步 4）。
- **系统 CAS 不记 receipt**（§5.6）：每次重试新铸 id，记了只会用永不复现
  的 id 撑满 ledger。

三处**判别力实测**（去掉规则则测试失败，装回则通过）：
1. 把 post-state 帽换成 pre-state（v6 形状）→
   `the_cap_counts_the_state_a_mutation_leaves_behind` FAILED
   （满 9 门的 policy 编辑被误拒）。
2. 反转 epoch 保留条件 → `a_policy_edit_keeps_the_epoch_and_a_desired_
   change_allocates_one` FAILED（left 2 / right 1）。
3. 关掉 receipt 查询 → `a_repeated_mutation_id_replays_...` FAILED，
   重放变成 `StaleRevision`——即"客户端丢了响应后重试,被告知过期,按
   §3.1 必须终止",用户的 START 静默不发生。

两点披露：
1. 写入器取 `&mut self` 并自开事务,**不可**嵌入 `consistent_read()`；
   这与 PR5a.1 的可组合性问题不同——那是读路径,写入器嵌进读事务本身
   就是错的。类型上也不可能(`&mut self` vs `&self`)。
2. **absent → STOP 会建 tombstone 并新铸一个 epoch**。这是刻意的:页面
   在 START 落库前取消时可达,建行让 revision 线从此处起算,使仍在途、
   携带 `basedOnRevision = 0` 的 START 失败而不是被放行。已单测钉死。

### S2-PR5c.1（Codex 驳回 `72c6383` 的 1 P1 + 3 P2 + 2 非阻断）

**P1（我的规格违反，最严重的一次）**：终局拒绝没有 receipt。§2.4 写明
"只记客户端提交的成功与**终局拒绝**"，§6.2 的表进一步逐项冻结：
**记** = 成功 / `STALE_REVISION` / `LIMIT_EXCEEDED` / `UNSUPPORTED_TARGET` /
`TERM_OUT_OF_RANGE` / `SECTION_NOT_FOUND`；**不记** = `STALE_GENERATION`
（步骤 1 即判定）/ `MUTATION_ID_CONFLICT`（由既有行推导）/ 两个非终局项 /
系统 CAS。我只实现了成功路径。
评审给的反例是精确的:X 对空行提交 `basedOnRevision=1` → 终局
`StaleRevision(current=0)`；Y 以 base 0 建行得到 revision 1；X **原样重试
即通过并提交**。同理,槽位释放会让原 `LIMIT_EXCEEDED` 变成成功,目录发布
会让原 `SECTION_NOT_FOUND` 变成成功。**被告知"否"的同一个 mutation 稍后
静默变成"是"**——正是本子系统存在的理由所要防的那类错位。
修复:终局拒绝写 receipt 并**提交该事务(仅 receipt)**,不写 desired 行、
不动计数器；重复呈现同 id 得到 `Replayed(原结果)`。

**P2-1**:惰性 admission 无法表达**非终局** unavailable。本地
`LocalWatchAdmission` 确有 `TargetUnavailable` 路径(快照未就绪 / 投影
建不起来 / 数据库锁被占)。旧枚举只有 `Admit`/`PendingGate`/永久 `Reject`,
调用方只能提前求值、误报永久拒绝、误当 `PendingGate` 提交,或 panic。
新增 `DesiredWatchAdmission::Unavailable` → outcome `Unavailable`:
**不写 receipt、不写任何东西、同 id 可重试**,且仍在 generation/receipt/
revision/cap 之后求值。

**P2-2**:`System` 未被限制为"退休仍然 desired 的现有行"。旧形状用
`source` 标志控制"是否记 receipt",于是 System START 能无 receipt 地
**创建**意图,System STOP 能对 absent/tombstone 行继续写。改为**独立
受限 API** `retire_desired_watch(section, based_on_revision, generation)`:
`DesiredWatchCommand` 删除 `source` 字段,退休**在类型上**只能清意图、
不能创建；行不存在或已是 tombstone → `NothingToRetire`(§5.6"用户已删除
则不再提交");不咨询准入(它存在的原因就是准入说了不);不记 receipt。

**P2-3**:持久格式缺 golden 与 reopen 验收。新增
`the_persisted_receipt_format_is_pinned`:逐字钉死 SHA-256 指纹与
`outcome_json` 四种形态(`COMMITTED` / `STALE_REVISION` / `LIMIT_EXCEEDED` /
`REJECTED`),并在注释中写明指纹**原像**(term‖0‖campus‖0‖index‖0‖
basedOnRevision(8B BE)‖desired(1B)‖policy JSON)；
`every_recorded_answer_survives_closing_and_reopening_the_database`:
9 次成功 + 三类可 receipted 拒绝,**关库重开**后逐条重放一致,且重放时
用 panicking 的 admission source 证明短路发生在重新判定之前。

**非阻断两项**已同步:cap 测试改用 `MAX_DESIRED_WATCHES`(此前误用
`MAX_SELECTED_SECTIONS`,二者恰好同值 9 但概念不同);crate 文档"只暴露
readers"已改写。

**自查另发现两项资源边界缺口（本片未修，需裁定归属）**。PR5c.1 提交前
用五个独立镜头 + 完整性批评做了一轮对抗复核，六项 finding 全被独立
skeptic 推翻，但完整性批评提出两条**没有任何镜头检查过**的、经我实跑
复现的缺口：

- **T1（tombstone 无界）**：`desired = false` 命令按 §3.2 步 4 跳过全部
  准入，而步 5 无条件写入——因此对**从未 START 过**的任意合法 SectionKey
  提交 `basedOnRevision = 0` 的 STOP，会**创建**一条永久 authority 行，
  且该 section 从未经过 campus / term / 目录校验。**实跑**：600 次此类
  STOP（admission source 用 panicking 的 `never`）→ 600 条 tombstone、
  `revision_counter = 600`、`materialization_counter = 600`、admission
  **零次**被咨询。§2.5 冻结 tombstone 硬帽 **512**，本 crate 未实现、
  未上报、无任何 512 相关代码。更根本的是：即便没有这条路径，用户在一个
  学期内反复 START/STOP 不同 section 同样会无界增长 tombstone——**帽与
  rotation 才是真正缺的东西**，本条路径只是让它更快到达。
  该缺口直接影响 **G6**（"用**最大合法状态**证明 FULL ≤ 256 KiB / 16 块；
  证明失败即阻断 PR 合并"）：最大合法状态目前无界，G6 **不可证**。
  我在 PR5c 的披露里把这条路径说成"页面在 START 落库前取消时可达"，
  **那个理由只覆盖确实发过 START 的 section，而代码接受任意 section
  key**——措辞过窄，一并更正。
- **T2（receipt 无界）**：PR5c.1 把 receipt 从"每次成功一行"改成"每次
  **尝试**一行"，而在双页共同编辑模型里 `STALE_REVISION` 是输掉 CAS 的
  那一方的**正常**结果，因此拒绝至少和提交一样常见。**实跑**：3000 个
  不同 mutationId 全部被正确拒绝 → 3000 条 receipt、零 desired 行、
  无错误、无帽。§2.5 冻结 receipt 硬帽 **2048** 与 80% 触发
  （`floor(2048*4/5) = 1638`），本 crate 同样未实现。

**未在本片修复的理由**：§2.6 的 rotation 不是纯存储操作——它要求
"**保留 `actorIncarnation`**"且"**不得在同一化身内重置 `transitionId`**"，
后者是 actor 状态，不在本 crate 内。因此 rotation 跨 store 与 actor 两
层，应与 authority actor 同片落地。本片能做且已做的是**如实上报**；
建议裁定：(a) rotation + §2.5 两个硬帽 + 80% 触发（A-45 要求
`threshold-1` / `threshold` / `hard cap` 三点验收）归 **PR5d**；
(b) 顺带裁定 base-0 STOP 是否应当**拒绝创建新行**——按 §3.2 字面它应当
写入，我因此**没有**擅自改动，但它让未校验的 section key 进入 authority
状态，值得一条明确裁决。（**措辞更正**：初稿此处写"并上 bootstrap
wire"，不准确——当前 protocol-v1 兼容视图 `desired_watches()` 过滤
tombstone，只有**未来的** authority bootstrap / projection 才会携带它们。）

**Codex 裁定（2026-08-23，随 `65cfd07` 批准）**：

- **T1 已裁定：base-0 用户 STOP 必须创建 tombstone**，不得拒绝、不得变
  no-op。否则"START 尚未落库 → 用户取消 → 延迟 START 携 base 0 到达"会
  复活已取消的意图。任意 SectionKey 带来的增长**由资源帽与 rotation
  解决**，不由拒绝解决。系统 retirement 对 absent/tombstone 返回
  `NothingToRetire` **仍然正确**。→ 现有实现无需改动。
- **T2 已裁定：归 PR5d，且 PR5d 是 P1 硬门**——在**任何**生产 caller /
  desired 路由 / filter / owner materialization 之前必须完成：
  1. tombstone / receipt 硬帽 **512 / 2048**；
  2. 80% 阈值精确为 **409 / 1638**（`floor(512*4/5)` / `floor(2048*4/5)`），
     覆盖 `threshold-1` / `threshold` / `hard-cap` 三点；
  3. **存储层 fail-closed**：即使 actor 调和遗漏，第 **513 / 2049** 行也
     不得落库；
  4. **原子 rotation**：升 generation、重写全部 `desired = 1`、**仅清**
     tombstone 与 receipt、修复 counters、**保留 `actorIncarnation`**、
     **不重置同化身内 `transitionId`**；
  5. **G6**：最大合法状态证明 FULL ≤ 256 KiB / 16 块；
  6. PR6 原子激活或能力握手，以及 published + gate-pass 的 true-path
     打包 E2E。

  **若在 PR5d 之前出现任何生产可达入口，T1/T2 立即升级为阻断。**

四处**判别力实测**:
1. 去掉终局拒绝的 receipt 写入 → 四个测试同时 FAILED
   (`a_terminal_rejection_is_recorded` / `a_freed_slot_cannot_turn...` /
   `a_section_that_becomes_admissible...` / `every_recorded_answer_survives...`)。
2. 把 `Unavailable` 当作 admit 放行 → `an_unavailable_admission...` FAILED。
3. （沿用 PR5c）post-state 帽换成 pre-state → cap 测试 FAILED。
4. （沿用 PR5c）反转 epoch 保留条件 → policy 编辑测试 FAILED。

## 范围缩减（产品所有者 2026-08-23 裁定）

"像 B 站那样"= **多页面共同编辑同一份服务端状态**，B 站的语义是"另一个
页面操作后本页面需要刷新才看得到"，**不要求实时推送**。实现方此前读成
"共同编辑 + 实时推送"，据此建的帧流/分块/双 ACK/化身/听众登记/leader
选举**全部作废**。同轮确认：浏览器只是展示面，本地端关掉浏览器即无响铃
通道，不响铃是正确行为。

新设计见 `2026-08-23-desired-watch-reduced-scope.md`；
`2026-08-22-desired-watch-revision-cas.md` 标记 SUPERSEDED 并列明哪些节
仍有效、哪些作废。

**作废对已完成工作的影响**：
- PR5a / PR5c / PR5d（存储、CAS、receipt、上限、rotation）**全部保留**
  ——它们是"两页各自提交不能互相覆盖"本身要求的，与推送无关；
- **PR5b 的路由集合对本功能不再需要**：期望监控改走 HTTP，与 settings /
  selection 同一处注册，不触及共享 host 路由表。PR5b 中仍有价值的是
  S2c 的 64 KiB 帧上限（已惠及既有 watch 路由）与 S2b 的 404 钉死；
  路由集合本身保留在代码中但**本功能不使用**。
- S2-D3 关闭条件改为新设计 §9 的第 3–6 项。

## reduced S2/L1 纵向闭环（M0-M1-001，2026-08-24）

新设计 §9 的第 3–6 项由 `M0-M1-001` 一次交付，当前门如下。**这一节只
覆盖本地 desired 的读写与物化**；S2 的其余各层（自动重连、五环
Readiness、音频自愈、页面级通知）与 P1/P2 一条未做，不得因本节闭合而
认为 S2 收口。

**合同门**

1. CAS 只判 generation、receipt/fingerprint、based-on revision、9 门
   post-state 与两个资源预算；**写入期不查 Catalog/term/campus/Open**；
2. **服务端不代用户撤销意图**：永久装配失败保留 desired、暴露原因、
   停止重试；`retire_desired_watch` 与 `DesiredWatchAdmission` /
   `DesiredWatchRejection` / `Rejected` / `Unavailable` 全部删除；
3. base-0 STOP 仍写 tombstone；
4. HTTP 状态由**业务结果**决定，重放沿用**原结果**的状态码
   （`REPLAYED` 是布尔字段而不是 outcome）：committed 200；
   stale generation/revision、mutation-id 冲突、limit exceeded 409；
   authority full 503 且不写伪终局 receipt；协议错误 400/403；
5. GET 是**版本化、local-only、strict-key** 的读取投影，**不发 `armed`
   布尔**——四元组由前端派生；不分页、不截断、不丢 tombstone，最大
   合法状态（9 + 512 = 521 行）以**显式 256 KiB 预算**的序列化测试证明；
6. rotation 有明确 production owner：coordinator 在提交路径上触发。

**运行时门**

7. 首个页面接入 → 按 desired 装配；PUT 成功 → 先 reconcile 再回响应；
   最后一个页面离开 → 拆物理 watch、**保留 desired**；
8. 每 section 至多一份物理 watch；其告警扇出到全部页面；
9. 暂态失败有界退避（5/10/20 封顶 30s），每次重试带
   `(generation, revision, epoch)` 戳并在触发时重读 authority，戳不匹配
   即丢弃；
10. 本地 socket **fail-closed 拒绝** `START_WATCH` / `STOP_WATCH` /
    `UPDATE_POLICY`；六条 episode 命令路由到 owner 连接；**公网
    connection-scoped 行为不变**；
11. owner 连接不可由 wire 构造，心跳由维护 tick 续期，process stop 后
    遗忘并重开。

**前端门**

12. 加载读取权威状态；START/STOP/policy 走 PUT，带 UUIDv4 mutation id
    与当前 generation/revision；
13. 409 后**重新 GET**，不用缓存 revision 自动重放用户手势；
14. 只有四元组全等才显示"监控中"；读取失败**不保留旧绿态**；
15. 跨 tab 不实时同步；告警继续走既有 watch WebSocket。

**边界门**

16. 三个新 marker（`desired_watch` / `authority_generation` /
    `materialization_epoch`）进入公网 SOURCE 负控：
    PERSISTENT_ACTIVE_WATCH 13 → 16，全局 212 → **215**，
    `markerSetVersion` **2**，两个摘要重算；Rust 四表面与前端
    source/bundle 均有反例。**前端 shared 属于公网闭包**，共享层端口
    因此使用目标中立词汇；
17. desired 路径普通 GET 成功，WS upgrade **拿不到 101**；
    presence 路径未注入，仍钉死 404。

**打包门**

18. `packaging/windows/verify.ps1` 走**非空**循环：写入意图 → 重启 →
    同 generation 恢复且尚未物化（还没有页面 attach）→ **第一页面
    WebSocket attach → GET 报 materialized 且四元组全等、`activeWatchId`
    可寻址、`activeWatchCount` 为 1** → Full Reset 报删除 1 + 1、抬
    generation 且 `activeWatchCount` 归零 → 再重启为空且 generation 不
    回退。同型断言另有 `crates/bcsp-local-runtime/tests/local_runtime.rs`
    的三生命周期 HTTP 版本，随 `cargo test --workspace` 常跑；两侧共用
    `crates/bcsp-local-runtime/tests/support/mod.rs` 的确定性 PUBLISHED +
    S1 Gate-pass fixture，由 harness-free 的
    `tests/desired_watch_fixture.rs` 播种（测试产物，不入 12 文件
    archive，不访问真实 Rutgers）。`-FixtureSeederPath` 是**必填**参数：
    可选参数正是一道门悄悄不再被跑的方式，而"恢复但未物化"是比本门弱
    得多的结论，不接受作为通过；
19. 迁移 10004 一旦升级即**不可回滚**（旧二进制拒绝未知迁移），因此
    不得发布未闭合的中间构建。

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
- [~] S2-PR5——第一版按 fenced sequencer 实现，自查三镜头实跑复现 2 项
      P1（活跃/持久反向错位；重放 START 重新落库），**未提交即作废**；
      路线改判为 revision/CAS。设计 v1（e53f525）驳回 5 组阻断 → v2
      （d798f9d）驳回 4 组 → v3 自查 11 项 → v4（278fc9b）驳回 5 组 →
      v5 自查 15 项 → v6 驳回 3 组 → v7 驳回 5 组 → v7.1（fdf0f24）
      驳回 4 组 → **v7.2 窄修闭合**，见
      `2026-08-22-desired-watch-revision-cas.md`；待设计复审通过后实现。
- [x] S2-PR4（应用层心跳，56a9f70）——**Codex 批准**，清单第 6 条关闭
      （三项披露获追认；新增携带项 S2f/S2g 归 PR6；扫描 0 findings）
- [x] S2-PR3（L1 期望监控表，da5be13）——驳回 2 P1 后经 PR3.1 修复
      **Codex 批准 8103168**（B1 关闭；B2 → S2-D3 接线硬门 + PR5
      执行语义 e-i；扫描 0 findings）
- [x] S2-PR5a（authority 存储，3802926）——**Codex 批准为完全 dormant**；
      3 项非阻断（P2 ledger 竞态、P3 TS reset 合同、P3 counts 非一致快照）
- [x] S2-PR5a.1（f13ff83）——驳回 2 项 P2：`personal_table_counts()`
      自开事务破坏 `consistent_read()` 可组合性；双 opener 竞态测试
      对旧实现同样通过（barrier 位置错误）
- [x] S2-PR5a.2（1c12567）——**Codex 批准**（单条八 scalar 子查询 SELECT
      恢复可组合性；`PRE_LOCK_RENDEZVOUS` 使竞态测试成为回归判别器，
      去掉锁内重读即 FAILED；扫描 0 findings）。非阻断建议：若将来同进程
      新增迁移单测，把全局 rendezvous 按 DB 路径作用域化并用 RAII 清理。
- [x] S2-PR5b（host 受校验二级路由集合，a77d9e0）——驳回 1 P2 + 1 P3 后
      经 PR5b.1（3aa4935，**Codex 批准**）与 PR5b.2（b32cce4，**Codex
      批准**）闭合；**S2-PR5b 无遗留 finding**，三轮扫描均 0 findings。
      详见下节
- [~] S2-PR5c（CAS 写入器，72c6383）——驳回 1 P1 + 3 P2 + 2 非阻断；
      经 PR5c.1 修复，见下节；待复审

## M0-M1-001-R1：reduced S2/L1 纵向闭环集中修复

Codex 对 `a4f8d22..107f3e5` 的独立验收结论是 `CHANGES_REQUIRED`。方向被
接受，五组阻断都是"页面可能在事实之外显示绿色，或用户失去关掉它的办法"。
下表记录每一项由哪个判别测试关闭；每条测试都对修复前的实现失败。

| R1 项 | 缺口 | 关闭它的判别测试 |
|---|---|---|
| A3 | arm 成功后 Catalog 撤下/term 滚出/campus 离开产品，物理 watch 与 GET 仍绿 | `desired_watch.rs::a_section_that_leaves_the_catalog_after_arming_is_taken_down_and_reported`、`::a_target_that_stops_being_watchable_after_arming_is_taken_down_and_reported`、`watch_socket.rs::the_maintenance_sweep_stops_an_owner_held_watch_that_leaves_the_term_window` |
| A3 | watch 在 coordinator 背后结束（或同 section 换了新 id）仍报 materialized | `desired_watch.rs::a_watch_that_ended_underneath_the_coordinator_is_never_reported_as_running`、`watch_socket.rs::owner_watch_targets_report_the_identity_of_each_running_watch` |
| A3 | 暂态撤销后不能自愈；健康 watch 被复核重启 | `::a_transient_revocation_clears_the_green_light_and_recovers_by_itself`、`::repeated_maintenance_never_restarts_a_healthy_watch` |
| A4 | 只增长 receipt 的终局拒绝不触发 rotation，账本可填满后永久 503 | `::terminal_refusal_receipts_alone_rotate_the_authority_exactly_once`、`::a_restart_at_the_receipt_hard_cap_recovers_and_the_stop_finally_commits` |
| A4 | check/act 不在同一排他域，一次跨阈两次 rotation | `::concurrent_callers_crossing_one_threshold_rotate_once` |
| A4 | 触发 rotation 的响应同时携带新旧两套 generation/revision | `::a_response_that_triggered_a_rotation_reports_one_authority_state` |
| A4 | 重放的终局拒绝必须仍是拒绝 | `::a_replayed_terminal_refusal_is_still_the_refusal_it_was` |
| A5 | reset 在 SQLite 上等待时 attach/reconcile 可留下 orphan watch | `local_runtime.rs::a_full_reset_blocked_in_sqlite_leaves_no_orphan_watch`、`desired_watch.rs::the_reset_barrier_stops_every_physical_watch_including_one_it_never_armed` |
| A5 | `seal_and_stop` 之后 synthetic owner 可被重建；普通 reset 必须仍可用 | `watch_socket.rs::a_sealed_socket_refuses_to_rebuild_the_owner_but_an_ordinary_stop_does_not` |
| A6 | 旧 GET / 旧 PUT 覆盖新 tombstone | `local-desired-watch-integrity.test.tsx::never lets an older read put back intent a newer write removed`、`::never lets an older write put back a section a newer write removed` |
| A6 | 后台 retry 武装成功或 watch 意外停止后投影不刷新 | `::re-reads through the same domain when a retry finally arms the watch`、`::stops showing a watch as running after an unexpected stop` |
| A7 | 晚加入页面把仍 desired/运行的 section 移出唯一管理列表 | `::counts, describes and can act on a watch it never saw start`、`::fails closed while the authority is unreadable` |
| A7 | desired 与 selection 历史不一致时形成不可管理的孤儿 | `::shows and can stop a saved watch that is no longer in the selection` |
| A8 | `intent.ts` 的原始 NUL 让 Git/rg/安全清单看不见该文件 | `verify-import-graph.test.mjs::rejects a raw NUL byte in an active source file`、`::the shipped active sources contain no raw NUL bytes` |
| A8 | mutation 响应解码过松 | `::the mutation answer is decoded as strictly as the read`（8 条反例 + 2 条正例） |
| A8 | 拆除失败后永久 `pendingDisarm`；policy 编辑失败丢掉仍活 watch 的地址 | `desired_watch.rs::a_failed_teardown_keeps_the_watch_addressable_and_a_later_tick_finishes_it`、`::a_failed_policy_edit_keeps_the_running_watch_addressable`（经 `DesiredWatchOwner` fault seam） |
| A9 | 打包门只证明持久化/reset，重启后要求 `materialized = null` | `packaging/windows/verify.ps1` 的 attach + 四元组物化断言，与 `local_runtime.rs::desired_intent_survives_a_restart_and_a_full_reset_clears_it` 共用同一 fixture |

R1 引入的两处新机制值得单独记：`DESIRED_WATCH_REVALIDATE_INTERVAL` 让
"仍然可以监控吗"成为一个持续问题而不是 arm 时的一次性结论；
`DesiredWatchOwner` 是一个 seam，唯一目的是让两条**只有失败时才有意义**
的物理路径可被测试——健康的 socket 无法诱发它们，而它们正是会留下
"仍在响但已无法寻址"的那两条。
