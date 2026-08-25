import { describe, expect, it } from 'vitest';

import type { WsServerEnvelope } from '../src/ui/shared/product/contracts/common';
import type { WatchServerEventV1 } from '../src/ui/shared/product/contracts/watch';
import {
  WatchClient,
  type WatchSessionDecision,
  type WatchSocket,
} from '../src/ui/shared/product/WatchClient';

class FakeSocket implements WatchSocket {
  readyState = 0;
  readonly sent: string[] = [];
  readonly listeners = new Map<string, Set<(event?: { readonly data: unknown }) => void>>();

  addEventListener(
    type: 'open' | 'close' | 'error' | 'message',
    listener: (event: never) => void,
  ): void {
    const listeners = this.listeners.get(type) ?? new Set();
    listeners.add(listener as (event?: { readonly data: unknown }) => void);
    this.listeners.set(type, listeners);
  }

  close(): void {
    this.readyState = 3;
    this.emit('close');
  }

  send(data: string): void {
    this.sent.push(data);
  }

  open(): void {
    this.readyState = 1;
    this.emit('open');
  }

  error(): void {
    this.emit('error');
  }

  message(data: string): void {
    this.emit('message', { data });
  }

  emit(type: string, event?: { readonly data: unknown }): void {
    this.listeners.get(type)?.forEach((listener) => listener(event));
  }

  /** True once the client has attached its handlers to this socket. */
  get built(): boolean {
    return this.listeners.size > 0;
  }
}

/**
 * A clock and a timer wheel the test drives by hand.
 *
 * Recovery is defined in seconds of waiting, and a test that waited them out
 * for real would be both slow and unable to say WHICH delay it observed.
 */
class ManualTimers {
  #now = 1_000_000;
  #next = 1;
  readonly scheduled = new Map<number, { readonly at: number; readonly callback: () => void }>();

  readonly clock = (): number => this.#now;

