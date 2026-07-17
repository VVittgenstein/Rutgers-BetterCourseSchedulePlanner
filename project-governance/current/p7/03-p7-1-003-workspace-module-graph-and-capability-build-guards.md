# P7.1-003 workspace module graph and capability build guards

## 1. Decision and entry gate

- Record: `P7-1-003-WORKSPACE-GRAPH-2026-07-13-001`
- Task: `P7.1-003 Workspace module graph and capability build guards`
- Required predecessor: `P7.1-002` commit `1d997f6d3cca70ef54ec5b7adb2124f0b5905fa3`
- Branch: `codex/p7-implementation`
- Package impact: `BOTH_TARGETS_SOURCE_GRAPH`
- Next task after PostPush: `P7.1-004`

This task replaces the compile-only dependency scaffold with the final one-workspace source graph. It establishes package and import boundaries only. It does not implement domain models, HTTP/WS contracts, Rutgers behavior, storage behavior, runtime lifecycle, product UI, packages, or real-world tests.

In this record, `package build` means assembly of an installable end-user archive or executable package. The local/public Vite asset builds below are validation-only and do not create either final delivery package.

## 2. Rust workspace contract

The workspace contains exactly fifteen first-party packages, uses Cargo resolver `3`, and keeps the default-member set equal to the full fifteen-member workspace. The root manifest section universe is frozen to workspace, workspace package/dependency, and P7 tooling metadata sections; Cargo source override sections or assignments are rejected:

| Role | Package | Manifest |
|---|---|---|
| shared contracts | `bcsp-contracts` | `crates/bcsp-contracts/Cargo.toml` |
| shared domain | `bcsp-domain` | `crates/bcsp-domain/Cargo.toml` |
| shared query | `bcsp-query` | `crates/bcsp-query/Cargo.toml` |
| shared Rutgers client | `bcsp-rutgers-client` | `crates/bcsp-rutgers-client/Cargo.toml` |
| shared operational storage | `bcsp-operational-storage` | `crates/bcsp-operational-storage/Cargo.toml` |
| shared Catalog runtime | `bcsp-catalog` | `crates/bcsp-catalog/Cargo.toml` |
| shared Open runtime | `bcsp-open` | `crates/bcsp-open/Cargo.toml` |
| shared watch/episode | `bcsp-watch` | `crates/bcsp-watch/Cargo.toml` |
| shared application | `bcsp-application` | `crates/bcsp-application/Cargo.toml` |
| local user state | `bcsp-local-user-state` | `crates/bcsp-local-user-state/Cargo.toml` |
| local runtime adapter | `bcsp-local-runtime` | `crates/bcsp-local-runtime/Cargo.toml` |
| public operations store | `bcsp-public-operations` | `crates/bcsp-public-operations/Cargo.toml` |
| public runtime adapter | `bcsp-public-runtime` | `crates/bcsp-public-runtime/Cargo.toml` |
| Windows-local entry | `bcsp-local` | `apps/bcsp-local/Cargo.toml` |
| Linux-public entry | `bcsp-server` | `apps/bcsp-server/Cargo.toml` |

Shared packages may not reach adapters or entries. The public entry may not reach either local-only package; the local entry may not reach either public-only package. Local/public product differences may not be implemented as shared Cargo features. The temporary `rbcsp-dependency-baseline` member must be absent.

Every internal Cargo path dependency carries `version = "=0.0.0"`; path-only dependency declarations are rejected so the workspace package identity and lock graph remain mechanically reviewable.

The authoritative Rust guard is `tools/architecture/verify-rust-graph.mjs`; its self-test is `tools/architecture/verify-rust-graph.test.mjs`. The guard also walks every `src/**/*.rs` file in the `bcsp-server` dependency closure, rejects code-source escape mechanisms, and applies the shared P4 SOURCE deny input. Both scripts are build/CI tools and never enter either final package.

## 3. Frontend graph contract

There is one frontend package and one shared React source graph:

- shared root: `frontend/src/ui/shared`
- local root: `frontend/src/ui/local`
- public root: `frontend/src/ui/public`
- local entry: `frontend/src/entry.local.tsx`
- public entry: `frontend/src/entry.public.tsx`

