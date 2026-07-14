#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  lstatSync,
  readFileSync,
  readdirSync,
  realpathSync,
} from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath, pathToFileURL } from 'node:url';

const NORMAL = 'normal';
const DEV = 'dev';
const CRATES_IO_SOURCE = 'registry+https://github.com/rust-lang/crates.io-index';
const PUBLIC_SOURCE_DENY_PATH = 'tools/architecture/p4-public-source-deny.json';
const PUBLIC_SOURCE_DENY_SEMANTIC_SHA256 = 'D0AAFC7B4EB7FF82859C9FA95F1D1EDB7C28BAC934DF751C6FB361A4DEC8CEB0';
const PUBLIC_SOURCE_MARKER_COUNT = 212;

const external = (req, usesDefaultFeatures, features = []) => Object.freeze({
  features: Object.freeze(features),
  req,
  source: CRATES_IO_SOURCE,
  usesDefaultFeatures,
});

export const EXTERNAL_DEPENDENCY_SPEC = Object.freeze({
  axum: external('=0.8.9', false, ['http1', 'json', 'tokio', 'ws']),
  include_dir: external('=0.7.4', false),
  jiff: external('=0.2.32', false, ['serde', 'std', 'tzdb-bundle-always']),
  open: external('=5.4.0', false),
  proptest: external('=1.11.0', false, ['bit-set', 'std']),
  reqwest: external('=0.13.4', false, ['gzip', 'json', 'query', 'rustls']),
  rusqlite: external('=0.40.1', false, ['bundled']),
  serde: external('=1.0.228', true, ['derive']),
  serde_json: external('=1.0.150', true),
  sha2: external('=0.10.9', false),
  tempfile: external('=3.27.0', true),
  thiserror: external('=2.0.18', true),
  time: external('=0.3.53', false, ['formatting', 'macros', 'parsing', 'serde', 'std']),
  tokio: external('=1.52.3', false, ['macros', 'net', 'rt-multi-thread', 'signal', 'sync', 'time']),
  tower: external('=0.5.3', false, ['util']),
  'tower-http': external('=0.6.11', false, ['compression-gzip', 'fs', 'limit', 'trace']),
  tracing: external('=0.1.44', false, ['attributes', 'std']),
  'tracing-subscriber': external('=0.3.23', false, ['env-filter', 'fmt', 'json', 'std']),
  uuid: external('=1.23.5', false, ['serde', 'std', 'v4']),
});

export const GRAPH_SPEC = Object.freeze({
  'bcsp-contracts': {
    dir: 'crates/bcsp-contracts',
    kind: 'lib',
    internal: [],
    external: [['serde', NORMAL], ['serde_json', NORMAL], ['thiserror', NORMAL], ['time', NORMAL], ['uuid', NORMAL]],
  },
  'bcsp-domain': {
    dir: 'crates/bcsp-domain',
    kind: 'lib',
    internal: ['bcsp-contracts'],
    external: [['jiff', NORMAL], ['proptest', DEV], ['serde', NORMAL], ['thiserror', NORMAL], ['time', NORMAL], ['uuid', NORMAL]],
  },
  'bcsp-query': {
    dir: 'crates/bcsp-query',
    kind: 'lib',
    internal: ['bcsp-contracts', 'bcsp-domain'],
    external: [['proptest', DEV], ['serde', NORMAL], ['thiserror', NORMAL], ['time', NORMAL]],
  },
  'bcsp-rutgers-client': {
    dir: 'crates/bcsp-rutgers-client',
    kind: 'lib',
    internal: ['bcsp-contracts'],
    external: [['reqwest', NORMAL], ['serde', NORMAL], ['serde_json', NORMAL], ['sha2', NORMAL], ['thiserror', NORMAL], ['tokio', NORMAL], ['tracing', NORMAL]],
  },
  'bcsp-operational-storage': {
    dir: 'crates/bcsp-operational-storage',
    kind: 'lib',
    internal: ['bcsp-contracts', 'bcsp-domain'],
    external: [['include_dir', NORMAL], ['rusqlite', NORMAL], ['serde', NORMAL], ['serde_json', NORMAL], ['sha2', NORMAL], ['tempfile', DEV], ['thiserror', NORMAL], ['time', NORMAL], ['tracing', NORMAL]],
  },
  'bcsp-catalog': {
    dir: 'crates/bcsp-catalog',
    kind: 'lib',
    internal: ['bcsp-contracts', 'bcsp-domain', 'bcsp-operational-storage', 'bcsp-rutgers-client'],
    external: [['serde', NORMAL], ['serde_json', NORMAL], ['tempfile', DEV], ['thiserror', NORMAL], ['time', NORMAL], ['tokio', NORMAL], ['tracing', NORMAL]],
  },
  'bcsp-open': {
    dir: 'crates/bcsp-open',
    kind: 'lib',
    internal: ['bcsp-contracts', 'bcsp-domain', 'bcsp-operational-storage', 'bcsp-rutgers-client'],
    external: [['jiff', NORMAL], ['proptest', DEV], ['serde', NORMAL], ['thiserror', NORMAL], ['time', NORMAL], ['tokio', NORMAL], ['tracing', NORMAL]],
  },
  'bcsp-watch': {
    dir: 'crates/bcsp-watch',
    kind: 'lib',
    internal: ['bcsp-contracts', 'bcsp-domain', 'bcsp-open'],
    external: [['proptest', DEV], ['serde', NORMAL], ['serde_json', NORMAL], ['thiserror', NORMAL], ['time', NORMAL], ['tokio', NORMAL], ['tracing', NORMAL], ['uuid', NORMAL]],
  },
  'bcsp-application': {
    dir: 'crates/bcsp-application',
    kind: 'lib',
    internal: ['bcsp-catalog', 'bcsp-contracts', 'bcsp-domain', 'bcsp-open', 'bcsp-operational-storage', 'bcsp-query', 'bcsp-rutgers-client', 'bcsp-watch'],
    external: [['axum', NORMAL], ['serde', NORMAL], ['serde_json', NORMAL], ['tempfile', DEV], ['thiserror', NORMAL], ['time', NORMAL], ['tokio', NORMAL], ['tower', NORMAL], ['tower-http', NORMAL], ['tracing', NORMAL]],
  },
  'bcsp-local-user-state': {
    dir: 'crates/bcsp-local-user-state',
    kind: 'lib',
    internal: ['bcsp-contracts', 'bcsp-domain'],
    external: [['rusqlite', NORMAL], ['serde', NORMAL], ['serde_json', NORMAL], ['sha2', NORMAL], ['tempfile', DEV], ['thiserror', NORMAL], ['tracing', NORMAL]],
  },
  'bcsp-local-runtime': {
    dir: 'crates/bcsp-local-runtime',
    kind: 'lib',
    internal: ['bcsp-application', 'bcsp-contracts', 'bcsp-local-user-state', 'bcsp-open', 'bcsp-operational-storage', 'bcsp-watch'],
    external: [['include_dir', NORMAL], ['open', NORMAL], ['rusqlite', DEV], ['serde', NORMAL], ['serde_json', NORMAL], ['tempfile', DEV], ['thiserror', NORMAL], ['tokio', NORMAL], ['tracing', NORMAL], ['tracing-subscriber', NORMAL]],
  },
  'bcsp-public-operations': {
    dir: 'crates/bcsp-public-operations',
    kind: 'lib',
    internal: ['bcsp-operational-storage'],
    external: [['rusqlite', NORMAL], ['tempfile', DEV], ['thiserror', NORMAL], ['tracing', NORMAL]],
  },
  'bcsp-public-runtime': {
    dir: 'crates/bcsp-public-runtime',
    kind: 'lib',
    internal: ['bcsp-application', 'bcsp-contracts', 'bcsp-open', 'bcsp-operational-storage', 'bcsp-public-operations', 'bcsp-watch'],
    external: [['axum', NORMAL], ['include_dir', NORMAL], ['reqwest', DEV], ['serde', NORMAL], ['serde_json', NORMAL], ['tempfile', DEV], ['thiserror', NORMAL], ['time', NORMAL], ['tokio', NORMAL], ['tower', DEV], ['tracing', NORMAL], ['tracing-subscriber', NORMAL]],
  },
  'bcsp-local': {
    dir: 'apps/bcsp-local',
    kind: 'bin',
    internal: ['bcsp-local-runtime'],
    external: [],
  },
  'bcsp-server': {
    dir: 'apps/bcsp-server',
    kind: 'bin',
    internal: ['bcsp-public-runtime'],
    external: [['tokio', NORMAL], ['tracing', NORMAL]],
  },
});

