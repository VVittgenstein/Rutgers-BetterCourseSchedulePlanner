// @vitest-environment jsdom

import { act, cleanup, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type {
  ProductApiPort,
  ProductRuntimePort,
  SectionKey,
  TraceId,
  WatchClientCommandV1,
  WatchClientPort,
  WatchConnectionState,
  WatchCueOutcome,
  WatchPolicyV1,
  WatchRecoveryState,
  WatchServerEventV1,
  WsServerEnvelope,
} from '../src/ui/shared/product';
import {
  WatchAudioController,
  type WatchAudioContextPort,
  type WatchAudioGainPort,
  type WatchAudioOscillatorPort,
  type WatchAudioParamPort,
  type WatchAudioUnlockResult,
} from '../src/ui/shared/watch/audio';
import {
  DEFAULT_WATCH_POLICY,
  LiveWatchProvider,
  useLiveWatch,
  type LiveWatchValue,
} from '../src/ui/shared/watch/LiveWatchProvider';
import {
  evaluateWatchReadiness,
  type WatchReadinessInput,
} from '../src/ui/shared/watch/readiness';
import type {
  WatchNotificationPermission,
  WatchNotificationPort,
} from '../src/ui/shared/watch/notification';
import type { WatchIntentPort, WatchIntentSnapshot } from '../src/ui/shared/watch/intent';

const AT = '2030-01-01T00:00:00Z';
const SECTION_A: SectionKey = { term: 'T2030F', campus: 'CAMPUS_A', index: '00001' };
const ACTIVE_A = '00000000-0000-4000-8000-0000000000a1';
const EPISODE_A = '00000000-0000-4000-8000-0000000000e1';
const ALERT_A = '00000000-0000-4000-8000-0000000000f1';
const CUE_A = '00000000-0000-4000-8000-0000000000c1';

/** A chain with every ring true. Each test breaks exactly one. */
const GREEN: WatchReadinessInput = {
  wanted: 1,
  armed: 1,
  connectionOpen: true,
  recovery: 'IDLE',
  connectionCutoff: false,
  hasContact: true,
  contactFresh: true,
  intentUnreadable: false,
  audio: 'READY',
  muted: false,
  volume: 70,
  pageVisible: true,
  notificationPermission: 'default',
  notificationsEnabled: true,
};

describe('the five-ring readiness chain', () => {
  it('is green only when every ring holds', () => {
    expect(evaluateWatchReadiness(GREEN).level).toBe('READY');
  });

  it('says nothing is watched rather than claiming anything about a chain', () => {
    const state = evaluateWatchReadiness({ ...GREEN, wanted: 0, armed: 0 });
    expect(state.level).toBe('STOPPED');
    // Not DEGRADED: a user who is watching nothing is not being failed.
    expect(state.rings).toEqual({
      CONNECTION: null,
      ARMED: null,
      AUDIO: null,
      DELIVERY: null,
      SOUND: null,
    });
  });

  it.each([
    ['a closed socket', { connectionOpen: false }, 'RECONNECTING', 'CONNECTION'],
    ['a cutoff over every answer', { connectionCutoff: true }, 'RECONNECTING', 'CONNECTION'],
    ['a user Disconnect', { connectionOpen: false, recovery: 'STOPPED_BY_USER' as const }, 'DISCONNECTED', 'CONNECTION'],
    ['a heartbeat older than the bound', { contactFresh: false }, 'CONTACT_STALE', 'CONNECTION'],
    ['no heartbeat at all', { hasContact: false, contactFresh: false }, 'CONTACT_STALE', 'CONNECTION'],
    ['a Section not armed yet', { armed: 0 }, 'PREPARING', 'ARMED'],
    ['an unreadable authority', { intentUnreadable: true }, 'INTENT_UNREADABLE', 'ARMED'],
    ['audio the browser blocked', { audio: 'BLOCKED' as const }, 'AUDIO_BLOCKED', 'AUDIO'],
    ['audio that failed', { audio: 'FAILED' as const }, 'AUDIO_FAILED', 'AUDIO'],
    ['audio that was never unlocked', { audio: null }, 'AUDIO_BLOCKED', 'AUDIO'],
    ['a hidden tab with no fallback', { pageVisible: false }, 'NO_FALLBACK_WHILE_HIDDEN', 'DELIVERY'],
    ['sound the user muted', { muted: true }, 'MUTED', 'SOUND'],
    ['a volume of zero', { volume: 0 }, 'MUTED', 'SOUND'],
  ])('goes yellow for %s', (_label, overrides, reason, ring) => {
    const state = evaluateWatchReadiness({ ...GREEN, ...overrides } as WatchReadinessInput);
    expect(state.level).toBe('DEGRADED');
    expect(state.reason).toBe(reason);
    expect(state.brokenRing).toBe(ring);
  });

  it('accepts a granted browser message as the fallback a hidden tab needs', () => {
    const state = evaluateWatchReadiness({
      ...GREEN,
      pageVisible: false,
      notificationPermission: 'granted',
    });
    expect(state.level).toBe('READY');
    // And says so: "you will hear it" and "you will be told" are different
    // promises, and the surface makes which one it is visible.
    expect(state.fallbackInUse).toBe(true);
  });

  it.each<[WatchNotificationPermission, boolean]>([
    ['default', true],
    ['denied', true],
    ['unsupported', true],
    ['granted', false],
  ])('refuses %s permission (enabled=%s) as a fallback for a hidden tab', (permission, enabled) => {
    const state = evaluateWatchReadiness({
      ...GREEN,
      pageVisible: false,
      notificationPermission: permission,
      notificationsEnabled: enabled,
    });
    expect(state.level).toBe('DEGRADED');
    expect(state.reason).toBe('NO_FALLBACK_WHILE_HIDDEN');
  });

  it('reports the FIRST broken ring, in the order of the chain', () => {
    // Everything is wrong at once. A user told to unmute while the socket is
    // down would press the wrong button and learn nothing.
    const state = evaluateWatchReadiness({
      ...GREEN,
      connectionOpen: false,
      armed: 0,
      audio: 'FAILED',
      muted: true,
      pageVisible: false,
    });
    expect(state.brokenRing).toBe('CONNECTION');
  });
});

class FakeWatch implements WatchClientPort {
  state: WatchConnectionState = 'OPEN';
  lastContactAt: number | null = null;
  recovery: WatchRecoveryState = { phase: 'IDLE', attempt: 0, nextAttemptAt: null };
  readonly commands: WatchClientCommandV1[] = [];
  readonly connect = vi.fn(() => undefined);
  readonly disconnect = vi.fn(() => undefined);
  readonly #events = new Set<(envelope: WsServerEnvelope<WatchServerEventV1>) => void>();
  readonly #states = new Set<(state: WatchConnectionState) => void>();
  readonly #recoveries = new Set<(state: WatchRecoveryState) => void>();
  readonly #contacts = new Set<(at: number) => void>();
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

  subscribeRecovery(listener: (state: WatchRecoveryState) => void): () => void {
    this.#recoveries.add(listener);
    return () => this.#recoveries.delete(listener);
  }

  subscribeContact(listener: (at: number) => void): () => void {
    this.#contacts.add(listener);
    return () => this.#contacts.delete(listener);
  }

  emit(payload: WatchServerEventV1): void {
    this.#message += 1;
    this.#events.forEach((listener) => listener({
      protocolVersion: 1,
      messageId: `00000000-0000-4000-8000-${this.#message.toString(16).padStart(12, '0')}`,
      payload,
    }));
  }

  transition(state: WatchConnectionState): void {
    this.state = state;
    this.#states.forEach((listener) => listener(state));
  }

  recover(state: WatchRecoveryState): void {
    this.recovery = state;
    this.#recoveries.forEach((listener) => listener(state));
  }

  contact(at: number): void {
    this.lastContactAt = at;
    this.#contacts.forEach((listener) => listener(at));
  }
}

