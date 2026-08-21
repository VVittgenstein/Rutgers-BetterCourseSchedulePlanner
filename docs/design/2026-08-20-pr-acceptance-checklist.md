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

## 通用

9. 每个 PR 的验收段引用本清单对应条目编号；条目完成后在本文件勾除。
