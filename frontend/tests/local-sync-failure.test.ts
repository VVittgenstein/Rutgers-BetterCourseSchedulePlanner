import { afterEach, describe, expect, it, vi } from 'vitest';

import {
  classifyLocalSyncFailure,
  createLocalPersonalSync,
  createLocalPersonalTabId,
  currentRevisionDetail,
  holdsInitialFiltersFor,
  isLocalPersonalSyncMessage,
  isSavedViewNameConflict,
  LOCAL_PERSONAL_SYNC_CHANNEL,
  localSyncFailureReason,
  localSyncMessageKey,
  type LocalPersonalSyncMessage,
  type LocalSnapshotOrigin,
  type LocalSyncStatus,
} from '../src/ui/local/personal';
import { ProductClientError, type ApiErrorEnvelope } from '../src/ui/shared/product';

const TRACE = '10000000-0000-4000-8000-000000000001';

function apiError(status: number, code: string, revision?: number): ProductClientError {
  const envelope = {
    protocolVersion: 1,
    error: {
      code,
      messageKey: `local.error.${code.toLowerCase()}`,
      traceId: TRACE,
      details: revision === undefined ? [] : [{ kind: 'CURRENT_REVISION', revision }],
    },
  } as unknown as ApiErrorEnvelope;
  return new ProductClientError(status, envelope);
}

function settle(): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, 20);
  });
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('classifyLocalSyncFailure', () => {
  it.each([
    ['SETTINGS_REVISION_CONFLICT', 'REVISION_CONFLICT'],
    ['CURRENT_FILTERS_REVISION_CONFLICT', 'REVISION_CONFLICT'],
    ['SAVED_VIEW_REVISION_CONFLICT', 'REVISION_CONFLICT'],
    ['USER_STATE_REVISION_CONFLICT', 'STATE_RESET'],
    ['SAVED_VIEW_NAME_CONFLICT', 'REJECTED'],
  ])('classifies a 409 %s as %s', (code, expected) => {
    expect(classifyLocalSyncFailure(apiError(409, code, 5))).toBe(expected);
  });

  it.each([
    [400, 'INVALID_REQUEST', 'REJECTED'],
    [404, 'SAVED_VIEW_NOT_FOUND', 'REJECTED'],
    [422, 'INVALID_FILTER', 'REJECTED'],
    [507, 'STORAGE_FULL', 'REJECTED'],
    [409, 'RESET_CONFIRMATION_EXPIRED', 'REJECTED'],
    [429, 'RATE_LIMITED', 'TRANSIENT'],
    [500, 'INTERNAL_ERROR', 'TRANSIENT'],
    [502, 'UPSTREAM_UNAVAILABLE', 'TRANSIENT'],
    [503, 'STORAGE_BUSY', 'TRANSIENT'],
    [503, 'RESET_INCOMPLETE', 'TRANSIENT'],
    [504, 'UPSTREAM_UNAVAILABLE', 'TRANSIENT'],
  ])('classifies HTTP %s %s as %s', (status, code, expected) => {
    expect(classifyLocalSyncFailure(apiError(status, code))).toBe(expected);
  });

  it('treats a 5xx without a JSON error body as transient and a 2xx envelope failure as rejected', () => {
    expect(classifyLocalSyncFailure(new ProductClientError(500, null))).toBe('TRANSIENT');
    expect(classifyLocalSyncFailure(new ProductClientError(503, null))).toBe('TRANSIENT');
    expect(classifyLocalSyncFailure(new ProductClientError(200, null))).toBe('REJECTED');
  });

  it('treats fetch network failures as transient, aborts as aborted, and everything else as rejected', () => {
    expect(classifyLocalSyncFailure(new TypeError('fetch failed'))).toBe('TRANSIENT');
    expect(classifyLocalSyncFailure(Object.assign(new Error('aborted'), { name: 'AbortError' }))).toBe('ABORTED');
    expect(classifyLocalSyncFailure(new DOMException('aborted', 'AbortError'))).toBe('ABORTED');
    expect(classifyLocalSyncFailure(new Error('The Saved view no longer exists.'))).toBe('REJECTED');
    expect(classifyLocalSyncFailure('nope')).toBe('REJECTED');
    expect(classifyLocalSyncFailure(undefined)).toBe('REJECTED');
  });

  it('maps failure kinds onto the FAILED reasons', () => {
    expect(localSyncFailureReason('TRANSIENT')).toBe('UNAVAILABLE');
    expect(localSyncFailureReason('REVISION_CONFLICT')).toBe('CONFLICT');
    expect(localSyncFailureReason('STATE_RESET')).toBe('STATE_RESET');
    expect(localSyncFailureReason('REJECTED')).toBe('REJECTED');
    expect(localSyncFailureReason('ABORTED')).toBe('REJECTED');
  });

  it('recognises the saved-view name conflict only on a 409 with that code', () => {
    expect(isSavedViewNameConflict(apiError(409, 'SAVED_VIEW_NAME_CONFLICT', 3))).toBe(true);
    expect(isSavedViewNameConflict(apiError(409, 'SAVED_VIEW_REVISION_CONFLICT', 3))).toBe(false);
    expect(isSavedViewNameConflict(apiError(422, 'SAVED_VIEW_NAME_CONFLICT'))).toBe(false);
    expect(isSavedViewNameConflict(new Error('SAVED_VIEW_NAME_CONFLICT'))).toBe(false);
  });
});

