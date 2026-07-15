# P7.5-002 failed-attempt record

The exact Windows candidate
`eb85374bbf97215124b4f2b64be4c51c96bc2af0502fc79b5230024709590610`
ran once under a fresh disposable standard user and real Chrome UI.

- Candidate had 11 files, no data directory, and no database before start.
- Package-relative database creation and process-owner checks passed.
- Dynamic current Campus count `N=15`.
- Catalog attempts `15/15`; 11 changed and 4 valid empty.
- Open attempts `45/18`; live budget exceeded by 27.
- Total Rutgers attempts `62/35`; live budget exceeded by 27.
- Catalog records existed, but published subject rows remained 0.
- Chrome clicked real Search, which remained loading; detail/watch assertions did
  not pass.
- The exit API was unresponsive; owner-verified force stop occurred 14.165
  seconds after the 480-second deadline.
- SQLite integrity was `ok`; all disposable state and real-course data were
  removed.

This candidate is not eligible for a P7.5 retry. Return to the earliest product
owners, rebuild both candidates into new hashes, repeat P7.4, and restart P7.5.

Gate: `P7_5_RETURN_TO_EARLIEST_OWNER`.
P7 completed: `false`.
