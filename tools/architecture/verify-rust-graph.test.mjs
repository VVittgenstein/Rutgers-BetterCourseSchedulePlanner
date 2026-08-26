#!/usr/bin/env node

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import {
  EXTERNAL_DEPENDENCY_SPEC,
  GRAPH_SPEC,
  SUPPLEMENTAL_RUST_MARKERS,
  auditWorkspaceTargetSources,
  auditRustSourceFiles,
  createReport,
  formatReport,
  publicClosurePackages,
  validatePublicSourceDenyDocument,
  validateRootCargoManifest,
  verifyGraph,
} from './verify-rust-graph.mjs';

const REPO_ROOT = '/repo';
const DENY_DOCUMENT = JSON.parse(readFileSync(new URL('./public-source-deny.json', import.meta.url), 'utf8'));

function internalDependency(name, kind = 'normal') {
  return {
    name,
    source: null,
    req: '=0.1.0',
    kind: kind === 'dev' ? 'dev' : null,
    rename: null,
    optional: false,
    uses_default_features: true,
    features: [],
    target: null,
    registry: null,
    path: `${REPO_ROOT}/${GRAPH_SPEC[name].dir}`,
  };
}

function externalDependency(name, kind = 'normal', features) {
  const spec = EXTERNAL_DEPENDENCY_SPEC[name];
  return {
    name,
    source: spec.source,
    req: spec.req,
    kind: kind === 'dev' ? 'dev' : null,
    rename: null,
    optional: false,
    uses_default_features: spec.usesDefaultFeatures,
    features: [...(features ?? spec.features)],
    target: null,
    registry: null,
  };
}

function fixture() {
  const packages = Object.entries(GRAPH_SPEC).map(([name, spec]) => {
    const id = `${name} 0.1.0 (path+file:///repo/${spec.dir})`;
    return {
      id,
      name,
      version: '0.1.0',
      edition: '2024',
      rust_version: '1.97.0',
      license: 'ISC',
      source: null,
      publish: [],
      manifest_path: `${REPO_ROOT}/${spec.dir}/Cargo.toml`,
      features: {},
      dependencies: [
        ...spec.internal.map(internalDependency),
        ...(spec.internalDev ?? []).map((name) => internalDependency(name, 'dev')),
        ...spec.external.map(([dependencyName, kind, features]) => externalDependency(dependencyName, kind, features)),
      ],
      targets: [{
        name: spec.kind === 'bin' ? name : name.replaceAll('-', '_'),
        kind: [spec.kind],
        crate_types: [spec.kind],
        required_features: [],
        src_path: `${REPO_ROOT}/${spec.dir}/src/${spec.kind === 'bin' ? 'main.rs' : 'lib.rs'}`,
        edition: '2024',
      }, ...(['bcsp-local-runtime', 'bcsp-public-runtime'].includes(name) ? [{
        name: 'build-script-build',
        kind: ['custom-build'],
        crate_types: ['bin'],
        required_features: [],
        src_path: `${REPO_ROOT}/${spec.dir}/build.rs`,
        edition: '2024',
      }] : [])],
    };
  });
  return {
    packages,
    workspace_members: packages.map((pkg) => pkg.id),
    workspace_default_members: packages.map((pkg) => pkg.id),
    workspace_root: REPO_ROOT,
  };
}

function packageByName(metadata, name) {
  return metadata.packages.find((pkg) => pkg.name === name);
}

function integrationTarget(name = 'wire_golden') {
  return {
    name,
    kind: ['test'],
    crate_types: ['bin'],
    required_features: [],
    src_path: `${REPO_ROOT}/crates/bcsp-contracts/tests/${name}.rs`,
    edition: '2024',
  };
}

function cleanSourceInspector(source) {
  return { isFile: true, linkedComponent: false, realPath: source };
}

function dependencyByName(metadata, packageName, dependencyName) {
  return packageByName(metadata, packageName).dependencies.find((dependency) => dependency.name === dependencyName);
}

function expectFailure(mutator, pattern) {
  const metadata = fixture();
  mutator(metadata);
  const errors = verifyGraph(metadata, { repoRoot: REPO_ROOT });
  assert.ok(errors.some((error) => pattern.test(error)), `expected ${pattern}, got:\n${errors.join('\n')}`);
}

