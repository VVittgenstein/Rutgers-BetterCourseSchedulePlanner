# P7.4-005 — Candidate clean-environment acceptance and P7.5 entry gate

- Task: `P7.4-005`
- Parent: `91744cbfbc691834f6668ca10b0b27389ce38ba1`
- Validated workflow commit: `91744cbfbc691834f6668ca10b0b27389ce38ba1`
- Frozen product source: `7d8297404d033e79b514333748b7072ebd3a0099`
- Branch: `codex/p7-implementation`
- Release version: `0.1.0`
- Verified at: `2026-07-15T06:10:45.3739833Z`
- Release set: exactly `2` archives

## Outcome

Two independent clean GitHub-hosted acceptance runs produced byte-identical
Windows and Linux candidate payloads. Each run passed its Windows standard-user,
disposable Ubuntu, real-browser fake-upstream, operating-system malware, and
package acceptance gates. Both downloaded candidate pairs independently passed
the joint release-set audit.

PreCommit gate: `P7_5_ELIGIBLE`.

`P7.5-001` remains blocked until this dedicated four-path commit is pushed and
the remote branch satisfies `P7_4_005_PASS_POST_PUSH`.

## Canonical candidate pair

| Candidate | Files | Payload bytes | SHA-256 | Clean runs | Artifact IDs |
|---|---:|---:|---|---|---|
| `rbcsp-windows-x86_64-0.1.0.zip` | 11 | 5,534,122 | `eb85374bbf97215124b4f2b64be4c51c96bc2af0502fc79b5230024709590610` | `29391985651`, `29392739883` | `8333808987`, `8334104281` |
| `rbcsp-linux-x86_64-0.1.0.tar.gz` | 20 | 6,366,047 | `77160882304fbe4d17a070a1cce16471cd618a1cd5cee18c9a2f6e9e8e920d07` | `29391985651`, `29392739883` | `8333722294`, `8334014409` |

The GitHub artifact wrapper ZIPs contain timestamped transport metadata and are
not the candidates. Reproducibility is measured on the inner release archives
listed above.

## Independent clean-environment acceptance

| Run | Event | Windows | Linux | Fake upstream | Malware |
|---:|---|---:|---:|---:|---:|
| `29391985651` | push | `87277268473` PASS | `87277268502` PASS | `87277268517` PASS | `87278986627` PASS |
| `29392739883` | workflow dispatch | `87279544529` PASS | `87279544540` PASS | `87279544528` PASS | `87281355232` PASS |

The two runs established:

- non-administrator Windows execution, read-only-root fail-fast behavior, two
  local browser scenarios, two graceful restarts, and Microsoft Defender scans;
- Ubuntu 24.04 package installation, real Caddy/browser scenarios, backup,
  restore, upgrade-failure rollback, and service-state verification;
- targeted deterministic fake-upstream product flow without Rutgers requests;
- ClamAV `1.5.3`, `3,627,896` signatures, two candidate files scanned, and zero
  infected files;
- a hash-keyed Linux candidate cache for the later manual-only P7.5 Actions tier
  while workflow permissions remain `contents: read`.

## Joint audit

Each downloaded candidate pair passed `packaging/verify-release-set.ps1`:

- package cardinality: exactly `2`;
- allowlists: Windows `11`, Linux `20` files;
- shared SBOM components: `169`;
- shared embedded frontend components: `10`;
- manifests, internal hashes, provenance, locked toolchains, notices, ISC
  license, and shared source identity: PASS;
- database, WAL/SHM, seed, preloaded real Catalog/Open data, source map, test,
  evidence, private inventory, secret, and development residue: absent;
- Windows absolute `C:\Users\` and `C:/Users/` paths: `0`;
- Windows DOS 8.3 `RUNNER~1` and `CARGO~1` paths: `0`.

## Candidate lineage

The P7.4-004 record remains immutable and valid for the exact bytes it audited;
this task does not rewrite or retroactively fail it. Its Windows
`325794d0…bf479` and Linux `3219cdba…9b40` archives were superseded by later
acceptance-driven product and tooling repairs and are not eligible for P7.5.

Run `29390164615` produced the intermediate Windows `69719b9b…cada4` and Linux
`77160882…d07` pair. Its CI jobs passed, but the subsequent joint audit rejected
that Windows archive because `RBCSP.exe` retained DOS 8.3 absolute Cargo user
paths. That Windows archive is not eligible for P7.5. The unchanged Linux hash
was rebuilt and reaccepted in both final runs.

Only the two full hashes in the canonical table are P7.5 candidates.

## Scope boundary

- Real Rutgers requests: `0`.
- Real-world E2E completed: `false`.
- P7 completed: `false`.
- GitHub Release created, authorized, or eligible: `false`.
- Vultr, staging, DNS, Cloudflare, certificate, and production mutations: `0`.
- Production deployment authorized: `false`.
- Candidate artifacts committed to Git: `false`.

The dedicated commit contains only this contract pair and its completion-record
pair. Its own SHA is intentionally excluded from the precommit record to avoid
self-reference. PostPush must prove the remote branch equals the resulting task
commit before `P7.5-001` starts.

Gate: `P7_5_ELIGIBLE`.
PostPush marker: `P7_4_005_PASS_POST_PUSH`.
Next task: `P7.5-001`.
