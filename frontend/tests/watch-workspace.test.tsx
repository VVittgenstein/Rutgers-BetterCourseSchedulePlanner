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
  WatchCueOutcome,
  WatchServerEventV1,
  WatchServerListener,
  WatchStateListener,
  WsServerEnvelope,
} from '../src/ui/shared/product';
import {
  LiveWatchProvider,
  SectionSelectionAction,
  WatchToastRegion,
  WatchWorkspace,
} from '../src/ui/shared/watch';
import {
  WatchAudioController,
  type WatchAudioUnlockResult,
} from '../src/ui/shared/watch/audio';

const NOW = '2026-07-15T04:00:00.000Z';
const FRESH_UNTIL = '2026-07-15T04:00:30.000Z';

function section(index: number, campus = 'NB'): SectionKey {
  return {
    campus,
    index: index.toString().padStart(5, '0'),
    term: '2026-9',
  };
}

class FakeWatchClient implements WatchClientPort {
  state: WatchConnectionState;
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

function productApi(): ProductApiPort {
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
  } as ProductApiPort;
}

function runtime(watch: FakeWatchClient): ProductRuntimePort {
  return {
    dispose: vi.fn(),
    product: productApi(),
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

function renderWatch(
  sections: readonly SectionKey[],
  watch = new FakeWatchClient(),
  audio = new FakeAudioController(),
  locale: SupportedLocale = 'en-US',
) {
  const result = render(
    <BcspI18nProvider initialLocale={locale}>
      <LiveWatchProvider audio={audio} runtime={runtime(watch)}>
        <SelectionActions sections={sections} />
        <WatchWorkspace />
        <WatchToastRegion />
      </LiveWatchProvider>
    </BcspI18nProvider>,
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
  document.documentElement.lang = '';
});

describe('Watch workspace product flow', () => {
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
    fireEvent.click(screen.getByRole('button', {
      name: '选择课节 00001 以进行监看',
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
        name: `Select Section ${sectionKey.index} for watch`,
      }));
    }

    expect(screen.getByLabelText('9 of 9 selected')).toBeTruthy();
    fireEvent.click(screen.getByRole('button', {
      name: 'Select Section 00010 for watch',
    }));

    expect(await screen.findByRole('heading', { name: 'Selection limit reached' })).toBeTruthy();
    expect(screen.getByText(/00010 \/ 2026-9 \/ NB/u)).toBeTruthy();
    for (const sectionKey of sections.slice(0, 9)) {
      expect(screen.getByRole('button', {
        name: `Remove Section ${sectionKey.index} for watch`,
      }).getAttribute('aria-pressed')).toBe('true');
    }
    expect(screen.getByRole('button', {
      name: 'Select Section 00010 for watch',
    }).getAttribute('aria-pressed')).toBe('false');
    expect(screen.getByLabelText('9 of 9 selected')).toBeTruthy();
  });

  it('keeps selection inactive until START_RESULT and sends the default one-shot policy', async () => {
    const sectionKey = section(1);
    const { audio, container, watch } = renderWatch([sectionKey]);

    fireEvent.click(screen.getByRole('button', { name: 'Select Section 00001 for watch' }));
    expect(metricValue('Selected')).toBe('1');
    expect(metricValue('Active')).toBe('0');
    expect(container.querySelector('[data-state="SELECTED"]')?.textContent).toBe('Selected');

    fireEvent.click(screen.getByRole('button', { name: /Start selected/u }));
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
      .toBe('Starting');

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

  it('keeps the empty Watch path compact until a Section or alert needs batch actions', () => {
    renderWatch([]);

    expect(screen.getByRole('heading', { name: 'Live watch console' })).toBeTruthy();
    expect(screen.queryByRole('button', { name: /Start selected/u })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Apply policy to active' })).toBeNull();
    expect(screen.queryByRole('button', { name: 'Acknowledge all' })).toBeNull();
  });

  it('gates continuous starts on confirmation and supports finite ten-minute and unlimited policies', async () => {
    const first = section(1);
    const second = section(2);
    const { watch } = renderWatch([first, second]);
    fireEvent.click(screen.getByRole('button', { name: 'Select Section 00001 for watch' }));
    fireEvent.click(screen.getByRole('radio', { name: 'Continuous episode alarm' }));

    const start = screen.getByRole('button', { name: /Start selected/u });
    expect(start.hasAttribute('disabled')).toBe(true);
    fireEvent.click(start);
    expect(watch.commands).toEqual([]);
    const confirmationRequired = screen.getByText(
      /Confirm the CONTINUOUS alarm before starting or applying this policy/u,
    );
    expect(confirmationRequired.getAttribute('role')).toBe('alert');

    const duration = screen.getByRole('combobox', { name: 'Continuous duration' }) as HTMLSelectElement;
    expect(duration.value).toBe('FINITE');
    expect(within(duration).getByRole('option', { name: '10 minutes' })).toBeTruthy();
    expect(within(duration).getByRole('option', { name: /Unlimited/u })).toBeTruthy();
    fireEvent.click(screen.getByRole('checkbox', { name: /I understand that CONTINUOUS sound persists/u }));
    expect(start.hasAttribute('disabled')).toBe(false);
    fireEvent.click(start);

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

    fireEvent.click(screen.getByRole('button', { name: 'Select Section 00002 for watch' }));
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
    const sections = [section(1, 'NB'), section(2, 'NK'), section(3, 'CM')];
    renderWatch(sections);
    for (const sectionKey of sections) {
      fireEvent.click(screen.getByRole('button', {
        name: `Select Section ${sectionKey.index} for watch`,
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

  it('clears telemetry after the final Section is removed', async () => {
    const sectionKey = section(1);
    renderWatch([sectionKey]);
    fireEvent.click(screen.getByRole('button', { name: 'Select Section 00001 for watch' }));
    const telemetry = screen.getByRole('region', {
      name: 'Freshness / lag / circuit / counters',
    });
    await waitFor(() => expect(telemetry.querySelectorAll('.watch-telemetry__batch')).toHaveLength(1));

    fireEvent.click(screen.getByRole('button', { name: 'Remove Section 00001 for watch' }));
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

  it('keeps the populated Watch desk keyboard-native, named, and axe-clean', async () => {
    const stale = section(2, 'NK');
    const unknown = section(3, 'CM');
    const view = renderWatch([stale, unknown]);
    fireEvent.click(screen.getByRole('button', { name: 'Select Section 00002 for watch' }));
    fireEvent.click(screen.getByRole('button', { name: 'Select Section 00003 for watch' }));
    fireEvent.click(screen.getByRole('radio', { name: 'Continuous episode alarm' }));
    fireEvent.click(screen.getByRole('checkbox', {
      name: /I understand that CONTINUOUS sound persists/u,
    }));
    fireEvent.click(screen.getByRole('button', { name: /Start selected/u }));

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
