//! Shared, target-neutral wire contracts.
//!
//! This crate owns validated external identities, protocol versions, stable
//! public error codes, and typed HTTP/WebSocket envelopes. Product semantics
//! stay in `bcsp-domain`; target-specific routes and host policy stay in the
//! adapter crates.

#![forbid(unsafe_code)]
#![deny(warnings)]

mod envelope;
mod error;
mod identity;
mod match_contract;
mod protocol;
mod schema;

pub use envelope::{HttpRequestEnvelope, HttpSuccessEnvelope, WsClientEnvelope, WsServerEnvelope};
pub use error::{
    ApiErrorBody, ApiErrorCode, ApiErrorDetail, ApiErrorEnvelope, ContractDecodeError, DetailName,
    StableErrorCode, StableErrorCodeSet, TraceId, TraceIdError, TraceIdSource, TypedApiErrorBody,
    TypedApiErrorEnvelope, decode_versioned_envelope_json,
};
pub use identity::{
    CampusCode, CourseGroupKey, CourseString, CourseVariantKey, IdentityError, IdentityErrorCode,
    SectionIndex, SectionKey, TermCampusKey, TermId, VariantFingerprint,
};
pub use match_contract::{
    MatchExplanation, MatchExplanationError, MatchOutcome, MatchReason, MatchReasonCode,
    ReasonField,
};
pub use protocol::{
    API_PROTOCOL_VERSION, ProtocolVersion, ProtocolVersionError, WS_PROTOCOL_VERSION,
};
pub use schema::{
    CONTRACT_SCHEMA_VERSION, ContractField, ContractManifest, ContractSchema, ContractVariant,
    ScalarConstraint, SchemaDirection, UnknownFieldPolicy, contract_manifest,
};

pub const PACKAGE_BOUNDARY: &str = "bcsp-contracts";

mod dependency_contract {
    use serde as _;
    use serde_json as _;
    use thiserror as _;
    use time as _;
    use uuid as _;
}
