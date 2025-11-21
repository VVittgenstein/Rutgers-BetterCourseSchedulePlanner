#!/usr/bin/env tsx
import process from 'node:process';
import { parseArgs } from 'node:util';

import { DiscordBot, loadDiscordBotConfig } from '../notifications/discord/bot.js';

type CliOptions = {
  config: string;
  channelId?: string;
  userId?: string;
  message: string;
  traceId?: string;
  dryRun: boolean;
};

function parseCliArgs(): CliOptions {
  const { values } = parseArgs({
    options: {
      config: { type: 'string', default: 'configs/discord_bot.example.json' },
      channel: { type: 'string' },
      user: { type: 'string' },
      message: { type: 'string', default: 'Hello from BetterCourseSchedulePlanner Discord test.' },
      trace: { type: 'string' },
      dryRun: { type: 'boolean', default: false },
    },
  });

  return {
    config: values.config,
    channelId: values.channel,
    userId: values.user,
    message: values.message,
    traceId: values.trace,
    dryRun: values.dryRun,
  };
}

async function main() {
  const opts = parseCliArgs();
  const config = await loadDiscordBotConfig(opts.config, process.env);
  const bot = new DiscordBot({
    ...config,
    testHooks: { ...config.testHooks, dryRun: opts.dryRun ?? config.testHooks.dryRun },
  });

  const result = await bot.send({
    target: { channelId: opts.channelId, userId: opts.userId },
    message: { content: opts.message },
    traceId: opts.traceId,
  });

  console.log('Discord send result:', JSON.stringify(result.finalResult, null, 2));
  if (result.finalResult.status !== 'sent') {
    console.error('Attempts:', JSON.stringify(result.attempts, null, 2));
    process.exitCode = 1;
  }
}

main().catch((error) => {
  console.error('Discord test send failed', error);
  process.exitCode = 1;
});
