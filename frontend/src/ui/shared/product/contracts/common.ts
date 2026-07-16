export type ProtocolVersionV1 = 1;
export type ContractVersionV1 = 1;
export type QueryContractVersion = 1 | 2;
export type TraceId = string;
export type IsoDateTime = string;

export interface TermCampusKey {
  readonly term: string;
  readonly campus: string;
}

export interface SectionKey extends TermCampusKey {
  readonly index: string;
}

export interface CourseGroupKey extends TermCampusKey {
  readonly courseString: string;
}

export interface CourseVariantKey {
  readonly group: CourseGroupKey;
  readonly fingerprint: string;
}

export interface HttpRequestEnvelope<T> {
  readonly protocolVersion: ProtocolVersionV1;
  readonly payload: T;
}

export interface HttpSuccessEnvelope<T> {
  readonly protocolVersion: ProtocolVersionV1;
  readonly data: T;
}

export type ApiErrorCode =
  | 'MALFORMED_REQUEST'
  | 'UNSUPPORTED_PROTOCOL_VERSION'
  | 'INVALID_SECTION_KEY'
  | 'INVALID_COURSE_GROUP_KEY'
  | 'INVALID_COURSE_VARIANT_KEY'
  | 'INVALID_FILTER'
  | 'INVALID_FILTER_OPTION'
  | 'SECTION_NOT_FOUND'
  | 'SELECTION_LIMIT_EXCEEDED'
  | 'CATALOG_NOT_READY'
  | 'SEARCH_DATA_NOT_READY'
  | 'UPSTREAM_UNAVAILABLE'
  | 'AUDIO_BLOCKED'
  | 'INTERNAL_ERROR';

export type ApiErrorDetail =
  | { readonly kind: 'INVALID_FIELD'; readonly field: string }
  | { readonly kind: 'LIMIT'; readonly name: string; readonly maximum: number }
  | { readonly kind: 'RETRY_AFTER_SECONDS'; readonly seconds: number }
  | { readonly kind: 'CURRENT_REVISION'; readonly revision: number };

export interface ApiErrorBody {
  readonly code: ApiErrorCode;
  readonly messageKey: string;
  readonly traceId: TraceId;
  readonly details: readonly ApiErrorDetail[];
}

export interface ApiErrorEnvelope {
  readonly protocolVersion: ProtocolVersionV1;
  readonly error: ApiErrorBody;
}

export interface WsClientEnvelope<T> {
  readonly protocolVersion: ProtocolVersionV1;
  readonly messageId: TraceId;
  readonly payload: T;
}

export interface WsServerEnvelope<T> {
  readonly protocolVersion: ProtocolVersionV1;
  readonly messageId: TraceId;
  readonly payload: T;
}

export type CatalogUnknownReason =
  | 'MISSING'
  | 'EXPLICIT_NULL'
  | 'MALFORMED'
  | 'SPARSE_EVIDENCE'
  | 'NOT_OBSERVED'
  | 'SOURCE_UNAVAILABLE'
  | 'CONFLICTING_EVIDENCE';

export type CatalogFieldPresence<T> =
  | { readonly presence: 'PRESENT'; readonly value: T }
  | { readonly presence: 'EXPLICIT_NULL' }
  | { readonly presence: 'ABSENT' };

export type CatalogFieldKnowledge<T> =
  | { readonly knowledge: 'KNOWN'; readonly presence: CatalogFieldPresence<T> }
  | { readonly knowledge: 'UNKNOWN'; readonly reason: CatalogUnknownReason };

export type MatchOutcome = 'MATCH' | 'UNCERTAIN' | 'NO_MATCH';
export type MatchReasonCode =
  | 'KNOWN_MISMATCH'
  | 'MISSING_RELIABLE_DATA'
  | 'UNKNOWN_VALUE'
  | 'TBA'
  | 'CONFLICTING_EVIDENCE'
  | 'INVALID_VALUE'
  | 'SOURCE_UNAVAILABLE';

export interface MatchReason {
  readonly code: MatchReasonCode;
  readonly field: string;
}

export interface MatchExplanation {
  readonly outcome: MatchOutcome;
  readonly reasons: readonly MatchReason[];
}
