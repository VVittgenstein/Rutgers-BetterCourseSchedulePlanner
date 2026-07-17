# P7.1-013 — Public zero-surface and dual-target build enforcement

- Task: `P7.1-013`
- Parent: `559b400ee2a48c8c11bb5616b73f56d0b106ecbb`
- Branch: `codex/p7-implementation`
- Next task after PostPush: `P7.1-014`

## Product result

Local and public frontend builds now use separate positive capability, route, and i18n allowlists. Vite rejects a resolved source module outside the selected entry, shared UI, and selected target graph before tree shaking, then emits the actual module and capability manifests into the build.

The public Rust host now has an explicit method/path inventory that drives built-in Axum route registration. Product extensions receive only method/path pairs they declare; unknown API routes return 404 before reaching an extension. The unused local-data-reset reason was removed from the shared WebSocket contract.

The frozen 18-capability public deny input is checked across exactly eight surfaces: `SOURCE`, `API`, `STORAGE`, `PACKAGE`, `DOM`, `ROUTE`, `I18N`, and `BUNDLE`. This yields 144 unique validation IDs. Checks include resolved source modules before tree shaking, rendered DOM in both locales, artifact names and bytes, Cargo graph/features, shared HTTP/WebSocket contracts, operational migrations, and current public package inputs.

Capability manifests are explicitly `PRE_UI_INTEGRATION` build allowlists. They do not claim that the search, filtering, detail, watch, or router UI is already implemented. Final unpacked archive inspection remains owned by P7.4.

## Verification

- `cargo fmt --all -- --check`: PASS
- `cargo test --workspace --all-targets --locked --offline`: PASS
- `cargo clippy --workspace --all-targets --locked --offline -- -D warnings`: PASS
- Rust graph and zero-surface guard self-tests/live checks: PASS
- `cargo deny check advisories bans licenses sources`: PASS
- `npm run verify`: PASS (`80/80` guard tests, `12/12` UI/i18n tests, typecheck, local/public builds)
- Exact validation union: PASS (`72` Rust + `72` frontend = `144`; duplicates `0`)

No Rutgers request, real course data, database artifact, credential, `.secrets/`, release, deployment, or production mutation was used or added.
