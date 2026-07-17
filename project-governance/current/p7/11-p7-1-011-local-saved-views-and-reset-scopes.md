# P7.1-011 — Local Saved views and reset scopes

- Task: `P7.1-011`
- Parent: `f134506bbddedff29e30b797ed2f1b44ec1f7176`
- Branch: `codex/p7-implementation`
- Next task after PostPush: `P7.1-012`

## Product result

The Windows-local runtime now provides versioned Saved view snapshots with create, list, apply, rename, update, duplicate, and delete operations. Mutations use optimistic revisions, names are trimmed and case-insensitively unique, no product count cap or eviction is imposed, and full storage is reported explicitly.

Snapshots use stable filter IDs. Newly introduced optional fields receive the shared canonical neutral value, while unknown, removed, malformed, or future-schema data remains preserved as raw incompatible content and is never executed as a query. The current filters retain their applied-view association and derive `CLEAN` or `MODIFIED` from canonical content comparison.

Three reset scopes remain distinct: filter reset clears only current filters; delete-all clears only the Saved view library while preserving and detaching current filters; confirmed local-user-data reset first stops watches, then atomically clears personal settings, current filters, Saved views, selections, and history. Operational Catalog/Open data and application schema are preserved. The destructive reset uses a short-lived single-use token and returns a new user-state revision.

No URL/share restore, cloud sync, import/export, or public-runtime personal persistence was added.

## Verification

- `cargo fmt --all -- --check`: PASS
- `cargo test --workspace --locked --offline`: PASS
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: PASS
- Rust architecture graph: PASS (`15` members, both binaries, `18/18` public source denies)
- `cargo deny check advisories bans licenses sources`: PASS
- `git diff --check`: PASS

The dedicated commit contains 26 product, test, migration, contract-golden, and task-record paths. It excludes databases, real Rutgers data, credentials, `.secrets/`, chat logs, caches, binaries, P1 material, and unrelated protected workspace files.
