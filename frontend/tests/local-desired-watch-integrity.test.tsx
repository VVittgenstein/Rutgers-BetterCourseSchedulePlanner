// @vitest-environment jsdom

import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { BcspI18nProvider } from '../src/ui/shared/i18n/runtime';
import { AppRouterProvider } from '../src/ui/shared/routing';
import type {
  ProductApiPort,
  ProductRuntimePort,
  SectionKey,
  TraceId,
  WatchClientCommandV1,
  WatchClientPort,
  WatchConnectionState,
  WatchPolicyV1,
  WatchServerEventV1,
  WsServerEnvelope,
} from '../src/ui/shared/product';
import {
  LiveWatchProvider,
  SectionSelectionAction,
  WatchWorkspace,
  useLiveWatch,
  type LiveWatchValue,
} from '../src/ui/shared/watch';
import { createLocalDesiredWatchApi } from '../src/ui/local/desired';
import { WatchAudioController } from '../src/ui/shared/watch/audio';

const SECTION: SectionKey = { term: 'T2030F', campus: 'NB', index: '00001' };
const OTHER: SectionKey = { term: 'T2030F', campus: 'NB', index: '00002' };
const POLICY: WatchPolicyV1 = {
  notificationMode: 'ONE_SHOT',
  maxAudible: 3,
  continuousDuration: { kind: 'FINITE', seconds: 600 },
};
const WATCH_ID = '00000000-0000-4000-8000-0000000000a1';

interface EntryOptions {
  readonly policy?: WatchPolicyV1 | null;
  readonly revision?: number;
  readonly epoch?: number;
  readonly running?: boolean;
  readonly pendingDisarm?: boolean;
}

function entry(section: SectionKey, options: EntryOptions = {}) {
  const revision = options.revision ?? 1;
  const epoch = options.epoch ?? 1;
  const policy = options.policy === undefined ? POLICY : options.policy;
  return {
    section,
    policy,
    revision,
    materializationEpoch: epoch,
    materialized: options.running === true
      ? {
        authorityGeneration: 1,
        revision,
        materializationEpoch: epoch,
        policy: policy ?? POLICY,
        activeWatchId: WATCH_ID,
      }
      : null,
    pendingDisarm: options.pendingDisarm ?? false,
    blockedOnSlot: false,
    failure: null,
  };
}

function snapshot(entries: readonly unknown[], generation = 1) {
  return { contractVersion: 1, authorityGeneration: generation, entries };
}

function committed(state: unknown, revision = 2) {
  return {
    contractVersion: 1,
    outcome: 'COMMITTED',
    replayed: false,
    authorityGeneration: 1,
    currentRevision: null,
    maximum: null,
    committed: { revision, materializationEpoch: revision, epochChanged: true },
    state,
  };
}

function ok(data: unknown, status = 200): Response {
  return new Response(JSON.stringify({ protocolVersion: 1, data }), {
    headers: { 'content-type': 'application/json' },
    status,
  });
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((resolvePromise) => {
    resolve = resolvePromise;
  });
  return { promise, resolve };
}

class FakeWatch implements WatchClientPort {
  state: WatchConnectionState = 'OPEN';
  readonly commands: WatchClientCommandV1[] = [];
  readonly connect = vi.fn(() => undefined);
  readonly disconnect = vi.fn(() => undefined);
  readonly #events = new Set<(envelope: WsServerEnvelope<WatchServerEventV1>) => void>();
  readonly #states = new Set<(state: WatchConnectionState) => void>();
  #message = 0;

  send(command: WatchClientCommandV1): TraceId {
    this.commands.push(command);
    this.#message += 1;
    return `00000000-0000-4000-8000-${this.#message.toString(16).padStart(12, '0')}`;
  }

