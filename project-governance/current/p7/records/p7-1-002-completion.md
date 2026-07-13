# P7.1-002 completion record

## Outcome

- Task: `P7.1-002` — dependency lock, license, and initial SBOM baseline
- State: `P7_1_002_PASS_COMMIT_ELIGIBLE`
- Branch: `codex/p7-implementation`
- Expected parent: `f39b033b491b7f22429348df11e3fd6191ef1615`
- License: `ISC`
- Copyright: `Copyright (c) 2026 VVittgenstein`
- Commit boundary: exactly the 26 paths in the task contract

This task creates only the dependency-resolution scaffold and its governance evidence. `P7.1-003` still owns the final Rust workspace/module graph and product source. P7.5 real-world execution, real Rutgers requests, Vultr mutation, GitHub Release, production deployment, DNS, Cloudflare, certificate, and production-traffic changes remain unauthorized.

## Locked closure

| Assertion | Result |
|---|---:|
| Cargo lock identities, including the workspace member | 250 |
| npm lock identities, including the frontend root | 175 |
| Combined canonical identities | 425 |
| Locked-component rows | 425 |
| Official-metadata rows | 425 |
| License-decision rows | 425 |
| License-file hash or explicit-absence rows | 649 |
| License-hash-covered identities | 425 |
| CycloneDX 1.6 components | 425 |
| Rejected dependencies in the active graph | 0 |
| Unknown or denied license decisions | 0 |
| Platform-excluded license decisions | 1 |

The only `PLATFORM_EXCLUDED` decision is `webpki-root-certs@1.0.8` under `CDLA-Permissive-2.0`; its Cargo package identity is absent from both approved target graphs. All other 424 license decisions are `APPROVED`. The zero-consumer claim remains limited to `ACTIVE_P7_TARGET_GRAPH_ONLY`; it is not a repository-wide claim.

## Integrity bindings

- `Cargo.lock` SHA-256: `93992B1D426C91202203E06ACE3BF55D21A91510A3C4E0D6D0CFED422F410263`
- `frontend/package-lock.json` SHA-256: `97322B86E314BE69E4601CD4EE9187E359AA9FDB7B9186F193EF1D98B5A06466`
- Advisory evidence: both required live-network commands passed; Cargo and npm vulnerabilities are zero.
- Toolchain evidence: seven pinned binaries and seven normalized version/help commands passed and are bound by repository-relative path, size, and SHA-256.
- The initial combined SBOM has 425 unique `bom-ref` values and passed the required cargo-cyclonedx 0.5.9 Rust cross-check.

## Quality gates

The following gates passed with exit code 0: locked/offline Cargo check, test, Clippy, and formatting; cargo-deny; isolated npm ci and npm ls; npm dedupe dry-run; evidence-builder verify; and canonical evidence write.

## Protected state and side effects

- All 167 pre-existing worktree paths remain protected under the P7.1-001 manifest.
- The excluded legacy graph remains `FROZEN_EXCLUDED_PENDING_OWNER_TASK`.
- Rutgers requests: 0.
- Vultr mutations: 0.
- Release publications: 0.
- Production mutations: 0.

`P7.1-003` remains blocked until this exact 26-path commit passes PreCommit, PostCommit, push, and PostPush validation.
