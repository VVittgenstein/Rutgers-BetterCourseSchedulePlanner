import { mkdir, readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import { chromium } from 'playwright';
import { localBootstrapFixture } from './local-bootstrap-fixture.mjs';

const baseUrl = process.env.BCSP_VISUAL_BASE_URL ?? 'http://127.0.0.1:4173';
const executablePath = process.env.BCSP_BROWSER_EXECUTABLE
  ?? (process.platform === 'win32'
    ? 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe'
    : undefined);
const outputDirectory = resolve(process.cwd(), '../project-governance/current/p7/evidence/p7-2-002');
const filterSchema = JSON.parse(await readFile(
  resolve(process.cwd(), '../crates/bcsp-contracts/tests/golden/filter-schema-v1.json'),
  'utf8',
));
const observedAt = '2026-07-15T03:09:30.000Z';
const known = (value) => ({ knowledge: 'KNOWN', presence: { presence: 'PRESENT', value } });
const success = (data) => ({ protocolVersion: 1, data });
const groupKey = { campus: 'NB', courseString: '01:198:211', term: '2026-9' };
const sectionKey = { campus: 'NB', index: '12345', term: '2026-9' };
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
  subjects: Array.from({ length: 300 }, (_, index) => {
    const code = index === 198 ? '198' : `S${String(index).padStart(3, '0')}`;
    return {
      code,
      label: known(index === 198 ? 'Computer Science' : `Published subject ${index}`),
      provenance: { kind: 'DISCOVERY', discovery: provenance },
      target: { campus: 'NB', term: '2026-9' },
    };
  }),
  targets: [
    {
      campusLabel: known('New Brunswick'),
      key: { campus: 'NB', term: '2026-9' },
      provenance,
      termLabel: known('Fall 2026'),
    },
    {
      campusLabel: known('Newark'),
      key: { campus: 'NK', term: '2026-9' },
      provenance,
      termLabel: known('Fall 2026'),
    },
  ],
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
  contractVersion: 1,
  items: [courseItem],
  page: { page: 1, pageSize: 25, total: 1, totalPages: 1 },
};
const sectionResponse = {
  contractVersion: 1,
  items: [{ courseFilterMatches: [], section: sectionItem, textMatch: null, variant }],
  page: { page: 1, pageSize: 25, total: 1, totalPages: 1 },
};

async function installApi(page, requests) {
  const routes = new Map([
    ['/api/v1/local/bootstrap', localBootstrapFixture()],
    ['/api/v1/query/filter-schema', filterSchema],
    ['/api/v1/catalog/discovery', discovery],
    ['/api/v1/query/courses', courseResponse],
    ['/api/v1/query/sections', sectionResponse],
    ['/api/v1/query/course-detail', { contractVersion: 1, course: courseItem }],
    ['/api/v1/query/section-detail', { contractVersion: 1, section: sectionItem, variant }],
  ]);
  await page.route('**/api/v1/**', async (route) => {
    const url = new URL(route.request().url());
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

async function settleVisualLayout(page) {
  await page.evaluate(async () => {
    globalThis.scrollTo(0, 0);
    const filters = document.querySelector('.bcsp-search-workspace__filters');
    if (filters instanceof HTMLElement) filters.scrollTop = 0;
    await new Promise((resolveFrame) => requestAnimationFrame(() => requestAnimationFrame(resolveFrame)));
  });
}

await mkdir(outputDirectory, { recursive: true });
const browser = await chromium.launch({
  ...(executablePath === undefined ? {} : { executablePath }),
  args: ['--disable-gpu'],
  headless: true,
});

const warmupPage = await browser.newPage({ viewport: { width: 320, height: 240 } });
await warmupPage.setContent('<style>html,body{margin:0;background:#efeee8}</style>');
await warmupPage.screenshot();
await warmupPage.close();

try {
  for (const viewport of [
    { name: 'course-results-desktop', width: 1440, height: 1600 },
    { name: 'course-results-mobile', width: 390, height: 844 },
  ]) {
    const requests = [];
    const page = await browser.newPage({
      colorScheme: 'light', locale: 'en-US', reducedMotion: 'reduce', viewport,
    });
    await installApi(page, requests);
    await page.goto(`${baseUrl}/local.html`, { waitUntil: 'domcontentloaded' });
    await page.getByRole('heading', { name: 'Search courses' }).waitFor();
    await page.getByLabel('Search text').fill('data structures');
    await page.getByText('Same-Section constraints', { exact: true }).click();
    await page.getByRole('group', { name: 'Open status' }).getByRole('checkbox', { name: 'Open' }).check();
    await page.getByRole('button', { name: 'Search', exact: true }).click();
    await page.getByRole('heading', { name: '01:198:211' }).waitFor();
    const query = requests.find(({ path }) => path === '/api/v1/query/courses');
    if (query?.body?.payload?.filters?.values?.text !== 'data structures'
      || query.body.payload.filters.values.openStatuses?.[0] !== 'OPEN') {
      throw new Error(`${viewport.name}: Course query did not preserve combined filters`);
    }
    await settleVisualLayout(page);
    await assertNoOverflow(page, viewport.name);
    if (viewport.width >= 800) await page.screenshot({ animations: 'disabled' });
    await page.screenshot({
      animations: 'disabled',
      fullPage: viewport.width < 800,
      path: resolve(outputDirectory, `${viewport.name}.png`),
    });
    await page.close();
  }

  for (const viewport of [
    { name: 'section-results-desktop', width: 1440, height: 1600 },
    { name: 'section-results-mobile', width: 390, height: 844 },
  ]) {
    const requests = [];
    const page = await browser.newPage({
      colorScheme: 'light', locale: 'en-US', reducedMotion: 'reduce', viewport,
    });
    await installApi(page, requests);
    await page.goto(`${baseUrl}/local.html`, { waitUntil: 'domcontentloaded' });
    await page.getByRole('link', { name: /Sections/u }).click();
    await page.getByRole('heading', { name: 'Search Sections' }).waitFor();
    await page.getByRole('textbox', { name: 'Section indexes', exact: true }).fill('12345');
    await page.getByRole('button', { name: 'Add Section indexes' }).click();
    await page.getByRole('button', { name: 'Search', exact: true }).click();
    await page.getByRole('heading', { name: /01:198:211 · Data Structures/u }).waitFor();
    const query = requests.find(({ path }) => path === '/api/v1/query/sections');
    if (query?.body?.payload?.filters?.values?.sectionIndexes?.[0] !== '12345') {
      throw new Error(`${viewport.name}: Section query lost its direct index filter`);
    }
    await settleVisualLayout(page);
    await assertNoOverflow(page, viewport.name);
    if (viewport.width >= 800) await page.screenshot({ animations: 'disabled' });
    await page.screenshot({
      animations: 'disabled',
      fullPage: viewport.width < 800,
      path: resolve(outputDirectory, `${viewport.name}.png`),
    });

    if (viewport.name === 'section-results-desktop') {
      await page.getByRole('link', { name: 'Open section' }).click();
      await page.getByRole('heading', { name: /01:198:211 · 12345/u }).waitFor();
      if (new URL(page.url()).pathname !== '/sections/2026-9/NB/12345') {
        throw new Error(`direct Section navigation did not preserve the canonical URL: ${page.url()}`);
      }
      const detail = requests.find(({ path }) => path === '/api/v1/query/section-detail');
      if (detail?.body?.payload?.key?.index !== '12345') {
        throw new Error('direct Section route did not load typed detail');
      }
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

process.stdout.write('P7.2-002 Course/Section flow snapshots: PASS (5/5)\n');
