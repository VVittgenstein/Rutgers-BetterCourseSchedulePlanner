use bcsp_domain::{
    MatchExplanation, MatchExplanationError, MatchOutcome, MatchOutcomeAlgebra, MatchReason,
    MatchReasonCode, ReasonField, evaluate_active_dimension,
};

const VALUES: [MatchOutcome; 3] = [
    MatchOutcome::Match,
    MatchOutcome::Uncertain,
    MatchOutcome::NoMatch,
];

#[test]
fn and_truth_table_is_exact() {
    use MatchOutcome::{Match, NoMatch, Uncertain};
    let expected = [
        [Match, Uncertain, NoMatch],
        [Uncertain, Uncertain, NoMatch],
        [NoMatch, NoMatch, NoMatch],
    ];
    for (left_index, left) in VALUES.iter().enumerate() {
        for (right_index, right) in VALUES.iter().enumerate() {
            assert_eq!(left.and(*right), expected[left_index][right_index]);
        }
    }
}

#[test]
fn or_truth_table_is_exact() {
    use MatchOutcome::{Match, NoMatch, Uncertain};
    let expected = [
        [Match, Match, Match],
        [Match, Uncertain, Uncertain],
        [Match, Uncertain, NoMatch],
    ];
    for (left_index, left) in VALUES.iter().enumerate() {
        for (right_index, right) in VALUES.iter().enumerate() {
            assert_eq!(left.or(*right), expected[left_index][right_index]);
        }
    }
}

#[test]
fn algebra_is_commutative_associative_and_idempotent() {
    for left in VALUES {
        assert_eq!(left.and(left), left);
        assert_eq!(left.or(left), left);
        for right in VALUES {
            assert_eq!(left.and(right), right.and(left));
            assert_eq!(left.or(right), right.or(left));
            for third in VALUES {
                assert_eq!(left.and(right).and(third), left.and(right.and(third)));
                assert_eq!(left.or(right).or(third), left.or(right.or(third)));
            }
        }
    }
}

#[test]
fn empty_inactive_dimension_is_match_not_empty_or() {
    assert_eq!(MatchOutcome::any([]), MatchOutcome::NoMatch);
    assert_eq!(evaluate_active_dimension([]), MatchOutcome::Match);
    assert_eq!(MatchOutcome::all([]), MatchOutcome::Match);
    assert_eq!(
        evaluate_active_dimension([MatchOutcome::NoMatch, MatchOutcome::Uncertain]),
        MatchOutcome::Uncertain
    );
}

#[test]
fn explanation_invariants_are_stable() {
    let reason = MatchReason::new(
        MatchReasonCode::MissingReliableData,
        ReasonField::try_from("FLT-S03").unwrap(),
    );
    let value = MatchExplanation::try_new(MatchOutcome::Uncertain, vec![reason]).unwrap();
    assert_eq!(value.outcome(), MatchOutcome::Uncertain);
    assert_eq!(value.reasons().len(), 1);
    assert_eq!(value.reasons()[0].field().as_str(), "FLT-S03");

    assert_eq!(
        MatchExplanation::try_new(
            MatchOutcome::Match,
            vec![MatchReason::new(
                MatchReasonCode::UnknownValue,
                ReasonField::try_from("FLT-S03").unwrap(),
            )],
        )
        .unwrap_err(),
        MatchExplanationError::MatchHasReasons
    );
    assert_eq!(
        MatchExplanation::try_new(MatchOutcome::Uncertain, Vec::new()).unwrap_err(),
        MatchExplanationError::UncertainReasonContract
    );
    assert_eq!(
        MatchExplanation::try_new(
            MatchOutcome::NoMatch,
            vec![MatchReason::new(
                MatchReasonCode::UnknownValue,
                ReasonField::try_from("FLT-S03").unwrap(),
            )],
        )
        .unwrap_err(),
        MatchExplanationError::NoMatchReasonContract
    );
}