assert.deepEqual(verifyGraph(fixture(), { repoRoot: REPO_ROOT }), []);
assert.deepEqual(GRAPH_SPEC['bcsp-catalog'].external, [
  ['serde', 'normal'],
  ['serde_json', 'normal'],
  ['tempfile', 'dev'],
  ['thiserror', 'normal'],
  ['time', 'normal'],
  ['tokio', 'normal'],
  ['tracing', 'normal'],
]);
assert.deepEqual(GRAPH_SPEC['bcsp-operational-storage'].external, [
  ['include_dir', 'normal'],
  ['rusqlite', 'normal', ['bundled', 'cache']],
  ['rusqlite', 'dev', ['bundled', 'hooks']],
  ['serde', 'normal'],
  ['serde_json', 'normal'],
  ['sha2', 'normal'],
  ['tempfile', 'dev'],
  ['thiserror', 'normal'],
  ['time', 'normal'],
  ['tracing', 'normal'],
]);
assert.deepEqual(GRAPH_SPEC['bcsp-query'].external, [
  ['proptest', 'dev'],
  ['serde', 'normal'],
  ['thiserror', 'normal'],
  ['time', 'normal'],
  ['tracing', 'normal'],
]);
assert.deepEqual(GRAPH_SPEC['bcsp-application'].external, [
  ['axum', 'normal'],
  ['rusqlite', 'normal'],
  ['serde', 'normal'],
  ['serde_json', 'normal'],
  ['sha2', 'dev'],
  ['tempfile', 'dev'],
  ['thiserror', 'normal'],
  ['time', 'normal'],
  ['tokio', 'normal'],
  ['tower', 'normal'],
  ['tower-http', 'normal'],
  ['tracing', 'normal'],
  ['tracing-subscriber', 'dev'],
]);
assert.deepEqual(GRAPH_SPEC['bcsp-local-runtime'].internalDev, [
  'bcsp-catalog',
  'bcsp-rutgers-client',
]);
assert.deepEqual(publicClosurePackages(fixture()), [
  'bcsp-contracts',
  'bcsp-domain',
  'bcsp-query',
  'bcsp-rutgers-client',
  'bcsp-operational-storage',
  'bcsp-catalog',
  'bcsp-open',
  'bcsp-watch',
  'bcsp-application',
  'bcsp-public-operations',
  'bcsp-public-runtime',
  'bcsp-server',
]);

expectFailure((metadata) => {
  metadata.workspace_members.pop();
}, /workspace member set mismatch/);

expectFailure((metadata) => {
  metadata.workspace_default_members.pop();
}, /workspace_default_members must exactly equal workspace_members/);

const rootManifest = (workspaceBody) => `[workspace]\n${workspaceBody}\n[workspace.package]\n\n[workspace.dependencies]\n\n[workspace.metadata.release.tooling]\n`;
assert.deepEqual(validateRootCargoManifest(rootManifest('members = []\nresolver = "3"')), []);
assert.ok(validateRootCargoManifest(rootManifest('resolver = "2"')).some((error) => /resolver must be exactly "3"/.test(error)));
assert.ok(validateRootCargoManifest(rootManifest('members = []')).some((error) => /exactly one resolver assignment/.test(error)));
assert.ok(validateRootCargoManifest(rootManifest('resolver = "3"\nresolver = "3"')).some((error) => /exactly one resolver assignment/.test(error)));
assert.ok(validateRootCargoManifest('[workspace]\nresolver = "3"\n[patch.crates-io]\nserde = { path = "vendor/serde" }\n').some((error) => /source override sections/.test(error)));
assert.ok(validateRootCargoManifest('[workspace]\nresolver = "3"\n[replace]\n"serde:1.0.0" = { path = "vendor/serde" }\n').some((error) => /source override sections/.test(error)));
assert.ok(validateRootCargoManifest('[workspace]\nresolver = "3"\n["patch".crates-io]\nserde = { path = "vendor/serde" }\n').some((error) => /source override sections/.test(error)));
assert.ok(validateRootCargoManifest('[workspace]\nresolver = "3"\n[patch . crates-io]\nserde = { path = "vendor/serde" }\n').some((error) => /source override sections/.test(error)));
assert.ok(validateRootCargoManifest('[workspace]\nresolver = "3"\n[ "replace" ]\n"serde:1.0.0" = { path = "vendor/serde" }\n').some((error) => /source override sections/.test(error)));
assert.ok(validateRootCargoManifest('patch = { crates-io = { serde = { path = "vendor/serde" } } }\n[workspace]\nresolver = "3"\n').some((error) => /preamble|section universe|source override assignments/.test(error)));
assert.ok(validateRootCargoManifest('"patch" = { crates-io = { serde = { path = "vendor/serde" } } }\n[workspace]\nresolver = "3"\n').some((error) => /preamble|section universe|source override assignments/.test(error)));
assert.ok(validateRootCargoManifest('replace = { "serde:1.0.228" = { path = "vendor/serde" } }\n[workspace]\nresolver = "3"\n').some((error) => /preamble|section universe|source override assignments/.test(error)));
assert.ok(validateRootCargoManifest('"p\\u0061tch" = { crates-io = { serde = { path = "vendor/serde" } } }\n[workspace]\nresolver = "3"\n').some((error) => /preamble/.test(error)));
assert.ok(validateRootCargoManifest('"repl\\u0061ce" = { "serde:1.0.228" = { path = "vendor/serde" } }\n[workspace]\nresolver = "3"\n').some((error) => /preamble/.test(error)));

