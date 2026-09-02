//! P2 hardening H4: the public WebSocket capacity gate and the frozen
//! resource numbers it enforces.
//!
//! Admission is one atomic step: the global and per-client counts are
//! checked and taken under a single lock, BEFORE the session lease, so a
//! connection is bounded from the moment it is admitted -- including the
//! window before its upgrade completes. The permit is RAII and rides the
//! upgrade closure next to the session lease; every exit path of the
//! transport pump releases both.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bcsp_application::{BoundedOutboundConfig, BoundedOutboundStats, OutboundByteBudget};

/// Global active public WebSocket cap (S2-D1 freeze, Codex 2026-08-22).
/// Strictly below the 4096-session registry so 3072 evictable sessions of
/// headroom remain: even with every connection slot pinned by a lease, the
/// registry can still issue and renew sessions.
pub(crate) const MAX_GLOBAL_WS_CONNECTIONS: u32 = 1024;

/// Default per-client active WebSocket cap (S2-D1 freeze: 64 as the launch
/// default). The client key is the issuance limiter's exact normalization;
/// campus NAT and shared IPv6 /64s can be accommodated by tuning ONLY this
/// value through [`crate::config::PUBLIC_WS_PER_CLIENT_LIMIT_ENVIRONMENT`].
pub(crate) const DEFAULT_PER_CLIENT_WS_CONNECTIONS: u32 = 64;

/// The whole tunable range for the per-client cap. Nothing else in this
/// module is configurable: one knob, not a configuration platform.
pub(crate) const PER_CLIENT_WS_LIMIT_RANGE: std::ops::RangeInclusive<u32> = 1..=1024;

/// Per-socket queued outbound payload budget (H4: 256 KiB). A byte budget,
/// not a frame count -- 256 frames of 64 KiB would be 16 MiB per socket,
/// which is exactly the memory contract this replaces.
pub(crate) const PER_SOCKET_OUTBOUND_BUDGET_BYTES: usize = 256 * 1024;

/// Process-wide queued outbound payload budget (H4: 64 MiB). The global cap
/// of 1024 sockets is only safe alongside this: per-socket budgets alone
/// would still admit 1024 x 256 KiB = 256 MiB of queued payload on a host
/// with roughly 955 MiB.
pub(crate) const GLOBAL_OUTBOUND_BUDGET_BYTES: usize = 64 * 1024 * 1024;

/// Deadline for every actual public socket write (H4: 5 s). A peer that
/// cannot take a frame for this long is a slow consumer and loses only its
/// own connection.
pub(crate) const SOCKET_WRITE_TIMEOUT: Duration = Duration::from_secs(5);

/// Everything the public host needs to bound its sockets: the admission
/// gate and the outbound-budget inputs for the bounded transport pump.
#[derive(Clone)]
pub(crate) struct PublicWsResources {
    pub(crate) capacity: Arc<WsConnectionCapacity>,
    pub(crate) global_outbound_budget: Arc<OutboundByteBudget>,
    pub(crate) per_socket_outbound_budget_bytes: usize,
    pub(crate) write_timeout: Duration,
    pub(crate) outbound_stats: Arc<BoundedOutboundStats>,
}

impl PublicWsResources {
    /// The production shape: every number frozen by the H4 contract, with
    /// only the per-client cap taken from configuration.
    pub(crate) fn production(per_client_limit: u32) -> Self {
        Self {
            capacity: WsConnectionCapacity::new(per_client_limit),
            global_outbound_budget: OutboundByteBudget::new(GLOBAL_OUTBOUND_BUDGET_BYTES),
            per_socket_outbound_budget_bytes: PER_SOCKET_OUTBOUND_BUDGET_BYTES,
            write_timeout: SOCKET_WRITE_TIMEOUT,
            outbound_stats: Arc::new(BoundedOutboundStats::default()),
        }
    }

    /// Small numbers for tests that must reach the boundaries this module
    /// exists to enforce; the mechanism under test is the production one.
    #[cfg(test)]
    pub(crate) fn with_limits(
        global_limit: u32,
        per_client_limit: u32,
        global_outbound_budget_bytes: usize,
        per_socket_outbound_budget_bytes: usize,
        write_timeout: Duration,
    ) -> Self {
        Self {
            capacity: WsConnectionCapacity::with_limits(global_limit, per_client_limit),
            global_outbound_budget: OutboundByteBudget::new(global_outbound_budget_bytes),
            per_socket_outbound_budget_bytes,
            write_timeout,
            outbound_stats: Arc::new(BoundedOutboundStats::default()),
        }
    }

    pub(crate) fn bounded_outbound_config(&self) -> BoundedOutboundConfig {
        BoundedOutboundConfig {
            per_socket_budget_bytes: self.per_socket_outbound_budget_bytes,
            write_timeout: self.write_timeout,
            global_budget: self.global_outbound_budget.clone(),
            stats: self.outbound_stats.clone(),
        }
    }
}

