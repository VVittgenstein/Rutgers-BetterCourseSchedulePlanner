# P7.4-003 completion

The Linux x86-64 `0.1.0` public-package implementation is ready for its
dedicated commit and exact-commit Ubuntu verification.

- The archive contract contains exactly twenty approved files.
- The package excludes databases, Rutgers course data, local-only persistence,
  tests, source maps, and development runtime.
- The builder and verifier enforce deterministic packaging, target and shell
  validity, exact hashes, metadata, permissions, and forbidden surfaces.
- Local Bash syntax and usage, workflow YAML, generator syntax, and diff checks
  pass.
- The public-operations workflow builds the frozen source on Ubuntu 24.04,
  rehearses the real extracted candidate, and uploads only the verified tarball.

The final package hash and post-push pass are recorded by the exact-commit CI
result and consumed by `P7.4-004`.

Pre-push gate: `P7_4_LINUX_PACKAGE_IMPLEMENTED_PREPUSH`.
