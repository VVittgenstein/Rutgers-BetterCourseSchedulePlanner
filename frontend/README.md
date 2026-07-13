# P7 dual-target frontend graph

This package has one active shared React source graph and two explicit target
entries:

- `src/entry.local.tsx` composes `src/ui/shared/**` with `src/ui/local/**`.
- `src/entry.public.tsx` composes `src/ui/shared/**` with `src/ui/public/**`.

The remaining historical files under `src/**` are frozen migration references.
They are deliberately outside every active TypeScript include, Vite input, and
import-graph closure. They must not be imported by either P7 target.

P7.1-003 only establishes composition and build boundaries. Product behavior,
routes, state, i18n, and visual design belong to later tasks.

## Verification

Use the pinned Node/npm toolchain and the committed lockfile:

```bash
npm ci
npm run guard
npm run typecheck:local
npm run typecheck:public
npm run build:local
npm run build:public
```

`npm run guard` parses the active sources with the pinned TypeScript compiler.
It rejects forbidden target edges, inactive or legacy imports, non-literal
dynamic imports, source-loading/code-generation escapes, unexpected source
extensions or symlinks, `import.meta.glob`, unreachable active files, and all
18 P4 public SOURCE-deny capabilities. Both Rust and frontend guards consume
the frozen `tools/architecture/p4-public-source-deny.json` audit input; it is
not a runtime capability manifest. The frontend guard also freezes the two
HTML/Vite composition roots so an extra input cannot bypass the import graph.

The local and public builds use separate HTML inputs, Vite configurations, and
content-addressed output directories under `dist/local` and `dist/public`.
Production source maps and recursive public-directory copying are disabled.
