//! Per-client token buckets shared by the two anonymous issuance surfaces:
//! document GETs (homepage nonce issuance) and `POST /api/v1/session/validate`
//! (renewal is issuance in disguise -- the design mandates one policy, one
//! bucket set, so neither path can be used to bypass the other's limit).
//!
//! Alongside the per-client buckets a single GLOBAL budget bounds total
//! issuance throughput regardless of client-key cardinality (an IPv6 holder
//! must not multiply its allowance by rotating addresses). A request is
//! admitted only when BOTH buckets hold a token, and tokens are consumed
//! from both only on admission.
//!
//! Structure: every bucket lives in a key map plus a staleness-ordered index
//! (`BTreeSet<(Instant, key)>`), so per-request work is O(log n) -- eviction
//! at capacity pops the stalest entry, and idle cleanup is INCREMENTAL (a
//! bounded batch per request), never a full-table sweep on the hot path.
//! Time is sampled INSIDE the lock; an out-of-order timestamp (test seam or
//! platform clock quirk) is treated as zero elapsed and never deletes or
//! refills a bucket.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Per-client bucket capacity in whole tokens (one request = one token).
/// Generous because campus NAT puts many users behind one key.
pub(crate) const ISSUANCE_BURST_TOKENS: u64 = 60;
/// Per-client sustained refill in tokens per second.
pub(crate) const ISSUANCE_REFILL_TOKENS_PER_SECOND: u64 = 2;
/// Global issuance budget across ALL clients: burst and sustained refill.
/// Bounds registry-flooding regardless of key cardinality; the H4 global
/// WebSocket caps (pre-deployment hardening gate) bound lease pinning.
pub(crate) const GLOBAL_ISSUANCE_BURST_TOKENS: u64 = 600;
pub(crate) const GLOBAL_ISSUANCE_REFILL_TOKENS_PER_SECOND: u64 = 10;
/// Buckets idle this long are forgotten (they are full again anyway).
const BUCKET_IDLE_EXPIRY: Duration = Duration::from_secs(10 * 60);
/// Hard bound on distinct client keys held at once; at capacity the stalest
/// bucket is evicted via the index (a stale bucket is full again, so the
/// eviction is behaviorally lossless).
const MAX_TRACKED_CLIENTS: usize = 65_536;
/// Incremental idle-cleanup batch per request: bounded work, no sweeps.
const SWEEP_BATCH: usize = 8;

const MILLI: u64 = 1_000;

static POISON_LOGGED: AtomicBool = AtomicBool::new(false);

const NANOS_PER_SECOND: u128 = 1_000_000_000;

#[derive(Clone, Copy)]
struct Bucket {
    milli_tokens: u64,
    last_refill: Instant,
    /// Fractional refill credit carried between samples: the remainder of
    /// `elapsed_ns * rate` modulo 1e9, always `< 1e9`. Without it, advancing
    /// `last_refill` on a sub-granularity elapsed would silently burn the
    /// time credit -- a stream of sub-millisecond denied requests could
    /// freeze refill forever (reviewer P1).
    refill_residual: u64,
}

impl Bucket {
    fn full(burst_milli: u64, now: Instant) -> Self {
        Self {
            milli_tokens: burst_milli,
            last_refill: now,
            refill_residual: 0,
        }
    }

    /// Advances the bucket to `now`. A `now` at or before `last_refill`
    /// (out-of-order sample) is zero elapsed: no refill, no timestamp
    /// rewind -- an exhausted bucket can never be reset by timestamp races.
    /// Credit arithmetic is exact: `credited = (elapsed_ns * rate + residual)
    /// / 1e9` milli-tokens with the remainder carried, so no request cadence
    /// can burn fractional credit.
    fn refill(&mut self, now: Instant, burst_milli: u64, refill_milli_per_second: u64) {
        let Some(elapsed) = now.checked_duration_since(self.last_refill) else {
            return;
        };
        let numerator = elapsed
            .as_nanos()
            .saturating_mul(u128::from(refill_milli_per_second))
            .saturating_add(u128::from(self.refill_residual));
        let credited = u64::try_from(numerator / NANOS_PER_SECOND).unwrap_or(u64::MAX);
        self.milli_tokens = self.milli_tokens.saturating_add(credited).min(burst_milli);
        self.refill_residual = if self.milli_tokens == burst_milli {
            // A full bucket holds no pending credit.
            0
        } else {
            (numerator % NANOS_PER_SECOND) as u64
        };
        self.last_refill = now;
    }

