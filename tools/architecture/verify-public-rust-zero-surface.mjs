#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import {
  existsSync,
  lstatSync,
  readFileSync,
  readdirSync,
  realpathSync,
} from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath, pathToFileURL } from 'node:url';

import {
  GRAPH_SPEC,
  publicClosurePackages,
  rustAuditTokens,
  validateRootCargoManifest,
  verifyGraph,
} from './verify-rust-graph.mjs';

export const PUBLIC_RUST_ZERO_SURFACES = Object.freeze(['SOURCE', 'API', 'STORAGE', 'PACKAGE']);
export const PUBLIC_RUST_SURFACE_SCOPES = Object.freeze({
  SOURCE: 'FULL_PUBLIC_RUST_CLOSURE',
  API: 'PUBLIC_HOST_ROUTES_AND_SHARED_HTTP_WS_CONTRACTS',
  STORAGE: 'FULL_PUBLIC_RUST_CLOSURE_PLUS_OPERATIONAL_MIGRATIONS',
  PACKAGE: 'PRE_PACKAGE_INPUTS',
});
const DENY_PATH = 'tools/architecture/public-source-deny.json';
const EXPECTED_CAPABILITY_COUNT = 18;
const LOCAL_ONLY_PACKAGES = Object.freeze(['bcsp-local-user-state', 'bcsp-local-runtime', 'bcsp-local']);
const SUPPLEMENTAL_MARKERS = Object.freeze({
  LOCAL_RESET: Object.freeze(['local_user_data_reset']),
});
const OPTIONAL_PUBLIC_INPUT_ROOTS = Object.freeze([
  'crates/bcsp-public-runtime/assets',
  'deploy/public',
]);

function normalizedToken(value) {
  return String(value).toLocaleLowerCase('en-US').replace(/[^a-z0-9]/gu, '');
}

function plainTokens(source) {
  const tokens = new Set();
  for (const line of source.split(/\r?\n/gu)) {
    for (const match of line.matchAll(/[A-Za-z][A-Za-z0-9_]*/gu)) {
      tokens.add(normalizedToken(match[0]));
    }
    const composite = normalizedToken(line);
    if (composite) tokens.add(composite);
  }
  return tokens;
}

function surfaceTokens(file, source) {
  if (path.extname(file).toLocaleLowerCase('en-US') === '.rs') return rustAuditTokens(source);
  return plainTokens(source);
}

function validateCapabilityRows(document) {
  if (!Array.isArray(document?.capabilities) || document.capabilities.length !== EXPECTED_CAPABILITY_COUNT) {
    return [`public capability input must contain ${EXPECTED_CAPABILITY_COUNT} rows`];
  }
  if (document.capabilities.some((row) => (
    typeof row?.denyId !== 'string'
    || !row.denyId.endsWith('-SOURCE')
    || typeof row.validationId !== 'string'
    || !row.validationId.endsWith('-SOURCE')
    || !Array.isArray(row.markers)
    || row.markers.length === 0
  ))) {
    return ['public capability rows require SOURCE deny/validation IDs and markers'];
  }
  return [];
}

export function auditSurface(files, denyDocument, surface) {
  const inputErrors = validateCapabilityRows(denyDocument);
  if (!PUBLIC_RUST_ZERO_SURFACES.includes(surface)) {
    inputErrors.push(`unsupported public Rust zero surface: ${surface}`);
  }
  const violations = [];
  if (inputErrors.length === 0) {
    for (const [file, source] of [...files.entries()].sort(([left], [right]) => left.localeCompare(right, 'en-US'))) {
      const tokens = surfaceTokens(file, source);
      if (surface === 'PACKAGE') {
        for (const token of plainTokens(file)) tokens.add(token);
      }
      for (const row of denyDocument.capabilities) {
        const markers = [...row.markers, ...(SUPPLEMENTAL_MARKERS[row.capability] ?? [])];
        for (const marker of markers) {
          const normalizedMarker = normalizedToken(marker);
          const token = [...tokens].find((candidate) => candidate.includes(normalizedMarker));
          if (!token) continue;
          violations.push({
            capability: row.capability,
            denyId: row.denyId.replace(/-SOURCE$/u, `-${surface}`),
            file,
            marker,
          });
          break;
        }
      }
    }
  }
  const errors = [
    ...inputErrors,
    ...violations.map((violation) =>
      `${violation.file}: public ${surface} deny ${violation.denyId} matched ${violation.marker}`),
  ];
  const denyIds = inputErrors.length === 0
    ? denyDocument.capabilities.map((row) => row.denyId.replace(/-SOURCE$/u, `-${surface}`))
    : [];
  const validationIds = inputErrors.length === 0
    ? denyDocument.capabilities.map((row) => row.validationId.replace(/-SOURCE$/u, `-${surface}`))
    : [];
  return {
    checkedCount: inputErrors.length === 0 ? EXPECTED_CAPABILITY_COUNT : 0,
    denyIds,
    errors,
    expectedCount: EXPECTED_CAPABILITY_COUNT,
    fileCount: files.size,
    scope: PUBLIC_RUST_SURFACE_SCOPES[surface],
    validationIds,
    violationCount: violations.length,
    violations,
  };
}