export const EXPECTED_PUBLIC_SOURCE_DENIES = Object.freeze([
  ['P4-D-SAVED_VIEWS-SOURCE', 'SAVED_VIEWS'],
  ['P4-D-PERSISTENT_PREFS-SOURCE', 'PERSISTENT_PREFS'],
  ['P4-D-PERSISTENT_HISTORY-SOURCE', 'PERSISTENT_HISTORY'],
  ['P4-D-PERSISTENT_SELECTION-SOURCE', 'PERSISTENT_SELECTION'],
  ['P4-D-PERSISTENT_ACTIVE_WATCH-SOURCE', 'PERSISTENT_ACTIVE_WATCH'],
  ['P4-D-EMAIL-SOURCE', 'EMAIL'],
  ['P4-D-DISCORD-SOURCE', 'DISCORD'],
  ['P4-D-SHARE_LINKS-SOURCE', 'SHARE_LINKS'],
  ['P4-D-WAITLIST-SOURCE', 'WAITLIST'],
  ['P4-D-COMPACT-SOURCE', 'COMPACT'],
  ['P4-D-MACOS-SOURCE', 'MACOS'],
  ['P4-D-NATIVE_APP-SOURCE', 'NATIVE_APP'],
  ['P4-D-SYSTEM_NOTIFICATIONS-SOURCE', 'SYSTEM_NOTIFICATIONS'],
  ['P4-D-WEB_PUSH-SOURCE', 'WEB_PUSH'],
  ['P4-D-CALENDAR-SOURCE', 'CALENDAR'],
  ['P4-D-QUIET_SNOOZE-SOURCE', 'QUIET_SNOOZE'],
  ['P4-D-PERSISTENT_SUBSCRIPTION_IDENTITY-SOURCE', 'PERSISTENT_SUBSCRIPTION_IDENTITY'],
  ['P4-D-LOCAL_RESET-SOURCE', 'LOCAL_RESET'],
]);

const MEMBER_NAMES = Object.freeze(Object.keys(GRAPH_SPEC));
const ROOT_MANIFEST_SECTION_NAMES = Object.freeze([
  'workspace',
  'workspace.package',
  'workspace.dependencies',
  'workspace.metadata.p7.tooling',
]);
const MEMBER_SET = new Set(MEMBER_NAMES);
const SHARED = new Set([
  'bcsp-contracts',
  'bcsp-domain',
  'bcsp-query',
  'bcsp-rutgers-client',
  'bcsp-operational-storage',
  'bcsp-catalog',
  'bcsp-open',
  'bcsp-watch',
  'bcsp-application',
]);
const LOCAL_ONLY = new Set(['bcsp-local-user-state', 'bcsp-local-runtime']);
const PUBLIC_ONLY = new Set(['bcsp-public-operations', 'bcsp-public-runtime']);
const EMBEDDED_WEB_BUILD_PACKAGES = new Set(['bcsp-local-runtime', 'bcsp-public-runtime']);

function normalizedPath(value) {
  return path.resolve(value).replaceAll('\\', '/');
}

function relativePath(root, value) {
  return path.relative(root, value).replaceAll('\\', '/');
}

function dependencyKind(dependency) {
  if (dependency.kind === null) return NORMAL;
  if (dependency.kind === DEV) return DEV;
  return null;
}

function pairKey(name, kind) {
  return `${name}:${kind}`;
}

function sameSet(actual, expected) {
  return actual.size === expected.size && [...actual].every((value) => expected.has(value));
}

function sameStringArrayAsSet(actual, expected) {
  return Array.isArray(actual)
    && actual.length === expected.length
    && sameSet(new Set(actual), new Set(expected));
}

function reachableFrom(start, edges, includeStart = false) {
  const visited = new Set(includeStart ? [start] : []);
  const pending = [...(edges.get(start) ?? [])];
  while (pending.length > 0) {
    const current = pending.pop();
    if (visited.has(current)) continue;
    visited.add(current);
    pending.push(...(edges.get(current) ?? []));
  }
  return visited;
}

function internalEdges(metadata) {
  const packagesById = new Map((metadata.packages ?? []).map((pkg) => [pkg.id, pkg]));
  const edges = new Map(MEMBER_NAMES.map((name) => [name, new Set()]));
  for (const id of metadata.workspace_members ?? []) {
    const pkg = packagesById.get(id);
    if (!pkg || !edges.has(pkg.name)) continue;
    for (const dependency of pkg.dependencies ?? []) {
      if (MEMBER_SET.has(dependency.name)) edges.get(pkg.name).add(dependency.name);
    }
  }
  return edges;
}

