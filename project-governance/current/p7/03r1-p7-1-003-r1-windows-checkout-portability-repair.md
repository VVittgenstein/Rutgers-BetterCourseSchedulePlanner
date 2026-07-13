# P7.1-003-R1 Windows checkout portability repair

## 1. Repair decision

- Repair task: `P7.1-003-R1`
- Immutable primary commit: `c2af7184aeec06704997f266530535003358c278`
- Primary parent: `1d997f6d3cca70ef54ec5b7adb2124f0b5905fa3`
- Branch: `codex/p7-implementation`
- Required topology: one ordinary fast-forward, single-parent repair commit directly after the primary commit
- Next task: `P7.1-004`, blocked until `P7_1_003_R1_PASS_POST_PUSH_CLEAN_REPLAY`

The primary P7.1-003 commit passed PreCommit, PostCommit, push and the shared-worktree PostPush gate. An additional remote Windows clone inherited `core.autocrlf=true`. Its checkout was clean under the effective Git configuration, but the P7 tools overrode that configuration with `core.autocrlf=false`; 133 normal CRLF checkout files were consequently presented as dirty. Raw worktree SHA-256 also made canonical text evidence dependent on checkout line endings. No denied file content was read while diagnosing the failure.

## 2. Repair boundary

This repair changes validation tooling and governance evidence only. It adds no product behavior, package content, dependency, Rutgers request, database mutation, Vultr mutation, release publication, or production change. It does not add a repository-wide `.gitattributes`: historical tracked text includes pre-existing CRLF or mixed blobs, so a global normalization rule could reinterpret unrelated history and the protected worktree.

The repair has two coupled rules:

1. Git status and diff commands use the checkout's effective Git configuration instead of forcing a different `core.autocrlf` value.
2. Known P7 text locks, evidence and completion records use a canonical text SHA-256: strict UTF-8, no BOM or NUL, no lone CR, CRLF normalized to LF, and a required final newline. Protected baseline sizes and hashes remain raw-byte checks.

The original 73-path task identity remains frozen to the primary commit. The primary builder verifies the primary commit's single parent and exact path set from four independent sources: the primary commit diff, primary contract blob, primary allowlist evidence blob, and primary completion blob. Replay is allowed only at that primary commit or its one direct, single-parent R1 child whose diff matches the independent R1 allowlist.

## 3. Required validation

The R1 builder and validator must prove:

1. immutable primary parent, ancestry and exact 73-path identity;
2. exact 10-path R1 allowlist;
3. effective-checkout Git status semantics;
4. raw and canonical text hashes remain distinct;
5. an isolated `core.autocrlf=true` clone is normally clean, is observably CRLF, reproduces the forced-false mismatch, and has canonical hashes equal to its LF repository blobs;
6. the full primary P7.1-003 builder still passes against the current checkout;
7. the shared worktree is exactly the preserved 167-path profile or a remote clean clone has zero foreign dirty paths;
8. publication blobs and commit messages contain no denied or sensitive material;
9. PreCommit, PostCommit and PostPush remote boundaries prove an ordinary fast-forward with no history rewrite.

The first PostPush run in the shared worktree may only produce `P7_1_003_R1_PASS_POST_PUSH`. The terminal clean-replay state is emitted only by a newly cloned remote checkout with profile `CLEAN_CHECKOUT_0`, effective `core.autocrlf=true`, observed CRLF text, a clean normal status, the isolated portability self-test passing, and all current canonical hashes and primary gates passing.

## 4. Non-effects

```text
rutgers_requests=0
database_mutations=0
package_builds=0
vultr_mutations=0
release_publications=0
production_mutations=0
```
