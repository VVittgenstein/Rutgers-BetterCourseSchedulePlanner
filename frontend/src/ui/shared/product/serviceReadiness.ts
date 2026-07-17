import { isServiceStatusV2, type ServiceStatus } from './contracts/service';
import type { ServiceStatusScope } from './ProductApi';

export interface ServiceDatasetProgress {
  readonly current: number;
  readonly stale: number;
  readonly total: number;
  readonly unavailable: number;
}

export interface SearchDataProgress {
  readonly anyReady: boolean;
  readonly catalog: ServiceDatasetProgress;
  readonly current: number;
  readonly open: ServiceDatasetProgress;
  readonly percent: number;
  readonly ready: boolean;
  readonly total: number;
}

function datasetProgress(
  status: Exclude<ServiceStatus, { readonly contractVersion: 2 }>,
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

export function searchDataProgress(status: ServiceStatus | null): SearchDataProgress {
  if (status === null) {
    const empty = { current: 0, stale: 0, total: 0, unavailable: 0 };
    return {
      anyReady: false,
      catalog: empty,
      current: 0,
      open: empty,
      percent: 0,
      ready: false,
      total: 0,
    };
  }
  if (isServiceStatusV2(status)) {
    const current = status.automaticTermSummaries
      .reduce((total, summary) => total + summary.readyTargetCount, 0);
    const total = status.automaticTermSummaries
      .reduce((sum, summary) => sum + summary.totalTargetCount, 0);
    const empty = { current: 0, stale: 0, total: 0, unavailable: 0 };
    return {
      catalog: empty,
      current,
      open: empty,
      percent: total === 0 ? 0 : Math.round((current / total) * 100),
      anyReady: current > 0,
      ready: total > 0 && current === total,
      total,
    };
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
    anyReady: catalog.current > 0 && open.current > 0,
    catalog,
    current,
    open,
    percent: total === 0 ? 0 : Math.round((current / total) * 100),
    ready,
    total,
  };
}

export function isSearchDataReady(
  status: ServiceStatus | null | undefined,
  scope?: ServiceStatusScope,
): boolean {
  if (status === null || status === undefined) return false;
  if (!isServiceStatusV2(status)) return searchDataProgress(status).ready;
  if (scope === undefined || scope.term === null || scope.campuses.length === 0) return false;
  const usable = new Set(status.targets
    .filter((target) => target.usable)
    .map((target) => `${target.target.term}\u0000${target.target.campus}`));
  return scope.campuses.every((campus) => usable.has(`${scope.term}\u0000${campus}`));
}
