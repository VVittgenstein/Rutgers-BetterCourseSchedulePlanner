// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useEffect, type ReactNode } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { LocalPageFrame } from '../src/ui/local/pages/PageFrame';
import { SettingsPage } from '../src/ui/local/pages/SettingsPage';
import {
  LocalPersonalApi,
  LocalPersonalProvider,
  localSyncMessageKey,
  parseLocalBootstrapData,
  useLocalPersonal,
  useLocalPersonalOptional,
  type LocalBootstrapData,
  type LocalPersonalApiPort,
  type LocalPersonalContextValue,
  type LocalPersonalProviderProps,
  type LocalPersonalSyncListener,
  type LocalPersonalSyncMessage,
  type LocalPersonalSyncPort,
  type LocalSettings,
  type LocalSyncStatus,
  type SavedViewLibrary,
  type SavedViewMutation,
  type StoredSettings,
} from '../src/ui/local/personal';
import { LocalProductBootstrap } from '../src/ui/local/product/LocalProductBootstrap';
import { PublicProductBootstrap } from '../src/ui/public/product/PublicProductBootstrap';
import { BcspI18nProvider } from '../src/ui/shared/i18n/runtime';
import {
  createNeutralFilterState,
  ProductClient,
  ProductBootstrapError,
  ProductClientError,
  toFilterRequestV1,
  useProductRuntimeState,
  type ApiErrorEnvelope,
  type ProductRuntimePort,
} from '../src/ui/shared/product';

const SESSION = '10000000-0000-4000-8000-000000000001';
const SAVED_VIEW = '20000000-0000-4000-8000-000000000001';
const INCOMPATIBLE_VIEW = '30000000-0000-4000-8000-000000000001';
const DEFAULT_SETTINGS = {
  localeOverride: 'system',
  catalogRefreshMinutes: 60,
  openRefreshSeconds: 30,
  watchFastLaneSeconds: 10,
  volumePercent: 70,
  soundPolicy: {
    notificationMode: 'ONE_SHOT',
    maxAudible: 3,
    continuousDuration: { kind: 'FINITE', seconds: 600 },
  },
} satisfies LocalSettings;

function localBootstrap(
  stateRevision = 7,
  settingsRevision = 2,
  settings: LocalSettings = DEFAULT_SETTINGS,
): LocalBootstrapData {
  return {
    mode: 'LOCAL',
    sessionNonce: SESSION,
    state: {
      stateRevision,
      settings: { revision: settingsRevision, value: settings },
      currentFilters: { stateRevision, revision: 1, value: null },
      savedViews: [],
      selectedSections: [],
      desiredWatches: [],
      episodeHistory: { items: [], total: 0, offset: 0, limit: 50 },
      activeWatchCount: 0,
    },
  };
}

function localBootstrapWithV2Filters(): LocalBootstrapData {
  const bootstrap = localBootstrap();
  const filters = toFilterRequestV1({
    ...createNeutralFilterState('92026'),
    campuses: ['NB'],
    keywords: ['data structures'],
  });
  return {
    ...bootstrap,
    state: {
      ...bootstrap.state,
      currentFilters: {
        stateRevision: bootstrap.state.stateRevision,
        revision: 3,
        value: {
          association: { kind: 'APPLIED', viewId: SAVED_VIEW, revision: 2 },
          content: { status: 'COMPATIBLE', filters },
        },
      },
      savedViews: [
        {
          id: SAVED_VIEW,
          name: 'Data structures',
          schemaVersion: 2,
          revision: 2,
          content: { status: 'COMPATIBLE', filters },
          createdAt: 1_752_566_400_000,
          updatedAt: 1_752_570_000_000,
        },
        {
          id: INCOMPATIBLE_VIEW,
          name: 'Legacy active fields',
          schemaVersion: 1,
          revision: 1,
          content: {
            status: 'INCOMPATIBLE',
            rawSnapshot: {
              codecVersion: 1,
              schemaVersion: 1,
              fields: { 'FLT-C10': ['legacy-location'] },
            },
            reason: { kind: 'UNKNOWN_FIELD', stableId: 'FLT-C10' },
          },
          createdAt: 1_752_566_400_000,
          updatedAt: 1_752_570_000_000,
        },
      ],
    },
  };
}

function localEnvelope(bootstrap = localBootstrap()): unknown {
  return { protocolVersion: 1, data: bootstrap };
}

function publicEnvelope(): unknown {
  return { protocolVersion: 1, data: { sessionNonce: SESSION } };
}

function savedViewLibrary(bootstrap: LocalBootstrapData): SavedViewLibrary {
  return {
    stateRevision: bootstrap.state.stateRevision,
    currentFilters: bootstrap.state.currentFilters,
    views: [],
  };
}

function fakeRuntime(): ProductRuntimePort {
  return { dispose: vi.fn() } as unknown as ProductRuntimePort;
}

function RuntimeAndPersonalProbe() {
  const runtime = useProductRuntimeState();
  const personal = useLocalPersonalOptional();
  const runtimeStatus = runtime.status === 'ERROR'
    ? `ERROR:${runtime.reason}`
    : runtime.status;
  return (
    <section>
      <output aria-label="Runtime status">{runtimeStatus}</output>
      <output aria-label="Local personal state">
        {personal === null
          ? 'NO_LOCAL_STATE'
          : `revision ${personal.state.stateRevision}; locale ${personal.state.settings.value.localeOverride}`}
      </output>
    </section>
  );
}

function unexpected(): Promise<never> {
  return Promise.reject(new Error('Unexpected local personal API call.'));
}

function fakePersonalApi(overrides: Partial<LocalPersonalApiPort>): LocalPersonalApiPort {
  return {
    bootstrap: unexpected,
    pullTerm: unexpected,
    settings: unexpected,
    updateSettings: unexpected,
    selection: unexpected,
    replaceSelection: unexpected,
    history: unexpected,
    currentFilters: unexpected,
    replaceCurrentFilters: unexpected,
    savedViews: unexpected,
    createSavedView: unexpected,
    applySavedView: unexpected,
    renameSavedView: unexpected,
    updateSavedView: unexpected,
    duplicateSavedView: unexpected,
    deleteSavedView: unexpected,
    deleteAllSavedViews: unexpected,
    resetCurrentFilters: unexpected,
    prepareUserDataReset: unexpected,
    confirmUserDataReset: unexpected,
    ...overrides,
  };
}

