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
  process.env.BCSP_VISUAL_OUTPUT ?? 'test-results/course-section-flow',
);
const filterSchema = JSON.parse(await readFile(
  resolve(process.cwd(), '../crates/bcsp-contracts/tests/golden/filter-schema-v1.json'),
  'utf8',
));
const observedAt = '2026-07-15T03:09:30.000Z';
const currentTerm = '72026';
const nextTerm = '92026';
const campuses = ['NB', 'NK', 'CM'];
const known = (value) => ({ knowledge: 'KNOWN', presence: { presence: 'PRESENT', value } });
const success = (data) => ({ protocolVersion: 1, data });
const groupKey = { campus: 'NB', courseString: '01:198:211', term: currentTerm };
const sectionKey = { campus: 'NB', index: '12345', term: currentTerm };
const variantKey = { fingerprint: 'lecture-4cr', group: groupKey };

const point = {
  contentVersion: 9,
  observationId: '10000000-0000-4000-8000-000000000009',
  observedAt,
};
const provenance = {
  observationId: point.observationId,
  observedAt,
  payloadDigest: 'a'.repeat(64),
  sourceId: 'rutgers-selector',
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
  subjects: Array.from({ length: 300 }, (_, index) => {
    const code = index === 198 ? '198' : `S${String(index).padStart(3, '0')}`;
    return {
      code,
      label: known(index === 198 ? 'Computer Science' : `Published subject ${index}`),
      provenance: { kind: 'DISCOVERY', discovery: provenance },
      target: { campus: 'NB', term: currentTerm },
    };
  }),
  targets: [currentTerm, nextTerm].flatMap((term) => campuses.map((campus) => ({
    campusLabel: known(campus),
    key: { campus, term },
    provenance,
    termLabel: known('Upstream term label must not render'),
  }))),
};

const visibleTerms = [
  { term: '02026', relativeOffset: -2, publication: 'PUBLISHED', autoManaged: false, manualPullAllowed: true, watchable: false },
  { term: '12026', relativeOffset: -1, publication: 'PUBLISHED', autoManaged: false, manualPullAllowed: true, watchable: false },
  { term: currentTerm, relativeOffset: 0, publication: 'PUBLISHED', autoManaged: true, manualPullAllowed: false, watchable: true },
  { term: nextTerm, relativeOffset: 1, publication: 'PUBLISHED', autoManaged: true, manualPullAllowed: false, watchable: true },
  { term: '02027', relativeOffset: 2, publication: 'UNPUBLISHED', autoManaged: false, manualPullAllowed: true, watchable: false },
];
const serviceStatus = {
  contractVersion: 2,
  observedAt,
  runtime: 'LOCAL',
  level: 'READY',
  discovery: discovery.status,
  termWindow: { currentTerm, nextTerm, visibleTerms },
  automaticTermSummaries: [
    { term: currentTerm, readyTargetCount: 3, totalTargetCount: 3 },
    { term: nextTerm, readyTargetCount: 3, totalTargetCount: 3 },
  ],
  operations: [],
  targets: visibleTerms.flatMap(({ term, relativeOffset }) => campuses.map((campus, index) => {
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
  })),
  issues: [],
};

const variant = {
  key: variantKey,
  title: known('Data Structures'),
  expandedTitle: known('Data Structures and Algorithms'),
  description: known('Foundational data structures, algorithm analysis, and implementation.'),
  notes: known('Department note'),
  subjectGroupNotes: known('Computer Science group'),
  subjectNotes: known('Computer Science'),
  unitNotes: known('School of Arts and Sciences'),
  synopsisUrl: known('https://example.invalid/01-198-211'),
  prerequisiteNotes: known('Placement or prior programming course'),
  prerequisiteState: 'HAS',
  credits: known('4.0'),
  level: known('UNDERGRADUATE'),
  subjectCode: known('198'),
  subjectDescription: known('Computer Science'),
  courseNumber: known('211'),
  supplementCode: known('A'),
  schoolCode: known('01'),
  offeringUnit: known('198'),
  offeringUnitTitle: known('Computer Science'),
  coreCodes: known(['QQ']),
  campusLocations: known(['NB']),
  sectionKeys: [sectionKey],
};

