# Storage growth and WAL maintenance

Date: 2026-09-01. Scope: the shared operational SQLite database of both
products (`bcsp-operational-storage`, `bcsp-application`) and the local
product's personal state (`bcsp-local-user-state`, `bcsp-local-runtime`).

## What was observed

One local session, ninety minutes, one page open, one New Brunswick target
polled every ten seconds:

- `rbcsp.sqlite-wal` grew to 3.0 GB. The WAL-index header showed
  `nBackfill = 0` and `nBackfillAttempted = 0` for the whole run, read-mark
  slot 1 pinned at frame 1 (the frame of the page's first
  `PUT /api/v1/local/current-filters`), while slot 2 advanced with every
  short read. That is the signature of a runtime-owned connection that began
  a read transaction during the page's first load and never ended it: every
  PASSIVE checkpoint stops at the oldest reader's mark, so nothing was ever
  copied into the main file.
- `rbcsp.sqlite` grew from 173 MB to 867 MB in the same run. Every Open
  attempt copies the target's full catalog membership (about 12,000 rows)
  into `open_attempt_catalog_sections`, and retention exempted every attempt
  of the CURRENT Rutgers day, so roughly 8,600 attempts per day were kept.
  The first attempt after midnight would then have deleted all of them in one
  cascading transaction.
- The shutdown `PRAGMA wal_checkpoint(TRUNCATE)`, run through a fresh
  connection after every runtime-owned connection was dropped, drained the
  3 GB log in about seventy seconds. The pinned reader was therefore in
  process, and it is gone once the runtime's connections are.

## Decisions

### Typed WAL maintenance, never raw SQL from dependents

`OperationalStorage` and `PersonalStateStore` both expose:

- `checkpoint_wal(mode)` with a typed `WalCheckpointMode`
  (`Passive | Full | Restart | Truncate`) returning the three
  `PRAGMA wal_checkpoint` counters (`busy`, `log_frames`,
  `checkpointed_frames`). In-memory operational fixtures return `None`.
- `transaction_state()` wrapping `sqlite3_txn_state`, so a host can ask a
  connection whether it is holding a transaction without touching the
  connection itself.

The `compile_fail` doctests that seal the raw connection are unchanged.

Every WRITER connection sets `PRAGMA journal_size_limit = 64 MiB`: both
`OperationalStorage::open` sites and every long-lived `PersonalStateStore`.
The read-only prepared-snapshot reader does not, because SQLite truncates the
log file from the connection that resets the log, so the setting would be
inert there. The limit bounds the FILE after a reset; it does nothing while a
reader pins the log, which is why the policy below exists.

### The maintenance policy lives in the refresh runtime

`bcsp_application::refresh_maintenance`:

- The commit path only raises a flag (`WalMaintenanceSignal`) from
  `ShortLockOpenPersistence::finish_open_pull_success/failure` and after every
  Catalog publication. Those run synchronously inside the async Open service
  on tokio worker threads; a checkpoint that waits for readers must not run
  there.
- The refresh supervisor consumes the flag on its 250 ms tick and runs
  `WalMaintenanceState::run` through `tokio::task::spawn_blocking` on the
  refresh writer connection, one run at a time.
- Every run is a PASSIVE checkpoint. `WalMaintenancePolicy::decide` (pure,
  unit-tested) escalates to RESTART above 16,384 log frames (64 MiB) and to
  TRUNCATE above 65,536 (256 MiB), at most once per 60 seconds; a Catalog
  publication earns a TRUNCATE outright.
- `WalStarvationDetector` counts consecutive PASSIVE reports that moved zero
  frames on a log of at least 4,096 frames. Three in a row suppress every
  escalation (RESTART and TRUNCATE would only wait out their busy timeout
  against the pinned reader) and log `WAL_CHECKPOINT_STARVED` at WARN once
  per episode, with the `transaction_state()` of every long-lived connection
  the host can name. The local runtime names seven: the refresh writer, the
  serving operational and personal connections, the mutation store, the
  desired-watch coordinator's store and the history worker's store (asked
  over its work queue with a 250 ms timeout). Every lookup is a `try_lock`;
  a connection whose owner is busy reports `LOCKED`.
- Reports go to `tracing::debug!` under target `bcsp_performance`
  (`phase = "wal_checkpoint"`).

The shutdown TRUNCATE through a fresh connection in `lifecycle.rs` is kept as
the last resort.

### Retention is a count, applied incrementally

`OPEN_DIAGNOSTIC_RETENTION_PER_TARGET` stays at 256 and the "must retain at
least 256" guard is unchanged. The Rutgers-day exemption is gone: the prune
keeps the 256 most recent attempts per target regardless of day, plus the
current last-known-good attempt and anything still `STARTED`. Daily, run and
service counters are separate aggregates and are never pruned.

