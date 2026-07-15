# P7.5-001-R2 - Replacement real-world preflight

- Parent: `f5ec8aa881ed708cea8c98f989fde1b2ec41b0c2`
- Branch: `codex/p7-implementation`
- Verified: `2026-07-15T13:32:29.4747962Z`
- Product source: `7d5debef005277e4d8f2ed2b9fb2f72c495e62f1`
- Real Rutgers requests in this task: `0`

The repaired replacement candidates passed the joint release-set verifier and
remain byte-for-byte equal to the two independently accepted GitHub-hosted
artifacts:

| Candidate | Bytes | Files | SHA-256 |
|---|---:|---:|---|
| Windows local ZIP | 5,574,364 | 11 | `eabdc3b5f4a705d8c22e6941831f55e0bb5b5c2a1c33e648e545f86007cab577` |
| Linux public tarball | 6,424,698 | 20 | `9bebd35808497e40ae36cb459c681ffb2ffe29c3b824988e460745d29f03605d` |

Exactly two packages are present. They share 169 SBOM components and 10
embedded frontend components. Neither package contains a database or preloaded
Rutgers Catalog/Open data. The Windows ZIP has no `data` directory.

The Windows host is ready for a fresh disposable standard-user/profile run:
setup is elevated, Secondary Logon and local-user cmdlets are available, Chrome
is installed, no prior candidate process or disposable test user remains, and
`BCSP_CI_NO_RUTGERS` is absent at process, user, and machine scope. A newly
established Chrome extension control session completed setup and documentation
without timeout. No replacement candidate was started during this preflight.

The approved live limits remain unchanged: discovery at most 2, Catalog at
most dynamic `N`, Open at most `N+3`, total Rutgers attempts at most `2N+5`, a
480-second window, one run per candidate/environment, no automatic retry or
manual refresh, and at least 15 minutes between environments. A `403`, `429`,
off-origin redirect, schema anomaly, or budget overrun requires immediate stop;
no current real Open section yields `LIVE_PRECONDITION_NOT_MET`.

Local and Vultr decisive evidence must come from Chrome operating the real UI;
background checks are supplemental. The manual-only Actions tier and the named
Vultr staging authority remain bounded by the approved P7.5 scope. Vultr must
receive an immediate baseline and restore-point check before mutation. GitHub
Release, production deployment, real DNS, Cloudflare, ACME, and production
traffic remain unauthorized.

Gate: `P7_5_WINDOWS_REAL_WORLD_ELIGIBLE_R2`.
PostPush marker: `P7_5_001_R2_PASS_POST_PUSH`.
Next task: `P7.5-002-R3`.
