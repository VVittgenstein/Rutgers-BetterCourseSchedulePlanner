import type {
  TraceId,
  WsClientEnvelope,
  WsServerEnvelope,
} from './contracts/common';
import type { WatchClientCommandV1, WatchServerEventV1 } from './contracts/watch';
import type { ProductSessionSource } from './ProductClient';

export const WATCH_SUBPROTOCOL = 'bcsp.v1';
export const WATCH_PATH = '/api/v1/watch';
const MAX_FRAME_BYTES = 64 * 1024;
const MAX_REMEMBERED_SERVER_EVENTS = 2_048;
const SERVER_EVENT_TYPES = new Set([
  'START_RESULT',
  'WATCH_STOPPED',
  'OPEN_OBSERVATION',
  'EPISODE_UPDATED',
  'ALERT_UPDATED',
  'AUDIO_DISPOSITION',
  'CUE_OUTCOME_RECORDED',
  'PING',
]);

/**
 * The approved recovery cadence, in seconds, for an unexpected close.
 *
 * The last entry repeats for every further attempt: recovery never gives up
 * on its own, because the page staying open IS the user still asking to be
 * told when the section opens.
 */
export const WATCH_RECOVERY_BACKOFF_SECONDS: readonly number[] = [1, 2, 4, 8, 16, 30];

/** How stale the last heartbeat may be before contact stops being a claim. */
export const WATCH_CONTACT_STALE_MILLISECONDS = 25_000;

interface WatchMessageEvent {
  readonly data: unknown;
}

export interface WatchSocket {
  readonly readyState: number;
  addEventListener(type: 'open' | 'close' | 'error', listener: () => void): void;
  addEventListener(type: 'message', listener: (event: WatchMessageEvent) => void): void;
  close(code?: number, reason?: string): void;
  send(data: string): void;
}

export type WatchSocketFactory = (url: string, protocols: readonly string[]) => WatchSocket;
export type WatchMessageIdSource = () => TraceId;
export type WatchServerListener = (envelope: WsServerEnvelope<WatchServerEventV1>) => void;
export type WatchConnectionState = 'IDLE' | 'CONNECTING' | 'OPEN' | 'CLOSED' | 'ERROR';
export type WatchStateListener = (state: WatchConnectionState) => void;

/**
 * What a target requires before a connection attempt may be made.
 *
 * Target-neutral on purpose. One target hands out a session ticket for the
 * lifetime of the process; another has to ask its server whether the ticket
 * it holds is still good and take a new one when it is not. The transport
 * knows only that some targets have a question to answer first, and that the
 * answer names the ticket THIS attempt must use.
 */
export type WatchSessionDecision =
  | { readonly kind: 'SESSION'; readonly session: string }
  /**
   * No ticket right now. `retryAfterSeconds` is a server-stated wait; it can
   * only ever LENGTHEN the next delay, never shorten the approved backoff.
   */
  | { readonly kind: 'UNAVAILABLE'; readonly retryAfterSeconds?: number | null };

export type WatchSessionGate = () => Promise<WatchSessionDecision>;

export type WatchRecoveryPhase =
  /** Connected, or never asked to connect. Nothing is scheduled. */
  | 'IDLE'
  /** An unexpected close is being waited out; `nextAttemptAt` is when. */
  | 'WAITING'
  /** A connection attempt is running right now. */
  | 'ATTEMPTING'
  /** The user disconnected. Nothing will reconnect until they ask again. */
  | 'STOPPED_BY_USER';

export interface WatchRecoveryState {
  readonly phase: WatchRecoveryPhase;
  /** Consecutive failed attempts since the last time the socket was open. */
  readonly attempt: number;
  /** Wall-clock milliseconds of the next attempt, while `WAITING`. */
  readonly nextAttemptAt: number | null;
}

export type WatchRecoveryListener = (state: WatchRecoveryState) => void;
/** Notified whenever the server proves it is still there. */
export type WatchContactListener = (lastContactAt: number) => void;

export interface WatchTimersPort {
  setTimeout(callback: () => void, delayMilliseconds: number): unknown;
  clearTimeout(handle: unknown): void;
}

