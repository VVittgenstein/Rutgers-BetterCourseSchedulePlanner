use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use bcsp_contracts::TermCampusKey;
use thiserror::Error;

/// Shared, process-local demand for Rutgers targets that product routes actually use.
///
/// The set is deliberately level-triggered: repeated browser requests for the same target do not
/// create more coordinator registrations or upstream work.
#[derive(Clone, Debug, Default)]
pub struct TargetRefreshDemand {
    targets: Arc<Mutex<BTreeSet<TermCampusKey>>>,
}

impl TargetRefreshDemand {
    pub fn request(&self, target: TermCampusKey) -> Result<bool, TargetRefreshDemandError> {
        self.targets
            .lock()
            .map_err(|_| TargetRefreshDemandError::Unavailable)
            .map(|mut targets| targets.insert(target))
    }

    pub fn request_all(
        &self,
        targets: &[TermCampusKey],
    ) -> Result<usize, TargetRefreshDemandError> {
        let mut requested = self
            .targets
            .lock()
            .map_err(|_| TargetRefreshDemandError::Unavailable)?;
        Ok(targets
            .iter()
            .filter(|target| requested.insert((*target).clone()))
            .count())
    }

    pub fn snapshot(&self) -> Result<Vec<TermCampusKey>, TargetRefreshDemandError> {
        self.targets
            .lock()
            .map_err(|_| TargetRefreshDemandError::Unavailable)
            .map(|targets| targets.iter().cloned().collect())
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
}
