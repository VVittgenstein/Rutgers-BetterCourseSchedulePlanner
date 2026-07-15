#!/usr/bin/env node

import { createRequire } from 'node:module';
import http from 'node:http';
import { resolve } from 'node:path';

class GateError extends Error {
  constructor(code) {
    super(code);
    this.code = code;
  }
}

function fail(code) {
  throw new GateError(code);
}

function requireGate(value, code) {
  if (!value) fail(code);
}

async function step(code, action) {
  try {
    return await action();
  } catch (error) {
    const detail = error instanceof Error ? error.message : String(error);
    process.stderr.write(`${code}: ${detail}\n`);
    fail(code);
  }
}

function usage() {
  process.stderr.write(
    'usage: real-world-browser.mjs --base-url URL --metrics-url URL --playwright-root PATH\n',
  );
  process.exit(2);
}

function parseArguments(argv) {
  const options = { baseUrl: '', metricsUrl: '', playwrightRoot: '' };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const value = argv[index + 1];
    if (value === undefined) usage();
    if (argument === '--base-url') options.baseUrl = value;
    else if (argument === '--metrics-url') options.metricsUrl = value;
    else if (argument === '--playwright-root') options.playwrightRoot = value;
    else usage();
    index += 1;
  }
  if (Object.values(options).some((value) => value.length === 0)) usage();
  return options;
}

const options = parseArguments(process.argv.slice(2));
const parsedBaseUrl = new URL(options.baseUrl);
const parsedMetricsUrl = new URL(options.metricsUrl);
requireGate(parsedBaseUrl.protocol === 'https:', 'E2E_PUBLIC_HTTPS_REQUIRED');
requireGate(parsedBaseUrl.pathname === '/' && parsedBaseUrl.search === ''
  && parsedBaseUrl.hash === '' && parsedBaseUrl.username === ''
  && parsedBaseUrl.password === '', 'E2E_BASE_URL_INVALID');
requireGate(parsedMetricsUrl.protocol === 'http:' && parsedMetricsUrl.hostname === '127.0.0.1'
  && parsedMetricsUrl.pathname === '/metrics' && parsedMetricsUrl.search === ''
  && parsedMetricsUrl.hash === '', 'E2E_METRICS_URL_INVALID');
const origin = parsedBaseUrl.origin;

const requireFromFrontend = createRequire(resolve(options.playwrightRoot, 'package.json'));
const { chromium, devices } = requireFromFrontend('playwright');

function metricsSnapshot() {
  return new Promise((resolveMetrics, rejectMetrics) => {
    const request = http.get({
      hostname: parsedMetricsUrl.hostname,
      path: parsedMetricsUrl.pathname,
      port: parsedMetricsUrl.port || 80,
      headers: { Host: parsedBaseUrl.host },
      timeout: 5_000,
    }, (response) => {
      let body = '';
      response.setEncoding('utf8');
      response.on('data', (chunk) => {
        body += chunk;
        if (body.length > 65_536) request.destroy();
      });
      response.on('end', () => {
        if (response.statusCode !== 200 || body.length > 65_536) {
          rejectMetrics(new Error('metrics response rejected'));
          return;
        }
        const metrics = new Map();
        for (const line of body.split('\n')) {
          const match = /^([a-z0-9_]+) ([0-9]+)$/u.exec(line);
          if (match !== null) metrics.set(match[1], Number(match[2]));
        }
        resolveMetrics(metrics);
      });
    });
    request.on('timeout', () => request.destroy());
    request.on('error', rejectMetrics);
  });
}

async function waitForReady(page) {
  await page.locator(
    '[data-bcsp-shared-application][data-bcsp-product-state="READY"]',
  ).waitFor({ state: 'visible', timeout: 120_000 });
}

async function publicSession(page) {
  return await page.locator('#bcsp-bootstrap').evaluate((element) => {
    const envelope = JSON.parse(element.textContent ?? 'null');
    const nonce = envelope?.data?.sessionNonce;
    if (typeof nonce !== 'string') throw new Error('missing session');
    return nonce;
  });
}

async function assertTls(response) {
  requireGate(response !== null && response.ok(), 'E2E_DOCUMENT_HTTP');
  const security = await response.securityDetails();
  requireGate(security !== null && /TLS/iu.test(security.protocol ?? ''), 'E2E_TLS_MISSING');
}

