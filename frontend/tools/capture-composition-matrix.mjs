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
const outputDirectory = resolve(process.cwd(), process.env.BCSP_EVIDENCE_DIR
  ?? '../project-governance/current/rc-iteration/evidence/round-05/stage-1/composition');
const evidenceStage = process.env.BCSP_EVIDENCE_STAGE ?? 'Stage1';
const installedApiDocuments = new WeakMap();
const [filterSchema, axeSource] = await Promise.all([
  readFile(resolve(process.cwd(), '../crates/bcsp-contracts/tests/golden/filter-schema-v1.json'), 'utf8')
    .then((source) => JSON.parse(source)),
  readFile(resolve(process.cwd(), 'node_modules/axe-core/axe.min.js'), 'utf8'),
]);

const observedAt = '2026-07-17T15:00:00.000Z';
const currentTerm = '72026';
const nextTerm = '92026';
const campuses = ['NB', 'NK', 'CM'];
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
  coreCodeDictionaries: [],
  subjects: [{
    code: '198',
    label: known('Computer Science'),
    provenance: { kind: 'DISCOVERY', discovery: provenance },
    target: { campus: 'NB', term: currentTerm },
  }],
  targets: [currentTerm, nextTerm].flatMap((term) => campuses.map((campus) => ({
    campusLabel: known(campus),
    key: { campus, term },
    provenance,
    termLabel: known('Upstream label must not render'),
  }))),
};

const visibleTermDefinitions = [
  { term: '02026', relativeOffset: -2, publication: 'PUBLISHED', autoManaged: false, manualPullAllowed: true, watchable: false },
  { term: '12026', relativeOffset: -1, publication: 'PUBLISHED', autoManaged: false, manualPullAllowed: true, watchable: false },
  { term: currentTerm, relativeOffset: 0, publication: 'PUBLISHED', autoManaged: true, manualPullAllowed: false, watchable: true },
  { term: nextTerm, relativeOffset: 1, publication: 'PUBLISHED', autoManaged: true, manualPullAllowed: false, watchable: true },
  { term: '02027', relativeOffset: 2, publication: 'UNPUBLISHED', autoManaged: false, manualPullAllowed: true, watchable: false },
];

function serviceStatus(runtime) {
  const visibleTerms = runtime === 'LOCAL'
    ? visibleTermDefinitions
    : visibleTermDefinitions.filter(({ relativeOffset }) => relativeOffset === 0 || relativeOffset === 1);
  const targets = visibleTerms.flatMap(({ term, relativeOffset }) => campuses.map((campus, index) => {
    const usable = relativeOffset === 0 || relativeOffset === 1;
    return {
      target: { campus, term },
      primary: relativeOffset === 0 && index === 0,
      snapshotAvailability: usable ? 'READY' : 'UNREQUESTED',
      workState: 'IDLE',
      stage: null,
      usable,
      catalogContentVersion: usable ? point.contentVersion : null,
      lastCompleteAt: usable ? observedAt : null,
      nextRetryAt: null,
      error: null,
    };
  }));
  return {
    contractVersion: 2,
    observedAt,
    runtime,
    level: 'READY',
    discovery: discovery.status,
    termWindow: { currentTerm, nextTerm, visibleTerms },
    automaticTermSummaries: [
      { term: currentTerm, readyTargetCount: 3, totalTargetCount: 3 },
      { term: nextTerm, readyTargetCount: 3, totalTargetCount: 3 },
    ],
    operations: [],
    targets,
    issues: [],
  };
}

