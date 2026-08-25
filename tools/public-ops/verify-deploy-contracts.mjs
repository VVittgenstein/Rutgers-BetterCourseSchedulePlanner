// Static contracts over the shipped deployment files (P2 hardening).
//
// The real proof for these files needs a Linux host -- systemd reading the
// unit back, Caddy holding a stream open across a reload -- and lives in
// ops/verify.sh, tests/disposable-host.sh, and the soak harness. This
// script pins the repository-side half: the frozen lines must exist, in
// the right section, with the frozen values, so a refactor cannot silently
// drop one before CI ever reaches a Linux runner.
//
// Run directly: node tools/public-ops/verify-deploy-contracts.mjs

import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const read = (relative) => readFileSync(join(repoRoot, relative), 'utf8');

// --- deploy/public/systemd/bcsp.service -----------------------------------

const unit = read('deploy/public/systemd/bcsp.service');

/** Returns the body of one INI section, comments stripped. */
function unitSection(name) {
  const lines = unit.split('\n');
  const start = lines.findIndex((line) => line.trim() === `[${name}]`);
  assert.notEqual(start, -1, `bcsp.service must carry a [${name}] section`);
  const body = [];
  for (const line of lines.slice(start + 1)) {
    if (/^\[.+\]$/.test(line.trim())) {
      break;
    }
    if (line.trim() !== '' && !line.trim().startsWith('#')) {
      body.push(line.trim());
    }
  }
  return body;
}

const unitSettings = unitSection('Unit');
const serviceSettings = unitSection('Service');

// H1: the file-descriptor ceiling, in [Service].
assert.ok(
  serviceSettings.includes('LimitNOFILE=65536'),
  'H1: [Service] must set LimitNOFILE=65536',
);

// H6: soft memory pressure and an unbroken restart policy.
assert.ok(
  serviceSettings.includes('MemoryHigh=700M'),
  'H6: [Service] must set MemoryHigh=700M',
);
const everySetting = [...unitSettings, ...serviceSettings, ...unitSection('Install')];
assert.ok(
  !everySetting.some((line) => line.startsWith('MemoryMax')),
  'H6: soft pressure only -- MemoryMax must stay unset',
);
assert.ok(
  unitSettings.includes('StartLimitIntervalSec=0'),
  'H6: [Unit] must disable start rate limiting with StartLimitIntervalSec=0',
);
assert.ok(
  !everySetting.some((line) => line.startsWith('StartLimitBurst')),
  'H6: a StartLimitBurst line under StartLimitIntervalSec=0 is dead config',
);
assert.ok(
  serviceSettings.includes('Restart=on-failure'),
  'H6: the restart policy the unbounded start limit exists for',
);
assert.ok(
  serviceSettings.includes('RestartSec=5s'),
  'H6: the retry cadence the runbook documents',
);

// The read-back half: ops/verify.sh must actually ask systemd for the
// loaded values, not trust the file (design H1 verification clause).
const opsVerify = read('deploy/public/ops/verify.sh');
for (const property of ['LimitNOFILE', 'MemoryHigh', 'StartLimitIntervalUSec']) {
  assert.ok(
    opsVerify.includes(`--property=${property}`),
    `ops/verify.sh must read ${property} back from the loaded unit`,
  );
}
assert.ok(
  opsVerify.includes('65536') && opsVerify.includes('734003200'),
  'ops/verify.sh must pin the exact read-back values (65536 fds, 700M in bytes)',
);

// The crash-loop drill: six SIGKILLs, each survived (design H6 verification
// clause). The count is the point -- five would pass under the old
// StartLimitBurst=5.
const disposable = read('deploy/public/tests/disposable-host.sh');
assert.ok(
  /for kill_attempt in 1 2 3 4 5 6; do/.test(disposable),
  'disposable-host.sh must run the six-SIGKILL recovery drill',
);
assert.ok(
  disposable.includes('kill -9 "$crashed_pid"'),
  'the drill must use a real SIGKILL',
);

// --- deploy/public/config environment surface -----------------------------

const schema = JSON.parse(read('deploy/public/config/bcsp.env.schema.json'));
assert.deepEqual(schema.required, ['BCSP_PUBLIC_ORIGIN']);
assert.deepEqual(
  Object.keys(schema.properties).sort(),
  ['BCSP_PUBLIC_ORIGIN', 'BCSP_PUBLIC_WS_PER_CLIENT_LIMIT'],
  'the env schema carries exactly the origin and the one tunable H4 value',
);
assert.equal(schema.additionalProperties, false);
assert.deepEqual(schema.secretVariables, []);

const limitPattern = new RegExp(
  `^(?:${schema.properties.BCSP_PUBLIC_WS_PER_CLIENT_LIMIT.pattern.slice(1, -1)})$`,
);
for (const accepted of ['1', '64', '999', '1000', '1019', '1020', '1024']) {
  assert.ok(limitPattern.test(accepted), `schema must accept ${accepted}`);
}
for (const refused of ['0', '1025', '01', '2000', '-1', '64.5', '']) {
  assert.ok(!limitPattern.test(refused), `schema must refuse ${refused}`);
}

const envExample = read('deploy/public/config/bcsp.env.example');
const activeLines = envExample
  .split('\n')
  .filter((line) => line.trim() !== '' && !line.trim().startsWith('#'));
assert.deepEqual(
  activeLines,
  ['BCSP_PUBLIC_ORIGIN=https://planner.invalid'],
  'the env example ships exactly one active setting; the tunable stays commented',
);
assert.ok(
  envExample.includes('BCSP_PUBLIC_WS_PER_CLIENT_LIMIT'),
  'the env example must document the one tunable value',
);

console.log('public deploy contracts: PASS');