  emit(payload: WatchServerEventV1): void {
    this.#message += 1;
    const envelope = {
      protocolVersion: 1,
      messageId: `00000000-0000-4000-8000-${this.#message.toString(16).padStart(12, '0')}`,
      payload,
    } as unknown as WsServerEnvelope<WatchServerEventV1>;
    for (const listener of this.#events) listener(envelope);
  }

  subscribe(listener: (envelope: WsServerEnvelope<WatchServerEventV1>) => void): () => void {
    this.#events.add(listener);
    return () => this.#events.delete(listener);
  }

  subscribeState(listener: (state: WatchConnectionState) => void): () => void {
    this.#states.add(listener);
    return () => this.#states.delete(listener);
  }
}

function unexpected(): never {
  throw new Error('unexpected product API call');
}

const COUNTS = { attempted: 1, succeeded: 1, failed: 0, empty: 0 };
const FRESHNESS = {
  state: 'UNKNOWN' as const,
  observedAt: null,
  freshUntil: null,
  lastKnownGoodAgeSeconds: null,
  uncertainty: 'NEVER_OBSERVED' as const,
};
const SNAPSHOT = {
  runCounts: COUNTS,
  todayCounts: COUNTS,
  rutgersDay: '2030-01-01',
  dayTimezone: 'America/New_York' as const,
};

function productApi(): ProductApiPort {
  return {
    catalogDiscovery: unexpected,
    courseDetail: unexpected,
    filterSchema: unexpected,
    openSectionStatus: vi.fn(async ({ sectionKey }) => ({
      contractVersion: 1 as const,
      sectionKey,
      state: 'UNKNOWN' as const,
      lastObservationId: null,
      catalogContentVersion: 1,
      freshness: FRESHNESS,
      schedulerLagMilliseconds: 0,
      counterSnapshot: SNAPSHOT,
    })),
    openStatus: vi.fn(async ({ batch }) => ({
      contractVersion: 1 as const,
      batch,
      catalogContentVersion: 1,
      latestAttempt: null,
      latestFailure: null,
      lastValidObservation: null,
      lastBodyChangeAt: null,
      lastStateChangeAt: null,
      freshness: FRESHNESS,
      scheduler: {
        lane: 'GENERAL' as const,
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
        state: 'CLOSED' as const,
        reason: null,
        openedAt: null,
        retryAt: null,
        diagnosticRecheckRequired: false,
      },
      counterSnapshot: SNAPSHOT,
    })),
    searchCourses: unexpected,
    searchSections: unexpected,
    sectionDetail: unexpected,
  } as ProductApiPort;
}

function silentAudio(): WatchAudioController {
  return new (class {
    unlock = vi.fn(async () => 'READY' as const);
    play = vi.fn(() => 'STARTED' as const);
    preview = vi.fn(() => 'STARTED' as const);
    startContinuous = vi.fn(() => 'STARTED' as const);
    stopContinuous = vi.fn();
    dispose = vi.fn();
  })() as unknown as WatchAudioController;
}

interface Call {
  readonly method: string;
  readonly body: unknown;
}

/**
 * A local authority whose answers a test resolves by hand.
 *
 * The reordering these tests are about is invisible with immediate
 * responses: every answer arrives in the order it was asked for, so any
 * implementation looks correct. Holding one open is the only way to ask
 * whether a stale answer can overwrite a fresh one.
 */
function authority() {
  const calls: Call[] = [];
  const responders: ((call: Call) => Promise<Response>)[] = [];
  const fetchImplementation = vi.fn(async (_input: RequestInfo | URL, init?: RequestInit) => {
    const call: Call = {
      method: init?.method ?? 'GET',
      body: typeof init?.body === 'string' ? JSON.parse(init.body) as unknown : null,
    };
    calls.push(call);
    const responder = responders.length > 1 ? responders.shift() : responders[0];
    if (responder === undefined) throw new Error('no responder queued');
    return await responder(call);
  }) as unknown as typeof fetch;
  return {
    calls,
    fetchImplementation,
    queue(...next: readonly ((call: Call) => Promise<Response>)[]) {
      responders.push(...next);
    },
    writes: () => calls.filter((call) => call.method === 'PUT'),
    reads: () => calls.filter((call) => call.method === 'GET'),
  };
}

/** One row of the watch desk's managed list, by Section index. */
function deskRow(section: SectionKey): HTMLElement {
  const list = document.querySelector('.watch-workspace__list');
  if (list === null) throw new Error('the watch desk rendered no managed list');
  const row = within(list as HTMLElement).getByText(section.index).closest('li');
  if (row === null) throw new Error(`the watch desk has no row for ${section.index}`);
  return row;
}

function Probe({ publish }: { readonly publish: (value: LiveWatchValue) => void }) {
  publish(useLiveWatch());
  return null;
}

interface DeskOptions {
  readonly selected?: readonly SectionKey[];
  readonly sections?: readonly SectionKey[];
}

function desk(
  fetchImplementation: typeof fetch,
  options: DeskOptions = {},
) {
  const intent = createLocalDesiredWatchApi({
    fetch: fetchImplementation,
    session: () => '00000000-0000-4000-8000-00000000f00d',
  });
  const watch = new FakeWatch();
  const runtime: ProductRuntimePort = { product: productApi(), watch, dispose: vi.fn() };
  let current: LiveWatchValue | null = null;
  const result = render(
    <AppRouterProvider initialPath="/">
      <BcspI18nProvider initialLocale="en-US">
        <LiveWatchProvider
          audio={silentAudio()}
          initialSelected={options.selected ?? [SECTION]}
          intent={intent}
          runtime={runtime}
        >
          <div aria-label="Section selection fixtures">
            {(options.sections ?? []).map((sectionKey) => (
              <SectionSelectionAction key={sectionKey.index} sectionKey={sectionKey} />
            ))}
          </div>
          <WatchWorkspace />
          <Probe publish={(value) => { current = value; }} />
        </LiveWatchProvider>
      </BcspI18nProvider>
    </AppRouterProvider>,
  );
  return {
    ...result,
    watch,
    value() {
      if (current === null) throw new Error('LiveWatch context was not published');
      return current;
    },
  };
}

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe('one ordered domain for every authority answer', () => {
  it('never lets an older read put back intent a newer write removed', async () => {
    const server = authority();
    const held = deferred<void>();
    // The mount read, then a read that is held open, then the STOP.
    server.queue(
      async () => ok(snapshot([entry(SECTION, { running: true })])),
      async () => {
        await held.promise;
        // The state as it was BEFORE the stop: this is the answer that must
        // not be allowed to reappear on top of the tombstone.
        return ok(snapshot([entry(SECTION, { running: true })]));
      },
      async () => ok(committed(snapshot([
        entry(SECTION, { policy: null, revision: 2, epoch: 2 }),
      ]))),
    );
    const view = desk(server.fetchImplementation);
    await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('WATCHING'));

    let refreshed: Promise<void> | null = null;
    let stopped: Promise<boolean> | null = null;
    await act(async () => {
      refreshed = view.value().refreshIntent();
      stopped = view.value().setSectionIntent(SECTION, null);
      // Let anything that would race actually race.
      await Promise.resolve();
      await Promise.resolve();
      held.resolve();
      await refreshed;
      await stopped;
    });

    expect(view.value().intentStateFor(SECTION)).toBe('NOT_WATCHING');
    expect(view.value().intent?.entries[0]?.policy).toBeNull();
    expect(server.writes()).toHaveLength(1);
  });

  it('never lets an older write put back a section a newer write removed', async () => {
    const server = authority();
    const held = deferred<void>();
    const stopped = new Set<string>();
    server.queue(
      async () => ok(snapshot([
        entry(SECTION, { running: true }),
        entry(OTHER, { running: true }),
      ])),
      async (call) => {
        const payload = (call.body as { payload: { section: SectionKey } }).payload;
        stopped.add(payload.section.index);
        const state = snapshot([
          entry(SECTION, stopped.has(SECTION.index)
            ? { policy: null, revision: 3, epoch: 3 }
            : { running: true }),
          entry(OTHER, stopped.has(OTHER.index)
            ? { policy: null, revision: 4, epoch: 4 }
            : { running: true }),
        ]);
        // The FIRST write is the one held open, so an implementation that
        // sends both at once applies the second answer and then the first.
        if (payload.section.index === SECTION.index) await held.promise;
        return ok(committed(state, payload.section.index === SECTION.index ? 3 : 4));
      },
    );
    const view = desk(server.fetchImplementation, { selected: [SECTION, OTHER] });
    await waitFor(() => expect(view.value().intentStateFor(OTHER)).toBe('WATCHING'));

    await act(async () => {
      const first = view.value().setSectionIntent(SECTION, null);
      const second = view.value().setSectionIntent(OTHER, null);
      await Promise.resolve();
      await Promise.resolve();
      held.resolve();
      await first;
      await second;
    });

    expect(view.value().intentStateFor(SECTION)).toBe('NOT_WATCHING');
    expect(view.value().intentStateFor(OTHER)).toBe('NOT_WATCHING');
  });

  it('re-reads through the same domain when a retry finally arms the watch', async () => {
    const server = authority();
    server.queue(
      async () => ok(snapshot([entry(SECTION)])),
      async () => ok(snapshot([entry(SECTION, { running: true })])),
    );
    const view = desk(server.fetchImplementation);
    await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('PREPARING'));

    // The server armed it in the background. The only thing that says so is
    // the owner's START_RESULT, which is not addressed to this page.
    await act(async () => {
      view.watch.emit({
        type: 'START_RESULT',
        result: {
          contractVersion: 1,
          activeWatchCount: 1,
          items: [{
            status: 'ACTIVE',
            sectionKey: SECTION,
            activeWatchId: WATCH_ID,
            startedAt: '2030-01-01T00:00:00.000Z',
          }],
        },
      } as WatchServerEventV1);
    });

    await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('WATCHING'));
  });

  it('stops showing a watch as running after an unexpected stop', async () => {
    const server = authority();
    server.queue(
      async () => ok(snapshot([entry(SECTION, { running: true })])),
      async () => ok(snapshot([entry(SECTION)])),
    );
    const view = desk(server.fetchImplementation);
    await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('WATCHING'));

    await act(async () => {
      view.watch.emit({
        type: 'WATCH_STOPPED',
        stopped: {
          contractVersion: 1,
          activeWatchId: WATCH_ID,
          sectionKey: SECTION,
          reason: 'TERM_OUT_OF_RANGE',
          stoppedAt: '2030-01-01T00:00:00.000Z',
        },
      } as WatchServerEventV1);
    });

    await waitFor(() => expect(view.value().intentStateFor(SECTION)).not.toBe('WATCHING'));
  });
});

