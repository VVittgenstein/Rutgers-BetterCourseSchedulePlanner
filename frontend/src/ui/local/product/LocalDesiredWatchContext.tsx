import { createContext, useContext, useMemo, type ReactNode } from 'react';

import type { WatchIntentPort } from '../../shared/watch';

/**
 * Carries the local desired-watch authority from the bootstrap that built it
 * to the composition that hands it to the watch desk.
 *
 * A context rather than a prop chain because the two ends are far apart and
 * everything between them is target-neutral: the shared application only
 * knows there may be a standing-intent port, and only the local build has
 * one to give it.
 */

const LocalWatchIntentContext = createContext<WatchIntentPort | null>(null);

export function LocalDesiredWatchProvider({
  children,
  watchIntent,
}: {
  readonly children: ReactNode;
  readonly watchIntent: WatchIntentPort;
}) {
  const value = useMemo(() => watchIntent, [watchIntent]);
  return (
    <LocalWatchIntentContext.Provider value={value}>
      {children}
    </LocalWatchIntentContext.Provider>
  );
}

export function useLocalWatchIntent(): WatchIntentPort | null {
  return useContext(LocalWatchIntentContext);
}