describe('currentRevisionDetail', () => {
  it('extracts the CURRENT_REVISION detail and ignores everything else', () => {
    expect(currentRevisionDetail(apiError(409, 'SETTINGS_REVISION_CONFLICT', 12))).toBe(12);
    expect(currentRevisionDetail(apiError(409, 'SETTINGS_REVISION_CONFLICT'))).toBeNull();
    expect(currentRevisionDetail(new ProductClientError(409, null))).toBeNull();
    expect(currentRevisionDetail(new Error('x'))).toBeNull();
    const mixed = {
      protocolVersion: 1,
      error: {
        code: 'SETTINGS_REVISION_CONFLICT',
        messageKey: 'x',
        traceId: TRACE,
        details: [{ kind: 'RETRY_AFTER_SECONDS', seconds: 3 }, { kind: 'CURRENT_REVISION', revision: 8 }],
      },
    } as unknown as ApiErrorEnvelope;
    expect(currentRevisionDetail(new ProductClientError(409, mixed))).toBe(8);
  });
});

describe('localSyncMessageKey', () => {
  const error = new Error('detail');
  it.each<[LocalSyncStatus, string | null]>([
    [{ phase: 'IDLE' }, null],
    [{ phase: 'SAVING' }, 'local.status.busy'],
    [{ phase: 'RETRYING', reason: 'BUSY', attempt: 1 }, 'local.sync.retrying_busy'],
    [{ phase: 'RETRYING', reason: 'CONFLICT', attempt: 1 }, 'local.sync.retrying_conflict'],
    [{ phase: 'RECOVERED', reason: 'CONFLICT' }, 'local.sync.recovered'],
    [{ phase: 'RECOVERED', reason: 'REFRESH' }, 'local.sync.recovered'],
    [{ phase: 'STALE', error }, 'local.sync.stale'],
    [{ phase: 'FAILED', reason: 'UNAVAILABLE', error }, 'local.sync.failed_unavailable'],
    [{ phase: 'FAILED', reason: 'CONFLICT', error }, 'local.sync.failed_conflict'],
    [{ phase: 'FAILED', reason: 'STATE_RESET', error }, 'local.sync.failed_reset'],
    [{ phase: 'FAILED', reason: 'REJECTED', error }, 'local.status.error'],
  ])('maps %o to %s', (status, expected) => {
    expect(localSyncMessageKey(status)).toBe(expected);
  });
});

describe('holdsInitialFiltersFor', () => {
  it('holds this tab\'s filter panel only against peer or visibility snapshots while editing', () => {
    const origins: readonly LocalSnapshotOrigin[] = ['INITIAL', 'SELF', 'PEER', 'VISIBILITY', 'RELOAD'];
    expect(origins.filter((origin) => holdsInitialFiltersFor(origin, true))).toEqual(['PEER', 'VISIBILITY']);
    expect(origins.filter((origin) => holdsInitialFiltersFor(origin, false))).toEqual([]);
  });
});

