//! Shared, target-neutral wire contracts.
//!
//! This crate owns validated external identities, protocol versions, stable
//! public error codes, and typed HTTP/WebSocket envelopes. Product semantics
//! stay in `bcsp-domain`; target-specific routes and host policy stay in the
//! adapter crates.

#![forbid(unsafe_code)]
#![deny(warnings)]

mod catalog;
mod envelope;
mod error;
mod identity;
mod match_contract;
mod open;
mod protocol;
mod query;
mod schema;
mod service;
mod watch;

pub use catalog::{
    CATALOG_CONTRACT_VERSION, CatalogCommentV1, CatalogContentVersion, CatalogContentVersionError,
    CatalogContractVersion, CatalogContractVersionError, CatalogCoreCodeDictionaryV1,
    CatalogCoreCodeOptionV1, CatalogDiagnosticCode, CatalogDiagnosticCodeError,
    CatalogDiscoveryAvailability, CatalogDiscoveryErrorClass, CatalogDiscoveryErrorV1,
    CatalogDiscoveryPointV1, CatalogDiscoveryProvenanceV1, CatalogDiscoveryRequestV1,
    CatalogDiscoveryResponseV1, CatalogDiscoverySourceId, CatalogDiscoverySourceIdError,
    CatalogDiscoverySourceKind, CatalogDiscoverySourceV1, CatalogDiscoveryStatusV1,
    CatalogEntityCountsV1, CatalogFieldKnowledge, CatalogFieldPresence,
    CatalogInstructorReliability, CatalogModality, CatalogOccurrenceEvidence,
    CatalogOccurrenceKeyV1, CatalogOccurrenceKind, CatalogOpenStatusProvenance,
    CatalogPayloadDigest, CatalogPayloadDigestError, CatalogPrerequisiteState, CatalogProvenanceV1,
    CatalogRefreshCheckpointPointV1, CatalogRefreshCheckpointV1, CatalogRefreshClassification,
    CatalogRefreshErrorClass, CatalogRefreshObservationV1, CatalogRefreshPointV1,
    CatalogRefreshStatusV1, CatalogRequiredness, CatalogSnapshotOpenStatusV1, CatalogSourceKind,
    CatalogSubjectCode, CatalogSubjectCodeError, CatalogSubjectProvenanceV1, CatalogSubjectV1,
    CatalogSynchronicity, CatalogTargetV1, CatalogTimeKnowledgeV1, CatalogUnitMajorV1,
    CatalogUnknownReason, NormalizedCatalogV1, NormalizedCourseGroupV1, NormalizedCourseVariantV1,
    NormalizedOccurrenceV1, NormalizedSectionV1,
};

