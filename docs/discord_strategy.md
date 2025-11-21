# Discord Notification Strategy

Defines how the Discord channel is used for seat-open alerts, what the default mode should be for MVP, and how to configure the bot safely. This is intentionally lightweight so the follow-up coding tasks can plug into a clear contract.

## Goals
- Choose a default delivery mode (DM vs. channel broadcast) with a fallback that avoids missed alerts or noisy spam.
- Document required bot scopes/permissions and the config surface exposed in `configs/discord_bot.example.json`.
- Provide a message shape and rate-limit guidance so the sender can stay under Discord limits while keeping latency low.

## Mode comparison (DM vs. channel)
| Mode | Use when | Pros | Risks / trade-offs | Rate-limit profile |
| --- | --- | --- | --- | --- |
| Direct Message (`discord_user`) | Small personal deployments; users need private alerts; can tolerate an opt-in handshake. | User-level privacy; no channel noise; per-user buckets mean churn on one user does not block others. | Requires the user to share a guild with the bot and allow DMs from that guild; DM creation fails with `403` when privacy blocks; needs a verification or slash-command handshake to prove ownership of the snowflake. | Buckets are per-DM channel; practical ceiling ≈ 20–30 msg/s before hitting global limits. |
| Channel broadcast (`discord_channel`) | Shared study servers; operators want one alert feed; Discord friction must be near-zero. | Easiest to operate; no per-user handshake; one message fans out to everyone watching the channel; avoids PII storage beyond channel id. | Noisy if not isolated to a dedicated channel; needs role/mention discipline; per-channel bucket is ~5 msg/5s so bursts to a single channel need queuing. | 5 per 5 seconds per channel bucket; global limit (~50 req/s) still applies across guilds. |

**Decision:** Default to **channel broadcast** for MVP because it avoids DM privacy failures and scales better when multiple students subscribe to the same course. Offer DM as an opt-in for users who link their Discord account via a slash-command handshake. **Fallback:** when a DM send fails with `403/401` or times out during verification, route the notification to the default channel (if provided) and mark the subscription as `pending` until the user re-verifies or switches to email.

## Permissions, scopes, and configuration
- Required scopes: `bot` (Send Messages, Embed Links) and `applications.commands` (slash commands for DM verification). No privileged intents are needed for simple sends; `GUILD_MEMBERS` is optional if you want to validate guild membership server-side.
- Create a dedicated channel (e.g., `#course-alerts`) and, if needed, a role to mention (e.g., `@course-alerts`). Avoid `@everyone` mentions by default.
- Populate `configs/discord_bot.example.json` with deployment-specific IDs:
  - `applicationId`, `botTokenEnv`, `publicKeyEnv` for auth and slash command verification (tokens live in environment variables, not the file).
  - `defaultMode="channel"` plus `defaultGuildId/defaultChannelId` for the broadcast feed.
  - `dm.requireGuildIds` and `dm.verificationCommand` to gate DM delivery to users who opted in from allowed guilds; `dm.fallbackChannelId` is used when DMs are blocked.
  - `rateLimit` bucket hints (global/per-channel) and retry/backoff knobs; `testHooks` lets you force dry-run or override targets during smoke tests.
- Invite link template: `https://discord.com/api/oauth2/authorize?client_id=<applicationId>&permissions=19456&scope=bot%20applications.commands` (`19456 = VIEW_CHANNEL + SEND_MESSAGES + EMBED_LINKS`). Rotate tokens if suspected leakage.

## Message format and rate limiting
- **Copy template (markdown):**
  ```
  📢 Seat open for {{courseTitle}} ({{indexNumber}} @ {{campusCode}})
  Status: {{statusAfter}} | Seats Δ: {{seatDelta}}
  When/Where: {{meetingSummary}}
  Manage: {{manageUrl}} | SOC: {{socUrl}}
  Trace: {{traceId}}
  ```
  - Use short course titles; keep under ~150 characters before links to avoid truncation on mobile.
  - Prefer embeds only when you need localization/formatting; plain text keeps payload small and avoids embed rate limits.
- **Rate-limit guidance:**
  - Cap global dispatch to **≤20 msgs/sec** with a worker queue; respect per-channel buckets (**5 msgs / 5s**). Add jitter (0.3) and exponential backoff on 429 using `retry_after`.
  - Deduplicate with the existing `open_event.dedupe_key` to avoid repeats inside a 5-minute window.
  - For DM verification, expire pending requests after 15 minutes; treat `403` as permanent until the user re-verifies.
  - Surface `traceId` in logs per send so operators can line up failures with SQLite `open_event_notifications`.

## Operational playbook
- DM opt-in: users run the configured slash command, the bot replies in-channel with a one-time token and sends a DM to confirm; store `userId` + `guildId` on success.
- Channel hygiene: pin a short README in the alert channel explaining cadence, how to switch to email or DM, and how to mute/leave. If role mentions are enabled, require users to self-assign the role via a reaction role or slash command.
- Fallback: if both DM and channel paths fail, log the error with `subscription_id` and move the subscription to `suppressed` to avoid hot loops; ask the user to re-subscribe using email or a working Discord mode.