expectFailure((metadata) => {
  packageByName(metadata, 'bcsp-public-runtime').dependencies.push(internalDependency('bcsp-local-user-state'));
}, /public entry reaches local-only package/);

expectFailure((metadata) => {
  packageByName(metadata, 'bcsp-application').features['public-mode'] = [];
}, /must declare no Cargo features/);

expectFailure((metadata) => {
  packageByName(metadata, 'bcsp-server').targets.push({
    name: 'third-binary',
    kind: ['bin'],
    crate_types: ['bin'],
    required_features: [],
    src_path: `${REPO_ROOT}/apps/bcsp-server/src/third.rs`,
    edition: '2024',
  });
}, /exactly 2 binary targets/);

expectFailure((metadata) => {
  packageByName(metadata, 'bcsp-contracts').dependencies.push(internalDependency('bcsp-application'));
}, /contains a cycle/);

expectFailure((metadata) => {
  packageByName(metadata, 'bcsp-local').targets[0].required_features.push('local-mode');
}, /must declare no target required features/);

// Dependency metadata is the feature-unification contract, not merely an owner list.
expectFailure((metadata) => {
  dependencyByName(metadata, 'bcsp-contracts', 'serde').kind = 'build';
}, /kind must be null \(normal\) or "dev"/);

expectFailure((metadata) => {
  dependencyByName(metadata, 'bcsp-contracts', 'serde').source = 'registry+https:\/\/example.invalid\/index';
}, /registry source mismatch/);

expectFailure((metadata) => {
  dependencyByName(metadata, 'bcsp-domain', 'bcsp-contracts').path = `${REPO_ROOT}/crates/bcsp-query`;
}, /internal path mismatch/);

expectFailure((metadata) => {
  dependencyByName(metadata, 'bcsp-domain', 'bcsp-contracts').source = EXTERNAL_DEPENDENCY_SPEC.serde.source;
}, /internal source must be null/);

expectFailure((metadata) => {
  dependencyByName(metadata, 'bcsp-domain', 'bcsp-contracts').req = '*';
}, /internal version requirement must be =0\.1\.0/);

expectFailure((metadata) => {
  dependencyByName(metadata, 'bcsp-local-runtime', 'bcsp-catalog').kind = null;
}, /bcsp-local-runtime -> bcsp-catalog dependency kind mismatch/);

expectFailure((metadata) => {
  packageByName(metadata, 'bcsp-domain').version = '0.2.0';
}, /workspace packages must share one version/);

expectFailure((metadata) => {
  dependencyByName(metadata, 'bcsp-contracts', 'serde').req = '*';
}, /version requirement mismatch/);

expectFailure((metadata) => {
  dependencyByName(metadata, 'bcsp-contracts', 'serde').features.push('rc');
}, /dependency features mismatch/);

expectFailure((metadata) => {
  dependencyByName(metadata, 'bcsp-contracts', 'time').uses_default_features = true;
}, /default-features mismatch/);

