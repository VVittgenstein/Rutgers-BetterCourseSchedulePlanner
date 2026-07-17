# P7.4-004-R2 - final replacement-pair audit

Two independent GitHub-hosted runs produced byte-identical candidate archives:

| Candidate | Bytes | Files | SHA-256 |
|---|---:|---:|---|
| Windows ZIP | 5,574,364 | 11 | `eabdc3b5f4a705d8c22e6941831f55e0bb5b5c2a1c33e648e545f86007cab577` |
| Linux tar.gz | 6,424,698 | 20 | `9bebd35808497e40ae36cb459c681ffb2ffe29c3b824988e460745d29f03605d` |

`packaging/verify-release-set.ps1` passed separately for both downloaded pairs
and again for final `release/0.1.0`. The final directory contains exactly these
two archives. The audit enforced exact allowlists, manifest/hash/provenance,
ISC licensing, notices, SBOMs, shared frozen source and epoch, secret/data
content checks, 169 shared components and 10 shared frontend components.
Neither package contains a database or preloaded Rutgers course/Open data.

Gate: `P7_4_CROSS_PACKAGE_AUDIT_R2_PASS`.
