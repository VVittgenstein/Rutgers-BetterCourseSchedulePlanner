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
  readFileSync(new URL('./public-source-deny.json', import.meta.url), 'utf8'),
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

// The page-level notification markers left the SHARED set so a browser page
// that declares the capability may use the browser API. Rust is not a browser
// page: a notification the server could raise would fire with no page running
// and nothing to verify it against, so all three stay denied here -- on every
// surface, and with no declaration that can exempt them.
for (const [marker, source] of [
  ['notification_permission', 'fn notification_permission() -> bool { true }'],
  ['browser_notification_api', 'const BROWSER_NOTIFICATION_API: &str = "x";'],
  ['desktop_notification', 'struct DesktopNotification;'],
]) {
  for (const surface of ['SOURCE', 'API', 'STORAGE', 'PACKAGE']) {
    const audit = auditSurface(
      new Map([['crates/public/src/notify.rs', source]]),
      DENY_DOCUMENT,
      surface,
    );
    assert.ok(
      audit.violations.some((violation) => (
        violation.denyId === `P4-D-SYSTEM_NOTIFICATIONS-${surface}`
        && violation.marker === marker
      )),
      `${marker} must still be refused on ${surface}`,
    );
  }
}

// And the split is narrow: what a page cannot own is still refused from the
// shared set itself, with no supplement involved.
for (const [marker, source] of [
  ['show_notification', 'fn show_notification() {}'],
  ['service_worker_notification', 'const SERVICE_WORKER_NOTIFICATION: u8 = 1;'],
  ['web_push', 'mod web_push {}'],
]) {
  const audit = auditSurface(
    new Map([['crates/public/src/notify.rs', source]]),
    DENY_DOCUMENT,
    'SOURCE',
  );
  assert.ok(
    audit.violations.some((violation) => violation.marker === marker),
    `${marker} must remain in the shared deny set`,
  );
}

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

// The desired-watch authority is local-only by construction, and these three
// markers are what keeps it that way once the construction stops being
// obvious. Each one is a distinct way the authority could leak into the
// public closure: its route or its types, the generation barrier it is
// versioned by, and the epoch its materialization is keyed on.
for (const [file, contents, marker] of [
  ['crates/public/src/host.rs', 'const PATH: &str = "/api/v1/local/desired-watch";', 'desired_watch'],
  ['crates/public/src/wire.rs', 'struct Bootstrap { authority_generation: u64 }', 'authority_generation'],
  ['crates/public/src/wire.rs', 'struct Row { materialization_epoch: u64 }', 'materialization_epoch'],
]) {
  const leaked = auditSurface(new Map([[file, contents]]), DENY_DOCUMENT, 'SOURCE');
  assert.ok(
    leaked.violations.some((violation) => (
      violation.denyId === 'P4-D-PERSISTENT_ACTIVE_WATCH-SOURCE' && violation.marker === marker
    )),
    `the public SOURCE surface must reject ${marker}`,
  );
}

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
