import { lstatSync, readFileSync, readdirSync } from 'node:fs';
import { dirname, extname, posix, resolve } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const toolsDirectory = dirname(fileURLToPath(import.meta.url));
const frontendRoot = resolve(toolsDirectory, '..');
const repositoryRoot = resolve(frontendRoot, '..');

const SHARED_CAPABILITIES = Object.freeze([
  'course-search',
  'course-section-details',
  'current-page-audio',
  'current-page-selection',
  'current-page-toast',
  'current-page-watch',
  'en-us-zh-cn',
  'filter-schema',
  'open-status',
  'refresh-diagnostics',
  'section-search',
]);

const EXPECTED_MANIFESTS = Object.freeze({
  local: Object.freeze({
    schemaVersion: 1,
    kind: 'TARGET_BUILD_ALLOWLIST',
    target: 'local',
    readiness: 'UI_INTEGRATION_COMPLETE',
    allowedCapabilities: Object.freeze([
      'configurable-refresh-intervals',
      ...SHARED_CAPABILITIES,
      'durable-episode-action-history',
      'local-run-counters',
      'persistent-current-filters',
      'persistent-selected-sections',
      'persistent-settings',
      'reset-local-user-data',
      'saved-views',
      'windows-launcher-lifecycle',
    ].sort()),
    allowedRoutes: Object.freeze([
      '/',
      '/history',
      '/saved-views',
      '/sections',
      '/sections/:term/:campus/:index',
      '/settings',
      '/watch',
    ]),
    allowedI18nCatalogs: Object.freeze(['shared', 'local']),
  }),
  public: Object.freeze({
    schemaVersion: 1,
    kind: 'TARGET_BUILD_ALLOWLIST',
    target: 'public',
    readiness: 'UI_INTEGRATION_COMPLETE',
    allowedCapabilities: Object.freeze([
      ...SHARED_CAPABILITIES,
      'ephemeral-document-session',
      'fixed-refresh-policy',
      'service-operations',
    ].sort()),
    allowedRoutes: Object.freeze([
      '/',
      '/sections',
      '/sections/:term/:campus/:index',
      '/watch',
    ]),
    allowedI18nCatalogs: Object.freeze(['shared']),
  }),
});

const SCANNED_SURFACES = Object.freeze(['DOM', 'ROUTE', 'I18N', 'BUNDLE']);
const TEXT_ASSET_EXTENSIONS = new Set(['.css', '.html', '.js', '.json', '.svg']);
const ALLOWED_ASSET_EXTENSIONS = new Set([
  '.avif', '.css', '.gif', '.ico', '.jpeg', '.jpg', '.js', '.json', '.mp3', '.ogg',
  '.png', '.svg', '.ttf', '.wasm', '.wav', '.webp', '.woff', '.woff2',
]);
const SURFACE_EXTENSIONS = Object.freeze({
  DOM: new Set(['.css', '.html', '.js', '.svg']),
  ROUTE: new Set(['.html', '.js', '.json']),
  I18N: new Set(['.js', '.json']),
});

// A bare `email` token is part of react-dom's standards table. Product-specific
// email markers still catch an email UI, client, route, catalog, or integration.
const AMBIGUOUS_LIBRARY_MARKERS = new Set(['email']);
const ADDITIONAL_PRODUCT_MARKERS = Object.freeze({
  EMAIL: Object.freeze([
    'contact_email',
    'email_address',
    'email_alert',
    'email_notification',
    'email_settings',
    'mail_settings',
  ]),
  LOCAL_RESET: Object.freeze(['local_user_data_reset']),
});

function cloneJson(value) {
  return JSON.parse(JSON.stringify(value));
}