/// The H4 connection-count gate. One lock covers both counts so admission
/// is a single step; the refusal counters live outside it so metrics never
/// contend with admission.
#[derive(Debug)]
pub(crate) struct WsConnectionCapacity {
    global_limit: u32,
    per_client_limit: u32,
    state: Mutex<CapacityState>,
    global_refusals: AtomicU64,
    client_refusals: AtomicU64,
    /// Every admission this process has ever granted, monotonic for its
    /// lifetime. The active count above answers "how many now"; this
    /// answers "how many ever", which is the only way an observer holding
    /// two readings can tell one connection that stayed from one that was
    /// replaced by another. It carries no client key, no session, no
    /// identity of any kind -- it is a count.
    admissions: AtomicU64,
}

#[derive(Debug, Default)]
struct CapacityState {
    global_active: u32,
    per_client: BTreeMap<String, u32>,
}

/// Why an admission was refused. Global and per-client refusals are both
/// 503 on the wire; the distinct variants exist so the log code and the
/// metric can tell them apart.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WsCapacityError {
    GlobalExhausted,
    ClientExhausted,
    Unavailable,
}

impl WsConnectionCapacity {
    pub(crate) fn new(per_client_limit: u32) -> Arc<Self> {
        Self::with_limits(MAX_GLOBAL_WS_CONNECTIONS, per_client_limit)
    }

    fn with_limits(global_limit: u32, per_client_limit: u32) -> Arc<Self> {
        Arc::new(Self {
            global_limit,
            per_client_limit,
            state: Mutex::new(CapacityState::default()),
            global_refusals: AtomicU64::new(0),
            client_refusals: AtomicU64::new(0),
            admissions: AtomicU64::new(0),
        })
    }

    /// Takes one connection slot for `client_key`, or says exactly why not.
    /// The global bound is judged first; a client over both limits reads as
    /// a global refusal. A poisoned lock fails closed: this is a resource
    /// safety gate, and "unavailable" must never mean "unbounded".
    pub(crate) fn admit(
        self: &Arc<Self>,
        client_key: String,
    ) -> Result<WsCapacityPermit, WsCapacityError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| WsCapacityError::Unavailable)?;
        if state.global_active >= self.global_limit {
            drop(state);
            self.global_refusals.fetch_add(1, Ordering::SeqCst);
            return Err(WsCapacityError::GlobalExhausted);
        }
        let current = state.per_client.get(&client_key).copied().unwrap_or(0);
        if current >= self.per_client_limit {
            // No entry was created for a refused key: the map only ever
            // holds clients with live connections, so it is bounded by the
            // global cap.
            drop(state);
            self.client_refusals.fetch_add(1, Ordering::SeqCst);
            return Err(WsCapacityError::ClientExhausted);
        }
        state.global_active += 1;
        state.per_client.insert(client_key.clone(), current + 1);
        drop(state);
        // Counted here and nowhere else: one increment per slot actually
        // taken, so a refusal never moves it and a reconnect always does.
        self.admissions.fetch_add(1, Ordering::SeqCst);
        Ok(WsCapacityPermit {
            capacity: Arc::clone(self),
            client_key,
        })
    }

    /// Currently admitted connections, upgrade-pending ones included.
    pub(crate) fn global_active(&self) -> u32 {
        self.state
            .lock()
            .map_or(u32::MAX, |state| state.global_active)
    }

    /// Admissions granted since the process started; never decreases.
    pub(crate) fn admissions(&self) -> u64 {
        self.admissions.load(Ordering::SeqCst)
    }

    pub(crate) fn global_refusals(&self) -> u64 {
        self.global_refusals.load(Ordering::SeqCst)
    }

    pub(crate) fn client_refusals(&self) -> u64 {
        self.client_refusals.load(Ordering::SeqCst)
    }

    #[cfg(test)]
    fn tracked_clients(&self) -> usize {
        self.state
            .lock()
            .map_or(usize::MAX, |state| state.per_client.len())
    }
}

/// One admitted connection. Dropping it returns the global slot and the
/// client's slot, removing the client's map entry at zero so the key set
/// never outgrows the live connection set.
#[derive(Debug)]
pub(crate) struct WsCapacityPermit {
    capacity: Arc<WsConnectionCapacity>,
    client_key: String,
}

