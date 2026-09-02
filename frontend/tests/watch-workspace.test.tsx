// @vitest-environment jsdom

import axe from 'axe-core';
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { BcspI18nProvider } from '../src/ui/shared/i18n/runtime';
import { AppRouterProvider } from '../src/ui/shared/routing';
import type { SupportedLocale } from '../src/ui/shared/i18n/contract';
import type {
  OpenCircuitReason,
  OpenCircuitState,
  OpenFreshnessState,
  OpenEpisodeV1,
  OpenRefreshStatusV1,
  OpenSectionStatusV1,
  ProductApiPort,
  ProductRuntimePort,
  SectionKey,
  TraceId,
  WatchAudioCueV1,
  WatchClientPort,
  WatchClientCommandV1,
  WatchConnectionState,
  WatchRecoveryState,
  WatchCueOutcome,
  WatchPolicyV1,
  WatchServerEventV1,
  WatchServerListener,
  WatchStateListener,
  WsServerEnvelope,
} from '../src/ui/shared/product';
import { ProductClientError } from '../src/ui/shared/product';
import {
  LiveWatchProvider,
  SectionSelectionAction,
  WATCH_WORKSPACE_CSS,
  WatchNotificationRegion,
  WatchToastRegion,
  WatchWorkspace,
} from '../src/ui/shared/watch';
import { useLiveWatch, type LiveWatchValue } from '../src/ui/shared/watch/LiveWatchProvider';
import {
  WatchAudioController,
  type WatchAudioUnlockResult,
} from '../src/ui/shared/watch/audio';
import type {
  WatchNotificationPermission,
  WatchNotificationPort,
} from '../src/ui/shared/watch/notification';

const NOW = '2026-07-15T04:00:00.000Z';
const FRESH_UNTIL = '2026-07-15T04:00:30.000Z';

