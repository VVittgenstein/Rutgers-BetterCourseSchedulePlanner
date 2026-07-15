# P7.5-002 - Windows real-world E2E failed attempt

- Task: `P7.5-002`
- Parent: `2153e5b4f7819bfe82e7c75dfe4de07c7c425b25`
- Candidate: `rbcsp-windows-x86_64-0.1.0.zip`
- SHA-256: `eb85374bbf97215124b4f2b64be4c51c96bc2af0502fc79b5230024709590610`
- Started: `2026-07-15T06:36:34.0939437Z`
- Stopped: `2026-07-15T06:44:48.2588643Z`
- Recorded: `2026-07-15T06:48:33.7155735Z`
- Result: `FAIL_RETURN_TO_EARLIEST_OWNER`

## Decisive result

The one permitted Windows live run did not pass. It used the exact candidate in
a new disposable standard-user profile and new package directory on the
existing physical Windows 11 host. Before start the archive contained 11 files,
no `data` directory, and no database material. The real `RBCSP.exe` process ran
under the disposable non-administrator SID and created only the package-relative
`data/rbcsp.sqlite` database.

Chrome opened the real loopback UI, verified the embedded product title and a
`READY` product shell, observed dynamic discovery data, selected a real Campus,
and clicked the real Search control. No background browser or localhost fixture
was substituted. The query then remained in the product loading state and did
not return Course results before the live window ended. The mandatory three
filters, Course/Section detail, WebSocket, watch, toast, and audio assertions
therefore could not pass.

## Redacted live ledger

Discovery produced `N=15` current valid Campus targets. The approved maxima were
therefore Catalog `15`, Open `18`, and total Rutgers attempts `35`.

| Lane | Observed | Approved maximum | Result |
|---|---:|---:|---|
| Discovery resource GETs | 2 | 2 | PASS |
| Catalog attempts | 15 | 15 | PASS |
| Open attempts | 45 | 18 | **FAIL +27** |
| Total Rutgers attempts | 62 | 35 | **FAIL +27** |

All recorded Catalog and Open responses completed without `403` or `429`:

- Discovery: one `APPLIED_CHANGED` observation, 9 terms, 135 total Campus
  entries, and 0 subjects.
- Catalog: 11 `APPLIED_CHANGED` plus 4 `EMPTY_VALID_INITIAL`; 15 targets,
  33,222,835 decoded bytes, 8,037 groups, 8,060 variants, 18,272 Sections, and
  25,455 occurrences.
- Subject dictionary rows after all 15 Catalog attempts: `0`.
- Open: 42 `VALID_APPLIED` plus 3 `VALID_EMPTY_NO_ROWS`, all HTTP 200;
  3,362,880 decoded bytes, 420,354 source values, 40,479 same-batch
  intersections, 379,875 orphans, and 0 duplicates.
- SQLite integrity after forced process stop: `ok`.

The Open scheduler continued its normal cadence during first initialization and
exceeded the live budget before the UI flow could finish. This alone invalidates
the run and the candidate's P7.5 eligibility.

## UI and shutdown failure

The first real Search request remained loading for several minutes even after
the database stopped growing and process CPU stopped advancing. A Chrome reload
also remained pending. Chrome control subsequently timed out while reading the
same live page; this is secondary evidence, not a replacement for the product
failure.

The local bootstrap endpoint then failed to answer within 10 seconds, so the
normal nonce-bearing 204 exit flow could not be completed. After verifying the
exact PID owner SID, the process tree was force-stopped. Final stop was 14.165
seconds after the 480-second deadline, which is also a hard failure.

## Cleanup and disposition

The disposable process, standard user, profile, package directory, and the
149,876,736-byte real-course database were removed. No local hosts or
certificate change was made. No raw
Rutgers body, URL/query, domain/IP, cookie, nonce, credential, Course title,
instructor, or Section identity is retained in this record or the repository.

The failed candidate must not be run again. Repair must return to the earliest
shared product owners for subject publication, bounded initial live scheduling,
and responsive query/shutdown behavior. After repair, both packages must be
rebuilt into new hashes, repeat P7.4 acceptance, and restart all three P7.5
environments from Windows.

Gate: `P7_5_RETURN_TO_EARLIEST_OWNER`.
Next task: `P7-REPAIR-001`.
P7 completed: `false`.
