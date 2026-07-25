import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  expectedManifestFor,
  verifyTargetArtifacts,
} from './verify-target-build.mjs';

const toolsDirectory = dirname(fileURLToPath(import.meta.url));
const denyCapabilities = JSON.parse(
  readFileSync(
    resolve(toolsDirectory, '../../tools/architecture/public-source-deny.json'),
    'utf8',
  ),
).capabilities;

function fixture(target) {
  const manifest = expectedManifestFor(target);
  const output = `assets/${target}.js`;
  return {
    target,
    sourceManifest: structuredClone(manifest),
    builtManifest: structuredClone(manifest),
    denyCapabilities,
    files: new Map([
      ['capability-manifest.json', `${JSON.stringify(manifest)}\n`],
      [
        'module-manifest.json',
        JSON.stringify({
          schemaVersion: 1,
          target,
          modules: [
            `src/entry.${target}.tsx`,
            `src/ui/${target}/${target === 'local' ? 'Local' : 'Public'}CompositionRoot.tsx`,
            'src/ui/shared/SharedApplication.tsx',
          ],
        }),
      ],
      [
        'asset-manifest.json',
        JSON.stringify({
          [`${target}.html`]: {
            file: output,
            isEntry: true,
            name: target,
            src: `${target}.html`,
          },
        }),
      ],
      [`${target}.html`, `<div id="root"></div><script src="./${output}"></script>`],
      [output, 'const application = "course search and current-page watch"; void application;'],
    ]),
  };
}

test('accepts both closed manifests and proves local capabilities survive as build output', () => {
  const local = verifyTargetArtifacts(fixture('local'));
  assert.equal(local.state, 'PASS', local.errors.join('\n'));
  assert.equal(expectedManifestFor('local').kind, 'TARGET_BUILD_ALLOWLIST');
  assert.equal(expectedManifestFor('local').readiness, 'UI_INTEGRATION_COMPLETE');
  assert.ok(expectedManifestFor('local').allowedCapabilities.includes('saved-views'));
  assert.ok(expectedManifestFor('local').allowedCapabilities.includes('reset-local-user-data'));
  assert.ok(expectedManifestFor('local').allowedCapabilities.includes('manual-term-pull'));
  assert.ok(expectedManifestFor('local').allowedRoutes.includes('/watch'));

  const publicReport = verifyTargetArtifacts(fixture('public'));
  assert.equal(publicReport.state, 'PASS', publicReport.errors.join('\n'));
  assert.equal(expectedManifestFor('public').readiness, 'UI_INTEGRATION_COMPLETE');
  assert.ok(expectedManifestFor('public').allowedRoutes.includes('/watch'));
  assert.equal(publicReport.publicZeroSurface.assertionCount, 76);
  assert.equal(new Set(publicReport.publicZeroSurface.denyIds).size, 76);
  assert.equal(new Set(publicReport.publicZeroSurface.validationIds).size, 76);
  assert.ok(publicReport.publicZeroSurface.denyIds.includes('P4-D-SAVED_VIEWS-DOM'));
  assert.ok(publicReport.publicZeroSurface.validationIds.includes('P4-Z-SAVED_VIEWS-BUNDLE'));
  assert.ok(publicReport.publicZeroSurface.denyIds.includes('RC3-D-LOCAL_TERM_PULL-DOM'));
  assert.ok(publicReport.publicZeroSurface.validationIds.includes('RC3-Z-LOCAL_TERM_PULL-BUNDLE'));
});

test('rejects Local term-pull client, DOM, i18n, and bundle markers from Public artifacts', () => {
  const input = fixture('public');
  input.files.set(
    'assets/public.js',
    'const route = "/api/v1/local/terms/pull"; const key = "scope.pull_failed";'
      + ' const flag = "manualPullAllowed"; void route; void key; void flag;',
  );
  const report = verifyTargetArtifacts(input);
  assert.equal(report.state, 'FAIL');
  for (const surface of ['DOM', 'ROUTE', 'I18N', 'BUNDLE']) {
    assert.ok(report.publicZeroSurface.violations.some(
      ({ capability, surface: actual }) => capability === 'LOCAL_TERM_PULL' && actual === surface,
    ));
  }
});

test('rejects stale pre-integration readiness after the UI integration gate', () => {
  const input = fixture('local');
  input.sourceManifest.readiness = 'PRE_UI_INTEGRATION';
  input.builtManifest.readiness = 'PRE_UI_INTEGRATION';
  const report = verifyTargetArtifacts(input);
  assert.equal(report.state, 'FAIL');
  assert.match(report.errors.join('\n'), /UI_INTEGRATION_COMPLETE/u);
});

