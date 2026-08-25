/**
 * Tells the local runtime this page exists.
 *
 * Local-only, and separate from the watch socket on purpose. A page that is
 * only browsing has no watch connection at all, and a runtime that counted
 * watch connections would exit under a user who was reading a course list.
 * This connection says one thing -- "a tab is open" -- and it says it for as
 * long as the tab is.
 *
 * It is also independent of Disconnect. Stopping alerts is a decision about
 * alerts; it is not the user leaving.
 */

export const LOCAL_PRESENCE_PATH = '/api/v1/local/presence';
export const LOCAL_PRESENCE_SUBPROTOCOL = 'bcsp.v1';

/** The same recovery cadence the watch transport uses, for the same reason. */
export const LOCAL_PRESENCE_BACKOFF_SECONDS: readonly number[] = [1, 2, 4, 8, 16, 30];

export interface LocalPresenceSocket {
  addEventListener(type: 'open' | 'close' | 'error', listener: () => void): void;
  addEventListener(type: 'message', listener: (event: { readonly data: unknown }) => void): void;
  close(code?: number, reason?: string): void;
  send(data: string): void;
}

export type LocalPresenceSocketFactory = (
  url: string,
  protocols: readonly string[],
) => LocalPresenceSocket;

export interface LocalPresenceTimersPort {
  setTimeout(callback: () => void, delayMilliseconds: number): unknown;
  clearTimeout(handle: unknown): void;
}

export interface LocalPresenceClientOptions {
  readonly baseUrl: string;
  readonly session: () => string | null;
  /**
   * This tab's identity, stable for the life of the page.
   *
   * A refresh is a NEW tab identity, and that is correct: the runtime binds a
   * tab to the connection currently speaking for it, so a reload simply
   * registers again. What must never happen is two live tabs sharing one
   * identity, which is why this is minted per page rather than stored.
   */
  readonly tabId?: string | undefined;
  readonly socket?: LocalPresenceSocketFactory | undefined;
  readonly timers?: LocalPresenceTimersPort | undefined;
  readonly messageId?: (() => string) | undefined;
}

export interface LocalPresenceClient {
  /** Opens the connection and keeps it open. */
  start(): void;
  /** Closes it for good. A disposed client never reconnects. */
  dispose(): void;
  /** How many attempts have failed since the last open connection. */
  readonly attempt: number;
}

const browserTimers: LocalPresenceTimersPort = {
  setTimeout(callback, delayMilliseconds) {
    return globalThis.setTimeout(callback, delayMilliseconds);
  },
  clearTimeout(handle) {
    globalThis.clearTimeout(handle as ReturnType<typeof globalThis.setTimeout>);
  },
};

function defaultSocketFactory(url: string, protocols: readonly string[]): LocalPresenceSocket {
  return new WebSocket(url, [...protocols]) as unknown as LocalPresenceSocket;
}

function randomId(): string {
  return crypto.randomUUID();
}

export function createLocalPresenceClient(
  options: LocalPresenceClientOptions,
): LocalPresenceClient {
  const socketFactory = options.socket ?? defaultSocketFactory;
  const timers = options.timers ?? browserTimers;
  const messageId = options.messageId ?? randomId;
  const tabId = options.tabId ?? randomId();
  let socket: LocalPresenceSocket | null = null;
  let retry: unknown | null = null;
  let attempt = 0;
  let disposed = false;
  let generation = 0;

  const scheduleRetry = () => {
    if (disposed) return;
    const index = Math.min(attempt, LOCAL_PRESENCE_BACKOFF_SECONDS.length - 1);
    const seconds = LOCAL_PRESENCE_BACKOFF_SECONDS[index] ?? 30;
    attempt += 1;
    retry = timers.setTimeout(() => {
      retry = null;
      open();
    }, seconds * 1_000);
  };

  const open = () => {
    if (disposed || socket !== null) return;
    const session = options.session();
    if (session === null || session.length === 0) {
      // No session yet. This is a page that has not finished starting, not a
      // page that is gone -- so keep trying rather than telling the runtime
      // nothing is here.
      scheduleRetry();
      return;
    }
    generation += 1;
    const attemptGeneration = generation;
    const url = new URL(LOCAL_PRESENCE_PATH, options.baseUrl);
    url.protocol = url.protocol === 'https:' ? 'wss:' : 'ws:';
    url.search = '';
    url.searchParams.set('session', session);
    let opened: LocalPresenceSocket;
    try {
      opened = socketFactory(url.toString(), [LOCAL_PRESENCE_SUBPROTOCOL]);
    } catch {
      scheduleRetry();
      return;
    }
    socket = opened;
    opened.addEventListener('open', () => {
      if (socket !== opened || generation !== attemptGeneration) return;
      attempt = 0;
      // The first frame is the identity. Until it arrives the runtime does
      // not count this connection at all, so nothing is claimed by merely
      // having connected.
      try {
        opened.send(JSON.stringify({
          protocolVersion: 1,
          messageId: messageId(),
          payload: { type: 'HELLO', tabId },
        }));
      } catch {
        // The socket died between opening and the first frame. Its close
        // handler starts the next attempt.
      }
    });
    opened.addEventListener('close', () => {
      if (socket !== opened) return;
      socket = null;
      // A presence connection that drops while the tab is still open is a
      // page the runtime can no longer see. Recovering it is what stops a
      // sixty-second countdown from starting under a tab that never left.
      scheduleRetry();
    });
    opened.addEventListener('error', () => {
      // The close that follows is what schedules the next attempt; an error
      // on its own is not a second outage.
    });
  };

  return {
    get attempt() {
      return attempt;
    },
    start() {
      if (disposed) return;
      open();
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      generation += 1;
      if (retry !== null) {
        timers.clearTimeout(retry);
        retry = null;
      }
      const closing = socket;
      socket = null;
      closing?.close(1000, 'page closed');
    },
  };
}
