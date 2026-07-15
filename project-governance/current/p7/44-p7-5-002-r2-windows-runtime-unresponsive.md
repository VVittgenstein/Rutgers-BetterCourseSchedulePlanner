# P7.5-002-R2 - approved Windows exception retry, runtime unresponsive

- Candidate: `2c807e475779f9d3f7e8549921655dad87c4dff3b06286d05ac2a985fa2907d2`
- Started: `2026-07-15T09:18:52.6223710Z`
- Stopped: `2026-07-15T09:21:23.8049812Z`

The approved one-time retry used the exact same ZIP, a new disposable standard
Windows user, no pre-existing data directory, and a package-relative first-run
database. Chrome navigation to the real loopback origin did not complete while
Catalog initialization was in progress. An independent request to the local
bootstrap endpoint also timed out, so this was a product-runtime failure rather
than a Chrome-control-only blocker.

At forced shutdown the candidate had completed or begun 13 Catalog attempts,
made no Open request, and received no 403 or 429. SQLite integrity was `ok`; the
large uncommitted WAL was removed with the disposable user, profile, package,
database, and raw Rutgers response data. No candidate process remains.

The same candidate is retired. Product repair and a newly built candidate are
required before P7.5-002 can run again.

Gate: `P7_5_WINDOWS_NEW_CANDIDATE_REQUIRED`.