expectFailure((metadata) => {
  dependencyByName(metadata, 'bcsp-catalog', 'tempfile').kind = null;
}, /dependency kind mismatch|external dependency owners mismatch/);

expectFailure((metadata) => {
  dependencyByName(metadata, 'bcsp-operational-storage', 'sha2').kind = 'dev';
}, /dependency kind mismatch|external dependency owners mismatch/);

expectFailure((metadata) => {
  dependencyByName(metadata, 'bcsp-operational-storage', 'rusqlite').features = ['bundled'];
}, /dependency features mismatch/);

expectFailure((metadata) => {
  packageByName(metadata, 'bcsp-operational-storage').dependencies
    .find((dependency) => dependency.name === 'rusqlite' && dependency.kind === 'dev')
    .features = ['bundled'];
}, /dependency features mismatch/);

expectFailure((metadata) => {
  packageByName(metadata, 'bcsp-catalog').dependencies.push(externalDependency('sha2'));
}, /external dependency owners mismatch/);

expectFailure((metadata) => {
  packageByName(metadata, 'bcsp-operational-storage').dependencies.push(externalDependency('open'));
}, /external dependency owners mismatch/);

expectFailure((metadata) => {
  dependencyByName(metadata, 'bcsp-contracts', 'serde').target = 'cfg(windows)';
}, /must not be target-qualified/);

expectFailure((metadata) => {
  dependencyByName(metadata, 'bcsp-domain', 'bcsp-contracts').path = '/REPO/crates/bcsp-contracts';
}, /internal path mismatch/);

expectFailure((metadata) => {
  packageByName(metadata, 'bcsp-contracts').manifest_path = '/REPO/crates/bcsp-contracts/Cargo.toml';
}, /manifest path mismatch/);

expectFailure((metadata) => {
  packageByName(metadata, 'bcsp-contracts').targets[0].name = 'renamed_contracts';
}, /library target name mismatch/);

{
  const metadata = fixture();
  packageByName(metadata, 'bcsp-contracts').targets.push(integrationTarget());
  assert.deepEqual(verifyGraph(metadata, { repoRoot: REPO_ROOT }), []);
  assert.deepEqual(
    auditWorkspaceTargetSources(metadata, {
      repoRoot: REPO_ROOT,
      inspectSource: cleanSourceInspector,
    }),
    [],
  );
}

expectFailure((metadata) => {
  packageByName(metadata, 'bcsp-contracts').targets.push({
    name: 'second_contracts',
    kind: ['lib'],
    crate_types: ['lib'],
    required_features: [],
    src_path: `${REPO_ROOT}/crates/bcsp-contracts/src/second.rs`,
    edition: '2024',
  });
}, /exactly one primary lib target/);

expectFailure((metadata) => {
  packageByName(metadata, 'bcsp-contracts').targets.push({
    name: 'escaped_test',
    kind: ['test'],
    crate_types: ['bin'],
    required_features: [],
    src_path: `${REPO_ROOT}/outside/escaped_test.rs`,
    edition: '2024',
  });
}, /integration-test source path mismatch/);

expectFailure((metadata) => {
  packageByName(metadata, 'bcsp-contracts').targets.push({
    ...integrationTarget('example_target'),
    kind: ['example'],
  });
}, /may expose only its primary lib target/);

expectFailure((metadata) => {
  packageByName(metadata, 'bcsp-contracts').targets.push({
    ...integrationTarget('bench_target'),
    kind: ['bench'],
  });
}, /may expose only its primary lib target/);

expectFailure((metadata) => {
  packageByName(metadata, 'bcsp-contracts').targets.push({
    ...integrationTarget(),
    required_features: ['hidden-test'],
  });
}, /must declare no target required features/);

{
  const metadata = fixture();
  packageByName(metadata, 'bcsp-contracts').targets.push(integrationTarget('linked_test'));
  const errors = auditWorkspaceTargetSources(metadata, {
    repoRoot: REPO_ROOT,
    inspectSource: (source) => ({
      isFile: true,
      linkedComponent: source.endsWith('linked_test.rs'),
      realPath: source,
    }),
  });
  assert.ok(errors.some((error) => /symlink or junction/.test(error)));
}

