import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from 'react';

import type { FilterRequestV3, SectionKey, TraceId } from '../../shared/product';
import type { LocalPersonalApiPort } from './LocalPersonalApi';
import {
  createLocalPersonalSync,
  createLocalPersonalTabId,
  type LocalPersonalSyncPort,
} from './LocalPersonalSync';
import type {
  LocalBootstrapData,
  LocalSettings,
  LocalTermPullResponse,
  PersonalResetResult,
  PersonalStateSnapshot,
  PreparedUserDataReset,
  SavedViewDefinition,
  SavedViewDeleteResult,
  SavedViewLibrary,
  SavedViewMutation,
  SavedViewsDeleteAllResult,
  StoredCurrentFilters,
  StoredSettings,
} from './contracts';
import {
  classifyLocalSyncFailure,
  isSavedViewNameConflict,
  LOCAL_SYNC_IDLE,
  localSyncFailureReason,
  type LocalSnapshotOrigin,
  type LocalSyncStatus,
} from './syncFailure';

/** Backoff before a mutation that hit a busy/rebuilding service is replayed. */
export const LOCAL_PERSONAL_RETRY_DELAYS_MS: readonly number[] = [300, 900];
/** Backoff for the bootstrap refresh that follows a committed mutation or a manual reload. */
export const LOCAL_PERSONAL_REFRESH_RETRY_DELAYS_MS: readonly number[] = [300, 900];
/** Background schedule once a committed mutation could not be followed by a refresh. */
export const LOCAL_PERSONAL_BACKGROUND_REFRESH_DELAYS_MS: readonly number[] = [2_000, 5_000, 15_000];
export const LOCAL_PERSONAL_PEER_REFRESH_DEBOUNCE_MS = 250;
export const LOCAL_PERSONAL_PEER_REFRESH_MIN_INTERVAL_MS = 1_000;
/** A hidden tab that becomes visible refreshes when its snapshot is older than this. */
export const LOCAL_PERSONAL_VISIBILITY_STALE_MS = 5_000;
export const LOCAL_PERSONAL_RECOVERED_NOTICE_MS = 4_000;

function unavailable(): never {
  throw new Error('The local personal-state provider is missing.');
}

function noop(): void {}

function sleep(milliseconds: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, milliseconds);
  });
}

function toError(caught: unknown, fallback: string): Error {
  return caught instanceof Error ? caught : new Error(fallback);
}

function initialLibrary(bootstrap: LocalBootstrapData): SavedViewLibrary {
  const currentFilters = bootstrap.state.currentFilters;
  return {
    stateRevision: bootstrap.state.stateRevision,
    currentFilters,
    views: bootstrap.state.savedViews.map((definition) => ({
      definition,
      matchState: definition.content.status === 'INCOMPATIBLE'
        ? 'INCOMPATIBLE'
        : definition.content.status === 'REVIEW_REQUIRED'
          ? 'REVIEW_REQUIRED'
        : currentFilters.value?.association.kind === 'APPLIED'
          && currentFilters.value.association.viewId === definition.id
          ? currentFilters.value.association.revision === definition.revision ? 'CLEAN' : 'MODIFIED'
          : 'NOT_APPLIED',
    })),
  };
}

function namedView(snapshot: PersonalStateSnapshot, id: TraceId): SavedViewDefinition {
  const view = snapshot.savedViews.find((candidate) => candidate.id === id);
  if (view === undefined) throw new Error('The Saved view no longer exists.');
  return view;
}

function sameSections(left: readonly SectionKey[], right: readonly SectionKey[]): boolean {
  return left.length === right.length && left.every((section, index) => {
    const other = right[index];
    return other !== undefined
      && other.term === section.term
      && other.campus === section.campus
      && other.index === section.index;
  });
}

function sameSavedViews(
  left: readonly SavedViewDefinition[],
  right: readonly SavedViewDefinition[],
): boolean {
  return left.length === right.length && left.every((view, index) => {
    const other = right[index];
    return other !== undefined
      && other.id === view.id
      && other.revision === view.revision
      && other.name === view.name
      && other.updatedAt === view.updatedAt;
  });
}

