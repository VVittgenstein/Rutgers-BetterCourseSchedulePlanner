# Compact - ST-20251113-act-004-03-event-integration

## 已落实事实
- 接入层：新增 Discord 适配器 `notifications/discord/adapter.ts`，将 `open_event_notifications` 记录 + 课程/meeting 补全为 Discord 消息，按配置模板填充 `courseTitle/index/status/seatDelta/meetingSummary/manageUrl/socUrl/traceId`，透传 `dedupeKey` 给 `DiscordBot`。
- 派发层：新增 worker `workers/discord_dispatcher.ts`，仅锁定 `contact_type in ('discord_user','discord_channel')` 的 pending 通知，构建 Discord 请求后调用 Bot；支持 channel allowlist（`--allow-channel`），不在名单则标记 `fanout_status=skipped`。发送成功写 `notify_sent`，retryable 根据 `Retry-After`/backoff 设锁时间，终态失败/跳过写 `notify_failed`。
- 邮件 worker 调整：`workers/mail_dispatcher.ts` 只抢占 `contact_type='email'` 的队列项，避免 Discord 通知被邮件 worker 误吞。
- 运行指引：新增 `docs/discord_runbook.md`（如何运行 dispatcher / 策略开关 / 监控查询）；验证记录 `reports/discord_channel_validation.md` 说明通路、429 退避与 allowlist 行为。Compact 汇总 `Compact/Compact-ST-20251113-act-004-03-event-integration-2025-11-21-T115350Z.md`。

## 接口/行为变更
- 新增 dispatcher CLI：`tsx workers/discord_dispatcher.ts --sqlite <db> --bot-config <cfg> [--allow-channel <id>] [--once] ...`，默认 channel 模式，可多次 `--allow-channel` 控制目标；使用 `DISCORD_BOT_TOKEN` 环境变量。
- `open_event_notifications` 的 Discord 通道现在走专用 worker；邮件 worker 不再处理非 email 通知。
- Bot 侧 dedupe：`dedupeKey` 从事件队列传递到 `DiscordBot.send`，并发重复会合并。

## 自测
- `npx tsx --test workers/tests/discord_dispatcher.test.ts` ✅ 覆盖频道发送、dedupeKey 透传、429 重试锁定、allowlist 拦截。

## 风险 / 限制 / TODO
- 仅带 stub/单元测试；尚未在真实 Discord 环境 & 实际队列跑 smoke，需要配置 `DISCORD_BOT_TOKEN`、允许频道并配合 poller 端到端验证。
- DM 验证/失败回退依赖 `DiscordBot` 现有逻辑（未新增 DM 授权流程）；运营需要根据 runbook 配置 `dm.fallbackChannelId`/allowlist。
- Backoff/锁定策略沿用 Bot `maxAttempts/backoffMs`；如生产需要更多重试或区分错误码，需后续调整配置即可。