export function auditPublicRustZeroSurfaces(surfaceFiles, denyDocument, { packageInputRoots = [] } = {}) {
  const surfaces = {};
  const errors = [];
  for (const surface of PUBLIC_RUST_ZERO_SURFACES) {
    const audit = auditSurface(surfaceFiles[surface] ?? new Map(), denyDocument, surface);
    if (surface === 'PACKAGE') {
      audit.inputRoots = packageInputRoots;
    }
    surfaces[surface] = audit;
    errors.push(...audit.errors);
  }
  return {
    checkedCount: Object.values(surfaces).reduce((sum, audit) => sum + audit.checkedCount, 0),
    errors: [...new Set(errors)].sort(),
    expectedCount: EXPECTED_CAPABILITY_COUNT * PUBLIC_RUST_ZERO_SURFACES.length,
    supplementalMarkers: SUPPLEMENTAL_MARKERS,
    surfaces,
  };
}

function relativePath(root, value) {
  return path.relative(root, value).replaceAll('\\', '/');
}

function collectTree(directory, repoRoot, files, errors, accepts) {
  let entries;
  try {
    const stat = lstatSync(directory);
    if (stat.isSymbolicLink() || !stat.isDirectory()) {
      errors.push(`${relativePath(repoRoot, directory)}: expected a real directory`);
      return;
    }
    entries = readdirSync(directory, { withFileTypes: true });
  } catch (error) {
    errors.push(`${relativePath(repoRoot, directory)}: cannot enumerate: ${error instanceof Error ? error.message : String(error)}`);
    return;
  }
  for (const entry of entries.sort((left, right) => left.name.localeCompare(right.name, 'en-US'))) {
    const absolute = path.join(directory, entry.name);
    const relative = relativePath(repoRoot, absolute);
    if (entry.isSymbolicLink()) {
      errors.push(`${relative}: symlink is forbidden in public package inputs`);
    } else if (entry.isDirectory()) {
      collectTree(absolute, repoRoot, files, errors, accepts);
    } else if (entry.isFile() && accepts(absolute)) {
      try {
        if (path.resolve(realpathSync.native(absolute)) !== path.resolve(absolute)) {
          errors.push(`${relative}: real path differs from declared path`);
        } else {
          files.set(relative, readFileSync(absolute, 'utf8'));
        }
      } catch (error) {
        errors.push(`${relative}: cannot read: ${error instanceof Error ? error.message : String(error)}`);
      }
    }
  }
}

function readInput(file, repoRoot, files, errors) {
  const relative = relativePath(repoRoot, file);
  try {
    const stat = lstatSync(file);
    if (stat.isSymbolicLink() || !stat.isFile() || path.resolve(realpathSync.native(file)) !== path.resolve(file)) {
      errors.push(`${relative}: expected a real regular file`);
      return;
    }
    files.set(relative, readFileSync(file, 'utf8'));
  } catch (error) {
    errors.push(`${relative}: cannot read: ${error instanceof Error ? error.message : String(error)}`);
  }
}

export function collectSurfaceFiles(metadata, repoRoot) {
  const errors = [];
  const closure = publicClosurePackages(metadata);
  const rustFiles = new Map();
  const manifests = new Map();
  const packageInputRoots = [
    ...closure.flatMap((packageName) => {
      const directory = GRAPH_SPEC[packageName]?.dir;
      return directory ? [`${directory}/src`, `${directory}/Cargo.toml`] : [];
    }),
    'crates/bcsp-operational-storage/migrations',
    'Cargo.lock',
  ];
  for (const packageName of closure) {
    const spec = GRAPH_SPEC[packageName];
    if (!spec) {
      errors.push(`public closure contains unknown package ${packageName}`);
      continue;
    }
    const packageRoot = path.join(repoRoot, spec.dir);
    collectTree(
      path.join(packageRoot, 'src'),
      repoRoot,
      rustFiles,
      errors,
      (file) => path.extname(file).toLocaleLowerCase('en-US') === '.rs',
    );
    readInput(path.join(packageRoot, 'Cargo.toml'), repoRoot, manifests, errors);
  }

  const apiFiles = new Map([...rustFiles].filter(([file]) => (
    file.startsWith('crates/bcsp-contracts/src/')
    || file.startsWith('crates/bcsp-public-runtime/src/')
    || [
      'crates/bcsp-application/src/host.rs',
      'crates/bcsp-application/src/query_service.rs',
      'crates/bcsp-application/src/watch_socket.rs',
    ].includes(file)
  )));
  readInput(
    path.join(repoRoot, 'crates/bcsp-contracts/tests/golden/contract-manifest-v1.json'),
    repoRoot,
    apiFiles,
    errors,
  );

  const storageFiles = new Map(rustFiles);
  collectTree(
    path.join(repoRoot, 'crates/bcsp-operational-storage/migrations'),
    repoRoot,
    storageFiles,
    errors,
    (file) => path.extname(file).toLocaleLowerCase('en-US') === '.sql',
  );

  const packageFiles = new Map([...rustFiles, ...storageFiles, ...manifests]);
  readInput(path.join(repoRoot, 'Cargo.lock'), repoRoot, packageFiles, errors);
  for (const optionalRoot of OPTIONAL_PUBLIC_INPUT_ROOTS) {
    const absolute = path.join(repoRoot, optionalRoot);
    if (!existsSync(absolute)) continue;
    packageInputRoots.push(optionalRoot);
    collectTree(absolute, repoRoot, packageFiles, errors, () => true);
  }
  return {
    closure,
    errors,
    packageInputRoots,
    surfaceFiles: {
      SOURCE: rustFiles,
      API: apiFiles,
      STORAGE: storageFiles,
      PACKAGE: packageFiles,
    },
  };
}

