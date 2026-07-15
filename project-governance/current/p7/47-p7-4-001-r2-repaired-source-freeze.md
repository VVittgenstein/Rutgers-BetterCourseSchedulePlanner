# P7.4-001-R2 - repaired source freeze

- Frozen source: `7d5debef005277e4d8f2ed2b9fb2f72c495e62f1`
- Source date epoch: `1784109539`
- Package count: exactly `2`
- Package build in this task: no

This source contains the Catalog writer/serving isolation repair and the
separate deterministic frontend-test repair. It is the sole input for the next
Windows local ZIP and Linux public tarball. Every earlier candidate remains
permanently ineligible.

Freeze gates passed: Rust formatting, locked/offline workspace/all-target tests,
Clippy with warnings denied, frontend guards 82/82, Vitest 105/105, TypeScript,
Local/Public production builds and target-surface gates. No dependency manifest
or lock file changed.

The builders now pin this source. The workflow intentionally retains the old
candidate hashes for one discovery run; that expected hash stop is not
acceptance evidence.

Next tasks: replacement Windows/Linux hash discovery and lock.
