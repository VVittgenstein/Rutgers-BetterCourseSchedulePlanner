use bcsp_contracts::{
    OpenFreshnessState, OpenFreshnessV1, OpenRefreshClassification, OpenSequence,
    OpenUncertaintyReason,
};
use thiserror::Error;
use time::{Duration, OffsetDateTime};

pub const OPEN_FRESHNESS_GRACE_SECONDS: i64 = 15;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LastValidOpenPoint {
    pub attempt_sequence: OpenSequence,
    pub observed_at: OffsetDateTime,
    pub completed_at: OffsetDateTime,
    pub requested_effective_interval_seconds: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LatestOpenAttemptPoint {
    pub attempt_sequence: OpenSequence,
    pub completed_at: OffsetDateTime,
    pub classification: OpenRefreshClassification,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum OpenFreshnessError {
    #[error("requested effective Open interval must be nonzero")]
    ZeroInterval,
    #[error("Open freshness timestamp is outside the supported range")]
    TimestampOverflow,
}

pub fn project_open_freshness(
    now: OffsetDateTime,
    last_valid: Option<LastValidOpenPoint>,
    latest_attempt: Option<LatestOpenAttemptPoint>,
    unavailable_reason: Option<OpenUncertaintyReason>,
) -> Result<OpenFreshnessV1, OpenFreshnessError> {
    let Some(last_valid) = last_valid else {
        return Ok(OpenFreshnessV1 {
            state: OpenFreshnessState::Unknown,
            observed_at: None,
            fresh_until: None,
            last_known_good_age_seconds: None,
            uncertainty: Some(unavailable_reason.unwrap_or(OpenUncertaintyReason::NeverObserved)),
        });
    };
    if last_valid.requested_effective_interval_seconds == 0 {
        return Err(OpenFreshnessError::ZeroInterval);
    }
    let lifetime_seconds = i64::from(last_valid.requested_effective_interval_seconds)
        .checked_mul(2)
        .and_then(|value| value.checked_add(OPEN_FRESHNESS_GRACE_SECONDS))
        .ok_or(OpenFreshnessError::TimestampOverflow)?;
    let fresh_until = last_valid
        .completed_at
        .checked_add(Duration::seconds(lifetime_seconds))
        .ok_or(OpenFreshnessError::TimestampOverflow)?;
    let age = now - last_valid.completed_at;
    let last_known_good_age_seconds = u64::try_from(age.whole_seconds().max(0)).unwrap_or(u64::MAX);

    let invalidation = latest_attempt
        .filter(|attempt| attempt.attempt_sequence > last_valid.attempt_sequence)
        .and_then(|attempt| invalidation_reason(attempt.classification));
    let (state, uncertainty) = if let Some(reason) = invalidation {
        (OpenFreshnessState::Stale, Some(reason))
    } else if now <= fresh_until {
        (OpenFreshnessState::Fresh, None)
    } else {
        (
            OpenFreshnessState::Stale,
            Some(OpenUncertaintyReason::StaleLastKnownGood),
        )
    };
    Ok(OpenFreshnessV1 {
        state,
        observed_at: Some(last_valid.observed_at),
        fresh_until: Some(fresh_until),
        last_known_good_age_seconds: Some(last_known_good_age_seconds),
        uncertainty,
    })
}

const fn invalidation_reason(
    classification: OpenRefreshClassification,
) -> Option<OpenUncertaintyReason> {
    match classification {
        OpenRefreshClassification::Failed => Some(OpenUncertaintyReason::LatestAttemptFailed),
        OpenRefreshClassification::UnsafeEmpty
        | OpenRefreshClassification::UnsafeZeroIntersection => {
            Some(OpenUncertaintyReason::LatestAttemptUnsafe)
        }
        OpenRefreshClassification::StaleCatalogRace => {
            Some(OpenUncertaintyReason::StaleCatalogRace)
        }
        OpenRefreshClassification::InFlight
        | OpenRefreshClassification::ValidApplied
        | OpenRefreshClassification::ValidEmptyNoRows => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(seconds).expect("synthetic timestamp")
    }

    fn last_valid() -> LastValidOpenPoint {
        LastValidOpenPoint {
            attempt_sequence: OpenSequence::try_from(1).expect("sequence"),
            observed_at: at(100),
            completed_at: at(100),
            requested_effective_interval_seconds: 30,
        }
    }

    #[test]
    fn never_observed_is_unknown_without_invented_timestamps() {
        let value = project_open_freshness(at(100), None, None, None).expect("freshness");
        assert_eq!(value.state, OpenFreshnessState::Unknown);
        assert_eq!(value.observed_at, None);
        assert_eq!(value.fresh_until, None);
        assert_eq!(
            value.uncertainty,
            Some(OpenUncertaintyReason::NeverObserved)
        );
    }

    #[test]
    fn freshness_boundary_is_inclusive_and_age_is_honest() {
        let at_boundary =
            project_open_freshness(at(175), Some(last_valid()), None, None).expect("freshness");
        assert_eq!(at_boundary.state, OpenFreshnessState::Fresh);
        assert_eq!(at_boundary.last_known_good_age_seconds, Some(75));

        let after =
            project_open_freshness(at(176), Some(last_valid()), None, None).expect("freshness");
        assert_eq!(after.state, OpenFreshnessState::Stale);
        assert_eq!(
            after.uncertainty,
            Some(OpenUncertaintyReason::StaleLastKnownGood)
        );
    }

    #[test]
    fn later_failure_unsafe_or_catalog_race_immediately_invalidates_lkg() {
        for (classification, reason) in [
            (
                OpenRefreshClassification::Failed,
                OpenUncertaintyReason::LatestAttemptFailed,
            ),
            (
                OpenRefreshClassification::UnsafeEmpty,
                OpenUncertaintyReason::LatestAttemptUnsafe,
            ),
            (
                OpenRefreshClassification::UnsafeZeroIntersection,
                OpenUncertaintyReason::LatestAttemptUnsafe,
            ),
            (
                OpenRefreshClassification::StaleCatalogRace,
                OpenUncertaintyReason::StaleCatalogRace,
            ),
        ] {
            let value = project_open_freshness(
                at(110),
                Some(last_valid()),
                Some(LatestOpenAttemptPoint {
                    attempt_sequence: OpenSequence::try_from(2).expect("sequence"),
                    completed_at: at(105),
                    classification,
                }),
                None,
            )
            .expect("freshness");
            assert_eq!(value.state, OpenFreshnessState::Stale);
            assert_eq!(value.uncertainty, Some(reason));
        }
    }

    #[test]
    fn in_flight_attempt_does_not_discard_still_fresh_lkg() {
        let value = project_open_freshness(
            at(110),
            Some(last_valid()),
            Some(LatestOpenAttemptPoint {
                attempt_sequence: OpenSequence::try_from(2).expect("sequence"),
                completed_at: at(105),
                classification: OpenRefreshClassification::InFlight,
            }),
            None,
        )
        .expect("freshness");
        assert_eq!(value.state, OpenFreshnessState::Fresh);
        assert_eq!(value.uncertainty, None);
    }

    #[test]
    fn later_attempt_sequence_invalidates_even_when_wall_clock_does_not_advance() {
        for completed_at in [at(100), at(90)] {
            let value = project_open_freshness(
                at(110),
                Some(last_valid()),
                Some(LatestOpenAttemptPoint {
                    attempt_sequence: OpenSequence::try_from(2).expect("sequence"),
                    completed_at,
                    classification: OpenRefreshClassification::Failed,
                }),
                None,
            )
            .expect("freshness");
            assert_eq!(value.state, OpenFreshnessState::Stale);
            assert_eq!(
                value.uncertainty,
                Some(OpenUncertaintyReason::LatestAttemptFailed)
            );
        }
    }
}
