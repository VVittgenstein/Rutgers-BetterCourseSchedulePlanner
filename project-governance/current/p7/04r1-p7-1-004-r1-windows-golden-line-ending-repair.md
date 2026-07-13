# P7.1-004-R1 Windows golden line-ending portability repair

## 1. Repair decision

- Record: `P7-1-004-R1-WINDOWS-GOLDEN-PORTABILITY-2026-07-13-001`
- Repair task: `P7.1-004-R1`
- Immutable primary commit: `008de8c53da39af5562cd8a4022f839100a8a11d`
- Primary parent: `870a45496a4ad37f8e9a8f0b1f9b208bec0c5c38`
- Branch: `codex/p7-implementation`
- Required topology: one ordinary fast-forward, single-parent repair commit directly after the primary commit
- Next task: `P7.1-005`, blocked until `P7_1_004_R1_PASS_POST_PUSH_CLEAN_REPLAY`

P7.1-004 passed its required shared-worktree PostPush validation. A separate fresh Windows checkout with effective `core.autocrlf=true` then exposed a portability defect in `crates/bcsp-contracts/tests/wire_golden.rs`: `include_str!` observes checkout line endings, while the serialized JSON under test is canonical LF. The checked-in golden payload is semantically unchanged; only the test comparison must canonicalize CRLF to LF and reject any remaining lone carriage return.

## 2. Repair boundary

The repair changes exactly one product-test file and eight new governance files. It adds one local helper, routes every golden comparison through it, and adds a focused regression test that supplies an explicit CRLF string. It does not modify a golden fixture, production contract, dependency manifest, lock file, runtime, package, or repository-wide line-ending policy.

The helper contract is strict:

1. replace each CRLF pair with LF;
2. reject any carriage return that remains after replacement;
3. preserve every other byte and semantic value;
4. compare the canonicalized checked-in golden string with the existing canonical serializer output.

The primary P7.1-004 commit and its 41-path identity remain immutable. The repair validator proves the primary commit's parent and exact path set from the committed primary contract, allowlist evidence, completion record, and commit diff. The repair allowlist is independently frozen at nine ordinal-sorted paths.

## 3. Required validation

The builder and validator must prove all of the following:

1. immutable primary topology and exact 41-path primary identity;
2. exact nine-path repair boundary and no unrelated staged or committed path;
3. the precise CRLF-to-LF helper, lone-CR rejection, routed comparison, and regression test contract;
4. Rust graph guard and its negative self-test;
5. locked/offline Windows, Linux and complete Cargo metadata resolution;
6. locked/offline full-workspace check, test and Clippy with warnings denied;
7. formatting, cargo-deny policy, focused `wire_golden`, complete `bcsp-contracts`, and complete `bcsp-domain` tests;
8. unchanged Cargo/npm manifests and lock blobs and zero third-party closure delta;
9. strict UTF-8 canonical LF hashes for governed text while protected baseline hashes remain raw bytes;
10. an isolated `core.autocrlf=true` fixture with clean normal status, observable CRLF, raw blob divergence, and equal canonical hashes;
11. exact protected-worktree profile `EXACT_PRESERVED_167` or `CLEAN_CHECKOUT_0` without reading opaque denied content;
12. publication safety, zero side effects, and builder Write/VerifyOnly convergence.

The first PostPush validation in the shared worktree emits `P7_1_004_R1_PASS_POST_PUSH`. The terminal state is emitted only by `PostPush -RemoteCleanReplay` in a newly cloned remote checkout whose profile is `CLEAN_CHECKOUT_0`, whose effective `core.autocrlf` is `true`, whose normal status remains clean after all gates, whose checkout has observable CRLF and raw worktree/blob divergence, and whose focused and full Rust gates pass.

## 4. Git and publication boundary

The expected repair parent and pre-push remote baseline are both `008de8c53da39af5562cd8a4022f839100a8a11d`. The repair commit must be a direct single-parent child, must be pushed by ordinary fast-forward, and must never be amended after publication or force-pushed. The public allowlist contains only the repaired test, this contract, derived evidence, completion records, and the two independent repair scripts.

P1 material, conversation logs, `.secrets/`, credentials, personal information, caches, temporary files and unrelated worktree paths remain ineligible. `.secrets/` is checked only through ignore/tracking policy and is never enumerated or read. Publication scans operate only on the nine allowlisted blobs and the repair commit message.

## 5. Non-effects

```text
rutgers_requests=0
database_mutations=0
package_builds=0
vultr_mutations=0
release_publications=0
production_mutations=0
```