export interface WatchClientOptions {
  readonly baseUrl?: string;
  readonly clock?: () => number;
  readonly messageId?: WatchMessageIdSource;
  readonly session: ProductSessionSource;
  /**
   * Asked before EVERY connection attempt, when a target supplies one.
   *
   * Single-flight is the transport's job, not the gate's: at most one attempt
   * is ever outstanding, so at most one question is ever in flight.
   */
  readonly sessionGate?: WatchSessionGate | undefined;
  readonly socket?: WatchSocketFactory;
  readonly timers?: WatchTimersPort;
}

export interface WatchClientPort {
  readonly state: WatchConnectionState;
  /** The last moment the server proved it was there, or `null`. */
  readonly lastContactAt: number | null;
  readonly recovery: WatchRecoveryState;
  connect(): void;
  disconnect(): void;
  dispose?(): void;
  send(command: WatchClientCommandV1): TraceId;
  subscribe(listener: WatchServerListener): () => void;
  subscribeState(listener: WatchStateListener): () => void;
  subscribeRecovery?(listener: WatchRecoveryListener): () => void;
  subscribeContact?(listener: WatchContactListener): () => void;
}

const browserTimers: WatchTimersPort = {
  setTimeout(callback, delayMilliseconds) {
    return globalThis.setTimeout(callback, delayMilliseconds);
  },
  clearTimeout(handle) {
    globalThis.clearTimeout(handle as ReturnType<typeof globalThis.setTimeout>);
  },
};

const IDLE_RECOVERY: WatchRecoveryState = { phase: 'IDLE', attempt: 0, nextAttemptAt: null };

export class WatchClient implements WatchClientPort {
  readonly #baseUrl: string;
  readonly #clock: () => number;
  readonly #messageId: WatchMessageIdSource;
  readonly #session: ProductSessionSource;
  readonly #sessionGate: WatchSessionGate | null;
  readonly #socketFactory: WatchSocketFactory;
  readonly #timers: WatchTimersPort;
  readonly #listeners = new Set<WatchServerListener>();
  readonly #stateListeners = new Set<WatchStateListener>();
  readonly #recoveryListeners = new Set<WatchRecoveryListener>();
  readonly #contactListeners = new Set<WatchContactListener>();
  readonly #seenServerEvents = new Set<string>();
  readonly #seenServerEventOrder: string[] = [];
  #socket: WatchSocket | null = null;
  #state: WatchConnectionState = 'IDLE';
  #recoveryState: WatchRecoveryState = IDLE_RECOVERY;
  #lastContactAt: number | null = null;
  /**
   * Bumped by every attempt, every close and every user decision.
   *
   * An attempt that has to await its target's answer can come back after the
   * user has pressed Disconnect, or after a newer attempt has already been
   * started. It compares this counter and drops itself rather than opening a
   * socket nobody asked for -- the one way a Disconnect could be undone by
   * work that was already running.
   */
  #generation = 0;
  #attempt = 0;
  #retryTimer: unknown | null = null;
  /** True from the user's Disconnect until the user asks to connect again. */
  #userStopped = false;
  #disposed = false;
  /** True while an attempt (including its gate question) is outstanding. */
  #attempting = false;

  constructor(options: WatchClientOptions) {
    this.#baseUrl = options.baseUrl ?? currentDocumentUrl();
    this.#clock = options.clock ?? (() => Date.now());
    this.#messageId = options.messageId ?? defaultMessageId;
    this.#session = options.session;
    this.#sessionGate = options.sessionGate ?? null;
    this.#socketFactory = options.socket ?? defaultSocketFactory;
    this.#timers = options.timers ?? browserTimers;
  }

  get state(): WatchConnectionState {
    return this.#state;
  }

  get lastContactAt(): number | null {
    return this.#lastContactAt;
  }

  get recovery(): WatchRecoveryState {
    return this.#recoveryState;
  }