const section = {
  key: sectionKey,
  variantKey,
  sectionNumber: known('01'),
  subtitle: known('Data structures studio'),
  subtopic: known('Trees and graphs'),
  sectionNotes: known('Bring a laptop'),
  sessionDates: known('09/01–12/14'),
  sessionDatePrintIndicator: known('Y'),
  comments: known([]),
  commentsText: known('No additional comments'),
  crossListedSectionsText: known('None'),
  crossListedSectionType: known('NONE'),
  instructors: known(['Ada Lovelace']),
  instructorReliability: 'NAME_ONLY',
  rawSectionCourseType: known('LEC'),
  deliveryModality: 'ON_CAMPUS_OR_IN_PERSON',
  synchronicity: 'SYNC',
  examCode: known('A'),
  examCodeText: known('Common final'),
  specialPermissionAddCode: known('N'),
  specialPermissionAddDescription: known('No permission required'),
  specialPermissionDropCode: known('N'),
  specialPermissionDropDescription: known('No permission required'),
  majorCodes: known([]),
  unitCodes: known([]),
  minorCodes: known([]),
  honorProgramCodes: known([]),
  unitMajors: known([]),
  eligibilityText: known('Open to all students'),
  openToText: known('All students'),
  catalogOpenStatus: { provenance: 'CATALOG_SNAPSHOT_ONLY', value: known(true) },
  occurrenceKeys: known([{ ordinal: 1, section: sectionKey }]),
};

const occurrence = {
  key: { ordinal: 1, section: sectionKey },
  rawCode: known('LEC'),
  rawDescription: known('Lecture'),
  modality: known('ON_CAMPUS_OR_IN_PERSON'),
  synchronicity: known('SYNC'),
  rawDay: known('MW'),
  days: known(['MONDAY', 'WEDNESDAY']),
  rawStartTime: known('10:20 AM'),
  rawEndTime: known('11:40 AM'),
  time: { endMinute: 700, knowledge: 'KNOWN', startMinute: 620 },
  startDate: known('2026-09-01'),
  endDate: known('2026-12-14'),
  campus: known('NB'),
  campusName: known('New Brunswick'),
  building: known('HLL'),
  room: known('116'),
  requiredness: 'REQUIRED',
  kind: 'SCHEDULED',
  evidence: 'PHYSICAL',
  normalizationReason: 'NORMALIZED_MEETING',
};
const sectionItem = {
  section,
  occurrences: [occurrence],
  open: {
    freshUntil: '2026-07-15T03:10:00.000Z',
    observedAt,
    state: 'OPEN',
    uncertainty: null,
  },
  explanation: { outcome: 'MATCH', reasons: [] },
  filterMatches: [{ fieldId: 'FLT-S06', explanation: { outcome: 'MATCH', reasons: [] } }],
};
const courseItem = {
  explanation: { outcome: 'MATCH', reasons: [] },
  group: { key: groupKey, variantKeys: [variantKey] },
  variants: [{
    explanation: { outcome: 'MATCH', reasons: [] },
    filterMatches: [],
    sections: [sectionItem],
    textMatch: { exactCourseIdentifier: true, matchedTokens: ['211'] },
    variant,
  }],
};
const courseResponse = {
  contractVersion: 3,
  items: [courseItem],
  page: { page: 1, pageSize: 25, total: 1, totalPages: 1 },
};
const sectionResponse = {
  contractVersion: 3,
  items: [{ courseFilterMatches: [], section: sectionItem, textMatch: null, variant }],
  page: { page: 1, pageSize: 25, total: 1, totalPages: 1 },
};

