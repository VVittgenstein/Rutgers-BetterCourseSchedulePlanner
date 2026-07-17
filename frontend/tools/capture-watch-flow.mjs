import { mkdir, readFile } from 'node:fs/promises';
import { resolve } from 'node:path';

import { chromium } from 'playwright';
import { localBootstrapFixture } from './local-bootstrap-fixture.mjs';

const baseUrl = process.env.BCSP_VISUAL_BASE_URL ?? 'http://127.0.0.1:4173';
const executablePath = process.env.BCSP_BROWSER_EXECUTABLE
  ?? (process.platform === 'win32'
    ? 'C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe'
    : undefined);
const outputDirectory = resolve(process.cwd(), '../project-governance/current/p7/evidence/p7-2-003');
const filterSchema = JSON.parse(await readFile(
  resolve(process.cwd(), '../crates/bcsp-contracts/tests/golden/filter-schema-v1.json'),
  'utf8',
));

const observedAt = '2030-01-15T14:00:00.000Z';
const freshUntil = '2030-01-15T14:00:45.000Z';
const success = (data) => ({ protocolVersion: 1, data });
const known = (value) => ({ knowledge: 'KNOWN', presence: { presence: 'PRESENT', value } });
const groupKey = { campus: 'NB', courseString: '01:198:211', term: '2026-9' };
const variantKey = { fingerprint: 'synthetic-watch-visual', group: groupKey };
const sectionKeys = ['00001', '00003', '00004'].map((index) => ({
  campus: 'NB',
  index,
  term: '2026-9',
}));