  readonly port = {
    setTimeout: (callback: () => void, delayMilliseconds: number): unknown => {
      const handle = this.#next++;
      this.scheduled.set(handle, { at: this.#now + delayMilliseconds, callback });
      return handle;
    },
    clearTimeout: (handle: unknown): void => {
      this.scheduled.delete(handle as number);
    },
  };

  /** The wait, in seconds, of the one timer that is pending. */
  pendingSeconds(): number {
    const entries = [...this.scheduled.values()];
    if (entries.length !== 1) {
      throw new Error(`expected exactly one pending timer, saw ${entries.length}`);
    }
    return (entries[0]!.at - this.#now) / 1_000;
  }

  advance(): void {
    const entries = [...this.scheduled.entries()];
    if (entries.length !== 1) {
      throw new Error(`expected exactly one pending timer, saw ${entries.length}`);
    }
    const [handle, entry] = entries[0]!;
    this.scheduled.delete(handle);
    this.#now = entry.at;
    entry.callback();
  }

  tick(milliseconds: number): void {
    this.#now += milliseconds;
  }
}

interface ClientHarness {
  readonly client: WatchClient;
  readonly urls: string[];
  builtCount(): number;
}

function harness(options: {
  readonly sockets: readonly FakeSocket[];
  readonly timers: ManualTimers;
  readonly gate?: () => Promise<WatchSessionDecision>;
  readonly session?: () => string | null;
}): ClientHarness {
  const urls: string[] = [];
  let built = 0;
  const client = new WatchClient({
    baseUrl: 'https://planner.invalid/',
    clock: options.timers.clock,
    session: options.session ?? (() => 'bootstrap-ticket'),
    socket: (url) => {
      urls.push(url);
      const socket = options.sockets[built++];
      if (socket === undefined) throw new Error('no further sockets were prepared');
      return socket;
    },
    ...(options.gate === undefined ? {} : { sessionGate: options.gate }),
    timers: options.timers.port,
    messageId: () => '00000000-0000-4000-8000-000000000001',
  });
  return { client, urls, builtCount: () => built };
}

/** Lets every already-resolved gate promise settle. */
async function settle(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

describe('WatchClient recovery', () => {
  it('walks the approved backoff after unexpected closes and resets it after a success', () => {
    const sockets = Array.from({ length: 10 }, () => new FakeSocket());
    const timers = new ManualTimers();
    const { client } = harness({ sockets, timers });

    client.connect();
    sockets[0]!.open();
    expect(client.state).toBe('OPEN');
    expect(client.recovery.phase).toBe('IDLE');

    // 1 / 2 / 4 / 8 / 16 / 30, then 30 for every further attempt.
    [1, 2, 4, 8, 16, 30, 30].forEach((seconds, attempt) => {
      sockets[attempt]!.close();
      expect(client.recovery.phase).toBe('WAITING');
      expect(client.recovery.attempt).toBe(attempt + 1);
      expect(timers.pendingSeconds()).toBe(seconds);
      timers.advance();
    });

    // Socket 7 is the one the seventh retry built. Opening it is a success, so
    // the next unexpected close starts from 1s again: a page that has been
    // connected for hours is not "on attempt eight".
    sockets[7]!.open();
    expect(client.recovery.phase).toBe('IDLE');
    sockets[7]!.close();
    expect(timers.pendingSeconds()).toBe(1);
  });

  it('recovers once from a socket that errors and then closes', () => {
    const sockets = [new FakeSocket(), new FakeSocket()];
    const timers = new ManualTimers();
    const { client } = harness({ sockets, timers });

    client.connect();
    sockets[0]!.open();
    sockets[0]!.error();
    sockets[0]!.close();

    // One recovery, not two. Two would put two sockets on one page, and with
    // them two of every alert.
    expect(timers.scheduled.size).toBe(1);
    expect(client.recovery.attempt).toBe(1);
  });

  it('never reconnects after an explicit disconnect; only connect() lifts the barrier', () => {
    const sockets = [new FakeSocket(), new FakeSocket()];
    const timers = new ManualTimers();
    const { client, builtCount } = harness({ sockets, timers });

    client.connect();
    sockets[0]!.open();
    client.disconnect();

    expect(client.state).toBe('CLOSED');
    expect(client.recovery.phase).toBe('STOPPED_BY_USER');
    expect(timers.scheduled.size).toBe(0);

    // The socket's own close event lands after the user's decision. It is not
    // an unexpected close and must schedule nothing.
    sockets[0]!.close();
    expect(timers.scheduled.size).toBe(0);
    expect(client.recovery.phase).toBe('STOPPED_BY_USER');
    expect(builtCount()).toBe(1);

    client.connect();
    expect(client.state).toBe('CONNECTING');
    expect(builtCount()).toBe(2);
  });

  it('does not reconnect after dispose, even when asked to connect again', () => {
    const sockets = [new FakeSocket(), new FakeSocket()];
    const timers = new ManualTimers();
    const { client, builtCount } = harness({ sockets, timers });

    client.connect();
    sockets[0]!.open();
    client.dispose();
    sockets[0]!.close();
    client.connect();

    expect(timers.scheduled.size).toBe(0);
    expect(client.state).toBe('CLOSED');
    expect(builtCount()).toBe(1);
    expect(sockets[1]!.built).toBe(false);
  });

  it('asks its session gate before every attempt and connects with the ticket it answers', async () => {
    const sockets = [new FakeSocket(), new FakeSocket()];
    const timers = new ManualTimers();
    const tickets = ['first-ticket', 'second-ticket'];
    let asked = 0;
    const { client, urls } = harness({
      sockets,
      timers,
      gate: async () => ({ kind: 'SESSION', session: tickets[asked++] ?? 'exhausted' }),
    });

    client.connect();
    await settle();
    sockets[0]!.open();
    sockets[0]!.close();
    timers.advance();
    await settle();

    expect(asked).toBe(2);
    // The reconnect uses the ticket the SECOND question answered. Reusing the
    // one the page booted with is exactly how a renewed session reconnects
    // with a credential the server has already thrown away.
    expect(urls).toEqual([
      'wss://planner.invalid/api/v1/watch?session=first-ticket',
      'wss://planner.invalid/api/v1/watch?session=second-ticket',
    ]);
  });

  it('lets a stated wait lengthen the backoff and never shorten it', async () => {
    const sockets = [new FakeSocket(), new FakeSocket(), new FakeSocket()];
    const timers = new ManualTimers();
    const waits: readonly (number | undefined)[] = [90, 0];
    let attempt = 0;
    const { client } = harness({
      sockets,
      timers,
      gate: async () => {
        const retryAfterSeconds = waits[attempt++];
        return retryAfterSeconds === undefined
          ? { kind: 'SESSION', session: 'nonce' }
          : { kind: 'UNAVAILABLE', retryAfterSeconds };
      },
    });

    client.connect();
    await settle();
    // Refused with a 90s wait: longer than the 1s base, so it wins.
    expect(timers.pendingSeconds()).toBe(90);
    timers.advance();
    await settle();
    // Refused with `Retry-After: 0`. The approved 2s base still applies -- a
    // zero from the server may not turn recovery into a spin.
    expect(timers.pendingSeconds()).toBe(2);
  });

  it('drops a gate answer that arrives after the user disconnected', async () => {
    const sockets = [new FakeSocket()];
    const timers = new ManualTimers();
    let release: ((decision: WatchSessionDecision) => void) | null = null;
    const { client, builtCount } = harness({
      sockets,
      timers,
      gate: () => new Promise<WatchSessionDecision>((resolve) => { release = resolve; }),
    });

    client.connect();
    await Promise.resolve();
    client.disconnect();
    release?.({ kind: 'SESSION', session: 'nonce' });
    await settle();

    // Nothing was built. The answer belongs to a question the user has since
    // withdrawn, and acting on it would undo their Disconnect.
    expect(builtCount()).toBe(0);
    expect(sockets[0]!.built).toBe(false);
    expect(client.state).toBe('CLOSED');
    expect(client.recovery.phase).toBe('STOPPED_BY_USER');
  });

  it('keeps exactly one attempt outstanding when a pending retry and connect() race', () => {
    const sockets = [new FakeSocket(), new FakeSocket(), new FakeSocket()];
    const timers = new ManualTimers();
    const { client, builtCount } = harness({ sockets, timers });

    client.connect();
    sockets[0]!.close();
    expect(timers.scheduled.size).toBe(1);

    // The user presses connect while the retry is still pending: the timer is
    // cancelled rather than left to fire into a second socket.
    client.connect();
    expect(timers.scheduled.size).toBe(0);
    expect(builtCount()).toBe(2);
  });

  it('keeps retrying when the ticket source itself is empty', () => {
    const sockets = [new FakeSocket(), new FakeSocket()];
    const timers = new ManualTimers();
    let ticket: string | null = 'bootstrap-ticket';
    const { client } = harness({ sockets, timers, session: () => ticket });

    client.connect();
    sockets[0]!.open();
    ticket = null;
    sockets[0]!.close();

    // The retry fires into a missing ticket. That is a failed attempt, not an
    // exception escaping a timer and ending recovery for good.
    expect(() => timers.advance()).not.toThrow();
    expect(timers.pendingSeconds()).toBe(2);
  });

  it('reports a missing ticket to the caller that asked for the connection', () => {
    const timers = new ManualTimers();
    const { client } = harness({ sockets: [], timers, session: () => null });

    expect(() => client.connect()).toThrow(/session is unavailable/u);
    expect(timers.scheduled.size).toBe(0);
  });
});

describe('WatchClient heartbeat evidence', () => {
  it('counts a valid heartbeat as contact and forgets contact when the socket closes', () => {
    const socket = new FakeSocket();
    const timers = new ManualTimers();
    const { client } = harness({ sockets: [socket, new FakeSocket()], timers });
    const contacts: number[] = [];
    client.subscribeContact((at) => contacts.push(at));

    client.connect();
    socket.open();
    const openedAt = client.lastContactAt;
    expect(openedAt).not.toBeNull();

    timers.tick(11_000);
    socket.message(JSON.stringify({
      protocolVersion: 1,
      messageId: '10000000-0000-4000-8000-000000000001',
      payload: { type: 'PING', sequence: 1 },
    }));
    expect(client.lastContactAt).toBe(openedAt! + 11_000);
    expect(contacts).toHaveLength(2);

    socket.close();
    // A closed socket is not stale contact, it is no contact. Leaving the last
    // timestamp behind would let a page twenty-four seconds into a dead
    // connection still claim it was in touch a moment ago.
    expect(client.lastContactAt).toBeNull();
  });

  it('refuses a heartbeat whose sequence is not a positive safe integer', () => {
    const socket = new FakeSocket();
    const timers = new ManualTimers();
    const { client } = harness({ sockets: [socket], timers });
    const received: WsServerEnvelope<WatchServerEventV1>[] = [];
    client.subscribe((envelope) => received.push(envelope));

    client.connect();
    socket.open();
    const openedAt = client.lastContactAt;
    socket.sent.length = 0;

    const refused: readonly unknown[] = [
      0,
      -1,
      1.5,
      '3',
      null,
      Number.NaN,
      Number.MAX_SAFE_INTEGER + 1,
    ];
    refused.forEach((sequence, index) => {
      timers.tick(1_000);
      socket.message(JSON.stringify({
        protocolVersion: 1,
        messageId: `10000000-0000-4000-8000-${String(index + 1).padStart(12, '0')}`,
        payload: { type: 'PING', sequence },
      }));
    });

    // No acknowledgement, no delivery, and -- the reason the rule exists -- no
    // refreshed contact. A malformed frame from an otherwise trusted server
    // would hold a green light up indefinitely.
    expect(socket.sent).toEqual([]);
    expect(received).toEqual([]);
    expect(client.lastContactAt).toBe(openedAt);
  });
});
