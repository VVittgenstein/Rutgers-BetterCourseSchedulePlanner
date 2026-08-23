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
7. ~~leader tab 接管后从持久期望监控表重水合~~ **（v4 CAS 设计改写）**：
   监控由**服务端逻辑 owner** 持有，leader 转移**不触发任何 re-arm**；
   持久 desired 表是唯一真相，所有 tab 经 CAS 平权编辑，投影由服务端
   单一原子帧下发。leader 只决定谁播放声音。见
   `2026-08-22-desired-watch-revision-cas.md` §1、§5、§10。

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
