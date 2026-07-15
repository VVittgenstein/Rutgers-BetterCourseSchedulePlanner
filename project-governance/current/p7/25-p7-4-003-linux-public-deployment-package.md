# P7.4-003 — Linux public deployment package

- Task: `P7.4-003`
- Parent: `d6b26745a20f604698ab6a3f28a981934693f261`
- Branch: `codex/p7-implementation`
- Package: `rbcsp-linux-x86_64-0.1.0.tar.gz`
- Package files: exactly `20`

## Product result

The Linux x86-64 public package is built from the frozen integrated source. It
contains the real `bcsp-server`, embedded public UI, Caddy and systemd assets,
operator scripts, ISC license, target-specific CycloneDX SBOM, third-party
notices, provenance, manifest, version, and hashes. It contains no database,
Rutgers course data, local-only saved views or history, tests, source maps, or
development runtime.

The builder creates a deterministic GNU ustar/gzip archive and the verifier
checks the exact twenty-file allowlist, modes, hashes, metadata, ELF target,
shell syntax, forbidden surfaces, and byte-identical repacking.

Local Bash syntax and usage contracts, workflow YAML, metadata-generator
syntax, and the repository diff check pass.

## Post-push acceptance

The exact pushed commit must build and verify the real archive on Ubuntu 24.04,
install that extracted candidate through the disposable-host rehearsal, and
upload the verified tarball as a GitHub Actions artifact. This is package
validation only: it does not create a GitHub Release or mutate production.

Pre-push gate: `P7_4_LINUX_PACKAGE_IMPLEMENTED_PREPUSH`.
Post-push target: `P7_4_LINUX_PACKAGE_PASS`.
