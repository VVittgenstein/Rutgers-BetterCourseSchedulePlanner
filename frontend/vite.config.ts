import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

import react from '@vitejs/plugin-react';
import type { UserConfig } from 'vite';

import { getTargetBuild, type FrontendTarget } from './build/target-build';

const frontendRoot = fileURLToPath(new URL('.', import.meta.url));

export function createTargetConfig(target: FrontendTarget): UserConfig {
  const descriptor = getTargetBuild(target);

  return {
    appType: 'spa',
    base: './',
    plugins: [react()],
    publicDir: false,
    root: frontendRoot,
    server: {
      port: descriptor.devPort,
      strictPort: true,
    },
    build: {
      emptyOutDir: true,
      manifest: 'asset-manifest.json',
      outDir: resolve(frontendRoot, descriptor.outDir),
      rollupOptions: {
        input: resolve(frontendRoot, descriptor.html),
      },
      sourcemap: false,
    },
  };
}
