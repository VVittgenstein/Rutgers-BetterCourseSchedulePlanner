// @vitest-environment jsdom

import { act, cleanup, render, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
  ProductApiPort,
  ProductRuntimePort,
  SectionKey,
  TraceId,
  WatchClientCommandV1,
  WatchClientPort,
  WatchConnectionState,
  WatchRecoveryState,
  WatchPolicyV1,
  WatchServerEventV1,
  WsServerEnvelope,
} from '../src/ui/shared/product';
import {
  LiveWatchProvider,
  useLiveWatch,
  type LiveWatchValue,
} from '../src/ui/shared/watch/LiveWatchProvider';
import { createLocalDesiredWatchApi } from '../src/ui/local/desired';
import { WatchAudioController } from '../src/ui/shared/watch/audio';

const SECTION: SectionKey = { term: 'T2030F', campus: 'CAMPUS_A', index: '00001' };
const OTHER: SectionKey = { term: 'T2030F', campus: 'CAMPUS_A', index: '00002' };
const POLICY: WatchPolicyV1 = {
  notificationMode: 'ONE_SHOT',
  maxAudible: 3,
  continuousDuration: { kind: 'FINITE', seconds: 600 },
};
const LOUD: WatchPolicyV1 = {
  notificationMode: 'CONTINUOUS',
  maxAudible: 7,
  continuousDuration: { kind: 'UNLIMITED' },
};
const UUID_V4 =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;

interface EntryOptions {
  readonly policy?: WatchPolicyV1 | null;
  readonly revision?: number;
  readonly epoch?: number;
  readonly running?: { readonly generation?: number; readonly revision?: number; readonly epoch?: number; readonly policy?: WatchPolicyV1 } | null;
  readonly failure?: { readonly classification: 'PERMANENT' | 'TRANSIENT'; readonly reason: string; readonly retryScheduled: boolean } | null;
  readonly pendingDisarm?: boolean;
  readonly blockedOnSlot?: boolean;
}

function entry(section: SectionKey, options: EntryOptions = {}) {
  const revision = options.revision ?? 1;
  const epoch = options.epoch ?? 1;
  const policy = options.policy === undefined ? POLICY : options.policy;
  const running = options.running === undefined || options.running === null
    ? null
    : {
      authorityGeneration: options.running.generation ?? 1,
      revision: options.running.revision ?? revision,
      materializationEpoch: options.running.epoch ?? epoch,
      policy: options.running.policy ?? policy ?? POLICY,
      activeWatchId: '00000000-0000-4000-8000-0000000000a1',
    };
  return {
    section,
    policy,
    revision,
    materializationEpoch: epoch,
    materialized: running,
    pendingDisarm: options.pendingDisarm ?? false,
    blockedOnSlot: options.blockedOnSlot ?? false,
    failure: options.failure ?? null,
  };
}

function state(entries: readonly unknown[], generation = 1) {
  return { contractVersion: 1, authorityGeneration: generation, entries };
}

function ok(data: unknown, status = 200): Response {
  return new Response(JSON.stringify({ protocolVersion: 1, data }), {
    headers: { 'content-type': 'application/json' },
    status,
  });
}

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
  readonly #events = new Set<(envelope: WsServerEnvelope<WatchServerEventV1>) => void>();
  readonly #states = new Set<(state: WatchConnectionState) => void>();
  #message = 0;

  send(command: WatchClientCommandV1): TraceId {
    this.commands.push(command);
    this.#message += 1;
    return `00000000-0000-4000-8000-${this.#message.toString(16).padStart(12, '0')}`;
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
  dayTimezone: 'America/New_York',
};

/** Telemetry is not what these tests are about; it just has to answer. */
function openStatus(batch: { readonly term: string; readonly campus: string }) {
  return {
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
  };
}

function openSectionStatus(sectionKey: SectionKey) {
  return {
    contractVersion: 1 as const,
    sectionKey,
    state: 'UNKNOWN' as const,
    lastObservationId: null,
    catalogContentVersion: 1,
    freshness: FRESHNESS,
    schedulerLagMilliseconds: 0,
    counterSnapshot: SNAPSHOT,
  };
}

interface Harness {
  readonly requests: { method: string; body: unknown }[];
  readonly watch: FakeWatch;
  value(): LiveWatchValue;
}

function Probe({ publish }: { readonly publish: (value: LiveWatchValue) => void }) {
  publish(useLiveWatch());
  return null;
}