const point = {
  contentVersion: 11,
  observationId: '10000000-0000-4000-8000-000000000011',
  observedAt,
};
const provenance = {
  observationId: point.observationId,
  observedAt,
  payloadDigest: 'a'.repeat(64),
  sourceId: 'synthetic-selector',
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

const variant = {
  key: variantKey,
  title: known('Synthetic Systems Studio'),
  expandedTitle: known('Synthetic Systems Studio — Watch Acceptance Fixture'),
  description: known('Deterministic browser-only fixture for watch, alert, and freshness validation.'),
  notes: known('No real course or upstream payload is used.'),
  subjectGroupNotes: known('Synthetic visual evidence'),
  subjectNotes: known('Computer Science'),
  unitNotes: known('Synthetic unit'),
  synopsisUrl: known('https://example.invalid/synthetic-watch'),
  prerequisiteNotes: known('None for this fixture'),
  prerequisiteState: 'NONE_REPORTED',
  credits: known('3.0'),
  level: known('UNDERGRADUATE'),
  subjectCode: known('198'),
  subjectDescription: known('Computer Science'),
  courseNumber: known('211'),
  supplementCode: known(''),
  schoolCode: known('01'),
  offeringUnit: known('198'),
  offeringUnitTitle: known('Computer Science'),
  coreCodes: known([]),
  campusLocations: known(['NB']),
  sectionKeys,
};

function makeSection(sectionKey, ordinal) {
  return {
    key: sectionKey,
    variantKey,
    sectionNumber: known(String(ordinal).padStart(2, '0')),
    subtitle: known(`Synthetic watch Section ${sectionKey.index}`),
    subtopic: known('Browser watch acceptance'),
    sectionNotes: known('Synthetic data only'),
    sessionDates: known('09/01-12/14'),
    sessionDatePrintIndicator: known('Y'),
    comments: known([]),
    commentsText: known('No comments'),
    crossListedSectionsText: known('None'),
    crossListedSectionType: known('NONE'),
    instructors: known([`Synthetic Instructor ${ordinal}`]),
    instructorReliability: 'NAME_ONLY',
    rawSectionCourseType: known('LEC'),
    deliveryModality: 'ON_CAMPUS_OR_IN_PERSON',
    synchronicity: 'SYNC',
    examCode: known('A'),
    examCodeText: known('Synthetic exam code'),
    specialPermissionAddCode: known('N'),
    specialPermissionAddDescription: known('No permission required'),
    specialPermissionDropCode: known('N'),
    specialPermissionDropDescription: known('No permission required'),
    majorCodes: known([]),
    unitCodes: known([]),
    minorCodes: known([]),
    honorProgramCodes: known([]),
    unitMajors: known([]),
    eligibilityText: known('Open to synthetic fixture users'),
    openToText: known('All fixture users'),
    catalogOpenStatus: { provenance: 'CATALOG_SNAPSHOT_ONLY', value: known(true) },
    occurrenceKeys: known([{ ordinal: 1, section: sectionKey }]),
  };
}

function makeOccurrence(sectionKey, ordinal) {
  return {
    key: { ordinal: 1, section: sectionKey },
    rawCode: known('LEC'),
    rawDescription: known('Lecture'),
    modality: known('ON_CAMPUS_OR_IN_PERSON'),
    synchronicity: known('SYNC'),
    rawDay: known('MW'),
    days: known(['MONDAY', 'WEDNESDAY']),
    rawStartTime: known(`${9 + ordinal}:00 AM`),
    rawEndTime: known(`${10 + ordinal}:00 AM`),
    time: { endMinute: 600 + ordinal * 60, knowledge: 'KNOWN', startMinute: 540 + ordinal * 60 },
    startDate: known('2026-09-01'),
    endDate: known('2026-12-14'),
    campus: known('NB'),
    campusName: known('New Brunswick'),
    building: known('SYN'),
    room: known(String(100 + ordinal)),
    requiredness: 'REQUIRED',
    kind: 'SCHEDULED',
    evidence: 'PHYSICAL',
    normalizationReason: 'SYNTHETIC_FIXTURE',
  };
}

const sectionItems = sectionKeys.map((sectionKey, index) => ({
  section: makeSection(sectionKey, index + 1),
  occurrences: [makeOccurrence(sectionKey, index + 1)],
  open: {
    freshUntil,
    observedAt,
    state: 'OPEN',
    uncertainty: null,
  },
  explanation: { outcome: 'MATCH', reasons: [] },
  filterMatches: [],
}));
const courseItem = {
  explanation: { outcome: 'MATCH', reasons: [] },
  group: { key: groupKey, variantKeys: [variantKey] },
  variants: [{
    explanation: { outcome: 'MATCH', reasons: [] },
    filterMatches: [],
    sections: sectionItems,
    textMatch: { exactCourseIdentifier: false, matchedTokens: ['synthetic'] },
    variant,
  }],
};
const courseResponse = {
  contractVersion: 3,
  items: [courseItem],
  page: { page: 1, pageSize: 25, total: 1, totalPages: 1 },
};

function counters() {
  return {
    runCounts: { attempted: 12, succeeded: 9, failed: 2, empty: 1 },
    todayCounts: { attempted: 42, succeeded: 36, failed: 4, empty: 2 },
    rutgersDay: '2030-01-15',
    dayTimezone: 'America/New_York',
  };
}

function batchStatus(profile) {
  const unknown = profile === 'UNKNOWN';
  const stale = profile === 'STALE';
  return {
    contractVersion: 1,
    batch: { term: '2026-9', campus: 'NB' },
    catalogContentVersion: unknown ? null : 11,
    latestAttempt: {
      attemptSequence: 12,
      startedAt: observedAt,
      completedAt: observedAt,
      classification: profile === 'FRESH' ? 'VALID_APPLIED' : 'FAILED',
      failureClass: profile === 'FRESH' ? null : stale ? 'HTTP_429' : 'NON_JSON',
    },
    latestFailure: profile === 'FRESH' ? null : {
      attemptSequence: 12,
      failedAt: observedAt,
      failureClass: stale ? 'HTTP_429' : 'NON_JSON',
    },
    lastValidObservation: unknown ? null : {
      observationId: '20000000-0000-4000-8000-000000000011',
      observationSequence: 9,
      catalogContentVersion: 11,
      observedAt,
      canonicalSetHash: 'b'.repeat(64),
      stateHash: 'c'.repeat(64),
    },
    lastBodyChangeAt: unknown ? null : observedAt,
    lastStateChangeAt: unknown ? null : observedAt,
    freshness: {
      state: profile,
      observedAt: unknown ? null : observedAt,
      freshUntil: profile === 'FRESH' ? freshUntil : null,
      lastKnownGoodAgeSeconds: unknown ? null : stale ? 125 : 0,
      uncertainty: profile === 'FRESH' ? null : stale ? 'LATEST_ATTEMPT_FAILED' : 'NEVER_OBSERVED',
    },
    scheduler: {
      lane: profile === 'FRESH' ? 'ACTIVE_WATCH' : 'GENERAL',
      requestedGeneralIntervalSeconds: 30,
      requestedEffectiveIntervalSeconds: profile === 'FRESH' ? 10 : 30,
      activeWatchCount: profile === 'FRESH' ? 1 : 0,
      nextDueAt: freshUntil,
      inFlight: false,
      schedulerLagMilliseconds: unknown ? 1744 : stale ? 902 : 487,
      actualStartToStartIntervalMilliseconds: unknown ? null : stale ? 31820 : 10487,
      failureStreak: profile === 'FRESH' ? 0 : 2,
    },
    circuit: {
      state: profile === 'FRESH' ? 'CLOSED' : stale ? 'RETRY_AFTER' : 'FATAL_DIAGNOSTIC',
      reason: profile === 'FRESH' ? null : stale ? 'RATE_LIMITED' : 'NON_JSON',
      openedAt: profile === 'FRESH' ? null : observedAt,
      retryAt: stale ? freshUntil : null,
      diagnosticRecheckRequired: unknown,
    },
    counterSnapshot: counters(),
  };
}

function sectionStatus(sectionKey, profile) {
  const unknown = profile === 'UNKNOWN';
  const stale = profile === 'STALE';
  return {
    contractVersion: 1,
    sectionKey,
    state: unknown ? 'UNKNOWN' : stale ? 'CLOSED' : 'OPEN',
    lastObservationId: unknown ? null : `30000000-0000-4000-8000-${sectionKey.index.padStart(12, '0')}`,
    catalogContentVersion: unknown ? null : 11,
    freshness: {
      state: profile,
      observedAt: unknown ? null : observedAt,
      freshUntil: profile === 'FRESH' ? freshUntil : null,
      lastKnownGoodAgeSeconds: unknown ? null : stale ? 125 : 0,
      uncertainty: profile === 'FRESH' ? null : stale ? 'STALE_LAST_KNOWN_GOOD' : 'NEVER_OBSERVED',
    },
    schedulerLagMilliseconds: unknown ? 1744 : stale ? 902 : 487,
    counterSnapshot: counters(),
  };
}

async function installApi(page, scenario) {
  const routes = new Map([
    ['/api/v1/local/bootstrap', localBootstrapFixture()],
    ['/api/v1/query/filter-schema', filterSchema],
    ['/api/v1/catalog/discovery', discovery],
    ['/api/v1/query/courses', courseResponse],
    ['/api/v1/query/course-detail', { contractVersion: 3, course: courseItem }],
  ]);
  await page.route('**/api/v1/**', async (route) => {
    const url = new URL(route.request().url());
    let response = routes.get(url.pathname);
    if (url.pathname === '/api/v1/open/status') response = batchStatus(scenario.profile);
    if (url.pathname === '/api/v1/open/section-status') {
      const request = route.request().postDataJSON();
      response = sectionStatus(request.payload.sectionKey, scenario.profile);
    }
    if (response === undefined) throw new Error(`${scenario.name}: unmocked API route ${url.pathname}`);
    await route.fulfill({
      body: JSON.stringify(success(response)),
      contentType: 'application/json',
    });
  });
}

async function installBrowserFakes(page, scenario) {
  await page.addInitScript(({ audioBlocked }) => {
    const syntheticCounts = {
      runCounts: { attempted: 12, succeeded: 9, failed: 2, empty: 1 },
      todayCounts: { attempted: 42, succeeded: 36, failed: 4, empty: 2 },
      rutgersDay: '2030-01-15',
      dayTimezone: 'America/New_York',
    };
    globalThis.__bcspWatchVisual = { audioTrace: [], commands: [], frames: [] };

    class FakeParam {
      setValueAtTime() {}
      linearRampToValueAtTime() {}
      exponentialRampToValueAtTime() {}
    }
    class FakeNode {
      connect(destination) { return destination; }
      disconnect() {}
    }
    class FakeGain extends FakeNode {
      gain = new FakeParam();
    }
    class FakeOscillator extends FakeNode {
      frequency = new FakeParam();
      onended = null;
      type = 'sine';
      start() { globalThis.__bcspWatchVisual.audioTrace.push('oscillator:start'); }
      stop() { queueMicrotask(() => this.onended?.()); }
    }
    class FakeAudioContext {
      constructor() {
        globalThis.__bcspWatchVisual.audioTrace.push('construct');
        this.currentTime = 0;
        this.destination = new FakeNode();
        this.state = 'suspended';
      }
      async resume() {
        globalThis.__bcspWatchVisual.audioTrace.push('resume');
        if (audioBlocked) {
          const error = new Error('Synthetic autoplay block');
          error.name = 'NotAllowedError';
          globalThis.__bcspWatchVisual.audioTrace.push(`reject:${error.name}`);
          throw error;
        }
        this.state = 'running';
      }
      async close() { this.state = 'closed'; }
      createGain() { return new FakeGain(); }
      createOscillator() { return new FakeOscillator(); }
    }

    class FakeWebSocket {
      static CONNECTING = 0;
      static OPEN = 1;
      static CLOSING = 2;
      static CLOSED = 3;
      static instances = [];
      readyState = FakeWebSocket.CONNECTING;
      listeners = new Map();

      constructor(url, protocols) {
        this.url = url;
        this.protocols = protocols;
        FakeWebSocket.instances.push(this);
        queueMicrotask(() => {
          this.readyState = FakeWebSocket.OPEN;
          this.emit('open');
        });
      }

      addEventListener(type, listener) {
        const listeners = this.listeners.get(type) ?? new Set();
        listeners.add(listener);
        this.listeners.set(type, listeners);
      }

      close() {
        this.readyState = FakeWebSocket.CLOSED;
        this.emit('close');
      }

      send(data) {
        const envelope = JSON.parse(data);
        globalThis.__bcspWatchVisual.commands.push(envelope.payload);
        if (envelope.payload.type !== 'START_WATCH') return;
        setTimeout(() => this.startWatch(envelope.payload.items), 25);
      }

      emit(type, event = {}) {
        this.listeners.get(type)?.forEach((listener) => listener(event));
      }

      server(payload, messageId) {
        globalThis.__bcspWatchVisual.frames.push({ messageId, type: payload.type });
        this.emit('message', {
          data: JSON.stringify({ protocolVersion: 1, messageId, payload }),
        });
      }

      startWatch(items) {
        const siblingMessageId = '90000000-0000-4000-8000-000000000001';
        const active = items.map((item, index) => ({
          ...item,
          activeWatchId: `80000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
          episodeId: `81000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
          alertId: `82000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
          observationId: `83000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
        }));
        this.server({
          type: 'START_RESULT',
          result: {
            contractVersion: 1,
            activeWatchCount: active.length,
            items: active.map((item) => ({
              status: 'ACTIVE',
              sectionKey: item.sectionKey,
              activeWatchId: item.activeWatchId,
              startedAt: '2030-01-15T14:00:00.000Z',
            })),
          },
        }, siblingMessageId);
        for (const item of active) {
          const episode = {
            contractVersion: 1,
            episodeId: item.episodeId,
            activeWatchId: item.activeWatchId,
            sectionKey: item.sectionKey,
            state: 'UNACKNOWLEDGED',
            notificationMode: item.policy.notificationMode,
            continuousDuration: item.policy.continuousDuration,
            maxAudible: item.policy.maxAudible,
            audibleCount: item.policy.notificationMode === 'ONE_SHOT' ? 1 : 0,
            firstObservedAt: '2030-01-15T14:00:01.000Z',
            lastObservedAt: '2030-01-15T14:00:01.000Z',
            observationCount: 1,
            latestObservationId: item.observationId,
            stateChangedAt: '2030-01-15T14:00:01.000Z',
            closedAt: null,
          };
          this.server({
            type: 'OPEN_OBSERVATION',
            fanout: {
              contractVersion: 1,
              activeWatchId: item.activeWatchId,
              observation: {
                contractVersion: 1,
                observationId: item.observationId,
                refreshObservationId: '84000000-0000-4000-8000-000000000001',
                batch: { term: item.sectionKey.term, campus: item.sectionKey.campus },
                sectionKey: item.sectionKey,
                pullSequence: 12,
                catalogContentVersion: 11,
                state: 'OPEN',
                observedAt: '2030-01-15T14:00:01.000Z',
                freshUntil: '2030-01-15T14:00:45.000Z',
                schedulerLagMilliseconds: 487,
                counterSnapshot: syntheticCounts,
              },
            },
          }, siblingMessageId);
          this.server({ type: 'EPISODE_UPDATED', episode }, siblingMessageId);
          this.server({
            type: 'ALERT_UPDATED',
            alert: {
              contractVersion: 1,
              alertId: item.alertId,
              disposition: 'OPENED',
              visible: true,
              episode,
            },
          }, siblingMessageId);
          if (item.policy.notificationMode === 'ONE_SHOT') {
            this.server({
              type: 'AUDIO_DISPOSITION',
              audio: {
                disposition: 'CUE_REQUESTED',
                cue: {
                  cueId: `85000000-0000-4000-8000-${item.sectionKey.index.padStart(12, '0')}`,
                  activeWatchId: item.activeWatchId,
                  sectionKey: item.sectionKey,
                  trigger: { kind: 'ONE_SHOT_OBSERVATION', observationId: item.observationId },
                  emittedAt: '2030-01-15T14:00:01.000Z',
                },
              },
            }, siblingMessageId);
          }
        }
        const continuousIds = active
          .filter((item) => item.policy.notificationMode === 'CONTINUOUS')
          .map((item) => item.episodeId);
        if (continuousIds.length > 0) {
          this.server({
            type: 'AUDIO_DISPOSITION',
            audio: {
              disposition: 'CONTINUOUS_MIXER_ACTIVE',
              episodeIds: continuousIds,
              emittedAt: '2030-01-15T14:00:01.000Z',
            },
          }, siblingMessageId);
        }
      }
    }

    globalThis.AudioContext = FakeAudioContext;
    globalThis.WebSocket = FakeWebSocket;
  }, { audioBlocked: scenario.audioBlocked === true });
}

async function selectSyntheticSections(page, count) {
  await page.getByRole('heading', { name: 'Search courses' }).waitFor();
  await page.getByLabel('Search text').fill('synthetic systems');
  await page.getByRole('button', { name: 'Search', exact: true }).click();
  await page.getByRole('heading', { name: '01:198:211' }).waitFor();
  for (const sectionKey of sectionKeys.slice(0, count)) {
    await page.getByRole('button', {
      name: `Select Section ${sectionKey.index} for watch`,
    }).click();
  }
  await page.getByRole('link', { name: /Watch desk/u }).click();
  await page.getByRole('heading', { name: 'Watch desk' }).first().waitFor();
  await page.getByLabel(`${count} of 9 selected`).waitFor();
}

async function startWatch(page, scenario) {
  if (scenario.continuous === true) {
    await page.getByRole('radio', { name: 'Continuous episode alarm' }).check();
    if (scenario.unlimited === true) {
      await page.getByRole('combobox', { name: 'Continuous duration' }).selectOption('UNLIMITED');
    }
    await page.getByRole('checkbox', {
      name: /I understand that CONTINUOUS sound persists/u,
    }).check();
  }
  await page.getByRole('button', { name: /Start selected/u }).click();
  await page.getByText('WATCHING', { exact: true }).first().waitFor();
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
    await document.fonts.ready;
    await new Promise((resolveFrame) => requestAnimationFrame(() => requestAnimationFrame(resolveFrame)));
  });
}