function deferred() {
  let resolve!: () => void;
  const promise = new Promise<void>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

function syncLabel(sync: LocalSyncStatus): string {
  return 'reason' in sync ? `${sync.phase}:${sync.reason}` : sync.phase;
}

function apiError(status: number, code: string, revision?: number): ProductClientError {
  const envelope = {
    protocolVersion: 1,
    error: {
      code,
      messageKey: `local.error.${code.toLowerCase()}`,
      traceId: SESSION,
      details: revision === undefined ? [] : [{ kind: 'CURRENT_REVISION', revision }],
    },
  } as unknown as ApiErrorEnvelope;
  return new ProductClientError(status, envelope);
}

function fakeSyncPort() {
  const listeners = new Set<LocalPersonalSyncListener>();
  const port: LocalPersonalSyncPort & {
    readonly publish: ReturnType<typeof vi.fn<(message: LocalPersonalSyncMessage) => void>>;
    readonly dispose: ReturnType<typeof vi.fn<() => void>>;
    readonly emit: (message: LocalPersonalSyncMessage) => void;
  } = {
    publish: vi.fn<(message: LocalPersonalSyncMessage) => void>(),
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    dispose: vi.fn<() => void>(),
    emit: (message) => {
      for (const listener of [...listeners]) listener(message);
    },
  };
  return port;
}

function setVisibility(value: DocumentVisibilityState): void {
  Object.defineProperty(document, 'visibilityState', { configurable: true, value });
  document.dispatchEvent(new Event('visibilitychange'));
}

function pause(milliseconds = 30): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, milliseconds);
  });
}

interface Holder {
  current: LocalPersonalContextValue | null;
}

interface ProbeProps {
  readonly holder: Holder;
  readonly phases: string[];
}

function ConnectedSettingsPage() {
  const personal = useLocalPersonal();
  return (
    <BcspI18nProvider initialLocale="en-US">
      <SettingsPage
        error={personal.error}
        notice={personal.sync}
        onConfirmUserDataReset={async () => undefined}
        onDeleteAllSavedViews={personal.deleteAllSavedViews}
        onPrepareUserDataReset={personal.prepareUserDataReset}
        onResetCurrentFilters={personal.resetCurrentFilters}
        onUpdateSettings={personal.updateSettings}
        savedViewCount={personal.savedViews.views.length}
        settings={personal.state.settings}
      />
    </BcspI18nProvider>
  );
}

function PersonalProbe({ holder, phases }: ProbeProps) {
  const personal = useLocalPersonal();
  holder.current = personal;
  const label = syncLabel(personal.sync);
  useEffect(() => {
    if (phases[phases.length - 1] !== label) phases.push(label);
  }, [label, phases]);
  return (
    <section>
      <output aria-label="Sync phase">{label}</output>
      <output aria-label="Sync message">{localSyncMessageKey(personal.sync) ?? ''}</output>
      <output aria-label="Sync error">{personal.error?.message ?? ''}</output>
      <output aria-label="Settings revision">{personal.state.settings.revision}</output>
      <output aria-label="Snapshot origin">{personal.snapshotOrigin}</output>
      <output aria-label="Refreshing">{personal.refreshing ? 'yes' : 'no'}</output>
    </section>
  );
}

// Mutation backoff stays non-zero so React can paint the RETRYING phase between
// attempts; everything else is as fast as the provider allows.
const FAST: Partial<LocalPersonalProviderProps> = {
  retryDelaysMs: [5, 5],
  refreshRetryDelaysMs: [0],
  backgroundRefreshDelaysMs: [0, 0, 0],
  peerRefreshDebounceMs: 0,
  peerRefreshMinIntervalMs: 0,
  recoveredNoticeMs: 200,
};

function renderProvider(
  api: LocalPersonalApiPort,
  initialBootstrap: LocalBootstrapData,
  overrides: Partial<LocalPersonalProviderProps> = {},
  children?: ReactNode,
) {
  const holder: Holder = { current: null };
  const phases: string[] = [];
  const view = render(
    <LocalPersonalProvider api={api} initialBootstrap={initialBootstrap} {...FAST} {...overrides}>
      <PersonalProbe holder={holder} phases={phases} />
      {children}
    </LocalPersonalProvider>,
  );
  const personal = (): LocalPersonalContextValue => {
    if (holder.current === null) throw new Error('The provider has not rendered.');
    return holder.current;
  };
  return { holder, personal, phases, view };
}

function PersonalSettingsFlow() {
  const personal = useLocalPersonal();
  const update = (patch: Partial<LocalSettings>) => {
    void personal.updateSettings({
      ...personal.state.settings.value,
      ...patch,
    }).catch(() => undefined);
  };
  return (
    <section>
      <output aria-label="Personal revision">{personal.state.stateRevision}</output>
      <output aria-label="Saved locale">{personal.state.settings.value.localeOverride}</output>
      <output aria-label="Save status">
        {personal.error?.message ?? (personal.busy ? 'SAVING' : 'READY')}
      </output>
      <output aria-label="Sync phase">{syncLabel(personal.sync)}</output>
      <button type="button" onClick={() => update({ localeOverride: 'en-US' })}>Use English</button>
      <button type="button" onClick={() => update({ localeOverride: 'zh-CN' })}>Use Chinese</button>
      <button type="button" onClick={() => update({ catalogRefreshMinutes: 13 })}>
        Trigger failed save
      </button>
      <button type="button" onClick={() => update({ catalogRefreshMinutes: 14 })}>
        Recover save
      </button>
    </section>
  );
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
  Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'visible' });
});

