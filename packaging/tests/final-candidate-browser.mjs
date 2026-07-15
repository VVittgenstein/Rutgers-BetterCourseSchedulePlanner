#!/usr/bin/env node

import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { resolve } from 'node:path';

function usage() {
  process.stderr.write(
    'usage: final-candidate-browser.mjs --mode public|local --base-url URL --playwright-root PATH\n',
  );
  process.exit(2);
}

function parseArguments(argv) {
  const options = { baseUrl: '', mode: '', playwrightRoot: '' };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = argv[index + 1];
    if (value === undefined) usage();
    if (argument === '--mode') options.mode = value;
    else if (argument === '--base-url') options.baseUrl = value;
    else if (argument === '--playwright-root') options.playwrightRoot = value;
    else usage();
    index += 1;
  }
  if (!['local', 'public'].includes(options.mode)
    || options.baseUrl.length === 0
    || options.playwrightRoot.length === 0) usage();
  return options;
}

const options = parseArguments(process.argv.slice(2));
const parsedBaseUrl = new URL(options.baseUrl);
assert.equal(parsedBaseUrl.username, '', 'the base URL must not contain credentials');
assert.equal(parsedBaseUrl.password, '', 'the base URL must not contain credentials');
assert.ok(parsedBaseUrl.pathname === '/' || parsedBaseUrl.pathname === '',
  'the base URL must not contain a path');
assert.equal(parsedBaseUrl.search, '', 'the base URL must not contain a query');
assert.equal(parsedBaseUrl.hash, '', 'the base URL must not contain a fragment');
if (options.mode === 'public') {
  assert.equal(parsedBaseUrl.protocol, 'https:', 'public acceptance requires HTTPS');
} else {
  assert.ok(['http:', 'https:'].includes(parsedBaseUrl.protocol),
    'local acceptance requires HTTP or HTTPS');
}
const origin = parsedBaseUrl.origin;

const requireFromFrontend = createRequire(resolve(options.playwrightRoot, 'package.json'));
const { chromium, devices } = requireFromFrontend('playwright');
const executablePath = process.env.BCSP_BROWSER_EXECUTABLE;
const browser = await chromium.launch({
  args: ['--no-proxy-server'],
  headless: true,
  ...(executablePath === undefined || executablePath.length === 0 ? {} : { executablePath }),
  ...(typeof process.getuid === 'function' && process.getuid() === 0
    ? { chromiumSandbox: false }
    : {}),
});

const scenarios = [
  {
    context: {
      colorScheme: 'light',
      locale: 'en-US',
      reducedMotion: 'reduce',
      viewport: { height: 1000, width: 1440 },
    },
    name: 'desktop',
  },
  {
    context: {
      ...devices['Pixel 7'],
      colorScheme: 'light',
      locale: 'en-US',
      reducedMotion: 'reduce',
    },
    name: 'mobile',
  },
];

async function waitForReady(page) {
  const application = page.locator('[data-bcsp-shared-application]');
  await application.waitFor({ state: 'visible' });
  await page.waitForFunction(() => (
    document.querySelector('[data-bcsp-shared-application]')
      ?.getAttribute('data-bcsp-product-state') !== 'LOADING'
  ));
  assert.equal(await application.getAttribute('data-bcsp-product-state'), 'READY');
}

async function assertNoHorizontalOverflow(page, scenario) {
  const dimensions = await page.evaluate(() => ({
    clientWidth: document.documentElement.clientWidth,
    scrollWidth: document.documentElement.scrollWidth,
  }));
  assert.ok(
    dimensions.scrollWidth <= dimensions.clientWidth + 1,
    `${scenario}: horizontal overflow ${dimensions.scrollWidth}/${dimensions.clientWidth}`,
  );
}

async function assertAssetPaths(page, scenario) {
  const paths = await page.locator('script[src]').evaluateAll((scripts) => scripts.map((script) => {
    const url = new URL(script.src);
    return { origin: url.origin, pathname: url.pathname };
  }));
  assert.ok(paths.length > 0, `${scenario}: the embedded module script is absent`);
  assert.ok(
    paths.every((path) => path.origin === origin && path.pathname.startsWith('/assets/')),
    `${scenario}: module scripts are not same-origin /assets resources: ${JSON.stringify(paths)}`,
  );
}

