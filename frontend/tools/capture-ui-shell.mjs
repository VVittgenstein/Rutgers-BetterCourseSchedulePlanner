import { mkdir } from 'node:fs/promises';
import { resolve } from 'node:path';

import { chromium } from 'playwright';

const baseUrl = process.env.BCSP_VISUAL_BASE_URL ?? 'http://127.0.0.1:4173';
const executablePath = process.env.BCSP_BROWSER_EXECUTABLE
  ?? (process.platform === 'win32'
    ? 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe'
    : undefined);
const outputDirectory = resolve(
  process.cwd(),
  '../project-governance/current/p7/evidence/p7-2-001',
);
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
  return {
    contractVersion: 1,
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
    targets: campus.slice(0, targetCount).map(([code, label]) => ({
      campusLabel: known(label),
      key: { campus: code, term: '2026-9' },
      provenance,
      termLabel: known('Fall 2026'),
    })),
  };
}

const success = (data) => ({ protocolVersion: 1, data });
const scenarios = [
  { name: 'ready-desktop', availability: 'CURRENT', viewport: { width: 1440, height: 1000 } },
  { name: 'stale-mobile', availability: 'STALE_LAST_SUCCESS', viewport: { width: 390, height: 844 } },
  { name: 'empty-mobile', availability: 'CURRENT', targetCount: 0, viewport: { width: 390, height: 844 } },
  { name: 'error-desktop', error: true, viewport: { width: 1024, height: 768 } },
  { name: 'loading-desktop', loading: true, viewport: { width: 1024, height: 768 } },
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
        body: JSON.stringify(success({ sessionNonce: '10000000-0000-4000-8000-000000000001' })),
        contentType: 'application/json',
      });
    });
    await page.route('**/api/v1/query/filter-schema', async (route) => {
      if (scenario.loading) await new Promise((resolveDelay) => setTimeout(resolveDelay, 2_000));
      await route.fulfill({
        body: JSON.stringify(success({ contractVersion: 1, fields: Array.from({ length: 22 }, () => ({})) })),
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
          : 'Course search is ready';
    try {
      await page.getByRole('heading', { name: expected }).waitFor();
    } catch (error) {
      const rendered = await page.locator('body').innerText().catch(() => '<body unavailable>');
      throw new Error(`${scenario.name}: expected ${expected}\n${rendered}`, { cause: error });
    }
    if (scenario.name === 'ready-desktop') {
      await page.getByRole('button', { name: /Fall 2026 Newark \/ NK/i }).click();
    }
    const viewport = await page.evaluate(() => ({
      clientWidth: document.documentElement.clientWidth,
      scrollWidth: document.documentElement.scrollWidth,
    }));
    if (viewport.scrollWidth > viewport.clientWidth) {
      throw new Error(`${scenario.name}: horizontal overflow ${viewport.scrollWidth}/${viewport.clientWidth}`);
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

process.stdout.write(`P7.2-001 shell snapshots: PASS (${scenarios.length}/${scenarios.length})\n`);
