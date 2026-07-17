# P7.1-009 — Shared en-US / zh-CN and error-key contract

- Status: `COMPLETE / PASS`
- Parent: `820a2b2d4fe0195271815f43f42e41dba8908362`
- Branch: `codex/p7-implementation`
- Verified: `2026-07-14T12:25:32.4976888Z`

## Product delivered

- One typed shared message catalog for `en-US` and `zh-CN`, with `en-US` as the deterministic fallback.
- Exact localized coverage for the 11 API error keys, seven match-reason keys, and 22 active filter keys.
- Independent locale runtimes for local and public composition roots, including HTML language, number, and date formatting.
- Public locale negotiation remains browser-only; the local root accepts an injected saved locale and supports system-locale redetection without Web Storage.
- Rust and TypeScript share a portable golden contract for locales, error keys, and match-reason keys.

## Verification

| Gate | Result |
|---|---|
| Rust i18n contract | `1 new; 87 package tests total` |
| Frontend i18n/runtime | `9 passed` |
| Frontend import guard | `72 passed` |
| Frontend typecheck + local/public builds | `PASS` |
| Workspace fmt/test/clippy | `PASS` |
| Architecture graph + cargo-deny | `PASS` |

## Boundary

This task adds localization infrastructure, not the visual UI implementation or UI polish tasks. It adds no local persistence implementation, Web Storage use, live Rutgers request, package, deployment, release, or production change.
