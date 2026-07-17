# P3 Open Request Stop 001 — Same-scope Join Conflict

## Status

- `FROZEN_SEMANTIC_STOP_AFTER_ROUND_1`
- Stopped at: `2026-07-13T11:04:53+08:00`
- Open manifest SHA-256: `707705756EEA4D269EDD1822F8529453B1E8C7838E23B1FC843789379B947703`
- Ledger-at-stop SHA-256: `A6FEF873ABFC511A0BADB71463B772194204015955323D8BB5007AEE28DED9CC`
- Attempts: 21/42, all first-round requests; 21 HTTP 200 successes, 0 HTTP failures
- Round 2: cancelled before its first request

## Why acquisition stopped

P2 的硬门要求每个 `openSections` 返回 index 在同一请求 `(term,campus)` Catalog scope 内恰好 join 一个规范化 Section。第一轮已在多个 scope 明确否定该假设：

| Scope | Open indexes | Same-scope matches | Same-scope orphans |
|---|---:|---:|---:|
| 92026/NB | 12,645 | 9,055 | 3,590 |
| 92026/NK | 12,645 | 2,101 | 10,544 |
| 92026/CM | 12,645 | 1,389 | 11,256 |
| 92026/B | 12,645 | 0 | 12,645 |
| 92026/D | 12,645 | 3 | 12,642 |
| 72026/NB | 2,069 | 1,338 | 731 |
| 72026/NK | 2,069 | 520 | 1,549 |

Fall NB/NK/CM 以及多个 off-campus 参数返回同一 body hash；Fall 三个 ONLINE 参数也返回相同的 1,099-index subset。Summer 同样出现 main 三 campus 同 body、online 三 campus 同 subset。`campus=CU` 是 observed empty，不能据此把其他 target 的语义统一解释为同一种行为。

继续第二轮只能观察重复/变化，不能修复已经确定的 same-scope join contract 冲突。依据用户“证据冲突即停”，停止优先于完成 42 次预算。

## Additional observed constraint

所有 21 个响应均为 `Cache-Control: max-age=30`。这不单独证明禁止 1 秒 attempt，但意味着低量样本无法证明一秒状态新鲜度或 21-target 持续 QPS 安全；不得把“未遇到 429”写成容量许可。

## Consequence

P3 不能完成本地一键包计划，P4/P5/P6 均不得启动。需要用户 Review 决定是否把 Open contract 改为：

- 将返回数组视为某种 term/campus-family superset，只对当前 Catalog target 的 intersection 更新状态并审计 orphans；或
- 发现另一个能返回 scope-qualified status 的官方来源；或
- 重新定义 Open target/scope 与刷新策略。

在 Review 前不得默认采用第一种，因为这会修改已批准的 same-scope exact-join 硬门，并牵动 absence=Closed、empty safety、QPS 与复合键语义。