const neutralFilters = {
  term: currentTerm,
  campuses: [],
  subjects: [],
  keywords: ['computer', 'science'],
  courseNumberBands: [],
  levels: [],
  credits: null,
  core: { codes: [], mode: 'ANY' },
  prerequisite: 'ANY',
  sectionIndexes: [],
  openStatuses: [],
  modalities: [],
  synchronicities: [],
  instructors: [],
  availability: [],
  meetingLocations: { locations: [], mode: 'ANY_MEETING' },
  examCodes: [],
  permission: 'ANY',
  includeIncomplete: {
    prerequisite: false,
    modality: false,
    synchronicity: false,
  },
};
const filterRequest = { contractVersion: 3, values: neutralFilters };
const savedView = {
  id: '20000000-0000-4000-8000-000000000001',
  name: 'Morning CS options',
  schemaVersion: 2,
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
        watchFastLaneSeconds: 10,
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
    ['/api/v1/service/status', serviceStatus(scenario.target === 'local' ? 'LOCAL' : 'PUBLIC')],
    ['/api/v1/local/bootstrap', local.bootstrap],
    ['/api/v1/local/saved-views', local.library],
  ]);
  installedApiDocuments.set(page, routes);
  await page.route('**/api/v1/**', async (route) => {
    const path = new URL(route.request().url()).pathname;
    if (path === '/api/v1/query/filter-options') {
      const request = route.request().postDataJSON()?.payload ?? {};
      const options = request.field === 'COURSE_NUMBER_BAND'
        ? ['0', '100', '400'].map((value) => ({ value, label: value }))
        : request.field === 'COURSE_LEVEL'
          ? [{ value: 'U', label: 'Undergraduate' }, { value: 'G', label: 'Graduate' }]
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
      return;
    }
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

function replaceFuturePublication(page, publication) {
  const routes = installedApiDocuments.get(page);
  const current = routes?.get('/api/v1/service/status');
  if (routes === undefined || current === undefined) {
    throw new Error('composition API documents are unavailable');
  }
  const replacement = structuredClone(current);
  const future = replacement.termWindow.visibleTerms.find(({ term }) => term === '02027');
  if (future === undefined) throw new Error('Winter 2027 is missing from Local status');
  future.publication = publication;
  routes.set('/api/v1/service/status', replacement);
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
      targets: nodes.slice(0, 3).map((node) => ({
        failureSummary: node.failureSummary,
        html: node.html,
        target: node.target,
      })),
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
        interactive: element.matches('a[href],button,input,select,textarea,summary,[tabindex]'),
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

async function assertReducedMotion(page, name) {
  const motion = await page.evaluate(() => {
    const interactive = [
      document.querySelector('.bcsp-navigation__link'),
      document.querySelector('.bcsp-action'),
      document.querySelector('.bcsp-language__button'),
    ].filter((element) => element instanceof HTMLElement);
    const parseDurations = (value) => value.split(',').map((part) => {
      const duration = part.trim();
      return duration.endsWith('ms')
        ? Number.parseFloat(duration)
        : Number.parseFloat(duration) * 1000;
    });
    return {
      maxTransitionMs: Math.max(0, ...interactive.flatMap((element) =>
        parseDurations(getComputedStyle(element).transitionDuration))),
      reduced: matchMedia('(prefers-reduced-motion: reduce)').matches,
    };
  });
  if (!motion.reduced || motion.maxTransitionMs > 0.0011) {
    throw new Error(`${name}: reduced-motion contract failed ${JSON.stringify(motion)}`);
  }
}

async function assertQueryScope(page, scenario) {
  const expectedLayout = scenario.target === 'local' ? 'local-2x5' : 'public-2x3-search';
  const scope = page.locator(`.query-scope[data-scope-layout="${expectedLayout}"]`);
  await scope.waitFor();
  const actualCells = await scope.locator('[data-scope-cell]').evaluateAll((elements) =>
    elements.map((element) => element.getAttribute('data-scope-cell')));
  const expectedCells = scenario.target === 'local'
    ? ['term--2', 'term--1', 'term-0', 'term-1', 'term-2', 'campus-NB', 'campus-NK', 'campus-CM', 'scope-action', 'search']
    : ['term-0', 'term-1', 'campus-NB', 'campus-NK', 'campus-CM', 'scope-action', 'search'];
  if (JSON.stringify(actualCells) !== JSON.stringify(expectedCells)) {
    throw new Error(`${scenario.name}: QueryScope matrix mismatch ${JSON.stringify(actualCells)}`);
  }
  if (scenario.viewport.width <= 390) {
    const boxes = await scope.locator('[data-scope-cell]').evaluateAll((elements) => elements.map((element) => {
      const box = element.getBoundingClientRect();
      return { left: box.left, top: box.top };
    }));
    const oneColumn = boxes.every((box) => Math.abs(box.left - (boxes[0]?.left ?? box.left)) <= 1)
      && boxes.every((box, index) => index === 0 || box.top > (boxes[index - 1]?.top ?? -1));
    if (!oneColumn) throw new Error(`${scenario.name}: narrow QueryScope is not one semantic column ${JSON.stringify(boxes)}`);
  }
  const termIds = await scope.locator('[data-scope-cell^="term-"] samp').allTextContents();
  const expectedTerms = scenario.target === 'local'
    ? ['02026', '12026', currentTerm, nextTerm, '02027']
    : [currentTerm, nextTerm];
  if (JSON.stringify(termIds) !== JSON.stringify(expectedTerms)) {
    throw new Error(`${scenario.name}: authoritative term order mismatch ${JSON.stringify(termIds)}`);
  }
  const scopeGeometry = await page.locator('.bcsp-search-workspace').evaluate((workspace) => {
    const scopeElement = workspace.querySelector('.bcsp-search-workspace__scope');
    const filtersElement = workspace.querySelector('.bcsp-search-workspace__filters');
    if (!(scopeElement instanceof HTMLElement) || !(filtersElement instanceof HTMLElement)) return null;
    const workspaceBox = workspace.getBoundingClientRect();
    const scopeBox = scopeElement.getBoundingClientRect();
    return {
      insideFilters: filtersElement.contains(scopeElement),
      scopeWidth: scopeBox.width,
      workspaceWidth: workspaceBox.width,
    };
  });
  if (scopeGeometry === null || !scopeGeometry.insideFilters) {
    throw new Error(`${scenario.name}: QueryScope is not a child of the filter rail ${JSON.stringify(scopeGeometry)}`);
  }
  if (scenario.viewport.width >= 768
    && scopeGeometry.scopeWidth >= scopeGeometry.workspaceWidth - 2) {
    throw new Error(`${scenario.name}: QueryScope incorrectly spans the desktop workspace ${JSON.stringify(scopeGeometry)}`);
  }
  if (await page.locator('.bcsp-search-workspace__results .bcsp-state-panel').count() !== 1) {
    throw new Error(`${scenario.name}: the RC3 idle StatePanel is not visible in the right workspace`);
  }
  if (await scope.locator('[data-scope-cell="search"] button[type="submit"]').count() !== 1
    || await page.getByRole('button', { name: /Search|搜索/u, exact: true }).count() !== 1) {
    throw new Error(`${scenario.name}: expected one native Search submit`);
  }
  const searchForm = await scope.locator('[data-scope-cell="search"] button').getAttribute('form');
  if (searchForm === null || await page.locator(`form[id="${searchForm}"]`).count() !== 1) {
    throw new Error(`${scenario.name}: QueryScope Search is not associated with the filter form`);
  }
  if (scenario.target === 'public' && await page.getByRole('button', { name: /Pull|拉取/u }).count() !== 0) {
    throw new Error(`${scenario.name}: Public exposed a Local-only Pull action`);
  }
  if (await page.locator('.filter-panel details').count() !== 0
    || await page.locator('.filter-panel [data-filter-row]').count() !== 16) {
    throw new Error(`${scenario.name}: filters 03–18 are not one flat, continuously visible sequence`);
  }
  const filterScroll = await page.locator('.bcsp-search-workspace__filters').evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      clientHeight: element.clientHeight,
      overflowY: style.overflowY,
      scrollbarColor: style.scrollbarColor,
      scrollbarWidth: style.scrollbarWidth,
      scrollHeight: element.scrollHeight,
      tabIndex: element.tabIndex,
    };
  });
  if (scenario.viewport.width >= 768) {
    if (!/auto|scroll/u.test(filterScroll.overflowY)
      || filterScroll.scrollHeight <= filterScroll.clientHeight + 1
      || filterScroll.tabIndex !== 0
      || filterScroll.scrollbarWidth !== 'thin'
      || filterScroll.scrollbarColor === 'auto') {
      throw new Error(`${scenario.name}: filter rail lost styled accessible scrolling ${JSON.stringify(filterScroll)}`);
    }
  } else if (/hidden|clip/u.test(filterScroll.overflowY)) {
    throw new Error(`${scenario.name}: narrow filter rail clips content ${JSON.stringify(filterScroll)}`);
  } else if (scenario.viewport.width <= 390) {
    await assertNarrowDocumentKeyboard(page, scenario.name);
  }
  if (scenario.locale === 'en-US' && scenario.viewport.width === 1440) {
    await assertFilterRailInputs(page, scenario.name);
  }
}