    fn seconds_until_one_token(&self, refill_milli_per_second: u64) -> u32 {
        let deficit_milli = MILLI.saturating_sub(self.milli_tokens);
        let seconds = deficit_milli.div_ceil(refill_milli_per_second.max(1));
        u32::try_from(seconds.max(1)).unwrap_or(u32::MAX)
    }
}

struct LimiterState {
    buckets: BTreeMap<String, Bucket>,
    /// Staleness index over `buckets`: exactly one `(last_refill, key)` entry
    /// per bucket. Eviction and incremental cleanup pop from the front.
    by_staleness: BTreeSet<(Instant, String)>,
    global: Bucket,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RateDecision {
    Allow,
    /// Denied; carry the whole-second wait until one token is available
    /// (the `Retry-After` response header, always at least 1).
    Deny {
        retry_after_seconds: u32,
    },
}

pub(crate) struct IssuanceRateLimiter {
    state: Mutex<Option<LimiterState>>,
    burst_milli_tokens: u64,
    refill_milli_tokens_per_second: u64,
    global_burst_milli_tokens: u64,
    global_refill_milli_tokens_per_second: u64,
}

impl Default for IssuanceRateLimiter {
    fn default() -> Self {
        Self::new(
            ISSUANCE_BURST_TOKENS,
            ISSUANCE_REFILL_TOKENS_PER_SECOND,
            GLOBAL_ISSUANCE_BURST_TOKENS,
            GLOBAL_ISSUANCE_REFILL_TOKENS_PER_SECOND,
        )
    }
}

impl IssuanceRateLimiter {
    fn new(
        burst_tokens: u64,
        refill_tokens_per_second: u64,
        global_burst_tokens: u64,
        global_refill_tokens_per_second: u64,
    ) -> Self {
        Self {
            state: Mutex::new(None),
            burst_milli_tokens: burst_tokens * MILLI,
            refill_milli_tokens_per_second: refill_tokens_per_second * MILLI,
            global_burst_milli_tokens: global_burst_tokens * MILLI,
            global_refill_milli_tokens_per_second: global_refill_tokens_per_second * MILLI,
        }
    }

    /// Production entry point: the timestamp is sampled INSIDE the lock, so
    /// two concurrent callers can never present each other with out-of-order
    /// times (the reviewer's exhausted-bucket-reset race).
    pub(crate) fn check(&self, client_key: &str) -> RateDecision {
        let Ok(mut state) = self.state.lock() else {
            // A poisoned limiter must not take the site down: fail open and
            // leave the registry's capacity discipline as the backstop.
            // Logged once, not per request.
            if !POISON_LOGGED.swap(true, Ordering::Relaxed) {
                tracing::error!(code = "PUBLIC_RATE_LIMITER_UNAVAILABLE");
            }
            return RateDecision::Allow;
        };
        let now = Instant::now();
        self.check_locked(&mut state, client_key, now)
    }

    /// Test seam with an injected clock; still takes the lock first, and the
    /// bucket arithmetic itself is monotonic-safe against reversed samples.
    #[cfg(test)]
    fn check_at(&self, client_key: &str, now: Instant) -> RateDecision {
        let Ok(mut state) = self.state.lock() else {
            return RateDecision::Allow;
        };
        self.check_locked(&mut state, client_key, now)
    }

