# P7.4-002 — Windows local release archive

- Task: `P7.4-002`
- Parent: `7aa314def7947d1e1160e3417a9b52cbf26e1058`
- Branch: `codex/p7-implementation`
- Package: `rbcsp-windows-x86_64-0.1.0.zip`
- Package files: exactly `11`

## Product result

The Windows x86-64 local package is now built from the frozen integrated
source with the local Vite UI embedded in `RBCSP.exe`. It has no development
runtime dependency, pre-created database, seed, or course data. First start
creates the sole package-relative database at `data/rbcsp.sqlite`.

The archive includes a direct launcher, user documentation, ISC license,
target-specific CycloneDX SBOM, complete third-party notices, provenance,
manifest, version, and hashes. Its executable uses the static MSVC runtime.

## Candidate verification

The real ZIP was extracted under a path containing spaces and Unicode. The
real executable was launched from a different working directory with Rutgers
refresh disabled by the exact CI switch. Verification then:

- confirmed an empty first-run personal state and no packaged database;
- created the database and wrote a synthetic selected Section through the
  authenticated local API;
- exited through the real local shutdown route and confirmed WAL cleanup;
- replaced only the eleven release files while preserving `data/`;
- restarted through `Start-RBCSP.bat` from a second working directory and
  recovered the same selected Section from the same database;
- confirmed a fresh session nonce and no active-watch restoration;
- confirmed no dynamic VC runtime imports.

Two independent clean worktrees produced the same `5,535,630`-byte ZIP with
SHA-256 `026370f2414cf0220461b12f75fa4f243781d7e7d6fbfe1c75069e97f1bb18d9`.
The ignored candidate is stored under `release/0.1.0`; it is not committed.

Gate: `P7_4_WINDOWS_LOCAL_ARCHIVE_PASS`.
