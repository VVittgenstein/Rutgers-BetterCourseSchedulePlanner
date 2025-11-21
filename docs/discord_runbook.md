# Discord notification runbook

## Scope
- Covers the Discord channel/DM fan-out path fed by `open_event_notifications` and `workers/discord_dispatcher.ts`.
- Defaults to channel broadcast with an allowlist gate; DM is available when subscriptions are created with `contact_type=discord_user`.

## Prerequisites
- Config: `configs/discord_bot.example.json` populated with `applicationId`, `defaultChannelId`, and `botTokenEnv` (token in env var). Keep `testHooks.dryRun=true` for staging.
- Database: `scripts/migrate_db.ts` applied; poller is writing `open_events` + `open_event_notifications`.
- Allowlist (optional but recommended for channel mode): pass `--allow-channel <channel_id>` per channel you trust; anything else is skipped with `fanout_status=skipped`.

## How to run
- Dispatcher (channel default):
  ```
  DISCORD_BOT_TOKEN=xxx \
  tsx workers/discord_dispatcher.ts \
    --sqlite data/local.db \
    --bot-config configs/discord_bot.example.json \
    --batch 25 \
    --app-base-url http://localhost:3000 \
    --allow-channel 345678901234567890
  ```
  - Add `--once` for smoke tests; drop `--allow-channel` to allow any channel id.
- Open-sections poller (feeds queue): `tsx workers/open_sections_poller.ts --term 12025 --campuses NB --interval 20000 --checkpoint data/poller_checkpoint.json`

## Strategy switches
- **Channel broadcast (default):** `contact_type=discord_channel`, dispatcher allowlist contains the target channel, `defaultMode` in config may stay `channel`.
- **DM fallback:** set subscription `contact_type=discord_user`; keep `dm.enabled=true` and optionally set `dm.fallbackChannelId` to the default broadcast channel for users who block DMs.
- **Disable/force one channel:** run dispatcher with only the approved channel(s) via repeated `--allow-channel` flags; other channel IDs are skipped (`fanout_status=skipped`, error=`channel_blocked`).
- **Dry-run:** set `testHooks.dryRun=true` in config or export `DISCORD_BOT_TOKEN=dummy`; dispatcher still marks sends as `sent` for plumbing tests.

## Monitoring & triage
- Queue state:
  - Pending: `SELECT fanout_status, COUNT(*) FROM open_event_notifications WHERE fanout_status='pending';`
  - Stale locks: `SELECT COUNT(*) FROM open_event_notifications WHERE fanout_status='pending' AND locked_at < datetime('now','-120 seconds');`
  - Discord errors: `SELECT substr(error,1,120) AS err, COUNT(*) FROM open_event_notifications WHERE fanout_status IN ('failed','skipped') GROUP BY err ORDER BY 2 DESC;`
- Logs: dispatcher prints send/429 retries with `traceId` from `open_events`. Use `testHooks.overrideChannelId` in config to redirect all sends during incident response.

## Message/template notes
- Adapter fills `{{courseTitle}}`, `{{indexNumber}}`, `{{statusAfter}}`, `{{seatDelta}}`, `{{meetingSummary}}`, `{{manageUrl}}`, `{{socUrl}}`, `{{traceId}}` from the open event + section join.
- Dedupe: `dedupe_key` from `open_event_notifications` is passed to the bot; concurrent sends with the same key collapse inside `DiscordBot`.
- Rate limit: token-bucket per config (`globalPerSecond`, `perChannelBurst/perChannelResetMs`) with backoff `[0,2000,5000]ms` by default; worker locks rows for the max of `Retry-After` or backoff to avoid hammering Discord.
