use std::time::Duration;

use bcsp_contracts::TermCampusKey;
use thiserror::Error;

pub const PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS: u32 = 30;
pub const LOCAL_DEFAULT_OPEN_INTERVAL_SECONDS: u32 = 30;
pub const LOCAL_MINIMUM_OPEN_INTERVAL_SECONDS: u32 = 3;
pub const LOCAL_MAXIMUM_OPEN_INTERVAL_SECONDS: u32 = 3_600;
pub const ACTIVE_WATCH_OPEN_INTERVAL_SECONDS: u32 = 10;
pub const FATAL_DIAGNOSTIC_COOLDOWN_SECONDS: u32 = 60;
pub const MISSING_RETRY_AFTER_CIRCUIT_SECONDS: u32 = 15 * 60;

const TRANSIENT_BACKOFF_SECONDS: [u32; 6] = [30, 60, 120, 240, 480, 600];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenRuntimeMode {
    Local,
    Public,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralOpenInterval {
    seconds: u32,
    mode: OpenRuntimeMode,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum OpenIntervalError {
    #[error("local Open interval must be between 3 and 3600 seconds")]
    LocalOutOfRange,
    #[error("the public Open interval is fixed at 30 seconds")]
    PublicIsFixed,
}

impl GeneralOpenInterval {
    pub fn local(seconds: u32) -> Result<Self, OpenIntervalError> {
        if !(LOCAL_MINIMUM_OPEN_INTERVAL_SECONDS..=LOCAL_MAXIMUM_OPEN_INTERVAL_SECONDS)
            .contains(&seconds)
        {
            return Err(OpenIntervalError::LocalOutOfRange);
        }
        Ok(Self {
            seconds,
            mode: OpenRuntimeMode::Local,
        })
    }

    pub const fn public() -> Self {
        Self {
            seconds: PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS,
            mode: OpenRuntimeMode::Public,
        }
    }

    pub const fn local_default() -> Self {
        Self {
            seconds: LOCAL_DEFAULT_OPEN_INTERVAL_SECONDS,
            mode: OpenRuntimeMode::Local,
        }
    }

    pub fn public_checked(seconds: u32) -> Result<Self, OpenIntervalError> {
        if seconds != PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS {
            return Err(OpenIntervalError::PublicIsFixed);
        }
        Ok(Self::public())
    }

    pub const fn seconds(self) -> u32 {
        self.seconds
    }

    pub const fn mode(self) -> OpenRuntimeMode {
        self.mode
    }

    pub const fn effective_seconds(self, has_active_watch: bool) -> u32 {
        if has_active_watch && self.seconds > ACTIVE_WATCH_OPEN_INTERVAL_SECONDS {
            ACTIVE_WATCH_OPEN_INTERVAL_SECONDS
        } else {
            self.seconds
        }
    }

    pub fn effective(self, has_active_watch: bool) -> Duration {
        Duration::from_secs(u64::from(self.effective_seconds(has_active_watch)))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OpenFailureKind {
    Transient,
    UnsafeEmpty,
    UnsafeZeroIntersection,
    RateLimited { retry_after: Option<Duration> },
    FatalProtocol,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RetryMode {
    Automatic,
    OriginCircuit,
    ExplicitDiagnosticRecheck,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryDirective {
    pub delay: Duration,
    pub mode: RetryMode,
    pub next_failure_streak: u32,
    /// A decoded 2xx response proves that an earlier fatal transport/protocol
    /// condition has cleared, even when reconcile still rejects its semantics.
    pub clears_fatal_diagnostic: bool,
}

pub fn retry_directive(
    target: &TermCampusKey,
    requested_effective: Duration,
    current_failure_streak: u32,
    failure: OpenFailureKind,
) -> RetryDirective {
    match failure {
        OpenFailureKind::Transient
        | OpenFailureKind::UnsafeEmpty
        | OpenFailureKind::UnsafeZeroIntersection => {
            let next_failure_streak = current_failure_streak.saturating_add(1);
            let step_index = usize::try_from(next_failure_streak.saturating_sub(1))
                .unwrap_or(usize::MAX)
                .min(TRANSIENT_BACKOFF_SECONDS.len() - 1);
            let backoff = Duration::from_secs(u64::from(TRANSIENT_BACKOFF_SECONDS[step_index]));
            let selected = requested_effective.max(backoff);
            RetryDirective {
                delay: selected + deterministic_jitter_v1(target, next_failure_streak, selected),
                mode: RetryMode::Automatic,
                next_failure_streak,
                clears_fatal_diagnostic: matches!(
                    failure,
                    OpenFailureKind::UnsafeEmpty | OpenFailureKind::UnsafeZeroIntersection
                ),
            }
        }
        OpenFailureKind::RateLimited { retry_after } => {
            let circuit = retry_after.unwrap_or(Duration::from_secs(u64::from(
                MISSING_RETRY_AFTER_CIRCUIT_SECONDS,
            )));
            RetryDirective {
                delay: requested_effective.max(circuit),
                mode: RetryMode::OriginCircuit,
                next_failure_streak: current_failure_streak.saturating_add(1),
                clears_fatal_diagnostic: true,
            }
        }
        OpenFailureKind::FatalProtocol => RetryDirective {
            delay: requested_effective.max(Duration::from_secs(u64::from(
                FATAL_DIAGNOSTIC_COOLDOWN_SECONDS,
            ))),
            mode: RetryMode::ExplicitDiagnosticRecheck,
            next_failure_streak: current_failure_streak.saturating_add(1),
            clears_fatal_diagnostic: false,
        },
    }
}

/// Stable positive jitter in the inclusive range `0..=10%` of `selected_delay`.
pub fn deterministic_jitter_v1(
    target: &TermCampusKey,
    failure_streak: u32,
    selected_delay: Duration,
) -> Duration {
    let preimage = format!(
        "bcsp-open-jitter-v1\0{}\0{}\0{}\0{}",
        target.term(),
        target.campus(),
        failure_streak,
        selected_delay.as_millis()
    );
    let digest = bcsp_rutgers_client::sha256_hex(preimage.as_bytes());
    let prefix = digest.bytes().take(16).fold(0_u64, |value, byte| {
        value.wrapping_mul(16) + hex_nibble(byte)
    });
    let maximum_millis = selected_delay.as_millis() / 10;
    let bounded_maximum = u64::try_from(maximum_millis).unwrap_or(u64::MAX);
    if bounded_maximum == 0 {
        Duration::ZERO
    } else {
        Duration::from_millis(prefix % bounded_maximum.saturating_add(1))
    }
}

const fn hex_nibble(byte: u8) -> u64 {
    match byte {
        b'0'..=b'9' => (byte - b'0') as u64,
        b'a'..=b'f' => (byte - b'a' + 10) as u64,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target() -> TermCampusKey {
        TermCampusKey::try_new("92026", "NB").expect("synthetic target")
    }

    #[test]
    fn local_and_public_boundaries_are_rejected_not_clamped() {
        assert!(GeneralOpenInterval::local(3).is_ok());
        assert!(GeneralOpenInterval::local(3_600).is_ok());
        assert_eq!(
            GeneralOpenInterval::local(2),
            Err(OpenIntervalError::LocalOutOfRange)
        );
        assert_eq!(
            GeneralOpenInterval::local(3_601),
            Err(OpenIntervalError::LocalOutOfRange)
        );
        assert_eq!(
            GeneralOpenInterval::public_checked(10),
            Err(OpenIntervalError::PublicIsFixed)
        );
        assert_eq!(GeneralOpenInterval::public().seconds(), 30);
    }

    #[test]
    fn active_watch_uses_minimum_without_clamping_three_seconds() {
        assert_eq!(
            GeneralOpenInterval::local(3)
                .expect("valid")
                .effective_seconds(true),
            3
        );
        assert_eq!(
            GeneralOpenInterval::local(30)
                .expect("valid")
                .effective_seconds(true),
            10
        );
        assert_eq!(GeneralOpenInterval::public().effective_seconds(false), 30);
    }

    #[test]
    fn transient_backoff_never_accelerates_a_long_requested_cadence() {
        let requested = Duration::from_secs(3_600);
        for streak in 0..10 {
            let directive =
                retry_directive(&target(), requested, streak, OpenFailureKind::Transient);
            assert!(directive.delay >= requested);
            assert!(directive.delay <= requested + requested / 10);
            assert_eq!(directive.mode, RetryMode::Automatic);
            assert!(!directive.clears_fatal_diagnostic);
        }

        assert!(
            retry_directive(
                &target(),
                Duration::from_secs(30),
                0,
                OpenFailureKind::UnsafeEmpty
            )
            .clears_fatal_diagnostic
        );
    }

    #[test]
    fn rate_limit_and_fatal_fail_closed_origin_wide() {
        let requested = Duration::from_secs(30);
        let missing = retry_directive(
            &target(),
            requested,
            0,
            OpenFailureKind::RateLimited { retry_after: None },
        );
        assert_eq!(missing.delay, Duration::from_secs(900));
        assert_eq!(missing.mode, RetryMode::OriginCircuit);

        let explicit = retry_directive(
            &target(),
            requested,
            0,
            OpenFailureKind::RateLimited {
                retry_after: Some(Duration::from_secs(120)),
            },
        );
        assert_eq!(explicit.delay, Duration::from_secs(120));

        let fatal = retry_directive(&target(), requested, 0, OpenFailureKind::FatalProtocol);
        assert_eq!(fatal.delay, Duration::from_secs(60));
        assert_eq!(fatal.mode, RetryMode::ExplicitDiagnosticRecheck);
    }

    #[test]
    fn jitter_is_stable_bounded_and_target_scoped() {
        let selected = Duration::from_secs(60);
        let first = deterministic_jitter_v1(&target(), 2, selected);
        assert_eq!(first, Duration::from_millis(1_615));
        assert_eq!(first, deterministic_jitter_v1(&target(), 2, selected));
        assert!(first <= selected / 10);
        assert_ne!(
            first,
            deterministic_jitter_v1(
                &TermCampusKey::try_new("92026", "NWK").expect("synthetic target"),
                2,
                selected
            )
        );
    }
}