async function assertWebSocket(page, sessionNonce, scenario) {
  const protocol = await page.evaluate(async ({ nonce }) => {
    const url = new URL('/api/v1/watch', window.location.href);
    url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
    url.search = '';
    url.searchParams.set('session', nonce);
    return await new Promise((resolveSocket, rejectSocket) => {
      const socket = new WebSocket(url, ['bcsp.v1']);
      const timeout = setTimeout(() => {
        socket.close();
        rejectSocket(new Error('WebSocket open timeout'));
      }, 8_000);
      socket.addEventListener('open', () => {
        clearTimeout(timeout);
        const selectedProtocol = socket.protocol;
        socket.close(1000, 'final candidate acceptance complete');
        resolveSocket(selectedProtocol);
      }, { once: true });
      socket.addEventListener('error', () => {
        clearTimeout(timeout);
        rejectSocket(new Error('WebSocket failed'));
      }, { once: true });
    });
  }, { nonce: sessionNonce });
  assert.equal(protocol, 'bcsp.v1', `${scenario}: the real WebSocket subprotocol was not accepted`);
}

async function assertPublicBoundary(page, scenario) {
  const boundary = await page.evaluate(() => {
    const body = document.body.innerText;
    const forbiddenMessages = [
      'Saved views',
      'Local watch history',
      'Local settings',
      'Reset scopes',
      'Reset all local user data',
    ];
    return {
      forbiddenMessages: forbiddenMessages.filter((message) => body.includes(message)),
      localDom: document.querySelectorAll(
        '.local-personal, [data-bcsp-local-page], .local-page__reset-scope',
      ).length,
      localLinks: document.querySelectorAll(
        'a[href="/saved-views"], a[href="/history"], a[href="/settings"]',
      ).length,
    };
  });
  assert.deepEqual(boundary, { forbiddenMessages: [], localDom: 0, localLinks: 0 },
    `${scenario}: public composition exposed a local-only surface`);
  const statuses = await page.evaluate(async () => Promise.all(
    ['/saved-views', '/history', '/settings'].map(async (path) => {
      const response = await fetch(path, { cache: 'no-store', redirect: 'manual' });
      return [path, response.status];
    }),
  ));
  assert.deepEqual(statuses, [
    ['/saved-views', 404],
    ['/history', 404],
    ['/settings', 404],
  ], `${scenario}: a local-only public deep link was served`);
}

async function publicSession(page) {
  return await page.locator('#bcsp-bootstrap').evaluate((element) => {
    const envelope = JSON.parse(element.textContent ?? 'null');
    const nonce = envelope?.data?.sessionNonce;
    if (typeof nonce !== 'string') throw new Error('public document session nonce is absent');
    return nonce;
  });
}

async function assertTls(response, scenario) {
  assert.ok(response !== null && response.ok(), `${scenario}: public document did not return HTTP 200`);
  const security = await response.securityDetails();
  assert.ok(security !== null, `${scenario}: browser did not establish TLS`);
  assert.match(security.protocol ?? '', /TLS/iu, `${scenario}: unexpected TLS protocol`);
}