async function assertNarrowDocumentKeyboard(page, name) {
  const rail = page.locator('.bcsp-search-workspace__filters');
  const initial = await rail.evaluate((element) => ({
    clientHeight: element.clientHeight,
    overflowY: getComputedStyle(element).overflowY,
    scrollHeight: element.scrollHeight,
  }));
  if (/auto|scroll/u.test(initial.overflowY) && initial.scrollHeight > initial.clientHeight + 1) {
    throw new Error(`${name}: narrow rail still owns an independent scrollport ${JSON.stringify(initial)}`);
  }

  await page.evaluate(() => window.scrollTo(0, 0));
  await rail.focus();
  for (const key of ['Home', 'End', 'PageUp', 'PageDown']) {
    const result = await rail.evaluate((element, pressedKey) => {
      let prevented = false;
      const listener = (event) => { prevented = event.defaultPrevented; };
      element.addEventListener('keydown', listener, { once: true });
      const dispatched = element.dispatchEvent(new KeyboardEvent('keydown', {
        bubbles: true,
        cancelable: true,
        key: pressedKey,
      }));
      return { dispatched, prevented };
    }, key);
    if (!result.dispatched || result.prevented) {
      throw new Error(`${name}: narrow document-flow rail canceled ${key} ${JSON.stringify(result)}`);
    }
  }

  await page.evaluate(() => window.scrollTo(0, 0));
  await page.keyboard.press('PageDown');
  await page.waitForTimeout(50);
  const documentScroll = await page.evaluate(() => window.scrollY);
  if (documentScroll <= 0) {
    throw new Error(`${name}: PageDown did not continue scrolling the document (${documentScroll})`);
  }
  await page.evaluate(() => window.scrollTo(0, 0));
}

