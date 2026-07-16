use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bcsp_contracts::{TermCampusKey, TermId};
use thiserror::Error;

/// Shared, process-local demand for Rutgers targets that product routes actually use.
///
/// The set is deliberately level-triggered: repeated browser requests for the same target do not
/// create more coordinator registrations or upstream work.
pub const TARGET_DEMAND_LEASE: Duration = Duration::from_secs(120);

#[derive(Clone, Debug)]
pub struct TargetRefreshDemand {
    targets: Arc<Mutex<BTreeMap<TermCampusKey, Instant>>>,
    manual_terms: Arc<Mutex<BTreeSet<TermId>>>,
    lease_duration: Duration,
}

impl Default for TargetRefreshDemand {
    fn default() -> Self {
        Self {
            targets: Arc::new(Mutex::new(BTreeMap::new())),
            manual_terms: Arc::new(Mutex::new(BTreeSet::new())),
            lease_duration: TARGET_DEMAND_LEASE,
        }
    }
}

impl TargetRefreshDemand {
    pub fn request(&self, target: TermCampusKey) -> Result<bool, TargetRefreshDemandError> {
        self.targets
            .lock()
            .map_err(|_| TargetRefreshDemandError::Unavailable)
            .map(|mut targets| {
                let now = Instant::now();
                let first = targets
                    .get(&target)
                    .is_none_or(|expires_at| *expires_at <= now);
                targets.insert(target, now + self.lease_duration);
                first
            })
    }

    pub fn request_all(
        &self,
        targets: &[TermCampusKey],
    ) -> Result<usize, TargetRefreshDemandError> {
        let mut requested = self
            .targets
            .lock()
            .map_err(|_| TargetRefreshDemandError::Unavailable)?;
        let now = Instant::now();
        requested.retain(|_, expires_at| *expires_at > now);
        let mut inserted = 0;
        for target in targets {
            if !requested.contains_key(target) {
                inserted += 1;
            }
            requested.insert(target.clone(), now + self.lease_duration);
        }
        Ok(inserted)
    }

    pub fn snapshot(&self) -> Result<Vec<TermCampusKey>, TargetRefreshDemandError> {
        let mut targets = self
            .targets
            .lock()
            .map_err(|_| TargetRefreshDemandError::Unavailable)?;
        let now = Instant::now();
        targets.retain(|_, expires_at| *expires_at > now);
        Ok(targets.keys().cloned().collect())
    }

    /// Requests all discovered real Campus targets for one Local manual term.
    ///
    /// The official runtime expands this level-triggered term demand against its current
    /// discovery LKG, so later discovery additions join the same supervisor automatically.
    pub fn request_manual_term(&self, term: TermId) -> Result<bool, TargetRefreshDemandError> {
        self.manual_terms
            .lock()
            .map_err(|_| TargetRefreshDemandError::Unavailable)
            .map(|mut terms| terms.insert(term))
    }

    pub fn manual_terms_snapshot(&self) -> Result<Vec<TermId>, TargetRefreshDemandError> {
        self.manual_terms
            .lock()
            .map_err(|_| TargetRefreshDemandError::Unavailable)
            .map(|terms| terms.iter().cloned().collect())
    }

    #[cfg(test)]
    fn with_lease_duration(lease_duration: Duration) -> Self {
        Self {
            lease_duration,
            ..Self::default()
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum TargetRefreshDemandError {
    #[error("target refresh demand is unavailable")]
    Unavailable,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    use bcsp_contracts::TermCampusKey;

    use super::TargetRefreshDemand;

    #[test]
    fn concurrent_duplicate_requests_are_recorded_once() {
        let demand = Arc::new(TargetRefreshDemand::default());
        let target = TermCampusKey::try_new("92026", "NB").expect("target");
        let workers = (0..8)
            .map(|_| {
                let demand = demand.clone();
                let target = target.clone();
                thread::spawn(move || demand.request(target).expect("request demand"))
            })
            .collect::<Vec<_>>();

        let inserted = workers
            .into_iter()
            .map(|worker| worker.join().expect("worker"))
            .filter(|inserted| *inserted)
            .count();
        assert_eq!(inserted, 1);
        assert_eq!(demand.snapshot().expect("demand snapshot"), vec![target]);
    }

    #[test]
    fn manual_term_requests_are_idempotent_and_separate_from_target_demand() {
        let demand = TargetRefreshDemand::default();
        let term = bcsp_contracts::TermId::try_from("12027").expect("term");
        assert!(
            demand
                .request_manual_term(term.clone())
                .expect("first request")
        );
        assert!(!demand.request_manual_term(term.clone()).expect("duplicate"));
        assert_eq!(
            demand.manual_terms_snapshot().expect("snapshot"),
            vec![term]
        );
        assert!(demand.snapshot().expect("targets").is_empty());
    }

    #[test]
    fn target_demand_is_renewed_and_expires() {
        let demand = TargetRefreshDemand::with_lease_duration(Duration::from_millis(10));
        let target = TermCampusKey::try_new("92026", "NB").expect("target");
        assert!(demand.request(target.clone()).expect("initial request"));
        assert!(!demand.request(target.clone()).expect("lease renewal"));
        thread::sleep(Duration::from_millis(20));
        assert!(demand.snapshot().expect("expired snapshot").is_empty());
        assert!(demand.request(target).expect("request after expiry"));
    }
}
