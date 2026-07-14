#!/usr/bin/env node

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

import {
  PUBLIC_RUST_SURFACE_SCOPES,
  PUBLIC_RUST_ZERO_SURFACES,
  auditPublicRustZeroSurfaces,
  auditSurface,
  createReport,
  formatReport,
} from './verify-public-rust-zero-surface.mjs';

const DENY_DOCUMENT = JSON.parse(
  readFileSync(new URL('./p4-public-source-deny.json', import.meta.url), 'utf8'),
);

const cleanFiles = Object.fromEntries(PUBLIC_RUST_ZERO_SURFACES.map((surface) => [
  surface,
  new Map([['crates/public/src/lib.rs', 'pub fn ordinary_course_search() {}']]),
]));
const clean = auditPublicRustZeroSurfaces(cleanFiles, DENY_DOCUMENT, {
  packageInputRoots: ['Cargo.lock'],
});
assert.equal(clean.checkedCount, 72);
assert.equal(clean.expectedCount, 72);
assert.deepEqual(clean.errors, []);
for (const surface of PUBLIC_RUST_ZERO_SURFACES) {
  assert.equal(clean.surfaces[surface].denyIds.length, 18);
  assert.equal(clean.surfaces[surface].validationIds.length, 18);
  assert.ok(clean.surfaces[surface].denyIds.every((id) => id.endsWith(`-${surface}`)));
  assert.ok(clean.surfaces[surface].validationIds.every((id) => id.endsWith(`-${surface}`)));
  assert.equal(clean.surfaces[surface].scope, PUBLIC_RUST_SURFACE_SCOPES[surface]);
}
assert.deepEqual(clean.surfaces.PACKAGE.inputRoots, ['Cargo.lock']);

const source = auditSurface(
  new Map([['crates/public/src/host.rs', 'router.route("/api/v1/saved-views", get(handler));']]),
  DENY_DOCUMENT,
  'SOURCE',
);
assert.ok(source.violations.some((violation) => violation.denyId === 'P4-D-SAVED_VIEWS-SOURCE'));

const api = auditSurface(
  new Map([['crates/public/src/api.rs', 'const BODY: &str = r#"{"savedViews":[]}"#;']]),
  DENY_DOCUMENT,
  'API',
);
assert.ok(api.violations.some((violation) => violation.denyId === 'P4-D-SAVED_VIEWS-API'));

const supplementalApi = auditSurface(
  new Map([['crates/public/src/api.rs', 'enum StopReason { LocalUserDataReset }']]),
  DENY_DOCUMENT,
  'API',
);
assert.ok(supplementalApi.violations.some((violation) => (
  violation.denyId === 'P4-D-LOCAL_RESET-API'
  && violation.marker === 'local_user_data_reset'
)));

const storage = auditSurface(
  new Map([['crates/public/migrations/0003_bad.sql', 'CREATE TABLE saved_views (id TEXT);']]),
  DENY_DOCUMENT,
  'STORAGE',
);
assert.ok(storage.violations.some((violation) => violation.denyId === 'P4-D-SAVED_VIEWS-STORAGE'));

const packageAudit = auditSurface(
  new Map([['crates/public/Cargo.toml', 'mailgun = "1"']]),
  DENY_DOCUMENT,
  'PACKAGE',
);
assert.ok(packageAudit.violations.some((violation) => violation.denyId === 'P4-D-EMAIL-PACKAGE'));

const packageFilename = auditSurface(
  new Map([['deploy/public/saved-views.toml', 'ordinary = true']]),
  DENY_DOCUMENT,
  'PACKAGE',
);
assert.ok(packageFilename.violations.some((violation) => (
  violation.denyId === 'P4-D-SAVED_VIEWS-PACKAGE'
)));

const invalidSurface = auditSurface(new Map(), DENY_DOCUMENT, 'DOM');
assert.ok(invalidSurface.errors.some((error) => /unsupported public Rust zero surface/u.test(error)));

const metadata = {
  packages: [{ id: 'public', features: {} }],
  workspace_members: ['public'],
};
const report = createReport({
  audit: clean,
  closure: ['bcsp-server', 'bcsp-public-runtime'],
  errors: [],
  metadata,
});
assert.equal(report.ok, true);
assert.equal(report.cargoGuard.declaredFeatureCount, 0);
assert.equal(report.cargoGuard.localOnlyPackagesAbsent, true);
assert.match(formatReport(report), /^PASS P7\.1-013_PUBLIC_RUST_ZERO_SURFACE:/u);
assert.equal(JSON.parse(formatReport(report, true)).publicRustZeroSurface.checkedCount, 72);

process.stdout.write('verify-public-rust-zero-surface tests: PASS\n');