test('rejects false placeholders in public and missing local capabilities', () => {
  const publicFixture = fixture('public');
  publicFixture.sourceManifest.allowedCapabilities = { 'saved-views': false };
  publicFixture.builtManifest.allowedCapabilities = { 'saved-views': false };
  const publicReport = verifyTargetArtifacts(publicFixture);
  assert.equal(publicReport.state, 'FAIL');
  assert.match(publicReport.errors.join('\n'), /positive string array/u);

  const localFixture = fixture('local');
  localFixture.sourceManifest.allowedCapabilities = localFixture.sourceManifest.allowedCapabilities.filter(
    (capability) => capability !== 'saved-views',
  );
  localFixture.builtManifest.allowedCapabilities = [...localFixture.sourceManifest.allowedCapabilities];
  const localReport = verifyTargetArtifacts(localFixture);
  assert.equal(localReport.state, 'FAIL');
  assert.match(localReport.errors.join('\n'), /local product surface/u);
});

test('rejects a hidden local-only DOM control in the public compiled assets', () => {
  const input = fixture('public');
  input.files.set('assets/public.css', '.saved-view-manager { display: none }');
  const report = verifyTargetArtifacts(input);
  assert.equal(report.state, 'FAIL');
  assert.ok(report.publicZeroSurface.violations.some(({ surface }) => surface === 'DOM'));
  assert.ok(report.publicZeroSurface.violations.some(({ surface }) => surface === 'BUNDLE'));
});

test('does not combine unrelated minified identifiers into a short deny token', () => {
  const input = fixture('public');
  input.files.set('assets/public.js', 'function update(o,s,Xo){return Xo(o,s)}');
  const report = verifyTargetArtifacts(input);
  assert.equal(report.state, 'PASS', report.errors.join('\n'));

  input.files.set('assets/public.js', 'const unsupportedPlatform = "osx";');
  const denied = verifyTargetArtifacts(input);
  assert.equal(denied.state, 'FAIL');
  assert.ok(denied.publicZeroSurface.violations.some(
    ({ capability, marker }) => capability === 'MACOS' && marker === 'osx',
  ));
});

test('rejects local-only public route and i18n strings after bundling', () => {
  const input = fixture('public');
  input.files.set(
    'assets/public.js',
    'const route = "/saved-views"; const messages = { "saved_view_manager.title": "Saved views" };',
  );
  const report = verifyTargetArtifacts(input);
  assert.equal(report.state, 'FAIL');
  assert.ok(report.publicZeroSurface.violations.some(({ surface }) => surface === 'ROUTE'));
  assert.ok(report.publicZeroSurface.violations.some(({ surface }) => surface === 'I18N'));
});

test('scans artifact names and every allowed binary asset for bundle markers', () => {
  const forbiddenName = fixture('public');
  forbiddenName.files.set('assets/saved-view.png', Buffer.from([0x89, 0x50, 0x4e, 0x47]));
  const nameReport = verifyTargetArtifacts(forbiddenName);
  assert.equal(nameReport.state, 'FAIL');
  assert.ok(nameReport.publicZeroSurface.violations.some(
    ({ file, location, surface }) =>
      file === 'assets/saved-view.png' && location === 'path' && surface === 'BUNDLE',
  ));

  const forbiddenBytes = fixture('public');
  forbiddenBytes.files.set(
    'assets/neutral.png',
    Buffer.from('binary-prefix local_user_data_reset binary-suffix', 'utf8'),
  );
  const bytesReport = verifyTargetArtifacts(forbiddenBytes);
  assert.equal(bytesReport.state, 'FAIL');
  assert.ok(bytesReport.publicZeroSurface.violations.some(
    ({ capability, file, location }) =>
      capability === 'LOCAL_RESET' && file === 'assets/neutral.png' && location === 'bytes',
  ));
});

test('rejects artifact extensions outside the positive allowlist', () => {
  const input = fixture('public');
  input.files.set('assets/neutral.bin', Buffer.from([0]));
  const report = verifyTargetArtifacts(input);
  assert.equal(report.state, 'FAIL');
  assert.match(report.errors.join('\n'), /artifact extension \.bin is not allowed/u);
});

test('rejects emitted-manifest drift, source maps, and the other target graph', () => {
  const input = fixture('public');
  input.builtManifest.allowedCapabilities = [
    ...input.builtManifest.allowedCapabilities,
    'extra-public-toggle',
  ];
  input.files.set('assets/public.js.map', '{}');
  input.files.set('asset-manifest.json', '{"local.html":{"file":"assets/public.js","isEntry":true}}');
  const report = verifyTargetArtifacts(input);
  assert.equal(report.state, 'FAIL');
  assert.match(report.errors.join('\n'), /differs from the source|source maps|exactly one public\.html/u);
});

test('rejects a cross-target module before tree shaking can remove it', () => {
  const input = fixture('public');
  const moduleManifest = JSON.parse(input.files.get('module-manifest.json'));
  moduleManifest.modules.push('src/ui/local/LocalCompositionRoot.tsx');
  input.files.set('module-manifest.json', JSON.stringify(moduleManifest));

  const report = verifyTargetArtifacts(input);
  assert.equal(report.state, 'FAIL');
  assert.match(report.errors.join('\n'), /src\/ui\/local.*outside the public source graph/u);
});
