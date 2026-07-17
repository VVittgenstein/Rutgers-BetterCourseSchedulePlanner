# P3 Open Poller QPS、Latency 与 Safety Model

## 1. Observed request facts

- 42 个两轮串行 Open GET 全部 HTTP 200。
- RTT：min 1,015.117 ms、median 1,195.597 ms、mean 1,241.159 ms、nearest-rank p95 1,501.020 ms、max 1,537.168 ms。
- 所有42个响应：`Cache-Control: max-age=30`。
- 没有 rate-limit header、公开 QPS allowance 或 SLA 证据。
- 低量样本没有 429，不是持续容量许可。

## 2. Approved 1-second policy 的请求上界

若 21 个证据 target 都每秒各请求一次：

```text
steady upstream QPS = 21
requests/day = 21 × 86,400 = 1,814,400
requests per 30-second cache freshness window = 630
```

即使只运行 Fall 15 campuses：

```text
steady upstream QPS = 15
requests/day = 1,296,000
```

平均 RTT 已超过 1 秒，因此“每 target 单飞且每秒完成一次”在当前观测 RTT 下自身就不成立；若强行保持每秒 start，会产生 overlap/concurrency。集中化只能阻止浏览器数量放大请求，不能消除 target 数的乘法。

## 3. Body equality 不能被当成稳定 coalescing contract

本轮 21 responses 只有 5 个 body clusters，看似可以按 term/family 合并，但：

- 官方没有声明这些 cluster 长期稳定；
- Fall CU 与其他 off-campus 行为不同；
- ETag 即使 body 相同也因 request 不同；
- 把 NB 响应当成全 term authoritative 会改变 target 与 failure isolation 语义。

因此 P3 不能仅凭本轮 hash 相等把 21 QPS 静默降为 2–5 QPS。

## 4. Cache implication

`max-age=30` 不等同于“禁止每秒请求”，但允许中间 cache 在 30 秒内复用响应，故：

- 1 秒 attempt 不证明 1 秒状态新鲜度；
- 1 秒请求可能在一个 freshness window 内重复取得相同 representation 30 次；
- 不应把 poll interval 当作 Rutgers change→browser audio latency；
- 若要声称一秒新鲜度，需要上游明确保证或可验证的 cache/revalidation contract，本轮没有。

## 5. Required scheduler guards

不论最终 cadence 如何，共享 poller 必须具备：

- 每 target single-flight；上一请求未完成时 coalesce/skip，不重叠；
- 浏览器reload、查询和watch数量不增加上游请求；
- attempt result 与 valid OpenObservation 分开计数；
- failure、unsafe empty、schema drift 不转状态；
- exponential backoff、Retry-After、429/5xx cooldown；
- bounded concurrency 与 per-origin request budget；
- complete/safe batch 才允许 absence-driven Closed；
- fake-upstream容量测试，不对 Rutgers 做压力测试。

## 6. Gate result

`FIXED_1_SECOND_UPSTREAM_SAFETY = REJECTED / SUPERSEDED BY USER DECISION`

当前证据既不能证明21-target持续1秒安全，也不能证明1秒新鲜度。用户随后批准public普通30秒、local默认30秒/范围3–3600秒、active-watch相关target10秒的双时钟；最终调度与安全合同见`23a/23b`。

## 7. Post-Review cache/latency correction

`max-age=30`只定义HTTP response的缓存新鲜期，不是Rutgers业务数据更新SLA。真实状态变化到通知必须拆为`U + C + P + B + F`：Rutgers内部发布延迟`U`未知，缓存/representation延迟`C`不由BCSP控制，poll相位`P`由effective interval决定，完整batch时间`B`尚待Round 2测量，只有valid observation后的fanout/audio `F`可由本产品冻结为≤1秒目标。

因此，“真实状态变化后严格30秒内通知”不可承诺；10秒已冻结为active-watch target fast lane，30秒为两包普通refresh默认。真实origin并发初始为1，饱和时必须显示actual interval与lag；见`19`与`23`。
