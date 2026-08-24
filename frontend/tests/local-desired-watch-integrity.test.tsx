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

  setState(state: WatchConnectionState): void {
    this.state = state;
    for (const listener of this.#states) listener(state);
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

describe('the queue orders answers, it does not re-base gestures', () => {
  it('never lets a queued gesture start the watch the one before it stopped', async () => {
    const server = authority();
    const held = deferred<void>();
    // The mount read; the STOP, held open; then whatever the second gesture
    // turns out to be.
    server.queue(
      async () => ok(snapshot([entry(SECTION, { running: true })])),
      async () => {
        await held.promise;
        return ok(committed(snapshot([
          entry(SECTION, { policy: null, revision: 2, epoch: 2 }),
        ]), 2));
      },
      async (call) => {
        // A real authority: the compare-and-swap is what decides, and it is
        // decided HERE rather than asserted here, so a re-based gesture is
        // not merely detected but actually applied -- putting the watch the
        // user stopped back, which is the failure this test is about.
        const payload = (call.body as { payload: { basedOnRevision: number } }).payload;
        if (payload.basedOnRevision !== 1) {
          return ok(committed(snapshot([entry(SECTION, { revision: 3, epoch: 3 })]), 3));
        }
        return ok({
          contractVersion: 1,
          outcome: 'STALE_REVISION',
          replayed: false,
          authorityGeneration: 1,
          currentRevision: 2,
          maximum: null,
          committed: null,
          state: null,
        }, 409);
      },
      async () => ok(snapshot([entry(SECTION, { policy: null, revision: 2, epoch: 2 })])),
    );
    const view = desk(server.fetchImplementation);
    await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('WATCHING'));

    // Both gestures are made against rev1: the user pressed Stop, then --
    // before it came back -- applied a policy to the same Section.
    let stopped = false;
    let applied = true;
    await act(async () => {
      const stopping = view.value().setSectionIntent(SECTION, null);
      const second = view.value().setSectionIntent(SECTION, POLICY);
      held.resolve();
      [stopped, applied] = await Promise.all([stopping, second]);
    });

    expect(stopped).toBe(true);
    expect(applied).toBe(false);
    // Two writes and no third: a refusal is re-read, never replayed.
    const writes = server.writes();
    expect(writes).toHaveLength(2);
    expect(
      (writes[1]?.body as { payload: { basedOnRevision: number } }).payload.basedOnRevision,
    ).toBe(1);
    await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('NOT_WATCHING'));
    expect(view.value().intent?.entries[0]?.policy).toBeNull();
  });

  it('still submits two different sections against the state the user saw', async () => {
    const server = authority();
    server.queue(
      async () => ok(snapshot([entry(SECTION, { policy: null }), entry(OTHER, { policy: null })])),
      async () => ok(committed(snapshot([
        entry(SECTION, { revision: 2, epoch: 2 }),
        entry(OTHER, { policy: null }),
      ]), 2)),
      async () => ok(committed(snapshot([
        entry(SECTION, { revision: 2, epoch: 2 }),
        entry(OTHER, { revision: 3, epoch: 3 }),
      ]), 3)),
    );
    const view = desk(server.fetchImplementation, { selected: [SECTION, OTHER] });
    await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('NOT_WATCHING'));

    let settled: readonly boolean[] = [];
    await act(async () => {
      const first = view.value().setSectionIntent(SECTION, POLICY);
      const second = view.value().setSectionIntent(OTHER, POLICY);
      settled = await Promise.all([first, second]);
    });

    expect(settled).toEqual([true, true]);
    const writes = server.writes();
    expect(writes).toHaveLength(2);
    for (const write of writes) {
      expect(
        (write.body as { payload: { basedOnRevision: number } }).payload.basedOnRevision,
      ).toBe(1);
    }
  });
});