async function assertPublicBoundary(page) {
  const boundary = await page.evaluate(async () => {
    const statuses = await Promise.all(['/saved-views', '/history', '/settings'].map(async (path) => {
      const response = await fetch(path, { cache: 'no-store', redirect: 'manual' });
      return response.status;
    }));
    return {
      localDom: document.querySelectorAll(
        '.local-personal, [data-bcsp-local-page], .local-page__reset-scope',
      ).length,
      localLinks: document.querySelectorAll(
        'a[href="/saved-views"], a[href="/history"], a[href="/settings"]',
      ).length,
      statuses,
    };
  });
  requireGate(boundary.localDom === 0 && boundary.localLinks === 0,
    'E2E_PUBLIC_LOCAL_SURFACE');
  requireGate(boundary.statuses.length === 3
    && boundary.statuses.every((status) => status === 404), 'E2E_PUBLIC_LOCAL_ROUTE');
}

async function attachNetworkGuard(context, page) {
  const state = { offOrigin: 0, watchSockets: 0 };
  await context.route('**/*', async (route) => {
    try {
      const url = new URL(route.request().url());
      if ((url.protocol === 'http:' || url.protocol === 'https:') && url.origin !== origin) {
        await route.abort('blockedbyclient');
        return;
      }
    } catch {
      await route.abort('blockedbyclient');
      return;
    }
    await route.continue();
  });
  context.on('request', (request) => {
    try {
      const url = new URL(request.url());
      if ((url.protocol === 'http:' || url.protocol === 'https:') && url.origin !== origin) {
        state.offOrigin += 1;
      }
    } catch {
      state.offOrigin += 1;
    }
  });
  page.on('websocket', (socket) => {
    try {
      const url = new URL(socket.url());
      if (url.protocol === 'wss:' && url.host === parsedBaseUrl.host
        && url.pathname === '/api/v1/watch') {
        state.watchSockets += 1;
      } else {
        state.offOrigin += 1;
      }
    } catch {
      state.offOrigin += 1;
    }
  });
  return state;
}

async function openSectionsWorkspace(page) {
  await page.locator('.bcsp-navigation a[href="/sections"]').click();
  await page.waitForURL(`${origin}/sections`);
  await waitForReady(page);
  await page.locator('.filter-panel [data-filter-row="FLT-C01"] select').waitFor({
    state: 'visible', timeout: 120_000,
  });
}

async function selectCurrentOpenSection(page) {
  const term = page.locator('[data-filter-row="FLT-C01"] select');
  const campuses = page.locator('[data-filter-row="FLT-C02"] input[type="checkbox"]:checked');
  requireGate((await term.inputValue()).length > 0, 'E2E_TERM_NOT_SELECTED');
  requireGate(await campuses.count() > 0, 'E2E_CAMPUS_NOT_SELECTED');

  const openRow = page.locator('[data-filter-row="FLT-S03"]');
  const sectionGroup = page.locator('details.filter-panel__group').filter({ has: openRow });
  if (await sectionGroup.getAttribute('open') === null) {
    await sectionGroup.locator(':scope > summary').click();
  }
  await openRow.locator('input[value="OPEN"]').check();

  for (const stableId of ['FLT-C01', 'FLT-C02', 'FLT-S03']) {
    requireGate(await page.locator(`[data-filter-chip="${stableId}"]`).count() === 1,
      'E2E_FILTER_CHIP_MISSING');
  }

  const responsePromise = page.waitForResponse((response) => {
    try {
      return new URL(response.url()).pathname === '/api/v1/query/sections'
        && response.request().method() === 'POST';
    } catch {
      return false;
    }
  }, { timeout: 180_000 });
  await page.locator('.filter-panel button[type="submit"]').click();
  const response = await responsePromise;
  requireGate(response.ok(), 'E2E_SECTION_SEARCH_HTTP');
  const responseEnvelope = await response.json();
  await page.waitForFunction(() => {
    const state = document.querySelector('.bcsp-search-workspace__state')
      ?.getAttribute('data-query-state');
    return state !== 'loading' && state !== 'idle';
  }, undefined, { timeout: 180_000 });
  requireGate(await page.locator(
    '.bcsp-search-workspace__state[data-query-state="sections"]',
  ).count() === 1, 'E2E_SECTION_SEARCH_FAILED');

  const standalone = page.locator('.search-results__standalone-section').filter({
    has: page.locator('.search-results__badge--open'),
  }).first();
  await standalone.waitFor({ state: 'visible', timeout: 30_000 });
  const card = standalone.locator('.search-results__section').filter({
    has: page.locator('.search-results__badge--open'),
  }).first();
  const index = await card.getAttribute('data-section-index');
  const termValue = await card.locator('data.search-results__meta').nth(0).getAttribute('value');
  const campusValue = await card.locator('data.search-results__meta').nth(1).getAttribute('value');
  requireGate(index !== null && /^\d{5}$/u.test(index)
    && termValue !== null && termValue.length > 0
    && campusValue !== null && campusValue.length > 0, 'E2E_SECTION_IDENTITY_INVALID');
  const responseItem = responseEnvelope?.data?.items?.find((item) => (
    item?.section?.section?.key?.index === index
      && item.section.section.key.term === termValue
      && item.section.section.key.campus === campusValue
  ));
  requireGate(responseItem?.section?.open?.state === 'OPEN'
    && responseItem.section.open.uncertainty === null
    && typeof responseItem.section.open.freshUntil === 'string'
    && Date.parse(responseItem.section.open.freshUntil) > Date.now(), 'E2E_OPEN_NOT_CURRENT');
  requireGate(await card.locator('.search-results__live .search-results__fact').count() === 3,
    'E2E_OPEN_FRESHNESS_MISSING');

  await standalone.locator(
    ':scope > .search-results__group-header .search-results__button',
  ).click();
  await page.locator('.bcsp-search-workspace__state .search-results__detail').waitFor({
    state: 'visible', timeout: 30_000,
  });
  await page.locator('.bcsp-search-workspace__detail-actions button').click();
  await standalone.waitFor({ state: 'visible', timeout: 30_000 });

  const restoredCard = standalone.locator('.search-results__section').filter({
    has: page.locator('.search-results__badge--open'),
  }).first();
  const selection = restoredCard.locator('.watch-selection-action');
  await selection.click();
  requireGate(await selection.getAttribute('aria-pressed') === 'true',
    'E2E_WATCH_SELECTION_FAILED');
  await restoredCard.locator('.search-results__section-link').click();
  await page.locator(
    '.bcsp-search-workspace[data-detail-route="true"] .search-results__detail',
  ).waitFor({ state: 'visible', timeout: 30_000 });

  return `${termValue}\u0000${campusValue}`;
}