describe('a page that joined late still sees what is running', () => {
  it('counts, describes and can act on a watch it never saw start', async () => {
    const server = authority();
    server.queue(async () => ok(snapshot([entry(SECTION, { running: true })])));
    const view = desk(server.fetchImplementation, { sections: [SECTION] });
    await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('WATCHING'));

    // No START_RESULT ever reached this page.
    expect(view.watch.commands).toEqual([]);
    expect(view.value().intent?.entries[0]?.running?.activeWatchId).toBe(WATCH_ID);
    expect(view.value().active).toHaveLength(1);
    expect(view.value().active[0]?.activeWatchId).toBe(WATCH_ID);
    expect(view.value().isActive(SECTION)).toBe(true);
    // The connection line, the top count and the audio prompt all read from
    // the same list, so all three say the same thing.
    expect(screen.getByText('Watching 1 Section (sound muted)')).toBeTruthy();
    const metric = screen.getByText('Active').closest('.bcsp-metric');
    expect(within(metric as HTMLElement).getByText('1')).toBeTruthy();

    // The episode controls for that watch are reachable, addressed by the id
    // the authority read carried -- not by a START frame this page never saw.
    const row = deskRow(SECTION);
    const clear = within(row).getByRole('button', {
      name: 'Reset count',
    });
    act(() => { clear.click(); });
    expect(view.watch.commands).toEqual([{
      type: 'RESET_AUDIBLE_COUNT',
      watch: { activeWatchId: WATCH_ID, sectionKey: SECTION },
    }]);

    // And the section cannot be quietly taken off the only list it can be
    // stopped from.
    expect(view.value().isRemovable(SECTION)).toBe(false);
    const fixtures = screen.getByLabelText('Section selection fixtures');
    const button = within(fixtures).getByRole('button');
    expect(button.hasAttribute('disabled')).toBe(true);
    act(() => { view.value().remove(SECTION); });
    expect(view.value().selected).toHaveLength(1);
  });

  it.each([
    ['a read that has not answered yet', false],
    ['a read that failed', true],
  ])('fails closed while the authority is unreadable: %s', async (_label, fail) => {
    const server = authority();
    const held = deferred<void>();
    server.queue(async () => {
      await held.promise;
      if (fail) return new Response('nope', { status: 500 });
      return ok(snapshot([entry(SECTION, { running: true })]));
    });
    const view = desk(server.fetchImplementation, { sections: [SECTION] });
    if (fail) {
      await act(async () => {
        held.resolve();
        await Promise.resolve();
      });
      await waitFor(() => expect(view.value().intentStatus).toBe('FAILED'));
    }

    expect(view.value().isRemovable(SECTION)).toBe(false);
    act(() => { view.value().remove(SECTION); });
    expect(view.value().selected).toHaveLength(1);
    const fixtures = screen.getByLabelText('Section selection fixtures');
    expect(within(fixtures).getByRole('button').hasAttribute('disabled')).toBe(true);
    if (!fail) held.resolve();
  });

  it('shows and can stop a saved watch that is no longer in the selection', async () => {
    const server = authority();
    server.queue(
      async () => ok(snapshot([entry(OTHER, { running: true })])),
      async () => ok(committed(snapshot([
        entry(OTHER, { policy: null, revision: 2, epoch: 2 }),
      ]))),
    );
    // The user's selection knows nothing about OTHER. The server does.
    const view = desk(server.fetchImplementation, { selected: [] });
    await waitFor(() => expect(view.value().intentStateFor(OTHER)).toBe('WATCHING'));

    expect(screen.getByText('Saved watch')).toBeTruthy();
    const stop = within(deskRow(OTHER)).getByRole('button', { name: 'Stop' });
    await act(async () => { stop.click(); });

    await waitFor(() => expect(view.value().intentStateFor(OTHER)).toBe('NOT_WATCHING'));
    expect(server.writes()).toHaveLength(1);
  });
});