async function assertFilterRailInputs(page, name) {
  const rail = page.locator('.bcsp-search-workspace__filters');
  const metrics = await rail.evaluate((element) => ({
    clientHeight: element.clientHeight,
    scrollHeight: element.scrollHeight,
    touchAction: getComputedStyle(element).touchAction,
  }));
  if (metrics.touchAction !== 'pan-y' || metrics.scrollHeight <= metrics.clientHeight) {
    throw new Error(`${name}: filter rail touch contract failed ${JSON.stringify(metrics)}`);
  }

  await rail.focus();
  await rail.press('End');
  await page.waitForTimeout(50);
  const endScroll = await rail.evaluate((element) => element.scrollTop);
  if (endScroll < metrics.scrollHeight - metrics.clientHeight - 2) {
    throw new Error(`${name}: End did not reach the filter rail end (${endScroll})`);
  }
  await rail.press('Home');
  await page.waitForTimeout(50);
  if (await rail.evaluate((element) => element.scrollTop) > 1) {
    throw new Error(`${name}: Home did not return to the filter rail start`);
  }
  await rail.press('PageDown');
  await page.waitForTimeout(50);
  if (await rail.evaluate((element) => element.scrollTop) <= 0) {
    throw new Error(`${name}: PageDown did not move the filter rail`);
  }
  await rail.press('PageUp');
  await page.waitForTimeout(50);
  if (await rail.evaluate((element) => element.scrollTop) > 1) {
    throw new Error(`${name}: PageUp did not return the filter rail to its start`);
  }

  await rail.hover();
  await page.mouse.wheel(0, 420);
  await page.waitForTimeout(50);
  if (await rail.evaluate((element) => element.scrollTop) <= 0) {
    throw new Error(`${name}: mouse wheel did not move the filter rail`);
  }

  await rail.evaluate((element) => { element.scrollTop = 0; });
  const box = await rail.boundingBox();
  const viewport = page.viewportSize();
  if (box === null || viewport === null) throw new Error(`${name}: filter rail touch geometry unavailable`);
  const cdp = await page.context().newCDPSession(page);
  const x = box.x + 8;
  const startY = Math.min(box.y + box.height - 16, viewport.height - 16);
  await cdp.send('Input.dispatchTouchEvent', {
    type: 'touchStart', touchPoints: [{ x, y: startY }], modifiers: 0,
  });
  for (let step = 1; step <= 5; step += 1) {
    await cdp.send('Input.dispatchTouchEvent', {
      type: 'touchMove', touchPoints: [{ x, y: startY - step * 35 }], modifiers: 0,
    });
  }
  await cdp.send('Input.dispatchTouchEvent', { type: 'touchEnd', touchPoints: [], modifiers: 0 });
  await cdp.detach();
  await page.waitForTimeout(100);
  if (await rail.evaluate((element) => element.scrollTop) <= 0) {
    throw new Error(`${name}: touch pan did not move the filter rail`);
  }
  await rail.evaluate((element) => {
    element.scrollTop = 0;
    element.blur();
  });
}

