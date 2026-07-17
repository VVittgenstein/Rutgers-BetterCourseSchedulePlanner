# P7.1-005 shared Catalog pipeline

## Task boundary

- Task: `P7.1-005`
- Record: `P7-1-005-SHARED-CATALOG-2026-07-13-001`
- Branch: `codex/p7-implementation`
- Parent: `b0eac56850f89fc1a2d0e3d4600bd988e97d748c`
- Predecessor: `P7_1_004_R1_PASS_POST_PUSH_CLEAN_REPLAY`
- Completion marker: `P7_1_005_PASS_POST_PUSH`
- Next task: `P7.1-006`

This task supplies one shared Catalog implementation for both delivery targets. It does not implement the query engine, live Open reconciliation, watch/WebSocket behavior, UI, packaging, release, or production deployment.

## Product outcome

P7.1-005 implements:

1. strict Rutgers Catalog and discovery DTOs that preserve missing, null, empty, malformed, and present states;
2. selector-derived term, campus, subject, and target discovery without a frozen campus allowlist;
3. target-scoped CourseGroup, CourseVariant, Section, occurrence, instructor, delivery, and provenance normalization;
4. independent versioned hashes for raw bodies, raw entities, semantic duplicates, variants, and normalized target content;
5. deterministic equivalent-duplicate collapse and fail-closed conflict or cross-variant ownership handling;
6. operational SQLite migrations, attempts, observations, checkpoints, normalized serving rows, and FTS5 rows;
7. atomic target replacement, unchanged-content handling, safe empty handling, rollback, restart recovery, and multi-target isolation;
8. typed API projections for discovery, normalized Catalog content, refresh observations, and freshness checkpoints.

`SectionKey=(term,campus,index)` and `CourseGroupKey=(term,campus,courseString)` remain complete identities. A `CourseVariantKey` hashes only the frozen offering-identity fields. Mutable canonical facts advance the content hash without churning variant identity.

Course and Section selector scope owns the requested term/campus identity. A payload's informational campus field may differ from the selector and is retained as source content; malformed campus shapes still fail closed.

The public normalized Course projection includes subject/unit notes, synopsis URL, and offering-unit title. The Section projection includes typed comments, section notes, subtitle/subtopic, session dates, deterministic cross-list text, add/drop permission, exam, eligibility, major/minor/honors, and unit-major facts. Occurrences expose campus name separately from meeting location. No ordinary UI contract exposes canonical raw JSON, and unknown or malformed collection members never disappear through silent filtering.

Delivery supports `T`, `H`, `O`, unknown/conflict states, and the contract-level `OTHER` value. Thursday parsing uses longest tokens (`TH`, `MTH`, `TTH`). Partial or invalid time evidence never becomes a definite availability match.

## Storage and empty safety

- A fresh migration creates schema only; no real or example course rows are seeded.
- FTS5 is required and probed explicitly; there is no silent `LIKE` fallback.
- Changed content replaces one target and advances its content version atomically with FTS and observation state.
- Unchanged content advances observation state without rewriting serving rows or incrementing the content version.
- A selector-confirmed first empty may publish an empty version.
- A nonempty-to-empty response is retained as suspect evidence and does not erase the last known rows. No unrecorded numeric confirmation threshold is invented.
- Failed normalization/publication rolls back live rows and FTS changes.
- Reusing an observation ID is idempotent only when target, original start time, and any already-stored source digest/byte count agree. Completion ordering is checked against the persisted original start time.
- Shared storage accepts a caller-supplied connection or relative database path and does not choose target-specific package paths.

## Recorded P3 replay

The opt-in, read-only recorded replay verified every registered body hash before decoding. It normalized all 21 scopes and projected all 17 nonempty scopes through SQLite and the public typed contract. Its publishable result is aggregate-only:

| Fact | Result |
|---|---:|
| Scopes | 21 |
| Nonempty / valid empty | 17 / 4 |
| Raw course rows | 10,629 |
| Raw section rows | 22,069 |
| Unique SectionKeys | 22,051 |
| Equivalent / conflicting duplicates | 18 / 0 |
| Meetings | 30,804 |
| Instructor assignments | 19,202 |
| Multi-object groups | 41 |
| Multi-variant groups | 31 |
| Cross-variant Section overlap | 0 |
| Delivery conflicts | 184 |

Only those counts and two non-sensitive aggregate hashes are committed. Raw JSON, course text, instructor names, generated databases, local absolute paths, and credentials are excluded.

## Verification

Normal verification is deliberately ordinary and repeatable:

- Rust architecture graph and graph self-test;
- `cargo fmt --all -- --check`;
- locked/offline workspace `check`, `test`, and `clippy -D warnings`;
- locked/offline cargo-deny advisories, bans, licenses, and sources;
- focused synthetic normalization, projection, storage, schema, and migration tests;
- the opt-in aggregate-only recorded replay;
- exact task-path staging, protected-worktree comparison, and sensitive-path/content scan;
- commit, push, remote-ref verification, and PostPush validation.

The earlier Windows-path and GitHub TLS failures were infrastructure failures while validating the predecessor. The predecessor subsequently passed. P7.1-005 does not retain a recovery-protocol implementation, validator self-hashes, source-marker cardinalities, or one-shot control-file state.

## Git and non-effects

The task allowlist contains 50 project paths. The abandoned predecessor-recovery helper was removed from the original 51-path draft because it adds no Catalog capability. The deny boundary includes `.secrets/`, credentials, personal information, chat logs, P1 material, raw P3 bodies, SQLite/WAL/SHM files, caches, build targets, and unrelated protected-worktree paths.

```text
live_rutgers_requests=0
package_builds=0
vultr_mutations=0
release_publications=0
production_mutations=0
```
