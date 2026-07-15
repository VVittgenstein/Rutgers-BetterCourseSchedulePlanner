# P7.4-004-R1 - Final replacement-pair audit

`packaging/verify-release-set.ps1` passed against the exact replacement pair
downloaded from each independent GitHub-hosted run.

| Candidate | Bytes | Files | SHA-256 |
|---|---:|---:|---|
| Windows ZIP | 5,564,014 | 11 | `2c807e475779f9d3f7e8549921655dad87c4dff3b06286d05ac2a985fa2907d2` |
| Linux tar.gz | 6,393,468 | 20 | `67c4f5ac228dee7e4cc69378b12fbe294f3175f0bc453f5818e18704eba6cf04` |

Both audits enforced exactly two archives, package allowlists, provenance,
notices, ISC licensing, shared source identity, secret/data deny rules, 169
shared SBOM components, and 10 shared embedded frontend components. Neither
candidate contains a database or preloaded Rutgers course/Open data.

Gate: `P7_4_CROSS_PACKAGE_AUDIT_R1_PASS`.
Next task: `P7.4-005-R1`.
