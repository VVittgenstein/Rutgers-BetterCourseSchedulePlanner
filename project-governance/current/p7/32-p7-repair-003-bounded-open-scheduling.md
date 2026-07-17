# P7-REPAIR-003 - bounded Open scheduling

- Parent task: `P7-REPAIR-002`
- Trigger: the first real run made 45 Open attempts for `N=15`
- Result required: `P7_REPAIR_003_PASS`

## Product repair

Each latest-term target receives one mandatory initial Open attempt. After a
valid or valid-empty success, a target without product demand or watch demand is
parked. Retry, Catalog-race and circuit states retain the initial obligation and
respect cooldowns.

Product demand travels through the existing FIFO refresh command channel,
activates an already registered parked target once, and deduplicates duplicate
demand. Sticky demand uses the existing 30-second cadence; watches use the
existing 10-second cadence. Removing the final demand parks the target again.
Catalog change/race may explicitly wake one reconciliation attempt.

## Acceptance

- `N=15` reaches 480 seconds with exactly 15 no-demand Open attempts;
- demand before/after first success, retry, race, cooldown, watch and removal
  transitions are covered;
- affected Rust tests, formatting, Clippy with warnings denied, and diff checks
  pass.

No Rutgers request is executed by this task. The next task is the affected
P7.1-P7.3 gate rerun before new candidate construction.