async function startWatch(page) {
  await page.locator('.bcsp-navigation a[href="/watch"]').click();
  await page.waitForURL(`${origin}/watch`);
  await page.locator('.watch-workspace').waitFor({ state: 'visible', timeout: 30_000 });

  await page.getByRole('button', { exact: true, name: 'Enable / test sound' }).click();
  await page.waitForFunction(() => {
    const status = document.querySelector('.watch-workspace__inline-status[role="status"]')
      ?.textContent ?? '';
    return status.includes('Audio Ready') && status.includes('audible');
  }, undefined, { timeout: 15_000 });

  await page.getByRole('button', { name: /^Start selected/u }).click();
  await page.locator('.watch-workspace[data-watch-connection="OPEN"]').waitFor({
    state: 'visible', timeout: 30_000,
  });
  const item = page.locator('.watch-workspace__item').first();
  await item.locator('.watch-workspace__badge[data-state="READY"]').waitFor({
    state: 'visible', timeout: 30_000,
  });
  await item.locator('.watch-workspace__badge[data-state="OPEN"]').waitFor({
    state: 'visible', timeout: 30_000,
  });
  await page.waitForFunction(() => (
    [...document.querySelectorAll('.watch-toast[data-tone="ALERT"] .watch-toast__title')]
      .some((title) => title.textContent?.trim() === 'A watched Section is open')
  ), undefined, { timeout: 30_000 });
  await page.locator('.watch-workspace__alert').first().waitFor({
    state: 'visible', timeout: 30_000,
  });
}

async function assertTelemetry(page) {
  const batch = page.locator('.watch-telemetry__batch[data-freshness]').first();
  const section = page.locator(
    '.watch-telemetry__section[data-freshness="FRESH"] '
      + '.watch-workspace__badge[data-state="OPEN"]',
  ).first();
  await batch.waitFor({ state: 'visible', timeout: 30_000 });
  await section.waitFor({ state: 'visible', timeout: 30_000 });
  const facts = await batch.locator('.watch-telemetry__fact').evaluateAll((elements) => (
    elements.map((element) => ({
      label: element.querySelector('dt')?.textContent?.trim() ?? '',
      valuePresent: (element.querySelector('dd')?.textContent?.trim().length ?? 0) > 0,
    }))
  ));
  const required = [
    'Last attempt', 'Last valid', 'Requested general', 'Requested effective',
    'Actual interval', 'Scheduler lag', 'Run counters',
  ];
  requireGate(required.every((label) => facts.some((fact) => (
    fact.label === label && fact.valuePresent
  ))), 'E2E_TELEMETRY_INCOMPLETE');
}