describe('physical proof withdraws the green light immediately', () => {
  it('stops showing a watch as running before the re-read comes back', async () => {
    const server = authority();
    const held = deferred<void>();
    server.queue(
      async () => ok(snapshot([entry(SECTION, { running: true })])),
      async () => {
        // Never resolves for the lifetime of the assertions: a read that
        // hangs must not be what stands between the user and the truth.
        await held.promise;
        return ok(snapshot([entry(SECTION)]));
      },
    );
    const view = desk(server.fetchImplementation);
    await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('WATCHING'));
    expect(view.value().active).toHaveLength(1);

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

    expect(view.value().intentStateFor(SECTION)).not.toBe('WATCHING');
    expect(view.value().active).toHaveLength(0);
    expect(view.value().isActive(SECTION)).toBe(false);
    expect(within(deskRow(SECTION)).queryByText('Watching')).toBeNull();
    // The row is still there, and the intent is still the user's.
    expect(view.value().intent?.entries[0]?.policy).not.toBeNull();
    held.resolve();
  });

  it.each([['CLOSED'], ['ERROR']] as const)(
    'stops showing a watch as running once the socket is %s',
    async (state) => {
      const server = authority();
      server.queue(async () => ok(snapshot([entry(SECTION, { running: true })])));
      const view = desk(server.fetchImplementation);
      await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('WATCHING'));

      await act(async () => { view.watch.setState(state); });

      // No further read was even asked for: the page can say this much on its
      // own, and saying it later would mean saying nothing now.
      expect(server.reads()).toHaveLength(1);
      expect(view.value().intentStateFor(SECTION)).not.toBe('WATCHING');
      expect(view.value().active).toHaveLength(0);
      expect(within(deskRow(SECTION)).queryByText('Watching')).toBeNull();
    },
  );

  it('keeps a saved row reachable across a failed read and restores it after', async () => {
    const server = authority();
    server.queue(
      async () => ok(snapshot([entry(OTHER, { running: true })])),
      async () => new Response('nope', { status: 500 }),
      async () => ok(snapshot([entry(OTHER, { running: true })])),
      async () => ok(committed(snapshot([
        entry(OTHER, { policy: null, revision: 2, epoch: 2 }),
      ]))),
    );
    // The user's selection knows nothing about OTHER. The server does.
    const view = desk(server.fetchImplementation, { selected: [] });
    await waitFor(() => expect(view.value().intentStateFor(OTHER)).toBe('WATCHING'));

    await act(async () => { await view.value().refreshIntent(); });

    // The read failed, so nothing is green and nothing is counted...
    expect(view.value().intentStatus).toBe('FAILED');
    expect(view.value().active).toHaveLength(0);
    expect(view.value().intentStateFor(OTHER)).toBeNull();
    // ...but the row is still on screen, because it is the only place this
    // watch can ever be stopped from.
    const row = deskRow(OTHER);
    expect(within(row).getByText('Watch state unreadable')).toBeTruthy();
    expect(within(row).getByText('Saved watch')).toBeTruthy();
    // And nothing on it may be submitted against a revision nobody can vouch
    // for, or quietly removed.
    expect(view.value().isRemovable(OTHER)).toBe(false);
    expect(within(row).getByRole('button', { name: 'Remove' }).hasAttribute('disabled')).toBe(true);
    expect(server.writes()).toHaveLength(0);

    // A successful read restores the real controls.
    await act(async () => { await view.value().refreshIntent(); });
    await waitFor(() => expect(view.value().intentStateFor(OTHER)).toBe('WATCHING'));
    const stop = within(deskRow(OTHER)).getByRole('button', { name: 'Stop' });
    await act(async () => { stop.click(); });
    await waitFor(() => expect(view.value().intentStateFor(OTHER)).toBe('NOT_WATCHING'));
  });
});

