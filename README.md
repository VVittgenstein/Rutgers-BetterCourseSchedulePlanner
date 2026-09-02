# Rutgers Better Course Schedule Planner

Rutgers Better Course Schedule Planner (RBCSP) is a course search, filtering,
section-status, and live-watch application for Rutgers University.

The repository contains two supported products built from one Rust workspace and
one shared React frontend:

- **Windows local application** — runs on the user's computer, stores desired
  watches and other personal state locally, and restores those choices after a
  restart.
- **Linux public service** — runs as a centrally operated web service with
  session-scoped watches. It does not persist personal desired-watch state or
  provide server-push, service-worker, or native notification infrastructure.

## Download

Published packages are available from
[GitHub Releases](https://github.com/VVittgenstein/Rutgers-BetterCourseSchedulePlanner/releases).
Version `0.1.2` uses these two assets:

- `rbcsp-windows-x86_64-0.1.2.zip`
- `rbcsp-linux-x86_64-0.1.2.tar.gz`

Windows users can extract the archive and run `Start-RBCSP.bat`. The Linux
archive includes deployment configuration and an operator runbook.

## Features

- Search by course code, title, instructor, subject, campus, level, credits, and
  meeting time.
- Inspect sections, meeting details, and open/closed status.
- Monitor seat availability with automatic connection recovery, honest
  readiness status, audio self-healing, and an optional current-page browser
  notification fallback.
- Keep desired watches across Windows-local restarts; the first page to return
  re-materializes every still-watchable section.
- Run either as a local desktop-oriented service or as a public Linux service.
- Serve the target-specific React application directly from the Rust binary.

## Source tree

```text
apps/               Rust binary entry points
crates/             Application, domain, storage, Rutgers, watch, and runtime crates
frontend/           Shared React frontend with local and public target builds
deploy/public/      Linux service, Caddy, configuration, and operator files
packaging/          Windows and Linux release builders and verifiers
tools/architecture/ Product-boundary and public-surface checks
.github/workflows/  Continuous-integration workflow
```

## Prerequisites

The pinned development toolchains are:

- Node.js: see `.node-version`
- Rust: see `rust-toolchain.toml`
- npm: see `frontend/package.json`

The Linux and Windows packaging scripts also require the platform tools listed
in their respective scripts.

## Build and test

Install frontend dependencies:

```bash
npm --prefix frontend ci
```

Run the complete frontend verification suite:

```bash
npm --prefix frontend run verify
```

Build target-specific frontend assets:

```bash
npm --prefix frontend run build:local
npm --prefix frontend run build:public
```

Run the Rust checks:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

The Rust release binaries embed the corresponding frontend build:

```bash
cargo build --release --locked --bin bcsp-local
cargo build --release --locked --bin bcsp-server
```

## Development

Start either frontend target with Vite:

```bash
npm --prefix frontend run dev:local
npm --prefix frontend run dev:public
```

The local and public product compositions are intentionally separate. Boundary
checks prevent personal-only code from entering the public build.

## Packaging

Windows package:

```powershell
pwsh ./packaging/windows/build.ps1 -SourceRoot .
```

Linux package:

```bash
bash ./packaging/linux/build.sh --source-root .
```

The release contract and expected archive contents are defined in
`packaging/release-inputs.json`. Package-specific verification scripts are kept
next to each builder.

## Security and privacy

- Persisted personal watch data is limited to the local product.
- The public product excludes persisted personal watch state and server-push,
  service-worker, and native notification surfaces. A currently open page may,
  with the user's permission, use a page-level browser notification fallback.
- Public deployment configuration uses loopback binding, a reverse proxy, and
  systemd sandboxing defaults.
- Secrets and local runtime data must not be committed.

## License

RBCSP is released under the [ISC License](LICENSE).