{
  const metadata = fixture();
  packageByName(metadata, 'bcsp-contracts').targets.push(integrationTarget('realpath_escape'));
  const errors = auditWorkspaceTargetSources(metadata, {
    repoRoot: REPO_ROOT,
    inspectSource: (source) => ({
      isFile: true,
      linkedComponent: false,
      realPath: source.endsWith('realpath_escape.rs') ? `${REPO_ROOT}/outside/escape.rs` : source,
    }),
  });
  assert.ok(errors.some((error) => /real path differs/.test(error)));
}

assert.deepEqual(validatePublicSourceDenyDocument(DENY_DOCUMENT), []);

const cleanAudit = auditRustSourceFiles(new Map([
  ['crates/shared/src/lib.rs', 'pub fn allowed() { let _ = "ordinary watch"; } // email dispatcher is not code'],
]), DENY_DOCUMENT);
assert.equal(cleanAudit.checkedCount, 18);
assert.equal(cleanAudit.violationCount, 0);
assert.equal(cleanAudit.sourceEscapeCount, 0);

const markerAudit = auditRustSourceFiles(new Map([
  ['crates/shared/src/lib.rs', 'pub fn email_dispatcher() {}'],
]), DENY_DOCUMENT);
assert.ok(markerAudit.violations.some((violation) => violation.denyId === 'P4-D-EMAIL-SOURCE'));

for (const encodedMarker of [
  'const VALUE: &str = "\\x73aved_view";',
  'const VALUE: &str = "\\u{73}aved_view";',
  'const VALUE: &str = "saved_\\\n  view";',
]) {
  const encodedMarkerAudit = auditRustSourceFiles(new Map([
    ['crates/shared/src/lib.rs', encodedMarker],
  ]), DENY_DOCUMENT);
  assert.ok(encodedMarkerAudit.violations.some((violation) => violation.denyId === 'P4-D-SAVED_VIEWS-SOURCE'));
}

const includeAudit = auditRustSourceFiles(new Map([
  ['crates/shared/src/lib.rs', 'include!("outside.rs");'],
]), DENY_DOCUMENT);
assert.ok(includeAudit.errors.some((error) => /include! is forbidden/.test(error)));

const pathAudit = auditRustSourceFiles(new Map([
  ['crates/shared/src/lib.rs', '#[path = "../outside.rs"] mod outside;'],
]), DENY_DOCUMENT);
assert.ok(pathAudit.errors.some((error) => /path-bearing attribute source redirection is forbidden/.test(error)));

const cfgPathAudit = auditRustSourceFiles(new Map([
  ['crates/shared/src/lib.rs', '#[cfg_attr(windows, path = "../outside.rs")] mod outside;'],
]), DENY_DOCUMENT);
assert.ok(cfgPathAudit.errors.some((error) => /path-bearing attribute source redirection is forbidden/.test(error)));

for (const macroName of ['include_str', 'include_bytes', 'include_dir']) {
  const macroAudit = auditRustSourceFiles(new Map([
    ['crates/shared/src/lib.rs', `const VALUE: &[u8] = ${macroName}!("../outside");`],
  ]), DENY_DOCUMENT);
  assert.ok(macroAudit.errors.some((error) => error.includes(`${macroName}! is forbidden`)));
}

const approvedPublicEmbedAudit = auditRustSourceFiles(new Map([[
  'crates/bcsp-public-runtime/src/host.rs',
  'use include_dir::{Dir, include_dir};\nstatic ASSETS: Dir<\'_> = include_dir!("$OUT_DIR/web-assets");',
]]), DENY_DOCUMENT);
assert.equal(approvedPublicEmbedAudit.sourceEscapeCount, 0);

const expandedPublicEmbedAudit = auditRustSourceFiles(new Map([[
  'crates/bcsp-public-runtime/src/host.rs',
  'use include_dir::{Dir, include_dir};\nstatic ASSETS: Dir<\'_> = include_dir!("$OUT_DIR/web-assets");\nstatic OTHER: Dir<\'_> = include_dir!("../outside");',
]]), DENY_DOCUMENT);
assert.ok(expandedPublicEmbedAudit.errors.some((error) => /include_dir! is forbidden/.test(error)));