class FakeAudio {
  state: WatchAudioUnlockResult | null = 'READY';
  readonly unlock = vi.fn(async (): Promise<WatchAudioUnlockResult> => 'READY');
  readonly play = vi.fn((): WatchCueOutcome => this.outcome);
  readonly preview = vi.fn((): WatchCueOutcome => 'STARTED');
  readonly startContinuous = vi.fn((): WatchCueOutcome => 'STARTED');
  readonly stopContinuous = vi.fn();
  readonly dispose = vi.fn();
  readonly subscribeState = vi.fn(() => () => undefined);
  readonly heal = vi.fn(async () => this.state);
  outcome: WatchCueOutcome = 'STARTED';
}

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

const silentParam: WatchAudioParamPort = {
  setValueAtTime: () => undefined,
  linearRampToValueAtTime: () => undefined,
  exponentialRampToValueAtTime: () => undefined,
};

/**
 * A browser context for tests that need the REAL controller: the output can
 * be broken independently of the context state, and a resume can be held
 * open the way a browser that never answers holds it.
 */
class RecoverableAudioContext implements WatchAudioContextPort {
  currentTime = 0;
  destination: unknown = { output: true };
  state: AudioContextState = 'suspended';
  failOscillator = false;
  holdResume = false;
  releaseResume: (() => void) | null = null;
  readonly #listeners = new Set<() => void>();

