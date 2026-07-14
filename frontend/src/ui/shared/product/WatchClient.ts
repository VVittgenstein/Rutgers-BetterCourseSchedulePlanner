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
]);

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

export interface WatchClientOptions {
  readonly baseUrl?: string;
  readonly messageId?: WatchMessageIdSource;
  readonly session: ProductSessionSource;
  readonly socket?: WatchSocketFactory;
}

export interface WatchClientPort {
  readonly state: WatchConnectionState;
  connect(): void;
  disconnect(): void;
  send(command: WatchClientCommandV1): TraceId;
  subscribe(listener: WatchServerListener): () => void;
  subscribeState(listener: WatchStateListener): () => void;
}

export class WatchClient implements WatchClientPort {
  readonly #baseUrl: string;
  readonly #messageId: WatchMessageIdSource;
  readonly #session: ProductSessionSource;
  readonly #socketFactory: WatchSocketFactory;
  readonly #listeners = new Set<WatchServerListener>();
  readonly #stateListeners = new Set<WatchStateListener>();
  readonly #seenServerEvents = new Set<string>();
  readonly #seenServerEventOrder: string[] = [];
  #socket: WatchSocket | null = null;
  #state: WatchConnectionState = 'IDLE';

  constructor(options: WatchClientOptions) {
    this.#baseUrl = options.baseUrl ?? currentDocumentUrl();
    this.#messageId = options.messageId ?? defaultMessageId;
    this.#session = options.session;
    this.#socketFactory = options.socket ?? defaultSocketFactory;
  }

  get state(): WatchConnectionState {
    return this.#state;
  }

  connect(): void {
    if (this.#state === 'CONNECTING' || this.#state === 'OPEN') return;
    const session = this.#session();
    if (session === null || session.length === 0) throw new Error('Watch session is unavailable');
    const url = new URL(WATCH_PATH, this.#baseUrl);
    url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
    url.search = '';
    url.searchParams.set('session', session);
    const socket = this.#socketFactory(url.toString(), [WATCH_SUBPROTOCOL]);
    this.#seenServerEvents.clear();
    this.#seenServerEventOrder.length = 0;
    this.#socket = socket;
    this.#setState('CONNECTING');
    socket.addEventListener('open', () => {
      if (this.#socket === socket) this.#setState('OPEN');
    });
    socket.addEventListener('message', (event) => {
      if (this.#socket !== socket) return;
      const envelope = decodeServerEnvelope(event.data);
      if (envelope !== null && this.#acceptServerEnvelope(envelope)) {
        this.#listeners.forEach((listener) => listener(envelope));
      }
    });
    socket.addEventListener('close', () => {
      if (this.#socket === socket) {
        this.#socket = null;
        this.#setState('CLOSED');
      }
    });
    socket.addEventListener('error', () => {
      if (this.#socket === socket) this.#setState('ERROR');
    });
  }

  disconnect(): void {
    const socket = this.#socket;
    this.#socket = null;
    if (socket !== null) socket.close(1000, 'client disconnect');
    this.#setState('CLOSED');
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

  #setState(state: WatchConnectionState): void {
    if (state === this.#state) return;
    this.#state = state;
    this.#stateListeners.forEach((listener) => listener(state));
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
    return value as WsServerEnvelope<WatchServerEventV1>;
  } catch {
    return null;
  }
}
