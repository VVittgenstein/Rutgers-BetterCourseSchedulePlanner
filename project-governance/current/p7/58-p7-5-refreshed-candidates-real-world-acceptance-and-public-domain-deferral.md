# P7.5 refreshed candidates - exact-package real-world acceptance and public-domain deferral

- Parent: `87d590db4c1535700f289d4aad2539aa40730223`
- Verified: `2026-07-15T22:01:50.4405106Z`
- Branch: `codex/p7-implementation`
- Product source: `ceeeabb798cc262475c4d2fe220a4a7921995cde`
- Source date epoch: `1784148947`
- Status: `PASS_EXACT_PACKAGE_REAL_WORLD_WITH_PUBLIC_DOMAIN_DEFERRED`

## Exact release set

| Platform | Archive | SHA-256 | Bytes | Files | Artifact |
| --- | --- | --- | ---: | ---: | ---: |
| Windows x86-64 | `rbcsp-windows-x86_64-0.1.0.zip` | `2948b9289a6a80e31b5db16c0b1ffb2721f19bd723b30eff37a8a0d40eaa41f9` | 5,627,034 | 11 | `8357861761` |
| Linux x86-64 | `rbcsp-linux-x86_64-0.1.0.tar.gz` | `bc7d22e7b5abc997590f85563cdacbded4c76bfe98aa9899daf1842103ec05c1` | 6,457,771 | 20 | `8357719088` |

Formal build and synthetic/security verification passed in GitHub Actions run
[`29451243954`](https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/actions/runs/29451243954).
The two concurrency regressions and the existing fake-upstream product flow
passed. The Windows isolated-account synthetic flow, Linux install/restore/
upgrade/rollback rehearsal, Defender probe, and two-candidate ClamAV scan also
passed; ClamAV reported zero infected files. Both artifacts were downloaded
locally, their original archive hashes and sizes were rechecked, and fresh
local extractions passed all package-internal `SHA256SUMS` entries. No GitHub
Release has been published.

## Windows ordinary-user real-world flow

The exact ZIP was copied into a new candidate directory and extracted into a
previously nonexistent `fresh-extract` directory. The extraction contained no
`data` directory or SQLite file. `BCSP_CI_NO_RUTGERS` and the other BCSP launch
variables were absent at Process, User, and Machine scope. No old RBCSP process
or run directory was reused. `RBCSP.exe` was then launched directly with no
arguments and a visible normal application window; the first database was
created only at the package-relative `data/rbcsp.sqlite` path.

The current real Chrome opened the loopback application and completed the
ordinary visible UI flow:

- selected Fall 2026, New Brunswick, and the Open filter;
- ran course search and received real non-empty results;
- opened Course `01:013:120` (`LITERARY EGYPT`) details;
- opened Section `10052` details and confirmed a current, fresh `OPEN` state;
- selected that Section, opened Watch, enabled/tested audible browser sound,
  and started the selected watch;
- observed WebSocket `OPEN`, `Watching`, fresh `OPEN`, the visible alert and
  toast, then stopped the watch and disconnected to `CLOSED`.

On the completely fresh database, the first fast search honestly retained
`SOURCE_UNAVAILABLE` uncertainty while the first central Open snapshot was in
flight; it did not label those rows Open. Without restarting or rebuilding,
the same ordinary Search action after that snapshot returned 3,799 Open-filter
matches, 31 current `OPEN` Sections on the visible page, and zero unknown
Sections. This is the documented uncertainty-preserving filter behavior, not a
false-positive Open result.

The Watch telemetry showed a 30-second general interval, a 10-second effective
watch interval, an actual interval of `10.00s`, `242ms` scheduler lag, closed
circuit, and 10 attempts / 10 successes / 0 failures for the visible watched
target. Chrome reported zero page error logs. The durable Windows ledger held
1 discovery, 15 Catalog, and 27 Open attempts (22 general and 5 active-watch),
43 observed Rutgers requests in total, and no Rutgers 403 or 429.

The run started at `2026-07-15T21:32:16.7779891Z`, requested graceful exit at
`2026-07-15T21:40:16.3926584Z`, and lasted `479.615s`. The authenticated local
exit returned 204; process and listener disappeared; SQLite integrity was
`ok`; no WAL/SHM remained; and the candidate ZIP hash was unchanged. The fresh
extraction, database, and Rutgers data were then removed while the exact ZIP
was retained.

## Linux disposable-host real-world flow

After a `906.435s` cross-environment interval, exactly one manual dispatch ran
the frozen cached Linux artifact without rebuilding it. GitHub Actions run
[`29453630878`](https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/actions/runs/29453630878)
restored the exact cache key, rechecked the archive hash, size, 20-file boundary,
internal hashes, and absence of data, then installed it on a disposable Ubuntu
host with trusted Caddy HTTPS at `planner.test`. `BCSP_CI_NO_RUTGERS` was absent.

The real desktop and mobile browser flows passed with two TLS same-origin
scenarios, two WebSockets, two ready audio previews, and the configured 30/10
cadence. The durable evidence was:

```text
discovery=1 catalog=11 open_attempts=21 current_open=13617
general30=3 watched10=4
minimum_watch_actual_interval_ms=10000
minimum_watch_scheduled_due_interval_ms=10000
maximum_watch_scheduler_lag_ms=0 cadence_tolerance_ms=250
duration_seconds=148
```

No hard-stop signal fired, the job passed its integrity and centralized-poller
checks, and the always-run disposable-host cleanup step succeeded. This scope
is `DISPOSABLE_HOST_CANDIDATE_FLOW_NOT_PUBLIC_DOMAIN_E2E`; `planner.test` is a
local disposable-host name and is not represented as a public deployment.

## Corrected live protocol and public-domain disposition

The fixed `N+3` and `2N+5` request totals remain observational and are not
pass/fail gates. Both environments retained a request ledger and a 480-second
maximum live window. Acceptance is based on the configured approximately
30-second page-demand cadence, approximately 10-second active-watch cadence,
one centralized poller with no browser amplification, and immediate stop on
Rutgers 403/429, off-origin redirect, schema anomaly, or unsafe runaway. The
Windows and Linux candidate flows each ran once; there was no live retry.

There is currently no project public domain. Per user direction, the full
public-domain E2E is `DEFERRED_NO_DOMAIN_PER_USER_DIRECTION`; this record does
not claim it was run. The exact two-hash release set is nevertheless eligible
to enter the release stage.

This record supersedes the status, accepted candidate identity, source-change,
candidate-rebuild, gate, and next-task fields in 57/57a and
`records/p7-5-002-r3-adjudication.md` / `.json`. It retains the corrected live
protocol and historical observations. Records 55/55a and 56/56a remain
historical preflight/failure evidence rather than being rewritten.

Gate: `P7_5_EXACT_PACKAGE_REAL_WORLD_PASS_PUBLIC_DOMAIN_DEFERRED`.

Release candidate eligible: `true`.

GitHub Release published: `false`.

Next task: `RELEASE_STAGE_FOR_EXACT_TWO_HASHES`.