describe('one press of a batch button is one basis', () => {
  it('starts every selected section against the revisions the click was made on', async () => {
    const server = authority();
    // The state another tab leaves behind while the first item is in flight:
    // SECTION started, and OTHER moved on to revision 5 by somebody else.
    const moved = () => snapshot([
      entry(SECTION, { revision: 2, epoch: 2 }),
      entry(OTHER, { policy: null, revision: 5, epoch: 5 }),
    ]);
    server.queue(
      async () => ok(snapshot([
        entry(SECTION, { policy: null }),
        entry(OTHER, { policy: null }),
      ])),
      async (call) => {
        if (call.method === 'GET') return ok(moved());
        const payload = (call.body as {
          readonly payload: { readonly section: SectionKey; readonly basedOnRevision: number };
        }).payload;
        if (payload.section.index === SECTION.index) return ok(committed(moved(), 2));
        // A real authority: the compare-and-swap decides here rather than
        // being asserted here, so a re-based second item is not merely
        // detected -- it is applied, overwriting a change the user never saw.
        if (payload.basedOnRevision !== 1) return ok(committed(snapshot([
          entry(SECTION, { revision: 2, epoch: 2 }),
          entry(OTHER, { revision: 6, epoch: 6 }),
        ]), 6));
        return ok({
          contractVersion: 1,
          outcome: 'STALE_REVISION',
          replayed: false,
          authorityGeneration: 1,
          currentRevision: 5,
          maximum: null,
          committed: null,
          state: null,
        }, 409);
      },
    );
    const view = desk(server.fetchImplementation, { selected: [SECTION, OTHER] });
    await waitFor(() => expect(view.value().intentStatus).toBe('READY'));

    const start = screen.getByRole('button', { name: 'Start selected · 2' });
    await act(async () => { start.click(); });
    await waitFor(() => expect(server.writes()).toHaveLength(2));

    const second = (server.writes()[1]?.body as {
      readonly payload: { readonly basedOnRevision: number; readonly authorityGeneration: number };
    }).payload;
    expect(second.basedOnRevision).toBe(1);
    expect(second.authorityGeneration).toBe(1);
    // Refused, re-read, and never resubmitted: two writes for two Sections.
    await waitFor(() => expect(server.reads().length).toBeGreaterThan(1));
    expect(server.writes()).toHaveLength(2);
    await waitFor(() => expect(view.value().intentStateFor(OTHER)).toBe('NOT_WATCHING'));
  });

  it('applies a policy against the generation the click was made on', async () => {
    const server = authority();
    // The first item's own commit crosses the rotation threshold: generation
    // 2, every row renumbered.
    const rotated = {
      contractVersion: 1,
      authorityGeneration: 2,
      entries: [
        { ...entry(SECTION, { running: true }), revision: 1, materializationEpoch: 1 },
        { ...entry(OTHER, { running: true }), revision: 2, materializationEpoch: 2 },
      ],
    };
    server.queue(
      async () => ok(snapshot([
        entry(SECTION, { running: true }),
        entry(OTHER, { running: true, revision: 2, epoch: 2 }),
      ])),
      async (call) => {
        if (call.method === 'GET') return ok(rotated);
        const payload = (call.body as {
          readonly payload: { readonly section: SectionKey; readonly authorityGeneration: number };
        }).payload;
        if (payload.section.index === SECTION.index) {
          return ok({
            contractVersion: 1,
            outcome: 'COMMITTED',
            replayed: false,
            authorityGeneration: 2,
            currentRevision: null,
            maximum: null,
            committed: { revision: 1, materializationEpoch: 1, epochChanged: false },
            state: rotated,
          });
        }
        // Whatever generation this item presents is what the authority
        // answers: presenting the one the rotation produced is admitted.
        if (payload.authorityGeneration !== 1) {
          return ok({
            contractVersion: 1,
            outcome: 'COMMITTED',
            replayed: false,
            authorityGeneration: 2,
            currentRevision: null,
            maximum: null,
            committed: { revision: 2, materializationEpoch: 2, epochChanged: false },
            state: rotated,
          });
        }
        return ok({
          contractVersion: 1,
          outcome: 'STALE_GENERATION',
          replayed: false,
          authorityGeneration: 2,
          currentRevision: null,
          maximum: null,
          committed: null,
          state: null,
        }, 409);
      },
    );
    const view = desk(server.fetchImplementation, { selected: [SECTION, OTHER] });
    await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('WATCHING'));

    const apply = screen.getByRole('button', { name: 'Apply policy to active' });
    await act(async () => { apply.click(); });
    await waitFor(() => expect(server.writes()).toHaveLength(2));

    const generations = server.writes().map((write) => (write.body as {
      readonly payload: { readonly authorityGeneration: number };
    }).payload.authorityGeneration);
    expect(generations).toEqual([1, 1]);
    // The second item was refused and re-read. It was not rebased onto the
    // authority the first item's answer happened to carry.
    await waitFor(() => expect(server.reads().length).toBeGreaterThan(1));
    expect(server.writes()).toHaveLength(2);
  });
});