export function createReport({ audit, closure, errors, metadata }) {
  const packagesById = new Map((metadata.packages ?? []).map((pkg) => [pkg.id, pkg]));
  const workspacePackages = (metadata.workspace_members ?? []).map((id) => packagesById.get(id)).filter(Boolean);
  const declaredFeatureCount = workspacePackages.reduce(
    (sum, pkg) => sum + Object.keys(pkg.features ?? {}).length,
    0,
  );
  const closureSet = new Set(closure);
  return {
    schemaVersion: 1,
    check: 'P7.1-013_PUBLIC_RUST_ZERO_SURFACE',
    ok: errors.length === 0,
    cargoGuard: {
      declaredFeatureCount,
      localOnlyPackagesAbsent: LOCAL_ONLY_PACKAGES.every((name) => !closureSet.has(name)),
      publicClosurePackages: closure,
    },
    publicRustZeroSurface: audit,
    errors,
  };
}

export function formatReport(report, jsonMode = false) {
  if (jsonMode) return `${JSON.stringify(report)}\n`;
  if (!report.ok) return `FAIL ${report.check}\n${report.errors.map((error) => `- ${error}`).join('\n')}\n`;
  const counts = PUBLIC_RUST_ZERO_SURFACES
    .map((surface) => `${surface}=${report.publicRustZeroSurface.surfaces[surface].checkedCount}/${EXPECTED_CAPABILITY_COUNT}`)
    .join(', ');
  return `PASS ${report.check}: ${counts}, Cargo closure=${report.cargoGuard.publicClosurePackages.length}, features=${report.cargoGuard.declaredFeatureCount}\n`;
}

async function main() {
  const args = process.argv.slice(2);
  const jsonMode = args.includes('--json');
  const unknown = args.filter((argument) => argument !== '--json');
  const scriptDir = path.dirname(fileURLToPath(import.meta.url));
  const repoRoot = path.resolve(scriptDir, '../..');
  let metadata = { packages: [], workspace_members: [] };
  let closure = [];
  let audit = auditPublicRustZeroSurfaces({}, null);
  let errors = unknown.length > 0 ? [`unknown argument(s): ${unknown.join(', ')}`] : [];

  if (errors.length === 0) {
    try {
      errors.push(...validateRootCargoManifest(readFileSync(path.join(repoRoot, 'Cargo.toml'), 'utf8')));
    } catch (error) {
      errors.push(`Cargo.toml: cannot read: ${error instanceof Error ? error.message : String(error)}`);
    }
    const cargo = process.env.CARGO || 'cargo';
    const result = spawnSync(cargo, ['metadata', '--format-version', '1', '--no-deps', '--locked', '--offline'], {
      cwd: repoRoot,
      encoding: 'utf8',
      shell: false,
      windowsHide: true,
    });
    if (result.error) {
      errors.push(`cargo metadata failed to start: ${result.error.message}`);
    } else if (result.status !== 0) {
      errors.push(`cargo metadata failed (exit ${result.status}): ${(result.stderr || '').trim()}`);
    } else {
      try {
        metadata = JSON.parse(result.stdout);
        errors.push(...verifyGraph(metadata, { repoRoot }));
        const collected = collectSurfaceFiles(metadata, repoRoot);
        closure = collected.closure;
        errors.push(...collected.errors);
        const denyDocument = JSON.parse(readFileSync(path.join(repoRoot, DENY_PATH), 'utf8'));
        audit = auditPublicRustZeroSurfaces(collected.surfaceFiles, denyDocument, {
          packageInputRoots: collected.packageInputRoots,
        });
        errors.push(...audit.errors);
      } catch (error) {
        errors.push(`public Rust zero-surface inspection failed: ${error instanceof Error ? error.message : String(error)}`);
      }
    }
  }

  const report = createReport({
    audit,
    closure,
    errors: [...new Set(errors)].sort(),
    metadata,
  });
  process.stdout.write(formatReport(report, jsonMode));
  process.exitCode = report.ok ? 0 : 1;
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : '';
if (import.meta.url === invokedPath) await main();