async function assertSharedMessageSiblings(page, name) {
  const frames = await page.evaluate(() => globalThis.__bcspWatchVisual.frames);
  const byMessage = new Map();
  for (const frame of frames) {
    const types = byMessage.get(frame.messageId) ?? new Set();
    types.add(frame.type);
    byMessage.set(frame.messageId, types);
  }
  const siblingTypes = [...byMessage.values()].find((types) =>
    types.has('START_RESULT') && types.has('EPISODE_UPDATED') && types.has('ALERT_UPDATED'));
  if (siblingTypes === undefined) {
    throw new Error(`${name}: Fake WebSocket did not emit sibling events with one messageId`);
  }
}

async function assertScenario(page, scenario) {
  await page.getByLabel('Watch session status').waitFor();
  await page.getByRole('heading', { name: 'Freshness / lag / circuit / counters' }).waitFor();
  await page.getByRole('status').filter({ hasText: /Audio /u }).waitFor();
  if (scenario.name.startsWith('active-')) {
    await page.getByRole('heading', { name: /Section 00001 .* UNACKNOWLEDGED/u }).waitFor();
    await page.locator('.watch-telemetry__batch[data-freshness="FRESH"]').waitFor();
    await assertSharedMessageSiblings(page, scenario.name);
  } else if (scenario.name === 'continuous-acd-desktop') {
    await page.getByText(/mixer 3 episodes/u).waitFor();
    for (const index of ['00001', '00003', '00004']) {
      await page.getByRole('heading', { name: new RegExp(`Section ${index} .* UNACKNOWLEDGED`, 'u') }).waitFor();
    }
    const alerts = await page.locator('.watch-workspace__alert').count();
    if (alerts !== 3) throw new Error(`${scenario.name}: expected 3 A/C/D alerts, received ${alerts}`);
    const audioTrace = await page.evaluate(() => globalThis.__bcspWatchVisual.audioTrace);
    if (!audioTrace.includes('oscillator:start')) {
      throw new Error(`${scenario.name}: mixer was visible but continuous browser audio never started`);
    }
    await assertSharedMessageSiblings(page, scenario.name);
  } else if (scenario.name === 'audio-blocked-desktop') {
    await page.waitForTimeout(250);
    const audioStatus = await page.getByRole('status').filter({ hasText: /Audio /u }).textContent();
    if (!audioStatus?.includes('Audio BLOCKED')) {
      const notices = await page.locator('.watch-toast').allTextContents();
      const audioDebug = await page.evaluate(() => ({
        audioContextName: globalThis.AudioContext?.name,
        trace: globalThis.__bcspWatchVisual.audioTrace,
      }));
      throw new Error(`${scenario.name}: expected Audio BLOCKED, received ${JSON.stringify({ audioStatus, notices, audioDebug })}`);
    }
    const audioTrace = await page.evaluate(() => globalThis.__bcspWatchVisual.audioTrace);
    if (!audioTrace.includes('reject:NotAllowedError')) {
      throw new Error(`${scenario.name}: autoplay-block rejection was not exercised`);
    }
    await page.getByRole('alert').filter({ hasText: 'Browser blocked sound' }).first().waitFor();
    await assertSharedMessageSiblings(page, scenario.name);
  } else if (scenario.name === 'stale-desktop') {
    await page.locator('.watch-telemetry__batch[data-freshness="STALE"]').waitFor();
    await page.getByText('LATEST_ATTEMPT_FAILED', { exact: true }).first().waitFor();
    await page.getByText(/RETRY_AFTER .* RATE_LIMITED/u).waitFor();
  } else if (scenario.name === 'unknown-disconnect-mobile') {
    await page.getByRole('button', { name: 'Disconnect' }).click();
    await page.getByRole('heading', { name: 'Watch connection ended' }).waitFor();
    await page.locator('.watch-telemetry__batch[data-freshness="UNKNOWN"]').waitFor();
    await page.locator('.watch-telemetry__section[data-freshness="UNKNOWN"]').waitFor();
    await page.getByText('NEVER_OBSERVED', { exact: true }).first().waitFor();
    const staleOpenBadge = await page.locator('.watch-workspace__item .watch-workspace__badge[data-state="OPEN"]').count();
    if (staleOpenBadge !== 0) {
      throw new Error(`${scenario.name}: disconnected selection retained a stale OPEN badge`);
    }
    const metrics = await page.getByLabel('Watch session status').evaluate((status) =>
      [...status.querySelectorAll('.bcsp-metric')].map((metric) => ({
        label: metric.querySelector('dt')?.textContent,
        value: metric.querySelector('dd')?.textContent,
      })));
    const selected = metrics.find(({ label }) => label === 'Selected')?.value;
    const active = metrics.find(({ label }) => label === 'Active')?.value;
    if (selected !== '1' || active !== '0') {
      throw new Error(`${scenario.name}: disconnect state selected=${selected} active=${active}`);
    }
  }
}

