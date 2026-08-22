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
  it('uses the session query, accepts same-message sibling events, and rejects exact replays', () => {
    const command = readGolden<WatchClientCommandV1>('watch-client-start-v1.json');
    const event = readGolden<WatchServerEventV1>('watch-server-start-result-v1.json');
    const episode = readGolden<WatchServerEventV1>('watch-server-episode-v1.json');
    const observation = readGolden<WatchServerEventV1>('watch-server-observation-v1.json');
    if (episode.type !== 'EPISODE_UPDATED') throw new Error('episode golden has the wrong type');
    const alert: WatchServerEventV1 = {
      type: 'ALERT_UPDATED',
      alert: {
        contractVersion: 1,
        alertId: '00000000-0000-4000-8000-000000000006',
        disposition: 'OPENED',
        visible: true,
        episode: episode.episode,
      },
    };
    const audio: WatchServerEventV1 = {
      type: 'AUDIO_DISPOSITION',
      audio: {
        disposition: 'CONTINUOUS_MIXER_ACTIVE',
        episodeIds: [episode.episode.episodeId],
        emittedAt: '1970-01-01T00:00:01Z',
      },
    };
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
    const siblingEnvelopes: WsServerEnvelope<WatchServerEventV1>[] = [episode, alert, audio].map(
      (payload) => ({
        protocolVersion: 1,
        messageId: envelope.messageId,
        payload,
      }),
    );
    siblingEnvelopes.forEach((sibling) => socket.message(JSON.stringify(sibling)));
    siblingEnvelopes.forEach((sibling) => socket.message(JSON.stringify(sibling)));
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
    expect(received).toEqual([envelope, ...siblingEnvelopes, observationEnvelope]);
  });

  it('passively acknowledges server PINGs from the message handler and dedups replays', () => {
    const ping = readGolden<WatchServerEventV1>('watch-server-ping-v1.json');
    const ack = readGolden<WatchClientCommandV1>('watch-client-heartbeat-ack-v1.json');
    if (ping.type !== 'PING') throw new Error('ping golden has the wrong type');
    const socket = new FakeSocket();
    let nextId = 0;
    const client = new WatchClient({
      baseUrl: 'https://planner.invalid/',
      session: () => 'nonce',
      socket: () => socket,
      messageId: () => `00000000-0000-4000-8000-${String(++nextId).padStart(12, '0')}`,
    });
    const received: WsServerEnvelope<WatchServerEventV1>[] = [];
    client.subscribe((envelope) => received.push(envelope));
    client.connect();
    socket.open();

    const envelope: WsServerEnvelope<WatchServerEventV1> = {
      protocolVersion: 1,
      messageId: '10000000-0000-4000-8000-000000000001',
      payload: ping,
    };
    socket.message(JSON.stringify(envelope));
    expect(socket.sent).toHaveLength(1);
    expect(JSON.parse(socket.sent[0] ?? '')).toEqual({
      protocolVersion: 1,
      messageId: '00000000-0000-4000-8000-000000000001',
      payload: ack,
    });
    expect(received).toEqual([envelope]);

    socket.message(JSON.stringify(envelope));
    expect(socket.sent).toHaveLength(1);
    expect(received).toEqual([envelope]);

    // A retransmitted heartbeat carries the SAME payload under a fresh
    // envelope ID: it is a distinct heartbeat and must be acknowledged again.
    const resent: WsServerEnvelope<WatchServerEventV1> = {
      ...envelope,
      messageId: '10000000-0000-4000-8000-000000000002',
    };
    socket.message(JSON.stringify(resent));
    expect(socket.sent).toHaveLength(2);
    expect(JSON.parse(socket.sent[1] ?? '').payload).toEqual(ack);

    const next: WsServerEnvelope<WatchServerEventV1> = {
      protocolVersion: 1,
      messageId: '10000000-0000-4000-8000-000000000003',
      payload: { type: 'PING', sequence: 2 },
    };
    socket.message(JSON.stringify(next));
    expect(socket.sent).toHaveLength(3);
    expect(JSON.parse(socket.sent[2] ?? '').payload).toEqual({
      type: 'HEARTBEAT_ACK',
      sequence: 2,
    });

    socket.readyState = 3;
    expect(() => socket.message(JSON.stringify({
      ...next,
      messageId: '10000000-0000-4000-8000-000000000004',
    }))).not.toThrow();
    expect(socket.sent).toHaveLength(3);
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