describe('createLocalPersonalSync', () => {
  it('returns a no-op port when BroadcastChannel is unavailable', () => {
    vi.stubGlobal('BroadcastChannel', undefined);
    const port = createLocalPersonalSync();
    const listener = vi.fn();
    const unsubscribe = port.subscribe(listener);
    expect(() => port.publish({ kind: 'MUTATED', tabId: 'a', at: 1 })).not.toThrow();
    unsubscribe();
    expect(() => port.dispose()).not.toThrow();
    expect(listener).not.toHaveBeenCalled();
  });

  it('returns a no-op port when the channel cannot be constructed', () => {
    vi.stubGlobal('BroadcastChannel', class {
      constructor() {
        throw new Error('restricted');
      }
    });
    const port = createLocalPersonalSync();
    const listener = vi.fn();
    port.subscribe(listener);
    expect(() => port.publish({ kind: 'MUTATED', tabId: 'a', at: 1 })).not.toThrow();
    port.dispose();
    expect(listener).not.toHaveBeenCalled();
  });

  it('delivers validated messages between ports and drops malformed ones', async () => {
    const channelName = `${LOCAL_PERSONAL_SYNC_CHANNEL}-test-${createLocalPersonalTabId()}`;
    const sender = createLocalPersonalSync(channelName);
    const receiver = createLocalPersonalSync(channelName);
    const received: LocalPersonalSyncMessage[] = [];
    const unsubscribe = receiver.subscribe((message) => received.push(message));
    const raw = new BroadcastChannel(channelName);
    try {
      raw.postMessage({ kind: 'MUTATED', tabId: '', at: 1 });
      raw.postMessage({ kind: 'MUTATED', tabId: 'x', at: Number.NaN });
      raw.postMessage({ kind: 'OTHER', tabId: 'x', at: 1 });
      raw.postMessage('MUTATED');
      raw.postMessage(null);
      sender.publish({ kind: 'MUTATED', tabId: 'peer', at: 42 });
      await settle();
      expect(received).toEqual([{ kind: 'MUTATED', tabId: 'peer', at: 42 }]);

      unsubscribe();
      sender.publish({ kind: 'MUTATED', tabId: 'peer', at: 43 });
      await settle();
      expect(received).toHaveLength(1);
    } finally {
      raw.close();
      sender.dispose();
      receiver.dispose();
    }
  });

  it('disposes idempotently and re-opens on the next subscribe', async () => {
    const channelName = `${LOCAL_PERSONAL_SYNC_CHANNEL}-test-${createLocalPersonalTabId()}`;
    const sender = createLocalPersonalSync(channelName);
    const receiver = createLocalPersonalSync(channelName);
    const listener = vi.fn();
    try {
      receiver.subscribe(listener);
      receiver.dispose();
      receiver.dispose();
      expect(() => receiver.dispose()).not.toThrow();
      sender.publish({ kind: 'MUTATED', tabId: 'peer', at: 1 });
      await settle();
      expect(listener).not.toHaveBeenCalled();

      // StrictMode re-runs the subscribe effect after its cleanup disposed the port.
      receiver.subscribe(listener);
      sender.publish({ kind: 'MUTATED', tabId: 'peer', at: 2 });
      await settle();
      expect(listener).toHaveBeenCalledTimes(1);
      expect(listener).toHaveBeenCalledWith({ kind: 'MUTATED', tabId: 'peer', at: 2 });
    } finally {
      sender.dispose();
      receiver.dispose();
    }
  });

  it('validates the message shape and mints distinct tab ids', () => {
    expect(isLocalPersonalSyncMessage({ kind: 'MUTATED', tabId: 'a', at: 0 })).toBe(true);
    expect(isLocalPersonalSyncMessage({ kind: 'MUTATED', tabId: 'a', at: Number.POSITIVE_INFINITY })).toBe(false);
    expect(isLocalPersonalSyncMessage({ kind: 'MUTATED', tabId: 1, at: 0 })).toBe(false);
    expect(isLocalPersonalSyncMessage([])).toBe(false);
    expect(createLocalPersonalTabId()).not.toBe(createLocalPersonalTabId());
  });
});