describe('P7.2-004 local personal state', () => {
  it('reaches READY with a complete local bootstrap and publishes the personal state', async () => {
    const bootstrap = localBootstrapWithV2Filters();
    const requests: string[] = [];
    const fetchMock = vi.fn<typeof fetch>(async (input) => {
      const path = new URL(String(input), 'https://planner.invalid').pathname;
      requests.push(path);
      if (path === '/api/v1/local/bootstrap') return Response.json(localEnvelope(bootstrap));
      if (path === '/api/v1/local/saved-views') {
        return Response.json({ protocolVersion: 1, data: savedViewLibrary(bootstrap) });
      }
      return new Response(null, { status: 404 });
    });

    render(
      <LocalProductBootstrap
        baseUrl="https://planner.invalid"
        fetch={fetchMock}
        runtimeFactory={() => fakeRuntime()}
      >
        <RuntimeAndPersonalProbe />
      </LocalProductBootstrap>,
    );

    await waitFor(() => expect(screen.getByLabelText('Runtime status').textContent).toBe('READY'));
    expect(screen.getByLabelText('Local personal state').textContent).toBe(
      'revision 7; locale system',
    );
    await waitFor(() => expect(requests).toContain('/api/v1/local/saved-views'));
  });

  it('accepts V3 current filters and Saved views while preserving backend incompatibility', () => {
    const bootstrap = localBootstrapWithV2Filters();

    const parsed = parseLocalBootstrapData(bootstrap);

    expect(parsed.state.currentFilters.value?.content).toMatchObject({
      status: 'COMPATIBLE',
      filters: { contractVersion: 3 },
    });
    expect(parsed.state.savedViews[0]?.content).toMatchObject({
      status: 'COMPATIBLE',
      filters: { contractVersion: 3 },
    });
    expect(parsed.state.savedViews[1]?.content).toEqual({
      status: 'INCOMPATIBLE',
      rawSnapshot: {
        codecVersion: 1,
        schemaVersion: 1,
        fields: { 'FLT-C10': ['legacy-location'] },
      },
      reason: { kind: 'UNKNOWN_FIELD', stableId: 'FLT-C10' },
    });
  });

  it('parses persisted desired watches and rejects legacy, live-watch, or overfull payloads', () => {
    const policy = {
      notificationMode: 'CONTINUOUS',
      maxAudible: 5,
      continuousDuration: { kind: 'UNLIMITED' },
    } as const;
    const desiredWatches = [
      { section: { term: '92026', campus: 'NB', index: '10855' }, policy },
      {
        section: { term: '92026', campus: 'CM', index: '04312' },
        policy: {
          notificationMode: 'ONE_SHOT',
          maxAudible: 3,
          continuousDuration: { kind: 'FINITE', seconds: 600 },
        },
      },
    ] as const;
    const bootstrap = localBootstrap();
    const withDesired = {
      ...bootstrap,
      state: { ...bootstrap.state, desiredWatches },
    };

    expect(parseLocalBootstrapData(withDesired).state.desiredWatches).toEqual(desiredWatches);

    const legacyPayload = structuredClone(bootstrap) as unknown as { state: Record<string, unknown> };
    delete legacyPayload.state.desiredWatches;
    const liveWatchLeak = {
      ...bootstrap,
      state: {
        ...bootstrap.state,
        desiredWatches: [{ ...desiredWatches[0], activeWatchId: SESSION }],
      },
    };
    const invalidPolicy = {
      ...bootstrap,
      state: {
        ...bootstrap.state,
        desiredWatches: [{ section: desiredWatches[0].section, policy: { ...policy, maxAudible: 0 } }],
      },
    };
    const overfull = {
      ...bootstrap,
      state: {
        ...bootstrap.state,
        desiredWatches: Array.from({ length: 10 }, (_, ordinal) => ({
          section: { term: '92026', campus: 'NB', index: String(10_000 + ordinal) },
          policy,
        })),
      },
    };

    for (const rejected of [legacyPayload, liveWatchLeak, invalidPolicy, overfull]) {
      expect(() => parseLocalBootstrapData(rejected)).toThrow(ProductBootstrapError);
    }
  });

  it('accepts every frozen REVIEW_REQUIRED migration reason and rejects unknown reason codes', () => {
    const bootstrap = localBootstrapWithV2Filters();
    const reasons = [
      { stableId: 'FLT-C04', code: 'ACTIVE_LEGACY_FILTER' },
      { stableId: 'FLT-C01', code: 'UNSUPPORTED_CAMPUS' },
      { stableId: 'FLT-C01', code: 'SCOPE_UNAVAILABLE' },
      { stableId: 'FLT-C05', code: 'DYNAMIC_VALUE_UNAVAILABLE' },
    ] as const;
    const withReview = {
      ...bootstrap,
      state: {
        ...bootstrap.state,
        currentFilters: {
          ...bootstrap.state.currentFilters,
          value: {
            association: { kind: 'CUSTOM' },
            content: { status: 'REVIEW_REQUIRED', rawSnapshot: { codecVersion: 2 }, reasons },
          },
        },
      },
    };
    expect(parseLocalBootstrapData(withReview).state.currentFilters.value?.content).toMatchObject({
      status: 'REVIEW_REQUIRED',
      reasons,
    });
    const unknownReason = structuredClone(withReview);
    if (unknownReason.state.currentFilters.value?.content.status !== 'REVIEW_REQUIRED') {
      throw new Error('Expected REVIEW_REQUIRED fixture.');
    }
    unknownReason.state.currentFilters.value.content.reasons[3]!.code = 'UNKNOWN_REASON';
    expect(() => parseLocalBootstrapData(unknownReason)).toThrow(ProductBootstrapError);
  });

  it('requires backend migration before legacy filters can be marked compatible', () => {
    const bootstrap = localBootstrapWithV2Filters();
    const compatible = bootstrap.state.currentFilters.value?.content;
    if (compatible?.status !== 'COMPATIBLE') throw new Error('Expected compatible test filters.');
    const legacyFilters = { ...compatible.filters, contractVersion: 1 };
    const legacyCurrentFilters = {
      ...bootstrap,
      state: {
        ...bootstrap.state,
        currentFilters: {
          ...bootstrap.state.currentFilters,
          value: {
            association: { kind: 'CUSTOM' },
            content: { status: 'COMPATIBLE', filters: legacyFilters },
          },
        },
      },
    };
    const legacySavedView = {
      ...bootstrap,
      state: {
        ...bootstrap.state,
        currentFilters: { ...bootstrap.state.currentFilters, value: null },
        savedViews: [{
          ...bootstrap.state.savedViews[0],
          content: { status: 'COMPATIBLE', filters: legacyFilters },
        }],
      },
    };

    expect(() => parseLocalBootstrapData(legacyCurrentFilters)).toThrow(ProductBootstrapError);
    expect(() => parseLocalBootstrapData(legacySavedView)).toThrow(ProductBootstrapError);
  });

  it('rejects a missing or invalid local state instead of publishing a partial personal context', async () => {
    const invalidState = localBootstrap();
    const invalidCases: readonly unknown[] = [
      { protocolVersion: 1, data: { mode: 'LOCAL', sessionNonce: SESSION } },
      localEnvelope({
        ...invalidState,
        state: { ...invalidState.state, activeWatchCount: 10 },
      }),
    ];

    for (const candidate of invalidCases) {
      const runtimeFactory = vi.fn(() => fakeRuntime());
      const view = render(
        <LocalProductBootstrap
          fetch={vi.fn<typeof fetch>(async () => Response.json(candidate))}
          runtimeFactory={runtimeFactory}
        >
          <RuntimeAndPersonalProbe />
        </LocalProductBootstrap>,
      );
      await waitFor(() => expect(screen.getByLabelText('Runtime status').textContent).toBe(
        'ERROR:BOOTSTRAP_INVALID',
      ));
      expect(screen.getByLabelText('Local personal state').textContent).toBe('NO_LOCAL_STATE');
      expect(runtimeFactory).not.toHaveBeenCalled();
      view.unmount();
    }
  });

  it('sends a setting save through the local API with the session and concurrency revisions', async () => {
    const next = { revision: 3, value: { ...DEFAULT_SETTINGS, localeOverride: 'zh-CN' } } satisfies StoredSettings;
    const fetchMock = vi.fn<typeof fetch>(async () => Response.json({ protocolVersion: 1, data: next }));
    const api = new LocalPersonalApi(new ProductClient({
      baseUrl: 'https://planner.invalid/',
      fetch: fetchMock,
      session: () => SESSION,
    }));

    await expect(api.updateSettings({
      expectedUserStateRevision: 7,
      expectedRevision: 2,
      value: next.value,
    })).resolves.toEqual(next);

    const [url, init] = fetchMock.mock.calls[0] ?? [];
    expect(url).toBe('https://planner.invalid/api/v1/local/settings');
    expect(init?.method).toBe('PUT');
    expect(new Headers(init?.headers).get('x-bcsp-session')).toBe(SESSION);
    expect(JSON.parse(String(init?.body))).toEqual({
      protocolVersion: 1,
      payload: {
        expectedUserStateRevision: 7,
        expectedRevision: 2,
        value: next.value,
      },
    });
  });

  it('registers manual term pull only on the Local API and sends the canonical request envelope', async () => {
    const response = {
      contractVersion: 1,
      term: '12026',
      disposition: 'ENQUEUED',
      targetCount: 12,
    } as const;
    const fetchMock = vi.fn<typeof fetch>(async () => Response.json({
      protocolVersion: 1,
      data: response,
    }));
    const api = new LocalPersonalApi(new ProductClient({
      baseUrl: 'https://planner.invalid/',
      fetch: fetchMock,
      session: () => SESSION,
    }));

    await expect(api.pullTerm({ contractVersion: 1, term: '12026' })).resolves.toEqual(response);
    const [url, init] = fetchMock.mock.calls[0] ?? [];
    expect(url).toBe('https://planner.invalid/api/v1/local/terms/pull');
    expect(init?.method).toBe('POST');
    expect(new Headers(init?.headers).get('x-bcsp-session')).toBe(SESSION);
    expect(JSON.parse(String(init?.body))).toEqual({
      protocolVersion: 1,
      payload: { contractVersion: 1, term: '12026' },
    });
  });

  it('serializes saves with the latest revision and continues after a failed save', async () => {
    let server = localBootstrap(10, 3);
    const firstSave = deferred();
    const secondSave = deferred();
    const updateSettings = vi.fn<LocalPersonalApiPort['updateSettings']>(async (request) => {
      const call = updateSettings.mock.calls.length;
      if (call === 1) await firstSave.promise;
      if (call === 2) await secondSave.promise;
      if (request.value.catalogRefreshMinutes === 13) {
        throw new Error('Simulated write failure');
      }
      server = localBootstrap(
        server.state.stateRevision + 1,
        server.state.settings.revision + 1,
        request.value,
      );
      return server.state.settings;
    });
    const api = fakePersonalApi({
      bootstrap: vi.fn(async () => server),
      savedViews: vi.fn(async () => savedViewLibrary(server)),
      updateSettings,
    });
    render(
      <LocalPersonalProvider api={api} initialBootstrap={server} retryDelaysMs={[]}>
        <PersonalSettingsFlow />
      </LocalPersonalProvider>,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Use English' }));
    fireEvent.click(screen.getByRole('button', { name: 'Use Chinese' }));
    await waitFor(() => expect(updateSettings).toHaveBeenCalledTimes(1));
    expect(updateSettings.mock.calls[0]?.[0]).toMatchObject({
      expectedUserStateRevision: 10,
      expectedRevision: 3,
      value: { localeOverride: 'en-US' },
    });

    await act(async () => {
      firstSave.resolve();
      await Promise.resolve();
    });
    await waitFor(() => expect(updateSettings).toHaveBeenCalledTimes(2));
    expect(updateSettings.mock.calls[1]?.[0]).toMatchObject({
      expectedUserStateRevision: 11,
      expectedRevision: 4,
      value: { localeOverride: 'zh-CN' },
    });

    await act(async () => {
      secondSave.resolve();
      await Promise.resolve();
    });
    await waitFor(() => expect(screen.getByLabelText('Personal revision').textContent).toBe('12'));
    expect(screen.getByLabelText('Saved locale').textContent).toBe('zh-CN');

    fireEvent.click(screen.getByRole('button', { name: 'Trigger failed save' }));
    await waitFor(() => expect(screen.getByLabelText('Save status').textContent).toBe(
      'Simulated write failure',
    ));
    // A plain Error is a rejected request: no retry, and the original Error object survives.
    expect(screen.getByLabelText('Sync phase').textContent).toBe('FAILED:REJECTED');
    expect(updateSettings).toHaveBeenCalledTimes(3);
    expect(updateSettings.mock.calls[2]?.[0]).toMatchObject({
      expectedUserStateRevision: 12,
      expectedRevision: 5,
    });

    fireEvent.click(screen.getByRole('button', { name: 'Recover save' }));
    await waitFor(() => expect(screen.getByLabelText('Personal revision').textContent).toBe('13'));
    expect(updateSettings).toHaveBeenCalledTimes(4);
    expect(updateSettings.mock.calls[3]?.[0]).toMatchObject({
      expectedUserStateRevision: 12,
      expectedRevision: 5,
      value: { catalogRefreshMinutes: 14 },
    });
    expect(screen.getByLabelText('Save status').textContent).toBe('READY');
    expect(screen.getByLabelText('Sync phase').textContent).toBe('IDLE');
  });

  it('keeps a public document session-only and never publishes local personal state', async () => {
    render(
      <PublicProductBootstrap bootstrap={publicEnvelope()} runtimeFactory={() => fakeRuntime()}>
        <RuntimeAndPersonalProbe />
      </PublicProductBootstrap>,
    );

    await waitFor(() => expect(screen.getByLabelText('Runtime status').textContent).toBe('READY'));
    expect(screen.getByLabelText('Local personal state').textContent).toBe('NO_LOCAL_STATE');
    expect(publicEnvelope()).toEqual({
      protocolVersion: 1,
      data: { sessionNonce: SESSION },
    });
  });
});

describe('local personal-state sync resilience', () => {
  it('retries an idempotent mutation once with fresh revisions after a 409 revision conflict', async () => {
    let server = localBootstrap(7, 5);
    const updateSettings = vi.fn<LocalPersonalApiPort['updateSettings']>(async (request) => {
      if (updateSettings.mock.calls.length === 1) {
        throw apiError(409, 'SETTINGS_REVISION_CONFLICT', 5);
      }
      server = localBootstrap(7, request.expectedRevision + 1, request.value);
      return server.state.settings;
    });
    // A refresh takes real time, like a fetch, so the RETRYING phase is painted.
    const bootstrap = vi.fn(async () => {
      await pause(5);
      return server;
    });
    const api = fakePersonalApi({
      bootstrap,
      savedViews: vi.fn(async () => savedViewLibrary(server)),
      updateSettings,
    });
    const { personal, phases } = renderProvider(api, localBootstrap(7, 2));

    let outcome: Promise<void> | undefined;
    await act(async () => {
      outcome = personal().updateSettings({ ...DEFAULT_SETTINGS, localeOverride: 'zh-CN' });
      await Promise.resolve();
    });
    await expect(outcome).resolves.toBeUndefined();

    expect(updateSettings).toHaveBeenCalledTimes(2);
    expect(updateSettings.mock.calls[0]?.[0]).toMatchObject({ expectedRevision: 2 });
    expect(updateSettings.mock.calls[1]?.[0]).toMatchObject({ expectedRevision: 5 });
    expect(bootstrap.mock.invocationCallOrder[0]).toBeLessThan(updateSettings.mock.invocationCallOrder[1]!);
    expect(bootstrap.mock.invocationCallOrder[0]).toBeGreaterThan(updateSettings.mock.invocationCallOrder[0]!);

    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('RECOVERED:CONFLICT'));
    expect(screen.getByLabelText('Sync message').textContent).toBe('local.sync.recovered');
    expect(screen.getByLabelText('Sync error').textContent).toBe('');
    expect(personal().error).toBeNull();
    expect(screen.getByLabelText('Settings revision').textContent).toBe('6');
    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('IDLE'));
    expect(phases).toContain('RETRYING:CONFLICT');
    expect(phases.indexOf('RETRYING:CONFLICT')).toBeLessThan(phases.indexOf('RECOVERED:CONFLICT'));
  });

  it('stays silent about a conflict recovery for debounced filter persists', async () => {
    const initial = localBootstrapWithV2Filters();
    let server = initial;
    const filters = toFilterRequestV1({ ...createNeutralFilterState('92026'), campuses: ['NK'] });
    const replaceCurrentFilters = vi.fn<LocalPersonalApiPort['replaceCurrentFilters']>(async (request) => {
      if (replaceCurrentFilters.mock.calls.length === 1) {
        throw apiError(409, 'CURRENT_FILTERS_REVISION_CONFLICT', 9);
      }
      const next = {
        stateRevision: server.state.stateRevision,
        revision: request.expectedCurrentFiltersRevision + 1,
        value: { association: { kind: 'CUSTOM' as const }, content: { status: 'COMPATIBLE' as const, filters: request.filters } },
      };
      server = { ...server, state: { ...server.state, currentFilters: next } };
      return next;
    });
    const api = fakePersonalApi({
      bootstrap: vi.fn(async () => {
        await pause(5);
        if (replaceCurrentFilters.mock.calls.length === 1) {
          return { ...initial, state: { ...initial.state, currentFilters: { ...initial.state.currentFilters, revision: 9 } } };
        }
        return server;
      }),
      savedViews: vi.fn(async () => savedViewLibrary(server)),
      replaceCurrentFilters,
    });
    const { personal, phases } = renderProvider(api, initial);

    let outcome: Promise<void> | undefined;
    await act(async () => {
      outcome = personal().replaceCurrentFilters(filters);
      await Promise.resolve();
    });
    await waitFor(() => expect(phases).toContain('RETRYING:CONFLICT'));
    await expect(outcome).resolves.toBeUndefined();

    expect(replaceCurrentFilters).toHaveBeenCalledTimes(2);
    expect(replaceCurrentFilters.mock.calls[1]?.[0]).toMatchObject({ expectedCurrentFiltersRevision: 9 });
    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('IDLE'));
    expect(phases).toContain('RETRYING:CONFLICT');
    expect(phases).not.toContain('RECOVERED:CONFLICT');
    expect(personal().state.currentFilters.revision).toBe(10);
  });

  it('surfaces a 409 without retrying for delete and reset flows', async () => {
    const initial = localBootstrapWithV2Filters();
    const bootstrap = vi.fn(async () => initial);
    const deleteSavedView = vi.fn(async () => {
      throw apiError(409, 'SAVED_VIEW_REVISION_CONFLICT', 3);
    });
    const resetCurrentFilters = vi.fn(async () => {
      throw apiError(409, 'CURRENT_FILTERS_REVISION_CONFLICT', 4);
    });
    const deleteAllSavedViews = vi.fn(async () => {
      throw apiError(409, 'CURRENT_FILTERS_REVISION_CONFLICT', 4);
    });
    const prepareUserDataReset = vi.fn(async () => {
      throw apiError(409, 'USER_STATE_REVISION_CONFLICT', 8);
    });
    const confirmUserDataReset = vi.fn(async () => {
      throw apiError(503, 'STORAGE_BUSY');
    });
    const api = fakePersonalApi({
      bootstrap,
      savedViews: vi.fn(async () => savedViewLibrary(initial)),
      confirmUserDataReset,
      deleteAllSavedViews,
      deleteSavedView,
      prepareUserDataReset,
      resetCurrentFilters,
    });
    const { personal } = renderProvider(api, initial);

    for (const [action, mock, phase, message] of [
      [() => personal().deleteSavedView(SAVED_VIEW), deleteSavedView, 'FAILED:CONFLICT', 'local.sync.failed_conflict'],
      [() => personal().resetCurrentFilters(), resetCurrentFilters, 'FAILED:CONFLICT', 'local.sync.failed_conflict'],
      [() => personal().deleteAllSavedViews(), deleteAllSavedViews, 'FAILED:CONFLICT', 'local.sync.failed_conflict'],
      [() => personal().prepareUserDataReset(), prepareUserDataReset, 'FAILED:STATE_RESET', 'local.sync.failed_reset'],
      [() => personal().confirmUserDataReset(SESSION), confirmUserDataReset, 'FAILED:UNAVAILABLE', 'local.sync.failed_unavailable'],
    ] as const) {
      let outcome: Promise<unknown> | undefined;
      await act(async () => {
        outcome = action().catch((error: unknown) => error);
        await Promise.resolve();
      });
      await expect(outcome).resolves.toBeInstanceOf(ProductClientError);
      expect(mock).toHaveBeenCalledTimes(1);
      await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe(phase));
      expect(screen.getByLabelText('Sync message').textContent).toBe(message);
      expect(personal().error).toBeInstanceOf(ProductClientError);
    }
    expect(bootstrap).not.toHaveBeenCalled();
  });

  it('retries a busy or rebuilding service with backoff before giving up', async () => {
    let server = localBootstrap(7, 2);
    const failures: unknown[] = [apiError(503, 'STORAGE_BUSY'), apiError(500, 'INTERNAL_ERROR')];
    const updateSettings = vi.fn<LocalPersonalApiPort['updateSettings']>(async (request) => {
      const failure = failures.shift();
      if (failure !== undefined) throw failure;
      server = localBootstrap(7, request.expectedRevision + 1, request.value);
      return server.state.settings;
    });
    const api = fakePersonalApi({
      bootstrap: vi.fn(async () => server),
      savedViews: vi.fn(async () => savedViewLibrary(server)),
      updateSettings,
    });
    const { personal, phases } = renderProvider(api, server);

    let outcome: Promise<unknown> | undefined;
    await act(async () => {
      outcome = personal().updateSettings({ ...DEFAULT_SETTINGS, volumePercent: 10 });
      await Promise.resolve();
    });
    await waitFor(() => expect(phases).toContain('RETRYING:BUSY'));
    await expect(outcome).resolves.toBeUndefined();
    expect(updateSettings).toHaveBeenCalledTimes(3);
    expect(updateSettings.mock.calls.map(([request]) => request.expectedRevision)).toEqual([2, 2, 2]);
    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('IDLE'));
    expect(phases).toContain('RETRYING:BUSY');
    expect(personal().error).toBeNull();
    expect(personal().state.settings.value.volumePercent).toBe(10);

    // Exhausted backoff: three busy answers, then a specific UNAVAILABLE failure.
    failures.push(apiError(503, 'STORAGE_BUSY'), apiError(503, 'STORAGE_BUSY'), apiError(503, 'STORAGE_BUSY'));
    await act(async () => {
      outcome = personal().updateSettings({ ...DEFAULT_SETTINGS, volumePercent: 20 }).catch((error: unknown) => error);
      await Promise.resolve();
    });
    await expect(outcome).resolves.toBeInstanceOf(ProductClientError);
    expect(updateSettings).toHaveBeenCalledTimes(6);
    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('FAILED:UNAVAILABLE'));
    expect(screen.getByLabelText('Sync message').textContent).toBe('local.sync.failed_unavailable');
    expect(personal().error).toBeInstanceOf(ProductClientError);

    // A network failure is retried too.
    failures.push(new TypeError('fetch failed'));
    await act(async () => {
      await personal().updateSettings({ ...DEFAULT_SETTINGS, volumePercent: 30 });
    });
    expect(updateSettings).toHaveBeenCalledTimes(8);
    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('IDLE'));

    // An abort is neither retried nor surfaced.
    failures.push(Object.assign(new Error('aborted'), { name: 'AbortError' }));
    await act(async () => {
      outcome = personal().updateSettings({ ...DEFAULT_SETTINGS, volumePercent: 40 }).catch((error: unknown) => error);
      await Promise.resolve();
    });
    await expect(outcome).resolves.toMatchObject({ name: 'AbortError' });
    expect(updateSettings).toHaveBeenCalledTimes(9);
    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('IDLE'));
    expect(personal().error).toBeNull();
  });

  it('does not report a saved mutation as failed when the follow-up bootstrap fails', async () => {
    let server = localBootstrap(7, 2);
    const bootstrap = vi.fn(async () => {
      if (bootstrap.mock.calls.length <= 3) throw apiError(500, 'INTERNAL_ERROR');
      return server;
    });
    const updateSettings = vi.fn<LocalPersonalApiPort['updateSettings']>(async (request) => {
      server = localBootstrap(7, 3, request.value);
      return server.state.settings;
    });
    const api = fakePersonalApi({
      bootstrap,
      savedViews: vi.fn(async () => savedViewLibrary(server)),
      updateSettings,
    });
    const { personal, phases } = renderProvider(api, server);

    await act(async () => {
      await personal().updateSettings({ ...DEFAULT_SETTINGS, localeOverride: 'en-US' });
    });
    expect(personal().error).toBeNull();
    // The mutation result is merged immediately, before any refresh succeeds.
    expect(personal().state.settings.revision).toBe(3);
    expect(personal().state.settings.value.localeOverride).toBe('en-US');
    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('RECOVERED:REFRESH'));
    expect(phases).toContain('STALE');
    expect(phases.indexOf('STALE')).toBeLessThan(phases.indexOf('RECOVERED:REFRESH'));
    expect(bootstrap).toHaveBeenCalledTimes(4);
    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('IDLE'));
    expect(personal().error).toBeNull();
  });

  it('keeps issuing mutations with the merged revisions while the snapshot is stale', async () => {
    let server = localBootstrap(7, 2);
    let serverUp = false;
    const bootstrap = vi.fn(async () => {
      if (!serverUp) throw apiError(503, 'STORAGE_BUSY');
      return server;
    });
    const updateSettings = vi.fn<LocalPersonalApiPort['updateSettings']>(async (request) => {
      server = localBootstrap(7, request.expectedRevision + 1, request.value);
      return server.state.settings;
    });
    const api = fakePersonalApi({
      bootstrap,
      savedViews: vi.fn(async () => savedViewLibrary(server)),
      updateSettings,
    });
    const { personal } = renderProvider(api, server, { backgroundRefreshDelaysMs: [0, 60_000] });

    await act(async () => {
      await personal().updateSettings({ ...DEFAULT_SETTINGS, localeOverride: 'en-US' });
    });
    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('STALE'));
    expect(screen.getByLabelText('Sync message').textContent).toBe('local.sync.stale');
    expect(personal().error).toBeNull();
    expect(screen.getByLabelText('Settings revision').textContent).toBe('3');

    await act(async () => {
      await personal().updateSettings({ ...DEFAULT_SETTINGS, localeOverride: 'zh-CN' });
    });
    expect(updateSettings).toHaveBeenCalledTimes(2);
    expect(updateSettings.mock.calls[1]?.[0]).toMatchObject({ expectedRevision: 3 });
    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('STALE'));
    expect(screen.getByLabelText('Settings revision').textContent).toBe('4');

    serverUp = true;
    await act(async () => {
      await personal().reload();
    });
    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('IDLE'));
    expect(screen.getByLabelText('Snapshot origin').textContent).toBe('RELOAD');
    expect(personal().state.settings.value.localeOverride).toBe('zh-CN');
  });

  it('refreshes bootstrap when the tab becomes visible and coalesces bursts', async () => {
    let clock = 0;
    const server = localBootstrap(7, 2);
    const bootstrap = vi.fn(async () => server);
    const api = fakePersonalApi({ bootstrap, savedViews: vi.fn(async () => savedViewLibrary(server)) });
    renderProvider(api, server, { now: () => clock });

    await act(async () => {
      setVisibility('hidden');
      await Promise.resolve();
    });
    expect(bootstrap).not.toHaveBeenCalled();

    clock = 10_000;
    await act(async () => {
      setVisibility('visible');
      setVisibility('visible');
      await Promise.resolve();
    });
    await waitFor(() => expect(bootstrap).toHaveBeenCalledTimes(1));
    await pause();
    expect(bootstrap).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(screen.getByLabelText('Snapshot origin').textContent).toBe('VISIBILITY'));

    // A tab that was refreshed moments ago does not hit the service again.
    clock = 12_000;
    await act(async () => {
      setVisibility('hidden');
      setVisibility('visible');
      await Promise.resolve();
    });
    await pause();
    expect(bootstrap).toHaveBeenCalledTimes(1);
  });

  it('refreshes after a peer tab mutation and ignores its own broadcasts', async () => {
    let server = localBootstrap(7, 2);
    const bootstrap = vi.fn(async () => server);
    const updateSettings = vi.fn<LocalPersonalApiPort['updateSettings']>(async (request) => {
      server = localBootstrap(7, request.expectedRevision + 1, request.value);
      return server.state.settings;
    });
    const api = fakePersonalApi({
      bootstrap,
      savedViews: vi.fn(async () => savedViewLibrary(server)),
      updateSettings,
    });
    const port = fakeSyncPort();
    const { personal, view } = renderProvider(api, server, { sync: port });

    await act(async () => {
      await personal().updateSettings({ ...DEFAULT_SETTINGS, volumePercent: 5 });
    });
    expect(port.publish).toHaveBeenCalledTimes(1);
    const published = port.publish.mock.calls[0]?.[0];
    expect(published).toMatchObject({ kind: 'MUTATED', at: expect.any(Number) });
    expect(typeof published?.tabId).toBe('string');
    expect(bootstrap).toHaveBeenCalledTimes(1);

    // The tab's own broadcast is ignored.
    await act(async () => {
      port.emit({ kind: 'MUTATED', tabId: published!.tabId, at: 2 });
      await Promise.resolve();
    });
    await pause();
    expect(bootstrap).toHaveBeenCalledTimes(1);

    // A peer's broadcast triggers exactly one refresh and never echoes.
    server = localBootstrap(7, 9);
    await act(async () => {
      port.emit({ kind: 'MUTATED', tabId: 'other', at: 3 });
      port.emit({ kind: 'MUTATED', tabId: 'other', at: 4 });
      await Promise.resolve();
    });
    await waitFor(() => expect(bootstrap).toHaveBeenCalledTimes(2));
    await pause();
    expect(bootstrap).toHaveBeenCalledTimes(2);
    expect(port.publish).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(screen.getByLabelText('Settings revision').textContent).toBe('9'));
    expect(screen.getByLabelText('Snapshot origin').textContent).toBe('PEER');

    // Hidden tabs defer until they become visible again.
    Object.defineProperty(document, 'visibilityState', { configurable: true, value: 'hidden' });
    await act(async () => {
      port.emit({ kind: 'MUTATED', tabId: 'other', at: 5 });
      await Promise.resolve();
    });
    await pause();
    expect(bootstrap).toHaveBeenCalledTimes(2);
    await act(async () => {
      setVisibility('visible');
      await Promise.resolve();
    });
    await waitFor(() => expect(bootstrap).toHaveBeenCalledTimes(3));

    view.unmount();
    expect(port.dispose).toHaveBeenCalled();
    await act(async () => {
      port.emit({ kind: 'MUTATED', tabId: 'other', at: 6 });
      setVisibility('hidden');
      setVisibility('visible');
      await Promise.resolve();
    });
    await pause();
    expect(bootstrap).toHaveBeenCalledTimes(3);
  });

  it('preserves settings object identity across refreshes so an unsaved draft survives', async () => {
    const initial = localBootstrap(7, 2);
    let server: LocalBootstrapData = {
      ...initial,
      state: { ...initial.state, settings: { revision: 2, value: { ...DEFAULT_SETTINGS } } },
    };
    const bootstrap = vi.fn(async () => server);
    const api = fakePersonalApi({ bootstrap, savedViews: vi.fn(async () => savedViewLibrary(server)) });
    const port = fakeSyncPort();
    const { personal } = renderProvider(api, initial, { sync: port }, <ConnectedSettingsPage />);
    const settingsBefore = personal().state.settings;
    const input = screen.getByLabelText('Catalog refresh interval (minutes)') as HTMLInputElement;
    fireEvent.change(input, { target: { value: '45' } });
    expect(input.value).toBe('45');

    await act(async () => {
      port.emit({ kind: 'MUTATED', tabId: 'other', at: 1 });
      await Promise.resolve();
    });
    await waitFor(() => expect(bootstrap).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(screen.getByLabelText('Snapshot origin').textContent).toBe('PEER'));
    expect(personal().state.settings).toBe(settingsBefore);
    expect(personal().state.settings.value).toBe(settingsBefore.value);
    expect(input.value).toBe('45');

    // A moved revision replaces the object and resets the form to the new baseline.
    server = localBootstrap(7, 3, { ...DEFAULT_SETTINGS, catalogRefreshMinutes: 90 });
    await act(async () => {
      port.emit({ kind: 'MUTATED', tabId: 'other', at: 2 });
      await Promise.resolve();
    });
    await waitFor(() => expect(bootstrap).toHaveBeenCalledTimes(2));
    await waitFor(() => expect(screen.getByLabelText('Settings revision').textContent).toBe('3'));
    expect(personal().state.settings).not.toBe(settingsBefore);
    await waitFor(() => expect(input.value).toBe('90'));

    // A snapshot older than the one held is ignored entirely.
    server = localBootstrap(6, 9);
    await act(async () => {
      port.emit({ kind: 'MUTATED', tabId: 'other', at: 3 });
      await Promise.resolve();
    });
    await waitFor(() => expect(bootstrap).toHaveBeenCalledTimes(3));
    await pause();
    expect(personal().state.stateRevision).toBe(7);
    expect(screen.getByLabelText('Settings revision').textContent).toBe('3');
  });

  it('treats a name conflict after a transient retry as success when the view now exists', async () => {
    const initial = localBootstrapWithV2Filters();
    const compatible = initial.state.currentFilters.value?.content;
    if (compatible?.status !== 'COMPATIBLE') throw new Error('Expected compatible fixture.');
    const created = {
      id: '40000000-0000-4000-8000-000000000001',
      name: 'Evening options',
      schemaVersion: 2,
      revision: 1,
      content: compatible,
      createdAt: 1,
      updatedAt: 1,
    } as const;
    let server = initial;
    const createSavedView = vi.fn<LocalPersonalApiPort['createSavedView']>(async (request) => {
      if (createSavedView.mock.calls.length === 1) {
        // The write committed but the response was lost.
        server = { ...server, state: { ...server.state, savedViews: [...server.state.savedViews, created] } };
        throw new TypeError('fetch failed');
      }
      throw apiError(409, 'SAVED_VIEW_NAME_CONFLICT', 1);
    });
    const duplicateSavedView = vi.fn<LocalPersonalApiPort['duplicateSavedView']>(async () => {
      if (duplicateSavedView.mock.calls.length === 1) throw apiError(503, 'STORAGE_BUSY');
      throw apiError(409, 'SAVED_VIEW_NAME_CONFLICT', 1);
    });
    const port = fakeSyncPort();
    const api = fakePersonalApi({
      bootstrap: vi.fn(async () => server),
      savedViews: vi.fn(async () => savedViewLibrary(server)),
      createSavedView,
      duplicateSavedView,
    });
    const { personal } = renderProvider(api, initial, { sync: port });

    await act(async () => {
      await personal().createSavedView('Evening options');
    });
    expect(createSavedView).toHaveBeenCalledTimes(2);
    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('IDLE'));
    expect(personal().error).toBeNull();
    expect(personal().state.savedViews.map((view) => view.name)).toContain('Evening options');
    expect(port.publish).toHaveBeenCalledTimes(1);

    // The same conflict for a name the library does not hold is still a rejection.
    let outcome: Promise<unknown> | undefined;
    await act(async () => {
      outcome = personal().duplicateSavedView(SAVED_VIEW, 'Missing copy').catch((error: unknown) => error);
      await Promise.resolve();
    });
    await expect(outcome).resolves.toBeInstanceOf(ProductClientError);
    expect(duplicateSavedView).toHaveBeenCalledTimes(2);
    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('FAILED:REJECTED'));
    expect(screen.getByLabelText('Sync message').textContent).toBe('local.status.error');
    expect(port.publish).toHaveBeenCalledTimes(1);
  });

  it('rejects a first-attempt name conflict without consulting the library', async () => {
    const initial = localBootstrapWithV2Filters();
    const bootstrap = vi.fn(async () => initial);
    const createSavedView = vi.fn<LocalPersonalApiPort['createSavedView']>(async () => {
      throw apiError(409, 'SAVED_VIEW_NAME_CONFLICT', 2);
    });
    const api = fakePersonalApi({
      bootstrap,
      savedViews: vi.fn(async () => savedViewLibrary(initial)),
      createSavedView,
    });
    const { personal } = renderProvider(api, initial);

    let outcome: Promise<unknown> | undefined;
    await act(async () => {
      outcome = personal().createSavedView('Data structures').catch((error: unknown) => error);
      await Promise.resolve();
    });
    await expect(outcome).resolves.toBeInstanceOf(ProductClientError);
    expect(createSavedView).toHaveBeenCalledTimes(1);
    expect(bootstrap).not.toHaveBeenCalled();
    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('FAILED:REJECTED'));
  });

  it('routes a failed term pull through the same classifier', async () => {
    const initial = localBootstrap();
    const pullTerm = vi.fn<LocalPersonalApiPort['pullTerm']>(async () => {
      throw apiError(503, 'STORAGE_BUSY');
    });
    const api = fakePersonalApi({ pullTerm, savedViews: vi.fn(async () => savedViewLibrary(initial)) });
    const { personal } = renderProvider(api, initial);

    let outcome: Promise<unknown> | undefined;
    await act(async () => {
      outcome = personal().pullTerm('12026').catch((error: unknown) => error);
      await Promise.resolve();
    });
    await expect(outcome).resolves.toBeInstanceOf(ProductClientError);
    expect(pullTerm).toHaveBeenCalledTimes(1);
    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('FAILED:UNAVAILABLE'));
    expect(personal().error).toBeInstanceOf(ProductClientError);
    await act(async () => {
      personal().clearError();
    });
    await waitFor(() => expect(screen.getByLabelText('Sync phase').textContent).toBe('IDLE'));
    expect(personal().error).toBeNull();
  });

  it('offers a page reload for a reset from another window and specific failure headlines', async () => {
    const error = new Error('local.error.user_state_revision_conflict');
    const reset: LocalSyncStatus = { phase: 'FAILED', reason: 'STATE_RESET', error };
    const reload = vi.fn(async () => undefined);
    const view = render(
      <BcspI18nProvider initialLocale="en-US">
        <LocalPageFrame error={error} intro="i" kicker="k" notice={reset} onReload={reload} title="t">
          <p>body</p>
        </LocalPageFrame>
      </BcspI18nProvider>,
    );
    const alert = screen.getByRole('alert');
    expect(alert.textContent).toContain('Local data was reset from another window. Reload this page to continue.');
    expect(screen.getByRole('button', { name: 'Reload page' })).toBeInstanceOf(HTMLButtonElement);
    expect(screen.getByRole('button', { name: 'Reload local state' })).toBeInstanceOf(HTMLButtonElement);
    expect(reload).not.toHaveBeenCalled();
    view.unmount();

    const conflict: LocalSyncStatus = { phase: 'FAILED', reason: 'CONFLICT', error };
    render(
      <BcspI18nProvider initialLocale="en-US">
        <LocalPageFrame error={error} intro="i" kicker="k" notice={conflict} title="t">
          <p>body</p>
        </LocalPageFrame>
      </BcspI18nProvider>,
    );
    expect(screen.getByRole('alert').textContent).toContain(
      'Another window changed the same data, so this change was not applied.',
    );
    expect(screen.queryByRole('button', { name: 'Reload page' })).toBeNull();
  });

  it('shows the specific failure headline on the settings form after a failed save', async () => {
    const error = new Error('local.error.settings_revision_conflict');
    const notice: LocalSyncStatus = { phase: 'FAILED', reason: 'UNAVAILABLE', error };
    render(
      <BcspI18nProvider initialLocale="en-US">
        <SettingsPage
          notice={notice}
          onConfirmUserDataReset={async () => undefined}
          onDeleteAllSavedViews={async () => undefined}
          onPrepareUserDataReset={async () => ({
            confirmationToken: SESSION,
            expectedUserStateRevision: 7,
            expiresInSeconds: 60,
          })}
          onResetCurrentFilters={async () => undefined}
          onUpdateSettings={async () => {
            throw error;
          }}
          savedViewCount={0}
          settings={{ revision: 2, value: DEFAULT_SETTINGS }}
        />
      </BcspI18nProvider>,
    );
    fireEvent.change(screen.getByLabelText('Catalog refresh interval (minutes)'), { target: { value: '45' } });
    fireEvent.click(screen.getByRole('button', { name: 'Save settings' }));
    // The volume <output> also carries an implicit status role; read the save status line.
    await waitFor(() => expect(document.querySelector('p[data-state="FAILED"]')?.textContent).toBe(
      'The local service did not respond, so this change was not saved. Try again or reload the local state.',
    ));
    expect(screen.queryByRole('alert')).toBeNull();
  });
});
