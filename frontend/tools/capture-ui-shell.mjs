import { mkdir, readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import { chromium } from 'playwright';
import { localBootstrapFixture } from './local-bootstrap-fixture.mjs';

const baseUrl = process.env.BCSP_VISUAL_BASE_URL ?? 'http://127.0.0.1:4173';
const executablePath = process.env.BCSP_BROWSER_EXECUTABLE
  ?? (process.platform === 'win32'
    ? 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe'
    : undefined);
const outputDirectory = resolve(
  process.cwd(),
  process.env.BCSP_VISUAL_OUTPUT ?? 'test-results/ui-shell',
);
const filterSchema = JSON.parse(await readFile(
  resolve(process.cwd(), '../crates/bcsp-contracts/tests/golden/filter-schema-v1.json'),
  'utf8',
));
const observedAt = '2026-07-14T16:30:00.000Z';

const point = {
  contentVersion: 7,
  observationId: '10000000-0000-4000-8000-000000000007',
  observedAt,
};
const provenance = {
  observationId: point.observationId,
  observedAt,
  payloadDigest: 'a'.repeat(64),
  sourceId: 'rutgers-selector',
  sourceKind: 'SELECTOR',
};
const known = (value) => ({ knowledge: 'KNOWN', presence: { presence: 'PRESENT', value } });

function discovery(availability, targetCount = 5) {
  const campus = [
    ['NB', 'New Brunswick'],
    ['NK', 'Newark'],
    ['CM', 'Camden'],
    ['OL', 'Online'],
    ['BH', 'Busch'],
  ];
  const targets = campus.slice(0, targetCount).map(([code, label]) => ({
    campusLabel: known(label),
    key: { campus: code, term: '2026-9' },
    provenance,
    termLabel: known('Fall 2026'),
  }));
  return {
    contractVersion: 1,
    coreCodeDictionaries: targets.map(({ key }) => ({
      contentVersion: point.contentVersion,
      options: [
        { code: 'CCO', description: known('Contemporary Challenges') },
        { code: 'QQ', description: known('Quantitative and Formal Reasoning') },
      ],
      provenance,
      target: key,
    })),
    observedAt,
    sources: [],
    status: {
      availability,
      error: availability === 'UNAVAILABLE_NO_FIRST_SUCCESS'
        ? { class: 'TRANSPORT', code: 'DISCOVERY_UNAVAILABLE' }
        : null,
      isStale: availability !== 'CURRENT',
      lastSuccess: availability === 'UNAVAILABLE_NO_FIRST_SUCCESS' ? null : point,
      latestAttempt: point,
    },
    subjects: [],
    targets,
  };
}

function serviceStatus(level, phase, availability, targetCount = 5) {
  const discoveryStatus = discovery(availability, targetCount).status;
  const catalogAvailable = level === 'INITIALIZING' || level === 'ERROR' ? 0 : targetCount;
  const openAvailable = level === 'READY' || level === 'DEGRADED' ? targetCount : 0;
  const stale = level === 'DEGRADED';
  const targets = discovery(availability, targetCount).targets.map(({ key }, index) => ({
    catalogAvailability: catalogAvailable === 0 ? 'UNAVAILABLE' : stale ? 'STALE' : 'CURRENT',
    catalogContentVersion: catalogAvailable === 0 ? null : point.contentVersion,
    openAvailability: openAvailable === 0 ? 'UNAVAILABLE' : stale ? 'STALE' : 'CURRENT',
    primary: index === 0,
    searchAvailable: catalogAvailable > 0,
    target: key,
  }));
  const issues = level === 'DEGRADED' || level === 'ERROR'
    ? [{
      code: level === 'DEGRADED' ? 'OPEN_LKG_STALE' : 'DISCOVERY_UNAVAILABLE',
      component: level === 'DEGRADED' ? 'OPEN' : 'DISCOVERY',
      recovery: 'AUTOMATIC_RETRY',
      retryAt: '2026-07-14T16:31:00.000Z',
      severity: level === 'DEGRADED' ? 'DEGRADED' : 'BLOCKING',
      target: level === 'DEGRADED' ? targets[0]?.target ?? null : null,
    }]
    : [];
  return {
    catalog: {
      availableTargetCount: catalogAvailable,
      currentTargetCount: stale ? 0 : catalogAvailable,
      staleTargetCount: stale ? catalogAvailable : 0,
      totalTargetCount: targetCount,
      unavailableTargetCount: targetCount - catalogAvailable,
    },
    contractVersion: 1,
    discovery: discoveryStatus,
    issues,
    level,
    observedAt,
    open: {
      availableTargetCount: openAvailable,
      currentTargetCount: stale ? 0 : openAvailable,
      staleTargetCount: stale ? openAvailable : 0,
      totalTargetCount: targetCount,
      unavailableTargetCount: targetCount - openAvailable,
    },
    operation: {
      nextRetryAt: phase === 'RETRY_WAIT' ? '2026-07-14T16:31:00.000Z' : null,
      phase,
      startedAt: observedAt,
      target: ['CATALOG_FETCH', 'CATALOG_PROCESS', 'CATALOG_PUBLISH', 'OPEN_FETCH'].includes(phase)
        ? targets[0]?.target ?? null
        : null,
    },
    runtime: 'LOCAL',
    targets,
  };
}

const success = (data) => ({ protocolVersion: 1, data });
const scenarios = [
  { name: 'ready-ultrawide', availability: 'CURRENT', level: 'READY', phase: 'IDLE', viewport: { width: 2560, height: 1280 } },
  { name: 'ready-fullhd-zh', availability: 'CURRENT', language: 'zh-CN', level: 'READY', phase: 'IDLE', viewport: { width: 1920, height: 1080 } },
  { name: 'partial-desktop', availability: 'CURRENT', level: 'PARTIALLY_READY', phase: 'OPEN_FETCH', viewport: { width: 1440, height: 900 } },
  { name: 'degraded-tablet', availability: 'STALE_LAST_SUCCESS', level: 'DEGRADED', phase: 'RETRY_WAIT', viewport: { width: 1024, height: 900 } },
  { name: 'ready-mobile', availability: 'CURRENT', level: 'READY', phase: 'IDLE', viewport: { width: 390, height: 844 } },
  { name: 'initializing-mobile', availability: 'UNAVAILABLE_NO_FIRST_SUCCESS', level: 'INITIALIZING', loading: true, phase: 'DISCOVERING', viewport: { width: 390, height: 844 } },
  { name: 'error-desktop', availability: 'UNAVAILABLE_NO_FIRST_SUCCESS', error: true, level: 'ERROR', phase: 'RETRY_WAIT', viewport: { width: 1440, height: 900 } },
];

await mkdir(outputDirectory, { recursive: true });
const browser = await chromium.launch({
  ...(executablePath === undefined ? {} : { executablePath }),
  headless: true,
});

try {
  for (const scenario of scenarios) {
    const page = await browser.newPage({
      colorScheme: 'light',
      locale: 'en-US',
      reducedMotion: 'reduce',
      viewport: scenario.viewport,
    });
    await page.route('**/api/v1/local/bootstrap', async (route) => {
      await route.fulfill({
        body: JSON.stringify(success(localBootstrapFixture())),
        contentType: 'application/json',
      });
    });
    await page.route('**/api/v1/query/filter-schema', async (route) => {
      if (scenario.loading) await new Promise((resolveDelay) => setTimeout(resolveDelay, 2_000));
      await route.fulfill({
        body: JSON.stringify(success(filterSchema)),
        contentType: 'application/json',
      }).catch(() => undefined);
    });
    await page.route('**/api/v1/query/filter-options', async (route) => {
      const request = route.request().postDataJSON()?.payload ?? {};
      const options = request.field === 'COURSE_LEVEL'
        ? [{ value: 'U', label: 'Undergraduate' }, { value: 'G', label: 'Graduate' }]
        : request.field === 'MEETING_LOCATION'
          ? [{ value: 'NB', label: 'New Brunswick' }, { value: 'NK', label: 'Newark' }]
          : [];
      await route.fulfill({
        body: JSON.stringify(success({
          contractVersion: 3,
          field: request.field,
          options,
          targetVersions: [],
          truncated: false,
        })),
        contentType: 'application/json',
      });
    });
    await page.route('**/api/v1/service/status', async (route) => {
      await route.fulfill({
        body: JSON.stringify(success(serviceStatus(
          scenario.level,
          scenario.phase,
          scenario.availability,
          scenario.targetCount,
        ))),
        contentType: 'application/json',
      }).catch(() => undefined);
    });
    await page.route('**/api/v1/catalog/discovery', async (route) => {
      if (scenario.loading) await new Promise((resolveDelay) => setTimeout(resolveDelay, 2_000));
      if (scenario.error) {
        await route.fulfill({
          body: JSON.stringify({
            protocolVersion: 1,
            error: {
              code: 'UPSTREAM_UNAVAILABLE',
              details: [],
              messageKey: 'error.upstream_unavailable',
              traceId: '10000000-0000-4000-8000-000000000099',
            },
          }),
          contentType: 'application/json',
          status: 503,
        });
        return;
      }
      await route.fulfill({
        body: JSON.stringify(success(discovery(scenario.availability, scenario.targetCount))),
        contentType: 'application/json',
      }).catch(() => undefined);
    });

    await page.goto(`${baseUrl}/local.html`, { waitUntil: 'domcontentloaded' });
    const expected = scenario.loading
      ? 'Opening the catalog console'
      : scenario.error
        ? 'Catalog index unavailable'
        : scenario.targetCount === 0
          ? 'No catalog targets available'
          : 'Build a precise search';
    try {
      await page.getByRole('heading', { name: expected }).first().waitFor();
    } catch (error) {
      const rendered = await page.locator('body').innerText().catch(() => '<body unavailable>');
      throw new Error(`${scenario.name}: expected ${expected}\n${rendered}`, { cause: error });
    }
    if (scenario.language === 'zh-CN') {
      await page.locator('.bcsp-language__button').nth(1).click();
      await page.getByRole('heading', { name: '搜索课程', exact: true }).first().waitFor();
    }
    const measurements = await page.evaluate(() => {
      const rectangle = (selector) => {
        const element = document.querySelector(selector);
        if (element === null) return null;
        const bounds = element.getBoundingClientRect();
        return { left: bounds.left, right: bounds.right, width: bounds.width };
      };
      return {
      clientWidth: document.documentElement.clientWidth,
      filter: rectangle('.bcsp-search-workspace__filters'),
      railCount: document.querySelectorAll('.bcsp-rail').length,
      results: rectangle('.bcsp-search-workspace__results'),
      search: rectangle('.bcsp-search-workspace'),
      scrollWidth: document.documentElement.scrollWidth,
      };
    });
    if (measurements.scrollWidth > measurements.clientWidth) {
      throw new Error(`${scenario.name}: horizontal overflow ${measurements.scrollWidth}/${measurements.clientWidth}`);
    }
    if (measurements.railCount !== 0) {
      throw new Error(`${scenario.name}: legacy full-height rail is still mounted`);
    }
    if (!scenario.loading && !scenario.error && (scenario.targetCount ?? 5) > 0) {
      if (measurements.search === null || measurements.results === null || measurements.filter === null) {
        throw new Error(`${scenario.name}: search workspace measurements unavailable`);
      }
      if (scenario.viewport.width >= 1920 && measurements.search.width < scenario.viewport.width * 0.9) {
        throw new Error(`${scenario.name}: search workspace uses only ${measurements.search.width}/${scenario.viewport.width}px`);
      }
      if (scenario.viewport.width >= 1920 && measurements.results.width < scenario.viewport.width * 0.6) {
        throw new Error(`${scenario.name}: result column uses only ${measurements.results.width}/${scenario.viewport.width}px`);
      }
      if (scenario.viewport.width <= 390 && Math.abs(measurements.search.width - measurements.results.width) > 2) {
        throw new Error(`${scenario.name}: narrow layout did not stack to one column`);
      }
    }
    await page.screenshot({
      animations: 'disabled',
      fullPage: true,
      path: resolve(outputDirectory, `${scenario.name}.png`),
    });
    await page.close();
  }
} finally {
  await browser.close();
}

process.stdout.write(`UI shell snapshots: PASS (${scenarios.length}/${scenarios.length})\n`);