async function installApi(page, requests) {
  const localBootstrap = localBootstrapFixture();
  const routes = new Map([
    ['/api/v1/local/bootstrap', localBootstrap],
    ['/api/v1/local/saved-views', {
      stateRevision: localBootstrap.state.stateRevision,
      currentFilters: localBootstrap.state.currentFilters,
      views: [],
    }],
    ['/api/v1/query/filter-schema', filterSchema],
    ['/api/v1/catalog/discovery', discovery],
    ['/api/v1/service/status', serviceStatus],
    ['/api/v1/query/courses', courseResponse],
    ['/api/v1/query/sections', sectionResponse],
    ['/api/v1/query/course-detail', { contractVersion: 3, course: courseItem }],
    ['/api/v1/query/section-detail', { contractVersion: 3, section: sectionItem, variant }],
  ]);
  await page.route('**/api/v1/**', async (route) => {
    const url = new URL(route.request().url());
    if (url.pathname === '/api/v1/local/current-filters') {
      const request = route.request().postDataJSON()?.payload ?? {};
      await route.fulfill({
        body: JSON.stringify(success({
          stateRevision: localBootstrap.state.stateRevision + 1,
          revision: localBootstrap.state.currentFilters.revision + 1,
          value: {
            association: { kind: 'CUSTOM' },
            content: { status: 'COMPATIBLE', filters: request.filters },
          },
        })),
        contentType: 'application/json',
      });
      return;
    }
    if (url.pathname === '/api/v1/query/filter-options') {
      const request = route.request().postDataJSON()?.payload ?? {};
      const options = request.field === 'COURSE_NUMBER_BAND'
        ? Array.from({ length: 20 }, (_, index) => {
            const band = index * 100;
            return {
              value: String(band),
              label: `${String(band).padStart(3, '0')}-level`,
            };
          })
        : request.field === 'COURSE_LEVEL'
          ? [{ value: 'U', label: 'Undergraduate' }, { value: 'G', label: 'Graduate' }]
          : request.field === 'KEYWORD'
            ? Array.from({ length: 20 }, (_, index) => ({
                value: `keyword-${String(index + 1).padStart(2, '0')}`,
                label: `Keyword ${String(index + 1).padStart(2, '0')}`,
              }))
          : [];
      await route.fulfill({
        body: JSON.stringify(success({
          contractVersion: 3,
          field: request.field,
          targetVersions: [],
          options,
          truncated: false,
        })),
        contentType: 'application/json',
      });
      return;
    }
    const response = routes.get(url.pathname);
    if (response === undefined) throw new Error(`Unmocked API route ${url.pathname}`);
    if (route.request().method() === 'POST') {
      requests.push({ path: url.pathname, body: route.request().postDataJSON() });
    }
    await route.fulfill({ body: JSON.stringify(success(response)), contentType: 'application/json' });
  });
}

async function assertNoOverflow(page, name) {
  const viewport = await page.evaluate(() => ({
    clientWidth: document.documentElement.clientWidth,
    scrollWidth: document.documentElement.scrollWidth,
  }));
  if (viewport.scrollWidth > viewport.clientWidth) {
    throw new Error(`${name}: horizontal overflow ${viewport.scrollWidth}/${viewport.clientWidth}`);
  }
}