/**
 * Merges an incoming snapshot into the one held so far. Returns null when the
 * incoming snapshot is older than what this tab already holds (a lower user
 * state revision, which only a full reset ever raises). Sub-states whose
 * revision did not move keep their object identity so unsaved drafts keyed on
 * them survive background refreshes.
 */
export function mergeBootstrap(
  previous: LocalBootstrapData,
  next: LocalBootstrapData,
): LocalBootstrapData | null {
  const before = previous.state;
  const after = next.state;
  if (after.stateRevision < before.stateRevision) return null;
  if (after.stateRevision > before.stateRevision) return next;
  const settings = after.settings.revision > before.settings.revision ? after.settings : before.settings;
  const currentFilters = after.currentFilters.revision > before.currentFilters.revision
    ? after.currentFilters
    : before.currentFilters;
  const savedViews = sameSavedViews(before.savedViews, after.savedViews)
    ? before.savedViews
    : after.savedViews;
  const selectedSections = sameSections(before.selectedSections, after.selectedSections)
    ? before.selectedSections
    : after.selectedSections;
  return {
    ...next,
    state: { ...after, settings, currentFilters, savedViews, selectedSections },
  };
}

function upsertView(
  views: readonly SavedViewDefinition[],
  definition: SavedViewDefinition,
): readonly SavedViewDefinition[] {
  const index = views.findIndex((view) => view.id === definition.id);
  if (index === -1) return [...views, definition];
  return views.map((view, position) => (position === index ? definition : view));
}

function withSettings(snapshot: PersonalStateSnapshot, settings: StoredSettings): PersonalStateSnapshot {
  return { ...snapshot, settings };
}

function withCurrentFilters(
  snapshot: PersonalStateSnapshot,
  currentFilters: StoredCurrentFilters,
): PersonalStateSnapshot {
  return { ...snapshot, currentFilters };
}

function withSavedViewMutation(
  snapshot: PersonalStateSnapshot,
  result: SavedViewMutation,
): PersonalStateSnapshot {
  return {
    ...snapshot,
    currentFilters: result.currentFilters,
    savedViews: upsertView(snapshot.savedViews, result.definition),
  };
}

function withSavedViewDeleted(
  snapshot: PersonalStateSnapshot,
  result: SavedViewDeleteResult,
): PersonalStateSnapshot {
  return {
    ...snapshot,
    currentFilters: result.currentFilters,
    savedViews: snapshot.savedViews.filter((view) => view.id !== result.deletedId),
  };
}

function withSavedViewsCleared(
  snapshot: PersonalStateSnapshot,
  result: SavedViewsDeleteAllResult,
): PersonalStateSnapshot {
  return { ...snapshot, currentFilters: result.currentFilters, savedViews: [] };
}

function withSelection(
  snapshot: PersonalStateSnapshot,
  selectedSections: readonly SectionKey[],
): PersonalStateSnapshot {
  return { ...snapshot, selectedSections };
}

interface MutationPolicyBase {
  /** Replay once against a fresh snapshot after a sub-revision 409. */
  readonly conflictRetry: boolean;
  /** Replay with backoff after a busy/rebuilding/unreachable service. */
  readonly transientRetry: boolean;
  /** Show the RECOVERED notice after a conflict replay (user-initiated actions only). */
  readonly announceRecovery: boolean;
}

interface MutationPolicy<T> extends MutationPolicyBase {
  /** Merge the response into the held snapshot before the follow-up refresh. */
  readonly apply?: (snapshot: PersonalStateSnapshot, result: T) => PersonalStateSnapshot;
  /**
   * After a transient replay, a SAVED_VIEW_NAME_CONFLICT can mean the first
   * attempt actually committed. When the fresh library satisfies this predicate
   * the mutation is treated as a success.
   */
  readonly settledByNameConflict?: (snapshot: PersonalStateSnapshot) => boolean;
}

