# P7-REPAIR-004 - deterministic frontend gates

The standard frontend gate exposed two test-only assumptions after the product
repairs: a fixed freshness timestamp had passed in wall-clock time, and the
complete 22-field interaction test consistently needed about 6.2 seconds while
using the default 5-second timeout.

This repair changes only those tests. The freshness case fixes `Date` to its
existing fixture instant and restores real timers after every test. The full
22-field interaction case receives a local 10-second timeout. Product source,
runtime semantics, assertions, dependencies and build configuration do not
change.

Acceptance: the two focused files pass 18/18 and standard `npm run verify`
passes guards 82/82, Vitest 105/105, TypeScript checking, both Local/Public
builds, and package-surface gates.

Next task: affected P7.1-P7.3 gate confirmation and P7.4 candidate rebuild.