Shared source cannot import either target root. Local cannot import public. Public cannot import local. A mixed target barrel, filesystem discovery, `import.meta.glob`, or unresolved dynamic import cannot be used to make the graph pass. Local and public have separate strict typechecks and explicit Vite builds; Node/npm/Vite remain build-only.

The authoritative frontend guard is `frontend/tools/verify-import-graph.mjs`; its self-test is `frontend/tools/verify-import-graph.test.mjs`. It also validates the two HTML roots, typed Vite descriptors/configuration, symlink and source-extension closure, code-loading mechanisms, and the frozen shared deny marker contract.

## 4. P4 source-only coverage

P7.1-003 enforces the `SOURCE` row for each of the eighteen P4 excluded capabilities before bundling across both the Rust `bcsp-server` source closure and the public React source closure. The shared, non-runtime audit input is the self-contained frozen snapshot `tools/architecture/p4-public-source-deny.json`; it carries every deny ID, capability, validation ID and marker required by both guards. Its P4 path is provenance metadata only, and no untracked or historical P4 file is required to run the guards, rebuild evidence, or validate a clean checkout. Both guards validate the snapshot's exact schema, identity and marker contract. The remaining 126 DOM, ROUTE, API, STORAGE, I18N, BUNDLE and PACKAGE rows remain owned by `P7.1-013`; this task does not claim their completion.

The eighteen capabilities are Saved views, persistent preferences, persistent history, persistent selection, persistent active watch, email, Discord, Share links, waitlist, Compact mode, macOS, native app, system notifications, Web Push, Calendar, quiet/snooze, persistent subscription identity, and local Reset.

## 5. Legacy graph disposition

The zero-consumer assertion is limited to `ACTIVE_P7_TARGET_GRAPH_ONLY`. Root Fastify, root `tsx`/backend types, and any alternate HTTP/query/persistence/i18n/Open stack must be unreachable from both P7 entries. This record makes no repository-wide zero-consumer or deletion claim and does not move later semantic parity into this architecture-only task. Retained legacy source is `FROZEN_EXCLUDED_PENDING_SEMANTIC_MIGRATION`.

## 6. Required gates

The hard gates are:

1. Rust guard and its negative self-test;
2. Windows-local and Linux-public Cargo metadata resolution;
3. locked/offline workspace check, test and Clippy, formatting, and cached advisory/bans/license/source policy;
4. frontend guard and its negative self-test;
5. independent strict local and public typechecks;
6. explicit local and public builds;
7. forbidden source reachability and all eighteen P4 SOURCE rows independently in the Rust and frontend public closures;
8. unchanged third-party dependency closure;
9. exact task allowlist, protected-worktree and publication-safety validation.

## 7. Git and publication boundary

The task uses a dedicated commit and immediate push after all gates pass. The allowlist is deliberately fail-closed until implementation paths stabilize: `03a` must be changed to `finalized=true` and populated with the exact derived path set before evidence write, staging, commit, or push.

The 167-path P7.1-001 baseline remains byte-preserved. P1 and conversation records stay opaque and unread; `.secrets/` is only checked for ignore/tracking policy and is never enumerated or read. Normal public repository source, tests, configuration, documentation, locks, guards and necessary audit evidence require no per-push confirmation. Secrets, credentials, personal information, chat logs, caches, temporary files, databases and unrelated workspace files are never eligible.

Validation accepts exactly two fail-closed worktree profiles. The shared development workspace must retain all 167 protected baseline rows with their recorded states; a genuine clean checkout must contain zero foreign dirty paths. Any partial baseline, extra foreign path, or mixture fails. The clean profile validates the baseline manifest but does not require any protected untracked file, including the P4 provenance source, to exist.

Validation order is `PreCommit -> dedicated commit -> PostCommit -> push -> PostPush`. `P7.1-004` begins only after `P7_1_003_PASS_POST_PUSH`.

## 8. Non-effects

```text
rutgers_requests=0
database_mutations=0
package_builds=0
vultr_mutations=0
release_publications=0
production_mutations=0
```