function normalizePath(value) {
  return posix.normalize(value.replaceAll('\\', '/')).replace(/^\.\//u, '');
}

function normalizeToken(value) {
  return value.toLocaleLowerCase('en-US').replace(/[^a-z0-9]/gu, '');
}

function containsMarker(value, marker) {
  const normalizedMarker = normalizeToken(marker);
  if (normalizedMarker.length === 0) return false;
  const text = artifactBytesAsText(value).toLocaleLowerCase('en-US');
  // Very short platform/protocol tokens must be contiguous. Collapsing JS
  // punctuation made unrelated minified identifiers such as `o,s),Xo` spell `osx`.
  return normalizedMarker.length <= 4
    ? text.includes(normalizedMarker)
    : normalizeToken(text).includes(normalizedMarker);
}

function artifactBytesAsText(contents) {
  if (typeof contents === 'string') return contents;
  if (ArrayBuffer.isView(contents)) {
    return Buffer.from(contents.buffer, contents.byteOffset, contents.byteLength).toString('latin1');
  }
  return '';
}

function validateArtifactPaths(files, target, errors) {
  const rootFiles = new Set([
    'asset-manifest.json',
    'capability-manifest.json',
    'module-manifest.json',
    `${target}.html`,
  ]);
  for (const filePath of files.keys()) {
    if (rootFiles.has(filePath)) continue;
    if (!/^assets\/[A-Za-z0-9][A-Za-z0-9._-]*$/u.test(filePath)) {
      errors.push(`${filePath}: artifact path is outside the ${target} build allowlist`);
      continue;
    }
    const extension = extname(filePath).toLocaleLowerCase('en-US');
    if (!ALLOWED_ASSET_EXTENSIONS.has(extension)) {
      errors.push(`${filePath}: artifact extension ${extension || '(none)'} is not allowed`);
    }
  }
}

function arraysEqual(left, right) {
  return JSON.stringify(left) === JSON.stringify(right);
}

function validateManifest(manifest, target, label) {
  const errors = [];
  const expected = EXPECTED_MANIFESTS[target];
  if (manifest === null || typeof manifest !== 'object' || Array.isArray(manifest)) {
    return [`${label}: capability manifest must be an object`];
  }
  if (manifest.schemaVersion !== 1) errors.push(`${label}: unsupported schemaVersion`);
  if (manifest.kind !== 'TARGET_BUILD_ALLOWLIST') errors.push(`${label}: kind must be TARGET_BUILD_ALLOWLIST`);
  if (manifest.target !== target) errors.push(`${label}: target must be ${target}`);
  if (manifest.readiness !== 'UI_INTEGRATION_COMPLETE') {
    errors.push(`${label}: readiness must be UI_INTEGRATION_COMPLETE after the UI integration gate`);
  }

  for (const field of ['allowedCapabilities', 'allowedRoutes', 'allowedI18nCatalogs']) {
    if (!Array.isArray(manifest[field]) || manifest[field].some((value) => typeof value !== 'string')) {
      errors.push(`${label}: ${field} must be a positive string array (no boolean placeholders)`);
      continue;
    }
    if (new Set(manifest[field]).size !== manifest[field].length) {
      errors.push(`${label}: ${field} contains duplicates`);
    }
    if (!arraysEqual(manifest[field], expected[field])) {
      errors.push(`${label}: ${field} does not match the ${target} product surface`);
    }
  }
  return errors;
}

function markersFor(row) {
  return [
    ...(row.markers ?? []).filter((marker) => !AMBIGUOUS_LIBRARY_MARKERS.has(marker)),
    ...(ADDITIONAL_PRODUCT_MARKERS[row.capability] ?? []),
  ];
}

function parseAssetManifest(files, target, errors) {
  const assetManifestText = files.get('asset-manifest.json');
  if (typeof assetManifestText !== 'string') {
    errors.push('asset-manifest.json: missing build manifest');
    return;
  }
  try {
    const assetManifest = JSON.parse(assetManifestText);
    const entries = Object.entries(assetManifest).filter(([, value]) => value?.isEntry === true);
    const expectedHtml = `${target}.html`;
    if (entries.length !== 1 || entries[0]?.[0] !== expectedHtml) {
      errors.push(`asset-manifest.json: ${target} build must have exactly one ${expectedHtml} entry`);
      return;
    }
    const outputFile = entries[0][1]?.file;
    if (typeof outputFile !== 'string' || !files.has(normalizePath(outputFile))) {
      errors.push('asset-manifest.json: entry output file is missing');
    }
  } catch (error) {
    errors.push(`asset-manifest.json: invalid JSON (${error instanceof Error ? error.message : String(error)})`);
  }
}

function parseModuleManifest(files, target, errors) {
  const text = files.get('module-manifest.json');
  if (typeof text !== 'string') {
    errors.push('module-manifest.json: missing pre-tree-shaking module manifest');
    return [];
  }
  try {
    const manifest = JSON.parse(text);
    if (manifest.schemaVersion !== 1 || manifest.target !== target || !Array.isArray(manifest.modules)) {
      errors.push('module-manifest.json: invalid target module manifest');
      return [];
    }
    const modules = manifest.modules;
    if (modules.some((modulePath) => typeof modulePath !== 'string')) {
      errors.push('module-manifest.json: modules must be relative source paths');
      return [];
    }
    if (new Set(modules).size !== modules.length) {
      errors.push('module-manifest.json: modules contain duplicates');
    }
    const expectedEntry = `src/entry.${target}.tsx`;
    if (!modules.includes(expectedEntry)) {
      errors.push(`module-manifest.json: missing ${expectedEntry}`);
    }
    for (const modulePath of modules) {
      const allowed = modulePath === expectedEntry ||
        modulePath.startsWith('src/ui/shared/') ||
        modulePath.startsWith(`src/ui/${target}/`);
      if (!allowed || modulePath.includes('\\') || modulePath.includes('..')) {
        errors.push(`module-manifest.json: ${modulePath} is outside the ${target} source graph`);
      }
    }
    return modules;
  } catch (error) {
    errors.push(`module-manifest.json: invalid JSON (${error instanceof Error ? error.message : String(error)})`);
    return [];
  }
}

export function expectedManifestFor(target) {
  const expected = EXPECTED_MANIFESTS[target];
  if (expected === undefined) throw new Error(`unknown frontend target: ${target}`);
  return cloneJson(expected);
}

export function verifyTargetArtifacts({
  target,
  sourceManifest,
  builtManifest,
  files,
  denyCapabilities,
}) {
  if (!(target in EXPECTED_MANIFESTS)) throw new Error(`unknown frontend target: ${target}`);
  const normalizedFiles = new Map(
    [...files].map(([filePath, contents]) => [normalizePath(filePath), contents]),
  );
  const errors = [
    ...validateManifest(sourceManifest, target, 'source'),
    ...validateManifest(builtManifest, target, 'built'),
  ];

  if (JSON.stringify(sourceManifest) !== JSON.stringify(builtManifest)) {
    errors.push('capability-manifest.json: emitted manifest differs from the source manifest');
  }
  validateArtifactPaths(normalizedFiles, target, errors);
  if (!normalizedFiles.has(`${target}.html`)) errors.push(`${target}.html: missing target HTML`);
  if ([...normalizedFiles].some(([filePath]) => filePath.endsWith('.map'))) {
    errors.push('source maps are forbidden in target build output');
  }
  parseAssetManifest(normalizedFiles, target, errors);
  const modules = parseModuleManifest(normalizedFiles, target, errors);

  const crossTargetMarkers = target === 'public'
    ? ['entrylocal', 'localhtml', 'srcuilocal']
    : ['entrypublic', 'publichtml', 'srcuipublic'];
  const normalizedContents = new Map(
    [...normalizedFiles].map(([filePath, contents]) => [
      filePath,
      normalizeToken(artifactBytesAsText(contents)),
    ]),
  );
  for (const [filePath, contents] of normalizedContents) {
    for (const marker of crossTargetMarkers) {
      if (contents.includes(marker)) {
        errors.push(`${filePath}: ${target} build contains the other target graph marker ${marker}`);
      }
    }
  }

  const violations = [];
  const zeroSurfaceAssertions = target === 'public'
    ? SCANNED_SURFACES.flatMap((surface) =>
        denyCapabilities.map((row) => ({
          denyId: row.denyId.replace(/-SOURCE$/u, `-${surface}`),
          surface,
          validationId: row.validationId.replace(/-SOURCE$/u, `-${surface}`),
        })))
    : [];
  if (target === 'public') {
    for (const surface of SCANNED_SURFACES) {
      for (const row of denyCapabilities) {
        for (const marker of markersFor(row)) {
          for (const [filePath, contents] of normalizedContents) {
            const scansBytes = surface === 'BUNDLE';
            const scansTextAsset = SURFACE_EXTENSIONS[surface]?.has(
              extname(filePath).toLocaleLowerCase('en-US'),
            ) === true;
            if ((!scansBytes && !scansTextAsset)
              || !containsMarker(normalizedFiles.get(filePath), marker)) continue;
            violations.push({
              capability: row.capability,
              denyId: row.denyId.replace(/-SOURCE$/u, `-${surface}`),
              file: filePath,
              location: 'bytes',
              marker,
              surface,
            });
          }
        }
        if (surface === 'BUNDLE') {
          const pathMarkers = [...(row.markers ?? []), ...(ADDITIONAL_PRODUCT_MARKERS[row.capability] ?? [])];
          for (const marker of pathMarkers) {
            for (const filePath of normalizedFiles.keys()) {
              if (!containsMarker(filePath, marker)) continue;
              violations.push({
                capability: row.capability,
                denyId: row.denyId.replace(/-SOURCE$/u, '-BUNDLE'),
                file: filePath,
                location: 'path',
                marker,
                surface,
              });
            }
          }
        }
      }
    }
  }

  for (const violation of violations) {
    errors.push(
      `${violation.file}: public ${violation.surface} deny ${violation.denyId} matched ${violation.marker}`,
    );
  }

  return {
    schemaVersion: 1,
    taskId: 'P7.1-013',
    target,
    state: errors.length === 0 ? 'PASS' : 'FAIL',
    artifactFiles: [...normalizedFiles.keys()].sort(),
    allowedCapabilityCount: builtManifest?.allowedCapabilities?.length ?? 0,
    moduleCount: modules.length,
    publicZeroSurface: {
      assertionCount: zeroSurfaceAssertions.length,
      denyIds: zeroSurfaceAssertions.map(({ denyId }) => denyId),
      surfaces: target === 'public' ? [...SCANNED_SURFACES] : [],
      validationIds: zeroSurfaceAssertions.map(({ validationId }) => validationId),
      violationCount: violations.length,
      violations,
    },
    errors: [...new Set(errors)].sort(),
  };
}

function readBuildFiles(directory) {
  const files = new Map();
  const walk = (relativeDirectory = '') => {
    const absoluteDirectory = resolve(directory, relativeDirectory);
    for (const entry of readdirSync(absoluteDirectory, { withFileTypes: true })) {
      const relativePath = normalizePath(posix.join(relativeDirectory, entry.name));
      const absolutePath = resolve(directory, relativePath);
      const stats = lstatSync(absolutePath);
      if (stats.isSymbolicLink()) throw new Error(`${relativePath}: symlinks are forbidden in build output`);
      if (stats.isDirectory()) {
        walk(relativePath);
      } else if (stats.isFile()) {
        const extension = extname(relativePath);
        files.set(
          relativePath,
          TEXT_ASSET_EXTENSIONS.has(extension) || extension === '.map'
            ? readFileSync(absolutePath, 'utf8')
            : readFileSync(absolutePath),
        );
      }
    }
  };
  walk();
  return files;
}

export function verifyBuildDirectory(target, root = frontendRoot) {
  if (!(target in EXPECTED_MANIFESTS)) throw new Error(`unknown frontend target: ${target}`);
  const sourceManifest = JSON.parse(
    readFileSync(resolve(root, `build/capabilities.${target}.json`), 'utf8'),
  );
  const outputDirectory = resolve(root, `dist/${target}`);
  const files = readBuildFiles(outputDirectory);
  const builtManifestText = files.get('capability-manifest.json');
  if (typeof builtManifestText !== 'string') {
    throw new Error('capability-manifest.json: emitted target manifest is missing');
  }
  const builtManifest = JSON.parse(builtManifestText);
  const denyDocument = JSON.parse(
    readFileSync(resolve(repositoryRoot, 'tools/architecture/p4-public-source-deny.json'), 'utf8'),
  );
  return verifyTargetArtifacts({
    target,
    sourceManifest,
    builtManifest,
    files,
    denyCapabilities: denyDocument.capabilities,
  });
}

function renderHumanReport(report) {
  if (report.state === 'FAIL') {
    return [
      `P7.1-013 ${report.target} target build: FAIL`,
      ...report.errors.map((error) => `- ${error}`),
    ].join('\n');
  }
  const zeroSurface = report.target === 'public'
    ? `; public DOM/ROUTE/I18N/BUNDLE assertions: ${report.publicZeroSurface.assertionCount}`
    : '';
  return `P7.1-013 ${report.target} target build: PASS; allowed capabilities: ${report.allowedCapabilityCount}${zeroSurface}`;
}

function isMainModule() {
  return process.argv[1] !== undefined && pathToFileURL(resolve(process.argv[1])).href === import.meta.url;
}

if (isMainModule()) {
  const args = process.argv.slice(2);
  const jsonMode = args.includes('--json');
  const positional = args.filter((argument) => argument !== '--json');
  try {
    if (positional.length !== 1 || !(positional[0] in EXPECTED_MANIFESTS)) {
      throw new Error('usage: verify-target-build.mjs <local|public> [--json]');
    }
    const report = verifyBuildDirectory(positional[0]);
    process.stdout.write(`${jsonMode ? JSON.stringify(report, null, 2) : renderHumanReport(report)}\n`);
    if (report.state !== 'PASS') process.exitCode = 1;
  } catch (error) {
    const failure = {
      schemaVersion: 1,
      taskId: 'P7.1-013',
      state: 'FAIL',
      errors: [error instanceof Error ? error.message : String(error)],
    };
    process.stdout.write(`${jsonMode ? JSON.stringify(failure, null, 2) : `P7.1-013 target build: FAIL\n- ${failure.errors[0]}`}\n`);
    process.exitCode = 1;
  }
}
