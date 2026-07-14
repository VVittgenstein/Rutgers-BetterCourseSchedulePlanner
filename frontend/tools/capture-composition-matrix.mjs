import { mkdir, readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import { chromium } from 'playwright';
import { localBootstrapFixture } from './local-bootstrap-fixture.mjs';

const sharedBaseUrl = process.env.BCSP_VISUAL_BASE_URL;
const localBaseUrl = process.env.BCSP_LOCAL_BASE_URL ?? sharedBaseUrl ?? 'http://127.0.0.1:4173';
const publicBaseUrl = process.env.BCSP_PUBLIC_BASE_URL ?? sharedBaseUrl ?? 'http://127.0.0.1:4174';
const executablePath = process.env.BCSP_BROWSER_EXECUTABLE
  ?? (process.platform === 'win32'
    ? 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe'
    : undefined);
const evidenceTask = process.env.BCSP_EVIDENCE_TASK ?? 'p7-2-004';
const outputDirectory = resolve(
  process.cwd(),
  `../project-governance/current/p7/evidence/${evidenceTask}`,
);
const [filterSchema, axeSource] = await Promise.all([
  readFile(resolve(process.cwd(), '../crates/bcsp-contracts/tests/golden/filter-schema-v1.json'), 'utf8')
    .then((source) => JSON.parse(source)),
  readFile(resolve(process.cwd(), 'node_modules/axe-core/axe.min.js'), 'utf8'),
]);

const observedAt = '2030-02-01T15:00:00.000Z';
const success = (data) => ({ protocolVersion: 1, data });
const known = (value) => ({ knowledge: 'KNOWN', presence: { presence: 'PRESENT', value } });
const point = {
  contentVersion: 12,
  observationId: '10000000-0000-4000-8000-000000000012',
  observedAt,
};
const provenance = {
  observationId: point.observationId,
  observedAt,
  payloadDigest: 'a'.repeat(64),
  sourceId: 'composition-matrix',
  sourceKind: 'SELECTOR',
};
const discovery = {
  contractVersion: 1,
  observedAt,
  sources: [],
  status: {
    availability: 'CURRENT',
    error: null,
    isStale: false,
    lastSuccess: point,
    latestAttempt: point,
  },
  subjects: [{
    code: '198',
    label: known('Computer Science'),
    provenance: { kind: 'DISCOVERY', discovery: provenance },
    target: { campus: 'NB', term: '2026-9' },
  }],
  targets: [{
    campusLabel: known('New Brunswick'),
    key: { campus: 'NB', term: '2026-9' },
    provenance,
    termLabel: known('Fall 2026'),
  }],
};

const neutralFilters = {
  term: '2026-9',
  campuses: [],
  subjects: [],
  text: 'computer science',
  courseNumbers: [],
  levels: [],
  credits: null,
  core: { codes: [], mode: 'ANY' },
  prerequisite: 'ANY',
  courseLocations: [],
  sectionIndexes: [],
  sectionNumbers: [],
  openStatuses: [],
  modalities: [],
  synchronicities: [],
  instructors: [],
  availability: [],
  meetingLocations: [],
  buildingRoom: { buildingCodes: [], roomNumbers: [] },
  examCodes: [],
  permission: 'ANY',
  eligibility: {
    majorCodes: [],
    minorCodes: [],
    honorProgramCodes: [],
    unitCodes: [],
    unitMajors: [],
  },
};
const filterRequest = { contractVersion: 1, values: neutralFilters };
const savedView = {
  id: '20000000-0000-4000-8000-000000000001',
  name: 'Morning CS options',
  schemaVersion: 1,
  revision: 3,
  content: { status: 'COMPATIBLE', filters: filterRequest },
  createdAt: Date.parse('2030-01-28T14:00:00.000Z'),
  updatedAt: Date.parse('2030-01-29T14:00:00.000Z'),
};
const currentFilters = {
  stateRevision: 7,
  revision: 4,
  value: {
    association: { kind: 'APPLIED', viewId: savedView.id, revision: savedView.revision },
    content: savedView.content,
  },
};
const episodeHistory = {
  items: [{
    identity: {
      sectionKey: { campus: 'NB', index: '00001', term: '2026-9' },
      runId: '30000000-0000-4000-8000-000000000001',
      episodeId: '40000000-0000-4000-8000-000000000001',
    },
    state: 'ACKNOWLEDGED',
    mode: 'ONE_SHOT',
    firstObservedAt: Date.parse('2030-01-30T14:00:00.000Z'),
    lastObservedAt: Date.parse('2030-01-30T14:01:00.000Z'),
    acknowledgedAt: Date.parse('2030-01-30T14:01:10.000Z'),
    timedOutAt: null,
    closedAt: null,
    disposition: { kind: 'ACKNOWLEDGED' },
    audibleCount: 1,
    observationCount: 2,
    lastObservationId: '50000000-0000-4000-8000-000000000001',
    actionCount: 1,
  }],
  total: 1,
  offset: 0,
  limit: 50,
};

function localState(locale) {
  const bootstrap = localBootstrapFixture({
    currentFilters,
    episodeHistory,
    savedViews: [savedView],
    settings: {
      revision: 2,
      value: {
        localeOverride: locale,
        catalogRefreshMinutes: 60,
        openRefreshSeconds: 30,
        volumePercent: 70,
        soundPolicy: {
          notificationMode: 'ONE_SHOT',
          maxAudible: 3,
          continuousDuration: { kind: 'FINITE', seconds: 600 },
        },
      },
    },
  });
  return {
    bootstrap,
    library: {
      stateRevision: bootstrap.state.stateRevision,
      currentFilters,
      views: [{ definition: savedView, matchState: 'CLEAN' }],
    },
  };
}

async function installApi(page, scenario) {
  const local = localState(scenario.locale);
  const routes = new Map([
    ['/api/v1/query/filter-schema', filterSchema],
    ['/api/v1/catalog/discovery', discovery],
    ['/api/v1/local/bootstrap', local.bootstrap],
    ['/api/v1/local/saved-views', local.library],
  ]);
  await page.route('**/api/v1/**', async (route) => {
    const path = new URL(route.request().url()).pathname;
    const response = routes.get(path);
    if (response === undefined || (scenario.target === 'public' && path.startsWith('/api/v1/local/'))) {
      await route.fulfill({
        body: JSON.stringify({
          protocolVersion: 1,
          error: {
            code: 'UNEXPECTED_MATRIX_ROUTE',
            details: [],
            messageKey: 'error.unexpected_matrix_route',
            traceId: '90000000-0000-4000-8000-000000000001',
          },
        }),
        contentType: 'application/json',
        status: 501,
      });
      return;
    }
    await route.fulfill({ body: JSON.stringify(success(response)), contentType: 'application/json' });
  });
}

async function installPublicDocuments(page) {
  let documentSequence = 0;
  await page.route('**/*', async (route) => {
    if (route.request().resourceType() !== 'document') {
      await route.fallback();
      return;
    }
    const source = await fetch(new URL('/public.html', publicBaseUrl));
    if (!source.ok) throw new Error(`public.html unavailable: HTTP ${source.status}`);
    documentSequence += 1;
    const sessionNonce = `10000000-0000-4000-8000-${String(documentSequence).padStart(12, '0')}`;
    const bootstrap = `<script id="bcsp-bootstrap" type="application/json">${JSON.stringify({
      protocolVersion: 1,
      data: { sessionNonce },
    })}</script>`;
    const html = (await source.text())
      .replace(/<script[^>]*id=(['"])bcsp-bootstrap\1[^>]*>[\s\S]*?<\/script>/giu, '')
      .replace('</head>', `${bootstrap}</head>`);
    await route.fulfill({ body: html, contentType: 'text/html', status: source.status });
  });
}

async function waitForComposition(page, scenario) {
  await page.locator(
    `.bcsp-shell[data-bcsp-product-state="READY"][data-bcsp-locale="${scenario.locale}"]`,
  ).waitFor();
}

async function assertAxe(page, name) {
  // The complete axe ruleset runs in Chrome; no rules are disabled for headless execution.
  const violations = await page.evaluate(async () => {
    if (globalThis.axe === undefined) throw new Error('axe-core did not initialize in Chrome');
    const result = await globalThis.axe.run(document);
    return result.violations.map(({ help, id, impact, nodes }) => ({
      help,
      id,
      impact,
      targets: nodes.slice(0, 3).map((node) => node.target),
    }));
  });
  if (violations.length > 0) {
    throw new Error(`${name}: axe violations ${JSON.stringify(violations)}`);
  }
}

async function assertKeyboardFlow(page, name) {
  await page.evaluate(() => {
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
  });
  const focused = [];
  for (let index = 0; index < 6; index += 1) {
    await page.keyboard.press('Tab');
    focused.push(await page.evaluate(() => {
      const element = document.activeElement;
      if (!(element instanceof HTMLElement)) return null;
      const rect = element.getBoundingClientRect();
      return {
        identity: `${element.tagName}:${element.getAttribute('href') ?? element.id}:${element.textContent?.trim().slice(0, 24)}`,
        interactive: element.matches('a[href],button,input,select,textarea,[tabindex]'),
        visible: rect.width > 0 && rect.height > 0,
      };
    }));
  }
  const valid = focused.filter((entry) => entry?.interactive && entry.visible);
  if (valid.length !== focused.length || new Set(valid.map(({ identity }) => identity)).size < 4) {
    throw new Error(`${name}: keyboard focus did not traverse visible controls ${JSON.stringify(focused)}`);
  }
  await page.evaluate(() => {
    if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
  });
  await page.waitForTimeout(200);
  const skipLinkSettled = await page.locator('.bcsp-skip-link').evaluate((element) => {
    const rectangle = element.getBoundingClientRect();
    return !element.matches(':focus') && rectangle.bottom <= 0;
  });
  if (!skipLinkSettled) throw new Error(`${name}: skip link did not leave the viewport after focus moved`);
}

async function assertNoOverflow(page, name) {
  const dimensions = await page.evaluate(() => ({
    clientWidth: document.documentElement.clientWidth,
    scrollWidth: document.documentElement.scrollWidth,
  }));
  if (dimensions.scrollWidth > dimensions.clientWidth + 1) {
    throw new Error(`${name}: horizontal overflow ${dimensions.scrollWidth}/${dimensions.clientWidth}`);
  }
}

async function assertPolishState(page, scenario) {
  const currentLinks = await page.locator('.bcsp-navigation a[aria-current="page"]').count();
  if (currentLinks !== 1) {
    throw new Error(`${scenario.name}: expected exactly one aria-current page link, received ${currentLinks}`);
  }

  if (scenario.target === 'local') {
    const savedState = page.locator('.local-page__settings-actions [data-state]');
    if (await savedState.getAttribute('data-state') !== 'UNCHANGED'
      || !await page.getByRole('button', { name: /Save settings|保存设置/u }).isDisabled()) {
      throw new Error(`${scenario.name}: unchanged Settings state was not truthful`);
    }
    if (await page.locator('.local-personal__hero').count() !== 0) {
      throw new Error(`${scenario.name}: duplicate local page hero remains`);
    }
    const localeControl = page.locator('.local-personal select').first();
    await localeControl.focus();
    const focus = await localeControl.evaluate((element) => {
      const style = getComputedStyle(element);
      return {
        color: style.outlineColor,
        style: style.outlineStyle,
        width: Number.parseFloat(style.outlineWidth),
      };
    });
    if (focus.style !== 'solid' || focus.width < 3 || focus.color === 'rgba(0, 0, 0, 0)') {
      throw new Error(`${scenario.name}: local form focus indicator is not visibly rendered ${JSON.stringify(focus)}`);
    }
  } else {
    const rail = await page.locator('.bcsp-rail__note').innerText();
    if (/A watched Section is open|监看的课节已有开放名额/u.test(rail)) {
      throw new Error(`${scenario.name}: empty Watch rail still reports an Open episode`);
    }
    const unavailableBatchActions = await page.getByRole('button', {
      name: /Start selected|Apply policy to active|Acknowledge all|开始监看所选课节|将策略应用到正在监看的课节|全部确认/u,
    }).count();
    if (unavailableBatchActions !== 0) {
      throw new Error(`${scenario.name}: empty Watch path exposes unavailable batch actions`);
    }
  }

  if (scenario.viewport.width <= 390) {
    const expectedColumns = scenario.viewport.width <= 336 ? 2 : 3;
    const navColumns = await page.locator('.bcsp-navigation').evaluate((element) =>
      getComputedStyle(element).gridTemplateColumns.split(' ').filter(Boolean).length);
    if (navColumns !== expectedColumns) {
      throw new Error(`${scenario.name}: navigation columns ${navColumns}/${expectedColumns}`);
    }
    const headingVisible = await page.locator('.bcsp-workspace__title').evaluate((element) => {
      const rectangle = element.getBoundingClientRect();
      return rectangle.top < globalThis.innerHeight && rectangle.bottom > 0;
    });
    if (!headingVisible) throw new Error(`${scenario.name}: current task title is outside the first viewport`);

    if (scenario.target === 'public') {
      const expectedStatusColumns = scenario.viewport.width <= 336 ? 1 : 2;
      for (const selector of ['.bcsp-status-grid', '.watch-workspace__status-strip']) {
        const columns = await page.locator(selector).evaluate((element) =>
          getComputedStyle(element).gridTemplateColumns.split(' ').filter(Boolean).length);
        if (columns !== expectedStatusColumns) {
          throw new Error(`${scenario.name}: ${selector} columns ${columns}/${expectedStatusColumns}`);
        }
      }
    }
  }
}

async function settle(page) {
  await page.evaluate(async () => {
    globalThis.scrollTo(0, 0);
    await document.fonts.ready;
    await new Promise((resolveFrame) => requestAnimationFrame(() => requestAnimationFrame(resolveFrame)));
  });
}

async function assertPublicBoundary(page, name) {
  const localLinks = await page.locator(
    'a[href="/saved-views"], a[href="/history"], a[href="/settings"]',
  ).count();
  const localDom = await page.locator('.local-personal, [data-bcsp-local-page]').count();
  const body = await page.locator('body').innerText();
  const forbidden = [
    'Saved views', 'Local watch history', 'Local settings', 'Reset scopes',
    '已保存视图', '历史记录', '本地设置', '重置范围',
  ].filter((message) => body.includes(message));
  if (localLinks !== 0 || localDom !== 0 || forbidden.length > 0) {
    throw new Error(`${name}: public composition exposed local UI ${JSON.stringify({
      forbidden,
      localDom,
      localLinks,
    })}`);
  }
}

async function exerciseLocal(page, scenario) {
  await page.goto(`${localBaseUrl}/local.html`, { waitUntil: 'domcontentloaded' });
  await waitForComposition(page, scenario);

  await page.locator('.bcsp-navigation a[href="/saved-views"]').click();
  await page.waitForURL(/\/saved-views$/u);
  await page.locator('.local-personal__card').filter({ hasText: savedView.name }).waitFor();
  await assertAxe(page, `${scenario.name}/saved-views`);

  await page.locator('.bcsp-navigation a[href="/history"]').click();
  await page.waitForURL(/\/history$/u);
  await page.locator('.local-personal').filter({ hasText: '00001' }).waitFor();
  await assertAxe(page, `${scenario.name}/history`);

  await page.locator('.bcsp-navigation a[href="/settings"]').click();
  await page.waitForURL(/\/settings$/u);
  const scopes = page.locator('.local-page__reset-scope');
  await scopes.first().waitFor();
  const scopeKinds = await scopes.evaluateAll((elements) => elements.map(
    (element) => element.getAttribute('data-scope'),
  ));
  const actionable = await scopes.evaluateAll((elements) => elements.every(
    (element) => element.querySelector('button') !== null,
  ));
  if (scopeKinds.length !== 3 || new Set(scopeKinds).size !== 3 || !actionable) {
    throw new Error(`${scenario.name}: expected three distinct actionable reset scopes, received ${JSON.stringify(scopeKinds)}`);
  }
  await assertAxe(page, `${scenario.name}/settings`);
}

async function exercisePublic(page, scenario) {
  await page.goto(`${publicBaseUrl}/public.html`, { waitUntil: 'domcontentloaded' });
  await waitForComposition(page, scenario);
  await page.locator('.bcsp-search-workspace').waitFor();
  await assertPublicBoundary(page, `${scenario.name}/search`);
  await assertAxe(page, `${scenario.name}/search`);

  await page.locator('.bcsp-navigation a[href="/watch"]').click();
  await page.locator('.watch-workspace').waitFor();
  const policy = page.locator('input[name="watch-mode"]');
  await policy.nth(1).check();
  await page.locator('.watch-workspace__confirm input[type="checkbox"]').check();
  await page.locator('#watch-max-audible').fill('5');
  await page.locator('#watch-volume').fill('35');
  if (!await policy.nth(1).isChecked() || await page.locator('#watch-volume').inputValue() !== '35') {
    throw new Error(`${scenario.name}: Watch controls did not accept an ephemeral edit`);
  }
  const firstSession = await page.locator('#bcsp-bootstrap').textContent();
  await page.reload({ waitUntil: 'domcontentloaded' });
  await waitForComposition(page, scenario);
  await page.locator('.watch-workspace').waitFor();
  const secondSession = await page.locator('#bcsp-bootstrap').textContent();
  const metrics = await page.locator(
    '.watch-workspace__status-strip .bcsp-metric dd',
  ).allTextContents();
  if (firstSession === secondSession
    || metrics[0] !== '0'
    || metrics[1] !== '0'
    || !await page.locator('input[name="watch-mode"]').first().isChecked()
    || await page.locator('#watch-max-audible').inputValue() !== '3'
    || await page.locator('#watch-volume').inputValue() !== '70') {
    throw new Error(`${scenario.name}: public reload did not restore a fresh session ${JSON.stringify({
      maxAudible: await page.locator('#watch-max-audible').inputValue(),
      metrics,
      oneShot: await page.locator('input[name="watch-mode"]').first().isChecked(),
      sessionChanged: firstSession !== secondSession,
      volume: await page.locator('#watch-volume').inputValue(),
    })}`);
  }
  await assertPublicBoundary(page, `${scenario.name}/reload`);
  await assertAxe(page, `${scenario.name}/reload`);
}

const scenarios = ['local', 'public'].flatMap((target) =>
  ['en-US', 'zh-CN'].flatMap((locale) => [
    { target, locale, viewport: { name: 'desktop', width: 1440, height: 1400 } },
    { target, locale, viewport: { name: 'mobile', width: 390, height: 844 } },
  ].map((scenario) => ({
    ...scenario,
    name: `${target}-${locale}-${scenario.viewport.name}`,
  }))));
const narrowScenarios = ['local', 'public'].map((target) => ({
  target,
  locale: 'en-US',
  viewport: { name: 'narrow', width: 320, height: 568 },
  name: `${target}-en-US-narrow`,
}));

await mkdir(outputDirectory, { recursive: true });
const browser = await chromium.launch({
  ...(executablePath === undefined ? {} : { executablePath }),
  args: ['--disable-gpu'],
  headless: true,
});

try {
  for (const scenario of scenarios) {
    const page = await browser.newPage({
      colorScheme: 'light',
      locale: scenario.locale,
      reducedMotion: 'reduce',
      viewport: { height: scenario.viewport.height, width: scenario.viewport.width },
    });
    try {
      await page.addInitScript({ content: axeSource });
      if (scenario.target === 'public') await installPublicDocuments(page);
      await installApi(page, scenario);
      if (scenario.target === 'local') await exerciseLocal(page, scenario);
      else await exercisePublic(page, scenario);
      await assertPolishState(page, scenario);
      await settle(page);
      await assertNoOverflow(page, scenario.name);
      if (scenario.locale === 'en-US' && scenario.viewport.name === 'mobile') {
        await page.screenshot({
          animations: 'disabled',
          fullPage: false,
          path: resolve(outputDirectory, `${scenario.target}-en-US-390-first-view.png`),
        });
      }
      await page.screenshot({
        animations: 'disabled',
        fullPage: scenario.viewport.name === 'mobile',
        path: resolve(outputDirectory, `${scenario.name}.png`),
      });
      await assertKeyboardFlow(page, scenario.name);
    } finally {
      await page.close();
    }
  }
  for (const scenario of narrowScenarios) {
    const page = await browser.newPage({
      colorScheme: 'light',
      locale: scenario.locale,
      reducedMotion: 'reduce',
      viewport: { height: scenario.viewport.height, width: scenario.viewport.width },
    });
    try {
      await page.addInitScript({ content: axeSource });
      if (scenario.target === 'public') await installPublicDocuments(page);
      await installApi(page, scenario);
      if (scenario.target === 'local') await exerciseLocal(page, scenario);
      else await exercisePublic(page, scenario);
      await assertPolishState(page, scenario);
      await settle(page);
      await assertNoOverflow(page, scenario.name);
      await page.screenshot({
        animations: 'disabled',
        fullPage: false,
        path: resolve(outputDirectory, `${scenario.target}-en-US-320-first-view.png`),
      });
      await assertKeyboardFlow(page, scenario.name);
    } finally {
      await page.close();
    }
  }
} finally {
  await browser.close();
}

const totalScenarios = scenarios.length + narrowScenarios.length;
process.stdout.write(`${evidenceTask} composition matrix: PASS (${totalScenarios}/${totalScenarios})\n`);