const USER_WRITE: MutationPolicyBase = {
  conflictRetry: true,
  transientRetry: true,
  announceRecovery: true,
};

const BACKGROUND_WRITE: MutationPolicyBase = {
  conflictRetry: true,
  transientRetry: true,
  announceRecovery: false,
};

const DESTRUCTIVE: MutationPolicyBase = {
  conflictRetry: false,
  transientRetry: true,
  announceRecovery: false,
};

const SINGLE_USE: MutationPolicyBase = {
  conflictRetry: false,
  transientRetry: false,
  announceRecovery: false,
};

export interface LocalPersonalContextValue {
  readonly bootstrap: LocalBootstrapData;
  readonly state: PersonalStateSnapshot;
  readonly savedViews: SavedViewLibrary;
  readonly busy: boolean;
  readonly reloading: boolean;
  /** True while any bootstrap refresh (post-mutation, background, peer, visibility, reload) is in flight. */
  readonly refreshing: boolean;
  readonly sync: LocalSyncStatus;
  /** Where the snapshot currently exposed as `state` came from. */
  readonly snapshotOrigin: LocalSnapshotOrigin;
  readonly error: Error | null;
  readonly clearError: () => void;
  readonly reload: () => Promise<void>;
  readonly updateSettings: (settings: LocalSettings) => Promise<void>;
  readonly replaceSelection: (sections: readonly SectionKey[]) => Promise<void>;
  readonly replaceCurrentFilters: (filters: FilterRequestV3) => Promise<void>;
  readonly createSavedView: (name: string) => Promise<void>;
  readonly applySavedView: (id: TraceId) => Promise<void>;
  readonly renameSavedView: (id: TraceId, name: string) => Promise<void>;
  readonly updateSavedView: (id: TraceId, filters: FilterRequestV3) => Promise<void>;
  readonly duplicateSavedView: (id: TraceId, name: string) => Promise<void>;
  readonly deleteSavedView: (id: TraceId) => Promise<void>;
  readonly deleteAllSavedViews: () => Promise<void>;
  readonly resetCurrentFilters: () => Promise<void>;
  readonly prepareUserDataReset: () => Promise<PreparedUserDataReset>;
  readonly confirmUserDataReset: (token: TraceId) => Promise<PersonalResetResult>;
  readonly pullTerm: (term: string) => Promise<LocalTermPullResponse>;
}

const LocalPersonalContext = createContext<LocalPersonalContextValue | null>(null);

export interface LocalPersonalProviderProps {
  readonly api: LocalPersonalApiPort;
  readonly children: ReactNode;
  readonly initialBootstrap: LocalBootstrapData;
  /** Cross-tab port. Created lazily (once per provider) when omitted. */
  readonly sync?: LocalPersonalSyncPort | undefined;
  readonly retryDelaysMs?: readonly number[] | undefined;
  readonly refreshRetryDelaysMs?: readonly number[] | undefined;
  readonly backgroundRefreshDelaysMs?: readonly number[] | undefined;
  readonly peerRefreshDebounceMs?: number | undefined;
  readonly peerRefreshMinIntervalMs?: number | undefined;
  readonly recoveredNoticeMs?: number | undefined;
  readonly now?: (() => number) | undefined;
}

function documentIsHidden(): boolean {
  return globalThis.document?.visibilityState === 'hidden';
}

