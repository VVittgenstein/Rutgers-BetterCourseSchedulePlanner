import {
  lazy,
  Suspense,
  useCallback,
  useEffect,
  useMemo,
  useRef,
} from 'react';

import {
  SharedApplication,
  type SharedExperienceConfiguration,
  type SharedWorkspaceExtension,
} from '../shared/SharedApplication';
import { useBcspI18n } from '../shared/i18n/runtime';
import {
  toFilterRequestV1,
  type FilterStateV1,
  type WatchPolicyV1,
} from '../shared/product';
import { currentSystemLanguages, resolveLocalLocale } from './i18n/localeBootstrap';
import { useLocalI18n } from './i18n/runtime';
import { LocalTermPullAction } from './LocalTermPullAction';
import {
  LocalPersonalStyles,
  useLocalPersonal,
  type LocalSettings,
  type PreparedUserDataReset,
  type SavedViewDefinition,
} from './personal';

const PERSIST_DELAY_MS = 300;

const HistoryPage = lazy(async () => ({
  default: (await import('./pages/HistoryPage')).HistoryPage,
}));
const SavedViewsPage = lazy(async () => ({
  default: (await import('./pages/SavedViewsPage')).SavedViewsPage,
}));
const SettingsPage = lazy(async () => ({
  default: (await import('./pages/SettingsPage')).SettingsPage,
}));

function LocalPageFallback() {
  const local = useLocalI18n();
  return (
    <p aria-live="polite" className="local-personal__notice" role="status">
      {local.t('local.status.busy')}
    </p>
  );
}

function storedFilterState(
  content: ReturnType<typeof useLocalPersonal>['state']['currentFilters']['value'],
): FilterStateV1 | undefined {
  return content?.content.status === 'COMPATIBLE' ? { ...content.content.filters.values } : undefined;
}

function currentCompatibleFilters(
  content: ReturnType<typeof useLocalPersonal>['state']['currentFilters']['value'],
) {
  if (content?.content.status !== 'COMPATIBLE') {
    throw new Error('Current filters are not compatible.');
  }
  return content.content.filters;
}