Each commit prunes at most `OPEN_DIAGNOSTIC_PRUNE_ATTEMPTS_PER_COMMIT` (8)
attempts, oldest first. Steady state produces one new attempt per commit, so
a backlog (a day's worth of exempt attempts on an upgraded database, or
attempts recovered as `INTERRUPTED` after a restart, which recovery does not
prune) is worked off eight attempts at a time and never cascades tens of
thousands of membership rows inside one Open transaction.

### Freed pages do not shrink the file

Pruning returns pages to SQLite's free list; the main database only shrinks
with `VACUUM`. A database that already grew to hundreds of megabytes under
the old rule keeps that size until it is vacuumed, which needs roughly twice
the file's size in free disk space and, on a laptop disk, a minute or more.
That decision is deliberately not automated here.

## Follow-up: per-catalog-version membership

The per-attempt membership copy is only consumed by the attempt's own
intersection and orphan counts. It should become one membership table per
`(target, catalog content version)` written at publication, plus a
per-candidate table keyed by the staged observation, so an attempt stores a
reference instead of 12,000 rows. That is a schema migration
(`open_attempt_catalog_sections` has millions of rows on a live database and
the migration has to decide about `VACUUM`), and it changes what
`crates/bcsp-local-runtime/tests/local_runtime.rs` and the migration tests
enumerate. Retention as implemented here makes it a size optimisation rather
than the fix for unbounded growth.

## Finding the pinned reader

Layer 1 of the fix is the invariant, pinned by tests, that every public read
API of both stores leaves its connection out of a transaction
(`crates/bcsp-operational-storage/tests/wal_maintenance.rs`,
`crates/bcsp-local-user-state/tests/personal_state.rs`), plus a runtime test
that replays the page's first-load HTTP sequence against a seeded database
and then proves, from a fresh connection after a small write, that a PASSIVE
checkpoint backfills every frame
(`the_first_page_load_leaves_no_connection_pinning_the_wal` in
`crates/bcsp-local-runtime/tests/local_runtime.rs`).

That replay does NOT reproduce the pinned reader observed live, and no read
path in the typed stores holds a transaction open: rusqlite resets a
statement when its rows are dropped, every `consistent_read` commits or rolls
back, and no `Transaction`, `Statement` or `Rows` is stored anywhere. The
connection that pinned the live log has therefore not been identified from
the code alone. The next live run answers it: the first time three PASSIVE
checkpoints in a row move nothing, `WAL_CHECKPOINT_STARVED` names the
connection sitting in `READ`.

To read the evidence by hand, copy `data/rbcsp.sqlite-shm` and parse the
WAL-index header: `mxFrame` at offset 16, `nBackfill` at 96,
`aReadMark[0..5]` at 100..120, `nBackfillAttempted` at 128. A slot whose
mark never changes while the others advance is a pinned reader;
`nBackfillAttempted == 0` means read lock 0 is pinned too.

### Found: a leaked OS read lock, not a reader (2026-09-01, merge verification)

The replay test above turned out to fail about half the time once it ran
enough, in this tree and in the patch's own worktree alike, and the failing
runs had every runtime-owned connection idle (`sqlite3_txn_state` NONE, no
busy statement) while a PASSIVE checkpoint from a fresh connection still
backfilled nothing. Polling the OS byte-range lock on `WAL_READ_LOCK(0)`
(shm byte 123) showed it held for the rest of the process even after every
runtime connection was closed, and released only when the last connection to
the file went away.

The cause is in SQLite's Windows VFS (bundled 3.53.2), not in a store:
`winIsUNCPath` returns true for any path that begins with two backslashes,
which includes the verbatim `\\?\C:\...` paths that `Path::canonicalize`
produces and that `LocalRuntimePaths` derives the database path from. For
such a path the WAL-index locks go through ONE file handle shared by every
connection of the process (`bUseSharedLockHandle`), with per-connection
`sharedMask` bookkeeping deciding whether to touch the OS lock; `winShmLock`
does the bookkeeping under the node mutex but takes or releases the OS lock
after leaving it, so two readers taking the same read lock at the same moment
both call `LockFileEx` (Windows stacks shared locks on one handle) and the
unlock path releases only one. From then on no checkpoint can take that slot
exclusively, which for slot 0 means no backfill at all: the 3 GB log.

Fix: `OperationalStorage` and `PersonalStateStore` strip the `\\?\` prefix
from drive-letter paths before handing them to SQLite (`sqlite_open_path`),
so every connection gets its own lock handle and the bookkeeping is not used.
Pinned by `concurrent_readers_on_a_verbatim_path_do_not_pin_the_wal` in both
crates' test suites, which races six readers on a verbatim path and then
demands a complete PASSIVE checkpoint; it failed on every run before the fix.

## Testing option: no browser window

`RBCSP_SKIP_BROWSER_LAUNCH=1` keeps both the primary instance and a secondary
launch from opening a browser. The session URL is reported through the local
console instead, so a harness can read it and open the page itself. Exactly
`1`; any other value behaves like the product.
