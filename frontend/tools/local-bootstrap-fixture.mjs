export const visualSessionNonce = '10000000-0000-4000-8000-000000000001';

export function localBootstrapFixture(overrides = {}) {
  const stateRevision = overrides.stateRevision ?? 7;
  return {
    mode: 'LOCAL',
    sessionNonce: visualSessionNonce,
    state: {
      stateRevision,
      settings: overrides.settings ?? {
        revision: 2,
        value: {
          localeOverride: 'system',
          catalogRefreshMinutes: 60,
          openRefreshSeconds: 30,
          volumePercent: 70,
          soundPolicy: {
            notificationMode: 'ONE_SHOT',
            maxAudible: 3,
            continuousDuration: { kind: 'FINITE', seconds: 600 },
          },
        },
      },
      currentFilters: overrides.currentFilters ?? {
        stateRevision,
        revision: 1,
        value: null,
      },
      savedViews: overrides.savedViews ?? [],
      selectedSections: overrides.selectedSections ?? [],
      episodeHistory: overrides.episodeHistory ?? {
        items: [],
        total: 0,
        offset: 0,
        limit: 50,
      },
      activeWatchCount: overrides.activeWatchCount ?? 0,
    },
  };
}