  addEventListener(_type: 'statechange', listener: () => void): void {
    this.#listeners.add(listener);
  }

  async resume(): Promise<void> {
    if (this.holdResume) {
      await new Promise<void>((resolve) => { this.releaseResume = resolve; });
    }
    this.state = 'running';
  }

  createGain(): WatchAudioGainPort {
    return { gain: silentParam, connect: (destination) => destination, disconnect: () => undefined };
  }

  createOscillator(): WatchAudioOscillatorPort {
    if (this.failOscillator) throw new Error('output device is gone');
    return {
      type: 'sine',
      frequency: silentParam,
      onended: null,
      connect: (destination) => destination,
      disconnect: () => undefined,
      start: () => undefined,
      stop: () => undefined,
    };
  }

  suspendAndAnnounce(): void {
    this.state = 'suspended';
    this.#listeners.forEach((listener) => listener());
  }
}

function unexpected(): never {
  throw new Error('unexpected product call');
}

function Probe({ publish }: { readonly publish: (value: LiveWatchValue) => void }) {
  publish(useLiveWatch());
  return null;
}

interface Harness {
  readonly audio: FakeAudio;
  readonly notifications: FakeNotifications;
  readonly watch: FakeWatch;
  readonly clock: { now: number };
  value(): LiveWatchValue;
  rerender(): void;
}

function harness(options: {
  readonly visible?: boolean;
  readonly intent?: WatchIntentPort;
  readonly notificationsEnabled?: boolean;
  /** A REAL controller for tests about audio truth; the stub otherwise. */
  readonly audio?: WatchAudioController;
} = {}): Harness {
  const watch = new FakeWatch();
  const audio = new FakeAudio();
  const notifications = new FakeNotifications();
  const clock = { now: 1_000_000 };
  const visible = { current: options.visible ?? true };
  const product: ProductApiPort = {
    catalogDiscovery: unexpected,
    courseDetail: unexpected,
    filterSchema: unexpected,
    // Telemetry is a different surface with its own tests. It answers with
    // "never observed" here so a selected Section does not drag it in.
    openSectionStatus: vi.fn(async ({ sectionKey }) => ({
      contractVersion: 1 as const,
      sectionKey,
      state: 'UNKNOWN' as const,
      lastObservationId: null,
      catalogContentVersion: 1,
      freshness: {
        state: 'UNKNOWN' as const,
        observedAt: null,
        freshUntil: null,
        lastKnownGoodAgeSeconds: null,
        uncertainty: 'NEVER_OBSERVED' as const,
      },
      schedulerLagMilliseconds: null,
      counterSnapshot: null,
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
      freshness: {
        state: 'UNKNOWN' as const,
        observedAt: null,
        freshUntil: null,
        lastKnownGoodAgeSeconds: null,
        uncertainty: 'NEVER_OBSERVED' as const,
      },
      scheduler: null,
      counterSnapshot: null,
    })),
    searchCourses: unexpected,
    searchSections: unexpected,
    sectionDetail: unexpected,
  } as unknown as ProductApiPort;
  const runtime: ProductRuntimePort = { product, watch, dispose: vi.fn() };
  let current: LiveWatchValue | null = null;
  const tree = () => (
    <LiveWatchProvider
      audio={options.audio ?? (audio as unknown as WatchAudioController)}
      clock={() => clock.now}
      initialNotificationsEnabled={options.notificationsEnabled ?? true}
      initialSelected={[SECTION_A]}
      notifications={notifications}
      pageVisibility={() => visible.current}
      runtime={runtime}
      {...(options.intent === undefined ? {} : { intent: options.intent })}
    >
      <Probe publish={(value) => { current = value; }} />
    </LiveWatchProvider>
  );
  const view = render(tree());
  return {
    audio,
    clock,
    notifications,
    watch,
    rerender: () => view.rerender(tree()),
    value: () => {
      if (current === null) throw new Error('LiveWatch context was not published');
      return current;
    },
  };
}

