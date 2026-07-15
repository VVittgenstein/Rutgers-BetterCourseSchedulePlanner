# P7.5-002-R1 - Windows Chrome UI pass, request-budget failure

- Candidate: `2c807e475779f9d3f7e8549921655dad87c4dff3b06286d05ac2a985fa2907d2`
- Started: `2026-07-15T09:02:41.7314278Z`
- Stopped: `2026-07-15T09:08:47.4172942Z`
- Live duration: `365.686` seconds

The exact 11-file ZIP was extracted for a new disposable standard Windows user.
It contained no data directory; first start created only the package-relative
database. Chrome operated the real loopback page and completed the product flow:
READY Catalog, multiple filters, real Course and Section details, live Open
association and freshness, WebSocket watch, Open toast, audible sound path,
lag/counters, Saved views, History, and Reset surfaces. Graceful exit completed
114.314 seconds before the deadline and SQLite integrity was `ok`.

The run nevertheless fails the approved request budget. Real discovery yielded
`N=15`, so Open was limited to `18` and total Rutgers attempts to `35`. The run
recorded `23` valid Open attempts and `40` total attempts: `+5` over both maxima.
No `403` or `429` occurred. The overrun happened because the active watch
remained running while local-only Saved views, History, and Reset checks were
completed; it is an operator sequencing failure, not a failed UI capability.

The process, disposable user/profile/package/database, and raw Rutgers data were
removed. Under the existing one-shot rule, the same candidate cannot be retried
without new authority.

Gate: `P7_5_WINDOWS_ONE_SHOT_RETRY_AUTHORITY_REQUIRED`.
P7 completed: `false`.
