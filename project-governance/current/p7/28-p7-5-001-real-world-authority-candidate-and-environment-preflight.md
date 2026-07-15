# P7.5-001 - Real-world authority, candidate, and environment preflight

- Task: `P7.5-001`
- Parent: `36290165c28573c9ab90a6db47674ca10cff5a64`
- Branch: `codex/p7-implementation`
- Verified at: `2026-07-15T06:26:06.1687574Z`
- Product source: `7d8297404d033e79b514333748b7072ebd3a0099`
- Real Rutgers requests in this task: `0`

## Outcome

The approved P7.5 real-world phase is ready to start its one-shot Windows run.
The P7.4-005 commit is present locally and remotely, its four-path PostPush gate
passed, and the exact two candidate archives remain unchanged:

| Candidate | Bytes | SHA-256 |
|---|---:|---|
| `rbcsp-windows-x86_64-0.1.0.zip` | 5,534,122 | `eb85374bbf97215124b4f2b64be4c51c96bc2af0502fc79b5230024709590610` |
| `rbcsp-linux-x86_64-0.1.0.tar.gz` | 6,366,047 | `77160882304fbe4d17a070a1cce16471cd618a1cd5cee18c9a2f6e9e8e920d07` |

The joint release-set audit passed with exactly two archives, `169` shared
SBOM components, and `10` shared embedded frontend components. Product runtime
and UI paths have no drift from the frozen source commit.

## Live authority and hard limits

The user approved the five P7.5 tasks and request budget, then explicitly
authorized continued P7 execution against these candidates. The current
browser requirement is binding: local and Vultr decisive E2E evidence must be
produced by Chrome operating the real UI. Background API, database, metric,
process, and request-ledger checks may supplement but never replace it.

For each environment, `N` is the current valid campus count produced by that
environment's real discovery. It is not hard-coded.

- discovery attempts: at most `2`;
- first-initialization Catalog attempts: at most `N`;
- Open attempts: at most `N + 3`;
- total Rutgers attempts: at most `2N + 5`;
- live window: at most `480` seconds and ends before a second 600-second Catalog
  refresh;
- environments: Windows, Actions Linux, and Vultr staging, strictly serial with
  at least `15` minutes between starts;
- one live run per candidate/environment, no automatic retry, matrix, cache
  bust, pressure test, fault injection, or manual refresh;
- origin concurrency `1` and per-target single-flight `1`;
- immediate stop on `403`, `429`, off-origin redirect, schema anomaly, or budget
  overrun; honor `Retry-After` and do not switch environments to evade it;
- no current real Open section yields `LIVE_PRECONDITION_NOT_MET`, never fake
  evidence or wait indefinitely.

Starting either candidate immediately begins real discovery. Therefore no
candidate was started during this preflight; process start is the live-window
start for its owning P7.5 environment.

## Environment readiness

### Windows and Chrome

- Physical Windows 11 host; the clean boundary is a new disposable standard
  user/profile and new package directory, not a new OS or VM.
- Elevated setup shell, Secondary Logon, local-user cmdlets, credentialed
  process launch, and installed Chrome: PASS.
- `BCSP_CI_NO_RUTGERS` is not inherited.
- Chrome control connection: PASS.
- The Windows archive has no preloaded database or course data. P7.5-002 must
  create only `<package>/data/rbcsp.sqlite` on first start.

### GitHub Actions

- The Linux hash is preserved by the two successful P7.4 clean-run cache keys.
- P7.5-003 will use a dedicated manual-only `workflow_dispatch`, one Ubuntu job,
  `contents: read`, no matrix/retry, and no OIDC, deployment, Release, Vultr, or
  Cloudflare permission.
- Actions browser automation is a supplemental Linux tier. It does not replace
  the user-required Vultr plus local-Chrome public E2E.

### Named Vultr staging instance

The user's prior control-plane/credential verification remains the identity
anchor. This task repeated authenticated, read-only guest preflight without
publishing an address, host key, credential, or private inventory:

- Ubuntu `24.04`, `x86_64`, systemd `running`, failed units `0`;
- `dpkg --audit` lines `0`, held packages `0`, reboot required `false`;
- `needrestart` service count `0`, kernel status healthy;
- `fwupd` active/success and simulated fwupd changes `0`;
- BCSP and Caddy units absent; listeners on 80/443: `0`/`0`;
- running services matched the established base-OS inventory;
- enough memory and root filesystem capacity for the bounded staging run.

The exact P7.5-004 mutation boundary is: verify a restorable control-plane
snapshot; install the unchanged Linux archive plus only `caddy`,
`libnss3-tools`, and `sqlite3`; create the package-defined `bcsp` user, FHS
paths, environment, and systemd unit; add only temporary TCP/443 access; run
test-only Caddy internal-CA HTTPS/WSS for a reserved test hostname; add and later
remove the matching local hosts entry and trusted public root certificate; run
the Chrome desktop/mobile E2E; then restore the snapshot or the already-approved
reinstall baseline and prove removal of services, data, user, paths, listeners,
firewall delta, certificate, and hosts entry.

This does not authorize real DNS, Cloudflare, ACME, a public certificate,
production traffic, GitHub Release, or production deployment.

Gate: `P7_5_WINDOWS_REAL_WORLD_ELIGIBLE`.
PostPush marker: `P7_5_001_PASS_POST_PUSH`.
Next task: `P7.5-002`.