export function LocalPersonalProvider({
  api,
  children,
  initialBootstrap,
  sync,
  retryDelaysMs = LOCAL_PERSONAL_RETRY_DELAYS_MS,
  refreshRetryDelaysMs = LOCAL_PERSONAL_REFRESH_RETRY_DELAYS_MS,
  backgroundRefreshDelaysMs = LOCAL_PERSONAL_BACKGROUND_REFRESH_DELAYS_MS,
  peerRefreshDebounceMs = LOCAL_PERSONAL_PEER_REFRESH_DEBOUNCE_MS,
  peerRefreshMinIntervalMs = LOCAL_PERSONAL_PEER_REFRESH_MIN_INTERVAL_MS,
  recoveredNoticeMs = LOCAL_PERSONAL_RECOVERED_NOTICE_MS,
  now = Date.now,
}: LocalPersonalProviderProps) {
  const [bootstrap, setBootstrap] = useState(initialBootstrap);
  const [savedViews, setSavedViews] = useState(() => initialLibrary(initialBootstrap));
  const [snapshotOrigin, setSnapshotOrigin] = useState<LocalSnapshotOrigin>('INITIAL');
  const [pendingCount, setPendingCount] = useState(0);
  const [refreshCount, setRefreshCount] = useState(0);
  const [reloading, setReloading] = useState(false);
  const [syncStatus, setSyncStatusState] = useState<LocalSyncStatus>(LOCAL_SYNC_IDLE);
  const [syncPort] = useState(() => sync ?? createLocalPersonalSync());
  const [tabId] = useState(createLocalPersonalTabId);

  const bootstrapRef = useRef(initialBootstrap);
  const queueRef = useRef<Promise<void>>(Promise.resolve());
  const syncStatusRef = useRef<LocalSyncStatus>(LOCAL_SYNC_IDLE);
  const mountedRef = useRef(false);
  const recoveredTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const backgroundTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const backgroundGenerationRef = useRef(0);
  const peerTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const refreshRequestedRef = useRef(false);
  const peerDirtyRef = useRef(false);
  const lastRefreshAtRef = useRef(now());
  const lastPeerRefreshAtRef = useRef(0);
  const settingsRef = useRef({
    api,
    retryDelaysMs,
    refreshRetryDelaysMs,
    backgroundRefreshDelaysMs,
    peerRefreshDebounceMs,
    peerRefreshMinIntervalMs,
    recoveredNoticeMs,
    now,
  });
  settingsRef.current = {
    api,
    retryDelaysMs,
    refreshRetryDelaysMs,
    backgroundRefreshDelaysMs,
    peerRefreshDebounceMs,
    peerRefreshMinIntervalMs,
    recoveredNoticeMs,
    now,
  };

  const setSyncStatus = useCallback((next: LocalSyncStatus) => {
    syncStatusRef.current = next;
    setSyncStatusState(next);
  }, []);

  const clearRecoveredTimer = useCallback(() => {
    if (recoveredTimerRef.current !== null) {
      clearTimeout(recoveredTimerRef.current);
      recoveredTimerRef.current = null;
    }
  }, []);

  const announce = useCallback((next: LocalSyncStatus) => {
    clearRecoveredTimer();
    setSyncStatus(next);
    if (next.phase !== 'RECOVERED') return;
    recoveredTimerRef.current = setTimeout(() => {
      recoveredTimerRef.current = null;
      // Only clear the notice this timer was armed for; a later phase wins.
      if (syncStatusRef.current === next) setSyncStatus(LOCAL_SYNC_IDLE);
    }, settingsRef.current.recoveredNoticeMs);
  }, [clearRecoveredTimer, setSyncStatus]);

  const cancelBackgroundRefresh = useCallback(() => {
    backgroundGenerationRef.current += 1;
    if (backgroundTimerRef.current !== null) {
      clearTimeout(backgroundTimerRef.current);
      backgroundTimerRef.current = null;
    }
  }, []);

  /** Every request that reads or writes the snapshot runs as one link of this chain. */
  const chain = useCallback(<T,>(task: () => Promise<T>): Promise<T> => {
    const run = queueRef.current.then(task);
    queueRef.current = run.then(noop, noop);
    return run;
  }, []);

  const replaceBootstrap = useCallback((next: LocalBootstrapData, origin: LocalSnapshotOrigin) => {
    const merged = mergeBootstrap(bootstrapRef.current, next);
    if (merged === null) return;
    bootstrapRef.current = merged;
    setBootstrap(merged);
    setSavedViews(initialLibrary(merged));
    setSnapshotOrigin(origin);
  }, []);

  const patchSnapshot = useCallback((
    patch: (snapshot: PersonalStateSnapshot) => PersonalStateSnapshot,
  ) => {
    const current = bootstrapRef.current;
    replaceBootstrap({ ...current, state: patch(current.state) }, 'SELF');
  }, [replaceBootstrap]);

  /**
   * One bootstrap GET (plus the optional exact-match library) with transient
   * backoff. Must be called from inside the chain so it can never overlap a
   * mutation and land a pre-mutation snapshot after a newer write.
   */
  const performRefresh = useCallback(async (
    origin: LocalSnapshotOrigin,
    delays: readonly number[],
  ): Promise<void> => {
    const { api: port, now: clock } = settingsRef.current;
    setRefreshCount((count) => count + 1);
    try {
      let attempt = 0;
      for (;;) {
        try {
          const next = await port.bootstrap();
          replaceBootstrap(next, origin);
          lastRefreshAtRef.current = clock();
          peerDirtyRef.current = false;
          cancelBackgroundRefresh();
          break;
        } catch (caught) {
          const delay = delays[attempt];
          if (classifyLocalSyncFailure(caught) === 'TRANSIENT' && delay !== undefined) {
            attempt += 1;
            await sleep(delay);
            continue;
          }
          throw caught;
        }
      }
      try {
        const library = await port.savedViews();
        if (library.stateRevision === bootstrapRef.current.state.stateRevision) setSavedViews(library);
      } catch {
        // Bootstrap already contains the complete library; exact match labels are optional.
      }
    } finally {
      setRefreshCount((count) => Math.max(0, count - 1));
    }
  }, [cancelBackgroundRefresh, replaceBootstrap]);

  const scheduleBackgroundRefresh = useCallback(() => {
    cancelBackgroundRefresh();
    const generation = backgroundGenerationRef.current;
    const step = (index: number) => {
      const delay = settingsRef.current.backgroundRefreshDelaysMs[index];
      if (delay === undefined) return; // Stay STALE; the banner offers a manual reload.
      backgroundTimerRef.current = setTimeout(() => {
        backgroundTimerRef.current = null;
        void chain(async () => {
          if (generation !== backgroundGenerationRef.current || !mountedRef.current) return;
          try {
            await performRefresh('SELF', []);
            if (syncStatusRef.current.phase === 'STALE') announce({ phase: 'RECOVERED', reason: 'REFRESH' });
          } catch {
            if (generation === backgroundGenerationRef.current) step(index + 1);
          }
        });
      }, delay);
    };
    step(0);
  }, [announce, cancelBackgroundRefresh, chain, performRefresh]);

  /** Debounced, coalesced refresh for peer/visibility/online signals; hidden tabs defer. */
  const requestRefresh = useCallback((origin: 'PEER' | 'VISIBILITY') => {
    if (!mountedRef.current) return;
    if (documentIsHidden()) {
      peerDirtyRef.current = true;
      return;
    }
    if (refreshRequestedRef.current) return;
    refreshRequestedRef.current = true;
    const { now: clock, peerRefreshDebounceMs: debounce, peerRefreshMinIntervalMs: minimum } =
      settingsRef.current;
    const elapsed = clock() - lastPeerRefreshAtRef.current;
    const delay = Math.max(debounce, minimum - elapsed, 0);
    peerTimerRef.current = setTimeout(() => {
      peerTimerRef.current = null;
      void chain(async () => {
        refreshRequestedRef.current = false;
        if (!mountedRef.current) return;
        lastPeerRefreshAtRef.current = settingsRef.current.now();
        try {
          await performRefresh(origin, settingsRef.current.refreshRetryDelaysMs);
        } catch {
          // Nothing was written; the next signal or a mutation refresh will catch up.
        }
      });
    }, delay);
  }, [chain, performRefresh]);

  useEffect(() => {
    let active = true;
    void api.savedViews().then((library) => {
      if (active && library.stateRevision === bootstrapRef.current.state.stateRevision) {
        setSavedViews(library);
      }
    }).catch(() => undefined);
    return () => { active = false; };
  }, [api]);

  useEffect(() => {
    mountedRef.current = true;
    const unsubscribe = syncPort.subscribe((message) => {
      if (message.tabId === tabId) return;
      peerDirtyRef.current = true;
      requestRefresh('PEER');
    });
    const onVisibilityChange = () => {
      if (documentIsHidden()) return;
      const { now: clock } = settingsRef.current;
      if (peerDirtyRef.current || clock() - lastRefreshAtRef.current > LOCAL_PERSONAL_VISIBILITY_STALE_MS) {
        requestRefresh('VISIBILITY');
      }
    };
    const onOnline = () => requestRefresh('VISIBILITY');
    globalThis.document?.addEventListener('visibilitychange', onVisibilityChange);
    globalThis.addEventListener?.('online', onOnline);
    return () => {
      mountedRef.current = false;
      globalThis.document?.removeEventListener('visibilitychange', onVisibilityChange);
      globalThis.removeEventListener?.('online', onOnline);
      unsubscribe();
      syncPort.dispose();
      if (peerTimerRef.current !== null) {
        clearTimeout(peerTimerRef.current);
        peerTimerRef.current = null;
      }
      refreshRequestedRef.current = false;
      if (backgroundTimerRef.current !== null) {
        clearTimeout(backgroundTimerRef.current);
        backgroundTimerRef.current = null;
      }
      if (recoveredTimerRef.current !== null) {
        clearTimeout(recoveredTimerRef.current);
        recoveredTimerRef.current = null;
      }
    };
  }, [requestRefresh, syncPort, tabId]);

  const reload = useCallback(async () => {
    setReloading(true);
    announce(LOCAL_SYNC_IDLE);
    try {
      await chain(async () => {
        try {
          await performRefresh('RELOAD', settingsRef.current.refreshRetryDelaysMs);
        } catch (caught) {
          const kind = classifyLocalSyncFailure(caught);
          const error = toError(caught, 'Local state reload failed.');
          if (kind !== 'ABORTED') {
            announce({ phase: 'FAILED', reason: localSyncFailureReason(kind), error });
          }
          throw error;
        }
      });
    } finally {
      setReloading(false);
    }
  }, [announce, chain, performRefresh]);

  const enqueue = useCallback(<T,>(
    operation: (snapshot: PersonalStateSnapshot) => Promise<T>,
    policy: MutationPolicy<T>,
  ): Promise<T> => {
    setPendingCount((count) => count + 1);
    return chain(async () => {
      try {
        announce(LOCAL_SYNC_IDLE);
        let transientAttempt = 0;
        let conflictRetried = false;
        let settledWithoutResult = false;
        let result: T | undefined;
        for (;;) {
          try {
            result = await operation(bootstrapRef.current.state);
            break;
          } catch (caught) {
            const kind = classifyLocalSyncFailure(caught);
            if (kind === 'ABORTED') throw caught;
            const retryDelay = settingsRef.current.retryDelaysMs[transientAttempt];
            if (kind === 'TRANSIENT' && policy.transientRetry && retryDelay !== undefined) {
              transientAttempt += 1;
              announce({ phase: 'RETRYING', reason: 'BUSY', attempt: transientAttempt });
              await sleep(retryDelay);
              continue;
            }
            if (kind === 'REVISION_CONFLICT' && policy.conflictRetry && !conflictRetried) {
              conflictRetried = true;
              announce({ phase: 'RETRYING', reason: 'CONFLICT', attempt: 1 });
              try {
                await performRefresh('SELF', settingsRef.current.refreshRetryDelaysMs);
                continue; // The operation re-reads bootstrapRef, so it replays with fresh revisions.
              } catch {
                // Surface the original conflict below.
              }
            }
            if (
              transientAttempt > 0
              && policy.settledByNameConflict !== undefined
              && isSavedViewNameConflict(caught)
            ) {
              // The first attempt may have committed before its response was lost.
              try {
                await performRefresh('SELF', settingsRef.current.refreshRetryDelaysMs);
              } catch {
                // Cannot verify; fall through and surface the rejection.
              }
              if (policy.settledByNameConflict(bootstrapRef.current.state)) {
                settledWithoutResult = true;
                break;
              }
            }
            const error = toError(caught, 'Local state mutation failed.');
            announce({ phase: 'FAILED', reason: localSyncFailureReason(kind), error });
            throw error;
          }
        }

        const publish = () => {
          if (!mountedRef.current) return;
          syncPort.publish({ kind: 'MUTATED', tabId, at: settingsRef.current.now() });
        };

        if (settledWithoutResult) {
          publish();
          announce(LOCAL_SYNC_IDLE);
          return result as T;
        }
        const committed = result as T;
        if (policy.apply !== undefined) {
          const apply = policy.apply;
          patchSnapshot((snapshot) => apply(snapshot, committed));
        }
        publish();
        try {
          await performRefresh('SELF', settingsRef.current.refreshRetryDelaysMs);
        } catch (caught) {
          // The mutation committed; only the follow-up read failed.
          announce({ phase: 'STALE', error: toError(caught, 'Local state refresh failed.') });
          scheduleBackgroundRefresh();
          return committed;
        }
        announce(conflictRetried && policy.announceRecovery
          ? { phase: 'RECOVERED', reason: 'CONFLICT' }
          : LOCAL_SYNC_IDLE);
        return committed;
      } finally {
        setPendingCount((count) => Math.max(0, count - 1));
      }
    });
  }, [announce, chain, patchSnapshot, performRefresh, scheduleBackgroundRefresh, syncPort, tabId]);

  const exposedSync = useMemo<LocalSyncStatus>(() => (
    syncStatus.phase === 'IDLE' && (pendingCount > 0 || reloading) ? { phase: 'SAVING' } : syncStatus
  ), [pendingCount, reloading, syncStatus]);

  const error = syncStatus.phase === 'FAILED' ? syncStatus.error : null;

  const value = useMemo<LocalPersonalContextValue>(() => ({
    bootstrap,
    state: bootstrap.state,
    savedViews,
    busy: pendingCount > 0,
    reloading,
    refreshing: refreshCount > 0,
    sync: exposedSync,
    snapshotOrigin,
    error,
    clearError: () => announce(LOCAL_SYNC_IDLE),
    reload,
    updateSettings: async (settings) => {
      await enqueue(async (snapshot) => api.updateSettings({
        expectedUserStateRevision: snapshot.stateRevision,
        expectedRevision: snapshot.settings.revision,
        value: settings,
      }), { ...USER_WRITE, apply: withSettings });
    },
    replaceSelection: async (sections) => {
      await enqueue(async (snapshot) => api.replaceSelection({
        expectedUserStateRevision: snapshot.stateRevision,
        sections,
      }), { ...BACKGROUND_WRITE, apply: withSelection });
    },
    replaceCurrentFilters: async (filters) => {
      await enqueue(async (snapshot) => api.replaceCurrentFilters({
        expectedUserStateRevision: snapshot.stateRevision,
        expectedCurrentFiltersRevision: snapshot.currentFilters.revision,
        filters,
      }), { ...BACKGROUND_WRITE, apply: withCurrentFilters });
    },
    createSavedView: async (name) => {
      await enqueue(async (snapshot) => {
        const content = snapshot.currentFilters.value?.content;
        if (content?.status !== 'COMPATIBLE') throw new Error('Current filters are not compatible.');
        return api.createSavedView({
          expectedUserStateRevision: snapshot.stateRevision,
          expectedCurrentFiltersRevision: snapshot.currentFilters.revision,
          filters: content.filters,
          name,
        });
      }, {
        ...USER_WRITE,
        apply: withSavedViewMutation,
        settledByNameConflict: (snapshot) => snapshot.savedViews.some((view) => view.name === name),
      });
    },
    applySavedView: async (id) => {
      await enqueue(async (snapshot) => {
        const view = namedView(snapshot, id);
        return api.applySavedView({
          expectedUserStateRevision: snapshot.stateRevision,
          expectedCurrentFiltersRevision: snapshot.currentFilters.revision,
          expectedViewRevision: view.revision,
          id,
        });
      }, { ...USER_WRITE, apply: withCurrentFilters });
    },
    renameSavedView: async (id, name) => {
      await enqueue(async (snapshot) => {
        const view = namedView(snapshot, id);
        return api.renameSavedView({
          expectedUserStateRevision: snapshot.stateRevision,
          expectedCurrentFiltersRevision: snapshot.currentFilters.revision,
          expectedViewRevision: view.revision,
          id,
          name,
        });
      }, { ...USER_WRITE, apply: withSavedViewMutation });
    },
    updateSavedView: async (id, filters) => {
      await enqueue(async (snapshot) => {
        const view = namedView(snapshot, id);
        return api.updateSavedView({
          expectedUserStateRevision: snapshot.stateRevision,
          expectedCurrentFiltersRevision: snapshot.currentFilters.revision,
          expectedViewRevision: view.revision,
          filters,
          id,
        });
      }, { ...USER_WRITE, apply: withSavedViewMutation });
    },
    duplicateSavedView: async (id, name) => {
      await enqueue(async (snapshot) => {
        const view = namedView(snapshot, id);
        return api.duplicateSavedView({
          expectedUserStateRevision: snapshot.stateRevision,
          expectedViewRevision: view.revision,
          id,
          name,
        });
      }, {
        ...USER_WRITE,
        apply: withSavedViewMutation,
        settledByNameConflict: (snapshot) => snapshot.savedViews.some((view) => view.name === name),
      });
    },
    deleteSavedView: async (id) => {
      await enqueue(async (snapshot) => {
        const view = namedView(snapshot, id);
        return api.deleteSavedView({
          expectedUserStateRevision: snapshot.stateRevision,
          expectedCurrentFiltersRevision: snapshot.currentFilters.revision,
          expectedViewRevision: view.revision,
          id,
        });
      }, { ...DESTRUCTIVE, apply: withSavedViewDeleted });
    },
    deleteAllSavedViews: async () => {
      await enqueue(async (snapshot) => api.deleteAllSavedViews({
        expectedUserStateRevision: snapshot.stateRevision,
        expectedCurrentFiltersRevision: snapshot.currentFilters.revision,
      }), { ...DESTRUCTIVE, apply: withSavedViewsCleared });
    },
    resetCurrentFilters: async () => {
      await enqueue(async (snapshot) => api.resetCurrentFilters({
        expectedUserStateRevision: snapshot.stateRevision,
        expectedCurrentFiltersRevision: snapshot.currentFilters.revision,
      }), { ...DESTRUCTIVE, apply: withCurrentFilters });
    },
    prepareUserDataReset: () => enqueue(
      async (snapshot) => api.prepareUserDataReset(snapshot.stateRevision),
      DESTRUCTIVE,
    ),
    confirmUserDataReset: (token) => enqueue(async () => api.confirmUserDataReset(token), SINGLE_USE),
    pullTerm: async (term) => {
      announce(LOCAL_SYNC_IDLE);
      try {
        return await api.pullTerm({ contractVersion: 1, term });
      } catch (caught) {
        const next = toError(caught, 'Local term pull failed.');
        const kind = classifyLocalSyncFailure(caught);
        if (kind !== 'ABORTED') {
          announce({ phase: 'FAILED', reason: localSyncFailureReason(kind), error: next });
        }
        throw next;
      }
    },
  }), [
    announce,
    api,
    bootstrap,
    enqueue,
    error,
    exposedSync,
    pendingCount,
    refreshCount,
    reload,
    reloading,
    savedViews,
    snapshotOrigin,
  ]);

  return <LocalPersonalContext.Provider value={value}>{children}</LocalPersonalContext.Provider>;
}

export function useLocalPersonal(): LocalPersonalContextValue {
  return useContext(LocalPersonalContext) ?? unavailable();
}

export function useLocalPersonalOptional(): LocalPersonalContextValue | null {
  return useContext(LocalPersonalContext);
}
