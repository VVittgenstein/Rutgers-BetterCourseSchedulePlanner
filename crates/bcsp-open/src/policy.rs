use std::time::Duration;

use bcsp_contracts::TermCampusKey;
use thiserror::Error;

pub const PUBLIC_GENERAL_OPEN_INTERVAL_SECONDS: u32 = 30;
pub const LOCAL_DEFAULT_OPEN_INTERVAL_SECONDS: u32 = 30;
pub const LOCAL_MINIMUM_OPEN_INTERVAL_SECONDS: u32 = 3;
pub const LOCAL_MAXIMUM_OPEN_INTERVAL_SECONDS: u32 = 3_600;
pub const PUBLIC_WATCH_OPEN_INTERVAL_SECONDS: u32 = 10;
pub const LOCAL_DEFAULT_WATCH_OPEN_INTERVAL_SECONDS: u32 = 10;
pub const LOCAL_MINIMUM_WATCH_OPEN_INTERVAL_SECONDS: u32 = 3;
pub const LOCAL_MAXIMUM_WATCH_OPEN_INTERVAL_SECONDS: u32 = 60;
/// Compatibility alias for the fixed Public fast-lane cadence.
pub const ACTIVE_WATCH_OPEN_INTERVAL_SECONDS: u32 = PUBLIC_WATCH_OPEN_INTERVAL_SECONDS;
pub const FATAL_DIAGNOSTIC_COOLDOWN_SECONDS: u32 = 60;
pub const MISSING_RETRY_AFTER_CIRCUIT_SECONDS: u32 = 60;

const NB_TRANSIENT_BACKOFF_SECONDS: [u32; 5] = [5, 10, 20, 30, 60];
const OTHER_TRANSIENT_BACKOFF_SECONDS: [u32; 5] = [15, 30, 60, 120, 300];
const CONTENT_BACKOFF_SECONDS: [u32; 4] = [60, 300, 900, 1_800];

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
    #[error("local watch Open interval must be between 3 and 60 seconds")]
    LocalWatchOutOfRange,
    #[error("the public watch Open interval is fixed at 10 seconds")]
    PublicWatchIsFixed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WatchOpenInterval {
    seconds: u32,
    mode: OpenRuntimeMode,
}

impl WatchOpenInterval {
    pub fn local(seconds: u32) -> Result<Self, OpenIntervalError> {
        if !(LOCAL_MINIMUM_WATCH_OPEN_INTERVAL_SECONDS..=LOCAL_MAXIMUM_WATCH_OPEN_INTERVAL_SECONDS)
            .contains(&seconds)
        {
            return Err(OpenIntervalError::LocalWatchOutOfRange);
        }
        Ok(Self {
            seconds,
            mode: OpenRuntimeMode::Local,
        })
    }

    pub const fn local_default() -> Self {
        Self {
            seconds: LOCAL_DEFAULT_WATCH_OPEN_INTERVAL_SECONDS,
            mode: OpenRuntimeMode::Local,
        }
    }

    pub const fn public() -> Self {
        Self {
            seconds: PUBLIC_WATCH_OPEN_INTERVAL_SECONDS,
            mode: OpenRuntimeMode::Public,
        }
    }

    pub fn public_checked(seconds: u32) -> Result<Self, OpenIntervalError> {
        if seconds != PUBLIC_WATCH_OPEN_INTERVAL_SECONDS {
            return Err(OpenIntervalError::PublicWatchIsFixed);
        }
        Ok(Self::public())
    }

    pub const fn seconds(self) -> u32 {
        self.seconds
    }