async function assertPointerOnlyPressFeedback(page, name, requests) {
  const search = page.getByRole('button', { name: 'Search', exact: true });
  const queriesBeforeGestures = requests.filter(({ path }) => path === '/api/v1/query/courses').length;
  await search.scrollIntoViewIfNeeded();
  const reducedBox = await search.boundingBox();
  if (reducedBox === null) throw new Error(`${name}: Search button is not measurable in reduced motion`);
  await page.mouse.move(reducedBox.x + reducedBox.width / 2, reducedBox.y + reducedBox.height / 2);
  await page.mouse.down();
  const reducedPointerState = await search.evaluate((element) => ({
    pressed: element.getAttribute('data-pointer-pressed'),
    transform: getComputedStyle(element).transform,
  }));
  await page.mouse.move(reducedBox.x + reducedBox.width + 24, reducedBox.y + reducedBox.height / 2);
  await page.mouse.up();
  if (reducedPointerState.pressed !== 'true' || reducedPointerState.transform !== 'none') {
    throw new Error(`${name}: reduced-motion pointer press must retain zero transform ${JSON.stringify(reducedPointerState)}`);
  }
  if (await search.getAttribute('data-pointer-pressed') !== null) {
    throw new Error(`${name}: reduced-motion pointer state did not clear after pointerup`);
  }
  if (requests.filter(({ path }) => path === '/api/v1/query/courses').length !== queriesBeforeGestures) {
    throw new Error(`${name}: reduced-motion pointer cancellation unexpectedly submitted a query`);
  }

  await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'no-preference' });
  try {
    for (const key of [' ', 'Enter']) {
      await search.focus();
      await search.evaluate((element) => {
        element.addEventListener('click', (event) => event.preventDefault(), { once: true });
      });
      await page.keyboard.down(key);
      const keyboardTransform = await search.evaluate((element) => getComputedStyle(element).transform);
      await page.keyboard.up(key);
      if (keyboardTransform !== 'none') {
        throw new Error(`${name}: keyboard ${JSON.stringify(key)} triggered pointer displacement (${keyboardTransform})`);
      }
    }

    await search.scrollIntoViewIfNeeded();
    const box = await search.boundingBox();
    if (box === null) throw new Error(`${name}: Search button is not measurable`);
    await page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await page.mouse.down();
    const pointerState = await search.evaluate((element) => ({
      pressed: element.getAttribute('data-pointer-pressed'),
      transform: getComputedStyle(element).transform,
    }));
    await page.mouse.move(box.x + box.width + 24, box.y + box.height / 2);
    await page.mouse.up();
    if (pointerState.pressed !== 'true' || pointerState.transform !== 'matrix(1, 0, 0, 1, 0, 1)') {
      throw new Error(`${name}: pointer press feedback was not the required 1px translation ${JSON.stringify(pointerState)}`);
    }
    if (await search.getAttribute('data-pointer-pressed') !== null) {
      throw new Error(`${name}: pointer press state did not clear after pointerup`);
    }
    if (requests.filter(({ path }) => path === '/api/v1/query/courses').length !== queriesBeforeGestures) {
      throw new Error(`${name}: pointer or keyboard feedback gesture unexpectedly submitted a query`);
    }
  } finally {
    await page.emulateMedia({ colorScheme: 'light', reducedMotion: 'reduce' });
  }
}