impl Drop for WsCapacityPermit {
    fn drop(&mut self) {
        let Ok(mut state) = self.capacity.state.lock() else {
            return;
        };
        state.global_active = state.global_active.saturating_sub(1);
        if let Some(count) = state.per_client.get_mut(&self.client_key) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                state.per_client.remove(&self.client_key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{MAX_DOCUMENT_SESSIONS, MAX_WS_CONNECTIONS_PER_SESSION};

    #[test]
    fn the_frozen_h4_numbers_and_their_relationships_hold() {
        assert_eq!(MAX_GLOBAL_WS_CONNECTIONS, 1024);
        assert_eq!(MAX_DOCUMENT_SESSIONS, 4096);
        assert!((MAX_GLOBAL_WS_CONNECTIONS as usize) < MAX_DOCUMENT_SESSIONS);
        assert_eq!(
            MAX_DOCUMENT_SESSIONS - MAX_GLOBAL_WS_CONNECTIONS as usize,
            3072,
            "pinned leases must leave 3072 evictable sessions of headroom",
        );
        assert!(
            MAX_DOCUMENT_SESSIONS - (MAX_GLOBAL_WS_CONNECTIONS as usize)
                >= MAX_DOCUMENT_SESSIONS / 2,
            "headroom must stay at or above half the registry",
        );
        assert_eq!(DEFAULT_PER_CLIENT_WS_CONNECTIONS, 64);
        assert!(PER_CLIENT_WS_LIMIT_RANGE.contains(&DEFAULT_PER_CLIENT_WS_CONNECTIONS));
        assert_eq!(PER_CLIENT_WS_LIMIT_RANGE, 1..=1024);
        assert_eq!(
            MAX_WS_CONNECTIONS_PER_SESSION, 4,
            "per-session cap is untouched"
        );
        assert_eq!(PER_SOCKET_OUTBOUND_BUDGET_BYTES, 256 * 1024);
        assert_eq!(GLOBAL_OUTBOUND_BUDGET_BYTES, 64 * 1024 * 1024);
        assert_eq!(SOCKET_WRITE_TIMEOUT, Duration::from_secs(5));
        assert!(
            PER_SOCKET_OUTBOUND_BUDGET_BYTES * (MAX_GLOBAL_WS_CONNECTIONS as usize)
                > GLOBAL_OUTBOUND_BUDGET_BYTES,
            "the global budget must bind before per-socket budgets can sum past it",
        );
    }

    #[test]
    fn the_global_gate_refuses_at_its_limit_and_readmits_after_release() {
        let capacity = WsConnectionCapacity::with_limits(2, 10);
        let first = capacity.admit("a".to_owned()).expect("first slot");
        let second = capacity.admit("b".to_owned()).expect("second slot");
        assert_eq!(
            capacity.admit("c".to_owned()).unwrap_err(),
            WsCapacityError::GlobalExhausted,
        );
        assert_eq!(capacity.global_refusals(), 1);
        assert_eq!(capacity.client_refusals(), 0);
        assert_eq!(capacity.global_active(), 2);
        drop(first);
        assert_eq!(capacity.global_active(), 1);
        let _third = capacity.admit("c".to_owned()).expect("freed slot readmits");
        drop(second);
    }

    #[test]
    fn the_client_gate_bounds_one_key_without_touching_another() {
        let capacity = WsConnectionCapacity::with_limits(100, 2);
        let one = capacity.admit("shared".to_owned()).expect("first");
        let two = capacity.admit("shared".to_owned()).expect("second");
        assert_eq!(
            capacity.admit("shared".to_owned()).unwrap_err(),
            WsCapacityError::ClientExhausted,
        );
        assert_eq!(capacity.client_refusals(), 1);
        assert_eq!(capacity.global_refusals(), 0);
        let other = capacity.admit("elsewhere".to_owned()).expect("another key");
        drop(one);
        let _again = capacity.admit("shared".to_owned()).expect("released slot");
        drop((two, other));
    }

    #[test]
    fn a_refused_key_leaves_no_entry_and_released_keys_are_forgotten() {
        let capacity = WsConnectionCapacity::with_limits(1, 1);
        let held = capacity.admit("holder".to_owned()).expect("the slot");
        assert_eq!(
            capacity.admit("newcomer".to_owned()).unwrap_err(),
            WsCapacityError::GlobalExhausted
        );
        assert_eq!(
            capacity.tracked_clients(),
            1,
            "a refusal must not add a key"
        );
        drop(held);
        assert_eq!(
            capacity.tracked_clients(),
            0,
            "a released key must be forgotten"
        );
        assert_eq!(capacity.global_active(), 0);
    }

    /// H9 (R3): the soak reads this counter twice and subtracts. That only
    /// means "one connection, the whole time" if the number counts slots
    /// TAKEN -- never refusals, and never giving anything back on release.
    /// A socket that went away and was replaced by another must be
    /// indistinguishable from two admissions, because that is what it is.
    #[test]
    fn admissions_count_every_slot_taken_and_nothing_else() {
        let capacity = WsConnectionCapacity::with_limits(1, 1);
        assert_eq!(capacity.admissions(), 0);
        let held = capacity.admit("holder".to_owned()).expect("the slot");
        assert_eq!(capacity.admissions(), 1);
        assert_eq!(
            capacity.admit("newcomer".to_owned()).unwrap_err(),
            WsCapacityError::GlobalExhausted
        );
        assert_eq!(
            capacity.admissions(),
            1,
            "a refused connection was never admitted"
        );
        drop(held);
        assert_eq!(
            capacity.admissions(),
            1,
            "releasing a slot does not un-admit it"
        );
        let replacement = capacity
            .admit("newcomer".to_owned())
            .expect("the freed slot");
        assert_eq!(
            capacity.admissions(),
            2,
            "the replacement is a second admission, which is the whole point"
        );
        assert_eq!(capacity.global_active(), 1);
        drop(replacement);
    }
}
