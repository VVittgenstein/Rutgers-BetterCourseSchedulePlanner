import {
  ProductBootstrapError,
  type FilterRequestV2,
  type OpenEpisodeState,
  type SectionKey,
  type TraceId,
  type WatchNotificationMode,
  type WatchPolicyV1,
} from '../../shared/product';

export type LocalLocaleOverride = 'system' | 'en-US' | 'zh-CN';

export interface LocalSettings {
  readonly localeOverride: LocalLocaleOverride;
  readonly catalogRefreshMinutes: number;
  readonly openRefreshSeconds: number;
  readonly watchFastLaneSeconds: number;
  readonly volumePercent: number;
  readonly soundPolicy: WatchPolicyV1;
}

export interface LocalTermPullRequest {
  readonly contractVersion: 1;
  readonly term: string;
}

export interface LocalTermPullResponse {
  readonly contractVersion: 1;
  readonly term: string;
  readonly disposition: 'ENQUEUED' | 'ALREADY_REQUESTED' | 'ALREADY_READY';
  readonly targetCount: number;
}

export interface StoredSettings {
  readonly revision: number;
  readonly value: LocalSettings;
}

export type FilterAssociation =
  | { readonly kind: 'CUSTOM' }
  | { readonly kind: 'APPLIED'; readonly viewId: TraceId; readonly revision: number };

export type SavedViewIncompatibility =
  | { readonly kind: 'INVALID_ENVELOPE' }
  | { readonly kind: 'UNSUPPORTED_CODEC_VERSION'; readonly observed: number | null }
  | { readonly kind: 'FUTURE_SCHEMA_VERSION'; readonly observed: number; readonly supported: number }
  | { readonly kind: 'UNKNOWN_FIELD'; readonly stableId: string }
  | { readonly kind: 'MISSING_REQUIRED_FIELD'; readonly stableId: string }
  | { readonly kind: 'INVALID_FIELD_DATA' };

export type SavedViewContent =
  | { readonly status: 'COMPATIBLE'; readonly filters: FilterRequestV2 }
  | {
    readonly status: 'INCOMPATIBLE';
    readonly rawSnapshot: unknown;
    readonly reason: SavedViewIncompatibility;
  };

export interface CurrentFilters {
  readonly association: FilterAssociation;
  readonly content: SavedViewContent;
}

export interface StoredCurrentFilters {
  readonly stateRevision: number;
  readonly revision: number;
  readonly value: CurrentFilters | null;
}

export interface SavedViewDefinition {
  readonly id: TraceId;
  readonly name: string;
  readonly schemaVersion: number;
  readonly revision: number;
  readonly content: SavedViewContent;
  readonly createdAt: number;
  readonly updatedAt: number;
}

export type SavedViewMatch = 'NOT_APPLIED' | 'CLEAN' | 'MODIFIED' | 'INCOMPATIBLE';

export interface SavedViewListItem {
  readonly definition: SavedViewDefinition;
  readonly matchState: SavedViewMatch;
}

export interface SavedViewLibrary {
  readonly stateRevision: number;
  readonly currentFilters: StoredCurrentFilters;
  readonly views: readonly SavedViewListItem[];
}

export interface SavedViewMutation {
  readonly stateRevision: number;
  readonly definition: SavedViewDefinition;
  readonly currentFilters: StoredCurrentFilters;
}

export interface SavedViewDeleteResult {
  readonly stateRevision: number;
  readonly deletedId: TraceId;
  readonly currentFilters: StoredCurrentFilters;
}

export interface SavedViewsDeleteAllResult {
  readonly stateRevision: number;
  readonly deletedViews: number;
  readonly currentFilters: StoredCurrentFilters;
}

export type WatchStopReason =
  | 'USER_REQUESTED'
  | 'CONNECTION_CLOSED'
  | 'HEARTBEAT_TIMEOUT'
  | 'SERVICE_STOPPING'
  | 'TERM_OUT_OF_RANGE';

export type EpisodeDisposition =
  | { readonly kind: 'SECTION_CLOSED' }
  | { readonly kind: 'ACKNOWLEDGED' }
  | { readonly kind: 'TIMED_OUT' }
  | { readonly kind: 'WATCH_STOPPED'; readonly reason: WatchStopReason };

export interface EpisodeHistoryIdentity {
  readonly sectionKey: SectionKey;
  readonly runId: TraceId;
  readonly episodeId: TraceId;
}

