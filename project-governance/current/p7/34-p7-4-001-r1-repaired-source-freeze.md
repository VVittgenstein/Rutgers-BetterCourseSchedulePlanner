# P7.4-001-R1 - repaired source freeze

- Frozen source: `476565cbe8e19075214cdc1427c86cf2dcf4e966`
- Package count: exactly `2`
- Package build in this task: no

The source repaired after the failed Windows live run is now the sole input for
replacement candidates. The release contract remains one Windows local ZIP and
one Linux public tarball with the existing exact allowlists, global denylist,
ISC license, and no preinstalled database or Rutgers data.

Freeze gates passed:

- Rust format, locked/offline workspace check, 449 passing tests with two
  existing opt-in ignores, and Clippy with warnings denied;
- frontend guards 82/82, Vitest 105/105, TypeScript, Local/Public builds, and
  package-surface checks;
- no dependency manifest or lock-file change in the repair series.

Every earlier candidate hash remains permanently ineligible. Replacement
packages must be built from the exact frozen source above and repeat P7.4
construction, audit, and clean-environment acceptance before P7.5 restarts.

Next tasks: `P7.4-002-R1` and `P7.4-003-R1`.
