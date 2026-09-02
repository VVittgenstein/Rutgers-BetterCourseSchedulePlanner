// @vitest-environment jsdom

import { StrictMode } from 'react';
import { act, cleanup, render, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
  OpenEpisodeV1,
  OpenRefreshStatusV1,
  OpenSectionStatusV1,
  ProductApiPort,
  ProductRuntimePort,
  SectionKey,
  TraceId,
  WatchClientCommandV1,
  WatchClientPort,
  WatchConnectionState,
  WatchRecoveryState,
  WatchCueOutcome,
  WatchPolicyV1,
  WatchServerEventV1,
  WsServerEnvelope,
} from '../src/ui/shared/product';
import { ProductClientError } from '../src/ui/shared/product';
import {
  BATCH_STATUS_COALESCE_MILLISECONDS,
  DEFAULT_WATCH_POLICY,
  LiveWatchProvider,
  useLiveWatch,
  type LiveWatchValue,
} from '../src/ui/shared/watch/LiveWatchProvider';
import {
  WatchAudioController,
  type WatchAudioUnlockResult,
} from '../src/ui/shared/watch/audio';

const AT = '2030-01-01T00:00:00Z';
const LATER = '2030-01-01T00:00:30Z';
const SECTION_A = { term: 'T2030F', campus: 'CAMPUS_A', index: '00001' } satisfies SectionKey;
const SECTION_C = { term: 'T2030F', campus: 'CAMPUS_A', index: '00003' } satisfies SectionKey;
const SECTION_D = { term: 'T2030F', campus: 'CAMPUS_A', index: '00004' } satisfies SectionKey;
/** A Section of a SECOND batch, so one batch can leave the desk while another stays. */
const SECTION_E = { term: 'T2030F', campus: 'CAMPUS_B', index: '00009' } satisfies SectionKey;
/** The only shape the server accepts for a batch key: nothing but these two fields. */
const BATCH_A = { term: SECTION_A.term, campus: SECTION_A.campus };
/** The provider's resource key joins term and campus with a NUL separator. */
const BATCH_A_RESOURCE_KEY = `batch:${SECTION_A.term}${String.fromCharCode(0)}${SECTION_A.campus}`;
const ACTIVE_A = '00000000-0000-4000-8000-0000000000a1';
const ACTIVE_C = '00000000-0000-4000-8000-0000000000c1';
const ACTIVE_D = '00000000-0000-4000-8000-0000000000d1';
const CONTINUOUS_POLICY = {
  notificationMode: 'CONTINUOUS',
  maxAudible: 3,
  continuousDuration: { kind: 'FINITE', seconds: 600 },
} satisfies WatchPolicyV1;

const COUNTS = {
  attempted: 1,
  succeeded: 1,
  failed: 0,
  empty: 0,
};

class FakeWatch implements WatchClientPort {
  state: WatchConnectionState = 'OPEN';
  lastContactAt: number | null = null;
  recovery: WatchRecoveryState = { phase: 'IDLE', attempt: 0, nextAttemptAt: null };
  readonly #recoveryListeners = new Set<(state: WatchRecoveryState) => void>();
  readonly #contactListeners = new Set<(at: number) => void>();

  subscribeRecovery(listener: (state: WatchRecoveryState) => void): () => void {
    this.#recoveryListeners.add(listener);
    return () => this.#recoveryListeners.delete(listener);
  }

  subscribeContact(listener: (at: number) => void): () => void {
    this.#contactListeners.add(listener);
    return () => this.#contactListeners.delete(listener);
  }

  /** Drives the recovery phase the way the real transport would. */
  recover(state: WatchRecoveryState): void {
    this.recovery = state;
    this.#recoveryListeners.forEach((listener) => listener(state));
  }

  /** The server proving it is there, at this moment. */
  contact(at: number): void {
    this.lastContactAt = at;
    this.#contactListeners.forEach((listener) => listener(at));
  }
  readonly commands: WatchClientCommandV1[] = [];
  readonly connect = vi.fn(() => undefined);
  readonly disconnect = vi.fn(() => undefined);
  readonly #eventListeners = new Set<(envelope: WsServerEnvelope<WatchServerEventV1>) => void>();
  readonly #stateListeners = new Set<(state: WatchConnectionState) => void>();
  #message = 0;

