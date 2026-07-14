use bcsp_contracts::{
    CatalogFieldKnowledge, CatalogFieldPresence, CatalogUnknownReason, MatchReasonCode,
};

use crate::PredicateEvaluation;

pub(crate) fn evaluate_known<T>(
    knowledge: &CatalogFieldKnowledge<T>,
    field: &'static str,
    predicate: impl FnOnce(&T) -> bool,
) -> PredicateEvaluation {
    match knowledge {
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::Present { value },
        } => {
            if predicate(value) {
                PredicateEvaluation::matched()
            } else {
                PredicateEvaluation::no_match(field)
            }
        }
        CatalogFieldKnowledge::Known {
            presence: CatalogFieldPresence::ExplicitNull | CatalogFieldPresence::Absent,
        } => PredicateEvaluation::uncertain(field, MatchReasonCode::MissingReliableData),
        CatalogFieldKnowledge::Unknown { reason } => {
            PredicateEvaluation::uncertain(field, unknown_reason_code(*reason))
        }
    }
}

pub(crate) const fn unknown_reason_code(reason: CatalogUnknownReason) -> MatchReasonCode {
    match reason {
        CatalogUnknownReason::Missing
        | CatalogUnknownReason::ExplicitNull
        | CatalogUnknownReason::SparseEvidence
        | CatalogUnknownReason::NotObserved => MatchReasonCode::MissingReliableData,
        CatalogUnknownReason::Malformed => MatchReasonCode::InvalidValue,
        CatalogUnknownReason::SourceUnavailable => MatchReasonCode::SourceUnavailable,
        CatalogUnknownReason::ConflictingEvidence => MatchReasonCode::ConflictingEvidence,
    }
}