export function publicClosurePackages(metadata) {
  const closure = reachableFrom('bcsp-server', internalEdges(metadata), true);
  return MEMBER_NAMES.filter((name) => closure.has(name));
}

function findCycle(edges) {
  const indegree = new Map(MEMBER_NAMES.map((name) => [name, 0]));
  for (const dependencies of edges.values()) {
    for (const dependency of dependencies) {
      indegree.set(dependency, (indegree.get(dependency) ?? 0) + 1);
    }
  }
  const pending = [...indegree].filter(([, count]) => count === 0).map(([name]) => name);
  let visited = 0;
  while (pending.length > 0) {
    const current = pending.pop();
    visited += 1;
    for (const dependency of edges.get(current) ?? []) {
      const next = indegree.get(dependency) - 1;
      indegree.set(dependency, next);
      if (next === 0) pending.push(dependency);
    }
  }
  return visited === MEMBER_NAMES.length ? [] : [...indegree].filter(([, count]) => count > 0).map(([name]) => name).sort();
}

function describe(value) {
  return value === undefined ? 'undefined' : JSON.stringify(value);
}

function validateDependencyCommon(packageName, dependency, expectedKind, expectedFeatures, expectedDefault, errors) {
  const expectedRawKind = expectedKind === NORMAL ? null : DEV;
  if (dependency.kind !== expectedRawKind) {
    errors.push(`${packageName} -> ${dependency.name} dependency kind mismatch: expected ${describe(expectedRawKind)}, got ${describe(dependency.kind)}`);
  }
  if (dependency.optional !== false) errors.push(`${packageName} -> ${dependency.name} must not be optional`);
  if (dependency.target !== null) errors.push(`${packageName} -> ${dependency.name} must not be target-qualified`);
  if (dependency.rename !== null) errors.push(`${packageName} -> ${dependency.name} must not be renamed`);
  if (dependency.registry !== null) errors.push(`${packageName} -> ${dependency.name} registry override must be null`);
  if (!sameStringArrayAsSet(dependency.features, expectedFeatures)) {
    errors.push(`${packageName} -> ${dependency.name} dependency features mismatch`);
  }
  if (dependency.uses_default_features !== expectedDefault) {
    errors.push(`${packageName} -> ${dependency.name} default-features mismatch`);
  }
}

function validateInternalDependency(packageName, dependency, repoRoot, workspaceVersion, errors) {
  const targetSpec = GRAPH_SPEC[dependency.name];
  validateDependencyCommon(packageName, dependency, NORMAL, [], true, errors);
  if (dependency.source !== null) errors.push(`${packageName} -> ${dependency.name} internal source must be null`);
  const expectedRequirement = `=${workspaceVersion}`;
  if (dependency.req !== expectedRequirement) {
    errors.push(`${packageName} -> ${dependency.name} internal version requirement must be ${expectedRequirement}`);
  }
  const expectedPath = normalizedPath(path.join(repoRoot, targetSpec.dir));
  if (typeof dependency.path !== 'string' || normalizedPath(dependency.path) !== expectedPath) {
    errors.push(`${packageName} -> ${dependency.name} internal path mismatch`);
  }
}

function validateExternalDependency(packageName, dependency, expectedKind, errors) {
  const expected = EXTERNAL_DEPENDENCY_SPEC[dependency.name];
  if (!expected) {
    errors.push(`${packageName} -> ${dependency.name} has no frozen external dependency contract`);
    return;
  }
  validateDependencyCommon(
    packageName,
    dependency,
    expectedKind,
    expected.features,
    expected.usesDefaultFeatures,
    errors,
  );
  if (dependency.source !== expected.source) errors.push(`${packageName} -> ${dependency.name} registry source mismatch`);
  if (dependency.req !== expected.req) errors.push(`${packageName} -> ${dependency.name} version requirement mismatch`);
  if ((dependency.path ?? null) !== null) errors.push(`${packageName} -> ${dependency.name} external path must be null`);
}