  send(command: WatchClientCommandV1): TraceId {
    this.commands.push(command);
    this.#message += 1;
    return this.id(this.#message);
  }

  subscribe(listener: (envelope: WsServerEnvelope<WatchServerEventV1>) => void): () => void {
    this.#eventListeners.add(listener);
    return () => this.#eventListeners.delete(listener);
  }

  subscribeState(listener: (state: WatchConnectionState) => void): () => void {
    this.#stateListeners.add(listener);
    return () => this.#stateListeners.delete(listener);
  }

  emit(payload: WatchServerEventV1): void {
    this.#message += 1;
    const envelope: WsServerEnvelope<WatchServerEventV1> = {
      protocolVersion: 1,
      messageId: this.id(this.#message),
      payload,
    };
    this.#eventListeners.forEach((listener) => listener(envelope));
  }

  transition(state: WatchConnectionState): void {
    this.state = state;
    this.#stateListeners.forEach((listener) => listener(state));
  }

  private id(value: number): TraceId {
    return `00000000-0000-4000-8000-${value.toString(16).padStart(12, '0')}`;
  }
}

class FakeAudio {
  readonly outcomes: WatchCueOutcome[] = [];
  readonly subscribeState = vi.fn(() => () => undefined);
  readonly heal = vi.fn(async () => null);
  readonly unlock = vi.fn(async (): Promise<WatchAudioUnlockResult> => 'READY');
  readonly play = vi.fn((): WatchCueOutcome => this.outcomes.shift() ?? 'STARTED');
  readonly preview = vi.fn((): WatchCueOutcome => this.outcomes.shift() ?? 'STARTED');
  readonly startContinuous = vi.fn((): WatchCueOutcome => this.outcomes.shift() ?? 'STARTED');
  readonly stopContinuous = vi.fn(() => undefined);
  readonly dispose = vi.fn(() => undefined);
}

interface Harness {
  readonly audio: FakeAudio;
  readonly onVolumeChange: ReturnType<typeof vi.fn>;
  readonly watch: FakeWatch;
  rerenderVolume(volume: number): void;
  value(): LiveWatchValue;
}

function Probe({ publish }: { readonly publish: (value: LiveWatchValue) => void }) {
  publish(useLiveWatch());
  return null;
}

function refreshStatus(batch: { readonly term: string; readonly campus: string }): OpenRefreshStatusV1 {
  return {
    contractVersion: 1,
    batch,
    catalogContentVersion: 1,
    latestAttempt: null,
    latestFailure: null,
    lastValidObservation: null,
    lastBodyChangeAt: null,
    lastStateChangeAt: null,
    freshness: {
      state: 'UNKNOWN',
      observedAt: null,
      freshUntil: null,
      lastKnownGoodAgeSeconds: null,
      uncertainty: 'NEVER_OBSERVED',
    },
    scheduler: {
      lane: 'GENERAL',
      requestedGeneralIntervalSeconds: 30,
      requestedEffectiveIntervalSeconds: 30,
      activeWatchCount: 0,
      nextDueAt: null,
      inFlight: false,
      schedulerLagMilliseconds: 0,
      actualStartToStartIntervalMilliseconds: null,
      failureStreak: 0,
    },
    circuit: {
      state: 'CLOSED',
      reason: null,
      openedAt: null,
      retryAt: null,
      diagnosticRecheckRequired: false,
    },
    counterSnapshot: {
      runCounts: COUNTS,
      todayCounts: COUNTS,
      rutgersDay: '2030-01-01',
      dayTimezone: 'America/New_York',
    },
  };
}

function sectionStatus(sectionKey: SectionKey): OpenSectionStatusV1 {
  return {
    contractVersion: 1,
    sectionKey,
    state: 'UNKNOWN',
    lastObservationId: null,
    catalogContentVersion: 1,
    freshness: {
      state: 'UNKNOWN',
      observedAt: null,
      freshUntil: null,
      lastKnownGoodAgeSeconds: null,
      uncertainty: 'NEVER_OBSERVED',
    },
    schedulerLagMilliseconds: 0,
    counterSnapshot: {
      runCounts: COUNTS,
      todayCounts: COUNTS,
      rutgersDay: '2030-01-01',
      dayTimezone: 'America/New_York',
    },
  };
}

function unexpected(): never {
  throw new Error('unexpected product API call');
}

function createHarness(
  initialState: WatchConnectionState = 'OPEN',
  strictMode = false,
  productOverrides: Partial<ProductApiPort> = {},
  initialVolume = 70,
): Harness {
  const watch = new FakeWatch();
  watch.state = initialState;
  const audio = new FakeAudio();
  const product: ProductApiPort = {
    catalogDiscovery: unexpected,
    courseDetail: unexpected,
    filterSchema: unexpected,
    openSectionStatus: vi.fn(async ({ sectionKey }) => sectionStatus(sectionKey)),
    openStatus: vi.fn(async ({ batch }) => refreshStatus(batch)),
    searchCourses: unexpected,
    searchSections: unexpected,
    sectionDetail: unexpected,
    ...productOverrides,
  };
  const runtime: ProductRuntimePort = {
    product,
    watch,
    dispose: vi.fn(),
  };
  let current: LiveWatchValue | null = null;
  const onVolumeChange = vi.fn();
  const provider = (volume: number) => (
    <LiveWatchProvider
      audio={audio as unknown as WatchAudioController}
      initialVolume={volume}
      onVolumeChange={onVolumeChange}
      runtime={runtime}
    >
      <Probe publish={(value) => { current = value; }} />
    </LiveWatchProvider>
  );
  const renderProvider = (volume: number) => strictMode
    ? <StrictMode>{provider(volume)}</StrictMode>
    : provider(volume);
  const view = render(renderProvider(initialVolume));
  return {
    audio,
    onVolumeChange,
    rerenderVolume(volume) {
      view.rerender(renderProvider(volume));
    },
    watch,
    value() {
      if (current === null) throw new Error('LiveWatch context was not published');
      return current;
    },
  };
}

/**
 * A desk that opens with Sections already on it, the way a reload does.
 *
 * The difference from `createHarness` is not cosmetic: a persisted selection
 * is present in the very first render, so the telemetry pass and the
 * connection-driven pass both run on mount and overlap -- which is the shape
 * the real page has and the one a `select()` in a test never produces.
 */
function createPersistedHarness(
  initialSelected: readonly SectionKey[],
  initialState: WatchConnectionState,
  productOverrides: Partial<ProductApiPort>,
  strictMode = false,
): Harness {
  const watch = new FakeWatch();
  watch.state = initialState;
  const audio = new FakeAudio();
  const product: ProductApiPort = {
    catalogDiscovery: unexpected,
    courseDetail: unexpected,
    filterSchema: unexpected,
    openSectionStatus: vi.fn(async ({ sectionKey }) => sectionStatus(sectionKey)),
    openStatus: vi.fn(async ({ batch }) => refreshStatus(batch)),
    searchCourses: unexpected,
    searchSections: unexpected,
    sectionDetail: unexpected,
    ...productOverrides,
  };
  const runtime: ProductRuntimePort = { product, watch, dispose: vi.fn() };
  let current: LiveWatchValue | null = null;
  const onVolumeChange = vi.fn();
  const tree = (
    <LiveWatchProvider
      audio={audio as unknown as WatchAudioController}
      initialSelected={initialSelected}
      onVolumeChange={onVolumeChange}
      runtime={runtime}
    >
      <Probe publish={(value) => { current = value; }} />
    </LiveWatchProvider>
  );
  render(strictMode ? <StrictMode>{tree}</StrictMode> : tree);
  return {
    audio,
    onVolumeChange,
    rerenderVolume() { throw new Error('not supported'); },
    watch,
    value() {
      if (current === null) throw new Error('LiveWatch context was not published');
      return current;
    },
  };
}

function startResult(
  entries: readonly (readonly [SectionKey, string])[],
): WatchServerEventV1 {
  return {
    type: 'START_RESULT',
    result: {
      contractVersion: 1,
      items: entries.map(([sectionKey, activeWatchId]) => ({
        status: 'ACTIVE' as const,
        sectionKey,
        activeWatchId,
        startedAt: AT,
      })),
      activeWatchCount: entries.length,
    },
  };
}

function stopped(sectionKey: SectionKey, activeWatchId: string): WatchServerEventV1 {
  return {
    type: 'WATCH_STOPPED',
    stopped: {
      contractVersion: 1,
      activeWatchId,
      sectionKey,
      reason: 'USER_REQUESTED',
      stoppedAt: LATER,
    },
  };
}

function audibleCap(
  ordinal: number,
  sectionKey: SectionKey = SECTION_A,
  activeWatchId: string = ACTIVE_A,
): WatchServerEventV1 {
  return {
    type: 'AUDIO_DISPOSITION',
    audio: {
      disposition: 'SILENT_MAX_AUDIBLE',
      activeWatchId,
      sectionKey,
      observationId: `20000000-0000-4000-8000-${ordinal.toString().padStart(12, '0')}`,
      audibleCount: 3,
      maxAudible: 3,
      emittedAt: LATER,
    },
  };
}

function observationEvent(
  freshUntil = LATER,
  observedAt = AT,
): WatchServerEventV1 {
  return {
    type: 'OPEN_OBSERVATION',
    fanout: {
      contractVersion: 1,
      activeWatchId: ACTIVE_A,
      observation: {
        contractVersion: 1,
        observationId: '20000000-0000-4000-8000-000000000001',
        refreshObservationId: '20000000-0000-4000-8000-000000000002',
        batch: { term: SECTION_A.term, campus: SECTION_A.campus },
        sectionKey: SECTION_A,
        pullSequence: 1,
        catalogContentVersion: 1,
        state: 'OPEN',
        observedAt,
        freshUntil,
        schedulerLagMilliseconds: 4,
        counterSnapshot: {
          runCounts: COUNTS,
          todayCounts: COUNTS,
          rutgersDay: '2030-01-01',
          dayTimezone: 'America/New_York',
        },
      },
    },
  };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((resolvePromise, rejectPromise) => {
    resolve = resolvePromise;
    reject = rejectPromise;
  });
  return { promise, reject, resolve };
}

function episode(
  sectionKey: SectionKey,
  activeWatchId: string,
  suffix: string,
  state: OpenEpisodeV1['state'] = 'UNACKNOWLEDGED',
): OpenEpisodeV1 {
  return {
    contractVersion: 1,
    episodeId: `00000000-0000-4000-8000-${suffix.padStart(12, '0')}`,
    activeWatchId,
    sectionKey,
    state,
    notificationMode: 'CONTINUOUS',
    continuousDuration: { kind: 'FINITE', seconds: 600 },
    maxAudible: 3,
    audibleCount: 0,
    firstObservedAt: AT,
    lastObservedAt: AT,
    observationCount: 1,
    latestObservationId: `10000000-0000-4000-8000-${suffix.padStart(12, '0')}`,
    stateChangedAt: state === 'TIMED_OUT' ? LATER : AT,
    closedAt: null,
  };
}

async function selectAndStart(
  harness: Harness,
  sections: readonly SectionKey[],
  policy: WatchPolicyV1 = DEFAULT_WATCH_POLICY,
): Promise<void> {
  await act(async () => {
    sections.forEach((sectionKey) => harness.value().select(sectionKey));
  });
  await act(async () => {
    await harness.value().startSelected(policy);
  });
}

async function emit(harness: Harness, event: WatchServerEventV1): Promise<void> {
  await act(async () => harness.watch.emit(event));
}

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe('LiveWatchProvider', () => {
  it('applies a persisted settings volume without remounting or writing it back', async () => {
    const harness = createHarness('OPEN', false, {}, 70);
    expect(harness.value().volume).toBe(70);

    await act(async () => harness.rerenderVolume(35));

    expect(harness.value().volume).toBe(35);
    expect(harness.onVolumeChange).not.toHaveBeenCalled();
  });

  it('keeps browser audio usable through React StrictMode effect replay', async () => {
    const harness = createHarness('OPEN', true);
    await selectAndStart(harness, [SECTION_A]);
    await act(async () => Promise.resolve());
    expect(harness.audio.dispose).not.toHaveBeenCalled();
    expect(harness.audio.unlock).toHaveBeenCalledOnce();
    expect(harness.audio.preview).not.toHaveBeenCalled();
    expect(harness.watch.commands).toContainEqual(expect.objectContaining({ type: 'START_WATCH' }));
  });

  it('settles the user-gesture audio unlock before sending START_WATCH', async () => {
    const unlock = deferred<WatchAudioUnlockResult>();
    const harness = createHarness();
    harness.audio.unlock.mockImplementationOnce(async () => unlock.promise);
    await act(async () => harness.value().select(SECTION_A));

    let start!: Promise<void>;
    act(() => {
      start = harness.value().startSelected(DEFAULT_WATCH_POLICY);
    });

    expect(harness.value().starting).toBe(true);
    expect(harness.value().pending).toEqual([SECTION_A]);
    expect(harness.watch.commands).toEqual([]);

    await act(async () => {
      unlock.resolve('READY');
      await start;
    });
    expect(harness.value().starting).toBe(false);
    expect(harness.watch.commands).toContainEqual(expect.objectContaining({ type: 'START_WATCH' }));
  });

  it.each(['BLOCKED', 'FAILED'] as const)(
    'starts live watch after a %s audio unlock and keeps the warning state visible',
    async (unlockResult) => {
      const harness = createHarness();
      harness.audio.unlock.mockResolvedValueOnce(unlockResult);
      await selectAndStart(harness, [SECTION_A]);

      expect(harness.watch.commands).toContainEqual(expect.objectContaining({ type: 'START_WATCH' }));
      expect(harness.value().audioState).toBe(unlockResult);
      expect(harness.value().notices).toContainEqual(expect.objectContaining({
        code: unlockResult === 'BLOCKED' ? 'AUDIO_BLOCKED' : 'AUDIO_FAILED',
      }));
    },
  );

  it('does not lose the first fresh OPEN cue after the ordered start', async () => {
    const harness = createHarness();
    await selectAndStart(harness, [SECTION_A]);
    await emit(harness, startResult([[SECTION_A, ACTIVE_A]]));
    await emit(harness, {
      type: 'AUDIO_DISPOSITION',
      audio: {
        disposition: 'CUE_REQUESTED',
        cue: {
          cueId: '30000000-0000-4000-8000-000000000001',
          activeWatchId: ACTIVE_A,
          sectionKey: SECTION_A,
          trigger: {
            kind: 'ONE_SHOT_OBSERVATION',
            observationId: '40000000-0000-4000-8000-000000000001',
          },
          emittedAt: LATER,
        },
      },
    });

    expect(harness.audio.play).toHaveBeenCalledOnce();
    expect(harness.watch.commands.at(-1)).toMatchObject({
      type: 'REPORT_CUE_OUTCOME',
      report: { outcome: 'STARTED', sectionKey: SECTION_A },
    });
  });

  it('plays a local preview only when the user explicitly tests sound', async () => {
    const harness = createHarness();
    await act(async () => {
      await expect(harness.value().testSound()).resolves.toBe('READY');
    });
    expect(harness.audio.unlock).toHaveBeenCalledOnce();
    expect(harness.audio.preview).toHaveBeenCalledWith(70);
    expect(harness.value().muted).toBe(false);

    harness.audio.outcomes.push('FAILED');
    await act(async () => {
      await expect(harness.value().testSound()).resolves.toBe('FAILED');
    });
    expect(harness.value().audioState).toBe('FAILED');
    expect(harness.value().notices).toContainEqual(expect.objectContaining({
      code: 'AUDIO_FAILED',
    }));
  });

  it('ignores an old observation telemetry response after a newer refresh wins', async () => {
    const slow = deferred<OpenRefreshStatusV1>();
    const openStatus = vi.fn<ProductApiPort['openStatus']>(async ({ batch }) => refreshStatus(batch));
    const harness = createHarness('OPEN', false, { openStatus });
    await act(async () => harness.value().select(SECTION_A));
    await waitFor(() => expect(openStatus).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(harness.value().telemetryLoading).toBe(false));

    openStatus.mockImplementationOnce(async () => slow.promise);
    await emit(harness, observationEvent());
    await waitFor(() => expect(openStatus).toHaveBeenCalledTimes(2));

    const newer = {
      ...refreshStatus({ term: SECTION_A.term, campus: SECTION_A.campus }),
      scheduler: {
        ...refreshStatus({ term: SECTION_A.term, campus: SECTION_A.campus }).scheduler,
        schedulerLagMilliseconds: 987,
      },
    };
    openStatus.mockResolvedValueOnce(newer);
    await act(async () => harness.value().refreshTelemetry());
    expect(harness.value().batchStatuses[0]?.scheduler.schedulerLagMilliseconds).toBe(987);

    slow.resolve({
      ...refreshStatus({ term: SECTION_A.term, campus: SECTION_A.campus }),
      scheduler: {
        ...refreshStatus({ term: SECTION_A.term, campus: SECTION_A.campus }).scheduler,
        schedulerLagMilliseconds: 1,
      },
    });
    await act(async () => Promise.resolve());
    expect(harness.value().batchStatuses[0]?.scheduler.schedulerLagMilliseconds).toBe(987);
  });

  it('keeps successful UNKNOWN telemetry as no-data without inventing a success time', async () => {
    const harness = createHarness();
    await act(async () => harness.value().select(SECTION_A));
    await waitFor(() => expect(harness.value().telemetryLoading).toBe(false));

    expect(harness.value().telemetryResources).toHaveLength(2);
    expect(harness.value().telemetryResources).toEqual(expect.arrayContaining([
      expect.objectContaining({
        availability: 'ERROR_NO_DATA',
        error: null,
        lastSuccessAt: null,
      }),
    ]));
    expect(harness.value().telemetryResources.every((resource) =>
      resource.availability === 'ERROR_NO_DATA'
      && resource.error === null
      && resource.lastSuccessAt === null)).toBe(true);
  });

  it('tracks telemetry failures per resource and clears only the resource that recovered', async () => {
    let sectionAFails = true;
    const openSectionStatus = vi.fn<ProductApiPort['openSectionStatus']>(async ({ sectionKey }) => {
      if (sectionKey.index === SECTION_A.index && sectionAFails) {
        throw new Error('initial Section telemetry failure');
      }
      return sectionStatus(sectionKey);
    });
    const openStatus = vi.fn<ProductApiPort['openStatus']>()
      .mockRejectedValueOnce(new Error('initial batch telemetry failure'))
      .mockImplementation(async ({ batch }) => refreshStatus(batch));
    const harness = createHarness('OPEN', false, { openSectionStatus, openStatus });

    await act(async () => {
      harness.value().select(SECTION_A);
      harness.value().select(SECTION_C);
    });
    await waitFor(() => expect(harness.value().telemetryLoading).toBe(false));
    expect(harness.value().telemetryResources.filter((resource) => resource.error !== null))
      .toHaveLength(2);
    expect(harness.value().sectionStatuses.map((status) => status.sectionKey)).toEqual([SECTION_C]);
    expect(harness.value().batchStatuses).toEqual([]);

    await emit(harness, observationEvent());
    await waitFor(() => expect(openStatus).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(harness.value().batchStatuses).toHaveLength(1));
    expect(harness.value().sectionStatuses.map((status) => status.sectionKey)).toEqual([
      SECTION_C,
      SECTION_A,
    ]);
    expect(harness.value().telemetryResources.filter((resource) => resource.error !== null))
      .toHaveLength(1);
    expect(harness.value().telemetryResources.find((resource) =>
      resource.kind === 'SECTION' && resource.sectionKey?.index === SECTION_A.index))
      .toMatchObject({ availability: 'ERROR_NO_DATA' });

    sectionAFails = false;
    const sectionResource = harness.value().telemetryResources.find((resource) =>
      resource.kind === 'SECTION' && resource.sectionKey?.index === SECTION_A.index);
    await act(async () => harness.value().retryTelemetryResource(sectionResource!.key));
    expect(harness.value().telemetryResources.filter((resource) => resource.error !== null))
      .toHaveLength(0);
  });

  it('keeps the telemetry error while another Section resource is still failed', async () => {
    const openSectionStatus = vi.fn<ProductApiPort['openSectionStatus']>(async () => {
      throw new Error('Section telemetry failure');
    });
    const openStatus = vi.fn<ProductApiPort['openStatus']>(async ({ batch }) => refreshStatus(batch));
    const harness = createHarness('OPEN', false, { openSectionStatus, openStatus });

    await act(async () => {
      harness.value().select(SECTION_A);
      harness.value().select(SECTION_D);
    });
    await waitFor(() => expect(harness.value().telemetryLoading).toBe(false));
    expect(harness.value().telemetryResources.filter((resource) => resource.error !== null))
      .toHaveLength(2);

    await emit(harness, observationEvent());
    await waitFor(() => expect(openStatus).toHaveBeenCalledTimes(2));
    expect(harness.value().sectionStatuses.map((status) => status.sectionKey)).toEqual([SECTION_A]);
    expect(harness.value().batchStatuses).toHaveLength(1);
    expect(harness.value().telemetryResources.some((resource) =>
      resource.kind === 'SECTION'
      && resource.sectionKey?.index === SECTION_D.index
      && resource.error !== null)).toBe(true);
  });

  it('does not resurrect telemetry after the final Section is removed', async () => {
    const slow = deferred<OpenRefreshStatusV1>();
    const openStatus = vi.fn<ProductApiPort['openStatus']>(async ({ batch }) => refreshStatus(batch));
    const harness = createHarness('OPEN', false, { openStatus });
    await act(async () => harness.value().select(SECTION_A));
    await waitFor(() => expect(harness.value().telemetryLoading).toBe(false));

    openStatus.mockImplementationOnce(async () => slow.promise);
    await emit(harness, observationEvent());
    await waitFor(() => expect(openStatus).toHaveBeenCalledTimes(2));
    await act(async () => harness.value().remove(SECTION_A));
    expect(harness.value().batchStatuses).toEqual([]);
    expect(harness.value().sectionStatuses).toEqual([]);

    slow.resolve(refreshStatus({ term: SECTION_A.term, campus: SECTION_A.campus }));
    await act(async () => Promise.resolve());
    expect(harness.value().batchStatuses).toEqual([]);
    expect(harness.value().sectionStatuses).toEqual([]);
    expect(harness.value().telemetryResources).toEqual([]);
  });

  it('expires a FRESH observation at freshUntil before the status reread completes', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(AT));
    const authoritative = deferred<OpenSectionStatusV1>();
    const openSectionStatus = vi.fn<ProductApiPort['openSectionStatus']>()
      .mockResolvedValueOnce(sectionStatus(SECTION_A))
      .mockImplementation(async () => authoritative.promise);
    const harness = createHarness('OPEN', false, { openSectionStatus });
    await act(async () => harness.value().select(SECTION_A));
    await act(async () => Promise.resolve());
    expect(openSectionStatus).toHaveBeenCalledOnce();

    const freshUntil = '2030-01-01T00:00:01Z';
    await emit(harness, observationEvent(freshUntil));
    expect(harness.value().sectionStatuses[0]?.freshness.state).toBe('FRESH');

    await act(async () => {
      vi.advanceTimersByTime(1_001);
      await Promise.resolve();
    });
    expect(harness.value().sectionStatuses[0]?.freshness).toMatchObject({
      state: 'STALE',
      uncertainty: 'STALE_LAST_KNOWN_GOOD',
      lastKnownGoodAgeSeconds: 1,
    });
    expect(harness.value().telemetryLoading).toBe(true);

    await emit(harness, observationEvent(AT));
    expect(harness.value().sectionStatuses[0]?.freshness.state).toBe('STALE');
  });

  it('sends only term and campus in every batch status request', async () => {
    const openStatus = vi.fn<ProductApiPort['openStatus']>(async ({ batch }) => refreshStatus(batch));
    const harness = createHarness('OPEN', false, { openStatus });
    await act(async () => harness.value().select(SECTION_A));
    await waitFor(() => expect(openStatus).toHaveBeenCalledTimes(1));
    // toStrictEqual + Object.keys are what catch a stray `index`: a Section
    // key satisfies the batch key type structurally, so nothing at compile
    // time would.
    expect(openStatus.mock.calls[0]![0]).toStrictEqual({ contractVersion: 1, batch: BATCH_A });
    expect(Object.keys(openStatus.mock.calls[0]![0].batch)).toEqual(['term', 'campus']);
    await waitFor(() => expect(harness.value().telemetryLoading).toBe(false));

    await emit(harness, observationEvent());
    await waitFor(() => expect(openStatus).toHaveBeenCalledTimes(2));
    expect(openStatus.mock.calls[1]![0]).toStrictEqual({ contractVersion: 1, batch: BATCH_A });
    expect(Object.keys(openStatus.mock.calls[1]![0].batch)).toEqual(['term', 'campus']);
    await waitFor(() => expect(harness.value().telemetryResources.every((resource) =>
      !resource.loading)).toBe(true));

    openStatus.mockRejectedValueOnce(new ProductClientError(503, null));
    await act(async () => harness.value().refreshTelemetry());
    expect(openStatus).toHaveBeenCalledTimes(3);
    expect(harness.value().telemetryResources.find((resource) => resource.kind === 'BATCH'))
      .toMatchObject({ error: { httpStatus: 503, retryable: true }, loading: false });

    await act(async () => harness.value().retryTelemetryResource(BATCH_A_RESOURCE_KEY));
    expect(openStatus).toHaveBeenCalledTimes(4);
    expect(openStatus.mock.calls[3]![0]).toStrictEqual({ contractVersion: 1, batch: BATCH_A });
    expect(Object.keys(openStatus.mock.calls[3]![0].batch)).toEqual(['term', 'campus']);
    const resource = harness.value().telemetryResources.find((value) => value.kind === 'BATCH');
    expect(resource?.error).toBeNull();
    expect(resource?.batch).toStrictEqual(BATCH_A);
    expect(Object.keys(resource!.batch!)).toEqual(['term', 'campus']);
  });

  it('reads one batch status for two Sections of the same batch', async () => {
    const openSectionStatus = vi.fn<ProductApiPort['openSectionStatus']>(async ({ sectionKey }) =>
      sectionStatus(sectionKey));
    const openStatus = vi.fn<ProductApiPort['openStatus']>(async ({ batch }) => refreshStatus(batch));
    const harness = createHarness('OPEN', false, { openSectionStatus, openStatus });
    await act(async () => {
      harness.value().select(SECTION_A);
      harness.value().select(SECTION_C);
    });
    await waitFor(() => expect(harness.value().telemetryLoading).toBe(false));

    expect(openSectionStatus).toHaveBeenCalledTimes(2);
    expect(openStatus).toHaveBeenCalledTimes(1);
    expect(openStatus.mock.calls[0]![0]).toStrictEqual({ contractVersion: 1, batch: BATCH_A });
  });

  it('coalesces a burst of observations for one batch into a single status read', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(AT));
    const openStatus = vi.fn<ProductApiPort['openStatus']>(async ({ batch }) => refreshStatus(batch));
    const harness = createHarness('OPEN', false, { openStatus });
    await act(async () => harness.value().select(SECTION_A));
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(openStatus).toHaveBeenCalledTimes(1);

    await emit(harness, observationEvent());
    await act(async () => { await vi.advanceTimersByTimeAsync(100); });
    await emit(harness, observationEvent());
    await act(async () => { await vi.advanceTimersByTimeAsync(100); });
    await emit(harness, observationEvent());
    expect(openStatus).toHaveBeenCalledTimes(1);

    await act(async () => { await vi.advanceTimersByTimeAsync(BATCH_STATUS_COALESCE_MILLISECONDS + 1); });
    expect(openStatus).toHaveBeenCalledTimes(2);
    expect(openStatus.mock.calls[1]![0]).toStrictEqual({ contractVersion: 1, batch: BATCH_A });
    expect(harness.value().batchStatuses).toHaveLength(1);

    // A fourth observation after the in-flight read completed is a new burst.
    await emit(harness, observationEvent());
    await act(async () => { await vi.advanceTimersByTimeAsync(BATCH_STATUS_COALESCE_MILLISECONDS + 1); });
    expect(openStatus).toHaveBeenCalledTimes(3);
  });

  it('queues at most one follow-up read while a batch status read is in flight', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(AT));
    const slow = deferred<OpenRefreshStatusV1>();
    const openStatus = vi.fn<ProductApiPort['openStatus']>(async ({ batch }) => refreshStatus(batch));
    const harness = createHarness('OPEN', false, { openStatus });
    await act(async () => harness.value().select(SECTION_A));
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(openStatus).toHaveBeenCalledTimes(1);

    openStatus.mockImplementationOnce(async () => slow.promise);
    await emit(harness, observationEvent());
    await act(async () => { await vi.advanceTimersByTimeAsync(BATCH_STATUS_COALESCE_MILLISECONDS + 1); });
    expect(openStatus).toHaveBeenCalledTimes(2);

    // Two more bursts while that read is still out: neither starts a read.
    await emit(harness, observationEvent());
    await act(async () => { await vi.advanceTimersByTimeAsync(BATCH_STATUS_COALESCE_MILLISECONDS + 1); });
    await emit(harness, observationEvent());
    await act(async () => { await vi.advanceTimersByTimeAsync(BATCH_STATUS_COALESCE_MILLISECONDS + 1); });
    expect(openStatus).toHaveBeenCalledTimes(2);

    slow.resolve(refreshStatus(BATCH_A));
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(openStatus).toHaveBeenCalledTimes(3);
    expect(openStatus.mock.calls[2]![0]).toStrictEqual({ contractVersion: 1, batch: BATCH_A });

    await act(async () => { await vi.advanceTimersByTimeAsync(BATCH_STATUS_COALESCE_MILLISECONDS * 4); });
    expect(openStatus).toHaveBeenCalledTimes(3);
  });

  it('does not auto-retry a non-retryable 4xx on freshness expiry while a 5xx keeps retrying', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(AT));
    const malformed = new ProductClientError(400, {
      protocolVersion: 1,
      error: {
        code: 'MALFORMED_REQUEST',
        messageKey: 'error.malformed_request',
        traceId: 'trace-batch-400',
        details: [],
      },
    });
    const openStatus = vi.fn<ProductApiPort['openStatus']>(async () => { throw malformed; });
    const freshFor = (milliseconds: number) => new Date(Date.now() + milliseconds).toISOString();
    const openSectionStatus = vi.fn<ProductApiPort['openSectionStatus']>(async ({ sectionKey }) => ({
      ...sectionStatus(sectionKey),
      state: 'OPEN',
      freshness: {
        state: 'FRESH',
        observedAt: freshFor(0),
        freshUntil: freshFor(1_000),
        lastKnownGoodAgeSeconds: 0,
        uncertainty: null,
      },
    }));
    const harness = createHarness('OPEN', false, { openSectionStatus, openStatus });
    const batchResource = () =>
      harness.value().telemetryResources.find((resource) => resource.kind === 'BATCH');

    await act(async () => harness.value().select(SECTION_A));
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(openSectionStatus).toHaveBeenCalledTimes(1);
    expect(openStatus).toHaveBeenCalledTimes(1);
    expect(batchResource()).toMatchObject({
      availability: 'ERROR_NO_DATA',
      loading: false,
      error: {
        httpStatus: 400,
        apiCode: 'MALFORMED_REQUEST',
        traceId: 'trace-batch-400',
        retryable: false,
      },
    });

    // Two freshness expiries: the Section is re-read each time, the batch is not.
    await act(async () => { await vi.advanceTimersByTimeAsync(1_001); });
    expect(openSectionStatus).toHaveBeenCalledTimes(2);
    expect(openStatus).toHaveBeenCalledTimes(1);
    expect(batchResource()).toMatchObject({ loading: false, error: { retryable: false } });
    await act(async () => { await vi.advanceTimersByTimeAsync(1_001); });
    expect(openSectionStatus).toHaveBeenCalledTimes(3);
    expect(openStatus).toHaveBeenCalledTimes(1);

    // An explicit read asks again.
    await act(async () => harness.value().refreshTelemetry());
    expect(openStatus).toHaveBeenCalledTimes(2);
    expect(batchResource()?.error?.retryable).toBe(false);

    // A 5xx is an outage, and the timer keeps asking.
    openStatus.mockImplementation(async () => { throw new ProductClientError(503, null); });
    await act(async () => harness.value().refreshTelemetry());
    expect(openStatus).toHaveBeenCalledTimes(3);
    expect(batchResource()?.error).toMatchObject({ httpStatus: 503, retryable: true });
    await act(async () => { await vi.advanceTimersByTimeAsync(1_001); });
    expect(openSectionStatus).toHaveBeenCalledTimes(6);
    expect(openStatus).toHaveBeenCalledTimes(4);
  });

  it('settles the batch resource after a follow-up queued behind the refresh pass', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(AT));
    const farFresh = '2030-01-01T01:00:00Z';
    const observed = (batch: { readonly term: string; readonly campus: string }): OpenRefreshStatusV1 => ({
      ...refreshStatus(batch),
      freshness: {
        state: 'FRESH',
        observedAt: AT,
        freshUntil: farFresh,
        lastKnownGoodAgeSeconds: 0,
        uncertainty: null,
      },
    });
    const sectionCalls: ReturnType<typeof deferred<OpenSectionStatusV1>>[] = [];
    const batchCalls: ReturnType<typeof deferred<OpenRefreshStatusV1>>[] = [];
    const openSectionStatus = vi.fn<ProductApiPort['openSectionStatus']>(async () => {
      const call = deferred<OpenSectionStatusV1>();
      sectionCalls.push(call);
      return call.promise;
    });
    const openStatus = vi.fn<ProductApiPort['openStatus']>(async () => {
      const call = deferred<OpenRefreshStatusV1>();
      batchCalls.push(call);
      return call.promise;
    });
    const harness = createHarness('OPEN', false, { openSectionStatus, openStatus });
    const batchResource = () =>
      harness.value().telemetryResources.find((resource) => resource.key === BATCH_A_RESOURCE_KEY);

    // Two persisted Sections of one batch: one pass of two Section reads and
    // one batch read, all slow and answered one after another.
    await act(async () => {
      harness.value().select(SECTION_A);
      harness.value().select(SECTION_C);
    });
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(openSectionStatus).toHaveBeenCalledTimes(2);
    expect(openStatus).toHaveBeenCalledTimes(1);
    expect(batchResource()).toMatchObject({ loading: true });

    sectionCalls[0]!.resolve(sectionStatus(SECTION_A));
    await act(async () => { await vi.advanceTimersByTimeAsync(3_300); });
    sectionCalls[1]!.resolve(sectionStatus(SECTION_C));
    await act(async () => { await vi.advanceTimersByTimeAsync(3_300); });

    // The server walks a refresh while the batch read is still out: the
    // observation asks for a follow-up, which queues behind the pass.
    await emit(harness, observationEvent(farFresh));
    await act(async () => { await vi.advanceTimersByTimeAsync(BATCH_STATUS_COALESCE_MILLISECONDS + 1); });
    expect(openStatus).toHaveBeenCalledTimes(1);

    batchCalls[0]!.resolve(observed(BATCH_A));
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(openStatus).toHaveBeenCalledTimes(2);
    expect(openStatus.mock.calls[1]![0]).toStrictEqual({ contractVersion: 1, batch: BATCH_A });
    // The pass's answer is the newest one this page has: the follow-up that
    // just started has not answered anything yet.
    expect(harness.value().batchStatuses).toHaveLength(1);
    expect(batchResource()).toMatchObject({ loading: true, error: null, lastSuccessAt: AT });

    batchCalls[1]!.resolve(observed(BATCH_A));
    await act(async () => { await vi.advanceTimersByTimeAsync(3_300); });
    expect(openStatus).toHaveBeenCalledTimes(2);
    expect(harness.value().batchStatuses).toHaveLength(1);
    expect(batchResource()).toMatchObject({ loading: false, error: null, lastSuccessAt: AT });
    // Every read this page issued has been answered, so nothing is loading.
    expect(harness.value().telemetryResources.map((resource) => [resource.key, resource.loading]))
      .toEqual(harness.value().telemetryResources.map((resource) => [resource.key, false]));
    expect(harness.value().telemetryLoading).toBe(false);
  });

  it('settles the batch resource for a reloaded desk whose passes overlap', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(AT));
    const farFresh = '2030-01-01T01:00:00Z';
    const observed = (batch: { readonly term: string; readonly campus: string }): OpenRefreshStatusV1 => ({
      ...refreshStatus(batch),
      freshness: {
        state: 'FRESH',
        observedAt: AT,
        freshUntil: farFresh,
        lastKnownGoodAgeSeconds: 0,
        uncertainty: null,
      },
    });
    // A read the page abandons is a read the transport aborts: the promise
    // rejects, exactly as `fetch` does, rather than hanging forever.
    const answerOnAbort = <T,>(
      call: ReturnType<typeof deferred<T>>,
      signal: AbortSignal | undefined,
    ) => {
      if (signal === undefined) return call.promise;
      if (signal.aborted) call.reject(new ProductClientError(null, null));
      signal.addEventListener('abort', () => call.reject(new ProductClientError(null, null)));
      return call.promise;
    };
    const sectionCalls: ReturnType<typeof deferred<OpenSectionStatusV1>>[] = [];
    const batchCalls: ReturnType<typeof deferred<OpenRefreshStatusV1>>[] = [];
    const openSectionStatus = vi.fn<ProductApiPort['openSectionStatus']>(async (_request, signal) => {
      const call = deferred<OpenSectionStatusV1>();
      sectionCalls.push(call);
      return answerOnAbort(call, signal);
    });
    const openStatus = vi.fn<ProductApiPort['openStatus']>(async (_request, signal) => {
      const call = deferred<OpenRefreshStatusV1>();
      batchCalls.push(call);
      return answerOnAbort(call, signal);
    });
    // The real page: two persisted Sections of one batch, and a socket that
    // has not connected yet -- so the selection pass and the connection pass
    // both run on mount, and the second abandons the first.
    const harness = createPersistedHarness([SECTION_A, SECTION_C], 'CLOSED', {
      openSectionStatus,
      openStatus,
    });
    const batchResource = () =>
      harness.value().telemetryResources.find((resource) => resource.key === BATCH_A_RESOURCE_KEY);
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    const liveSections = sectionCalls.slice(-2);
    const liveBatch = batchCalls[batchCalls.length - 1]!;
    expect(batchResource()).toMatchObject({ loading: true });

    // The Section reads answer first, as they do on the real server.
    liveSections[0]!.resolve(sectionStatus(SECTION_A));
    liveSections[1]!.resolve(sectionStatus(SECTION_C));
    await act(async () => { await vi.advanceTimersByTimeAsync(3_300); });

    // The server walks a refresh while the batch read is still out: the
    // observation asks for a follow-up, which queues behind the pass.
    await emit(harness, observationEvent(farFresh));
    await act(async () => { await vi.advanceTimersByTimeAsync(BATCH_STATUS_COALESCE_MILLISECONDS + 1); });

    liveBatch.resolve(observed(BATCH_A));
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(harness.value().batchStatuses).toHaveLength(1);
    const followUp = batchCalls[batchCalls.length - 1]!;
    followUp.resolve(observed(BATCH_A));
    await act(async () => { await vi.advanceTimersByTimeAsync(3_300); });

    // Every read this page issued has been answered. The row may not still
    // be saying it is reading the current state.
    expect(batchResource()).toMatchObject({
      availability: 'CURRENT',
      loading: false,
      error: null,
      lastSuccessAt: AT,
    });
    expect(harness.value().telemetryResources.every((resource) => !resource.loading)).toBe(true);
    expect(harness.value().telemetryLoading).toBe(false);
  });

  it('settles a batch read that answers after the last watch for it stopped', async () => {
    const batchCalls: {
      readonly batch: { readonly term: string; readonly campus: string };
      readonly call: ReturnType<typeof deferred<OpenRefreshStatusV1>>;
    }[] = [];
    const openSectionStatus = vi.fn<ProductApiPort['openSectionStatus']>(async ({ sectionKey }) =>
      sectionStatus(sectionKey));
    const openStatus = vi.fn<ProductApiPort['openStatus']>(async ({ batch }) => {
      const call = deferred<OpenRefreshStatusV1>();
      batchCalls.push({ batch, call });
      return call.promise;
    });
    const harness = createHarness('OPEN', false, { openSectionStatus, openStatus });
    const batchResource = () =>
      harness.value().telemetryResources.find((resource) => resource.key === BATCH_A_RESOURCE_KEY);

    // One Section of another batch is on the desk, so the desk survives what
    // happens to this one.
    await act(async () => harness.value().select(SECTION_E));
    await act(async () => {
      batchCalls.forEach(({ batch, call }) => call.resolve(refreshStatus(batch)));
    });

    // A watch this page never selected is running in BATCH_A, so the pass
    // reads that batch too: its row goes on the desk saying it is reading.
    await emit(harness, startResult([[SECTION_A, ACTIVE_A]]));
    await act(async () => { void harness.value().refreshTelemetry(); });
    expect(batchResource()).toMatchObject({ loading: true });
    const pendingBatchA = batchCalls.filter(({ batch }) => batch.campus === SECTION_A.campus).at(-1)!;

    // The watch ends while that read is still out, so nothing on the desk
    // wants BATCH_A any more -- and then the read answers.
    await emit(harness, stopped(SECTION_A, ACTIVE_A));
    await act(async () => {
      pendingBatchA.call.resolve(refreshStatus(BATCH_A));
      batchCalls.forEach(({ batch, call }) => call.resolve(refreshStatus(batch)));
    });

    // The read this page issued has been answered. Whatever it decided to do
    // with the answer, the row may not still say it is reading.
    expect(batchResource()).toMatchObject({ loading: false });
    expect(harness.value().telemetryResources.filter((resource) => resource.loading)).toEqual([]);
  });

  it('stops saying it is reading when a removal cancels the pass it started', async () => {
    const sectionCalls: ReturnType<typeof deferred<OpenSectionStatusV1>>[] = [];
    const openSectionStatus = vi.fn<ProductApiPort['openSectionStatus']>(async (_request, signal) => {
      const call = deferred<OpenSectionStatusV1>();
      sectionCalls.push(call);
      signal?.addEventListener('abort', () => call.reject(new ProductClientError(null, null)));
      return call.promise;
    });
    const batchCalls: ReturnType<typeof deferred<OpenRefreshStatusV1>>[] = [];
    const openStatus = vi.fn<ProductApiPort['openStatus']>(async (_request, signal) => {
      const call = deferred<OpenRefreshStatusV1>();
      batchCalls.push(call);
      signal?.addEventListener('abort', () => call.reject(new ProductClientError(null, null)));
      return call.promise;
    });
    const harness = createHarness('OPEN', false, { openSectionStatus, openStatus });
    await act(async () => harness.value().select(SECTION_A));
    expect(harness.value().telemetryLoading).toBe(true);

    // Removing a Section the desk was not reading for cancels the pass -- and
    // leaves the selection, so no new pass follows to finish the sentence.
    await act(async () => harness.value().remove(SECTION_D));
    await act(async () => {
      sectionCalls.forEach((call) => call.resolve(sectionStatus(SECTION_A)));
      batchCalls.forEach((call) => call.resolve(refreshStatus(BATCH_A)));
    });

    expect(harness.value().selected).toEqual([SECTION_A]);
    expect(harness.value().telemetryLoading).toBe(false);
    expect(harness.value().telemetryResources.filter((resource) => resource.loading)).toEqual([]);
  });

  it('books a superseded batch read without unsettling the newer one still in flight', async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(AT));
    const farFresh = '2030-01-01T01:00:00Z';
    const observed = (batch: { readonly term: string; readonly campus: string }): OpenRefreshStatusV1 => ({
      ...refreshStatus(batch),
      freshness: {
        state: 'FRESH',
        observedAt: AT,
        freshUntil: farFresh,
        lastKnownGoodAgeSeconds: 0,
        uncertainty: null,
      },
    });
    const batchCalls: ReturnType<typeof deferred<OpenRefreshStatusV1>>[] = [];
    const openStatus = vi.fn<ProductApiPort['openStatus']>(async () => {
      const call = deferred<OpenRefreshStatusV1>();
      batchCalls.push(call);
      return call.promise;
    });
    const harness = createHarness('OPEN', false, { openStatus });
    const batchResource = () =>
      harness.value().telemetryResources.find((resource) => resource.key === BATCH_A_RESOURCE_KEY);

    await act(async () => harness.value().select(SECTION_A));
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(openStatus).toHaveBeenCalledTimes(1);
    batchCalls[0]!.resolve(refreshStatus(BATCH_A));
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(batchResource()).toMatchObject({ loading: false, lastSuccessAt: null });

    // An observation-driven read goes out, and an explicit refresh asks
    // again before it is answered: two reads for one batch are out at once.
    await emit(harness, observationEvent(farFresh));
    await act(async () => { await vi.advanceTimersByTimeAsync(BATCH_STATUS_COALESCE_MILLISECONDS + 1); });
    expect(openStatus).toHaveBeenCalledTimes(2);
    await act(async () => { void harness.value().refreshTelemetry(); });
    expect(openStatus).toHaveBeenCalledTimes(3);
    expect(batchResource()).toMatchObject({ loading: true });

    // The older read lands first: its success is booked, and the row keeps
    // saying a read is out, because one is.
    batchCalls[1]!.resolve(observed(BATCH_A));
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(batchResource()).toMatchObject({
      availability: 'CURRENT',
      loading: true,
      error: null,
      lastSuccessAt: AT,
    });

    // The newer read fails: that is the newest answer, and it settles the row.
    batchCalls[2]!.reject(new ProductClientError(503, null));
    await act(async () => { await vi.advanceTimersByTimeAsync(0); });
    expect(batchResource()).toMatchObject({
      availability: 'LKG',
      loading: false,
      error: { httpStatus: 503, retryable: true },
      lastSuccessAt: AT,
    });
    expect(harness.value().telemetryLoading).toBe(false);
    expect(harness.value().batchStatuses).toHaveLength(1);
  });

  it('reduces active start/stop and keeps an active watch when its audible cap is reached', async () => {
    const harness = createHarness();
    await selectAndStart(harness, [SECTION_A]);
    expect(harness.watch.commands[0]).toMatchObject({ type: 'START_WATCH' });

    await emit(harness, startResult([[SECTION_A, ACTIVE_A]]));
    expect(harness.value().pending).toEqual([]);
    expect(harness.value().active).toMatchObject([{
      activeWatchId: ACTIVE_A,
      sectionKey: SECTION_A,
      policy: DEFAULT_WATCH_POLICY,
    }]);

    await emit(harness, audibleCap(1));
    expect(harness.value().active).toHaveLength(1);
    expect(harness.value().notices).toContainEqual(expect.objectContaining({
      code: 'AUDIO_CAP_REACHED',
      sectionKey: SECTION_A,
      detail: '3/3',
    }));

    await act(async () => harness.value().stop(harness.value().active[0]!));
    expect(harness.watch.commands.at(-1)).toMatchObject({
      type: 'STOP_WATCH',
      watch: { activeWatchId: ACTIVE_A, sectionKey: SECTION_A },
    });
    await emit(harness, stopped(SECTION_A, ACTIVE_A));
    expect(harness.value().active).toEqual([]);
    expect(harness.value().selected).toEqual([SECTION_A]);
    expect(harness.value().notices).toContainEqual(expect.objectContaining({
      code: 'WATCH_STOPPED',
      sectionKey: SECTION_A,
    }));
  });

  it('deduplicates repeated audible-cap notices for the same active watch', async () => {
    const harness = createHarness();
    await selectAndStart(harness, [SECTION_A]);
    await emit(harness, startResult([[SECTION_A, ACTIVE_A]]));

    for (let ordinal = 1; ordinal <= 6; ordinal += 1) {
      await emit(harness, audibleCap(ordinal));
    }

    expect(harness.value().notices.filter((notice) => notice.code === 'AUDIO_CAP_REACHED'))
      .toHaveLength(1);
  });

  it('allows a new audible-cap notice after reset and after the watch stops', async () => {
    const harness = createHarness();
    await selectAndStart(harness, [SECTION_A]);
    await emit(harness, startResult([[SECTION_A, ACTIVE_A]]));
    await emit(harness, audibleCap(1));

    await act(async () => harness.value().resetAudibleCount(harness.value().active[0]!));
    expect(harness.watch.commands.at(-1)).toMatchObject({
      type: 'RESET_AUDIBLE_COUNT',
      watch: { activeWatchId: ACTIVE_A, sectionKey: SECTION_A },
    });
    await emit(harness, audibleCap(2));
    expect(harness.value().notices.filter((notice) => notice.code === 'AUDIO_CAP_REACHED'))
      .toHaveLength(2);

    await emit(harness, stopped(SECTION_A, ACTIVE_A));
    await emit(harness, startResult([[SECTION_A, ACTIVE_A]]));
    await emit(harness, audibleCap(3));
    expect(harness.value().notices.filter((notice) => notice.code === 'AUDIO_CAP_REACHED'))
      .toHaveLength(3);
  });

  it('reports every cue outcome and makes autoplay/device failures visible', async () => {
    const harness = createHarness();
    await selectAndStart(harness, [SECTION_A]);
    await emit(harness, startResult([[SECTION_A, ACTIVE_A]]));

    const outcomes: readonly WatchCueOutcome[] = [
      'STARTED',
      'MUTED',
      'AUTOPLAY_BLOCKED',
      'FAILED',
    ];
    for (const [index, outcome] of outcomes.entries()) {
      harness.audio.outcomes.push(outcome);
      await emit(harness, {
        type: 'AUDIO_DISPOSITION',
        audio: {
          disposition: 'CUE_REQUESTED',
          cue: {
            cueId: `30000000-0000-4000-8000-${index.toString().padStart(12, '0')}`,
            activeWatchId: ACTIVE_A,
            sectionKey: SECTION_A,
            trigger: {
              kind: 'ONE_SHOT_OBSERVATION',
              observationId: `40000000-0000-4000-8000-${index.toString().padStart(12, '0')}`,
            },
            emittedAt: LATER,
          },
        },
      });
      expect(harness.watch.commands.at(-1)).toMatchObject({
        type: 'REPORT_CUE_OUTCOME',
        report: {
          activeWatchId: ACTIVE_A,
          sectionKey: SECTION_A,
          outcome,
        },
      });
    }

    expect(harness.audio.play).toHaveBeenCalledTimes(4);
    expect(harness.value().notices).toEqual(expect.arrayContaining([
      expect.objectContaining({ code: 'AUDIO_BLOCKED', sectionKey: SECTION_A }),
      expect.objectContaining({ code: 'AUDIO_FAILED', sectionKey: SECTION_A }),
    ]));
    expect(harness.value().audioState).toBe('FAILED');
  });

  it('targets continuous A/C/D episodes for individual ack, ack-all, and timed-out resume', async () => {
    const harness = createHarness();
    await selectAndStart(harness, [SECTION_A, SECTION_C, SECTION_D], CONTINUOUS_POLICY);
    await emit(harness, startResult([
      [SECTION_A, ACTIVE_A],
      [SECTION_C, ACTIVE_C],
      [SECTION_D, ACTIVE_D],
    ]));
    const episodeA = episode(SECTION_A, ACTIVE_A, 'a2');
    const episodeC = episode(SECTION_C, ACTIVE_C, 'c2');
    const episodeD = episode(SECTION_D, ACTIVE_D, 'd2');
    await emit(harness, { type: 'EPISODE_UPDATED', episode: episodeA });
    await emit(harness, { type: 'EPISODE_UPDATED', episode: episodeC });
    await emit(harness, { type: 'EPISODE_UPDATED', episode: episodeD });
    await emit(harness, {
      type: 'AUDIO_DISPOSITION',
      audio: {
        disposition: 'CONTINUOUS_MIXER_ACTIVE',
        episodeIds: [episodeA.episodeId, episodeC.episodeId, episodeD.episodeId],
        emittedAt: LATER,
      },
    });
    expect(harness.value().continuousEpisodeIds).toEqual([
      episodeA.episodeId,
      episodeC.episodeId,
      episodeD.episodeId,
    ]);
    expect(harness.audio.startContinuous).toHaveBeenLastCalledWith(70, false);

    await act(async () => harness.value().setVolume(35));
    expect(harness.audio.startContinuous).toHaveBeenLastCalledWith(35, false);
    await act(async () => harness.value().setMuted(true));
    expect(harness.audio.stopContinuous).toHaveBeenCalled();
    await act(async () => harness.value().setMuted(false));
    expect(harness.audio.startContinuous).toHaveBeenLastCalledWith(35, false);

    await act(async () => harness.value().acknowledge(episodeA));
    expect(harness.watch.commands.at(-1)).toEqual({
      type: 'ACKNOWLEDGE_EPISODE',
      episode: {
        activeWatchId: ACTIVE_A,
        episodeId: episodeA.episodeId,
        sectionKey: SECTION_A,
      },
    });
    await act(async () => harness.value().acknowledgeAll());
    expect(harness.watch.commands.at(-1)).toEqual({ type: 'ACKNOWLEDGE_ALL_EPISODES' });

    await emit(harness, {
      type: 'AUDIO_DISPOSITION',
      audio: {
        disposition: 'CONTINUOUS_MIXER_STOPPED',
        reason: 'NO_UNACKNOWLEDGED_EPISODES',
        emittedAt: LATER,
      },
    });
    expect(harness.value().continuousEpisodeIds).toEqual([]);
    expect(harness.audio.stopContinuous).toHaveBeenCalled();

    const timedOutD = episode(SECTION_D, ACTIVE_D, 'd2', 'TIMED_OUT');
    await emit(harness, { type: 'EPISODE_UPDATED', episode: timedOutD });
    const storedTimedOutD = harness.value().episodes.find((value) => value.episodeId === timedOutD.episodeId);
    expect(storedTimedOutD?.state).toBe('TIMED_OUT');
    await act(async () => harness.value().resume(storedTimedOutD!));
    expect(harness.watch.commands.at(-1)).toEqual({
      type: 'RESUME_TIMED_OUT_EPISODE',
      episode: {
        activeWatchId: ACTIVE_D,
        episodeId: timedOutD.episodeId,
        sectionKey: SECTION_D,
      },
    });
  });

  it('clears connection-owned active state but retains selection without automatic restart', async () => {
    const harness = createHarness();
    await selectAndStart(harness, [SECTION_A]);
    await emit(harness, startResult([[SECTION_A, ACTIVE_A]]));
    await emit(harness, {
      type: 'OPEN_OBSERVATION',
      fanout: {
        contractVersion: 1,
        activeWatchId: ACTIVE_A,
        observation: {
          contractVersion: 1,
          observationId: '20000000-0000-4000-8000-000000000001',
          refreshObservationId: '20000000-0000-4000-8000-000000000002',
          batch: { term: SECTION_A.term, campus: SECTION_A.campus },
          sectionKey: SECTION_A,
          pullSequence: 1,
          catalogContentVersion: 1,
          state: 'OPEN',
          observedAt: AT,
          freshUntil: LATER,
          schedulerLagMilliseconds: 4,
          counterSnapshot: {
            runCounts: COUNTS,
            todayCounts: COUNTS,
            rutgersDay: '2030-01-01',
            dayTimezone: 'America/New_York',
          },
        },
      },
    });
    expect(harness.value().observations).toHaveLength(1);
    const activeEpisode = episode(SECTION_A, ACTIVE_A, 'a2');
    await emit(harness, { type: 'EPISODE_UPDATED', episode: activeEpisode });
    const startsBeforeDisconnect = harness.watch.commands.filter(({ type }) => type === 'START_WATCH');
    expect(startsBeforeDisconnect).toHaveLength(1);

    const connectsBeforeDisconnect = harness.watch.connect.mock.calls.length;
    await act(async () => {
      harness.value().disconnect();
      harness.watch.recover({ phase: 'STOPPED_BY_USER', attempt: 0, nextAttemptAt: null });
      harness.watch.transition('CLOSED');
    });
    expect(harness.watch.disconnect).toHaveBeenCalledOnce();
    expect(harness.value().connection).toBe('CLOSED');
    expect(harness.value().active).toEqual([]);
    expect(harness.value().observations).toEqual([]);
    expect(harness.value().episodes).toEqual([]);
    expect(harness.value().selected).toEqual([SECTION_A]);
    expect(harness.value().notices).toContainEqual(expect.objectContaining({
      code: 'CONNECTION_LOST',
    }));

    // The barrier. Re-rendering the provider must not reach for the
    // connection the user just gave up: `connect()` is an explicit decision,
    // and calling it from a render is how a Disconnect gets undone by the
    // next state change that happens to arrive.
    await act(async () => {
      harness.rerenderVolume(71);
      await Promise.resolve();
    });
    expect(harness.watch.connect.mock.calls.length).toBe(connectsBeforeDisconnect);
    expect(harness.value().recovery.phase).toBe('STOPPED_BY_USER');

    // Only the user asking again lifts it -- and then this page's own plan
    // comes back, because they never stopped watching the Section, only the
    // connection.
    await act(async () => {
      harness.value().reconnect();
      harness.watch.recover({ phase: 'IDLE', attempt: 0, nextAttemptAt: null });
      harness.watch.transition('OPEN');
    });
    expect(harness.value().connection).toBe('OPEN');
    expect(harness.watch.commands.filter(({ type }) => type === 'START_WATCH')).toHaveLength(2);
    await waitFor(() => expect(harness.value().selected).toEqual([SECTION_A]));
  });

  it('re-arms what this page started when the connection comes back on its own', async () => {
    const harness = createHarness('IDLE');
    await selectAndStart(harness, [SECTION_A]);
    expect(harness.watch.connect).toHaveBeenCalledOnce();
    expect(harness.value().pending).toEqual([SECTION_A]);

    await act(async () => harness.watch.transition('ERROR'));
    expect(harness.value().pending).toEqual([]);
    expect(harness.value().active).toEqual([]);
    expect(harness.value().selected).toEqual([SECTION_A]);
    expect(harness.value().notices).toContainEqual(expect.objectContaining({
      code: 'CONNECTION_LOST',
    }));

    // The attempt the user made did not survive the failed connection, but
    // the decision they made did. Recovery reconnects, and the Section they
    // pressed Start on is armed again -- exactly once, from the plan rather
    // than from the selection.
    await act(async () => harness.watch.transition('OPEN'));
    const starts = harness.watch.commands.filter((command) => command.type === 'START_WATCH');
    expect(starts).toHaveLength(1);
    expect(starts[0]).toEqual({
      type: 'START_WATCH',
      items: [{ sectionKey: SECTION_A, policy: DEFAULT_WATCH_POLICY }],
    });
  });

  it('does not re-arm a Section the user stopped before the connection came back', async () => {
    const harness = createHarness('OPEN');
    await selectAndStart(harness, [SECTION_A]);
    await emit(harness, {
      type: 'START_RESULT',
      result: {
        contractVersion: 1,
        items: [{
          sectionKey: SECTION_A,
          status: 'ACTIVE',
          activeWatchId: ACTIVE_A,
          startedAt: AT,
          reason: null,
        }],
      },
    });
    await act(async () => {
      const [watch] = harness.value().active;
      if (watch === undefined) throw new Error('expected an active watch');
      harness.value().stop(watch);
    });

    await act(async () => harness.watch.transition('CLOSED'));
    await act(async () => harness.watch.transition('OPEN'));

    // One START from the press, one STOP from the stop, and nothing from the
    // reconnect: a watch the user stopped stays stopped however many times
    // the connection comes back.
    expect(harness.watch.commands.filter(({ type }) => type === 'START_WATCH')).toHaveLength(1);
  });
});
