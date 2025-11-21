# Compact - ST-20251113-act-004-03-event-integration (update after CR)

## 已落实事实（变更点）
- Discord dispatcher 重试计数修正：`workers/discord_dispatcher.ts` 现在以 Bot 实际尝试次数（`final.attempt`/`attempts.length`）叠加 `fanout_attempts` 计算 `attempts`，`exhausted` 与退避锁定基于该累计值，避免 Bot 内已耗尽重试仍被 worker 重复多轮调用。
- 单元测试矩阵保持通过：`npx tsx --test workers/tests/discord_dispatcher.test.ts` ✅。

## 风险 / 限制
- 仍需在真实 Discord 环境端到端验证；重试策略参数沿用配置，生产可根据运行情况调整 `rateLimit.maxAttempts/backoffMs`。

## Code Review - ST-20251113-act-004-03-event-integration - 2025-11-21T13:10:15Z

workers/discord_dispatcher.ts
Comment on lines +318 to +336



    const sendResult = await this.bot.send(built.request);
    const attempts = job.fanoutAttempts + 1;
    const final = sendResult.finalResult;
    const serializedError = JSON.stringify({ finalResult: final, attempts: sendResult.attempts });

    if (final.status === 'sent') {
      this.persistOutcome(job, {
        fanoutStatus: 'sent',
        attempts,
        error: serializedError,
        subscriptionEvent: { type: 'notify_sent', providerMessageId: final.messageId },
        updateLastNotified: true,
      });
      return;
    }

    if (final.status === 'retryable') {
      const maxAttempts = this.config.rateLimit.maxAttempts;
      const exhausted = attempts >= maxAttempts;


@chatgpt-codex-connector
chatgpt-codex-connector bot

P1 Badge Respect bot retry count when deciding exhaustion

When DiscordBot.send already consumes its internal retry budget (e.g., repeated 429s), final.status is still 'retryable' with final.attempt equal to config.rateLimit.maxAttempts, but the dispatcher treats it as the first attempt because attempts is derived only from fanout_attempts and is compared to maxAttempts. This requeues the row as pending even though the bot has already hit the retry cap, so a single notification can trigger multiple worker passes of maxAttempts API calls (up to 9 with the defaults), overshooting the configured retry limits and hammering Discord unnecessarily. Exhaustion should be based on the bot’s attempt count (e.g., final.attempt or sendResult.attempts.length) rather than just the fanout_attempts counter.
