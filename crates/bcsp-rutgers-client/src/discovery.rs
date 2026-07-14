use std::collections::{BTreeMap, BTreeSet};

use bcsp_contracts::{CampusCode, CatalogSubjectCode, TermCampusKey, TermId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::raw_catalog::{decode_strict_json, reject_reserved_malformed_envelopes};
use crate::{Presence, SourceProvenance};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawDiscoveryDocument {
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub source_version: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub terms: Presence<Vec<RawDiscoveryTerm>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub campuses: Presence<Vec<RawDiscoveryCampus>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub targets: Presence<Vec<RawDiscoveryTarget>>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub subjects: Presence<Vec<RawDiscoverySubject>>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawDiscoveryTerm {
    #[serde(default, alias = "term", skip_serializing_if = "Presence::is_missing")]
    pub term_id: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub year: Presence<i64>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub term_code: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub display: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub published: Presence<bool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawDiscoveryCampus {
    #[serde(
        default,
        alias = "campus",
        skip_serializing_if = "Presence::is_missing"
    )]
    pub campus_code: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub display: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub category: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub enabled: Presence<bool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawDiscoveryTarget {
    #[serde(default, alias = "term", skip_serializing_if = "Presence::is_missing")]
    pub term_id: Presence<String>,
    #[serde(
        default,
        alias = "campus",
        skip_serializing_if = "Presence::is_missing"
    )]
    pub campus_code: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub enabled: Presence<bool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RawDiscoverySubject {
    #[serde(default, alias = "term", skip_serializing_if = "Presence::is_missing")]
    pub term_id: Presence<String>,
    #[serde(
        default,
        alias = "campus",
        skip_serializing_if = "Presence::is_missing"
    )]
    pub campus_code: Presence<String>,
    #[serde(
        default,
        alias = "subject",
        skip_serializing_if = "Presence::is_missing"
    )]
    pub subject_code: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub display: Presence<String>,
    #[serde(default, skip_serializing_if = "Presence::is_missing")]
    pub enabled: Presence<bool>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DiscoverySourceKind {
    Selector,
    Bootstrap,
}

