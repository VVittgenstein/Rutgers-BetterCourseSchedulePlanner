# Compact – ST-20251113-act-009-01-subscription-model

## 已确认事实
- 新增 `docs/subscription_model.md`，集中定义 sections/subscriptions/subscription_events 关系、字段约束、状态机、偏好 JSON 结构及索引策略，确保与 `local_data_model`、后续写入/通知任务共享同一契约。（docs/subscription_model.md）
- 详述订阅偏好（notifyOn、maxNotifications、deliveryWindow、snoozeUntil、channelMetadata 等）及 `metadata` 下游字段/客户端上下文，确立默认值与扩展约束。（docs/subscription_model.md:41-55）
- 给出订阅状态含义与转移（pending/active/paused/suppressed/unsubscribed），并以 Mermaid 状态图锁定事件驱动的生命周期。（docs/subscription_model.md:86-116）
- 明确 `POST /api/subscribe`/`POST /api/unsubscribe` 的请求/响应字段、错误码、幂等语义和示例 JSON，服务器负责 section 解析、contact hash、重复合并和 traceId 传播。（docs/subscription_model.md:117-239）
- 记录安全/合规策略：邮箱与 Discord 校验、rate limit 维度（perContact/perSection/perIP）、重复订阅处理、quiet hours、审计要求与 PII 削减流程，为后续实现提供检查清单。（docs/subscription_model.md:241-253）

## 接口与行为影响
- 文档中首次固化 `subscribe/unsubscribe` API 形状，所有消费者（前端、通知 worker、运营工具）需遵守字段名、状态与错误码；后续实现前应校验兼容性。
- 偏好与状态机定义意味着通知 worker/后台必须解析 `metadata.preferences` 并写入 `subscription_events` 记录，否则将违背契约。

## 风险 / TODO
- 当前仅为文档，无自动化校验或 schema 迁移；需要在 `ST-20251113-act-009-02` 等后续任务落实 API、DB 及 worker 逻辑。
- rate limit 与验证策略尚未与现有配置文件/中间件绑定，实施时需确认可观测性与错误码映射。 

## 自测
- 文档任务，未执行自动化测试。
