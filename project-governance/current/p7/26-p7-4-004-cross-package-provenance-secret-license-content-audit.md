# P7.4-004 — Cross-package provenance, secret, license, and content audit

- Task: `P7.4-004`
- Parent: `cba4d713923685be721c5ec536a72a861b9505dc`
- Branch: `codex/p7-implementation`
- Release set: exactly `2` archives

## Product result

`packaging/verify-release-set.ps1` verifies the real Windows and Linux
candidates together. It safely extracts each archive, enforces the `11`/`20`
file allowlists, validates manifests, hashes, SBOMs, notices, ISC license, and
provenance, and rejects databases, preloaded course data, secrets, private build
paths, development residue, and platform-crossing content.

The audit also requires both packages to share the frozen source identity,
toolchain and normalized input hashes, and requires common SBOM and embedded
frontend components to be identical.

## Verified candidates

- Windows: `325794d064fdb4af3c16146e1ebdad21febcebe76f5a284f46bcde28f50bf479`
  (`5,534,138` bytes, two independent byte-identical builds).
- Linux: `3219cdba886f15409d65d9cb9b3e965971d5568cfcac6e73cdf92d72d4e49b40`
  (`6,366,055` bytes, two independent byte-identical Ubuntu runs).
- Joint result: `PASS`, `169` shared SBOM components and `10` shared frontend
  components.

The archives remain ignored release candidates. No GitHub Release or production
mutation occurred.

Gate: `P7_4_CROSS_PACKAGE_AUDIT_PASS`.