function section(index: number, campus = 'NB'): SectionKey {
  return {
    campus,
    index: index.toString().padStart(5, '0'),
    term: '2026-9',
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

class FakeWatchClient implements WatchClientPort {
  state: WatchConnectionState;
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
  readonly connect = vi.fn(() => this.setState('OPEN'));
  readonly disconnect = vi.fn(() => this.setState('CLOSED'));
  readonly #serverListeners = new Set<WatchServerListener>();
  readonly #stateListeners = new Set<WatchStateListener>();
  #messageSequence = 0;

  constructor(state: WatchConnectionState = 'OPEN') {
    this.state = state;
  }

  send(command: WatchClientCommandV1): TraceId {
    this.commands.push(command);
    this.#messageSequence += 1;
    return `client-message-${this.#messageSequence}`;
  }

  subscribe(listener: WatchServerListener): () => void {
    this.#serverListeners.add(listener);
    return () => {
      this.#serverListeners.delete(listener);
    };
  }

  subscribeState(listener: WatchStateListener): () => void {
    this.#stateListeners.add(listener);
    return () => {
      this.#stateListeners.delete(listener);
    };
  }

  emit(payload: WatchServerEventV1): void {
    this.#messageSequence += 1;
    const envelope: WsServerEnvelope<WatchServerEventV1> = {
      protocolVersion: 1,
      messageId: `server-message-${this.#messageSequence}`,
      payload,
    };
    this.#serverListeners.forEach((listener) => listener(envelope));
  }

  setState(state: WatchConnectionState): void {
    this.state = state;
    this.#stateListeners.forEach((listener) => listener(state));
  }
}

class FakeAudioController extends WatchAudioController {
  readonly unlockCalls = vi.fn(async (): Promise<WatchAudioUnlockResult> => 'READY');
  readonly playCalls = vi.fn(
    (_cue: WatchAudioCueV1, _volume: number, _muted: boolean): WatchCueOutcome => 'STARTED',
  );
  readonly previewCalls = vi.fn((_volume: number): WatchCueOutcome => 'STARTED');
  readonly continuousCalls = vi.fn(
    (_volume: number, _muted: boolean): WatchCueOutcome => 'STARTED',
  );
  readonly stopCalls = vi.fn();
  readonly disposeCalls = vi.fn();

  override unlock(): Promise<WatchAudioUnlockResult> {
    return this.unlockCalls();
  }

  override play(cue: WatchAudioCueV1, volume: number, muted: boolean): WatchCueOutcome {
    return this.playCalls(cue, volume, muted);
  }

  override preview(volume: number): WatchCueOutcome {
    return this.previewCalls(volume);
  }

  override startContinuous(volume: number, muted: boolean): WatchCueOutcome {
    return this.continuousCalls(volume, muted);
  }

  override stopContinuous(): void {
    this.stopCalls();
  }

  override dispose(): void {
    this.disposeCalls();
  }
}

interface StatusProfile {
  readonly freshness: OpenFreshnessState;
  readonly circuit: OpenCircuitState;
  readonly circuitReason: OpenCircuitReason | null;
  readonly lag: number;
}

function statusProfile(campus: string): StatusProfile {
  if (campus === 'NK') {
    return {
      freshness: 'STALE',
      circuit: 'RETRY_AFTER',
      circuitReason: 'RATE_LIMITED',
      lag: 902,
    };
  }
  if (campus === 'CM') {
    return {
      freshness: 'UNKNOWN',
      circuit: 'FATAL_DIAGNOSTIC',
      circuitReason: 'NON_JSON',
      lag: 1_744,
    };
  }
  return {
    freshness: 'FRESH',
    circuit: 'CLOSED',
    circuitReason: null,
    lag: 487,
  };
}

function counts() {
  return {
    runCounts: { attempted: 12, succeeded: 9, failed: 2, empty: 1 },
    todayCounts: { attempted: 42, succeeded: 36, failed: 4, empty: 2 },
    rutgersDay: '2026-07-15',
    dayTimezone: 'America/New_York' as const,
  };
}

function batchStatus(batch: { readonly term: string; readonly campus: string }): OpenRefreshStatusV1 {
  const profile = statusProfile(batch.campus);
  return {
    contractVersion: 1,
    batch,
    catalogContentVersion: profile.freshness === 'UNKNOWN' ? null : 7,
    latestAttempt: {
      attemptSequence: 12,
      startedAt: NOW,
      completedAt: NOW,
      classification: profile.freshness === 'FRESH' ? 'VALID_APPLIED' : 'FAILED',
      failureClass: profile.freshness === 'FRESH' ? null : 'HTTP_429',
    },
    latestFailure: profile.freshness === 'FRESH' ? null : {
      attemptSequence: 12,
      failedAt: NOW,
      failureClass: profile.freshness === 'UNKNOWN' ? 'NON_JSON' : 'HTTP_429',
    },
    lastValidObservation: profile.freshness === 'UNKNOWN' ? null : {
      observationId: `batch-observation-${batch.campus}`,
      observationSequence: 9,
      catalogContentVersion: 7,
      observedAt: NOW,
      canonicalSetHash: 'a'.repeat(64),
      stateHash: 'b'.repeat(64),
    },
    lastBodyChangeAt: profile.freshness === 'UNKNOWN' ? null : NOW,
    lastStateChangeAt: profile.freshness === 'UNKNOWN' ? null : NOW,
    freshness: {
      state: profile.freshness,
      observedAt: profile.freshness === 'UNKNOWN' ? null : NOW,
      freshUntil: profile.freshness === 'FRESH' ? FRESH_UNTIL : null,
      lastKnownGoodAgeSeconds: profile.freshness === 'UNKNOWN' ? null : 18,
      uncertainty: profile.freshness === 'FRESH'
        ? null
        : profile.freshness === 'STALE'
          ? 'LATEST_ATTEMPT_FAILED'
          : 'NEVER_OBSERVED',
    },
    scheduler: {
      lane: 'ACTIVE_WATCH',
      requestedGeneralIntervalSeconds: 30,
      requestedEffectiveIntervalSeconds: 10,
      activeWatchCount: 3,
      nextDueAt: FRESH_UNTIL,
      inFlight: false,
      schedulerLagMilliseconds: profile.lag,
      actualStartToStartIntervalMilliseconds: profile.freshness === 'UNKNOWN' ? null : 12_500,
      failureStreak: profile.freshness === 'FRESH' ? 0 : 2,
    },
    circuit: {
      state: profile.circuit,
      reason: profile.circuitReason,
      openedAt: profile.circuit === 'CLOSED' ? null : NOW,
      retryAt: profile.circuit === 'RETRY_AFTER' ? FRESH_UNTIL : null,
      diagnosticRecheckRequired: profile.circuit === 'FATAL_DIAGNOSTIC',
    },
    counterSnapshot: counts(),
  };
}

function sectionStatus(sectionKey: SectionKey): OpenSectionStatusV1 {
  const profile = statusProfile(sectionKey.campus);
  return {
    contractVersion: 1,
    sectionKey,
    state: profile.freshness === 'FRESH'
      ? 'OPEN'
      : profile.freshness === 'STALE'
        ? 'CLOSED'
        : 'UNKNOWN',
    lastObservationId: profile.freshness === 'UNKNOWN' ? null : `section-observation-${sectionKey.index}`,
    catalogContentVersion: profile.freshness === 'UNKNOWN' ? null : 7,
    freshness: {
      state: profile.freshness,
      observedAt: profile.freshness === 'UNKNOWN' ? null : NOW,
      freshUntil: profile.freshness === 'FRESH' ? FRESH_UNTIL : null,
      lastKnownGoodAgeSeconds: profile.freshness === 'UNKNOWN' ? null : 18,
      uncertainty: profile.freshness === 'FRESH'
        ? null
        : profile.freshness === 'STALE'
          ? 'STALE_LAST_KNOWN_GOOD'
          : 'NEVER_OBSERVED',
    },
    schedulerLagMilliseconds: profile.lag,
    counterSnapshot: counts(),
  };
}

function productApi(overrides: Partial<ProductApiPort> = {}): ProductApiPort {
  const unused = vi.fn(async () => {
    throw new Error('unused product API');
  });
  return {
    catalogDiscovery: unused,
    courseDetail: unused,
    filterSchema: unused,
    openSectionStatus: vi.fn(async ({ sectionKey }) => sectionStatus(sectionKey)),
    openStatus: vi.fn(async ({ batch }) => batchStatus(batch)),
    searchCourses: unused,
    searchSections: unused,
    sectionDetail: unused,
    ...overrides,
  } as ProductApiPort;
}

function runtime(
  watch: FakeWatchClient,
  productOverrides: Partial<ProductApiPort> = {},
): ProductRuntimePort {
  return {
    dispose: vi.fn(),
    product: productApi(productOverrides),
    watch,
  };
}

function SelectionActions({ sections }: { readonly sections: readonly SectionKey[] }) {
  return (
    <div aria-label="Section selection fixtures">
      {sections.map((sectionKey) => (
        <SectionSelectionAction key={sectionKey.index} sectionKey={sectionKey} />
      ))}
    </div>
  );
}

/** The browser's notification seam, recording what was asked and posted. */
class FakeNotifications implements WatchNotificationPort {
  permission: WatchNotificationPermission = 'default';
  /** What the browser will answer, and whether it was ever asked. */
  answer: WatchNotificationPermission = 'granted';
  readonly asked: number[] = [];
  readonly posted: Array<{ title: string; body: string; tag: string }> = [];
  #sequence = 0;

  async requestPermission(): Promise<WatchNotificationPermission> {
    this.#sequence += 1;
    this.asked.push(this.#sequence);
    if (this.permission !== 'default') return this.permission;
    this.permission = this.answer;
    return this.permission;
  }

  show(title: string, body: string, tag: string): void {
    this.posted.push({ title, body, tag });
  }
}

function LiveWatchProbe({ publish }: { readonly publish: (value: LiveWatchValue) => void }) {
  publish(useLiveWatch());
  return null;
}

interface RenderWatchOptions {
  readonly notifications?: WatchNotificationPort;
  readonly pageVisibility?: () => boolean;
  /** Mounts the production posting region so port.show is reachable. */
  readonly notificationRegion?: boolean;
  /** Read-only observation of the context; never used to drive it. */
  readonly probe?: (value: LiveWatchValue) => void;
}

function renderWatch(
  sections: readonly SectionKey[],
  watch = new FakeWatchClient(),
  audio = new FakeAudioController(),
  locale: SupportedLocale = 'en-US',
  productOverrides: Partial<ProductApiPort> = {},
  initialWatchableTerms?: readonly string[],
  initialSelected: readonly SectionKey[] = [],
  options: RenderWatchOptions = {},
) {
  const result = render(
    <AppRouterProvider initialPath="/">
      <BcspI18nProvider initialLocale={locale}>
        <LiveWatchProvider
          audio={audio}
          initialSelected={initialSelected}
          initialWatchableTerms={initialWatchableTerms}
          runtime={runtime(watch, productOverrides)}
          {...(options.notifications === undefined ? {} : { notifications: options.notifications })}
          {...(options.pageVisibility === undefined ? {} : { pageVisibility: options.pageVisibility })}
        >
          <SelectionActions sections={sections} />
          <WatchWorkspace />
          <WatchToastRegion />
          {options.notificationRegion === true ? <WatchNotificationRegion /> : null}
          {options.probe === undefined ? null : <LiveWatchProbe publish={options.probe} />}
        </LiveWatchProvider>
      </BcspI18nProvider>
    </AppRouterProvider>,
  );
  return { ...result, audio, watch };
}

function activeStart(sectionKey: SectionKey, ordinal: number): WatchServerEventV1 {
  return {
    type: 'START_RESULT',
    result: {
      contractVersion: 1,
      activeWatchCount: ordinal,
      items: [{
        status: 'ACTIVE',
        sectionKey,
        activeWatchId: `active-watch-${ordinal}`,
        startedAt: NOW,
      }],
    },
  };
}

function watchStopped(sectionKey: SectionKey): WatchServerEventV1 {
  return {
    type: 'WATCH_STOPPED',
    stopped: {
      contractVersion: 1,
      activeWatchId: 'active-watch-status-toast',
      sectionKey,
      reason: 'USER_REQUESTED',
      stoppedAt: NOW,
    },
  };
}

function continuousEpisode(
  sectionKey: SectionKey,
  ordinal: number,
  state: 'UNACKNOWLEDGED' | 'TIMED_OUT',
): OpenEpisodeV1 {
  return {
    contractVersion: 1,
    episodeId: `episode-${ordinal}`,
    activeWatchId: `active-watch-${ordinal}`,
    sectionKey,
    state,
    notificationMode: 'CONTINUOUS',
    continuousDuration: { kind: 'FINITE', seconds: 600 },
    maxAudible: 3,
    audibleCount: 1,
    firstObservedAt: NOW,
    lastObservedAt: NOW,
    observationCount: ordinal + 1,
    latestObservationId: `episode-observation-${ordinal}`,
    stateChangedAt: NOW,
  };
}

function emitVisibleAlert(
  watch: FakeWatchClient,
  episode: OpenEpisodeV1,
  ordinal: number,
): void {
  watch.emit({ type: 'EPISODE_UPDATED', episode });
  watch.emit({
    type: 'ALERT_UPDATED',
    alert: {
      contractVersion: 1,
      alertId: `alert-${ordinal}`,
      disposition: 'OPENED',
      visible: true,
      episode,
    },
  });
}

function metricValue(label: string): string | null | undefined {
  const status = screen.getByLabelText('Watch session status');
  const term = within(status).getByText(label);
  return term.parentElement?.querySelector('dd')?.textContent;
}

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  document.documentElement.lang = '';
});

describe('Watch workspace product flow', () => {
  it('rejects new out-of-range selections while retaining an old one for removal', () => {
    const current = section(1);
    const old = { ...section(2), term: '2025-9' };
    renderWatch(
      [current, old],
      new FakeWatchClient(),
      new FakeAudioController(),
      'en-US',
      {},
      ['2026-9', '2027-0'],
      [old],
    );

    const oldAdd = screen.getByRole('button', { name: /Section 00002/u }) as HTMLButtonElement;
    expect(oldAdd.disabled).toBe(false, 'an existing selection remains removable');
    expect(screen.getAllByText('Outside watch range').length).toBeGreaterThan(0);
    expect(screen.getByText(/selection is retained so you can remove it/u)).toBeTruthy();
    const start = screen.getByRole('button', { name: /Start selected · 0/u }) as HTMLButtonElement;
    expect(start.disabled).toBe(true);
    fireEvent.click(oldAdd);
    expect(screen.queryByText(/selection is retained so you can remove it/u)).toBeNull();

    const oldUnselected = screen.getByRole('button', { name: /Section 00002/u }) as HTMLButtonElement;
    expect(oldUnselected.disabled).toBe(true);
    fireEvent.click(oldUnselected);
    expect(screen.getByLabelText('0 of 9 selected')).toBeTruthy();
  });

  it('translates the Chinese Watch chrome while preserving Section identifiers', () => {
    const sectionKey = section(1);
    renderWatch(
      [sectionKey],
      new FakeWatchClient(),
      new FakeAudioController(),
      'zh-CN',
    );

    expect(screen.getByRole('heading', { name: '实时监看控制台' })).toBeTruthy();
    expect(screen.getByText('提醒方式')).toBeTruthy();
    expect(screen.getByRole('checkbox', { name: '此页面听不见时改用浏览器消息' })).toBeTruthy();
    fireEvent.click(screen.getByRole('button', {
      name: '将课节 00001 加入监看列表',
    }));
    expect(screen.getByLabelText('已选 1 / 9')).toBeTruthy();
    expect(screen.getByText('00001')).toBeTruthy();
    expect(screen.getByText('2026-9 / NB')).toBeTruthy();
  });

  it('selects the first nine Sections and explicitly rejects the tenth without replacement', async () => {
    const sections = Array.from({ length: 10 }, (_, index) => section(index + 1));
    renderWatch(sections);

    for (const sectionKey of sections.slice(0, 9)) {
      fireEvent.click(screen.getByRole('button', {
        name: `Add Section ${sectionKey.index} to the watch list`,
      }));
    }

    expect(screen.getByLabelText('9 of 9 selected')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', {
      name: 'Add Section 00010 to the watch list',
    }));

    expect(await screen.findByRole('heading', { name: 'Selection limit reached' })).toBeTruthy();
    expect(screen.getByText(/00010 \/ 2026-9 \/ NB/u)).toBeTruthy();
    for (const sectionKey of sections.slice(0, 9)) {
      expect(screen.getByRole('button', {
        name: `Remove Section ${sectionKey.index} from the watch list`,
      }).getAttribute('aria-pressed')).toBe('true');
    }
    expect(screen.getByRole('button', {
      name: 'Add Section 00010 to the watch list',
    }).getAttribute('aria-pressed')).toBe('false');
    expect(screen.getByLabelText('9 of 9 selected')).toBeTruthy();
  });

  it('keeps selection inactive until START_RESULT and sends the default one-shot policy', async () => {
    const sectionKey = section(1);
    const { audio, container, watch } = renderWatch([sectionKey]);

    fireEvent.click(screen.getByRole('button', { name: 'Add Section 00001 to the watch list' }));
    expect(metricValue('Selected')).toBe('1');
    expect(metricValue('Active')).toBe('0');
    expect(container.querySelector('[data-state="SELECTED"]')?.textContent).toBe('Added · not watching');
    expect(screen.getByRole('link', { name: 'Go to Watch desk to configure alerts and start' })).toBeTruthy();

    fireEvent.click(screen.getByRole('button', { name: /Start selected/u }));
    await waitFor(() => expect(watch.commands).toHaveLength(1));
    expect(watch.commands).toEqual([{
      type: 'START_WATCH',
      items: [{
        sectionKey,
        policy: {
          notificationMode: 'ONE_SHOT',
          maxAudible: 3,
          continuousDuration: { kind: 'FINITE', seconds: 600 },
        },
      }],
    }]);
    expect(audio.unlockCalls).toHaveBeenCalledOnce();
    expect(metricValue('Selected')).toBe('1');
    expect(metricValue('Active')).toBe('0');
    expect(container.querySelector('.watch-workspace__item [data-state="SELECTED"]')?.textContent)
      .toBe('Starting watch');

    act(() => watch.emit(activeStart(sectionKey, 1)));
    await waitFor(() => expect(metricValue('Active')).toBe('1'));
    expect(metricValue('Selected')).toBe('1');
    expect(container.querySelector('.watch-workspace__item [data-state="READY"]')?.textContent)
      .toBe('Watching');
    expect(screen.getByText(/One-shot .* max 3 .* 600s/u)).toBeTruthy();
  });

  it('plays a real preview when the sound-test control is used', async () => {
    const { audio } = renderWatch([]);
    fireEvent.click(screen.getByRole('button', { name: 'Enable / test sound' }));
    await waitFor(() => expect(audio.unlockCalls).toHaveBeenCalledOnce());
    expect(audio.previewCalls).toHaveBeenCalledWith(70);
  });

  it('does not persist an unchanged policy when the parent callback identity changes', async () => {
    const watch = new FakeWatchClient();
    const audio = new FakeAudioController();
    const watchRuntime = runtime(watch);
    const firstCallback = vi.fn<(policy: WatchPolicyV1) => void>();
    const secondCallback = vi.fn<(policy: WatchPolicyV1) => void>();
    const tree = (onPolicyChange: (policy: WatchPolicyV1) => void) => (
      <AppRouterProvider initialPath="/">
        <BcspI18nProvider initialLocale="en-US">
          <LiveWatchProvider audio={audio} runtime={watchRuntime}>
            <WatchWorkspace onPolicyChange={onPolicyChange} />
          </LiveWatchProvider>
        </BcspI18nProvider>
      </AppRouterProvider>
    );
    const view = render(tree(firstCallback));

    expect(firstCallback).not.toHaveBeenCalled();
    view.rerender(tree(secondCallback));
    expect(firstCallback).not.toHaveBeenCalled();
    expect(secondCallback).not.toHaveBeenCalled();

    fireEvent.change(screen.getByRole('spinbutton', {
      name: 'Maximum audible cues per Section',
    }), { target: { value: '4' } });
    await waitFor(() => expect(secondCallback).toHaveBeenCalledOnce());
    expect(secondCallback).toHaveBeenCalledWith({
      notificationMode: 'ONE_SHOT',
      maxAudible: 4,
      continuousDuration: { kind: 'FINITE', seconds: 600 },
    });
  });

  it('keeps the empty Watch path compact until a Section or alert needs batch actions', () => {
    renderWatch([]);

    expect(screen.getByRole('heading', { name: 'Live watch console' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: /Start selected/u })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Apply policy to active' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Acknowledge all' })).toBeNull();
  });

  it('uses plain-language connection state and keeps WebSocket in diagnostics', () => {
    const watch = new FakeWatchClient('IDLE');
    const view = renderWatch([], watch);

    const status = view.container.querySelector('.watch-workspace__status-strip');
    if (!(status instanceof HTMLElement)) throw new Error('Watch status strip was not rendered');
    expect(within(status).getByText('Live watch has not started')).toBeTruthy();
    expect(within(status).queryByText('WebSocket')).toBeNull();
    expect(view.container.querySelector('.watch-workspace__diagnostics')?.textContent)
      .toContain('WebSocket / IDLE');
  });

  it('keeps a blocked-audio warning visible after live watch becomes active', async () => {
    const sectionKey = section(1);
    const audio = new FakeAudioController();
    audio.unlockCalls.mockResolvedValueOnce('BLOCKED');
    const view = renderWatch([sectionKey], new FakeWatchClient(), audio);

    fireEvent.click(screen.getByRole('button', { name: 'Add Section 00001 to the watch list' }));
    fireEvent.click(screen.getByRole('button', { name: /Start selected/u }));
    await waitFor(() => expect(view.watch.commands).toHaveLength(1));
    act(() => view.watch.emit(activeStart(sectionKey, 1)));

    expect(await screen.findByText(/Live watch is active, but browser sound is not enabled/u))
      .toBeTruthy();
    expect(screen.getByText('Watching 1 Section (sound muted)')).toBeTruthy();
  });

  it('gates continuous starts on confirmation and supports finite ten-minute and unlimited policies', async () => {
    const first = section(1);
    const second = section(2);
    const { watch } = renderWatch([first, second]);
    fireEvent.click(screen.getByRole('button', { name: 'Add Section 00001 to the watch list' }));
    fireEvent.click(screen.getByRole('radio', { name: 'Continuous episode alarm' }));

    const start = screen.getByRole('button', { name: /Start selected/u });
    expect(start.hasAttribute('disabled')).toBe(true);
    fireEvent.click(start);
    expect(watch.commands).toEqual([]);
    const confirmationRequired = screen.getByText(
      /Confirm the CONTINUOUS alarm before starting or applying this policy/u,
    );
    expect(confirmationRequired.getAttribute('role')).toBe('alert');
    // A policy waiting on a confirmation is a warning banner (spec 4.9); red is
    // kept for something that actually failed.
    expect(confirmationRequired.className).toContain('watch-workspace__notice');

    const duration = screen.getByRole('combobox', { name: 'Continuous duration' }) as HTMLSelectElement;
    expect(duration.value).toBe('FINITE');
    expect(within(duration).getByRole('option', { name: '10 minutes' })).toBeTruthy();
    expect(within(duration).getByRole('option', { name: /Unlimited/u })).toBeTruthy();
    fireEvent.click(screen.getByRole('checkbox', { name: /I understand that CONTINUOUS sound persists/u }));
    expect(start.hasAttribute('disabled')).toBe(false);
    fireEvent.click(start);
    await waitFor(() => expect(watch.commands).toHaveLength(1));
    expect(watch.commands[0]).toEqual({
      type: 'START_WATCH',
      items: [{
        sectionKey: first,
        policy: {
          notificationMode: 'CONTINUOUS',
          maxAudible: 3,
          continuousDuration: { kind: 'FINITE', seconds: 600 },
        },
      }],
    });
    act(() => watch.emit(activeStart(first, 1)));
    await waitFor(() => expect(metricValue('Active')).toBe('1'));

    fireEvent.click(screen.getByRole('button', { name: 'Add Section 00002 to the watch list' }));
    fireEvent.change(duration, { target: { value: 'UNLIMITED' } });
    fireEvent.click(screen.getByRole('button', { name: /Start selected/u }));
    expect(watch.commands[1]).toEqual({
      type: 'START_WATCH',
      items: [{
        sectionKey: second,
        policy: {
          notificationMode: 'CONTINUOUS',
          maxAudible: 3,
          continuousDuration: { kind: 'UNLIMITED' },
        },
      }],
    });
  });

  it('exposes freshness, cadence, lag, circuit, and run/today counters as labelled text', async () => {
    vi.useFakeTimers({ toFake: ['Date'] });
    vi.setSystemTime(new Date(NOW));
    const sections = [section(1, 'NB'), section(2, 'NK'), section(3, 'CM')];
    renderWatch(sections);
    for (const sectionKey of sections) {
      fireEvent.click(screen.getByRole('button', {
        name: `Add Section ${sectionKey.index} to the watch list`,
      }));
    }

    const telemetry = screen.getByRole('region', {
      name: 'Freshness / lag / circuit / counters',
    });
    await waitFor(() => {
      expect(telemetry.querySelectorAll('.watch-telemetry__batch')).toHaveLength(3);
    });

    expect(telemetry.querySelector('[data-freshness="FRESH"]')).toBeTruthy();
    expect(telemetry.querySelector('[data-freshness="STALE"]')).toBeTruthy();
    expect(telemetry.querySelector('[data-freshness="UNKNOWN"]')).toBeTruthy();
    expect(within(telemetry).getAllByText(
      'No valid live status has been observed yet.',
    )).toHaveLength(2);
    expect(within(telemetry).getAllByText('Requested general')).toHaveLength(3);
    expect(within(telemetry).getAllByText('Requested effective')).toHaveLength(3);
    expect(within(telemetry).getAllByText('Actual interval')).toHaveLength(3);
    expect(within(telemetry).getAllByText('Scheduler lag')).toHaveLength(3);
    expect(within(telemetry).getAllByText('Circuit')).toHaveLength(3);
    expect(within(telemetry).getAllByText('Run counters')).toHaveLength(3);
    expect(within(telemetry).getAllByText(/Rutgers day/u)).toHaveLength(3);

    expect(within(telemetry).getAllByText('30s')).toHaveLength(3);
    expect(within(telemetry).getAllByText(/10s .* Active watch/u)).toHaveLength(3);
    expect(within(telemetry).getAllByText('12.50s')).toHaveLength(2);
    expect(within(telemetry).getByText('487ms')).toBeTruthy();
    expect(within(telemetry).getByText(/Retry after .* RATE_LIMITED/u)).toBeTruthy();
    expect(within(telemetry).getByText(/Fatal diagnostic .* NON_JSON/u)).toBeTruthy();
    expect(within(telemetry).getAllByText(
      '12 attempted / 9 succeeded / 2 failed / 1 empty',
    )).toHaveLength(3);
    expect(within(telemetry).getAllByText(
      '42 attempted / 36 succeeded / 4 failed / 2 empty',
    )).toHaveLength(3);
    expect(screen.getByLabelText('Selected Section Open status')).toBeTruthy();
  });

  it('does not show the empty telemetry instruction while a selected Section is loading', async () => {
    const sectionKey = section(1);
    const pendingBatch = deferred<OpenRefreshStatusV1>();
    const openStatus = vi.fn<ProductApiPort['openStatus']>(() => pendingBatch.promise);
    renderWatch(
      [sectionKey],
      new FakeWatchClient(),
      new FakeAudioController(),
      'en-US',
      { openStatus },
    );

    fireEvent.click(screen.getByRole('button', { name: 'Add Section 00001 to the watch list' }));
    await waitFor(() => expect(openStatus).toHaveBeenCalledOnce());
    expect(openStatus.mock.calls[0]![0]).toStrictEqual({
      contractVersion: 1,
      batch: { term: sectionKey.term, campus: sectionKey.campus },
    });

    const telemetry = screen.getByRole('region', {
      name: 'Freshness / lag / circuit / counters',
    });
    expect(within(telemetry).queryByText(
      /Select a Section to read its current BCSP Open status/u,
    )).toBeNull();

    // A read in flight is not a failure: it carries the busy tone, and nothing
    // on the panel is dressed as an error while it is the only thing happening.
    const loading = telemetry.querySelectorAll('.watch-telemetry__resource[data-loading]');
    expect(loading.length).toBeGreaterThan(0);
    for (const resource of loading) expect(resource.getAttribute('data-tone')).toBe('busy');
    expect(telemetry.querySelectorAll('.watch-telemetry__resource[data-tone="danger"]'))
      .toHaveLength(0);

    await act(async () => {
      pendingBatch.resolve(batchStatus({ term: sectionKey.term, campus: sectionKey.campus }));
      await pendingBatch.promise;
    });
  });

  it('names a rejected batch request as a contract error and offers no Retry for it', async () => {
    const sectionKey = section(1);
    const openStatus = vi.fn<ProductApiPort['openStatus']>().mockRejectedValue(
      new ProductClientError(400, {
        protocolVersion: 1,
        error: {
          code: 'MALFORMED_REQUEST',
          messageKey: 'error.malformed_request',
          traceId: 'trace-batch-400',
          details: [],
        },
      }),
    );
    renderWatch(
      [sectionKey],
      new FakeWatchClient(),
      new FakeAudioController(),
      'en-US',
      { openStatus },
    );

    fireEvent.click(screen.getByRole('button', { name: 'Add Section 00001 to the watch list' }));
    const telemetry = screen.getByRole('region', {
      name: 'Freshness / lag / circuit / counters',
    });
    const message = await within(telemetry).findByText(
      'The server rejected the live status request (HTTP 400, MALFORMED_REQUEST). '
      + 'This is a client-side defect; retrying will not recover it. Please update the app.',
    );
    expect(within(telemetry).queryByText(
      'Live status is temporarily unavailable; no result is available yet.',
    )).toBeNull();
    const resource = message.closest('article');
    if (!(resource instanceof HTMLElement)) throw new Error('Batch telemetry resource was not rendered');
    expect(within(resource).queryByRole('button', { name: 'Retry this status' })).toBeNull();
    fireEvent.click(within(resource).getByText('Technical diagnostics'));
    expect(within(resource).getByText('400')).toBeTruthy();
    expect(within(resource).getByText('MALFORMED_REQUEST')).toBeTruthy();
    expect(within(resource).getByText('trace-batch-400')).toBeTruthy();
    expect(openStatus).toHaveBeenCalledOnce();
  });

  it('shows status, API code, and trace ID for one failed resource and retries only it', async () => {
    const sectionKey = section(1);
    const failure = new ProductClientError(503, {
      protocolVersion: 1,
      error: {
        code: 'UPSTREAM_UNAVAILABLE',
        messageKey: 'error.upstream_unavailable',
        traceId: 'trace-section-00001',
        details: [],
      },
    });
    const openSectionStatus = vi.fn<ProductApiPort['openSectionStatus']>()
      .mockRejectedValueOnce(failure)
      .mockImplementation(async ({ sectionKey: key }) => sectionStatus(key));
    renderWatch(
      [sectionKey],
      new FakeWatchClient(),
      new FakeAudioController(),
      'en-US',
      { openSectionStatus },
    );

    fireEvent.click(screen.getByRole('button', { name: 'Add Section 00001 to the watch list' }));
    const telemetry = screen.getByRole('region', {
      name: 'Freshness / lag / circuit / counters',
    });
    expect(await within(telemetry).findByText(
      'Live status is temporarily unavailable; no result is available yet.',
    )).toBeTruthy();
    const resource = within(telemetry).getByText('Section 00001 status').closest('article');
    if (!(resource instanceof HTMLElement)) throw new Error('Section telemetry resource was not rendered');
    fireEvent.click(within(resource).getByText('Technical diagnostics'));
    expect(within(resource).getByText('503')).toBeTruthy();
    expect(within(resource).getByText('UPSTREAM_UNAVAILABLE')).toBeTruthy();
    expect(within(resource).getByText('trace-section-00001')).toBeTruthy();

    // No data and a request that actually failed is the one case that earns the
    // danger surface (spec 4.7).
    expect(resource.getAttribute('data-tone')).toBe('danger');

    fireEvent.click(within(resource).getByRole('button', { name: 'Retry this status' }));
    await waitFor(() => expect(openSectionStatus).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(within(resource).queryByRole('alert')).toBeNull());
    expect(within(resource).getByText(/Showing the latest valid result from/u)).toBeTruthy();
    // Held-over data is a warning, not a failure.
    expect(resource.getAttribute('data-tone')).toBe('warn');
  });

  it('clears telemetry after the final Section is removed', async () => {
    const sectionKey = section(1);
    renderWatch([sectionKey]);
    fireEvent.click(screen.getByRole('button', { name: 'Add Section 00001 to the watch list' }));
    const telemetry = screen.getByRole('region', {
      name: 'Freshness / lag / circuit / counters',
    });
    await waitFor(() => expect(telemetry.querySelectorAll('.watch-telemetry__batch')).toHaveLength(1));

    fireEvent.click(screen.getByRole('button', { name: 'Remove Section 00001 from the watch list' }));
    await waitFor(() => expect(telemetry.querySelectorAll('.watch-telemetry__batch')).toHaveLength(0));
    expect(within(telemetry).getByText(/Select a Section to read its current BCSP Open status/u)).toBeTruthy();
  });

  it('keeps per-Section episode controls after its alert card is dismissed', async () => {
    const sectionKey = section(1);
    const view = renderWatch([sectionKey]);
    const episode = continuousEpisode(sectionKey, 1, 'UNACKNOWLEDGED');
    act(() => emitVisibleAlert(view.watch, episode, 1));

    fireEvent.click(screen.getByRole('button', { name: 'Dismiss alert' }));
    expect(screen.queryByRole('button', { name: 'Dismiss alert' })).toBeNull();
    expect(screen.getByText(/alert card dismissed/u)).toBeTruthy();
    fireEvent.click(screen.getByRole('button', { name: 'Acknowledge' }));
    expect(view.watch.commands.at(-1)).toEqual({
      type: 'ACKNOWLEDGE_EPISODE',
      episode: {
        activeWatchId: episode.activeWatchId,
        episodeId: episode.episodeId,
        sectionKey,
      },
    });
  });

  it('auto-dismisses STATUS toasts after visible time while pausing for hover, focus, and hidden documents', () => {
    vi.useFakeTimers();
    const hiddenDescriptor = Object.getOwnPropertyDescriptor(document, 'hidden');
    const view = renderWatch([]);
    try {
      act(() => view.watch.emit(watchStopped(section(1))));
      const title = screen.getByRole('heading', { name: 'Watch stopped' });
      const toast = title.closest('.watch-toast');
      if (!(toast instanceof HTMLElement)) throw new Error('Status toast was not rendered');
      const dismiss = screen.getByRole('button', { name: 'Dismiss Watch stopped' });

      expect(toast.getAttribute('data-state')).toBe('VISIBLE');
      act(() => vi.advanceTimersByTime(1_000));

      fireEvent.mouseEnter(toast);
      expect(toast.getAttribute('data-paused')).toBe('true');
      act(() => vi.advanceTimersByTime(10_000));
      expect(toast.getAttribute('data-state')).toBe('VISIBLE');
      fireEvent.mouseLeave(toast);
      act(() => vi.advanceTimersByTime(1_000));

      act(() => dismiss.focus());
      expect(toast.getAttribute('data-paused')).toBe('true');
      act(() => vi.advanceTimersByTime(10_000));
      act(() => dismiss.blur());
      act(() => vi.advanceTimersByTime(1_000));

      Object.defineProperty(document, 'hidden', { configurable: true, value: true });
      fireEvent(document, new Event('visibilitychange'));
      expect(toast.getAttribute('data-paused')).toBe('true');
      act(() => vi.advanceTimersByTime(10_000));
      Object.defineProperty(document, 'hidden', { configurable: true, value: false });
      fireEvent(document, new Event('visibilitychange'));

      act(() => vi.advanceTimersByTime(1_999));
      expect(toast.getAttribute('data-state')).toBe('VISIBLE');
      act(() => vi.advanceTimersByTime(1));
      expect(toast.getAttribute('data-state')).toBe('EXITING');
      act(() => vi.advanceTimersByTime(180));
      expect(screen.queryByRole('heading', { name: 'Watch stopped' })).toBeNull();
    } finally {
      view.unmount();
      if (hiddenDescriptor === undefined) Reflect.deleteProperty(document, 'hidden');
      else Object.defineProperty(document, 'hidden', hiddenDescriptor);
      vi.useRealTimers();
    }
  });

  it('keeps ALERT toasts until manual dismissal and exposes an exit state', () => {
    vi.useFakeTimers();
    const view = renderWatch([]);
    try {
      act(() => view.watch.setState('CLOSED'));
      const title = screen.getByRole('heading', { name: 'Watch connection ended' });
      const toast = title.closest('.watch-toast');
      if (!(toast instanceof HTMLElement)) throw new Error('Alert toast was not rendered');

      act(() => vi.advanceTimersByTime(30_000));
      expect(toast.getAttribute('data-state')).toBe('VISIBLE');
      fireEvent.click(screen.getByRole('button', { name: 'Dismiss Watch connection ended' }));
      expect(toast.getAttribute('data-state')).toBe('EXITING');
      act(() => vi.advanceTimersByTime(180));
      expect(screen.queryByRole('heading', { name: 'Watch connection ended' })).toBeNull();
    } finally {
      view.unmount();
      vi.useRealTimers();
    }
  });

  it('turns page notifications off from the real watch console control', async () => {
    const watch = new FakeWatchClient();
    const notifications = new FakeNotifications();
    notifications.permission = 'granted';
    renderWatch([section(1), section(2)], watch, new FakeAudioController(), 'en-US', {}, undefined, [], {
      notifications,
      pageVisibility: () => false,
      notificationRegion: true,
    });

    // A hidden page with granted permission: an opening section is posted
    // through the production region.
    await act(async () => {
      watch.emit(activeStart(section(1), 1));
      emitVisibleAlert(watch, continuousEpisode(section(1), 1, 'UNACKNOWLEDGED'), 1);
    });
    expect(notifications.posted).toHaveLength(1);

    // The user turns the real control off -- the checkbox in the console,
    // not a provider call.
    fireEvent.click(screen.getByRole('checkbox', {
      name: 'Browser messages when this page cannot be heard',
    }));

    // A second opening posts nothing.
    await act(async () => {
      emitVisibleAlert(watch, continuousEpisode(section(1), 2, 'UNACKNOWLEDGED'), 2);
    });
    expect(notifications.posted).toHaveLength(1);

    // And a Start pressed while off never asks the browser for permission.
    const askedBefore = notifications.asked.length;
    fireEvent.click(screen.getByRole('button', { name: 'Add Section 00002 to the watch list' }));
    fireEvent.click(screen.getByRole('button', { name: /Start selected/u }));
    await waitFor(() => expect(watch.commands.some((command) => command.type === 'START_WATCH')).toBe(true));
    expect(notifications.asked).toHaveLength(askedBefore);
  });

  it('clears the pending queue when the real control is switched off', async () => {
    const watch = new FakeWatchClient();
    const notifications = new FakeNotifications();
    notifications.permission = 'granted';
    const probe: { value: LiveWatchValue | null } = { value: null };
    // No posting region mounted, so the queued request stays PENDING and the
    // clearing is observable; the probe only reads.
    renderWatch([section(1)], watch, new FakeAudioController(), 'en-US', {}, undefined, [], {
      notifications,
      pageVisibility: () => false,
      probe: (value) => { probe.value = value; },
    });

    await act(async () => {
      watch.emit(activeStart(section(1), 1));
      emitVisibleAlert(watch, continuousEpisode(section(1), 1, 'UNACKNOWLEDGED'), 1);
    });
    expect(probe.value?.notifications).toHaveLength(1);

    const control = () => screen.getByRole('checkbox', {
      name: 'Browser messages when this page cannot be heard',
    });
    fireEvent.click(control());
    expect(probe.value?.notifications).toHaveLength(0);

    // Re-enabling does not resurrect what the user said not to deliver.
    fireEvent.click(control());
    expect(probe.value?.notifications).toHaveLength(0);
  });

  it('asks for permission inside the same gesture that re-enables the switch', async () => {
    const watch = new FakeWatchClient();
    const notifications = new FakeNotifications();
    renderWatch([section(1)], watch, new FakeAudioController(), 'en-US', {}, undefined, [], {
      notifications,
      pageVisibility: () => false,
      notificationRegion: true,
    });

    const control = () => screen.getByRole('checkbox', {
      name: 'Browser messages when this page cannot be heard',
    });
    // Turning it OFF asks nothing.
    fireEvent.click(control());
    expect(notifications.asked).toHaveLength(0);

    // Turning it ON asks exactly once, riding the click's user activation --
    // the same boundary as the readiness region's recovery action.
    fireEvent.click(control());
    expect(notifications.asked).toHaveLength(1);
    await act(async () => { await Promise.resolve(); });
    expect(notifications.permission).toBe('granted');

    // The granted permission actually delivers.
    await act(async () => {
      watch.emit(activeStart(section(1), 1));
      emitVisibleAlert(watch, continuousEpisode(section(1), 1, 'UNACKNOWLEDGED'), 1);
    });
    expect(notifications.posted).toHaveLength(1);
  });

  it('keeps the populated Watch desk keyboard-native, named, and axe-clean', async () => {
    const stale = section(2, 'NK');
    const unknown = section(3, 'CM');
    const view = renderWatch([stale, unknown]);
    fireEvent.click(screen.getByRole('button', { name: 'Add Section 00002 to the watch list' }));
    fireEvent.click(screen.getByRole('button', { name: 'Add Section 00003 to the watch list' }));
    fireEvent.click(screen.getByRole('radio', { name: 'Continuous episode alarm' }));
    fireEvent.click(screen.getByRole('checkbox', {
      name: /I understand that CONTINUOUS sound persists/u,
    }));
    fireEvent.click(screen.getByRole('button', { name: /Start selected/u }));
    await waitFor(() => expect(view.watch.commands.some((command) => command.type === 'START_WATCH')).toBe(true));

    act(() => {
      view.watch.emit(activeStart(stale, 1));
      view.watch.emit(activeStart(unknown, 2));
      emitVisibleAlert(view.watch, continuousEpisode(stale, 1, 'UNACKNOWLEDGED'), 1);
      emitVisibleAlert(view.watch, continuousEpisode(unknown, 2, 'TIMED_OUT'), 2);
    });

    const telemetry = screen.getByRole('region', {
      name: 'Freshness / lag / circuit / counters',
    });
    await waitFor(() => {
      expect(telemetry.querySelector('[data-freshness="STALE"]')).toBeTruthy();
      expect(telemetry.querySelector('[data-freshness="UNKNOWN"]')).toBeTruthy();
      expect(metricValue('Active')).toBe('2');
    });

    const workspace = view.container.querySelector('.watch-workspace');
    if (!(workspace instanceof HTMLElement)) throw new Error('Watch workspace was not rendered');
    const scoped = within(workspace);
    const namedButtons = [
      scoped.getByRole('button', { name: /Start selected/u }),
      ...scoped.getAllByRole('button', { name: 'Stop' }),
      scoped.getByRole('button', { name: 'Acknowledge' }),
      scoped.getByRole('button', { name: 'Resume alarm' }),
      ...scoped.getAllByRole('button', { name: 'Dismiss alert' }),
    ];
    for (const control of namedButtons) {
      expect(control).toBeInstanceOf(HTMLButtonElement);
      expect((control as HTMLButtonElement).type).toBe('button');
      expect(control.getAttribute('aria-label') ?? control.textContent?.trim()).not.toBe('');
    }
    expect(scoped.getAllByRole('button', { name: 'Stop' })).toHaveLength(2);
    expect(scoped.getAllByRole('button', { name: 'Dismiss alert' })).toHaveLength(2);

    const volume = scoped.getByRole('slider', { name: /Sound volume/u });
    expect(volume).toBeInstanceOf(HTMLInputElement);
    expect((volume as HTMLInputElement).type).toBe('range');
    expect(volume.getAttribute('aria-label') ?? volume.getAttribute('id')).not.toBeNull();

    const accessibility = await axe.run(workspace, {
      rules: { 'color-contrast': { enabled: false } },
    });
    expect(accessibility.violations).toEqual([]);
  });
});

