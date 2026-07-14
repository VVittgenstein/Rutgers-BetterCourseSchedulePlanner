import type {
  CatalogSynchronicity,
  CatalogUnknownReason,
  FilterSerializationIssue,
  LiveOpenStateV1,
  MatchOutcome,
  MatchReasonCode,
  ModalityFilterV1,
  OpenCircuitState,
  OpenEpisodeState,
  OpenFreshnessState,
  OpenRefreshClassification,
  OpenSchedulerLane,
  OpenState,
  OpenUncertaintyReason,
  PermissionFilterV1,
  PrerequisiteFilterV1,
  WatchNotificationMode,
  WeekdayV1,
} from '../product';
import type { MessageKey } from './contract';

export const weekdayMessageKeys = {
  MONDAY: 'filter.option.monday',
  TUESDAY: 'filter.option.tuesday',
  WEDNESDAY: 'filter.option.wednesday',
  THURSDAY: 'filter.option.thursday',
  FRIDAY: 'filter.option.friday',
  SATURDAY: 'filter.option.saturday',
  SUNDAY: 'filter.option.sunday',
} as const satisfies Readonly<Record<WeekdayV1, MessageKey>>;

export const prerequisiteMessageKeys = {
  ANY: 'filter.option.any',
  HAS: 'filter.option.has_prerequisite',
  NONE_REPORTED: 'filter.option.none_reported',
} as const satisfies Readonly<Record<PrerequisiteFilterV1, MessageKey>>;

export const permissionMessageKeys = {
  ANY: 'filter.option.any',
  REQUIRED: 'filter.option.required',
  NOT_REQUIRED: 'filter.option.not_required',
} as const satisfies Readonly<Record<PermissionFilterV1, MessageKey>>;

export const openStateMessageKeys = {
  OPEN: 'open.state.open',
  CLOSED: 'open.state.closed',
  UNKNOWN: 'open.state.unknown',
} as const satisfies Readonly<Record<OpenState | LiveOpenStateV1, MessageKey>>;

export const modalityMessageKeys = {
  ON_CAMPUS_OR_IN_PERSON: 'filter.option.on_campus',
  ONLINE: 'filter.option.online',
  HYBRID: 'filter.option.hybrid',
  OTHER: 'filter.option.other',
  UNKNOWN: 'common.unknown',
} as const satisfies Readonly<Record<ModalityFilterV1, MessageKey>>;

export const synchronicityMessageKeys = {
  SYNC: 'filter.option.sync',
  ASYNC: 'filter.option.async',
  MIXED: 'filter.option.mixed',
  UNSPECIFIED: 'filter.option.unspecified',
  UNKNOWN: 'common.unknown',
} as const satisfies Readonly<Record<CatalogSynchronicity, MessageKey>>;

export const catalogUnknownReasonMessageKeys = {
  MISSING: 'catalog.unknown.missing',
  EXPLICIT_NULL: 'catalog.unknown.explicit_null',
  MALFORMED: 'catalog.unknown.malformed',
  SPARSE_EVIDENCE: 'catalog.unknown.sparse_evidence',
  NOT_OBSERVED: 'catalog.unknown.not_observed',
  SOURCE_UNAVAILABLE: 'catalog.unknown.source_unavailable',
  CONFLICTING_EVIDENCE: 'catalog.unknown.conflicting_evidence',
} as const satisfies Readonly<Record<CatalogUnknownReason, MessageKey>>;

export const matchOutcomeMessageKeys = {
  MATCH: 'match.outcome.match',
  UNCERTAIN: 'match.outcome.uncertain',
  NO_MATCH: 'match.outcome.no_match',
} as const satisfies Readonly<Record<MatchOutcome, MessageKey>>;

export const matchReasonMessageKeys = {
  KNOWN_MISMATCH: 'match.reason.known_mismatch',
  MISSING_RELIABLE_DATA: 'match.reason.missing_reliable_data',
  UNKNOWN_VALUE: 'match.reason.unknown_value',
  TBA: 'match.reason.tba',
  CONFLICTING_EVIDENCE: 'match.reason.conflicting_evidence',
  INVALID_VALUE: 'match.reason.invalid_value',
  SOURCE_UNAVAILABLE: 'match.reason.source_unavailable',
} as const satisfies Readonly<Record<MatchReasonCode, MessageKey>>;

