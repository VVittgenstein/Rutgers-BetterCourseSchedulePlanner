# P7.4-005-R1 - Clean-run acceptance and P7.5 entry gate

Two independent GitHub-hosted runs rebuilt and accepted byte-identical final
candidates:

| Run | Windows | Linux | Product flow | Malware |
|---:|---|---|---|---|
| `29400462225` | PASS | PASS | PASS | PASS |
| `29401683685` | PASS | PASS | PASS | PASS |

The runs used separate hosted runners and artifact IDs. Both Windows ZIPs are
5,564,014 bytes with SHA-256 `2c807e475779f9d3f7e8549921655dad87c4dff3b06286d05ac2a985fa2907d2`;
both Linux archives are 6,393,468 bytes with SHA-256
`67c4f5ac228dee7e4cc69378b12fbe294f3175f0bc453f5818e18704eba6cf04`.

Because the approved isolation keeps the workflow off the default branch,
GitHub does not expose `workflow_dispatch`. The second clean run was therefore
triggered by a no-product-change workflow-comment commit. It is a distinct run
and candidate set, not a rerun of the first run.

Real Rutgers E2E has not begun. No GitHub Release, production deployment, DNS,
Cloudflare, ACME, or production traffic mutation occurred.

Gate: `P7_5_ELIGIBLE_R1`.
Next task: `P7.5-001-R1`.
