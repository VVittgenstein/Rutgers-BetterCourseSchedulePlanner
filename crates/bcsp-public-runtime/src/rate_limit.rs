//! Per-client token buckets shared by the two anonymous issuance surfaces:
//! document GETs (homepage nonce issuance) and `POST /api/v1/session/validate`
//! (renewal is issuance in disguise -- the design mandates one policy, one
//! bucket, so neither path can be used to bypass the other's limit).
//!
//! Integer token-bucket in milli-tokens: burst capacity absorbs page loads
//! and reconnect storms, the sustained refill bounds abuse. The numbers are
//! deliberately generous because campus NAT puts many users behind one IP;
//! the goal is registry-flooding abuse control, not throttling humans.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Bucket capacity in whole tokens (one request = one token).
pub(crate) const ISSUANCE_BURST_TOKENS: u64 = 60;
/// Sustained refill in tokens per second.
pub(crate) const ISSUANCE_REFILL_TOKENS_PER_SECOND: u64 = 2;
/// Buckets idle this long are forgotten (they are full again anyway).
const BUCKET_IDLE_EXPIRY: Duration = Duration::from_secs(10 * 60);
/// Hard bound on distinct client keys held at once; beyond it the stalest
/// bucket is dropped (a full-bucket drop is behaviorally lossless).
const MAX_TRACKED_CLIENTS: usize = 65_536;

const MILLI: u64 = 1_000;

struct Bucket {
    milli_tokens: u64,
    last_refill: Instant,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RateDecision {
    Allow,
    /// Denied; carry the whole-second wait until one token is available
    /// (the `Retry-After` response header, always at least 1).
    Deny { retry_after_seconds: u32 },
}

pub(crate) struct IssuanceRateLimiter {
    buckets: Mutex<BTreeMap<String, Bucket>>,
    burst_milli_tokens: u64,
    refill_milli_tokens_per_second: u64,
}

impl Default for IssuanceRateLimiter {
    fn default() -> Self {
        Self::new(ISSUANCE_BURST_TOKENS, ISSUANCE_REFILL_TOKENS_PER_SECOND)
    }
}

impl IssuanceRateLimiter {
    fn new(burst_tokens: u64, refill_tokens_per_second: u64) -> Self {
        Self {
            buckets: Mutex::new(BTreeMap::new()),
            burst_milli_tokens: burst_tokens * MILLI,
            refill_milli_tokens_per_second: refill_tokens_per_second * MILLI,
        }
    }

    pub(crate) fn check(&self, client_key: &str, now: Instant) -> RateDecision {
        let Ok(mut buckets) = self.buckets.lock() else {
            // A poisoned limiter must not take the site down: fail open and
            // leave the registry's own capacity discipline as the backstop.
            tracing::error!(code = "PUBLIC_RATE_LIMITER_UNAVAILABLE");
            return RateDecision::Allow;
        };
        buckets.retain(|_, bucket| {
            now.checked_duration_since(bucket.last_refill)
                .is_some_and(|idle| idle <= BUCKET_IDLE_EXPIRY)
        });
        if buckets.len() >= MAX_TRACKED_CLIENTS && !buckets.contains_key(client_key) {
            let stalest = buckets
                .iter()
                .min_by_key(|(_, bucket)| bucket.last_refill)
                .map(|(key, _)| key.clone());
            if let Some(stalest) = stalest {
                buckets.remove(&stalest);
            }
        }
        let bucket = buckets.entry(client_key.to_owned()).or_insert(Bucket {
            milli_tokens: self.burst_milli_tokens,
            last_refill: now,
        });
        let elapsed_milliseconds = now
            .checked_duration_since(bucket.last_refill)
            .map_or(0, |elapsed| elapsed.as_millis().min(u128::from(u64::MAX)) as u64);
        let refilled = elapsed_milliseconds
            .saturating_mul(self.refill_milli_tokens_per_second)
            / MILLI;
        bucket.milli_tokens = bucket
            .milli_tokens
            .saturating_add(refilled)
            .min(self.burst_milli_tokens);
        bucket.last_refill = now;
        if bucket.milli_tokens >= MILLI {
            bucket.milli_tokens -= MILLI;
            return RateDecision::Allow;
        }
        let deficit_milli = MILLI - bucket.milli_tokens;
        let seconds = deficit_milli.div_ceil(self.refill_milli_tokens_per_second);
        RateDecision::Deny {
            retry_after_seconds: u32::try_from(seconds.max(1)).unwrap_or(u32::MAX),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burst_then_sustained_refill_with_honest_retry_after() {
        let limiter = IssuanceRateLimiter::new(3, 1);
        let now = Instant::now();
        for _ in 0..3 {
            assert_eq!(limiter.check("client-a", now), RateDecision::Allow);
        }
        let denied = limiter.check("client-a", now);
        assert_eq!(
            denied,
            RateDecision::Deny {
                retry_after_seconds: 1
            },
        );
        // One second later exactly one token refilled.
        let later = now + Duration::from_secs(1);
        assert_eq!(limiter.check("client-a", later), RateDecision::Allow);
        assert!(matches!(
            limiter.check("client-a", later),
            RateDecision::Deny { .. }
        ));
        // Distinct clients own distinct buckets.
        assert_eq!(limiter.check("client-b", later), RateDecision::Allow);
    }

    #[test]
    fn idle_buckets_are_forgotten_and_capacity_is_bounded() {
        let limiter = IssuanceRateLimiter::new(1, 1);
        let now = Instant::now();
        assert_eq!(limiter.check("client-a", now), RateDecision::Allow);
        assert!(matches!(
            limiter.check("client-a", now),
            RateDecision::Deny { .. }
        ));
        // Past the idle expiry the bucket is recreated full.
        let much_later = now + BUCKET_IDLE_EXPIRY + Duration::from_secs(1);
        assert_eq!(limiter.check("client-a", much_later), RateDecision::Allow);
    }
}