describe('Watch workspace stylesheet', () => {
  it('keeps the readiness bar sticky below the app bar instead of under it', () => {
    expect(WATCH_WORKSPACE_CSS).toMatch(/\.watch-readiness\s*\{[^}]*position:\s*sticky/u);
    expect(WATCH_WORKSPACE_CSS).toMatch(/\.watch-readiness\s*\{[^}]*top:\s*var\(--bcsp-navigation-height/u);
    expect(WATCH_WORKSPACE_CSS).not.toMatch(/\.watch-readiness[^{]*\{[^}]*border-left:\s*6px/u);
  });

  it('tints a telemetry resource by tone and never paints a pending read as an error', () => {
    expect(WATCH_WORKSPACE_CSS).toMatch(
      /\.watch-telemetry__resource\[data-tone='warn'\]\s*\{[^}]*background:\s*var\(--bcsp-warn-tint\)/u,
    );
    expect(WATCH_WORKSPACE_CSS).toMatch(
      /\.watch-telemetry__resource\[data-tone='danger'\]\s*\{[^}]*background:\s*var\(--bcsp-danger-tint\)/u,
    );
    // The old rule tinted every resource without data -- which includes every
    // resource that has merely not answered yet -- with the danger surface.
    expect(WATCH_WORKSPACE_CSS).not.toMatch(
      /\.watch-telemetry__resource\[data-availability='ERROR_NO_DATA'\][^{]*\{[^}]*danger-tint/u,
    );
    expect(WATCH_WORKSPACE_CSS).toMatch(
      /\.watch-telemetry__resource\[data-tone='busy'\] p::before\s*\{[^}]*animation:\s*bcsp-telemetry-turn/u,
    );
    expect(WATCH_WORKSPACE_CSS).toContain('@keyframes bcsp-telemetry-turn');
    expect(WATCH_WORKSPACE_CSS).toMatch(
      /@media \(prefers-reduced-motion: reduce\)[\s\S]*data-tone='busy'\] p::before[^}]*animation:\s*none/u,
    );
  });

  it('states an unconfirmed CONTINUOUS policy as a warn banner instead of loose red text', () => {
    expect(WATCH_WORKSPACE_CSS).toMatch(
      /\.watch-workspace__notice\s*\{[^}]*background:\s*var\(--bcsp-warn-tint\)/u,
    );
    expect(WATCH_WORKSPACE_CSS).toMatch(
      /\.watch-workspace__notice\s*\{[^}]*border:\s*1px solid var\(--bcsp-warn-line\)/u,
    );
    expect(WATCH_WORKSPACE_CSS).toMatch(
      /\.watch-workspace__notice::before\s*\{[^}]*background:\s*var\(--bcsp-warn\)/u,
    );
  });

  it('pulses the watching dot only when motion is allowed', () => {
    expect(WATCH_WORKSPACE_CSS).toContain('@keyframes bcsp-pulse');
    expect(WATCH_WORKSPACE_CSS).toMatch(
      /@media \(prefers-reduced-motion: reduce\)[\s\S]*\.watch-workspace__badge::before[^}]*animation:\s*none/u,
    );
  });
});
