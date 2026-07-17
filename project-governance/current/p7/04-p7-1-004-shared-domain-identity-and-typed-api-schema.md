# P7.1-004 shared domain identity and typed API schema

## 1. Decision and entry gate

- Record: `P7-1-004-SHARED-DOMAIN-API-2026-07-13-001`
- Task: `P7.1-004 Shared domain identity and typed API schema`
- Required predecessor: `P7.1-003-R1` commit `870a45496a4ad37f8e9a8f0b1f9b208bec0c5c38`
- Required predecessor terminal state: `P7_1_003_R1_PASS_POST_PUSH_CLEAN_REPLAY`
- Expected remote baseline: `870a45496a4ad37f8e9a8f0b1f9b208bec0c5c38`
- Branch: `codex/p7-implementation`
- Package impact: `BOTH_TARGETS_SHARED_CORE`
- Next task after PostPush: `P7.1-005`

This task replaces the compile-only `bcsp-contracts` and `bcsp-domain` markers with the shared, target-neutral identity, domain-invariant, match-result and typed wire-contract foundation consumed by both final packages. It does not implement Rutgers requests, Catalog normalization or persistence, database behavior, runtime lifecycle, formal product UI, package assembly, Vultr changes, release publication, production deployment, or real-world E2E behavior. It adds no dependency and changes no dependency lock.

Entry is represented by the pre-existing single `EntryCapture` JSON already stored at `project-governance/current/p7/evidence/p7-1-004/predecessor-r1-clean-replay.json`. During evidence-builder Write, the builder must read and preserve that capture, invoke the replay helper exactly once with `Purpose=PreCommitReplay`, and then replace the single-capture document with the combined proof `{entryCapture, preCommitReplay, stableProjectionSha256, equivalent:true}`. The two results must be structurally equivalent over the stable result projection. No execution ID exists, and no execution ID may be required or invented. VerifyOnly, PostCommit and PostPush validation consume the combined proof and never invoke or network-replay the predecessor.

## 2. Shared identity contract

Identity is dynamic data, not a hard-coded Rutgers selector catalog. No identity constructor trims, case-rewrites, or silently repairs an input.

- `TermCampusKey` is `(term, campus)` and identifies a Rutgers target scope.
- `SectionKey` is the complete `(term, campus, index)` identity. `index` is exactly five ASCII digits and leading zeroes are identity-significant. A partial index is never a section identity.
- `CourseGroupKey` is `(term, campus, courseString)`. `courseString` remains an opaque upstream identifier; it is not reparsed into a second identity model.
- `CourseVariantKey` is `(CourseGroupKey, VariantFingerprint)`. The fingerprint is explicitly algorithm-versioned as `v1:` followed by 64 lower-case SHA-256 hexadecimal characters.
- Term and campus values are bounded canonical tokens. Campus remains dynamic and is not validated against a fixed campus allowlist.

Identity values are validated at construction and deserialization, serialize in camelCase containers, reject unknown identity fields, and preserve exact equality and ordering semantics. Golden fixtures and malformed-wire tests freeze valid and invalid forms. The collision fixture is synthetic-only and contains no captured Rutgers payload or real course record.

## 3. Shared course-domain invariants

`CourseVariant` and `CourseGroup` are invariant-bearing domain aggregates, not wire DTOs. They intentionally do not derive or implement Serde. P7.1-005 owns normalized Catalog DTOs and persistence mappings and must consume these shared identities instead of creating a parallel domain model.

The constructors enforce all of the following:

1. a course group contains at least one variant;
2. every variant belongs to the group key supplied to the aggregate;
3. variant fingerprints are unique within a group;
4. every section has the same term/campus scope as its variant's group;
5. a section is unique within a variant and cannot be attached across variants in one group;
6. duplicate classification requires complete equal `SectionKey` values;
7. multiple records collapse only when their semantic fingerprints are audited equivalent; conflicting duplicates fail closed.

Construction produces stable ordering. Domain failures map to stable typed reason codes. The implementation has no IO, persistence, request, clock, session, runtime-host, or target-specific policy.

## 4. Three-value match contract

Filter and domain matching use exactly three outcomes: `MATCH`, `UNCERTAIN`, and `NO_MATCH`. Missing, TBA, invalid, conflicting, unknown, or unavailable evidence must not be silently converted to a positive match or known mismatch.

- `MATCH` carries no reasons.
- `UNCERTAIN` carries at least one uncertainty reason and cannot carry `KNOWN_MISMATCH`.
- `NO_MATCH` carries at least one `KNOWN_MISMATCH` reason.
- Reason fields are bounded stable identifiers; unknown fields are rejected on input.

Conjunction is ordered `NO_MATCH > UNCERTAIN > MATCH`; disjunction is ordered `MATCH > UNCERTAIN > NO_MATCH`. Empty `all` is `MATCH`, empty `any` is `NO_MATCH`, and an inactive empty filter dimension evaluates to `MATCH`. Exhaustive truth-table tests bind these semantics.

## 5. Typed HTTP, WebSocket and error schema

API and WebSocket envelopes share protocol version `1`. Decoding probes and validates `protocolVersion` before decoding the typed payload, so malformed JSON and unsupported versions have distinct typed failures.