pub use envelope::{HttpRequestEnvelope, HttpSuccessEnvelope, WsClientEnvelope, WsServerEnvelope};
pub use error::{
    ApiErrorBody, ApiErrorCode, ApiErrorDetail, ApiErrorEnvelope, ContractDecodeError, DetailName,
    StableErrorCode, StableErrorCodeSet, SystemTraceIdSource, TraceId, TraceIdError, TraceIdSource,
    TypedApiErrorBody, TypedApiErrorEnvelope, decode_versioned_envelope_json,
};
pub use identity::{
    CampusCode, CourseGroupKey, CourseString, CourseVariantKey, IdentityError, IdentityErrorCode,
    SectionIndex, SectionKey, TermCampusKey, TermId, VariantFingerprint,
};
pub use match_contract::{
    MatchExplanation, MatchExplanationError, MatchOutcome, MatchReason, MatchReasonCode,
    ReasonField,
};
pub use open::{
    OPEN_CONTRACT_VERSION, OpenAppliedClassification, OpenAttemptPointV1, OpenAttemptScheduleV1,
    OpenBatchKey, OpenCanonicalSetHash, OpenCanonicalSetHashError, OpenCircuitReason,
    OpenCircuitState, OpenCircuitStatusV1, OpenContractVersion, OpenContractVersionError,
    OpenCounterSnapshotV1, OpenDecodedBodySha256, OpenDecodedBodySha256Error, OpenFailureClass,
    OpenFailurePointV1, OpenFreshnessState, OpenFreshnessV1, OpenHttpHeaderValue,
    OpenHttpHeaderValueError, OpenHttpMetadataV1, OpenObservationPointV1,
    OpenObservationTargetMismatchError, OpenObservationV1, OpenPullAttemptV1, OpenPullCountsV1,
    OpenReconcileCountsV1, OpenRefreshClassification, OpenRefreshObservationV1,
    OpenRefreshStatusV1, OpenSchedulerLane, OpenSchedulerStatusV1, OpenSectionStatusRequestV1,
    OpenSectionStatusV1, OpenSequence, OpenSequenceError, OpenState, OpenStateHash,
    OpenStateHashError, OpenStatusRequestV1, OpenUncertaintyReason, RutgersDay, RutgersDayError,
    RutgersDayTimezone,
};
pub use protocol::{
    API_PROTOCOL_VERSION, ProtocolVersion, ProtocolVersionError, WS_PROTOCOL_VERSION,
};
pub use query::{
    AvailabilityWindowV1, CoreFilterV1, CourseDetailRequestV1, CourseDetailResponseV1,
    CourseQueryItemV1, CourseQueryRequestV1, CourseQueryRequestV2, CourseQueryRequestV3,
    CourseQueryResponseV1, CourseQueryResponseV2, CourseQueryResponseV3, CourseSortFieldV1,
    CourseSortV1, CourseVariantQueryItemV1, CreditRangeV1, DEFAULT_FILTER_OPTIONS_LIMIT,
    DEFAULT_PAGE_SIZE, DynamicFilterValidationRequestV3, DynamicFilterValidationResponseV3,
    FILTER_FIELD_COUNT, FilterCanonicalNeutralV1, FilterFieldId, FilterFieldSchemaV1,
    FilterFieldSchemaV2, FilterFieldSchemaV3, FilterMatchV1, FilterNormalizationV1,
    FilterOptionTargetVersionV2, FilterOptionV2, FilterOptionsFieldV2, FilterOptionsRequestError,
    FilterOptionsRequestV2, FilterOptionsResponseV2, FilterQueryEncodingV1, FilterRequestV1,
    FilterRequestV2, FilterRequestV3, FilterSchemaV1, FilterSchemaV2, FilterSchemaV3,
    FilterSchemaValueV1, FilterScopeV1, FilterSearchTextError, FilterSearchTextV1, FilterSetModeV1,
    FilterTokenError, FilterTokenV1, FilterValidationV1, FilterValueError, FilterValueKindV1,
    FilterValuesInputV1, FilterValuesInputV2, FilterValuesInputV3, IncludeIncompleteV3,
    InvalidDynamicFilterValueV3, LiveOpenEvidenceV1, LiveOpenStateV1, MAX_AVAILABILITY_WINDOWS,
    MAX_FILTER_OPTIONS_LIMIT, MAX_FILTER_VALUES_PER_FIELD, MAX_PAGE_SIZE, MAX_TOTAL_FILTER_VALUES,
    MeetingLocationFilterV2, MeetingLocationMatchModeV2, NormalizedFilterValuesV1,
    NormalizedFilterValuesV2, NormalizedFilterValuesV3, PageInfoV1, PageRequestV1,
    PermissionFilterV1, PrerequisiteFilterV1, QUERY_CONTRACT_VERSION, QueryContractVersion,
    QueryContractVersionError, SectionDetailRequestV1, SectionDetailResponseV1, SectionQueryItemV1,
    SectionQueryRequestV1, SectionQueryRequestV2, SectionQueryRequestV3, SectionQueryResponseV1,
    SectionQueryResponseV2, SectionQueryResponseV3, SectionSearchItemV1, SectionSortFieldV1,
    SectionSortV1, SortDirectionV1, TextMatchEvidenceV1, UserModalityV3, UserSynchronicityV3,
    WeekdayV1, course_number_band, filter_schema_v1, filter_schema_v2, filter_schema_v3,
};
pub use schema::{
    CONTRACT_SCHEMA_VERSION, ContractField, ContractManifest, ContractSchema, ContractVariant,
    ScalarConstraint, SchemaDirection, UnknownFieldPolicy, contract_manifest,
};
pub use service::{
    SERVICE_STATUS_CONTRACT_VERSION, SERVICE_STATUS_V2_CONTRACT_VERSION,
    ServiceAutomaticTermSummaryV2, ServiceAvailabilitySummaryV1, ServiceAvailabilityV1,
    ServiceIssueComponentV1, ServiceIssueRecoveryV1, ServiceIssueSeverityV1, ServiceIssueV1,
    ServiceLevelV1, ServiceOperationPhaseV1, ServiceOperationStageV2, ServiceOperationV1,
    ServiceOperationV2, ServiceRuntimeV1, ServiceSnapshotAvailabilityV2, ServiceStatusV1,
    ServiceStatusV2, ServiceTargetErrorV2, ServiceTargetStatusV1, ServiceTargetStatusV2,
    ServiceTermPublicationV2, ServiceTermWindowV2, ServiceVisibleTermV2, ServiceWorkStateV2,
};
pub use watch::{
    ACTIVE_WATCH_STATE_PERSISTENT, ActiveWatchId, ActiveWatchTargetV1,
    DEFAULT_CONTINUOUS_DURATION_SECONDS, DEFAULT_MAX_AUDIBLE, MAX_ACTIVE_WATCHES, OpenEpisodeId,
    OpenEpisodeState, OpenEpisodeV1, OpenEpisodeValidationError, WATCH_CONTRACT_VERSION,
    WatchAlertDisposition, WatchAlertId, WatchAlertTargetV1, WatchAlertV1, WatchAudioCueV1,
    WatchAudioDispositionV1, WatchAudioTriggerV1, WatchClientCommandV1, WatchContinuousDurationV1,
    WatchContinuousMixerStopReason, WatchContractVersion, WatchContractVersionError,
    WatchCueCancellationReason, WatchCueId, WatchCueOutcome, WatchCueOutcomeReceiptV1,
    WatchCueOutcomeReportV1, WatchEpisodeTargetV1, WatchMaxAudible, WatchNotificationMode,
    WatchOpenObservationFanoutV1, WatchPolicyV1, WatchPositiveValueError, WatchServerEventV1,
    WatchStartItemResultV1, WatchStartItemV1, WatchStartItemsError, WatchStartItemsV1,
    WatchStartRejectionReason, WatchStartResultError, WatchStartResultV1, WatchStopReason,
    WatchStoppedV1,
};

pub const PACKAGE_BOUNDARY: &str = "bcsp-contracts";

mod dependency_contract {
    use serde as _;
    use serde_json as _;
    use thiserror as _;
    use time as _;
    use uuid as _;
}