export interface EpisodeHistorySummary {
  readonly identity: EpisodeHistoryIdentity;
  readonly state: OpenEpisodeState;
  readonly mode: WatchNotificationMode;
  readonly firstObservedAt: number;
  readonly lastObservedAt: number;
  readonly acknowledgedAt: number | null;
  readonly timedOutAt: number | null;
  readonly closedAt: number | null;
  readonly disposition: EpisodeDisposition | null;
  readonly audibleCount: number;
  readonly observationCount: number;
  readonly lastObservationId: TraceId;
  readonly actionCount: number;
}

export interface HistoryPage<T> {
  readonly items: readonly T[];
  readonly total: number;
  readonly offset: number;
  readonly limit: number;
}

export interface PersonalStateSnapshot {
  readonly stateRevision: number;
  readonly settings: StoredSettings;
  readonly currentFilters: StoredCurrentFilters;
  readonly savedViews: readonly SavedViewDefinition[];
  readonly selectedSections: readonly SectionKey[];
  readonly episodeHistory: HistoryPage<EpisodeHistorySummary>;
  readonly activeWatchCount: number;
}

export interface LocalBootstrapData {
  readonly mode: 'LOCAL';
  readonly sessionNonce: string;
  readonly state: PersonalStateSnapshot;
}

export interface UpdateSettingsRequest {
  readonly expectedUserStateRevision: number;
  readonly expectedRevision: number;
  readonly value: LocalSettings;
}

export interface ReplaceSelectionRequest {
  readonly expectedUserStateRevision: number;
  readonly sections: readonly SectionKey[];
}

export interface ReplaceCurrentFiltersRequest {
  readonly expectedUserStateRevision: number;
  readonly expectedCurrentFiltersRevision: number;
  readonly filters: FilterRequestV2;
}

export interface CreateSavedViewRequest extends ReplaceCurrentFiltersRequest {
  readonly name: string;
}

export interface CurrentFiltersCommand {
  readonly expectedUserStateRevision: number;
  readonly expectedCurrentFiltersRevision: number;
}

export interface SavedViewCommand extends CurrentFiltersCommand {
  readonly id: TraceId;
  readonly expectedViewRevision: number;
}

export interface RenameSavedViewRequest extends SavedViewCommand {
  readonly name: string;
}

export interface UpdateSavedViewRequest extends SavedViewCommand {
  readonly filters: FilterRequestV2;
}

export interface DuplicateSavedViewRequest {
  readonly expectedUserStateRevision: number;
  readonly id: TraceId;
  readonly expectedViewRevision: number;
  readonly name: string;
}

export interface PrepareUserDataResetRequest {
  readonly expectedUserStateRevision: number;
}

export interface PreparedUserDataReset {
  readonly confirmationToken: TraceId;
  readonly expectedUserStateRevision: number;
  readonly expiresInSeconds: number;
}

export interface ConfirmUserDataResetRequest {
  readonly confirmationToken: TraceId;
}

export interface PersonalResetResult {
  readonly stateRevision: number;
  readonly deletedSettings: number;
  readonly deletedCurrentFilters: number;
  readonly deletedSavedViews: number;
  readonly deletedSelectedSections: number;
  readonly deletedEpisodeSummaries: number;
  readonly deletedEpisodeActions: number;
}

const CANONICAL_V4_UUID =
  /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/u;

function invalidBootstrap(): never {
  throw new ProductBootstrapError('BOOTSTRAP_INVALID');
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function hasKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  return Object.keys(value).length === keys.length && keys.every((key) => Object.hasOwn(value, key));
}

function isNonnegativeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && typeof value === 'number' && value >= 0;
}

function isPositiveInteger(value: unknown): value is number {
  return isNonnegativeInteger(value) && value > 0;
}

function isNullableNonnegativeInteger(value: unknown): value is number | null {
  return value === null || isNonnegativeInteger(value);
}

function isTraceId(value: unknown): value is TraceId {
  return typeof value === 'string' && CANONICAL_V4_UUID.test(value);
}

function isSectionKey(value: unknown): value is SectionKey {
  return isRecord(value)
    && hasKeys(value, ['term', 'campus', 'index'])
    && typeof value.term === 'string'
    && value.term.length > 0
    && typeof value.campus === 'string'
    && value.campus.length > 0
    && typeof value.index === 'string'
    && value.index.length > 0;
}

function isFilterRequest(value: unknown): value is FilterRequestV2 {
  return isRecord(value)
    && hasKeys(value, ['contractVersion', 'values'])
    && value.contractVersion === 2
    && isRecord(value.values);
}

