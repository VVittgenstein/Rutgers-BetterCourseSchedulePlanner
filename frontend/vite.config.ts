import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { relative, resolve } from 'node:path';

import react from '@vitejs/plugin-react';
import type { Plugin, UserConfig } from 'vite';

import { getTargetBuild, type FrontendTarget } from './build/target-build';

const frontendRoot = fileURLToPath(new URL('.', import.meta.url));

function isAllowedTargetSource(target: FrontendTarget, modulePath: string): boolean {
  return modulePath === `src/entry.${target}.tsx` ||
    modulePath.startsWith('src/ui/shared/') ||
    modulePath.startsWith(`src/ui/${target}/`);
}

function targetManifestPlugin(target: FrontendTarget): Plugin {
  const manifestPath = resolve(frontendRoot, `build/capabilities.${target}.json`);
  const sourceModules = new Set<string>();
  return {
    name: 'bcsp-target-manifests',
    buildStart() {
      sourceModules.clear();
      this.addWatchFile(manifestPath);
    },
    moduleParsed({ id }) {
      const cleanId = id.split('?', 1)[0];
      if (cleanId === undefined) return;
      const modulePath = relative(frontendRoot, cleanId).replaceAll('\\', '/');
      if (!modulePath.startsWith('src/')) return;
      if (!isAllowedTargetSource(target, modulePath)) {
        this.error(`${modulePath} is outside the ${target} frontend source graph`);
      }
      sourceModules.add(modulePath);
    },
    generateBundle() {
      const source = readFileSync(manifestPath, 'utf8');
      JSON.parse(source);
      this.emitFile({ fileName: 'capability-manifest.json', source, type: 'asset' });
      this.emitFile({
        fileName: 'module-manifest.json',
        source: `${JSON.stringify({
          schemaVersion: 1,
          target,
          modules: [...sourceModules].sort(),
        }, null, 2)}\n`,
        type: 'asset',
      });
    },
  };
}

export function createTargetConfig(target: FrontendTarget): UserConfig {
  const descriptor = getTargetBuild(target);

  return {
    appType: 'spa',
    base: '/',
    plugins: [react(), targetManifestPlugin(target)],
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