- Client-to-server HTTP and WebSocket envelopes reject unknown fields.
- Server-to-client success, error and WebSocket envelopes tolerate unknown fields for additive forward compatibility while preserving required fields.
- WebSocket envelopes carry a typed canonical trace/message ID.
- The shared error set contains eleven stable codes, stable message keys, canonical lower-case RFC 4122 random UUID-v4 trace IDs, and a tagged typed detail union.
- The authoritative shared error schema binds the concrete `ApiErrorBody` and `ApiErrorEnvelope` aliases to `ApiErrorCode`. A separately versioned target-specific error set must own a distinct schema and compatibility gate; it cannot silently extend the shared enum.

The contract manifest is schema version `1`. Every scalar and schema reference must resolve, tagged-union discriminator values must be unique, schema field/variant definitions must match the actual Serde boundary, and direction plus unknown-field policy must be explicit. Checked-in golden JSON freezes identity, match, HTTP success/error/request, WebSocket client/server and manifest shapes. Compatibility tests prove additive server fields remain readable while client inputs, invalid enum values, invalid identities, malformed payloads and unsupported versions fail closed.

## 6. Required gates

The hard gates are:

1. the preserved predecessor `EntryCapture`, exactly one builder-Write `PreCommitReplay`, and their stable structured equivalence;
2. Rust graph guard and its negative self-test;
3. locked/offline Windows-local, Linux-public and complete-closure Cargo metadata resolution;
4. locked/offline full-workspace all-target check, test and Clippy with warnings denied;
5. formatting and cached advisory, bans, license and source policy;
6. targeted locked/offline `bcsp-contracts` and `bcsp-domain` tests;
7. unchanged Cargo and frontend npm lock artifacts and zero third-party closure delta;
8. manifest, schema-reference, Serde-binding and checked-in golden canonical hashes;
9. synthetic-only fixture and publication-safety scanning without publishing raw command output;
10. exact 41-path task allowlist and either the exact 167-row protected shared-worktree profile or a genuine zero-foreign-path clean checkout;
11. zero Rutgers requests, database mutations, package builds, Vultr mutations, release publications and production mutations;
12. evidence builder Write and VerifyOnly convergence.

All governed text hashes use strict UTF-8 canonical LF SHA-256: no BOM, NUL, or lone CR; CRLF is normalized to LF; a final newline is required. Protected baseline sizes and hashes remain raw-byte checks.

## 7. Predecessor replay boundary

`project-governance/current/p7/tools/invoke-p7-1-003-r1-clean-replay.ps1` is the only predecessor replay helper. Evidence-builder Write invokes it exactly once with `Purpose=PreCommitReplay`; the helper must execute against the immutable predecessor commit and produce a redacted structured result, never raw checkout content or command output.

Before builder Write, `project-governance/current/p7/evidence/p7-1-004/predecessor-r1-clean-replay.json` is the pre-existing single `EntryCapture` JSON. Write reads and preserves that complete capture, obtains the separate `PreCommitReplay`, and rewrites the document as `{entryCapture, preCommitReplay, stableProjectionSha256, equivalent:true}`. Their stable projections include predecessor task and commit identity, branch and remote identity, terminal state, clean-checkout profile, effective `core.autocrlf=true`, normal clean status, observed CRLF and raw-blob-mismatch minima, portability/self-test results, and canonical governed hashes. The stable projections and their canonical SHA-256 must match. The source records expose no execution ID; validation must neither require nor invent one.

PreCommit requires the combined proof, its two preserved results, `stableProjectionSha256`, and `equivalent=true`. VerifyOnly, PostCommit and PostPush only validate that proof and its hashes. They must not call the replay helper, clone for predecessor replay, or query the network for predecessor state. The ordinary current-task PostPush remote-boundary check is separate and remains required.

## 8. Git and publication boundary

P7.1-004 uses one dedicated ordinary fast-forward, single-parent commit whose parent is exactly `870a45496a4ad37f8e9a8f0b1f9b208bec0c5c38`, followed immediately by push after all gates pass. The exact task allowlist contains 41 lexicographically sorted paths and is duplicated in the machine-readable contract and evidence.

The 167-path protected P7.1-001 baseline remains byte-preserved. P1 and conversation records stay opaque and unread; `.secrets/` is checked only through safe ignore/tracking policy and is never enumerated or read. Publication scans inspect only eligible task paths, generated evidence and commit metadata. Source, tests, synthetic fixtures, configuration, documentation, guards and necessary audit evidence are normal public-repository content; secrets, credentials, personal information, chat logs, databases, caches, temporary files and unrelated workspace files are never eligible.

Validation order is `pre-existing EntryCapture -> implementation -> evidence Write (exactly one Purpose=PreCommitReplay helper call) -> evidence VerifyOnly (zero helper calls) -> PreCommit -> dedicated commit -> PostCommit -> push -> PostPush`. Completion records must not contain or hash an actual task commit self-reference. `P7.1-005` begins only after `P7_1_004_PASS_POST_PUSH`.

## 9. Non-effects

```text
rutgers_requests=0
database_mutations=0
package_builds=0
vultr_mutations=0
release_publications=0
production_mutations=0
```