function isWatchPolicy(value: unknown): value is WatchPolicyV1 {
  if (!isRecord(value) || !hasKeys(value, ['notificationMode', 'maxAudible', 'continuousDuration'])) {
    return false;
  }
  if (!['ONE_SHOT', 'CONTINUOUS'].includes(String(value.notificationMode)) || !isPositiveInteger(value.maxAudible)) {
    return false;
  }
  const duration = value.continuousDuration;
  return isRecord(duration)
    && ((hasKeys(duration, ['kind', 'seconds']) && duration.kind === 'FINITE' && isPositiveInteger(duration.seconds))
      || (hasKeys(duration, ['kind']) && duration.kind === 'UNLIMITED'));
}

function isLocalSettings(value: unknown): value is LocalSettings {
  return isRecord(value)
    && hasKeys(value, [
      'localeOverride',
      'catalogRefreshMinutes',
      'openRefreshSeconds',
      'watchFastLaneSeconds',
      'volumePercent',
      'soundPolicy',
    ])
    && ['system', 'en-US', 'zh-CN'].includes(String(value.localeOverride))
    && isNonnegativeInteger(value.catalogRefreshMinutes)
    && value.catalogRefreshMinutes >= 1
    && value.catalogRefreshMinutes <= 1_440
    && isNonnegativeInteger(value.openRefreshSeconds)
    && value.openRefreshSeconds >= 3
    && value.openRefreshSeconds <= 3_600
    && isNonnegativeInteger(value.watchFastLaneSeconds)
    && value.watchFastLaneSeconds >= 3
    && value.watchFastLaneSeconds <= 60
    && isNonnegativeInteger(value.volumePercent)
    && value.volumePercent <= 100
    && isWatchPolicy(value.soundPolicy);
}

function isStoredSettings(value: unknown): value is StoredSettings {
  return isRecord(value)
    && hasKeys(value, ['revision', 'value'])
    && isNonnegativeInteger(value.revision)
    && isLocalSettings(value.value);
}

function isAssociation(value: unknown): value is FilterAssociation {
  if (!isRecord(value) || typeof value.kind !== 'string') return false;
  return (value.kind === 'CUSTOM' && hasKeys(value, ['kind']))
    || (value.kind === 'APPLIED'
      && hasKeys(value, ['kind', 'viewId', 'revision'])
      && isTraceId(value.viewId)
      && isNonnegativeInteger(value.revision));
}

function isIncompatibility(value: unknown): value is SavedViewIncompatibility {
  if (!isRecord(value) || typeof value.kind !== 'string') return false;
  switch (value.kind) {
    case 'INVALID_ENVELOPE':
    case 'INVALID_FIELD_DATA':
      return hasKeys(value, ['kind']);
    case 'UNSUPPORTED_CODEC_VERSION':
      return hasKeys(value, ['kind', 'observed'])
        && (value.observed === null || isNonnegativeInteger(value.observed));
    case 'FUTURE_SCHEMA_VERSION':
      return hasKeys(value, ['kind', 'observed', 'supported'])
        && isNonnegativeInteger(value.observed)
        && isNonnegativeInteger(value.supported);
    case 'UNKNOWN_FIELD':
    case 'MISSING_REQUIRED_FIELD':
      return hasKeys(value, ['kind', 'stableId']) && typeof value.stableId === 'string';
    default:
      return false;
  }
}

function isSavedViewContent(value: unknown): value is SavedViewContent {
  if (!isRecord(value) || typeof value.status !== 'string') return false;
  return (value.status === 'COMPATIBLE'
      && hasKeys(value, ['status', 'filters'])
      && isFilterRequest(value.filters))
    || (value.status === 'INCOMPATIBLE'
      && hasKeys(value, ['status', 'rawSnapshot', 'reason'])
      && isIncompatibility(value.reason));
}

function isCurrentFilters(value: unknown): value is CurrentFilters {
  return isRecord(value)
    && hasKeys(value, ['association', 'content'])
    && isAssociation(value.association)
    && isSavedViewContent(value.content);
}

function isStoredCurrentFilters(value: unknown): value is StoredCurrentFilters {
  return isRecord(value)
    && hasKeys(value, ['stateRevision', 'revision', 'value'])
    && isNonnegativeInteger(value.stateRevision)
    && isNonnegativeInteger(value.revision)
    && (value.value === null || isCurrentFilters(value.value));
}

