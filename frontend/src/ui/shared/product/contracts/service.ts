import type {
  CatalogDiscoveryStatusV1,
} from './catalog';
import type {
  ContractVersionV1,
  IsoDateTime,
  TermCampusKey,
} from './common';

export type ServiceRuntimeV1 = 'LOCAL' | 'PUBLIC';
export type ServiceLevelV1 =
  | 'INITIALIZING'
  | 'PARTIALLY_READY'
  | 'READY'
  | 'DEGRADED'
  | 'ERROR';
export type ServiceOperationPhaseV1 =
  | 'STARTING'
  | 'DISCOVERING'
  | 'CATALOG_FETCH'
  | 'CATALOG_PROCESS'
  | 'CATALOG_PUBLISH'
  | 'OPEN_FETCH'
  | 'IDLE'
  | 'RETRY_WAIT'
  | 'STOPPED';
export type ServiceAvailabilityV1 = 'UNAVAILABLE' | 'CURRENT' | 'STALE';
export type ServiceIssueComponentV1 =
  | 'DISCOVERY'
  | 'CATALOG'
  | 'OPEN'
  | 'SCHEDULER'
  | 'STORAGE';
export type ServiceIssueSeverityV1 = 'DEGRADED' | 'BLOCKING';
export type ServiceIssueRecoveryV1 = 'AUTOMATIC_RETRY' | 'USER_ACTION_REQUIRED';

export interface ServiceOperationV1 {
  readonly phase: ServiceOperationPhaseV1;
  readonly target: TermCampusKey | null;
  readonly startedAt: IsoDateTime | null;
  readonly nextRetryAt: IsoDateTime | null;
}

export interface ServiceDatasetSummaryV1 {
  readonly totalTargetCount: number;
  readonly availableTargetCount: number;
  readonly currentTargetCount: number;
  readonly staleTargetCount: number;
  readonly unavailableTargetCount: number;
}

export interface ServiceTargetStatusV1 {
  readonly target: TermCampusKey;
  readonly primary: boolean;
  readonly catalogAvailability: ServiceAvailabilityV1;
  readonly catalogContentVersion: number | null;
  readonly openAvailability: ServiceAvailabilityV1;
  readonly searchAvailable: boolean;
}

export interface ServiceIssueV1 {
  readonly component: ServiceIssueComponentV1;
  readonly target: TermCampusKey | null;
  readonly code: string;
  readonly severity: ServiceIssueSeverityV1;
  readonly recovery: ServiceIssueRecoveryV1;
  readonly retryAt: IsoDateTime | null;
}

export interface ServiceStatusV1 {
  readonly contractVersion: ContractVersionV1;
  readonly observedAt: IsoDateTime;
  readonly runtime: ServiceRuntimeV1;
  readonly level: ServiceLevelV1;
  readonly operation: ServiceOperationV1;
  readonly discovery: CatalogDiscoveryStatusV1;
  readonly catalog: ServiceDatasetSummaryV1;
  readonly open: ServiceDatasetSummaryV1;
  readonly targets: readonly ServiceTargetStatusV1[];
  readonly issues: readonly ServiceIssueV1[];
}