describe('a mutation whose outcome is unknown withdraws what it was about', () => {
  it('keeps a lost START addressable, ungreen and unsubmittable until a read lands', async () => {
    const server = authority();
    const held = deferred<void>();
    const running = () => snapshot([entry(SECTION, { revision: 2, epoch: 2, running: true })]);
    server.queue(
      async () => ok(snapshot([entry(SECTION, { policy: null })])),
      async (call) => {
        // The write reaches the server and is applied; only the answer is
        // lost. Everything after it is a read that never comes back.
        if (call.method === 'PUT') throw new TypeError('the response was lost');
        await held.promise;
        return ok(running());
      },
    );
    // Not in the user's selection: the desk row exists only because the page
    // has something outstanding about this Section.
    const view = desk(server.fetchImplementation, { selected: [] });
    await waitFor(() => expect(view.value().intentStatus).toBe('READY'));
    expect(document.querySelector('.watch-workspace__list')).toBeNull();

    await act(async () => {
      void view.value().setSectionIntent(SECTION, POLICY);
      await Promise.resolve();
      await Promise.resolve();
    });
    await waitFor(() => expect(server.reads()).toHaveLength(2));

    // The row is on the desk, because the watch may already be running.
    const row = deskRow(SECTION);
    expect(within(row).queryByText('Watching')).toBeNull();
    expect(view.value().active).toHaveLength(0);
    expect(view.value().isActive(SECTION)).toBe(false);
    // And nothing on it may be acted on while the answer is unknown.
    expect(view.value().isRemovable(SECTION)).toBe(false);
    act(() => { view.value().remove(SECTION); });
    let resubmitted = true;
    await act(async () => { resubmitted = await view.value().setSectionIntent(SECTION, POLICY); });
    expect(resubmitted).toBe(false);
    expect(server.writes()).toHaveLength(1);
    expect(server.reads()).toHaveLength(2);

    // The read finally lands, and the page adopts what the server really has.
    await act(async () => {
      held.resolve();
      await Promise.resolve();
    });
    await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('WATCHING'));
    expect(server.writes()).toHaveLength(1);
  });

  it('withdraws the green light on a lost STOP before the re-read comes back', async () => {
    const server = authority();
    const held = deferred<void>();
    server.queue(
      async () => ok(snapshot([entry(SECTION, { running: true })])),
      async (call) => {
        if (call.method === 'PUT') throw new TypeError('the response was lost');
        await held.promise;
        return ok(snapshot([entry(SECTION, { policy: null, revision: 2, epoch: 2 })]));
      },
    );
    const view = desk(server.fetchImplementation);
    await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('WATCHING'));

    await act(async () => {
      void view.value().setSectionIntent(SECTION, null);
      await Promise.resolve();
      await Promise.resolve();
    });
    await waitFor(() => expect(server.reads()).toHaveLength(2));

    expect(view.value().intentStateFor(SECTION)).not.toBe('WATCHING');
    expect(view.value().active).toHaveLength(0);
    expect(within(deskRow(SECTION)).queryByText('Watching')).toBeNull();
    expect(view.value().isRemovable(SECTION)).toBe(false);
    // The gesture is never sent again on the page's own initiative.
    expect(server.writes()).toHaveLength(1);

    await act(async () => {
      held.resolve();
      await Promise.resolve();
    });
    await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('NOT_WATCHING'));
    expect(server.writes()).toHaveLength(1);
  });

  it('keeps a row whose teardown is still running across a failed read', async () => {
    const server = authority();
    server.queue(
      async () => ok(snapshot([
        entry(OTHER, { policy: null, revision: 2, epoch: 2, pendingDisarm: true }),
      ])),
      async () => new Response('nope', { status: 500 }),
    );
    // The user never selected it, and the server no longer wants it -- but
    // its physical watch has not gone, so it is still the only thing that
    // can be pressed to stop it.
    const view = desk(server.fetchImplementation, { selected: [] });
    await waitFor(() => expect(view.value().intentStateFor(OTHER)).toBe('STOPPING'));
    expect(view.value().intentSaved.map((section) => section.index)).toEqual([OTHER.index]);

    await act(async () => { await view.value().refreshIntent(); });

    expect(view.value().intentStatus).toBe('FAILED');
    const row = deskRow(OTHER);
    expect(within(row).getByText('Watch state unreadable')).toBeTruthy();
    expect(view.value().isRemovable(OTHER)).toBe(false);
  });
});

