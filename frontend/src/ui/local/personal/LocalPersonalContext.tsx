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

import type { FilterRequestV2, SectionKey, TraceId } from '../../shared/product';
import type { LocalPersonalApiPort } from './LocalPersonalApi';
import type {
  LocalBootstrapData,
  LocalSettings,
  LocalTermPullResponse,
  PersonalResetResult,
  PersonalStateSnapshot,
  PreparedUserDataReset,
  SavedViewDefinition,
  SavedViewLibrary,
} from './contracts';

function unavailable(): never {
  throw new Error('The local personal-state provider is missing.');
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

export interface LocalPersonalContextValue {
  readonly bootstrap: LocalBootstrapData;
  readonly state: PersonalStateSnapshot;
  readonly savedViews: SavedViewLibrary;
  readonly busy: boolean;
  readonly reloading: boolean;
  readonly error: Error | null;
  readonly clearError: () => void;
  readonly reload: () => Promise<void>;
  readonly updateSettings: (settings: LocalSettings) => Promise<void>;
  readonly replaceSelection: (sections: readonly SectionKey[]) => Promise<void>;
  readonly replaceCurrentFilters: (filters: FilterRequestV2) => Promise<void>;
  readonly createSavedView: (name: string) => Promise<void>;
  readonly applySavedView: (id: TraceId) => Promise<void>;
  readonly renameSavedView: (id: TraceId, name: string) => Promise<void>;
  readonly updateSavedView: (id: TraceId, filters: FilterRequestV2) => Promise<void>;
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
}

export function LocalPersonalProvider({ api, children, initialBootstrap }: LocalPersonalProviderProps) {
  const [bootstrap, setBootstrap] = useState(initialBootstrap);
  const [savedViews, setSavedViews] = useState(() => initialLibrary(initialBootstrap));
  const [pendingCount, setPendingCount] = useState(0);
  const [reloading, setReloading] = useState(false);
  const [error, setError] = useState<Error | null>(null);
  const bootstrapRef = useRef(initialBootstrap);
  const queueRef = useRef<Promise<void>>(Promise.resolve());

  const replaceBootstrap = useCallback((next: LocalBootstrapData) => {
    bootstrapRef.current = next;
    setBootstrap(next);
    setSavedViews(initialLibrary(next));
  }, []);

  const refresh = useCallback(async () => {
    const next = await api.bootstrap();
    replaceBootstrap(next);
    try {
      const library = await api.savedViews();
      if (library.stateRevision === bootstrapRef.current.state.stateRevision) setSavedViews(library);
    } catch {
      // Bootstrap already contains the complete library; exact match labels are optional.
    }
  }, [api, replaceBootstrap]);

  useEffect(() => {
    let active = true;
    void api.savedViews().then((library) => {
      if (active && library.stateRevision === bootstrapRef.current.state.stateRevision) {
        setSavedViews(library);
      }
    }).catch(() => undefined);
    return () => { active = false; };
  }, [api]);

  const reload = useCallback(async () => {
    setReloading(true);
    setError(null);
    try {
      await refresh();
    } catch (caught) {
      const next = caught instanceof Error ? caught : new Error('Local state reload failed.');
      setError(next);
      throw next;
    } finally {
      setReloading(false);
    }
  }, [refresh]);

  const enqueue = useCallback(<T,>(
    operation: (snapshot: PersonalStateSnapshot) => Promise<T>,
  ): Promise<T> => {
    setPendingCount((count) => count + 1);
    setError(null);
    const task = queueRef.current.then(async () => {
      try {
        const result = await operation(bootstrapRef.current.state);
        await refresh();
        return result;
      } catch (caught) {
        const next = caught instanceof Error ? caught : new Error('Local state mutation failed.');
        setError(next);
        throw next;
      } finally {
        setPendingCount((count) => Math.max(0, count - 1));
      }
    });
    queueRef.current = task.then(() => undefined, () => undefined);
    return task;
  }, [refresh]);

  const value = useMemo<LocalPersonalContextValue>(() => ({
    bootstrap,
    state: bootstrap.state,
    savedViews,
    busy: pendingCount > 0,
    reloading,
    error,
    clearError: () => setError(null),
    reload,
    updateSettings: async (settings) => {
      await enqueue(async (snapshot) => api.updateSettings({
        expectedUserStateRevision: snapshot.stateRevision,
        expectedRevision: snapshot.settings.revision,
        value: settings,
      }));
    },
    replaceSelection: async (sections) => {
      await enqueue(async (snapshot) => api.replaceSelection({
        expectedUserStateRevision: snapshot.stateRevision,
        sections,
      }));
    },
    replaceCurrentFilters: async (filters) => {
      await enqueue(async (snapshot) => api.replaceCurrentFilters({
        expectedUserStateRevision: snapshot.stateRevision,
        expectedCurrentFiltersRevision: snapshot.currentFilters.revision,
        filters,
      }));
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
      });
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
      });
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
      });
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
      });
    },
    deleteAllSavedViews: async () => {
      await enqueue(async (snapshot) => api.deleteAllSavedViews({
        expectedUserStateRevision: snapshot.stateRevision,
        expectedCurrentFiltersRevision: snapshot.currentFilters.revision,
      }));
    },
    resetCurrentFilters: async () => {
      await enqueue(async (snapshot) => api.resetCurrentFilters({
        expectedUserStateRevision: snapshot.stateRevision,
        expectedCurrentFiltersRevision: snapshot.currentFilters.revision,
      }));
    },
    prepareUserDataReset: () => enqueue(async (snapshot) =>
      api.prepareUserDataReset(snapshot.stateRevision)),
    confirmUserDataReset: (token) => enqueue(async () => api.confirmUserDataReset(token)),
    pullTerm: async (term) => {
      setError(null);
      try {
        return await api.pullTerm({ contractVersion: 1, term });
      } catch (caught) {
        const next = caught instanceof Error ? caught : new Error('Local term pull failed.');
        setError(next);
        throw next;
      }
    },
  }), [api, bootstrap, enqueue, error, pendingCount, reload, reloading, savedViews]);

  return <LocalPersonalContext.Provider value={value}>{children}</LocalPersonalContext.Provider>;
}

export function useLocalPersonal(): LocalPersonalContextValue {
  return useContext(LocalPersonalContext) ?? unavailable();
}

export function useLocalPersonalOptional(): LocalPersonalContextValue | null {
  return useContext(LocalPersonalContext);
}
