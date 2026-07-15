# P7-REPAIR-001 - shared live-data responsiveness

- Parent: `f2d85fd61f1504a46a264438e676cc2c01c32fd4`
- Trigger: failed Windows real-world run `P7.5-002`
- Result required: `P7_REPAIR_001_PASS`

## Product repair

This task repairs the earliest shared owners of the live-run failure:

- read refresh policy before taking the local database mutex, eliminating the
  recursive non-reentrant lock in product routes;
- move synchronous SQLite/query route extensions onto blocking workers so HTTP,
  WebSocket and exit handling remain responsive;
- evaluate and sort lightweight query candidates, then materialize only the
  requested result page;
- replace repeated Catalog projection and semantic-hash scans with indexed
  linear passes;
- publish Catalog-derived subjects only after every target in the latest
  selectable Rutgers term has a current publication, including valid-empty
  publications.

The scope is the nine affected Rust source/test files plus this task and its
completion records. It does not change dependencies, package contents, public
operations, or production infrastructure.

## Acceptance

- regression coverage proves policy lookup occurs outside the storage lock;
- two-target discovery stays selector-only while incomplete and exposes both
  subjects atomically once complete;
- an 8,037-candidate query materializes only the 25-item requested page;
- large Course/Section query coverage, affected package tests, formatting,
  Clippy with warnings denied, and diff checks pass.

The failed P7.4 candidates remain permanently ineligible. No Rutgers request,
package build, Vultr mutation, release publication, or production mutation is
authorized by this repair task.

Next task: `P7-REPAIR-002`.
