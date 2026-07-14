// @vitest-environment jsdom

import { useEffect } from 'react';
import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { LocalProductBootstrap } from '../src/ui/local/product/LocalProductBootstrap';
import { PublicProductBootstrap } from '../src/ui/public/product/PublicProductBootstrap';
import {
  createCourseQueryRequestV1,
  createNeutralFilterState,
  createSectionQueryRequestV1,
  parseProductSessionBootstrap,
  ProductBootstrapError,
  useProductRuntimeState,
  WATCH_SUBPROTOCOL,
  type ProductRuntimePort,
  type WatchSocket,
} from '../src/ui/shared/product';

const FIRST_SESSION = '10000000-0000-4000-8000-000000000001';
const SECOND_SESSION = '20000000-0000-4000-8000-000000000002';

function bootstrapEnvelope(sessionNonce: string): unknown {
  return {
    protocolVersion: 1,
    data: { sessionNonce },
  };
}

function localBootstrapEnvelope(sessionNonce: string): unknown {
  return {
    protocolVersion: 1,
    data: {
      mode: 'LOCAL',
      sessionNonce,
      state: {
        stateRevision: 0,
        settings: {
          revision: 0,
          value: {
            localeOverride: 'system',
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
        currentFilters: { stateRevision: 0, revision: 0, value: null },
        savedViews: [],
        selectedSections: [],
        episodeHistory: { items: [], total: 0, offset: 0, limit: 50 },
        activeWatchCount: 0,
      },
    },
  };
}

class FakeSocket implements WatchSocket {
  readyState = 0;
  closeCount = 0;
  readonly listeners = new Map<string, Set<() => void>>();

  addEventListener(
    type: 'open' | 'close' | 'error' | 'message',
    listener: (() => void) | ((event: { readonly data: unknown }) => void),
  ): void {
    const listeners = this.listeners.get(type) ?? new Set();
    listeners.add(listener as () => void);
    this.listeners.set(type, listeners);
  }

  close(): void {
    this.closeCount += 1;
    this.readyState = 3;
    this.listeners.get('close')?.forEach((listener) => listener());
  }

  send(): void {}
}

function RuntimeProbe({ onReady }: { readonly onReady: (runtime: ProductRuntimePort) => void }) {
  const state = useProductRuntimeState();
  useEffect(() => {
    if (state.status === 'READY') onReady(state.runtime);
  }, [onReady, state]);
  return (
    <output data-testid="runtime-state">
      {state.status === 'ERROR' ? `${state.status}:${state.reason}` : state.status}
    </output>
  );
}

function embeddedBootstrap(sessionNonce: string): HTMLScriptElement {
  const script = document.createElement('script');
  script.id = 'bcsp-bootstrap';
  script.type = 'application/json';
  script.textContent = JSON.stringify(bootstrapEnvelope(sessionNonce));
  document.body.prepend(script);
  return script;
}

afterEach(() => {
  cleanup();
  document.querySelectorAll('[id="bcsp-bootstrap"]').forEach((element) => element.remove());
  vi.restoreAllMocks();
});

describe('product bootstrap contract', () => {
  it('accepts only a v1 envelope with a canonical random-v4 session nonce', () => {
    expect(parseProductSessionBootstrap(bootstrapEnvelope(FIRST_SESSION))).toEqual({
      sessionNonce: FIRST_SESSION,
    });
    for (const value of [
      undefined,
      { protocolVersion: 2, data: { sessionNonce: FIRST_SESSION } },
      { protocolVersion: 1, data: { sessionNonce: 'not-a-session' } },
      { protocolVersion: 1, data: { sessionNonce: FIRST_SESSION }, extra: true },
    ]) {
      expect(() => parseProductSessionBootstrap(value)).toThrow(ProductBootstrapError);
    }
  });

  it('loads the loopback bootstrap once, propagates its session, and disposes the runtime', async () => {
    const requests: Array<{ readonly init?: RequestInit; readonly url: string }> = [];
    const fetchMock = vi.fn<typeof fetch>(async (input, init) => {
      requests.push({ ...(init === undefined ? {} : { init }), url: String(input) });
      return requests.length === 1
        ? Response.json(localBootstrapEnvelope(FIRST_SESSION))
        : Response.json({ protocolVersion: 1, data: { accepted: true } });
    });
    const socket = new FakeSocket();
    const socketFactory = vi.fn(() => socket);
    let runtime: ProductRuntimePort | null = null;
    const onReady = vi.fn((value: ProductRuntimePort) => {
      runtime = value;
    });
    const view = render(
      <LocalProductBootstrap
        baseUrl="https://planner.invalid/"
        fetch={fetchMock}
        socket={socketFactory}
      >
        <RuntimeProbe onReady={onReady} />
      </LocalProductBootstrap>,
    );

    await waitFor(() => expect(screen.getByTestId('runtime-state').textContent).toBe('READY'));
    expect(requests[0]?.url).toBe('https://planner.invalid/api/v1/local/bootstrap');
    expect(requests[0]?.init).toMatchObject({
      cache: 'no-store',
      credentials: 'same-origin',
      method: 'GET',
    });
    if (runtime === null) throw new Error('runtime was not published');
    await act(async () => {
      await (runtime as ProductRuntimePort).product.searchCourses(
        createCourseQueryRequestV1(createNeutralFilterState('T2026F')),
      );
    });
    const courseSearch = requests.find(({ url }) => url.endsWith('/api/v1/query/courses'));
    expect(new Headers(courseSearch?.init?.headers).get('x-bcsp-session')).toBe(FIRST_SESSION);
    (runtime as ProductRuntimePort).watch.connect();
    expect(socketFactory).toHaveBeenCalledWith(
      `wss://planner.invalid/api/v1/watch?session=${FIRST_SESSION}`,
      [WATCH_SUBPROTOCOL],
    );

    view.unmount();
    expect(socket.closeCount).toBe(1);
  });

  it('fails closed when the loopback request or envelope is invalid', async () => {
    const unavailable = render(
      <LocalProductBootstrap fetch={vi.fn<typeof fetch>(async () => new Response(null, { status: 503 }))}>
        <RuntimeProbe onReady={() => undefined} />
      </LocalProductBootstrap>,
    );
    await waitFor(() => expect(screen.getByTestId('runtime-state').textContent).toBe(
      'ERROR:BOOTSTRAP_REQUEST_FAILED',
    ));
    unavailable.unmount();

    render(
      <LocalProductBootstrap fetch={vi.fn<typeof fetch>(async () => Response.json({ nope: true }))}>
        <RuntimeProbe onReady={() => undefined} />
      </LocalProductBootstrap>,
    );
    await waitFor(() => expect(screen.getByTestId('runtime-state').textContent).toBe(
      'ERROR:BOOTSTRAP_INVALID',
    ));
  });

  it('fails closed when the embedded bootstrap is absent or malformed', async () => {
    const missing = render(
      <PublicProductBootstrap>
        <RuntimeProbe onReady={() => undefined} />
      </PublicProductBootstrap>,
    );
    await waitFor(() => expect(screen.getByTestId('runtime-state').textContent).toBe(
      'ERROR:BOOTSTRAP_UNAVAILABLE',
    ));
    missing.unmount();

    const script = embeddedBootstrap(FIRST_SESSION);
    script.textContent = '{bad json';
    render(
      <PublicProductBootstrap>
        <RuntimeProbe onReady={() => undefined} />
      </PublicProductBootstrap>,
    );
    await waitFor(() => expect(screen.getByTestId('runtime-state').textContent).toBe(
      'ERROR:BOOTSTRAP_INVALID',
    ));
  });

  it('uses the current document session without storage and gets a new one after remount', async () => {
    const storageReads = vi.fn();
    const storageDescriptors = ['localStorage', 'sessionStorage'].map((property) => [
      property,
      Object.getOwnPropertyDescriptor(window, property),
    ] as const);
    for (const [property] of storageDescriptors) {
      Object.defineProperty(window, property, {
        configurable: true,
        get() {
          storageReads(property);
          throw new Error('storage access is forbidden');
        },
      });
    }

    const requests: RequestInit[] = [];
    const fetchMock = vi.fn<typeof fetch>(async (_input, init) => {
      requests.push(init ?? {});
      return Response.json({ protocolVersion: 1, data: { accepted: true } });
    });
    try {
      for (const session of [FIRST_SESSION, SECOND_SESSION]) {
        const script = embeddedBootstrap(session);
        let runtime: ProductRuntimePort | null = null;
        const view = render(
          <PublicProductBootstrap baseUrl="https://planner.invalid/path" fetch={fetchMock}>
            <RuntimeProbe onReady={(value) => { runtime = value; }} />
          </PublicProductBootstrap>,
        );
        await waitFor(() => expect(screen.getByTestId('runtime-state').textContent).toBe('READY'));
        if (runtime === null) throw new Error('runtime was not published');
        await act(async () => {
          await (runtime as ProductRuntimePort).product.searchSections(
            createSectionQueryRequestV1(createNeutralFilterState('T2026F')),
          );
        });
        view.unmount();
        script.remove();
      }
    } finally {
      for (const [property, descriptor] of storageDescriptors) {
        if (descriptor === undefined) Reflect.deleteProperty(window, property);
        else Object.defineProperty(window, property, descriptor);
      }
    }

    expect(requests.map((init) => new Headers(init.headers).get('x-bcsp-session'))).toEqual([
      FIRST_SESSION,
      SECOND_SESSION,
    ]);
    expect(storageReads).not.toHaveBeenCalled();
  });
});
