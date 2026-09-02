//! Startup re-derivation of stored delivery columns.
//!
//! The serving tables persist derived delivery facts (section modality and
//! synchronicity; occurrence kind, modality, synchronicity, evidence and
//! normalization reason) next to the canonical facts they were derived from.
//! The read-side projection recomputes that derivation and rejects any stored
//! row that disagrees, while publication skips a target whose raw semantic
//! hash is unchanged. A binary that carries a newer derivation rule therefore
//! has to rewrite the stored rows in place before it serves anything; this
//! module does that, keyed by the per-target derivation stamp.

use std::collections::BTreeMap;

use bcsp_contracts::TermCampusKey;
use bcsp_operational_storage::{
    CatalogDeliveryRewrite, OccurrenceDeliveryRewrite, OperationalStorage,
    PublishedCatalogSnapshot, SectionDeliveryRewrite, StorageError, StoredOccurrence,
};
use bcsp_rutgers_client::{Presence, RawCatalogSection};
use thiserror::Error;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::delivery::{classify_delivery, normalize_occurrence};
use crate::mapping::{occurrence_delivery_wire, occurrence_key, section_delivery_wire};
use crate::projection::{ProjectionError, decode_facts};

/// The derivation rule version of this binary.
///
/// Bump it whenever `normalize_occurrence` or `classify_delivery` can produce
/// a different result for identical canonical facts. Version 1 is the v0.1.1
/// rule (every mode-90 occurrence UNSPECIFIED, heterogeneous sections
/// UNKNOWN); version 2 derives mode-90 synchronicity from the time facts and
/// synthesizes MIXED from reliable SYNC+ASYNC sets.
pub const CATALOG_DERIVATION_VERSION: u32 = 2;

/// The version assumed for a target without a derivation stamp.
pub const LEGACY_CATALOG_DERIVATION_VERSION: u32 = 1;