async function exercisePublic(page, scenario) {
  const response = await page.goto(`${origin}/`, { waitUntil: 'domcontentloaded' });
  await assertTls(response, `${scenario}/root`);
  await waitForReady(page);
  await assertAssetPaths(page, `${scenario}/root`);
  await assertPublicBoundary(page, `${scenario}/root`);
  await assertNoHorizontalOverflow(page, `${scenario}/root`);
  const firstSession = await publicSession(page);
  await assertWebSocket(page, firstSession, `${scenario}/root`);

  await page.locator('.bcsp-navigation a[href="/watch"]').click();
  await page.waitForURL(`${origin}/watch`);
  await page.locator('.watch-workspace').waitFor({ state: 'visible' });
  const watchModes = page.locator('input[name="watch-mode"]');
  await watchModes.nth(1).check();
  await page.locator('.watch-workspace__confirm input[type="checkbox"]').check();
  await page.locator('#watch-max-audible').fill('5');
  await page.locator('#watch-volume').fill('35');
  assert.equal(await page.locator('#watch-max-audible').inputValue(), '5');
  assert.equal(await page.locator('#watch-volume').inputValue(), '35');

  const reloadResponse = await page.reload({ waitUntil: 'domcontentloaded' });
  await assertTls(reloadResponse, `${scenario}/reload`);
  await waitForReady(page);
  await page.locator('.watch-workspace').waitFor({ state: 'visible' });
  const secondSession = await publicSession(page);
  assert.notEqual(secondSession, firstSession, `${scenario}: reload reused the public session`);
  assert.equal(await page.locator('input[name="watch-mode"]').first().isChecked(), true);
  assert.equal(await page.locator('#watch-max-audible').inputValue(), '3');
  assert.equal(await page.locator('#watch-volume').inputValue(), '70');
  const metrics = await page.locator(
    '.watch-workspace__status-strip .bcsp-metric dd',
  ).allTextContents();
  assert.equal(metrics[0]?.trim(), '0', `${scenario}: selected Sections survived reload`);
  assert.equal(metrics[1]?.trim(), '0', `${scenario}: active watches survived reload`);
  await assertAssetPaths(page, `${scenario}/reload`);
  await assertPublicBoundary(page, `${scenario}/reload`);
  await assertNoHorizontalOverflow(page, `${scenario}/reload`);
  await assertWebSocket(page, secondSession, `${scenario}/reload`);
}

async function localSession(page) {
  return await page.evaluate(async () => {
    const response = await fetch('/api/v1/local/bootstrap', { cache: 'no-store' });
    if (!response.ok) throw new Error(`local bootstrap returned HTTP ${response.status}`);
    const envelope = await response.json();
    const nonce = envelope?.data?.sessionNonce;
    if (typeof nonce !== 'string') throw new Error('local session nonce is absent');
    return nonce;
  });
}

async function exerciseLocal(page, scenario) {
  const response = await page.goto(`${origin}/`, { waitUntil: 'domcontentloaded' });
  assert.ok(response !== null && response.ok(), `${scenario}: local root did not return HTTP 200`);
  await waitForReady(page);
  await assertAssetPaths(page, `${scenario}/root`);
  await assertNoHorizontalOverflow(page, `${scenario}/root`);
  await assertWebSocket(page, await localSession(page), `${scenario}/root`);

  const settingsResponse = await page.goto(`${origin}/settings`, { waitUntil: 'domcontentloaded' });
  assert.ok(settingsResponse !== null && settingsResponse.ok(),
    `${scenario}: local Settings deep link did not return HTTP 200`);
  await waitForReady(page);
  assert.equal(new URL(page.url()).pathname, '/settings');
  await assertAssetPaths(page, `${scenario}/settings`);
  const scopes = page.locator('.local-page__reset-scope');
  await scopes.first().waitFor({ state: 'visible' });
  const scopeKinds = (await scopes.evaluateAll((elements) => elements.map(
    (element) => element.getAttribute('data-scope'),
  ))).sort();
  assert.deepEqual(scopeKinds, ['ALL', 'FILTERS', 'VIEWS'],
    `${scenario}: local reset scopes are incomplete`);
  const filterScope = page.locator('.local-page__reset-scope[data-scope="FILTERS"]');
  const filterReset = filterScope.locator('button').first();
  assert.ok(await filterReset.count() > 0,
    `${scenario}: Reset filters UI is not actionable`);
  assert.equal(await filterReset.isEnabled(), true,
    `${scenario}: Reset filters action is disabled`);
  await filterReset.click();
  const confirmReset = filterScope.locator('[role="group"] button').first();
  await confirmReset.waitFor({ state: 'visible' });
  assert.equal(await confirmReset.isEnabled(), true,
    `${scenario}: Reset filters confirmation is disabled`);
  await confirmReset.click();
  await filterScope.locator('[role="group"]').waitFor({ state: 'detached' });
  await assertNoHorizontalOverflow(page, `${scenario}/settings`);
}

try {
  for (const scenario of scenarios) {
    const context = await browser.newContext(scenario.context);
    const page = await context.newPage();
    try {
      if (options.mode === 'public') await exercisePublic(page, scenario.name);
      else await exerciseLocal(page, scenario.name);
    } finally {
      await context.close();
    }
  }
  process.stdout.write(`FINAL_CANDIDATE_BROWSER_PASS mode=${options.mode} scenarios=${scenarios.length}\n`);
} finally {
  await browser.close();
}
