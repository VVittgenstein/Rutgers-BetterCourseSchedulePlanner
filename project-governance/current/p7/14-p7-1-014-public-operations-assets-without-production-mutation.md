# P7.1-014 — Public operations assets without production mutation

- Task: `P7.1-014`
- Parent: `a3648a6cb04d88961e27e18cfb074b58bc0a5421`
- Branch: `codex/p7-implementation`
- Next task after PostPush: `P7.1-015`

## Product result

The Linux public package now has an actual systemd service, an operator-owned
Caddy example, a one-variable non-secret configuration contract, and executable
install, verify, backup, restore, upgrade, and paired rollback operations.

The fixed layout is `/opt/bcsp/releases/<version>`, `/opt/bcsp/current`,
`/etc/bcsp`, `/var/lib/bcsp/rbcsp.sqlite`, and `/var/backups/bcsp`. Releases are
immutable and contain the real service binary and public web build, but never an
environment file, secret, certificate, or course database. The service runs as
the non-login `bcsp` user and only writes its state directory.

The Ubuntu workflow builds the real frontend and Rust binary, assembles the
candidate, prevents external network access at the systemd boundary, and then
rehearses first install, empty database creation, backup/restore, successful
upgrade, paired rollback, and automatic rollback after a failing release. It
does not mutate Caddy, DNS, certificates, Vultr, or production traffic.

## Verification

- Bash syntax for all eight operations/test scripts: PASS
- Environment example and JSON schema: PASS
- GitHub Actions workflow YAML: PASS
- Existing public Rust zero-surface package guard: PASS
- Real Ubuntu systemd/Caddy/SQLite rehearsal: required PostPush gate

The task is complete only when the pushed `Public operations rehearsal`
workflow passes for the exact commit.