async function startWatching(context: Harness): Promise<void> {
  await act(async () => {
    await context.value().startSelected(DEFAULT_WATCH_POLICY);
  });
  await act(async () => {
    context.watch.contact(context.clock.now);
    context.watch.emit({
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
        activeWatchCount: 1,
      },
    } as WatchServerEventV1);
  });
}

function openAlert(): WatchServerEventV1 {
  return {
    type: 'ALERT_UPDATED',
    alert: {
      contractVersion: 1,
      alertId: ALERT_A,
      disposition: 'OPENED',
      visible: true,
      episode: {
        contractVersion: 1,
        episodeId: EPISODE_A,
        activeWatchId: ACTIVE_A,
        sectionKey: SECTION_A,
        state: 'OPEN',
        openedAt: AT,
        acknowledgedAt: null,
        audibleCount: 1,
        maxAudible: 3,
        timedOutAt: null,
      },
    },
  } as WatchServerEventV1;
}

function requestedCue(): WatchServerEventV1 {
  return {
    type: 'AUDIO_DISPOSITION',
    audio: {
      disposition: 'CUE_REQUESTED',
      cue: {
        cueId: CUE_A,
        activeWatchId: ACTIVE_A,
        sectionKey: SECTION_A,
        trigger: { kind: 'ONE_SHOT_OBSERVATION', observationId: '00000000-0000-4000-8000-0000000000b1' },
        emittedAt: AT,
      },
    },
  } as WatchServerEventV1;
}

afterEach(cleanup);

describe('readiness in the running page', () => {
  it('expires a heartbeat with no further event of any kind', async () => {
    const context = harness();
    await startWatching(context);
    expect(context.value().readiness.level).toBe('READY');

    // Nothing happens. No frame, no click, no network event -- only time.
    // A page that derived readiness from the socket state alone stays green
    // over a server that has stopped answering.
    await act(async () => {
      context.clock.now += 25_001;
      context.rerender();
    });
    expect(context.value().readiness.reason).toBe('CONTACT_STALE');
    expect(context.value().readiness.level).toBe('DEGRADED');

    await act(async () => {
      context.clock.now += 1_000;
      context.watch.contact(context.clock.now);
    });
    expect(context.value().readiness.level).toBe('READY');
  });

  it('drops the green light when the socket closes, before any read answers', async () => {
    const context = harness();
    await startWatching(context);
    await act(async () => {
      context.watch.transition('CLOSED');
      context.watch.recover({ phase: 'WAITING', attempt: 1, nextAttemptAt: context.clock.now + 1_000 });
    });
    expect(context.value().readiness.level).toBe('DEGRADED');
    expect(context.value().readiness.reason).toBe('RECONNECTING');
  });

  it('tells a disconnected user that it is their decision, and offers the way back', async () => {
    const context = harness();
    await startWatching(context);
    await act(async () => {
      context.value().disconnect();
      context.watch.recover({ phase: 'STOPPED_BY_USER', attempt: 0, nextAttemptAt: null });
      context.watch.transition('CLOSED');
    });
    expect(context.value().readiness.reason).toBe('DISCONNECTED');
    expect(context.value().readiness.action).toBe('RECONNECT');

    await act(async () => context.value().reconnect());
    expect(context.watch.connect).toHaveBeenCalled();
  });

  it('does not go green for a watch armed under a policy the user has since changed', async () => {
    const context = harness();
    await startWatching(context);
    expect(context.value().readiness.level).toBe('READY');

    await act(async () => {
      const [watch] = context.value().active;
      if (watch === undefined) throw new Error('expected an active watch');
      context.value().updatePolicy(watch, { ...DEFAULT_WATCH_POLICY, maxAudible: 9 });
    });
    // The plan now says maxAudible 9 and the running watch says 3. This
    // protocol has no answer to an UPDATE_POLICY, so the page cannot claim
    // the watch is running the way the user asked for -- and a page that
    // compared its own request against its own echo would always agree with
    // itself.
    expect(context.value().readiness.level).toBe('DEGRADED');
    expect(context.value().readiness.reason).toBe('PREPARING');
  });
});

