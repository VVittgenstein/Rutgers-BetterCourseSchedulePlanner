# ST-20251113-act-004-03-event-integration

- Added Discord event adapter + dispatcher (`notifications/discord/adapter.ts`, `workers/discord_dispatcher.ts`) that hydrates `open_event_notifications`, applies templates, enforces optional channel allowlists, and forwards dedupe keys to `DiscordBot`.
- Worker tests (`workers/tests/discord_dispatcher.test.ts`) cover channel sends, rate-limit retry/lock semantics, dedupeKey propagation, and allowlist skips.
- Runbook/report: `docs/discord_runbook.md` documents default channel config, switches, and ops queries; `reports/discord_channel_validation.md` summarizes the dispatcher test run and rate-limit/allowlist behavior.