async function captureQueryScope(page, path) {
  const scope = page.locator('.query-scope');
  const originalViewport = page.viewportSize();
  if (originalViewport === null) throw new Error('QueryScope evidence requires a fixed viewport');
  await page.evaluate(() => globalThis.scrollTo(0, 0));
  const geometry = await scope.evaluate((element) => {
    const box = element.getBoundingClientRect();
    return { bottom: box.bottom, height: box.height };
  });
  const evidenceHeight = Math.min(4096, Math.max(
    originalViewport.height,
    Math.ceil(geometry.bottom + 24),
    Math.ceil(geometry.height + 24),
  ));
  if (evidenceHeight !== originalViewport.height) {
    await page.setViewportSize({ width: originalViewport.width, height: evidenceHeight });
    await page.evaluate(() => globalThis.scrollTo(0, 0));
  }
  await scope.screenshot({ animations: 'disabled', path });
  if (evidenceHeight !== originalViewport.height) await page.setViewportSize(originalViewport);
  await page.evaluate(() => globalThis.scrollTo(0, 0));
}

async function captureFigureOneStates(page, scenario) {
  const pullName = scenario.locale === 'zh-CN' ? '拉取' : 'Pull';
  const publicationReason = {
    UNPUBLISHED: scenario.locale === 'zh-CN' ? 'Rutgers 尚未发布' : 'Not published by Rutgers',
    UNKNOWN: scenario.locale === 'zh-CN' ? '发布状态未知' : 'Publication status unknown',
  };
  const assertDisabledPullWithoutVisibleReason = async (publication, evidenceName) => {
    const pull = page.getByRole('button', { name: pullName, exact: true });
    const reasonId = await pull.getAttribute('aria-describedby');
    const accessibleReason = reasonId === null ? null : page.locator(`[id="${reasonId}"]`);
    const accessibleReasonText = accessibleReason === null ? null : await accessibleReason.textContent();
    const accessibleReasonVisible = accessibleReason === null ? null : await accessibleReason.isVisible();
    const pullTitle = await pull.getAttribute('title');
    const actionText = (await pull.innerText()).trim();
    const scopeText = await page.locator('.query-scope').innerText();
    const forbiddenPublicationCopy = /Not published by Rutgers|Publication status unknown|尚未发布|发布状态未知/u;
    if (!await pull.isDisabled()
      || accessibleReasonText !== publicationReason[publication]
      || accessibleReasonVisible !== false
      || pullTitle !== null
      || forbiddenPublicationCopy.test(scopeText)
      || actionText.toLocaleLowerCase(scenario.locale) !== pullName.toLocaleLowerCase(scenario.locale)) {
      throw new Error(`${scenario.name}: ${publication} Winter 2027 Pull must be disabled without a visible publication explanation while retaining its screen-reader description; disabled=${await pull.isDisabled()} describedby=${reasonId} reason=${accessibleReasonText} reasonVisible=${accessibleReasonVisible} title=${pullTitle} action=${JSON.stringify(actionText)} scope=${JSON.stringify(scopeText)}`);
    }
    await assertAxe(page, `${scenario.name}/${publication.toLocaleLowerCase('en-US')}-pull`);
    await settle(page);
    await captureQueryScope(page, resolve(outputDirectory, evidenceName));
  };

  await page.locator('[data-scope-cell="term--2"] input[type="radio"]').check();
  const publishedPull = page.getByRole('button', { name: pullName, exact: true });
  if (await publishedPull.isDisabled()) {
    throw new Error(`${scenario.name}: published 0/3 Winter 2026 Pull is disabled`);
  }
  await settle(page);
  await captureQueryScope(page, resolve(outputDirectory, `${scenario.name}-figure1-published-pull.png`));

  await page.locator('[data-scope-cell="term-2"] input[type="radio"]').check();
  await assertDisabledPullWithoutVisibleReason(
    'UNPUBLISHED',
    `${scenario.name}-figure1-unpublished-pull.png`,
  );

  replaceFuturePublication(page, 'UNKNOWN');
  await page.reload({ waitUntil: 'domcontentloaded' });
  await waitForComposition(page, scenario);
  await page.locator('[data-scope-cell="term-2"] input[type="radio"]').check();
  await assertDisabledPullWithoutVisibleReason(
    'UNKNOWN',
    `${scenario.name}-figure1-unknown-pull.png`,
  );

  for (const cell of ['term-0', 'term-1']) {
    await page.locator(`[data-scope-cell="${cell}"] input[type="radio"]`).check();
    if (await page.getByRole('button', { name: pullName, exact: true }).count() !== 0) {
      throw new Error(`${scenario.name}: ${cell} exposed a forbidden Pull action`);
    }
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
    const railNote = page.locator('.bcsp-rail__note');
    if (await railNote.count() > 0) {
      const rail = await railNote.innerText();
      if (/A watched Section is open|监看的课节已有开放名额/u.test(rail)) {
        throw new Error(`${scenario.name}: empty Watch rail still reports an Open episode`);
      }
    }
    const unavailableBatchActions = await page.getByRole('button', {
      name: /Start selected|Apply policy to active|Acknowledge all|开始监看所选课节|将策略应用到正在监看的课节|全部确认/u,
    }).count();
    if (unavailableBatchActions !== 0) {
      throw new Error(`${scenario.name}: empty Watch path exposes unavailable batch actions`);
    }
  }

  if (scenario.viewport.width <= 390) {
    const expectedColumns = scenario.target === 'public' || scenario.viewport.width <= 336 ? 2 : 3;
    const navColumns = await page.locator('.bcsp-navigation').evaluate((element) =>
      getComputedStyle(element).gridTemplateColumns.split(' ').filter(Boolean).length);
    if (navColumns !== expectedColumns) {
      throw new Error(`${scenario.name}: navigation columns ${navColumns}/${expectedColumns}`);
    }
    if (scenario.viewport.width >= 390) {
      const headingVisible = await page.locator('.bcsp-workspace__title').evaluate((element) => {
        const rectangle = element.getBoundingClientRect();
        return rectangle.top < globalThis.innerHeight && rectangle.bottom > 0;
      });
      if (!headingVisible) throw new Error(`${scenario.name}: current task title is outside the first viewport`);
    }

    if (scenario.target === 'public') {
      const expectedStatusColumns = scenario.viewport.width <= 336 ? 1 : 2;
      for (const selector of ['.bcsp-status-grid', '.watch-workspace__status-strip']) {
        if (await page.locator(selector).count() === 0) continue;
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
  await assertReducedMotion(page, scenario.name);
  await page.locator('.bcsp-search-workspace').waitFor();
  await assertQueryScope(page, scenario);
  await captureQueryScope(page, resolve(outputDirectory, `${scenario.name}-query-scope.png`));
  if (scenario.viewport.width === 1440) {
    await page.screenshot({
      animations: 'disabled',
      fullPage: false,
      path: resolve(outputDirectory, `${scenario.name}-search-idle.png`),
    });
  }
  if (scenario.viewport.width === 1440) await captureFigureOneStates(page, scenario);

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
  await assertReducedMotion(page, scenario.name);
  await page.locator('.bcsp-search-workspace').waitFor();
  await assertQueryScope(page, scenario);
  await captureQueryScope(page, resolve(outputDirectory, `${scenario.name}-query-scope.png`));
  if (scenario.viewport.width === 1440) {
    await page.screenshot({
      animations: 'disabled',
      fullPage: false,
      path: resolve(outputDirectory, `${scenario.name}-search-idle.png`),
    });
  }
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

const frozenViewports = [
  { name: '390', width: 390, height: 844 },
  { name: '768', width: 768, height: 1024 },
  { name: '1440', width: 1440, height: 1200 },
  { name: '1920', width: 1920, height: 1200 },
  { name: '2560', width: 2560, height: 1280 },
];
const scenarios = ['local', 'public'].flatMap((target) =>
  ['en-US', 'zh-CN'].flatMap((locale) => frozenViewports.map((viewport) => ({
    target,
    locale,
    viewport,
    name: `${target}-${locale}-${viewport.name}`,
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
      if (scenario.locale === 'en-US' && scenario.viewport.width === 390) {
        await page.screenshot({
          animations: 'disabled',
          fullPage: false,
          path: resolve(outputDirectory, `${scenario.target}-en-US-390-first-view.png`),
        });
      }
      await page.screenshot({
        animations: 'disabled',
        fullPage: scenario.viewport.width <= 390,
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
process.stdout.write(`RC5 ${evidenceStage} composition matrix: PASS (${totalScenarios}/${totalScenarios})\n`);