describe('page-level notifications', () => {
  it('asks for permission inside the Start gesture, before anything is awaited', async () => {
    const context = harness();
    await act(async () => {
      const started = context.value().startSelected(DEFAULT_WATCH_POLICY);
      // The ask has already happened: it is the first statement of the
      // gesture, not something scheduled after an await. A browser only
      // honours it while the activation is still live.
      expect(context.notifications.asked).toHaveLength(1);
      await started;
    });
    expect(context.notifications.asked).toHaveLength(1);
  });

  it('does not ask again once the browser has answered', async () => {
    const context = harness();
    context.notifications.permission = 'denied';
    await act(async () => { await context.value().startSelected(DEFAULT_WATCH_POLICY); });
    await act(async () => { await context.value().startSelected(DEFAULT_WATCH_POLICY); });
    expect(context.notifications.permission).toBe('denied');
  });

  it('never asks when the user turned the setting off', async () => {
    const context = harness({ notificationsEnabled: false });
    await act(async () => { await context.value().startSelected(DEFAULT_WATCH_POLICY); });
    expect(context.notifications.asked).toHaveLength(0);
  });

  it('posts one message when a Section opens on a hidden page', async () => {
    const context = harness({ visible: false });
    await startWatching(context);
    await act(async () => context.watch.emit(openAlert()));

    expect(context.value().notifications).toHaveLength(1);
    expect(context.value().notifications[0]?.kind).toBe('SECTION_OPEN');
    expect(context.value().notifications[0]?.subject).toBe(`episode:${EPISODE_A}`);
  });

  it('posts nothing when the page is visible and its sound works', async () => {
    const context = harness();
    await startWatching(context);
    await act(async () => context.watch.emit(openAlert()));
    expect(context.value().notifications).toEqual([]);
  });

  it('posts after the fact when the alert looked fine and its sound then failed', async () => {
    const context = harness();
    await startWatching(context);
    // Visible, audio READY: nothing is posted for the alert itself.
    await act(async () => context.watch.emit(openAlert()));
    expect(context.value().notifications).toEqual([]);

    context.audio.outcome = 'AUTOPLAY_BLOCKED';
    await act(async () => context.watch.emit(requestedCue()));

    const posted = context.value().notifications;
    expect(posted).toHaveLength(1);
    expect(posted[0]?.kind).toBe('CUE_FAILED');
  });

  it('does not post twice about one Section that opened and then failed to ring', async () => {
    const context = harness({ visible: false });
    await startWatching(context);
    await act(async () => context.watch.emit(openAlert()));
    context.audio.outcome = 'FAILED';
    await act(async () => context.watch.emit(requestedCue()));

    // One thing happened to the user -- a Section they watch opened and they
    // could not hear it -- so they are told once.
    expect(context.value().notifications).toHaveLength(1);
    expect(context.value().notifications[0]?.kind).toBe('SECTION_OPEN');
  });

  it('queues nothing while permission has not been granted', async () => {
    const context = harness({ visible: false });
    context.notifications.answer = 'denied';
    await startWatching(context);
    await act(async () => context.watch.emit(openAlert()));
    expect(context.value().notifications).toEqual([]);
    // And the readiness surface says so rather than quietly dropping it.
    expect(context.value().readiness.reason).toBe('NO_FALLBACK_WHILE_HIDDEN');
  });

  it('reports one outage after two minutes of failed recovery, not one per attempt', async () => {
    const context = harness();
    await startWatching(context);
    await act(async () => {
      context.watch.transition('CLOSED');
      context.watch.recover({ phase: 'WAITING', attempt: 1, nextAttemptAt: context.clock.now + 1_000 });
    });
    expect(context.value().notifications).toEqual([]);

    for (const attempt of [2, 3, 4, 5, 6, 7]) {
      await act(async () => {
        context.clock.now += 30_000;
        context.watch.recover({
          phase: 'WAITING',
          attempt,
          nextAttemptAt: context.clock.now + 30_000,
        });
      });
    }

    const outages = context.value().notifications.filter((request) =>
      request.kind === 'MONITORING_DEGRADED');
    expect(outages).toHaveLength(1);
  });

  it('drops what was queued when the user turns notifications off', async () => {
    const context = harness({ visible: false });
    await startWatching(context);
    await act(async () => context.watch.emit(openAlert()));
    expect(context.value().notifications).toHaveLength(1);

    await act(async () => context.value().setNotificationsEnabled(false));
    // The user has just said they do not want to be told. Posting a backlog
    // after that is the surface ignoring an instruction it was given.
    expect(context.value().notifications).toEqual([]);
  });
});

