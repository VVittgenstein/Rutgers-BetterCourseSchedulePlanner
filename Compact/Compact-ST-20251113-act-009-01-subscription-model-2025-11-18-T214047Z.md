# Compact – ST-20251113-act-009-01-subscription-model

## 已确认事实
- 新增 `docs/subscription_model.md`，统一描述 sections / subscriptions / subscription_events 关系、字段与索引策略，为数据库与通知组件共享契约。（docs/subscription_model.md）
- 订阅偏好字段（notifyOn、maxNotifications、deliveryWindow、snoozeUntil、channelMetadata）及 `metadata` 中的 client/discord 信息均已定义默认值与扩展规则。（docs/subscription_model.md:41-55）
- 状态机（pending/active/paused/suppressed/unsubscribed）配合 Mermaid 图明确了状态含义与 transition，所有变更需记录在 `subscription_events`。（docs/subscription_model.md:86-116）
- `POST /api/subscribe`/`POST /api/unsubscribe` 的请求/响应、错误码、示例 JSON、幂等逻辑已固化；新增 `sectionResolved` 字段说明当 section 缺失时仍接受请求、存储 term/campus/index 并返回该标记，只有非法 term/campus 才 404。（docs/subscription_model.md:117-239）
- 安全/合规策略涵盖邮箱与 Discord 校验、perContact/perSection/perIP 限速、重复订阅处理、quiet hours、审计与 PII 清理要求。（docs/subscription_model.md:241-253）

## 接口与行为影响
- 所有消费订阅接口的前后端/worker 必须遵守文档中的 payload、响应字段（含 `sectionResolved`）、状态语义与错误码，否则与 API/DB 契约不一致。
- 通知/后台系统需解析 `metadata.preferences`、遵循状态机并持续写入 `subscription_events`，否则无法满足回溯与速率控制需求。

## 风险 / TODO
- 当前仅完成文档，尚无 schema 迁移或实现；后续 `ST-20251113-act-009-02` 等任务需将契约落地。
- rate limit/验证/quiet hours 尚未与实际配置或监控绑定，实现阶段需确认配置和可观察性。

## 自测
- 文档任务，未执行自动化测试。

## Code Review - ST-20251113-act-009-01-subscription-model - 2025-11-18T21:50:40Z
docs/subscription_model.md
Comment on lines +134 to +136


**Behavior**
- The API finds the `sections` row by `(term, campus, sectionIndex)`. If missing it returns `404` with error code `section_not_found`.
- `contact_hash` is computed server-side; the unique partial index guarantees idempotency. When a matching `pending/active` record exists the endpoint returns `200` with `existing: true` and never inserts a duplicate.


@chatgpt-codex-connector
chatgpt-codex-connector bot

P2 Badge Clarify behavior when section record missing

The subscribe contract here says the API must 404 when (term, campus, sectionIndex) is not found, but the earlier constraint section (lines 56-60) describes deferring section_id resolution when the section is absent so the subscription can be cleaned up later. These two statements contradict each other, leaving implementers unsure whether they should reject or accept requests when a section row is missing, which affects idempotency and retention behavior.
