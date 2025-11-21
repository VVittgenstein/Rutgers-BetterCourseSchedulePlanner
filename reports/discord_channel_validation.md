# Discord channel validation

## Scenario
- Goal: validate the Discord dispatcher turns `open_event_notifications` into channel sends, respects channel allowlists, and backs off on 429s without duplicate fan-out.
- Method: `npx tsx --test workers/tests/discord_dispatcher.test.ts` using the stub Discord bot + temp SQLite seeded with one open event and a channel subscription.

## Results
- Channel send path: pending notification → `DiscordBot.send` with `target.channelId` from `subscriptions.contact_value`, `dedupeKey=dedupe-001`, message content filled with course/index + manage link → `fanout_status` set to `sent` and `subscription_events.notify_sent` recorded.
- Rate-limit handling: stubbed `retryAfterSeconds=3` kept `fanout_status='pending'`, `fanout_attempts=1`, and set `locked_at` ~3s shy of the 120s TTL to delay the next pick-up (uses max of `Retry-After` or backoff).
- Channel allowlist: starting dispatcher with `--allow-channel <id>` skips any other channel targets, marking rows `skipped` with `error` containing `channel_blocked` and no Discord calls emitted.

## Reproduce / inspect
1) `npx tsx --test workers/tests/discord_dispatcher.test.ts`
2) Inspect database rows logged by tests:
   - `SELECT fanout_status, fanout_attempts, error FROM open_event_notifications;`
   - `SELECT event_type, payload FROM subscription_events;`
3) For live dry-runs: configure `testHooks.dryRun=true` and run `tsx workers/discord_dispatcher.ts --once --allow-channel <channel>` to watch logs without posting to Discord.