  /**
   * The user asking for a connection.
   *
   * Also the ONLY thing that lifts the barrier a Disconnect puts up, and the
   * only thing that resets the backoff: an explicit ask is a new decision,
   * not the continuation of a failing series.
   */
  connect(): void {
    if (this.#disposed) return;
    this.#userStopped = false;
    if (this.#state === 'CONNECTING' || this.#state === 'OPEN' || this.#attempting) return;
    this.#cancelRetry();
    this.#attempt = 0;
    this.#beginAttempt();
  }

  /**
   * The user asking to stop.
   *
   * A hard barrier. The timer is cancelled, any attempt still awaiting its
   * target's answer is invalidated, and nothing but `connect()` starts
   * another one -- an unexpected close is the only thing recovery reacts to,
   * and this close is not unexpected.
   */
  disconnect(): void {
    this.#userStopped = true;
    this.#cancelRetry();
    this.#generation += 1;
    this.#attempting = false;
    const socket = this.#socket;
    this.#socket = null;
    this.#lastContactAt = null;
    if (socket !== null) socket.close(1000, 'client disconnect');
    this.#setRecovery({ phase: 'STOPPED_BY_USER', attempt: 0, nextAttemptAt: null });
    this.#setState('CLOSED');
  }

  /** Tears the client down for good. A disposed client never reconnects. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.disconnect();
    this.#listeners.clear();
    this.#recoveryListeners.clear();
    this.#contactListeners.clear();
  }

  send(command: WatchClientCommandV1): TraceId {
    if (this.#socket === null || this.#state !== 'OPEN' || this.#socket.readyState !== 1) {
      throw new Error('Watch socket is not open');
    }
    const messageId = this.#messageId();
    const frame = JSON.stringify({
      protocolVersion: 1,
      messageId,
      payload: command,
    } satisfies WsClientEnvelope<WatchClientCommandV1>);
    if (new TextEncoder().encode(frame).length > MAX_FRAME_BYTES) {
      throw new Error('Watch frame exceeds 64 KiB');
    }
    this.#socket.send(frame);
    return messageId;
  }

  subscribe(listener: WatchServerListener): () => void {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  }

  subscribeState(listener: WatchStateListener): () => void {
    this.#stateListeners.add(listener);
    return () => this.#stateListeners.delete(listener);
  }

  subscribeRecovery(listener: WatchRecoveryListener): () => void {
    this.#recoveryListeners.add(listener);
    return () => this.#recoveryListeners.delete(listener);
  }

  subscribeContact(listener: WatchContactListener): () => void {
    this.#contactListeners.add(listener);
    return () => this.#contactListeners.delete(listener);
  }

  /**
   * Runs one connection attempt, asking the target's gate first when there is
   * one.
   *
   * `#attempting` is set before anything can await, so a second caller --
   * a re-entrant `connect()`, a retry timer that fired -- finds an attempt
   * already outstanding and does nothing. Two sockets for one page is not a
   * faster recovery; it is two servers' worth of watches and two of every
   * alert.
   */
  #beginAttempt(): void {
    this.#generation += 1;
    const generation = this.#generation;
    this.#attempting = true;
    this.#setRecovery({ phase: 'ATTEMPTING', attempt: this.#attempt, nextAttemptAt: null });
    if (this.#sessionGate === null) {
      this.#attempting = false;
      let session: string;
      try {
        session = this.#requireSession();
      } catch (error) {
        // An explicit ask gets the error; a recovery attempt gets another
        // try. A throw inside the retry timer would escape into the page
        // and end recovery for good.
        if (this.#attempt === 0) {
          this.#setRecovery(IDLE_RECOVERY);
          throw error;
        }
        this.#scheduleRetry(null);
        return;
      }
      this.#openSocket(session, generation);
      return;
    }
    void this.#sessionGate().then(
      (decision) => {
        this.#attempting = false;
        if (generation !== this.#generation || this.#userStopped || this.#disposed) return;
        if (decision.kind === 'SESSION') {
          this.#openSocket(decision.session, generation);
          return;
        }
        this.#scheduleRetry(decision.retryAfterSeconds ?? null);
      },
      () => {
        this.#attempting = false;
        if (generation !== this.#generation || this.#userStopped || this.#disposed) return;
        this.#scheduleRetry(null);
      },
    );
  }

  #requireSession(): string {
    const session = this.#session();
    if (session === null || session.length === 0) throw new Error('Watch session is unavailable');
    return session;
  }

  #openSocket(session: string, generation: number): void {
    const url = new URL(WATCH_PATH, this.#baseUrl);
    url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
    url.search = '';
    url.searchParams.set('session', session);
    let socket: WatchSocket;
    try {
      socket = this.#socketFactory(url.toString(), [WATCH_SUBPROTOCOL]);
    } catch (error) {
      // Constructing the socket is allowed to throw synchronously (a browser
      // refuses a URL, a factory is exhausted). That is an attempt that
      // failed, not a reason to stop trying -- unless this was the user's
      // own first ask, in which case the caller sees the error.
      if (this.#attempt === 0) {
        this.#setRecovery(IDLE_RECOVERY);
        throw error;
      }
      this.#scheduleRetry(null);
      return;
    }
    this.#seenServerEvents.clear();
    this.#seenServerEventOrder.length = 0;
    this.#socket = socket;
    this.#setState('CONNECTING');
    socket.addEventListener('open', () => {
      if (this.#socket !== socket) return;
      this.#attempt = 0;
      this.#setRecovery(IDLE_RECOVERY);
      // The completed upgrade is itself first-hand evidence the server is
      // there. Heartbeats keep it fresh from here; nothing else does.
      this.#recordContact();
      this.#setState('OPEN');
    });
    socket.addEventListener('message', (event) => {
      if (this.#socket !== socket) return;
      const envelope = decodeServerEnvelope(event.data);
      if (envelope !== null && this.#acceptServerEnvelope(envelope)) {
        if (envelope.payload.type === 'PING') {
          this.#recordContact();
          this.#acknowledgePing(envelope.payload.sequence);
        }
        this.#listeners.forEach((listener) => listener(envelope));
      }
    });
    socket.addEventListener('close', () => {
      if (this.#socket !== socket) return;
      this.#socket = null;
      this.#lastContactAt = null;
      this.#setState('CLOSED');
      this.#recoverFromUnexpectedClose(generation);
    });
    socket.addEventListener('error', () => {
      if (this.#socket !== socket) return;
      this.#setState('ERROR');
    });
  }

  /**
   * Schedules the next attempt after a close nobody asked for.
   *
   * A socket that errored and then closed produces one recovery, not two:
   * the close clears `#socket`, and every later event from it is ignored.
   */
  #recoverFromUnexpectedClose(generation: number): void {
    if (this.#userStopped || this.#disposed || generation !== this.#generation) return;
    this.#scheduleRetry(null);
  }

  #scheduleRetry(retryAfterSeconds: number | null): void {
    if (this.#userStopped || this.#disposed) return;
    this.#cancelRetry();
    const attempt = this.#attempt;
    this.#attempt = attempt + 1;
    const base = WATCH_RECOVERY_BACKOFF_SECONDS[
      Math.min(attempt, WATCH_RECOVERY_BACKOFF_SECONDS.length - 1)
    ] ?? 30;
    // A server-stated wait can only make the page WAIT LONGER. Taking it as
    // the delay outright would let a `Retry-After: 0` turn the approved
    // backoff into a hot loop against a server that is already struggling.
    const seconds = Number.isFinite(retryAfterSeconds) && retryAfterSeconds !== null
      ? Math.max(base, retryAfterSeconds)
      : base;
    const delay = Math.max(0, seconds) * 1_000;
    this.#setRecovery({
      phase: 'WAITING',
      attempt: this.#attempt,
      nextAttemptAt: this.#clock() + delay,
    });
    this.#retryTimer = this.#timers.setTimeout(() => {
      this.#retryTimer = null;
      if (this.#userStopped || this.#disposed) return;
      if (this.#state === 'OPEN' || this.#state === 'CONNECTING' || this.#attempting) return;
      this.#beginAttempt();
    }, delay);
  }

  #cancelRetry(): void {
    if (this.#retryTimer === null) return;
    this.#timers.clearTimeout(this.#retryTimer);
    this.#retryTimer = null;
  }

  #recordContact(): void {
    const at = this.#clock();
    this.#lastContactAt = at;
    this.#contactListeners.forEach((listener) => listener(at));
  }

  // Passive heartbeat reply, issued from the message handler itself: hidden
  // tabs throttle client timers, but message events keep firing, so this is
  // the only reply path that stays alive in a background tab. Never throws --
  // a PING racing the socket teardown is dropped, not fatal.
  #acknowledgePing(sequence: number): void {
    if (this.#socket === null || this.#state !== 'OPEN' || this.#socket.readyState !== 1) return;
    try {
      this.send({ type: 'HEARTBEAT_ACK', sequence });
    } catch {
      // Teardown races leave the ACK unsent; the next PING retries.
    }
  }

  #setState(state: WatchConnectionState): void {
    if (state === this.#state) return;
    this.#state = state;
    this.#stateListeners.forEach((listener) => listener(state));
  }

  #setRecovery(state: WatchRecoveryState): void {
    const current = this.#recoveryState;
    if (
      current.phase === state.phase
      && current.attempt === state.attempt
      && current.nextAttemptAt === state.nextAttemptAt
    ) return;
    this.#recoveryState = state;
    this.#recoveryListeners.forEach((listener) => listener(state));
  }

  #acceptServerEnvelope(envelope: WsServerEnvelope<WatchServerEventV1>): boolean {
    if (!this.#rememberServerEvent(
      `message:${envelope.messageId}:payload:${JSON.stringify(envelope.payload)}`,
    )) return false;
    if (envelope.payload.type !== 'OPEN_OBSERVATION') return true;
    const { activeWatchId, observation } = envelope.payload.fanout;
    return this.#rememberServerEvent(`observation:${activeWatchId}:${observation.observationId}`);
  }

  #rememberServerEvent(key: string): boolean {
    if (this.#seenServerEvents.has(key)) return false;
    this.#seenServerEvents.add(key);
    this.#seenServerEventOrder.push(key);
    const expired = this.#seenServerEventOrder.length > MAX_REMEMBERED_SERVER_EVENTS
      ? this.#seenServerEventOrder.shift()
      : undefined;
    if (expired !== undefined) this.#seenServerEvents.delete(expired);
    return true;
  }
}