    fn check_locked(
        &self,
        slot: &mut Option<LimiterState>,
        client_key: &str,
        now: Instant,
    ) -> RateDecision {
        let state = slot.get_or_insert_with(|| LimiterState {
            buckets: BTreeMap::new(),
            by_staleness: BTreeSet::new(),
            global: Bucket::full(self.global_burst_milli_tokens, now),
        });

        // Incremental idle cleanup: at most SWEEP_BATCH stale entries per
        // request, popped from the front of the staleness index.
        for _ in 0..SWEEP_BATCH {
            let Some((stale_at, key)) = state.by_staleness.first().cloned() else {
                break;
            };
            let expired = now
                .checked_duration_since(stale_at)
                .is_some_and(|idle| idle > BUCKET_IDLE_EXPIRY);
            if !expired {
                break;
            }
            state.by_staleness.remove(&(stale_at, key.clone()));
            state.buckets.remove(&key);
        }

        state.global.refill(
            now,
            self.global_burst_milli_tokens,
            self.global_refill_milli_tokens_per_second,
        );

        let mut bucket = match state.buckets.get(client_key) {
            Some(existing) => {
                state
                    .by_staleness
                    .remove(&(existing.last_refill, client_key.to_owned()));
                let mut bucket = *existing;
                bucket.refill(
                    now,
                    self.burst_milli_tokens,
                    self.refill_milli_tokens_per_second,
                );
                bucket
            }
            None => {
                if state.buckets.len() >= MAX_TRACKED_CLIENTS {
                    // Indexed eviction of the stalest bucket: O(log n).
                    if let Some((stale_at, key)) = state.by_staleness.pop_first() {
                        let _ = stale_at;
                        state.buckets.remove(&key);
                    }
                }
                Bucket::full(self.burst_milli_tokens, now)
            }
        };

        // Admit only when BOTH buckets hold a token; consume from both only
        // on admission, so a denied request burns neither budget.
        let client_has_token = bucket.milli_tokens >= MILLI;
        let global_has_token = state.global.milli_tokens >= MILLI;
        let decision = if client_has_token && global_has_token {
            bucket.milli_tokens -= MILLI;
            state.global.milli_tokens -= MILLI;
            RateDecision::Allow
        } else {
            let client_wait = if client_has_token {
                0
            } else {
                bucket.seconds_until_one_token(self.refill_milli_tokens_per_second)
            };
            let global_wait = if global_has_token {
                0
            } else {
                state
                    .global
                    .seconds_until_one_token(self.global_refill_milli_tokens_per_second)
            };
            RateDecision::Deny {
                retry_after_seconds: client_wait.max(global_wait).max(1),
            }
        };
        state.buckets.insert(client_key.to_owned(), bucket);
        state
            .by_staleness
            .insert((bucket.last_refill, client_key.to_owned()));
        decision
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limiter(burst: u64, refill: u64) -> IssuanceRateLimiter {
        // A wide global budget so per-client behavior is isolated.
        IssuanceRateLimiter::new(burst, refill, 1_000_000, 1_000)
    }

    #[test]
    fn burst_then_sustained_refill_with_honest_retry_after() {
        let limiter = limiter(3, 1);
        let now = Instant::now();
        for _ in 0..3 {
            assert_eq!(limiter.check_at("client-a", now), RateDecision::Allow);
        }
        assert_eq!(
            limiter.check_at("client-a", now),
            RateDecision::Deny {
                retry_after_seconds: 1
            },
        );
        // One second later exactly one token refilled.
        let later = now + Duration::from_secs(1);
        assert_eq!(limiter.check_at("client-a", later), RateDecision::Allow);
        assert!(matches!(
            limiter.check_at("client-a", later),
            RateDecision::Deny { .. }
        ));
        // Distinct clients own distinct buckets.
        assert_eq!(limiter.check_at("client-b", later), RateDecision::Allow);
    }

    #[test]
    fn reversed_timestamps_never_reset_an_exhausted_bucket() {
        // The reviewer's race: exhaust at t2, then a caller that sampled
        // t1 < t2 arrives late. The bucket must stay denied and must not be
        // deleted or refilled.
        let limiter = limiter(2, 1);
        let t1 = Instant::now();
        let t2 = t1 + Duration::from_secs(5);
        assert_eq!(limiter.check_at("client-a", t2), RateDecision::Allow);
        assert_eq!(limiter.check_at("client-a", t2), RateDecision::Allow);
        assert!(matches!(
            limiter.check_at("client-a", t2),
            RateDecision::Deny { .. }
        ));
        assert!(
            matches!(limiter.check_at("client-a", t1), RateDecision::Deny { .. }),
            "an out-of-order timestamp must not reset the exhausted bucket",
        );
        // And the timestamp did not rewind: one second AFTER t2 refills.
        assert_eq!(
            limiter.check_at("client-a", t2 + Duration::from_secs(1)),
            RateDecision::Allow,
        );
    }

    #[test]
    fn idle_buckets_are_cleaned_incrementally() {
        let limiter = limiter(1, 1);
        let now = Instant::now();
        assert_eq!(limiter.check_at("client-a", now), RateDecision::Allow);
        assert!(matches!(
            limiter.check_at("client-a", now),
            RateDecision::Deny { .. }
        ));
        // Past the idle expiry the entry is swept and recreated full.
        let much_later = now + BUCKET_IDLE_EXPIRY + Duration::from_secs(1);
        assert_eq!(
            limiter.check_at("client-a", much_later),
            RateDecision::Allow
        );
    }

    #[test]
    fn sub_millisecond_denials_cannot_freeze_the_refill() {
        // Reviewer P1: 10,000 denied probes at 100us intervals must not burn
        // the fractional time credit -- at exactly t0+1s the 1-token/s bucket
        // must have accrued its token.
        let limiter = limiter(1, 1);
        let t0 = Instant::now();
        assert_eq!(limiter.check_at("client-a", t0), RateDecision::Allow);
        for ordinal in 1..10_000_u64 {
            let at = t0 + Duration::from_micros(100 * ordinal);
            assert!(
                matches!(limiter.check_at("client-a", at), RateDecision::Deny { .. }),
                "probe {ordinal} inside the first second must stay denied",
            );
        }
        assert_eq!(
            limiter.check_at("client-a", t0 + Duration::from_secs(1)),
            RateDecision::Allow,
            "the full second of fractional credit must have accumulated",
        );
    }

    #[test]
    fn a_denied_request_burns_credit_in_neither_bucket() {
        // Global exhausted by client A; client B's denied attempts must not
        // consume B's own (full) bucket, and must not stall the global
        // refill: one second later B is admitted on the refilled global
        // token with its burst intact.
        let limiter = IssuanceRateLimiter::new(1, 1, 1, 1);
        let t0 = Instant::now();
        assert_eq!(limiter.check_at("client-a", t0), RateDecision::Allow);
        for ordinal in 0..50_u64 {
            let at = t0 + Duration::from_micros(200 * ordinal);
            assert!(matches!(
                limiter.check_at("client-b", at),
                RateDecision::Deny { .. }
            ));
        }
        assert_eq!(
            limiter.check_at("client-b", t0 + Duration::from_secs(1)),
            RateDecision::Allow,
            "B's burst survived the denials and the global bucket refilled",
        );
    }

    #[test]
    fn the_global_budget_bounds_total_issuance_across_keys() {
        // Per-client generous, global tiny: rotating keys must not multiply
        // the allowance.
        let limiter = IssuanceRateLimiter::new(60, 2, 3, 1);
        let now = Instant::now();
        for ordinal in 0..3 {
            assert_eq!(
                limiter.check_at(&format!("client-{ordinal}"), now),
                RateDecision::Allow,
            );
        }
        let denied = limiter.check_at("client-fresh", now);
        assert!(
            matches!(denied, RateDecision::Deny { .. }),
            "a fresh key must not bypass the exhausted global budget",
        );
        // Refill restores service.
        assert_eq!(
            limiter.check_at("client-later", now + Duration::from_secs(1)),
            RateDecision::Allow,
        );
    }
}
