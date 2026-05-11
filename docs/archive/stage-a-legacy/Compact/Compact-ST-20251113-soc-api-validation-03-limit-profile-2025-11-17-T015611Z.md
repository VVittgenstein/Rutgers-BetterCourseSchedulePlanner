### Subtask
- ID: ST-20251113-soc-api-validation-03-limit-profile — Rutgers SOC rate-limit & error-code profile.

### 已落实事实
- 新增 `scripts/soc_api_client.ts`，封装 `decodeSemester`、`performProbe`、`SOCRequestError` 与统一 retry hint/structured error 输出；`scripts/soc_probe.ts` 现完全复用此模块，CLI 行为和参数保持一致但具备更完整的错误提示。
- 引入 `scripts/soc_rate_limit.ts` + npm 脚本 `soc:rate-limit`，支持对 `courses.json` / `openSections` 进行自定义并发与间隔的批量压测，记录每个场景的 2xx/4xx/5xx/timeout/network/json 计数、平均/95 分位时延，并可输出 JSON 结果（见 docs/soc_rate_limit*.json）。
- 在 New Brunswick 12024 学期跑通 baseline（1×1200ms、3×600ms、6×300ms）与多组 stress 场景（至 32 worker / 50ms gap 以及 openSections 50 worker 无间隔），所有请求均返回 2xx；原始数据保存于 `docs/soc_rate_limit.latest.json`, `docs/soc_rate_limit.courses_stress*.json`, `docs/soc_rate_limit.openSections_blitz.json` 等文件。
- `docs/soc_rate_limit.md` 整理压测方法、表格化的并发/间隔 vs 实测 RPS 与延迟、推荐的全量/增量抓取与 openSections 轮询频率、以及 429/5xx/timeout 等错误码的处理建议。
- 根据 code review，`scripts/soc_rate_limit.ts` 现对 interval=0 的场景返回有限的 `estimatedRps`，并新增 `actualRps` 字段与打印，存档 JSON 不再出现 `null` 吞掉的信息。
- `record.json` 将该 Subtask 标记为 `done`，同时把 `docs/soc_rate_limit.md` 记为产出；满足“形成限流策略与错误码动作表”的验收要求。

### 接口 / 行为变更
- CLI：`npm run soc:rate-limit -- [flags]` 成为正式入口，对后续抓取/通知服务是新的内部工具；`scripts/soc_probe.ts` 的错误输出格式稍变（来自共享 client）。
- 文档：`docs/soc_rate_limit.md` 定义的推荐频率、回退策略将影响数据抓取与通知轮询的配置参数。

### 限制 / 风险 / TODO
- 压测仅覆盖 term=12024、campus=NB；其他校区/学期可能拥有不同 payload 大小与带宽瓶颈，需要未来补充数据以验证假设。
- 未在实测中触发 429/503 等错误，相关处理策略基于历史经验而非本次验证；仍需在生产监控中观察是否出现更严格的限流规则。
- Stress 结果显示 `courses.json` 受带宽限制而非请求数限制；若部署环境网络/CPU 更弱，需要重新校准推荐并发度。

### 自测
- `npm run soc:probe -- --term 12024 --campus NB --subject 198 --samples 1`
- `npm run soc:rate-limit -- --term 12024 --campus NB --subject 198 --endpoint both --schedule 1:1200,3:600,6:300 --iterations 20 --rest 4000 --output docs/soc_rate_limit.latest.json --label "2025-11-16 NB baseline"`
- `npm run soc:rate-limit -- --term 12024 --campus NB --endpoint courses --schedule 8:200,12:150 --iterations 32 --rest 3000 --label stress --output docs/soc_rate_limit.courses_stress.json`
- `npm run soc:rate-limit -- --term 12024 --campus NB --endpoint courses --schedule 16:100,32:50 --iterations 40 --rest 3000 --label stress2 --output docs/soc_rate_limit.courses_stress2.json`
- `npm run soc:rate-limit -- --term 12024 --campus NB --endpoint openSections --schedule 20:0 --iterations 120 --rest 2000 --label "openSections blitz" --output docs/soc_rate_limit.openSections_blitz.json`
- `npm run soc:rate-limit -- --term 12024 --campus NB --endpoint openSections --schedule 50:0 --iterations 500 --rest 0`
- `npm run soc:rate-limit -- --term 12024 --campus NB --endpoint openSections --schedule 3:0 --iterations 3 --rest 0`

## Code Review - ST-20251113-soc-api-validation-03-limit-profile - 2025-11-17T02:03:33Z

💡 Codex Review
Here are some automated review suggestions for this pull request.

ℹ️ About Codex in GitHub
scripts/soc_rate_limit.ts
Comment on lines +393 to +396
function estimateRps(scenario: ScenarioSpec): number {
  if (scenario.intervalMs === 0) {
    return Infinity;
  }
@chatgpt-codex-connector
chatgpt-codex-connector bot
now
P2 Badge Prevent null estimatedRps for zero-interval scenarios

When a scenario uses intervalMs of 0 (allowed by parseSchedule, used in the blitz example), estimateRps returns Infinity. JSON serialization turns Infinity into null, so saved rate-limit payloads misreport throughput (docs/soc_rate_limit.openSections_blitz.json shows estimatedRps: null). Use a finite calculation for zero-interval runs (e.g., based on duration) or avoid serializing Infinity so zero-interval profiles retain a meaningful RPS value.

## Code Review - ST-20251113-soc-api-validation-03-limit-profile - 2025-11-17T02:11:00Z

Codex Review: Didn't find any major issues. Already looking forward to the next dif
