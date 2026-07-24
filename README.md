# Rutgers Better Course Schedule Planner

Rutgers Better Course Schedule Planner (RBCSP) is a course-centered search,
filtering, section-status, and live-watch application for Rutgers University.
It is built as a Rust workspace with a shared React frontend and supports two
explicit product targets:

- **Local desktop runtime** — starts a loopback-only web application, keeps
  user state on the local machine, and opens the browser automatically.
- **Public deployment runtime** — serves the public web application with
  operational storage, refresh scheduling, and server-side watch services.

The project is independent and is not affiliated with or endorsed by Rutgers
University.

## Release downloads

Version `0.1.0` provides two verified x86-64 archives in the repository's
[GitHub Releases](https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases):

| Platform | Asset | Use case |
| --- | --- | --- |
| Windows x86-64 | `rbcsp-windows-x86_64-0.1.0.zip` | Local desktop runtime |
| Linux x86-64 | `rbcsp-linux-x86_64-0.1.0.tar.gz` | Public deployment runtime |

Each archive includes `BUILD-PROVENANCE.json`, `MANIFEST.json`,
`SBOM.cdx.json`, `SHA256SUMS`, the ISC license, and third-party notices.

### Windows quick start

1. Download and extract `rbcsp-windows-x86_64-0.1.0.zip`.
2. Keep the extracted directory intact.
3. Run `Start-RBCSP.cmd`.
4. RBCSP opens in the default browser and stores writable state under
   `%LOCALAPPDATA%\\RBCSP` by default.

The Windows archive is intended for a real Windows x86-64 host and requires an
installed Chrome browser for the packaged runtime workflow.

### Linux deployment

1. Download and extract `rbcsp-linux-x86_64-0.1.0.tar.gz`.
2. Review `docs/operator-runbook.md` and `config/bcsp.env.example` in the
   archive.
3. Configure the public origin, operational database path, mail delivery, and
   reverse proxy/TLS for your environment.
4. Install and run `bin/bcsp-server`; example `systemd` and Caddy files are
   included.

The Linux archive targets x86-64 Linux with glibc 2.31 or newer. It is a
self-hosted deployment package, not a desktop installer.

## Product capabilities

RBCSP currently provides:

- Rutgers term discovery and course-data refresh;
- keyword search across course and section fields;
- subject, campus, level, credit, meeting-time, day, instructor, and section
  status filters;
- course and section detail views;
- open/closed section status and freshness metadata;
- live-watch workflows for tracked sections;
- English and Simplified Chinese interface copy;
- separate local and public frontend capability boundaries.

Rutgers remains the authoritative source for course and section data. Data can
be delayed, incomplete, or unavailable, and users should confirm important
registration decisions in official Rutgers systems.

## Repository layout

```text
apps/        Rust entry points for the local and public runtimes
crates/      Shared domain, query, storage, Rutgers client, and runtime crates
frontend/    Shared React source with local and public build targets
api/         TypeScript API and supporting services retained in the workspace
packaging/   Release builders, validators, manifests, and deployment templates
deploy/      Deployment configuration and operational assets
configs/     Checked-in example and schema configuration
```

## Development

### Prerequisites

- Rust stable toolchain
- Node.js `24.18.0` (see `.node-version`)
- npm `11.16.0` for the frontend workspace
- PowerShell 7 for the Windows packaging scripts

### Frontend

```bash
cd frontend
npm ci
npm run verify
```

Useful target-specific commands:

```bash
npm run dev:local
npm run dev:public
npm run build:local
npm run build:public
npm run test:product
```

The local entry point is `frontend/src/entry.local.tsx`; the public entry point
is `frontend/src/entry.public.tsx`. Import-boundary checks prevent one target
from accidentally depending on the other target's private modules.

### Rust workspace

```bash
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Run the local product from source:

```bash
cargo run -p bcsp-local
```

The local runtime binds to loopback, opens the browser, and places its writable
state in the platform-specific local data directory.

Run the public server from source:

```bash
cargo run -p bcsp-server
```

A production public deployment also requires the operational database, mail,
origin, TLS/reverse-proxy, and service configuration described by the Linux
release runbook and the files under `packaging/linux/`.

## Verification and release integrity

Release tooling lives under `packaging/`. The release workflow validates
archive structure, manifests, checksums, provenance, SBOMs, line endings,
platform binaries, and product smoke tests before publication.

After downloading an asset, compare it with the included checksum file:

```powershell
Get-FileHash .\rbcsp-windows-x86_64-0.1.0.zip -Algorithm SHA256
```

```bash
sha256sum rbcsp-linux-x86_64-0.1.0.tar.gz
```

## Security and privacy

Do not commit credentials, mail passwords, API tokens, production databases,
or user data. Use the checked-in example configuration as a template and keep
real values outside version control.

The local target keeps personal watch state on the user's machine. Public
deployments are operator-managed and must be configured with appropriate
secrets, access controls, TLS, backups, and retention policies.

## License

RBCSP is licensed under the [ISC License](LICENSE).