const includeAliasAudit = auditRustSourceFiles(new Map([
  ['crates/shared/src/lib.rs', 'use std::include_str as load_text;\nconst VALUE: &str = load_text!("../outside");'],
]), DENY_DOCUMENT);
assert.ok(includeAliasAudit.errors.some((error) => /include_str alias\/reference is forbidden/.test(error)));

const groupedIncludeAliasAudit = auditRustSourceFiles(new Map([
  ['crates/shared/src/lib.rs', 'use std::{include as splice_source};\nsplice_source!("../outside.rs");'],
]), DENY_DOCUMENT);
assert.ok(groupedIncludeAliasAudit.errors.some((error) => /include alias\/reference is forbidden/.test(error)));

const ordinaryIncludeBindingAudit = auditRustSourceFiles(new Map([
  ['crates/shared/src/lib.rs', 'for (include, expected) in [(false, 1), (true, 2)] { assert_eq!(include, expected > 1); }'],
]), DENY_DOCUMENT);
assert.equal(ordinaryIncludeBindingAudit.sourceEscapeCount, 0);

const concatMarkerAudit = auditRustSourceFiles(new Map([
  ['crates/shared/src/lib.rs', 'const VALUE: &str = concat!("saved", "_view");'],
]), DENY_DOCUMENT);
assert.ok(concatMarkerAudit.errors.some((error) => /concat! is forbidden/.test(error)));

const literalEscapeAudit = auditRustSourceFiles(new Map([
  ['crates/shared/src/lib.rs', 'const NOTE: &str = r#"include!(\\"outside.rs\\") and #[path]"#;'],
]), DENY_DOCUMENT);
assert.equal(literalEscapeAudit.sourceEscapeCount, 0);

const changedDenyDocument = structuredClone(DENY_DOCUMENT);
changedDenyDocument.capabilities[0].markers[0] = 'changed_marker';
assert.ok(validatePublicSourceDenyDocument(changedDenyDocument).some((error) => /digest mismatch/.test(error)));

const changedValidationIdDocument = structuredClone(DENY_DOCUMENT);
changedValidationIdDocument.capabilities[0].validationId = 'P4-Z-CHANGED-SOURCE';
assert.ok(validatePublicSourceDenyDocument(changedValidationIdDocument).some((error) => /validation ID/.test(error)));

const liveDependencyDocument = structuredClone(DENY_DOCUMENT);
liveDependencyDocument.runtimeSourceRequired = true;
assert.ok(validatePublicSourceDenyDocument(liveDependencyDocument).some((error) => /must not require/.test(error)));

const jsonText = formatReport(createReport([]), true);
assert.equal(JSON.parse(jsonText).ok, true);
assert.equal(JSON.parse(jsonText).schemaVersion, 2);
assert.equal(JSON.parse(jsonText).cargoResolver, '3');
assert.deepEqual(JSON.parse(jsonText).rootManifestSections, [
  'workspace',
  'workspace.package',
  'workspace.dependencies',
  'workspace.metadata.release.tooling',
]);
assert.equal(JSON.parse(jsonText).workspaceDefaultMembers, 15);
assert.equal(jsonText.trim(), JSON.stringify(JSON.parse(jsonText)));


// The two Rust scanners must refuse the same things. The page-level
// notification markers left the SHARED set so a browser page that declares
// the capability may use the browser API; nothing about that applies to Rust,
// and a scanner that quietly stopped checking would leave the other one as
// the only thing standing between a server-side notification and the public
// build.
{
  const zeroSurface = await import('./verify-public-rust-zero-surface.mjs');
  assert.deepEqual(
    SUPPLEMENTAL_RUST_MARKERS,
    zeroSurface.SUPPLEMENTAL_MARKERS,
    'the two Rust scanners must apply the same supplemental markers',
  );
  for (const marker of ['notification_permission', 'browser_notification_api', 'desktop_notification']) {
    const audit = auditRustSourceFiles(
      new Map([['crates/bcsp-public-runtime/src/notify.rs', `fn ${marker}() {}`]]),
      DENY_DOCUMENT,
    );
    assert.ok(
      audit.violations.some((violation) => violation.marker === marker),
      `${marker} must still be refused in the public Rust source closure`,
    );
  }
}
process.stdout.write('verify-rust-graph tests: PASS\n');