function isSavedViewDefinition(value: unknown): value is SavedViewDefinition {
  return isRecord(value)
    && hasKeys(value, [
      'id',
      'name',
      'schemaVersion',
      'revision',
      'content',
      'createdAt',
      'updatedAt',
    ])
    && isTraceId(value.id)
    && typeof value.name === 'string'
    && isNonnegativeInteger(value.schemaVersion)
    && isNonnegativeInteger(value.revision)
    && isSavedViewContent(value.content)
    && isNonnegativeInteger(value.createdAt)
    && isNonnegativeInteger(value.updatedAt);
}

function isDisposition(value: unknown): value is EpisodeDisposition {
  if (!isRecord(value) || typeof value.kind !== 'string') return false;
  if (['SECTION_CLOSED', 'ACKNOWLEDGED', 'TIMED_OUT'].includes(value.kind)) {
    return hasKeys(value, ['kind']);
  }
  return value.kind === 'WATCH_STOPPED'
    && hasKeys(value, ['kind', 'reason'])
    && ['USER_REQUESTED', 'CONNECTION_CLOSED', 'HEARTBEAT_TIMEOUT', 'SERVICE_STOPPING', 'TERM_OUT_OF_RANGE']
      .includes(String(value.reason));
}

function isHistorySummary(value: unknown): value is EpisodeHistorySummary {
  if (!isRecord(value) || !hasKeys(value, [
    'identity',
    'state',
    'mode',
    'firstObservedAt',
    'lastObservedAt',
    'acknowledgedAt',
    'timedOutAt',
    'closedAt',
    'disposition',
    'audibleCount',
    'observationCount',
    'lastObservationId',
    'actionCount',
  ])) return false;
  const identity = value.identity;
  return isRecord(identity)
    && hasKeys(identity, ['sectionKey', 'runId', 'episodeId'])
    && isSectionKey(identity.sectionKey)
    && isTraceId(identity.runId)
    && isTraceId(identity.episodeId)
    && ['UNACKNOWLEDGED', 'ACKNOWLEDGED', 'TIMED_OUT', 'CLOSED'].includes(String(value.state))
    && ['ONE_SHOT', 'CONTINUOUS'].includes(String(value.mode))
    && isNonnegativeInteger(value.firstObservedAt)
    && isNonnegativeInteger(value.lastObservedAt)
    && isNullableNonnegativeInteger(value.acknowledgedAt)
    && isNullableNonnegativeInteger(value.timedOutAt)
    && isNullableNonnegativeInteger(value.closedAt)
    && (value.disposition === null || isDisposition(value.disposition))
    && isNonnegativeInteger(value.audibleCount)
    && isPositiveInteger(value.observationCount)
    && isTraceId(value.lastObservationId)
    && isNonnegativeInteger(value.actionCount);
}

function isHistoryPage(value: unknown): value is HistoryPage<EpisodeHistorySummary> {
  return isRecord(value)
    && hasKeys(value, ['items', 'total', 'offset', 'limit'])
    && Array.isArray(value.items)
    && value.items.every(isHistorySummary)
    && isNonnegativeInteger(value.total)
    && isNonnegativeInteger(value.offset)
    && isPositiveInteger(value.limit);
}

function isPersonalStateSnapshot(value: unknown): value is PersonalStateSnapshot {
  return isRecord(value)
    && hasKeys(value, [
      'stateRevision',
      'settings',
      'currentFilters',
      'savedViews',
      'selectedSections',
      'episodeHistory',
      'activeWatchCount',
    ])
    && isNonnegativeInteger(value.stateRevision)
    && isStoredSettings(value.settings)
    && isStoredCurrentFilters(value.currentFilters)
    && Array.isArray(value.savedViews)
    && value.savedViews.every(isSavedViewDefinition)
    && Array.isArray(value.selectedSections)
    && value.selectedSections.every(isSectionKey)
    && isHistoryPage(value.episodeHistory)
    && isNonnegativeInteger(value.activeWatchCount)
    && value.activeWatchCount <= 9;
}

export function parseLocalBootstrapData(value: unknown): LocalBootstrapData {
  if (!isRecord(value)
    || !hasKeys(value, ['mode', 'sessionNonce', 'state'])
    || value.mode !== 'LOCAL'
    || !isTraceId(value.sessionNonce)
    || !isPersonalStateSnapshot(value.state)) {
    return invalidBootstrap();
  }
  return {
    mode: value.mode,
    sessionNonce: value.sessionNonce,
    state: value.state,
  };
}

export function parseLocalBootstrapEnvelope(value: unknown): LocalBootstrapData {
  if (!isRecord(value)
    || !hasKeys(value, ['protocolVersion', 'data'])
    || value.protocolVersion !== 1) {
    return invalidBootstrap();
  }
  return parseLocalBootstrapData(value.data);
}
