/**
 * Cross-tab freshness for the Windows-local personal state.
 *
 * Every provider publishes a MUTATED message after it commits a write; other
 * tabs refresh their bootstrap from it. The port wraps one BroadcastChannel so
 * tests can inject a fake, restricted contexts degrade to a no-op, and messages
 * from other tabs are validated before anybody acts on them.
 */

export interface LocalPersonalSyncMessage {
  readonly kind: 'MUTATED';
  readonly tabId: string;
  readonly at: number;
}

export type LocalPersonalSyncListener = (message: LocalPersonalSyncMessage) => void;

export interface LocalPersonalSyncPort {
  publish(message: LocalPersonalSyncMessage): void;
  subscribe(listener: LocalPersonalSyncListener): () => void;
  /** Idempotent. The port re-opens its channel on the next publish/subscribe. */
  dispose(): void;
}

export const LOCAL_PERSONAL_SYNC_CHANNEL = 'rbcsp-local-personal';

interface BroadcastChannelLike {
  postMessage(message: unknown): void;
  close(): void;
  onmessage: ((event: { readonly data: unknown }) => void) | null;
  unref?: () => void;
}

type BroadcastChannelConstructor = new (name: string) => BroadcastChannelLike;

/**
 * Feature detection without naming the callable type: a present global is
 * treated as a constructor and `open()` below tolerates a throwing `new`.
 */
function broadcastChannelConstructor(): BroadcastChannelConstructor | null {
  const candidate = (globalThis as { readonly BroadcastChannel?: unknown }).BroadcastChannel;
  if (candidate === undefined || candidate === null) return null;
  return candidate as BroadcastChannelConstructor;
}

export function isLocalPersonalSyncMessage(value: unknown): value is LocalPersonalSyncMessage {
  if (typeof value !== 'object' || value === null) return false;
  const record = value as { readonly kind?: unknown; readonly tabId?: unknown; readonly at?: unknown };
  return record.kind === 'MUTATED'
    && typeof record.tabId === 'string'
    && record.tabId.length > 0
    && typeof record.at === 'number'
    && Number.isFinite(record.at);
}

export function createLocalPersonalTabId(): string {
  const cryptoApi = (globalThis as { readonly crypto?: { readonly randomUUID?: () => string } }).crypto;
  try {
    const id = cryptoApi?.randomUUID?.();
    if (id !== undefined && id.length > 0) return id;
  } catch {
    // Fall through to the weaker identity below.
  }
  return `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
}

const NOOP_PORT: LocalPersonalSyncPort = {
  publish() {},
  subscribe() {
    return () => {};
  },
  dispose() {},
};

export function createLocalPersonalSync(
  channelName: string = LOCAL_PERSONAL_SYNC_CHANNEL,
): LocalPersonalSyncPort {
  const Channel = broadcastChannelConstructor();
  if (Channel === null) return NOOP_PORT;

  const listeners = new Set<LocalPersonalSyncListener>();
  let channel: BroadcastChannelLike | null = null;
  let unusable = false;

  const dispatch = (event: { readonly data: unknown }) => {
    const message = event.data;
    if (!isLocalPersonalSyncMessage(message)) return;
    const frozen: LocalPersonalSyncMessage = { kind: 'MUTATED', tabId: message.tabId, at: message.at };
    for (const listener of [...listeners]) listener(frozen);
  };

  const open = (): BroadcastChannelLike | null => {
    if (channel !== null || unusable) return channel;
    try {
      channel = new Channel(channelName);
      channel.onmessage = dispatch;
      // Node's BroadcastChannel would otherwise keep a test process alive.
      channel.unref?.();
    } catch {
      unusable = true;
      channel = null;
    }
    return channel;
  };

  return {
    publish(message) {
      const target = open();
      if (target === null) return;
      try {
        target.postMessage(message);
      } catch {
        // A closed or restricted channel must never break the mutation that published.
      }
    },
    subscribe(listener) {
      listeners.add(listener);
      open();
      return () => {
        listeners.delete(listener);
      };
    },
    dispose() {
      const target = channel;
      channel = null;
      if (target === null) return;
      target.onmessage = null;
      try {
        target.close();
      } catch {
        // Already closed.
      }
    },
  };
}