#[derive(Debug, Error)]
pub enum RederivationError {
    #[error("operational storage failed during catalog re-derivation")]
    Storage(#[from] StorageError),
    #[error("stored catalog rows cannot be re-derived: {0}")]
    Projection(#[from] ProjectionError),
    #[error("stored catalog rows for {target:?} cannot be re-derived at {context}: {reason}")]
    InvalidStoredRows {
        target: TermCampusKey,
        context: &'static str,
        reason: &'static str,
    },
    #[error("the current time cannot be formatted as an RFC 3339 timestamp")]
    Timestamp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetRederivationReport {
    pub target: TermCampusKey,
    pub previous_version: u32,
    pub content_version: u64,
    pub sections_rewritten: u64,
    pub occurrences_rewritten: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RederivationReport {
    pub derivation_version: u32,
    /// One entry per target whose stamp differed from the binary's version.
    pub targets: Vec<TargetRederivationReport>,
}

impl RederivationReport {
    pub fn is_noop(&self) -> bool {
        self.targets.is_empty()
    }

    pub fn sections_rewritten(&self) -> u64 {
        self.targets
            .iter()
            .map(|target| target.sections_rewritten)
            .sum()
    }

    pub fn occurrences_rewritten(&self) -> u64 {
        self.targets
            .iter()
            .map(|target| target.occurrences_rewritten)
            .sum()
    }
}

/// Re-derives every stored target whose stamp differs from
/// [`CATALOG_DERIVATION_VERSION`], stamping the current time.
pub fn rederive_stored_delivery_now(
    storage: &mut OperationalStorage,
) -> Result<RederivationReport, RederivationError> {
    let now = OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|_| RederivationError::Timestamp)?;
    rederive_stored_delivery(storage, &now)
}

/// Re-derives every stored target whose stamp differs from
/// [`CATALOG_DERIVATION_VERSION`].
///
/// A target stamped with a HIGHER version than this binary is re-derived too:
/// after a downgrade the projection would otherwise reject its rows. Targets
/// that never published (content version 0) are only stamped. The operation is
/// idempotent; a second run finds every stamp current and touches nothing.
pub fn rederive_stored_delivery(
    storage: &mut OperationalStorage,
    now: &str,
) -> Result<RederivationReport, RederivationError> {
    let stamps = storage.catalog_derivation_versions()?;
    let mut report = RederivationReport {
        derivation_version: CATALOG_DERIVATION_VERSION,
        targets: Vec::new(),
    };
    for target_version in storage.catalog_target_versions()? {
        let target = target_version.target;
        let previous_version = stamps
            .get(&target)
            .copied()
            .unwrap_or(LEGACY_CATALOG_DERIVATION_VERSION);
        if previous_version == CATALOG_DERIVATION_VERSION {
            continue;
        }
        let rewrite = match storage.published_catalog_snapshot(&target)? {
            Some(published) if target_version.current_content_version > 0 => {
                plan_rewrite(&published, now)?
            }
            _ => CatalogDeliveryRewrite {
                derivation_version: CATALOG_DERIVATION_VERSION,
                stamped_at: now.to_owned(),
                sections: Vec::new(),
                occurrences: Vec::new(),
            },
        };
        let outcome = storage.rewrite_catalog_delivery(&target, &rewrite)?;
        tracing::info!(
            code = "CATALOG_DERIVATION_REPROJECTED",
            term = %target.term(),
            campus = %target.campus(),
            previous_version,
            derivation_version = CATALOG_DERIVATION_VERSION,
            content_version = target_version.current_content_version,
            sections_rewritten = outcome.sections_rewritten,
            occurrences_rewritten = outcome.occurrences_rewritten,
            "stored catalog delivery derivation re-projected in place"
        );
        report.targets.push(TargetRederivationReport {
            target,
            previous_version,
            content_version: target_version.current_content_version,
            sections_rewritten: outcome.sections_rewritten,
            occurrences_rewritten: outcome.occurrences_rewritten,
        });
    }
    Ok(report)
}

fn plan_rewrite(
    published: &PublishedCatalogSnapshot,
    now: &str,
) -> Result<CatalogDeliveryRewrite, RederivationError> {
    let target = &published.target;
    let mut stored_occurrences: BTreeMap<&str, Vec<&StoredOccurrence>> = BTreeMap::new();
    for occurrence in &published.snapshot.occurrences {
        stored_occurrences
            .entry(occurrence.section_key.index().as_str())
            .or_default()
            .push(occurrence);
    }
    let mut rewrite = CatalogDeliveryRewrite {
        derivation_version: CATALOG_DERIVATION_VERSION,
        stamped_at: now.to_owned(),
        sections: Vec::new(),
        occurrences: Vec::new(),
    };
    for section in &published.snapshot.sections {
        let raw: RawCatalogSection =
            decode_facts("catalog.section.canonicalFacts", &section.canonical_facts)?;
        let normalized = match &raw.meeting_times {
            Presence::Value(values) => values
                .iter()
                .enumerate()
                .map(|(ordinal, value)| normalize_occurrence(value, ordinal))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| RederivationError::InvalidStoredRows {
                    target: target.clone(),
                    context: "catalog.section.meetingTimes",
                    reason: "canonical occurrences cannot be normalized",
                })?,
            Presence::Missing | Presence::Null | Presence::Malformed(_) => Vec::new(),
        };
        let (delivery_modality, synchronicity) =
            classify_delivery(&raw.section_course_type, &normalized);
        let (expected_modality, expected_synchronicity) =
            section_delivery_wire(delivery_modality, synchronicity);
        if section.delivery_modality != expected_modality
            || section.synchronicity != expected_synchronicity
        {
            rewrite.sections.push(SectionDeliveryRewrite {
                key: section.key.clone(),
                delivery_modality: expected_modality.to_owned(),
                synchronicity: expected_synchronicity.to_owned(),
            });
        }

        let stored = stored_occurrences
            .remove(section.key.index().as_str())
            .unwrap_or_default();
        if stored.len() != normalized.len() {
            return Err(RederivationError::InvalidStoredRows {
                target: target.clone(),
                context: "catalog.section.meetingTimes",
                reason: "canonical occurrence count does not match stored rows",
            });
        }
        for expected in &normalized {
            let key =
                occurrence_key(expected).map_err(|_| RederivationError::InvalidStoredRows {
                    target: target.clone(),
                    context: "catalog.occurrence.key",
                    reason: "occurrence ordinal does not fit the stored key",
                })?;
            let actual = stored
                .iter()
                .find(|occurrence| occurrence.occurrence_key == key)
                .ok_or_else(|| RederivationError::InvalidStoredRows {
                    target: target.clone(),
                    context: "catalog.occurrence.key",
                    reason: "canonical occurrence has no stored row",
                })?;
            let wire = occurrence_delivery_wire(expected);
            if actual.occurrence_kind != wire.kind
                || actual.modality != wire.modality
                || actual.synchronicity != wire.synchronicity
                || actual.evidence != wire.evidence
                || actual.normalization_reason != wire.normalization_reason
            {
                rewrite.occurrences.push(OccurrenceDeliveryRewrite {
                    section_key: section.key.clone(),
                    occurrence_key: key,
                    occurrence_kind: wire.kind.to_owned(),
                    modality: wire.modality.to_owned(),
                    synchronicity: wire.synchronicity.to_owned(),
                    evidence: wire.evidence.to_owned(),
                    normalization_reason: wire.normalization_reason.to_owned(),
                });
            }
        }
    }
    if !stored_occurrences.is_empty() {
        return Err(RederivationError::InvalidStoredRows {
            target: target.clone(),
            context: "catalog.occurrence.section",
            reason: "stored occurrence rows belong to no stored section",
        });
    }
    Ok(rewrite)
}
