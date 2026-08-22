// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  LocalPersonalApi,
  LocalPersonalProvider,
  parseLocalBootstrapData,
  useLocalPersonal,
  useLocalPersonalOptional,
  type LocalBootstrapData,
  type LocalPersonalApiPort,
  type LocalSettings,
  type SavedViewLibrary,
  type StoredSettings,
} from '../src/ui/local/personal';
import { LocalProductBootstrap } from '../src/ui/local/product/LocalProductBootstrap';
import { PublicProductBootstrap } from '../src/ui/public/product/PublicProductBootstrap';
import {
  createNeutralFilterState,
  ProductClient,
  ProductBootstrapError,
  toFilterRequestV1,
  useProductRuntimeState,
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
      <LocalPersonalProvider api={api} initialBootstrap={server}>
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
