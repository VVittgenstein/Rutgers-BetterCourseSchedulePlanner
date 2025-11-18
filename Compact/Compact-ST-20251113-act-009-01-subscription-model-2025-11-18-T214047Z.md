# Compact – ST-20251113-act-009-01-subscription-model

## 已确认事实
- 新增 `docs/subscription_model.md`，统一描述 sections / subscriptions / subscription_events 关系、字段、索引与偏好 JSON 结构，作为数据库和通知模块的共享契约。（docs/subscription_model.md）
- 订阅偏好（notifyOn、maxNotifications、deliveryWindow、snoozeUntil、channelMetadata）及 `metadata` 中的 client/discord 信息的默认值、扩展规则已定义。（docs/subscription_model.md:41-55）
- 状态机（pending/active/paused/suppressed/unsubscribed）和 Mermaid 图明确了状态语义，所有状态变更、通知与错误需写入 `subscription_events` 保障追踪。（docs/subscription_model.md:86-116）
- `POST /api/subscribe`/`POST /api/unsubscribe` 的请求/响应字段、幂等语义、错误码及示例 JSON 已固化；`sectionResolved` 与错误码说明：合法 term/campus 但 section 缺失时返回 `200 + sectionResolved=false`，仅 term/campus 无效时报 `404 section_not_found`。（docs/subscription_model.md:117-239）
- 安全/合规策略涵盖邮箱/Discord 校验、perContact/perSection/perIP 限速、重复订阅防护、quiet hours、审计日志和 PII 清理要求。（docs/subscription_model.md:241-253）
- 新增针对 section 缺失场景的幂等方案：文档要求实现 `(term_id, campus_code, index_number, contact_hash, contact_type)` 层的部分唯一索引或哈希，并在 Behavior 段明确 `unresolvedKey` 以重用已有记录，避免重复订阅。（docs/subscription_model.md:52-60, 117-143）

## 接口与行为影响
- 所有消费订阅 API 的前端、后端、worker 必须遵循文档中的 payload、响应（含 `sectionResolved`）和错误码，保持与 SQLite 契约一致。
- 通知/后台系统需解析 `metadata.preferences` 并遵守状态机写入 `subscription_events`，否则无法满足追踪和限速要求。
- 订阅接口实现者必须支持 unresolved sections 的唯一约束/哈希，否则无法保证幂等。

## 风险 / TODO
- 文档尚未配套 schema 迁移或代码实现；需要 `ST-20251113-act-009-02` 等后续任务落地。
- rate limit、验证、quiet hours 仍需与真实配置/可观察性整合。

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

## Code Review - ST-20251113-act-009-01-subscription-model - 2025-11-18T22:21:11Z
BetterCourseSchedulePlanner/docs/subscription_model.md

Lines 134 to 136 in 033bcfc



 **Behavior** 
 - The API looks up the `sections` row by `(term, campus, sectionIndex)`. When found the row's `section_id` populates the subscription immediately. When missing (e.g., data lag or removed section) the API still accepts the request, stores the denormalized `term/campus/index` with `section_id = null`, and flags the response with `sectionResolved: false` so callers know the join is deferred. Only malformed `term`/`campus` combinations return `404 section_not_found`. 
 - `contact_hash` is computed server-side; the unique partial index guarantees idempotency. When a matching `pending/active` record exists the endpoint returns `200` with `existing: true` and never inserts a duplicate. 



P1 Badge Preserve idempotency when section data is missing
With the new behavior at subscribe time the API now accepts requests even when (term, campus, sectionIndex) is missing and saves the row with section_id = null, but the only dedupe described (idx_subscriptions_active_unique on section_id, contact_hash, contact_type) ignores NULL keys, so repeated calls during ingestion gaps can insert multiple rows even though this paragraph claims the unique index guarantees idempotency. When the section record eventually exists, backfilling section_id will either create duplicate notifications or fail on the unique index, so the contract needs an explicit dedupe key for unresolved sections or a different error path.