async function assertStyledOptionScroll(page, name) {
  const bandGroup = page.locator('[role="group"][aria-label="Course number band"]');
  const scrollRegion = page.locator('.filter-panel__dictionary-options').filter({ has: bandGroup });
  const metrics = await scrollRegion.evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      clientHeight: element.clientHeight,
      clientWidth: element.clientWidth,
      overflowY: style.overflowY,
      scrollHeight: element.scrollHeight,
      scrollWidth: element.scrollWidth,
      scrollbarColor: style.scrollbarColor,
      scrollbarWidth: style.scrollbarWidth,
      touchAction: style.touchAction,
    };
  });
  if (metrics.overflowY !== 'auto'
    || metrics.scrollHeight <= metrics.clientHeight
    || metrics.scrollWidth > metrics.clientWidth + 1
    || metrics.scrollbarWidth !== 'thin'
    || metrics.scrollbarColor === 'auto'
    || metrics.touchAction !== 'pan-y') {
    throw new Error(`${name}: option scrollbar contract failed ${JSON.stringify(metrics)}`);
  }
  await scrollRegion.evaluate((element) => { element.scrollTop = 0; });
  await scrollRegion.hover();
  await page.mouse.wheel(0, 420);
  await page.waitForTimeout(50);
  if (await scrollRegion.evaluate((element) => element.scrollTop) <= 0) {
    throw new Error(`${name}: mouse wheel did not move the long option list`);
  }

  const bands = bandGroup.getByRole('checkbox');
  const count = await bands.count();
  await bands.first().focus();
  for (let index = 1; index < count; index += 1) await page.keyboard.press('Tab');
  const lastReachable = await bands.last().evaluate((element) => {
    const container = element.closest('.filter-panel__dictionary-options');
    if (!(container instanceof HTMLElement)) return false;
    const optionBox = element.getBoundingClientRect();
    const containerBox = container.getBoundingClientRect();
    return document.activeElement === element
      && optionBox.top >= containerBox.top - 1
      && optionBox.bottom <= containerBox.bottom + 1;
  });
  if (!lastReachable) throw new Error(`${name}: keyboard focus could not reach the last band`);
  await scrollRegion.hover();
  await page.mouse.wheel(0, -1);
  await scrollRegion.screenshot({
    animations: 'disabled',
    path: resolve(outputDirectory, 'option-scroll-styled-and-reachable.png'),
  });

  const keyword = page.getByRole('combobox', { name: 'Keyword match' });
  await keyword.focus();
  await page.getByRole('option', { name: 'Keyword 20' }).waitFor();
  await keyword.press('End');
  if (!/option-19$/u.test(await keyword.getAttribute('aria-activedescendant') ?? '')) {
    throw new Error(`${name}: End did not reach the last dictionary option`);
  }
  await keyword.press('Home');
  if (!/option-0$/u.test(await keyword.getAttribute('aria-activedescendant') ?? '')) {
    throw new Error(`${name}: Home did not reach the first dictionary option`);
  }
  await keyword.press('Escape');

  const subjectRegion = page.locator('.filter-panel__subject-list');
  const subjectMetrics = await subjectRegion.evaluate((element) => {
    const style = getComputedStyle(element);
    return {
      clientHeight: element.clientHeight,
      clientWidth: element.clientWidth,
      overflowY: style.overflowY,
      scrollHeight: element.scrollHeight,
      scrollWidth: element.scrollWidth,
      scrollbarColor: style.scrollbarColor,
      scrollbarWidth: style.scrollbarWidth,
      touchAction: style.touchAction,
    };
  });
  if (subjectMetrics.overflowY !== 'auto'
    || subjectMetrics.scrollHeight <= subjectMetrics.clientHeight
    || subjectMetrics.scrollWidth > subjectMetrics.clientWidth + 1
    || subjectMetrics.scrollbarWidth !== metrics.scrollbarWidth
    || subjectMetrics.scrollbarColor !== metrics.scrollbarColor
    || subjectMetrics.touchAction !== 'pan-y') {
    throw new Error(`${name}: subject scrollbar contract failed ${JSON.stringify(subjectMetrics)}`);
  }
  await subjectRegion.evaluate((element) => { element.scrollTop = 0; });
  await subjectRegion.hover();
  await page.mouse.wheel(0, 420);
  await page.waitForTimeout(50);
  if (await subjectRegion.evaluate((element) => element.scrollTop) <= 0) {
    throw new Error(`${name}: mouse wheel did not move the long subject list`);
  }

  const subjectInputs = subjectRegion.getByRole('checkbox');
  await subjectInputs.last().focus();
  const lastSubjectVisible = await subjectInputs.last().evaluate((element) => {
    const container = element.closest('.filter-panel__subject-list');
    if (!(container instanceof HTMLElement)) return false;
    const optionBox = element.getBoundingClientRect();
    const containerBox = container.getBoundingClientRect();
    return document.activeElement === element
      && optionBox.top >= containerBox.top - 1
      && optionBox.bottom <= containerBox.bottom + 1;
  });
  if (!lastSubjectVisible) throw new Error(`${name}: the last subject is clipped after keyboard focus`);
  await subjectRegion.hover();
  await page.mouse.wheel(0, -1);
  await subjectRegion.screenshot({
    animations: 'disabled',
    path: resolve(outputDirectory, 'subject-scroll-styled-and-reachable.png'),
  });

  await subjectRegion.scrollIntoViewIfNeeded();
  await subjectRegion.evaluate((element) => { element.scrollTop = 0; });
  const box = await subjectRegion.boundingBox();
  if (box === null) throw new Error(`${name}: subject touch region has no geometry`);
  const cdp = await page.context().newCDPSession(page);
  const x = box.x + Math.min(box.width - 12, 60);
  const startY = box.y + box.height - 16;
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
  if (await subjectRegion.evaluate((element) => element.scrollTop) <= 0) {
    throw new Error(`${name}: touch pan did not move the long subject list`);
  }
}

