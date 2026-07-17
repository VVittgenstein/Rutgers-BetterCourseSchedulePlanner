# P7.2-003 — Watch toast audio freshness and subscription-management UI

- Task: `P7.2-003`
- Parent: `06f56cae05c5965ed18c5d22d863af69e3d9e709`
- Branch: `codex/p7-implementation`
- Skills used: `$industrial-brutalist-ui`, `$design-taste-frontend`
- Next task after PostPush: `P7.2-004`

## Product result

The shared local/public UI now has a connection-bound Watch desk. Users can
select at most nine Sections, start and stop watches explicitly, update active
policies, reset one-shot counts, disconnect without losing their selection, and
see selected, pending, and active state separately. A server-authoritative alert
center keeps per-Section acknowledge and timed-out resume controls available even
after an alert card is dismissed.

One-shot cues report STARTED, MUTED, AUTOPLAY_BLOCKED, and FAILED outcomes to the
server; reaching the audible cap silences only audio and leaves the watch active.
Continuous 10-minute and Unlimited policies require explicit confirmation and
drive one aggregate browser mixer for the server's active episode set. Mute,
volume changes, disconnect, acknowledgement, and mixer-stop events stop or update
that sound immediately. Browser audio remains usable under React StrictMode, and
autoplay rejection is surfaced as BLOCKED rather than hidden as a generic failure.
The explicit sound-test control emits a local preview cue without consuming a
server cue or one-shot count.

The Watch desk shows batch and Section state, freshness, observation time,
requested and actual cadence, scheduler lag, circuit/retry state, last-known-good
age, uncertainty, and run/today counters. Disconnect clears connection-owned Open
observations and rereads status, so retained selections cannot display stale Open
state as current. Superseded status responses cannot overwrite newer telemetry or
repopulate a cleared selection, and FRESH state expires locally at `freshUntil`
while the authoritative status reread is in flight. `/watch` is reload-safe in the Windows local shell and shares
the same UI and WebSocket protocol in both targets.

## Verification

- Frontend guard: PASS 81/81
- Vitest product/component suite: PASS 80/80
- TypeScript and local/public production builds: PASS
- Public DOM/route/i18n/bundle boundary: PASS 72/72
- Rust workspace fmt, check, and tests: PASS
- axe watch-workspace scan: PASS, zero violations in the jsdom-supported rules
- Synthetic real-Chrome Watch flows: PASS 6/6, including desktop/mobile, A/C/D
  continuous episodes, audio blocked, stale, unknown, and disconnect
- Browser assertion: continuous mixer started a real Web Audio oscillator
- Desktop/mobile horizontal overflow: PASS none

Snapshots are under `project-governance/current/p7/evidence/p7-2-003/`.