async function stopAndReload(page, firstSession) {
  const item = page.locator('.watch-workspace__item').first();
  await item.locator('.watch-workspace__actions .bcsp-action--accent').click();
  await item.locator('.watch-workspace__badge[data-state="READY"]').waitFor({
    state: 'detached', timeout: 30_000,
  });
  await page.getByRole('button', { exact: true, name: 'Disconnect' }).click();
  await page.locator('.watch-workspace[data-watch-connection="CLOSED"]').waitFor({
    state: 'visible', timeout: 15_000,
  });
  const response = await page.reload({ waitUntil: 'domcontentloaded' });
  await assertTls(response);
  await waitForReady(page);
  requireGate(new URL(page.url()).pathname === '/watch', 'E2E_RELOAD_ROUTE_CHANGED');
  requireGate(await publicSession(page) !== firstSession, 'E2E_SESSION_REUSED');
  requireGate(await page.locator('.watch-workspace__item').count() === 0,
    'E2E_PUBLIC_SESSION_PERSISTED');
  await assertPublicBoundary(page);
}

const browser = await chromium.launch({
  args: ['--no-proxy-server'],
  headless: true,
});

const scenarioDefinitions = [
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

const scenarios = [];
let failureCode = null;
try {
  for (const definition of scenarioDefinitions) {
    const context = await browser.newContext(definition.context);
    const page = await context.newPage();
    scenarios.push({
      context,
      firstSession: '',
      identity: '',
      name: definition.name,
      network: await attachNetworkGuard(context, page),
      page,
    });
  }

  for (const scenario of scenarios) {
    await step(`E2E_${scenario.name.toUpperCase()}_ROOT`, async () => {
      const response = await scenario.page.goto(`${origin}/`, { waitUntil: 'domcontentloaded' });
      await assertTls(response);
      await waitForReady(scenario.page);
      await assertPublicBoundary(scenario.page);
      scenario.firstSession = await publicSession(scenario.page);
    });
  }

  for (const scenario of scenarios) {
    await step(`E2E_${scenario.name.toUpperCase()}_SEARCH_DETAIL`, async () => {
      await openSectionsWorkspace(scenario.page);
      scenario.identity = await selectCurrentOpenSection(scenario.page);
    });
  }
  requireGate(scenarios[0].identity === scenarios[1].identity, 'E2E_BROWSER_TARGET_DIVERGED');

  await new Promise((resolveDelay) => setTimeout(resolveDelay, 32_000));
  for (const scenario of scenarios) {
    await step(`E2E_${scenario.name.toUpperCase()}_WATCH`, async () => {
      await startWatch(scenario.page);
    });
  }

  const metrics = await step('E2E_METRICS_UNAVAILABLE', metricsSnapshot);
  requireGate(metrics.get('bcsp_websocket_connections') === 2, 'E2E_WEBSOCKET_COUNT');
  requireGate(metrics.get('bcsp_active_watches') === 2, 'E2E_ACTIVE_WATCH_COUNT');
  requireGate(metrics.get('bcsp_open_general_requested_interval_seconds') === 30,
    'E2E_GENERAL_REFRESH_CADENCE');
  requireGate(metrics.get('bcsp_open_watched_requested_interval_seconds') === 10,
    'E2E_WATCH_REFRESH_CADENCE');

  await new Promise((resolveDelay) => setTimeout(resolveDelay, 22_000));
  for (const scenario of scenarios) {
    await step(`E2E_${scenario.name.toUpperCase()}_TELEMETRY`, async () => {
      await assertTelemetry(scenario.page);
    });
    requireGate(scenario.network.offOrigin === 0,
      `E2E_${scenario.name.toUpperCase()}_OFF_ORIGIN_REQUEST`);
    requireGate(scenario.network.watchSockets >= 1,
      `E2E_${scenario.name.toUpperCase()}_WEBSOCKET_MISSING`);
  }

  for (const scenario of scenarios) {
    await step(`E2E_${scenario.name.toUpperCase()}_RELOAD`, async () => {
      await stopAndReload(scenario.page, scenario.firstSession);
    });
  }
} catch (error) {
  failureCode = error instanceof GateError ? error.code : 'E2E_UNEXPECTED_FAILURE';
} finally {
  for (const scenario of scenarios) {
    await scenario.context.close().catch(() => undefined);
  }
  await browser.close().catch(() => undefined);
}

if (failureCode !== null) {
  process.stderr.write(`${failureCode}\n`);
  process.exit(1);
}
process.stdout.write(
  'P7_5_LINUX_REAL_WORLD_BROWSER_PASS scenarios=2 tls=1 same_origin=1 websocket=2 audio_preview_ready=2 cadence_config=30/10\n',
);
