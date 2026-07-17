# P3 Shared Open Evidence Request Manifest

## 1. Frozen status

- Machine manifest: `10a-open-request-manifest.json`
- SHA-256: `707705756EEA4D269EDD1822F8529453B1E8C7838E23B1FC843789379B947703`
- Status: `FROZEN_BEFORE_FIRST_OPEN_REQUEST`
- Frozen at: `2026-07-13T10:59:53+08:00`
- Endpoint: `GET https://classes.rutgers.edu/soc/api/openSections.json?year={year}&term={term}&campus={campus}`
- Authentication/cookies: none

## 2. Why this is P3

真实 Open shape、same-scope join、empty/error 与调度安全是本地一键包完整计划的基础，不是公网包才需要的 delta。P3 必须先关闭这套 `BASELINE_SHARED` 证据；P4 继承它并只设计公网包/部署差异，不再重复请求。

## 3. Exact scope

Open 复用 P3 当前 21 个成功 Catalog scopes：

- Fall 2026：全部当前 15 campus，包括 amendment-002 新增的 `D`；
- Summer 2026：6 个 main/online 结构样本；
- Catalog scope digest：`09F91D594F183A26FAC1D546C03D5A68557DD104FB06A66E783D884E59D7E4F7`；
- `04b` SHA-256：`B08B41AA7B43D362D9C665C3BC67BDFBF07DCA138FFA09F5FF1624A17CA53624`；
- P3 Open 阶段允许的新 Catalog 请求数：`0`。

不扩成两个 term × 15 campus。每个 Open scope 在机器 manifest 中携带对应 Catalog request ID 与 body SHA-256，join 必须只使用这一份缓存。

## 4. Observation plan

- 两轮，每轮每 scope 一次，共 42 个精确 request ID。
- concurrency=`1`；任意 attempt start 间隔至少 5 秒。
- 第二轮只能在第一轮最后一次完成至少 60 秒后开始，因此同一 scope 两次观测间隔必然大于 60 秒。
- timeout=30 秒；automatic retry=`0`；hard attempt limit=`42`。
- 单 decoded body 最大 10 MiB。
- 保存 decoded raw、wire/decoded bytes 与 SHA-256、允许的响应 headers、root element type 分布、raw duplicate 数；normalize 前不得 String()、去重或排序。

## 5. Terminal stop

以下任一发生即记录并停止，剩余请求不得继续：403、429、任一非 2xx、off-origin redirect、非 JSON content type、JSON parse failure、root 非 array、response 超限、timeout/network failure、ledger/raw/hash 不一致。

不得发送无效参数、burst 或压力请求来制造 429/5xx/HTML。未自然观察到的错误分支只能用本地 fixture/injection 验证。

## 6. Interpretation limits

- 两轮低量串行观测可以证明当前 shape、join 与跨轮变化，不证明持续固定 1 秒容量。
- 没遇到 429 不能写成上游允许 21 QPS。
- HTTP 200 empty 只记录为 observed empty；在 P3 明确冻结 absence/empty 规则前，不得把现有 Open 全部转 Closed。
- 固定 1 秒策略必须另外计算 target/QPS 上界并给出 single-flight/coalescing/cooldown 与 P7 fake-upstream 容量验证；关键安全结论仍为 UNKNOWN 时按停止门回 Review。
