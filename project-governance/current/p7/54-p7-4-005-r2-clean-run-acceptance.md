# P7.4-005-R2 - clean-run acceptance and P7.5 entry gate

Two independent GitHub-hosted push runs rebuilt and accepted byte-identical
final candidates:

| Run | Windows | Linux | Product flow | Malware |
|---:|---|---|---|---|
| `29408621103` | PASS | PASS | PASS | PASS |
| `29409582329` | PASS | PASS | PASS | PASS |

Both runs used separate hosted runners and artifact IDs. Windows is
5,574,364 bytes with SHA-256 `eabdc3b5f4a705d8c22e6941831f55e0bb5b5c2a1c33e648e545f86007cab577`;
Linux is 6,424,698 bytes with SHA-256
`9bebd35808497e40ae36cb459c681ffb2ffe29c3b824988e460745d29f03605d`.

The second run was triggered by a workflow-comment-only commit, not a rerun.
Real Rutgers E2E has not started for these candidates. No GitHub Release,
production deployment, DNS, Cloudflare, ACME or production traffic mutation
occurred.

Gate: `P7_5_ELIGIBLE_R2`. Next task: replacement P7.5 preflight and Windows
Chrome real-world E2E.