async function settleVisualLayout(page) {
  await page.evaluate(async () => {
    globalThis.scrollTo(0, 0);
    await new Promise((resolveFrame) => requestAnimationFrame(() => requestAnimationFrame(resolveFrame)));
  });
}

async function applyNewBrunswickScope(page, name) {
  const scope = page.locator('.query-scope[data-scope-layout="local-2x5"]');
  await scope.waitFor();
  await scope.locator('[data-scope-cell="campus-NB"] input').check();
  await scope.getByRole('button', { name: 'Apply', exact: true }).click();
  await scope.getByRole('button', { name: 'Applied', exact: true }).waitFor();
  if (!await scope.getByRole('button', { name: 'Applied', exact: true }).isDisabled()) {
    throw new Error(`${name}: applied scope was not rendered as a disabled current state`);
  }
}

async function assertPlainEnglishFilterLabels(page, name) {
  for (const label of [
    'Course level',
    'Prerequisites',
    'Class format',
    'Meeting timing',
    'In person',
    'Synchronous',
    'Asynchronous',
  ]) {
    if (await page.getByText(label, { exact: true }).count() === 0) {
      throw new Error(`${name}: missing plain-language filter label ${JSON.stringify(label)}`);
    }
  }
}

async function assertSectionFactGrid(page, name) {
  const sectionFields = page.locator('.search-results__field-list').last();
  const permissionField = sectionFields.locator('.search-results__field--full-row');
  if (await permissionField.count() !== 1) {
    throw new Error(`${name}: Section permission fact is not explicitly assigned a full row`);
  }
  const geometry = await sectionFields.evaluate((list) => {
    const field = list.querySelector('.search-results__field--full-row');
    if (!(field instanceof HTMLElement)) return null;
    const listBox = list.getBoundingClientRect();
    const fieldBox = field.getBoundingClientRect();
    return {
      fieldLeft: fieldBox.left,
      fieldRight: fieldBox.right,
      listLeft: listBox.left,
      listRight: listBox.right,
    };
  });
  if (geometry === null
    || Math.abs(geometry.fieldLeft - geometry.listLeft) > 1
    || Math.abs(geometry.fieldRight - geometry.listRight) > 1) {
    throw new Error(`${name}: Section permission fact left an empty grid cavity ${JSON.stringify(geometry)}`);
  }
}

await mkdir(outputDirectory, { recursive: true });
const browser = await chromium.launch({
  ...(executablePath === undefined ? {} : { executablePath }),
  args: ['--disable-gpu', '--disable-features=OverlayScrollbar,FluentOverlayScrollbar'],
  headless: true,
});

const warmupPage = await browser.newPage({ viewport: { width: 320, height: 240 } });
await warmupPage.setContent('<style>html,body{margin:0;background:#efeee8}</style>');
await warmupPage.screenshot();
await warmupPage.close();

const flowViewports = [
  { name: '390', width: 390, height: 844 },
  { name: '768', width: 768, height: 1024 },
  { name: '1440', width: 1440, height: 1200 },
  { name: '1920', width: 1920, height: 1200 },
  { name: '2560', width: 2560, height: 1280 },
  { name: '320-stress', width: 320, height: 568 },
];

