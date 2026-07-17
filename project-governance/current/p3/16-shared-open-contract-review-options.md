# P3 Shared Open Contract — Review Options

> **Post-Review resolution（2026-07-13）**：`JOIN A`与双时钟均获用户批准；Round 2已42/42完成。当前权威合同见`18`–`23`，以下选项仅保留为作出决定时的历史审计背景。

## 1. 必须由用户裁决的两个冲突

### JOIN-REV-01

已批准硬门要求每个 Open index same-scope exact join；真实证据与当前官方 SOC UI 都表明 Open arrays 被合并后由 Catalog Sections 做 membership lookup，额外值被忽略。

### RATE-REV-01

已批准 local default/public fixed Open cadence 为 1 秒且无watch仍对全部service scope运行；21-target模型为21 QPS/181万次每天，而所有响应允许30秒fresh cache，且没有上游容量许可。

## 2. JOIN 选择

### A. 批准 merged-set intersection（建议）

共享 contract 改为：

1. 每个 term 的已批准 target batch 分别请求；
2. 只有 batch 中所有 required requests 都 valid/safe 时，合并 raw Open string sets；
3. 遍历该 term 已加载的 Catalog Sections，以 index membership 更新每个 `(term,campus,index)` scoped copy；
4. Open set 中无法解释的值不报错中止，记录 orphan count/sample/hash并忽略；
5. presence→Open；absence 只有在 complete safe batch 中才可→Closed；任一失败/unsafe empty 保留 last-known；
6. 外部 Section key 仍是 `(term,campus,index)`，不因 membership lookup 改回裸 index。

这与官方 UI 的数据流最接近，但属于对 P2 hard gate 的明确修订，不能由 Agent自行采用。

### B. 保留 exact join，寻找其他官方来源

在找到返回 term/campus-qualified status 的可靠来源前，Open filter/watch/audio 不可实现，P3继续阻塞。

### C. 保留 exact join 并移除当前 Open能力

这会改变已批准核心产品目标，不建议，且需重新审查筛选、watch、audio、history和refresh合同。

## 3. RATE 选择

### A. 上游 fetch 30秒；新 observation 后1秒内fanout/audio（建议）

- local default Open fetch 改为30秒；用户后续已把配置范围修订为`3–3600`；
- public fixed Open fetch 改为30秒；
- “1秒内”改为从服务获得新的 valid OpenObservation 到WebUI提醒的目标，而不是Rutgers原始状态变化到提醒；
- 该选择与 observed `max-age=30` 一致，但仍不构成Rutgers SLA。

### B. 保留1秒，但等待 Rutgers 明确容量/缓存许可

在取得可引用的上游保证前P3保持阻塞；不能用本轮21次200响应代替许可。

### C. 仅active scope/watch 1秒

降低请求量，但会修改“无watch仍刷新全部已批准scope”合同，并使course card/filter状态不持续更新；需单独产品裁决。

## 4. 若批准建议组合

若批准 `JOIN A + RATE A`，后续需要：

1. 新冻结 amendment，允许在修订contract下完成第二轮低量证据；
2. 增加 complete-batch/partial-failure/unsafe-empty fixtures；
3. 重写 P3 `07` 的 Open scheduler、history/watch/audio 前置计划；
4. 重跑 traceability 与P3 gate；
5. P3通过后再进入只做public/deployment delta的P4。