function harness(responses: readonly (() => Response)[], selected: readonly SectionKey[] = [SECTION]): Harness {
  const requests: { method: string; body: unknown }[] = [];
  const queue = [...responses];
  const fetchImplementation = vi.fn(async (_input: RequestInfo | URL, init?: RequestInit) => {
    requests.push({
      method: init?.method ?? 'GET',
      body: typeof init?.body === 'string' ? JSON.parse(init.body) as unknown : null,
    });
    const next = queue.length > 1 ? queue.shift() : queue[0];
    if (next === undefined) throw new Error('no response queued');
    return next();
  }) as unknown as typeof fetch;
  const intent = createLocalDesiredWatchApi({
    fetch: fetchImplementation,
    session: () => '00000000-0000-4000-8000-00000000f00d',
  });
  const watch = new FakeWatch();
  const product: ProductApiPort = {
    catalogDiscovery: unexpected,
    courseDetail: unexpected,
    filterSchema: unexpected,
    openSectionStatus: vi.fn(async ({ sectionKey }) => openSectionStatus(sectionKey)),
    openStatus: vi.fn(async ({ batch }) => openStatus(batch)),
    searchCourses: unexpected,
    searchSections: unexpected,
    sectionDetail: unexpected,
  };
  const runtime: ProductRuntimePort = { product, watch, dispose: vi.fn() };
  let current: LiveWatchValue | null = null;
  render(
    <LiveWatchProvider
      audio={new (class {
        unlock = vi.fn(async () => 'READY' as const);
        play = vi.fn(() => 'STARTED' as const);
        preview = vi.fn(() => 'STARTED' as const);
        startContinuous = vi.fn(() => 'STARTED' as const);
        stopContinuous = vi.fn();
        dispose = vi.fn();
        subscribeState = vi.fn(() => () => undefined);
        heal = vi.fn(async () => null);
      })() as unknown as WatchAudioController}
      initialSelected={selected}
      intent={intent}
      runtime={runtime}
    >
      <Probe publish={(value) => { current = value; }} />
    </LiveWatchProvider>,
  );
  return {
    requests,
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

describe('local desired-watch intent', () => {
  it('shows a section as watched only when the whole server stamp matches', async () => {
    const view = harness([() => ok(state([
      entry(SECTION, { running: {} }),
      entry(OTHER, { revision: 2, epoch: 2, running: { revision: 1 } }),
    ]))]);
    await waitFor(() => expect(view.value().intentStatus).toBe('READY'));

    expect(view.value().intentStateFor(SECTION)).toBe('WATCHING');
    // Same section, same policy, a watch really running -- but armed under a
    // revision the intent has since moved past. The user changed something
    // and the server has not caught up, so this is preparing, not watching.
    expect(view.value().intentStateFor(OTHER)).toBe('PREPARING');
  });

  it.each([
    ['a stale generation', { running: { generation: 2 } }],
    ['a stale epoch', { running: { epoch: 9 } }],
    ['a different policy', { running: { policy: LOUD } }],
    ['nothing running', { running: null }],
  ])('never reports watched with %s', async (_label, options) => {
    const view = harness([() => ok(state([entry(SECTION, options as EntryOptions)]))]);
    await waitFor(() => expect(view.value().intentStatus).toBe('READY'));
    expect(view.value().intentStateFor(SECTION)).not.toBe('WATCHING');
  });

  it('reports stopping and needs-attention as their own states', async () => {
    const view = harness([() => ok(state([
      entry(SECTION, { policy: null, revision: 3, epoch: 3, pendingDisarm: true }),
      entry(OTHER, {
        failure: { classification: 'PERMANENT', reason: 'UNSUPPORTED_TARGET', retryScheduled: false },
      }),
    ]))]);
    await waitFor(() => expect(view.value().intentStatus).toBe('READY'));

    expect(view.value().intentStateFor(SECTION)).toBe('STOPPING');
    expect(view.value().intentStateFor(OTHER)).toBe('ATTENTION');
    // A section the server cannot watch KEEPS its intent. The row is still
    // there and still a watch, so the user is the one who decides to stop it.
    const kept = view.value().intent?.entries.find((value) => value.section.index === OTHER.index);
    expect(kept?.policy).not.toBeNull();
    expect(kept?.problem?.permanent).toBe(true);
  });

  it('shows a failed read as failed rather than keeping the last green answer', async () => {
    let attempt = 0;
    const view = harness([() => {
      attempt += 1;
      return attempt === 1
        ? ok(state([entry(SECTION, { running: {} })]))
        : new Response('nope', { status: 500 });
    }]);
    await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('WATCHING'));

    await act(async () => { await view.value().refreshIntent(); });
    expect(view.value().intentStatus).toBe('FAILED');
    expect(view.value().intent).toBeNull();
    expect(view.value().intentStateFor(SECTION)).toBeNull();
  });
});

describe('local desired-watch submissions', () => {
  it('starts by writing intent, with a fresh id and the revision it read', async () => {
    const view = harness([
      () => ok(state([entry(SECTION, { policy: null, revision: 4 })])),
      () => ok({
        contractVersion: 1,
        outcome: 'COMMITTED',
        replayed: false,
        authorityGeneration: 1,
        currentRevision: null,
        maximum: null,
        committed: { revision: 5, materializationEpoch: 5, epochChanged: true },
        state: state([entry(SECTION, { revision: 5, epoch: 5, running: { revision: 5, epoch: 5 } })]),
      }),
    ]);
    await waitFor(() => expect(view.value().intentStatus).toBe('READY'));

    await act(async () => { await view.value().startSelected(POLICY); });

    // The socket carried nothing. Locally the durable row is the only answer
    // to "is this watched"; a START frame would be a second one.
    expect(view.watch.commands).toEqual([]);
    const put = view.requests.find((request) => request.method === 'PUT');
    expect(put).toBeDefined();
    const payload = (put?.body as { payload: Record<string, unknown> }).payload;
    expect(payload.section).toEqual(SECTION);
    expect(payload.policy).toEqual(POLICY);
    expect(payload.basedOnRevision).toBe(4);
    expect(payload.authorityGeneration).toBe(1);
    expect(String(payload.mutationId)).toMatch(UUID_V4);
    expect(view.value().intentStateFor(SECTION)).toBe('WATCHING');
  });

  it('stops by writing a removal, and can stop a section that never armed', async () => {
    const view = harness([
      () => ok(state([entry(SECTION, {
        revision: 7,
        running: null,
        failure: { classification: 'PERMANENT', reason: 'SECTION_NOT_FOUND', retryScheduled: false },
      })])),
      () => ok({
        contractVersion: 1,
        outcome: 'COMMITTED',
        replayed: false,
        authorityGeneration: 1,
        currentRevision: null,
        maximum: null,
        committed: { revision: 8, materializationEpoch: 8, epochChanged: true },
        state: state([entry(SECTION, { policy: null, revision: 8, epoch: 8, running: null })]),
      }),
    ]);
    await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('ATTENTION'));

    await act(async () => { await view.value().setSectionIntent(SECTION, null); });

    const put = view.requests.find((request) => request.method === 'PUT');
    const payload = (put?.body as { payload: Record<string, unknown> }).payload;
    expect(payload.policy).toBeNull();
    expect(payload.basedOnRevision).toBe(7);
    expect(view.value().intentStateFor(SECTION)).toBe('NOT_WATCHING');
    expect(view.watch.commands).toEqual([]);
  });

  it('re-reads after a conflict instead of replaying the gesture', async () => {
    const bodies: (() => Response)[] = [
      () => ok(state([entry(SECTION, { policy: null, revision: 2 })])),
      () => ok({
        contractVersion: 1,
        outcome: 'STALE_REVISION',
        replayed: false,
        authorityGeneration: 1,
        currentRevision: 9,
        maximum: null,
        committed: null,
        state: null,
      }, 409),
      () => ok(state([entry(SECTION, { revision: 9, epoch: 9, running: { revision: 9, epoch: 9 } })])),
    ];
    const view = harness(bodies);
    await waitFor(() => expect(view.value().intentStatus).toBe('READY'));

    await act(async () => { await view.value().setSectionIntent(SECTION, POLICY); });

    const writes = view.requests.filter((request) => request.method === 'PUT');
    expect(writes).toHaveLength(1);
    // Exactly one write, then a fresh read. Re-sending the same gesture
    // against the revision the refusal reported would apply a decision the
    // user made about a state that no longer exists.
    expect(view.requests.at(-1)?.method).toBe('GET');
    await waitFor(() => expect(view.value().intentStateFor(SECTION)).toBe('WATCHING'));
    expect(view.watch.commands).toEqual([]);
  });

  it('reports the product cap without writing anything else', async () => {
    const view = harness([
      () => ok(state([entry(SECTION, { policy: null, revision: 1 })])),
      () => ok({
        contractVersion: 1,
        outcome: 'LIMIT_EXCEEDED',
        replayed: false,
        authorityGeneration: 1,
        currentRevision: null,
        maximum: 9,
        committed: null,
        state: null,
      }, 409),
      () => ok(state([entry(SECTION, { policy: null, revision: 1 })])),
    ]);
    await waitFor(() => expect(view.value().intentStatus).toBe('READY'));

    await act(async () => { await view.value().setSectionIntent(SECTION, POLICY); });

    expect(view.requests.filter((request) => request.method === 'PUT')).toHaveLength(1);
    expect(view.value().notices.some((notice) => notice.code === 'SELECTION_LIMIT')).toBe(true);
    expect(view.value().intentStateFor(SECTION)).toBe('NOT_WATCHING');
  });

  it('attaches the socket so the server has an audience to materialize for', async () => {
    const view = harness([() => ok(state([entry(SECTION, { running: null })]))]);
    view.watch.state = 'CLOSED';
    await waitFor(() => expect(view.value().intentStatus).toBe('READY'));
    await waitFor(() => expect(view.watch.connect).toHaveBeenCalled());
  });
});