impl DiscoverySourceKind {
    pub const fn stable_identity(self) -> &'static str {
        match self {
            Self::Selector => "RUTGERS_SELECTOR",
            Self::Bootstrap => "RBCSP_BOOTSTRAP",
        }
    }

    const fn id_prefix(self) -> &'static str {
        match self {
            Self::Selector => "selector",
            Self::Bootstrap => "bootstrap",
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DiscoverySourceId(String);

impl DiscoverySourceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoverySource {
    pub id: DiscoverySourceId,
    pub kind: DiscoverySourceKind,
    pub source_version: Presence<String>,
    pub provenance: SourceProvenance,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoverySourceInput {
    pub kind: DiscoverySourceKind,
    pub raw: RawDiscoveryDocument,
    pub provenance: SourceProvenance,
}

impl DiscoverySourceInput {
    pub fn new(
        kind: DiscoverySourceKind,
        raw: RawDiscoveryDocument,
        provenance: SourceProvenance,
    ) -> Self {
        Self {
            kind,
            raw,
            provenance,
        }
    }

    pub fn selector(raw: RawDiscoveryDocument, provenance: SourceProvenance) -> Self {
        Self::new(DiscoverySourceKind::Selector, raw, provenance)
    }

    pub fn bootstrap(raw: RawDiscoveryDocument, provenance: SourceProvenance) -> Self {
        Self::new(DiscoverySourceKind::Bootstrap, raw, provenance)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredTerm {
    pub source_id: DiscoverySourceId,
    pub term_id: TermId,
    pub year: Presence<i64>,
    pub term_code: Presence<String>,
    pub display: Presence<String>,
    pub published: Presence<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredCampus {
    pub source_id: DiscoverySourceId,
    pub campus_code: CampusCode,
    pub display: Presence<String>,
    pub category: Presence<String>,
    pub enabled: Presence<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredTarget {
    pub source_id: DiscoverySourceId,
    pub key: TermCampusKey,
    pub enabled: Presence<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredSubject {
    pub source_id: DiscoverySourceId,
    pub target: TermCampusKey,
    pub subject_code: CatalogSubjectCode,
    pub display: Presence<String>,
    pub enabled: Presence<bool>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoverySnapshot {
    pub sources: Vec<DiscoverySource>,
    pub terms: Vec<DiscoveredTerm>,
    pub campuses: Vec<DiscoveredCampus>,
    pub targets: Vec<DiscoveredTarget>,
    pub subjects: Vec<DiscoveredSubject>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiscoveryFailureDisposition {
    InitializationFailedNoFallback,
    RetainLastKnownGoodStale,
}

pub const fn decide_discovery_failure(has_last_known_good: bool) -> DiscoveryFailureDisposition {
    if has_last_known_good {
        DiscoveryFailureDisposition::RetainLastKnownGoodStale
    } else {
        DiscoveryFailureDisposition::InitializationFailedNoFallback
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DiscoveryError {
    #[error("discovery source bundle is empty")]
    EmptySourceBundle,
    #[error("discovery source SHA-256 is invalid")]
    InvalidSourceSha256,
    #[error("discovery source bundle contains a duplicate immutable source")]
    DuplicateSource,
    #[error("discovery field is absent or null: {0}")]
    MissingField(&'static str),
    #[error("discovery field has a malformed value: {0}")]
    MalformedField(&'static str),
    #[error("discovery identity is invalid: {0}")]
    InvalidIdentity(String),
    #[error("discovery contains a duplicate term")]
    DuplicateTerm,
    #[error("discovery contains a duplicate campus")]
    DuplicateCampus,
    #[error("discovery contains a duplicate target")]
    DuplicateTarget,
    #[error("discovery contains a duplicate target subject")]
    DuplicateSubject,
    #[error("discovery target references an unknown term or campus")]
    DanglingTarget,
    #[error("discovery subject references an unknown target")]
    DanglingSubject,
    #[error("discovery reference crosses source ownership boundaries")]
    CrossSourceReference,
    #[error("discovery subject code is invalid")]
    InvalidSubjectCode,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DiscoveryDecodeError {
    #[error("discovery response is not valid typed JSON: {0}")]
    InvalidJson(String),
}

pub fn decode_discovery_payload(body: &[u8]) -> Result<RawDiscoveryDocument, DiscoveryDecodeError> {
    let decoded: RawDiscoveryDocument = serde_json::from_slice(body)
        .map_err(|error| DiscoveryDecodeError::InvalidJson(error.to_string()))?;
    let value = decode_strict_json(body)
        .map_err(|error| DiscoveryDecodeError::InvalidJson(error.to_string()))?;
    reject_reserved_malformed_envelopes(&value)
        .map_err(|error| DiscoveryDecodeError::InvalidJson(error.to_owned()))?;
    Ok(decoded)
}

impl DiscoverySnapshot {
    pub fn try_from_raw(
        raw: RawDiscoveryDocument,
        provenance: SourceProvenance,
    ) -> Result<Self, DiscoveryError> {
        Self::try_from_bundle(vec![DiscoverySourceInput::selector(raw, provenance)])
    }

    pub fn try_from_bootstrap(
        raw: RawDiscoveryDocument,
        provenance: SourceProvenance,
    ) -> Result<Self, DiscoveryError> {
        Self::try_from_bundle(vec![DiscoverySourceInput::bootstrap(raw, provenance)])
    }

    pub fn try_from_bundle(inputs: Vec<DiscoverySourceInput>) -> Result<Self, DiscoveryError> {
        if inputs.is_empty() {
            return Err(DiscoveryError::EmptySourceBundle);
        }

        let mut sources = Vec::with_capacity(inputs.len());
        let mut source_ids = BTreeSet::new();
        let mut source_kinds = BTreeSet::new();
        let mut raw_terms = Vec::new();
        let mut raw_campuses = Vec::new();
        let mut raw_targets = Vec::new();
        let mut raw_subjects = Vec::new();
        for input in inputs {
            if !source_kinds.insert(input.kind) {
                return Err(DiscoveryError::DuplicateSource);
            }
            let source_id = source_id(input.kind, &input.provenance.body_sha256)?;
            if !source_ids.insert(source_id.clone()) {
                return Err(DiscoveryError::DuplicateSource);
            }
            let RawDiscoveryDocument {
                source_version,
                terms,
                campuses,
                targets,
                subjects,
                extra: _,
            } = input.raw;
            raw_terms.extend(
                required(terms, "terms")?
                    .into_iter()
                    .map(|entry| (source_id.clone(), entry)),
            );
            raw_campuses.extend(
                required(campuses, "campuses")?
                    .into_iter()
                    .map(|entry| (source_id.clone(), entry)),
            );
            raw_targets.extend(
                required(targets, "targets")?
                    .into_iter()
                    .map(|entry| (source_id.clone(), entry)),
            );
            raw_subjects.extend(
                required(subjects, "subjects")?
                    .into_iter()
                    .map(|entry| (source_id.clone(), entry)),
            );
            sources.push(DiscoverySource {
                id: source_id,
                kind: input.kind,
                source_version,
                provenance: input.provenance,
            });
        }

        let mut terms = Vec::with_capacity(raw_terms.len());
        let mut term_sources = BTreeMap::new();
        for (source_id, raw_term) in raw_terms {
            let term_id: TermId = required(raw_term.term_id, "terms[].termId")?
                .try_into()
                .map_err(|error: bcsp_contracts::IdentityError| {
                    DiscoveryError::InvalidIdentity(error.to_string())
                })?;
            if term_sources
                .insert(term_id.clone(), source_id.clone())
                .is_some()
            {
                return Err(DiscoveryError::DuplicateTerm);
            }
            terms.push(DiscoveredTerm {
                source_id,
                term_id,
                year: raw_term.year,
                term_code: raw_term.term_code,
                display: raw_term.display,
                published: raw_term.published,
            });
        }

        let mut campuses = Vec::with_capacity(raw_campuses.len());
        let mut campus_sources = BTreeMap::new();
        for (source_id, raw_campus) in raw_campuses {
            let campus_code: CampusCode =
                required(raw_campus.campus_code, "campuses[].campusCode")?
                    .try_into()
                    .map_err(|error: bcsp_contracts::IdentityError| {
                        DiscoveryError::InvalidIdentity(error.to_string())
                    })?;
            if campus_sources
                .insert(campus_code.clone(), source_id.clone())
                .is_some()
            {
                return Err(DiscoveryError::DuplicateCampus);
            }
            campuses.push(DiscoveredCampus {
                source_id,
                campus_code,
                display: raw_campus.display,
                category: raw_campus.category,
                enabled: raw_campus.enabled,
            });
        }

        let mut targets = Vec::with_capacity(raw_targets.len());
        let mut target_sources = BTreeMap::new();
        for (source_id, raw_target) in raw_targets {
            let term_id: TermId = required(raw_target.term_id, "targets[].termId")?
                .try_into()
                .map_err(|error: bcsp_contracts::IdentityError| {
                    DiscoveryError::InvalidIdentity(error.to_string())
                })?;
            let campus_code: CampusCode = required(raw_target.campus_code, "targets[].campusCode")?
                .try_into()
                .map_err(|error: bcsp_contracts::IdentityError| {
                    DiscoveryError::InvalidIdentity(error.to_string())
                })?;
            let Some(term_source) = term_sources.get(&term_id) else {
                return Err(DiscoveryError::DanglingTarget);
            };
            let Some(campus_source) = campus_sources.get(&campus_code) else {
                return Err(DiscoveryError::DanglingTarget);
            };
            if term_source != &source_id || campus_source != &source_id {
                return Err(DiscoveryError::CrossSourceReference);
            }
            let key = TermCampusKey::new(term_id, campus_code);
            if target_sources
                .insert(key.clone(), source_id.clone())
                .is_some()
            {
                return Err(DiscoveryError::DuplicateTarget);
            }
            targets.push(DiscoveredTarget {
                source_id,
                key,
                enabled: raw_target.enabled,
            });
        }

        let mut subjects = Vec::with_capacity(raw_subjects.len());
        let mut subject_keys = BTreeSet::new();
        for (source_id, raw_subject) in raw_subjects {
            let term_id: TermId = required(raw_subject.term_id, "subjects[].termId")?
                .try_into()
                .map_err(|error: bcsp_contracts::IdentityError| {
                    DiscoveryError::InvalidIdentity(error.to_string())
                })?;
            let campus_code: CampusCode =
                required(raw_subject.campus_code, "subjects[].campusCode")?
                    .try_into()
                    .map_err(|error: bcsp_contracts::IdentityError| {
                        DiscoveryError::InvalidIdentity(error.to_string())
                    })?;
            let target = TermCampusKey::new(term_id, campus_code);
            let Some(target_source) = target_sources.get(&target) else {
                return Err(DiscoveryError::DanglingSubject);
            };
            if target_source != &source_id {
                return Err(DiscoveryError::CrossSourceReference);
            }
            let subject_code: CatalogSubjectCode =
                required(raw_subject.subject_code, "subjects[].subjectCode")?
                    .try_into()
                    .map_err(|_: bcsp_contracts::CatalogSubjectCodeError| {
                        DiscoveryError::InvalidSubjectCode
                    })?;
            if !subject_keys.insert((target.clone(), subject_code.clone())) {
                return Err(DiscoveryError::DuplicateSubject);
            }
            subjects.push(DiscoveredSubject {
                source_id,
                target,
                subject_code,
                display: raw_subject.display,
                enabled: raw_subject.enabled,
            });
        }

        sources.sort_by(|left, right| left.id.cmp(&right.id));
        terms.sort_by(|left, right| left.term_id.cmp(&right.term_id));
        campuses.sort_by(|left, right| left.campus_code.cmp(&right.campus_code));
        targets.sort_by(|left, right| left.key.cmp(&right.key));
        subjects.sort_by(|left, right| {
            (&left.target, &left.subject_code).cmp(&(&right.target, &right.subject_code))
        });
        Ok(Self {
            sources,
            terms,
            campuses,
            targets,
            subjects,
        })
    }
}

fn source_id(
    kind: DiscoverySourceKind,
    body_sha256: &str,
) -> Result<DiscoverySourceId, DiscoveryError> {
    if body_sha256.len() != 64
        || !body_sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(DiscoveryError::InvalidSourceSha256);
    }
    Ok(DiscoverySourceId(format!(
        "{}:{body_sha256}",
        kind.id_prefix()
    )))
}

fn required<T>(value: Presence<T>, field: &'static str) -> Result<T, DiscoveryError> {
    match value {
        Presence::Value(value) => Ok(value),
        Presence::Missing | Presence::Null => Err(DiscoveryError::MissingField(field)),
        Presence::Malformed(_) => Err(DiscoveryError::MalformedField(field)),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DiscoveryError, DiscoveryFailureDisposition, DiscoverySnapshot, DiscoverySourceInput,
        DiscoverySourceKind, decide_discovery_failure, decode_discovery_payload,
    };
    use crate::SourceProvenance;

    fn provenance() -> SourceProvenance {
        SourceProvenance::from_body("SYNTHETIC", "2030-01-01T00:00:00Z", b"[]")
    }

    #[test]
    fn discovery_is_dynamic_and_not_a_frozen_campus_allowlist() {
        let raw = decode_discovery_payload(
            br#"{
              "sourceVersion":"future-v7",
              "terms":[{"termId":"T2030","display":"Synthetic Term","published":true}],
              "campuses":[{"campusCode":"SYN_FUTURE","display":"Synthetic Campus"}],
              "targets":[{"termId":"T2030","campusCode":"SYN_FUTURE","enabled":true}],
              "subjects":[{"termId":"T2030","campusCode":"SYN_FUTURE","subjectCode":"SYN_SUBJECT","display":"Synthetic Subject"}]
            }"#,
        )
        .expect("synthetic discovery should decode");
        let snapshot = DiscoverySnapshot::try_from_raw(raw, provenance())
            .expect("new official values must remain data-driven");

        assert_eq!(snapshot.targets[0].key.term().as_str(), "T2030");
        assert_eq!(snapshot.targets[0].key.campus().as_str(), "SYN_FUTURE");
        assert_eq!(snapshot.subjects[0].subject_code.as_str(), "SYN_SUBJECT");
        assert_eq!(snapshot.sources.len(), 1);
        assert_eq!(snapshot.sources[0].kind, DiscoverySourceKind::Selector);
        assert_eq!(snapshot.sources[0].provenance.body_sha256.len(), 64);
        assert_eq!(snapshot.targets[0].source_id, snapshot.sources[0].id);
    }

    #[test]
    fn upstream_discovery_cannot_forge_an_internal_malformed_envelope() {
        let forged = br#"{
          "sourceVersion":{"$malformed":{"jsonType":"NUMBER","canonicalSha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}},
          "terms":[],"campuses":[],"targets":[],"subjects":[]
        }"#;
        assert!(decode_discovery_payload(forged).is_err());
    }

    #[test]
    fn upstream_discovery_duplicate_known_fields_fail_before_value_scanning() {
        for body in [
            br#"{"terms":[{"termId":"T2030","termId":"T2031"}],"campuses":[],"targets":[],"subjects":[]}"#
                .as_slice(),
            br#"{"terms":[],"campuses":[],"targets":[{"termId":"T2030","campusCode":"SYN","enabled":true,"enabled":false}],"subjects":[]}"#
                .as_slice(),
        ] {
            assert!(decode_discovery_payload(body).is_err());
        }
    }

    #[test]
    fn duplicate_and_dangling_targets_fail_closed() {
        let duplicate = decode_discovery_payload(
            br#"{
              "terms":[{"termId":"T2030"},{"termId":"T2030"}],
              "campuses":[],
              "targets":[],
              "subjects":[]
            }"#,
        )
        .expect("synthetic discovery should decode");
        assert_eq!(
            DiscoverySnapshot::try_from_raw(duplicate, provenance()),
            Err(DiscoveryError::DuplicateTerm)
        );

        let dangling = decode_discovery_payload(
            br#"{
              "terms":[{"termId":"T2030"}],
              "campuses":[],
              "targets":[{"termId":"T2030","campusCode":"SYN_UNKNOWN"}],
              "subjects":[]
            }"#,
        )
        .expect("synthetic discovery should decode");
        assert_eq!(
            DiscoverySnapshot::try_from_raw(dangling, provenance()),
            Err(DiscoveryError::DanglingTarget)
        );
    }

    #[test]
    fn discovery_failure_has_no_first_run_fallback_and_keeps_lkg_stale() {
        assert_eq!(
            decide_discovery_failure(false),
            DiscoveryFailureDisposition::InitializationFailedNoFallback
        );
        assert_eq!(
            decide_discovery_failure(true),
            DiscoveryFailureDisposition::RetainLastKnownGoodStale
        );
    }

    #[test]
    fn selector_and_bootstrap_bundle_retains_per_entry_ownership() {
        let selector_body = r#"{
          "sourceVersion":"selector-v1",
          "terms":[{"termId":"T2030"}],
          "campuses":[{"campusCode":"SYN_SELECTOR"}],
          "targets":[{"termId":"T2030","campusCode":"SYN_SELECTOR"}],
          "subjects":[{"termId":"T2030","campusCode":"SYN_SELECTOR","subjectCode":"SYN_SELECTOR_SUBJECT"}]
        }"#;
        let bootstrap_body = r#"{
          "sourceVersion":"bootstrap-v1",
          "terms":[{"termId":"T2029"}],
          "campuses":[{"campusCode":"SYN_BOOTSTRAP"}],
          "targets":[{"termId":"T2029","campusCode":"SYN_BOOTSTRAP"}],
          "subjects":[]
        }"#;
        let input = |kind, body: &str, request_id: &str| {
            DiscoverySourceInput::new(
                kind,
                decode_discovery_payload(body.as_bytes()).expect("synthetic discovery"),
                SourceProvenance::from_body(request_id, "2030-01-01T00:00:00Z", body.as_bytes()),
            )
        };
        let snapshot = DiscoverySnapshot::try_from_bundle(vec![
            input(
                DiscoverySourceKind::Selector,
                selector_body,
                "SYN_SELECTOR_REQUEST",
            ),
            input(
                DiscoverySourceKind::Bootstrap,
                bootstrap_body,
                "SYN_BOOTSTRAP_REQUEST",
            ),
        ])
        .expect("disjoint source bundle should normalize");

        assert_eq!(snapshot.sources.len(), 2);
        let selector = snapshot
            .sources
            .iter()
            .find(|source| source.kind == DiscoverySourceKind::Selector)
            .expect("selector source");
        let bootstrap = snapshot
            .sources
            .iter()
            .find(|source| source.kind == DiscoverySourceKind::Bootstrap)
            .expect("bootstrap source");
        assert_eq!(selector.kind.stable_identity(), "RUTGERS_SELECTOR");
        assert_eq!(bootstrap.kind.stable_identity(), "RBCSP_BOOTSTRAP");
        assert!(
            snapshot
                .terms
                .iter()
                .filter(|term| term.term_id.as_str() == "T2030")
                .all(|term| term.source_id == selector.id)
        );
        assert!(
            snapshot
                .targets
                .iter()
                .filter(|target| target.key.term().as_str() == "T2029")
                .all(|target| target.source_id == bootstrap.id)
        );
    }

    #[test]
    fn cross_source_references_fail_instead_of_being_misattributed() {
        let selector_body = r#"{
          "terms":[{"termId":"T2030"}],
          "campuses":[{"campusCode":"SYN_SELECTOR"}],
          "targets":[],
          "subjects":[]
        }"#;
        let bootstrap_body = r#"{
          "terms":[],
          "campuses":[],
          "targets":[{"termId":"T2030","campusCode":"SYN_SELECTOR"}],
          "subjects":[]
        }"#;
        let input = |kind, body: &str| {
            DiscoverySourceInput::new(
                kind,
                decode_discovery_payload(body.as_bytes()).expect("synthetic discovery"),
                SourceProvenance::from_body("SYNTHETIC", "2030-01-01T00:00:00Z", body.as_bytes()),
            )
        };
        assert_eq!(
            DiscoverySnapshot::try_from_bundle(vec![
                input(DiscoverySourceKind::Selector, selector_body),
                input(DiscoverySourceKind::Bootstrap, bootstrap_body),
            ]),
            Err(DiscoveryError::CrossSourceReference)
        );
    }

    #[test]
    fn different_bodies_cannot_duplicate_one_stable_source_kind() {
        let first_body = br#"{
          "terms":[{"termId":"T2030"}],"campuses":[],"targets":[],"subjects":[]
        }"#;
        let second_body = br#"{
          "terms":[{"termId":"T2031"}],"campuses":[],"targets":[],"subjects":[]
        }"#;
        let input = |body: &[u8], request_id: &str| {
            DiscoverySourceInput::selector(
                decode_discovery_payload(body).expect("synthetic discovery"),
                SourceProvenance::from_body(request_id, "2030-01-01T00:00:00Z", body),
            )
        };
        assert_eq!(
            DiscoverySnapshot::try_from_bundle(vec![
                input(first_body, "SYNTHETIC_A"),
                input(second_body, "SYNTHETIC_B"),
            ]),
            Err(DiscoveryError::DuplicateSource)
        );
    }
}
