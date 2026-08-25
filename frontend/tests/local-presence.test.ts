import { describe, expect, it, vi } from 'vitest';

import {
  createLocalPresenceClient,
  LOCAL_PRESENCE_SUBPROTOCOL,
  type LocalPresenceSocket,
} from '../src/ui/local/presence/LocalPresenceClient';

class FakeSocket implements LocalPresenceSocket {
  readonly sent: string[] = [];
  closedWith: number | null = null;
  readonly listeners = new Map<string, Set<() => void>>();

  addEventListener(type: string, listener: () => void): void {
    const listeners = this.listeners.get(type) ?? new Set();
    listeners.add(listener);
    this.listeners.set(type, listeners);
  }

  close(code?: number): void {
    this.closedWith = code ?? null;
    this.emit('close');
  }

  send(data: string): void {
    this.sent.push(data);
  }

  open(): void {
    this.emit('open');
  }

  emit(type: string): void {
    this.listeners.get(type)?.forEach((listener) => listener());
  }
}

class ManualTimers {
  #next = 1;
  readonly scheduled = new Map<number, { readonly delay: number; readonly callback: () => void }>();

  readonly port = {
    setTimeout: (callback: () => void, delay: number): unknown => {
      const handle = this.#next++;
      this.scheduled.set(handle, { callback, delay });
      return handle;
    },
    clearTimeout: (handle: unknown): void => {
      this.scheduled.delete(handle as number);
    },
  };

  pendingSeconds(): number {
    const entries = [...this.scheduled.values()];
    if (entries.length !== 1) throw new Error(`expected one timer, saw ${entries.length}`);
    return entries[0]!.delay / 1_000;
  }

  fire(): void {
    const entries = [...this.scheduled.entries()];
    if (entries.length !== 1) throw new Error(`expected one timer, saw ${entries.length}`);
    const [handle, entry] = entries[0]!;
    this.scheduled.delete(handle);
    entry.callback();
  }
}

function harness(session: () => string | null = () => 'session-nonce') {
  const sockets: FakeSocket[] = [];
  const urls: string[] = [];
  const protocols: string[][] = [];
  const timers = new ManualTimers();
  const client = createLocalPresenceClient({
    baseUrl: 'http://127.0.0.1:5173/local.html',
    session,
    tabId: '00000000-0000-4000-8000-0000000ab001',
    messageId: () => '00000000-0000-4000-8000-000000000001',
    socket: (url, requested) => {
      urls.push(url);
      protocols.push([...requested]);
      const socket = new FakeSocket();
      sockets.push(socket);
      return socket;
    },
    timers: timers.port,
  });
  return { client, protocols, sockets, timers, urls };
}

describe('the local presence connection', () => {
  it('identifies its tab on the first frame, and not before', () => {
    const { client, protocols, sockets, urls } = harness();
    client.start();

    expect(urls).toEqual(['ws://127.0.0.1:5173/api/v1/local/presence?session=session-nonce']);
    expect(protocols[0]).toEqual([LOCAL_PRESENCE_SUBPROTOCOL]);
    // Nothing has been claimed yet: the runtime counts a connection only once
    // it says which tab it is.
    expect(sockets[0]?.sent).toEqual([]);

    sockets[0]?.open();
    expect(JSON.parse(sockets[0]?.sent[0] ?? '')).toEqual({
      protocolVersion: 1,
      messageId: '00000000-0000-4000-8000-000000000001',
      payload: { type: 'HELLO', tabId: '00000000-0000-4000-8000-0000000ab001' },
    });
  });

  it('recovers on its own when the connection drops under a tab that is still open', () => {
    const { client, sockets, timers } = harness();
    client.start();
    sockets[0]?.open();

    sockets[0]?.emit('close');
    // A dropped presence connection is a page the runtime can no longer see.
    // Left unrecovered it starts a sixty-second countdown under a tab that
    // never went anywhere.
    expect(timers.pendingSeconds()).toBe(1);
    timers.fire();
    expect(sockets).toHaveLength(2);
    sockets[1]?.open();
    expect(sockets[1]?.sent).toHaveLength(1);

    // A success resets the ladder: an outage an hour ago is not this one.
    sockets[1]?.emit('close');
    expect(timers.pendingSeconds()).toBe(1);
  });

  it('walks the same backoff as the watch transport when attempts keep failing', () => {
    const { client, sockets, timers } = harness();
    client.start();

    [1, 2, 4, 8, 16, 30, 30].forEach((seconds, index) => {
      sockets[index]?.emit('close');
      expect(timers.pendingSeconds()).toBe(seconds);
      timers.fire();
    });
  });

  it('keeps trying while the page has no session yet', () => {
    let session: string | null = null;
    const { client, sockets, timers } = harness(() => session);
    client.start();

    // Not "there is nobody here" -- "this page has not finished starting".
    expect(sockets).toHaveLength(0);
    expect(timers.pendingSeconds()).toBe(1);
    session = 'session-nonce';
    timers.fire();
    expect(sockets).toHaveLength(1);
  });

  it('stops for good when the page goes away', () => {
    const { client, sockets, timers } = harness();
    client.start();
    sockets[0]?.open();

    client.dispose();
    expect(sockets[0]?.closedWith).toBe(1000);
    expect(timers.scheduled.size).toBe(0);

    // The close it just caused must not schedule a reconnection for a page
    // that is being torn down.
    sockets[0]?.emit('close');
    expect(timers.scheduled.size).toBe(0);
    client.start();
    expect(sockets).toHaveLength(1);
  });

  it('does not treat an error and its close as two outages', () => {
    const { client, sockets, timers } = harness();
    client.start();
    sockets[0]?.open();

    sockets[0]?.emit('error');
    sockets[0]?.emit('close');
    expect(timers.scheduled.size).toBe(1);
  });

  it('never sends the session anywhere but the connection query', () => {
    const { client, sockets } = harness();
    client.start();
    sockets[0]?.open();
    const frames = sockets[0]?.sent ?? [];
    expect(frames).toHaveLength(1);
    expect(frames[0]).not.toContain('session-nonce');
    expect(vi.isMockFunction(client.start)).toBe(false);
  });
});
