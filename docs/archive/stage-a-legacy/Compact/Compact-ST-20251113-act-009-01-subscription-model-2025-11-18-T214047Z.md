# Compact – ST-20251113-act-009-01-subscription-model

## 已确认事实
- `docs/subscription_model.md` 新增并统一描述 sections / subscriptions / subscription_events 关系、字段、索引及偏好 JSON 结构，作为数据库与通知模块公共契约。（docs/subscription_model.md）
- 订阅偏好（notifyOn、maxNotifications、deliveryWindow、snoozeUntil、channelMetadata）及 `metadata` 中 client/discord 信息的默认值与扩展规则已明确。（docs/subscription_model.md:41-55）
- 状态机（pending/active/paused/suppressed/unsubscribed）及 Mermaid 图定义了状态语义，所有状态变更、通知、错误需写入 `subscription_events`。（docs/subscription_model.md:86-116）
- `POST /api/subscribe`/`POST /api/unsubscribe` 的请求/响应字段、幂等语义、错误码及示例 JSON 已固化；`sectionResolved` 与 `404 section_not_found` 的含义明确：合法 term/campus 但缺 section 时返回 `200 + sectionResolved=false`，仅 term/campus 无效时报 404。（docs/subscription_model.md:117-239）
- 针对第一轮 code review 的“section 缺失是否 404”疑问，文档现已在 Behavior 与 Error codes 段落一致说明：缺 section 但 term/campus 合法时接受并回传 `sectionResolved=false`，仅非法 term/campus 才返回 404，消除了实现歧义。（docs/subscription_model.md:132-143, 191-195）
- 安全/合规策略覆盖邮箱/Discord 校验、perContact/perSection/perIP 限速、重复订阅防护、quiet hours、审计日志与 PII 清理。（docs/subscription_model.md:241-253）
- 针对 section 缺失场景新增幂等策略：文档要求 `(term_id, campus_code, index_number, contact_hash, contact_type)` 级别的部分唯一索引/哈希，并在 Behavior 中定义 `unresolvedKey`（含 contact 维度）以复用已有记录，确保多用户在数据缺口期仍能独立订阅。（docs/subscription_model.md:52-60, 133-143）

## 接口与行为影响
- 所有订阅 API 消费方需遵守文档中的 payload、响应（含 `sectionResolved`）、错误码以保持契约一致。
- 通知/后台系统必须解析 `metadata.preferences`、遵循状态机并写入 `subscription_events`，否则无法满足追踪和限速需求。
- 订阅实现必须支持 unresolved sections 的 per-contact 去重策略，避免 ingestion gap 期间重复或丢失订阅。

## 风险 / TODO
- 目前仅有文档，尚未提供 schema 迁移或实现；需 `ST-20251113-act-009-02` 等任务落地。
- rate limit、验证、quiet hours 仍需与实际配置/监控对齐。

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

## Code Review - ST-20251113-act-009-01-subscription-model - 2025-11-18T22:31:33Z
docs/subscription_model.md
Comment on lines 136 to +137


**Behavior**
- The API looks up the `sections` row by `(term, campus, sectionIndex)`. When found the row's `section_id` populates the subscription immediately. When missing (e.g., data lag or removed section) the API still accepts the request, stores the denormalized `term/campus/index` with `section_id = null`, and flags the response with `sectionResolved: false` so callers know the join is deferred. Only malformed `term`/`campus` combinations return `404 section_not_found`.
- The API looks up the `sections` row by `(term, campus, sectionIndex)`. When found the row's `section_id` populates the subscription immediately. When missing (e.g., data lag or removed section) the API still accepts the request, stores the denormalized `term/campus/index` with `section_id = null`, derives an internal `unresolvedKey = sha1(term|campus|index)` (or equivalent tuple) for duplicate detection, and flags the response with `sectionResolved: false` so callers know the join is deferred. Only malformed `term`/`campus` combinations return `404 section_not_found`.


@chatgpt-codex-connector
chatgpt-codex-connector bot

P1 Badge Keep contact dimension in unresolved dedupe key

In docs/subscription_model.md lines 136‑137 the subscribe flow now says unresolved sections are deduped via unresolvedKey = sha1(term|campus|index), which drops contact_hash/contact_type. During ingestion gaps, two different users subscribing to the same section would collide on that key and the second call would be treated as an existing row instead of creating their own subscription, contrary to the per-contact uniqueness described in the index list above (lines 56‑62). The unresolved dedupe must include the contact dimensions, otherwise legitimate subscribers are blocked whenever the section record is missing.

## Code Review - ST-20251113-act-009-01-subscription-model - 2025-11-18T22:40:58Z
docs/subscription_model.md
Comment on lines 136 to +137


**Behavior**
- The API looks up the `sections` row by `(term, campus, sectionIndex)`. When found the row's `section_id` populates the subscription immediately. When missing (e.g., data lag or removed section) the API still accepts the request, stores the denormalized `term/campus/index` with `section_id = null`, and flags the response with `sectionResolved: false` so callers know the join is deferred. Only malformed `term`/`campus` combinations return `404 section_not_found`.
- The API looks up the `sections` row by `(term, campus, sectionIndex)`. When found the row's `section_id` populates the subscription immediately. When missing (e.g., data lag or removed section) the API still accepts the request, stores the denormalized `term/campus/index` with `section_id = null`, derives an internal `unresolvedKey = sha1(term|campus|index|contact_hash|contact_type)` (or hashes the tuple `(term, campus, index, contact_hash, contact_type)`) for duplicate detection, and flags the response with `sectionResolved: false` so callers know the join is deferred. Only malformed `term`/`campus` combinations return `404 section_not_found`.


@chatgpt-codex-connector
chatgpt-codex-connector bot

P1 Badge Keep contact dimension in unresolved dedupe key

In docs/subscription_model.md lines 136‑137 the subscribe flow now says unresolved sections are deduped via unresolvedKey = sha1(term|campus|index|contact_hash|contact_type) which is consistent with the index list. No further action required.

## Code Review - ST-20251113-act-009-01-subscription-model - 2025-11-18T22:41:36Z
Codex Review: Didn't find any major issues. Can't wait for the next one!