const scenarios = [
  { name: 'active-desktop', profile: 'FRESH', selected: 1, start: true, viewport: { width: 1440, height: 1800 } },
  { name: 'active-mobile', profile: 'FRESH', selected: 1, start: true, viewport: { width: 390, height: 844 } },
  {
    name: 'continuous-acd-desktop',
    profile: 'FRESH',
    selected: 3,
    start: true,
    continuous: true,
    unlimited: true,
    viewport: { width: 1440, height: 2000 },
  },
  {
    name: 'audio-blocked-desktop',
    profile: 'FRESH',
    selected: 1,
    start: true,
    audioBlocked: true,
    viewport: { width: 1440, height: 1800 },
  },
  { name: 'stale-desktop', profile: 'STALE', selected: 1, start: false, viewport: { width: 1440, height: 1600 } },
  {
    name: 'unknown-disconnect-mobile',
    profile: 'UNKNOWN',
    selected: 1,
    start: true,
    viewport: { width: 390, height: 844 },
  },
];
const requestedScenario = process.env.BCSP_VISUAL_SCENARIO;
const selectedScenarios = requestedScenario === undefined
  ? scenarios
  : scenarios.filter(({ name }) => name === requestedScenario);
if (selectedScenarios.length === 0) {
  throw new Error(`Unknown BCSP_VISUAL_SCENARIO: ${requestedScenario}`);
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
  for (const scenario of selectedScenarios) {
    const page = await browser.newPage({
      colorScheme: 'light',
      locale: 'en-US',
      reducedMotion: 'reduce',
      viewport: scenario.viewport,
    });
    await installBrowserFakes(page, scenario);
    await installApi(page, scenario);
    await page.goto(`${baseUrl}/local.html`, { waitUntil: 'domcontentloaded' });
    await selectSyntheticSections(page, scenario.selected);
    if (scenario.start) await startWatch(page, scenario);
    await assertScenario(page, scenario);
    await settleVisualLayout(page);
    await assertNoOverflow(page, scenario.name);
    if (scenario.viewport.width >= 800) await page.screenshot({ animations: 'disabled' });
    await page.screenshot({
      animations: 'disabled',
      fullPage: scenario.viewport.width < 800,
      path: resolve(outputDirectory, `${scenario.name}.png`),
    });
    await page.close();
  }
} finally {
  await browser.close();
}

process.stdout.write(`P7.2-003 Watch flow snapshots: PASS (${selectedScenarios.length}/${selectedScenarios.length})\n`);