describe('a closed socket is a cutoff over every answer, not a list of sections', () => {
  it.each([
    ['CLOSED', 'a snapshot that had not landed yet', 'NONE'],
    ['ERROR', 'a snapshot that had not landed yet', 'NONE'],
    ['CLOSED', 'a snapshot whose only row was still preparing', 'PREPARING'],
    ['ERROR', 'a snapshot whose only row was still preparing', 'PREPARING'],
  ] as const)(
    'a read in flight when the socket went %s cannot light up %s',
    async (state, _label, before) => {
      const server = authority();
      const held = deferred<void>();
      const running = () => ok(snapshot([entry(SECTION, { running: true })]));
      const inFlight = async () => {
        await held.promise;
        return running();
      };
      if (before === 'PREPARING') {
        server.queue(async () => ok(snapshot([entry(SECTION)])), inFlight, running);
      } else {
        server.queue(inFlight, running);
      }
      const view = desk(server.fetchImplementation);
      if (before === 'PREPARING') {
        await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('PREPARING'));
        act(() => { void view.value().refreshIntent(); });
      }
      await waitFor(() => expect(server.reads()).toHaveLength(before === 'PREPARING' ? 2 : 1));

      await act(async () => { view.watch.setState(state); });
      // The answer to the read that was already in flight arrives AFTER the
      // close, and says the server had it running.
      await act(async () => {
        held.resolve();
        await Promise.resolve();
        await Promise.resolve();
      });

      expect(view.value().intentStateFor(SECTION)).not.toBe('WATCHING');
      expect(view.value().active).toHaveLength(0);
      expect(view.value().isActive(SECTION)).toBe(false);
      expect(within(deskRow(SECTION)).queryByText('Watching')).toBeNull();

      // Only a read this page asked for after the socket came back may lift
      // it -- and then the page says what is really running again.
      await act(async () => { view.watch.setState('OPEN'); });
      await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('WATCHING'));
      expect(view.value().active).toHaveLength(1);
    },
  );
});