export function verifyGraph(metadata, { repoRoot } = {}) {
  const errors = [];
  const root = normalizedPath(repoRoot ?? metadata.workspace_root ?? process.cwd());
  const packagesById = new Map((metadata.packages ?? []).map((pkg) => [pkg.id, pkg]));
  const workspacePackages = (metadata.workspace_members ?? []).map((id) => packagesById.get(id)).filter(Boolean);
  const actualNames = new Set(workspacePackages.map((pkg) => pkg.name));
  const workspaceVersions = new Set(workspacePackages.map((pkg) => pkg.version));
  const workspaceVersion = workspaceVersions.size === 1 ? [...workspaceVersions][0] : undefined;

  if ((metadata.workspace_members ?? []).length !== workspacePackages.length) {
    errors.push('workspace_members contains an ID absent from packages');
  }
  if (!sameSet(actualNames, MEMBER_SET)) {
    errors.push(`workspace member set mismatch: got ${[...actualNames].sort().join(', ')}`);
  }
  if (workspacePackages.length !== MEMBER_NAMES.length) {
    errors.push(`workspace must contain exactly ${MEMBER_NAMES.length} members, got ${workspacePackages.length}`);
  }
  if (workspaceVersion === undefined) {
    errors.push(`workspace packages must share one version, got ${[...workspaceVersions].sort().join(', ')}`);
  }
  if (!sameStringArrayAsSet(metadata.workspace_default_members, metadata.workspace_members ?? [])) {
    errors.push('workspace_default_members must exactly equal workspace_members');
  }

  const edges = new Map(MEMBER_NAMES.map((name) => [name, new Set()]));
  const binaries = [];
  for (const pkg of workspacePackages) {
    const spec = GRAPH_SPEC[pkg.name];
    if (!spec) continue;

    const expectedManifest = `${root}/${spec.dir}/Cargo.toml`;
    if (normalizedPath(pkg.manifest_path) !== expectedManifest) {
      errors.push(`${pkg.name} manifest path mismatch`);
    }
    if (pkg.edition !== '2024' || pkg.rust_version !== '1.97.0' || pkg.license !== 'ISC') {
      errors.push(`${pkg.name} workspace package metadata mismatch`);
    }
    if (pkg.source !== null || !Array.isArray(pkg.publish) || pkg.publish.length !== 0) {
      errors.push(`${pkg.name} must remain an unpublished path package`);
    }
    if (Object.keys(pkg.features ?? {}).length !== 0) {
      errors.push(`${pkg.name} must declare no Cargo features`);
    }

    const targets = pkg.targets ?? [];
    const primaryTargets = targets.filter((target) => (
      sameStringArrayAsSet(target.kind, [spec.kind])
      && sameStringArrayAsSet(target.crate_types, [spec.kind])
    ));
    if (primaryTargets.length !== 1) {
      errors.push(`${pkg.name} must expose exactly one primary ${spec.kind} target`);
    }
    const integrationTestTargets = targets.filter((target) => (
      sameStringArrayAsSet(target.kind, ['test'])
      && sameStringArrayAsSet(target.crate_types, ['bin'])
    ));
    const customBuildTargets = targets.filter((target) => (
      sameStringArrayAsSet(target.kind, ['custom-build'])
      && sameStringArrayAsSet(target.crate_types, ['bin'])
    ));
    const expectedCustomBuildCount = EMBEDDED_WEB_BUILD_PACKAGES.has(pkg.name) ? 1 : 0;
    if (customBuildTargets.length !== expectedCustomBuildCount) {
      errors.push(`${pkg.name} must expose exactly ${expectedCustomBuildCount} approved custom-build target(s)`);
    }
    if (targets.length !== primaryTargets.length + integrationTestTargets.length + customBuildTargets.length) {
      errors.push(`${pkg.name} may expose only its primary ${spec.kind} target, explicit integration-test targets, and its approved custom-build target`);
    }
    for (const target of targets) {
      if ((target.required_features ?? []).length !== 0) {
        errors.push(`${pkg.name}/${target.name} must declare no target required features`);
      }
      if (target.kind?.includes('bin')) binaries.push({ package: pkg.name, name: target.name });
    }
    for (const target of integrationTestTargets) {
      const testRoot = `${root}/${spec.dir}/tests/`;
      const source = normalizedPath(target.src_path);
      if (!source.startsWith(testRoot) || !source.endsWith('.rs')) {
        errors.push(`${pkg.name}/${target.name} integration-test source path mismatch`);
      }
      if (target.edition !== '2024') errors.push(`${pkg.name}/${target.name} integration-test edition mismatch`);
    }
    for (const target of customBuildTargets) {
      const expectedSource = `${root}/${spec.dir}/build.rs`;
      if (target.name !== 'build-script-build' || normalizedPath(target.src_path) !== expectedSource) {
        errors.push(`${pkg.name} custom-build target must be exactly ${expectedSource}`);
      }
      if (target.edition !== '2024') errors.push(`${pkg.name} custom-build target edition mismatch`);
    }
    const target = primaryTargets[0];
    if (target) {
      const expectedSource = `${root}/${spec.dir}/src/${spec.kind === 'bin' ? 'main.rs' : 'lib.rs'}`;
      if (normalizedPath(target.src_path) !== expectedSource) errors.push(`${pkg.name} target source path mismatch`);
      if (target.edition !== '2024') errors.push(`${pkg.name} target edition mismatch`);
      if (spec.kind === 'bin' && target.name !== pkg.name) errors.push(`${pkg.name} binary target name mismatch`);
      if (spec.kind === 'lib' && target.name !== pkg.name.replaceAll('-', '_')) {
        errors.push(`${pkg.name} library target name mismatch`);
      }
    }

    const actualInternal = [];
    const actualExternal = [];
    const expectedInternal = new Set(spec.internal);
    const expectedExternal = new Map(spec.external);
    for (const dependency of pkg.dependencies ?? []) {
      const kind = dependencyKind(dependency);
      if (kind === null) {
        errors.push(`${pkg.name} -> ${dependency.name} dependency kind must be null (normal) or "dev"; got ${describe(dependency.kind)}`);
      }
      if (MEMBER_SET.has(dependency.name)) {
        actualInternal.push(dependency.name);
        edges.get(pkg.name).add(dependency.name);
        if (expectedInternal.has(dependency.name) && workspaceVersion !== undefined) {
          validateInternalDependency(pkg.name, dependency, root, workspaceVersion, errors);
        }
      } else {
        actualExternal.push(pairKey(dependency.name, kind ?? `invalid(${dependency.kind})`));
        const expectedKind = expectedExternal.get(dependency.name);
        if (expectedKind) validateExternalDependency(pkg.name, dependency, expectedKind, errors);
      }
    }
    if (new Set(actualInternal).size !== actualInternal.length) errors.push(`${pkg.name} contains duplicate internal dependency entries`);
    if (new Set(actualExternal).size !== actualExternal.length) errors.push(`${pkg.name} contains duplicate external dependency entries`);
    if (!sameSet(new Set(actualInternal), expectedInternal)) {
      errors.push(`${pkg.name} internal edges mismatch: got ${[...new Set(actualInternal)].sort().join(', ')}`);
    }
    const expectedExternalKeys = new Set(spec.external.map(([name, kind]) => pairKey(name, kind)));
    if (!sameSet(new Set(actualExternal), expectedExternalKeys)) {
      errors.push(`${pkg.name} external dependency owners mismatch: got ${[...new Set(actualExternal)].sort().join(', ')}`);
    }
  }

  if (binaries.length !== 2) errors.push(`workspace must expose exactly 2 binary targets, got ${binaries.length}`);
  const binaryKeys = new Set(binaries.map(({ package: pkg, name }) => `${pkg}:${name}`));
  if (!sameSet(binaryKeys, new Set(['bcsp-local:bcsp-local', 'bcsp-server:bcsp-server']))) {
    errors.push(`binary target set mismatch: got ${[...binaryKeys].sort().join(', ')}`);
  }

  const cycle = findCycle(edges);
  if (cycle.length > 0) errors.push(`internal dependency graph contains a cycle: ${cycle.join(', ')}`);

  const localReachable = reachableFrom('bcsp-local', edges);
  const publicReachable = reachableFrom('bcsp-server', edges);
  for (const name of SHARED) {
    if (!localReachable.has(name)) errors.push(`local entry does not reach shared package ${name}`);
    if (!publicReachable.has(name)) errors.push(`public entry does not reach shared package ${name}`);
  }
  for (const name of PUBLIC_ONLY) {
    if (localReachable.has(name)) errors.push(`local entry reaches public-only package ${name}`);
  }
  for (const name of LOCAL_ONLY) {
    if (publicReachable.has(name)) errors.push(`public entry reaches local-only package ${name}`);
  }
  for (const name of SHARED) {
    const reachable = reachableFrom(name, edges);
    for (const variant of [...LOCAL_ONLY, ...PUBLIC_ONLY]) {
      if (reachable.has(variant)) errors.push(`shared package ${name} reaches variant package ${variant}`);
    }
  }

  const directOpenOwners = workspacePackages
    .filter((pkg) => (pkg.dependencies ?? []).some((dependency) => dependency.name === 'open'))
    .map((pkg) => pkg.name);
  if (!sameSet(new Set(directOpenOwners), new Set(['bcsp-local-runtime']))) {
    errors.push(`open dependency must be owned only by bcsp-local-runtime, got ${directOpenOwners.sort().join(', ')}`);
  }

  return [...new Set(errors)].sort();
}

