use std::collections::BTreeSet;

use bcsp_contracts::{MatchExplanation, MatchOutcome, MatchReason, MatchReasonCode, ReasonField};

/// One predicate or aggregate result with stable, contract-valid reasons.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PredicateEvaluation {
    outcome: MatchOutcome,
    reasons: Vec<MatchReason>,
}

impl PredicateEvaluation {
    pub fn matched() -> Self {
        Self {
            outcome: MatchOutcome::Match,
            reasons: Vec::new(),
        }
    }

    pub fn uncertain(field: &'static str, code: MatchReasonCode) -> Self {
        debug_assert!(!code.is_known_mismatch());
        Self {
            outcome: MatchOutcome::Uncertain,
            reasons: vec![stable_reason(field, code)],
        }
    }

    pub fn no_match(field: &'static str) -> Self {
        Self {
            outcome: MatchOutcome::NoMatch,
            reasons: vec![stable_reason(field, MatchReasonCode::KnownMismatch)],
        }
    }

    pub const fn outcome(&self) -> MatchOutcome {
        self.outcome
    }

    pub fn reasons(&self) -> &[MatchReason] {
        &self.reasons
    }

    pub fn into_explanation(self) -> MatchExplanation {
        MatchExplanation::try_new(self.outcome, self.reasons)
            .expect("query evaluation always creates a contract-valid explanation")
    }
}

/// Three-valued AND. An empty set is the inactive-dimension identity MATCH.
pub fn and_all(evaluations: impl IntoIterator<Item = PredicateEvaluation>) -> PredicateEvaluation {
    let evaluations = evaluations.into_iter().collect::<Vec<_>>();
    if evaluations
        .iter()
        .any(|evaluation| evaluation.outcome == MatchOutcome::NoMatch)
    {
        aggregate(MatchOutcome::NoMatch, evaluations, false)
    } else if evaluations
        .iter()
        .any(|evaluation| evaluation.outcome == MatchOutcome::Uncertain)
    {
        aggregate(MatchOutcome::Uncertain, evaluations, false)
    } else {
        PredicateEvaluation::matched()
    }
}

/// Three-valued OR for an active dimension. An empty set is neutral MATCH,
/// because no selected values means that the dimension is inactive.
pub fn or_active(
    evaluations: impl IntoIterator<Item = PredicateEvaluation>,
) -> PredicateEvaluation {
    let evaluations = evaluations.into_iter().collect::<Vec<_>>();
    if evaluations.is_empty()
        || evaluations
            .iter()
            .any(|evaluation| evaluation.outcome == MatchOutcome::Match)
    {
        PredicateEvaluation::matched()
    } else if evaluations
        .iter()
        .any(|evaluation| evaluation.outcome == MatchOutcome::Uncertain)
    {
        // Known mismatches from other OR branches do not explain an uncertain
        // aggregate and are not valid UNCERTAIN reasons on the wire.
        aggregate(MatchOutcome::Uncertain, evaluations, true)
    } else {
        aggregate(MatchOutcome::NoMatch, evaluations, false)
    }
}

fn aggregate(
    outcome: MatchOutcome,
    evaluations: Vec<PredicateEvaluation>,
    uncertainty_only: bool,
) -> PredicateEvaluation {
    let mut seen = BTreeSet::new();
    let mut reasons = Vec::new();
    for reason in evaluations
        .into_iter()
        .flat_map(|evaluation| evaluation.reasons)
        .filter(|reason| !uncertainty_only || !reason.code().is_known_mismatch())
    {
        let key = (
            reason.code().wire_name(),
            reason.field().as_str().to_owned(),
        );
        if seen.insert(key) {
            reasons.push(reason);
        }
    }
    reasons.sort_by(|left, right| {
        left.field()
            .as_str()
            .cmp(right.field().as_str())
            .then_with(|| left.code().wire_name().cmp(right.code().wire_name()))
    });
    PredicateEvaluation { outcome, reasons }
}

fn stable_reason(field: &'static str, code: MatchReasonCode) -> MatchReason {
    MatchReason::new(
        code,
        ReasonField::try_from(field).expect("query reason fields are compile-time contract IDs"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn truth_tables_and_reason_contracts_are_preserved() {
        let values = [
            PredicateEvaluation::matched(),
            PredicateEvaluation::uncertain("FLT-S06", MatchReasonCode::Tba),
            PredicateEvaluation::no_match("FLT-S03"),
        ];

        assert_eq!(and_all(values.clone()).outcome(), MatchOutcome::NoMatch);
        let uncertain = or_active(values[1..].iter().cloned());
        assert_eq!(uncertain.outcome(), MatchOutcome::Uncertain);
        assert!(
            uncertain
                .reasons()
                .iter()
                .all(|reason| !reason.code().is_known_mismatch())
        );
        assert_eq!(or_active(std::iter::empty()).outcome(), MatchOutcome::Match);
        assert_eq!(and_all(std::iter::empty()).outcome(), MatchOutcome::Match);
        let _ = and_all(values).into_explanation();
    }

    fn value(index: u8, field: &'static str) -> PredicateEvaluation {
        match index % 3 {
            0 => PredicateEvaluation::matched(),
            1 => PredicateEvaluation::uncertain(field, MatchReasonCode::UnknownValue),
            _ => PredicateEvaluation::no_match(field),
        }
    }

    proptest! {
        #[test]
        fn three_value_aggregates_are_commutative_and_associative(a in any::<u8>(), b in any::<u8>(), c in any::<u8>()) {
            let a = value(a, "FLT-C03");
            let b = value(b, "FLT-S03");
            let c = value(c, "FLT-S06");
            prop_assert_eq!(
                and_all([a.clone(), b.clone()]),
                and_all([b.clone(), a.clone()])
            );
            prop_assert_eq!(
                or_active([a.clone(), b.clone()]),
                or_active([b.clone(), a.clone()])
            );
            prop_assert_eq!(
                and_all([and_all([a.clone(), b.clone()]), c.clone()]),
                and_all([a.clone(), and_all([b.clone(), c.clone()])])
            );
            prop_assert_eq!(
                or_active([or_active([a.clone(), b.clone()]), c.clone()]),
                or_active([a, or_active([b, c])])
            );
        }
    }
}
