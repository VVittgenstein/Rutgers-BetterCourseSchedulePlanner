# Compact – ST-20251113-act-009-01-subscription-model

## 已确认事实
- 新增 `docs/subscription_model.md`，统一描述 sections / subscriptions / subscription_events 关系、字段、索引及偏好 JSON 结构，作为数据库与通知组件的共享契约。（docs/subscription_model.md）
- 订阅偏好（notifyOn、maxNotifications、deliveryWindow、snoozeUntil、channelMetadata）及 `metadata` 中 client/discord 信息的默认值、扩展规则已定义。（docs/subscription_model.md:41-55）
- 状态机（pending/active/paused/suppressed/unsubscribed）与 Mermaid 图明确了状态语义，所有状态变更、通知、错误都要写入 `subscription_events` 以供追踪。（docs/subscription_model.md:86-116）
- `POST /api/subscribe`/`POST /api/unsubscribe` 的请求/响应字段、幂等语义、错误码与示例 JSON 均已固化；`sectionResolved` 字段和 404 语义明确：term/campus 合法但 section 缺失时返回 `200 + sectionResolved=false`，仅当 term/campus 无效才 `404 section_not_found`。（docs/subscription_model.md:117-239）
- 安全/合规策略包括邮箱与 Discord 校验、perContact/perSection/perIP 限速、重复订阅防护、quiet hours、审计日志以及 PII 清理指引。（docs/subscription_model.md:241-253）

## 接口与行为影响
- 所有消费订阅 API 的前端、后端、worker 必须遵循文档定义的 payload、响应（含 `sectionResolved`）与错误码，确保与 SQLite 契约一致。
- 通知/后台系统需读取 `metadata.preferences` 并严格按照状态机写入 `subscription_events`，否则无法满足回溯、幂等和速率控制需求。

## 风险 / TODO
- 当前仅有文档，无 schema 迁移或实现；需由 `ST-20251113-act-009-02` 等后续任务落地 API、DB 与 worker。
- rate limit、验证、quiet hours 仍未与实际配置/监控集成，实施阶段需要确认配置来源与可观察性。

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

## Code Review - ST-20251113-act-009-01-subscription-model - 2025-11-18T22:10:03Z
docs/subscription_model.md
Comment on lines 134 to 136


**Behavior**
- The API finds the `sections` row by `(term, campus, sectionIndex)`. If missing it returns `404` with error code `section_not_found`.
- The API looks up the `sections` row by `(term, campus, sectionIndex)`. When found the row's `section_id` populates the subscription immediately. When missing (e.g., data lag or removed section) the API still accepts the request, stores the denormalized `term/campus/index` with `section_id = null`, and flags the response with `sectionResolved: false` so callers know the join is deferred. Only malformed `term`/`campus` combinations return `404 section_not_found`.
- `contact_hash` is computed server-side; the unique partial index guarantees idempotency. When a matching `pending/active` record exists the endpoint returns `200` with `existing: true` and never inserts a duplicate.


@chatgpt-codex-connector
chatgpt-codex-connector bot

P2 Badge Response semantics contradict error codes

The new behavior now says missing (term, campus, sectionIndex) rows should still be accepted and marked sectionResolved: false, with 404 only for malformed term/campus values, but the error-code list later in the same doc still claims a 404 when a section row is not present (lines 191‑195). This contradiction leaves API implementers unsure whether to reject or accept a request when the section hasn’t been ingested, which affects both idempotency and client UX.