export function validateRootCargoManifest(source) {
  if (typeof source !== 'string') return ['root Cargo.toml must be UTF-8 text'];
  const lines = source.replaceAll('\r\n', '\n').replaceAll('\r', '\n').split('\n');
  const firstSectionLine = lines.findIndex((line) => /^\s*\[[^\]]+\]\s*(?:#.*)?$/u.test(line));
  const preambleStatements = lines
    .slice(0, firstSectionLine < 0 ? lines.length : firstSectionLine)
    .filter((line) => line.trim() !== '' && !/^\s*#/u.test(line));
  if (preambleStatements.length > 0) {
    return ['root Cargo.toml must not contain top-level preamble assignments'];
  }
  const workspaceSections = [];
  const forbiddenOverrideSections = [];
  const sectionNames = [];
  const quotedSectionNames = [];
  for (let index = 0; index < lines.length; index += 1) {
    const match = lines[index].match(/^\s*\[([^\]]+)\]\s*(?:#.*)?$/u);
    const sectionName = match?.[1].trim();
    const normalizedSectionName = sectionName?.replace(/[\s"']/gu, '');
    if (normalizedSectionName) sectionNames.push(normalizedSectionName);
    if (/["']/u.test(sectionName ?? '')) quotedSectionNames.push(sectionName);
    if (normalizedSectionName === 'workspace') workspaceSections.push(index);
    if (/^(?:patch(?:\.|$)|replace$)/u.test(normalizedSectionName ?? '')) {
      forbiddenOverrideSections.push(sectionName);
    }
  }
  if (forbiddenOverrideSections.length > 0) {
    return [`root Cargo.toml must not contain source override sections: ${forbiddenOverrideSections.join(', ')}`];
  }
  if (quotedSectionNames.length > 0) {
    return [`root Cargo.toml must not contain quoted section keys: ${quotedSectionNames.join(', ')}`];
  }
  if (JSON.stringify(sectionNames) !== JSON.stringify(ROOT_MANIFEST_SECTION_NAMES)) {
    return [`root Cargo.toml section universe mismatch: ${sectionNames.join(', ')}`];
  }
  if (lines.some((line) => /^(?:patch|replace)(?:\.|=)/u.test(line.replace(/[\s"']/gu, '')))) {
    return ['root Cargo.toml must not contain source override assignments'];
  }
  if (workspaceSections.length !== 1) {
    return [`root Cargo.toml must contain exactly one [workspace] section, got ${workspaceSections.length}`];
  }
  const start = workspaceSections[0] + 1;
  let end = lines.length;
  for (let index = start; index < lines.length; index += 1) {
    if (/^\s*\[[^\]]+\]\s*(?:#.*)?$/u.test(lines[index])) {
      end = index;
      break;
    }
  }
  const assignments = lines.slice(start, end).filter((line) => /^\s*resolver\s*=/u.test(line));
  if (assignments.length !== 1) {
    return [`[workspace] must contain exactly one resolver assignment, got ${assignments.length}`];
  }
  if (!/^\s*resolver\s*=\s*"3"\s*(?:#.*)?$/u.test(assignments[0])) {
    return ['[workspace] resolver must be exactly "3"'];
  }
  return [];
}

function semanticSha256(value) {
  return createHash('sha256').update(JSON.stringify(value), 'utf8').digest('hex').toUpperCase();
}

function normalizeAuditToken(value) {
  return String(value).toLocaleLowerCase('en-US').replace(/[^a-z0-9]/gu, '');
}

export function validatePublicSourceDenyDocument(document) {
  const errors = [];
  if (!document || typeof document !== 'object' || Array.isArray(document)) {
    return ['public SOURCE deny document must be an object'];
  }
  const expectedTopLevelKeys = ['schemaVersion', 'markerSetVersion', 'kind', 'surface', 'isCapabilityManifest', 'snapshotMode', 'sourceReference', 'runtimeSourceRequired', 'capabilities'];
  if (JSON.stringify(Object.keys(document)) !== JSON.stringify(expectedTopLevelKeys)) {
    errors.push('public SOURCE deny document top-level keys or key order mismatch');
  }
  if (document.schemaVersion !== 2 || document.markerSetVersion !== 1) errors.push('public SOURCE deny document version mismatch');
  if (document.kind !== 'P4_PUBLIC_SOURCE_DENY_FROZEN_SNAPSHOT') errors.push('public SOURCE deny document kind mismatch');
  if (document.surface !== 'SOURCE') errors.push('public SOURCE deny document surface mismatch');
  if (document.isCapabilityManifest !== false) errors.push('public SOURCE deny document must not be a capability manifest');
  if (document.snapshotMode !== 'FROZEN_P7_1_003_NO_LIVE_SOURCE_DEPENDENCY') errors.push('public SOURCE deny snapshot mode mismatch');
  if (document.sourceReference !== 'project-governance/current/p4/05-public-capability-deny-audit.tsv') errors.push('public SOURCE deny provenance reference mismatch');
  if (document.runtimeSourceRequired !== false) errors.push('public SOURCE deny snapshot must not require its provenance source at runtime');
  if (!Array.isArray(document.capabilities)) {
    errors.push('public SOURCE deny capabilities must be an array');
    return errors;
  }
  if (document.capabilities.length !== EXPECTED_PUBLIC_SOURCE_DENIES.length) {
    errors.push(`public SOURCE deny document must contain exactly ${EXPECTED_PUBLIC_SOURCE_DENIES.length} rows`);
  }
  let markerCount = 0;
  const normalizedMarkers = [];
  EXPECTED_PUBLIC_SOURCE_DENIES.forEach(([denyId, capability], index) => {
    const row = document.capabilities[index];
    if (!row || typeof row !== 'object' || Array.isArray(row)) {
      errors.push(`public SOURCE deny row ${index + 1} must be an object`);
      return;
    }
    if (JSON.stringify(Object.keys(row)) !== JSON.stringify(['denyId', 'capability', 'validationId', 'markers'])) {
      errors.push(`public SOURCE deny row ${index + 1} keys or key order mismatch`);
    }
    if (row.denyId !== denyId || row.capability !== capability) {
      errors.push(`public SOURCE deny row ${index + 1} must be ${denyId}/${capability}`);
    }
    const validationId = denyId.replace(/^P4-D-/u, 'P4-Z-');
    if (row.validationId !== validationId) {
      errors.push(`public SOURCE deny row ${index + 1} validation ID must be ${validationId}`);
    }
    if (!Array.isArray(row.markers) || row.markers.length === 0 || row.markers.some((marker) => typeof marker !== 'string' || marker.length === 0)) {
      errors.push(`${denyId}: markers must be a non-empty string array`);
      return;
    }
    markerCount += row.markers.length;
    normalizedMarkers.push(...row.markers.map(normalizeAuditToken));
    const sorted = [...row.markers].sort((left, right) => left.localeCompare(right, 'en-US'));
    if (JSON.stringify(row.markers) !== JSON.stringify(sorted)) errors.push(`${denyId}: markers must be in ordinal order`);
  });
  if (markerCount !== PUBLIC_SOURCE_MARKER_COUNT) errors.push(`public SOURCE marker count must be ${PUBLIC_SOURCE_MARKER_COUNT}, got ${markerCount}`);
  if (new Set(normalizedMarkers).size !== normalizedMarkers.length) errors.push('public SOURCE normalized markers must be globally unique');
  if (semanticSha256(document) !== PUBLIC_SOURCE_DENY_SEMANTIC_SHA256) errors.push('public SOURCE deny marker contract digest mismatch');
  return errors;
}

function inspectTargetSource(source, repoRoot) {
  let current = source;
  let linkedComponent = false;
  while (true) {
    const stat = lstatSync(current);
    linkedComponent ||= stat.isSymbolicLink();
    if (normalizedPath(current) === normalizedPath(repoRoot)) break;
    const parent = path.dirname(current);
    if (parent === current) break;
    current = parent;
  }
  const sourceStat = lstatSync(source);
  return {
    isFile: sourceStat.isFile(),
    linkedComponent,
    realPath: realpathSync.native(source),
  };
}

export function auditWorkspaceTargetSources(metadata, { repoRoot, inspectSource = inspectTargetSource } = {}) {
  const errors = [];
  const root = path.resolve(repoRoot ?? metadata.workspace_root ?? process.cwd());
  const packagesById = new Map((metadata.packages ?? []).map((pkg) => [pkg.id, pkg]));
  const workspacePackages = (metadata.workspace_members ?? [])
    .map((id) => packagesById.get(id))
    .filter(Boolean);

  for (const pkg of workspacePackages) {
    for (const target of pkg.targets ?? []) {
      const label = `${pkg.name}/${target.name}`;
      if (typeof target.src_path !== 'string' || target.src_path.length === 0) {
        errors.push(`${label} target source path is missing`);
        continue;
      }
      const declared = path.resolve(target.src_path);
      const relative = path.relative(root, declared);
      if (relative === '' || relative === '..' || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
        errors.push(`${label} target source escapes the workspace root`);
        continue;
      }
      try {
        const inspected = inspectSource(declared, root);
        if (!inspected || inspected.linkedComponent) {
          errors.push(`${label} target source path contains a symlink or junction`);
          continue;
        }
        if (!inspected.isFile) {
          errors.push(`${label} target source is not a regular file`);
          continue;
        }
        if (normalizedPath(path.resolve(inspected.realPath)) !== normalizedPath(declared)) {
          errors.push(`${label} target real path differs from its declared path`);
        }
      } catch {
        errors.push(`${label} target source is not readable`);
      }
    }
  }
  return errors;
}

function rawStringStart(source, index) {
  let cursor = index;
  if (source[cursor] === 'b' && source[cursor + 1] === 'r') cursor += 1;
  if (source[cursor] !== 'r') return null;
  cursor += 1;
  let hashes = 0;
  while (source[cursor] === '#') {
    hashes += 1;
    cursor += 1;
  }
  if (source[cursor] !== '"') return null;
  return { end: cursor + 1, hashes };
}

function charLiteralEnd(source, index) {
  if (source[index] !== "'") return -1;
  let cursor = index + 1;
  if (source[cursor] === '\\') cursor += 2;
  else cursor += 1;
  return source[cursor] === "'" ? cursor + 1 : -1;
}

function maskRust(source, maskLiterals) {
  let output = '';
  let index = 0;
  let blockDepth = 0;
  let state = 'code';
  let rawHashes = 0;
  const appendMasked = (character) => {
    output += character === '\n' || character === '\r' ? character : ' ';
  };
  while (index < source.length) {
    const character = source[index];
    const next = source[index + 1];
    if (state === 'line-comment') {
      appendMasked(character);
      index += 1;
      if (character === '\n') state = 'code';
      continue;
    }
    if (state === 'block-comment') {
      if (character === '/' && next === '*') {
        appendMasked(character);
        appendMasked(next);
        blockDepth += 1;
        index += 2;
      } else if (character === '*' && next === '/') {
        appendMasked(character);
        appendMasked(next);
        blockDepth -= 1;
        index += 2;
        if (blockDepth === 0) state = 'code';
      } else {
        appendMasked(character);
        index += 1;
      }
      continue;
    }
    if (state === 'string') {
      if (maskLiterals) appendMasked(character);
      else output += character;
      index += 1;
      if (character === '\\' && index < source.length) {
        if (maskLiterals) appendMasked(source[index]);
        else output += source[index];
        index += 1;
      } else if (character === '"') state = 'code';
      continue;
    }
    if (state === 'raw-string') {
      const closing = `"${'#'.repeat(rawHashes)}`;
      if (source.startsWith(closing, index)) {
        for (const closingCharacter of closing) {
          if (maskLiterals) appendMasked(closingCharacter);
          else output += closingCharacter;
        }
        index += closing.length;
        state = 'code';
      } else {
        if (maskLiterals) appendMasked(character);
        else output += character;
        index += 1;
      }
      continue;
    }
    if (character === '/' && next === '/') {
      appendMasked(character);
      appendMasked(next);
      index += 2;
      state = 'line-comment';
      continue;
    }
    if (character === '/' && next === '*') {
      appendMasked(character);
      appendMasked(next);
      index += 2;
      blockDepth = 1;
      state = 'block-comment';
      continue;
    }
    const rawStart = rawStringStart(source, index);
    if (rawStart) {
      const prefix = source.slice(index, rawStart.end);
      for (const prefixCharacter of prefix) {
        if (maskLiterals) appendMasked(prefixCharacter);
        else output += prefixCharacter;
      }
      index = rawStart.end;
      rawHashes = rawStart.hashes;
      state = 'raw-string';
      continue;
    }
    if (character === '"') {
      if (maskLiterals) appendMasked(character);
      else output += character;
      index += 1;
      state = 'string';
      continue;
    }
    const characterEnd = charLiteralEnd(source, index);
    if (characterEnd > index) {
      const literal = source.slice(index, characterEnd);
      for (const literalCharacter of literal) {
        if (maskLiterals) appendMasked(literalCharacter);
        else output += literalCharacter;
      }
      index = characterEnd;
      continue;
    }
    output += character;
    index += 1;
  }
  return output;
}

export function rustAuditTokens(source) {
  const withoutComments = maskRust(source, false);
  const tokens = new Set();
  const addTokens = (value, includeComposite = false) => {
    for (const match of value.matchAll(/[A-Za-z][A-Za-z0-9_]*/gu)) {
      const normalized = normalizeAuditToken(match[0]);
      if (normalized) tokens.add(normalized);
    }
    if (includeComposite) {
      const composite = normalizeAuditToken(value);
      if (composite) tokens.add(composite);
    }
  };
  addTokens(withoutComments);
  for (const match of withoutComments.matchAll(/b?"((?:\\[\s\S]|[^"\\])*)"/gu)) {
    const quoteIndex = match.index + (match[0].startsWith('b"') ? 1 : 0);
    if (['r', '#'].includes(withoutComments[quoteIndex - 1])) continue;
    let valid = true;
    let decoded = match[1].replace(/\\\r?\n[\t ]*/gu, '');
    decoded = decoded.replace(/\\u\{([0-9a-fA-F_]{1,8})\}/gu, (_whole, rawHex) => {
      const hex = rawHex.replaceAll('_', '');
      const codePoint = Number.parseInt(hex, 16);
      if (!Number.isFinite(codePoint) || codePoint > 0x10ffff) {
        valid = false;
        return '';
      }
      return String.fromCodePoint(codePoint);
    });
    decoded = decoded.replace(/\\x([0-9a-fA-F]{2})/gu, (_whole, hex) =>
      String.fromCodePoint(Number.parseInt(hex, 16)));
    const simpleEscapes = new Map([
      ['0', '\0'],
      ['n', '\n'],
      ['r', '\r'],
      ['t', '\t'],
      ['"', '"'],
      ["'", "'"],
      ['\\', '\u0000'],
    ]);
    decoded = decoded.replace(/\\([0nrt"'\\])/gu, (_whole, escape) => simpleEscapes.get(escape));
    if (decoded.includes('\\')) valid = false;
    if (valid) addTokens(decoded.replaceAll('\u0000', '\\'), true);
  }
  for (const match of withoutComments.matchAll(/b?r(#{0,255})"([\s\S]*?)"\1/gu)) {
    addTokens(match[2], true);
  }
  return tokens;
}

export function auditRustSourceFiles(files, denyDocument) {
  const inputErrors = validatePublicSourceDenyDocument(denyDocument);
  const errors = [...inputErrors];
  const violations = [];
  const sourceEscapes = [];
  if (errors.length === 0) {
    for (const [file, source] of [...files.entries()].sort(([left], [right]) => left.localeCompare(right, 'en-US'))) {
      const codeOnly = maskRust(source, true);
      for (const macroName of ['include', 'include_str', 'include_bytes', 'include_dir', 'concat', 'concat_bytes', 'concat_idents']) {
        if (macroName === 'include_dir' && approvedPublicWebAssetEmbed(file, source, codeOnly)) continue;
        const pattern = new RegExp(`(^|[^A-Za-z0-9_])${macroName}\\s*!`, 'u');
        const referencePattern = new RegExp(`\\b${macroName}\\b`, 'u');
        const referenceSurface = macroName === 'include_dir'
          ? codeOnly.replace(/\buse\s+include_dir\s+as\s+_\s*;/gu, '')
          : codeOnly;
        if (pattern.test(codeOnly)) {
          sourceEscapes.push({ file, mechanism: `${macroName}!` });
          errors.push(`${file}: ${macroName}! is forbidden in the public Rust source closure`);
        } else if (referencePattern.test(referenceSurface)) {
          sourceEscapes.push({ file, mechanism: `${macroName} reference` });
          errors.push(`${file}: ${macroName} alias/reference is forbidden in the public Rust source closure`);
        }
      }
      if (/#\s*!?\s*\[[^\]]*\bpath\s*=/u.test(codeOnly)) {
        sourceEscapes.push({ file, mechanism: '#[path]' });
        errors.push(`${file}: path-bearing attribute source redirection is forbidden in the public Rust source closure`);
      }
      const tokens = rustAuditTokens(source);
      for (const row of denyDocument.capabilities) {
        for (const marker of row.markers) {
          const normalizedMarker = normalizeAuditToken(marker);
          const token = [...tokens].find((candidate) => candidate.includes(normalizedMarker));
          if (token) {
            violations.push({ capability: row.capability, denyId: row.denyId, file, marker, token });
          }
        }
      }
    }
  }
  for (const violation of violations) {
    errors.push(`${violation.file}: public SOURCE deny ${violation.denyId} matched ${violation.marker}`);
  }
  return {
    checkedCount: inputErrors.length === 0 ? EXPECTED_PUBLIC_SOURCE_DENIES.length : 0,
    errors,
    expectedCount: EXPECTED_PUBLIC_SOURCE_DENIES.length,
    markerCount: PUBLIC_SOURCE_MARKER_COUNT,
    sourceEscapeCount: sourceEscapes.length,
    sourceEscapes,
    violationCount: violations.length,
    violations,
  };
}

function approvedPublicWebAssetEmbed(file, source, codeOnly) {
  if (file !== 'crates/bcsp-public-runtime/src/host.rs') return false;
  if ([...codeOnly.matchAll(/include_dir\s*!\s*\(/gu)].length !== 1) return false;
  const approvedArguments = [...source.matchAll(
    /include_dir\s*!\s*\(\s*"\$OUT_DIR\/web-assets"\s*\)/gu,
  )];
  if (approvedArguments.length !== 1) return false;
  const withoutMacro = codeOnly.replace(
    /include_dir\s*!\s*\([^)]*\)/gu,
    '',
  );
  if (withoutMacro === codeOnly) return false;
  const withoutImport = withoutMacro.replace(
    /use\s+include_dir\s*::\s*\{\s*Dir\s*,\s*include_dir\s*\}\s*;/gu,
    '',
  );
  return !/\binclude_dir\b/u.test(withoutImport);
}

function enumerateRustSources(directory, repoRoot, files, errors) {
  let directoryStat;
  try {
    directoryStat = lstatSync(directory);
  } catch (error) {
    errors.push(`${relativePath(repoRoot, directory)}: source directory is not readable: ${error instanceof Error ? error.message : String(error)}`);
    return;
  }
  if (directoryStat.isSymbolicLink()) {
    errors.push(`${relativePath(repoRoot, directory)}: symlink is forbidden in the public Rust source closure`);
    return;
  }
  if (!directoryStat.isDirectory()) {
    errors.push(`${relativePath(repoRoot, directory)}: expected a source directory`);
    return;
  }
  let entries;
  try {
    entries = readdirSync(directory, { withFileTypes: true }).sort((left, right) => left.name.localeCompare(right.name, 'en-US'));
  } catch (error) {
    errors.push(`${relativePath(repoRoot, directory)}: source directory enumeration failed: ${error instanceof Error ? error.message : String(error)}`);
    return;
  }
  for (const entry of entries) {
    const absolute = path.join(directory, entry.name);
    let stat;
    try {
      stat = lstatSync(absolute);
    } catch (error) {
      errors.push(`${relativePath(repoRoot, absolute)}: source entry is not readable: ${error instanceof Error ? error.message : String(error)}`);
      continue;
    }
    if (stat.isSymbolicLink()) {
      errors.push(`${relativePath(repoRoot, absolute)}: symlink is forbidden in the public Rust source closure`);
      continue;
    }
    if (stat.isDirectory()) {
      enumerateRustSources(absolute, repoRoot, files, errors);
      continue;
    }
    if (!stat.isFile() || path.extname(entry.name) !== '.rs') continue;
    const relative = relativePath(repoRoot, absolute);
    try {
      const real = realpathSync.native(absolute);
      if (normalizedPath(real) !== normalizedPath(absolute)) {
        errors.push(`${relative}: real path differs from the declared public source path`);
        continue;
      }
      files.set(relative, readFileSync(absolute, 'utf8'));
    } catch (error) {
      errors.push(`${relative}: Rust source read failed: ${error instanceof Error ? error.message : String(error)}`);
    }
  }
}

export function scanPublicRustSources(metadata, { denyDocument, denyInputPath = PUBLIC_SOURCE_DENY_PATH, repoRoot } = {}) {
  const root = path.resolve(repoRoot ?? metadata.workspace_root ?? process.cwd());
  const closure = publicClosurePackages(metadata);
  const files = new Map();
  const enumerationErrors = [];
  for (const packageName of closure) {
    const spec = GRAPH_SPEC[packageName];
    if (!spec) {
      enumerationErrors.push(`public closure contains unknown package ${packageName}`);
      continue;
    }
    const before = files.size;
    enumerateRustSources(path.join(root, spec.dir, 'src'), root, files, enumerationErrors);
    if (files.size === before) enumerationErrors.push(`${spec.dir}/src: public closure package contains no Rust sources`);
  }
  const audit = auditRustSourceFiles(files, denyDocument);
  return {
    checkedCount: audit.checkedCount,
    denyInput: denyInputPath,
    errors: [...enumerationErrors, ...audit.errors],
    expectedCount: audit.expectedCount,
    markerCount: audit.markerCount,
    publicClosurePackages: closure,
    scannedFileCount: files.size,
    scannedFiles: [...files.keys()].sort((left, right) => left.localeCompare(right, 'en-US')),
    sourceEscapeCount: audit.sourceEscapeCount,
    sourceEscapes: audit.sourceEscapes,
    violationCount: audit.violationCount,
    violations: audit.violations,
  };
}

function emptyPublicSourceDeny() {
  return {
    checkedCount: 0,
    denyInput: PUBLIC_SOURCE_DENY_PATH,
    errors: [],
    expectedCount: EXPECTED_PUBLIC_SOURCE_DENIES.length,
    markerCount: PUBLIC_SOURCE_MARKER_COUNT,
    publicClosurePackages: [],
    scannedFileCount: 0,
    scannedFiles: [],
    sourceEscapeCount: 0,
    sourceEscapes: [],
    violationCount: 0,
    violations: [],
  };
}

export function createReport(errors, publicSourceDeny = emptyPublicSourceDeny()) {
  return {
    schemaVersion: 2,
    check: 'P7.1-003_RUST_GRAPH',
    ok: errors.length === 0,
    cargoResolver: '3',
    rootManifestSections: ROOT_MANIFEST_SECTION_NAMES,
    workspaceMembers: MEMBER_NAMES.length,
    workspaceDefaultMembers: MEMBER_NAMES.length,
    binaryTargets: ['bcsp-local', 'bcsp-server'],
    publicSourceDeny,
    errors,
  };
}

export function formatReport(report, jsonMode = false) {
  if (jsonMode) return `${JSON.stringify(report)}\n`;
  if (report.ok) {
    return `PASS ${report.check}: ${report.workspaceMembers} members, bins=${report.binaryTargets.join(',')}, public SOURCE denies=${report.publicSourceDeny.checkedCount}/${report.publicSourceDeny.expectedCount}\n`;
  }
  return `FAIL ${report.check}\n${report.errors.map((error) => `- ${error}`).join('\n')}\n`;
}

async function main() {
  const args = process.argv.slice(2);
  const jsonMode = args.includes('--json');
  const unknown = args.filter((arg) => arg !== '--json');
  let errors = [];
  let publicSourceDeny = emptyPublicSourceDeny();

  if (unknown.length > 0) {
    errors = [`unknown argument(s): ${unknown.join(', ')}`];
  } else {
    const scriptDir = path.dirname(fileURLToPath(import.meta.url));
    const repoRoot = path.resolve(scriptDir, '../..');
    const cargo = process.env.CARGO || 'cargo';
    try {
      const rootManifest = readFileSync(path.join(repoRoot, 'Cargo.toml'), 'utf8');
      errors.push(...validateRootCargoManifest(rootManifest));
    } catch (error) {
      errors.push(`Cargo.toml: read failed: ${error instanceof Error ? error.message : String(error)}`);
    }
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
        const metadata = JSON.parse(result.stdout);
        errors = verifyGraph(metadata, { repoRoot });
        errors.push(...auditWorkspaceTargetSources(metadata, { repoRoot }));
        let denyDocument;
        try {
          denyDocument = JSON.parse(readFileSync(path.join(repoRoot, PUBLIC_SOURCE_DENY_PATH), 'utf8'));
        } catch (error) {
          denyDocument = null;
          errors.push(`${PUBLIC_SOURCE_DENY_PATH}: read or JSON parse failed: ${error instanceof Error ? error.message : String(error)}`);
        }
        publicSourceDeny = scanPublicRustSources(metadata, { denyDocument, repoRoot });
        errors.push(...publicSourceDeny.errors);
      } catch (error) {
        errors.push(`cargo metadata JSON parse failed: ${error instanceof Error ? error.message : String(error)}`);
      }
    }
  }

  const report = createReport([...new Set(errors)].sort(), publicSourceDeny);
  process.stdout.write(formatReport(report, jsonMode));
  process.exitCode = report.ok ? 0 : 1;
}

const invokedPath = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : '';
if (import.meta.url === invokedPath) {
  await main();
}
