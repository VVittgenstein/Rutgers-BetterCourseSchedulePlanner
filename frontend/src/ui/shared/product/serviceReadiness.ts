import type { ServiceStatusV1 } from './contracts/service';

export interface ServiceDatasetProgress {
  readonly current: number;
  readonly stale: number;
  readonly total: number;
  readonly unavailable: number;
}

export interface SearchDataProgress {
  readonly catalog: ServiceDatasetProgress;
  readonly current: number;
  readonly open: ServiceDatasetProgress;
  readonly percent: number;
  readonly ready: boolean;
  readonly total: number;
}

function datasetProgress(
  status: ServiceStatusV1,
  dataset: 'catalog' | 'open',
): ServiceDatasetProgress {
  const summary = status[dataset];
  const availability = dataset === 'catalog' ? 'catalogAvailability' : 'openAvailability';
  const current = summary.currentTargetCount
    ?? status.targets.filter((target) => target[availability] === 'CURRENT').length;
  const stale = summary.staleTargetCount
    ?? status.targets.filter((target) => target[availability] === 'STALE').length;
  const unavailable = summary.unavailableTargetCount
    ?? Math.max(0, summary.totalTargetCount - current - stale);
  return { current, stale, total: summary.totalTargetCount, unavailable };
}

export function searchDataProgress(status: ServiceStatusV1 | null): SearchDataProgress {
  if (status === null) {
    const empty = { current: 0, stale: 0, total: 0, unavailable: 0 };
    return { catalog: empty, current: 0, open: empty, percent: 0, ready: false, total: 0 };
  }
  const catalog = datasetProgress(status, 'catalog');
  const open = datasetProgress(status, 'open');
  const current = catalog.current + open.current;
  const total = catalog.total + open.total;
  const blocking = status.issues.some(({ severity }) => severity === 'BLOCKING');
  const ready = !blocking
    && catalog.total > 0
    && open.total > 0
    && catalog.total === open.total
    && catalog.current === catalog.total
    && open.current === open.total;
  return {
    catalog,
    current,
    open,
    percent: total === 0 ? 0 : Math.round((current / total) * 100),
    ready,
    total,
  };
}

export function isSearchDataReady(status: ServiceStatusV1 | null | undefined): boolean {
  return status !== undefined && searchDataProgress(status).ready;
}
