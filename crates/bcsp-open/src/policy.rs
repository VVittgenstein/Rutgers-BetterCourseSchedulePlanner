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
    /// Integrity-gate hold: the target is quarantined and needs dense probe
    /// samples to drive recovery/confirmation (gate design section 4).
    SuspectPartial,
    RateLimited {
        retry_after: Option<Duration>,
    },
    FatalProtocol,
}

/// Quarantine probe cadence: exactly `min(30s, effective interval)` for every
/// campus, deliberately NOT a backoff ladder -- adjacent samples must stay
/// well inside the gate's 120s max gap or confirmation would be unreachable
/// (the NK/CM transient ladder exceeds it, per review).
///
/// The cadence is also NOT jittered. Every other retry path spreads its
/// delay by up to +10% so a fleet of targets does not resynchronize on the
/// origin, but this path is a bounded contract, not a backoff: the gate
/// design fixes the probe at `min(30s, watch interval)`, and adding positive
/// jitter on top of a 30s selection produces up to 33s. That is a cadence the
/// approved design does not permit, and it eats headroom against the 120s max
/// sample gap for nothing -- a quarantined target is a single target probing
/// a single origin, so there is no fleet to de-synchronize.
fn quarantine_probe_directive(
    requested_effective: Duration,
    current_failure_streak: u32,
) -> RetryDirective {
    let cap = Duration::from_secs(crate::GATE_QUARANTINE_PROBE_CAP_SECONDS);
    let base = if requested_effective.is_zero() {
        cap
    } else {
        requested_effective.min(cap)
    };
    RetryDirective {
        delay: base,
        mode: RetryMode::Automatic,
        next_failure_streak: current_failure_streak.saturating_add(1),
        clears_fatal_diagnostic: true,
    }
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
    requested_effective: Duration,
    current_failure_streak: u32,
    failure: OpenFailureKind,
) -> RetryDirective {
    match failure {
        OpenFailureKind::Transient => {
            target_retry_directive(target, current_failure_streak, TargetRetryClass::Transient)
        }
        OpenFailureKind::SuspectPartial => {
            quarantine_probe_directive(requested_effective, current_failure_streak)
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
    use crate::{GATE_MAX_SAMPLE_GAP_SECONDS, GATE_QUARANTINE_PROBE_CAP_SECONDS};

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

    /// S1 gate contract: the quarantine probe is `min(30s, effective watch
    /// interval)` EXACTLY -- an upper bound, not a target to jitter around.
    ///
    /// This is written as an exact equality on purpose. The shipped code
    /// selected the same base and then added `deterministic_jitter_v1`, which
    /// is up to +10%, so a 30s selection could be dispatched at up to 33s.
    /// The first assertion below proves the jitter this path used to add is
    /// genuinely non-zero for at least one of these streaks, so the equality
    /// assertions cannot pass by coincidence: restoring the jitter term makes
    /// them fail.
    #[test]
    fn the_quarantine_probe_is_exactly_min_thirty_and_the_interval_without_jitter() {
        let nb = target();
        let cap = Duration::from_secs(GATE_QUARANTINE_PROBE_CAP_SECONDS);
        assert!(
            (0..6).any(|streak| { deterministic_jitter_v1(&nb, streak + 1, cap) > Duration::ZERO }),
            "the removed jitter term must be non-zero somewhere in this range,              otherwise the equality assertions below prove nothing",
        );

        for (requested, expected) in [
            (Duration::ZERO, cap),
            (Duration::from_secs(3), Duration::from_secs(3)),
            (Duration::from_secs(10), Duration::from_secs(10)),
            (Duration::from_secs(29), Duration::from_secs(29)),
            (Duration::from_secs(30), cap),
            (Duration::from_secs(31), cap),
            (Duration::from_secs(60), cap),
            (Duration::from_secs(3_600), cap),
        ] {
            for streak in 0..6 {
                let directive =
                    retry_directive(&nb, requested, streak, OpenFailureKind::SuspectPartial);
                assert_eq!(
                    directive.delay, expected,
                    "requested={requested:?} streak={streak}"
                );
                assert_eq!(directive.next_failure_streak, streak + 1);
                assert_eq!(directive.mode, RetryMode::Automatic);
                assert!(directive.clears_fatal_diagnostic);
            }
        }
    }

    /// Every campus, every legal local watch interval: the dispatched probe
    /// delay never exceeds `min(30s, interval)`. `<=` rather than `==` here so
    /// the bound itself is pinned independently of the exact-value test above.
    #[test]
    fn no_campus_and_no_legal_interval_can_push_the_probe_past_its_bound() {
        for campus in ["NB", "NK", "CM"] {
            let target = TermCampusKey::try_new("92026", campus).expect("target");
            for seconds in LOCAL_MINIMUM_WATCH_OPEN_INTERVAL_SECONDS
                ..=LOCAL_MAXIMUM_WATCH_OPEN_INTERVAL_SECONDS
            {
                let requested = Duration::from_secs(u64::from(seconds));
                let bound = requested.min(Duration::from_secs(GATE_QUARANTINE_PROBE_CAP_SECONDS));
                for streak in 0..4 {
                    let delay = retry_directive(
                        &target,
                        requested,
                        streak,
                        OpenFailureKind::SuspectPartial,
                    )
                    .delay;
                    assert!(
                        delay <= bound,
                        "{campus} interval={seconds}s streak={streak} delay={delay:?} > {bound:?}"
                    );
                    assert!(
                        delay < Duration::from_secs(GATE_MAX_SAMPLE_GAP_SECONDS as u64),
                        "{campus} probe must stay inside the gate max sample gap"
                    );
                }
            }
        }
    }

    /// The fix is narrow: only the quarantine probe path loses its jitter.
    /// Transient, content, fatal and rate-limited retries are untouched.
    #[test]
    fn removing_the_probe_jitter_does_not_disturb_the_other_retry_paths() {
        let nb = target();
        for streak in 0..5 {
            let base = Duration::from_secs(u64::from(
                NB_TRANSIENT_BACKOFF_SECONDS[usize::try_from(streak).expect("streak")],
            ));
            assert_eq!(
                retry_directive(
                    &nb,
                    Duration::from_secs(30),
                    streak,
                    OpenFailureKind::Transient
                )
                .delay,
                base + deterministic_jitter_v1(&nb, streak + 1, base),
            );
        }
        let content_base = Duration::from_secs(u64::from(CONTENT_BACKOFF_SECONDS[0]));
        assert_eq!(
            retry_directive(
                &nb,
                Duration::from_secs(30),
                0,
                OpenFailureKind::UnsafeEmpty
            )
            .delay,
            content_base + deterministic_jitter_v1(&nb, 1, content_base),
        );
        let limited = retry_directive(
            &nb,
            Duration::from_secs(30),
            0,
            OpenFailureKind::RateLimited {
                retry_after: Some(Duration::from_secs(7)),
            },
        );
        assert_eq!(limited.delay, Duration::from_secs(7));
        assert_eq!(limited.mode, RetryMode::OriginCircuit);
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