    pub const fn mode(self) -> OpenRuntimeMode {
        self.mode
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpenRefreshIntervals {
    general: GeneralOpenInterval,
    watch: WatchOpenInterval,
}

impl OpenRefreshIntervals {
    pub const fn new(general: GeneralOpenInterval, watch: WatchOpenInterval) -> Self {
        Self { general, watch }
    }

    pub const fn local_default() -> Self {
        Self::new(
            GeneralOpenInterval::local_default(),
            WatchOpenInterval::local_default(),
        )
    }

    pub const fn public() -> Self {
        Self::new(GeneralOpenInterval::public(), WatchOpenInterval::public())
    }

    pub const fn general(self) -> GeneralOpenInterval {
        self.general
    }

    pub const fn watch(self) -> WatchOpenInterval {
        self.watch
    }

    pub const fn effective_seconds(self, has_active_watch: bool) -> u32 {
        if has_active_watch && self.watch.seconds < self.general.seconds {
            self.watch.seconds
        } else {
            self.general.seconds
        }
    }

    pub fn effective(self, has_active_watch: bool) -> Duration {
        Duration::from_secs(u64::from(self.effective_seconds(has_active_watch)))
    }
}

impl From<GeneralOpenInterval> for OpenRefreshIntervals {
    fn from(general: GeneralOpenInterval) -> Self {
        Self::new(general, WatchOpenInterval::public())
    }
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
        OpenRefreshIntervals::new(self, WatchOpenInterval::public())
            .effective_seconds(has_active_watch)
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetRetryClass {
    Transient,
    Content,
}

pub fn target_retry_directive(
    target: &TermCampusKey,
    current_failure_streak: u32,
    class: TargetRetryClass,
) -> RetryDirective {
    let next_failure_streak = current_failure_streak.saturating_add(1);
    let steps = match class {
        TargetRetryClass::Content => &CONTENT_BACKOFF_SECONDS[..],
        TargetRetryClass::Transient if target.campus().as_str().eq_ignore_ascii_case("NB") => {
            &NB_TRANSIENT_BACKOFF_SECONDS[..]
        }
        TargetRetryClass::Transient => &OTHER_TRANSIENT_BACKOFF_SECONDS[..],
    };
    let step_index = usize::try_from(next_failure_streak.saturating_sub(1))
        .unwrap_or(usize::MAX)
        .min(steps.len() - 1);
    let base = Duration::from_secs(u64::from(steps[step_index]));
    RetryDirective {
        delay: base + deterministic_jitter_v1(target, next_failure_streak, base),
        mode: RetryMode::Automatic,
        next_failure_streak,
        clears_fatal_diagnostic: false,
    }
}

pub fn retry_directive(
    target: &TermCampusKey,
    _requested_effective: Duration,
    current_failure_streak: u32,
    failure: OpenFailureKind,
) -> RetryDirective {
    match failure {
        OpenFailureKind::Transient => {
            target_retry_directive(target, current_failure_streak, TargetRetryClass::Transient)
        }
        OpenFailureKind::UnsafeEmpty | OpenFailureKind::UnsafeZeroIntersection => {
            let mut directive =
                target_retry_directive(target, current_failure_streak, TargetRetryClass::Content);
            directive.clears_fatal_diagnostic = true;
            directive
        }
        OpenFailureKind::FatalProtocol => {
            target_retry_directive(target, current_failure_streak, TargetRetryClass::Content)
        }
        OpenFailureKind::RateLimited { retry_after } => {
            let circuit = retry_after.unwrap_or(Duration::from_secs(u64::from(
                MISSING_RETRY_AFTER_CIRCUIT_SECONDS,
            )));
            RetryDirective {
                delay: circuit,
                mode: RetryMode::OriginCircuit,
                next_failure_streak: current_failure_streak.saturating_add(1),
                clears_fatal_diagnostic: true,
            }
        }
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
    fn local_watch_interval_is_independently_bounded_and_combined_by_minimum() {
        assert!(WatchOpenInterval::local(3).is_ok());
        assert!(WatchOpenInterval::local(60).is_ok());
        assert_eq!(
            WatchOpenInterval::local(2),
            Err(OpenIntervalError::LocalWatchOutOfRange)
        );
        assert_eq!(
            WatchOpenInterval::local(61),
            Err(OpenIntervalError::LocalWatchOutOfRange)
        );
        assert_eq!(
            WatchOpenInterval::public_checked(30),
            Err(OpenIntervalError::PublicWatchIsFixed)
        );

        let faster_general = OpenRefreshIntervals::new(
            GeneralOpenInterval::local(3).expect("general"),
            WatchOpenInterval::local(10).expect("watch"),
        );
        assert_eq!(faster_general.effective_seconds(false), 3);
        assert_eq!(faster_general.effective_seconds(true), 3);

        let faster_watch = OpenRefreshIntervals::new(
            GeneralOpenInterval::local(30).expect("general"),
            WatchOpenInterval::local(7).expect("watch"),
        );
        assert_eq!(faster_watch.effective_seconds(false), 30);
        assert_eq!(faster_watch.effective_seconds(true), 7);
        assert_eq!(OpenRefreshIntervals::public().effective_seconds(true), 10);
    }

    #[test]
    fn target_retry_sequences_are_exact_before_deterministic_jitter() {
        let nb = target();
        let other = TermCampusKey::try_new("92026", "CM").expect("other target");
        for (target, class, expected) in [
            (
                &nb,
                TargetRetryClass::Transient,
                &[5_u64, 10, 20, 30, 60, 60][..],
            ),
            (
                &other,
                TargetRetryClass::Transient,
                &[15_u64, 30, 60, 120, 300, 300][..],
            ),
            (
                &nb,
                TargetRetryClass::Content,
                &[60_u64, 300, 900, 1_800, 1_800][..],
            ),
        ] {
            for (streak, expected_seconds) in expected.iter().enumerate() {
                let streak = u32::try_from(streak).expect("streak");
                let base = Duration::from_secs(*expected_seconds);
                let directive = target_retry_directive(target, streak, class);
                assert_eq!(
                    directive.delay,
                    base + deterministic_jitter_v1(target, streak + 1, base)
                );
                assert_eq!(directive.mode, RetryMode::Automatic);
            }
        }
    }

    #[test]
    fn only_explicit_rate_limit_opens_origin_circuit_and_is_not_interval_clamped() {
        let requested = Duration::from_secs(3_600);
        let missing = retry_directive(
            &target(),
            requested,
            0,
            OpenFailureKind::RateLimited { retry_after: None },
        );
        assert_eq!(missing.delay, Duration::from_secs(60));
        assert_eq!(missing.mode, RetryMode::OriginCircuit);

        let explicit = retry_directive(
            &target(),
            requested,
            0,
            OpenFailureKind::RateLimited {
                retry_after: Some(Duration::from_secs(7)),
            },
        );
        assert_eq!(explicit.delay, Duration::from_secs(7));

        let fatal = retry_directive(&target(), requested, 0, OpenFailureKind::FatalProtocol);
        assert!(fatal.delay >= Duration::from_secs(60));
        assert!(fatal.delay <= Duration::from_secs(66));
        assert_eq!(fatal.mode, RetryMode::Automatic);
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