describe('the mutation answer is decoded as strictly as the read', () => {
  function port(response: () => Response) {
    return createLocalDesiredWatchApi({
      fetch: (async () => response()) as unknown as typeof fetch,
      session: () => '00000000-0000-4000-8000-00000000f00d',
    });
  }

  const state = snapshot([entry(SECTION, { policy: null, revision: 2, epoch: 2 })]);

  it.each([
    [
      'an outcome nothing knows',
      () => ok({ ...committed(state), outcome: 'RETIRED' }),
    ],
    [
      'a committed answer on a refusal status',
      () => ok(committed(state), 409),
    ],
    [
      'a refusal on a success status',
      () => ok({
        contractVersion: 1,
        outcome: 'STALE_REVISION',
        replayed: false,
        authorityGeneration: 1,
        currentRevision: 9,
        maximum: null,
        committed: null,
        state: null,
      }),
    ],
    [
      'a commit with no state to show',
      () => ok({ ...committed(state), state: null }),
    ],
    [
      'a commit that does not say what it wrote',
      () => ok({ ...committed(state), committed: null }),
    ],
    [
      'a negative maximum',
      () => ok({
        contractVersion: 1,
        outcome: 'LIMIT_EXCEEDED',
        replayed: false,
        authorityGeneration: 1,
        currentRevision: null,
        maximum: -1,
        committed: null,
        state: null,
      }, 409),
    ],
    [
      'a field nobody agreed to',
      () => ok({ ...committed(state), future: true }),
    ],
    [
      'a refusal carrying a state it must not have',
      () => ok({
        contractVersion: 1,
        outcome: 'AUTHORITY_FULL',
        replayed: false,
        authorityGeneration: 1,
        currentRevision: null,
        maximum: 2048,
        committed: null,
        state,
      }, 503),
    ],
  ])('refuses %s', async (_label, response) => {
    await expect(port(response).submit(
      { section: SECTION, policy: null },
      snapshot([entry(SECTION)]) as never,
    )).rejects.toThrow();
  });

  it('accepts the answers the authority really gives', async () => {
    const commit = await port(() => ok(committed(state))).submit(
      { section: SECTION, policy: null },
      snapshot([entry(SECTION)]) as never,
    );
    expect(commit.outcome).toBe('COMMITTED');
    expect(commit.snapshot?.entries[0]?.policy).toBeNull();

    const full = await port(() => ok({
      contractVersion: 1,
      outcome: 'AUTHORITY_FULL',
      replayed: false,
      authorityGeneration: 1,
      currentRevision: null,
      maximum: 2048,
      committed: null,
      state: null,
    }, 503)).submit(
      { section: SECTION, policy: null },
      snapshot([entry(SECTION)]) as never,
    );
    expect(full.outcome).toBe('UNAVAILABLE');
    expect(full.maximum).toBe(2048);
    expect(full.snapshot).toBeNull();
  });
});
