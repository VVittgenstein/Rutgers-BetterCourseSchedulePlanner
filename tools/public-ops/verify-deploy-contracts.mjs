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

/**
 * systemd is last-value-wins, so `includes` alone would bless a unit that
 * sets the frozen value and then overrides it further down. Each pinned
 * key must therefore appear EXACTLY once in its section, with the frozen
 * value.
 */
function assertOnlySetting(settings, expected, why) {
  const key = expected.slice(0, expected.indexOf('=') + 1);
  const occurrences = settings.filter((line) => line.startsWith(key));
  assert.deepEqual(occurrences, [expected], why);
}

// H1: the file-descriptor ceiling, in [Service].
assertOnlySetting(
  serviceSettings,
  'LimitNOFILE=65536',
  'H1: [Service] must set LimitNOFILE=65536 exactly once',
);

// H6: soft memory pressure and an unbroken restart policy.
assertOnlySetting(
  serviceSettings,
  'MemoryHigh=700M',
  'H6: [Service] must set MemoryHigh=700M exactly once',
);
const everySetting = [...unitSettings, ...serviceSettings, ...unitSection('Install')];
assert.ok(
  !everySetting.some((line) => line.startsWith('MemoryMax')),
  'H6: soft pressure only -- MemoryMax must stay unset',
);
assertOnlySetting(
  unitSettings,
  'StartLimitIntervalSec=0',
  'H6: [Unit] must disable start rate limiting with StartLimitIntervalSec=0, exactly once',
);
assert.ok(
  !everySetting.some((line) => line.startsWith('StartLimitBurst')),
  'H6: a StartLimitBurst line under StartLimitIntervalSec=0 is dead config',
);
assertOnlySetting(
  serviceSettings,
  'Restart=on-failure',
  'H6: the restart policy the unbounded start limit exists for',
);
assertOnlySetting(
  serviceSettings,
  'RestartSec=5s',
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

// --- deploy/public/ops/upgrade.sh -----------------------------------------

// H3: the same-release branch is a liveness check and nothing else. The
// executable proof is deploy/public/tests/upgrade-noop.sh (Linux; recording
// systemctl stub must stay empty); this pin catches the regression on any
// platform by reading the branch itself.
const upgrade = read('deploy/public/ops/upgrade.sh');
const sameReleaseStart = upgrade.indexOf('if [[ "$previous" == "$release_path" ]]; then');
assert.notEqual(sameReleaseStart, -1, 'upgrade.sh must keep its same-release branch');
// The exact closing line, not a prefix: '\n  fi' would also match a line
// like '  file=...' and silently truncate the scanned branch.
const sameReleaseEnd = upgrade.indexOf('\n  fi\n', sameReleaseStart);
assert.notEqual(sameReleaseEnd, -1);
const sameReleaseBranch = upgrade.slice(sameReleaseStart, sameReleaseEnd);
assert.ok(
  sameReleaseBranch.includes('bcsp_wait_for_liveness'),
  'H3: the same-release branch must still verify liveness',
);
for (const forbidden of ['bcsp_restart_service', 'bcsp_reload_enable_service']) {
  assert.ok(
    !sameReleaseBranch.includes(forbidden),
    `H3: the same-release branch must not call ${forbidden} -- that restart is what cleared every watch`,
  );
}

// --- deploy/public/caddy/Caddyfile.example --------------------------------

const caddyfile = read('deploy/public/caddy/Caddyfile.example');
const caddySettings = caddyfile
  .split('\n')
  .map((line) => line.trim())
  .filter((line) => line !== '' && !line.startsWith('#'));

// H2: the reverse_proxy directive must open a block whose settings include
// the four-hour stream drain; a bare `reverse_proxy 127.0.0.1:8080` is the
// exact pre-hardening shape that cut every monitor on reload.
const proxyAt = caddySettings.findIndex((line) =>
  line.startsWith('reverse_proxy 127.0.0.1:8080'),
);
assert.notEqual(proxyAt, -1, 'the site must proxy to 127.0.0.1:8080');
assert.ok(
  caddySettings[proxyAt].endsWith('{'),
  'H2: reverse_proxy must open a block (stream_close_delay lives inside it)',
);
const proxyBlockEnd = caddySettings.indexOf('}', proxyAt);
assert.notEqual(proxyBlockEnd, -1, 'the reverse_proxy block must close');
assert.ok(
  caddySettings.slice(proxyAt + 1, proxyBlockEnd).includes('stream_close_delay 4h'),
  'H2: reverse_proxy must set stream_close_delay 4h',
);

// The metrics refusal must survive edits around the proxy block.
assert.ok(
  caddySettings.includes('respond @metrics 404'),
  'the public edge must keep refusing /metrics',
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

// --- packaging allowlist consistency (H8) ---------------------------------

// The shipped-file list lives in four hardcoded places; this pins the two
// machine-readable ones against each other so adding a file (like
// ops/preflight.sh) cannot drift them apart. build.sh, verify.sh, and
// verify-release-set.ps1 pin the count again at build time.
const releaseInputs = JSON.parse(read('packaging/release-inputs.json'));
const linuxPackage = releaseInputs.packages.find(
  (entry) => entry.id === 'LINUX_PUBLIC_DEPLOYMENT_PACKAGE',
);
assert.ok(linuxPackage, 'release-inputs.json must define the Linux package');
assert.equal(linuxPackage.allowlist.length, 22, 'the Linux allowlist ships 22 files');
assert.ok(
  linuxPackage.allowlist.includes('ops/preflight.sh'),
  'H8: the preflight ships with the candidate',
);

const lib = read('deploy/public/ops/lib.sh');
const expectedFilesMatch = lib.match(/expected_files=\$'([^']+)'/);
assert.ok(expectedFilesMatch, 'lib.sh must keep its literal expected_files list');
const libFiles = expectedFilesMatch[1].split('\\n');
assert.deepEqual(
  [...linuxPackage.allowlist].sort(),
  [...libFiles].sort(),
  'release-inputs.json and bcsp_validate_candidate_root must agree on the shipped files',
);

// The Ubuntu 24.04 archive currently carries Caddy 2.6, which cannot parse
// H2's stream_close_delay. The real-stack soak must therefore select an
// official, immutable release asset, verify its bytes, and prove which
// version it is about to use. Falling back to `apt install caddy` turns the
// 600-second gate into an environment-dependent preflight failure.
const workflow = read('.github/workflows/public-ops.yml');
for (const fragment of [
  'CADDY_VERSION: 2.11.4',
  'CADDY_LINUX_AMD64_SHA256: 527fbf917c39189a1e3b31d34fa955601680b2d5c8055d2a87b8b9588dec7bb9',
  'caddyserver/caddy/releases/download/v${CADDY_VERSION}',
  'sha256sum --check --strict',
  'test "$(caddy version | awk',
]) {
  assert.ok(workflow.includes(fragment), `the soak workflow must retain ${fragment}`);
}
assert.ok(
  !/apt-get install[^\n]*\bcaddy\b/.test(workflow),
  'the soak workflow must not fall back to Ubuntu\'s too-old Caddy package',
);

// H9 reads an aggregate accepted-ACK counter. Its pre-browser evidence must
// bracket any competing capacity permit: monotonic admissions first, the
// permit gauge second, and ACKs last. Sampling the post-upgrade connection
// gauge would leave an upgrade-pending socket invisible.
const soak = read('deploy/public/tests/public-soak.sh');
const admissionsAt = soak.indexOf(
  'ADMISSIONS_BASELINE="$(read_public_metric bcsp_websocket_admissions_granted)"',
);
const admittedAt = soak.indexOf(
  'ADMITTED_BEFORE="$(read_public_metric bcsp_websocket_admitted_connections)"',
);
const ackAt = soak.indexOf(
  'ACK_BASELINE="$(read_public_metric bcsp_websocket_heartbeat_acks_accepted)"',
);
assert.ok(
  admissionsAt >= 0 && admissionsAt < admittedAt && admittedAt < ackAt,
  'H9 must read admissions, the admitted permit gauge, then the ACK baseline',
);
assert.ok(
  soak.includes('awk \'$1 == "bcsp_websocket_admitted_connections" { print $2 }\''),
  'H9 samples must count upgrade-pending capacity permits, not only open sockets',
);
assert.ok(
  soak.includes('--admitted-before "$ADMITTED_BEFORE"'),
  'H9 must pass the pre-browser permit gauge to the evidence analyzer',
);

console.log('public deploy contracts: PASS');
