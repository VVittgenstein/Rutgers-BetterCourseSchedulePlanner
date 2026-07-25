# RBCSP frontend

This package contains one shared React codebase with two explicit product
entries:

- `src/entry.local.tsx` combines shared UI with local-only personal watch
  capabilities.
- `src/entry.public.tsx` combines shared UI with the public search experience.

Target-specific composition lives under `src/ui/local/**` and
`src/ui/public/**`; reusable product UI lives under `src/ui/shared/**`.
Architecture checks prevent local-only modules and capability markers from
entering the public build.

## Commands

```bash
npm ci
npm run dev:local
npm run dev:public
npm run build:local
npm run build:public
npm run verify
```

`npm run verify` runs the import guards, unit and integration tests, TypeScript
checks, and both target-build verifiers.

The Rust runtime crates embed `dist/local` and `dist/public` in release builds.
Checked-in fallback assets allow Rust tests and development builds to run before
the frontend distributions have been generated.
