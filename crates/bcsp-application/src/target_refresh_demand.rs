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
    manual_target_retries: Arc<Mutex<ManualTargetRetryState>>,
    lease_duration: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManualTargetRetryRequest {
    target: TermCampusKey,
    generation: u64,
}

impl ManualTargetRetryRequest {
    pub const fn target(&self) -> &TermCampusKey {
        &self.target
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Debug, Default)]
struct ManualTargetRetryState {
    next_generation: u64,
    pending: BTreeMap<TermCampusKey, u64>,
}

impl Default for TargetRefreshDemand {
    fn default() -> Self {
        Self {
            targets: Arc::new(Mutex::new(BTreeMap::new())),
            manual_terms: Arc::new(Mutex::new(BTreeSet::new())),
            manual_target_retries: Arc::new(Mutex::new(ManualTargetRetryState::default())),
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

    /// Records one explicit Local retry generation for each target that is not already pending.
    ///
    /// The generation is target-bound and remains pending until the official refresh supervisor
    /// observes that target as queued/running (or otherwise accepts the command). Repeated clicks
    /// before that acknowledgement cannot multiply work.
    pub fn request_manual_target_retries(
        &self,
        targets: &[TermCampusKey],
    ) -> Result<usize, TargetRefreshDemandError> {
        let mut state = self
            .manual_target_retries
            .lock()
            .map_err(|_| TargetRefreshDemandError::Unavailable)?;
        let new_targets = targets
            .iter()
            .filter(|target| !state.pending.contains_key(*target))
            .cloned()
            .collect::<BTreeSet<_>>();
        let required =
            u64::try_from(new_targets.len()).map_err(|_| TargetRefreshDemandError::Unavailable)?;
        if state.next_generation.checked_add(required).is_none() {
            return Err(TargetRefreshDemandError::Unavailable);
        }
        for target in new_targets {
            state.next_generation += 1;
            let generation = state.next_generation;
            state.pending.insert(target, generation);
        }
        usize::try_from(required).map_err(|_| TargetRefreshDemandError::Unavailable)
    }

    pub fn pending_manual_target_retries(
        &self,
    ) -> Result<Vec<ManualTargetRetryRequest>, TargetRefreshDemandError> {
        self.manual_target_retries
            .lock()
            .map_err(|_| TargetRefreshDemandError::Unavailable)
            .map(|state| {
                state
                    .pending
                    .iter()
                    .map(|(target, generation)| ManualTargetRetryRequest {
                        target: target.clone(),
                        generation: *generation,
                    })
                    .collect()
            })
    }

    /// Acknowledges exactly the observed generation. A newer request can never be erased by a
    /// delayed supervisor acknowledgement.
    pub fn acknowledge_manual_target_retry(
        &self,
        request: &ManualTargetRetryRequest,
    ) -> Result<bool, TargetRefreshDemandError> {
        let mut state = self
            .manual_target_retries
            .lock()
            .map_err(|_| TargetRefreshDemandError::Unavailable)?;
        if state.pending.get(request.target()) != Some(&request.generation()) {
            return Ok(false);
        }
        state.pending.remove(request.target());
        Ok(true)
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
    use std::collections::BTreeSet;
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
    fn manual_target_retry_generations_are_per_target_pending_and_exactly_acknowledged() {
        let demand = TargetRefreshDemand::default();
        let nb = TermCampusKey::try_new("12027", "NB").expect("NB");
        let nk = TermCampusKey::try_new("12027", "NK").expect("NK");
        let cm = TermCampusKey::try_new("12027", "CM").expect("CM");

        assert_eq!(
            demand
                .request_manual_target_retries(&[nb.clone(), cm.clone(), nb.clone()])
                .expect("first target requests"),
            2,
        );
        assert_eq!(
            demand
                .request_manual_target_retries(&[nb.clone(), cm.clone()])
                .expect("pending duplicates"),
            0,
        );
        let first = demand
            .pending_manual_target_retries()
            .expect("pending generations");
        assert_eq!(
            first
                .iter()
                .map(|request| request.target().clone())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([nb.clone(), cm.clone()]),
            "only the independently missing targets are queued in stable target order",
        );
        let first_nb = first
            .iter()
            .find(|request| request.target() == &nb)
            .expect("first NB generation")
            .clone();
        assert!(
            demand
                .acknowledge_manual_target_retry(&first_nb)
                .expect("ack NB")
        );
        assert_eq!(
            demand
                .request_manual_target_retries(&[nb.clone(), nk.clone()])
                .expect("next retry generation"),
            2,
        );
        let second = demand
            .pending_manual_target_retries()
            .expect("second generations");
        let next_nb = second
            .iter()
            .find(|request| request.target() == &nb)
            .expect("new NB generation");
        assert!(next_nb.generation() > first_nb.generation());
        assert!(
            !demand
                .acknowledge_manual_target_retry(&first_nb)
                .expect("stale ack"),
            "a delayed acknowledgement cannot erase the newer target generation",
        );
        assert_eq!(
            second
                .iter()
                .map(|request| request.target().clone())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([nb, nk, cm]),
        );
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