describe('the search entry says exactly what the desk says', () => {
  function fixtureButton(): HTMLElement {
    return within(screen.getByLabelText('Section selection fixtures')).getByRole('button');
  }

  it.each([
    [
      'a complete four-part stamp match',
      () => ok(snapshot([entry(SECTION, { running: true })])),
      'Watching',
    ],
    [
      'intent with nothing materialized',
      () => ok(snapshot([entry(SECTION)])),
      'Preparing',
    ],
    [
      'a watch running under a stamp the intent has moved past',
      () => ok(snapshot([{ ...entry(SECTION, { running: true }), revision: 5 }])),
      'Preparing',
    ],
    [
      'a watch running under a policy the user did not ask for',
      () => ok(snapshot([{
        ...entry(SECTION, { running: true }),
        policy: { ...POLICY, maxAudible: 9 },
      }])),
      'Preparing',
    ],
    [
      'a permanent failure',
      () => ok(snapshot([{
        ...entry(SECTION),
        failure: { classification: 'PERMANENT', reason: 'SECTION_NOT_FOUND', retryScheduled: false },
      }])),
      'Cannot watch · needs your decision',
    ],
    [
      'a teardown still running',
      () => ok(snapshot([entry(SECTION, { policy: null, pendingDisarm: true })])),
      'Stopping',
    ],
    [
      'a teardown still running under intent the user has restored',
      () => ok(snapshot([entry(SECTION, { pendingDisarm: true })])),
      'Preparing',
    ],
    [
      'no intent at all',
      () => ok(snapshot([])),
      'Add to watch list',
    ],
  ])('says %s', async (_label, response, expected) => {
    const server = authority();
    server.queue(async () => response());
    const view = desk(server.fetchImplementation, { selected: [], sections: [SECTION] });
    await waitFor(() => expect(view.value().intentStatus).toBe('READY'));
    expect(fixtureButton().textContent).toBe(expected);
  });

  it('says the state is unreadable rather than guessing at it', async () => {
    const server = authority();
    server.queue(async () => new Response('nope', { status: 500 }));
    const view = desk(server.fetchImplementation, { selected: [], sections: [SECTION] });
    await waitFor(() => expect(view.value().intentStatus).toBe('FAILED'));
    expect(fixtureButton().textContent).toBe('Watch state unreadable');
  });

  it('says the state is still being read rather than guessing at it', async () => {
    const server = authority();
    const held = deferred<void>();
    server.queue(async () => {
      await held.promise;
      return ok(snapshot([entry(SECTION, { running: true })]));
    });
    const view = desk(server.fetchImplementation, { selected: [], sections: [SECTION] });
    await waitFor(() => expect(fixtureButton().textContent).toBe('Reading watch state…'));
    held.resolve();
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
    [
      'an envelope carrying a field nobody agreed to',
      () => new Response(
        JSON.stringify({ protocolVersion: 1, data: committed(state), meta: {} }),
        { headers: { 'content-type': 'application/json' }, status: 200 },
      ),
    ],
    [
      'an envelope with no data at all',
      () => new Response(
        JSON.stringify({ protocolVersion: 1 }),
        { headers: { 'content-type': 'application/json' }, status: 200 },
      ),
    ],
    [
      'a commit carrying a currentRevision only a refusal has',
      () => ok({ ...committed(state), currentRevision: 1 }),
    ],
    [
      'a commit carrying a maximum only a refusal has',
      () => ok({ ...committed(state), maximum: 9 }),
    ],
    [
      'a stale revision that does not say which revision',
      () => ok({
        contractVersion: 1,
        outcome: 'STALE_REVISION',
        replayed: false,
        authorityGeneration: 1,
        currentRevision: null,
        maximum: null,
        committed: null,
        state: null,
      }, 409),
    ],
    [
      'a stale generation carrying a revision it cannot have meant',
      () => ok({
        contractVersion: 1,
        outcome: 'STALE_GENERATION',
        replayed: false,
        authorityGeneration: 2,
        currentRevision: 7,
        maximum: null,
        committed: null,
        state: null,
      }, 409),
    ],
    [
      'a capacity refusal that does not say what the cap is',
      () => ok({
        contractVersion: 1,
        outcome: 'LIMIT_EXCEEDED',
        replayed: false,
        authorityGeneration: 1,
        currentRevision: null,
        maximum: null,
        committed: null,
        state: null,
      }, 409),
    ],
    [
      'a mutation id conflict carrying a cap it has nothing to do with',
      () => ok({
        contractVersion: 1,
        outcome: 'MUTATION_ID_CONFLICT',
        replayed: false,
        authorityGeneration: 1,
        currentRevision: null,
        maximum: 9,
        committed: null,
        state: null,
      }, 409),
    ],
    [
      'a commit whose generation is not the generation of the state it returned',
      () => ok({ ...committed(state), authorityGeneration: 2 }),
    ],
    [
      'a commit whose revision no row in its own state holds',
      () => ok({
        ...committed(state),
        committed: { revision: 3, materializationEpoch: 2, epochChanged: true },
      }),
    ],
    [
      'a commit whose epoch no row in its own state holds',
      () => ok({
        ...committed(state),
        committed: { revision: 2, materializationEpoch: 3, epochChanged: true },
      }),
    ],
    [
      'a commit whose state says nothing about the Section it wrote',
      () => ok(committed(snapshot([entry(OTHER, { revision: 2, epoch: 2 })]))),
    ],
    [
      'a commit whose row is gone but which reports the numbers it used to hold',
      () => ok(committed(snapshot([entry(OTHER, { revision: 1, epoch: 1 })]), 2)),
    ],
    [
      'a commit whose row is gone and which reports half of the absent shape',
      () => ok({
        ...committed(snapshot([]), 0),
        committed: { revision: 0, materializationEpoch: 4, epochChanged: true },
      }),
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

  /**
   * A STOP whose own receipt crosses the rotation threshold writes a
   * tombstone and then rotates it away inside the same call, so the state it
   * ships genuinely has no row for the Section it just wrote. That is a
   * legitimate answer, and this is the exact body the local authority
   * produces for it -- `crates/bcsp-local-runtime/tests/desired_watch.rs`
   * pins the same shape from the other side.
   */
  it('accepts a commit whose row was legitimately collected by a rotation', async () => {
    const result = await port(() => ok({
      contractVersion: 1,
      outcome: 'COMMITTED',
      replayed: false,
      authorityGeneration: 2,
      currentRevision: null,
      maximum: null,
      committed: { revision: 0, materializationEpoch: 0, epochChanged: true },
      state: { contractVersion: 1, authorityGeneration: 2, entries: [] },
    })).submit(
      { section: SECTION, policy: null },
      snapshot([entry(SECTION)]) as never,
    );
    expect(result.outcome).toBe('COMMITTED');
    expect(result.snapshot?.generation).toBe(2);
    expect(result.snapshot?.entries).toEqual([]);
  });
});