describe('readiness with a durable authority', () => {
  const snapshot = (options: {
    readonly policy: WatchPolicyV1 | null;
    readonly running: boolean;
  }): WatchIntentSnapshot => ({
    generation: 4,
    entries: [{
      section: SECTION_A,
      policy: options.policy,
      revision: 7,
      epoch: 2,
      running: options.running
        ? { generation: 4, revision: 7, epoch: 2, policy: DEFAULT_WATCH_POLICY, activeWatchId: ACTIVE_A }
        : null,
      stopping: false,
      waitingForSlot: false,
      problem: null,
    }],
  });

  function intentPort(read: () => Promise<WatchIntentSnapshot>): WatchIntentPort {
    return {
      read,
      submit: async () => ({ outcome: 'COMMITTED', snapshot: await read(), maximum: null }),
    };
  }

  it('is green only while the whole stamp matches, and never over a cutoff', async () => {
    let armed = true;
    const context = harness({
      intent: intentPort(async () => snapshot({ policy: DEFAULT_WATCH_POLICY, running: armed })),
    });
    await act(async () => {
      context.watch.contact(context.clock.now);
      await Promise.resolve();
    });
    await act(async () => { await context.value().refreshIntent(); });
    // The server is watching, but nothing on this page can make a sound yet:
    // audio needs a gesture, and a restored page has not had one. That is a
    // real broken ring, not a technicality -- the page would not ring.
    expect(context.value().readiness.reason).toBe('AUDIO_BLOCKED');

    await act(async () => { await context.value().enableSound(); });
    expect(context.value().readiness.level).toBe('READY');

    // The socket goes. The authority still says a watch is materialized, but
    // this page can no longer assert that it would hear it.
    await act(async () => {
      context.watch.transition('CLOSED');
      context.watch.recover({ phase: 'WAITING', attempt: 1, nextAttemptAt: context.clock.now + 1_000 });
    });
    expect(context.value().readiness.level).toBe('DEGRADED');

    // Reopening is not evidence either -- only a read taken after it is.
    await act(async () => {
      context.watch.transition('OPEN');
      context.watch.contact(context.clock.now);
      context.watch.recover({ phase: 'IDLE', attempt: 0, nextAttemptAt: null });
    });
    await act(async () => { await context.value().refreshIntent(); });
    expect(context.value().readiness.level).toBe('READY');

    armed = false;
    await act(async () => { await context.value().refreshIntent(); });
    expect(context.value().readiness.reason).toBe('PREPARING');
  });

  it('sends no legacy lifecycle command when the connection comes back', async () => {
    const context = harness({
      intent: intentPort(async () => snapshot({ policy: DEFAULT_WATCH_POLICY, running: true })),
    });
    await act(async () => { await context.value().refreshIntent(); });
    await act(async () => {
      context.watch.transition('CLOSED');
      context.watch.transition('OPEN');
      context.watch.contact(context.clock.now);
    });
    await act(async () => { await Promise.resolve(); });

    // The server owns what is watched here. A page that re-armed would be
    // asserting a second answer to a question the authority has answered.
    expect(context.watch.commands.filter((command) =>
      command.type === 'START_WATCH'
      || command.type === 'STOP_WATCH'
      || command.type === 'UPDATE_POLICY')).toEqual([]);
  });
});

