# P3 Open Cache 与通知延迟合同

## 1. 结论先行

`Cache-Control: max-age=30` 不是 Rutgers 数据更新 SLA，也不表示 API 必定每 30 秒产生新状态。它只声明缓存可把该 HTTP response 视为 fresh 的时间。P3 因此不能承诺“真实座位状态变化后严格 30 秒内通知”。

当前可以冻结为可验证目标的是：

- BCSP 接受一个新的 valid Open batch 后，完成 reconcile、WebSocket fanout 与开始 WebUI audio 的目标不超过 1 秒；
- “Rutgers 真实状态变化 → Rutgers API 可见”的延迟属于未知上游时间；
- “Rutgers API 已暴露新快照 → BCSP 首次观察并通知”只能设为 best-effort/P95 目标，不能写成 Rutgers 保证；
- UI 文案使用“BCSP 首次观察到 Open”，不能声称准确知道课程实际开放时间。

## 2. 已观察 HTTP 事实

P3 第一轮 21 个 Open responses：

- 全部 `Cache-Control: max-age=30`；
- 有 `Date` 和 `ETag`；
- `Age` 与 `Last-Modified` 均未观察到；
- observed RTT：min 1.033s、median 1.164s、mean 1.216s、p95 1.433s、max 1.439s。

`Age` 缺失没有显示明确的“未经验证缓存复用”，但也不能证明 Rutgers 业务状态实时。`Date` 是 HTTP message 时间，不是 section 状态改变时间；`ETag` 是精确 URI representation 的 opaque validator，不能跨 campus/URI 比较，也不能证明内部刷新周期。

## 3. 延迟分解

真实状态变化到声音的延迟应写成：

```text
D_true = U + C + P + B + F
```

- `U`：真实状态变化到 Rutgers API origin 可见；当前无 SLA，未知且无可证上界。
- `C`：缓存/已发布 representation 的剩余新鲜时间；`max-age=30` 允许正常缓存路径复用约 30 秒，但不是固定延迟。
- `P`：BCSP 等待下一次目标调度的相位延迟，最多约一个 effective interval。
- `B`：完整 term/campus batch 的 HTTP、校验与合并时间；不是单请求 RTT。
- `F`：BCSP 接受 valid batch 后 reconcile/fanout/audio；工程目标 ≤1 秒。

即使暂时只用 observed 单请求 p95 1.433 秒代替尚未实测的 batch duration，并忽略未知 `U`：

| Effective poll | 正常30秒cache预算 + poll + 单请求p95 + fanout目标 | 能否作为真实变化硬保证 |
|---:|---:|---|
| 3s | 35.433s以上 | 不能 |
| 10s | 42.433s以上 | 不能 |
| 30s | 62.433s以上 | 不能 |

这些是风险预算示例，不是实际延迟预测；完整 batch、失败、serve-stale许可和未知 `U` 只会扩大不确定性。

## 4. 已批准的两时钟设计

用户已于 2026-07-13 确认：

1. 普通 Open refresh：public固定30秒；local默认30秒、可配3–3600秒。
2. Active watch fast lane：相关共享 batch 目标10秒。
3. `effective_interval = min(general_interval, 10)`；本地用户设3秒时仍是3秒。
4. 同 batch single-flight；普通与watch lane同时到期只产生一次上游batch。
5. 公网跨所有用户共享一个batch；watch数量不放大Rutgers请求。
6. 不做missed-tick追赶；前次未完成则coalesce/skip，避免重叠和补跑风暴。
7. failure、unsafe empty、partial batch或无法确认shape时保留last-known-good，不允许absence→Closed。

该设计可以把“端点已经暴露新快照后的发现”通常压到约一个10秒调度周期加batch时间，但 `max-age=30` 和未知 `U` 仍使严格30秒端到端保证不可成立。

## 5. UI 与审计字段

至少显示或记录：

- requested general interval；
- effective interval与触发lane；
- batch started/completed、完整性与target失败；
- last checked、last valid observation、last new body observed；
- response `Date`、`Age`、`ETag`、body hash与计算得到的apparent/current age；
- observation→fanout/audio latency；
- latest failure与last-known-good age。

不得用“每3/10/30秒更新”掩盖实际effective interval、batch未完成或重复cache representation。

## 6. 进一步证据

用户已确认时钟映射；仍须由独立冻结的新 amendment 授权后，才可执行：

- 原第二轮的低量跨轮变化观察；
- 同一精确URI的少量条件请求，观察`If-None-Match`、304、`Date`与`Age`行为；
- 不使用cache-busting query，不用burst/压力请求制造变化；
- 该测试仍不能诱发真实seat change，也不能证明Rutgers内部`U`上界。

HTTP语义依据：[RFC 9111 HTTP Caching](https://www.rfc-editor.org/rfc/rfc9111.html) 与 [RFC 9110 HTTP Semantics](https://www.rfc-editor.org/rfc/rfc9110.html)。