try {
  for (const viewport of flowViewports) {
    const name = `course-results-${viewport.name}`;
    const requests = [];
    const page = await browser.newPage({
      colorScheme: 'light', hasTouch: viewport.width === 390, locale: 'en-US', reducedMotion: 'reduce', viewport,
    });
    await installApi(page, requests);
    await page.goto(`${baseUrl}/local.html`, { waitUntil: 'domcontentloaded' });
    await page.getByRole('heading', { name: 'Search courses' }).waitFor();
    await applyNewBrunswickScope(page, name);
    await assertPlainEnglishFilterLabels(page, name);
    if (viewport.width === 390) await assertStyledOptionScroll(page, name);
    const bandGroup = page.locator('[role="group"][aria-label="Course number band"]');
    await bandGroup.getByRole('checkbox', { name: '200-level', exact: true }).check();
    await page.getByRole('group', { name: 'Open status' }).getByRole('checkbox', { name: 'Open' }).check();
    if (viewport.width === 1440) await assertPointerOnlyPressFeedback(page, name, requests);
    const queriesBeforeSearch = requests.filter(({ path }) => path === '/api/v1/query/courses').length;
    await page.getByRole('button', { name: 'Search', exact: true }).click();
    await page.getByRole('heading', { name: '01:198:211' }).waitFor();
    const queriesAfterSearch = requests.filter(({ path }) => path === '/api/v1/query/courses').length;
    if (queriesAfterSearch !== queriesBeforeSearch + 1) {
      throw new Error(`${name}: explicit Search submitted ${queriesAfterSearch - queriesBeforeSearch} course queries instead of one`);
    }
    const query = requests.find(({ path }) => path === '/api/v1/query/courses');
    if (query?.body?.payload?.filters?.contractVersion !== 3
      || query.body.payload.filters.values.courseNumberBands?.[0] !== 200
      || query.body.payload.filters.values.openStatuses?.[0] !== 'OPEN'
      || query.body.payload.filters.values.includeIncomplete?.modality !== false) {
      throw new Error(`${name}: Course query did not preserve the combined filters`);
    }
    const disclosures = page.locator('details.search-results__section-disclosure');
    if (await disclosures.count() !== 1 || await disclosures.first().getAttribute('open') !== null) {
      throw new Error(`${name}: Course Sections must exist and remain collapsed by default`);
    }
    await settleVisualLayout(page);
    await assertNoOverflow(page, name);
    await page.screenshot({
      animations: 'disabled',
      fullPage: viewport.width < 800,
      path: resolve(outputDirectory, `${name}.png`),
    });

    if (viewport.width === 1440) {
      await page.getByText('Show 1 Sections', { exact: true }).click();
      await page.getByRole('link', { name: 'Open section' }).click();
      await page.getByRole('heading', { name: /01:198:211.*12345/u }).waitFor();
      if (new URL(page.url()).pathname !== `/sections/${currentTerm}/NB/12345`) {
        throw new Error(`direct Section navigation did not preserve the canonical URL: ${page.url()}`);
      }
      const detail = requests.find(({ path }) => path === '/api/v1/query/section-detail');
      if (detail?.body?.payload?.key?.index !== '12345') {
        throw new Error('direct Section route did not load typed detail');
      }
      await assertSectionFactGrid(page, 'section-detail-desktop');
      await settleVisualLayout(page);
      await assertNoOverflow(page, 'section-detail-desktop');
      await page.screenshot({
        animations: 'disabled',
        fullPage: false,
        path: resolve(outputDirectory, 'section-detail-desktop.png'),
      });
    }
    await page.close();
  }
} finally {
  await browser.close();
}

const flowSnapshotCount = flowViewports.length + 1;
process.stdout.write(`Course and section flow snapshots: PASS (${flowSnapshotCount}/${flowSnapshotCount})\n`);