describe('what an independent review found, pinned', () => {
  it('arms a Section once when the Start press is what opened the socket', async () => {
    const context = harness();
    context.watch.state = 'IDLE';
    await act(async () => { await context.value().startSelected(DEFAULT_WATCH_POLICY); });
    await act(async () => {
      context.watch.transition('OPEN');
      context.watch.contact(context.clock.now);
    });

    // One press, one watch. The press opens the connection AND queues its own
    // frame; a re-arm that did not notice would arm every Section twice, and
    // the user would be told about the same opening twice.
    const starts = context.watch.commands.filter((command) => command.type === 'START_WATCH');
    expect(starts).toHaveLength(1);
    expect(starts[0]).toEqual({
      type: 'START_WATCH',
      items: [{ sectionKey: SECTION_A, policy: DEFAULT_WATCH_POLICY }],
    });
  });

  it('recovers from a connection that dropped and comes back, without latching yellow', async () => {
    const context = harness();
    await startWatching(context);
    expect(context.value().readiness.level).toBe('READY');

    await act(async () => {
      context.watch.transition('CLOSED');
      context.watch.recover({ phase: 'WAITING', attempt: 1, nextAttemptAt: context.clock.now + 1_000 });
    });
    expect(context.value().readiness.level).toBe('DEGRADED');

    await act(async () => {
      context.watch.transition('OPEN');
      context.watch.contact(context.clock.now);
      context.watch.recover({ phase: 'IDLE', attempt: 0, nextAttemptAt: null });
    });
    await act(async () => {
      context.watch.emit({
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
          activeWatchCount: 1,
        },
      } as WatchServerEventV1);
    });

    // Everything is true again: open, armed, audible, visible, unmuted. A
    // page still reporting "reconnecting" here would be a permanent red over
    // a connection that is fine, which is its own kind of lie.
    expect(context.value().readiness.level).toBe('READY');
  });

  it('does not promise anything while a submission it cannot see the answer to is outstanding', async () => {
    // A durable-intent page: Section A is watched and green. The user starts
    // a SECOND Section and the answer never comes back.
    const other: SectionKey = { term: 'T2030F', campus: 'CAMPUS_A', index: '00002' };
    const snapshot: WatchIntentSnapshot = {
      generation: 4,
      entries: [{
        section: SECTION_A,
        policy: DEFAULT_WATCH_POLICY,
        revision: 7,
        epoch: 2,
        running: {
          generation: 4,
          revision: 7,
          epoch: 2,
          policy: DEFAULT_WATCH_POLICY,
          activeWatchId: ACTIVE_A,
        },
        stopping: false,
        waitingForSlot: false,
        problem: null,
      }],
    };
    let releaseSubmit: (() => void) | null = null;
    const context = harness({
      intent: {
        read: async () => snapshot,
        submit: async () => {
          await new Promise<void>((resolve) => { releaseSubmit = resolve; });
          throw new Error('never answered');
        },
      },
    });
    await act(async () => {
      context.watch.contact(context.clock.now);
      await context.value().refreshIntent();
    });
    await act(async () => { await context.value().enableSound(); });
    expect(context.value().readiness.level).toBe('READY');

    await act(async () => {
      void context.value().setSectionIntent(other, DEFAULT_WATCH_POLICY);
      await Promise.resolve();
    });

    // The snapshot has never heard of this Section, so nothing about it can
    // be read off the projection -- and that is exactly why the page must not
    // keep saying it will ring. The server may have armed it, or may not.
    expect(context.value().readiness.level).toBe('DEGRADED');
    releaseSubmit?.();
  });

  it('says it cannot read the authority instead of saying nothing is watched', async () => {
    const snapshot: WatchIntentSnapshot = {
      generation: 4,
      entries: [{
        section: SECTION_A,
        policy: DEFAULT_WATCH_POLICY,
        revision: 7,
        epoch: 2,
        running: {
          generation: 4,
          revision: 7,
          epoch: 2,
          policy: DEFAULT_WATCH_POLICY,
          activeWatchId: ACTIVE_A,
        },
        stopping: false,
        waitingForSlot: false,
        problem: null,
      }],
    };
    let fail = false;
    const context = harness({
      intent: {
        read: async () => {
          if (fail) throw new Error('read failed');
          return snapshot;
        },
        submit: async () => ({ outcome: 'COMMITTED', snapshot, maximum: null }),
      },
    });
    await act(async () => {
      context.watch.contact(context.clock.now);
      await context.value().refreshIntent();
    });
    await act(async () => { await context.value().enableSound(); });
    expect(context.value().readiness.level).toBe('READY');

    fail = true;
    await act(async () => { await context.value().refreshIntent(); });

    // "Nothing is being watched" would be false and would take the readiness
    // line off the screen at the one moment it is most needed. The desk keeps
    // the row and says it is unreadable; this says the same thing.
    expect(context.value().readiness.level).toBe('DEGRADED');
    expect(context.value().readiness.reason).toBe('INTENT_UNREADABLE');
  });

  it('posts one message for an opening and its failed sound delivered in one burst', async () => {
    const context = harness({ visible: false });
    await startWatching(context);
    context.audio.outcome = 'FAILED';

    // Both frames in ONE turn: the server writes the alert and the cue into a
    // single dispatch, so the browser hands them to the page back to back
    // with no render in between.
    await act(async () => {
      context.watch.emit(openAlert());
      context.watch.emit(requestedCue());
    });

    expect(context.value().notifications).toHaveLength(1);
  });

  it('keeps a device failure reported after the tab comes back', async () => {
    const context = harness();
    await startWatching(context);
    context.audio.state = 'FAILED';
    context.audio.outcome = 'FAILED';
    await act(async () => context.watch.emit(requestedCue()));
    expect(context.value().readiness.reason).toBe('AUDIO_FAILED');

    // Returning from hidden re-reads the browser's audio state. A context
    // that never stopped running says nothing about an output that did.
    await act(async () => {
      document.dispatchEvent(new Event('visibilitychange'));
      await Promise.resolve();
    });
    expect(context.value().readiness.reason).toBe('AUDIO_FAILED');
  });

  it('does not let the sound recovery relight an output that is still failing', async () => {
    const browserAudio = new RecoverableAudioContext();
    const controller = new WatchAudioController({ createContext: () => browserAudio });
    const context = harness({ audio: controller });
    await startWatching(context);
    expect(context.value().readiness.level).toBe('READY');

    // A real voice fails on a running context: the device went away.
    browserAudio.failOscillator = true;
    await act(async () => context.watch.emit(requestedCue()));
    expect(context.value().readiness.reason).toBe('AUDIO_FAILED');

    // The user presses the recovery control while the output is still gone.
    // The context still reports `running`; the page must not go green on the
    // strength of a button press that proved nothing.
    await act(async () => { await context.value().enableSound(); });
    expect(context.value().audioState).toBe('FAILED');
    expect(context.value().readiness.level).toBe('DEGRADED');
    expect(context.value().readiness.reason).toBe('AUDIO_FAILED');

    // The device comes back. The same press now retries the output for
    // real, and only that success is allowed to restore the green light.
    browserAudio.failOscillator = false;
    await act(async () => { await context.value().enableSound(); });
    expect(context.value().audioState).toBe('READY');
    expect(context.value().readiness.level).toBe('READY');
  });

  it('degrades the standing readiness while a held resume is still pending', async () => {
    const browserAudio = new RecoverableAudioContext();
    const controller = new WatchAudioController({ createContext: () => browserAudio });
    const context = harness({ audio: controller });
    await startWatching(context);
    expect(context.value().readiness.level).toBe('READY');

    // The system suspends the context; the repair's resume never settles.
    // The standing readiness must drop NOW, not when the browser gets
    // around to answering -- it may never.
    browserAudio.holdResume = true;
    await act(async () => {
      browserAudio.suspendAndAnnounce();
      await Promise.resolve();
    });
    expect(context.value().audioState).toBe('BLOCKED');
    expect(context.value().readiness.level).toBe('DEGRADED');
    expect(context.value().readiness.reason).toBe('AUDIO_BLOCKED');

    // The browser finally answers; the light may come back on. The release
    // travels through two awaits (the held promise, then the resume call),
    // so the flush yields once per link in that chain.
    await act(async () => {
      browserAudio.releaseResume?.();
      await Promise.resolve();
      await Promise.resolve();
      await Promise.resolve();
    });
    expect(context.value().audioState).toBe('READY');
    expect(context.value().readiness.level).toBe('READY');
  });

  it('still reports an outage to a user who grants permission during it', async () => {
    const context = harness();
    // The user was asked at Start and did not answer the browser prompt.
    context.notifications.answer = 'default';
    await startWatching(context);
    expect(context.notifications.permission).toBe('default');
    await act(async () => {
      context.watch.transition('CLOSED');
      context.watch.recover({ phase: 'WAITING', attempt: 1, nextAttemptAt: context.clock.now + 1_000 });
    });

    // Two minutes pass with no permission: nothing is posted, and nothing is
    // spent either.
    await act(async () => {
      context.clock.now += 150_000;
      context.watch.recover({ phase: 'WAITING', attempt: 2, nextAttemptAt: context.clock.now + 2_000 });
    });
    expect(context.value().notifications).toEqual([]);

    // The user grants it. The outage they are still in is reportable.
    await act(async () => {
      context.notifications.permission = 'granted';
      context.value().requestNotificationPermission();
      await Promise.resolve();
    });
    await act(async () => {
      context.clock.now += 30_000;
      context.watch.recover({ phase: 'WAITING', attempt: 3, nextAttemptAt: context.clock.now + 4_000 });
    });
    expect(context.value().notifications.filter((request) =>
      request.kind === 'MONITORING_DEGRADED')).toHaveLength(1);
  });
});
