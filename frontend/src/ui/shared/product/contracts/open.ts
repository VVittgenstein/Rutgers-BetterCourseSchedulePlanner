import type {
  ContractVersionV1,
  IsoDateTime,
  SectionKey,
  TermCampusKey,
  TraceId,
} from './common';

export type OpenState = 'OPEN' | 'CLOSED' | 'UNKNOWN';
export type OpenFreshnessState = 'UNKNOWN' | 'FRESH' | 'STALE';
export type OpenUncertaintyReason =
  | 'NEVER_OBSERVED'
  | 'STALE_LAST_KNOWN_GOOD'
  | 'LATEST_ATTEMPT_FAILED'
  | 'LATEST_ATTEMPT_UNSAFE'
  | 'SUSPECT_PARTIAL_UPSTREAM'
  | 'STALE_CATALOG_RACE'
  | 'CATALOG_VERSION_UNAVAILABLE';
export type OpenRefreshClassification =
  | 'IN_FLIGHT'
  | 'VALID_APPLIED'
  | 'VALID_EMPTY_NO_ROWS'
  | 'UNSAFE_EMPTY'
  | 'UNSAFE_ZERO_INTERSECTION'
  | 'SUSPECT_PARTIAL_SNAPSHOT'
  | 'STALE_CATALOG_RACE'
  | 'FAILED';
export type OpenFailureClass =
  | 'NETWORK'
  | 'TIMEOUT'
  | 'HTTP_408'
  | 'HTTP_429'
  | 'FORBIDDEN'
  | 'OTHER_HTTP_4XX'
  | 'HTTP_5XX'
  | 'OFF_ORIGIN_REDIRECT'
  | 'NON_JSON'
  | 'OVERSIZE'
  | 'SCHEMA_VIOLATION'
  | 'INVALID_VALUE'
  | 'PERSIST'
  | 'UNSAFE_EMPTY'
  | 'UNSAFE_ZERO_INTERSECTION'
  | 'SUSPECT_PARTIAL_SNAPSHOT';
export type OpenSchedulerLane =
  | 'GENERAL'
  | 'ACTIVE_WATCH'
  | 'FIRST_LOAD'
  | 'MANUAL_REFRESH'
  | 'CATALOG_RACE_RECHECK';
export type OpenCircuitState = 'CLOSED' | 'RETRY_AFTER' | 'FATAL_DIAGNOSTIC';
export type OpenCircuitReason =
  | 'RATE_LIMITED'
  | 'FORBIDDEN'
  | 'OTHER_HTTP_4XX'
  | 'OFF_ORIGIN_REDIRECT'
  | 'SAME_ORIGIN_REDIRECT'
  | 'INVALID_REDIRECT'
  | 'NON_JSON'
  | 'INVALID_CONTENT_TYPE'
  | 'SCHEMA_VIOLATION'
  | 'INVALID_VALUE'
  | 'OVERSIZE';

export interface OpenPullCountsV1 {
  readonly attempted: number;
  readonly succeeded: number;
  readonly failed: number;
  readonly empty: number;
}

export interface OpenCounterSnapshotV1 {
  readonly runCounts?: OpenPullCountsV1;
  readonly todayCounts: OpenPullCountsV1;
  readonly rutgersDay: string;
  readonly dayTimezone: 'America/New_York';
}

export interface OpenFreshnessV1 {
  readonly state: OpenFreshnessState;
  readonly observedAt: IsoDateTime | null;
  readonly freshUntil: IsoDateTime | null;
  readonly lastKnownGoodAgeSeconds: number | null;
  readonly uncertainty: OpenUncertaintyReason | null;
}

export interface OpenAttemptPointV1 {
  readonly attemptSequence: number;
  readonly startedAt: IsoDateTime;
  readonly completedAt: IsoDateTime | null;
  readonly classification: OpenRefreshClassification;
  readonly failureClass: OpenFailureClass | null;
}

export interface OpenFailurePointV1 {
  readonly attemptSequence: number;
  readonly failedAt: IsoDateTime;
  readonly failureClass: OpenFailureClass;
}

export interface OpenObservationPointV1 {
  readonly observationId: TraceId;
  readonly observationSequence: number;
  readonly catalogContentVersion: number;
  readonly observedAt: IsoDateTime;
  readonly canonicalSetHash: string;
  readonly stateHash: string;
}

export interface OpenCircuitStatusV1 {
  readonly state: OpenCircuitState;
  readonly reason: OpenCircuitReason | null;
  readonly openedAt: IsoDateTime | null;
  readonly retryAt: IsoDateTime | null;
  readonly diagnosticRecheckRequired: boolean;
}

export interface OpenSchedulerStatusV1 {
  readonly lane: OpenSchedulerLane;
  readonly requestedGeneralIntervalSeconds: number;
  readonly requestedEffectiveIntervalSeconds: number;
  readonly activeWatchCount: number;
  readonly nextDueAt: IsoDateTime | null;
  readonly inFlight: boolean;
  readonly schedulerLagMilliseconds: number;
  readonly actualStartToStartIntervalMilliseconds: number | null;
  readonly failureStreak: number;
}

export interface OpenRefreshStatusV1 {
  readonly contractVersion: ContractVersionV1;
  readonly batch: TermCampusKey;
  readonly catalogContentVersion: number | null;
  readonly latestAttempt: OpenAttemptPointV1 | null;
  readonly latestFailure: OpenFailurePointV1 | null;
  readonly lastValidObservation: OpenObservationPointV1 | null;
  readonly lastBodyChangeAt: IsoDateTime | null;
  readonly lastStateChangeAt: IsoDateTime | null;
  readonly freshness: OpenFreshnessV1;
  readonly scheduler: OpenSchedulerStatusV1;
  readonly circuit: OpenCircuitStatusV1;
  readonly counterSnapshot: OpenCounterSnapshotV1;
}

export interface OpenSectionStatusV1 {
  readonly contractVersion: ContractVersionV1;
  readonly sectionKey: SectionKey;
  readonly state: OpenState;
  readonly lastObservationId: TraceId | null;
  readonly catalogContentVersion: number | null;
  readonly freshness: OpenFreshnessV1;
  readonly schedulerLagMilliseconds: number;
  readonly counterSnapshot: OpenCounterSnapshotV1;
}

export interface OpenStatusRequestV1 {
  readonly contractVersion: ContractVersionV1;
  readonly batch: TermCampusKey;
}

export interface OpenBatchStatusRequestV1 {
  readonly contractVersion: ContractVersionV1;
  readonly batch: TermCampusKey;
  readonly sectionKeys: readonly SectionKey[];
}

export interface OpenBatchStatusV1 {
  readonly contractVersion: ContractVersionV1;
  readonly refresh: OpenRefreshStatusV1;
  readonly sections: readonly OpenSectionStatusV1[];
}

export interface OpenSectionStatusRequestV1 {
  readonly contractVersion: ContractVersionV1;
  readonly sectionKey: SectionKey;
}

export interface OpenObservationV1 {
  readonly contractVersion: ContractVersionV1;
  readonly observationId: TraceId;
  readonly refreshObservationId: TraceId;
  readonly batch: TermCampusKey;
  readonly sectionKey: SectionKey;
  readonly pullSequence: number;
  readonly catalogContentVersion: number;
  readonly state: OpenState;
  readonly observedAt: IsoDateTime;
  readonly freshUntil: IsoDateTime;
  readonly schedulerLagMilliseconds: number;
  readonly counterSnapshot: OpenCounterSnapshotV1;
}
