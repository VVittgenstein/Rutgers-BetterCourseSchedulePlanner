use std::time::Duration;

/// Minimum interval between successful discovery generations.
pub const DISCOVERY_REFRESH_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Discovery cadence is independent of the bounded target supervisor. Target retries no longer
/// extend or stop discovery, so fleet size cannot recreate the RC2 global serialized sweep.
pub(crate) const fn refresh_generation_interval(_target_count: usize) -> Duration {
    DISCOVERY_REFRESH_INTERVAL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discovery_remains_six_hours_independent_of_target_count() {
        assert_eq!(refresh_generation_interval(0), DISCOVERY_REFRESH_INTERVAL);
        assert_eq!(refresh_generation_interval(24), DISCOVERY_REFRESH_INTERVAL);
        assert_eq!(refresh_generation_interval(137), DISCOVERY_REFRESH_INTERVAL);
        assert_eq!(
            refresh_generation_interval(10_000),
            DISCOVERY_REFRESH_INTERVAL
        );
    }
}
