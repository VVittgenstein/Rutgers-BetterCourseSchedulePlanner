#!/usr/bin/env node

import assert from 'node:assert/strict';

import { chromium } from 'playwright';

const origin = process.env.BCSP_EMBEDDED_RUNTIME_URL;
assert.ok(origin, 'BCSP_EMBEDDED_RUNTIME_URL is required');
const executablePath = process.env.BCSP_BROWSER_EXECUTABLE
  ?? (process.platform === 'win32'
    ? 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe'
    : undefined);

const browser = await chromium.launch({
  headless: true,
  ...(executablePath === undefined ? {} : { executablePath }),
});

try {
  const page = await browser.newPage();
  const browserErrors = [];
  const failedRequests = [];
  const nonSuccessfulResponses = [];
  const observedResponses = new Map();

  page.on('console', (message) => {
    if (message.type() === 'error') {
      const location = message.location();
      browserErrors.push(
        `${message.text()} @ ${location.url || '<unknown>'}:${location.lineNumber}:${location.columnNumber}`,
      );
    }
  });
  page.on('pageerror', (error) => browserErrors.push(error.message));
  page.on('requestfailed', (request) => {
    failedRequests.push(`${request.method()} ${request.url()}: ${request.failure()?.errorText ?? 'failed'}`);
  });
  page.on('response', (response) => {
    const url = new URL(response.url());
    if (url.origin === origin) {
      observedResponses.set(url.pathname, response.status());
      if (response.status() >= 400) {
        nonSuccessfulResponses.push(
          `${response.request().method()} ${url.pathname}${url.search}: ${response.status()}`,
        );
      }
    }
  });

  await page.goto(`${origin}/`, { waitUntil: 'domcontentloaded' });
  const application = page.locator('[data-bcsp-shared-application]');
  await application.waitFor({ state: 'visible' });
  await page.waitForFunction(() => {
    const shell = document.querySelector('[data-bcsp-shared-application]');
    return shell?.getAttribute('data-bcsp-product-state') !== 'LOADING';
  });

  assert.equal(await application.getAttribute('data-bcsp-product-state'), 'READY');
  assert.ok(
    await page.locator('style[data-bcsp-design-system], style[data-bcsp-shell-styles]').count() >= 2,
    'the React design-system styles must be present',
  );
  const visualState = await application.evaluate((element) => {
    const style = getComputedStyle(element);
    return { display: style.display, fontFamily: style.fontFamily };
  });
  assert.notEqual(visualState.display, 'none');
  assert.ok(visualState.fontFamily.length > 0);

  const socketProtocol = await page.evaluate(async () => {
    const bootstrapResponse = await fetch('/api/v1/local/bootstrap');
    if (!bootstrapResponse.ok) throw new Error(`bootstrap status ${bootstrapResponse.status}`);
    const bootstrap = await bootstrapResponse.json();
    const session = bootstrap?.data?.sessionNonce;
    if (typeof session !== 'string') throw new Error('missing bootstrap session nonce');
    const url = new URL('/api/v1/watch', window.location.href);
    url.protocol = 'ws:';
    url.searchParams.set('session', session);
    return await new Promise((resolve, reject) => {
      const socket = new WebSocket(url, ['bcsp.v1']);
      const timeout = setTimeout(() => reject(new Error('WebSocket open timeout')), 5_000);
      socket.addEventListener('open', () => {
        clearTimeout(timeout);
        const protocol = socket.protocol;
        socket.close(1000, 'embedded runtime smoke complete');
        resolve(protocol);
      }, { once: true });
      socket.addEventListener('error', () => {
        clearTimeout(timeout);
        reject(new Error('WebSocket failed'));
      }, { once: true });
    });
  });
  assert.equal(socketProtocol, 'bcsp.v1');

  await page.goto(`${origin}/settings`, { waitUntil: 'domcontentloaded' });
  await page.locator('[data-bcsp-shared-application]').waitFor({ state: 'visible' });
  await page.waitForFunction(() => (
    document.querySelector('[data-bcsp-shared-application]')
      ?.getAttribute('data-bcsp-product-state') !== 'LOADING'
  ));
  assert.equal(new URL(page.url()).pathname, '/settings');
  const scriptPaths = await page.locator('script[src]').evaluateAll((scripts) => (
    scripts.map((script) => new URL(script.src).pathname)
  ));
  assert.ok(scriptPaths.length > 0, 'the embedded module script must be loaded');
  assert.ok(scriptPaths.every((path) => path.startsWith('/assets/')),
    `deep-link scripts must resolve from /assets, got ${scriptPaths.join(', ')}`);

  for (const requiredPath of [
    '/api/v1/catalog/discovery',
    '/api/v1/local/bootstrap',
    '/api/v1/query/filter-schema',
  ]) {
    assert.equal(observedResponses.get(requiredPath), 200, `${requiredPath} must be served by Rust`);
  }
  assert.ok(
    [...observedResponses].some(([path, status]) => path.startsWith('/assets/') && status === 200),
    'at least one embedded JavaScript asset must be served by Rust',
  );
  assert.deepEqual(nonSuccessfulResponses, []);
  assert.deepEqual(failedRequests, []);
  assert.deepEqual(browserErrors, []);

  process.stdout.write('embedded local browser runtime: PASS\n');
} finally {
  await browser.close();
}