export function LocalApplication() {
  const personal = useLocalPersonal();
  const personalRef = useRef(personal);
  personalRef.current = personal;
  const i18n = useBcspI18n();
  const local = useLocalI18n();
  const storedLocaleOverride = personal.state.settings.value.localeOverride;
  const filterTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const settingsTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const settingsDraft = useRef(personal.state.settings.value);
  const settingsVersion = useRef(0);
  const settingsDirty = useRef(false);

  useEffect(() => {
    if (!settingsDirty.current) settingsDraft.current = personal.state.settings.value;
  }, [personal.state.settings.value]);

  useEffect(() => () => {
    if (filterTimer.current !== null) clearTimeout(filterTimer.current);
    if (settingsTimer.current !== null) clearTimeout(settingsTimer.current);
  }, []);

  useEffect(() => {
    const locale = resolveLocalLocale({
      savedLocale: storedLocaleOverride === 'system' ? null : storedLocaleOverride,
      systemLanguages: currentSystemLanguages(),
    });
    if (locale !== i18n.locale) void i18n.changeLocale(locale);
    // Locale button changes are optimistic; only a persisted override change re-runs this sync.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [i18n.changeLocale, storedLocaleOverride]);

  const persistSettings = useCallback((value: LocalSettings, delay = PERSIST_DELAY_MS) => {
    settingsDraft.current = value;
    settingsDirty.current = true;
    settingsVersion.current += 1;
    const version = settingsVersion.current;
    if (settingsTimer.current !== null) clearTimeout(settingsTimer.current);
    settingsTimer.current = setTimeout(() => {
      settingsTimer.current = null;
      const next = settingsDraft.current;
      void personalRef.current.updateSettings(next).then(() => {
        if (settingsVersion.current === version) settingsDirty.current = false;
      }).catch(() => undefined);
    }, delay);
  }, []);

  const persistSettingsNow = useCallback(async (value: LocalSettings) => {
    settingsDraft.current = value;
    settingsDirty.current = true;
    settingsVersion.current += 1;
    const version = settingsVersion.current;
    if (settingsTimer.current !== null) {
      clearTimeout(settingsTimer.current);
      settingsTimer.current = null;
    }
    try {
      await personalRef.current.updateSettings(value);
      if (settingsVersion.current === version) settingsDirty.current = false;
    } catch (error) {
      throw error;
    }
  }, []);

  const persistFilters = useCallback((filters: FilterStateV1) => {
    let request;
    try {
      request = toFilterRequestV1(filters);
    } catch {
      return;
    }
    if (filterTimer.current !== null) clearTimeout(filterTimer.current);
    filterTimer.current = setTimeout(() => {
      filterTimer.current = null;
      void personalRef.current.replaceCurrentFilters(request).catch(() => undefined);
    }, PERSIST_DELAY_MS);
  }, []);

  const patchSettings = useCallback((patch: Partial<LocalSettings>) => {
    persistSettings({ ...settingsDraft.current, ...patch });
  }, [persistSettings]);

  const persistSelection = useCallback((sections: SharedExperienceConfiguration['initialSelectedSections']) => {
    if (sections === undefined) return;
    void personalRef.current.replaceSelection(sections).catch(() => undefined);
  }, []);

  const persistVolume = useCallback((volumePercent: number) => {
    patchSettings({ volumePercent });
  }, [patchSettings]);

  const persistWatchPolicy = useCallback((soundPolicy: WatchPolicyV1) => {
    patchSettings({ soundPolicy });
  }, [patchSettings]);

  const persistLocale = useCallback((localeOverride: LocalSettings['localeOverride']) => {
    patchSettings({ localeOverride });
  }, [patchSettings]);

  const commonPageState = {
    pending: personal.busy || personal.reloading,
    error: personal.error,
    onClearError: personal.clearError,
    onReload: personal.reload,
  } as const;

  const savedPage = (
    <Suspense fallback={<LocalPageFallback />}>
      <SavedViewsPage
        {...commonPageState}
        library={personal.savedViews}
        onApply={(view: SavedViewDefinition) => personal.applySavedView(view.id)}
        onCreate={personal.createSavedView}
        onDelete={(view: SavedViewDefinition) => personal.deleteSavedView(view.id)}
        onDeleteAll={personal.deleteAllSavedViews}
        onDuplicate={(view: SavedViewDefinition, name: string) =>
          personal.duplicateSavedView(view.id, name)}
        onRename={(view: SavedViewDefinition, name: string) =>
          personal.renameSavedView(view.id, name)}
        onUpdate={(view: SavedViewDefinition) => personal.updateSavedView(
          view.id,
          currentCompatibleFilters(personal.state.currentFilters.value),
        )}
      />
    </Suspense>
  );
  const historyPage = (
    <Suspense fallback={<LocalPageFallback />}>
      <HistoryPage {...commonPageState} history={personal.state.episodeHistory} />
    </Suspense>
  );
  const settingsPage = (
    <Suspense fallback={<LocalPageFallback />}>
      <SettingsPage
        {...commonPageState}
        onConfirmUserDataReset={async (prepared: PreparedUserDataReset) => {
          await personal.confirmUserDataReset(prepared.confirmationToken);
          globalThis.location?.reload();
        }}
        onDeleteAllSavedViews={personal.deleteAllSavedViews}
        onPrepareUserDataReset={personal.prepareUserDataReset}
        onResetCurrentFilters={personal.resetCurrentFilters}
        onUpdateSettings={persistSettingsNow}
        savedViewCount={personal.savedViews.views.length}
        settings={personal.state.settings}
      />
    </Suspense>
  );

  const workspaceExtensions = useMemo<readonly SharedWorkspaceExtension[]>(() => [
    {
      content: savedPage,
      intro: local.t('local.saved.intro'),
      navigationLabel: local.t('local.nav.saved_views'),
      path: '/saved-views',
      sequence: '04',
      title: local.t('local.saved.title'),
    },
    {
      content: historyPage,
      intro: local.t('local.history.intro'),
      navigationLabel: local.t('local.nav.history'),
      path: '/history',
      sequence: '05',
      title: local.t('local.history.title'),
    },
    {
      content: settingsPage,
      intro: local.t('local.settings.intro'),
      navigationLabel: local.t('local.nav.settings'),
      path: '/settings',
      sequence: '06',
      title: local.t('local.settings.title'),
    },
  ], [historyPage, local, savedPage, settingsPage]);

  const experience = useMemo<SharedExperienceConfiguration>(() => ({
    initialFilters: storedFilterState(personal.state.currentFilters.value),
    initialSelectedSections: personal.state.selectedSections,
    initialVolume: personal.state.settings.value.volumePercent,
    initialWatchPolicy: personal.state.settings.value.soundPolicy,
    onFiltersChange: persistFilters,
    onSelectedSectionsChange: persistSelection,
    onVolumeChange: persistVolume,
    onWatchPolicyChange: persistWatchPolicy,
    renderUnavailableScopeAction: (context) => (
      <LocalTermPullAction
        {...context}
        onPull={async (term) => {
          await personal.pullTerm(term);
        }}
      />
    ),
  }), [
    local,
    persistFilters,
    personal,
    persistSelection,
    persistVolume,
    persistWatchPolicy,
    personal.state.currentFilters.value,
    personal.state.selectedSections,
    personal.state.settings.value.soundPolicy,
    personal.state.settings.value.volumePercent,
  ]);

  return (
    <>
      <LocalPersonalStyles />
      <SharedApplication
        experience={experience}
        onLocaleChange={persistLocale}
        workspaceExtensions={workspaceExtensions}
      />
      <div
        aria-live="polite"
        className="local-personal-sync"
        data-error={personal.error === null ? undefined : true}
        role={personal.error === null ? 'status' : 'alert'}
      >
        {personal.error !== null
          ? local.t('local.status.error')
          : personal.busy || personal.reloading
            ? local.t('local.status.busy')
            : null}
      </div>
    </>
  );
}