function currentDocumentUrl(): string {
  if (typeof window === 'undefined') throw new Error('WatchClient requires an explicit baseUrl');
  return window.location.href;
}

function defaultSocketFactory(url: string, protocols: readonly string[]): WatchSocket {
  return new WebSocket(url, [...protocols]);
}

function defaultMessageId(): TraceId {
  return crypto.randomUUID();
}

/**
 * A heartbeat's sequence, or `null` when the frame cannot be one.
 *
 * Contact is the only evidence behind a green light that no user action
 * refreshes, so the frame that carries it is checked rather than trusted: a
 * sequence that is absent, fractional, zero, negative, or past the range
 * where integers are exact is a malformed frame from a server that may or
 * may not be the one we think we are talking to, and treating it as contact
 * is exactly the false green this surface exists to prevent.
 */
function heartbeatSequence(payload: { readonly type: string; readonly sequence?: unknown }): number | null {
  const { sequence } = payload;
  if (typeof sequence !== 'number' || !Number.isSafeInteger(sequence) || sequence <= 0) return null;
  return sequence;
}

function decodeServerEnvelope(data: unknown): WsServerEnvelope<WatchServerEventV1> | null {
  if (typeof data !== 'string' || new TextEncoder().encode(data).length > MAX_FRAME_BYTES) return null;
  try {
    const value = JSON.parse(data) as unknown;
    if (
      typeof value !== 'object'
      || value === null
      || Array.isArray(value)
      || !('protocolVersion' in value)
      || value.protocolVersion !== 1
      || !('messageId' in value)
      || typeof value.messageId !== 'string'
      || !('payload' in value)
      || typeof value.payload !== 'object'
      || value.payload === null
      || Array.isArray(value.payload)
      || !('type' in value.payload)
      || typeof value.payload.type !== 'string'
      || !SERVER_EVENT_TYPES.has(value.payload.type)
    ) {
      return null;
    }
    if (
      value.payload.type === 'PING'
      && heartbeatSequence(value.payload as { readonly type: string; readonly sequence?: unknown }) === null
    ) {
      return null;
    }
    return value as WsServerEnvelope<WatchServerEventV1>;
  } catch {
    return null;
  }
}
