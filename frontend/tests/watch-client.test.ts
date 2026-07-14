import { readFileSync } from 'node:fs';

import { describe, expect, it, vi } from 'vitest';

import type { WsServerEnvelope } from '../src/ui/shared/product/contracts/common';
import type { WatchClientCommandV1, WatchServerEventV1 } from '../src/ui/shared/product/contracts/watch';
import {
  WATCH_SUBPROTOCOL,
  WatchClient,
  type WatchMessageIdSource,
  type WatchSocket,
} from '../src/ui/shared/product/WatchClient';

function readGolden<T>(name: string): T {
  return JSON.parse(readFileSync(
    new URL(`../../crates/bcsp-contracts/tests/golden/${name}`, import.meta.url),
    'utf8',
  )) as T;
}

class FakeSocket implements WatchSocket {
  readyState = 0;
  readonly sent: string[] = [];
  readonly listeners = new Map<string, Set<(event?: { readonly data: unknown }) => void>>();

  addEventListener(type: 'open' | 'close' | 'error' | 'message', listener: (event: never) => void): void {
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

  message(data: string): void {
    this.emit('message', { data });
  }

  emit(type: string, event?: { readonly data: unknown }): void {
    this.listeners.get(type)?.forEach((listener) => listener(event));
  }
}

describe('WatchClient', () => {
  it('uses the session query, bcsp.v1 subprotocol, and Rust envelopes', () => {
    const command = readGolden<WatchClientCommandV1>('watch-client-start-v1.json');
    const event = readGolden<WatchServerEventV1>('watch-server-start-result-v1.json');
    const observation = readGolden<WatchServerEventV1>('watch-server-observation-v1.json');
    const socket = new FakeSocket();
    const socketFactory = vi.fn(() => socket);
    const messageId: WatchMessageIdSource = () => '00000000-0000-4000-8000-000000000001';
    const client = new WatchClient({
      baseUrl: 'https://planner.invalid/course',
      session: () => 'nonce with space',
      socket: socketFactory,
      messageId,
    });
    const received: WsServerEnvelope<WatchServerEventV1>[] = [];
    client.subscribe((envelope) => received.push(envelope));

    client.connect();
    expect(socketFactory).toHaveBeenCalledWith(
      'wss://planner.invalid/api/v1/watch?session=nonce+with+space',
      [WATCH_SUBPROTOCOL],
    );
    socket.open();
    expect(client.state).toBe('OPEN');
    expect(client.send(command)).toBe('00000000-0000-4000-8000-000000000001');
    expect(JSON.parse(socket.sent[0] ?? '')).toEqual({
      protocolVersion: 1,
      messageId: '00000000-0000-4000-8000-000000000001',
      payload: command,
    });

    const envelope: WsServerEnvelope<WatchServerEventV1> = {
      protocolVersion: 1,
      messageId: '00000000-0000-4000-8000-000000000001',
      payload: event,
    };
    socket.message(JSON.stringify(envelope));
    socket.message(JSON.stringify(envelope));
    const observationEnvelope: WsServerEnvelope<WatchServerEventV1> = {
      protocolVersion: 1,
      messageId: '00000000-0000-4000-8000-000000000004',
      payload: observation,
    };
    socket.message(JSON.stringify(observationEnvelope));
    socket.message(JSON.stringify({
      ...observationEnvelope,
      messageId: '00000000-0000-4000-8000-000000000005',
    }));
    socket.message('{bad json');
    expect(received).toEqual([envelope, observationEnvelope]);
  });

  it('does not open implicitly and fails closed without a session', () => {
    const socketFactory = vi.fn(() => new FakeSocket());
    const client = new WatchClient({
      baseUrl: 'http://127.0.0.1:1234/',
      session: () => null,
      socket: socketFactory,
    });
    expect(client.state).toBe('IDLE');
    expect(socketFactory).not.toHaveBeenCalled();
    expect(() => client.connect()).toThrow(/session is unavailable/u);
    expect(socketFactory).not.toHaveBeenCalled();
  });
});
