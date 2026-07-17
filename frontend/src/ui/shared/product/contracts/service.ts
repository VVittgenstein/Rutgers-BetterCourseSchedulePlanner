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

export type ServiceTermRelativeOffsetV2 = -2 | -1 | 0 | 1 | 2;
export type ServiceOperationStageV2 =
  | 'CATALOG_FETCH'
  | 'CATALOG_PROCESS'
  | 'OPEN_FETCH'
  | 'SNAPSHOT_PUBLISH';
export type ServiceSnapshotAvailabilityV2 =
  | 'UNREQUESTED'
  | 'NO_COMPLETE_SNAPSHOT'
  | 'READY';
export type ServiceWorkStateV2 = 'IDLE' | 'QUEUED' | 'RUNNING' | 'RETRY_WAIT';
export type ServiceTermPublicationV2 = 'PUBLISHED' | 'UNPUBLISHED' | 'UNKNOWN';

export interface ServiceVisibleTermV2 {
  readonly term: string;
  readonly relativeOffset: ServiceTermRelativeOffsetV2;
  readonly publication: ServiceTermPublicationV2;
  readonly autoManaged: boolean;
  readonly manualPullAllowed: boolean;
  readonly watchable: boolean;
}

export interface ServiceTermWindowV2 {
  readonly currentTerm: string;
  readonly nextTerm: string;
  readonly visibleTerms: readonly ServiceVisibleTermV2[];
}

export interface ServiceAutomaticTermSummaryV2 {
  readonly term: string;
  readonly readyTargetCount: number;
  readonly totalTargetCount: number;
}

export interface ServiceOperationV2 {
  readonly target: TermCampusKey;
  readonly stage: ServiceOperationStageV2;
  readonly startedAt: IsoDateTime;
}

export interface ServiceTargetErrorV2 {
  readonly code: string;
  readonly httpStatus: number | null;
  readonly contentType: string | null;
  readonly contentEncoding: string | null;
  readonly decodedBytes: number | null;
  readonly errorClass: string | null;
  readonly errorChain: string | null;
  readonly traceId: string | null;
}

export interface ServiceTargetStatusV2 {
  readonly target: TermCampusKey;
  readonly primary: boolean;
  readonly snapshotAvailability: ServiceSnapshotAvailabilityV2;
  readonly workState: ServiceWorkStateV2;
  readonly stage: ServiceOperationStageV2 | null;
  readonly usable: boolean;
  readonly catalogContentVersion: number | null;
  readonly lastCompleteAt: IsoDateTime | null;
  readonly nextRetryAt: IsoDateTime | null;
  readonly error: ServiceTargetErrorV2 | null;
}

export interface ServiceStatusV2 {
  readonly contractVersion: 2;
  readonly observedAt: IsoDateTime;
  readonly runtime: ServiceRuntimeV1;
  readonly level: ServiceLevelV1;
  readonly discovery: CatalogDiscoveryStatusV1;
  readonly termWindow: ServiceTermWindowV2;
  readonly automaticTermSummaries: readonly ServiceAutomaticTermSummaryV2[];
  readonly operations: readonly ServiceOperationV2[];
  readonly targets: readonly ServiceTargetStatusV2[];
  readonly issues: readonly ServiceIssueV1[];
}

export type ServiceStatus = ServiceStatusV1 | ServiceStatusV2;

export function isServiceStatusV2(status: ServiceStatus): status is ServiceStatusV2 {
  return status.contractVersion === 2;
}
