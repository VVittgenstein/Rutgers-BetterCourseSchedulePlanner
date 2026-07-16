use std::time::Duration;

use bcsp_rutgers_client::{CATALOG_TOTAL_TIMEOUT, OPEN_TOTAL_TIMEOUT};

/// Minimum interval between successful discovery generations.
pub const DISCOVERY_REFRESH_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

/// Serialized origin work includes two coordinator wakeups plus Catalog
/// normalization/publication and Open reconciliation. Keep this deliberately
/// small relative to the hard network timeouts while still budgeting the
/// non-network work once per discovered target.
const PER_TARGET_PROCESSING_MARGIN: Duration = Duration::from_secs(5);

/// One shared duration governs both the next discovery deadline and the
/// first-load Open freshness basis. Consequently a successful generation is
/// never stopped before its worst-case serialized Catalog/Open sweep, and
/// `2 * interval + 15s` remains long enough to bridge the next interval plus
/// the following replacement sweep.
pub(crate) fn refresh_generation_interval(target_count: usize) -> Duration {
    let target_count = u64::try_from(target_count).unwrap_or(u64::MAX);
    let per_target_seconds = CATALOG_TOTAL_TIMEOUT
        .as_secs()
        .saturating_add(OPEN_TOTAL_TIMEOUT.as_secs())
        .saturating_add(PER_TARGET_PROCESSING_MARGIN.as_secs());
    let fleet_seconds = per_target_seconds.saturating_mul(target_count);
    // Open attempt persistence currently represents the freshness basis as a
    // u32 count of seconds. Discovery input is byte-bounded far below this
    // ceiling; applying the same ceiling here keeps both consumers identical.
    let interval_seconds = DISCOVERY_REFRESH_INTERVAL
        .as_secs()
        .max(fleet_seconds)
        .min(u64::from(u32::MAX));
    Duration::from_secs(interval_seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn per_target_seconds() -> u64 {
        CATALOG_TOTAL_TIMEOUT
            .as_secs()
            .saturating_add(OPEN_TOTAL_TIMEOUT.as_secs())
            .saturating_add(PER_TARGET_PROCESSING_MARGIN.as_secs())
    }

    #[test]
    fn current_fleet_keeps_six_hours_and_larger_fleets_expand_the_generation() {
        assert_eq!(refresh_generation_interval(135), DISCOVERY_REFRESH_INTERVAL);

        let boundary =
            usize::try_from(DISCOVERY_REFRESH_INTERVAL.as_secs() / per_target_seconds() + 1)
                .expect("dynamic boundary");
        let expanded = refresh_generation_interval(boundary);
        let fleet_seconds = per_target_seconds()
            .saturating_mul(u64::try_from(boundary).expect("boundary target count"));
        assert!(expanded > DISCOVERY_REFRESH_INTERVAL);
        assert_eq!(expanded, Duration::from_secs(fleet_seconds));
    }

    #[test]
    fn shared_interval_bridges_the_next_wait_and_replacement_sweep() {
        let boundary =
            usize::try_from(DISCOVERY_REFRESH_INTERVAL.as_secs() / per_target_seconds() + 1)
                .expect("dynamic boundary");
        let interval = refresh_generation_interval(boundary).as_secs();
        let replacement_sweep = per_target_seconds()
            .saturating_mul(u64::try_from(boundary).expect("boundary target count"));

        assert!(
            interval.saturating_mul(2).saturating_add(15)
                >= interval.saturating_add(replacement_sweep)
        );
    }
}