export const freshnessMessageKeys = {
  FRESH: 'freshness.fresh',
  STALE: 'freshness.stale',
  UNKNOWN: 'freshness.unknown',
} as const satisfies Readonly<Record<OpenFreshnessState, MessageKey>>;

export const openUncertaintyMessageKeys = {
  NEVER_OBSERVED: 'open.uncertainty.never_observed',
  STALE_LAST_KNOWN_GOOD: 'open.uncertainty.stale_last_known_good',
  LATEST_ATTEMPT_FAILED: 'open.uncertainty.latest_attempt_failed',
  LATEST_ATTEMPT_UNSAFE: 'open.uncertainty.latest_attempt_unsafe',
  STALE_CATALOG_RACE: 'open.uncertainty.stale_catalog_race',
  CATALOG_VERSION_UNAVAILABLE: 'open.uncertainty.catalog_version_unavailable',
} as const satisfies Readonly<Record<OpenUncertaintyReason, MessageKey>>;

export const refreshClassificationMessageKeys = {
  IN_FLIGHT: 'open.refresh.in_flight',
  VALID_APPLIED: 'open.refresh.valid_applied',
  VALID_EMPTY_NO_ROWS: 'open.refresh.valid_empty',
  UNSAFE_EMPTY: 'open.refresh.unsafe_empty',
  UNSAFE_ZERO_INTERSECTION: 'open.refresh.unsafe_zero_intersection',
  STALE_CATALOG_RACE: 'open.refresh.stale_catalog_race',
  FAILED: 'open.refresh.failed',
} as const satisfies Readonly<Record<OpenRefreshClassification, MessageKey>>;

export const schedulerLaneMessageKeys = {
  GENERAL: 'open.lane.general',
  ACTIVE_WATCH: 'open.lane.active_watch',
  FIRST_LOAD: 'open.lane.first_load',
  MANUAL_REFRESH: 'open.lane.manual_refresh',
  CATALOG_RACE_RECHECK: 'open.lane.catalog_race_recheck',
} as const satisfies Readonly<Record<OpenSchedulerLane, MessageKey>>;

export const circuitStateMessageKeys = {
  CLOSED: 'open.circuit.closed',
  RETRY_AFTER: 'open.circuit.retry_after',
  FATAL_DIAGNOSTIC: 'open.circuit.fatal_diagnostic',
} as const satisfies Readonly<Record<OpenCircuitState, MessageKey>>;

export const watchNotificationMessageKeys = {
  ONE_SHOT: 'watch.notification.one_shot',
  CONTINUOUS: 'watch.notification.continuous',
} as const satisfies Readonly<Record<WatchNotificationMode, MessageKey>>;

export const episodeStateMessageKeys = {
  UNACKNOWLEDGED: 'watch.episode.unacknowledged',
  ACKNOWLEDGED: 'watch.episode.acknowledged',
  TIMED_OUT: 'watch.episode.timed_out',
  CLOSED: 'watch.episode.closed',
} as const satisfies Readonly<Record<OpenEpisodeState, MessageKey>>;

export const filterSerializationIssueMessageKeys = {
  INVALID_AVAILABILITY: 'filter.validation.invalid_availability',
  INVALID_CREDIT_RANGE: 'filter.validation.invalid_credit_range',
  INVALID_SECTION_INDEX: 'filter.validation.invalid_section_index',
  INVALID_TEXT: 'filter.validation.invalid_text',
  TERM_REQUIRED: 'filter.validation.term_required',
  INVALID_PAGE: 'filter.validation.invalid_page',
  INVALID_TOKEN: 'filter.validation.invalid_token',
  TOO_MANY_VALUES: 'filter.validation.too_many_values',
} as const satisfies Readonly<Record<FilterSerializationIssue, MessageKey>>;

const filterOptionMessageKeys: Readonly<Record<string, MessageKey>> = {
  ...weekdayMessageKeys,
  ...modalityMessageKeys,
  ...synchronicityMessageKeys,
  ...openStateMessageKeys,
  ANY: 'filter.option.any',
  ALL: 'filter.option.all',
  HAS: 'filter.option.has_prerequisite',
  NONE_REPORTED: 'filter.option.none_reported',
  REQUIRED: 'filter.option.required',
  NOT_REQUIRED: 'filter.option.not_required',
};

export function filterOptionMessageKey(value: string): MessageKey | undefined {
  return filterOptionMessageKeys[value];
}
